use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::domain::RunFilter;
use crate::error::AppError;
use crate::service::model_service::LoadOptions;

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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunQuery {
    pub model_id: Option<Uuid>,
    pub category: Option<String>,
    pub benchmark: Option<String>,
    pub limit: Option<i64>,
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
            limit: q.limit,
        })
    }
}

#[cfg(test)]
mod tests;
