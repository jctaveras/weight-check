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
