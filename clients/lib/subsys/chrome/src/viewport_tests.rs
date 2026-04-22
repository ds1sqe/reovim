use std::{borrow::Cow, cell::RefCell};

use {
    super::*,
    reovim_client_subsys_module::{
        BufferId, ChromeSurface, ClientModule, CursorInfo, LineNumberMode, Rect, RenderBehavior,
        SelectionInfo, Style, SyntaxToken, ThemeProvider, TokenProvider, ViewportContext,
        ViewportRenderer, VirtualLine, VirtualLinePosition,
    },
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

impl ChromeSurface for RecordingSurface {
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
            "comment" => Style::new().fg(reovim_arch::Color::DarkGreen),
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
    assert_eq!(classify_with_modules(&modules, "keyword"), RenderBehavior::Highlight);
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
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            false
        }
    }
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(NonContribModule)];
    assert_eq!(classify_with_modules(&modules, "keyword"), RenderBehavior::Highlight);
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
        segments: vec![("fn ".to_string(), Some(style)), ("main".to_string(), None)],
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
fn render_line_content_narrowest_token_wins() {
    // When a broad `comment` token and a narrow `keyword` token overlap,
    // the narrowest (most specific) token should win. This is the key
    // behavior for injection highlighting in doc comment code blocks.
    let mut surface = RecordingSurface::new(80, 24);
    let tokens = vec![
        SyntaxToken {
            line: 0,
            start_col: 0,
            end_col: 15, // broad: "comment" spans entire line
            category: "comment".to_owned(),
        },
        SyntaxToken {
            line: 0,
            start_col: 4,
            end_col: 6, // narrow: "keyword" spans just "fn"
            category: "keyword".to_owned(),
        },
    ];
    let token_provider = MockTokenProvider::with_tokens(tokens);
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_line_content(
        &mut surface,
        0,
        0,
        80,
        "/// fn main() {}",
        1.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );

    let writes = surface.writes.borrow();
    // Col 0-3 ("/// "): only comment overlaps → comment style (DarkGreen)
    assert_eq!(
        writes[0].3.fg,
        Some(reovim_arch::Color::DarkGreen),
        "col 0 should be comment-styled (DarkGreen)"
    );
    // Col 4 ("f"): both comment and keyword overlap → keyword wins (narrower)
    assert_eq!(
        writes[4].3.fg,
        Some(reovim_arch::Color::Blue),
        "col 4 should be keyword-styled (Blue), not comment"
    );
    // Col 6 ("m"): only comment overlaps again → comment style
    assert_eq!(
        writes[6].3.fg,
        Some(reovim_arch::Color::DarkGreen),
        "col 6 should be comment-styled"
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
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
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
    assert!(!text.contains("http"), "URL should not be visible: {text}");
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
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
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
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
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
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
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
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
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
        fn id(&self) -> &'static str {
            "mock-ann"
        }
        fn name(&self) -> &'static str {
            "Mock"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_annotations(&self) -> bool {
            true
        }
        fn annotation_priority(&self) -> u16 {
            100
        }
        fn annotation_column_width(
            &self,
            _ctx: &reovim_client_subsys_module::AnnotationContext,
            _caps: &dyn reovim_client_subsys_module::PlatformCapabilities,
        ) -> reovim_client_subsys_module::ColumnWidth {
            reovim_client_subsys_module::ColumnWidth::Fixed(4)
        }
        fn annotate(
            &self,
            line: usize,
            _ctx: &reovim_client_subsys_module::AnnotationContext,
        ) -> Option<reovim_client_subsys_module::GutterCell> {
            Some(reovim_client_subsys_module::GutterCell {
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
    fn rendering_model(&self) -> reovim_client_subsys_module::RenderingModel {
        reovim_client_subsys_module::RenderingModel::CellGrid
    }
    fn grid_size(&self) -> Option<(u16, u16)> {
        Some((80, 24))
    }
    fn color_depth(&self) -> reovim_client_subsys_module::ColorDepth {
        reovim_client_subsys_module::ColorDepth::TrueColor
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
    fn safe_area(&self) -> reovim_client_subsys_module::Insets {
        reovim_client_subsys_module::Insets::ZERO
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
        mode: reovim_client_subsys_module::SelectionMode::Char,
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
        mode: reovim_client_subsys_module::SelectionMode::Char,
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
        mode: reovim_client_subsys_module::SelectionMode::Char,
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
        mode: reovim_client_subsys_module::SelectionMode::Line,
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
    let remote = reovim_client_subsys_module::RemoteClientInfo {
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
            mode: reovim_client_subsys_module::SelectionMode::Char,
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
    let remote = reovim_client_subsys_module::RemoteClientInfo {
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
    let remote = reovim_client_subsys_module::RemoteClientInfo {
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
    assert_eq!(compute_cursor_visual_col(&ctx, &cursor, &tokens, &MockTheme, &modules), 5);
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
    assert_eq!(compute_cursor_visual_col(&ctx, &cursor, &tokens, &MockTheme, &modules), 3);
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
    assert_eq!(compute_cursor_visual_col(&ctx, &cursor, &tokens, &MockTheme, &modules), 3);
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
        mode: reovim_client_subsys_module::SelectionMode::Char,
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

// =============================================================================
// Coverage gap: multi-line Char selection (2-line)
// =============================================================================

#[test]
fn render_selection_char_two_lines() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string(), "world".to_string()];
    let local_sel = SelectionInfo {
        start_line: 0,
        start_col: 2,
        end_line: 1,
        end_col: 3,
        mode: reovim_client_subsys_module::SelectionMode::Char,
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
    let overlays = surface.overlays.borrow();
    // Line 0: cols 2..5 (start_col to end-of-line "hello".len()=5) => 3 overlays
    // Line 1: cols 0..4 (0 to end_col+1=4) => 4 overlays
    assert_eq!(overlays.len(), 3 + 4, "2-line Char selection overlay count");
}

// =============================================================================
// Coverage gap: multi-line Char selection (3+ lines, middle-line branch)
// =============================================================================

#[test]
fn render_selection_char_three_lines() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["aaaa".to_string(), "bbbb".to_string(), "cccc".to_string()];
    let local_sel = SelectionInfo {
        start_line: 0,
        start_col: 1,
        end_line: 2,
        end_col: 2,
        mode: reovim_client_subsys_module::SelectionMode::Char,
        color: reovim_arch::Color::Green,
    };
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        local_selection: Some(local_sel),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_selections(&mut surface, &ctx, 0, 24, &modules);
    let overlays = surface.overlays.borrow();
    // Line 0: cols 1..4 (start_col=1 to visual_line_len=4) => 3 overlays
    // Line 1: cols 0..4 (0 to visual_line_len=4, middle line) => 4 overlays
    // Line 2: cols 0..3 (0 to end_col+1=3) => 3 overlays
    assert_eq!(overlays.len(), 3 + 4 + 3, "3-line Char selection overlay count");
}

// =============================================================================
// Coverage gap: Block mode selection
// =============================================================================

#[test]
fn render_selection_block_mode() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["aaaa".to_string(), "bbbb".to_string(), "cccc".to_string()];
    let local_sel = SelectionInfo {
        start_line: 0,
        start_col: 1,
        end_line: 2,
        end_col: 2,
        mode: reovim_client_subsys_module::SelectionMode::Block,
        color: reovim_arch::Color::Red,
    };
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        local_selection: Some(local_sel),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_selections(&mut surface, &ctx, 0, 24, &modules);
    let overlays = surface.overlays.borrow();
    // Block mode: each line gets cols 1..3 (start_col=1 to end_col+1=3) => 2 per line, 3 lines => 6
    assert_eq!(overlays.len(), 6, "Block selection overlay count");
}

// =============================================================================
// Coverage gap: render_gutter_annotations — col_width == 0 continue
// =============================================================================

#[test]
fn render_gutter_annotations_col_width_zero() {
    struct ZeroWidthAnnotation;
    impl ClientModule for ZeroWidthAnnotation {
        fn id(&self) -> &'static str {
            "zero-ann"
        }
        fn name(&self) -> &'static str {
            "Zero"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_annotations(&self) -> bool {
            true
        }
        fn annotation_column_width(
            &self,
            _ctx: &reovim_client_subsys_module::AnnotationContext,
            _caps: &dyn reovim_client_subsys_module::PlatformCapabilities,
        ) -> reovim_client_subsys_module::ColumnWidth {
            reovim_client_subsys_module::ColumnWidth::Fixed(0)
        }
        fn annotate(
            &self,
            _line: usize,
            _ctx: &reovim_client_subsys_module::AnnotationContext,
        ) -> Option<reovim_client_subsys_module::GutterCell> {
            Some(reovim_client_subsys_module::GutterCell {
                text: "X".to_string(),
                style: Style::default(),
            })
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string()];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        gutter_width: 4,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(ZeroWidthAnnotation)];
    render_gutter_annotations(&mut surface, 0, 0, 4, 0, &modules, &ctx);
    // col_width == 0 means the annotate() call is skipped; no writes
    assert!(
        surface.writes.borrow().is_empty(),
        "zero-width annotation should produce no output"
    );
}

// =============================================================================
// Coverage gap: render_gutter_annotations — annotate returns None
// =============================================================================

#[test]
fn render_gutter_annotations_annotate_returns_none() {
    struct NoneAnnotation;
    impl ClientModule for NoneAnnotation {
        fn id(&self) -> &'static str {
            "none-ann"
        }
        fn name(&self) -> &'static str {
            "None"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_annotations(&self) -> bool {
            true
        }
        fn annotation_column_width(
            &self,
            _ctx: &reovim_client_subsys_module::AnnotationContext,
            _caps: &dyn reovim_client_subsys_module::PlatformCapabilities,
        ) -> reovim_client_subsys_module::ColumnWidth {
            reovim_client_subsys_module::ColumnWidth::Fixed(4)
        }
        fn annotate(
            &self,
            _line: usize,
            _ctx: &reovim_client_subsys_module::AnnotationContext,
        ) -> Option<reovim_client_subsys_module::GutterCell> {
            None
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string()];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        gutter_width: 4,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(NoneAnnotation)];
    render_gutter_annotations(&mut surface, 0, 0, 4, 0, &modules, &ctx);
    // annotate returns None; no writes but col_x advances
    assert!(surface.writes.borrow().is_empty(), "None annotate should produce no output");
}

// =============================================================================
// Coverage gap: render_gutter_annotations — ColumnWidth::Dynamic arm
// =============================================================================

#[test]
fn render_gutter_annotations_dynamic_width() {
    struct DynamicAnnotation;
    impl ClientModule for DynamicAnnotation {
        fn id(&self) -> &'static str {
            "dyn-ann"
        }
        fn name(&self) -> &'static str {
            "Dyn"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_annotations(&self) -> bool {
            true
        }
        fn annotation_column_width(
            &self,
            _ctx: &reovim_client_subsys_module::AnnotationContext,
            _caps: &dyn reovim_client_subsys_module::PlatformCapabilities,
        ) -> reovim_client_subsys_module::ColumnWidth {
            reovim_client_subsys_module::ColumnWidth::Dynamic(3)
        }
        fn annotate(
            &self,
            line: usize,
            _ctx: &reovim_client_subsys_module::AnnotationContext,
        ) -> Option<reovim_client_subsys_module::GutterCell> {
            Some(reovim_client_subsys_module::GutterCell {
                text: (line + 1).to_string(),
                style: Style::default(),
            })
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string()];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        gutter_width: 3,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(DynamicAnnotation)];
    render_gutter_annotations(&mut surface, 0, 0, 3, 0, &modules, &ctx);
    let writes = surface.writes.borrow();
    assert!(!writes.is_empty(), "Dynamic annotation should produce output");
    let text: String = writes.iter().map(|(_, _, t, _)| t.as_str()).collect();
    assert!(text.contains('1'), "should contain line number 1: {text}");
}

// =============================================================================
// Coverage gap: compute_cursor_visual_col with active conceals
// =============================================================================

#[test]
fn compute_cursor_visual_col_with_conceals() {
    struct ConcealModule;
    impl ClientModule for ConcealModule {
        fn id(&self) -> &'static str {
            "conceal"
        }
        fn name(&self) -> &'static str {
            "Conceal"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        fn classify_token(&self, category: &str) -> Option<RenderBehavior> {
            if category == "conceal.url" {
                Some(RenderBehavior::Conceal {
                    replacement: Cow::Borrowed("L"),
                })
            } else {
                None
            }
        }
    }

    // Text: "abhttp://ex.comcd" (17 chars)
    // Conceal covers [2, 15) -> replaced with "L"
    // Display: a(0) b(1) L(2) c(3) d(4)
    let lines = vec!["abhttp://ex.comcd".to_string()];
    let tokens = vec![SyntaxToken {
        line: 0,
        start_col: 2,
        end_col: 15,
        category: "conceal.url".to_owned(),
    }];
    let token_provider = MockTokenProvider::with_tokens(tokens);
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(ConcealModule)];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        ..empty_ctx()
    };
    // Cursor at source col 16 ("d") should map to display col 4
    let cursor = CursorInfo {
        line: 0,
        column: 16,
    };
    let result = compute_cursor_visual_col(&ctx, &cursor, &token_provider, &MockTheme, &modules);
    // After concealing cols 2..15 -> "L": a(0) b(1) L(2) c(3) d(4)
    // Source col 16 = "d" = display col 4
    assert_eq!(result, 4, "cursor should be at display col 4 after conceal");
}

