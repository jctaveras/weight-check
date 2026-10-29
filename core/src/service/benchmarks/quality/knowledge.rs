use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::sources::{ARC_CHALLENGE_TEST, ARC_CHALLENGE_TRAIN, MMLU_DEV, MMLU_TEST};
use super::{
    QualityDeps, Tally, dataset_details, effective_concurrency, eval_all, items_metric, load_rows,
    pct, sample, sample_stratified, validate_common,
};
use crate::domain::{Category, Metric};
use crate::error::{AppError, AppResult};
use crate::infra::llama::{CompletionRequest, TokenProbs};
use crate::service::benchmarks::{
    Benchmark, BenchmarkContext, BenchmarkDescriptor, BenchmarkOutcome, default_config,
    parse_config, resolve_config,
};

/// Multiple-choice knowledge and reasoning (MMLU, ARC-Challenge), scored from the probabilities
/// of the answer-letter tokens after `Answer:`, as in the original MMLU evaluation.
pub struct Knowledge {
    deps: QualityDeps,
}

impl Knowledge {
    pub fn new(deps: QualityDeps) -> Self {
        Self { deps }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dataset {
    #[default]
    Mmlu,
    ArcChallenge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub dataset: Dataset,
    /// Worked examples before each question (MMLU: from its dev split, at most 5).
    pub n_shot: u32,
    /// MMLU subjects to include, e.g. `["college_physics"]`; empty means all 57.
    pub subjects: Vec<String>,
    pub limit: u32,
    pub seed: u64,
    pub concurrency: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dataset: Dataset::Mmlu,
            n_shot: 5,
            subjects: Vec::new(),
            limit: 200,
            seed: 0,
            concurrency: 1,
        }
    }
}

const MAX_SHOTS: u32 = 5;
const LETTERS: [&str; 5] = ["A", "B", "C", "D", "E"];
/// Candidate tokens requested per position; the letters are almost always among them.
const N_PROBS: u32 = 20;

/// One multiple-choice item, normalised across datasets.
#[derive(Debug, Clone)]
struct Question {
    id: String,
    subject: String,
    question: String,
    choices: Vec<String>,
    answer: usize,
}

#[derive(Deserialize)]
struct MmluRow {
    question: String,
    subject: String,
    choices: Vec<String>,
    answer: usize,
}

#[derive(Deserialize)]
struct ArcRow {
    id: String,
    question: String,
    choices: ArcChoices,
    #[serde(rename = "answerKey")]
    answer_key: String,
}

#[derive(Deserialize)]
struct ArcChoices {
    label: Vec<String>,
    text: Vec<String>,
}

fn from_mmlu(rows: Vec<MmluRow>) -> Vec<Question> {
    rows.into_iter()
        .enumerate()
        .map(|(i, r)| Question {
            id: format!("{}/{i}", r.subject),
            subject: r.subject,
            question: r.question,
            choices: r.choices,
            answer: r.answer,
        })
        .collect()
}

/// ARC labels are `A`–`E` or `1`–`5`; either way the answer is the position of its label.
fn from_arc(rows: Vec<ArcRow>) -> Vec<Question> {
    rows.into_iter()
        .filter_map(|r| {
            let answer = r.choices.label.iter().position(|l| *l == r.answer_key)?;
            Some(Question {
                id: r.id,
                subject: "arc_challenge".into(),
                question: r.question,
                choices: r.choices.text,
                answer,
            })
        })
        .filter(|q| q.choices.len() <= LETTERS.len())
        .collect()
}

fn format_question(q: &Question) -> String {
    let mut s = format!("{}\n", q.question.trim());
    for (letter, choice) in LETTERS.iter().zip(&q.choices) {
        s.push_str(&format!("{letter}. {choice}\n"));
    }
    s.push_str("Answer:");
    s
}

/// The few-shot prompt: a header, the solved examples, then the open question.
fn build_prompt(q: &Question, shots: &[Question]) -> String {
    let topic = if q.subject == "arc_challenge" {
        "science".to_string()
    } else {
        q.subject.replace('_', " ")
    };
    let mut prompt =
        format!("The following are multiple choice questions (with answers) about {topic}.\n\n");
    for shot in shots {
        prompt.push_str(&format_question(shot));
        prompt.push_str(&format!(" {}\n\n", LETTERS[shot.answer]));
    }
    prompt.push_str(&format_question(q));
    prompt
}

/// The best log-probability for each answer letter among the first token's candidates.
fn letter_logprobs(probs: &[TokenProbs], n_choices: usize) -> Vec<Option<f64>> {
    let mut best = vec![None; n_choices];
    if let Some(first) = probs.first() {
        for cand in &first.top_logprobs {
            if let Some(i) = LETTERS[..n_choices]
                .iter()
                .position(|l| cand.token.trim() == *l)
            {
                let lp = best[i].get_or_insert(cand.logprob);
                *lp = lp.max(cand.logprob);
            }
        }
    }
    best
}

