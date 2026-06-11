//! Tests for `id.rs` — `DomainId`, `BufferId`, `WindowId` round-trip coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std selftest
//! runner via the `kernel-selftest` fixture bin.

use core::num::NonZeroU32;

use reovim_arch::arch_test;

use crate::id::{BufferId, DomainId, WindowId};

arch_test!(domain_id_round_trip, {
    let id = DomainId::new(NonZeroU32::new(7).unwrap());
    assert_eq!(id.as_u32(), 7);
});

arch_test!(domain_id_equality, {
    let a = DomainId::new(NonZeroU32::new(1).unwrap());
    let b = DomainId::new(NonZeroU32::new(1).unwrap());
    let c = DomainId::new(NonZeroU32::new(2).unwrap());
    assert_eq!(a, b, "same value → equal");
    assert_ne!(a, c, "different value → not equal");
});

arch_test!(buffer_id_round_trip, {
    let id = BufferId::new(42);
    assert_eq!(id.as_u32(), 42);
});

arch_test!(window_id_round_trip, {
    let id = WindowId::new(3);
    assert_eq!(id.as_u32(), 3);
});
