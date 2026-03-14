//! Bridge helpers for `render_engine.rs` to work with `ClientModule`.
//!
//! Provides type conversion functions between client-driver types and display
//! types, plus adapter structs for the render engine to call `ClientModule`
//! methods through the existing `RenderBackend` infrastructure.
//!
//! Temporary -- deleted in Phase 12.4 when the render engine is rewritten
//! to use client-driver types natively.

use {
    reovim_client_driver::{
        ChromePosition, ClientModule, ColorDepth, Insets, PlatformCapabilities, Rect,
        RenderSurface, RenderingModel,
    },
    reovim_driver_display::{
        Attributes as DisplayAttributes, ColorMode, DisplayCapabilities, Style as DisplayStyle,
        render_backend::RenderBackend,
    },
};

use reovim_client_driver::types::Color;

// =============================================================================
// BackendSurfaceAdapter
// =============================================================================

/// Wraps `&mut dyn RenderBackend` as `RenderSurface` for CORE dispatch.
///
/// Used by `render_engine.rs` to call `ClientModule::chrome_render()` with its
/// existing `RenderBackend`. The bridge's `SurfaceBackendAdapter` then wraps
/// this back to `RenderBackend` — a double-wrap that is temporary and has
/// negligible cost for the few chrome extensions.
pub struct BackendSurfaceAdapter<'a> {
    backend: &'a mut dyn RenderBackend,
}

impl<'a> BackendSurfaceAdapter<'a> {
    pub fn new(backend: &'a mut dyn RenderBackend) -> Self {
        Self { backend }
    }
}

impl RenderSurface for BackendSurfaceAdapter<'_> {
    fn write_styled(
        &mut self,
        x: u16,
        y: u16,
        text: &str,
        style: reovim_client_driver::Style,
    ) -> u16 {
        let display_style = convert_driver_style_to_display_style(&style);
        self.backend.write_str(x, y, text, &display_style)
    }

    fn apply_style(&mut self, x: u16, y: u16, style: reovim_client_driver::Style) {
        let display_style = convert_driver_style_to_display_style(&style);
        self.backend.apply_style(x, y, &display_style);
    }

    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color) {
        self.backend.overlay_bg(x, y, bg);
    }

    fn fill(&mut self, rect: Rect, ch: char, style: reovim_client_driver::Style) {
        let display_style = convert_driver_style_to_display_style(&style);
        self.backend
            .fill_region(rect.x, rect.y, rect.width, rect.height, ch, &display_style);
    }

    fn clear(&mut self, rect: Rect) {
        let display_style = DisplayStyle::default();
        self.backend
            .fill_region(rect.x, rect.y, rect.width, rect.height, ' ', &display_style);
    }

    fn size(&self) -> (u16, u16) {
        self.backend.size()
    }
}

// =============================================================================
// TuiRenderSurface (generic)
// =============================================================================

/// Generic `RenderSurface` wrapping a concrete `RenderBackend`.
///
/// Unlike `BackendSurfaceAdapter` (which uses `dyn RenderBackend`), this is
/// generic over `B` for monomorphization in the hot render path.
/// Both adapters coexist: use `BackendSurfaceAdapter` when the backend type
/// is erased, `TuiRenderSurface` when the concrete type is known.
pub struct TuiRenderSurface<'a, B: RenderBackend> {
    backend: &'a mut B,
}

impl<'a, B: RenderBackend> TuiRenderSurface<'a, B> {
    /// Wrap a concrete `RenderBackend` as `RenderSurface`.
    pub const fn new(backend: &'a mut B) -> Self {
        Self { backend }
    }
}

impl<B: RenderBackend> RenderSurface for TuiRenderSurface<'_, B> {
    fn write_styled(
        &mut self,
        x: u16,
        y: u16,
        text: &str,
        style: reovim_client_driver::Style,
    ) -> u16 {
        let display_style = convert_driver_style_to_display_style(&style);
        self.backend.write_str(x, y, text, &display_style)
    }

    fn apply_style(&mut self, x: u16, y: u16, style: reovim_client_driver::Style) {
        let display_style = convert_driver_style_to_display_style(&style);
        self.backend.apply_style(x, y, &display_style);
    }

    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color) {
        self.backend.overlay_bg(x, y, bg);
    }

    fn fill(&mut self, rect: Rect, ch: char, style: reovim_client_driver::Style) {
        let display_style = convert_driver_style_to_display_style(&style);
        self.backend
            .fill_region(rect.x, rect.y, rect.width, rect.height, ch, &display_style);
    }

    fn clear(&mut self, rect: Rect) {
        let display_style = DisplayStyle::default();
        self.backend
            .fill_region(rect.x, rect.y, rect.width, rect.height, ' ', &display_style);
    }

    fn size(&self) -> (u16, u16) {
        self.backend.size()
    }
}

