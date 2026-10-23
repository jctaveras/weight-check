//! Hardware monitoring adapters.

pub mod amdgpu;
pub mod host;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GpuSnapshot {
    pub name: String,
    pub vram_used_bytes: u64,
    pub vram_total_bytes: u64,
    pub busy_pct: Option<f64>,
    pub power_w: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostSnapshot {
    pub cpu: String,
    pub cpu_cores: usize,
    pub cpu_usage_pct: f64,
    pub ram_total_bytes: u64,
    pub ram_used_bytes: u64,
    /// Resident memory of the watched process (llama-server), if requested.
    pub process_rss_bytes: Option<u64>,
}

/// Vendor-specific GPU telemetry. AMD is implemented via sysfs; NVML can be added later.
pub trait GpuMonitor: Send + Sync {
    fn name(&self) -> String;
    fn driver_version(&self) -> Option<String>;
    fn snapshot(&self) -> Option<GpuSnapshot>;
}

pub trait HostMonitor: Send + Sync {
    fn snapshot(&self, pid: Option<u32>) -> HostSnapshot;
    fn os_version(&self) -> Option<String>;
}
