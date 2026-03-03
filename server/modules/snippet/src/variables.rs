//! Variable resolution for snippet expansion (#136, Phase 3).
//!
//! Resolves built-in variables (`TM_FILENAME`, `CURRENT_YEAR`, etc.)
//! during snippet expansion.

use std::time::SystemTime;

/// Context for variable resolution during snippet expansion.
///
/// Populated from the session runtime before expanding a snippet body.
#[derive(Debug, Clone, Default)]
pub struct VariableContext {
    /// Full file path of the active buffer (e.g., "/src/main.rs").
    pub file_path: Option<String>,
    /// Currently selected text, if any.
    pub selected_text: Option<String>,
    /// Clipboard contents.
    pub clipboard: Option<String>,
    /// Zero-based line number of the cursor.
    pub line_number: usize,
}

impl VariableContext {
    /// Create an empty variable context.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            file_path: None,
            selected_text: None,
            clipboard: None,
            line_number: 0,
        }
    }
}

/// Resolve a built-in variable by name.
///
/// Returns `Some(value)` if the variable is known and can be resolved,
/// `None` if the variable is unknown.
#[must_use]
pub fn resolve_variable(name: &str, ctx: &VariableContext) -> Option<String> {
    match name {
        // File variables
        "TM_FILENAME" => ctx.file_path.as_deref().map(filename),
        "TM_FILENAME_BASE" => ctx.file_path.as_deref().map(filename_base),
        "TM_FILEPATH" => ctx.file_path.clone(),
        "TM_DIRECTORY" => ctx.file_path.as_deref().map(directory),

        // Cursor/selection variables
        "TM_LINE_INDEX" => Some(ctx.line_number.to_string()),
        "TM_LINE_NUMBER" => Some((ctx.line_number + 1).to_string()),
        "TM_SELECTED_TEXT" => ctx.selected_text.clone(),
        "CLIPBOARD" => ctx.clipboard.clone(),

        // Date/time variables
        "CURRENT_YEAR" => Some(current_datetime("%Y")),
        "CURRENT_YEAR_SHORT" => Some(current_datetime("%y")),
        "CURRENT_MONTH" => Some(current_datetime("%m")),
        "CURRENT_MONTH_NAME" => Some(current_datetime("%B")),
        "CURRENT_MONTH_NAME_SHORT" => Some(current_datetime("%b")),
        "CURRENT_DATE" => Some(current_datetime("%d")),
        "CURRENT_DAY_NAME" => Some(current_datetime("%A")),
        "CURRENT_DAY_NAME_SHORT" => Some(current_datetime("%a")),
        "CURRENT_HOUR" => Some(current_datetime("%H")),
        "CURRENT_MINUTE" => Some(current_datetime("%M")),
        "CURRENT_SECOND" => Some(current_datetime("%S")),
        "CURRENT_SECONDS_UNIX" => Some(unix_timestamp()),

        // Random variables
        "RANDOM" => Some(random_decimal()),
        "RANDOM_HEX" => Some(random_hex()),
        "UUID" => Some(uuid_v4()),

        _ => None,
    }
}

// =============================================================================
// File helpers
// =============================================================================

/// Extract filename from a path (e.g., "/src/main.rs" → "main.rs").
fn filename(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Extract filename without extension (e.g., "/src/main.rs" → "main").
fn filename_base(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Extract directory from a path (e.g., "/src/main.rs" → "/src").
fn directory(path: &str) -> String {
    std::path::Path::new(path)
        .parent()
        .map_or_else(String::new, |p| p.to_string_lossy().into_owned())
}

// =============================================================================
// Date/time helpers (no external dependency)
// =============================================================================

/// Format current local time using a simple strftime-like format.
///
/// Only supports the specific format specifiers needed for snippet variables.
fn current_datetime(fmt: &str) -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    let (year, month, day, hour, minute, second, weekday) = unix_to_local_datetime(secs);

    match fmt {
        "%Y" => format!("{year:04}"),
        "%y" => format!("{:02}", year % 100),
        "%m" => format!("{month:02}"),
        "%d" => format!("{day:02}"),
        "%H" => format!("{hour:02}"),
        "%M" => format!("{minute:02}"),
        "%S" => format!("{second:02}"),
        "%B" => month_name(month).to_string(),
        "%b" => month_name_short(month).to_string(),
        "%A" => day_name(weekday).to_string(),
        "%a" => day_name_short(weekday).to_string(),
        _ => String::new(),
    }
}

/// Convert Unix timestamp to local datetime components.
///
/// Returns (year, month, day, hour, minute, second, weekday).
/// Weekday: 0=Sunday, 1=Monday, ..., 6=Saturday.
///
/// Uses UTC (no timezone offset). For snippet variables, UTC is acceptable
/// as a baseline; proper timezone support can be added later if needed.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
const fn unix_to_local_datetime(secs: u64) -> (i32, u32, u32, u32, u32, u32, u32) {
    let days = (secs / 86400).cast_signed();
    let time_of_day = secs % 86400;

    let hour = (time_of_day / 3600) as u32;
    let minute = ((time_of_day % 3600) / 60) as u32;
    let second = (time_of_day % 60) as u32;

    // Day of week: Jan 1, 1970 was Thursday (4)
    let weekday = ((days + 4).rem_euclid(7)) as u32;

    // Civil date from days since epoch
    let (year, month, day) = civil_from_days(days);

    (year, month, day, hour, minute, second, weekday)
}

/// Convert days since Unix epoch to (year, month, day).
///
/// Algorithm from Howard Hinnant's `chrono`-compatible date library.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
const fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    #[allow(clippy::cast_lossless)]
    let y = (yoe as i64 + era * 400) as i32;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

const fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

const fn month_name_short(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "Unk",
    }
}

