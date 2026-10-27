//! Data-storage layer. Services depend only on these traits.

pub mod sqlite;

use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use crate::domain::{
    HardwareProfile, Metric, ModelInfo, NewHardwareProfile, NewModel, NewRun, Run, RunFilter,
};
use crate::error::AppResult;

#[async_trait]
pub trait ModelRepository: Send + Sync {
    /// Insert or update a model keyed by its file path.
    async fn upsert(&self, model: NewModel) -> AppResult<ModelInfo>;
    async fn get(&self, id: Uuid) -> AppResult<Option<ModelInfo>>;
}

#[async_trait]
pub trait HardwareRepository: Send + Sync {
    /// Insert or return the existing profile with the same fingerprint.
    async fn upsert(&self, profile: NewHardwareProfile) -> AppResult<HardwareProfile>;
}

#[async_trait]
pub trait RunRepository: Send + Sync {
    async fn create(&self, run: NewRun) -> AppResult<Run>;
    async fn mark_running(&self, id: Uuid) -> AppResult<()>;
    /// Store the outcome and its metrics atomically.
    async fn complete(&self, id: Uuid, details: Value, metrics: &[Metric]) -> AppResult<()>;
    async fn fail(&self, id: Uuid, error: &str) -> AppResult<()>;
    async fn get(&self, id: Uuid) -> AppResult<Option<Run>>;
    async fn list(&self, filter: &RunFilter) -> AppResult<Vec<Run>>;
    async fn metrics(&self, run_id: Uuid) -> AppResult<Vec<Metric>>;
}

#[async_trait]
pub trait HealthRepository: Send + Sync {
    async fn ping(&self) -> AppResult<()>;
}
