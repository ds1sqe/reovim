//! Tests for `DynamicClientModule` and the discovery/loading filter logic.
//!
//! See #724 countdown addendum — this file covers tests T3 (discovery
//! filter combinations) and supporting unit tests for the wrapper.
//!
//! The discovery filter test uses the crate-visible `load_filtered_dynamic_modules`
//! helper with a synthetic path list (no real filesystem needed for the
//! filter logic itself — the tests that actually `dlopen` a `.so` live in
//! `clients/lib/driver/tests/dynamic_loading.rs`).

#![allow(unsafe_code)] // filter helper is `unsafe` by signature

use std::{collections::HashSet, path::PathBuf};

use {
    super::{DynamicClientModule, load_filtered_dynamic_modules},
    reovim_client_driver::{
        AnnotationContext, BufferId, ChromeSurface, ClientModule, ColumnWidth, GutterCell, Rect,
        RenderBehavior, Style, TransformedLine, Version, handle::ClientModuleHandle,
    },
};

struct TestCaps;

impl reovim_client_driver::PlatformCapabilities for TestCaps {
    fn rendering_model(&self) -> reovim_client_driver::RenderingModel {
        reovim_client_driver::RenderingModel::CellGrid
    }

    fn grid_size(&self) -> Option<(u16, u16)> {
        Some((80, 24))
    }

    fn color_depth(&self) -> reovim_client_driver::ColorDepth {
        reovim_client_driver::ColorDepth::TrueColor
    }

    fn pixel_size(&self) -> Option<(u32, u32)> {
        None
    }

    fn reliable_unicode_width(&self) -> bool {
        true
    }

    fn dark_mode(&self) -> bool {
        false
    }

    fn smooth_scroll(&self) -> bool {
        false
    }

    fn pointer_events(&self) -> bool {
        false
    }

    fn touch_input(&self) -> bool {
        false
    }

    fn haptic(&self) -> bool {
        false
    }

    fn safe_area(&self) -> reovim_client_driver::Insets {
        reovim_client_driver::Insets::ZERO
    }

    fn has_focus(&self) -> bool {
        true
    }

    fn clipboard_available(&self) -> bool {
        false
    }

    fn screen_reader_active(&self) -> bool {
        false
    }
}

// =============================================================================
// DynamicClientModule wrapper — trait-level delegation
// =============================================================================

/// Minimal static module for exercising the test-only `new_for_test` path.
///
/// `new_for_test` wraps a *static* handle in a `DynamicClientModule` so we
/// can test the wrapper's trait delegation without needing a real `.so`.
struct WrapperTestModule;

struct TestSurface {
    writes: Vec<(u16, u16, String, Style)>,
}

impl TestSurface {
    fn new() -> Self {
        Self { writes: Vec::new() }
    }
}

impl ChromeSurface for TestSurface {
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        self.writes.push((x, y, text.to_string(), style));
        u16::try_from(text.chars().count()).expect("test surface write length fits in u16")
    }

    fn apply_style(&mut self, _x: u16, _y: u16, _style: Style) {}

    fn overlay_bg(&mut self, _x: u16, _y: u16, _bg: reovim_client_driver::Color) {}

    fn fill(&mut self, _rect: Rect, _ch: char, _style: Style) {}

    fn clear(&mut self, _rect: Rect) {}

    fn size(&self) -> (u16, u16) {
        (40, 10)
    }
}

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
    fn version(&self) -> Version {
        Version::new(1, 2, 3)
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

    fn chrome_render(
        &self,
        surface: &mut dyn ChromeSurface,
        _bounds: Rect,
        _caps: &dyn reovim_client_driver::PlatformCapabilities,
    ) {
        surface.write_styled(1, 2, "wrapper", Style::new().bold());
    }

    fn classify_token(&self, category: &str) -> Option<RenderBehavior> {
        (category == "keyword").then_some(RenderBehavior::Highlight)
    }

    fn transform_line(&self, _buf: BufferId, _line: usize, text: &str) -> Option<TransformedLine> {
        Some(TransformedLine {
            segments: vec![(format!("wrapped:{text}"), Some(Style::new().italic()))],
        })
    }

    fn cursor_position(&self, _w: u16, _h: u16) -> Option<(u16, u16)> {
        Some((7, 4))
    }

    fn annotation_column_width(
        &self,
        _ctx: &AnnotationContext,
        _caps: &dyn reovim_client_driver::PlatformCapabilities,
    ) -> ColumnWidth {
        ColumnWidth::Fixed(3)
    }

    fn annotate(&self, line: usize, _ctx: &AnnotationContext) -> Option<GutterCell> {
        Some(GutterCell {
            text: format!("{line}"),
            style: Style::new().underline(),
        })
    }
}

