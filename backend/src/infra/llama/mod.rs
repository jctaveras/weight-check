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
