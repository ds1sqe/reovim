use {super::*, reovim_client_driver::BufferId};

fn make_ctx(total_lines: usize, cursor_line: usize) -> AnnotationContext {
    AnnotationContext {
        buffer_id: BufferId(0),
        total_lines,
        visible_range: (0, total_lines),
        cursor_line,
        gutter_style: Style::default(),
    }
}

struct TestCaps;
impl PlatformCapabilities for TestCaps {
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
    fn safe_area(&self) -> reovim_client_driver::Insets {
        reovim_client_driver::Insets::ZERO
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

// =============================================================================
// Identity
// =============================================================================

#[test]
fn identity() {
    let module = LineNumbersModule::new();
    assert_eq!(module.id(), "line-numbers");
    assert_eq!(module.name(), "Line Numbers");
    assert_eq!(module.version(), Version::new(0, 1, 0));
}

// =============================================================================
// has_annotations
// =============================================================================

#[test]
fn has_annotations_none_mode() {
    let module = LineNumbersModule::new();
    assert!(!module.has_annotations());
}

#[test]
fn has_annotations_absolute_mode() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Absolute);
    assert!(module.has_annotations());
}

// =============================================================================
// annotation_column_width
// =============================================================================

#[test]
fn column_width_none_mode() {
    let module = LineNumbersModule::new();
    let ctx = make_ctx(100, 0);
    assert_eq!(module.annotation_column_width(&ctx, &TestCaps), ColumnWidth::Fixed(0));
}

#[test]
fn column_width_small_file() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Absolute);
    let ctx = make_ctx(9, 0);
    // 9 lines → 1 digit + 1 padding = Dynamic(2)
    assert_eq!(module.annotation_column_width(&ctx, &TestCaps), ColumnWidth::Dynamic(2));
}

#[test]
fn column_width_100_lines() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Absolute);
    let ctx = make_ctx(100, 0);
    // 100 lines → 3 digits + 1 padding = Dynamic(4)
    assert_eq!(module.annotation_column_width(&ctx, &TestCaps), ColumnWidth::Dynamic(4));
}

#[test]
fn column_width_empty_file() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Absolute);
    let ctx = make_ctx(0, 0);
    // 0 lines → 1 digit + 1 padding = Dynamic(2)
    assert_eq!(module.annotation_column_width(&ctx, &TestCaps), ColumnWidth::Dynamic(2));
}

// =============================================================================
// annotate — Absolute mode
// =============================================================================

#[test]
fn annotate_absolute_first_line() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Absolute);
    let ctx = make_ctx(10, 0);
    let cell = module.annotate(0, &ctx).unwrap();
    assert_eq!(cell.text, "1");
}

#[test]
fn annotate_absolute_cursor_line_yellow() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Absolute);
    let ctx = make_ctx(10, 5);
    let cell = module.annotate(5, &ctx).unwrap();
    assert_eq!(cell.text, "6");
    assert_eq!(cell.style.fg, Some(reovim_arch::Color::Yellow));
}

#[test]
fn annotate_absolute_non_cursor_grey() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Absolute);
    let ctx = make_ctx(10, 5);
    let cell = module.annotate(3, &ctx).unwrap();
    assert_eq!(cell.text, "4");
    assert_eq!(cell.style.fg, Some(reovim_arch::Color::DarkGrey));
}

// =============================================================================
// annotate — Relative mode
// =============================================================================

#[test]
fn annotate_relative_cursor_shows_absolute() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Relative);
    let ctx = make_ctx(20, 10);
    let cell = module.annotate(10, &ctx).unwrap();
    assert_eq!(cell.text, "11"); // absolute on cursor line
    assert_eq!(cell.style.fg, Some(reovim_arch::Color::Yellow));
}

#[test]
fn annotate_relative_above_cursor() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Relative);
    let ctx = make_ctx(20, 10);
    let cell = module.annotate(7, &ctx).unwrap();
    assert_eq!(cell.text, "3"); // 10 - 7 = 3
}

