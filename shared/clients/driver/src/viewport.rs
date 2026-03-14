//! Default viewport renderer implementation.
//!
//! Handles buffer content rendering including syntax highlighting,
//! text concealment, fold handling, virtual lines, and line numbers.

use crate::{
    BufferId, ClientModule, ConcealDecoration, CursorInfo, PlatformCapabilities, Rect,
    RenderBehavior, RenderSurface, SelectionInfo, SelectionMode, Style, ThemeProvider,
    TokenProvider, TransformedLine, ViewportContext, VirtualLinePosition,
    conceal::{apply_conceals, dim_style, source_to_display_col},
};

/// Default background color for opacity blending.
const DEFAULT_BG: reovim_arch::Color = reovim_arch::Color::Black;

// =============================================================================
// DefaultViewportRenderer
// =============================================================================

/// Default viewport renderer.
///
/// Implements the standard buffer rendering pipeline:
/// syntax highlighting, conceals, folds, virtual lines, and line numbers.
pub struct DefaultViewportRenderer;

impl crate::ViewportRenderer for DefaultViewportRenderer {
    fn gutter_width(
        &self,
        modules: &[Box<dyn ClientModule>],
        caps: &dyn PlatformCapabilities,
    ) -> u16 {
        let ctx = crate::AnnotationContext {
            buffer_id: BufferId(0),
            total_lines: 0,
            visible_range: (0, 0),
            cursor_line: 0,
            gutter_style: Style::default(),
        };
        modules
            .iter()
            .filter(|m| m.has_annotations())
            .map(|m| match m.annotation_column_width(&ctx, caps) {
                crate::ColumnWidth::Fixed(w) => w,
                crate::ColumnWidth::Dynamic(min) => min,
            })
            .sum()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_viewport(
        &self,
        surface: &mut dyn RenderSurface,
        viewport: Rect,
        ctx: &ViewportContext<'_>,
        modules: &[Box<dyn ClientModule>],
        tokens: &dyn TokenProvider,
        theme: &dyn ThemeProvider,
        _caps: &dyn PlatformCapabilities,
    ) {
        let content_x = viewport.x + ctx.sidebar_width + ctx.gutter_width;
        let content_height = viewport.height;

        render_buffer_content(surface, viewport, ctx, modules, tokens, theme);

        // Selections (behind cursors — background overlay)
        render_selections(surface, ctx, content_x, content_height, modules);

        // Cursors (on top of selections)
        render_remote_cursors(surface, ctx, content_x, content_height);

        if ctx.render_self_cursor {
            render_self_cursor(surface, ctx, content_x, content_height, tokens, theme, modules);
        }
    }
}

// =============================================================================
// Buffer content rendering
// =============================================================================

/// Render buffer content into the viewport.
#[allow(clippy::too_many_arguments, clippy::cast_possible_truncation)]
fn render_buffer_content(
    surface: &mut dyn RenderSurface,
    viewport: Rect,
    ctx: &ViewportContext<'_>,
    modules: &[Box<dyn ClientModule>],
    tokens: &dyn TokenProvider,
    theme: &dyn ThemeProvider,
) {
    let content_x = viewport.x + ctx.sidebar_width + ctx.gutter_width;
    let content_width = viewport
        .width
        .saturating_sub(ctx.sidebar_width + ctx.gutter_width);
    let content_height = viewport.height;
    let cursor_line = ctx.cursor.map(|c| c.line as usize);

    let mut screen_row: u16 = 0;
    let mut line_idx = ctx.scroll_top;

    while screen_row < content_height {
        if is_line_folded(line_idx, ctx.fold_ranges) {
            line_idx += 1;
            continue;
        }

        // Virtual lines before this line
        screen_row += render_positioned_virtual_lines(
            surface,
            ctx,
            content_x,
            viewport.y,
            screen_row,
            content_width,
            content_height,
            line_idx,
            VirtualLinePosition::Before,
        );
        if screen_row >= content_height {
            break;
        }

        // Gutter annotations (line numbers, git signs, etc.)
        if ctx.gutter_width > 0 {
            render_gutter_annotations(
                surface,
                viewport.x + ctx.sidebar_width,
                viewport.y + screen_row,
                ctx.gutter_width,
                line_idx,
                modules,
                ctx,
            );
        }

        // Line content
        if let Some(lines) = ctx.buffer_lines {
            if line_idx < lines.len() {
                let line = &lines[line_idx];
                let transform = transform_line(modules, ctx.buffer_id, line_idx, line);

                if let Some(ref transformed) = transform {
                    render_transformed_line(
                        surface,
                        content_x,
                        viewport.y + screen_row,
                        content_width,
                        transformed,
                    );
                } else {
                    render_line_content(
                        surface,
                        content_x,
                        viewport.y + screen_row,
                        content_width,
                        line,
                        ctx.opacity,
                        ctx.buffer_id,
                        line_idx,
                        tokens,
                        theme,
                        ctx.is_insert_mode && cursor_line == Some(line_idx),
                        modules,
                    );
                }
            } else {
                // Empty line indicator (tilde)
                let tilde_style =
                    apply_opacity(&Style::new().fg(reovim_arch::Color::DarkGrey), ctx.opacity);
                let mut buf = [0u8; 4];
                let s = '~'.encode_utf8(&mut buf);
                surface.write_styled(content_x, viewport.y + screen_row, s, tilde_style);
            }
        }

        screen_row += 1;

        // Virtual lines after this line
        screen_row += render_positioned_virtual_lines(
            surface,
            ctx,
            content_x,
            viewport.y,
            screen_row,
            content_width,
            content_height,
            line_idx,
            VirtualLinePosition::After,
        );

        line_idx += 1;
    }
}

/// Render virtual lines for a given buffer line in the specified position.
///
/// Returns the number of screen rows consumed.
#[allow(clippy::too_many_arguments, clippy::cast_possible_truncation)]
fn render_positioned_virtual_lines(
    surface: &mut dyn RenderSurface,
    ctx: &ViewportContext<'_>,
    content_x: u16,
    viewport_y: u16,
    screen_row: u16,
    content_width: u16,
    content_height: u16,
    line_idx: usize,
    position: VirtualLinePosition,
) -> u16 {
    let mut rows_used: u16 = 0;
    for vl in ctx
        .virtual_lines
        .iter()
        .filter(|vl| vl.buffer_line == line_idx && vl.position == position)
    {
        if screen_row + rows_used >= content_height {
            break;
        }
        render_virtual_line(
            surface,
            content_x,
            viewport_y + screen_row + rows_used,
            content_width,
            &vl.content,
            &vl.style,
        );
        rows_used += 1;
    }
    rows_used
}

// =============================================================================
// Line rendering
// =============================================================================

/// Render a line of buffer content with syntax highlighting and conceals.
#[allow(clippy::too_many_arguments, clippy::cast_possible_truncation)]
fn render_line_content(
    surface: &mut dyn RenderSurface,
    x: u16,
    y: u16,
    width: u16,
    line: &str,
    opacity: f32,
    buffer_id: Option<BufferId>,
    line_idx: usize,
    token_provider: &dyn TokenProvider,
    theme: &dyn ThemeProvider,
    skip_conceals: bool,
    modules: &[Box<dyn ClientModule>],
) {
    let tokens = buffer_id
        .map(|bid| token_provider.tokens_for_line(bid, line_idx as u32))
        .unwrap_or_default();

    let default_style = apply_opacity(&Style::default(), opacity);

    // Build conceals and backgrounds from token classification
    let mut conceals: Vec<ConcealDecoration> = Vec::new();
    let mut backgrounds: Vec<(u32, u32, Style)> = Vec::new();

    for t in &tokens {
        let behavior = classify_with_modules(modules, &t.category);
        match behavior {
            RenderBehavior::Conceal { replacement } if !skip_conceals => {
                conceals.push(ConcealDecoration {
                    start_col: t.start_col as usize,
                    end_col: t.end_col as usize,
                    replacement: Some(replacement.into_owned()),
                    style: Some(theme.highlight(&t.category)),
                });
            }
            RenderBehavior::Hide if !skip_conceals => {
                conceals.push(ConcealDecoration {
                    start_col: t.start_col as usize,
                    end_col: t.end_col as usize,
                    replacement: None,
                    style: None,
                });
            }
            RenderBehavior::FullWidthLine { ch, .. } if !skip_conceals => {
                conceals.push(ConcealDecoration {
                    start_col: t.start_col as usize,
                    end_col: t.end_col as usize,
                    replacement: Some(ch.to_string().repeat(width as usize)),
                    style: Some(theme.highlight(&t.category)),
                });
            }
            RenderBehavior::Background(_) => {
                backgrounds.push((t.start_col, t.end_col, theme.highlight(&t.category)));
            }
            _ => {} // Highlight is default — style applied via token lookup
        }
    }

    // Apply conceals to get display text
    let concealed = apply_conceals(line, &conceals);

    // Render display text with syntax highlighting
    for (col_offset, (display_col, ch)) in (0_u16..).zip(concealed.text.chars().enumerate()) {
        if col_offset >= width {
            break;
        }

        let style = if let Some(Some(conceal_style)) = concealed.styles.get(display_col) {
            apply_opacity(conceal_style, opacity)
        } else {
            // Map display column back to source column for highlight lookup
            let source_col = concealed.col_mapping.get(display_col).copied().unwrap_or(0);
            let source_col = u32::from(source_col);

            tokens
                .iter()
                .find(|t| {
                    matches!(classify_with_modules(modules, &t.category), RenderBehavior::Highlight)
                        && source_col >= t.start_col
                        && source_col < t.end_col
                })
                .map_or_else(
                    || default_style.clone(),
                    |t| apply_opacity(&theme.highlight(&t.category), opacity),
                )
        };

        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        surface.write_styled(x + col_offset, y, s, style);
    }

    // Overlay background tokens
    let line_char_count = line.chars().count() as u32;
    for (start_col, end_col, bg_style) in &backgrounds {
        let bg = apply_opacity(bg_style, opacity);
        for source_col in *start_col..(*end_col).min(line_char_count) {
            let display_col = source_to_display_col(&concealed, source_col as usize);
            #[allow(clippy::cast_possible_truncation)]
            let col_u16 = display_col as u16;
            if col_u16 < width
                && let Some(bg_color) = bg.bg
            {
                surface.overlay_bg(x + col_u16, y, bg_color);
            }
        }
    }

    // Apply inline decorations from buffer-contrib modules
    for module in modules.iter().filter(|m| m.has_buffer_contrib()) {
        for dec in module.inline_decorations(line_idx) {
            for col in dec.col_start..dec.col_end.min(width) {
                let dec_style = apply_opacity(&dec.style, opacity);
                surface.apply_style(x + col, y, dec_style);
            }
        }
    }
}

/// Render a virtual line.
#[allow(clippy::cast_possible_truncation)]
fn render_virtual_line(
    surface: &mut dyn RenderSurface,
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
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        surface.write_styled(x + col_u16, y, s, style.clone());
    }
}

