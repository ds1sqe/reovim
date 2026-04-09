use std::sync::Arc;

use crate::{
    BufferId, BufferUpdateEvent, ChromePosition, ClientModule, ClientModuleError, ModuleContext,
    OptionValue, ProbeResult, Style, Version,
    handle::ClientModuleHandle,
    traits::{PlatformCapabilities, ServerHandle, ThemeProvider},
    types::{ColorDepth, Insets, RenderingModel},
};

// =============================================================================
// Mock module for handle tests
// =============================================================================

struct HandleTestModule {
    init_result: ProbeResult,
    exit_ok: bool,
    on_all_loaded_called: bool,
    last_notification: Option<String>,
    last_mode: Option<String>,
    last_cursor: Option<(BufferId, usize, usize)>,
    last_buffer_focus: Option<BufferId>,
    last_buffer_update_id: Option<BufferId>,
    last_option: Option<(String, OptionValue)>,
    tick_returns: bool,
}

impl HandleTestModule {
    fn new() -> Self {
        Self {
            init_result: ProbeResult::Success,
            exit_ok: true,
            on_all_loaded_called: false,
            last_notification: None,
            last_mode: None,
            last_cursor: None,
            last_buffer_focus: None,
            last_buffer_update_id: None,
            last_option: None,
            tick_returns: false,
        }
    }

    fn with_init_result(mut self, result: ProbeResult) -> Self {
        self.init_result = result;
        self
    }

    fn with_exit_fail(mut self) -> Self {
        self.exit_ok = false;
        self
    }

    fn with_tick_returns(mut self, val: bool) -> Self {
        self.tick_returns = val;
        self
    }
}

impl ClientModule for HandleTestModule {
    fn id(&self) -> &'static str {
        "handle-test"
    }
    fn kind(&self) -> &'static str {
        "handle-test"
    }
    fn name(&self) -> &'static str {
        "Handle Test Module"
    }
    fn version(&self) -> Version {
        Version::new(2, 3, 4)
    }
    fn dependencies(&self) -> &[&str] {
        &["dep-a"]
    }
    fn optional_dependencies(&self) -> &[&str] {
        &["opt-b"]
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        // Take the stored result (can only init once with configured result)
        std::mem::replace(&mut self.init_result, ProbeResult::Success)
    }
    fn exit(&mut self) -> Result<(), ClientModuleError> {
        if self.exit_ok {
            Ok(())
        } else {
            Err(ClientModuleError::exit_failed("exit failed"))
        }
    }
    fn on_all_loaded(&mut self, _ctx: &ModuleContext) {
        self.on_all_loaded_called = true;
    }
    fn on_notification(&mut self, data: &str) {
        self.last_notification = Some(data.to_string());
    }
    fn on_mode_change(&mut self, mode: &str) {
        self.last_mode = Some(mode.to_string());
    }
    fn on_cursor_update(&mut self, buffer_id: BufferId, line: usize, col: usize) {
        self.last_cursor = Some((buffer_id, line, col));
    }
    fn on_buffer_focus(&mut self, buffer_id: BufferId) {
        self.last_buffer_focus = Some(buffer_id);
    }
    fn on_buffer_update(&mut self, event: &BufferUpdateEvent) {
        self.last_buffer_update_id = Some(event.buffer_id);
    }
    fn on_option_changed(&mut self, name: &str, value: &OptionValue) {
        self.last_option = Some((name.to_string(), value.clone()));
    }
    fn tick(&mut self) -> bool {
        self.tick_returns
    }
    fn has_chrome(&self) -> bool {
        true
    }
    fn has_buffer_contrib(&self) -> bool {
        false
    }
    fn has_annotations(&self) -> bool {
        true
    }
    fn chrome_position(&self) -> ChromePosition {
        ChromePosition::Top
    }
    fn chrome_requested_size(&self, _caps: &dyn PlatformCapabilities) -> u16 {
        3
    }
    fn chrome_priority(&self) -> u16 {
        42
    }
    fn chrome_z_order(&self) -> u16 {
        7
    }
    fn buffer_contrib_priority(&self) -> u16 {
        99
    }
    fn annotation_priority(&self) -> u16 {
        15
    }
}

// =============================================================================
// Test helpers
// =============================================================================

