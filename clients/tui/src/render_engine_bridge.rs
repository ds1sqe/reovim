//! Bridge helpers for `render_engine.rs` to work with `ClientModule`.
//!
//! Provides type conversion functions between client-driver types and display
//! types, plus adapter structs for the render engine to call `ClientModule`
//! methods through the existing `RenderBackend` infrastructure.
//!
//! Temporary -- deleted in Phase 12.4 when the render engine is rewritten
//! to use client-driver types natively.

use reovim_client_driver::{
    ChromePosition, ClientModule, ColorDepth, Insets, PlatformCapabilities, Rect, RenderSurface,
    RenderingModel,
};
use reovim_driver_display::{
    Attributes as DisplayAttributes, Style as DisplayStyle,
    render_backend::{
        RenderBackend, RenderBehavior as DisplayRenderBehavior,
        TransformedLine as DisplayTransformedLine, VirtualLine as DisplayVirtualLine,
        VirtualLinePosition as DisplayVirtualLinePosition,
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
// TuiPlatformCapabilities
// =============================================================================

/// Minimal `PlatformCapabilities` for TUI render dispatch.
///
/// Provides enough information for bridge extensions to render chrome.
pub struct TuiPlatformCapabilities {
    grid_size: (u16, u16),
}

impl TuiPlatformCapabilities {
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self {
            grid_size: (width, height),
        }
    }
}

impl PlatformCapabilities for TuiPlatformCapabilities {
    fn rendering_model(&self) -> RenderingModel {
        RenderingModel::CellGrid
    }

    fn grid_size(&self) -> Option<(u16, u16)> {
        Some(self.grid_size)
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

/// Collect virtual lines from all buffer-contrib extensions, converted to display types.
#[must_use]
pub fn collect_virtual_lines(extensions: &[Box<dyn ClientModule>]) -> Vec<DisplayVirtualLine> {
    extensions
        .iter()
        .filter(|e| e.has_buffer_contrib())
        .flat_map(|e| e.virtual_lines().iter())
        .map(|vl| DisplayVirtualLine {
            buffer_line: vl.buffer_line,
            position: match vl.position {
                reovim_client_driver::VirtualLinePosition::Before => {
                    DisplayVirtualLinePosition::Before
                }
                reovim_client_driver::VirtualLinePosition::After => {
                    DisplayVirtualLinePosition::After
                }
            },
            content: vl.content.clone(),
            style: convert_driver_style_to_display_style(&vl.style),
        })
        .collect()
}

/// Classify a token via extension dispatch, returning display driver `RenderBehavior`.
#[must_use]
pub fn classify_with_extensions(
    extensions: &[Box<dyn ClientModule>],
    category: &str,
) -> DisplayRenderBehavior {
    extensions
        .iter()
        .filter(|e| e.has_buffer_contrib())
        .find_map(|e| {
            e.classify_token(category)
                .map(|rb| convert_driver_rb_to_display(&rb))
        })
        .unwrap_or(DisplayRenderBehavior::Highlight)
}

/// Get `transform_line` result converted to display type.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn transform_line(
    extensions: &[Box<dyn ClientModule>],
    buffer_id: u64,
    line_idx: usize,
    line: &str,
) -> Option<DisplayTransformedLine> {
    extensions
        .iter()
        .filter(|e| e.has_buffer_contrib())
        .find_map(|e| {
            e.transform_line(
                reovim_client_driver::BufferId(buffer_id as usize),
                line_idx,
                line,
            )
            .map(|tl| convert_driver_tl_to_display(&tl))
        })
}

/// Get `map_cursor_column` result via extension dispatch.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn map_cursor_column(
    extensions: &[Box<dyn ClientModule>],
    buffer_id: u64,
    line_idx: usize,
    col: usize,
) -> Option<u16> {
    extensions.iter().filter(|e| e.has_buffer_contrib()).find_map(|e| {
        e.map_cursor_column(
            reovim_client_driver::BufferId(buffer_id as usize),
            line_idx,
            col,
        )
    })
}

/// Get visual line length (from `transform_line` or raw text).
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn visual_line_len(
    extensions: &[Box<dyn ClientModule>],
    buffer_id: u64,
    line_idx: usize,
    line_text: Option<&String>,
) -> Option<u16> {
    extensions
        .iter()
        .filter(|e| e.has_buffer_contrib())
        .find_map(|e| {
            e.transform_line(
                reovim_client_driver::BufferId(buffer_id as usize),
                line_idx,
                line_text.map_or("", String::as_str),
            )
        })
        .map(|t| {
            #[allow(clippy::cast_possible_truncation)]
            let len = t.segments.iter().map(|(s, _)| s.chars().count()).sum::<usize>() as u16;
            len
        })
        .or_else(|| {
            #[allow(clippy::cast_possible_truncation)]
            line_text.map(|s| s.len() as u16)
        })
}

// =============================================================================
// Type conversion: client-driver -> display
// =============================================================================

/// Convert client-driver `RenderBehavior` to display `RenderBehavior`.
fn convert_driver_rb_to_display(
    rb: &reovim_client_driver::RenderBehavior,
) -> DisplayRenderBehavior {
    match rb {
        reovim_client_driver::RenderBehavior::Highlight => DisplayRenderBehavior::Highlight,
        reovim_client_driver::RenderBehavior::Conceal { replacement } => {
            DisplayRenderBehavior::Conceal {
                replacement: replacement.clone(),
            }
        }
        reovim_client_driver::RenderBehavior::Background(_) => DisplayRenderBehavior::Background,
        reovim_client_driver::RenderBehavior::Hide => DisplayRenderBehavior::Hide,
        reovim_client_driver::RenderBehavior::FullWidthLine { ch, .. } => {
            DisplayRenderBehavior::FullWidthLine { ch: *ch }
        }
    }
}

/// Convert client-driver `TransformedLine` to display `TransformedLine`.
fn convert_driver_tl_to_display(
    tl: &reovim_client_driver::TransformedLine,
) -> DisplayTransformedLine {
    let mut text = String::new();
    let mut styles = Vec::new();

    for (seg_text, seg_style) in &tl.segments {
        let display_style = seg_style.as_ref().map(convert_driver_style_to_display_style);
        for ch in seg_text.chars() {
            text.push(ch);
            styles.push(display_style.clone());
        }
    }

    DisplayTransformedLine { text, styles }
}

/// Convert client-driver `Style` to display `Style`.
fn convert_driver_style_to_display_style(
    style: &reovim_client_driver::Style,
) -> DisplayStyle {
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

#[cfg(test)]
#[path = "render_engine_bridge_tests.rs"]
mod tests;
