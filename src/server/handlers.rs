use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use rocket::{
    fs::FileServer, http::Status, response::content, serde::json::Json, Build, Rocket, State,
};

use crate::{
    create_paste_store, EncryptionAlgorithm, PasteError, PasteFormat, PasteMetadata,
    PersistenceLocator, SharedPasteStore, StoredContent, StoredPaste, WebhookConfig,
};
use rocket::{get, post, routes};
use serde_json;
use sha2::{Digest, Sha256};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rand::Rng;

use super::attestation::{self, AttestationVerdict};
use super::blockchain::{
    default_anchor_relayer, infer_attestation_ref, infer_retention_class, manifest_hash,
    AnchorManifest, AnchorPayload, SharedAnchorRelayer,
};
use super::bundles::build_bundle_overview;
use super::cors::{api_preflight, Cors};
use super::crypto::{decrypt_content, encrypt_content, DecryptError};
use super::models::{
    AnchorRequest, AnchorResponse, AuthChallengeResponse, AuthLoginRequest, AuthLoginResponse,
    AuthLogoutResponse, CreatePasteRequest, CreatePasteResponse, PasteViewQuery,
    PersistenceRequest, StatsSummaryResponse, StegoRequest, TimeLockRequest,
    UserPasteCountResponse, UserPasteListItem, UserPasteListResponse, WebhookRequest,
};
use super::render::{
    render_attestation_prompt, render_expired, render_invalid_key, render_key_prompt,
    render_paste_view, render_time_locked, StoredPasteView,
};
use super::stego::parse_data_uri;
use super::time::{current_timestamp, evaluate_time_lock, parse_timestamp};
use super::tor::{OnionAccess, TorConfig};
use super::webhook::{trigger_webhook, WebhookEvent};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub fn build_rocket(store: SharedPasteStore) -> Rocket<Build> {
    let tor_config = TorConfig::from_env();

    rocket::build()
        .manage(store)
        .manage(default_anchor_relayer())
        .manage(tor_config)
        .attach(Cors)
        .mount(
            "/",
            routes![
                api_preflight,
                index,
                about,
                spa_fallback,
                create,
                create_api,
                anchor_api,
                api_test,
                api_echo,
                show_api,
                show,
                show_raw,
                stats_summary_api,
                auth_challenge_api,
                auth_login_api,
                auth_logout_api,
                user_paste_count_api,
                user_paste_list_api,
                health_api,
                health_detailed_api
            ],
        )
        .mount("/static", FileServer::from("static"))
}

pub async fn launch() -> Result<(), Box<dyn std::error::Error>> {
    let store = create_paste_store();
    build_rocket(store).launch().await?;
    Ok(())
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

    let crypto_status = match reqwest::get(format!("{}/health", crypto_verifier_url)).await {
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

    let overall_status = if crypto_status.status == "ok" && storage_status.status == "ok" {
        "ok"
    } else if crypto_status.status == "unavailable" || storage_status.status == "unavailable" {
        "degraded"
    } else {
        "ok"
    };

    Json(DetailedHealthResponse {
        status: overall_status.to_string(),
        timestamp: current_timestamp(),
        version: env!("CARGO_PKG_VERSION").to_string(),
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

    // Generate session token (simple random for now)
    let token = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect::<String>();

    // TODO: Store token with pubkey_hash for session validation

    Ok(Json(AuthLoginResponse { token, pubkey_hash }))
}

#[utoipa::path(
    post,
    path = "/api/auth/logout",
    responses((status = 200, description = "Auth logout response", body = AuthLogoutResponse))
)]
#[post("/api/auth/logout")]
async fn auth_logout_api() -> Json<AuthLogoutResponse> {
    // For now, logout is stateless - just return success
    // In the future, this could invalidate server-side sessions if implemented
    Json(AuthLogoutResponse { success: true })
}

#[utoipa::path(
    get,
    path = "/api/user/paste-count",
    params(("pubkey_hash" = String, description = "Pubkey hash")),
    responses((status = 200, description = "User paste count response", body = UserPasteCountResponse))
)]
#[get("/api/user/paste-count?<pubkey_hash>")]
async fn user_paste_count_api(
    store: &State<SharedPasteStore>,
    pubkey_hash: String,
    onion: OnionAccess,
) -> Json<UserPasteCountResponse> {
    if onion.suppress_logs() {
        rocket::info!("user paste count accessed via onion host");
    }

    // Count pastes owned by this user
    let all_pastes = store.get_all_paste_ids().await;
    let mut count = 0;

    for id in all_pastes {
        if let Ok(paste) = store.get_paste(&id).await {
            if let Some(owner_hash) = &paste.metadata.owner_pubkey_hash {
                if owner_hash == &pubkey_hash {
                    count += 1;
                }
            }
        }
    }

    Json(UserPasteCountResponse { paste_count: count })
}

