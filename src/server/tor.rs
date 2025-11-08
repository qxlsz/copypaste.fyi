use std::env;

use rocket::request::{FromRequest, Outcome};
use rocket::Request;

#[derive(Debug, Clone)]
pub struct TorConfig {
    pub onion_host: Option<String>,
    pub suppress_logs: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use rocket::{get, http::Header, local::blocking::Client, routes};
    use std::env;
    use std::sync::Mutex;

    static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn run_with_env(vars: &[(&str, &str)], f: impl FnOnce()) {
        for (key, _) in vars {
            env::remove_var(key);
        }
        for (key, value) in vars {
            env::set_var(key, value);
        }
        f();
        for (key, _) in vars {
            env::remove_var(key);
        }
    }

    #[test]
    fn tor_config_defaults_when_env_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var("COPYPASTE_ONION_HOST");
        env::remove_var("COPYPASTE_TOR_SUPPRESS_LOGS");

        let cfg = TorConfig::from_env();
        assert!(cfg.onion_host.is_none());
        assert!(cfg.suppress_logs);
    }

    #[test]
    fn tor_config_respects_env_vars() {
        let _guard = ENV_LOCK.lock().unwrap();
        run_with_env(
            &[
                ("COPYPASTE_ONION_HOST", "Example.Onion  "),
                ("COPYPASTE_TOR_SUPPRESS_LOGS", "false"),
            ],
            || {
                let cfg = TorConfig::from_env();
                assert_eq!(cfg.onion_host.as_deref(), Some("Example.Onion"));
                assert!(!cfg.suppress_logs);
            },
        );
    }

    #[test]
    fn is_onion_host_handles_configured_and_suffix_cases() {
        let cfg = TorConfig {
            onion_host: Some("example.onion".into()),
            suppress_logs: true,
        };

        assert!(cfg.is_onion_host("example.onion"));
        assert!(cfg.is_onion_host("sub.example.onion"));
        assert!(!cfg.is_onion_host("example.com"));

        let suffix_only = TorConfig {
            onion_host: None,
            suppress_logs: true,
        };
        assert!(suffix_only.is_onion_host("any.onion"));
        assert!(!suffix_only.is_onion_host("not-onion"));
    }

    #[derive(Clone, Copy)]
    struct Suppressed(bool);

    #[rocket::async_trait]
    impl<'r> FromRequest<'r> for Suppressed {
        type Error = ();

        async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
            Outcome::Success(Suppressed(logs_suppressed(request)))
        }
    }

    #[get("/status")]
    fn status(access: OnionAccess, suppressed: Suppressed) -> String {
        format!(
            "{}|{}|{}",
            access.is_onion(),
            access.host().unwrap_or(""),
            suppressed.0
        )
    }

    fn build_client(config: TorConfig) -> Client {
        let rocket = rocket::build().manage(config).mount("/", routes![status]);
        Client::tracked(rocket).expect("client")
    }

    #[test]
    fn onion_requests_set_flags_and_log_suppression() {
        let client = build_client(TorConfig {
            onion_host: Some("secure.onion".into()),
            suppress_logs: true,
        });

        let response = client
            .get("/status")
            .header(Header::new("X-Forwarded-Host", "secure.onion"))
            .dispatch();
        let body = response.into_string().expect("body");
        assert_eq!(body, "true|secure.onion|true");
    }

    #[test]
    fn plain_requests_leave_flags_unset() {
        let client = build_client(TorConfig {
            onion_host: Some("secure.onion".into()),
            suppress_logs: true,
        });

        let response = client
            .get("/status")
            .header(Header::new("Host", "example.com"))
            .dispatch();
        let body = response.into_string().expect("body");
        assert_eq!(body, "false|example.com|false");
    }

    #[test]
    fn suppress_logs_respected_when_disabled() {
        let client = build_client(TorConfig {
            onion_host: Some("secure.onion".into()),
            suppress_logs: false,
        });

        let response = client
            .get("/status")
            .header(Header::new("X-Forwarded-Host", "secure.onion"))
            .dispatch();
        let body = response.into_string().expect("body");
        assert_eq!(body, "true|secure.onion|false");
    }
}

impl TorConfig {
    pub fn from_env() -> Self {
        let onion_host = env::var("COPYPASTE_ONION_HOST")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let suppress_logs = env::var("COPYPASTE_TOR_SUPPRESS_LOGS")
            .map(|value| !matches!(value.trim(), "0" | "false" | "off"))
            .unwrap_or(true);

        Self {
            onion_host,
            suppress_logs,
        }
    }

    pub fn is_onion_host(&self, host: &str) -> bool {
        let normalized = host.trim().to_ascii_lowercase();
        if let Some(configured_host) = &self.onion_host {
            normalized == configured_host.to_ascii_lowercase() || normalized.ends_with(".onion")
        } else {
            normalized.ends_with(".onion")
        }
    }
}

#[derive(Debug, Clone)]
pub struct OnionAccess {
    is_onion: bool,
    host: Option<String>,
    suppress_logs: bool,
}

impl OnionAccess {
    pub fn is_onion(&self) -> bool {
        self.is_onion
    }

    pub fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }

    pub fn suppress_logs(&self) -> bool {
        self.suppress_logs
    }
}

#[derive(Copy, Clone)]
struct LogSuppressionFlag(bool);

fn header_host(request: &Request<'_>) -> Option<String> {
    request
        .headers()
        .get_one("x-forwarded-host")
        .or_else(|| request.headers().get_one("host"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for OnionAccess {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let host = header_host(request);
        let config = request
            .rocket()
            .state::<TorConfig>()
            .cloned()
            .unwrap_or(TorConfig {
                onion_host: None,
                suppress_logs: true,
            });

        let is_onion = host
            .as_deref()
            .map(|value| config.is_onion_host(value))
            .unwrap_or(false);

        let suppress_logs = is_onion && config.suppress_logs;
        if suppress_logs {
            request.local_cache(|| LogSuppressionFlag(true));
        }

        Outcome::Success(OnionAccess {
            is_onion,
            host,
            suppress_logs,
        })
    }
}

pub fn logs_suppressed(request: &Request<'_>) -> bool {
    request.local_cache(|| LogSuppressionFlag(false)).0
}
