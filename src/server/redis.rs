use std::env;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use urlencoding::encode;

use crate::{PersistenceAdapter, PersistenceError, StoredPaste};

const DEFAULT_KEY_PREFIX: &str = "paste:";
const KEY_PREFIX_ENV: &str = "COPYPASTE_REDIS_KEY_PREFIX";

#[derive(Clone)]
pub struct RedisPersistenceAdapter {
    client: Client,
    base_url: String,
    token: String,
    key_prefix: String,
}

#[derive(Deserialize)]
struct RedisResponse<T> {
    result: Option<T>,
    error: Option<String>,
}

impl RedisPersistenceAdapter {
    pub fn from_env() -> Result<Arc<dyn PersistenceAdapter>, String> {
        let base_url = env::var("UPSTASH_REDIS_REST_URL")
            .map_err(|_| "UPSTASH_REDIS_REST_URL missing".to_string())?;
        let token = env::var("UPSTASH_REDIS_REST_TOKEN")
            .map_err(|_| "UPSTASH_REDIS_REST_TOKEN missing".to_string())?;
        let key_prefix =
            env::var(KEY_PREFIX_ENV).unwrap_or_else(|_| DEFAULT_KEY_PREFIX.to_string());

        let adapter = RedisPersistenceAdapter {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
            key_prefix,
        };

        Ok(Arc::new(adapter))
    }

    fn key(&self, id: &str) -> String {
        format!("{}{}", self.key_prefix, id)
    }

    async fn post_command(
        &self,
        command: &str,
        key: &str,
        extra: &[&str],
    ) -> Result<(), PersistenceError> {
        let mut path = format!("{}/{}/{}", self.base_url, command, encode(key));
        for segment in extra {
            path.push('/');
            path.push_str(&encode(segment));
        }

        let response = self
            .client
            .post(&path)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|error| PersistenceError::Save(key.to_string(), error.to_string()))?;

        if !response.status().is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "<empty>".to_string());
            return Err(PersistenceError::Save(
                key.to_string(),
                format!("Redis command failed: {}", text),
            ));
        }

        Ok(())
    }

    async fn get_value(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        let url = format!("{}/get/{}", self.base_url, encode(key));

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|error| PersistenceError::Load(key.to_string(), error.to_string()))?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "<empty>".to_string());
            return Err(PersistenceError::Load(
                key.to_string(),
                format!("Redis GET failed: {}", text),
            ));
        }

        let body: RedisResponse<String> = response
            .json()
            .await
            .map_err(|error| PersistenceError::Load(key.to_string(), error.to_string()))?;

        if let Some(error) = body.error {
            return Err(PersistenceError::Load(key.to_string(), error));
        }

        Ok(body.result)
    }

    async fn delete_key(&self, key: &str) -> Result<(), PersistenceError> {
        self.post_command("del", key, &[]).await
    }
}

#[async_trait]
impl PersistenceAdapter for RedisPersistenceAdapter {
    async fn save(&self, id: &str, paste: &StoredPaste) -> Result<(), PersistenceError> {
        let key = self.key(id);
        let serialized = serde_json::to_string(paste)
            .map_err(|error| PersistenceError::Save(id.to_string(), error.to_string()))?;

        let ttl_seconds = paste.expires_at.and_then(|expires_at| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or_default();
            let remaining = expires_at - now;
            if remaining > 0 {
                Some(remaining as u64)
            } else {
                None
            }
        });

        if let Some(ttl) = ttl_seconds {
            self.post_command("setex", &key, &[&ttl.to_string(), &serialized])
                .await
        } else {
            self.post_command("set", &key, &[&serialized]).await
        }
    }

    async fn load(&self, id: &str) -> Result<Option<StoredPaste>, PersistenceError> {
        let key = self.key(id);
        if let Some(value) = self.get_value(&key).await? {
            let paste: StoredPaste = serde_json::from_str(&value)
                .map_err(|error| PersistenceError::Load(id.to_string(), error.to_string()))?;
            Ok(Some(paste))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, id: &str) -> Result<(), PersistenceError> {
        let key = self.key(id);
        self.delete_key(&key).await
    }
}
