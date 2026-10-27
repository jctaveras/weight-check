//! `#[tauri::command]` entry points. Each one only unwraps `tauri::State` and delegates to the
//! plain command of the same name in the parent module. All are async so none of them runs on
//! (and blocks) the main thread; Tauri requires async commands that borrow state to return a
//! `Result`, hence `Ok(...)` around the infallible ones.
//!
//! Command names are snake_case. Tauri maps camelCase JS argument keys to these snake_case
//! parameters; the single-word `request`, `filter` and `id` are the same in both.

use serde_json::Value;
use tauri::ipc::Invoke;
use tauri::{Runtime, State};

use weight_check_core::domain::{ModelInfo, Run};
use weight_check_core::service::benchmark_service::RunWithMetrics;
use weight_check_core::service::benchmarks::CategoryCatalog;
use weight_check_core::service::model_service::{ActiveModel, ModelStatus};
use weight_check_core::service::system_service::SystemSnapshot;

use super::{AppState, CommandResult};

#[tauri::command]
pub async fn health(state: State<'_, AppState>) -> CommandResult<Value> {
    super::health(&state).await
}

#[tauri::command]
pub async fn system_snapshot(state: State<'_, AppState>) -> CommandResult<SystemSnapshot> {
    Ok(super::system_snapshot(&state))
}

#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> CommandResult<Vec<ModelInfo>> {
    super::list_models(&state).await
}

#[tauri::command]
pub async fn model_status(state: State<'_, AppState>) -> CommandResult<ModelStatus> {
    Ok(super::model_status(&state))
}

#[tauri::command]
pub async fn load_model(state: State<'_, AppState>, request: Value) -> CommandResult<ActiveModel> {
    super::load_model(&state, request).await
}

#[tauri::command]
pub async fn unload_model(state: State<'_, AppState>) -> CommandResult<()> {
    super::unload_model(&state).await
}

#[tauri::command]
pub async fn benchmark_catalog(state: State<'_, AppState>) -> CommandResult<Vec<CategoryCatalog>> {
    Ok(super::benchmark_catalog(&state))
}

#[tauri::command]
pub async fn list_runs(
    state: State<'_, AppState>,
    filter: Option<Value>,
) -> CommandResult<Vec<Run>> {
    super::list_runs(&state, filter).await
}

#[tauri::command]
pub async fn start_run(state: State<'_, AppState>, request: Value) -> CommandResult<Run> {
    super::start_run(&state, request).await
}

#[tauri::command]
pub async fn get_run(state: State<'_, AppState>, id: String) -> CommandResult<RunWithMetrics> {
    super::get_run(&state, &id).await
}

/// Every command, for `Builder::invoke_handler`.
pub fn handler<R: Runtime>() -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        health,
        system_snapshot,
        list_models,
        model_status,
        load_model,
        unload_model,
        benchmark_catalog,
        list_runs,
        start_run,
        get_run,
    ]
}

#[cfg(test)]
mod tests;
