//! Tests for the `StaleCheck` trait contract.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use reovim_kernel::api::v1::BufferId;

use super::StaleCheck;

struct CountingCheck {
    calls: AtomicUsize,
}

impl StaleCheck for CountingCheck {
    fn refresh_if_stale(&self, _buffer: BufferId) {
        self.calls.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn counting_check_tracks_calls() {
    let check = Arc::new(CountingCheck {
        calls: AtomicUsize::new(0),
    });

    let trait_object: Arc<dyn StaleCheck> = Arc::clone(&check) as _;
    trait_object.refresh_if_stale(BufferId::from_raw(1));
    trait_object.refresh_if_stale(BufferId::from_raw(2));
    trait_object.refresh_if_stale(BufferId::from_raw(2));

    assert_eq!(check.calls.load(Ordering::Relaxed), 3);
}

#[test]
fn stale_check_is_object_safe() {
    // Compile-time check: trait must be usable as a trait object.
    fn accepts_dyn(_hook: Arc<dyn StaleCheck>) {}
    accepts_dyn(Arc::new(CountingCheck {
        calls: AtomicUsize::new(0),
    }));
}
