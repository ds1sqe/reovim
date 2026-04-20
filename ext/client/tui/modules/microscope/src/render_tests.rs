use {
    super::*,
    crate::{ItemData, PreviewData, PreviewHighlightData},
    reovim_client_driver::testing::RecordingSurface,
};

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
            ItemData {
                display: "main.rs".to_owned(),
                detail: Some("src/main.rs".to_owned()),
                icon: None,
            },
            ItemData {
                display: "lib.rs".to_owned(),
                detail: None,
                icon: None,
            },
        ],
        total_count: 100,
        matched_count: 2,
        preview: None,
    }
}

#[test]
fn render_with_items() {
    let mut surface = RecordingSurface::new(100, 30);
    let data = make_data(true);
    let bounds = LayoutBounds::calculate(100, 30);

    render_microscope(&mut surface, &data, &bounds);

    // Query row should contain prompt and query.
    assert_eq!(surface.char_at(0, bounds.query_row), '>');
    assert_eq!(surface.char_at(1, bounds.query_row), ' ');
    assert_eq!(surface.char_at(2, bounds.query_row), 't');
}

#[test]
fn render_without_items() {
    let mut surface = RecordingSurface::new(80, 24);
    let data = MicroscopeData {
        active: true,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    let bounds = LayoutBounds::calculate(80, 24);

    // Should not panic with empty items.
    render_microscope(&mut surface, &data, &bounds);
}

#[test]
fn render_with_preview() {
    let mut surface = RecordingSurface::new(100, 30);
    let mut data = make_data(true);
    data.preview = Some(PreviewData {
        lines: vec!["fn main() {".to_owned(), "}".to_owned()],
        highlight_line: Some(0),
        highlights: Vec::new(),
    });
    let bounds = LayoutBounds::calculate(100, 30);

    render_microscope(&mut surface, &data, &bounds);

    // Preview separator should be a vertical line.
    if bounds.show_preview {
        let sep_col = bounds.results_width;
        let sep_row = bounds.panel_start_y;
        assert_eq!(surface.char_at(sep_col, sep_row), '\u{2502}');
    }
}

#[test]
fn render_narrow_no_preview() {
    let mut surface = RecordingSurface::new(50, 24);
    let data = make_data(true);
    let bounds = LayoutBounds::calculate(50, 24);

    assert!(!bounds.show_preview);
    render_microscope(&mut surface, &data, &bounds);
}

#[test]
fn separator_row() {
    let mut surface = RecordingSurface::new(80, 24);
    let data = make_data(true);
    let bounds = LayoutBounds::calculate(80, 24);

    render_microscope(&mut surface, &data, &bounds);

    assert_eq!(surface.char_at(0, bounds.query_row + 1), '\u{2500}');
}

#[test]
fn selected_item_indicator() {
    let mut surface = RecordingSurface::new(80, 24);
    let data = make_data(true);
    let bounds = LayoutBounds::calculate(80, 24);

    render_microscope(&mut surface, &data, &bounds);

    // First item should have '>' indicator.
    assert_eq!(surface.char_at(0, bounds.panel_start_y), '>');

    // Second item should have ' ' indicator.
    if bounds.panel_height > 1 {
        assert_eq!(surface.char_at(0, bounds.panel_start_y + 1), ' ');
    }
}

#[test]
fn render_items_overflow_panel_height() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = LayoutBounds::calculate(80, 24);
    let many_items: Vec<ItemData> = (0..100)
        .map(|i| ItemData {
            display: format!("item_{i}"),
            detail: None,
            icon: None,
        })
        .collect();
    let data = MicroscopeData {
        active: true,
        items: many_items,
        matched_count: 100,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut surface, &data, &bounds);
}

#[test]
fn render_long_display_text_truncated() {
    let mut surface = RecordingSurface::new(30, 24);
    let bounds = LayoutBounds::calculate(30, 24);
    let data = MicroscopeData {
        active: true,
        items: vec![ItemData {
            display: "a".repeat(200),
            detail: None,
            icon: None,
        }],
        matched_count: 1,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut surface, &data, &bounds);
}

