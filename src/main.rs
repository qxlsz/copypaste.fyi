use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce as AesNonce};
use base64::{engine::general_purpose, Engine};
use chacha20poly1305::{ChaCha20Poly1305, Nonce as ChaNonce, XChaCha20Poly1305, XNonce};
use chrono::DateTime;
use copypaste::{
    create_paste_store, AttestationRequirement, BundleMetadata, BundlePointer, EncryptionAlgorithm,
    PasteError, PasteFormat, PasteMetadata, PersistenceLocator, SharedPasteStore, StoredContent,
    StoredPaste, WebhookConfig, WebhookProvider,
};
use data_encoding::BASE32;
use hmac::{Hmac, Mac};
use html_escape::encode_safe;
use pulldown_cmark::{html, Options, Parser};
use rand::{rngs::OsRng, RngCore};
use rocket::form::FromForm;
use rocket::fs::{FileServer, NamedFile};
use rocket::http::Status;
use rocket::response::content;
use rocket::serde::json::Json;
use rocket::serde::Deserialize;
use rocket::{get, post, routes, Build, Rocket, State};
use sha1::Sha1;
use sha2::{Digest, Sha256};

#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
struct EncryptionRequest {
    algorithm: EncryptionAlgorithm,
    key: String,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct CreateBundleRequest {
    children: Vec<CreateBundleChildRequest>,
}

#[derive(Deserialize, Clone)]
struct CreateBundleChildRequest {
    content: String,
    #[serde(default)]
    format: Option<PasteFormat>,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct TimeLockRequest {
    not_before: Option<String>,
    not_after: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum AttestationRequest {
    Totp {
        secret: String,
        #[serde(default)]
        digits: Option<u32>,
        #[serde(default)]
        step: Option<u64>,
        #[serde(default)]
        allowed_drift: Option<u32>,
        #[serde(default)]
        issuer: Option<String>,
    },
    SharedSecret {
        secret: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PersistenceRequest {
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

#[derive(Deserialize, Default)]
#[serde(default)]
struct WebhookRequest {
    url: String,
    provider: Option<WebhookProvider>,
    view_template: Option<String>,
    burn_template: Option<String>,
}

fn build_rocket(store: SharedPasteStore) -> Rocket<Build> {
    rocket::build()
        .manage(store)
        .mount("/", routes![index, create, show, show_raw, static_files])
        .mount("/", FileServer::from("static"))
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = rocket::Config {
        address: "0.0.0.0".parse()?,
        port: 8000,
        ..rocket::Config::debug_default()
    };

    build_rocket(create_paste_store())
        .configure(config)
        .launch()
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste::{
        AttestationRequirement, BundleMetadata, BundlePointer, MemoryPasteStore, PasteMetadata,
    };
    use rocket::http::{ContentType, Status};
    use rocket::local::asynchronous::Client;
    use serde_json::json;
    use std::sync::Arc;

    async fn rocket_client() -> Client {
        Client::tracked(super::build_rocket(create_paste_store()))
            .await
            .expect("valid rocket instance")
    }

    async fn rocket_client_with_store(store: SharedPasteStore) -> Client {
        Client::tracked(super::build_rocket(store))
            .await
            .expect("valid rocket instance")
    }

    #[rocket::async_test]
    async fn post_plain_text_returns_id() {
        let client = rocket_client().await;
        let payload = json!({
            "content": "plain content",
            "format": "plain_text",
            "retention_minutes": 60
        });

        let response = client
            .post("/")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch()
            .await;

        assert_eq!(response.status(), Status::Ok);
        let body = response.into_string().await.expect("response body");
        assert!(body.starts_with('/'));

        let get_response = client.get(&body).dispatch().await;
        assert_eq!(get_response.status(), Status::Ok);
        let html = get_response.into_string().await.expect("html body");
        assert!(html.contains("plain content"));
    }

    #[rocket::async_test]
    async fn post_encrypted_requires_key() {
        let client = rocket_client().await;
        let payload = json!({
            "content": "secret text",
            "format": "markdown",
            "retention_minutes": 0,
            "encryption": {
                "algorithm": "aes256_gcm",
                "key": "passphrase"
            }
        });

        let response = client
            .post("/")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch()
            .await;

        assert_eq!(response.status(), Status::Ok);
        let path = response.into_string().await.expect("body");

        let without_key = client.get(&path).dispatch().await;
        let without_body = without_key.into_string().await.expect("html");
        assert!(without_body.contains("Provide the encryption key"));

        let with_key = client
            .get(format!("{}?key=passphrase", path))
            .dispatch()
            .await;
        let html = with_key.into_string().await.expect("html");
        assert!(html.contains("secret text"));
    }

    #[rocket::async_test]
    async fn post_encrypted_chacha_roundtrip() {
        let client = rocket_client().await;
        let payload = json!({
            "content": "orbital payload",
            "format": "kotlin",
            "retention_minutes": 0,
            "encryption": {
                "algorithm": "chacha20_poly1305",
                "key": "retro-synthwave-9001"
            }
        });

        let response = client
            .post("/")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch()
            .await;

        assert_eq!(response.status(), Status::Ok);
        let path = response.into_string().await.expect("body");

        let with_key = client
            .get(format!("{}?key=retro-synthwave-9001", path))
            .dispatch()
            .await;
        let html = with_key.into_string().await.expect("html");
        assert!(html.contains("orbital payload"));
    }

    #[rocket::async_test]
    async fn post_encrypted_xchacha_roundtrip() {
        let client = rocket_client().await;
        let payload = json!({
            "content": "vault dweller",
            "format": "go",
            "retention_minutes": 0,
            "encryption": {
                "algorithm": "xchacha20_poly1305",
                "key": "matrix-quantum-4040"
            }
        });

        let response = client
            .post("/")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch()
            .await;

        assert_eq!(response.status(), Status::Ok);
        let path = response.into_string().await.expect("body");

        let with_key = client
            .get(format!("{}?key=matrix-quantum-4040", path))
            .dispatch()
            .await;
        let html = with_key.into_string().await.expect("html");
        assert!(html.contains("vault dweller"));
    }

    #[rocket::async_test]
    async fn expired_paste_shows_expired_message() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::default());
        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "short lived".into(),
            },
            format: PasteFormat::PlainText,
            created_at: 0,
            expires_at: Some(-1),
            burn_after_reading: false,
            metadata: Default::default(),
        };

        let id = store.create_paste(paste).await;
        let client = rocket_client_with_store(store).await;

        let expired = client.get(format!("/{}", id)).dispatch().await;
        let html = expired.into_string().await.expect("html");
        assert!(html.contains("Paste expired"));
    }

    #[rocket::async_test]
    async fn burn_after_reading_deletes_paste() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::default());
        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "vanish".into(),
            },
            format: PasteFormat::PlainText,
            created_at: current_timestamp(),
            expires_at: None,
            burn_after_reading: true,
            metadata: Default::default(),
        };

        let id = store.create_paste(paste).await;
        let client = rocket_client_with_store(store.clone()).await;

        let first = client.get(format!("/{}", id)).dispatch().await;
        assert_eq!(first.status(), Status::Ok);

        let second = client.get(format!("/{}", id)).dispatch().await;
        assert_eq!(second.status(), Status::NotFound);
    }

    #[rocket::async_test]
    async fn raw_endpoint_honors_burn_after_reading() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::default());
        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "raw-body".into(),
            },
            format: PasteFormat::PlainText,
            created_at: current_timestamp(),
            expires_at: None,
            burn_after_reading: true,
            metadata: Default::default(),
        };

        let id = store.create_paste(paste).await;
        let client = rocket_client_with_store(store.clone()).await;

        let first = client.get(format!("/raw/{}", id)).dispatch().await;
        assert_eq!(first.status(), Status::Ok);
        let body = first.into_string().await.expect("body");
        assert_eq!(body, "raw-body");

        let second = client.get(format!("/raw/{}", id)).dispatch().await;
        assert_eq!(second.status(), Status::NotFound);
    }

    #[rocket::async_test]
    async fn raw_endpoint_requires_key_for_encrypted_content() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::default());
        let encrypted = encrypt_content(
            "stealth payload",
            "super-secret",
            EncryptionAlgorithm::Aes256Gcm,
        )
        .expect("encryption successful");
        let paste = StoredPaste {
            content: encrypted,
            format: PasteFormat::PlainText,
            created_at: current_timestamp(),
            expires_at: None,
            burn_after_reading: false,
            metadata: Default::default(),
        };

        let id = store.create_paste(paste).await;
        let client = rocket_client_with_store(store.clone()).await;

        let missing_key = client.get(format!("/raw/{}", id)).dispatch().await;
        assert_eq!(missing_key.status(), Status::Unauthorized);

        let wrong_key = client
            .get(format!("/raw/{}?key=not-it", id))
            .dispatch()
            .await;
        assert_eq!(wrong_key.status(), Status::Forbidden);

        let ok = client
            .get(format!("/raw/{}?key=super-secret", id))
            .dispatch()
            .await;
        assert_eq!(ok.status(), Status::Ok);
        let content = ok.into_string().await.expect("body");
        assert_eq!(content, "stealth payload");
    }

    #[rocket::async_test]
    async fn time_lock_blocks_until_window() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::default());
        let metadata = PasteMetadata {
            not_before: Some(current_timestamp() + 3600),
            ..Default::default()
        };
        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "future secret".into(),
            },
            format: PasteFormat::PlainText,
            created_at: current_timestamp(),
            expires_at: None,
            burn_after_reading: false,
            metadata,
        };

        let id = store.create_paste(paste).await;
        let client = rocket_client_with_store(store).await;

        let response = client.get(format!("/{}", id)).dispatch().await;
        let html = response.into_string().await.expect("html");
        assert!(html.contains("Time-locked paste"));
    }

    #[rocket::async_test]
    async fn shared_secret_attestation_enforced() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::default());
        let secret = "trust-me";
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        let digest = hasher.finalize();
        let hash = general_purpose::STANDARD.encode(digest);

        let metadata = PasteMetadata {
            attestation: Some(AttestationRequirement::SharedSecret { hash }),
            ..Default::default()
        };

        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "attested".into(),
            },
            format: PasteFormat::PlainText,
            created_at: current_timestamp(),
            expires_at: None,
            burn_after_reading: false,
            metadata,
        };

        let id = store.create_paste(paste).await;
        let client = rocket_client_with_store(store.clone()).await;

        let missing = client.get(format!("/{}", id)).dispatch().await;
        let missing_html = missing.into_string().await.expect("html");
        assert!(missing_html.contains("Additional verification required"));

        let wrong = client
            .get(format!("/{id}?attest=incorrect"))
            .dispatch()
            .await;
        let wrong_html = wrong.into_string().await.expect("html");
        assert!(wrong_html.contains("Verification failed"));

        let ok = client
            .get(format!("/{id}?attest={}", urlencoding::encode(secret)))
            .dispatch()
            .await;
        let ok_html = ok.into_string().await.expect("html");
        assert!(ok_html.contains("attested"));
    }

    #[rocket::async_test]
    async fn bundle_links_render_and_children_burn() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::default());

        let child_metadata = PasteMetadata {
            bundle_label: Some("Alpha".into()),
            ..Default::default()
        };
        let child = StoredPaste {
            content: StoredContent::Plain {
                text: "child secret".into(),
            },
            format: PasteFormat::PlainText,
            created_at: current_timestamp(),
            expires_at: None,
            burn_after_reading: true,
            metadata: child_metadata,
        };

        let child_id = store.create_paste(child).await;

        let parent_metadata = PasteMetadata {
            bundle: Some(BundleMetadata {
                children: vec![BundlePointer {
                    id: child_id.clone(),
                    label: Some("Alpha".into()),
                }],
            }),
            ..Default::default()
        };

        let parent = StoredPaste {
            content: StoredContent::Plain {
                text: "parent".into(),
            },
            format: PasteFormat::PlainText,
            created_at: current_timestamp(),
            expires_at: None,
            burn_after_reading: false,
            metadata: parent_metadata,
        };

        let parent_id = store.create_paste(parent).await;
        let client = rocket_client_with_store(store.clone()).await;

        let response = client
            .get(format!("/{parent_id}"))
            .dispatch()
            .await
            .into_string()
            .await
            .expect("html");
        assert!(response.contains("Bundle shares"));

        let child_response = client.get(format!("/{child_id}")).dispatch().await;
        assert_eq!(child_response.status(), Status::Ok);

        let consumed = client.get(format!("/{child_id}")).dispatch().await;
        assert_eq!(consumed.status(), Status::NotFound);
    }
    #[test]
    fn encrypt_then_decrypt_roundtrip() {
        let key = "correct horse battery staple";
        let stored = encrypt_content("super secret", key, EncryptionAlgorithm::Aes256Gcm)
            .expect("encryption succeeds");
        let decrypted =
            decrypt_content(&stored, Some(key)).expect("decrypting with same key succeeds");
        assert_eq!(decrypted, "super secret");
    }

    #[test]
    fn chacha_roundtrip() {
        let key = "tachyon-vector-2048";
        let stored = encrypt_content("ghost signal", key, EncryptionAlgorithm::ChaCha20Poly1305)
            .expect("encryption succeeds");
        let decrypted =
            decrypt_content(&stored, Some(key)).expect("decrypting with same key succeeds");
        assert_eq!(decrypted, "ghost signal");
    }

    #[test]
    fn decrypt_requires_key_for_encrypted_content() {
        let stored = encrypt_content(
            "classified",
            "moonbase",
            EncryptionAlgorithm::XChaCha20Poly1305,
        )
        .expect("encryption succeeds");
        match decrypt_content(&stored, None) {
            Err(DecryptError::MissingKey) => {}
            other => panic!("expected missing key error, got {:?}", other),
        }
    }

    #[test]
    fn xchacha_roundtrip() {
        let key = "tachyon-subroutine-7331";
        let stored = encrypt_content("link shell", key, EncryptionAlgorithm::XChaCha20Poly1305)
            .expect("encryption succeeds");
        let decrypted =
            decrypt_content(&stored, Some(key)).expect("decrypting with same key succeeds");
        assert_eq!(decrypted, "link shell");
    }

    #[test]
    fn format_json_pretty_prints() {
        let result = format_json(r#"{"foo":1,"bar":[true,false]}"#);
        assert!(
            result.contains("\n"),
            "formatted JSON should contain newlines"
        );
        assert!(result.contains("foo"));
        assert!(result.starts_with("<pre><code>"));
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct CreatePasteRequest {
    content: String,
    #[serde(default)]
    format: Option<PasteFormat>,
    retention_minutes: Option<u64>,
    encryption: Option<EncryptionRequest>,
    #[serde(default)]
    burn_after_reading: bool,
    #[serde(default)]
    bundle: Option<CreateBundleRequest>,
    #[serde(default)]
    time_lock: Option<TimeLockRequest>,
    #[serde(default)]
    attestation: Option<AttestationRequest>,
    #[serde(default)]
    persistence: Option<PersistenceRequest>,
    #[serde(default)]
    webhook: Option<WebhookRequest>,
}

#[derive(Debug)]
enum DecryptError {
    MissingKey,
    InvalidKey,
}

#[derive(FromForm, Default)]
struct PasteViewQuery {
    key: Option<String>,
    code: Option<String>,
    attest: Option<String>,
}

#[get("/")]
async fn index() -> content::RawHtml<&'static str> {
    content::RawHtml(include_str!("../static/index.html"))
}

#[post("/", data = "<body>")]
async fn create(
    store: &State<SharedPasteStore>,
    body: Json<CreatePasteRequest>,
) -> Result<String, (Status, String)> {
    let body = body.into_inner();
    let now = current_timestamp();
    let format = body.format.unwrap_or_default();
    let expires_at = body.retention_minutes.and_then(|mins| match mins {
        0 => None,
        minutes => Some(now + i64::try_from(minutes).unwrap_or(0) * 60),
    });
    let burn_after_reading = body.burn_after_reading;

    let mut metadata = PasteMetadata::default();

    if let Some(lock) = body.time_lock.as_ref() {
        if let Some(ref not_before) = lock.not_before {
            let ts = parse_timestamp(not_before)
                .map_err(|e| (Status::BadRequest, format!("invalid not_before: {e}")))?;
            metadata.not_before = Some(ts);
        }
        if let Some(ref not_after) = lock.not_after {
            let ts = parse_timestamp(not_after)
                .map_err(|e| (Status::BadRequest, format!("invalid not_after: {e}")))?;
            metadata.not_after = Some(ts);
        }
        if let (Some(start), Some(end)) = (metadata.not_before, metadata.not_after) {
            if end <= start {
                return Err((
                    Status::BadRequest,
                    "time_lock not_after must be greater than not_before".to_string(),
                ));
            }
        }
    }

    if let Some(attestation) = body.attestation.as_ref() {
        let requirement = match attestation {
            AttestationRequest::Totp {
                secret,
                digits,
                step,
                allowed_drift,
                issuer,
            } => {
                if secret.trim().is_empty() {
                    return Err((Status::BadRequest, "TOTP secret cannot be empty".into()));
                }
                let digits = digits.unwrap_or(6);
                if !(4..=10).contains(&digits) {
                    return Err((
                        Status::BadRequest,
                        "TOTP digits must be between 4 and 10".into(),
                    ));
                }
                let step = step.unwrap_or(30);
                if step == 0 {
                    return Err((
                        Status::BadRequest,
                        "TOTP step must be greater than zero".into(),
                    ));
                }
                let allowed_drift = allowed_drift.unwrap_or(1);
                AttestationRequirement::Totp {
                    secret: secret.clone(),
                    digits,
                    step,
                    allowed_drift,
                    issuer: issuer.clone(),
                }
            }
            AttestationRequest::SharedSecret { secret } => {
                if secret.trim().is_empty() {
                    return Err((Status::BadRequest, "Shared secret cannot be empty".into()));
                }
                let mut hasher = Sha256::new();
                hasher.update(secret.as_bytes());
                let digest = hasher.finalize();
                AttestationRequirement::SharedSecret {
                    hash: general_purpose::STANDARD.encode(digest),
                }
            }
        };
        metadata.attestation = Some(requirement);
    }

    if let Some(persistence) = body.persistence.as_ref() {
        let locator = match persistence {
            PersistenceRequest::Memory => PersistenceLocator::Memory,
            PersistenceRequest::Vault { key_path } => {
                if key_path.trim().is_empty() {
                    return Err((Status::BadRequest, "Vault key_path cannot be empty".into()));
                }
                PersistenceLocator::Vault {
                    key_path: key_path.clone(),
                }
            }
            PersistenceRequest::S3 { bucket, prefix } => {
                if bucket.trim().is_empty() {
                    return Err((Status::BadRequest, "S3 bucket cannot be empty".into()));
                }
                PersistenceLocator::S3 {
                    bucket: bucket.clone(),
                    prefix: prefix.clone(),
                }
            }
        };
        metadata.persistence = Some(locator);
    }

    if let Some(webhook) = body.webhook.as_ref() {
        if webhook.url.trim().is_empty() {
            return Err((Status::BadRequest, "Webhook url cannot be empty".into()));
        }
        metadata.webhook = Some(WebhookConfig {
            url: webhook.url.clone(),
            provider: webhook.provider.clone(),
            view_template: webhook.view_template.clone(),
            burn_template: webhook.burn_template.clone(),
        });
    }

    let content = if let Some(enc) = &body.encryption {
        let algorithm = enc.algorithm;
        match algorithm {
            EncryptionAlgorithm::None => StoredContent::Plain {
                text: body.content.clone(),
            },
            EncryptionAlgorithm::Aes256Gcm
            | EncryptionAlgorithm::ChaCha20Poly1305
            | EncryptionAlgorithm::XChaCha20Poly1305 => {
                encrypt_content(&body.content, &enc.key, algorithm)
                    .map_err(|e| (Status::BadRequest, e))?
            }
        }
    } else {
        StoredContent::Plain {
            text: body.content.clone(),
        }
    };

    let mut bundle_children: Vec<BundlePointer> = Vec::new();

    if let Some(bundle) = body.bundle.as_ref() {
        if !bundle.children.is_empty() {
            let enc = body.encryption.as_ref().ok_or_else(|| {
                (
                    Status::BadRequest,
                    "Bundle creation requires an encryption key".to_string(),
                )
            })?;

            if matches!(enc.algorithm, EncryptionAlgorithm::None) {
                return Err((
                    Status::BadRequest,
                    "Bundle creation requires a non-zero encryption algorithm".to_string(),
                ));
            }

            for child in &bundle.children {
                let encrypted_child = encrypt_content(&child.content, &enc.key, enc.algorithm)
                    .map_err(|e| {
                        (
                            Status::BadRequest,
                            format!("failed to encrypt bundle child: {e}"),
                        )
                    })?;
                let mut child_metadata = metadata.clone();
                child_metadata.bundle = None;
                child_metadata.bundle_label = child.label.clone();
                let child_paste = StoredPaste {
                    content: encrypted_child,
                    format: child.format.unwrap_or(format),
                    created_at: now,
                    expires_at,
                    burn_after_reading: true,
                    metadata: child_metadata,
                };
                let child_id = store.create_paste(child_paste).await;
                bundle_children.push(BundlePointer {
                    id: child_id,
                    label: child.label.clone(),
                });
            }
        }
    }

    if !bundle_children.is_empty() {
        metadata.bundle = Some(BundleMetadata {
            children: bundle_children,
        });
    }

    let paste = StoredPaste {
        content,
        format,
        created_at: now,
        expires_at,
        burn_after_reading,
        metadata,
    };

    let id = store.create_paste(paste).await;
    Ok(format!("/{}", id))
}

#[get("/<id>?<query..>")]
async fn show(
    store: &State<SharedPasteStore>,
    id: String,
    query: PasteViewQuery,
) -> Result<content::RawHtml<String>, Status> {
    match store.get_paste(&id).await {
        Ok(paste) => {
            let now = current_timestamp();
            if let Some(lock_state) = evaluate_time_lock(&paste.metadata, now) {
                return Ok(content::RawHtml(render_time_locked(&id, lock_state)));
            }

            if let Some(requirement) = paste.metadata.attestation.as_ref() {
                match verify_attestation(requirement, &query, now) {
                    AttestationVerdict::Granted => {}
                    AttestationVerdict::Prompt { invalid } => {
                        let needs_key_field =
                            matches!(paste.content, StoredContent::Encrypted { .. })
                                && query.key.is_none();
                        return Ok(content::RawHtml(render_attestation_prompt(
                            &id,
                            needs_key_field,
                            query.key.as_deref(),
                            requirement,
                            invalid,
                        )));
                    }
                }
            }

            match decrypt_content(&paste.content, query.key.as_deref()) {
                Ok(text) => {
                    let bundle_html = if let Some(bundle) = paste.metadata.bundle.clone() {
                        build_bundle_overview(store.inner().clone(), &bundle, &query).await
                    } else {
                        None
                    };

                    let webhook_config = paste.metadata.webhook.clone();
                    let mut events_to_fire = Vec::new();

                    if paste.burn_after_reading {
                        if let Some(config) = webhook_config.clone() {
                            events_to_fire.push((config.clone(), WebhookEvent::Viewed));
                        }
                    }

                    if paste.burn_after_reading {
                        let deleted = store.delete_paste(&id).await;
                        if deleted {
                            if let Some(config) = webhook_config.clone() {
                                events_to_fire.push((config, WebhookEvent::Consumed));
                            }
                        }
                    }

                    for (config, event) in events_to_fire {
                        trigger_webhook(config, event, &id, paste.metadata.bundle_label.clone());
                    }

                    Ok(content::RawHtml(render_paste_view(
                        &id,
                        &paste,
                        &text,
                        bundle_html,
                    )))
                }
                Err(DecryptError::MissingKey) => Ok(content::RawHtml(render_key_prompt(&id))),
                Err(DecryptError::InvalidKey) => Ok(content::RawHtml(render_invalid_key(&id))),
            }
        }
        Err(PasteError::NotFound(_)) => Err(Status::NotFound),
        Err(PasteError::Expired(_)) => Ok(content::RawHtml(render_expired(&id))),
    }
}

#[get("/raw/<id>?<query..>")]
async fn show_raw(
    store: &State<SharedPasteStore>,
    id: String,
    query: PasteViewQuery,
) -> Result<content::RawText<String>, Status> {
    match store.get_paste(&id).await {
        Ok(paste) => {
            let now = current_timestamp();
            if evaluate_time_lock(&paste.metadata, now).is_some() {
                return Err(Status::Locked);
            }

            if let Some(requirement) = paste.metadata.attestation.as_ref() {
                match verify_attestation(requirement, &query, now) {
                    AttestationVerdict::Granted => {}
                    AttestationVerdict::Prompt { invalid: false } => {
                        return Err(Status::Unauthorized);
                    }
                    AttestationVerdict::Prompt { invalid: true } => {
                        return Err(Status::Forbidden);
                    }
                }
            }

            match decrypt_content(&paste.content, query.key.as_deref()) {
                Ok(text) => {
                    if paste.burn_after_reading {
                        let webhook_config = paste.metadata.webhook.clone();
                        if let Some(config) = webhook_config.clone() {
                            trigger_webhook(
                                config,
                                WebhookEvent::Viewed,
                                &id,
                                paste.metadata.bundle_label.clone(),
                            );
                        }
                        let deleted = store.delete_paste(&id).await;
                        if deleted {
                            if let Some(config) = paste.metadata.webhook.clone() {
                                trigger_webhook(
                                    config,
                                    WebhookEvent::Consumed,
                                    &id,
                                    paste.metadata.bundle_label.clone(),
                                );
                            }
                        }
                    }
                    Ok(content::RawText(text))
                }
                Err(DecryptError::MissingKey) => Err(Status::Unauthorized),
                Err(DecryptError::InvalidKey) => Err(Status::Forbidden),
            }
        }
        Err(PasteError::NotFound(_)) => Err(Status::NotFound),
        Err(PasteError::Expired(_)) => Err(Status::Gone),
    }
}

#[get("/static/<path..>")]
async fn static_files(path: PathBuf) -> Option<NamedFile> {
    NamedFile::open(PathBuf::from("static").join(path))
        .await
        .ok()
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

fn encrypt_content(
    text: &str,
    key: &str,
    algorithm: EncryptionAlgorithm,
) -> Result<StoredContent, String> {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);

    let derived = derive_key_material(key, &salt);

    match algorithm {
        EncryptionAlgorithm::None => Ok(StoredContent::Plain {
            text: text.to_owned(),
        }),
        EncryptionAlgorithm::Aes256Gcm => {
            let cipher = Aes256Gcm::new_from_slice(&derived)
                .map_err(|_| "failed to initialise cipher".to_string())?;
            let mut nonce_bytes = [0u8; 12];
            OsRng.fill_bytes(&mut nonce_bytes);
            let nonce = AesNonce::from(nonce_bytes);

            let ciphertext = cipher
                .encrypt(&nonce, text.as_bytes())
                .map_err(|_| "failed to encrypt content".to_string())?;

            Ok(StoredContent::Encrypted {
                algorithm,
                ciphertext: general_purpose::STANDARD.encode(ciphertext),
                nonce: general_purpose::STANDARD.encode(nonce_bytes),
                salt: general_purpose::STANDARD.encode(salt),
            })
        }
        EncryptionAlgorithm::ChaCha20Poly1305 => {
            let cipher = ChaCha20Poly1305::new_from_slice(&derived)
                .map_err(|_| "failed to initialise cipher".to_string())?;
            let mut nonce_bytes = [0u8; 12];
            OsRng.fill_bytes(&mut nonce_bytes);
            let nonce = ChaNonce::from(nonce_bytes);

            let ciphertext = cipher
                .encrypt(&nonce, text.as_bytes())
                .map_err(|_| "failed to encrypt content".to_string())?;

            Ok(StoredContent::Encrypted {
                algorithm,
                ciphertext: general_purpose::STANDARD.encode(ciphertext),
                nonce: general_purpose::STANDARD.encode(nonce_bytes),
                salt: general_purpose::STANDARD.encode(salt),
            })
        }
        EncryptionAlgorithm::XChaCha20Poly1305 => {
            let cipher = XChaCha20Poly1305::new_from_slice(&derived)
                .map_err(|_| "failed to initialise cipher".to_string())?;
            let mut nonce_bytes = [0u8; 24];
            OsRng.fill_bytes(&mut nonce_bytes);
            let nonce = XNonce::from(nonce_bytes);

            let ciphertext = cipher
                .encrypt(&nonce, text.as_bytes())
                .map_err(|_| "failed to encrypt content".to_string())?;

            Ok(StoredContent::Encrypted {
                algorithm,
                ciphertext: general_purpose::STANDARD.encode(ciphertext),
                nonce: general_purpose::STANDARD.encode(nonce_bytes),
                salt: general_purpose::STANDARD.encode(salt),
            })
        }
    }
}

fn decrypt_content(content: &StoredContent, key: Option<&str>) -> Result<String, DecryptError> {
    match content {
        StoredContent::Plain { text } => Ok(text.clone()),
        StoredContent::Encrypted {
            algorithm,
            ciphertext,
            nonce,
            salt,
        } => {
            let key = key.ok_or(DecryptError::MissingKey)?;
            let salt_bytes = general_purpose::STANDARD
                .decode(salt)
                .map_err(|_| DecryptError::InvalidKey)?;
            let nonce_bytes_vec = general_purpose::STANDARD
                .decode(nonce)
                .map_err(|_| DecryptError::InvalidKey)?;
            let cipher_bytes = general_purpose::STANDARD
                .decode(ciphertext)
                .map_err(|_| DecryptError::InvalidKey)?;

            let derived = derive_key_material(key, &salt_bytes);

            match algorithm {
                EncryptionAlgorithm::None => {
                    String::from_utf8(cipher_bytes).map_err(|_| DecryptError::InvalidKey)
                }
                EncryptionAlgorithm::Aes256Gcm => {
                    let cipher = Aes256Gcm::new_from_slice(&derived)
                        .map_err(|_| DecryptError::InvalidKey)?;
                    let nonce_array: [u8; 12] = nonce_bytes_vec
                        .try_into()
                        .map_err(|_| DecryptError::InvalidKey)?;
                    let nonce = AesNonce::from(nonce_array);

                    cipher
                        .decrypt(&nonce, cipher_bytes.as_ref())
                        .map_err(|_| DecryptError::InvalidKey)
                        .and_then(|bytes| {
                            String::from_utf8(bytes).map_err(|_| DecryptError::InvalidKey)
                        })
                }
                EncryptionAlgorithm::ChaCha20Poly1305 => {
                    let cipher = ChaCha20Poly1305::new_from_slice(&derived)
                        .map_err(|_| DecryptError::InvalidKey)?;
                    let nonce_array: [u8; 12] = nonce_bytes_vec
                        .try_into()
                        .map_err(|_| DecryptError::InvalidKey)?;
                    let nonce = ChaNonce::from(nonce_array);

                    cipher
                        .decrypt(&nonce, cipher_bytes.as_ref())
                        .map_err(|_| DecryptError::InvalidKey)
                        .and_then(|bytes| {
                            String::from_utf8(bytes).map_err(|_| DecryptError::InvalidKey)
                        })
                }
                EncryptionAlgorithm::XChaCha20Poly1305 => {
                    let cipher = XChaCha20Poly1305::new_from_slice(&derived)
                        .map_err(|_| DecryptError::InvalidKey)?;
                    let nonce_array: [u8; 24] = nonce_bytes_vec
                        .try_into()
                        .map_err(|_| DecryptError::InvalidKey)?;
                    let nonce = XNonce::from(nonce_array);

                    cipher
                        .decrypt(&nonce, cipher_bytes.as_ref())
                        .map_err(|_| DecryptError::InvalidKey)
                        .and_then(|bytes| {
                            String::from_utf8(bytes).map_err(|_| DecryptError::InvalidKey)
                        })
                }
            }
        }
    }
}

fn derive_key_material(key: &str, salt: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(salt);
    hasher.update(key.as_bytes());
    hasher.finalize().into()
}

fn render_paste_view(
    id: &str,
    paste: &StoredPaste,
    text: &str,
    bundle_html: Option<String>,
) -> String {
    let rendered_body = match paste.format {
        PasteFormat::PlainText => format_plain(text),
        PasteFormat::Markdown => format_markdown(text),
        PasteFormat::Code
        | PasteFormat::Go
        | PasteFormat::Cpp
        | PasteFormat::Kotlin
        | PasteFormat::Java => format_code(text),
        PasteFormat::Json => format_json(text),
    };

    let created = DateTime::from_timestamp(paste.created_at, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let expires = paste.expires_at.and_then(|ts| {
        DateTime::from_timestamp(ts, 0).map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
    });

    let encryption = match paste.content {
        StoredContent::Plain { .. } => "None".to_string(),
        StoredContent::Encrypted { ref algorithm, .. } => match algorithm {
            EncryptionAlgorithm::None => "None".to_string(),
            EncryptionAlgorithm::Aes256Gcm => "AES-256-GCM".to_string(),
            EncryptionAlgorithm::ChaCha20Poly1305 => "ChaCha20-Poly1305".to_string(),
            EncryptionAlgorithm::XChaCha20Poly1305 => "XChaCha20-Poly1305".to_string(),
        },
    };

    let burn_status = if paste.burn_after_reading {
        "Yes (link disabled after this view)".to_string()
    } else {
        "No".to_string()
    };

    let burn_note = if paste.burn_after_reading {
        r#"<p class="burn-note">This paste was configured to burn after reading. The link is now invalid for future visits.</p>"#.to_string()
    } else {
        String::new()
    };

    let time_lock = match (paste.metadata.not_before, paste.metadata.not_after) {
        (None, None) => "None".to_string(),
        (Some(start), Some(end)) => {
            format!("{} → {}", format_timestamp(start), format_timestamp(end))
        }
        (Some(start), None) => format!("After {}", format_timestamp(start)),
        (None, Some(end)) => format!("Before {}", format_timestamp(end)),
    };

    let attestation = match paste.metadata.attestation {
        None => "None".to_string(),
        Some(AttestationRequirement::Totp { ref issuer, .. }) => issuer
            .as_ref()
            .map(|iss| format!("TOTP ({iss})"))
            .unwrap_or_else(|| "TOTP".to_string()),
        Some(AttestationRequirement::SharedSecret { .. }) => "Shared secret".to_string(),
    };

    let persistence = paste
        .metadata
        .persistence
        .as_ref()
        .map(|locator| match locator {
            PersistenceLocator::Memory => "Memory".to_string(),
            PersistenceLocator::Vault { key_path } => format!("Vault ({})", key_path),
            PersistenceLocator::S3 { bucket, prefix } => match prefix {
                Some(p) if !p.is_empty() => format!("S3 {bucket}/{p}"),
                _ => format!("S3 {bucket}"),
            },
        })
        .unwrap_or_else(|| "Ephemeral".to_string());

    let webhook = paste
        .metadata
        .webhook
        .as_ref()
        .map(|config| match config.provider {
            Some(WebhookProvider::Slack) => "Slack".to_string(),
            Some(WebhookProvider::Teams) => "Teams".to_string(),
            Some(WebhookProvider::Generic) => "Webhook".to_string(),
            None => "Webhook".to_string(),
        })
        .unwrap_or_else(|| "None".to_string());

    let bundle_summary = paste
        .metadata
        .bundle
        .as_ref()
        .map(|bundle| format!("{} link(s)", bundle.children.len()))
        .unwrap_or_else(|| "None".to_string());

    let bundle_section = bundle_html.unwrap_or_default();

    layout(
        "copypaste.fyi | View paste",
        format!(
            r#"<section class="meta">
    <div><strong>ID:</strong> {id}</div>
    <div><strong>Format:</strong> {format:?}</div>
    <div><strong>Created:</strong> {created}</div>
    <div><strong>Retention:</strong> {retention}</div>
    <div><strong>Encryption:</strong> {encryption}</div>
    <div><strong>Burn after reading:</strong> {burn}</div>
    <div><strong>Time lock:</strong> {time_lock}</div>
    <div><strong>Attestation:</strong> {attestation}</div>
    <div><strong>Persistence:</strong> {persistence}</div>
    <div><strong>Webhook:</strong> {webhook}</div>
    <div><strong>Bundle:</strong> {bundle_summary}</div>
</section>
<article class="content">
    {burn_note}
    {bundle_section}
    {rendered_body}
</article>
"#,
            id = encode_safe(id),
            format = paste.format,
            created = created,
            retention = expires.unwrap_or_else(|| "No expiry".to_string()),
            encryption = encryption,
            burn = burn_status,
            burn_note = burn_note,
            time_lock = encode_safe(&time_lock),
            attestation = encode_safe(&attestation),
            persistence = encode_safe(&persistence),
            webhook = encode_safe(&webhook),
            bundle_summary = encode_safe(&bundle_summary),
            bundle_section = bundle_section,
            rendered_body = rendered_body,
        ),
    )
}

#[derive(Copy, Clone)]
enum TimeLockState {
    TooEarly(i64),
    TooLate(i64),
}

fn evaluate_time_lock(metadata: &PasteMetadata, now: i64) -> Option<TimeLockState> {
    if let Some(not_before) = metadata.not_before {
        if now < not_before {
            return Some(TimeLockState::TooEarly(not_before));
        }
    }
    if let Some(not_after) = metadata.not_after {
        if now > not_after {
            return Some(TimeLockState::TooLate(not_after));
        }
    }
    None
}

fn render_time_locked(_id: &str, state: TimeLockState) -> String {
    let (heading, message) = match state {
        TimeLockState::TooEarly(ts) => (
            "Time-locked paste",
            format!(
                "This paste unlocks after {}.",
                encode_safe(&format_timestamp(ts))
            ),
        ),
        TimeLockState::TooLate(ts) => (
            "Time window elapsed",
            format!(
                "Access window closed at {}.",
                encode_safe(&format_timestamp(ts))
            ),
        ),
    };

    layout(
        "copypaste.fyi | Locked",
        format!(
            r#"<section class="notice">
    <h2>{heading}</h2>
    <p>{message}</p>
    <p class="hint">Bookmark this link and try again when the unlock window is active.</p>
</section>
"#,
            heading = heading,
            message = message,
        ),
    )
}

#[derive(Copy, Clone)]
enum AttestationVerdict {
    Granted,
    Prompt { invalid: bool },
}

fn verify_attestation(
    requirement: &AttestationRequirement,
    query: &PasteViewQuery,
    now: i64,
) -> AttestationVerdict {
    match requirement {
        AttestationRequirement::Totp {
            secret,
            digits,
            step,
            allowed_drift,
            ..
        } => {
            let code = match query.code.as_deref() {
                Some(value) if !value.trim().is_empty() => value.trim(),
                _ => return AttestationVerdict::Prompt { invalid: false },
            };
            if verify_totp(secret, code, *digits, *step, *allowed_drift, now) {
                AttestationVerdict::Granted
            } else {
                AttestationVerdict::Prompt { invalid: true }
            }
        }
        AttestationRequirement::SharedSecret { hash } => {
            let provided = match query.attest.as_deref() {
                Some(value) if !value.is_empty() => value,
                _ => return AttestationVerdict::Prompt { invalid: false },
            };
            let mut hasher = Sha256::new();
            hasher.update(provided.as_bytes());
            let digest = hasher.finalize();
            let encoded = general_purpose::STANDARD.encode(digest);
            if &encoded == hash {
                AttestationVerdict::Granted
            } else {
                AttestationVerdict::Prompt { invalid: true }
            }
        }
    }
}

fn render_attestation_prompt(
    id: &str,
    needs_key_field: bool,
    existing_key: Option<&str>,
    requirement: &AttestationRequirement,
    invalid: bool,
) -> String {
    let (prompt_label, field_name, field_type, helper) = match requirement {
        AttestationRequirement::Totp { issuer, .. } => (
            issuer
                .as_ref()
                .map(|name| format!("One-time code ({name})"))
                .unwrap_or_else(|| "One-time code".to_string()),
            "code",
            "text",
            "Enter the current code from your authenticator.",
        ),
        AttestationRequirement::SharedSecret { .. } => (
            "Shared secret".to_string(),
            "attest",
            "password",
            "Provide the shared secret agreed upon with the sender.",
        ),
    };

    let mut form_inputs = String::new();

    if needs_key_field {
        form_inputs.push_str(
            r#"        <label for="key">Encryption key</label>
        <input type="password" name="key" id="key" required />
"#,
        );
    } else if let Some(key) = existing_key {
        let escaped = encode_safe(key);
        form_inputs.push_str(&format!(
            "        <input type=\"hidden\" name=\"key\" value=\"{escaped}\" />\n"
        ));
    }

    form_inputs.push_str(&format!(
        "        <label for=\"{field_name}\">{prompt_label}</label>\n",
        field_name = field_name,
        prompt_label = encode_safe(&prompt_label),
    ));

    let mut field_attributes = String::new();
    if matches!(requirement, AttestationRequirement::Totp { .. }) {
        field_attributes.push_str(" pattern=\"[0-9]{6,10}\"");
        field_attributes.push_str(" inputmode=\"numeric\"");
    }

    form_inputs.push_str(&format!(
        "        <input type=\"{field_type}\" name=\"{field_name}\" id=\"{field_name}\" required{attrs} />\n",
        field_type = field_type,
        field_name = field_name,
        attrs = field_attributes,
    ));

    let error = if invalid {
        "<p class=\"error\">Verification failed. Double-check your entry and try again.</p>\n"
            .to_string()
    } else {
        String::new()
    };

    layout(
        "copypaste.fyi | Verification required",
        format!(
            r#"<section class="notice">
    <h2>Additional verification required</h2>
    <p>{helper}</p>
    {error}
    <form method="get" action="/{id}">
{inputs}        <button type="submit">Continue</button>
    </form>
</section>
"#,
            helper = encode_safe(helper),
            error = error,
            inputs = form_inputs,
            id = encode_safe(id),
        ),
    )
}

fn format_timestamp(ts: i64) -> String {
    DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| ts.to_string())
}

async fn build_bundle_overview(
    store: SharedPasteStore,
    bundle: &BundleMetadata,
    query: &PasteViewQuery,
) -> Option<String> {
    if bundle.children.is_empty() {
        return None;
    }

    let mut items = String::new();
    for (idx, child) in bundle.children.iter().enumerate() {
        let label = child.label.as_deref().unwrap_or("");
        let label_display = if label.is_empty() {
            format!("Share {}", idx + 1)
        } else {
            label.to_string()
        };

        let status = match store.get_paste(&child.id).await {
            Ok(_) => ("available", "Available"),
            Err(PasteError::Expired(_)) => ("expired", "Expired"),
            Err(PasteError::NotFound(_)) => ("consumed", "Consumed"),
        };

        let url = build_child_url(&child.id, query);
        items.push_str(&format!(
            r#"        <li>
            <div class="bundle-link">
                <a href="{url}">{label}</a>
                <span class="status {class}">{status}</span>
                <code>{id}</code>
            </div>
        </li>
"#,
            url = encode_safe(&url),
            label = encode_safe(&label_display),
            class = status.0,
            status = status.1,
            id = encode_safe(&child.id),
        ));
    }

    Some(format!(
        r#"<section class="bundle">
    <h2>Bundle shares</h2>
    <p>Each child paste burns after the first successful view.</p>
    <ul class="bundle-links">
{items}    </ul>
</section>
"#,
        items = items,
    ))
}

fn build_child_url(child_id: &str, query: &PasteViewQuery) -> String {
    if let Some(key) = query.key.as_ref() {
        format!("/{child_id}?key={}", urlencoding::encode(key))
    } else {
        format!("/{child_id}")
    }
}

fn parse_timestamp(input: &str) -> Result<i64, String> {
    if let Ok(value) = input.parse::<i64>() {
        return Ok(value);
    }
    DateTime::parse_from_rfc3339(input)
        .map(|dt| dt.timestamp())
        .map_err(|_| "expected UNIX seconds or RFC3339 timestamp".to_string())
}

type HmacSha1 = Hmac<Sha1>;

fn verify_totp(
    secret: &str,
    code: &str,
    digits: u32,
    step: u64,
    allowed_drift: u32,
    now: i64,
) -> bool {
    let secret_bytes = match decode_totp_secret(secret) {
        Some(bytes) => bytes,
        None => return false,
    };

    let sanitized_code: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
    if sanitized_code.len() != digits as usize {
        return false;
    }

    let now = if now.is_negative() { 0 } else { now as u64 };
    if step == 0 {
        return false;
    }
    let counter = now / step;

    for offset in -(allowed_drift as i32)..=(allowed_drift as i32) {
        let adjusted_counter = if offset < 0 {
            counter.checked_sub(offset.unsigned_abs() as u64)
        } else {
            counter.checked_add(offset as u64)
        };

        let Some(candidate_counter) = adjusted_counter else {
            continue;
        };
        if let Some(candidate) = totp_code(&secret_bytes, candidate_counter, digits) {
            if candidate == sanitized_code {
                return true;
            }
        }
    }

    false
}

fn decode_totp_secret(secret: &str) -> Option<Vec<u8>> {
    let normalized: String = secret
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();
    BASE32.decode(normalized.as_bytes()).ok()
}

fn totp_code(secret: &[u8], counter: u64, digits: u32) -> Option<String> {
    let mut mac = <HmacSha1 as Mac>::new_from_slice(secret).ok()?;
    mac.update(&counter.to_be_bytes());
    let result = mac.finalize().into_bytes();
    let offset = (result[result.len() - 1] & 0x0f) as usize;
    if offset + 4 > result.len() {
        return None;
    }
    let slice = &result[offset..offset + 4];
    let binary: u32 = ((slice[0] as u32 & 0x7f) << 24)
        | ((slice[1] as u32) << 16)
        | ((slice[2] as u32) << 8)
        | (slice[3] as u32);
    let modulo = 10u64.pow(digits);
    let value = (binary as u64) % modulo;
    Some(format!("{:0width$}", value, width = digits as usize))
}

#[derive(Clone, Copy)]
enum WebhookEvent {
    Viewed,
    Consumed,
}

fn trigger_webhook(
    config: WebhookConfig,
    event: WebhookEvent,
    paste_id: &str,
    bundle_label: Option<String>,
) {
    let id = paste_id.to_string();
    tokio::spawn(async move {
        if let Err(err) = send_webhook(config, event, id, bundle_label).await {
            eprintln!("webhook dispatch failed: {err}");
        }
    });
}

async fn send_webhook(
    config: WebhookConfig,
    event: WebhookEvent,
    paste_id: String,
    bundle_label: Option<String>,
) -> Result<(), reqwest::Error> {
    let client = reqwest::Client::new();
    let message = resolve_webhook_message(&config, event, &paste_id, bundle_label.as_deref());
    let payload = match config.provider {
        Some(WebhookProvider::Slack) | Some(WebhookProvider::Generic) | None => {
            serde_json::json!({ "text": message })
        }
        Some(WebhookProvider::Teams) => serde_json::json!({ "text": message }),
    };

    client
        .post(&config.url)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

fn resolve_webhook_message(
    config: &WebhookConfig,
    event: WebhookEvent,
    paste_id: &str,
    bundle_label: Option<&str>,
) -> String {
    let template = match event {
        WebhookEvent::Viewed => config.view_template.as_deref(),
        WebhookEvent::Consumed => config.burn_template.as_deref(),
    };

    let default = match event {
        WebhookEvent::Viewed => {
            if let Some(label) = bundle_label {
                format!("Bundle share '{label}' for paste {paste_id} was opened")
            } else {
                format!("Paste {paste_id} was opened")
            }
        }
        WebhookEvent::Consumed => {
            if let Some(label) = bundle_label {
                format!("Bundle share '{label}' for paste {paste_id} was consumed")
            } else {
                format!("Paste {paste_id} self-destructed")
            }
        }
    };

    if let Some(tpl) = template {
        apply_template(
            tpl,
            paste_id,
            bundle_label,
            match event {
                WebhookEvent::Viewed => "viewed",
                WebhookEvent::Consumed => "consumed",
            },
        )
    } else {
        default
    }
}

fn apply_template(template: &str, id: &str, label: Option<&str>, event: &str) -> String {
    let mut result = template.replace("{{id}}", id);
    result = result.replace("{{event}}", event);
    result = result.replace("{{label}}", label.unwrap_or(""));
    result
}

fn render_key_prompt(id: &str) -> String {
    layout(
        "copypaste.fyi | Encrypted paste",
        format!(
            r#"<section class="notice">
    <h2>This paste is encrypted</h2>
    <p>Provide the encryption key to view the content.</p>
    <form method="get" action="/{id}">
        <label for="key">Encryption key</label>
        <input type="password" name="key" id="key" required />
        <button type="submit">Decrypt</button>
    </form>
</section>
"#,
            id = encode_safe(id),
        ),
    )
}

fn render_invalid_key(id: &str) -> String {
    layout(
        "copypaste.fyi | Invalid key",
        format!(
            r#"<section class="notice error">
    <h2>Invalid encryption key</h2>
    <p>The key you entered could not decrypt this paste.</p>
    <form method="get" action="/{id}">
        <label for="key">Try again</label>
        <input type="password" name="key" id="key" required />
        <button type="submit">Decrypt</button>
    </form>
</section>
"#,
            id = encode_safe(id),
        ),
    )
}

fn render_expired(id: &str) -> String {
    layout(
        "copypaste.fyi | Paste expired",
        format!(
            r#"<section class="notice error">
    <h2>Paste expired</h2>
    <p>Paste {id} has reached its retention limit and is no longer available.</p>
</section>
"#,
            id = encode_safe(id),
        ),
    )
}

fn format_plain(text: &str) -> String {
    format!("<pre>{}</pre>", encode_safe(text))
}

fn format_markdown(text: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(text, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

fn format_code(text: &str) -> String {
    format!("<pre><code>{}</code></pre>", encode_safe(text))
}

fn format_json(text: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) => {
            format_code(&serde_json::to_string_pretty(&value).unwrap_or_else(|_| text.to_string()))
        }
        Err(_) => format_code(text),
    }
}

fn layout(title: &str, body: String) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{title}</title>
    <link rel="stylesheet" href="/static/view.css" />
</head>
<body>
    <header>
        <h1><a href="/">copypaste.fyi</a></h1>
    </header>
    <main>
        {body}
    </main>
</body>
</html>
"#,
        title = encode_safe(title),
        body = body,
    )
}
