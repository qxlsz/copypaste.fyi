use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use rocket::{
    data::{Limits, ToByteUnit},
    delete,
    fs::FileServer,
    get,
    http::Status,
    patch, post, put,
    request::{FromRequest, Outcome},
    response::content,
    routes,
    serde::json::Json,
    Build, Request, Rocket, State,
};
use subtle::ConstantTimeEq;

use crate::{
    create_paste_store, AttestationRequirement, EncryptionAlgorithm, PasteError, PasteFormat,
    PasteMetadata, PersistenceLocator, SharedPasteStore, StoredContent, StoredPaste, WebhookConfig,
};
use sha2::{Digest, Sha256};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rand::Rng;

use super::api_keys::{
    RateLimiter, RequireAdminAuth, SharedApiKeyStore, SharedRateLimiter, SqliteApiKeyStore,
};
use super::attestation::{self, AttestationVerdict};
use super::blockchain::{
    default_anchor_relayer, infer_attestation_ref, infer_retention_class, manifest_hash,
    AnchorManifest, AnchorPayload, SharedAnchorRelayer,
};
use super::bundles::build_bundle_overview;
use super::cors::{api_preflight, Cors};
use super::crypto::{decrypt_content, encrypt_content, DecryptError};
use super::models::{
    AnchorRequest, AnchorResponse, ApiError, ApiKeyInfo, AuthChallengeResponse, AuthLoginRequest,
    AuthLoginResponse, AuthLogoutResponse, CreateApiKeyRequest, CreateApiKeyResponse,
    CreatePasteRequest, CreatePasteResponse, FinalizePasteRequest, FinalizePasteResponse,
    ListApiKeysResponse, PasteAttestationInfo, PasteEncryptionInfo, PastePersistenceInfo,
    PasteStegoInfo, PasteTimeLockInfo, PasteViewQuery, PasteViewResponse, PasteWebhookInfo,
    PersistenceRequest, RevokeApiKeyResponse, StatsSummaryResponse, StegoRequest, TimeLockRequest,
    UpdatePasteRequest, UpdatePasteResponse, UserPasteCountResponse, UserPasteListItem,
    UserPasteListResponse, WebhookRequest, WorkspacePasteItem, WorkspacePasteListResponse,
};
use super::rate_limit::{CreateRateLimit, PasteRateLimiter, ReadRateLimit};
use super::render::{
    render_attestation_prompt, render_expired, render_invalid_key, render_key_prompt,
    render_paste_view, render_time_locked, StoredPasteView,
};
use super::sessions::{BearerToken, RequireUserSession, SessionStore, SharedSessionStore};
use super::stego::{embed_payload, parse_data_uri, StegoCarrierSource};
use super::time::{current_timestamp, evaluate_time_lock, parse_timestamp, TimeLockState};
use super::tor::{OnionAccess, TorConfig};
use super::webhook::{trigger_webhook, validate_webhook_url, WebhookClient, WebhookEvent};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};
use utoipa_scalar::{Scalar, Servable};

pub fn build_rocket(store: SharedPasteStore) -> Rocket<Build> {
    let tor_config = TorConfig::from_env();
    let api_key_store: SharedApiKeyStore = std::sync::Arc::new(
        SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"),
    );
    let rate_limiter: SharedRateLimiter = std::sync::Arc::new(RateLimiter::new());
    let webhook_client = WebhookClient::new();
    let session_store: SharedSessionStore = std::sync::Arc::new(SessionStore::new());
    let paste_rate_limiter = PasteRateLimiter::from_env();

    rocket::build()
        .configure(rocket::Config {
            limits: Limits::default().limit("json", 11u64.mebibytes()),
            ..Default::default()
        })
        .manage(store)
        .manage(default_anchor_relayer())
        .manage(tor_config)
        .manage(api_key_store)
        .manage(rate_limiter)
        .manage(webhook_client)
        .manage(session_store)
        .manage(paste_rate_limiter)
        .attach(Cors)
        .mount(
            "/",
            routes![
                api_preflight,
                index,
                about,
                create,
                create_api,
                update_api,
                finalize_api,
                anchor_api,
                show_api,
                show,
                show_raw,
                stats_summary_api,
                auth_challenge_api,
                auth_login_api,
                auth_logout_api,
                user_paste_count_api,
                user_paste_list_api,
                workspace_pastes_api,
                health_api,
                health_detailed_api,
                admin_create_key_api,
                admin_list_keys_api,
                admin_delete_key_api,
                openapi_json,
                spa_fallback
            ],
        )
        .mount("/", Scalar::with_url("/api/docs", ApiDoc::openapi()))
        .mount("/static", FileServer::from("static"))
}

pub async fn launch() -> Result<(), Box<dyn std::error::Error>> {
    let store = create_paste_store();
    build_rocket(store).launch().await?;
    Ok(())
}

/// OpenAPI document aggregating all `#[utoipa::path]`-annotated handlers.
/// Rendered interactively by Scalar at `/api/docs`; the raw JSON document is
/// served at `/api/openapi.json`.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "copypaste.fyi API",
        description = "Paste sharing with encryption, burn-after-reading, time locks, attestation, webhooks, and blockchain anchoring."
    ),
    paths(
        create,
        create_api,
        update_api,
        finalize_api,
        show_api,
        show,
        anchor_api,
        stats_summary_api,
        auth_challenge_api,
        auth_login_api,
        auth_logout_api,
        user_paste_count_api,
        user_paste_list_api,
        workspace_pastes_api,
        health_detailed_api,
    ),
    components(schemas(
        CreatePasteRequest,
        CreatePasteResponse,
        UpdatePasteRequest,
        UpdatePasteResponse,
        FinalizePasteRequest,
        FinalizePasteResponse,
        PasteViewResponse,
        PasteEncryptionInfo,
        PasteTimeLockInfo,
        PasteAttestationInfo,
        PastePersistenceInfo,
        PasteWebhookInfo,
        PasteStegoInfo,
        AnchorRequest,
        AnchorResponse,
        StatsSummaryResponse,
        AuthChallengeResponse,
        AuthLoginRequest,
        AuthLoginResponse,
        AuthLogoutResponse,
        UserPasteCountResponse,
        UserPasteListItem,
        UserPasteListResponse,
        WorkspacePasteItem,
        WorkspacePasteListResponse,
        TimeLockRequest,
        PersistenceRequest,
        WebhookRequest,
        StegoRequest,
        ApiError,
        super::models::EncryptionRequest,
        super::models::CreateBundleRequest,
        super::models::CreateBundleChildRequest,
        super::attestation::AttestationRequest,
        crate::PasteFormat,
        crate::EncryptionAlgorithm,
        crate::BundleMetadata,
        crate::BundlePointer,
        crate::WebhookProvider,
        crate::StoredContent,
        crate::PasteMetadata,
        crate::AttestationRequirement,
        crate::PersistenceLocator,
        crate::WebhookConfig,
        super::models::FormatUsageResponse,
        super::models::EncryptionUsageResponse,
        super::models::DailyCountResponse,
        super::blockchain::AnchorManifest,
        super::blockchain::AnchorReceipt,
    ))
)]
struct ApiDoc;

/// Raw OpenAPI 3 document as JSON.
#[get("/api/openapi.json")]
async fn openapi_json() -> content::RawJson<String> {
    content::RawJson(
        ApiDoc::openapi()
            .to_pretty_json()
            .unwrap_or_else(|_| "{}".to_string()),
    )
}

#[derive(Serialize, Deserialize, ToSchema)]
struct HealthResponse {
    status: String,
    timestamp: i64,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema)]
struct DetailedHealthResponse {
    status: String,
    timestamp: i64,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_message: Option<String>,
    services: ServiceHealth,
}

#[derive(Serialize, Deserialize)]
struct ServiceHealth {
    backend: ServiceStatus,
    crypto_verifier: ServiceStatus,
    storage: ServiceStatus,
}

#[derive(Serialize, Deserialize)]
struct ServiceStatus {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[get("/health")]
async fn health_api() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        timestamp: current_timestamp(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit: option_env!("GIT_COMMIT").map(String::from),
    })
}

#[utoipa::path(
    get,
    path = "/api/health",
    responses((status = 200, description = "Detailed health", body = DetailedHealthResponse))
)]
#[get("/api/health")]
async fn health_detailed_api(store: &State<SharedPasteStore>) -> Json<DetailedHealthResponse> {
    // Check storage
    let stats = store.stats().await;
    let storage_status = ServiceStatus {
        status: "ok".to_string(),
        message: Some(format!("Total pastes: {}", stats.total_pastes)),
    };

    // Check crypto verifier
    let crypto_verifier_url = std::env::var("CRYPTO_VERIFIER_URL")
        .unwrap_or_else(|_| "http://localhost:8001".to_string());

    let crypto_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_default();
    let crypto_status = match crypto_client
        .get(format!("{}/health", crypto_verifier_url))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => ServiceStatus {
            status: "ok".to_string(),
            message: Some("Crypto verifier responding".to_string()),
        },
        Ok(resp) => ServiceStatus {
            status: "degraded".to_string(),
            message: Some(format!("HTTP {}", resp.status())),
        },
        Err(e) => ServiceStatus {
            status: "unavailable".to_string(),
            message: Some(format!("Connection failed: {}", e)),
        },
    };

    let overall_status = if storage_status.status == "ok" && crypto_status.status == "ok" {
        "ok"
    } else if storage_status.status == "unavailable" || crypto_status.status == "unavailable" {
        "unavailable"
    } else {
        "degraded"
    };

    Json(DetailedHealthResponse {
        status: overall_status.to_string(),
        timestamp: current_timestamp(),
        version: option_env!("COPYPASTE_VERSION")
            .map(String::from)
            .or_else(|| std::env::var("COPYPASTE_VERSION").ok())
            .unwrap_or_else(|| "unknown".to_string()),
        commit: option_env!("GIT_COMMIT").map(String::from),
        commit_message: option_env!("GIT_COMMIT_MESSAGE").map(String::from),
        services: ServiceHealth {
            backend: ServiceStatus {
                status: "ok".to_string(),
                message: Some("Backend operational".to_string()),
            },
            crypto_verifier: crypto_status,
            storage: storage_status,
        },
    })
}

#[utoipa::path(
    get,
    path = "/api/stats/summary",
    responses((status = 200, description = "Stats summary", body = StatsSummaryResponse))
)]
#[get("/api/stats/summary")]
async fn stats_summary_api(
    store: &State<SharedPasteStore>,
    onion: OnionAccess,
) -> Json<StatsSummaryResponse> {
    if onion.suppress_logs() {
        rocket::info!("stats_summary accessed via onion host");
    }
    let stats = store.stats().await;
    Json(stats.into())
}

#[utoipa::path(
    get,
    path = "/api/auth/challenge",
    responses((status = 200, description = "Auth challenge", body = AuthChallengeResponse))
)]
#[get("/api/auth/challenge")]
async fn auth_challenge_api() -> Json<AuthChallengeResponse> {
    let challenge = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect::<String>();
    Json(AuthChallengeResponse { challenge })
}

#[utoipa::path(
    post,
    path = "/api/auth/login",
    request_body = AuthLoginRequest,
    responses(
        (status = 200, description = "Auth login response", body = AuthLoginResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    )
)]
#[post("/api/auth/login", data = "<body>")]
async fn auth_login_api(
    sessions: &State<SharedSessionStore>,
    body: Json<AuthLoginRequest>,
) -> Result<Json<AuthLoginResponse>, (Status, String)> {
    let body = body.into_inner();

    // Decode pubkey and signature
    let pubkey_bytes: [u8; 32] = BASE64_STANDARD
        .decode(&body.pubkey)
        .map_err(|_| (Status::BadRequest, "Invalid pubkey encoding".to_string()))?
        .try_into()
        .map_err(|_| (Status::BadRequest, "Invalid pubkey length".to_string()))?;
    let pubkey = VerifyingKey::from_bytes(&pubkey_bytes)
        .map_err(|_| (Status::BadRequest, "Invalid pubkey".to_string()))?;

    let signature_bytes: [u8; 64] = BASE64_STANDARD
        .decode(&body.signature)
        .map_err(|_| (Status::BadRequest, "Invalid signature encoding".to_string()))?
        .try_into()
        .map_err(|_| (Status::BadRequest, "Invalid signature length".to_string()))?;
    let signature = Signature::from_bytes(&signature_bytes);

    // Verify signature
    pubkey
        .verify(body.challenge.as_bytes(), &signature)
        .map_err(|_| {
            (
                Status::Unauthorized,
                "Signature verification failed".to_string(),
            )
        })?;

    // Compute pubkey hash
    let mut hasher = Sha256::new();
    hasher.update(pubkey_bytes);
    let pubkey_hash = format!("{:x}", hasher.finalize());

    // Generate and persist the session token (24 h expiry, in-memory store).
    // The token authorises the user-scoped endpoints (`/api/user/*`,
    // `/api/workspaces/*`) via the `RequireUserSession` request guard.
    let token = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect::<String>();
    sessions.insert(&token, &pubkey_hash);

    Ok(Json(AuthLoginResponse { token, pubkey_hash }))
}

