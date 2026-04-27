//! Rendering logic for the microscope picker.

use reovim_client_driver::{ChromeSurface, Rect, Style, types::Color};

use crate::{MicroscopeData, PreviewHighlightData, layout::LayoutBounds};

/// Render the microscope overlay.
pub fn render_microscope(
    surface: &mut dyn ChromeSurface,
    data: &MicroscopeData,
    bounds: &LayoutBounds,
) {
    clear_area(surface, bounds);
    render_query_row(surface, data, bounds);
    render_separator(surface, bounds, bounds.query_row + 1);
    render_results(surface, data, bounds);

    if bounds.show_preview {
        render_preview_separator(surface, bounds);
        render_preview(surface, data, bounds);
    }
}

/// Clear the picker area with the background color.
fn clear_area(surface: &mut dyn ChromeSurface, bounds: &LayoutBounds) {
    let style = Style::new().bg(Color::AnsiValue(235));
    surface.fill(Rect::new(bounds.x, bounds.y, bounds.width, bounds.total_height), ' ', style);
}

/// Render the query input row: `{prompt}{query}  [{matched}/{total}]`
#[allow(clippy::cast_possible_truncation)]
fn render_query_row(surface: &mut dyn ChromeSurface, data: &MicroscopeData, bounds: &LayoutBounds) {
    let row = bounds.query_row;
    let prompt_style = Style::new()
        .fg(Color::AnsiValue(75))
        .bg(Color::AnsiValue(235));
    let query_style = Style::new().fg(Color::White).bg(Color::AnsiValue(235));
    let count_style = Style::new()
        .fg(Color::AnsiValue(245))
        .bg(Color::AnsiValue(235));

    // Render prompt character by character.
    let mut col = bounds.x;
    for ch in data.prompt.chars() {
        if col < bounds.x + bounds.width {
            surface.write_styled(col, row, &ch.to_string(), prompt_style.clone());
            col += 1;
        }
    }

    // Render query text character by character.
    for ch in data.query.chars() {
        if col < bounds.x + bounds.width {
            surface.write_styled(col, row, &ch.to_string(), query_style.clone());
            col += 1;
        }
    }

    // Render count indicator on the right: [matched/total]
    let count_text = format!("[{}/{}]", data.matched_count, data.total_count);
    let count_start = (bounds.x + bounds.width).saturating_sub(count_text.len() as u16);
    let mut col = count_start;
    for ch in count_text.chars() {
        if col < bounds.x + bounds.width {
            surface.write_styled(col, row, &ch.to_string(), count_style.clone());
            col += 1;
        }
    }
}

/// Render a horizontal separator line.
fn render_separator(surface: &mut dyn ChromeSurface, bounds: &LayoutBounds, row: u16) {
    let style = Style::new()
        .fg(Color::AnsiValue(240))
        .bg(Color::AnsiValue(235));
    surface.fill(
        Rect::new(bounds.x, row, bounds.width, 1),
        '\u{2500}', // '─'
        style,
    );
}

/// Render the vertical separator between results and preview.
fn render_preview_separator(surface: &mut dyn ChromeSurface, bounds: &LayoutBounds) {
    let style = Style::new()
        .fg(Color::AnsiValue(240))
        .bg(Color::AnsiValue(235));
    surface.fill(
        Rect::new(bounds.results_width, bounds.panel_start_y, 1, bounds.panel_height),
        '\u{2502}', // '│'
        style,
    );
}

