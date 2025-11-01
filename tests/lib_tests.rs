use copypaste::{create_paste_store, EncryptionAlgorithm, PasteFormat, StoredContent, StoredPaste};

#[tokio::test]
async fn store_round_trip_plain() {
    let store = create_paste_store();
    let paste = StoredPaste {
        content: StoredContent::Plain {
            text: "roundtrip".into(),
        },
        format: PasteFormat::PlainText,
        created_at: 1,
        expires_at: None,
        burn_after_reading: false,
        metadata: Default::default(),
    };

    let id = store.create_paste(paste.clone()).await;
    let stored = store.get_paste(&id).await.expect("paste should exist");
    assert!(matches!(stored.content, StoredContent::Plain { .. }));
    assert_eq!(stored.format, paste.format);
}

#[tokio::test]
async fn store_expired_returns_error() {
    let store = create_paste_store();
    let paste = StoredPaste {
        content: StoredContent::Plain {
            text: "ephemeral".into(),
        },
        format: PasteFormat::PlainText,
        created_at: 10,
        expires_at: Some(5),
        burn_after_reading: false,
        metadata: Default::default(),
    };

    let id = store.create_paste(paste).await;
    assert!(store.get_paste(&id).await.is_err());
}

#[tokio::test]
async fn store_handles_encrypted_variant() {
    let store = create_paste_store();
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
        metadata: Default::default(),
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
    let paste = StoredPaste {
        content: StoredContent::Encrypted {
            algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
            ciphertext: "cipher".into(),
            nonce: "nonce".into(),
            salt: "salt".into(),
        },
        format: PasteFormat::Code,
        created_at: 0,
        expires_at: None,
        burn_after_reading: false,
        metadata: Default::default(),
    };

    let id = store.create_paste(paste).await;
    let stored = store
        .get_paste(&id)
        .await
        .expect("encrypted paste should exist");
    assert!(matches!(stored.content, StoredContent::Encrypted { .. }));
}
