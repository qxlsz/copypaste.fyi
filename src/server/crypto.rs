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
use std::sync::Once;
use zeroize::Zeroizing;

const MAX_VERIFIER_RESPONSE_BYTES: usize = 16 * 1024;

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

            Ok((
                StoredContent::Encrypted {
                    algorithm,
                    ciphertext: ciphertext_b64,
                    nonce: nonce_b64,
                    salt: salt_b64,
                },
                // The OCaml verifier does not implement XChaCha20/HChaCha20.
                // Returning no verifier payload is an explicit capability
                // decision; strict mode still fails closed for algorithms the
                // verifier claims to support.
                None,
            ))
        }
        EncryptionAlgorithm::KyberHybridAes256Gcm => {
            // Derive a deterministic ML-KEM-768 keypair from the passphrase using HKDF.
            // The passphrase acts as a static identity: the same passphrase always re-derives
            // the same (dk, ek) pair.  Fresh OS randomness in `ek.encapsulate` ensures each
            // call produces a distinct (kem_ct, shared_secret), preserving IND-CPA security.
            let hk = Hkdf::<Sha256>::new(None, key.as_bytes());
            // Seed material is passphrase-derived secret data; Zeroizing wipes
            // the buffers on drop so they do not linger on the heap/stack.
            let mut d_bytes = Zeroizing::new([0u8; 32]);
            let mut z_bytes = Zeroizing::new([0u8; 32]);
            hk.expand(b"ml-kem-768-keygen-d", &mut *d_bytes)
                .map_err(|e| format!("HKDF expand error (d): {}", e))?;
            hk.expand(b"ml-kem-768-keygen-z", &mut *z_bytes)
                .map_err(|e| format!("HKDF expand error (z): {}", e))?;
            let d: B32 = (*d_bytes).into();
            let z: B32 = (*z_bytes).into();
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

/// One-time warning that XChaCha20-Poly1305 and the ML-KEM hybrid are not
/// covered by the OCaml dual-verification service (mirage-crypto exposes
/// neither XChaCha20/HChaCha20 nor ML-KEM). Emitted the first time such an
/// algorithm is used so operators know these are Rust-verified only.
static DUAL_VERIFY_GAP_WARNING: Once = Once::new();

pub(crate) fn warn_dual_verification_gap(algorithm: EncryptionAlgorithm) {
    if matches!(
        algorithm,
        EncryptionAlgorithm::XChaCha20Poly1305 | EncryptionAlgorithm::KyberHybridAes256Gcm
    ) {
        DUAL_VERIFY_GAP_WARNING.call_once(|| {
            log::warn!(
                "{:?} is not covered by the OCaml dual-verification service; \
                 XChaCha20-Poly1305 and ML-KEM-768 hybrid ciphertexts are verified \
                 by the Rust implementation only (see docs/encryption.md)",
                algorithm
            );
        });
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
    warn_dual_verification_gap(algorithm);
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
                        // Passphrase-derived seed material — wiped on drop.
                        let mut d_bytes = Zeroizing::new([0u8; 32]);
                        let mut z_bytes = Zeroizing::new([0u8; 32]);
                        hk.expand(b"ml-kem-768-keygen-d", &mut *d_bytes)
                            .map_err(|_| DecryptError::InvalidKey)?;
                        hk.expand(b"ml-kem-768-keygen-z", &mut *z_bytes)
                            .map_err(|_| DecryptError::InvalidKey)?;
                        let d: B32 = (*d_bytes).into();
                        let z: B32 = (*z_bytes).into();
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
                        // Passphrase-derived shared secret — wiped on drop.
                        let shared_secret: Zeroizing<[u8; 32]> =
                            Zeroizing::new(secret_hasher.finalize().into());

                        let mut key_hasher = Sha256::new();
                        key_hasher.update(*shared_secret);
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

fn verifier_http_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(2))
        .timeout(std::time::Duration::from_secs(5))
        // Verification bodies contain plaintext and raw encryption keys.
        // Never replay them to a redirect target, even when the verifier is
        // compromised or its URL is misconfigured.
        .redirect(reqwest::redirect::Policy::none())
        .build()
}

pub(super) fn validate_verifier_base_url(raw: &str) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse(raw.trim())
        .map_err(|_| "CRYPTO_VERIFIER_URL must be a valid URL".to_string())?;
    let loopback_or_private_ingress = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(domain)) => {
            domain.eq_ignore_ascii_case("localhost")
                || domain.to_ascii_lowercase().ends_with(".internal")
        }
        None => false,
    };
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.path().trim_matches('/').is_empty()
        || !(url.scheme() == "https" || (url.scheme() == "http" && loopback_or_private_ingress))
    {
        return Err(
            "CRYPTO_VERIFIER_URL must be clean HTTPS or HTTP on loopback/Fly private ingress"
                .to_string(),
        );
    }
    url.set_path("/");
    Ok(url)
}

