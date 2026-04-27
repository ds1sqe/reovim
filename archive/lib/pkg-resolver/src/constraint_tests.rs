//! Unit tests for the [`constraint`] module.

use semver::Version;

use {
    super::{check, parse_constraint, parse_version},
    crate::graph::ResolveError,
};

#[test]
fn parse_version_accepts_semver() {
    let v = parse_version("foo", "1.2.3").expect("valid");
    assert_eq!(v, Version::new(1, 2, 3));
}

#[test]
fn parse_version_rejects_empty() {
    let err = parse_version("foo", "").expect_err("empty must fail");
    match err {
        ResolveError::InvalidVersion { pkg, raw, .. } => {
            assert_eq!(pkg, "foo");
            assert_eq!(raw, "");
        }
        other => panic!("expected InvalidVersion, got {other:?}"),
    }
}

#[test]
fn parse_version_rejects_gibberish() {
    let err = parse_version("foo", "not-a-version").expect_err("gibberish must fail");
    assert!(matches!(err, ResolveError::InvalidVersion { .. }));
}

#[test]
fn parse_constraint_accepts_range() {
    let req = parse_constraint("foo", "root", "^1.0").expect("valid");
    assert!(req.matches(&Version::new(1, 2, 3)));
    assert!(!req.matches(&Version::new(2, 0, 0)));
}

#[test]
fn parse_constraint_rejects_gibberish() {
    let err = parse_constraint("foo", "root", "bogus").expect_err("gibberish must fail");
    match err {
        ResolveError::InvalidConstraint {
            pkg, requirer, raw, ..
        } => {
            assert_eq!(pkg, "foo");
            assert_eq!(requirer, "root");
            assert_eq!(raw, "bogus");
        }
        other => panic!("expected InvalidConstraint, got {other:?}"),
    }
}

#[test]
fn check_accepts_matching_version() {
    let actual = Version::new(1, 2, 3);
    let required = semver::VersionReq::parse("^1.0").unwrap();
    check("foo", "root", &actual, &required).expect("should match");
}

#[test]
fn check_rejects_mismatch_with_context() {
    let actual = Version::new(2, 0, 0);
    let required = semver::VersionReq::parse("^1.0").unwrap();
    let err = check("foo", "root", &actual, &required).expect_err("should fail");
    match err {
        ResolveError::VersionMismatch {
            pkg,
            requirer,
            required,
            actual,
        } => {
            assert_eq!(pkg, "foo");
            assert_eq!(requirer, "root");
            assert_eq!(required, "^1.0");
            assert_eq!(actual, "2.0.0");
        }
        other => panic!("expected VersionMismatch, got {other:?}"),
    }
}
