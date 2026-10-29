use super::*;

#[test]
fn category_round_trips_through_str_display_and_serde() {
    for c in Category::ALL {
        assert_eq!(c.as_str().parse::<Category>().unwrap(), c);
        assert_eq!(c.to_string(), c.as_str());
        assert_eq!(serde_json::to_value(c).unwrap(), c.as_str());
        assert!(!c.title().is_empty());
        assert!(!c.description().is_empty());
    }
    assert!("speed".parse::<Category>().is_err());
}

#[test]
fn run_status_parses_known_values_only() {
    for s in [
        RunStatus::Queued,
        RunStatus::Running,
        RunStatus::Completed,
        RunStatus::Failed,
        RunStatus::Cancelled,
    ] {
        assert_eq!(s.as_str().parse::<RunStatus>().unwrap(), s);
    }
    assert!("done".parse::<RunStatus>().is_err());
}

#[test]
fn fingerprint_uses_placeholders_without_gpu() {
    let mut profile = NewHardwareProfile {
        cpu: "Ryzen".into(),
        gpu: None,
        vram_total_bytes: None,
        ram_total_bytes: 64,
        backend: "cpu".into(),
        driver_version: Some("ignored".into()),
    };
    assert_eq!(profile.fingerprint(), "Ryzen|-|0|64|cpu");
    profile.gpu = Some("RX".into());
    profile.vram_total_bytes = Some(24);
    assert_eq!(profile.fingerprint(), "Ryzen|RX|24|64|cpu");
}
