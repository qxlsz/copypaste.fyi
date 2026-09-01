use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use nanoid::nanoid;
use rand::Rng;
use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
    Request, State,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use super::sessions::SharedSessionStore;

// ── Rate limiter ──────────────────────────────────────────────────────────────

const MAX_FAILED_ATTEMPTS: u32 = 10;
const RATE_LIMIT_WINDOW_SECS: u64 = 300; // 5 minutes
const MAX_TRACKED_AUTH_CLIENTS: usize = 10_000;

/// Per-IP sliding-window failed-attempt counter (BUG-002).
pub struct RateLimiter {
    fails: Mutex<HashMap<String, (u32, Instant)>>,
    max_tracked_clients: usize,
}

pub type SharedRateLimiter = Arc<RateLimiter>;

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            fails: Mutex::new(HashMap::new()),
            max_tracked_clients: MAX_TRACKED_AUTH_CLIENTS,
        }
    }

    #[cfg(test)]
    fn with_capacity(max_tracked_clients: usize) -> Self {
        Self {
            fails: Mutex::new(HashMap::new()),
            max_tracked_clients: max_tracked_clients.max(1),
        }
    }

    /// Returns `true` if the IP has exceeded the failure threshold within the window.
    pub fn is_limited(&self, ip: &str) -> bool {
        let map = self.fails.lock().unwrap();
        map.get(ip)
            .map(|(count, since)| {
                since.elapsed().as_secs() <= RATE_LIMIT_WINDOW_SECS && *count >= MAX_FAILED_ATTEMPTS
            })
            .unwrap_or(false)
    }

    /// Records a failed authentication attempt.
    pub fn record_failure(&self, ip: &str) {
        self.record_failure_at(ip, Instant::now());
    }

    fn record_failure_at(&self, ip: &str, now: Instant) {
        let mut map = self.fails.lock().unwrap();

        // Bound attacker-controlled IP cardinality. Cleanup is O(n) only when
        // a new client reaches the cap; stale windows are removed first, then
        // the oldest live window is evicted if necessary.
        if !map.contains_key(ip) && map.len() >= self.max_tracked_clients {
            map.retain(|_, (_, since)| {
                now.saturating_duration_since(*since).as_secs() <= RATE_LIMIT_WINDOW_SECS
            });
            if map.len() >= self.max_tracked_clients {
                if let Some(oldest) = map
                    .iter()
                    .min_by_key(|(_, (_, since))| *since)
                    .map(|(client, _)| client.clone())
                {
                    map.remove(&oldest);
                }
            }
        }

        let entry = map.entry(ip.to_string()).or_insert((0, now));
        if now.saturating_duration_since(entry.1).as_secs() > RATE_LIMIT_WINDOW_SECS {
            // Window expired — reset counter, counting this failure
            *entry = (1, now);
        } else {
            entry.0 += 1;
        }
    }

    /// Clears the failure counter for an IP on successful authentication.
    pub fn clear_ip(&self, ip: &str) {
        self.fails.lock().unwrap().remove(ip);
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Scope ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApiScope {
    Read,
    #[default]
    Write,
    Admin,
}

impl ApiScope {
    pub fn can_read(self) -> bool {
        matches!(self, ApiScope::Read | ApiScope::Write | ApiScope::Admin)
    }

    pub fn can_write(self) -> bool {
        matches!(self, ApiScope::Write | ApiScope::Admin)
    }

    pub fn is_admin(self) -> bool {
        matches!(self, ApiScope::Admin)
    }
}

impl std::str::FromStr for ApiScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "read" => Ok(ApiScope::Read),
            "write" => Ok(ApiScope::Write),
            "admin" => Ok(ApiScope::Admin),
            other => Err(format!("unknown scope: {other}")),
        }
    }
}

impl std::fmt::Display for ApiScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiScope::Read => write!(f, "read"),
            ApiScope::Write => write!(f, "write"),
            ApiScope::Admin => write!(f, "admin"),
        }
    }
}

// ── Stored key (no hash) ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredApiKey {
    pub id: String,
    pub name: String,
    pub scope: ApiScope,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub expires_at: Option<i64>,
}

// ── Store ─────────────────────────────────────────────────────────────────────

pub struct SqliteApiKeyStore {
    conn: Option<Mutex<Connection>>,
}

pub type SharedApiKeyStore = Arc<SqliteApiKeyStore>;

#[derive(Debug, thiserror::Error)]
pub enum ApiKeyStoreOpenError {
    #[error("COPYPASTE_SQLITE_PATH is required for persistent API-key storage")]
    MissingPath,
    #[error("API-key database parent directory does not exist or is not a directory: {0}")]
    MissingParent(PathBuf),
    #[error("API-key database parent directory must be owned by this process and mode 0700: {0}")]
    InsecureParent(PathBuf),
    #[error("API-key database path is not a regular owner-controlled file: {0}")]
    InsecureFile(PathBuf),
    #[error("API-key database filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("API-key database initialization failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Static bearer tokens loaded once when Rocket is built.
///
/// Reading these values into managed state avoids process-global environment
/// lookups on every request and makes the effective authentication policy
/// immutable for the lifetime of a server instance.
#[derive(Debug, Default)]
pub struct StaticAuthTokens {
    write_token: Option<String>,
    admin_token: Option<String>,
    require_write_auth: bool,
    allow_session_writes: bool,
}

impl StaticAuthTokens {
    pub fn from_env() -> Self {
        Self::new(
            static_token_env("COPYPASTE_AUTH_TOKEN"),
            static_token_env("COPYPASTE_ADMIN_TOKEN"),
        )
        .with_required_write_auth(required_write_auth_from_env())
        .with_session_writes(boolean_env("COPYPASTE_ALLOW_SESSION_WRITES"))
    }

