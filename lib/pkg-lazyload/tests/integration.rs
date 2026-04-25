//! Cold-start integration: full `scan_paths` vs filtered `scan_eager`
//! against the same on-disk fixture. The filtered scan must drop
//! every entry when every package is lazy and must surface every
//! entry the full scan finds when no package is lazy.

use std::{fs, path::PathBuf};

use {
    reovim_dylib_loader::{Kind, cdylib_filename, scan_paths},
    reovim_pkg_lazyload::{LazyRegistry, scan_eager},
    reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
    tempfile::tempdir,
};

const PACKAGES: &[&str] = &["alpha", "beta", "gamma"];

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
fn scan_eager_with_all_lazy_registry_skips_every_dlopen() {
    let root = tempdir().expect("tempdir");
    let dir = root.path().join(Kind::Driver.subdir());
    fs::create_dir_all(&dir).unwrap();
    let bytes = driver_poc_bytes();
    for name in PACKAGES {
        fs::write(dir.join(cdylib_filename(name)), &bytes).expect("write cdylib");
    }

    let lock = Lockfile {
        version: 1,
        packages: PACKAGES
            .iter()
            .map(|n| pkg(n, Some("on-domain:text")))
            .collect(),
    };
    let registry = LazyRegistry::from_lockfile(&lock).expect("registry");

    let dirs = vec![dir];
    let full = scan_paths(&dirs);
    let eager = scan_eager(root.path(), Kind::Driver, &registry);

    assert_eq!(full.len(), PACKAGES.len(), "full scan should see every cdylib");
    assert!(eager.is_empty(), "all-lazy registry must drop every entry");
}
