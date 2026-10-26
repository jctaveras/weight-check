use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::domain::LoadSettings;

/// Writes an executable shell script standing in for `llama-server`.
fn fake_server(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("llama-server");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn args() -> LaunchArgs {
    LaunchArgs {
        model_path: PathBuf::from("/models/tiny.gguf"),
        settings: LoadSettings {
            ctx_size: 2048,
            n_gpu_layers: 10,
            parallel: 2,
            threads: Some(4),
        },
    }
}

async fn launch_err(bin: PathBuf, timeout: Duration) -> String {
    let launcher = LlamaServerLauncher::new(bin, timeout);
    // A concurrent fork in another test can briefly hold the script open for writing.
    for _ in 0..5 {
        match launcher.launch(&args()).await {
            Err(AppError::Llama(msg)) if msg.contains("Text file busy") => {
                tokio::time::sleep(Duration::from_millis(50)).await
            }
            Err(AppError::Llama(msg)) => return msg,
            Err(e) => panic!("unexpected error {e}"),
            Ok(_) => panic!("fake server should not become healthy"),
        }
    }
    panic!("script stayed busy");
}

#[tokio::test]
async fn missing_binary_fails_to_spawn() {
    let msg = launch_err("/nonexistent/llama-server".into(), Duration::from_secs(1)).await;
    assert!(msg.contains("failed to spawn"), "{msg}");
}

#[tokio::test]
async fn early_exit_reports_args_and_log_tail() {
    let dir = tempfile::tempdir().unwrap();
    let bin = fake_server(
        dir.path(),
        "echo \"args: $*\"\necho 'error: bad model' >&2\nexit 3",
    );
    let msg = launch_err(bin, Duration::from_secs(10)).await;
    assert!(msg.contains("exited during load"), "{msg}");
    assert!(msg.contains("error: bad model"), "{msg}");
    assert!(
        msg.contains("-m /models/tiny.gguf -c 2048 -ngl 10 -np 2 --host 127.0.0.1 --port"),
        "{msg}"
    );
    assert!(msg.contains("-t 4"), "{msg}");
}

#[tokio::test]
async fn unhealthy_server_times_out() {
    let dir = tempfile::tempdir().unwrap();
    let bin = fake_server(dir.path(), "echo starting\nexec sleep 30");
    let msg = launch_err(bin, Duration::from_millis(300)).await;
    assert!(msg.contains("did not become healthy"), "{msg}");
    assert!(msg.contains("starting"), "{msg}");
}

#[tokio::test]
async fn log_tail_is_capped() {
    let dir = tempfile::tempdir().unwrap();
    let bin = fake_server(
        dir.path(),
        "i=1; while [ $i -le 25 ]; do echo \"line $i\"; i=$((i+1)); done; exit 1",
    );
    let msg = launch_err(bin, Duration::from_secs(10)).await;
    let logged: Vec<_> = msg.lines().filter(|l| l.starts_with("line ")).collect();
    assert_eq!(logged.len(), LOG_TAIL_LINES, "{msg}");
    assert_eq!(logged.first(), Some(&"line 6"));
    assert_eq!(logged.last(), Some(&"line 25"));
}

#[test]
fn log_tail_keeps_the_last_lines() {
    let tail = Mutex::new(VecDeque::from(["a".to_string(), "b".to_string()]));
    assert_eq!(tail_text(&tail), "a\nb");
}

#[test]
fn detects_backends_from_libraries_next_to_binary() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("llama-server");
    assert_eq!(detect_backend(&bin), "cpu");

    std::fs::write(dir.path().join("libggml-vulkan.so"), b"").unwrap();
    std::fs::write(dir.path().join("libggml-hip.so"), b"").unwrap();
    assert_eq!(detect_backend(&bin), "hip+vulkan");

    assert_eq!(
        detect_backend(Path::new("weight-check-no-such-binary")),
        "unknown"
    );
}
