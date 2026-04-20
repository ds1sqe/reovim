use {super::*, crate::Attributes};

fn bold() -> Attributes {
    let mut a = Attributes::new();
    a.set(Attributes::BOLD);
    a
}

fn italic() -> Attributes {
    let mut a = Attributes::new();
    a.set(Attributes::ITALIC);
    a
}

#[test]
fn test_lerp_color_rgb_full_opacity() {
    let from = Color::Black;
    let to = Color::Rgb {
        r: 200,
        g: 100,
        b: 50,
    };
    let result = lerp_color(from, to, 1.0);
    assert_eq!(
        result,
        Color::Rgb {
            r: 200,
            g: 100,
            b: 50
        }
    );
}

#[test]
fn test_lerp_color_rgb_zero_opacity() {
    let from = Color::Black;
    let to = Color::Rgb {
        r: 200,
        g: 100,
        b: 50,
    };
    let result = lerp_color(from, to, 0.0);
    assert_eq!(result, Color::Black);
}

#[test]
fn test_lerp_color_rgb_midpoint() {
    let from = Color::Rgb { r: 0, g: 0, b: 0 };
    let to = Color::Rgb {
        r: 200,
        g: 100,
        b: 50,
    };
    let result = lerp_color(from, to, 0.5);
    assert_eq!(
        result,
        Color::Rgb {
            r: 100,
            g: 50,
            b: 25
        }
    );
}

#[test]
fn test_lerp_color_ansi_named() {
    let from = Color::Black;
    let to = Color::White;
    let result = lerp_color(from, to, 0.5);
    // Black (0,0,0) -> White (255,255,255) at 0.5 = (127,127,127)
    assert_eq!(
        result,
        Color::Rgb {
            r: 127,
            g: 127,
            b: 127
        }
    );
}

#[test]
fn test_lerp_color_clamps_t() {
    let from = Color::Black;
    let to = Color::White;

    // t > 1.0 should clamp to 1.0
    let result = lerp_color(from, to, 2.0);
    assert_eq!(result, Color::White);

    // t < 0.0 should clamp to 0.0
    let result = lerp_color(from, to, -1.0);
    assert_eq!(result, Color::Black);
}

#[test]
fn test_lerp_color_reset() {
    let from = Color::Reset;
    let to = Color::Rgb {
        r: 100,
        g: 100,
        b: 100,
    };
    let result = lerp_color(from, to, 0.5);
    assert_eq!(
        result,
        Color::Rgb {
            r: 50,
            g: 50,
            b: 50
        }
    );
}

#[test]
fn test_lerp_color_ansi_value() {
    let from = Color::AnsiValue(42);
    let to = Color::Rgb {
        r: 100,
        g: 100,
        b: 100,
    };
    // AnsiValue is approximated as black for blending
    let result = lerp_color(from, to, 0.5);
    assert_eq!(
        result,
        Color::Rgb {
            r: 50,
            g: 50,
            b: 50
        }
    );
}

#[test]
fn test_dim_style_full_opacity() {
    let style = Style {
        fg: Some(Color::Red),
        bg: Some(Color::Blue),
        attributes: bold(),
        underline_color: None,
    };
    let result = dim_style(&style, 1.0, Color::Black);
    assert_eq!(result.fg, style.fg);
    assert_eq!(result.bg, style.bg);
    assert_eq!(result.attributes, bold());
}

#[test]
fn test_dim_style_zero_opacity() {
    let style = Style {
        fg: Some(Color::White),
        bg: Some(Color::Blue),
        attributes: italic(),
        underline_color: Some(Color::Red),
    };
    let bg = Color::Black;
    let result = dim_style(&style, 0.0, bg);

    // All colors should be background (black)
    assert_eq!(result.fg, Some(Color::Black));
    assert_eq!(result.bg, Some(Color::Black));
    assert_eq!(result.underline_color, Some(Color::Black));
    // Attributes preserved
    assert_eq!(result.attributes, italic());
}

#[test]
fn test_dim_style_half_opacity() {
    let style = Style {
        fg: Some(Color::Rgb {
            r: 200,
            g: 100,
            b: 50,
        }),
        bg: None,
        attributes: Attributes::new(),
        underline_color: None,
    };
    let bg = Color::Rgb { r: 0, g: 0, b: 0 };
    let result = dim_style(&style, 0.5, bg);

    assert_eq!(
        result.fg,
        Some(Color::Rgb {
            r: 100,
            g: 50,
            b: 25
        })
    );
    assert_eq!(result.bg, None);
}

#[test]
fn test_dim_style_preserves_none_colors() {
    let style = Style {
        fg: None,
        bg: None,
        attributes: Attributes::new(),
        underline_color: None,
    };
    let result = dim_style(&style, 0.5, Color::Black);

    assert_eq!(result.fg, None);
    assert_eq!(result.bg, None);
    assert_eq!(result.underline_color, None);
}

#[test]
fn test_lerp_color_dark_ansi_colors() {
    // At t=0.5, ANSI colors get converted to RGB via approximation
    let colors = [
        Color::DarkRed,
        Color::DarkGreen,
        Color::DarkYellow,
        Color::DarkBlue,
        Color::DarkMagenta,
        Color::DarkCyan,
        Color::DarkGrey,
        Color::Grey,
    ];

    for color in &colors {
        let result = lerp_color(Color::Black, *color, 0.5);
        assert!(matches!(result, Color::Rgb { .. }), "color {color:?}");
    }
}

#[test]
fn test_lerp_color_bright_ansi_colors() {
    let colors = [
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
    ];

    for color in &colors {
        let result = lerp_color(Color::Black, *color, 0.5);
        assert!(matches!(result, Color::Rgb { .. }), "color {color:?}");
    }
}
