//! Thin compositor for TUI rendering.
//!
//! This module acts as the CORE compositor: it builds a `ViewportContext` from
//! TUI state, delegates buffer rendering to `DefaultViewportRenderer`, and
//! dispatches chrome rendering to modules via position-based allocation.
//!
//! The heavy rendering logic (syntax, conceals, selections, cursors) lives in
//! `reovim_client_driver::viewport::DefaultViewportRenderer`.

use {
    reovim_arch::Color,
    reovim_driver_display::{AnnotationCacheManager, ThemeManager},
};

use reovim_client_driver::ClientModule;

use crate::{
    LineNumberMode, TuiCoreState,
    render_backend::RenderBackend,
    render_engine_bridge::{self, BackendSurfaceAdapter, TuiPlatformCapabilities},
};

/// Render configuration for a frame.
///
/// Controls what features are rendered and provides optional data
/// like syntax tokens and theme styles.
#[derive(Debug)]
pub struct RenderConfig {
    /// Whether to show line numbers in the gutter.
    pub show_line_numbers: bool,
    /// Line number display mode.
    pub line_number_mode: LineNumberMode,
    /// Whether to render self cursor in the backend.
    ///
    /// - `false`: Interactive mode (uses terminal cursor)
    /// - `true`: Headless mode (cursor rendered in buffer)
    pub render_self_cursor: bool,
    /// Gutter width (for line numbers).
    pub gutter_width: u16,
    /// Window opacity for the current render pass (#400).
    ///
    /// When < 1.0, all text styles are dimmed via `dim_style()`.
    /// When == 0.0, the window is fully transparent (skip rendering).
    /// Default: 1.0 (fully opaque).
    pub opacity: f32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            show_line_numbers: false,
            line_number_mode: LineNumberMode::None,
            render_self_cursor: false,
            gutter_width: 0,
            opacity: 1.0,
        }
    }
}

/// CBF-8 colorblind-friendly palette for remote cursors.
///
/// Re-exported from client-driver viewport module for use by TUI code
/// that still references `render_engine` palettes.
pub const CBF8_PALETTE: [Color; 8] = reovim_client_driver::viewport::CBF8_PALETTE;

/// Get a color from the CBF-8 palette for a client ID.
#[must_use]
pub const fn client_color(client_id: u64) -> Color {
    reovim_client_driver::viewport::client_color(client_id)
}

/// Dimmed CBF-8 palette for selection backgrounds.
pub const CBF8_DIMMED: [Color; 8] = reovim_client_driver::viewport::CBF8_DIMMED;

/// Get a dimmed color for selection backgrounds.
#[must_use]
pub const fn dimmed_client_color(client_id: u64) -> Color {
    reovim_client_driver::viewport::dimmed_client_color(client_id)
}

