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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EncryptionAlgorithm, PasteFormat, PasteMetadata, StoredContent, StoredPaste};
    use httpmock::prelude::*;
    use regex::Regex;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_adapter(server: &MockServer) -> RedisPersistenceAdapter {
        RedisPersistenceAdapter {
            client: Client::new(),
            base_url: server.base_url(),
            token: "token".to_string(),
            key_prefix: "prefix:".to_string(),
        }
    }

    fn sample_paste() -> StoredPaste {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        StoredPaste {
            content: StoredContent::Encrypted {
                algorithm: EncryptionAlgorithm::Aes256Gcm,
                ciphertext: "cipher".into(),
                nonce: "nonce".into(),
                salt: "salt".into(),
            },
            format: PasteFormat::Json,
            created_at: now - 60,
            expires_at: Some(now + 3600),
            burn_after_reading: false,
            metadata: PasteMetadata::default(),
            bundle: None,
            bundle_parent: None,
            bundle_label: None,
            not_before: None,
            not_after: None,
            persistence: None,
            webhook: None,
        }
    }

    #[tokio::test]
    async fn post_command_sends_authorized_request() {
        let server = MockServer::start();
        let adapter = test_adapter(&server);
        let key = adapter.key("abc");
        let encoded_key = urlencoding::encode(&key).into_owned();

        let pattern = Regex::new(&format!(r"^/set/{}/.+", regex::escape(&encoded_key))).unwrap();
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path_matches(pattern.clone())
                .header("authorization", "Bearer token");
            then.status(200);
        });

        adapter
            .post_command("set", &key, &["value"])
            .await
            .expect("post_command should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn get_value_handles_found_and_not_found() {
        let server = MockServer::start();
        let adapter = test_adapter(&server);
        let key = adapter.key("xyz");
        let encoded_key = urlencoding::encode(&key).into_owned();

        let value_path = format!("/get/{encoded_key}");
        let found_path = value_path.clone();
        let found_mock = server.mock(move |when, then| {
            when.method(GET)
                .path(found_path.clone())
                .header("authorization", "Bearer token");
            then.status(200)
                .json_body(json!({"result": "payload", "error": null}));
        });

        let value = adapter
            .get_value(&key)
            .await
            .expect("get_value should succeed")
            .expect("value should exist");
        assert_eq!(value, "payload");
        found_mock.assert();
        let server = MockServer::start();
        let adapter = test_adapter(&server);
        let key = adapter.key("missing");
        let encoded_key = urlencoding::encode(&key).into_owned();
        let value_path = format!("/get/{encoded_key}");
        let not_found_path = value_path.clone();

        let not_found_mock = server.mock(move |when, then| {
            when.method(GET)
                .path(not_found_path.clone())
                .header("authorization", "Bearer token");
            then.status(404);
        });

        let none = adapter
            .get_value(&key)
            .await
            .expect("404 should map to Ok(None)");
        assert!(none.is_none());
        not_found_mock.assert();
    }

    #[tokio::test]
    async fn save_and_load_flow_roundtrips_payload() {
        let server = MockServer::start();
        let adapter = test_adapter(&server);
        let key = adapter.key("roundtrip");
        let encoded_key = urlencoding::encode(&key).into_owned();
        let paste = sample_paste();
        let serialized = serde_json::to_string(&paste).unwrap();
        let setex_pattern =
            Regex::new(&format!(r"^/setex/{}/.+", regex::escape(&encoded_key))).unwrap();
        let setex_mock = server.mock(move |when, then| {
            when.method(POST)
                .path_matches(setex_pattern.clone())
                .header("authorization", "Bearer token");
            then.status(200);
        });

        let get_path = format!("/get/{encoded_key}");
        let load_path = get_path.clone();
        let body = serialized.clone();
        let load_mock = server.mock(move |when, then| {
            when.method(GET)
                .path(load_path.clone())
                .header("authorization", "Bearer token");
            then.status(200).json_body(json!({
                "result": body,
                "error": null
            }));
        });

        let delete_pattern =
            Regex::new(&format!(r"^/del/{}$", regex::escape(&encoded_key))).unwrap();
        let delete_mock = server.mock(move |when, then| {
            when.method(POST)
                .path_matches(delete_pattern.clone())
                .header("authorization", "Bearer token");
            then.status(200);
        });

        adapter
            .save("roundtrip", &paste)
            .await
            .expect("save succeeds");
        setex_mock.assert();

        let loaded = adapter
            .load("roundtrip")
            .await
            .expect("load succeeds")
            .expect("paste should exist");
        assert_eq!(loaded.created_at, paste.created_at);
        load_mock.assert();

        adapter.delete("roundtrip").await.expect("delete succeeds");
        delete_mock.assert();
    }
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
