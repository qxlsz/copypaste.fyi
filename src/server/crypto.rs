use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce as AesNonce};
use base64::engine::general_purpose;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use chacha20poly1305::{ChaCha20Poly1305, Nonce as ChaNonce, XChaCha20Poly1305, XNonce};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::convert::TryInto;
use zeroize::Zeroizing;

use hkdf::Hkdf;
use ml_kem::kem::{Decapsulate, Encapsulate};
use ml_kem::{Ciphertext, KemCore, MlKem768, B32};

use crate::{EncryptionAlgorithm, StoredContent};

#[derive(Debug)]
pub enum DecryptError {
    MissingKey,
    InvalidKey,
}

/// Arguments needed to call the OCaml verification service after CPU-bound encryption.
struct OcamlVerifyArgs {
    algorithm: EncryptionAlgorithm,
    plaintext: String,
    ciphertext: String,
    key: String,
    nonce: Option<String>,
    salt: Option<String>,
}

/// CPU-bound encryption work, suitable for running inside `spawn_blocking`.
///
/// Returns the encrypted content and, for algorithms that support OCaml
/// defense-in-depth verification, the arguments needed for that async step.
fn encrypt_content_sync(
    text: &str,
    key: &str,
    algorithm: EncryptionAlgorithm,
) -> Result<(StoredContent, Option<OcamlVerifyArgs>), String> {
    match algorithm {
        EncryptionAlgorithm::None => Ok((
            StoredContent::Plain {
                text: text.to_owned(),
            },
            None,
        )),
        EncryptionAlgorithm::Aes256Gcm => {
            let mut salt = [0u8; 16];
            OsRng.fill_bytes(&mut salt);
            let derived = derive_key_material(key, &salt);

            let cipher = Aes256Gcm::new_from_slice(&*derived)
                .map_err(|_| "failed to initialise cipher".to_string())?;
            let mut nonce_bytes = [0u8; 12];
            OsRng.fill_bytes(&mut nonce_bytes);
            let nonce = AesNonce::from(nonce_bytes);

            let ciphertext = cipher
                .encrypt(&nonce, text.as_bytes())
                .map_err(|_| "failed to encrypt content".to_string())?;

            let ciphertext_b64 = general_purpose::STANDARD.encode(&ciphertext);
            let nonce_b64 = general_purpose::STANDARD.encode(nonce_bytes);
            let salt_b64 = general_purpose::STANDARD.encode(salt);

            let verify = OcamlVerifyArgs {
                algorithm,
                plaintext: text.to_owned(),
                ciphertext: ciphertext_b64.clone(),
                key: key.to_owned(),
                nonce: Some(nonce_b64.clone()),
                salt: Some(salt_b64.clone()),
            };

            Ok((
                StoredContent::Encrypted {
                    algorithm,
                    ciphertext: ciphertext_b64,
                    nonce: nonce_b64,
                    salt: salt_b64,
                },
                Some(verify),
            ))
        }
        EncryptionAlgorithm::ChaCha20Poly1305 => {
            let mut salt = [0u8; 16];
            OsRng.fill_bytes(&mut salt);
            let derived = derive_key_material(key, &salt);

            let cipher = ChaCha20Poly1305::new_from_slice(&*derived)
                .map_err(|_| "failed to initialise cipher".to_string())?;
            let mut nonce_bytes = [0u8; 12];
            OsRng.fill_bytes(&mut nonce_bytes);
            let nonce = ChaNonce::from(nonce_bytes);

            let ciphertext = cipher
                .encrypt(&nonce, text.as_bytes())
                .map_err(|_| "failed to encrypt content".to_string())?;

            let ciphertext_b64 = general_purpose::STANDARD.encode(&ciphertext);
            let nonce_b64 = general_purpose::STANDARD.encode(nonce_bytes);
            let salt_b64 = general_purpose::STANDARD.encode(salt);

            let verify = OcamlVerifyArgs {
                algorithm,
                plaintext: text.to_owned(),
                ciphertext: ciphertext_b64.clone(),
                key: key.to_owned(),
                nonce: Some(nonce_b64.clone()),
                salt: Some(salt_b64.clone()),
            };

            Ok((
                StoredContent::Encrypted {
                    algorithm,
                    ciphertext: ciphertext_b64,
                    nonce: nonce_b64,
                    salt: salt_b64,
                },
                Some(verify),
            ))
        }
        EncryptionAlgorithm::XChaCha20Poly1305 => {
            let mut salt = [0u8; 16];
            OsRng.fill_bytes(&mut salt);
            let derived = derive_key_material(key, &salt);

            let cipher = XChaCha20Poly1305::new_from_slice(&*derived)
                .map_err(|_| "failed to initialise cipher".to_string())?;
            let mut nonce_bytes = [0u8; 24];
            OsRng.fill_bytes(&mut nonce_bytes);
            let nonce = XNonce::from(nonce_bytes);

            let ciphertext = cipher
                .encrypt(&nonce, text.as_bytes())
                .map_err(|_| "failed to encrypt content".to_string())?;

            let ciphertext_b64 = general_purpose::STANDARD.encode(&ciphertext);
            let nonce_b64 = general_purpose::STANDARD.encode(nonce_bytes);
            let salt_b64 = general_purpose::STANDARD.encode(salt);

            let verify = OcamlVerifyArgs {
                algorithm,
                plaintext: text.to_owned(),
                ciphertext: ciphertext_b64.clone(),
                key: key.to_owned(),
                nonce: Some(nonce_b64.clone()),
                salt: Some(salt_b64.clone()),
            };

            Ok((
                StoredContent::Encrypted {
                    algorithm,
                    ciphertext: ciphertext_b64,
                    nonce: nonce_b64,
                    salt: salt_b64,
                },
                Some(verify),
            ))
        }
        EncryptionAlgorithm::KyberHybridAes256Gcm => {
            // Derive a deterministic ML-KEM-768 keypair from the passphrase using HKDF.
            // The passphrase acts as a static identity: the same passphrase always re-derives
            // the same (dk, ek) pair.  Fresh OS randomness in `ek.encapsulate` ensures each
            // call produces a distinct (kem_ct, shared_secret), preserving IND-CPA security.
            let hk = Hkdf::<Sha256>::new(None, key.as_bytes());
            let mut d_bytes = [0u8; 32];
            let mut z_bytes = [0u8; 32];
            hk.expand(b"ml-kem-768-keygen-d", &mut d_bytes)
                .map_err(|e| format!("HKDF expand error (d): {}", e))?;
            hk.expand(b"ml-kem-768-keygen-z", &mut z_bytes)
                .map_err(|e| format!("HKDF expand error (z): {}", e))?;
            let d: B32 = d_bytes.into();
            let z: B32 = z_bytes.into();
            let (_, ek) = MlKem768::generate_deterministic(&d, &z);

            // Encapsulate using OsRng — `encapsulate(&mut OsRng)` passes OS entropy as the
            // ephemeral `m` value (FIPS 203 §6.2), so two encryptions with the same passphrase
            // produce computationally unlinkable (kem_ct, shared_secret) pairs.
            let (kem_ct, shared_secret) = ek
                .encapsulate(&mut OsRng)
                .map_err(|_| "ML-KEM-768 encapsulation failed".to_string())?;

            // Derive AES-256-GCM key from the KEM shared secret via HKDF.
            let hk2 = Hkdf::<Sha256>::new(None, &shared_secret);
            let mut aes_key = Zeroizing::new([0u8; 32]);
            hk2.expand(b"aes-256-gcm-key", &mut *aes_key)
                .map_err(|e| format!("HKDF expand error (aes-key): {}", e))?;

            let cipher = Aes256Gcm::new_from_slice(&*aes_key)
                .map_err(|_| "failed to initialise AES cipher".to_string())?;
            let mut nonce_bytes = [0u8; 12];
            OsRng.fill_bytes(&mut nonce_bytes);
            let nonce = AesNonce::from(nonce_bytes);
            let aes_ciphertext = cipher
                .encrypt(&nonce, text.as_bytes())
                .map_err(|_| "failed to encrypt content with AES".to_string())?;

            // 3-part storage format (new ML-KEM-768, distinct from legacy 4/5-part blobs):
            //   kem_ct_b64 | aes_ct_b64 | aes_nonce_b64
            // The decapsulation key is NOT stored; it is re-derived from the passphrase on
            // decryption, so server-side access to the blob cannot decrypt the content.
            let combined = format!(
                "{}|{}|{}",
                BASE64_STANDARD.encode(&*kem_ct),
                BASE64_STANDARD.encode(&aes_ciphertext),
                BASE64_STANDARD.encode(nonce_bytes),
            );

            Ok((
                StoredContent::Encrypted {
                    algorithm,
                    ciphertext: combined,
                    nonce: String::new(),
                    salt: String::new(),
                },
                None,
            ))
        }
    }
}

