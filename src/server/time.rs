use chrono::DateTime;

use crate::PasteMetadata;

#[derive(Copy, Clone)]
pub enum TimeLockState {
    TooEarly(i64),
    TooLate(i64),
}

pub fn current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

pub fn parse_timestamp(input: &str) -> Result<i64, String> {
    if let Ok(value) = input.parse::<i64>() {
        return Ok(value);
    }
    DateTime::parse_from_rfc3339(input)
        .map(|dt| dt.timestamp())
        .map_err(|_| "expected UNIX seconds or RFC3339 timestamp".to_string())
}

pub fn format_timestamp(ts: i64) -> String {
    DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| ts.to_string())
}

pub fn evaluate_time_lock(metadata: &PasteMetadata, now: i64) -> Option<TimeLockState> {
    if let Some(not_before) = metadata.not_before {
        if now < not_before {
            return Some(TimeLockState::TooEarly(not_before));
        }
    }
    if let Some(not_after) = metadata.not_after {
        if now > not_after {
            return Some(TimeLockState::TooLate(not_after));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_timestamp_accepts_unix_seconds() {
        assert_eq!(parse_timestamp("42").unwrap(), 42);
    }

    #[test]
    fn parse_timestamp_accepts_rfc3339() {
        assert_eq!(parse_timestamp("1970-01-01T00:00:10Z").unwrap(), 10);
    }

    #[test]
    fn parse_timestamp_rejects_invalid_input() {
        assert!(parse_timestamp("not-a-timestamp").is_err());
    }

    #[test]
    fn format_timestamp_renders_utc_string() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00:00 UTC");
    }

    #[test]
    fn format_timestamp_falls_back_for_invalid_input() {
        let rendered = format_timestamp(i64::MAX);
        assert_eq!(rendered, i64::MAX.to_string());
    }

    #[test]
    fn evaluate_time_lock_detects_too_early() {
        let metadata = PasteMetadata {
            not_before: Some(100),
            ..Default::default()
        };
        assert!(matches!(
            evaluate_time_lock(&metadata, 50),
            Some(TimeLockState::TooEarly(100))
        ));
    }

    #[test]
    fn evaluate_time_lock_detects_too_late() {
        let metadata = PasteMetadata {
            not_after: Some(20),
            ..Default::default()
        };
        assert!(matches!(
            evaluate_time_lock(&metadata, 30),
            Some(TimeLockState::TooLate(20))
        ));
    }

    #[test]
    fn evaluate_time_lock_allows_within_window() {
        let metadata = PasteMetadata {
            not_before: Some(10),
            not_after: Some(20),
            ..Default::default()
        };
        assert!(evaluate_time_lock(&metadata, 15).is_none());
    }
}
