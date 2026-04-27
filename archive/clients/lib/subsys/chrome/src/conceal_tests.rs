use {super::*, reovim_client_subsys_module::Attributes};

// =============================================================================
// ConcealedLine::identity
// =============================================================================

#[test]
fn identity_empty() {
    let c = ConcealedLine::identity("");
    assert_eq!(c.text, "");
    assert_eq!(c.col_mapping, vec![0]);
    assert!(c.styles.is_empty());
}

#[test]
fn identity_ascii() {
    let c = ConcealedLine::identity("hello");
    assert_eq!(c.text, "hello");
    assert_eq!(c.col_mapping, vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(c.styles.len(), 5);
    assert!(c.styles.iter().all(Option::is_none));
}

#[test]
fn identity_unicode() {
    let c = ConcealedLine::identity("héllo");
    assert_eq!(c.text, "héllo");
    assert_eq!(c.col_mapping.len(), 6); // 5 chars + 1
}

// =============================================================================
// apply_conceals
// =============================================================================

#[test]
fn apply_conceals_empty_decorations() {
    let c = apply_conceals("hello world", &[]);
    assert_eq!(c.text, "hello world");
}

#[test]
fn apply_conceals_hide() {
    let decorations = vec![ConcealDecoration {
        start_col: 5,
        end_col: 11,
        replacement: None,
        style: None,
    }];
    let c = apply_conceals("hello world", &decorations);
    assert_eq!(c.text, "hello");
}

#[test]
fn apply_conceals_replace() {
    let decorations = vec![ConcealDecoration {
        start_col: 5,
        end_col: 11,
        replacement: Some("!".to_owned()),
        style: None,
    }];
    let c = apply_conceals("hello world", &decorations);
    assert_eq!(c.text, "hello!");
}

#[test]
fn apply_conceals_replace_with_style() {
    let style = Style::new().fg(reovim_arch::Color::Red);
    let decorations = vec![ConcealDecoration {
        start_col: 0,
        end_col: 5,
        replacement: Some("X".to_owned()),
        style: Some(style.clone()),
    }];
    let c = apply_conceals("hello world", &decorations);
    assert_eq!(c.text, "X world");
    assert_eq!(c.styles[0], Some(style));
    assert!(c.styles[1].is_none()); // space
}

#[test]
fn apply_conceals_multiple() {
    let decorations = vec![
        ConcealDecoration {
            start_col: 0,
            end_col: 1,
            replacement: Some("A".to_owned()),
            style: None,
        },
        ConcealDecoration {
            start_col: 4,
            end_col: 5,
            replacement: Some("B".to_owned()),
            style: None,
        },
    ];
    let c = apply_conceals("hello", &decorations);
    assert_eq!(c.text, "AellB");
}

#[test]
fn apply_conceals_overlapping_regions() {
    let decorations = vec![
        ConcealDecoration {
            start_col: 0,
            end_col: 5,
            replacement: Some("X".to_owned()),
            style: None,
        },
        ConcealDecoration {
            start_col: 3,
            end_col: 8,
            replacement: Some("Y".to_owned()),
            style: None,
        },
    ];
    let c = apply_conceals("hello world", &decorations);
    // Second region starts at 3, but source_char is already at 5, so skipped
    assert_eq!(c.text, "X world");
}

#[test]
fn apply_conceals_at_end() {
    let decorations = vec![ConcealDecoration {
        start_col: 3,
        end_col: 5,
        replacement: None,
        style: None,
    }];
    let c = apply_conceals("hello", &decorations);
    assert_eq!(c.text, "hel");
}

#[test]
fn apply_conceals_col_mapping() {
    // "hello" -> conceal [1..3] with "X" -> "hXlo"
    let decorations = vec![ConcealDecoration {
        start_col: 1,
        end_col: 3,
        replacement: Some("X".to_owned()),
        style: None,
    }];
    let c = apply_conceals("hello", &decorations);
    assert_eq!(c.text, "hXlo");
    // col_mapping: [0, 1, 3, 4, 5]
    // display 0 -> source 0 (h)
    // display 1 -> source 1 (X replaces e,l)
    // display 2 -> source 3 (l)
    // display 3 -> source 4 (o)
    // display 4 -> source 5 (end)
    assert_eq!(c.col_mapping[0], 0);
    assert_eq!(c.col_mapping[1], 1);
    assert_eq!(c.col_mapping[2], 3);
    assert_eq!(c.col_mapping[3], 4);
    assert_eq!(c.col_mapping[4], 5);
}

// =============================================================================
// source_to_display_col
// =============================================================================

#[test]
fn source_to_display_identity() {
    let c = ConcealedLine::identity("hello");
    assert_eq!(source_to_display_col(&c, 0), 0);
    assert_eq!(source_to_display_col(&c, 2), 2);
    assert_eq!(source_to_display_col(&c, 5), 5);
}

#[test]
fn source_to_display_with_conceal() {
    // "hello" -> conceal [1..3] with "X" -> "hXlo"
    let decorations = vec![ConcealDecoration {
        start_col: 1,
        end_col: 3,
        replacement: Some("X".to_owned()),
        style: None,
    }];
    let c = apply_conceals("hello", &decorations);
    assert_eq!(source_to_display_col(&c, 0), 0); // h -> h
    assert_eq!(source_to_display_col(&c, 1), 1); // e -> X
    assert_eq!(source_to_display_col(&c, 3), 2); // l -> l
    assert_eq!(source_to_display_col(&c, 4), 3); // o -> o
}

#[test]
fn source_to_display_past_end() {
    let c = ConcealedLine::identity("hi");
    assert_eq!(source_to_display_col(&c, 10), 2); // past end returns text.len()
}

// =============================================================================
// dim_style
// =============================================================================

#[test]
fn dim_style_full_opacity() {
    let style = Style::new().fg(reovim_arch::Color::Red);
    let dimmed = dim_style(&style, 1.0, reovim_arch::Color::Black);
    assert_eq!(dimmed, style);
}

#[test]
fn dim_style_zero_opacity() {
    let style = Style::new().fg(reovim_arch::Color::White);
    let dimmed = dim_style(&style, 0.0, reovim_arch::Color::Black);
    // Should be close to black
    assert!(dimmed.fg.is_some());
    if let Some(reovim_arch::Color::Rgb { r, g, b }) = dimmed.fg {
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }
}

#[test]
fn dim_style_half_opacity() {
    let style = Style::new().fg(reovim_arch::Color::Rgb {
        r: 200,
        g: 100,
        b: 0,
    });
    let dimmed = dim_style(&style, 0.5, reovim_arch::Color::Black);
    if let Some(reovim_arch::Color::Rgb { r, g, b }) = dimmed.fg {
        assert_eq!(r, 100);
        assert_eq!(g, 50);
        assert_eq!(b, 0);
    }
}

#[test]
fn dim_style_preserves_attributes() {
    let style = Style::new().fg(reovim_arch::Color::Red).bold().italic();
    let dimmed = dim_style(&style, 0.5, reovim_arch::Color::Black);
    assert!(dimmed.attributes.contains(Attributes::BOLD));
    assert!(dimmed.attributes.contains(Attributes::ITALIC));
}

#[test]
fn dim_style_no_colors() {
    let style = Style::new().bold();
    let dimmed = dim_style(&style, 0.5, reovim_arch::Color::Black);
    assert!(dimmed.fg.is_none());
    assert!(dimmed.bg.is_none());
    assert!(dimmed.attributes.contains(Attributes::BOLD));
}

// =============================================================================
// SyntaxToken
// =============================================================================

#[test]
fn syntax_token_debug() {
    let token = SyntaxToken {
        line: 0,
        start_col: 0,
        end_col: 5,
        category: "keyword".to_owned(),
    };
    let debug = format!("{token:?}");
    assert!(debug.contains("keyword"));
}

// =============================================================================
// ConcealDecoration
// =============================================================================

#[test]
fn conceal_decoration_debug() {
    let d = ConcealDecoration {
        start_col: 0,
        end_col: 5,
        replacement: Some("X".to_owned()),
        style: None,
    };
    let debug = format!("{d:?}");
    assert!(debug.contains("ConcealDecoration"));
}

// =============================================================================
// color_to_rgb
// =============================================================================

#[test]
fn color_to_rgb_named_colors() {
    assert_eq!(color_to_rgb(reovim_arch::Color::Black), (0, 0, 0));
    assert_eq!(color_to_rgb(reovim_arch::Color::White), (255, 255, 255));
    assert_eq!(color_to_rgb(reovim_arch::Color::Red), (255, 0, 0));
}

#[test]
fn color_to_rgb_rgb() {
    assert_eq!(
        color_to_rgb(reovim_arch::Color::Rgb {
            r: 42,
            g: 84,
            b: 126
        }),
        (42, 84, 126)
    );
}

#[test]
fn color_to_rgb_ansi256() {
    // Standard color 0 = black
    assert_eq!(color_to_rgb(reovim_arch::Color::AnsiValue(0)), (0, 0, 0));
    // Grayscale 232 = darkest gray
    let (r, g, b) = color_to_rgb(reovim_arch::Color::AnsiValue(232));
    assert_eq!(r, g);
    assert_eq!(g, b);
    assert_eq!(r, 8);
}

// =============================================================================
// ansi256_to_rgb
// =============================================================================

#[test]
fn ansi256_standard() {
    assert_eq!(ansi256_to_rgb(0), (0, 0, 0));
    assert_eq!(ansi256_to_rgb(15), (255, 255, 255));
}

#[test]
fn ansi256_cube() {
    // Color 16 = (0,0,0) in cube
    assert_eq!(ansi256_to_rgb(16), (0, 0, 0));
    // Color 196 = (5,0,0) = red
    assert_eq!(ansi256_to_rgb(196), (255, 0, 0));
}

#[test]
fn ansi256_grayscale() {
    assert_eq!(ansi256_to_rgb(232), (8, 8, 8));
    assert_eq!(ansi256_to_rgb(255), (238, 238, 238));
}
