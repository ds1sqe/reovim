//! Tests for [`super::LoaderHandle`].
//!
//! `load_named` is `unsafe` and resolves a real cdylib path; the tests
//! mirror the integration-test pattern of staging the workspace
//! `reovim-test-dynamic-module` fixture under the package-manager
//! filename convention. When the fixture is missing the test logs a
//! SKIP message and returns early — same shape as `tests/integration.rs`.

use std::path::PathBuf;

use reovim_dylib_loader::Kind;

use {super::LoaderHandle, crate::loader::ModuleLoader};

fn fixture_so() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent()?.parent()?.parent()?.parent()?;
    let so = workspace_root
        .join("target")
        .join("debug")
        .join("libreovim_test_dynamic_module.so");
    so.exists().then_some(so)
}

fn stage_fixture(pkg_name: &str) -> Option<(tempfile::TempDir, String)> {
    let so = fixture_so()?;
    let tmp = tempfile::tempdir().ok()?;
    let dir = tmp.path().join(Kind::Module.subdir());
    std::fs::create_dir_all(&dir).ok()?;
    let staged = dir.join(reovim_dylib_loader::cdylib_filename(pkg_name));
    std::fs::copy(&so, &staged).ok()?;
    Some((tmp, pkg_name.to_owned()))
}

macro_rules! require_fixture {
    ($pkg:expr) => {
        match stage_fixture($pkg) {
            Some(staged) => staged,
            None => {
                eprintln!(
                    "SKIP: test fixture .so missing; run `cargo build -p reovim-test-dynamic-module`"
                );
                return;
            }
        }
    };
}

#[test]
fn load_named_dlopens_at_canonical_path() {
    let (tmp, name) = require_fixture!("demo");
    let handle = LoaderHandle::new(ModuleLoader::new());

    #[allow(unsafe_code)]
    let id =
        unsafe { handle.load_named(&name, Kind::Module, tmp.path()) }.expect("dlopen succeeds");
    assert_eq!(id.as_str(), "test-dynamic");
}

#[test]
fn load_named_returns_load_failed_for_missing_cdylib() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join(Kind::Module.subdir())).expect("modules dir");
    let handle = LoaderHandle::new(ModuleLoader::new());

    #[allow(unsafe_code)]
    let err = unsafe { handle.load_named("absent", Kind::Module, tmp.path()) }
        .expect_err("missing cdylib must error");
    let msg = err.to_string();
    assert!(
        msg.contains("absent") || msg.contains("dlopen") || msg.contains("No such file"),
        "error should mention the missing path or dlopen failure: {msg}"
    );
}

#[test]
fn load_named_rejects_duplicate_load() {
    let (tmp, name) = require_fixture!("demo");
    let handle = LoaderHandle::new(ModuleLoader::new());

    #[allow(unsafe_code)]
    let _first = unsafe { handle.load_named(&name, Kind::Module, tmp.path()) }.expect("first load");
    #[allow(unsafe_code)]
    let err = unsafe { handle.load_named(&name, Kind::Module, tmp.path()) }
        .expect_err("duplicate must error");
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("already") || msg.to_lowercase().contains("duplicate"),
        "duplicate-load error should mention the cause: {msg}"
    );
}

#[test]
fn from_arc_shares_underlying_loader() {
    let inner = std::sync::Arc::new(parking_lot::Mutex::new(ModuleLoader::new()));
    let a = LoaderHandle::from_arc(std::sync::Arc::clone(&inner));
    let b = LoaderHandle::from_arc(inner);

    let (tmp, name) = require_fixture!("shared");
    #[allow(unsafe_code)]
    let _id = unsafe { a.load_named(&name, Kind::Module, tmp.path()) }.expect("load via handle a");
    #[allow(unsafe_code)]
    let err = unsafe { b.load_named(&name, Kind::Module, tmp.path()) }
        .expect_err("handle b sees the prior load");
    assert!(err.to_string().to_lowercase().contains("already"));
}
