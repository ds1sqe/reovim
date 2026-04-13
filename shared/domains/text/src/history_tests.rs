use {super::*, crate::RegisterContent};

fn content(text: &str) -> RegisterContent {
    RegisterContent::characterwise(text.to_string())
}

fn linewise_content(text: &str) -> RegisterContent {
    RegisterContent::linewise(text.to_string())
}

// ========== Constructor tests ==========

#[test]
fn test_new_empty() {
    let ring = HistoryRing::new();
    assert!(ring.is_empty());
    assert_eq!(ring.len(), 0);
    assert_eq!(ring.capacity(), 256);
}

#[test]
fn test_with_capacity() {
    let ring = HistoryRing::with_capacity(5);
    assert!(ring.is_empty());
    assert_eq!(ring.capacity(), 5);
}

#[test]
fn test_default() {
    let ring = HistoryRing::default();
    assert!(ring.is_empty());
    assert_eq!(ring.capacity(), 256);
}

// ========== Push and get tests ==========

#[test]
fn test_push_single() {
    let mut ring = HistoryRing::new();
    ring.push(content("hello"));
    assert_eq!(ring.len(), 1);
    assert!(!ring.is_empty());
    assert_eq!(ring.get(0).map(|r| r.text.as_str()), Some("hello"));
}

#[test]
fn test_push_multiple_lifo_order() {
    let mut ring = HistoryRing::new();
    ring.push(content("first"));
    ring.push(content("second"));
    ring.push(content("third"));

    assert_eq!(ring.len(), 3);
    assert_eq!(ring.get(0).map(|r| r.text.as_str()), Some("third"));
    assert_eq!(ring.get(1).map(|r| r.text.as_str()), Some("second"));
    assert_eq!(ring.get(2).map(|r| r.text.as_str()), Some("first"));
}

#[test]
fn test_push_evicts_oldest_at_capacity() {
    let mut ring = HistoryRing::with_capacity(3);
    ring.push(content("a"));
    ring.push(content("b"));
    ring.push(content("c"));
    assert_eq!(ring.len(), 3);

    ring.push(content("d"));
    assert_eq!(ring.len(), 3);
    assert_eq!(ring.get(0).map(|r| r.text.as_str()), Some("d"));
    assert_eq!(ring.get(1).map(|r| r.text.as_str()), Some("c"));
    assert_eq!(ring.get(2).map(|r| r.text.as_str()), Some("b"));
    // "a" was evicted
}

#[test]
fn test_push_preserves_yank_type() {
    let mut ring = HistoryRing::new();
    ring.push(linewise_content("line\n"));

    let entry = ring.get(0).unwrap();
    assert!(entry.is_linewise());
    assert_eq!(entry.text, "line\n");
}

// ========== get_numbered tests ==========

#[test]
fn test_get_numbered_valid() {
    let mut ring = HistoryRing::new();
    ring.push(content("zero"));
    ring.push(content("one"));
    ring.push(content("two"));

    assert_eq!(ring.get_numbered('0').map(|r| r.text), Some("two".to_string()));
    assert_eq!(ring.get_numbered('1').map(|r| r.text), Some("one".to_string()));
    assert_eq!(ring.get_numbered('2').map(|r| r.text), Some("zero".to_string()));
}

#[test]
fn test_get_numbered_out_of_range() {
    let mut ring = HistoryRing::new();
    ring.push(content("only"));

    assert!(ring.get_numbered('0').is_some());
    assert!(ring.get_numbered('1').is_none());
    assert!(ring.get_numbered('9').is_none());
}

#[test]
fn test_get_numbered_invalid_char() {
    let ring = HistoryRing::new();
    assert!(ring.get_numbered('a').is_none());
    assert!(ring.get_numbered('+').is_none());
    assert!(ring.get_numbered('*').is_none());
}

#[test]
fn test_get_numbered_empty() {
    let ring = HistoryRing::new();
    assert!(ring.get_numbered('0').is_none());
}

#[test]
fn test_get_numbered_all_digits() {
    let mut ring = HistoryRing::new();
    for i in 0..10 {
        ring.push(content(&format!("entry-{i}")));
    }

    // Most recent push is "entry-9" at index 0
    for digit in '0'..='9' {
        let index = (digit as u8 - b'0') as usize;
        let expected = format!("entry-{}", 9 - index);
        assert_eq!(
            ring.get_numbered(digit).map(|r| r.text),
            Some(expected),
            "register '{digit}' mismatch"
        );
    }
}

// ========== get out of bounds ==========

#[test]
fn test_get_out_of_bounds() {
    let ring = HistoryRing::new();
    assert!(ring.get(0).is_none());
    assert!(ring.get(100).is_none());
}