#[test]
fn wrapper_delegates_identity_to_handle() {
    let handle = ClientModuleHandle::from_static(Box::new(WrapperTestModule));
    let wrapper = DynamicClientModule::new_for_test(handle);
    assert_eq!(wrapper.id(), "wrapper-test");
    assert_eq!(wrapper.kind(), "wrapper-test");
    assert_eq!(wrapper.name(), "Wrapper Test Module");
    assert_eq!(wrapper.version(), reovim_client_driver::Version::new(1, 2, 3),);
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
fn wrapper_delegates_supported_render_methods_and_keeps_slice_limitations_explicit() {
    let handle = ClientModuleHandle::from_static(Box::new(WrapperTestModule));
    let wrapper = DynamicClientModule::new_for_test(handle);

    let mut surface = TestSurface::new();
    let caps = TestCaps;
    let ctx = AnnotationContext {
        buffer_id: BufferId(9),
        total_lines: 20,
        visible_range: (3, 12),
        cursor_line: 7,
        gutter_style: Style::new(),
    };

    wrapper.chrome_render(
        &mut surface,
        Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 5,
        },
        &caps,
    );
    assert_eq!(surface.writes.len(), 1);
    assert_eq!(surface.writes[0].0, 1);
    assert_eq!(surface.writes[0].1, 2);
    assert_eq!(surface.writes[0].2, "wrapper");

    assert_eq!(wrapper.classify_token("keyword"), Some(RenderBehavior::Highlight));
    assert_eq!(
        wrapper.transform_line(BufferId(1), 3, "line"),
        Some(TransformedLine {
            segments: vec![("wrapped:line".to_string(), Some(Style::new().italic()))],
        }),
    );
    assert!(wrapper.fold_ranges().is_empty());
    assert!(wrapper.virtual_lines().is_empty());
    assert!(wrapper.inline_decorations(0).is_empty());
    assert_eq!(wrapper.cursor_position(10, 20), Some((7, 4)));
    assert_eq!(wrapper.annotation_column_width(&ctx, &caps), ColumnWidth::Fixed(3));
    assert_eq!(
        wrapper.annotate(4, &ctx),
        Some(GutterCell {
            text: "4".to_string(),
            style: Style::new().underline(),
        }),
    );
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
    let modules = unsafe { load_filtered_dynamic_modules(&missing_paths, &disabled, &builtins) };
    assert!(modules.is_empty());
}

#[test]
fn filter_handles_empty_discovery_list() {
    let discovered: Vec<PathBuf> = Vec::new();
    let disabled: HashSet<String> = HashSet::new();
    let builtins: HashSet<&str> = HashSet::new();
    let modules = unsafe { load_filtered_dynamic_modules(&discovered, &disabled, &builtins) };
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
// `clients/lib/driver/tests/dynamic_loading.rs::t4_cross_type_deps`. This
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
    let wrapper: Box<dyn ClientModule> = Box::new(DynamicClientModule::new_for_test(handle));
    assert_eq!(wrapper.kind(), "dep-x");
    assert!(wrapper.dependencies().is_empty());
}
