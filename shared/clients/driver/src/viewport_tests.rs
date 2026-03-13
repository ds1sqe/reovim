use std::borrow::Cow;
use std::cell::RefCell;

use super::*;
use crate::{
    BufferId, CursorInfo, LineNumberMode, SyntaxToken, ViewportContext, ViewportRenderer,
    VirtualLine, VirtualLinePosition,
};

// =============================================================================
// Test infrastructure
// =============================================================================

/// Recording surface that captures all rendering calls.
struct RecordingSurface {
    width: u16,
    height: u16,
    writes: RefCell<Vec<(u16, u16, String, Style)>>,
    overlays: RefCell<Vec<(u16, u16, reovim_arch::Color)>>,
}

impl RecordingSurface {
    fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            writes: RefCell::new(Vec::new()),
            overlays: RefCell::new(Vec::new()),
        }
    }

    fn text_at(&self, y: u16) -> String {
        let writes = self.writes.borrow();
        let mut chars: Vec<(u16, String)> = writes
            .iter()
            .filter(|(_, wy, _, _)| *wy == y)
            .map(|(wx, _, text, _)| (*wx, text.clone()))
            .collect();
        chars.sort_by_key(|(x, _)| *x);
        chars.into_iter().map(|(_, t)| t).collect()
    }
}

impl RenderSurface for RecordingSurface {
    #[allow(clippy::cast_possible_truncation)]
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        let width = text.chars().count() as u16;
        self.writes
            .borrow_mut()
            .push((x, y, text.to_owned(), style));
        width
    }

    fn apply_style(&mut self, _x: u16, _y: u16, _style: Style) {}

    fn overlay_bg(&mut self, x: u16, y: u16, bg: reovim_arch::Color) {
        self.overlays.borrow_mut().push((x, y, bg));
    }

    fn fill(&mut self, _rect: Rect, _ch: char, _style: Style) {}

    fn clear(&mut self, _rect: Rect) {}

    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}

struct MockTokenProvider {
    tokens: Vec<SyntaxToken>,
}

impl MockTokenProvider {
    fn empty() -> Self {
        Self { tokens: Vec::new() }
    }

    fn with_tokens(tokens: Vec<SyntaxToken>) -> Self {
        Self { tokens }
    }
}

impl TokenProvider for MockTokenProvider {
    fn tokens_for_line(&self, _buffer_id: BufferId, _line: u32) -> Vec<SyntaxToken> {
        self.tokens.clone()
    }
}

struct MockTheme;

impl ThemeProvider for MockTheme {
    fn highlight(&self, group: &str) -> Style {
        match group {
            "keyword" => Style::new().fg(reovim_arch::Color::Blue),
            "string" => Style::new().fg(reovim_arch::Color::Green),
            "bg.token" => Style::new().bg(reovim_arch::Color::Red),
            _ => Style::default(),
        }
    }

    fn highlight_with_fallback(&self, groups: &[&str]) -> Style {
        groups
            .first()
            .map_or_else(Style::default, |g| self.highlight(g))
    }

    fn foreground(&self) -> Style {
        Style::new().fg(reovim_arch::Color::White)
    }

    fn background(&self) -> Style {
        Style::new().bg(reovim_arch::Color::Black)
    }

    fn is_dark(&self) -> bool {
        true
    }
}

fn empty_ctx<'a>() -> ViewportContext<'a> {
    ViewportContext {
        buffer_id: None,
        buffer_lines: None,
        cursor: None,
        scroll_top: 0,
        local_selection: None,
        remote_clients: &[],
        fold_ranges: &[],
        virtual_lines: &[],
        opacity: 1.0,
        line_number_mode: LineNumberMode::None,
        gutter_width: 0,
        sidebar_width: 0,
        is_insert_mode: false,
        render_self_cursor: false,
        my_client_id: 0,
    }
}

// =============================================================================
// is_line_folded
// =============================================================================

#[test]
fn is_line_folded_empty_ranges() {
    assert!(!is_line_folded(0, &[]));
}

#[test]
fn is_line_folded_in_range() {
    let folds = [(5, 3)]; // lines 5, 6, 7 folded
    assert!(!is_line_folded(4, &folds));
    assert!(is_line_folded(5, &folds));
    assert!(is_line_folded(6, &folds));
    assert!(is_line_folded(7, &folds));
    assert!(!is_line_folded(8, &folds));
}

