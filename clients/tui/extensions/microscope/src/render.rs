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

    for (i, item) in data.items.iter().enumerate() {
        if i as u16 >= bounds.panel_height {
            break;
        }

        let row = bounds.panel_start_y + i as u16;
        let is_selected = i == data.selected;
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
mod tests {
    use super::*;

    /// Minimal `RenderBackend` for testing.
    struct MockBackend {
        width: u16,
        height: u16,
        cells: Vec<Vec<(char, Style)>>,
    }

    impl MockBackend {
        fn new(width: u16, height: u16) -> Self {
            let default_style = Style::new();
            Self {
                width,
                height,
                cells: vec![vec![(' ', default_style); width as usize]; height as usize],
            }
        }

        fn char_at(&self, x: u16, y: u16) -> char {
            self.cells[y as usize][x as usize].0
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    impl RenderBackend for MockBackend {
        fn size(&self) -> (u16, u16) {
            (self.width, self.height)
        }

        fn set_cell(&mut self, x: u16, y: u16, ch: char, style: &Style) {
            if x < self.width && y < self.height {
                self.cells[y as usize][x as usize] = (ch, style.clone());
            }
        }

        fn apply_style(&mut self, x: u16, y: u16, style: &Style) {
            if x < self.width && y < self.height {
                self.cells[y as usize][x as usize].1 = style.clone();
            }
        }

        fn write_str(&mut self, x: u16, y: u16, text: &str, style: &Style) -> u16 {
            let mut col = x;
            for ch in text.chars() {
                if col >= self.width {
                    break;
                }
                self.set_cell(col, y, ch, style);
                col += 1;
            }
            col - x
        }

        fn clear(&mut self) {
            let default_style = Style::new();
            self.cells =
                vec![vec![(' ', default_style); self.width as usize]; self.height as usize];
        }

        fn overlay_bg(&mut self, x: u16, y: u16, bg: Color) {
            if x < self.width && y < self.height {
                self.cells[y as usize][x as usize].1.bg = Some(bg);
            }
        }
    }

    fn make_data(active: bool) -> MicroscopeData {
        MicroscopeData {
            active,
            query: "test".to_owned(),
            cursor: 4,
            selected: 0,
            scroll_offset: 0,
            picker_title: "Files".to_owned(),
            prompt: "> ".to_owned(),
            items: vec![
                super::super::ItemData {
                    display: "main.rs".to_owned(),
                    detail: Some("src/main.rs".to_owned()),
                },
                super::super::ItemData {
                    display: "lib.rs".to_owned(),
                    detail: None,
                },
            ],
            total_count: 100,
            matched_count: 2,
            preview: None,
        }
    }

    #[test]
    fn render_with_items() {
        let mut backend = MockBackend::new(100, 30);
        let data = make_data(true);
        let bounds = LayoutBounds::calculate(100, 30);

        render_microscope(&mut backend, &data, &bounds);

        // Query row should contain prompt and query.
        let query_row = bounds.query_row as usize;
        assert_eq!(backend.cells[query_row][0].0, '>');
        assert_eq!(backend.cells[query_row][1].0, ' ');
        assert_eq!(backend.cells[query_row][2].0, 't');
    }

    #[test]
    fn render_without_items() {
        let mut backend = MockBackend::new(80, 24);
        let data = MicroscopeData {
            active: true,
            prompt: "> ".to_owned(),
            ..MicroscopeData::default()
        };
        let bounds = LayoutBounds::calculate(80, 24);

        // Should not panic with empty items.
        render_microscope(&mut backend, &data, &bounds);
    }

    #[test]
    fn render_with_preview() {
        let mut backend = MockBackend::new(100, 30);
        let mut data = make_data(true);
        data.preview = Some(super::super::PreviewData {
            lines: vec!["fn main() {".to_owned(), "}".to_owned()],
            highlight_line: Some(0),
        });
        let bounds = LayoutBounds::calculate(100, 30);

        render_microscope(&mut backend, &data, &bounds);

        // Preview separator should be a vertical line.
        if bounds.show_preview {
            let sep_col = bounds.results_width;
            let sep_row = bounds.panel_start_y;
            assert_eq!(backend.char_at(sep_col, sep_row), '│');
        }
    }

    #[test]
    fn render_narrow_no_preview() {
        let mut backend = MockBackend::new(50, 24);
        let data = make_data(true);
        let bounds = LayoutBounds::calculate(50, 24);

        assert!(!bounds.show_preview);
        render_microscope(&mut backend, &data, &bounds);
    }

    #[test]
    fn separator_row() {
        let mut backend = MockBackend::new(80, 24);
        let data = make_data(true);
        let bounds = LayoutBounds::calculate(80, 24);

        render_microscope(&mut backend, &data, &bounds);

        let sep_row = (bounds.query_row + 1) as usize;
        assert_eq!(backend.cells[sep_row][0].0, '─');
    }

    #[test]
    fn selected_item_indicator() {
        let mut backend = MockBackend::new(80, 24);
        let data = make_data(true);
        let bounds = LayoutBounds::calculate(80, 24);

        render_microscope(&mut backend, &data, &bounds);

        // First item should have '>' indicator.
        let first_item_row = bounds.panel_start_y as usize;
        assert_eq!(backend.cells[first_item_row][0].0, '>');

        // Second item should have ' ' indicator.
        if bounds.panel_height > 1 {
            let second_item_row = (bounds.panel_start_y + 1) as usize;
            assert_eq!(backend.cells[second_item_row][0].0, ' ');
        }
    }

    #[test]
    fn count_indicator_on_right() {
        let mut backend = MockBackend::new(80, 24);
        let data = make_data(true);
        let bounds = LayoutBounds::calculate(80, 24);

        render_microscope(&mut backend, &data, &bounds);

        // Count "[2/100]" should be near the right edge.
        let query_row = bounds.query_row as usize;
        let count_text = "[2/100]";
        let start = (bounds.width as usize) - count_text.len();
        let rendered: String = (start..bounds.width as usize)
            .map(|col| backend.cells[query_row][col].0)
            .collect();
        assert_eq!(rendered, count_text);
    }
}
