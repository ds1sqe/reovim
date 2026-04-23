use {
    super::*,
    reovim_client_subsys_capability::{CapabilityId, FeatureFlag},
};

// Smoke tests for lib.rs re-exports.
// The full Requirements and Platform test suites live in requirements_tests.rs.

#[test]
fn lib_re_exports_requirements() {
    let r: Requirements = Requirements::none();
    assert!(r.is_empty());
}

#[test]
fn lib_re_exports_requirements_new() {
    static CAPS: &[CapabilityId] = &[CapabilityId(1)];
    static FEATS: &[FeatureFlag] = &[FeatureFlag(2)];
    let r: Requirements = Requirements::new(CAPS, FEATS);
    assert_eq!(r.caps.len(), 1);
    assert_eq!(r.features.len(), 1);
}

#[test]
fn lib_re_exports_platform_trait() {
    #[derive(Debug)]
    struct MockPlatform;
    impl Platform for MockPlatform {
        fn name(&self) -> &'static str {
            "mock"
        }
    }
    let p: Box<dyn Platform> = Box::new(MockPlatform);
    assert_eq!(p.name(), "mock");
}
