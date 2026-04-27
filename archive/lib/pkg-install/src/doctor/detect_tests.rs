//! Tests for [`super::audit`] and the per-class detectors.

use std::{
    fs,
    path::{Path, PathBuf},
};

use {
    reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
    reovim_pkg_manifest::{LazyTrigger, PackageKind, trigger_str},
    tempfile::tempdir,
};

use {crate::error::InstallError, reovim_dylib_loader::cdylib_filename};

use super::audit;

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

fn write_cdylib(library_root: &Path, kind: PackageKind, name: &str, bytes: &[u8]) -> PathBuf {
    let sub = match kind {
        PackageKind::Driver => "driver",
        PackageKind::Module => "modules",
    };
    let dir = library_root.join(sub);
    fs::create_dir_all(&dir).unwrap();
    let dst = dir.join(cdylib_filename(name));
    fs::write(&dst, bytes).unwrap();
    dst
}

fn sha_of(bytes: &[u8]) -> String {
    use std::fmt::Write;

    use sha2::{Digest, Sha256};
    let out = Sha256::digest(bytes);
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        write!(s, "{b:02x}").expect("write hex");
    }
    s
}

fn lock_entry(name: &str, kind: PackageKind, sha: &str) -> PackageLock {
    PackageLock {
        name: name.into(),
        version: "1.0.0".into(),
        source: Source::LocalPath(PathBuf::from(format!("/pkgs/{name}"))),
        target: Some("x86_64-unknown-linux-gnu".into()),
        kind: Some(
            (match kind {
                PackageKind::Driver => "driver",
                PackageKind::Module => "module",
            })
            .into(),
        ),
        sha256: Some(sha.into()),
        trigger: Some(trigger_str(&LazyTrigger::Eager)),
        dependencies: Vec::new(),
    }
}

fn write_lockfile(lockfile: &Path, entries: Vec<PackageLock>) {
    let lock = Lockfile {
        version: 1,
        packages: entries,
    };
    fs::write(lockfile, lock.to_toml_string().unwrap()).unwrap();
}

#[test]
fn audit_clean_root_returns_empty_report() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    fs::create_dir_all(lib.join("modules")).unwrap();
    let lockfile = root.path().join("pkg.lock");
    write_lockfile(&lockfile, vec![]);

    let report = audit(&lib, &lockfile).expect("audit");
    assert!(report.is_clean(), "{report:?}");
}

#[test]
fn audit_missing_lockfile_yields_orphans_only() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let bytes = driver_poc_bytes();
    write_cdylib(&lib, PackageKind::Driver, "alpha", &bytes);
    write_cdylib(&lib, PackageKind::Module, "beta", &bytes);
    let lockfile = root.path().join("pkg.lock");

    let report = audit(&lib, &lockfile).expect("audit");
    assert_eq!(report.orphan.len(), 2);
    assert!(report.unloadable.is_empty());
    assert!(report.missing.is_empty());
    assert!(report.drift.is_empty());
}

#[test]
fn audit_with_missing_cdylib_emits_missing_finding() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let bytes = driver_poc_bytes();
    let path = write_cdylib(&lib, PackageKind::Module, "gone", &bytes);
    let lockfile = root.path().join("pkg.lock");
    write_lockfile(&lockfile, vec![lock_entry("gone", PackageKind::Module, &sha_of(&bytes))]);
    fs::remove_file(&path).unwrap();

    let report = audit(&lib, &lockfile).expect("audit");
    assert_eq!(report.missing.len(), 1);
    assert_eq!(report.missing[0].name, "gone");
    assert!(report.drift.is_empty());
    assert!(report.unloadable.is_empty());
}

#[test]
fn audit_with_drift_emits_drift_finding() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let bytes = driver_poc_bytes();
    write_cdylib(&lib, PackageKind::Module, "skewed", &bytes);
    let recorded = "0".repeat(64);
    let lockfile = root.path().join("pkg.lock");
    write_lockfile(&lockfile, vec![lock_entry("skewed", PackageKind::Module, &recorded)]);

    let report = audit(&lib, &lockfile).expect("audit");
    assert_eq!(report.drift.len(), 1);
    assert_eq!(report.drift[0].name, "skewed");
    assert_eq!(report.drift[0].expected, recorded);
    assert_ne!(report.drift[0].actual, recorded);
}

