use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use super::AppState;
use super::dto::{RunQuery, StartRunRequest};
use super::extract::{ApiJson, ApiQuery};
use crate::domain::Run;
use crate::error::{AppError, AppResult};
use crate::service::benchmark_service::RunWithMetrics;

pub async fn start(
    State(state): State<AppState>,
    ApiJson(body): ApiJson<StartRunRequest>,
) -> AppResult<(StatusCode, Json<Run>)> {
    let run = state
        .benchmarks
        .start_run(&body.benchmark_id, body.config)
        .await?;
    Ok((StatusCode::ACCEPTED, Json(run)))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<RunWithMetrics>> {
    let id: Uuid = id
        .parse()
        .map_err(|_| AppError::Validation(format!("invalid run id '{id}'")))?;
    state.benchmarks.get_run(id).await.map(Json)
}

pub async fn list(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<RunQuery>,
) -> AppResult<Json<Vec<Run>>> {
    state
        .benchmarks
        .list_runs(query.try_into()?)
        .await
        .map(Json)
}
