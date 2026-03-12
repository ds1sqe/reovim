use super::*;

#[test]
fn test_colormode_parse() {
    assert_eq!(ColorMode::parse("ansi"), Some(ColorMode::Ansi16));
    assert_eq!(ColorMode::parse("256"), Some(ColorMode::Color256));
    assert_eq!(ColorMode::parse("truecolor"), Some(ColorMode::TrueColor));
    assert_eq!(ColorMode::parse("invalid"), None);
}

#[test]
fn test_style_builder() {
    let style = Style::new().fg(Color::Red).bold().underline();
    assert_eq!(style.fg, Some(Color::Red));
    assert!(style.attributes.contains(Attributes::BOLD));
    assert!(style.attributes.contains(Attributes::UNDERLINE));
}

#[test]
fn test_style_merge() {
    let base = Style::new().fg(Color::Red).bold();
    let overlay = Style::new().bg(Color::Blue).italic();
    let merged = base.merge(&overlay);

    assert_eq!(merged.fg, Some(Color::Red));
    assert_eq!(merged.bg, Some(Color::Blue));
    assert!(merged.attributes.contains(Attributes::BOLD));
    assert!(merged.attributes.contains(Attributes::ITALIC));
}

#[test]
fn test_downgrade_truecolor() {
    let rgb = Color::Rgb { r: 255, g: 0, b: 0 };
    assert_eq!(downgrade_color(rgb, ColorMode::TrueColor), rgb);
}

#[test]
fn test_downgrade_to_256() {
    let rgb = Color::Rgb { r: 255, g: 0, b: 0 };
    let converted = downgrade_color(rgb, ColorMode::Color256);
    assert!(matches!(converted, Color::AnsiValue(_)));
}

// =========================================================================
// Attributes FromStr / Display tests
// =========================================================================

#[test]
fn test_attributes_from_str_single() {
    let attrs: Attributes = "bold".parse().unwrap();
    assert!(attrs.contains(Attributes::BOLD));
    assert!(!attrs.contains(Attributes::ITALIC));
}

#[test]
fn test_attributes_from_str_multiple() {
    let attrs: Attributes = "bold,italic,underline".parse().unwrap();
    assert!(attrs.contains(Attributes::BOLD));
    assert!(attrs.contains(Attributes::ITALIC));
    assert!(attrs.contains(Attributes::UNDERLINE));
    assert!(!attrs.contains(Attributes::REVERSE));
}

#[test]
fn test_attributes_from_str_case_insensitive() {
    let attrs: Attributes = "BOLD,Italic,UNDERLINE".parse().unwrap();
    assert!(attrs.contains(Attributes::BOLD));
    assert!(attrs.contains(Attributes::ITALIC));
    assert!(attrs.contains(Attributes::UNDERLINE));
}

#[test]
fn test_attributes_from_str_with_spaces() {
    let attrs: Attributes = "bold , italic , underline".parse().unwrap();
    assert!(attrs.contains(Attributes::BOLD));
    assert!(attrs.contains(Attributes::ITALIC));
    assert!(attrs.contains(Attributes::UNDERLINE));
}

#[test]
fn test_attributes_from_str_extended() {
    let attrs: Attributes = "curly_underline,overline,hidden".parse().unwrap();
    assert!(attrs.contains(Attributes::CURLY_UNDERLINE));
    assert!(attrs.contains(Attributes::OVERLINE));
    assert!(attrs.contains(Attributes::HIDDEN));
}

#[test]
fn test_attributes_from_str_unknown_ignored() {
    let attrs: Attributes = "bold,foobar,italic".parse().unwrap();
    assert!(attrs.contains(Attributes::BOLD));
    assert!(attrs.contains(Attributes::ITALIC));
}

#[test]
fn test_attributes_display_single() {
    let mut attrs = Attributes::new();
    attrs.set(Attributes::BOLD);
    assert_eq!(attrs.to_string(), "bold");
}

#[test]
fn test_attributes_display_multiple() {
    let mut attrs = Attributes::new();
    attrs.set(Attributes::BOLD);
    attrs.set(Attributes::ITALIC);
    attrs.set(Attributes::UNDERLINE);
    assert_eq!(attrs.to_string(), "bold,italic,underline");
}

#[test]
fn test_attributes_display_empty() {
    let attrs = Attributes::new();
    assert_eq!(attrs.to_string(), "");
}

#[test]
fn test_attributes_roundtrip() {
    // Test that parse(to_string()) == original
    let mut original = Attributes::new();
    original.set(Attributes::BOLD);
    original.set(Attributes::ITALIC);
    original.set(Attributes::CURLY_UNDERLINE);

    let s = original.to_string();
    let parsed: Attributes = s.parse().unwrap();
    assert_eq!(parsed.0, original.0);
}

// =========================================================================
// Style::from_wire tests
// =========================================================================

#[test]
fn test_style_from_wire_full() {
    let style = Style::from_wire(Some("red"), Some("#000000"), Some("bold,italic"));
    assert_eq!(style.fg, Some(Color::Red));
    assert_eq!(style.bg, Some(Color::Rgb { r: 0, g: 0, b: 0 }));
    assert!(style.attributes.contains(Attributes::BOLD));
    assert!(style.attributes.contains(Attributes::ITALIC));
}

