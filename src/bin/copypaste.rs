use std::io::{self, IsTerminal, Read};

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use urlencoding::encode;

#[derive(Parser)]
#[command(name = "copypaste", about = "Open-source paste sharing service")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the HTTP server
    Serve {
        /// Path to a TOML config file
        #[arg(long)]
        config: Option<String>,
    },
    /// Submit text to a copypaste instance and print the resulting URL
    Send(SendArgs),
    /// Config file management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Print an annotated example config to stdout, or write it to --path
    Init {
        /// Write the generated config to this file instead of stdout
        #[arg(long)]
        path: Option<String>,
    },
}

/// Arguments for the `send` subcommand.
#[derive(Parser, Debug)]
struct SendArgs {
    /// Text to paste. When omitted, reads from piped stdin.
    #[arg(conflicts_with = "stdin")]
    text: Option<String>,

    /// Read input from stdin (explicit; piped stdin is auto-detected).
    #[arg(long)]
    stdin: bool,

    /// Base URL of the copypaste server (e.g. http://127.0.0.1:8000).
    #[arg(long, default_value = "http://127.0.0.1:8000")]
    host: String,

    /// Output rendering format.
    #[arg(long, value_enum, default_value_t = CliFormat::PlainText)]
    format: CliFormat,

    /// TTL for the paste, e.g. 5m, 2h, 7d, 1w. Overrides --retention.
    #[arg(long, conflicts_with = "retention")]
    ttl: Option<String>,

    /// Retention window in minutes (0 = no expiry). Use --ttl for human-friendly units.
    #[arg(long, default_value_t = 0)]
    retention: u64,

    /// Encryption algorithm to use for this paste.
    #[arg(long, value_enum, default_value_t = CliEncryption::None)]
    encryption_mode: CliEncryption,

    /// Encryption key (required when encryption is not "none").
    #[arg(long = "key")]
    encryption_key: Option<String>,

    /// Delete the paste immediately after the first successful view.
    #[arg(long, alias = "burn")]
    burn_after_reading: bool,
}

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
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Command::Serve { config } => {
            use copypaste::server::{config::Config, handlers};

            let config = Config::load(config.as_deref()).map_err(|e| format!("{e}"))?;
            config.bridge_to_env();

            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?
                .block_on(handlers::launch())
        }
        Command::Send(args) => {
            let url = execute_send(args)?;
            if io::stdout().is_terminal() {
                println!("Paste link: {}", url);
            } else {
                println!("{}", url);
            }
            Ok(())
        }
        Command::Config { action } => match action {
            ConfigAction::Init { path } => {
                let content = copypaste::server::config::EXAMPLE_CONFIG;
                match path {
                    Some(p) => {
                        std::fs::write(&p, content)?;
                        println!("Config written to {p}");
                    }
                    None => print!("{content}"),
                }
                Ok(())
            }
        },
    }
}

fn parse_ttl(s: &str) -> io::Result<u64> {
    let s = s.trim();
    if let Ok(n) = s.parse::<u64>() {
        return Ok(n);
    }
    let (value, mult) = if let Some(rest) = s.strip_suffix('m') {
        (rest, 1u64)
    } else if let Some(rest) = s.strip_suffix('h') {
        (rest, 60u64)
    } else if let Some(rest) = s.strip_suffix('d') {
        (rest, 60u64 * 24)
    } else if let Some(rest) = s.strip_suffix('w') {
        (rest, 60u64 * 24 * 7)
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Invalid TTL '{s}'. Use e.g. 5m, 2h, 7d, 1w or a raw number of minutes."),
        ));
    };
    value.trim().parse::<u64>().map(|n| n * mult).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Invalid TTL '{s}'. Use e.g. 5m, 2h, 7d, 1w or a raw number of minutes."),
        )
    })
}

