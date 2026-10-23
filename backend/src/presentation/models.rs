use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use super::AppState;
use super::dto::LoadModelRequest;
use super::extract::ApiJson;
use crate::domain::ModelInfo;
use crate::error::AppResult;
use crate::service::model_service::{ActiveModel, ModelStatus};

pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<ModelInfo>>> {
    state.models.list_available().await.map(Json)
}

pub async fn status(State(state): State<AppState>) -> Json<ModelStatus> {
    Json(state.models.status())
}

pub async fn load(
    State(state): State<AppState>,
    ApiJson(body): ApiJson<LoadModelRequest>,
) -> AppResult<Json<ActiveModel>> {
    state.models.load(body.into()).await.map(Json)
}

pub async fn unload(State(state): State<AppState>) -> AppResult<StatusCode> {
    state.models.unload().await.map(|_| StatusCode::NO_CONTENT)
}