#[utoipa::path(
    post,
    path = "/api/auth/logout",
    responses((status = 200, description = "Auth logout response", body = AuthLogoutResponse))
)]
#[post("/api/auth/logout")]
async fn auth_logout_api(
    sessions: &State<SharedSessionStore>,
    token: BearerToken,
) -> Json<AuthLogoutResponse> {
    // Invalidate the server-side session if a bearer token was supplied.
    // Always reports success so logout is idempotent.
    if let Some(token) = token.0 {
        sessions.remove(&token);
    }
    Json(AuthLogoutResponse { success: true })
}

/// Reject a `pubkey_hash` query parameter that does not match the
/// authenticated session. The parameter is kept for backward compatibility,
/// but the session is the single source of truth: callers can no longer
/// enumerate pastes for arbitrary hashes.
fn check_pubkey_hash_param(
    session: &RequireUserSession,
    requested: Option<&str>,
) -> Result<(), (Status, Json<ApiError>)> {
    match requested {
        Some(hash) if hash != session.pubkey_hash => Err((
            Status::Forbidden,
            Json(ApiError::new(
                "forbidden",
                "pubkey_hash does not match the authenticated session",
            )),
        )),
        _ => Ok(()),
    }
}

#[utoipa::path(
    get,
    path = "/api/user/paste-count",
    params(("pubkey_hash" = Option<String>, Query, description = "Optional; must match the session's pubkey hash")),
    responses(
        (status = 200, description = "User paste count response", body = UserPasteCountResponse),
        (status = 401, description = "Missing or invalid session token"),
        (status = 403, description = "pubkey_hash does not match the session", body = ApiError),
    )
)]
#[get("/api/user/paste-count?<pubkey_hash>")]
async fn user_paste_count_api(
    store: &State<SharedPasteStore>,
    session: RequireUserSession,
    pubkey_hash: Option<String>,
    onion: OnionAccess,
) -> Result<Json<UserPasteCountResponse>, (Status, Json<ApiError>)> {
    if onion.suppress_logs() {
        rocket::info!("user paste count accessed via onion host");
    }
    check_pubkey_hash_param(&session, pubkey_hash.as_deref())?;

    // Count pastes owned by the authenticated user only.
    let all_pastes = store.get_all_paste_ids().await;
    let mut count = 0;

    for id in all_pastes {
        if let Ok(paste) = store.get_paste(&id).await {
            if paste.metadata.owner_pubkey_hash.as_deref() == Some(session.pubkey_hash.as_str()) {
                count += 1;
            }
        }
    }

    Ok(Json(UserPasteCountResponse { paste_count: count }))
}

#[utoipa::path(
    get,
    path = "/api/user/pastes",
    params(("pubkey_hash" = Option<String>, Query, description = "Optional; must match the session's pubkey hash")),
    responses(
        (status = 200, description = "User paste list response", body = UserPasteListResponse),
        (status = 401, description = "Missing or invalid session token"),
        (status = 403, description = "pubkey_hash does not match the session", body = ApiError),
    )
)]
#[get("/api/user/pastes?<pubkey_hash>")]
async fn user_paste_list_api(
    store: &State<SharedPasteStore>,
    session: RequireUserSession,
    pubkey_hash: Option<String>,
    onion: OnionAccess,
) -> Result<Json<UserPasteListResponse>, (Status, Json<ApiError>)> {
    if onion.suppress_logs() {
        rocket::info!("user paste list accessed via onion host");
    }
    check_pubkey_hash_param(&session, pubkey_hash.as_deref())?;

    // List pastes owned by the authenticated user only.
    let all_pastes = store.get_all_paste_ids().await;
    let mut user_pastes = Vec::new();

    for id in all_pastes {
        if let Ok(paste) = store.get_paste(&id).await {
            if paste.metadata.owner_pubkey_hash.as_deref() == Some(session.pubkey_hash.as_str()) {
                let retention_minutes = paste.expires_at.map(|exp| {
                    let now = current_timestamp();
                    if exp > now {
                        (exp - now) / 60
                    } else {
                        0
                    }
                });

                user_pastes.push(UserPasteListItem {
                    id: id.clone(),
                    url: format!("/{}", id),
                    created_at: paste.created_at,
                    expires_at: paste.expires_at,
                    retention_minutes,
                    burn_after_reading: paste.burn_after_reading,
                    format: format!("{:?}", paste.format).to_lowercase(),
                    access_count: paste.metadata.access_count,
                    workspace: paste.metadata.workspace.clone(),
                });
            }
        }
    }

    // Sort by created_at descending (newest first)
    user_pastes.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(Json(UserPasteListResponse {
        pastes: user_pastes,
    }))
}

#[utoipa::path(
    get,
    path = "/api/workspaces/{name}/pastes",
    params(("name" = String, Path, description = "Workspace identifier")),
    responses(
        (status = 200, description = "Workspace paste list", body = WorkspacePasteListResponse),
        (status = 401, description = "Missing or invalid session token"),
    )
)]
#[get("/api/workspaces/<name>/pastes")]
async fn workspace_pastes_api(
    store: &State<SharedPasteStore>,
    session: RequireUserSession,
    name: String,
) -> Json<WorkspacePasteListResponse> {
    // Only the caller's own pastes within the workspace are listed.
    let all_pastes = store.get_all_paste_ids().await;
    let mut pastes = Vec::new();

    for id in all_pastes {
        if let Ok(paste) = store.get_paste(&id).await {
            if paste.metadata.workspace.as_deref() == Some(name.as_str())
                && paste.metadata.owner_pubkey_hash.as_deref() == Some(session.pubkey_hash.as_str())
            {
                pastes.push(WorkspacePasteItem {
                    id: id.clone(),
                    url: format!("/{}", id),
                    workspace: paste.metadata.workspace.clone(),
                    created_at: paste.created_at,
                });
            }
        }
    }

    pastes.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Json(WorkspacePasteListResponse { pastes })
}

#[utoipa::path(
    post,
    path = "/api/pastes/{id}/anchor",
    request_body = AnchorRequest,
    params(("id" = String, description = "Paste identifier")),
    responses(
        (status = 200, description = "Paste anchored", body = AnchorResponse),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Paste not found"),
        (status = 410, description = "Paste expired"),
    )
)]
#[post("/api/pastes/<id>/anchor", data = "<body>")]
async fn anchor_api(
    store: &State<SharedPasteStore>,
    relayer: &State<SharedAnchorRelayer>,
    id: String,
    body: Option<Json<AnchorRequest>>,
    onion: OnionAccess,
) -> Result<Json<AnchorResponse>, (Status, String)> {
    let request = body.map(|json| json.into_inner()).unwrap_or_default();

    let paste = match store.get_paste(&id).await {
        Ok(paste) => paste,
        Err(PasteError::NotFound(_)) => return Err((Status::NotFound, "Paste not found".into())),
        Err(PasteError::Expired(_)) => return Err((Status::Gone, "Paste expired".into())),
    };

    if paste.metadata.tor_access_only && !onion.is_onion() {
        return Err((
            Status::Forbidden,
            "This paste can only be accessed via the Tor hidden service".into(),
        ));
    }

    let manifest = AnchorManifest::from_paste(id.clone(), &paste);
    let hash = manifest_hash(&manifest).map_err(|error| {
        (
            Status::InternalServerError,
            format!("Failed to hash manifest: {error}"),
        )
    })?;

    let retention_class = request
        .retention_class
        .or_else(|| infer_retention_class(&manifest));
    let attestation_ref = request
        .attestation_ref
        .or_else(|| infer_attestation_ref(&manifest.metadata));

    let payload = AnchorPayload::new(
        manifest.clone(),
        hash.clone(),
        retention_class,
        attestation_ref.clone(),
    );

    let relayer = relayer.inner().clone();
    let receipt = relayer
        .submit(payload)
        .await
        .map_err(|error| (Status::BadGateway, format!("Relayer error: {error}")))?;

    let response = AnchorResponse {
        paste_id: id,
        hash,
        retention_class,
        attestation_ref,
        manifest,
        receipt,
    };

    Ok(Json(response))
}

/// Convert a status code to a machine-readable error code string.
fn status_to_code(status: Status) -> &'static str {
    match status.code {
        400 => "bad_request",
        401 => "unauthorized",
        403 => "forbidden",
        404 => "not_found",
        409 => "conflict",
        410 => "gone",
        413 => "payload_too_large",
        423 => "locked",
        429 => "too_many_requests",
        500 => "internal_error",
        502 => "bad_gateway",
        _ => "error",
    }
}

/// Map a `(Status, String)` pair from an internal helper into the standardised
/// `(Status, Json<ApiError>)` responder used by JSON API handlers.
fn to_api_err(status: Status, message: String) -> (Status, Json<ApiError>) {
    (status, Json(ApiError::new(status_to_code(status), message)))
}

/// Infallible guard extracting the optional `X-Paste-Key` request header.
///
/// Passing decryption keys via header keeps them out of server/proxy access
/// logs and `Referer` headers, unlike the legacy `?key=` query parameter.
pub struct PasteKeyHeader(pub Option<String>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for PasteKeyHeader {
    type Error = std::convert::Infallible;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        Outcome::Success(PasteKeyHeader(
            req.headers().get_one("X-Paste-Key").map(str::to_owned),
        ))
    }
}

