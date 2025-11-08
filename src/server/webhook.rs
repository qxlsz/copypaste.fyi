use crate::{WebhookConfig, WebhookProvider};

#[derive(Clone, Copy)]
pub enum WebhookEvent {
    Viewed,
    Consumed,
}

pub fn trigger_webhook(
    config: WebhookConfig,
    event: WebhookEvent,
    paste_id: &str,
    bundle_label: Option<String>,
) {
    let id = paste_id.to_string();
    tokio::spawn(async move {
        if let Err(err) = send_webhook(config, event, id, bundle_label).await {
            eprintln!("webhook dispatch failed: {err}");
        }
    });
}

async fn send_webhook(
    config: WebhookConfig,
    event: WebhookEvent,
    paste_id: String,
    bundle_label: Option<String>,
) -> Result<(), reqwest::Error> {
    let client = reqwest::Client::new();
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

fn apply_template(template: &str, id: &str, label: Option<&str>, event: &str) -> String {
    let mut result = template.replace("{{id}}", id);
    result = result.replace("{{event}}", event);
    result = result.replace("{{label}}", label.unwrap_or(""));
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
}