struct MockCaps;
impl PlatformCapabilities for MockCaps {
    fn rendering_model(&self) -> RenderingModel {
        RenderingModel::CellGrid
    }
    fn grid_size(&self) -> Option<(u16, u16)> {
        Some((80, 24))
    }
    fn color_depth(&self) -> ColorDepth {
        ColorDepth::TrueColor
    }
    fn pixel_size(&self) -> Option<(u32, u32)> {
        None
    }
    fn reliable_unicode_width(&self) -> bool {
        true
    }
    fn dark_mode(&self) -> bool {
        true
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
    fn safe_area(&self) -> Insets {
        Insets::ZERO
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

struct MockServer;
impl ServerHandle for MockServer {
    fn get_options(&self, _names: &[&str]) -> Vec<(String, OptionValue)> {
        Vec::new()
    }
    fn execute_command(&self, _command: &str) {}
}

struct MockTheme;
impl ThemeProvider for MockTheme {
    fn highlight(&self, _group: &str) -> Style {
        Style::default()
    }
    fn highlight_with_fallback(&self, _groups: &[&str]) -> Style {
        Style::default()
    }
    fn foreground(&self) -> Style {
        Style::default()
    }
    fn background(&self) -> Style {
        Style::default()
    }
    fn is_dark(&self) -> bool {
        true
    }
}

fn make_ctx() -> ModuleContext<'static> {
    let caps: &'static dyn PlatformCapabilities = Box::leak(Box::new(MockCaps));
    let theme: &'static dyn ThemeProvider = Box::leak(Box::new(MockTheme));
    ModuleContext {
        capabilities: caps,
        server: Arc::new(MockServer),
        theme,
        services: None,
        module_registry: None,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[test]
fn handle_from_static() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    assert_eq!(handle.kind(), "handle-test");
    assert_eq!(handle.name(), "Handle Test Module");
    assert_eq!(handle.version(), Version::new(2, 3, 4));
    assert!(handle.is_static());
    assert!(!handle.is_dynamic());
}

#[test]
fn handle_init_static_success() {
    let module = Box::new(HandleTestModule::new());
    let mut handle = ClientModuleHandle::from_static(module);
    let ctx = make_ctx();
    let result = handle.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));
}

#[test]
fn handle_init_static_defer() {
    let module = Box::new(
        HandleTestModule::new().with_init_result(ProbeResult::Defer("not ready".to_string())),
    );
    let mut handle = ClientModuleHandle::from_static(module);
    let ctx = make_ctx();
    let result = handle.init(&ctx);
    assert!(matches!(result, ProbeResult::Defer(_)));
}

#[test]
fn handle_init_static_fail() {
    let module = Box::new(HandleTestModule::new().with_init_result(ProbeResult::Failed(
        ClientModuleError::init_failed("init failed", None),
    )));
    let mut handle = ClientModuleHandle::from_static(module);
    let ctx = make_ctx();
    let result = handle.init(&ctx);
    assert!(matches!(result, ProbeResult::Failed(_)));
}

#[test]
fn handle_exit_static() {
    let module = Box::new(HandleTestModule::new());
    let mut handle = ClientModuleHandle::from_static(module);
    assert!(handle.exit().is_ok());
}

#[test]
fn handle_exit_static_fail() {
    let module = Box::new(HandleTestModule::new().with_exit_fail());
    let mut handle = ClientModuleHandle::from_static(module);
    assert!(handle.exit().is_err());
}

#[test]
fn handle_on_all_loaded_static() {
    let module = Box::new(HandleTestModule::new());
    let mut handle = ClientModuleHandle::from_static(module);
    let ctx = make_ctx();
    // on_all_loaded dispatches to the static module without panic
    handle.on_all_loaded(&ctx);
    // Verify the module is still accessible
    assert!(handle.as_module().is_some());
}

#[test]
fn handle_is_static_true() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    assert!(handle.is_static());
}

#[test]
fn handle_is_dynamic_false() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    assert!(!handle.is_dynamic());
}

#[test]
fn handle_dependencies_static() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    assert_eq!(handle.dependencies(), &["dep-a"]);
    assert_eq!(handle.optional_dependencies(), &["opt-b"]);
}

#[test]
fn handle_as_module() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    let m = handle.as_module();
    assert!(m.is_some());
    assert_eq!(m.unwrap().kind(), "handle-test");
}

#[test]
fn handle_path_none_for_static() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    assert!(handle.path().is_none());
}

#[test]
fn handle_debug() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    let debug = format!("{handle:?}");
    assert!(debug.contains("handle-test"));
    assert!(debug.contains("Handle Test Module"));
}

// =============================================================================
// Event dispatch tests (static)
// =============================================================================

