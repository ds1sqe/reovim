//! Tests for [`super::repair`].

use std::{
    fmt::Write as _,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use {
    reovim_dylib_loader::cdylib_filename,
    reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
    reovim_pkg_manifest::{LazyTrigger, trigger_str},
    sha2::{Digest, Sha256},
    tempfile::tempdir,
};

use {super::repair, crate::error::InstallError};

use super::super::report::{
    DoctorReport, DriftFinding, MissingFinding, OrphanFinding, UnloadableFinding,
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

fn write_orphan(library_root: &Path, name: &str) -> PathBuf {
    let dir = library_root.join("driver");
    fs::create_dir_all(&dir).unwrap();
    let dst = dir.join(cdylib_filename(name));
    fs::write(&dst, driver_poc_bytes()).unwrap();
    dst
}

fn lockfile_with_one_module(lockfile: &Path, library_root: &Path, name: &str) -> PathBuf {
    let installed = library_root.join("modules").join(cdylib_filename(name));
    fs::create_dir_all(installed.parent().unwrap()).unwrap();
    fs::write(&installed, driver_poc_bytes()).unwrap();
    let lock = Lockfile {
        version: 1,
        packages: vec![PackageLock {
            name: name.into(),
            version: "1.0.0".into(),
            source: Source::LocalPath(PathBuf::from(format!("/pkgs/{name}"))),
            target: Some("x86_64-unknown-linux-gnu".into()),
            kind: Some("module".into()),
            sha256: Some(sha_of(&driver_poc_bytes())),
            trigger: Some(trigger_str(&LazyTrigger::Eager)),
            dependencies: Vec::new(),
        }],
    };
    fs::write(lockfile, lock.to_toml_string().unwrap()).unwrap();
    installed
}

fn orphan_finding(path: PathBuf) -> OrphanFinding {
    OrphanFinding {
        filename: path.file_name().unwrap().to_string_lossy().into_owned(),
        path,
    }
}

fn missing_finding(name: &str, path: PathBuf) -> MissingFinding {
    MissingFinding {
        name: name.into(),
        path,
    }
}

fn drift_finding() -> DriftFinding {
    DriftFinding {
        name: "skewed".into(),
        path: PathBuf::from("/lib/modules/libreovim_pkg_skewed.so"),
        expected: "abcd".into(),
        actual: "ef01".into(),
    }
}

fn unloadable_finding() -> UnloadableFinding {
    UnloadableFinding {
        name: "broken".into(),
        path: PathBuf::from("/lib/modules/libreovim_pkg_broken.so"),
        reason: "file too short".into(),
    }
}

#[test]
fn repair_deletes_orphan_files() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let orphan_path = write_orphan(&lib, "ghost");
    let lockfile = root.path().join("pkg.lock");
    fs::write(&lockfile, "version = 1\n").unwrap();
    let report = DoctorReport {
        orphan: vec![orphan_finding(orphan_path.clone())],
        ..DoctorReport::default()
    };

    let outcome = repair(&report, &lib, &lockfile).expect("repair");
    assert_eq!(outcome.removed_orphans.len(), 1);
    assert!(outcome.residual.orphan.is_empty());
    assert!(!orphan_path.exists());
}

#[test]
fn repair_drops_missing_inventory_entries() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let lockfile = root.path().join("pkg.lock");
    let installed = lockfile_with_one_module(&lockfile, &lib, "gone");
    fs::remove_file(&installed).unwrap();

    let report = DoctorReport {
        missing: vec![missing_finding("gone", installed)],
        ..DoctorReport::default()
    };
    let outcome = repair(&report, &lib, &lockfile).expect("repair");
    assert_eq!(outcome.dropped_missing.len(), 1);
    assert!(outcome.residual.missing.is_empty());

    let after = crate::inventory::list(&lockfile, &lib).expect("list");
    assert!(after.iter().all(|p| p.name != "gone"));
}

