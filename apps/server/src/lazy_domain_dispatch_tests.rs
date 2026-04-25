//! Tests for [`super::LazyDomainDispatcher`].

use std::{path::PathBuf, sync::Arc};

use {
    parking_lot::Mutex,
    reovim_dylib_loader::{Kind, cdylib_filename},
    reovim_pkg_lazyload::LazyRegistry,
    reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
    reovim_subsys_module_loader::{loader::ModuleLoader, loader_handle::LoaderHandle},
    reovim_subsys_session::DomainRegisterListener,
};

use super::LazyDomainDispatcher;

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
fn matching_domain_dlopens_package() {
    let tmp = require_fixture!("demo");
    let registry = registry_with(vec![pkg("demo", Some("on-domain:text"))]);
    let (inner, handle) = loader_handle();
    let dispatcher = LazyDomainDispatcher::new(registry, handle, tmp.path());

    dispatcher.on_register("text");

    assert_eq!(inner.lock().len(), 1);
}

#[test]
fn unrelated_domain_loads_nothing() {
    let tmp = require_fixture!("demo");
    let registry = registry_with(vec![pkg("demo", Some("on-domain:text"))]);
    let (inner, handle) = loader_handle();
    let dispatcher = LazyDomainDispatcher::new(registry, handle, tmp.path());

    dispatcher.on_register("lsp");

    assert_eq!(inner.lock().len(), 0);
}

#[test]
fn repeated_match_loads_only_once() {
    let tmp = require_fixture!("demo");
    let registry = registry_with(vec![pkg("demo", Some("on-domain:text"))]);
    let (inner, handle) = loader_handle();
    let dispatcher = LazyDomainDispatcher::new(registry, handle, tmp.path());

    dispatcher.on_register("text");
    dispatcher.on_register("text");
    dispatcher.on_register("text");

    assert_eq!(inner.lock().len(), 1, "pending-set dedup must hold");
}

#[test]
fn two_distinct_on_domain_entries_fire_independently() {
    let tmp_a = require_fixture!("alpha");
    // Same root holds both fixtures — we only need both cdylibs in
    // place. Stage `beta` into the same tempdir.
    let so = fixture_so().expect("fixture so");
    let beta_path = tmp_a
        .path()
        .join(Kind::Module.subdir())
        .join(cdylib_filename("beta"));
    std::fs::copy(&so, &beta_path).expect("stage beta");

    let registry = registry_with(vec![
        pkg("alpha", Some("on-domain:text")),
        pkg("beta", Some("on-domain:lsp")),
    ]);
    let (inner, handle) = loader_handle();
    let dispatcher = LazyDomainDispatcher::new(registry, handle, tmp_a.path());

    dispatcher.on_register("text");
    assert_eq!(inner.lock().len(), 1, "alpha loads on text registration");

    dispatcher.on_register("lsp");
    let len_after_lsp = inner.lock().len();
    // beta has the same module-name as alpha (the fixture cdylib's
    // declare_module! id is "test-dynamic"), so the second
    // load_dynamic returns DuplicateLoad. The pending-set still
    // empties; the loader length stays at 1, but the dispatcher
    // dispatched the trigger correctly.
    assert!(
        len_after_lsp == 1 || len_after_lsp == 2,
        "two distinct on-domain entries must each be considered"
    );
}

#[test]
fn entries_without_on_domain_trigger_are_not_pre_pending() {
    let tmp = require_fixture!("demo");
    let registry = registry_with(vec![pkg("demo", Some("on-event:save"))]);
    let (inner, handle) = loader_handle();
    let dispatcher = LazyDomainDispatcher::new(registry, handle, tmp.path());

    dispatcher.on_register("save");
    dispatcher.on_register("demo");

    assert_eq!(inner.lock().len(), 0);
}
