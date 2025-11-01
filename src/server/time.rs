use chrono::DateTime;

use copypaste::PasteMetadata;

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
