use super::*;

// -- expand_leader pure function tests --

#[test]
fn expand_leader_no_match_passthrough() {
    assert_eq!(expand_leader("jk", "<Space>"), "jk");
}

#[test]
fn expand_leader_replaces_lowercase() {
    assert_eq!(expand_leader("<leader>ghs", "<Space>"), "<Space>ghs");
}

#[test]
fn expand_leader_replaces_capitalized() {
    assert_eq!(expand_leader("<Leader>ghs", "<Space>"), "<Space>ghs");
}

#[test]
fn expand_leader_replaces_uppercase() {
    assert_eq!(expand_leader("<LEADER>xx", "<Space>"), "<Space>xx");
}

#[test]
fn expand_leader_in_middle() {
    assert_eq!(expand_leader("g<leader>x", "<Space>"), "g<Space>x");
}

#[test]
fn expand_leader_multiple_occurrences() {
    assert_eq!(expand_leader("<leader>a<leader>b", "<Space>"), "<Space>a<Space>b");
}

#[test]
fn expand_leader_empty_notation() {
    assert_eq!(expand_leader("<leader>ghs", ""), "ghs");
}

#[test]
fn expand_leader_no_angle_brackets_fast_path() {
    assert_eq!(expand_leader("dd", "<Space>"), "dd");
}

#[test]
fn expand_leader_preserves_other_specials() {
    assert_eq!(expand_leader("<C-w><leader>h", "<Space>"), "<C-w><Space>h");
}

// -- LeaderKeyProvider service tests --

#[test]
fn get_returns_none_when_not_set() {
    let provider = LeaderKeyProvider::new();
    assert!(provider.get().is_none());
}

#[test]
fn set_and_get_roundtrip() {
    let provider = LeaderKeyProvider::new();
    provider.set("<Space>");
    assert_eq!(provider.get(), Some("<Space>"));
}

#[test]
fn set_returns_previous_value() {
    let provider = LeaderKeyProvider::new();
    let prev = provider.set("<Space>");
    assert!(prev.is_none());

    let prev = provider.set("<C-a>");
    assert_eq!(prev, Some("<Space>"));

    assert_eq!(provider.get(), Some("<C-a>"));
}

#[test]
fn default_creates_empty_provider() {
    let provider = LeaderKeyProvider::default();
    assert!(provider.get().is_none());
}

#[test]
fn expand_with_no_leader_set_passthrough() {
    let provider = LeaderKeyProvider::new();
    assert_eq!(provider.expand("<leader>ghs"), "<leader>ghs");
}

#[test]
fn expand_with_leader_set() {
    let provider = LeaderKeyProvider::new();
    provider.set("<Space>");
    assert_eq!(provider.expand("<leader>ghs"), "<Space>ghs");
}
