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
        self.cells = vec![vec![(' ', default_style); self.width as usize]; self.height as usize];
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
fn render_items_overflow_panel_height() {
    // Many items exceeding panel_height triggers line 110 break.
    let mut backend = MockBackend::new(80, 24);
    let bounds = LayoutBounds::calculate(80, 24);
    let many_items: Vec<super::super::ItemData> = (0..100)
        .map(|i| super::super::ItemData {
            display: format!("item_{i}"),
            detail: None,
        })
        .collect();
    let data = MicroscopeData {
        active: true,
        items: many_items,
        matched_count: 100,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut backend, &data, &bounds);
}

#[test]
fn render_long_display_text_truncated() {
    // Display text wider than results_width triggers line 137 break.
    let mut backend = MockBackend::new(30, 24);
    let bounds = LayoutBounds::calculate(30, 24);
    let data = MicroscopeData {
        active: true,
        items: vec![super::super::ItemData {
            display: "a".repeat(200),
            detail: None,
        }],
        matched_count: 1,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut backend, &data, &bounds);
}

#[test]
fn render_long_detail_text_truncated() {
    // Detail text overflow triggers line 151 break.
    let mut backend = MockBackend::new(40, 24);
    let bounds = LayoutBounds::calculate(40, 24);
    let data = MicroscopeData {
        active: true,
        items: vec![super::super::ItemData {
            display: "x".to_owned(),
            detail: Some("d".repeat(200)),
        }],
        matched_count: 1,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut backend, &data, &bounds);
}

#[test]
fn render_preview_overflow_panel_height() {
    // Many preview lines exceeding panel_height triggers line 177 break.
    let mut backend = MockBackend::new(100, 24);
    let bounds = LayoutBounds::calculate(100, 24);
    let data = MicroscopeData {
        active: true,
        preview: Some(super::super::PreviewData {
            lines: (0..100).map(|i| format!("preview line {i}")).collect(),
            highlight_line: Some(5),
        }),
        matched_count: 0,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut backend, &data, &bounds);
}

#[test]
fn render_preview_long_line_truncated() {
    // Preview line wider than preview_width triggers lines 193/202 break.
    let mut backend = MockBackend::new(60, 24);
    let bounds = LayoutBounds::calculate(60, 24);
    let data = MicroscopeData {
        active: true,
        preview: Some(super::super::PreviewData {
            lines: vec!["x".repeat(200)],
            highlight_line: None,
        }),
        matched_count: 0,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut backend, &data, &bounds);
}

