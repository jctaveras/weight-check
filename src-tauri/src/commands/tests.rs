//! Command-level ports of the old router tests: the same payloads and assertions, with HTTP
//! status codes replaced by `CommandError` codes and results checked as serialized JSON.

use std::path::PathBuf;

use serde::Serialize;
use uuid::Uuid;

use super::*;
use weight_check_core::domain::NewModel;
use weight_check_core::repository::ModelRepository;
use weight_check_core::service::benchmarks::Registry;
use weight_check_core::service::testing::{
    FakeGpu, FakeHealth, FakeHost, FakeLauncher, FakeSession, InMemoryHardware, InMemoryModels,
    InMemoryRuns, gguf_bytes,
};

pub(super) struct TestApp {
    pub(super) state: AppState,
    pub(super) model_id: Uuid,
    _dir: tempfile::TempDir,
}

pub(super) async fn app(healthy: bool) -> TestApp {
    let dir = tempfile::tempdir().unwrap();
    let model_path = dir.path().join("tiny.gguf");
    std::fs::write(&model_path, gguf_bytes("llama", 8, 7)).unwrap();

    let models = Arc::new(InMemoryModels::default());
    let model = models
        .upsert(NewModel {
            name: "tiny".into(),
            file_path: model_path.to_string_lossy().into_owned(),
            architecture: None,
            quantization: None,
            size_label: None,
            block_count: Some(8),
            file_size_bytes: 1,
        })
        .await
        .unwrap();

    let state = AppState {
        models: Arc::new(ModelService::new(
            models,
            Arc::new(FakeLauncher::default()),
            None,
            PathBuf::from(dir.path()),
            None,
        )),
        benchmarks: Arc::new(BenchmarkService::new(
            Registry::default(),
            Arc::new(InMemoryRuns::default()),
            Arc::new(FakeSession::loaded()),
            None,
            Arc::new(FakeHost),
            Uuid::new_v4(),
        )),
        system: Arc::new(SystemService::new(
            Some(Arc::new(FakeGpu)),
            Arc::new(FakeHost),
            Arc::new(InMemoryHardware::default()),
            Arc::new(FakeHealth { ok: healthy }),
            "vulkan".into(),
        )),
    };
    TestApp {
        state,
        model_id: model.id,
        _dir: dir,
    }
}

/// What the UI receives: the serialized value, or the serialized error.
fn ipc<T: Serialize>(result: CommandResult<T>) -> Result<Value, Value> {
    result
        .map(|v| serde_json::to_value(v).unwrap())
        .map_err(|e| serde_json::to_value(e).unwrap())
}

fn code<T>(result: CommandResult<T>) -> &'static str {
    match result {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.code,
    }
}

#[tokio::test]
async fn health_reflects_database() {
    let body = ipc(health(&app(true).await.state).await).unwrap();
    assert_eq!(body, json!({"status": "ok"}));

    let err = ipc(health(&app(false).await.state).await).unwrap_err();
    assert_eq!(err["code"], "internal");
    assert_eq!(err["message"], "database error: connection refused");
}

#[tokio::test]
async fn system_and_catalog() {
    let app = app(true).await;
    let body = ipc(Ok(system_snapshot(&app.state))).unwrap();
    assert_eq!(body["backend"], "vulkan");
    assert_eq!(body["gpu"]["name"], "Fake GPU");

    let body = ipc(Ok(benchmark_catalog(&app.state))).unwrap();
    assert_eq!(body.as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn model_lifecycle() {
    let app = app(true).await;
    let body = ipc(list_models(&app.state).await).unwrap();
    assert_eq!(body[0]["name"], "tiny");
    assert_eq!(body[0]["quantization"], "Q8_0");

    let body = ipc(Ok(model_status(&app.state))).unwrap();
    assert_eq!(body["loaded"], Value::Null);

    let load = json!({"model_id": app.model_id, "ctx_size": 1024});
    let body = ipc(load_model(&app.state, load).await).unwrap();
    assert_eq!(body["settings"]["ctx_size"], 1024);

    let body = ipc(Ok(model_status(&app.state))).unwrap();
    assert_eq!(body["loaded"]["model"]["id"], json!(app.model_id));

    let body = ipc(unload_model(&app.state).await).unwrap();
    assert_eq!(body, Value::Null);

    let err = ipc(unload_model(&app.state).await).unwrap_err();
    assert_eq!(err["code"], "no_model_loaded");
}

#[tokio::test]
async fn load_rejects_bad_payloads() {
    let app = app(true).await;
    for bad in [
        json!("{"),
        json!({"model_id": "x"}),
        json!({"model_id": null, "extra": 1}),
        Value::Null,
    ] {
        let err = ipc(load_model(&app.state, bad.clone()).await).unwrap_err();
        assert_eq!(err["code"], "validation", "{bad}");
    }

    let unknown = json!({"model_id": Uuid::new_v4()});
    assert_eq!(code(load_model(&app.state, unknown).await), "not_found");
}

#[tokio::test]
async fn runs_start_get_and_list() {
    let app = app(true).await;
    let request = json!({"benchmark_id": "inference.latency", "config": {"repetitions": 1}});
    let run = ipc(start_run(&app.state, request).await).unwrap();
    assert_eq!(run["status"], "queued");

    let id = run["id"].as_str().unwrap();
    let body = ipc(get_run(&app.state, id).await).unwrap();
    assert_eq!(body["benchmark_name"], "inference.latency");
    assert!(body["metrics"].is_array());

    let filter = json!({"category": "inference_performance", "limit": 10});
    let body = ipc(list_runs(&app.state, Some(filter)).await).unwrap();
    assert_eq!(body.as_array().unwrap().len(), 1);

    // An empty, null or missing filter lists everything.
    for filter in [Some(json!({})), Some(Value::Null), None] {
        let body = ipc(list_runs(&app.state, filter).await).unwrap();
        assert_eq!(body.as_array().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn runs_map_errors() {
    let app = app(true).await;
    let start = |body: Value| start_run(&app.state, body);
    assert_eq!(
        code(start(json!({"benchmark_id": "quality.reasoning"})).await),
        "not_implemented"
    );
    assert_eq!(
        code(start(json!({"benchmark_id": "nope"})).await),
        "not_found"
    );
    assert_eq!(code(start(json!({"benchmark": "x"})).await), "validation");

    assert_eq!(code(get_run(&app.state, "not-a-uuid").await), "validation");
    let missing = Uuid::new_v4().to_string();
    assert_eq!(code(get_run(&app.state, &missing).await), "not_found");

    for filter in [
        json!({"category": "nope"}),
        json!({"limit": "x"}),
        json!({"limit": 0}),
        json!({"unknown": 1}),
    ] {
        let err = ipc(list_runs(&app.state, Some(filter.clone())).await).unwrap_err();
        assert_eq!(err["code"], "validation", "{filter}");
    }
}

#[tokio::test]
async fn start_run_without_a_loaded_model() {
    let mut app = app(true).await;
    app.state.benchmarks = Arc::new(BenchmarkService::new(
        Registry::default(),
        Arc::new(InMemoryRuns::default()),
        Arc::new(FakeSession::empty()),
        None,
        Arc::new(FakeHost),
        Uuid::new_v4(),
    ));
    let request = json!({"benchmark_id": "inference.latency"});
    assert_eq!(
        code(start_run(&app.state, request).await),
        "no_model_loaded"
    );
}
