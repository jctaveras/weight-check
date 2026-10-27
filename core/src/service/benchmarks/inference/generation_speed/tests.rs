use super::*;

#[test]
fn resolve_config_rejects_empty_or_zero_values() {
    for bad in [
        json!({"prompts": []}),
        json!({"repetitions": 0}),
        json!({"n_predict": 0}),
    ] {
        assert!(matches!(
            GenerationSpeed.resolve_config(Some(bad)),
            Err(AppError::Validation(_))
        ));
    }
}
