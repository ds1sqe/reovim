use std::hash::{Hash, Hasher};

use super::*;

// ============================================================================
// BracketPair tests
// ============================================================================

#[test]
fn bracket_pair_new() {
    let pair = BracketPair::new('(', ')');
    assert_eq!(pair.open, '(');
    assert_eq!(pair.close, ')');
}

#[test]
fn bracket_pair_symmetric() {
    assert!(BracketPair::new('"', '"').is_symmetric());
    assert!(BracketPair::new('\'', '\'').is_symmetric());
    assert!(!BracketPair::new('(', ')').is_symmetric());
    assert!(!BracketPair::new('[', ']').is_symmetric());
    assert!(!BracketPair::new('{', '}').is_symmetric());
}

#[test]
fn bracket_pair_debug_clone_eq_hash() {
    let pair = BracketPair::new('(', ')');
    let cloned = pair;
    assert_eq!(pair, cloned);
    assert_eq!(format!("{pair:?}"), "BracketPair { open: '(', close: ')' }");

    // Hash consistency
    let mut h1 = std::collections::hash_map::DefaultHasher::new();
    let mut h2 = std::collections::hash_map::DefaultHasher::new();
    pair.hash(&mut h1);
    cloned.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
}

// ============================================================================
// BracketConfig tests
// ============================================================================

#[test]
fn bracket_config_new_empty() {
    let config = BracketConfig::new("test");
    assert_eq!(config.language_id(), "test");
    assert!(config.rainbow_pairs().is_empty());
    assert!(config.autopair_pairs().is_empty());
    assert!(config.highlight_pairs().is_empty());
}

#[test]
fn bracket_config_builder() {
    let config = BracketConfig::new("rust")
        .with_rainbow([('(', ')'), ('[', ']'), ('{', '}')])
        .with_autopair([('(', ')'), ('"', '"')])
        .with_highlight([('(', ')')]);

    assert_eq!(config.language_id(), "rust");
    assert_eq!(config.rainbow_pairs().len(), 3);
    assert_eq!(config.autopair_pairs().len(), 2);
    assert_eq!(config.highlight_pairs().len(), 1);
}

#[test]
fn bracket_config_rainbow_pairs_content() {
    let config = BracketConfig::new("rust").with_rainbow([('(', ')'), ('[', ']')]);

    assert_eq!(config.rainbow_pairs()[0], BracketPair::new('(', ')'));
    assert_eq!(config.rainbow_pairs()[1], BracketPair::new('[', ']'));
}

#[test]
fn bracket_config_clone_debug() {
    let config = BracketConfig::new("rust").with_rainbow([('(', ')')]);
    let cloned = config.clone();
    assert_eq!(cloned.language_id(), "rust");
    assert_eq!(cloned.rainbow_pairs().len(), 1);

    // Debug
    let debug = format!("{config:?}");
    assert!(debug.contains("BracketConfig"));
    assert!(debug.contains("rust"));
}

// ============================================================================
// default_bracket_config tests
// ============================================================================

#[test]
fn default_config_has_expected_pairs() {
    let config = default_bracket_config();
    assert_eq!(config.language_id(), "*");
    assert_eq!(config.rainbow_pairs().len(), 3);
    assert_eq!(config.autopair_pairs().len(), 5);
    assert_eq!(config.highlight_pairs().len(), 3);

    // Rainbow: ()[]{}
    assert!(config.rainbow_pairs().contains(&BracketPair::new('(', ')')));
    assert!(config.rainbow_pairs().contains(&BracketPair::new('[', ']')));
    assert!(config.rainbow_pairs().contains(&BracketPair::new('{', '}')));

    // Autopair includes quotes
    assert!(
        config
            .autopair_pairs()
            .contains(&BracketPair::new('"', '"'))
    );
    assert!(
        config
            .autopair_pairs()
            .contains(&BracketPair::new('\'', '\''))
    );
}

// ============================================================================
// BracketConfigStore tests
// ============================================================================

#[test]
fn store_new_is_empty() {
    let store = BracketConfigStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn store_default_is_empty() {
    let store = BracketConfigStore::default();
    assert!(store.is_empty());
}

#[test]
fn store_add_and_find() {
    let store = BracketConfigStore::new();
    store.add(BracketConfig::new("rust").with_rainbow([('(', ')'), ('[', ']'), ('{', '}')]));

    assert_eq!(store.len(), 1);
    assert!(!store.is_empty());

    let found = store.find("rust");
    assert!(found.is_some());
    assert_eq!(found.as_ref().unwrap().language_id(), "rust");
    assert_eq!(found.unwrap().rainbow_pairs().len(), 3);
}

#[test]
fn store_find_not_found() {
    let store = BracketConfigStore::new();
    assert!(store.find("nonexistent").is_none());
}

#[test]
fn store_find_or_default_found() {
    let store = BracketConfigStore::new();
    store.add(BracketConfig::new("rust").with_rainbow([('(', ')')]));

    let config = store.find_or_default("rust");
    assert_eq!(config.language_id(), "rust");
    assert_eq!(config.rainbow_pairs().len(), 1);
}

#[test]
fn store_find_or_default_not_found() {
    let store = BracketConfigStore::new();
    let config = store.find_or_default("unknown");
    assert_eq!(config.language_id(), "*");
    assert_eq!(config.rainbow_pairs().len(), 3);
}

#[test]
fn store_take_all_drains() {
    let store = BracketConfigStore::new();
    store.add(BracketConfig::new("rust"));
    store.add(BracketConfig::new("markdown"));

    assert_eq!(store.len(), 2);
    let taken = store.take_all();
    assert_eq!(taken.len(), 2);
    assert!(store.is_empty());
}

#[test]
fn store_multiple_languages() {
    let store = BracketConfigStore::new();
    store.add(BracketConfig::new("rust").with_rainbow([('(', ')'), ('[', ']'), ('{', '}')]));
    store.add(BracketConfig::new("markdown").with_rainbow([('(', ')'), ('[', ']')]));

    assert_eq!(store.len(), 2);
    let rust = store.find("rust").unwrap();
    assert_eq!(rust.rainbow_pairs().len(), 3);

    let md = store.find("markdown").unwrap();
    assert_eq!(md.rainbow_pairs().len(), 2);
}

#[test]
fn store_implements_service() {
    fn assert_service<T: Service>() {}
    assert_service::<BracketConfigStore>();
}

#[test]
fn store_debug() {
    let store = BracketConfigStore::new();
    store.add(BracketConfig::new("rust"));
    let debug = format!("{store:?}");
    assert!(debug.contains("BracketConfigStore"));
    assert!(debug.contains("count"));
}