// =============================================================================
// Coverage gap: compute_cursor_visual_col with Hide behavior
// =============================================================================

#[test]
fn compute_cursor_visual_col_with_hide() {
    struct HideModule;
    impl ClientModule for HideModule {
        fn id(&self) -> &'static str {
            "hide"
        }
        fn name(&self) -> &'static str {
            "Hide"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
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

    let lines = vec!["hello world".to_string()];
    let tokens = vec![SyntaxToken {
        line: 0,
        start_col: 5,
        end_col: 11,
        category: "hidden".to_owned(),
    }];
    let token_provider = MockTokenProvider::with_tokens(tokens);
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(HideModule)];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        ..empty_ctx()
    };
    // " world" (cols 5..11) is hidden, so original col 5 should still be 5
    // but col 11 would map differently... actually cursor at col 3 is before
    // the hidden range so should be unchanged
    let cursor = CursorInfo { line: 0, column: 3 };
    let result = compute_cursor_visual_col(&ctx, &cursor, &token_provider, &MockTheme, &modules);
    assert_eq!(result, 3, "cursor before hidden range should be unchanged");
}

// =============================================================================
// Coverage gap: compute_cursor_visual_col with FullWidthLine behavior
// =============================================================================

#[test]
fn compute_cursor_visual_col_with_full_width_line() {
    struct FwlModule;
    impl ClientModule for FwlModule {
        fn id(&self) -> &'static str {
            "fwl"
        }
        fn name(&self) -> &'static str {
            "FWL"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
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

    let lines = vec!["---".to_string()];
    let tokens = vec![SyntaxToken {
        line: 0,
        start_col: 0,
        end_col: 3,
        category: "hr".to_owned(),
    }];
    let token_provider = MockTokenProvider::with_tokens(tokens);
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(FwlModule)];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        ..empty_ctx()
    };
    let cursor = CursorInfo { line: 0, column: 0 };
    let result = compute_cursor_visual_col(&ctx, &cursor, &token_provider, &MockTheme, &modules);
    // With FullWidthLine conceal the display column may shift
    assert!(result < 600, "cursor visual col should be reasonable");
}

