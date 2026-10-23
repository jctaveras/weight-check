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
mod tests {
    use super::*;

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn discovers_discrete_gpu_and_reads_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let igpu = root.path().join("card0/device");
        write(&igpu.join("vendor"), "0x1002\n");
        write(&igpu.join("mem_info_vram_total"), "536870912\n");
        write(&igpu.join("mem_info_vram_used"), "1000\n");

        let dgpu = root.path().join("card1/device");
        write(&dgpu.join("vendor"), "0x1002\n");
        write(&dgpu.join("product_name"), "Radeon Test\n");
        write(&dgpu.join("mem_info_vram_total"), "25753026560\n");
        write(&dgpu.join("mem_info_vram_used"), "2147483648\n");
        write(&dgpu.join("gpu_busy_percent"), "42\n");
        write(&dgpu.join("hwmon/hwmon3/power1_average"), "150000000\n");

        // Connector entries must be ignored.
        fs::create_dir_all(root.path().join("card1-DP-1")).unwrap();

        let monitor = AmdGpuMonitor::discover(root.path()).expect("gpu found");
        let snap = monitor.snapshot().expect("snapshot");
        assert_eq!(snap.name, "Radeon Test");
        assert_eq!(snap.vram_total_bytes, 25_753_026_560);
        assert_eq!(snap.vram_used_bytes, 2_147_483_648);
        assert_eq!(snap.busy_pct, Some(42.0));
        assert_eq!(snap.power_w, Some(150.0));
    }

    #[test]
    fn no_amd_gpu_returns_none() {
        let root = tempfile::tempdir().unwrap();
        write(&root.path().join("card0/device/vendor"), "0x10de\n");
        assert!(AmdGpuMonitor::discover(root.path()).is_none());
    }

    #[test]
    fn falls_back_to_device_id_and_power_input() {
        let root = tempfile::tempdir().unwrap();
        let dev = root.path().join("card0/device");
        write(&dev.join("vendor"), "0x1002\n");
        write(&dev.join("device"), "0xfffe\n");
        write(&dev.join("mem_info_vram_total"), "1024\n");
        write(&dev.join("mem_info_vram_used"), "512\n");
        write(&dev.join("hwmon/hwmon0/power1_input"), "50000000\n");

        let monitor = AmdGpuMonitor::discover(root.path()).expect("gpu found");
        // 0xfffe is not a real device, so no pci.ids entry matches.
        assert_eq!(monitor.name(), "AMD GPU 1002:fffe");
        assert!(monitor.driver_version().unwrap().starts_with("amdgpu"));
        let snap = monitor.snapshot().unwrap();
        assert_eq!(snap.busy_pct, None);
        assert_eq!(snap.power_w, Some(50.0));

        fs::remove_file(dev.join("mem_info_vram_used")).unwrap();
        assert!(monitor.snapshot().is_none());
    }

    #[test]
    fn card_dir_names() {
        assert!(is_card_dir("card0"));
        assert!(is_card_dir("card12"));
        assert!(!is_card_dir("card"));
        assert!(!is_card_dir("card1-DP-1"));
        assert!(!is_card_dir("renderD128"));
    }

    #[test]
    fn pci_ids_lookup() {
        let db = "# comment\n1002  Advanced Micro Devices\n\t744c  Navi 31 [Radeon RX 7900 XTX]\n\t\t1002 0e3b  Sub\n10de  NVIDIA\n\t744c  Wrong\n";
        assert_eq!(
            lookup_pci_name(db, "1002", "744c").as_deref(),
            Some("Navi 31 [Radeon RX 7900 XTX]")
        );
        assert_eq!(lookup_pci_name(db, "1002", "ffff"), None);
    }
}
