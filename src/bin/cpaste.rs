use std::io::{self, Read};

use clap::Parser;
use clap::{ArgGroup, ValueEnum};
use serde::Serialize;
use urlencoding::encode;

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq, Default)]
enum CliFormat {
    #[value(name = "plain_text")]
    #[default]
    PlainText,
    #[value(name = "markdown")]
    Markdown,
    #[value(name = "code")]
    Code,
    #[value(name = "json")]
    Json,
    #[value(name = "go")]
    Go,
    #[value(name = "cpp")]
    Cpp,
    #[value(name = "kotlin")]
    Kotlin,
    #[value(name = "java")]
    Java,
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq, Default)]
enum CliEncryption {
    #[value(name = "none")]
    #[default]
    None,
    #[value(name = "aes256_gcm")]
    Aes256Gcm,
    #[value(name = "chacha20_poly1305")]
    ChaCha20Poly1305,
    #[value(name = "xchacha20_poly1305")]
    XChaCha20Poly1305,
}

/// Submit text to a copypaste.fyi instance and print the resulting URL.
#[derive(Parser, Debug)]
#[command(author, version, about)]
#[command(group(ArgGroup::new("input").args(["text", "stdin"]).required(true)))]
struct Cli {
    /// Text to paste. When omitted, stdin is read instead.
    #[arg(conflicts_with = "stdin")]
    text: Option<String>,

    /// Read input from stdin.
    #[arg(long)]
    stdin: bool,

    /// Base URL of the copypaste server (e.g. http://127.0.0.1:8000).
    #[arg(long, default_value = "http://127.0.0.1:8000")]
    host: String,

    /// Output rendering format.
    #[arg(long, value_enum, default_value_t = CliFormat::PlainText)]
    format: CliFormat,

    /// Retention window in minutes (0 = no expiry).
    #[arg(long, default_value_t = 0)]
    retention: u64,

    /// Encryption algorithm to use for this paste.
    #[arg(long, value_enum, default_value_t = CliEncryption::None)]
    encryption_mode: CliEncryption,

    /// Encryption key (required when encryption is not "none").
    #[arg(long = "key")]
    encryption_key: Option<String>,

    /// Delete the paste immediately after the first successful view.
    #[arg(long)]
    burn_after_reading: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct EncryptionPayload<'a> {
    algorithm: &'static str,
    key: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct PastePayload<'a> {
    content: &'a str,
    format: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    retention_minutes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encryption: Option<EncryptionPayload<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    burn_after_reading: Option<bool>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let url = execute(cli)?;
    println!("Paste link: {}", url);
    Ok(())
}

fn execute(cli: Cli) -> io::Result<String> {
    let Cli {
        text,
        stdin,
        host,
        format,
        retention,
        encryption_mode,
        encryption_key,
        burn_after_reading,
    } = cli;

    let content = if stdin {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer.trim().to_owned()
    } else {
        text.unwrap_or_default()
    };

    if content.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "No input provided.",
        ));
    }

    let key_ref = encryption_key.as_deref().filter(|k| !k.trim().is_empty());
    let encryption = match encryption_mode {
        CliEncryption::None => None,
        CliEncryption::Aes256Gcm => Some(EncryptionPayload {
            algorithm: "aes256_gcm",
            key: key_ref.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "--key must be supplied when using --encryption-mode aes256_gcm",
                )
            })?,
        }),
        CliEncryption::ChaCha20Poly1305 => Some(EncryptionPayload {
            algorithm: "chacha20_poly1305",
            key: key_ref.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "--key must be supplied when using --encryption-mode chacha20_poly1305",
                )
            })?,
        }),
        CliEncryption::XChaCha20Poly1305 => Some(EncryptionPayload {
            algorithm: "xchacha20_poly1305",
            key: key_ref.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "--key must be supplied when using --encryption-mode xchacha20_poly1305",
                )
            })?,
        }),
    };

    let has_encryption = encryption.is_some();

    let retention = if retention == 0 {
        None
    } else {
        Some(retention)
    };

    let payload = PastePayload {
        content: &content,
        format: match format {
            CliFormat::PlainText => "plain_text",
            CliFormat::Markdown => "markdown",
            CliFormat::Code => "code",
            CliFormat::Json => "json",
            CliFormat::Go => "go",
            CliFormat::Cpp => "cpp",
            CliFormat::Kotlin => "kotlin",
            CliFormat::Java => "java",
        },
        retention_minutes: retention,
        encryption: encryption.clone(),
        burn_after_reading: if burn_after_reading { Some(true) } else { None },
    };

    let base_url = host.trim_end_matches('/').to_owned();
    let client = reqwest::blocking::Client::builder()
        .build()
        .map_err(io::Error::other)?;

    let response = client
        .post(&base_url)
        .json(&payload)
        .send()
        .map_err(io::Error::other)?;

    if !response.status().is_success() {
        return Err(io::Error::other(format!(
            "Request failed with status: {}",
            response.status()
        )));
    }

    let path = response
        .text()
        .map_err(io::Error::other)?
        .trim()
        .to_string();

    if path.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Server returned an empty response.",
        ));
    }

    let mut full_url = if path.starts_with("http://") || path.starts_with("https://") {
        path
    } else {
        format!("{}{}", base_url, path)
    };

    if has_encryption {
        if let Some(key) = encryption_key.as_deref() {
            let separator = if full_url.contains('?') { '&' } else { '?' };
            full_url.push(separator);
            full_url.push_str("key=");
            full_url.push_str(&encode(key));
        }
    }

    Ok(full_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use serde_json::json;

    #[test]
    fn execute_submits_plain_text_and_returns_url() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/").json_body_partial(
                json!({ "content": "hello", "format": "plain_text" }).to_string(),
            );
            then.status(200).body("/paste/abc123");
        });

        let base = server.base_url();
        let cli = Cli::parse_from(["cpaste", "hello", "--host", base.as_str()]);
        let url = execute(cli).expect("url");
        assert_eq!(url, format!("{}/paste/abc123", base));
        mock.assert();
    }

    #[test]
    fn execute_appends_encryption_key() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/").json_body_partial(
                json!({ "encryption": { "algorithm": "aes256_gcm" } }).to_string(),
            );
            then.status(200).body("/secret");
        });

        let base = server.base_url();
        let cli = Cli::parse_from([
            "cpaste",
            "payload",
            "--host",
            base.as_str(),
            "--encryption-mode",
            "aes256_gcm",
            "--key",
            "super key",
        ]);
        let url = execute(cli).expect("url");
        assert_eq!(url, format!("{}/secret?key=super%20key", base));
        mock.assert();
    }

    #[test]
    fn execute_requires_key_for_encryption() {
        let cli = Cli::parse_from(["cpaste", "payload", "--encryption-mode", "aes256_gcm"]);
        let err = execute(cli).expect_err("missing key should fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err
            .to_string()
            .contains("--key must be supplied when using --encryption-mode"));
    }

    #[test]
    fn execute_rejects_empty_input() {
        let cli = Cli::parse_from(["cpaste", " "]);
        let err = execute(cli).expect_err("empty input should fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn execute_reports_http_error() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(500).body("error");
        });

        let base = server.base_url();
        let cli = Cli::parse_from(["cpaste", "hello", "--host", base.as_str()]);
        let err = execute(cli).expect_err("http failure expected");
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert!(err.to_string().contains("Request failed"));
        mock.assert();
    }
}
