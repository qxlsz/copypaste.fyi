use std::{
    env,
    io::{self, Write},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use hex::encode as hex_encode;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{EncryptionAlgorithm, PasteFormat, PasteMetadata, StoredContent, StoredPaste};
use utoipa::ToSchema;

const ENCRYPTED_CONTENT_COMMITMENT_DOMAIN: &[u8] = b"copypaste.fyi:anchor:encrypted-content:v1\0";
const MAX_RELAYER_RESPONSE_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnchorAttestationKind {
    Totp,
    SharedSecret,
}

impl AnchorAttestationKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Totp => "totp",
            Self::SharedSecret => "shared_secret",
        }
    }
}

/// Non-sensitive metadata retained in an anchor manifest. Secret-derived
/// commitments are deliberately omitted: deterministic hashes of short paste
/// bodies or attestation secrets would be offline guessing oracles.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnchorMetadataSummary {
    pub has_attestation: bool,
    pub has_webhook: bool,
    pub tor_access_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation_kind: Option<AnchorAttestationKind>,
}

impl AnchorMetadataSummary {
    fn from_metadata(metadata: &PasteMetadata) -> Self {
        let attestation_kind = match metadata.attestation.as_ref() {
            Some(crate::AttestationRequirement::Totp { .. }) => Some(AnchorAttestationKind::Totp),
            Some(crate::AttestationRequirement::SharedSecret { .. }) => {
                Some(AnchorAttestationKind::SharedSecret)
            }
            None => None,
        };

        Self {
            has_attestation: metadata.attestation.is_some(),
            has_webhook: metadata.webhook.is_some(),
            tor_access_only: metadata.tor_access_only,
            attestation_kind,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnchorManifest {
    pub id: String,
    pub format: PasteFormat,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    pub burn_after_reading: bool,
    pub encryption_algorithm: EncryptionAlgorithm,
    /// Commitment to randomized encrypted storage fields only. Plaintext is
    /// never hashed into a public manifest because that enables dictionary
    /// attacks on short or predictable pastes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_content_digest: Option<String>,
    pub metadata: AnchorMetadataSummary,
}

impl AnchorManifest {
    pub fn from_paste(id: impl Into<String>, paste: &StoredPaste) -> Self {
        let (encryption_algorithm, encrypted_content_digest) = match &paste.content {
            StoredContent::Plain { .. } => (EncryptionAlgorithm::None, None),
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
            } => (
                *algorithm,
                Some(sha256_commitment(
                    ENCRYPTED_CONTENT_COMMITMENT_DOMAIN,
                    &(*algorithm, ciphertext, nonce, salt),
                )),
            ),
        };

        Self {
            id: id.into(),
            format: paste.format,
            created_at: paste.created_at,
            expires_at: paste.expires_at,
            burn_after_reading: paste.burn_after_reading,
            encryption_algorithm,
            encrypted_content_digest,
            metadata: AnchorMetadataSummary::from_metadata(&paste.metadata),
        }
    }
}

fn sha256_commitment<T: Serialize>(domain: &[u8], value: &T) -> String {
    // The encrypted storage tuple contains only JSON-safe Rust types. A
    // serialization failure here would indicate a programming invariant was
    // broken, rather than malformed user input.
    let mut hasher = Sha256::new();
    hasher.update(domain);
    serde_json::to_writer(Sha256Writer(&mut hasher), value)
        .expect("anchor commitment input must serialize");
    hex_encode(hasher.finalize())
}

/// Streams serialized commitment input directly into SHA-256 so anchoring a
/// large paste does not allocate a second copy of its sensitive content.
struct Sha256Writer<'a>(&'a mut Sha256);

impl Write for Sha256Writer<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnchorReceipt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
}

#[derive(Debug, Error)]
pub enum AnchorError {
    #[error("failed to serialize manifest: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("invalid relayer response: {0}")]
    Response(serde_json::Error),
    #[error("relayer error: {0}")]
    Relayer(String),
    #[error("invalid relayer configuration: {0}")]
    Configuration(String),
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

pub fn infer_attestation_ref(metadata: &AnchorMetadataSummary) -> Option<String> {
    let kind = metadata.attestation_kind?;
    Some(kind.as_str().to_string())
}

#[async_trait]
pub trait AnchorRelayer: Send + Sync + 'static {
    async fn submit(&self, payload: AnchorPayload) -> Result<AnchorReceipt, AnchorError>;
}

