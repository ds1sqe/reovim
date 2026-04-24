//! Per-entry failure and non-fatal-contract tests for `scan_paths`.
//!
//! The contract: `scan_paths` is infallible at the API level. A
//! malformed cdylib, an unreadable file, or a missing directory never
//! prevents other cdylibs from being surfaced. Each failure becomes a
//! structured [`ScanEntryError`] on the individual entry.
//!
//! Exercises the four failure modes called out in the plan:
//! valid-opens-success, non-library-skipped, malformed-bytes-reported,
//! permission-denied-reported, missing-directory-warns-and-skips.

#![allow(unsafe_code)]

use {
    reovim_dylib_loader::{ScanEntryError, library_filename, scan_paths},
    std::{env, fs, path::PathBuf},
};

fn poc_cdylib_source() -> PathBuf {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let workspace = PathBuf::from(&manifest)
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf();
    let target =
        env::var("CARGO_TARGET_DIR").map_or_else(|_| workspace.join("target"), PathBuf::from);
    target
        .join("debug")
        .join(library_filename("reovim_driver_abi_poc"))
}

#[test]
fn scan_over_empty_directory_returns_empty_report() {
    let dir = tempfile::tempdir().unwrap();
    let report = scan_paths(&[dir.path().to_path_buf()]);
    assert!(report.is_empty(), "expected empty; got {} entries", report.len());
}

#[test]
fn scan_over_missing_directory_returns_empty_report_without_panic() {
    let report = scan_paths(&[PathBuf::from("/this/path/does/not/exist/anywhere")]);
    assert!(report.is_empty());
}

#[test]
fn scan_skips_non_library_extensions_silently() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("README.md"), b"not a cdylib").unwrap();
    fs::write(dir.path().join("notes.txt"), b"also not a cdylib").unwrap();

    let report = scan_paths(&[dir.path().to_path_buf()]);
    assert!(report.is_empty(), "non-library files must be silently skipped");
}

#[test]
fn scan_opens_valid_cdylib_and_reports_success() {
    let src = poc_cdylib_source();
    assert!(
        src.exists(),
        "`PoC` cdylib not found at {}; run `cargo build -p reovim-driver-abi-poc` first",
        src.display(),
    );

    let dir = tempfile::tempdir().unwrap();
    let staged = dir.path().join(src.file_name().unwrap());
    fs::copy(&src, &staged).unwrap();

    let report = scan_paths(&[dir.path().to_path_buf()]);
    assert_eq!(report.len(), 1);
    let entry = &report.entries()[0];
    assert_eq!(entry.path, staged);
    assert!(
        entry.outcome.is_ok(),
        "expected Ok; got {:?}",
        entry.outcome.as_ref().map(|_| ()),
    );
}

#[test]
fn scan_reports_not_a_library_for_garbage_bytes_with_cdylib_extension() {
    let dir = tempfile::tempdir().unwrap();
    let garbage = dir.path().join(library_filename("garbage"));
    fs::write(&garbage, [0u8, 1, 2, 3]).unwrap();

    let report = scan_paths(&[dir.path().to_path_buf()]);
    assert_eq!(report.len(), 1);
    match &report.entries()[0].outcome {
        Err(
            ScanEntryError::NotALibrary { path, .. } | ScanEntryError::DlopenFailed { path, .. },
        ) => {
            assert_eq!(path, &garbage);
        }
        other => panic!("expected NotALibrary/DlopenFailed, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn scan_reports_io_error_for_permission_denied_cdylib() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join(library_filename("locked"));
    fs::write(&locked, [0u8, 1, 2, 3]).unwrap();
    // TODO(#769-O4): permission-based failure classification is
    // unix-specific; the Windows CI job in Phase 1.F validates the
    // cross-platform equivalent.
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o0)).unwrap();

    let report = scan_paths(&[dir.path().to_path_buf()]);
    assert_eq!(report.len(), 1);
    match &report.entries()[0].outcome {
        Err(ScanEntryError::Io { path, .. } | ScanEntryError::DlopenFailed { path, .. }) => {
            assert_eq!(path, &locked);
        }
        other => panic!("expected Io/DlopenFailed, got {other:?}"),
    }

    // Restore permissions so tempdir cleanup works.
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();
}

#[test]
fn scan_mixed_directory_surfaces_both_successes_and_failures() {
    let src = poc_cdylib_source();
    if !src.exists() {
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    fs::copy(&src, dir.path().join(src.file_name().unwrap())).unwrap();
    fs::write(dir.path().join(library_filename("garbage")), [0u8, 1, 2]).unwrap();
    fs::write(dir.path().join("README.md"), b"skip me").unwrap();

    let report = scan_paths(&[dir.path().to_path_buf()]);
    assert_eq!(report.len(), 2, "expected 2 cdylib candidates (README skipped)");

    let (oks, errs): (Vec<_>, Vec<_>) = report.entries().iter().partition(|e| e.outcome.is_ok());
    assert_eq!(oks.len(), 1, "expected 1 successful open");
    assert_eq!(errs.len(), 1, "expected 1 error");
}

#[test]
#[tracing_test::traced_test]
fn scan_warns_once_for_missing_directory() {
    let _ = scan_paths(&[PathBuf::from("/definitely-does-not-exist-12345")]);
    assert!(
        logs_contain("skipping inaccessible directory"),
        "expected warn!() for missing directory"
    );
}