/// Render a complete frame to the backend.
///
/// This is the single entry point for all TUI rendering.
/// Both interactive and headless TUIs call this function.
///
/// The render engine acts as a thin compositor:
/// 1. Clear → 2. Build context → 3. Viewport rendering → 4. Chrome rendering
///
/// Buffer content, selections, and cursors are delegated to
/// `DefaultViewportRenderer` via the `ViewportRenderer` trait.
/// Chrome is rendered by CORE dispatch (position-based allocation).
pub fn render_frame<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    config: &RenderConfig,
    extensions: &[Box<dyn ClientModule>],
    token_cache: &AnnotationCacheManager,
    theme: &ThemeManager,
) {
    backend.clear();

    let (width, height) = backend.size();
    let content_height = height.saturating_sub(1);

    let caps = TuiPlatformCapabilities::for_test(width, height);
    let sidebar_width = render_engine_bridge::sidebar_width(extensions, &caps);

    // Build ViewportContext from TUI state
    let fold_ranges = render_engine_bridge::collect_fold_ranges(extensions);
    let virtual_lines = render_engine_bridge::collect_driver_virtual_lines(extensions);
    let remote_clients = build_remote_clients(state);
    let local_selection = build_local_selection(state);

    let buffer_id = state.get_focused_buffer_id();
    let buffer_lines: Option<&[String]> =
        buffer_id.and_then(|id| state.buffer_cache.get(&id).map(Vec::as_slice));

    #[allow(clippy::cast_possible_truncation)]
    let ctx = reovim_client_driver::ViewportContext {
        buffer_id: buffer_id.map(|id| reovim_client_driver::BufferId(id as usize)),
        buffer_lines,
        cursor: state
            .get_focused_cursor()
            .map(|c| reovim_client_driver::CursorInfo {
                line: c.line,
                column: c.column,
            }),
        scroll_top: state.get_focused_scroll_top(),
        local_selection,
        remote_clients: &remote_clients,
        fold_ranges: &fold_ranges,
        virtual_lines: &virtual_lines,
        opacity: config.opacity,
        line_number_mode: convert_line_number_mode(config.line_number_mode),
        gutter_width: config.gutter_width,
        sidebar_width,
        is_insert_mode: state.is_insert_mode(),
        render_self_cursor: config.render_self_cursor,
        my_client_id: state.my_client_id,
    };

    // Adapters for display-driver types → client-driver traits
    let token_provider = render_engine_bridge::TokenProviderAdapter::new(token_cache);
    let theme_provider = render_engine_bridge::ThemeProviderAdapter::new(theme);

    // Viewport rendering (buffer content, selections, cursors)
    let viewport = reovim_client_driver::Rect::new(0, 0, width, content_height);
    let renderer = reovim_client_driver::viewport::DefaultViewportRenderer;
    {
        let mut surface = render_engine_bridge::TuiRenderSurface::new(backend);
        reovim_client_driver::ViewportRenderer::render_viewport(
            &renderer,
            &mut surface,
            viewport,
            &ctx,
            extensions,
            &token_provider,
            &theme_provider,
            &caps,
        );
    }

    // Chrome rendering (CORE dispatch — position-based allocation)
    render_chrome(backend, extensions, width, height, &caps);
}

// =============================================================================
// Context builders
// =============================================================================

/// Build `RemoteClientInfo` list from TUI state.
#[allow(clippy::cast_possible_truncation)]
fn build_remote_clients(state: &TuiCoreState) -> Vec<reovim_client_driver::RemoteClientInfo> {
    let current_buffer_id = state.windows.first().and_then(|w| w.buffer_id);
    state
        .other_clients
        .values()
        .filter(|r| r.buffer_id == current_buffer_id)
        .map(|r| reovim_client_driver::RemoteClientInfo {
            client_id: r.client_id,
            display_name: r.display_name.clone(),
            cursor_line: r.cursor_line,
            cursor_col: r.cursor_col,
            mode: r.mode.clone(),
            cursor_color: reovim_client_driver::viewport::client_color(r.client_id),
            selection: r.selection.as_ref().map(|sel| {
                let (sl, sc, el, ec) =
                    if (sel.start.line, sel.start.column) <= (sel.end.line, sel.end.column) {
                        (sel.start.line, sel.start.column, sel.end.line, sel.end.column)
                    } else {
                        (sel.end.line, sel.end.column, sel.start.line, sel.start.column)
                    };
                reovim_client_driver::SelectionInfo {
                    start_line: sl,
                    start_col: sc,
                    end_line: el,
                    end_col: ec,
                    mode: sel_mode_from_str(&sel.mode),
                    color: reovim_client_driver::viewport::dimmed_client_color(r.client_id),
                }
            }),
        })
        .collect()
}

/// Build local `SelectionInfo` from TUI state.
fn build_local_selection(state: &TuiCoreState) -> Option<reovim_client_driver::SelectionInfo> {
    let sel = state.window_selections.get(&state.focused_window_id)?;
    let (sl, sc, el, ec) = if (sel.start.line, sel.start.column) <= (sel.end.line, sel.end.column) {
        (sel.start.line, sel.start.column, sel.end.line, sel.end.column)
    } else {
        (sel.end.line, sel.end.column, sel.start.line, sel.start.column)
    };
    Some(reovim_client_driver::SelectionInfo {
        start_line: sl,
        start_col: sc,
        end_line: el,
        end_col: ec,
        mode: sel_mode_from_str(&sel.mode),
        color: reovim_client_driver::viewport::LOCAL_SELECTION_BG,
    })
}