#[test]
fn is_line_folded_multiple_ranges() {
    let folds = [(2, 2), (10, 1)]; // lines 2-3 and 10 folded
    assert!(is_line_folded(2, &folds));
    assert!(is_line_folded(3, &folds));
    assert!(!is_line_folded(4, &folds));
    assert!(is_line_folded(10, &folds));
    assert!(!is_line_folded(11, &folds));
}

// =============================================================================
// buffer_to_screen_row_vl
// =============================================================================

#[test]
fn buffer_to_screen_row_no_virtual_lines() {
    assert_eq!(buffer_to_screen_row_vl(5, 0, &[]), 5);
    assert_eq!(buffer_to_screen_row_vl(10, 5, &[]), 5);
}

#[test]
fn buffer_to_screen_row_with_virtual_lines() {
    let vl = vec![VirtualLine {
        buffer_line: 3,
        position: VirtualLinePosition::Before,
        content: "diag".to_string(),
        style: Style::default(),
    }];
    // VL at line 3, so lines >= 3 shift down by 1
    assert_eq!(buffer_to_screen_row_vl(2, 0, &vl), 2);
    assert_eq!(buffer_to_screen_row_vl(3, 0, &vl), 4); // +1 for VL
    assert_eq!(buffer_to_screen_row_vl(5, 0, &vl), 6); // +1 for VL
}

#[test]
fn buffer_to_screen_row_vl_before_scroll() {
    let vl = vec![VirtualLine {
        buffer_line: 1,
        position: VirtualLinePosition::After,
        content: "x".to_string(),
        style: Style::default(),
    }];
    // VL at line 1, scroll_top=5, so VL is before visible area
    assert_eq!(buffer_to_screen_row_vl(5, 5, &vl), 0);
}

// =============================================================================
// apply_opacity
// =============================================================================

#[test]
fn apply_opacity_full() {
    let style = Style::new().fg(reovim_arch::Color::Red);
    let result = apply_opacity(&style, 1.0);
    assert_eq!(result, style);
}

#[test]
fn apply_opacity_partial() {
    let style = Style::new().fg(reovim_arch::Color::White);
    let result = apply_opacity(&style, 0.5);
    // Should be dimmed but not fully black
    assert!(result.fg.is_some());
    assert_ne!(result, style);
}

// =============================================================================
// classify_with_modules
// =============================================================================

#[test]
fn classify_with_modules_no_modules() {
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    assert_eq!(
        classify_with_modules(&modules, "keyword"),
        RenderBehavior::Highlight
    );
}

#[test]
fn classify_with_modules_no_buffer_contrib() {
    struct NonContribModule;
    impl ClientModule for NonContribModule {
        fn id(&self) -> &'static str {
            "non-contrib"
        }
        fn name(&self) -> &'static str {
            "Non-Contrib"
        }
        fn version(&self) -> crate::Version {
            crate::Version::new(1, 0, 0)
        }
        fn init(&mut self, _ctx: &crate::ModuleContext) -> crate::ProbeResult {
            crate::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), crate::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            false
        }
    }
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(NonContribModule)];
    assert_eq!(
        classify_with_modules(&modules, "keyword"),
        RenderBehavior::Highlight
    );
}

// =============================================================================
// transform_line
// =============================================================================

#[test]
fn transform_line_no_modules() {
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    assert!(transform_line(&modules, Some(BufferId(0)), 0, "hello").is_none());
}

#[test]
fn transform_line_no_buffer_id() {
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    assert!(transform_line(&modules, None, 0, "hello").is_none());
}

// =============================================================================
// render_virtual_line
// =============================================================================

#[test]
fn render_virtual_line_basic() {
    let mut surface = RecordingSurface::new(80, 24);
    let style = Style::new().fg(reovim_arch::Color::Grey);
    render_virtual_line(&mut surface, 4, 0, 20, "---table---", &style);
    let text = surface.text_at(0);
    assert_eq!(text, "---table---");
}

#[test]
fn render_virtual_line_truncated() {
    let mut surface = RecordingSurface::new(80, 24);
    render_virtual_line(&mut surface, 0, 0, 5, "hello world", &Style::default());
    let text = surface.text_at(0);
    assert_eq!(text, "hello");
}

// =============================================================================
// render_transformed_line
// =============================================================================

#[test]
fn render_transformed_line_single_segment() {
    let mut surface = RecordingSurface::new(80, 24);
    let transformed = TransformedLine {
        segments: vec![("hello".to_string(), None)],
    };
    render_transformed_line(&mut surface, 4, 0, 20, &transformed);
    let text = surface.text_at(0);
    assert_eq!(text, "hello");
}

