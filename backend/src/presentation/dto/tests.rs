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