/// Fetch a paste as JSON.
///
/// The decryption key may be supplied either via the `X-Paste-Key` request
/// header (preferred — keys in headers do not end up in access logs or
/// referrers) or via the legacy `?key=` query parameter. When both are
/// present, **the header takes precedence**; `?key=` is kept for backward
/// compatibility with the frontend and CLI.
#[utoipa::path(
    get,
    path = "/api/pastes/{id}",
    params(
        ("id" = String, Path, description = "Paste identifier"),
        ("X-Paste-Key" = Option<String>, Header, description = "Decryption key (takes precedence over ?key=)"),
    ),
    responses(
        (status = 200, description = "Paste content", body = PasteViewResponse),
        (status = 401, description = "Key required", body = ApiError),
        (status = 403, description = "Invalid key", body = ApiError),
        (status = 404, description = "Paste not found", body = ApiError),
    )
)]
#[get("/api/pastes/<id>?<query..>", rank = 1)]
async fn show_api(
    store: &State<SharedPasteStore>,
    id: String,
    query: PasteViewQuery,
    key_header: PasteKeyHeader,
    _rate: ReadRateLimit,
) -> Result<Json<PasteViewResponse>, (Status, Json<ApiError>)> {
    rocket::info!("show_api called with id: {}", id);

    // Header key wins over the query-string key (see handler docs above).
    let key = key_header.0.or_else(|| query.key.clone());

    let paste = match store.get_paste(&id).await {
        Ok(paste) => paste,
        Err(e) => {
            rocket::error!("Paste not found for id: {}, error: {:?}", id, e);
            return Err((
                Status::NotFound,
                Json(ApiError::new(
                    "paste_not_found",
                    format!("Paste '{}' not found", id),
                )),
            ));
        }
    };

    rocket::info!("Paste found for id: {}", id);

    let text = match decrypt_content(&paste.content, key.as_deref()) {
        Ok(text) => {
            rocket::info!(
                "Decryption successful for id: {}, content length: {}",
                id,
                text.len()
            );
            text
        }
        Err(DecryptError::MissingKey) => {
            rocket::info!("Missing key for encrypted paste: {}", id);
            return Err((
                Status::Unauthorized,
                Json(ApiError::new(
                    "key_required",
                    "This paste requires an encryption key",
                )),
            ));
        }
        Err(DecryptError::InvalidKey) => {
            rocket::error!("Invalid key for paste: {}", id);
            return Err((
                Status::Forbidden,
                Json(ApiError::new(
                    "invalid_key",
                    "The provided encryption key is incorrect",
                )),
            ));
        }
    };

    let encryption = match &paste.content {
        StoredContent::Plain { .. } => PasteEncryptionInfo {
            algorithm: EncryptionAlgorithm::None,
            requires_key: false,
        },
        StoredContent::Encrypted { algorithm, .. } | StoredContent::Stego { algorithm, .. } => {
            PasteEncryptionInfo {
                algorithm: *algorithm,
                requires_key: true,
            }
        }
    };

    let stego = match &paste.content {
        StoredContent::Stego {
            carrier_mime,
            carrier_image,
            payload_digest,
            ..
        } => Some(PasteStegoInfo {
            carrier_mime: carrier_mime.clone(),
            carrier_image: carrier_image.clone(),
            payload_digest: payload_digest.clone(),
        }),
        _ => None,
    };

    let time_lock = match (paste.not_before, paste.not_after) {
        (None, None) => None,
        (not_before, not_after) => Some(PasteTimeLockInfo {
            not_before,
            not_after,
        }),
    };

    let attestation = paste.metadata.attestation.as_ref().map(|req| match req {
        AttestationRequirement::Totp { issuer, .. } => PasteAttestationInfo {
            kind: "totp".to_string(),
            issuer: issuer.clone(),
        },
        AttestationRequirement::SharedSecret { .. } => PasteAttestationInfo {
            kind: "shared_secret".to_string(),
            issuer: None,
        },
    });

    let persistence = paste.metadata.persistence.as_ref().map(|loc| match loc {
        PersistenceLocator::Memory => PastePersistenceInfo {
            kind: "memory".to_string(),
            detail: None,
        },
        PersistenceLocator::Vault { key_path } => PastePersistenceInfo {
            kind: "vault".to_string(),
            detail: Some(key_path.clone()),
        },
        PersistenceLocator::S3 { bucket, .. } => PastePersistenceInfo {
            kind: "s3".to_string(),
            detail: Some(bucket.clone()),
        },
    });

    let webhook = paste.metadata.webhook.as_ref().map(|w| PasteWebhookInfo {
        provider: w.provider.clone(),
    });

    Ok(Json(PasteViewResponse {
        id,
        format: paste.format,
        content: text,
        created_at: paste.created_at,
        expires_at: paste.expires_at,
        burn_after_reading: paste.burn_after_reading,
        // `paste` is owned here; move the bundle instead of cloning it.
        bundle: paste.bundle,
        encryption,
        tor_access_only: paste.metadata.tor_access_only,
        access_count: paste.metadata.access_count,
        is_live: paste.is_live,
        time_lock,
        attestation,
        persistence,
        webhook,
        stego,
        workspace: paste.metadata.workspace,
    }))
}

#[utoipa::path(
    post,
    path = "/",
    request_body = CreatePasteRequest,
    responses(
        (status = 200, description = "Paste created", body = String),
        (status = 400, description = "Invalid paste request"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error"),
    )
)]
#[post("/", data = "<body>")]
async fn create(
    store: &State<SharedPasteStore>,
    body: Json<CreatePasteRequest>,
    onion: OnionAccess,
    _rate: CreateRateLimit,
) -> Result<String, (Status, String)> {
    let body = body.into_inner();
    let created = create_paste_internal(store.inner(), body, &onion).await?;
    Ok(created.path)
}

#[utoipa::path(
    post,
    path = "/api/pastes",
    request_body = CreatePasteRequest,
    responses(
        (status = 200, description = "Paste created", body = CreatePasteResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 401, description = "Authentication required", body = ApiError),
        (status = 403, description = "Forbidden", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError),
    )
)]
#[post("/api/pastes", data = "<body>")]
async fn create_api(
    store: &State<SharedPasteStore>,
    body: Result<Json<CreatePasteRequest>, rocket::serde::json::Error<'_>>,
    onion: OnionAccess,
    _rate: CreateRateLimit,
) -> Result<Json<CreatePasteResponse>, (Status, Json<ApiError>)> {
    let body = match body {
        Ok(json) => {
            rocket::info!("Successfully deserialized JSON request");
            json
        }
        Err(e) => {
            rocket::error!("JSON deserialization failed: {:?}", e);
            return Err((
                Status::BadRequest,
                Json(ApiError::new(
                    "invalid_request",
                    format!("Invalid JSON: {e}"),
                )),
            ));
        }
    };

    rocket::info!("Received create paste request");

    let body = body.into_inner();
    rocket::info!(
        "Processing paste creation: content length={}, format={:?}, encryption={:?}",
        body.content.len(),
        body.format,
        body.encryption
            .as_ref()
            .map(|e| format!("{:?}", e.algorithm))
    );

    let created = create_paste_internal(store.inner(), body, &onion)
        .await
        .map_err(|(s, msg)| to_api_err(s, msg))?;
    Ok(Json(created))
}

#[utoipa::path(
    get,
    path = "/{id}",
    params(("id" = String, description = "Paste identifier")),
    responses(
        (status = 200, description = "Paste rendered as HTML", content_type = "text/html"),
        (status = 401, description = "Key required"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Paste not found"),
    )
)]
#[get("/<id>?<query..>")]
async fn show(
    store: &State<SharedPasteStore>,
    http: &State<WebhookClient>,
    id: String,
    query: PasteViewQuery,
    onion: OnionAccess,
    _rate: ReadRateLimit,
) -> Result<content::RawHtml<String>, Status> {
    match store.get_paste(&id).await {
        Ok(paste) => {
            if paste.metadata.tor_access_only && !onion.is_onion() {
                return Err(Status::Forbidden);
            }

            let now = current_timestamp();
            if let Some(lock_state) = evaluate_time_lock(&paste.metadata, now) {
                return Ok(content::RawHtml(render_time_locked(lock_state)));
            }

            if let Some(requirement) = paste.metadata.attestation.as_ref() {
                match attestation::verify_attestation(requirement, &query, now) {
                    AttestationVerdict::Granted => {}
                    AttestationVerdict::Prompt { invalid } => {
                        let needs_key_field =
                            matches!(paste.content, StoredContent::Encrypted { .. })
                                && query.key.is_none();
                        return Ok(content::RawHtml(render_attestation_prompt(
                            &id,
                            needs_key_field,
                            query.key.as_deref(),
                            requirement,
                            invalid,
                        )));
                    }
                }
            }

            match decrypt_content(&paste.content, query.key.as_deref()) {
                Ok(text) => {
                    let bundle_html = if let Some(bundle) = paste.metadata.bundle.clone() {
                        build_bundle_overview(store.inner().clone(), &bundle, &query).await
                    } else {
                        None
                    };

                    let webhook_config = paste.metadata.webhook.clone();
                    let mut events_to_fire = Vec::new();

                    if paste.burn_after_reading {
                        if let Some(config) = webhook_config.clone() {
                            events_to_fire.push((config.clone(), WebhookEvent::Viewed));
                        }
                    }

                    if paste.burn_after_reading {
                        let deleted = store.delete_paste(&id).await;
                        if deleted {
                            if let Some(config) = webhook_config.clone() {
                                events_to_fire.push((config, WebhookEvent::Consumed));
                            }
                        }
                    }

                    for (config, event) in events_to_fire {
                        trigger_webhook(
                            http.inner().0.clone(),
                            config,
                            event,
                            &id,
                            paste.metadata.bundle_label.clone(),
                        );
                    }

                    let view = StoredPasteView {
                        content: &paste.content,
                        format: paste.format,
                        created_at: paste.created_at,
                        expires_at: paste.expires_at,
                        burn_after_reading: paste.burn_after_reading,
                        metadata: &paste.metadata,
                    };

                    Ok(content::RawHtml(render_paste_view(
                        &id,
                        &view,
                        &text,
                        bundle_html,
                    )))
                }
                Err(DecryptError::MissingKey) => Ok(content::RawHtml(render_key_prompt(&id))),
                Err(DecryptError::InvalidKey) => Ok(content::RawHtml(render_invalid_key(&id))),
            }
        }
        Err(PasteError::NotFound(_)) => Err(Status::NotFound),
        Err(PasteError::Expired(_)) => Ok(content::RawHtml(render_expired(&id))),
    }
}

#[get("/raw/<id>?<query..>")]
async fn show_raw(
    store: &State<SharedPasteStore>,
    http: &State<WebhookClient>,
    id: String,
    query: PasteViewQuery,
    onion: OnionAccess,
    _rate: ReadRateLimit,
) -> Result<content::RawText<String>, Status> {
    match store.get_paste(&id).await {
        Ok(paste) => {
            if paste.metadata.tor_access_only && !onion.is_onion() {
                return Err(Status::Forbidden);
            }

            let now = current_timestamp();
            match evaluate_time_lock(&paste.metadata, now) {
                Some(TimeLockState::TooEarly(_)) => return Err(Status::Locked),
                Some(TimeLockState::TooLate(_)) => return Err(Status::Gone),
                None => {}
            }

            if let Some(requirement) = paste.metadata.attestation.as_ref() {
                match attestation::verify_attestation(requirement, &query, now) {
                    AttestationVerdict::Granted => {}
                    AttestationVerdict::Prompt { invalid: false } => {
                        return Err(Status::Unauthorized);
                    }
                    AttestationVerdict::Prompt { invalid: true } => {
                        return Err(Status::Forbidden);
                    }
                }
            }

            match decrypt_content(&paste.content, query.key.as_deref()) {
                Ok(text) => {
                    if paste.burn_after_reading {
                        let webhook_config = paste.metadata.webhook.clone();
                        if let Some(config) = webhook_config.clone() {
                            trigger_webhook(
                                http.inner().0.clone(),
                                config,
                                WebhookEvent::Viewed,
                                &id,
                                paste.metadata.bundle_label.clone(),
                            );
                        }
                        let deleted = store.delete_paste(&id).await;
                        if deleted {
                            if let Some(config) = webhook_config {
                                trigger_webhook(
                                    http.inner().0.clone(),
                                    config,
                                    WebhookEvent::Consumed,
                                    &id,
                                    paste.metadata.bundle_label.clone(),
                                );
                            }
                        }
                    }

                    Ok(content::RawText(text))
                }
                Err(DecryptError::MissingKey) => Err(Status::Unauthorized),
                Err(DecryptError::InvalidKey) => Err(Status::Forbidden),
            }
        }
        Err(PasteError::NotFound(_)) => Err(Status::NotFound),
        Err(PasteError::Expired(_)) => Err(Status::Gone),
    }
}

fn apply_time_lock(
    lock: &TimeLockRequest,
    metadata: &mut PasteMetadata,
) -> Result<(), (Status, String)> {
    if let Some(ref not_before) = lock.not_before {
        let ts = parse_timestamp(not_before)
            .map_err(|e| (Status::BadRequest, format!("invalid not_before: {e}")))?;
        metadata.not_before = Some(ts);
    }
    if let Some(ref not_after) = lock.not_after {
        let ts = parse_timestamp(not_after)
            .map_err(|e| (Status::BadRequest, format!("invalid not_after: {e}")))?;
        metadata.not_after = Some(ts);
    }
    if let (Some(start), Some(end)) = (metadata.not_before, metadata.not_after) {
        if end <= start {
            return Err((
                Status::BadRequest,
                "time_lock not_after must be greater than not_before".to_string(),
            ));
        }
    }
    Ok(())
}

