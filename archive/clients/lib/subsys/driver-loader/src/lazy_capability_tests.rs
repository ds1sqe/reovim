//! Tests for [`super::CapabilityLazyHook`].
//!
//! Tests stage the workspace `reovim-driver-abi-poc` (render) and
//! `reovim-driver-debug-poc` (debug) fixture cdylibs under the
//! `libreovim_pkg_<name>.so` convention, build a fixture
//! `LazyRegistry` keyed to that name, and exercise
//! `dispatch_capability` directly.
//!
//! Clippy's `significant_drop_tightening` flags the
//! `let store = hook.drivers().lock(); store.<assertion>` pattern as
//! a candidate for collapse — but the lock guard `store` must outlive
//! every assertion that reads through it. Suppressed at module scope.

#![allow(clippy::significant_drop_tightening)]

use std::{path::PathBuf, sync::Arc};

use {
    reovim_dylib_loader::{Kind, cdylib_filename},
    reovim_pkg_lazyload::LazyRegistry,
    reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
};

use super::CapabilityLazyHook;

fn fixture_workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .map(std::path::Path::to_path_buf)
        .expect("workspace root")
}

fn render_fixture_so() -> Option<PathBuf> {
    let so = fixture_workspace_root()
        .join("target")
        .join("debug")
        .join("libreovim_driver_abi_poc.so");
    so.exists().then_some(so)
}

fn debug_fixture_so() -> Option<PathBuf> {
    let so = fixture_workspace_root()
        .join("target")
        .join("debug")
        .join("libreovim_driver_debug_poc.so");
    so.exists().then_some(so)
}

/// Stage one or more fixture cdylibs into a tempdir's `driver/`
/// subdir under the package-manager filename convention.
fn stage(spec: &[(&str, FixtureKind)]) -> Option<tempfile::TempDir> {
    let tmp = tempfile::tempdir().ok()?;
    let dir = tmp.path().join(Kind::Driver.subdir());
    std::fs::create_dir_all(&dir).ok()?;
    for (pkg_name, kind) in spec {
        let so = match kind {
            FixtureKind::Render => render_fixture_so()?,
            FixtureKind::Debug => debug_fixture_so()?,
        };
        let staged = dir.join(cdylib_filename(pkg_name));
        std::fs::copy(&so, &staged).ok()?;
    }
    Some(tmp)
}

#[derive(Clone, Copy)]
enum FixtureKind {
    Render,
    Debug,
}

macro_rules! require_fixtures {
    ($spec:expr) => {
        match stage($spec) {
            Some(tmp) => tmp,
            None => {
                eprintln!(
                    "SKIP: fixture cdylibs missing; run `cargo build -p reovim-driver-abi-poc -p reovim-driver-debug-poc`"
                );
                return;
            }
        }
    };
}

fn registry_with(packages: Vec<PackageLock>) -> Arc<LazyRegistry> {
    Arc::new(
        LazyRegistry::from_lockfile(&Lockfile {
            version: 1,
            packages,
        })
        .expect("registry"),
    )
}

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

#[test]
fn matching_capability_loads_render_driver() {
    let tmp = require_fixtures!(&[("cell-driver", FixtureKind::Render)]);
    let registry = registry_with(vec![pkg("cell-driver", Some("on-capability:cell"))]);
    let hook = CapabilityLazyHook::new(registry, tmp.path());

    hook.dispatch_capability("cell").expect("dispatch ok");
    let store = hook.drivers().lock();
    assert!(
        store.render.contains_key("cell-driver"),
        "matching capability must load the render driver into DriverStore"
    );
    assert_eq!(store.debug.len(), 0);
}

#[test]
fn matching_capability_loads_debug_driver() {
    let tmp = require_fixtures!(&[("debug-driver", FixtureKind::Debug)]);
    let registry = registry_with(vec![pkg("debug-driver", Some("on-capability:debug"))]);
    let hook = CapabilityLazyHook::new(registry, tmp.path());

    hook.dispatch_capability("debug").expect("dispatch ok");
    let store = hook.drivers().lock();
    assert!(
        store.debug.contains_key("debug-driver"),
        "matching capability must load the debug driver"
    );
    assert_eq!(store.render.len(), 0);
}

#[test]
fn unrelated_capability_loads_nothing() {
    let tmp = require_fixtures!(&[("cell-driver", FixtureKind::Render)]);
    let registry = registry_with(vec![pkg("cell-driver", Some("on-capability:cell"))]);
    let hook = CapabilityLazyHook::new(registry, tmp.path());

    hook.dispatch_capability("dom").expect("dispatch ok");
    let store = hook.drivers().lock();
    assert_eq!(store.render.len(), 0);
    assert_eq!(store.debug.len(), 0);
}

#[test]
fn repeated_dispatch_loads_only_once() {
    let tmp = require_fixtures!(&[("cell-driver", FixtureKind::Render)]);
    let registry = registry_with(vec![pkg("cell-driver", Some("on-capability:cell"))]);
    let hook = CapabilityLazyHook::new(registry, tmp.path());

    hook.dispatch_capability("cell").expect("first dispatch");
    hook.dispatch_capability("cell").expect("second dispatch");
    hook.dispatch_capability("cell").expect("third dispatch");

    let store = hook.drivers().lock();
    assert_eq!(store.render.len(), 1, "pending-set dedup must hold");
}

#[test]
fn missing_cdylib_returns_load_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join(Kind::Driver.subdir())).expect("driver dir");

    let registry = registry_with(vec![pkg("absent", Some("on-capability:cell"))]);
    let hook = CapabilityLazyHook::new(registry, tmp.path());

    let err = hook
        .dispatch_capability("cell")
        .expect_err("missing cdylib should error");
    let msg = err.to_string();
    assert!(
        msg.contains("absent") || msg.to_lowercase().contains("library"),
        "error should mention the missing path: {msg}"
    );

    // Pending-set drained: a second dispatch is a no-op (no error).
    hook.dispatch_capability("cell")
        .expect("retry must short-circuit since pending drained");
}

#[test]
fn entries_without_on_capability_trigger_are_not_pre_pending() {
    let tmp = require_fixtures!(&[("driver", FixtureKind::Render)]);
    let registry = registry_with(vec![pkg("driver", Some("on-event:save"))]);
    let hook = CapabilityLazyHook::new(registry, tmp.path());

    hook.dispatch_capability("save").expect("dispatch");
    hook.dispatch_capability("driver").expect("dispatch");

    let store = hook.drivers().lock();
    assert_eq!(store.render.len(), 0);
    assert_eq!(store.debug.len(), 0);
}
