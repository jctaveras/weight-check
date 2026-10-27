use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::time::Instant;
use uuid::Uuid;

use crate::domain::{LoadSettings, ModelInfo, NewModel};
use crate::error::{AppError, AppResult};
use crate::infra::gguf;
use crate::infra::llama::{LaunchArgs, LlamaApi, RunningServer, ServerLauncher};
use crate::infra::monitor::GpuMonitor;
use crate::repository::ModelRepository;

pub const DEFAULT_CTX_SIZE: u32 = 4096;
/// Large enough to offload every layer of any current model.
pub const DEFAULT_GPU_LAYERS: i32 = 999;
pub const DEFAULT_PARALLEL: u32 = 1;

/// Options for loading a model; unset fields fall back to the defaults above.
#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    pub model_id: Uuid,
    pub ctx_size: Option<u32>,
    pub n_gpu_layers: Option<i32>,
    pub parallel: Option<u32>,
    pub threads: Option<i32>,
}

/// The currently loaded model and how it was loaded.
#[derive(Clone, Serialize)]
pub struct ActiveModel {
    pub model: ModelInfo,
    pub settings: LoadSettings,
    pub pid: Option<u32>,
    pub load_time_ms: f64,
    pub vram_before_load_bytes: Option<u64>,
    pub vram_after_load_bytes: Option<u64>,
    pub loaded_at: DateTime<Utc>,
    #[serde(skip)]
    pub api: Arc<dyn LlamaApi>,
}

#[derive(Serialize)]
pub struct ModelStatus {
    pub loaded: Option<ActiveModel>,
    pub loading_model_id: Option<Uuid>,
    pub active_runs: usize,
    pub idle_secs: f64,
    pub idle_timeout_secs: Option<u64>,
}

/// Something using the loaded model. While any `Checkout` is alive, the model is not
/// unloaded (neither by request nor by the idle watchdog).
pub struct Checkout {
    pub model: ActiveModel,
    pub guard: ActivityGuard,
}

/// Access to the loaded model for other services.
pub trait ModelSession: Send + Sync {
    fn checkout(&self) -> Option<Checkout>;
}

struct Activity {
    last: Mutex<Instant>,
    active: AtomicUsize,
}

impl Activity {
    fn touch(&self) {
        *self.last.lock().expect("activity mutex poisoned") = Instant::now();
    }

    fn idle_for(&self) -> Duration {
        self.last.lock().expect("activity mutex poisoned").elapsed()
    }
}

impl Default for Activity {
    fn default() -> Self {
        Self {
            last: Mutex::new(Instant::now()),
            active: AtomicUsize::new(0),
        }
    }
}

pub struct ActivityGuard(Arc<Activity>);