fn persistence_locator_from_request(
    request: &PersistenceRequest,
) -> Result<PersistenceLocator, (Status, String)> {
    Ok(match request {
        PersistenceRequest::Memory => PersistenceLocator::Memory,
        PersistenceRequest::Vault { key_path } => {
            if key_path.trim().is_empty() {
                return Err((Status::BadRequest, "Vault key_path cannot be empty".into()));
            }
            PersistenceLocator::Vault {
                key_path: key_path.clone(),
            }
        }
        PersistenceRequest::S3 { bucket, prefix } => {
            if bucket.trim().is_empty() {
                return Err((Status::BadRequest, "S3 bucket cannot be empty".into()));
            }
            PersistenceLocator::S3 {
                bucket: bucket.clone(),
                prefix: prefix.clone(),
            }
        }
    })
}

fn webhook_config_from_request(
    request: &WebhookRequest,
) -> Result<WebhookConfig, (Status, String)> {
    if request.url.trim().is_empty() {
        return Err((Status::BadRequest, "Webhook url cannot be empty".into()));
    }
    // SSRF guard: only public http(s) endpoints may be registered as webhooks.
    validate_webhook_url(&request.url).map_err(|e| (Status::BadRequest, e))?;
    const MAX_TEMPLATE_LEN: usize = 4096;
    if let Some(ref t) = request.view_template {
        if t.len() > MAX_TEMPLATE_LEN {
            return Err((
                Status::BadRequest,
                "view_template must not exceed 4096 characters".into(),
            ));
        }
    }
    if let Some(ref t) = request.burn_template {
        if t.len() > MAX_TEMPLATE_LEN {
            return Err((
                Status::BadRequest,
                "burn_template must not exceed 4096 characters".into(),
            ));
        }
    }
    Ok(WebhookConfig {
        url: request.url.clone(),
        provider: request.provider.clone(),
        view_template: request.view_template.clone(),
        burn_template: request.burn_template.clone(),
    })
}

/// Resolve stored content from plaintext, encrypting when requested.
///
/// Takes ownership of `text` so the plain-text path stores the buffer without
/// copying it (paste content can be up to 10 MiB).
async fn resolve_content(
    text: String,
    encryption: Option<&super::models::EncryptionRequest>,
) -> Result<StoredContent, (Status, String)> {
    match encryption {
        Some(enc) if enc.algorithm != EncryptionAlgorithm::None => {
            encrypt_content(&text, &enc.key, enc.algorithm)
                .await
                .map_err(|e| (Status::BadRequest, e))
        }
        _ => Ok(StoredContent::Plain { text }),
    }
}

/// Read a `u64` minutes value from an env var (unset/unparsable → `None`).
fn env_minutes(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse::<u64>().ok())
}

async fn create_paste_internal(
    store: &SharedPasteStore,
    mut body: CreatePasteRequest,
    _onion: &OnionAccess,
) -> Result<CreatePasteResponse, (Status, String)> {
    // Validate content
    if body.content.trim().is_empty() {
        return Err((Status::BadRequest, "Content cannot be empty".into()));
    }
    let max_paste_size = std::env::var("COPYPASTE_MAX_PASTE_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(10_485_760); // 10 MB
    if body.content.len() > max_paste_size {
        return Err((
            Status::PayloadTooLarge,
            "Content exceeds maximum paste size".into(),
        ));
    }

    // Validate workspace
    if let Some(ref ws) = body.workspace {
        if ws.len() > 128 {
            return Err((
                Status::BadRequest,
                "Workspace identifier must not exceed 128 bytes".into(),
            ));
        }
    }

    // Resolve content (handle encryption). Move the content buffer out of the
    // request so the plain-text path avoids cloning up to 10 MiB.
    let content_text = std::mem::take(&mut body.content);
    let content = resolve_content(content_text, body.encryption.as_ref()).await?;

    // Build metadata
    let mut metadata = PasteMetadata::default();

    // Handle attestation
    if let Some(attestation_req) = &body.attestation {
        let requirement = attestation::requirement_from_request(attestation_req)
            .map_err(|e| (Status::BadRequest, e))?;
        metadata.attestation = Some(requirement);
    }

    // Handle time lock
    if let Some(ref time_lock) = body.time_lock {
        apply_time_lock(time_lock, &mut metadata)?;
    }

    // Handle persistence
    if let Some(ref persistence_req) = body.persistence {
        metadata.persistence = Some(persistence_locator_from_request(persistence_req)?);
    }

    // Handle webhook
    if let Some(ref webhook_req) = body.webhook {
        metadata.webhook = Some(webhook_config_from_request(webhook_req)?);
    }

    // Handle stego — embed encrypted ciphertext into carrier image
    let content = if let Some(ref stego_req) = body.stego {
        let (algorithm, ciphertext_b64, nonce, salt) = match content {
            StoredContent::Encrypted {
                algorithm,
                ciphertext,
                nonce,
                salt,
            } => (algorithm, ciphertext, nonce, salt),
            _ => {
                return Err((
                    Status::BadRequest,
                    "Steganography requires encryption to be enabled".into(),
                ))
            }
        };
        let ciphertext_bytes = BASE64_STANDARD.decode(&ciphertext_b64).map_err(|_| {
            (
                Status::InternalServerError,
                "Failed to decode ciphertext".into(),
            )
        })?;
        let carrier_source = match stego_req {
            StegoRequest::Builtin { carrier } => StegoCarrierSource::BuiltIn(carrier.clone()),
            StegoRequest::Uploaded { data_uri } => {
                if data_uri.len() > 10_000_000 {
                    return Err((
                        Status::PayloadTooLarge,
                        "Carrier data URI must not exceed 10 MB".into(),
                    ));
                }
                let (mime, data) = parse_data_uri(data_uri)
                    .map_err(|e| (Status::BadRequest, format!("Invalid data URI: {}", e)))?;
                if !matches!(mime.as_str(), "image/png" | "image/bmp" | "image/jpeg") {
                    return Err((
                        Status::BadRequest,
                        "Carrier image must be PNG, BMP, or JPEG".into(),
                    ));
                }
                if data.len() > 1_048_576 {
                    return Err((
                        Status::PayloadTooLarge,
                        "Carrier image must not exceed 1 MB".into(),
                    ));
                }
                StegoCarrierSource::Uploaded { mime, data }
            }
        };
        let payload = ciphertext_bytes.clone();
        let result = tokio::task::spawn_blocking(move || embed_payload(carrier_source, &payload))
            .await
            .map_err(|_| {
                (
                    Status::InternalServerError,
                    "Steganography task failed".into(),
                )
            })?
            .map_err(|e| (Status::BadRequest, format!("Steganography failed: {e}")))?;
        let payload_digest = {
            let mut hasher = Sha256::new();
            hasher.update(&ciphertext_bytes);
            format!("{:x}", hasher.finalize())
        };
        StoredContent::Stego {
            algorithm,
            ciphertext: ciphertext_b64,
            nonce,
            salt,
            carrier_mime: "image/png".to_string(),
            carrier_image: BASE64_STANDARD.encode(&result.image_data),
            payload_digest,
        }
    } else {
        content
    };

    // Handle bundle
    if let Some(ref bundle_req) = body.bundle {
        if bundle_req.children.len() > 50 {
            return Err((
                Status::BadRequest,
                "Bundle exceeds maximum child count".into(),
            ));
        }
        // Enforce encryption for bundles
        if body.encryption.is_none() {
            return Err((
                Status::BadRequest,
                "Bundles require an encryption key".to_string(),
            ));
        }

        // Create bundle metadata
        metadata.bundle = Some(crate::BundleMetadata {
            children: bundle_req
                .children
                .iter()
                .map(|child| crate::BundlePointer {
                    id: "".to_string(), // Will be set when child pastes are created
                    label: child.label.clone(),
                })
                .collect(),
        });
    }

    // Set tor access only
    metadata.tor_access_only = body.tor_access_only;
    metadata.owner_pubkey_hash = body.owner_pubkey_hash;
    metadata.workspace = body.workspace;

    // Calculate expiration, honouring the bridged retention config knobs:
    // apply the configured default when the request omits retention, and
    // reject retentions above the configured maximum.
    let retention_minutes = body
        .retention_minutes
        .or_else(|| env_minutes("COPYPASTE_RETENTION_DEFAULT_MINUTES"));
    if let (Some(requested), Some(max)) = (
        retention_minutes,
        env_minutes("COPYPASTE_RETENTION_MAX_MINUTES"),
    ) {
        if requested > max {
            return Err((
                Status::BadRequest,
                format!("retention_minutes {requested} exceeds the configured maximum of {max}"),
            ));
        }
    }
    let expires_at = retention_minutes.map(|minutes| current_timestamp() + (minutes as i64 * 60));

    // Handle live paste ownership token
    let (is_live, owner_token_hash, plaintext_token) = if body.live {
        let token: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        (true, Some(hash), Some(token))
    } else {
        (false, None, None)
    };

    // Create the paste
    let paste = StoredPaste {
        content,
        format: body.format.unwrap_or(PasteFormat::PlainText),
        created_at: current_timestamp(),
        expires_at,
        burn_after_reading: body.burn_after_reading,
        bundle: metadata.bundle.clone(),
        bundle_parent: metadata.bundle_parent.clone(),
        bundle_label: metadata.bundle_label.clone(),
        not_before: metadata.not_before,
        not_after: metadata.not_after,
        persistence: metadata.persistence.clone(),
        webhook: metadata.webhook.clone(),
        metadata,
        is_live,
        owner_token_hash,
    };

    // Store the paste
    let id = store.create_paste(paste).await;
    let path = format!("/{}", id);

    Ok(CreatePasteResponse {
        id: id.clone(),
        path: path.clone(),
        shareable_url: path,
        token: plaintext_token,
        is_live,
    })
}

/// Verify the live-paste ownership token supplied as `Authorization: Bearer`.
///
/// The stored hash is SHA-256(token); comparison is constant-time.
fn verify_owner_token(paste: &StoredPaste, token: Option<&str>) -> Result<(), (Status, String)> {
    let expected = paste.owner_token_hash.as_deref().ok_or((
        Status::Conflict,
        "This paste has no ownership token and cannot be modified".to_string(),
    ))?;
    let token = token.ok_or((
        Status::Unauthorized,
        "Ownership token required (Authorization: Bearer <token>)".to_string(),
    ))?;
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let actual = format!("{:x}", hasher.finalize());
    if bool::from(actual.as_bytes().ct_eq(expected.as_bytes())) {
        Ok(())
    } else {
        Err((Status::Forbidden, "Invalid ownership token".to_string()))
    }
}

/// Fetch a paste for a live-paste mutation, mapping store errors to API errors.
async fn get_paste_for_mutation(
    store: &SharedPasteStore,
    id: &str,
) -> Result<StoredPaste, (Status, String)> {
    match store.get_paste(id).await {
        Ok(paste) => Ok(paste),
        Err(PasteError::NotFound(_)) => Err((Status::NotFound, format!("Paste '{id}' not found"))),
        Err(PasteError::Expired(_)) => Err((Status::Gone, format!("Paste '{id}' expired"))),
    }
}

/// Update the content of a live paste.
///
/// Requires the ownership token issued at creation (`live: true`) via
/// `Authorization: Bearer <token>`. Rejected once the paste is finalized.
#[utoipa::path(
    put,
    path = "/api/pastes/{id}",
    request_body = UpdatePasteRequest,
    params(("id" = String, Path, description = "Paste identifier")),
    responses(
        (status = 200, description = "Paste updated", body = UpdatePasteResponse),
        (status = 401, description = "Ownership token required", body = ApiError),
        (status = 403, description = "Invalid ownership token", body = ApiError),
        (status = 404, description = "Paste not found", body = ApiError),
        (status = 409, description = "Paste is not live", body = ApiError),
        (status = 410, description = "Paste expired", body = ApiError),
    )
)]
#[put("/api/pastes/<id>", data = "<body>")]
async fn update_api(
    store: &State<SharedPasteStore>,
    id: String,
    body: Json<UpdatePasteRequest>,
    token: BearerToken,
) -> Result<Json<UpdatePasteResponse>, (Status, Json<ApiError>)> {
    let body = body.into_inner();

    let paste = get_paste_for_mutation(store.inner(), &id)
        .await
        .map_err(|(s, m)| to_api_err(s, m))?;

    verify_owner_token(&paste, token.0.as_deref()).map_err(|(s, m)| to_api_err(s, m))?;

    if !paste.is_live {
        return Err(to_api_err(
            Status::Conflict,
            "Paste has been finalized and can no longer be updated".to_string(),
        ));
    }

    if body.content.trim().is_empty() {
        return Err(to_api_err(
            Status::BadRequest,
            "Content cannot be empty".to_string(),
        ));
    }
    let max_paste_size = std::env::var("COPYPASTE_MAX_PASTE_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(10_485_760);
    if body.content.len() > max_paste_size {
        return Err(to_api_err(
            Status::PayloadTooLarge,
            "Content exceeds maximum paste size".to_string(),
        ));
    }

    let content = resolve_content(body.content, body.encryption.as_ref())
        .await
        .map_err(|(s, m)| to_api_err(s, m))?;

    store
        .update_paste(&id, content)
        .await
        .map_err(|e| match e {
            PasteError::NotFound(_) => {
                to_api_err(Status::NotFound, format!("Paste '{id}' not found"))
            }
            PasteError::Expired(_) => to_api_err(Status::Gone, format!("Paste '{id}' expired")),
        })?;

    Ok(Json(UpdatePasteResponse { id, is_live: true }))
}

/// Finalize a live paste so it can no longer be updated.
///
/// Requires the ownership token via `Authorization: Bearer <token>`.
/// Idempotent: finalizing an already-finalized paste succeeds.
#[utoipa::path(
    patch,
    path = "/api/pastes/{id}/finalize",
    request_body = FinalizePasteRequest,
    params(("id" = String, Path, description = "Paste identifier")),
    responses(
        (status = 200, description = "Paste finalized", body = FinalizePasteResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 401, description = "Ownership token required", body = ApiError),
        (status = 403, description = "Invalid ownership token", body = ApiError),
        (status = 404, description = "Paste not found", body = ApiError),
        (status = 410, description = "Paste expired", body = ApiError),
    )
)]
#[patch("/api/pastes/<id>/finalize", data = "<body>")]
async fn finalize_api(
    store: &State<SharedPasteStore>,
    id: String,
    body: Option<Json<FinalizePasteRequest>>,
    token: BearerToken,
) -> Result<Json<FinalizePasteResponse>, (Status, Json<ApiError>)> {
    if let Some(ref body) = body {
        if body.live {
            return Err(to_api_err(
                Status::BadRequest,
                "'live' must be false — a finalized paste cannot be re-opened".to_string(),
            ));
        }
    }

    let paste = get_paste_for_mutation(store.inner(), &id)
        .await
        .map_err(|(s, m)| to_api_err(s, m))?;

    verify_owner_token(&paste, token.0.as_deref()).map_err(|(s, m)| to_api_err(s, m))?;

    if paste.is_live {
        store.finalize_paste(&id).await.map_err(|e| match e {
            PasteError::NotFound(_) => {
                to_api_err(Status::NotFound, format!("Paste '{id}' not found"))
            }
            PasteError::Expired(_) => to_api_err(Status::Gone, format!("Paste '{id}' expired")),
        })?;
    }

    Ok(Json(FinalizePasteResponse { id, is_live: false }))
}

