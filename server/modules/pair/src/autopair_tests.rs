use reovim_driver_syntax::{BracketConfig, SyntaxContext};

use super::*;

fn rust_config() -> BracketConfig {
    BracketConfig::new("rust").with_autopair([('(', ')'), ('[', ']'), ('{', '}'), ('"', '"')])
}

fn markdown_config() -> BracketConfig {
    BracketConfig::new("markdown").with_autopair([('(', ')'), ('[', ']'), ('"', '"'), ('`', '`')])
}

#[test]
fn autopair_paren() {
    assert_eq!(should_autopair('(', &rust_config(), SyntaxContext::Code), Some(')'));
}

#[test]
fn autopair_bracket() {
    assert_eq!(should_autopair('[', &rust_config(), SyntaxContext::Code), Some(']'));
}

#[test]
fn autopair_brace() {
    assert_eq!(should_autopair('{', &rust_config(), SyntaxContext::Code), Some('}'));
}

#[test]
fn autopair_quote() {
    assert_eq!(should_autopair('"', &rust_config(), SyntaxContext::Code), Some('"'));
}

#[test]
fn no_autopair_in_string() {
    assert_eq!(should_autopair('(', &rust_config(), SyntaxContext::String), None);
}

#[test]
fn no_autopair_in_comment() {
    assert_eq!(should_autopair('(', &rust_config(), SyntaxContext::Comment), None);
}

#[test]
fn no_autopair_for_unknown_char() {
    assert_eq!(should_autopair('<', &rust_config(), SyntaxContext::Code), None);
}

#[test]
fn rust_no_single_quote() {
    // Single quote not in Rust autopair config (lifetime conflict)
    assert_eq!(should_autopair('\'', &rust_config(), SyntaxContext::Code), None);
}

#[test]
fn markdown_backtick() {
    assert_eq!(should_autopair('`', &markdown_config(), SyntaxContext::Code), Some('`'));
}

#[test]
fn markdown_no_brace() {
    assert_eq!(should_autopair('{', &markdown_config(), SyntaxContext::Code), None);
}
