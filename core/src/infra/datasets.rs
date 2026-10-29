//! Evaluation datasets: pinned files downloaded from the Hugging Face Hub, verified and cached.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use parquet::file::reader::{FileReader, SerializedFileReader};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

const HF_BASE_URL: &str = "https://huggingface.co";

/// One file of a Hugging Face dataset, pinned to a commit and checked against its sha256.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatasetFile {
    pub repo: &'static str,
    pub revision: &'static str,
    pub path: &'static str,
    pub sha256: &'static str,
}

impl DatasetFile {
    /// Location inside the cache dir: `<repo>/<revision>/<path>`.
    pub fn cache_path(&self, cache_dir: &Path) -> PathBuf {
        cache_dir
            .join(self.repo)
            .join(self.revision)
            .join(self.path)
    }
}

#[async_trait]
pub trait DatasetStore: Send + Sync {
    /// Returns the local path of `file`, downloading it first if it is not cached.
    async fn fetch(&self, file: &DatasetFile) -> AppResult<PathBuf>;
}

/// Downloads dataset files over HTTP into a local cache directory.
pub struct HttpDatasetStore {
    http: reqwest::Client,
    base_url: String,
    cache_dir: PathBuf,
}

impl HttpDatasetStore {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self::with_base_url(cache_dir, HF_BASE_URL)
    }

    /// Like `new`, but fetching from another host (a stand-in server in tests).
    pub fn with_base_url(cache_dir: impl Into<PathBuf>, base_url: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .build()
            .expect("reqwest client");
        Self {
            http,
            base_url: base_url.into(),
            cache_dir: cache_dir.into(),
        }
    }

    fn url(&self, file: &DatasetFile) -> String {
        format!(
            "{}/datasets/{}/resolve/{}/{}",
            self.base_url, file.repo, file.revision, file.path
        )
    }
}

fn download_err(file: &DatasetFile, e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!(
        "downloading dataset {}/{} failed: {e}",
        file.repo, file.path
    ))
}

/// reqwest's top-level message ("error sending request") hides the cause; append the chain.
fn request_err(file: &DatasetFile, e: reqwest::Error) -> AppError {
    let mut msg = e.to_string();
    let mut source = std::error::Error::source(&e);
    while let Some(cause) = source {
        msg.push_str(&format!(": {cause}"));
        source = cause.source();
    }
    download_err(
        file,
        format!("{msg}. Check your internet connection and try again"),
    )
}

#[async_trait]
impl DatasetStore for HttpDatasetStore {
    async fn fetch(&self, file: &DatasetFile) -> AppResult<PathBuf> {
        let dest = file.cache_path(&self.cache_dir);
        if dest.is_file() {
            return Ok(dest);
        }
        let dir = dest.parent().expect("cache path has a parent");
        std::fs::create_dir_all(dir).map_err(|e| download_err(file, e))?;

        tracing::info!(repo = file.repo, path = file.path, "downloading dataset");
        let resp = self
            .http
            .get(self.url(file))
            .send()
            .await
            .map_err(|e| request_err(file, e))?;
        if !resp.status().is_success() {
            return Err(download_err(file, format!("HTTP {}", resp.status())));
        }

        // Stream into a temp file next to the destination, hashing as we go, so a partial or
        // corrupt download never lands at the cached path.
        let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(|e| download_err(file, e))?;
        let mut hasher = Sha256::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| request_err(file, e))?;
            hasher.update(&chunk);
            tmp.write_all(&chunk).map_err(|e| download_err(file, e))?;
        }
        let actual = hex(&hasher.finalize());
        if actual != file.sha256 {
            return Err(AppError::Validation(format!(
                "dataset {}/{} has sha256 {actual}, expected {}",
                file.repo, file.path, file.sha256
            )));
        }
        tmp.persist(&dest).map_err(|e| download_err(file, e))?;
        Ok(dest)
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn read_err(path: &Path, e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("reading dataset {}: {e}", path.display()))
}

/// Reads a JSON Lines file into `T`s, skipping blank lines.
pub fn read_jsonl<T: DeserializeOwned>(path: &Path) -> AppResult<Vec<T>> {
    let file = File::open(path).map_err(|e| read_err(path, e))?;
    let mut rows = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| read_err(path, e))?;
        if line.trim().is_empty() {
            continue;
        }
        rows.push(serde_json::from_str(&line).map_err(|e| read_err(path, e))?);
    }
    Ok(rows)
}

/// Reads every row of a Parquet file as a JSON object keyed by column name.
pub fn read_parquet(path: &Path) -> AppResult<Vec<Value>> {
    let file = File::open(path).map_err(|e| read_err(path, e))?;
    let reader = SerializedFileReader::new(file).map_err(|e| read_err(path, e))?;
    let rows = reader.get_row_iter(None).map_err(|e| read_err(path, e))?;
    rows.map(|row| {
        row.map(|r| r.to_json_value())
            .map_err(|e| read_err(path, e))
    })
    .collect()
}

#[cfg(test)]
mod tests;