/// Render a transformed line from a module.
#[allow(clippy::cast_possible_truncation)]
fn render_transformed_line(
    surface: &mut dyn RenderSurface,
    x: u16,
    y: u16,
    width: u16,
    transformed: &TransformedLine,
) {
    let mut col: u16 = 0;
    for (seg_text, seg_style) in &transformed.segments {
        for ch in seg_text.chars() {
            if col >= width {
                return;
            }
            let style = seg_style.clone().unwrap_or_default();
            let mut buf = [0u8; 4];
            let s = ch.encode_utf8(&mut buf);
            surface.write_styled(x + col, y, s, style);
            col += 1;
        }
    }
}

/// Render gutter annotations from all annotation modules.
///
/// Modules are sorted by priority (highest = leftmost). Each module's
/// `GutterCell` is right-aligned within its allocated column width.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::cast_possible_truncation)]
fn render_gutter_annotations(
    surface: &mut dyn RenderSurface,
    x: u16,
    y: u16,
    _total_width: u16,
    line_idx: usize,
    modules: &[Box<dyn ClientModule>],
    ctx: &ViewportContext<'_>,
) {
    let ann_ctx = crate::AnnotationContext {
        buffer_id: ctx.buffer_id.unwrap_or(BufferId(0)),
        total_lines: ctx.buffer_lines.map_or(0, <[String]>::len),
        visible_range: (ctx.scroll_top, ctx.scroll_top + 100),
        cursor_line: ctx.cursor.map_or(0, |c| c.line as usize),
        gutter_style: Style::default(),
    };

    // Collect annotation modules sorted by priority (highest first = leftmost)
    let mut ann_modules: Vec<&Box<dyn ClientModule>> =
        modules.iter().filter(|m| m.has_annotations()).collect();
    ann_modules.sort_by_key(|m| std::cmp::Reverse(m.annotation_priority()));

    let caps = DummyCaps;
    let mut col_x = x;

    for module in &ann_modules {
        let col_width = match module.annotation_column_width(&ann_ctx, &caps) {
            crate::ColumnWidth::Fixed(w) => w,
            crate::ColumnWidth::Dynamic(min) => min,
        };

        if col_width == 0 {
            continue;
        }

        if let Some(cell) = module.annotate(line_idx, &ann_ctx) {
            let style = apply_opacity(&cell.style, ctx.opacity);
            // Right-align the text within the column
            let padding = (col_width as usize).saturating_sub(cell.text.len() + 1);
            let display = format!("{:>width$} ", cell.text, width = padding + cell.text.len());
            surface.write_styled(col_x, y, &display, style);
        }

        col_x += col_width;
    }
}