// =============================================================================
// Coverage gap: render_remote_cursor_labels — visible label
// =============================================================================

#[test]
fn render_remote_cursor_labels_visible() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string()];
    let remote = reovim_client_subsys_module::RemoteClientInfo {
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
    render_remote_cursor_labels(&mut surface, &ctx, 0, 24);
    let writes = surface.writes.borrow();
    assert!(!writes.is_empty(), "should render a cursor label");
    let text: String = writes.iter().map(|(_, _, t, _)| t.as_str()).collect();
    assert!(text.contains("Alice"), "label should contain display name: {text}");
    assert!(text.contains("[N]"), "label should contain mode abbreviation: {text}");
}

// =============================================================================
// Coverage gap: render_remote_cursor_labels — label overflow (too wide)
// =============================================================================

#[test]
fn render_remote_cursor_labels_overflow_skip() {
    // Use a very narrow surface so that label_x + label_width > width
    let mut surface = RecordingSurface::new(10, 24);
    let lines = vec!["hello".to_string()];
    let remote = reovim_client_subsys_module::RemoteClientInfo {
        client_id: 1,
        display_name: "VeryLongName".to_string(),
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
    render_remote_cursor_labels(&mut surface, &ctx, 0, 24);
    // Label should be skipped because it overflows the surface width
    assert!(surface.writes.borrow().is_empty(), "label should be skipped when it overflows");
}

// =============================================================================
// Coverage gap: render_remote_cursor_labels — cursor before scroll
// =============================================================================

#[test]
fn render_remote_cursor_labels_before_scroll() {
    let mut surface = RecordingSurface::new(80, 24);
    let remote = reovim_client_subsys_module::RemoteClientInfo {
        client_id: 1,
        display_name: "Alice".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        mode: "normal".to_string(),
        cursor_color: reovim_arch::Color::Blue,
        selection: None,
    };
    let remotes = vec![remote];
    let ctx = ViewportContext {
        scroll_top: 5,
        remote_clients: &remotes,
        ..empty_ctx()
    };
    render_remote_cursor_labels(&mut surface, &ctx, 0, 24);
    assert!(
        surface.writes.borrow().is_empty(),
        "label should be skipped when cursor is before scroll"
    );
}

// =============================================================================
// Coverage gap: render_remote_cursor_labels — cursor past content_height
// =============================================================================

#[test]
fn render_remote_cursor_labels_past_content_height() {
    let mut surface = RecordingSurface::new(80, 24);
    let remote = reovim_client_subsys_module::RemoteClientInfo {
        client_id: 1,
        display_name: "Bob".to_string(),
        cursor_line: 30,
        cursor_col: 0,
        mode: "normal".to_string(),
        cursor_color: reovim_arch::Color::Blue,
        selection: None,
    };
    let remotes = vec![remote];
    let ctx = ViewportContext {
        remote_clients: &remotes,
        ..empty_ctx()
    };
    render_remote_cursor_labels(&mut surface, &ctx, 0, 5);
    assert!(
        surface.writes.borrow().is_empty(),
        "label should be skipped when cursor is past content_height"
    );
}

// =============================================================================
// Coverage gap: render_buffer_content — break after virtual-lines-before fill viewport
// =============================================================================

#[test]
fn render_buffer_content_vl_before_fills_viewport() {
    // Viewport height=2, one line with 2 virtual-lines-before fills the viewport
    // and triggers the `screen_row >= content_height` break
    let mut surface = RecordingSurface::new(80, 2);
    let lines = vec!["hello".to_string(), "world".to_string()];
    let vlines = vec![
        VirtualLine {
            buffer_line: 0,
            position: VirtualLinePosition::Before,
            content: "vl1".to_string(),
            style: Style::default(),
        },
        VirtualLine {
            buffer_line: 0,
            position: VirtualLinePosition::Before,
            content: "vl2".to_string(),
            style: Style::default(),
        },
    ];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        virtual_lines: &vlines,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let token_provider = MockTokenProvider::empty();
    let viewport = Rect::new(0, 0, 80, 2);

    render_buffer_content(&mut surface, viewport, &ctx, &modules, &token_provider, &MockTheme);

    // Only the 2 virtual lines should appear; "hello" should not because viewport is full
    assert_eq!(surface.text_at(0), "vl1");
    assert_eq!(surface.text_at(1), "vl2");
}

// =============================================================================
// Coverage gap: render_line_content — inline_decorations from modules
// =============================================================================

#[test]
fn render_line_content_with_inline_decorations() {
    struct InlineDecModule {
        decorations: Vec<reovim_client_subsys_module::InlineDecoration>,
    }
    impl ClientModule for InlineDecModule {
        fn id(&self) -> &'static str {
            "inline-dec"
        }
        fn name(&self) -> &'static str {
            "InlineDec"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        fn inline_decorations(
            &self,
            _line: usize,
        ) -> &[reovim_client_subsys_module::InlineDecoration] {
            &self.decorations
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let token_provider = MockTokenProvider::empty();
    let module = InlineDecModule {
        decorations: vec![reovim_client_subsys_module::InlineDecoration {
            col_start: 0,
            col_end: 3,
            style: Style::new().fg(reovim_arch::Color::Red),
        }],
    };
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(module)];
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
    // apply_style is a no-op on RecordingSurface, but the code path is exercised
}

// =============================================================================
// Coverage gap: DefaultViewportRenderer::gutter_width — ColumnWidth::Dynamic
// =============================================================================

#[test]
fn default_viewport_renderer_gutter_width_dynamic() {
    struct DynAnnotation;
    impl ClientModule for DynAnnotation {
        fn id(&self) -> &'static str {
            "dyn-gutter"
        }
        fn name(&self) -> &'static str {
            "DynGutter"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_annotations(&self) -> bool {
            true
        }
        fn annotation_column_width(
            &self,
            _ctx: &reovim_client_subsys_module::AnnotationContext,
            _caps: &dyn reovim_client_subsys_module::PlatformCapabilities,
        ) -> reovim_client_subsys_module::ColumnWidth {
            reovim_client_subsys_module::ColumnWidth::Dynamic(5)
        }
    }

    let renderer = DefaultViewportRenderer;
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(DynAnnotation)];
    let caps = TestCaps;
    assert_eq!(renderer.gutter_width(&modules, &caps), 5, "Dynamic(5) should use min=5");
}