    pub fn new(write_token: Option<String>, admin_token: Option<String>) -> Self {
        Self {
            write_token: write_token.filter(|value| !value.trim().is_empty()),
            admin_token: admin_token.filter(|value| !value.trim().is_empty()),
            require_write_auth: false,
            allow_session_writes: false,
        }
    }

    pub fn with_required_write_auth(mut self, required: bool) -> Self {
        self.require_write_auth = required;
        self
    }

    /// Allow signed browser login sessions to create pastes.
    ///
    /// This is intentionally disabled by default: the login endpoint is
    /// self-service identity proof, not an invitation or authorization grant.
    pub fn with_session_writes(mut self, allowed: bool) -> Self {
        self.allow_session_writes = allowed;
        self
    }
}

fn required_write_auth_from_env() -> bool {
    boolean_env("COPYPASTE_REQUIRE_WRITE_AUTH")
}

fn boolean_env(name: &str) -> bool {
    let Ok(value) = std::env::var(name) else {
        return false;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "0" | "false" | "no" | "off" => false,
        "1" | "true" | "yes" | "on" => true,
        _ => panic!("{name} must be true or false"),
    }
}

fn valid_static_token(value: &str) -> bool {
    (43..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn static_token_env(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if value.trim().is_empty() => None,
        Ok(value) if valid_static_token(&value) => Some(value),
        Ok(_) => {
            panic!("{name} must be 43 to 128 base64url characters (letters, digits, '_' or '-')")
        }
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            panic!("{name} must contain valid Unicode")
        }
    }
}

impl SqliteApiKeyStore {
    /// Open a durable API-key database at an operator-controlled path.
    ///
    /// The parent directory must already exist. On Unix it must be owned by
    /// the effective process user with mode 0700; the database itself is
    /// created/opened without following a final symlink and forced to mode
    /// 0600. WAL plus a busy timeout supports multiple processes sharing this
    /// exact file. Deployments whose instances do not share a filesystem must
    /// run a single app instance or use a shared credential store instead.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ApiKeyStoreOpenError> {
        let path = path.as_ref();
        prepare_api_key_database_path(path)?;

        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        let store = Self {
            conn: Some(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// Open the path explicitly configured for the production server.
    pub fn open_configured() -> Result<Self, ApiKeyStoreOpenError> {
        let path = std::env::var_os("COPYPASTE_SQLITE_PATH")
            .filter(|value| !value.is_empty())
            .ok_or(ApiKeyStoreOpenError::MissingPath)?;
        Self::open(PathBuf::from(path))
    }

    pub fn in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Some(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// A production-safe mode for deployments that use only static tokens.
    /// Dynamic API-key verification and management are unavailable.
    pub fn disabled() -> Self {
        Self { conn: None }
    }

    pub fn is_enabled(&self) -> bool {
        self.conn.is_some()
    }

    fn init_schema(&self) -> Result<(), rusqlite::Error> {
        let Some(conn) = self.conn.as_ref() else {
            return Ok(());
        };
        let conn = conn.lock().unwrap();
        // Create the table with key_prefix for new databases.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS api_keys (
                id           TEXT PRIMARY KEY,
                name         TEXT NOT NULL,
                scope        TEXT NOT NULL,
                key_hash     TEXT NOT NULL,
                key_prefix   TEXT NOT NULL DEFAULT '',
                created_at   INTEGER NOT NULL,
                last_used_at INTEGER,
                expires_at   INTEGER
            );",
        )?;

        // Migrate databases created before the indexed prefix was introduced.
        // Inspecting the schema avoids swallowing unrelated ALTER TABLE errors.
        let has_key_prefix = {
            let mut statement = conn.prepare("PRAGMA table_info(api_keys)")?;
            let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
            let mut found = false;
            for column in columns {
                if column? == "key_prefix" {
                    found = true;
                    break;
                }
            }
            found
        };
        if !has_key_prefix {
            conn.execute_batch(
                "ALTER TABLE api_keys ADD COLUMN key_prefix TEXT NOT NULL DEFAULT '';",
            )?;
        }
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_api_keys_prefix ON api_keys(key_prefix);",
        )?;
        Ok(())
    }