#[utoipa::path(
    get,
    path = "/api/user/pastes",
    params(("pubkey_hash" = String, description = "Pubkey hash")),
    responses((status = 200, description = "User paste list response", body = UserPasteListResponse))
)]
#[get("/api/user/pastes?<pubkey_hash>")]
async fn user_paste_list_api(
    store: &State<SharedPasteStore>,
    pubkey_hash: String,
    onion: OnionAccess,
) -> Json<UserPasteListResponse> {
    if onion.suppress_logs() {
        rocket::info!("user paste list accessed via onion host");
    }

    // Get all pastes owned by this user
    let all_pastes = store.get_all_paste_ids().await;
    let mut user_pastes = Vec::new();

    for id in all_pastes {
        if let Ok(paste) = store.get_paste(&id).await {
            if let Some(owner_hash) = &paste.metadata.owner_pubkey_hash {
                if owner_hash == &pubkey_hash {
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
                    });
                }
            }
        }
    }

    // Sort by created_at descending (newest first)
    user_pastes.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Json(UserPasteListResponse {
        pastes: user_pastes,
    })
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

#[utoipa::path(
    get,
    path = "/api/test",
    responses((status = 200, description = "API connectivity test"))
)]
#[get("/api/test", rank = 1)]
async fn api_test() -> Json<serde_json::Value> {
    Json(serde_json::json!({"message": "API test successful"}))
}

#[utoipa::path(
    get,
    path = "/api/echo/{id}",
    params(("id" = String, description = "Echo value")),
    responses((status = 200, description = "Echo response"))
)]
#[get("/api/echo/<id>", rank = 1)]
async fn api_echo(id: String) -> Json<serde_json::Value> {
    Json(serde_json::json!({"echo": id}))
}

