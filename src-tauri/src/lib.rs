//! The weight-check desktop app: wires the core services together and exposes them to the UI
//! as Tauri commands.

pub mod commands;

use std::path::Path;
use std::sync::Arc;

use tauri::{Manager, RunEvent};
use tracing_subscriber::EnvFilter;

use weight_check_core::config::Config;
use weight_check_core::error::AppError;
use weight_check_core::infra::llama::process::{LlamaServerLauncher, detect_backend};
use weight_check_core::infra::monitor::amdgpu::AmdGpuMonitor;
use weight_check_core::infra::monitor::host::SysinfoHostMonitor;
use weight_check_core::infra::monitor::{GpuMonitor, HostMonitor};
use weight_check_core::repository::sqlite::SqliteStore;
use weight_check_core::service::benchmark_service::BenchmarkService;
use weight_check_core::service::benchmarks::Registry;
use weight_check_core::service::model_service::ModelService;
use weight_check_core::service::system_service::SystemService;

use commands::AppState;

/// File name of the SQLite database inside the app data directory.
pub const DB_FILE_NAME: &str = "weight-check.db";

const DEFAULT_LOG_FILTER: &str = "weight_check_lib=info,weight_check_core=info";

/// Builds the services: store → monitors → services, and starts the idle watchdog.
/// Must run inside a tokio runtime (the watchdog is a spawned task).
pub async fn build_state(config: &Config, db_path: &Path) -> Result<AppState, AppError> {
    let store = Arc::new(SqliteStore::open(db_path).await?);
    tracing::info!(path = %db_path.display(), "database ready");

    let gpu: Option<Arc<dyn GpuMonitor>> = AmdGpuMonitor::discover(Path::new("/sys/class/drm"))
        .map(|g| Arc::new(g) as Arc<dyn GpuMonitor>);
    match &gpu {
        Some(g) => tracing::info!(gpu = %g.name(), "GPU monitor ready"),
        None => tracing::warn!("no supported GPU found; GPU metrics will be unavailable"),
    }
    let host: Arc<dyn HostMonitor> = Arc::new(SysinfoHostMonitor::new());

    let system = Arc::new(SystemService::new(
        gpu.clone(),
        host.clone(),
        store.clone(),
        store.clone(),
        detect_backend(&config.llama_server_bin),
    ));
    let hardware = system.register_hardware().await?;
    tracing::info!(cpu = %hardware.cpu, backend = %hardware.backend, "hardware profile registered");

    let models = Arc::new(ModelService::new(
        store.clone(),
        Arc::new(LlamaServerLauncher::new(
            config.llama_server_bin.clone(),
            config.load_timeout,
        )),
        gpu.clone(),
        config.models_dir.clone(),
        config.idle_timeout,
    ));
    models.spawn_idle_watchdog();

    let benchmarks = Arc::new(BenchmarkService::new(
        Registry::default(),
        store,
        models.clone(),
        gpu,
        host,
        hardware.id,
    ));

    Ok(AppState {
        models,
        benchmarks,
        system,
    })
}

/// Stops the loaded llama-server, if any, even while a benchmark is using it, so the child
/// process never outlives the app.
pub async fn shutdown(state: &AppState) {
    tracing::info!("shutting down");
    state.models.shutdown().await;
}

/// Reads the config from the environment and builds the services over the database in
/// `data_dir`, creating the directory if needed.
async fn init(data_dir: &Path) -> Result<AppState, Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    std::fs::create_dir_all(data_dir)?;
    Ok(build_state(&config, &data_dir.join(DB_FILE_NAME)).await?)
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));
    // `try_init` so a second call (e.g. from tests) is harmless.
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

/// Starts the desktop app. Returns once the last window is closed and the model is unloaded.
pub fn run() {
    init_tracing();

    let app = tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let state = tauri::async_runtime::block_on(init(&data_dir)).inspect_err(|e| {
                tracing::error!("startup failed: {e}");
            })?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(commands::ipc::handler())
        .build(tauri::generate_context!())
        .expect("failed to build the weight-check app");

    app.run(|app, event| {
        if let RunEvent::Exit = event
            && let Some(state) = app.try_state::<AppState>()
        {
            tauri::async_runtime::block_on(shutdown(&state));
        }
    });
}

#[cfg(test)]
mod tests;
