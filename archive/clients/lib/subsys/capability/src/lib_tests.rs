use super::*;

// =============================================================================
// Capability marker trait
// =============================================================================

struct StubCapability;
impl Capability for StubCapability {}

fn assert_capability<C: Capability>(_: &C) {}

#[test]
fn capability_trait_is_implementable() {
    // Compile-time: StubCapability implements Capability.
    assert_capability(&StubCapability);
}

// =============================================================================
// CapabilityId
// =============================================================================

#[test]
fn capability_id_tuple_field() {
    let id = CapabilityId(42);
    assert_eq!(id.0, 42);
}

#[test]
fn capability_id_equality() {
    assert_eq!(CapabilityId(1), CapabilityId(1));
    assert_ne!(CapabilityId(1), CapabilityId(2));
}

#[test]
fn capability_id_copy() {
    let a = CapabilityId(7);
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn capability_id_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(CapabilityId(1));
    set.insert(CapabilityId(2));
    set.insert(CapabilityId(1));
    assert_eq!(set.len(), 2);
}

#[test]
fn capability_id_in_static_slice() {
    const IDS: &[CapabilityId] = &[CapabilityId(10), CapabilityId(20)];
    assert_eq!(IDS[0].0, 10);
    assert_eq!(IDS[1].0, 20);
}

#[test]
fn capability_id_debug() {
    let id = CapabilityId(99);
    let s = format!("{id:?}");
    assert!(s.contains("99"));
}

// =============================================================================
// FeatureFlag
// =============================================================================

#[test]
fn feature_flag_tuple_field() {
    let f = FeatureFlag(0);
    assert_eq!(f.0, 0);
}

#[test]
fn feature_flag_equality() {
    assert_eq!(FeatureFlag(3), FeatureFlag(3));
    assert_ne!(FeatureFlag(3), FeatureFlag(4));
}

#[test]
fn feature_flag_copy() {
    let a = FeatureFlag(5);
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn feature_flag_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(FeatureFlag(10));
    set.insert(FeatureFlag(20));
    set.insert(FeatureFlag(10));
    assert_eq!(set.len(), 2);
}

#[test]
fn feature_flag_in_static_slice() {
    const FLAGS: &[FeatureFlag] = &[FeatureFlag(1), FeatureFlag(2)];
    assert_eq!(FLAGS[0].0, 1);
    assert_eq!(FLAGS[1].0, 2);
}

#[test]
fn feature_flag_debug() {
    let f = FeatureFlag(77);
    let s = format!("{f:?}");
    assert!(s.contains("77"));
}
