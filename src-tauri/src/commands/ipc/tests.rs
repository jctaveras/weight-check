//! Drives the real IPC handler on Tauri's mock runtime, checking the wire contract the UI codes
//! against: command names, argument keys, and the JSON of results and errors.

use serde_json::json;
use tauri::Manager;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, MockRuntime, mock_builder, mock_context, noop_assets};
use tauri::webview::{InvokeRequest, WebviewWindow};

use super::*;
use crate::commands::tests::{TestApp, app};

struct Ipc {
    webview: WebviewWindow<MockRuntime>,
    fixture: TestApp,
}

fn ipc(healthy: bool) -> Ipc {
    let fixture = tauri::async_runtime::block_on(app(healthy));
    let tauri_app = mock_builder()
        .invoke_handler(handler())
        .build(mock_context(noop_assets()))
        .unwrap();
    tauri_app.manage(fixture.state.clone());
    let webview = tauri::WebviewWindowBuilder::new(&tauri_app, "main", Default::default())
        .build()
        .unwrap();
    Ipc { webview, fixture }
}

impl Ipc {
    /// Invokes `cmd` with the JSON `args`, as `invoke(cmd, args)` from the UI would.
    fn invoke(&self, cmd: &str, args: Value) -> Result<Value, Value> {
        tauri::test::get_ipc_response(
            &self.webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<Value>().unwrap())
    }
}

#[test]
fn read_only_commands_return_the_old_endpoint_json() {
    let ipc = ipc(true);
    assert_eq!(
        ipc.invoke("health", json!({})).unwrap(),
        json!({"status": "ok"})
    );
    let system = ipc.invoke("system_snapshot", json!({})).unwrap();
    assert_eq!(system["backend"], "vulkan");
    let models = ipc.invoke("list_models", json!({})).unwrap();
    assert_eq!(models[0]["name"], "tiny");
    let status = ipc.invoke("model_status", json!({})).unwrap();
    assert_eq!(status["loaded"], Value::Null);
    let catalog = ipc.invoke("benchmark_catalog", json!({})).unwrap();
    assert_eq!(catalog.as_array().unwrap().len(), 5);
}

#[test]
fn errors_serialize_as_code_and_message() {
    let ipc = ipc(false);
    assert_eq!(
        ipc.invoke("health", json!({})).unwrap_err(),
        json!({"code": "internal", "message": "database error: connection refused"})
    );
}

#[test]
fn model_commands_take_a_request_argument() {
    let ipc = ipc(true);
    let request = json!({"request": {"model_id": ipc.fixture.model_id, "ctx_size": 1024}});
    let active = ipc.invoke("load_model", request).unwrap();
    assert_eq!(active["settings"]["ctx_size"], 1024);

    assert_eq!(ipc.invoke("unload_model", json!({})).unwrap(), Value::Null);
    let err = ipc.invoke("unload_model", json!({})).unwrap_err();
    assert_eq!(err["code"], "no_model_loaded");

    let bad = json!({"request": {"model_id": "x"}});
    assert_eq!(
        ipc.invoke("load_model", bad).unwrap_err()["code"],
        "validation"
    );
    // A missing argument is rejected by Tauri itself, as a plain string.
    assert!(ipc.invoke("load_model", json!({})).unwrap_err().is_string());
}

#[test]
fn run_commands_take_request_filter_and_id() {
    let ipc = ipc(true);
    let request =
        json!({"request": {"benchmark_id": "inference.latency", "config": {"repetitions": 1}}});
    let run = ipc.invoke("start_run", request).unwrap();
    assert_eq!(run["status"], "queued");

    let fetched = ipc.invoke("get_run", json!({"id": run["id"]})).unwrap();
    assert_eq!(fetched["benchmark_name"], "inference.latency");
    assert!(fetched["metrics"].is_array());
    let err = ipc.invoke("get_run", json!({"id": "nope"})).unwrap_err();
    assert_eq!(err["code"], "validation");

    for args in [
        json!({}),
        json!({"filter": {}}),
        json!({"filter": {"category": "inference_performance", "limit": 10}}),
    ] {
        let runs = ipc.invoke("list_runs", args.clone()).unwrap();
        assert_eq!(runs.as_array().unwrap().len(), 1, "{args}");
    }
    let err = ipc
        .invoke("list_runs", json!({"filter": {"limit": 0}}))
        .unwrap_err();
    assert_eq!(err["code"], "validation");
}
