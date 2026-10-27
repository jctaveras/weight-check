use std::time::Instant;

use async_trait::async_trait;
use futures::future::try_join_all;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::DEFAULT_PROMPTS;
use crate::domain::{Category, Metric};
use crate::error::{AppError, AppResult};
use crate::service::benchmarks::{
    Benchmark, BenchmarkContext, BenchmarkDescriptor, BenchmarkOutcome, completion_request,
    default_config, parse_config, resolve_config, stats,
};

/// Aggregate throughput with several concurrent clients.
pub struct Throughput;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Concurrent clients; defaults to the server's `parallel` slot count.
    pub concurrency: Option<u32>,
    pub requests_per_client: u32,
    pub n_predict: u32,
    pub prompt: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            concurrency: None,
            requests_per_client: 2,
            n_predict: 128,
            prompt: DEFAULT_PROMPTS[1].to_string(),
        }
    }
}

#[async_trait]
impl Benchmark for Throughput {
    fn descriptor(&self) -> BenchmarkDescriptor {
        BenchmarkDescriptor {
            id: "inference.throughput",
            category: Category::InferencePerformance,
            name: "Throughput",
            description: "Aggregate tokens/s and requests/s with concurrent clients (load the model with parallel > 1)",
            implemented: true,
            default_config: default_config::<Config>(),
        }
    }

    fn resolve_config(&self, raw: Option<Value>) -> AppResult<Value> {
        let value = resolve_config::<Config>(raw)?;
        let config: Config = parse_config(value.clone())?;
        if config.concurrency == Some(0) || config.requests_per_client == 0 || config.n_predict == 0
        {
            return Err(AppError::Validation(
                "concurrency, requests_per_client and n_predict must be positive".into(),
            ));
        }
        Ok(value)
    }

    async fn run(&self, ctx: &BenchmarkContext, config: Value) -> AppResult<BenchmarkOutcome> {
        let config: Config = parse_config(config)?;
        let slots = ctx.model.settings.parallel;
        let concurrency = config.concurrency.unwrap_or(slots);

        let started = Instant::now();
        let clients = (0..concurrency).map(|client| {
            let llama = ctx.llama.clone();
            let config = &config;
            async move {
                let mut results = Vec::new();
                for i in 0..config.requests_per_client {
                    let seed = (client * config.requests_per_client + i) as i64;
                    let req = completion_request(&config.prompt, config.n_predict, seed);
                    results.push(llama.completion(&req).await?.timings);
                }
                AppResult::Ok(results)
            }
        });
        let timings: Vec<_> = try_join_all(clients).await?.into_iter().flatten().collect();
        let wall_s = started.elapsed().as_secs_f64();

        let total_tokens: f64 = timings.iter().map(|t| t.predicted_n).sum();
        let per_request_tps: Vec<f64> = timings.iter().map(|t| t.predicted_per_second).collect();
        let per_request = stats::summarize(&per_request_tps);

        let mut metrics = vec![
            Metric::new("throughput_tps", total_tokens / wall_s, "tok/s"),
            Metric::new("requests_per_sec", timings.len() as f64 / wall_s, "req/s"),
            Metric::new("concurrency", concurrency as f64, "clients"),
        ];
        if let Some(s) = per_request {
            metrics.push(Metric::new("per_request_tps_mean", s.mean, "tok/s"));
        }

        Ok(BenchmarkOutcome {
            metrics,
            details: json!({
                "concurrency": concurrency,
                "server_slots": slots,
                "queued": concurrency > slots,
                "wall_time_s": wall_s,
                "total_tokens": total_tokens,
                "per_request_tps": per_request,
                "requests": timings,
            }),
        })
    }
}

#[cfg(test)]
mod tests;
