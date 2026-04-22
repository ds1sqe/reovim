use {
    super::*,
    reovim_client_driver::testing::MockPlatformCapabilities,
    reovim_ext_client_tui_cap_cell::{CellCapability, CellStyle},
};

// Plan 23 / 17-β.2b-impl-c bulk migration helpers.

fn has_content(g: &CellCapability) -> bool {
    g.iter().any(|(_, c)| c.ch != ' ')
}

fn char_at(g: &CellCapability, x: u16, y: u16) -> char {
    g.get_cell(x, y).map_or(' ', |c| c.ch)
}

fn style_at(g: &CellCapability, x: u16, y: u16) -> CellStyle {
    g.get_cell(x, y).map(|c| c.style).unwrap_or_default()
}

// =============================================================================
// Helpers
// =============================================================================

fn render(module: &CmdlineModule, w: u16, h: u16) -> CellCapability {
    let mut surface = CellCapability::new(w, h);
    let bounds = Rect {
        x: 0,
        y: 0,
        width: w,
        height: h,
    };
    module.chrome_render(&mut surface, bounds, &MockPlatformCapabilities::new());
    surface
}

// =============================================================================
// Identity tests
// =============================================================================

#[test]
fn identity() {
    let m = CmdlineModule::new();
    assert_eq!(m.id(), "cmdline");
    assert_eq!(m.kind(), "cmdline");
    assert_eq!(m.name(), "Cmdline");
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

#[test]
fn default_is_inactive() {
    let m = CmdlineModule::default();
    assert!(!m.active);
}

// =============================================================================
// Chrome role tests
// =============================================================================

#[test]
fn chrome_role() {
    let m = CmdlineModule::new();
    assert!(m.has_chrome());
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.chrome_position(), ChromePosition::Overlay);
    assert_eq!(m.chrome_priority(), 90);
}

// =============================================================================
// Lifecycle tests
// =============================================================================

#[test]
fn lifecycle() {
    let mut m = CmdlineModule::new();
    assert!(m.exit().is_ok());
}

// =============================================================================
// Notification tests
// =============================================================================

#[test]
fn notification_activates() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#);
    assert!(m.active);
    assert_eq!(m.prompt, ":");
    assert_eq!(m.input, "wq");
    assert_eq!(m.cursor, 2);
}

#[test]
fn notification_deactivates() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#);
    assert!(m.active);

    m.on_notification(r#"{"active":false}"#);
    assert!(!m.active);
}

#[test]
fn notification_invalid_json() {
    let mut m = CmdlineModule::new();
    m.on_notification("not json{{{");
    assert!(!m.active);
}

#[test]
fn notification_with_completions() {
    let mut m = CmdlineModule::new();
    m.on_notification(
        r#"{"active":true,"prompt":":","input":"w","cursor":1,"completions":["write","wq","wall"],"completion_index":0}"#,
    );
    assert_eq!(m.completions.len(), 3);
    assert_eq!(m.completions[0], "write");
    assert_eq!(m.completions[1], "wq");
    assert_eq!(m.completions[2], "wall");
    assert_eq!(m.completion_index, Some(0));
}

#[test]
fn notification_search_prompt() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":"/","input":"hello","cursor":5}"#);
    assert_eq!(m.prompt, "/");
    assert_eq!(m.input, "hello");
    assert_eq!(m.cursor, 5);
}

// =============================================================================
// Cursor position tests
// =============================================================================

#[test]
fn cursor_position_when_active() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#);

    let pos = m.cursor_position(80, 24);
    assert!(pos.is_some());
    let (x, y) = pos.unwrap();
    // y should be 2 (popup at y=1, content at y=2)
    assert_eq!(y, 2);
    // x = popup_x + 2 (border) + 1 (prompt ":") + 2 (cursor pos)
    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    assert_eq!(x, px + 2 + 1 + 2);
}

#[test]
fn cursor_position_when_inactive() {
    let m = CmdlineModule::new();
    assert!(m.cursor_position(80, 24).is_none());
}

// =============================================================================
// Render tests
// =============================================================================

#[test]
fn render_inactive_no_op() {
    let m = CmdlineModule::new();
    let surface = render(&m, 80, 24);
    assert!(!has_content(&surface));
}

#[test]
fn render_shows_popup() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#);

    let surface = render(&m, 80, 24);
    assert!(has_content(&surface));

    // Check border at py=1
    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    assert_eq!(char_at(&surface, px, 1), '\u{256D}'); // top-left corner
    assert_eq!(char_at(&surface, px + pw - 1, 1), '\u{256E}'); // top-right corner

    // Check prompt character
    let content_x = px + 2;
    assert_eq!(char_at(&surface, content_x, 2), ':');
}

