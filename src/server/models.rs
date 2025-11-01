use copypaste::{EncryptionAlgorithm, PasteFormat, WebhookProvider};
use rocket::form::FromForm;
use rocket::serde::Deserialize;

use crate::server::attestation::AttestationRequest;

#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct EncryptionRequest {
    pub algorithm: EncryptionAlgorithm,
    pub key: String,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct CreateBundleRequest {
    pub children: Vec<CreateBundleChildRequest>,
}

#[derive(Deserialize, Clone)]
pub struct CreateBundleChildRequest {
    pub content: String,
    #[serde(default)]
    pub format: Option<PasteFormat>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct TimeLockRequest {
    pub not_before: Option<String>,
    pub not_after: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct CreatePasteRequest {
    pub content: String,
    #[serde(default)]
    pub format: Option<PasteFormat>,
    pub retention_minutes: Option<u64>,
    pub encryption: Option<EncryptionRequest>,
    #[serde(default)]
    pub burn_after_reading: bool,
    #[serde(default)]
    pub bundle: Option<CreateBundleRequest>,
    #[serde(default)]
    pub time_lock: Option<TimeLockRequest>,
    #[serde(default)]
    pub attestation: Option<AttestationRequest>,
    #[serde(default)]
    pub persistence: Option<PersistenceRequest>,
    #[serde(default)]
    pub webhook: Option<WebhookRequest>,
}

#[derive(Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PersistenceRequest {
    #[default]
    Memory,
    Vault {
        key_path: String,
    },
    S3 {
        bucket: String,
        #[serde(default)]
        prefix: Option<String>,
    },
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct WebhookRequest {
    pub url: String,
    pub provider: Option<WebhookProvider>,
    pub view_template: Option<String>,
    pub burn_template: Option<String>,
}

#[derive(FromForm, Default)]
pub struct PasteViewQuery {
    pub key: Option<String>,
    pub code: Option<String>,
    pub attest: Option<String>,
}