#[post("/api/admin/keys", data = "<body>")]
async fn admin_create_key_api(
    key_store: &State<SharedApiKeyStore>,
    body: Json<CreateApiKeyRequest>,
    _auth: RequireAdminAuth,
) -> Result<Json<CreateApiKeyResponse>, (Status, Json<ApiError>)> {
    let body = body.into_inner();
    let store = key_store.inner().clone();
    let (key_info, plaintext_key) = tokio::task::spawn_blocking(move || {
        store.create_key(&body.name, body.scope, body.expires_at)
    })
    .await
    .map_err(|_| to_api_err(Status::InternalServerError, "Internal error".to_string()))?
    .map_err(|e| to_api_err(Status::InternalServerError, e))?;

    Ok(Json(CreateApiKeyResponse {
        id: key_info.id,
        name: key_info.name,
        scope: key_info.scope,
        key: plaintext_key,
        created_at: key_info.created_at,
    }))
}

#[get("/api/admin/keys")]
async fn admin_list_keys_api(
    key_store: &State<SharedApiKeyStore>,
    _auth: RequireAdminAuth,
) -> Result<Json<ListApiKeysResponse>, (Status, Json<ApiError>)> {
    let store = key_store.inner().clone();
    let keys = tokio::task::spawn_blocking(move || store.list_keys())
        .await
        .map_err(|_| to_api_err(Status::InternalServerError, "Internal error".to_string()))?
        .map_err(|e| to_api_err(Status::InternalServerError, e))?;

    let key_infos = keys
        .into_iter()
        .map(|k| ApiKeyInfo {
            id: k.id,
            name: k.name,
            scope: k.scope,
            created_at: k.created_at,
            last_used_at: k.last_used_at,
            expires_at: k.expires_at,
        })
        .collect();

    Ok(Json(ListApiKeysResponse { keys: key_infos }))
}

#[delete("/api/admin/keys/<id>")]
async fn admin_delete_key_api(
    key_store: &State<SharedApiKeyStore>,
    id: String,
    _auth: RequireAdminAuth,
) -> Result<Json<RevokeApiKeyResponse>, (Status, Json<ApiError>)> {
    let store = key_store.inner().clone();
    let revoked = tokio::task::spawn_blocking(move || store.revoke_key(&id))
        .await
        .map_err(|_| to_api_err(Status::InternalServerError, "Internal error".to_string()))?
        .map_err(|e| to_api_err(Status::InternalServerError, e))?;

    Ok(Json(RevokeApiKeyResponse { revoked }))
}

#[get("/")]
async fn index() -> content::RawHtml<String> {
    content::RawHtml(include_str!("../../static/index.html").to_string())
}

#[get("/about")]
async fn about() -> content::RawHtml<String> {
    content::RawHtml(include_str!("../../static/index.html").to_string())
}

