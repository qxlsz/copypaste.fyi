use crate::server::api_keys::ApiScope;
use crate::{
    BundleMetadata, DailyCount, EncryptionAlgorithm, EncryptionUsage, FormatUsage, PasteFormat,
    StoreStats, WebhookProvider,
};
use rocket::form::FromForm;
use rocket::serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::server::attestation::AttestationRequest;
use crate::server::blockchain::{AnchorManifest, AnchorReceipt};

#[derive(Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct EncryptionRequest {
    pub algorithm: EncryptionAlgorithm,
    pub key: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatePasteResponse {
    pub id: String,
    pub path: String,
    pub shareable_url: String,
    /// Ownership token — only present when `live: true` was requested.
    /// Store this securely; it authorises PUT and PATCH updates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default)]
    pub is_live: bool,
}

#[derive(Serialize, Deserialize, Default, ToSchema)]
#[serde(default, rename_all = "camelCase")]
pub struct AnchorRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_class: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation_ref: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
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

#[derive(Serialize, Deserialize, ToSchema)]
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
    #[serde(default)]
    pub tor_access_only: bool,
    #[serde(default)]
    pub access_count: u64,
    #[serde(default)]
    pub is_live: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_lock: Option<PasteTimeLockInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<PasteAttestationInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistence: Option<PastePersistenceInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook: Option<PasteWebhookInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stego: Option<PasteStegoInfo>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PasteEncryptionInfo {
    pub algorithm: EncryptionAlgorithm,
    pub requires_key: bool,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PasteTimeLockInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_before: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_after: Option<i64>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PasteAttestationInfo {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PastePersistenceInfo {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PasteWebhookInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<WebhookProvider>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PasteStegoInfo {
    pub carrier_mime: String,
    pub carrier_image: String,
    pub payload_digest: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
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

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FormatUsageResponse {
    pub format: PasteFormat,
    pub count: usize,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EncryptionUsageResponse {
    pub algorithm: EncryptionAlgorithm,
    pub count: usize,
}

#[derive(Serialize, Deserialize, ToSchema)]
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
                    |EncryptionUsage { algorithm, count }| EncryptionUsageResponse {
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

#[derive(Serialize, Deserialize, Default, Clone, ToSchema)]
#[serde(default)]
pub struct CreateBundleRequest {
    pub children: Vec<CreateBundleChildRequest>,
}

#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct CreateBundleChildRequest {
    pub content: String,
    #[serde(default)]
    pub format: Option<PasteFormat>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Serialize, Deserialize, Default, ToSchema)]
#[serde(default)]
pub struct TimeLockRequest {
    pub not_before: Option<String>,
    pub not_after: Option<String>,
}

#[derive(Serialize, Deserialize, Default, ToSchema)]
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
    #[serde(default)]
    pub stego: Option<StegoRequest>,
    #[serde(default)]
    pub tor_access_only: bool,
    pub owner_pubkey_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// When `true`, paste starts in live mode — content can be updated via `PUT /api/pastes/{id}`.
    #[serde(default)]
    pub live: bool,
}

/// Request body for `PUT /api/pastes/{id}` (update live paste content).
#[derive(Serialize, Deserialize, Default, ToSchema)]
#[serde(default)]
pub struct UpdatePasteRequest {
    pub content: String,
    #[serde(default)]
    pub encryption: Option<EncryptionRequest>,
}

/// Request body for `PATCH /api/pastes/{id}` (finalize live paste).
#[derive(Serialize, Deserialize, ToSchema)]
pub struct FinalizePasteRequest {
    pub live: bool,
}

#[derive(Serialize, Deserialize, Default, ToSchema)]
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

#[derive(Serialize, Deserialize, Default, ToSchema)]
#[serde(default)]
pub struct WebhookRequest {
    pub url: String,
    pub provider: Option<WebhookProvider>,
    pub view_template: Option<String>,
    pub burn_template: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, ToSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum StegoRequest {
    Builtin { carrier: String },
    Uploaded { data_uri: String },
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthChallengeResponse {
    pub challenge: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthLoginRequest {
    pub challenge: String,
    pub signature: String,
    pub pubkey: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserPasteCountResponse {
    pub paste_count: usize,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserPasteListItem {
    pub id: String,
    pub url: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub retention_minutes: Option<i64>,
    pub burn_after_reading: bool,
    pub format: String,
    pub access_count: u64,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserPasteListResponse {
    pub pastes: Vec<UserPasteListItem>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthLoginResponse {
    pub token: String,
    pub pubkey_hash: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthLogoutResponse {
    pub success: bool,
}

// ── Admin key management ──────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiKeyRequest {
    pub name: String,
    #[serde(default)]
    pub scope: ApiScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiKeyResponse {
    pub id: String,
    pub name: String,
    pub scope: ApiScope,
    /// Plaintext key — shown **once**, store it securely.
    pub key: String,
    pub created_at: i64,
}

// ── Workspace listing ─────────────────────────────────────────────────────────

#[derive(FromForm, Default)]
pub struct WorkspacePasteQuery {
    pub workspace: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePasteItem {
    pub id: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    pub created_at: i64,
}

// ── Admin key listing & revocation responses ─────────────────────────────────

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyInfo {
    pub id: String,
    pub name: String,
    pub scope: ApiScope,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListApiKeysResponse {
    pub keys: Vec<ApiKeyInfo>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RevokeApiKeyResponse {
    pub revoked: bool,
}

// ── Standardised error shape ──────────────────────────────────────────────────

/// Machine-readable error envelope returned by all API error responses.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    /// Machine-readable error code, e.g. `"paste_not_found"`.
    pub code: String,
    /// Human-readable description.
    pub message: String,
    /// Optional structured details (validation errors, upstream messages, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: Some(details),
        }
    }
}

// ── Existing query ────────────────────────────────────────────────────────────

#[derive(FromForm, Default)]
pub struct PasteViewQuery {
    pub key: Option<String>,
    pub code: Option<String>,
    pub attest: Option<String>,
}
