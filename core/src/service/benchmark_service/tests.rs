use std::time::Duration;

use super::*;
use crate::domain::RunStatus;
use crate::service::testing::{FakeHost, FakeSession, InMemoryRuns};

fn service(session: FakeSession) -> (BenchmarkService, Arc<InMemoryRuns>) {
    let runs = Arc::new(InMemoryRuns::default());
    let service = BenchmarkService::new(
        Registry::default(),
        runs.clone(),
        Arc::new(session),
        None,
        Arc::new(FakeHost),
        Uuid::new_v4(),
    );
    (service, runs)
}

async fn wait_finished(service: &BenchmarkService, id: Uuid) -> RunWithMetrics {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let run = service.get_run(id).await.unwrap();
            if matches!(run.run.status, RunStatus::Completed | RunStatus::Failed) {
                return run;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("run finished")
}

#[test]
fn catalog_lists_all_categories() {
    let (service, _) = service(FakeSession::loaded());
    let catalog = service.catalog();
    assert_eq!(catalog.len(), 5);
    assert!(catalog.iter().all(|c| !c.benchmarks.is_empty()));
}

#[tokio::test]
async fn generation_speed_run_completes_with_metrics() {
    let (service, _) = service(FakeSession::loaded());
    let run = service
        .start_run(
            "inference.generation_speed",
            Some(json!({"repetitions": 2, "warmup": false})),
        )
        .await
        .unwrap();
    assert_eq!(run.status, RunStatus::Queued);

    let done = wait_finished(&service, run.id).await;
    assert_eq!(
        done.run.status,
        RunStatus::Completed,
        "{:?}",
        done.run.error
    );
    let tps = done
        .metrics
        .iter()
        .find(|m| m.name == "generation_tps_mean")
        .expect("tps metric");
    assert_eq!(tps.value, 50.0);
    assert_eq!(done.run.config["benchmark"]["repetitions"], 2);
    assert_eq!(done.run.config["benchmark"]["n_predict"], 256);
}

#[tokio::test]
async fn latency_run_measures_streaming() {
    let (service, _) = service(FakeSession::loaded());
    let run = service.start_run("inference.latency", None).await.unwrap();
    let done = wait_finished(&service, run.id).await;
    assert_eq!(
        done.run.status,
        RunStatus::Completed,
        "{:?}",
        done.run.error
    );
    assert!(done.metrics.iter().any(|m| m.name == "ttft_ms_p95"));
    assert!(done.metrics.iter().any(|m| m.name == "itl_ms_median"));
}

#[tokio::test]
async fn rejects_unknown_stub_and_invalid() {
    let (service, _) = service(FakeSession::loaded());
    assert!(matches!(
        service.start_run("nope", None).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        service.start_run("quality.reasoning", None).await,
        Err(AppError::NotImplemented(_))
    ));
    assert!(matches!(
        service
            .start_run("inference.latency", Some(json!({"bogus": 1})))
            .await,
        Err(AppError::Validation(_))
    ));
}

#[tokio::test]
async fn requires_loaded_model() {
    let (service, _) = service(FakeSession::empty());
    assert!(matches!(
        service.start_run("inference.latency", None).await,
        Err(AppError::NoModelLoaded)
    ));
}

#[tokio::test]
async fn only_one_run_at_a_time() {
    let (service, _) = service(FakeSession::loaded_slow());
    let first = service.start_run("inference.latency", None).await.unwrap();
    assert!(matches!(
        service.start_run("inference.latency", None).await,
        Err(AppError::Busy(_))
    ));
    wait_finished(&service, first.id).await;
    service.start_run("inference.latency", None).await.unwrap();
}

#[tokio::test]
async fn failed_benchmark_is_recorded() {
    let (service, _) = service(FakeSession::loaded_failing());
    let run = service
        .start_run("inference.generation_speed", None)
        .await
        .unwrap();
    let done = wait_finished(&service, run.id).await;
    assert_eq!(done.run.status, RunStatus::Failed);
    assert!(done.run.error.unwrap().contains("boom"));
    assert!(done.metrics.is_empty());
}

#[tokio::test]
async fn get_and_list_runs() {
    let (service, _) = service(FakeSession::loaded());
    assert!(matches!(
        service.get_run(Uuid::new_v4()).await,
        Err(AppError::NotFound(_))
    ));

    let run = service.start_run("inference.latency", None).await.unwrap();
    wait_finished(&service, run.id).await;
    let runs = service.list_runs(RunFilter::default()).await.unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, run.id);

    for limit in [0, 1001] {
        assert!(matches!(
            service
                .list_runs(RunFilter {
                    limit: Some(limit),
                    ..Default::default()
                })
                .await,
            Err(AppError::Validation(_))
        ));
    }
}

async fn seed_run(runs: &InMemoryRuns, benchmark: &str, completed: bool) -> Uuid {
    let run = runs
        .create(NewRun {
            model_id: Uuid::new_v4(),
            hardware_id: Uuid::new_v4(),
            category: crate::domain::Category::InferencePerformance,
            benchmark_name: benchmark.into(),
            config: json!({}),
        })
        .await
        .unwrap();
    if completed {
        let metrics = [Metric::new("generation_tps_mean", 42.0, "tok/s")];
        runs.complete(run.id, json!({}), &metrics).await.unwrap();
    }
    run.id
}

#[tokio::test]
async fn compare_runs_returns_metrics_in_request_order() {
    let (service, runs) = service(FakeSession::loaded());
    let a = seed_run(&runs, "inference.latency", true).await;
    let b = seed_run(&runs, "inference.latency", true).await;

    let compared = service.compare_runs(vec![b, a]).await.unwrap();
    let ids: Vec<Uuid> = compared.iter().map(|r| r.run.id).collect();
    assert_eq!(ids, [b, a]);
    assert!(compared.iter().all(|r| r.metrics.len() == 1));
}

#[tokio::test]
async fn compare_runs_validates_ids() {
    let (service, runs) = service(FakeSession::loaded());
    let a = seed_run(&runs, "inference.latency", true).await;

    let too_many = vec![a; MAX_COMPARE_RUNS + 1];
    for ids in [vec![], too_many, vec![a, a]] {
        assert!(matches!(
            service.compare_runs(ids).await,
            Err(AppError::Validation(_))
        ));
    }
    assert!(matches!(
        service.compare_runs(vec![a, Uuid::new_v4()]).await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn compare_runs_requires_completed_runs_of_one_benchmark() {
    let (service, runs) = service(FakeSession::loaded());
    let a = seed_run(&runs, "inference.latency", true).await;
    let other = seed_run(&runs, "inference.throughput", true).await;
    let queued = seed_run(&runs, "inference.latency", false).await;

    for ids in [vec![a, other], vec![a, queued]] {
        assert!(matches!(
            service.compare_runs(ids).await,
            Err(AppError::Validation(_))
        ));
    }
}