fn strict_verification_from_env() -> Result<bool, String> {
    match std::env::var("COPYPASTE_REQUIRE_CRYPTO_VERIFICATION") {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            _ => Err("COPYPASTE_REQUIRE_CRYPTO_VERIFICATION must be true or false".to_string()),
        },
        Err(std::env::VarError::NotPresent) => Ok(false),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err("COPYPASTE_REQUIRE_CRYPTO_VERIFICATION must contain valid UTF-8".to_string())
        }
    }
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
    let verifier_url = validate_verifier_base_url(&verifier_url)?;
    let require_verification = strict_verification_from_env()?;

    let client = match verifier_http_client() {
        Ok(client) => client,
        Err(_) => {
            log::warn!("Failed to create HTTP client for crypto verification");
            if require_verification {
                return Err("Crypto verification unavailable (client build failed)".to_string());
            }
            return Ok(());
        }
    };

    let endpoint = verifier_url
        .join(&format!("verify/{verification_type}"))
        .map_err(|_| "Invalid crypto verifier endpoint".to_string())?;

    match client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .body(request_body)
        .send()
        .await
    {
        Ok(mut response) => {
            let status = response.status();
            if status.is_success() {
                let mut body = Vec::new();
                loop {
                    let chunk = match response.chunk().await {
                        Ok(Some(chunk)) => chunk,
                        Ok(None) => break,
                        Err(_) if require_verification => {
                            return Err(
                                "Crypto verification response could not be read".to_string()
                            );
                        }
                        Err(_) => {
                            log::warn!("OCaml crypto verification response could not be read");
                            return Ok(());
                        }
                    };
                    if body.len().saturating_add(chunk.len()) > MAX_VERIFIER_RESPONSE_BYTES {
                        if require_verification {
                            return Err("Crypto verification response was too large".to_string());
                        }
                        log::warn!("OCaml crypto verification response exceeded limit");
                        return Ok(());
                    }
                    body.extend_from_slice(&chunk);
                }
                match serde_json::from_slice::<serde_json::Value>(&body) {
                    Ok(json) => {
                        if json.get("valid").and_then(|v| v.as_bool()).unwrap_or(false) {
                            log::info!("Cryptographic verification successful via OCaml service");
                            Ok(())
                        } else {
                            log::error!(
                                "OCaml crypto verifier returned valid=false for {}",
                                verification_type
                            );
                            if require_verification {
                                Err("Crypto verification failed".to_string())
                            } else {
                                Ok(())
                            }
                        }
                    }
                    Err(_) => {
                        log::warn!("Failed to parse OCaml verification response");
                        if require_verification {
                            Err("Crypto verification response parse failed".to_string())
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
        Err(_) => {
            log::warn!("OCaml crypto verification service unavailable");
            if require_verification {
                Err("Crypto verification service unreachable".to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    /// The dual-verification gap warning must be callable repeatedly for any
    /// algorithm without panicking (it fires at most once per process).
    #[test]
    fn dual_verification_gap_warning_is_idempotent() {
        warn_dual_verification_gap(EncryptionAlgorithm::XChaCha20Poly1305);
        warn_dual_verification_gap(EncryptionAlgorithm::XChaCha20Poly1305);
        warn_dual_verification_gap(EncryptionAlgorithm::KyberHybridAes256Gcm);
        // Algorithms with OCaml coverage never trigger the warning path.
        warn_dual_verification_gap(EncryptionAlgorithm::Aes256Gcm);
        warn_dual_verification_gap(EncryptionAlgorithm::None);
    }

    #[test]
    fn verifier_payloads_exist_only_for_supported_algorithms() {
        for algorithm in [
            EncryptionAlgorithm::Aes256Gcm,
            EncryptionAlgorithm::ChaCha20Poly1305,
        ] {
            let (_, verification) = encrypt_content_sync("payload", "strong-test-key", algorithm)
                .expect("supported encryption");
            assert!(
                verification.is_some(),
                "{algorithm:?} must fail closed through OCaml"
            );
        }

        for algorithm in [
            EncryptionAlgorithm::XChaCha20Poly1305,
            EncryptionAlgorithm::KyberHybridAes256Gcm,
        ] {
            let (_, verification) = encrypt_content_sync("payload", "strong-test-key", algorithm)
                .expect("unsupported-verifier encryption");
            assert!(
                verification.is_none(),
                "{algorithm:?} must bypass an unsupported verifier capability"
            );
        }
    }

    #[tokio::test]
    async fn verifier_client_never_replays_secrets_through_redirects() {
        let server = MockServer::start();
        let base = server.base_url();
        let redirected_url = format!("{base}/captured");
        let redirect = server.mock(|when, then| {
            when.method(POST).path("/verify/encryption");
            then.status(307).header("Location", redirected_url.as_str());
        });
        let captured = server.mock(|when, then| {
            when.method(POST).path("/captured");
            then.status(200).body(r#"{"valid":true}"#);
        });

        let response = verifier_http_client()
            .expect("verifier client")
            .post(format!("{base}/verify/encryption"))
            .body(r#"{"plaintext":"secret","key":"raw-key"}"#)
            .send()
            .await
            .expect("redirect response");

        assert_eq!(response.status(), reqwest::StatusCode::TEMPORARY_REDIRECT);
        redirect.assert();
        assert_eq!(captured.calls(), 0);
    }

    #[test]
    fn verifier_url_rejects_untrusted_plaintext_and_credentialed_origins() {
        assert!(validate_verifier_base_url("http://example.com:8001").is_err());
        assert!(validate_verifier_base_url("https://user:pass@example.com").is_err());
        assert!(validate_verifier_base_url("https://example.com?token=x").is_err());
        assert!(validate_verifier_base_url("http://localhost:8001").is_ok());
        assert!(
            validate_verifier_base_url("http://crypto-verifier.process.example.internal:8001")
                .is_ok()
        );
        assert!(validate_verifier_base_url("https://verifier.example.com").is_ok());
    }
}
