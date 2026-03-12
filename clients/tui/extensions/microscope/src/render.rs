//! Rendering logic for the microscope picker.

use {
    reovim_arch::Color,
    reovim_driver_display::{Style, render_backend::RenderBackend},
};

use crate::{MicroscopeData, layout::LayoutBounds};

/// Render the microscope overlay.
pub fn render_microscope(
    backend: &mut dyn RenderBackend,
    data: &MicroscopeData,
    bounds: &LayoutBounds,
) {
    clear_area(backend, bounds);
    render_query_row(backend, data, bounds);
    render_separator(backend, bounds, bounds.query_row + 1);
    render_results(backend, data, bounds);

    if bounds.show_preview {
        render_preview_separator(backend, bounds);
        render_preview(backend, data, bounds);
    }
}

/// Clear the picker area with the background color.
fn clear_area(backend: &mut dyn RenderBackend, bounds: &LayoutBounds) {
    let style = Style::new().bg(Color::AnsiValue(235));
    backend.fill_region(bounds.x, bounds.y, bounds.width, bounds.total_height, ' ', &style);
}

/// Render the query input row: `{prompt}{query}  [{matched}/{total}]`
#[allow(clippy::cast_possible_truncation)]
fn render_query_row(backend: &mut dyn RenderBackend, data: &MicroscopeData, bounds: &LayoutBounds) {
    let row = bounds.query_row;
    let prompt_style = Style::new()
        .fg(Color::AnsiValue(75))
        .bg(Color::AnsiValue(235));
    let query_style = Style::new().fg(Color::White).bg(Color::AnsiValue(235));
    let count_style = Style::new()
        .fg(Color::AnsiValue(245))
        .bg(Color::AnsiValue(235));

    // Render prompt.
    let mut col = bounds.x;
    for ch in data.prompt.chars() {
        if col < bounds.x + bounds.width {
            backend.set_cell(col, row, ch, &prompt_style);
            col += 1;
        }
    }

    // Render query text.
    for ch in data.query.chars() {
        if col < bounds.x + bounds.width {
            backend.set_cell(col, row, ch, &query_style);
            col += 1;
        }
    }

    // Render count indicator on the right: [matched/total]
    let count_text = format!("[{}/{}]", data.matched_count, data.total_count);
    let count_start = (bounds.x + bounds.width).saturating_sub(count_text.len() as u16);
    let mut col = count_start;
    for ch in count_text.chars() {
        if col < bounds.x + bounds.width {
            backend.set_cell(col, row, ch, &count_style);
            col += 1;
        }
    }
}

/// Render a horizontal separator line.
fn render_separator(backend: &mut dyn RenderBackend, bounds: &LayoutBounds, row: u16) {
    let style = Style::new()
        .fg(Color::AnsiValue(240))
        .bg(Color::AnsiValue(235));
    backend.fill_horizontal(bounds.x, row, bounds.width, '─', &style);
}

/// Render the vertical separator between results and preview.
fn render_preview_separator(backend: &mut dyn RenderBackend, bounds: &LayoutBounds) {
    let style = Style::new()
        .fg(Color::AnsiValue(240))
        .bg(Color::AnsiValue(235));
    backend.fill_vertical(
        bounds.results_width,
        bounds.panel_start_y,
        bounds.panel_height,
        '│',
        &style,
    );
}

/// Render the results list.
#[allow(clippy::cast_possible_truncation)]
fn render_results(backend: &mut dyn RenderBackend, data: &MicroscopeData, bounds: &LayoutBounds) {
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
        backend.set_cell(col, row, indicator, style);
        col += 1;
        backend.set_cell(col, row, ' ', style);
        col += 1;

        // Display text.
        for ch in item.display.chars() {
            if col >= bounds.results_width {
                break;
            }
            backend.set_cell(col, row, ch, style);
            col += 1;
        }

        // Detail text (if room).
        if let Some(ref detail) = item.detail
            && col + 2 < bounds.results_width
        {
            backend.set_cell(col, row, ' ', d_style);
            col += 1;
            for ch in detail.chars() {
                if col >= bounds.results_width {
                    break;
                }
                backend.set_cell(col, row, ch, d_style);
                col += 1;
            }
        }
    }
}

/// Render the preview panel.
#[allow(clippy::cast_possible_truncation)]
fn render_preview(backend: &mut dyn RenderBackend, data: &MicroscopeData, bounds: &LayoutBounds) {
    let Some(ref preview) = data.preview else {
        return;
    };

    let normal_style = Style::new()
        .fg(Color::AnsiValue(250))
        .bg(Color::AnsiValue(235));
    let highlight_style = Style::new().fg(Color::White).bg(Color::AnsiValue(237));
    let line_num_style = Style::new()
        .fg(Color::AnsiValue(240))
        .bg(Color::AnsiValue(235));

    for (i, line) in preview.lines.iter().enumerate() {
        if i as u16 >= bounds.panel_height {
            break;
        }

        let row = bounds.panel_start_y + i as u16;
        let is_highlight = preview.highlight_line == Some(i);
        let style = if is_highlight {
            &highlight_style
        } else {
            &normal_style
        };

        // Line number.
        let line_num = format!("{:>3} ", i + 1);
        let mut col = bounds.preview_x;
        for ch in line_num.chars() {
            if col >= bounds.preview_x + bounds.preview_width {
                break;
            }
            backend.set_cell(col, row, ch, &line_num_style);
            col += 1;
        }

        // Line content.
        for ch in line.chars() {
            if col >= bounds.preview_x + bounds.preview_width {
                break;
            }
            backend.set_cell(col, row, ch, style);
            col += 1;
        }
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
