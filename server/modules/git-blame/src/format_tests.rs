use reovim_subsys_git::types::BlameEntry;

use super::*;

fn entry() -> BlameEntry {
    BlameEntry {
        line: 1,
        short_hash: "abc1234".to_owned(),
        author: "Alice".to_owned(),
        date: "2025-01-15".to_owned(),
        summary: "feat: add feature".to_owned(),
    }
}

#[test]
fn test_format_full() {
    let result = format_blame(&entry(), None);
    assert_eq!(result, "abc1234 Alice feat: add feature");
}

#[test]
fn test_format_truncated() {
    let result = format_blame(&entry(), Some(20));
    assert_eq!(result.len(), 20);
    assert!(result.ends_with("..."));
}

#[test]
fn test_format_no_truncation_when_fits() {
    let result = format_blame(&entry(), Some(100));
    assert_eq!(result, "abc1234 Alice feat: add feature");
}

#[test]
fn test_format_tiny_max() {
    // max_width <= 3 doesn't truncate (can't fit "...")
    let result = format_blame(&entry(), Some(3));
    assert_eq!(result, "abc1234 Alice feat: add feature");
}
