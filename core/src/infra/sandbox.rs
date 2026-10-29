//! Runs model-written programs for the coding benchmark.
//!
//! This is isolation for repeatability, not security: each program gets a fresh temp dir, an
//! empty environment, closed-off Python site packages (`-I`) and a timeout, but it runs as the
//! current user. Only run it against models you trust to write ordinary code.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use crate::error::{AppError, AppResult};

/// Captured output is cut off after this many bytes per stream.
const MAX_OUTPUT_BYTES: u64 = 1 << 20;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecResult {
    /// The program exited with status 0 before the timeout.
    pub success: bool,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
}

#[async_trait]
pub trait CodeRunner: Send + Sync {
    /// Runs `program` with `stdin` as its input, killing it after `timeout`.
    async fn run(&self, program: &str, stdin: &str, timeout: Duration) -> AppResult<ExecResult>;
}

/// Runs programs with a Python 3 interpreter.
pub struct PythonRunner {
    bin: PathBuf,
}

impl PythonRunner {
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self { bin: bin.into() }
    }
}

fn runner_err(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(format!("running generated code failed: {e}"))
}

async fn read_capped(stream: Option<impl AsyncRead + Unpin>) -> String {
    let mut buf = Vec::new();
    if let Some(s) = stream {
        let _ = s.take(MAX_OUTPUT_BYTES).read_to_end(&mut buf).await;
    }
    String::from_utf8_lossy(&buf).into_owned()
}

#[async_trait]
impl CodeRunner for PythonRunner {
    async fn run(&self, program: &str, stdin: &str, timeout: Duration) -> AppResult<ExecResult> {
        let dir = tempfile::tempdir().map_err(runner_err)?;
        let script = dir.path().join("main.py");
        tokio::fs::write(&script, program)
            .await
            .map_err(runner_err)?;

        let mut child = Command::new(&self.bin)
            .arg("-I")
            .arg(&script)
            .current_dir(dir.path())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("PYTHONHASHSEED", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| runner_err(format!("{}: {e}", self.bin.display())))?;

        let input = stdin.to_owned();
        let mut pipe = child.stdin.take();
        let feed = async move {
            if let Some(p) = pipe.as_mut() {
                // A program that exits without reading its input closes the pipe; that's fine.
                let _ = p.write_all(input.as_bytes()).await;
            }
            drop(pipe);
        };
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let finished = async {
            let ((), out, err, status) =
                tokio::join!(feed, read_capped(stdout), read_capped(stderr), child.wait());
            (out, err, status)
        };

        match tokio::time::timeout(timeout, finished).await {
            Ok((stdout, stderr, status)) => Ok(ExecResult {
                success: status.map_err(runner_err)?.success(),
                timed_out: false,
                stdout,
                stderr,
            }),
            // Dropping the future drops the child, which kills it (`kill_on_drop`).
            Err(_) => Ok(ExecResult {
                timed_out: true,
                ..ExecResult::default()
            }),
        }
    }
}

#[cfg(test)]
mod tests;
