//! Unified render engine for TUI.
//!
//! This module provides a single rendering implementation that works with
//! any `RenderBackend` (`Screen` for interactive, `FrameBuffer` for headless).
//! This is the "thick" layer in the thin-TUI architecture.
//!
//! # Status
//!
//! This is a skeleton for the unified render engine. Full implementation
//! will be migrated from `app.rs` incrementally.

use {
    reovim_arch::Color,
    reovim_driver_display::{
        AnnotationCacheManager, Decoration, Span, Style, ThemeManager, apply_conceals, dim_style,
        source_to_display_col,
        ui::{display_width, truncate_end},
    },
};

use reovim_client_driver::ClientModule;

use crate::{
    LineNumberMode, SelectionState, TuiCoreState,
    render_backend::{RenderBackend, RenderBehavior, TransformedLine, VirtualLine, VirtualLinePosition},
    render_engine_bridge::{
        self, BackendSurfaceAdapter, TuiPlatformCapabilities,
    },
};

// =============================================================================
// Extension-based token classification
// =============================================================================

/// Classify a token category via extension dispatch.
///
/// Queries all buffer-contrib extensions; first `Some` wins.
/// Falls back to `Highlight` if no extension claims the category.
#[cfg_attr(coverage_nightly, coverage(off))]
fn classify_with_extensions(
    extensions: &[Box<dyn ClientModule>],
    category: &str,
) -> RenderBehavior {
    render_engine_bridge::classify_with_extensions(extensions, category)
}

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
/// These 8 colors are distinguishable by people with common color vision
/// deficiencies (protanopia, deuteranopia, tritanopia).
pub const CBF8_PALETTE: [Color; 8] = [
    Color::Rgb {
        r: 0,
        g: 114,
        b: 178,
    }, // Blue
    Color::Rgb {
        r: 230,
        g: 159,
        b: 0,
    }, // Orange
    Color::Rgb {
        r: 86,
        g: 180,
        b: 233,
    }, // Sky blue
    Color::Rgb {
        r: 0,
        g: 158,
        b: 115,
    }, // Green
    Color::Rgb {
        r: 240,
        g: 228,
        b: 66,
    }, // Yellow
    Color::Rgb {
        r: 213,
        g: 94,
        b: 0,
    }, // Vermilion
    Color::Rgb {
        r: 204,
        g: 121,
        b: 167,
    }, // Pink
    Color::Rgb { r: 0, g: 0, b: 0 }, // Black (fallback)
];

/// Get a color from the CBF-8 palette for a client ID.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn client_color(client_id: u64) -> Color {
    // Safe: modulo ensures we stay within palette bounds
    CBF8_PALETTE[(client_id as usize) % CBF8_PALETTE.len()]
}

/// Dimmed CBF-8 palette for selection backgrounds.
///
/// Lower-intensity versions of the cursor palette, suitable for
/// background overlays that don't obscure text.
const CBF8_DIMMED: [Color; 8] = [
    Color::Rgb { r: 0, g: 45, b: 70 }, // Blue
    Color::Rgb { r: 75, g: 50, b: 0 }, // Orange
    Color::Rgb {
        r: 25,
        g: 60,
        b: 75,
    }, // Sky blue
    Color::Rgb { r: 0, g: 55, b: 35 }, // Green
    Color::Rgb {
        r: 70,
        g: 65,
        b: 20,
    }, // Yellow
    Color::Rgb { r: 70, g: 30, b: 0 }, // Vermilion
    Color::Rgb {
        r: 65,
        g: 38,
        b: 55,
    }, // Pink
    Color::Rgb {
        r: 30,
        g: 30,
        b: 30,
    }, // Dark grey (fallback)
];

/// Get a dimmed color for selection backgrounds.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn dimmed_client_color(client_id: u64) -> Color {
    CBF8_DIMMED[(client_id as usize) % CBF8_DIMMED.len()]
}

/// Local selection background color (subtle blue).
const LOCAL_SELECTION_BG: Color = Color::Rgb {
    r: 50,
    g: 50,
    b: 100,
};

/// Apply opacity dimming to a style.
///
/// When opacity is 1.0, returns the style unchanged. When < 1.0,
/// dims the foreground and background colors toward `default_bg`.
#[must_use]
fn apply_opacity(style: &Style, opacity: f32, default_bg: Color) -> Style {
    if (opacity - 1.0).abs() < f32::EPSILON {
        return style.clone();
    }
    dim_style(style, opacity, default_bg)
}

