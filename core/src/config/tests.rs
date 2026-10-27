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
    let c = config(&[]).unwrap();
    assert_eq!(c.llama_server_bin, PathBuf::from("llama-server"));
    assert_eq!(c.models_dir, PathBuf::from("./models"));
    assert_eq!(c.idle_timeout, Some(Duration::from_secs(600)));
    assert_eq!(c.load_timeout, Duration::from_secs(300));
}

#[test]
fn reads_overrides_and_zero_disables_watchdog() {
    let c = config(&[
        ("LLAMA_SERVER_BIN", "/opt/llama-server"),
        ("MODELS_DIR", "/models"),
        ("IDLE_TIMEOUT_SECS", "0"),
        ("LLAMA_LOAD_TIMEOUT_SECS", "5"),
    ])
    .unwrap();
    assert_eq!(c.llama_server_bin, PathBuf::from("/opt/llama-server"));
    assert_eq!(c.models_dir, PathBuf::from("/models"));
    assert_eq!(c.idle_timeout, None);
    assert_eq!(c.load_timeout, Duration::from_secs(5));
}

#[test]
fn reports_invalid_values() {
    let bad_num = config(&[("IDLE_TIMEOUT_SECS", "-1")]);
    assert!(bad_num.unwrap_err().contains("IDLE_TIMEOUT_SECS"));
    let bad_load = config(&[("LLAMA_LOAD_TIMEOUT_SECS", "soon")]);
    assert!(bad_load.unwrap_err().contains("LLAMA_LOAD_TIMEOUT_SECS"));
}
