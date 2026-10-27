use std::time::Duration;

use super::*;

fn config(models_dir: &Path) -> Config {
    Config {
        llama_server_bin: "llama-server".into(),
        models_dir: models_dir.to_path_buf(),
        idle_timeout: Some(Duration::from_secs(600)),
        load_timeout: Duration::from_secs(1),
    }
}

#[tokio::test]
async fn build_state_wires_working_services() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join(DB_FILE_NAME);
    let state = build_state(&config(dir.path()), &db).await.unwrap();
    assert!(db.is_file());

    let health = commands::health(&state).await.unwrap();
    assert_eq!(health["status"], "ok");
    assert!(commands::list_models(&state).await.unwrap().is_empty());
    let status = commands::model_status(&state);
    assert!(status.loaded.is_none());
    assert_eq!(status.idle_timeout_secs, Some(600));
    assert_eq!(commands::benchmark_catalog(&state).len(), 5);
    assert!(commands::list_runs(&state, None).await.unwrap().is_empty());

    // Nothing loaded: shutdown is a no-op and the state stays usable.
    shutdown(&state).await;
    let err = commands::unload_model(&state).await.unwrap_err();
    assert_eq!(err.code, "no_model_loaded");
}

#[tokio::test]
async fn build_state_reuses_an_existing_database() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join(DB_FILE_NAME);
    let first = build_state(&config(dir.path()), &db).await.unwrap();
    drop(first);
    let second = build_state(&config(dir.path()), &db).await.unwrap();
    commands::health(&second).await.unwrap();
}

#[tokio::test]
async fn build_state_reports_an_unusable_database_path() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("missing-dir").join(DB_FILE_NAME);
    let err = build_state(&config(dir.path()), &db).await.err().unwrap();
    assert!(matches!(err, AppError::Database(_)), "{err}");
}

#[tokio::test]
async fn init_creates_the_data_dir() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().join("nested").join("app");
    let state = init(&data_dir).await.unwrap();
    assert!(data_dir.join(DB_FILE_NAME).is_file());
    commands::health(&state).await.unwrap();
}
