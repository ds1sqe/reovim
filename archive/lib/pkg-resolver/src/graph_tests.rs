//! Unit tests for the [`graph`] module.
//!
//! These tests build tiny on-disk manifests in `tempfile::tempdir()`
//! to exercise the DFS walker's contract at unit granularity. The
//! larger end-to-end fixtures live under `tests/fixtures/` and drive
//! [`resolve`] through [`tests/resolve_ok.rs`] and
//! [`tests/resolve_errors.rs`].

use std::{fs, path::Path};

use {semver::Version, tempfile::tempdir};

use super::{ResolveError, resolve};

fn write_manifest(dir: &Path, body: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("pkg.toml"), body).unwrap();
}

fn runtime() -> Version {
    Version::new(0, 15, 0)
}

#[test]
fn resolve_empty_root_has_no_packages() {
    let root = tempdir().unwrap();
    write_manifest(
        root.path(),
        r#"
[package]
name = "root-setup"
reovim-version = "^0.15"
"#,
    );
    let resolved = resolve(root.path(), &runtime()).expect("ok");
    assert!(resolved.packages.is_empty());
}

#[test]
fn resolve_records_single_path_dep() {
    let root = tempdir().unwrap();
    write_manifest(
        &root.path().join("lib-foo"),
        r#"
[package]
name = "foo"
version = "1.0.0"
reovim-version = "^0.15"
"#,
    );
    write_manifest(
        root.path(),
        r#"
[package]
name = "root-setup"
reovim-version = "^0.15"

[dependencies]
foo = { path = "lib-foo" }
"#,
    );

    let resolved = resolve(root.path(), &runtime()).expect("ok");
    assert_eq!(resolved.packages.len(), 1);
    assert_eq!(resolved.packages[0].name, "foo");
    assert_eq!(resolved.packages[0].version, Version::new(1, 0, 0));
    assert!(resolved.packages[0].dependencies.is_empty());
}

#[test]
fn resolve_detects_simple_cycle_and_preserves_dfs_order() {
    // root → foo → bar → foo  (cycle on foo)
    let root = tempdir().unwrap();
    write_manifest(
        &root.path().join("foo"),
        r#"
[package]
name = "foo"
version = "1.0.0"
reovim-version = "^0.15"

[dependencies]
bar = { path = "../bar" }
"#,
    );
    write_manifest(
        &root.path().join("bar"),
        r#"
[package]
name = "bar"
version = "1.0.0"
reovim-version = "^0.15"

[dependencies]
foo = { path = "../foo" }
"#,
    );
    write_manifest(
        root.path(),
        r#"
[package]
name = "root"
reovim-version = "^0.15"

[dependencies]
foo = { path = "foo" }
"#,
    );

    let err = resolve(root.path(), &runtime()).expect_err("cycle must fail");
    match err {
        ResolveError::Cycle { chain } => {
            // DFS order: ancestor first, offender last.
            assert_eq!(chain, vec!["root", "foo", "bar", "foo"]);
        }
        other => panic!("expected Cycle, got {other:?}"),
    }
}

#[test]
fn resolve_detects_duplicate_package_on_different_paths() {
    let root = tempdir().unwrap();
    write_manifest(
        &root.path().join("first"),
        r#"
[package]
name = "same"
version = "1.0.0"
reovim-version = "^0.15"
"#,
    );
    write_manifest(
        &root.path().join("second"),
        r#"
[package]
name = "same"
version = "2.0.0"
reovim-version = "^0.15"
"#,
    );
    write_manifest(
        root.path(),
        r#"
[package]
name = "root"
reovim-version = "^0.15"

[dependencies]
a = { path = "first" }
b = { path = "second" }
"#,
    );

    let err = resolve(root.path(), &runtime()).expect_err("duplicate must fail");
    match err {
        ResolveError::DuplicatePackage {
            pkg,
            path_a,
            path_b,
        } => {
            assert_eq!(pkg, "same");
            assert_ne!(path_a, path_b);
        }
        other => panic!("expected DuplicatePackage, got {other:?}"),
    }
}

#[test]
fn resolve_rejects_dep_without_path() {
    let root = tempdir().unwrap();
    write_manifest(
        root.path(),
        r#"
[package]
name = "root"
reovim-version = "^0.15"

[dependencies]
bare = "1.0"
"#,
    );
    let err = resolve(root.path(), &runtime()).expect_err("bare-version must fail");
    match err {
        ResolveError::UnresolvableDependency {
            pkg,
            requirer,
            reason,
        } => {
            assert_eq!(pkg, "bare");
            assert_eq!(requirer, "root");
            assert!(reason.contains("path"));
        }
        other => panic!("expected UnresolvableDependency, got {other:?}"),
    }
}

