use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce as AesNonce};
use base64::{engine::general_purpose, Engine};
use chacha20poly1305::{ChaCha20Poly1305, Nonce as ChaNonce, XChaCha20Poly1305, XNonce};
use rand::{rngs::OsRng, RngCore};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{EncryptionAlgorithm, StoredContent};

#[derive(Debug)]
pub enum DecryptError {
    MissingKey,
    InvalidKey,
}

pub async fn encrypt_content(
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

            let ciphertext_b64 = general_purpose::STANDARD.encode(&ciphertext);
            let nonce_b64 = general_purpose::STANDARD.encode(nonce_bytes);

            // Optional OCaml verification (doesn't affect success/failure)
            let _ = verify_encryption_with_ocaml(
                algorithm,
                text,
                &ciphertext_b64,
                key,
                Some(&nonce_b64),
            )
            .await;

            Ok(StoredContent::Encrypted {
                algorithm,
                ciphertext: ciphertext_b64,
                nonce: nonce_b64,
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

            let ciphertext_b64 = general_purpose::STANDARD.encode(&ciphertext);
            let nonce_b64 = general_purpose::STANDARD.encode(nonce_bytes);

            // Optional OCaml verification
            let _ = verify_encryption_with_ocaml(
                algorithm,
                text,
                &ciphertext_b64,
                key,
                Some(&nonce_b64),
            )
            .await;

            Ok(StoredContent::Encrypted {
                algorithm,
                ciphertext: ciphertext_b64,
                nonce: nonce_b64,
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

            let ciphertext_b64 = general_purpose::STANDARD.encode(&ciphertext);
            let nonce_b64 = general_purpose::STANDARD.encode(nonce_bytes);

            // Optional OCaml verification
            let _ = verify_encryption_with_ocaml(
                algorithm,
                text,
                &ciphertext_b64,
                key,
                Some(&nonce_b64),
            )
            .await;

            Ok(StoredContent::Encrypted {
                algorithm,
                ciphertext: ciphertext_b64,
                nonce: nonce_b64,
                salt: general_purpose::STANDARD.encode(salt),
            })
        }
    }
}

pub fn decrypt_content(content: &StoredContent, key: Option<&str>) -> Result<String, DecryptError> {
    match content {
        StoredContent::Plain { text } => Ok(text.clone()),
        StoredContent::Encrypted {
            algorithm,
            ciphertext,
            nonce,
            salt,
        }
        | StoredContent::Stego {
            algorithm,
            ciphertext,
            nonce,
            salt,
            ..
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

#[derive(Serialize)]
struct EncryptionVerificationRequest {
    algorithm: String,
    plaintext: String,
    ciphertext: String,
    key: String,
    nonce: Option<String>,
    aad: Option<String>,
}

#[derive(Serialize)]
struct SignatureVerificationRequest {
    algorithm: String,
    message: String,
    signature: String,
    public_key: String,
}

/// Optional verification using OCaml crypto verifier service
async fn verify_with_ocaml_crypto_service(
    verification_type: &str,
    request_body: String,
) -> Result<(), String> {
    let verifier_url = std::env::var("CRYPTO_VERIFIER_URL")
        .unwrap_or_else(|_| "http://localhost:8001".to_string());

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            log::warn!(
                "Failed to create HTTP client for crypto verification: {}",
                e
            );
            return Ok(()); // Don't fail the operation if verifier is unavailable
        }
    };

    let url = format!("{}/verify/{}", verifier_url, verification_type);

    match client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(request_body)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<serde_json::Value>().await {
                    Ok(json) => {
                        if json.get("valid").and_then(|v| v.as_bool()).unwrap_or(false) {
                            log::info!("Cryptographic verification successful via OCaml service");
                            Ok(())
                        } else {
                            let details = json
                                .get("details")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown verification error");
                            log::warn!("Cryptographic verification failed: {}", details);
                            // Don't fail the operation - OCaml verification is supplementary
                            Ok(())
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to parse OCaml verification response: {}", e);
                        Ok(())
                    }
                }
            } else {
                log::warn!(
                    "OCaml verification service returned HTTP {}: {}",
                    response.status(),
                    response.status().canonical_reason().unwrap_or("Unknown")
                );
                Ok(())
            }
        }
        Err(e) => {
            log::warn!("OCaml crypto verification service unavailable: {}", e);
            Ok(()) // Don't fail the main operation
        }
    }
}

/// Verify encryption operation with OCaml service (optional)
pub async fn verify_encryption_with_ocaml(
    algorithm: EncryptionAlgorithm,
    plaintext: &str,
    ciphertext: &str,
    key: &str,
    nonce: Option<&str>,
) -> Result<(), String> {
    let algorithm_str = match algorithm {
        EncryptionAlgorithm::Aes256Gcm => "aes256_gcm",
        EncryptionAlgorithm::ChaCha20Poly1305 => "chacha20_poly1305",
        EncryptionAlgorithm::XChaCha20Poly1305 => "xchacha20_poly1305",
        EncryptionAlgorithm::None => return Ok(()), // No verification needed for plaintext
    };

    let request = EncryptionVerificationRequest {
        algorithm: algorithm_str.to_string(),
        plaintext: plaintext.to_string(),
        ciphertext: ciphertext.to_string(),
        key: key.to_string(),
        nonce: nonce.map(|s| s.to_string()),
        aad: None,
    };

    let request_body = serde_json::to_string(&request)
        .map_err(|e| format!("Failed to serialize verification request: {}", e))?;

    verify_with_ocaml_crypto_service("encryption", request_body).await
}

/// Verify signature operation with OCaml service (optional)
pub async fn verify_signature_with_ocaml(
    message: &str,
    signature: &str,
    public_key: &str,
) -> Result<(), String> {
    let request = SignatureVerificationRequest {
        algorithm: "ed25519".to_string(),
        message: message.to_string(),
        signature: signature.to_string(),
        public_key: public_key.to_string(),
    };

    let request_body = serde_json::to_string(&request)
        .map_err(|e| format!("Failed to serialize signature verification request: {}", e))?;

    verify_with_ocaml_crypto_service("signature", request_body).await
}