pub type SharedAnchorRelayer = Arc<dyn AnchorRelayer>;

#[derive(Default)]
pub struct DisabledAnchorRelayer;

#[async_trait]
impl AnchorRelayer for DisabledAnchorRelayer {
    async fn submit(&self, _payload: AnchorPayload) -> Result<AnchorReceipt, AnchorError> {
        Err(AnchorError::Configuration(
            "ANCHOR_RELAY_ENDPOINT is not configured".to_string(),
        ))
    }
}

pub fn default_anchor_relayer() -> SharedAnchorRelayer {
    match env::var("ANCHOR_RELAY_ENDPOINT") {
        Ok(endpoint) if !endpoint.trim().is_empty() => {
            let api_key = env::var("ANCHOR_RELAY_API_KEY").ok();
            Arc::new(
                HttpAnchorRelayer::new(endpoint, api_key)
                    .unwrap_or_else(|error| panic!("invalid ANCHOR_RELAY_ENDPOINT: {error}")),
            )
        }
        _ => Arc::new(DisabledAnchorRelayer),
    }
}

#[derive(Clone)]
pub struct HttpAnchorRelayer {
    client: Client,
    endpoint: reqwest::Url,
    api_key: Option<String>,
}

impl HttpAnchorRelayer {
    pub fn new(endpoint: impl AsRef<str>, api_key: Option<String>) -> Result<Self, AnchorError> {
        let endpoint = reqwest::Url::parse(endpoint.as_ref())
            .map_err(|_| AnchorError::Configuration("endpoint must be a valid URL".to_string()))?;
        let loopback = match endpoint.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            Some(url::Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
            None => false,
        };
        if endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || !(endpoint.scheme() == "https" || (endpoint.scheme() == "http" && loopback))
        {
            return Err(AnchorError::Configuration(
                "endpoint must use HTTPS (HTTP is loopback-only) and contain no credentials, query, or fragment"
                    .to_string(),
            ));
        }
        let client = Client::builder()
            .user_agent("copypaste-anchor/0.1.0")
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| AnchorError::Configuration(error.to_string()))?;

        Ok(Self {
            client,
            endpoint,
            api_key,
        })
    }
}

#[async_trait]
impl AnchorRelayer for HttpAnchorRelayer {
    async fn submit(&self, payload: AnchorPayload) -> Result<AnchorReceipt, AnchorError> {
        let mut request = self.client.post(self.endpoint.clone()).json(&payload);
        if let Some(token) = &self.api_key {
            request = request.bearer_auth(token);
        }

        let mut response = request
            .send()
            .await
            .map_err(|error| AnchorError::Relayer(error.without_url().to_string()))?;
        if !response.status().is_success() {
            return Err(AnchorError::Relayer(format!(
                "relayer returned HTTP {}",
                response.status().as_u16()
            )));
        }

        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| AnchorError::Relayer(error.without_url().to_string()))?
        {
            if body.len().saturating_add(chunk.len()) > MAX_RELAYER_RESPONSE_BYTES {
                return Err(AnchorError::Relayer(
                    "relayer response exceeded 16 KiB".to_string(),
                ));
            }
            body.extend_from_slice(&chunk);
        }
        serde_json::from_slice::<AnchorReceipt>(&body).map_err(AnchorError::Response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AttestationRequirement, WebhookConfig, WebhookProvider};

    fn make_paste(
        content: StoredContent,
        metadata: PasteMetadata,
        created_at: i64,
        expires_at: Option<i64>,
    ) -> StoredPaste {
        StoredPaste {
            content,
            format: PasteFormat::PlainText,
            created_at,
            expires_at,
            burn_after_reading: false,
            bundle: metadata.bundle.clone(),
            bundle_parent: metadata.bundle_parent.clone(),
            bundle_label: metadata.bundle_label.clone(),
            not_before: metadata.not_before,
            not_after: metadata.not_after,
            persistence: metadata.persistence.clone(),
            webhook: metadata.webhook.clone(),
            metadata,
            is_live: false,
            owner_token_hash: None,
        }
    }

