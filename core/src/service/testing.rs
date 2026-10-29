//! In-memory fakes for service unit tests. Other crates enable them with the `testing` feature.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use super::benchmarks::BenchmarkContext;
use super::benchmarks::quality::QualityDeps;
use super::model_service::{ActiveModel, ActivityGuardFactory, Checkout, ModelSession};
use crate::domain::{
    HardwareProfile, LoadSettings, Metric, ModelInfo, NewHardwareProfile, NewModel, NewRun, Run,
    RunFilter, RunStatus,
};
use crate::error::{AppError, AppResult};
use crate::infra::datasets::{DatasetFile, DatasetStore};
use crate::infra::llama::{
    ChatRequest, ChatResponse, CompletionRequest, CompletionResponse, LaunchArgs, LlamaApi,
    RunningServer, ServerLauncher, StreamedCompletion, Timings, TokenLogprob, TokenProbs,
};
use crate::infra::monitor::{GpuMonitor, GpuSnapshot, HostMonitor, HostSnapshot};
use crate::infra::sandbox::{CodeRunner, ExecResult};
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
            probs: Vec::new(),
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

    async fn chat(&self, _req: &ChatRequest) -> AppResult<ChatResponse> {
        self.simulate().await?;
        Ok(ChatResponse {
            content: "ok".into(),
            finish_reason: "stop".into(),
            prompt_tokens: 10,
            completion_tokens: 1,
            ..Default::default()
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

type ChatFn = dyn Fn(&ChatRequest) -> ChatResponse + Send + Sync;
type CompletionFn = dyn Fn(&CompletionRequest) -> Vec<(String, f64)> + Send + Sync;

/// A llama-server stand-in whose answers are computed from each request, for quality
/// benchmarks. Completions answer with one token and the given candidate log-probabilities.
#[derive(Default)]
pub struct ScriptedLlama {
    chat: Option<Box<ChatFn>>,
    completion: Option<Box<CompletionFn>>,
    /// Every chat request received, in order.
    pub chat_requests: Mutex<Vec<ChatRequest>>,
    /// Every completion prompt received, in order.
    pub prompts: Mutex<Vec<String>>,
}

impl ScriptedLlama {
    pub fn on_chat(
        mut self,
        f: impl Fn(&ChatRequest) -> ChatResponse + Send + Sync + 'static,
    ) -> Self {
        self.chat = Some(Box::new(f));
        self
    }

    pub fn on_completion(
        mut self,
        f: impl Fn(&CompletionRequest) -> Vec<(String, f64)> + Send + Sync + 'static,
    ) -> Self {
        self.completion = Some(Box::new(f));
        self
    }
}

/// A finished chat answer with `content`.
pub fn chat_reply(content: impl Into<String>) -> ChatResponse {
    ChatResponse {
        content: content.into(),
        finish_reason: "stop".into(),
        prompt_tokens: 10,
        completion_tokens: 20,
        ..Default::default()
    }
}

/// The text of a chat request's last message.
pub fn last_message(req: &ChatRequest) -> &str {
    req.messages.last().map_or("", |m| m.content.as_str())
}

#[async_trait]
impl LlamaApi for ScriptedLlama {
    async fn health(&self) -> AppResult<bool> {
        Ok(true)
    }

    async fn completion(&self, req: &CompletionRequest) -> AppResult<CompletionResponse> {
        self.prompts.lock().unwrap().push(req.prompt.clone());
        let f = self.completion.as_ref().expect("no completion script");
        let top: Vec<TokenLogprob> = f(req)
            .into_iter()
            .map(|(token, logprob)| TokenLogprob { token, logprob })
            .collect();
        let first = top.first().cloned().unwrap_or_default();
        Ok(CompletionResponse {
            content: first.token.clone(),
            tokens_predicted: 1,
            probs: vec![TokenProbs {
                token: first.token,
                logprob: first.logprob,
                top_logprobs: top,
            }],
            ..Default::default()
        })
    }

    async fn completion_stream(&self, _req: &CompletionRequest) -> AppResult<StreamedCompletion> {
        Err(AppError::Llama("ScriptedLlama does not stream".into()))
    }

    async fn chat(&self, req: &ChatRequest) -> AppResult<ChatResponse> {
        self.chat_requests.lock().unwrap().push(req.clone());
        let f = self.chat.as_ref().expect("no chat script");
        Ok(f(req))
    }
}

/// Serves dataset fixtures from a temp dir. Rows are stored as JSON Lines whatever the real
/// file's format, and `load_rows` picks the reader by extension.
pub struct FakeDatasetStore {
    dir: tempfile::TempDir,
}

impl Default for FakeDatasetStore {
    fn default() -> Self {
        Self {
            dir: tempfile::tempdir().expect("temp dir"),
        }
    }
}

impl FakeDatasetStore {
    fn fixture_path(&self, file: &DatasetFile) -> PathBuf {
        let mut path = file.cache_path(self.dir.path()).into_os_string();
        path.push(".jsonl");
        path.into()
    }

    pub fn put(&self, file: &DatasetFile, rows: &[Value]) {
        let path = self.fixture_path(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let body: String = rows.iter().map(|r| format!("{r}\n")).collect();
        std::fs::write(path, body).unwrap();
    }
}

#[async_trait]
impl DatasetStore for FakeDatasetStore {
    async fn fetch(&self, file: &DatasetFile) -> AppResult<PathBuf> {
        let path = self.fixture_path(file);
        if path.is_file() {
            Ok(path)
        } else {
            Err(AppError::Internal(format!(
                "no fixture for {}/{}",
                file.repo, file.path
            )))
        }
    }
}

type RunFn = dyn Fn(&str, &str) -> ExecResult + Send + Sync;

/// A code runner that decides each outcome from the program and its stdin.
pub struct FakeCodeRunner(Box<RunFn>);

impl FakeCodeRunner {
    pub fn new(f: impl Fn(&str, &str) -> ExecResult + Send + Sync + 'static) -> Self {
        Self(Box::new(f))
    }
}

#[async_trait]
impl CodeRunner for FakeCodeRunner {
    async fn run(&self, program: &str, stdin: &str, _timeout: Duration) -> AppResult<ExecResult> {
        Ok((self.0)(program, stdin))
    }
}

/// Quality dependencies with no fixtures and a runner that fails every program.
pub fn fake_quality_deps() -> QualityDeps {
    QualityDeps {
        datasets: Arc::new(FakeDatasetStore::default()),
        code_runner: Arc::new(FakeCodeRunner::new(|_, _| ExecResult::default())),
    }
}

pub struct FakeSession {
    model: Option<ActiveModel>,
    guards: ActivityGuardFactory,
}

impl FakeSession {
    pub fn with(llama: Option<FakeLlama>) -> Self {
        Self::with_api(llama.map(|l| Arc::new(l) as Arc<dyn LlamaApi>))
    }

    /// A session around any llama stand-in (e.g. `ScriptedLlama`).
    pub fn with_api(api: Option<Arc<dyn LlamaApi>>) -> Self {
        Self {
            model: api.map(|api| ActiveModel {
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
                api,
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

    /// A `BenchmarkContext` on this session's model, without GPU monitoring.
    pub fn context(&self) -> BenchmarkContext {
        let model = self.active();
        BenchmarkContext {
            llama: model.api.clone(),
            model,
            gpu: None,
            host: Arc::new(FakeHost),
        }
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

/// Appends a GGUF string (u64 length prefix + bytes).
pub fn put_gguf_str(buf: &mut Vec<u8>, s: &str) {
    buf.extend((s.len() as u64).to_le_bytes());
    buf.extend(s.as_bytes());
}

/// A minimal valid GGUF header with architecture, block count, file type and name.
pub fn gguf_bytes(arch: &str, block_count: u32, file_type: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend(b"GGUF");
    buf.extend(3u32.to_le_bytes());
    buf.extend(0u64.to_le_bytes());
    buf.extend(4u64.to_le_bytes());
    put_gguf_str(&mut buf, "general.architecture");
    buf.extend(8u32.to_le_bytes());
    put_gguf_str(&mut buf, arch);
    put_gguf_str(&mut buf, &format!("{arch}.block_count"));
    buf.extend(4u32.to_le_bytes());
    buf.extend(block_count.to_le_bytes());
    put_gguf_str(&mut buf, "general.file_type");
    buf.extend(4u32.to_le_bytes());
    buf.extend(file_type.to_le_bytes());
    put_gguf_str(&mut buf, "general.size_label");
    buf.extend(8u32.to_le_bytes());
    put_gguf_str(&mut buf, "1B");
    buf
}
