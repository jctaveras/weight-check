use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;

use super::{CompletionRequest, CompletionResponse, LlamaApi, StreamedCompletion, Timings};
use crate::error::{AppError, AppResult};

/// HTTP client for a llama-server instance.
pub struct LlamaHttpClient {
    http: reqwest::Client,
    base_url: String,
}

impl LlamaHttpClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("reqwest client");
        Self {
            http,
            base_url: base_url.into(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

fn llama_err(e: impl std::fmt::Display) -> AppError {
    AppError::Llama(e.to_string())
}

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    content: String,
    #[serde(default)]
    stop: bool,
    timings: Option<Timings>,
}

#[async_trait]
impl LlamaApi for LlamaHttpClient {
    async fn health(&self) -> AppResult<bool> {
        match self.http.get(self.url("/health")).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(e) if e.is_connect() => Ok(false),
            Err(e) => Err(llama_err(e)),
        }
    }

    async fn completion(&self, req: &CompletionRequest) -> AppResult<CompletionResponse> {
        let mut body = serde_json::to_value(req).map_err(llama_err)?;
        body["stream"] = json!(false);
        let resp = self
            .http
            .post(self.url("/completion"))
            .json(&body)
            .send()
            .await
            .map_err(llama_err)?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Llama(format!(
                "/completion returned {status}: {text}"
            )));
        }
        resp.json().await.map_err(llama_err)
    }

    async fn completion_stream(&self, req: &CompletionRequest) -> AppResult<StreamedCompletion> {
        let mut body = serde_json::to_value(req).map_err(llama_err)?;
        body["stream"] = json!(true);

        let start = Instant::now();
        let resp = self
            .http
            .post(self.url("/completion"))
            .json(&body)
            .send()
            .await
            .map_err(llama_err)?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Llama(format!(
                "/completion returned {status}: {text}"
            )));
        }

        let mut out = StreamedCompletion::default();
        let mut buffer = String::new();
        let mut stream = resp.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(llama_err)?;
            let arrived_ms = start.elapsed().as_secs_f64() * 1000.0;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Server-sent events are separated by blank lines.
            while let Some(pos) = buffer.find("\n\n") {
                let event: String = buffer.drain(..pos + 2).collect();
                for line in event.lines() {
                    let Some(data) = line.strip_prefix("data: ") else {
                        continue;
                    };
                    let parsed: StreamChunk = serde_json::from_str(data).map_err(llama_err)?;
                    if !parsed.content.is_empty() {
                        out.ttft_ms.get_or_insert(arrived_ms);
                        out.token_offsets_ms.push(arrived_ms);
                    }
                    if parsed.stop {
                        out.timings = parsed.timings;
                    }
                }
            }
        }

        out.total_ms = start.elapsed().as_secs_f64() * 1000.0;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
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
}
