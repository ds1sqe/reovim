use super::*;
// `super::*` from `file.rs` only pulls items defined in / pub-used
// from the module. `ThemeError`, `ThemeProvider`, `Color`,
// `Attributes` are imported privately by `file.rs` and so are NOT
// reachable through the glob; bring them in explicitly here.
use {reovim_arch::Color, reovim_driver_display_registry::theme::ThemeError};

use crate::{highlight::Attributes, style::StyledTheme};

#[test]
fn test_parse_minimal_theme() {
    let toml = r#"
        [meta]
        name = "Test Theme"
    "#;

    let theme = FileTheme::parse(toml).unwrap();
    assert_eq!(theme.name(), "Test Theme");
}

#[test]
fn test_parse_theme_with_palette() {
    let toml = r##"
        [meta]
        name = "Palette Test"

        [palette]
        red = "#ff0000"
        blue = "#0000ff"

        [syntax]
        keyword = { fg = "red", bold = true }
        function = { fg = "blue" }
    "##;

    let theme = FileTheme::parse(toml).unwrap();
    let keyword = theme.get_style("keyword").unwrap();
    assert_eq!(keyword.fg, Some(Color::Rgb { r: 255, g: 0, b: 0 }));
    assert!(keyword.attributes.contains(Attributes::BOLD));

    let function = theme.get_style("function").unwrap();
    assert_eq!(function.fg, Some(Color::Rgb { r: 0, g: 0, b: 255 }));
}

#[test]
fn test_parse_hex_colors_directly() {
    let toml = r##"
        [meta]
        name = "Direct Hex"

        [ui]
        background = { bg = "#1a1b26" }
        cursor = { fg = "#ffffff", bg = "#ff9e64" }
    "##;

    let theme = FileTheme::parse(toml).unwrap();
    let bg = theme.get_style("background").unwrap();
    assert_eq!(
        bg.bg,
        Some(Color::Rgb {
            r: 26,
            g: 27,
            b: 38
        })
    );

    let cursor = theme.get_style("cursor").unwrap();
    assert_eq!(
        cursor.fg,
        Some(Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        })
    );
    assert_eq!(
        cursor.bg,
        Some(Color::Rgb {
            r: 255,
            g: 158,
            b: 100
        })
    );
}

#[test]
fn test_parse_diagnostic_section() {
    let toml = r##"
        [meta]
        name = "Diagnostic Test"

        [diagnostic]
        error = { fg = "#f7768e" }
        warn = { fg = "#e0af68" }
    "##;

    let theme = FileTheme::parse(toml).unwrap();

    // Diagnostic keys get "diagnostic." prefix
    let error = theme.get_style("diagnostic.error").unwrap();
    assert_eq!(
        error.fg,
        Some(Color::Rgb {
            r: 247,
            g: 118,
            b: 142
        })
    );
}

#[test]
fn test_parse_gutter_section() {
    let toml = r##"
        [meta]
        name = "Gutter Test"

        [gutter]
        "git.add" = { fg = "#98c379" }
        "fold.open" = { fg = "#565f89" }
    "##;

    let theme = FileTheme::parse(toml).unwrap();
    let git_add = theme.get_style("git.add").unwrap();
    assert_eq!(
        git_add.fg,
        Some(Color::Rgb {
            r: 152,
            g: 195,
            b: 121
        })
    );
}

#[test]
fn test_parse_all_attributes() {
    let toml = r##"
        [meta]
        name = "Attributes Test"

        [syntax]
        test = { fg = "#ffffff", bold = true, italic = true, underline = true, strikethrough = true }
    "##;

    let theme = FileTheme::parse(toml).unwrap();
    let style = theme.get_style("test").unwrap();
    assert!(style.attributes.contains(Attributes::BOLD));
    assert!(style.attributes.contains(Attributes::ITALIC));
    assert!(style.attributes.contains(Attributes::UNDERLINE));
    assert!(style.attributes.contains(Attributes::STRIKETHROUGH));
}

#[test]
fn test_invalid_color_error() {
    let toml = r#"
        [meta]
        name = "Invalid Color"

        [syntax]
        keyword = { fg = "not-a-color" }
    "#;

    let err = FileTheme::parse(toml).unwrap_err();
    assert_eq!(err.to_string(), "Invalid color 'not-a-color' for key 'keyword'");
}

#[test]
fn test_named_ansi_colors() {
    let toml = r#"
        [meta]
        name = "ANSI Test"

        [syntax]
        keyword = { fg = "red" }
        string = { fg = "green" }
    "#;

    let theme = FileTheme::parse(toml).unwrap();
    let keyword = theme.get_style("keyword").unwrap();
    assert_eq!(keyword.fg, Some(Color::Red));

    let string = theme.get_style("string").unwrap();
    assert_eq!(string.fg, Some(Color::Green));
}

#[test]
fn test_default_style_from_foreground() {
    let toml = r##"
        [meta]
        name = "Default Style Test"

        [ui]
        foreground = { fg = "#abcdef" }
    "##;

    let theme = FileTheme::parse(toml).unwrap();
    let default = theme.default_style();
    assert_eq!(
        default.fg,
        Some(Color::Rgb {
            r: 171,
            g: 205,
            b: 239
        })
    );
}

