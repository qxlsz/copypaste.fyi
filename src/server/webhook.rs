use std::net::IpAddr;
use std::time::Duration;

use url::{Host, Url};

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
    ///
    /// Redirects are disabled entirely: webhook URLs are validated against
    /// internal/private address ranges at paste-creation time, and following a
    /// redirect would let an attacker-controlled public URL 302 the request
    /// back into the internal network (SSRF).
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to build webhook HTTP client");
        WebhookClient(client)
    }
}

/// Validate a user-supplied webhook URL to prevent SSRF.
///
/// Rejects:
/// - non-http(s) schemes,
/// - IP-literal hosts in loopback / private / link-local / unique-local /
///   unspecified ranges (127.0.0.0/8, 10/8, 172.16/12, 192.168/16, 169.254/16,
///   0.0.0.0, ::1, ::, fc00::/7, fe80::/10, and IPv4-mapped equivalents),
/// - hostnames that plainly target internal infrastructure (`localhost`,
///   `*.localhost`, `*.internal`, `*.local`).
///
/// The shared [`WebhookClient`] additionally disables redirects so a public
/// URL cannot bounce the delivery into the internal network at send time.
pub fn validate_webhook_url(raw: &str) -> Result<(), String> {
    let url = Url::parse(raw.trim()).map_err(|e| format!("Webhook url is not a valid URL: {e}"))?;

    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(format!(
                "Webhook url scheme must be http or https, got '{other}'"
            ))
        }
    }

    match url.host() {
        None => Err("Webhook url must include a host".to_string()),
        Some(Host::Ipv4(ip)) if is_forbidden_ip(IpAddr::V4(ip)) => Err(format!(
            "Webhook url must not target a private, loopback, or link-local address ({ip})"
        )),
        Some(Host::Ipv6(ip)) if is_forbidden_ip(IpAddr::V6(ip)) => Err(format!(
            "Webhook url must not target a private, loopback, or link-local address ({ip})"
        )),
        Some(Host::Domain(domain)) if is_forbidden_hostname(domain) => Err(format!(
            "Webhook url must not target an internal hostname ('{domain}')"
        )),
        Some(_) => Ok(()),
    }
}

fn is_forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
        }
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_forbidden_ip(IpAddr::V4(mapped));
            }
            let first_segment = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || (first_segment & 0xfe00) == 0xfc00 // fc00::/7 unique local
                || (first_segment & 0xffc0) == 0xfe80 // fe80::/10 link local
        }
    }
}

fn is_forbidden_hostname(domain: &str) -> bool {
    let normalized = domain.trim_end_matches('.').to_ascii_lowercase();
    normalized == "localhost"
        || normalized.ends_with(".localhost")
        || normalized.ends_with(".internal")
        || normalized.ends_with(".local")
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

    // ── SSRF validation ────────────────────────────────────────────────────

    #[test]
    fn validate_accepts_public_urls() {
        for url in [
            "https://example.com/webhook",
            "http://example.com",
            "https://hooks.slack.com/services/T000/B000/XXX",
            "http://8.8.8.8/notify",
            "https://[2606:4700:4700::1111]/hook",
        ] {
            assert!(validate_webhook_url(url).is_ok(), "should accept {url}");
        }
    }

    #[test]
    fn validate_rejects_non_http_schemes() {
        for url in [
            "ftp://example.com/x",
            "file:///etc/passwd",
            "gopher://example.com",
        ] {
            assert!(validate_webhook_url(url).is_err(), "should reject {url}");
        }
    }

    #[test]
    fn validate_rejects_malformed_urls() {
        assert!(validate_webhook_url("not a url").is_err());
        assert!(validate_webhook_url("").is_err());
    }

    #[test]
    fn validate_rejects_private_ipv4_ranges() {
        for url in [
            "http://127.0.0.1/hook",
            "http://127.8.9.10:8080/hook",
            "http://10.0.0.1/hook",
            "http://172.16.5.5/hook",
            "http://172.31.255.255/hook",
            "http://192.168.1.1/hook",
            "http://169.254.169.254/latest/meta-data",
            "http://0.0.0.0/hook",
            "http://255.255.255.255/hook",
        ] {
            assert!(validate_webhook_url(url).is_err(), "should reject {url}");
        }
    }

    #[test]
    fn validate_rejects_private_ipv6_ranges() {
        for url in [
            "http://[::1]/hook",
            "http://[::]/hook",
            "http://[fc00::1]/hook",
            "http://[fd12:3456::1]/hook",
            "http://[fe80::1]/hook",
            "http://[::ffff:127.0.0.1]/hook",
            "http://[::ffff:10.0.0.1]/hook",
        ] {
            assert!(validate_webhook_url(url).is_err(), "should reject {url}");
        }
    }

    #[test]
    fn validate_rejects_internal_hostnames() {
        for url in [
            "http://localhost/hook",
            "http://localhost:9200/hook",
            "http://LOCALHOST/hook",
            "http://foo.localhost/hook",
            "http://metadata.internal/computeMetadata",
            "http://printer.local/hook",
            "http://db.internal./hook",
        ] {
            assert!(validate_webhook_url(url).is_err(), "should reject {url}");
        }
    }

    #[test]
    fn validate_accepts_domains_containing_but_not_ending_in_blocked_suffixes() {
        // "internal"/"local" only blocked as label suffixes, not substrings.
        for url in [
            "https://internal-tools.example.com/hook",
            "https://localhost.example.com/hook",
            "https://mylocal.example.com/hook",
        ] {
            assert!(validate_webhook_url(url).is_ok(), "should accept {url}");
        }
    }
}
