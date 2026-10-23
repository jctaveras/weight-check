use std::sync::Arc;

use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::Semaphore;
use uuid::Uuid;

use super::benchmarks::{BenchmarkContext, CategoryCatalog, Registry};
use super::model_service::ModelSession;
use crate::domain::{Metric, NewRun, Run, RunFilter};
use crate::error::{AppError, AppResult};
use crate::infra::monitor::{GpuMonitor, HostMonitor};
use crate::repository::RunRepository;

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
        tokio::spawn(async move {
            // Held until the run finishes: blocks other runs and model unloading.
            let _permit = permit;
            let _guard = checkout.guard;

            if let Err(e) = runs.mark_running(run_id).await {
                tracing::error!(%run_id, "failed to mark run as running: {e}");
            }
            tracing::info!(%run_id, benchmark = descriptor.id, "benchmark started");
            let stored = match benchmark.run(&ctx, resolved).await {
                Ok(outcome) => {
                    tracing::info!(%run_id, "benchmark completed");
                    runs.complete(run_id, outcome.details, &outcome.metrics)
                        .await
                }
                Err(e) => {
                    tracing::warn!(%run_id, "benchmark failed: {e}");
                    runs.fail(run_id, &e.to_string()).await
                }
            };
            if let Err(e) = stored {
                tracing::error!(%run_id, "failed to store run result: {e}");
            }
        });

        Ok(run)
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

    pub async fn list_runs(&self, filter: RunFilter) -> AppResult<Vec<Run>> {
        if filter.limit.is_some_and(|l| !(1..=1000).contains(&l)) {
            return Err(AppError::Validation(
                "limit must be between 1 and 1000".into(),
            ));
        }
        self.runs.list(&filter).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::domain::RunStatus;
    use crate::service::testing::{FakeHost, FakeSession, InMemoryRuns};

    fn service(session: FakeSession) -> (BenchmarkService, Arc<InMemoryRuns>) {
        let runs = Arc::new(InMemoryRuns::default());
        let service = BenchmarkService::new(
            Registry::default(),
            runs.clone(),
            Arc::new(session),
            None,
            Arc::new(FakeHost),
            Uuid::new_v4(),
        );
        (service, runs)
    }

    async fn wait_finished(service: &BenchmarkService, id: Uuid) -> RunWithMetrics {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let run = service.get_run(id).await.unwrap();
                if matches!(run.run.status, RunStatus::Completed | RunStatus::Failed) {
                    return run;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("run finished")
    }

    #[test]
    fn catalog_lists_all_categories() {
        let (service, _) = service(FakeSession::loaded());
        let catalog = service.catalog();
        assert_eq!(catalog.len(), 5);
        assert!(catalog.iter().all(|c| !c.benchmarks.is_empty()));
    }

    #[tokio::test]
    async fn generation_speed_run_completes_with_metrics() {
        let (service, _) = service(FakeSession::loaded());
        let run = service
            .start_run(
                "inference.generation_speed",
                Some(json!({"repetitions": 2, "warmup": false})),
            )
            .await
            .unwrap();
        assert_eq!(run.status, RunStatus::Queued);

        let done = wait_finished(&service, run.id).await;
        assert_eq!(
            done.run.status,
            RunStatus::Completed,
            "{:?}",
            done.run.error
        );
        let tps = done
            .metrics
            .iter()
            .find(|m| m.name == "generation_tps_mean")
            .expect("tps metric");
        assert_eq!(tps.value, 50.0);
        assert_eq!(done.run.config["benchmark"]["repetitions"], 2);
        assert_eq!(done.run.config["benchmark"]["n_predict"], 256);
    }

    #[tokio::test]
    async fn latency_run_measures_streaming() {
        let (service, _) = service(FakeSession::loaded());
        let run = service.start_run("inference.latency", None).await.unwrap();
        let done = wait_finished(&service, run.id).await;
        assert_eq!(
            done.run.status,
            RunStatus::Completed,
            "{:?}",
            done.run.error
        );
        assert!(done.metrics.iter().any(|m| m.name == "ttft_ms_p95"));
        assert!(done.metrics.iter().any(|m| m.name == "itl_ms_median"));
    }

    #[tokio::test]
    async fn rejects_unknown_stub_and_invalid() {
        let (service, _) = service(FakeSession::loaded());
        assert!(matches!(
            service.start_run("nope", None).await,
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(
            service.start_run("quality.reasoning", None).await,
            Err(AppError::NotImplemented(_))
        ));
        assert!(matches!(
            service
                .start_run("inference.latency", Some(json!({"bogus": 1})))
                .await,
            Err(AppError::Validation(_))
        ));
    }

    #[tokio::test]
    async fn requires_loaded_model() {
        let (service, _) = service(FakeSession::empty());
        assert!(matches!(
            service.start_run("inference.latency", None).await,
            Err(AppError::NoModelLoaded)
        ));
    }

    #[tokio::test]
    async fn only_one_run_at_a_time() {
        let (service, _) = service(FakeSession::loaded_slow());
        let first = service.start_run("inference.latency", None).await.unwrap();
        assert!(matches!(
            service.start_run("inference.latency", None).await,
            Err(AppError::Busy(_))
        ));
        wait_finished(&service, first.id).await;
        service.start_run("inference.latency", None).await.unwrap();
    }

    #[tokio::test]
    async fn failed_benchmark_is_recorded() {
        let (service, _) = service(FakeSession::loaded_failing());
        let run = service
            .start_run("inference.generation_speed", None)
            .await
            .unwrap();
        let done = wait_finished(&service, run.id).await;
        assert_eq!(done.run.status, RunStatus::Failed);
        assert!(done.run.error.unwrap().contains("boom"));
        assert!(done.metrics.is_empty());
    }

    #[tokio::test]
    async fn get_and_list_runs() {
        let (service, _) = service(FakeSession::loaded());
        assert!(matches!(
            service.get_run(Uuid::new_v4()).await,
            Err(AppError::NotFound(_))
        ));

        let run = service.start_run("inference.latency", None).await.unwrap();
        wait_finished(&service, run.id).await;
        let runs = service.list_runs(RunFilter::default()).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, run.id);

        for limit in [0, 1001] {
            assert!(matches!(
                service
                    .list_runs(RunFilter {
                        limit: Some(limit),
                        ..Default::default()
                    })
                    .await,
                Err(AppError::Validation(_))
            ));
        }
    }
}