#[test]
fn annotate_relative_below_cursor() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Relative);
    let ctx = make_ctx(20, 10);
    let cell = module.annotate(13, &ctx).unwrap();
    assert_eq!(cell.text, "3"); // 13 - 10 = 3
}

// =============================================================================
// annotate — Hybrid mode
// =============================================================================

#[test]
fn annotate_hybrid_cursor_shows_absolute() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Hybrid);
    let ctx = make_ctx(20, 5);
    let cell = module.annotate(5, &ctx).unwrap();
    assert_eq!(cell.text, "6");
    assert_eq!(cell.style.fg, Some(reovim_arch::Color::Yellow));
}

// =============================================================================
// annotate — None mode
// =============================================================================

#[test]
fn annotate_none_returns_none() {
    let module = LineNumbersModule::new();
    let ctx = make_ctx(10, 0);
    assert!(module.annotate(0, &ctx).is_none());
}

// =============================================================================
// annotation_priority
// =============================================================================

#[test]
fn priority() {
    let module = LineNumbersModule::new();
    assert_eq!(module.annotation_priority(), 100);
}

// =============================================================================
// on_option_changed
// =============================================================================

#[test]
fn option_number_on() {
    let mut module = LineNumbersModule::new();
    module.on_option_changed("number", &OptionValue::Bool(true));
    assert_eq!(module.mode, LineNumberMode::Absolute);
}

#[test]
fn option_number_off() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Absolute);
    module.on_option_changed("number", &OptionValue::Bool(false));
    assert_eq!(module.mode, LineNumberMode::None);
}

#[test]
fn option_relativenumber_on_from_absolute() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Absolute);
    module.on_option_changed("relativenumber", &OptionValue::Bool(true));
    assert_eq!(module.mode, LineNumberMode::Hybrid);
}

#[test]
fn option_relativenumber_on_from_none() {
    let mut module = LineNumbersModule::new();
    module.on_option_changed("relativenumber", &OptionValue::Bool(true));
    assert_eq!(module.mode, LineNumberMode::Relative);
}

#[test]
fn option_relativenumber_off_from_hybrid() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Hybrid);
    module.on_option_changed("relativenumber", &OptionValue::Bool(false));
    assert_eq!(module.mode, LineNumberMode::Absolute);
}

#[test]
fn option_relativenumber_off_from_relative() {
    let mut module = LineNumbersModule::new();
    module.set_mode(LineNumberMode::Relative);
    module.on_option_changed("relativenumber", &OptionValue::Bool(false));
    assert_eq!(module.mode, LineNumberMode::None);
}

#[test]
fn option_unknown_ignored() {
    let mut module = LineNumbersModule::new();
    module.on_option_changed("unknown", &OptionValue::Bool(true));
    assert_eq!(module.mode, LineNumberMode::None);
}

// =============================================================================
// Lifecycle
// =============================================================================

#[test]
fn lifecycle() {
    let mut module = LineNumbersModule::new();
    let caps: &'static dyn PlatformCapabilities = Box::leak(Box::new(TestCaps));
    let theme: &'static dyn reovim_client_driver::ThemeProvider = Box::leak(Box::new(MockTheme));
    let ctx = ModuleContext {
        capabilities: caps,
        server: std::sync::Arc::new(MockServer),
        theme,
    };
    assert!(matches!(module.init(&ctx), ProbeResult::Success));
    assert!(module.exit().is_ok());
}

struct MockTheme;
impl reovim_client_driver::ThemeProvider for MockTheme {
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

struct MockServer;
impl reovim_client_driver::ServerHandle for MockServer {
    fn get_options(&self, _names: &[&str]) -> Vec<(String, OptionValue)> {
        Vec::new()
    }
    fn execute_command(&self, _command: &str) {}
}
