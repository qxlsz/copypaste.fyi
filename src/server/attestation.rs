use base64::Engine;
use copypaste::AttestationRequirement;
use data_encoding::BASE32;
use hmac::{Hmac, Mac};
use rocket::serde::Deserialize;
use sha1::Sha1;
use sha2::{Digest, Sha256};

use super::models::PasteViewQuery;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AttestationRequest {
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

#[derive(Copy, Clone)]
pub enum AttestationVerdict {
    Granted,
    Prompt { invalid: bool },
}

type HmacSha1 = Hmac<Sha1>;

pub fn verify_attestation(
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
            let encoded = base64::engine::general_purpose::STANDARD.encode(digest);
            if &encoded == hash {
                AttestationVerdict::Granted
            } else {
                AttestationVerdict::Prompt { invalid: true }
            }
        }
    }
}

pub fn requirement_from_request(
    request: &AttestationRequest,
) -> Result<AttestationRequirement, String> {
    Ok(match request {
        AttestationRequest::Totp {
            secret,
            digits,
            step,
            allowed_drift,
            issuer,
        } => {
            let secret = secret.trim();
            if secret.is_empty() {
                return Err("TOTP secret cannot be empty".into());
            }
            let digits = digits.unwrap_or(6);
            if !(4..=10).contains(&digits) {
                return Err("TOTP digits must be between 4 and 10".into());
            }
            let step = step.unwrap_or(30);
            if step == 0 {
                return Err("TOTP step must be greater than zero".into());
            }
            let allowed_drift = allowed_drift.unwrap_or(1);
            AttestationRequirement::Totp {
                secret: secret.to_string(),
                digits,
                step,
                allowed_drift,
                issuer: issuer.clone(),
            }
        }
        AttestationRequest::SharedSecret { secret } => {
            let secret = secret.trim();
            if secret.is_empty() {
                return Err("Shared secret cannot be empty".into());
            }
            let mut hasher = Sha256::new();
            hasher.update(secret.as_bytes());
            let digest = hasher.finalize();
            AttestationRequirement::SharedSecret {
                hash: base64::engine::general_purpose::STANDARD.encode(digest),
            }
        }
    })
}

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

    if step == 0 {
        return false;
    }

    let now = now.max(0) as u64;
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