#[test]
fn render_transformed_line_multi_segment() {
    let mut surface = RecordingSurface::new(80, 24);
    let style = Style::new().fg(reovim_arch::Color::Red);
    let transformed = TransformedLine {
        segments: vec![
            ("fn ".to_string(), Some(style)),
            ("main".to_string(), None),
        ],
    };
    render_transformed_line(&mut surface, 0, 0, 20, &transformed);
    let text = surface.text_at(0);
    assert_eq!(text, "fn main");
}

#[test]
fn render_transformed_line_truncated() {
    let mut surface = RecordingSurface::new(80, 24);
    let transformed = TransformedLine {
        segments: vec![("hello world foo bar".to_string(), None)],
    };
    render_transformed_line(&mut surface, 0, 0, 5, &transformed);
    let text = surface.text_at(0);
    assert_eq!(text, "hello");
}

// render_line_number tests moved to reovim-tui-mod-line-numbers

// =============================================================================
// render_line_content
// =============================================================================

#[test]
fn render_line_content_plain_text() {
    let mut surface = RecordingSurface::new(80, 24);
    let token_provider = MockTokenProvider::empty();
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_line_content(
        &mut surface,
        0,
        0,
        80,
        "hello",
        1.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );
    let text = surface.text_at(0);
    assert_eq!(text, "hello");
}

#[test]
fn render_line_content_with_highlight_tokens() {
    let mut surface = RecordingSurface::new(80, 24);
    let tokens = vec![SyntaxToken {
        line: 0,
        start_col: 0,
        end_col: 2,
        category: "keyword".to_owned(),
    }];
    let token_provider = MockTokenProvider::with_tokens(tokens);
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_line_content(
        &mut surface,
        0,
        0,
        80,
        "fn main",
        1.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );
    let text = surface.text_at(0);
    assert_eq!(text, "fn main");
    // First two chars should have keyword style (blue)
    let writes = surface.writes.borrow();
    assert_eq!(
        writes[0].3.fg,
        Some(reovim_arch::Color::Blue),
        "first char should be keyword-styled"
    );
}

#[test]
fn render_line_content_no_buffer_id() {
    let mut surface = RecordingSurface::new(80, 24);
    let token_provider = MockTokenProvider::empty();
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_line_content(
        &mut surface,
        0,
        0,
        80,
        "hello",
        1.0,
        None, // no buffer ID
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );
    let text = surface.text_at(0);
    assert_eq!(text, "hello");
}

#[test]
fn render_line_content_with_opacity() {
    let mut surface = RecordingSurface::new(80, 24);
    let token_provider = MockTokenProvider::empty();
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_line_content(
        &mut surface,
        0,
        0,
        80,
        "hi",
        0.5, // half opacity
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );
    let text = surface.text_at(0);
    assert_eq!(text, "hi");
}

#[test]
fn render_line_content_truncated() {
    let mut surface = RecordingSurface::new(80, 24);
    let token_provider = MockTokenProvider::empty();
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_line_content(
        &mut surface,
        0,
        0,
        3, // only 3 columns
        "hello world",
        1.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );
    let text = surface.text_at(0);
    assert_eq!(text, "hel");
}

#[test]
fn render_line_content_with_conceal() {
    struct ConcealModule;
    impl ClientModule for ConcealModule {
        fn id(&self) -> &'static str {
            "conceal"
        }
        fn name(&self) -> &'static str {
            "Conceal"
        }
        fn version(&self) -> crate::Version {
            crate::Version::new(1, 0, 0)
        }
        fn init(&mut self, _ctx: &crate::ModuleContext) -> crate::ProbeResult {
            crate::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), crate::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        fn classify_token(&self, category: &str) -> Option<RenderBehavior> {
            if category == "conceal.url" {
                Some(RenderBehavior::Conceal {
                    replacement: Cow::Borrowed("..."),
                })
            } else {
                None
            }
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let tokens = vec![SyntaxToken {
        line: 0,
        start_col: 5,
        end_col: 25,
        category: "conceal.url".to_owned(),
    }];
    let token_provider = MockTokenProvider::with_tokens(tokens);
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(ConcealModule)];
    render_line_content(
        &mut surface,
        0,
        0,
        80,
        "text http://example.com rest",
        1.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );
    let text = surface.text_at(0);
    assert!(text.contains("..."), "URL should be concealed: {text}");
    assert!(
        !text.contains("http"),
        "URL should not be visible: {text}"
    );
}

