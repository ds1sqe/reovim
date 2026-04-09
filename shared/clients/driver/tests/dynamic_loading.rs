//! Integration tests for dynamic client module loading (flight #724).
//!
//! Exercises `ClientModuleHandle::load_from_path` + `LoadError` error paths
//! against a real `.so` built from `shared/clients/driver/tests/fixtures/
//! minimal_client_module/`. Also covers the `HANDLE_LEAKS_PER_MODULE_FIXED`
//! leak-bound contract (T2) and the cross-type dependency resolution in
//! `ClientModuleLoader::new_with_dynamic` (T4).
//!
//! # Prerequisites
//!
//! Build the fixture before running:
//!
//! ```sh
//! cargo build -p reovim-minimal-client-module
//! ```
//!
//! Tests that need the fixture will skip with a message if the `.so` is not
//! found, matching the pattern used by
//! `server/lib/drivers/module-loader/tests/integration.rs`.

#![allow(unsafe_code)] // FFI loading intrinsically requires unsafe

use std::{fs, path::PathBuf};

use reovim_client_driver::handle::{
    ClientModuleHandle, HANDLE_LEAKS_PER_MODULE_FIXED, LoadError,
};

// =============================================================================
// Fixture locator
// =============================================================================

/// Locate a fixture `.so` in `target/debug/` by library filename stem.
fn fixture_lib_path(stem: &str) -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()? // shared/clients
        .parent()? // shared
        .parent()?; // workspace root

    for ext in ["so", "dylib"] {
        let p = workspace_root
            .join("target")
            .join("debug")
            .join(format!("lib{stem}.{ext}"));
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Locate the minimal client module `.so`.
fn fixture_so_path() -> Option<PathBuf> {
    fixture_lib_path("reovim_minimal_client_module")
}

/// Locate the incompatible-API fixture (T5).
fn incompatible_fixture_path() -> Option<PathBuf> {
    fixture_lib_path("reovim_incompatible_api_client_module")
}

/// Locate the no-init fixture (T5).
fn no_init_fixture_path() -> Option<PathBuf> {
    fixture_lib_path("reovim_no_init_client_module")
}

/// Locate the bare fixture (T1 optional-symbol degradation).
fn bare_fixture_path() -> Option<PathBuf> {
    fixture_lib_path("reovim_bare_client_module")
}

/// Skip test if fixture `.so` is not present.
macro_rules! require_fixture {
    () => {
        match fixture_so_path() {
            Some(p) => p,
            None => {
                eprintln!(
                    "SKIP: minimal client module .so not found. \
                     Run: cargo build -p reovim-minimal-client-module",
                );
                return;
            }
        }
    };
}

// =============================================================================
// T1 — FFI symbol resolution and optional-symbol graceful degradation
// =============================================================================

#[test]
fn t1_load_minimal_client_module_resolves_all_required_symbols() {
    let so_path = require_fixture!();

    // SAFETY: Fixture is built from in-tree source with matching ABI.
    let handle = unsafe { ClientModuleHandle::load_from_path(&so_path) }
        .expect("load_from_path should succeed on fixture");

    assert_eq!(handle.kind(), "minimal-client");
    assert_eq!(handle.name(), "Minimal Client Module");
    assert!(handle.is_dynamic());
    assert!(!handle.is_static());
    assert_eq!(handle.path(), Some(&so_path));
}

#[test]
fn t1_bare_fixture_loads_with_no_optional_symbols() {
    let Some(so_path) = bare_fixture_path() else {
        eprintln!(
            "SKIP: bare fixture .so not found. \
             Run: cargo build -p reovim-bare-client-module",
        );
        return;
    };

    // SAFETY: bare fixture exports only the 6 required FFI symbols; the
    // resulting handle must dispatch every optional trampoline to its
    // documented default without panicking.
    let mut handle = unsafe { ClientModuleHandle::load_from_path(&so_path) }
        .expect("bare fixture should load successfully");

    // Identity — required symbols work.
    assert_eq!(handle.kind(), "bare-client");
    assert_eq!(handle.name(), "Bare Client Fixture");
    assert!(handle.is_dynamic());

    // Role declaration — all optional, must default to false.
    assert!(
        !handle.has_chrome(),
        "missing has_chrome symbol should default to false",
    );
    assert!(
        !handle.has_buffer_contrib(),
        "missing has_buffer_contrib symbol should default to false",
    );
    assert!(
        !handle.has_annotations(),
        "missing has_annotations symbol should default to false",
    );

    // Chrome metadata — all optional, must return defaults.
    assert_eq!(
        handle.chrome_position(),
        reovim_client_driver::ChromePosition::Bottom,
        "missing chrome_position symbol should default to Bottom",
    );
    assert_eq!(
        handle.chrome_priority(),
        0,
        "missing chrome_priority symbol should default to 0",
    );
    assert_eq!(
        handle.chrome_z_order(),
        0,
        "missing chrome_z_order symbol should default to 0",
    );

    // Priorities — all optional, default 0.
    assert_eq!(
        handle.buffer_contrib_priority(),
        0,
        "missing buffer_contrib_priority symbol should default to 0",
    );
    assert_eq!(
        handle.annotation_priority(),
        0,
        "missing annotation_priority symbol should default to 0",
    );

    // Tick — optional, defaults to false.
    assert!(
        !handle.tick(),
        "missing tick symbol should default to false",
    );

    // Events — optional, calling them must not panic (no-op).
    handle.on_notification(r#"{"k":"v"}"#);
    handle.on_mode_change("normal");
    handle.on_cursor_update(reovim_client_driver::BufferId(1), 0, 0);
    handle.on_buffer_focus(reovim_client_driver::BufferId(1));
    handle.on_option_changed(
        "wrap",
        &reovim_client_driver::OptionValue::Bool(true),
    );
    // No assertions — the contract is "no crash, no error." Reaching this
    // point proves graceful degradation for 5 event trampolines.
}

#[test]
fn t1_optional_symbols_missing_returns_default_behavior() {
    // The `declare_client_module!` macro generates ALL event + role + chrome
    // symbols unconditionally. A true "optional missing" test requires a
    // hand-written .so with symbols stripped, which is out of scope for this
    // flight. What we CAN verify is that the handle's dispatch methods for
    // optional trampolines return documented defaults when the symbol is
    // absent — that logic lives in handle.rs and is covered by the
    // `handle_has_chrome_default_false` style tests in handle_tests.rs.
    //
    // Here we simply document the contract and assert the handle's dispatch
    // matches the fixture's trait defaults (MinimalClientModule has no
    // chrome, no buffer contrib, no annotations, returns false / defaults).
    let so_path = require_fixture!();
    let handle = unsafe { ClientModuleHandle::load_from_path(&so_path) }.unwrap();

    assert!(!handle.has_chrome());
    assert!(!handle.has_buffer_contrib());
    assert!(!handle.has_annotations());
    // Tick with no override should return false via trait default.
    let mut handle = handle;
    assert!(!handle.tick());
}

// =============================================================================
// T2 — Leak bound
// =============================================================================

#[test]
fn t2_handle_leak_count_matches_constant() {
    // Note: HANDLE_LEAK_COUNTER is `pub(crate)` and only visible inside the
    // crate's own `#[cfg(test)]` module tree. For the integration test here,
    // we assert the public constant contract instead: constructing a handle
    // with N deps + M opt deps should leak HANDLE_LEAKS_PER_MODULE_FIXED + N
    // + M allocations.
    //
    // The minimal fixture declares 1 required dep + 1 optional dep, so the
    // expected per-handle leak count is HANDLE_LEAKS_PER_MODULE_FIXED + 1 + 1.
    //
    // The precise counter assertion (delta across N constructions) lives in
    // `handle_tests.rs::handle_leak_count_bounded` inside the crate, which
    // has access to the private HANDLE_LEAK_COUNTER static.
    assert_eq!(HANDLE_LEAKS_PER_MODULE_FIXED, 4);

    let so_path = require_fixture!();
    // SAFETY: fixture ABI-compatible.
    let handle = unsafe { ClientModuleHandle::load_from_path(&so_path) }.unwrap();
    // Post-condition: handle identity strings match the fixture probe.
    assert_eq!(handle.kind(), "minimal-client");
    assert_eq!(handle.dependencies(), &["required-dep"]);
    assert_eq!(handle.optional_dependencies(), &["optional-dep"]);
}

// =============================================================================
// T5 — load_from_path error paths
// =============================================================================

#[test]
fn t5_file_not_found() {
    let missing = PathBuf::from("/tmp/nonexistent-reovim-client-module-724.so");
    // SAFETY: path does not exist, so load_from_path returns an error
    // before any unsafe FFI calls.
    let err = unsafe { ClientModuleHandle::load_from_path(&missing) }.unwrap_err();
    assert!(
        matches!(err, LoadError::FileNotFound(_)),
        "expected FileNotFound, got {err:?}",
    );
    let msg = err.to_string();
    assert!(msg.contains("dlopen failed"), "message: {msg}");
}

#[test]
fn t5_not_a_shared_library() {
    // Write a temp file with non-ELF content and try to load it.
    let tmp = std::env::temp_dir().join("reovim-t5-not-a-so.so");
    fs::write(&tmp, b"this is not a shared library\n").unwrap();

    // SAFETY: path exists but is not a valid .so; libloading will reject it.
    let err = unsafe { ClientModuleHandle::load_from_path(&tmp) }.unwrap_err();

    fs::remove_file(&tmp).ok();

    assert!(
        matches!(err, LoadError::DlopenFailed(_)),
        "expected DlopenFailed, got {err:?}",
    );
    let msg = err.to_string();
    assert!(msg.contains("dlopen failed"), "message: {msg}");
}

#[test]
fn t5_incompatible_api_version() {
    let Some(so_path) = incompatible_fixture_path() else {
        eprintln!(
            "SKIP: incompatible fixture .so not found. \
             Run: cargo build -p reovim-incompatible-api-client-module",
        );
        return;
    };

    // SAFETY: fixture exports a version symbol with a hard-coded major
    // version bump; load_from_path fails the version check before running
    // any user code.
    let err = unsafe { ClientModuleHandle::load_from_path(&so_path) }.unwrap_err();
    match &err {
        LoadError::IncompatibleApiVersion { module, host } => {
            assert_eq!(module.0, 99, "fixture advertises major=99");
            // Host version comes from CLIENT_MODULE_API_VERSION at link time;
            // just verify the struct was populated.
            assert!(host.0 < 99);
        }
        other => panic!("expected IncompatibleApiVersion, got {other:?}"),
    }
    let msg = err.to_string();
    assert!(
        msg.contains("incompatible API version"),
        "message: {msg}",
    );
    assert!(msg.contains("99"), "message should cite module version: {msg}");
}

#[test]
fn t5_missing_init_symbol() {
    let Some(so_path) = no_init_fixture_path() else {
        eprintln!(
            "SKIP: no-init fixture .so not found. \
             Run: cargo build -p reovim-no-init-client-module",
        );
        return;
    };

    // SAFETY: fixture exports API version + probe + entry but deliberately
    // omits reovim_client_module_init; load_from_path fails at the required
    // symbol resolution step.
    let err = unsafe { ClientModuleHandle::load_from_path(&so_path) }.unwrap_err();
    match &err {
        LoadError::MissingSymbol { symbol, .. } => {
            assert_eq!(
                symbol, "reovim_client_module_init",
                "expected missing init symbol, got {symbol}",
            );
        }
        other => panic!("expected MissingSymbol for init, got {other:?}"),
    }
    let msg = err.to_string();
    assert!(
        msg.contains("no reovim_client_module_init symbol"),
        "message: {msg}",
    );
}

#[test]
fn t5_fixture_loads_and_probe_has_expected_identity() {
    // This is the "happy path" twin of the error-path tests above — it
    // proves the same code path reaches the success branch when given a
    // valid fixture. Covers the implicit contract that error-path tests
    // rely on a real success branch existing.
    let so_path = require_fixture!();
    let handle = unsafe { ClientModuleHandle::load_from_path(&so_path) }
        .expect("fixture should load");
    assert_eq!(handle.kind(), "minimal-client");
    assert_eq!(handle.version().major, 1);
}

// =============================================================================
// T4 — cross-type dependency resolution via ClientModuleLoader::new_with_dynamic
// =============================================================================

mod t4_cross_type_deps {
    use std::collections::{HashMap, HashSet};

    use reovim_client_driver::{
        BufferId, BufferUpdateEvent, ChromePosition, ClientModule, ClientModuleError,
        ClientModuleFactory, ClientModuleLoader, ModuleContext, OptionValue, ProbeResult, Version,
        handle::ClientModuleHandle,
    };

    /// Static factory module "static-a" that depends on dynamic kind "dynamic-b".
    struct StaticA;

    impl ClientModule for StaticA {
        fn id(&self) -> &'static str {
            "static-a"
        }
        fn kind(&self) -> &'static str {
            "static-a"
        }
        fn name(&self) -> &'static str {
            "Static A"
        }
        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }
        fn dependencies(&self) -> &[&str] {
            &["dynamic-b"]
        }
        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), ClientModuleError> {
            Ok(())
        }
    }

    /// Static module "dynamic-b" — stands in for a dynamic module via
    /// `ClientModuleHandle::from_static`. The `new_for_test` constructor on
    /// `DynamicClientModule` (inside the TUI crate) wraps this into a
    /// `Box<dyn ClientModule>` that the loader treats as dynamic. For this
    /// integration test (which cannot reach into the TUI crate) we bypass
    /// the wrapper and pass the module directly — `new_with_dynamic`
    /// concatenates factories + dynamics and feeds both through the resolver,
    /// so the test proves the merge logic works.
    struct DynamicBStub;

    impl ClientModule for DynamicBStub {
        fn id(&self) -> &'static str {
            "dynamic-b"
        }
        fn kind(&self) -> &'static str {
            "dynamic-b"
        }
        fn name(&self) -> &'static str {
            "Dynamic B (stub)"
        }
        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }
        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), ClientModuleError> {
            Ok(())
        }
        // explicitly unused trait methods
        fn on_notification(&mut self, _: &str) {}
        fn on_option_changed(&mut self, _: &str, _: &OptionValue) {}
        fn on_buffer_update(&mut self, _: &BufferUpdateEvent) {}
        fn on_cursor_update(&mut self, _: BufferId, _: usize, _: usize) {}
        fn on_buffer_focus(&mut self, _: BufferId) {}
        fn on_mode_change(&mut self, _: &str) {}
        fn chrome_position(&self) -> ChromePosition {
            ChromePosition::Bottom
        }
    }

    #[test]
    fn static_module_depends_on_dynamic_module_resolves() {
        let mut factories: HashMap<&'static str, ClientModuleFactory> = HashMap::new();
        factories.insert("static-a", (|| Box::new(StaticA)) as ClientModuleFactory);

        let disabled: HashSet<String> = HashSet::new();
        let dynamic: Vec<Box<dyn ClientModule>> = vec![Box::new(DynamicBStub)];

        let loader = ClientModuleLoader::new_with_dynamic(factories, &disabled, dynamic)
            .expect("cross-type dep resolution should succeed");

        assert_eq!(loader.module_count(), 2);

        // Verify both kinds are present and b is initialized before a.
        let kinds: Vec<&str> = loader.modules().iter().map(|m| m.kind()).collect();
        // After resolve_and_reorder, dynamic-b must come before static-a.
        let pos_a = kinds.iter().position(|k| *k == "static-a").unwrap();
        let pos_b = kinds.iter().position(|k| *k == "dynamic-b").unwrap();
        assert!(
            pos_b < pos_a,
            "dynamic-b should be ordered before static-a but got {kinds:?}",
        );

        // Drop to silence unused-field warnings.
        drop(loader);
        let _ = ClientModuleHandle::from_static(Box::new(StaticA));
    }
}
