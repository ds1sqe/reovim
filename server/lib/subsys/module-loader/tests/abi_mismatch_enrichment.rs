//! Integration tests for the wave-3a server-side ABI-mismatch
//! enrichment path.
//!
//! Builds on the `reovim-pkg-wrong-api-module` fixture: a cdylib whose
//! exported `REOVIM_MODULE_API_VERSION` reports `major = u32::MAX`.
//! When the cdylib is staged under `<root>/modules/` with a
//! convention-matching filename
//! (`libreovim_pkg_wrong_api_module.<ext>`),
//! [`ModuleLoader::from_path_scan_filtered_diag`] enriches the
//! per-entry `Err(IncompatibleVersion {..})` into
//! `LoadDiagnostic::AbiMismatchAtPackage` carrying
//! `package == "wrong-api-module"`. When the same cdylib is staged
//! under a non-convention filename, the original opaque
//! `ModuleError::IncompatibleVersion` survives as
//! `LoadDiagnostic::Bare(_)`.

#![allow(unsafe_code)]

use std::{env, fs, path::PathBuf};

use {
    reovim_kernel::api::v1::ModuleError,
    reovim_pkg_lazyload::LazyRegistry,
    reovim_subsys_module_loader::{diagnostic::LoadDiagnostic, loader::ModuleLoader},
};

fn workspace_target_dir() -> PathBuf {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let workspace = PathBuf::from(&manifest)
        .ancestors()
        .nth(4)
        .expect("workspace root")
        .to_path_buf();
    workspace.join("target").join("debug")
}

fn cdylib_filename(stem: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
}

fn wrong_api_cdylib_source() -> Option<PathBuf> {
    let path = workspace_target_dir().join(cdylib_filename("reovim_pkg_wrong_api_module"));
    if path.exists() { Some(path) } else { None }
}

macro_rules! require_fixture {
    () => {
        match wrong_api_cdylib_source() {
            Some(path) => path,
            None => {
                eprintln!(
                    "SKIP: wrong-api-module cdylib not found. \
                     Run: cargo build -p reovim-pkg-wrong-api-module"
                );
                return;
            }
        }
    };
}

#[test]
fn convention_filename_diag_enriches_incompatible_version() {
    let src = require_fixture!();
    let root = tempfile::tempdir().unwrap();
    let modules_dir = root.path().join("modules");
    fs::create_dir_all(&modules_dir).unwrap();
    let dest = modules_dir.join(src.file_name().unwrap());
    fs::copy(&src, &dest).expect("stage convention-named wrong-api cdylib");

    let mut loader = ModuleLoader::new();
    let registry = LazyRegistry::empty();
    // SAFETY: the fixture is a workspace-built cdylib; the staged
    // copy is bit-identical to the source.
    let results = unsafe { loader.from_path_scan_filtered_diag(root.path(), &registry) };
    assert_eq!(results.len(), 1, "expected 1 entry, got {}", results.len());
    let err = results
        .into_iter()
        .next()
        .unwrap()
        .expect_err("expected per-entry AbiMismatchAtPackage error");
    match err {
        LoadDiagnostic::AbiMismatchAtPackage { package, source } => {
            assert_eq!(package, "wrong-api-module");
            assert!(matches!(source, ModuleError::IncompatibleVersion { .. }));
            let display = LoadDiagnostic::AbiMismatchAtPackage { package, source }.to_string();
            assert!(display.contains("wrong-api-module"), "missing pkg: {display}");
            assert!(display.contains("4294967295"), "missing inner version text: {display}");
        }
        bare @ LoadDiagnostic::Bare(_) => {
            panic!("expected AbiMismatchAtPackage, got {bare:?}")
        }
    }
}

#[test]
fn non_convention_filename_diag_passes_through_as_bare() {
    let src = require_fixture!();
    let root = tempfile::tempdir().unwrap();
    let modules_dir = root.path().join("modules");
    fs::create_dir_all(&modules_dir).unwrap();
    let dest = modules_dir.join(cdylib_filename("some_arbitrary_name"));
    fs::copy(&src, &dest).expect("stage non-convention wrong-api cdylib");

    let mut loader = ModuleLoader::new();
    let registry = LazyRegistry::empty();
    // SAFETY: see above.
    let results = unsafe { loader.from_path_scan_filtered_diag(root.path(), &registry) };
    assert_eq!(results.len(), 1);
    let err = results
        .into_iter()
        .next()
        .unwrap()
        .expect_err("expected per-entry Bare error");
    match err {
        LoadDiagnostic::Bare(ModuleError::IncompatibleVersion { .. }) => (),
        other => panic!("expected Bare(IncompatibleVersion), got {other:?}"),
    }
}