    fn make_manifest(created_at: i64, expires_at: Option<i64>) -> AnchorManifest {
        let paste = make_paste(
            StoredContent::Plain { text: "x".into() },
            PasteMetadata::default(),
            created_at,
            expires_at,
        );
        AnchorManifest::from_paste("test", &paste)
    }

    #[test]
    fn manifest_hash_is_stable() {
        let metadata = PasteMetadata::default();
        let paste = make_paste(
            StoredContent::Plain {
                text: "hello world".into(),
            },
            metadata,
            42,
            Some(84),
        );
        let manifest = AnchorManifest::from_paste("abc123", &paste);
        let hash = manifest_hash(&manifest).expect("hash");
        let same_hash = manifest_hash(&manifest).expect("hash");
        assert_eq!(hash, same_hash);

        let mut changed = manifest;
        changed.burn_after_reading = true;
        assert_ne!(hash, manifest_hash(&changed).expect("changed hash"));
    }

    #[test]
    fn manifests_never_commit_plaintext_or_secret_metadata() {
        let first_plaintext = make_paste(
            StoredContent::Plain { text: "one".into() },
            PasteMetadata::default(),
            42,
            Some(84),
        );
        let second_plaintext = make_paste(
            StoredContent::Plain { text: "two".into() },
            PasteMetadata::default(),
            42,
            Some(84),
        );
        let secret_metadata = make_paste(
            StoredContent::Plain { text: "one".into() },
            PasteMetadata {
                attestation: Some(crate::AttestationRequirement::SharedSecret {
                    hash: "short-guessable-secret".into(),
                }),
                ..Default::default()
            },
            42,
            Some(84),
        );

        let first = AnchorManifest::from_paste("test", &first_plaintext);
        let second = AnchorManifest::from_paste("test", &second_plaintext);
        let secret = AnchorManifest::from_paste("test", &secret_metadata);

        assert!(first.encrypted_content_digest.is_none());
        assert!(second.encrypted_content_digest.is_none());
        assert_eq!(
            manifest_hash(&first).expect("first hash"),
            manifest_hash(&second).expect("second hash"),
            "plaintext must not influence a public manifest hash"
        );
        let secret_json = serde_json::to_string(&secret).expect("secret manifest");
        assert!(!secret_json.contains("short-guessable-secret"));
        assert!(!secret_json.contains("attestationDigest"));
        assert!(!secret_json.contains("metadataDigest"));
    }

    #[test]
    fn encrypted_content_commitment_covers_randomized_ciphertext_fields() {
        let encrypted = |ciphertext: &str| {
            make_paste(
                StoredContent::Encrypted {
                    algorithm: EncryptionAlgorithm::Aes256Gcm,
                    ciphertext: ciphertext.into(),
                    nonce: "random-nonce".into(),
                    salt: "random-salt".into(),
                },
                PasteMetadata::default(),
                42,
                Some(84),
            )
        };
        let first = AnchorManifest::from_paste("test", &encrypted("ciphertext-one"));
        let second = AnchorManifest::from_paste("test", &encrypted("ciphertext-two"));
        assert_ne!(
            first.encrypted_content_digest,
            second.encrypted_content_digest
        );
    }

    #[test]
    fn infer_retention_class_none_when_no_expires() {
        let manifest = make_manifest(1000, None);
        assert!(infer_retention_class(&manifest).is_none());
    }

    #[test]
    fn infer_retention_class_none_when_expires_leq_created() {
        let manifest = make_manifest(1000, Some(500));
        assert!(infer_retention_class(&manifest).is_none());

        let manifest_equal = make_manifest(1000, Some(1000));
        assert!(infer_retention_class(&manifest_equal).is_none());
    }

    #[test]
    fn infer_retention_class_maps_known_ttls() {
        // 5 minutes = 300 seconds → class 1
        assert_eq!(infer_retention_class(&make_manifest(0, Some(300))), Some(1));
        // 60 minutes = 3600 seconds → class 2
        assert_eq!(
            infer_retention_class(&make_manifest(0, Some(3600))),
            Some(2)
        );
        // 1440 minutes = 86400 seconds → class 3
        assert_eq!(
            infer_retention_class(&make_manifest(0, Some(86_400))),
            Some(3)
        );
    }

