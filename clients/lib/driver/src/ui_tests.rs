use super::*;

// =============================================================================
// display_width tests
// =============================================================================

#[test]
fn display_width_ascii() {
    assert_eq!(display_width("Hello"), 5);
}

#[test]
fn display_width_empty() {
    assert_eq!(display_width(""), 0);
}

#[test]
fn display_width_cjk() {
    // Each CJK character is width 2
    assert_eq!(display_width("你好"), 4);
}

#[test]
fn display_width_mixed() {
    // "Hi" (2) + "你" (2) = 4
    assert_eq!(display_width("Hi你"), 4);
}

// =============================================================================
// truncate_end tests
// =============================================================================

#[test]
fn truncate_end_fits() {
    assert_eq!(truncate_end("Hello", 10), "Hello");
}

#[test]
fn truncate_end_exact_fit() {
    assert_eq!(truncate_end("Hello", 5), "Hello");
}

#[test]
fn truncate_end_needs_truncation() {
    assert_eq!(truncate_end("Hello, World!", 8), "Hello...");
}

#[test]
fn truncate_end_very_short_max() {
    assert_eq!(truncate_end("Hello", 3), "...");
    assert_eq!(truncate_end("Hello", 2), "..");
    assert_eq!(truncate_end("Hello", 1), ".");
    assert_eq!(truncate_end("Hello", 0), "");
}

#[test]
fn truncate_end_cjk() {
    // "你好世界" has width 8
    let result = truncate_end("你好世界", 7);
    // target_width = 4, can fit "你好" (width 4)
    assert_eq!(result, "你好...");
}

// =============================================================================
// truncate_start tests
// =============================================================================

#[test]
fn truncate_start_fits() {
    assert_eq!(truncate_start("Hello", 10), "Hello");
}

#[test]
fn truncate_start_needs_truncation() {
    let result = truncate_start("/very/long/path/file.rs", 15);
    // target_width = 12, working backwards
    assert!(result.starts_with("..."));
    assert!(result.len() <= 15);
}

#[test]
fn truncate_start_very_short_max() {
    assert_eq!(truncate_start("Hello", 3), "...");
    assert_eq!(truncate_start("Hello", 2), "..");
    assert_eq!(truncate_start("Hello", 1), ".");
    assert_eq!(truncate_start("Hello", 0), "");
}
