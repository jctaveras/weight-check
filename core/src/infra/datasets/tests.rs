use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::extract::State;
use axum::routing::get;
use parquet::data_type::{ByteArray, ByteArrayType, Int64Type};
use parquet::file::properties::WriterProperties;
use parquet::file::writer::SerializedFileWriter;
use parquet::schema::parser::parse_message_type;
use serde_json::json;

use super::*;

const BODY: &[u8] = b"{\"a\":1}\n\n{\"a\":2}\n";

fn file(sha256: &'static str) -> DatasetFile {
    DatasetFile {
        repo: "org/data",
        revision: "abc123",
        path: "sub/test.jsonl",
        sha256,
    }
}

async fn serve(hits: Arc<AtomicUsize>) -> String {
    let router = Router::new()
        .route(
            "/datasets/org/data/resolve/abc123/sub/test.jsonl",
            get(|State(hits): State<Arc<AtomicUsize>>| async move {
                hits.fetch_add(1, Ordering::SeqCst);
                BODY
            }),
        )
        .with_state(hits);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("http://{addr}")
}

/// The sha256 of `BODY` (leaked: `DatasetFile` holds `&'static str`s).
fn body_sha() -> &'static str {
    Box::leak(hex(&Sha256::digest(BODY)).into_boxed_str())
}

#[tokio::test]
async fn fetch_downloads_verifies_and_caches() {
    let dir = tempfile::tempdir().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let store = HttpDatasetStore::with_base_url(dir.path(), serve(hits.clone()).await);
    let f = file(body_sha());

    let path = store.fetch(&f).await.unwrap();
    assert_eq!(path, dir.path().join("org/data/abc123/sub/test.jsonl"));
    assert_eq!(std::fs::read(&path).unwrap(), BODY);

    // The second fetch is served from the cache.
    assert_eq!(store.fetch(&f).await.unwrap(), path);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn fetch_rejects_a_hash_mismatch_and_caches_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store =
        HttpDatasetStore::with_base_url(dir.path(), serve(Arc::new(AtomicUsize::new(0))).await);
    let f = file("00");

    let err = store.fetch(&f).await.unwrap_err();
    assert!(
        matches!(err, AppError::Validation(ref m) if m.contains("expected 00")),
        "{err}"
    );
    assert!(!f.cache_path(dir.path()).exists());
    // No temp files are left behind either.
    let parent = f.cache_path(dir.path()).parent().unwrap().to_path_buf();
    assert_eq!(std::fs::read_dir(parent).unwrap().count(), 0);
}

#[tokio::test]
async fn fetch_reports_http_errors() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve(Arc::new(AtomicUsize::new(0))).await;
    let store = HttpDatasetStore::with_base_url(dir.path(), base);
    let missing = DatasetFile {
        path: "nope.jsonl",
        ..file("00")
    };
    let err = store.fetch(&missing).await.unwrap_err();
    assert!(
        matches!(err, AppError::Internal(ref m) if m.contains("404") && m.contains("org/data/nope.jsonl")),
        "{err}"
    );
}

#[test]
fn read_jsonl_skips_blank_lines_and_reports_bad_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rows.jsonl");
    std::fs::write(&path, BODY).unwrap();
    let rows: Vec<Value> = read_jsonl(&path).unwrap();
    assert_eq!(rows, [json!({"a": 1}), json!({"a": 2})]);

    std::fs::write(&path, "{nope\n").unwrap();
    assert!(matches!(
        read_jsonl::<Value>(&path),
        Err(AppError::Internal(_))
    ));
    assert!(read_jsonl::<Value>(&dir.path().join("missing.jsonl")).is_err());
}

#[test]
fn read_parquet_returns_rows_as_json_objects() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rows.parquet");
    let schema = Arc::new(
        parse_message_type(
            "message schema { REQUIRED BYTE_ARRAY question (UTF8); REQUIRED INT64 answer; }",
        )
        .unwrap(),
    );
    let mut writer = SerializedFileWriter::new(
        File::create(&path).unwrap(),
        schema,
        Arc::new(WriterProperties::builder().build()),
    )
    .unwrap();
    let mut group = writer.next_row_group().unwrap();
    let mut col = group.next_column().unwrap().unwrap();
    col.typed::<ByteArrayType>()
        .write_batch(
            &[ByteArray::from("2+2?"), ByteArray::from("3+3?")],
            None,
            None,
        )
        .unwrap();
    col.close().unwrap();
    let mut col = group.next_column().unwrap().unwrap();
    col.typed::<Int64Type>()
        .write_batch(&[4, 6], None, None)
        .unwrap();
    col.close().unwrap();
    group.close().unwrap();
    writer.close().unwrap();

    assert_eq!(
        read_parquet(&path).unwrap(),
        [
            json!({"question": "2+2?", "answer": 4}),
            json!({"question": "3+3?", "answer": 6}),
        ]
    );
    std::fs::write(&path, b"not parquet").unwrap();
    assert!(read_parquet(&path).is_err());
}
