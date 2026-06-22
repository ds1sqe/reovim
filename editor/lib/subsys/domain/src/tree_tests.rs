//! Tests for `tree.rs` — attachment refs, pending context, and focus records.

use core::num::NonZeroU32;

use {reovim_arch::arch_test, reovim_lib_ds::Bytes};

use crate::{
    carrier::{PositionCarrier, PositionHeader},
    id::{
        BufferId, ClientId, DomainAttachmentId, DomainId, PendingAttachmentId, ReplayQueueId,
        SessionId, WindowId,
    },
    tree::{
        DispatchVerdict, DomainAllocError, DomainAttachment, DomainScope, FocusEntry,
        FocusEntrySnapshot, FocusTransition, PendingAttachment,
    },
};

fn domain(raw: u32) -> DomainId {
    DomainId::new(NonZeroU32::new(raw).unwrap())
}

fn scope() -> DomainScope {
    DomainScope::new(
        PositionCarrier::new(PositionHeader::from_raw_parts(0, 0, 0), Bytes::new()),
        PositionCarrier::new(PositionHeader::from_raw_parts(0, 0, 0), Bytes::new()),
        0,
    )
}

arch_test!(attachment_detach_requires_no_focus_refs, {
    let cold = DomainAttachment::new(
        DomainAttachmentId::new(1),
        BufferId::new(1),
        domain(1),
        scope(),
        None,
        0,
        1,
        0,
    );
    assert!(cold.can_detach());

    let focused = DomainAttachment::resolved(
        DomainAttachmentId::new(2),
        BufferId::new(1),
        domain(1),
        scope(),
        None,
        1,
    );
    assert!(!focused.can_detach());
});

arch_test!(attachment_children_keep_append_order, {
    let mut parent = DomainAttachment::new(
        DomainAttachmentId::new(1),
        BufferId::new(1),
        domain(1),
        scope(),
        None,
        0,
        1,
        0,
    );
    parent
        .try_push_child(DomainAttachmentId::new(10))
        .expect("alloc");
    parent
        .try_push_child(DomainAttachmentId::new(11))
        .expect("alloc");
    assert_eq!(parent.children.as_slice()[0], DomainAttachmentId::new(10));
    assert_eq!(parent.children.as_slice()[1], DomainAttachmentId::new(11));
});

arch_test!(attachment_child_growth_failure_returns_domain_alloc_error, {
    let mut parent = DomainAttachment::new(
        DomainAttachmentId::new(1),
        BufferId::new(1),
        domain(1),
        scope(),
        None,
        0,
        1,
        0,
    );

    reovim_arch::alloc::fault::fail_after(0);
    let result = parent.try_push_child(DomainAttachmentId::new(10));
    reovim_arch::alloc::fault::reset();

    assert!(matches!(result, Err(DomainAllocError)));
    assert!(parent.children.is_empty());
});

arch_test!(focus_entry_helpers_distinguish_pending_and_resolved, {
    let pending = FocusEntry::Pending(PendingAttachmentId::new(5));
    let resolved = FocusEntry::Resolved(DomainAttachmentId::new(6));
    assert!(pending.is_pending());
    assert_eq!(pending.resolved_id(), None);
    assert_eq!(resolved.resolved_id(), Some(DomainAttachmentId::new(6)));
});

arch_test!(pending_attachment_owns_resolution_context, {
    let mut payload = Bytes::new();
    payload.try_extend_from_slice(b"boot").expect("alloc");
    let pending = PendingAttachment::new(
        PendingAttachmentId::new(1),
        domain(2),
        Some(DomainAttachmentId::new(7)),
        scope(),
        BufferId::new(3),
        ClientId::new(4),
        WindowId::new(5),
        payload,
        ReplayQueueId::new(6),
    );
    assert_eq!(pending.domain_id, domain(2));
    assert_eq!(pending.parent, Some(DomainAttachmentId::new(7)));
    assert_eq!(pending.bootstrap_payload.as_slice(), b"boot");
});

arch_test!(focus_transition_records_before_after_snapshots, {
    let mut transition = FocusTransition::new(
        SessionId::new(1),
        ClientId::new(2),
        BufferId::new(3),
        WindowId::new(4),
        99,
    );
    transition
        .before
        .try_push(FocusEntrySnapshot::Resolved(DomainAttachmentId::new(8), domain(1)))
        .expect("alloc");
    transition
        .after
        .try_push(FocusEntrySnapshot::Pending(PendingAttachmentId::new(9), domain(2)))
        .expect("alloc");
    assert_eq!(transition.seq, 99);
    assert_eq!(transition.before.as_slice()[0].domain_id(), domain(1));
    assert_eq!(transition.after.as_slice()[0].domain_id(), domain(2));
});

arch_test!(dispatch_verdict_stops_for_claimed_and_pending, {
    assert!(DispatchVerdict::Claimed.stops_walk());
    assert!(!DispatchVerdict::Ignored.stops_walk());
    assert!(DispatchVerdict::QueuedPending(PendingAttachmentId::new(1)).stops_walk());
});
