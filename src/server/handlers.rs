use std::path::PathBuf;

use copypaste::{
    create_paste_store, AttestationRequirement, BundleMetadata, BundlePointer, EncryptionAlgorithm,
    PasteError, PasteFormat, PasteMetadata, PersistenceLocator, SharedPasteStore, StoredContent,
    StoredPaste, WebhookConfig,
};
use rocket::fs::{FileServer, NamedFile};
use rocket::http::Status;
use rocket::response::content;
use rocket::serde::json::Json;
use rocket::{get, post, routes, Build, Rocket, State};

use super::attestation::{self, AttestationRequest, AttestationVerdict};
use super::bundles::build_bundle_overview;
use super::crypto::{decrypt_content, encrypt_content, DecryptError};
use super::models::{
    CreatePasteRequest, PasteViewQuery, PersistenceRequest, TimeLockRequest, WebhookRequest,
};
use super::render::{
    layout, render_attestation_prompt, render_expired, render_invalid_key, render_key_prompt,
    render_paste_view, render_time_locked, StoredPasteView,
};
use super::time::{current_timestamp, evaluate_time_lock, parse_timestamp};
use super::webhook::{trigger_webhook, WebhookEvent};

pub fn build_rocket(store: SharedPasteStore) -> Rocket<Build> {
    rocket::build()
        .manage(store)
        .mount("/", routes![index, create, show, show_raw, static_files])
        .mount("/", FileServer::from("static"))
}

pub async fn launch() -> Result<(), Box<dyn std::error::Error>> {
    let config = rocket::Config {
        address: "0.0.0.0".parse()?,
        port: 8000,
        ..rocket::Config::debug_default()
    };

    build_rocket(create_paste_store())
        .configure(config)
        .launch()
        .await?;

    Ok(())
}

#[get("/")]
async fn index() -> content::RawHtml<&'static str> {
    content::RawHtml(include_str!("../../static/index.html"))
}

#[post("/", data = "<body>")]
async fn create(
    store: &State<SharedPasteStore>,
    body: Json<CreatePasteRequest>,
) -> Result<String, (Status, String)> {
    let body = body.into_inner();
    let now = current_timestamp();
    let format = body.format.unwrap_or_default();
    let expires_at = body.retention_minutes.and_then(|mins| match mins {
        0 => None,
        minutes => Some(now + i64::try_from(minutes).unwrap_or(0) * 60),
    });
    let burn_after_reading = body.burn_after_reading;

    let mut metadata = PasteMetadata::default();

    if let Some(lock) = body.time_lock.as_ref() {
        apply_time_lock(lock, &mut metadata)?;
    }

    if let Some(attestation) = body.attestation.as_ref() {
        metadata.attestation = Some(
            attestation::requirement_from_request(attestation)
                .map_err(|e| (Status::BadRequest, e))?,
        );
    }

    if let Some(persistence) = body.persistence.as_ref() {
        metadata.persistence = Some(persistence_locator_from_request(persistence)?);
    }

    if let Some(webhook) = body.webhook.as_ref() {
        metadata.webhook = Some(webhook_config_from_request(webhook)?);
    }

    let content = resolve_content(&body, format)?;

    let mut bundle_children: Vec<BundlePointer> = Vec::new();

    if let Some(bundle) = body.bundle.as_ref() {
        if !bundle.children.is_empty() {
            let enc = body.encryption.as_ref().ok_or_else(|| {
                (
                    Status::BadRequest,
                    "Bundle creation requires an encryption key".to_string(),
                )
            })?;

            if matches!(enc.algorithm, EncryptionAlgorithm::None) {
                return Err((
                    Status::BadRequest,
                    "Bundle creation requires a non-zero encryption algorithm".to_string(),
                ));
            }

            for child in &bundle.children {
                let encrypted_child = encrypt_content(&child.content, &enc.key, enc.algorithm)
                    .map_err(|e| {
                        (
                            Status::BadRequest,
                            format!("failed to encrypt bundle child: {e}"),
                        )
                    })?;
                let mut child_metadata = metadata.clone();
                child_metadata.bundle = None;
                child_metadata.bundle_label = child.label.clone();
                let child_paste = StoredPaste {
                    content: encrypted_child,
                    format: child.format.unwrap_or(format),
                    created_at: now,
                    expires_at,
                    burn_after_reading: true,
                    metadata: child_metadata,
                };
                let child_id = store.create_paste(child_paste).await;
                bundle_children.push(BundlePointer {
                    id: child_id,
                    label: child.label.clone(),
                });
            }
        }
    }

    if !bundle_children.is_empty() {
        metadata.bundle = Some(BundleMetadata {
            children: bundle_children,
        });
    }

    let paste = StoredPaste {
        content,
        format,
        created_at: now,
        expires_at,
        burn_after_reading,
        metadata,
    };

    let id = store.create_paste(paste).await;
    Ok(format!("/{}", id))
}

#[get("/<id>?<query..>")]
async fn show(
    store: &State<SharedPasteStore>,
    id: String,
    query: PasteViewQuery,
) -> Result<content::RawHtml<String>, Status> {
    match store.get_paste(&id).await {
        Ok(paste) => {
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
) -> Result<content::RawText<String>, Status> {
    match store.get_paste(&id).await {
        Ok(paste) => {
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

#[get("/static/<path..>")]
async fn static_files(path: PathBuf) -> Option<NamedFile> {
    NamedFile::open(PathBuf::from("static").join(path))
        .await
        .ok()
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

fn resolve_content(
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
            | EncryptionAlgorithm::XChaCha20Poly1305 => {
                encrypt_content(&body.content, &enc.key, algorithm)
                    .map_err(|e| (Status::BadRequest, e))
            }
        }
    } else {
        Ok(StoredContent::Plain {
            text: body.content.clone(),
        })
    }
}

pub fn format_attestation_requirement(requirement: &AttestationRequirement) -> AttestationRequest {
    match requirement {
        AttestationRequirement::Totp {
            secret,
            digits,
            step,
            allowed_drift,
            issuer,
        } => AttestationRequest::Totp {
            secret: secret.clone(),
            digits: Some(*digits),
            step: Some(*step),
            allowed_drift: Some(*allowed_drift),
            issuer: issuer.clone(),
        },
        AttestationRequirement::SharedSecret { .. } => AttestationRequest::SharedSecret {
            secret: String::new(),
        },
    }
}

pub fn render_bundle_empty() -> content::RawHtml<String> {
    content::RawHtml(layout("copypaste.fyi", "<p>No bundle content.</p>".into()))
}
