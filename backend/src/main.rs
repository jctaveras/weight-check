use std::path::Path;
use std::sync::Arc;

use tracing_subscriber::EnvFilter;

use weight_check::config::Config;
use weight_check::infra::llama::process::{LlamaServerLauncher, detect_backend};
use weight_check::infra::monitor::amdgpu::AmdGpuMonitor;
use weight_check::infra::monitor::host::SysinfoHostMonitor;
use weight_check::infra::monitor::{GpuMonitor, HostMonitor};
use weight_check::presentation::{self, AppState};
use weight_check::repository::sqlite::SqliteStore;
use weight_check::service::benchmark_service::BenchmarkService;
use weight_check::service::benchmarks::Registry;
use weight_check::service::model_service::ModelService;
use weight_check::service::system_service::SystemService;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("weight_check=info,tower_http=info")),
        )
        .init();

    let config = Config::from_env()?;

    let store = Arc::new(SqliteStore::connect(&config.database_url).await?);
    tracing::info!(url = %config.database_url, "database ready");

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
        store.clone(),
        models.clone(),
        gpu,
        host,
        hardware.id,
    ));

    let app = presentation::router(AppState {
        models: models.clone(),
        benchmarks,
        system,
    });

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!("listening on http://{}", config.bind_addr);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("shutting down");
        })
        .await?;

    models.shutdown().await;
    Ok(())
}
