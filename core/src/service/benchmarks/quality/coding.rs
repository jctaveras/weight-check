mod livecodebench;

use std::collections::BTreeMap;
use std::sync::LazyLock;
use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::sources::{HUMANEVAL, LIVECODEBENCH_V6};
use super::{
    QualityDeps, RESPONSE_PREVIEW_CHARS, Tally, Thinking, chat_request, dataset_details,
    effective_concurrency, eval_all, items_metric, load_rows, pct, sample, truncate,
    validate_common,
};
use crate::domain::{Category, Metric};
use crate::error::{AppError, AppResult};
use crate::infra::llama::ChatMessage;
use crate::infra::sandbox::CodeRunner;
use crate::service::benchmarks::{
    Benchmark, BenchmarkContext, BenchmarkDescriptor, BenchmarkOutcome, default_config,
    parse_config, resolve_config,
};

/// Code generation scored by running the problem's tests against the model's program
/// (HumanEval or LiveCodeBench), reported as pass@1 with greedy decoding.
pub struct Coding {
    deps: QualityDeps,
}

impl Coding {
    pub fn new(deps: QualityDeps) -> Self {
        Self { deps }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dataset {
    #[default]
    Humaneval,
    Livecodebench,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub dataset: Dataset,
    pub limit: u32,
    pub seed: u64,
    /// Cap on generated tokens, including any thinking.
    pub max_tokens: u32,
    pub thinking: Thinking,
    pub concurrency: u32,
    /// Seconds each test program may run before it counts as failed.
    pub timeout_secs: u32,
    /// LiveCodeBench only: tests run per problem (public first, then private).
    pub max_tests: u32,
    /// LiveCodeBench only: keep problems from contests on or after this date (`YYYY-MM-DD`),
    /// to stay clear of the model's training data.
    pub since: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dataset: Dataset::Humaneval,
            limit: 200,
            seed: 0,
            max_tokens: 4096,
            thinking: Thinking::Auto,
            concurrency: 1,
            timeout_secs: 10,
            max_tests: 20,
            since: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct HumanEvalRow {
    task_id: String,
    prompt: String,
    test: String,
    entry_point: String,
}

static CODE_BLOCK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)```[ \t]*(?:python|py|Python)?[ \t]*\n(.*?)```").unwrap());
static DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap());

/// The code in a response: the longest fenced block, or `None` without one.
pub(crate) fn extract_code(response: &str) -> Option<String> {
    CODE_BLOCK
        .captures_iter(response)
        .map(|c| c[1].to_string())
        .max_by_key(String::len)
        .filter(|c| !c.trim().is_empty())
}

/// HumanEval program: the prompt (signature + docstring), the model's code, then the tests.
/// Works whether the model returns a whole function (it redefines the stub) or just a body.
fn humaneval_program(row: &HumanEvalRow, code: &str) -> String {
    format!(
        "{}\n{}\n\n{}\n\ncheck({})\n",
        row.prompt, code, row.test, row.entry_point
    )
}

fn humaneval_messages(row: &HumanEvalRow) -> Vec<ChatMessage> {
    vec![ChatMessage::user(format!(
        "Complete the following Python function. Return the complete function, including the signature and any imports it needs, in a single ```python code block.\n\n```python\n{}```",
        row.prompt
    ))]
}

/// What happened to one problem.
#[derive(Debug, Default)]
struct Verdict {
    passed: bool,
    timed_out: bool,
    /// No code block in the response, or it was cut off by `max_tokens`.
    invalid: bool,
    tests_run: usize,
    error: Option<String>,
}

impl Verdict {
    fn invalid(reason: &str) -> Self {
        Self {
            invalid: true,
            error: Some(reason.into()),
            ..Self::default()
        }
    }
}

/// Keep `details` small: the tail of stderr holds the traceback's last frames.
fn stderr_tail(s: &str) -> String {
    let start = s.char_indices().rev().nth(500).map_or(0, |(i, _)| i);
    s[start..].to_string()
}

struct Problem {
    id: String,
    messages: Vec<ChatMessage>,
    kind: Kind,
}

enum Kind {
    HumanEval(HumanEvalRow),
    LiveCodeBench {
        tests: Vec<livecodebench::TestCase>,
        func_name: Option<String>,
        difficulty: String,
    },
}

impl Coding {
    async fn load(&self, config: &Config) -> AppResult<(Vec<Problem>, Value)> {
        let store = self.deps.datasets.as_ref();
        match config.dataset {
            Dataset::Humaneval => {
                let rows: Vec<HumanEvalRow> = load_rows(store, &HUMANEVAL).await?;
                let problems = sample(rows, config.limit, config.seed)
                    .into_iter()
                    .map(|row| Problem {
                        id: row.task_id.clone(),
                        messages: humaneval_messages(&row),
                        kind: Kind::HumanEval(row),
                    })
                    .collect();
                Ok((problems, dataset_details(&[&HUMANEVAL])))
            }
            Dataset::Livecodebench => {
                let rows: Vec<livecodebench::Problem> = load_rows(store, &LIVECODEBENCH_V6).await?;
                let rows: Vec<_> = rows
                    .into_iter()
                    .filter(|p| config.since.as_ref().is_none_or(|s| p.contest_date >= *s))
                    .collect();
                let mut problems = Vec::new();
                for p in sample(rows, config.limit, config.seed) {
                    problems.push(Problem {
                        id: format!("{}/{}", p.platform, p.question_id),
                        messages: livecodebench::messages(&p),
                        kind: Kind::LiveCodeBench {
                            tests: livecodebench::tests(&p, config.max_tests as usize)?,
                            func_name: livecodebench::func_name(&p),
                            difficulty: p.difficulty.clone(),
                        },
                    });
                }
                Ok((problems, dataset_details(&[&LIVECODEBENCH_V6])))
            }
        }
    }
}

/// Runs the problem's tests against `code`, stopping at the first failure.
async fn judge(
    runner: &dyn CodeRunner,
    kind: &Kind,
    code: &str,
    timeout: Duration,
) -> AppResult<Verdict> {
    let mut verdict = Verdict::default();
    match kind {
        Kind::HumanEval(row) => {
            let out = runner
                .run(&humaneval_program(row, code), "", timeout)
                .await?;
            verdict.tests_run = 1;
            verdict.passed = out.success;
            verdict.timed_out = out.timed_out;
            if !out.success && !out.timed_out {
                verdict.error = Some(stderr_tail(&out.stderr));
            }
        }
        Kind::LiveCodeBench {
            tests, func_name, ..
        } => {
            verdict.passed = !tests.is_empty();
            for test in tests {
                let functional = test.testtype == "functional";
                let program = match (functional, func_name) {
                    (true, Some(f)) => livecodebench::functional_program(code, f),
                    (true, None) => {
                        verdict.passed = false;
                        verdict.error = Some("functional test without func_name".into());
                        break;
                    }
                    (false, _) => livecodebench::stdin_program(code),
                };
                let out = runner.run(&program, &test.input, timeout).await?;
                verdict.tests_run += 1;
                let matches = if functional {
                    livecodebench::functional_matches(&out.stdout, &test.output)
                } else {
                    livecodebench::stdout_matches(&out.stdout, &test.output)
                };
                if !(out.success && matches) {
                    verdict.passed = false;
                    verdict.timed_out = out.timed_out;
                    verdict.error = Some(if out.timed_out {
                        format!("test {} timed out", verdict.tests_run)
                    } else if !out.success {
                        stderr_tail(&out.stderr)
                    } else {
                        format!("test {}: wrong answer", verdict.tests_run)
                    });
                    break;
                }
            }
        }
    }
    Ok(verdict)
}

#[async_trait]
impl Benchmark for Coding {
    fn descriptor(&self) -> BenchmarkDescriptor {
        BenchmarkDescriptor {
            id: "quality.coding",
            category: Category::ModelQuality,
            name: "Coding (HumanEval, LiveCodeBench)",
            description: "Python problems scored by running their tests (pass@1). Runs model-written code locally with PYTHON_BIN, unsandboxed",
            implemented: true,
            default_config: default_config::<Config>(),
        }
    }

