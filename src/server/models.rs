use copypaste::{
    BundleMetadata, DailyCount, EncryptionAlgorithm, FormatUsage, PasteFormat, StoreStats,
    WebhookProvider,
};
use rocket::form::FromForm;
use rocket::serde::{Deserialize, Serialize};

use crate::server::attestation::AttestationRequest;
use crate::server::blockchain::{AnchorManifest, AnchorReceipt};

#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct EncryptionRequest {
    pub algorithm: EncryptionAlgorithm,
    pub key: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePasteResponse {
    pub id: String,
    pub path: String,
    pub shareable_url: String,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AnchorRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_class: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation_ref: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnchorResponse {
    pub paste_id: String,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_class: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation_ref: Option<String>,
    pub manifest: AnchorManifest,
    pub receipt: AnchorReceipt,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteViewResponse {
    pub id: String,
    pub format: PasteFormat,
    pub content: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub burn_after_reading: bool,
    pub bundle: Option<BundleMetadata>,
    pub encryption: PasteEncryptionInfo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_lock: Option<PasteTimeLockInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<PasteAttestationInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistence: Option<PastePersistenceInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook: Option<PasteWebhookInfo>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteEncryptionInfo {
    pub algorithm: EncryptionAlgorithm,
    pub requires_key: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteTimeLockInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_before: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_after: Option<i64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteAttestationInfo {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PastePersistenceInfo {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteWebhookInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<WebhookProvider>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsSummaryResponse {
    pub total_pastes: usize,
    pub active_pastes: usize,
    pub expired_pastes: usize,
    pub burn_after_reading_count: usize,
    pub time_locked_count: usize,
    pub formats: Vec<FormatUsageResponse>,
    pub encryption_usage: Vec<EncryptionUsageResponse>,
    pub created_by_day: Vec<DailyCountResponse>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatUsageResponse {
    pub format: PasteFormat,
    pub count: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptionUsageResponse {
    pub algorithm: EncryptionAlgorithm,
    pub count: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyCountResponse {
    pub date: String,
    pub count: usize,
}

impl From<StoreStats> for StatsSummaryResponse {
    fn from(stats: StoreStats) -> Self {
        Self {
            total_pastes: stats.total_pastes,
            active_pastes: stats.active_pastes,
            expired_pastes: stats.expired_pastes,
            burn_after_reading_count: stats.burn_after_reading_count,
            time_locked_count: stats.time_locked_count,
            formats: stats
                .formats
                .into_iter()
                .map(|FormatUsage { format, count }| FormatUsageResponse { format, count })
                .collect(),
            encryption_usage: stats
                .encryption_usage
                .into_iter()
                .map(
                    |copypaste::EncryptionUsage { algorithm, count }| EncryptionUsageResponse {
                        algorithm,
                        count,
                    },
                )
                .collect(),
            created_by_day: stats
                .created_by_day
                .into_iter()
                .map(|DailyCount { date, count }| DailyCountResponse { date, count })
                .collect(),
        }
    }
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