impl Drop for ActivityGuard {
    fn drop(&mut self) {
        self.0.touch();
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Lets test doubles of `ModelSession` hand out guards.
#[cfg(any(test, feature = "testing"))]
#[derive(Default)]
pub struct ActivityGuardFactory(Arc<Activity>);

#[cfg(any(test, feature = "testing"))]
impl ActivityGuardFactory {
    pub fn guard(&self) -> ActivityGuard {
        self.0.active.fetch_add(1, Ordering::SeqCst);
        ActivityGuard(self.0.clone())
    }
}

struct Loaded {
    active: ActiveModel,
    server: Box<dyn RunningServer>,
}

pub struct ModelService {
    models: Arc<dyn ModelRepository>,
    launcher: Arc<dyn ServerLauncher>,
    gpu: Option<Arc<dyn GpuMonitor>>,
    models_dir: PathBuf,
    idle_timeout: Option<Duration>,
    /// Serializes load/unload so only one lifecycle change happens at a time.
    lifecycle: tokio::sync::Mutex<()>,
    /// Checkouts and unloads both happen under this lock, so they cannot interleave.
    state: Mutex<Option<Loaded>>,
    loading: Mutex<Option<Uuid>>,
    activity: Arc<Activity>,
}

impl ModelService {
    pub fn new(
        models: Arc<dyn ModelRepository>,
        launcher: Arc<dyn ServerLauncher>,
        gpu: Option<Arc<dyn GpuMonitor>>,
        models_dir: PathBuf,
        idle_timeout: Option<Duration>,
    ) -> Self {
        Self {
            models,
            launcher,
            gpu,
            models_dir,
            idle_timeout,
            lifecycle: tokio::sync::Mutex::new(()),
            state: Mutex::new(None),
            loading: Mutex::new(None),
            activity: Arc::new(Activity::default()),
        }
    }

    /// Scans the models directory for GGUF files and registers them.
    pub async fn list_available(&self) -> AppResult<Vec<ModelInfo>> {
        let dir = self.models_dir.clone();
        let found = tokio::task::spawn_blocking(move || scan_models_dir(&dir))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??;

        let mut models = Vec::with_capacity(found.len());
        for m in found {
            models.push(self.models.upsert(m).await?);
        }
        models.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(models)
    }

    pub async fn load(&self, opts: LoadOptions) -> AppResult<ActiveModel> {
        let settings = LoadSettings {
            ctx_size: opts.ctx_size.unwrap_or(DEFAULT_CTX_SIZE),
            n_gpu_layers: opts.n_gpu_layers.unwrap_or(DEFAULT_GPU_LAYERS),
            parallel: opts.parallel.unwrap_or(DEFAULT_PARALLEL),
            threads: opts.threads,
        };
        validate_settings(&settings)?;

        let model = self
            .models
            .get(opts.model_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("model {}", opts.model_id)))?;
        if !Path::new(&model.file_path).is_file() {
            return Err(AppError::NotFound(format!(
                "model file {}",
                model.file_path
            )));
        }

        let _lifecycle = self.lifecycle.lock().await;
        if let Some(previous) = self.take_loaded()? {
            tracing::info!(model = %previous.active.model.name, "unloading before new load");
            previous.server.stop().await;
        }

        *self.loading.lock().expect("loading mutex poisoned") = Some(model.id);
        let vram_before_load_bytes = self.vram_used();
        let started = Instant::now();
        let result = self
            .launcher
            .launch(&LaunchArgs {
                model_path: PathBuf::from(&model.file_path),
                settings: settings.clone(),
            })
            .await;
        *self.loading.lock().expect("loading mutex poisoned") = None;
        let server = result?;

        let active = ActiveModel {
            model,
            settings,
            pid: server.pid(),
            load_time_ms: started.elapsed().as_secs_f64() * 1000.0,
            vram_before_load_bytes,
            vram_after_load_bytes: self.vram_used(),
            loaded_at: Utc::now(),
            api: server.api(),
        };
        tracing::info!(
            model = %active.model.name,
            load_time_ms = active.load_time_ms,
            "model loaded"
        );
        *self.state.lock().expect("state mutex poisoned") = Some(Loaded {
            active: active.clone(),
            server,
        });
        self.activity.touch();
        Ok(active)
    }

    pub async fn unload(&self) -> AppResult<()> {
        let _lifecycle = self.lifecycle.lock().await;
        match self.take_loaded()? {
            Some(loaded) => {
                tracing::info!(model = %loaded.active.model.name, "unloading model");
                loaded.server.stop().await;
                Ok(())
            }
            None => Err(AppError::NoModelLoaded),
        }
    }

    pub fn status(&self) -> ModelStatus {
        let loaded = self
            .state
            .lock()
            .expect("state mutex poisoned")
            .as_ref()
            .map(|l| l.active.clone());
        ModelStatus {
            loaded,
            loading_model_id: *self.loading.lock().expect("loading mutex poisoned"),
            active_runs: self.activity.active.load(Ordering::SeqCst),
            idle_secs: self.activity.idle_for().as_secs_f64(),
            idle_timeout_secs: self.idle_timeout.map(|d| d.as_secs()),
        }
    }

    /// Periodically unloads the model once it has been idle for `idle_timeout`.
    pub fn spawn_idle_watchdog(self: &Arc<Self>) {
        let Some(timeout) = self.idle_timeout else {
            return;
        };
        let service = Arc::clone(self);
        let interval = (timeout / 4).clamp(Duration::from_millis(100), Duration::from_secs(10));
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                if service.activity.idle_for() < timeout {
                    continue;
                }
                let _lifecycle = service.lifecycle.lock().await;
                if service.activity.idle_for() < timeout {
                    continue;
                }
                if let Ok(Some(loaded)) = service.take_loaded() {
                    tracing::info!(
                        model = %loaded.active.model.name,
                        idle_secs = timeout.as_secs(),
                        "idle timeout reached, unloading model"
                    );
                    loaded.server.stop().await;
                }
            }
        });
    }

    /// Stops the loaded server, if any. Used on shutdown.
    pub async fn shutdown(&self) {
        let _lifecycle = self.lifecycle.lock().await;
        let loaded = self.state.lock().expect("state mutex poisoned").take();
        if let Some(loaded) = loaded {
            loaded.server.stop().await;
        }
    }

    /// Removes the loaded model from state unless something is still using it.
    fn take_loaded(&self) -> AppResult<Option<Loaded>> {
        let mut state = self.state.lock().expect("state mutex poisoned");
        if state.is_some() && self.activity.active.load(Ordering::SeqCst) > 0 {
            return Err(AppError::Busy(
                "the loaded model is in use by a running benchmark".to_string(),
            ));
        }
        Ok(state.take())
    }

    fn vram_used(&self) -> Option<u64> {
        self.gpu
            .as_ref()
            .and_then(|g| g.snapshot())
            .map(|s| s.vram_used_bytes)
    }
}

