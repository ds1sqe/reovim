//! Tests for [`super::names_to_load`] and [`super::load_triggered`].

use std::{
    fs,
    path::{Path, PathBuf},
};

use {
    reovim_dylib_loader::{Kind, cdylib_filename},
    reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
    tempfile::tempdir,
};

use {
    super::{TriggerEvent, load_triggered, names_to_load},
    crate::registry::LazyRegistry,
};

fn driver_poc_bytes() -> Vec<u8> {
    let exe = std::env::current_exe().expect("test binary path");
    let deps = exe.parent().expect("test binary dir");
    for name in [
        "libreovim_driver_abi_poc.so",
        "libreovim_driver_abi_poc.dylib",
        "reovim_driver_abi_poc.dll",
    ] {
        let candidate = deps.join(name);
        if candidate.is_file() {
            return fs::read(&candidate).expect("read driver-abi-poc cdylib");
        }
    }
    panic!("driver-abi-poc cdylib not found next to {}", exe.display());
}

fn write_cdylib(dir: &Path, pkg_name: &str, bytes: &[u8]) -> PathBuf {
    let dst = dir.join(cdylib_filename(pkg_name));
    fs::write(&dst, bytes).expect("write cdylib");
    dst
}

fn pkg(name: &str, trigger: Option<&str>) -> PackageLock {
    PackageLock {
        name: name.into(),
        version: "1.0.0".into(),
        source: Source::LocalPath(PathBuf::from(format!("/pkgs/{name}"))),
        target: None,
        kind: None,
        sha256: None,
        trigger: trigger.map(str::to_owned),
        dependencies: Vec::new(),
    }
}

fn registry_with(packages: Vec<PackageLock>) -> LazyRegistry {
    let lock = Lockfile {
        version: 1,
        packages,
    };
    LazyRegistry::from_lockfile(&lock).expect("registry")
}

#[test]
fn names_to_load_filters_by_domain() {
    let registry = registry_with(vec![
        pkg("text-pkg", Some("on-domain:text")),
        pkg("html-pkg", Some("on-domain:html")),
        pkg("save-pkg", Some("on-event:save")),
        pkg("eager-pkg", Some("eager")),
    ]);
    let names = names_to_load(&registry, &TriggerEvent::Domain("text"));
    assert_eq!(names, vec!["text-pkg"]);
    let none = names_to_load(&registry, &TriggerEvent::Domain("unknown"));
    assert!(none.is_empty());
}

#[test]
fn names_to_load_filters_by_event() {
    let registry = registry_with(vec![
        pkg("save-pkg", Some("on-event:save")),
        pkg("autosave-pkg", Some("on-event:save")),
        pkg("text-pkg", Some("on-domain:text")),
    ]);
    let mut names = names_to_load(&registry, &TriggerEvent::Event("save"));
    names.sort_unstable();
    assert_eq!(names, vec!["autosave-pkg", "save-pkg"]);
}

#[test]
fn names_to_load_filters_by_capability() {
    let registry = registry_with(vec![
        pkg("render-pkg", Some("on-capability:render")),
        pkg("text-pkg", Some("on-domain:text")),
        pkg("eager-pkg", None),
    ]);
    let names = names_to_load(&registry, &TriggerEvent::Capability("render"));
    assert_eq!(names, vec!["render-pkg"]);
}

#[test]
fn load_triggered_opens_exactly_the_matching_cdylibs() {
    let root = tempdir().expect("tempdir");
    let dir = root.path().join(Kind::Driver.subdir());
    fs::create_dir_all(&dir).unwrap();
    let bytes = driver_poc_bytes();
    write_cdylib(&dir, "text-pkg", &bytes);
    write_cdylib(&dir, "html-pkg", &bytes);

    let registry = registry_with(vec![
        pkg("text-pkg", Some("on-domain:text")),
        pkg("html-pkg", Some("on-domain:html")),
    ]);

    let opened =
        load_triggered(root.path(), Kind::Driver, &registry, &TriggerEvent::Domain("text"));
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0].0, "text-pkg");
    assert!(opened[0].1.is_ok(), "open: {:?}", opened[0].1);
}

#[test]
fn load_triggered_surfaces_missing_file_error() {
    let root = tempdir().expect("tempdir");
    fs::create_dir_all(root.path().join(Kind::Driver.subdir())).unwrap();

    let registry = registry_with(vec![pkg("ghost-pkg", Some("on-domain:text"))]);
    let opened =
        load_triggered(root.path(), Kind::Driver, &registry, &TriggerEvent::Domain("text"));
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0].0, "ghost-pkg");
    assert!(opened[0].1.is_err(), "expected open error: {:?}", opened[0].1);
}