#[test]
fn handle_on_notification_static() {
    let module = Box::new(HandleTestModule::new());
    let mut handle = ClientModuleHandle::from_static(module);
    // Dispatches to the static module without panic.
    handle.on_notification(r#"{"type":"test"}"#);
    assert!(handle.is_static());
}

#[test]
fn handle_on_mode_change_static() {
    let module = Box::new(HandleTestModule::new());
    let mut handle = ClientModuleHandle::from_static(module);
    handle.on_mode_change("insert");
    assert!(handle.is_static());
}

#[test]
fn handle_on_cursor_update_static() {
    let module = Box::new(HandleTestModule::new());
    let mut handle = ClientModuleHandle::from_static(module);
    handle.on_cursor_update(BufferId(1), 10, 5);
    assert!(handle.is_static());
}

#[test]
fn handle_on_buffer_focus_static() {
    let module = Box::new(HandleTestModule::new());
    let mut handle = ClientModuleHandle::from_static(module);
    handle.on_buffer_focus(BufferId(42));
    assert!(handle.is_static());
}

#[test]
fn handle_on_buffer_update_static() {
    let module = Box::new(HandleTestModule::new());
    let mut handle = ClientModuleHandle::from_static(module);
    let event = BufferUpdateEvent {
        buffer_id: BufferId(1),
        revision: 5,
        changed_range: 0..10,
        new_lines: vec!["hello".to_string()],
        total_lines: 100,
    };
    handle.on_buffer_update(&event);
    assert!(handle.is_static());
}

#[test]
fn handle_on_option_changed_bool_static() {
    let module = Box::new(HandleTestModule::new());
    let mut handle = ClientModuleHandle::from_static(module);
    handle.on_option_changed("wrap", &OptionValue::Bool(true));
    assert!(handle.is_static());
}

#[test]
fn handle_on_option_changed_integer_static() {
    let module = Box::new(HandleTestModule::new());
    let mut handle = ClientModuleHandle::from_static(module);
    handle.on_option_changed("tabstop", &OptionValue::Integer(4));
    assert!(handle.is_static());
}

#[test]
fn handle_on_option_changed_string_static() {
    let module = Box::new(HandleTestModule::new());
    let mut handle = ClientModuleHandle::from_static(module);
    handle.on_option_changed("filetype", &OptionValue::String("rust".to_string()));
    assert!(handle.is_static());
}

#[test]
fn handle_tick_static_false() {
    let module = Box::new(HandleTestModule::new());
    let mut handle = ClientModuleHandle::from_static(module);
    assert!(!handle.tick());
}

#[test]
fn handle_tick_static_true() {
    let module = Box::new(HandleTestModule::new().with_tick_returns(true));
    let mut handle = ClientModuleHandle::from_static(module);
    assert!(handle.tick());
}

// =============================================================================
// Role declaration dispatch tests (static)
// =============================================================================

#[test]
fn handle_has_chrome_static() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    assert!(handle.has_chrome());
}

#[test]
fn handle_has_buffer_contrib_static() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    assert!(!handle.has_buffer_contrib());
}

#[test]
fn handle_has_annotations_static() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    assert!(handle.has_annotations());
}

#[test]
fn handle_chrome_position_static() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    assert_eq!(handle.chrome_position(), ChromePosition::Top);
}

#[test]
fn handle_chrome_requested_size_static() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    assert_eq!(handle.chrome_requested_size(), 3);
}

#[test]
fn handle_chrome_priority_static() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    assert_eq!(handle.chrome_priority(), 42);
}

#[test]
fn handle_chrome_z_order_static() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    assert_eq!(handle.chrome_z_order(), 7);
}

#[test]
fn handle_buffer_contrib_priority_static() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    assert_eq!(handle.buffer_contrib_priority(), 99);
}

#[test]
fn handle_annotation_priority_static() {
    let module = Box::new(HandleTestModule::new());
    let handle = ClientModuleHandle::from_static(module);
    assert_eq!(handle.annotation_priority(), 15);
}

// =============================================================================
// Compile-time checks
// =============================================================================

#[test]
fn ffi_symbols_send_check() {
    fn assert_send<T: Send>() {}
    assert_send::<crate::handle::ClientFfiSymbols>();
}

