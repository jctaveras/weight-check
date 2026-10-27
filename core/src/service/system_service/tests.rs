use super::*;
use crate::error::AppError;
use crate::service::testing::{FakeGpu, FakeHealth, FakeHost, InMemoryHardware};

fn service(gpu: bool, healthy: bool) -> SystemService {
    SystemService::new(
        gpu.then(|| Arc::new(FakeGpu) as Arc<dyn GpuMonitor>),
        Arc::new(FakeHost),
        Arc::new(InMemoryHardware::default()),
        Arc::new(FakeHealth { ok: healthy }),
        "vulkan".into(),
    )
}

#[tokio::test]
async fn registers_hardware_with_gpu_once() {
    let service = service(true, true);
    let first = service.register_hardware().await.unwrap();
    assert_eq!(first.cpu, "Fake CPU");
    assert_eq!(first.gpu.as_deref(), Some("Fake GPU"));
    assert_eq!(first.vram_total_bytes, Some(24 << 30));
    assert_eq!(first.driver_version.as_deref(), Some("fake 1.0"));
    assert_eq!(first.backend, "vulkan");

    let again = service.register_hardware().await.unwrap();
    assert_eq!(again.id, first.id);
}

#[tokio::test]
async fn registers_hardware_without_gpu() {
    let profile = service(false, true).register_hardware().await.unwrap();
    assert_eq!(profile.gpu, None);
    assert_eq!(profile.vram_total_bytes, None);
    assert_eq!(profile.driver_version, None);
}

#[test]
fn snapshot_includes_backend_and_gpu() {
    let snap = service(true, true).snapshot(None);
    assert_eq!(snap.backend, "vulkan");
    assert_eq!(snap.host.cpu_cores, 8);
    assert_eq!(snap.gpu.unwrap().power_w, Some(200.0));
    assert!(service(false, true).snapshot(None).gpu.is_none());
}

#[tokio::test]
async fn health_propagates_database_errors() {
    service(false, true).health().await.unwrap();
    assert!(matches!(
        service(false, false).health().await,
        Err(AppError::Database(_))
    ));
}
