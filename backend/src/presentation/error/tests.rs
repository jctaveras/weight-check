use serde_json::Value;

use super::*;

async fn render(err: AppError) -> (StatusCode, Value) {
    let resp = err.into_response();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn maps_every_variant_to_status_and_code() {
    let cases = [
        (AppError::NotFound("run 1".into()), 404, "not_found"),
        (AppError::NoModelLoaded, 409, "no_model_loaded"),
        (AppError::Busy("x".into()), 409, "busy"),
        (AppError::Validation("x".into()), 422, "validation"),
        (AppError::NotImplemented("x".into()), 501, "not_implemented"),
        (AppError::Llama("x".into()), 502, "llama_server"),
        (AppError::Database("x".into()), 500, "internal"),
        (AppError::Internal("x".into()), 500, "internal"),
    ];
    for (err, status, code) in cases {
        let message = err.to_string();
        let (got, body) = render(err).await;
        assert_eq!(got.as_u16(), status, "{code}");
        assert_eq!(body["error"]["code"], code);
        assert_eq!(body["error"]["message"], message);
    }
}
