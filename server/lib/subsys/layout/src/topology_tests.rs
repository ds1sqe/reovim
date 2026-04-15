use super::*;

// ---------------------------------------------------------------------------
// Permil::new — valid boundary values
// ---------------------------------------------------------------------------

#[test]
fn permil_new_zero() {
    let p = Permil::new(0).expect("0 is valid");
    assert_eq!(p.value(), 0);
}

#[test]
fn permil_new_half() {
    let p = Permil::new(500).expect("500 is valid");
    assert_eq!(p.value(), 500);
}

#[test]
fn permil_new_full() {
    let p = Permil::new(1000).expect("1000 is valid");
    assert_eq!(p.value(), 1000);
}

// ---------------------------------------------------------------------------
// Permil::new — invalid values
// ---------------------------------------------------------------------------

#[test]
fn permil_new_invalid_1001() {
    assert_eq!(Permil::new(1001), Err(SplitError::InvalidRatio(1001)));
}

#[test]
fn permil_new_invalid_max() {
    assert_eq!(Permil::new(u16::MAX), Err(SplitError::InvalidRatio(u16::MAX)));
}

// ---------------------------------------------------------------------------
// Permil constants
// ---------------------------------------------------------------------------

#[test]
fn permil_full_constant() {
    assert_eq!(Permil::FULL.value(), 1000);
}

#[test]
fn permil_half_constant() {
    assert_eq!(Permil::HALF.value(), 500);
}

// ---------------------------------------------------------------------------
// LayoutTopology::Single
// ---------------------------------------------------------------------------

#[test]
fn layout_single() {
    let id = WindowId::from_raw(1);
    let topo = LayoutTopology::Single(id);
    let LayoutTopology::Single(got) = topo else {
        panic!("expected Single variant");
    };
    assert_eq!(got, id);
}

// ---------------------------------------------------------------------------
// LayoutTopology::Split nested
// ---------------------------------------------------------------------------

#[test]
fn layout_split_nested() {
    use crate::SplitDirection;

    let id_a = WindowId::from_raw(101);
    let id_b = WindowId::from_raw(102);
    let id_c = WindowId::from_raw(103);

    // Build: Vertical split with A on the left and (Horizontal B|C) on the right.
    let inner = LayoutTopology::Split {
        direction: SplitDirection::Horizontal,
        ratio: Permil::HALF,
        first: Box::new(LayoutTopology::Single(id_b)),
        second: Box::new(LayoutTopology::Single(id_c)),
    };

    let outer = LayoutTopology::Split {
        direction: SplitDirection::Vertical,
        ratio: Permil::new(400).expect("valid"),
        first: Box::new(LayoutTopology::Single(id_a)),
        second: Box::new(inner),
    };

    // Verify outer variant and ratio.
    let LayoutTopology::Split {
        direction,
        ratio,
        first,
        second,
    } = outer
    else {
        panic!("expected Split variant");
    };

    assert_eq!(direction, SplitDirection::Vertical);
    assert_eq!(ratio.value(), 400);

    // Verify first leaf.
    let LayoutTopology::Single(got_a) = *first else {
        panic!("expected Single in first");
    };
    assert_eq!(got_a, id_a);

    // Verify inner split.
    let LayoutTopology::Split {
        direction: inner_dir,
        first: inner_first,
        second: inner_second,
        ..
    } = *second
    else {
        panic!("expected nested Split in second");
    };
    assert_eq!(inner_dir, SplitDirection::Horizontal);

    let LayoutTopology::Single(got_b) = *inner_first else {
        panic!("expected Single b");
    };
    let LayoutTopology::Single(got_c) = *inner_second else {
        panic!("expected Single c");
    };
    assert_eq!(got_b, id_b);
    assert_eq!(got_c, id_c);
}

// ---------------------------------------------------------------------------
// SplitError Display
// ---------------------------------------------------------------------------

#[test]
fn split_error_display_invalid_ratio() {
    let msg = SplitError::InvalidRatio(1234).to_string();
    assert!(msg.contains("1234"), "message was: {msg}");
    assert!(msg.contains("1000"), "message was: {msg}");
}

#[test]
fn split_error_display_too_small() {
    let msg = SplitError::TooSmall.to_string();
    assert!(!msg.is_empty());
}
