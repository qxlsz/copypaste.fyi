//! Optional create-challenge. Off on the public site.
//!
//! Self-hosters set `COPYPASTE_REQUIRE_CHALLENGE=true`. Browsers fetch
//! `GET /api/challenge` and send `X-CopyPaste-Challenge` on create. Tokens
//! are single-use and live two minutes. This stops drive-by POST scripts
//! that never fetch a ticket. It is not a CAPTCHA.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{rngs::OsRng, RngCore};
use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
    Request, State,
};

use super::time::current_timestamp;

pub type SharedChallengeStore = Arc<ChallengeStore>;

const TTL_SECS: i64 = 120;
const MAX_OUTSTANDING: usize = 10_000;

pub struct ChallengeStore {
    required: bool,
    tickets: Mutex<HashMap<String, i64>>,
}

impl ChallengeStore {
    pub fn from_env() -> Self {
        let required = std::env::var("COPYPASTE_REQUIRE_CHALLENGE")
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false);
        Self {
            required,
            tickets: Mutex::new(HashMap::new()),
        }
    }

    pub fn required(&self) -> bool {
        self.required
    }

    pub fn issue(&self) -> String {
        let mut bytes = [0u8; 24];
        OsRng.fill_bytes(&mut bytes);
        let token = URL_SAFE_NO_PAD.encode(bytes);
        let expires = current_timestamp() + TTL_SECS;
        if let Ok(mut map) = self.tickets.lock() {
            if map.len() >= MAX_OUTSTANDING {
                let now = current_timestamp();
                map.retain(|_, expiry| *expiry > now);
            }
            map.insert(token.clone(), expires);
        }
        token
    }

    pub fn consume(&self, token: &str) -> bool {
        if token.is_empty() || token.len() > 64 {
            return false;
        }
        let Ok(mut map) = self.tickets.lock() else {
            return false;
        };
        let Some(expires) = map.remove(token) else {
            return false;
        };
        expires > current_timestamp()
    }
}

pub struct RequireCreateChallenge;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RequireCreateChallenge {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let store = match req.guard::<&State<SharedChallengeStore>>().await {
            Outcome::Success(store) => store,
            _ => return Outcome::Success(RequireCreateChallenge),
        };
        if !store.required() {
            return Outcome::Success(RequireCreateChallenge);
        }
        let token = req.headers().get_one("X-CopyPaste-Challenge").unwrap_or("");
        if store.consume(token) {
            Outcome::Success(RequireCreateChallenge)
        } else {
            Outcome::Error((Status::Forbidden, ()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issued_ticket_works_once() {
        let store = ChallengeStore {
            required: true,
            tickets: Mutex::new(HashMap::new()),
        };
        let token = store.issue();
        assert!(store.consume(&token));
        assert!(!store.consume(&token));
    }

    #[test]
    fn empty_ticket_is_rejected() {
        let store = ChallengeStore {
            required: true,
            tickets: Mutex::new(HashMap::new()),
        };
        assert!(!store.consume(""));
    }
}
