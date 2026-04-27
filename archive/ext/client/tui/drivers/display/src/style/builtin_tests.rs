use {super::*, crate::highlight::Attributes};

#[test]
fn test_dark_palette_all_groups() {
    let palette = &DARK_PALETTE;
    for group in groups::ALL_GROUPS {
        assert!(palette.contains_key(group), "Dark palette missing group: {group}");
    }
    assert_eq!(palette.len(), groups::ALL_GROUPS.len());
}

#[test]
fn test_light_palette_all_groups() {
    let palette = &LIGHT_PALETTE;
    for group in groups::ALL_GROUPS {
        assert!(palette.contains_key(group), "Light palette missing group: {group}");
    }
    assert_eq!(palette.len(), groups::ALL_GROUPS.len());
}

#[test]
fn test_tokyo_night_palette_all_groups() {
    let palette = &TOKYO_NIGHT_ORANGE_PALETTE;
    for group in groups::ALL_GROUPS {
        assert!(palette.contains_key(group), "TokyoNightOrange palette missing group: {group}");
    }
    assert_eq!(palette.len(), groups::ALL_GROUPS.len());
}

#[test]
fn test_get_palette_returns_correct_variant() {
    let dark = get_palette(super::super::BuiltinTheme::Dark);
    let light = get_palette(super::super::BuiltinTheme::Light);
    let tokyo = get_palette(super::super::BuiltinTheme::TokyoNightOrange);

    // Each should be different (they have different colors)
    let dark_keyword = dark.get(groups::KEYWORD).unwrap();
    let light_keyword = light.get(groups::KEYWORD).unwrap();
    let tokyo_keyword = tokyo.get(groups::KEYWORD).unwrap();

    assert_ne!(dark_keyword.fg, light_keyword.fg);
    assert_ne!(dark_keyword.fg, tokyo_keyword.fg);
}

#[test]
fn test_dark_theme_has_bold_keywords() {
    let palette = &DARK_PALETTE;
    let keyword = palette.get(groups::KEYWORD).unwrap();
    assert!(keyword.attributes.contains(Attributes::BOLD));
}

#[test]
fn test_all_themes_have_italic_comments() {
    for variant in super::super::BuiltinTheme::all() {
        let palette = get_palette(*variant);
        let name = variant.name();
        let comment = palette.get(groups::COMMENT).unwrap();
        assert!(
            comment.attributes.contains(Attributes::ITALIC),
            "{name} theme should have italic comments",
        );
    }
}
