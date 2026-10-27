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
mod tests;
