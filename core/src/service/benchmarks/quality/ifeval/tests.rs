use std::sync::Arc;

use super::*;
use crate::service::testing::{
    FakeDatasetStore, FakeSession, ScriptedLlama, chat_reply, fake_quality_deps, last_message,
};

fn store() -> Arc<FakeDatasetStore> {
    let store = FakeDatasetStore::default();
    store.put(
        &IFEVAL,
        &[
            // Both instructions followed.
            json!({"key": 1, "prompt": "P1", "instruction_id_list": ["punctuation:no_comma", "change_case:english_lowercase"], "kwargs": [{}, {}]}),
            // Strict fails on the leading "Sure!" line; loose drops it and passes.
            json!({"key": 2, "prompt": "P2", "instruction_id_list": ["startend:quotation"], "kwargs": [{}]}),
            // One of two followed; the language instruction is skipped.
            json!({"key": 3, "prompt": "P3", "instruction_id_list": ["punctuation:no_comma", "keywords:existence", "language:response_language"], "kwargs": [{}, {"keywords": ["moon"]}, {"language": "fr"}]}),
            // Only an unsupported instruction: dropped entirely.
            json!({"key": 4, "prompt": "P4", "instruction_id_list": ["language:response_language"], "kwargs": [{"language": "de"}]}),
        ],
    );
    Arc::new(store)
}

#[tokio::test]
async fn scores_prompt_and_instruction_level_strict_and_loose() {
    let bench = InstructionFollowing::new(QualityDeps {
        datasets: store(),
        ..fake_quality_deps()
    });
    let llama = ScriptedLlama::default().on_chat(|req| {
        chat_reply(match last_message(req) {
            "P1" => "all lowercase and no commas",
            "P2" => "Sure!\n\"quoted answer\"",
            _ => "No moon, sorry",
        })
    });
    let ctx = FakeSession::with_api(Some(Arc::new(llama))).context();
    let config = bench.resolve_config(None).unwrap();
    let outcome = bench.run(&ctx, config).await.unwrap();

    let metric = |name: &str| {
        outcome
            .metrics
            .iter()
            .find(|m| m.name == name)
            .unwrap()
            .value
    };
    assert_eq!(metric("items"), 3.0);
    assert!((metric("prompt_strict_acc") - 100.0 / 3.0).abs() < 1e-9);
    assert!((metric("prompt_loose_acc") - 200.0 / 3.0).abs() < 1e-9);
    // Instructions: 2 + 1 + 2 = 5. Strict: P1 both, P3 keyword = 3. Loose adds P2.
    assert_eq!(metric("inst_strict_acc"), 60.0);
    assert_eq!(metric("inst_loose_acc"), 80.0);

    let d = &outcome.details;
    assert_eq!(d["skipped_instructions"]["language:response_language"], 2);
    assert_eq!(d["per_instruction"]["punctuation:no_comma"]["total"], 2);
    assert_eq!(
        d["per_instruction"]["punctuation:no_comma"]["strict_accuracy"],
        50.0
    );
    assert_eq!(d["samples"][1]["prompt_strict"], false);
    assert_eq!(d["samples"][1]["prompt_loose"], true);
    assert_eq!(d["samples"][2]["instructions"].as_array().unwrap().len(), 2);
}

#[test]
fn empty_responses_never_pass() {
    let inst = Instruction {
        id: "punctuation:no_comma".into(),
        kwargs: Map::new(),
    };
    assert_eq!(follows(&inst, "  "), (false, false));
    assert_eq!(follows(&inst, "fine"), (true, true));
}
