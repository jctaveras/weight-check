use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::{Semaphore, oneshot};
use uuid::Uuid;

use super::benchmarks::{BenchmarkContext, CategoryCatalog, Registry};
use super::model_service::ModelSession;
use crate::domain::{Metric, NewRun, Run, RunFilter, RunStatus};
use crate::error::{AppError, AppResult};
use crate::infra::monitor::{GpuMonitor, HostMonitor};
use crate::repository::RunRepository;

/// Most runs `compare_runs` accepts at once.
pub const MAX_COMPARE_RUNS: usize = 8;

#[derive(Debug, Clone, Serialize)]
pub struct RunWithMetrics {
    #[serde(flatten)]
    pub run: Run,
    pub metrics: Vec<Metric>,
}

pub struct BenchmarkService {
    registry: Registry,
    runs: Arc<dyn RunRepository>,
    session: Arc<dyn ModelSession>,
    gpu: Option<Arc<dyn GpuMonitor>>,
    host: Arc<dyn HostMonitor>,
    hardware_id: Uuid,
    /// One benchmark at a time: concurrent runs would skew each other's numbers.
    gate: Arc<Semaphore>,
    /// The run in progress, if any, with the sender that cancels it.
    active: Arc<Mutex<Option<ActiveRun>>>,
}

struct ActiveRun {
    id: Uuid,
    cancel: oneshot::Sender<()>,
}

impl BenchmarkService {
    pub fn new(
        registry: Registry,
        runs: Arc<dyn RunRepository>,
        session: Arc<dyn ModelSession>,
        gpu: Option<Arc<dyn GpuMonitor>>,
        host: Arc<dyn HostMonitor>,
        hardware_id: Uuid,
    ) -> Self {
        Self {
            registry,
            runs,
            session,
            gpu,
            host,
            hardware_id,
            gate: Arc::new(Semaphore::new(1)),
            active: Arc::new(Mutex::new(None)),
        }
    }

    pub fn catalog(&self) -> Vec<CategoryCatalog> {
        self.registry.catalog()
    }

    /// Validates and queues a run against the loaded model; it executes in the background.
    pub async fn start_run(&self, benchmark_id: &str, config: Option<Value>) -> AppResult<Run> {
        let benchmark = self
            .registry
            .get(benchmark_id)
            .ok_or_else(|| AppError::NotFound(format!("benchmark '{benchmark_id}'")))?;
        let descriptor = benchmark.descriptor();
        let resolved = benchmark.resolve_config(config)?;

        let permit = self
            .gate
            .clone()
            .try_acquire_owned()
            .map_err(|_| AppError::Busy("another benchmark run is in progress".into()))?;
        let checkout = self.session.checkout().ok_or(AppError::NoModelLoaded)?;

        let run = self
            .runs
            .create(NewRun {
                model_id: checkout.model.model.id,
                hardware_id: self.hardware_id,
                category: descriptor.category,
                benchmark_name: descriptor.id.to_string(),
                config: json!({
                    "benchmark": resolved,
                    "server": checkout.model.settings,
                }),
            })
            .await?;

        let ctx = BenchmarkContext {
            llama: checkout.model.api.clone(),
            model: checkout.model.clone(),
            gpu: self.gpu.clone(),
            host: self.host.clone(),
        };
        let runs = self.runs.clone();
        let run_id = run.id;
        let (cancel, cancelled) = oneshot::channel();
        *self.active.lock().unwrap() = Some(ActiveRun { id: run_id, cancel });
        let active = self.active.clone();
        tokio::spawn(async move {
            if let Err(e) = runs.mark_running(run_id).await {
                tracing::error!(%run_id, "failed to mark run as running: {e}");
            }
            tracing::info!(%run_id, benchmark = descriptor.id, "benchmark started");
            // Cancelling drops the benchmark future, which aborts in-flight llama-server
            // requests and kills any child processes it started.
            let outcome = tokio::select! {
                result = benchmark.run(&ctx, resolved) => Some(result),
                Ok(()) = cancelled => None,
            };

            // Free the gate, the model and the active slot before storing the result, so a
            // client that sees the final status can start the next run straight away.
            {
                let mut active = active.lock().unwrap();
                if active.as_ref().is_some_and(|a| a.id == run_id) {
                    *active = None;
                }
            }
            drop(permit);
            drop(checkout.guard);

            let stored = match outcome {
                Some(Ok(outcome)) => {
                    tracing::info!(%run_id, "benchmark completed");
                    runs.complete(run_id, outcome.details, &outcome.metrics)
                        .await
                }
                Some(Err(e)) => {
                    tracing::warn!(%run_id, "benchmark failed: {e}");
                    runs.fail(run_id, &e.to_string()).await
                }
                None => {
                    tracing::info!(%run_id, "benchmark cancelled");
                    runs.cancel(run_id).await.map(|_| ())
                }
            };
            if let Err(e) = stored {
                tracing::error!(%run_id, "failed to store run result: {e}");
            }
        });

        Ok(run)
    }

