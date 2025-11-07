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

pub mod server;

use crate::server::redis::RedisPersistenceAdapter;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, Hash)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EncryptionAlgorithm {
    #[default]
    None,
    Aes256Gcm,
    #[serde(rename = "chacha20_poly1305")]
    ChaCha20Poly1305,
    #[serde(rename = "xchacha20_poly1305")]
    XChaCha20Poly1305,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatUsage {
    pub format: PasteFormat,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptionUsage {
    pub algorithm: EncryptionAlgorithm,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyCount {
    pub date: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BundleMetadata {
    pub children: Vec<BundlePointer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundlePointer {
    pub id: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookProvider {
    Slack,
    Teams,
    Generic,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
}
