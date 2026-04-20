use crate::provider::SnippetRegistry;

use super::*;

// =========================================================================
// extract_word_before
// =========================================================================

#[test]
fn test_extract_word_at_end() {
    assert_eq!(extract_word_before("hello fn", 8), "fn");
}

#[test]
fn test_extract_word_empty() {
    assert_eq!(extract_word_before("hello ", 6), "");
}

#[test]
fn test_extract_word_at_beginning() {
    assert_eq!(extract_word_before("fn", 2), "fn");
}

#[test]
fn test_extract_word_with_dash() {
    assert_eq!(extract_word_before("my-func", 7), "my-func");
}

#[test]
fn test_extract_word_with_underscore() {
    assert_eq!(extract_word_before("my_func", 7), "my_func");
}

#[test]
fn test_extract_word_middle_of_line() {
    assert_eq!(extract_word_before("abc fn xyz", 6), "fn");
}

#[test]
fn test_extract_word_at_col_zero() {
    assert_eq!(extract_word_before("fn", 0), "");
}

#[test]
fn test_extract_word_col_beyond_line() {
    assert_eq!(extract_word_before("fn", 100), "fn");
}

// =========================================================================
// is_word_char
// =========================================================================

#[test]
fn test_is_word_char() {
    assert!(is_word_char(b'a'));
    assert!(is_word_char(b'Z'));
    assert!(is_word_char(b'0'));
    assert!(is_word_char(b'_'));
    assert!(is_word_char(b'-'));
    assert!(!is_word_char(b' '));
    assert!(!is_word_char(b'('));
    assert!(!is_word_char(b'.'));
}

// =========================================================================
// Command trait
// =========================================================================

#[test]
fn test_command_id() {
    let cmd = ExpandSnippet::new(SnippetRegistryHandle::new(SnippetRegistry::new()));
    assert_eq!(cmd.id(), ids::EXPAND);
}

#[test]
fn test_command_description() {
    let cmd = ExpandSnippet::new(SnippetRegistryHandle::new(SnippetRegistry::new()));
    assert!(!cmd.description().is_empty());
}
