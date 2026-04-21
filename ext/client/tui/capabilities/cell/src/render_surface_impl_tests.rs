use {
    super::{convert_attrs, convert_color, convert_style},
    crate::{
        capability::{Cell, CellCapability},
        style::{CellAttrs, CellColor, CellStyle},
    },
    reovim_client_driver::{
        Rect, Style,
        traits::RenderSurface,
        types::{Attributes, Color},
    },
};

// =========================================================================
// convert_color — 19-variant exhaustive coverage
// =========================================================================

#[test]
fn convert_color_reset_maps_to_default() {
    assert_eq!(convert_color(Color::Reset), CellColor::Default);
}

#[test]
fn convert_color_named_ansi_0_through_15() {
    let cases = [
        (Color::Black, 0u8),
        (Color::DarkRed, 1),
        (Color::DarkGreen, 2),
        (Color::DarkYellow, 3),
        (Color::DarkBlue, 4),
        (Color::DarkMagenta, 5),
        (Color::DarkCyan, 6),
        (Color::Grey, 7),
        (Color::DarkGrey, 8),
        (Color::Red, 9),
        (Color::Green, 10),
        (Color::Yellow, 11),
        (Color::Blue, 12),
        (Color::Magenta, 13),
        (Color::Cyan, 14),
        (Color::White, 15),
    ];
    for (driver_color, ansi_idx) in cases {
        assert_eq!(
            convert_color(driver_color),
            CellColor::Named(ansi_idx),
            "{driver_color:?} should map to Named({ansi_idx})",
        );
    }
}

#[test]
fn convert_color_ansi_value_preserves_index() {
    assert_eq!(convert_color(Color::AnsiValue(42)), CellColor::Ansi256(42));
    assert_eq!(
        convert_color(Color::AnsiValue(0)),
        CellColor::Ansi256(0),
    );
    assert_eq!(
        convert_color(Color::AnsiValue(255)),
        CellColor::Ansi256(255),
    );
}

#[test]
fn convert_color_rgb_preserves_components() {
    assert_eq!(
        convert_color(Color::Rgb { r: 1, g: 2, b: 3 }),
        CellColor::Rgb(1, 2, 3),
    );
    assert_eq!(
        convert_color(Color::Rgb {
            r: 255,
            g: 128,
            b: 0,
        }),
        CellColor::Rgb(255, 128, 0),
    );
}

// =========================================================================
// convert_attrs — per-flag coverage
// =========================================================================

#[test]
fn convert_attrs_empty_is_empty() {
    assert_eq!(convert_attrs(Attributes::new()), CellAttrs::empty());
}

#[test]
fn convert_attrs_bold_maps() {
    assert_eq!(convert_attrs(Attributes::BOLD), CellAttrs::BOLD);
}

#[test]
fn convert_attrs_italic_maps() {
    assert_eq!(convert_attrs(Attributes::ITALIC), CellAttrs::ITALIC);
}

#[test]
fn convert_attrs_underline_maps() {
    assert_eq!(
        convert_attrs(Attributes::UNDERLINE),
        CellAttrs::UNDERLINE,
    );
}

#[test]
fn convert_attrs_reverse_maps() {
    assert_eq!(convert_attrs(Attributes::REVERSE), CellAttrs::REVERSE);
}

#[test]
fn convert_attrs_dim_maps() {
    assert_eq!(convert_attrs(Attributes::DIM), CellAttrs::DIM);
}

#[test]
fn convert_attrs_strikethrough_is_dropped() {
    // STRIKETHROUGH has no CellAttrs counterpart per §2.2 locked.
    // The converter silently drops it.
    assert_eq!(
        convert_attrs(Attributes::STRIKETHROUGH),
        CellAttrs::empty(),
    );
}

#[test]
fn convert_attrs_strikethrough_combined_with_bold_drops_only_strikethrough() {
    let mixed = Attributes::BOLD | Attributes::STRIKETHROUGH;
    assert_eq!(convert_attrs(mixed), CellAttrs::BOLD);
}

#[test]
fn convert_attrs_all_convertible_flags_together() {
    let all = Attributes::BOLD
        | Attributes::ITALIC
        | Attributes::UNDERLINE
        | Attributes::REVERSE
        | Attributes::DIM;
    let out = convert_attrs(all);
    assert!(out.contains(CellAttrs::BOLD));
    assert!(out.contains(CellAttrs::ITALIC));
    assert!(out.contains(CellAttrs::UNDERLINE));
    assert!(out.contains(CellAttrs::REVERSE));
    assert!(out.contains(CellAttrs::DIM));
}

