use reovim_driver_syntax::BracketPair;

use super::*;

fn default_pairs() -> Vec<BracketPair> {
    vec![
        BracketPair::new('(', ')'),
        BracketPair::new('[', ']'),
        BracketPair::new('{', '}'),
    ]
}

#[test]
fn empty_content() {
    let result = compute_bracket_depths("", &default_pairs());
    assert!(result.is_empty());
}

#[test]
fn no_brackets() {
    let result = compute_bracket_depths("hello world", &default_pairs());
    assert!(result.is_empty());
}

#[test]
fn empty_pairs() {
    let result = compute_bracket_depths("(hello)", &[]);
    assert!(result.is_empty());
}

#[test]
fn simple_parens() {
    let result = compute_bracket_depths("(a)", &default_pairs());
    assert_eq!(result.len(), 2);

    let open = result.get(&(0, 0)).unwrap();
    assert_eq!(open.depth, 0);
    assert_eq!(open.ch, '(');

    let close = result.get(&(0, 2)).unwrap();
    assert_eq!(close.depth, 0);
    assert_eq!(close.ch, ')');
}

#[test]
fn nested_brackets() {
    let result = compute_bracket_depths("(a[b]c)", &default_pairs());
    assert_eq!(result.len(), 4);

    // Outer parens: depth 0
    assert_eq!(result.get(&(0, 0)).unwrap().depth, 0);
    assert_eq!(result.get(&(0, 6)).unwrap().depth, 0);

    // Inner brackets: depth 0 (independent stacks per pair type)
    assert_eq!(result.get(&(0, 2)).unwrap().depth, 0);
    assert_eq!(result.get(&(0, 4)).unwrap().depth, 0);
}

#[test]
fn same_type_nested() {
    let result = compute_bracket_depths("((a))", &default_pairs());
    assert_eq!(result.len(), 4);

    // Outer: depth 0
    assert_eq!(result.get(&(0, 0)).unwrap().depth, 0);
    assert_eq!(result.get(&(0, 4)).unwrap().depth, 0);

    // Inner: depth 1
    assert_eq!(result.get(&(0, 1)).unwrap().depth, 1);
    assert_eq!(result.get(&(0, 3)).unwrap().depth, 1);
}

#[test]
fn multiline() {
    let content = "fn main() {\n  println!()\n}";
    let result = compute_bracket_depths(content, &default_pairs());

    // ( at line 0, col 7
    assert_eq!(result.get(&(0, 7)).unwrap().depth, 0);
    // ) at line 0, col 8
    assert_eq!(result.get(&(0, 8)).unwrap().depth, 0);
    // { at line 0, col 10
    assert_eq!(result.get(&(0, 10)).unwrap().depth, 0);
    // ( at line 1, col 10
    assert_eq!(result.get(&(1, 10)).unwrap().depth, 0);
    // ) at line 1, col 11
    assert_eq!(result.get(&(1, 11)).unwrap().depth, 0);
    // } at line 2, col 0
    assert_eq!(result.get(&(2, 0)).unwrap().depth, 0);
}

#[test]
fn unmatched_open() {
    let result = compute_bracket_depths("(a", &default_pairs());
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(&(0, 0)).unwrap().depth, usize::MAX);
}

#[test]
fn unmatched_close() {
    let result = compute_bracket_depths("a)", &default_pairs());
    assert_eq!(result.len(), 1);
    assert_eq!(result.get(&(0, 1)).unwrap().depth, usize::MAX);
}

#[test]
fn deeply_nested() {
    let result = compute_bracket_depths("(((())))", &default_pairs());
    assert_eq!(result.len(), 8);

    // Depth 0, 1, 2, 3 for openers
    assert_eq!(result.get(&(0, 0)).unwrap().depth, 0);
    assert_eq!(result.get(&(0, 1)).unwrap().depth, 1);
    assert_eq!(result.get(&(0, 2)).unwrap().depth, 2);
    assert_eq!(result.get(&(0, 3)).unwrap().depth, 3);
    // Matching closers have same depths
    assert_eq!(result.get(&(0, 4)).unwrap().depth, 3);
    assert_eq!(result.get(&(0, 5)).unwrap().depth, 2);
    assert_eq!(result.get(&(0, 6)).unwrap().depth, 1);
    assert_eq!(result.get(&(0, 7)).unwrap().depth, 0);
}

#[test]
fn bracket_info_debug_clone_eq() {
    let info = BracketInfo {
        line: 0,
        col: 0,
        depth: 0,
        ch: '(',
    };
    let cloned = info;
    assert_eq!(info, cloned);
    let debug = format!("{info:?}");
    assert!(debug.contains("BracketInfo"));
}
