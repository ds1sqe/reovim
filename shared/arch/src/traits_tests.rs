use super::*;

// =========================================================================
// Color Parsing Tests
// =========================================================================

#[test]
fn test_color_parse_named() {
    assert_eq!("red".parse::<Color>().unwrap(), Color::Red);
    assert_eq!("RED".parse::<Color>().unwrap(), Color::Red);
    assert_eq!("Red".parse::<Color>().unwrap(), Color::Red);
    assert_eq!("green".parse::<Color>().unwrap(), Color::Green);
    assert_eq!("blue".parse::<Color>().unwrap(), Color::Blue);
    assert_eq!("yellow".parse::<Color>().unwrap(), Color::Yellow);
    assert_eq!("magenta".parse::<Color>().unwrap(), Color::Magenta);
    assert_eq!("cyan".parse::<Color>().unwrap(), Color::Cyan);
    assert_eq!("white".parse::<Color>().unwrap(), Color::White);
    assert_eq!("black".parse::<Color>().unwrap(), Color::Black);
    assert_eq!("grey".parse::<Color>().unwrap(), Color::Grey);
    assert_eq!("gray".parse::<Color>().unwrap(), Color::Grey);
    assert_eq!("darkred".parse::<Color>().unwrap(), Color::DarkRed);
    assert_eq!("darkgrey".parse::<Color>().unwrap(), Color::DarkGrey);
    assert_eq!("darkgray".parse::<Color>().unwrap(), Color::DarkGrey);
}

#[test]
fn test_color_parse_default() {
    assert_eq!("default".parse::<Color>().unwrap(), Color::Reset);
    assert_eq!("reset".parse::<Color>().unwrap(), Color::Reset);
}

#[test]
fn test_color_parse_hex() {
    assert_eq!("#ff0000".parse::<Color>().unwrap(), Color::Rgb { r: 255, g: 0, b: 0 });
    assert_eq!("#00ff00".parse::<Color>().unwrap(), Color::Rgb { r: 0, g: 255, b: 0 });
    assert_eq!("#0000ff".parse::<Color>().unwrap(), Color::Rgb { r: 0, g: 0, b: 255 });
    assert_eq!(
        "#abcdef".parse::<Color>().unwrap(),
        Color::Rgb {
            r: 171,
            g: 205,
            b: 239
        }
    );
}

#[test]
fn test_color_parse_hex_short() {
    // #rgb expands to #rrggbb
    assert_eq!("#f00".parse::<Color>().unwrap(), Color::Rgb { r: 255, g: 0, b: 0 });
    assert_eq!("#0f0".parse::<Color>().unwrap(), Color::Rgb { r: 0, g: 255, b: 0 });
    assert_eq!(
        "#abc".parse::<Color>().unwrap(),
        Color::Rgb {
            r: 170,
            g: 187,
            b: 204
        }
    );
}

#[test]
fn test_color_parse_ansi() {
    assert_eq!("ansi:196".parse::<Color>().unwrap(), Color::AnsiValue(196));
    assert_eq!("ansi:0".parse::<Color>().unwrap(), Color::AnsiValue(0));
    assert_eq!("ansi:255".parse::<Color>().unwrap(), Color::AnsiValue(255));
    assert_eq!("ANSI:100".parse::<Color>().unwrap(), Color::AnsiValue(100));
}

#[test]
fn test_color_parse_rgb_func() {
    assert_eq!("rgb(255,0,0)".parse::<Color>().unwrap(), Color::Rgb { r: 255, g: 0, b: 0 });
    assert_eq!("rgb(0, 255, 0)".parse::<Color>().unwrap(), Color::Rgb { r: 0, g: 255, b: 0 });
    assert_eq!(
        "RGB(100,150,200)".parse::<Color>().unwrap(),
        Color::Rgb {
            r: 100,
            g: 150,
            b: 200
        }
    );
}

#[test]
fn test_color_parse_errors() {
    assert!("#ff00".parse::<Color>().is_err()); // Wrong length
    assert!("#gggggg".parse::<Color>().is_err()); // Invalid hex
    assert!("ansi:300".parse::<Color>().is_err()); // Out of range
    assert!("unknown".parse::<Color>().is_err()); // Unknown name
    assert!("rgb(1,2)".parse::<Color>().is_err()); // Missing component
}

#[test]
fn test_color_display() {
    assert_eq!(Color::Red.to_string(), "red");
    assert_eq!(Color::DarkBlue.to_string(), "darkblue");
    assert_eq!(Color::Reset.to_string(), "default");
    assert_eq!(Color::Rgb { r: 255, g: 0, b: 0 }.to_string(), "#ff0000");
    assert_eq!(
        Color::Rgb {
            r: 171,
            g: 205,
            b: 239
        }
        .to_string(),
        "#abcdef"
    );
    assert_eq!(Color::AnsiValue(196).to_string(), "ansi:196");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_color_roundtrip() {
    // All named colors should roundtrip
    let named_colors = [
        Color::Reset,
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::White,
        Color::Grey,
        Color::DarkGrey,
        Color::DarkRed,
        Color::DarkGreen,
        Color::DarkYellow,
        Color::DarkBlue,
        Color::DarkMagenta,
        Color::DarkCyan,
    ];
    for color in named_colors {
        let s = color.to_string();
        let parsed: Color = s.parse().unwrap();
        assert_eq!(parsed, color, "roundtrip failed for {color:?}");
    }

    // RGB colors should roundtrip
    let rgb_colors = [
        Color::Rgb { r: 255, g: 0, b: 0 },
        Color::Rgb { r: 0, g: 255, b: 0 },
        Color::Rgb { r: 0, g: 0, b: 255 },
        Color::Rgb {
            r: 171,
            g: 205,
            b: 239,
        },
    ];
    for color in rgb_colors {
        let s = color.to_string();
        let parsed: Color = s.parse().unwrap();
        assert_eq!(parsed, color, "roundtrip failed for {color:?}");
    }

    // ANSI colors should roundtrip
    for n in [0, 15, 100, 196, 255] {
        let color = Color::AnsiValue(n);
        let s = color.to_string();
        let parsed: Color = s.parse().unwrap();
        assert_eq!(parsed, color, "roundtrip failed for {color:?}");
    }
}

#[test]
fn test_color_parse_convenience() {
    assert_eq!(Color::parse("red"), Some(Color::Red));
    assert_eq!(Color::parse("#ff0000"), Some(Color::Rgb { r: 255, g: 0, b: 0 }));
    assert_eq!(Color::parse("invalid"), None);
}

// =========================================================================
// Existing Tests
// =========================================================================

#[test]
fn test_modifiers_operations() {
    let shift = Modifiers::SHIFT;
    let ctrl = Modifiers::CTRL;
    let combined = shift | ctrl;

    assert!(combined.contains(Modifiers::SHIFT));
    assert!(combined.contains(Modifiers::CTRL));
    assert!(!combined.contains(Modifiers::ALT));
    assert!(!combined.is_empty());
    assert!(Modifiers::NONE.is_empty());
}

#[test]
fn test_terminal_size() {
    let size = TerminalSize::new(80, 24);
    assert_eq!(size.cols, 80);
    assert_eq!(size.rows, 24);
    assert_eq!(size.pixel_width, 0);
    assert_eq!(size.pixel_height, 0);

    let size_with_pixels = TerminalSize::with_pixels(80, 24, 800, 480);
    assert_eq!(size_with_pixels.pixel_width, 800);
    assert_eq!(size_with_pixels.pixel_height, 480);
}

#[test]
fn test_key_event_creation() {
    let key = KeyEvent::new(KeyCode::Char('a'));
    assert_eq!(key.code, KeyCode::Char('a'));
    assert_eq!(key.modifiers, Modifiers::NONE);
    assert_eq!(key.kind, KeyEventKind::Press);

    let ctrl_a = KeyEvent::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL);
    assert!(ctrl_a.modifiers.contains(Modifiers::CTRL));
}

