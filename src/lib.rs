use std::collections::{BTreeMap, HashMap};
use std::env;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use nanoid::nanoid;
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
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
}

#[async_trait]
pub trait PasteStore: Send + Sync + 'static {
    async fn create_paste(&self, paste: StoredPaste) -> String;
    async fn get_paste(&self, id: &str) -> Result<StoredPaste, PasteError>;
    async fn delete_paste(&self, id: &str) -> bool;
    async fn get_all_paste_ids(&self) -> Vec<String>;
    async fn stats(&self) -> StoreStats;
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

pub struct MemoryPasteStore {
    entries: RwLock<HashMap<String, StoredPaste>>,
    persistence: Option<Arc<dyn PersistenceAdapter>>,
}

impl MemoryPasteStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            persistence: None,
        }
    }

    pub fn with_persistence(adapter: Arc<dyn PersistenceAdapter>) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            persistence: Some(adapter),
        }
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
        now > expires_at
    } else {
        false
    }
}

pub(crate) fn bool_is_false(value: &bool) -> bool {
    !*value
}

const PASTE_ID_ADJECTIVES: &[&str] = &[
    "stellar", "quantum", "luminous", "neon", "orbital", "cosmic", "radiant", "sonic", "velvet",
    "ember",
];

const PASTE_ID_NOUNS: &[&str] = &[
    "otter", "phoenix", "nebula", "cipher", "comet", "matrix", "falcon", "vertex", "galaxy",
    "aurora",
];

fn generate_paste_id(map: &HashMap<String, StoredPaste>) -> String {
    let mut rng = rand::thread_rng();

    for _ in 0..12 {
        let adjective = PASTE_ID_ADJECTIVES
            .choose(&mut rng)
            .unwrap_or(&PASTE_ID_ADJECTIVES[0]);
        let noun = PASTE_ID_NOUNS
            .choose(&mut rng)
            .unwrap_or(&PASTE_ID_NOUNS[0]);
        let number: u16 = rng.gen_range(10..100);
        let candidate = format!("{adjective}-{noun}-{number}");
        if !map.contains_key(&candidate) {
            return candidate;
        }
    }

    nanoid!(10)
}

#[async_trait]
impl PasteStore for MemoryPasteStore {
    async fn create_paste(&self, paste: StoredPaste) -> String {
        let mut map = self.entries.write().await;
        let id = generate_paste_id(&map);
        map.insert(id.clone(), paste.clone());
        if let Some(adapter) = &self.persistence {
            let _ = adapter.save(&id, &paste).await;
        }
        id
    }

    async fn get_paste(&self, id: &str) -> Result<StoredPaste, PasteError> {
        let mut map = self.entries.write().await;
        match map.get(id) {
            Some(paste) if !is_expired(paste) => Ok(paste.clone()),
            Some(_) => {
                map.remove(id);
                Err(PasteError::Expired(id.to_string()))
            }
            None => {
                if let Some(adapter) = &self.persistence {
                    match adapter.load(id).await {
                        Ok(Some(paste)) => {
                            if is_expired(&paste) {
                                return Err(PasteError::Expired(id.to_string()));
                            }
                            map.insert(id.to_string(), paste.clone());
                            Ok(paste)
                        }
                        Ok(None) => Err(PasteError::NotFound(id.to_string())),
                        Err(_) => Err(PasteError::NotFound(id.to_string())),
                    }
                } else {
                    Err(PasteError::NotFound(id.to_string()))
                }
            }
        }
    }

    async fn delete_paste(&self, id: &str) -> bool {
        let mut map = self.entries.write().await;
        let existed = map.remove(id).is_some();
        if let Some(adapter) = &self.persistence {
            let _ = adapter.delete(id).await;
        }
        existed
    }

    async fn stats(&self) -> StoreStats {
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
    }

    async fn get_all_paste_ids(&self) -> Vec<String> {
        let map = self.entries.read().await;
        map.keys().cloned().collect()
    }
}

pub type SharedPasteStore = Arc<dyn PasteStore>;

