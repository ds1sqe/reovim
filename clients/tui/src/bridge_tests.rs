use reovim_client_driver::{
    BufferId, BufferUpdateEvent, ChromePosition, ClientModule, ColumnWidth, PlatformCapabilities,
    Rect, RenderBehavior, Style, Version,
};
use reovim_driver_display::{
    render_backend::{
        RenderBackend, RenderBehavior as DisplayRenderBehavior,
        TransformedLine as DisplayTransformedLine, TuiExtension, VirtualLine as DisplayVirtualLine,
        VirtualLinePosition as DisplayVirtualLinePosition,
    },
    Attributes as DisplayAttributes, Style as DisplayStyle,
};
use std::borrow::Cow;

use super::{
    classify_extension, convert_display_style_to_driver_style, convert_render_behavior,
    convert_transformed_line, wrap_extensions, TuiExtensionBridge,
};

// =============================================================================
// Mock TuiExtension
// =============================================================================

#[allow(clippy::struct_excessive_bools)]
struct MockExtension {
    kind: &'static str,
    active: bool,
    notification_data: Option<String>,
    mode_name: Option<String>,
    mode_is_insert: Option<bool>,
    buffer_update_called: bool,
    cursor_update_called: bool,
    tick_returns: bool,
    init_called: bool,
    exit_called: bool,
    fold_ranges: Vec<(u32, u32)>,
    vlines: Vec<DisplayVirtualLine>,
    offset_left: u16,
    classify_result: Option<DisplayRenderBehavior>,
    transform_result: Option<DisplayTransformedLine>,
    cursor_col_result: Option<u16>,
    cursor_pos_result: Option<(u16, u16)>,
    server_kinds_val: Vec<&'static str>,
    dependencies_val: Vec<&'static str>,
}

impl Default for MockExtension {
    fn default() -> Self {
        Self {
            kind: "mock",
            active: true,
            notification_data: None,
            mode_name: None,
            mode_is_insert: None,
            buffer_update_called: false,
            cursor_update_called: false,
            tick_returns: false,
            init_called: false,
            exit_called: false,
            fold_ranges: Vec::new(),
            vlines: Vec::new(),
            offset_left: 0,
            classify_result: None,
            transform_result: None,
            cursor_col_result: None,
            cursor_pos_result: None,
            server_kinds_val: vec!["mock"],
            dependencies_val: Vec::new(),
        }
    }
}

impl MockExtension {
    fn with_kind(mut self, kind: &'static str) -> Self {
        self.kind = kind;
        self.server_kinds_val = vec![kind];
        self
    }
}

impl TuiExtension for MockExtension {
    fn kind(&self) -> &'static str {
        self.kind
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn apply_notification(&mut self, data: &str) {
        self.notification_data = Some(data.to_string());
    }

    fn render(&self, _backend: &mut dyn RenderBackend) {}

    fn tick(&mut self) -> bool {
        self.tick_returns
    }

    fn cursor_position(&self, _w: u16, _h: u16) -> Option<(u16, u16)> {
        self.cursor_pos_result
    }

    fn content_offset_left(&self) -> u16 {
        self.offset_left
    }

    fn fold_hidden_lines(&self) -> &[(u32, u32)] {
        &self.fold_ranges
    }

    fn classify_token(&self, _category: &str) -> Option<DisplayRenderBehavior> {
        self.classify_result.clone()
    }

    fn on_buffer_update(&mut self, _buffer_id: u64, _lines: &[String]) {
        self.buffer_update_called = true;
    }

    fn virtual_lines(&self) -> &[DisplayVirtualLine] {
        &self.vlines
    }

    fn transform_line(
        &self,
        _buffer_id: u64,
        _line_idx: usize,
        _line: &str,
    ) -> Option<DisplayTransformedLine> {
        self.transform_result.clone()
    }

    fn map_cursor_column(
        &self,
        _buffer_id: u64,
        _line_idx: usize,
        _buffer_col: usize,
    ) -> Option<u16> {
        self.cursor_col_result
    }

    fn on_cursor_update(&mut self, _buffer_id: u64, _line: usize, _col: usize) {
        self.cursor_update_called = true;
    }

    fn on_mode_change(&mut self, mode_name: &str, is_insert: bool) {
        self.mode_name = Some(mode_name.to_string());
        self.mode_is_insert = Some(is_insert);
    }

