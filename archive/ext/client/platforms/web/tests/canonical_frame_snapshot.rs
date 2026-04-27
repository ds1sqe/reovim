//! Canonical-frame SVG snapshot + HTML-page integration test.
//!
//! Loads `canonical_frame_fixture.svg` via `include_str!` and asserts
//! that `render_frame_svg(canonical_frame())` matches it byte-for-byte.
//! A failure almost always means a cosmetic reformat changed
//! whitespace — that is the snapshot contract, not a bug.

use reovim_client_ext_platform_web::{canonical_frame, render_frame_svg, render_page};

const FIXTURE: &str = include_str!("canonical_frame_fixture.svg");

/// Strip the leading comment banner (`<!-- … -->`) + any trailing
/// blank line so the remainder is the exact SVG payload. Uses
/// `rsplit_once("-->")` so that a literal `-->` inside the banner
/// body (e.g. inside quoted prose) does not shadow the real closing
/// delimiter.
fn fixture_svg() -> &'static str {
    let tail = FIXTURE
        .rsplit_once("-->")
        .expect("fixture must end its banner with `-->`")
        .1
        .trim_start_matches(['\n', '\r']);
    tail.strip_suffix('\n').unwrap_or(tail)
}

#[test]
fn canonical_svg_matches_fixture_byte_for_byte() {
    let actual = render_frame_svg(&canonical_frame());
    let expected = fixture_svg();
    assert_eq!(
        actual, expected,
        "canonical SVG diverged from fixture. actual:\n{actual}\n---\nexpected:\n{expected}"
    );
}

#[test]
fn canonical_page_contains_canonical_svg() {
    let page = render_page(&canonical_frame());
    let svg = fixture_svg();
    assert!(
        page.contains(svg),
        "rendered page does not contain the canonical fixture SVG. page:\n{page}"
    );
}

#[test]
fn canonical_svg_rendering_is_idempotent() {
    let a = render_frame_svg(&canonical_frame());
    let b = render_frame_svg(&canonical_frame());
    assert_eq!(a, b);
}