#[test]
fn test_style_from_wire_colors_only() {
    let style = Style::from_wire(Some("blue"), Some("ansi:196"), None);
    assert_eq!(style.fg, Some(Color::Blue));
    assert_eq!(style.bg, Some(Color::AnsiValue(196)));
    assert_eq!(style.attributes.0, 0);
}

#[test]
fn test_style_from_wire_none() {
    let style = Style::from_wire(None, None, None);
    assert!(style.fg.is_none());
    assert!(style.bg.is_none());
    assert_eq!(style.attributes.0, 0);
}

#[test]
fn test_style_from_wire_default_color() {
    // "default" means no color (uses terminal default)
    let style = Style::from_wire(Some("default"), Some("default"), None);
    assert!(style.fg.is_none());
    assert!(style.bg.is_none());
}

#[test]
fn test_style_from_wire_hex_colors() {
    let style = Style::from_wire(Some("#ff5500"), Some("#003366"), None);
    assert_eq!(
        style.fg,
        Some(Color::Rgb {
            r: 255,
            g: 85,
            b: 0
        })
    );
    assert_eq!(
        style.bg,
        Some(Color::Rgb {
            r: 0,
            g: 51,
            b: 102
        })
    );
}

// =========================================================================
// Extended Attributes tests
// =========================================================================

#[test]
fn test_attributes_from_str_all_standard() {
    let attrs: Attributes = "bold,italic,underline,strikethrough,reverse,blink,dim"
        .parse()
        .unwrap();
    assert!(attrs.contains(Attributes::BOLD));
    assert!(attrs.contains(Attributes::ITALIC));
    assert!(attrs.contains(Attributes::UNDERLINE));
    assert!(attrs.contains(Attributes::STRIKETHROUGH));
    assert!(attrs.contains(Attributes::REVERSE));
    assert!(attrs.contains(Attributes::BLINK));
    assert!(attrs.contains(Attributes::DIM));
}

#[test]
fn test_attributes_from_str_all_extended() {
    let attrs: Attributes =
        "double_underline,curly_underline,dotted_underline,dashed_underline,overline,hidden"
            .parse()
            .unwrap();
    assert!(attrs.contains(Attributes::DOUBLE_UNDERLINE));
    assert!(attrs.contains(Attributes::CURLY_UNDERLINE));
    assert!(attrs.contains(Attributes::DOTTED_UNDERLINE));
    assert!(attrs.contains(Attributes::DASHED_UNDERLINE));
    assert!(attrs.contains(Attributes::OVERLINE));
    assert!(attrs.contains(Attributes::HIDDEN));
}

#[test]
fn test_attributes_display_extended() {
    let mut attrs = Attributes::new();
    attrs.set(Attributes::DOUBLE_UNDERLINE);
    attrs.set(Attributes::OVERLINE);
    attrs.set(Attributes::HIDDEN);
    assert_eq!(attrs.to_string(), "double_underline,overline,hidden");
}

#[test]
fn test_attributes_display_all_standard() {
    let mut attrs = Attributes::new();
    attrs.set(Attributes::BOLD);
    attrs.set(Attributes::ITALIC);
    attrs.set(Attributes::UNDERLINE);
    attrs.set(Attributes::STRIKETHROUGH);
    attrs.set(Attributes::REVERSE);
    attrs.set(Attributes::BLINK);
    attrs.set(Attributes::DIM);
    assert_eq!(attrs.to_string(), "bold,italic,underline,strikethrough,reverse,blink,dim");
}

#[test]
fn test_attributes_has_any_underline() {
    let mut attrs = Attributes::new();
    assert!(!attrs.has_any_underline());

    attrs.set(Attributes::UNDERLINE);
    assert!(attrs.has_any_underline());

    let mut attrs2 = Attributes::new();
    attrs2.set(Attributes::DOUBLE_UNDERLINE);
    assert!(attrs2.has_any_underline());

    let mut attrs3 = Attributes::new();
    attrs3.set(Attributes::CURLY_UNDERLINE);
    assert!(attrs3.has_any_underline());

    let mut attrs4 = Attributes::new();
    attrs4.set(Attributes::DOTTED_UNDERLINE);
    assert!(attrs4.has_any_underline());

    let mut attrs5 = Attributes::new();
    attrs5.set(Attributes::DASHED_UNDERLINE);
    assert!(attrs5.has_any_underline());
}

#[test]
fn test_attributes_union() {
    let mut a = Attributes::new();
    a.set(Attributes::BOLD);
    let mut b = Attributes::new();
    b.set(Attributes::ITALIC);
    let c = a.union(b);
    assert!(c.contains(Attributes::BOLD));
    assert!(c.contains(Attributes::ITALIC));
}

// =========================================================================
// Style builder tests for extended attributes
// =========================================================================

