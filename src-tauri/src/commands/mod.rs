//! IPC layer: argument parsing and result mapping only. No business logic.
//!
//! Each command here is a plain async fn over `&AppState` that parses its arguments and calls a
//! single service method. The `#[tauri::command]` wrappers in `ipc` only unwrap `tauri::State`.
//! Results serialize to exactly the JSON the old HTTP endpoints returned.

pub mod dto;
mod error;
pub mod ipc;

use std::sync::Arc;

use serde_json::{Value, json};

use weight_check_core::domain::{ModelInfo, Run};
use weight_check_core::service::benchmark_service::{BenchmarkService, RunWithMetrics};
use weight_check_core::service::benchmarks::CategoryCatalog;
use weight_check_core::service::model_service::{ActiveModel, ModelService, ModelStatus};
use weight_check_core::service::system_service::{SystemService, SystemSnapshot};

use dto::{LoadModelRequest, RunQuery, StartRunRequest};
pub use error::CommandError;

pub type CommandResult<T> = Result<T, CommandError>;

/// The services, managed by Tauri for the app's lifetime.
#[derive(Clone)]
pub struct AppState {
    pub models: Arc<ModelService>,
    pub benchmarks: Arc<BenchmarkService>,
    pub system: Arc<SystemService>,
}

/// Was `GET /health`.
pub async fn health(state: &AppState) -> CommandResult<Value> {
    state.system.health().await?;
    Ok(json!({ "status": "ok" }))
}

/// Was `GET /system`.
pub fn system_snapshot(state: &AppState) -> SystemSnapshot {
    state.system.snapshot(None)
}

/// Was `GET /models`.
pub async fn list_models(state: &AppState) -> CommandResult<Vec<ModelInfo>> {
    Ok(state.models.list_available().await?)
}

/// Was `GET /models/status`.
pub fn model_status(state: &AppState) -> ModelStatus {
    state.models.status()
}

/// Was `POST /models/load`.
pub async fn load_model(state: &AppState, request: Value) -> CommandResult<ActiveModel> {
    let request: LoadModelRequest = dto::parse(request)?;
    Ok(state.models.load(request.into()).await?)
}

/// Was `POST /models/unload`.
pub async fn unload_model(state: &AppState) -> CommandResult<()> {
    Ok(state.models.unload().await?)
}

/// Was `GET /benchmarks`.
pub fn benchmark_catalog(state: &AppState) -> Vec<CategoryCatalog> {
    state.benchmarks.catalog()
}

/// Was `GET /runs`. A missing or `null` filter lists everything.
pub async fn list_runs(state: &AppState, filter: Option<Value>) -> CommandResult<Vec<Run>> {
    let query: RunQuery = match filter {
        Some(Value::Null) | None => RunQuery::default(),
        Some(filter) => dto::parse(filter)?,
    };
    Ok(state.benchmarks.list_runs(query.try_into()?).await?)
}

/// Was `POST /runs`.
pub async fn start_run(state: &AppState, request: Value) -> CommandResult<Run> {
    let request: StartRunRequest = dto::parse(request)?;
    Ok(state
        .benchmarks
        .start_run(&request.benchmark_id, request.config)
        .await?)
}

/// Was `GET /runs/{id}`.
pub async fn get_run(state: &AppState, id: &str) -> CommandResult<RunWithMetrics> {
    let id = dto::parse_run_id(id)?;
    Ok(state.benchmarks.get_run(id).await?)
}

/// Asks a queued or running run to stop; it becomes `cancelled` shortly after.
pub async fn cancel_run(state: &AppState, id: &str) -> CommandResult<Run> {
    let id = dto::parse_run_id(id)?;
    Ok(state.benchmarks.cancel_run(id).await?)
}

/// Deletes a finished run and its metrics.
pub async fn delete_run(state: &AppState, id: &str) -> CommandResult<()> {
    let id = dto::parse_run_id(id)?;
    Ok(state.benchmarks.delete_run(id).await?)
}

/// Completed runs of one benchmark, with metrics, for the Compare page.
pub async fn compare_runs(state: &AppState, ids: &[String]) -> CommandResult<Vec<RunWithMetrics>> {
    let ids = dto::parse_run_ids(ids)?;
    Ok(state.benchmarks.compare_runs(ids).await?)
}

#[cfg(test)]
mod tests;
