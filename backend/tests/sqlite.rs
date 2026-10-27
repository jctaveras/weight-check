//! Repository integration tests against a fresh SQLite database in a temp dir
//! (migrations are applied to it by `SqliteStore::connect`).

use std::str::FromStr;

use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use tempfile::TempDir;
use uuid::Uuid;

use weight_check::domain::{
    Category, Metric, NewHardwareProfile, NewModel, NewRun, RunFilter, RunStatus,
};
use weight_check::error::AppError;
use weight_check::repository::sqlite::SqliteStore;
use weight_check::repository::{
    HardwareRepository, HealthRepository, ModelRepository, RunRepository,
};

/// A store over a new database file. Keep the `TempDir` alive for the test.
async fn store() -> (TempDir, SqliteStore) {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = SqliteStore::connect(&db_url(&dir))
        .await
        .expect("connect and migrate");
    (dir, store)
}

fn db_url(dir: &TempDir) -> String {
    format!("sqlite://{}", dir.path().join("test.db").display())
}

/// A second, raw connection to the same file, for checks below the repository API.
async fn raw_pool(dir: &TempDir) -> SqlitePool {
    let options = SqliteConnectOptions::from_str(&db_url(dir))
        .unwrap()
        .foreign_keys(true);
    SqlitePool::connect_with(options).await.unwrap()
}

#[tokio::test]
async fn run_lifecycle_round_trip() {
    let (_dir, store) = store().await;

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

async fn model_and_hardware(store: &SqliteStore) -> (Uuid, Uuid) {
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
    let (_dir, store) = store().await;
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
    let (_dir, store) = store().await;
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
    let (_dir, store) = store().await;
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
    let (_dir, store) = store().await;
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

#[tokio::test]
async fn connect_creates_the_file_and_reapplies_migrations_idempotently() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    assert!(!path.exists());
    let store = SqliteStore::connect(&db_url(&dir)).await.unwrap();
    assert!(path.exists());
    let (model_id, _) = model_and_hardware(&store).await;
    drop(store);

    // Reopening runs the migrator again without touching existing data.
    let reopened = SqliteStore::connect(&db_url(&dir)).await.unwrap();
    assert!(
        ModelRepository::get(&reopened, model_id)
            .await
            .unwrap()
            .is_some()
    );
    reopened.ping().await.unwrap();
}

#[tokio::test]
async fn connect_reports_bad_urls_as_database_errors() {
    let err = SqliteStore::connect("postgres://nope").await.err().unwrap();
    assert!(matches!(err, AppError::Database(_)), "{err}");
}

#[tokio::test]
async fn json_columns_round_trip_nested_values() {
    let (dir, store) = store().await;
    let (model_id, hardware_id) = model_and_hardware(&store).await;
    let config = json!({
        "benchmark": { "n_predict": 8, "prompts": ["a", "ñ €"], "ratio": 0.25 },
        "server": { "n_gpu_layers": 12, "flash_attn": true, "extra": null },
    });
    let details = json!({ "samples": [{ "ms": 1.5 }, { "ms": 2e-3 }], "empty": {} });

    let mut new = new_run(model_id, hardware_id, Category::Agentic, "agentic.x");
    new.config = config.clone();
    let run = store.create(new).await.unwrap();
    assert_eq!(run.config, config);
    assert_eq!(run.details, None);

    store.complete(run.id, details.clone(), &[]).await.unwrap();
    let stored = RunRepository::get(&store, run.id).await.unwrap().unwrap();
    assert_eq!(stored.config, config);
    assert_eq!(stored.details, Some(details));

    // Stored as valid JSON text, so SQLite's JSON functions can query it.
    let pool = raw_pool(&dir).await;
    let n: i64 = sqlx::query_scalar(
        "SELECT json_extract(config, '$.server.n_gpu_layers') FROM benchmark_runs WHERE id = ?1",
    )
    .bind(run.id.hyphenated())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 12);
}