#[test]
fn test_raw_mode_guard_disabled() {
    let guard = RawModeGuard::disabled();
    drop(guard); // Should not panic
}

// =========================================================================
// Color Default, Clone, Copy, Eq, Hash
// =========================================================================

#[test]
fn test_color_default() {
    let color: Color = Color::default();
    assert_eq!(color, Color::Reset);
}

#[test]
fn test_color_clone() {
    let color = Color::Rgb {
        r: 100,
        g: 200,
        b: 50,
    };
    let cloned = color;
    assert_eq!(color, cloned);
}

#[test]
fn test_color_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(Color::Red);
    set.insert(Color::Blue);
    set.insert(Color::Red); // duplicate
    assert_eq!(set.len(), 2);

    set.insert(Color::Rgb { r: 1, g: 2, b: 3 });
    set.insert(Color::Rgb { r: 1, g: 2, b: 3 }); // duplicate
    assert_eq!(set.len(), 3);

    set.insert(Color::AnsiValue(42));
    set.insert(Color::AnsiValue(42)); // duplicate
    assert_eq!(set.len(), 4);
}

#[test]
fn test_color_eq_and_ne() {
    assert_eq!(Color::Red, Color::Red);
    assert_ne!(Color::Red, Color::Blue);
    assert_eq!(Color::Rgb { r: 0, g: 0, b: 0 }, Color::Rgb { r: 0, g: 0, b: 0 });
    assert_ne!(Color::Rgb { r: 0, g: 0, b: 0 }, Color::Rgb { r: 0, g: 0, b: 1 });
    assert_ne!(Color::Black, Color::Rgb { r: 0, g: 0, b: 0 });
    assert_eq!(Color::AnsiValue(0), Color::AnsiValue(0));
    assert_ne!(Color::AnsiValue(0), Color::AnsiValue(1));
}

// =========================================================================
// Color Display - all named variants
// =========================================================================

#[test]
fn test_color_display_all_named() {
    assert_eq!(Color::Reset.to_string(), "default");
    assert_eq!(Color::Black.to_string(), "black");
    assert_eq!(Color::DarkRed.to_string(), "darkred");
    assert_eq!(Color::DarkGreen.to_string(), "darkgreen");
    assert_eq!(Color::DarkYellow.to_string(), "darkyellow");
    assert_eq!(Color::DarkBlue.to_string(), "darkblue");
    assert_eq!(Color::DarkMagenta.to_string(), "darkmagenta");
    assert_eq!(Color::DarkCyan.to_string(), "darkcyan");
    assert_eq!(Color::Grey.to_string(), "grey");
    assert_eq!(Color::DarkGrey.to_string(), "darkgrey");
    assert_eq!(Color::Red.to_string(), "red");
    assert_eq!(Color::Green.to_string(), "green");
    assert_eq!(Color::Yellow.to_string(), "yellow");
    assert_eq!(Color::Blue.to_string(), "blue");
    assert_eq!(Color::Magenta.to_string(), "magenta");
    assert_eq!(Color::Cyan.to_string(), "cyan");
    assert_eq!(Color::White.to_string(), "white");
}

#[test]
fn test_color_display_rgb_leading_zeros() {
    assert_eq!(Color::Rgb { r: 0, g: 0, b: 0 }.to_string(), "#000000");
    assert_eq!(Color::Rgb { r: 1, g: 2, b: 3 }.to_string(), "#010203");
}

#[test]
fn test_color_display_ansi_values() {
    assert_eq!(Color::AnsiValue(0).to_string(), "ansi:0");
    assert_eq!(Color::AnsiValue(128).to_string(), "ansi:128");
    assert_eq!(Color::AnsiValue(255).to_string(), "ansi:255");
}

// =========================================================================
// ParseColorError Display - all kinds
// =========================================================================

#[test]
fn test_parse_color_error_invalid_hex_length() {
    let err = ParseColorError {
        input: "#abcd".to_string(),
        kind: ParseColorErrorKind::InvalidHexLength,
    };
    let display = format!("{err}");
    assert!(display.contains("#abcd"));
    assert!(display.contains("#rgb or #rrggbb"));
}

#[test]
fn test_parse_color_error_invalid_hex_digit() {
    let err = ParseColorError {
        input: "#gggggg".to_string(),
        kind: ParseColorErrorKind::InvalidHexDigit,
    };
    let display = format!("{err}");
    assert!(display.contains("#gggggg"));
    assert!(display.contains("invalid hex digit"));
}

#[test]
fn test_parse_color_error_invalid_rgb_format() {
    let err = ParseColorError {
        input: "rgb(1,2)".to_string(),
        kind: ParseColorErrorKind::InvalidRgbFormat,
    };
    let display = format!("{err}");
    assert!(display.contains("rgb(1,2)"));
    assert!(display.contains("rgb(r,g,b)"));
}

#[test]
fn test_parse_color_error_invalid_ansi_index() {
    let err = ParseColorError {
        input: "ansi:300".to_string(),
        kind: ParseColorErrorKind::InvalidAnsiIndex,
    };
    let display = format!("{err}");
    assert!(display.contains("ansi:300"));
    assert!(display.contains("0-255"));
}

#[test]
fn test_parse_color_error_unknown_color_name() {
    let err = ParseColorError {
        input: "salmon".to_string(),
        kind: ParseColorErrorKind::UnknownColorName,
    };
    let display = format!("{err}");
    assert!(display.contains("salmon"));
    assert!(display.contains("unknown color name"));
}

