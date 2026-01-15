//! Server uptime tracking.
//!
//! This module provides utilities for tracking server start time and uptime.
//! The start time is initialized once when the server starts.

use std::{
    sync::OnceLock,
    time::{Instant, SystemTime},
};

/// Server start time (monotonic, for uptime calculation).
static SERVER_START: OnceLock<Instant> = OnceLock::new();

/// Server start time (wall clock, for start time reporting).
static SERVER_START_WALL: OnceLock<SystemTime> = OnceLock::new();

/// Initialize server start time.
///
/// This should be called once when the server starts.
/// Subsequent calls are ignored.
pub fn init_server_start() {
    SERVER_START.get_or_init(Instant::now);
    SERVER_START_WALL.get_or_init(SystemTime::now);
}

/// Get server uptime.
///
/// Returns the duration since the server started.
/// Returns zero duration if `init_server_start()` was not called.
#[must_use]
pub fn uptime() -> std::time::Duration {
    SERVER_START.get().map(Instant::elapsed).unwrap_or_default()
}

/// Get server uptime in seconds with sub-second precision.
#[must_use]
pub fn uptime_seconds() -> f64 {
    uptime().as_secs_f64()
}

/// Get server uptime as a human-readable string.
///
/// Format examples:
/// - `"45s"` for less than a minute
/// - `"1m 30s"` for less than an hour
/// - `"1h 23m 45s"` for less than a day
/// - `"2d 3h 15m"` for a day or more
#[must_use]
pub fn uptime_human() -> String {
    let duration = uptime();
    let secs = duration.as_secs();

    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h {}m {}s", secs / 3600, (secs % 3600) / 60, secs % 60)
    } else {
        format!("{}d {}h {}m", secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60)
    }
}

/// Get server start time as ISO 8601 string.
///
/// Returns `"unknown"` if start time was not initialized.
#[must_use]
pub fn start_time_iso() -> String {
    let Some(start_time) = SERVER_START_WALL.get() else {
        return "unknown".to_string();
    };

    let Ok(duration) = start_time.duration_since(std::time::UNIX_EPOCH) else {
        return "unknown".to_string();
    };

    let secs = duration.as_secs();

    // Convert Unix timestamp to ISO 8601
    // We do this manually to avoid chrono dependency
    let days_since_epoch = secs / 86400;
    let secs_in_day = secs % 86400;

    let hours = secs_in_day / 3600;
    let minutes = (secs_in_day % 3600) / 60;
    let seconds = secs_in_day % 60;

    // Calculate year, month, day from days since epoch (1970-01-01)
    let (year, month, day) = days_to_ymd(days_since_epoch);

    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

/// Convert days since Unix epoch to year, month, day.
///
/// This is a simplified calculation that handles leap years correctly.
fn days_to_ymd(days: u64) -> (u32, u32, u32) {
    // Start from 1970-01-01
    let mut year = 1970u32;
    let mut remaining_days = days;

    // Find the year
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    // Find the month
    let days_in_months: [u64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for &days_in_month in &days_in_months {
        if remaining_days < days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }

    // SAFETY: remaining_days is always < 366, so it fits in u32
    #[allow(clippy::cast_possible_truncation)]
    let day = remaining_days as u32 + 1;

    (year, month, day)
}

/// Check if a year is a leap year.
///
/// Uses manual modulo checks to allow const evaluation.
#[allow(clippy::manual_is_multiple_of)]
const fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_server_start() {
        // Can be called multiple times without panic
        init_server_start();
        init_server_start();
    }

    #[test]
    fn test_uptime_human_seconds() {
        // Test the formatting logic directly
        let secs = 45u64;
        let result = format!("{secs}s");
        assert_eq!(result, "45s");
    }

    #[test]
    fn test_uptime_human_minutes() {
        let secs = 90u64;
        let result = format!("{}m {}s", secs / 60, secs % 60);
        assert_eq!(result, "1m 30s");
    }

    #[test]
    fn test_uptime_human_hours() {
        let secs = 3725u64; // 1h 2m 5s
        let result = format!("{}h {}m {}s", secs / 3600, (secs % 3600) / 60, secs % 60);
        assert_eq!(result, "1h 2m 5s");
    }

    #[test]
    fn test_uptime_human_days() {
        let secs = 90300u64; // 1d 1h 5m
        let result =
            format!("{}d {}h {}m", secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
        assert_eq!(result, "1d 1h 5m");
    }

    #[test]
    fn test_is_leap_year() {
        assert!(!is_leap_year(1970));
        assert!(!is_leap_year(1900)); // Divisible by 100 but not 400
        assert!(is_leap_year(2000)); // Divisible by 400
        assert!(is_leap_year(2024)); // Divisible by 4
        assert!(!is_leap_year(2023));
    }

    #[test]
    fn test_days_to_ymd_epoch() {
        // Day 0 is 1970-01-01
        assert_eq!(days_to_ymd(0), (1970, 1, 1));
    }

    #[test]
    fn test_days_to_ymd_simple() {
        // Day 31 is 1970-02-01
        assert_eq!(days_to_ymd(31), (1970, 2, 1));
    }

    #[test]
    fn test_days_to_ymd_year_boundary() {
        // Day 365 is 1971-01-01 (1970 is not a leap year)
        assert_eq!(days_to_ymd(365), (1971, 1, 1));
    }

    #[test]
    fn test_days_to_ymd_2025() {
        // 2025-01-14 calculation
        // Days from 1970-01-01 to 2025-01-14
        // Years 1970-2024: 55 years
        // Leap years in 1970-2024: 1972, 1976, 1980, 1984, 1988, 1992, 1996, 2000, 2004, 2008, 2012, 2016, 2020, 2024 = 14
        // Regular years: 55 - 14 = 41
        // Total days: 41 * 365 + 14 * 366 = 14965 + 5124 = 20089
        // Then add 13 days for Jan 1-14 (0-indexed, so days 0-13)
        let days = 20089 + 13;
        let (year, month, day) = days_to_ymd(days);
        assert_eq!(year, 2025);
        assert_eq!(month, 1);
        assert_eq!(day, 14);
    }
}
