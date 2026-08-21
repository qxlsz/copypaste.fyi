use std::env;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use reqwest::redirect::Policy;
use reqwest::{Client, Response, Url};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{PersistenceAdapter, PersistenceError, StoredPaste};

const DEFAULT_KEY_PREFIX: &str = "paste:";
const KEY_PREFIX_ENV: &str = "COPYPASTE_REDIS_KEY_PREFIX";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
// Stored encryption fields can expand the validated 1 MiB content limit through
// base64 and record metadata. Keep Redis responses bounded with ample overhead.
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_KEY_PREFIX_BYTES: usize = 256;

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
            is_live: false,
            owner_token_hash: None,
        }
    }

    #[tokio::test]
    async fn command_uses_json_body_at_base_url_with_authorization() {
        let server = MockServer::start();
        let adapter = test_adapter(&server);
        let key = adapter.key("abc");

        let expected_key = key.clone();
        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Bearer token")
                .json_body(json!(["SET", expected_key, "value"]));
            then.status(200).json_body(json!({"result": "OK"}));
        });

        let result: Option<String> = adapter
            .execute_command(
                vec![json!("SET"), json!(&key), json!("value")],
                &key,
                RedisOperation::Save,
            )
            .await
            .expect("post_command should succeed");
        assert_eq!(result.as_deref(), Some("OK"));
        mock.assert();
    }

    #[tokio::test]
    async fn get_value_handles_found_and_not_found() {
        let server = MockServer::start();
        let adapter = test_adapter(&server);
        let key = adapter.key("xyz");

        let found_key = key.clone();
        let found_mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Bearer token")
                .json_body(json!(["GET", found_key]));
            then.status(200).json_body(json!({"result": "payload"}));
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

        let missing_key = key.clone();
        let not_found_mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Bearer token")
                .json_body(json!(["GET", missing_key]));
            then.status(200).json_body(json!({"result": null}));
        });

        let none = adapter
            .get_value(&key)
            .await
            .expect("null result should map to Ok(None)");
        assert!(none.is_none());
        not_found_mock.assert();
    }

    #[tokio::test]
    async fn load_http_404_is_storage_failure_not_missing_key() {
        let server = MockServer::start();
        let adapter = test_adapter(&server);
        let key = adapter.key("misrouted");

        let expected_key = key.clone();
        let failure = server.mock(move |when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Bearer token")
                .json_body(json!(["GET", expected_key]));
            then.status(404);
        });

        let error = adapter
            .get_value(&key)
            .await
            .expect_err("HTTP 404 from the provider must fail closed");
        assert!(matches!(error, PersistenceError::Load(_, _)));
        failure.assert();
    }

    #[tokio::test]
    async fn save_load_and_delete_use_body_commands_without_content_in_url() {
        let server = MockServer::start();
        let adapter = test_adapter(&server);
        let key = adapter.key("roundtrip");
        let mut paste = sample_paste();
        paste.expires_at = None;
        if let StoredContent::Encrypted { ciphertext, .. } = &mut paste.content {
            *ciphertext = "url-secret-marker/with?query#fragment".into();
        }
        let serialized = serde_json::to_string(&paste).unwrap();

        let save_key = key.clone();
        let save_body = serialized.clone();
        let save_mock = server.mock(move |when, then| {
            when.method(POST)
                // Exact root path proves neither serialized content nor the
                // secret marker was placed in the request target.
                .path("/")
                .header("authorization", "Bearer token")
                .json_body(json!(["SET", save_key, save_body]));
            then.status(200).json_body(json!({"result": "OK"}));
        });

        let load_key = key.clone();
        let body = serialized.clone();
        let load_mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Bearer token")
                .json_body(json!(["GET", load_key]));
            then.status(200).json_body(json!({"result": body}));
        });

        let delete_key = key.clone();
        let delete_mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Bearer token")
                .json_body(json!(["DEL", delete_key]));
            then.status(200).json_body(json!({"result": 1}));
        });

        adapter
            .save("roundtrip", &paste)
            .await
            .expect("save succeeds");
        save_mock.assert();

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

    #[tokio::test]
    async fn expiring_pastes_use_setex_and_keep_payload_in_body() {
        let server = MockServer::start();
        let adapter = test_adapter(&server);
        let paste = sample_paste();

        let mock = server.mock(move |when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Bearer token")
                .body_includes("[\"SETEX\",\"prefix:ttl\",\"")
                .body_includes("cipher");
            then.status(200).json_body(json!({"result": "OK"}));
        });

        adapter.save("ttl", &paste).await.expect("SETEX succeeds");
        mock.assert();
    }

    #[tokio::test]
    async fn already_expired_pastes_are_deleted_instead_of_saved_without_ttl() {
        let server = MockServer::start();
        let adapter = test_adapter(&server);
        let mut paste = sample_paste();
        paste.expires_at = Some(0);

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("authorization", "Bearer token")
                .json_body(json!(["DEL", "prefix:expired"]));
            then.status(200).json_body(json!({"result": 1}));
        });

        adapter
            .save("expired", &paste)
            .await
            .expect("expired record deletion succeeds");
        mock.assert();
    }

    #[tokio::test]
    async fn provider_errors_do_not_echo_response_content() {
        let server = MockServer::start();
        let adapter = test_adapter(&server);
        let key = adapter.key("error");
        let marker = "sensitive-provider-echo";

        let mock = server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200)
                .json_body(json!({"error": "sensitive-provider-echo"}));
        });

        let error = adapter
            .get_value(&key)
            .await
            .expect_err("provider error should fail");
        assert!(!error.to_string().contains(marker));
        mock.assert();
    }

    #[tokio::test]
    async fn oversized_responses_are_rejected() {
        let server = MockServer::start();
        let adapter = test_adapter(&server);
        let key = adapter.key("oversized");

        let mock = server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200).body("x".repeat(MAX_RESPONSE_BYTES + 1));
        });

        let error = adapter
            .get_value(&key)
            .await
            .expect_err("oversized response should fail");
        assert!(error.to_string().contains("exceeded size limit"));
        mock.assert();
    }

    #[test]
    fn production_base_url_requires_clean_https_origin() {
        assert_eq!(
            validate_base_url(" https://redis.example.com/ ").as_deref(),
            Ok("https://redis.example.com")
        );
        assert_eq!(
            validate_base_url("http://127.0.0.1:8079/").as_deref(),
            Ok("http://127.0.0.1:8079")
        );
        for invalid in [
            "http://redis.example.com",
            "http://127.0.0.2.example.com",
            "https://user@redis.example.com",
            "https://redis.example.com/command",
            "https://redis.example.com?token=secret",
            "not a URL",
        ] {
            assert!(validate_base_url(invalid).is_err(), "accepted {invalid}");
        }
    }
}

