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

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{rngs::OsRng, RngCore};
use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
    Request, State,
};

use super::time::current_timestamp;

/// Lifetime of a login session: 24 hours.
pub const SESSION_TTL_SECS: i64 = 24 * 60 * 60;

/// Login challenges are deliberately short-lived and single-use.
pub const CHALLENGE_TTL_SECS: i64 = 5 * 60;

/// Bound unauthenticated challenge state to prevent memory exhaustion.
const MAX_OUTSTANDING_CHALLENGES: usize = 10_000;

/// Bound authenticated session state to prevent memory exhaustion.
const MAX_ACTIVE_SESSIONS: usize = 10_000;

#[derive(Debug, Clone)]
struct Session {
    pubkey_hash: String,
    expires_at: i64,
}

/// In-memory session store, kept on Rocket managed state (mirrors the
/// `SharedRateLimiter` pattern in `api_keys.rs`). To keep normal insertions
/// O(1), expired entries are purged and the oldest live entry is evicted only
/// when the relevant map reaches its configured capacity.
pub struct SessionStore {
    sessions: RwLock<HashMap<String, Session>>,
    challenges: RwLock<HashMap<String, i64>>,
    max_active_sessions: usize,
    max_outstanding_challenges: usize,
}

pub type SharedSessionStore = Arc<SessionStore>;

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            challenges: RwLock::new(HashMap::new()),
            max_active_sessions: MAX_ACTIVE_SESSIONS,
            max_outstanding_challenges: MAX_OUTSTANDING_CHALLENGES,
        }
    }

    #[cfg(test)]
    fn with_capacities(max_active_sessions: usize, max_outstanding_challenges: usize) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            challenges: RwLock::new(HashMap::new()),
            max_active_sessions: max_active_sessions.max(1),
            max_outstanding_challenges: max_outstanding_challenges.max(1),
        }
    }

    /// Issue and remember a cryptographically random login challenge.
    pub fn issue_challenge(&self) -> String {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        let challenge = URL_SAFE_NO_PAD.encode(bytes);
        self.insert_challenge_with_expiry(&challenge, current_timestamp() + CHALLENGE_TTL_SECS);
        challenge
    }

    fn insert_challenge_with_expiry(&self, challenge: &str, expires_at: i64) {
        let mut challenges = self.challenges.write().unwrap();
        if !challenges.contains_key(challenge)
            && challenges.len() >= self.max_outstanding_challenges
        {
            let now = current_timestamp();
            challenges.retain(|_, expiry| *expiry > now);
            if challenges.len() >= self.max_outstanding_challenges {
                if let Some(oldest) = challenges
                    .iter()
                    .min_by_key(|(_, expiry)| **expiry)
                    .map(|(challenge, _)| challenge.clone())
                {
                    challenges.remove(&oldest);
                }
            }
        }
        challenges.insert(challenge.to_owned(), expires_at);
    }

    /// Atomically consume a valid issued challenge. Unknown, expired, and
    /// already-consumed challenges all return `false`.
    pub fn consume_challenge(&self, challenge: &str) -> bool {
        let now = current_timestamp();
        let mut challenges = self.challenges.write().unwrap();
        match challenges.remove(challenge) {
            Some(expires_at) => expires_at > now,
            None => false,
        }
    }

    /// Register a session token for a pubkey hash (24 h expiry).
    pub fn insert(&self, token: &str, pubkey_hash: &str) {
        self.insert_with_expiry(token, pubkey_hash, current_timestamp() + SESSION_TTL_SECS);
    }

    /// Register a session token with an explicit expiry timestamp.
    /// At capacity, expired sessions are purged before the oldest live session
    /// is evicted to make room.
    pub fn insert_with_expiry(&self, token: &str, pubkey_hash: &str, expires_at: i64) {
        let mut map = self.sessions.write().unwrap();
        if !map.contains_key(token) && map.len() >= self.max_active_sessions {
            let now = current_timestamp();
            map.retain(|_, session| session.expires_at > now);
            if map.len() >= self.max_active_sessions {
                if let Some(oldest) = map
                    .iter()
                    .min_by_key(|(_, session)| session.expires_at)
                    .map(|(token, _)| token.clone())
                {
                    map.remove(&oldest);
                }
            }
        }
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
    fn expired_sessions_are_purged_when_capacity_is_reached() {
        let store = SessionStore::with_capacities(2, 2);
        store.insert_with_expiry("stale", "hash-b", current_timestamp() - 1);
        store.insert("active", "hash-c");
        store.insert("fresh", "hash-c");
        // Reaching capacity purges the stale entry before considering a live
        // session for eviction.
        assert!(!store.remove("stale"));
        assert_eq!(store.validate("active").as_deref(), Some("hash-c"));
        assert_eq!(store.validate("fresh").as_deref(), Some("hash-c"));
        assert_eq!(store.sessions.read().unwrap().len(), 2);
    }

    #[test]
    fn sessions_are_bounded_and_evict_oldest_at_capacity() {
        let store = SessionStore::with_capacities(2, 2);
        let now = current_timestamp();
        store.insert_with_expiry("oldest", "hash-a", now + 10);
        store.insert_with_expiry("newer", "hash-b", now + 20);
        store.insert_with_expiry("newest", "hash-c", now + 30);

        assert!(store.validate("oldest").is_none());
        assert_eq!(store.validate("newer").as_deref(), Some("hash-b"));
        assert_eq!(store.validate("newest").as_deref(), Some("hash-c"));
        assert_eq!(store.sessions.read().unwrap().len(), 2);
    }

    #[test]
    fn remove_deletes_session() {
        let store = SessionStore::new();
        store.insert("token-2", "hash-d");
        assert!(store.remove("token-2"));
        assert!(store.validate("token-2").is_none());
        assert!(!store.remove("token-2"));
    }

    #[test]
    fn challenge_is_single_use() {
        let store = SessionStore::new();
        let challenge = store.issue_challenge();
        assert!(store.consume_challenge(&challenge));
        assert!(!store.consume_challenge(&challenge));
    }

    #[test]
    fn unknown_and_expired_challenges_are_rejected() {
        let store = SessionStore::new();
        assert!(!store.consume_challenge("not-issued"));

        store.insert_challenge_with_expiry("expired", current_timestamp() - 1);
        assert!(!store.consume_challenge("expired"));
    }

    #[test]
    fn challenges_are_bounded_and_evict_oldest_at_capacity() {
        let store = SessionStore::with_capacities(2, 2);
        let now = current_timestamp();
        store.insert_challenge_with_expiry("oldest", now + 10);
        store.insert_challenge_with_expiry("newer", now + 20);
        store.insert_challenge_with_expiry("newest", now + 30);

        assert!(!store.consume_challenge("oldest"));
        assert!(store.consume_challenge("newer"));
        assert!(store.consume_challenge("newest"));
        assert!(store.challenges.read().unwrap().is_empty());
    }

    #[test]
    fn expired_challenges_are_purged_at_capacity_before_live_eviction() {
        let store = SessionStore::with_capacities(2, 2);
        let now = current_timestamp();
        store.insert_challenge_with_expiry("expired", now - 1);
        store.insert_challenge_with_expiry("active", now + 20);
        store.insert_challenge_with_expiry("fresh", now + 30);

        assert!(!store.consume_challenge("expired"));
        assert!(store.consume_challenge("active"));
        assert!(store.consume_challenge("fresh"));
    }
}