#[test]
fn test_style_builder_all_attributes() {
    let style = Style::new()
        .bold()
        .italic()
        .underline()
        .strikethrough()
        .reverse()
        .blink()
        .dim()
        .double_underline()
        .curly_underline()
        .dotted_underline()
        .dashed_underline()
        .overline()
        .hidden();

    assert!(style.attributes.contains(Attributes::BOLD));
    assert!(style.attributes.contains(Attributes::ITALIC));
    assert!(style.attributes.contains(Attributes::UNDERLINE));
    assert!(style.attributes.contains(Attributes::STRIKETHROUGH));
    assert!(style.attributes.contains(Attributes::REVERSE));
    assert!(style.attributes.contains(Attributes::BLINK));
    assert!(style.attributes.contains(Attributes::DIM));
    assert!(style.attributes.contains(Attributes::DOUBLE_UNDERLINE));
    assert!(style.attributes.contains(Attributes::CURLY_UNDERLINE));
    assert!(style.attributes.contains(Attributes::DOTTED_UNDERLINE));
    assert!(style.attributes.contains(Attributes::DASHED_UNDERLINE));
    assert!(style.attributes.contains(Attributes::OVERLINE));
    assert!(style.attributes.contains(Attributes::HIDDEN));
}

#[test]
fn test_style_underline_color() {
    let style = Style::new().underline_color(Color::Red);
    assert_eq!(style.underline_color, Some(Color::Red));
}

#[test]
fn test_style_merge_underline_color() {
    let base = Style::new().underline_color(Color::Red);
    let overlay = Style::new().underline_color(Color::Blue);
    let merged = base.merge(&overlay);
    assert_eq!(merged.underline_color, Some(Color::Blue));

    // overlay without underline_color should keep base
    let overlay2 = Style::new();
    let merged2 = base.merge(&overlay2);
    assert_eq!(merged2.underline_color, Some(Color::Red));
}

#[test]
fn test_style_merge_fg_override() {
    let base = Style::new().fg(Color::Red);
    let overlay = Style::new().fg(Color::Blue);
    let merged = base.merge(&overlay);
    assert_eq!(merged.fg, Some(Color::Blue));
}

// =========================================================================
// ANSI output tests
// =========================================================================

#[test]
fn test_ansi_reset() {
    assert_eq!(Style::ansi_reset(), "\x1b[0m");
}

#[test]
fn test_style_to_ansi_empty() {
    let style = Style::new();
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    // Empty style with no fg, no bg should have the bg reset code "49"
    assert!(ansi.contains("49"));
}

#[test]
fn test_style_to_ansi_fg_bg() {
    let style = Style::new()
        .fg(Color::Rgb { r: 255, g: 0, b: 0 })
        .bg(Color::Rgb { r: 0, g: 0, b: 255 });
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(ansi.contains("38;2;255;0;0")); // fg RGB
    assert!(ansi.contains("48;2;0;0;255")); // bg RGB
}

#[test]
fn test_style_to_ansi_bold_italic() {
    let style = Style::new().bold().italic();
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(ansi.contains('1')); // bold
    assert!(ansi.contains('3')); // italic
}

#[test]
fn test_style_to_ansi_all_standard_attrs() {
    let style = Style::new()
        .bold()
        .dim()
        .italic()
        .reverse()
        .hidden()
        .strikethrough()
        .blink();
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(ansi.contains('1')); // bold
    assert!(ansi.contains('2')); // dim
    assert!(ansi.contains('3')); // italic
    assert!(ansi.contains('7')); // reverse
    assert!(ansi.contains('8')); // hidden
    assert!(ansi.contains('9')); // strikethrough
    assert!(ansi.contains('5')); // blink
}

#[test]
fn test_style_to_ansi_curly_underline() {
    let style = Style::new().curly_underline();
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(ansi.contains("4:3"));
}

#[test]
fn test_style_to_ansi_dotted_underline() {
    let style = Style::new().dotted_underline();
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(ansi.contains("4:4"));
}

#[test]
fn test_style_to_ansi_dashed_underline() {
    let style = Style::new().dashed_underline();
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(ansi.contains("4:5"));
}

#[test]
fn test_style_to_ansi_double_underline() {
    let style = Style::new().double_underline();
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(ansi.contains("21"));
}

#[test]
fn test_style_to_ansi_plain_underline() {
    let style = Style::new().underline();
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(ansi.contains('4'));
}

#[test]
fn test_style_to_ansi_overline() {
    let style = Style::new().overline();
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(ansi.contains("53"));
}

#[test]
fn test_style_to_ansi_underline_color() {
    let style = Style::new().underline().underline_color(Color::Rgb {
        r: 255,
        g: 128,
        b: 0,
    });
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(ansi.contains("58;2;255;128;0"));
}

#[test]
fn test_style_to_ansi_underline_color_ansi256() {
    let style = Style::new()
        .underline()
        .underline_color(Color::AnsiValue(196));
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(ansi.contains("58;5;196"));
}

#[test]
fn test_style_to_ansi_underline_color_named() {
    let style = Style::new().underline().underline_color(Color::Red);
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    // Named colors get converted via named_color_to_ansi_index
    assert!(ansi.contains("58;5;9")); // Red = index 9
}