    /// Create a new API key. Returns `(metadata, plaintext_key)`.
    /// The plaintext key is shown **once** — store it securely.
    pub fn create_key(
        &self,
        name: &str,
        scope: ApiScope,
        expires_at: Option<i64>,
    ) -> Result<(StoredApiKey, String), String> {
        let connection = self
            .conn
            .as_ref()
            .ok_or_else(|| "Dynamic API-key management is disabled".to_string())?;
        let raw_key = generate_api_key();
        let key_hash = hash_key(&raw_key)?;
        let prefix = key_prefix_for(&raw_key);

        let id = nanoid!(12);
        let now = current_ts();

        let key = StoredApiKey {
            id: id.clone(),
            name: name.to_string(),
            scope,
            created_at: now,
            last_used_at: None,
            expires_at,
        };

        let conn = connection.lock().unwrap();
        conn.execute(
            "INSERT INTO api_keys (id, name, scope, key_hash, key_prefix, created_at, last_used_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &key.id,
                &key.name,
                &key.scope.to_string(),
                &key_hash,
                &prefix,
                key.created_at,
                key.last_used_at,
                key.expires_at,
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok((key, raw_key))
    }

    /// List all keys (metadata only, no hashes).
    pub fn list_keys(&self) -> Result<Vec<StoredApiKey>, String> {
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| "Dynamic API-key management is disabled".to_string())?
            .lock()
            .unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, scope, created_at, last_used_at, expires_at
                 FROM api_keys ORDER BY created_at DESC",
            )
            .map_err(|e| e.to_string())?;