#[test]
fn test_parse_color_error_debug() {
    let err = ParseColorError {
        input: "bad".to_string(),
        kind: ParseColorErrorKind::UnknownColorName,
    };
    let debug = format!("{err:?}");
    assert!(debug.contains("ParseColorError"));
    assert!(debug.contains("UnknownColorName"));
}

#[test]
fn test_parse_color_error_is_std_error() {
    let err = ParseColorError {
        input: "bad".to_string(),
        kind: ParseColorErrorKind::UnknownColorName,
    };
    let _: &dyn std::error::Error = &err;
}

#[test]
fn test_parse_color_error_clone_eq() {
    let err1 = ParseColorError {
        input: "bad".to_string(),
        kind: ParseColorErrorKind::UnknownColorName,
    };
    let err2 = err1.clone();
    assert_eq!(err1, err2);

    let err3 = ParseColorError {
        input: "bad".to_string(),
        kind: ParseColorErrorKind::InvalidHexLength,
    };
    assert_ne!(err1, err3);
}

#[test]
fn test_parse_color_error_kind_clone_copy_eq() {
    let kind = ParseColorErrorKind::InvalidHexDigit;
    let copied = kind;
    assert_eq!(kind, copied);

    let cloned = kind;
    assert_eq!(kind, cloned);
}

// =========================================================================
// Color parsing edge cases
// =========================================================================

#[test]
fn test_color_parse_all_named_dark_variants() {
    assert_eq!("darkgreen".parse::<Color>().unwrap(), Color::DarkGreen);
    assert_eq!("darkyellow".parse::<Color>().unwrap(), Color::DarkYellow);
    assert_eq!("darkblue".parse::<Color>().unwrap(), Color::DarkBlue);
    assert_eq!("darkmagenta".parse::<Color>().unwrap(), Color::DarkMagenta);
    assert_eq!("darkcyan".parse::<Color>().unwrap(), Color::DarkCyan);
}

#[test]
fn test_color_parse_case_insensitive_mixed() {
    assert_eq!("DarkGreen".parse::<Color>().unwrap(), Color::DarkGreen);
    assert_eq!("DARKBLUE".parse::<Color>().unwrap(), Color::DarkBlue);
    assert_eq!("DarkMagenta".parse::<Color>().unwrap(), Color::DarkMagenta);
    assert_eq!("DARKCYAN".parse::<Color>().unwrap(), Color::DarkCyan);
    assert_eq!("DARKYELLOW".parse::<Color>().unwrap(), Color::DarkYellow);
}

#[test]
fn test_color_parse_hex_invalid_length() {
    let err = "#a".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidHexLength);
    assert_eq!(err.input, "#a");

    let err = "#abcde".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidHexLength);

    let err = "#abcdefg".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidHexLength);
}

#[test]
fn test_color_parse_hex_invalid_digits_6char() {
    let err = "#zz0000".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidHexDigit);

    let err = "#00zz00".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidHexDigit);

    let err = "#0000zz".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidHexDigit);
}

#[test]
fn test_color_parse_hex_invalid_digits_3char() {
    let err = "#z00".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidHexDigit);

    let err = "#0z0".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidHexDigit);

    let err = "#00z".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidHexDigit);
}

#[test]
fn test_color_parse_ansi_invalid() {
    let err = "ansi:256".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidAnsiIndex);

    let err = "ansi:-1".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidAnsiIndex);

    let err = "ansi:abc".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidAnsiIndex);
}

#[test]
fn test_color_parse_rgb_func_too_few_parts() {
    let err = "rgb(1,2)".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidRgbFormat);
}

#[test]
fn test_color_parse_rgb_func_too_many_parts() {
    let err = "rgb(1,2,3,4)".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidRgbFormat);
}

#[test]
fn test_color_parse_rgb_func_invalid_values() {
    let err = "rgb(abc,0,0)".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidRgbFormat);

    let err = "rgb(0,abc,0)".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidRgbFormat);

    let err = "rgb(0,0,abc)".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidRgbFormat);
}

#[test]
fn test_color_parse_rgb_func_overflow() {
    let err = "rgb(256,0,0)".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::InvalidRgbFormat);
}

#[test]
fn test_color_parse_rgb_func_with_spaces() {
    assert_eq!(
        "rgb( 10 , 20 , 30 )".parse::<Color>().unwrap(),
        Color::Rgb {
            r: 10,
            g: 20,
            b: 30
        }
    );
}

#[test]
fn test_color_parse_convenience_returns_none_on_error() {
    assert!(Color::parse("").is_none());
    assert!(Color::parse("notacolor").is_none());
    assert!(Color::parse("#xyz").is_none());
}

#[test]
fn test_color_parse_empty_string() {
    let err = "".parse::<Color>().unwrap_err();
    assert_eq!(err.kind, ParseColorErrorKind::UnknownColorName);
}

// =========================================================================
// TerminalSize
// =========================================================================

#[test]
fn test_terminal_size_default() {
    let size = TerminalSize::default();
    assert_eq!(size.cols, 0);
    assert_eq!(size.rows, 0);
    assert_eq!(size.pixel_width, 0);
    assert_eq!(size.pixel_height, 0);
}

#[test]
fn test_terminal_size_new() {
    let size = TerminalSize::new(120, 40);
    assert_eq!(size.cols, 120);
    assert_eq!(size.rows, 40);
    assert_eq!(size.pixel_width, 0);
    assert_eq!(size.pixel_height, 0);
}

#[test]
fn test_terminal_size_with_pixels() {
    let size = TerminalSize::with_pixels(80, 24, 1920, 1080);
    assert_eq!(size.cols, 80);
    assert_eq!(size.rows, 24);
    assert_eq!(size.pixel_width, 1920);
    assert_eq!(size.pixel_height, 1080);
}

#[test]
fn test_terminal_size_eq_clone_copy() {
    let size1 = TerminalSize::new(80, 24);
    let size2 = size1; // Copy
    assert_eq!(size1, size2);

    let size3 = size1;
    assert_eq!(size1, size3);
}

#[test]
fn test_terminal_size_debug() {
    let size = TerminalSize::new(80, 24);
    let debug = format!("{size:?}");
    assert!(debug.contains("80"));
    assert!(debug.contains("24"));
}

// =========================================================================
// Modifiers
// =========================================================================

#[test]
fn test_modifiers_default() {
    let mods = Modifiers::default();
    assert!(mods.is_empty());
    assert_eq!(mods, Modifiers::NONE);
}

