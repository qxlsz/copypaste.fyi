use std::env;

use rocket::request::{FromRequest, Outcome};
use rocket::Request;

#[derive(Debug, Clone)]
pub struct TorConfig {
    pub onion_host: Option<String>,
    pub suppress_logs: bool,
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