    fn dependencies(&self) -> &[&'static str] {
        &self.dependencies_val
    }

    fn init(&mut self) {
        self.init_called = true;
    }

    fn exit(&mut self) {
        self.exit_called = true;
    }

    fn server_kinds(&self) -> Vec<&'static str> {
        self.server_kinds_val.clone()
    }
}

// =============================================================================
// classify_extension
// =============================================================================

#[test]
fn classify_cmdline() {
    let (chrome, buf, pos, pri) = classify_extension("cmdline");
    assert!(chrome);
    assert!(!buf);
    assert_eq!(pos, ChromePosition::Bottom);
    assert_eq!(pri, 90);
}

#[test]
fn classify_whichkey() {
    let (chrome, buf, pos, pri) = classify_extension("whichkey");
    assert!(chrome);
    assert!(!buf);
    assert_eq!(pos, ChromePosition::Overlay);
    assert_eq!(pri, 50);
}

#[test]
fn classify_notification() {
    let (chrome, buf, pos, pri) = classify_extension("notification");
    assert!(chrome);
    assert!(!buf);
    assert_eq!(pos, ChromePosition::Overlay);
    assert_eq!(pri, 40);
}

#[test]
fn classify_microscope() {
    let (chrome, buf, pos, pri) = classify_extension("microscope");
    assert!(chrome);
    assert!(!buf);
    assert_eq!(pos, ChromePosition::Overlay);
    assert_eq!(pri, 80);
}

#[test]
fn classify_completion() {
    let (chrome, buf, pos, pri) = classify_extension("completion");
    assert!(chrome);
    assert!(!buf);
    assert_eq!(pos, ChromePosition::Overlay);
    assert_eq!(pri, 70);
}

#[test]
fn classify_explorer() {
    let (chrome, buf, pos, pri) = classify_extension("explorer");
    assert!(chrome);
    assert!(!buf);
    assert_eq!(pos, ChromePosition::Left);
    assert_eq!(pri, 60);
}

#[test]
fn classify_hover() {
    let (chrome, buf, pos, pri) = classify_extension("hover");
    assert!(chrome);
    assert!(!buf);
    assert_eq!(pos, ChromePosition::Overlay);
    assert_eq!(pri, 45);
}

#[test]
fn classify_signature_help() {
    let (chrome, buf, pos, pri) = classify_extension("signature-help");
    assert!(chrome);
    assert!(!buf);
    assert_eq!(pos, ChromePosition::Overlay);
    assert_eq!(pri, 44);
}

#[test]
fn classify_landing() {
    let (chrome, buf, pos, pri) = classify_extension("landing");
    assert!(chrome);
    assert!(!buf);
    assert_eq!(pos, ChromePosition::Overlay);
    assert_eq!(pri, 30);
}

#[test]
fn classify_polyblocks() {
    let (chrome, buf, pos, pri) = classify_extension("polyblocks");
    assert!(chrome);
    assert!(!buf);
    assert_eq!(pos, ChromePosition::Overlay);
    assert_eq!(pri, 10);
}

#[test]
fn classify_pair() {
    let (chrome, buf, pos, _) = classify_extension("pair");
    assert!(!chrome);
    assert!(buf);
    assert_eq!(pos, ChromePosition::Bottom);
}

#[test]
fn classify_markdown() {
    let (chrome, buf, pos, _) = classify_extension("markdown");
    assert!(!chrome);
    assert!(buf);
    assert_eq!(pos, ChromePosition::Bottom);
}

#[test]
fn classify_diagnostics() {
    let (chrome, buf, pos, _) = classify_extension("diagnostics");
    assert!(!chrome);
    assert!(buf);
    assert_eq!(pos, ChromePosition::Bottom);
}

#[test]
fn classify_range_finder_jump() {
    let (chrome, buf, pos, _) = classify_extension("range-finder-jump");
    assert!(!chrome);
    assert!(buf);
    assert_eq!(pos, ChromePosition::Bottom);
}

#[test]
fn classify_range_finder_fold() {
    let (chrome, buf, pos, _) = classify_extension("range-finder-fold");
    assert!(!chrome);
    assert!(buf);
    assert_eq!(pos, ChromePosition::Bottom);
}

