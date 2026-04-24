//! Integration test: open a Phase 0 `PoC` cdylib and resolve a symbol.
//!
//! Proves the safe-wrapper `Library::open` + `Library::symbol` pair
//! round-trip against a real cdylib on the current target OS. The test
//! exercises the happy path only; malformed and missing-symbol cases
//! are covered by `clients/lib/subsys/driver-loader/tests/` (Phase 0)
//! and `scan_nonfatal.rs` (Phase 1.C).

#![allow(unsafe_code)]

use {
    reovim_dylib_loader::{Library, library_filename},
    std::{env, path::PathBuf},
};

fn poc_cdylib_path() -> PathBuf {
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
fn open_and_resolve_vtable_symbol() {
    let path = poc_cdylib_path();
    assert!(
        path.exists(),
        "PoC cdylib not found at {}; run `cargo build -p reovim-driver-abi-poc` first",
        path.display()
    );

    let lib = Library::open(&path).expect("Library::open on PoC cdylib");

    // SAFETY: the PoC crate's `declare_client_render_driver!` macro
    // exports this symbol as a `*const ClientRenderVTable`; resolving
    // it as an opaque pointer here is correct — this test only
    // exercises the resolver, not the ABI layout.
    let result: Result<reovim_dylib_loader::Symbol<'_, *const core::ffi::c_void>, _> =
        unsafe { lib.symbol(b"REOVIM_CLIENT_RENDER_DRIVER_VTABLE") };
    assert!(result.is_ok(), "symbol resolution failed: {result:?}");
}

#[test]
fn symbol_not_found_surfaces_structured_error() {
    let path = poc_cdylib_path();
    if !path.exists() {
        // Build gate handled by the earlier test; skip here.
        return;
    }
    let lib = Library::open(&path).expect("Library::open on PoC cdylib");

    // SAFETY: the symbol does not exist; we assert the error path.
    // The returned Symbol is never dereferenced because resolution
    // fails.
    let result: Result<reovim_dylib_loader::Symbol<'_, *const core::ffi::c_void>, _> =
        unsafe { lib.symbol(b"NOT_A_REAL_SYMBOL_XYZ") };
    match result {
        Err(reovim_dylib_loader::LoaderError::SymbolNotFound { symbol, .. }) => {
            assert_eq!(symbol, "NOT_A_REAL_SYMBOL_XYZ");
        }
        other => panic!("expected SymbolNotFound, got {other:?}"),
    }
}

#[test]
fn library_open_missing_file_surfaces_structured_error() {
    let path = PathBuf::from("/nonexistent/libdefinitely_not_a_real_cdylib.so");
    match Library::open(&path) {
        Err(reovim_dylib_loader::LoaderError::LibraryOpen { path: p, .. }) => {
            assert_eq!(p, path);
        }
        other => panic!("expected LibraryOpen error, got {other:?}"),
    }
}