#[test]
fn test_style_to_ansi_underline_color_reset() {
    let style = Style::new().underline().underline_color(Color::Reset);
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(ansi.contains("59")); // underline color reset
}

#[test]
fn test_style_to_ansi_no_underline_color_without_underline() {
    // Underline color should NOT be emitted if no underline attribute is set
    let style = Style::new().underline_color(Color::Red);
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(!ansi.contains("58"));
}

#[test]
fn test_style_to_ansi_named_fg_colors() {
    let colors = [
        (Color::Reset, "39"),
        (Color::Black, "30"),
        (Color::DarkGrey, "90"),
        (Color::Red, "31"),
        (Color::DarkRed, "91"),
        (Color::Green, "32"),
        (Color::DarkGreen, "92"),
        (Color::Yellow, "33"),
        (Color::DarkYellow, "93"),
        (Color::Blue, "34"),
        (Color::DarkBlue, "94"),
        (Color::Magenta, "35"),
        (Color::DarkMagenta, "95"),
        (Color::Cyan, "36"),
        (Color::DarkCyan, "96"),
        (Color::White, "37"),
        (Color::Grey, "97"),
    ];

    for (color, expected_code) in colors {
        let style = Style::new().fg(color);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(
            ansi.contains(expected_code),
            "fg color {color:?} should produce code {expected_code}, got: {ansi}"
        );
    }
}

#[test]
fn test_style_to_ansi_named_bg_colors() {
    let colors = [
        (Color::Reset, "49"),
        (Color::Black, "40"),
        (Color::DarkGrey, "100"),
        (Color::Red, "41"),
        (Color::DarkRed, "101"),
        (Color::Green, "42"),
        (Color::DarkGreen, "102"),
        (Color::Yellow, "43"),
        (Color::DarkYellow, "103"),
        (Color::Blue, "44"),
        (Color::DarkBlue, "104"),
        (Color::Magenta, "45"),
        (Color::DarkMagenta, "105"),
        (Color::Cyan, "46"),
        (Color::DarkCyan, "106"),
        (Color::White, "47"),
        (Color::Grey, "107"),
    ];

    for (color, expected_code) in colors {
        let style = Style::new().bg(color);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(
            ansi.contains(expected_code),
            "bg color {color:?} should produce code {expected_code}, got: {ansi}"
        );
    }
}

#[test]
fn test_style_to_ansi_ansi256_fg() {
    let style = Style::new().fg(Color::AnsiValue(196));
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(ansi.contains("38;5;196"));
}

#[test]
fn test_style_to_ansi_ansi256_bg() {
    let style = Style::new().bg(Color::AnsiValue(52));
    let ansi = style.to_ansi_start(ColorMode::TrueColor);
    assert!(ansi.contains("48;5;52"));
}

// =========================================================================
// Color downgrade tests
// =========================================================================

#[test]
fn test_downgrade_rgb_to_ansi16() {
    let rgb = Color::Rgb { r: 255, g: 0, b: 0 };
    let converted = downgrade_color(rgb, ColorMode::Ansi16);
    // Pure red should map to Red (index 9)
    assert_eq!(converted, Color::Red);
}

#[test]
fn test_downgrade_ansi256_to_ansi16() {
    // Index 0-15 should map to named colors
    let c = downgrade_color(Color::AnsiValue(0), ColorMode::Ansi16);
    assert_eq!(c, Color::Black);

    let c = downgrade_color(Color::AnsiValue(15), ColorMode::Ansi16);
    assert_eq!(c, Color::White);

    // Color cube (16-231) should map to nearest ANSI 16
    let c = downgrade_color(Color::AnsiValue(196), ColorMode::Ansi16);
    // 196 is bright red in 6x6x6 cube
    assert_eq!(c, Color::Red);

    // Grayscale (232-255) should map to nearest ANSI 16
    let c = downgrade_color(Color::AnsiValue(232), ColorMode::Ansi16);
    // 232 is very dark gray -> should map to Black
    assert_eq!(c, Color::Black);

    let c = downgrade_color(Color::AnsiValue(255), ColorMode::Ansi16);
    // 255 is near-white gray -> should map to White
    assert_eq!(c, Color::White);
}

#[test]
fn test_downgrade_named_to_ansi16_noop() {
    // Named colors should pass through unchanged
    assert_eq!(downgrade_color(Color::Red, ColorMode::Ansi16), Color::Red);
}

#[test]
fn test_downgrade_noop_for_same_or_higher_mode() {
    let ansi = Color::AnsiValue(100);
    // AnsiValue to 256 should be noop
    assert_eq!(downgrade_color(ansi, ColorMode::Color256), ansi);
    // AnsiValue to TrueColor should be noop
    assert_eq!(downgrade_color(ansi, ColorMode::TrueColor), ansi);
    // Named color stays as-is
    assert_eq!(downgrade_color(Color::Red, ColorMode::TrueColor), Color::Red);
}

// =========================================================================
// rgb_to_ansi256 tests
// =========================================================================

#[test]
fn test_rgb_to_ansi256_grayscale() {
    // Pure black -> 16
    assert_eq!(rgb_to_ansi256(0, 0, 0), 16);
    // Near-white -> 231
    assert_eq!(rgb_to_ansi256(255, 255, 255), 231);
    // Mid gray -> grayscale range
    let mid = rgb_to_ansi256(128, 128, 128);
    assert!(mid >= 232);
}