/// Minimal `PlatformCapabilities` for annotation width queries.
struct DummyCaps;

impl PlatformCapabilities for DummyCaps {
    fn rendering_model(&self) -> crate::RenderingModel {
        crate::RenderingModel::CellGrid
    }
    fn grid_size(&self) -> Option<(u16, u16)> {
        None
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
// Helpers
// =============================================================================

/// Check if a buffer line is hidden by any fold range.
#[must_use]
pub fn is_line_folded(line: usize, fold_ranges: &[(usize, usize)]) -> bool {
    for &(start, count) in fold_ranges {
        if line >= start && line < start + count {
            return true;
        }
    }
    false
}

/// Convert a buffer line to a screen row, accounting for virtual lines.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn buffer_to_screen_row_vl(
    buffer_line: u64,
    scroll_top: u64,
    virtual_lines: &[crate::VirtualLine],
) -> u64 {
    let count = virtual_lines
        .iter()
        .filter(|vl| {
            (vl.buffer_line as u64) >= scroll_top && (vl.buffer_line as u64) <= buffer_line
        })
        .count();
    (buffer_line - scroll_top) + count as u64
}

/// Apply opacity to a style (blend toward `DEFAULT_BG`).
fn apply_opacity(style: &Style, opacity: f32) -> Style {
    if (opacity - 1.0).abs() < f32::EPSILON {
        return style.clone();
    }
    dim_style(style, opacity, DEFAULT_BG)
}

/// Classify a token category via module dispatch.
///
/// Queries all buffer-contrib modules; first `Some` wins.
/// Falls back to `Highlight` if no module claims the category.
fn classify_with_modules(modules: &[Box<dyn ClientModule>], category: &str) -> RenderBehavior {
    modules
        .iter()
        .filter(|m| m.has_buffer_contrib())
        .find_map(|m| m.classify_token(category))
        .unwrap_or(RenderBehavior::Highlight)
}

/// Get a transformed line from modules.
fn transform_line(
    modules: &[Box<dyn ClientModule>],
    buffer_id: Option<BufferId>,
    line_idx: usize,
    text: &str,
) -> Option<TransformedLine> {
    let bid = buffer_id?;
    modules
        .iter()
        .filter(|m| m.has_buffer_contrib())
        .find_map(|m| m.transform_line(bid, line_idx, text))
}

// =============================================================================
// Color palettes
// =============================================================================

/// CBF-8 colorblind-friendly palette for remote cursors.
///
/// These 8 colors are distinguishable by people with common color vision
/// deficiencies (protanopia, deuteranopia, tritanopia).
pub const CBF8_PALETTE: [reovim_arch::Color; 8] = [
    reovim_arch::Color::Rgb {
        r: 0,
        g: 114,
        b: 178,
    }, // Blue
    reovim_arch::Color::Rgb {
        r: 230,
        g: 159,
        b: 0,
    }, // Orange
    reovim_arch::Color::Rgb {
        r: 86,
        g: 180,
        b: 233,
    }, // Sky blue
    reovim_arch::Color::Rgb {
        r: 0,
        g: 158,
        b: 115,
    }, // Green
    reovim_arch::Color::Rgb {
        r: 240,
        g: 228,
        b: 66,
    }, // Yellow
    reovim_arch::Color::Rgb {
        r: 213,
        g: 94,
        b: 0,
    }, // Vermilion
    reovim_arch::Color::Rgb {
        r: 204,
        g: 121,
        b: 167,
    }, // Pink
    reovim_arch::Color::Rgb { r: 0, g: 0, b: 0 }, // Black (fallback)
];

/// Get a color from the CBF-8 palette for a client ID.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn client_color(client_id: u64) -> reovim_arch::Color {
    CBF8_PALETTE[(client_id as usize) % CBF8_PALETTE.len()]
}