// =============================================================================
// Coverage gap: dimmed_client_color
// =============================================================================

#[test]
fn dimmed_client_color_palette() {
    let c0 = dimmed_client_color(0);
    let c1 = dimmed_client_color(1);
    assert_ne!(format!("{c0:?}"), format!("{c1:?}"));
    // Wraps around
    let c8 = dimmed_client_color(8);
    assert_eq!(format!("{c0:?}"), format!("{c8:?}"));
}

// =============================================================================
// Coverage gap: selection before scroll_top (skip path)
// =============================================================================

#[test]
fn render_selection_before_scroll() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec![
        "line0".to_string(),
        "line1".to_string(),
        "line2".to_string(),
    ];
    let local_sel = SelectionInfo {
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 3,
        mode: reovim_client_subsys_module::SelectionMode::Char,
        color: reovim_arch::Color::Blue,
    };
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        local_selection: Some(local_sel),
        scroll_top: 2,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_selections(&mut surface, &ctx, 0, 24, &modules);
    // Selection is on line 0, scroll_top=2 — selection is before viewport
    assert!(
        surface.overlays.borrow().is_empty(),
        "selection before scroll should produce no overlays"
    );
}

// =============================================================================
// Coverage gap: selection past content_height (break path)
// =============================================================================

#[test]
fn render_selection_past_content_height() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["line0".to_string()];
    let local_sel = SelectionInfo {
        start_line: 10,
        start_col: 0,
        end_line: 10,
        end_col: 3,
        mode: reovim_client_subsys_module::SelectionMode::Char,
        color: reovim_arch::Color::Blue,
    };
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        local_selection: Some(local_sel),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_selections(&mut surface, &ctx, 0, 3, &modules);
    assert!(
        surface.overlays.borrow().is_empty(),
        "selection past content_height should produce no overlays"
    );
}

