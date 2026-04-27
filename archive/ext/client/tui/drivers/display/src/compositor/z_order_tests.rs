use super::*;

#[test]
fn test_z_group_ordering() {
    assert!(ZGroup::Base < ZGroup::Sidebar);
    assert!(ZGroup::Sidebar < ZGroup::Editor);
    assert!(ZGroup::Editor < ZGroup::Floating);
    assert!(ZGroup::Floating < ZGroup::Overlay);
    assert!(ZGroup::Overlay < ZGroup::Popup);
    assert!(ZGroup::Popup < ZGroup::Panel);
    assert!(ZGroup::Panel < ZGroup::Modal);
    assert!(ZGroup::Modal < ZGroup::Alert);
}

#[test]
fn test_zgroup_values_match_spec() {
    // CRITICAL: These values are part of the API contract!
    assert_eq!(ZGroup::Base as u16, 0);
    assert_eq!(ZGroup::Sidebar as u16, 100);
    assert_eq!(ZGroup::Editor as u16, 200);
    assert_eq!(ZGroup::Floating as u16, 300);
    assert_eq!(ZGroup::Overlay as u16, 400);
    assert_eq!(ZGroup::Popup as u16, 500);
    assert_eq!(ZGroup::Panel as u16, 600);
    assert_eq!(ZGroup::Modal as u16, 700);
    assert_eq!(ZGroup::Alert as u16, 800);
}

#[test]
fn test_z_group_value_method() {
    assert_eq!(ZGroup::Base.value(), 0);
    assert_eq!(ZGroup::Alert.value(), 800);
}

#[test]
fn test_z_order_ordering_across_groups() {
    let base = ZOrder::base();
    let editor = ZOrder::editor(0);
    let modal = ZOrder::modal(0);
    assert!(base < editor);
    assert!(editor < modal);
}

#[test]
fn test_z_order_sub_order_within_group() {
    let low = ZOrder::editor(0);
    let high = ZOrder::editor(10);
    assert!(low < high);
}

#[test]
fn test_z_order_sequence_tie_breaking() {
    let first = ZOrder::editor(0);
    let second = ZOrder::editor(0);
    // Same group and sub_order, but second has higher sequence (global counter)
    assert!(first < second);
}

#[test]
fn test_z_order_bring_to_front() {
    let mut z = ZOrder::editor(5);
    let old_seq = z.sequence;
    z.bring_to_front();
    assert!(z.sequence > old_seq);
    assert_eq!(z.group, ZGroup::Editor); // Group unchanged
    assert_eq!(z.sub_order, 5); // Sub-order unchanged
}

#[test]
fn test_z_order_send_to_back() {
    let mut z = ZOrder::editor(5);
    z.bring_to_front(); // Ensure non-zero sequence
    z.send_to_back();
    assert_eq!(z.sequence, 0);
    assert_eq!(z.group, ZGroup::Editor); // Group unchanged
    assert_eq!(z.sub_order, 5); // Sub-order unchanged
}

#[test]
fn test_z_order_all_constructors() {
    // Verify all 9 constructors exist and produce correct groups
    assert_eq!(ZOrder::base().group, ZGroup::Base);
    assert_eq!(ZOrder::sidebar(0).group, ZGroup::Sidebar);
    assert_eq!(ZOrder::editor(0).group, ZGroup::Editor);
    assert_eq!(ZOrder::floating(0).group, ZGroup::Floating);
    assert_eq!(ZOrder::overlay(0).group, ZGroup::Overlay);
    assert_eq!(ZOrder::popup(0).group, ZGroup::Popup);
    assert_eq!(ZOrder::panel(0).group, ZGroup::Panel);
    assert_eq!(ZOrder::modal(0).group, ZGroup::Modal);
    assert_eq!(ZOrder::alert(0).group, ZGroup::Alert);
}