/// Dimmed CBF-8 palette for selection backgrounds.
///
/// Lower-intensity versions of the cursor palette, suitable for
/// background overlays that don't obscure text.
pub const CBF8_DIMMED: [reovim_arch::Color; 8] = [
    reovim_arch::Color::Rgb { r: 0, g: 45, b: 70 }, // Blue
    reovim_arch::Color::Rgb { r: 75, g: 50, b: 0 }, // Orange
    reovim_arch::Color::Rgb {
        r: 25,
        g: 60,
        b: 75,
    }, // Sky blue
    reovim_arch::Color::Rgb { r: 0, g: 55, b: 35 }, // Green
    reovim_arch::Color::Rgb {
        r: 70,
        g: 65,
        b: 20,
    }, // Yellow
    reovim_arch::Color::Rgb { r: 70, g: 30, b: 0 }, // Vermilion
    reovim_arch::Color::Rgb {
        r: 65,
        g: 38,
        b: 55,
    }, // Pink
    reovim_arch::Color::Rgb {
        r: 30,
        g: 30,
        b: 30,
    }, // Dark grey (fallback)
];

/// Get a dimmed color for selection backgrounds.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub const fn dimmed_client_color(client_id: u64) -> reovim_arch::Color {
    CBF8_DIMMED[(client_id as usize) % CBF8_DIMMED.len()]
}

