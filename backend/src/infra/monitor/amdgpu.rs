use std::fs;
use std::path::{Path, PathBuf};

use super::{GpuMonitor, GpuSnapshot};

const AMD_VENDOR_ID: &str = "0x1002";
const PCI_IDS_PATHS: [&str; 2] = ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids"];

/// Reads AMD GPU telemetry from the amdgpu driver's sysfs files.
pub struct AmdGpuMonitor {
    device_dir: PathBuf,
    name: String,
}

impl AmdGpuMonitor {
    /// Picks the AMD card with the most VRAM under `drm_root` (normally `/sys/class/drm`),
    /// which skips integrated GPUs that only have a small carve-out.
    pub fn discover(drm_root: &Path) -> Option<Self> {
        let device_dir = fs::read_dir(drm_root)
            .ok()?
            .filter_map(Result::ok)
            .filter(|e| is_card_dir(&e.file_name().to_string_lossy()))
            .map(|e| e.path().join("device"))
            .filter(|d| read_trimmed(&d.join("vendor")).as_deref() == Some(AMD_VENDOR_ID))
            .filter_map(|d| read_u64(&d.join("mem_info_vram_total")).map(|v| (d, v)))
            .max_by_key(|(_, vram)| *vram)
            .map(|(d, _)| d)?;

        let name = gpu_name(&device_dir);
        Some(Self { device_dir, name })
    }

    fn power_w(&self) -> Option<f64> {
        let hwmon = fs::read_dir(self.device_dir.join("hwmon"))
            .ok()?
            .filter_map(Result::ok)
            .next()?
            .path();
        ["power1_average", "power1_input"]
            .iter()
            .find_map(|f| read_u64(&hwmon.join(f)))
            .map(|microwatts| microwatts as f64 / 1_000_000.0)
    }
}

impl GpuMonitor for AmdGpuMonitor {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn driver_version(&self) -> Option<String> {
        read_trimmed(Path::new("/sys/module/amdgpu/version"))
            .map(|v| format!("amdgpu {v}"))
            .or_else(|| Some("amdgpu".to_string()))
    }

    fn snapshot(&self) -> Option<GpuSnapshot> {
        Some(GpuSnapshot {
            name: self.name.clone(),
            vram_used_bytes: read_u64(&self.device_dir.join("mem_info_vram_used"))?,
            vram_total_bytes: read_u64(&self.device_dir.join("mem_info_vram_total"))?,
            busy_pct: read_u64(&self.device_dir.join("gpu_busy_percent")).map(|v| v as f64),
            power_w: self.power_w(),
        })
    }
}

fn is_card_dir(name: &str) -> bool {
    name.strip_prefix("card")
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn read_u64(path: &Path) -> Option<u64> {
    read_trimmed(path)?.parse().ok()
}

fn gpu_name(device_dir: &Path) -> String {
    if let Some(name) = read_trimmed(&device_dir.join("product_name")).filter(|n| !n.is_empty()) {
        return name;
    }
    let device_id = read_trimmed(&device_dir.join("device")).unwrap_or_default();
    let device_id = device_id.trim_start_matches("0x");
    PCI_IDS_PATHS
        .iter()
        .find_map(|p| fs::read_to_string(p).ok())
        .and_then(|db| lookup_pci_name(&db, "1002", device_id))
        .map(|n| format!("AMD {n}"))
        .unwrap_or_else(|| format!("AMD GPU 1002:{device_id}"))
}

/// Finds a device name in the `pci.ids` database format.
fn lookup_pci_name(db: &str, vendor: &str, device: &str) -> Option<String> {
    let mut in_vendor = false;
    for line in db.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if !line.starts_with('\t') {
            in_vendor = line.starts_with(vendor);
            continue;
        }
        if in_vendor && !line.starts_with("\t\t") {
            let entry = line.trim_start_matches('\t');
            if let Some(name) = entry.strip_prefix(device) {
                return Some(name.trim().to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests;