#[test]
fn test_modifiers_bits_round_trip() {
    let mods = Modifiers::SHIFT | Modifiers::ALT;
    let bits = mods.bits();
    let restored = Modifiers::from_bits(bits);
    assert_eq!(mods, restored);
}

#[test]
fn test_modifiers_union() {
    let combined = Modifiers::CTRL.union(Modifiers::SUPER);
    assert!(combined.contains(Modifiers::CTRL));
    assert!(combined.contains(Modifiers::SUPER));
    assert!(!combined.contains(Modifiers::SHIFT));
}

#[test]
fn test_modifiers_bitor_assign() {
    let mut mods = Modifiers::NONE;
    mods |= Modifiers::SHIFT;
    assert!(mods.contains(Modifiers::SHIFT));
    assert!(!mods.contains(Modifiers::CTRL));

    mods |= Modifiers::CTRL;
    assert!(mods.contains(Modifiers::SHIFT));
    assert!(mods.contains(Modifiers::CTRL));
}

#[test]
fn test_modifiers_individual_bits() {
    assert_eq!(Modifiers::NONE.bits(), 0);
    assert_eq!(Modifiers::SHIFT.bits(), 1);
    assert_eq!(Modifiers::CTRL.bits(), 2);
    assert_eq!(Modifiers::ALT.bits(), 4);
    assert_eq!(Modifiers::SUPER.bits(), 8);
    assert_eq!(Modifiers::HYPER.bits(), 16);
    assert_eq!(Modifiers::META.bits(), 32);
}

#[test]
fn test_modifiers_contains_none() {
    // NONE should be contained in everything (0 & 0 == 0)
    assert!(Modifiers::NONE.contains(Modifiers::NONE));
    assert!(Modifiers::SHIFT.contains(Modifiers::NONE));
}

#[test]
fn test_modifiers_all_combined() {
    let all = Modifiers::SHIFT
        | Modifiers::CTRL
        | Modifiers::ALT
        | Modifiers::SUPER
        | Modifiers::HYPER
        | Modifiers::META;
    assert!(all.contains(Modifiers::SHIFT));
    assert!(all.contains(Modifiers::CTRL));
    assert!(all.contains(Modifiers::ALT));
    assert!(all.contains(Modifiers::SUPER));
    assert!(all.contains(Modifiers::HYPER));
    assert!(all.contains(Modifiers::META));
    assert!(!all.is_empty());
}

#[test]
fn test_modifiers_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(Modifiers::SHIFT);
    set.insert(Modifiers::CTRL);
    set.insert(Modifiers::SHIFT); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn test_modifiers_debug() {
    let mods = Modifiers::SHIFT;
    let debug = format!("{mods:?}");
    assert!(debug.contains("Modifiers"));
}

// =========================================================================
// KeyEventState
// =========================================================================

#[test]
fn test_key_event_state_default() {
    let state = KeyEventState::default();
    assert!(state.is_empty());
    assert_eq!(state, KeyEventState::NONE);
}

#[test]
fn test_key_event_state_contains() {
    let state = KeyEventState::KEYPAD.union(KeyEventState::CAPS_LOCK);
    assert!(state.contains(KeyEventState::KEYPAD));
    assert!(state.contains(KeyEventState::CAPS_LOCK));
    assert!(!state.contains(KeyEventState::NUM_LOCK));
}

#[test]
fn test_key_event_state_union() {
    let state = KeyEventState::KEYPAD.union(KeyEventState::NUM_LOCK);
    assert!(state.contains(KeyEventState::KEYPAD));
    assert!(state.contains(KeyEventState::NUM_LOCK));
}

#[test]
fn test_key_event_state_is_empty() {
    assert!(KeyEventState::NONE.is_empty());
    assert!(!KeyEventState::KEYPAD.is_empty());
    assert!(!KeyEventState::CAPS_LOCK.is_empty());
    assert!(!KeyEventState::NUM_LOCK.is_empty());
}

#[test]
fn test_key_event_state_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(KeyEventState::NONE);
    set.insert(KeyEventState::KEYPAD);
    set.insert(KeyEventState::NONE); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn test_key_event_state_debug() {
    let state = KeyEventState::CAPS_LOCK;
    let debug = format!("{state:?}");
    assert!(debug.contains("KeyEventState"));
}

// =========================================================================
// KeyEventKind
// =========================================================================

#[test]
fn test_key_event_kind_default() {
    let kind = KeyEventKind::default();
    assert_eq!(kind, KeyEventKind::Press);
}

#[test]
fn test_key_event_kind_eq_clone_copy() {
    let kind = KeyEventKind::Repeat;
    let copied = kind;
    assert_eq!(kind, copied);

    let cloned = kind;
    assert_eq!(kind, cloned);
}

#[test]
fn test_key_event_kind_ne() {
    assert_ne!(KeyEventKind::Press, KeyEventKind::Repeat);
    assert_ne!(KeyEventKind::Press, KeyEventKind::Release);
    assert_ne!(KeyEventKind::Repeat, KeyEventKind::Release);
}

#[test]
fn test_key_event_kind_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(KeyEventKind::Press);
    set.insert(KeyEventKind::Repeat);
    set.insert(KeyEventKind::Release);
    set.insert(KeyEventKind::Press); // duplicate
    assert_eq!(set.len(), 3);
}

#[test]
fn test_key_event_kind_debug() {
    assert_eq!(format!("{:?}", KeyEventKind::Press), "Press");
    assert_eq!(format!("{:?}", KeyEventKind::Repeat), "Repeat");
    assert_eq!(format!("{:?}", KeyEventKind::Release), "Release");
}

// =========================================================================
// KeyCode
// =========================================================================

#[test]
fn test_key_code_char_eq() {
    assert_eq!(KeyCode::Char('a'), KeyCode::Char('a'));
    assert_ne!(KeyCode::Char('a'), KeyCode::Char('b'));
}

#[test]
fn test_key_code_f_keys() {
    assert_eq!(KeyCode::F(1), KeyCode::F(1));
    assert_ne!(KeyCode::F(1), KeyCode::F(2));
}

#[test]
fn test_key_code_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(KeyCode::Enter);
    set.insert(KeyCode::Escape);
    set.insert(KeyCode::Tab);
    set.insert(KeyCode::Enter); // duplicate
    assert_eq!(set.len(), 3);
}