/// Local selection background color (subtle blue).
pub const LOCAL_SELECTION_BG: reovim_arch::Color = reovim_arch::Color::Rgb {
    r: 50,
    g: 50,
    b: 100,
};

/// Maximum display width for cursor label names.
const MAX_LABEL_WIDTH: usize = 16;

// =============================================================================
// Selection rendering
// =============================================================================

/// Render all selections (local + remote) as background overlays.
#[allow(clippy::cast_possible_truncation)]
fn render_selections(
    surface: &mut dyn RenderSurface,
    ctx: &ViewportContext<'_>,
    content_x: u16,
    content_height: u16,
    modules: &[Box<dyn ClientModule>],
) {
    let (width, _) = surface.size();
    let content_width = width.saturating_sub(content_x);
    let scroll_top = ctx.scroll_top as u64;

    // Remote selections
    for remote in ctx.remote_clients {
        if let Some(sel) = &remote.selection {
            render_selection_range(
                surface,
                sel,
                content_x,
                content_height,
                content_width,
                ctx.buffer_lines,
                scroll_top,
                ctx.virtual_lines,
                modules,
                ctx.buffer_id,
            );
        }
    }

    // Local selection
    if let Some(sel) = &ctx.local_selection {
        render_selection_range(
            surface,
            sel,
            content_x,
            content_height,
            content_width,
            ctx.buffer_lines,
            scroll_top,
            ctx.virtual_lines,
            modules,
            ctx.buffer_id,
        );
    }
}

