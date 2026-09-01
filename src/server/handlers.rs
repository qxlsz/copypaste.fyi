use std::{collections::HashSet, path::PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use rocket::{
    catch, catchers,
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
    create_paste_store, EncryptionAlgorithm, PasteError, PasteFormat, PasteMetadata,
    PasteMutationError, PersistenceLocator, SharedPasteStore, StoredContent, StoredPaste,
    WebhookConfig,
};
use sha2::{Digest, Sha256};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rand::Rng;

use super::api_keys::{
    ApiKeyStoreOpenError, OwnerBearerToken, RateLimiter, RequireAdminAuth,
    RequireMutationWriteAuth, RequireWriteAuth, SharedApiKeyStore, SharedRateLimiter,
    SqliteApiKeyStore, StaticAuthTokens, WritePrincipal,
};
use super::attestation::{self, AttestationVerdict};
use super::blockchain::{
    default_anchor_relayer, infer_attestation_ref, infer_retention_class, manifest_hash,
    AnchorManifest, AnchorPayload, SharedAnchorRelayer,
};
use super::cors::{api_preflight, Cors};
use super::crypto::{decrypt_content, encrypt_content, DecryptError};
use super::models::{
    AdminDeletePasteResponse, AdminPasteMetadataResponse, AnchorRequest, AnchorResponse, ApiError,
    ApiKeyInfo, AuthChallengeResponse, AuthLoginRequest, AuthLoginResponse, AuthLogoutResponse,
    CreateApiKeyRequest, CreateApiKeyResponse, CreatePasteApiSchema, CreatePasteRequest,
    CreatePasteResponse, FinalizePasteRequest, FinalizePasteResponse, ListApiKeysResponse,
    PasteEncryptionInfo, PasteTimeLockInfo, PasteViewQuery, PasteViewResponse, PersistenceRequest,
    RevokeApiKeyResponse, StatsSummaryResponse, StegoRequest, TimeLockRequest, UpdatePasteRequest,
    UpdatePasteResponse, UserPasteCountResponse, UserPasteListItem, UserPasteListResponse,
    WebhookRequest, WorkspacePasteItem, WorkspacePasteListResponse,
};
use super::rate_limit::{CreateRateLimit, PasteRateLimiter, ReadRateLimit};
use super::render::{
    render_attestation_prompt, render_invalid_key, render_key_prompt, render_paste_view,
    render_time_locked, StoredPasteView,
};
use super::sessions::{BearerToken, RequireUserSession, SessionStore, SharedSessionStore};
use super::stego::{embed_payload, parse_data_uri, StegoCarrierSource};
use super::time::{current_timestamp, evaluate_time_lock, parse_timestamp, TimeLockState};
use super::tor::{OnionAccess, TorConfig};
use super::webhook::{trigger_webhook, validate_webhook_url, WebhookClient, WebhookEvent};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

#[derive(Default)]
struct BlockedPasteIds(HashSet<String>);

impl BlockedPasteIds {
    fn from_env() -> Self {
        let Some(value) = std::env::var("COPYPASTE_BLOCKED_PASTE_IDS")
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            return Self::default();
        };
        Self::from_csv(&value).unwrap_or_else(|()| {
            panic!("COPYPASTE_BLOCKED_PASTE_IDS contains an invalid paste identifier")
        })
    }

    fn from_csv(value: &str) -> Result<Self, ()> {
        let mut ids = HashSet::new();
        for id in value.split(',').map(str::trim).filter(|id| !id.is_empty()) {
            if !is_valid_paste_id(id) {
                return Err(());
            }
            ids.insert(id.to_string());
        }
        Ok(Self(ids))
    }

    fn contains(&self, id: &str) -> bool {
        self.0.contains(id)
    }
}

#[derive(Default)]
struct FeaturePolicy {
    allow_webhooks: bool,
    allow_uploaded_stego: bool,
    allow_attestations: bool,
    allow_stego: bool,
}

impl FeaturePolicy {
    fn from_env() -> Self {
        for disabled_flag in [
            "COPYPASTE_ALLOW_WEBHOOKS",
            "COPYPASTE_ALLOW_ATTESTATIONS",
            "COPYPASTE_ALLOW_UPLOADED_STEGO",
            "COPYPASTE_ALLOW_STEGO",
        ] {
            if boolean_env(disabled_flag) {
                panic!("{disabled_flag} is unsupported in the hardened server build");
            }
        }
        Self::default()
    }

    #[cfg(test)]
    fn new(allow_webhooks: bool, allow_uploaded_stego: bool) -> Self {
        Self {
            allow_webhooks,
            allow_uploaded_stego,
            allow_attestations: false,
            allow_stego: allow_uploaded_stego,
        }
    }

    #[cfg(test)]
    fn with_attestations(mut self, allow_attestations: bool) -> Self {
        self.allow_attestations = allow_attestations;
        self
    }

    #[cfg(test)]
    fn with_stego(mut self, allow_stego: bool) -> Self {
        self.allow_stego = allow_stego;
        self
    }
}

fn boolean_env(name: &str) -> bool {
    let Ok(value) = std::env::var(name) else {
        return false;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "0" | "false" | "no" | "off" => false,
        "1" | "true" | "yes" | "on" => true,
        _ => panic!("{name} must be true or false"),
    }
}

fn is_valid_paste_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub fn build_rocket(store: SharedPasteStore) -> Rocket<Build> {
    let api_key_store = api_key_store_from_env()
        .unwrap_or_else(|error| panic!("failed to initialise configured API key store: {error}"));
    build_rocket_with_defaults(store, api_key_store)
}

fn api_key_store_from_env() -> Result<SharedApiKeyStore, ApiKeyStoreOpenError> {
    let store = match std::env::var_os("COPYPASTE_SQLITE_PATH") {
        // An explicitly configured path must open safely; never fall back.
        Some(_) => SqliteApiKeyStore::open_configured()?,
        // Without a durable path, static auth tokens continue to work while
        // dynamic API-key verification/management remains unavailable.
        None => SqliteApiKeyStore::disabled(),
    };
    Ok(std::sync::Arc::new(store))
}

fn build_rocket_with_defaults(
    store: SharedPasteStore,
    api_key_store: SharedApiKeyStore,
) -> Rocket<Build> {
    build_rocket_with_components(
        store,
        api_key_store,
        PasteRateLimiter::from_env(),
        StaticAuthTokens::from_env(),
        FeaturePolicy::from_env(),
        BlockedPasteIds::from_env(),
        std::env::var("COPYPASTE_TRUSTED_IP_HEADER")
            .ok()
            .filter(|header| !header.is_empty()),
    )
}

fn build_rocket_with_components(
    store: SharedPasteStore,
    api_key_store: SharedApiKeyStore,
    paste_rate_limiter: PasteRateLimiter,
    static_auth_tokens: StaticAuthTokens,
    feature_policy: FeaturePolicy,
    blocked_paste_ids: BlockedPasteIds,
    trusted_ip_header: Option<String>,
) -> Rocket<Build> {
    let tor_config = TorConfig::from_env();
    let rate_limiter: SharedRateLimiter = std::sync::Arc::new(RateLimiter::new());
    let webhook_client = WebhookClient::new();
    let session_store: SharedSessionStore = std::sync::Arc::new(SessionStore::new());

    // Merge onto Rocket's standard figment so ROCKET_ADDRESS / ROCKET_PORT /
    // Rocket.toml still apply — `.configure(Config { ..Default::default() })`
    // would silently discard them (Default binds 127.0.0.1, which broke Fly).
    let figment = rocket::Config::figment()
        .merge(("limits", Limits::default().limit("json", 2u64.mebibytes())));
    // Rocket otherwise trusts X-Real-IP by default. Forwarded client-IP
    // headers are only safe when an explicitly trusted edge proxy overwrites
    // them (Fly deployments should configure `Fly-Client-IP`).
    let figment = match trusted_ip_header {
        Some(header) => figment.merge(("ip_header", header)),
        None => figment.merge(("ip_header", false)),
    };

    rocket::custom(figment)
        .manage(store)
        .manage(default_anchor_relayer())
        .manage(tor_config)
        .manage(api_key_store)
        .manage(static_auth_tokens)
        .manage(feature_policy)
        .manage(blocked_paste_ids)
        .manage(rate_limiter)
        .manage(webhook_client)
        .manage(session_store)
        .manage(paste_rate_limiter)
        .attach(Cors)
        .register("/", catchers![unauthorized_api])
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
                show_share,
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
                admin_get_paste_api,
                admin_delete_paste_api,
                openapi_json,
                agent_discovery,
                spa_fallback
            ],
        )
        .mount("/static", FileServer::from("static"))
}

pub async fn launch() -> Result<(), Box<dyn std::error::Error>> {
    let store = create_paste_store()?;
    // Dynamic API keys are enabled only when a secure durable path is
    // configured. A bad explicit path aborts startup; an absent path uses
    // static tokens and leaves dynamic key management disabled.
    let api_key_store = api_key_store_from_env()?;
    build_rocket_with_defaults(store, api_key_store)
        .launch()
        .await?;
    Ok(())
}

/// OpenAPI document aggregating all `#[utoipa::path]`-annotated handlers.
/// Served as JSON at `/api/openapi.json` so the API reference does not require
/// a third-party interactive-doc runtime on the secrets origin.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "copypaste.fyi API",
        description = "Paste sharing with encryption, burn-after-reading, time locks, and administrative moderation."
    ),
    paths(
        create,
        create_api,
        update_api,
        finalize_api,
        show_api,
        show_share,
        anchor_api,
        stats_summary_api,
        auth_challenge_api,
        auth_login_api,
        auth_logout_api,
        user_paste_count_api,
        user_paste_list_api,
        workspace_pastes_api,
        health_detailed_api,
        agent_discovery,
        admin_get_paste_api,
        admin_delete_paste_api,
    ),
    components(schemas(
        CreatePasteApiSchema,
        CreatePasteResponse,
        UpdatePasteRequest,
        UpdatePasteResponse,
        FinalizePasteRequest,
        FinalizePasteResponse,
        PasteViewResponse,
        PasteEncryptionInfo,
        PasteTimeLockInfo,
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
        AdminPasteMetadataResponse,
        AdminDeletePasteResponse,
        TimeLockRequest,
        AgentDiscovery,
        ApiError,
        super::models::EncryptionRequest,
        crate::PasteFormat,
        crate::EncryptionAlgorithm,
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

/// How another agent sends and reads pastes. Tokens go in headers, not argv.
#[derive(Serialize, Deserialize, ToSchema)]
struct AgentDiscovery {
    copypaste: u8,
    create: String,
    read: String,
    write_header: String,
    key_header: String,
    encryption: Vec<String>,
    note: String,
}

#[utoipa::path(
    get,
    path = "/.well-known/copypaste.json",
    responses((status = 200, description = "Agent discovery document", body = AgentDiscovery))
)]
#[get("/.well-known/copypaste.json")]
fn agent_discovery() -> Json<AgentDiscovery> {
    Json(AgentDiscovery {
        copypaste: 1,
        create: "/api/pastes".to_string(),
        read: "/api/pastes/{id}".to_string(),
        write_header: "X-CopyPaste-Write-Token".to_string(),
        key_header: "X-Paste-Key".to_string(),
        encryption: vec![
            "aes256_gcm".to_string(),
            "chacha20_poly1305".to_string(),
            "xchacha20_poly1305".to_string(),
        ],
        note: "Without X-Paste-Key the body stays ciphertext. Missing, burned, and expired reads are the same 404.".to_string(),
    })
}

#[get("/health")]
async fn health_api() -> Json<HealthResponse> {
    Json(public_health_response())
}

fn public_health_response() -> HealthResponse {
    HealthResponse {
        status: "ok".to_string(),
        timestamp: current_timestamp(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit: option_env!("GIT_COMMIT").map(String::from),
    }
}

#[utoipa::path(
    get,
    path = "/api/health",
    responses((status = 200, description = "Minimal public liveness", body = HealthResponse))
)]
#[get("/api/health")]
async fn health_detailed_api() -> Json<HealthResponse> {
    // Public health checks are deliberately liveness-only. Dependency status,
    // paste counts, internal URLs, and upstream errors belong in protected
    // telemetry rather than an unauthenticated response.
    Json(public_health_response())
}

#[catch(401)]
fn unauthorized_api() -> (Status, Json<ApiError>) {
    (
        Status::Unauthorized,
        Json(ApiError::new("unauthorized", "Authentication required.")),
    )
}