#[test]
fn test_rgb_to_ansi256_color_cube() {
    // Pure red -> color cube
    let red = rgb_to_ansi256(255, 0, 0);
    assert!((16..232).contains(&red));
}

// =========================================================================
// ColorMode parse extended
// =========================================================================

#[test]
fn test_colormode_parse_all_variants() {
    assert_eq!(ColorMode::parse("ansi16"), Some(ColorMode::Ansi16));
    assert_eq!(ColorMode::parse("16"), Some(ColorMode::Ansi16));
    assert_eq!(ColorMode::parse("256color"), Some(ColorMode::Color256));
    assert_eq!(ColorMode::parse("true"), Some(ColorMode::TrueColor));
    assert_eq!(ColorMode::parse("rgb"), Some(ColorMode::TrueColor));
    assert_eq!(ColorMode::parse("24bit"), Some(ColorMode::TrueColor));
    assert_eq!(ColorMode::parse("TRUECOLOR"), Some(ColorMode::TrueColor));
}

#[test]
fn test_colormode_default() {
    assert_eq!(ColorMode::default(), ColorMode::TrueColor);
}

// =========================================================================
// Style::from_wire extended tests
// =========================================================================

#[test]
fn test_style_from_wire_attrs_only() {
    let style = Style::from_wire(None, None, Some("bold,underline"));
    assert!(style.fg.is_none());
    assert!(style.bg.is_none());
    assert!(style.attributes.contains(Attributes::BOLD));
    assert!(style.attributes.contains(Attributes::UNDERLINE));
}

#[test]
fn test_style_to_ansi_256_mode_downgrade() {
    let style = Style::new()
        .fg(Color::Rgb { r: 255, g: 0, b: 0 })
        .bg(Color::Rgb { r: 0, g: 255, b: 0 });
    let ansi = style.to_ansi_start(ColorMode::Color256);
    // In 256 mode, RGB colors are downgraded to 256 palette
    assert!(ansi.contains("38;5;")); // fg 256-color
    assert!(ansi.contains("48;5;")); // bg 256-color
}

#[test]
fn test_named_color_to_ansi_index() {
    assert_eq!(named_color_to_ansi_index(&Color::Black), 0);
    assert_eq!(named_color_to_ansi_index(&Color::DarkRed), 1);
    assert_eq!(named_color_to_ansi_index(&Color::DarkGreen), 2);
    assert_eq!(named_color_to_ansi_index(&Color::DarkYellow), 3);
    assert_eq!(named_color_to_ansi_index(&Color::DarkBlue), 4);
    assert_eq!(named_color_to_ansi_index(&Color::DarkMagenta), 5);
    assert_eq!(named_color_to_ansi_index(&Color::DarkCyan), 6);
    assert_eq!(named_color_to_ansi_index(&Color::Grey), 7);
    assert_eq!(named_color_to_ansi_index(&Color::DarkGrey), 8);
    assert_eq!(named_color_to_ansi_index(&Color::Red), 9);
    assert_eq!(named_color_to_ansi_index(&Color::Green), 10);
    assert_eq!(named_color_to_ansi_index(&Color::Yellow), 11);
    assert_eq!(named_color_to_ansi_index(&Color::Blue), 12);
    assert_eq!(named_color_to_ansi_index(&Color::Magenta), 13);
    assert_eq!(named_color_to_ansi_index(&Color::Cyan), 14);
    assert_eq!(named_color_to_ansi_index(&Color::White), 15);
    assert_eq!(named_color_to_ansi_index(&Color::Reset), 0);
}

// =========================================================================
// named_color_to_ansi_index: AnsiValue passthrough (line 718)
// =========================================================================

#[test]
fn test_named_color_to_ansi_index_ansi_value() {
    assert_eq!(named_color_to_ansi_index(&Color::AnsiValue(42)), 42);
    assert_eq!(named_color_to_ansi_index(&Color::AnsiValue(0)), 0);
    assert_eq!(named_color_to_ansi_index(&Color::AnsiValue(255)), 255);
}

// =========================================================================
// Attributes Display: dotted_underline and dashed_underline (lines 227, 230)
// =========================================================================

#[test]
fn test_attributes_display_dotted_underline() {
    let mut attrs = Attributes::new();
    attrs.set(Attributes::DOTTED_UNDERLINE);
    assert_eq!(attrs.to_string(), "dotted_underline");
}

#[test]
fn test_attributes_display_dashed_underline() {
    let mut attrs = Attributes::new();
    attrs.set(Attributes::DASHED_UNDERLINE);
    assert_eq!(attrs.to_string(), "dashed_underline");
}

#[test]
fn test_attributes_display_all_extended() {
    let mut attrs = Attributes::new();
    attrs.set(Attributes::DOUBLE_UNDERLINE);
    attrs.set(Attributes::CURLY_UNDERLINE);
    attrs.set(Attributes::DOTTED_UNDERLINE);
    attrs.set(Attributes::DASHED_UNDERLINE);
    attrs.set(Attributes::OVERLINE);
    attrs.set(Attributes::HIDDEN);
    assert_eq!(
        attrs.to_string(),
        "double_underline,curly_underline,dotted_underline,dashed_underline,overline,hidden"
    );
}

