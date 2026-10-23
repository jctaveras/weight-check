//! In-memory fakes for service unit tests.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use super::model_service::{ActiveModel, ActivityGuardFactory, Checkout, ModelSession};
use crate::domain::{
    HardwareProfile, LoadSettings, Metric, ModelInfo, NewHardwareProfile, NewModel, NewRun, Run,
    RunFilter, RunStatus,
};
use crate::error::{AppError, AppResult};
use crate::infra::llama::{
    CompletionRequest, CompletionResponse, LaunchArgs, LlamaApi, RunningServer, ServerLauncher,
    StreamedCompletion, Timings,
};
use crate::infra::monitor::{GpuMonitor, GpuSnapshot, HostMonitor, HostSnapshot};
use crate::repository::{HardwareRepository, HealthRepository, ModelRepository, RunRepository};

#[derive(Default)]
pub struct InMemoryModels {
    models: Mutex<HashMap<Uuid, ModelInfo>>,
}

#[async_trait]
impl ModelRepository for InMemoryModels {
    async fn upsert(&self, m: NewModel) -> AppResult<ModelInfo> {
        let info = ModelInfo {
            id: Uuid::new_v4(),
            name: m.name,
            file_path: m.file_path,
            architecture: m.architecture,
            quantization: m.quantization,
            size_label: m.size_label,
            block_count: m.block_count,
            file_size_bytes: m.file_size_bytes,
            created_at: Utc::now(),
        };
        self.models.lock().unwrap().insert(info.id, info.clone());
        Ok(info)
    }

    async fn get(&self, id: Uuid) -> AppResult<Option<ModelInfo>> {
        Ok(self.models.lock().unwrap().get(&id).cloned())
    }
}

#[derive(Default)]
pub struct InMemoryRuns {
    runs: Mutex<HashMap<Uuid, (Run, Vec<Metric>)>>,
}

impl InMemoryRuns {
    fn update(&self, id: Uuid, f: impl FnOnce(&mut Run, &mut Vec<Metric>)) -> AppResult<()> {
        let mut runs = self.runs.lock().unwrap();
        let (run, metrics) = runs
            .get_mut(&id)
            .ok_or_else(|| AppError::NotFound(format!("run {id}")))?;
        f(run, metrics);
        Ok(())
    }
}

#[async_trait]
impl RunRepository for InMemoryRuns {
    async fn create(&self, r: NewRun) -> AppResult<Run> {
        let run = Run {
            id: Uuid::new_v4(),
            model_id: r.model_id,
            hardware_id: r.hardware_id,
            category: r.category,
            benchmark_name: r.benchmark_name,
            status: RunStatus::Queued,
            config: r.config,
            details: None,
            error: None,
            started_at: None,
            finished_at: None,
            created_at: Utc::now(),
        };
        self.runs
            .lock()
            .unwrap()
            .insert(run.id, (run.clone(), Vec::new()));
        Ok(run)
    }

    async fn mark_running(&self, id: Uuid) -> AppResult<()> {
        self.update(id, |run, _| {
            run.status = RunStatus::Running;
            run.started_at = Some(Utc::now());
        })
    }

    async fn complete(&self, id: Uuid, details: Value, m: &[Metric]) -> AppResult<()> {
        self.update(id, |run, metrics| {
            run.status = RunStatus::Completed;
            run.details = Some(details);
            run.finished_at = Some(Utc::now());
            *metrics = m.to_vec();
        })
    }

    async fn fail(&self, id: Uuid, error: &str) -> AppResult<()> {
        self.update(id, |run, _| {
            run.status = RunStatus::Failed;
            run.error = Some(error.to_string());
            run.finished_at = Some(Utc::now());
        })
    }

    async fn get(&self, id: Uuid) -> AppResult<Option<Run>> {
        Ok(self.runs.lock().unwrap().get(&id).map(|(r, _)| r.clone()))
    }

    async fn list(&self, _filter: &RunFilter) -> AppResult<Vec<Run>> {
        Ok(self
            .runs
            .lock()
            .unwrap()
            .values()
            .map(|(r, _)| r.clone())
            .collect())
    }

    async fn metrics(&self, run_id: Uuid) -> AppResult<Vec<Metric>> {
        Ok(self
            .runs
            .lock()
            .unwrap()
            .get(&run_id)
            .map(|(_, m)| m.clone())
            .unwrap_or_default())
    }
}

/// Deterministic llama-server stand-in: 50 tok/s generation, 500 tok/s prompt.
#[derive(Default)]
pub struct FakeLlama {
    pub delay: Option<Duration>,
    pub fail: bool,
}

impl FakeLlama {
    async fn simulate(&self) -> AppResult<()> {
        if let Some(d) = self.delay {
            tokio::time::sleep(d).await;
        }
        if self.fail {
            return Err(AppError::Llama("boom".into()));
        }
        Ok(())
    }
}

#[async_trait]
impl LlamaApi for FakeLlama {
    async fn health(&self) -> AppResult<bool> {
        Ok(true)
    }

    async fn completion(&self, req: &CompletionRequest) -> AppResult<CompletionResponse> {
        self.simulate().await?;
        Ok(CompletionResponse {
            content: "ok".into(),
            tokens_predicted: req.n_predict as u64,
            tokens_evaluated: 10,
            timings: Timings {
                prompt_n: 10.0,
                prompt_ms: 20.0,
                prompt_per_second: 500.0,
                predicted_n: req.n_predict as f64,
                predicted_ms: req.n_predict as f64 * 20.0,
                predicted_per_second: 50.0,
            },
        })
    }