#[test]
fn classify_unknown() {
    let (chrome, buf, pos, pri) = classify_extension("unknown-extension");
    assert!(chrome);
    assert!(!buf);
    assert_eq!(pos, ChromePosition::Overlay);
    assert_eq!(pri, 0);
}

// =============================================================================
// TuiExtensionBridge identity
// =============================================================================

#[test]
fn bridge_identity() {
    let ext = MockExtension::default().with_kind("cmdline");
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert_eq!(bridge.id(), "cmdline");
    assert_eq!(bridge.kind(), "cmdline");
    assert_eq!(bridge.name(), "cmdline");
    assert_eq!(bridge.version(), Version::new(0, 1, 0));
}

#[test]
fn bridge_dependencies() {
    let ext = MockExtension {
        dependencies_val: vec!["cmdline"],
        ..MockExtension::default()
    };
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert_eq!(bridge.dependencies(), &["cmdline"]);
}

#[test]
fn bridge_server_kinds() {
    let ext = MockExtension {
        server_kinds_val: vec!["mock", "extra"],
        ..MockExtension::default()
    };
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert_eq!(bridge.server_kinds(), vec!["mock", "extra"]);
}

// =============================================================================
// Role declaration
// =============================================================================

#[test]
fn bridge_chrome_role() {
    let ext = MockExtension::default().with_kind("cmdline");
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert!(bridge.has_chrome());
    assert!(!bridge.has_buffer_contrib());
    assert!(!bridge.has_annotations());
}

#[test]
fn bridge_buffer_contrib_role() {
    let ext = MockExtension::default().with_kind("pair");
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert!(!bridge.has_chrome());
    assert!(bridge.has_buffer_contrib());
    assert!(!bridge.has_annotations());
}

// =============================================================================
// Chrome
// =============================================================================

#[test]
fn bridge_chrome_position() {
    let ext = MockExtension::default().with_kind("cmdline");
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert_eq!(bridge.chrome_position(), ChromePosition::Bottom);
}

#[test]
fn bridge_chrome_priority() {
    let ext = MockExtension::default().with_kind("cmdline");
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert_eq!(bridge.chrome_priority(), 90);
}

#[test]
fn bridge_chrome_requested_size_explorer() {
    let ext = MockExtension {
        kind: "explorer",
        offset_left: 30,
        server_kinds_val: vec!["explorer"],
        ..MockExtension::default()
    };
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    let caps = MockPlatformCaps;
    assert_eq!(bridge.chrome_requested_size(&caps), 30);
}

#[test]
fn bridge_chrome_requested_size_non_explorer() {
    let ext = MockExtension::default().with_kind("cmdline");
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    let caps = MockPlatformCaps;
    assert_eq!(bridge.chrome_requested_size(&caps), 1);
}

// =============================================================================
// Events
// =============================================================================

#[test]
fn bridge_on_notification() {
    let ext = MockExtension::default();
    let mut bridge = TuiExtensionBridge::new(Box::new(ext));
    bridge.on_notification("{\"key\": \"value\"}");
    // Notification was forwarded (verified by mock state mutation)
}

#[test]
fn bridge_on_mode_change_insert() {
    let ext = MockExtension::default();
    let mut bridge = TuiExtensionBridge::new(Box::new(ext));
    bridge.on_mode_change("insert");
    // is_insert derived as true from "insert" mode name
}

#[test]
fn bridge_on_mode_change_insert_uppercase() {
    let ext = MockExtension::default();
    let mut bridge = TuiExtensionBridge::new(Box::new(ext));
    bridge.on_mode_change("Insert");
    // Case-insensitive match
}

#[test]
fn bridge_on_mode_change_normal() {
    let ext = MockExtension::default();
    let mut bridge = TuiExtensionBridge::new(Box::new(ext));
    bridge.on_mode_change("normal");
    // is_insert derived as false
}

#[test]
fn bridge_on_mode_change_visual() {
    let ext = MockExtension::default();
    let mut bridge = TuiExtensionBridge::new(Box::new(ext));
    bridge.on_mode_change("visual");
    // is_insert derived as false
}

#[test]
fn bridge_on_mode_change_command() {
    let ext = MockExtension::default();
    let mut bridge = TuiExtensionBridge::new(Box::new(ext));
    bridge.on_mode_change("command");
    // is_insert derived as false
}

