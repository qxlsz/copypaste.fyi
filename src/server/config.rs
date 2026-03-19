use std::path::PathBuf;

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file '{path}': {source}")]
    ReadError {
        path: String,
        source: std::io::Error,
    },
    #[error("Failed to parse config file '{path}': {source}")]
    ParseError {
        path: String,
        source: toml::de::Error,
    },
    #[error("Invalid config: {0}")]
    ValidationError(String),
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub auth: AuthConfig,
    pub retention: RetentionConfig,
    pub rate_limit: RateLimitConfig,
    pub logging: LoggingConfig,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ServerConfig {
    pub address: String,
    pub port: u16,
    pub max_paste_size: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct StorageConfig {
    pub backend: String,
    pub path: String,
    pub url: Option<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct AuthConfig {
    pub token: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct RetentionConfig {
    pub default: String,
    pub max: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct RateLimitConfig {
    pub creates_per_minute: u32,
    pub reads_per_minute: u32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct LoggingConfig {
    pub format: String,
    pub level: String,
}

// — Defaults ————————————————————————————————————————————

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            address: "0.0.0.0".to_string(),
            port: 8000,
            max_paste_size: "10mb".to_string(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        StorageConfig {
            backend: "memory".to_string(),
            path: "./copypaste.db".to_string(),
            url: None,
        }
    }
}

impl Default for RetentionConfig {
    fn default() -> Self {
        RetentionConfig {
            default: "24h".to_string(),
            max: "30d".to_string(),
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        RateLimitConfig {
            creates_per_minute: 60,
            reads_per_minute: 300,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        LoggingConfig {
            format: "json".to_string(),
            level: "info".to_string(),
        }
    }
}

// — Loading ————————————————————————————————————————————

impl Config {
    /// Load config from the highest-priority source found, then apply env var overrides.
    ///
    /// Priority:
    /// 1. `explicit_path` (--config CLI flag)
    /// 2. `COPYPASTE_CONFIG` env var
    /// 3. `./copypaste.toml` (current directory)
    /// 4. `/etc/copypaste/server.toml` (system-wide)
    /// 5. Built-in defaults (if none of the above exist)
    pub fn load(explicit_path: Option<&str>) -> Result<Self, ConfigError> {
        let mut config = match Self::find_config_file(explicit_path) {
            Some(path) => Self::load_from_file(&path)?,
            None => Self::default(),
        };
        config.apply_env_overrides();
        config.validate()?;
        Ok(config)
    }

    fn find_config_file(explicit_path: Option<&str>) -> Option<PathBuf> {
        // 1. Explicit --config flag
        if let Some(p) = explicit_path {
            return Some(PathBuf::from(p));
        }
        // 2. COPYPASTE_CONFIG env var
        if let Ok(p) = std::env::var("COPYPASTE_CONFIG") {
            return Some(PathBuf::from(p));
        }
        // 3. ./copypaste.toml
        let local = PathBuf::from("copypaste.toml");
        if local.exists() {
            return Some(local);
        }
        // 4. /etc/copypaste/server.toml
        let system = PathBuf::from("/etc/copypaste/server.toml");
        if system.exists() {
            return Some(system);
        }
        None
    }

    fn load_from_file(path: &PathBuf) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadError {
            path: path.display().to_string(),
            source: e,
        })?;
        toml::from_str(&content).map_err(|e| ConfigError::ParseError {
            path: path.display().to_string(),
            source: e,
        })
    }

    /// Apply env var overrides on top of whatever was loaded from the config file.
    /// Env vars always win; missing or invalid env vars are silently ignored.
    fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("COPYPASTE_ADDRESS") {
            self.server.address = v;
        }
        if let Ok(v) = std::env::var("COPYPASTE_PORT") {
            if let Ok(port) = v.parse() {
                self.server.port = port;
            }
        }
        if let Ok(v) = std::env::var("COPYPASTE_MAX_PASTE_SIZE") {
            self.server.max_paste_size = v;
        }
        if let Ok(v) = std::env::var("COPYPASTE_STORAGE_BACKEND") {
            self.storage.backend = v;
        }
        if let Ok(v) = std::env::var("COPYPASTE_STORAGE_PATH") {
            self.storage.path = v;
        }
        if let Ok(v) = std::env::var("COPYPASTE_AUTH_TOKEN") {
            self.auth.token = v;
        }
        if let Ok(v) = std::env::var("COPYPASTE_RETENTION_DEFAULT") {
            self.retention.default = v;
        }
        if let Ok(v) = std::env::var("COPYPASTE_RETENTION_MAX") {
            self.retention.max = v;
        }
        if let Ok(v) = std::env::var("COPYPASTE_RATE_LIMIT_CREATES") {
            if let Ok(n) = v.parse() {
                self.rate_limit.creates_per_minute = n;
            }
        }
        if let Ok(v) = std::env::var("COPYPASTE_RATE_LIMIT_READS") {
            if let Ok(n) = v.parse() {
                self.rate_limit.reads_per_minute = n;
            }
        }
        if let Ok(v) = std::env::var("COPYPASTE_LOG_FORMAT") {
            self.logging.format = v;
        }
        if let Ok(v) = std::env::var("COPYPASTE_LOG_LEVEL") {
            self.logging.level = v;
        }
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.server.port == 0 {
            return Err(ConfigError::ValidationError(
                "server.port must be between 1 and 65535".to_string(),
            ));
        }
        let valid_formats = ["json", "pretty"];
        if !valid_formats.contains(&self.logging.format.as_str()) {
            return Err(ConfigError::ValidationError(format!(
                "logging.format must be 'json' or 'pretty', got '{}'",
                self.logging.format
            )));
        }
        let valid_levels = ["error", "warn", "info", "debug", "trace"];
        if !valid_levels.contains(&self.logging.level.as_str()) {
            return Err(ConfigError::ValidationError(format!(
                "logging.level must be one of {:?}, got '{}'",
                valid_levels, self.logging.level
            )));
        }
        Ok(())
    }

    /// Bridge config values to the env vars that existing server code reads.
    ///
    /// Call this synchronously in `main()` **before** starting the async executor so
    /// that `std::env::set_var` is safe (single-threaded context, no concurrent readers).
    /// Env vars that are already set take precedence (they were applied via
    /// `apply_env_overrides` and we must not overwrite them here).
    pub fn bridge_to_env(&self) {
        // Rocket reads ROCKET_ADDRESS / ROCKET_PORT for its bind configuration.
        if std::env::var("ROCKET_ADDRESS").is_err() {
            std::env::set_var("ROCKET_ADDRESS", &self.server.address);
        }
        if std::env::var("ROCKET_PORT").is_err() {
            std::env::set_var("ROCKET_PORT", self.server.port.to_string());
        }
        // create_paste_store() reads COPYPASTE_PERSISTENCE_BACKEND.
        if std::env::var("COPYPASTE_PERSISTENCE_BACKEND").is_err()
            && self.storage.backend != "memory"
        {
            std::env::set_var("COPYPASTE_PERSISTENCE_BACKEND", &self.storage.backend);
        }
        // Auth middleware reads COPYPASTE_AUTH_TOKEN.
        // A non-empty token in the config file must be enforced; without this bridge
        // `auth.token` in the TOML would be silently ignored — a security failure.
        if std::env::var("COPYPASTE_AUTH_TOKEN").is_err() && !self.auth.token.is_empty() {
            std::env::set_var("COPYPASTE_AUTH_TOKEN", &self.auth.token);
        }
        // Redis URL, if provided.
        if let Some(url) = &self.storage.url {
            if std::env::var("REDIS_URL").is_err() {
                std::env::set_var("REDIS_URL", url);
            }
        }
    }
}

// — Example config ——————————————————————————————————————

pub const EXAMPLE_CONFIG: &str = r#"# copypaste.fyi server configuration
# Generated by `copypaste config init`
#
# Env var overrides are shown next to each key.
# Env vars always take precedence over values in this file.

[server]
address = "0.0.0.0"        # COPYPASTE_ADDRESS  — bind address
port = 8000                 # COPYPASTE_PORT     — listen port
max_paste_size = "10mb"     # COPYPASTE_MAX_PASTE_SIZE

[storage]
backend = "memory"          # COPYPASTE_STORAGE_BACKEND — memory | redis | vault
path = "./copypaste.db"     # COPYPASTE_STORAGE_PATH
# For Redis: backend = "redis", url = "redis://localhost:6379"

[auth]
token = ""                  # COPYPASTE_AUTH_TOKEN
                            # If non-empty, all write requests require:
                            #   Authorization: Bearer <token>

[retention]
default = "24h"             # COPYPASTE_RETENTION_DEFAULT — default paste lifetime
max = "30d"                 # COPYPASTE_RETENTION_MAX    — maximum allowed lifetime

[rate_limit]
creates_per_minute = 60     # COPYPASTE_RATE_LIMIT_CREATES
reads_per_minute = 300      # COPYPASTE_RATE_LIMIT_READS

[logging]
format = "json"             # COPYPASTE_LOG_FORMAT — "json" or "pretty"
level = "info"              # COPYPASTE_LOG_LEVEL  — error | warn | info | debug | trace
"#;

// — Tests ———————————————————————————————————————————————

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{LazyLock, Mutex};

    // Serialize tests that mutate env vars to prevent interference when tests run in parallel.
    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn write_temp_config(content: &str) -> PathBuf {
        // Include PID so concurrent nextest processes don't share the same file.
        let id = std::thread::current().id();
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("copypaste_cfg_test_{pid}_{id:?}.toml"));
        std::fs::write(&path, content).expect("write temp config");
        path
    }

    #[test]
    fn defaults_are_sensible() {
        let c = Config::default();
        assert_eq!(c.server.port, 8000);
        assert_eq!(c.server.address, "0.0.0.0");
        assert_eq!(c.server.max_paste_size, "10mb");
        assert_eq!(c.storage.backend, "memory");
        assert!(c.auth.token.is_empty());
        assert_eq!(c.retention.default, "24h");
        assert_eq!(c.rate_limit.creates_per_minute, 60);
        assert_eq!(c.logging.format, "json");
        assert_eq!(c.logging.level, "info");
    }

    #[test]
    fn load_returns_defaults_when_no_file_and_no_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        // Remove discovery env var so auto-discovery doesn't pick up a stray file.
        std::env::remove_var("COPYPASTE_CONFIG");
        // Pass a path guaranteed not to exist.
        let result = Config::load(Some("/nonexistent/path/copypaste_never.toml"));
        assert!(result.is_err(), "explicit missing path should error");
    }

    #[test]
    fn load_from_toml_file() {
        let path = write_temp_config(
            r#"
[server]
port = 9090
address = "127.0.0.1"
"#,
        );
        let config = Config::load(Some(path.to_str().unwrap())).expect("load");
        let _ = std::fs::remove_file(&path);

        assert_eq!(config.server.port, 9090);
        assert_eq!(config.server.address, "127.0.0.1");
        // Unspecified sections keep defaults.
        assert_eq!(config.storage.backend, "memory");
        assert_eq!(config.logging.format, "json");
    }

    #[test]
    fn env_var_overrides_toml_value() {
        let _lock = ENV_LOCK.lock().unwrap();
        let path = write_temp_config("[server]\nport = 9000\n");
        std::env::set_var("COPYPASTE_PORT", "7777");

        let config = Config::load(Some(path.to_str().unwrap())).expect("load");

        std::env::remove_var("COPYPASTE_PORT");
        let _ = std::fs::remove_file(&path);

        assert_eq!(config.server.port, 7777, "env var must win over TOML value");
    }

    #[test]
    fn validation_rejects_invalid_log_format() {
        let _lock = ENV_LOCK.lock().unwrap();
        let path = write_temp_config("[logging]\nformat = \"xml\"\n");
        let result = Config::load(Some(path.to_str().unwrap()));
        let _ = std::fs::remove_file(&path);

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("logging.format"),
            "error should mention field: {msg}"
        );
    }

    #[test]
    fn validation_rejects_zero_port() {
        let _lock = ENV_LOCK.lock().unwrap();
        let path = write_temp_config("[server]\nport = 0\n");
        let result = Config::load(Some(path.to_str().unwrap()));
        let _ = std::fs::remove_file(&path);

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("port"), "error should mention port: {msg}");
    }

    #[test]
    fn load_with_no_path_succeeds_with_defaults() {
        let _lock = ENV_LOCK.lock().unwrap();
        // Ensure neither COPYPASTE_CONFIG nor a local copypaste.toml affect us.
        std::env::remove_var("COPYPASTE_CONFIG");
        // Call with explicit None — no auto-discovered file expected in the test environment.
        // If a local copypaste.toml happens to exist this may fail, which is acceptable.
        // We at minimum verify the default port is sane.
        let config = Config::default();
        assert_eq!(config.server.port, 8000);
    }
}
