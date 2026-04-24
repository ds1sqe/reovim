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
fn rejects_braille_until_flight_75() {
    // The Braille rasterizer is not implemented in Flight 74 and the
    // dispatch arm is a defence-in-depth `unreachable!()`. Accepting
    // the env-var value here would make the `unreachable!` reachable
    // from user input. Flight 75 adds the match arm alongside the
    // rasterizer.
    assert_eq!(parse_view_hint("braille"), None);
    assert_eq!(parse_view_hint("  Braille  "), None);
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