// =========================================================================
// rgb_to_ansi256: to_cube returning 1 for v in [48, 115) (line 541)
// =========================================================================

#[test]
fn test_rgb_to_ansi256_cube_index_1() {
    // r=80 is in [48, 115) so to_cube(80) = 1, g=0 and b=0 are < 48 so to_cube = 0
    // Not grayscale because r != g
    // Result: 16 + 36*1 + 6*0 + 0 = 52
    assert_eq!(rgb_to_ansi256(80, 0, 0), 52);
}

#[test]
fn test_rgb_to_ansi256_cube_index_1_green() {
    // g=100 is in [48, 115), r and b are different
    // to_cube(0) = 0, to_cube(100) = 1, to_cube(0) = 0
    // Result: 16 + 36*0 + 6*1 + 0 = 22
    assert_eq!(rgb_to_ansi256(0, 100, 0), 22);
}

#[test]
fn test_rgb_to_ansi256_cube_index_1_blue() {
    // b=60 is in [48, 115)
    // to_cube(0) = 0, to_cube(0) = 0, to_cube(60) = 1
    // Result: 16 + 36*0 + 6*0 + 1 = 17
    assert_eq!(rgb_to_ansi256(0, 0, 60), 17);
}

// =========================================================================
// ansi_index_to_color: all 16 indices (lines 614-629)
// =========================================================================

#[test]
fn test_ansi256_to_ansi16_all_basic_indices() {
    // Exercise ansi256_to_ansi16 with indices 0-15 which directly call
    // ansi_index_to_color, covering all match arms in lines 612-631.
    let expected = [
        (0, Color::Black),
        (1, Color::DarkRed),
        (2, Color::DarkGreen),
        (3, Color::DarkYellow),
        (4, Color::DarkBlue),
        (5, Color::DarkMagenta),
        (6, Color::DarkCyan),
        (7, Color::Grey),
        (8, Color::DarkGrey),
        (9, Color::Red),
        (10, Color::Green),
        (11, Color::Yellow),
        (12, Color::Blue),
        (13, Color::Magenta),
        (14, Color::Cyan),
        (15, Color::White),
    ];

    for (idx, expected_color) in expected {
        let result = ansi256_to_ansi16(idx);
        assert_eq!(
            result, expected_color,
            "ansi256_to_ansi16({idx}) should be {expected_color:?}, got {result:?}"
        );
    }
}

#[test]
fn test_ansi_index_to_color_out_of_range() {
    // Index >= 16 should map to Color::Reset (line 629)
    assert_eq!(ansi_index_to_color(16), Color::Reset);
    assert_eq!(ansi_index_to_color(255), Color::Reset);
}

// =========================================================================
// ColorMode::detect_from_env() (covers lines 43-44, 47-48, 50-51, 54)
// =========================================================================

#[test]
fn test_colormode_detect_truecolor() {
    assert_eq!(ColorMode::detect_from_env(Some("truecolor"), None), ColorMode::TrueColor,);
}

#[test]
fn test_colormode_detect_truecolor_uppercase() {
    assert_eq!(ColorMode::detect_from_env(Some("TRUECOLOR"), None), ColorMode::TrueColor,);
}

#[test]
fn test_colormode_detect_24bit() {
    assert_eq!(ColorMode::detect_from_env(Some("24bit"), None), ColorMode::TrueColor,);
}

#[test]
fn test_colormode_detect_256color() {
    assert_eq!(ColorMode::detect_from_env(None, Some("xterm-256color")), ColorMode::Color256,);
}

#[test]
fn test_colormode_detect_256color_screen() {
    assert_eq!(ColorMode::detect_from_env(None, Some("screen-256color")), ColorMode::Color256,);
}

#[test]
fn test_colormode_detect_ansi16_fallback() {
    assert_eq!(ColorMode::detect_from_env(None, Some("xterm")), ColorMode::Ansi16,);
}

#[test]
fn test_colormode_detect_no_env_vars() {
    assert_eq!(ColorMode::detect_from_env(None, None), ColorMode::Ansi16,);
}

#[test]
fn test_colormode_detect_colorterm_non_truecolor_with_256_term() {
    // COLORTERM set but not "truecolor" or "24bit" -> falls through to TERM check
    assert_eq!(
        ColorMode::detect_from_env(Some("something-else"), Some("xterm-256color")),
        ColorMode::Color256,
    );
}

#[test]
fn test_colormode_detect_colorterm_non_truecolor_no_term() {
    // COLORTERM set to unrecognized value, no TERM -> fallback to Ansi16
    assert_eq!(ColorMode::detect_from_env(Some("something-else"), None), ColorMode::Ansi16,);
}

#[test]
fn test_colormode_detect_truecolor_overrides_256_term() {
    // COLORTERM=truecolor should take precedence even if TERM has 256color
    assert_eq!(
        ColorMode::detect_from_env(Some("truecolor"), Some("xterm-256color")),
        ColorMode::TrueColor,
    );
}

