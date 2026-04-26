//! Wave-3a chain-acceptance matrix test (client side).
//!
//! Drives one `tempfile::TempDir` library root through chain bullets
//! (a)/(b)/(c-α)/(d) sequentially against the client driver-loader
//! tier. (c-α) means the dispatcher composition path is exercised
//! directly: registry → `CapabilityLazyHook::dispatch_capability` →
//! probe → dlopen → `DriverStore`. Platform-startup integration of
//! the hook (TUI render-path threading) is a separate concern; it
//! is not exercised here.

#![allow(unsafe_code)]
#![allow(clippy::significant_drop_tightening)]

use std::{env, fs, path::PathBuf, sync::Arc};

use {
    reovim_client_subsys_driver_loader::{CapabilityLazyHook, LoadedClientRender, ScanEntryError},
    reovim_dylib_loader::{Kind, cdylib_filename},
    reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
    reovim_pkg_runtime_loader::load_registry,
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
        kind: Some("driver".into()),
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
fn wave_3a_chain_proof_client_side() {
    // ─── Setup: locate fixture cdylibs (skip cleanly if absent) ───
    let Some(render_src) = fixture_so("reovim_driver_abi_poc") else {
        eprintln!(
            "SKIP: render-driver cdylib missing — run `cargo build -p reovim-driver-abi-poc`"
        );
        return;
    };
    let Some(bad_abi_src) = fixture_so("reovim_pkg_wrong_abi_poc") else {
        eprintln!("SKIP: wrong-abi cdylib missing — run `cargo build -p reovim-pkg-wrong-abi-poc`");
        return;
    };

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    let driver_dir = root.join(Kind::Driver.subdir());
    fs::create_dir_all(&driver_dir).expect("driver dir");

    // Same source cdylib staged twice under distinct package names
    // — the registry classification is the only difference between
    // eager-render and lazy-render.
    fs::copy(&render_src, driver_dir.join(cdylib_filename("eager-render")))
        .expect("stage eager render");
    fs::copy(&render_src, driver_dir.join(cdylib_filename("lazy-render")))
        .expect("stage lazy render");
    fs::copy(&bad_abi_src, driver_dir.join(cdylib_filename("wrong-abi-poc")))
        .expect("stage wrong-abi");

    write_lockfile(
        root,
        vec![
            pkg("eager-render", "eager"),
            pkg("lazy-render", "on-capability:cell"),
            pkg("wrong-abi-poc", "eager"),
        ],
    );

    // ─── (a) — sub-plan 01: lockfile read ─────────────────────────
    let registry = load_registry(root).expect("load registry");
    assert!(registry.is_lazy("lazy-render"), "(a) lazy-render lazy");
    assert!(!registry.is_lazy("eager-render"), "(a) eager-render not lazy");
    assert!(!registry.is_lazy("wrong-abi-poc"), "(a) wrong-abi-poc not lazy");

    // ─── (b) — sub-plan 03: eager-only at startup + ───────────────
    // ─── (d) — sub-plan 04: enriched ABI diagnostic ───────────────
    let render_results = LoadedClientRender::from_path_scan_filtered(root, &registry);
    assert_eq!(
        render_results.len(),
        2,
        "(b) lazy-render filtered before construction; eager + bad-abi remain",
    );

    let ok_count = render_results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(ok_count, 1, "(b) exactly one render driver loads at startup (eager-render)");

    let err = render_results
        .into_iter()
        .find_map(Result::err)
        .expect("(d) expected one error variant for the bad-abi cdylib");
    let ScanEntryError::AbiMismatchAtPackage { package, .. } = err else {
        panic!("(d) expected AbiMismatchAtPackage, got {err:?}");
    };
    assert_eq!(package, "wrong-abi-poc", "(d) package name attached");

    // ─── (c-α) — sub-plan 03: capability hook fires lazy load ─────
    let hook = CapabilityLazyHook::new(Arc::clone(&registry), root.to_path_buf());
    hook.dispatch_capability("cell").expect("(c-α) dispatch ok");

    let store = hook.drivers().lock();
    assert!(
        store.render.contains_key("lazy-render"),
        "(c-α) on-capability=\"cell\" trigger dlopens lazy-render into the store; \
         got render keys {:?}",
        store.render.keys().collect::<Vec<_>>(),
    );
    assert_eq!(
        store.debug.len(),
        0,
        "(c-α) no debug driver expected; got {} entries",
        store.debug.len(),
    );
}