#[utoipa::path(
    get,
    path = "/api/stats/summary",
    responses(
        (status = 200, description = "Stats summary", body = StatsSummaryResponse),
        (status = 429, description = "Read rate limit exceeded"),
    )
)]
#[get("/api/stats/summary")]
async fn stats_summary_api(
    _rate: ReadRateLimit,
    store: &State<SharedPasteStore>,
) -> Json<StatsSummaryResponse> {
    let stats = store.stats().await;
    Json(stats.into())
}

#[utoipa::path(
    get,
    path = "/api/auth/challenge",
    responses(
        (status = 200, description = "Auth challenge", body = AuthChallengeResponse),
        (status = 429, description = "Rate limit exceeded"),
    )
)]
#[get("/api/auth/challenge")]
async fn auth_challenge_api(
    _rate: CreateRateLimit,
    sessions: &State<SharedSessionStore>,
) -> Json<AuthChallengeResponse> {
    let challenge = sessions.issue_challenge();
    Json(AuthChallengeResponse { challenge })
}

const MAX_AUTH_CHALLENGE_LEN: usize = 128;
const MAX_AUTH_PUBKEY_B64_LEN: usize = 64;
const MAX_AUTH_SIGNATURE_B64_LEN: usize = 128;

#[utoipa::path(
    post,
    path = "/api/auth/login",
    request_body = AuthLoginRequest,
    responses(
        (status = 200, description = "Auth login response", body = AuthLoginResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 429, description = "Rate limit exceeded"),
    )
)]
#[post("/api/auth/login", data = "<body>")]
async fn auth_login_api(
    _rate: CreateRateLimit,
    sessions: &State<SharedSessionStore>,
    body: Json<AuthLoginRequest>,
) -> Result<Json<AuthLoginResponse>, (Status, String)> {
    let body = body.into_inner();

    if body.challenge.is_empty() || body.challenge.len() > MAX_AUTH_CHALLENGE_LEN {
        return Err((Status::BadRequest, "Invalid challenge length".to_string()));
    }
    if body.pubkey.is_empty() || body.pubkey.len() > MAX_AUTH_PUBKEY_B64_LEN {
        return Err((Status::BadRequest, "Invalid pubkey length".to_string()));
    }
    if body.signature.is_empty() || body.signature.len() > MAX_AUTH_SIGNATURE_B64_LEN {
        return Err((Status::BadRequest, "Invalid signature length".to_string()));
    }

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

    // Issuance is checked only after signature verification. Successful
    // consumption is atomic, making replayed or arbitrary signed challenges
    // unusable while allowing a client to retry after a malformed signature.
    if !sessions.consume_challenge(&body.challenge) {
        return Err((
            Status::Unauthorized,
            "Challenge is unknown, expired, or already used".to_string(),
        ));
    }

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
        (status = 429, description = "Read rate limit exceeded"),
        (status = 503, description = "Paste storage unavailable", body = ApiError),
    )
)]
#[get("/api/user/paste-count?<pubkey_hash>")]
async fn user_paste_count_api(
    _rate: ReadRateLimit,
    store: &State<SharedPasteStore>,
    blocked: &State<BlockedPasteIds>,
    session: RequireUserSession,
    pubkey_hash: Option<String>,
) -> Result<Json<UserPasteCountResponse>, (Status, Json<ApiError>)> {
    check_pubkey_hash_param(&session, pubkey_hash.as_deref())?;

    // Count pastes owned by the authenticated user only.
    let all_pastes = store.get_all_paste_ids().await;
    let mut count = 0;

    for id in all_pastes {
        if blocked.contains(&id) {
            continue;
        }
        match store.get_paste(&id).await {
            Ok(paste) => {
                if paste.metadata.owner_pubkey_hash.as_deref() == Some(session.pubkey_hash.as_str())
                {
                    count += 1;
                }
            }
            Err(PasteError::NotFound(_) | PasteError::Expired(_)) => {}
            Err(PasteError::Persistence(_)) => {
                return Err(to_api_err(
                    Status::ServiceUnavailable,
                    "Paste storage is temporarily unavailable".to_string(),
                ));
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
        (status = 429, description = "Read rate limit exceeded"),
        (status = 503, description = "Paste storage unavailable", body = ApiError),
    )
)]
#[get("/api/user/pastes?<pubkey_hash>")]
async fn user_paste_list_api(
    _rate: ReadRateLimit,
    store: &State<SharedPasteStore>,
    blocked: &State<BlockedPasteIds>,
    session: RequireUserSession,
    pubkey_hash: Option<String>,
) -> Result<Json<UserPasteListResponse>, (Status, Json<ApiError>)> {
    check_pubkey_hash_param(&session, pubkey_hash.as_deref())?;

    // List pastes owned by the authenticated user only.
    let all_pastes = store.get_all_paste_ids().await;
    let mut user_pastes = Vec::new();

    for id in all_pastes {
        if blocked.contains(&id) {
            continue;
        }
        match store.get_paste(&id).await {
            Ok(paste)
                if paste.metadata.owner_pubkey_hash.as_deref()
                    == Some(session.pubkey_hash.as_str()) =>
            {
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
                    url: format!("/p/{id}"),
                    created_at: paste.created_at,
                    expires_at: paste.expires_at,
                    retention_minutes,
                    burn_after_reading: paste.burn_after_reading,
                    format: format!("{:?}", paste.format).to_lowercase(),
                    workspace: paste.metadata.workspace.clone(),
                });
            }
            Ok(_) | Err(PasteError::NotFound(_) | PasteError::Expired(_)) => {}
            Err(PasteError::Persistence(_)) => {
                return Err(to_api_err(
                    Status::ServiceUnavailable,
                    "Paste storage is temporarily unavailable".to_string(),
                ));
            }
        }
    }

    // Sort by created_at descending (newest first)
    user_pastes.sort_by_key(|p| std::cmp::Reverse(p.created_at));

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
        (status = 429, description = "Read rate limit exceeded"),
        (status = 503, description = "Paste storage unavailable", body = ApiError),
    )
)]
#[get("/api/workspaces/<name>/pastes")]
async fn workspace_pastes_api(
    _rate: ReadRateLimit,
    store: &State<SharedPasteStore>,
    blocked: &State<BlockedPasteIds>,
    session: RequireUserSession,
    name: String,
) -> Result<Json<WorkspacePasteListResponse>, (Status, Json<ApiError>)> {
    // Only the caller's own pastes within the workspace are listed.
    let all_pastes = store.get_all_paste_ids().await;
    let mut pastes = Vec::new();

    for id in all_pastes {
        if blocked.contains(&id) {
            continue;
        }
        match store.get_paste(&id).await {
            Ok(paste)
                if paste.metadata.workspace.as_deref() == Some(name.as_str())
                    && paste.metadata.owner_pubkey_hash.as_deref()
                        == Some(session.pubkey_hash.as_str()) =>
            {
                pastes.push(WorkspacePasteItem {
                    id: id.clone(),
                    url: format!("/p/{id}"),
                    workspace: paste.metadata.workspace.clone(),
                    created_at: paste.created_at,
                });
            }
            Ok(_) | Err(PasteError::NotFound(_) | PasteError::Expired(_)) => {}
            Err(PasteError::Persistence(_)) => {
                return Err(to_api_err(
                    Status::ServiceUnavailable,
                    "Paste storage is temporarily unavailable".to_string(),
                ));
            }
        }
    }

    pastes.sort_by_key(|p| std::cmp::Reverse(p.created_at));

    Ok(Json(WorkspacePasteListResponse { pastes }))
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
        (status = 503, description = "Paste storage unavailable"),
    )
)]
#[post("/api/pastes/<id>/anchor", data = "<body>")]
#[allow(clippy::too_many_arguments)] // Rocket request guards are handler parameters.
async fn anchor_api(
    store: &State<SharedPasteStore>,
    relayer: &State<SharedAnchorRelayer>,
    blocked: &State<BlockedPasteIds>,
    id: String,
    body: Option<Json<AnchorRequest>>,
    onion: OnionAccess,
    _auth: RequireAdminAuth,
    _rate: CreateRateLimit,
) -> Result<Json<AnchorResponse>, (Status, String)> {
    if blocked.contains(&id) {
        return Err((Status::NotFound, "Paste not found".into()));
    }
    let request = body.map(|json| json.into_inner()).unwrap_or_default();

    let paste = match store.get_paste(&id).await {
        Ok(paste) => paste,
        Err(PasteError::NotFound(_)) => return Err((Status::NotFound, "Paste not found".into())),
        Err(PasteError::Expired(_)) => return Err((Status::Gone, "Paste expired".into())),
        Err(PasteError::Persistence(_)) => {
            return Err((
                Status::ServiceUnavailable,
                "Paste storage is temporarily unavailable".into(),
            ));
        }
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
        503 => "service_unavailable",
        _ => "error",
    }
}

/// Map a `(Status, String)` pair from an internal helper into the standardised
/// `(Status, Json<ApiError>)` responder used by JSON API handlers.
fn to_api_err(status: Status, message: String) -> (Status, Json<ApiError>) {
    (status, Json(ApiError::new(status_to_code(status), message)))
}