    #[test]
    fn infer_retention_class_none_for_unrecognized_ttl() {
        // 7 minutes is not in RETENTION_MAP
        let manifest = make_manifest(0, Some(7 * 60));
        assert!(infer_retention_class(&manifest).is_none());
    }

    #[test]
    fn infer_attestation_ref_none_when_no_attestation() {
        let metadata = PasteMetadata::default();
        let summary = AnchorMetadataSummary::from_metadata(&metadata);
        assert!(infer_attestation_ref(&summary).is_none());
    }

    #[test]
    fn infer_attestation_ref_shared_secret() {
        let metadata = PasteMetadata {
            attestation: Some(AttestationRequirement::SharedSecret {
                hash: "abc123".into(),
            }),
            ..Default::default()
        };
        let summary = AnchorMetadataSummary::from_metadata(&metadata);
        let reference = infer_attestation_ref(&summary).expect("attestation reference");
        assert_eq!(reference, "shared_secret");
    }

    #[test]
    fn infer_attestation_ref_totp_with_issuer() {
        let metadata = PasteMetadata {
            attestation: Some(AttestationRequirement::Totp {
                secret: "BASE32SECRET".into(),
                digits: 6,
                step: 30,
                allowed_drift: 1,
                issuer: Some("Acme Corp".into()),
            }),
            ..Default::default()
        };
        let summary = AnchorMetadataSummary::from_metadata(&metadata);
        let reference = infer_attestation_ref(&summary).expect("attestation reference");
        assert_eq!(reference, "totp");
        assert!(!reference.contains("BASE32SECRET"));
        assert!(!reference.contains("Acme Corp"));
    }

    #[test]
    fn infer_attestation_ref_totp_without_issuer() {
        let metadata = PasteMetadata {
            attestation: Some(AttestationRequirement::Totp {
                secret: "BASE32SECRET".into(),
                digits: 6,
                step: 30,
                allowed_drift: 1,
                issuer: None,
            }),
            ..Default::default()
        };
        let summary = AnchorMetadataSummary::from_metadata(&metadata);
        let reference = infer_attestation_ref(&summary).expect("attestation reference");
        assert_eq!(reference, "totp");
        assert!(!reference.contains("BASE32SECRET"));
    }

