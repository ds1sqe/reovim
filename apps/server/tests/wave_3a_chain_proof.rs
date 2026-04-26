//! Wave-3a chain-acceptance matrix test (server side).
//!
//! Drives one `tempfile::TempDir` library root through chain bullets
//! (a)/(b)/(c-α)/(d) sequentially. (c-α) means the dispatcher
//! composition path is exercised directly: registry → dispatcher →
//! `LoaderHandle` → dlopen. Bootstrap-level *registration* of the
//! dispatcher on the live `CommandNameIndex` is a separate concern;
//! it is not exercised here.
//!
//! Test placement is `apps/server/tests/` because
//! [`LazyCommandDispatcher`] lives in `apps/server/src/`. A test
//! under `server/lib/subsys/module-loader/tests/` would have to
//! depend upward on `apps/server`, which violates the layer model.
//!
//! [`LazyCommandDispatcher`]: reovim_app_server::lazy_command_dispatch::LazyCommandDispatcher

#![allow(unsafe_code)]

use std::{env, fs, path::PathBuf, sync::Arc};

use {
    parking_lot::Mutex,
    reovim_app_server::lazy_command_dispatch::LazyCommandDispatcher,
    reovim_dylib_loader::{Kind, cdylib_filename},
    reovim_kernel::api::v1::ModuleId,
    reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
    reovim_pkg_runtime_loader::load_registry,
    reovim_subsys_command::CommandResolutionListener,
    reovim_subsys_module_loader::{
        diagnostic::LoadDiagnostic, loader::ModuleLoader, loader_handle::LoaderHandle,
    },
};

fn workspace_target_dir() -> PathBuf {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let target_dir = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        let workspace = PathBuf::from(&manifest)
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .to_path_buf();
        workspace.join("target").display().to_string()
    });
    PathBuf::from(target_dir).join("debug")
}

fn fixture_so(stem: &str) -> Option<PathBuf> {
    let path = workspace_target_dir().join(cdylib_filename_raw(stem));
    path.exists().then_some(path)
}

fn cdylib_filename_raw(crate_underscore_name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{crate_underscore_name}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{crate_underscore_name}.dylib")
    } else {
        format!("lib{crate_underscore_name}.so")
    }
}

fn pkg(name: &str, trigger: &str) -> PackageLock {
    PackageLock {
        name: name.into(),
        version: "1.0.0".into(),
        source: Source::LocalPath(format!("/pkgs/{name}").into()),
        target: None,
        kind: Some("module".into()),
        sha256: None,
        trigger: Some(trigger.into()),
        dependencies: Vec::new(),
    }
}

fn write_lockfile(root: &std::path::Path, packages: Vec<PackageLock>) {
    let lockfile = Lockfile {
        version: 1,
        packages,
    };
    let toml = lockfile.to_toml_string().expect("serialize lockfile");
    fs::write(root.join("pkg.lock"), toml).expect("write pkg.lock");
}