/// Render a complete frame to the backend.
///
/// This is the single entry point for all TUI rendering.
/// Both interactive and headless TUIs call this function.
///
/// # Current Implementation
///
/// This is a basic implementation that renders:
/// - Buffer content with syntax highlighting (tokens from `AnnotationCacheManager`)
/// - Remote selections (dimmed background overlay)
/// - Local selection (background overlay)
/// - Remote cursors (CBF-8 colorblind-friendly palette)
/// - Self cursor (if `render_self_cursor` is true)
/// - Statusline
pub fn render_frame<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    config: &RenderConfig,
    extensions: &[Box<dyn ClientModule>],
    token_cache: &AnnotationCacheManager,
    theme: &ThemeManager,
) {
    // Clear the backend
    backend.clear();

    let (width, height) = backend.size();

    // Reserve space for statusline only (cmdline floats on top)
    let content_height = height.saturating_sub(1);

    // Compute sidebar offset from chrome extensions
    let caps = TuiPlatformCapabilities::for_test(width, height);
    let sidebar_width = render_engine_bridge::sidebar_width(extensions, &caps);
    let content_x = config.gutter_width + sidebar_width;

    // Collect fold hidden ranges from buffer-contrib extensions
    let fold_ranges_usize = render_engine_bridge::collect_fold_ranges(extensions);
    #[allow(clippy::cast_possible_truncation)]
    let fold_ranges: Vec<(u32, u32)> = fold_ranges_usize
        .iter()
        .map(|&(s, c)| (s as u32, c as u32))
        .collect();

    // Collect virtual lines from buffer-contrib extensions (converted to display types)
    let display_virtual_lines = render_engine_bridge::collect_virtual_lines(extensions);
    let virtual_lines: Vec<&VirtualLine> = display_virtual_lines.iter().collect();

    // Render buffer content
    render_buffer_content(
        backend,
        state,
        config,
        content_height,
        sidebar_width,
        &fold_ranges,
        token_cache,
        theme,
        &virtual_lines,
        extensions,
    );

    // Render selections (behind cursors — background overlay)
    render_remote_selections(backend, state, content_x, content_height, &virtual_lines, extensions);
    render_local_selection(backend, state, content_x, content_height, &virtual_lines, extensions);

    // Render cursors (on top of selections)
    render_remote_cursors(backend, state, content_x, content_height, &virtual_lines);
    render_remote_cursor_labels(backend, state, content_x, content_height, &virtual_lines);
    if config.render_self_cursor {
        render_self_cursor(
            backend,
            state,
            content_x,
            content_height,
            token_cache,
            theme,
            &virtual_lines,
            extensions,
        );
    }

    // Render chrome extensions with position-based bounds synthesis.
    // Engine has ZERO knowledge of specific chrome modules.
    let caps = TuiPlatformCapabilities::for_test(width, height);
    render_chrome(backend, extensions, width, height, &caps);
}

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
                reovim_client_driver::Rect { x: 0, y, width, height: size }
            }
            ChromePosition::Top => {
                let y = allocated_top;
                allocated_top += size;
                reovim_client_driver::Rect { x: 0, y, width, height: size }
            }
            ChromePosition::Left => {
                let x = allocated_left;
                allocated_left += size;
                reovim_client_driver::Rect { x, y: 0, width: size, height }
            }
            ChromePosition::Right => {
                let x = width.saturating_sub(allocated_right + size);
                allocated_right += size;
                reovim_client_driver::Rect { x, y: 0, width: size, height }
            }
            ChromePosition::Overlay => {
                // Overlays get full screen bounds
                reovim_client_driver::Rect { x: 0, y: 0, width, height }
            }
        };

        let mut surface = BackendSurfaceAdapter::new(backend);
        ext.chrome_render(&mut surface, bounds, caps);
    }
}

/// Default background color for opacity blending.
///
/// Terminal backgrounds are typically black; this is used as the
/// blend target when dimming styles for transparent windows.
const DEFAULT_BG: Color = Color::Black;

