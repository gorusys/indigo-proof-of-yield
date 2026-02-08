//! Normalization of slot/time for deterministic requests.

use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Error, Debug)]
pub enum NormalizeError {
    #[error("invalid slot: {0}")]
    InvalidSlot(String),
    #[error("invalid time: {0}")]
    InvalidTime(String),
}

/// Parse slot number from string (decimal).
#[allow(dead_code)]
pub fn parse_slot(s: &str) -> Result<u64, NormalizeError> {
    s.trim()
        .parse::<u64>()
        .map_err(|_| NormalizeError::InvalidSlot(s.to_string()))
}

/// Parse RFC3339 timestamp and return Unix timestamp for normalization.
pub fn parse_time_rfc3339(s: &str) -> Result<i64, NormalizeError> {
    let dt = OffsetDateTime::parse(s.trim(), &Rfc3339)
        .map_err(|e| NormalizeError::InvalidTime(e.to_string()))?;
    Ok(dt.unix_timestamp())
}

/// Normalize slot_or_time input: if it looks like a number, treat as slot; else RFC3339.
/// Returns (slot_opt, unix_ts_opt). Caller uses the appropriate one for the API.
pub fn normalize_slot_time(
    slot_or_time: &str,
) -> Result<(Option<u64>, Option<i64>), NormalizeError> {
    let s = slot_or_time.trim();
    if s.is_empty() {
        return Ok((None, None));
    }
    if let Ok(slot) = s.parse::<u64>() {
        return Ok((Some(slot), None));
    }
    let ts = parse_time_rfc3339(s)?;
    Ok((None, Some(ts)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_slot() {
        let (slot, ts) = normalize_slot_time("12345").unwrap();
        assert_eq!(slot, Some(12345));
        assert_eq!(ts, None);
    }

    #[test]
    fn normalize_time() {
        let (slot, ts) = normalize_slot_time("2026-02-08T05:32:54Z").unwrap();
        assert_eq!(slot, None);
        assert!(ts.is_some());
    }

    #[test]
    fn normalize_empty() {
        let (slot, ts) = normalize_slot_time("").unwrap();
        assert_eq!(slot, None);
        assert_eq!(ts, None);
    }
}
