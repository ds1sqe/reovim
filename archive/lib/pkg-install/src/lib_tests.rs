//! Unit tests for the public `install` / `uninstall` entry points.
//! End-to-end fixture tests live under `tests/`.

use std::{fs, path::Path};

use {
    reovim_pkg_manifest::PackageKind, reovim_pkg_resolver::resolve, semver::Version,
    tempfile::tempdir,
};

use {
    super::{install, uninstall},
    crate::error::InstallError,
};

fn driver_poc_bytes() -> Vec<u8> {
    // Resolve the cdylib at test-run time from the test binary's
    // sibling `deps/` directory. Build-time env vars race cargo's
    // crate-build order under `cargo test --workspace`.
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

fn write(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, body).unwrap();
}

fn stage_module_package(root: &Path, name: &str) {
    write(
        &root.join(name).join("pkg.toml"),
        &format!(
            "[package]\nname = \"{name}\"\nversion = \"1.0.0\"\nkind = \"module\"\nreovim-version = \"^0.15\"\n",
        ),
    );
    let filename = reovim_dylib_loader::cdylib_filename(name);
    let dst = root.join(name).join("dist").join(&filename);
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::write(&dst, driver_poc_bytes()).unwrap();
}

fn stage_root_manifest(root: &Path, dep_name: &str) {
    write(
        &root.join("pkg.toml"),
        &format!(
            "[package]\nname = \"root-setup\"\nreovim-version = \"^0.15\"\n\n[dependencies]\n{dep_name} = {{ path = \"{dep_name}\" }}\n",
        ),
    );
}

#[test]
fn install_single_module_populates_inventory_and_filesystem() {
    let dir = tempdir().unwrap();
    stage_module_package(dir.path(), "alpha");
    stage_root_manifest(dir.path(), "alpha");
    let lock = dir.path().join("pkg.lock");
    let lib = dir.path().join("library-root");

    let resolved = resolve(dir.path(), &Version::new(0, 15, 0)).expect("resolve");
    let installed = install(&resolved, &lib, &lock).expect("install");
    assert_eq!(installed.len(), 1);
    assert_eq!(installed[0].name, "alpha");
    assert_eq!(installed[0].kind, PackageKind::Module);
    assert!(installed[0].installed_path.is_file());

    let listed = crate::inventory::list(&lock, &lib).expect("list");
    assert_eq!(listed, installed);
}

#[test]
fn install_detects_tamper_against_existing_lockfile() {
    let dir = tempdir().unwrap();
    stage_module_package(dir.path(), "alpha");
    stage_root_manifest(dir.path(), "alpha");
    let lock = dir.path().join("pkg.lock");
    let lib = dir.path().join("library-root");

    let resolved = resolve(dir.path(), &Version::new(0, 15, 0)).expect("resolve");
    install(&resolved, &lib, &lock).expect("first install");

    // Tamper: rewrite the on-disk cdylib so its bytes diverge from
    // the lockfile's recorded sha256.
    let art = dir
        .path()
        .join("alpha/dist")
        .join(reovim_dylib_loader::cdylib_filename("alpha"));
    let mut bytes = fs::read(&art).unwrap();
    bytes.extend_from_slice(b"extra-bytes-that-change-the-hash");
    fs::write(&art, &bytes).unwrap();

    let err = install(&resolved, &lib, &lock).expect_err("tamper must fail");
    match err {
        InstallError::TamperedArtifact {
            pkg,
            expected,
            actual,
        } => {
            assert_eq!(pkg, "alpha");
            assert_ne!(expected, actual);
        }
        other => panic!("expected TamperedArtifact, got {other:?}"),
    }
}

#[test]
fn uninstall_removes_from_lockfile_and_disk() {
    let dir = tempdir().unwrap();
    stage_module_package(dir.path(), "alpha");
    stage_root_manifest(dir.path(), "alpha");
    let lock = dir.path().join("pkg.lock");
    let lib = dir.path().join("library-root");

    let resolved = resolve(dir.path(), &Version::new(0, 15, 0)).expect("resolve");
    install(&resolved, &lib, &lock).expect("install");

    let removed = uninstall("alpha", &lib, &lock).expect("uninstall");
    assert_eq!(removed.name, "alpha");
    assert!(!removed.installed_path.exists(), "cdylib still on disk");
    let listed = crate::inventory::list(&lock, &lib).expect("list after remove");
    assert!(listed.is_empty());
}