// =========================================================================
// to_ansi_start: empty output path (line 447)
// =========================================================================
//
// Line 447 (String::new() when both codes and owned_codes are empty) is
// unreachable with the current logic: when bg is None, "49" is always
// pushed to codes; when bg is Some, the bg ANSI code is pushed to
// owned_codes. So at least one collection always has an element.
// No test needed for dead code.

// =========================================================================
// Underline color with named color via underline_ansi (exercises
// named_color_to_ansi_index through color_to_underline_ansi)
// =========================================================================

#[test]
fn test_underline_color_named_all_variants() {
    // Test that all named colors produce valid underline ANSI via
    // the named_color_to_ansi_index -> format!("58;5;{ansi}") path
    let named_colors = [
        (Color::DarkRed, "58;5;1"),
        (Color::DarkGreen, "58;5;2"),
        (Color::DarkYellow, "58;5;3"),
        (Color::DarkBlue, "58;5;4"),
        (Color::DarkMagenta, "58;5;5"),
        (Color::DarkCyan, "58;5;6"),
        (Color::Grey, "58;5;7"),
        (Color::DarkGrey, "58;5;8"),
        (Color::Green, "58;5;10"),
        (Color::Yellow, "58;5;11"),
        (Color::Blue, "58;5;12"),
        (Color::Magenta, "58;5;13"),
        (Color::Cyan, "58;5;14"),
        (Color::White, "58;5;15"),
        (Color::Black, "58;5;0"),
    ];

    for (color, expected_code) in named_colors {
        let style = Style::new().underline().underline_color(color);
        let ansi = style.to_ansi_start(ColorMode::TrueColor);
        assert!(
            ansi.contains(expected_code),
            "underline color {color:?} should produce {expected_code}, got: {ansi}"
        );
    }
}

#[test]
fn test_color_to_fg_ansi_direct_all() {
    assert_eq!(color_to_fg_ansi(&Color::Reset), "39");
    assert_eq!(color_to_fg_ansi(&Color::Black), "30");
    assert_eq!(color_to_fg_ansi(&Color::DarkGrey), "90");
    assert_eq!(color_to_fg_ansi(&Color::Red), "31");
    assert_eq!(color_to_fg_ansi(&Color::DarkRed), "91");
    assert_eq!(color_to_fg_ansi(&Color::Green), "32");
    assert_eq!(color_to_fg_ansi(&Color::DarkGreen), "92");
    assert_eq!(color_to_fg_ansi(&Color::Yellow), "33");
    assert_eq!(color_to_fg_ansi(&Color::DarkYellow), "93");
    assert_eq!(color_to_fg_ansi(&Color::Blue), "34");
    assert_eq!(color_to_fg_ansi(&Color::DarkBlue), "94");
    assert_eq!(color_to_fg_ansi(&Color::Magenta), "35");
    assert_eq!(color_to_fg_ansi(&Color::DarkMagenta), "95");
    assert_eq!(color_to_fg_ansi(&Color::Cyan), "36");
    assert_eq!(color_to_fg_ansi(&Color::DarkCyan), "96");
    assert_eq!(color_to_fg_ansi(&Color::White), "37");
    assert_eq!(color_to_fg_ansi(&Color::Grey), "97");
    assert_eq!(color_to_fg_ansi(&Color::AnsiValue(196)), "38;5;196");
    assert_eq!(
        color_to_fg_ansi(&Color::Rgb {
            r: 10,
            g: 20,
            b: 30
        }),
        "38;2;10;20;30"
    );
}

#[test]
fn test_color_to_bg_ansi_direct_all() {
    assert_eq!(color_to_bg_ansi(&Color::Reset), "49");
    assert_eq!(color_to_bg_ansi(&Color::Black), "40");
    assert_eq!(color_to_bg_ansi(&Color::DarkGrey), "100");
    assert_eq!(color_to_bg_ansi(&Color::Red), "41");
    assert_eq!(color_to_bg_ansi(&Color::DarkRed), "101");
    assert_eq!(color_to_bg_ansi(&Color::Green), "42");
    assert_eq!(color_to_bg_ansi(&Color::DarkGreen), "102");
    assert_eq!(color_to_bg_ansi(&Color::Yellow), "43");
    assert_eq!(color_to_bg_ansi(&Color::DarkYellow), "103");
    assert_eq!(color_to_bg_ansi(&Color::Blue), "44");
    assert_eq!(color_to_bg_ansi(&Color::DarkBlue), "104");
    assert_eq!(color_to_bg_ansi(&Color::Magenta), "45");
    assert_eq!(color_to_bg_ansi(&Color::DarkMagenta), "105");
    assert_eq!(color_to_bg_ansi(&Color::Cyan), "46");
    assert_eq!(color_to_bg_ansi(&Color::DarkCyan), "106");
    assert_eq!(color_to_bg_ansi(&Color::White), "47");
    assert_eq!(color_to_bg_ansi(&Color::Grey), "107");
    assert_eq!(color_to_bg_ansi(&Color::AnsiValue(52)), "48;5;52");
    assert_eq!(
        color_to_bg_ansi(&Color::Rgb {
            r: 10,
            g: 20,
            b: 30
        }),
        "48;2;10;20;30"
    );
}

