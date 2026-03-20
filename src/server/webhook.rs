use std::time::Duration;

use crate::{WebhookConfig, WebhookProvider};

/// Shared HTTP client for webhook delivery, stored on Rocket state.
///
/// Using a single client avoids allocating a new TLS context and connection pool
/// on every delivery (BUG-002) and allows a uniform connect/request timeout to
/// be enforced so a slow webhook endpoint cannot stall Tokio worker threads
/// indefinitely (BUG-001).
pub struct WebhookClient(pub reqwest::Client);

impl WebhookClient {
    /// Build the shared client with a 5 s connect timeout and a 10 s overall
    /// request timeout.
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build webhook HTTP client");
        WebhookClient(client)
    }
}

impl Default for WebhookClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
pub enum WebhookEvent {
    Viewed,
    Consumed,
}

pub fn trigger_webhook(
    client: reqwest::Client,
    config: WebhookConfig,
    event: WebhookEvent,
    paste_id: &str,
    bundle_label: Option<String>,
) {
    let id = paste_id.to_string();
    tokio::spawn(async move {
        if let Err(err) = send_webhook(&client, config, event, id, bundle_label).await {
            eprintln!("webhook dispatch failed: {err}");
        }
    });
}

async fn send_webhook(
    client: &reqwest::Client,
    config: WebhookConfig,
    event: WebhookEvent,
    paste_id: String,
    bundle_label: Option<String>,
) -> Result<(), reqwest::Error> {
    let message = resolve_webhook_message(&config, event, &paste_id, bundle_label.as_deref());
    let payload = match config.provider {
        Some(WebhookProvider::Slack) | Some(WebhookProvider::Generic) | None => {
            serde_json::json!({ "text": message })
        }
        Some(WebhookProvider::Teams) => serde_json::json!({ "text": message }),
    };

    client
        .post(&config.url)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

fn resolve_webhook_message(
    config: &WebhookConfig,
    event: WebhookEvent,
    paste_id: &str,
    bundle_label: Option<&str>,
) -> String {
    let template = match event {
        WebhookEvent::Viewed => config.view_template.as_deref(),
        WebhookEvent::Consumed => config.burn_template.as_deref(),
    };

    let default = match event {
        WebhookEvent::Viewed => {
            if let Some(label) = bundle_label {
                format!("Bundle share '{label}' for paste {paste_id} was opened")
            } else {
                format!("Paste {paste_id} was opened")
            }
        }
        WebhookEvent::Consumed => {
            if let Some(label) = bundle_label {
                format!("Bundle share '{label}' for paste {paste_id} was consumed")
            } else {
                format!("Paste {paste_id} self-destructed")
            }
        }
    };

    if let Some(tpl) = template {
        apply_template(
            tpl,
            paste_id,
            bundle_label,
            match event {
                WebhookEvent::Viewed => "viewed",
                WebhookEvent::Consumed => "consumed",
            },
        )
    } else {
        default
    }
}

/// Substitute `{{id}}`, `{{event}}`, and `{{label}}` in `template`.
///
/// # BUG-004 — template injection via paste ID / label
///
/// If `id` or `label` contain `{{...}}` sequences, a naive sequential replace
/// would allow them to be re-processed as template placeholders in a later
/// substitution pass.  For example, an `id` of `{{event}}` would survive the
/// `{{id}}` replacement unchanged and then be replaced by "viewed"/"consumed"
/// in the `{{event}}` pass.
///
/// To prevent this, `id` and `label` are sanitised by stripping `{{` and `}}`
/// before substitution.  `event` is always an internal constant so it does
/// not need sanitisation.
fn apply_template(template: &str, id: &str, label: Option<&str>, event: &str) -> String {
    let safe_id = id.replace("{{", "").replace("}}", "");
    let safe_label = label.unwrap_or("").replace("{{", "").replace("}}", "");
    let mut result = template.replace("{{id}}", &safe_id);
    result = result.replace("{{event}}", event);
    result = result.replace("{{label}}", &safe_label);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config() -> WebhookConfig {
        WebhookConfig {
            url: "https://example.test/webhook".into(),
            provider: Some(WebhookProvider::Generic),
            view_template: None,
            burn_template: None,
        }
    }

    #[test]
    fn default_view_message_without_label() {
        let config = base_config();
        let message = resolve_webhook_message(&config, WebhookEvent::Viewed, "abc123", None);
        assert_eq!(message, "Paste abc123 was opened");
    }

    #[test]
    fn default_consumed_message_with_label() {
        let config = base_config();
        let message = resolve_webhook_message(
            &config,
            WebhookEvent::Consumed,
            "xyz789",
            Some("Premium bundle"),
        );
        assert_eq!(
            message,
            "Bundle share 'Premium bundle' for paste xyz789 was consumed"
        );
    }

    #[test]
    fn template_is_applied_with_placeholders() {
        let mut config = base_config();
        config.view_template = Some("Paste {{id}} was {{event}} by {{label}}".into());

        let output = resolve_webhook_message(&config, WebhookEvent::Viewed, "p123", Some("Alice"));

        assert_eq!(output, "Paste p123 was viewed by Alice");
    }

    #[test]
    fn apply_template_handles_missing_label() {
        let rendered = apply_template("{{id}} {{event}} {{label}}", "id", None, "viewed");
        assert_eq!(rendered, "id viewed ");
    }

    /// BUG-004: an id containing `{{event}}` must not cascade into the event
    /// placeholder substitution and produce "viewed" or "consumed" in place of
    /// the literal id text.
    #[test]
    fn apply_template_sanitises_id_containing_braces() {
        let rendered = apply_template("{{id}}", "{{event}}", None, "viewed");
        // After sanitisation `{{event}}` becomes `event`, not "viewed".
        assert_eq!(rendered, "event");
        assert!(!rendered.contains("viewed"));
    }

    /// BUG-004: a user-supplied label must not inject additional placeholders.
    #[test]
    fn apply_template_sanitises_label_containing_braces() {
        let rendered = apply_template("{{label}}", "id", Some("{{id}}"), "consumed");
        // After sanitisation `{{id}}` in label becomes `id`, not the paste id.
        assert_eq!(rendered, "id");
    }

    #[test]
    fn webhook_client_new_builds_successfully() {
        // Smoke-test that building the shared client does not panic.
        let _ = WebhookClient::new();
    }
}
