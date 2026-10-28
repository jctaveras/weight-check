use serde_json::json;

use super::*;
use weight_check_core::domain::{Category, RunStatus};

#[test]
fn run_query_parses_category() {
    let filter: RunFilter = RunQuery {
        model_id: None,
        category: Some("agentic".into()),
        benchmark: Some("x".into()),
        status: Some("completed".into()),
        limit: Some(5),
        offset: Some(10),
    }
    .try_into()
    .unwrap();
    assert_eq!(filter.category, Some(Category::Agentic));
    assert_eq!(filter.benchmark_name.as_deref(), Some("x"));
    assert_eq!(filter.status, Some(RunStatus::Completed));
    assert_eq!(filter.limit, Some(5));
    assert_eq!(filter.offset, Some(10));
}

#[test]
fn run_query_rejects_unknown_category() {
    let err = RunFilter::try_from(RunQuery {
        model_id: None,
        category: Some("nope".into()),
        benchmark: None,
        status: None,
        limit: None,
        offset: None,
    })
    .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
}

#[test]
fn run_query_rejects_unknown_status() {
    let err = RunFilter::try_from(RunQuery {
        status: Some("done".into()),
        ..Default::default()
    })
    .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
}

#[test]
fn run_query_defaults_missing_fields_and_rejects_unknown_ones() {
    let empty: RunQuery = parse(json!({})).unwrap();
    assert!(empty.model_id.is_none() && empty.category.is_none());
    assert!(empty.benchmark.is_none() && empty.limit.is_none());

    let id = Uuid::new_v4();
    let full: RunQuery = parse(json!({
        "model_id": id, "category": "agentic", "benchmark": "b", "status": "failed", "limit": 3, "offset": 6
    }))
    .unwrap();
    assert_eq!(full.model_id, Some(id));
    assert_eq!(full.limit, Some(3));
    assert_eq!(full.status.as_deref(), Some("failed"));
    assert_eq!(full.offset, Some(6));

    for bad in [json!({"unknown": 1}), json!({"limit": "x"}), json!("x")] {
        let err = parse::<RunQuery>(bad.clone()).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "{bad}");
    }
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

#[test]
fn parse_reports_validation_errors() {
    let err = parse::<StartRunRequest>(json!({"benchmark": "x"})).unwrap_err();
    match err {
        AppError::Validation(msg) => assert!(msg.starts_with("invalid request: "), "{msg}"),
        other => panic!("expected validation, got {other:?}"),
    }
}

#[test]
fn parse_run_id_accepts_uuids_only() {
    let id = Uuid::new_v4();
    assert_eq!(parse_run_id(&id.to_string()).unwrap(), id);
    let err = parse_run_id("not-a-uuid").unwrap_err();
    assert_eq!(err.to_string(), "invalid run id 'not-a-uuid'");
}

#[test]
fn parse_run_ids_rejects_any_malformed_id() {
    let id = Uuid::new_v4();
    assert_eq!(parse_run_ids(&[id.to_string()]).unwrap(), [id]);
    let err = parse_run_ids(&[id.to_string(), "nope".into()]).unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
}
