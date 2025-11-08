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