/// Check if a buffer line is hidden by any fold range.
///
/// Returns `true` if `line` falls within any `(start, count)` range,
/// meaning `start <= line < start + count`.
fn is_line_folded(line: usize, fold_ranges: &[(u32, u32)]) -> bool {
    for &(start, count) in fold_ranges {
        let start = start as usize;
        let end = start + count as usize;
        if line >= start && line < end {
            return true;
        }
    }
    false
}

/// Convert a buffer line to a screen row, accounting for virtual lines from extensions.
#[allow(clippy::cast_possible_truncation)]
fn buffer_to_screen_row_vl(
    buffer_line: u64,
    scroll_top: u64,
    virtual_lines: &[&VirtualLine],
) -> u64 {
    let count = virtual_lines
        .iter()
        .filter(|vl| {
            (vl.buffer_line as u64) >= scroll_top && (vl.buffer_line as u64) <= buffer_line
        })
        .count();
    (buffer_line - scroll_top) + count as u64
}

/// Render buffer content.
#[allow(
    clippy::cast_possible_truncation,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn render_buffer_content<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    config: &RenderConfig,
    content_height: u16,
    sidebar_width: u16,
    fold_ranges: &[(u32, u32)],
    token_cache: &AnnotationCacheManager,
    theme: &ThemeManager,
    virtual_lines: &[&VirtualLine],
    extensions: &[Box<dyn ClientModule>],
) {
    let (width, _) = backend.size();
    let gutter_width = config.gutter_width;
    let content_x = sidebar_width + gutter_width;
    let content_width = width - content_x;
    let opacity = config.opacity;

    // TODO(#494): Multi-window — iterate all windows with tiling layout
    let buffer_id = state.windows.first().and_then(|w| w.buffer_id);

    let lines = buffer_id.and_then(|id| state.buffer_cache.get(&id));

    let scroll_top = state.get_focused_scroll_top();

    // Determine cursor line and insert mode for conceal bypass:
    // In insert mode, reveal raw text on the cursor line (no conceals).
    let cursor_line = state.get_focused_cursor().map(|c| c.line as usize);
    let is_insert = state.is_insert_mode();

    // Build visible line indices, skipping folded lines
    let mut screen_row: u16 = 0;
    let mut line_idx = scroll_top;

    while screen_row < content_height {
        // Skip folded lines
        if is_line_folded(line_idx, fold_ranges) {
            line_idx += 1;
            continue;
        }

        // Before the line: render virtual lines with VirtualLinePosition::Before
        for vl in virtual_lines
            .iter()
            .filter(|vl| vl.buffer_line == line_idx && vl.position == VirtualLinePosition::Before)
        {
            render_virtual_line(
                backend,
                content_x,
                screen_row,
                content_width,
                &vl.content,
                &vl.style,
            );
            screen_row += 1;
            if screen_row >= content_height {
                break;
            }
        }
        if screen_row >= content_height {
            break;
        }

        // Render line number if enabled
        if config.show_line_numbers && gutter_width > 0 {
            render_line_number(
                backend,
                sidebar_width,
                screen_row,
                gutter_width,
                line_idx,
                cursor_line.unwrap_or(0),
                config,
            );
        }

        // Render line content
        if let Some(lines) = lines {
            if line_idx < lines.len() {
                let line = &lines[line_idx];

                // Check if any extension wants to transform this line
                let transform =
                    render_engine_bridge::transform_line(extensions, buffer_id.unwrap_or(0), line_idx, line);

                if let Some(transformed) = transform {
                    render_transformed_line(
                        backend,
                        content_x,
                        screen_row,
                        content_width,
                        &transformed,
                    );
                } else {
                    let skip_conceals = is_insert && cursor_line == Some(line_idx);
                    render_line_content(
                        backend,
                        content_x,
                        screen_row,
                        content_width,
                        line,
                        opacity,
                        buffer_id,
                        line_idx,
                        token_cache,
                        theme,
                        skip_conceals,
                        extensions,
                    );
                }
            } else {
                // Empty line indicator
                let tilde_style =
                    apply_opacity(&Style::default().fg(Color::DarkGrey), opacity, DEFAULT_BG);
                backend.set_cell(content_x, screen_row, '~', &tilde_style);
            }
        }

        screen_row += 1;

        // After the line: render virtual lines with VirtualLinePosition::After
        for vl in virtual_lines
            .iter()
            .filter(|vl| vl.buffer_line == line_idx && vl.position == VirtualLinePosition::After)
        {
            if screen_row >= content_height {
                break;
            }
            render_virtual_line(
                backend,
                content_x,
                screen_row,
                content_width,
                &vl.content,
                &vl.style,
            );
            screen_row += 1;
        }

        line_idx += 1;
    }
}

