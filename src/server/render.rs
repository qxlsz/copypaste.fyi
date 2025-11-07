use crate::{
    AttestationRequirement, EncryptionAlgorithm, PasteFormat, PasteMetadata, PersistenceLocator,
    StoredContent, WebhookProvider,
};
use html_escape::encode_safe;
use pulldown_cmark::{html, Options, Parser};

use super::time::format_timestamp;

pub fn layout(title: &str, body: String) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{title}</title>
    <link rel="stylesheet" href="/static/view.css" />
</head>
<body>
    <header>
        <h1><a href="/">copypaste.fyi</a></h1>
    </header>
    <main>
        {body}
    </main>
</body>
</html>
"#,
        title = encode_safe(title),
        body = body,
    )
}

pub fn render_paste_view(
    id: &str,
    paste: &StoredPasteView,
    text: &str,
    bundle_html: Option<String>,
) -> String {
    let rendered_body = match paste.format {
        PasteFormat::PlainText => format_plain(text),
        PasteFormat::Markdown => format_markdown(text),
        PasteFormat::Json => format_json(text),
        PasteFormat::Code
        | PasteFormat::Javascript
        | PasteFormat::Typescript
        | PasteFormat::Python
        | PasteFormat::Rust
        | PasteFormat::Go
        | PasteFormat::Cpp
        | PasteFormat::Kotlin
        | PasteFormat::Java
        | PasteFormat::Csharp
        | PasteFormat::Php
        | PasteFormat::Ruby
        | PasteFormat::Bash
        | PasteFormat::Yaml
        | PasteFormat::Sql
        | PasteFormat::Swift
        | PasteFormat::Html
        | PasteFormat::Css => format_code(text),
    };

    let created = format_timestamp(paste.created_at);
    let retention = paste
        .expires_at
        .map(format_timestamp)
        .unwrap_or_else(|| "No expiry".to_string());

    let encryption = match paste.content {
        StoredContent::Plain { .. } => "None".to_string(),
        StoredContent::Encrypted { ref algorithm, .. }
        | StoredContent::Stego { ref algorithm, .. } => match algorithm {
            EncryptionAlgorithm::None => "None".to_string(),
            EncryptionAlgorithm::Aes256Gcm => "AES-256-GCM".to_string(),
            EncryptionAlgorithm::ChaCha20Poly1305 => "ChaCha20-Poly1305".to_string(),
            EncryptionAlgorithm::XChaCha20Poly1305 => "XChaCha20-Poly1305".to_string(),
            EncryptionAlgorithm::KyberHybridAes256Gcm => "Kyber Hybrid AES-256-GCM".to_string(),
        },
    };

    let burn_status = if paste.burn_after_reading {
        "Yes (link disabled after this view)".to_string()
    } else {
        "No".to_string()
    };

    let burn_note = if paste.burn_after_reading {
        r#"<p class="burn-note">This paste was configured to burn after reading. The link is now invalid for future visits.</p>"#.to_string()
    } else {
        String::new()
    };

    let time_lock = match (paste.metadata.not_before, paste.metadata.not_after) {
        (None, None) => "None".to_string(),
        (Some(start), Some(end)) => {
            format!("{} → {}", format_timestamp(start), format_timestamp(end))
        }
        (Some(start), None) => format!("After {}", format_timestamp(start)),
        (None, Some(end)) => format!("Before {}", format_timestamp(end)),
    };

    let attestation = match paste.metadata.attestation {
        None => "None".to_string(),
        Some(AttestationRequirement::Totp { ref issuer, .. }) => issuer
            .as_ref()
            .map(|iss| format!("TOTP ({iss})"))
            .unwrap_or_else(|| "TOTP".to_string()),
        Some(AttestationRequirement::SharedSecret { .. }) => "Shared secret".to_string(),
    };

    let persistence = paste
        .metadata
        .persistence
        .as_ref()
        .map(|locator| match locator {
            PersistenceLocator::Memory => "Memory".to_string(),
            PersistenceLocator::Vault { key_path } => format!("Vault ({})", key_path),
            PersistenceLocator::S3 { bucket, prefix } => match prefix {
                Some(p) if !p.is_empty() => format!("S3 {bucket}/{p}"),
                _ => format!("S3 {bucket}"),
            },
        })
        .unwrap_or_else(|| "Ephemeral".to_string());

    let webhook = paste
        .metadata
        .webhook
        .as_ref()
        .map(|config| match config.provider {
            Some(WebhookProvider::Slack) => "Slack".to_string(),
            Some(WebhookProvider::Teams) => "Teams".to_string(),
            Some(WebhookProvider::Generic) => "Webhook".to_string(),
            None => "Webhook".to_string(),
        })
        .unwrap_or_else(|| "None".to_string());

    let bundle_summary = paste
        .metadata
        .bundle
        .as_ref()
        .map(|bundle| format!("{} link(s)", bundle.children.len()))
        .unwrap_or_else(|| "None".to_string());

    let bundle_section = bundle_html.unwrap_or_default();

    layout(
        "copypaste.fyi | View paste",
        format!(
            r#"<section class="meta">
    <div><strong>ID:</strong> {id}</div>
    <div><strong>Format:</strong> {format:?}</div>
    <div><strong>Created:</strong> {created}</div>
    <div><strong>Retention:</strong> {retention}</div>
    <div><strong>Encryption:</strong> {encryption}</div>
    <div><strong>Burn after reading:</strong> {burn}</div>
    <div><strong>Time lock:</strong> {time_lock}</div>
    <div><strong>Attestation:</strong> {attestation}</div>
    <div><strong>Persistence:</strong> {persistence}</div>
    <div><strong>Webhook:</strong> {webhook}</div>
    <div><strong>Bundle:</strong> {bundle_summary}</div>
</section>
<article class="content">
    {burn_note}
    {bundle_section}
    {rendered_body}
</article>
"#,
            id = encode_safe(id),
            format = paste.format,
            created = created,
            retention = retention,
            encryption = encryption,
            burn = burn_status,
            burn_note = burn_note,
            time_lock = encode_safe(&time_lock),
            attestation = encode_safe(&attestation),
            persistence = encode_safe(&persistence),
            webhook = encode_safe(&webhook),
            bundle_summary = encode_safe(&bundle_summary),
            bundle_section = bundle_section,
            rendered_body = rendered_body,
        ),
    )
}

