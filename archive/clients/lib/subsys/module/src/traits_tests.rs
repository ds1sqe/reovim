use {
    super::*,
    crate::{
        testing::{
            MockPlatformCapabilities, MockServerHandle, MockThemeProvider, TestModuleContext,
            surface::TestChromeGrid,
        },
        types::{
            AnnotationContext, BufferId, BufferUpdateEvent, ChromePosition, ClientModuleError,
            ColorDepth, ColumnWidth, Insets, ProbeResult, Rect, RenderingModel, Style, Version,
            WindowId, WindowLayout,
        },
    },
};

// =============================================================================
// Mock module (test-specific ClientModule impl, kept local)
// =============================================================================

struct MockModule;

impl ClientModule for MockModule {
    fn id(&self) -> &'static str {
        "mock"
    }
    fn name(&self) -> &'static str {
        "Mock Module"
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
}

// =============================================================================
// ClientModule default methods
// =============================================================================

#[test]
fn client_module_identity_defaults() {
    let module = MockModule;
    assert_eq!(module.id(), "mock");
    assert_eq!(module.kind(), "mock"); // default: delegates to id()
    assert_eq!(module.name(), "Mock Module");
    assert_eq!(module.version(), Version::new(1, 0, 0));
    assert!(module.dependencies().is_empty());
    assert!(module.optional_dependencies().is_empty());
    assert_eq!(module.server_kinds(), vec!["mock"]); // default: vec![kind()]
}

#[test]
fn client_module_lifecycle() {
    let mut module = MockModule;
    let ctx = {
        let ctx = Box::leak(Box::new(TestModuleContext::builder().build()));
        ctx.as_context()
    };
    assert!(matches!(module.init(&ctx), ProbeResult::Success));
    assert!(module.exit().is_ok());
}

#[test]
fn client_module_role_defaults() {
    let module = MockModule;
    assert!(!module.has_chrome());
    assert!(!module.has_buffer_contrib());
    assert!(!module.has_annotations());
}

#[test]
fn client_module_event_defaults() {
    use crate::types::OptionValue;
    let mut module = MockModule;
    // All event methods are no-ops by default — just verify they don't panic.
    module.on_notification("{}");
    module.on_option_changed("test", &OptionValue::Bool(true));
    module.on_buffer_update(&BufferUpdateEvent {
        buffer_id: BufferId(0),
        revision: 1,
        changed_range: 0..1,
        new_lines: vec!["x".to_string()],
        total_lines: 1,
    });
    module.on_cursor_update(BufferId(0), 0, 0);
    module.on_buffer_focus(BufferId(0));
    module.on_mode_change("normal");
    module.on_capabilities_changed(&MockPlatformCapabilities::new());
    module.on_theme_changed(&MockThemeProvider::new());
    assert!(!module.tick());
}

#[test]
fn client_module_chrome_defaults() {
    let module = MockModule;
    assert_eq!(module.chrome_position(), ChromePosition::Bottom);
    assert_eq!(module.chrome_requested_size(&MockPlatformCapabilities::new()), 1);
    assert_eq!(module.chrome_priority(), 0);
    assert_eq!(module.chrome_z_order(), 0);
    // chrome_render default is a no-op — just verify no panic.
}

#[test]
fn client_module_buffer_contrib_defaults() {
    let module = MockModule;
    assert_eq!(module.buffer_contrib_priority(), 0);
    assert!(module.classify_token("keyword").is_none());
    assert!(module.transform_line(BufferId(0), 0, "hello").is_none());
    assert!(module.map_cursor_column(BufferId(0), 0, 5).is_none());
    assert!(module.fold_ranges().is_empty());
    assert!(module.virtual_lines().is_empty());
    assert!(module.inline_decorations(0).is_empty());
    assert!(module.cursor_position(80, 24).is_none());
}

#[test]
fn client_module_annotation_defaults() {
    let module = MockModule;
    let ctx = AnnotationContext {
        buffer_id: BufferId(0),
        total_lines: 100,
        visible_range: (0, 40),
        cursor_line: 10,
        gutter_style: Style::default(),
    };
    assert_eq!(
        module.annotation_column_width(&ctx, &MockPlatformCapabilities::new()),
        ColumnWidth::Fixed(0)
    );
    assert!(module.annotate(0, &ctx).is_none());
    assert_eq!(module.annotation_priority(), 0);
}