#[test]
fn render_line_content_conceal_skip_in_insert() {
    struct ConcealModule;
    impl ClientModule for ConcealModule {
        fn id(&self) -> &'static str {
            "conceal"
        }
        fn name(&self) -> &'static str {
            "Conceal"
        }
        fn version(&self) -> crate::Version {
            crate::Version::new(1, 0, 0)
        }
        fn init(&mut self, _ctx: &crate::ModuleContext) -> crate::ProbeResult {
            crate::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), crate::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        fn classify_token(&self, category: &str) -> Option<RenderBehavior> {
            if category == "conceal.url" {
                Some(RenderBehavior::Hide)
            } else {
                None
            }
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let tokens = vec![SyntaxToken {
        line: 0,
        start_col: 5,
        end_col: 10,
        category: "conceal.url".to_owned(),
    }];
    let token_provider = MockTokenProvider::with_tokens(tokens);
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(ConcealModule)];
    render_line_content(
        &mut surface,
        0,
        0,
        80,
        "hello world",
        1.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        true, // skip conceals (insert mode)
        &modules,
    );
    let text = surface.text_at(0);
    assert_eq!(text, "hello world", "conceals should be skipped in insert mode");
}

#[test]
fn render_line_content_with_background() {
    struct BgModule;
    impl ClientModule for BgModule {
        fn id(&self) -> &'static str {
            "bg"
        }
        fn name(&self) -> &'static str {
            "Bg"
        }
        fn version(&self) -> crate::Version {
            crate::Version::new(1, 0, 0)
        }
        fn init(&mut self, _ctx: &crate::ModuleContext) -> crate::ProbeResult {
            crate::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), crate::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        fn classify_token(&self, category: &str) -> Option<RenderBehavior> {
            if category == "bg.token" {
                Some(RenderBehavior::Background(reovim_arch::Color::Red))
            } else {
                None
            }
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let tokens = vec![SyntaxToken {
        line: 0,
        start_col: 0,
        end_col: 5,
        category: "bg.token".to_owned(),
    }];
    let token_provider = MockTokenProvider::with_tokens(tokens);
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(BgModule)];
    render_line_content(
        &mut surface,
        0,
        0,
        80,
        "hello",
        1.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );
    // Background overlays should have been generated
    let overlays = surface.overlays.borrow();
    assert!(!overlays.is_empty(), "should have background overlays");
}

#[test]
fn render_line_content_with_hide() {
    struct HideModule;
    impl ClientModule for HideModule {
        fn id(&self) -> &'static str {
            "hide"
        }
        fn name(&self) -> &'static str {
            "Hide"
        }
        fn version(&self) -> crate::Version {
            crate::Version::new(1, 0, 0)
        }
        fn init(&mut self, _ctx: &crate::ModuleContext) -> crate::ProbeResult {
            crate::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), crate::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        fn classify_token(&self, category: &str) -> Option<RenderBehavior> {
            if category == "hidden" {
                Some(RenderBehavior::Hide)
            } else {
                None
            }
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let tokens = vec![SyntaxToken {
        line: 0,
        start_col: 5,
        end_col: 11,
        category: "hidden".to_owned(),
    }];
    let token_provider = MockTokenProvider::with_tokens(tokens);
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(HideModule)];
    render_line_content(
        &mut surface,
        0,
        0,
        80,
        "hello world",
        1.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );
    let text = surface.text_at(0);
    assert_eq!(text, "hello", "hidden text should not be visible");
}

#[test]
fn render_line_content_full_width_line() {
    struct FwlModule;
    impl ClientModule for FwlModule {
        fn id(&self) -> &'static str {
            "fwl"
        }
        fn name(&self) -> &'static str {
            "FWL"
        }
        fn version(&self) -> crate::Version {
            crate::Version::new(1, 0, 0)
        }
        fn init(&mut self, _ctx: &crate::ModuleContext) -> crate::ProbeResult {
            crate::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), crate::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        fn classify_token(&self, category: &str) -> Option<RenderBehavior> {
            if category == "hr" {
                Some(RenderBehavior::FullWidthLine {
                    ch: '-',
                    style: Style::default(),
                })
            } else {
                None
            }
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let tokens = vec![SyntaxToken {
        line: 0,
        start_col: 0,
        end_col: 3,
        category: "hr".to_owned(),
    }];
    let token_provider = MockTokenProvider::with_tokens(tokens);
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(FwlModule)];
    render_line_content(
        &mut surface,
        0,
        0,
        10,
        "---",
        1.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );
    let text = surface.text_at(0);
    // Should have dashes repeated for width
    assert!(text.contains("----------"), "should be full-width dashes: {text}");
}

