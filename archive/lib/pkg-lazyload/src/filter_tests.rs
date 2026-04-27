//! Tests for [`super::scan_eager`].

use std::{
    fs,
    path::{Path, PathBuf},
};

use {
    reovim_dylib_loader::{Kind, cdylib_filename},
    reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
    tempfile::tempdir,
};

use {super::scan_eager, crate::registry::LazyRegistry};

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

#[test]
fn scan_eager_returns_all_when_registry_empty() {
    let root = tempdir().expect("tempdir");
    let dir = root.path().join(Kind::Driver.subdir());
    fs::create_dir_all(&dir).unwrap();
    let bytes = driver_poc_bytes();
    write_cdylib(&dir, "alpha", &bytes);
    write_cdylib(&dir, "beta", &bytes);

    let lock = Lockfile {
        version: 1,
        packages: Vec::new(),
    };
    let registry = LazyRegistry::from_lockfile(&lock).expect("registry");

    let entries = scan_eager(root.path(), Kind::Driver, &registry);
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.outcome.is_ok()));
}

#[test]
fn scan_eager_drops_lazy_entries() {
    let root = tempdir().expect("tempdir");
    let dir = root.path().join(Kind::Driver.subdir());
    fs::create_dir_all(&dir).unwrap();
    let bytes = driver_poc_bytes();
    write_cdylib(&dir, "alpha", &bytes);
    write_cdylib(&dir, "lazy-one", &bytes);

    let lock = Lockfile {
        version: 1,
        packages: vec![
            pkg("alpha", Some("eager")),
            pkg("lazy-one", Some("on-domain:text")),
        ],
    };
    let registry = LazyRegistry::from_lockfile(&lock).expect("registry");

    let entries = scan_eager(root.path(), Kind::Driver, &registry);
    let names: Vec<String> = entries
        .iter()
        .map(|e| e.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.len(), 1);
    assert!(names[0].contains("alpha"), "kept entry: {names:?}");
    assert!(
        !names.iter().any(|n| n.contains("lazy_one")),
        "lazy_one must be filtered: {names:?}",
    );
}

#[test]
fn scan_eager_preserves_unknown_entries() {
    let root = tempdir().expect("tempdir");
    let dir = root.path().join(Kind::Driver.subdir());
    fs::create_dir_all(&dir).unwrap();
    let bytes = driver_poc_bytes();
    write_cdylib(&dir, "ghost", &bytes);

    let lock = Lockfile {
        version: 1,
        packages: vec![pkg("known", Some("on-domain:text"))],
    };
    let registry = LazyRegistry::from_lockfile(&lock).expect("registry");

    let entries = scan_eager(root.path(), Kind::Driver, &registry);
    assert_eq!(entries.len(), 1);
    assert!(
        entries[0]
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("ghost"),
        "ghost survived: {:?}",
        entries[0].path,
    );
}
