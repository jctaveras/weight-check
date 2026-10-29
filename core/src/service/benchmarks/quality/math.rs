use std::sync::LazyLock;

use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::sources::GSM8K_TEST;
use super::{
    QualityDeps, RESPONSE_PREVIEW_CHARS, Tally, Thinking, chat_request, dataset_details,
    effective_concurrency, eval_all, items_metric, load_rows, pct, sample, truncate,
    validate_common,
};
use crate::domain::{Category, Metric};
use crate::error::AppResult;
use crate::infra::llama::ChatMessage;
use crate::service::benchmarks::{
    Benchmark, BenchmarkContext, BenchmarkDescriptor, BenchmarkOutcome, default_config,
    parse_config, resolve_config, stats,
};

/// Grade-school math word problems (GSM8K), solved with chain of thought and scored by the
/// final number.
pub struct Math {
    deps: QualityDeps,
}

impl Math {
    pub fn new(deps: QualityDeps) -> Self {
        Self { deps }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub limit: u32,
    pub seed: u64,
    /// Cap on generated tokens, including any thinking.
    pub max_tokens: u32,
    pub thinking: Thinking,
    pub concurrency: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            limit: 200,
            seed: 0,
            max_tokens: 4096,
            thinking: Thinking::Auto,
            concurrency: 1,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Row {
    question: String,
    /// Worked solution ending in `#### <answer>`.
    answer: String,
}

const INSTRUCTION: &str = "Solve the following math problem step by step. The last line of your response should be of the form \"#### ANSWER\" (without quotes) where ANSWER is the final numerical answer.";

static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-?\$?\d[\d,]*(?:\.\d+)?").unwrap());
static HASHES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"####\s*\**\s*(-?\$?\d[\d,]*(?:\.\d+)?)").unwrap());
static BOXED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\boxed\{([^{}]*)\}").unwrap());

/// Parses a number written as `1,000`, `$5.50`, `-3` or `7.` (dropping the decoration).
fn parse_number(s: &str) -> Option<f64> {
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(c, ',' | '$' | ' '))
        .collect();
    cleaned.trim_end_matches('.').parse().ok()
}

/// The model's final answer: the last `#### n`, else the last `\boxed{n}`, else the last number.
pub(crate) fn extract_answer(response: &str) -> Option<f64> {
    if let Some(m) = HASHES.captures_iter(response).last() {
        return parse_number(&m[1]);
    }
    if let Some(m) = BOXED.captures_iter(response).last()
        && let Some(n) = NUMBER.find_iter(&m[1]).last()
    {
        return parse_number(n.as_str());
    }
    NUMBER
        .find_iter(response)
        .last()
        .and_then(|m| parse_number(m.as_str()))
}

/// The reference answer after `####` in a GSM8K solution.
fn reference_answer(solution: &str) -> Option<f64> {
    solution
        .rsplit("####")
        .next()
        .and_then(|a| parse_number(a.trim()))
}

fn same_number(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[async_trait]
impl Benchmark for Math {
    fn descriptor(&self) -> BenchmarkDescriptor {
        BenchmarkDescriptor {
            id: "quality.math",
            category: Category::ModelQuality,
            name: "Math reasoning (GSM8K)",
            description: "Grade-school math word problems solved step by step, scored by exact match on the final number",
            implemented: true,
            default_config: default_config::<Config>(),
        }
    }

    fn resolve_config(&self, raw: Option<Value>) -> AppResult<Value> {
        let value = resolve_config::<Config>(raw)?;
        let config: Config = parse_config(value.clone())?;
        validate_common(config.limit, config.concurrency, Some(config.max_tokens))?;
        Ok(value)
    }

    async fn run(&self, ctx: &BenchmarkContext, config: Value) -> AppResult<BenchmarkOutcome> {
        let config: Config = parse_config(config)?;
        let rows: Vec<Row> = load_rows(self.deps.datasets.as_ref(), &GSM8K_TEST).await?;
        let items: Vec<(usize, Row)> = sample(
            rows.into_iter().enumerate().collect(),
            config.limit,
            config.seed,
        );
        let concurrency = effective_concurrency(ctx, config.concurrency);

        let results = eval_all(items, concurrency, |(index, row)| {
            let req = chat_request(
                vec![ChatMessage::user(format!(
                    "{INSTRUCTION}\n\n{}",
                    row.question
                ))],
                config.max_tokens,
                config.seed,
                config.thinking,
            );
            async move { Ok((index, row, ctx.llama.chat(&req).await?)) }
        })
        .await?;

        let mut tally = Tally::default();
        let mut invalid = 0;
        let mut completion_tokens = Vec::new();
        let mut samples = Vec::new();
        for (index, row, resp) in &results {
            let truncated = resp.finish_reason == "length";
            let predicted = if truncated {
                None
            } else {
                extract_answer(&resp.content)
            };
            let expected = reference_answer(&row.answer);
            let correct = matches!((predicted, expected), (Some(p), Some(e)) if same_number(p, e));
            tally.add(correct);
            invalid += usize::from(predicted.is_none());
            completion_tokens.push(resp.completion_tokens as f64);
            samples.push(json!({
                "index": index,
                "expected": expected,
                "predicted": predicted,
                "correct": correct,
                "truncated": truncated,
                "completion_tokens": resp.completion_tokens,
                "response": truncate(&resp.content, RESPONSE_PREVIEW_CHARS),
            }));
        }

        let mut metrics = vec![
            Metric::new("accuracy", tally.accuracy(), "%"),
            Metric::new("invalid_rate", pct(invalid, results.len()), "%"),
        ];
        let tokens = stats::summarize(&completion_tokens);
        if let Some(s) = &tokens {
            metrics.push(Metric::new("completion_tokens_mean", s.mean, "tok"));
        }
        metrics.push(items_metric(results.len()));

        Ok(BenchmarkOutcome {
            metrics,
            details: json!({
                "datasets": dataset_details(&[&GSM8K_TEST]),
                "concurrency": concurrency,
                "accuracy": tally.to_json(),
                "invalid": invalid,
                "completion_tokens": tokens,
                "samples": samples,
            }),
        })
    }
}

#[cfg(test)]
mod tests;
