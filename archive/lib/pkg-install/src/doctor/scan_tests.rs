//! Tests for [`super::scan_orphans`].

use std::{
    fs,
    path::{Path, PathBuf},
};

use {reovim_dylib_loader::cdylib_filename, tempfile::tempdir};

use {
    crate::inventory::InstalledPackage,
    reovim_pkg_lockfile::Source,
    reovim_pkg_manifest::{LazyTrigger, PackageKind},
};

use super::scan_orphans;

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

fn write_cdylib(dir: &Path, pkg_name: &str) -> PathBuf {
    fs::create_dir_all(dir).expect("mkdir");
    let dst = dir.join(cdylib_filename(pkg_name));
    fs::write(&dst, driver_poc_bytes()).expect("write cdylib");
    dst
}

fn entry(name: &str, kind: PackageKind, library_root: &Path) -> InstalledPackage {
    let sub = match kind {
        PackageKind::Driver => "driver",
        PackageKind::Module => "modules",
    };
    let filename = cdylib_filename(name);
    InstalledPackage {
        name: name.into(),
        version: "1.0.0".into(),
        source: Source::LocalPath(PathBuf::from(format!("/pkgs/{name}"))),
        kind,
        target: "x86_64-unknown-linux-gnu".into(),
        sha256: "0".repeat(64),
        installed_path: library_root.join(sub).join(filename),
        trigger: Some(LazyTrigger::Eager),
    }
}

#[test]
fn scan_orphans_finds_unowned_cdylib() {
    let root = tempdir().expect("tempdir");
    let driver_dir = root.path().join("driver");
    write_cdylib(&driver_dir, "ghost");
    let orphans = scan_orphans(root.path(), &[]);
    assert_eq!(orphans.len(), 1);
    assert!(orphans[0].path.ends_with(cdylib_filename("ghost")));
}

#[test]
fn scan_orphans_skips_inventoried_cdylib() {
    let root = tempdir().expect("tempdir");
    let driver_dir = root.path().join("driver");
    write_cdylib(&driver_dir, "alpha");
    let inventory = vec![entry("alpha", PackageKind::Driver, root.path())];
    let orphans = scan_orphans(root.path(), &inventory);
    assert!(orphans.is_empty());
}

#[test]
fn scan_orphans_skips_foreign_filename() {
    let root = tempdir().expect("tempdir");
    let driver_dir = root.path().join("driver");
    fs::create_dir_all(&driver_dir).unwrap();
    fs::write(driver_dir.join("libsomething.so"), driver_poc_bytes()).unwrap();
    let orphans = scan_orphans(root.path(), &[]);
    assert!(orphans.is_empty(), "foreign cdylib should not be flagged: {orphans:?}");
}

#[test]
fn scan_orphans_walks_both_kinds() {
    let root = tempdir().expect("tempdir");
    write_cdylib(&root.path().join("driver"), "drv-orphan");
    write_cdylib(&root.path().join("modules"), "mod-orphan");
    let orphans = scan_orphans(root.path(), &[]);
    assert_eq!(orphans.len(), 2);
    let stems: Vec<_> = orphans
        .iter()
        .filter_map(super::OrphanFinding::package_name)
        .collect();
    assert!(stems.contains(&"drv-orphan".to_string()));
    assert!(stems.contains(&"mod-orphan".to_string()));
}

#[test]
fn scan_orphans_empty_when_root_missing() {
    let root = tempdir().expect("tempdir");
    let nonexistent = root.path().join("does-not-exist");
    let orphans = scan_orphans(&nonexistent, &[]);
    assert!(orphans.is_empty());
}

#[test]
fn scan_orphans_skips_non_cdylib_files() {
    let root = tempdir().expect("tempdir");
    let driver_dir = root.path().join("driver");
    fs::create_dir_all(&driver_dir).unwrap();
    fs::write(driver_dir.join("README.md"), "hello").unwrap();
    fs::write(driver_dir.join("notes.txt"), "stuff").unwrap();
    let orphans = scan_orphans(root.path(), &[]);
    assert!(orphans.is_empty());
}