    fn resolve_config(&self, raw: Option<Value>) -> AppResult<Value> {
        let value = resolve_config::<Config>(raw)?;
        let config: Config = parse_config(value.clone())?;
        validate_common(config.limit, config.concurrency, Some(config.max_tokens))?;
        if config.timeout_secs == 0 || config.max_tests == 0 {
            return Err(AppError::Validation(
                "timeout_secs and max_tests must be > 0".into(),
            ));
        }
        if let Some(since) = &config.since {
            if config.dataset != Dataset::Livecodebench {
                return Err(AppError::Validation(
                    "since only applies to livecodebench".into(),
                ));
            }
            if !DATE.is_match(since) {
                return Err(AppError::Validation(
                    "since must be a YYYY-MM-DD date".into(),
                ));
            }
        }
        Ok(value)
    }

    async fn run(&self, ctx: &BenchmarkContext, config: Value) -> AppResult<BenchmarkOutcome> {
        let config: Config = parse_config(config)?;
        let (problems, files) = self.load(&config).await?;
        if problems.is_empty() {
            return Err(AppError::Validation(
                "no problems match the config (check since)".into(),
            ));
        }
        let concurrency = effective_concurrency(ctx, config.concurrency);
        let timeout = Duration::from_secs(config.timeout_secs.into());
        let runner = self.deps.code_runner.as_ref();

        let results = eval_all(problems, concurrency, |problem| {
            let req = chat_request(
                problem.messages.clone(),
                config.max_tokens,
                config.seed,
                config.thinking,
            );
            async move {
                let resp = ctx.llama.chat(&req).await?;
                let verdict = if resp.finish_reason == "length" {
                    Verdict::invalid("response cut off by max_tokens")
                } else if let Some(code) = extract_code(&resp.content) {
                    judge(runner, &problem.kind, &code, timeout).await?
                } else {
                    Verdict::invalid("no code block in the response")
                };
                Ok((problem, resp, verdict))
            }
        })
        .await?;

        let mut tally = Tally::default();
        let (mut timeouts, mut invalid) = (0, 0);
        let mut by_difficulty: BTreeMap<String, Tally> = BTreeMap::new();
        let mut samples = Vec::new();
        for (problem, resp, verdict) in &results {
            tally.add(verdict.passed);
            timeouts += usize::from(verdict.timed_out);
            invalid += usize::from(verdict.invalid);
            let difficulty = match &problem.kind {
                Kind::LiveCodeBench { difficulty, .. } => Some(difficulty.clone()),
                Kind::HumanEval(_) => None,
            };
            if let Some(d) = &difficulty {
                by_difficulty
                    .entry(d.clone())
                    .or_default()
                    .add(verdict.passed);
            }
            samples.push(json!({
                "id": problem.id,
                "difficulty": difficulty,
                "passed": verdict.passed,
                "timed_out": verdict.timed_out,
                "invalid": verdict.invalid,
                "tests_run": verdict.tests_run,
                "error": verdict.error,
                "completion_tokens": resp.completion_tokens,
                "response": truncate(&resp.content, RESPONSE_PREVIEW_CHARS),
            }));
        }

        let metrics = vec![
            Metric::new("pass_at_1", tally.accuracy(), "%"),
            Metric::new("timeout_rate", pct(timeouts, results.len()), "%"),
            Metric::new("invalid_rate", pct(invalid, results.len()), "%"),
            items_metric(results.len()),
        ];
        Ok(BenchmarkOutcome {
            metrics,
            details: json!({
                "datasets": files,
                "concurrency": concurrency,
                "pass_at_1": tally.to_json(),
                "timeouts": timeouts,
                "invalid": invalid,
                "per_difficulty": by_difficulty.iter().map(|(d, t)| (d.clone(), t.to_json())).collect::<serde_json::Map<_, _>>(),
                "samples": samples,
            }),
        })
    }
}

#[cfg(test)]
mod tests;
