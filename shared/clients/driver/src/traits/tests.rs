use std::sync::Arc;

use {
    super::*,
    crate::{
        BufferId, ChromePosition, ClientModuleError, ColorDepth, ColumnWidth, Insets, ProbeResult,
        Rect, RenderingModel, Style, Version,
    },
};

// =============================================================================
// Mock implementations
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

struct MockPlatformCapabilities;

impl PlatformCapabilities for MockPlatformCapabilities {
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
        Insets::default()
    }
    fn has_focus(&self) -> bool {
        true
    }
    fn clipboard_available(&self) -> bool {
        true
    }
    fn screen_reader_active(&self) -> bool {
        false
    }
}

struct MockServerHandle;

impl ServerHandle for MockServerHandle {
    fn get_options(&self, _names: &[&str]) -> Vec<(String, crate::OptionValue)> {
        Vec::new()
    }
    fn execute_command(&self, _command: &str) {}
}

struct MockThemeProvider;

impl ThemeProvider for MockThemeProvider {
    fn highlight(&self, _group: &str) -> Style {
        Style::default()
    }
    fn highlight_with_fallback(&self, groups: &[&str]) -> Style {
        if groups.is_empty() {
            return Style::default();
        }
        self.highlight(groups[0])
    }
    fn foreground(&self) -> Style {
        Style {
            fg: Some(crate::Color::White),
            ..Style::default()
        }
    }
    fn background(&self) -> Style {
        Style {
            bg: Some(crate::Color::Black),
            ..Style::default()
        }
    }
    fn is_dark(&self) -> bool {
        true
    }
}

fn make_module_context() -> ModuleContext<'static> {
    // Use leaked references for test simplicity — tests are short-lived.
    let caps: &'static dyn PlatformCapabilities = Box::leak(Box::new(MockPlatformCapabilities));
    let theme: &'static dyn ThemeProvider = Box::leak(Box::new(MockThemeProvider));
    ModuleContext {
        capabilities: caps,
        server: Arc::new(MockServerHandle),
        theme,
        services: None,
        module_registry: None,
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
    let ctx = make_module_context();
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
    let mut module = MockModule;
    // All event methods are no-ops by default — just verify they don't panic.
    module.on_notification("{}");
    module.on_option_changed("test", &crate::OptionValue::Bool(true));
    module.on_buffer_update(&crate::BufferUpdateEvent {
        buffer_id: BufferId(0),
        revision: 1,
        changed_range: 0..1,
        new_lines: vec!["x".to_string()],
        total_lines: 1,
    });
    module.on_cursor_update(BufferId(0), 0, 0);
    module.on_buffer_focus(BufferId(0));
    module.on_mode_change("normal");
    module.on_capabilities_changed(&MockPlatformCapabilities);
    module.on_theme_changed(&MockThemeProvider);
    assert!(!module.tick());
}

#[test]
fn client_module_chrome_defaults() {
    let module = MockModule;
    assert_eq!(module.chrome_position(), ChromePosition::Bottom);
    assert_eq!(module.chrome_requested_size(&MockPlatformCapabilities), 1);
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
    let ctx = crate::AnnotationContext {
        buffer_id: BufferId(0),
        total_lines: 100,
        visible_range: (0, 40),
        cursor_line: 10,
        gutter_style: Style::default(),
    };
    assert_eq!(
        module.annotation_column_width(&ctx, &MockPlatformCapabilities),
        ColumnWidth::Fixed(0)
    );
    assert!(module.annotate(0, &ctx).is_none());
    assert_eq!(module.annotation_priority(), 0);
}

#[test]
fn client_module_on_all_loaded_default() {
    let mut module = MockModule;
    let ctx = make_module_context();
    // Default is a no-op — just verify no panic.
    module.on_all_loaded(&ctx);
}

// =============================================================================
// PlatformCapabilities mock
// =============================================================================

#[test]
fn platform_capabilities_tui_defaults() {
    let caps = MockPlatformCapabilities;
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
    let server = MockServerHandle;
    let result = server.get_options(&["number", "relativenumber"]);
    assert!(result.is_empty());
}

#[test]
fn server_handle_execute_command() {
    let server = MockServerHandle;
    // No-op — just verify no panic.
    server.execute_command("echo hello");
}

#[test]
fn server_handle_list_commands_default() {
    let server = MockServerHandle;
    assert!(server.list_commands().is_empty());
}