#[test]
fn resolve_requires_path_dep_version() {
    let root = tempdir().unwrap();
    write_manifest(
        &root.path().join("versionless"),
        r#"
[package]
name = "versionless"
reovim-version = "^0.15"
"#,
    );
    write_manifest(
        root.path(),
        r#"
[package]
name = "root"
reovim-version = "^0.15"

[dependencies]
versionless = { path = "versionless" }
"#,
    );

    let err = resolve(root.path(), &runtime()).expect_err("missing version must fail");
    match err {
        ResolveError::MissingPackageVersion { pkg, at } => {
            assert_eq!(pkg, "versionless");
            assert!(at.ends_with("versionless/pkg.toml"));
        }
        other => panic!("expected MissingPackageVersion, got {other:?}"),
    }
}

#[test]
fn resolve_rejects_incompatible_reovim_version() {
    let root = tempdir().unwrap();
    write_manifest(
        root.path(),
        r#"
[package]
name = "root"
reovim-version = "^0.20"
"#,
    );
    let err = resolve(root.path(), &runtime()).expect_err("incompat must fail");
    match err {
        ResolveError::IncompatibleReovimVersion { required, actual } => {
            assert_eq!(required, "^0.20");
            assert_eq!(actual, "0.15.0");
        }
        other => panic!("expected IncompatibleReovimVersion, got {other:?}"),
    }
}

#[test]
fn resolve_rejects_invalid_reovim_constraint() {
    let root = tempdir().unwrap();
    write_manifest(
        root.path(),
        r#"
[package]
name = "root"
reovim-version = "totally not semver"
"#,
    );
    let err = resolve(root.path(), &runtime()).expect_err("bad constraint must fail");
    assert!(matches!(err, ResolveError::InvalidConstraint { .. }));
}

#[test]
fn resolve_rejects_invalid_package_version() {
    let root = tempdir().unwrap();
    write_manifest(
        &root.path().join("broken"),
        r#"
[package]
name = "broken"
version = "not-a-version"
reovim-version = "^0.15"
"#,
    );
    write_manifest(
        root.path(),
        r#"
[package]
name = "root"
reovim-version = "^0.15"

[dependencies]
broken = { path = "broken" }
"#,
    );
    let err = resolve(root.path(), &runtime()).expect_err("bad version must fail");
    assert!(matches!(err, ResolveError::InvalidVersion { .. }));
}

#[test]
fn resolve_rejects_invalid_dep_constraint() {
    let root = tempdir().unwrap();
    write_manifest(
        &root.path().join("foo"),
        r#"
[package]
name = "foo"
version = "1.0.0"
reovim-version = "^0.15"
"#,
    );
    write_manifest(
        root.path(),
        r#"
[package]
name = "root"
reovim-version = "^0.15"

[dependencies]
foo = { version = "~~bogus~~", path = "foo" }
"#,
    );
    let err = resolve(root.path(), &runtime()).expect_err("bad dep constraint must fail");
    assert!(matches!(err, ResolveError::InvalidConstraint { .. }));
}

#[test]
fn resolve_accepts_compatible_revisit_and_absolute_path() {
    // Diamond where both a and b accept leaf 1.0.0; leaf is referenced
    // via an absolute path from the root to exercise the
    // `candidate.is_absolute()` branch in `resolve_relative`.
    let root = tempdir().unwrap();
    write_manifest(
        &root.path().join("leaf"),
        r#"
[package]
name = "leaf"
version = "1.0.0"
reovim-version = "^0.15"
"#,
    );
    let abs_leaf = root.path().join("leaf").canonicalize().unwrap();
    write_manifest(
        &root.path().join("a"),
        &format!(
            r#"
[package]
name = "a"
version = "1.0.0"
reovim-version = "^0.15"

[dependencies]
leaf = {{ version = "^1", path = "{}" }}
"#,
            abs_leaf.display(),
        ),
    );
    write_manifest(
        &root.path().join("b"),
        &format!(
            r#"
[package]
name = "b"
version = "1.0.0"
reovim-version = "^0.15"

[dependencies]
leaf = {{ version = ">=1.0", path = "{}" }}
"#,
            abs_leaf.display(),
        ),
    );
    write_manifest(
        root.path(),
        r#"
[package]
name = "root"
reovim-version = "^0.15"

[dependencies]
a = { path = "a" }
b = { path = "b" }
"#,
    );

    let resolved = resolve(root.path(), &runtime()).expect("diamond with compatible re-check");
    let names: Vec<&str> = resolved.packages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["a", "b", "leaf"]);
}

#[test]
fn resolve_accepts_revisit_without_constraint() {
    // Diamond where the second requirer has no `version =` on the dep,
    // exercising the None arm of the revisit constraint check.
    let root = tempdir().unwrap();
    write_manifest(
        &root.path().join("leaf"),
        r#"
[package]
name = "leaf"
version = "1.0.0"
reovim-version = "^0.15"
"#,
    );
    write_manifest(
        &root.path().join("a"),
        r#"
[package]
name = "a"
version = "1.0.0"
reovim-version = "^0.15"

[dependencies]
leaf = { version = "^1", path = "../leaf" }
"#,
    );
    write_manifest(
        &root.path().join("b"),
        r#"
[package]
name = "b"
version = "1.0.0"
reovim-version = "^0.15"

[dependencies]
leaf = { path = "../leaf" }
"#,
    );
    write_manifest(
        root.path(),
        r#"
[package]
name = "root"
reovim-version = "^0.15"

[dependencies]
a = { path = "a" }
b = { path = "b" }
"#,
    );

    let resolved = resolve(root.path(), &runtime()).expect("no-constraint revisit ok");
    assert_eq!(resolved.packages.len(), 3);
}