fn execute_send(args: SendArgs) -> io::Result<String> {
    let SendArgs {
        text,
        stdin,
        host,
        format,
        ttl,
        retention,
        encryption_mode,
        encryption_key,
        burn_after_reading,
    } = args;

    let content = if let Some(t) = text {
        t
    } else if stdin || !io::stdin().is_terminal() {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer.trim().to_owned()
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "No input provided. Pass text as an argument or pipe it via stdin.",
        ));
    };

    if content.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "No input provided.",
        ));
    }

    let retention_minutes = if let Some(ttl_str) = ttl {
        let mins = parse_ttl(&ttl_str)?;
        if mins == 0 {
            None
        } else {
            Some(mins)
        }
    } else if retention == 0 {
        None
    } else {
        Some(retention)
    };

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
        retention_minutes,
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
    fn send_submits_plain_text_and_returns_url() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/").json_body_partial(
                json!({ "content": "hello", "format": "plain_text" }).to_string(),
            );
            then.status(200).body("/paste/abc123");
        });

        let base = server.base_url();
        let args = SendArgs::parse_from(["copypaste-send", "hello", "--host", base.as_str()]);
        let url = execute_send(args).expect("url");
        assert_eq!(url, format!("{}/paste/abc123", base));
        mock.assert();
    }

    #[test]
    fn send_appends_encryption_key() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/").json_body_partial(
                json!({ "encryption": { "algorithm": "aes256_gcm" } }).to_string(),
            );
            then.status(200).body("/secret");
        });

        let base = server.base_url();
        let args = SendArgs::parse_from([
            "copypaste-send",
            "payload",
            "--host",
            base.as_str(),
            "--encryption-mode",
            "aes256_gcm",
            "--key",
            "super key",
        ]);
        let url = execute_send(args).expect("url");
        assert_eq!(url, format!("{}/secret?key=super%20key", base));
        mock.assert();
    }

    #[test]
    fn send_requires_key_for_encryption() {
        let args = SendArgs::parse_from([
            "copypaste-send",
            "payload",
            "--encryption-mode",
            "aes256_gcm",
        ]);
        let err = execute_send(args).expect_err("missing key should fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err
            .to_string()
            .contains("--key must be supplied when using --encryption-mode"));
    }

    #[test]
    fn send_rejects_empty_input() {
        let args = SendArgs::parse_from(["copypaste-send", " "]);
        let err = execute_send(args).expect_err("empty input should fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn send_reports_http_error() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(500).body("error");
        });

        let base = server.base_url();
        let args = SendArgs::parse_from(["copypaste-send", "hello", "--host", base.as_str()]);
        let err = execute_send(args).expect_err("http failure expected");
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert!(err.to_string().contains("Request failed"));
        mock.assert();
    }

    #[test]
    fn parse_ttl_minutes() {
        assert_eq!(parse_ttl("5m").unwrap(), 5);
        assert_eq!(parse_ttl("30m").unwrap(), 30);
    }

    #[test]
    fn parse_ttl_hours() {
        assert_eq!(parse_ttl("2h").unwrap(), 120);
        assert_eq!(parse_ttl("1h").unwrap(), 60);
    }

    #[test]
    fn parse_ttl_days() {
        assert_eq!(parse_ttl("1d").unwrap(), 1440);
        assert_eq!(parse_ttl("7d").unwrap(), 10080);
    }

    #[test]
    fn parse_ttl_weeks() {
        assert_eq!(parse_ttl("1w").unwrap(), 10080);
        assert_eq!(parse_ttl("2w").unwrap(), 20160);
    }

    #[test]
    fn parse_ttl_raw_minutes() {
        assert_eq!(parse_ttl("60").unwrap(), 60);
        assert_eq!(parse_ttl("0").unwrap(), 0);
    }

    #[test]
    fn parse_ttl_invalid() {
        assert!(parse_ttl("5x").is_err());
        assert!(parse_ttl("abc").is_err());
        assert!(parse_ttl("").is_err());
    }

    #[test]
    fn send_ttl_flag_sends_retention_minutes() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .json_body_partial(json!({ "retention_minutes": 120 }).to_string());
            then.status(200).body("/paste/timed");
        });

        let base = server.base_url();
        let args = SendArgs::parse_from([
            "copypaste-send",
            "hello",
            "--host",
            base.as_str(),
            "--ttl",
            "2h",
        ]);
        let url = execute_send(args).expect("url");
        assert_eq!(url, format!("{}/paste/timed", base));
        mock.assert();
    }

    #[test]
    fn send_burn_alias_sends_burn_after_reading() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .json_body_partial(json!({ "burn_after_reading": true }).to_string());
            then.status(200).body("/paste/burned");
        });

        let base = server.base_url();
        let args =
            SendArgs::parse_from(["copypaste-send", "hello", "--host", base.as_str(), "--burn"]);
        let url = execute_send(args).expect("url");
        assert_eq!(url, format!("{}/paste/burned", base));
        mock.assert();
    }
}
