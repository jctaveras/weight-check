use std::sync::Arc;

use super::*;
use crate::infra::llama::TokenLogprob;
use crate::service::testing::{FakeDatasetStore, FakeSession, ScriptedLlama, fake_quality_deps};

fn mmlu_row(subject: &str, question: &str, answer: usize) -> Value {
    json!({"subject": subject, "question": question, "choices": ["w", "x", "y", "z"], "answer": answer})
}

fn store() -> Arc<FakeDatasetStore> {
    let store = FakeDatasetStore::default();
    store.put(
        &MMLU_TEST,
        &[
            mmlu_row("astronomy", "Q-right", 1),
            mmlu_row("astronomy", "Q-wrong", 2),
            mmlu_row("virology", "Q-invalid", 0),
        ],
    );
    store.put(
        &MMLU_DEV,
        &[
            mmlu_row("astronomy", "Shot-astro", 3),
            mmlu_row("virology", "Shot-viro", 0),
        ],
    );
    store.put(
        &ARC_CHALLENGE_TEST,
        &[
            json!({"id": "arc1", "question": "Q-right", "answerKey": "2",
            "choices": {"label": ["1", "2", "3"], "text": ["a", "b", "c"]}}),
        ],
    );
    store.put(
        &ARC_CHALLENGE_TRAIN,
        &[json!({"id": "t1", "question": "Shot-arc", "answerKey": "A",
            "choices": {"label": ["A", "B"], "text": ["yes", "no"]}})],
    );
    Arc::new(store)
}

/// Answers B to "Q-right" (correct), D to "Q-wrong" (expected C) and no letter at all to
/// "Q-invalid".
fn llama() -> ScriptedLlama {
    ScriptedLlama::default().on_completion(|req| {
        let question = req.prompt.rsplit("\n\n").next().unwrap_or_default();
        let top = if question.starts_with("Q-right") {
            vec![(" B", -0.1), (" A", -3.0), ("B", -0.05)]
        } else if question.starts_with("Q-wrong") {
            vec![(" D", -0.2), (" C", -1.5)]
        } else {
            vec![(" The", -0.1), ("\n", -2.0)]
        };
        top.into_iter().map(|(t, lp)| (t.to_string(), lp)).collect()
    })
}

async fn run(config: Value) -> (BenchmarkOutcome, Arc<ScriptedLlama>) {
    let deps = QualityDeps {
        datasets: store(),
        ..fake_quality_deps()
    };
    let bench = Knowledge::new(deps);
    let llama = Arc::new(llama());
    let ctx = FakeSession::with_api(Some(llama.clone())).context();
    let config = bench.resolve_config(Some(config)).unwrap();
    (bench.run(&ctx, config).await.unwrap(), llama)
}

fn metric(o: &BenchmarkOutcome, name: &str) -> f64 {
    o.metrics.iter().find(|m| m.name == name).unwrap().value
}

#[tokio::test]
async fn mmlu_scores_letters_and_counts_invalid_answers() {
    let (outcome, llama) = run(json!({"n_shot": 1})).await;
    assert!((metric(&outcome, "accuracy") - 100.0 / 3.0).abs() < 1e-9);
    assert!((metric(&outcome, "invalid_rate") - 100.0 / 3.0).abs() < 1e-9);
    assert_eq!(metric(&outcome, "items"), 3.0);

    let samples = outcome.details["samples"].as_array().unwrap();
    assert_eq!(samples[0]["predicted"], "B");
    assert_eq!(samples[0]["correct"], true);
    // The higher of " B" and "B" counts.
    assert_eq!(samples[0]["logprobs"]["B"], -0.05);
    assert_eq!(samples[1]["predicted"], "D");
    assert_eq!(samples[2]["predicted"], Value::Null);
    assert_eq!(outcome.details["per_subject"]["astronomy"]["correct"], 1);
    assert_eq!(outcome.details["per_subject"]["virology"]["total"], 1);
    assert_eq!(outcome.details["datasets"][0]["repo"], "cais/mmlu");

    // Each prompt carries the subject header and that subject's shot with its answer.
    let prompts = llama.prompts.lock().unwrap();
    assert!(prompts[0].starts_with(
        "The following are multiple choice questions (with answers) about astronomy.\n\nShot-astro\nA. w\nB. x\nC. y\nD. z\nAnswer: D\n\nQ-right\n"
    ));
    assert!(prompts[0].ends_with("D. z\nAnswer:"));
    assert!(prompts[2].contains("Shot-viro") && !prompts[2].contains("Shot-astro"));
}

#[tokio::test]
async fn zero_shot_and_subject_filter() {
    let (outcome, llama) = run(json!({"n_shot": 0, "subjects": ["virology"]})).await;
    assert_eq!(metric(&outcome, "items"), 1.0);
    assert!(!llama.prompts.lock().unwrap()[0].contains("Shot"));
}

#[tokio::test]
async fn unknown_subject_fails_the_run() {
    let bench = Knowledge::new(QualityDeps {
        datasets: store(),
        ..fake_quality_deps()
    });
    let ctx = FakeSession::with_api(Some(Arc::new(llama()))).context();
    let config = bench
        .resolve_config(Some(json!({"subjects": ["alchemy"]})))
        .unwrap();
    let err = bench.run(&ctx, config).await.unwrap_err();
    assert!(matches!(err, AppError::Validation(ref m) if m.contains("alchemy")));
}

#[tokio::test]
async fn arc_maps_numeric_labels_to_letters() {
    let (outcome, llama) = run(json!({"dataset": "arc_challenge", "n_shot": 1})).await;
    assert_eq!(metric(&outcome, "accuracy"), 100.0);
    assert_eq!(outcome.details["samples"][0]["expected"], "B");
    let prompt = &llama.prompts.lock().unwrap()[0];
    assert!(prompt.contains("about science."));
    assert!(prompt.contains("Shot-arc\nA. yes\nB. no\nAnswer: A\n\n"));
    assert!(prompt.ends_with("C. c\nAnswer:"));
}

#[test]
fn letter_logprobs_ignore_letters_beyond_the_choices() {
    let probs = vec![TokenProbs {
        token: " E".into(),
        logprob: -0.1,
        top_logprobs: vec![
            TokenLogprob {
                token: " E".into(),
                logprob: -0.1,
            },
            TokenLogprob {
                token: " A".into(),
                logprob: -1.0,
            },
        ],
    }];
    let lp = letter_logprobs(&probs, 4);
    assert_eq!(lp, [Some(-1.0), None, None, None]);
    assert_eq!(predict(&lp), Some(0));
    assert_eq!(predict(&letter_logprobs(&[], 4)), None);
}

#[test]
fn config_validation() {
    let bench = Knowledge::new(fake_quality_deps());
    let resolved = bench.resolve_config(None).unwrap();
    assert_eq!(resolved["dataset"], "mmlu");
    assert_eq!(resolved["n_shot"], 5);
    for bad in [
        json!({"n_shot": 6}),
        json!({"limit": 0}),
        json!({"dataset": "arc_challenge", "subjects": ["virology"]}),
        json!({"dataset": "gpqa"}),
        json!({"bogus": 1}),
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