#[test]
fn audit_with_unloadable_cdylib_emits_unloadable_finding() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let garbage = b"hi\n";
    write_cdylib(&lib, PackageKind::Module, "broken", garbage);
    let recorded = sha_of(garbage);
    let lockfile = root.path().join("pkg.lock");
    write_lockfile(&lockfile, vec![lock_entry("broken", PackageKind::Module, &recorded)]);

    let report = audit(&lib, &lockfile).expect("audit");
    assert_eq!(report.unloadable.len(), 1);
    assert_eq!(report.unloadable[0].name, "broken");
    assert!(!report.unloadable[0].reason.is_empty());
    assert!(report.drift.is_empty(), "drift should not double-fire: {report:?}");
}

#[test]
fn audit_with_orphan_emits_orphan_finding() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let bytes = driver_poc_bytes();
    write_cdylib(&lib, PackageKind::Driver, "ghost", &bytes);
    let lockfile = root.path().join("pkg.lock");
    write_lockfile(&lockfile, vec![]);

    let report = audit(&lib, &lockfile).expect("audit");
    assert_eq!(report.orphan.len(), 1);
    assert_eq!(report.orphan[0].package_name().as_deref(), Some("ghost"));
}

#[test]
fn audit_with_all_four_classes_emits_all_four_vectors() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let bytes = driver_poc_bytes();

    let _good = write_cdylib(&lib, PackageKind::Module, "good", &bytes);
    let _broken = write_cdylib(&lib, PackageKind::Module, "broken", b"hi\n");
    let gone_path = write_cdylib(&lib, PackageKind::Module, "gone", &bytes);
    let skewed_path = write_cdylib(&lib, PackageKind::Module, "skewed", &bytes);
    write_cdylib(&lib, PackageKind::Driver, "ghost", &bytes);

    let lockfile = root.path().join("pkg.lock");
    write_lockfile(
        &lockfile,
        vec![
            lock_entry("good", PackageKind::Module, &sha_of(&bytes)),
            lock_entry("broken", PackageKind::Module, &sha_of(b"hi\n")),
            lock_entry("gone", PackageKind::Module, &sha_of(&bytes)),
            lock_entry("skewed", PackageKind::Module, &"0".repeat(64)),
        ],
    );
    fs::remove_file(&gone_path).unwrap();
    let _ = skewed_path;

    let report = audit(&lib, &lockfile).expect("audit");
    assert_eq!(report.unloadable.len(), 1);
    assert_eq!(report.orphan.len(), 1);
    assert_eq!(report.missing.len(), 1);
    assert_eq!(report.drift.len(), 1);
    assert_eq!(report.total_findings(), 4);
    assert!(!report.is_clean());
}

#[test]
fn audit_findings_are_sorted_by_name() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let bytes = driver_poc_bytes();

    for name in ["zeta", "alpha", "mu"] {
        let path = write_cdylib(&lib, PackageKind::Module, name, &bytes);
        fs::remove_file(&path).unwrap();
    }
    let lockfile = root.path().join("pkg.lock");
    write_lockfile(
        &lockfile,
        vec![
            lock_entry("zeta", PackageKind::Module, &sha_of(&bytes)),
            lock_entry("alpha", PackageKind::Module, &sha_of(&bytes)),
            lock_entry("mu", PackageKind::Module, &sha_of(&bytes)),
        ],
    );

    let report = audit(&lib, &lockfile).expect("audit");
    let names: Vec<&str> = report.missing.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "mu", "zeta"]);
}

#[test]
fn audit_lockfile_parse_failure_bubbles_up() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    fs::create_dir_all(&lib).unwrap();
    let lockfile = root.path().join("pkg.lock");
    fs::write(&lockfile, "this is = = not toml").unwrap();

    let err = audit(&lib, &lockfile).expect_err("must fail");
    assert!(matches!(err, InstallError::Lockfile(_)), "got {err:?}");
}