#[get("/<_path..>", rank = 100)]
async fn spa_fallback(_path: PathBuf) -> content::RawHtml<String> {
    content::RawHtml(include_str!("../../static/index.html").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryPasteStore;
    use rocket::http::ContentType;
    use rocket::local::blocking::Client;
    use serde_json::json;
    use std::sync::Arc;

    /// Perform a full Ed25519 challenge login and return `(token, pubkey_hash)`.
    fn login(client: &Client) -> (String, String) {
        use ed25519_dalek::{Signer, SigningKey};

        let secret_bytes: [u8; 32] = [42u8; 32];
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();
        let challenge = "handler-test-challenge";
        let signature = signing_key.sign(challenge.as_bytes());

        let resp = client
            .post("/api/auth/login")
            .header(ContentType::JSON)
            .body(
                json!({
                    "pubkey": BASE64_STANDARD.encode(verifying_key.as_bytes()),
                    "signature": BASE64_STANDARD.encode(signature.to_bytes()),
                    "challenge": challenge
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(resp.status(), Status::Ok, "login should succeed");
        let parsed: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
        (
            parsed["token"].as_str().unwrap().to_string(),
            parsed["pubkeyHash"].as_str().unwrap().to_string(),
        )
    }

    fn bearer(token: &str) -> rocket::http::Header<'static> {
        rocket::http::Header::new("Authorization", format!("Bearer {token}"))
    }

    #[test]
    fn apply_time_lock_validates_order() {
        let mut metadata = PasteMetadata::default();
        let request = TimeLockRequest {
            not_before: Some("2024-01-01T00:00:00Z".into()),
            not_after: Some("2024-01-02T00:00:00Z".into()),
        };

        apply_time_lock(&request, &mut metadata).expect("valid window");
        assert!(metadata.not_before.unwrap() < metadata.not_after.unwrap());
    }

    #[test]
    fn apply_time_lock_rejects_inverted_window() {
        let mut metadata = PasteMetadata::default();
        let request = TimeLockRequest {
            not_before: Some("2024-01-02T00:00:00Z".into()),
            not_after: Some("2024-01-01T00:00:00Z".into()),
        };

        let err = apply_time_lock(&request, &mut metadata).expect_err("window invalid");
        assert_eq!(err.0, Status::BadRequest);
    }

    #[test]
    fn persistence_locator_validates_inputs() {
        let memory = persistence_locator_from_request(&PersistenceRequest::Memory).unwrap();
        matches!(memory, PersistenceLocator::Memory);

        let err = persistence_locator_from_request(&PersistenceRequest::Vault {
            key_path: "".into(),
        })
        .expect_err("empty key path");
        assert_eq!(err.0, Status::BadRequest);

        let loc = persistence_locator_from_request(&PersistenceRequest::S3 {
            bucket: "bucket".into(),
            prefix: Some("prefix".into()),
        })
        .unwrap();
        matches!(loc, PersistenceLocator::S3 { .. });
    }

    #[test]
    fn webhook_config_requires_url() {
        let err = webhook_config_from_request(&WebhookRequest {
            url: " ".into(),
            ..Default::default()
        })
        .expect_err("empty url should fail");
        assert_eq!(err.0, Status::BadRequest);

        let cfg = webhook_config_from_request(&WebhookRequest {
            url: "https://example.com".into(),
            ..Default::default()
        })
        .expect("valid webhook");
        assert_eq!(cfg.url, "https://example.com");
    }

    #[test]
    fn show_route_triggers_burn_after_reading_flow() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "payload",
            "format": "plain_text",
            "burn_after_reading": true,
            "webhook": {
                "url": "https://example.com/webhook"
            }
        });

        let response = client
            .post("/")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();

        assert_eq!(response.status(), Status::Ok);
        let id = response.into_string().unwrap();

        let view = client.get(&id).dispatch();
        assert_eq!(view.status(), Status::Ok);

        let second = client.get(&id).dispatch();
        assert_eq!(second.status(), Status::NotFound);
    }

    #[test]
    fn create_api_returns_json_and_persists_paste() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "hello world",
            "format": "plain_text"
        });

        let response = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();

        assert_eq!(response.status(), Status::Ok);
        let body = response.into_string().expect("json body");
        let parsed: CreatePasteResponse = serde_json::from_str(&body).expect("parse");
        assert!(parsed.path.starts_with('/'));
        assert_eq!(parsed.path, parsed.shareable_url);

        // Fetch the paste to ensure it was stored.
        let get_response = client.get(&parsed.path).dispatch();
        assert_eq!(get_response.status(), Status::Ok);
    }

    #[test]
    fn stats_summary_endpoint_returns_counts() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(Arc::clone(&store));
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "diagnostic entry",
            "format": "markdown",
            "encryption": {
                "algorithm": "aes256_gcm",
                "key": "secret-key"
            }
        });

        let create_response = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();

        assert_eq!(create_response.status(), Status::Ok);

        let response = client.get("/api/stats/summary").dispatch();
        assert_eq!(response.status(), Status::Ok);

        let body = response.into_string().expect("body");
        let stats: StatsSummaryResponse = serde_json::from_str(&body).expect("stats payload");

        assert!(stats.total_pastes >= 1);
        assert!(stats.active_pastes >= 1);
        assert!(!stats.formats.is_empty());
        assert!(!stats.encryption_usage.is_empty());
    }

    #[test]
    fn health_endpoint_returns_ok_status() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let response = client.get("/health").dispatch();
        assert_eq!(response.status(), Status::Ok);

        let body = response.into_string().expect("body");
        let health: HealthResponse = serde_json::from_str(&body).expect("parse health");

        assert_eq!(health.status, "ok");
        assert!(health.timestamp > 0);
    }

    #[test]
    fn detailed_health_endpoint_checks_services() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let response = client.get("/api/health").dispatch();
        assert_eq!(response.status(), Status::Ok);

        let body = response.into_string().expect("body");
        let health: DetailedHealthResponse =
            serde_json::from_str(&body).expect("parse detailed health");

        assert!(!health.status.is_empty());
        assert!(health.timestamp > 0);
        assert_eq!(health.services.backend.status, "ok");
        assert_eq!(health.services.storage.status, "ok");
        // crypto_verifier status depends on whether service is running
        assert!(!health.services.crypto_verifier.status.is_empty());
    }

    #[test]
    fn status_to_code_maps_known_codes() {
        assert_eq!(status_to_code(Status::BadRequest), "bad_request");
        assert_eq!(status_to_code(Status::Unauthorized), "unauthorized");
        assert_eq!(status_to_code(Status::Forbidden), "forbidden");
        assert_eq!(status_to_code(Status::NotFound), "not_found");
        assert_eq!(status_to_code(Status::Gone), "gone");
        assert_eq!(status_to_code(Status::Locked), "locked");
        assert_eq!(
            status_to_code(Status::InternalServerError),
            "internal_error"
        );
        assert_eq!(status_to_code(Status::BadGateway), "bad_gateway");
        assert_eq!(status_to_code(Status::ServiceUnavailable), "error");
    }

    #[test]
    fn to_api_err_constructs_error_envelope() {
        let (status, Json(err)) = to_api_err(Status::NotFound, "not found".into());
        assert_eq!(status, Status::NotFound);
        assert_eq!(err.code, "not_found");
        assert_eq!(err.message, "not found");
    }

    #[test]
    fn show_api_returns_not_found_for_missing_paste() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let response = client.get("/api/pastes/nonexistent-id").dispatch();
        assert_eq!(response.status(), Status::NotFound);
    }

    #[test]
    fn show_api_encrypted_paste_requires_key() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(Arc::clone(&store));
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "secret content",
            "format": "plain_text",
            "encryption": {
                "algorithm": "aes256_gcm",
                "key": "mypassword"
            }
        });

        let create = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(create.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create.into_string().unwrap()).unwrap();

        // No key → 401
        let no_key = client.get(format!("/api/pastes/{}", created.id)).dispatch();
        assert_eq!(no_key.status(), Status::Unauthorized);

        // Wrong key → 403
        let wrong_key = client
            .get(format!("/api/pastes/{}?key=wrongpassword", created.id))
            .dispatch();
        assert_eq!(wrong_key.status(), Status::Forbidden);

        // Correct key → 200 with rich response
        let ok = client
            .get(format!("/api/pastes/{}?key=mypassword", created.id))
            .dispatch();
        assert_eq!(ok.status(), Status::Ok);
        let view: PasteViewResponse = serde_json::from_str(&ok.into_string().unwrap()).unwrap();
        assert_eq!(view.content, "secret content");
        assert!(view.encryption.requires_key);
    }

    #[test]
    fn show_api_plain_paste_returns_full_response() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let create = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(json!({"content": "hello", "format": "plain_text"}).to_string())
            .dispatch();
        assert_eq!(create.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create.into_string().unwrap()).unwrap();

        let get = client.get(format!("/api/pastes/{}", created.id)).dispatch();
        assert_eq!(get.status(), Status::Ok);
        let view: PasteViewResponse = serde_json::from_str(&get.into_string().unwrap()).unwrap();
        assert_eq!(view.content, "hello");
        assert!(!view.encryption.requires_key);
        assert!(!view.burn_after_reading);
    }

    #[test]
    fn auth_challenge_returns_nonempty_string() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let resp = client.get("/api/auth/challenge").dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let body = resp.into_string().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(!parsed["challenge"].as_str().unwrap().is_empty());
    }

    #[test]
    fn auth_logout_returns_success() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let resp = client
            .post("/api/auth/logout")
            .header(ContentType::JSON)
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let body = resp.into_string().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["success"], true);
    }

    #[test]
    fn create_api_rejects_malformed_json() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body("{not valid json}")
            .dispatch();
        assert_eq!(resp.status(), Status::BadRequest);
    }

    #[test]
    fn user_paste_count_requires_session_and_returns_own_count() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        // Without a session token → 401.
        let resp = client
            .get("/api/user/paste-count?pubkey_hash=nonexistent")
            .dispatch();
        assert_eq!(resp.status(), Status::Unauthorized);

        // With a valid session → own count (zero pastes yet).
        let (token, _) = login(&client);
        let resp = client
            .get("/api/user/paste-count")
            .header(bearer(&token))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let parsed: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
        assert_eq!(parsed["pasteCount"], 0);
    }

    #[test]
    fn user_paste_list_requires_session_and_returns_own_pastes() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        // Without a session token → 401.
        let resp = client
            .get("/api/user/pastes?pubkey_hash=nonexistent")
            .dispatch();
        assert_eq!(resp.status(), Status::Unauthorized);

        // With a garbage token → 401.
        let resp = client
            .get("/api/user/pastes")
            .header(bearer("not-a-real-session"))
            .dispatch();
        assert_eq!(resp.status(), Status::Unauthorized);

        // With a valid session, only own pastes are listed.
        let (token, pubkey_hash) = login(&client);
        let create_resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "mine",
                    "format": "plain_text",
                    "owner_pubkey_hash": pubkey_hash
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(create_resp.status(), Status::Ok);

        let resp = client
            .get("/api/user/pastes")
            .header(bearer(&token))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let parsed: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
        assert_eq!(parsed["pastes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn logout_invalidates_session_token() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let (token, _) = login(&client);

        // Session works before logout.
        let resp = client
            .get("/api/user/paste-count")
            .header(bearer(&token))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);

        // Logout removes the server-side session.
        let resp = client
            .post("/api/auth/logout")
            .header(bearer(&token))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);

        // The token no longer authorises user endpoints.
        let resp = client
            .get("/api/user/paste-count")
            .header(bearer(&token))
            .dispatch();
        assert_eq!(resp.status(), Status::Unauthorized);
    }

    #[test]
    fn admin_endpoints_require_auth() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        // No auth → 401
        let resp = client.get("/api/admin/keys").dispatch();
        assert_eq!(resp.status(), Status::Unauthorized);

        let resp = client
            .post("/api/admin/keys")
            .header(ContentType::JSON)
            .body(json!({"name": "test"}).to_string())
            .dispatch();
        assert_eq!(resp.status(), Status::Unauthorized);
    }

    #[test]
    fn raw_route_enforces_time_lock() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "time-locked secret",
            "format": "plain_text",
            "time_lock": {
                "not_before": "9999-01-01T00:00:00Z"
            }
        });

        let response = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();

        assert_eq!(response.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&response.into_string().unwrap()).unwrap();

        // Raw endpoint must honour time-lock and return 423 before not_before.
        let raw = client.get(format!("/raw/{}", created.id)).dispatch();
        assert_eq!(raw.status(), Status::Locked);
    }

    #[test]
    fn raw_route_enforces_attestation() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "attested secret",
            "format": "plain_text",
            "attestation": {
                "kind": "shared_secret",
                "secret": "topsecret"
            }
        });

        let response = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();

        assert_eq!(response.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&response.into_string().unwrap()).unwrap();

        // No credentials → 401 Unauthorized (prompt, no invalid flag).
        let no_creds = client.get(format!("/raw/{}", created.id)).dispatch();
        assert_eq!(no_creds.status(), Status::Unauthorized);

        // Wrong credentials → 403 Forbidden (prompt with invalid flag).
        let wrong_creds = client
            .get(format!("/raw/{}?attest=wrongsecret", created.id))
            .dispatch();
        assert_eq!(wrong_creds.status(), Status::Forbidden);
    }

    #[test]
    fn raw_route_triggers_burn_after_reading_flow() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "burn after raw read",
            "format": "plain_text",
            "burn_after_reading": true,
            "webhook": {
                "url": "https://example.com/webhook"
            }
        });

        let response = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();

        assert_eq!(response.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&response.into_string().unwrap()).unwrap();

        // First fetch via raw endpoint → 200, paste is consumed.
        let first = client.get(format!("/raw/{}", created.id)).dispatch();
        assert_eq!(first.status(), Status::Ok);

        // Second fetch → 404, paste has been deleted.
        let second = client.get(format!("/raw/{}", created.id)).dispatch();
        assert_eq!(second.status(), Status::NotFound);
    }

    #[test]
    fn stego_builtin_carrier_embeds_and_returns_carrier_image() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "hidden message",
            "format": "plain_text",
            "encryption": {
                "algorithm": "aes256_gcm",
                "key": "stegokey"
            },
            "stego": {
                "mode": "builtin",
                "carrier": "aurora"
            }
        });

        let create = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(create.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create.into_string().unwrap()).unwrap();

        // Fetch via API with key — response must include stego info
        let get = client
            .get(format!("/api/pastes/{}?key=stegokey", created.id))
            .dispatch();
        assert_eq!(get.status(), Status::Ok);
        let view: PasteViewResponse = serde_json::from_str(&get.into_string().unwrap()).unwrap();
        assert_eq!(view.content, "hidden message");
        assert!(view.encryption.requires_key);
        let stego = view.stego.expect("stego info should be present");
        assert_eq!(stego.carrier_mime, "image/png");
        assert!(!stego.carrier_image.is_empty());
        assert!(!stego.payload_digest.is_empty());
    }

    #[test]
    fn stego_without_encryption_returns_bad_request() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "plain text with stego",
            "format": "plain_text",
            "stego": {
                "mode": "builtin",
                "carrier": "aurora"
            }
        });

        let create = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(create.status(), Status::BadRequest);
    }

    #[test]
    fn stego_uploaded_carrier_too_large_returns_payload_too_large() {
        use rocket::data::{Limits, ToByteUnit};

        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        // Increase Rocket's JSON body limit so our 1 MB application check is exercised
        // (Rocket's default 1 MiB limit would reject the request before the handler runs)
        let rocket = build_rocket(store).configure(rocket::Config {
            limits: Limits::default().limit("json", 10.mebibytes()),
            ..Default::default()
        });
        let client = Client::tracked(rocket).expect("client");

        // Build a data URI whose decoded bytes exceed 1 MB
        let large_data = vec![0u8; 1_048_577];
        let data_uri = format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD.encode(&large_data)
        );

        let payload = json!({
            "content": "hidden",
            "format": "plain_text",
            "encryption": {
                "algorithm": "aes256_gcm",
                "key": "key"
            },
            "stego": {
                "mode": "uploaded",
                "data_uri": data_uri
            }
        });

        let create = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(create.status(), Status::PayloadTooLarge);
    }

    #[test]
    fn stego_payload_digest_matches_ciphertext_sha256() {
        use sha2::{Digest, Sha256};

        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(Arc::clone(&store));
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "digest check",
            "format": "plain_text",
            "encryption": {
                "algorithm": "aes256_gcm",
                "key": "testkey"
            },
            "stego": {
                "mode": "builtin",
                "carrier": "nebula"
            }
        });

        let create = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(create.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create.into_string().unwrap()).unwrap();

        // Retrieve the stored paste directly to get the raw ciphertext
        let stored = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.get_paste(&created.id))
            .expect("paste should exist");

        let (ciphertext_b64, expected_digest) = match stored.content {
            StoredContent::Stego {
                ciphertext,
                payload_digest,
                ..
            } => (ciphertext, payload_digest),
            _ => panic!("expected Stego content variant"),
        };

        let raw = BASE64_STANDARD.decode(&ciphertext_b64).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&raw);
        let computed = format!("{:x}", hasher.finalize());
        assert_eq!(computed, expected_digest);
    }

    #[test]
    fn admin_create_list_delete_keys_with_bootstrap_token() {
        std::env::set_var("COPYPASTE_ADMIN_TOKEN", "test-admin-bootstrap");

        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        // Create a key
        let create_resp = client
            .post("/api/admin/keys")
            .header(ContentType::JSON)
            .header(rocket::http::Header::new(
                "Authorization",
                "Bearer test-admin-bootstrap",
            ))
            .body(json!({"name": "my-key", "scope": "write"}).to_string())
            .dispatch();
        assert_eq!(create_resp.status(), Status::Ok);
        let created: CreateApiKeyResponse =
            serde_json::from_str(&create_resp.into_string().unwrap()).unwrap();
        assert_eq!(created.name, "my-key");
        assert!(!created.key.is_empty());
        let key_id = created.id.clone();

        // List keys
        let list_resp = client
            .get("/api/admin/keys")
            .header(rocket::http::Header::new(
                "Authorization",
                "Bearer test-admin-bootstrap",
            ))
            .dispatch();
        assert_eq!(list_resp.status(), Status::Ok);
        let list: ListApiKeysResponse =
            serde_json::from_str(&list_resp.into_string().unwrap()).unwrap();
        assert!(!list.keys.is_empty());

        // Delete the key
        let delete_resp = client
            .delete(format!("/api/admin/keys/{}", key_id))
            .header(rocket::http::Header::new(
                "Authorization",
                "Bearer test-admin-bootstrap",
            ))
            .dispatch();
        assert_eq!(delete_resp.status(), Status::Ok);
        let deleted: RevokeApiKeyResponse =
            serde_json::from_str(&delete_resp.into_string().unwrap()).unwrap();
        assert!(deleted.revoked);
    }

    // ── Auth system adversarial tests ─────────────────────────────────────────

    #[test]
    fn auth_login_invalid_pubkey_length_returns_bad_request() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        // A pubkey that decodes to wrong byte length (not 32 bytes)
        let short_pubkey = BASE64_STANDARD.encode(b"tooshort");
        let resp = client
            .post("/api/auth/login")
            .header(ContentType::JSON)
            .body(
                json!({
                    "pubkey": short_pubkey,
                    "signature": BASE64_STANDARD.encode([0u8; 64]),
                    "challenge": "testchallenge"
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(resp.status(), Status::BadRequest);
    }

    #[test]
    fn auth_login_valid_pubkey_wrong_signature_returns_unauthorized() {
        use ed25519_dalek::SigningKey;

        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        // Generate a valid keypair from deterministic random bytes
        let secret_bytes: [u8; 32] = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32,
        ];
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();
        let pubkey_b64 = BASE64_STANDARD.encode(verifying_key.as_bytes());

        // Send a valid pubkey but all-zeros signature (wrong signature)
        let wrong_sig_b64 = BASE64_STANDARD.encode([0u8; 64]);

        let resp = client
            .post("/api/auth/login")
            .header(ContentType::JSON)
            .body(
                json!({
                    "pubkey": pubkey_b64,
                    "signature": wrong_sig_b64,
                    "challenge": "random-challenge-string-32-chars!!"
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(resp.status(), Status::Unauthorized);
    }

    // ── Tor access control tests ──────────────────────────────────────────────

    #[test]
    fn tor_only_paste_rejected_on_clearnet_show_route() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        // Create a tor-only paste
        let create_resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "secret tor paste",
                    "format": "plain_text",
                    "tor_access_only": true
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(create_resp.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create_resp.into_string().unwrap()).unwrap();

        // GET without onion header → 403 Forbidden
        let resp = client.get(format!("/{}", created.id)).dispatch();
        assert_eq!(resp.status(), Status::Forbidden);
    }

    #[test]
    fn tor_only_paste_rejected_on_clearnet_raw_route() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let create_resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "secret tor raw paste",
                    "format": "plain_text",
                    "tor_access_only": true
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(create_resp.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create_resp.into_string().unwrap()).unwrap();

        // GET /raw/{id} without onion header → 403 Forbidden
        let resp = client.get(format!("/raw/{}", created.id)).dispatch();
        assert_eq!(resp.status(), Status::Forbidden);
    }

    // ── Admin auth with missing env var ────────────────────────────────────────

    #[test]
    fn admin_auth_with_no_env_var_rejects_arbitrary_bearer_token() {
        std::env::remove_var("COPYPASTE_ADMIN_TOKEN");

        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        // Send a non-empty bearer token; no env var and no key in DB → rejected
        let resp = client
            .get("/api/admin/keys")
            .header(rocket::http::Header::new(
                "Authorization",
                "Bearer notarealtoken",
            ))
            .dispatch();
        // No admin key in DB and env var is unset → Unauthorized (no matching key)
        assert_eq!(resp.status(), Status::Unauthorized);
    }

    // ── Time lock HTTP enforcement ─────────────────────────────────────────────

    #[test]
    fn show_route_time_lock_before_not_before_renders_locked_page() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let create_resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "future paste",
                    "format": "plain_text",
                    "time_lock": { "not_before": "9999-01-01T00:00:00Z" }
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(create_resp.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create_resp.into_string().unwrap()).unwrap();

        let resp = client.get(format!("/{}", created.id)).dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let body = resp.into_string().unwrap();
        assert!(body.contains("Time-locked paste") || body.contains("unlocks after"));
    }

    #[test]
    fn show_route_time_lock_after_not_after_renders_elapsed_page() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let create_resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "expired window paste",
                    "format": "plain_text",
                    "time_lock": { "not_after": "2000-01-01T00:00:00Z" }
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(create_resp.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create_resp.into_string().unwrap()).unwrap();

        let resp = client.get(format!("/{}", created.id)).dispatch();
        // After not_after, the access window has closed
        assert_eq!(resp.status(), Status::Ok);
        let body = resp.into_string().unwrap();
        assert!(body.contains("Time window elapsed") || body.contains("Access window closed"));
    }

    #[test]
    fn raw_route_time_lock_after_not_after_returns_gone() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let create_resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "expired window raw",
                    "format": "plain_text",
                    "time_lock": { "not_after": "2000-01-01T00:00:00Z" }
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(create_resp.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create_resp.into_string().unwrap()).unwrap();

        // After not_after has elapsed, raw endpoint must return 410 Gone (not 423 Locked)
        let resp = client.get(format!("/raw/{}", created.id)).dispatch();
        assert_eq!(resp.status(), Status::Gone);
    }

    // ── Attestation handler-level integration ────────────────────────────────

    #[test]
    fn show_route_attestation_without_code_renders_prompt() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let create_resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "attested content",
                    "format": "plain_text",
                    "attestation": { "kind": "shared_secret", "secret": "s3cr3t" }
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(create_resp.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create_resp.into_string().unwrap()).unwrap();

        // No credentials → HTML prompt (200 OK, not the content)
        let resp = client.get(format!("/{}", created.id)).dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let body = resp.into_string().unwrap();
        assert!(!body.contains("attested content"), "content must not leak");
        assert!(body.contains("form") || body.contains("attest") || body.contains("password"));
    }

    #[test]
    fn show_route_attestation_wrong_secret_renders_prompt() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let create_resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "attested content",
                    "format": "plain_text",
                    "attestation": { "kind": "shared_secret", "secret": "correct_secret" }
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(create_resp.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create_resp.into_string().unwrap()).unwrap();

        // Wrong credentials → still renders prompt (not the content)
        let resp = client
            .get(format!("/{0}?attest=wrongsecret", created.id))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let body = resp.into_string().unwrap();
        assert!(
            !body.contains("attested content"),
            "content must not leak on wrong secret"
        );
    }

    #[test]
    fn show_route_attestation_correct_secret_shows_content() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let create_resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "attested secret content",
                    "format": "plain_text",
                    "attestation": { "kind": "shared_secret", "secret": "correct_secret" }
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(create_resp.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create_resp.into_string().unwrap()).unwrap();

        // Correct credentials → content is shown
        let resp = client
            .get(format!("/{0}?attest=correct_secret", created.id))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let body = resp.into_string().unwrap();
        assert!(
            body.contains("attested secret content"),
            "correct secret must grant access"
        );
    }

    // ── User paste enumeration (fixed: session auth is now required) ──────────

    #[test]
    fn user_paste_list_cannot_enumerate_other_users() {
        // /api/user/pastes now requires a valid session token and never returns
        // data for a pubkey_hash other than the session's own.
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        // Create a paste with a victim's owner hash.
        let create_resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "owner-only paste",
                    "format": "plain_text",
                    "owner_pubkey_hash": "victim_hash_abc123"
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(create_resp.status(), Status::Ok);

        // Unauthenticated enumeration attempt → 401.
        let resp = client
            .get("/api/user/pastes?pubkey_hash=victim_hash_abc123")
            .dispatch();
        assert_eq!(resp.status(), Status::Unauthorized);

        // Authenticated as a different user, requesting the victim's hash → 403.
        let (token, pubkey_hash) = login(&client);
        assert_ne!(pubkey_hash, "victim_hash_abc123");
        let resp = client
            .get("/api/user/pastes?pubkey_hash=victim_hash_abc123")
            .header(bearer(&token))
            .dispatch();
        assert_eq!(resp.status(), Status::Forbidden);

        // Same for the paste-count endpoint.
        let resp = client
            .get("/api/user/paste-count?pubkey_hash=victim_hash_abc123")
            .header(bearer(&token))
            .dispatch();
        assert_eq!(resp.status(), Status::Forbidden);

        // The session's own (matching) hash is still accepted as a query param
        // for backward compatibility and returns only the caller's pastes.
        let resp = client
            .get(format!("/api/user/pastes?pubkey_hash={pubkey_hash}"))
            .header(bearer(&token))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let parsed: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
        assert!(parsed["pastes"].as_array().unwrap().is_empty());
    }

    // ── Input validation tests ─────────────────────────────────────────────────

    #[test]
    fn create_api_rejects_oversized_content() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let content = "a".repeat(10_485_761);
        let payload = json!({ "content": content, "format": "plain_text" });

        let resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(resp.status(), Status::PayloadTooLarge);
    }

    #[test]
    fn create_api_accepts_content_at_size_limit() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let content = "a".repeat(10_485_760);
        let payload = json!({ "content": content, "format": "plain_text" });

        let resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
    }

    #[test]
    fn webhook_config_rejects_oversized_view_template() {
        let err = webhook_config_from_request(&WebhookRequest {
            url: "https://example.com".into(),
            view_template: Some("x".repeat(4097)),
            ..Default::default()
        })
        .expect_err("long view_template should fail");
        assert_eq!(err.0, Status::BadRequest);
    }

    #[test]
    fn webhook_config_rejects_oversized_burn_template() {
        let err = webhook_config_from_request(&WebhookRequest {
            url: "https://example.com".into(),
            burn_template: Some("x".repeat(4097)),
            ..Default::default()
        })
        .expect_err("long burn_template should fail");
        assert_eq!(err.0, Status::BadRequest);
    }

    #[test]
    fn webhook_config_accepts_templates_at_limit() {
        let template = "x".repeat(4096);
        let cfg = webhook_config_from_request(&WebhookRequest {
            url: "https://example.com".into(),
            view_template: Some(template.clone()),
            burn_template: Some(template),
            ..Default::default()
        })
        .expect("templates at limit should succeed");
        assert_eq!(cfg.url, "https://example.com");
    }

    #[test]
    fn create_api_rejects_bundle_exceeding_child_limit() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let children: Vec<_> = (0..51)
            .map(|i| json!({ "content": format!("child {}", i) }))
            .collect();
        let payload = json!({
            "content": "bundle parent",
            "format": "plain_text",
            "encryption": { "algorithm": "aes256_gcm", "key": "bundlekey" },
            "bundle": { "children": children }
        });

        let resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(resp.status(), Status::BadRequest);
    }

    #[test]
    fn stego_uploaded_rejects_invalid_mime_type() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let data_uri = format!(
            "data:text/plain;base64,{}",
            BASE64_STANDARD.encode(b"fake image data")
        );
        let payload = json!({
            "content": "hidden",
            "format": "plain_text",
            "encryption": { "algorithm": "aes256_gcm", "key": "key" },
            "stego": { "mode": "uploaded", "data_uri": data_uri }
        });

        let resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(resp.status(), Status::BadRequest);
    }

    #[test]
    fn stego_uploaded_rejects_oversized_data_uri_string() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        // data_uri string length > 10_000_000; '!' is invalid base64, so without
        // the string-length check this would return 400 (invalid URI), not 413.
        let data_uri = format!("data:image/png;base64,{}", "!".repeat(10_000_001));
        let payload = json!({
            "content": "hidden",
            "format": "plain_text",
            "encryption": { "algorithm": "aes256_gcm", "key": "key" },
            "stego": { "mode": "uploaded", "data_uri": data_uri }
        });

        let resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(resp.status(), Status::PayloadTooLarge);
    }

    // ── Live paste owner token hash ───────────────────────────────────────────

    #[test]
    fn live_paste_owner_token_hash_is_sha256_of_plaintext_token() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(Arc::clone(&store));
        let client = Client::tracked(rocket).expect("client");

        let create_resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "live paste content",
                    "format": "plain_text",
                    "live": true
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(create_resp.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create_resp.into_string().unwrap()).unwrap();

        let plaintext_token = created
            .token
            .expect("live paste must include plaintext token");
        assert!(!plaintext_token.is_empty());

        // Retrieve the stored paste and verify the hash
        let rt = tokio::runtime::Runtime::new().unwrap();
        let stored = rt
            .block_on(store.get_paste(&created.id))
            .expect("paste should exist");

        let stored_hash = stored
            .owner_token_hash
            .expect("live paste must store token hash");

        // The stored hash must be the SHA-256 hex of the plaintext token
        let mut hasher = Sha256::new();
        hasher.update(plaintext_token.as_bytes());
        let expected_hash = format!("{:x}", hasher.finalize());
        assert_eq!(
            stored_hash, expected_hash,
            "owner_token_hash must be SHA-256 of plaintext token"
        );
    }

    // ── X-Paste-Key header (keys out of query strings) ────────────────────────

    #[test]
    fn show_api_accepts_key_via_header_and_header_wins_over_query() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "header secret",
            "format": "plain_text",
            "encryption": { "algorithm": "aes256_gcm", "key": "headerpass" }
        });
        let create = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(create.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create.into_string().unwrap()).unwrap();

        // Key via header only → 200.
        let ok = client
            .get(format!("/api/pastes/{}", created.id))
            .header(rocket::http::Header::new("X-Paste-Key", "headerpass"))
            .dispatch();
        assert_eq!(ok.status(), Status::Ok);
        let view: PasteViewResponse = serde_json::from_str(&ok.into_string().unwrap()).unwrap();
        assert_eq!(view.content, "header secret");

        // Wrong header + correct query param → header takes precedence → 403.
        let forbidden = client
            .get(format!("/api/pastes/{}?key=headerpass", created.id))
            .header(rocket::http::Header::new("X-Paste-Key", "wrong"))
            .dispatch();
        assert_eq!(forbidden.status(), Status::Forbidden);

        // Query param alone still works (backward compatibility).
        let compat = client
            .get(format!("/api/pastes/{}?key=headerpass", created.id))
            .dispatch();
        assert_eq!(compat.status(), Status::Ok);
    }

    // ── Webhook SSRF validation at paste creation ──────────────────────────────

    #[test]
    fn create_api_rejects_ssrf_webhook_urls_with_structured_400() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        for url in [
            "http://127.0.0.1:8000/internal",
            "http://169.254.169.254/latest/meta-data",
            "http://localhost/hook",
            "http://10.1.2.3/hook",
            "file:///etc/passwd",
        ] {
            let payload = json!({
                "content": "payload",
                "format": "plain_text",
                "webhook": { "url": url }
            });
            let resp = client
                .post("/api/pastes")
                .header(ContentType::JSON)
                .body(payload.to_string())
                .dispatch();
            assert_eq!(resp.status(), Status::BadRequest, "should reject {url}");
            let err: ApiError = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
            assert_eq!(err.code, "bad_request");
        }

        // A public webhook URL is still accepted.
        let payload = json!({
            "content": "payload",
            "format": "plain_text",
            "webhook": { "url": "https://hooks.slack.com/services/T/B/X" }
        });
        let resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
    }

    // ── Retention config enforcement ───────────────────────────────────────────

    #[test]
    fn create_api_rejects_retention_above_configured_max() {
        std::env::set_var("COPYPASTE_RETENTION_MAX_MINUTES", "60");

        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let over = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(json!({ "content": "x", "retention_minutes": 61 }).to_string())
            .dispatch();
        assert_eq!(over.status(), Status::BadRequest);

        let at_limit = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(json!({ "content": "x", "retention_minutes": 60 }).to_string())
            .dispatch();
        assert_eq!(at_limit.status(), Status::Ok);

        std::env::remove_var("COPYPASTE_RETENTION_MAX_MINUTES");
    }

    #[test]
    fn create_api_applies_default_retention_when_none_requested() {
        std::env::set_var("COPYPASTE_RETENTION_DEFAULT_MINUTES", "30");

        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(Arc::clone(&store));
        let client = Client::tracked(rocket).expect("client");

        let resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(json!({ "content": "defaults", "format": "plain_text" }).to_string())
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&resp.into_string().unwrap()).unwrap();

        let stored = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.get_paste(&created.id))
            .expect("paste should exist");
        let expires_at = stored.expires_at.expect("default retention must apply");
        let expected = current_timestamp() + 30 * 60;
        assert!(
            (expires_at - expected).abs() <= 5,
            "expires_at should be ~30 minutes out"
        );

        std::env::remove_var("COPYPASTE_RETENTION_DEFAULT_MINUTES");
    }

    // ── Per-IP rate limiting (config knobs wired up) ───────────────────────────

    #[test]
    fn create_rate_limit_returns_429_when_exceeded() {
        std::env::set_var("COPYPASTE_RATE_LIMIT_CREATES", "2");

        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        std::env::remove_var("COPYPASTE_RATE_LIMIT_CREATES");

        let body = json!({ "content": "rate", "format": "plain_text" }).to_string();
        for _ in 0..2 {
            let resp = client
                .post("/api/pastes")
                .header(ContentType::JSON)
                .body(body.clone())
                .dispatch();
            assert_eq!(resp.status(), Status::Ok);
        }
        let resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(body)
            .dispatch();
        assert_eq!(resp.status(), Status::TooManyRequests);
    }

    // ── Workspace persistence & listing ────────────────────────────────────────

    #[test]
    fn workspace_is_persisted_and_returned_in_view_response() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(Arc::clone(&store));
        let client = Client::tracked(rocket).expect("client");

        let resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "ws content",
                    "format": "plain_text",
                    "workspace": "team-alpha"
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&resp.into_string().unwrap()).unwrap();

        // Persisted on the stored paste metadata.
        let stored = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.get_paste(&created.id))
            .expect("paste should exist");
        assert_eq!(stored.metadata.workspace.as_deref(), Some("team-alpha"));

        // Surfaced in the JSON view response.
        let view_resp = client.get(format!("/api/pastes/{}", created.id)).dispatch();
        assert_eq!(view_resp.status(), Status::Ok);
        let view: PasteViewResponse =
            serde_json::from_str(&view_resp.into_string().unwrap()).unwrap();
        assert_eq!(view.workspace.as_deref(), Some("team-alpha"));
    }

    #[test]
    fn workspace_listing_requires_session_and_scopes_to_owner() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        // Unauthenticated → 401.
        let resp = client.get("/api/workspaces/team-alpha/pastes").dispatch();
        assert_eq!(resp.status(), Status::Unauthorized);

        let (token, pubkey_hash) = login(&client);

        // One paste owned by the session in the workspace, one owned by someone else.
        for (owner, content) in [(pubkey_hash.as_str(), "mine"), ("someone_else", "theirs")] {
            let resp = client
                .post("/api/pastes")
                .header(ContentType::JSON)
                .body(
                    json!({
                        "content": content,
                        "format": "plain_text",
                        "workspace": "team-alpha",
                        "owner_pubkey_hash": owner
                    })
                    .to_string(),
                )
                .dispatch();
            assert_eq!(resp.status(), Status::Ok);
        }

        let resp = client
            .get("/api/workspaces/team-alpha/pastes")
            .header(bearer(&token))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let parsed: serde_json::Value = serde_json::from_str(&resp.into_string().unwrap()).unwrap();
        let pastes = parsed["pastes"].as_array().unwrap();
        assert_eq!(pastes.len(), 1, "only the caller's own paste is listed");
        assert_eq!(pastes[0]["workspace"], "team-alpha");
    }

    // ── Live paste update & finalize routes ────────────────────────────────────

    fn create_live_paste(client: &Client) -> (String, String) {
        let resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "live v1",
                    "format": "plain_text",
                    "live": true
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&resp.into_string().unwrap()).unwrap();
        (created.id, created.token.expect("ownership token"))
    }

    #[test]
    fn update_api_requires_ownership_token() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let (id, token) = create_live_paste(&client);
        let body = json!({ "content": "live v2" }).to_string();

        // Missing token → 401.
        let resp = client
            .put(format!("/api/pastes/{id}"))
            .header(ContentType::JSON)
            .body(body.clone())
            .dispatch();
        assert_eq!(resp.status(), Status::Unauthorized);

        // Wrong token → 403.
        let resp = client
            .put(format!("/api/pastes/{id}"))
            .header(ContentType::JSON)
            .header(bearer("wrong-token"))
            .body(body.clone())
            .dispatch();
        assert_eq!(resp.status(), Status::Forbidden);

        // Correct token → 200 and the content is replaced.
        let resp = client
            .put(format!("/api/pastes/{id}"))
            .header(ContentType::JSON)
            .header(bearer(&token))
            .body(body)
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);

        let view = client.get(format!("/api/pastes/{id}")).dispatch();
        assert_eq!(view.status(), Status::Ok);
        let view: PasteViewResponse = serde_json::from_str(&view.into_string().unwrap()).unwrap();
        assert_eq!(view.content, "live v2");
        assert!(view.is_live);
    }

    #[test]
    fn update_api_returns_404_for_missing_paste() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let resp = client
            .put("/api/pastes/does-not-exist")
            .header(ContentType::JSON)
            .header(bearer("any"))
            .body(json!({ "content": "x" }).to_string())
            .dispatch();
        assert_eq!(resp.status(), Status::NotFound);
    }

    #[test]
    fn update_api_rejects_non_live_paste() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        // A regular (non-live) paste has no ownership token → 409 conflict.
        let resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(json!({ "content": "static", "format": "plain_text" }).to_string())
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&resp.into_string().unwrap()).unwrap();

        let resp = client
            .put(format!("/api/pastes/{}", created.id))
            .header(ContentType::JSON)
            .header(bearer("any"))
            .body(json!({ "content": "y" }).to_string())
            .dispatch();
        assert_eq!(resp.status(), Status::Conflict);
    }

    #[test]
    fn finalize_api_stops_further_updates() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let (id, token) = create_live_paste(&client);

        // Finalize without a token → 401.
        let resp = client
            .patch(format!("/api/pastes/{id}/finalize"))
            .dispatch();
        assert_eq!(resp.status(), Status::Unauthorized);

        // Finalize with the wrong token → 403.
        let resp = client
            .patch(format!("/api/pastes/{id}/finalize"))
            .header(bearer("wrong"))
            .dispatch();
        assert_eq!(resp.status(), Status::Forbidden);

        // Requesting live=true is invalid.
        let resp = client
            .patch(format!("/api/pastes/{id}/finalize"))
            .header(ContentType::JSON)
            .header(bearer(&token))
            .body(json!({ "live": true }).to_string())
            .dispatch();
        assert_eq!(resp.status(), Status::BadRequest);

        // Finalize with the correct token → 200.
        let resp = client
            .patch(format!("/api/pastes/{id}/finalize"))
            .header(bearer(&token))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let finalized: FinalizePasteResponse =
            serde_json::from_str(&resp.into_string().unwrap()).unwrap();
        assert!(!finalized.is_live);

        // Update after finalize is rejected with 409, even with the right token.
        let resp = client
            .put(format!("/api/pastes/{id}"))
            .header(ContentType::JSON)
            .header(bearer(&token))
            .body(json!({ "content": "after finalize" }).to_string())
            .dispatch();
        assert_eq!(resp.status(), Status::Conflict);

        // Finalizing again is idempotent.
        let resp = client
            .patch(format!("/api/pastes/{id}/finalize"))
            .header(bearer(&token))
            .dispatch();
        assert_eq!(resp.status(), Status::Ok);
    }

    // ── OpenAPI docs ───────────────────────────────────────────────────────────

    #[test]
    fn openapi_json_and_scalar_docs_are_served() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let resp = client.get("/api/openapi.json").dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let doc: serde_json::Value =
            serde_json::from_str(&resp.into_string().unwrap()).expect("valid OpenAPI JSON");
        assert!(doc["paths"]["/api/pastes"].is_object());
        assert!(doc["paths"]["/api/pastes/{id}"].is_object());
        assert!(doc["components"]["schemas"]["CreatePasteRequest"].is_object());

        let resp = client.get("/api/docs").dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let html = resp.into_string().unwrap();
        assert!(html.contains("<html") || html.contains("scalar"));
    }
}
