//! Tests for [`super::load_registry`] and [`super::lockfile_path`].

use std::path::Path;

use {
    reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
    reovim_pkg_manifest::{LazyTrigger, trigger_str},
};

use {
    super::{load_registry, lockfile_path},
    crate::error::RuntimeLoaderError,
};

fn pkg(name: &str, trigger: Option<&str>) -> PackageLock {
    PackageLock {
        name: name.into(),
        version: "1.0.0".into(),
        source: Source::LocalPath(format!("/pkgs/{name}").into()),
        target: None,
        kind: None,
        sha256: None,
        trigger: trigger.map(str::to_owned),
        dependencies: Vec::new(),
    }
}

fn lockfile(packages: Vec<PackageLock>) -> Lockfile {
    Lockfile {
        version: 1,
        packages,
    }
}

fn write_lockfile(root: &Path, lock: &Lockfile) {
    let body = lock.to_toml_string().expect("serialize lockfile");
    std::fs::write(root.join("pkg.lock"), body).expect("write lockfile");
}

#[test]
fn lockfile_path_appends_pkg_lock() {
    assert_eq!(lockfile_path(Path::new("/lib/reovim")), Path::new("/lib/reovim/pkg.lock"));
}

#[test]
fn missing_lockfile_returns_empty_registry() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let registry = load_registry(tmp.path()).expect("missing-lockfile is not an error");
    assert_eq!(registry.entries().count(), 0);
}

#[test]
fn malformed_lockfile_surfaces_lockfile_malformed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(tmp.path().join("pkg.lock"), "this is not toml at all").expect("write garbage");
    let err = load_registry(tmp.path()).expect_err("should reject malformed toml");
    match err {
        RuntimeLoaderError::LockfileMalformed { path, .. } => {
            assert_eq!(path, tmp.path().join("pkg.lock"));
        }
        other => panic!("expected LockfileMalformed, got {other:?}"),
    }
}

#[test]
fn malformed_trigger_surfaces_registry_build_failed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_lockfile(tmp.path(), &lockfile(vec![pkg("broken", Some("garbage"))]));
    let err = load_registry(tmp.path()).expect_err("should reject malformed trigger");
    match err {
        RuntimeLoaderError::RegistryBuildFailed { path, source } => {
            assert_eq!(path, tmp.path().join("pkg.lock"));
            let msg = source.to_string();
            assert!(msg.contains("broken"), "msg should name the package: {msg}");
            assert!(msg.contains("garbage"), "msg should echo the offending value: {msg}");
        }
        other => panic!("expected RegistryBuildFailed, got {other:?}"),
    }
}

#[test]
fn happy_path_builds_registry_with_expected_entries() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let lock = lockfile(vec![
        pkg("eager-one", Some(&trigger_str(&LazyTrigger::Eager))),
        pkg("on-domain-one", Some(&trigger_str(&LazyTrigger::OnDomain("text".into())))),
        pkg("on-event-one", Some(&trigger_str(&LazyTrigger::OnEvent("save".into())))),
        pkg("legacy", None),
    ]);
    write_lockfile(tmp.path(), &lock);
    let registry = load_registry(tmp.path()).expect("happy-path build");

    assert_eq!(registry.trigger_for("eager-one"), LazyTrigger::Eager);
    assert_eq!(registry.trigger_for("on-domain-one"), LazyTrigger::OnDomain("text".into()));
    assert_eq!(registry.trigger_for("on-event-one"), LazyTrigger::OnEvent("save".into()));
    assert_eq!(registry.trigger_for("legacy"), LazyTrigger::Eager);
}

#[test]
fn unreadable_lockfile_surfaces_lockfile_read_failed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let lockfile = tmp.path().join("pkg.lock");
    std::fs::create_dir(&lockfile).expect("dir-as-lockfile");

    let err = load_registry(tmp.path()).expect_err("dir-as-file should error");
    match err {
        RuntimeLoaderError::LockfileReadFailed { path, reason } => {
            assert_eq!(path, lockfile);
            assert!(!reason.is_empty(), "reason should be populated");
        }
        other => panic!("expected LockfileReadFailed, got {other:?}"),
    }
}