#[test]
fn ffi_symbols_all_none_backward_compat() {
    // Verify that ClientFfiSymbols can be constructed with all optional fields None.
    // This proves backward compatibility with pre-0.3.0 modules.
    let ffi = crate::handle::ClientFfiSymbols {
        init: dummy_init,
        exit: dummy_exit,
        destroy: dummy_destroy,
        on_all_loaded: None,
        on_notification: None,
        on_mode_change: None,
        on_cursor_update: None,
        on_buffer_focus: None,
        on_buffer_update: None,
        on_option_changed: None,
        tick: None,
        has_chrome: None,
        has_buffer_contrib: None,
        has_annotations: None,
        chrome_position: None,
        chrome_requested_size: None,
        chrome_priority: None,
        chrome_z_order: None,
        buffer_contrib_priority: None,
        annotation_priority: None,
        // #723 render-path symbols — all absent in pre-0.4.0 modules.
        chrome_render: None,
        annotate: None,
        annotation_column_width: None,
        transform_line: None,
        free_transformed_line: None,
        map_cursor_column: None,
        fold_ranges: None,
        virtual_lines: None,
        inline_decorations: None,
        cursor_position: None,
        classify_token: None,
        on_capabilities_changed: None,
        on_theme_changed: None,
    };
    // Verify all optional fields are None (backward compat with pre-0.4.0).
    assert!(ffi.on_notification.is_none());
    assert!(ffi.tick.is_none());
    assert!(ffi.has_chrome.is_none());
    assert!(ffi.chrome_render.is_none());
    assert!(ffi.transform_line.is_none());
}

unsafe extern "C" fn dummy_init(_: *mut std::ffi::c_void, _: *const std::ffi::c_void) -> i32 {
    0
}
unsafe extern "C" fn dummy_exit(_: *mut std::ffi::c_void) -> i32 {
    0
}
unsafe extern "C" fn dummy_destroy(_: *mut std::ffi::c_void) {}

// =============================================================================
// T2 — leak counter + HANDLE_LEAKS_PER_MODULE_FIXED contract (#724)
// =============================================================================
//
// The counter lives on an atomic shared across parallel test threads, so a
// precise delta-equals-expected assertion would be racy. These tests assert
// the LOWER-BOUND contract: each leak helper call adds AT LEAST its expected
// allocation to the counter. Combined with the identity assertions elsewhere
// (handle_dependencies_static) and the load-path integration tests in
// `tests/dynamic_loading.rs`, this catches regressions where the wrapper
// forgets to leak (delta < minimum) without requiring global test
// serialization.

#[test]
fn handle_leaks_per_module_fixed_is_documented_constant() {
    // Kind + name + deps_slice + opt_deps_slice = 4 fixed leaks.
    assert_eq!(crate::handle::HANDLE_LEAKS_PER_MODULE_FIXED, 4);
}

#[test]
fn expected_handle_leaks_counts_fixed_plus_deps() {
    use crate::handle::{HANDLE_LEAKS_PER_MODULE_FIXED, expected_handle_leaks};
    // Zero deps: exactly the fixed count.
    assert_eq!(expected_handle_leaks(0, 0), HANDLE_LEAKS_PER_MODULE_FIXED);
    // Mixed deps: fixed + required + optional.
    assert_eq!(expected_handle_leaks(2, 3), HANDLE_LEAKS_PER_MODULE_FIXED + 5,);
}

#[test]
fn leak_wrapper_increments_counter_lower_bound() {
    use std::sync::atomic::Ordering;

    let before = crate::handle::HANDLE_LEAK_COUNTER.load(Ordering::Relaxed);
    let _a = crate::handle::test_leak_str("hello".to_string());
    let _b = crate::handle::test_leak_str("world".to_string());
    // slice with 3 elements — expects 3 string leaks + 1 slice leak = 4
    let _s =
        crate::handle::test_leak_str_slice(vec!["x".to_string(), "y".to_string(), "z".to_string()]);
    let after = crate::handle::HANDLE_LEAK_COUNTER.load(Ordering::Relaxed);

    let delta = after.saturating_sub(before);
    // Our calls contributed exactly 2 + 3 + 1 = 6 increments. Parallel tests
    // may raise the delta higher, but never lower.
    let our_contribution: usize = 2 + 3 + 1;
    assert!(
        delta >= our_contribution,
        "expected at least {our_contribution} leaks, got delta {delta}",
    );
}

#[test]
fn from_static_contributes_expected_leak_count_lower_bound() {
    use std::sync::atomic::Ordering;

    let before = crate::handle::HANDLE_LEAK_COUNTER.load(Ordering::Relaxed);
    // HandleTestModule: 1 required dep + 1 optional dep.
    let _h = ClientModuleHandle::from_static(Box::new(HandleTestModule::new()));
    let after = crate::handle::HANDLE_LEAK_COUNTER.load(Ordering::Relaxed);

    let delta = after.saturating_sub(before);
    let expected = crate::handle::expected_handle_leaks(1, 1);
    assert!(
        delta >= expected,
        "from_static should contribute >= {expected} leaks, got {delta}",
    );
}