#[utoipa::path(
    get,
    path = "/api/pastes/{id}",
    params(("id" = String, description = "Paste identifier")),
    responses(
        (status = 200, description = "Paste content", body = PasteViewResponse),
        (status = 401, description = "Key required"),
        (status = 403, description = "Invalid key"),
        (status = 404, description = "Paste not found"),
    )
)]
#[get("/api/pastes/<id>?<query..>", rank = 1)]
async fn show_api(
    store: &State<SharedPasteStore>,
    id: String,
    query: PasteViewQuery,
) -> Result<Json<serde_json::Value>, Status> {
    rocket::info!(
        "show_api called with id: {} and query.key: {:?}",
        id,
        query.key
    );
    match store.get_paste(&id).await {
        Ok(paste) => {
            rocket::info!("Paste found for id: {}", id);
            match decrypt_content(&paste.content, query.key.as_deref()) {
                Ok(text) => {
                    rocket::info!(
                        "Decryption successful for id: {}, content length: {}",
                        id,
                        text.len()
                    );
                    let response = serde_json::json!({
                        "id": id,
                        "content": text,
                        "format": format!("{:?}", paste.format).to_lowercase(),
                        "created_at": paste.created_at,
                        "expires_at": paste.expires_at,
                        "burn_after_reading": paste.burn_after_reading,
                        "encryption": match &paste.content {
                            StoredContent::Plain { .. } => serde_json::json!({"algorithm": "none", "requires_key": false}),
                            StoredContent::Encrypted { algorithm, .. } | StoredContent::Stego { algorithm, .. } =>
                                serde_json::json!({"algorithm": format!("{:?}", algorithm).to_lowercase(), "requires_key": true}),
                        }
                    });
                    Ok(Json(response))
                }
                Err(DecryptError::MissingKey) => {
                    rocket::info!("Missing key for encrypted paste: {}", id);
                    Err(Status::Unauthorized)
                }
                Err(DecryptError::InvalidKey) => {
                    rocket::error!("Invalid key for paste: {}", id);
                    Err(Status::Forbidden)
                }
            }
        }
        Err(e) => {
            rocket::error!("Paste not found for id: {}, error: {:?}", id, e);
            Err(Status::NotFound)
        }
    }
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
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error"),
    )
)]
#[post("/api/pastes", data = "<body>")]
async fn create_api(
    store: &State<SharedPasteStore>,
    body: Result<Json<CreatePasteRequest>, rocket::serde::json::Error<'_>>,
    onion: OnionAccess,
) -> Result<Json<CreatePasteResponse>, (Status, String)> {
    // Handle JSON deserialization errors
    let body = match body {
        Ok(json) => {
            rocket::info!("Successfully deserialized JSON request");
            json
        }
        Err(e) => {
            rocket::error!("JSON deserialization failed: {:?}", e);
            return Err((Status::BadRequest, format!("Invalid JSON: {}", e)));
        }
    };

    // Debug logging
    rocket::info!("Received create paste request");
    // Note: Cannot serialize CreatePasteRequest for logging since it doesn't implement Serialize

    let body = body.into_inner();
    rocket::info!(
        "Processing paste creation: content length={}, format={:?}, encryption={:?}",
        body.content.len(),
        body.format,
        body.encryption
            .as_ref()
            .map(|e| format!("{:?}", e.algorithm))
    );

    let created = create_paste_internal(store.inner(), body, &onion).await?;
    let response = CreatePasteResponse {
        id: created.id,
        path: created.path.clone(),
        shareable_url: created.path,
    };
    Ok(Json(response))
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
    id: String,
    query: PasteViewQuery,
    onion: OnionAccess,
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
                        trigger_webhook(config, event, &id, paste.metadata.bundle_label.clone());
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
    id: String,
    query: PasteViewQuery,
    onion: OnionAccess,
) -> Result<content::RawText<String>, Status> {
    match store.get_paste(&id).await {
        Ok(paste) => {
            if paste.metadata.tor_access_only && !onion.is_onion() {
                return Err(Status::Forbidden);
            }

            let now = current_timestamp();
            if evaluate_time_lock(&paste.metadata, now).is_some() {
                return Err(Status::Locked);
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
    Ok(WebhookConfig {
        url: request.url.clone(),
        provider: request.provider.clone(),
        view_template: request.view_template.clone(),
        burn_template: request.burn_template.clone(),
    })
}

async fn resolve_content(
    body: &CreatePasteRequest,
    _base_format: PasteFormat,
) -> Result<StoredContent, (Status, String)> {
    if let Some(enc) = &body.encryption {
        let algorithm = enc.algorithm;
        match algorithm {
            EncryptionAlgorithm::None => Ok(StoredContent::Plain {
                text: body.content.clone(),
            }),
            EncryptionAlgorithm::Aes256Gcm
            | EncryptionAlgorithm::ChaCha20Poly1305
            | EncryptionAlgorithm::XChaCha20Poly1305
            | EncryptionAlgorithm::KyberHybridAes256Gcm => {
                encrypt_content(&body.content, &enc.key, algorithm)
                    .await
                    .map_err(|e| (Status::BadRequest, e))
            }
        }
    } else {
        Ok(StoredContent::Plain {
            text: body.content.clone(),
        })
    }
}

async fn create_paste_internal(
    store: &SharedPasteStore,
    body: CreatePasteRequest,
    _onion: &OnionAccess,
) -> Result<CreatePasteResponse, (Status, String)> {
    // Validate content
    if body.content.trim().is_empty() {
        return Err((Status::BadRequest, "Content cannot be empty".into()));
    }

    // Resolve content (handle encryption)
    let content = resolve_content(&body, body.format.unwrap_or(PasteFormat::PlainText)).await?;

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

    // Handle stego
    if let Some(ref stego_req) = body.stego {
        match stego_req {
            StegoRequest::Builtin { carrier: _ } => {
                // For now, just store the original content
                // In a full implementation, this would embed the content in the carrier
            }
            StegoRequest::Uploaded { data_uri } => {
                // Parse and embed in uploaded carrier
                let _carrier = parse_data_uri(data_uri)
                    .map_err(|e| (Status::BadRequest, format!("Invalid data URI: {}", e)))?;
                // For now, just store the original content
            }
        }
    }

    // Handle bundle
    if let Some(ref bundle_req) = body.bundle {
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

    // Calculate expiration
    let expires_at = body
        .retention_minutes
        .map(|minutes| current_timestamp() + (minutes as i64 * 60));

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
    };

    // Store the paste
    let id = store.create_paste(paste).await;
    let path = format!("/{}", id);

    Ok(CreatePasteResponse {
        id: id.clone(),
        path: path.clone(),
        shareable_url: path,
    })
}

#[get("/")]
async fn index() -> content::RawHtml<String> {
    content::RawHtml(include_str!("../../static/index.html").to_string())
}

#[get("/about")]
async fn about() -> content::RawHtml<String> {
    content::RawHtml(include_str!("../../static/index.html").to_string())
}

#[get("/<_path..>")]
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
}