#[test]
fn client_module_on_all_loaded_default() {
    let mut module = MockModule;
    let ctx = {
        let ctx = Box::leak(Box::new(TestModuleContext::builder().build()));
        ctx.as_context()
    };
    // Default is a no-op — just verify no panic.
    module.on_all_loaded(&ctx);
}

// =============================================================================
// PlatformCapabilities mock
// =============================================================================

#[test]
fn platform_capabilities_tui_defaults() {
    let caps = MockPlatformCapabilities::new();
    assert_eq!(caps.rendering_model(), RenderingModel::CellGrid);
    assert_eq!(caps.grid_size(), Some((80, 24)));
    assert_eq!(caps.color_depth(), ColorDepth::TrueColor);
    assert!(caps.pixel_size().is_none());
    assert!(caps.reliable_unicode_width());
    assert!(caps.dark_mode());
    assert!(!caps.smooth_scroll());
    assert!(!caps.pointer_events());
    assert!(!caps.touch_input());
    assert!(!caps.haptic());
    assert_eq!(caps.safe_area(), Insets::default());
    assert!(caps.has_focus());
    assert!(caps.clipboard_available());
    assert!(!caps.screen_reader_active());
}

// =============================================================================
// ServerHandle mock
// =============================================================================

#[test]
fn server_handle_get_options_empty() {
    let server = MockServerHandle::new();
    let result = server.get_options(&["number", "relativenumber"]);
    assert!(result.is_empty());
}

#[test]
fn server_handle_execute_command() {
    let server = MockServerHandle::new();
    // No-op — just verify no panic.
    server.execute_command("echo hello");
}

#[test]
fn server_handle_list_commands_default() {
    let server = MockServerHandle::new();
    assert!(server.list_commands().is_empty());
}

#[test]
fn server_handle_get_option_metadata_default() {
    let server = MockServerHandle::new();
    assert!(server.get_option_metadata("number").is_none());
}

// =============================================================================
// ThemeProvider mock
// =============================================================================

#[test]
fn theme_provider_highlight() {
    let theme = MockThemeProvider::new();
    assert_eq!(theme.highlight("Normal"), Style::default());
}

#[test]
fn theme_provider_fallback_empty() {
    let theme = MockThemeProvider::new();
    let empty: &[&str] = &[];
    assert_eq!(theme.highlight_with_fallback(empty), Style::default());
}

#[test]
fn theme_provider_fallback_nonempty() {
    let theme = MockThemeProvider::new();
    let result = theme.highlight_with_fallback(&["CursorLine", "Normal"]);
    assert_eq!(result, Style::default());
}

#[test]
fn theme_provider_foreground_background() {
    use crate::types::Color;
    let theme = MockThemeProvider::new();
    assert_eq!(theme.foreground().fg, Some(Color::White));
    assert_eq!(theme.background().bg, Some(Color::Black));
}

#[test]
fn theme_provider_is_dark() {
    let theme = MockThemeProvider::new();
    assert!(theme.is_dark());
}

// =============================================================================
// ModuleContext
// =============================================================================

#[test]
fn module_context_construction() {
    let ctx = {
        let ctx = Box::leak(Box::new(TestModuleContext::builder().build()));
        ctx.as_context()
    };
    assert_eq!(ctx.capabilities.grid_size(), Some((80, 24)));
    assert!(ctx.theme.is_dark());
}

// =============================================================================
// ChromeSurface trait (verify trait is object-safe)
// =============================================================================

#[test]
fn render_surface_object_safety() {
    use crate::types::Color;
    let mut surface: Box<dyn ChromeSurface> = Box::new(TestChromeGrid::new(80, 24));
    let cols = surface.write_styled(0, 0, "hello", Style::default());
    assert_eq!(cols, 5);
    surface.apply_style(0, 0, Style::default());
    surface.overlay_bg(0, 0, Color::Red);
    surface.fill(Rect::new(0, 0, 10, 1), ' ', Style::default());
    surface.clear(Rect::new(0, 0, 80, 24));
    assert_eq!(surface.size(), (80, 24));
}

// =============================================================================
// TokenProvider trait (verify trait is object-safe)
// =============================================================================

struct MockTokenProvider;

impl TokenProvider for MockTokenProvider {
    fn tokens_for_line(&self, _buffer_id: BufferId, _line: u32) -> Vec<crate::types::SyntaxToken> {
        Vec::new()
    }
}