/// Render a virtual line (e.g., table border).
#[allow(clippy::cast_possible_truncation)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn render_virtual_line<B: RenderBackend>(
    backend: &mut B,
    x: u16,
    y: u16,
    width: u16,
    content: &str,
    style: &Style,
) {
    for (col, ch) in content.chars().enumerate() {
        let col_u16 = col as u16;
        if col_u16 >= width {
            break;
        }
        backend.set_cell(x + col_u16, y, ch, style);
    }
}

/// Render a transformed line from an extension.
#[allow(clippy::cast_possible_truncation)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn render_transformed_line<B: RenderBackend>(
    backend: &mut B,
    x: u16,
    y: u16,
    width: u16,
    transformed: &TransformedLine,
) {
    for (col, ch) in transformed.text.chars().enumerate() {
        let col_u16 = col as u16;
        if col_u16 >= width {
            break;
        }
        let style = transformed
            .styles
            .get(col)
            .and_then(|s| s.as_ref())
            .cloned()
            .unwrap_or_default();
        backend.set_cell(x + col_u16, y, ch, &style);
    }
}

/// Render a line number in the gutter.
#[cfg_attr(coverage_nightly, coverage(off))]
fn render_line_number<B: RenderBackend>(
    backend: &mut B,
    x: u16,
    y: u16,
    width: u16,
    line_idx: usize,
    cursor_line: usize,
    config: &RenderConfig,
) {
    let line_num = line_idx + 1; // 1-indexed display

    let (display_num, is_cursor_line) = match config.line_number_mode {
        LineNumberMode::Absolute | LineNumberMode::Hybrid => (line_num, line_idx == cursor_line),
        LineNumberMode::Relative => {
            let rel = if line_idx == cursor_line {
                line_num // Show absolute for cursor line
            } else {
                line_idx.abs_diff(cursor_line)
            };
            (rel, line_idx == cursor_line)
        }
        LineNumberMode::None => return,
    };

    let base_style = if is_cursor_line {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGrey)
    };
    let style = apply_opacity(&base_style, config.opacity, DEFAULT_BG);

    // Right-align the number
    let num_str = display_num.to_string();
    let padding = (width as usize).saturating_sub(num_str.len() + 1);
    let display = format!("{:>width$} ", num_str, width = padding + num_str.len());

    backend.write_str(x, y, &display, &style);
}