// =============================================================================
// TuiPlatformCapabilities
// =============================================================================

/// TUI `PlatformCapabilities` wrapping `DisplayCapabilities` and terminal state.
///
/// Provides platform information for bridge extensions to render chrome.
/// Tracks mutable state (grid size, focus) that changes during the session.
pub struct TuiPlatformCapabilities {
    grid_size: (u16, u16),
    display: DisplayCapabilities,
    dark_mode: bool,
    has_focus: bool,
}

impl TuiPlatformCapabilities {
    /// Create from terminal dimensions and detected display capabilities.
    #[must_use]
    pub fn new(width: u16, height: u16, display: DisplayCapabilities) -> Self {
        Self {
            grid_size: (width, height),
            dark_mode: detect_dark_mode(),
            has_focus: true,
            display,
        }
    }

    /// Create a test-friendly instance with default capabilities.
    ///
    /// Avoids environment-dependent detection in unit tests.
    #[must_use]
    pub const fn for_test(width: u16, height: u16) -> Self {
        Self {
            grid_size: (width, height),
            display: DisplayCapabilities {
                color_mode: ColorMode::TrueColor,
                supports_underline_color: false,
                supports_extended_underlines: false,
                supports_mouse: false,
                supports_kitty_graphics: false,
                supports_sixel: false,
            },
            dark_mode: true,
            has_focus: true,
        }
    }

    /// Update grid size (called on terminal resize).
    pub const fn update_grid_size(&mut self, width: u16, height: u16) {
        self.grid_size = (width, height);
    }

    /// Update focus state.
    pub const fn set_focus(&mut self, focused: bool) {
        self.has_focus = focused;
    }
}

/// Detect dark mode from `COLORFGBG` environment variable.
///
/// The `COLORFGBG` variable is `fg;bg` where bg >= 8 usually means dark.
/// Returns `true` (dark) as the default when the variable is absent or
/// cannot be parsed.
#[must_use]
fn detect_dark_mode() -> bool {
    std::env::var("COLORFGBG")
        .ok()
        .and_then(|val| {
            let bg = val.rsplit(';').next()?;
            bg.parse::<u8>().ok()
        })
        .is_none_or(|bg| bg >= 8 || bg == 0)
}

impl PlatformCapabilities for TuiPlatformCapabilities {
    fn rendering_model(&self) -> RenderingModel {
        RenderingModel::CellGrid
    }

    fn grid_size(&self) -> Option<(u16, u16)> {
        Some(self.grid_size)
    }

    fn color_depth(&self) -> ColorDepth {
        match self.display.color_mode {
            ColorMode::Ansi16 => ColorDepth::Ansi16,
            ColorMode::Color256 => ColorDepth::Ansi256,
            ColorMode::TrueColor => ColorDepth::TrueColor,
        }
    }

    fn pixel_size(&self) -> Option<(u32, u32)> {
        None
    }

    fn reliable_unicode_width(&self) -> bool {
        true
    }

    fn dark_mode(&self) -> bool {
        self.dark_mode
    }

    fn smooth_scroll(&self) -> bool {
        false
    }

    fn pointer_events(&self) -> bool {
        self.display.supports_mouse
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
        self.has_focus
    }

    fn clipboard_available(&self) -> bool {
        true
    }

    fn screen_reader_active(&self) -> bool {
        false
    }
}

// =============================================================================
// Extension query helpers
// =============================================================================

/// Compute sidebar width from chrome extensions with `Left` position.
pub fn sidebar_width(extensions: &[Box<dyn ClientModule>], caps: &dyn PlatformCapabilities) -> u16 {
    extensions
        .iter()
        .filter(|e| e.has_chrome() && e.chrome_position() == ChromePosition::Left)
        .map(|e| e.chrome_requested_size(caps))
        .sum()
}

/// Collect fold ranges from all buffer-contrib extensions.
#[must_use]
pub fn collect_fold_ranges(extensions: &[Box<dyn ClientModule>]) -> Vec<(usize, usize)> {
    extensions
        .iter()
        .filter(|e| e.has_buffer_contrib())
        .flat_map(|e| e.fold_ranges().iter().copied())
        .collect()
}

