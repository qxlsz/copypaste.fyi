use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use nanoid::nanoid;
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Error, Debug)]
pub enum PasteError {
    #[error("paste not found: {0}")]
    NotFound(String),
}

#[async_trait]
pub trait PasteStore: Send + Sync + 'static {
    async fn create_paste(&self, content: String) -> Result<String, PasteError>;
    async fn get_paste(&self, id: &str) -> Result<String, PasteError>;
}

pub struct MemoryPasteStore {
    entries: RwLock<HashMap<String, String>>,
}

impl MemoryPasteStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl PasteStore for MemoryPasteStore {
    async fn create_paste(&self, content: String) -> Result<String, PasteError> {
        let id = nanoid!(8);
        let mut map = self.entries.write().await;
        map.insert(id.clone(), content);
        Ok(id)
    }

    async fn get_paste(&self, id: &str) -> Result<String, PasteError> {
        let map = self.entries.read().await;
        map.get(id)
            .cloned()
            .ok_or_else(|| PasteError::NotFound(id.to_string()))
    }
}

pub type SharedPasteStore = Arc<dyn PasteStore>;

pub fn create_paste_store() -> SharedPasteStore {
    Arc::new(MemoryPasteStore::new())
}
