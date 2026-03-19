use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

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

// ── Rate limiter ──────────────────────────────────────────────────────────────

const MAX_FAILED_ATTEMPTS: u32 = 10;
const RATE_LIMIT_WINDOW_SECS: u64 = 300; // 5 minutes

/// Per-IP sliding-window failed-attempt counter (BUG-002).
pub struct RateLimiter {
    fails: Mutex<HashMap<String, (u32, Instant)>>,
}

pub type SharedRateLimiter = Arc<RateLimiter>;

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            fails: Mutex::new(HashMap::new()),
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
        let mut map = self.fails.lock().unwrap();
        let now = Instant::now();
        let entry = map.entry(ip.to_string()).or_insert((0, now));
        if entry.1.elapsed().as_secs() > RATE_LIMIT_WINDOW_SECS {
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
            other => Err(format!("unknown scope: {}", other)),
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
    conn: Mutex<Connection>,
}

pub type SharedApiKeyStore = Arc<SqliteApiKeyStore>;

impl SqliteApiKeyStore {
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
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
            );
            CREATE INDEX IF NOT EXISTS idx_api_keys_prefix ON api_keys(key_prefix);",
        )?;
        // Migration for existing databases: add key_prefix if absent.
        // ALTER TABLE fails silently when the column already exists.
        let _ = conn
            .execute_batch("ALTER TABLE api_keys ADD COLUMN key_prefix TEXT NOT NULL DEFAULT '';");
        // Recreate index in case it was absent on the existing table.
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_api_keys_prefix ON api_keys(key_prefix);",
        );
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

        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
            let conn = self.conn.lock().unwrap();

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
            let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO api_keys (id, name, scope, key_hash, key_prefix, created_at)
             VALUES (?1, ?2, ?3, ?4, '', ?5)",
            params![id, name, &scope.to_string(), &key_hash, now],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
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
        let auth_header = req.headers().get_one("Authorization");
        let token = match auth_header.and_then(|h| h.strip_prefix("Bearer ")) {
            None => return Outcome::Success(OptionalApiKeyAuth(None)),
            Some(t) => t.to_owned(),
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

/// Required admin guard: fails (401) if no Bearer token; fails (403) if token
/// is valid but scope is not Admin; succeeds if Admin scope.
///
/// Also accepts the `COPYPASTE_ADMIN_TOKEN` env var as a bootstrap admin token.
pub struct RequireAdminAuth(pub AuthenticatedKey);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RequireAdminAuth {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let auth_header = req.headers().get_one("Authorization");
        let token = match auth_header.and_then(|h| h.strip_prefix("Bearer ")) {
            None => return Outcome::Error((Status::Unauthorized, ())),
            Some(t) => t.to_owned(),
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

        // Bootstrap: allow COPYPASTE_ADMIN_TOKEN env var as admin token.
        // Constant-time comparison prevents timing oracle (BUG-001).
        if let Ok(admin_token) = std::env::var("COPYPASTE_ADMIN_TOKEN") {
            if !admin_token.is_empty() && token.as_bytes().ct_eq(admin_token.as_bytes()).into() {
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
}