#[test]
fn wave_3a_chain_proof_server_side() {
    // ─── Setup: locate fixture cdylibs (skip cleanly if absent) ───
    let Some(eager_src) = fixture_so("reovim_module_sample") else {
        eprintln!("SKIP: sample-module cdylib missing — run `cargo build -p reovim-module-sample`");
        return;
    };
    let Some(lazy_src) = fixture_so("reovim_test_dynamic_module") else {
        eprintln!(
            "SKIP: test-dynamic-module cdylib missing — \
             run `cargo build -p reovim-test-dynamic-module`"
        );
        return;
    };
    let Some(bad_abi_src) = fixture_so("reovim_pkg_wrong_api_module") else {
        eprintln!(
            "SKIP: wrong-api-module cdylib missing — \
             run `cargo build -p reovim-pkg-wrong-api-module`"
        );
        return;
    };

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    let modules_dir = root.join(Kind::Module.subdir());
    fs::create_dir_all(&modules_dir).expect("modules dir");

    // Stage each fixture under the package-name convention so
    // `pkg_name_from_cdylib_filename` recovers the intended name.
    fs::copy(&eager_src, modules_dir.join(cdylib_filename("eager"))).expect("stage eager");
    fs::copy(&lazy_src, modules_dir.join(cdylib_filename("lazy-event"))).expect("stage lazy");
    fs::copy(&bad_abi_src, modules_dir.join(cdylib_filename("wrong-api-module")))
        .expect("stage wrong-api-module");

    write_lockfile(
        root,
        vec![
            pkg("eager", "eager"),
            pkg("lazy-event", "on-event:save"),
            pkg("wrong-api-module", "eager"),
        ],
    );

    // ─── (a) — sub-plan 01: lockfile read ─────────────────────────
    // The runtime registry reflects the on-disk lockfile and
    // classifies each package's trigger.
    let registry = load_registry(root).expect("load registry");
    assert!(registry.is_lazy("lazy-event"), "(a) lazy-event lazy");
    assert!(!registry.is_lazy("eager"), "(a) eager not lazy");
    assert!(!registry.is_lazy("wrong-api-module"), "(a) wrong-api-module not lazy");

    // ─── (b) — sub-plan 02: eager-only at startup + ───────────────
    // ─── (d) — sub-plan 04: enriched ABI diagnostic ───────────────
    // Both bullets fall out of one `from_path_scan_filtered_diag`
    // call: the lazy entry is filtered before dlopen, the eager
    // entry loads, and the bad-ABI entry surfaces with the package
    // name attached.
    let mut loader = ModuleLoader::new();
    // SAFETY: every staged cdylib is a workspace fixture build; the
    // bytes are bit-identical to the source `.so`.
    let results = unsafe { loader.from_path_scan_filtered_diag(root, &registry) };
    assert_eq!(
        results.len(),
        2,
        "(b) expected lazy entry skipped, eager + bad-abi scanned, got {}",
        results.len(),
    );

    let loaded_ids: Vec<ModuleId> = results
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .cloned()
        .collect();
    assert!(
        loaded_ids.iter().any(|id| id.as_str() == "sample"),
        "(b) eager-classified `sample` must load; got {loaded_ids:?}",
    );
    assert!(
        loaded_ids.iter().all(|id| id.as_str() != "test-dynamic"),
        "(b) lazy-classified `test-dynamic` must NOT load at startup; got {loaded_ids:?}",
    );

    let bad_diag = results
        .iter()
        .filter_map(|r| r.as_ref().err())
        .find(|d| matches!(d, LoadDiagnostic::AbiMismatchAtPackage { .. }))
        .expect("(d) expected AbiMismatchAtPackage for wrong-api-module");
    let LoadDiagnostic::AbiMismatchAtPackage { package, .. } = bad_diag else {
        unreachable!("matched above")
    };
    assert_eq!(package, "wrong-api-module", "(d) package name attached");
    let display = bad_diag.to_string();
    assert!(
        display.contains("wrong-api-module"),
        "(d) Display includes package name: {display}",
    );

    // ─── (c-α) — sub-plan 02: dispatcher fires lazy load ──────────
    // Move the loader behind a shared handle and compose the
    // dispatcher; on_resolve("save") matches the on-event trigger
    // and dlopens the lazy cdylib.
    assert_eq!(loader.len(), 1, "pre-dispatch baseline: only `sample` is loaded");
    let inner = Arc::new(Mutex::new(loader));
    let handle = LoaderHandle::from_arc(Arc::clone(&inner));
    let dispatcher = LazyCommandDispatcher::new(Arc::clone(&registry), handle, root.to_path_buf());

    dispatcher.on_resolve("save");

    let post_ids: Vec<String> = inner
        .lock()
        .loaded_ids()
        .map(|id| id.as_str().to_owned())
        .collect();
    assert!(
        post_ids.iter().any(|s| s == "test-dynamic"),
        "(c-α) on-event trigger dlopens the lazy cdylib; got {post_ids:?}",
    );
    assert_eq!(inner.lock().len(), 2, "(c-α) loader holds eager + lazy after dispatch");
}