pub fn create_paste_store() -> SharedPasteStore {
    match env::var("COPYPASTE_PERSISTENCE_BACKEND") {
        Ok(value) if value.eq_ignore_ascii_case("vault") => {
            if let Ok(adapter) = vault::VaultPersistenceAdapter::from_env() {
                return Arc::new(MemoryPasteStore::with_persistence(adapter));
            }
            Arc::new(MemoryPasteStore::new())
        }
        Ok(value) if value.eq_ignore_ascii_case("redis") => {
            if let Ok(adapter) = RedisPersistenceAdapter::from_env() {
                return Arc::new(MemoryPasteStore::with_persistence(adapter));
            }
            Arc::new(MemoryPasteStore::new())
        }
        Ok(value) if value.eq_ignore_ascii_case("memory") || value.trim().is_empty() => {
            Arc::new(MemoryPasteStore::new())
        }
        _ => Arc::new(MemoryPasteStore::new()),
    }
}

pub mod vault {
    use super::{PersistenceAdapter, PersistenceError, StoredPaste};
    use async_trait::async_trait;
    use reqwest::Client;
    use serde::Deserialize;
    use serde_json::json;
    use std::env;
    use std::sync::Arc;

    #[derive(Clone)]
    pub struct VaultPersistenceAdapter {
        client: Client,
        addr: String,
        token: String,
        mount: String,
        namespace: Option<String>,
        key_prefix: String,
    }

    impl VaultPersistenceAdapter {
        pub fn from_env() -> Result<Arc<dyn PersistenceAdapter>, String> {
            let addr = env::var("COPYPASTE_VAULT_ADDR")
                .map_err(|_| "COPYPASTE_VAULT_ADDR missing".to_string())?;
            let token = env::var("COPYPASTE_VAULT_TOKEN")
                .map_err(|_| "COPYPASTE_VAULT_TOKEN missing".to_string())?;
            let mount = env::var("COPYPASTE_VAULT_MOUNT").unwrap_or_else(|_| "secret".to_string());
            let namespace = env::var("COPYPASTE_VAULT_NAMESPACE").ok();
            let key_prefix =
                env::var("COPYPASTE_VAULT_PREFIX").unwrap_or_else(|_| "copypaste".to_string());
            let client = Client::builder()
                .build()
                .map_err(|e| format!("failed to build vault client: {e}"))?;

            let adapter = VaultPersistenceAdapter {
                client,
                addr,
                token,
                mount,
                namespace,
                key_prefix,
            };

            let arc: Arc<dyn PersistenceAdapter> = Arc::new(adapter);
            Ok(arc)
        }

        fn data_path(&self, id: &str) -> String {
            format!(
                "{}/v1/{}/data/{}",
                self.addr.trim_end_matches('/'),
                self.mount.trim_matches('/'),
                self.namespaced_id(id)
            )
        }

        fn metadata_path(&self, id: &str) -> String {
            format!(
                "{}/v1/{}/metadata/{}",
                self.addr.trim_end_matches('/'),
                self.mount.trim_matches('/'),
                self.namespaced_id(id)
            )
        }

        fn namespaced_id(&self, id: &str) -> String {
            if self.key_prefix.is_empty() {
                id.to_string()
            } else {
                format!("{}/{}", self.key_prefix.trim_matches('/'), id)
            }
        }

