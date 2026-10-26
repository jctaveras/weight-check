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
mod tests;