/// Render a line of buffer content with syntax highlighting and decorations.
///
/// Handles three annotation kinds:
/// - **Highlight**: Resolve category to style via theme (the common case)
/// - **Conceal**: Hide source text, optionally show replacement text
/// - **Background**: Overlay background color independent of text style
#[allow(clippy::cast_possible_truncation, clippy::too_many_arguments)]
fn render_line_content<B: RenderBackend>(
    backend: &mut B,
    x: u16,
    y: u16,
    width: u16,
    line: &str,
    opacity: f32,
    buffer_id: Option<u64>,
    line_idx: usize,
    token_cache: &AnnotationCacheManager,
    theme: &ThemeManager,
    skip_conceals: bool,
    extensions: &[Box<dyn ClientModule>],
) {
    let tokens = buffer_id
        .map(|bid| token_cache.tokens_for_line(bid, line_idx as u32))
        .unwrap_or_default();

    let default_style = apply_opacity(&Style::default(), opacity, DEFAULT_BG);
    let line_u32 = line_idx as u32;

    // Partition tokens into highlights and decorations
    let mut conceals: Vec<&Decoration> = Vec::new();
    let mut conceal_decorations: Vec<Decoration> = Vec::new();
    let mut backgrounds: Vec<(u32, u32, Style)> = Vec::new();

    for t in &tokens {
        match classify_with_extensions(extensions, &t.category) {
            RenderBehavior::Conceal { replacement } if !skip_conceals => {
                conceal_decorations.push(Decoration::Conceal {
                    span: Span::line(line_u32, t.start_col, t.end_col),
                    replacement: replacement.into_owned(),
                    style: Some(theme.get_style(&t.category)),
                });
            }
            RenderBehavior::Hide if !skip_conceals => {
                conceal_decorations.push(Decoration::Conceal {
                    span: Span::line(line_u32, t.start_col, t.end_col),
                    replacement: String::new(),
                    style: None,
                });
            }
            RenderBehavior::FullWidthLine { ch } if !skip_conceals => {
                conceal_decorations.push(Decoration::Conceal {
                    span: Span::line(line_u32, t.start_col, t.end_col),
                    replacement: ch.to_string().repeat(width as usize),
                    style: Some(theme.get_style(&t.category)),
                });
            }
            RenderBehavior::Background => {
                backgrounds.push((t.start_col, t.end_col, theme.get_style(&t.category)));
            }
            _ => {} // Highlight is default — theme style applied via token iteration
        }
    }

    // Build conceal references
    for d in &conceal_decorations {
        conceals.push(d);
    }

    // Apply conceals to get display text
    let concealed = apply_conceals(line, line_u32, &conceals);

    // Render the display text with syntax highlighting
    for (display_col, ch) in concealed.text.chars().enumerate() {
        let col_u16 = display_col as u16;
        if col_u16 >= width {
            break;
        }

        // Check if this display position has a conceal-provided style
        let style = if let Some(Some(conceal_style)) = concealed.styles.get(display_col) {
            apply_opacity(conceal_style, opacity, DEFAULT_BG)
        } else {
            // Map display column back to source column for highlight lookup
            let source_col = concealed.col_mapping.get(display_col).copied().unwrap_or(0);
            let source_col = u32::from(source_col);

            tokens
                .iter()
                .find(|t| {
                    matches!(
                        classify_with_extensions(extensions, &t.category),
                        RenderBehavior::Highlight
                    ) && source_col >= t.start_col
                        && source_col < t.end_col
                })
                .map_or_else(
                    || default_style.clone(),
                    |t| apply_opacity(&theme.get_style(&t.category), opacity, DEFAULT_BG),
                )
        };

        backend.set_cell(x + col_u16, y, ch, &style);
    }

    // Overlay background tokens
    let line_char_count = line.chars().count() as u32;
    for (start_col, end_col, bg_style) in &backgrounds {
        let bg = apply_opacity(bg_style, opacity, DEFAULT_BG);
        for source_col in *start_col..(*end_col).min(line_char_count) {
            // Map source column to display column for background overlay
            let display_col = concealed
                .col_mapping
                .iter()
                .position(|&c| u32::from(c) >= source_col)
                .unwrap_or(concealed.text.len());
            let col_u16 = display_col as u16;
            if col_u16 < width {
                // Read existing cell, merge background
                if let Some(ch) = concealed.text.chars().nth(display_col) {
                    let mut existing = tokens
                        .iter()
                        .find(|t| {
                            matches!(
                                classify_with_extensions(extensions, &t.category),
                                RenderBehavior::Highlight
                            ) && source_col >= t.start_col
                                && source_col < t.end_col
                        })
                        .map_or_else(
                            || default_style.clone(),
                            |t| apply_opacity(&theme.get_style(&t.category), opacity, DEFAULT_BG),
                        );
                    if let Some(bg_color) = bg.bg {
                        existing.bg = Some(bg_color);
                    }
                    backend.set_cell(x + col_u16, y, ch, &existing);
                }
            }
        }
    }
}

/// Render remote clients' visual selections.
///
/// Overlays dimmed background colors on selected ranges. Rendered before
/// cursors so cursors appear on top.
#[allow(clippy::cast_possible_truncation)]
fn render_remote_selections<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    gutter_width: u16,
    content_height: u16,
    virtual_lines: &[&VirtualLine],
    extensions: &[Box<dyn ClientModule>],
) {
    let (width, _) = backend.size();

    // TODO(#494): Multi-window — iterate all windows with tiling layout
    let current_buffer_id = state.windows.first().and_then(|w| w.buffer_id);
    let lines = current_buffer_id.and_then(|id| state.buffer_cache.get(&id));
    let buffer_id = current_buffer_id.unwrap_or(0);

    for remote in state.other_clients.values() {
        if remote.buffer_id != current_buffer_id {
            continue;
        }
        let Some(sel) = &remote.selection else {
            continue;
        };

        let sel_color = dimmed_client_color(remote.client_id);
        let scroll_top = state.get_focused_scroll_top() as u64;
        render_selection_range(
            backend,
            sel,
            sel_color,
            gutter_width,
            content_height,
            width,
            lines.map(Vec::as_slice),
            scroll_top,
            virtual_lines,
            extensions,
            buffer_id,
        );
    }
}