// =============================================================================
// Coverage gap: render_self_cursor with extension column mapping
// =============================================================================

#[test]
fn render_self_cursor_with_column_mapping() {
    struct ColMapModule;
    impl ClientModule for ColMapModule {
        fn id(&self) -> &'static str {
            "colmap"
        }
        fn name(&self) -> &'static str {
            "ColMap"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        #[allow(clippy::cast_possible_truncation)]
        fn map_cursor_column(&self, _buf: BufferId, _line: usize, col: usize) -> Option<u16> {
            Some((col + 5) as u16)
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string()];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        cursor: Some(CursorInfo { line: 0, column: 2 }),
        render_self_cursor: true,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(ColMapModule)];
    let tokens = MockTokenProvider::empty();
    render_self_cursor(&mut surface, &ctx, 0, 24, &tokens, &MockTheme, &modules);
    // No panic; apply_style is a no-op but the extension path is exercised
}

// =============================================================================
// Coverage gap: render_remote_cursor_labels with no buffer_lines
// =============================================================================

#[test]
fn render_remote_cursor_labels_no_buffer_lines() {
    let mut surface = RecordingSurface::new(80, 24);
    let remote = reovim_client_subsys_module::RemoteClientInfo {
        client_id: 1,
        display_name: "Alice".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        mode: "normal".to_string(),
        cursor_color: reovim_arch::Color::Blue,
        selection: None,
    };
    let remotes = vec![remote];
    let ctx = ViewportContext {
        buffer_lines: None,
        remote_clients: &remotes,
        ..empty_ctx()
    };
    render_remote_cursor_labels(&mut surface, &ctx, 0, 24);
    // eol_col defaults to 0 when buffer_lines is None
    let writes = surface.writes.borrow();
    assert!(!writes.is_empty(), "should still render label at col 1");
}

// =============================================================================
// Coverage gap: render_selection_range with module column mapping
// =============================================================================

#[test]
fn render_selection_char_with_column_mapping_module() {
    struct ColMapModule;
    impl ClientModule for ColMapModule {
        fn id(&self) -> &'static str {
            "colmap"
        }
        fn name(&self) -> &'static str {
            "ColMap"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        #[allow(clippy::cast_possible_truncation)]
        fn map_cursor_column(&self, _buf: BufferId, _line: usize, col: usize) -> Option<u16> {
            // Shift columns right by 2 (simulates a table extension)
            Some((col + 2) as u16)
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello world".to_string()];
    let local_sel = SelectionInfo {
        start_line: 0,
        start_col: 1,
        end_line: 0,
        end_col: 3,
        mode: reovim_client_subsys_module::SelectionMode::Char,
        color: reovim_arch::Color::Blue,
    };
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        local_selection: Some(local_sel),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(ColMapModule)];
    render_selections(&mut surface, &ctx, 0, 24, &modules);
    // map_col(1) = 3, map_col(3) = 5 => cols 3..6 (3 overlays)
    let overlays = surface.overlays.borrow();
    assert_eq!(overlays.len(), 3, "column-mapped Char selection overlay count");
}

#[test]
fn render_selection_block_with_column_mapping_module() {
    struct ColMapModule;
    impl ClientModule for ColMapModule {
        fn id(&self) -> &'static str {
            "colmap"
        }
        fn name(&self) -> &'static str {
            "ColMap"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        #[allow(clippy::cast_possible_truncation)]
        fn map_cursor_column(&self, _buf: BufferId, _line: usize, col: usize) -> Option<u16> {
            Some((col + 2) as u16)
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string(), "world".to_string()];
    let local_sel = SelectionInfo {
        start_line: 0,
        start_col: 1,
        end_line: 1,
        end_col: 2,
        mode: reovim_client_subsys_module::SelectionMode::Block,
        color: reovim_arch::Color::Red,
    };
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        local_selection: Some(local_sel),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(ColMapModule)];
    render_selections(&mut surface, &ctx, 0, 24, &modules);
    // Block: map_col(1)=3, map_col(2)+1=5, visual_line_len=5 => cols 3..5 = 2 per line, 2 lines => 4
    let overlays = surface.overlays.borrow();
    assert_eq!(overlays.len(), 4, "block selection with column mapping overlay count");
}

// =============================================================================
// Coverage gap: render_selection_range with transform_line providing visual width
// =============================================================================