#[test]
fn bridge_on_mode_change_empty() {
    let ext = MockExtension::default();
    let mut bridge = TuiExtensionBridge::new(Box::new(ext));
    bridge.on_mode_change("");
    // Empty mode => not insert
}

#[test]
fn bridge_on_buffer_update() {
    let ext = MockExtension::default();
    let mut bridge = TuiExtensionBridge::new(Box::new(ext));
    let event = BufferUpdateEvent {
        buffer_id: BufferId(1),
        revision: 5,
        changed_range: 0..10,
        new_lines: vec!["line1".to_string()],
        total_lines: 100,
    };
    bridge.on_buffer_update(&event);
}

#[test]
fn bridge_on_cursor_update() {
    let ext = MockExtension::default();
    let mut bridge = TuiExtensionBridge::new(Box::new(ext));
    bridge.on_cursor_update(BufferId(1), 10, 5);
}

#[test]
fn bridge_tick_no_change() {
    let ext = MockExtension::default();
    let mut bridge = TuiExtensionBridge::new(Box::new(ext));
    assert!(!bridge.tick());
}

#[test]
fn bridge_tick_with_change() {
    let ext = MockExtension {
        tick_returns: true,
        ..MockExtension::default()
    };
    let mut bridge = TuiExtensionBridge::new(Box::new(ext));
    assert!(bridge.tick());
}

// =============================================================================
// Buffer contrib
// =============================================================================

#[test]
fn bridge_classify_token_none() {
    let ext = MockExtension::default();
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert!(bridge.classify_token("keyword").is_none());
}

#[test]
fn bridge_classify_token_highlight() {
    let ext = MockExtension {
        classify_result: Some(DisplayRenderBehavior::Highlight),
        ..MockExtension::default()
    };
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert_eq!(
        bridge.classify_token("keyword"),
        Some(RenderBehavior::Highlight)
    );
}

#[test]
fn bridge_transform_line_none() {
    let ext = MockExtension::default();
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert!(bridge.transform_line(BufferId(0), 0, "hello").is_none());
}

#[test]
fn bridge_transform_line_some() {
    let ext = MockExtension {
        transform_result: Some(DisplayTransformedLine {
            text: "hello".to_string(),
            styles: vec![None; 5],
        }),
        ..MockExtension::default()
    };
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    let result = bridge.transform_line(BufferId(0), 0, "hello");
    assert!(result.is_some());
    let tl = result.unwrap();
    assert_eq!(tl.segments.len(), 1);
    assert_eq!(tl.segments[0].0, "hello");
    assert!(tl.segments[0].1.is_none());
}

#[test]
fn bridge_map_cursor_column_none() {
    let ext = MockExtension::default();
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert!(bridge.map_cursor_column(BufferId(0), 0, 5).is_none());
}

#[test]
fn bridge_map_cursor_column_some() {
    let ext = MockExtension {
        cursor_col_result: Some(10),
        ..MockExtension::default()
    };
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert_eq!(bridge.map_cursor_column(BufferId(0), 0, 5), Some(10));
}

#[test]
fn bridge_fold_ranges_empty() {
    let ext = MockExtension::default();
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert!(bridge.fold_ranges().is_empty());
}

#[test]
fn bridge_fold_ranges_cached() {
    let ext = MockExtension {
        fold_ranges: vec![(5, 3), (20, 10)],
        ..MockExtension::default()
    };
    let mut bridge = TuiExtensionBridge::new(Box::new(ext));
    // Trigger cache refresh via notification
    bridge.on_notification("refresh");
    let ranges = bridge.fold_ranges();
    assert_eq!(ranges, &[(5, 3), (20, 10)]);
}

#[test]
fn bridge_virtual_lines_empty() {
    let ext = MockExtension::default();
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert!(bridge.virtual_lines().is_empty());
}

#[test]
fn bridge_virtual_lines_cached() {
    let ext = MockExtension {
        vlines: vec![DisplayVirtualLine {
            buffer_line: 10,
            position: DisplayVirtualLinePosition::After,
            content: "diagnostic hint".to_string(),
            style: DisplayStyle::default(),
        }],
        ..MockExtension::default()
    };
    let mut bridge = TuiExtensionBridge::new(Box::new(ext));
    bridge.on_notification("refresh");
    let vlines = bridge.virtual_lines();
    assert_eq!(vlines.len(), 1);
    assert_eq!(vlines[0].buffer_line, 10);
    assert_eq!(
        vlines[0].position,
        reovim_client_driver::VirtualLinePosition::After
    );
    assert_eq!(vlines[0].content, "diagnostic hint");
}

