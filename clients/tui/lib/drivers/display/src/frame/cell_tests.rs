use super::*;

#[test]
fn test_char_width_ascii() {
    assert_eq!(char_width('a'), 1);
    assert_eq!(char_width('Z'), 1);
    assert_eq!(char_width('0'), 1);
    assert_eq!(char_width(' '), 1);
    assert_eq!(char_width('!'), 1);
}

#[test]
fn test_char_width_cjk() {
    // CJK characters are width 2
    assert_eq!(char_width('中'), 2);
    assert_eq!(char_width('日'), 2);
    assert_eq!(char_width('本'), 2);
    assert_eq!(char_width('語'), 2);
    assert_eq!(char_width('한'), 2); // Korean
    assert_eq!(char_width('あ'), 2); // Hiragana
    assert_eq!(char_width('ア'), 2); // Katakana
}

#[test]
fn test_char_width_fullwidth() {
    // Fullwidth ASCII variants
    assert_eq!(char_width('Ａ'), 2); // Fullwidth A
    assert_eq!(char_width('１'), 2); // Fullwidth 1
}

#[test]
fn test_cell_new() {
    let cell = Cell::new('x', Style::default());
    assert_eq!(cell.char, 'x');
    assert_eq!(cell.width, 1);
    assert!(!cell.is_continuation);
}

#[test]
fn test_cell_from_char() {
    let cell = Cell::from_char('中');
    assert_eq!(cell.char, '中');
    assert_eq!(cell.width, 2);
    assert!(!cell.is_continuation);
}

#[test]
fn test_cell_continuation() {
    let cell = Cell::continuation();
    assert_eq!(cell.char, ' ');
    assert_eq!(cell.width, 0);
    assert!(cell.is_continuation);
}

#[test]
fn test_cell_empty() {
    let cell = Cell::empty();
    assert_eq!(cell.char, ' ');
    assert_eq!(cell.width, 1);
    assert!(!cell.is_continuation);
    assert!(cell.is_empty());
}

#[test]
fn test_cell_differs_from() {
    let cell1 = Cell::from_char('a');
    let cell2 = Cell::from_char('a');
    let cell3 = Cell::from_char('b');

    assert!(!cell1.differs_from(&cell2));
    assert!(cell1.differs_from(&cell3));
}

#[test]
fn test_cell_is_wide() {
    assert!(!Cell::from_char('a').is_wide());
    assert!(Cell::from_char('中').is_wide());
}

#[test]
fn test_cell_default() {
    let cell = Cell::default();
    assert!(cell.is_empty());
}

// MC/DC tests for differs_from (3-term OR: char || style || continuation)
#[test]
fn test_differs_from_style_only() {
    // Same char, different style -> differs (second term triggers)
    let cell1 = Cell::new('a', Style::default());
    let cell2 = Cell::new('a', Style::new().bold());
    assert!(cell1.differs_from(&cell2));
}

#[test]
fn test_differs_from_continuation_only() {
    // Same char, same style, different continuation -> differs (third term triggers)
    let mut cell1 = Cell::new(' ', Style::default());
    let mut cell2 = Cell::new(' ', Style::default());
    cell1.is_continuation = false;
    cell2.is_continuation = true;
    assert!(cell1.differs_from(&cell2));
}

// MC/DC tests for is_empty (3-term AND: char==' ' && style==default && !continuation)
#[test]
fn test_is_empty_non_space_char() {
    // Non-space char, default style, not continuation -> not empty (first term false)
    let cell = Cell::new('x', Style::default());
    assert!(!cell.is_empty());
}

#[test]
fn test_is_empty_non_default_style() {
    // Space char, non-default style, not continuation -> not empty (second term false)
    let cell = Cell::new(' ', Style::new().bold());
    assert!(!cell.is_empty());
}

#[test]
fn test_is_empty_continuation() {
    // Space char, default style, IS continuation -> not empty (third term false)
    let cell = Cell::continuation();
    assert!(!cell.is_empty());
}