const fn day_name(weekday: u32) -> &'static str {
    match weekday {
        0 => "Sunday",
        1 => "Monday",
        2 => "Tuesday",
        3 => "Wednesday",
        4 => "Thursday",
        5 => "Friday",
        6 => "Saturday",
        _ => "Unknown",
    }
}

const fn day_name_short(weekday: u32) -> &'static str {
    match weekday {
        0 => "Sun",
        1 => "Mon",
        2 => "Tue",
        3 => "Wed",
        4 => "Thu",
        5 => "Fri",
        6 => "Sat",
        _ => "Unk",
    }
}

fn unix_timestamp() -> String {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
        .to_string()
}

// =============================================================================
// Random helpers (no external dependency)
// =============================================================================

/// Generate a 6-digit random decimal string (000000-999999).
fn random_decimal() -> String {
    format!("{:06}", simple_random() % 1_000_000)
}

/// Generate a 6-character random hex string (000000-ffffff).
fn random_hex() -> String {
    format!("{:06x}", simple_random() % 0x1_000_000)
}

/// Generate a UUID v4 (random).
fn uuid_v4() -> String {
    let r1 = simple_random();
    let r2 = simple_random();

    let r1_bytes = r1.to_be_bytes();
    let r2_bytes = r2.to_be_bytes();

    let b = [
        r1_bytes[0],
        r1_bytes[1],
        r1_bytes[2],
        r1_bytes[3],
        r1_bytes[4],
        r1_bytes[5],
        (r1_bytes[6] & 0x0F) | 0x40, // version 4
        r1_bytes[7],
        (r2_bytes[0] & 0x3F) | 0x80, // variant 1
        r2_bytes[1],
        r2_bytes[2],
        r2_bytes[3],
        r2_bytes[4],
        r2_bytes[5],
        r2_bytes[6],
        r2_bytes[7],
    ];

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0],
        b[1],
        b[2],
        b[3],
        b[4],
        b[5],
        b[6],
        b[7],
        b[8],
        b[9],
        b[10],
        b[11],
        b[12],
        b[13],
        b[14],
        b[15],
    )
}