        let keys = stmt
            .query_map([], |row| {
                let scope_str: String = row.get(2)?;
                Ok(StoredApiKey {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    scope: scope_str.parse().unwrap_or(ApiScope::Read),
                    created_at: row.get(3)?,
                    last_used_at: row.get(4)?,
                    expires_at: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(keys)
    }

    /// Revoke a key by ID. Returns `true` if the key existed.
    pub fn revoke_key(&self, id: &str) -> Result<bool, String> {
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| "Dynamic API-key management is disabled".to_string())?
            .lock()
            .unwrap();
        let n = conn
            .execute("DELETE FROM api_keys WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(n > 0)
    }

    /// Verify a raw API key; returns metadata if valid (not expired, hash matches).
    ///
    /// The SQLite mutex is released before running Argon2 to avoid holding the
    /// lock during the expensive (~100 ms) hash verification (BUG-010 fix).
    pub fn verify_key(&self, raw_key: &str) -> Option<StoredApiKey> {
        struct Row {
            id: String,
            name: String,
            scope: String,
            key_hash: String,
            created_at: i64,
            expires_at: Option<i64>,
        }

        let now = current_ts();
        let prefix = key_prefix_for(raw_key);

        // Acquire lock, fetch candidates, then release before Argon2 (BUG-010).
        let candidates: Vec<Row> = {
            let conn = self.conn.as_ref()?.lock().unwrap();

            // Fast path: indexed lookup by key_prefix (BUG-003 fix).
            let fast: Vec<Row> = conn
                .prepare(
                    "SELECT id, name, scope, key_hash, created_at, expires_at
                     FROM api_keys WHERE key_prefix = ?1",
                )
                .ok()?
                .query_map(params![&prefix], |row| {
                    Ok(Row {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        scope: row.get(2)?,
                        key_hash: row.get(3)?,
                        created_at: row.get(4)?,
                        expires_at: row.get(5)?,
                    })
                })
                .ok()?
                .filter_map(|r| r.ok())
                .collect();

            if !fast.is_empty() {
                fast
            } else {
                // Backward-compat fallback: scan legacy rows with empty prefix.
                // These are keys created before the key_prefix migration.
                conn.prepare(
                    "SELECT id, name, scope, key_hash, created_at, expires_at
                     FROM api_keys WHERE key_prefix = ''",
                )
                .ok()?
                .query_map([], |row| {
                    Ok(Row {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        scope: row.get(2)?,
                        key_hash: row.get(3)?,
                        created_at: row.get(4)?,
                        expires_at: row.get(5)?,
                    })
                })
                .ok()?
                .filter_map(|r| r.ok())
                .collect()
            }
            // conn (and the mutex) is dropped here.
        };

        // Argon2 verification WITHOUT holding the SQLite mutex (BUG-010).
        let matched = candidates.into_iter().find(|row| {
            if let Some(exp) = row.expires_at {
                if exp < now {
                    return false;
                }
            }
            verify_key_hash(raw_key, &row.key_hash)
        })?;

        // Re-acquire lock briefly to update last_used_at.
        {
            let conn = self.conn.as_ref()?.lock().unwrap();
            let _ = conn.execute(
                "UPDATE api_keys SET last_used_at = ?1 WHERE id = ?2",
                params![now, &matched.id],
            );
        }

        Some(StoredApiKey {
            id: matched.id,
            name: matched.name,
            scope: matched.scope.parse().unwrap_or(ApiScope::Read),
            created_at: matched.created_at,
            last_used_at: Some(now),
            expires_at: matched.expires_at,
        })
    }

    /// Insert a key with an empty `key_prefix` to simulate a pre-migration row.
    /// Test-only helper for verifying the backward-compat fallback path.
    #[cfg(test)]
    pub fn insert_legacy_key_for_test(
        &self,
        id: &str,
        name: &str,
        scope: ApiScope,
        raw_key: &str,
    ) -> Result<(), String> {
        let key_hash = hash_key(raw_key)?;
        let now = current_ts();
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| "Dynamic API-key management is disabled".to_string())?
            .lock()
            .unwrap();
        conn.execute(
            "INSERT INTO api_keys (id, name, scope, key_hash, key_prefix, created_at)
             VALUES (?1, ?2, ?3, ?4, '', ?5)",
            params![id, name, &scope.to_string(), &key_hash, now],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn prepare_api_key_database_path(path: &Path) -> Result<(), ApiKeyStoreOpenError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_metadata =
        fs::metadata(parent).map_err(|_| ApiKeyStoreOpenError::MissingParent(parent.into()))?;
    if !parent_metadata.is_dir() {
        return Err(ApiKeyStoreOpenError::MissingParent(parent.into()));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

        if parent_metadata.uid() != unsafe { libc::geteuid() }
            || parent_metadata.permissions().mode() & 0o077 != 0
        {
            return Err(ApiKeyStoreOpenError::InsecureParent(parent.into()));
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.uid() != unsafe { libc::geteuid() } {
            return Err(ApiKeyStoreOpenError::InsecureFile(path.into()));
        }
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }

    #[cfg(not(unix))]
    {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;
        if !file.metadata()?.is_file() {
            return Err(ApiKeyStoreOpenError::InsecureFile(path.into()));
        }
    }

    Ok(())
}

// ── Helper functions ──────────────────────────────────────────────────────────

fn current_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn generate_api_key() -> String {
    let bytes: Vec<u8> = rand::thread_rng()
        .sample_iter(&rand::distributions::Standard)
        .take(32)
        .collect();
    format!("cp_{}", URL_SAFE_NO_PAD.encode(&bytes))
}

/// Computes a fast-lookup prefix: SHA-256 of the first 16 bytes of the key,
/// hex-encoded. Used as an indexed column to narrow Argon2 candidates to O(1).
fn key_prefix_for(raw_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(&raw_key.as_bytes()[..raw_key.len().min(16)]);
    hex::encode(hasher.finalize())
}

fn argon2_instance() -> Argon2<'static> {
    #[cfg(test)]
    {
        use argon2::{Algorithm, Params, Version};
        // Reduced params for fast tests (1 MiB, 1 iter, 1 thread)
        Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(1024, 1, 1, None).expect("valid test params"),
        )
    }
    #[cfg(not(test))]
    {
        Argon2::default()
    }
}

fn hash_key(key: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    argon2_instance()
        .hash_password(key.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

fn verify_key_hash(key: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| {
            argon2_instance()
                .verify_password(key.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

/// Compare bearer tokens without exposing the configured token length through
/// the comparison path. Hashing both inputs gives `subtle` fixed-size arrays.
fn constant_time_token_eq(candidate: &str, expected: &str) -> bool {
    let candidate_digest = Sha256::digest(candidate.as_bytes());
    let expected_digest = Sha256::digest(expected.as_bytes());
    candidate_digest.ct_eq(&expected_digest).into()
}

// ── Request guards ────────────────────────────────────────────────────────────

/// Authenticated key info extracted from a valid Bearer token.
#[derive(Debug, Clone)]
pub struct AuthenticatedKey {
    pub key_id: String,
    pub name: String,
    pub scope: ApiScope,
}

/// Optional auth guard: succeeds with `None` if no `Authorization` header is
/// present, succeeds with `Some` for a valid key, and fails (401) for an
/// invalid/expired key.
pub struct OptionalApiKeyAuth(pub Option<AuthenticatedKey>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for OptionalApiKeyAuth {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let token = match authorization_bearer(req) {
            Ok(None) => return Outcome::Success(OptionalApiKeyAuth(None)),
            Ok(Some(token)) => token,
            Err(status) => return Outcome::Error((status, ())),
        };

        let client_ip = req.client_ip().map(|ip| ip.to_string()).unwrap_or_default();

        // Rate limit check (BUG-002).
        let rl_arc = match req.guard::<&State<SharedRateLimiter>>().await {
            Outcome::Success(rl) => Some(rl.inner().clone()),
            _ => None,
        };
        if let Some(rl) = &rl_arc {
            if rl.is_limited(&client_ip) {
                return Outcome::Error((Status::TooManyRequests, ()));
            }
        }

        let store = match req.guard::<&State<SharedApiKeyStore>>().await {
            Outcome::Success(s) => s,
            _ => return Outcome::Error((Status::InternalServerError, ())),
        };

        let store_arc = store.inner().clone();
        let result = tokio::task::spawn_blocking(move || store_arc.verify_key(&token))
            .await
            .unwrap_or(None);

        match result {
            Some(key) => {
                if let Some(rl) = &rl_arc {
                    rl.clear_ip(&client_ip);
                }
                Outcome::Success(OptionalApiKeyAuth(Some(AuthenticatedKey {
                    key_id: key.id,
                    name: key.name,
                    scope: key.scope,
                })))
            }
            None => {
                if let Some(rl) = &rl_arc {
                    rl.record_failure(&client_ip);
                }
                Outcome::Error((Status::Unauthorized, ()))
            }
        }
    }
}

/// Creation guard honoring the configured write authorization policy.
///
/// `Authorization` may carry a signed user session so the created paste gets a
/// validated owner identity. On closed deployments, service admission is a
/// separate static/API credential in `X-CopyPaste-Write-Token`. Legacy clients
/// may still put only that service credential in `Authorization`, but a
/// self-issued user session is not admission unless the operator explicitly
/// enables the compatibility option `COPYPASTE_ALLOW_SESSION_WRITES=true`.
#[derive(Debug, Clone)]
pub enum WritePrincipal {
    Anonymous,
    StaticToken,
    ApiKey(AuthenticatedKey),
    UserSession { pubkey_hash: String },
}

pub struct RequireWriteAuth(pub WritePrincipal);

/// Separate admission guard for live-paste mutations.
///
/// `Authorization` remains exclusively the paste ownership capability. Closed
/// deployments require a service-level write credential in this distinct
/// header so possession of one capability never substitutes for the other.
pub struct RequireMutationWriteAuth(pub WritePrincipal);

pub const MUTATION_WRITE_TOKEN_HEADER: &str = "X-CopyPaste-Write-Token";

fn single_header_value(req: &Request<'_>, name: &str) -> Result<Option<String>, Status> {
    let mut values = req.headers().get(name);
    match (values.next(), values.next()) {
        (Some(value), None) if !value.trim().is_empty() => Ok(Some(value.to_owned())),
        (None, None) => Ok(None),
        // Duplicate or empty credentials are ambiguous and fail closed.
        _ => Err(Status::Unauthorized),
    }
}

fn authorization_bearer(req: &Request<'_>) -> Result<Option<String>, Status> {
    match single_header_value(req, "Authorization")? {
        Some(value) => value
            .strip_prefix("Bearer ")
            .filter(|token| !token.is_empty())
            .map(str::to_owned)
            .ok_or(Status::Unauthorized)
            .map(Some),
        None => Ok(None),
    }
}

/// Strict optional bearer capability for live-paste ownership checks.
/// Duplicate, blank, or malformed Authorization headers fail closed.
pub struct OwnerBearerToken(pub Option<String>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for OwnerBearerToken {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match authorization_bearer(req) {
            Ok(token) => Outcome::Success(Self(token)),
            Err(status) => Outcome::Error((status, ())),
        }
    }
}

async fn verify_service_write_credential(
    req: &Request<'_>,
    token: String,
) -> Result<WritePrincipal, Status> {
    let tokens = match req.guard::<&State<StaticAuthTokens>>().await {
        Outcome::Success(tokens) => tokens,
        _ => return Err(Status::InternalServerError),
    };

    let configured_token = tokens.write_token.as_deref();
    let client_ip = req.client_ip().map(|ip| ip.to_string()).unwrap_or_default();
    let rate_limiter = match req.guard::<&State<SharedRateLimiter>>().await {
        Outcome::Success(limiter) => Some(limiter.inner().clone()),
        _ => None,
    };
    if rate_limiter
        .as_ref()
        .is_some_and(|limiter| limiter.is_limited(&client_ip))
    {
        return Err(Status::TooManyRequests);
    }

    let matches_write_token =
        configured_token.is_some_and(|expected| constant_time_token_eq(&token, expected));
    let matches_admin_token = tokens
        .admin_token
        .as_deref()
        .is_some_and(|expected| constant_time_token_eq(&token, expected));
    if matches_write_token || matches_admin_token {
        if let Some(limiter) = &rate_limiter {
            limiter.clear_ip(&client_ip);
        }
        return Ok(WritePrincipal::StaticToken);
    }

    let store = match req.guard::<&State<SharedApiKeyStore>>().await {
        Outcome::Success(store) => store,
        _ => return Err(Status::InternalServerError),
    };
    let store = store.inner().clone();
    let verified = tokio::task::spawn_blocking(move || store.verify_key(&token))
        .await
        .unwrap_or(None);

    match verified {
        Some(key) if key.scope.can_write() => {
            if let Some(limiter) = &rate_limiter {
                limiter.clear_ip(&client_ip);
            }
            Ok(WritePrincipal::ApiKey(AuthenticatedKey {
                key_id: key.id,
                name: key.name,
                scope: key.scope,
            }))
        }
        Some(_) => {
            if let Some(limiter) = &rate_limiter {
                limiter.record_failure(&client_ip);
            }
            Err(Status::Forbidden)
        }
        None => {
            if let Some(limiter) = &rate_limiter {
                limiter.record_failure(&client_ip);
            }
            Err(Status::Unauthorized)
        }
    }
}

async fn session_identity(req: &Request<'_>, token: &str) -> Result<Option<String>, Status> {
    let sessions = match req.guard::<&State<SharedSessionStore>>().await {
        Outcome::Success(sessions) => sessions,
        _ => return Err(Status::InternalServerError),
    };
    Ok(sessions.validate(token))
}

async fn write_policy(req: &Request<'_>) -> Result<(bool, bool), Status> {
    let tokens = match req.guard::<&State<StaticAuthTokens>>().await {
        Outcome::Success(tokens) => tokens,
        _ => return Err(Status::InternalServerError),
    };
    // COPYPASTE_REQUIRE_WRITE_AUTH is the lock. A configured write token is a
    // valid credential, not an implicit lock — otherwise setting
    // COPYPASTE_AUTH_TOKEN on the public pastebin silently breaks Get link.
    let closed = tokens.require_write_auth;
    Ok((closed, tokens.allow_session_writes))
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RequireWriteAuth {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let authorization = match authorization_bearer(req) {
            Ok(token) => token,
            Err(status) => return Outcome::Error((status, ())),
        };
        let explicit_admission = match single_header_value(req, MUTATION_WRITE_TOKEN_HEADER) {
            Ok(token) => token,
            Err(status) => return Outcome::Error((status, ())),
        };
        let (closed, allow_session_writes) = match write_policy(req).await {
            Ok(policy) => policy,
            Err(status) => return Outcome::Error((status, ())),
        };

        // Authorization carries optional user identity. If it is not a valid
        // session, it may still be a legacy service credential for clients
        // that cannot yet send the dedicated admission header.
        let session_owner = match authorization.as_deref() {
            Some(token) => match session_identity(req, token).await {
                Ok(identity) => identity,
                Err(status) => return Outcome::Error((status, ())),
            },
            None => None,
        };

        let explicit_principal = match explicit_admission {
            Some(token) => match verify_service_write_credential(req, token).await {
                Ok(principal) => Some(principal),
                Err(status) => return Outcome::Error((status, ())),
            },
            None => None,
        };

        let compatibility_principal = if session_owner.is_none() {
            match authorization {
                Some(token) => match verify_service_write_credential(req, token).await {
                    Ok(principal) => Some(principal),
                    Err(status) => return Outcome::Error((status, ())),
                },
                None => None,
            }
        } else {
            None
        };

        let session_is_admitted = session_owner.is_some() && allow_session_writes;
        if closed
            && explicit_principal.is_none()
            && compatibility_principal.is_none()
            && !session_is_admitted
        {
            return Outcome::Error((Status::Unauthorized, ()));
        }

        let principal = match session_owner {
            Some(pubkey_hash) => WritePrincipal::UserSession { pubkey_hash },
            None => explicit_principal
                .or(compatibility_principal)
                .unwrap_or(WritePrincipal::Anonymous),
        };
        Outcome::Success(RequireWriteAuth(principal))
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RequireMutationWriteAuth {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let token = match single_header_value(req, MUTATION_WRITE_TOKEN_HEADER) {
            Ok(token) => token,
            Err(status) => return Outcome::Error((status, ())),
        };
        let (closed, _) = match write_policy(req).await {
            Ok(policy) => policy,
            Err(status) => return Outcome::Error((status, ())),
        };

        match token {
            Some(token) => match verify_service_write_credential(req, token).await {
                Ok(principal) => Outcome::Success(RequireMutationWriteAuth(principal)),
                Err(status) => Outcome::Error((status, ())),
            },
            None if !closed => {
                Outcome::Success(RequireMutationWriteAuth(WritePrincipal::Anonymous))
            }
            None => Outcome::Error((Status::Unauthorized, ())),
        }
    }
}

/// Required admin guard: fails (401) if no Bearer token; fails (403) if token
/// is valid but scope is not Admin; succeeds if Admin scope.
///
/// Also accepts the `COPYPASTE_ADMIN_TOKEN` env var as a bootstrap admin token.
pub struct RequireAdminAuth(pub AuthenticatedKey);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RequireAdminAuth {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let token = match authorization_bearer(req) {
            Ok(Some(token)) => token,
            Ok(None) => return Outcome::Error((Status::Unauthorized, ())),
            Err(status) => return Outcome::Error((status, ())),
        };

        let client_ip = req.client_ip().map(|ip| ip.to_string()).unwrap_or_default();

        // Rate limit check (BUG-002).
        let rl_arc = match req.guard::<&State<SharedRateLimiter>>().await {
            Outcome::Success(rl) => Some(rl.inner().clone()),
            _ => None,
        };
        if let Some(rl) = &rl_arc {
            if rl.is_limited(&client_ip) {
                return Outcome::Error((Status::TooManyRequests, ()));
            }
        }

        let tokens = match req.guard::<&State<StaticAuthTokens>>().await {
            Outcome::Success(tokens) => tokens,
            _ => return Outcome::Error((Status::InternalServerError, ())),
        };

        // Bootstrap: allow COPYPASTE_ADMIN_TOKEN as loaded at server startup.
        // Constant-time comparison prevents timing oracle (BUG-001).
        if let Some(admin_token) = tokens.admin_token.as_deref() {
            if constant_time_token_eq(&token, admin_token) {
                if let Some(rl) = &rl_arc {
                    rl.clear_ip(&client_ip);
                }
                return Outcome::Success(RequireAdminAuth(AuthenticatedKey {
                    key_id: "env-admin".to_string(),
                    name: "admin".to_string(),
                    scope: ApiScope::Admin,
                }));
            }
        }

        let store = match req.guard::<&State<SharedApiKeyStore>>().await {
            Outcome::Success(s) => s,
            _ => return Outcome::Error((Status::InternalServerError, ())),
        };

        let store_arc = store.inner().clone();
        let result = tokio::task::spawn_blocking(move || store_arc.verify_key(&token))
            .await
            .unwrap_or(None);

        match result {
            Some(key) if key.scope.is_admin() => {
                if let Some(rl) = &rl_arc {
                    rl.clear_ip(&client_ip);
                }
                Outcome::Success(RequireAdminAuth(AuthenticatedKey {
                    key_id: key.id,
                    name: key.name,
                    scope: key.scope,
                }))
            }
            Some(_) => {
                if let Some(rl) = &rl_arc {
                    rl.record_failure(&client_ip);
                }
                Outcome::Error((Status::Forbidden, ()))
            }
            None => {
                if let Some(rl) = &rl_arc {
                    rl.record_failure(&client_ip);
                }
                Outcome::Error((Status::Unauthorized, ()))
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> SqliteApiKeyStore {
        SqliteApiKeyStore::in_memory().expect("in-memory SQLite")
    }

    #[test]
    fn static_deployment_tokens_require_a_bounded_base64url_alphabet() {
        assert!(valid_static_token(&"A".repeat(43)));
        assert!(valid_static_token(&"aB0_-".repeat(20)));
        assert!(!valid_static_token(&"A".repeat(42)));
        assert!(!valid_static_token(&"A".repeat(129)));
        assert!(!valid_static_token(&format!("{}!", "A".repeat(42))));
        assert!(!valid_static_token(&format!("{}\n", "A".repeat(43))));
    }

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn secure() -> Self {
            let path = std::env::temp_dir().join(format!("copypaste-api-keys-{}", nanoid!(16)));
            fs::create_dir(&path).expect("create temporary API-key directory");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                    .expect("secure temporary API-key directory");
            }
            Self(path)
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn file_backed_keys_survive_store_restart() {
        let directory = TemporaryDirectory::secure();
        let database_path = directory.0.join("api-keys.db");

        let raw_key = {
            let store = SqliteApiKeyStore::open(&database_path).expect("open durable key store");
            let (_, raw_key) = store
                .create_key("restart-test", ApiScope::Write, None)
                .expect("create durable key");
            assert!(store.verify_key(&raw_key).is_some());
            raw_key
        };

        let reopened = SqliteApiKeyStore::open(&database_path)
            .expect("reopen durable key store after restart");
        assert!(reopened.verify_key(&raw_key).is_some());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&database_path)
                    .expect("database metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn file_backed_legacy_schema_is_migrated_before_index_creation() {
        let directory = TemporaryDirectory::secure();
        let database_path = directory.0.join("legacy-api-keys.db");
        let raw_key = "cp_legacy_persistent_key_material";
        {
            let connection = Connection::open(&database_path).expect("create legacy database");
            connection
                .execute_batch(
                    "CREATE TABLE api_keys (
                        id           TEXT PRIMARY KEY,
                        name         TEXT NOT NULL,
                        scope        TEXT NOT NULL,
                        key_hash     TEXT NOT NULL,
                        created_at   INTEGER NOT NULL,
                        last_used_at INTEGER,
                        expires_at   INTEGER
                    );",
                )
                .expect("create legacy schema");
            let key_hash = hash_key(raw_key).expect("hash legacy key");
            connection
                .execute(
                    "INSERT INTO api_keys (id, name, scope, key_hash, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params!["legacy-file", "legacy", "write", key_hash, current_ts()],
                )
                .expect("insert legacy key");
        }

        let migrated = SqliteApiKeyStore::open(&database_path).expect("migrate legacy key store");
        assert!(migrated.verify_key(raw_key).is_some());
    }

    #[test]
    fn disabled_store_rejects_dynamic_keys_without_fallback() {
        let store = SqliteApiKeyStore::disabled();
        assert!(!store.is_enabled());
        assert!(store.verify_key("cp_untrusted").is_none());
        assert!(store.create_key("disabled", ApiScope::Write, None).is_err());
        assert!(store.list_keys().is_err());
        assert!(store.revoke_key("missing").is_err());
    }

    #[test]
    fn create_and_verify_key() {
        let s = store();
        let (info, raw) = s.create_key("ci-bot", ApiScope::Write, None).unwrap();
        assert_eq!(info.name, "ci-bot");
        assert_eq!(info.scope, ApiScope::Write);
        assert!(raw.starts_with("cp_"));

        let verified = s.verify_key(&raw).expect("key should verify");
        assert_eq!(verified.id, info.id);
        assert_eq!(verified.scope, ApiScope::Write);
    }

    #[test]
    fn verify_invalid_key_returns_none() {
        let s = store();
        s.create_key("bot", ApiScope::Read, None).unwrap();
        assert!(s.verify_key("not-a-real-key").is_none());
        assert!(s.verify_key("cp_wrongkey").is_none());
    }

    #[test]
    fn revoke_key_prevents_verification() {
        let s = store();
        let (info, raw) = s.create_key("temp", ApiScope::Read, None).unwrap();
        assert!(s.verify_key(&raw).is_some());
        assert!(s.revoke_key(&info.id).unwrap());
        assert!(s.verify_key(&raw).is_none());
    }

    #[test]
    fn revoke_nonexistent_key_returns_false() {
        let s = store();
        assert!(!s.revoke_key("doesnotexist").unwrap());
    }

    #[test]
    fn list_keys_shows_all_created() {
        let s = store();
        s.create_key("key1", ApiScope::Read, None).unwrap();
        s.create_key("key2", ApiScope::Admin, None).unwrap();

        let keys = s.list_keys().unwrap();
        assert_eq!(keys.len(), 2);
        let names: Vec<&str> = keys.iter().map(|k| k.name.as_str()).collect();
        assert!(names.contains(&"key1"));
        assert!(names.contains(&"key2"));
    }

    #[test]
    fn expired_key_cannot_be_verified() {
        let s = store();
        let past = current_ts() - 3600;
        let (_, raw) = s.create_key("old", ApiScope::Write, Some(past)).unwrap();
        assert!(s.verify_key(&raw).is_none());
    }

    #[test]
    fn scope_hierarchy() {
        assert!(ApiScope::Admin.can_read());
        assert!(ApiScope::Admin.can_write());
        assert!(ApiScope::Admin.is_admin());

        assert!(ApiScope::Write.can_read());
        assert!(ApiScope::Write.can_write());
        assert!(!ApiScope::Write.is_admin());

        assert!(ApiScope::Read.can_read());
        assert!(!ApiScope::Read.can_write());
        assert!(!ApiScope::Read.is_admin());
    }

    #[test]
    fn scope_display_and_parse_roundtrip() {
        for scope in [ApiScope::Read, ApiScope::Write, ApiScope::Admin] {
            let s = scope.to_string();
            let parsed: ApiScope = s.parse().unwrap();
            assert_eq!(parsed, scope);
        }
    }

    #[test]
    fn verify_updates_last_used_at() {
        let s = store();
        let (info, raw) = s.create_key("bot", ApiScope::Write, None).unwrap();
        assert!(info.last_used_at.is_none());

        let verified = s.verify_key(&raw).unwrap();
        assert!(verified.last_used_at.is_some());
    }

    /// Verifies that keys created before the key_prefix migration (stored with
    /// key_prefix = '') can still be verified via the backward-compat fallback
    /// scan. This covers the upgrade path that the previous attempt missed.
    #[test]
    fn verify_legacy_key_backward_compat() {
        let s = store();
        let raw = "cp_legacykeyAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        s.insert_legacy_key_for_test("legacy-001", "legacy-key", ApiScope::Write, raw)
            .expect("insert legacy key");

        // Must verify successfully even though key_prefix = ''
        let verified = s
            .verify_key(raw)
            .expect("legacy key with empty prefix should verify");
        assert_eq!(verified.id, "legacy-001");
        assert_eq!(verified.scope, ApiScope::Write);
    }

    /// After migration, a new key coexists with a legacy key; each resolves correctly.
    #[test]
    fn new_and_legacy_keys_coexist() {
        let s = store();

        // Insert a legacy key (empty prefix)
        let legacy_raw = "cp_legacykeyBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
        s.insert_legacy_key_for_test("legacy-002", "old-key", ApiScope::Read, legacy_raw)
            .unwrap();

        // Create a new key (has prefix)
        let (new_info, new_raw) = s.create_key("new-key", ApiScope::Admin, None).unwrap();

        // Both must verify
        let legacy_verified = s.verify_key(legacy_raw).expect("legacy key verifies");
        assert_eq!(legacy_verified.id, "legacy-002");

        let new_verified = s.verify_key(&new_raw).expect("new key verifies");
        assert_eq!(new_verified.id, new_info.id);
    }

    // ── Rate limiter unit tests ────────────────────────────────────────────────

    #[test]
    fn rate_limiter_blocks_after_max_failures() {
        let rl = RateLimiter::new();
        let ip = "192.168.1.1";

        for _ in 0..MAX_FAILED_ATTEMPTS {
            assert!(!rl.is_limited(ip));
            rl.record_failure(ip);
        }
        assert!(rl.is_limited(ip));
    }

    #[test]
    fn rate_limiter_clears_on_success() {
        let rl = RateLimiter::new();
        let ip = "10.0.0.1";

        for _ in 0..MAX_FAILED_ATTEMPTS {
            rl.record_failure(ip);
        }
        assert!(rl.is_limited(ip));

        rl.clear_ip(ip);
        assert!(!rl.is_limited(ip));
    }

    #[test]
    fn rate_limiter_does_not_block_different_ips() {
        let rl = RateLimiter::new();
        let ip_a = "1.1.1.1";
        let ip_b = "2.2.2.2";

        for _ in 0..MAX_FAILED_ATTEMPTS {
            rl.record_failure(ip_a);
        }
        assert!(rl.is_limited(ip_a));
        assert!(!rl.is_limited(ip_b));
    }

    #[test]
    fn failed_auth_map_is_hard_capped_and_evicts_oldest() {
        let rl = RateLimiter::with_capacity(2);
        let base = Instant::now();

        rl.record_failure_at("oldest", base);
        rl.record_failure_at("newer", base + std::time::Duration::from_secs(1));
        rl.record_failure_at("newest", base + std::time::Duration::from_secs(2));

        let map = rl.fails.lock().unwrap();
        assert_eq!(map.len(), 2);
        assert!(!map.contains_key("oldest"));
        assert!(map.contains_key("newer"));
        assert!(map.contains_key("newest"));
    }

    #[test]
    fn failed_auth_map_purges_expired_before_live_eviction() {
        let rl = RateLimiter::with_capacity(2);
        let base = Instant::now();
        let live = base + std::time::Duration::from_secs(RATE_LIMIT_WINDOW_SECS + 1);

        rl.record_failure_at("expired", base);
        rl.record_failure_at("active", live);
        rl.record_failure_at("fresh", live + std::time::Duration::from_secs(1));

        let map = rl.fails.lock().unwrap();
        assert_eq!(map.len(), 2);
        assert!(!map.contains_key("expired"));
        assert!(map.contains_key("active"));
        assert!(map.contains_key("fresh"));
    }
}
