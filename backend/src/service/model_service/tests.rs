use super::*;
use crate::infra::gguf::tests::gguf_bytes;
use crate::service::testing::{FakeGpu, FakeLauncher, InMemoryModels};

async fn service_with_model(idle: Option<Duration>) -> (Arc<ModelService>, Uuid, FakeLauncher) {
    let file = tempfile::NamedTempFile::new().unwrap();
    let (file, path) = file.keep().unwrap();
    drop(file);
    let models = Arc::new(InMemoryModels::default());
    let model = models
        .upsert(NewModel {
            name: "tiny".into(),
            file_path: path.to_string_lossy().into_owned(),
            architecture: None,
            quantization: None,
            size_label: None,
            block_count: Some(8),
            file_size_bytes: 1,
        })
        .await
        .unwrap();
    let launcher = FakeLauncher::default();
    let service = Arc::new(ModelService::new(
        models,
        Arc::new(launcher.clone()),
        None,
        PathBuf::from("."),
        idle,
    ));
    (service, model.id, launcher)
}

#[tokio::test]
async fn load_and_unload() {
    let (service, model_id, launcher) = service_with_model(None).await;
    let active = service
        .load(LoadOptions {
            model_id,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(active.settings.ctx_size, DEFAULT_CTX_SIZE);
    assert!(service.status().loaded.is_some());

    service.unload().await.unwrap();
    assert!(service.status().loaded.is_none());
    assert_eq!(launcher.stopped(), 1);
    assert!(matches!(
        service.unload().await,
        Err(AppError::NoModelLoaded)
    ));
}

#[tokio::test]
async fn rejects_invalid_settings() {
    let (service, model_id, _) = service_with_model(None).await;
    let err = service
        .load(LoadOptions {
            model_id,
            parallel: Some(0),
            ..Default::default()
        })
        .await
        .err()
        .unwrap();
    assert!(matches!(err, AppError::Validation(_)));
}

#[tokio::test]
async fn unload_is_refused_while_checked_out() {
    let (service, model_id, _) = service_with_model(None).await;
    service
        .load(LoadOptions {
            model_id,
            ..Default::default()
        })
        .await
        .unwrap();
    let checkout = service.checkout().unwrap();
    assert!(matches!(service.unload().await, Err(AppError::Busy(_))));
    drop(checkout);
    service.unload().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn idle_watchdog_unloads_model() {
    let (service, model_id, launcher) = service_with_model(Some(Duration::from_secs(30))).await;
    service
        .load(LoadOptions {
            model_id,
            ..Default::default()
        })
        .await
        .unwrap();
    service.spawn_idle_watchdog();

    tokio::time::sleep(Duration::from_secs(20)).await;
    assert!(service.status().loaded.is_some(), "still within timeout");

    tokio::time::sleep(Duration::from_secs(20)).await;
    assert!(service.status().loaded.is_none(), "unloaded after timeout");
    assert_eq!(launcher.stopped(), 1);
}

#[tokio::test(start_paused = true)]
async fn idle_watchdog_keeps_model_while_in_use() {
    let (service, model_id, _) = service_with_model(Some(Duration::from_secs(30))).await;
    service
        .load(LoadOptions {
            model_id,
            ..Default::default()
        })
        .await
        .unwrap();
    service.spawn_idle_watchdog();

    let checkout = service.checkout().unwrap();
    tokio::time::sleep(Duration::from_secs(120)).await;
    assert!(service.status().loaded.is_some());
    drop(checkout);
}

fn service_for_dir(dir: &Path) -> ModelService {
    ModelService::new(
        Arc::new(InMemoryModels::default()),
        Arc::new(FakeLauncher::default()),
        Some(Arc::new(FakeGpu)),
        dir.to_path_buf(),
        None,
    )
}

#[tokio::test]
async fn list_available_registers_readable_gguf_files_only() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("b-model.gguf"), gguf_bytes("qwen3", 28, 7)).unwrap();
    std::fs::write(dir.path().join("A-Model.GGUF"), gguf_bytes("llama", 16, 15)).unwrap();
    std::fs::write(dir.path().join("mmproj-b.gguf"), gguf_bytes("clip", 1, 1)).unwrap();
    std::fs::write(dir.path().join("broken.gguf"), b"not a gguf").unwrap();
    std::fs::write(dir.path().join("notes.txt"), b"hi").unwrap();
    std::fs::create_dir(dir.path().join("sub.gguf")).unwrap();

    let models = service_for_dir(dir.path()).list_available().await.unwrap();
    let names: Vec<_> = models.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["A-Model", "b-model"]);
    let b = &models[1];
    assert_eq!(b.architecture.as_deref(), Some("qwen3"));
    assert_eq!(b.quantization.as_deref(), Some("Q8_0"));
    assert_eq!(b.size_label.as_deref(), Some("1B"));
    assert_eq!(b.block_count, Some(28));
    assert!(b.file_size_bytes > 0);
}