#[test]
fn render_long_detail_text_truncated() {
    let mut surface = RecordingSurface::new(40, 24);
    let bounds = LayoutBounds::calculate(40, 24);
    let data = MicroscopeData {
        active: true,
        items: vec![ItemData {
            display: "x".to_owned(),
            detail: Some("d".repeat(200)),
            icon: None,
        }],
        matched_count: 1,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut surface, &data, &bounds);
}

#[test]
fn render_preview_overflow_panel_height() {
    let mut surface = RecordingSurface::new(100, 24);
    let bounds = LayoutBounds::calculate(100, 24);
    let data = MicroscopeData {
        active: true,
        preview: Some(PreviewData {
            lines: (0..100).map(|i| format!("preview line {i}")).collect(),
            highlight_line: Some(5),
            highlights: Vec::new(),
        }),
        matched_count: 0,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut surface, &data, &bounds);
}

#[test]
fn render_preview_long_line_truncated() {
    let mut surface = RecordingSurface::new(60, 24);
    let bounds = LayoutBounds::calculate(60, 24);
    let data = MicroscopeData {
        active: true,
        preview: Some(PreviewData {
            lines: vec!["x".repeat(200)],
            highlight_line: None,
            highlights: Vec::new(),
        }),
        matched_count: 0,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut surface, &data, &bounds);
}

#[test]
fn render_preview_line_num_overflow() {
    let mut surface = RecordingSurface::new(100, 30);
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
        preview: Some(PreviewData {
            lines: vec!["hello".to_owned()],
            highlight_line: None,
            highlights: Vec::new(),
        }),
        matched_count: 0,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut surface, &data, &bounds);
}

#[test]
fn count_indicator_on_right() {
    let mut surface = RecordingSurface::new(80, 24);
    let data = make_data(true);
    let bounds = LayoutBounds::calculate(80, 24);

    render_microscope(&mut surface, &data, &bounds);

    // Count "[2/100]" should be near the right edge.
    let count_text = "[2/100]";
    #[allow(clippy::cast_possible_truncation)]
    let start = bounds.width - count_text.len() as u16;
    let rendered: String = (start..bounds.width)
        .map(|col| surface.char_at(col, bounds.query_row))
        .collect();
    assert_eq!(rendered, count_text);
}

