use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: SocketAddr,
    pub llama_server_bin: PathBuf,
    pub models_dir: PathBuf,
    /// `None` disables the idle watchdog.
    pub idle_timeout: Option<Duration>,
    pub load_timeout: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|key| env::var(key).ok())
    }

    /// Builds the config from any key lookup, so tests need not touch the process env.
    fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let env_or = |key: &str, default: &str| get(key).unwrap_or_else(|| default.to_string());
        let database_url = get("DATABASE_URL").ok_or("DATABASE_URL must be set")?;
        let bind_addr = env_or("BIND_ADDR", "127.0.0.1:8787")
            .parse()
            .map_err(|e| format!("invalid BIND_ADDR: {e}"))?;
        let idle_secs: u64 = parse_var(&get, "IDLE_TIMEOUT_SECS", 600)?;
        let load_secs: u64 = parse_var(&get, "LLAMA_LOAD_TIMEOUT_SECS", 300)?;

        Ok(Self {
            database_url,
            bind_addr,
            llama_server_bin: env_or("LLAMA_SERVER_BIN", "llama-server").into(),
            models_dir: env_or("MODELS_DIR", "./models").into(),
            idle_timeout: (idle_secs > 0).then(|| Duration::from_secs(idle_secs)),
            load_timeout: Duration::from_secs(load_secs),
        })
    }
}

fn parse_var<T: std::str::FromStr>(
    get: impl Fn(&str) -> Option<String>,
    key: &str,
    default: T,
) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    match get(key) {
        Some(v) => v.parse().map_err(|e| format!("invalid {key}: {e}")),
        None => Ok(default),
    }
}

#[cfg(test)]
mod tests {
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
}