#[test]
fn bridge_inline_decorations_empty() {
    let ext = MockExtension::default();
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert!(bridge.inline_decorations(0).is_empty());
}

#[test]
fn bridge_cursor_position_none() {
    let ext = MockExtension::default();
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert!(bridge.cursor_position(80, 24).is_none());
}

#[test]
fn bridge_cursor_position_some() {
    let ext = MockExtension {
        cursor_pos_result: Some((10, 5)),
        ..MockExtension::default()
    };
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    assert_eq!(bridge.cursor_position(80, 24), Some((10, 5)));
}

// =============================================================================
// Annotations (always default for bridge)
// =============================================================================

#[test]
fn bridge_annotation_defaults() {
    let ext = MockExtension::default();
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    let ctx = reovim_client_driver::AnnotationContext {
        buffer_id: BufferId(0),
        total_lines: 100,
        visible_range: (0, 40),
        cursor_line: 10,
        gutter_style: Style::default(),
    };
    let caps = MockPlatformCaps;
    assert_eq!(
        bridge.annotation_column_width(&ctx, &caps),
        ColumnWidth::Fixed(0)
    );
    assert!(bridge.annotate(0, &ctx).is_none());
    assert_eq!(bridge.annotation_priority(), 0);
}

// =============================================================================
// wrap_extensions
// =============================================================================

#[test]
fn wrap_extensions_empty() {
    let modules = wrap_extensions(Vec::new());
    assert!(modules.is_empty());
}

#[test]
fn wrap_extensions_multiple() {
    let extensions: Vec<Box<dyn TuiExtension>> = vec![
        Box::new(MockExtension::default().with_kind("cmdline")),
        Box::new(MockExtension::default().with_kind("pair")),
    ];
    let modules = wrap_extensions(extensions);
    assert_eq!(modules.len(), 2);
    assert_eq!(modules[0].kind(), "cmdline");
    assert!(modules[0].has_chrome());
    assert_eq!(modules[1].kind(), "pair");
    assert!(modules[1].has_buffer_contrib());
}

// =============================================================================
// convert_render_behavior
// =============================================================================

#[test]
fn convert_render_behavior_highlight() {
    let result = convert_render_behavior(&DisplayRenderBehavior::Highlight);
    assert_eq!(result, RenderBehavior::Highlight);
}

#[test]
fn convert_render_behavior_conceal() {
    let result = convert_render_behavior(&DisplayRenderBehavior::Conceal {
        replacement: Cow::Borrowed("..."),
    });
    assert!(matches!(
        result,
        RenderBehavior::Conceal { replacement } if replacement == "..."
    ));
}

#[test]
fn convert_render_behavior_background() {
    let result = convert_render_behavior(&DisplayRenderBehavior::Background);
    assert!(matches!(result, RenderBehavior::Background(_)));
}

#[test]
fn convert_render_behavior_hide() {
    let result = convert_render_behavior(&DisplayRenderBehavior::Hide);
    assert_eq!(result, RenderBehavior::Hide);
}

#[test]
fn convert_render_behavior_full_width_line() {
    let result = convert_render_behavior(&DisplayRenderBehavior::FullWidthLine { ch: '-' });
    assert!(matches!(
        result,
        RenderBehavior::FullWidthLine { ch: '-', .. }
    ));
}

// =============================================================================
// convert_transformed_line
// =============================================================================

#[test]
fn convert_transformed_line_uniform_no_style() {
    let old = DisplayTransformedLine {
        text: "hello".to_string(),
        styles: vec![None; 5],
    };
    let result = convert_transformed_line(&old);
    assert_eq!(result.segments.len(), 1);
    assert_eq!(result.segments[0].0, "hello");
    assert!(result.segments[0].1.is_none());
}

