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
