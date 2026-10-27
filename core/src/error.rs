use thiserror::Error;

/// Domain-level error shared by all layers. The app's command layer maps it to error codes.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0} not found")]
    NotFound(String),
    #[error("no model is loaded")]
    NoModelLoaded,
    #[error("{0}")]
    Busy(String),
    #[error("{0}")]
    Validation(String),
    #[error("{0} is not implemented yet")]
    NotImplemented(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("llama-server error: {0}")]
    Llama(String),
    #[error("{0}")]
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;
