use std::sync::Arc;

use super::*;
use crate::infra::llama::ChatResponse;
use crate::service::testing::{
    FakeDatasetStore, FakeSession, ScriptedLlama, chat_reply, fake_quality_deps, last_message,
};

#[test]
fn extracts_the_final_answer() {
    for (response, want) in [
        ("so 3 + 4 = 7\n#### 7", Some(7.0)),
        ("#### $1,000.", Some(1000.0)),
        ("#### **-12.5**", Some(-12.5)),
        ("first #### 3 then #### 4", Some(4.0)),
        ("The answer is \\boxed{42}.", Some(42.0)),
        ("\\boxed{\\$18}", Some(18.0)),
        ("She makes 9 * 2 = 18 dollars.", Some(18.0)),
        ("The total is $5.50.", Some(5.5)),
        ("10 - 3 leaves seven", Some(3.0)),
        ("no numbers here", None),
    ] {
        assert_eq!(extract_answer(response), want, "{response}");
    }
}

#[test]
fn reads_the_reference_answer() {
    assert_eq!(
        reference_answer("9 * 2 = <<9*2=18>>18\n#### 18"),
        Some(18.0)
    );
    assert_eq!(reference_answer("#### 1,234"), Some(1234.0));
}

fn store() -> Arc<FakeDatasetStore> {
    let store = FakeDatasetStore::default();
    store.put(
        &GSM8K_TEST,
        &[
            json!({"question": "Q-right", "answer": "... #### 18"}),
            json!({"question": "Q-wrong", "answer": "... #### 5"}),
            json!({"question": "Q-cutoff", "answer": "... #### 3"}),
        ],
    );
    Arc::new(store)
}

fn reply(req: &crate::infra::llama::ChatRequest) -> ChatResponse {
    let msg = last_message(req);
    if msg.ends_with("Q-right") {
        chat_reply("9 * 2 = 18\n#### 18")
    } else if msg.ends_with("Q-wrong") {
        chat_reply("#### 6")
    } else {
        // Ran out of tokens mid-thought: even a number in the text doesn't count.
        ChatResponse {
            finish_reason: "length".into(),
            ..chat_reply("Let me think about 3")
        }
    }
}

#[tokio::test]
async fn scores_exact_numeric_matches_and_counts_cutoffs_as_invalid() {
    let bench = Math::new(QualityDeps {
        datasets: store(),
        ..fake_quality_deps()
    });
    let llama = Arc::new(ScriptedLlama::default().on_chat(reply));
    let ctx = FakeSession::with_api(Some(llama.clone())).context();
    let config = bench
        .resolve_config(Some(json!({"thinking": "off", "max_tokens": 512})))
        .unwrap();
    let outcome = bench.run(&ctx, config).await.unwrap();

    let metric = |name: &str| {
        outcome
            .metrics
            .iter()
            .find(|m| m.name == name)
            .unwrap()
            .value
    };
    assert!((metric("accuracy") - 100.0 / 3.0).abs() < 1e-9);
    assert!((metric("invalid_rate") - 100.0 / 3.0).abs() < 1e-9);
    assert_eq!(metric("completion_tokens_mean"), 20.0);
    assert_eq!(metric("items"), 3.0);
    let samples = outcome.details["samples"].as_array().unwrap();
    assert_eq!(samples[1]["predicted"], 6.0);
    assert_eq!(samples[2]["truncated"], true);
    assert_eq!(samples[2]["predicted"], Value::Null);

    let requests = llama.chat_requests.lock().unwrap();
    assert!(last_message(&requests[0]).starts_with(INSTRUCTION));
    assert_eq!(requests[0].max_tokens, 512);
    assert_eq!(
        requests[0].chat_template_kwargs,
        Some(json!({"enable_thinking": false}))
    );
}

#[test]
fn config_validation() {
    let bench = Math::new(fake_quality_deps());
    assert_eq!(bench.resolve_config(None).unwrap()["thinking"], "auto");
    for bad in [
        json!({"max_tokens": 0}),
        json!({"concurrency": 0}),
        json!({"thinking": "maybe"}),
    ] {
        assert!(matches!(
            bench.resolve_config(Some(bad)),
            Err(crate::error::AppError::Validation(_))
        ));
    }
}
