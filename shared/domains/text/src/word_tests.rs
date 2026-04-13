use super::*;

#[test]
fn test_char_kind_word() {
    assert_eq!(char_kind('a'), CharKind::Word);
    assert_eq!(char_kind('Z'), CharKind::Word);
    assert_eq!(char_kind('5'), CharKind::Word);
    assert_eq!(char_kind('_'), CharKind::Word);
}

#[test]
fn test_char_kind_punctuation() {
    assert_eq!(char_kind('.'), CharKind::Punctuation);
    assert_eq!(char_kind(','), CharKind::Punctuation);
    assert_eq!(char_kind('!'), CharKind::Punctuation);
    assert_eq!(char_kind('@'), CharKind::Punctuation);
    assert_eq!(char_kind('('), CharKind::Punctuation);
}

#[test]
fn test_char_kind_whitespace() {
    assert_eq!(char_kind(' '), CharKind::Whitespace);
    assert_eq!(char_kind('\t'), CharKind::Whitespace);
    assert_eq!(char_kind('\n'), CharKind::Whitespace);
}

#[test]
fn test_char_kind_unicode() {
    // Unicode letters are word characters
    assert_eq!(char_kind('\u{00e9}'), CharKind::Word);
    assert_eq!(char_kind('\u{65e5}'), CharKind::Word);
    assert_eq!(char_kind('\u{03b1}'), CharKind::Word);
    // Unicode punctuation
    assert_eq!(char_kind('\u{2026}'), CharKind::Punctuation);
}

#[test]
fn test_word_start_small() {
    let text: Vec<char> = "hello world".chars().collect();
    assert_eq!(word_start(&text, 0, WordType::Small), 0);
    assert_eq!(word_start(&text, 3, WordType::Small), 0);
    assert_eq!(word_start(&text, 4, WordType::Small), 0);
    assert_eq!(word_start(&text, 6, WordType::Small), 6);
    assert_eq!(word_start(&text, 10, WordType::Small), 6);
}

#[test]
fn test_word_end_small() {
    let text: Vec<char> = "hello world".chars().collect();
    assert_eq!(word_end(&text, 0, WordType::Small), 4);
    assert_eq!(word_end(&text, 3, WordType::Small), 4);
    assert_eq!(word_end(&text, 6, WordType::Small), 10);
}

#[test]
fn test_word_start_big() {
    let text: Vec<char> = "foo.bar baz".chars().collect();
    // Big word: foo.bar is one word
    assert_eq!(word_start(&text, 4, WordType::Big), 0);
    assert_eq!(word_start(&text, 6, WordType::Big), 0);
}

#[test]
fn test_word_end_big() {
    let text: Vec<char> = "foo.bar baz".chars().collect();
    // Big word: foo.bar ends at 6
    assert_eq!(word_end(&text, 0, WordType::Big), 6);
    assert_eq!(word_end(&text, 4, WordType::Big), 6);
}

#[test]
fn test_word_bounds() {
    let text: Vec<char> = "hello world".chars().collect();
    let (start, end) = word_bounds(&text, 2, WordType::Small);
    assert_eq!(start, 0);
    assert_eq!(end, 4);
}

#[test]
fn test_word_bounds_empty_slice() {
    let empty: Vec<char> = vec![];
    let (start, end) = word_bounds(&empty, 0, WordType::Small);
    assert_eq!(start, 0);
    assert_eq!(end, 0);
}

#[test]
fn test_word_start_at_boundary() {
    let text: Vec<char> = "hello".chars().collect();
    assert_eq!(word_start(&text, 0, WordType::Small), 0);
}

#[test]
fn test_word_end_at_boundary() {
    let text: Vec<char> = "hello".chars().collect();
    assert_eq!(word_end(&text, 4, WordType::Small), 4);
}

#[test]
fn test_word_small_with_punctuation() {
    let text: Vec<char> = "foo.bar".chars().collect();
    // Small word: foo, ., bar are separate
    assert_eq!(word_end(&text, 0, WordType::Small), 2); // "foo"
    assert_eq!(word_start(&text, 3, WordType::Small), 3); // "."
    assert_eq!(word_end(&text, 3, WordType::Small), 3); // "."
    assert_eq!(word_start(&text, 4, WordType::Small), 4); // "bar"
}

#[test]
fn test_next_word_start() {
    let text: Vec<char> = "hello world test".chars().collect();
    assert_eq!(next_word_start(&text, 0, WordType::Small), 6); // -> "world"
    assert_eq!(next_word_start(&text, 6, WordType::Small), 12); // -> "test"
}

