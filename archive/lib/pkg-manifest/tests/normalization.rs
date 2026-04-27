//! [`Dependency::into_detailed`] normalization test.
//!
//! Required by the plan (Phase 0.B step 7): the bare-version sugar
//! must lift into a [`DetailedDep`] with `path = None` and
//! `features = []`; the detailed variant must pass through unchanged.

use std::path::PathBuf;

use reovim_pkg_manifest::{Dependency, DetailedDep};

#[test]
fn version_variant_lifts_to_detailed_with_defaults() {
    let dep = Dependency::Version("1.2.3".to_string());
    let detailed = dep.into_detailed();
    assert_eq!(detailed.version.as_deref(), Some("1.2.3"));
    assert!(detailed.path.is_none());
    assert!(detailed.features.is_empty());
}

#[test]
fn detailed_variant_passes_through_unchanged() {
    let original = DetailedDep {
        version: Some("2.0".to_string()),
        path: Some(PathBuf::from("/local/x")),
        features: vec!["a".to_string(), "b".to_string()],
    };
    let dep = Dependency::Detailed(original.clone());
    let normalized = dep.into_detailed();
    assert_eq!(normalized, original);
}
