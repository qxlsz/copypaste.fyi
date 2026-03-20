use base64::Engine;
use copypaste::server::crypto::decrypt_content;
use copypaste::{EncryptionAlgorithm, StoredContent};

#[tokio::test]
async fn encrypt_decrypt_roundtrip_aes_gcm() {
    let plaintext = "Hello, World! This is a test message.";
    let key = "test-key-12345678901234567890123456789012";

    let encrypted =
        copypaste::server::crypto::encrypt_content(plaintext, key, EncryptionAlgorithm::Aes256Gcm)
            .await
            .expect("encryption should succeed");

    let StoredContent::Encrypted {
        ciphertext,
        nonce,
        salt,
        ..
    } = encrypted
    else {
        panic!("expected encrypted content");
    };

    let stored_content = StoredContent::Encrypted {
        algorithm: EncryptionAlgorithm::Aes256Gcm,
        ciphertext,
        nonce,
        salt,
    };

    let decrypted = decrypt_content(&stored_content, Some(key)).expect("decryption should succeed");

    assert_eq!(decrypted, plaintext);
}

#[tokio::test]
async fn encrypt_decrypt_roundtrip_chacha20() {
    let plaintext = "Testing ChaCha20-Poly1305 encryption.";
    let key = "chacha-key-12345678901234567890123456789012";

    let encrypted = copypaste::server::crypto::encrypt_content(
        plaintext,
        key,
        EncryptionAlgorithm::ChaCha20Poly1305,
    )
    .await
    .expect("encryption should succeed");

    let StoredContent::Encrypted {
        ciphertext,
        nonce,
        salt,
        ..
    } = encrypted
    else {
        panic!("expected encrypted content");
    };

    let stored_content = StoredContent::Encrypted {
        algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
        ciphertext,
        nonce,
        salt,
    };

    let decrypted = decrypt_content(&stored_content, Some(key)).expect("decryption should succeed");

    assert_eq!(decrypted, plaintext);
}

#[tokio::test]
async fn encrypt_decrypt_roundtrip_xchacha20() {
    let plaintext = "Testing XChaCha20-Poly1305 encryption.";
    let key = "xchacha-key-12345678901234567890123456789012";

    let encrypted = copypaste::server::crypto::encrypt_content(
        plaintext,
        key,
        EncryptionAlgorithm::XChaCha20Poly1305,
    )
    .await
    .expect("encryption should succeed");

    let StoredContent::Encrypted {
        ciphertext,
        nonce,
        salt,
        ..
    } = encrypted
    else {
        panic!("expected encrypted content");
    };

    let stored_content = StoredContent::Encrypted {
        algorithm: EncryptionAlgorithm::XChaCha20Poly1305,
        ciphertext,
        nonce,
        salt,
    };

    let decrypted = decrypt_content(&stored_content, Some(key)).expect("decryption should succeed");

    assert_eq!(decrypted, plaintext);
}

#[tokio::test]
async fn encrypt_decrypt_roundtrip_kyber_hybrid() {
    let plaintext = "Testing Kyber hybrid encryption.";
    let key = "kyber-key-12345678901234567890123456789012";

    let encrypted = copypaste::server::crypto::encrypt_content(
        plaintext,
        key,
        EncryptionAlgorithm::KyberHybridAes256Gcm,
    )
    .await
    .expect("encryption should succeed");

    let StoredContent::Encrypted { ciphertext, .. } = encrypted else {
        panic!("expected encrypted content");
    };

    let stored_content = StoredContent::Encrypted {
        algorithm: EncryptionAlgorithm::KyberHybridAes256Gcm,
        ciphertext,
        nonce: String::new(),
        salt: String::new(),
    };

    let decrypted = decrypt_content(&stored_content, Some(key)).expect("decryption should succeed");

    assert_eq!(decrypted, plaintext);
}

#[test]
fn decrypt_plain_content() {
    let content = StoredContent::Plain {
        text: "plain text content".to_string(),
    };

    let result = decrypt_content(&content, None);
    assert_eq!(result.unwrap(), "plain text content");
}

#[test]
fn decrypt_encrypted_missing_key() {
    let content = StoredContent::Encrypted {
        algorithm: EncryptionAlgorithm::Aes256Gcm,
        ciphertext: "dummy".to_string(),
        nonce: "dummy".to_string(),
        salt: "dummy".to_string(),
    };

    let result = decrypt_content(&content, None);
    assert!(result.is_err());
}

