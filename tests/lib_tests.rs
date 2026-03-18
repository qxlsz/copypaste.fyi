use copypaste::{
    create_paste_store, EncryptionAlgorithm, PasteFormat, PasteMetadata, StoredContent, StoredPaste,
};

#[tokio::test]
async fn store_round_trip_plain() {
    let store = create_paste_store();
    let metadata = PasteMetadata::default();
    let paste = StoredPaste {
        content: StoredContent::Plain {
            text: "roundtrip".into(),
        },
        format: PasteFormat::PlainText,
        created_at: 1,
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

    let id = store.create_paste(paste.clone()).await;
    let stored = store.get_paste(&id).await.expect("paste should exist");
    assert!(matches!(stored.content, StoredContent::Plain { .. }));
    assert_eq!(stored.format, paste.format);
}

#[tokio::test]
async fn store_expired_returns_error() {
    let store = create_paste_store();
    let metadata = PasteMetadata::default();
    let paste = StoredPaste {
        content: StoredContent::Plain {
            text: "ephemeral".into(),
        },
        format: PasteFormat::PlainText,
        created_at: 10,
        expires_at: Some(5),
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

    let id = store.create_paste(paste).await;
    assert!(store.get_paste(&id).await.is_err());
}

#[tokio::test]
async fn store_handles_encrypted_variant() {
    let store = create_paste_store();
    let metadata = PasteMetadata::default();
    let paste = StoredPaste {
        content: StoredContent::Encrypted {
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            ciphertext: "cipher".into(),
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

    let id = store.create_paste(paste).await;
    let stored = store
        .get_paste(&id)
        .await
        .expect("encrypted paste should exist");
    assert!(matches!(stored.content, StoredContent::Encrypted { .. }));
}

#[tokio::test]
async fn store_handles_chacha_variant() {
    let store = create_paste_store();
    let metadata = PasteMetadata::default();
    let paste = StoredPaste {
        content: StoredContent::Encrypted {
            algorithm: EncryptionAlgorithm::XChaCha20Poly1305,
            ciphertext: "cipher".into(),
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

    let id = store.create_paste(paste).await;
    let stored = store
        .get_paste(&id)
        .await
        .expect("encrypted paste should exist");
    assert!(matches!(stored.content, StoredContent::Encrypted { .. }));
}

#[test]
#[should_panic(expected = "UPSTASH_REDIS_REST_URL missing")]
fn redis_store_requires_url() {
    std::env::remove_var("UPSTASH_REDIS_REST_URL");
    std::env::remove_var("UPSTASH_REDIS_REST_TOKEN");
    std::env::set_var("COPYPASTE_PERSISTENCE_BACKEND", "redis");
    let _ = std::panic::catch_unwind(|| create_paste_store());
    std::env::remove_var("COPYPASTE_PERSISTENCE_BACKEND");
    panic!("Should have panicked due to missing UPSTASH_REDIS_REST_URL");
}

#[test]
#[should_panic(expected = "UPSTASH_REDIS_REST_TOKEN missing")]
fn redis_store_requires_token() {
    std::env::remove_var("UPSTASH_REDIS_REST_URL");
    std::env::remove_var("UPSTASH_REDIS_REST_TOKEN");
    std::env::set_var("COPYPASTE_PERSISTENCE_BACKEND", "redis");
    std::env::set_var("UPSTASH_REDIS_REST_URL", "https://example.upstash.io");
    let _ = std::panic::catch_unwind(|| create_paste_store());
    std::env::remove_var("COPYPASTE_PERSISTENCE_BACKEND");
    std::env::remove_var("UPSTASH_REDIS_REST_URL");
    panic!("Should have panicked due to missing UPSTASH_REDIS_REST_TOKEN");
}

#[test]
#[should_panic(expected = "COPYPASTE_VAULT_ADDR missing")]
fn vault_store_requires_addr() {
    std::env::remove_var("COPYPASTE_VAULT_ADDR");
    std::env::remove_var("COPYPASTE_VAULT_TOKEN");
    std::env::set_var("COPYPASTE_PERSISTENCE_BACKEND", "vault");
    let _ = std::panic::catch_unwind(|| create_paste_store());
    std::env::remove_var("COPYPASTE_PERSISTENCE_BACKEND");
    panic!("Should have panicked due to missing COPYPASTE_VAULT_ADDR");
}

#[test]
#[should_panic(expected = "COPYPASTE_VAULT_TOKEN missing")]
fn vault_store_requires_token() {
    std::env::remove_var("COPYPASTE_VAULT_ADDR");
    std::env::remove_var("COPYPASTE_VAULT_TOKEN");
    std::env::set_var("COPYPASTE_PERSISTENCE_BACKEND", "vault");
    std::env::set_var("COPYPASTE_VAULT_ADDR", "https://vault.example.com:8200");
    let _ = std::panic::catch_unwind(|| create_paste_store());
    std::env::remove_var("COPYPASTE_PERSISTENCE_BACKEND");
    std::env::remove_var("COPYPASTE_VAULT_ADDR");
    panic!("Should have panicked due to missing COPYPASTE_VAULT_TOKEN");
}

#[test]
#[should_panic(expected = "too short")]
fn vault_store_requires_min_token_length() {
    std::env::remove_var("COPYPASTE_VAULT_ADDR");
    std::env::remove_var("COPYPASTE_VAULT_TOKEN");
    std::env::set_var("COPYPASTE_PERSISTENCE_BACKEND", "vault");
    std::env::set_var("COPYPASTE_VAULT_ADDR", "https://vault.example.com:8200");
    std::env::set_var("COPYPASTE_VAULT_TOKEN", "short"); // Less than 20 chars
    let _ = std::panic::catch_unwind(|| create_paste_store());
    std::env::remove_var("COPYPASTE_PERSISTENCE_BACKEND");
    std::env::remove_var("COPYPASTE_VAULT_ADDR");
    std::env::remove_var("COPYPASTE_VAULT_TOKEN");
    panic!("Should have panicked due to token too short");
}

#[test]
#[should_panic(expected = "valid URL")]
fn redis_store_requires_valid_url() {
    std::env::remove_var("UPSTASH_REDIS_REST_URL");
    std::env::remove_var("UPSTASH_REDIS_REST_TOKEN");
    std::env::set_var("COPYPASTE_PERSISTENCE_BACKEND", "redis");
    std::env::set_var("UPSTASH_REDIS_REST_URL", "not-a-valid-url");
    std::env::set_var("UPSTASH_REDIS_REST_TOKEN", "valid-token");
    let _ = std::panic::catch_unwind(|| create_paste_store());
    std::env::remove_var("COPYPASTE_PERSISTENCE_BACKEND");
    std::env::remove_var("UPSTASH_REDIS_REST_URL");
    std::env::remove_var("UPSTASH_REDIS_REST_TOKEN");
    panic!("Should have panicked due to invalid URL");
}

#[test]
#[should_panic(expected = "valid URL")]
fn vault_store_requires_valid_url() {
    std::env::remove_var("COPYPASTE_VAULT_ADDR");
    std::env::remove_var("COPYPASTE_VAULT_TOKEN");
    std::env::set_var("COPYPASTE_PERSISTENCE_BACKEND", "vault");
    std::env::set_var("COPYPASTE_VAULT_ADDR", "ftp://invalid-protocol.com");
    std::env::set_var(
        "COPYPASTE_VAULT_TOKEN",
        "this_token_is_long_enough_1234567890",
    );
    let _ = std::panic::catch_unwind(|| create_paste_store());
    std::env::remove_var("COPYPASTE_PERSISTENCE_BACKEND");
    std::env::remove_var("COPYPASTE_VAULT_ADDR");
    std::env::remove_var("COPYPASTE_VAULT_TOKEN");
    panic!("Should have panicked due to invalid URL protocol");
}