#[test]
fn render_selection_with_transform_line_visual_width() {
    struct TransformModule;
    impl ClientModule for TransformModule {
        fn id(&self) -> &'static str {
            "transform"
        }
        fn name(&self) -> &'static str {
            "Transform"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        fn transform_line(
            &self,
            _buf: BufferId,
            _line: usize,
            _text: &str,
        ) -> Option<reovim_client_subsys_module::TransformedLine> {
            // Transform yields a shorter visual representation
            Some(reovim_client_subsys_module::TransformedLine {
                segments: vec![("ab".to_string(), None), ("cd".to_string(), None)],
            })
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello world".to_string()];
    let local_sel = SelectionInfo {
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 2,
        mode: reovim_client_subsys_module::SelectionMode::Char,
        color: reovim_arch::Color::Blue,
    };
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        local_selection: Some(local_sel),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(TransformModule)];
    render_selections(&mut surface, &ctx, 0, 24, &modules);
    // transform_line returns "abcd" (4 chars)
    // Char single-line: cols 0..min(3, 4)=3 => 3 overlays
    let overlays = surface.overlays.borrow();
    assert_eq!(overlays.len(), 3, "selection with transform_line visual width");
}

// =============================================================================
// Coverage gap: render_selection_range with no buffer_lines
// =============================================================================

#[test]
fn render_selection_no_buffer_lines() {
    let mut surface = RecordingSurface::new(80, 24);
    let local_sel = SelectionInfo {
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 3,
        mode: reovim_client_subsys_module::SelectionMode::Char,
        color: reovim_arch::Color::Blue,
    };
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: None,
        local_selection: Some(local_sel),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_selections(&mut surface, &ctx, 0, 24, &modules);
    // visual_line_len defaults to content_width when lines is None
    let overlays = surface.overlays.borrow();
    assert_eq!(overlays.len(), 4, "selection with no buffer_lines defaults to content_width");
}

// =============================================================================
// Coverage gap: render_buffer_content with transformed line from module
// =============================================================================

#[test]
fn render_buffer_content_with_transformed_line() {
    struct TransformModule;
    impl ClientModule for TransformModule {
        fn id(&self) -> &'static str {
            "transform"
        }
        fn name(&self) -> &'static str {
            "Transform"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        fn transform_line(
            &self,
            _buf: BufferId,
            _line: usize,
            _text: &str,
        ) -> Option<reovim_client_subsys_module::TransformedLine> {
            Some(reovim_client_subsys_module::TransformedLine {
                segments: vec![
                    (">>".to_string(), Some(Style::new().fg(reovim_arch::Color::Red))),
                    ("replaced".to_string(), None),
                ],
            })
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["original text".to_string()];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(TransformModule)];
    let token_provider = MockTokenProvider::empty();
    let viewport = Rect::new(0, 0, 80, 24);

    render_buffer_content(&mut surface, viewport, &ctx, &modules, &token_provider, &MockTheme);

    let text = surface.text_at(0);
    assert_eq!(text, ">>replaced", "buffer content should use transformed line");
}

// =============================================================================
// Coverage gap: render_line_content — background token with opacity that
// removes bg (bg.bg becomes None after dim_style)
// =============================================================================

#[test]
fn render_line_content_bg_token_zero_opacity() {
    struct BgModule;
    impl ClientModule for BgModule {
        fn id(&self) -> &'static str {
            "bg"
        }
        fn name(&self) -> &'static str {
            "Bg"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
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
    // opacity=0.0 should make the bg color blend to black (DEFAULT_BG),
    // but the style may still have bg=Some(Black) or bg=None depending on dim_style
    render_line_content(
        &mut surface,
        0,
        0,
        80,
        "hello",
        0.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );
    // Exercise the path; text should still render
    let text = surface.text_at(0);
    assert_eq!(text, "hello");
}

// =============================================================================
// Coverage gap: render_line_content — Conceal/Hide/FWL tokens skipped when
// skip_conceals=true, falling through to `_ => {}` arm
// =============================================================================

#[test]
fn render_line_content_conceal_tokens_skip_all_types() {
    struct AllBehaviorModule;
    impl ClientModule for AllBehaviorModule {
        fn id(&self) -> &'static str {
            "all-behavior"
        }
        fn name(&self) -> &'static str {
            "AllBehavior"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        fn classify_token(&self, category: &str) -> Option<RenderBehavior> {
            match category {
                "conceal" => Some(RenderBehavior::Conceal {
                    replacement: Cow::Borrowed("X"),
                }),
                "hidden" => Some(RenderBehavior::Hide),
                "hr" => Some(RenderBehavior::FullWidthLine {
                    ch: '-',
                    style: Style::default(),
                }),
                _ => None,
            }
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let tokens = vec![
        SyntaxToken {
            line: 0,
            start_col: 0,
            end_col: 2,
            category: "conceal".to_owned(),
        },
        SyntaxToken {
            line: 0,
            start_col: 3,
            end_col: 5,
            category: "hidden".to_owned(),
        },
        SyntaxToken {
            line: 0,
            start_col: 6,
            end_col: 8,
            category: "hr".to_owned(),
        },
    ];
    let token_provider = MockTokenProvider::with_tokens(tokens);
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(AllBehaviorModule)];
    render_line_content(
        &mut surface,
        0,
        0,
        80,
        "ab cd efgh",
        1.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        true, // skip conceals — all Conceal/Hide/FWL fall through to `_ => {}`
        &modules,
    );
    // All text should be visible (conceals skipped)
    let text = surface.text_at(0);
    assert_eq!(text, "ab cd efgh", "all conceals should be skipped in insert mode");
}

// =============================================================================
// Coverage gap: render_remote_cursors — cursor col beyond surface width
// =============================================================================