/// Render the results list.
#[allow(clippy::cast_possible_truncation)]
pub fn render_results(
    surface: &mut dyn ChromeSurface,
    data: &MicroscopeData,
    bounds: &LayoutBounds,
) {
    let normal_style = Style::new().fg(Color::White).bg(Color::AnsiValue(235));
    let selected_style = Style::new().fg(Color::White).bg(Color::AnsiValue(238));
    let detail_style = Style::new()
        .fg(Color::AnsiValue(245))
        .bg(Color::AnsiValue(235));
    let selected_detail = Style::new()
        .fg(Color::AnsiValue(245))
        .bg(Color::AnsiValue(238));

    // Compute scroll offset to keep selection visible within the panel.
    let panel_h = bounds.panel_height as usize;
    let scroll = if panel_h == 0 || data.selected < panel_h {
        0
    } else {
        data.selected - panel_h + 1
    };

    for (vi, item) in data.items.iter().skip(scroll).enumerate() {
        if vi as u16 >= bounds.panel_height {
            break;
        }

        let row = bounds.panel_start_y + vi as u16;
        let is_selected = (scroll + vi) == data.selected;
        let style = if is_selected {
            &selected_style
        } else {
            &normal_style
        };
        let d_style = if is_selected {
            &selected_detail
        } else {
            &detail_style
        };

        // Selection indicator.
        let mut col = bounds.x;
        let indicator = if is_selected { '>' } else { ' ' };
        surface.write_styled(col, row, &indicator.to_string(), style.clone());
        col += 1;
        surface.write_styled(col, row, " ", style.clone());
        col += 1;

        // Icon (if present).
        if let Some(ref icon) = item.icon
            && col + 2 < bounds.results_width
        {
            surface.write_styled(col, row, icon, style.clone());
            col += 1;
            surface.write_styled(col, row, " ", style.clone());
            col += 1;
        }

        // Display text.
        for ch in item.display.chars() {
            if col >= bounds.results_width {
                break;
            }
            surface.write_styled(col, row, &ch.to_string(), style.clone());
            col += 1;
        }

        // Detail text (if room).
        if let Some(ref detail) = item.detail
            && col + 2 < bounds.results_width
        {
            surface.write_styled(col, row, " ", d_style.clone());
            col += 1;
            for ch in detail.chars() {
                if col >= bounds.results_width {
                    break;
                }
                surface.write_styled(col, row, &ch.to_string(), d_style.clone());
                col += 1;
            }
        }
    }
}

/// Map a syntax category to a foreground color.
///
/// Uses a simple prefix-match to determine the color for each category.
fn syntax_category_color(category: &str) -> Option<Color> {
    if category.starts_with("keyword") {
        Some(Color::AnsiValue(141)) // purple
    } else if category.starts_with("string") {
        Some(Color::AnsiValue(114)) // green
    } else if category.starts_with("comment") {
        Some(Color::AnsiValue(245)) // grey
    } else if category.starts_with("type") {
        Some(Color::AnsiValue(221)) // yellow
    } else if category.starts_with("function") {
        Some(Color::AnsiValue(81)) // blue
    } else if category.starts_with("number") || category.starts_with("boolean") {
        Some(Color::AnsiValue(208)) // orange
    } else if category.starts_with("operator") || category.starts_with("punctuation") {
        Some(Color::AnsiValue(250)) // light
    } else if category.starts_with("variable") {
        Some(Color::AnsiValue(253)) // white-ish
    } else if category.starts_with("constant") {
        Some(Color::AnsiValue(208)) // orange
    } else if category.starts_with("attribute") {
        Some(Color::AnsiValue(114)) // green
    } else {
        None
    }
}

/// Find the syntax highlight color for a byte offset within a line.
fn syntax_color_at(
    highlights: &[PreviewHighlightData],
    line_idx: u16,
    byte_col: u16,
) -> Option<Color> {
    for h in highlights {
        if h.line == line_idx && h.col_start <= byte_col && byte_col < h.col_end {
            return syntax_category_color(&h.category);
        }
    }
    None
}

/// Render the preview panel.
#[allow(clippy::cast_possible_truncation)]
fn render_preview(surface: &mut dyn ChromeSurface, data: &MicroscopeData, bounds: &LayoutBounds) {
    let Some(ref preview) = data.preview else {
        return;
    };

    let bg_normal = Color::AnsiValue(235);
    let bg_highlight = Color::AnsiValue(237);
    let fg_normal = Color::AnsiValue(250);
    let line_num_style = Style::new().fg(Color::AnsiValue(240)).bg(bg_normal);

    for (i, line) in preview.lines.iter().enumerate() {
        if i as u16 >= bounds.panel_height {
            break;
        }

        let row = bounds.panel_start_y + i as u16;
        let is_highlight = preview.highlight_line == Some(i);

        // Line number.
        let line_num = format!("{:>3} ", i + 1);
        let mut col = bounds.preview_x;
        for ch in line_num.chars() {
            if col >= bounds.preview_x + bounds.preview_width {
                break;
            }
            surface.write_styled(col, row, &ch.to_string(), line_num_style.clone());
            col += 1;
        }

        // Line content — apply syntax highlighting per character.
        for (byte_idx, ch) in line.char_indices() {
            if col >= bounds.preview_x + bounds.preview_width {
                break;
            }
            let fg = syntax_color_at(&preview.highlights, i as u16, byte_idx as u16)
                .unwrap_or(fg_normal);
            let style = if is_highlight {
                Style::new().fg(fg).bg(bg_highlight)
            } else {
                Style::new().fg(fg).bg(bg_normal)
            };
            surface.write_styled(col, row, &ch.to_string(), style);
            col += 1;
        }
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
