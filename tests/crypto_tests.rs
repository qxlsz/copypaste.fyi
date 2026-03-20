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

/// Verify the new ML-KEM-768 blob uses exactly 3 pipe-delimited parts.
/// No private key is stored; it is re-derived from the passphrase at decryption time.
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
        3,
        "ML-KEM-768 ciphertext must have exactly 3 parts (kem_ct|aes_ct|nonce): got {}",
        parts.len()
    );
}

/// Verify that legacy simulation blobs (the old SHA-256-based 4-part and 5-part formats
/// produced before ML-KEM-768 was implemented) can still be decrypted.
#[test]
fn kyber_legacy_simulation_blob_decrypts_successfully() {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce as AesNonce};
    use base64::engine::general_purpose::STANDARD as B64;
    use sha2::{Digest, Sha256};

    let key = "kyber-legacy-key-1234567890123456789";
    let plaintext = "legacy compatibility check";

    // Reproduce the old SHA-256 simulation encrypt logic exactly.
    let pq_pub_hash = Sha256::new()
        .chain_update(b"pq_public_key")
        .chain_update(key.as_bytes())
        .finalize();

    let sim_secret = Sha256::new()
        .chain_update(b"kem_shared_secret")
        .chain_update(key.as_bytes())
        .finalize();

    let ct_hash = Sha256::new()
        .chain_update(b"kem_ciphertext")
        .chain_update(key.as_bytes())
        .finalize();
    let mut kem_ciphertext = [0u8; 64];
    kem_ciphertext[..32].copy_from_slice(&ct_hash);
    kem_ciphertext[32..].copy_from_slice(&ct_hash);

    // Use a fixed nonce so the test is deterministic.
    let nonce_bytes = [0x55u8; 12];

    let aes_key_bytes: [u8; 32] = Sha256::new()
        .chain_update(sim_secret)
        .chain_update(key.as_bytes())
        .finalize()
        .into();

    let cipher = Aes256Gcm::new_from_slice(&aes_key_bytes).unwrap();
    let nonce = AesNonce::from(nonce_bytes);
    let aes_ciphertext = cipher.encrypt(&nonce, plaintext.as_bytes()).unwrap();

    let legacy_4part = format!(
        "{}|{}|{}|{}",
        B64.encode(kem_ciphertext),
        B64.encode(&pq_pub_hash[..32]),
        B64.encode(&aes_ciphertext),
        B64.encode(nonce_bytes),
    );

    // 4-part legacy blob must decrypt.
    let stored_4 = StoredContent::Encrypted {
        algorithm: EncryptionAlgorithm::KyberHybridAes256Gcm,
        ciphertext: legacy_4part.clone(),
        nonce: String::new(),
        salt: String::new(),
    };
    let decrypted = decrypt_content(&stored_4, Some(key))
        .expect("legacy 4-part simulation blob must still decrypt");
    assert_eq!(decrypted, plaintext);

    // 5-part legacy blob (with trailing ignored component) must also decrypt.
    let legacy_5part = format!("{}|ZmFrZXByaXZhdGVrZXk=", legacy_4part);
    let stored_5 = StoredContent::Encrypted {
        algorithm: EncryptionAlgorithm::KyberHybridAes256Gcm,
        ciphertext: legacy_5part,
        nonce: String::new(),
        salt: String::new(),
    };
    let decrypted5 = decrypt_content(&stored_5, Some(key))
        .expect("legacy 5-part simulation blob must still decrypt");
    assert_eq!(decrypted5, plaintext);
}

/// Decrypting a ML-KEM-768 blob with the wrong passphrase must return an error.
#[tokio::test]
async fn decrypt_wrong_key_kyber_returns_invalid_key() {
    let plaintext = "secret kyber data";
    let correct_key = "correct-kyber-key-123456789012345678901";

    let encrypted = copypaste::server::crypto::encrypt_content(
        plaintext,
        correct_key,
        EncryptionAlgorithm::KyberHybridAes256Gcm,
    )
    .await
    .expect("encryption should succeed");

    let result = decrypt_content(
        &encrypted,
        Some("wrong-kyber-key-XXXXXXXXXXXXXXXXXXXXXXXXX"),
    );
    assert!(result.is_err(), "decryption with wrong key must fail");
}

/// Two ML-KEM-768 encryptions of the same plaintext with the same passphrase must
/// produce different KEM ciphertexts, proving that `encapsulate` uses OS randomness.
#[tokio::test]
async fn kyber_same_passphrase_produces_different_kem_ciphertext() {
    let plaintext = "randomness check";
    let key = "rand-check-key-123456789012345678901234";

    let enc1 = copypaste::server::crypto::encrypt_content(
        plaintext,
        key,
        EncryptionAlgorithm::KyberHybridAes256Gcm,
    )
    .await
    .expect("first encryption should succeed");

    let enc2 = copypaste::server::crypto::encrypt_content(
        plaintext,
        key,
        EncryptionAlgorithm::KyberHybridAes256Gcm,
    )
    .await
    .expect("second encryption should succeed");

    let ct1 = match &enc1 {
        StoredContent::Encrypted { ciphertext, .. } => {
            ciphertext.split('|').next().unwrap().to_string()
        }
        _ => panic!("expected encrypted"),
    };
    let ct2 = match &enc2 {
        StoredContent::Encrypted { ciphertext, .. } => {
            ciphertext.split('|').next().unwrap().to_string()
        }
        _ => panic!("expected encrypted"),
    };

    assert_ne!(
        ct1, ct2,
        "KEM ciphertexts must differ across encryptions (OsRng must be used)"
    );
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