// =============================================================================
// render_buffer_content (integration)
// =============================================================================

#[test]
fn render_buffer_content_basic() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string(), "world".to_string()];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let token_provider = MockTokenProvider::empty();
    let viewport = Rect::new(0, 0, 80, 24);

    render_buffer_content(&mut surface, viewport, &ctx, &modules, &token_provider, &MockTheme);

    assert_eq!(surface.text_at(0), "hello");
    assert_eq!(surface.text_at(1), "world");
}

#[test]
fn render_buffer_content_with_scroll() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec![
        "line0".to_string(),
        "line1".to_string(),
        "line2".to_string(),
    ];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        scroll_top: 1,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let token_provider = MockTokenProvider::empty();
    let viewport = Rect::new(0, 0, 80, 24);

    render_buffer_content(&mut surface, viewport, &ctx, &modules, &token_provider, &MockTheme);

    assert_eq!(surface.text_at(0), "line1");
    assert_eq!(surface.text_at(1), "line2");
}

#[test]
fn render_buffer_content_past_end_shows_tilde() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string()];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let token_provider = MockTokenProvider::empty();
    let viewport = Rect::new(0, 0, 80, 3);

    render_buffer_content(&mut surface, viewport, &ctx, &modules, &token_provider, &MockTheme);

    assert_eq!(surface.text_at(0), "hello");
    assert_eq!(surface.text_at(1), "~");
}

#[test]
fn render_buffer_content_with_folds() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec![
        "line0".to_string(),
        "line1".to_string(),
        "line2".to_string(),
        "line3".to_string(),
    ];
    let folds = vec![(1, 2)]; // lines 1-2 folded
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        fold_ranges: &folds,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let token_provider = MockTokenProvider::empty();
    let viewport = Rect::new(0, 0, 80, 24);

    render_buffer_content(&mut surface, viewport, &ctx, &modules, &token_provider, &MockTheme);

    assert_eq!(surface.text_at(0), "line0");
    assert_eq!(surface.text_at(1), "line3");
}

#[test]
fn render_buffer_content_with_virtual_lines_before() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string(), "world".to_string()];
    let vlines = vec![VirtualLine {
        buffer_line: 1,
        position: VirtualLinePosition::Before,
        content: "-- virtual --".to_string(),
        style: Style::default(),
    }];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        virtual_lines: &vlines,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let token_provider = MockTokenProvider::empty();
    let viewport = Rect::new(0, 0, 80, 24);

    render_buffer_content(&mut surface, viewport, &ctx, &modules, &token_provider, &MockTheme);

    assert_eq!(surface.text_at(0), "hello");
    assert_eq!(surface.text_at(1), "-- virtual --");
    assert_eq!(surface.text_at(2), "world");
}

#[test]
fn render_buffer_content_with_virtual_lines_after() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string(), "world".to_string()];
    let vlines = vec![VirtualLine {
        buffer_line: 0,
        position: VirtualLinePosition::After,
        content: "-- after --".to_string(),
        style: Style::default(),
    }];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        virtual_lines: &vlines,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let token_provider = MockTokenProvider::empty();
    let viewport = Rect::new(0, 0, 80, 24);

    render_buffer_content(&mut surface, viewport, &ctx, &modules, &token_provider, &MockTheme);

    assert_eq!(surface.text_at(0), "hello");
    assert_eq!(surface.text_at(1), "-- after --");
    assert_eq!(surface.text_at(2), "world");
}