#[tokio::test]
async fn check_constraints_reject_unknown_enum_values() {
    let (dir, store) = store().await;
    let (model_id, hardware_id) = model_and_hardware(&store).await;
    let pool = raw_pool(&dir).await;
    let insert = |category: &'static str, status: &'static str| {
        sqlx::query(
            "INSERT INTO benchmark_runs (id, model_id, hardware_id, category, benchmark_name, \
                                         status, created_at)
             VALUES (?1, ?2, ?3, ?4, 'x', ?5, '2026-01-01T00:00:00Z')",
        )
        .bind(Uuid::new_v4().hyphenated())
        .bind(model_id.hyphenated())
        .bind(hardware_id.hyphenated())
        .bind(category)
        .bind(status)
        .execute(&pool)
    };

    let bad_category = insert("not_a_category", "queued").await.unwrap_err();
    assert!(bad_category.to_string().contains("CHECK"), "{bad_category}");
    let bad_status = insert("agentic", "paused").await.unwrap_err();
    assert!(bad_status.to_string().contains("CHECK"), "{bad_status}");
    insert("agentic", "queued").await.unwrap();
}

#[tokio::test]
async fn foreign_keys_are_enforced_by_the_store() {
    let (_dir, store) = store().await;
    let err = store
        .create(new_run(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Category::InferencePerformance,
            "inference.latency",
        ))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("FOREIGN KEY"), "{err}");
}

#[tokio::test]
async fn deleting_a_run_cascades_to_its_metrics() {
    let (dir, store) = store().await;
    let (model_id, hardware_id) = model_and_hardware(&store).await;
    let run = store
        .create(new_run(
            model_id,
            hardware_id,
            Category::InferencePerformance,
            "inference.latency",
        ))
        .await
        .unwrap();
    store
        .complete(run.id, json!({}), &[Metric::new("tps", 50.0, "tok/s")])
        .await
        .unwrap();
    assert_eq!(store.metrics(run.id).await.unwrap().len(), 1);

    let pool = raw_pool(&dir).await;
    sqlx::query("DELETE FROM benchmark_runs WHERE id = ?1")
        .bind(run.id.hyphenated())
        .execute(&pool)
        .await
        .unwrap();
    assert!(store.metrics(run.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn complete_is_atomic_when_a_metric_insert_fails() {
    let (_dir, store) = store().await;
    let (model_id, hardware_id) = model_and_hardware(&store).await;
    let run = store
        .create(new_run(
            model_id,
            hardware_id,
            Category::InferencePerformance,
            "inference.latency",
        ))
        .await
        .unwrap();
    store.mark_running(run.id).await.unwrap();

    // Duplicate metric names violate UNIQUE (run_id, name), rolling back the update.
    let dup = [
        Metric::new("tps", 1.0, "tok/s"),
        Metric::new("tps", 2.0, "tok/s"),
    ];
    assert!(
        store
            .complete(run.id, json!({ "x": 1 }), &dup)
            .await
            .is_err()
    );

    let stored = RunRepository::get(&store, run.id).await.unwrap().unwrap();
    assert_eq!(stored.status, RunStatus::Running);
    assert_eq!(stored.details, None);
    assert!(stored.finished_at.is_none());
    assert!(store.metrics(run.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn timestamps_round_trip_and_order_runs_newest_first() {
    let (_dir, store) = store().await;
    let (model_id, hardware_id) = model_and_hardware(&store).await;
    let before = chrono::Utc::now();
    let mut ids = Vec::new();
    for _ in 0..5 {
        let run = store
            .create(new_run(model_id, hardware_id, Category::Agentic, "a"))
            .await
            .unwrap();
        ids.push(run.id);
    }
    let listed = store
        .list(&RunFilter {
            model_id: Some(model_id),
            ..Default::default()
        })
        .await
        .unwrap();
    let listed_ids: Vec<Uuid> = listed.iter().map(|r| r.id).collect();
    ids.reverse();
    assert_eq!(listed_ids, ids);
    assert!(listed.iter().all(|r| r.created_at >= before));
    assert!(
        listed
            .windows(2)
            .all(|w| w[0].created_at >= w[1].created_at)
    );
}
