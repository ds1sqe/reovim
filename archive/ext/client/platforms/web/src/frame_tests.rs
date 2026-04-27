use crate::frame::{WebCell, WebColor, WebFrame, canonical_frame};

#[test]
fn canonical_frame_has_expected_dimensions() {
    let f = canonical_frame();
    assert_eq!(f.width(), 20);
    assert_eq!(f.height(), 3);
}

#[test]
fn canonical_frame_row_zero_spot_checks() {
    let f = canonical_frame();
    // `  HELLO, RENDER-CODEC`: 'H' at x=2.
    assert_eq!(f.cell(2, 0).unwrap().ch, 'H');
    assert_eq!(f.cell(2, 0).unwrap().fg, Some(WebColor::Named("slategray")));
    assert_eq!(f.cell(0, 0).unwrap().ch, ' ');
    assert!(f.cell(0, 0).unwrap().fg.is_none());
}

#[test]
fn canonical_frame_row_one_underline() {
    let f = canonical_frame();
    // U+2500 box-drawings-light-horizontal at x=2..=18.
    assert_eq!(f.cell(2, 1).unwrap().ch, '\u{2500}');
    assert_eq!(f.cell(2, 1).unwrap().fg, Some(WebColor::Named("dimgray")));
}

#[test]
fn canonical_frame_row_two_footer_is_default_color() {
    let f = canonical_frame();
    // `  FROM apps/web #753`: 'F' at x=2, default color.
    assert_eq!(f.cell(2, 2).unwrap().ch, 'F');
    assert!(f.cell(2, 2).unwrap().fg.is_none());
}

#[test]
fn cell_returns_none_out_of_bounds() {
    let f = canonical_frame();
    assert!(f.cell(999, 0).is_none());
    assert!(f.cell(0, 999).is_none());
    assert!(f.cell(20, 0).is_none()); // exactly at width
    assert!(f.cell(0, 3).is_none()); // exactly at height
}

#[test]
fn new_frame_has_default_cells() {
    let f = WebFrame::new(4, 2);
    assert_eq!(f.width(), 4);
    assert_eq!(f.height(), 2);
    for y in 0..2 {
        for x in 0..4 {
            let c = f.cell(x, y).unwrap();
            assert_eq!(c.ch, ' ');
            assert!(c.fg.is_none());
        }
    }
}

#[test]
fn set_cell_writes_in_bounds_and_ignores_oob() {
    let mut f = WebFrame::new(2, 2);
    f.set_cell(
        1,
        1,
        WebCell {
            ch: 'X',
            fg: Some(WebColor::Rgb(10, 20, 30)),
        },
    );
    assert_eq!(f.cell(1, 1).unwrap().ch, 'X');
    assert_eq!(f.cell(1, 1).unwrap().fg, Some(WebColor::Rgb(10, 20, 30)));
    // OOB write — silent no-op, existing cells unchanged.
    f.set_cell(99, 99, WebCell { ch: 'Z', fg: None });
    assert_eq!(f.cell(0, 0).unwrap().ch, ' ');
}

#[test]
fn webcell_default_is_space_no_color() {
    let c = WebCell::default();
    assert_eq!(c.ch, ' ');
    assert!(c.fg.is_none());
}
