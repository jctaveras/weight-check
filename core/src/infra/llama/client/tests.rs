use std::convert::Infallible;

use axum::body::{Body, Bytes};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::Value;

use super::*;

const SSE: &str = concat!(
    "data: {\"content\":\"Hel\",\"stop\":false}\n\n",
    ": keep-alive comment\n\n",
    "data: {\"content\":\"\",\"stop\":false}\n\n",
    "data: {\"content\":\"lo\",\"stop\":false}\n\n",
    "data: {\"content\":\"\",\"stop\":true,\"timings\":{\"predicted_n\":2,\"predicted_per_second\":40.0}}\n\n",
);

async fn completion(Json(body): Json<Value>) -> Response {
    if body["stream"] == true {
        let text = if body["prompt"] == "malformed" {
            "data: {nope\n\n"
        } else {
            SSE
        };
        // Odd-sized chunks so events straddle chunk boundaries.
        let chunks: Vec<Result<Bytes, Infallible>> = text
            .as_bytes()
            .chunks(7)
            .map(|c| Ok(Bytes::copy_from_slice(c)))
            .collect();
        Body::from_stream(futures::stream::iter(chunks)).into_response()
    } else {
        Json(json!({
            "content": "hi",
            "tokens_predicted": 3,
            "tokens_evaluated": 5,
            "timings": {"predicted_n": 3, "predicted_per_second": 30.0},
            "extra_field": true,
        }))
        .into_response()
    }
}

fn healthy() -> Router {
    Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route("/completion", post(completion))
}

fn failing() -> Router {
    Router::new()
        .route("/health", get(|| async { StatusCode::SERVICE_UNAVAILABLE }))
        .route(
            "/completion",
            post(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "kaboom") }),
        )
}

async fn serve(router: Router) -> LlamaHttpClient {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    LlamaHttpClient::new(format!("http://{addr}"))
}

fn request(prompt: &str) -> CompletionRequest {
    CompletionRequest {
        prompt: prompt.into(),
        n_predict: 2,
        temperature: 0.0,
        seed: 0,
        cache_prompt: false,
        ignore_eos: true,
    }
}

#[tokio::test]
async fn health_reports_status_and_unreachable_server() {
    assert!(serve(healthy()).await.health().await.unwrap());
    assert!(!serve(failing()).await.health().await.unwrap());

    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let closed = LlamaHttpClient::new(format!("http://127.0.0.1:{port}"));
    assert!(!closed.health().await.unwrap());
}

#[tokio::test]
async fn completion_parses_response() {
    let resp = serve(healthy())
        .await
        .completion(&request("x"))
        .await
        .unwrap();
    assert_eq!(resp.content, "hi");
    assert_eq!(resp.tokens_predicted, 3);
    assert_eq!(resp.timings.predicted_per_second, 30.0);
    assert_eq!(resp.timings.prompt_ms, 0.0);
}

#[tokio::test]
async fn non_success_status_is_a_llama_error() {
    let client = serve(failing()).await;
    for result in [
        client.completion(&request("x")).await.map(|_| ()),
        client.completion_stream(&request("x")).await.map(|_| ()),
    ] {
        match result {
            Err(AppError::Llama(msg)) => {
                assert!(msg.contains("500") && msg.contains("kaboom"), "{msg}")
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[tokio::test]
async fn stream_records_token_arrivals_and_final_timings() {
    let out = serve(healthy())
        .await
        .completion_stream(&request("x"))
        .await
        .unwrap();
    assert_eq!(out.token_offsets_ms.len(), 2);
    assert_eq!(out.ttft_ms, out.token_offsets_ms.first().copied());
    assert!(out.total_ms >= out.ttft_ms.unwrap());
    let timings = out.timings.expect("timings from stop chunk");
    assert_eq!(timings.predicted_n, 2.0);
    assert_eq!(timings.predicted_per_second, 40.0);
}

#[tokio::test]
async fn malformed_stream_event_is_an_error() {
    let err = serve(healthy())
        .await
        .completion_stream(&request("malformed"))
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Llama(_)));
}
