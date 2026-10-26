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

/// Prompt-processing and token-generation speed as reported by llama-server.
pub struct GenerationSpeed;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub prompts: Vec<String>,
    pub n_predict: u32,
    pub repetitions: u32,
    pub warmup: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            prompts: DEFAULT_PROMPTS.iter().map(|p| p.to_string()).collect(),
            n_predict: 256,
            repetitions: 3,
            warmup: true,
        }
    }
}

#[async_trait]
impl Benchmark for GenerationSpeed {
    fn descriptor(&self) -> BenchmarkDescriptor {
        BenchmarkDescriptor {
            id: "inference.generation_speed",
            category: Category::InferencePerformance,
            name: "Generation speed",
            description: "Prompt processing and token generation speed (tokens/s) over repeated prompts",
            implemented: true,
            default_config: default_config::<Config>(),
        }
    }

    fn resolve_config(&self, raw: Option<Value>) -> AppResult<Value> {
        let value = resolve_config::<Config>(raw)?;
        let config: Config = parse_config(value.clone())?;
        if config.prompts.is_empty() || config.repetitions == 0 || config.n_predict == 0 {
            return Err(AppError::Validation(
                "prompts, repetitions and n_predict must be non-empty/positive".into(),
            ));
        }
        Ok(value)
    }

    async fn run(&self, ctx: &BenchmarkContext, config: Value) -> AppResult<BenchmarkOutcome> {
        let config: Config = parse_config(config)?;

        if config.warmup {
            ctx.llama
                .completion(&completion_request(&config.prompts[0], 16, 0))
                .await?;
        }

        let mut generation_tps = Vec::new();
        let mut prompt_tps = Vec::new();
        let mut samples = Vec::new();
        for rep in 0..config.repetitions {
            for (i, prompt) in config.prompts.iter().enumerate() {
                let req = completion_request(prompt, config.n_predict, rep as i64);
                let timings = ctx.llama.completion(&req).await?.timings;
                generation_tps.push(timings.predicted_per_second);
                prompt_tps.push(timings.prompt_per_second);
                samples.push(json!({ "repetition": rep, "prompt_index": i, "timings": timings }));
            }
        }

        let generation = stats::summarize(&generation_tps)
            .ok_or_else(|| AppError::Llama("no generation timings returned".into()))?;
        let mut metrics = Vec::new();
        push_summary(&mut metrics, "generation_tps", "tok/s", &generation);
        if let Some(prompt) = stats::summarize(&prompt_tps) {
            metrics.push(Metric::new("prompt_tps_mean", prompt.mean, "tok/s"));
            metrics.push(Metric::new("prompt_tps_median", prompt.median, "tok/s"));
        }

        Ok(BenchmarkOutcome {
            metrics,
            details: json!({
                "generation_tps": generation,
                "prompt_tps": stats::summarize(&prompt_tps),
                "samples": samples,
            }),
        })
    }
}

#[cfg(test)]
mod tests;
