use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

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

    /// Read the write bearer token from an owner-only file. If omitted,
    /// COPYPASTE_AUTH_TOKEN is used when set. Tokens are never accepted in argv.
    #[arg(long, value_name = "PATH")]
    auth_token_file: Option<PathBuf>,

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

    /// Read the encryption key from an owner-only file. If omitted,
    /// COPYPASTE_ENCRYPTION_KEY is used. Keys are never accepted in argv.
    #[arg(long, value_name = "PATH")]
    encryption_key_file: Option<PathBuf>,

    /// Request best-effort deletion after a successful view.
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
                println!("Paste link: {url}");
            } else {
                println!("{url}");
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
    value
        .trim()
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(mult))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid TTL '{s}'. Use e.g. 5m, 2h, 7d, 1w or a raw number of minutes."),
            )
        })
}

fn load_secret(
    path: Option<&Path>,
    environment_name: &str,
    label: &str,
    maximum_bytes: usize,
) -> io::Result<Option<String>> {
    let token = if let Some(path) = path {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        // Open exactly once, refusing symlinks on Unix, then validate and read
        // through the same descriptor to avoid path replacement races.
        let file = options.open(path)?;
        let metadata = file.metadata()?;
        if !metadata.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{label} path must be a regular file."),
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "{label} file must not be accessible by group or other users (mode 0600)."
                    ),
                ));
            }
        }
        if metadata.len() > maximum_bytes as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{label} file is too large."),
            ));
        }
        let mut value = String::new();
        file.take(maximum_bytes as u64 + 1)
            .read_to_string(&mut value)?;
        if value.len() > maximum_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{label} file is too large."),
            ));
        }
        Some(value)
    } else {
        std::env::var(environment_name).ok()
    };

    token
        .map(|value| {
            let value = value.trim().to_owned();
            if value.is_empty()
                || value.len() > maximum_bytes
                || value.bytes().any(|byte| matches!(byte, b'\r' | b'\n'))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "{label} must be a single non-empty value no larger than {maximum_bytes} bytes."
                    ),
                ));
            }
            Ok(value)
        })
        .transpose()
}

fn validated_base_url(host: &str) -> io::Result<reqwest::Url> {
    let mut url = reqwest::Url::parse(host.trim())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "--host must be a valid URL"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--host must be an http(s) origin without credentials, query, or fragment",
        ));
    }
    let loopback = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        None => false,
    };
    if url.scheme() == "http" && !loopback {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Remote --host URLs must use HTTPS; plain HTTP is allowed only for loopback development.",
        ));
    }
    let normalized_path = url.path().trim_end_matches('/').to_owned();
    url.set_path(&normalized_path);
    Ok(url)
}