/// Encrypt content using the specified algorithm.
///
/// CPU-bound cipher work runs inside `tokio::task::spawn_blocking` so it does not
/// occupy an async worker thread.  The optional OCaml defense-in-depth verification
/// is performed afterward on the async thread as it is an I/O-bound network call.
pub async fn encrypt_content(
    text: &str,
    key: &str,
    algorithm: EncryptionAlgorithm,
) -> Result<StoredContent, String> {
    let text = text.to_owned();
    let key = key.to_owned();

    let (content, verify_args) =
        tokio::task::spawn_blocking(move || encrypt_content_sync(&text, &key, algorithm))
            .await
            .map_err(|_| "encryption thread panicked".to_string())??;

    // Defense-in-depth OCaml verification (configurable via COPYPASTE_REQUIRE_CRYPTO_VERIFICATION)
    if let Some(args) = verify_args {
        verify_encryption_with_ocaml(
            args.algorithm,
            &args.plaintext,
            &args.ciphertext,
            &args.key,
            args.nonce.as_deref(),
            args.salt.as_deref(),
        )
        .await?;
    }

    Ok(content)
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
            let extracted_key = key.ok_or(DecryptError::MissingKey)?;
            log::info!("Starting decryption for algorithm: {:?}", algorithm);

            // KyberHybridAes256Gcm uses a different storage layout; handle it separately.
            if matches!(algorithm, EncryptionAlgorithm::KyberHybridAes256Gcm) {
                let key_str = extracted_key;
                let parts: Vec<&str> = ciphertext.split('|').collect();

                match parts.len() {
                    3 => {
                        // New ML-KEM-768 format: kem_ct_b64|aes_ct_b64|aes_nonce_b64
                        let hk = Hkdf::<Sha256>::new(None, key_str.as_bytes());
                        let mut d_bytes = [0u8; 32];
                        let mut z_bytes = [0u8; 32];
                        hk.expand(b"ml-kem-768-keygen-d", &mut d_bytes)
                            .map_err(|_| DecryptError::InvalidKey)?;
                        hk.expand(b"ml-kem-768-keygen-z", &mut z_bytes)
                            .map_err(|_| DecryptError::InvalidKey)?;
                        let d: B32 = d_bytes.into();
                        let z: B32 = z_bytes.into();
                        let (dk, _) = MlKem768::generate_deterministic(&d, &z);

                        let kem_ct_bytes = BASE64_STANDARD
                            .decode(parts[0])
                            .map_err(|_| DecryptError::InvalidKey)?;
                        // ML-KEM-768 ciphertext is exactly 1088 bytes.
                        let kem_ct_arr: [u8; 1088] = kem_ct_bytes
                            .try_into()
                            .map_err(|_| DecryptError::InvalidKey)?;
                        let kem_ct: Ciphertext<MlKem768> = kem_ct_arr.into();

                        let shared_secret = dk
                            .decapsulate(&kem_ct)
                            .map_err(|_| DecryptError::InvalidKey)?;

                        let hk2 = Hkdf::<Sha256>::new(None, &shared_secret);
                        let mut aes_key = Zeroizing::new([0u8; 32]);
                        hk2.expand(b"aes-256-gcm-key", &mut *aes_key)
                            .map_err(|_| DecryptError::InvalidKey)?;

                        let aes_ciphertext = BASE64_STANDARD
                            .decode(parts[1])
                            .map_err(|_| DecryptError::InvalidKey)?;
                        let aes_nonce_bytes = BASE64_STANDARD
                            .decode(parts[2])
                            .map_err(|_| DecryptError::InvalidKey)?;

                        let cipher = Aes256Gcm::new_from_slice(&*aes_key)
                            .map_err(|_| DecryptError::InvalidKey)?;
                        let nonce_arr: [u8; 12] = aes_nonce_bytes
                            .try_into()
                            .map_err(|_| DecryptError::InvalidKey)?;
                        let nonce = AesNonce::from(nonce_arr);

                        return cipher
                            .decrypt(&nonce, aes_ciphertext.as_ref())
                            .map_err(|_| DecryptError::InvalidKey)
                            .and_then(|bytes| {
                                String::from_utf8(bytes).map_err(|_| DecryptError::InvalidKey)
                            });
                    }
                    4 | 5 => {
                        // Legacy simulation format (4 or 5 parts):
                        //   pq_ct_b64 | pub_key_b64 | aes_ct_b64 | aes_nonce_b64 [| ignored]
                        // Re-derive the SHA-256 simulation shared secret for backward compat.
                        let aes_ciphertext = BASE64_STANDARD
                            .decode(parts[2])
                            .map_err(|_| DecryptError::InvalidKey)?;
                        let aes_nonce_bytes = BASE64_STANDARD
                            .decode(parts[3])
                            .map_err(|_| DecryptError::InvalidKey)?;

                        let mut secret_hasher = Sha256::new();
                        secret_hasher.update(b"kem_shared_secret");
                        secret_hasher.update(key_str.as_bytes());
                        let shared_secret: [u8; 32] = secret_hasher.finalize().into();

                        let mut key_hasher = Sha256::new();
                        key_hasher.update(shared_secret);
                        key_hasher.update(key_str.as_bytes());
                        let aes_key: Zeroizing<[u8; 32]> =
                            Zeroizing::new(key_hasher.finalize().into());

                        let cipher = Aes256Gcm::new_from_slice(&*aes_key)
                            .map_err(|_| DecryptError::InvalidKey)?;
                        let nonce_arr: [u8; 12] = aes_nonce_bytes
                            .try_into()
                            .map_err(|_| DecryptError::InvalidKey)?;
                        let nonce = AesNonce::from(nonce_arr);

                        return cipher
                            .decrypt(&nonce, aes_ciphertext.as_ref())
                            .map_err(|_| DecryptError::InvalidKey)
                            .and_then(|bytes| {
                                String::from_utf8(bytes).map_err(|_| DecryptError::InvalidKey)
                            });
                    }
                    _ => return Err(DecryptError::InvalidKey),
                }
            }

            // Normal algorithms that use base64 encoding
            let salt_bytes = general_purpose::STANDARD
                .decode(salt)
                .map_err(|_| DecryptError::InvalidKey)?;
            let nonce_bytes_vec = general_purpose::STANDARD
                .decode(nonce)
                .map_err(|_| DecryptError::InvalidKey)?;
            let cipher_bytes = general_purpose::STANDARD
                .decode(ciphertext)
                .map_err(|_| DecryptError::InvalidKey)?;

            let derived = derive_key_material(extracted_key, &salt_bytes);

            match algorithm {
                EncryptionAlgorithm::None => {
                    String::from_utf8(cipher_bytes).map_err(|_| DecryptError::InvalidKey)
                }
                EncryptionAlgorithm::Aes256Gcm => {
                    let cipher = Aes256Gcm::new_from_slice(&*derived)
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
                    let cipher = ChaCha20Poly1305::new_from_slice(&*derived)
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
                    let cipher = XChaCha20Poly1305::new_from_slice(&*derived)
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
                EncryptionAlgorithm::KyberHybridAes256Gcm => {
                    // This should never be reached due to early return above
                    Err(DecryptError::InvalidKey)
                }
            }
        }
    }
}