/// Missing, burned, and expired reads look the same so IDs cannot be fished.
fn public_paste_absence() -> (Status, Json<ApiError>) {
    (
        Status::NotFound,
        Json(ApiError::new("paste_not_found", "Paste not found")),
    )
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
        (status = 404, description = "Paste not found, burned, or expired", body = ApiError),
        (status = 503, description = "Paste storage unavailable", body = ApiError),
    )
)]
#[get("/api/pastes/<id>?<query..>", rank = 1)]
#[allow(clippy::too_many_arguments)] // Rocket request guards are handler parameters.
async fn show_api(
    store: &State<SharedPasteStore>,
    http: &State<WebhookClient>,
    features: &State<FeaturePolicy>,
    blocked: &State<BlockedPasteIds>,
    id: String,
    query: PasteViewQuery,
    key_header: PasteKeyHeader,
    onion: OnionAccess,
    _rate: ReadRateLimit,
) -> Result<Json<PasteViewResponse>, (Status, Json<ApiError>)> {
    if blocked.contains(&id) {
        return Err(to_api_err(Status::NotFound, "Paste not found".to_string()));
    }

    // Header key wins over the query-string key (see handler docs above).
    let key = key_header.0.or_else(|| query.key.clone());

    let paste = match store.get_paste(&id).await {
        Ok(paste) => paste,
        Err(PasteError::NotFound(_)) | Err(PasteError::Expired(_)) => {
            return Err(public_paste_absence());
        }
        Err(PasteError::Persistence(_)) => {
            return Err(to_api_err(
                Status::ServiceUnavailable,
                "Paste storage is temporarily unavailable".to_string(),
            ));
        }
    };

    // Mirror the access controls enforced by the HTML `show` route — the API
    // is the SPA's primary read path and must not bypass them.
    if paste.metadata.tor_access_only && !onion.is_onion() {
        return Err((
            Status::Forbidden,
            Json(ApiError::new(
                "tor_only",
                "This paste is only accessible via its Tor onion address",
            )),
        ));
    }
    if paste.metadata.attestation.is_some() && !features.allow_attestations {
        return Err(to_api_err(
            Status::ServiceUnavailable,
            "This paste uses a feature disabled on this deployment".to_string(),
        ));
    }
    if matches!(&paste.content, StoredContent::Stego { .. }) && !features.allow_stego {
        return Err(to_api_err(
            Status::ServiceUnavailable,
            "This paste uses a feature disabled on this deployment".to_string(),
        ));
    }

    let now = current_timestamp();
    if let Some(lock_state) = evaluate_time_lock(&paste.metadata, now) {
        let (code, message) = match lock_state {
            TimeLockState::TooEarly(_) => ("time_locked", "This paste is not yet available"),
            TimeLockState::TooLate(_) => {
                ("time_lock_elapsed", "This paste's access window has closed")
            }
        };
        return Err((Status::Locked, Json(ApiError::new(code, message))));
    }

    if let Some(requirement) = paste.metadata.attestation.as_ref() {
        match attestation::verify_attestation(requirement, &query, now) {
            AttestationVerdict::Granted => {}
            AttestationVerdict::Prompt { invalid } => {
                let (code, message) = if invalid {
                    (
                        "attestation_invalid",
                        "The provided attestation code is incorrect",
                    )
                } else {
                    (
                        "attestation_required",
                        "This paste requires an attestation code",
                    )
                };
                return Err((Status::Unauthorized, Json(ApiError::new(code, message))));
            }
        }
    }

    let text = match decrypt_content(&paste.content, key.as_deref()) {
        Ok(text) => text,
        Err(DecryptError::MissingKey) => {
            return Err((
                Status::Unauthorized,
                Json(ApiError::new(
                    "key_required",
                    "This paste requires an encryption key",
                )),
            ));
        }
        Err(DecryptError::InvalidKey) => {
            return Err((
                Status::Forbidden,
                Json(ApiError::new(
                    "invalid_key",
                    "The provided encryption key is incorrect",
                )),
            ));
        }
    };

    // Burn-after-reading is serialized within this process. A durable deletion
    // failure is fail-closed; cross-instance reads still require an atomic
    // shared-store consume primitive to provide an exactly-once guarantee.
    if paste.burn_after_reading {
        let webhook_config = features
            .allow_webhooks
            .then(|| paste.metadata.webhook.clone())
            .flatten();
        let mut events_to_fire = Vec::new();
        match store.delete_paste(&id).await {
            Ok(true) => {
                if let Some(config) = webhook_config.clone() {
                    events_to_fire.push((config, WebhookEvent::Viewed));
                }
                if let Some(config) = webhook_config {
                    events_to_fire.push((config, WebhookEvent::Consumed));
                }
            }
            Ok(false) => {
                return Err((
                    Status::Gone,
                    Json(ApiError::new(
                        "paste_consumed",
                        "This paste was already consumed",
                    )),
                ));
            }
            Err(_) => {
                rocket::error!("burn_delete_failed");
                return Err((
                    Status::ServiceUnavailable,
                    Json(ApiError::new(
                        "delete_failed",
                        "The paste could not be consumed safely; try again later",
                    )),
                ));
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
    }

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

    let time_lock = match (paste.not_before, paste.not_after) {
        (None, None) => None,
        (not_before, not_after) => Some(PasteTimeLockInfo {
            not_before,
            not_after,
        }),
    };

    Ok(Json(PasteViewResponse {
        id,
        format: paste.format,
        content: text,
        created_at: paste.created_at,
        expires_at: paste.expires_at,
        burn_after_reading: paste.burn_after_reading,
        encryption,
        tor_access_only: paste.metadata.tor_access_only,
        is_live: paste.is_live,
        time_lock,
    }))
}

#[utoipa::path(
    post,
    path = "/",
    request_body = CreatePasteApiSchema,
    params(("X-CopyPaste-Write-Token" = Option<String>, Header, description = "Service admission credential; Authorization may carry optional user-session identity")),
    responses(
        (status = 200, description = "Paste created", body = String),
        (status = 400, description = "Invalid paste request"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error"),
    )
)]
#[post("/", data = "<body>")]
async fn create(
    _rate: CreateRateLimit,
    auth: RequireWriteAuth,
    store: &State<SharedPasteStore>,
    features: &State<FeaturePolicy>,
    body: Json<CreatePasteRequest>,
    onion: OnionAccess,
) -> Result<String, (Status, String)> {
    let mut body = body.into_inner();
    body.owner_pubkey_hash = authenticated_owner(&auth);
    let created = create_paste_internal(store.inner(), body, &onion, features.inner()).await?;
    // Preserve the legacy server-rendered route for non-API clients. The JSON
    // API returns the React share route (`/p/{id}`).
    Ok(format!("/{}", created.id))
}

#[utoipa::path(
    post,
    path = "/api/pastes",
    request_body = CreatePasteApiSchema,
    params(("X-CopyPaste-Write-Token" = Option<String>, Header, description = "Service admission credential; Authorization may carry optional user-session identity")),
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
    _rate: CreateRateLimit,
    auth: RequireWriteAuth,
    store: &State<SharedPasteStore>,
    features: &State<FeaturePolicy>,
    body: Result<Json<CreatePasteRequest>, rocket::serde::json::Error<'_>>,
    onion: OnionAccess,
) -> Result<Json<CreatePasteResponse>, (Status, Json<ApiError>)> {
    let body = match body {
        Ok(json) => json,
        Err(_) => {
            return Err((
                Status::BadRequest,
                Json(ApiError::new("invalid_request", "Invalid JSON request")),
            ));
        }
    };

    let mut body = body.into_inner();
    // Ownership is derived exclusively from a validated login session. Client
    // claims are ignored for anonymous, static-token, and API-key principals.
    body.owner_pubkey_hash = authenticated_owner(&auth);

    let created = create_paste_internal(store.inner(), body, &onion, features.inner())
        .await
        .map_err(|(s, msg)| to_api_err(s, msg))?;
    Ok(Json(created))
}

fn authenticated_owner(auth: &RequireWriteAuth) -> Option<String> {
    match &auth.0 {
        WritePrincipal::UserSession { pubkey_hash } => Some(pubkey_hash.clone()),
        WritePrincipal::Anonymous | WritePrincipal::StaticToken | WritePrincipal::ApiKey(_) => None,
    }
}

#[utoipa::path(
    get,
    path = "/p/{id}",
    params(
        ("id" = String, description = "Paste identifier"),
        ("X-Paste-Key" = Option<String>, Header, description = "Decryption key (takes precedence over ?key=)"),
    ),
    responses(
        (status = 200, description = "Paste rendered as HTML", content_type = "text/html"),
        (status = 401, description = "Key required"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Paste not found"),
        (status = 503, description = "Paste storage or required feature unavailable"),
    )
)]
#[get("/p/<id>?<query..>")]
#[allow(clippy::too_many_arguments)] // Rocket request guards are handler parameters.
async fn show_share(
    store: &State<SharedPasteStore>,
    http: &State<WebhookClient>,
    features: &State<FeaturePolicy>,
    blocked: &State<BlockedPasteIds>,
    id: String,
    query: PasteViewQuery,
    key_header: PasteKeyHeader,
    onion: OnionAccess,
    _rate: ReadRateLimit,
) -> Result<content::RawHtml<String>, Status> {
    show_html_core(store, http, features, blocked, id, query, key_header, onion).await
}

/// Legacy server-rendered path retained for existing links and CLI clients.
#[get("/<id>?<query..>")]
#[allow(clippy::too_many_arguments)] // Rocket request guards are handler parameters.
async fn show(
    store: &State<SharedPasteStore>,
    http: &State<WebhookClient>,
    features: &State<FeaturePolicy>,
    blocked: &State<BlockedPasteIds>,
    id: String,
    query: PasteViewQuery,
    key_header: PasteKeyHeader,
    onion: OnionAccess,
    _rate: ReadRateLimit,
) -> Result<content::RawHtml<String>, Status> {
    show_html_core(store, http, features, blocked, id, query, key_header, onion).await
}