#[derive(Deserialize)]
struct RedisResponse<T> {
    result: Option<T>,
    error: Option<String>,
}

#[derive(Clone, Copy)]
enum RedisOperation {
    Save,
    Load,
    Delete,
}

impl RedisOperation {
    fn error(self, key: &str, message: impl Into<String>) -> PersistenceError {
        match self {
            Self::Save => PersistenceError::Save(key.to_string(), message.into()),
            Self::Load => PersistenceError::Load(key.to_string(), message.into()),
            Self::Delete => PersistenceError::Delete(key.to_string(), message.into()),
        }
    }
}

impl RedisPersistenceAdapter {
    pub fn from_env() -> Result<Arc<dyn PersistenceAdapter>, String> {
        let base_url = required_unicode_env("UPSTASH_REDIS_REST_URL")?;
        let token = required_unicode_env("UPSTASH_REDIS_REST_TOKEN")?;
        let key_prefix = match env::var(KEY_PREFIX_ENV) {
            Ok(value) => value,
            Err(env::VarError::NotPresent) => DEFAULT_KEY_PREFIX.to_string(),
            Err(env::VarError::NotUnicode(_)) => {
                return Err(format!("{KEY_PREFIX_ENV} must contain valid Unicode"));
            }
        };

        let base_url = validate_base_url(&base_url)?;
        if token.is_empty()
            || token.len() > 4096
            || !token.as_bytes().iter().all(|byte| byte.is_ascii_graphic())
        {
            return Err(
                "UPSTASH_REDIS_REST_TOKEN must be a non-empty visible-ASCII token".to_string(),
            );
        }
        if key_prefix.len() > MAX_KEY_PREFIX_BYTES
            || !key_prefix
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_graphic())
        {
            return Err(format!(
                "{KEY_PREFIX_ENV} must contain at most {MAX_KEY_PREFIX_BYTES} visible-ASCII bytes"
            ));
        }

        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            // Never forward the Redis authorization header or command body to
            // a different origin through an HTTP redirect.
            .redirect(Policy::none())
            .build()
            .map_err(|error| format!("failed to build Redis HTTP client: {error}"))?;

        let adapter = RedisPersistenceAdapter {
            client,
            base_url,
            token,
            key_prefix,
        };

