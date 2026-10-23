use std::sync::Mutex;

use sysinfo::{
    CpuRefreshKind, MemoryRefreshKind, Pid, ProcessRefreshKind, ProcessesToUpdate, System,
};

use super::{HostMonitor, HostSnapshot};

/// Host CPU/RAM/process telemetry backed by `sysinfo`.
pub struct SysinfoHostMonitor {
    system: Mutex<System>,
    cpu_name: String,
    cpu_cores: usize,
}

impl SysinfoHostMonitor {
    pub fn new() -> Self {
        let mut system = System::new();
        system.refresh_cpu_all();
        let cpu_name = system
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_else(|| "unknown CPU".to_string());
        let cpu_cores = system.cpus().len();
        Self {
            system: Mutex::new(system),
            cpu_name,
            cpu_cores,
        }
    }
}

impl Default for SysinfoHostMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl HostMonitor for SysinfoHostMonitor {
    fn snapshot(&self, pid: Option<u32>) -> HostSnapshot {
        let mut sys = self.system.lock().expect("sysinfo mutex poisoned");
        sys.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
        sys.refresh_cpu_specifics(CpuRefreshKind::nothing().with_cpu_usage());

        let process_rss_bytes = pid.and_then(|pid| {
            let pid = Pid::from_u32(pid);
            sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                true,
                ProcessRefreshKind::nothing().with_memory(),
            );
            sys.process(pid).map(|p| p.memory())
        });

        HostSnapshot {
            cpu: self.cpu_name.clone(),
            cpu_cores: self.cpu_cores,
            cpu_usage_pct: sys.global_cpu_usage() as f64,
            ram_total_bytes: sys.total_memory(),
            ram_used_bytes: sys.used_memory(),
            process_rss_bytes,
        }
    }

    fn os_version(&self) -> Option<String> {
        System::long_os_version()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_host_and_own_process_memory() {
        let monitor = SysinfoHostMonitor::default();
        let snap = monitor.snapshot(Some(std::process::id()));
        assert!(!snap.cpu.is_empty());
        assert!(snap.cpu_cores > 0);
        assert!(snap.ram_total_bytes > 0);
        assert!(snap.ram_used_bytes <= snap.ram_total_bytes);
        assert!(snap.process_rss_bytes.is_some_and(|rss| rss > 0));

        assert_eq!(monitor.snapshot(None).process_rss_bytes, None);
        assert!(monitor.os_version().is_some());
    }
}