#[test]
fn test_key_code_debug_variants() {
    let debug = format!("{:?}", KeyCode::Char('x'));
    assert!(debug.contains("Char"));
    assert!(debug.contains('x'));

    let debug = format!("{:?}", KeyCode::F(5));
    assert!(debug.contains("F(5)"));

    // Navigation keys
    assert!(format!("{:?}", KeyCode::Up).contains("Up"));
    assert!(format!("{:?}", KeyCode::Down).contains("Down"));
    assert!(format!("{:?}", KeyCode::Left).contains("Left"));
    assert!(format!("{:?}", KeyCode::Right).contains("Right"));
    assert!(format!("{:?}", KeyCode::Home).contains("Home"));
    assert!(format!("{:?}", KeyCode::End).contains("End"));
    assert!(format!("{:?}", KeyCode::PageUp).contains("PageUp"));
    assert!(format!("{:?}", KeyCode::PageDown).contains("PageDown"));

    // Editing keys
    assert!(format!("{:?}", KeyCode::Insert).contains("Insert"));
    assert!(format!("{:?}", KeyCode::Delete).contains("Delete"));
    assert!(format!("{:?}", KeyCode::Backspace).contains("Backspace"));

    // Special keys
    assert!(format!("{:?}", KeyCode::Null).contains("Null"));
    assert!(format!("{:?}", KeyCode::CapsLock).contains("CapsLock"));
    assert!(format!("{:?}", KeyCode::ScrollLock).contains("ScrollLock"));
    assert!(format!("{:?}", KeyCode::NumLock).contains("NumLock"));
    assert!(format!("{:?}", KeyCode::PrintScreen).contains("PrintScreen"));
    assert!(format!("{:?}", KeyCode::Pause).contains("Pause"));
    assert!(format!("{:?}", KeyCode::Menu).contains("Menu"));
    assert!(format!("{:?}", KeyCode::KeypadBegin).contains("KeypadBegin"));
    assert!(format!("{:?}", KeyCode::BackTab).contains("BackTab"));

    // Media keys
    assert!(format!("{:?}", KeyCode::MediaPlay).contains("MediaPlay"));
    assert!(format!("{:?}", KeyCode::MediaPause).contains("MediaPause"));
    assert!(format!("{:?}", KeyCode::MediaPlayPause).contains("MediaPlayPause"));
    assert!(format!("{:?}", KeyCode::MediaStop).contains("MediaStop"));
    assert!(format!("{:?}", KeyCode::MediaReverse).contains("MediaReverse"));
    assert!(format!("{:?}", KeyCode::MediaFastForward).contains("MediaFastForward"));
    assert!(format!("{:?}", KeyCode::MediaRewind).contains("MediaRewind"));
    assert!(format!("{:?}", KeyCode::MediaNext).contains("MediaNext"));
    assert!(format!("{:?}", KeyCode::MediaPrevious).contains("MediaPrevious"));
    assert!(format!("{:?}", KeyCode::MediaRecord).contains("MediaRecord"));
    assert!(format!("{:?}", KeyCode::MediaLowerVolume).contains("MediaLowerVolume"));
    assert!(format!("{:?}", KeyCode::MediaRaiseVolume).contains("MediaRaiseVolume"));
    assert!(format!("{:?}", KeyCode::MediaMuteVolume).contains("MediaMuteVolume"));

    // Modifier keys
    assert!(format!("{:?}", KeyCode::LeftShift).contains("LeftShift"));
    assert!(format!("{:?}", KeyCode::RightShift).contains("RightShift"));
    assert!(format!("{:?}", KeyCode::LeftCtrl).contains("LeftCtrl"));
    assert!(format!("{:?}", KeyCode::RightCtrl).contains("RightCtrl"));
    assert!(format!("{:?}", KeyCode::LeftAlt).contains("LeftAlt"));
    assert!(format!("{:?}", KeyCode::RightAlt).contains("RightAlt"));
    assert!(format!("{:?}", KeyCode::LeftSuper).contains("LeftSuper"));
    assert!(format!("{:?}", KeyCode::RightSuper).contains("RightSuper"));
    assert!(format!("{:?}", KeyCode::LeftHyper).contains("LeftHyper"));
    assert!(format!("{:?}", KeyCode::RightHyper).contains("RightHyper"));
    assert!(format!("{:?}", KeyCode::LeftMeta).contains("LeftMeta"));
    assert!(format!("{:?}", KeyCode::RightMeta).contains("RightMeta"));
    assert!(format!("{:?}", KeyCode::IsoLevel3Shift).contains("IsoLevel3Shift"));
    assert!(format!("{:?}", KeyCode::IsoLevel5Shift).contains("IsoLevel5Shift"));
}

// =========================================================================
// KeyEvent
// =========================================================================

#[test]
fn test_key_event_new_defaults() {
    let key = KeyEvent::new(KeyCode::Enter);
    assert_eq!(key.code, KeyCode::Enter);
    assert_eq!(key.modifiers, Modifiers::NONE);
    assert_eq!(key.kind, KeyEventKind::Press);
    assert_eq!(key.state, KeyEventState::NONE);
}

#[test]
fn test_key_event_with_modifiers() {
    let key = KeyEvent::with_modifiers(KeyCode::Char('c'), Modifiers::CTRL);
    assert_eq!(key.code, KeyCode::Char('c'));
    assert!(key.modifiers.contains(Modifiers::CTRL));
    assert_eq!(key.kind, KeyEventKind::Press);
    assert_eq!(key.state, KeyEventState::NONE);
}

#[test]
fn test_key_event_eq_hash() {
    use std::collections::HashSet;
    let key1 = KeyEvent::new(KeyCode::Char('a'));
    let key2 = KeyEvent::new(KeyCode::Char('a'));
    let key3 = KeyEvent::new(KeyCode::Char('b'));

    assert_eq!(key1, key2);
    assert_ne!(key1, key3);

    let mut set = HashSet::new();
    set.insert(key1);
    set.insert(key2); // duplicate
    set.insert(key3);
    assert_eq!(set.len(), 2);
}

#[test]
fn test_key_event_debug() {
    let key = KeyEvent::with_modifiers(KeyCode::Char('x'), Modifiers::CTRL | Modifiers::SHIFT);
    let debug = format!("{key:?}");
    assert!(debug.contains("KeyEvent"));
    assert!(debug.contains("Char"));
}

// =========================================================================
// MouseButton
// =========================================================================

#[test]
fn test_mouse_button_eq_clone_copy() {
    let btn = MouseButton::Left;
    let copied = btn;
    assert_eq!(btn, copied);

    let cloned = btn;
    assert_eq!(btn, cloned);

    assert_ne!(MouseButton::Left, MouseButton::Right);
    assert_ne!(MouseButton::Left, MouseButton::Middle);
    assert_ne!(MouseButton::Right, MouseButton::Middle);
}

#[test]
fn test_mouse_button_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(MouseButton::Left);
    set.insert(MouseButton::Right);
    set.insert(MouseButton::Middle);
    set.insert(MouseButton::Left); // duplicate
    assert_eq!(set.len(), 3);
}

