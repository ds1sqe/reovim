//! Tests for InputSequence — opaque payload sequence.

use {
    super::input_sequence::InputSequence,
    std::collections::HashSet,
};

fn make_payload(kind: u16, extra: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 + extra.len());
    v.extend(kind.to_le_bytes());
    v.extend([0u8; 6]); // flags + context
    v.extend_from_slice(extra);
    v
}

#[test]
fn default_is_empty() {
    let seq = InputSequence::default();
    assert!(seq.is_empty());
    assert_eq!(seq.len(), 0);
    assert!(seq.as_slice().is_empty());
}

#[test]
fn new_is_empty() {
    let seq = InputSequence::new();
    assert!(seq.is_empty());
}

#[test]
fn push_and_len() {
    let mut seq = InputSequence::new();
    seq.push(make_payload(1, &[0x41]));
    assert_eq!(seq.len(), 1);
    assert!(!seq.is_empty());
    seq.push(make_payload(1, &[0x42]));
    assert_eq!(seq.len(), 2);
}

#[test]
fn clear_empties_sequence() {
    let mut seq = InputSequence::new();
    seq.push(make_payload(1, &[]));
    seq.push(make_payload(2, &[]));
    seq.clear();
    assert!(seq.is_empty());
    assert_eq!(seq.len(), 0);
}

#[test]
fn as_slice_preserves_order() {
    let mut seq = InputSequence::new();
    let p1 = make_payload(1, &[0xAA]);
    let p2 = make_payload(2, &[0xBB]);
    seq.push(p1.clone());
    seq.push(p2.clone());
    let slice = seq.as_slice();
    assert_eq!(slice[0], p1);
    assert_eq!(slice[1], p2);
}

#[test]
fn starts_with_empty_always_true() {
    let mut seq = InputSequence::new();
    seq.push(make_payload(1, &[]));
    let empty = InputSequence::new();
    assert!(seq.starts_with(&empty));

    let also_empty = InputSequence::new();
    assert!(also_empty.starts_with(&empty));
}

#[test]
fn starts_with_self_is_true() {
    let mut seq = InputSequence::new();
    seq.push(make_payload(1, &[0xAA]));
    seq.push(make_payload(2, &[0xBB]));
    let clone = seq.clone();
    assert!(seq.starts_with(&clone));
}

#[test]
fn starts_with_longer_other_is_false() {
    let mut short = InputSequence::new();
    short.push(make_payload(1, &[]));

    let mut longer = InputSequence::new();
    longer.push(make_payload(1, &[]));
    longer.push(make_payload(2, &[]));

    assert!(!short.starts_with(&longer));
}

#[test]
fn starts_with_prefix_match() {
    let p1 = make_payload(1, &[0xAA]);
    let p2 = make_payload(2, &[0xBB]);

    let mut full = InputSequence::new();
    full.push(p1.clone());
    full.push(p2.clone());

    let mut prefix = InputSequence::new();
    prefix.push(p1.clone());

    assert!(full.starts_with(&prefix));
    assert!(!prefix.starts_with(&full));
}

#[test]
fn starts_with_different_first_element_is_false() {
    let mut seq = InputSequence::new();
    seq.push(make_payload(1, &[0xAA]));

    let mut other = InputSequence::new();
    other.push(make_payload(1, &[0xBB])); // different body

    assert!(!seq.starts_with(&other));
}

#[test]
fn equality_and_hash_consistent() {
    let p = make_payload(1, &[0xCC]);
    let mut a = InputSequence::new();
    a.push(p.clone());
    let mut b = InputSequence::new();
    b.push(p.clone());

    assert_eq!(a, b);

    let mut set = HashSet::new();
    set.insert(a.clone());
    assert!(set.contains(&b));
}

#[test]
fn different_sequences_not_equal() {
    let mut a = InputSequence::new();
    a.push(make_payload(1, &[0xAA]));

    let mut b = InputSequence::new();
    b.push(make_payload(1, &[0xBB]));

    assert_ne!(a, b);
}

#[test]
fn clone_produces_independent_copy() {
    let mut original = InputSequence::new();
    original.push(make_payload(1, &[0x01]));
    let mut clone = original.clone();
    clone.push(make_payload(2, &[0x02]));
    // original should be unchanged
    assert_eq!(original.len(), 1);
    assert_eq!(clone.len(), 2);
}
