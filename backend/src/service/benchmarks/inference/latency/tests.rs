use super::*;

#[test]
fn resolve_config_needs_two_tokens_and_a_repetition() {
    for bad in [json!({"repetitions": 0}), json!({"n_predict": 1})] {
        assert!(matches!(
            Latency.resolve_config(Some(bad)),
            Err(AppError::Validation(_))
        ));
    }
    assert!(
        Latency
            .resolve_config(Some(json!({"n_predict": 2})))
            .is_ok()
    );
}
