use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::infra::llama::ChatResponse;
use crate::infra::sandbox::{ExecResult, PythonRunner};
use crate::service::testing::{
    FakeCodeRunner, FakeDatasetStore, FakeSession, ScriptedLlama, chat_reply, fake_quality_deps,
    last_message,
};

#[test]
fn extracts_the_longest_fenced_block() {
    let r = "Here:\n```python\ndef f():\n    return 1\n```\nTest:\n```\nf()\n```";
    assert_eq!(extract_code(r).as_deref(), Some("def f():\n    return 1\n"));
    assert_eq!(
        extract_code("```py\nx = 1\n```").as_deref(),
        Some("x = 1\n")
    );
    assert_eq!(extract_code("no code"), None);
    assert_eq!(extract_code("```python\n\n```"), None);
}

fn humaneval_row(task: &str, entry: &str) -> Value {
    json!({
        "task_id": task,
        "prompt": format!("def {entry}(a, b):\n    \"\"\"Add.\"\"\"\n"),
        "canonical_solution": "    return a + b\n",
        "test": "def check(candidate):\n    assert candidate(1, 2) == 3\n",
        "entry_point": entry,
    })
}

fn deps(runner: FakeCodeRunner) -> QualityDeps {
    let store = FakeDatasetStore::default();
    store.put(
        &HUMANEVAL,
        &[
            humaneval_row("HumanEval/0", "add"),
            humaneval_row("HumanEval/1", "slow"),
            humaneval_row("HumanEval/2", "prose"),
            humaneval_row("HumanEval/3", "cut"),
        ],
    );
    let lcb = |id: &str, date: &str, public: &str| {
        json!({
            "question_id": id, "question_title": id, "question_content": format!("Problem {id}"),
            "platform": "atcoder", "contest_date": date, "difficulty": "easy", "starter_code": "",
            "public_test_cases": public, "private_test_cases": "[]", "metadata": "{}",
        })
    };
    store.put(
        &LIVECODEBENCH_V6,
        &[
            lcb("old", "2025-01-04T00:00:00", r#"[{"input": "1", "output": "1", "testtype": "stdin"}]"#),
            lcb(
                "new",
                "2025-03-01T00:00:00",
                r#"[{"input": "1", "output": "2", "testtype": "stdin"}, {"input": "2", "output": "4", "testtype": "stdin"}]"#,
            ),
        ],
    );
    QualityDeps {
        datasets: Arc::new(store),
        code_runner: Arc::new(runner),
    }
}

fn reply(req: &crate::infra::llama::ChatRequest) -> ChatResponse {
    let msg = last_message(req);
    if msg.contains("def add(") {
        chat_reply("```python\ndef add(a, b):\n    return a + b\n```")
    } else if msg.contains("def slow(") {
        chat_reply("```python\ndef slow(a, b):\n    while True: pass\n```")
    } else if msg.contains("def prose(") {
        chat_reply("You should add the numbers.")
    } else if msg.contains("def cut(") {
        ChatResponse {
            finish_reason: "length".into(),
            ..chat_reply("```python\ndef cut(a, b):\n")
        }
    } else {
        chat_reply("```python\nprint(int(input()) * 2)\n```")
    }
}

/// Passes programs by what they contain, as a real interpreter would for these fixtures.
fn runner() -> FakeCodeRunner {
    FakeCodeRunner::new(|program, stdin| {
        if program.contains("while True") {
            ExecResult {
                timed_out: true,
                ..ExecResult::default()
            }
        } else if program.contains("int(input()) * 2") {
            let n: i64 = stdin.trim().parse().unwrap();
            ExecResult {
                success: true,
                stdout: format!("{}\n", n * 2),
                ..ExecResult::default()
            }
        } else {
            ExecResult {
                success: program.contains("return a + b") && program.ends_with("check(add)\n"),
                stderr: "AssertionError".into(),
                ..ExecResult::default()
            }
        }
    })
}

async fn run(config: Value) -> (BenchmarkOutcome, Arc<ScriptedLlama>) {
    let bench = Coding::new(deps(runner()));
    let llama = Arc::new(ScriptedLlama::default().on_chat(reply));
    let ctx = FakeSession::with_api(Some(llama.clone())).context();
    let config = bench.resolve_config(Some(config)).unwrap();
    (bench.run(&ctx, config).await.unwrap(), llama)
}

fn metric(o: &BenchmarkOutcome, name: &str) -> f64 {
    o.metrics.iter().find(|m| m.name == name).unwrap().value
}

#[tokio::test]
async fn humaneval_pass_at_1_with_timeouts_and_invalid_responses() {
    let (outcome, llama) = run(json!({})).await;
    assert_eq!(metric(&outcome, "items"), 4.0);
    assert_eq!(metric(&outcome, "pass_at_1"), 25.0);
    assert_eq!(metric(&outcome, "timeout_rate"), 25.0);
    assert_eq!(metric(&outcome, "invalid_rate"), 50.0);

    let samples = outcome.details["samples"].as_array().unwrap();
    assert_eq!(samples[0]["passed"], true);
    assert_eq!(samples[1]["timed_out"], true);
    assert_eq!(samples[2]["error"], "no code block in the response");
    assert_eq!(samples[3]["error"], "response cut off by max_tokens");
    assert_eq!(samples[3]["tests_run"], 0);

    let first = &llama.chat_requests.lock().unwrap()[0];
    assert!(last_message(first).contains("```python\ndef add(a, b):\n    \"\"\"Add.\"\"\"\n```"));
}

#[tokio::test]
async fn livecodebench_runs_each_test_and_filters_by_date() {
    let (outcome, llama) = run(json!({"dataset": "livecodebench"})).await;
    assert_eq!(metric(&outcome, "items"), 2.0);
    // "old" expects 1 for input 1 but the program doubles: wrong answer on the first test.
    let samples = outcome.details["samples"].as_array().unwrap();
    assert_eq!(samples[0]["passed"], false);
    assert_eq!(samples[0]["error"], "test 1: wrong answer");
    assert_eq!(samples[1]["passed"], true);
    assert_eq!(samples[1]["tests_run"], 2);
    assert_eq!(outcome.details["per_difficulty"]["easy"]["total"], 2);
    assert_eq!(
        llama.chat_requests.lock().unwrap()[0].messages[0].role,
        "system"
    );

    let (outcome, _) = run(json!({"dataset": "livecodebench", "since": "2025-02-01"})).await;
    assert_eq!(metric(&outcome, "items"), 1.0);
    assert_eq!(metric(&outcome, "pass_at_1"), 100.0);
}

#[tokio::test]
async fn no_matching_problems_fails_the_run() {
    let bench = Coding::new(deps(runner()));
    let ctx =
        FakeSession::with_api(Some(Arc::new(ScriptedLlama::default().on_chat(reply)))).context();
    let config = bench
        .resolve_config(Some(
            json!({"dataset": "livecodebench", "since": "2030-01-01"}),
        ))
        .unwrap();
    assert!(matches!(
        bench.run(&ctx, config).await,
        Err(AppError::Validation(_))
    ));
}

#[test]
fn config_validation() {
    let bench = Coding::new(fake_quality_deps());
    let resolved = bench.resolve_config(None).unwrap();
    assert_eq!(resolved["dataset"], "humaneval");
    assert_eq!(resolved["since"], Value::Null);
    for bad in [
        json!({"timeout_secs": 0}),
        json!({"max_tests": 0}),
        json!({"since": "2025-01-01"}),
        json!({"dataset": "livecodebench", "since": "Jan 2025"}),
    ] {
        assert!(
            matches!(
                bench.resolve_config(Some(bad.clone())),
                Err(AppError::Validation(_))
            ),
            "{bad}"
        );
    }
}

/// The assembled HumanEval program under a real python3, for both answer shapes.
#[tokio::test]
async fn humaneval_program_runs_under_python() {
    let Ok(out) = std::process::Command::new("python3")
        .arg("--version")
        .output()
    else {
        return;
    };
    if !out.status.success() {
        return;
    }
    let row: HumanEvalRow = serde_json::from_value(humaneval_row("HumanEval/0", "add")).unwrap();
    let py = PythonRunner::new("python3");
    let timeout = Duration::from_secs(10);
    for (code, passes) in [
        ("def add(a, b):\n    return a + b\n", true),
        ("    return a + b\n", true),
        ("def add(a, b):\n    return a - b\n", false),
    ] {
        let out = py
            .run(&humaneval_program(&row, code), "", timeout)
            .await
            .unwrap();
        assert_eq!(out.success, passes, "{code:?}: {}", out.stderr);
    }
}