// =========================================================================
// convert_style — composes color + attrs
// =========================================================================

#[test]
fn convert_style_preserves_fg_bg_attrs() {
    let s = Style::new()
        .fg(Color::Red)
        .bg(Color::Rgb { r: 1, g: 2, b: 3 })
        .bold();
    let cs = convert_style(&s);
    assert_eq!(cs.fg, Some(CellColor::Named(9)));
    assert_eq!(cs.bg, Some(CellColor::Rgb(1, 2, 3)));
    assert!(cs.attrs.contains(CellAttrs::BOLD));
}

#[test]
fn convert_style_unspecified_fg_bg_stay_none() {
    let s = Style::new();
    let cs = convert_style(&s);
    assert_eq!(cs.fg, None);
    assert_eq!(cs.bg, None);
    assert!(cs.attrs.is_empty());
}

// =========================================================================
// RenderSurface impl — 6 methods + bounds behaviour
// =========================================================================

fn grid() -> CellCapability {
    CellCapability::new(10, 5)
}

#[test]
fn size_returns_width_height() {
    let g = grid();
    assert_eq!(g.size(), (10, 5));
}

#[test]
fn write_styled_writes_chars_at_position() {
    let mut g = grid();
    let written = g.write_styled(2, 1, "abc", Style::new().fg(Color::Red));
    assert_eq!(written, 3);
    assert_eq!(g.get_cell(2, 1).map(|c| c.ch), Some('a'));
    assert_eq!(g.get_cell(3, 1).map(|c| c.ch), Some('b'));
    assert_eq!(g.get_cell(4, 1).map(|c| c.ch), Some('c'));
    // Style propagated.
    assert_eq!(
        g.get_cell(2, 1).unwrap().style.fg,
        Some(CellColor::Named(9)),
    );
}

#[test]
fn write_styled_truncates_at_width() {
    let mut g = grid();
    // Width 10, start at x=8, text "hello" (5 chars) — only 2 fit.
    let written = g.write_styled(8, 0, "hello", Style::new());
    assert_eq!(written, 2);
    assert_eq!(g.get_cell(8, 0).map(|c| c.ch), Some('h'));
    assert_eq!(g.get_cell(9, 0).map(|c| c.ch), Some('e'));
}

#[test]
fn write_styled_empty_text_returns_zero() {
    let mut g = grid();
    assert_eq!(g.write_styled(0, 0, "", Style::new()), 0);
}

#[test]
fn write_styled_at_oob_y_is_noop() {
    let mut g = grid();
    let written = g.write_styled(0, 99, "abc", Style::new());
    // Per §2.4: one char = one cell; we return chars_count capped at
    // remaining width (no y-OOB fast-path). write_cell rejects each
    // write but the counter still advances since chars are iterated.
    // What the counter returns isn't critical — the bounds-respect
    // check is: no cell was actually written.
    assert!(written <= 3);
    // Grid unchanged (all cells remain at default).
    for y in 0..5 {
        for x in 0..10 {
            assert_eq!(g.get_cell(x, y), Some(&Cell::default()));
        }
    }
}

#[test]
fn apply_style_changes_style_preserves_char() {
    let mut g = grid();
    g.write_styled(0, 0, "X", Style::new());
    g.apply_style(0, 0, Style::new().fg(Color::Red).bold());
    let c = g.get_cell(0, 0).unwrap();
    assert_eq!(c.ch, 'X');
    assert_eq!(c.style.fg, Some(CellColor::Named(9)));
    assert!(c.style.attrs.contains(CellAttrs::BOLD));
}

#[test]
fn apply_style_oob_is_noop() {
    let mut g = grid();
    g.apply_style(99, 99, Style::new().fg(Color::Red));
    // No panic, no change.
    assert_eq!(g.get_cell(0, 0), Some(&Cell::default()));
}