/// Render local client's visual selection.
///
/// Overlays a subtle background color on the local selection range.
#[allow(clippy::cast_possible_truncation)]
fn render_local_selection<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    gutter_width: u16,
    content_height: u16,
    virtual_lines: &[&VirtualLine],
    extensions: &[Box<dyn ClientModule>],
) {
    let (width, _) = backend.size();
    let Some(sel) = state.window_selections.get(&state.focused_window_id) else {
        return;
    };

    // TODO(#494): Multi-window — iterate all windows with tiling layout
    let current_buffer_id = state.windows.first().and_then(|w| w.buffer_id);
    let lines = current_buffer_id.and_then(|id| state.buffer_cache.get(&id));
    let buffer_id = current_buffer_id.unwrap_or(0);

    let scroll_top = state.get_focused_scroll_top() as u64;
    render_selection_range(
        backend,
        sel,
        LOCAL_SELECTION_BG,
        gutter_width,
        content_height,
        width,
        lines.map(Vec::as_slice),
        scroll_top,
        virtual_lines,
        extensions,
        buffer_id,
    );
}

/// Render a selection range with a background color overlay.
///
/// Handles three visual modes:
/// - **char**: Contiguous character range (first/last line partial, middle lines full)
/// - **line**: Entire lines highlighted
/// - **block**: Rectangular column range on each line
#[allow(clippy::cast_possible_truncation, clippy::too_many_arguments)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn render_selection_range<B: RenderBackend>(
    backend: &mut B,
    sel: &SelectionState,
    color: Color,
    gutter_width: u16,
    content_height: u16,
    screen_width: u16,
    lines: Option<&[String]>,
    scroll_top: u64,
    virtual_lines: &[&VirtualLine],
    extensions: &[Box<dyn ClientModule>],
    buffer_id: u64,
) {
    let (start_line, start_col, end_line, end_col) = normalize_selection(sel);
    let content_width = screen_width.saturating_sub(gutter_width);

    for line in start_line..=end_line {
        if line < scroll_top {
            continue;
        }
        let screen_line = buffer_to_screen_row_vl(line, scroll_top, virtual_lines);
        if screen_line as u16 >= content_height {
            break;
        }

        let line_idx = line as usize;

        // Map a buffer column through extensions (table column mapping)
        let map_col = |buf_col: u64| -> u16 {
            #[allow(clippy::cast_possible_truncation)]
            render_engine_bridge::map_cursor_column(extensions, buffer_id, line_idx, buf_col as usize)
                .unwrap_or(buf_col as u16)
        };

        // Visual line length: transformed text length or buffer text length
        let line_text = lines.and_then(|l| l.get(line_idx));
        let visual_line_len = render_engine_bridge::visual_line_len(extensions, buffer_id, line_idx, line_text)
            .unwrap_or(content_width);

        let (col_start, col_end) = match sel.mode.as_str() {
            "line" => (0u16, content_width),
            "block" => (map_col(start_col), (map_col(end_col) + 1).min(visual_line_len)),
            _ => {
                // Char mode — clamp to visual line length
                if start_line == end_line {
                    (map_col(start_col), (map_col(end_col) + 1).min(visual_line_len))
                } else if line == start_line {
                    (map_col(start_col), visual_line_len)
                } else if line == end_line {
                    (0, (map_col(end_col) + 1).min(visual_line_len))
                } else {
                    (0, visual_line_len)
                }
            }
        };

        #[allow(clippy::cast_possible_truncation)]
        let screen_y = screen_line as u16;
        for col in col_start..col_end.min(content_width) {
            backend.overlay_bg(gutter_width + col, screen_y, color);
        }
    }
}

/// Normalize selection so start <= end.
fn normalize_selection(sel: &SelectionState) -> (u64, u64, u64, u64) {
    if (sel.start.line, sel.start.column) <= (sel.end.line, sel.end.column) {
        (sel.start.line, sel.start.column, sel.end.line, sel.end.column)
    } else {
        (sel.end.line, sel.end.column, sel.start.line, sel.start.column)
    }
}

