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
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq, Default)]
enum CliEncryption {
    #[value(name = "none")]
    #[default]
    None,
    #[value(name = "aes256_gcm")]
    Aes256Gcm,
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
    #[arg(long = "encryption", value_enum, default_value_t = CliEncryption::None)]
    encryption_mode: CliEncryption,

    /// Encryption key (required when --encryption aes256_gcm is set).
    #[arg(long = "key")]
    encryption_key: Option<String>,
}

#[derive(Serialize)]
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let content = if cli.stdin {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer.trim().to_owned()
    } else {
        cli.text.unwrap()
    };

    if content.is_empty() {
        eprintln!("No input provided.");
        std::process::exit(1);
    }

    let encryption = match cli.encryption_mode {
        CliEncryption::None => None,
        CliEncryption::Aes256Gcm => {
            let key = cli
                .encryption_key
                .as_deref()
                .filter(|k| !k.trim().is_empty())
                .unwrap_or_else(|| {
                    eprintln!("--key must be supplied when using --encryption aes256_gcm");
                    std::process::exit(1);
                });
            Some(EncryptionPayload {
                algorithm: "aes256_gcm",
                key,
            })
        }
    };

    let retention = if cli.retention == 0 {
        None
    } else {
        Some(cli.retention)
    };

    let payload = PastePayload {
        content: &content,
        format: match cli.format {
            CliFormat::PlainText => "plain_text",
            CliFormat::Markdown => "markdown",
            CliFormat::Code => "code",
            CliFormat::Json => "json",
        },
        retention_minutes: retention,
        encryption,
    };

    let base_url = cli.host.trim_end_matches('/');
    let client = reqwest::blocking::Client::builder().build()?;

    let response = client.post(base_url).json(&payload).send()?;

    if !response.status().is_success() {
        eprintln!("Request failed with status: {}", response.status());
        std::process::exit(1);
    }

    let path = response.text()?.trim().to_string();
    if path.is_empty() {
        eprintln!("Server returned an empty response.");
        std::process::exit(1);
    }

    let mut full_url = if path.starts_with("http://") || path.starts_with("https://") {
        path.clone()
    } else {
        format!("{}{}", base_url, path)
    };

    if cli.encryption_mode == CliEncryption::Aes256Gcm {
        if let Some(key) = cli.encryption_key.as_deref() {
            let separator = if full_url.contains('?') { '&' } else { '?' };
            full_url.push(separator);
            full_url.push_str("key=");
            full_url.push_str(&encode(key));
        }
    }

    println!("Paste link: {}", full_url);

    Ok(())
}
