use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::error::AppError;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            AppError::NoModelLoaded => (StatusCode::CONFLICT, "no_model_loaded"),
            AppError::Busy(_) => (StatusCode::CONFLICT, "busy"),
            AppError::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "validation"),
            AppError::NotImplemented(_) => (StatusCode::NOT_IMPLEMENTED, "not_implemented"),
            AppError::Llama(_) => (StatusCode::BAD_GATEWAY, "llama_server"),
            AppError::Database(_) | AppError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        };
        if status.is_server_error() {
            tracing::error!("{self}");
        }
        let body = json!({ "error": { "code": code, "message": self.to_string() } });
        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
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
}
