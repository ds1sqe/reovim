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

use {
    reovim_client_driver::{ChromeSurface, ClientModule},
    reovim_ext_client_tui_cap_cell::CellCapability,
    reovim_ext_client_tui_cap_cell_view::{
        BackendRasterOutput, HalfBlockRasterizer, ViewRasterizer,
    },
};

use crate::{
    LineNumberMode, TuiCoreState,
    layout_mirror::ServerLayoutMirror,
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
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::too_many_arguments)]
pub fn render_frame<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    config: &RenderConfig,
    extensions: &[Box<dyn ClientModule>],
    token_cache: &AnnotationCacheManager,
    theme: &ThemeManager,
    mirror: &ServerLayoutMirror,
) {
    backend.clear();

    let (width, height) = backend.size();
    let content_height = height.saturating_sub(1);

    let caps = TuiPlatformCapabilities::for_test(width, height);
    let sidebar_width = render_engine_bridge::sidebar_width(extensions, &caps);

    // Shared context data (computed once, reused per viewport)
    let fold_ranges = render_engine_bridge::collect_fold_ranges(extensions);
    let virtual_lines = render_engine_bridge::collect_driver_virtual_lines(extensions);
    let remote_clients = build_remote_clients(state);

    let token_provider = render_engine_bridge::TokenProviderAdapter::new(token_cache);
    let theme_provider = render_engine_bridge::ThemeProviderAdapter::new(theme);
    let renderer = reovim_client_driver::viewport::DefaultViewportRenderer;

    if mirror.has_multiple_windows() {
        // Multi-viewport rendering: iterate over layout placements
        for placement in mirror.placements() {
            let viewport = reovim_client_driver::Rect::new(
                placement.x,
                placement.y,
                placement.width,
                placement.height,
            );

            let is_focused = placement.focused;
            let ctx = build_viewport_context_for_window(
                state,
                placement.buffer_id,
                placement.window_id,
                is_focused,
                &remote_clients,
                &fold_ranges,
                &virtual_lines,
                config,
                sidebar_width,
            );

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

        // Draw window separators between adjacent panes
        draw_window_separators(backend, mirror, content_height);
    } else {
        // Single-window fast path (original code)
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

        let viewport = reovim_client_driver::Rect::new(0, 0, width, content_height);
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

    dispatch_chrome_by_view_hint(backend, state, extensions, width, height, &caps);
}

/// Hint-dispatch seam. The focused window's `ViewHint` picks the
/// rasterization path. `FullBlock` writes directly through the
/// backend; `HalfBlock` buffers chrome into a `CellCapability` twice
/// as tall as the terminal and rasterizes with `HalfBlockRasterizer`;
/// `Braille` lands in Flight 75 / `02-braille-rasterizer.md`. The
/// Braille arm is a defence-in-depth `unreachable!()` — the env-var
/// parser (`view_hint_env::parse_view_hint`) rejects `"braille"`
/// until Flight 75, so no user input can reach it.
#[cfg_attr(coverage_nightly, coverage(off))]
fn dispatch_chrome_by_view_hint<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    extensions: &[Box<dyn ClientModule>],
    width: u16,
    height: u16,
    caps: &TuiPlatformCapabilities,
) {
    match state.view_hint_for(state.focused_window_id) {
        reovim_ext_client_tui_cap_cell_view::ViewHint::FullBlock => {
            render_chrome(backend, extensions, width, height, caps);
        }
        reovim_ext_client_tui_cap_cell_view::ViewHint::HalfBlock => {
            dispatch_chrome_halfblock(backend, extensions, width, height);
        }
        reovim_ext_client_tui_cap_cell_view::ViewHint::Braille => {
            unreachable!(
                "ViewHint::Braille rasterizer lands in Flight 75 \
                 (02-braille-rasterizer.md). This arm is gated by \
                 `view_hint_env::parse_view_hint` which rejects \
                 `REOVIM_VIEW_HINT=braille` until the rasterizer \
                 ships; no other path currently constructs this \
                 variant in a WindowViewHints entry."
            );
        }
    }
}

/// `HalfBlock` dispatch: buffer chrome into a logical-height-doubled
/// `CellCapability`, then rasterize via [`HalfBlockRasterizer`] into
/// the backend. Chrome modules see a `TuiPlatformCapabilities` whose
/// reported grid size is `(width, height * 2)` — bottom-docked chrome
/// lands at logical row `height * 2 - 1`, which the rasterizer maps to
/// the terminal's bottom row.
#[cfg_attr(coverage_nightly, coverage(off))]
fn dispatch_chrome_halfblock<B: RenderBackend>(
    backend: &mut B,
    extensions: &[Box<dyn ClientModule>],
    width: u16,
    height: u16,
) {
    let logical_height = height.saturating_mul(2);
    let mut grid = CellCapability::new(width, logical_height);
    let caps_logical = TuiPlatformCapabilities::for_test(width, logical_height);
    render_chrome_into_surface(&mut grid, extensions, width, logical_height, &caps_logical);
    let mut out = BackendRasterOutput::new(backend);
    HalfBlockRasterizer::new().rasterize(&grid, &mut out);
}

// =============================================================================
// Context builders
// =============================================================================

/// Build `RemoteClientInfo` list from TUI state.
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
fn sel_mode_from_str(mode: &str) -> reovim_client_driver::SelectionMode {
    match mode {
        "line" => reovim_client_driver::SelectionMode::Line,
        "block" => reovim_client_driver::SelectionMode::Block,
        _ => reovim_client_driver::SelectionMode::Char,
    }
}

/// Convert TUI `LineNumberMode` to client-driver `LineNumberMode`.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn convert_line_number_mode(mode: LineNumberMode) -> reovim_client_driver::LineNumberMode {
    match mode {
        LineNumberMode::None => reovim_client_driver::LineNumberMode::None,
        LineNumberMode::Absolute => reovim_client_driver::LineNumberMode::Absolute,
        LineNumberMode::Relative => reovim_client_driver::LineNumberMode::Relative,
        LineNumberMode::Hybrid => reovim_client_driver::LineNumberMode::Hybrid,
    }
}