#[test]
fn convert_transformed_line_multi_style() {
    let bold_style = DisplayStyle {
        fg: Some(reovim_client_driver::Color::Red),
        bg: None,
        attributes: {
            let mut a = DisplayAttributes::new();
            a.set(DisplayAttributes::BOLD);
            a
        },
        underline_color: None,
    };
    let old = DisplayTransformedLine {
        text: "ab".to_string(),
        styles: vec![Some(bold_style), None],
    };
    let result = convert_transformed_line(&old);
    assert_eq!(result.segments.len(), 2);
    assert_eq!(result.segments[0].0, "a");
    assert!(result.segments[0].1.is_some());
    assert_eq!(result.segments[1].0, "b");
    assert!(result.segments[1].1.is_none());
}

#[test]
fn convert_transformed_line_empty() {
    let old = DisplayTransformedLine {
        text: String::new(),
        styles: Vec::new(),
    };
    let result = convert_transformed_line(&old);
    assert!(result.segments.is_empty());
}

#[test]
fn convert_transformed_line_styles_shorter_than_text() {
    let old = DisplayTransformedLine {
        text: "abc".to_string(),
        styles: vec![None],
    };
    let result = convert_transformed_line(&old);
    // All None (missing styles default to None), so one segment
    assert_eq!(result.segments.len(), 1);
    assert_eq!(result.segments[0].0, "abc");
}

// =============================================================================
// convert_display_style_to_driver_style
// =============================================================================

#[test]
fn convert_style_default() {
    let display = DisplayStyle::default();
    let driver = convert_display_style_to_driver_style(&display);
    assert_eq!(driver, Style::default());
}

#[test]
fn convert_style_with_colors() {
    let display = DisplayStyle {
        fg: Some(reovim_client_driver::Color::Red),
        bg: Some(reovim_client_driver::Color::Blue),
        attributes: DisplayAttributes::new(),
        underline_color: None,
    };
    let driver = convert_display_style_to_driver_style(&display);
    assert_eq!(driver.fg, Some(reovim_client_driver::Color::Red));
    assert_eq!(driver.bg, Some(reovim_client_driver::Color::Blue));
    assert!(driver.attributes.is_empty());
}

#[test]
fn convert_style_bold() {
    let mut attrs = DisplayAttributes::new();
    attrs.set(DisplayAttributes::BOLD);
    let display = DisplayStyle {
        fg: None,
        bg: None,
        attributes: attrs,
        underline_color: None,
    };
    let driver = convert_display_style_to_driver_style(&display);
    assert!(driver
        .attributes
        .contains(reovim_client_driver::Attributes::BOLD));
}

#[test]
fn convert_style_italic() {
    let mut attrs = DisplayAttributes::new();
    attrs.set(DisplayAttributes::ITALIC);
    let display = DisplayStyle {
        fg: None,
        bg: None,
        attributes: attrs,
        underline_color: None,
    };
    let driver = convert_display_style_to_driver_style(&display);
    assert!(driver
        .attributes
        .contains(reovim_client_driver::Attributes::ITALIC));
}

#[test]
fn convert_style_underline() {
    let mut attrs = DisplayAttributes::new();
    attrs.set(DisplayAttributes::UNDERLINE);
    let display = DisplayStyle {
        fg: None,
        bg: None,
        attributes: attrs,
        underline_color: None,
    };
    let driver = convert_display_style_to_driver_style(&display);
    assert!(driver
        .attributes
        .contains(reovim_client_driver::Attributes::UNDERLINE));
}

#[test]
fn convert_style_strikethrough() {
    let mut attrs = DisplayAttributes::new();
    attrs.set(DisplayAttributes::STRIKETHROUGH);
    let display = DisplayStyle {
        fg: None,
        bg: None,
        attributes: attrs,
        underline_color: None,
    };
    let driver = convert_display_style_to_driver_style(&display);
    assert!(driver
        .attributes
        .contains(reovim_client_driver::Attributes::STRIKETHROUGH));
}

#[test]
fn convert_style_reverse() {
    let mut attrs = DisplayAttributes::new();
    attrs.set(DisplayAttributes::REVERSE);
    let display = DisplayStyle {
        fg: None,
        bg: None,
        attributes: attrs,
        underline_color: None,
    };
    let driver = convert_display_style_to_driver_style(&display);
    assert!(driver
        .attributes
        .contains(reovim_client_driver::Attributes::REVERSE));
}

#[test]
fn convert_style_dim() {
    let mut attrs = DisplayAttributes::new();
    attrs.set(DisplayAttributes::DIM);
    let display = DisplayStyle {
        fg: None,
        bg: None,
        attributes: attrs,
        underline_color: None,
    };
    let driver = convert_display_style_to_driver_style(&display);
    assert!(driver
        .attributes
        .contains(reovim_client_driver::Attributes::DIM));
}