#[test]
fn test_color_to_underline_ansi_direct_all() {
    assert_eq!(color_to_underline_ansi(&Color::Reset), "59");
    assert_eq!(color_to_underline_ansi(&Color::AnsiValue(42)), "58;5;42");
    assert_eq!(color_to_underline_ansi(&Color::Rgb { r: 1, g: 2, b: 3 }), "58;2;1;2;3");
    assert_eq!(color_to_underline_ansi(&Color::Black), "58;5;0");
    assert_eq!(color_to_underline_ansi(&Color::DarkRed), "58;5;1");
    assert_eq!(color_to_underline_ansi(&Color::DarkGreen), "58;5;2");
    assert_eq!(color_to_underline_ansi(&Color::DarkYellow), "58;5;3");
    assert_eq!(color_to_underline_ansi(&Color::DarkBlue), "58;5;4");
    assert_eq!(color_to_underline_ansi(&Color::DarkMagenta), "58;5;5");
    assert_eq!(color_to_underline_ansi(&Color::DarkCyan), "58;5;6");
    assert_eq!(color_to_underline_ansi(&Color::Grey), "58;5;7");
    assert_eq!(color_to_underline_ansi(&Color::DarkGrey), "58;5;8");
    assert_eq!(color_to_underline_ansi(&Color::Red), "58;5;9");
    assert_eq!(color_to_underline_ansi(&Color::Green), "58;5;10");
    assert_eq!(color_to_underline_ansi(&Color::Yellow), "58;5;11");
    assert_eq!(color_to_underline_ansi(&Color::Blue), "58;5;12");
    assert_eq!(color_to_underline_ansi(&Color::Magenta), "58;5;13");
    assert_eq!(color_to_underline_ansi(&Color::Cyan), "58;5;14");
    assert_eq!(color_to_underline_ansi(&Color::White), "58;5;15");
}

#[test]
fn test_ansi_index_to_color_direct_all() {
    assert_eq!(ansi_index_to_color(0), Color::Black);
    assert_eq!(ansi_index_to_color(1), Color::DarkRed);
    assert_eq!(ansi_index_to_color(2), Color::DarkGreen);
    assert_eq!(ansi_index_to_color(3), Color::DarkYellow);
    assert_eq!(ansi_index_to_color(4), Color::DarkBlue);
    assert_eq!(ansi_index_to_color(5), Color::DarkMagenta);
    assert_eq!(ansi_index_to_color(6), Color::DarkCyan);
    assert_eq!(ansi_index_to_color(7), Color::Grey);
    assert_eq!(ansi_index_to_color(8), Color::DarkGrey);
    assert_eq!(ansi_index_to_color(9), Color::Red);
    assert_eq!(ansi_index_to_color(10), Color::Green);
    assert_eq!(ansi_index_to_color(11), Color::Yellow);
    assert_eq!(ansi_index_to_color(12), Color::Blue);
    assert_eq!(ansi_index_to_color(13), Color::Magenta);
    assert_eq!(ansi_index_to_color(14), Color::Cyan);
    assert_eq!(ansi_index_to_color(15), Color::White);
}

#[test]
fn test_ansi256_to_ansi16_grayscale_direct() {
    let dark = ansi256_to_ansi16(232);
    assert_eq!(dark, Color::Black);
    let light = ansi256_to_ansi16(255);
    assert_eq!(light, Color::White);
    let mid = ansi256_to_ansi16(244);
    assert!(
        matches!(mid, Color::Grey | Color::DarkGrey | Color::White),
        "mid-grayscale {mid:?} should be a grey/white variant"
    );
}

#[test]
fn test_ansi256_to_ansi16_color_cube_direct() {
    let red = ansi256_to_ansi16(196);
    assert_eq!(red, Color::Red);
    let green = ansi256_to_ansi16(46);
    assert_eq!(green, Color::Green);
    let blue = ansi256_to_ansi16(21);
    assert_eq!(blue, Color::Blue);
}

#[test]
fn test_rgb_to_nearest_ansi16_all_exact() {
    for (idx, &(r, g, b)) in ANSI16_RGB.iter().enumerate() {
        let result = rgb_to_nearest_ansi16(r, g, b);
        #[allow(clippy::cast_possible_truncation)]
        let expected = ansi_index_to_color(idx as u8);
        assert_eq!(
            result, expected,
            "RGB ({r},{g},{b}) at index {idx} should map to {expected:?}, got {result:?}"
        );
    }
}

#[test]
fn test_attributes_from_str_infallible() {
    let result: Result<Attributes, std::convert::Infallible> = "bold".parse();
    assert!(result.is_ok());
    let attrs = result.unwrap();
    assert!(attrs.contains(Attributes::BOLD));
}

#[test]
fn test_style_struct_fields_default() {
    let style = Style::default();
    assert_eq!(style.attributes, Attributes::default());
    assert!(style.underline_color.is_none());
}