// =============================================================================
// Multi-viewport helpers
// =============================================================================

/// Build a `ViewportContext` for a specific window in multi-viewport mode.
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::cast_possible_truncation, clippy::too_many_arguments)]
fn build_viewport_context_for_window<'a>(
    state: &'a TuiCoreState,
    buffer_id: Option<u64>,
    window_id: u64,
    is_focused: bool,
    remote_clients: &'a [reovim_client_driver::RemoteClientInfo],
    fold_ranges: &'a [(usize, usize)],
    virtual_lines: &'a [reovim_client_driver::VirtualLine],
    config: &RenderConfig,
    sidebar_width: u16,
) -> reovim_client_driver::ViewportContext<'a> {
    let buffer_lines: Option<&[String]> =
        buffer_id.and_then(|id| state.buffer_cache.get(&id).map(Vec::as_slice));

    let cursor = if is_focused {
        state.get_focused_cursor()
    } else {
        state
            .window_cursors
            .get(&window_id)
            .map(|c| crate::core_state::CursorPosition {
                line: c.line,
                column: c.column,
            })
    };

    let scroll_top = state.scroll_tops.get(&window_id).copied().unwrap_or(0);

    let local_selection = if is_focused {
        build_local_selection(state)
    } else {
        None
    };

    reovim_client_driver::ViewportContext {
        buffer_id: buffer_id.map(|id| reovim_client_driver::BufferId(id as usize)),
        buffer_lines,
        cursor: cursor.map(|c| reovim_client_driver::CursorInfo {
            line: c.line,
            column: c.column,
        }),
        scroll_top,
        local_selection,
        remote_clients,
        fold_ranges,
        virtual_lines,
        opacity: config.opacity,
        line_number_mode: convert_line_number_mode(config.line_number_mode),
        gutter_width: config.gutter_width,
        sidebar_width,
        is_insert_mode: state.is_insert_mode(),
        render_self_cursor: config.render_self_cursor,
        my_client_id: state.my_client_id,
    }
}

/// Draw separators between adjacent windows.
#[cfg_attr(coverage_nightly, coverage(off))]
fn draw_window_separators<B: RenderBackend>(
    backend: &mut B,
    mirror: &ServerLayoutMirror,
    _content_height: u16,
) {
    let sep_style = reovim_driver_display::Style {
        fg: Some(reovim_arch::Color::DarkGrey),
        ..reovim_driver_display::Style::default()
    };

    let placements = mirror.placements();
    for p in placements {
        // Draw vertical separator on the left edge of non-leftmost windows
        if p.x > 0 {
            let sep_x = p.x - 1;
            for y in p.y..p.y + p.height {
                backend.set_cell(sep_x, y, '\u{2502}', &sep_style);
            }
        }
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
#[cfg_attr(coverage_nightly, coverage(off))]
fn render_chrome<B: RenderBackend>(
    backend: &mut B,
    extensions: &[Box<dyn ClientModule>],
    width: u16,
    height: u16,
    caps: &dyn reovim_client_driver::PlatformCapabilities,
) {
    let mut surface = BackendSurfaceAdapter::new(backend);
    render_chrome_into_surface(&mut surface, extensions, width, height, caps);
}

/// Chrome-module layout + render loop parameterised on the destination
/// surface. `render_chrome` wraps the backend; `dispatch_chrome_halfblock`
/// passes a `CellCapability` (which implements `ChromeSurface` directly)
/// so the half-block rasterizer can project it onto the backend in a
/// second pass.
#[cfg_attr(coverage_nightly, coverage(off))]
fn render_chrome_into_surface(
    surface: &mut dyn ChromeSurface,
    extensions: &[Box<dyn ClientModule>],
    width: u16,
    height: u16,
    caps: &dyn reovim_client_driver::PlatformCapabilities,
) {
    use reovim_client_driver::ChromePosition;

    let mut chrome_modules: Vec<(usize, &Box<dyn ClientModule>)> = extensions
        .iter()
        .enumerate()
        .filter(|(_, ext)| ext.has_chrome())
        .collect();

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
            ChromePosition::Overlay => reovim_client_driver::Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
        };

        ext.chrome_render(surface, bounds, caps);
    }
}

#[cfg(test)]
#[path = "render_engine_tests.rs"]
mod tests;
