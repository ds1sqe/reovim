use crate::{
    frame::{WebCell, WebColor, WebFrame, canonical_frame},
    svg::{render_frame_svg, render_page},
};

fn put(f: &mut WebFrame, x: u16, y: u16, ch: char, fg: Option<WebColor>) {
    f.set_cell(x, y, WebCell { ch, fg });
}

#[test]
fn render_frame_svg_empty_frame_emits_svg_wrapper_only() {
    let f = WebFrame::new(4, 2);
    let svg = render_frame_svg(&f);
    assert!(svg.starts_with("<svg "));
    assert!(svg.ends_with("</svg>"));
    assert!(!svg.contains("<text"), "empty frame should emit no <text> elements; got: {svg}");
    assert!(svg.contains("viewBox=\"0 0 32 32\""));
    assert!(svg.contains("font-family=\"monospace\""));
    assert!(svg.contains("font-size=\"14\""));
}

#[test]
fn render_frame_svg_skips_space_cells() {
    let mut f = WebFrame::new(3, 1);
    put(&mut f, 0, 0, ' ', None);
    put(&mut f, 1, 0, 'X', None);
    put(&mut f, 2, 0, ' ', None);
    let svg = render_frame_svg(&f);
    // Exactly one <text> element.
    assert_eq!(svg.matches("<text").count(), 1);
    assert!(svg.contains(">X</text>"));
}

#[test]
fn render_frame_svg_emits_rgb_fill() {
    let mut f = WebFrame::new(1, 1);
    put(&mut f, 0, 0, 'A', Some(WebColor::Rgb(10, 20, 30)));
    let svg = render_frame_svg(&f);
    assert!(svg.contains("fill=\"#0a141e\""));
    assert!(svg.contains(">A</text>"));
}

#[test]
fn render_frame_svg_emits_named_fill() {
    let mut f = WebFrame::new(1, 1);
    put(&mut f, 0, 0, 'B', Some(WebColor::Named("red")));
    let svg = render_frame_svg(&f);
    assert!(svg.contains("fill=\"red\""));
}

#[test]
fn render_frame_svg_omits_fill_for_default_color() {
    let mut f = WebFrame::new(1, 1);
    put(&mut f, 0, 0, 'C', Some(WebColor::Default));
    let svg = render_frame_svg(&f);
    assert!(
        !svg.contains("fill=\""),
        "default color must not emit a fill attribute; got: {svg}"
    );
    assert!(svg.contains(">C</text>"));
}

#[test]
fn render_frame_svg_omits_fill_for_none_fg() {
    let mut f = WebFrame::new(1, 1);
    put(&mut f, 0, 0, 'D', None);
    let svg = render_frame_svg(&f);
    assert!(!svg.contains("fill=\""));
}

#[test]
fn render_frame_svg_xml_escapes_lt_gt_amp() {
    let mut f = WebFrame::new(3, 1);
    put(&mut f, 0, 0, '<', None);
    put(&mut f, 1, 0, '>', None);
    put(&mut f, 2, 0, '&', None);
    let svg = render_frame_svg(&f);
    assert!(svg.contains(">&lt;</text>"));
    assert!(svg.contains(">&gt;</text>"));
    assert!(svg.contains(">&amp;</text>"));
}

#[test]
fn render_frame_svg_positions_row_major() {
    // 2 cells in different rows — 'A' at (1, 0) and 'B' at (0, 1).
    let mut f = WebFrame::new(2, 2);
    put(&mut f, 1, 0, 'A', None);
    put(&mut f, 0, 1, 'B', None);
    let svg = render_frame_svg(&f);
    let a_idx = svg.find(">A</text>").expect("A rendered");
    let b_idx = svg.find(">B</text>").expect("B rendered");
    assert!(a_idx < b_idx, "row-major order: A (y=0) must precede B (y=1)");
    // A's baseline = (0 + 1) * 16 - 3 = 13; B's = 29.
    assert!(svg.contains("<text x=\"8\" y=\"13\""));
    assert!(svg.contains("<text x=\"0\" y=\"29\""));
}

#[test]
fn render_page_wraps_svg_with_html_doctype() {
    let f = canonical_frame();
    let html = render_page(&f);
    assert!(html.starts_with("<!DOCTYPE html><html><head>"));
    assert!(html.contains("<meta charset=\"utf-8\">"));
    assert!(html.ends_with("</body></html>"));
    assert!(html.contains(&render_frame_svg(&f)));
}

#[test]
fn render_page_title_contains_reovim_web() {
    let html = render_page(&canonical_frame());
    assert!(html.contains("<title>reovim-web — canonical frame</title>"));
}

#[test]
fn render_page_has_no_inter_tag_newlines() {
    // Whitespace contract: no newlines anywhere in the document.
    let html = render_page(&canonical_frame());
    assert!(!html.contains('\n'), "render_page must emit a single-line document");
}