fn derive_key_material(key: &str, salt: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut hasher = Sha256::new();
    hasher.update(salt);
    hasher.update(key.as_bytes());
    Zeroizing::new(hasher.finalize().into())
}

#[derive(Serialize)]
struct EncryptionVerificationRequest {
    algorithm: String,
    plaintext: String,
    ciphertext: String,
    key: String,
    nonce: Option<String>,
    salt: Option<String>,
    aad: Option<String>,
}

#[derive(Serialize)]
struct SignatureVerificationRequest {
    algorithm: String,
    message: String,
    signature: String,
    public_key: String,
}

/// Optional/configurable verification using OCaml crypto verifier service.
///
/// By default this is defense-in-depth only: all failure paths are logged but do NOT block
/// paste operations. Set `COPYPASTE_REQUIRE_CRYPTO_VERIFICATION=true` to enable strict mode
/// where verifier failures (network errors, non-2xx responses, or `valid: false`) cause the
/// operation to return an error. The verifier URL is configured via `CRYPTO_VERIFIER_URL`
/// (default: `http://localhost:8001`).
async fn verify_with_ocaml_crypto_service(
    verification_type: &str,
    request_body: String,
) -> Result<(), String> {
    let verifier_url = std::env::var("CRYPTO_VERIFIER_URL")
        .unwrap_or_else(|_| "http://localhost:8001".to_string());

    let require_verification = std::env::var("COPYPASTE_REQUIRE_CRYPTO_VERIFICATION")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);

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
            if require_verification {
                return Err(format!(
                    "Crypto verification unavailable (client build failed): {}",
                    e
                ));
            }
            return Ok(());
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
            let status = response.status();
            if status.is_success() {
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
                            // Log at warn level for expected gaps (algorithm not supported),
                            // error level for actual verification failures.
                            if details.contains("not yet implemented")
                                || details.contains("not supported")
                                || details.contains("Unsupported")
                            {
                                log::warn!(
                                    "OCaml crypto verifier: algorithm not supported for {}: {}",
                                    verification_type,
                                    details
                                );
                            } else {
                                log::error!(
                                    "OCaml crypto verifier returned valid=false for {}: {}",
                                    verification_type,
                                    details
                                );
                            }
                            if require_verification {
                                Err(format!("Crypto verification failed: {}", details))
                            } else {
                                Ok(())
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to parse OCaml verification response: {}", e);
                        if require_verification {
                            Err(format!("Crypto verification response parse failed: {}", e))
                        } else {
                            Ok(())
                        }
                    }
                }
            } else {
                log::warn!(
                    "OCaml verification service returned HTTP {}: {}",
                    status,
                    status.canonical_reason().unwrap_or("Unknown")
                );
                if require_verification {
                    Err(format!(
                        "Crypto verification service returned HTTP {}",
                        status
                    ))
                } else {
                    Ok(())
                }
            }
        }
        Err(e) => {
            log::warn!("OCaml crypto verification service unavailable: {}", e);
            if require_verification {
                Err(format!("Crypto verification service unreachable: {}", e))
            } else {
                Ok(()) // Don't fail the main operation
            }
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
    salt: Option<&str>,
) -> Result<(), String> {
    let algorithm_str = match algorithm {
        EncryptionAlgorithm::Aes256Gcm => "aes256_gcm",
        EncryptionAlgorithm::ChaCha20Poly1305 => "chacha20_poly1305",
        EncryptionAlgorithm::XChaCha20Poly1305 => "xchacha20_poly1305",
        EncryptionAlgorithm::KyberHybridAes256Gcm => "aes256_gcm", // Verify AES portion of hybrid
        EncryptionAlgorithm::None => return Ok(()), // No verification needed for plaintext
    };

    let request = EncryptionVerificationRequest {
        algorithm: algorithm_str.to_string(),
        plaintext: plaintext.to_string(),
        ciphertext: ciphertext.to_string(),
        key: key.to_string(),
        nonce: nonce.map(|s| s.to_string()),
        salt: salt.map(|s| s.to_string()),
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