#[test]
fn test_into_arc() {
    let toml = r#"
        [meta]
        name = "Arc Test"
    "#;

    let theme = FileTheme::parse(toml).unwrap();
    let arc_theme = theme.into_arc();
    assert_eq!(arc_theme.name(), "Arc Test");
}

// =========================================================================
// ThemeError Display and Error trait impls
// =========================================================================

#[test]
fn test_theme_error_display_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err = ThemeError::Io(io_err);
    let msg = err.to_string();
    assert!(msg.starts_with("IO error:"));
    assert!(msg.contains("file not found"));
}

#[test]
fn test_theme_error_display_parse() {
    let parse_err = toml::from_str::<ThemeFile>("{{invalid toml").unwrap_err();
    let err = ThemeError::Parse(Box::new(parse_err));
    let msg = err.to_string();
    assert!(msg.starts_with("TOML parse error:"));
}

#[test]
fn test_theme_error_display_invalid_color() {
    let err = ThemeError::InvalidColor {
        key: "keyword".to_string(),
        value: "not-a-color".to_string(),
    };
    assert_eq!(err.to_string(), "Invalid color 'not-a-color' for key 'keyword'");
}

#[test]
fn test_theme_error_display_palette_not_found() {
    let err = ThemeError::PaletteNotFound {
        key: "keyword".to_string(),
        reference: "my_red".to_string(),
    };
    assert_eq!(err.to_string(), "Palette color 'my_red' not found for key 'keyword'");
}

#[test]
fn test_theme_error_source_io() {
    use std::error::Error as _;
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
    let err = ThemeError::Io(io_err);
    assert!(err.source().is_some());
}

#[test]
fn test_theme_error_source_parse() {
    use std::error::Error as _;
    let parse_err = toml::from_str::<ThemeFile>("{{invalid").unwrap_err();
    let err = ThemeError::Parse(Box::new(parse_err));
    assert!(err.source().is_some());
}

#[test]
fn test_theme_error_source_invalid_color_is_none() {
    use std::error::Error as _;
    let err = ThemeError::InvalidColor {
        key: "k".to_string(),
        value: "v".to_string(),
    };
    assert!(err.source().is_none());
}

#[test]
fn test_theme_error_source_palette_not_found_is_none() {
    use std::error::Error as _;
    let err = ThemeError::PaletteNotFound {
        key: "k".to_string(),
        reference: "r".to_string(),
    };
    assert!(err.source().is_none());
}

#[test]
fn test_theme_error_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let err: ThemeError = io_err.into();
    assert!(matches!(err, ThemeError::Io(_)));
}

#[test]
fn test_theme_error_parse_via_factory() {
    // The registry's ThemeError::Parse takes a boxed dyn error so
    // the registry crate doesn't depend on `toml`. Display-side
    // parse failures wrap the toml error explicitly via
    // `parse_error()` (mirrored here).
    let parse_err = toml::from_str::<ThemeFile>("{{bad").unwrap_err();
    let err = ThemeError::Parse(Box::new(parse_err));
    assert!(matches!(err, ThemeError::Parse(_)));
}

#[test]
fn test_default_theme_name_used_when_name_missing() {
    // When [meta] section exists but name is missing, default_theme_name() is used
    let toml = r#"
        [meta]
        author = "Test Author"
    "#;
    let theme = FileTheme::parse(toml).unwrap();
    assert_eq!(theme.name(), "Unnamed Theme");
}

// =========================================================================
// Base theme inheritance tests
// =========================================================================

#[test]
fn test_parse_theme_with_base_dark() {
    let toml = r##"
        [meta]
        name = "Custom Dark"
        base = "dark"

        [syntax]
        keyword = { fg = "#ff00ff", bold = true }
    "##;

    let theme = FileTheme::parse(toml).unwrap();
    assert_eq!(theme.name(), "Custom Dark");

    // Custom keyword override
    let keyword = theme.get_style("keyword").unwrap();
    assert_eq!(
        keyword.fg,
        Some(Color::Rgb {
            r: 255,
            g: 0,
            b: 255
        })
    );
    assert!(keyword.attributes.contains(Attributes::BOLD));

    // Inherited from dark base: "function" should exist
    assert!(theme.get_style("function").is_some());
}

#[test]
fn test_parse_theme_with_base_light() {
    let toml = r#"
        [meta]
        name = "Custom Light"
        base = "light"
    "#;

    let theme = FileTheme::parse(toml).unwrap();
    // All 42 base groups should be inherited from light
    assert!(theme.get_style("keyword").is_some());
    assert!(theme.get_style("function").is_some());
    assert!(theme.get_style("string").is_some());
    assert!(theme.get_style("type").is_some());
}