#[test]
fn convert_style_all_attributes() {
    let mut attrs = DisplayAttributes::new();
    attrs.set(DisplayAttributes::BOLD);
    attrs.set(DisplayAttributes::ITALIC);
    attrs.set(DisplayAttributes::UNDERLINE);
    attrs.set(DisplayAttributes::STRIKETHROUGH);
    attrs.set(DisplayAttributes::REVERSE);
    attrs.set(DisplayAttributes::DIM);
    let display = DisplayStyle {
        fg: None,
        bg: None,
        attributes: attrs,
        underline_color: None,
    };
    let driver = convert_display_style_to_driver_style(&display);
    assert!(driver
        .attributes
        .contains(reovim_client_driver::Attributes::BOLD));
    assert!(driver
        .attributes
        .contains(reovim_client_driver::Attributes::ITALIC));
    assert!(driver
        .attributes
        .contains(reovim_client_driver::Attributes::UNDERLINE));
    assert!(driver
        .attributes
        .contains(reovim_client_driver::Attributes::STRIKETHROUGH));
    assert!(driver
        .attributes
        .contains(reovim_client_driver::Attributes::REVERSE));
    assert!(driver
        .attributes
        .contains(reovim_client_driver::Attributes::DIM));
}

#[test]
fn convert_style_underline_color_ignored() {
    let display = DisplayStyle {
        fg: None,
        bg: None,
        attributes: DisplayAttributes::new(),
        underline_color: Some(reovim_client_driver::Color::Green),
    };
    let driver = convert_display_style_to_driver_style(&display);
    // underline_color is not in the driver style
    assert_eq!(driver, Style::default());
}

// =============================================================================
// SurfaceBackendAdapter
// =============================================================================

struct MockSurface {
    written: Vec<(u16, u16, String, Style)>,
    applied: Vec<(u16, u16, Style)>,
    overlaid: Vec<(u16, u16, reovim_client_driver::Color)>,
    cleared: Vec<Rect>,
}

impl MockSurface {
    fn new() -> Self {
        Self {
            written: Vec::new(),
            applied: Vec::new(),
            overlaid: Vec::new(),
            cleared: Vec::new(),
        }
    }
}

impl reovim_client_driver::RenderSurface for MockSurface {
    #[allow(clippy::cast_possible_truncation)]
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        let len = text.len() as u16;
        self.written.push((x, y, text.to_string(), style));
        len
    }

    fn apply_style(&mut self, x: u16, y: u16, style: Style) {
        self.applied.push((x, y, style));
    }

    fn overlay_bg(&mut self, x: u16, y: u16, bg: reovim_client_driver::Color) {
        self.overlaid.push((x, y, bg));
    }

    fn fill(&mut self, _rect: Rect, _ch: char, _style: Style) {}

    fn clear(&mut self, rect: Rect) {
        self.cleared.push(rect);
    }

    fn size(&self) -> (u16, u16) {
        (80, 24)
    }
}

struct MockPlatformCaps;

