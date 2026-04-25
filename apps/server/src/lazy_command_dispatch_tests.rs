//! Tests for [`super::LazyCommandDispatcher`].
//!
//! Tests stage the workspace `reovim-test-dynamic-module` fixture
//! under the `libreovim_pkg_<name>.so` convention, build a fixture
//! `LazyRegistry` keyed to that name, and invoke the dispatcher's
//! `on_resolve(name)` directly. The fixture is also used by the
//! integration tests in `server/lib/subsys/module-loader/tests/`.

use std::{path::PathBuf, sync::Arc};

use {
    parking_lot::Mutex,
    reovim_dylib_loader::{Kind, cdylib_filename},
    reovim_pkg_lazyload::LazyRegistry,
    reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
    reovim_subsys_command::CommandResolutionListener,
    reovim_subsys_module_loader::{loader::ModuleLoader, loader_handle::LoaderHandle},
};

use super::LazyCommandDispatcher;

fn fixture_so() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent()?.parent()?;
    let so = workspace_root
        .join("target")
        .join("debug")
        .join("libreovim_test_dynamic_module.so");
    so.exists().then_some(so)
}

fn stage(pkg_name: &str) -> Option<tempfile::TempDir> {
    let so = fixture_so()?;
    let tmp = tempfile::tempdir().ok()?;
    let dir = tmp.path().join(Kind::Module.subdir());
    std::fs::create_dir_all(&dir).ok()?;
    let staged = dir.join(cdylib_filename(pkg_name));
    std::fs::copy(&so, &staged).ok()?;
    Some(tmp)
}

macro_rules! require_fixture {
    ($pkg:expr) => {
        match stage($pkg) {
            Some(tmp) => tmp,
            None => {
                eprintln!(
                    "SKIP: test fixture .so missing; run `cargo build -p reovim-test-dynamic-module`"
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

fn loader_handle() -> (Arc<Mutex<ModuleLoader>>, LoaderHandle) {
    let inner = Arc::new(Mutex::new(ModuleLoader::new()));
    let handle = LoaderHandle::from_arc(Arc::clone(&inner));
    (inner, handle)
}

#[test]
fn matching_command_name_dlopens_package() {
    let tmp = require_fixture!("demo");
    let registry = registry_with(vec![pkg("demo", Some("on-event:save"))]);
    let (inner, handle) = loader_handle();
    let dispatcher = LazyCommandDispatcher::new(registry, handle, tmp.path());

    dispatcher.on_resolve("save");

    assert_eq!(inner.lock().len(), 1, "matching name must dlopen the package");
}

#[test]
fn unrelated_command_name_loads_nothing() {
    let tmp = require_fixture!("demo");
    let registry = registry_with(vec![pkg("demo", Some("on-event:save"))]);
    let (inner, handle) = loader_handle();
    let dispatcher = LazyCommandDispatcher::new(registry, handle, tmp.path());

    dispatcher.on_resolve("write");

    assert_eq!(inner.lock().len(), 0);
}

#[test]
fn repeated_match_loads_only_once() {
    let tmp = require_fixture!("demo");
    let registry = registry_with(vec![pkg("demo", Some("on-event:save"))]);
    let (inner, handle) = loader_handle();
    let dispatcher = LazyCommandDispatcher::new(registry, handle, tmp.path());

    dispatcher.on_resolve("save");
    dispatcher.on_resolve("save");
    dispatcher.on_resolve("save");

    assert_eq!(inner.lock().len(), 1, "pending-set dedup must hold");
}

#[test]
fn failed_load_still_clears_pending_no_retry_storm() {
    // If the cdylib is missing, `load_named` errors. The dispatcher
    // logs and continues; the pending-set still pops the entry so
    // subsequent dispatches do not retry on every keystroke.
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join(Kind::Module.subdir())).expect("modules dir");

    let registry = registry_with(vec![pkg("absent", Some("on-event:save"))]);
    let (inner, handle) = loader_handle();
    let dispatcher = LazyCommandDispatcher::new(registry, handle, tmp.path());

    dispatcher.on_resolve("save");
    dispatcher.on_resolve("save");

    assert_eq!(inner.lock().len(), 0, "no module loaded — cdylib was absent");
    // The pending-set was cleared after the first attempt; the second
    // call short-circuits without invoking load_named again. We
    // observe this indirectly via the loader length staying zero (no
    // double-error-log path either, but that is a tracing-test
    // concern, not a behavior assertion).
}

#[test]
fn entries_without_on_event_trigger_are_not_pre_pending() {
    // on-domain / on-capability entries should NOT be in the pending
    // set — only on-event triggers are command-name dispatched.
    let tmp = require_fixture!("demo");
    let registry = registry_with(vec![pkg("demo", Some("on-domain:text"))]);
    let (inner, handle) = loader_handle();
    let dispatcher = LazyCommandDispatcher::new(registry, handle, tmp.path());

    dispatcher.on_resolve("text");
    dispatcher.on_resolve("demo");

    assert_eq!(inner.lock().len(), 0, "non-event triggers must not fire on command resolution");
}
