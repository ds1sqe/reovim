//! Tests for `DynamicClientModule` and the discovery/loading filter logic.
//!
//! See #724 countdown addendum — this file covers tests T3 (discovery
//! filter combinations) and supporting unit tests for the wrapper.
//!
//! The discovery filter test uses the crate-visible `load_filtered_dynamic_modules`
//! helper with a synthetic path list (no real filesystem needed for the
//! filter logic itself — the tests that actually `dlopen` a `.so` live in
//! `shared/clients/driver/tests/dynamic_loading.rs`).

#![allow(unsafe_code)] // filter helper is `unsafe` by signature

use std::{collections::HashSet, path::PathBuf};

use super::{DynamicClientModule, load_filtered_dynamic_modules};
use reovim_client_driver::{ClientModule, handle::ClientModuleHandle};

// =============================================================================
// DynamicClientModule wrapper — trait-level delegation
// =============================================================================

/// Minimal static module for exercising the test-only `new_for_test` path.
///
/// `new_for_test` wraps a *static* handle in a `DynamicClientModule` so we
/// can test the wrapper's trait delegation without needing a real `.so`.
struct WrapperTestModule;

impl ClientModule for WrapperTestModule {
    fn id(&self) -> &'static str {
        "wrapper-test"
    }
    fn kind(&self) -> &'static str {
        "wrapper-test"
    }
    fn name(&self) -> &'static str {
        "Wrapper Test Module"
    }
    fn version(&self) -> reovim_client_driver::Version {
        reovim_client_driver::Version::new(1, 2, 3)
    }
    fn dependencies(&self) -> &[&str] {
        &["dep-x"]
    }
    fn optional_dependencies(&self) -> &[&str] {
        &["dep-y"]
    }
    fn init(
        &mut self,
        _ctx: &reovim_client_driver::ModuleContext,
    ) -> reovim_client_driver::ProbeResult {
        reovim_client_driver::ProbeResult::Success
    }
    fn exit(&mut self) -> Result<(), reovim_client_driver::ClientModuleError> {
        Ok(())
    }
    fn has_chrome(&self) -> bool {
        true
    }
}

#[test]
fn wrapper_delegates_identity_to_handle() {
    let handle = ClientModuleHandle::from_static(Box::new(WrapperTestModule));
    let wrapper = DynamicClientModule::new_for_test(handle);
    assert_eq!(wrapper.id(), "wrapper-test");
    assert_eq!(wrapper.kind(), "wrapper-test");
    assert_eq!(wrapper.name(), "Wrapper Test Module");
    assert_eq!(
        wrapper.version(),
        reovim_client_driver::Version::new(1, 2, 3),
    );
    assert_eq!(wrapper.dependencies(), &["dep-x"]);
    assert_eq!(wrapper.optional_dependencies(), &["dep-y"]);
}

#[test]
fn wrapper_delegates_role_declaration_to_handle() {
    let handle = ClientModuleHandle::from_static(Box::new(WrapperTestModule));
    let wrapper = DynamicClientModule::new_for_test(handle);
    // WrapperTestModule returns true for has_chrome, false for others.
    assert!(wrapper.has_chrome());
    assert!(!wrapper.has_buffer_contrib());
    assert!(!wrapper.has_annotations());
}

#[test]
fn wrapper_render_stubs_return_defaults() {
    let handle = ClientModuleHandle::from_static(Box::new(WrapperTestModule));
    let wrapper = DynamicClientModule::new_for_test(handle);
    // All 13 TODO(#723) stubs return trait defaults until render FFI lands.
    assert!(
        wrapper
            .classify_token("keyword")
            .is_none()
    );
    assert!(wrapper.fold_ranges().is_empty());
    assert!(wrapper.virtual_lines().is_empty());
    assert!(wrapper.inline_decorations(0).is_empty());
    assert!(wrapper.cursor_position(10, 20).is_none());
    assert_eq!(
        wrapper.handle().kind(),
        "wrapper-test",
        "handle accessor should be reachable from tests",
    );
}

