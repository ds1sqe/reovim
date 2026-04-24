//! Unit tests for the [`inventory`] module.

use std::path::PathBuf;

use {reovim_pkg_lockfile::Source, reovim_pkg_manifest::PackageKind, tempfile::tempdir};

use {
    super::{InstalledPackage, list, read_inventory, remove_from_inventory, write_inventory},
    crate::error::InstallError,
};

fn entry(name: &str, kind: PackageKind, library_root: &std::path::Path) -> InstalledPackage {
    let sub = match kind {
        PackageKind::Driver => "driver",
        PackageKind::Module => "modules",
    };
    let filename = super::cdylib_filename(name);
    InstalledPackage {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        source: Source::LocalPath(PathBuf::from(format!("/src/{name}"))),
        kind,
        target: "x86_64-unknown-linux-gnu".to_string(),
        sha256: "a".repeat(64),
        installed_path: library_root.join(sub).join(filename),
    }
}

#[test]
fn list_of_missing_lockfile_is_empty() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("pkg.lock");
    let root = dir.path();
    let items = list(&lock, root).expect("ok");
    assert!(items.is_empty());
}

#[test]
fn write_then_read_round_trips_and_sorts() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("pkg.lock");
    let root = dir.path();
    let a = entry("alpha", PackageKind::Driver, root);
    let z = entry("zeta", PackageKind::Module, root);
    write_inventory(&lock, &[z, a]).unwrap();

    let items = read_inventory(&lock, root).expect("ok");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "alpha");
    assert_eq!(items[0].kind, PackageKind::Driver);
    assert_eq!(items[1].name, "zeta");
    assert_eq!(items[1].kind, PackageKind::Module);
}

#[test]
fn remove_from_inventory_returns_record() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("pkg.lock");
    let root = dir.path();
    let a = entry("alpha", PackageKind::Driver, root);
    let b = entry("beta", PackageKind::Module, root);
    write_inventory(&lock, &[a, b]).unwrap();

    let removed = remove_from_inventory(&lock, root, "alpha").unwrap();
    assert_eq!(removed.name, "alpha");
    let items = read_inventory(&lock, root).expect("ok");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "beta");
}

#[test]
fn remove_from_inventory_on_missing_errors() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("pkg.lock");
    let root = dir.path();
    write_inventory(&lock, &[]).unwrap();
    let err = remove_from_inventory(&lock, root, "ghost").expect_err("must fail");
    assert!(matches!(err, InstallError::NotInstalled { ref name } if name == "ghost"));
}

#[test]
fn missing_kind_field_surfaces_error() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("pkg.lock");
    let src = r#"
version = 1

[[package]]
name = "stray"
version = "1.0.0"
target = "x86_64-unknown-linux-gnu"
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[package.source]
kind = "local-path"
path = "/src/stray"
"#;
    std::fs::write(&lock, src).unwrap();
    let err = read_inventory(&lock, dir.path()).expect_err("must fail");
    assert!(matches!(err, InstallError::MissingPackageKind { ref pkg } if pkg == "stray"));
}

#[test]
fn malformed_lockfile_surfaces_lockfile_error() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("pkg.lock");
    std::fs::write(&lock, "not == valid ==").unwrap();
    let err = read_inventory(&lock, dir.path()).expect_err("must fail");
    assert!(matches!(err, InstallError::Lockfile(_)));
}

#[test]
fn missing_lockfile_file_surfaces_read_error() {
    let err = read_inventory(std::path::Path::new("/no/such/lock"), std::path::Path::new("/"))
        .expect_err("must fail");
    assert!(matches!(err, InstallError::ArtifactReadFailed { .. }));
}