#[test]
fn render_shows_completions() {
    let mut m = CmdlineModule::new();
    m.on_notification(
        r#"{"active":true,"prompt":":","input":"w","cursor":1,"completions":["write","wq"],"completion_index":0}"#,
    );

    let surface = render(&m, 80, 24);

    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    let content_x = px + 2;

    // Height = 3 (border+content+border) + 2 completions = 5
    // Bottom border should be at y = 1 + 4 = 5
    assert_eq!(char_at(&surface, px, 5), '\u{2570}'); // bottom-left

    // Selected item should have indicator
    assert_eq!(char_at(&surface, content_x, 3), '\u{25B8}'); // selected indicator

    // Second completion should have space indicator (not selected)
    assert_eq!(char_at(&surface, content_x, 4), ' ');
}

#[test]
fn render_cursor_style() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"hello","cursor":2}"#);

    let surface = render(&m, 80, 24);

    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    let content_x = px + 2;
    // cursor at position 2 in input, prompt is 1 char
    let cursor_x = content_x + 1 + 2;

    // Cursor should have white bg and black fg (apply_style overrides)
    let style = style_at(&surface, cursor_x, 2);
    assert_eq!(style.bg, Some(reovim_ext_client_tui_cap_cell::CellColor::Named(15)));
    assert_eq!(style.fg, Some(reovim_ext_client_tui_cap_cell::CellColor::Named(0)));
}

#[test]
fn render_cursor_at_end_of_input() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"ab","cursor":2}"#);

    let surface = render(&m, 80, 24);

    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    let content_x = px + 2;
    // cursor at position 2 (past end of 2-char input), prompt is 1 char
    let cursor_x = content_x + 1 + 2;

    // When cursor is at end, a space with cursor style is written
    let style = style_at(&surface, cursor_x, 2);
    assert_eq!(style.bg, Some(reovim_ext_client_tui_cap_cell::CellColor::Named(15)));
    assert_eq!(style.fg, Some(reovim_ext_client_tui_cap_cell::CellColor::Named(0)));
    assert_eq!(char_at(&surface, cursor_x, 2), ' ');
}

#[test]
fn render_narrow_terminal() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"test","cursor":4}"#);

    // Narrow terminal (32 wide)
    let surface = render(&m, 32, 24);

    // Should render without panic
    let pw = reovim_client_driver::chrome_utils::popup_width(32);
    let px = reovim_client_driver::chrome_utils::popup_x(32, pw);
    assert_eq!(char_at(&surface, px, 1), '\u{256D}');
}

#[test]
fn render_overflow_narrow_popup() {
    let mut m = CmdlineModule::new();
    // Long input + completions that exceed popup content width
    m.on_notification(
        r#"{"active":true,"prompt":":","input":"this_is_a_very_long_command_that_overflows","cursor":42,"completions":["this_is_a_very_long_completion_candidate"],"completion_index":0}"#,
    );

    // Very narrow terminal -- content overflows trigger break branches
    let surface = render(&m, 16, 24);
    // Should render without panic -- overflow branches hit
    assert!(has_content(&surface));
}

#[test]
fn render_prompt_style() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"","cursor":0}"#);

    let surface = render(&m, 80, 24);

    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    let content_x = px + 2;

    // Prompt should have yellow foreground
    let style = style_at(&surface, content_x, 2);
    assert_eq!(style.fg, Some(reovim_ext_client_tui_cap_cell::CellColor::Named(11)));
}

#[test]
fn render_border_style() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"","cursor":0}"#);

    let surface = render(&m, 80, 24);

    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);

    // Border should have DarkGrey foreground
    let style = style_at(&surface, px, 1);
    assert_eq!(style.fg, Some(reovim_ext_client_tui_cap_cell::CellColor::Named(8)));
}

#[test]
fn render_selected_completion_style() {
    let mut m = CmdlineModule::new();
    m.on_notification(
        r#"{"active":true,"prompt":":","input":"w","cursor":1,"completions":["write","wq"],"completion_index":1}"#,
    );

    let surface = render(&m, 80, 24);

    let pw = reovim_client_driver::chrome_utils::popup_width(80);
    let px = reovim_client_driver::chrome_utils::popup_x(80, pw);
    let content_x = px + 2;

    // First completion (index 0) is not selected -- default style
    let first_style = style_at(&surface, content_x, 3);
    assert_eq!(first_style.bg, None);

    // Second completion (index 1) is selected -- DarkGrey bg, White fg
    let second_style = style_at(&surface, content_x, 4);
    assert_eq!(second_style.bg, Some(reovim_ext_client_tui_cap_cell::CellColor::Named(8)));
    assert_eq!(second_style.fg, Some(reovim_ext_client_tui_cap_cell::CellColor::Named(15)));
}

#[test]
fn notification_no_completions_field() {
    let mut m = CmdlineModule::new();
    m.on_notification(r#"{"active":true,"prompt":":","input":"q","cursor":1}"#);
    assert!(m.completions.is_empty());
    assert!(m.completion_index.is_none());
}

#[test]
fn notification_defaults() {
    let mut m = CmdlineModule::new();
    // Minimal JSON -- only "active" provided
    m.on_notification(r#"{"active":true}"#);
    assert!(m.active);
    assert_eq!(m.prompt, ":");
    assert!(m.input.is_empty());
    assert_eq!(m.cursor, 0);
    assert!(m.completions.is_empty());
    assert!(m.completion_index.is_none());
}