#[test]
fn overlay_bg_swaps_only_background() {
    let mut g = grid();
    g.write_styled(0, 0, "Q", Style::new().fg(Color::Blue));
    g.overlay_bg(0, 0, Color::Green);
    let c = g.get_cell(0, 0).unwrap();
    assert_eq!(c.ch, 'Q');
    assert_eq!(c.style.fg, Some(CellColor::Named(12))); // Blue
    assert_eq!(c.style.bg, Some(CellColor::Named(10))); // Green
}

#[test]
fn overlay_bg_oob_is_noop() {
    let mut g = grid();
    g.overlay_bg(99, 99, Color::Green);
    assert_eq!(g.get_cell(0, 0), Some(&Cell::default()));
}

#[test]
fn fill_paints_rect() {
    let mut g = grid();
    let rect = Rect {
        x: 1,
        y: 1,
        width: 3,
        height: 2,
    };
    // Disambiguate: `g.fill(rect, ...)` would call CellCapability::fill
    // (whole-grid, 2 args) — we want RenderSurface::fill (rect, 3 args).
    RenderSurface::fill(&mut g, rect, '#', Style::new().fg(Color::Red));
    for y in 1..3 {
        for x in 1..4 {
            let c = g.get_cell(x, y).unwrap();
            assert_eq!(c.ch, '#');
            assert_eq!(c.style.fg, Some(CellColor::Named(9)));
        }
    }
    // Outside rect — untouched.
    assert_eq!(g.get_cell(0, 0), Some(&Cell::default()));
    assert_eq!(g.get_cell(5, 3), Some(&Cell::default()));
}

#[test]
fn fill_partially_oob_clamps_via_write_cell() {
    let mut g = grid();
    // Rect extends past the 10x5 grid; OOB cells silently noop.
    let rect = Rect {
        x: 8,
        y: 3,
        width: 5,
        height: 5,
    };
    RenderSurface::fill(&mut g, rect, '@', Style::new());
    // In-bounds cells filled.
    assert_eq!(g.get_cell(8, 3).map(|c| c.ch), Some('@'));
    assert_eq!(g.get_cell(9, 4).map(|c| c.ch), Some('@'));
    // OOB: no panic.
}

#[test]
fn fill_with_zero_dim_rect_is_noop() {
    // Plan 21 landing telemetry P1 fold (Plan 22 Phase B): both
    // zero-width and zero-height rects skip every cell.
    let mut g = grid();
    let before: Vec<_> = g.iter().map(|((x, y), c)| (x, y, c.clone())).collect();

    RenderSurface::fill(
        &mut g,
        Rect {
            x: 5,
            y: 2,
            width: 0,
            height: 3,
        },
        'X',
        Style::new(),
    );
    RenderSurface::fill(
        &mut g,
        Rect {
            x: 5,
            y: 2,
            width: 3,
            height: 0,
        },
        'Y',
        Style::new(),
    );

    let after: Vec<_> = g.iter().map(|((x, y), c)| (x, y, c.clone())).collect();
    assert_eq!(before, after, "zero-dim rect fill must be a noop");
}

#[test]
fn clear_resets_rect_to_default() {
    let mut g = grid();
    RenderSurface::fill(
        &mut g,
        Rect {
            x: 0,
            y: 0,
            width: 10,
            height: 5,
        },
        '*',
        Style::new().fg(Color::Red),
    );
    RenderSurface::clear(
        &mut g,
        Rect {
            x: 2,
            y: 1,
            width: 3,
            height: 1,
        },
    );
    // Cleared cells back to default (space + default style).
    for x in 2..5 {
        let c = g.get_cell(x, 1).unwrap();
        assert_eq!(c.ch, ' ');
        assert_eq!(c.style, CellStyle::default());
    }
    // Outside the clear rect — still '*'.
    assert_eq!(g.get_cell(0, 0).map(|c| c.ch), Some('*'));
}

fn takes_surface(s: &mut dyn RenderSurface) {
    s.write_styled(0, 0, "ok", Style::new());
}

#[test]
fn cellcapability_is_usable_as_dyn_rendersurface() {
    // The core contract: a CellCapability can be passed anywhere a
    // `&mut dyn RenderSurface` is expected. 17-β.2b-impl-b consumers
    // rely on this.
    let mut g = grid();
    takes_surface(&mut g);
    assert_eq!(g.get_cell(0, 0).map(|c| c.ch), Some('o'));
    assert_eq!(g.get_cell(1, 0).map(|c| c.ch), Some('k'));
}