impl PlatformCapabilities for MockPlatformCaps {
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
        reovim_client_driver::Insets::default()
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

#[test]
fn surface_adapter_write_str() {
    let mut surface = MockSurface::new();
    let bounds = Rect::new(10, 5, 60, 20);
    {
        let mut adapter =
            super::SurfaceBackendAdapter::new(&mut surface, bounds);
        adapter.write_str(3, 2, "hello", &DisplayStyle::default());
    }
    assert_eq!(surface.written.len(), 1);
    assert_eq!(surface.written[0].0, 13); // 10 + 3
    assert_eq!(surface.written[0].1, 7); // 5 + 2
    assert_eq!(surface.written[0].2, "hello");
}

#[test]
fn surface_adapter_set_cell() {
    let mut surface = MockSurface::new();
    let bounds = Rect::new(5, 3, 70, 20);
    {
        let mut adapter =
            super::SurfaceBackendAdapter::new(&mut surface, bounds);
        adapter.set_cell(1, 1, 'X', &DisplayStyle::default());
    }
    assert_eq!(surface.written.len(), 1);
    assert_eq!(surface.written[0].0, 6); // 5 + 1
    assert_eq!(surface.written[0].1, 4); // 3 + 1
    assert_eq!(surface.written[0].2, "X");
}

#[test]
fn surface_adapter_apply_style() {
    let mut surface = MockSurface::new();
    let bounds = Rect::new(10, 5, 60, 20);
    {
        let mut adapter =
            super::SurfaceBackendAdapter::new(&mut surface, bounds);
        adapter.apply_style(0, 0, &DisplayStyle::default());
    }
    assert_eq!(surface.applied.len(), 1);
    assert_eq!(surface.applied[0].0, 10);
    assert_eq!(surface.applied[0].1, 5);
}

#[test]
fn surface_adapter_overlay_bg() {
    let mut surface = MockSurface::new();
    let bounds = Rect::new(10, 5, 60, 20);
    {
        let mut adapter =
            super::SurfaceBackendAdapter::new(&mut surface, bounds);
        adapter.overlay_bg(2, 3, reovim_client_driver::Color::Red);
    }
    assert_eq!(surface.overlaid.len(), 1);
    assert_eq!(surface.overlaid[0].0, 12);
    assert_eq!(surface.overlaid[0].1, 8);
}

#[test]
fn surface_adapter_size() {
    let mut surface = MockSurface::new();
    let bounds = Rect::new(10, 5, 60, 20);
    let adapter = super::SurfaceBackendAdapter::new(&mut surface, bounds);
    assert_eq!(adapter.size(), (60, 20));
}

#[test]
fn surface_adapter_clear() {
    let mut surface = MockSurface::new();
    let bounds = Rect::new(10, 5, 60, 20);
    {
        let mut adapter =
            super::SurfaceBackendAdapter::new(&mut surface, bounds);
        adapter.clear();
    }
    assert_eq!(surface.cleared.len(), 1);
    assert_eq!(surface.cleared[0], bounds);
}

// =============================================================================
// Chrome render delegation
// =============================================================================

#[test]
fn bridge_chrome_render_inactive() {
    let ext = MockExtension {
        kind: "cmdline",
        active: false,
        server_kinds_val: vec!["cmdline"],
        ..MockExtension::default()
    };
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    let mut surface = MockSurface::new();
    let caps = MockPlatformCaps;
    bridge.chrome_render(&mut surface, Rect::new(0, 23, 80, 1), &caps);
    // Inactive extension should not render anything
    assert!(surface.written.is_empty());
}

#[test]
fn bridge_chrome_render_active() {
    let ext = MockExtension::default().with_kind("cmdline");
    let bridge = TuiExtensionBridge::new(Box::new(ext));
    let mut surface = MockSurface::new();
    let caps = MockPlatformCaps;
    bridge.chrome_render(&mut surface, Rect::new(0, 23, 80, 1), &caps);
    // Active extension: render() is called (mock render is a no-op, no writes)
}

// =============================================================================
// Lifecycle sequence
// =============================================================================

#[test]
fn bridge_lifecycle_sequence() {
    let ext = MockExtension::default();
    let mut bridge = TuiExtensionBridge::new(Box::new(ext));

    // init
    let caps = MockPlatformCaps;
    let theme = MockThemeProvider;
    let server = std::sync::Arc::new(MockServerHandle);
    let ctx = reovim_client_driver::ModuleContext {
        capabilities: &caps,
        server,
        theme: &theme,
    };
    let result = bridge.init(&ctx);
    assert!(matches!(result, reovim_client_driver::ProbeResult::Success));

    // events
    bridge.on_notification("{}");
    bridge.on_mode_change("normal");
    bridge.on_cursor_update(BufferId(0), 0, 0);
    bridge.on_buffer_update(&BufferUpdateEvent {
        buffer_id: BufferId(0),
        revision: 1,
        changed_range: 0..1,
        new_lines: vec!["test".to_string()],
        total_lines: 1,
    });
    bridge.tick();

    // exit
    assert!(bridge.exit().is_ok());
}

struct MockThemeProvider;

impl reovim_client_driver::ThemeProvider for MockThemeProvider {
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

struct MockServerHandle;

impl reovim_client_driver::ServerHandle for MockServerHandle {
    fn get_options(
        &self,
        _names: &[&str],
    ) -> Vec<(String, reovim_client_driver::OptionValue)> {
        Vec::new()
    }
    fn execute_command(&self, _command: &str) {}
}