pub fn render_time_locked(state: super::time::TimeLockState) -> String {
    let (heading, message) = match state {
        super::time::TimeLockState::TooEarly(ts) => (
            "Time-locked paste",
            format!(
                "This paste unlocks after {}.",
                encode_safe(&format_timestamp(ts))
            ),
        ),
        super::time::TimeLockState::TooLate(ts) => (
            "Time window elapsed",
            format!(
                "Access window closed at {}.",
                encode_safe(&format_timestamp(ts))
            ),
        ),
    };

    layout(
        "copypaste.fyi | Locked",
        format!(
            r#"<section class="notice">
    <h2>{heading}</h2>
    <p>{message}</p>
    <p class="hint">Bookmark this link and try again when the unlock window is active.</p>
</section>
"#,
            heading = heading,
            message = message,
        ),
    )
}

pub fn render_attestation_prompt(
    id: &str,
    needs_key_field: bool,
    existing_key: Option<&str>,
    requirement: &AttestationRequirement,
    invalid: bool,
) -> String {
    let (prompt_label, field_name, field_type, helper) = match requirement {
        AttestationRequirement::Totp { issuer, .. } => (
            issuer
                .as_ref()
                .map(|name| format!("One-time code ({name})"))
                .unwrap_or_else(|| "One-time code".to_string()),
            "code",
            "text",
            "Enter the current code from your authenticator.",
        ),
        AttestationRequirement::SharedSecret { .. } => (
            "Shared secret".to_string(),
            "attest",
            "password",
            "Provide the shared secret agreed upon with the sender.",
        ),
    };

    let mut form_inputs = String::new();

    if needs_key_field {
        form_inputs.push_str(
            r#"        <label for="key">Encryption key</label>
        <input type="password" name="key" id="key" required />
"#,
        );
    } else if let Some(key) = existing_key {
        let escaped = encode_safe(key);
        form_inputs.push_str(&format!(
            "        <input type=\"hidden\" name=\"key\" value=\"{escaped}\" />\n"
        ));
    }

    form_inputs.push_str(&format!(
        "        <label for=\"{field_name}\">{prompt_label}</label>\n",
        field_name = field_name,
        prompt_label = encode_safe(&prompt_label),
    ));

    let mut field_attributes = String::new();
    if matches!(requirement, AttestationRequirement::Totp { .. }) {
        field_attributes.push_str(" pattern=\"[0-9]{6,10}\"");
        field_attributes.push_str(" inputmode=\"numeric\"");
    }

    form_inputs.push_str(&format!(
        "        <input type=\"{field_type}\" name=\"{field_name}\" id=\"{field_name}\" required{attrs} />\n",
        field_type = field_type,
        field_name = field_name,
        attrs = field_attributes,
    ));

    let error = if invalid {
        "<p class=\"error\">Verification failed. Double-check your entry and try again.</p>\n"
            .to_string()
    } else {
        String::new()
    };

    layout(
        "copypaste.fyi | Verification required",
        format!(
            r#"<section class="notice">
    <h2>Additional verification required</h2>
    <p>{helper}</p>
    {error}
    <form method="get" action="/{id}">
{inputs}        <button type="submit">Continue</button>
    </form>
</section>
"#,
            helper = encode_safe(helper),
            error = error,
            inputs = form_inputs,
            id = encode_safe(id),
        ),
    )
}

