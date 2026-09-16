//! First-party visit counts. No paste ids, no IPs, no cookies.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub type SharedTraffic = Arc<TrafficStore>;

#[derive(Default)]
pub struct TrafficStore {
    pageviews: AtomicU64,
    pages: Mutex<BTreeMap<String, u64>>,
    referrers: Mutex<BTreeMap<String, u64>>,
    devices: Mutex<BTreeMap<String, u64>>,
}

#[derive(Debug, Deserialize)]
pub struct CollectBody {
    pub path: Option<String>,
    pub referrer: Option<String>,
    pub device: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrafficResponse {
    pub pageviews: u64,
    pub pages: Vec<NamedCount>,
    pub referrers: Vec<NamedCount>,
    pub devices: Vec<NamedCount>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NamedCount {
    pub name: String,
    pub count: u64,
}

impl TrafficStore {
    pub fn record(&self, path: &str, referrer: &str, device: &str) {
        self.pageviews.fetch_add(1, Ordering::Relaxed);
        bump(&self.pages, path);
        bump(&self.referrers, referrer);
        bump(&self.devices, device);
    }

    pub fn snapshot(&self) -> TrafficResponse {
        TrafficResponse {
            pageviews: self.pageviews.load(Ordering::Relaxed),
            pages: snapshot(&self.pages),
            referrers: snapshot(&self.referrers),
            devices: snapshot(&self.devices),
        }
    }
}

fn bump(map: &Mutex<BTreeMap<String, u64>>, key: &str) {
    if let Ok(mut guard) = map.lock() {
        *guard.entry(key.to_string()).or_insert(0) += 1;
    }
}

fn snapshot(map: &Mutex<BTreeMap<String, u64>>) -> Vec<NamedCount> {
    map.lock()
        .map(|guard| {
            let mut rows: Vec<NamedCount> = guard
                .iter()
                .map(|(name, count)| NamedCount {
                    name: name.clone(),
                    count: *count,
                })
                .collect();
            rows.sort_by(|a, b| b.count.cmp(&a.count).then(a.name.cmp(&b.name)));
            rows.truncate(20);
            rows
        })
        .unwrap_or_default()
}

pub fn classify_path(raw: &str) -> &'static str {
    let path = raw
        .split('?')
        .next()
        .unwrap_or(raw)
        .split('#')
        .next()
        .unwrap_or(raw);
    match path {
        "/" | "" => "home",
        "/about" => "about",
        "/stats" => "stats",
        "/login" => "login",
        other if other.starts_with("/p/") || other.starts_with("/raw/") => "share",
        _ => "other",
    }
}

pub fn classify_referrer(raw: Option<&str>) -> String {
    let Some(value) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return "direct".into();
    };
    let parsed = url::Url::parse(value).ok();
    let host = parsed
        .as_ref()
        .and_then(|u| u.host_str())
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_lowercase();
    if host.is_empty()
        || host == "copypaste.fyi"
        || host.ends_with(".copypaste.fyi")
        || host == "localhost"
        || host == "127.0.0.1"
    {
        return "direct".into();
    }
    host.chars().take(80).collect()
}

pub fn classify_device(ua: Option<&str>) -> &'static str {
    let ua = ua.unwrap_or("").to_ascii_lowercase();
    if ua.is_empty() {
        return "unknown";
    }
    if ua.contains("bot")
        || ua.contains("spider")
        || ua.contains("crawler")
        || ua.contains("preview")
    {
        return "bot";
    }
    if ua.contains("mobile") || ua.contains("android") || ua.contains("iphone") {
        return "mobile";
    }
    "desktop"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_paste_ids_and_query_from_path() {
        assert_eq!(classify_path("/p/9LIhAn9e5WLd7Mo0n01LsxgK#key=x"), "share");
        assert_eq!(classify_path("/about?utm=1"), "about");
        assert_eq!(classify_path("/"), "home");
    }

    #[test]
    fn keeps_only_referrer_host() {
        assert_eq!(classify_referrer(Some("https://t.co/abc?s=1")), "t.co");
        assert_eq!(
            classify_referrer(Some("https://www.copypaste.fyi/p/secret")),
            "direct"
        );
        assert_eq!(classify_referrer(None), "direct");
    }
}