#[test]
fn uninstall_when_cdylib_already_gone_is_ok() {
    let dir = tempdir().unwrap();
    stage_module_package(dir.path(), "alpha");
    stage_root_manifest(dir.path(), "alpha");
    let lock = dir.path().join("pkg.lock");
    let lib = dir.path().join("library-root");

    let resolved = resolve(dir.path(), &Version::new(0, 15, 0)).expect("resolve");
    let installed = install(&resolved, &lib, &lock).expect("install");
    fs::remove_file(&installed[0].installed_path).unwrap();

    let removed = uninstall("alpha", &lib, &lock).expect("uninstall ok");
    assert_eq!(removed.name, "alpha");
}

#[test]
fn install_handles_driver_kind() {
    let dir = tempdir().unwrap();
    write(
        &dir.path().join("alpha/pkg.toml"),
        "[package]\nname = \"alpha\"\nversion = \"1.0.0\"\nkind = \"driver\"\nreovim-version = \"^0.15\"\n",
    );
    let fname = reovim_dylib_loader::cdylib_filename("alpha");
    let dst = dir.path().join("alpha/dist").join(&fname);
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::write(&dst, driver_poc_bytes()).unwrap();
    stage_root_manifest(dir.path(), "alpha");

    let resolved = resolve(dir.path(), &Version::new(0, 15, 0)).expect("resolve");
    let lock = dir.path().join("pkg.lock");
    let lib = dir.path().join("library-root");
    let installed = install(&resolved, &lib, &lock).expect("install");
    assert_eq!(installed[0].kind, PackageKind::Driver);
    assert!(
        installed[0]
            .installed_path
            .parent()
            .unwrap()
            .ends_with("driver")
    );
}

#[test]
fn install_handles_mixed_kinds_in_one_graph() {
    let dir = tempdir().unwrap();
    stage_module_package(dir.path(), "mod-dep");
    write(
        &dir.path().join("drv-dep/pkg.toml"),
        "[package]\nname = \"drv-dep\"\nversion = \"1.0.0\"\nkind = \"driver\"\nreovim-version = \"^0.15\"\n",
    );
    let fname = reovim_dylib_loader::cdylib_filename("drv-dep");
    let dst = dir.path().join("drv-dep/dist").join(&fname);
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::write(&dst, driver_poc_bytes()).unwrap();
    write(
        &dir.path().join("pkg.toml"),
        "[package]\nname = \"root-setup\"\nreovim-version = \"^0.15\"\n\n[dependencies]\nmod-dep = { path = \"mod-dep\" }\ndrv-dep = { path = \"drv-dep\" }\n",
    );

    let resolved = resolve(dir.path(), &Version::new(0, 15, 0)).expect("resolve");
    let lock = dir.path().join("pkg.lock");
    let lib = dir.path().join("library-root");
    let installed = install(&resolved, &lib, &lock).expect("install");
    assert_eq!(installed.len(), 2);
    let kinds: Vec<_> = installed
        .iter()
        .map(|p| (p.name.as_str(), p.kind))
        .collect();
    assert!(kinds.contains(&("drv-dep", PackageKind::Driver)));
    assert!(kinds.contains(&("mod-dep", PackageKind::Module)));
}

#[test]
fn install_reports_missing_dist_dir() {
    let dir = tempdir().unwrap();
    write(
        &dir.path().join("alpha/pkg.toml"),
        "[package]\nname = \"alpha\"\nversion = \"1.0.0\"\nkind = \"module\"\nreovim-version = \"^0.15\"\n",
    );
    // NO dist/ dir on purpose.
    stage_root_manifest(dir.path(), "alpha");

    let resolved = resolve(dir.path(), &Version::new(0, 15, 0)).expect("resolve");
    let lock = dir.path().join("pkg.lock");
    let lib = dir.path().join("library-root");
    let err = install(&resolved, &lib, &lock).expect_err("missing dist must fail");
    assert!(matches!(err, InstallError::MissingDistDir { ref pkg, .. } if pkg == "alpha"));
}

