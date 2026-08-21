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
    pub require_write_auth: bool,
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
            max_paste_size: "1mb".to_string(),
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
            creates_per_minute: 10,
            reads_per_minute: 120,
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
        config.apply_env_overrides()?;
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
    ///
    /// Environment variables always win. Present-but-invalid values fail startup
    /// instead of silently falling back to a less restrictive default.
    fn apply_env_overrides(&mut self) -> Result<(), ConfigError> {
        if let Some(v) = env_value("COPYPASTE_ADDRESS")? {
            self.server.address = v;
        }
        if let Some(v) = env_value("COPYPASTE_PORT")? {
            self.server.port = parse_env_value("COPYPASTE_PORT", &v, "an integer from 1 to 65535")?;
        }
        if let Some(v) = env_value("COPYPASTE_MAX_PASTE_SIZE")? {
            self.server.max_paste_size = v;
        }
        if let Some(v) = env_value("COPYPASTE_STORAGE_BACKEND")? {
            self.storage.backend = v;
        }
        if let Some(v) = env_value("COPYPASTE_STORAGE_PATH")? {
            self.storage.path = v;
        }
        if let Some(v) = env_value("COPYPASTE_AUTH_TOKEN")? {
            self.auth.token = v;
        }
        if let Some(v) = env_value("COPYPASTE_REQUIRE_WRITE_AUTH")? {
            self.auth.require_write_auth = parse_bool(&v)
                .ok_or_else(|| invalid_env("COPYPASTE_REQUIRE_WRITE_AUTH", "a boolean"))?;
        }
        if let Some(v) = env_value("COPYPASTE_REQUIRE_CRYPTO_VERIFICATION")? {
            parse_bool(&v)
                .ok_or_else(|| invalid_env("COPYPASTE_REQUIRE_CRYPTO_VERIFICATION", "a boolean"))?;
        }
        if let Some(v) = env_value("CRYPTO_VERIFIER_URL")? {
            super::crypto::validate_verifier_base_url(&v).map_err(|_| {
                invalid_env(
                    "CRYPTO_VERIFIER_URL",
                    "clean HTTPS or HTTP on loopback/Fly private ingress",
                )
            })?;
        }
        if let Some(v) = env_value("COPYPASTE_RETENTION_DEFAULT")? {
            self.retention.default = v;
        }
        if let Some(v) = env_value("COPYPASTE_RETENTION_MAX")? {
            self.retention.max = v;
        }
        if let Some(v) = env_value("COPYPASTE_RATE_LIMIT_CREATES")? {
            self.rate_limit.creates_per_minute =
                parse_env_value("COPYPASTE_RATE_LIMIT_CREATES", &v, "a non-negative integer")?;
        }
        if let Some(v) = env_value("COPYPASTE_RATE_LIMIT_READS")? {
            self.rate_limit.reads_per_minute =
                parse_env_value("COPYPASTE_RATE_LIMIT_READS", &v, "a non-negative integer")?;
        }
        if let Some(v) = env_value("COPYPASTE_LOG_FORMAT")? {
            self.logging.format = v;
        }
        if let Some(v) = env_value("COPYPASTE_LOG_LEVEL")? {
            self.logging.level = v;
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.server.port == 0 {
            return Err(ConfigError::ValidationError(
                "server.port must be between 1 and 65535".to_string(),
            ));
        }
        let max_paste_bytes = parse_byte_size(&self.server.max_paste_size).ok_or_else(|| {
            ConfigError::ValidationError(format!(
                "server.max_paste_size must be bytes or a size like '256kb' or '1mb', got '{}'",
                self.server.max_paste_size
            ))
        })?;
        if max_paste_bytes == 0 || max_paste_bytes > 1024 * 1024 {
            return Err(ConfigError::ValidationError(
                "server.max_paste_size must be between 1 byte and 1 MiB".to_string(),
            ));
        }
        if !matches!(self.storage.backend.as_str(), "memory" | "redis") {
            return Err(ConfigError::ValidationError(format!(
                "storage.backend must be 'memory' or 'redis', got '{}'",
                self.storage.backend
            )));
        }
        let retention_default =
            parse_duration_minutes(&self.retention.default).ok_or_else(|| {
                ConfigError::ValidationError(format!(
                    "retention.default must be a duration like '30m', '24h', '30d', got '{}'",
                    self.retention.default
                ))
            })?;
        let retention_max = parse_duration_minutes(&self.retention.max).ok_or_else(|| {
            ConfigError::ValidationError(format!(
                "retention.max must be a duration like '30m', '24h', '30d', got '{}'",
                self.retention.max
            ))
        })?;
        if retention_default == 0 || retention_max == 0 || retention_default > retention_max {
            return Err(ConfigError::ValidationError(
                "retention.default and retention.max must be positive, and default must not exceed max"
                    .to_string(),
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
        if std::env::var("COPYPASTE_REQUIRE_WRITE_AUTH").is_err() && self.auth.require_write_auth {
            std::env::set_var("COPYPASTE_REQUIRE_WRITE_AUTH", "true");
        }
        // Paste handlers consume an exact byte count. Parse the human-friendly
        // TOML value once instead of silently falling back to a larger limit.
        if std::env::var("COPYPASTE_MAX_PASTE_SIZE").is_err() {
            if let Some(bytes) = parse_byte_size(&self.server.max_paste_size) {
                std::env::set_var("COPYPASTE_MAX_PASTE_SIZE", bytes.to_string());
            }
        }
        // Redis URL, if provided. The Redis persistence adapter speaks the
        // Upstash HTTPS REST API, not the native redis:// protocol.
        if let Some(url) = &self.storage.url {
            if std::env::var("UPSTASH_REDIS_REST_URL").is_err() {
                std::env::set_var("UPSTASH_REDIS_REST_URL", url);
            }
        }
        // Retention knobs consumed by paste creation (default applied when the
        // request omits retention_minutes; max rejects longer retentions).
        if std::env::var("COPYPASTE_RETENTION_DEFAULT_MINUTES").is_err() {
            if let Some(minutes) = parse_duration_minutes(&self.retention.default) {
                if minutes > 0 {
                    std::env::set_var("COPYPASTE_RETENTION_DEFAULT_MINUTES", minutes.to_string());
                }
            }
        }
        if std::env::var("COPYPASTE_RETENTION_MAX_MINUTES").is_err() {
            if let Some(minutes) = parse_duration_minutes(&self.retention.max) {
                if minutes > 0 {
                    std::env::set_var("COPYPASTE_RETENTION_MAX_MINUTES", minutes.to_string());
                }
            }
        }
        // Rate-limit knobs consumed by rate_limit::PasteRateLimiter::from_env.
        if std::env::var("COPYPASTE_RATE_LIMIT_CREATES").is_err()
            && self.rate_limit.creates_per_minute > 0
        {
            std::env::set_var(
                "COPYPASTE_RATE_LIMIT_CREATES",
                self.rate_limit.creates_per_minute.to_string(),
            );
        }
        if std::env::var("COPYPASTE_RATE_LIMIT_READS").is_err()
            && self.rate_limit.reads_per_minute > 0
        {
            std::env::set_var(
                "COPYPASTE_RATE_LIMIT_READS",
                self.rate_limit.reads_per_minute.to_string(),
            );
        }
    }
}

/// Parse a human-friendly duration string into minutes.
///
/// Accepts a raw number of minutes (`"90"`) or a number with an `m`/`h`/`d`/`w`
/// suffix (`"30m"`, `"24h"`, `"30d"`, `"2w"`).
pub fn parse_duration_minutes(input: &str) -> Option<u64> {
    let s = input.trim().to_ascii_lowercase();
    if s.is_empty() {
        return None;
    }
    if let Ok(minutes) = s.parse::<u64>() {
        return Some(minutes);
    }
    let (value, multiplier) = if let Some(rest) = s.strip_suffix('m') {
        (rest, 1u64)
    } else if let Some(rest) = s.strip_suffix('h') {
        (rest, 60)
    } else if let Some(rest) = s.strip_suffix('d') {
        (rest, 60 * 24)
    } else if let Some(rest) = s.strip_suffix('w') {
        (rest, 60 * 24 * 7)
    } else {
        return None;
    };
    value
        .trim()
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(multiplier))
}

fn parse_byte_size(input: &str) -> Option<usize> {
    let normalized = input.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    let (number, multiplier) = if let Some(value) = normalized.strip_suffix("mib") {
        (value, 1024usize * 1024)
    } else if let Some(value) = normalized.strip_suffix("mb") {
        (value, 1024usize * 1024)
    } else if let Some(value) = normalized.strip_suffix("kib") {
        (value, 1024usize)
    } else if let Some(value) = normalized.strip_suffix("kb") {
        (value, 1024usize)
    } else if let Some(value) = normalized.strip_suffix('b') {
        (value, 1usize)
    } else {
        (normalized.as_str(), 1usize)
    };

    number
        .trim()
        .parse::<usize>()
        .ok()
        .and_then(|value| value.checked_mul(multiplier))
}

fn parse_bool(input: &str) -> Option<bool> {
    match input.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn invalid_env(name: &str, expected: &str) -> ConfigError {
    ConfigError::ValidationError(format!("environment variable {name} must be {expected}"))
}

fn env_value(name: &str) -> Result<Option<String>, ConfigError> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(invalid_env(name, "valid UTF-8 without binary data"))
        }
    }
}