/// Convert client-driver `Style` to display `Style`.
fn convert_driver_style_to_display_style(style: &reovim_client_driver::Style) -> DisplayStyle {
    let mut attrs = DisplayAttributes::new();

    if style
        .attributes
        .contains(reovim_client_driver::Attributes::BOLD)
    {
        attrs.set(DisplayAttributes::BOLD);
    }
    if style
        .attributes
        .contains(reovim_client_driver::Attributes::ITALIC)
    {
        attrs.set(DisplayAttributes::ITALIC);
    }
    if style
        .attributes
        .contains(reovim_client_driver::Attributes::UNDERLINE)
    {
        attrs.set(DisplayAttributes::UNDERLINE);
    }
    if style
        .attributes
        .contains(reovim_client_driver::Attributes::STRIKETHROUGH)
    {
        attrs.set(DisplayAttributes::STRIKETHROUGH);
    }
    if style
        .attributes
        .contains(reovim_client_driver::Attributes::REVERSE)
    {
        attrs.set(DisplayAttributes::REVERSE);
    }
    if style
        .attributes
        .contains(reovim_client_driver::Attributes::DIM)
    {
        attrs.set(DisplayAttributes::DIM);
    }

    DisplayStyle {
        fg: style.fg,
        bg: style.bg,
        attributes: attrs,
        underline_color: None,
    }
}

// =============================================================================
// Input event conversion: TUI driver -> client-driver
// =============================================================================

/// Convert a TUI driver `KeyEvent` to a platform-agnostic `InputEvent`.
#[must_use]
pub const fn convert_key_event(
    key: &reovim_driver_tui::KeyEvent,
) -> reovim_client_driver::InputEvent {
    use {
        crossterm::event::{KeyCode as CK, KeyModifiers as CM},
        reovim_client_driver::{KeyCode as DK, KeyEvent as DE, Modifiers as DM},
    };

    let code = match key.code {
        CK::Char(c) => DK::Char(c),
        CK::Enter => DK::Enter,
        CK::Esc => DK::Esc,
        CK::Tab | CK::BackTab => DK::Tab,
        CK::Backspace => DK::Backspace,
        CK::Left => DK::Left,
        CK::Right => DK::Right,
        CK::Up => DK::Up,
        CK::Down => DK::Down,
        CK::Home => DK::Home,
        CK::End => DK::End,
        CK::PageUp => DK::PageUp,
        CK::PageDown => DK::PageDown,
        CK::Insert => DK::Insert,
        CK::Delete => DK::Delete,
        CK::F(n) => DK::F(n),
        _ => DK::Null,
    };

    let mut modifiers = DM::new();
    if key.modifiers.contains(CM::SHIFT) {
        modifiers.set(DM::SHIFT);
    }
    if key.modifiers.contains(CM::CONTROL) {
        modifiers.set(DM::CTRL);
    }
    if key.modifiers.contains(CM::ALT) {
        modifiers.set(DM::ALT);
    }

    reovim_client_driver::InputEvent::Key(DE::new(code, modifiers))
}

/// Convert a TUI driver `MouseEvent` to a platform-agnostic `InputEvent`.
#[must_use]
pub fn convert_mouse_event(
    mouse: &reovim_driver_tui::MouseEvent,
) -> reovim_client_driver::InputEvent {
    use {
        crossterm::event::{MouseButton as CB, MouseEventKind as MK},
        reovim_client_driver::{Modifiers, PointerButton as PB, PointerEvent, PointerKind},
    };

    let button_map = |b: CB| match b {
        CB::Left => PB::Left,
        CB::Right => PB::Right,
        CB::Middle => PB::Middle,
    };

    let kind = match mouse.kind {
        MK::Down(b) => PointerKind::Down(button_map(b)),
        MK::Up(b) => PointerKind::Up(button_map(b)),
        MK::Drag(b) => PointerKind::Drag(button_map(b)),
        MK::Moved => PointerKind::Move,
        MK::ScrollUp => PointerKind::ScrollUp,
        MK::ScrollDown | MK::ScrollLeft | MK::ScrollRight => PointerKind::ScrollDown,
    };

    reovim_client_driver::InputEvent::Pointer(PointerEvent {
        kind,
        x: mouse.column,
        y: mouse.row,
        modifiers: Modifiers::NONE,
    })
}

// =============================================================================
// Type conversion: display -> client-driver
// =============================================================================

