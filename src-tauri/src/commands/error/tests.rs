use serde_json::json;

use super::*;

#[test]
fn maps_every_variant_to_code_and_message() {
    let cases = [
        (AppError::NotFound("run 1".into()), "not_found"),
        (AppError::NoModelLoaded, "no_model_loaded"),
        (AppError::Busy("x".into()), "busy"),
        (AppError::Validation("x".into()), "validation"),
        (AppError::NotImplemented("x".into()), "not_implemented"),
        (AppError::Llama("x".into()), "llama_server"),
        (AppError::Database("x".into()), "internal"),
        (AppError::Internal("x".into()), "internal"),
    ];
    for (err, code) in cases {
        let message = err.to_string();
        let body = serde_json::to_value(CommandError::from(err)).unwrap();
        assert_eq!(body, json!({ "code": code, "message": message }));
    }
}