#[test]
fn selected_item_scrolled_into_view() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = LayoutBounds::calculate(80, 24);
    let many_items: Vec<ItemData> = (0..100)
        .map(|i| ItemData {
            display: format!("item_{i}"),
            detail: None,
            icon: None,
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
    render_microscope(&mut surface, &data, &bounds);

    // The selected item must appear somewhere in the panel.
    let mut found_selected = false;
    for row in bounds.panel_start_y..(bounds.panel_start_y + bounds.panel_height) {
        if surface.char_at(0, row) == '>' {
            let text: String = (2..10).map(|col| surface.char_at(col, row)).collect();
            assert!(text.starts_with("item_50"), "selected row shows: {text}");
            found_selected = true;
            break;
        }
    }
    assert!(found_selected, "selected item must be visible in panel");
}

#[test]
fn scroll_shows_correct_items() {
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
    let mut surface = RecordingSurface::new(80, 12);
    let many_items: Vec<ItemData> = (0..20)
        .map(|i| ItemData {
            display: format!("item_{i:02}"),
            detail: None,
            icon: None,
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
    render_results(&mut surface, &data, &bounds);

    // First visible row should be item_03 (scroll offset = 3).
    let first_row = bounds.panel_start_y;
    let text: String = (2..9).map(|col| surface.char_at(col, first_row)).collect();
    assert_eq!(text, "item_03");

    // Last visible row should be item_10 (selected, with '>').
    let last_row = bounds.panel_start_y + bounds.panel_height - 1;
    assert_eq!(surface.char_at(0, last_row), '>');
    let text: String = (2..9).map(|col| surface.char_at(col, last_row)).collect();
    assert_eq!(text, "item_10");
}

#[test]
fn no_scroll_when_selected_in_view() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = LayoutBounds::calculate(80, 24);
    let items: Vec<ItemData> = (0..5)
        .map(|i| ItemData {
            display: format!("item_{i}"),
            detail: None,
            icon: None,
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
    render_microscope(&mut surface, &data, &bounds);

    // First item should be item_0 (no scroll).
    let first_row = bounds.panel_start_y;
    let text: String = (2..8).map(|col| surface.char_at(col, first_row)).collect();
    assert_eq!(text, "item_0");

    // Third item (index 2) should have '>' indicator.
    let sel_row = bounds.panel_start_y + 2;
    assert_eq!(surface.char_at(0, sel_row), '>');
}

#[test]
fn query_row_overflow_narrow_width() {
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
    let mut surface = RecordingSurface::new(10, 10);
    let data = MicroscopeData {
        active: true,
        prompt: "> > > ".to_owned(), // 6 chars, wider than width=4
        query: "abcdef".to_owned(),  // also wider
        ..MicroscopeData::default()
    };

    // Render the full microscope (which calls render_query_row internally).
    render_microscope(&mut surface, &data, &bounds);

    // Cells beyond width should remain as default space.
    assert_eq!(surface.char_at(4, 0), ' ');
    assert_eq!(surface.char_at(5, 0), ' ');
}

#[test]
fn zero_panel_height_scroll() {
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
    let mut surface = RecordingSurface::new(80, 10);
    let data = MicroscopeData {
        active: true,
        items: vec![ItemData {
            display: "item".to_owned(),
            detail: None,
            icon: None,
        }],
        selected: 5,
        matched_count: 1,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    // Should not panic -- no items rendered when panel_height is 0.
    render_results(&mut surface, &data, &bounds);
}

#[test]
fn detail_skipped_when_no_room() {
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
    let mut surface = RecordingSurface::new(20, 10);
    let data = MicroscopeData {
        active: true,
        items: vec![ItemData {
            display: "longname".to_owned(), // 8 chars + 2 prefix = fills results_width=10
            detail: Some("detail".to_owned()),
            icon: None,
        }],
        matched_count: 1,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_results(&mut surface, &data, &bounds);
    // Detail should be skipped because col + 2 >= results_width after display.
}

#[test]
fn icon_rendered_before_display_text() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = LayoutBounds::calculate(80, 24);
    let data = MicroscopeData {
        active: true,
        items: vec![ItemData {
            display: "main.rs".to_owned(),
            detail: None,
            icon: Some("\u{e7a8}".to_owned()),
        }],
        matched_count: 1,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_results(&mut surface, &data, &bounds);

    // Layout: '>' ' ' icon ' ' 'm' 'a' 'i' 'n' ...
    // col 0: '>' (selected), col 1: ' ', col 2: icon, col 3: ' ', col 4: 'm'
    assert_eq!(surface.char_at(0, bounds.panel_start_y), '>');
    assert_eq!(surface.char_at(2, bounds.panel_start_y), '\u{e7a8}');
    assert_eq!(surface.char_at(3, bounds.panel_start_y), ' ');
    assert_eq!(surface.char_at(4, bounds.panel_start_y), 'm');
}

#[test]
fn no_icon_same_position_as_before() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = LayoutBounds::calculate(80, 24);
    let data = MicroscopeData {
        active: true,
        items: vec![ItemData {
            display: "main.rs".to_owned(),
            detail: None,
            icon: None,
        }],
        matched_count: 1,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_results(&mut surface, &data, &bounds);

    // Without icon: '>' ' ' 'm' 'a' 'i' 'n' ...
    assert_eq!(surface.char_at(0, bounds.panel_start_y), '>');
    assert_eq!(surface.char_at(2, bounds.panel_start_y), 'm');
}

#[test]
fn icon_skipped_when_no_room() {
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
    let mut surface = RecordingSurface::new(10, 10);
    let data = MicroscopeData {
        active: true,
        items: vec![ItemData {
            display: "x".to_owned(),
            detail: None,
            icon: Some("\u{e7a8}".to_owned()),
        }],
        matched_count: 1,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_results(&mut surface, &data, &bounds);
    // col=2 after indicator, col+2=4 which is NOT < results_width=4, so icon skipped.
    // Display text 'x' should be at col 2.
    assert_eq!(surface.char_at(2, bounds.panel_start_y), 'x');
}

// ========================================================================
// Syntax highlighting tests
// ========================================================================

#[test]
fn syntax_category_color_mapping() {
    assert_eq!(syntax_category_color("keyword"), Some(Color::AnsiValue(141)));
    assert_eq!(syntax_category_color("keyword.control"), Some(Color::AnsiValue(141)));
    assert_eq!(syntax_category_color("string"), Some(Color::AnsiValue(114)));
    assert_eq!(syntax_category_color("string.escape"), Some(Color::AnsiValue(114)));
    assert_eq!(syntax_category_color("comment"), Some(Color::AnsiValue(245)));
    assert_eq!(syntax_category_color("type"), Some(Color::AnsiValue(221)));
    assert_eq!(syntax_category_color("type.builtin"), Some(Color::AnsiValue(221)));
    assert_eq!(syntax_category_color("function"), Some(Color::AnsiValue(81)));
    assert_eq!(syntax_category_color("function.method"), Some(Color::AnsiValue(81)));
    assert_eq!(syntax_category_color("number"), Some(Color::AnsiValue(208)));
    assert_eq!(syntax_category_color("boolean"), Some(Color::AnsiValue(208)));
    assert_eq!(syntax_category_color("operator"), Some(Color::AnsiValue(250)));
    assert_eq!(syntax_category_color("punctuation"), Some(Color::AnsiValue(250)));
    assert_eq!(syntax_category_color("variable"), Some(Color::AnsiValue(253)));
    assert_eq!(syntax_category_color("constant"), Some(Color::AnsiValue(208)));
    assert_eq!(syntax_category_color("attribute"), Some(Color::AnsiValue(114)));
    assert_eq!(syntax_category_color("unknown_category"), None);
}

#[test]
fn syntax_color_at_finds_matching_highlight() {
    let highlights = vec![
        PreviewHighlightData {
            line: 0,
            col_start: 0,
            col_end: 2,
            category: "keyword".to_owned(),
        },
        PreviewHighlightData {
            line: 0,
            col_start: 3,
            col_end: 7,
            category: "function".to_owned(),
        },
    ];
    assert_eq!(syntax_color_at(&highlights, 0, 0), Some(Color::AnsiValue(141)));
    assert_eq!(syntax_color_at(&highlights, 0, 1), Some(Color::AnsiValue(141)));
    assert_eq!(syntax_color_at(&highlights, 0, 2), None); // between spans
    assert_eq!(syntax_color_at(&highlights, 0, 3), Some(Color::AnsiValue(81)));
    assert_eq!(syntax_color_at(&highlights, 0, 6), Some(Color::AnsiValue(81)));
    assert_eq!(syntax_color_at(&highlights, 0, 7), None); // past end
    assert_eq!(syntax_color_at(&highlights, 1, 0), None); // different line
}

#[test]
fn render_preview_with_syntax_highlights() {
    let mut surface = RecordingSurface::new(100, 30);
    let bounds = LayoutBounds::calculate(100, 30);
    let data = MicroscopeData {
        active: true,
        preview: Some(PreviewData {
            lines: vec!["fn main() {}".to_owned()],
            highlight_line: None,
            highlights: vec![PreviewHighlightData {
                line: 0,
                col_start: 0,
                col_end: 2,
                category: "keyword".to_owned(),
            }],
        }),
        matched_count: 0,
        prompt: "> ".to_owned(),
        ..MicroscopeData::default()
    };
    render_microscope(&mut surface, &data, &bounds);

    // The "fn" text should be rendered with keyword color (AnsiValue 141)
    // Line number takes 4 chars, so "f" is at preview_x + 4.
    let content_col = bounds.preview_x + 4;
    let row = bounds.panel_start_y;
    let style = surface.style_at(content_col, row);
    assert_eq!(style.fg, Some(Color::AnsiValue(141)));

    // "m" in "main" (byte index 3) has no highlight, so uses default fg
    let main_col = content_col + 3;
    let main_style = surface.style_at(main_col, row);
    assert_eq!(main_style.fg, Some(Color::AnsiValue(250)));
}
