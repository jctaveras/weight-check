use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

use super::*;
use crate::domain::NewModel;
use crate::repository::ModelRepository;
use crate::service::benchmarks::Registry;
use crate::service::testing::{
    FakeGpu, FakeHealth, FakeHost, FakeLauncher, FakeSession, InMemoryHardware, InMemoryModels,
    InMemoryRuns,
};

struct TestApp {
    router: Router,
    model_id: Uuid,
    _dir: tempfile::TempDir,
}

async fn app(healthy: bool) -> TestApp {
    let dir = tempfile::tempdir().unwrap();
    let model_path = dir.path().join("tiny.gguf");
    std::fs::write(
        &model_path,
        crate::infra::gguf::tests::gguf_bytes("llama", 8, 7),
    )
    .unwrap();

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
        router: router(state),
        model_id: model.id,
        _dir: dir,
    }
}

async fn call(app: &TestApp, method: Method, uri: &str, body: Option<&str>) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body.map(|b| Body::from(b.to_string())).unwrap_or_default())
        .unwrap();
    let resp = app.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, json)
}

async fn get(app: &TestApp, uri: &str) -> (StatusCode, Value) {
    call(app, Method::GET, uri, None).await
}

async fn post(app: &TestApp, uri: &str, body: &str) -> (StatusCode, Value) {
    call(app, Method::POST, uri, Some(body)).await
}

#[tokio::test]
async fn health_reflects_database() {
    let (status, body) = get(&app(true).await, "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"status": "ok"}));

    let (status, body) = get(&app(false).await, "/health").await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"]["code"], "internal");
}

#[tokio::test]
async fn system_and_catalog() {
    let app = app(true).await;
    let (status, body) = get(&app, "/system").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["backend"], "vulkan");
    assert_eq!(body["gpu"]["name"], "Fake GPU");

    let (status, body) = get(&app, "/benchmarks").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn model_lifecycle() {
    let app = app(true).await;
    let (status, body) = get(&app, "/models").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body[0]["name"], "tiny");
    assert_eq!(body[0]["quantization"], "Q8_0");

    let (_, body) = get(&app, "/models/status").await;
    assert_eq!(body["loaded"], Value::Null);

    let load = json!({"model_id": app.model_id, "ctx_size": 1024}).to_string();
    let (status, body) = post(&app, "/models/load", &load).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["settings"]["ctx_size"], 1024);

    let (_, body) = get(&app, "/models/status").await;
    assert_eq!(body["loaded"]["model"]["id"], json!(app.model_id));

    let (status, body) = post(&app, "/models/unload", "").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(body, Value::Null);

    let (status, body) = post(&app, "/models/unload", "").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "no_model_loaded");
}

#[tokio::test]
async fn load_rejects_bad_payloads() {
    let app = app(true).await;
    for bad in [
        "{",
        r#"{"model_id": "x"}"#,
        r#"{"model_id": null, "extra": 1}"#,
    ] {
        let (status, body) = post(&app, "/models/load", bad).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{bad}");
        assert_eq!(body["error"]["code"], "validation");
    }

    let unknown = json!({"model_id": Uuid::new_v4()}).to_string();
    let (status, _) = post(&app, "/models/load", &unknown).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn runs_start_get_and_list() {
    let app = app(true).await;
    let (status, run) = post(
        &app,
        "/runs",
        r#"{"benchmark_id": "inference.latency", "config": {"repetitions": 1}}"#,
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{run}");
    assert_eq!(run["status"], "queued");

    let id = run["id"].as_str().unwrap();
    let (status, body) = get(&app, &format!("/runs/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["benchmark_name"], "inference.latency");
    assert!(body["metrics"].is_array());

    let (status, body) = get(&app, "/runs?category=inference_performance&limit=10").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn runs_map_errors() {
    let app = app(true).await;
    let (status, _) = post(&app, "/runs", r#"{"benchmark_id": "quality.reasoning"}"#).await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    let (status, _) = post(&app, "/runs", r#"{"benchmark_id": "nope"}"#).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(&app, "/runs", r#"{"benchmark": "x"}"#).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, _) = get(&app, "/runs/not-a-uuid").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = get(&app, &format!("/runs/{}", Uuid::new_v4())).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    for query in ["category=nope", "limit=x", "limit=0", "unknown=1"] {
        let (status, body) = get(&app, &format!("/runs?{query}")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}");
        assert_eq!(body["error"]["code"], "validation");
    }
}