/// Render a selection range with a background color overlay.
#[allow(clippy::too_many_arguments, clippy::cast_possible_truncation)]
fn render_selection_range(
    surface: &mut dyn RenderSurface,
    sel: &SelectionInfo,
    content_x: u16,
    content_height: u16,
    content_width: u16,
    lines: Option<&[String]>,
    scroll_top: u64,
    virtual_lines: &[crate::VirtualLine],
    modules: &[Box<dyn ClientModule>],
    buffer_id: Option<BufferId>,
) {
    let (start_line, start_col, end_line, end_col) = normalize_selection(sel);

    for line in start_line..=end_line {
        if line < scroll_top {
            continue;
        }
        let screen_line = buffer_to_screen_row_vl(line, scroll_top, virtual_lines);
        if screen_line as u16 >= content_height {
            break;
        }

        let line_idx = line as usize;

        // Map columns through extensions (table column mapping)
        let map_col = |buf_col: u64| -> u16 {
            buffer_id
                .and_then(|bid| {
                    modules
                        .iter()
                        .filter(|m| m.has_buffer_contrib())
                        .find_map(|m| m.map_cursor_column(bid, line_idx, buf_col as usize))
                })
                .unwrap_or(buf_col as u16)
        };

        // Visual line length from transform or raw text
        let visual_line_len = buffer_id
            .and_then(|bid| {
                modules
                    .iter()
                    .filter(|m| m.has_buffer_contrib())
                    .find_map(|m| {
                        let text = lines
                            .and_then(|l| l.get(line_idx))
                            .map_or("", String::as_str);
                        m.transform_line(bid, line_idx, text)
                    })
            })
            .map_or_else(
                || {
                    lines
                        .and_then(|l| l.get(line_idx))
                        .map_or(content_width, |s| s.len() as u16)
                },
                |t| {
                    t.segments
                        .iter()
                        .map(|(s, _)| s.chars().count())
                        .sum::<usize>() as u16
                },
            );

        let (col_start, col_end) = match sel.mode {
            SelectionMode::Line => (0u16, content_width),
            SelectionMode::Block => {
                (map_col(start_col), (map_col(end_col) + 1).min(visual_line_len))
            }
            SelectionMode::Char => {
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
            surface.overlay_bg(content_x + col, screen_y, sel.color);
        }
    }
}

/// Normalize selection so start <= end.
fn normalize_selection(sel: &SelectionInfo) -> (u64, u64, u64, u64) {
    if (sel.start_line, sel.start_col) <= (sel.end_line, sel.end_col) {
        (sel.start_line, sel.start_col, sel.end_line, sel.end_col)
    } else {
        (sel.end_line, sel.end_col, sel.start_line, sel.start_col)
    }
}

// =============================================================================
// Cursor rendering
// =============================================================================

/// Render remote client cursors.
#[allow(clippy::cast_possible_truncation)]
fn render_remote_cursors(
    surface: &mut dyn RenderSurface,
    ctx: &ViewportContext<'_>,
    content_x: u16,
    content_height: u16,
) {
    let (width, _) = surface.size();
    let scroll_top = ctx.scroll_top as u64;

    for remote in ctx.remote_clients {
        if remote.cursor_line < scroll_top {
            continue;
        }
        let screen_line =
            buffer_to_screen_row_vl(remote.cursor_line, scroll_top, ctx.virtual_lines);
        let screen_col = remote.cursor_col as u16 + content_x;

        if screen_line < u64::from(content_height) && screen_col < width {
            let cursor_style = Style::new()
                .bg(remote.cursor_color)
                .fg(reovim_arch::Color::White);
            let screen_y = screen_line as u16;
            surface.apply_style(screen_col, screen_y, cursor_style);
        }
    }

    // Render cursor labels after line content
    render_remote_cursor_labels(surface, ctx, content_x, content_height);
}

/// Render name labels for remote cursors.
#[allow(clippy::cast_possible_truncation)]
fn render_remote_cursor_labels(
    surface: &mut dyn RenderSurface,
    ctx: &ViewportContext<'_>,
    content_x: u16,
    content_height: u16,
) {
    let (width, _) = surface.size();
    let scroll_top = ctx.scroll_top as u64;

    for remote in ctx.remote_clients {
        if remote.cursor_line < scroll_top {
            continue;
        }
        let screen_line =
            buffer_to_screen_row_vl(remote.cursor_line, scroll_top, ctx.virtual_lines);
        if screen_line >= u64::from(content_height) {
            continue;
        }
        let screen_y = screen_line as u16;

        // End-of-line position from buffer cache
        let eol_col = ctx
            .buffer_lines
            .and_then(|l| l.get(remote.cursor_line as usize))
            .map_or(0, |line| crate::ui::display_width(line) as u16);

        let label_x = content_x + eol_col + 1;
        let label = label_text(&remote.display_name, &remote.mode);
        let label_width = crate::ui::display_width(&label) as u16;

        if label_x + label_width > width {
            continue;
        }

        let label_style = Style::new().fg(remote.cursor_color).underline();

        surface.write_styled(label_x, screen_y, &label, label_style);
    }
}

/// Render self cursor in the surface (for headless mode).
#[allow(clippy::too_many_arguments, clippy::cast_possible_truncation)]
fn render_self_cursor(
    surface: &mut dyn RenderSurface,
    ctx: &ViewportContext<'_>,
    content_x: u16,
    content_height: u16,
    tokens: &dyn TokenProvider,
    theme: &dyn ThemeProvider,
    modules: &[Box<dyn ClientModule>],
) {
    let Some(cursor) = ctx.cursor else {
        return;
    };

    let scroll_top = ctx.scroll_top as u64;
    if cursor.line < scroll_top {
        return;
    }
    let (width, _) = surface.size();

    let screen_line = buffer_to_screen_row_vl(cursor.line, scroll_top, ctx.virtual_lines);

    // Check extension column mapping first
    let visual_col = ctx
        .buffer_id
        .and_then(|bid| {
            modules
                .iter()
                .filter(|m| m.has_buffer_contrib())
                .find_map(|m| {
                    m.map_cursor_column(bid, cursor.line as usize, cursor.column as usize)
                })
        })
        .unwrap_or_else(|| {
            if ctx.is_insert_mode {
                cursor.column as u16
            } else {
                compute_cursor_visual_col(ctx, &cursor, tokens, theme, modules)
            }
        });
    let screen_col = visual_col + content_x;

    if screen_line < u64::from(content_height) && screen_col < width {
        let cursor_style = Style::new()
            .bg(reovim_arch::Color::White)
            .fg(reovim_arch::Color::Black);
        let screen_y = screen_line as u16;
        surface.apply_style(screen_col, screen_y, cursor_style);
    }
}

/// Compute the visual column for a cursor on a concealed line.
#[allow(clippy::cast_possible_truncation)]
fn compute_cursor_visual_col(
    ctx: &ViewportContext<'_>,
    cursor: &CursorInfo,
    tokens: &dyn TokenProvider,
    theme: &dyn ThemeProvider,
    modules: &[Box<dyn ClientModule>],
) -> u16 {
    let Some(bid) = ctx.buffer_id else {
        return cursor.column as u16;
    };

    let line_content = ctx
        .buffer_lines
        .and_then(|lines| lines.get(cursor.line as usize));
    let Some(line) = line_content else {
        return cursor.column as u16;
    };

    let line_tokens = tokens.tokens_for_line(bid, cursor.line as u32);
    let mut conceals: Vec<ConcealDecoration> = Vec::new();
    for t in &line_tokens {
        match classify_with_modules(modules, &t.category) {
            RenderBehavior::Conceal { replacement } => {
                conceals.push(ConcealDecoration {
                    start_col: t.start_col as usize,
                    end_col: t.end_col as usize,
                    replacement: Some(replacement.into_owned()),
                    style: Some(theme.highlight(&t.category)),
                });
            }
            RenderBehavior::Hide => {
                conceals.push(ConcealDecoration {
                    start_col: t.start_col as usize,
                    end_col: t.end_col as usize,
                    replacement: None,
                    style: None,
                });
            }
            RenderBehavior::FullWidthLine { ch, .. } => {
                conceals.push(ConcealDecoration {
                    start_col: t.start_col as usize,
                    end_col: t.end_col as usize,
                    replacement: Some(ch.to_string().repeat(500)),
                    style: Some(theme.highlight(&t.category)),
                });
            }
            _ => {}
        }
    }

    if conceals.is_empty() {
        return cursor.column as u16;
    }

    let concealed = apply_conceals(line, &conceals);
    source_to_display_col(&concealed, cursor.column as usize) as u16
}

/// Prepare label text from a display name and mode.
fn label_text(display_name: &str, mode: &str) -> String {
    let name = if display_name.is_empty() {
        "?"
    } else {
        display_name
    };
    let mode_abbrev = mode_abbreviation(mode);
    let name = crate::ui::truncate_end(name, MAX_LABEL_WIDTH);
    format!(" {name} {mode_abbrev} ")
}

/// Abbreviate a mode name for compact display.
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

#[cfg(test)]
#[path = "viewport_tests.rs"]
mod tests;