        fn auth_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
            let builder = builder.header("X-Vault-Token", &self.token);
            if let Some(ns) = &self.namespace {
                builder.header("X-Vault-Namespace", ns)
            } else {
                builder
            }
        }
    }

    #[derive(Deserialize)]
    struct VaultReadResponse {
        data: VaultReadData,
    }

    #[derive(Deserialize)]
    struct VaultReadData {
        data: VaultPayload,
    }

    #[derive(Deserialize)]
    struct VaultPayload {
        payload: String,
    }

    #[async_trait]
    impl PersistenceAdapter for VaultPersistenceAdapter {
        async fn save(&self, id: &str, paste: &StoredPaste) -> Result<(), PersistenceError> {
            let serialized = serde_json::to_string(paste)
                .map_err(|e| PersistenceError::Save(id.to_string(), e.to_string()))?;
            let payload = json!({
                "data": {
                    "payload": serialized,
                }
            });

            let request = self
                .auth_headers(self.client.post(self.data_path(id)))
                .json(&payload);

            request
                .send()
                .await
                .map_err(|e| PersistenceError::Save(id.to_string(), e.to_string()))?
                .error_for_status()
                .map_err(|e| PersistenceError::Save(id.to_string(), e.to_string()))?;
            Ok(())
        }

        async fn load(&self, id: &str) -> Result<Option<StoredPaste>, PersistenceError> {
            let request = self.auth_headers(self.client.get(self.data_path(id)));
            let response = request.send().await;
            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let body: VaultReadResponse = resp
                            .json()
                            .await
                            .map_err(|e| PersistenceError::Load(id.to_string(), e.to_string()))?;
                        let paste: StoredPaste = serde_json::from_str(&body.data.data.payload)
                            .map_err(|e| PersistenceError::Load(id.to_string(), e.to_string()))?;
                        Ok(Some(paste))
                    } else if resp.status().as_u16() == 404 {
                        Ok(None)
                    } else {
                        Err(PersistenceError::Load(
                            id.to_string(),
                            format!("unexpected status {}", resp.status()),
                        ))
                    }
                }
                Err(err) => Err(PersistenceError::Load(id.to_string(), err.to_string())),
            }
        }

        async fn delete(&self, id: &str) -> Result<(), PersistenceError> {
            let request = self.auth_headers(self.client.delete(self.metadata_path(id)));
            let response = request
                .send()
                .await
                .map_err(|e| PersistenceError::Delete(id.to_string(), e.to_string()))?;
            if response.status().is_success() || response.status().as_u16() == 404 {
                Ok(())
            } else {
                Err(PersistenceError::Delete(
                    id.to_string(),
                    format!("unexpected status {}", response.status()),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingAdapter {
        saved: Mutex<Vec<String>>,
        deleted: Mutex<Vec<String>>,
        load_queue: Mutex<VecDeque<Result<Option<StoredPaste>, PersistenceError>>>,
    }

    impl RecordingAdapter {
        fn with_load_results(results: Vec<Result<Option<StoredPaste>, PersistenceError>>) -> Self {
            Self {
                saved: Mutex::new(Vec::new()),
                deleted: Mutex::new(Vec::new()),
                load_queue: Mutex::new(results.into_iter().collect()),
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
    }

    #[async_trait]
    impl PersistenceAdapter for RecordingAdapter {
        async fn save(&self, id: &str, _paste: &StoredPaste) -> Result<(), PersistenceError> {
            self.saved.lock().unwrap().push(id.to_string());
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
        }
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
        };

        let id = store.create_paste(paste).await;
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
        };

        let id = store.create_paste(paste).await;
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
        };

        let id = store.create_paste(paste).await;
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

        let id = store.create_paste(paste).await;
        assert!(store.delete_paste(&id).await);
        assert_eq!(adapter.take_deleted(), vec![id.clone()]);

        // Second deletion still triggers adapter delete but reports false
        assert!(!store.delete_paste(&id).await);
        assert_eq!(adapter.take_deleted(), vec![id.clone()]);
        assert_eq!(adapter.take_saved(), vec![id]);
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
    async fn get_paste_returns_not_found_on_adapter_error() {
        let adapter = Arc::new(RecordingAdapter::with_load_results(vec![Err(
            PersistenceError::Load("err".into(), "boom".into()),
        )]));
        let store = MemoryPasteStore::with_persistence(adapter);

        let err = store
            .get_paste("missing-id")
            .await
            .expect_err("adapter error should surface as not found");
        assert!(matches!(err, PasteError::NotFound(id) if id == "missing-id"));
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

        let id1 = store.create_paste(plain).await;
        let id2 = store.create_paste(encrypted).await;
        let id3 = store.create_paste(stego).await;

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
