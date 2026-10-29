mod checks;

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::sources::IFEVAL;
use super::{
    QualityDeps, RESPONSE_PREVIEW_CHARS, Tally, Thinking, chat_request, dataset_details,
    effective_concurrency, eval_all, items_metric, load_rows, sample, truncate, validate_common,
};
use crate::domain::{Category, Metric};
use crate::error::AppResult;
use crate::infra::llama::ChatMessage;
use crate::service::benchmarks::{
    Benchmark, BenchmarkContext, BenchmarkDescriptor, BenchmarkOutcome, default_config,
    parse_config, resolve_config,
};

/// Verifiable instruction following (IFEval): prompts such as "answer in under 100 words, in
/// lowercase", checked programmatically in strict and loose mode as in the paper.
pub struct InstructionFollowing {
    deps: QualityDeps,
}

impl InstructionFollowing {
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
    key: u64,
    prompt: String,
    instruction_id_list: Vec<String>,
    kwargs: Vec<Map<String, Value>>,
}

/// One instruction of a prompt, with its kwargs.
#[derive(Debug, Clone)]
struct Instruction {
    id: String,
    kwargs: Map<String, Value>,
}

/// A prompt with only the instructions we can check. Prompts left with none are dropped.
#[derive(Debug, Clone)]
struct Item {
    key: u64,
    prompt: String,
    instructions: Vec<Instruction>,
}

/// Splits rows into checkable items, counting the instructions that had to be skipped.
fn to_items(rows: Vec<Row>, skipped: &mut BTreeMap<String, usize>) -> Vec<Item> {
    rows.into_iter()
        .filter_map(|row| {
            let mut instructions = Vec::new();
            for (id, kwargs) in row.instruction_id_list.into_iter().zip(row.kwargs) {
                if checks::SUPPORTED.contains(&id.as_str()) {
                    instructions.push(Instruction { id, kwargs });
                } else {
                    *skipped.entry(id).or_default() += 1;
                }
            }
            (!instructions.is_empty()).then_some(Item {
                key: row.key,
                prompt: row.prompt,
                instructions,
            })
        })
        .collect()
}

/// Strict: the response itself passes. Loose: any of the paper's relaxed variants passes.
fn follows(inst: &Instruction, response: &str) -> (bool, bool) {
    let passes =
        |r: &str| !r.trim().is_empty() && checks::check(&inst.id, &inst.kwargs, r).unwrap_or(false);
    let strict = passes(response);
    let loose = strict || checks::loose_variants(response).iter().any(|v| passes(v));
    (strict, loose)
}

#[async_trait]
impl Benchmark for InstructionFollowing {
    fn descriptor(&self) -> BenchmarkDescriptor {
        BenchmarkDescriptor {
            id: "quality.instruction_following",
            category: Category::ModelQuality,
            name: "Instruction following (IFEval)",
            description: "Prompts with verifiable constraints (length, format, case, keywords), checked programmatically in strict and loose mode",
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
        let rows: Vec<Row> = load_rows(self.deps.datasets.as_ref(), &IFEVAL).await?;
        let mut skipped = BTreeMap::new();
        let items = sample(to_items(rows, &mut skipped), config.limit, config.seed);
        let concurrency = effective_concurrency(ctx, config.concurrency);

        let results = eval_all(items, concurrency, |item| {
            let req = chat_request(
                vec![ChatMessage::user(item.prompt.clone())],
                config.max_tokens,
                config.seed,
                config.thinking,
            );
            async move { Ok((item, ctx.llama.chat(&req).await?)) }
        })
        .await?;

        let (mut prompt_strict, mut prompt_loose) = (Tally::default(), Tally::default());
        let (mut inst_strict, mut inst_loose) = (Tally::default(), Tally::default());
        let mut per_instruction: BTreeMap<&str, (Tally, Tally)> = BTreeMap::new();
        let mut samples = Vec::new();
        for (item, resp) in &results {
            let mut outcomes = Vec::new();
            for inst in &item.instructions {
                let (strict, loose) = follows(inst, &resp.content);
                inst_strict.add(strict);
                inst_loose.add(loose);
                let entry = per_instruction.entry(inst.id.as_str()).or_default();
                entry.0.add(strict);
                entry.1.add(loose);
                outcomes.push(json!({"id": inst.id, "strict": strict, "loose": loose}));
            }
            let all = |key: &str| outcomes.iter().all(|o| o[key] == true);
            prompt_strict.add(all("strict"));
            prompt_loose.add(all("loose"));
            samples.push(json!({
                "key": item.key,
                "prompt_strict": all("strict"),
                "prompt_loose": all("loose"),
                "instructions": outcomes,
                "truncated": resp.finish_reason == "length",
                "response": truncate(&resp.content, RESPONSE_PREVIEW_CHARS),
            }));
        }

        let metrics = vec![
            Metric::new("prompt_strict_acc", prompt_strict.accuracy(), "%"),
            Metric::new("inst_strict_acc", inst_strict.accuracy(), "%"),
            Metric::new("prompt_loose_acc", prompt_loose.accuracy(), "%"),
            Metric::new("inst_loose_acc", inst_loose.accuracy(), "%"),
            items_metric(results.len()),
        ];
        let per_instruction: Map<String, Value> = per_instruction
            .into_iter()
            .map(|(id, (strict, loose))| {
                (
                    id.to_string(),
                    json!({"total": strict.total, "strict_accuracy": strict.accuracy(), "loose_accuracy": loose.accuracy()}),
                )
            })
            .collect();
        Ok(BenchmarkOutcome {
            metrics,
            details: json!({
                "datasets": dataset_details(&[&IFEVAL]),
                "concurrency": concurrency,
                "prompt_strict": prompt_strict.to_json(),
                "prompt_loose": prompt_loose.to_json(),
                "instruction_strict": inst_strict.to_json(),
                "instruction_loose": inst_loose.to_json(),
                "per_instruction": per_instruction,
                "skipped_instructions": skipped,
                "samples": samples,
            }),
        })
    }
}

#[cfg(test)]
mod tests;
