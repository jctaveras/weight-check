use std::sync::Arc;

use serde::Serialize;

use crate::domain::{HardwareProfile, NewHardwareProfile};
use crate::error::AppResult;
use crate::infra::monitor::{GpuMonitor, GpuSnapshot, HostMonitor, HostSnapshot};
use crate::repository::{HardwareRepository, HealthRepository};

#[derive(Debug, Clone, Serialize)]
pub struct SystemSnapshot {
    pub os: Option<String>,
    pub backend: String,
    pub host: HostSnapshot,
    pub gpu: Option<GpuSnapshot>,
}

pub struct SystemService {
    gpu: Option<Arc<dyn GpuMonitor>>,
    host: Arc<dyn HostMonitor>,
    hardware: Arc<dyn HardwareRepository>,
    health: Arc<dyn HealthRepository>,
    backend: String,
}

impl SystemService {
    pub fn new(
        gpu: Option<Arc<dyn GpuMonitor>>,
        host: Arc<dyn HostMonitor>,
        hardware: Arc<dyn HardwareRepository>,
        health: Arc<dyn HealthRepository>,
        backend: String,
    ) -> Self {
        Self {
            gpu,
            host,
            hardware,
            health,
            backend,
        }
    }

    /// Detects the current machine and stores (or reuses) its hardware profile.
    pub async fn register_hardware(&self) -> AppResult<HardwareProfile> {
        let host = self.host.snapshot(None);
        let gpu = self.gpu.as_ref().and_then(|g| g.snapshot());
        self.hardware
            .upsert(NewHardwareProfile {
                cpu: host.cpu,
                gpu: self.gpu.as_ref().map(|g| g.name()),
                vram_total_bytes: gpu.map(|g| g.vram_total_bytes as i64),
                ram_total_bytes: host.ram_total_bytes as i64,
                backend: self.backend.clone(),
                driver_version: self.gpu.as_ref().and_then(|g| g.driver_version()),
            })
            .await
    }

    pub fn snapshot(&self, pid: Option<u32>) -> SystemSnapshot {
        SystemSnapshot {
            os: self.host.os_version(),
            backend: self.backend.clone(),
            host: self.host.snapshot(pid),
            gpu: self.gpu.as_ref().and_then(|g| g.snapshot()),
        }
    }

    pub async fn health(&self) -> AppResult<()> {
        self.health.ping().await
    }
}

#[cfg(test)]
mod tests {
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
}
