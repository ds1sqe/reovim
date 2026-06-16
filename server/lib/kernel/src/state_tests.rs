//! Tests for `state.rs` — Phase 3 state substrate storage mechanics.
//!
//! Registered under the `selftest` feature; runs on the arch no_std selftest
//! runner via the `kernel-selftest` fixture bin.

use core::{
    num::NonZeroU32,
    sync::atomic::{AtomicUsize, Ordering},
};

use {
    reovim_arch::{arch_test, ds::Bytes},
    reovim_subsys_domain::{
        carrier::{CarrierStatus, CursorCarrier, CursorHeader, PositionCarrier, PositionHeader},
        id::{
            BufferId, CdylibId, ClientId, DomainId, RegisterId, ServiceKey, SessionId, SlotKindId,
            WindowId,
        },
        routing::RegistrationPhase,
        service::{ServiceAccessError, ServiceDescriptorMeta, ServiceFlags, ServiceRowState},
        state::{EditOrigin, RegisterScope, ViewSlotFlags, ViewSlotKey},
        stream::{StreamControlClass, StreamControlOp, StreamScheme, StreamState},
    },
};

use crate::{
    Init, LauncherArgs,
    event_bus::{DS12Event, EVT_CARRIER_VALIDATION_ERROR},
    router::DomainApiVersion,
    state::{
        BufferRecord, CarrierKernelLimits, KernelStateLimits, ServiceRegistry, StateSubstrate,
    },
};

static CARRIER_EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);

