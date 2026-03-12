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

#[test]
fn test_current_datetime_unknown_format() {
    // Unknown format specifier returns empty string
    let result = current_datetime("%Q");
    assert!(result.is_empty());
}

#[test]
fn test_civil_from_days_negative() {
    // Negative days (before epoch) exercises the else branch in era calculation
    let (y, m, d) = civil_from_days(-1);
    assert_eq!((y, m, d), (1969, 12, 31));
}

#[test]
fn test_civil_from_days_far_negative() {
    // Very negative days (year well before epoch)
    let (y, _, _) = civil_from_days(-365 * 2000);
    assert!(y < 0);
}
