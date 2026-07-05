//! Server-side login sessions.
//!
//! `POST /api/auth/login` issues a random bearer token after verifying an
//! Ed25519 challenge signature. This module stores those tokens so that the
//! user-scoped endpoints (`/api/user/*`, `/api/workspaces/*`) can require a
//! valid `Authorization: Bearer <token>` header and only ever return data for
//! the session's own `pubkey_hash` — closing the unauthenticated paste
//! enumeration hole where any caller could list pastes for an arbitrary hash.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
    Request, State,
};

use super::time::current_timestamp;

/// Lifetime of a login session: 24 hours.
pub const SESSION_TTL_SECS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone)]
struct Session {
    pubkey_hash: String,
    expires_at: i64,
}

/// In-memory session store, kept on Rocket managed state (mirrors the
/// `SharedRateLimiter` pattern in `api_keys.rs`). Expired entries are purged
/// lazily on every insert.
pub struct SessionStore {
    sessions: RwLock<HashMap<String, Session>>,
}

pub type SharedSessionStore = Arc<SessionStore>;

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Register a session token for a pubkey hash (24 h expiry).
    pub fn insert(&self, token: &str, pubkey_hash: &str) {
        self.insert_with_expiry(token, pubkey_hash, current_timestamp() + SESSION_TTL_SECS);
    }

    /// Register a session token with an explicit expiry timestamp.
    /// Lazily purges any already-expired sessions.
    pub fn insert_with_expiry(&self, token: &str, pubkey_hash: &str, expires_at: i64) {
        let now = current_timestamp();
        let mut map = self.sessions.write().unwrap();
        map.retain(|_, session| session.expires_at > now);
        map.insert(
            token.to_owned(),
            Session {
                pubkey_hash: pubkey_hash.to_owned(),
                expires_at,
            },
        );
    }

    /// Return the `pubkey_hash` for a valid (present and unexpired) token.
    pub fn validate(&self, token: &str) -> Option<String> {
        let now = current_timestamp();
        let map = self.sessions.read().unwrap();
        map.get(token)
            .filter(|session| session.expires_at > now)
            .map(|session| session.pubkey_hash.clone())
    }

    /// Remove a session token (logout). Returns `true` if it existed.
    pub fn remove(&self, token: &str) -> bool {
        self.sessions.write().unwrap().remove(token).is_some()
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Request guard: requires a valid `Authorization: Bearer <session token>`
/// header and yields the session's `pubkey_hash`. Fails with 401 otherwise.
pub struct RequireUserSession {
    pub pubkey_hash: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RequireUserSession {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let token = match req
            .headers()
            .get_one("Authorization")
            .and_then(|h| h.strip_prefix("Bearer "))
        {
            Some(t) if !t.is_empty() => t,
            _ => return Outcome::Error((Status::Unauthorized, ())),
        };

        let store = match req.guard::<&State<SharedSessionStore>>().await {
            Outcome::Success(s) => s,
            _ => return Outcome::Error((Status::InternalServerError, ())),
        };

        match store.validate(token) {
            Some(pubkey_hash) => Outcome::Success(RequireUserSession { pubkey_hash }),
            None => Outcome::Error((Status::Unauthorized, ())),
        }
    }
}

/// Infallible guard that extracts an optional bearer token (used by logout,
/// which must succeed whether or not a session exists).
pub struct BearerToken(pub Option<String>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for BearerToken {
    type Error = std::convert::Infallible;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let token = req
            .headers()
            .get_one("Authorization")
            .and_then(|h| h.strip_prefix("Bearer "))
            .filter(|t| !t.is_empty())
            .map(str::to_owned);
        Outcome::Success(BearerToken(token))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_validate_roundtrip() {
        let store = SessionStore::new();
        store.insert("token-1", "hash-a");
        assert_eq!(store.validate("token-1").as_deref(), Some("hash-a"));
    }

    #[test]
    fn validate_unknown_token_returns_none() {
        let store = SessionStore::new();
        assert!(store.validate("nope").is_none());
    }

    #[test]
    fn expired_session_is_rejected() {
        let store = SessionStore::new();
        store.insert_with_expiry("stale", "hash-b", current_timestamp() - 1);
        assert!(store.validate("stale").is_none());
    }

    #[test]
    fn expired_sessions_are_purged_on_insert() {
        let store = SessionStore::new();
        store.insert_with_expiry("stale", "hash-b", current_timestamp() - 1);
        store.insert("fresh", "hash-c");
        // The stale entry must be gone from the map entirely.
        assert!(!store.remove("stale"));
        assert_eq!(store.validate("fresh").as_deref(), Some("hash-c"));
    }

    #[test]
    fn remove_deletes_session() {
        let store = SessionStore::new();
        store.insert("token-2", "hash-d");
        assert!(store.remove("token-2"));
        assert!(store.validate("token-2").is_none());
        assert!(!store.remove("token-2"));
    }
}