/// Convert display `Style` to client-driver `Style`.
const fn convert_display_style_to_driver_style(
    style: &reovim_driver_display::Style,
) -> reovim_client_driver::Style {
    let mut attrs = reovim_client_driver::Attributes::new();

    if style.attributes.contains(DisplayAttributes::BOLD) {
        attrs.set(reovim_client_driver::Attributes::BOLD);
    }
    if style.attributes.contains(DisplayAttributes::ITALIC) {
        attrs.set(reovim_client_driver::Attributes::ITALIC);
    }
    if style.attributes.contains(DisplayAttributes::UNDERLINE) {
        attrs.set(reovim_client_driver::Attributes::UNDERLINE);
    }
    if style.attributes.contains(DisplayAttributes::STRIKETHROUGH) {
        attrs.set(reovim_client_driver::Attributes::STRIKETHROUGH);
    }
    if style.attributes.contains(DisplayAttributes::REVERSE) {
        attrs.set(reovim_client_driver::Attributes::REVERSE);
    }
    if style.attributes.contains(DisplayAttributes::DIM) {
        attrs.set(reovim_client_driver::Attributes::DIM);
    }

    reovim_client_driver::Style {
        fg: style.fg,
        bg: style.bg,
        attributes: attrs,
    }
}

// =============================================================================
// TokenProvider adapter
// =============================================================================

/// Wraps `AnnotationCacheManager` as `TokenProvider` for the viewport renderer.
pub struct TokenProviderAdapter<'a> {
    cache: &'a reovim_driver_display::AnnotationCacheManager,
}

impl<'a> TokenProviderAdapter<'a> {
    #[must_use]
    pub const fn new(cache: &'a reovim_driver_display::AnnotationCacheManager) -> Self {
        Self { cache }
    }
}

impl reovim_client_driver::TokenProvider for TokenProviderAdapter<'_> {
    #[allow(clippy::cast_possible_truncation)]
    fn tokens_for_line(
        &self,
        buffer_id: reovim_client_driver::BufferId,
        line: u32,
    ) -> Vec<reovim_client_driver::SyntaxToken> {
        self.cache
            .tokens_for_line(buffer_id.0 as u64, line)
            .into_iter()
            .map(|t| reovim_client_driver::SyntaxToken {
                line: t.line,
                start_col: t.start_col,
                end_col: t.end_col,
                category: t.category.clone(),
            })
            .collect()
    }
}

// =============================================================================
// ThemeProvider adapter
// =============================================================================

/// Wraps `ThemeManager` as `ThemeProvider` for the viewport renderer.
pub struct ThemeProviderAdapter<'a> {
    theme: &'a reovim_driver_display::ThemeManager,
}

impl<'a> ThemeProviderAdapter<'a> {
    #[must_use]
    pub const fn new(theme: &'a reovim_driver_display::ThemeManager) -> Self {
        Self { theme }
    }
}

impl reovim_client_driver::ThemeProvider for ThemeProviderAdapter<'_> {
    fn highlight(&self, group: &str) -> reovim_client_driver::Style {
        convert_display_style_to_driver_style(&self.theme.get_style(group))
    }

    fn highlight_with_fallback(&self, groups: &[&str]) -> reovim_client_driver::Style {
        for group in groups {
            let style = self.theme.get_style(group);
            if style != reovim_driver_display::Style::default() {
                return convert_display_style_to_driver_style(&style);
            }
        }
        reovim_client_driver::Style::default()
    }

    fn foreground(&self) -> reovim_client_driver::Style {
        convert_display_style_to_driver_style(&self.theme.get_style("Normal"))
    }

    fn background(&self) -> reovim_client_driver::Style {
        let style = self.theme.get_style("Normal");
        reovim_client_driver::Style {
            fg: None,
            bg: style.bg,
            attributes: reovim_client_driver::Attributes::new(),
        }
    }

    fn is_dark(&self) -> bool {
        // Heuristic: if Normal bg is dark or absent, assume dark
        let style = self.theme.get_style("Normal");
        style.bg.is_none_or(|c| {
            let Color::Rgb { r, g, b } = c else {
                return true;
            };
            // Luminance < 128 = dark
            (u16::from(r) + u16::from(g) + u16::from(b)) / 3 < 128
        })
    }
}

/// Collect virtual lines from modules as client-driver types (no display conversion).
#[must_use]
pub fn collect_driver_virtual_lines(
    extensions: &[Box<dyn ClientModule>],
) -> Vec<reovim_client_driver::VirtualLine> {
    extensions
        .iter()
        .filter(|e| e.has_buffer_contrib())
        .flat_map(|e| e.virtual_lines().iter().cloned())
        .collect()
}

#[cfg(test)]
#[path = "render_engine_bridge_tests.rs"]
mod tests;
