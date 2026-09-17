//! First-party visit counts. No paste ids, no IPs, no cookies.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::server::time::current_timestamp;

pub type SharedTraffic = Arc<TrafficStore>;

pub struct TrafficStore {
    started_at: i64,
    pageviews: AtomicU64,
    pages: Mutex<BTreeMap<String, u64>>,
    referrers: Mutex<BTreeMap<String, u64>>,
    devices: Mutex<BTreeMap<String, u64>>,
    oses: Mutex<BTreeMap<String, u64>>,
    countries: Mutex<BTreeMap<String, u64>>,
}

impl Default for TrafficStore {
    fn default() -> Self {
        Self {
            started_at: current_timestamp(),
            pageviews: AtomicU64::new(0),
            pages: Mutex::new(BTreeMap::new()),
            referrers: Mutex::new(BTreeMap::new()),
            devices: Mutex::new(BTreeMap::new()),
            oses: Mutex::new(BTreeMap::new()),
            countries: Mutex::new(BTreeMap::new()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CollectBody {
    pub path: Option<String>,
    pub referrer: Option<String>,
    pub device: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrafficResponse {
    pub pageviews: u64,
    pub started_at: i64,
    pub pages: Vec<NamedCount>,
    pub referrers: Vec<NamedCount>,
    pub devices: Vec<NamedCount>,
    pub oses: Vec<NamedCount>,
    pub countries: Vec<NamedCount>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NamedCount {
    pub name: String,
    pub count: u64,
}

impl TrafficStore {
    pub fn record(&self, path: &str, referrer: &str, device: &str, os: &str, country: &str) {
        self.pageviews.fetch_add(1, Ordering::Relaxed);
        bump(&self.pages, path);
        bump(&self.referrers, referrer);
        bump(&self.devices, device);
        bump(&self.oses, os);
        bump(&self.countries, country);
    }

    pub fn snapshot(&self) -> TrafficResponse {
        TrafficResponse {
            pageviews: self.pageviews.load(Ordering::Relaxed),
            started_at: self.started_at,
            pages: snapshot(&self.pages),
            referrers: snapshot(&self.referrers),
            devices: snapshot(&self.devices),
            oses: snapshot(&self.oses),
            countries: snapshot(&self.countries),
        }
    }
}

fn bump(map: &Mutex<BTreeMap<String, u64>>, key: &str) {
    if let Ok(mut guard) = map.lock() {
        if !guard.contains_key(key) && guard.len() >= 256 {
            if let Some(smallest) = guard
                .iter()
                .min_by_key(|(_, count)| *count)
                .map(|(name, _)| name.clone())
            {
                guard.remove(&smallest);
            }
        }
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
        || ua.contains("slurp")
    {
        return "bot";
    }
    if ua.contains("ipad")
        || ua.contains("tablet")
        || (ua.contains("android") && !ua.contains("mobile"))
    {
        return "tablet";
    }
    if ua.contains("mobile") || ua.contains("iphone") || ua.contains("android") {
        return "phone";
    }
    "desktop"
}

pub fn classify_os(ua: Option<&str>) -> &'static str {
    let ua = ua.unwrap_or("").to_ascii_lowercase();
    if ua.is_empty() {
        return "unknown";
    }
    if ua.contains("iphone") || ua.contains("ipad") || ua.contains("ios") {
        return "ios";
    }
    if ua.contains("android") {
        return "android";
    }
    if ua.contains("windows") {
        return "windows";
    }
    if ua.contains("mac os") || ua.contains("macos") || ua.contains("macintosh") {
        return "macos";
    }
    if ua.contains("cros") {
        return "chromeos";
    }
    if ua.contains("linux") {
        return "linux";
    }
    "other"
}

pub fn classify_country(header: Option<&str>, language: Option<&str>) -> String {
    if let Some(code) = header.and_then(two_letter_country) {
        return code;
    }
    if let Some(code) = language.and_then(region_from_language) {
        return code;
    }
    "unknown".into()
}

fn two_letter_country(raw: &str) -> Option<String> {
    let code = raw.trim().to_ascii_uppercase();
    if code.len() == 2
        && code.bytes().all(|b| b.is_ascii_alphabetic())
        && code != "XX"
        && code != "T1"
    {
        Some(code)
    } else {
        None
    }
}

fn region_from_language(raw: &str) -> Option<String> {
    let primary = raw.split(',').next()?.trim();
    let tag = primary.split(';').next()?.trim();
    let region = tag.split('-').nth(1)?.chars().take(2).collect::<String>();
    two_letter_country(&region)
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

    #[test]
    fn device_os_and_country_are_coarse() {
        assert_eq!(
            classify_device(Some("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0)")),
            "phone"
        );
        assert_eq!(
            classify_os(Some("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0)")),
            "ios"
        );
        assert_eq!(
            classify_device(Some("Mozilla/5.0 (iPad; CPU OS 17_0)")),
            "tablet"
        );
        assert_eq!(
            classify_os(Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")),
            "windows"
        );
        assert_eq!(classify_country(Some("PL"), None), "PL");
        assert_eq!(classify_country(None, Some("en-GB,en;q=0.9")), "GB");
        assert_eq!(classify_country(None, None), "unknown");
    }
}
