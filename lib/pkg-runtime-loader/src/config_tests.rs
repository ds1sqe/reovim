//! Tests for [`super::RuntimeLoaderConfig`].

use std::{path::PathBuf, sync::Arc};

use {reovim_dylib_loader::Kind, reovim_pkg_lazyload::LazyRegistry};

use super::RuntimeLoaderConfig;

#[test]
fn from_registry_bundles_inputs_unchanged() {
    let registry = Arc::new(LazyRegistry::empty());
    let cfg =
        RuntimeLoaderConfig::from_registry("/var/lib/reovim", Kind::Module, Arc::clone(&registry));
    assert_eq!(cfg.library_root, PathBuf::from("/var/lib/reovim"));
    assert_eq!(cfg.kind, Kind::Module);
    assert!(Arc::ptr_eq(&cfg.registry, &registry));
}

#[test]
fn scan_dir_uses_kind_subdir() {
    let registry = Arc::new(LazyRegistry::empty());
    let driver_cfg =
        RuntimeLoaderConfig::from_registry("/root", Kind::Driver, Arc::clone(&registry));
    assert_eq!(driver_cfg.scan_dir(), PathBuf::from("/root/driver"));

    let module_cfg = RuntimeLoaderConfig::from_registry("/root", Kind::Module, registry);
    assert_eq!(module_cfg.scan_dir(), PathBuf::from("/root/modules"));
}

#[test]
fn load_returns_empty_registry_when_lockfile_missing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = RuntimeLoaderConfig::load(tmp.path(), Kind::Module).expect("load");
    assert_eq!(cfg.library_root, tmp.path());
    assert_eq!(cfg.registry.entries().count(), 0);
    assert_eq!(cfg.kind, Kind::Module);
}
