use {
    super::*,
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

const TEST_MODULE: ModuleId = ModuleId::new("test");

fn make_info(name: &'static str, cat: Option<&'static str>, layer: BindingLayer) -> BindingInfo {
    BindingInfo::new(CommandId::new(TEST_MODULE, name), "", cat, layer)
}

fn key_g() -> KeySequence {
    KeySequence::from_keys(&[reovim_driver_input::KeyEvent::new(
        reovim_driver_input::KeyCode::Char('g'),
    )])
}

// ========================================================================
// Default / create tests
// ========================================================================

#[test]
fn test_filter_config_create_defaults() {
    let config = WhichKeyFilterConfig::create();
    assert!(config.categories.is_none());
    assert!(config.key_pattern.is_none());
    assert!(config.layer_filter.is_none());
}

#[test]
fn test_filter_config_clear() {
    let mut config = WhichKeyFilterConfig::create();
    config.categories = Some(vec!["motion".to_owned()]);
    config.key_pattern = Some("g".to_owned());
    config.layer_filter = Some(BindingLayerFilter::UserOnly);

    config.clear();
    assert!(config.categories.is_none());
    assert!(config.key_pattern.is_none());
    assert!(config.layer_filter.is_none());
}

// ========================================================================
// No-filter tests
// ========================================================================

#[test]
fn test_matches_no_filter_passes_all() {
    let config = WhichKeyFilterConfig::create();
    let info = make_info("cmd", Some("motion"), BindingLayer::Policy);
    assert!(config.matches(&key_g(), &info));
}

#[test]
fn test_matches_no_filter_passes_no_category() {
    let config = WhichKeyFilterConfig::create();
    let info = make_info("cmd", None, BindingLayer::Policy);
    assert!(config.matches(&key_g(), &info));
}

// ========================================================================
// Category filter tests
// ========================================================================

#[test]
fn test_matches_category_single_match() {
    let mut config = WhichKeyFilterConfig::create();
    config.categories = Some(vec!["motion".to_owned()]);

    let info = make_info("cmd", Some("motion"), BindingLayer::Policy);
    assert!(config.matches(&key_g(), &info));
}

#[test]
fn test_matches_category_single_no_match() {
    let mut config = WhichKeyFilterConfig::create();
    config.categories = Some(vec!["motion".to_owned()]);

    let info = make_info("cmd", Some("operator"), BindingLayer::Policy);
    assert!(!config.matches(&key_g(), &info));
}

#[test]
fn test_matches_category_multiple() {
    let mut config = WhichKeyFilterConfig::create();
    config.categories = Some(vec!["motion".to_owned(), "operator".to_owned()]);

    let motion = make_info("cmd1", Some("motion"), BindingLayer::Policy);
    let operator = make_info("cmd2", Some("operator"), BindingLayer::Policy);
    let edit = make_info("cmd3", Some("edit"), BindingLayer::Policy);

    assert!(config.matches(&key_g(), &motion));
    assert!(config.matches(&key_g(), &operator));
    assert!(!config.matches(&key_g(), &edit));
}

#[test]
fn test_matches_category_filter_rejects_none_category() {
    let mut config = WhichKeyFilterConfig::create();
    config.categories = Some(vec!["motion".to_owned()]);

    let info = make_info("cmd", None, BindingLayer::Policy);
    assert!(!config.matches(&key_g(), &info));
}

// ========================================================================
// Key pattern filter tests
// ========================================================================

#[test]
fn test_matches_key_pattern_match() {
    let mut config = WhichKeyFilterConfig::create();
    config.key_pattern = Some("g".to_owned());

    let info = make_info("cmd", None, BindingLayer::Policy);
    assert!(config.matches(&key_g(), &info));
}

#[test]
fn test_matches_key_pattern_no_match() {
    let mut config = WhichKeyFilterConfig::create();
    config.key_pattern = Some("z".to_owned());

    let info = make_info("cmd", None, BindingLayer::Policy);
    assert!(!config.matches(&key_g(), &info));
}

// ========================================================================
// Layer filter tests
// ========================================================================

#[test]
fn test_matches_layer_user_only() {
    let mut config = WhichKeyFilterConfig::create();
    config.layer_filter = Some(BindingLayerFilter::UserOnly);

    let user = make_info("cmd", None, BindingLayer::User);
    let policy = make_info("cmd", None, BindingLayer::Policy);
    let base = make_info("cmd", None, BindingLayer::Base);

    assert!(config.matches(&key_g(), &user));
    assert!(!config.matches(&key_g(), &policy));
    assert!(!config.matches(&key_g(), &base));
}

#[test]
fn test_matches_layer_defaults_only() {
    let mut config = WhichKeyFilterConfig::create();
    config.layer_filter = Some(BindingLayerFilter::DefaultsOnly);

    let user = make_info("cmd", None, BindingLayer::User);
    let policy = make_info("cmd", None, BindingLayer::Policy);
    let base = make_info("cmd", None, BindingLayer::Base);

    assert!(!config.matches(&key_g(), &user));
    assert!(config.matches(&key_g(), &policy));
    assert!(config.matches(&key_g(), &base));
}

#[test]
fn test_matches_layer_specific() {
    let mut config = WhichKeyFilterConfig::create();
    config.layer_filter = Some(BindingLayerFilter::Specific(BindingLayer::Policy));

    let user = make_info("cmd", None, BindingLayer::User);
    let policy = make_info("cmd", None, BindingLayer::Policy);
    let base = make_info("cmd", None, BindingLayer::Base);

    assert!(!config.matches(&key_g(), &user));
    assert!(config.matches(&key_g(), &policy));
    assert!(!config.matches(&key_g(), &base));
}

// ========================================================================
// Combined filter tests
// ========================================================================

#[test]
fn test_matches_combined_category_and_layer() {
    let mut config = WhichKeyFilterConfig::create();
    config.categories = Some(vec!["motion".to_owned()]);
    config.layer_filter = Some(BindingLayerFilter::DefaultsOnly);

    // motion + Policy → pass
    let a = make_info("cmd1", Some("motion"), BindingLayer::Policy);
    assert!(config.matches(&key_g(), &a));

    // motion + User → fail (layer)
    let b = make_info("cmd2", Some("motion"), BindingLayer::User);
    assert!(!config.matches(&key_g(), &b));

    // operator + Policy → fail (category)
    let c = make_info("cmd3", Some("operator"), BindingLayer::Policy);
    assert!(!config.matches(&key_g(), &c));
}

// ========================================================================
// BindingLayerFilter enum tests
// ========================================================================

#[test]
fn test_binding_layer_filter_eq() {
    assert_eq!(BindingLayerFilter::UserOnly, BindingLayerFilter::UserOnly);
    assert_ne!(BindingLayerFilter::UserOnly, BindingLayerFilter::DefaultsOnly);
    assert_eq!(
        BindingLayerFilter::Specific(BindingLayer::Policy),
        BindingLayerFilter::Specific(BindingLayer::Policy)
    );
    assert_ne!(
        BindingLayerFilter::Specific(BindingLayer::Policy),
        BindingLayerFilter::Specific(BindingLayer::User)
    );
}

#[test]
fn test_binding_layer_filter_clone() {
    let a = BindingLayerFilter::DefaultsOnly;
    let b = a;
    assert_eq!(a, b);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_binding_layer_filter_debug() {
    let debug = format!("{:?}", BindingLayerFilter::UserOnly);
    assert!(debug.contains("UserOnly"));
}