#[test]
fn render_remote_cursors_col_beyond_width() {
    let mut surface = RecordingSurface::new(20, 24);
    let lines = vec!["hello".to_string()];
    let remote = reovim_client_subsys_module::RemoteClientInfo {
        client_id: 1,
        display_name: "Alice".to_string(),
        cursor_line: 0,
        cursor_col: 100, // far beyond width=20
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
    // screen_col = 100 + 0 = 100, which is >= width=20
    // The if condition fails, so apply_style is not called
    render_remote_cursors(&mut surface, &ctx, 0, 24);
    // No panic; the cursor is simply not rendered
}

// =============================================================================
// Coverage gap: render_remote_cursors — cursor past content_height
// =============================================================================

#[test]
fn render_remote_cursors_past_content_height() {
    let mut surface = RecordingSurface::new(80, 24);
    let remote = reovim_client_subsys_module::RemoteClientInfo {
        client_id: 1,
        display_name: "Alice".to_string(),
        cursor_line: 30,
        cursor_col: 0,
        mode: "normal".to_string(),
        cursor_color: reovim_arch::Color::Blue,
        selection: None,
    };
    let remotes = vec![remote];
    let ctx = ViewportContext {
        remote_clients: &remotes,
        ..empty_ctx()
    };
    // cursor_line=30, content_height=5 => screen_line=30 >= 5, if condition fails
    render_remote_cursors(&mut surface, &ctx, 0, 5);
}

// =============================================================================
// Coverage gap: render_self_cursor — cursor past content_height
// =============================================================================

#[test]
fn render_self_cursor_past_content_height() {
    let mut surface = RecordingSurface::new(80, 24);
    let ctx = ViewportContext {
        cursor: Some(CursorInfo {
            line: 50,
            column: 0,
        }),
        render_self_cursor: true,
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let tokens = MockTokenProvider::empty();
    // content_height=5, cursor at line 50 => screen_line=50 >= 5
    render_self_cursor(&mut surface, &ctx, 0, 5, &tokens, &MockTheme, &modules);
}

// =============================================================================
// Coverage gap: render_self_cursor — screen_col beyond width
// =============================================================================

#[test]
fn render_self_cursor_col_beyond_width() {
    let mut surface = RecordingSurface::new(10, 24);
    let lines = vec!["hello".to_string()];
    let ctx = ViewportContext {
        buffer_id: Some(BufferId(0)),
        buffer_lines: Some(&lines),
        cursor: Some(CursorInfo {
            line: 0,
            column: 100,
        }),
        render_self_cursor: true,
        is_insert_mode: true, // use insert mode path for visual_col = column
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    let tokens = MockTokenProvider::empty();
    // visual_col=100, content_x=0, screen_col=100 >= width=10
    render_self_cursor(&mut surface, &ctx, 0, 24, &tokens, &MockTheme, &modules);
}

// =============================================================================
// Coverage gap: render_selection_range — multi-line char with no buffer_id
// (map_col closure returns buf_col as u16)
// =============================================================================

#[test]
fn render_selection_char_multiline_no_buffer_id() {
    let mut surface = RecordingSurface::new(80, 24);
    let lines = vec!["hello".to_string(), "world".to_string()];
    let local_sel = SelectionInfo {
        start_line: 0,
        start_col: 2,
        end_line: 1,
        end_col: 3,
        mode: reovim_client_subsys_module::SelectionMode::Char,
        color: reovim_arch::Color::Blue,
    };
    let ctx = ViewportContext {
        buffer_id: None, // No buffer ID — map_col just returns buf_col
        buffer_lines: Some(&lines),
        local_selection: Some(local_sel),
        ..empty_ctx()
    };
    let modules: Vec<Box<dyn ClientModule>> = Vec::new();
    render_selections(&mut surface, &ctx, 0, 24, &modules);
    let overlays = surface.overlays.borrow();
    // Line 0: cols 2..5 = 3 overlays
    // Line 1: cols 0..4 = 4 overlays
    assert_eq!(overlays.len(), 7, "multi-line char selection without buffer_id");
}

// =============================================================================
// Coverage gap: render_line_content — conceal style lookup in render loop
// (conceal_style is Some vs None, and source_col mapping with highlight)
// =============================================================================

#[test]
fn render_line_content_conceal_with_style_and_unmapped_cols() {
    struct ConcealModule;
    impl ClientModule for ConcealModule {
        fn id(&self) -> &'static str {
            "conceal"
        }
        fn name(&self) -> &'static str {
            "Conceal"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        fn classify_token(&self, category: &str) -> Option<RenderBehavior> {
            if category == "conceal.url" {
                Some(RenderBehavior::Conceal {
                    replacement: Cow::Borrowed("LINK"),
                })
            } else {
                None
            }
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    // "ab" (0..2), then conceal covers 2..7 ("cdefg"), then "hi" (7..9)
    let tokens = vec![
        SyntaxToken {
            line: 0,
            start_col: 0,
            end_col: 2,
            category: "keyword".to_owned(),
        },
        SyntaxToken {
            line: 0,
            start_col: 2,
            end_col: 7,
            category: "conceal.url".to_owned(),
        },
    ];
    let token_provider = MockTokenProvider::with_tokens(tokens);
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(ConcealModule)];
    render_line_content(
        &mut surface,
        0,
        0,
        80,
        "abcdefghi",
        1.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );
    let text = surface.text_at(0);
    // "ab" + "LINK" + "hi" = "abLINKhi"
    assert!(text.contains("LINK"), "conceal replacement should appear: {text}");
    assert!(text.contains("ab"), "prefix should remain: {text}");
    assert!(text.contains("hi"), "suffix should remain: {text}");
}

// =============================================================================
// Coverage gap: render_line_content — background token with bg beyond line length
// =============================================================================

#[test]
fn render_line_content_bg_token_end_beyond_line() {
    struct BgModule;
    impl ClientModule for BgModule {
        fn id(&self) -> &'static str {
            "bg"
        }
        fn name(&self) -> &'static str {
            "Bg"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
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
        end_col: 100, // beyond "hi" length
        category: "bg.token".to_owned(),
    }];
    let token_provider = MockTokenProvider::with_tokens(tokens);
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(BgModule)];
    render_line_content(
        &mut surface,
        0,
        0,
        80,
        "hi",
        1.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );
    // min(end_col=100, line_char_count=2) => only 2 bg overlays
    let overlays = surface.overlays.borrow();
    assert_eq!(overlays.len(), 2, "bg overlay clamped to line length");
}

