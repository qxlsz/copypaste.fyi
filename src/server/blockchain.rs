use std::{env, sync::Arc};

use async_trait::async_trait;
use hex::encode as hex_encode;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{AttestationRequirement, PasteFormat, PasteMetadata, StoredContent, StoredPaste};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnchorManifest {
    pub id: String,
    pub format: PasteFormat,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    pub burn_after_reading: bool,
    pub content: StoredContent,
    pub metadata: PasteMetadata,
}

impl AnchorManifest {
    pub fn from_paste(id: impl Into<String>, paste: &StoredPaste) -> Self {
        Self {
            id: id.into(),
            format: paste.format,
            created_at: paste.created_at,
            expires_at: paste.expires_at,
            burn_after_reading: paste.burn_after_reading,
            content: paste.content.clone(),
            metadata: paste.metadata.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnchorPayload {
    pub manifest: AnchorManifest,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_class: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation_ref: Option<String>,
}

impl AnchorPayload {
    pub fn new(
        manifest: AnchorManifest,
        hash: String,
        retention_class: Option<u8>,
        attestation_ref: Option<String>,
    ) -> Self {
        Self {
            manifest,
            hash,
            retention_class,
            attestation_ref,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnchorReceipt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
}

#[derive(Debug, Error)]
pub enum AnchorError {
    #[error("failed to serialize manifest: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("relayer error: {0}")]
    Relayer(String),
}

pub fn manifest_hash(manifest: &AnchorManifest) -> Result<String, AnchorError> {
    let bytes = serde_json::to_vec(manifest)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex_encode(hasher.finalize()))
}

const RETENTION_MAP: &[(i64, u8)] = &[
    (5, 1),
    (60, 2),
    (1440, 3),
    (4320, 4),
    (10_080, 5),
    (20_160, 6),
    (43_200, 7),
    (86_400, 8),
];

pub fn infer_retention_class(manifest: &AnchorManifest) -> Option<u8> {
    let expires_at = manifest.expires_at?;
    if expires_at <= manifest.created_at {
        return None;
    }
    let ttl_secs = expires_at - manifest.created_at;
    let ttl_minutes = ttl_secs / 60;
    RETENTION_MAP
        .iter()
        .find(|(minutes, _)| *minutes == ttl_minutes)
        .map(|(_, class)| *class)
}

pub fn infer_attestation_ref(metadata: &PasteMetadata) -> Option<String> {
    match metadata.attestation.as_ref() {
        Some(AttestationRequirement::Totp { issuer, .. }) => issuer.clone(),
        Some(AttestationRequirement::SharedSecret { hash }) => {
            Some(format!("shared_secret:{}", hash))
        }
        None => None,
    }
}

#[async_trait]
pub trait AnchorRelayer: Send + Sync + 'static {
    async fn submit(&self, payload: AnchorPayload) -> Result<AnchorReceipt, AnchorError>;
}

pub type SharedAnchorRelayer = Arc<dyn AnchorRelayer>;

#[derive(Default)]
pub struct NoopAnchorRelayer;

#[async_trait]
impl AnchorRelayer for NoopAnchorRelayer {
    async fn submit(&self, payload: AnchorPayload) -> Result<AnchorReceipt, AnchorError> {
        println!(
            "[anchor] noop relayer invoked for paste {} (hash {}, retention_class {:?}, attestation_ref {:?})",
            payload.manifest.id, payload.hash, payload.retention_class, payload.attestation_ref
        );
        Ok(AnchorReceipt::default())
    }
}

pub fn default_anchor_relayer() -> SharedAnchorRelayer {
    match env::var("ANCHOR_RELAY_ENDPOINT") {
        Ok(endpoint) if !endpoint.trim().is_empty() => {
            let api_key = env::var("ANCHOR_RELAY_API_KEY").ok();
            Arc::new(HttpAnchorRelayer::new(endpoint, api_key))
        }
        _ => Arc::new(NoopAnchorRelayer),
    }
}

#[derive(Clone)]
pub struct HttpAnchorRelayer {
    client: Client,
    endpoint: String,
    api_key: Option<String>,
}

impl HttpAnchorRelayer {
    pub fn new(endpoint: impl Into<String>, api_key: Option<String>) -> Self {
        let client = Client::builder()
            .user_agent("copypaste-anchor/0.1.0")
            .build()
            .expect("anchor http client");

        Self {
            client,
            endpoint: endpoint.into(),
            api_key,
        }
    }
}

#[async_trait]
impl AnchorRelayer for HttpAnchorRelayer {
    async fn submit(&self, payload: AnchorPayload) -> Result<AnchorReceipt, AnchorError> {
        let mut request = self.client.post(&self.endpoint).json(&payload);
        if let Some(token) = &self.api_key {
            request = request.bearer_auth(token);
        }

        let response = request
            .send()
            .await
            .map_err(|error| AnchorError::Relayer(error.to_string()))?
            .error_for_status()
            .map_err(|error| AnchorError::Relayer(error.to_string()))?;

        response
            .json::<AnchorReceipt>()
            .await
            .map_err(|error| AnchorError::Relayer(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_hash_is_stable() {
        let metadata = PasteMetadata::default();
        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "hello world".into(),
            },
            format: PasteFormat::PlainText,
            created_at: 42,
            expires_at: Some(84),
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
        let manifest = AnchorManifest::from_paste("abc123", &paste);
        let hash = manifest_hash(&manifest).expect("hash");
        assert_eq!(
            hash,
            "8bfbab22eec3935ec83940c9192e763c2f4e344f2a3f90a1431be917981e620b"
        );

        let same_hash = manifest_hash(&manifest).expect("hash");
        assert_eq!(hash, same_hash);
    }
}
