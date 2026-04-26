//! Integration tests for the wave-3a ABI-mismatch enrichment path.
//!
//! Builds on the `reovim-pkg-wrong-abi-poc` fixture: a cdylib whose
//! exported `ClientRenderVTable.abi_version` is `u32::MAX`. When the
//! cdylib is staged under `<root>/driver/` with a convention-matching
//! filename (`libreovim_pkg_wrong_abi_poc.<ext>`), the loader's
//! enrichment lifts the `ValidationError::AbiVersionMismatch` into
//! `LoadError::AbiMismatchAtPackage` / `ScanEntryError::AbiMismatchAtPackage`
//! carrying `package == "wrong-abi-poc"`. When the same cdylib is
//! staged under a non-convention filename, the original opaque
//! `ValidationError` survives.

#![allow(unsafe_code)]

use {
    reovim_client_subsys_driver_loader::{
        LoadError, LoadedClientRender, ScanEntryError, ValidationError,
    },
    std::{env, fs, path::PathBuf},
};

fn workspace_target_dir() -> PathBuf {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let target_dir = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        let workspace = PathBuf::from(&manifest)
            .ancestors()
            .nth(4)
            .expect("workspace root")
            .to_path_buf();
        workspace.join("target").display().to_string()
    });
    PathBuf::from(target_dir).join("debug")
}

fn cdylib_filename(crate_underscore_name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{crate_underscore_name}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{crate_underscore_name}.dylib")
    } else {
        format!("lib{crate_underscore_name}.so")
    }
}

fn wrong_abi_cdylib_source() -> PathBuf {
    let path = workspace_target_dir().join(cdylib_filename("reovim_pkg_wrong_abi_poc"));
    assert!(
        path.exists(),
        "wrong-abi cdylib not found at {}; run `cargo build -p reovim-pkg-wrong-abi-poc` first",
        path.display()
    );
    path
}

#[test]
fn convention_filename_load_from_path_enriches_validation_error() {
    let path = wrong_abi_cdylib_source();
    let result = LoadedClientRender::load_from_path(&path);
    let Err(err) = result else {
        panic!("expected ABI mismatch, got Ok")
    };
    match err {
        LoadError::AbiMismatchAtPackage { package, source } => {
            assert_eq!(package, "wrong-abi-poc");
            assert!(matches!(source, ValidationError::AbiVersionMismatch { .. }));
            let display = LoadError::AbiMismatchAtPackage { package, source }.to_string();
            assert!(display.contains("wrong-abi-poc"), "missing pkg name: {display}");
            assert!(display.contains("ABI version mismatch"), "missing inner: {display}");
        }
        other => panic!("expected AbiMismatchAtPackage, got {other:?}"),
    }
}

#[test]
fn convention_filename_from_path_scan_enriches_validation_error() {
    let src = wrong_abi_cdylib_source();
    let root = tempfile::tempdir().unwrap();
    let driver_dir = root.path().join("driver");
    fs::create_dir_all(&driver_dir).unwrap();
    let dest = driver_dir.join(src.file_name().unwrap());
    fs::copy(&src, &dest).expect("stage convention-named wrong-abi cdylib");

    let results = LoadedClientRender::from_path_scan(root.path());
    assert_eq!(results.len(), 1, "expected 1 entry, got {}", results.len());
    let err = results
        .into_iter()
        .next()
        .unwrap()
        .err()
        .expect("expected per-entry AbiMismatchAtPackage error");
    match err {
        ScanEntryError::AbiMismatchAtPackage { package, source } => {
            assert_eq!(package, "wrong-abi-poc");
            assert!(matches!(source, ValidationError::AbiVersionMismatch { .. }));
        }
        other => panic!("expected AbiMismatchAtPackage, got {other:?}"),
    }
}

#[test]
fn non_convention_filename_load_from_path_passes_validation_through() {
    let src = wrong_abi_cdylib_source();
    let staging = tempfile::tempdir().unwrap();
    let dest = staging.path().join(cdylib_filename("some_arbitrary_name"));
    fs::copy(&src, &dest).expect("stage non-convention wrong-abi cdylib");

    let Err(err) = LoadedClientRender::load_from_path(&dest) else {
        panic!("expected ABI mismatch, got Ok")
    };
    match err {
        LoadError::Validation(ValidationError::AbiVersionMismatch { .. }) => (),
        other => panic!("expected Validation(AbiVersionMismatch), got {other:?}"),
    }
}

#[test]
fn non_convention_filename_from_path_scan_passes_validation_through() {
    let src = wrong_abi_cdylib_source();
    let root = tempfile::tempdir().unwrap();
    let driver_dir = root.path().join("driver");
    fs::create_dir_all(&driver_dir).unwrap();
    let dest = driver_dir.join(cdylib_filename("some_arbitrary_name"));
    fs::copy(&src, &dest).expect("stage non-convention wrong-abi cdylib");

    let results = LoadedClientRender::from_path_scan(root.path());
    assert_eq!(results.len(), 1);
    let err = results
        .into_iter()
        .next()
        .unwrap()
        .err()
        .expect("expected per-entry AbiMismatch error");
    match err {
        ScanEntryError::AbiMismatch(ValidationError::AbiVersionMismatch { .. }) => (),
        other => panic!("expected AbiMismatch(AbiVersionMismatch), got {other:?}"),
    }
}
