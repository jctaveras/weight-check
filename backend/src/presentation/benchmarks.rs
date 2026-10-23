use axum::Json;
use axum::extract::State;

use super::AppState;
use crate::service::benchmarks::CategoryCatalog;

pub async fn catalog(State(state): State<AppState>) -> Json<Vec<CategoryCatalog>> {
    Json(state.benchmarks.catalog())
}
