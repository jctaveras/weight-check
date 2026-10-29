use std::time::Duration;

use super::*;
use crate::service::testing::{FakeDatasetStore, FakeSession};

#[test]
fn sample_is_deterministic_in_dataset_order_and_capped() {
    let items: Vec<u32> = (0..100).collect();
    let a = sample(items.clone(), 10, 7);
    assert_eq!(a, sample(items.clone(), 10, 7));
    assert_eq!(a.len(), 10);
    assert!(a.windows(2).all(|w| w[0] < w[1]), "{a:?}");
    assert_ne!(a, sample(items.clone(), 10, 8));
    assert_ne!(a, (0..10).collect::<Vec<_>>());
    // A limit past the end takes everything.
    assert_eq!(sample(items.clone(), 1000, 7), items);
}

#[test]
fn sample_picks_are_stable_for_a_fixed_seed() {
    // Pins the PRNG: changing it would silently change every benchmark's item set.
    assert_eq!(sample((0..20).collect::<Vec<u32>>(), 4, 0), [4, 6, 10, 14]);
}

#[test]
fn stratified_sample_spreads_picks_across_groups() {
    let items: Vec<(char, u32)> = (0..30)
        .map(|i| (['a', 'b', 'c'][i % 3], i as u32))
        .collect();
    let picked = sample_stratified(items, 6, 1, |(g, _)| g.to_string());
    for g in ['a', 'b', 'c'] {
        assert_eq!(
            picked.iter().filter(|(x, _)| *x == g).count(),
            2,
            "{picked:?}"
        );
    }
    // Uneven groups: small ones run out and the rest fill the limit.
    let uneven: Vec<(char, u32)> = (0..10)
        .map(|i| (if i == 0 { 'x' } else { 'y' }, i))
        .collect();
    let picked = sample_stratified(uneven, 5, 1, |(g, _)| g.to_string());
    assert_eq!(picked.len(), 5);
    assert_eq!(picked[0], ('x', 0));
}

#[tokio::test(start_paused = true)]
async fn eval_all_keeps_input_order_and_bounds_concurrency() {
    let in_flight = std::sync::atomic::AtomicUsize::new(0);
    let peak = std::sync::atomic::AtomicUsize::new(0);
    let out = eval_all((0..8u64).collect(), 3, |i| {
        let (in_flight, peak) = (&in_flight, &peak);
        async move {
            use std::sync::atomic::Ordering::SeqCst;
            let now = in_flight.fetch_add(1, SeqCst) + 1;
            peak.fetch_max(now, SeqCst);
            // Later items finish first, yet results stay in input order.
            tokio::time::sleep(Duration::from_millis(100 - i * 10)).await;
            in_flight.fetch_sub(1, SeqCst);
            Ok(i * 2)
        }
    })
    .await
    .unwrap();
    assert_eq!(out, [0, 2, 4, 6, 8, 10, 12, 14]);
    assert_eq!(peak.load(std::sync::atomic::Ordering::SeqCst), 3);

    let err = eval_all(vec![1, 2], 1, |i| async move {
        if i == 2 {
            Err(AppError::Llama("boom".into()))
        } else {
            Ok(i)
        }
    })
    .await
    .unwrap_err();
    assert!(matches!(err, AppError::Llama(_)));
}

#[test]
fn effective_concurrency_is_capped_by_server_slots() {
    let ctx = FakeSession::loaded().with_parallel(4).context();
    assert_eq!(effective_concurrency(&ctx, 2), 2);
    assert_eq!(effective_concurrency(&ctx, 16), 4);
}

#[test]
fn chat_request_is_greedy_and_can_disable_thinking() {
    let req = chat_request(vec![ChatMessage::user("hi")], 64, 3, Thinking::Auto);
    assert_eq!((req.temperature, req.seed, req.max_tokens), (0.0, 3, 64));
    assert!(req.chat_template_kwargs.is_none());
    let req = chat_request(vec![], 64, 3, Thinking::Off);
    assert_eq!(
        req.chat_template_kwargs,
        Some(json!({"enable_thinking": false}))
    );
}

#[test]
fn validate_common_rejects_zeroes() {
    assert!(validate_common(1, 1, Some(1)).is_ok());
    assert!(validate_common(1, 1, None).is_ok());
    for (l, c, m) in [(0, 1, None), (1, 0, None), (1, 1, Some(0))] {
        assert!(matches!(
            validate_common(l, c, m),
            Err(AppError::Validation(_))
        ));
    }
}

#[test]
fn helpers_format_percentages_tallies_and_previews() {
    assert_eq!(pct(1, 4), 25.0);
    assert_eq!(pct(0, 0), 0.0);
    let mut t = Tally::default();
    t.add(true);
    t.add(false);
    assert_eq!(
        t.to_json(),
        json!({"correct": 1, "total": 2, "accuracy": 50.0})
    );
    assert_eq!(truncate("héllo", 2), "hé…");
    assert_eq!(truncate("hi", 2), "hi");
}

#[tokio::test]
async fn load_rows_reads_fixtures_and_reports_bad_rows() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Row {
        a: u32,
    }
    let store = FakeDatasetStore::default();
    let file = sources::GSM8K_TEST;
    store.put(&file, &[json!({"a": 1}), json!({"a": 2})]);
    let rows: Vec<Row> = load_rows(&store, &file).await.unwrap();
    assert_eq!(rows, [Row { a: 1 }, Row { a: 2 }]);

    store.put(&file, &[json!({"b": 1})]);
    assert!(load_rows::<Row>(&store, &file).await.is_err());
    assert!(load_rows::<Row>(&store, &sources::IFEVAL).await.is_err());
}

#[test]
fn dataset_details_lists_pinned_files() {
    let d = dataset_details(&[&sources::IFEVAL]);
    assert_eq!(d[0]["repo"], "google/IFEval");
    assert_eq!(d[0]["sha256"], sources::IFEVAL.sha256);
}
