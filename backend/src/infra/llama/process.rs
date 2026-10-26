use std::collections::VecDeque;
use std::env;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::time::Instant;

use super::client::LlamaHttpClient;
use super::{LaunchArgs, LlamaApi, RunningServer, ServerLauncher};
use crate::error::{AppError, AppResult};

const LOG_TAIL_LINES: usize = 20;
const HEALTH_POLL: Duration = Duration::from_millis(250);

/// Spawns `llama-server` as a child process bound to a free localhost port.
pub struct LlamaServerLauncher {
    bin: PathBuf,
    load_timeout: Duration,
}

impl LlamaServerLauncher {
    pub fn new(bin: PathBuf, load_timeout: Duration) -> Self {
        Self { bin, load_timeout }
    }
}

struct LlamaServer {
    child: Child,
    pid: Option<u32>,
    api: Arc<LlamaHttpClient>,
}

#[async_trait]
impl RunningServer for LlamaServer {
    fn api(&self) -> Arc<dyn LlamaApi> {
        self.api.clone()
    }

    fn pid(&self) -> Option<u32> {
        self.pid
    }

    async fn stop(mut self: Box<Self>) {
        if let Err(e) = self.child.kill().await {
            tracing::warn!("failed to stop llama-server: {e}");
        }
    }
}

#[async_trait]
impl ServerLauncher for LlamaServerLauncher {
    async fn launch(&self, args: &LaunchArgs) -> AppResult<Box<dyn RunningServer>> {
        let port = free_port()?;
        let s = &args.settings;

        let mut cmd = Command::new(&self.bin);
        cmd.arg("-m")
            .arg(&args.model_path)
            .args(["-c", &s.ctx_size.to_string()])
            .args(["-ngl", &s.n_gpu_layers.to_string()])
            .args(["-np", &s.parallel.to_string()])
            .args(["--host", "127.0.0.1", "--port", &port.to_string()]);
        if let Some(threads) = s.threads {
            cmd.args(["-t", &threads.to_string()]);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        tracing::info!(model = %args.model_path.display(), port, "spawning llama-server");
        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::Llama(format!("failed to spawn {}: {e}", self.bin.display())))?;

        let tail = Arc::new(Mutex::new(VecDeque::with_capacity(LOG_TAIL_LINES)));
        if let Some(out) = child.stdout.take() {
            tokio::spawn(forward_logs(out, tail.clone()));
        }
        if let Some(err) = child.stderr.take() {
            tokio::spawn(forward_logs(err, tail.clone()));
        }

        let api = Arc::new(LlamaHttpClient::new(format!("http://127.0.0.1:{port}")));
        let deadline = Instant::now() + self.load_timeout;
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                // Give the log readers a moment to drain the final lines.
                tokio::time::sleep(Duration::from_millis(100)).await;
                return Err(AppError::Llama(format!(
                    "llama-server exited during load ({status}):\n{}",
                    tail_text(&tail)
                )));
            }
            if api.health().await.unwrap_or(false) {
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill().await;
                return Err(AppError::Llama(format!(
                    "llama-server did not become healthy within {:?}:\n{}",
                    self.load_timeout,
                    tail_text(&tail)
                )));
            }
            tokio::time::sleep(HEALTH_POLL).await;
        }

        Ok(Box::new(LlamaServer {
            pid: child.id(),
            child,
            api,
        }))
    }
}

fn free_port() -> AppResult<u16> {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| AppError::Internal(format!("no free port: {e}")))
}

async fn forward_logs<R: AsyncRead + Unpin>(reader: R, tail: Arc<Mutex<VecDeque<String>>>) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        tracing::debug!(target: "llama_server", "{line}");
        let mut tail = tail.lock().expect("log tail mutex poisoned");
        if tail.len() == LOG_TAIL_LINES {
            tail.pop_front();
        }
        tail.push_back(line);
    }
}

fn tail_text(tail: &Mutex<VecDeque<String>>) -> String {
    let tail = tail.lock().expect("log tail mutex poisoned");
    tail.iter().cloned().collect::<Vec<_>>().join("\n")
}

/// Infers the ggml backend(s) llama-server was built with from the shared libraries
/// next to the binary, e.g. `libggml-vulkan.so` → "vulkan".
pub fn detect_backend(bin: &Path) -> String {
    let resolved = if bin.components().count() > 1 {
        Some(bin.to_path_buf())
    } else {
        env::var_os("PATH").and_then(|paths| {
            env::split_paths(&paths)
                .map(|p| p.join(bin))
                .find(|p| p.is_file())
        })
    };
    let Some(dir) = resolved.as_deref().and_then(Path::parent) else {
        return "unknown".to_string();
    };

    let backends: Vec<&str> = ["cuda", "hip", "vulkan", "metal", "sycl", "opencl"]
        .into_iter()
        .filter(|b| dir.join(format!("libggml-{b}.so")).exists())
        .collect();
    if backends.is_empty() {
        "cpu".to_string()
    } else {
        backends.join("+")
    }
}

#[cfg(test)]
mod tests;
