use {super::parse_view_hint, reovim_ext_client_tui_cap_cell_view::ViewHint};

#[test]
fn parses_halfblock_forms() {
    for v in ["half", "halfblock", "HalfBlock", "HALF_BLOCK", "half-block"] {
        assert_eq!(parse_view_hint(v), Some(ViewHint::HalfBlock), "input: {v}");
    }
}

#[test]
fn parses_fullblock_forms() {
    for v in ["full", "fullblock", "Full_Block", "FULL-BLOCK"] {
        assert_eq!(parse_view_hint(v), Some(ViewHint::FullBlock), "input: {v}");
    }
}

#[test]
fn parses_braille_forms() {
    for v in ["braille", "Braille", "BRAILLE", "  braille  "] {
        assert_eq!(parse_view_hint(v), Some(ViewHint::Braille), "input: {v}");
    }
}

#[test]
fn unknown_returns_none() {
    assert_eq!(parse_view_hint(""), None);
    assert_eq!(parse_view_hint("pixel"), None);
    assert_eq!(parse_view_hint("quarter-block"), None);
}

#[test]
fn trims_and_lowercases() {
    assert_eq!(parse_view_hint("  HALFBLOCK\n"), Some(ViewHint::HalfBlock));
}
