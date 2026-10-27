use serde::Serialize;

use weight_check_core::error::AppError;

/// The error every command rejects with. Serializes as `{"code": ..., "message": ...}`, with the
/// same codes the old HTTP API put in its `error` object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl From<AppError> for CommandError {
    fn from(err: AppError) -> Self {
        let (code, server_side) = match &err {
            AppError::NotFound(_) => ("not_found", false),
            AppError::NoModelLoaded => ("no_model_loaded", false),
            AppError::Busy(_) => ("busy", false),
            AppError::Validation(_) => ("validation", false),
            AppError::NotImplemented(_) => ("not_implemented", true),
            AppError::Llama(_) => ("llama_server", true),
            AppError::Database(_) | AppError::Internal(_) => ("internal", true),
        };
        // Same rule as the old HTTP mapping: log what would have been a 5xx.
        if server_side {
            tracing::error!("{err}");
        }
        Self {
            code,
            message: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests;
