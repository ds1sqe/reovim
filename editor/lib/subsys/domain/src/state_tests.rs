//! Tests for `state.rs` — view slots, registers, and byte-range undo contracts.

use core::num::NonZeroU32;

use {reovim_arch::arch_test, reovim_lib_ds::Bytes};

use crate::{
    id::{BufferId, CdylibId, ClientId, RegisterId, SlotKindId, WindowId},
    state::{
        EditOrigin, EditRecord, RegisterKey, RegisterScope, UndoGroup, UndoGroupId, UndoStack,
        ViewSlotFlags, ViewSlotKey, ViewSlotScope,
    },
};

fn owner(raw: u32) -> CdylibId {
    CdylibId::new(NonZeroU32::new(raw).unwrap())
}

arch_test!(view_slot_keys_encode_window_and_buffer_scopes, {
    let window = ViewSlotKey::window(
        ClientId::new(1),
        BufferId::new(2),
        WindowId::new(3),
        SlotKindId::new(4),
    );
    assert_eq!(window.scope(), ViewSlotScope::Window);
    assert_eq!(window.buffer_id(), BufferId::new(2));
    assert_eq!(window.kind_id(), SlotKindId::new(4));

    let buffer = ViewSlotKey::buffer(BufferId::new(5), SlotKindId::new(6));
    assert_eq!(buffer.scope(), ViewSlotScope::Buffer);
    assert_eq!(buffer.buffer_id(), BufferId::new(5));
});

arch_test!(view_slot_flags_report_known_bits, {
    let flags = ViewSlotFlags::new(ViewSlotFlags::HOSTAPI_REENTRANT | ViewSlotFlags::SEND_SAFE);
    assert!(flags.hostapi_reentrant());
    assert!(flags.contains(ViewSlotFlags::SEND_SAFE));
    assert!(!flags.contains(ViewSlotFlags::DROP_MAY_CALL_HOSTAPI));
});

arch_test!(register_scope_lookup_order_is_client_session_system, {
    assert_eq!(RegisterScope::Client.lookup_rank(), 0);
    assert_eq!(RegisterScope::Session.lookup_rank(), 1);
    assert_eq!(RegisterScope::System.lookup_rank(), 2);
    let key = RegisterKey::new(RegisterScope::Client, RegisterId::new(9));
    assert_eq!(key.id, RegisterId::new(9));
});

arch_test!(edit_origin_controls_undo_participation, {
    assert!(
        EditOrigin::User {
            client_id: ClientId::new(1),
        }
        .participates_in_undo()
    );
    assert!(
        EditOrigin::Module {
            cdylib_id: owner(1)
        }
        .participates_in_undo()
    );
    assert!(!EditOrigin::External.participates_in_undo());
    assert!(!EditOrigin::Restore.participates_in_undo());
    assert!(!EditOrigin::Replay.participates_in_undo());
});

arch_test!(edit_record_retains_old_and_new_bytes, {
    let old = Bytes::try_from_slice(b"old").expect("alloc");
    let new = Bytes::try_from_slice(b"new").expect("alloc");
    let record = EditRecord::new(1, 4, old, new);
    assert_eq!(record.start, 1);
    assert_eq!(record.end, 4);
    assert_eq!(record.old_bytes.as_slice(), b"old");
    assert_eq!(record.new_bytes.as_slice(), b"new");
    assert!(!record.is_noop());
});

arch_test!(undo_group_and_stack_track_open_group, {
    let mut group = UndoGroup::new(
        UndoGroupId::new(1),
        EditOrigin::User {
            client_id: ClientId::new(2),
        },
        123,
    );
    group
        .edits
        .try_push(EditRecord::new(0, 0, Bytes::new(), Bytes::new()))
        .expect("alloc");
    assert!(group.is_undoable());
    assert_eq!(group.edits.as_slice().len(), 1);

    let mut stack = UndoStack::new();
    stack.group_open = Some(UndoGroupId::new(1));
    stack.groups.try_push(group).expect("alloc");
    assert_eq!(stack.group_open, Some(UndoGroupId::new(1)));
    assert_eq!(stack.groups.as_slice().len(), 1);
});