#[tokio::test]
async fn decrypt_wrong_key_aes_gcm_returns_invalid_key() {
    let plaintext = "secret data";
    let correct_key = "correct-key-12345678901234567890123456789";

    let encrypted = copypaste::server::crypto::encrypt_content(
        plaintext,
        correct_key,
        EncryptionAlgorithm::Aes256Gcm,
    )
    .await
    .expect("encryption should succeed");

    let result = decrypt_content(
        &encrypted,
        Some("wrong-key-XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"),
    );
    assert!(result.is_err(), "decryption with wrong key must fail");
}

#[tokio::test]
async fn decrypt_tampered_ciphertext_aes_gcm_returns_invalid_key() {
    let plaintext = "tamper me";
    let key = "tamper-key-12345678901234567890123456789";

    let encrypted =
        copypaste::server::crypto::encrypt_content(plaintext, key, EncryptionAlgorithm::Aes256Gcm)
            .await
            .expect("encryption should succeed");

    // Flip a byte in the ciphertext to break the AEAD tag
    let tampered = match encrypted {
        StoredContent::Encrypted {
            algorithm,
            mut ciphertext,
            nonce,
            salt,
        } => {
            let mut decoded = base64::engine::general_purpose::STANDARD
                .decode(&ciphertext)
                .expect("valid base64");
            decoded[0] ^= 0xff;
            ciphertext = base64::engine::general_purpose::STANDARD.encode(&decoded);
            StoredContent::Encrypted {
                algorithm,
                ciphertext,
                nonce,
                salt,
            }
        }
        _ => panic!("expected encrypted"),
    };

    let result = decrypt_content(&tampered, Some(key));
    assert!(
        result.is_err(),
        "decryption of tampered ciphertext must fail"
    );
}

#[tokio::test]
async fn decrypt_truncated_ciphertext_aes_gcm_returns_invalid_key() {
    let plaintext = "truncate this";
    let key = "truncate-key-12345678901234567890123456";

    let encrypted =
        copypaste::server::crypto::encrypt_content(plaintext, key, EncryptionAlgorithm::Aes256Gcm)
            .await
            .expect("encryption should succeed");

    // Truncate the ciphertext (removes the AEAD authentication tag)
    let truncated = match encrypted {
        StoredContent::Encrypted {
            algorithm,
            mut ciphertext,
            nonce,
            salt,
        } => {
            let mut decoded = base64::engine::general_purpose::STANDARD
                .decode(&ciphertext)
                .expect("valid base64");
            // Remove the last 16 bytes (GCM authentication tag length)
            let new_len = decoded.len().saturating_sub(16);
            decoded.truncate(new_len);
            ciphertext = base64::engine::general_purpose::STANDARD.encode(&decoded);
            StoredContent::Encrypted {
                algorithm,
                ciphertext,
                nonce,
                salt,
            }
        }
        _ => panic!("expected encrypted"),
    };

    let result = decrypt_content(&truncated, Some(key));
    assert!(
        result.is_err(),
        "decryption of truncated ciphertext must fail"
    );
}

/// Verify that the Kyber ciphertext no longer contains the private key as the 5th component.
/// The stored blob must have exactly 4 pipe-delimited parts.
#[tokio::test]
async fn kyber_ciphertext_does_not_contain_private_key() {
    let plaintext = "private key must not be stored";
    let key = "kyber-sec-key-12345678901234567890123";

    let encrypted = copypaste::server::crypto::encrypt_content(
        plaintext,
        key,
        copypaste::EncryptionAlgorithm::KyberHybridAes256Gcm,
    )
    .await
    .expect("encryption should succeed");

    let ciphertext = match &encrypted {
        copypaste::StoredContent::Encrypted { ciphertext, .. } => ciphertext.clone(),
        _ => panic!("expected encrypted content"),
    };

    let parts: Vec<&str> = ciphertext.split('|').collect();
    assert_eq!(
        parts.len(),
        4,
        "Kyber ciphertext must have exactly 4 parts (no private key stored): got {}",
        parts.len()
    );
}

