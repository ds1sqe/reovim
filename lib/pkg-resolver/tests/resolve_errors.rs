//! Red-path integration tests: 5 fixtures must produce the expected
//! [`ResolveError`] variant. Every error's [`Display`] string is
//! asserted to contain the offending package name.

use std::path::{Path, PathBuf};

use {
    reovim_pkg_resolver::{ResolveError, resolve},
    semver::Version,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

const fn runtime() -> Version {
    Version::new(0, 15, 0)
}

#[test]
fn fixture_06_cycle_simple_errors() {
    let err = resolve(&fixture("06-cycle-simple"), &runtime()).expect_err("must fail");
    match err {
        ResolveError::Cycle { ref chain } => {
            assert_eq!(chain.first().map(String::as_str), Some("root-06"));
            assert!(chain.iter().any(|s| s == "foo"));
            assert!(chain.iter().any(|s| s == "bar"));
            assert_eq!(chain.last().map(String::as_str), Some("foo"));
        }
        other => panic!("expected Cycle, got {other:?}"),
    }
    assert!(err.to_string().contains("foo"));
}

#[test]
fn fixture_07_cycle_self_errors() {
    let err = resolve(&fixture("07-cycle-self"), &runtime()).expect_err("must fail");
    match err {
        ResolveError::Cycle { ref chain } => {
            assert!(chain.iter().filter(|s| *s == "loop-pkg").count() >= 2);
        }
        other => panic!("expected Cycle, got {other:?}"),
    }
    assert!(err.to_string().contains("loop-pkg"));
}

#[test]
fn fixture_08_conflict_version_errors() {
    let err = resolve(&fixture("08-conflict-version"), &runtime()).expect_err("must fail");
    match err {
        ResolveError::DuplicatePackage { ref pkg, .. } => {
            assert_eq!(pkg, "shared");
        }
        other => panic!("expected DuplicatePackage, got {other:?}"),
    }
    assert!(err.to_string().contains("shared"));
}

#[test]
fn fixture_09_conflict_constraint_errors() {
    let err = resolve(&fixture("09-conflict-constraint"), &runtime()).expect_err("must fail");
    match err {
        ResolveError::VersionMismatch {
            ref pkg,
            ref requirer,
            ref required,
            ref actual,
        } => {
            assert_eq!(pkg, "leaf");
            assert_eq!(requirer, "b");
            assert_eq!(required, "^2");
            assert_eq!(actual, "1.0.0");
        }
        other => panic!("expected VersionMismatch, got {other:?}"),
    }
    assert!(err.to_string().contains("leaf"));
}

#[test]
fn fixture_10_bare_version_errors() {
    let err = resolve(&fixture("10-bare-version"), &runtime()).expect_err("must fail");
    match err {
        ResolveError::UnresolvableDependency {
            ref pkg,
            ref requirer,
            reason,
        } => {
            assert_eq!(pkg, "vim-core");
            assert_eq!(requirer, "root-10");
            assert!(reason.contains("path"));
        }
        other => panic!("expected UnresolvableDependency, got {other:?}"),
    }
    assert!(err.to_string().contains("vim-core"));
}
