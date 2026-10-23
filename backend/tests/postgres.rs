//! Repository integration tests. Skipped unless `TEST_DATABASE_URL` points at a
//! disposable PostgreSQL database (migrations are applied to it).

use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use weight_check::domain::{
    Category, Metric, NewHardwareProfile, NewModel, NewRun, RunFilter, RunStatus,
};
use weight_check::repository::postgres::PgStore;
use weight_check::repository::{
    HardwareRepository, HealthRepository, ModelRepository, RunRepository,
};

async fn store() -> Option<PgStore> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("connect to TEST_DATABASE_URL");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");
    Some(PgStore::new(pool))
}

#[tokio::test]
async fn run_lifecycle_round_trip() {
    let Some(store) = store().await else {
        eprintln!("TEST_DATABASE_URL not set; skipping");
        return;
    };

    let path = format!("/tmp/{}.gguf", Uuid::new_v4());
    let model = ModelRepository::upsert(
        &store,
        NewModel {
            name: "test".into(),
            file_path: path.clone(),
            architecture: Some("llama".into()),
            quantization: Some("Q4_K_M".into()),
            size_label: None,
            block_count: Some(32),
            file_size_bytes: 42,
        },
    )
    .await
    .unwrap();
    // Upserting the same path keeps the id.
    let again = ModelRepository::upsert(
        &store,
        NewModel {
            name: "renamed".into(),
            file_path: path,
            architecture: None,
            quantization: None,
            size_label: None,
            block_count: None,
            file_size_bytes: 43,
        },
    )
    .await
    .unwrap();
    assert_eq!(model.id, again.id);
    assert_eq!(again.name, "renamed");

    let hardware = HardwareRepository::upsert(
        &store,
        NewHardwareProfile {
            cpu: format!("cpu-{}", Uuid::new_v4()),
            gpu: Some("gpu".into()),
            vram_total_bytes: Some(1),
            ram_total_bytes: 2,
            backend: "vulkan".into(),
            driver_version: None,
        },
    )
    .await
    .unwrap();

    let run = store
        .create(NewRun {
            model_id: model.id,
            hardware_id: hardware.id,
            category: Category::InferencePerformance,
            benchmark_name: "inference.latency".into(),
            config: json!({ "n_predict": 8 }),
        })
        .await
        .unwrap();
    assert_eq!(run.status, RunStatus::Queued);

    store.mark_running(run.id).await.unwrap();
    store
        .complete(
            run.id,
            json!({ "ok": true }),
            &[
                Metric::new("ttft_ms_p95", 120.5, "ms"),
                Metric::new("itl_ms_median", 30.0, "ms"),
            ],
        )
        .await
        .unwrap();

    let stored = RunRepository::get(&store, run.id).await.unwrap().unwrap();
    assert_eq!(stored.status, RunStatus::Completed);
    assert!(stored.started_at.is_some() && stored.finished_at.is_some());
    assert_eq!(stored.details, Some(json!({ "ok": true })));
    assert_eq!(store.metrics(run.id).await.unwrap().len(), 2);

    let listed = store
        .list(&RunFilter {
            model_id: Some(model.id),
            category: Some(Category::InferencePerformance),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    let none = store
        .list(&RunFilter {
            model_id: Some(model.id),
            category: Some(Category::Agentic),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(none.is_empty());
}

async fn model_and_hardware(store: &PgStore) -> (Uuid, Uuid) {
    let model = ModelRepository::upsert(
        store,
        NewModel {
            name: "fixture".into(),
            file_path: format!("/tmp/{}.gguf", Uuid::new_v4()),
            architecture: None,
            quantization: None,
            size_label: None,
            block_count: None,
            file_size_bytes: 1,
        },
    )
    .await
    .unwrap();
    let hardware = HardwareRepository::upsert(store, hardware_profile(&Uuid::new_v4().to_string()))
        .await
        .unwrap();
    (model.id, hardware.id)
}

fn hardware_profile(cpu: &str) -> NewHardwareProfile {
    NewHardwareProfile {
        cpu: cpu.into(),
        gpu: None,
        vram_total_bytes: None,
        ram_total_bytes: 1,
        backend: "cpu".into(),
        driver_version: Some("v1".into()),
    }
}

fn new_run(model_id: Uuid, hardware_id: Uuid, category: Category, name: &str) -> NewRun {
    NewRun {
        model_id,
        hardware_id,
        category,
        benchmark_name: name.into(),
        config: json!({}),
    }
}

#[tokio::test]
async fn hardware_upsert_is_keyed_by_fingerprint() {
    let Some(store) = store().await else {
        return;
    };
    let cpu = format!("cpu-{}", Uuid::new_v4());
    let first = HardwareRepository::upsert(&store, hardware_profile(&cpu))
        .await
        .unwrap();
    let mut updated = hardware_profile(&cpu);
    updated.driver_version = Some("v2".into());
    let again = HardwareRepository::upsert(&store, updated).await.unwrap();
    assert_eq!(first.id, again.id);
    assert_eq!(again.driver_version.as_deref(), Some("v2"));
    assert_eq!(again.fingerprint, format!("{cpu}|-|0|1|cpu"));
}

#[tokio::test]
async fn failed_runs_keep_their_error() {
    let Some(store) = store().await else {
        return;
    };
    let (model_id, hardware_id) = model_and_hardware(&store).await;
    let run = store
        .create(new_run(
            model_id,
            hardware_id,
            Category::ResourceEfficiency,
            "resource.profile",
        ))
        .await
        .unwrap();
    store.mark_running(run.id).await.unwrap();
    store
        .fail(run.id, "llama-server error: boom")
        .await
        .unwrap();

    let stored = RunRepository::get(&store, run.id).await.unwrap().unwrap();
    assert_eq!(stored.status, RunStatus::Failed);
    assert_eq!(stored.category, Category::ResourceEfficiency);
    assert_eq!(stored.error.as_deref(), Some("llama-server error: boom"));
    assert!(stored.finished_at.is_some());
    assert!(store.metrics(run.id).await.unwrap().is_empty());

    // Completing with no metrics skips the metrics insert.
    store.complete(run.id, json!({}), &[]).await.unwrap();
    assert!(store.metrics(run.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn list_filters_by_name_and_limit() {
    let Some(store) = store().await else {
        return;
    };
    let (model_id, hardware_id) = model_and_hardware(&store).await;
    for name in [
        "inference.latency",
        "inference.latency",
        "inference.throughput",
    ] {
        store
            .create(new_run(
                model_id,
                hardware_id,
                Category::InferencePerformance,
                name,
            ))
            .await
            .unwrap();
    }
    let by_model = |benchmark_name: Option<&str>, limit| RunFilter {
        model_id: Some(model_id),
        benchmark_name: benchmark_name.map(Into::into),
        limit,
        ..Default::default()
    };

    assert_eq!(store.list(&by_model(None, None)).await.unwrap().len(), 3);
    let latency = store
        .list(&by_model(Some("inference.latency"), None))
        .await
        .unwrap();
    assert_eq!(latency.len(), 2);
    assert!(
        latency
            .iter()
            .all(|r| r.benchmark_name == "inference.latency")
    );
    // Newest first.
    assert!(latency[0].created_at >= latency[1].created_at);
    assert_eq!(store.list(&by_model(None, Some(1))).await.unwrap().len(), 1);
}

#[tokio::test]
async fn lookups_of_unknown_ids_return_none_and_ping_works() {
    let Some(store) = store().await else {
        return;
    };
    assert!(
        ModelRepository::get(&store, Uuid::new_v4())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        RunRepository::get(&store, Uuid::new_v4())
            .await
            .unwrap()
            .is_none()
    );
    assert!(store.metrics(Uuid::new_v4()).await.unwrap().is_empty());
    store.ping().await.unwrap();
}