/// The most probable letter, or `None` when no letter is among the candidates.
fn predict(logprobs: &[Option<f64>]) -> Option<usize> {
    logprobs
        .iter()
        .enumerate()
        .filter_map(|(i, lp)| lp.map(|lp| (i, lp)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

impl Knowledge {
    /// The sampled questions plus few-shot examples per subject.
    async fn load(
        &self,
        config: &Config,
    ) -> AppResult<(Vec<Question>, BTreeMap<String, Vec<Question>>, Value)> {
        let store = self.deps.datasets.as_ref();
        let (questions, shots, files) = match config.dataset {
            Dataset::Mmlu => {
                let test = from_mmlu(load_rows(store, &MMLU_TEST).await?);
                let dev = if config.n_shot > 0 {
                    from_mmlu(load_rows(store, &MMLU_DEV).await?)
                } else {
                    Vec::new()
                };
                (test, dev, dataset_details(&[&MMLU_TEST, &MMLU_DEV]))
            }
            Dataset::ArcChallenge => {
                let test = from_arc(load_rows(store, &ARC_CHALLENGE_TEST).await?);
                let train = if config.n_shot > 0 {
                    from_arc(load_rows(store, &ARC_CHALLENGE_TRAIN).await?)
                } else {
                    Vec::new()
                };
                let files = dataset_details(&[&ARC_CHALLENGE_TEST, &ARC_CHALLENGE_TRAIN]);
                (test, train, files)
            }
        };

        let questions: Vec<Question> = if config.subjects.is_empty() {
            questions
        } else {
            let unknown: Vec<&String> = config
                .subjects
                .iter()
                .filter(|s| !questions.iter().any(|q| &q.subject == *s))
                .collect();
            if !unknown.is_empty() {
                return Err(AppError::Validation(format!(
                    "unknown subjects: {unknown:?}"
                )));
            }
            questions
                .into_iter()
                .filter(|q| config.subjects.contains(&q.subject))
                .collect()
        };
        let questions =
            sample_stratified(questions, config.limit, config.seed, |q| q.subject.clone());

        let mut by_subject: BTreeMap<String, Vec<Question>> = BTreeMap::new();
        for shot in shots {
            by_subject
                .entry(shot.subject.clone())
                .or_default()
                .push(shot);
        }
        for pool in by_subject.values_mut() {
            // MMLU's dev split has exactly five per subject; ARC's train set is drawn from.
            let picked = sample(std::mem::take(pool), config.n_shot, config.seed);
            *pool = picked;
        }
        Ok((questions, by_subject, files))
    }
}

#[async_trait]
impl Benchmark for Knowledge {
    fn descriptor(&self) -> BenchmarkDescriptor {
        BenchmarkDescriptor {
            id: "quality.knowledge",
            category: Category::ModelQuality,
            name: "Knowledge (MMLU, ARC)",
            description: "Multiple-choice knowledge and reasoning on MMLU or ARC-Challenge, scored from answer-letter probabilities",
            implemented: true,
            default_config: default_config::<Config>(),
        }
    }

    fn resolve_config(&self, raw: Option<Value>) -> AppResult<Value> {
        let value = resolve_config::<Config>(raw)?;
        let config: Config = parse_config(value.clone())?;
        validate_common(config.limit, config.concurrency, None)?;
        if config.n_shot > MAX_SHOTS {
            return Err(AppError::Validation(format!(
                "n_shot must be <= {MAX_SHOTS}"
            )));
        }
        if config.dataset != Dataset::Mmlu && !config.subjects.is_empty() {
            return Err(AppError::Validation("subjects only apply to mmlu".into()));
        }
        Ok(value)
    }

    async fn run(&self, ctx: &BenchmarkContext, config: Value) -> AppResult<BenchmarkOutcome> {
        let config: Config = parse_config(config)?;
        let (questions, shots, files) = self.load(&config).await?;
        let concurrency = effective_concurrency(ctx, config.concurrency);

        let results = eval_all(questions, concurrency, |q| {
            let prompt = build_prompt(&q, shots.get(&q.subject).map_or(&[][..], Vec::as_slice));
            let req = CompletionRequest {
                prompt,
                n_predict: 1,
                temperature: 0.0,
                seed: config.seed as i64,
                cache_prompt: false,
                ignore_eos: false,
                n_probs: Some(N_PROBS),
            };
            async move {
                let resp = ctx.llama.completion(&req).await?;
                let logprobs = letter_logprobs(&resp.probs, q.choices.len());
                Ok((q, predict(&logprobs), logprobs))
            }
        })
        .await?;

        let mut overall = Tally::default();
        let mut per_subject: BTreeMap<String, Tally> = BTreeMap::new();
        let mut invalid = 0;
        let mut samples = Vec::new();
        for (q, predicted, logprobs) in &results {
            let correct = *predicted == Some(q.answer);
            overall.add(correct);
            per_subject
                .entry(q.subject.clone())
                .or_default()
                .add(correct);
            invalid += usize::from(predicted.is_none());
            samples.push(json!({
                "id": q.id,
                "subject": q.subject,
                "expected": LETTERS[q.answer],
                "predicted": predicted.map(|i| LETTERS[i]),
                "correct": correct,
                "logprobs": LETTERS.iter().zip(logprobs).map(|(l, lp)| (l.to_string(), json!(lp))).collect::<serde_json::Map<_, _>>(),
            }));
        }

        let metrics = vec![
            Metric::new("accuracy", overall.accuracy(), "%"),
            Metric::new("invalid_rate", pct(invalid, results.len()), "%"),
            items_metric(results.len()),
        ];
        Ok(BenchmarkOutcome {
            metrics,
            details: json!({
                "datasets": files,
                "concurrency": concurrency,
                "accuracy": overall.to_json(),
                "invalid": invalid,
                "per_subject": per_subject.iter().map(|(s, t)| (s.clone(), t.to_json())).collect::<serde_json::Map<_, _>>(),
                "samples": samples,
            }),
        })
    }
}

#[cfg(test)]
mod tests;
