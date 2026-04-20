use reovim_driver_text_syntax::BracketPair;

use {super::*, crate::rainbow::compute_bracket_depths};

fn default_pairs() -> Vec<BracketPair> {
    vec![
        BracketPair::new('(', ')'),
        BracketPair::new('[', ']'),
        BracketPair::new('{', '}'),
    ]
}

#[test]
fn no_brackets() {
    let brackets = compute_bracket_depths("hello", &default_pairs());
    let result = find_innermost_pair(&brackets, &default_pairs(), 0, 2);
    assert!(result.is_none());
}

#[test]
fn cursor_inside_simple_pair() {
    let brackets = compute_bracket_depths("(hello)", &default_pairs());
    let result = find_innermost_pair(&brackets, &default_pairs(), 0, 3);
    assert!(result.is_some());

    let pair = result.unwrap();
    assert_eq!(pair.open.col, 0);
    assert_eq!(pair.close.col, 6);
}

#[test]
fn cursor_on_open_bracket() {
    let brackets = compute_bracket_depths("(hello)", &default_pairs());
    let result = find_innermost_pair(&brackets, &default_pairs(), 0, 0);
    assert!(result.is_some());
    assert_eq!(result.unwrap().open.col, 0);
}

#[test]
fn cursor_on_close_bracket() {
    let brackets = compute_bracket_depths("(hello)", &default_pairs());
    let result = find_innermost_pair(&brackets, &default_pairs(), 0, 6);
    assert!(result.is_some());
    assert_eq!(result.unwrap().close.col, 6);
}

#[test]
fn cursor_outside_pair() {
    let brackets = compute_bracket_depths("x(hello)y", &default_pairs());
    let result = find_innermost_pair(&brackets, &default_pairs(), 0, 8);
    assert!(result.is_none());
}

#[test]
fn nested_returns_innermost() {
    let brackets = compute_bracket_depths("((inner))", &default_pairs());
    let result = find_innermost_pair(&brackets, &default_pairs(), 0, 4);
    assert!(result.is_some());

    let pair = result.unwrap();
    // Inner pair: positions 1 and 7
    assert_eq!(pair.open.col, 1);
    assert_eq!(pair.close.col, 7);
}

#[test]
fn matched_pair_debug_clone_eq() {
    let pair = MatchedPair {
        open: BracketInfo {
            line: 0,
            col: 0,
            depth: 0,
            ch: '(',
        },
        close: BracketInfo {
            line: 0,
            col: 5,
            depth: 0,
            ch: ')',
        },
    };
    let cloned = pair;
    assert_eq!(pair, cloned);
    let debug = format!("{pair:?}");
    assert!(debug.contains("MatchedPair"));
}

#[test]
fn symmetric_pairs_are_skipped() {
    // Symmetric pairs like quotes should be skipped
    let mut pairs = default_pairs();
    pairs.push(BracketPair::new('"', '"'));
    let brackets = compute_bracket_depths("\"hello\"", &pairs);
    // Only asymmetric pairs should be found
    let result = find_innermost_pair(&brackets, &pairs, 0, 3);
    // No asymmetric pair surrounds cursor, so should be None
    assert!(result.is_none());
}

#[test]
fn unmatched_opener_returns_none() {
    // Single opener with no closer
    let brackets = compute_bracket_depths("(hello", &default_pairs());
    let result = find_innermost_pair(&brackets, &default_pairs(), 0, 3);
    assert!(result.is_none());
}

#[test]
fn cursor_before_open_bracket() {
    // Cursor at col 0, before the opening bracket at col 2
    let brackets = compute_bracket_depths("xx(hello)xx", &default_pairs());
    let result = find_innermost_pair(&brackets, &default_pairs(), 0, 0);
    assert!(result.is_none());
}

#[test]
fn cursor_after_close_bracket() {
    // Cursor at col 10, after the closing bracket at col 8
    let brackets = compute_bracket_depths("xx(hello)xx", &default_pairs());
    let result = find_innermost_pair(&brackets, &default_pairs(), 0, 10);
    assert!(result.is_none());
}
