//! Fault-injection integration: stage four packages with different
//! fault classes (and one clean package), audit, repair, re-audit.

use std::{fmt::Write as _, fs, path::Path};

use {
    reovim_dylib_loader::cdylib_filename,
    reovim_pkg_install::{InstalledPackage, audit, install, list, repair},
    reovim_pkg_lockfile::Lockfile,
    reovim_pkg_resolver::resolve,
    semver::Version,
    sha2::{Digest, Sha256},
    tempfile::tempdir,
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

fn sha_of(bytes: &[u8]) -> String {
    let out = Sha256::digest(bytes);
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        write!(s, "{b:02x}").unwrap();
    }
    s
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
    let dst = root.join(name).join("dist").join(cdylib_filename(name));
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::write(&dst, driver_poc_bytes()).unwrap();
}

fn rewrite_lockfile_sha(lockfile: &Path, name: &str, new_sha: &str) {
    let raw = fs::read_to_string(lockfile).unwrap();
    let mut lock = Lockfile::from_toml_str(&raw).unwrap();
    let pkg = lock.packages.iter_mut().find(|p| p.name == name).unwrap();
    pkg.sha256 = Some(new_sha.into());
    fs::write(lockfile, lock.to_toml_string().unwrap()).unwrap();
}

fn inject_unloadable(installed: &InstalledPackage, lockfile: &Path) {
    let bytes = b"hi\n";
    fs::write(&installed.installed_path, bytes).unwrap();
    rewrite_lockfile_sha(lockfile, &installed.name, &sha_of(bytes));
}

fn inject_drift(installed: &InstalledPackage) {
    let mut bytes = fs::read(&installed.installed_path).unwrap();
    bytes.extend_from_slice(b"\n");
    fs::write(&installed.installed_path, bytes).unwrap();
}

fn inject_missing(installed: &InstalledPackage) {
    fs::remove_file(&installed.installed_path).unwrap();
}

fn inject_orphan(library_root: &Path, name: &str) {
    let dir = library_root.join("modules");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(cdylib_filename(name)), driver_poc_bytes()).unwrap();
}

#[test]
fn doctor_detects_and_repairs_every_class_simultaneously() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    let lib = root.join("library-root");
    let lockfile = root.join("pkg.lock");

    for name in ["good", "broken", "gone", "skewed"] {
        stage_module_package(root, name);
    }
    write(
        &root.join("pkg.toml"),
        "[package]\nname = \"root-setup\"\nreovim-version = \"^0.15\"\n\n\
         [dependencies]\n\
         good = { path = \"good\" }\n\
         broken = { path = \"broken\" }\n\
         gone = { path = \"gone\" }\n\
         skewed = { path = \"skewed\" }\n",
    );

    let resolved = resolve(root, &Version::new(0, 15, 0)).expect("resolve");
    let installed: Vec<InstalledPackage> = install(&resolved, &lib, &lockfile).expect("install");
    let by = |n: &str| installed.iter().find(|p| p.name == n).expect(n);

    inject_unloadable(by("broken"), &lockfile);
    inject_missing(by("gone"));
    inject_drift(by("skewed"));
    inject_orphan(&lib, "ghost");

    let report = audit(&lib, &lockfile).expect("audit");
    assert_eq!(report.unloadable.len(), 1, "{report:?}");
    assert_eq!(report.unloadable[0].name, "broken");
    assert_eq!(report.orphan.len(), 1, "{report:?}");
    assert_eq!(report.orphan[0].package_name().as_deref(), Some("ghost"));
    assert_eq!(report.missing.len(), 1, "{report:?}");
    assert_eq!(report.missing[0].name, "gone");
    assert_eq!(report.drift.len(), 1, "{report:?}");
    assert_eq!(report.drift[0].name, "skewed");
    assert_ne!(report.drift[0].expected, report.drift[0].actual);
    assert_eq!(report.total_findings(), 4);
    assert!(!report.is_clean());

    let outcome = repair(&report, &lib, &lockfile).expect("repair");
    assert_eq!(outcome.removed_orphans.len(), 1);
    assert_eq!(outcome.dropped_missing.len(), 1);
    assert!(outcome.residual.orphan.is_empty());
    assert!(outcome.residual.missing.is_empty());
    assert_eq!(outcome.residual.unloadable, report.unloadable);
    assert_eq!(outcome.residual.drift, report.drift);
    assert!(!outcome.is_clean());

    let re_audit = audit(&lib, &lockfile).expect("re-audit");
    assert!(re_audit.orphan.is_empty(), "orphan should not regress");
    assert!(re_audit.missing.is_empty(), "missing should not regress");
    assert_eq!(re_audit.unloadable.len(), 1);
    assert_eq!(re_audit.drift.len(), 1);

    let after_install_list = list(&lockfile, &lib).expect("list");
    assert!(
        after_install_list.iter().all(|p| p.name != "gone"),
        "gone entry should be dropped from inventory",
    );
    assert!(
        after_install_list.iter().all(|p| p.name != "ghost"),
        "ghost was never inventoried",
    );
}

// Backward-compat sentinel: the test above installs from a manifest with no
// `[lazy]` table, so every lockfile entry's `trigger` field is missing.
// Doctor reads the lockfile via `read_inventory`, which decodes the
// optional trigger field cleanly. If a future schema change broke that
// path, this test would fail at the audit() call.
#[test]
fn doctor_accepts_phase_2_lockfile_without_trigger_field() {
    let dir = tempdir().expect("tempdir");
    let root = dir.path();
    let lib = root.join("library-root");
    let lockfile = root.join("pkg.lock");

    stage_module_package(root, "alpha");
    write(
        &root.join("pkg.toml"),
        "[package]\nname = \"root-setup\"\nreovim-version = \"^0.15\"\n\n\
         [dependencies]\nalpha = { path = \"alpha\" }\n",
    );

    let resolved = resolve(root, &Version::new(0, 15, 0)).expect("resolve");
    install(&resolved, &lib, &lockfile).expect("install");

    let raw = fs::read_to_string(&lockfile).unwrap();
    let mut lock = Lockfile::from_toml_str(&raw).unwrap();
    for p in &mut lock.packages {
        p.trigger = None;
    }
    fs::write(&lockfile, lock.to_toml_string().unwrap()).unwrap();

    let report = audit(&lib, &lockfile).expect("audit must accept Phase-2 lockfile");
    assert!(report.is_clean(), "clean tree, no findings: {report:?}");
}