#[test]
fn test_mouse_button_debug() {
    assert_eq!(format!("{:?}", MouseButton::Left), "Left");
    assert_eq!(format!("{:?}", MouseButton::Right), "Right");
    assert_eq!(format!("{:?}", MouseButton::Middle), "Middle");
}

// =========================================================================
// MouseEventKind
// =========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_mouse_event_kind_variants() {
    let kinds = [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Moved,
        MouseEventKind::ScrollUp,
        MouseEventKind::ScrollDown,
        MouseEventKind::ScrollLeft,
        MouseEventKind::ScrollRight,
    ];
    // Verify all variants are distinct
    for (i, k1) in kinds.iter().enumerate() {
        for (j, k2) in kinds.iter().enumerate() {
            if i == j {
                assert_eq!(k1, k2);
            } else {
                assert_ne!(k1, k2, "index {i} and {j} should differ");
            }
        }
    }
}

#[test]
fn test_mouse_event_kind_with_different_buttons() {
    assert_ne!(
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Down(MouseButton::Right)
    );
    assert_ne!(MouseEventKind::Up(MouseButton::Left), MouseEventKind::Up(MouseButton::Middle));
    assert_ne!(
        MouseEventKind::Drag(MouseButton::Right),
        MouseEventKind::Drag(MouseButton::Middle)
    );
}

#[test]
fn test_mouse_event_kind_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(MouseEventKind::ScrollUp);
    set.insert(MouseEventKind::ScrollDown);
    set.insert(MouseEventKind::ScrollUp); // duplicate
    assert_eq!(set.len(), 2);
}

#[test]
fn test_mouse_event_kind_debug() {
    let debug = format!("{:?}", MouseEventKind::Down(MouseButton::Left));
    assert!(debug.contains("Down"));
    assert!(debug.contains("Left"));

    let debug = format!("{:?}", MouseEventKind::Moved);
    assert!(debug.contains("Moved"));

    let debug = format!("{:?}", MouseEventKind::ScrollUp);
    assert!(debug.contains("ScrollUp"));
}

// =========================================================================
// MouseEvent
// =========================================================================

#[test]
fn test_mouse_event_creation() {
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 10,
        row: 20,
        modifiers: Modifiers::CTRL,
    };
    assert!(matches!(event.kind, MouseEventKind::Down(MouseButton::Left)));
    assert_eq!(event.column, 10);
    assert_eq!(event.row, 20);
    assert!(event.modifiers.contains(Modifiers::CTRL));
}

#[test]
fn test_mouse_event_eq_clone_copy() {
    let event1 = MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 5,
        row: 3,
        modifiers: Modifiers::NONE,
    };
    let event2 = event1; // Copy
    assert_eq!(event1, event2);

    let event3 = event1;
    assert_eq!(event1, event3);
}

#[test]
fn test_mouse_event_debug() {
    let event = MouseEvent {
        kind: MouseEventKind::Moved,
        column: 42,
        row: 7,
        modifiers: Modifiers::ALT,
    };
    let debug = format!("{event:?}");
    assert!(debug.contains("MouseEvent"));
    assert!(debug.contains("Moved"));
    assert!(debug.contains('4'));
    assert!(debug.contains('7'));
}

// =========================================================================
// InputEvent
// =========================================================================

#[test]
fn test_input_event_key() {
    let event = InputEvent::Key(KeyEvent::new(KeyCode::Char('a')));
    assert!(matches!(event, InputEvent::Key(_)));
}

#[test]
fn test_input_event_mouse() {
    let event = InputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 0,
        modifiers: Modifiers::NONE,
    });
    assert!(matches!(event, InputEvent::Mouse(_)));
}

#[test]
fn test_input_event_resize() {
    let event = InputEvent::Resize(TerminalSize::new(80, 24));
    assert!(matches!(event, InputEvent::Resize(_)));
}

