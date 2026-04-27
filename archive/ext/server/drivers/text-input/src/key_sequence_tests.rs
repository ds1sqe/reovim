//! Tests for `KeySequence` and `ToKeyToken`.

use super::{KeySequence, ToKeyToken};

#[test]
fn key_sequence_basics_and_prefixes() {
    let mut seq = KeySequence::new();
    assert!(seq.is_empty());
    assert_eq!(seq.len(), 0);

    seq.push("g");
    seq.push("g");
    assert_eq!(seq.as_slice(), ["g".to_owned(), "g".to_owned()].as_slice());
    assert_eq!(seq.keys(), ["g".to_owned(), "g".to_owned()].as_slice());
    assert_eq!(seq.as_string(), "gg");
    assert_eq!(format!("{seq}"), "gg");

    let prefix = KeySequence::from_keys(&["g"]);
    let other = KeySequence::from_keys(&["d"]);
    assert!(seq.starts_with(&prefix));
    assert!(!seq.starts_with(&other));

    seq.clear();
    assert!(seq.is_empty());
}

#[test]
fn parse_normalizes_plain_special_and_modified_tokens() {
    assert_eq!(KeySequence::parse("gg").unwrap().as_slice(), ["g", "g"]);
    assert_eq!(KeySequence::parse("<Esc>").unwrap().as_slice(), ["<Esc>"]);
    assert_eq!(KeySequence::parse("<Escape>").unwrap().as_slice(), ["<Esc>"]);
    assert_eq!(KeySequence::parse("<CR>").unwrap().as_slice(), ["<Enter>"]);
    assert_eq!(KeySequence::parse("<Space>").unwrap().as_slice(), [" "]);
    assert_eq!(KeySequence::parse("<lt>").unwrap().as_slice(), ["<lt>"]);
    assert_eq!(KeySequence::parse("<gt>").unwrap().as_slice(), ["<gt>"]);
    assert_eq!(KeySequence::parse("<C-w>h").unwrap().as_slice(), ["<C-w>", "h"]);
    assert_eq!(KeySequence::parse("<M-x>").unwrap().as_slice(), ["<A-x>"]);
    assert_eq!(KeySequence::parse("<S-Tab>").unwrap().as_slice(), ["<S-Tab>"]);
    assert_eq!(KeySequence::parse("<C-A-S-x>").unwrap().as_slice(), ["<C-A-S-x>"]);
    assert_eq!(KeySequence::parse("\u{1F389}").unwrap().as_slice(), ["\u{1F389}"]);
    assert_eq!(KeySequence::parse("<F12>").unwrap().as_slice(), ["<F12>"]);
}

#[test]
fn parse_rejects_invalid_notation() {
    assert!(KeySequence::parse("").is_none());
    assert!(KeySequence::parse("<Ctrl").is_none());
    assert!(KeySequence::parse("<Unknown>").is_none());
    assert!(KeySequence::parse("<F13>").is_none());
}

#[test]
fn string_inputs_are_accepted_via_to_key_token() {
    let owned = String::from("<C-w>");
    let borrowed = "h";
    let seq = KeySequence::from_keys(&[owned.as_str(), borrowed]);
    assert_eq!(seq.as_string(), "<C-w>h");
}

#[test]
fn to_key_token_impls() {
    let s = String::from("abc");
    assert_eq!(s.to_key_token(), "abc");
    let b = "xyz";
    assert_eq!(b.to_key_token(), "xyz");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn debug_mentions_type_name() {
    assert!(format!("{:?}", KeySequence::default()).contains("KeySequence"));
}