#[tokio::test]
async fn list_available_reports_missing_dir() {
    let dir = tempfile::tempdir().unwrap();
    let service = service_for_dir(&dir.path().join("missing"));
    assert!(matches!(
        service.list_available().await,
        Err(AppError::Internal(_))
    ));
}

#[tokio::test]
async fn load_reports_unknown_model_and_missing_file() {
    let (service, model_id, _) = service_with_model(None).await;
    let unknown = service
        .load(LoadOptions {
            model_id: Uuid::new_v4(),
            ..Default::default()
        })
        .await;
    assert!(matches!(unknown, Err(AppError::NotFound(_))));

    let path = service
        .models
        .get(model_id)
        .await
        .unwrap()
        .unwrap()
        .file_path;
    std::fs::remove_file(path).unwrap();
    let missing = service
        .load(LoadOptions {
            model_id,
            ..Default::default()
        })
        .await;
    assert!(matches!(missing, Err(AppError::NotFound(m)) if m.contains("model file")));
}

#[test]
fn validates_ctx_size_and_gpu_layers() {
    let ok = LoadSettings {
        ctx_size: 128,
        n_gpu_layers: 0,
        parallel: 64,
        threads: None,
    };
    validate_settings(&ok).unwrap();
    for bad in [
        LoadSettings {
            ctx_size: 127,
            ..ok.clone()
        },
        LoadSettings {
            n_gpu_layers: -1,
            ..ok.clone()
        },
        LoadSettings {
            parallel: 65,
            ..ok.clone()
        },
    ] {
        assert!(matches!(
            validate_settings(&bad),
            Err(AppError::Validation(_))
        ));
    }
}

#[tokio::test]
async fn loading_again_replaces_the_previous_server() {
    let (service, model_id, launcher) = service_with_model(None).await;
    let opts = LoadOptions {
        model_id,
        ctx_size: Some(2048),
        n_gpu_layers: Some(10),
        parallel: Some(2),
        threads: Some(4),
    };
    service.load(opts.clone()).await.unwrap();
    let active = service.load(opts).await.unwrap();
    assert_eq!(launcher.stopped(), 1);
    assert_eq!(
        active.settings,
        LoadSettings {
            ctx_size: 2048,
            n_gpu_layers: 10,
            parallel: 2,
            threads: Some(4),
        }
    );
}

#[tokio::test]
async fn status_reports_activity_and_shutdown_stops_server() {
    let (service, model_id, launcher) = service_with_model(Some(Duration::from_secs(60))).await;
    let status = service.status();
    assert!(status.loaded.is_none());
    assert_eq!(status.idle_timeout_secs, Some(60));

    service
        .load(LoadOptions {
            model_id,
            ..Default::default()
        })
        .await
        .unwrap();
    let checkout = service.checkout().unwrap();
    let status = service.status();
    assert_eq!(status.active_runs, 1);
    assert_eq!(status.loading_model_id, None);
    drop(checkout);
    assert_eq!(service.status().active_runs, 0);

    service.shutdown().await;
    assert!(service.status().loaded.is_none());
    assert_eq!(launcher.stopped(), 1);
    // Shutting down with nothing loaded is a no-op.
    service.shutdown().await;
    assert_eq!(launcher.stopped(), 1);
}

#[tokio::test]
async fn load_records_vram_around_load() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let models = Arc::new(InMemoryModels::default());
    let model = models
        .upsert(NewModel {
            name: "m".into(),
            file_path: file.path().to_string_lossy().into_owned(),
            architecture: None,
            quantization: None,
            size_label: None,
            block_count: None,
            file_size_bytes: 1,
        })
        .await
        .unwrap();
    let service = ModelService::new(
        models,
        Arc::new(FakeLauncher::default()),
        Some(Arc::new(FakeGpu)),
        PathBuf::from("."),
        None,
    );
    let active = service
        .load(LoadOptions {
            model_id: model.id,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(active.vram_before_load_bytes, Some(2 << 30));
    assert_eq!(active.vram_after_load_bytes, Some(2 << 30));
}