/// Convert selection mode string to enum.
fn sel_mode_from_str(mode: &str) -> reovim_client_driver::SelectionMode {
    match mode {
        "line" => reovim_client_driver::SelectionMode::Line,
        "block" => reovim_client_driver::SelectionMode::Block,
        _ => reovim_client_driver::SelectionMode::Char,
    }
}

/// Convert TUI `LineNumberMode` to client-driver `LineNumberMode`.
const fn convert_line_number_mode(mode: LineNumberMode) -> reovim_client_driver::LineNumberMode {
    match mode {
        LineNumberMode::None => reovim_client_driver::LineNumberMode::None,
        LineNumberMode::Absolute => reovim_client_driver::LineNumberMode::Absolute,
        LineNumberMode::Relative => reovim_client_driver::LineNumberMode::Relative,
        LineNumberMode::Hybrid => reovim_client_driver::LineNumberMode::Hybrid,
    }
}

// =============================================================================
// Chrome rendering
// =============================================================================

/// Render all chrome modules with priority-based allocation.
///
/// Modules are sorted by priority (highest first). Each module gets a
/// `Rect` synthesized from its `chrome_position()`:
/// - `Bottom`: allocated from the bottom edge upward
/// - `Left`: allocated from the left edge rightward
/// - `Overlay`: full screen (overlays on top of everything)
/// - `Top`/`Right`: allocated from top/right edges (not currently used)
fn render_chrome<B: RenderBackend>(
    backend: &mut B,
    extensions: &[Box<dyn ClientModule>],
    width: u16,
    height: u16,
    caps: &dyn reovim_client_driver::PlatformCapabilities,
) {
    use reovim_client_driver::ChromePosition;

    // Collect chrome modules with their indices for stable sort
    let mut chrome_modules: Vec<(usize, &Box<dyn ClientModule>)> = extensions
        .iter()
        .enumerate()
        .filter(|(_, ext)| ext.has_chrome())
        .collect();

    // Sort by priority descending (highest priority gets allocated first)
    chrome_modules.sort_by_key(|b| std::cmp::Reverse(b.1.chrome_priority()));

    let mut allocated_bottom: u16 = 0;
    let mut allocated_left: u16 = 0;
    let mut allocated_top: u16 = 0;
    let mut allocated_right: u16 = 0;

    for (_, ext) in &chrome_modules {
        let size = ext.chrome_requested_size(caps);
        let bounds = match ext.chrome_position() {
            ChromePosition::Bottom => {
                let y = height.saturating_sub(allocated_bottom + size);
                allocated_bottom += size;
                reovim_client_driver::Rect {
                    x: 0,
                    y,
                    width,
                    height: size,
                }
            }
            ChromePosition::Top => {
                let y = allocated_top;
                allocated_top += size;
                reovim_client_driver::Rect {
                    x: 0,
                    y,
                    width,
                    height: size,
                }
            }
            ChromePosition::Left => {
                let x = allocated_left;
                allocated_left += size;
                reovim_client_driver::Rect {
                    x,
                    y: 0,
                    width: size,
                    height,
                }
            }
            ChromePosition::Right => {
                let x = width.saturating_sub(allocated_right + size);
                allocated_right += size;
                reovim_client_driver::Rect {
                    x,
                    y: 0,
                    width: size,
                    height,
                }
            }
            ChromePosition::Overlay => {
                // Overlays get full screen bounds
                reovim_client_driver::Rect {
                    x: 0,
                    y: 0,
                    width,
                    height,
                }
            }
        };

        let mut surface = BackendSurfaceAdapter::new(backend);
        ext.chrome_render(&mut surface, bounds, caps);
    }
}

#[cfg(test)]
#[path = "render_engine_tests.rs"]
mod tests;
