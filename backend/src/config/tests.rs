use std::collections::HashMap;

use super::*;

fn config(vars: &[(&str, &str)]) -> Result<Config, String> {
    let vars: HashMap<String, String> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Config::from_lookup(|k| vars.get(k).cloned())
}

#[test]
fn applies_defaults() {
    let c = config(&[("DATABASE_URL", "postgres://x")]).unwrap();
    assert_eq!(c.database_url, "postgres://x");
    assert_eq!(c.bind_addr.to_string(), "127.0.0.1:8787");
    assert_eq!(c.llama_server_bin, PathBuf::from("llama-server"));
    assert_eq!(c.models_dir, PathBuf::from("./models"));
    assert_eq!(c.idle_timeout, Some(Duration::from_secs(600)));
    assert_eq!(c.load_timeout, Duration::from_secs(300));
}

#[test]
fn reads_overrides_and_zero_disables_watchdog() {
    let c = config(&[
        ("DATABASE_URL", "postgres://x"),
        ("BIND_ADDR", "0.0.0.0:9000"),
        ("LLAMA_SERVER_BIN", "/opt/llama-server"),
        ("MODELS_DIR", "/models"),
        ("IDLE_TIMEOUT_SECS", "0"),
        ("LLAMA_LOAD_TIMEOUT_SECS", "5"),
    ])
    .unwrap();
    assert_eq!(c.bind_addr.port(), 9000);
    assert_eq!(c.llama_server_bin, PathBuf::from("/opt/llama-server"));
    assert_eq!(c.models_dir, PathBuf::from("/models"));
    assert_eq!(c.idle_timeout, None);
    assert_eq!(c.load_timeout, Duration::from_secs(5));
}

#[test]
fn reports_missing_and_invalid_values() {
    assert!(config(&[]).unwrap_err().contains("DATABASE_URL"));
    let bad_addr = config(&[("DATABASE_URL", "x"), ("BIND_ADDR", "nope")]);
    assert!(bad_addr.unwrap_err().contains("BIND_ADDR"));
    let bad_num = config(&[("DATABASE_URL", "x"), ("IDLE_TIMEOUT_SECS", "-1")]);
    assert!(bad_num.unwrap_err().contains("IDLE_TIMEOUT_SECS"));
}
