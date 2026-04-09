//! Tests for the codec-side [`StaleCheck`] adapter.

use {reovim_driver_session::StaleCheck, reovim_kernel::api::v1::BufferId};

use super::{InodeStaleCheck, install};

#[test]
fn new_adapter_starts_with_zero_calls() {
    let adapter = InodeStaleCheck::new();
    assert_eq!(adapter.call_count(), 0);
}

#[test]
fn refresh_increments_counter() {
    let adapter = InodeStaleCheck::new();
    adapter.refresh_if_stale(BufferId::from_raw(1));
    assert_eq!(adapter.call_count(), 1);

    adapter.refresh_if_stale(BufferId::from_raw(2));
    adapter.refresh_if_stale(BufferId::from_raw(3));
    assert_eq!(adapter.call_count(), 3);
}

#[test]
fn install_returns_working_adapter() {
    let hook = install();
    // Compile-time: install returns Arc<dyn StaleCheck>.
    hook.refresh_if_stale(BufferId::from_raw(99));
}

#[test]
fn default_adapter_is_equivalent_to_new() {
    let fresh = InodeStaleCheck::default();
    assert_eq!(fresh.call_count(), 0);
}
