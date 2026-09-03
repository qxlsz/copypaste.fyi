use copypaste::server::handlers::build_rocket;
use copypaste::server::models::ApiError;
use copypaste::{
    MemoryPasteStore, PasteFormat, PasteMetadata, SharedPasteStore, StoredContent, StoredPaste,
};
use rocket::http::{ContentType, Header, Status};
use rocket::local::asynchronous::Client;
use serde_json::json;
use std::sync::Arc;

async fn rocket_client() -> Client {
    Client::tracked(build_rocket(Arc::new(MemoryPasteStore::new())))
        .await
        .expect("valid rocket instance")
}

async fn rocket_client_with_store(store: SharedPasteStore) -> Client {
    Client::tracked(build_rocket(store))
        .await
        .expect("valid rocket instance")
}

fn expired_plain_paste(text: &str) -> StoredPaste {
    StoredPaste {
        content: StoredContent::Plain {
            text: text.to_string(),
        },
        format: PasteFormat::PlainText,
        created_at: 1,
        expires_at: Some(1),
        burn_after_reading: false,
        bundle: None,
        bundle_parent: None,
        bundle_label: None,
        not_before: None,
        not_after: None,
        persistence: None,
        webhook: None,
        metadata: PasteMetadata::default(),
        is_live: false,
        owner_token_hash: None,
    }
}

#[rocket::async_test]
async fn create_api_rejects_empty_and_whitespace_content() {
    let client = rocket_client().await;
    for content in ["", "   ", "\n\t"] {
        let response = client
            .post("/api/pastes")
            .header(ContentType::JSON)
            .body(json!({ "content": content, "format": "plain_text" }).to_string())
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::BadRequest, "content={content:?}");
        let err: ApiError =
            serde_json::from_str(&response.into_string().await.expect("error body")).expect("json");
        assert_eq!(err.code, "bad_request");
        assert!(
            err.message.to_lowercase().contains("empty"),
            "{}",
            err.message
        );
    }
}

#[rocket::async_test]
async fn legacy_create_rejects_empty_content_as_plain_text() {
    let client = rocket_client().await;
    let response = client
        .post("/")
        .header(ContentType::JSON)
        .body(json!({ "content": " ", "format": "plain_text" }).to_string())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::BadRequest);
    let body = response.into_string().await.expect("legacy error");
    assert!(body.contains("empty"), "{body}");
    assert!(
        serde_json::from_str::<ApiError>(&body).is_err(),
        "legacy create must not use the JSON API envelope"
    );
}

#[rocket::async_test]
async fn create_api_malformed_json_uses_api_error_envelope() {
    let client = rocket_client().await;
    let response = client
        .post("/api/pastes")
        .header(ContentType::JSON)
        .body("{not valid json}")
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::BadRequest);
    let err: ApiError =
        serde_json::from_str(&response.into_string().await.expect("error body")).expect("json");
    assert_eq!(err.code, "invalid_request");
}

#[rocket::async_test]
async fn show_api_time_lock_after_not_after_returns_423() {
    let client = rocket_client().await;
    let response = client
        .post("/api/pastes")
        .header(ContentType::JSON)
        .body(
            json!({
                "content": "window closed secret",
                "format": "plain_text",
                "time_lock": { "not_after": "2000-01-01T00:00:00Z" }
            })
            .to_string(),
        )
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let created: serde_json::Value =
        serde_json::from_str(&response.into_string().await.expect("create body")).expect("json");
    let id = created["id"].as_str().expect("id");

    let locked = client.get(format!("/api/pastes/{id}")).dispatch().await;
    assert_eq!(locked.status(), Status::Locked);
    let body = locked.into_string().await.expect("locked body");
    let err: ApiError = serde_json::from_str(&body).expect("json");
    assert_eq!(err.code, "time_lock_elapsed");
    assert!(!body.contains("window closed secret"));
}

#[rocket::async_test]
async fn expired_html_raw_and_share_match_missing_404() {
    let store: SharedPasteStore = Arc::new(MemoryPasteStore::new());
    let id = store
        .create_paste(expired_plain_paste("should never leak"))
        .await
        .expect("create expired paste");
    let client = rocket_client_with_store(store).await;

    for prefix in ["/api/pastes/", "/p/", "/raw/", "/"] {
        let expired = client.get(format!("{prefix}{id}")).dispatch().await;
        let missing = client
            .get(format!("{prefix}no-such-expired-paste"))
            .dispatch()
            .await;
        assert_eq!(expired.status(), Status::NotFound, "expired {prefix}");
        assert_eq!(missing.status(), Status::NotFound, "missing {prefix}");
        assert_eq!(
            expired.into_string().await.expect("expired body"),
            missing.into_string().await.expect("missing body"),
            "expired {prefix} must match missing"
        );
    }
}