#[test]
fn test_input_event_focus() {
    let gained = InputEvent::FocusGained;
    let lost = InputEvent::FocusLost;
    assert!(matches!(gained, InputEvent::FocusGained));
    assert!(matches!(lost, InputEvent::FocusLost));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_input_event_paste() {
    let event = InputEvent::Paste("hello world".to_string());
    if let InputEvent::Paste(text) = event {
        assert_eq!(text, "hello world");
    } else {
        panic!("Expected Paste variant");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_input_event_clone() {
    let event = InputEvent::Key(KeyEvent::new(KeyCode::Enter));
    let cloned = event;
    assert!(matches!(cloned, InputEvent::Key(_)));

    let paste = InputEvent::Paste("test".to_string());
    let paste_cloned = paste;
    if let InputEvent::Paste(text) = paste_cloned {
        assert_eq!(text, "test");
    } else {
        panic!("Expected Paste variant after clone");
    }
}

#[test]
fn test_input_event_debug() {
    let event = InputEvent::Key(KeyEvent::new(KeyCode::Escape));
    let debug = format!("{event:?}");
    assert!(debug.contains("Key"));

    let event = InputEvent::FocusGained;
    let debug = format!("{event:?}");
    assert!(debug.contains("FocusGained"));

    let event = InputEvent::FocusLost;
    let debug = format!("{event:?}");
    assert!(debug.contains("FocusLost"));

    let event = InputEvent::Paste("data".to_string());
    let debug = format!("{event:?}");
    assert!(debug.contains("Paste"));
}

// =========================================================================
// ClearType
// =========================================================================

#[test]
fn test_clear_type_eq_clone_copy() {
    let ct = ClearType::All;
    let copied = ct;
    assert_eq!(ct, copied);

    let cloned = ct;
    assert_eq!(ct, cloned);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_clear_type_all_variants() {
    let variants = [
        ClearType::All,
        ClearType::FromCursorDown,
        ClearType::FromCursorUp,
        ClearType::CurrentLine,
        ClearType::UntilNewLine,
        ClearType::Purge,
    ];
    for (i, v1) in variants.iter().enumerate() {
        for (j, v2) in variants.iter().enumerate() {
            if i == j {
                assert_eq!(v1, v2);
            } else {
                assert_ne!(v1, v2, "ClearType index {i} and {j} should differ");
            }
        }
    }
}

#[test]
fn test_clear_type_debug() {
    assert_eq!(format!("{:?}", ClearType::All), "All");
    assert_eq!(format!("{:?}", ClearType::FromCursorDown), "FromCursorDown");
    assert_eq!(format!("{:?}", ClearType::FromCursorUp), "FromCursorUp");
    assert_eq!(format!("{:?}", ClearType::CurrentLine), "CurrentLine");
    assert_eq!(format!("{:?}", ClearType::UntilNewLine), "UntilNewLine");
    assert_eq!(format!("{:?}", ClearType::Purge), "Purge");
}

// =========================================================================
// RawModeGuard
// =========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_raw_mode_guard_new_calls_restore_on_drop() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);

    let guard = RawModeGuard::new(move || {
        called_clone.store(true, Ordering::SeqCst);
        Ok(())
    });

    assert!(!called.load(Ordering::SeqCst));
    drop(guard);
    assert!(called.load(Ordering::SeqCst));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_raw_mode_guard_take_prevents_restore() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);

    let mut guard = RawModeGuard::new(move || {
        called_clone.store(true, Ordering::SeqCst);
        Ok(())
    });

    let taken = guard.take();
    assert!(taken.is_some());

    drop(guard);
    // Restore should NOT have been called since we took the function
    assert!(!called.load(Ordering::SeqCst));
}

#[test]
fn test_raw_mode_guard_take_returns_none_when_disabled() {
    let mut guard = RawModeGuard::disabled();
    let taken = guard.take();
    assert!(taken.is_none());
}

#[test]
fn test_raw_mode_guard_take_returns_none_after_first_take() {
    let mut guard = RawModeGuard::new(|| Ok(()));
    let first_take = guard.take();
    assert!(first_take.is_some());

    let second_take = guard.take();
    assert!(second_take.is_none());
}

#[test]
fn test_raw_mode_guard_restore_error_ignored_on_drop() {
    // Verify that errors from the restore function are silently ignored
    let guard = RawModeGuard::new(|| Err(io::Error::other("restore failed")));
    drop(guard); // Should not panic
}

// =========================================================================
// Terminal::write_str default method
// =========================================================================

struct MockTerminal {
    written: std::sync::Mutex<Vec<u8>>,
}

impl MockTerminal {
    fn new() -> Self {
        Self {
            written: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn written_bytes(&self) -> Vec<u8> {
        self.written.lock().unwrap().clone()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Terminal for MockTerminal {
    fn size(&self) -> io::Result<TerminalSize> {
        Ok(TerminalSize::new(80, 24))
    }
    fn enable_raw_mode(&mut self) -> io::Result<RawModeGuard> {
        Ok(RawModeGuard::disabled())
    }
    fn supports_keyboard_enhancement(&self) -> bool {
        false
    }
    fn enable_keyboard_enhancement(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn disable_keyboard_enhancement(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn enter_alternate_screen(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn leave_alternate_screen(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn hide_cursor(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn show_cursor(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn move_cursor(&mut self, _col: u16, _row: u16) -> io::Result<()> {
        Ok(())
    }
    fn clear(&mut self, _clear_type: ClearType) -> io::Result<()> {
        Ok(())
    }
    fn enable_mouse_capture(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn disable_mouse_capture(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.written.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    // write_str uses default implementation
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_terminal_write_str_default() {
    let mut term = MockTerminal::new();
    term.write_str("hello world").unwrap();
    assert_eq!(term.written_bytes(), b"hello world");
}

#[test]
fn test_terminal_write_str_empty() {
    let mut term = MockTerminal::new();
    term.write_str("").unwrap();
    assert!(term.written_bytes().is_empty());
}

#[test]
fn test_terminal_write_str_unicode() {
    let mut term = MockTerminal::new();
    term.write_str("hello\u{1F600}").unwrap();
    assert_eq!(term.written_bytes(), "hello\u{1F600}".as_bytes());
}

// =========================================================================
// InputSource::drain default method
// =========================================================================

struct MockInputSource {
    events: std::sync::Mutex<Vec<InputEvent>>,
}

impl MockInputSource {
    fn with_events(events: Vec<InputEvent>) -> Self {
        Self {
            events: std::sync::Mutex::new(events),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl InputSource for MockInputSource {
    fn poll(&mut self, _timeout: Duration) -> io::Result<bool> {
        Ok(!self.events.lock().unwrap().is_empty())
    }

    fn read_event(&mut self) -> io::Result<InputEvent> {
        let mut events = self.events.lock().unwrap();
        if events.is_empty() {
            Err(io::Error::new(io::ErrorKind::WouldBlock, "no events"))
        } else {
            Ok(events.remove(0))
        }
    }
    // drain uses default implementation
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_input_source_drain_default_with_events() {
    let mut source = MockInputSource::with_events(vec![
        InputEvent::Key(KeyEvent::new(KeyCode::Char('a'))),
        InputEvent::Key(KeyEvent::new(KeyCode::Char('b'))),
        InputEvent::FocusGained,
    ]);
    let events = source.drain();
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[0], InputEvent::Key(k) if k.code == KeyCode::Char('a')));
    assert!(matches!(&events[1], InputEvent::Key(k) if k.code == KeyCode::Char('b')));
    assert!(matches!(&events[2], InputEvent::FocusGained));
}

#[test]
fn test_input_source_drain_default_empty() {
    let mut source = MockInputSource::with_events(vec![]);
    let events = source.drain();
    assert!(events.is_empty());
}

// =========================================================================
// Additional coverage tests
// =========================================================================

/// Mock input source that returns an error on poll.
struct ErrorPollInputSource;

#[cfg_attr(coverage_nightly, coverage(off))]
impl InputSource for ErrorPollInputSource {
    fn poll(&mut self, _timeout: Duration) -> io::Result<bool> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "poll error"))
    }

    fn read_event(&mut self) -> io::Result<InputEvent> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "read error"))
    }
}

#[test]
fn test_input_source_drain_poll_error_returns_empty() {
    // When poll returns an error, drain should return empty (unwrap_or(false))
    let mut source = ErrorPollInputSource;
    let events = source.drain();
    assert!(events.is_empty());
}

/// Mock input source that returns error on `read_event`.
struct ErrorReadInputSource {
    poll_count: std::cell::Cell<u32>,
}

impl ErrorReadInputSource {
    fn new() -> Self {
        Self {
            poll_count: std::cell::Cell::new(0),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl InputSource for ErrorReadInputSource {
    fn poll(&mut self, _timeout: Duration) -> io::Result<bool> {
        let count = self.poll_count.get();
        self.poll_count.set(count + 1);
        // Return true for first poll, false for second
        Ok(count < 1)
    }

    fn read_event(&mut self) -> io::Result<InputEvent> {
        Err(io::Error::other("read error"))
    }
}

#[test]
fn test_input_source_drain_read_error_skips_event() {
    // When read_event returns error, drain should skip that event
    let mut source = ErrorReadInputSource::new();
    let events = source.drain();
    // No events collected since read_event failed
    assert!(events.is_empty());
}

#[test]
fn test_input_source_drain_single_event() {
    let mut source = MockInputSource::with_events(vec![InputEvent::FocusGained]);
    let events = source.drain();
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0], InputEvent::FocusGained));
}

#[test]
fn test_raw_mode_guard_drop_with_restore() {
    // Test that RawModeGuard::new with a closure that runs on drop
    let guard = RawModeGuard::new(|| Ok(()));
    drop(guard); // Should not panic
}

#[test]
fn test_terminal_size_ne() {
    let s1 = TerminalSize::new(80, 24);
    let s2 = TerminalSize::new(120, 40);
    assert_ne!(s1, s2);
}

#[test]
fn test_terminal_size_with_pixels_ne() {
    let s1 = TerminalSize::with_pixels(80, 24, 800, 480);
    let s2 = TerminalSize::with_pixels(80, 24, 1920, 1080);
    assert_ne!(s1, s2);
}

#[test]
fn test_modifiers_from_bits_specific_values() {
    assert_eq!(Modifiers::from_bits(0), Modifiers::NONE);
    assert_eq!(Modifiers::from_bits(1), Modifiers::SHIFT);
    assert_eq!(Modifiers::from_bits(2), Modifiers::CTRL);
    assert_eq!(Modifiers::from_bits(4), Modifiers::ALT);
    assert_eq!(Modifiers::from_bits(8), Modifiers::SUPER);
    assert_eq!(Modifiers::from_bits(16), Modifiers::HYPER);
    assert_eq!(Modifiers::from_bits(32), Modifiers::META);
}

#[test]
fn test_key_event_state_all_combined() {
    let all = KeyEventState::KEYPAD
        .union(KeyEventState::CAPS_LOCK)
        .union(KeyEventState::NUM_LOCK);
    assert!(all.contains(KeyEventState::KEYPAD));
    assert!(all.contains(KeyEventState::CAPS_LOCK));
    assert!(all.contains(KeyEventState::NUM_LOCK));
    assert!(!all.is_empty());
}

#[test]
fn test_key_event_state_contains_none() {
    // NONE should be contained in everything
    assert!(KeyEventState::NONE.contains(KeyEventState::NONE));
    assert!(KeyEventState::KEYPAD.contains(KeyEventState::NONE));
}

#[test]
fn test_key_event_full_construction() {
    let key = KeyEvent {
        code: KeyCode::Char('x'),
        modifiers: Modifiers::CTRL | Modifiers::SHIFT,
        kind: KeyEventKind::Repeat,
        state: KeyEventState::KEYPAD.union(KeyEventState::CAPS_LOCK),
    };
    assert_eq!(key.code, KeyCode::Char('x'));
    assert!(key.modifiers.contains(Modifiers::CTRL));
    assert!(key.modifiers.contains(Modifiers::SHIFT));
    assert_eq!(key.kind, KeyEventKind::Repeat);
    assert!(key.state.contains(KeyEventState::KEYPAD));
    assert!(key.state.contains(KeyEventState::CAPS_LOCK));
}

#[test]
fn test_mouse_event_ne() {
    let e1 = MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 0,
        row: 0,
        modifiers: Modifiers::NONE,
    };
    let e2 = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 0,
        row: 0,
        modifiers: Modifiers::NONE,
    };
    assert_ne!(e1, e2);
}

#[test]
fn test_mouse_event_different_positions() {
    let e1 = MouseEvent {
        kind: MouseEventKind::Moved,
        column: 5,
        row: 10,
        modifiers: Modifiers::NONE,
    };
    let e2 = MouseEvent {
        kind: MouseEventKind::Moved,
        column: 6,
        row: 10,
        modifiers: Modifiers::NONE,
    };
    assert_ne!(e1, e2);
}

#[test]
fn test_input_event_resize_debug() {
    let event = InputEvent::Resize(TerminalSize::new(80, 24));
    let debug = format!("{event:?}");
    assert!(debug.contains("Resize"));
}

#[test]
fn test_input_event_mouse_debug() {
    let event = InputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 2,
        modifiers: Modifiers::NONE,
    });
    let debug = format!("{event:?}");
    assert!(debug.contains("Mouse"));
}

#[test]
fn test_parse_color_error_source() {
    let err = ParseColorError {
        input: "bad".to_string(),
        kind: ParseColorErrorKind::UnknownColorName,
    };
    // Verify it implements std::error::Error with source() returning None
    let source = std::error::Error::source(&err);
    assert!(source.is_none());
}

#[test]
fn test_color_debug_all_variants() {
    // Verify Debug impl for all variants
    let variants: Vec<Color> = vec![
        Color::Reset,
        Color::Black,
        Color::DarkRed,
        Color::DarkGreen,
        Color::DarkYellow,
        Color::DarkBlue,
        Color::DarkMagenta,
        Color::DarkCyan,
        Color::Grey,
        Color::DarkGrey,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::White,
        Color::AnsiValue(42),
        Color::Rgb { r: 1, g: 2, b: 3 },
    ];
    for color in &variants {
        let debug = format!("{color:?}");
        assert!(!debug.is_empty());
    }
}

#[test]
fn test_mock_terminal_all_methods() {
    let mut term = MockTerminal::new();
    assert_eq!(term.size().unwrap(), TerminalSize::new(80, 24));
    assert!(!term.supports_keyboard_enhancement());
    assert!(term.enable_raw_mode().is_ok());
    assert!(term.enable_keyboard_enhancement().is_ok());
    assert!(term.disable_keyboard_enhancement().is_ok());
    assert!(term.enter_alternate_screen().is_ok());
    assert!(term.leave_alternate_screen().is_ok());
    assert!(term.hide_cursor().is_ok());
    assert!(term.show_cursor().is_ok());
    assert!(term.move_cursor(10, 20).is_ok());
    assert!(term.clear(ClearType::All).is_ok());
    assert!(term.enable_mouse_capture().is_ok());
    assert!(term.disable_mouse_capture().is_ok());
    assert!(term.flush().is_ok());
}

#[test]
fn test_input_event_clone_all_variants() {
    let events: Vec<InputEvent> = vec![
        InputEvent::Key(KeyEvent::new(KeyCode::Char('a'))),
        InputEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: Modifiers::NONE,
        }),
        InputEvent::Resize(TerminalSize::new(80, 24)),
        InputEvent::FocusGained,
        InputEvent::FocusLost,
        InputEvent::Paste("hello".to_string()),
    ];
    for event in events {
        let cloned = event.clone();
        let debug_orig = format!("{event:?}");
        let debug_cloned = format!("{cloned:?}");
        assert_eq!(debug_orig, debug_cloned);
    }
}