// =============================================================================
// Coverage gap: render_line_content — background token col_u16 >= width
// =============================================================================

#[test]
fn render_line_content_bg_token_col_beyond_width() {
    struct BgModule;
    impl ClientModule for BgModule {
        fn id(&self) -> &'static str {
            "bg"
        }
        fn name(&self) -> &'static str {
            "Bg"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
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
    // width=3 means display cols >= 3 should be skipped by the `col_u16 < width` check
    render_line_content(
        &mut surface,
        0,
        0,
        3, // narrow width
        "hello",
        1.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );
    // Only cols 0..3 should get overlays (clamped by width check)
    let overlays = surface.overlays.borrow();
    assert_eq!(overlays.len(), 3, "bg overlay clamped to content width");
}

// =============================================================================
// Coverage gap: render_line_content — inline decoration col_end clamped by width
// =============================================================================

#[test]
fn render_line_content_inline_dec_clamped_by_width() {
    struct InlineDecModule {
        decorations: Vec<reovim_client_subsys_module::InlineDecoration>,
    }
    impl ClientModule for InlineDecModule {
        fn id(&self) -> &'static str {
            "inline-dec"
        }
        fn name(&self) -> &'static str {
            "InlineDec"
        }
        fn version(&self) -> reovim_client_subsys_module::Version {
            reovim_client_subsys_module::Version::new(1, 0, 0)
        }
        fn init(
            &mut self,
            _ctx: &reovim_client_subsys_module::ModuleContext,
        ) -> reovim_client_subsys_module::ProbeResult {
            reovim_client_subsys_module::ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), reovim_client_subsys_module::ClientModuleError> {
            Ok(())
        }
        fn has_buffer_contrib(&self) -> bool {
            true
        }
        fn inline_decorations(
            &self,
            _line: usize,
        ) -> &[reovim_client_subsys_module::InlineDecoration] {
            &self.decorations
        }
    }

    let mut surface = RecordingSurface::new(80, 24);
    let module = InlineDecModule {
        decorations: vec![reovim_client_subsys_module::InlineDecoration {
            col_start: 0,
            col_end: 100, // far beyond width
            style: Style::new().fg(reovim_arch::Color::Red),
        }],
    };
    let modules: Vec<Box<dyn ClientModule>> = vec![Box::new(module)];
    let token_provider = MockTokenProvider::empty();
    render_line_content(
        &mut surface,
        0,
        0,
        5, // narrow width
        "hello world",
        1.0,
        Some(BufferId(0)),
        0,
        &token_provider,
        &MockTheme,
        false,
        &modules,
    );
    // col_end.min(width) = 5, so only cols 0..5 get decorations
    let text = surface.text_at(0);
    assert_eq!(text, "hello", "content should be truncated to width");
}

// =============================================================================
// Coverage gap: render_positioned_virtual_lines — VL past content_height
// =============================================================================

#[test]
fn render_positioned_vl_past_content_height() {
    let mut surface = RecordingSurface::new(80, 2);
    let vlines = vec![
        VirtualLine {
            buffer_line: 0,
            position: VirtualLinePosition::After,
            content: "after1".to_string(),
            style: Style::default(),
        },
        VirtualLine {
            buffer_line: 0,
            position: VirtualLinePosition::After,
            content: "after2".to_string(),
            style: Style::default(),
        },
        VirtualLine {
            buffer_line: 0,
            position: VirtualLinePosition::After,
            content: "after3".to_string(),
            style: Style::default(),
        },
    ];
    let ctx = ViewportContext {
        virtual_lines: &vlines,
        ..empty_ctx()
    };
    // content_height=2, screen_row=1, so only 1 VL fits before reaching content_height
    let rows = render_positioned_virtual_lines(
        &mut surface,
        &ctx,
        0,
        0,
        1,
        80,
        2,
        0,
        VirtualLinePosition::After,
    );
    assert_eq!(rows, 1, "only 1 VL should fit within remaining viewport height");
}

// =============================================================================
// Coverage gap: DummyCaps coverage (render_gutter_annotations uses it)
// =============================================================================

#[test]
fn dummy_caps_coverage() {
    let caps = DummyCaps;
    assert_eq!(caps.rendering_model(), reovim_client_subsys_module::RenderingModel::CellGrid);
    assert_eq!(caps.grid_size(), None);
    assert_eq!(caps.color_depth(), reovim_client_subsys_module::ColorDepth::TrueColor);
    assert_eq!(caps.pixel_size(), None);
    assert!(caps.reliable_unicode_width());
    assert!(caps.dark_mode());
    assert!(!caps.smooth_scroll());
    assert!(!caps.pointer_events());
    assert!(!caps.touch_input());
    assert!(!caps.haptic());
    assert_eq!(caps.safe_area(), reovim_client_subsys_module::Insets::ZERO);
    assert!(caps.has_focus());
    assert!(caps.clipboard_available());
    assert!(!caps.screen_reader_active());
}

// =============================================================================
// Coverage gap: render_selection_range — Line mode multi-line
// =============================================================================

#[test]
fn render_selection_line_mode_multi_line() {
    let mut surface = RecordingSurface::new(20, 24);
    let lines = vec!["hello".to_string(), "world".to_string()];
    let local_sel = SelectionInfo {
        start_line: 0,
        start_col: 0,
        end_line: 1,
        end_col: 0,
        mode: reovim_client_subsys_module::SelectionMode::Line,
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
    // Line mode: each line gets full content_width=20 overlay, 2 lines => 40
    let overlays = surface.overlays.borrow();
    assert_eq!(overlays.len(), 40, "Line mode multi-line selection");
}
