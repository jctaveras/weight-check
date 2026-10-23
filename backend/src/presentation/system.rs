use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use super::AppState;
use crate::error::AppResult;
use crate::service::system_service::SystemSnapshot;

pub async fn health(State(state): State<AppState>) -> AppResult<Json<Value>> {
    state.system.health().await?;
    Ok(Json(json!({ "status": "ok" })))
}

pub async fn snapshot(State(state): State<AppState>) -> Json<SystemSnapshot> {
    Json(state.system.snapshot(None))
}
