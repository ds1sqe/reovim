use super::*;

#[test]
fn test_direction_opposite() {
    assert_eq!(Direction::Forward.opposite(), Direction::Backward);
    assert_eq!(Direction::Backward.opposite(), Direction::Forward);
}

#[test]
fn test_direction_is_forward_backward() {
    assert!(Direction::Forward.is_forward());
    assert!(!Direction::Forward.is_backward());
    assert!(!Direction::Backward.is_forward());
    assert!(Direction::Backward.is_backward());
}

#[test]
fn test_word_boundary_word() {
    let boundary = WordBoundary::Word;
    assert!(boundary.is_word_char('a'));
    assert!(boundary.is_word_char('Z'));
    assert!(boundary.is_word_char('0'));
    assert!(boundary.is_word_char('_'));
    assert!(!boundary.is_word_char('-'));
    assert!(!boundary.is_word_char(' '));
    assert!(!boundary.is_word_char('.'));
}

#[test]
fn test_word_boundary_big_word() {
    let boundary = WordBoundary::BigWord;
    assert!(boundary.is_word_char('a'));
    assert!(boundary.is_word_char('-'));
    assert!(boundary.is_word_char('.'));
    assert!(boundary.is_word_char('_'));
    assert!(!boundary.is_word_char(' '));
    assert!(!boundary.is_word_char('\t'));
    assert!(!boundary.is_word_char('\n'));
}