#[test]
fn render_buffer_content_with_gutter_annotations() {
    // Mock annotation module that returns line numbers
    struct MockAnnotation;
    impl ClientModule for MockAnnotation {
        fn id(&self) -> &'static str { "mock-ann" }
        fn name(&self) -> &'static str { "Mock" }
        fn version(&self) -> crate::Version { crate::Version::new(1, 0, 0) }
        fn init(&mut self, _ctx: &crate::ModuleContext) -> crate::ProbeResult {
            crate::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), crate::ClientModuleError> { Ok(()) }
        fn has_annotations(&self) -> bool { true }
        fn annotation_priority(&self) -> u16 { 100 }
        fn annotation_column_width(
            &self,
            _ctx: &crate::AnnotationContext,
            _caps: &dyn crate::PlatformCapabilities,
        ) -> crate::ColumnWidth {
            crate::ColumnWidth::Fixed(4)
        }
        fn annotate(&self, line: usize, _ctx: &crate::AnnotationContext) -> Option<crate::GutterCell> {
            Some(crate::GutterCell {
                text: (line + 1).to_string(),
                style: Style::new().fg(reovim_arch::Color::DarkGrey),
            })
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string(), "world".to_string()];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        cursor: Some(CursorInfo { line: 0, column: 0 }),
        gutter_width: 4,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(MockAnnotation)];
    let token_provider = MockTokenProvider::empty();
    let viewport = Rect::new(0, 0, 84, 24);

    render_buffer_content(&mut surface, viewport, &ctx, &modules, &token_provider, &MockTheme);

    // Line numbers from annotation module should be at x=0, content at x=4
    let row0 = surface.text_at(0);
    assert!(row0.contains('1'), "should have line number 1: {row0}");
    assert!(row0.contains("hello"), "should have content: {row0}");
}

#[test]
fn render_buffer_content_no_buffer_lines() {
    let mut surface = RecordingSurface::new(80, 24);
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: None, // no buffer content
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let token_provider = MockTokenProvider::empty();
    let viewport = Rect::new(0, 0, 80, 3);

    render_buffer_content(&mut surface, viewport, &ctx, &modules, &token_provider, &MockTheme);

    // Should render nothing (no buffer lines)
    assert!(surface.writes.borrow().is_empty());
}

// =============================================================================
// DefaultViewportRenderer (trait impl)
// =============================================================================

#[test]
fn default_viewport_renderer_gutter_width_no_annotations() {
    let renderer = DefaultViewportRenderer;
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let caps = TestCaps;
    assert_eq!(renderer.gutter_width(&modules, &caps), 0);
}

#[test]
fn default_viewport_renderer_render_viewport() {
    let renderer = DefaultViewportRenderer;
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string()];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let token_provider = MockTokenProvider::empty();
    let viewport = Rect::new(0, 0, 80, 24);

    renderer.render_viewport(
        &mut surface,
        viewport,
        &ctx,
        &modules,
        &token_provider,
        &MockTheme,
        &TestCaps,
    );

    assert_eq!(surface.text_at(0), "hello");
}

struct TestCaps;

impl PlatformCapabilities for TestCaps {
    fn rendering_model(&self) -> crate::RenderingModel {
        crate::RenderingModel::CellGrid
    }
    fn grid_size(&self) -> Option<(u16, u16)> {
        Some((80, 24))
    }
    fn color_depth(&self) -> crate::ColorDepth {
        crate::ColorDepth::TrueColor
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
    fn safe_area(&self) -> crate::Insets {
        crate::Insets::ZERO
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
// normalize_selection
// =============================================================================

#[test]
fn normalize_selection_already_normalized() {
    let sel = SelectionInfo {
        start_line: 1,
        start_col: 0,
        end_line: 3,
        end_col: 5,
        mode: crate::SelectionMode::Char,
        color: reovim_arch::Color::Blue,
    };
    let (sl, sc, el, ec) = normalize_selection(&sel);
    assert_eq!((sl, sc, el, ec), (1, 0, 3, 5));
}

#[test]
fn normalize_selection_reversed() {
    let sel = SelectionInfo {
        start_line: 5,
        start_col: 10,
        end_line: 2,
        end_col: 3,
        mode: crate::SelectionMode::Char,
        color: reovim_arch::Color::Blue,
    };
    let (sl, sc, el, ec) = normalize_selection(&sel);
    assert_eq!((sl, sc, el, ec), (2, 3, 5, 10));
}

// =============================================================================
// render_selections
// =============================================================================

#[test]
fn render_selections_no_selections() {
    let mut surface = RecordingSurface::new(80, 24);
    let ctx = empty_ctx();
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_selections(&mut surface, &ctx, 0, 24, &modules);
    assert!(surface.overlays.borrow().is_empty());
}

#[test]
fn render_selections_local_char_selection() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello world".to_string()];
    let local_sel = SelectionInfo {
        start_line: 0,
        start_col: 2,
        end_line: 0,
        end_col: 5,
        mode: crate::SelectionMode::Char,
        color: reovim_arch::Color::Blue,
    };
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        local_selection: Some(local_sel),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_selections(&mut surface, &ctx, 0, 24, &modules);
    // Should have overlays for columns 2..=5 (4 columns)
    let overlays = surface.overlays.borrow();
    assert_eq!(overlays.len(), 4);
}

#[test]
fn render_selections_line_mode() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string()];
    let local_sel = SelectionInfo {
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 0,
        mode: crate::SelectionMode::Line,
        color: reovim_arch::Color::Blue,
    };
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        local_selection: Some(local_sel),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_selections(&mut surface, &ctx, 0, 24, &modules);
    // Line mode highlights entire width (80 cols)
    let overlays = surface.overlays.borrow();
    assert_eq!(overlays.len(), 80);
}

