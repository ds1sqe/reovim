use reovim_driver_display::render_backend::TuiExtension;

use super::*;

#[test]
fn kind_returns_pair() {
    let ext = PairExtension::new();
    assert_eq!(ext.kind(), "pair");
}

#[test]
fn not_active_initially() {
    let ext = PairExtension::new();
    assert!(!ext.is_active());
}

#[test]
fn default_impl() {
    let ext = PairExtension::default();
    assert!(!ext.is_active());
}

#[test]
fn apply_notification_activates() {
    let mut ext = PairExtension::new();
    let json = r#"{"active":true,"rainbow":true,"matchpair":true,"brackets":[],"matched":null}"#;
    ext.apply_notification(json);
    assert!(ext.is_active());
    assert!(ext.rainbow_enabled);
    assert!(ext.matchpair_enabled);
    assert!(ext.brackets.is_empty());
    assert!(ext.matched.is_none());
}

#[test]
fn apply_notification_with_brackets() {
    let mut ext = PairExtension::new();
    let json = r#"{
        "active": true,
        "rainbow": true,
        "matchpair": true,
        "brackets": [
            {"line": 0, "col": 5, "depth": 0, "char": "(", "unmatched": false},
            {"line": 0, "col": 10, "depth": 0, "char": ")", "unmatched": false}
        ],
        "matched": {"open": {"line": 0, "col": 5}, "close": {"line": 0, "col": 10}}
    }"#;
    ext.apply_notification(json);

    assert_eq!(ext.brackets.len(), 2);
    assert!(ext.matched.is_some());

    let m = ext.matched.as_ref().unwrap();
    assert_eq!(m.open.line, 0);
    assert_eq!(m.open.col, 5);
    assert_eq!(m.close.line, 0);
    assert_eq!(m.close.col, 10);
}

#[test]
fn apply_notification_disabled_options() {
    let mut ext = PairExtension::new();
    let json = r#"{"active":true,"rainbow":false,"matchpair":false,"brackets":[],"matched":null}"#;
    ext.apply_notification(json);
    assert!(!ext.rainbow_enabled);
    assert!(!ext.matchpair_enabled);
}

#[test]
fn apply_notification_invalid_json() {
    let mut ext = PairExtension::new();
    ext.apply_notification("not json");
    assert!(!ext.is_active());
}

#[test]
fn apply_notification_unmatched_bracket() {
    let mut ext = PairExtension::new();
    let json = r#"{
        "active": true,
        "rainbow": true,
        "matchpair": true,
        "brackets": [
            {"line": 0, "col": 0, "depth": 18446744073709551615, "char": "(", "unmatched": true}
        ],
        "matched": null
    }"#;
    ext.apply_notification(json);
    assert_eq!(ext.brackets.len(), 1);
    assert!(ext.brackets[0].unmatched);
}

#[test]
fn bracket_entry_debug() {
    let entry = BracketEntry {
        line: 0,
        col: 5,
        depth: 0,
        char: "(".to_string(),
        unmatched: false,
    };
    let debug = format!("{entry:?}");
    assert!(debug.contains("BracketEntry"));
}

#[test]
fn matched_entry_debug() {
    let entry = MatchedEntry {
        open: PosEntry { line: 0, col: 0 },
        close: PosEntry { line: 0, col: 5 },
    };
    let debug = format!("{entry:?}");
    assert!(debug.contains("MatchedEntry"));
}

#[test]
fn pos_entry_debug() {
    let entry = PosEntry { line: 0, col: 5 };
    let debug = format!("{entry:?}");
    assert!(debug.contains("PosEntry"));
}