/// Simple pseudorandom number using timing entropy.
///
/// Good enough for snippet variable randomness (not cryptographic).
fn simple_random() -> u64 {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };

    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());

    // Mix timing with thread ID and counter for uniqueness
    let mut hasher = DefaultHasher::new();
    now.hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    COUNTER
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx() -> VariableContext {
        VariableContext {
            file_path: Some("/home/user/project/src/main.rs".to_string()),
            selected_text: Some("selected text".to_string()),
            clipboard: Some("clipboard content".to_string()),
            line_number: 42,
        }
    }

    // =========================================================================
    // VariableContext
    // =========================================================================

    #[test]
    fn test_variable_context_empty() {
        let ctx = VariableContext::empty();
        assert!(ctx.file_path.is_none());
        assert!(ctx.selected_text.is_none());
        assert!(ctx.clipboard.is_none());
        assert_eq!(ctx.line_number, 0);
    }

    #[test]
    fn test_variable_context_default() {
        let ctx = VariableContext::default();
        assert!(ctx.file_path.is_none());
    }

    #[test]
    fn test_variable_context_clone_debug() {
        let ctx = test_ctx();
        let cloned = ctx.clone();
        assert_eq!(cloned.line_number, 42);
        let debug = format!("{ctx:?}");
        assert!(debug.contains("VariableContext"));
    }

    // =========================================================================
    // File variables
    // =========================================================================

    #[test]
    fn test_tm_filename() {
        let ctx = test_ctx();
        assert_eq!(resolve_variable("TM_FILENAME", &ctx).unwrap(), "main.rs");
    }

    #[test]
    fn test_tm_filename_base() {
        let ctx = test_ctx();
        assert_eq!(resolve_variable("TM_FILENAME_BASE", &ctx).unwrap(), "main");
    }

    #[test]
    fn test_tm_filepath() {
        let ctx = test_ctx();
        assert_eq!(
            resolve_variable("TM_FILEPATH", &ctx).unwrap(),
            "/home/user/project/src/main.rs"
        );
    }

    #[test]
    fn test_tm_directory() {
        let ctx = test_ctx();
        assert_eq!(resolve_variable("TM_DIRECTORY", &ctx).unwrap(), "/home/user/project/src");
    }

    #[test]
    fn test_file_vars_no_path() {
        let ctx = VariableContext::empty();
        assert!(resolve_variable("TM_FILENAME", &ctx).is_none());
        assert!(resolve_variable("TM_FILENAME_BASE", &ctx).is_none());
        assert!(resolve_variable("TM_FILEPATH", &ctx).is_none());
        assert!(resolve_variable("TM_DIRECTORY", &ctx).is_none());
    }

    // =========================================================================
    // Cursor/selection variables
    // =========================================================================

    #[test]
    fn test_tm_line_index() {
        let ctx = test_ctx();
        assert_eq!(resolve_variable("TM_LINE_INDEX", &ctx).unwrap(), "42");
    }

    #[test]
    fn test_tm_line_number() {
        let ctx = test_ctx();
        assert_eq!(resolve_variable("TM_LINE_NUMBER", &ctx).unwrap(), "43");
    }

    #[test]
    fn test_tm_selected_text() {
        let ctx = test_ctx();
        assert_eq!(resolve_variable("TM_SELECTED_TEXT", &ctx).unwrap(), "selected text");
    }

    #[test]
    fn test_tm_selected_text_none() {
        let ctx = VariableContext::empty();
        assert!(resolve_variable("TM_SELECTED_TEXT", &ctx).is_none());
    }

    #[test]
    fn test_clipboard() {
        let ctx = test_ctx();
        assert_eq!(resolve_variable("CLIPBOARD", &ctx).unwrap(), "clipboard content");
    }

    #[test]
    fn test_clipboard_none() {
        let ctx = VariableContext::empty();
        assert!(resolve_variable("CLIPBOARD", &ctx).is_none());
    }

    // =========================================================================
    // Date/time variables (verify format, not exact values)
    // =========================================================================

    #[test]
    fn test_current_year() {
        let ctx = VariableContext::empty();
        let year = resolve_variable("CURRENT_YEAR", &ctx).unwrap();
        assert_eq!(year.len(), 4);
        assert!(year.parse::<u32>().is_ok());
    }

    #[test]
    fn test_current_year_short() {
        let ctx = VariableContext::empty();
        let year = resolve_variable("CURRENT_YEAR_SHORT", &ctx).unwrap();
        assert_eq!(year.len(), 2);
    }

    #[test]
    fn test_current_month() {
        let ctx = VariableContext::empty();
        let month = resolve_variable("CURRENT_MONTH", &ctx).unwrap();
        assert_eq!(month.len(), 2);
        let m: u32 = month.parse().unwrap();
        assert!((1..=12).contains(&m));
    }

    #[test]
    fn test_current_month_name() {
        let ctx = VariableContext::empty();
        let name = resolve_variable("CURRENT_MONTH_NAME", &ctx).unwrap();
        assert!(!name.is_empty());
    }

    #[test]
    fn test_current_month_name_short() {
        let ctx = VariableContext::empty();
        let name = resolve_variable("CURRENT_MONTH_NAME_SHORT", &ctx).unwrap();
        assert_eq!(name.len(), 3);
    }

    #[test]
    fn test_current_date() {
        let ctx = VariableContext::empty();
        let day = resolve_variable("CURRENT_DATE", &ctx).unwrap();
        assert_eq!(day.len(), 2);
        let d: u32 = day.parse().unwrap();
        assert!((1..=31).contains(&d));
    }

    #[test]
    fn test_current_day_name() {
        let ctx = VariableContext::empty();
        let name = resolve_variable("CURRENT_DAY_NAME", &ctx).unwrap();
        assert!(!name.is_empty());
    }

    #[test]
    fn test_current_day_name_short() {
        let ctx = VariableContext::empty();
        let name = resolve_variable("CURRENT_DAY_NAME_SHORT", &ctx).unwrap();
        assert_eq!(name.len(), 3);
    }

    #[test]
    fn test_current_hour() {
        let ctx = VariableContext::empty();
        let hour = resolve_variable("CURRENT_HOUR", &ctx).unwrap();
        assert_eq!(hour.len(), 2);
        let h: u32 = hour.parse().unwrap();
        assert!(h < 24);
    }

    #[test]
    fn test_current_minute() {
        let ctx = VariableContext::empty();
        let min = resolve_variable("CURRENT_MINUTE", &ctx).unwrap();
        assert_eq!(min.len(), 2);
        let m: u32 = min.parse().unwrap();
        assert!(m < 60);
    }

    #[test]
    fn test_current_second() {
        let ctx = VariableContext::empty();
        let sec = resolve_variable("CURRENT_SECOND", &ctx).unwrap();
        assert_eq!(sec.len(), 2);
        let s: u32 = sec.parse().unwrap();
        assert!(s < 60);
    }

    #[test]
    fn test_current_seconds_unix() {
        let ctx = VariableContext::empty();
        let ts = resolve_variable("CURRENT_SECONDS_UNIX", &ctx).unwrap();
        let secs: u64 = ts.parse().unwrap();
        // Should be a reasonable Unix timestamp (after 2020)
        assert!(secs > 1_577_836_800);
    }

    // =========================================================================
    // Random variables
    // =========================================================================

    #[test]
    fn test_random_decimal() {
        let ctx = VariableContext::empty();
        let val = resolve_variable("RANDOM", &ctx).unwrap();
        assert_eq!(val.len(), 6);
        assert!(val.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_random_hex() {
        let ctx = VariableContext::empty();
        let val = resolve_variable("RANDOM_HEX", &ctx).unwrap();
        assert_eq!(val.len(), 6);
        assert!(val.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_uuid() {
        let ctx = VariableContext::empty();
        let val = resolve_variable("UUID", &ctx).unwrap();
        // UUID v4 format: 8-4-4-4-12
        assert_eq!(val.len(), 36);
        let parts: Vec<&str> = val.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
        // Version nibble should be 4
        assert!(parts[2].starts_with('4'));
        // Variant should be 8, 9, a, or b
        let variant = parts[3].as_bytes()[0];
        assert!(variant == b'8' || variant == b'9' || variant == b'a' || variant == b'b');
    }

    #[test]
    fn test_random_uniqueness() {
        let ctx = VariableContext::empty();
        let v1 = resolve_variable("UUID", &ctx).unwrap();
        let v2 = resolve_variable("UUID", &ctx).unwrap();
        assert_ne!(v1, v2);
    }

    // =========================================================================
    // Unknown variable
    // =========================================================================

    #[test]
    fn test_unknown_variable() {
        let ctx = VariableContext::empty();
        assert!(resolve_variable("NONEXISTENT", &ctx).is_none());
    }

    // =========================================================================
    // Helper functions
    // =========================================================================

    #[test]
    fn test_filename_helper() {
        assert_eq!(filename("/src/main.rs"), "main.rs");
        assert_eq!(filename("main.rs"), "main.rs");
        assert_eq!(filename("/"), "");
    }

    #[test]
    fn test_filename_base_helper() {
        assert_eq!(filename_base("/src/main.rs"), "main");
        assert_eq!(filename_base("file.tar.gz"), "file.tar");
        assert_eq!(filename_base("noext"), "noext");
    }

    #[test]
    fn test_directory_helper() {
        assert_eq!(directory("/src/main.rs"), "/src");
        assert_eq!(directory("main.rs"), "");
        assert_eq!(directory("/root"), "/");
    }

    #[test]
    fn test_civil_from_days_epoch() {
        // Day 0 = 1970-01-01
        let (y, m, d) = civil_from_days(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_civil_from_days_known_date() {
        // 2024-01-01 = day 19723 from epoch
        let (y, m, d) = civil_from_days(19_723);
        assert_eq!((y, m, d), (2024, 1, 1));
    }

    #[test]
    fn test_month_name_all() {
        for m in 1..=12 {
            assert_ne!(month_name(m), "Unknown");
        }
        assert_eq!(month_name(0), "Unknown");
        assert_eq!(month_name(13), "Unknown");
    }

    #[test]
    fn test_month_name_short_all() {
        for m in 1..=12 {
            assert_ne!(month_name_short(m), "Unk");
        }
        assert_eq!(month_name_short(0), "Unk");
    }

    #[test]
    fn test_day_name_all() {
        for d in 0..=6 {
            assert_ne!(day_name(d), "Unknown");
        }
        assert_eq!(day_name(7), "Unknown");
    }

    #[test]
    fn test_day_name_short_all() {
        for d in 0..=6 {
            assert_ne!(day_name_short(d), "Unk");
        }
        assert_eq!(day_name_short(7), "Unk");
    }
}