#[test]
fn render_selections_remote_client() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello world".to_string()];
    let remote = crate::RemoteClientInfo {
        client_id: 1,
        display_name: "Alice".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        mode: "visual".to_string(),
        cursor_color: reovim_arch::Color::Blue,
        selection: Some(SelectionInfo {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 3,
            mode: crate::SelectionMode::Char,
            color: reovim_arch::Color::DarkBlue,
        }),
    };
    let remotes = vec![remote];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        remote_clients: &remotes,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_selections(&mut surface, &ctx, 0, 24, &modules);
    let overlays = surface.overlays.borrow();
    assert_eq!(overlays.len(), 4); // cols 0..=3
}

// =============================================================================
// render_remote_cursors
// =============================================================================

#[test]
fn render_remote_cursors_empty() {
    let mut surface = RecordingSurface::new(80, 24);
    let ctx = empty_ctx();
    render_remote_cursors(&mut surface, &ctx, 0, 24);
    // No style applications
    // (RecordingSurface doesn't record apply_style, but no crash)
}

#[test]
fn render_remote_cursors_visible() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string()];
    let remote = crate::RemoteClientInfo {
        client_id: 1,
        display_name: "Alice".to_string(),
        cursor_line: 0,
        cursor_col: 2,
        mode: "normal".to_string(),
        cursor_color: reovim_arch::Color::Blue,
        selection: None,
    };
    let remotes = vec![remote];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        remote_clients: &remotes,
        ..empty_ctx()
    };
    // No panic
    render_remote_cursors(&mut surface, &ctx, 0, 24);
}

#[test]
fn render_remote_cursors_before_scroll() {
    let mut surface = RecordingSurface::new(80, 24);
    let remote = crate::RemoteClientInfo {
        client_id: 1,
        display_name: "Bob".to_string(),
        cursor_line: 2,
        cursor_col: 0,
        mode: "normal".to_string(),
        cursor_color: reovim_arch::Color::Green,
        selection: None,
    };
    let remotes = vec![remote];
    let ctx = ViewportContext {
        scroll_top: 5,
        remote_clients: &remotes,
        ..empty_ctx()
    };
    // Cursor at line 2 but scroll_top=5, should be skipped
    render_remote_cursors(&mut surface, &ctx, 0, 24);
}

// =============================================================================
// render_self_cursor
// =============================================================================

#[test]
fn render_self_cursor_no_cursor() {
    let mut surface = RecordingSurface::new(80, 24);
    let ctx = ViewportContext {
        cursor: None,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let tokens = MockTokenProvider::empty();
    render_self_cursor(&mut surface, &ctx, 0, 24, &tokens, &MockTheme, &modules);
    // No panic, no writes
}

#[test]
fn render_self_cursor_visible() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string()];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        cursor: Some(CursorInfo { line: 0, column: 2 }),
        render_self_cursor: true,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let tokens = MockTokenProvider::empty();
    render_self_cursor(&mut surface, &ctx, 0, 24, &tokens, &MockTheme, &modules);
    // No panic (apply_style is a no-op on RecordingSurface)
}

#[test]
fn render_self_cursor_insert_mode() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string()];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        cursor: Some(CursorInfo { line: 0, column: 3 }),
        is_insert_mode: true,
        render_self_cursor: true,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let tokens = MockTokenProvider::empty();
    render_self_cursor(&mut surface, &ctx, 0, 24, &tokens, &MockTheme, &modules);
}

