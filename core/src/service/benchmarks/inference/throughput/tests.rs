use std::sync::Arc;

use super::*;
use crate::service::testing::{FakeHost, FakeLlama, FakeSession};

fn ctx(session: FakeSession) -> BenchmarkContext {
    let model = session.active();
    BenchmarkContext {
        llama: model.api.clone(),
        model,
        gpu: None,
        host: Arc::new(FakeHost),
    }
}

fn metric(outcome: &BenchmarkOutcome, name: &str) -> f64 {
    outcome
        .metrics
        .iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("metric {name}"))
        .value
}

#[test]
fn resolve_config_fills_defaults_and_rejects_zeros() {
    let resolved = Throughput.resolve_config(None).unwrap();
    assert_eq!(resolved["requests_per_client"], 2);
    assert_eq!(resolved["concurrency"], Value::Null);
    for bad in [
        json!({"concurrency": 0}),
        json!({"requests_per_client": 0}),
        json!({"n_predict": 0}),
    ] {
        assert!(matches!(
            Throughput.resolve_config(Some(bad)),
            Err(AppError::Validation(_))
        ));
    }
}

#[tokio::test]
async fn concurrency_above_slots_is_flagged_as_queued() {
    let config = json!({"concurrency": 3, "requests_per_client": 2, "n_predict": 8});
    let config = Throughput.resolve_config(Some(config)).unwrap();
    let outcome = Throughput
        .run(&ctx(FakeSession::loaded()), config)
        .await
        .unwrap();
    assert_eq!(outcome.details["queued"], true);
    assert_eq!(outcome.details["total_tokens"], 3.0 * 2.0 * 8.0);
    assert_eq!(outcome.details["requests"].as_array().unwrap().len(), 6);
    assert_eq!(metric(&outcome, "concurrency"), 3.0);
    assert_eq!(metric(&outcome, "per_request_tps_mean"), 50.0);
    assert!(metric(&outcome, "throughput_tps") > 0.0);
    assert!(metric(&outcome, "requests_per_sec") > 0.0);
}

#[tokio::test]
async fn concurrency_defaults_to_server_slots() {
    let config = Throughput
        .resolve_config(Some(json!({"n_predict": 4})))
        .unwrap();
    let session = FakeSession::with(Some(FakeLlama {
        delay: Some(std::time::Duration::from_millis(1)),
        fail: false,
    }))
    .with_parallel(4);
    let outcome = Throughput.run(&ctx(session), config).await.unwrap();
    assert_eq!(outcome.details["concurrency"], 4);
    assert_eq!(outcome.details["queued"], false);
}

#[tokio::test]
async fn propagates_llama_errors() {
    let config = Throughput.resolve_config(None).unwrap();
    let err = Throughput
        .run(&ctx(FakeSession::loaded_failing()), config)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Llama(_)));
}
