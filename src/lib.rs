use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PasteFormat {
    #[default]
    PlainText,
    Markdown,
    Code,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EncryptionAlgorithm {
    #[default]
    None,
    Aes256Gcm,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPaste {
    pub content: StoredContent,
    pub format: PasteFormat,
    pub created_at: i64,
    pub expires_at: Option<i64>,
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
}

pub struct MemoryPasteStore {
    entries: RwLock<HashMap<String, StoredPaste>>,
}

impl MemoryPasteStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
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

#[async_trait]
impl PasteStore for MemoryPasteStore {
    async fn create_paste(&self, paste: StoredPaste) -> String {
        let id = nanoid!(8);
        let mut map = self.entries.write().await;
        map.insert(id.clone(), paste);
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
            None => Err(PasteError::NotFound(id.to_string())),
        }
    }
}

pub type SharedPasteStore = Arc<dyn PasteStore>;

pub fn create_paste_store() -> SharedPasteStore {
    Arc::new(MemoryPasteStore::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn creates_and_reads_plain_paste() {
        let store = MemoryPasteStore::default();
        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "hello world".into(),
            },
            format: PasteFormat::Markdown,
            created_at: 1234,
            expires_at: None,
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
        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "stale".into(),
            },
            format: PasteFormat::PlainText,
            created_at: 100,
            expires_at: Some(50),
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
        };

        let id = store.create_paste(paste).await;
        let stored = store.get_paste(&id).await.expect("paste should exist");
        assert!(matches!(stored.content, StoredContent::Encrypted { .. }));
    }
}
