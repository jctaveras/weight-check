//! llama.cpp adapters: a launcher for the `llama-server` child process and an HTTP client.

pub mod client;
pub mod process;

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::LoadSettings;
use crate::error::AppResult;

#[derive(Debug, Clone, Serialize)]
pub struct CompletionRequest {
    pub prompt: String,
    pub n_predict: u32,
    pub temperature: f32,
    pub seed: i64,
    pub cache_prompt: bool,
    /// Keep generating until `n_predict` tokens, so runs are comparable.
    pub ignore_eos: bool,
    /// Report the top `n` candidate tokens (with log-probabilities) for each generated token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_probs: Option<u32>,
}

/// Server-side timings reported by llama-server for one completion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Timings {
    pub prompt_n: f64,
    pub prompt_ms: f64,
    pub prompt_per_second: f64,
    pub predicted_n: f64,
    pub predicted_ms: f64,
    pub predicted_per_second: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CompletionResponse {
    pub content: String,
    pub tokens_predicted: u64,
    pub tokens_evaluated: u64,
    pub timings: Timings,
    /// One entry per generated token when `n_probs` was requested.
    #[serde(rename = "completion_probabilities")]
    pub probs: Vec<TokenProbs>,
}

/// A generated token and the most likely candidates at its position.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TokenProbs {
    pub token: String,
    pub logprob: f64,
    pub top_logprobs: Vec<TokenLogprob>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TokenLogprob {
    pub token: String,
    pub logprob: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
}

/// An OpenAI-style chat request, formatted with the model's own chat template.
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub seed: i64,
    /// Extra variables for the chat template, e.g. `{"enable_thinking": false}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template_kwargs: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ChatResponse {
    /// The answer, without any extracted reasoning.
    pub content: String,
    pub reasoning_content: Option<String>,
    /// `stop`, or `length` when `max_tokens` cut the answer off.
    pub finish_reason: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

/// Client-observed timing of a streamed completion.
#[derive(Debug, Clone, Default)]
pub struct StreamedCompletion {
    /// Wall-clock time from request start to the first generated token.
    pub ttft_ms: Option<f64>,
    /// Arrival time of every streamed token, relative to request start.
    pub token_offsets_ms: Vec<f64>,
    pub total_ms: f64,
    pub timings: Option<Timings>,
}

#[async_trait]
pub trait LlamaApi: Send + Sync {
    async fn health(&self) -> AppResult<bool>;
    async fn completion(&self, req: &CompletionRequest) -> AppResult<CompletionResponse>;
    async fn completion_stream(&self, req: &CompletionRequest) -> AppResult<StreamedCompletion>;
    async fn chat(&self, req: &ChatRequest) -> AppResult<ChatResponse>;
}

#[derive(Debug, Clone)]
pub struct LaunchArgs {
    pub model_path: PathBuf,
    pub settings: LoadSettings,
}

/// A started, healthy llama-server instance.
#[async_trait]
pub trait RunningServer: Send + Sync {
    fn api(&self) -> Arc<dyn LlamaApi>;
    fn pid(&self) -> Option<u32>;
    async fn stop(self: Box<Self>);
}

#[async_trait]
pub trait ServerLauncher: Send + Sync {
    /// Spawn a server and wait until it reports healthy.
    async fn launch(&self, args: &LaunchArgs) -> AppResult<Box<dyn RunningServer>>;
}