fn execute_send(args: SendArgs) -> io::Result<String> {
    let SendArgs {
        text,
        stdin,
        host,
        auth_token_file,
        format,
        ttl,
        retention,
        encryption_mode,
        encryption_key_file,
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

    let encryption_key = load_secret(
        encryption_key_file.as_deref(),
        "COPYPASTE_ENCRYPTION_KEY",
        "Encryption key",
        1024,
    )?;
    let key_ref = encryption_key.as_deref();
    let encryption = match encryption_mode {
        CliEncryption::None => None,
        CliEncryption::Aes256Gcm => Some(EncryptionPayload {
            algorithm: "aes256_gcm",
            key: key_ref.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "--encryption-key-file or COPYPASTE_ENCRYPTION_KEY must be supplied when using --encryption-mode aes256_gcm",
                )
            })?,
        }),
        CliEncryption::ChaCha20Poly1305 => Some(EncryptionPayload {
            algorithm: "chacha20_poly1305",
            key: key_ref.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "--encryption-key-file or COPYPASTE_ENCRYPTION_KEY must be supplied when using --encryption-mode chacha20_poly1305",
                )
            })?,
        }),
        CliEncryption::XChaCha20Poly1305 => Some(EncryptionPayload {
            algorithm: "xchacha20_poly1305",
            key: key_ref.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "--encryption-key-file or COPYPASTE_ENCRYPTION_KEY must be supplied when using --encryption-mode xchacha20_poly1305",
                )
            })?,
        }),
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
        retention_minutes,
        encryption: encryption.clone(),
        burn_after_reading: if burn_after_reading { Some(true) } else { None },
    };

    let base_url = validated_base_url(&host)?;
    let auth_token = load_secret(
        auth_token_file.as_deref(),
        "COPYPASTE_AUTH_TOKEN",
        "Auth token",
        4096,
    )?;
    let client = reqwest::blocking::Client::builder()
        // Paste bodies and write credentials must never be replayed to a
        // redirect target. Operators must configure the final HTTPS origin.
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(io::Error::other)?;

    let mut request = client.post(base_url.clone()).json(&payload);
    if let Some(token) = auth_token.as_deref() {
        request = request.header("X-CopyPaste-Write-Token", token);
    }
    let response = request.send().map_err(io::Error::other)?;

    if !response.status().is_success() {
        return Err(io::Error::other(format!(
            "Request failed with status: {}",
            response.status()
        )));
    }

    const MAX_RESPONSE_BYTES: u64 = 4096;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES)
    {
        return Err(io::Error::other("Server response is too large."));
    }
    let mut path = String::new();
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_string(&mut path)
        .map_err(io::Error::other)?;
    if path.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(io::Error::other("Server response is too large."));
    }
    let path = path.trim();

    if path.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Server returned an empty response.",
        ));
    }

    let returned = base_url
        .join(path)
        .map_err(|_| io::Error::other("Server returned an invalid paste URL."))?;
    if !matches!(returned.scheme(), "http" | "https")
        || !returned.username().is_empty()
        || returned.password().is_some()
        || returned.origin() != base_url.origin()
    {
        return Err(io::Error::other(
            "Server returned a paste URL on an unexpected origin.",
        ));
    }
    let full_url = returned.to_string();

    Ok(full_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use serde_json::json;
    use std::io::Write;

    fn write_owner_only_secret_file(token: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("copypaste-token-{}", nanoid::nanoid!()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = options.open(&path).expect("create token file");
        file.write_all(token.as_bytes()).expect("write token file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .expect("restrict token file");
        }
        path
    }

    #[test]
    fn send_submits_plain_text_and_returns_url() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/").json_body_includes(
                json!({ "content": "hello", "format": "plain_text" }).to_string(),
            );
            then.status(200).body("/paste/abc123");
        });

        let base = server.base_url();
        let args = SendArgs::parse_from(["copypaste-send", "hello", "--host", base.as_str()]);
        let url = execute_send(args).expect("url");
        assert_eq!(url, format!("{base}/paste/abc123"));
        mock.assert();
    }

    #[test]
    fn send_reads_auth_token_from_owner_only_file() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("x-copypaste-write-token", "operator-issued-token");
            then.status(200).body("/paste/authorized");
        });
        let token_file = write_owner_only_secret_file("operator-issued-token\n");

        let base = server.base_url();
        let args = SendArgs::parse_from([
            "copypaste-send",
            "hello",
            "--host",
            base.as_str(),
            "--auth-token-file",
            token_file.to_str().expect("utf-8 token path"),
        ]);
        let url = execute_send(args).expect("authorized send");
        std::fs::remove_file(token_file).expect("remove token file");

        assert_eq!(url, format!("{base}/paste/authorized"));
        assert!(!url.contains("operator-issued-token"));
        mock.assert();
    }

    #[test]
    fn send_does_not_follow_redirects_with_paste_or_token() {
        let server = MockServer::start();
        let base = server.base_url();
        let redirected_url = format!("{base}/capture");
        let redirect = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("x-copypaste-write-token", "no-redirect-token");
            then.status(307).header("Location", redirected_url.as_str());
        });
        let capture = server.mock(|when, then| {
            when.method(POST).path("/capture");
            then.status(200).body("/paste/leaked");
        });
        let token_file = write_owner_only_secret_file("no-redirect-token");

        let args = SendArgs::parse_from([
            "copypaste-send",
            "sensitive body",
            "--host",
            base.as_str(),
            "--auth-token-file",
            token_file.to_str().expect("utf-8 token path"),
        ]);
        let error = execute_send(args).expect_err("redirect must not be followed");
        std::fs::remove_file(token_file).expect("remove token file");

        assert!(error.to_string().contains("307"));
        redirect.assert();
        assert_eq!(capture.calls(), 0);
    }

    #[test]
    fn send_keeps_encryption_key_out_of_url() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/").json_body_includes(
                json!({ "encryption": { "algorithm": "aes256_gcm" } }).to_string(),
            );
            then.status(200).body("/secret");
        });

        let base = server.base_url();
        let key_file = write_owner_only_secret_file("super key");
        let args = SendArgs::parse_from([
            "copypaste-send",
            "payload",
            "--host",
            base.as_str(),
            "--encryption-mode",
            "aes256_gcm",
            "--encryption-key-file",
            key_file.to_str().expect("utf-8 key path"),
        ]);
        let url = execute_send(args).expect("url");
        std::fs::remove_file(key_file).expect("remove key file");
        assert_eq!(url, format!("{base}/secret"));
        assert!(!url.contains("super"));
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
            .contains("--encryption-key-file or COPYPASTE_ENCRYPTION_KEY"));
    }

    #[test]
    fn send_rejects_remote_plain_http() {
        let args =
            SendArgs::parse_from(["copypaste-send", "payload", "--host", "http://example.com"]);
        let error = execute_send(args).expect_err("remote HTTP must fail before sending");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("must use HTTPS"));
    }

    #[test]
    fn send_rejects_network_path_response_on_another_origin() {
        let server = MockServer::start();
        let response = server.mock(|when, then| {
            when.method(POST).path("/");
            then.status(200).body("//evil.example/paste");
        });
        let base = server.base_url();
        let args = SendArgs::parse_from(["copypaste-send", "payload", "--host", base.as_str()]);

        let error = execute_send(args).expect_err("cross-origin result must fail");
        assert!(error.to_string().contains("unexpected origin"));
        response.assert();
    }

    #[cfg(unix)]
    #[test]
    fn secret_file_symlinks_are_rejected() {
        use std::os::unix::fs::symlink;

        let target = write_owner_only_secret_file("secret-token");
        let link = std::env::temp_dir().join(format!("copypaste-token-link-{}", nanoid::nanoid!()));
        symlink(&target, &link).expect("create token symlink");

        let error =
            load_secret(Some(&link), "UNUSED", "Auth token", 4096).expect_err("symlink must fail");
        std::fs::remove_file(link).expect("remove link");
        std::fs::remove_file(target).expect("remove target");

        assert!(matches!(
            error.raw_os_error(),
            Some(code) if code == libc::ELOOP
        ));
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
        assert!(parse_ttl("18446744073709551615w").is_err());
    }

    #[test]
    fn send_ttl_flag_sends_retention_minutes() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .json_body_includes(json!({ "retention_minutes": 120 }).to_string());
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
        assert_eq!(url, format!("{base}/paste/timed"));
        mock.assert();
    }

    #[test]
    fn send_burn_alias_sends_burn_after_reading() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .json_body_includes(json!({ "burn_after_reading": true }).to_string());
            then.status(200).body("/paste/burned");
        });

        let base = server.base_url();
        let args =
            SendArgs::parse_from(["copypaste-send", "hello", "--host", base.as_str(), "--burn"]);
        let url = execute_send(args).expect("url");
        assert_eq!(url, format!("{base}/paste/burned"));
        mock.assert();
    }
}
