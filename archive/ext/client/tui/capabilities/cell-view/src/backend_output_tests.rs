//! Tests for `backend_output::BackendRasterOutput` and the
//! `convert_cell_style_to_display_style` helper.
//!
//! The adapter is the seam between capability-layer styling
//! (`CellStyle` / `CellColor` / `CellAttrs`) and display-driver styling
//! (`Style` / `Color` / `Attributes`). Coverage targets every branch of
//! the conversion helper plus the two behavioural contracts on the
//! adapter itself: `set_cell` forwards to the backend with the
//! converted style; `size` returns the **terminal** dimensions, never a
//! doubled logical size.

use {
    reovim_driver_display::{Attributes, Color, Style, render_backend::RenderBackend},
    reovim_ext_client_tui_cap_cell::{CellAttrs, CellColor, CellStyle},
};

use crate::{
    backend_output::{BackendRasterOutput, convert_cell_style_to_display_style},
    raster::{RasterCell, RasterOutput},
};

/// Minimal `RenderBackend` that records every `set_cell` call and a
/// fixed `size`. Only `set_cell` and `size` are exercised by the
/// adapter; the other trait methods are `unreachable!()` guards.
struct FakeBackend {
    size: (u16, u16),
    writes: Vec<(u16, u16, char, Style)>,
}

impl FakeBackend {
    fn new(w: u16, h: u16) -> Self {
        Self {
            size: (w, h),
            writes: Vec::new(),
        }
    }
}

impl RenderBackend for FakeBackend {
    fn set_cell(&mut self, x: u16, y: u16, ch: char, style: &Style) {
        self.writes.push((x, y, ch, style.clone()));
    }
    fn apply_style(&mut self, _x: u16, _y: u16, _style: &Style) {
        unreachable!("adapter should not call apply_style")
    }
    fn write_str(&mut self, _x: u16, _y: u16, _text: &str, _style: &Style) -> u16 {
        unreachable!("adapter should not call write_str")
    }
    fn size(&self) -> (u16, u16) {
        self.size
    }
    fn clear(&mut self) {
        unreachable!("adapter should not call clear")
    }
    fn overlay_bg(&mut self, _x: u16, _y: u16, _bg: Color) {
        unreachable!("adapter should not call overlay_bg")
    }
}

// ============================================================================
// BackendRasterOutput
// ============================================================================

#[test]
fn set_cell_writes_char_and_converted_style() {
    let mut backend = FakeBackend::new(80, 24);
    let mut out = BackendRasterOutput::new(&mut backend);
    let style = CellStyle::plain()
        .with_fg(CellColor::Rgb(10, 20, 30))
        .with_bg(CellColor::Ansi256(7))
        .with_attrs(CellAttrs::BOLD);
    out.set_cell(RasterCell {
        x: 3,
        y: 5,
        ch: '▀',
        style,
    });

    assert_eq!(backend.writes.len(), 1);
    let (x, y, ch, got) = &backend.writes[0];
    assert_eq!((*x, *y, *ch), (3, 5, '▀'));
    assert_eq!(
        got.fg,
        Some(Color::Rgb {
            r: 10,
            g: 20,
            b: 30
        })
    );
    assert_eq!(got.bg, Some(Color::AnsiValue(7)));
    assert!(got.attributes.contains(Attributes::BOLD));
}

/// `size()` MUST return the terminal's physical dimensions. If it ever
/// returned the HalfBlock-doubled logical height the rasterizer's
/// out-of-bounds clamp (`h_term = (grid.height() / 2).min(h_out)`)
/// would degenerate to a no-op and write past the bottom of the screen.
#[test]
fn set_cell_delegates_size_returns_terminal_size() {
    let mut backend = FakeBackend::new(120, 40);
    let out = BackendRasterOutput::new(&mut backend);
    assert_eq!(out.size(), (120, 40));
}

// ============================================================================
// convert_cell_style_to_display_style — color mapping
// ============================================================================

#[test]
fn convert_style_maps_rgb_fg_and_bg() {
    let style = CellStyle::plain()
        .with_fg(CellColor::Rgb(1, 2, 3))
        .with_bg(CellColor::Rgb(4, 5, 6));
    let out = convert_cell_style_to_display_style(&style);
    assert_eq!(out.fg, Some(Color::Rgb { r: 1, g: 2, b: 3 }));
    assert_eq!(out.bg, Some(Color::Rgb { r: 4, g: 5, b: 6 }));
}

#[test]
fn convert_style_maps_ansi256() {
    let style = CellStyle::plain()
        .with_fg(CellColor::Ansi256(42))
        .with_bg(CellColor::Ansi256(7));
    let out = convert_cell_style_to_display_style(&style);
    assert_eq!(out.fg, Some(Color::AnsiValue(42)));
    assert_eq!(out.bg, Some(Color::AnsiValue(7)));
}

#[test]
fn convert_style_maps_named_to_ansi_value() {
    let style = CellStyle::plain()
        .with_fg(CellColor::Named(1))
        .with_bg(CellColor::Named(15));
    let out = convert_cell_style_to_display_style(&style);
    assert_eq!(out.fg, Some(Color::AnsiValue(1)));
    assert_eq!(out.bg, Some(Color::AnsiValue(15)));
}

#[test]
fn convert_style_maps_default_to_reset() {
    let style = CellStyle::plain()
        .with_fg(CellColor::Default)
        .with_bg(CellColor::Default);
    let out = convert_cell_style_to_display_style(&style);
    assert_eq!(out.fg, Some(Color::Reset));
    assert_eq!(out.bg, Some(Color::Reset));
}

#[test]
fn convert_style_none_fg_bg_stays_none() {
    let style = CellStyle::plain();
    let out = convert_cell_style_to_display_style(&style);
    assert!(out.fg.is_none());
    assert!(out.bg.is_none());
}

// ============================================================================
// convert_cell_style_to_display_style — attribute mapping
// ============================================================================

#[test]
fn convert_style_attrs_empty() {
    let style = CellStyle::plain();
    let out = convert_cell_style_to_display_style(&style);
    assert!(!out.attributes.contains(Attributes::BOLD));
    assert!(!out.attributes.contains(Attributes::ITALIC));
    assert!(!out.attributes.contains(Attributes::UNDERLINE));
    assert!(!out.attributes.contains(Attributes::REVERSE));
    assert!(!out.attributes.contains(Attributes::DIM));
}

#[test]
fn convert_style_attrs_all_five_flags() {
    let all = CellAttrs::BOLD
        | CellAttrs::ITALIC
        | CellAttrs::UNDERLINE
        | CellAttrs::REVERSE
        | CellAttrs::DIM;
    let style = CellStyle::plain().with_attrs(all);
    let out = convert_cell_style_to_display_style(&style);
    assert!(out.attributes.contains(Attributes::BOLD));
    assert!(out.attributes.contains(Attributes::ITALIC));
    assert!(out.attributes.contains(Attributes::UNDERLINE));
    assert!(out.attributes.contains(Attributes::REVERSE));
    assert!(out.attributes.contains(Attributes::DIM));
}

#[test]
fn convert_style_attrs_bold_only_does_not_set_italic() {
    let style = CellStyle::plain().with_attrs(CellAttrs::BOLD);
    let out = convert_cell_style_to_display_style(&style);
    assert!(out.attributes.contains(Attributes::BOLD));
    assert!(!out.attributes.contains(Attributes::ITALIC));
    assert!(!out.attributes.contains(Attributes::UNDERLINE));
    assert!(!out.attributes.contains(Attributes::REVERSE));
    assert!(!out.attributes.contains(Attributes::DIM));
}
