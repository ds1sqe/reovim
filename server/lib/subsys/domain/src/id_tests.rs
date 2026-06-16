//! Tests for `id.rs` — `DomainId`, `BufferId`, `WindowId` round-trip coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std selftest
//! runner via the `kernel-selftest` fixture bin.

use core::num::NonZeroU32;

use reovim_arch::arch_test;

use crate::id::{
    BufferId, CdylibId, ClientId, DomainAttachmentId, DomainId, PendingAttachmentId, RegisterId,
    ReplayQueueId, ServiceKey, ServiceLeaseId, SessionId, SlotKindId, StreamId, WindowId,
};

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

arch_test!(session_and_client_ids_round_trip, {
    assert_eq!(SessionId::new(9).as_u32(), 9);
    assert_eq!(ClientId::new(10).as_u32(), 10);
});

arch_test!(attachment_ids_round_trip, {
    assert_eq!(DomainAttachmentId::new(11).as_u32(), 11);
    assert_eq!(PendingAttachmentId::new(12).as_u32(), 12);
    assert_eq!(ReplayQueueId::new(13).as_u32(), 13);
});

arch_test!(owner_and_registry_ids_round_trip, {
    let owner = CdylibId::new(NonZeroU32::new(14).unwrap());
    assert_eq!(owner.as_u32(), 14);
    assert_eq!(RegisterId::new(15).as_u32(), 15);
    assert_eq!(SlotKindId::new(16).as_u32(), 16);
});

arch_test!(service_and_stream_ids_round_trip, {
    assert_eq!(ServiceKey::new(17).as_u32(), 17);
    assert_eq!(ServiceLeaseId::new(18).as_u64(), 18);
    assert_eq!(StreamId::new(19).as_u64(), 19);
});
