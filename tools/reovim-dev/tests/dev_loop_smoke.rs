//! Linux-only end-to-end smoke test for the dev loop.
//!
//! Stages a synthetic workspace that contains a Phase 0 `PoC` cdylib
//! under `target/debug/`, runs `stage_all` to symlink it into
//! `target/reovim-dev/driver/`, then runs `scan_staging` and asserts
//! the cdylib is present. Exercises the glue between the stage
//! subcommand and the scan subcommand end-to-end; unit tests already
//! cover the individual pieces.
//!
//! macOS / Windows dev-loop parity is deferred under `TODO(#769-O4)`
//! for Phase 2a's `/e2e` harness update.

#![cfg(unix)]

use {
    reovim_dev::stage::{scan_staging, stage_all},
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
        .join(reovim_dylib_loader::library_filename("reovim_driver_abi_poc"))
}

#[test]
fn stage_then_scan_surfaces_the_poc_cdylib() {
    let src = poc_cdylib_source();
    assert!(
        src.exists(),
        "`PoC` cdylib not found at {}; run `cargo build -p reovim-driver-abi-poc` first",
        src.display()
    );

    // Build a synthetic workspace. target/debug/ holds a copy of the
    // PoC with a driver-prefixed filename so stage_all routes it.
    let ws = tempfile::tempdir().unwrap();
    let debug = ws.path().join("target").join("debug");
    fs::create_dir_all(&debug).unwrap();
    let staged_src = debug.join(reovim_dylib_loader::library_filename("reovim_driver_abi_poc"));
    fs::copy(&src, &staged_src).unwrap();

    let report = stage_all(ws.path()).expect("stage_all");
    assert_eq!(report.staged_count(), 1, "expected exactly 1 cdylib staged");

    let scan = scan_staging(ws.path());
    assert_eq!(scan.driver.len(), 1, "driver/ should contain 1 entry");
    assert!(scan.driver[0].opened, "PoC open failed: {:?}", scan.driver[0].error);
    assert!(scan.modules.is_empty(), "modules/ should be empty");
}