    async fn completion_stream(&self, req: &CompletionRequest) -> AppResult<StreamedCompletion> {
        self.simulate().await?;
        let offsets: Vec<f64> = (0..req.n_predict)
            .map(|i| 100.0 + i as f64 * 20.0)
            .collect();
        Ok(StreamedCompletion {
            ttft_ms: offsets.first().copied(),
            total_ms: offsets.last().copied().unwrap_or(0.0) + 1.0,
            token_offsets_ms: offsets,
            timings: None,
        })
    }
}

pub fn fake_model() -> ModelInfo {
    ModelInfo {
        id: Uuid::new_v4(),
        name: "fake".into(),
        file_path: "/models/fake.gguf".into(),
        architecture: Some("llama".into()),
        quantization: Some("Q4_K_M".into()),
        size_label: Some("1B".into()),
        block_count: Some(16),
        file_size_bytes: 1,
        created_at: Utc::now(),
    }
}

pub struct FakeSession {
    model: Option<ActiveModel>,
    guards: ActivityGuardFactory,
}

impl FakeSession {
    pub fn with(llama: Option<FakeLlama>) -> Self {
        Self {
            model: llama.map(|llama| ActiveModel {
                model: fake_model(),
                settings: LoadSettings {
                    ctx_size: 4096,
                    n_gpu_layers: 999,
                    parallel: 1,
                    threads: None,
                },
                pid: None,
                load_time_ms: 1000.0,
                vram_before_load_bytes: None,
                vram_after_load_bytes: None,
                loaded_at: Utc::now(),
                api: Arc::new(llama),
            }),
            guards: ActivityGuardFactory::default(),
        }
    }

    pub fn loaded() -> Self {
        Self::with(Some(FakeLlama::default()))
    }

    pub fn loaded_slow() -> Self {
        Self::with(Some(FakeLlama {
            delay: Some(Duration::from_millis(20)),
            fail: false,
        }))
    }

    pub fn loaded_failing() -> Self {
        Self::with(Some(FakeLlama {
            delay: None,
            fail: true,
        }))
    }

    pub fn empty() -> Self {
        Self::with(None)
    }

    /// The checked-out model, for building a `BenchmarkContext` directly.
    pub fn active(&self) -> ActiveModel {
        self.model.clone().expect("session has a model")
    }

    pub fn with_parallel(mut self, parallel: u32) -> Self {
        if let Some(m) = self.model.as_mut() {
            m.settings.parallel = parallel;
        }
        self
    }
}

impl ModelSession for FakeSession {
    fn checkout(&self) -> Option<Checkout> {
        Some(Checkout {
            model: self.model.clone()?,
            guard: self.guards.guard(),
        })
    }
}

pub struct FakeHost;

impl HostMonitor for FakeHost {
    fn snapshot(&self, _pid: Option<u32>) -> HostSnapshot {
        HostSnapshot {
            cpu: "Fake CPU".into(),
            cpu_cores: 8,
            cpu_usage_pct: 0.0,
            ram_total_bytes: 32 << 30,
            ram_used_bytes: 8 << 30,
            process_rss_bytes: None,
        }
    }

    fn os_version(&self) -> Option<String> {
        None
    }
}

#[derive(Clone, Default)]
pub struct FakeLauncher {
    stopped: Arc<AtomicUsize>,
}

impl FakeLauncher {
    pub fn stopped(&self) -> usize {
        self.stopped.load(Ordering::SeqCst)
    }
}

struct FakeServer {
    stopped: Arc<AtomicUsize>,
}

#[async_trait]
impl RunningServer for FakeServer {
    fn api(&self) -> Arc<dyn LlamaApi> {
        Arc::new(FakeLlama::default())
    }

    fn pid(&self) -> Option<u32> {
        None
    }

    async fn stop(self: Box<Self>) {
        self.stopped.fetch_add(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl ServerLauncher for FakeLauncher {
    async fn launch(&self, _args: &LaunchArgs) -> AppResult<Box<dyn RunningServer>> {
        Ok(Box::new(FakeServer {
            stopped: self.stopped.clone(),
        }))
    }
}

/// Hardware profiles deduplicated by fingerprint, like the real table.
#[derive(Default)]
pub struct InMemoryHardware {
    profiles: Mutex<HashMap<String, HardwareProfile>>,
}

#[async_trait]
impl HardwareRepository for InMemoryHardware {
    async fn upsert(&self, p: NewHardwareProfile) -> AppResult<HardwareProfile> {
        let fingerprint = p.fingerprint();
        let mut profiles = self.profiles.lock().unwrap();
        let profile = profiles
            .entry(fingerprint.clone())
            .or_insert_with(|| HardwareProfile {
                id: Uuid::new_v4(),
                cpu: p.cpu,
                gpu: p.gpu,
                vram_total_bytes: p.vram_total_bytes,
                ram_total_bytes: p.ram_total_bytes,
                backend: p.backend,
                driver_version: p.driver_version,
                fingerprint,
            });
        Ok(profile.clone())
    }
}

pub struct FakeHealth {
    pub ok: bool,
}

#[async_trait]
impl HealthRepository for FakeHealth {
    async fn ping(&self) -> AppResult<()> {
        if self.ok {
            Ok(())
        } else {
            Err(AppError::Database("connection refused".into()))
        }
    }
}

/// A 24 GiB GPU with 2 GiB used, 90% busy, drawing 200 W.
pub struct FakeGpu;

impl GpuMonitor for FakeGpu {
    fn name(&self) -> String {
        "Fake GPU".into()
    }

    fn driver_version(&self) -> Option<String> {
        Some("fake 1.0".into())
    }

    fn snapshot(&self) -> Option<GpuSnapshot> {
        Some(GpuSnapshot {
            name: self.name(),
            vram_used_bytes: 2 << 30,
            vram_total_bytes: 24 << 30,
            busy_pct: Some(90.0),
            power_w: Some(200.0),
        })
    }
}