#[test]
fn repair_leaves_unloadable_and_drift_in_residual() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let lockfile = root.path().join("pkg.lock");
    fs::write(&lockfile, "version = 1\n").unwrap();
    let report = DoctorReport {
        unloadable: vec![unloadable_finding()],
        drift: vec![drift_finding()],
        ..DoctorReport::default()
    };

    let outcome = repair(&report, &lib, &lockfile).expect("repair");
    assert_eq!(outcome.removed_orphans.len(), 0);
    assert_eq!(outcome.dropped_missing.len(), 0);
    assert_eq!(outcome.residual.unloadable, report.unloadable);
    assert_eq!(outcome.residual.drift, report.drift);
    assert!(!outcome.is_clean());
}

#[test]
fn repair_idempotent_when_orphan_already_gone() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let lockfile = root.path().join("pkg.lock");
    fs::write(&lockfile, "version = 1\n").unwrap();
    let stale = lib.join("driver/libreovim_pkg_already_gone.so");
    let report = DoctorReport {
        orphan: vec![orphan_finding(stale)],
        ..DoctorReport::default()
    };

    let outcome = repair(&report, &lib, &lockfile).expect("repair");
    assert_eq!(outcome.removed_orphans.len(), 0);
    assert!(outcome.residual.orphan.is_empty());
}

#[test]
fn repair_idempotent_when_missing_already_dropped() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let lockfile = root.path().join("pkg.lock");
    fs::write(&lockfile, "version = 1\n").unwrap();
    let report = DoctorReport {
        missing: vec![missing_finding(
            "phantom",
            lib.join("modules/libreovim_pkg_phantom.so"),
        )],
        ..DoctorReport::default()
    };

    let outcome = repair(&report, &lib, &lockfile).expect("repair");
    assert_eq!(outcome.dropped_missing.len(), 0);
    assert!(outcome.residual.missing.is_empty());
}

#[test]
fn repair_propagates_lockfile_write_failure() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let lockfile = root.path().join("pkg.lock");
    let installed = lockfile_with_one_module(&lockfile, &lib, "gone");
    fs::remove_file(&installed).unwrap();
    let original_mode = fs::metadata(&lockfile).unwrap().permissions().mode();
    fs::set_permissions(&lockfile, fs::Permissions::from_mode(0o400)).unwrap();

    let report = DoctorReport {
        missing: vec![missing_finding("gone", installed)],
        ..DoctorReport::default()
    };
    let err = repair(&report, &lib, &lockfile).expect_err("must fail");
    assert!(matches!(err, InstallError::WriteFailed { .. }), "got {err:?}");

    fs::set_permissions(&lockfile, fs::Permissions::from_mode(original_mode)).unwrap();
}

#[test]
fn repair_propagates_orphan_delete_failure() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let dir = lib.join("driver");
    fs::create_dir_all(&dir).unwrap();
    let orphan_path = dir.join(cdylib_filename("ghost"));
    fs::write(&orphan_path, driver_poc_bytes()).unwrap();
    let lockfile = root.path().join("pkg.lock");
    fs::write(&lockfile, "version = 1\n").unwrap();
    let original_dir_mode = fs::metadata(&dir).unwrap().permissions().mode();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).unwrap();

    let report = DoctorReport {
        orphan: vec![orphan_finding(orphan_path)],
        ..DoctorReport::default()
    };
    let err = repair(&report, &lib, &lockfile).expect_err("must fail");
    assert!(matches!(err, InstallError::WriteFailed { .. }), "got {err:?}");

    fs::set_permissions(&dir, fs::Permissions::from_mode(original_dir_mode)).unwrap();
}

#[test]
fn repair_with_clean_report_is_noop() {
    let root = tempdir().expect("tempdir");
    let lib = root.path().join("library-root");
    let lockfile = root.path().join("pkg.lock");
    fs::write(&lockfile, "version = 1\n").unwrap();
    let outcome = repair(&DoctorReport::default(), &lib, &lockfile).expect("repair");
    assert_eq!(outcome.removed_orphans.len(), 0);
    assert_eq!(outcome.dropped_missing.len(), 0);
    assert!(outcome.is_clean());
}
