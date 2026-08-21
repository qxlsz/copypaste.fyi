use std::env;

use rocket::request::{FromRequest, Outcome};
use rocket::Request;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;

/// Header injected by the trusted onion ingress after it has removed any
/// client-supplied value with the same name.
pub const ONION_INGRESS_HEADER: &str = "X-Copypaste-Onion-Ingress";

const MIN_INGRESS_TOKEN_BYTES: usize = 32;
const MAX_INGRESS_TOKEN_BYTES: usize = 512;

#[derive(Clone)]
pub struct TorConfig {
    pub onion_host: Option<String>,
    // Keep only a digest in managed application state so accidental Debug or
    // error output cannot disclose the ingress credential.
    ingress_token_digest: Option<[u8; 32]>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TorConfigError {
    #[error("COPYPASTE_ONION_HOST and COPYPASTE_ONION_INGRESS_TOKEN must be configured together")]
    IncompleteIngressPair,
    #[error("COPYPASTE_ONION_HOST must contain a valid .onion hostname")]
    InvalidOnionHost,
    #[error("COPYPASTE_ONION_INGRESS_TOKEN must contain 32 to 512 visible-ASCII bytes")]
    InvalidIngressToken,
    #[error("{0} must contain valid Unicode")]
    NonUnicode(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use rocket::{
        get,
        http::{uri::Host, Header},
        local::blocking::{Client, LocalRequest},
        routes,
    };
    use std::env;
    use std::ffi::OsString;
    use std::sync::Mutex;

    static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
    const TOR_ENV_VARS: [&str; 2] = ["COPYPASTE_ONION_HOST", "COPYPASTE_ONION_INGRESS_TOKEN"];

    struct EnvSnapshot(Vec<(&'static str, Option<OsString>)>);

    impl EnvSnapshot {
        fn capture() -> Self {
            Self(
                TOR_ENV_VARS
                    .iter()
                    .map(|name| (*name, env::var_os(name)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (name, value) in self.0.drain(..) {
                if let Some(value) = value {
                    env::set_var(name, value);
                } else {
                    env::remove_var(name);
                }
            }
        }
    }

    fn run_with_env(vars: &[(&str, &str)], f: impl FnOnce()) {
        let _snapshot = EnvSnapshot::capture();
        TOR_ENV_VARS.iter().for_each(|name| env::remove_var(name));
        for (key, value) in vars {
            env::set_var(key, value);
        }
        f();
    }

    #[test]
    fn tor_config_defaults_when_env_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        run_with_env(&[], || {
            let cfg = TorConfig::from_env();
            assert!(cfg.onion_host.is_none());
            assert!(cfg.ingress_token_digest.is_none());
        });
    }

    #[test]
    fn empty_onion_environment_pair_is_unconfigured() {
        let _guard = ENV_LOCK.lock().unwrap();
        run_with_env(
            &[
                ("COPYPASTE_ONION_HOST", ""),
                ("COPYPASTE_ONION_INGRESS_TOKEN", "   "),
            ],
            || {
                let cfg = TorConfig::try_from_env().expect("empty pair is disabled");
                assert!(cfg.onion_host.is_none());
                assert!(cfg.ingress_token_digest.is_none());
            },
        );
    }

    #[test]
    fn tor_config_respects_env_vars() {
        let _guard = ENV_LOCK.lock().unwrap();
        let ingress_token = "t".repeat(MIN_INGRESS_TOKEN_BYTES);
        run_with_env(
            &[
                ("COPYPASTE_ONION_HOST", "Example.Onion  "),
                ("COPYPASTE_ONION_INGRESS_TOKEN", ingress_token.as_str()),
            ],
            || {
                let cfg = TorConfig::from_env();
                assert_eq!(cfg.onion_host.as_deref(), Some("example.onion"));
                assert!(cfg.ingress_token_digest.is_some());
                assert!(cfg.is_trusted_onion_ingress("example.onion", Some(&ingress_token)));
            },
        );
    }

    #[test]
    fn short_or_non_header_safe_ingress_tokens_fail_closed() {
        let _guard = ENV_LOCK.lock().unwrap();
        for invalid in ["short", "this-token-is-long-enough-but-has a-space"] {
            run_with_env(
                &[
                    ("COPYPASTE_ONION_HOST", "example.onion"),
                    ("COPYPASTE_ONION_INGRESS_TOKEN", invalid),
                ],
                || {
                    assert_eq!(
                        TorConfig::try_from_env().err(),
                        Some(TorConfigError::InvalidIngressToken)
                    );
                },
            );
        }
    }

    #[test]
    fn onion_ingress_configuration_requires_an_exact_pair() {
        let _guard = ENV_LOCK.lock().unwrap();
        let token = "t".repeat(MIN_INGRESS_TOKEN_BYTES);

        run_with_env(&[("COPYPASTE_ONION_HOST", "example.onion")], || {
            assert_eq!(
                TorConfig::try_from_env().err(),
                Some(TorConfigError::IncompleteIngressPair)
            );
        });
        run_with_env(&[("COPYPASTE_ONION_INGRESS_TOKEN", token.as_str())], || {
            assert_eq!(
                TorConfig::try_from_env().err(),
                Some(TorConfigError::IncompleteIngressPair)
            );
        });
        run_with_env(
            &[
                ("COPYPASTE_ONION_HOST", "not-an-onion-host.example"),
                ("COPYPASTE_ONION_INGRESS_TOKEN", token.as_str()),
            ],
            || {
                assert_eq!(
                    TorConfig::try_from_env().err(),
                    Some(TorConfigError::InvalidOnionHost)
                );
            },
        );
        run_with_env(
            &[
                ("COPYPASTE_ONION_HOST", "example.onion"),
                ("COPYPASTE_ONION_INGRESS_TOKEN", token.as_str()),
            ],
            || {
                let config = TorConfig::try_from_env().expect("valid ingress pair");
                assert!(config.is_trusted_onion_ingress("example.onion", Some(&token)));
            },
        );
    }

    #[test]
    fn incomplete_onion_ingress_configuration_stops_startup() {
        let _guard = ENV_LOCK.lock().unwrap();
        run_with_env(&[("COPYPASTE_ONION_HOST", "example.onion")], || {
            let failure = std::panic::catch_unwind(TorConfig::from_env);
            let payload = match failure {
                Ok(_) => panic!("incomplete Tor ingress configuration must stop startup"),
                Err(payload) => payload,
            };
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied());
            assert_eq!(
                message,
                Some(
                    "invalid Tor configuration: COPYPASTE_ONION_HOST and \
                     COPYPASTE_ONION_INGRESS_TOKEN must be configured together"
                )
            );
        });
    }

    #[test]
    fn only_the_exact_configured_onion_hostname_is_trusted() {
        let cfg = TorConfig {
            onion_host: Some("example.onion".into()),
            ingress_token_digest: None,
        };

        assert!(cfg.is_onion_host("example.onion"));
        assert!(cfg.is_onion_host("EXAMPLE.ONION."));
        assert!(!cfg.is_onion_host("sub.example.onion"));
        assert!(!cfg.is_onion_host("example.com"));

        let unconfigured = TorConfig {
            onion_host: None,
            ingress_token_digest: None,
        };
        assert!(!unconfigured.is_onion_host("any.onion"));
        assert!(!unconfigured.is_onion_host("not-onion"));
    }

    #[get("/status")]
    fn status(access: OnionAccess) -> String {
        format!("{}|{}", access.is_onion(), access.host().unwrap_or(""))
    }

    fn build_client(config: TorConfig) -> Client {
        let rocket = rocket::build().manage(config).mount("/", routes![status]);
        Client::tracked(rocket).expect("client")
    }

    fn test_config(onion_host: Option<&str>, token: Option<&str>) -> TorConfig {
        TorConfig {
            onion_host: onion_host.map(str::to_string),
            ingress_token_digest: token.and_then(ingress_token_digest),
        }
    }

    fn valid_token() -> String {
        "v".repeat(MIN_INGRESS_TOKEN_BYTES)
    }

    fn request_with_host<'c>(client: &'c Client, host: &str) -> LocalRequest<'c> {
        let mut request = client.get("/status");
        request.inner_mut().set_host(
            Host::parse_owned(host.to_string()).expect("test Host header should be valid"),
        );
        request
    }

    #[test]
    fn exact_host_and_valid_ingress_token_set_onion_flags() {
        let token = valid_token();
        let client = build_client(test_config(Some("secure.onion"), Some(&token)));

        let response = request_with_host(&client, "secure.onion:443")
            .header(Header::new(ONION_INGRESS_HEADER, token))
            .dispatch();
        let body = response.into_string().expect("body");
        assert_eq!(body, "true|secure.onion");
    }

    #[test]
    fn matching_host_without_ingress_token_is_not_trusted() {
        let token = valid_token();
        let client = build_client(test_config(Some("secure.onion"), Some(&token)));

        let response = request_with_host(&client, "secure.onion").dispatch();
        let body = response.into_string().expect("body");
        assert_eq!(body, "false|secure.onion");
    }

    #[test]
    fn matching_host_with_wrong_ingress_token_is_not_trusted() {
        let token = valid_token();
        let client = build_client(test_config(Some("secure.onion"), Some(&token)));

        let response = request_with_host(&client, "secure.onion")
            .header(Header::new(
                ONION_INGRESS_HEADER,
                "w".repeat(MIN_INGRESS_TOKEN_BYTES),
            ))
            .dispatch();
        let body = response.into_string().expect("body");
        assert_eq!(body, "false|secure.onion");
    }

    #[test]
    fn forwarded_host_cannot_spoof_onion_access() {
        let token = valid_token();
        let client = build_client(test_config(Some("secure.onion"), Some(&token)));

        let response = request_with_host(&client, "copypaste.fyi")
            .header(Header::new("X-Forwarded-Host", "secure.onion"))
            .header(Header::new(ONION_INGRESS_HEADER, token))
            .dispatch();
        let body = response.into_string().expect("body");
        assert_eq!(body, "false|copypaste.fyi");
    }

    #[test]
    fn onion_host_is_not_inferred_without_configuration() {
        let token = valid_token();
        let client = build_client(test_config(None, Some(&token)));

        let response = request_with_host(&client, "arbitrary.onion")
            .header(Header::new(ONION_INGRESS_HEADER, token))
            .dispatch();
        let body = response.into_string().expect("body");
        assert_eq!(body, "false|arbitrary.onion");
    }

    #[test]
    fn plain_requests_leave_flags_unset() {
        let token = valid_token();
        let client = build_client(test_config(Some("secure.onion"), Some(&token)));

        let response = request_with_host(&client, "example.com")
            .header(Header::new(ONION_INGRESS_HEADER, token))
            .dispatch();
        let body = response.into_string().expect("body");
        assert_eq!(body, "false|example.com");
    }

    #[test]
    fn duplicate_ingress_headers_fail_closed() {
        let token = valid_token();
        let client = build_client(test_config(Some("secure.onion"), Some(&token)));

        let response = request_with_host(&client, "secure.onion")
            .header(Header::new(ONION_INGRESS_HEADER, token.clone()))
            .header(Header::new(ONION_INGRESS_HEADER, token))
            .dispatch();
        let body = response.into_string().expect("body");
        assert_eq!(body, "false|secure.onion");
    }
}

impl TorConfig {
    pub fn from_env() -> Self {
        Self::try_from_env().unwrap_or_else(|error| panic!("invalid Tor configuration: {error}"))
    }

    pub fn try_from_env() -> Result<Self, TorConfigError> {
        let onion_host = optional_unicode_env("COPYPASTE_ONION_HOST")?;
        let ingress_token = optional_unicode_env("COPYPASTE_ONION_INGRESS_TOKEN")?;

        match (onion_host, ingress_token) {
            (None, None) => Ok(Self {
                onion_host: None,
                ingress_token_digest: None,
            }),
            (Some(_), None) | (None, Some(_)) => Err(TorConfigError::IncompleteIngressPair),
            (Some(onion_host), Some(ingress_token)) => {
                let onion_host = normalize_onion_hostname(&onion_host)
                    .ok_or(TorConfigError::InvalidOnionHost)?;
                let ingress_token_digest = ingress_token_digest(&ingress_token)
                    .ok_or(TorConfigError::InvalidIngressToken)?;
                Ok(Self {
                    onion_host: Some(onion_host),
                    ingress_token_digest: Some(ingress_token_digest),
                })
            }
        }
    }

    pub fn is_onion_host(&self, host: &str) -> bool {
        let Some(normalized) = normalize_hostname(host) else {
            return false;
        };

        self.onion_host
            .as_deref()
            .and_then(normalize_onion_hostname)
            .is_some_and(|configured| normalized == configured)
    }

    fn is_trusted_onion_ingress(&self, host: &str, token: Option<&str>) -> bool {
        if !self.is_onion_host(host) {
            return false;
        }

        let (Some(expected), Some(candidate)) = (
            self.ingress_token_digest,
            token.and_then(ingress_token_digest),
        ) else {
            return false;
        };

        bool::from(expected.ct_eq(&candidate))
    }
}

fn optional_unicode_env(name: &'static str) -> Result<Option<String>, TorConfigError> {
    env::var_os(name)
        .map(|value| {
            value
                .into_string()
                .map_err(|_| TorConfigError::NonUnicode(name))
                .map(|value| (!value.trim().is_empty()).then_some(value))
        })
        .transpose()
        .map(Option::flatten)
}

fn ingress_token_digest(value: &str) -> Option<[u8; 32]> {
    let bytes = value.as_bytes();
    let is_header_safe = bytes.len() >= MIN_INGRESS_TOKEN_BYTES
        && bytes.len() <= MAX_INGRESS_TOKEN_BYTES
        && bytes.iter().all(|byte| byte.is_ascii_graphic());
    is_header_safe.then(|| Sha256::digest(bytes).into())
}

fn normalize_hostname(value: &str) -> Option<String> {
    let normalized = value.trim().trim_end_matches('.').to_ascii_lowercase();
    (!normalized.is_empty()).then_some(normalized)
}

fn normalize_onion_hostname(value: &str) -> Option<String> {
    let normalized = normalize_hostname(value)?;
    let is_valid = normalized.ends_with(".onion")
        && !normalized.contains(':')
        && normalized.split('.').all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        });

    is_valid.then_some(normalized)
}

#[derive(Debug, Clone)]
pub struct OnionAccess {
    is_onion: bool,
    host: Option<String>,
}

impl OnionAccess {
    pub fn is_onion(&self) -> bool {
        self.is_onion
    }

    pub fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }
}

fn header_host(request: &Request<'_>) -> Option<String> {
    request
        .host()
        .and_then(|host| normalize_hostname(host.domain().as_str()))
}

fn ingress_token_header<'r>(request: &'r Request<'_>) -> Option<&'r str> {
    let mut values = request.headers().get(ONION_INGRESS_HEADER);
    let value = values.next()?;
    values.next().is_none().then_some(value)
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
                ingress_token_digest: None,
            });

        let is_onion = host
            .as_deref()
            .map(|value| config.is_trusted_onion_ingress(value, ingress_token_header(request)))
            .unwrap_or(false);

        Outcome::Success(OnionAccess { is_onion, host })
    }
}