#[allow(clippy::too_many_arguments)] // Shared policy core keeps both HTML routes identical.
async fn show_html_core(
    store: &State<SharedPasteStore>,
    http: &State<WebhookClient>,
    features: &State<FeaturePolicy>,
    blocked: &State<BlockedPasteIds>,
    id: String,
    query: PasteViewQuery,
    key_header: PasteKeyHeader,
    onion: OnionAccess,
) -> Result<content::RawHtml<String>, Status> {
    if blocked.contains(&id) {
        return Err(Status::NotFound);
    }
    match store.get_paste(&id).await {
        Ok(paste) => {
            if paste.metadata.tor_access_only && !onion.is_onion() {
                return Err(Status::Forbidden);
            }
            if paste.metadata.attestation.is_some() && !features.allow_attestations {
                return Err(Status::ServiceUnavailable);
            }
            if matches!(&paste.content, StoredContent::Stego { .. }) && !features.allow_stego {
                return Err(Status::ServiceUnavailable);
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

            let decryption_key = key_header.0.as_deref().or(query.key.as_deref());
            match decrypt_content(&paste.content, decryption_key) {
                Ok(text) => {
                    let webhook_config = features
                        .allow_webhooks
                        .then(|| paste.metadata.webhook.clone())
                        .flatten();
                    let mut events_to_fire = Vec::new();

                    if paste.burn_after_reading {
                        match store.delete_paste(&id).await {
                            Ok(true) => {
                                if let Some(config) = webhook_config.clone() {
                                    events_to_fire.push((config.clone(), WebhookEvent::Viewed));
                                }
                                if let Some(config) = webhook_config.clone() {
                                    events_to_fire.push((config, WebhookEvent::Consumed));
                                }
                            }
                            Ok(false) => return Err(Status::Gone),
                            Err(_) => {
                                rocket::error!("burn_delete_failed");
                                return Err(Status::ServiceUnavailable);
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

                    // Existing bundle pointers are intentionally withheld. The
                    // legacy implementation never created valid child records,
                    // and rendering those IDs can bypass moderation filtering.
                    let mut public_metadata = paste.metadata.clone();
                    public_metadata.bundle = None;
                    public_metadata.bundle_parent = None;
                    public_metadata.bundle_label = None;
                    public_metadata.persistence = None;
                    if !features.allow_webhooks {
                        public_metadata.webhook = None;
                    }
                    let view = StoredPasteView {
                        content: &paste.content,
                        format: paste.format,
                        created_at: paste.created_at,
                        expires_at: paste.expires_at,
                        burn_after_reading: paste.burn_after_reading,
                        metadata: &public_metadata,
                    };

                    Ok(content::RawHtml(render_paste_view(&id, &view, &text, None)))
                }
                Err(DecryptError::MissingKey) => Ok(content::RawHtml(render_key_prompt(&id))),
                Err(DecryptError::InvalidKey) => Ok(content::RawHtml(render_invalid_key(&id))),
            }
        }
        Err(PasteError::NotFound(_) | PasteError::Expired(_)) => Err(Status::NotFound),
        Err(PasteError::Persistence(_)) => Err(Status::ServiceUnavailable),
    }
}

#[get("/raw/<id>?<query..>")]
#[allow(clippy::too_many_arguments)] // Rocket request guards are handler parameters.
async fn show_raw(
    store: &State<SharedPasteStore>,
    http: &State<WebhookClient>,
    features: &State<FeaturePolicy>,
    blocked: &State<BlockedPasteIds>,
    id: String,
    query: PasteViewQuery,
    key_header: PasteKeyHeader,
    onion: OnionAccess,
    _rate: ReadRateLimit,
) -> Result<content::RawText<String>, Status> {
    if blocked.contains(&id) {
        return Err(Status::NotFound);
    }
    match store.get_paste(&id).await {
        Ok(paste) => {
            if paste.metadata.tor_access_only && !onion.is_onion() {
                return Err(Status::Forbidden);
            }
            if paste.metadata.attestation.is_some() && !features.allow_attestations {
                return Err(Status::ServiceUnavailable);
            }
            if matches!(&paste.content, StoredContent::Stego { .. }) && !features.allow_stego {
                return Err(Status::ServiceUnavailable);
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

            let decryption_key = key_header.0.as_deref().or(query.key.as_deref());
            match decrypt_content(&paste.content, decryption_key) {
                Ok(text) => {
                    if paste.burn_after_reading {
                        let webhook_config = features
                            .allow_webhooks
                            .then(|| paste.metadata.webhook.clone())
                            .flatten();
                        match store.delete_paste(&id).await {
                            Ok(true) => {
                                if let Some(config) = webhook_config.clone() {
                                    trigger_webhook(
                                        http.inner().0.clone(),
                                        config,
                                        WebhookEvent::Viewed,
                                        &id,
                                        paste.metadata.bundle_label.clone(),
                                    );
                                }
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
                            Ok(false) => return Err(Status::Gone),
                            Err(_) => {
                                rocket::error!("burn_delete_failed");
                                return Err(Status::ServiceUnavailable);
                            }
                        }
                    }

                    Ok(content::RawText(text))
                }
                Err(DecryptError::MissingKey) => Err(Status::Unauthorized),
                Err(DecryptError::InvalidKey) => Err(Status::Forbidden),
            }
        }
        Err(PasteError::NotFound(_) | PasteError::Expired(_)) => Err(Status::NotFound),
        Err(PasteError::Persistence(_)) => Err(Status::ServiceUnavailable),
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
    match request {
        PersistenceRequest::Memory => Ok(PersistenceLocator::Memory),
        PersistenceRequest::Vault { .. } | PersistenceRequest::S3 { .. } => Err((
            Status::BadRequest,
            "Client-selected external persistence is disabled; storage is controlled by the deployment"
                .into(),
        )),
    }
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
const MAX_ENCRYPTION_KEY_BYTES: usize = 1024;

fn validate_encryption_request(
    encryption: Option<&super::models::EncryptionRequest>,
) -> Result<(), (Status, String)> {
    if let Some(enc) = encryption.filter(|enc| enc.algorithm != EncryptionAlgorithm::None) {
        if enc.key.trim().is_empty() {
            return Err((
                Status::BadRequest,
                "Encryption key cannot be empty".to_string(),
            ));
        }
        if enc.key.len() > MAX_ENCRYPTION_KEY_BYTES {
            return Err((
                Status::PayloadTooLarge,
                format!("Encryption key must not exceed {MAX_ENCRYPTION_KEY_BYTES} bytes"),
            ));
        }
    }
    Ok(())
}

async fn resolve_content(
    text: String,
    encryption: Option<&super::models::EncryptionRequest>,
) -> Result<StoredContent, (Status, String)> {
    // Validate attacker-controlled key material before entering Argon2/HKDF or
    // any encryption implementation. This bounds both CPU and memory inputs.
    validate_encryption_request(encryption)?;
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

fn expiration_from_retention(
    retention_minutes: Option<u64>,
    max_retention_minutes: Option<u64>,
    created_at: i64,
) -> Result<Option<i64>, (Status, String)> {
    let Some(minutes) = retention_minutes else {
        return Ok(None);
    };

    if max_retention_minutes.is_some_and(|max| minutes > max) {
        return Err((
            Status::BadRequest,
            "retention_minutes exceeds the configured maximum".to_string(),
        ));
    }

    let minutes = i64::try_from(minutes).map_err(|_| {
        (
            Status::BadRequest,
            "retention_minutes is outside the supported range".to_string(),
        )
    })?;
    let seconds = minutes.checked_mul(60).ok_or_else(|| {
        (
            Status::BadRequest,
            "retention_minutes is outside the supported range".to_string(),
        )
    })?;
    let expires_at = created_at.checked_add(seconds).ok_or_else(|| {
        (
            Status::BadRequest,
            "retention_minutes is outside the supported range".to_string(),
        )
    })?;

    Ok(Some(expires_at))
}

async fn create_paste_internal(
    store: &SharedPasteStore,
    mut body: CreatePasteRequest,
    _onion: &OnionAccess,
    features: &FeaturePolicy,
) -> Result<CreatePasteResponse, (Status, String)> {
    // Validate content
    if body.content.trim().is_empty() {
        return Err((Status::BadRequest, "Content cannot be empty".into()));
    }
    let max_paste_size = std::env::var("COPYPASTE_MAX_PASTE_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1_048_576); // 1 MiB
    if body.content.len() > max_paste_size {
        return Err((
            Status::PayloadTooLarge,
            "Content exceeds maximum paste size".into(),
        ));
    }

    if body.webhook.is_some() && !features.allow_webhooks {
        return Err((
            Status::Forbidden,
            "Webhooks are disabled on this deployment".into(),
        ));
    }
    if body.attestation.is_some() && !features.allow_attestations {
        return Err((
            Status::Forbidden,
            "Attestations are disabled on this deployment".into(),
        ));
    }
    if body.bundle.is_some() {
        return Err((
            Status::Forbidden,
            "Bundles are disabled on this deployment".into(),
        ));
    }
    if body.stego.is_some() && !features.allow_stego {
        return Err((
            Status::Forbidden,
            "Steganography is disabled on this deployment".into(),
        ));
    }
    if matches!(body.stego.as_ref(), Some(StegoRequest::Uploaded { .. }))
        && !features.allow_uploaded_stego
    {
        return Err((
            Status::Forbidden,
            "Uploaded steganography carriers are disabled on this deployment".into(),
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

    // Reject unsupported storage claims before any cryptographic work.
    let requested_persistence = body
        .persistence
        .as_ref()
        .map(persistence_locator_from_request)
        .transpose()?;

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
    metadata.persistence = requested_persistence;

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
                    .map_err(|e| (Status::BadRequest, format!("Invalid data URI: {e}")))?;
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
    let created_at = current_timestamp();
    let expires_at = expiration_from_retention(
        retention_minutes,
        env_minutes("COPYPASTE_RETENTION_MAX_MINUTES"),
        created_at,
    )?;

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
        created_at,
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
    let id = store.create_paste(paste).await.map_err(|_| {
        (
            Status::ServiceUnavailable,
            "Paste storage is temporarily unavailable".to_string(),
        )
    })?;
    let path = format!("/p/{id}");

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
/// The stored hash is SHA-256(token) as lowercase hex. Comparison is against
/// the raw 32-byte digest so encoding differences cannot leak via length.
fn verify_owner_token(paste: &StoredPaste, token: Option<&str>) -> Result<(), (Status, String)> {
    let expected = paste.owner_token_hash.as_deref().ok_or((
        Status::Conflict,
        "This paste has no ownership token and cannot be modified".to_string(),
    ))?;
    let token = token.ok_or((
        Status::Unauthorized,
        "Ownership token required (Authorization: Bearer <token>)".to_string(),
    ))?;
    let actual: [u8; 32] = Sha256::digest(token.as_bytes()).into();
    let mut expected_bytes = [0u8; 32];
    let decoded = hex::decode(expected).ok();
    let valid_len = decoded.as_ref().is_some_and(|bytes| bytes.len() == 32);
    if let Some(bytes) = decoded.as_deref() {
        if bytes.len() == 32 {
            expected_bytes.copy_from_slice(bytes);
        }
    }
    if valid_len && bool::from(actual.ct_eq(&expected_bytes)) {
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
        Err(PasteError::Persistence(_)) => Err((
            Status::ServiceUnavailable,
            "Paste storage is temporarily unavailable".to_string(),
        )),
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
    params(
        ("id" = String, Path, description = "Paste identifier"),
        ("X-CopyPaste-Write-Token" = Option<String>, Header, description = "Service write credential required on closed deployments; ownership token remains in Authorization"),
    ),
    responses(
        (status = 200, description = "Paste updated", body = UpdatePasteResponse),
        (status = 401, description = "Ownership token required", body = ApiError),
        (status = 403, description = "Invalid ownership token", body = ApiError),
        (status = 404, description = "Paste not found", body = ApiError),
        (status = 409, description = "Paste is not live", body = ApiError),
        (status = 410, description = "Paste expired", body = ApiError),
        (status = 429, description = "Mutation rate limit exceeded", body = ApiError),
        (status = 503, description = "Durable mutation could not be confirmed", body = ApiError),
    )
)]
#[put("/api/pastes/<id>", data = "<body>")]
async fn update_api(
    _rate: CreateRateLimit,
    _admission: RequireMutationWriteAuth,
    store: &State<SharedPasteStore>,
    blocked: &State<BlockedPasteIds>,
    id: String,
    body: Json<UpdatePasteRequest>,
    token: OwnerBearerToken,
) -> Result<Json<UpdatePasteResponse>, (Status, Json<ApiError>)> {
    if blocked.contains(&id) {
        return Err(to_api_err(Status::NotFound, "Paste not found".to_string()));
    }
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
        .unwrap_or(1_048_576);
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
            PasteMutationError::NotFound(_) => {
                to_api_err(Status::NotFound, format!("Paste '{id}' not found"))
            }
            PasteMutationError::Expired(_) => {
                to_api_err(Status::Gone, format!("Paste '{id}' expired"))
            }
            PasteMutationError::Finalized(_) => to_api_err(
                Status::Conflict,
                "Paste has been finalized and can no longer be updated".to_string(),
            ),
            PasteMutationError::Persistence(_) => to_api_err(
                Status::ServiceUnavailable,
                "Durable paste update could not be confirmed".to_string(),
            ),
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
    params(
        ("id" = String, Path, description = "Paste identifier"),
        ("X-CopyPaste-Write-Token" = Option<String>, Header, description = "Service write credential required on closed deployments; ownership token remains in Authorization"),
    ),
    responses(
        (status = 200, description = "Paste finalized", body = FinalizePasteResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 401, description = "Ownership token required", body = ApiError),
        (status = 403, description = "Invalid ownership token", body = ApiError),
        (status = 404, description = "Paste not found", body = ApiError),
        (status = 410, description = "Paste expired", body = ApiError),
        (status = 429, description = "Mutation rate limit exceeded"),
        (status = 503, description = "Durable mutation could not be confirmed", body = ApiError),
    )
)]
#[patch("/api/pastes/<id>/finalize", data = "<body>")]
async fn finalize_api(
    _rate: CreateRateLimit,
    _admission: RequireMutationWriteAuth,
    store: &State<SharedPasteStore>,
    blocked: &State<BlockedPasteIds>,
    id: String,
    body: Option<Json<FinalizePasteRequest>>,
    token: OwnerBearerToken,
) -> Result<Json<FinalizePasteResponse>, (Status, Json<ApiError>)> {
    if blocked.contains(&id) {
        return Err(to_api_err(Status::NotFound, "Paste not found".to_string()));
    }
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
            PasteMutationError::NotFound(_) => {
                to_api_err(Status::NotFound, format!("Paste '{id}' not found"))
            }
            PasteMutationError::Expired(_) => {
                to_api_err(Status::Gone, format!("Paste '{id}' expired"))
            }
            PasteMutationError::Finalized(_) => {
                to_api_err(Status::Conflict, "Paste is already finalized".to_string())
            }
            PasteMutationError::Persistence(_) => to_api_err(
                Status::ServiceUnavailable,
                "Durable paste finalization could not be confirmed".to_string(),
            ),
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
    if !key_store.is_enabled() {
        return Err(to_api_err(
            Status::ServiceUnavailable,
            "Dynamic API-key management is disabled on this deployment".to_string(),
        ));
    }
    let body = body.into_inner();
    let store = key_store.inner().clone();
    let (key_info, plaintext_key) = tokio::task::spawn_blocking(move || {
        store.create_key(&body.name, body.scope, body.expires_at)
    })
    .await
    .map_err(|_| {
        to_api_err(
            Status::ServiceUnavailable,
            "API-key store unavailable".to_string(),
        )
    })?
    .map_err(|_| {
        to_api_err(
            Status::ServiceUnavailable,
            "API-key store unavailable".to_string(),
        )
    })?;

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
    if !key_store.is_enabled() {
        return Err(to_api_err(
            Status::ServiceUnavailable,
            "Dynamic API-key management is disabled on this deployment".to_string(),
        ));
    }
    let store = key_store.inner().clone();
    let keys = tokio::task::spawn_blocking(move || store.list_keys())
        .await
        .map_err(|_| {
            to_api_err(
                Status::ServiceUnavailable,
                "API-key store unavailable".to_string(),
            )
        })?
        .map_err(|_| {
            to_api_err(
                Status::ServiceUnavailable,
                "API-key store unavailable".to_string(),
            )
        })?;

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
    if !key_store.is_enabled() {
        return Err(to_api_err(
            Status::ServiceUnavailable,
            "Dynamic API-key management is disabled on this deployment".to_string(),
        ));
    }
    let store = key_store.inner().clone();
    let revoked = tokio::task::spawn_blocking(move || store.revoke_key(&id))
        .await
        .map_err(|_| {
            to_api_err(
                Status::ServiceUnavailable,
                "API-key store unavailable".to_string(),
            )
        })?
        .map_err(|_| {
            to_api_err(
                Status::ServiceUnavailable,
                "API-key store unavailable".to_string(),
            )
        })?;

    Ok(Json(RevokeApiKeyResponse { revoked }))
}

fn validate_paste_id(id: &str) -> Result<(), (Status, Json<ApiError>)> {
    if !is_valid_paste_id(id) {
        return Err(to_api_err(
            Status::BadRequest,
            "Invalid paste identifier".to_string(),
        ));
    }
    Ok(())
}

fn moderation_content_summary(content: &StoredContent) -> (bool, EncryptionAlgorithm, usize) {
    match content {
        StoredContent::Plain { text } => (false, EncryptionAlgorithm::None, text.len()),
        StoredContent::Encrypted {
            algorithm,
            ciphertext,
            nonce,
            salt,
        } => (
            true,
            *algorithm,
            ciphertext
                .len()
                .saturating_add(nonce.len())
                .saturating_add(salt.len()),
        ),
        StoredContent::Stego {
            algorithm,
            ciphertext,
            nonce,
            salt,
            carrier_mime,
            carrier_image,
            payload_digest,
        } => (
            true,
            *algorithm,
            ciphertext
                .len()
                .saturating_add(nonce.len())
                .saturating_add(salt.len())
                .saturating_add(carrier_mime.len())
                .saturating_add(carrier_image.len())
                .saturating_add(payload_digest.len()),
        ),
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/pastes/{id}",
    params(("id" = String, description = "Exact paste identifier")),
    responses(
        (status = 200, description = "Metadata-only moderation view", body = AdminPasteMetadataResponse),
        (status = 400, description = "Invalid identifier", body = ApiError),
        (status = 401, description = "Authentication required", body = ApiError),
        (status = 403, description = "Admin scope required", body = ApiError),
        (status = 404, description = "Paste not found", body = ApiError),
        (status = 410, description = "Paste expired", body = ApiError),
        (status = 503, description = "Paste storage unavailable", body = ApiError),
    )
)]
#[get("/api/admin/pastes/<id>")]
async fn admin_get_paste_api(
    store: &State<SharedPasteStore>,
    id: String,
    auth: RequireAdminAuth,
) -> Result<Json<AdminPasteMetadataResponse>, (Status, Json<ApiError>)> {
    if let Err(error) = validate_paste_id(&id) {
        rocket::info!(
            "admin_audit key_id={} action=inspect outcome=invalid_id",
            auth.0.key_id
        );
        return Err(error);
    }
    let paste = match store.get_paste(&id).await {
        Ok(paste) => paste,
        Err(PasteError::NotFound(_)) => {
            rocket::info!(
                "admin_audit key_id={} action=inspect outcome=not_found",
                auth.0.key_id
            );
            return Err(to_api_err(Status::NotFound, "Paste not found".to_string()));
        }
        Err(PasteError::Expired(_)) => {
            rocket::info!(
                "admin_audit key_id={} action=inspect outcome=expired",
                auth.0.key_id
            );
            return Err(to_api_err(Status::Gone, "Paste expired".to_string()));
        }
        Err(PasteError::Persistence(_)) => {
            rocket::error!(
                "admin_audit key_id={} action=inspect outcome=storage_error",
                auth.0.key_id
            );
            return Err(to_api_err(
                Status::ServiceUnavailable,
                "Paste storage is temporarily unavailable".to_string(),
            ));
        }
    };

    let (encrypted, encryption_algorithm, approximate_stored_bytes) =
        moderation_content_summary(&paste.content);
    let response = AdminPasteMetadataResponse {
        id: id.clone(),
        format: paste.format,
        created_at: paste.created_at,
        expires_at: paste.expires_at,
        burn_after_reading: paste.burn_after_reading,
        encrypted,
        encryption_algorithm,
        approximate_stored_bytes,
        tor_access_only: paste.metadata.tor_access_only,
        has_attestation: paste.metadata.attestation.is_some(),
        has_webhook: paste.metadata.webhook.is_some(),
        has_workspace: paste.metadata.workspace.is_some(),
    };
    rocket::info!(
        "admin_audit key_id={} action=inspect outcome=found",
        auth.0.key_id
    );
    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/api/admin/pastes/{id}",
    params(("id" = String, description = "Exact paste identifier")),
    responses(
        (status = 200, description = "Deletion or absence acknowledged by this instance and its configured backing store", body = AdminDeletePasteResponse),
        (status = 400, description = "Invalid identifier", body = ApiError),
        (status = 401, description = "Authentication required", body = ApiError),
        (status = 403, description = "Admin scope required", body = ApiError),
        (status = 503, description = "Deletion could not be confirmed by this instance", body = ApiError),
    )
)]
#[delete("/api/admin/pastes/<id>")]
async fn admin_delete_paste_api(
    store: &State<SharedPasteStore>,
    id: String,
    auth: RequireAdminAuth,
) -> Result<Json<AdminDeletePasteResponse>, (Status, Json<ApiError>)> {
    if let Err(error) = validate_paste_id(&id) {
        rocket::info!(
            "admin_audit key_id={} action=delete outcome=invalid_id",
            auth.0.key_id
        );
        return Err(error);
    }

    // Delete directly rather than loading the record first. This avoids
    // bringing suspected content into an operator request path and ensures a
    // persistence outage is reported as 503 instead of being misreported as
    // "not found" by the cache/load compatibility layer.
    match store.delete_paste(&id).await {
        Ok(_) => {
            rocket::info!(
                "admin_audit key_id={} action=delete outcome=deleted_or_absent",
                auth.0.key_id
            );
            Ok(Json(AdminDeletePasteResponse { id, deleted: true }))
        }
        Err(_) => {
            rocket::error!(
                "admin_audit key_id={} action=delete outcome=storage_error",
                auth.0.key_id
            );
            Err((
                Status::ServiceUnavailable,
                Json(ApiError::new(
                    "delete_failed",
                    "Deletion could not be confirmed by this instance",
                )),
            ))
        }
    }
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
        let challenge_response = client.get("/api/auth/challenge").dispatch();
        assert_eq!(challenge_response.status(), Status::Ok);
        let challenge_json: serde_json::Value = serde_json::from_str(
            &challenge_response
                .into_string()
                .expect("challenge response body"),
        )
        .expect("challenge response JSON");
        let challenge = challenge_json["challenge"]
            .as_str()
            .expect("challenge string");
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

    fn mutation_write_token(token: &str) -> rocket::http::Header<'static> {
        rocket::http::Header::new(
            super::super::api_keys::MUTATION_WRITE_TOKEN_HEADER,
            token.to_string(),
        )
    }

    fn build_rocket_with_features(
        store: SharedPasteStore,
        features: FeaturePolicy,
    ) -> Rocket<Build> {
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::default(),
            features,
            BlockedPasteIds::default(),
            None,
        )
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
    fn encrypted_content_requires_a_bounded_nonblank_key() {
        let blank = super::super::models::EncryptionRequest {
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            key: " \t\n".to_string(),
        };
        let blank_error = validate_encryption_request(Some(&blank)).unwrap_err();
        assert_eq!(blank_error.0, Status::BadRequest);

        // String::len() is a UTF-8 byte count. This is only 513 characters but
        // 1,026 bytes and must be rejected before cryptographic processing.
        let oversized = super::super::models::EncryptionRequest {
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            key: "é".repeat(513),
        };
        assert_eq!(oversized.key.chars().count(), 513);
        assert_eq!(oversized.key.len(), 1026);
        let oversized_error = validate_encryption_request(Some(&oversized)).unwrap_err();
        assert_eq!(oversized_error.0, Status::PayloadTooLarge);

        let boundary = super::super::models::EncryptionRequest {
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            key: "k".repeat(MAX_ENCRYPTION_KEY_BYTES),
        };
        assert!(validate_encryption_request(Some(&boundary)).is_ok());

        let plaintext = super::super::models::EncryptionRequest {
            algorithm: EncryptionAlgorithm::None,
            key: String::new(),
        };
        assert!(validate_encryption_request(Some(&plaintext)).is_ok());
    }

    #[test]
    fn create_api_rejects_invalid_encryption_keys_without_storing_a_paste() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let client = Client::tracked(build_rocket(Arc::clone(&store))).expect("client");

        let blank = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "secret",
                    "encryption": {"algorithm": "aes256_gcm", "key": "  \n"}
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(blank.status(), Status::BadRequest);

        let oversized = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "secret",
                    "encryption": {
                        "algorithm": "aes256_gcm",
                        "key": "é".repeat(513)
                    }
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(oversized.status(), Status::PayloadTooLarge);

        let ids = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.get_all_paste_ids());
        assert!(ids.is_empty());
    }

    #[test]
    fn persistence_locator_validates_inputs() {
        let memory = persistence_locator_from_request(&PersistenceRequest::Memory).unwrap();
        matches!(memory, PersistenceLocator::Memory);

        let vault = persistence_locator_from_request(&PersistenceRequest::Vault {
            key_path: "secret/path".into(),
        })
        .expect_err("client-selected Vault storage must be rejected");
        assert_eq!(vault.0, Status::BadRequest);

        let s3 = persistence_locator_from_request(&PersistenceRequest::S3 {
            bucket: "bucket".into(),
            prefix: Some("prefix".into()),
        })
        .expect_err("client-selected S3 storage must be rejected");
        assert_eq!(s3.0, Status::BadRequest);
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
    fn risky_optional_features_are_disabled_by_default() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let client = Client::tracked(build_rocket_with_features(store, FeaturePolicy::default()))
            .expect("client");

        let webhook = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "webhook",
                    "webhook": {"url": "https://example.com/hook"}
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(webhook.status(), Status::Forbidden);

        let uploaded_stego = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "uploaded",
                    "stego": {
                        "mode": "uploaded",
                        "data_uri": "data:image/png;base64,AA=="
                    }
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(uploaded_stego.status(), Status::Forbidden);

        let attestation = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "attested",
                    "attestation": {"kind": "shared_secret", "secret": "secret"}
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(attestation.status(), Status::Forbidden);
    }

    #[test]
    fn stored_attestation_pastes_fail_closed_when_feature_is_disabled() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let enabled = Client::tracked(build_rocket_with_features(
            Arc::clone(&store),
            FeaturePolicy::default().with_attestations(true),
        ))
        .expect("enabled client");
        let response = enabled
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "legacy attested paste",
                    "attestation": {"kind": "shared_secret", "secret": "secret"}
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&response.into_string().unwrap()).unwrap();
        drop(enabled);

        let disabled = Client::tracked(build_rocket_with_features(store, FeaturePolicy::default()))
            .expect("disabled client");
        assert_eq!(
            disabled
                .get(format!("/api/pastes/{}", created.id))
                .dispatch()
                .status(),
            Status::ServiceUnavailable
        );
    }

    #[test]
    fn show_route_triggers_burn_after_reading_flow() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "payload",
            "format": "plain_text",
            "burn_after_reading": true
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
    fn show_api_triggers_burn_after_reading_flow() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "api burn payload",
            "format": "plain_text",
            "burn_after_reading": true
        });

        let response = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        let body = response.into_string().expect("json body");
        let parsed: CreatePasteResponse = serde_json::from_str(&body).expect("parse");

        let api_path = format!("/api/pastes/{}", parsed.id);
        let first = client.get(&api_path).dispatch();
        assert_eq!(first.status(), Status::Ok);
        let view: serde_json::Value =
            serde_json::from_str(&first.into_string().unwrap()).expect("view json");
        assert_eq!(view["content"], "api burn payload");

        // The successful API read consumed the paste.
        let second = client.get(&api_path).dispatch();
        assert_eq!(second.status(), Status::NotFound);
    }

    #[test]
    fn show_api_does_not_burn_when_key_is_missing_or_wrong() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "secret",
            "format": "plain_text",
            "burn_after_reading": true,
            "encryption": { "algorithm": "aes256_gcm", "key": "correct-horse" }
        });

        let response = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        let parsed: CreatePasteResponse =
            serde_json::from_str(&response.into_string().unwrap()).expect("parse");
        let api_path = format!("/api/pastes/{}", parsed.id);

        // Missing key and wrong key must NOT consume the paste.
        let missing = client.get(&api_path).dispatch();
        assert_eq!(missing.status(), Status::Unauthorized);
        let wrong = client.get(format!("{api_path}?key=nope")).dispatch();
        assert_eq!(wrong.status(), Status::Forbidden);

        // Correct key reads and consumes it.
        let good = client
            .get(format!("{api_path}?key=correct-horse"))
            .dispatch();
        assert_eq!(good.status(), Status::Ok);
        let gone = client
            .get(format!("{api_path}?key=correct-horse"))
            .dispatch();
        assert_eq!(gone.status(), Status::NotFound);
    }

    #[test]
    fn show_api_enforces_time_lock_and_attestation() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket =
            build_rocket_with_features(store, FeaturePolicy::default().with_attestations(true));
        let client = Client::tracked(rocket).expect("client");

        // Time-locked paste: not available yet via the API.
        let locked = json!({
            "content": "future",
            "format": "plain_text",
            "time_lock": { "not_before": (current_timestamp() + 3600).to_string() }
        });
        let response = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(locked.to_string())
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        let parsed: CreatePasteResponse =
            serde_json::from_str(&response.into_string().unwrap()).expect("parse");
        let res = client.get(format!("/api/pastes/{}", parsed.id)).dispatch();
        assert_eq!(res.status(), Status::Locked);

        // Attestation-gated paste: requires the shared secret via the API.
        let gated = json!({
            "content": "gated",
            "format": "plain_text",
            "attestation": { "kind": "shared_secret", "secret": "open-sesame" }
        });
        let response = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(gated.to_string())
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        let parsed: CreatePasteResponse =
            serde_json::from_str(&response.into_string().unwrap()).expect("parse");
        let no_code = client.get(format!("/api/pastes/{}", parsed.id)).dispatch();
        assert_eq!(no_code.status(), Status::Unauthorized);
        let with_code = client
            .get(format!("/api/pastes/{}?attest=open-sesame", parsed.id))
            .dispatch();
        assert_eq!(with_code.status(), Status::Ok);
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
        assert_eq!(parsed.path, format!("/p/{}", parsed.id));
        assert_eq!(parsed.path, parsed.shareable_url);

        // Fetch the paste to ensure it was stored.
        let get_response = client.get(&parsed.path).dispatch();
        assert_eq!(get_response.status(), Status::Ok);
    }

    #[test]
    fn share_and_legacy_html_routes_use_the_same_access_policy() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let client = Client::tracked(build_rocket(store)).expect("client");
        let response = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "same guarded content",
                    "encryption": {"algorithm": "aes256_gcm", "key": "route-key"}
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&response.into_string().unwrap()).unwrap();
        let legacy_path = format!("/{}", created.id);
        assert_eq!(created.path, format!("/p/{}", created.id));

        for path in [&created.path, &legacy_path] {
            let prompt = client.get(path).dispatch();
            assert_eq!(prompt.status(), Status::Ok);
            let prompt = prompt.into_string().expect("key prompt");
            assert!(prompt.contains("Encryption key"));
            assert!(!prompt.contains("same guarded content"));

            let content = client.get(format!("{path}?key=route-key")).dispatch();
            assert_eq!(content.status(), Status::Ok);
            assert!(content
                .into_string()
                .expect("guarded content")
                .contains("same guarded content"));
        }
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
    fn aggregate_and_user_listing_routes_enforce_read_limits() {
        fn limited_client() -> Client {
            let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
            let api_key_store: SharedApiKeyStore = Arc::new(SqliteApiKeyStore::disabled());
            Client::tracked(build_rocket_with_components(
                store,
                api_key_store,
                PasteRateLimiter::new(None, Some(1)),
                StaticAuthTokens::default(),
                FeaturePolicy::default(),
                BlockedPasteIds::default(),
                None,
            ))
            .expect("client")
        }

        let stats_client = limited_client();
        assert_eq!(
            stats_client.get("/api/stats/summary").dispatch().status(),
            Status::Ok
        );
        assert_eq!(
            stats_client.get("/api/stats/summary").dispatch().status(),
            Status::TooManyRequests
        );

        let user_client = limited_client();
        let (session_token, _) = login(&user_client);
        assert_eq!(
            user_client
                .get("/api/user/pastes")
                .header(bearer(&session_token))
                .dispatch()
                .status(),
            Status::Ok
        );
        assert_eq!(
            user_client
                .get("/api/user/pastes")
                .header(bearer(&session_token))
                .dispatch()
                .status(),
            Status::TooManyRequests
        );
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
    fn api_health_is_minimal_and_excludes_dependency_details() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let response = client.get("/api/health").dispatch();
        assert_eq!(response.status(), Status::Ok);

        let body = response.into_string().expect("body");
        let health: serde_json::Value = serde_json::from_str(&body).expect("parse health");

        assert_eq!(health["status"], "ok");
        assert!(health["timestamp"].as_i64().is_some_and(|value| value > 0));
        assert!(health.get("services").is_none());
        assert!(health.get("commit_message").is_none());
    }

    #[test]
    fn well_known_tells_agents_how_to_send_and_read() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let response = client.get("/.well-known/copypaste.json").dispatch();
        assert_eq!(response.status(), Status::Ok);
        let body: serde_json::Value =
            serde_json::from_str(&response.into_string().expect("body")).expect("json");
        assert_eq!(body["copypaste"], 1);
        assert_eq!(body["create"], "/api/pastes");
        assert_eq!(body["key_header"], "X-Paste-Key");
        assert_eq!(body["write_header"], "X-CopyPaste-Write-Token");
        assert!(body["encryption"]
            .as_array()
            .unwrap()
            .contains(&json!("aes256_gcm")));
        assert!(body["note"].as_str().unwrap().contains("ciphertext"));
    }

    #[test]
    fn plaintext_send_roundtrip_is_readable_then_gone_when_missing() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let created = client
            .post("/")
            .header(ContentType::JSON)
            .body(json!({ "content": "agent ping", "format": "plain_text" }).to_string())
            .dispatch();
        assert_eq!(created.status(), Status::Ok);
        let path = created.into_string().expect("path");
        let id = path.trim().trim_start_matches('/');
        let fetched = client.get(format!("/api/pastes/{id}")).dispatch();
        assert_eq!(fetched.status(), Status::Ok);
        let body: serde_json::Value =
            serde_json::from_str(&fetched.into_string().expect("body")).expect("json");
        assert_eq!(body["content"], "agent ping");

        let missing = client.get("/api/pastes/no-such-id").dispatch();
        assert_eq!(missing.status(), Status::NotFound);
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
        assert_eq!(
            status_to_code(Status::ServiceUnavailable),
            "service_unavailable"
        );
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
        let missing_body = response.into_string().expect("body");
        assert!(missing_body.contains("paste_not_found"));
        assert!(!missing_body.contains("nonexistent-id"));
    }

    #[test]
    fn show_api_expired_paste_matches_missing_paste() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "gone".to_string(),
            },
            format: PasteFormat::PlainText,
            created_at: 1,
            expires_at: Some(1),
            burn_after_reading: false,
            bundle: None,
            bundle_parent: None,
            bundle_label: None,
            not_before: None,
            not_after: None,
            persistence: None,
            webhook: None,
            metadata: PasteMetadata::default(),
            is_live: false,
            owner_token_hash: None,
        };
        let id = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.create_paste(paste))
            .expect("create expired paste");
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let expired = client.get(format!("/api/pastes/{id}")).dispatch();
        let missing = client.get("/api/pastes/no-such-paste").dispatch();
        assert_eq!(expired.status(), Status::NotFound);
        assert_eq!(missing.status(), Status::NotFound);
        assert_eq!(
            expired.into_string().expect("expired body"),
            missing.into_string().expect("missing body")
        );
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
        let body = get.into_string().unwrap();
        let raw: serde_json::Value = serde_json::from_str(&body).unwrap();
        for sensitive_or_disabled_field in [
            "accessCount",
            "bundle",
            "persistence",
            "workspace",
            "attestation",
            "webhook",
            "stego",
        ] {
            assert!(
                raw.get(sensitive_or_disabled_field).is_none(),
                "public response unexpectedly exposed {sensitive_or_disabled_field}"
            );
        }
        let view: PasteViewResponse = serde_json::from_str(&body).unwrap();
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
    fn auth_challenge_and_login_are_rate_limited() {
        let challenge_store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let challenge_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let challenge_rocket = build_rocket_with_components(
            challenge_store,
            challenge_key_store,
            PasteRateLimiter::new(Some(1), None),
            StaticAuthTokens::default(),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let challenge_client = Client::tracked(challenge_rocket).expect("client");

        assert_eq!(
            challenge_client
                .get("/api/auth/challenge")
                .dispatch()
                .status(),
            Status::Ok
        );
        assert_eq!(
            challenge_client
                .get("/api/auth/challenge")
                .dispatch()
                .status(),
            Status::TooManyRequests
        );

        let login_store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let login_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let login_rocket = build_rocket_with_components(
            login_store,
            login_key_store,
            PasteRateLimiter::new(Some(1), None),
            StaticAuthTokens::default(),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let login_client = Client::tracked(login_rocket).expect("client");
        let malformed = json!({"pubkey": "", "signature": "", "challenge": ""}).to_string();

        assert_eq!(
            login_client
                .post("/api/auth/login")
                .header(ContentType::JSON)
                .body(malformed.clone())
                .dispatch()
                .status(),
            Status::BadRequest
        );
        assert_eq!(
            login_client
                .post("/api/auth/login")
                .header(ContentType::JSON)
                .body(malformed)
                .dispatch()
                .status(),
            Status::TooManyRequests
        );
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
            .header(bearer(&token))
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
        assert!(parsed["pastes"][0].get("accessCount").is_none());
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
        let rocket =
            build_rocket_with_features(store, FeaturePolicy::default().with_attestations(true));
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
            "burn_after_reading": true
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
    fn stego_builtin_internal_policy_round_trips_content() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket_with_features(store, FeaturePolicy::default().with_stego(true));
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

        // The test-only policy exercises legacy decoding without advertising
        // hidden-carrier metadata in the hardened public response.
        let get = client
            .get(format!("/api/pastes/{}?key=stegokey", created.id))
            .dispatch();
        assert_eq!(get.status(), Status::Ok);
        let view: PasteViewResponse = serde_json::from_str(&get.into_string().unwrap()).unwrap();
        assert_eq!(view.content, "hidden message");
        assert!(view.encryption.requires_key);
    }

    #[test]
    fn stego_without_encryption_returns_bad_request() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket_with_features(store, FeaturePolicy::default().with_stego(true));
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
        let rocket = build_rocket_with_features(store, FeaturePolicy::new(false, true)).configure(
            rocket::Config {
                limits: Limits::default().limit("json", 10.mebibytes()),
                ..Default::default()
            },
        );
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
        let rocket = build_rocket_with_features(
            Arc::clone(&store),
            FeaturePolicy::default().with_stego(true),
        );
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
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::new(None, Some("test-admin-bootstrap".to_string())),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
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
            .delete(format!("/api/admin/keys/{key_id}"))
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

    #[test]
    fn static_admin_starts_with_dynamic_key_management_disabled() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore = Arc::new(SqliteApiKeyStore::disabled());
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::new(None, Some("static-admin".to_string())),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");

        let response = client
            .post("/api/admin/keys")
            .header(ContentType::JSON)
            .header(bearer("static-admin"))
            .body(json!({"name": "must-not-be-ephemeral", "scope": "write"}).to_string())
            .dispatch();
        assert_eq!(response.status(), Status::ServiceUnavailable);
        let body = response.into_string().expect("error response");
        assert!(body.contains("Dynamic API-key management is disabled"));
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

    #[test]
    fn auth_login_rejects_signed_but_unissued_challenge() {
        use ed25519_dalek::{Signer, SigningKey};

        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let client = Client::tracked(build_rocket(store)).expect("client");
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let challenge = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let signature = signing_key.sign(challenge.as_bytes());

        let response = client
            .post("/api/auth/login")
            .header(ContentType::JSON)
            .body(
                json!({
                    "pubkey": BASE64_STANDARD.encode(signing_key.verifying_key().as_bytes()),
                    "signature": BASE64_STANDARD.encode(signature.to_bytes()),
                    "challenge": challenge
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(response.status(), Status::Unauthorized);
    }

    #[test]
    fn auth_login_challenge_cannot_be_replayed() {
        use ed25519_dalek::{Signer, SigningKey};

        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let client = Client::tracked(build_rocket(store)).expect("client");
        let challenge_response = client.get("/api/auth/challenge").dispatch();
        let challenge_json: serde_json::Value = serde_json::from_str(
            &challenge_response
                .into_string()
                .expect("challenge response body"),
        )
        .expect("challenge response JSON");
        let challenge = challenge_json["challenge"]
            .as_str()
            .expect("challenge")
            .to_string();
        let signing_key = SigningKey::from_bytes(&[8u8; 32]);
        let signature = signing_key.sign(challenge.as_bytes());
        let body = json!({
            "pubkey": BASE64_STANDARD.encode(signing_key.verifying_key().as_bytes()),
            "signature": BASE64_STANDARD.encode(signature.to_bytes()),
            "challenge": challenge
        })
        .to_string();

        let first = client
            .post("/api/auth/login")
            .header(ContentType::JSON)
            .body(body.clone())
            .dispatch();
        assert_eq!(first.status(), Status::Ok);

        let replay = client
            .post("/api/auth/login")
            .header(ContentType::JSON)
            .body(body)
            .dispatch();
        assert_eq!(replay.status(), Status::Unauthorized);
    }

    #[test]
    fn auth_login_rejects_oversized_challenge_before_verification() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let client = Client::tracked(build_rocket(store)).expect("client");
        let response = client
            .post("/api/auth/login")
            .header(ContentType::JSON)
            .body(
                json!({
                    "pubkey": BASE64_STANDARD.encode([0u8; 32]),
                    "signature": BASE64_STANDARD.encode([0u8; 64]),
                    "challenge": "x".repeat(MAX_AUTH_CHALLENGE_LEN + 1)
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(response.status(), Status::BadRequest);
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
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::default(),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
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

    #[test]
    fn configured_write_auth_token_protects_both_create_routes() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::new(Some("private-write-token".to_string()), None)
                .with_required_write_auth(true),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");
        let body = json!({"content": "protected", "format": "plain_text"}).to_string();

        assert_eq!(
            client
                .post("/api/pastes")
                .header(ContentType::JSON)
                .body(body.clone())
                .dispatch()
                .status(),
            Status::Unauthorized
        );
        assert_eq!(
            client
                .post("/")
                .header(ContentType::JSON)
                .body(body.clone())
                .dispatch()
                .status(),
            Status::Unauthorized
        );
        assert_eq!(
            client
                .post("/api/pastes")
                .header(ContentType::JSON)
                .header(bearer("private-write-token"))
                .body(body)
                .dispatch()
                .status(),
            Status::Ok
        );
    }

    #[test]
    fn public_create_read_and_stats_work_when_write_token_is_not_required() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::new(Some("private-write-token".to_string()), None),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");
        let created = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(json!({"content": "public get link", "format": "plain_text"}).to_string())
            .dispatch();
        assert_eq!(created.status(), Status::Ok);
        let body: CreatePasteResponse =
            serde_json::from_str(&created.into_string().unwrap()).unwrap();

        let view = client.get(format!("/api/pastes/{}", body.id)).dispatch();
        assert_eq!(view.status(), Status::Ok);
        let paste: PasteViewResponse = serde_json::from_str(&view.into_string().unwrap()).unwrap();
        assert_eq!(paste.content, "public get link");

        let stats_resp = client.get("/api/stats/summary").dispatch();
        assert_eq!(stats_resp.status(), Status::Ok);
        let stats: StatsSummaryResponse =
            serde_json::from_str(&stats_resp.into_string().unwrap()).unwrap();
        assert!(stats.total_pastes >= 1);
        assert!(stats.active_pastes >= 1);
    }

    #[test]
    fn configured_write_auth_accepts_write_key_and_rejects_read_key() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let (_, write_key) = api_key_store
            .create_key("writer", super::super::api_keys::ApiScope::Write, None)
            .expect("write key");
        let (_, read_key) = api_key_store
            .create_key("reader", super::super::api_keys::ApiScope::Read, None)
            .expect("read key");
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::new(Some("private-write-token".to_string()), None),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");
        let body = json!({"content": "scoped", "format": "plain_text"}).to_string();

        assert_eq!(
            client
                .post("/api/pastes")
                .header(ContentType::JSON)
                .header(bearer(&write_key))
                .body(body.clone())
                .dispatch()
                .status(),
            Status::Ok
        );
        assert_eq!(
            client
                .post("/api/pastes")
                .header(ContentType::JSON)
                .header(bearer(&read_key))
                .body(body)
                .dispatch()
                .status(),
            Status::Forbidden
        );
    }

    #[test]
    fn required_write_auth_accepts_static_admin_token() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::new(None, Some("private-admin-token".to_string()))
                .with_required_write_auth(true),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");

        assert_eq!(
            client
                .post("/api/pastes")
                .header(ContentType::JSON)
                .header(bearer("private-admin-token"))
                .body(json!({"content": "admin-authorized"}).to_string())
                .dispatch()
                .status(),
            Status::Ok
        );
    }

    #[test]
    fn required_write_auth_rejects_self_service_session_by_default() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::default().with_required_write_auth(true),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");
        let (token, _) = login(&client);

        let response = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .header(bearer(&token))
            .body(json!({"content": "not-authorized"}).to_string())
            .dispatch();

        assert_eq!(response.status(), Status::Unauthorized);
    }

    #[test]
    fn required_write_auth_accepts_session_only_with_explicit_opt_in() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            Arc::clone(&store),
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::default()
                .with_required_write_auth(true)
                .with_session_writes(true),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");
        let body = json!({"content": "session-owned", "owner_pubkey_hash": "spoofed"}).to_string();
        let anonymous = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(body.clone())
            .dispatch();
        assert_eq!(anonymous.status(), Status::Unauthorized);

        let (token, pubkey_hash) = login(&client);
        let created = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .header(bearer(&token))
            .body(body)
            .dispatch();
        assert_eq!(created.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&created.into_string().unwrap()).unwrap();
        let stored = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.get_paste(&created.id))
            .expect("stored paste");
        assert_eq!(
            stored.metadata.owner_pubkey_hash.as_deref(),
            Some(pubkey_hash.as_str())
        );
    }

    #[test]
    fn closed_create_combines_service_admission_with_session_ownership() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            Arc::clone(&store),
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::new(Some("service-write-token".to_string()), None)
                .with_required_write_auth(true),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");
        let (session_token, pubkey_hash) = login(&client);

        let response = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .header(bearer(&session_token))
            .header(mutation_write_token("service-write-token"))
            .body(json!({"content": "owned and admitted"}).to_string())
            .dispatch();
        assert_eq!(response.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&response.into_string().unwrap()).unwrap();
        let stored = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.get_paste(&created.id))
            .expect("stored paste");
        assert_eq!(
            stored.metadata.owner_pubkey_hash.as_deref(),
            Some(pubkey_hash.as_str())
        );
    }

    #[test]
    fn duplicate_or_blank_write_credentials_fail_closed() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore = Arc::new(SqliteApiKeyStore::disabled());
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::new(Some("service-write-token".to_string()), None),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");
        let body = json!({"content": "must not be admitted"}).to_string();

        let blank = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .header(mutation_write_token("   "))
            .body(body.clone())
            .dispatch();
        assert_eq!(blank.status(), Status::Unauthorized);

        let mut duplicate_admission_request = client.post("/api/pastes");
        duplicate_admission_request.add_header(ContentType::JSON);
        duplicate_admission_request.add_header(mutation_write_token("service-write-token"));
        duplicate_admission_request.add_header(mutation_write_token("service-write-token"));
        let duplicate_admission = duplicate_admission_request.body(body.clone()).dispatch();
        assert_eq!(duplicate_admission.status(), Status::Unauthorized);

        let mut duplicate_authorization_request = client.post("/api/pastes");
        duplicate_authorization_request.add_header(ContentType::JSON);
        duplicate_authorization_request.add_header(bearer("service-write-token"));
        duplicate_authorization_request.add_header(bearer("service-write-token"));
        let duplicate_authorization = duplicate_authorization_request.body(body).dispatch();
        assert_eq!(duplicate_authorization.status(), Status::Unauthorized);
    }

    #[test]
    fn anchor_route_requires_admin_auth_before_lookup() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let client = Client::tracked(build_rocket(store)).expect("client");
        let response = client
            .post("/api/pastes/not-a-real-id/anchor")
            .header(ContentType::JSON)
            .body("{}")
            .dispatch();
        assert_eq!(response.status(), Status::Unauthorized);
    }

    #[test]
    fn admin_moderation_is_metadata_only_and_can_delete_exact_id() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            Arc::clone(&store),
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::new(None, Some("moderator-token".to_string())),
            FeaturePolicy::new(true, false).with_attestations(true),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");
        let content_secret = "CONTENT_SHOULD_NEVER_BE_IN_MODERATION_JSON";
        let attestation_secret = "ATTESTATION_SHOULD_NEVER_LEAK";
        let webhook_secret = "https://hooks.example.com/private-webhook-token";
        let created = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": content_secret,
                    "format": "markdown",
                    "attestation": {"kind": "totp", "secret": attestation_secret},
                    "webhook": {"url": webhook_secret},
                    "workspace": "trust-and-safety"
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(created.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&created.into_string().unwrap()).unwrap();

        let unauthenticated = client
            .get(format!("/api/admin/pastes/{}", created.id))
            .dispatch();
        assert_eq!(unauthenticated.status(), Status::Unauthorized);

        let metadata_response = client
            .get(format!("/api/admin/pastes/{}", created.id))
            .header(bearer("moderator-token"))
            .dispatch();
        assert_eq!(metadata_response.status(), Status::Ok);
        let metadata_body = metadata_response.into_string().expect("metadata body");
        assert!(!metadata_body.contains(content_secret));
        assert!(!metadata_body.contains(attestation_secret));
        assert!(!metadata_body.contains(webhook_secret));
        assert!(!metadata_body.contains("trust-and-safety"));
        assert!(!metadata_body.contains("\"accessCount\""));
        let metadata: AdminPasteMetadataResponse =
            serde_json::from_str(&metadata_body).expect("moderation metadata JSON");
        assert_eq!(metadata.id, created.id);
        assert_eq!(metadata.approximate_stored_bytes, content_secret.len());
        assert!(metadata.has_attestation);
        assert!(metadata.has_webhook);
        assert!(metadata.has_workspace);
        assert!(!metadata.encrypted);

        let delete_response = client
            .delete(format!("/api/admin/pastes/{}", created.id))
            .header(bearer("moderator-token"))
            .dispatch();
        assert_eq!(delete_response.status(), Status::Ok);
        let deleted: AdminDeletePasteResponse =
            serde_json::from_str(&delete_response.into_string().unwrap()).unwrap();
        assert!(deleted.deleted);

        let runtime = tokio::runtime::Runtime::new().unwrap();
        assert!(matches!(
            runtime.block_on(store.get_paste(&created.id)),
            Err(PasteError::NotFound(_))
        ));
    }

    #[test]
    fn blocked_paste_ids_are_hidden_from_public_routes_but_visible_to_admin() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let metadata = PasteMetadata::default();
        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "quarantined".to_string(),
            },
            format: PasteFormat::PlainText,
            created_at: current_timestamp(),
            expires_at: None,
            burn_after_reading: false,
            bundle: None,
            bundle_parent: None,
            bundle_label: None,
            not_before: None,
            not_after: None,
            persistence: None,
            webhook: None,
            metadata,
            is_live: true,
            owner_token_hash: Some(hex::encode(Sha256::digest(b"owner-token"))),
        };
        let id = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.create_paste(paste))
            .expect("create quarantined paste");
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::new(None, Some("moderator-token".to_string())),
            FeaturePolicy::default(),
            BlockedPasteIds::from_csv(&id).expect("valid blocked id"),
            None,
        );
        let client = Client::tracked(rocket).expect("client");

        assert_eq!(
            client.get(format!("/api/pastes/{id}")).dispatch().status(),
            Status::NotFound
        );
        assert_eq!(
            client.get(format!("/{id}")).dispatch().status(),
            Status::NotFound
        );
        assert_eq!(
            client.get(format!("/raw/{id}")).dispatch().status(),
            Status::NotFound
        );
        assert_eq!(
            client
                .put(format!("/api/pastes/{id}"))
                .header(ContentType::JSON)
                .header(bearer("owner-token"))
                .body(json!({"content": "tampered after quarantine"}).to_string())
                .dispatch()
                .status(),
            Status::NotFound
        );
        assert_eq!(
            client
                .patch(format!("/api/pastes/{id}/finalize"))
                .header(bearer("owner-token"))
                .dispatch()
                .status(),
            Status::NotFound
        );
        assert_eq!(
            client
                .post(format!("/api/pastes/{id}/anchor"))
                .header(ContentType::JSON)
                .header(bearer("moderator-token"))
                .body("{}")
                .dispatch()
                .status(),
            Status::NotFound
        );
        assert_eq!(
            client
                .get(format!("/api/admin/pastes/{id}"))
                .header(bearer("moderator-token"))
                .dispatch()
                .status(),
            Status::Ok
        );
    }

    #[test]
    fn rocket_does_not_trust_forwarded_ip_headers_unless_configured() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let default_rocket = build_rocket_with_components(
            Arc::clone(&store),
            Arc::clone(&api_key_store),
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::default(),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        assert!(default_rocket.figment().find_value("ip_header").is_ok());
        let default_client = Client::tracked(default_rocket).expect("default client");
        assert!(default_client.rocket().config().ip_header.is_none());

        let fly_rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::default(),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            Some("Fly-Client-IP".to_string()),
        );
        let fly_client = Client::tracked(fly_rocket).expect("fly client");
        assert_eq!(
            fly_client
                .rocket()
                .config()
                .ip_header
                .as_ref()
                .map(|header| header.as_str()),
            Some("Fly-Client-IP")
        );
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
        let rocket =
            build_rocket_with_features(store, FeaturePolicy::default().with_attestations(true));
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
        let rocket =
            build_rocket_with_features(store, FeaturePolicy::default().with_attestations(true));
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
        let rocket =
            build_rocket_with_features(store, FeaturePolicy::default().with_attestations(true));
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

    #[test]
    fn paste_ownership_is_derived_from_session_not_client_input() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let client = Client::tracked(build_rocket(Arc::clone(&store))).expect("client");

        let anonymous = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "anonymous",
                    "owner_pubkey_hash": "spoofed-victim-hash"
                })
                .to_string(),
            )
            .dispatch();
        let anonymous: CreatePasteResponse =
            serde_json::from_str(&anonymous.into_string().unwrap()).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let anonymous_stored = runtime
            .block_on(store.get_paste(&anonymous.id))
            .expect("anonymous paste");
        assert!(anonymous_stored.metadata.owner_pubkey_hash.is_none());

        let (token, pubkey_hash) = login(&client);
        let authenticated = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .header(bearer(&token))
            .body(
                json!({
                    "content": "authenticated",
                    "owner_pubkey_hash": "another-spoofed-hash"
                })
                .to_string(),
            )
            .dispatch();
        let authenticated: CreatePasteResponse =
            serde_json::from_str(&authenticated.into_string().unwrap()).unwrap();
        let authenticated_stored = runtime
            .block_on(store.get_paste(&authenticated.id))
            .expect("authenticated paste");
        assert_eq!(
            authenticated_stored.metadata.owner_pubkey_hash.as_deref(),
            Some(pubkey_hash.as_str())
        );
    }

    // ── Input validation tests ─────────────────────────────────────────────────

    #[test]
    fn create_api_rejects_oversized_content() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let content = "a".repeat(1_048_577);
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

        let content = "a".repeat(1_048_576);
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
    fn create_api_rejects_all_bundle_requests() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let payload = json!({
            "content": "bundle parent",
            "format": "plain_text",
            "encryption": { "algorithm": "aes256_gcm", "key": "bundlekey" },
            "bundle": { "children": [{"content": "child"}] }
        });

        let resp = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch();
        assert_eq!(resp.status(), Status::Forbidden);
    }

    #[test]
    fn legacy_bundle_child_pointers_are_not_exposed() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let metadata = PasteMetadata {
            bundle: Some(crate::BundleMetadata {
                children: vec![crate::BundlePointer {
                    id: "blocked-child-sensitive-id".to_string(),
                    label: Some("sensitive child".to_string()),
                }],
            }),
            persistence: Some(PersistenceLocator::Vault {
                key_path: "secret/legacy/paste-location".to_string(),
            }),
            ..Default::default()
        };
        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "legacy parent".to_string(),
            },
            format: PasteFormat::PlainText,
            created_at: current_timestamp(),
            expires_at: None,
            burn_after_reading: false,
            bundle: metadata.bundle.clone(),
            bundle_parent: None,
            bundle_label: None,
            not_before: None,
            not_after: None,
            persistence: None,
            webhook: None,
            metadata,
            is_live: false,
            owner_token_hash: None,
        };
        let id = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.create_paste(paste))
            .expect("store legacy bundle parent");
        let client = Client::tracked(build_rocket(store)).expect("client");

        let api = client.get(format!("/api/pastes/{id}")).dispatch();
        assert_eq!(api.status(), Status::Ok);
        let api_body = api.into_string().expect("API response");
        assert!(!api_body.contains("blocked-child-sensitive-id"));
        let parsed: serde_json::Value = serde_json::from_str(&api_body).unwrap();
        assert!(parsed.get("bundle").is_none());
        assert!(parsed.get("persistence").is_none());

        let html = client.get(format!("/{id}")).dispatch();
        assert_eq!(html.status(), Status::Ok);
        let html_body = html.into_string().expect("HTML response");
        assert!(!html_body.contains("blocked-child-sensitive-id"));
        assert!(!html_body.contains("secret/legacy/paste-location"));
        assert!(!html_body.contains("Persistence:"));
    }

    #[test]
    fn stego_uploaded_rejects_invalid_mime_type() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket_with_features(store, FeaturePolicy::new(false, true));
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
        use rocket::data::{Limits, ToByteUnit};

        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        // Test-only legacy policy and raised transport limit let the inner
        // defense be exercised; production rejects this feature outright.
        let rocket = build_rocket_with_features(store, FeaturePolicy::new(false, true)).configure(
            rocket::Config {
                limits: Limits::default().limit("json", 12.mebibytes()),
                ..Default::default()
            },
        );
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

    #[test]
    fn raw_route_accepts_key_via_header_and_header_wins_over_query() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let create = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(
                json!({
                    "content": "raw header secret",
                    "format": "plain_text",
                    "encryption": { "algorithm": "aes256_gcm", "key": "headerpass" }
                })
                .to_string(),
            )
            .dispatch();
        assert_eq!(create.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create.into_string().unwrap()).unwrap();

        let ok = client
            .get(format!("/raw/{}", created.id))
            .header(rocket::http::Header::new("X-Paste-Key", "headerpass"))
            .dispatch();
        assert_eq!(ok.status(), Status::Ok);
        assert_eq!(ok.into_string().as_deref(), Some("raw header secret"));

        let forbidden = client
            .get(format!("/raw/{}?key=headerpass", created.id))
            .header(rocket::http::Header::new("X-Paste-Key", "wrong"))
            .dispatch();
        assert_eq!(forbidden.status(), Status::Forbidden);
    }

    // ── Webhook SSRF validation at paste creation ──────────────────────────────

    #[test]
    fn create_api_rejects_ssrf_webhook_urls_with_structured_400() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket_with_features(store, FeaturePolicy::new(true, false));
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
    fn create_api_rejects_retention_that_cannot_fit_timestamp() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let response = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(json!({ "content": "x", "retention_minutes": u64::MAX }).to_string())
            .dispatch();
        assert_eq!(response.status(), Status::BadRequest);
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
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(Some(2), None),
            StaticAuthTokens::default(),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");

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
        assert_eq!(resp.headers().get_one("Retry-After"), Some("60"));
    }

    // ── Workspace persistence & listing ────────────────────────────────────────

    #[test]
    fn workspace_is_stored_but_not_echoed_by_public_view_response() {
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

        // Workspace labels are available only through owner/admin-scoped
        // endpoints, never echoed by the public content response.
        let view_resp = client.get(format!("/api/pastes/{}", created.id)).dispatch();
        assert_eq!(view_resp.status(), Status::Ok);
        let view: serde_json::Value =
            serde_json::from_str(&view_resp.into_string().unwrap()).unwrap();
        assert!(view.get("workspace").is_none());
        assert!(view.get("persistence").is_none());
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

        // One authenticated paste owned by the session and one anonymous paste.
        for (owner, content) in [(pubkey_hash.as_str(), "mine"), ("someone_else", "theirs")] {
            let mut request = client.post("/api/pastes").header(ContentType::JSON);
            if content == "mine" {
                request = request.header(bearer(&token));
            }
            let resp = request
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
    fn closed_update_requires_service_admission_and_owner_capability() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::new(Some("service-write-token".to_string()), None)
                .with_required_write_auth(true),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");

        let create = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .header(bearer("service-write-token"))
            .body(json!({"content": "live v1", "live": true}).to_string())
            .dispatch();
        assert_eq!(create.status(), Status::Ok);
        let created: CreatePasteResponse =
            serde_json::from_str(&create.into_string().unwrap()).unwrap();
        let owner_token = created.token.expect("ownership token");
        let update_body = json!({"content": "live v2"}).to_string();

        // Ownership alone never grants admission to a closed service.
        let owner_only = client
            .put(format!("/api/pastes/{}", created.id))
            .header(ContentType::JSON)
            .header(bearer(&owner_token))
            .body(update_body.clone())
            .dispatch();
        assert_eq!(owner_only.status(), Status::Unauthorized);

        // Service admission alone never substitutes for paste ownership.
        let admission_only = client
            .put(format!("/api/pastes/{}", created.id))
            .header(ContentType::JSON)
            .header(mutation_write_token("service-write-token"))
            .body(update_body.clone())
            .dispatch();
        assert_eq!(admission_only.status(), Status::Unauthorized);

        let mut duplicate_owner_request = client.put(format!("/api/pastes/{}", created.id));
        duplicate_owner_request.add_header(ContentType::JSON);
        duplicate_owner_request.add_header(bearer(&owner_token));
        duplicate_owner_request.add_header(bearer(&owner_token));
        duplicate_owner_request.add_header(mutation_write_token("service-write-token"));
        let duplicate_owner = duplicate_owner_request.body(update_body.clone()).dispatch();
        assert_eq!(duplicate_owner.status(), Status::Unauthorized);

        let admitted_owner = client
            .put(format!("/api/pastes/{}", created.id))
            .header(ContentType::JSON)
            .header(bearer(&owner_token))
            .header(mutation_write_token("service-write-token"))
            .body(update_body)
            .dispatch();
        assert_eq!(admitted_owner.status(), Status::Ok);
    }

    #[test]
    fn live_updates_share_the_mutation_rate_limit() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(Some(2), None),
            StaticAuthTokens::default(),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");
        let (id, owner_token) = create_live_paste(&client);

        // Creation consumed one request; the first update consumes the second.
        let first = client
            .put(format!("/api/pastes/{id}"))
            .header(ContentType::JSON)
            .header(bearer(&owner_token))
            .body(json!({"content": "live v2"}).to_string())
            .dispatch();
        assert_eq!(first.status(), Status::Ok);

        let limited = client
            .put(format!("/api/pastes/{id}"))
            .header(ContentType::JSON)
            .header(bearer(&owner_token))
            .body(json!({"content": "live v3"}).to_string())
            .dispatch();
        assert_eq!(limited.status(), Status::TooManyRequests);
    }

    #[test]
    fn closed_finalize_requires_service_admission_and_owner_capability() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(None, None),
            StaticAuthTokens::new(Some("service-write-token".to_string()), None)
                .with_required_write_auth(true),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");
        let create = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .header(bearer("service-write-token"))
            .body(json!({"content": "live v1", "live": true}).to_string())
            .dispatch();
        let created: CreatePasteResponse =
            serde_json::from_str(&create.into_string().unwrap()).unwrap();
        let owner_token = created.token.expect("ownership token");

        let owner_only = client
            .patch(format!("/api/pastes/{}/finalize", created.id))
            .header(ContentType::JSON)
            .header(bearer(&owner_token))
            .body(json!({"live": false}).to_string())
            .dispatch();
        assert_eq!(owner_only.status(), Status::Unauthorized);

        let admission_only = client
            .patch(format!("/api/pastes/{}/finalize", created.id))
            .header(ContentType::JSON)
            .header(mutation_write_token("service-write-token"))
            .body(json!({"live": false}).to_string())
            .dispatch();
        assert_eq!(admission_only.status(), Status::Unauthorized);

        let both = client
            .patch(format!("/api/pastes/{}/finalize", created.id))
            .header(ContentType::JSON)
            .header(bearer(&owner_token))
            .header(mutation_write_token("service-write-token"))
            .body(json!({"live": false}).to_string())
            .dispatch();
        assert_eq!(both.status(), Status::Ok);
    }

    #[test]
    fn finalize_shares_the_mutation_rate_limit() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let api_key_store: SharedApiKeyStore =
            Arc::new(SqliteApiKeyStore::in_memory().expect("failed to initialise API key store"));
        let rocket = build_rocket_with_components(
            store,
            api_key_store,
            PasteRateLimiter::new(Some(2), None),
            StaticAuthTokens::default(),
            FeaturePolicy::default(),
            BlockedPasteIds::default(),
            None,
        );
        let client = Client::tracked(rocket).expect("client");
        let (id, owner_token) = create_live_paste(&client);

        let first = client
            .patch(format!("/api/pastes/{id}/finalize"))
            .header(ContentType::JSON)
            .header(bearer(&owner_token))
            .body(json!({"live": false}).to_string())
            .dispatch();
        assert_eq!(first.status(), Status::Ok);

        let limited = client
            .patch(format!("/api/pastes/{id}/finalize"))
            .header(ContentType::JSON)
            .header(bearer(&owner_token))
            .body(json!({"live": false}).to_string())
            .dispatch();
        assert_eq!(limited.status(), Status::TooManyRequests);
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
    fn openapi_json_is_served_without_interactive_runtime() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
        let rocket = build_rocket(store);
        let client = Client::tracked(rocket).expect("client");

        let resp = client.get("/api/openapi.json").dispatch();
        assert_eq!(resp.status(), Status::Ok);
        let doc: serde_json::Value =
            serde_json::from_str(&resp.into_string().unwrap()).expect("valid OpenAPI JSON");
        assert!(doc["paths"]["/api/pastes"].is_object());
        assert!(doc["paths"]["/api/pastes/{id}"].is_object());
        assert!(doc["paths"]["/p/{id}"].is_object());
        let create_schema = &doc["components"]["schemas"]["CreatePasteApiSchema"];
        assert!(create_schema.is_object());
        for disabled in ["bundle", "attestation", "persistence", "webhook", "stego"] {
            assert!(create_schema["properties"].get(disabled).is_none());
        }
        let description = doc["info"]["description"].as_str().unwrap();
        assert!(!description.contains("bundle"));
        assert!(!description.contains("attestation"));
        assert!(!description.contains("webhook"));
        assert!(!description.contains("stego"));
    }
}
