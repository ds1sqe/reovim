use {super::*, crate::wire::OverlayState};

// Interaction tests
#[test]
fn test_interaction_select_next_prev() {
    assert!(Interaction::SelectNext.is_navigation());
    assert!(Interaction::SelectPrev.is_navigation());
    assert!(!Interaction::SelectNext.is_terminal());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_interaction_select_index() {
    let interaction = Interaction::SelectIndex(5);
    assert!(interaction.is_navigation());
    if let Interaction::SelectIndex(idx) = interaction {
        assert_eq!(idx, 5);
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_interaction_filter() {
    let interaction = Interaction::filter("hello");
    assert!(!interaction.is_navigation());
    assert!(!interaction.is_terminal());
    if let Interaction::Filter(text) = interaction {
        assert_eq!(text, "hello");
    }
}

#[test]
fn test_interaction_confirm_cancel() {
    assert!(Interaction::Confirm.is_terminal());
    assert!(Interaction::Cancel.is_terminal());
    assert!(!Interaction::Confirm.is_navigation());
}

#[test]
fn test_interaction_scroll() {
    assert!(Interaction::ScrollDown(5).is_navigation());
    assert!(Interaction::ScrollUp(5).is_navigation());
    assert!(Interaction::PageDown.is_navigation());
    assert!(Interaction::PageUp.is_navigation());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_interaction_custom() {
    let interaction = Interaction::custom("my-action");
    assert!(!interaction.is_navigation());
    assert!(!interaction.is_terminal());
    if let Interaction::Custom(action) = interaction {
        assert_eq!(action, "my-action");
    }
}

// InteractionResult tests
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_result_state_update() {
    let state = OverlayState::with_selection(3);
    let result = InteractionResult::state_update(state.clone());
    assert!(!result.closes_overlay());
    assert!(result.was_handled());
    if let InteractionResult::StateUpdate(s) = result {
        assert_eq!(s, state);
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_result_confirm() {
    let result = InteractionResult::confirm(serde_json::json!({"item": "foo"}));
    assert!(result.closes_overlay());
    assert!(result.was_handled());
    if let InteractionResult::Confirm(value) = result {
        assert_eq!(value["item"], "foo");
    }
}

#[test]
fn test_result_cancel() {
    let result = InteractionResult::Cancel;
    assert!(result.closes_overlay());
    assert!(result.was_handled());
}

#[test]
fn test_result_pass_through() {
    let result = InteractionResult::PassThrough;
    assert!(!result.closes_overlay());
    assert!(!result.was_handled());
}

#[test]
fn test_interaction_not_terminal() {
    assert!(!Interaction::SelectNext.is_terminal());
    assert!(!Interaction::SelectPrev.is_terminal());
    assert!(!Interaction::ScrollDown(1).is_terminal());
    assert!(!Interaction::ScrollUp(1).is_terminal());
    assert!(!Interaction::PageDown.is_terminal());
    assert!(!Interaction::PageUp.is_terminal());
    assert!(!Interaction::SelectIndex(0).is_terminal());
    assert!(!Interaction::filter("x").is_terminal());
    assert!(!Interaction::custom("x").is_terminal());
}

#[test]
fn test_interaction_not_navigation() {
    assert!(!Interaction::Confirm.is_navigation());
    assert!(!Interaction::Cancel.is_navigation());
    assert!(!Interaction::filter("x").is_navigation());
    assert!(!Interaction::custom("x").is_navigation());
}

#[test]
fn test_result_consumed() {
    let result = InteractionResult::Consumed;
    assert!(!result.closes_overlay());
    assert!(result.was_handled());
}