    /// Asks the active run to stop. It becomes `cancelled` once its task has wound down,
    /// usually within a moment.
    pub async fn cancel_run(&self, id: Uuid) -> AppResult<Run> {
        let signal = {
            let mut active = self.active.lock().unwrap();
            match active.take() {
                Some(a) if a.id == id => Some(a.cancel),
                other => {
                    *active = other;
                    None
                }
            }
        };
        let run = self.get_run(id).await?.run;
        if !matches!(run.status, RunStatus::Queued | RunStatus::Running) {
            return Err(AppError::Validation(format!(
                "run is already {}",
                run.status.as_str()
            )));
        }
        match signal {
            // The task stores `cancelled` once it has stopped (unless it finished first).
            Some(cancel) => {
                let _ = cancel.send(());
            }
            // No task behind it: a repeated cancel while the task winds down, or a run
            // orphaned by an app crash. Mark it directly; a finished run is left alone.
            None => {
                self.runs.cancel(id).await?;
            }
        }
        Ok(run)
    }

    /// Deletes a finished run and its metrics. The active run must be cancelled first.
    pub async fn delete_run(&self, id: Uuid) -> AppResult<()> {
        if self.is_active(id) {
            return Err(AppError::Busy("cancel the run before deleting it".into()));
        }
        if self.runs.delete(id).await? {
            Ok(())
        } else {
            Err(AppError::NotFound(format!("run {id}")))
        }
    }

    fn is_active(&self, id: Uuid) -> bool {
        self.active
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|a| a.id == id)
    }

    pub async fn get_run(&self, id: Uuid) -> AppResult<RunWithMetrics> {
        let run = self
            .runs
            .get(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("run {id}")))?;
        let metrics = self.runs.metrics(id).await?;
        Ok(RunWithMetrics { run, metrics })
    }

    /// Completed runs of one benchmark with their metrics, in the order of `ids`.
    pub async fn compare_runs(&self, ids: Vec<Uuid>) -> AppResult<Vec<RunWithMetrics>> {
        if !(1..=MAX_COMPARE_RUNS).contains(&ids.len()) {
            return Err(AppError::Validation(format!(
                "compare between 1 and {MAX_COMPARE_RUNS} runs"
            )));
        }
        if ids.iter().enumerate().any(|(i, id)| ids[..i].contains(id)) {
            return Err(AppError::Validation("run ids must be unique".into()));
        }

        let mut runs = Vec::with_capacity(ids.len());
        for id in ids {
            runs.push(self.get_run(id).await?);
        }
        let benchmark = &runs[0].run.benchmark_name;
        if runs
            .iter()
            .any(|r| r.run.status != RunStatus::Completed || &r.run.benchmark_name != benchmark)
        {
            return Err(AppError::Validation(
                "runs must all be completed runs of the same benchmark".into(),
            ));
        }
        Ok(runs)
    }

    pub async fn list_runs(&self, filter: RunFilter) -> AppResult<Vec<Run>> {
        if filter.limit.is_some_and(|l| !(1..=1000).contains(&l)) {
            return Err(AppError::Validation(
                "limit must be between 1 and 1000".into(),
            ));
        }
        if filter.offset.is_some_and(|o| o < 0) {
            return Err(AppError::Validation("offset must not be negative".into()));
        }
        self.runs.list(&filter).await
    }
}

#[cfg(test)]
mod tests;