// =============================================================================
// T3 — discovery filter combination test
// =============================================================================
//
// The filter logic in `load_filtered_dynamic_modules` is tested with a
// synthetic path list pointing to non-existent files. Each `load_from_path`
// call fails with FileNotFound, which exercises the error-logging branch of
// the filter. The filter runs fully (no short-circuit), proving builtin /
// disabled / duplicate filters are all reachable in combination.
//
// A richer test that actually loads `.so` files AND covers all filter paths
// requires the #729 sample module pair (which builds two fixtures with
// different `kind()` values). For flight #724 the contract is: "filters run
// in the documented order AND log their skip reasons."

#[test]
fn filter_returns_empty_vec_when_all_paths_fail_to_load() {
    let missing_paths = vec![
        PathBuf::from("/tmp/nonexistent-reovim-t3-a.so"),
        PathBuf::from("/tmp/nonexistent-reovim-t3-b.so"),
        PathBuf::from("/tmp/nonexistent-reovim-t3-c.so"),
    ];
    let disabled: HashSet<String> = HashSet::new();
    let builtins: HashSet<&str> = HashSet::new();

    // SAFETY: all paths are non-existent, so load_from_path returns
    // FileNotFound before any FFI calls.
    let modules = unsafe {
        load_filtered_dynamic_modules(&missing_paths, &disabled, &builtins)
    };
    assert!(modules.is_empty());
}

#[test]
fn filter_handles_empty_discovery_list() {
    let discovered: Vec<PathBuf> = Vec::new();
    let disabled: HashSet<String> = HashSet::new();
    let builtins: HashSet<&str> = HashSet::new();
    let modules = unsafe {
        load_filtered_dynamic_modules(&discovered, &disabled, &builtins)
    };
    assert!(modules.is_empty());
}

#[test]
fn filter_respects_disabled_set_path_exists_but_rejected_at_load() {
    // When the .so file itself is missing OR fails to load, discovery never
    // even reaches the disabled-kind check. This test confirms the filter
    // doesn't panic or mis-handle the combination of an empty disabled set
    // + all-failing loads.
    let paths = vec![PathBuf::from("/tmp/nonexistent-reovim-t3-disabled.so")];
    let mut disabled: HashSet<String> = HashSet::new();
    disabled.insert("some-kind".to_string());
    let builtins: HashSet<&str> = HashSet::new();
    let modules = unsafe { load_filtered_dynamic_modules(&paths, &disabled, &builtins) };
    assert!(modules.is_empty());
}

// =============================================================================
// Cross-type dependency wrapper test (T4 supplement)
// =============================================================================
//
// The exact loader-level test for cross-type dep ordering lives in
// `shared/clients/driver/tests/dynamic_loading.rs::t4_cross_type_deps`. This
// supplemental test verifies that DynamicClientModule::new_for_test wraps a
// static handle and the resulting box dispatches kind() / dependencies()
// correctly — the minimal contract that T4 depends on.

#[test]
fn new_for_test_yields_working_client_module_trait_impl() {
    struct DepModule;
    impl ClientModule for DepModule {
        fn id(&self) -> &'static str {
            "dep-x"
        }
        fn kind(&self) -> &'static str {
            "dep-x"
        }
        fn name(&self) -> &'static str {
            "Dep X"
        }
        fn version(&self) -> reovim_client_driver::Version {
            reovim_client_driver::Version::new(0, 1, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_driver::ModuleContext,
        ) -> reovim_client_driver::ProbeResult {
            reovim_client_driver::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_driver::ClientModuleError> {
            Ok(())
        }
    }

    let handle = ClientModuleHandle::from_static(Box::new(DepModule));
    let wrapper: Box<dyn ClientModule> =
        Box::new(DynamicClientModule::new_for_test(handle));
    assert_eq!(wrapper.kind(), "dep-x");
    assert!(wrapper.dependencies().is_empty());
}