#[test]
fn render_preview_line_num_overflow() {
    // preview_width < 4 triggers line 193 break (line number is 4 chars).
    let mut backend = MockBackend::new(100, 30);
    let bounds = LayoutBounds {
        x: 0,
        y: 18,
        width: 100,
        total_height: 12,
        query_row: 18,
        panel_start_y: 20,
        panel_height: 10,
        results_width: 40,
        show_preview: true,
        preview_width: 2, // Narrower than a line number
        preview_x: 41,
    };
    let data = MicroscopeData {
        active: true,
        preview: Some(super::super::PreviewData {
            lines: vec!["hello".to_owned()],
            highlight_line: None,
        }),
        matched_count: 0,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut backend, &data, &bounds);
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

#[test]
fn selected_item_scrolled_into_view() {
    // 100 items with selected=50 — item 50 should be visible with '>' indicator.
    let mut backend = MockBackend::new(80, 24);
    let bounds = LayoutBounds::calculate(80, 24);
    let many_items: Vec<super::super::ItemData> = (0..100)
        .map(|i| super::super::ItemData {
            display: format!("item_{i}"),
            detail: None,
        })
        .collect();
    let data = MicroscopeData {
        active: true,
        items: many_items,
        selected: 50,
        matched_count: 100,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut backend, &data, &bounds);

    // The selected item must appear somewhere in the panel.
    let mut found_selected = false;
    for row in bounds.panel_start_y..(bounds.panel_start_y + bounds.panel_height) {
        if backend.char_at(0, row) == '>' {
            // Verify it's item_50 by checking display text starts after "> ".
            let text: String = (2..10).map(|col| backend.char_at(col, row)).collect();
            assert!(text.starts_with("item_50"), "selected row shows: {text}");
            found_selected = true;
            break;
        }
    }
    assert!(found_selected, "selected item must be visible in panel");
}

#[test]
fn scroll_shows_correct_items() {
    // With panel_height=8 and selected=10, items 3..11 should be shown
    // (scroll = 10 - 8 + 1 = 3).
    let bounds = LayoutBounds {
        x: 0,
        y: 0,
        width: 80,
        total_height: 10,
        query_row: 0,
        panel_start_y: 2,
        panel_height: 8,
        results_width: 80,
        show_preview: false,
        preview_width: 0,
        preview_x: 0,
    };
    let mut backend = MockBackend::new(80, 12);
    let many_items: Vec<super::super::ItemData> = (0..20)
        .map(|i| super::super::ItemData {
            display: format!("item_{i:02}"),
            detail: None,
        })
        .collect();
    let data = MicroscopeData {
        active: true,
        items: many_items,
        selected: 10,
        matched_count: 20,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_results(&mut backend, &data, &bounds);

    // First visible row should be item_03 (scroll offset = 3).
    let first_row = bounds.panel_start_y;
    let text: String = (2..9).map(|col| backend.char_at(col, first_row)).collect();
    assert_eq!(text, "item_03");

    // Last visible row should be item_10 (selected, with '>').
    let last_row = bounds.panel_start_y + bounds.panel_height - 1;
    assert_eq!(backend.char_at(0, last_row), '>');
    let text: String = (2..9).map(|col| backend.char_at(col, last_row)).collect();
    assert_eq!(text, "item_10");
}

#[test]
fn no_scroll_when_selected_in_view() {
    // 5 items with selected=2 — no scrolling needed, items start from 0.
    let mut backend = MockBackend::new(80, 24);
    let bounds = LayoutBounds::calculate(80, 24);
    let items: Vec<super::super::ItemData> = (0..5)
        .map(|i| super::super::ItemData {
            display: format!("item_{i}"),
            detail: None,
        })
        .collect();
    let data = MicroscopeData {
        active: true,
        items,
        selected: 2,
        matched_count: 5,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut backend, &data, &bounds);

    // First item should be item_0 (no scroll).
    let first_row = bounds.panel_start_y;
    let text: String = (2..8).map(|col| backend.char_at(col, first_row)).collect();
    assert_eq!(text, "item_0");

    // Third item (index 2) should have '>' indicator.
    let sel_row = bounds.panel_start_y + 2;
    assert_eq!(backend.char_at(0, sel_row), '>');
}

#[test]
fn query_row_overflow_narrow_width() {
    // Prompt and query wider than width — exercises overflow branches
    // at lines 48, 56, and 67 (false branches: col >= width).
    let bounds = LayoutBounds {
        x: 0,
        y: 0,
        width: 4,
        total_height: 4,
        query_row: 0,
        panel_start_y: 2,
        panel_height: 2,
        results_width: 4,
        show_preview: false,
        preview_width: 0,
        preview_x: 0,
    };
    let mut backend = MockBackend::new(10, 10);
    let data = MicroscopeData {
        active: true,
        prompt: "> > > ".to_owned(), // 6 chars, wider than width=4
        query: "abcdef".to_owned(),  // also wider
        ..MicroscopeData::default()
    };
    render_query_row(&mut backend, &data, &bounds);
    // Cells beyond width should remain as default space.
    assert_eq!(backend.char_at(4, 0), ' ');
    assert_eq!(backend.char_at(5, 0), ' ');
}

#[test]
fn zero_panel_height_scroll() {
    // panel_height=0 triggers the panel_h == 0 branch (line 110).
    let bounds = LayoutBounds {
        x: 0,
        y: 0,
        width: 80,
        total_height: 2,
        query_row: 0,
        panel_start_y: 2,
        panel_height: 0,
        results_width: 80,
        show_preview: false,
        preview_width: 0,
        preview_x: 0,
    };
    let mut backend = MockBackend::new(80, 10);
    let data = MicroscopeData {
        active: true,
        items: vec![super::super::ItemData {
            display: "item".to_owned(),
            detail: None,
        }],
        selected: 5,
        matched_count: 1,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    // Should not panic — no items rendered when panel_height is 0.
    render_results(&mut backend, &data, &bounds);
}

#[test]
fn detail_skipped_when_no_room() {
    // Detail present but col + 2 >= results_width (line 153 false branch).
    let bounds = LayoutBounds {
        x: 0,
        y: 0,
        width: 10,
        total_height: 4,
        query_row: 0,
        panel_start_y: 2,
        panel_height: 2,
        results_width: 10,
        show_preview: false,
        preview_width: 0,
        preview_x: 0,
    };
    let mut backend = MockBackend::new(20, 10);
    let data = MicroscopeData {
        active: true,
        items: vec![super::super::ItemData {
            display: "longname".to_owned(), // 8 chars + 2 prefix = fills results_width=10
            detail: Some("detail".to_owned()),
        }],
        matched_count: 1,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_results(&mut backend, &data, &bounds);
    // Detail should be skipped because col + 2 >= results_width after display.
}