fn count_carrier_event(event: &DS12Event) {
    if event.event == EVT_CARRIER_VALIDATION_ERROR {
        CARRIER_EVENT_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

fn owner(raw: u32) -> CdylibId {
    CdylibId::new(NonZeroU32::new(raw).unwrap())
}

fn domain(raw: u32) -> DomainId {
    DomainId::new(NonZeroU32::new(raw).unwrap())
}

fn carrier(domain_raw: u32, content: &[u8]) -> CursorCarrier {
    cursor_carrier(domain_raw, 1, content)
}

fn cursor_carrier(domain_raw: u32, inner_id: u16, content: &[u8]) -> CursorCarrier {
    CursorCarrier::new(
        CursorHeader::from_raw_parts(domain_raw, inner_id, 0),
        Bytes::try_from_slice(content).expect("carrier content"),
    )
}

fn position_carrier(domain_raw: u32, inner_id: u16, content: &[u8]) -> PositionCarrier {
    PositionCarrier::new(
        PositionHeader::from_raw_parts(domain_raw, inner_id, 0),
        Bytes::try_from_slice(content).expect("carrier content"),
    )
}

fn stream_scheme(name: &[u8], owner_cdylib_id: CdylibId) -> StreamScheme {
    StreamScheme::new(
        Bytes::try_from_slice(name).expect("scheme name"),
        owner_cdylib_id,
        DomainApiVersion::new(0, 16),
    )
}

fn service_meta(key: ServiceKey, owner_cdylib_id: CdylibId) -> ServiceDescriptorMeta {
    ServiceDescriptorMeta::new(
        key,
        owner_cdylib_id,
        DomainApiVersion::new(0, 16),
        DomainApiVersion::new(0, 16),
        Bytes::try_from_slice(b"fixture.service").expect("type name"),
        16,
        ServiceFlags::new(ServiceFlags::SEND_SAFE | ServiceFlags::SYNC_SAFE),
    )
}

arch_test!(state_registers_validate_carriers_and_follow_scope_fallback, {
    let mut state = StateSubstrate::new();
    let client = ClientId::new(1);
    let session = reovim_subsys_domain::SessionId::new(2);
    let register = RegisterId::new(3);

    let invalid = carrier(0, b"bad");
    assert!(
        state
            .set_register(RegisterScope::System, client, session, register, &invalid)
            .is_err(),
        "non-empty cursor carrier with domain_id 0 is rejected"
    );

    let system = carrier(1, b"system");
    let session_value = carrier(1, b"session");
    let client_value = carrier(1, b"client");
    state
        .set_register(RegisterScope::System, client, session, register, &system)
        .expect("system register");
    assert_eq!(
        state
            .lookup_register(client, session, register)
            .expect("system fallback")
            .content
            .as_slice(),
        b"system"
    );

    state
        .set_register(RegisterScope::Session, client, session, register, &session_value)
        .expect("session register");
    assert_eq!(
        state
            .lookup_register(client, session, register)
            .expect("session fallback")
            .content
            .as_slice(),
        b"session"
    );

    state
        .set_register(RegisterScope::Client, client, session, register, &client_value)
        .expect("client register");
    assert_eq!(
        state
            .lookup_register(client, session, register)
            .expect("client value")
            .content
            .as_slice(),
        b"client"
    );

    assert!(state.clear_register(RegisterScope::Client, client, session, register));
    assert_eq!(
        state
            .lookup_register(client, session, register)
            .expect("session fallback after client clear")
            .content
            .as_slice(),
        b"session"
    );
});

arch_test!(state_register_caps_enforce_scope_counts_and_carrier_bytes, {
    let limits = KernelStateLimits::new(16, 4, 100).with_register_limits(3, 1, 1, 1);
    let mut state = StateSubstrate::with_limits(limits);
    let client = ClientId::new(1);
    let session = reovim_subsys_domain::SessionId::new(2);

    state
        .set_register(
            RegisterScope::Client,
            client,
            session,
            RegisterId::new(1),
            &carrier(1, b"abc"),
        )
        .expect("first client register");
    assert!(
        state
            .set_register(
                RegisterScope::Client,
                client,
                session,
                RegisterId::new(2),
                &carrier(1, b"d"),
            )
            .is_err(),
        "second client register exceeds per-client cap"
    );
    assert!(
        state
            .set_register(
                RegisterScope::Session,
                client,
                session,
                RegisterId::new(3),
                &carrier(1, b"toolong"),
            )
            .is_err(),
        "carrier byte cap enforced"
    );
});

arch_test!(state_carrier_codecs_track_known_opaque_invalid_and_revalidation, {
    let limits = KernelStateLimits::new(16, 4, 100).with_carrier_limits(CarrierKernelLimits {
        max_position_carrier_bytes: 4,
        max_cursor_carrier_bytes: 4,
        max_opaque_carrier_bytes_per_session: 8,
        max_deferred_restore_carriers: 8,
        max_codecs_per_domain: 2,
        max_domain_table_entries: 8,
        max_codec_display_bytes: 8,
        max_invalid_carrier_per_min: 2,
    });
    let mut state = StateSubstrate::with_limits(limits);
    let session = SessionId::new(1);
    let owner_id = owner(1);
    let known = position_carrier(1, 7, b"abcd");

    let opaque = state
        .validate_position_carrier(session, 10, 0, &known)
        .expect("unknown codec validates opaque");
    assert_eq!(opaque.status, CarrierStatus::ValidOpaque);
    assert!(!opaque.emit_ds12);

    state
        .register_position_codec(domain(1), 7, owner_id, 1, 4)
        .expect("register position codec");
    assert_eq!(state.codec_registry().len(), 1);
    let report = state
        .validate_position_carrier(session, 10, 1, &known)
        .expect("known codec validates");
    assert_eq!(report.status, CarrierStatus::ValidKnown);
    assert!(!report.emit_ds12);

    let too_long = position_carrier(1, 7, b"abcde");
    assert!(
        state
            .validate_position_carrier(session, 22, 2, &too_long)
            .expect("invalid report")
            .emit_ds12,
        "first invalid event is emitted"
    );
    assert!(
        state
            .validate_position_carrier(session, 22, 3, &too_long)
            .expect("invalid report")
            .emit_ds12,
        "second invalid event is emitted within rate cap"
    );
    assert!(
        !state
            .validate_position_carrier(session, 22, 4, &too_long)
            .expect("invalid report")
            .emit_ds12,
        "third invalid event is rate-limited"
    );

    assert_eq!(state.revoke_codec_owner(owner_id, 2).expect("revoke"), 1);
    assert_eq!(
        state
            .validate_position_carrier(session, 10, 5, &known)
            .expect("revoked codec validates opaque")
            .status,
        CarrierStatus::ValidOpaque
    );
    state
        .register_position_codec(domain(1), 7, owner_id, 2, 4)
        .expect("re-register owner generation");
    assert!(
        state
            .register_position_codec(domain(1), 7, owner_id, 1, 4)
            .is_err(),
        "stale owner generation cannot replace a newer row"
    );
    assert_eq!(
        state
            .validate_position_carrier(session, 10, 6, &known)
            .expect("known after re-register")
            .status,
        CarrierStatus::ValidKnown
    );
});

arch_test!(state_carrier_registry_enforces_codec_and_opaque_caps, {
    let limits = KernelStateLimits::new(16, 4, 100).with_carrier_limits(CarrierKernelLimits {
        max_position_carrier_bytes: 8,
        max_cursor_carrier_bytes: 8,
        max_opaque_carrier_bytes_per_session: 3,
        max_deferred_restore_carriers: 8,
        max_codecs_per_domain: 1,
        max_domain_table_entries: 8,
        max_codec_display_bytes: 8,
        max_invalid_carrier_per_min: 8,
    });
    let mut state = StateSubstrate::with_limits(limits);
    let session = SessionId::new(2);
    let owner_id = owner(2);

    state
        .register_position_codec(domain(1), 1, owner_id, 1, 8)
        .expect("first codec row");
    assert!(
        state
            .register_cursor_codec(domain(1), 2, owner_id, 1, 8)
            .is_err(),
        "per-domain codec cap spans carrier kinds"
    );

    let empty_opaque = cursor_carrier(0, 0, b"");
    assert_eq!(
        state
            .validate_cursor_carrier(session, 1, 0, &empty_opaque)
            .expect("empty opaque")
            .status,
        CarrierStatus::ValidOpaque
    );
    let invalid_zero_domain = cursor_carrier(0, 0, b"x");
    assert_eq!(
        state
            .validate_cursor_carrier(session, 1, 1, &invalid_zero_domain)
            .expect("invalid zero-domain report")
            .status,
        CarrierStatus::Invalid
    );

    let unknown = cursor_carrier(2, 9, b"ab");
    assert_eq!(
        state
            .validate_cursor_carrier(session, 1, 2, &unknown)
            .expect("first opaque bytes")
            .status,
        CarrierStatus::ValidOpaque
    );
    assert_eq!(
        state
            .validate_cursor_carrier(session, 1, 3, &unknown)
            .expect("opaque cap exceeded")
            .status,
        CarrierStatus::Invalid
    );
});

arch_test!(state_stream_registry_enforces_init_window_and_lifecycle, {
    let limits = KernelStateLimits::new(16, 4, 100).with_stream_limits(1, 1, 16, 16, 100);
    let mut state = StateSubstrate::with_limits(limits);
    let owner_id = owner(4);
    let session = SessionId::new(4);
    let buffer = BufferId::new(4);

    assert!(
        state
            .register_stream_scheme(stream_scheme(b"pipe", owner_id), RegistrationPhase::Active)
            .is_err(),
        "stream schemes are init-only"
    );
    state
        .register_stream_scheme(stream_scheme(b"pipe", owner_id), RegistrationPhase::Init)
        .expect("register scheme");
    assert_eq!(state.stream_registry().scheme_count(), 1);
    assert!(
        state
            .register_stream_scheme(stream_scheme(b"pipe", owner_id), RegistrationPhase::Init)
            .is_err(),
        "scheme names are unique"
    );
    assert!(
        state
            .register_stream_scheme(stream_scheme(b"other", owner_id), RegistrationPhase::Init)
            .is_err(),
        "scheme cap enforced"
    );

    let stream = state
        .open_stream(b"pipe", Some(session), Some(buffer))
        .expect("open stream");
    assert_eq!(stream.as_u64(), 1);
    assert!(
        state
            .open_stream(b"pipe", Some(session), Some(BufferId::new(5)))
            .is_err(),
        "per-session stream cap enforced"
    );
    assert_eq!(
        state
            .stream_registry()
            .handle(stream)
            .expect("handle")
            .state,
        StreamState::Running
    );
    state.stream_mark_stale(stream).expect("mark stale");
    assert_eq!(
        state
            .stream_registry()
            .handle(stream)
            .expect("handle")
            .state,
        StreamState::Stale
    );
    assert!(state.stream_emit(stream, b"x", 10).is_err());
    state.stream_close(stream).expect("close");
    assert_eq!(
        state
            .stream_registry()
            .handle(stream)
            .expect("handle")
            .state,
        StreamState::Closed
    );

    let reopened = state
        .open_stream(b"pipe", Some(session), Some(buffer))
        .expect("reopen after close");
    assert_eq!(state.revoke_stream_owner(owner_id).expect("revoke owner"), 2);
    assert_eq!(
        state
            .stream_registry()
            .handle(reopened)
            .expect("reopened handle")
            .state,
        StreamState::Draining
    );
});

arch_test!(state_stream_registry_queues_subscriptions_and_applies_byte_edits, {
    let limits = KernelStateLimits::new(16, 4, 100).with_stream_limits(4, 4, 64, 64, 100);
    let mut state = StateSubstrate::with_limits(limits);
    let owner_id = owner(5);
    let session = SessionId::new(5);
    let buffer_one = BufferId::new(51);
    let buffer_two = BufferId::new(52);

    state
        .register_stream_scheme(stream_scheme(b"pipe", owner_id), RegistrationPhase::Init)
        .expect("register scheme");
    let stream = state
        .open_stream(b"pipe", Some(session), Some(buffer_one))
        .expect("open stream");
    assert!(
        state
            .stream_subscribe(stream, session, buffer_one, 8)
            .expect("subscribe one")
    );
    assert!(
        !state
            .stream_subscribe(stream, session, buffer_one, 8)
            .expect("duplicate subscription"),
        "duplicate subscriptions are idempotent"
    );
    assert!(
        state
            .stream_subscribe(stream, session, buffer_two, 4)
            .expect("subscribe two")
    );
    assert_eq!(state.stream_registry().subscription_count(stream), 2);

    let counters = state.stream_emit(stream, b"abc", 10).expect("emit abc");
    assert_eq!(counters.bytes_in_flight, 6);
    assert_eq!(counters.bytes_buffered, 6);
    let drained = state
        .stream_drain_subscription(stream, buffer_one, 20)
        .expect("drain one")
        .expect("subscription bytes");
    assert_eq!(drained.as_slice(), b"abc");
    let handle = state.stream_registry().handle(stream).expect("handle");
    assert_eq!(handle.backpressure.bytes_in_flight, 3);
    assert_eq!(handle.backpressure.bytes_buffered, 3);

    assert!(
        state.stream_emit(stream, b"de", 30).is_err(),
        "second subscription hits its per-buffer cap"
    );
    assert_eq!(
        state
            .stream_registry()
            .handle(stream)
            .expect("handle")
            .state,
        StreamState::BackpressureBlocked
    );
    assert_eq!(
        state
            .stream_drain_subscription(stream, buffer_two, 40)
            .expect("drain two")
            .expect("subscription bytes")
            .as_slice(),
        b"abc"
    );
    assert_eq!(
        state
            .stream_registry()
            .handle(stream)
            .expect("handle")
            .state,
        StreamState::Running
    );

    assert_eq!(
        state
            .stream_control_class(stream, StreamControlOp::KERNEL_DRAIN)
            .expect("kernel op"),
        StreamControlClass::Kernel
    );
    assert_eq!(
        state
            .stream_control_class(stream, StreamControlOp::new(101))
            .expect("scheme-private op"),
        StreamControlClass::SchemePrivate
    );
    assert!(
        state
            .stream_control_class(stream, StreamControlOp::new(0))
            .is_err()
    );
    assert!(
        state
            .stream_control_class(stream, StreamControlOp::new(300))
            .is_err()
    );

    state.stream_emit(stream, b"xy", 50).expect("emit xy");
    assert!(
        state
            .stream_drain_to_buffer(stream, buffer_one, 60)
            .expect("drain to buffer"),
        "queued bytes applied to buffer"
    );
    assert_eq!(
        state
            .buffer_bytes(buffer_one)
            .expect("buffer bytes")
            .expect("buffer")
            .as_slice(),
        b"xy"
    );
});

arch_test!(state_view_slots_are_isolated_capped_and_dropped_by_owner, {
    let mut state = StateSubstrate::with_limits(KernelStateLimits::new(4, 1, 100));
    let client = ClientId::new(1);
    let buffer = BufferId::new(2);
    let window = WindowId::new(3);
    let owner_id = owner(1);
    let key = ViewSlotKey::window(client, buffer, window, SlotKindId::new(4));
    let other_window_key =
        ViewSlotKey::window(client, buffer, WindowId::new(9), SlotKindId::new(4));
    let second_slot_same_window = ViewSlotKey::window(client, buffer, window, SlotKindId::new(5));

    state
        .put_view_slot(key, owner_id, ViewSlotFlags::new(ViewSlotFlags::SEND_SAFE), b"slot")
        .expect("window slot");
    assert_eq!(state.view_slot(key).expect("slot").bytes.as_slice(), b"slot");
    assert!(
        state.view_slot(other_window_key).is_none(),
        "window key must not see another window's slot"
    );
    assert!(
        state
            .put_view_slot(second_slot_same_window, owner_id, ViewSlotFlags::new(0), b"next")
            .is_err(),
        "per-window slot cap enforced"
    );
    assert!(
        state
            .put_view_slot(key, owner_id, ViewSlotFlags::new(0), b"too long")
            .is_err(),
        "slot byte cap enforced"
    );

    let buffer_key = ViewSlotKey::buffer(buffer, SlotKindId::new(6));
    state
        .put_view_slot(buffer_key, owner_id, ViewSlotFlags::new(0), b"buf")
        .expect("buffer slot");
    assert_eq!(state.drop_owner_view_slots(owner_id).expect("drop owner"), 2);
    assert!(state.view_slot(key).is_none());
    assert!(state.view_slot(buffer_key).is_none());
});

arch_test!(state_view_slots_drop_window_and_buffer_scopes_on_detach, {
    let mut state = StateSubstrate::new();
    let client = ClientId::new(1);
    let buffer = BufferId::new(2);
    let window = WindowId::new(3);
    let other_window = WindowId::new(4);
    let owner_id = owner(1);
    let window_key = ViewSlotKey::window(client, buffer, window, SlotKindId::new(1));
    let other_window_key = ViewSlotKey::window(client, buffer, other_window, SlotKindId::new(1));
    let buffer_key = ViewSlotKey::buffer(buffer, SlotKindId::new(2));

    state
        .put_view_slot(window_key, owner_id, ViewSlotFlags::new(0), b"w1")
        .expect("window slot");
    state
        .put_view_slot(other_window_key, owner_id, ViewSlotFlags::new(0), b"w2")
        .expect("other window slot");
    state
        .put_view_slot(buffer_key, owner_id, ViewSlotFlags::new(0), b"buf")
        .expect("buffer slot");

    assert_eq!(
        state
            .drop_window_view_slots(client, buffer, window)
            .expect("drop window"),
        1
    );
    assert!(state.view_slot(window_key).is_none());
    assert!(state.view_slot(other_window_key).is_some());
    assert!(state.view_slot(buffer_key).is_some());

    assert_eq!(state.drop_buffer_view_slots(buffer).expect("drop buffer"), 2);
    assert!(state.view_slot(other_window_key).is_none());
    assert!(state.view_slot(buffer_key).is_none());
});

arch_test!(buffer_record_groups_edits_undoes_redoes_and_clears_redo_on_new_edit, {
    let mut buffer = BufferRecord::new(BufferId::new(1));
    let origin = EditOrigin::User {
        client_id: ClientId::new(7),
    };

    let group = buffer.open_undo_group(origin, 1).expect("open group");
    assert_eq!(group.as_u64(), 1);
    buffer.apply_edit(origin, 0, 0, b"a", 2).expect("insert a");
    buffer.apply_edit(origin, 1, 1, b"b", 3).expect("insert b");
    buffer.apply_edit(origin, 2, 2, b"c", 4).expect("insert c");
    buffer.close_undo_group().expect("close group");
    assert_eq!(buffer.bytes().as_slice(), b"abc");
    assert_eq!(buffer.undo_stack().groups.as_slice().len(), 1);

    assert!(buffer.undo().expect("undo group"));
    assert_eq!(buffer.bytes().as_slice(), b"");
    assert_eq!(buffer.undo_stack().redo_groups.as_slice().len(), 1);

    assert!(buffer.redo().expect("redo group"));
    assert_eq!(buffer.bytes().as_slice(), b"abc");

    assert!(buffer.undo().expect("undo again"));
    buffer
        .apply_edit(origin, 0, 0, b"z", 5)
        .expect("new edit clears redo");
    assert_eq!(buffer.bytes().as_slice(), b"z");
    assert_eq!(buffer.undo_stack().redo_groups.as_slice().len(), 0);
    assert!(!buffer.redo().expect("redo empty"));
});

arch_test!(buffer_record_undo_group_cap_drops_oldest_group, {
    let mut state = StateSubstrate::with_limits(
        KernelStateLimits::new(16, 4, 100).with_undo_limits(1, 16, 1024),
    );
    let buffer = BufferId::new(1);
    let origin = EditOrigin::User {
        client_id: ClientId::new(1),
    };

    state
        .apply_buffer_edit(buffer, origin, 0, 0, b"a", 1)
        .expect("first edit");
    state
        .apply_buffer_edit(buffer, origin, 1, 1, b"b", 2)
        .expect("second edit");
    assert_eq!(
        state
            .buffer(buffer)
            .expect("buffer")
            .undo_stack()
            .groups
            .len(),
        1,
        "oldest group dropped at cap"
    );
    assert!(state.undo_buffer(buffer).expect("undo newest only"));
    assert_eq!(state.buffer(buffer).expect("buffer").bytes().as_slice(), b"a");
});

arch_test!(buffer_record_external_marker_stops_undo_before_rewriting_external_bytes, {
    let mut buffer = BufferRecord::new(BufferId::new(1));
    let origin = EditOrigin::User {
        client_id: ClientId::new(7),
    };
    buffer
        .apply_edit(origin, 0, 0, b"abc", 1)
        .expect("user edit");
    buffer
        .apply_edit(EditOrigin::External, 1, 2, b"X", 2)
        .expect("external marker");
    assert_eq!(buffer.bytes().as_slice(), b"aXc");
    assert!(
        !buffer.undo().expect("external barrier"),
        "external marker stops older undo groups"
    );
    assert_eq!(buffer.bytes().as_slice(), b"aXc");
});

arch_test!(service_registry_lease_becomes_stale_after_owner_revocation, {
    let mut registry = ServiceRegistry::new(50);
    let key = ServiceKey::new(1);
    let owner_id = owner(1);
    registry
        .register(service_meta(key, owner_id), 1)
        .expect("service row");

    let borrow = registry.borrow(key).expect("borrow");
    assert_eq!(borrow.owner_cdylib_id, owner_id);
    assert_eq!(borrow.owner_generation, 1);

    let lease = registry.lease(key, 10).expect("lease");
    registry
        .validate_lease(lease.id, 20)
        .expect("lease still valid");
    assert_eq!(registry.revoke_owner(owner_id, 2).expect("revoke owner"), 1);
    assert_eq!(registry.row(key).expect("row").state, ServiceRowState::DrainingHidden);
    assert_eq!(registry.validate_lease(lease.id, 20), Err(ServiceAccessError::Stale));
    assert!(registry.release_lease(lease.id));
    assert_eq!(registry.row(key).expect("row").state, ServiceRowState::Revoked);
});

arch_test!(service_registry_enforces_send_and_sync_call_gates, {
    let mut registry = ServiceRegistry::new(50);
    let key = ServiceKey::new(1);
    let owner_id = owner(1);
    let mut meta = service_meta(key, owner_id);
    meta.flags = ServiceFlags::new(0);
    registry
        .register_with_thread(meta, 1, 7)
        .expect("service row");

    assert_eq!(
        registry.begin_call(key, 8),
        Err(ServiceAccessError::InvalidState),
        "!SEND_SAFE row rejects non-owner thread"
    );
    let call = registry.begin_call(key, 7).expect("owner thread call");
    assert_eq!(
        registry.begin_call(key, 7),
        Err(ServiceAccessError::Busy),
        "!SYNC_SAFE row rejects concurrent call"
    );
    assert!(registry.end_call(call));
    let call = registry.begin_call(key, 7).expect("call after release");
    assert!(registry.end_call(call));
});

arch_test!(phase3_kernel_state_hostapi_integration_smoke, {
    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");
    let session_id = reovim_subsys_domain::SessionId::new(1);
    let client_one = ClientId::new(1);
    let client_two = ClientId::new(2);
    let buffer = BufferId::new(1);
    let window_one = WindowId::new(1);
    let window_two = WindowId::new(2);
    let owner_id = owner(3);

    {
        let mut session = kernel.session.state.lock();
        session
            .ensure_default_membership()
            .expect("default membership");
        assert!(session.try_attach_client(client_two).expect("client two"));
        assert!(session.try_attach_window(window_two).expect("window two"));
        assert_eq!(session.root_attachment.as_u32(), 1);
    }

    let slot_one = ViewSlotKey::window(client_one, buffer, window_one, SlotKindId::new(1));
    let slot_two = ViewSlotKey::window(client_two, buffer, window_two, SlotKindId::new(1));
    kernel
        .hostapi_view_slot_put(slot_one, owner_id, ViewSlotFlags::new(0), b"one")
        .expect("client one slot");
    kernel
        .hostapi_view_slot_put(slot_two, owner_id, ViewSlotFlags::new(0), b"two")
        .expect("client two slot");
    assert_eq!(
        kernel
            .hostapi_view_slot_get(slot_one)
            .expect("slot one")
            .expect("slot one")
            .bytes
            .as_slice(),
        b"one"
    );
    assert_eq!(
        kernel
            .hostapi_view_slot_get(slot_two)
            .expect("slot two")
            .expect("slot two")
            .bytes
            .as_slice(),
        b"two"
    );

    let register = RegisterId::new(9);
    kernel
        .hostapi_register_set(
            RegisterScope::Session,
            client_one,
            session_id,
            register,
            &carrier(1, b"session"),
        )
        .expect("session register");
    assert_eq!(
        kernel
            .hostapi_register_get(client_two, session_id, register)
            .expect("register get")
            .expect("session fallback")
            .content
            .as_slice(),
        b"session"
    );
    kernel
        .hostapi_register_set(
            RegisterScope::Client,
            client_one,
            session_id,
            register,
            &carrier(1, b"client"),
        )
        .expect("client register");
    assert_eq!(
        kernel
            .hostapi_register_get(client_one, session_id, register)
            .expect("client get")
            .expect("client value")
            .content
            .as_slice(),
        b"client"
    );
    assert_eq!(
        kernel
            .hostapi_register_get(client_two, session_id, register)
            .expect("client two get")
            .expect("session fallback retained")
            .content
            .as_slice(),
        b"session"
    );

    let origin = EditOrigin::User {
        client_id: client_one,
    };
    kernel
        .hostapi_buffer_apply_edit(buffer, origin, 0, 0, b"ab", 1)
        .expect("insert ab");
    kernel
        .hostapi_undo_group_open(buffer, origin, 2)
        .expect("open undo group");
    kernel
        .hostapi_buffer_apply_edit(buffer, origin, 2, 2, b"c", 3)
        .expect("insert c");
    kernel
        .hostapi_undo_group_close(buffer)
        .expect("close undo group");
    assert_eq!(
        kernel
            .hostapi_buffer_bytes(buffer)
            .expect("buffer bytes")
            .expect("buffer")
            .as_slice(),
        b"abc"
    );
    assert!(kernel.hostapi_undo(buffer).expect("undo"));
    assert_eq!(
        kernel
            .hostapi_buffer_bytes(buffer)
            .expect("buffer bytes after undo")
            .expect("buffer")
            .as_slice(),
        b"ab"
    );
    assert!(kernel.hostapi_redo(buffer).expect("redo"));

    let service = ServiceKey::new(10);
    kernel
        .hostapi_service_register(service_meta(service, owner_id), 1)
        .expect("service register");
    let lease = kernel
        .hostapi_service_lease(service, 10)
        .expect("service lease");
    kernel
        .hostapi_service_revoke_owner(owner_id, 2)
        .expect("service revoke");
    assert_eq!(
        kernel.hostapi_service_lease_validate(lease.id, 20),
        Err(ServiceAccessError::Stale)
    );
});

arch_test!(phase4_kernel_carrier_and_stream_hostapi_integration_smoke, {
    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");
    let owner_id = owner(8);
    let session = SessionId::new(8);
    let buffer = BufferId::new(80);

    {
        let mut router = kernel.domain_router.write();
        router
            .register_static_owner(owner_id, 8)
            .expect("owner init phase");
    }

    CARRIER_EVENT_COUNT.store(0, Ordering::SeqCst);
    kernel
        .event_bus
        .subscribe(count_carrier_event)
        .expect("subscribe carrier counter");

    kernel
        .hostapi_position_codec_register(domain(8), 4, owner_id, 1, 8)
        .expect("codec register");
    assert_eq!(
        kernel
            .hostapi_validate_position_carrier(session, 1, 0, &position_carrier(8, 4, b"known"),)
            .expect("known carrier"),
        CarrierStatus::ValidKnown
    );
    assert!(
        kernel
            .hostapi_validate_cursor_carrier(session, 99, 1, &cursor_carrier(0, 1, b"bad"))
            .is_err(),
        "invalid carrier is rejected at kernel ingress"
    );
    assert_eq!(CARRIER_EVENT_COUNT.load(Ordering::SeqCst), 1);

    kernel
        .hostapi_stream_scheme_register(stream_scheme(b"pipe", owner_id))
        .expect("stream scheme");
    let stream = kernel
        .hostapi_stream_open(b"pipe", Some(session), Some(buffer))
        .expect("open stream");
    assert!(
        kernel
            .hostapi_stream_subscribe(stream, session, buffer, 16)
            .expect("subscribe buffer")
    );
    assert_eq!(
        kernel
            .hostapi_stream_emit(stream, b"from-stream", 10)
            .expect("emit stream bytes")
            .bytes_buffered,
        11
    );
    assert!(
        kernel
            .hostapi_stream_drain_to_buffer(stream, buffer, 20)
            .expect("drain to byte edit"),
        "stream bytes apply through buffer state"
    );
    assert_eq!(
        kernel
            .hostapi_buffer_bytes(buffer)
            .expect("buffer bytes")
            .expect("buffer")
            .as_slice(),
        b"from-stream"
    );
    assert_eq!(
        kernel
            .hostapi_stream_control_class(stream, StreamControlOp::KERNEL_INSPECT)
            .expect("inspect control"),
        StreamControlClass::Kernel
    );

    {
        let mut router = kernel.domain_router.write();
        router.activate_owner(owner_id).expect("activate owner");
    }
    assert!(
        kernel
            .hostapi_stream_scheme_register(stream_scheme(b"late", owner_id))
            .is_err(),
        "stream scheme registration closes after init"
    );
});