#[test]
fn server_handle_get_option_metadata_default() {
    let server = MockServerHandle;
    assert!(server.get_option_metadata("number").is_none());
}

// =============================================================================
// ThemeProvider mock
// =============================================================================

#[test]
fn theme_provider_highlight() {
    let theme = MockThemeProvider;
    assert_eq!(theme.highlight("Normal"), Style::default());
}

#[test]
fn theme_provider_fallback_empty() {
    let theme = MockThemeProvider;
    let empty: &[&str] = &[];
    assert_eq!(theme.highlight_with_fallback(empty), Style::default());
}

#[test]
fn theme_provider_fallback_nonempty() {
    let theme = MockThemeProvider;
    let result = theme.highlight_with_fallback(&["CursorLine", "Normal"]);
    assert_eq!(result, Style::default());
}

#[test]
fn theme_provider_foreground_background() {
    let theme = MockThemeProvider;
    assert_eq!(theme.foreground().fg, Some(crate::Color::White));
    assert_eq!(theme.background().bg, Some(crate::Color::Black));
}

#[test]
fn theme_provider_is_dark() {
    let theme = MockThemeProvider;
    assert!(theme.is_dark());
}

// =============================================================================
// ModuleContext
// =============================================================================

#[test]
fn module_context_construction() {
    let ctx = make_module_context();
    assert_eq!(ctx.capabilities.grid_size(), Some((80, 24)));
    assert!(ctx.theme.is_dark());
}

// =============================================================================
// RenderSurface trait (verify trait is object-safe)
// =============================================================================

struct MockRenderSurface;

impl RenderSurface for MockRenderSurface {
    #[allow(clippy::cast_possible_truncation)]
    fn write_styled(&mut self, _x: u16, _y: u16, text: &str, _style: Style) -> u16 {
        text.len() as u16
    }
    fn apply_style(&mut self, _x: u16, _y: u16, _style: Style) {}
    fn overlay_bg(&mut self, _x: u16, _y: u16, _bg: crate::Color) {}
    fn fill(&mut self, _rect: Rect, _ch: char, _style: Style) {}
    fn clear(&mut self, _rect: Rect) {}
    fn size(&self) -> (u16, u16) {
        (80, 24)
    }
}

#[test]
fn render_surface_object_safety() {
    let mut surface: Box<dyn RenderSurface> = Box::new(MockRenderSurface);
    let cols = surface.write_styled(0, 0, "hello", Style::default());
    assert_eq!(cols, 5);
    surface.apply_style(0, 0, Style::default());
    surface.overlay_bg(0, 0, crate::Color::Red);
    surface.fill(Rect::new(0, 0, 10, 1), ' ', Style::default());
    surface.clear(Rect::new(0, 0, 80, 24));
    assert_eq!(surface.size(), (80, 24));
}

// =============================================================================
// TokenProvider trait (verify trait is object-safe)
// =============================================================================

struct MockTokenProvider;

impl TokenProvider for MockTokenProvider {
    fn tokens_for_line(&self, _buffer_id: crate::BufferId, _line: u32) -> Vec<crate::SyntaxToken> {
        Vec::new()
    }
}

#[test]
fn token_provider_object_safety() {
    let provider: Box<dyn TokenProvider> = Box::new(MockTokenProvider);
    let tokens = provider.tokens_for_line(crate::BufferId(0), 0);
    assert!(tokens.is_empty());
}

#[test]
fn token_provider_returns_tokens() {
    struct FilledTokenProvider;
    impl TokenProvider for FilledTokenProvider {
        fn tokens_for_line(
            &self,
            _buffer_id: crate::BufferId,
            _line: u32,
        ) -> Vec<crate::SyntaxToken> {
            vec![crate::SyntaxToken {
                line: 0,
                start_col: 0,
                end_col: 5,
                category: "keyword".to_owned(),
            }]
        }
    }
    let provider = FilledTokenProvider;
    let tokens = provider.tokens_for_line(crate::BufferId(1), 0);
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
        _surface: &mut dyn RenderSurface,
        _viewport: Rect,
        _ctx: &crate::ViewportContext<'_>,
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
    let caps = MockPlatformCapabilities;
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
        windows: &[crate::WindowId],
        _focused: crate::WindowId,
    ) -> Vec<crate::WindowLayout> {
        windows
            .iter()
            .map(|&wid| crate::WindowLayout {
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
    let windows = [crate::WindowId(0)];
    let result = policy.layout(viewport, &windows, crate::WindowId(0));
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
