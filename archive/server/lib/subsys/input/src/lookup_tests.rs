//! Tests for generic keybinding lookup primitives.

use {
    super::lookup::{EagerLookupPolicy, KeymapQuery, LookupPolicy, LookupResult, LookupState},
    reovim_input_codec::InputSequence,
};

fn make_seq(tag: u8) -> InputSequence {
    let mut seq = InputSequence::new();
    let mut payload = vec![0u8; 8];
    payload[0] = tag;
    seq.push(payload);
    seq
}

// ─── LookupResult helpers ─────────────────────────────────────────────────────

#[test]
fn lookup_result_found_helpers() {
    let r: LookupResult<u32> = LookupResult::Found(42);
    assert!(r.is_found());
    assert!(!r.is_prefix());
    assert!(!r.is_not_found());
    assert_eq!(r.as_found(), Some(&42));
}

#[test]
fn lookup_result_prefix_helpers() {
    let r: LookupResult<u32> = LookupResult::Prefix;
    assert!(!r.is_found());
    assert!(r.is_prefix());
    assert!(!r.is_not_found());
    assert!(r.as_found().is_none());
}

#[test]
fn lookup_result_not_found_helpers() {
    let r: LookupResult<u32> = LookupResult::NotFound;
    assert!(!r.is_found());
    assert!(!r.is_prefix());
    assert!(r.is_not_found());
    assert!(r.as_found().is_none());
}

// ─── EagerLookupPolicy ────────────────────────────────────────────────────────

#[test]
fn eager_policy_exact_only_returns_found() {
    let policy = EagerLookupPolicy;
    let state: LookupState<u32> = LookupState::ExactOnly(7);
    assert_eq!(policy.resolve(state), LookupResult::Found(7));
}

#[test]
fn eager_policy_exact_with_longer_returns_found() {
    let policy = EagerLookupPolicy;
    let state: LookupState<u32> = LookupState::ExactWithLonger { exact: 99 };
    assert_eq!(policy.resolve(state), LookupResult::Found(99));
}

#[test]
fn eager_policy_prefix_only_returns_prefix() {
    let policy = EagerLookupPolicy;
    let state: LookupState<u32> = LookupState::PrefixOnly;
    assert_eq!(policy.resolve(state), LookupResult::Prefix);
}

#[test]
fn eager_policy_not_found_returns_not_found() {
    let policy = EagerLookupPolicy;
    let state: LookupState<u32> = LookupState::NotFound;
    assert_eq!(policy.resolve(state), LookupResult::NotFound);
}

// ─── KeymapQuery default helpers ─────────────────────────────────────────────

struct MockQuery(LookupState<&'static str>);

impl KeymapQuery<&'static str> for MockQuery {
    fn query(&self, _mode: &str, _keys: &InputSequence) -> LookupState<&'static str> {
        self.0.clone()
    }
}

#[test]
fn keymap_query_has_longer_bindings_prefix_only() {
    let q = MockQuery(LookupState::PrefixOnly);
    assert!(q.has_longer_bindings("normal", &make_seq(1)));
}

#[test]
fn keymap_query_has_longer_bindings_exact_with_longer() {
    let q = MockQuery(LookupState::ExactWithLonger { exact: "cmd" });
    assert!(q.has_longer_bindings("normal", &make_seq(1)));
}

#[test]
fn keymap_query_no_longer_bindings_exact_only() {
    let q = MockQuery(LookupState::ExactOnly("cmd"));
    assert!(!q.has_longer_bindings("normal", &make_seq(1)));
}

#[test]
fn keymap_query_get_exact_from_exact_only() {
    let q = MockQuery(LookupState::ExactOnly("delete"));
    assert_eq!(q.get_exact("normal", &make_seq(1)), Some("delete"));
}

#[test]
fn keymap_query_get_exact_from_prefix_only_is_none() {
    let q = MockQuery(LookupState::PrefixOnly);
    assert!(q.get_exact("normal", &make_seq(1)).is_none());
}

#[test]
fn keymap_query_get_exact_from_not_found_is_none() {
    let q = MockQuery(LookupState::NotFound);
    assert!(q.get_exact("normal", &make_seq(1)).is_none());
}