#[test]
fn resolve_rechecks_constraint_on_revisited_package() {
    // Diamond: root → a, root → b; a and b both depend on `leaf`.
    // The second visit (through b) must re-check its own constraint
    // against the already-resolved version — covers the revisit branch.
    let root = tempdir().unwrap();
    write_manifest(
        &root.path().join("leaf"),
        r#"
[package]
name = "leaf"
version = "1.0.0"
reovim-version = "^0.15"
"#,
    );
    write_manifest(
        &root.path().join("a"),
        r#"
[package]
name = "a"
version = "1.0.0"
reovim-version = "^0.15"

[dependencies]
leaf = { version = "^1", path = "../leaf" }
"#,
    );
    write_manifest(
        &root.path().join("b"),
        r#"
[package]
name = "b"
version = "1.0.0"
reovim-version = "^0.15"

[dependencies]
leaf = { version = "^2", path = "../leaf" }
"#,
    );
    write_manifest(
        root.path(),
        r#"
[package]
name = "root"
reovim-version = "^0.15"

[dependencies]
a = { path = "a" }
b = { path = "b" }
"#,
    );
    let err = resolve(root.path(), &runtime()).expect_err("re-check must surface mismatch");
    match err {
        ResolveError::VersionMismatch {
            pkg,
            requirer,
            required,
            actual,
        } => {
            assert_eq!(pkg, "leaf");
            assert_eq!(requirer, "b");
            assert_eq!(required, "^2");
            assert_eq!(actual, "1.0.0");
        }
        other => panic!("expected VersionMismatch on revisit, got {other:?}"),
    }
}

#[test]
fn resolve_error_display_is_user_facing() {
    // Table-driven assertion: every ResolveError variant must render
    // its stored context into Display so `eprintln!("{err}")` is
    // sufficient.
    use std::path::PathBuf;

    let manifest_err = reovim_pkg_manifest::Manifest::from_toml_str("[package]\nname=\"x\"")
        .expect_err("malformed for Display fixture");
    let req_err = semver::VersionReq::parse("gibberish").unwrap_err();

    let cases: Vec<(ResolveError, Vec<&str>)> = vec![
        (
            ResolveError::ManifestNotFound {
                at: PathBuf::from("/tmp/x"),
            },
            vec!["manifest not found", "/tmp/x"],
        ),
        (
            ResolveError::ManifestParse {
                at: PathBuf::from("/tmp/y/pkg.toml"),
                source: manifest_err,
            },
            vec!["parse manifest", "/tmp/y/pkg.toml"],
        ),
        (
            ResolveError::MissingPackageVersion {
                pkg: "no-ver".into(),
                at: PathBuf::from("/tmp/z/pkg.toml"),
            },
            vec!["no-ver", "/tmp/z/pkg.toml", "version"],
        ),
        (
            ResolveError::InvalidVersion {
                pkg: "bad-ver".into(),
                raw: "nope".into(),
                source: semver::Version::parse("not-a-version").unwrap_err(),
            },
            vec!["bad-ver", "nope"],
        ),
        (
            ResolveError::InvalidConstraint {
                pkg: "bad-req".into(),
                requirer: "parent".into(),
                raw: "?!".into(),
                source: req_err,
            },
            vec!["bad-req", "parent", "?!"],
        ),
        (
            ResolveError::VersionMismatch {
                pkg: "vm".into(),
                requirer: "vm-parent".into(),
                required: "^2".into(),
                actual: "1.0.0".into(),
            },
            vec!["vm", "vm-parent", "^2", "1.0.0"],
        ),
        (
            ResolveError::Cycle {
                chain: vec!["a".into(), "b".into(), "a".into()],
            },
            vec!["cycle", "a", "b"],
        ),
        (
            ResolveError::UnresolvableDependency {
                pkg: "ur".into(),
                requirer: "ur-parent".into(),
                reason: "no registry yet",
            },
            vec!["ur", "ur-parent", "no registry yet"],
        ),
        (
            ResolveError::IncompatibleReovimVersion {
                required: "^0.20".into(),
                actual: "0.15.0".into(),
            },
            vec!["^0.20", "0.15.0"],
        ),
        (
            ResolveError::DuplicatePackage {
                pkg: "dup".into(),
                path_a: PathBuf::from("/a"),
                path_b: PathBuf::from("/b"),
            },
            vec!["dup", "/a", "/b"],
        ),
    ];

    for (err, needles) in cases {
        let msg = err.to_string();
        for needle in needles {
            assert!(msg.contains(needle), "Display for {err:?} missing `{needle}`: got `{msg}`");
        }
    }
}
