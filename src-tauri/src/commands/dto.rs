//! Command payloads. They keep the field names of the old HTTP API, so the UI's types are unchanged.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

use weight_check_core::domain::RunFilter;
use weight_check_core::error::AppError;
use weight_check_core::service::model_service::LoadOptions;

/// Decodes a raw command argument, reporting malformed payloads as `AppError::Validation`
/// (Tauri's own argument errors would reach the UI as bare strings).
pub fn parse<T: DeserializeOwned>(value: Value) -> Result<T, AppError> {
    serde_json::from_value(value).map_err(|e| AppError::Validation(format!("invalid request: {e}")))
}

/// Parses a run id the way the old `/runs/{id}` path did.
pub fn parse_run_id(id: &str) -> Result<Uuid, AppError> {
    id.parse()
        .map_err(|_| AppError::Validation(format!("invalid run id '{id}'")))
}

/// Parses every id of `compare_runs`, failing on the first malformed one.
pub fn parse_run_ids(ids: &[String]) -> Result<Vec<Uuid>, AppError> {
    ids.iter().map(|id| parse_run_id(id)).collect()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoadModelRequest {
    pub model_id: Uuid,
    pub ctx_size: Option<u32>,
    pub n_gpu_layers: Option<i32>,
    pub parallel: Option<u32>,
    pub threads: Option<i32>,
}

impl From<LoadModelRequest> for LoadOptions {
    fn from(r: LoadModelRequest) -> Self {
        Self {
            model_id: r.model_id,
            ctx_size: r.ctx_size,
            n_gpu_layers: r.n_gpu_layers,
            parallel: r.parallel,
            threads: r.threads,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartRunRequest {
    pub benchmark_id: String,
    pub config: Option<Value>,
}

/// Filters for `list_runs`. Every field is optional, so `{}` lists everything.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RunQuery {
    pub model_id: Option<Uuid>,
    pub category: Option<String>,
    pub benchmark: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl TryFrom<RunQuery> for RunFilter {
    type Error = AppError;

    fn try_from(q: RunQuery) -> Result<Self, Self::Error> {
        Ok(Self {
            model_id: q.model_id,
            category: q
                .category
                .map(|c| c.parse())
                .transpose()
                .map_err(AppError::Validation)?,
            benchmark_name: q.benchmark,
            status: q
                .status
                .map(|s| s.parse())
                .transpose()
                .map_err(AppError::Validation)?,
            limit: q.limit,
            offset: q.offset,
        })
    }
}

#[cfg(test)]
mod tests;