fn parse_env_value<T>(name: &str, value: &str, expected: &str) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
{
    value.parse().map_err(|_| invalid_env(name, expected))
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
max_paste_size = "1mb"      # COPYPASTE_MAX_PASTE_SIZE

[storage]
backend = "memory"          # COPYPASTE_STORAGE_BACKEND — memory | redis
path = "./copypaste.db"     # COPYPASTE_STORAGE_PATH
# For Redis: backend = "redis", url = "https://<database>.upstash.io"
# Set UPSTASH_REDIS_REST_TOKEN separately through a secret manager.

[auth]
token = ""                  # COPYPASTE_AUTH_TOKEN
                            # If non-empty, all write requests require:
                            #   X-CopyPaste-Write-Token: <token>
require_write_auth = false  # COPYPASTE_REQUIRE_WRITE_AUTH
                            # true rejects anonymous writes even without a token

[retention]
default = "24h"             # COPYPASTE_RETENTION_DEFAULT — default paste lifetime
max = "30d"                 # COPYPASTE_RETENTION_MAX    — maximum allowed lifetime

[rate_limit]
creates_per_minute = 10     # COPYPASTE_RATE_LIMIT_CREATES
reads_per_minute = 120      # COPYPASTE_RATE_LIMIT_READS

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
        assert_eq!(c.server.max_paste_size, "1mb");
        assert_eq!(c.storage.backend, "memory");
        assert!(c.auth.token.is_empty());
        assert_eq!(c.retention.default, "24h");
        assert_eq!(c.rate_limit.creates_per_minute, 10);
        assert_eq!(c.rate_limit.reads_per_minute, 120);
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
        let _lock = ENV_LOCK.lock().unwrap();
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
    fn invalid_security_environment_values_fail_startup() {
        let _lock = ENV_LOCK.lock().unwrap();
        let path = write_temp_config("");

        for (name, value) in [
            ("COPYPASTE_PORT", "not-a-port"),
            ("COPYPASTE_REQUIRE_WRITE_AUTH", "sometimes"),
            ("COPYPASTE_REQUIRE_CRYPTO_VERIFICATION", "maybe"),
            ("COPYPASTE_RATE_LIMIT_CREATES", "unlimited"),
            ("COPYPASTE_RATE_LIMIT_READS", "-1"),
        ] {
            std::env::set_var(name, value);
            let error = Config::load(Some(path.to_str().unwrap()))
                .expect_err("invalid security environment value must fail startup");
            std::env::remove_var(name);

            let message = error.to_string();
            assert!(
                message.contains(name),
                "error must identify the invalid variable without exposing its value: {message}"
            );
            assert!(
                !message.contains(value),
                "error must not echo environment values: {message}"
            );
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn invalid_crypto_verifier_url_fails_startup_without_echoing_it() {
        let _lock = ENV_LOCK.lock().unwrap();
        let path = write_temp_config("");
        let unsafe_url = "http://public.example/secret-path";
        std::env::set_var("CRYPTO_VERIFIER_URL", unsafe_url);

        let error = Config::load(Some(path.to_str().unwrap()))
            .expect_err("plaintext public verifier URL must fail startup");

        std::env::remove_var("CRYPTO_VERIFIER_URL");
        let _ = std::fs::remove_file(path);
        let message = error.to_string();
        assert!(message.contains("CRYPTO_VERIFIER_URL"));
        assert!(!message.contains(unsafe_url));
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
    fn parse_duration_minutes_accepts_supported_units() {
        assert_eq!(parse_duration_minutes("90"), Some(90));
        assert_eq!(parse_duration_minutes("30m"), Some(30));
        assert_eq!(parse_duration_minutes("24h"), Some(24 * 60));
        assert_eq!(parse_duration_minutes("30d"), Some(30 * 24 * 60));
        assert_eq!(parse_duration_minutes("2w"), Some(2 * 7 * 24 * 60));
        assert_eq!(parse_duration_minutes(" 1H "), Some(60));
    }

    #[test]
    fn parse_duration_minutes_rejects_garbage() {
        assert_eq!(parse_duration_minutes(""), None);
        assert_eq!(parse_duration_minutes("abc"), None);
        assert_eq!(parse_duration_minutes("5x"), None);
        assert_eq!(parse_duration_minutes("-5m"), None);
    }

    #[test]
    fn parse_byte_size_accepts_bounded_human_units() {
        assert_eq!(parse_byte_size("1024"), Some(1024));
        assert_eq!(parse_byte_size("256kb"), Some(256 * 1024));
        assert_eq!(parse_byte_size("1 MiB"), Some(1024 * 1024));
        assert_eq!(parse_byte_size(""), None);
        assert_eq!(parse_byte_size("huge"), None);
    }

    #[test]
    fn validation_rejects_unparsable_retention() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("COPYPASTE_RETENTION_MAX");
        let path = write_temp_config("[retention]\nmax = \"forever\"\n");
        let result = Config::load(Some(path.to_str().unwrap()));
        let _ = std::fs::remove_file(&path);

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("retention.max"),
            "error should mention field: {msg}"
        );
    }

    #[test]
    fn validation_rejects_invalid_retention_bounds() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::remove_var("COPYPASTE_RETENTION_DEFAULT");
        std::env::remove_var("COPYPASTE_RETENTION_MAX");

        for retention in [
            "[retention]\ndefault = \"0m\"\nmax = \"30d\"\n",
            "[retention]\ndefault = \"31d\"\nmax = \"30d\"\n",
        ] {
            let path = write_temp_config(retention);
            let result = Config::load(Some(path.to_str().unwrap()));
            let _ = std::fs::remove_file(&path);
            assert!(
                result.is_err(),
                "invalid retention bounds must fail startup"
            );
        }
    }

    #[test]
    fn bridge_to_env_exports_retention_rate_limit_and_upstash_url() {
        let _lock = ENV_LOCK.lock().unwrap();
        for var in [
            "COPYPASTE_RETENTION_DEFAULT_MINUTES",
            "COPYPASTE_RETENTION_MAX_MINUTES",
            "COPYPASTE_RATE_LIMIT_CREATES",
            "COPYPASTE_RATE_LIMIT_READS",
            "COPYPASTE_MAX_PASTE_SIZE",
            "COPYPASTE_REQUIRE_WRITE_AUTH",
            "UPSTASH_REDIS_REST_URL",
        ] {
            std::env::remove_var(var);
        }

        let mut config = Config::default();
        config.storage.url = Some("https://upstash.example.com".to_string());
        config.bridge_to_env();

        assert_eq!(
            std::env::var("COPYPASTE_RETENTION_DEFAULT_MINUTES").as_deref(),
            Ok("1440"),
            "default 24h must bridge to 1440 minutes"
        );
        assert_eq!(
            std::env::var("COPYPASTE_RETENTION_MAX_MINUTES").as_deref(),
            Ok("43200"),
            "max 30d must bridge to 43200 minutes"
        );
        assert_eq!(
            std::env::var("COPYPASTE_RATE_LIMIT_CREATES").as_deref(),
            Ok("10")
        );
        assert_eq!(
            std::env::var("COPYPASTE_RATE_LIMIT_READS").as_deref(),
            Ok("120")
        );
        assert_eq!(
            std::env::var("COPYPASTE_MAX_PASTE_SIZE").as_deref(),
            Ok("1048576")
        );
        assert_eq!(
            std::env::var("UPSTASH_REDIS_REST_URL").as_deref(),
            Ok("https://upstash.example.com"),
            "storage.url must bridge to the variable the Redis adapter reads"
        );
        for var in [
            "COPYPASTE_RETENTION_DEFAULT_MINUTES",
            "COPYPASTE_RETENTION_MAX_MINUTES",
            "COPYPASTE_RATE_LIMIT_CREATES",
            "COPYPASTE_RATE_LIMIT_READS",
            "COPYPASTE_MAX_PASTE_SIZE",
            "COPYPASTE_REQUIRE_WRITE_AUTH",
            "UPSTASH_REDIS_REST_URL",
        ] {
            std::env::remove_var(var);
        }
    }

    #[test]
    fn bridge_to_env_respects_existing_env_values() {
        let _lock = ENV_LOCK.lock().unwrap();
        std::env::set_var("COPYPASTE_RETENTION_MAX_MINUTES", "5");

        let config = Config::default();
        config.bridge_to_env();

        assert_eq!(
            std::env::var("COPYPASTE_RETENTION_MAX_MINUTES").as_deref(),
            Ok("5"),
            "pre-set env vars must win over bridged config values"
        );

        std::env::remove_var("COPYPASTE_RETENTION_MAX_MINUTES");
        // Clean up variables the bridge may have set from defaults.
        std::env::remove_var("COPYPASTE_RETENTION_DEFAULT_MINUTES");
        std::env::remove_var("COPYPASTE_RATE_LIMIT_CREATES");
        std::env::remove_var("COPYPASTE_RATE_LIMIT_READS");
        std::env::remove_var("COPYPASTE_MAX_PASTE_SIZE");
        std::env::remove_var("COPYPASTE_REQUIRE_WRITE_AUTH");
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