#[rocket::async_test]
async fn html_share_route_accepts_x_paste_key() {
    let client = rocket_client().await;
    let response = client
        .post("/api/pastes")
        .header(ContentType::JSON)
        .body(
            json!({
                "content": "html header secret",
                "format": "plain_text",
                "encryption": { "algorithm": "aes256_gcm", "key": "headerpass" }
            })
            .to_string(),
        )
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let created: serde_json::Value =
        serde_json::from_str(&response.into_string().await.expect("create body")).expect("json");
    let path = created["shareableUrl"].as_str().expect("shareableUrl");
    assert!(path.starts_with("/p/"), "{path}");

    let ok = client
        .get(path)
        .header(Header::new("X-Paste-Key", "headerpass"))
        .dispatch()
        .await;
    assert_eq!(ok.status(), Status::Ok);
    let html = ok.into_string().await.expect("html");
    assert!(html.contains("html header secret"));
    assert!(!html.contains("headerpass"));
}

#[rocket::async_test]
async fn cors_allows_create_from_default_origin_and_denies_lookalike() {
    let client = rocket_client().await;
    let allowed = client
        .post("/api/pastes")
        .header(ContentType::JSON)
        .header(Header::new("Origin", "https://www.copypaste.fyi"))
        .body(json!({ "content": "cors create", "format": "plain_text" }).to_string())
        .dispatch()
        .await;
    assert_eq!(allowed.status(), Status::Ok);
    assert_eq!(
        allowed.headers().get_one("Access-Control-Allow-Origin"),
        Some("https://www.copypaste.fyi")
    );
    assert_eq!(
        allowed.headers().get_one("Access-Control-Expose-Headers"),
        Some("Content-Type,Retry-After")
    );
    assert!(allowed
        .headers()
        .get_one("Access-Control-Allow-Credentials")
        .is_none());

    let denied = client
        .post("/api/pastes")
        .header(ContentType::JSON)
        .header(Header::new("Origin", "https://www.copypaste.fyi.evil"))
        .body(json!({ "content": "cors denied", "format": "plain_text" }).to_string())
        .dispatch()
        .await;
    assert_eq!(denied.status(), Status::Ok);
    assert!(denied
        .headers()
        .get_one("Access-Control-Allow-Origin")
        .is_none());
}

#[rocket::async_test]
async fn cors_preflight_allows_put_and_paste_key_header() {
    let client = rocket_client().await;
    let response = client
        .options("/api/pastes/example-id")
        .header(Header::new("Origin", "https://copypaste.fyi"))
        .header(Header::new("Access-Control-Request-Method", "PUT"))
        .header(Header::new(
            "Access-Control-Request-Headers",
            "X-Paste-Key,Authorization",
        ))
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::NoContent);
    assert_eq!(
        response.headers().get_one("Access-Control-Allow-Origin"),
        Some("https://copypaste.fyi")
    );
    let methods = response
        .headers()
        .get_one("Access-Control-Allow-Methods")
        .unwrap_or_default();
    assert!(methods.contains("PUT"), "{methods}");
    assert!(methods.contains("PATCH"), "{methods}");
    let headers = response
        .headers()
        .get_one("Access-Control-Allow-Headers")
        .unwrap_or_default();
    assert!(headers.contains("X-Paste-Key"), "{headers}");
    assert!(headers.contains("X-CopyPaste-Write-Token"), "{headers}");
}

#[rocket::async_test]
async fn openapi_documents_json_time_lock_and_rate_limit_status() {
    let client = rocket_client().await;
    let response = client.get("/api/openapi.json").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let doc: serde_json::Value =
        serde_json::from_str(&response.into_string().await.expect("openapi")).expect("json");
    let read = &doc["paths"]["/api/pastes/{id}"]["get"]["responses"];
    assert!(read.get("423").is_some(), "JSON reads must document 423");
    assert!(read.get("429").is_some(), "JSON reads must document 429");
    let create = &doc["paths"]["/api/pastes"]["post"]["responses"];
    assert!(create.get("429").is_some(), "create must document 429");
}

#[rocket::async_test]
async fn well_known_and_skills_advertise_api_read_contract() {
    let client = rocket_client().await;
    let discovery = client.get("/.well-known/copypaste.json").dispatch().await;
    assert_eq!(discovery.status(), Status::Ok);
    let body: serde_json::Value =
        serde_json::from_str(&discovery.into_string().await.expect("body")).expect("json");
    assert_eq!(body["read"], "/api/pastes/{id}");
    assert_eq!(body["create"], "/api/pastes");
    assert!(body["encryption"]
        .as_array()
        .unwrap()
        .contains(&json!("aes256_gcm")));
    assert!(!body["encryption"]
        .as_array()
        .unwrap()
        .contains(&json!("kyber_hybrid")));

    let llms = client.get("/llms.txt").dispatch().await;
    assert_eq!(llms.status(), Status::Ok);
    assert!(llms
        .headers()
        .get_one("Content-Type")
        .unwrap_or_default()
        .starts_with("text/plain"));
}
