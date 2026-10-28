use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::DEFAULT_PROMPTS;
use crate::domain::{Category, Metric};
use crate::error::{AppError, AppResult};
use crate::service::benchmarks::{
    Benchmark, BenchmarkContext, BenchmarkDescriptor, BenchmarkOutcome, completion_request,
    default_config, parse_config, push_summary, resolve_config, stats,
};

/// Client-observed responsiveness: time to first token and inter-token latency.
pub struct Latency;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub prompt: String,
    pub n_predict: u32,
    pub repetitions: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            prompt: DEFAULT_PROMPTS[0].to_string(),
            n_predict: 128,
            repetitions: 5,
        }
    }
}

#[async_trait]
impl Benchmark for Latency {
    fn descriptor(&self) -> BenchmarkDescriptor {
        BenchmarkDescriptor {
            id: "inference.latency",
            category: Category::InferencePerformance,
            name: "Latency",
            description: "Time to first token (TTFT), inter-token latency and end-to-end latency over streaming",
            implemented: true,
            default_config: default_config::<Config>(),
        }
    }

    fn resolve_config(&self, raw: Option<Value>) -> AppResult<Value> {
        let value = resolve_config::<Config>(raw)?;
        let config: Config = parse_config(value.clone())?;
        if config.repetitions == 0 || config.n_predict < 2 {
            return Err(AppError::Validation(
                "repetitions must be > 0 and n_predict >= 2".into(),
            ));
        }
        Ok(value)
    }

    async fn run(&self, ctx: &BenchmarkContext, config: Value) -> AppResult<BenchmarkOutcome> {
        let config: Config = parse_config(config)?;

        let mut ttft = Vec::new();
        let mut itl = Vec::new();
        let mut e2e = Vec::new();
        let mut samples = Vec::new();
        for rep in 0..config.repetitions {
            let req = completion_request(&config.prompt, config.n_predict, rep as i64);
            let streamed = ctx.llama.completion_stream(&req).await?;
            let gaps = stats::deltas(&streamed.token_offsets_ms);

            if let Some(t) = streamed.ttft_ms {
                ttft.push(t);
            }
            e2e.push(streamed.total_ms);
            samples.push(json!({
                "repetition": rep,
                "ttft_ms": streamed.ttft_ms,
                "total_ms": streamed.total_ms,
                "tokens": streamed.token_offsets_ms.len(),
                "itl": stats::summarize(&gaps),
            }));
            itl.extend(gaps);
        }

        let ttft_summary = stats::summarize(&ttft)
            .ok_or_else(|| AppError::Llama("no tokens were streamed".into()))?;
        let mut metrics = Vec::new();
        push_summary(&mut metrics, "ttft_ms", "ms", &ttft_summary);
        if let Some(s) = stats::summarize(&itl) {
            metrics.push(Metric::new("itl_ms_mean", s.mean, "ms"));
            metrics.push(Metric::new("itl_ms_median", s.median, "ms"));
            metrics.push(Metric::new("itl_ms_p95", s.p95, "ms"));
        }
        if let Some(s) = stats::summarize(&e2e) {
            metrics.push(Metric::new("e2e_ms_median", s.median, "ms"));
            metrics.push(Metric::new("e2e_ms_p95", s.p95, "ms"));
        }

        Ok(BenchmarkOutcome {
            metrics,
            details: json!({
                "ttft_ms": ttft_summary,
                "itl_ms": stats::summarize(&itl),
                "e2e_ms": stats::summarize(&e2e),
                "samples": samples,
            }),
        })
    }
}

#[cfg(test)]
mod tests;
