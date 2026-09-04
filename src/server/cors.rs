use std::env;

use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::{Header, Method, Status};
use rocket::{options, Request, Response};
use url::Url;

const DEFAULT_ALLOWED_ORIGINS: &str = "https://copypaste.fyi,https://www.copypaste.fyi";
const ALLOWED_METHODS: &str = "GET,POST,PUT,PATCH,DELETE,OPTIONS";
const ALLOWED_HEADERS: &str =
    "Content-Type,Authorization,X-Requested-With,X-Paste-Key,X-CopyPaste-Write-Token";
const EXPOSED_HEADERS: &str = "Content-Type,Retry-After";
const MAX_AGE_SECONDS: &str = "86400";

// The legacy backend index in `static/index.html` contains inline CSS and
// JavaScript. Keep these two narrowly documented exceptions until that page is
// migrated to hashed external assets. All high-risk navigation and embedding
// directives remain closed.
const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; base-uri 'none'; object-src 'none'; \
    frame-src 'none'; frame-ancestors 'none'; form-action 'self'; connect-src 'self'; \
    img-src 'self' data:; font-src 'self'; \
    style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; \
    worker-src 'self' blob:; manifest-src 'self'; media-src 'self'";

const PERMISSIONS_POLICY: &str = "accelerometer=(), ambient-light-sensor=(), autoplay=(), \
    battery=(), camera=(), display-capture=(), geolocation=(), gyroscope=(), \
    magnetometer=(), microphone=(), midi=(), payment=(), publickey-credentials-get=(), \
    screen-wake-lock=(), usb=()";

#[derive(Clone, Copy)]
pub struct Cors;

fn normalize_origin(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value == "*" {
        return None;
    }

    let parsed = Url::parse(value).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return None;
    }

    Some(parsed.origin().ascii_serialization())
}

fn allowed_origins() -> Vec<String> {
    let configured = env::var("COPYPASTE_ALLOWED_ORIGINS").ok();
    let source = configured.as_deref().unwrap_or(DEFAULT_ALLOWED_ORIGINS);

    source.split(',').filter_map(normalize_origin).collect()
}

fn allowed_request_origin(request: &Request<'_>) -> Option<String> {
    let request_origin = request.headers().get_one("Origin")?;
    let normalized = normalize_origin(request_origin)?;
    allowed_origins()
        .iter()
        .any(|allowed| allowed == &normalized)
        .then_some(normalized)
}

fn is_generated_paste_id(segment: &str) -> bool {
    let slug_parts: Vec<_> = segment.split('-').collect();
    let is_readable_slug = slug_parts.len() == 3
        && !slug_parts[0].is_empty()
        && !slug_parts[1].is_empty()
        && slug_parts[0].bytes().all(|byte| byte.is_ascii_lowercase())
        && slug_parts[1].bytes().all(|byte| byte.is_ascii_lowercase())
        && slug_parts[2].len() == 2
        && slug_parts[2].bytes().all(|byte| byte.is_ascii_digit());

    let is_nanoid = matches!(segment.len(), 10 | 24)
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));

    is_readable_slug || is_nanoid
}

fn is_paste_route(path: &str) -> bool {
    path == "/api/pastes"
        || path.starts_with("/api/pastes/")
        || path.starts_with("/p/")
        || path.starts_with("/raw/")
        || path
            .strip_prefix('/')
            .filter(|segment| !segment.contains('/'))
            .is_some_and(is_generated_paste_id)
}