    #[test]
    fn serialized_manifests_exclude_sensitive_content_and_metadata() {
        let plaintext = "plain text SECRET <do-not-anchor>";
        let totp_secret = "JBSWY3DPEHPK3PXP";
        let webhook_url = "https://hooks.example.invalid/private/token-123";
        let plaintext_metadata = PasteMetadata {
            attestation: Some(AttestationRequirement::Totp {
                secret: totp_secret.into(),
                digits: 6,
                step: 30,
                allowed_drift: 1,
                issuer: Some("Private Issuer".into()),
            }),
            webhook: Some(WebhookConfig {
                url: webhook_url.into(),
                provider: Some(WebhookProvider::Generic),
                view_template: Some("private view template".into()),
                burn_template: Some("private burn template".into()),
            }),
            ..Default::default()
        };
        let plaintext_paste = make_paste(
            StoredContent::Plain {
                text: plaintext.into(),
            },
            plaintext_metadata,
            42,
            Some(84),
        );
        let plaintext_manifest = AnchorManifest::from_paste("plain", &plaintext_paste);
        let plaintext_json = serde_json::to_string(&plaintext_manifest).expect("manifest JSON");

        for secret in [
            plaintext,
            totp_secret,
            webhook_url,
            "Private Issuer",
            "private view template",
            "private burn template",
        ] {
            assert!(
                !plaintext_json.contains(secret),
                "serialized manifest leaked sensitive value: {secret}"
            );
        }

        let plaintext_value =
            serde_json::to_value(&plaintext_manifest).expect("manifest JSON value");
        assert!(plaintext_value.get("content").is_none());
        assert!(plaintext_value.get("encryptedContentDigest").is_none());
        assert!(plaintext_value.get("metadataDigest").is_none());
        let metadata = plaintext_value
            .get("metadata")
            .and_then(serde_json::Value::as_object)
            .expect("safe metadata summary");
        assert!(metadata.get("attestation").is_none());
        assert!(metadata.get("webhook").is_none());
        assert_eq!(
            metadata.get("hasAttestation"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(metadata.get("hasWebhook"), Some(&serde_json::json!(true)));

        let ciphertext = "ciphertext-SENSITIVE-PAYLOAD";
        let nonce = "nonce-SENSITIVE";
        let salt = "salt-SENSITIVE";
        let shared_secret_hash = "stored-shared-secret-hash-SENSITIVE";
        let encrypted_metadata = PasteMetadata {
            attestation: Some(AttestationRequirement::SharedSecret {
                hash: shared_secret_hash.into(),
            }),
            ..Default::default()
        };
        let encrypted_paste = make_paste(
            StoredContent::Encrypted {
                algorithm: EncryptionAlgorithm::Aes256Gcm,
                ciphertext: ciphertext.into(),
                nonce: nonce.into(),
                salt: salt.into(),
            },
            encrypted_metadata,
            42,
            Some(84),
        );
        let encrypted_json =
            serde_json::to_string(&AnchorManifest::from_paste("encrypted", &encrypted_paste))
                .expect("manifest JSON");

        for secret in [ciphertext, nonce, salt, shared_secret_hash] {
            assert!(
                !encrypted_json.contains(secret),
                "serialized manifest leaked sensitive value: {secret}"
            );
        }
    }

    #[test]
    fn anchor_payload_new_stores_all_fields() {
        let manifest = make_manifest(0, None);
        let payload =
            AnchorPayload::new(manifest, "hash123".into(), Some(2), Some("ref-val".into()));
        assert_eq!(payload.hash, "hash123");
        assert_eq!(payload.retention_class, Some(2));
        assert_eq!(payload.attestation_ref, Some("ref-val".into()));
    }

    #[test]
    fn anchor_payload_new_accepts_none_fields() {
        let manifest = make_manifest(0, None);
        let payload = AnchorPayload::new(manifest, "h".into(), None, None);
        assert!(payload.retention_class.is_none());
        assert!(payload.attestation_ref.is_none());
    }

    #[tokio::test]
    async fn disabled_relayer_fails_instead_of_claiming_an_anchor() {
        let relayer = DisabledAnchorRelayer;
        let payload = AnchorPayload::new(make_manifest(0, None), "hash".into(), None, None);
        let error = relayer
            .submit(payload)
            .await
            .expect_err("disabled anchoring must fail closed");
        assert!(matches!(error, AnchorError::Configuration(_)));
    }

    #[test]
    fn relayer_rejects_remote_plain_http_and_url_credentials() {
        assert!(HttpAnchorRelayer::new("http://example.com/relay", None).is_err());
        assert!(HttpAnchorRelayer::new("https://user:pass@example.com/relay", None).is_err());
    }

    #[tokio::test]
    async fn relayer_does_not_follow_redirects_with_api_key_or_manifest() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        let base = server.base_url();
        let redirected_url = format!("{base}/capture");
        let redirect = server.mock(|when, then| {
            when.method(POST)
                .path("/relay")
                .header("authorization", "Bearer relayer-secret");
            then.status(307).header("Location", redirected_url.as_str());
        });
        let capture = server.mock(|when, then| {
            when.method(POST).path("/capture");
            then.status(200).body("{}");
        });
        let relayer =
            HttpAnchorRelayer::new(format!("{base}/relay"), Some("relayer-secret".to_string()))
                .expect("loopback relayer");
        let payload = AnchorPayload::new(make_manifest(0, None), "hash".into(), None, None);

        let error = relayer
            .submit(payload)
            .await
            .expect_err("redirect is not success");
        assert!(matches!(error, AnchorError::Relayer(_)));
        redirect.assert();
        assert_eq!(capture.calls(), 0);
    }

    #[test]
    fn default_anchor_relayer_without_endpoint_is_disabled() {
        std::env::remove_var("ANCHOR_RELAY_ENDPOINT");
        let _relayer = default_anchor_relayer();
        // Just verifies construction doesn't panic
    }
}
