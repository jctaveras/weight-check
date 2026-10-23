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
mod tests {
    use serde_json::json;

    use super::*;
    use crate::domain::Category;

    #[test]
    fn run_query_parses_category() {
        let filter: RunFilter = RunQuery {
            model_id: None,
            category: Some("agentic".into()),
            benchmark: Some("x".into()),
            limit: Some(5),
        }
        .try_into()
        .unwrap();
        assert_eq!(filter.category, Some(Category::Agentic));
        assert_eq!(filter.benchmark_name.as_deref(), Some("x"));
        assert_eq!(filter.limit, Some(5));
    }

    #[test]
    fn run_query_rejects_unknown_category() {
        let err = RunFilter::try_from(RunQuery {
            model_id: None,
            category: Some("nope".into()),
            benchmark: None,
            limit: None,
        })
        .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn load_request_maps_to_options_and_rejects_unknown_fields() {
        let id = Uuid::new_v4();
        let req: LoadModelRequest = serde_json::from_value(json!({
            "model_id": id, "ctx_size": 2048, "n_gpu_layers": 10, "parallel": 2, "threads": 4
        }))
        .unwrap();
        let opts = LoadOptions::from(req);
        assert_eq!(opts.model_id, id);
        assert_eq!(opts.ctx_size, Some(2048));
        assert_eq!(opts.n_gpu_layers, Some(10));
        assert_eq!(opts.parallel, Some(2));
        assert_eq!(opts.threads, Some(4));

        let extra = json!({ "model_id": id, "gpu": true });
        assert!(serde_json::from_value::<LoadModelRequest>(extra).is_err());
    }
}