#[test]
fn test_next_word_end() {
    let text: Vec<char> = "hello world".chars().collect();
    assert_eq!(next_word_end(&text, 0, WordType::Small), 4); // end of "hello"
    assert_eq!(next_word_end(&text, 4, WordType::Small), 10); // end of "world"
}

#[test]
fn test_word_type_default() {
    assert_eq!(WordType::default(), WordType::Small);
}

#[test]
fn test_whitespace_handling() {
    let text: Vec<char> = "  hello  ".chars().collect();
    // At whitespace, word_start/end returns the whitespace position
    assert_eq!(word_start(&text, 1, WordType::Small), 1);
    assert_eq!(word_end(&text, 1, WordType::Small), 1);
}

// === Coverage: next_word_start BigWord from whitespace ===

#[test]
fn test_next_word_start_bigword_from_whitespace() {
    let text: Vec<char> = "  hello.world".chars().collect();
    // Starting at whitespace with BigWord
    let result = next_word_start(&text, 0, WordType::Big);
    assert_eq!(result, 2); // Skip whitespace, land on 'h'
}

// === Coverage: next_word_start BigWord from non-whitespace ===

#[test]
fn test_next_word_start_bigword_from_nonws() {
    let text: Vec<char> = "hello.world foo".chars().collect();
    let result = next_word_start(&text, 0, WordType::Big);
    assert_eq!(result, 12); // Skip hello.world + space, land on 'f'
}

// === Coverage: next_word_end at last position ===

#[test]
fn test_next_word_end_at_end() {
    let text: Vec<char> = "hello".chars().collect();
    let result = next_word_end(&text, 4, WordType::Small);
    // Already at end, should stay clamped
    assert_eq!(result, 4);
}

// === Coverage: next_word_end empty ===

#[test]
fn test_next_word_end_empty() {
    let empty: Vec<char> = vec![];
    assert_eq!(next_word_end(&empty, 0, WordType::Small), 0);
}

// === Coverage: next_word_start empty ===

#[test]
fn test_next_word_start_empty() {
    let empty: Vec<char> = vec![];
    assert_eq!(next_word_start(&empty, 0, WordType::Small), 0);
}

// === Coverage: word_start beyond length (clamp) ===

#[test]
fn test_word_start_beyond_length() {
    let text: Vec<char> = "hello".chars().collect();
    // pos is beyond length, should be clamped to len-1
    let result = word_start(&text, 100, WordType::Small);
    assert_eq!(result, 0); // "hello" starts at 0
}

// === Coverage: word_end beyond length (clamp) ===

#[test]
fn test_word_end_beyond_length() {
    let text: Vec<char> = "hello".chars().collect();
    let result = word_end(&text, 100, WordType::Small);
    assert_eq!(result, 4); // "hello" ends at 4
}

// === Coverage: next_word_end with whitespace gap ===

#[test]
fn test_next_word_end_with_whitespace_gap() {
    let text: Vec<char> = "hello   world".chars().collect();
    // At end of "hello", next word end should be end of "world"
    let result = next_word_end(&text, 4, WordType::Small);
    assert_eq!(result, 12); // end of "world"
}

// === Coverage: next_word_end BigWord same_word logic ===

#[test]
fn test_next_word_end_bigword() {
    let text: Vec<char> = "hello.world foo".chars().collect();
    let result = next_word_end(&text, 0, WordType::Big);
    assert_eq!(result, 10); // end of "hello.world"
}

// === Coverage: next_word_start past end of text ===

#[test]
fn test_next_word_start_at_end() {
    let text: Vec<char> = "hello".chars().collect();
    let result = next_word_start(&text, 4, WordType::Small);
    assert_eq!(result, 5); // past end
}

// === Coverage: word_bounds Big with punctuation ===

#[test]
fn test_word_bounds_big_with_punctuation() {
    let text: Vec<char> = "foo.bar baz".chars().collect();
    let (start, end) = word_bounds(&text, 2, WordType::Big);
    assert_eq!(start, 0);
    assert_eq!(end, 6); // "foo.bar" is one BigWord
}

// === Coverage: CharKind Hash ===

#[test]
fn test_char_kind_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(CharKind::Word);
    set.insert(CharKind::Punctuation);
    set.insert(CharKind::Whitespace);
    assert_eq!(set.len(), 3);
}

// === Coverage: WordType Hash ===

#[test]
fn test_word_type_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(WordType::Small);
    set.insert(WordType::Big);
    assert_eq!(set.len(), 2);
}