        Ok(Arc::new(adapter))
    }

    fn key(&self, id: &str) -> String {
        format!("{}{}", self.key_prefix, id)
    }

    async fn execute_command<T: DeserializeOwned>(
        &self,
        command: Vec<Value>,
        key: &str,
        operation: RedisOperation,
    ) -> Result<Option<T>, PersistenceError> {
        // Upstash accepts the entire Redis command as a JSON array at the base
        // REST endpoint. In particular, paste JSON must remain in the HTTPS
        // request body and must never become part of a request target or log.
        let response = self
            .client
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&command)
            .send()
            .await
            .map_err(|error| operation.error(key, error.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(operation.error(key, format!("Redis command failed with HTTP {status}")));
        }
        let body = read_bounded_body(response)
            .await
            .map_err(|error| operation.error(key, error))?;

        let body: RedisResponse<T> = serde_json::from_slice(&body)
            .map_err(|error| operation.error(key, error.to_string()))?;

        if body.error.is_some() {
            // Do not copy the provider's message into logs: command errors can
            // echo arguments, and arguments may contain paste content.
            return Err(operation.error(key, "Redis command returned an error"));
        }

        Ok(body.result)
    }

    async fn get_value(&self, key: &str) -> Result<Option<String>, PersistenceError> {
        self.execute_command(vec![json!("GET"), json!(key)], key, RedisOperation::Load)
            .await
    }

    async fn delete_key(&self, key: &str) -> Result<(), PersistenceError> {
        let _: Option<u64> = self
            .execute_command(vec![json!("DEL"), json!(key)], key, RedisOperation::Delete)
            .await?;
        Ok(())
    }
}

fn required_unicode_env(name: &'static str) -> Result<String, String> {
    match env::var(name) {
        Ok(value) => Ok(value),
        Err(env::VarError::NotPresent) => Err(format!("{name} missing")),
        Err(env::VarError::NotUnicode(_)) => Err(format!("{name} must contain valid Unicode")),
    }
}

fn validate_base_url(value: &str) -> Result<String, String> {
    let trimmed = value.trim().trim_end_matches('/');
    let parsed =
        Url::parse(trimmed).map_err(|_| "UPSTASH_REDIS_REST_URL is invalid".to_string())?;
    let loopback_http = parsed.scheme() == "http"
        && parsed.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        });
    if (parsed.scheme() != "https" && !loopback_http)
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(
            "UPSTASH_REDIS_REST_URL must be a clean HTTPS origin (HTTP is allowed only on loopback) with no credentials, path, query, or fragment".to_string(),
        );
    }
    Ok(trimmed.to_string())
}

async fn read_bounded_body(mut response: Response) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err("Redis response exceeded size limit".to_string());
    }

    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("failed to read Redis response: {error}"))?
    {
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err("Redis response exceeded size limit".to_string());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[async_trait]
impl PersistenceAdapter for RedisPersistenceAdapter {
    async fn save(&self, id: &str, paste: &StoredPaste) -> Result<(), PersistenceError> {
        let key = self.key(id);
        let serialized = serde_json::to_string(paste)
            .map_err(|error| PersistenceError::Save(id.to_string(), error.to_string()))?;

        let ttl_seconds = paste.expires_at.map(|expires_at| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or_default();
            expires_at - now
        });

        if ttl_seconds.is_some_and(|ttl| ttl <= 0) {
            // Never turn an already-expired record into an unbounded SET.
            let _: Option<u64> = self
                .execute_command(vec![json!("DEL"), json!(&key)], &key, RedisOperation::Save)
                .await?;
            Ok(())
        } else if let Some(ttl) = ttl_seconds {
            let result: Option<String> = self
                .execute_command(
                    vec![
                        json!("SETEX"),
                        json!(&key),
                        // Redis command arguments are strings in Upstash's
                        // JSON-body form (and in the local REST shim).
                        json!((ttl as u64).to_string()),
                        json!(&serialized),
                    ],
                    &key,
                    RedisOperation::Save,
                )
                .await?;
            expect_ok_result(&key, result)
        } else {
            let result: Option<String> = self
                .execute_command(
                    vec![json!("SET"), json!(&key), json!(&serialized)],
                    &key,
                    RedisOperation::Save,
                )
                .await?;
            expect_ok_result(&key, result)
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

fn expect_ok_result(key: &str, result: Option<String>) -> Result<(), PersistenceError> {
    if result.as_deref() == Some("OK") {
        Ok(())
    } else {
        Err(PersistenceError::Save(
            key.to_string(),
            "Redis save did not return OK".to_string(),
        ))
    }
}