#[test]
fn install_rejects_missing_kind_in_path_manifest() {
    let dir = tempdir().unwrap();
    write(
        &dir.path().join("alpha/pkg.toml"),
        "[package]\nname = \"alpha\"\nversion = \"1.0.0\"\nreovim-version = \"^0.15\"\n",
    );
    let fname = reovim_dylib_loader::cdylib_filename("alpha");
    let dst = dir.path().join("alpha/dist").join(&fname);
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::write(&dst, driver_poc_bytes()).unwrap();
    stage_root_manifest(dir.path(), "alpha");

    let resolved = resolve(dir.path(), &Version::new(0, 15, 0)).expect("resolve");
    let lock = dir.path().join("pkg.lock");
    let lib = dir.path().join("library-root");
    let err = install(&resolved, &lib, &lock).expect_err("missing kind must fail");
    assert!(matches!(err, InstallError::MissingPackageKind { ref pkg } if pkg == "alpha"));
}

#[test]
fn install_reports_malformed_path_dep_manifest() {
    // Resolver succeeds (it parses the root manifest + path-dep
    // manifest into its own model). Installer then re-reads the
    // path-dep manifest via load_manifest; we corrupt the file
    // between the resolver call and the install call so only the
    // installer sees the parse error.
    let dir = tempdir().unwrap();
    stage_module_package(dir.path(), "alpha");
    stage_root_manifest(dir.path(), "alpha");

    let resolved = resolve(dir.path(), &Version::new(0, 15, 0)).expect("resolve");

    // Truncate the path-dep manifest to invalid TOML.
    fs::write(dir.path().join("alpha/pkg.toml"), "this is not = = toml").unwrap();

    let lock = dir.path().join("pkg.lock");
    let lib = dir.path().join("library-root");
    let err = install(&resolved, &lib, &lock).expect_err("bad manifest must fail");
    assert!(matches!(err, InstallError::Manifest { .. }), "got {err:?}");
}

#[test]
fn install_round_trip_preserves_trigger() {
    use reovim_pkg_manifest::LazyTrigger;

    let dir = tempdir().unwrap();
    stage_module_package(dir.path(), "lazy-dep");
    stage_module_package(dir.path(), "eager-dep");
    write(
        &dir.path().join("pkg.toml"),
        "[package]\n\
         name = \"root-setup\"\n\
         reovim-version = \"^0.15\"\n\
         \n\
         [dependencies]\n\
         lazy-dep = { path = \"lazy-dep\" }\n\
         eager-dep = { path = \"eager-dep\" }\n\
         \n\
         [lazy]\n\
         lazy-dep = { on-domain = \"text\" }\n\
         eager-dep = { eager = true }\n",
    );

    let resolved = resolve(dir.path(), &Version::new(0, 15, 0)).expect("resolve");
    let lock = dir.path().join("pkg.lock");
    let lib = dir.path().join("library-root");
    let installed = install(&resolved, &lib, &lock).expect("install");

    let by_name: std::collections::BTreeMap<&str, &super::InstalledPackage> =
        installed.iter().map(|p| (p.name.as_str(), p)).collect();
    assert_eq!(
        by_name["lazy-dep"].trigger.as_ref(),
        Some(&LazyTrigger::OnDomain("text".into())),
    );
    assert_eq!(by_name["eager-dep"].trigger.as_ref(), Some(&LazyTrigger::Eager));

    let listed = crate::inventory::list(&lock, &lib).expect("list");
    let listed_by_name: std::collections::BTreeMap<&str, &super::InstalledPackage> =
        listed.iter().map(|p| (p.name.as_str(), p)).collect();
    assert_eq!(
        listed_by_name["lazy-dep"].trigger.as_ref(),
        Some(&LazyTrigger::OnDomain("text".into())),
    );
    assert_eq!(listed_by_name["eager-dep"].trigger.as_ref(), Some(&LazyTrigger::Eager),);
}