/// Render remote client cursors.
#[allow(clippy::cast_possible_truncation)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn render_remote_cursors<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    gutter_width: u16,
    content_height: u16,
    virtual_lines: &[&VirtualLine],
) {
    let (width, _) = backend.size();

    // Get current buffer ID (simplified)
    let current_buffer_id = state.windows.first().and_then(|w| w.buffer_id);

    for remote in state.other_clients.values() {
        // Only render if in same buffer
        if remote.buffer_id != current_buffer_id {
            continue;
        }

        let scroll_top = state.get_focused_scroll_top() as u64;
        if remote.cursor_line < scroll_top {
            continue;
        }

        let cursor_color = client_color(remote.client_id);
        let cursor_style = Style::default().bg(cursor_color).fg(Color::White);

        let screen_line = buffer_to_screen_row_vl(remote.cursor_line, scroll_top, virtual_lines);
        let screen_col = remote.cursor_col as u16 + gutter_width;

        if screen_line < u64::from(content_height) && screen_col < width {
            #[allow(clippy::cast_possible_truncation)]
            let screen_y = screen_line as u16;
            backend.apply_style(screen_col, screen_y, &cursor_style);
        }
    }
}

/// Maximum display width for cursor label names.
const MAX_LABEL_WIDTH: usize = 16;

/// Prepare label text from a display name and mode.
///
/// Returns a padded, truncated label string with mode indicator.
/// Empty names show `" ? "`, mode is abbreviated (e.g., `[N]`, `[I]`).
fn label_text(display_name: &str, mode: &str) -> String {
    let name = if display_name.is_empty() {
        "?"
    } else {
        display_name
    };
    let mode_abbrev = mode_abbreviation(mode);
    let name = truncate_end(name, MAX_LABEL_WIDTH);
    format!(" {name} {mode_abbrev} ")
}

/// Abbreviate a mode name for compact display in cursor labels.
fn mode_abbreviation(mode: &str) -> &'static str {
    let lower = mode.to_lowercase();
    if lower.contains("insert") {
        "[I]"
    } else if lower.contains("visual") {
        "[V]"
    } else if lower.contains("command") || lower.contains("cmdline") {
        "[C]"
    } else if lower.contains("replace") {
        "[R]"
    } else {
        "[N]"
    }
}

/// Render name labels for remote cursors after line content.
///
/// Shows a colored, underlined tag with the client's display name and mode
/// on the same line as the remote cursor, positioned after the end of the
/// line text. Skipped if the label doesn't fit within the terminal width.
#[allow(clippy::cast_possible_truncation)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn render_remote_cursor_labels<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    gutter_width: u16,
    content_height: u16,
    virtual_lines: &[&VirtualLine],
) {
    let (width, _) = backend.size();

    let current_buffer_id = state.windows.first().and_then(|w| w.buffer_id);

    let lines = current_buffer_id.and_then(|id| state.buffer_cache.get(&id));

    let scroll_top = state.get_focused_scroll_top() as u64;

    for remote in state.other_clients.values() {
        if remote.buffer_id != current_buffer_id {
            continue;
        }

        if remote.cursor_line < scroll_top {
            continue;
        }
        let screen_line = buffer_to_screen_row_vl(remote.cursor_line, scroll_top, virtual_lines);
        if screen_line >= u64::from(content_height) {
            continue;
        }
        #[allow(clippy::cast_possible_truncation)]
        let screen_y = screen_line as u16;

        // Calculate end-of-line position from buffer cache (absolute line index)
        let eol_col = lines
            .and_then(|l| l.get(remote.cursor_line as usize))
            .map_or(0, |line| display_width(line) as u16);

        // Place label after line content with 1-col gap
        let label_x = gutter_width + eol_col + 1;

        let label = label_text(&remote.display_name, &remote.mode);
        let label_width = display_width(&label) as u16;

        // Skip if label doesn't fit on screen
        if label_x + label_width > width {
            continue;
        }

        let label_color = client_color(remote.client_id);
        let label_style = Style::default()
            .fg(label_color)
            .underline()
            .underline_color(label_color);

        backend.write_str(label_x, screen_y, &label, &label_style);
    }
}