#[test]
fn token_provider_object_safety() {
    let provider: Box<dyn TokenProvider> = Box::new(MockTokenProvider);
    let tokens = provider.tokens_for_line(BufferId(0), 0);
    assert!(tokens.is_empty());
}

#[test]
fn token_provider_returns_tokens() {
    struct FilledTokenProvider;
    impl TokenProvider for FilledTokenProvider {
        fn tokens_for_line(
            &self,
            _buffer_id: BufferId,
            _line: u32,
        ) -> Vec<crate::types::SyntaxToken> {
            vec![crate::types::SyntaxToken {
                line: 0,
                start_col: 0,
                end_col: 5,
                category: "keyword".to_owned(),
            }]
        }
    }
    let provider = FilledTokenProvider;
    let tokens = provider.tokens_for_line(BufferId(1), 0);
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].category, "keyword");
}

// =============================================================================
// ViewportRenderer trait (verify trait is object-safe)
// =============================================================================

struct MockViewportRenderer;

impl ViewportRenderer for MockViewportRenderer {
    fn gutter_width(
        &self,
        _modules: &[Box<dyn ClientModule>],
        _caps: &dyn PlatformCapabilities,
    ) -> u16 {
        4
    }
    fn render_viewport(
        &self,
        _surface: &mut dyn ChromeSurface,
        _viewport: Rect,
        _ctx: &crate::types::ViewportContext<'_>,
        _modules: &[Box<dyn ClientModule>],
        _tokens: &dyn TokenProvider,
        _theme: &dyn ThemeProvider,
        _caps: &dyn PlatformCapabilities,
    ) {
    }
}

#[test]
fn viewport_renderer_object_safety() {
    let renderer: Box<dyn ViewportRenderer> = Box::new(MockViewportRenderer);
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(MockModule)];
    let caps = MockPlatformCapabilities::new();
    assert_eq!(renderer.gutter_width(&modules, &caps), 4);
}

// =============================================================================
// LayoutPolicy trait (verify trait is object-safe)
// =============================================================================

struct MockLayoutPolicy;

impl LayoutPolicy for MockLayoutPolicy {
    fn layout(
        &self,
        viewport: Rect,
        windows: &[WindowId],
        _focused: WindowId,
    ) -> Vec<WindowLayout> {
        windows
            .iter()
            .map(|&wid| WindowLayout {
                window_id: wid,
                bounds: viewport,
            })
            .collect()
    }
}

#[test]
fn layout_policy_object_safety() {
    let policy: Box<dyn LayoutPolicy> = Box::new(MockLayoutPolicy);
    let viewport = Rect::new(0, 0, 80, 24);
    let windows = [WindowId(0)];
    let result = policy.layout(viewport, &windows, WindowId(0));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].bounds, viewport);
}

// =============================================================================
// Trait object compatibility (dyn ClientModule)
// =============================================================================

#[test]
fn client_module_as_trait_object() {
    let module: Box<dyn ClientModule> = Box::new(MockModule);
    assert_eq!(module.id(), "mock");
    assert_eq!(module.kind(), "mock");
    assert!(!module.has_chrome());
}

#[test]
fn client_module_vec_of_trait_objects() {
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(MockModule), Box::new(MockModule)];
    assert_eq!(modules.len(), 2);
    assert_eq!(modules[0].id(), "mock");
    assert_eq!(modules[1].id(), "mock");
}

// =============================================================================
// LocalInputResult
// =============================================================================

#[test]
fn local_input_result_variants() {
    assert_eq!(LocalInputResult::Consumed, LocalInputResult::Consumed);
    assert_eq!(LocalInputResult::Unhandled, LocalInputResult::Unhandled);
    assert_eq!(LocalInputResult::SendToServer, LocalInputResult::SendToServer);
    assert_ne!(LocalInputResult::Consumed, LocalInputResult::Unhandled);
}

#[test]
fn local_input_result_copy() {
    let a = LocalInputResult::Consumed;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn local_input_result_debug() {
    assert!(format!("{:?}", LocalInputResult::Consumed).contains("Consumed"));
}

#[test]
fn client_module_local_input_defaults() {
    let mut module = MockModule;
    assert!(!module.wants_local_input());
    assert_eq!(module.on_local_input(b"dw"), LocalInputResult::Unhandled);
}