#[test]
fn render_self_cursor_before_scroll() {
    let mut surface = RecordingSurface::new(80, 24);
    let ctx = ViewportContext {
        cursor: Some(CursorInfo { line: 0, column: 0 }),
        scroll_top: 5,
        render_self_cursor: true,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let tokens = MockTokenProvider::empty();
    render_self_cursor(&mut surface, &ctx, 0, 24, &tokens, &MockTheme, &modules);
}

// =============================================================================
// compute_cursor_visual_col
// =============================================================================

#[test]
fn compute_cursor_visual_col_no_buffer() {
    let ctx = ViewportContext {
        buffer_id: None,
        ..empty_ctx()
    };
    let cursor = CursorInfo { line: 0, column: 5 };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let tokens = MockTokenProvider::empty();
    assert_eq!(
        compute_cursor_visual_col(&ctx, &cursor, &tokens, &MockTheme, &modules),
        5
    );
}

#[test]
fn compute_cursor_visual_col_no_content() {
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: None,
        ..empty_ctx()
    };
    let cursor = CursorInfo { line: 0, column: 3 };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let tokens = MockTokenProvider::empty();
    assert_eq!(
        compute_cursor_visual_col(&ctx, &cursor, &tokens, &MockTheme, &modules),
        3
    );
}

#[test]
fn compute_cursor_visual_col_no_conceals() {
    let lines = vec!["hello".to_string()];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        ..empty_ctx()
    };
    let cursor = CursorInfo { line: 0, column: 3 };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let tokens = MockTokenProvider::empty();
    assert_eq!(
        compute_cursor_visual_col(&ctx, &cursor, &tokens, &MockTheme, &modules),
        3
    );
}

// =============================================================================
// label_text
// =============================================================================

#[test]
fn label_text_normal() {
    let label = label_text("Alice", "normal");
    assert!(label.contains("Alice"));
    assert!(label.contains("[N]"));
}

#[test]
fn label_text_insert() {
    let label = label_text("Bob", "insert");
    assert!(label.contains("Bob"));
    assert!(label.contains("[I]"));
}

#[test]
fn label_text_visual() {
    let label = label_text("Carol", "visual");
    assert!(label.contains("[V]"));
}

#[test]
fn label_text_command() {
    let label = label_text("Dan", "command");
    assert!(label.contains("[C]"));
}

#[test]
fn label_text_replace() {
    let label = label_text("Eve", "replace");
    assert!(label.contains("[R]"));
}

#[test]
fn label_text_empty_name() {
    let label = label_text("", "normal");
    assert!(label.contains('?'));
}

// =============================================================================
// mode_abbreviation
// =============================================================================

#[test]
fn mode_abbreviation_variants() {
    assert_eq!(mode_abbreviation("normal"), "[N]");
    assert_eq!(mode_abbreviation("INSERT"), "[I]");
    assert_eq!(mode_abbreviation("visual line"), "[V]");
    assert_eq!(mode_abbreviation("cmdline"), "[C]");
    assert_eq!(mode_abbreviation("REPLACE"), "[R]");
    assert_eq!(mode_abbreviation("unknown"), "[N]");
}

// =============================================================================
// client_color
// =============================================================================

#[test]
fn client_color_palette() {
    let c0 = client_color(0);
    let c1 = client_color(1);
    assert_ne!(format!("{c0:?}"), format!("{c1:?}"));
    // Wraps around
    let c8 = client_color(8);
    assert_eq!(format!("{c0:?}"), format!("{c8:?}"));
}

// =============================================================================
// Selection + cursor integration with render_viewport
// =============================================================================

#[test]
fn render_viewport_with_local_selection() {
    let renderer = DefaultViewportRenderer;
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello world".to_string()];
    let local_sel = SelectionInfo {
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 4,
        mode: crate::SelectionMode::Char,
        color: reovim_arch::Color::Blue,
    };
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        local_selection: Some(local_sel),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let token_provider = MockTokenProvider::empty();
    let viewport = Rect::new(0, 0, 80, 24);

    renderer.render_viewport(
        &mut surface,
        viewport,
        &ctx,
        &modules,
        &token_provider,
        &MockTheme,
        &TestCaps,
    );

    // Should have text and selection overlays
    assert_eq!(surface.text_at(0), "hello world");
    let overlays = surface.overlays.borrow();
    assert_eq!(overlays.len(), 5, "should have 5 overlay columns (0..=4)");
}

#[test]
fn render_viewport_with_self_cursor() {
    let renderer = DefaultViewportRenderer;
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string()];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        cursor: Some(CursorInfo { line: 0, column: 2 }),
        render_self_cursor: true,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let token_provider = MockTokenProvider::empty();
    let viewport = Rect::new(0, 0, 80, 24);

    renderer.render_viewport(
        &mut surface,
        viewport,
        &ctx,
        &modules,
        &token_provider,
        &MockTheme,
        &TestCaps,
    );

    assert_eq!(surface.text_at(0), "hello");
}
