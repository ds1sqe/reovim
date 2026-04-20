use reovim_driver_text_session::{
    ExtensionMap,
    bridges::{ExtensionScope, ExtensionStateBridge},
};

use {
    super::*,
    crate::{
        matched::MatchedPair,
        rainbow::BracketInfo,
        state::{PairOptions, PairState},
    },
};

#[test]
fn bridge_kind() {
    let bridge = PairBridge;
    assert_eq!(bridge.kind(), "pair");
}

#[test]
fn bridge_scope_is_client() {
    let bridge = PairBridge;
    assert_eq!(bridge.scope(), ExtensionScope::Client);
}

#[test]
fn bridge_not_active_when_no_state() {
    let bridge = PairBridge;
    let extensions = ExtensionMap::default();
    assert!(!bridge.is_active(&extensions));
}

#[test]
fn bridge_snapshot_none_when_no_state() {
    let bridge = PairBridge;
    let extensions = ExtensionMap::default();
    assert!(bridge.snapshot(&extensions).is_none());
}

#[test]
fn bridge_active_with_state() {
    let bridge = PairBridge;
    let mut extensions = ExtensionMap::default();
    extensions.get_or_insert::<PairState>();
    assert!(bridge.is_active(&extensions));
}

#[test]
fn bridge_snapshot_empty_brackets() {
    let bridge = PairBridge;
    let mut extensions = ExtensionMap::default();
    extensions.get_or_insert::<PairState>();

    let snapshot = bridge.snapshot(&extensions).unwrap();
    assert_eq!(snapshot["active"], true);
    assert_eq!(snapshot["rainbow"], true);
    assert_eq!(snapshot["matchpair"], true);
    assert!(snapshot["brackets"].as_array().unwrap().is_empty());
    assert!(snapshot["matched"].is_null());
}

#[test]
fn bridge_snapshot_with_brackets() {
    let bridge = PairBridge;
    let mut extensions = ExtensionMap::default();

    let state = extensions.get_or_insert::<PairState>();
    let info = BracketInfo {
        line: 0,
        col: 5,
        depth: 0,
        ch: '(',
    };
    state.brackets.insert((0, 5), info);

    let snapshot = bridge.snapshot(&extensions).unwrap();
    let brackets = snapshot["brackets"].as_array().unwrap();
    assert_eq!(brackets.len(), 1);
    assert_eq!(brackets[0]["line"], 0);
    assert_eq!(brackets[0]["col"], 5);
    assert_eq!(brackets[0]["depth"], 0);
    assert_eq!(brackets[0]["char"], "(");
    assert_eq!(brackets[0]["unmatched"], false);
}

#[test]
fn bridge_snapshot_with_matched_pair() {
    let bridge = PairBridge;
    let mut extensions = ExtensionMap::default();

    let state = extensions.get_or_insert::<PairState>();
    state.matched = Some(MatchedPair {
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
    });

    let snapshot = bridge.snapshot(&extensions).unwrap();
    let matched = &snapshot["matched"];
    assert_eq!(matched["open"]["line"], 0);
    assert_eq!(matched["open"]["col"], 0);
    assert_eq!(matched["close"]["line"], 0);
    assert_eq!(matched["close"]["col"], 5);
}

#[test]
fn bridge_snapshot_unmatched_bracket() {
    let bridge = PairBridge;
    let mut extensions = ExtensionMap::default();

    let state = extensions.get_or_insert::<PairState>();
    state.brackets.insert(
        (0, 0),
        BracketInfo {
            line: 0,
            col: 0,
            depth: usize::MAX,
            ch: '(',
        },
    );

    let snapshot = bridge.snapshot(&extensions).unwrap();
    let brackets = snapshot["brackets"].as_array().unwrap();
    assert_eq!(brackets[0]["unmatched"], true);
}

#[test]
fn bridge_snapshot_disabled_options() {
    let bridge = PairBridge;
    let mut extensions = ExtensionMap::default();

    let state = extensions.get_or_insert::<PairState>();
    state.options = PairOptions {
        rainbow: false,
        autopair: false,
        matchpair: false,
    };

    let snapshot = bridge.snapshot(&extensions).unwrap();
    assert_eq!(snapshot["rainbow"], false);
    assert_eq!(snapshot["matchpair"], false);
}
