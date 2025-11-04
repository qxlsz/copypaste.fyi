use copypaste::server::handlers;

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    handlers::launch().await
}

#[cfg(test)]
mod tests {
    use base64::engine::general_purpose;
    use base64::Engine;
    use copypaste::server::crypto::{decrypt_content, encrypt_content, DecryptError};
    use copypaste::server::handlers::build_rocket;
    use copypaste::server::render::format_json;
    use copypaste::server::time::current_timestamp;
    use copypaste::{
        create_paste_store, AttestationRequirement, EncryptionAlgorithm, MemoryPasteStore,
        PasteFormat, PasteMetadata, SharedPasteStore, StoredContent, StoredPaste,
    };
    use rocket::http::{ContentType, Status};
    use rocket::local::asynchronous::Client;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::sync::Arc;
    use urlencoding::encode;

    async fn rocket_client() -> Client {
        Client::tracked(build_rocket(create_paste_store()))
            .await
            .expect("valid rocket instance")
    }

    async fn rocket_client_with_store(store: SharedPasteStore) -> Client {
        Client::tracked(build_rocket(store))
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
    async fn raw_endpoint_requires_key_for_encrypted_content() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::default());
        let encrypted = encrypt_content(
            "stealth payload",
            "super-secret",
            EncryptionAlgorithm::Aes256Gcm,
        )
        .expect("encryption successful");

        let metadata = PasteMetadata::default();
        let paste = StoredPaste {
            content: encrypted,
            format: PasteFormat::PlainText,
            created_at: current_timestamp(),
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
            .get(format!("/{id}?attest={}", encode(secret)))
            .dispatch()
            .await;
        let ok_html = ok.into_string().await.expect("html");
        assert!(ok_html.contains("attested"));
    }

    #[test]
    fn encrypt_then_decrypt_roundtrip() {
        let key = "correct horse battery staple";
        let stored =
            encrypt_content("super secret", key, EncryptionAlgorithm::Aes256Gcm).expect("encrypt");
        let decrypted = decrypt_content(&stored, Some(key)).expect("decrypt");
        assert_eq!(decrypted, "super secret");
    }

    #[test]
    fn chacha_roundtrip() {
        let key = "tachyon-vector-2048";
        let stored = encrypt_content("ghost signal", key, EncryptionAlgorithm::ChaCha20Poly1305)
            .expect("encrypt");
        let decrypted = decrypt_content(&stored, Some(key)).expect("decrypt");
        assert_eq!(decrypted, "ghost signal");
    }

    #[test]
    fn xchacha_roundtrip() {
        let key = "tachyon-subroutine-7331";
        let stored = encrypt_content("link shell", key, EncryptionAlgorithm::XChaCha20Poly1305)
            .expect("encrypt");
        let decrypted = decrypt_content(&stored, Some(key)).expect("decrypt");
        assert_eq!(decrypted, "link shell");
    }

    #[test]
    fn decrypt_requires_key_for_encrypted_content() {
        let stored = encrypt_content(
            "classified",
            "moonbase",
            EncryptionAlgorithm::XChaCha20Poly1305,
        )
        .expect("encrypt");
        match decrypt_content(&stored, None) {
            Err(DecryptError::MissingKey) => {}
            other => panic!("expected missing key error, got {:?}", other),
        }
    }

    #[test]
    fn format_json_pretty_prints() {
        let result = format_json(r#"{"foo":1,"bar":[true,false]}"#);
        assert!(result.contains('\n'));
        assert!(result.starts_with("<pre><code>"));
        assert!(result.contains("&quot;foo&quot;"));
    }

    #[rocket::async_test]
    async fn time_locked_paste_is_protected() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::default());
        let future_unlock = current_timestamp() + 3600;
        let metadata = PasteMetadata {
            not_before: Some(future_unlock),
            ..Default::default()
        };

        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "sealed".into(),
            },
            format: PasteFormat::PlainText,
            created_at: current_timestamp(),
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
        let client = rocket_client_with_store(store.clone()).await;

        let gated = client.get(format!("/{id}")).dispatch().await;
        assert_eq!(gated.status(), Status::Ok);
        let gated_html = gated.into_string().await.expect("html body");
        assert!(gated_html.contains("Time-locked paste"));

        let raw = client.get(format!("/raw/{id}")).dispatch().await;
        assert_eq!(raw.status(), Status::Locked);
    }

    #[rocket::async_test]
    async fn bundle_creation_requires_encryption() {
        let client = rocket_client().await;
        let payload = json!({
            "content": "parent plain",
            "format": "plain_text",
            "bundle": {
                "children": [
                    { "content": "child secret" }
                ]
            }
        });

        let response = client
            .post("/")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch()
            .await;

        assert_eq!(response.status(), Status::BadRequest);
        let message = response.into_string().await.expect("error body");
        assert!(message.contains("requires an encryption key"));
    }

    #[rocket::async_test]
    async fn bundle_creation_renders_children() {
        let client = rocket_client().await;
        let payload = json!({
            "content": "parent encrypted",
            "format": "plain_text",
            "encryption": {
                "algorithm": "aes256_gcm",
                "key": "bundle-pass"
            },
            "bundle": {
                "children": [
                    {
                        "content": "child payload",
                        "label": "child-one"
                    }
                ]
            }
        });

        let response = client
            .post("/")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch()
            .await;

        assert_eq!(response.status(), Status::Ok);
        let path = response.into_string().await.expect("path body");

        let html_response = client
            .get(format!("{}?key=bundle-pass", path))
            .dispatch()
            .await;
        assert_eq!(html_response.status(), Status::Ok);
        let html = html_response.into_string().await.expect("html body");
        assert!(html.contains("Bundle shares"));
        assert!(html.contains("child-one"));
    }
}
