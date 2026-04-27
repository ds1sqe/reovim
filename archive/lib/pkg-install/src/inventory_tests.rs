//! Unit tests for the [`inventory`] module.

use std::path::PathBuf;

use {
    reovim_dylib_loader::cdylib_filename, reovim_pkg_lockfile::Source,
    reovim_pkg_manifest::PackageKind, tempfile::tempdir,
};

use {
    super::{InstalledPackage, list, read_inventory, remove_from_inventory, write_inventory},
    crate::error::InstallError,
};

fn entry(name: &str, kind: PackageKind, library_root: &std::path::Path) -> InstalledPackage {
    let sub = match kind {
        PackageKind::Driver => "driver",
        PackageKind::Module => "modules",
    };
    let filename = cdylib_filename(name);
    InstalledPackage {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        source: Source::LocalPath(PathBuf::from(format!("/src/{name}"))),
        kind,
        target: "x86_64-unknown-linux-gnu".to_string(),
        sha256: "a".repeat(64),
        trigger: None,
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
fn read_inventory_rejects_invalid_kind() {
    // A lockfile with `kind = "garbage"` is corrupted; the parser
    // surfaces it as `MissingPackageKind` rather than silently
    // skipping the entry.
    let dir = tempdir().unwrap();
    let lock = dir.path().join("pkg.lock");
    let src = r#"
version = 1

[[package]]
name = "broken"
version = "1.0.0"
kind = "garbage"
target = "x86_64-unknown-linux-gnu"
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[package.source]
kind = "local-path"
path = "/src/broken"
"#;
    std::fs::write(&lock, src).unwrap();
    let err = read_inventory(&lock, dir.path()).expect_err("must fail");
    assert!(matches!(err, InstallError::MissingPackageKind { ref pkg } if pkg == "broken"));
}

#[test]
fn read_inventory_rejects_malformed_trigger() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("pkg.lock");
    let src = r#"
version = 1

[[package]]
name = "lazy"
version = "1.0.0"
kind = "module"
target = "x86_64-unknown-linux-gnu"
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
trigger = "garbage"

[package.source]
kind = "local-path"
path = "/src/lazy"
"#;
    std::fs::write(&lock, src).unwrap();
    let err = read_inventory(&lock, dir.path()).expect_err("must fail");
    assert!(matches!(err, InstallError::Manifest { .. }), "got {err:?}");
}

#[test]
fn read_inventory_skips_pre_install_entries() {
    // `pkg lock` writes lockfile entries with `kind = None` (kind
    // is install-time metadata). `read_inventory` must treat those
    // entries as "not yet installed" and skip them, so that a
    // subsequent `pkg install` can populate the real inventory
    // without tripping the missing-kind error.
    let dir = tempdir().unwrap();
    let lock = dir.path().join("pkg.lock");
    let src = r#"
version = 1

[[package]]
name = "pre-install"
version = "1.0.0"

[package.source]
kind = "local-path"
path = "/src/pre-install"
"#;
    std::fs::write(&lock, src).unwrap();
    let inventory = read_inventory(&lock, dir.path()).expect("read inventory");
    assert!(inventory.is_empty(), "pre-install entries must not appear: {inventory:?}");
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
