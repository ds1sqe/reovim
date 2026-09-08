#![forbid(unsafe_code)]
//! Composed public contracts; scripted acceptance is not a Core engine.

use reovim_kernel_interface_public_consumer::journey;

#[test]
fn heading_boot_open_edit_history_and_shutdown() {
    journey::exercise_heading_journey();
}

#[test]
fn keyed_model_uses_the_same_public_journey() {
    journey::exercise_keyed_journey();
}

#[test]
fn clients_choose_independent_semantic_selection_responses() {
    assert!(journey::exercise_client_responses());
}

#[test]
fn rejected_stale_cancelled_and_duplicate_requests_preserve_state() {
    journey::exercise_request_faults();
}

#[test]
fn notification_correlation_gaps_and_client_teardown_are_explicit() {
    journey::exercise_delivery_faults();
}

#[test]
fn notifications_reject_false_acceptance_and_mismatched_frontiers() {
    assert!(journey::exercise_notification_fences());
}

#[test]
fn failed_open_rolls_back_without_publishing_a_buffer() {
    journey::exercise_root_faults();
}