/// Verify that legacy 5-part Kyber blobs (with the private key embedded) can still be
/// decrypted — the 5th component is ignored and the key is re-derived from the passphrase.
#[tokio::test]
async fn kyber_legacy_5part_blob_decrypts_successfully() {
    let key = "kyber-legacy-key-1234567890123456789";

    // Build a fresh 4-part blob, then manually append a (bogus) 5th component to simulate
    // a pre-fix legacy record. Decryption must succeed and ignore the extra component.
    let encrypted = copypaste::server::crypto::encrypt_content(
        "legacy compatibility check",
        key,
        copypaste::EncryptionAlgorithm::KyberHybridAes256Gcm,
    )
    .await
    .expect("encryption should succeed");

    let ciphertext_4part = match encrypted {
        copypaste::StoredContent::Encrypted { ciphertext, .. } => ciphertext,
        _ => panic!("expected encrypted content"),
    };

    // Append a fake 5th component (as the legacy format did)
    let legacy_ciphertext = format!("{}|ZmFrZXByaXZhdGVrZXk=", ciphertext_4part);

    let legacy_stored = copypaste::StoredContent::Encrypted {
        algorithm: copypaste::EncryptionAlgorithm::KyberHybridAes256Gcm,
        ciphertext: legacy_ciphertext,
        nonce: String::new(),
        salt: String::new(),
    };

    let decrypted = copypaste::server::crypto::decrypt_content(&legacy_stored, Some(key))
        .expect("legacy 5-part Kyber blob must still decrypt");
    assert_eq!(decrypted, "legacy compatibility check");
}

// OCaml verification behaviour tests
// Each test runs in its own process under nextest, so env var mutations are safe.

#[tokio::test]
async fn ocaml_service_unavailable_does_not_block_by_default() {
    // Default mode: COPYPASTE_REQUIRE_CRYPTO_VERIFICATION unset → verifier failures are
    // logged but must NOT prevent encryption from succeeding.
    std::env::remove_var("COPYPASTE_REQUIRE_CRYPTO_VERIFICATION");
    // Port 1 is reserved and always refuses connections immediately.
    std::env::set_var("CRYPTO_VERIFIER_URL", "http://127.0.0.1:1");

    let result = copypaste::server::crypto::encrypt_content(
        "hello world",
        "test-key-00000000000000000000000000000000",
        copypaste::EncryptionAlgorithm::Aes256Gcm,
    )
    .await;

    assert!(
        result.is_ok(),
        "encryption must succeed when OCaml service is unavailable (defense-in-depth mode)"
    );
}

#[tokio::test]
async fn ocaml_valid_false_blocks_when_strict_mode_enabled() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/verify/encryption");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"valid":false,"details":"test forced failure"}"#);
    });

    std::env::set_var("CRYPTO_VERIFIER_URL", server.base_url());
    std::env::set_var("COPYPASTE_REQUIRE_CRYPTO_VERIFICATION", "true");

    let result = copypaste::server::crypto::encrypt_content(
        "hello world",
        "test-key-00000000000000000000000000000000",
        copypaste::EncryptionAlgorithm::Aes256Gcm,
    )
    .await;

    assert!(
        result.is_err(),
        "encryption must fail when OCaml verifier returns valid=false in strict mode"
    );
}

#[tokio::test]
async fn ocaml_service_unavailable_blocks_when_strict_mode_enabled() {
    // Strict mode: COPYPASTE_REQUIRE_CRYPTO_VERIFICATION=true → unreachable verifier is an error.
    std::env::set_var("COPYPASTE_REQUIRE_CRYPTO_VERIFICATION", "true");
    std::env::set_var("CRYPTO_VERIFIER_URL", "http://127.0.0.1:1");

    let result = copypaste::server::crypto::encrypt_content(
        "hello world",
        "test-key-00000000000000000000000000000000",
        copypaste::EncryptionAlgorithm::Aes256Gcm,
    )
    .await;

    assert!(
        result.is_err(),
        "encryption must fail when OCaml service is unreachable in strict mode"
    );
}

#[tokio::test]
async fn ocaml_valid_true_allows_encryption_in_strict_mode() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/verify/encryption");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"valid":true}"#);
    });

    std::env::set_var("CRYPTO_VERIFIER_URL", server.base_url());
    std::env::set_var("COPYPASTE_REQUIRE_CRYPTO_VERIFICATION", "true");

    let result = copypaste::server::crypto::encrypt_content(
        "hello world",
        "test-key-00000000000000000000000000000000",
        copypaste::EncryptionAlgorithm::Aes256Gcm,
    )
    .await;

    assert!(
        result.is_ok(),
        "encryption must succeed when OCaml verifier returns valid=true in strict mode"
    );
}
