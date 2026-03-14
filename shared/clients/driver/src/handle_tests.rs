use std::sync::Arc;

use crate::{
    ClientModule, ClientModuleError, ModuleContext, OptionValue, ProbeResult, Style, Version,
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
}

impl HandleTestModule {
    fn new() -> Self {
        Self {
            init_result: ProbeResult::Success,
            exit_ok: true,
            on_all_loaded_called: false,
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
            Err(ClientModuleError {
                message: "exit failed".to_string(),
            })
        }
    }
    fn on_all_loaded(&mut self, _ctx: &ModuleContext) {
        self.on_all_loaded_called = true;
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
        ClientModuleError {
            message: "init failed".to_string(),
        },
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
    assert_eq!(handle.dependencies(), vec!["dep-a"]);
    assert_eq!(handle.optional_dependencies(), vec!["opt-b"]);
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
