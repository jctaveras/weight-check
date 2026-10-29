use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;

use super::{
    ChatRequest, ChatResponse, CompletionRequest, CompletionResponse, LlamaApi, StreamedCompletion,
    Timings,
};
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

#[derive(Deserialize)]
struct ChatCompletion {
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: ChatUsage,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Deserialize, Default)]
struct ChatUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
}

/// Fails with the status and body when llama-server rejected the request.
async fn check_status(resp: reqwest::Response, path: &str) -> AppResult<reqwest::Response> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    Err(AppError::Llama(format!("{path} returned {status}: {text}")))
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
        let resp = check_status(resp, "/completion").await?;
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
        let resp = check_status(resp, "/completion").await?;

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

    async fn chat(&self, req: &ChatRequest) -> AppResult<ChatResponse> {
        let resp = self
            .http
            .post(self.url("/v1/chat/completions"))
            .json(req)
            .send()
            .await
            .map_err(llama_err)?;
        let resp = check_status(resp, "/v1/chat/completions").await?;
        let body: ChatCompletion = resp.json().await.map_err(llama_err)?;
        let choice = body
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Llama("chat completion returned no choices".into()))?;
        Ok(ChatResponse {
            content: choice.message.content.unwrap_or_default(),
            reasoning_content: choice.message.reasoning_content.filter(|r| !r.is_empty()),
            finish_reason: choice.finish_reason.unwrap_or_default(),
            prompt_tokens: body.usage.prompt_tokens,
            completion_tokens: body.usage.completion_tokens,
        })
    }
}

#[cfg(test)]
mod tests;
