use std::collections::{BTreeMap, HashMap};
use std::env;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{Mutex as AsyncMutex, RwLock};
use utoipa::ToSchema;

pub mod server;

use crate::server::redis::RedisPersistenceAdapter;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, Hash, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PasteFormat {
    #[default]
    PlainText,
    Markdown,
    Code,
    Json,
    #[serde(rename = "javascript")]
    Javascript,
    #[serde(rename = "typescript")]
    Typescript,
    Python,
    Rust,
    #[serde(rename = "go")]
    Go,
    #[serde(rename = "cpp")]
    Cpp,
    Kotlin,
    Java,
    #[serde(rename = "csharp")]
    Csharp,
    #[serde(rename = "php")]
    Php,
    #[serde(rename = "ruby")]
    Ruby,
    #[serde(rename = "bash")]
    Bash,
    #[serde(rename = "yaml")]
    Yaml,
    #[serde(rename = "sql")]
    Sql,
    Swift,
    Html,
    Css,
}

impl std::fmt::Display for PasteFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PasteFormat::PlainText => "plain_text",
            PasteFormat::Markdown => "markdown",
            PasteFormat::Code => "code",
            PasteFormat::Json => "json",
            PasteFormat::Javascript => "javascript",
            PasteFormat::Typescript => "typescript",
            PasteFormat::Python => "python",
            PasteFormat::Rust => "rust",
            PasteFormat::Go => "go",
            PasteFormat::Cpp => "cpp",
            PasteFormat::Kotlin => "kotlin",
            PasteFormat::Java => "java",
            PasteFormat::Csharp => "csharp",
            PasteFormat::Php => "php",
            PasteFormat::Ruby => "ruby",
            PasteFormat::Bash => "bash",
            PasteFormat::Yaml => "yaml",
            PasteFormat::Sql => "sql",
            PasteFormat::Swift => "swift",
            PasteFormat::Html => "html",
            PasteFormat::Css => "css",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, Hash, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EncryptionAlgorithm {
    #[default]
    None,
    Aes256Gcm,
    #[serde(rename = "chacha20_poly1305")]
    ChaCha20Poly1305,
    #[serde(rename = "xchacha20_poly1305")]
    XChaCha20Poly1305,
    #[serde(rename = "kyber_hybrid_aes256_gcm")]
    KyberHybridAes256Gcm,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StoredContent {
    Plain {
        text: String,
    },
    Encrypted {
        algorithm: EncryptionAlgorithm,
        ciphertext: String,
        nonce: String,
        salt: String,
    },
    Stego {
        algorithm: EncryptionAlgorithm,
        ciphertext: String,
        nonce: String,
        salt: String,
        carrier_mime: String,
        carrier_image: String,
        payload_digest: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StoredPaste {
    pub content: StoredContent,
    pub format: PasteFormat,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub burn_after_reading: bool,
    #[serde(default)]
    pub metadata: PasteMetadata,
    pub bundle: Option<BundleMetadata>,
    pub bundle_parent: Option<String>,
    pub bundle_label: Option<String>,
    pub not_before: Option<i64>,
    pub not_after: Option<i64>,
    pub persistence: Option<PersistenceLocator>,
    pub webhook: Option<WebhookConfig>,
    /// Whether this paste is still being updated (live log sharing).
    #[serde(default)]
    pub is_live: bool,
    /// SHA-256 hash of the ownership token (only set when is_live was true at creation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_token_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StoreStats {
    pub total_pastes: usize,
    pub active_pastes: usize,
    pub expired_pastes: usize,
    pub burn_after_reading_count: usize,
    pub time_locked_count: usize,
    pub formats: Vec<FormatUsage>,
    pub encryption_usage: Vec<EncryptionUsage>,
    pub created_by_day: Vec<DailyCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FormatUsage {
    pub format: PasteFormat,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EncryptionUsage {
    pub algorithm: EncryptionAlgorithm,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DailyCount {
    pub date: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(default)]
pub struct PasteMetadata {
    pub bundle: Option<BundleMetadata>,
    pub bundle_parent: Option<String>,
    pub bundle_label: Option<String>,
    pub not_before: Option<i64>,
    pub not_after: Option<i64>,
    pub attestation: Option<AttestationRequirement>,
    pub persistence: Option<PersistenceLocator>,
    pub webhook: Option<WebhookConfig>,
    #[serde(skip_serializing_if = "crate::bool_is_false")]
    pub tor_access_only: bool,
    pub owner_pubkey_hash: Option<String>,
    pub access_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(default)]
pub struct BundleMetadata {
    pub children: Vec<BundlePointer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BundlePointer {
    pub id: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AttestationRequirement {
    Totp {
        secret: String,
        digits: u32,
        step: u64,
        #[serde(default = "default_attestation_drift")]
        allowed_drift: u32,
        #[serde(default)]
        issuer: Option<String>,
    },
    SharedSecret {
        hash: String,
    },
}

const fn default_attestation_drift() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PersistenceLocator {
    Memory,
    Vault {
        key_path: String,
    },
    S3 {
        bucket: String,
        #[serde(default)]
        prefix: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WebhookProvider {
    Slack,
    Teams,
    Generic,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(default)]
pub struct WebhookConfig {
    pub url: String,
    pub provider: Option<WebhookProvider>,
    pub view_template: Option<String>,
    pub burn_template: Option<String>,
}

#[derive(Error, Debug)]
pub enum PasteError {
    #[error("paste not found: {0}")]
    NotFound(String),
    #[error("paste expired: {0}")]
    Expired(String),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}

#[derive(Error, Debug)]
pub enum PersistenceError {
    #[error("persistence save failed for {0}: {1}")]
    Save(String, String),
    #[error("persistence load failed for {0}: {1}")]
    Load(String, String),
    #[error("persistence delete failed for {0}: {1}")]
    Delete(String, String),
}

#[derive(Error, Debug)]
pub enum PasteMutationError {
    #[error("paste not found: {0}")]
    NotFound(String),
    #[error("paste expired: {0}")]
    Expired(String),
    #[error("paste finalized: {0}")]
    Finalized(String),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}

#[async_trait]
pub trait PasteStore: Send + Sync + 'static {
    async fn create_paste(&self, paste: StoredPaste) -> Result<String, PersistenceError>;
    async fn get_paste(&self, id: &str) -> Result<StoredPaste, PasteError>;
    async fn delete_paste(&self, id: &str) -> Result<bool, PersistenceError>;
    async fn get_all_paste_ids(&self) -> Vec<String>;
    async fn stats(&self) -> StoreStats;
    /// Replace the content of a live paste (requires ownership token verification at handler level).
    async fn update_paste(
        &self,
        id: &str,
        content: StoredContent,
    ) -> Result<(), PasteMutationError>;
    /// Mark a live paste as finalized (no longer live).
    async fn finalize_paste(&self, id: &str) -> Result<(), PasteMutationError>;
}

#[async_trait]
pub trait PersistenceAdapter: Send + Sync + 'static {
    async fn save(&self, id: &str, paste: &StoredPaste) -> Result<(), PersistenceError>;
    async fn load(&self, id: &str) -> Result<Option<StoredPaste>, PersistenceError>;
    async fn delete(&self, id: &str) -> Result<(), PersistenceError>;
}

pub struct NoopPersistence;

#[async_trait]
impl PersistenceAdapter for NoopPersistence {
    async fn save(&self, _id: &str, _paste: &StoredPaste) -> Result<(), PersistenceError> {
        Ok(())
    }

    async fn load(&self, _id: &str) -> Result<Option<StoredPaste>, PersistenceError> {
        Ok(None)
    }

    async fn delete(&self, _id: &str) -> Result<(), PersistenceError> {
        Ok(())
    }
}

struct StatsCache {
    stats: StoreStats,
    computed_at: Instant,
}

/// TTL for the stats cache used by `MemoryPasteStore::stats()`.
///
/// Health-check endpoints call `stats()` on every request.  Recomputing the full
/// O(N) scan each time holds the entries read-lock for the duration and blocks
/// writers (paste creation/deletion).  Caching with a short TTL keeps the endpoint
/// responsive while bounding staleness to an acceptable window.
const STATS_CACHE_TTL: Duration = Duration::from_secs(5);

pub struct MemoryPasteStore {
    entries: RwLock<HashMap<String, StoredPaste>>,
    persistence: Option<Arc<dyn PersistenceAdapter>>,
    stats_cache: Mutex<Option<StatsCache>>,
    // Mutations and persistence cache fills are serialized per paste ID within
    // this store instance. Persistent multi-instance deployments still need a
    // distributed CAS/tombstone for global ordering; incident takedowns must
    // quarantine the ID and roll every instance. The weak map does not retain
    // locks after the last operation on an ID ends.
    operation_locks: Mutex<HashMap<String, Weak<AsyncMutex<()>>>>,
}

impl MemoryPasteStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            persistence: None,
            stats_cache: Mutex::new(None),
            operation_locks: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_persistence(adapter: Arc<dyn PersistenceAdapter>) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            persistence: Some(adapter),
            stats_cache: Mutex::new(None),
            operation_locks: Mutex::new(HashMap::new()),
        }
    }

    fn operation_lock(&self, id: &str) -> Arc<AsyncMutex<()>> {
        let mut locks = self.operation_locks.lock().unwrap();
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(id).and_then(Weak::upgrade) {
            return lock;
        }

        let lock = Arc::new(AsyncMutex::new(()));
        locks.insert(id.to_string(), Arc::downgrade(&lock));
        lock
    }
}

impl Default for MemoryPasteStore {
    fn default() -> Self {
        Self::new()
    }
}

fn is_expired(paste: &StoredPaste) -> bool {
    if let Some(expires_at) = paste.expires_at {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or_default();
        now >= expires_at
    } else {
        false
    }
}

pub(crate) fn bool_is_false(value: &bool) -> bool {
    !*value
}

fn generate_paste_id(map: &HashMap<String, StoredPaste>) -> String {
    loop {
        // nanoid's default URL-safe alphabet contains 64 symbols. At 24
        // characters this carries 144 bits of CSPRNG-backed entropy, making
        // paste identifiers infeasible to enumerate.
        let candidate = nanoid!(24);
        if !map.contains_key(&candidate) {
            return candidate;
        }
    }
}

#[async_trait]
impl PasteStore for MemoryPasteStore {
    async fn create_paste(&self, paste: StoredPaste) -> Result<String, PersistenceError> {
        loop {
            let id = {
                let map = self.entries.read().await;
                generate_paste_id(&map)
            };
            let operation_lock = self.operation_lock(&id);
            let _operation = operation_lock.lock().await;

            // An astronomically unlikely random collision may have been
            // inserted while this task waited for the per-ID lock.
            if self.entries.read().await.contains_key(&id) {
                continue;
            }

            if let Some(adapter) = &self.persistence {
                // Never acknowledge a paste that the configured durable
                // backend failed to store. Only the per-ID async operation
                // lock, never the entries map lock, spans adapter I/O.
                adapter.save(&id, &paste).await?;
            }
            self.entries.write().await.insert(id.clone(), paste);
            return Ok(id);
        }
    }

    async fn get_paste(&self, id: &str) -> Result<StoredPaste, PasteError> {
        {
            let map = self.entries.read().await;
            if let Some(paste) = map.get(id).filter(|paste| !is_expired(paste)) {
                return Ok(paste.clone());
            }
        }

        let operation_lock = self.operation_lock(id);
        let _operation = operation_lock.lock().await;

        // Recheck after waiting: another cache fill or mutation may have
        // completed while this operation was queued.
        {
            let mut map = self.entries.write().await;
            match map.get(id) {
                Some(paste) if !is_expired(paste) => return Ok(paste.clone()),
                Some(_) => {
                    map.remove(id);
                    return Err(PasteError::Expired(id.to_string()));
                }
                None => {}
            }
        }

        let Some(adapter) = &self.persistence else {
            return Err(PasteError::NotFound(id.to_string()));
        };
        match adapter.load(id).await? {
            Some(paste) if !is_expired(&paste) => {
                self.entries
                    .write()
                    .await
                    .insert(id.to_string(), paste.clone());
                Ok(paste)
            }
            Some(_) => Err(PasteError::Expired(id.to_string())),
            None => Err(PasteError::NotFound(id.to_string())),
        }
    }

    async fn delete_paste(&self, id: &str) -> Result<bool, PersistenceError> {
        let operation_lock = self.operation_lock(id);
        let _operation = operation_lock.lock().await;
        // Delete durable state first. If that fails, retain the in-memory copy
        // and propagate the error so callers never claim a takedown succeeded
        // while the paste can reappear after a restart. Per-ID serialization
        // within this process prevents an earlier local update save from
        // completing after this delete.
        if let Some(adapter) = &self.persistence {
            adapter.delete(id).await?;
        }
        let existed = self.entries.write().await.remove(id).is_some();
        Ok(existed)
    }

    async fn stats(&self) -> StoreStats {
        // Return cached result if still within TTL (O(1) fast path).
        {
            let cache = self.stats_cache.lock().unwrap();
            if let Some(ref cached) = *cache {
                if cached.computed_at.elapsed() < STATS_CACHE_TTL {
                    return cached.stats.clone();
                }
            }
        }

        // Cache miss or expired: perform the full O(N) scan.
        let stats = {
            let map = self.entries.read().await;
            let mut total = 0usize;
            let mut active = 0usize;
            let mut expired = 0usize;
            let mut burn_count = 0usize;
            let mut time_locked = 0usize;
            let mut format_counts: HashMap<PasteFormat, usize> = HashMap::new();
            let mut encryption_counts: HashMap<EncryptionAlgorithm, usize> = HashMap::new();
            let mut daily_counts: BTreeMap<String, usize> = BTreeMap::new();

            for paste in map.values() {
                total += 1;
                let paste_expired = is_expired(paste);
                if paste_expired {
                    expired += 1;
                } else {
                    active += 1;
                }

                if paste.burn_after_reading {
                    burn_count += 1;
                }

                if paste.metadata.not_before.is_some() || paste.metadata.not_after.is_some() {
                    time_locked += 1;
                }

                *format_counts.entry(paste.format).or_default() += 1;

                let algorithm = match &paste.content {
                    StoredContent::Plain { .. } => EncryptionAlgorithm::None,
                    StoredContent::Encrypted { algorithm, .. }
                    | StoredContent::Stego { algorithm, .. } => *algorithm,
                };
                *encryption_counts.entry(algorithm).or_default() += 1;

                if let Some(dt) = DateTime::<Utc>::from_timestamp(paste.created_at, 0) {
                    let date = dt.date_naive().format("%Y-%m-%d").to_string();
                    *daily_counts.entry(date).or_default() += 1;
                }
            }

            StoreStats {
                total_pastes: total,
                active_pastes: active,
                expired_pastes: expired,
                burn_after_reading_count: burn_count,
                time_locked_count: time_locked,
                formats: format_counts
                    .into_iter()
                    .map(|(format, count)| FormatUsage { format, count })
                    .collect(),
                encryption_usage: encryption_counts
                    .into_iter()
                    .map(|(algorithm, count)| EncryptionUsage { algorithm, count })
                    .collect(),
                created_by_day: daily_counts
                    .into_iter()
                    .map(|(date, count)| DailyCount { date, count })
                    .collect(),
            }
        };

        // Store in cache for subsequent requests within the TTL window.
        *self.stats_cache.lock().unwrap() = Some(StatsCache {
            stats: stats.clone(),
            computed_at: Instant::now(),
        });

        stats
    }

    async fn get_all_paste_ids(&self) -> Vec<String> {
        let map = self.entries.read().await;
        map.keys().cloned().collect()
    }

    async fn update_paste(
        &self,
        id: &str,
        content: StoredContent,
    ) -> Result<(), PasteMutationError> {
        let operation_lock = self.operation_lock(id);
        let _operation = operation_lock.lock().await;
        // Work on a clone so a failed durable write cannot partially mutate
        // the read cache. Never retain an entries map lock across adapter I/O.
        let updated = {
            let map = self.entries.read().await;
            match map.get(id) {
                Some(paste) if is_expired(paste) => {
                    return Err(PasteMutationError::Expired(id.to_string()));
                }
                Some(paste) if !paste.is_live => {
                    return Err(PasteMutationError::Finalized(id.to_string()));
                }
                Some(paste) => {
                    let mut updated = paste.clone();
                    updated.content = content;
                    updated
                }
                None => return Err(PasteMutationError::NotFound(id.to_string())),
            }
        };

        if let Some(adapter) = &self.persistence {
            adapter.save(id, &updated).await?;
        }

        let mut map = self.entries.write().await;
        match map.get_mut(id) {
            Some(paste) if !is_expired(paste) => {
                *paste = updated;
                Ok(())
            }
            Some(_) => Err(PasteMutationError::Expired(id.to_string())),
            None => Err(PasteMutationError::NotFound(id.to_string())),
        }
    }

    async fn finalize_paste(&self, id: &str) -> Result<(), PasteMutationError> {
        let operation_lock = self.operation_lock(id);
        let _operation = operation_lock.lock().await;
        // As with content updates, persist a complete modified snapshot before
        // publishing it to the in-memory read cache.
        let finalized = {
            let map = self.entries.read().await;
            match map.get(id) {
                Some(paste) if !is_expired(paste) => {
                    let mut finalized = paste.clone();
                    finalized.is_live = false;
                    finalized
                }
                Some(_) => return Err(PasteMutationError::Expired(id.to_string())),
                None => return Err(PasteMutationError::NotFound(id.to_string())),
            }
        };

        if let Some(adapter) = &self.persistence {
            adapter.save(id, &finalized).await?;
        }

        let mut map = self.entries.write().await;
        match map.get_mut(id) {
            Some(paste) if !is_expired(paste) => {
                *paste = finalized;
                Ok(())
            }
            Some(_) => Err(PasteMutationError::Expired(id.to_string())),
            None => Err(PasteMutationError::NotFound(id.to_string())),
        }
    }
}

pub type SharedPasteStore = Arc<dyn PasteStore>;

#[derive(Error, Debug)]
pub enum PasteStoreInitError {
    #[error("failed to initialize configured {backend} persistence: {reason}")]
    ConfiguredBackend {
        backend: &'static str,
        reason: String,
    },
    #[error("unsupported persistence backend: {0}")]
    UnsupportedBackend(String),
    #[error("COPYPASTE_PERSISTENCE_BACKEND must contain valid Unicode")]
    InvalidBackendValue,
}

pub fn create_paste_store() -> Result<SharedPasteStore, PasteStoreInitError> {
    let configured = match env::var("COPYPASTE_PERSISTENCE_BACKEND") {
        Ok(configured) => configured,
        Err(env::VarError::NotPresent) => "memory".to_string(),
        Err(env::VarError::NotUnicode(_)) => return Err(PasteStoreInitError::InvalidBackendValue),
    };
    match configured.trim().to_ascii_lowercase().as_str() {
        "vault" => vault::VaultPersistenceAdapter::from_env()
            .map(|adapter| {
                Arc::new(MemoryPasteStore::with_persistence(adapter)) as SharedPasteStore
            })
            .map_err(|reason| PasteStoreInitError::ConfiguredBackend {
                backend: "vault",
                reason,
            }),
        "redis" => RedisPersistenceAdapter::from_env()
            .map(|adapter| {
                Arc::new(MemoryPasteStore::with_persistence(adapter)) as SharedPasteStore
            })
            .map_err(|reason| PasteStoreInitError::ConfiguredBackend {
                backend: "redis",
                reason,
            }),
        "memory" | "" => Ok(Arc::new(MemoryPasteStore::new())),
        _ => Err(PasteStoreInitError::UnsupportedBackend(configured)),
    }
}

pub mod vault {
    use super::PersistenceAdapter;
    use std::sync::Arc;

    pub struct VaultPersistenceAdapter;

    impl VaultPersistenceAdapter {
        pub fn from_env() -> Result<Arc<dyn PersistenceAdapter>, String> {
            Err("Vault persistence is disabled in this hardened build".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use once_cell::sync::Lazy;
    use std::collections::{HashMap, VecDeque};
    use std::ffi::OsString;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use tokio::sync::{oneshot, Notify};

    static STORE_ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    struct EnvSnapshot(Vec<(&'static str, Option<OsString>)>);

    impl EnvSnapshot {
        fn capture(names: &[&'static str]) -> Self {
            Self(
                names
                    .iter()
                    .map(|name| (*name, std::env::var_os(name)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (name, value) in self.0.drain(..) {
                if let Some(value) = value {
                    std::env::set_var(name, value);
                } else {
                    std::env::remove_var(name);
                }
            }
        }
    }

    #[derive(Default)]
    struct RecordingAdapter {
        saved: Mutex<Vec<String>>,
        last_saved_paste: Mutex<Option<(String, StoredPaste)>>,
        deleted: Mutex<Vec<String>>,
        load_queue: Mutex<VecDeque<Result<Option<StoredPaste>, PersistenceError>>>,
        save_error: Mutex<Option<String>>,
        delete_error: Mutex<Option<String>>,
    }

    impl RecordingAdapter {
        fn with_load_results(results: Vec<Result<Option<StoredPaste>, PersistenceError>>) -> Self {
            Self {
                saved: Mutex::new(Vec::new()),
                last_saved_paste: Mutex::new(None),
                deleted: Mutex::new(Vec::new()),
                load_queue: Mutex::new(results.into_iter().collect()),
                save_error: Mutex::new(None),
                delete_error: Mutex::new(None),
            }
        }

        fn push_load_result(&self, result: Result<Option<StoredPaste>, PersistenceError>) {
            self.load_queue.lock().unwrap().push_back(result);
        }

        fn take_deleted(&self) -> Vec<String> {
            std::mem::take(&mut *self.deleted.lock().unwrap())
        }

        fn take_saved(&self) -> Vec<String> {
            std::mem::take(&mut *self.saved.lock().unwrap())
        }

        fn last_saved_paste(&self) -> Option<(String, StoredPaste)> {
            self.last_saved_paste.lock().unwrap().clone()
        }

        fn fail_next_delete(&self, message: &str) {
            *self.delete_error.lock().unwrap() = Some(message.to_string());
        }

        fn fail_next_save(&self, message: &str) {
            *self.save_error.lock().unwrap() = Some(message.to_string());
        }
    }

    #[async_trait]
    impl PersistenceAdapter for RecordingAdapter {
        async fn save(&self, id: &str, paste: &StoredPaste) -> Result<(), PersistenceError> {
            self.saved.lock().unwrap().push(id.to_string());
            *self.last_saved_paste.lock().unwrap() = Some((id.to_string(), paste.clone()));
            if let Some(message) = self.save_error.lock().unwrap().take() {
                return Err(PersistenceError::Save(id.to_string(), message));
            }
            Ok(())
        }

        async fn load(&self, id: &str) -> Result<Option<StoredPaste>, PersistenceError> {
            let result = self
                .load_queue
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Ok(None));
            match result {
                Ok(opt) => Ok(opt),
                Err(err) => Err(match err {
                    PersistenceError::Save(_, msg) => PersistenceError::Load(id.to_string(), msg),
                    PersistenceError::Load(_, msg) => PersistenceError::Load(id.to_string(), msg),
                    PersistenceError::Delete(_, msg) => PersistenceError::Load(id.to_string(), msg),
                }),
            }
        }

        async fn delete(&self, id: &str) -> Result<(), PersistenceError> {
            self.deleted.lock().unwrap().push(id.to_string());
            if let Some(message) = self.delete_error.lock().unwrap().take() {
                return Err(PersistenceError::Delete(id.to_string(), message));
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct CoordinatedAdapter {
        records: Mutex<HashMap<String, StoredPaste>>,
        block_next_save: AtomicBool,
        save_started: Notify,
        release_save: Notify,
    }

    impl CoordinatedAdapter {
        fn block_next_save(&self) {
            self.block_next_save.store(true, Ordering::SeqCst);
        }

        async fn wait_for_blocked_save(&self) {
            self.save_started.notified().await;
        }

        fn release_blocked_save(&self) {
            self.release_save.notify_one();
        }

        fn contains(&self, id: &str) -> bool {
            self.records.lock().unwrap().contains_key(id)
        }

        fn record(&self, id: &str) -> Option<StoredPaste> {
            self.records.lock().unwrap().get(id).cloned()
        }
    }

    #[async_trait]
    impl PersistenceAdapter for CoordinatedAdapter {
        async fn save(&self, id: &str, paste: &StoredPaste) -> Result<(), PersistenceError> {
            if self.block_next_save.swap(false, Ordering::SeqCst) {
                self.save_started.notify_one();
                self.release_save.notified().await;
            }
            self.records
                .lock()
                .unwrap()
                .insert(id.to_string(), paste.clone());
            Ok(())
        }

        async fn load(&self, id: &str) -> Result<Option<StoredPaste>, PersistenceError> {
            Ok(self.records.lock().unwrap().get(id).cloned())
        }

        async fn delete(&self, id: &str) -> Result<(), PersistenceError> {
            self.records.lock().unwrap().remove(id);
            Ok(())
        }
    }

    fn build_paste(content: StoredContent) -> StoredPaste {
        StoredPaste {
            content,
            format: PasteFormat::PlainText,
            created_at: 1_700_000_000,
            expires_at: None,
            burn_after_reading: false,
            bundle: None,
            bundle_parent: None,
            bundle_label: None,
            not_before: None,
            not_after: None,
            persistence: None,
            webhook: None,
            metadata: PasteMetadata::default(),
            is_live: false,
            owner_token_hash: None,
        }
    }

    #[test]
    fn unconfigured_or_explicit_memory_backend_initializes_in_memory_store() {
        let _lock = STORE_ENV_LOCK.lock().unwrap();
        let _env = EnvSnapshot::capture(&["COPYPASTE_PERSISTENCE_BACKEND"]);

        std::env::remove_var("COPYPASTE_PERSISTENCE_BACKEND");
        assert!(create_paste_store().is_ok());

        std::env::set_var("COPYPASTE_PERSISTENCE_BACKEND", "memory");
        assert!(create_paste_store().is_ok());
    }

    #[test]
    fn explicitly_configured_persistence_never_falls_back_to_memory() {
        let _lock = STORE_ENV_LOCK.lock().unwrap();
        let _env = EnvSnapshot::capture(&[
            "COPYPASTE_PERSISTENCE_BACKEND",
            "UPSTASH_REDIS_REST_URL",
            "UPSTASH_REDIS_REST_TOKEN",
            "COPYPASTE_VAULT_ADDR",
            "COPYPASTE_VAULT_TOKEN",
        ]);

        std::env::remove_var("UPSTASH_REDIS_REST_URL");
        std::env::remove_var("UPSTASH_REDIS_REST_TOKEN");
        std::env::set_var("COPYPASTE_PERSISTENCE_BACKEND", "redis");
        assert!(matches!(
            create_paste_store(),
            Err(PasteStoreInitError::ConfiguredBackend {
                backend: "redis",
                ..
            })
        ));

        std::env::remove_var("COPYPASTE_VAULT_ADDR");
        std::env::remove_var("COPYPASTE_VAULT_TOKEN");
        std::env::set_var("COPYPASTE_PERSISTENCE_BACKEND", "vault");
        match create_paste_store() {
            Err(PasteStoreInitError::ConfiguredBackend { backend, reason }) => {
                assert_eq!(backend, "vault");
                assert_eq!(
                    reason,
                    "Vault persistence is disabled in this hardened build"
                );
            }
            _ => panic!("configured Vault backend must fail closed"),
        }
    }

    #[test]
    fn unknown_persistence_backend_fails_closed() {
        let _lock = STORE_ENV_LOCK.lock().unwrap();
        let _env = EnvSnapshot::capture(&["COPYPASTE_PERSISTENCE_BACKEND"]);
        std::env::set_var("COPYPASTE_PERSISTENCE_BACKEND", "typo-backend");

        assert!(matches!(
            create_paste_store(),
            Err(PasteStoreInitError::UnsupportedBackend(value)) if value == "typo-backend"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_persistence_backend_fails_closed() {
        use std::os::unix::ffi::OsStringExt;

        let _lock = STORE_ENV_LOCK.lock().unwrap();
        let _env = EnvSnapshot::capture(&["COPYPASTE_PERSISTENCE_BACKEND"]);
        std::env::set_var(
            "COPYPASTE_PERSISTENCE_BACKEND",
            OsString::from_vec(vec![0xff]),
        );

        assert!(matches!(
            create_paste_store(),
            Err(PasteStoreInitError::InvalidBackendValue)
        ));
    }

    #[tokio::test]
    async fn creates_and_reads_plain_paste() {
        let store = MemoryPasteStore::default();
        let metadata = PasteMetadata::default();
        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "hello world".into(),
            },
            format: PasteFormat::Markdown,
            created_at: 1234,
            expires_at: None,
            burn_after_reading: false,
            bundle: metadata.bundle.clone(),
            bundle_parent: metadata.bundle_parent.clone(),
            bundle_label: metadata.bundle_label.clone(),
            not_before: metadata.not_before,
            not_after: metadata.not_after,
            persistence: metadata.persistence.clone(),
            webhook: metadata.webhook.clone(),
            metadata,
            is_live: false,
            owner_token_hash: None,
        };

        let id = store.create_paste(paste).await.expect("create paste");
        let stored = store.get_paste(&id).await.expect("paste should exist");

        match stored.content {
            StoredContent::Plain { ref text } => assert_eq!(text, "hello world"),
            _ => panic!("unexpected content variant"),
        }
    }

    #[tokio::test]
    async fn expired_paste_is_removed() {
        let store = MemoryPasteStore::default();
        let metadata = PasteMetadata::default();
        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "stale".into(),
            },
            format: PasteFormat::PlainText,
            created_at: 100,
            expires_at: Some(50),
            burn_after_reading: false,
            bundle: metadata.bundle.clone(),
            bundle_parent: metadata.bundle_parent.clone(),
            bundle_label: metadata.bundle_label.clone(),
            not_before: metadata.not_before,
            not_after: metadata.not_after,
            persistence: metadata.persistence.clone(),
            webhook: metadata.webhook.clone(),
            metadata,
            is_live: false,
            owner_token_hash: None,
        };

        let id = store.create_paste(paste).await.expect("create paste");
        let result = store.get_paste(&id).await;

        assert!(matches!(result, Err(PasteError::Expired(_))));
        assert!(matches!(
            store.get_paste(&id).await,
            Err(PasteError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn stores_encrypted_content() {
        let store = MemoryPasteStore::default();
        let metadata = PasteMetadata::default();
        let paste = StoredPaste {
            content: StoredContent::Encrypted {
                algorithm: EncryptionAlgorithm::Aes256Gcm,
                ciphertext: "abc".into(),
                nonce: "nonce".into(),
                salt: "salt".into(),
            },
            format: PasteFormat::Code,
            created_at: 0,
            expires_at: None,
            burn_after_reading: false,
            bundle: metadata.bundle.clone(),
            bundle_parent: metadata.bundle_parent.clone(),
            bundle_label: metadata.bundle_label.clone(),
            not_before: metadata.not_before,
            not_after: metadata.not_after,
            persistence: metadata.persistence.clone(),
            webhook: metadata.webhook.clone(),
            metadata,
            is_live: false,
            owner_token_hash: None,
        };

        let id = store.create_paste(paste).await.expect("create paste");
        let stored = store.get_paste(&id).await.expect("paste should exist");
        assert!(matches!(stored.content, StoredContent::Encrypted { .. }));
    }

    #[tokio::test]
    async fn delete_paste_invokes_persistence_adapter() {
        let adapter = Arc::new(RecordingAdapter::default());
        let store = MemoryPasteStore::with_persistence(adapter.clone());
        let paste = build_paste(StoredContent::Plain {
            text: "tracked".into(),
        });

        let id = store.create_paste(paste).await.expect("create paste");
        assert!(store.delete_paste(&id).await.unwrap());
        assert_eq!(adapter.take_deleted(), vec![id.clone()]);

        // Second deletion still triggers adapter delete but reports false
        assert!(!store.delete_paste(&id).await.unwrap());
        assert_eq!(adapter.take_deleted(), vec![id.clone()]);
        assert_eq!(adapter.take_saved(), vec![id]);
    }

    #[tokio::test]
    async fn failed_persistent_create_is_reported_and_not_cached() {
        let adapter = Arc::new(RecordingAdapter::default());
        let store = MemoryPasteStore::with_persistence(adapter.clone());
        adapter.fail_next_save("backend unavailable");

        let result = store
            .create_paste(build_paste(StoredContent::Plain {
                text: "must not be acknowledged".into(),
            }))
            .await;

        assert!(matches!(result, Err(PersistenceError::Save(_, _))));
        assert!(store.get_all_paste_ids().await.is_empty());
    }

    #[tokio::test]
    async fn failed_persistent_delete_is_reported_and_keeps_memory_copy() {
        let adapter = Arc::new(RecordingAdapter::default());
        let store = MemoryPasteStore::with_persistence(adapter.clone());
        let paste = build_paste(StoredContent::Plain {
            text: "must remain until durable deletion succeeds".into(),
        });
        let id = store.create_paste(paste).await.expect("create paste");
        adapter.fail_next_delete("backend unavailable");

        assert!(matches!(
            store.delete_paste(&id).await,
            Err(PersistenceError::Delete(_, _))
        ));
        assert!(store.get_paste(&id).await.is_ok());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_waits_for_local_inflight_update_and_cannot_be_resurrected() {
        let adapter = Arc::new(CoordinatedAdapter::default());
        let store = Arc::new(MemoryPasteStore::with_persistence(adapter.clone()));
        let mut paste = build_paste(StoredContent::Plain {
            text: "original".into(),
        });
        paste.is_live = true;
        let id = store.create_paste(paste).await.expect("create paste");

        adapter.block_next_save();
        let update_store = store.clone();
        let update_id = id.clone();
        let update = tokio::spawn(async move {
            update_store
                .update_paste(
                    &update_id,
                    StoredContent::Plain {
                        text: "inflight update".into(),
                    },
                )
                .await
        });
        adapter.wait_for_blocked_save().await;

        let delete_store = store.clone();
        let delete_id = id.clone();
        let (delete_started_tx, delete_started_rx) = oneshot::channel();
        let delete = tokio::spawn(async move {
            let _ = delete_started_tx.send(());
            delete_store.delete_paste(&delete_id).await
        });
        delete_started_rx.await.expect("delete task started");
        // On the single-threaded scheduler, the delete task now either waits
        // on the ID lock (fixed behavior) or has already raced ahead.
        tokio::task::yield_now().await;

        adapter.release_blocked_save();
        assert!(update.await.expect("update task").is_ok());
        assert!(matches!(delete.await.expect("delete task"), Ok(true)));
        assert!(!adapter.contains(&id), "durable record was resurrected");
        assert!(matches!(
            store.get_paste(&id).await,
            Err(PasteError::NotFound(_))
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn finalize_orders_after_local_inflight_update_and_cannot_be_reopened() {
        let adapter = Arc::new(CoordinatedAdapter::default());
        let store = Arc::new(MemoryPasteStore::with_persistence(adapter.clone()));
        let mut paste = build_paste(StoredContent::Plain {
            text: "original".into(),
        });
        paste.is_live = true;
        let id = store.create_paste(paste).await.expect("create paste");

        adapter.block_next_save();
        let update_store = store.clone();
        let update_id = id.clone();
        let update = tokio::spawn(async move {
            update_store
                .update_paste(
                    &update_id,
                    StoredContent::Plain {
                        text: "final content".into(),
                    },
                )
                .await
        });
        adapter.wait_for_blocked_save().await;

        let finalize_store = store.clone();
        let finalize_id = id.clone();
        let (finalize_started_tx, finalize_started_rx) = oneshot::channel();
        let finalize = tokio::spawn(async move {
            let _ = finalize_started_tx.send(());
            finalize_store.finalize_paste(&finalize_id).await
        });
        finalize_started_rx.await.expect("finalize task started");
        tokio::task::yield_now().await;

        adapter.release_blocked_save();
        assert!(update.await.expect("update task").is_ok());
        assert!(finalize.await.expect("finalize task").is_ok());

        let cached = store.get_paste(&id).await.expect("cached record");
        let durable = adapter.record(&id).expect("durable record");
        assert!(!cached.is_live);
        assert!(!durable.is_live);
        assert!(matches!(
            durable.content,
            StoredContent::Plain { ref text } if text == "final content"
        ));
    }

    #[tokio::test]
    async fn generated_ids_have_at_least_128_bits_of_entropy() {
        let store = MemoryPasteStore::default();
        let id = store
            .create_paste(build_paste(StoredContent::Plain { text: "id".into() }))
            .await
            .expect("create paste");

        assert_eq!(id.len(), 24);
        assert!(id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')));
    }

    #[tokio::test]
    async fn get_paste_uses_persistence_fallback() {
        let adapter = Arc::new(RecordingAdapter::default());
        let store = MemoryPasteStore::with_persistence(adapter.clone());

        let paste = build_paste(StoredContent::Plain {
            text: "persisted".into(),
        });
        adapter.push_load_result(Ok(Some(paste.clone())));

        let fetched = store
            .get_paste("persisted-id")
            .await
            .expect("should load from persistence");
        assert!(matches!(
            fetched.content,
            StoredContent::Plain { ref text } if text == "persisted"
        ));

        // Subsequent call is served from in-memory cache
        let again = store
            .get_paste("persisted-id")
            .await
            .expect("should still be present");
        assert!(matches!(again.content, StoredContent::Plain { .. }));
    }

    #[tokio::test]
    async fn get_paste_reports_expired_from_persistence() {
        let adapter = Arc::new(RecordingAdapter::default());
        let store = MemoryPasteStore::with_persistence(adapter.clone());

        let mut expired = build_paste(StoredContent::Plain { text: "old".into() });
        expired.expires_at = Some(0);
        adapter.push_load_result(Ok(Some(expired)));

        let err = store
            .get_paste("old-id")
            .await
            .expect_err("should be expired");
        assert!(matches!(err, PasteError::Expired(id) if id == "old-id"));
    }

    #[tokio::test]
    async fn get_paste_surfaces_adapter_error_as_unavailable() {
        let adapter = Arc::new(RecordingAdapter::with_load_results(vec![Err(
            PersistenceError::Load("err".into(), "boom".into()),
        )]));
        let store = MemoryPasteStore::with_persistence(adapter);

        let err = store
            .get_paste("missing-id")
            .await
            .expect_err("adapter error should remain distinguishable");
        assert!(matches!(
            err,
            PasteError::Persistence(PersistenceError::Load(id, _)) if id == "missing-id"
        ));
    }

    #[tokio::test]
    async fn update_paste_replaces_content() {
        let store = MemoryPasteStore::default();
        let mut paste = build_paste(StoredContent::Plain {
            text: "original".into(),
        });
        paste.is_live = true;
        let id = store.create_paste(paste).await.expect("create paste");

        store
            .update_paste(
                &id,
                StoredContent::Plain {
                    text: "updated".into(),
                },
            )
            .await
            .expect("update should succeed");

        let fetched = store.get_paste(&id).await.expect("paste should exist");
        match fetched.content {
            StoredContent::Plain { text } => assert_eq!(text, "updated"),
            _ => panic!("unexpected content variant"),
        }
    }

    #[tokio::test]
    async fn update_paste_persists_modified_record_before_cache_update() {
        let adapter = Arc::new(RecordingAdapter::default());
        let store = MemoryPasteStore::with_persistence(adapter.clone());
        let mut paste = build_paste(StoredContent::Plain {
            text: "original".into(),
        });
        paste.is_live = true;
        let id = store.create_paste(paste).await.expect("create paste");

        store
            .update_paste(
                &id,
                StoredContent::Plain {
                    text: "durable update".into(),
                },
            )
            .await
            .expect("update paste");

        let (saved_id, saved) = adapter.last_saved_paste().expect("saved snapshot");
        assert_eq!(saved_id, id);
        assert!(matches!(
            saved.content,
            StoredContent::Plain { ref text } if text == "durable update"
        ));
        let cached = store.get_paste(&id).await.expect("cached paste");
        assert!(matches!(
            cached.content,
            StoredContent::Plain { ref text } if text == "durable update"
        ));
    }

    #[tokio::test]
    async fn failed_persistent_update_is_reported_and_leaves_cache_unchanged() {
        let adapter = Arc::new(RecordingAdapter::default());
        let store = MemoryPasteStore::with_persistence(adapter.clone());
        let mut paste = build_paste(StoredContent::Plain {
            text: "original".into(),
        });
        paste.is_live = true;
        let id = store.create_paste(paste).await.expect("create paste");
        adapter.fail_next_save("backend unavailable");

        let result = store
            .update_paste(
                &id,
                StoredContent::Plain {
                    text: "must not enter cache".into(),
                },
            )
            .await;

        assert!(matches!(
            result,
            Err(PasteMutationError::Persistence(PersistenceError::Save(
                _,
                _
            )))
        ));
        let cached = store.get_paste(&id).await.expect("original cached paste");
        assert!(matches!(
            cached.content,
            StoredContent::Plain { ref text } if text == "original"
        ));
    }

    #[tokio::test]
    async fn update_paste_not_found_returns_error() {
        let store = MemoryPasteStore::default();
        let err = store
            .update_paste("nonexistent", StoredContent::Plain { text: "x".into() })
            .await
            .expect_err("should fail");
        assert!(matches!(err, PasteMutationError::NotFound(_)));
    }

    #[tokio::test]
    async fn update_paste_rejects_finalized_record_inside_store_boundary() {
        let store = MemoryPasteStore::default();
        let id = store
            .create_paste(build_paste(StoredContent::Plain {
                text: "final".into(),
            }))
            .await
            .expect("create paste");

        let error = store
            .update_paste(
                &id,
                StoredContent::Plain {
                    text: "must not reopen".into(),
                },
            )
            .await
            .expect_err("finalized paste must reject updates");
        assert!(matches!(error, PasteMutationError::Finalized(_)));
    }

    #[tokio::test]
    async fn finalize_paste_clears_is_live() {
        let store = MemoryPasteStore::default();
        let mut paste = build_paste(StoredContent::Plain {
            text: "live log".into(),
        });
        paste.is_live = true;
        let id = store.create_paste(paste).await.expect("create paste");

        assert!(store.get_paste(&id).await.unwrap().is_live);

        store
            .finalize_paste(&id)
            .await
            .expect("finalize should succeed");

        assert!(!store.get_paste(&id).await.unwrap().is_live);
    }

    #[tokio::test]
    async fn finalize_paste_persists_modified_record_before_cache_update() {
        let adapter = Arc::new(RecordingAdapter::default());
        let store = MemoryPasteStore::with_persistence(adapter.clone());
        let mut paste = build_paste(StoredContent::Plain {
            text: "live log".into(),
        });
        paste.is_live = true;
        let id = store.create_paste(paste).await.expect("create paste");

        store.finalize_paste(&id).await.expect("finalize paste");

        let (saved_id, saved) = adapter.last_saved_paste().expect("saved snapshot");
        assert_eq!(saved_id, id);
        assert!(!saved.is_live);
        assert!(!store.get_paste(&id).await.expect("cached paste").is_live);
    }

    #[tokio::test]
    async fn failed_persistent_finalize_is_reported_and_leaves_cache_live() {
        let adapter = Arc::new(RecordingAdapter::default());
        let store = MemoryPasteStore::with_persistence(adapter.clone());
        let mut paste = build_paste(StoredContent::Plain {
            text: "live log".into(),
        });
        paste.is_live = true;
        let id = store.create_paste(paste).await.expect("create paste");
        adapter.fail_next_save("backend unavailable");

        let result = store.finalize_paste(&id).await;

        assert!(matches!(
            result,
            Err(PasteMutationError::Persistence(PersistenceError::Save(
                _,
                _
            )))
        ));
        assert!(
            store
                .get_paste(&id)
                .await
                .expect("original cached paste")
                .is_live
        );
    }

    #[tokio::test]
    async fn finalize_paste_not_found_returns_error() {
        let store = MemoryPasteStore::default();
        let err = store
            .finalize_paste("nonexistent")
            .await
            .expect_err("should fail");
        assert!(matches!(err, PasteMutationError::NotFound(_)));
    }

    #[tokio::test]
    async fn stats_caches_result_within_ttl() {
        let store = MemoryPasteStore::default();

        let paste = build_paste(StoredContent::Plain { text: "one".into() });
        store.create_paste(paste).await.expect("create paste");

        let stats1 = store.stats().await;
        assert_eq!(stats1.total_pastes, 1);

        // Create a second paste — should not be visible within the TTL window.
        let paste2 = build_paste(StoredContent::Plain { text: "two".into() });
        store.create_paste(paste2).await.expect("create paste");

        let stats2 = store.stats().await;
        assert_eq!(
            stats2.total_pastes, 1,
            "stats should be served from cache within the TTL window"
        );
    }

    #[tokio::test]
    async fn stats_reports_counts_and_breakdowns() {
        let store = MemoryPasteStore::default();

        let mut plain = build_paste(StoredContent::Plain { text: "one".into() });
        plain.burn_after_reading = true;
        plain.metadata.not_before = Some(1_700_000_100);
        plain.metadata.not_after = Some(1_700_000_200);

        let mut encrypted = build_paste(StoredContent::Encrypted {
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            ciphertext: "cipher".into(),
            nonce: "nonce".into(),
            salt: "salt".into(),
        });
        encrypted.format = PasteFormat::Json;
        encrypted.expires_at = Some(0);
        encrypted.created_at = 1_650_000_000;

        let mut stego = build_paste(StoredContent::Stego {
            algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
            ciphertext: "payload".into(),
            nonce: "nonce".into(),
            salt: "salt".into(),
            carrier_mime: "image/png".into(),
            carrier_image: "data".into(),
            payload_digest: "digest".into(),
        });
        stego.format = PasteFormat::Markdown;
        stego.created_at = 1_700_086_400;

        let id1 = store.create_paste(plain).await.expect("create paste");
        let id2 = store.create_paste(encrypted).await.expect("create paste");
        let id3 = store.create_paste(stego).await.expect("create paste");

        let stats = store.stats().await;

        assert_eq!(stats.total_pastes, 3);
        assert_eq!(stats.active_pastes, 2);
        assert_eq!(stats.expired_pastes, 1);
        assert_eq!(stats.burn_after_reading_count, 1);
        assert_eq!(stats.time_locked_count, 1);

        let format_counts: HashMap<_, _> = stats
            .formats
            .iter()
            .map(|entry| (entry.format, entry.count))
            .collect();
        assert_eq!(format_counts.get(&PasteFormat::PlainText), Some(&1));
        assert_eq!(format_counts.get(&PasteFormat::Json), Some(&1));
        assert_eq!(format_counts.get(&PasteFormat::Markdown), Some(&1));

        let encryption_counts: HashMap<_, _> = stats
            .encryption_usage
            .iter()
            .map(|entry| (entry.algorithm, entry.count))
            .collect();
        assert_eq!(encryption_counts.get(&EncryptionAlgorithm::None), Some(&1));
        assert_eq!(
            encryption_counts.get(&EncryptionAlgorithm::Aes256Gcm),
            Some(&1)
        );
        assert_eq!(
            encryption_counts.get(&EncryptionAlgorithm::ChaCha20Poly1305),
            Some(&1)
        );

        let day_total: usize = stats.created_by_day.iter().map(|entry| entry.count).sum();
        assert_eq!(day_total, 3);

        let mut ids = store.get_all_paste_ids().await;
        ids.sort();
        let mut expected = vec![id1, id2, id3];
        expected.sort();
        assert_eq!(ids, expected);
    }
}
