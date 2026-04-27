use {
    super::*,
    reovim_client_subsys_capability::{CapabilityId, FeatureFlag},
};

// =============================================================================
// Requirements::none
// =============================================================================

#[test]
fn requirements_none_has_empty_caps() {
    let r = Requirements::none();
    assert!(r.caps.is_empty());
}

#[test]
fn requirements_none_has_empty_features() {
    let r = Requirements::none();
    assert!(r.features.is_empty());
}

#[test]
fn requirements_none_is_empty() {
    let r = Requirements::none();
    assert!(r.is_empty());
}

// =============================================================================
// Requirements::new
// =============================================================================

static CAPS: &[CapabilityId] = &[CapabilityId(1), CapabilityId(2)];
static FEATURES: &[FeatureFlag] = &[FeatureFlag(10)];

#[test]
fn requirements_new_stores_caps() {
    let r = Requirements::new(CAPS, &[]);
    assert_eq!(r.caps.len(), 2);
    assert_eq!(r.caps[0], CapabilityId(1));
    assert_eq!(r.caps[1], CapabilityId(2));
}

#[test]
fn requirements_new_stores_features() {
    let r = Requirements::new(&[], FEATURES);
    assert_eq!(r.features.len(), 1);
    assert_eq!(r.features[0], FeatureFlag(10));
}

#[test]
fn requirements_new_not_empty() {
    let r = Requirements::new(CAPS, FEATURES);
    assert!(!r.is_empty());
}

#[test]
fn requirements_new_only_caps_not_empty() {
    let r = Requirements::new(CAPS, &[]);
    assert!(!r.is_empty());
}

#[test]
fn requirements_new_only_features_not_empty() {
    let r = Requirements::new(&[], FEATURES);
    assert!(!r.is_empty());
}

// =============================================================================
// Requirements const evaluation
// =============================================================================

const CONST_REQ: Requirements = Requirements::new(&[CapabilityId(42)], &[]);

#[test]
fn requirements_usable_as_const() {
    assert_eq!(CONST_REQ.caps[0], CapabilityId(42));
}

const NONE_REQ: Requirements = Requirements::none();

#[test]
fn requirements_none_usable_as_const() {
    assert!(NONE_REQ.is_empty());
}

// =============================================================================
// Requirements PartialEq / Copy
// =============================================================================

#[test]
fn requirements_equality() {
    let a = Requirements::new(CAPS, &[]);
    let b = Requirements::new(CAPS, &[]);
    assert_eq!(a, b);
}

#[test]
fn requirements_inequality_caps() {
    let a = Requirements::new(CAPS, &[]);
    let b = Requirements::none();
    assert_ne!(a, b);
}

#[test]
fn requirements_copy() {
    let a = Requirements::new(CAPS, FEATURES);
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn requirements_debug() {
    let r = Requirements::new(CAPS, FEATURES);
    let s = format!("{r:?}");
    assert!(s.contains("Requirements"));
}

// =============================================================================
// Platform trait — compile-time object safety
// =============================================================================

#[derive(Debug)]
struct StubPlatform;

impl Platform for StubPlatform {
    fn name(&self) -> &'static str {
        "stub"
    }
}

#[test]
fn platform_name() {
    let p = StubPlatform;
    assert_eq!(p.name(), "stub");
}

#[test]
fn platform_object_safety() {
    let p: Box<dyn Platform> = Box::new(StubPlatform);
    assert_eq!(p.name(), "stub");
}

#[test]
fn platform_debug() {
    let p = StubPlatform;
    let s = format!("{p:?}");
    assert!(s.contains("StubPlatform"));
}
