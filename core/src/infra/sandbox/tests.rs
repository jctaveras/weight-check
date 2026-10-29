use super::*;

const TIMEOUT: Duration = Duration::from_secs(10);

/// The runner, or `None` (skipping the test) when python3 isn't installed.
fn python() -> Option<PythonRunner> {
    let found = std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    found.then(|| PythonRunner::new("python3"))
}

#[tokio::test]
async fn passing_program_reads_stdin_and_writes_stdout() {
    let Some(py) = python() else { return };
    let out = py
        .run(
            "print(sum(int(x) for x in input().split()))",
            "2 3\n",
            TIMEOUT,
        )
        .await
        .unwrap();
    assert!(out.success && !out.timed_out, "{out:?}");
    assert_eq!(out.stdout, "5\n");
}

#[tokio::test]
async fn failing_program_reports_stderr() {
    let Some(py) = python() else { return };
    let out = py.run("assert 1 == 2, 'nope'", "", TIMEOUT).await.unwrap();
    assert!(!out.success && !out.timed_out);
    assert!(
        out.stderr.contains("AssertionError: nope"),
        "{}",
        out.stderr
    );
}

#[tokio::test]
async fn program_runs_in_an_empty_temp_dir_with_a_clean_env() {
    let Some(py) = python() else { return };
    let program = "import os\nprint(sorted(os.listdir('.')), 'HOME' in os.environ)";
    let out = py.run(program, "", TIMEOUT).await.unwrap();
    assert_eq!(out.stdout, "['main.py'] False\n");
}

#[tokio::test]
async fn slow_program_times_out() {
    let Some(py) = python() else { return };
    let out = py
        .run(
            "import time\ntime.sleep(30)",
            "",
            Duration::from_millis(300),
        )
        .await
        .unwrap();
    assert!(out.timed_out && !out.success);
}

#[tokio::test]
async fn missing_interpreter_is_an_error() {
    let err = PythonRunner::new("/nonexistent/python")
        .run("print(1)", "", TIMEOUT)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Internal(ref m) if m.contains("/nonexistent/python")));
}
