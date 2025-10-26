use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose, Engine};
use chrono::DateTime;
use copypaste::{
    create_paste_store, EncryptionAlgorithm, PasteError, PasteFormat, SharedPasteStore,
    StoredContent, StoredPaste,
};
use html_escape::encode_safe;
use pulldown_cmark::{html, Options, Parser};
use rand::{rngs::OsRng, RngCore};
use rocket::fs::{FileServer, NamedFile};
use rocket::http::Status;
use rocket::response::content;
use rocket::serde::json::Json;
use rocket::serde::Deserialize;
use rocket::{get, post, routes, Build, Rocket, State};
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct EncryptionRequest {
    algorithm: EncryptionAlgorithm,
    key: String,
}

fn build_rocket(store: SharedPasteStore) -> Rocket<Build> {
    rocket::build()
        .manage(store)
        .mount("/", routes![index, create, show, static_files])
        .mount("/", FileServer::from("static"))
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste::MemoryPasteStore;
    use rocket::http::{ContentType, Status};
    use rocket::local::asynchronous::Client;
    use serde_json::json;
    use std::sync::Arc;

    async fn rocket_client() -> Client {
        Client::tracked(super::build_rocket(create_paste_store()))
            .await
            .expect("valid rocket instance")
    }

    async fn rocket_client_with_store(store: SharedPasteStore) -> Client {
        Client::tracked(super::build_rocket(store))
            .await
            .expect("valid rocket instance")
    }

    #[rocket::async_test]
    async fn post_plain_text_returns_id() {
        let client = rocket_client().await;
        let payload = json!({
            "content": "plain content",
            "format": "plain_text",
            "retention_minutes": 60
        });

        let response = client
            .post("/")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch()
            .await;

        assert_eq!(response.status(), Status::Ok);
        let body = response.into_string().await.expect("response body");
        assert!(body.starts_with('/'));

        let get_response = client.get(&body).dispatch().await;
        assert_eq!(get_response.status(), Status::Ok);
        let html = get_response.into_string().await.expect("html body");
        assert!(html.contains("plain content"));
    }

    #[rocket::async_test]
    async fn post_encrypted_requires_key() {
        let client = rocket_client().await;
        let payload = json!({
            "content": "secret text",
            "format": "markdown",
            "retention_minutes": 0,
            "encryption": {
                "algorithm": "aes256_gcm",
                "key": "passphrase"
            }
        });

        let response = client
            .post("/")
            .header(ContentType::JSON)
            .body(payload.to_string())
            .dispatch()
            .await;

        assert_eq!(response.status(), Status::Ok);
        let path = response.into_string().await.expect("body");

        let without_key = client.get(&path).dispatch().await;
        let without_body = without_key.into_string().await.expect("html");
        assert!(without_body.contains("Provide the encryption key"));

        let with_key = client
            .get(format!("{}?key=passphrase", path))
            .dispatch()
            .await;
        let html = with_key.into_string().await.expect("html");
        assert!(html.contains("secret text"));
    }

    #[rocket::async_test]
    async fn expired_paste_shows_expired_message() {
        let store: SharedPasteStore = Arc::new(MemoryPasteStore::default());
        let paste = StoredPaste {
            content: StoredContent::Plain {
                text: "short lived".into(),
            },
            format: PasteFormat::PlainText,
            created_at: 0,
            expires_at: Some(-1),
        };

        let id = store.create_paste(paste).await;
        let client = rocket_client_with_store(store).await;

        let expired = client.get(format!("/{}", id)).dispatch().await;
        let html = expired.into_string().await.expect("html");
        assert!(html.contains("Paste expired"));
    }

    #[test]
    fn encrypt_then_decrypt_roundtrip() {
        let key = "correct horse battery staple";
        let stored = encrypt_content("super secret", key).expect("encryption succeeds");
        let decrypted =
            decrypt_content(&stored, Some(key)).expect("decrypting with same key succeeds");
        assert_eq!(decrypted, "super secret");
    }

    #[test]
    fn decrypt_requires_key_for_encrypted_content() {
        let stored = encrypt_content("classified", "moonbase").expect("encryption succeeds");
        match decrypt_content(&stored, None) {
            Err(DecryptError::MissingKey) => {}
            other => panic!("expected missing key error, got {:?}", other),
        }
    }

    #[test]
    fn format_json_pretty_prints() {
        let result = format_json(r#"{"foo":1,"bar":[true,false]}"#);
        assert!(
            result.contains("\n"),
            "formatted JSON should contain newlines"
        );
        assert!(result.contains("foo"));
        assert!(result.starts_with("<pre><code>"));
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct CreatePasteRequest {
    content: String,
    #[serde(default)]
    format: Option<PasteFormat>,
    retention_minutes: Option<u64>,
    encryption: Option<EncryptionRequest>,
}

#[derive(Debug)]
enum DecryptError {
    MissingKey,
    InvalidKey,
}

#[get("/")]
async fn index() -> content::RawHtml<&'static str> {
    content::RawHtml(include_str!("../static/index.html"))
}

#[post("/", data = "<body>")]
async fn create(
    store: &State<SharedPasteStore>,
    body: Json<CreatePasteRequest>,
) -> Result<String, (Status, String)> {
    let now = current_timestamp();
    let format = body.format.clone().unwrap_or_default();
    let expires_at = body.retention_minutes.and_then(|mins| match mins {
        0 => None,
        minutes => Some(now + i64::try_from(minutes).unwrap_or(0) * 60),
    });

    let content = if let Some(enc) = &body.encryption {
        match enc.algorithm {
            EncryptionAlgorithm::None => StoredContent::Plain {
                text: body.content.clone(),
            },
            EncryptionAlgorithm::Aes256Gcm => {
                encrypt_content(&body.content, &enc.key).map_err(|e| (Status::BadRequest, e))?
            }
        }
    } else {
        StoredContent::Plain {
            text: body.content.clone(),
        }
    };

    let paste = StoredPaste {
        content,
        format,
        created_at: now,
        expires_at,
    };

    let id = store.create_paste(paste).await;
    Ok(format!("/{}", id))
}

#[get("/<id>?<key>")]
async fn show(
    store: &State<SharedPasteStore>,
    id: String,
    key: Option<String>,
) -> Result<content::RawHtml<String>, Status> {
    match store.get_paste(&id).await {
        Ok(paste) => Ok(content::RawHtml(render_paste(&id, &paste, key.as_deref()))),
        Err(PasteError::NotFound(_)) => Err(Status::NotFound),
        Err(PasteError::Expired(_)) => Ok(content::RawHtml(render_expired(&id))),
    }
}

#[get("/static/<path..>")]
async fn static_files(path: PathBuf) -> Option<NamedFile> {
    NamedFile::open(PathBuf::from("static").join(path))
        .await
        .ok()
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

fn encrypt_content(text: &str, key: &str) -> Result<StoredContent, String> {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);

    let mut hasher = Sha256::new();
    hasher.update(&salt[..]);
    hasher.update(key.as_bytes());
    let derived = hasher.finalize();
    let derived: [u8; 32] = derived.into();

    let cipher = Aes256Gcm::new_from_slice(&derived)
        .map_err(|_| "failed to initialise cipher".to_string())?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from(nonce_bytes);

    let ciphertext = cipher
        .encrypt(&nonce, text.as_bytes())
        .map_err(|_| "failed to encrypt content".to_string())?;

    Ok(StoredContent::Encrypted {
        algorithm: EncryptionAlgorithm::Aes256Gcm,
        ciphertext: general_purpose::STANDARD.encode(ciphertext),
        nonce: general_purpose::STANDARD.encode(nonce_bytes),
        salt: general_purpose::STANDARD.encode(salt),
    })
}

fn decrypt_content(content: &StoredContent, key: Option<&str>) -> Result<String, DecryptError> {
    match content {
        StoredContent::Plain { text } => Ok(text.clone()),
        StoredContent::Encrypted {
            algorithm,
            ciphertext,
            nonce,
            salt,
        } => {
            if *algorithm == EncryptionAlgorithm::None {
                return Ok(ciphertext.clone());
            }

            let key = key.ok_or(DecryptError::MissingKey)?;
            let salt_bytes = general_purpose::STANDARD
                .decode(salt)
                .map_err(|_| DecryptError::InvalidKey)?;
            let nonce_bytes_vec = general_purpose::STANDARD
                .decode(nonce)
                .map_err(|_| DecryptError::InvalidKey)?;
            let cipher_bytes = general_purpose::STANDARD
                .decode(ciphertext)
                .map_err(|_| DecryptError::InvalidKey)?;

            let mut hasher = Sha256::new();
            hasher.update(&salt_bytes);
            hasher.update(key.as_bytes());
            let derived = hasher.finalize();
            let derived: [u8; 32] = derived.into();

            let cipher =
                Aes256Gcm::new_from_slice(&derived).map_err(|_| DecryptError::InvalidKey)?;
            let nonce_array: [u8; 12] = nonce_bytes_vec
                .try_into()
                .map_err(|_| DecryptError::InvalidKey)?;
            let nonce = Nonce::from(nonce_array);

            cipher
                .decrypt(&nonce, cipher_bytes.as_ref())
                .map_err(|_| DecryptError::InvalidKey)
                .and_then(|bytes| String::from_utf8(bytes).map_err(|_| DecryptError::InvalidKey))
        }
    }
}

fn render_paste(id: &str, paste: &StoredPaste, key: Option<&str>) -> String {
    match decrypt_content(&paste.content, key) {
        Ok(text) => render_paste_view(id, paste, &text),
        Err(DecryptError::MissingKey) => render_key_prompt(id),
        Err(DecryptError::InvalidKey) => render_invalid_key(id),
    }
}

fn render_paste_view(id: &str, paste: &StoredPaste, text: &str) -> String {
    let rendered_body = match paste.format {
        PasteFormat::PlainText => format_plain(text),
        PasteFormat::Markdown => format_markdown(text),
        PasteFormat::Code => format_code(text),
        PasteFormat::Json => format_json(text),
    };

    let created = DateTime::from_timestamp(paste.created_at, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let expires = paste.expires_at.and_then(|ts| {
        DateTime::from_timestamp(ts, 0).map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
    });

    let encryption = match paste.content {
        StoredContent::Plain { .. } => "None".to_string(),
        StoredContent::Encrypted { ref algorithm, .. } => match algorithm {
            EncryptionAlgorithm::None => "None".to_string(),
            EncryptionAlgorithm::Aes256Gcm => "AES-256-GCM".to_string(),
        },
    };

    layout(
        "copypaste.fyi | View paste",
        format!(
            r#"<section class="meta">
    <div><strong>ID:</strong> {id}</div>
    <div><strong>Format:</strong> {format:?}</div>
    <div><strong>Created:</strong> {created}</div>
    <div><strong>Retention:</strong> {retention}</div>
    <div><strong>Encryption:</strong> {encryption}</div>
</section>
<article class="content">
    {rendered_body}
</article>
"#,
            id = encode_safe(id),
            format = paste.format,
            created = created,
            retention = expires.unwrap_or_else(|| "No expiry".to_string()),
            encryption = encryption,
            rendered_body = rendered_body,
        ),
    )
}

fn render_key_prompt(id: &str) -> String {
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

fn render_invalid_key(id: &str) -> String {
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

fn render_expired(id: &str) -> String {
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

fn format_plain(text: &str) -> String {
    format!("<pre>{}</pre>", encode_safe(text))
}

fn format_markdown(text: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(text, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

fn format_code(text: &str) -> String {
    format!("<pre><code>{}</code></pre>", encode_safe(text))
}

fn format_json(text: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) => {
            format_code(&serde_json::to_string_pretty(&value).unwrap_or_else(|_| text.to_string()))
        }
        Err(_) => format_code(text),
    }
}

fn layout(title: &str, body: String) -> String {
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
