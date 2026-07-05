//! Per-IP fixed-window rate limiting for paste creation and reads.
//!
//! Wires up the `[rate_limit]` config knobs (`creates_per_minute`,
//! `reads_per_minute`) that were previously parsed and validated but never
//! consumed. `config::Config::bridge_to_env` exports them as
//! `COPYPASTE_RATE_LIMIT_CREATES` / `COPYPASTE_RATE_LIMIT_READS`; this module
//! reads those env vars at rocket build time. When a knob is unset (or `0`),
//! the corresponding limiter is disabled, so embedded/test usage is unaffected.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
    Request, State,
};

/// Fixed rate-limit window length.
const WINDOW: Duration = Duration::from_secs(60);

/// Purge stale windows once the map grows beyond this many client entries.
const PURGE_THRESHOLD: usize = 10_000;

/// Per-IP fixed-window counters for paste creates and reads.
pub struct PasteRateLimiter {
    creates_per_minute: Option<u32>,
    reads_per_minute: Option<u32>,
    creates: Mutex<HashMap<String, (u32, Instant)>>,
    reads: Mutex<HashMap<String, (u32, Instant)>>,
}

impl PasteRateLimiter {
    /// `None` (or `Some(0)`) disables the corresponding limiter.
    pub fn new(creates_per_minute: Option<u32>, reads_per_minute: Option<u32>) -> Self {
        Self {
            creates_per_minute: creates_per_minute.filter(|n| *n > 0),
            reads_per_minute: reads_per_minute.filter(|n| *n > 0),
            creates: Mutex::new(HashMap::new()),
            reads: Mutex::new(HashMap::new()),
        }
    }

    /// Build from `COPYPASTE_RATE_LIMIT_CREATES` / `COPYPASTE_RATE_LIMIT_READS`.
    /// Unset, unparsable, or zero values disable the respective limiter.
    pub fn from_env() -> Self {
        Self::new(
            limit_from_env("COPYPASTE_RATE_LIMIT_CREATES"),
            limit_from_env("COPYPASTE_RATE_LIMIT_READS"),
        )
    }

    /// Returns `true` when a create request from `ip` is allowed.
    pub fn allow_create(&self, ip: &str) -> bool {
        Self::allow(&self.creates, self.creates_per_minute, ip)
    }

    /// Returns `true` when a read request from `ip` is allowed.
    pub fn allow_read(&self, ip: &str) -> bool {
        Self::allow(&self.reads, self.reads_per_minute, ip)
    }

    fn allow(map: &Mutex<HashMap<String, (u32, Instant)>>, limit: Option<u32>, ip: &str) -> bool {
        let Some(limit) = limit else {
            return true;
        };
        let mut map = map.lock().unwrap();
        let now = Instant::now();
        if map.len() > PURGE_THRESHOLD {
            map.retain(|_, (_, start)| now.duration_since(*start) <= WINDOW);
        }
        let entry = map.entry(ip.to_owned()).or_insert((0, now));
        if now.duration_since(entry.1) > WINDOW {
            *entry = (0, now);
        }
        if entry.0 >= limit {
            return false;
        }
        entry.0 += 1;
        true
    }
}

fn limit_from_env(name: &str) -> Option<u32> {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|n| *n > 0)
}

fn client_key(req: &Request<'_>) -> String {
    req.client_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Request guard enforcing the create limit; fails with 429 when exceeded.
pub struct CreateRateLimit;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for CreateRateLimit {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let limiter = match req.guard::<&State<PasteRateLimiter>>().await {
            Outcome::Success(limiter) => limiter,
            _ => return Outcome::Success(CreateRateLimit),
        };
        if limiter.allow_create(&client_key(req)) {
            Outcome::Success(CreateRateLimit)
        } else {
            Outcome::Error((Status::TooManyRequests, ()))
        }
    }
}

/// Request guard enforcing the read limit; fails with 429 when exceeded.
pub struct ReadRateLimit;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ReadRateLimit {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let limiter = match req.guard::<&State<PasteRateLimiter>>().await {
            Outcome::Success(limiter) => limiter,
            _ => return Outcome::Success(ReadRateLimit),
        };
        if limiter.allow_read(&client_key(req)) {
            Outcome::Success(ReadRateLimit)
        } else {
            Outcome::Error((Status::TooManyRequests, ()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_limiter_allows_everything() {
        let limiter = PasteRateLimiter::new(None, None);
        for _ in 0..1000 {
            assert!(limiter.allow_create("1.2.3.4"));
            assert!(limiter.allow_read("1.2.3.4"));
        }
    }

    #[test]
    fn zero_limit_means_disabled() {
        let limiter = PasteRateLimiter::new(Some(0), Some(0));
        for _ in 0..100 {
            assert!(limiter.allow_create("1.2.3.4"));
            assert!(limiter.allow_read("1.2.3.4"));
        }
    }

    #[test]
    fn create_limit_blocks_after_threshold() {
        let limiter = PasteRateLimiter::new(Some(3), None);
        assert!(limiter.allow_create("5.6.7.8"));
        assert!(limiter.allow_create("5.6.7.8"));
        assert!(limiter.allow_create("5.6.7.8"));
        assert!(!limiter.allow_create("5.6.7.8"));
        // Reads are unlimited even when creates are limited.
        assert!(limiter.allow_read("5.6.7.8"));
    }

    #[test]
    fn read_limit_blocks_after_threshold() {
        let limiter = PasteRateLimiter::new(None, Some(2));
        assert!(limiter.allow_read("9.9.9.9"));
        assert!(limiter.allow_read("9.9.9.9"));
        assert!(!limiter.allow_read("9.9.9.9"));
    }

    #[test]
    fn limits_are_tracked_per_ip() {
        let limiter = PasteRateLimiter::new(Some(1), None);
        assert!(limiter.allow_create("10.0.0.1"));
        assert!(!limiter.allow_create("10.0.0.1"));
        assert!(limiter.allow_create("10.0.0.2"));
    }

    #[test]
    fn from_env_disabled_when_unset() {
        std::env::remove_var("COPYPASTE_RATE_LIMIT_CREATES");
        std::env::remove_var("COPYPASTE_RATE_LIMIT_READS");
        let limiter = PasteRateLimiter::from_env();
        for _ in 0..100 {
            assert!(limiter.allow_create("1.1.1.1"));
            assert!(limiter.allow_read("1.1.1.1"));
        }
    }
}