#[test]
fn test_parse_theme_with_base_tokyo_night() {
    let toml = r#"
        [meta]
        name = "Custom Tokyo Night"
        base = "tokyo-night-orange"
    "#;

    let theme = FileTheme::parse(toml).unwrap();
    assert!(theme.get_style("keyword").is_some());
    assert!(theme.get_style("function").is_some());
}

#[test]
fn test_base_theme_overlay_overrides_base() {
    use crate::style::{BuiltinTheme, builtin};
    // Get the dark theme's keyword color for comparison
    let dark_palette = builtin::get_palette(BuiltinTheme::Dark);
    let dark_keyword = dark_palette.get("keyword").unwrap();

    let toml = r##"
        [meta]
        name = "Override Test"
        base = "dark"

        [syntax]
        keyword = { fg = "#aabbcc" }
    "##;

    let theme = FileTheme::parse(toml).unwrap();
    let keyword = theme.get_style("keyword").unwrap();

    // Should NOT match the dark base keyword
    assert_ne!(keyword.fg, dark_keyword.fg);
    // Should be our custom color
    assert_eq!(
        keyword.fg,
        Some(Color::Rgb {
            r: 170,
            g: 187,
            b: 204
        })
    );
}

#[test]
fn test_base_theme_preserves_unset_groups() {
    use crate::style::{BuiltinTheme, builtin};
    let dark_palette = builtin::get_palette(BuiltinTheme::Dark);
    let dark_function = dark_palette.get("function").unwrap();

    let toml = r##"
        [meta]
        name = "Preserve Test"
        base = "dark"

        [syntax]
        keyword = { fg = "#111111" }
    "##;

    let theme = FileTheme::parse(toml).unwrap();
    // "function" was not overridden, should match dark base exactly
    let function = theme.get_style("function").unwrap();
    assert_eq!(function.fg, dark_function.fg);
}

#[test]
fn test_invalid_base_theme_returns_error() {
    let toml = r#"
        [meta]
        name = "Bad Base"
        base = "nonexistent"
    "#;

    let err = FileTheme::parse(toml).unwrap_err();
    assert!(matches!(err, ThemeError::InvalidBase { .. }));
    assert!(err.to_string().contains("nonexistent"));
}

#[test]
fn test_no_base_theme_works() {
    // Existing behavior: no base field at all
    let toml = r##"
        [meta]
        name = "No Base"

        [syntax]
        keyword = { fg = "#ff0000" }
    "##;

    let theme = FileTheme::parse(toml).unwrap();
    // Should only have explicitly defined styles
    assert!(theme.get_style("keyword").is_some());
    assert!(theme.get_style("function").is_none());
}

// =========================================================================
// Decoration section tests
// =========================================================================

#[test]
fn test_decoration_section_parsing() {
    let toml = r##"
        [meta]
        name = "Decoration Test"

        [decoration]
        conceal = { fg = "#888888" }
        virtual_text = { fg = "#666666", italic = true }
    "##;

    let theme = FileTheme::parse(toml).unwrap();
    let conceal = theme.get_style("decoration.conceal").unwrap();
    assert_eq!(
        conceal.fg,
        Some(Color::Rgb {
            r: 136,
            g: 136,
            b: 136
        })
    );

    let vtext = theme.get_style("decoration.virtual_text").unwrap();
    assert!(vtext.attributes.contains(Attributes::ITALIC));
}

#[test]
fn test_decoration_section_with_dotted_keys() {
    let toml = r##"
        [meta]
        name = "Dotted Decoration"

        [decoration]
        "heading.1" = { fg = "#ff0000", bold = true }
        "heading.2" = { fg = "#00ff00" }
    "##;

    let theme = FileTheme::parse(toml).unwrap();
    let h1 = theme.get_style("decoration.heading.1").unwrap();
    assert_eq!(h1.fg, Some(Color::Rgb { r: 255, g: 0, b: 0 }));
    assert!(h1.attributes.contains(Attributes::BOLD));

    let h2 = theme.get_style("decoration.heading.2").unwrap();
    assert_eq!(h2.fg, Some(Color::Rgb { r: 0, g: 255, b: 0 }));
}

// =========================================================================
// InvalidBase error trait tests
// =========================================================================

#[test]
fn test_theme_error_display_invalid_base() {
    let err = ThemeError::InvalidBase {
        name: "no-such-theme".to_string(),
    };
    assert_eq!(err.to_string(), "Invalid base theme: 'no-such-theme'");
}

#[test]
fn test_theme_error_source_invalid_base_is_none() {
    use std::error::Error as _;
    let err = ThemeError::InvalidBase {
        name: "x".to_string(),
    };
    assert!(err.source().is_none());
}

#[test]
fn test_resolve_base_theme_valid() {
    assert!(FileTheme::resolve_base_theme("dark").is_some());
    assert!(FileTheme::resolve_base_theme("light").is_some());
    assert!(FileTheme::resolve_base_theme("tokyo-night-orange").is_some());
}

#[test]
fn test_resolve_base_theme_invalid() {
    assert!(FileTheme::resolve_base_theme("nonexistent").is_none());
    assert!(FileTheme::resolve_base_theme("").is_none());
}
