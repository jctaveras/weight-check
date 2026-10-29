//! Category 3 — Model Intelligence & Quality.
//!
//! Each benchmark scores a deterministic sample of a pinned public dataset (see `sources.rs`).
//! Prompts and scoring follow lm-eval-harness and the datasets' own papers closely, but the
//! numbers are only strictly comparable between runs with the same benchmark config.

mod coding;
mod ifeval;
mod knowledge;
mod math;
mod sources;

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use futures::{StreamExt, TryStreamExt};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub use coding::Coding;
pub use ifeval::InstructionFollowing;
pub use knowledge::Knowledge;
pub use math::Math;

use super::BenchmarkContext;
use crate::domain::Metric;
use crate::error::{AppError, AppResult};
use crate::infra::datasets::{DatasetFile, DatasetStore, read_jsonl, read_parquet};
use crate::infra::llama::{ChatMessage, ChatRequest};
use crate::infra::sandbox::CodeRunner;

/// What the quality benchmarks need beyond the loaded model.
#[derive(Clone)]
pub struct QualityDeps {
    pub datasets: Arc<dyn DatasetStore>,
    pub code_runner: Arc<dyn CodeRunner>,
}

/// Whether reasoning models may think before answering.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Thinking {
    /// Leave it to the chat template's default.
    #[default]
    Auto,
    /// Ask the template to skip thinking (`enable_thinking: false`).
    Off,
}

pub(crate) fn chat_request(
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    seed: u64,
    thinking: Thinking,
) -> ChatRequest {
    ChatRequest {
        messages,
        max_tokens,
        temperature: 0.0,
        seed: seed as i64,
        chat_template_kwargs: (thinking == Thinking::Off)
            .then(|| json!({"enable_thinking": false})),
    }
}

/// Checks the fields every quality config shares.
pub(crate) fn validate_common(
    limit: u32,
    concurrency: u32,
    max_tokens: Option<u32>,
) -> AppResult<()> {
    if limit == 0 || concurrency == 0 || max_tokens == Some(0) {
        return Err(AppError::Validation(
            "limit, concurrency and max_tokens must be > 0".into(),
        ));
    }
    Ok(())
}

/// Requests in flight at once: the configured concurrency, capped at the server's slots.
pub(crate) fn effective_concurrency(ctx: &BenchmarkContext, requested: u32) -> usize {
    requested.min(ctx.model.settings.parallel.max(1)) as usize
}

/// SplitMix64: a tiny, fixed PRNG, so the same seed picks the same items on every build.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// Fisher–Yates shuffle driven by `seed`.
fn shuffle<T>(items: &mut [T], seed: u64) {
    let mut rng = SplitMix64(seed);
    for i in (1..items.len()).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        items.swap(i, j);
    }
}

/// Picks up to `limit` items with a seeded shuffle, returned in dataset order.
pub(crate) fn sample<T>(items: Vec<T>, limit: u32, seed: u64) -> Vec<T> {
    sample_stratified(items, limit, seed, |_| String::new())
}

/// Like `sample`, but spreads the picks evenly across the groups `key` assigns: groups are
/// shuffled independently and drawn from round-robin until `limit` items are taken.
pub(crate) fn sample_stratified<T>(
    items: Vec<T>,
    limit: u32,
    seed: u64,
    key: impl Fn(&T) -> String,
) -> Vec<T> {
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, item) in items.iter().enumerate() {
        groups.entry(key(item)).or_default().push(i);
    }
    let mut groups: Vec<Vec<usize>> = groups.into_values().collect();
    for (n, group) in groups.iter_mut().enumerate() {
        shuffle(group, seed.wrapping_add(n as u64));
        group.reverse(); // pop() then yields the shuffled order
    }

    let mut picked = Vec::new();
    while picked.len() < limit as usize && groups.iter().any(|g| !g.is_empty()) {
        for group in &mut groups {
            if picked.len() == limit as usize {
                break;
            }
            if let Some(i) = group.pop() {
                picked.push(i);
            }
        }
    }
    picked.sort_unstable();

    let mut slots: Vec<Option<T>> = items.into_iter().map(Some).collect();
    picked.into_iter().filter_map(|i| slots[i].take()).collect()
}

/// Evaluates `items` with up to `concurrency` in flight, returning results in input order.
pub(crate) async fn eval_all<I, O, F, Fut>(
    items: Vec<I>,
    concurrency: usize,
    f: F,
) -> AppResult<Vec<O>>
where
    F: Fn(I) -> Fut,
    Fut: Future<Output = AppResult<O>>,
{
    futures::stream::iter(items.into_iter().map(f))
        .buffered(concurrency.max(1))
        .try_collect()
        .await
}

/// Downloads (or reuses) a dataset file and decodes its rows into `T`, reading Parquet or
/// JSON Lines by the local file's extension.
pub(crate) async fn load_rows<T>(store: &dyn DatasetStore, file: &DatasetFile) -> AppResult<Vec<T>>
where
    T: DeserializeOwned + Send + 'static,
{
    let path = store.fetch(file).await?;
    let file = *file;
    let read = move || {
        if path.extension().is_some_and(|e| e == "parquet") {
            read_parquet(&path)?
                .into_iter()
                .map(|row| {
                    serde_json::from_value(row).map_err(|e| {
                        AppError::Internal(format!(
                            "unexpected row in {}/{}: {e}",
                            file.repo, file.path
                        ))
                    })
                })
                .collect()
        } else {
            read_jsonl(&path)
        }
    };
    tokio::task::spawn_blocking(read)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Where a run's items came from, for `details`.
pub(crate) fn dataset_details(files: &[&DatasetFile]) -> Value {
    json!(
        files
            .iter()
            .map(|f| json!({
                "repo": f.repo,
                "revision": f.revision,
                "path": f.path,
                "sha256": f.sha256,
            }))
            .collect::<Vec<_>>()
    )
}

/// `part` as a percentage of `total` (0 for an empty total).
pub(crate) fn pct(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 * 100.0 / total as f64
    }
}

pub(crate) fn items_metric(n: usize) -> Metric {
    Metric::new("items", n as f64, "items")
}

/// Running tally of correct answers, per group and overall.
#[derive(Debug, Default, Serialize)]
pub(crate) struct Tally {
    pub correct: usize,
    pub total: usize,
}

impl Tally {
    pub fn add(&mut self, correct: bool) {
        self.total += 1;
        self.correct += usize::from(correct);
    }

    pub fn accuracy(&self) -> f64 {
        pct(self.correct, self.total)
    }

    pub fn to_json(&self) -> Value {
        json!({"correct": self.correct, "total": self.total, "accuracy": self.accuracy()})
    }
}

/// Keeps `details` small: at most `max` characters of a model response.
pub(crate) fn truncate(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

/// Characters of each response kept in `details`.
pub(crate) const RESPONSE_PREVIEW_CHARS: usize = 2000;

#[cfg(test)]
mod tests;