pub fn render_key_prompt(id: &str) -> String {
    layout(
        "copypaste.fyi | Encrypted paste",
        format!(
            r#"<section class="notice">
    <h2>This paste is encrypted</h2>
    <p>Provide the encryption key to view the content.</p>
    <form method="get" action="/{id}">
        <label for="key">Encryption key</label>
        <input type="password" name="key" id="key" required />
        <button type="submit">Decrypt</button>
    </form>
</section>
"#,
            id = encode_safe(id),
        ),
    )
}

pub fn render_invalid_key(id: &str) -> String {
    layout(
        "copypaste.fyi | Invalid key",
        format!(
            r#"<section class="notice error">
    <h2>Invalid encryption key</h2>
    <p>The key you entered could not decrypt this paste.</p>
    <form method="get" action="/{id}">
        <label for="key">Try again</label>
        <input type="password" name="key" id="key" required />
        <button type="submit">Decrypt</button>
    </form>
</section>
"#,
            id = encode_safe(id),
        ),
    )
}

pub fn render_expired(id: &str) -> String {
    layout(
        "copypaste.fyi | Paste expired",
        format!(
            r#"<section class="notice error">
    <h2>Paste expired</h2>
    <p>Paste {id} has reached its retention limit and is no longer available.</p>
</section>
"#,
            id = encode_safe(id),
        ),
    )
}

pub struct StoredPasteView<'a> {
    pub content: &'a StoredContent,
    pub format: PasteFormat,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub burn_after_reading: bool,
    pub metadata: &'a PasteMetadata,
}

pub fn format_plain(text: &str) -> String {
    format!("<pre>{}</pre>", encode_safe(text))
}

pub fn format_markdown(text: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(text, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

pub fn format_code(text: &str) -> String {
    format!("<pre><code>{}</code></pre>", encode_safe(text))
}

pub fn format_json(text: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) => {
            format_code(&serde_json::to_string_pretty(&value).unwrap_or_else(|_| text.to_string()))
        }
        Err(_) => format_code(text),
    }
}
