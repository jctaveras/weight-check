use std::convert::Infallible;

use axum::body::{Body, Bytes};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::Value;

use crate::infra::llama::ChatMessage;

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
        let probs = if body["n_probs"] == 2 {
            json!([{"id": 1, "token": " B", "logprob": -0.1, "bytes": [32, 66],
            "top_logprobs": [
                {"id": 1, "token": " B", "logprob": -0.1, "bytes": [32, 66]},
                {"id": 2, "token": " A", "logprob": -2.5, "bytes": [32, 65]}
            ]}])
        } else {
            json!(null)
        };
        let mut resp = json!({
            "content": "hi",
            "tokens_predicted": 3,
            "tokens_evaluated": 5,
            "timings": {"predicted_n": 3, "predicted_per_second": 30.0},
            "extra_field": true,
        });
        if !probs.is_null() {
            resp["completion_probabilities"] = probs;
        }
        Json(resp).into_response()
    }
}

async fn chat(Json(body): Json<Value>) -> Response {
    // Echo the request back so tests can assert on what was sent.
    let thinking = body["chat_template_kwargs"]["enable_thinking"] != false;
    Json(json!({
        "choices": [{
            "index": 0,
            "finish_reason": if body["max_tokens"] == 1 { "length" } else { "stop" },
            "message": {
                "role": "assistant",
                "content": body["messages"][0]["content"],
                "reasoning_content": if thinking { "hmm" } else { "" },
            }
        }],
        "usage": {"prompt_tokens": 7, "completion_tokens": 3, "total_tokens": 10}
    }))
    .into_response()
}

fn healthy() -> Router {
    Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route("/completion", post(completion))
        .route("/v1/chat/completions", post(chat))
}

fn failing() -> Router {
    Router::new()
        .route("/health", get(|| async { StatusCode::SERVICE_UNAVAILABLE }))
        .route(
            "/completion",
            post(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "kaboom") }),
        )
        .route(
            "/v1/chat/completions",
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
        n_probs: None,
    }
}

fn chat_request(content: &str, max_tokens: u32, thinking: bool) -> ChatRequest {
    ChatRequest {
        messages: vec![ChatMessage::user(content)],
        max_tokens,
        temperature: 0.0,
        seed: 0,
        chat_template_kwargs: (!thinking).then(|| json!({"enable_thinking": false})),
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
    assert!(resp.probs.is_empty());
}

#[tokio::test]
async fn completion_parses_token_probabilities() {
    let mut req = request("x");
    req.n_probs = Some(2);
    let resp = serve(healthy()).await.completion(&req).await.unwrap();
    assert_eq!(resp.probs.len(), 1);
    assert_eq!(resp.probs[0].token, " B");
    let top: Vec<_> = resp.probs[0]
        .top_logprobs
        .iter()
        .map(|p| (p.token.as_str(), p.logprob))
        .collect();
    assert_eq!(top, [(" B", -0.1), (" A", -2.5)]);
}

#[tokio::test]
async fn chat_parses_content_reasoning_and_usage() {
    let client = serve(healthy()).await;
    let resp = client.chat(&chat_request("hello", 64, true)).await.unwrap();
    assert_eq!(resp.content, "hello");
    assert_eq!(resp.reasoning_content.as_deref(), Some("hmm"));
    assert_eq!(resp.finish_reason, "stop");
    assert_eq!((resp.prompt_tokens, resp.completion_tokens), (7, 3));

    let resp = client.chat(&chat_request("hi", 1, false)).await.unwrap();
    assert_eq!(resp.reasoning_content, None);
    assert_eq!(resp.finish_reason, "length");
}

#[tokio::test]
async fn non_success_status_is_a_llama_error() {
    let client = serve(failing()).await;
    for result in [
        client.completion(&request("x")).await.map(|_| ()),
        client.completion_stream(&request("x")).await.map(|_| ()),
        client.chat(&chat_request("x", 8, true)).await.map(|_| ()),
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
