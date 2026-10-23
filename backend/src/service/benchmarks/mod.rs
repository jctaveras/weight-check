//! Benchmark contract and the catalog of available benchmarks.

pub mod inference;
pub mod resource;
pub mod stats;
pub mod stubs;

use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::domain::{Category, Metric};
use crate::error::{AppError, AppResult};
use crate::infra::llama::{CompletionRequest, LlamaApi};
use crate::infra::monitor::{GpuMonitor, HostMonitor};
use crate::service::model_service::ActiveModel;

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkDescriptor {
    pub id: &'static str,
    pub category: Category,
    pub name: &'static str,
    pub description: &'static str,
    pub implemented: bool,
    pub default_config: Value,
}

#[derive(Debug, Clone)]
pub struct BenchmarkOutcome {
    /// Headline numbers, stored in the `metrics` table for cross-run comparison.
    pub metrics: Vec<Metric>,
    /// Raw, benchmark-specific output, stored as JSONB.
    pub details: Value,
}

/// Everything a benchmark may use while running.
pub struct BenchmarkContext {
    pub llama: Arc<dyn LlamaApi>,
    pub model: ActiveModel,
    pub gpu: Option<Arc<dyn GpuMonitor>>,
    pub host: Arc<dyn HostMonitor>,
}

#[async_trait]
pub trait Benchmark: Send + Sync {
    fn descriptor(&self) -> BenchmarkDescriptor;

    /// Validates a user-supplied config and fills in defaults.
    fn resolve_config(&self, raw: Option<Value>) -> AppResult<Value>;

    async fn run(&self, ctx: &BenchmarkContext, config: Value) -> AppResult<BenchmarkOutcome>;
}

/// Parses `raw` into `C` (missing fields take defaults) and returns the complete config.
pub fn resolve_config<C>(raw: Option<Value>) -> AppResult<Value>
where
    C: DeserializeOwned + Serialize + Default,
{
    let config: C = match raw {
        None | Some(Value::Null) => C::default(),
        Some(v) => parse_config(v)?,
    };
    serde_json::to_value(config).map_err(|e| AppError::Internal(e.to_string()))
}

pub fn parse_config<C: DeserializeOwned>(value: Value) -> AppResult<C> {
    serde_json::from_value(value).map_err(|e| AppError::Validation(format!("invalid config: {e}")))
}

pub fn default_config<C: Serialize + Default>() -> Value {
    serde_json::to_value(C::default()).unwrap_or(Value::Null)
}

/// A deterministic completion request used across benchmarks.
pub fn completion_request(prompt: &str, n_predict: u32, seed: i64) -> CompletionRequest {
    CompletionRequest {
        prompt: prompt.to_string(),
        n_predict,
        temperature: 0.0,
        seed,
        cache_prompt: false,
        ignore_eos: true,
    }
}

/// Adds `<prefix>_{mean,median,p95,min,max,stddev}` metrics for a summary.
pub fn push_summary(metrics: &mut Vec<Metric>, prefix: &str, unit: &str, s: &stats::Summary) {
    for (suffix, value) in [
        ("mean", s.mean),
        ("median", s.median),
        ("p95", s.p95),
        ("min", s.min),
        ("max", s.max),
        ("stddev", s.stddev),
    ] {
        metrics.push(Metric::new(format!("{prefix}_{suffix}"), value, unit));
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryCatalog {
    pub category: Category,
    pub title: &'static str,
    pub description: &'static str,
    pub benchmarks: Vec<BenchmarkDescriptor>,
}

pub struct Registry {
    benchmarks: Vec<Arc<dyn Benchmark>>,
}

impl Registry {
    pub fn new(benchmarks: Vec<Arc<dyn Benchmark>>) -> Self {
        Self { benchmarks }
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn Benchmark>> {
        self.benchmarks
            .iter()
            .find(|b| b.descriptor().id == id)
            .cloned()
    }

    pub fn catalog(&self) -> Vec<CategoryCatalog> {
        Category::ALL
            .into_iter()
            .map(|category| CategoryCatalog {
                category,
                title: category.title(),
                description: category.description(),
                benchmarks: self
                    .benchmarks
                    .iter()
                    .map(|b| b.descriptor())
                    .filter(|d| d.category == category)
                    .collect(),
            })
            .collect()
    }
}

impl Default for Registry {
    fn default() -> Self {
        let mut benchmarks: Vec<Arc<dyn Benchmark>> = vec![
            Arc::new(inference::GenerationSpeed),
            Arc::new(inference::Latency),
            Arc::new(inference::Throughput),
            Arc::new(resource::ResourceProfile),
        ];
        benchmarks.extend(stubs::all());
        Self::new(benchmarks)
    }
}