// ========== iter tests ==========

#[test]
fn test_iter_order() {
    let mut ring = HistoryRing::new();
    ring.push(content("a"));
    ring.push(content("b"));
    ring.push(content("c"));

    let texts: Vec<&str> = ring.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(texts, vec!["c", "b", "a"]);
}

#[test]
fn test_iter_empty() {
    let ring = HistoryRing::new();
    assert_eq!(ring.iter().count(), 0);
}

// ========== clear tests ==========

#[test]
fn test_clear() {
    let mut ring = HistoryRing::new();
    ring.push(content("a"));
    ring.push(content("b"));
    assert_eq!(ring.len(), 2);

    ring.clear();
    assert!(ring.is_empty());
    assert_eq!(ring.len(), 0);
}

// ========== Clone tests ==========

#[test]
fn test_clone() {
    let mut ring = HistoryRing::new();
    ring.push(content("hello"));

    let cloned = ring.clone();
    assert_eq!(cloned.len(), 1);
    assert_eq!(cloned.get(0).map(|r| r.text.as_str()), Some("hello"));
    assert_eq!(cloned.capacity(), ring.capacity());
}

// ========== Full capacity rotation ==========

#[test]
fn test_full_rotation() {
    let mut ring = HistoryRing::with_capacity(3);

    // Fill completely
    ring.push(content("a"));
    ring.push(content("b"));
    ring.push(content("c"));

    // Rotate multiple times
    ring.push(content("d"));
    ring.push(content("e"));
    ring.push(content("f"));

    assert_eq!(ring.len(), 3);
    assert_eq!(ring.get(0).map(|r| r.text.as_str()), Some("f"));
    assert_eq!(ring.get(1).map(|r| r.text.as_str()), Some("e"));
    assert_eq!(ring.get(2).map(|r| r.text.as_str()), Some("d"));
}

// ========== Debug ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_debug() {
    let ring = HistoryRing::new();
    let debug_str = format!("{ring:?}");
    assert!(debug_str.contains("HistoryRing"));
}

// ========== get_by_index tests (#515 Phase 5) ==========

#[test]
fn test_get_by_index_empty() {
    let ring = HistoryRing::new();
    assert!(ring.get_by_index(0).is_none());
    assert!(ring.get_by_index(255).is_none());
}

#[test]
fn test_get_by_index_single() {
    let mut ring = HistoryRing::new();
    ring.push(content("entry"));
    assert_eq!(ring.get_by_index(0).map(|r| r.text.as_str()), Some("entry"));
    assert!(ring.get_by_index(1).is_none());
}

#[test]
fn test_get_by_index_multiple() {
    let mut ring = HistoryRing::new();
    ring.push(content("first"));
    ring.push(content("second"));
    ring.push(content("third"));
    // LIFO: index 0 = most recent
    assert_eq!(ring.get_by_index(0).map(|r| r.text.as_str()), Some("third"));
    assert_eq!(ring.get_by_index(1).map(|r| r.text.as_str()), Some("second"));
    assert_eq!(ring.get_by_index(2).map(|r| r.text.as_str()), Some("first"));
}

#[test]
fn test_get_by_index_u8_max() {
    // Ensure u8::MAX (255) works without overflow
    let ring = HistoryRing::new();
    assert!(ring.get_by_index(u8::MAX).is_none());
}

// ========== Expanded capacity tests (#515 Phase 5) ==========

#[test]
fn test_default_capacity_is_256() {
    let ring = HistoryRing::new();
    assert_eq!(ring.capacity(), 256);
}

#[test]
fn test_expanded_capacity_holds_more_than_10() {
    let mut ring = HistoryRing::new();
    // Push 20 entries - should all fit (default capacity is 256)
    for i in 0..20 {
        ring.push(content(&format!("entry-{i}")));
    }
    assert_eq!(ring.len(), 20);
    // Most recent at index 0
    assert_eq!(ring.get_by_index(0).map(|r| r.text.as_str()), Some("entry-19"));
    // Oldest at index 19
    assert_eq!(ring.get_by_index(19).map(|r| r.text.as_str()), Some("entry-0"));
}

#[test]
fn test_expanded_capacity_evicts_at_256() {
    let mut ring = HistoryRing::new();
    // Push 257 entries - oldest should be evicted
    for i in 0..257 {
        ring.push(content(&format!("e{i}")));
    }
    assert_eq!(ring.len(), 256);
    // Most recent
    assert_eq!(ring.get_by_index(0).map(|r| r.text.as_str()), Some("e256"));
    // Oldest remaining (e1, since e0 was evicted)
    assert_eq!(ring.get_by_index(255).map(|r| r.text.as_str()), Some("e1"));
}