fn is_path_or_child(path: &str, prefix: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn is_private_route(path: &str) -> bool {
    is_paste_route(path)
        || [
            "/login",
            "/dashboard",
            "/admin",
            "/user",
            "/workspace",
            "/workspaces",
            "/api/auth",
            "/api/authenticate",
            "/api/admin",
            "/api/user",
            "/api/workspace",
            "/api/workspaces",
        ]
        .iter()
        .any(|prefix| is_path_or_child(path, prefix))
}

fn set_security_headers(response: &mut Response<'_>) {
    response.set_header(Header::new("Referrer-Policy", "no-referrer"));
    response.set_header(Header::new("X-Content-Type-Options", "nosniff"));
    response.set_header(Header::new("X-Frame-Options", "DENY"));
    response.set_header(Header::new(
        "Content-Security-Policy",
        CONTENT_SECURITY_POLICY,
    ));
    response.set_header(Header::new("Permissions-Policy", PERMISSIONS_POLICY));
    response.set_header(Header::new("Cross-Origin-Opener-Policy", "same-origin"));
    response.set_header(Header::new(
        "Strict-Transport-Security",
        "max-age=63072000; includeSubDomains; preload",
    ));
}

#[rocket::async_trait]
impl Fairing for Cors {
    fn info(&self) -> Info {
        Info {
            name: "Security and allowlisted CORS headers",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        set_security_headers(response);

        let path = request.uri().path().as_str();
        let is_paste_response = is_paste_route(path)
            || (path == "/" && matches!(request.method(), Method::Post | Method::Put));
        if is_paste_response {
            response.set_header(Header::new(
                "X-Robots-Tag",
                "noindex, nofollow, noarchive, nosnippet, noimageindex",
            ));
        }
        if is_paste_response || is_private_route(path) {
            response.set_header(Header::new("Cache-Control", "private, no-store"));
            response.set_header(Header::new("Pragma", "no-cache"));
        }

        if response.status() == Status::TooManyRequests
            && response.headers().get_one("Retry-After").is_none()
        {
            // Fixed-window limiter uses a 60-second bucket. Advertise that so
            // clients wait out the window instead of retrying immediately.
            response.set_header(Header::new("Retry-After", "60"));
        }

        // All responses vary by Origin, including denied requests, so an edge
        // cache can never replay an allowlisted response to a different origin.
        response.adjoin_header(Header::new("Vary", "Origin"));

        if let Some(origin) = allowed_request_origin(request) {
            response.set_header(Header::new("Access-Control-Allow-Origin", origin));
            response.set_header(Header::new(
                "Access-Control-Expose-Headers",
                EXPOSED_HEADERS,
            ));

            if request.method() == Method::Options {
                response.set_header(Header::new("Access-Control-Allow-Methods", ALLOWED_METHODS));
                response.set_header(Header::new("Access-Control-Allow-Headers", ALLOWED_HEADERS));
                response.set_header(Header::new("Access-Control-Max-Age", MAX_AGE_SECONDS));
            }
        }

        if request.method() == Method::Options {
            response.set_status(Status::NoContent);
            response.set_header(Header::new("Content-Length", "0"));
        }
    }
}

#[options("/api/<_..>")]
pub fn api_preflight() -> Status {
    Status::NoContent
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use rocket::local::blocking::Client;
    use rocket::{get, routes};
    use std::path::PathBuf;
    use std::sync::Mutex;

    static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    struct EnvRestore(Option<String>);

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            match self.0.take() {
                Some(value) => env::set_var("COPYPASTE_ALLOWED_ORIGINS", value),
                None => env::remove_var("COPYPASTE_ALLOWED_ORIGINS"),
            }
        }
    }

    fn with_allowed_origins<T>(value: Option<&str>, f: impl FnOnce() -> T) -> T {
        let _lock = ENV_LOCK.lock().unwrap();
        let _restore = EnvRestore(env::var("COPYPASTE_ALLOWED_ORIGINS").ok());
        match value {
            Some(value) => env::set_var("COPYPASTE_ALLOWED_ORIGINS", value),
            None => env::remove_var("COPYPASTE_ALLOWED_ORIGINS"),
        }
        f()
    }

    #[get("/<_path..>")]
    fn any_route(_path: PathBuf) -> &'static str {
        "ok"
    }

    fn client() -> Client {
        Client::tracked(
            rocket::build()
                .attach(Cors)
                .mount("/", routes![api_preflight, any_route]),
        )
        .expect("client")
    }

    #[test]
    fn default_origin_allowlist_is_exact_and_never_wildcard() {
        with_allowed_origins(None, || {
            let client = client();
            let response = client
                .get("/health")
                .header(Header::new("Origin", "https://www.copypaste.fyi"))
                .dispatch();

            assert_eq!(
                response.headers().get_one("Access-Control-Allow-Origin"),
                Some("https://www.copypaste.fyi")
            );
            assert_eq!(response.headers().get_one("Vary"), Some("Origin"));
            assert_eq!(
                response.headers().get_one("Access-Control-Expose-Headers"),
                Some(EXPOSED_HEADERS)
            );
            assert!(response
                .headers()
                .get_one("Access-Control-Allow-Credentials")
                .is_none());

            let denied = client
                .get("/health")
                .header(Header::new("Origin", "https://www.copypaste.fyi.evil"))
                .dispatch();
            assert!(denied
                .headers()
                .get_one("Access-Control-Allow-Origin")
                .is_none());
        });
    }

    #[test]
    fn configured_local_origin_is_allowed_and_invalid_entries_fail_closed() {
        with_allowed_origins(
            Some("*, http://127.0.0.1:5173, https://example.com/path"),
            || {
                let client = client();
                let allowed = client
                    .get("/health")
                    .header(Header::new("Origin", "http://127.0.0.1:5173"))
                    .dispatch();
                assert_eq!(
                    allowed.headers().get_one("Access-Control-Allow-Origin"),
                    Some("http://127.0.0.1:5173")
                );

                for denied_origin in ["https://copypaste.fyi", "https://example.com"] {
                    let denied = client
                        .get("/health")
                        .header(Header::new("Origin", denied_origin))
                        .dispatch();
                    assert!(denied
                        .headers()
                        .get_one("Access-Control-Allow-Origin")
                        .is_none());
                }
            },
        );
    }

    #[test]
    fn preflight_returns_no_content_only_for_the_exact_allowlisted_origin() {
        with_allowed_origins(Some("https://allowed.example"), || {
            let client = client();
            let response = client
                .options("/api/pastes")
                .header(Header::new("Origin", "https://allowed.example"))
                .header(Header::new("Access-Control-Request-Method", "POST"))
                .dispatch();

            assert_eq!(response.status(), Status::NoContent);
            assert_eq!(
                response.headers().get_one("Access-Control-Allow-Methods"),
                Some(ALLOWED_METHODS)
            );
            assert_eq!(
                response.headers().get_one("Access-Control-Allow-Headers"),
                Some(ALLOWED_HEADERS)
            );
            assert!(response
                .headers()
                .get_one("Access-Control-Allow-Headers")
                .is_some_and(|headers| headers.contains("X-CopyPaste-Write-Token")));

            let denied = client
                .options("/api/pastes")
                .header(Header::new("Origin", "https://denied.example"))
                .dispatch();
            assert_eq!(denied.status(), Status::NoContent);
            assert!(denied
                .headers()
                .get_one("Access-Control-Allow-Origin")
                .is_none());
            assert!(denied
                .headers()
                .get_one("Access-Control-Allow-Methods")
                .is_none());
        });
    }

    #[test]
    fn security_headers_and_private_cache_policy_cover_sensitive_routes() {
        with_allowed_origins(None, || {
            let client = client();
            let paste = client.get("/gentle-comet-42").dispatch();

            assert_eq!(
                paste.headers().get_one("Referrer-Policy"),
                Some("no-referrer")
            );
            assert_eq!(
                paste.headers().get_one("X-Content-Type-Options"),
                Some("nosniff")
            );
            assert_eq!(paste.headers().get_one("X-Frame-Options"), Some("DENY"));
            assert_eq!(
                paste.headers().get_one("Cross-Origin-Opener-Policy"),
                Some("same-origin")
            );
            assert_eq!(
                paste.headers().get_one("Strict-Transport-Security"),
                Some("max-age=63072000; includeSubDomains; preload")
            );
            let csp = paste
                .headers()
                .get_one("Content-Security-Policy")
                .expect("CSP");
            for directive in [
                "base-uri 'none'",
                "object-src 'none'",
                "frame-src 'none'",
                "frame-ancestors 'none'",
                "form-action 'self'",
                "connect-src 'self'",
            ] {
                assert!(csp.contains(directive));
            }
            assert!(!csp.contains("api.qrserver.com"));
            assert!(paste.headers().get_one("Permissions-Policy").is_some());
            assert_eq!(
                paste.headers().get_one("X-Robots-Tag"),
                Some("noindex, nofollow, noarchive, nosnippet, noimageindex")
            );
            assert_eq!(
                paste.headers().get_one("Cache-Control"),
                Some("private, no-store")
            );

            let user_api = client.get("/api/user/pastes").dispatch();
            assert_eq!(
                user_api.headers().get_one("Cache-Control"),
                Some("private, no-store")
            );
            assert!(user_api.headers().get_one("X-Robots-Tag").is_none());

            let legacy_create = client.post("/").dispatch();
            assert_eq!(
                legacy_create.headers().get_one("Cache-Control"),
                Some("private, no-store")
            );

            let public_health = client.get("/health").dispatch();
            assert!(public_health.headers().get_one("Cache-Control").is_none());
        });
    }

    #[test]
    fn current_and_legacy_random_ids_are_treated_as_sensitive() {
        assert!(is_generated_paste_id("abcdefghij"));
        assert!(is_generated_paste_id("abcdefghijklmnopqrstuvwx"));
        assert!(!is_generated_paste_id("abcdefghijklmnopqrstuvw"));
    }

    #[get("/limited")]
    fn limited_route() -> Status {
        Status::TooManyRequests
    }

    #[test]
    fn too_many_requests_advertise_retry_after() {
        let client = Client::tracked(
            rocket::build()
                .attach(Cors)
                .mount("/", routes![limited_route]),
        )
        .expect("client");
        let response = client.get("/limited").dispatch();
        assert_eq!(response.status(), Status::TooManyRequests);
        assert_eq!(response.headers().get_one("Retry-After"), Some("60"));
    }
}