impl ModelSession for ModelService {
    fn checkout(&self) -> Option<Checkout> {
        let state = self.state.lock().expect("state mutex poisoned");
        let model = state.as_ref()?.active.clone();
        self.activity.active.fetch_add(1, Ordering::SeqCst);
        self.activity.touch();
        Some(Checkout {
            model,
            guard: ActivityGuard(self.activity.clone()),
        })
    }
}

fn validate_settings(s: &LoadSettings) -> AppResult<()> {
    if s.ctx_size < 128 {
        return Err(AppError::Validation("ctx_size must be at least 128".into()));
    }
    if !(1..=64).contains(&s.parallel) {
        return Err(AppError::Validation(
            "parallel must be between 1 and 64".into(),
        ));
    }
    if s.n_gpu_layers < 0 {
        return Err(AppError::Validation("n_gpu_layers must be >= 0".into()));
    }
    Ok(())
}

/// Lists `*.gguf` files directly inside `dir`, skipping multimodal projectors.
fn scan_models_dir(dir: &Path) -> AppResult<Vec<NewModel>> {
    let entries = std::fs::read_dir(dir).map_err(|e| {
        AppError::Internal(format!("cannot read models dir {}: {e}", dir.display()))
    })?;

    let mut models = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_lowercase();
        if !path.is_file() || !file_name.ends_with(".gguf") || file_name.contains("mmproj") {
            continue;
        }
        let meta = match gguf::read_metadata(&path) {
            Ok(meta) => meta,
            Err(e) => {
                tracing::warn!(path = %path.display(), "skipping unreadable GGUF: {e}");
                continue;
            }
        };
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        models.push(NewModel {
            name: stem,
            file_path: path.to_string_lossy().into_owned(),
            quantization: meta.quantization().map(str::to_string),
            architecture: meta.architecture,
            size_label: meta.size_label,
            block_count: meta.block_count.map(|b| b as i32),
            file_size_bytes: entry.metadata().map(|m| m.len() as i64).unwrap_or(0),
        });
    }
    Ok(models)
}

#[cfg(test)]
mod tests;
