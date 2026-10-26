//! HTTP layer: routing, payload parsing and response mapping only. No business logic.

mod benchmarks;
mod dto;
mod error;
mod extract;
mod models;
mod runs;
mod system;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::service::benchmark_service::BenchmarkService;
use crate::service::model_service::ModelService;
use crate::service::system_service::SystemService;

#[derive(Clone)]
pub struct AppState {
    pub models: Arc<ModelService>,
    pub benchmarks: Arc<BenchmarkService>,
    pub system: Arc<SystemService>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(system::health))
        .route("/system", get(system::snapshot))
        .route("/models", get(models::list))
        .route("/models/status", get(models::status))
        .route("/models/load", post(models::load))
        .route("/models/unload", post(models::unload))
        .route("/benchmarks", get(benchmarks::catalog))
        .route("/runs", get(runs::list).post(runs::start))
        .route("/runs/{id}", get(runs::get))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

#[cfg(test)]
mod tests;
