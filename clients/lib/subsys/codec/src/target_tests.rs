use crate::target::FrameTarget;

#[derive(Debug, PartialEq, Eq)]
struct CapA(u32);

#[derive(Debug, PartialEq, Eq)]
struct CapB {
    tag: &'static str,
    count: usize,
}

#[test]
fn new_target_is_empty() {
    let target = FrameTarget::new();
    assert!(target.is_empty());
    assert_eq!(target.len(), 0);
    assert!(!target.has::<CapA>());
    assert!(!target.has::<CapB>());
}

#[test]
fn default_equals_new() {
    let a = FrameTarget::new();
    let b = FrameTarget::default();
    assert_eq!(a.len(), b.len());
    assert!(a.is_empty() && b.is_empty());
}

#[test]
fn insert_and_get_mut_same_type_round_trip() {
    let mut target = FrameTarget::new();
    target.insert(CapA(42));

    let cap = target.get_mut::<CapA>().expect("slot present");
    assert_eq!(cap, &mut CapA(42));

    cap.0 = 99;
    let cap2 = target.get_mut::<CapA>().expect("slot still present");
    assert_eq!(cap2, &mut CapA(99));
}

#[test]
fn get_returns_none_for_missing_type() {
    let mut target = FrameTarget::new();
    target.insert(CapA(1));
    assert!(target.get::<CapB>().is_none());
    assert!(target.get_mut::<CapB>().is_none());
}

#[test]
fn has_reports_slot_presence_per_type() {
    let mut target = FrameTarget::new();
    assert!(!target.has::<CapA>());
    target.insert(CapA(1));
    assert!(target.has::<CapA>());
    assert!(!target.has::<CapB>());
    target.insert(CapB { tag: "hi", count: 2 });
    assert!(target.has::<CapA>());
    assert!(target.has::<CapB>());
}

#[test]
fn insert_same_type_replaces_prior_slot() {
    let mut target = FrameTarget::new();
    target.insert(CapA(1));
    target.insert(CapA(2));
    assert_eq!(target.len(), 1);
    assert_eq!(target.get::<CapA>(), Some(&CapA(2)));
}

#[test]
fn insert_different_types_coexist() {
    let mut target = FrameTarget::new();
    target.insert(CapA(7));
    target.insert(CapB { tag: "two", count: 2 });
    assert_eq!(target.len(), 2);
    assert_eq!(target.get::<CapA>(), Some(&CapA(7)));
    assert_eq!(
        target.get::<CapB>(),
        Some(&CapB { tag: "two", count: 2 })
    );
}

#[test]
fn remove_returns_owned_value_and_clears_slot() {
    let mut target = FrameTarget::new();
    target.insert(CapA(3));
    let removed = target.remove::<CapA>();
    assert_eq!(removed, Some(CapA(3)));
    assert!(!target.has::<CapA>());
    assert!(target.is_empty());
}

#[test]
fn remove_missing_type_returns_none() {
    let mut target = FrameTarget::new();
    assert!(target.remove::<CapA>().is_none());
}

#[test]
fn clear_drops_all_slots() {
    let mut target = FrameTarget::new();
    target.insert(CapA(1));
    target.insert(CapB { tag: "x", count: 9 });
    assert_eq!(target.len(), 2);
    target.clear();
    assert!(target.is_empty());
    assert!(!target.has::<CapA>());
    assert!(!target.has::<CapB>());
}

#[test]
fn get_mut_allows_in_place_mutation_of_complex_cap() {
    let mut target = FrameTarget::new();
    target.insert(CapB { tag: "start", count: 0 });

    let cap = target.get_mut::<CapB>().unwrap();
    cap.count = 10;
    cap.tag = "updated";

    assert_eq!(
        target.get::<CapB>(),
        Some(&CapB { tag: "updated", count: 10 })
    );
}

#[test]
fn debug_impl_reports_slot_count_without_any_bounds_on_slots() {
    let mut target = FrameTarget::new();
    target.insert(CapA(0));
    let rendered = format!("{target:?}");
    assert!(rendered.contains("FrameTarget"));
    assert!(rendered.contains("slot_count"));
    assert!(rendered.contains('1'));
}
