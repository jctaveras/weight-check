use super::*;
use crate::service::testing::{FakeGpu, FakeHost, FakeLlama, FakeSession};

fn ctx(session: FakeSession, gpu: bool) -> BenchmarkContext {
    let mut model = session.active();
    model.vram_before_load_bytes = Some(1 << 30);
    model.vram_after_load_bytes = Some(3 << 30);
    BenchmarkContext {
        llama: model.api.clone(),
        model,
        gpu: gpu.then(|| Arc::new(FakeGpu) as Arc<dyn GpuMonitor>),
        host: Arc::new(FakeHost),
    }
}

fn slow_session() -> FakeSession {
    FakeSession::with(Some(FakeLlama {
        delay: Some(Duration::from_millis(60)),
        fail: false,
    }))
}

fn quick_config() -> Value {
    ResourceProfile
        .resolve_config(Some(json!({
            "sample_interval_ms": 50,
            "idle_ms": 120,
            "n_predict": 16,
            "repetitions": 3,
        })))
        .unwrap()
}

fn has(outcome: &BenchmarkOutcome, name: &str) -> bool {
    outcome.metrics.iter().any(|m| m.name == name)
}

#[test]
fn resolve_config_bounds() {
    assert!(ResourceProfile.resolve_config(None).is_ok());
    for bad in [
        json!({"sample_interval_ms": 49}),
        json!({"repetitions": 0}),
        json!({"n_predict": 0}),
    ] {
        assert!(matches!(
            ResourceProfile.resolve_config(Some(bad)),
            Err(AppError::Validation(_))
        ));
    }
}

#[tokio::test]
async fn samples_idle_and_busy_phases_with_gpu() {
    let outcome = ResourceProfile
        .run(&ctx(slow_session(), true), quick_config())
        .await
        .unwrap();
    let samples = outcome.details["samples"].as_array().unwrap();
    assert!(samples.iter().any(|s| s["idle"] == true));
    assert!(samples.iter().any(|s| s["idle"] == false));
    assert_eq!(outcome.details["tokens_generated"], 48.0);
    assert_eq!(outcome.details["gpu"], "Fake GPU");

    let get = |n: &str| outcome.metrics.iter().find(|m| m.name == n).unwrap().value;
    assert_eq!(get("model_vram_mb"), 2048.0);
    // 16 blocks + output layer, all offloaded with n_gpu_layers = 999.
    assert_eq!(get("gpu_layers_offloaded"), 17.0);
    assert_eq!(get("offload_pct"), 100.0);
    assert_eq!(get("vram_peak_mb"), 2048.0);
    assert_eq!(get("power_avg_w"), 200.0);
    assert_eq!(get("power_idle_avg_w"), 200.0);
    assert!(get("energy_j") > 0.0);
    assert!(has(&outcome, "tokens_per_joule"));
    assert!(has(&outcome, "ram_used_peak_mb"));
    assert!(get("generation_tps") > 0.0);
}

#[tokio::test]
async fn omits_gpu_metrics_without_gpu() {
    let outcome = ResourceProfile
        .run(&ctx(slow_session(), false), quick_config())
        .await
        .unwrap();
    assert_eq!(outcome.details["gpu"], Value::Null);
    for name in [
        "vram_peak_mb",
        "gpu_util_avg_pct",
        "power_avg_w",
        "energy_j",
    ] {
        assert!(!has(&outcome, name), "{name}");
    }
    assert!(has(&outcome, "ram_used_peak_mb"));
}

#[tokio::test]
async fn failed_workload_stops_sampler_and_errors() {
    let err = ResourceProfile
        .run(&ctx(FakeSession::loaded_failing(), true), quick_config())
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Llama(_)));
}

#[test]
fn energy_needs_two_power_samples() {
    let one = sample(0.0, false, 100.0);
    assert_eq!(energy_joules(&[&one]), None);
}

fn sample(t_ms: f64, idle: bool, power: f64) -> Sample {
    Sample {
        t_ms,
        idle,
        vram_used_bytes: Some(1024 * 1024 * 1024),
        gpu_busy_pct: Some(if idle { 0.0 } else { 90.0 }),
        power_w: Some(power),
        ram_used_bytes: 0,
        process_rss_bytes: None,
    }
}

#[test]
fn energy_is_integrated_over_busy_samples() {
    let samples = [
        sample(0.0, true, 20.0),
        sample(1000.0, false, 100.0),
        sample(2000.0, false, 100.0),
        sample(3000.0, false, 200.0),
    ];
    let metrics = sample_metrics(&samples, 300.0);
    let get = |n: &str| metrics.iter().find(|m| m.name == n).unwrap().value;
    // 1s at 100W + 1s averaging 150W
    assert_eq!(get("energy_j"), 250.0);
    assert_eq!(get("tokens_per_joule"), 300.0 / 250.0);
    assert_eq!(get("power_idle_avg_w"), 20.0);
    assert_eq!(get("gpu_util_avg_pct"), 90.0);
    assert_eq!(get("vram_peak_mb"), 1024.0);
}
