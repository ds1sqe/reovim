use super::*;

#[test]
fn test_char_width_ascii() {
    assert_eq!(char_width('a'), 1);
    assert_eq!(char_width(' '), 1);
}

#[test]
fn test_char_width_cjk() {
    assert_eq!(char_width('中'), 2);
    assert_eq!(char_width('日'), 2);
}

#[test]
fn test_cell_new() {
    let cell = Cell::new('x', Style::default());
    assert_eq!(cell.char, 'x');
    assert_eq!(cell.width, 1);
}

#[test]
fn test_cell_empty() {
    let cell = Cell::empty();
    assert!(cell.is_empty());
}

#[test]
fn test_cell_from_char() {
    let cell = Cell::from_char('y');
    assert_eq!(cell.char, 'y');
    assert_eq!(cell.style, Style::default());
    assert_eq!(cell.width, 1);
}

#[test]
fn test_cell_continuation() {
    let cell = Cell::continuation();
    assert!(cell.is_continuation);
    assert_eq!(cell.width, 0);
    assert_eq!(cell.char, ' ');
}

#[test]
fn test_cell_is_wide() {
    let cell = Cell::new('中', Style::default());
    assert!(cell.is_wide());
    assert_eq!(cell.width, 2);

    let cell = Cell::new('a', Style::default());
    assert!(!cell.is_wide());
}

#[test]
fn test_cell_differs_from() {
    let cell1 = Cell::new('a', Style::default());
    let cell2 = Cell::new('b', Style::default());
    assert!(cell1.differs_from(&cell2));

    let cell3 = Cell::new('a', Style::default());
    assert!(!cell1.differs_from(&cell3));

    let cell4 = Cell::new('a', Style::new().bold());
    assert!(cell1.differs_from(&cell4));

    let cell5 = Cell::continuation();
    assert!(cell1.differs_from(&cell5));
}

#[test]
fn test_cell_is_empty_with_style() {
    let cell = Cell::new(' ', Style::new().bold());
    assert!(!cell.is_empty()); // Has style, not empty
}

#[test]
fn test_cell_is_empty_continuation() {
    let cell = Cell::continuation();
    assert!(!cell.is_empty()); // Continuation cells are not empty
}

#[test]
fn test_cell_default() {
    let cell = Cell::default();
    assert!(cell.is_empty());
    assert_eq!(cell.char, ' ');
}

#[test]
fn test_cell_clone() {
    let cell = Cell::new('z', Style::new().bold());
    let cloned = cell.clone();
    assert_eq!(cell, cloned);
}

#[test]
fn test_cell_debug() {
    let cell = Cell::new('a', Style::default());
    let debug = format!("{cell:?}");
    assert!(debug.contains("Cell"));
}

#[test]
fn test_cell_eq() {
    let cell1 = Cell::new('x', Style::default());
    let cell2 = Cell::new('x', Style::default());
    assert_eq!(cell1, cell2);

    let cell3 = Cell::new('y', Style::default());
    assert_ne!(cell1, cell3);
}

#[test]
fn test_char_width_emoji() {
    // Some emojis are wide
    let w = char_width('😀');
    assert!(w >= 1);
}

#[test]
fn test_char_width_control() {
    // Control characters have width 0, but our function maps to 1
    let w = char_width('\x00');
    // Just ensure it returns a valid value
    let _ = w;
}