/// Render self cursor in the backend (for headless mode).
///
/// In normal mode, the cursor column is remapped through the conceal column
/// mapping so it visually lands on the correct display position. In insert
/// mode, conceals are bypassed on the cursor line so no remapping is needed.
#[allow(clippy::cast_possible_truncation, clippy::too_many_arguments)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn render_self_cursor<B: RenderBackend>(
    backend: &mut B,
    state: &TuiCoreState,
    gutter_width: u16,
    content_height: u16,
    token_cache: &AnnotationCacheManager,
    theme: &ThemeManager,
    virtual_lines: &[&VirtualLine],
    extensions: &[Box<dyn ClientModule>],
) {
    // Skip rendering if no cursor data yet (e.g., before first CursorMoved notification)
    let Some(cursor) = state.get_focused_cursor() else {
        return;
    };

    let scroll_top = state.get_focused_scroll_top() as u64;
    if cursor.line < scroll_top {
        return;
    }
    let (width, _) = backend.size();

    // Account for virtual lines from extensions
    let screen_line = buffer_to_screen_row_vl(cursor.line, scroll_top, virtual_lines);

    // Check if any extension wants to map the cursor column
    let buffer_id_val = state.windows.first().and_then(|w| w.buffer_id).unwrap_or(0);
    #[allow(clippy::cast_possible_truncation)]
    let visual_col = render_engine_bridge::map_cursor_column(
        extensions,
        buffer_id_val,
        cursor.line as usize,
        cursor.column as usize,
    )
        .unwrap_or_else(|| {
            if state.is_insert_mode() {
                cursor.column as u16
            } else {
                compute_cursor_visual_col(
                    state,
                    cursor.line as usize,
                    cursor.column as usize,
                    token_cache,
                    theme,
                    extensions,
                )
            }
        });
    let screen_col = visual_col + gutter_width;

    if screen_line < u64::from(content_height) && screen_col < width {
        // Use inverse video for self cursor
        let cursor_style = Style::default().bg(Color::White).fg(Color::Black);
        #[allow(clippy::cast_possible_truncation)]
        let screen_y = screen_line as u16;
        backend.apply_style(screen_col, screen_y, &cursor_style);
    }
}

/// Compute the visual (display) column for a cursor on a concealed line.
///
/// Builds the conceal list for the given line and remaps the source column
/// through `source_to_display_col`.
#[allow(clippy::cast_possible_truncation)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn compute_cursor_visual_col(
    state: &TuiCoreState,
    line_idx: usize,
    source_col: usize,
    token_cache: &AnnotationCacheManager,
    theme: &ThemeManager,
    extensions: &[Box<dyn ClientModule>],
) -> u16 {
    let buffer_id = state.windows.first().and_then(|w| w.buffer_id);
    let Some(bid) = buffer_id else {
        return source_col as u16;
    };

    // Get line content from buffer cache
    let line_content = state
        .buffer_cache
        .get(&bid)
        .and_then(|lines| lines.get(line_idx));
    let Some(line) = line_content else {
        return source_col as u16;
    };

    // Build conceal decorations for this line
    let tokens = token_cache.tokens_for_line(bid, line_idx as u32);
    let line_u32 = line_idx as u32;
    let mut conceal_decorations: Vec<Decoration> = Vec::new();
    for t in &tokens {
        match classify_with_extensions(extensions, &t.category) {
            RenderBehavior::Conceal { replacement } => {
                conceal_decorations.push(Decoration::Conceal {
                    span: Span::line(line_u32, t.start_col, t.end_col),
                    replacement: replacement.into_owned(),
                    style: Some(theme.get_style(&t.category)),
                });
            }
            RenderBehavior::Hide => {
                conceal_decorations.push(Decoration::Conceal {
                    span: Span::line(line_u32, t.start_col, t.end_col),
                    replacement: String::new(),
                    style: None,
                });
            }
            RenderBehavior::FullWidthLine { ch } => {
                conceal_decorations.push(Decoration::Conceal {
                    span: Span::line(line_u32, t.start_col, t.end_col),
                    replacement: ch.to_string().repeat(500),
                    style: Some(theme.get_style(&t.category)),
                });
            }
            _ => {}
        }
    }

    if conceal_decorations.is_empty() {
        return source_col as u16;
    }

    let conceal_refs: Vec<&Decoration> = conceal_decorations.iter().collect();
    let concealed = apply_conceals(line, line_u32, &conceal_refs);
    source_to_display_col(&concealed, source_col) as u16
}



#[cfg(test)]
#[path = "render_engine_tests.rs"]
mod tests;
