//! Tests for `session.rs` — `Session`, `SessionState`, and the CC14 dispatch
//! shape (walking-skeleton, #797).
//!
//! Registered under the `selftest` feature; runs on the arch no_std selftest
//! runner via the `kernel-selftest` fixture bin.
//!
//! ## Integration smoke (Phase 2 AC)
//!
//! The `session_dispatch_integration_smoke` test boots a kernel, registers
//! the text Domain, attaches a buffer, drives a `RawInput` batch through the
//! router's `OnRawInput` → `Render`, and asserts the resulting `Projection`
//! bytes match the expected fixture — proving session + router + Domain dispatch
//! compose end-to-end inside the kernel before any wire is involved.
//!
//! The text Domain types live in `reovim-domain-text` (an `ext/server/domain/*`
//! crate, Category::ServerExt). They are wired in here under `selftest` only;
//! the kernel crate does NOT have a runtime dep on the text Domain crate (the
//! core/ext boundary is maintained). The kernel-selftest fixture bin enables
//! `reovim-domain-text/selftest` and links it.

use core::{
    num::NonZeroU32,
    sync::atomic::{AtomicU32, Ordering},
};

use reovim_arch::{arch_test, ds::Bytes, sync::RwLock};

use {
    crate::{
        Init, LauncherArgs,
        projection::{Projection, ProjectionSpan},
        router::{
            CdylibId, DomainApiVersion, DomainId, DomainManifest, DomainRouter, HandlerId,
            OnRawInputHandler, ProjectorId, RawInputResult, RenderProjector,
        },
        session::{
            BufferId, DispatchRoutes, DomainAttachmentId, DomainScope, FocusEntry,
            MAX_FOCUS_CHAIN_DEPTH, PositionCarrier, PositionHeader, Session, SessionId,
            SessionState, SessionTable, WindowId,
        },
        state::StateSubstrate,
    },
    reovim_subsys_domain::{
        PendingAttachmentId,
        id::{ClientId, ServiceKey, SlotKindId},
        service::{ServiceDescriptorMeta, ServiceFlags},
        state::{EditOrigin, ViewSlotFlags, ViewSlotKey},
    },
};

struct RootHandler;

impl OnRawInputHandler for RootHandler {
    fn on_raw_input(&self, mut buffer: Bytes, cursor: usize, _input: &[u8]) -> RawInputResult {
        buffer.try_push(b'r').expect("append root");
        RawInputResult::claimed(buffer, cursor + 1)
    }
}

struct ChildHandler;

impl OnRawInputHandler for ChildHandler {
    fn on_raw_input(&self, mut buffer: Bytes, cursor: usize, _input: &[u8]) -> RawInputResult {
        buffer.try_push(b'c').expect("append child");
        RawInputResult::claimed(buffer, cursor + 1)
    }
}

struct IgnoringChildHandler;

impl OnRawInputHandler for IgnoringChildHandler {
    fn on_raw_input(&self, buffer: Bytes, cursor: usize, _input: &[u8]) -> RawInputResult {
        RawInputResult::ignored(buffer, cursor)
    }
}

struct TextFixtureHandler;

impl OnRawInputHandler for TextFixtureHandler {
    fn on_raw_input(&self, mut buffer: Bytes, cursor: usize, _input: &[u8]) -> RawInputResult {
        buffer.try_push(b't').expect("append text");
        RawInputResult::claimed(buffer, cursor + 1)
    }
}

struct HexFixtureHandler;

impl OnRawInputHandler for HexFixtureHandler {
    fn on_raw_input(&self, mut buffer: Bytes, cursor: usize, _input: &[u8]) -> RawInputResult {
        buffer.try_push(b'h').expect("append hex");
        RawInputResult::claimed(buffer, cursor + 1)
    }
}

struct TestProjector;

impl RenderProjector for TestProjector {
    fn render(
        &self,
        buffer: Bytes,
        cursor: usize,
        buffer_id: BufferId,
        window_id: WindowId,
    ) -> Result<Projection, &'static str> {
        Ok(Projection {
            buffer_id,
            window_id,
            span: ProjectionSpan {
                start: 0,
                end: buffer.len(),
            },
            cursor_byte: cursor,
            content: buffer,
        })
    }
}

static ROOT_HANDLER: RootHandler = RootHandler;
static CHILD_HANDLER: ChildHandler = ChildHandler;
static IGNORING_CHILD_HANDLER: IgnoringChildHandler = IgnoringChildHandler;
static TEXT_FIXTURE_HANDLER: TextFixtureHandler = TextFixtureHandler;
static HEX_FIXTURE_HANDLER: HexFixtureHandler = HexFixtureHandler;
static PROJECTOR: TestProjector = TestProjector;

struct HostApiProbeHandler<'a> {
    session: &'a Session,
    state: &'a RwLock<StateSubstrate>,
    flags: &'a AtomicU32,
    buffer_id: BufferId,
    client_id: ClientId,
    window_id: WindowId,
    service_key: ServiceKey,
}

impl OnRawInputHandler for HostApiProbeHandler<'_> {
    fn on_raw_input(&self, mut buffer: Bytes, cursor: usize, _input: &[u8]) -> RawInputResult {
        if let Some(guard) = self.session.state.try_lock() {
            drop(guard);
            self.flags.fetch_or(0x01, Ordering::SeqCst);
        }

        let mut state = self.state.write();
        state
            .open_buffer_undo_group(
                self.buffer_id,
                EditOrigin::User {
                    client_id: self.client_id,
                },
                1,
            )
            .expect("open undo group from handler");
        state
            .apply_buffer_edit(
                self.buffer_id,
                EditOrigin::User {
                    client_id: self.client_id,
                },
                0,
                0,
                b"u",
                2,
            )
            .expect("handler HostApi buffer edit");
        state
            .close_buffer_undo_group(self.buffer_id)
            .expect("close undo group from handler");
        state
            .put_view_slot(
                ViewSlotKey::window(
                    self.client_id,
                    self.buffer_id,
                    self.window_id,
                    SlotKindId::new(7),
                ),
                owner(9),
                ViewSlotFlags::new(ViewSlotFlags::HOSTAPI_REENTRANT),
                b"slot",
            )
            .expect("handler HostApi view slot");
        let call = state
            .service_registry
            .begin_call(self.service_key, reovim_arch::sys::gettid())
            .expect("handler HostApi service begin");
        assert!(state.service_registry.end_call(call), "handler HostApi service end");
        self.flags.fetch_or(0x04, Ordering::SeqCst);

        buffer.try_push(b'p').expect("handler projection byte");
        RawInputResult::claimed(buffer, cursor + 1)
    }
}

struct HostApiProbeProjector<'a> {
    session: &'a Session,
    state: &'a RwLock<StateSubstrate>,
    flags: &'a AtomicU32,
    buffer_id: BufferId,
    client_id: ClientId,
    window_id: WindowId,
}

impl RenderProjector for HostApiProbeProjector<'_> {
    fn render(
        &self,
        buffer: Bytes,
        cursor: usize,
        buffer_id: BufferId,
        window_id: WindowId,
    ) -> Result<Projection, &'static str> {
        if let Some(guard) = self.session.state.try_lock() {
            drop(guard);
            self.flags.fetch_or(0x02, Ordering::SeqCst);
        }

        let state = self.state.read();
        let slot_key =
            ViewSlotKey::window(self.client_id, self.buffer_id, self.window_id, SlotKindId::new(7));
        if state
            .buffer(self.buffer_id)
            .is_some_and(|record| record.bytes().as_slice() == b"u")
            && state
                .view_slot(slot_key)
                .is_some_and(|slot| slot.bytes.as_slice() == b"slot")
        {
            self.flags.fetch_or(0x08, Ordering::SeqCst);
        }

        Ok(Projection {
            buffer_id,
            window_id,
            span: ProjectionSpan {
                start: 0,
                end: buffer.len(),
            },
            cursor_byte: cursor,
            content: buffer,
        })
    }
}

fn domain(raw: u32) -> DomainId {
    DomainId::new(NonZeroU32::new(raw).unwrap())
}

fn owner(raw: u32) -> CdylibId {
    CdylibId::new(NonZeroU32::new(raw).unwrap())
}

fn scope() -> DomainScope {
    DomainScope::new(
        PositionCarrier::new(PositionHeader::from_raw_parts(0, 0, 0), Bytes::new()),
        PositionCarrier::new(PositionHeader::from_raw_parts(0, 0, 0), Bytes::new()),
        0,
    )
}

fn payload_scope() -> DomainScope {
    DomainScope::new(
        PositionCarrier::new(
            PositionHeader::from_raw_parts(1, 1, 0),
            Bytes::try_from_slice(b"start").expect("start carrier"),
        ),
        PositionCarrier::new(
            PositionHeader::from_raw_parts(1, 1, 0),
            Bytes::try_from_slice(b"end").expect("end carrier"),
        ),
        0,
    )
}

fn register_manifest(
    router: &mut DomainRouter,
    id: DomainId,
    name: &[u8],
    handlers: &[HandlerId],
    projectors: &[ProjectorId],
) {
    let mut manifest = DomainManifest::new(
        id,
        Bytes::try_from_slice(name).expect("manifest name"),
        owner(1),
        DomainApiVersion::new(0, 16),
    );
    for handler in handlers {
        manifest
            .handler_kinds
            .try_push(*handler)
            .expect("handler kind");
    }
    for projector in projectors {
        manifest
            .projector_kinds
            .try_push(*projector)
            .expect("projector kind");
    }
    router.register_manifest(manifest).expect("manifest");
}

fn service_meta(key: ServiceKey, owner_cdylib_id: CdylibId) -> ServiceDescriptorMeta {
    ServiceDescriptorMeta::new(
        key,
        owner_cdylib_id,
        DomainApiVersion::new(0, 16),
        DomainApiVersion::new(0, 16),
        Bytes::try_from_slice(b"session.fixture.service").expect("type name"),
        16,
        ServiceFlags::new(ServiceFlags::SEND_SAFE | ServiceFlags::SYNC_SAFE),
    )
}

arch_test!(session_id_round_trip, {
    let id = SessionId::new(7);
    assert_eq!(id.as_u32(), 7);
});

arch_test!(domain_attachment_id_round_trip, {
    let id = DomainAttachmentId::new(10);
    assert_eq!(id.as_u32(), 10);
});

arch_test!(focus_entry_resolved_variant, {
    let entry = FocusEntry::Resolved(DomainAttachmentId::new(1));
    match entry {
        FocusEntry::Resolved(id) => assert_eq!(id.as_u32(), 1),
        FocusEntry::Pending(_) => panic!("expected resolved"),
    }
});

arch_test!(session_state_new_empty_buffer, {
    let domain_id = domain(1);
    let state = SessionState::new(
        domain_id,
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    assert!(state.buffer.is_empty(), "new session state has empty buffer");
    assert_eq!(state.cursor, 0, "cursor starts at 0");
    assert_eq!(state.buffer_id.as_u32(), 1);
    assert_eq!(state.window_id.as_u32(), 1);
    assert_eq!(state.root_attachment_record.domain_id, domain_id);
    assert_eq!(state.root_attachment_record.struct_refs, 1);
    assert_eq!(state.root_attachment_record.focus_refs, 1);
});

arch_test!(session_table_creates_multiple_sessions_and_tracks_membership, {
    let mut table = SessionTable::new();
    let first = table.create_session(domain(1)).expect("first session");
    let second = table.create_session(domain(1)).expect("second session");
    assert_eq!(table.len(), 2);
    assert_ne!(first.id, second.id);

    {
        let mut guard = first.state.lock();
        let primary_client = guard.client_id;
        let primary_buffer = guard.buffer_id;
        let primary_window = guard.window_id;
        assert!(guard.has_client(primary_client));
        assert!(guard.has_buffer(primary_buffer));
        assert!(guard.has_window(primary_window));
        assert!(
            guard
                .try_attach_client(ClientId::new(99))
                .expect("client add")
        );
        assert!(
            !guard
                .try_attach_client(ClientId::new(99))
                .expect("client duplicate")
        );
        assert!(
            guard
                .try_attach_buffer(BufferId::new(99))
                .expect("buffer add")
        );
        assert!(
            guard
                .try_attach_window(WindowId::new(99))
                .expect("window add")
        );
    }

    assert!(table.get(first.id).is_some());
    assert!(table.remove(first.id).is_some());
    assert_eq!(table.len(), 1);
});

arch_test!(session_state_focus_chain_is_single_resolved, {
    let domain_id = domain(1);
    let attach_id = DomainAttachmentId::new(1);
    let state = SessionState::new(domain_id, attach_id, BufferId::new(1), WindowId::new(1));
    // Single-entry focus chain [Resolved(root)] (§4.2 subset note).
    assert_eq!(state.focus_entries().len(), 1);
    match state.focus_entries()[0] {
        FocusEntry::Resolved(id) => assert_eq!(id.as_u32(), 1),
        FocusEntry::Pending(_) => panic!("expected resolved"),
    }
});

arch_test!(session_state_focus_push_and_pop_preserves_root, {
    let domain_id = domain(1);
    let mut state = SessionState::new(
        domain_id,
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    let pending_id = state
        .try_create_pending_child(DomainAttachmentId::new(1), domain(2), scope(), Bytes::new())
        .expect("pending child");
    state
        .try_focus_push(FocusEntry::Pending(pending_id))
        .expect("push pending leaf");
    assert_eq!(state.focus_entries().len(), 2);
    match state.focus_pop().expect("pop pending") {
        Some(FocusEntry::Pending(id)) => assert_eq!(id, pending_id),
        _ => panic!("expected pending leaf"),
    }
    assert_eq!(state.focus_entries().len(), 1);
    assert!(state.focus_pop().expect("root pop").is_none(), "root focus entry is preserved");
    assert_eq!(state.focus_transitions.len(), 2);
});

arch_test!(session_state_focus_depth_is_bounded, {
    let domain_id = domain(1);
    let mut state = SessionState::new(
        domain_id,
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    let mut i = 1usize;
    while i < MAX_FOCUS_CHAIN_DEPTH {
        let child = state
            .try_attach_child(
                state.focus_entries()[i - 1]
                    .resolved_id()
                    .expect("resolved"),
                domain(u32::try_from(i).unwrap() + 1),
                scope(),
            )
            .expect("child attachment");
        state
            .try_focus_push(FocusEntry::Resolved(child))
            .expect("push until full");
        i += 1;
    }
    assert!(
        state
            .try_focus_push(FocusEntry::Pending(PendingAttachmentId::new(99)))
            .is_err()
    );
});

arch_test!(session_state_attaches_roots_and_children_in_append_order, {
    let mut state = SessionState::new(
        domain(1),
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );

    let second_root = state
        .try_attach_root(domain(2), scope())
        .expect("second root");
    let first_child = state
        .try_attach_child(DomainAttachmentId::new(1), domain(3), scope())
        .expect("first child");
    let second_child = state
        .try_attach_child(DomainAttachmentId::new(1), domain(4), scope())
        .expect("second child");

    assert_eq!(state.attachment(second_root).expect("root").child_ordinal, 1);
    assert_eq!(
        state
            .attachment(first_child)
            .expect("first child")
            .child_ordinal,
        0
    );
    assert_eq!(
        state
            .attachment(second_child)
            .expect("second child")
            .child_ordinal,
        1
    );
    assert_eq!(state.root_attachment_record.children.as_slice(), &[first_child, second_child]);
});

arch_test!(focus_push_pop_updates_refs_and_transition_sequence, {
    let mut state = SessionState::new(
        domain(1),
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    let child = state
        .try_attach_child(DomainAttachmentId::new(1), domain(2), scope())
        .expect("child");
    assert_eq!(state.attachment(child).expect("child").focus_refs, 0);

    state
        .try_focus_push(FocusEntry::Resolved(child))
        .expect("focus child");
    assert_eq!(state.attachment(child).expect("child").focus_refs, 1);
    assert_eq!(state.last_focus_transition().expect("transition").seq, 1);

    let popped = state.focus_pop().expect("pop").expect("popped");
    assert_eq!(popped, FocusEntry::Resolved(child));
    assert_eq!(state.attachment(child).expect("child").focus_refs, 0);
    assert_eq!(state.last_focus_transition().expect("transition").seq, 2);
});

arch_test!(pending_leaf_queues_dispatch_and_blocks_ancestor_fallthrough, {
    let mut router = DomainRouter::new();
    router
        .register_static_owner(owner(1), 0)
        .expect("owner record");
    register_manifest(
        &mut router,
        domain(1),
        b"root",
        &[HandlerId::OnRawInput],
        &[ProjectorId::Render],
    );
    router
        .register_handler(domain(1), &ROOT_HANDLER)
        .expect("root handler");
    router
        .register_projector(domain(1), &PROJECTOR)
        .expect("root projector");

    let mut state = SessionState::new(
        domain(1),
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    let pending_id = state
        .try_focus_pending_child(DomainAttachmentId::new(1), domain(2), scope(), Bytes::new())
        .expect("pending focus");
    let replay_queue_id = state
        .pending_attachments
        .get(&pending_id)
        .expect("pending")
        .replay_queue_id;
    let session = Session::new(SessionId::new(1), state);

    assert!(session.dispatch_input(b"x", &router).is_err());
    let guard = session.state.lock();
    assert_eq!(guard.buffer.as_slice(), b"", "root handler did not run");
    assert_eq!(guard.replay_queue_len(replay_queue_id), Some(1));
});

arch_test!(pending_leaf_resolves_transactionally_into_focused_attachment, {
    let mut state = SessionState::new(
        domain(1),
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    let pending_id = state
        .try_focus_pending_child(DomainAttachmentId::new(1), domain(2), scope(), Bytes::new())
        .expect("pending focus");
    let (attachment_id, replay_queue_id) = state
        .resolve_pending_leaf(pending_id)
        .expect("resolve pending");

    assert_eq!(
        state.focus_entries()[state.focus_entries().len() - 1],
        FocusEntry::Resolved(attachment_id)
    );
    assert_eq!(state.attachment(attachment_id).expect("child").focus_refs, 1);
    assert_eq!(state.replay_queue_len(replay_queue_id), Some(0));
    assert_eq!(
        state
            .last_focus_transition()
            .expect("transition")
            .after
            .as_slice()[1],
        reovim_subsys_domain::FocusEntrySnapshot::Resolved(attachment_id, domain(2))
    );
});

arch_test!(pending_resolution_failure_preserves_pending_leaf_and_replay_queue, {
    let mut saw_failure = false;
    let mut saw_success = false;
    let mut k = 0isize;

    while k < 64 {
        let mut state = SessionState::new(
            domain(1),
            DomainAttachmentId::new(1),
            BufferId::new(1),
            WindowId::new(1),
        );
        let pending_id = state
            .try_focus_pending_child(
                DomainAttachmentId::new(1),
                domain(2),
                payload_scope(),
                Bytes::try_from_slice(b"boot").expect("bootstrap"),
            )
            .expect("pending focus");
        let replay_queue_id = state
            .pending_attachments
            .get(&pending_id)
            .expect("pending")
            .replay_queue_id;

        reovim_arch::alloc::fault::fail_after(k);
        let result = state.resolve_pending_leaf(pending_id);
        reovim_arch::alloc::fault::reset();

        if result.is_ok() {
            saw_success = true;
            break;
        }

        saw_failure = true;
        assert_eq!(
            state.focus_entries()[state.focus_entries().len() - 1],
            FocusEntry::Pending(pending_id)
        );
        assert!(
            state.pending_attachments.get(&pending_id).is_some(),
            "pending record remains after failed resolution"
        );
        assert_eq!(state.replay_queue_len(replay_queue_id), Some(0));
        assert!(
            state.root_attachment_record.children.is_empty(),
            "failed resolution leaves no partial child attachment"
        );
        assert_eq!(
            state.focus_transitions.len(),
            1,
            "failed resolution publishes no extra transition"
        );
        k += 1;
    }

    assert!(saw_failure, "fault sweep must hit at least one rollback path");
    assert!(saw_success, "fault sweep must eventually allow resolution");
});

arch_test!(leaf_claimed_dispatch_prevents_ancestor_handler, {
    let mut router = DomainRouter::new();
    router
        .register_static_owner(owner(1), 0)
        .expect("owner record");
    register_manifest(
        &mut router,
        domain(1),
        b"root",
        &[HandlerId::OnRawInput],
        &[ProjectorId::Render],
    );
    register_manifest(
        &mut router,
        domain(2),
        b"child",
        &[HandlerId::OnRawInput],
        &[ProjectorId::Render],
    );
    router
        .register_handler(domain(1), &ROOT_HANDLER)
        .expect("root handler");
    router
        .register_projector(domain(1), &PROJECTOR)
        .expect("root projector");
    router
        .register_handler(domain(2), &CHILD_HANDLER)
        .expect("child handler");
    router
        .register_projector(domain(2), &PROJECTOR)
        .expect("child projector");

    let mut state = SessionState::new(
        domain(1),
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    let child = state
        .try_attach_child(DomainAttachmentId::new(1), domain(2), scope())
        .expect("child attach");
    state
        .try_focus_push(FocusEntry::Resolved(child))
        .expect("focus child");
    let session = Session::new(SessionId::new(1), state);

    let projection = session.dispatch_input(b"x", &router).expect("dispatch");
    assert_eq!(projection.content.as_slice(), b"c");
});

arch_test!(leaf_ignored_dispatch_falls_through_to_ancestor_handler, {
    let mut router = DomainRouter::new();
    router
        .register_static_owner(owner(1), 0)
        .expect("owner record");
    register_manifest(
        &mut router,
        domain(1),
        b"root",
        &[HandlerId::OnRawInput],
        &[ProjectorId::Render],
    );
    register_manifest(
        &mut router,
        domain(2),
        b"child",
        &[HandlerId::OnRawInput],
        &[ProjectorId::Render],
    );
    router
        .register_handler(domain(1), &ROOT_HANDLER)
        .expect("root handler");
    router
        .register_projector(domain(1), &PROJECTOR)
        .expect("root projector");
    router
        .register_handler(domain(2), &IGNORING_CHILD_HANDLER)
        .expect("child handler");
    router
        .register_projector(domain(2), &PROJECTOR)
        .expect("child projector");

    let mut state = SessionState::new(
        domain(1),
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    let child = state
        .try_attach_child(DomainAttachmentId::new(1), domain(2), scope())
        .expect("child attach");
    state
        .try_focus_push(FocusEntry::Resolved(child))
        .expect("focus child");
    let session = Session::new(SessionId::new(1), state);

    let projection = session.dispatch_input(b"x", &router).expect("dispatch");
    assert_eq!(projection.content.as_slice(), b"r");
});

arch_test!(phase2_static_text_hex_domain_tree_integration_smoke, {
    let mut router = DomainRouter::new();
    router
        .register_static_owner(owner(1), 0)
        .expect("owner record");
    let text_domain = router.intern_named("text").expect("intern text");
    let hex_domain = router.intern_named("hex").expect("intern hex");
    assert_eq!(router.name_of(text_domain).expect("text name"), b"text");
    assert_eq!(router.name_of(hex_domain).expect("hex name"), b"hex");

    register_manifest(
        &mut router,
        text_domain,
        b"text",
        &[HandlerId::OnRawInput],
        &[ProjectorId::Render],
    );
    register_manifest(
        &mut router,
        hex_domain,
        b"hex",
        &[HandlerId::OnRawInput],
        &[ProjectorId::Render],
    );
    router
        .register_handler(text_domain, &TEXT_FIXTURE_HANDLER)
        .expect("text handler");
    router
        .register_projector(text_domain, &PROJECTOR)
        .expect("text projector");
    router
        .register_handler(hex_domain, &HEX_FIXTURE_HANDLER)
        .expect("hex handler");
    router
        .register_projector(hex_domain, &PROJECTOR)
        .expect("hex projector");

    let state = SessionState::new(
        text_domain,
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    let session = Session::new(SessionId::new(7), state);

    let (second_root, pending_id, replay_queue_id) = {
        let mut guard = session.state.lock();
        let second_root = guard
            .try_attach_root(hex_domain, scope())
            .expect("second root");
        assert_eq!(guard.root_attachments.as_slice(), &[second_root]);
        assert_eq!(
            guard
                .attachment(second_root)
                .expect("second root")
                .child_ordinal,
            1
        );

        let pending_id = guard
            .try_focus_pending_child(DomainAttachmentId::new(1), hex_domain, scope(), Bytes::new())
            .expect("pending hex child");
        let replay_queue_id = guard
            .pending_attachments
            .get(&pending_id)
            .expect("pending record")
            .replay_queue_id;
        let transition = guard.last_focus_transition().expect("pending transition");
        assert_eq!(transition.seq, 1);
        assert_eq!(transition.session_id, SessionId::new(7));
        assert_eq!(
            transition.after.as_slice()[1],
            reovim_subsys_domain::FocusEntrySnapshot::Pending(pending_id, hex_domain)
        );
        (second_root, pending_id, replay_queue_id)
    };

    assert!(session.dispatch_input(b"x", &router).is_err());
    {
        let guard = session.state.lock();
        assert_eq!(guard.buffer.as_slice(), b"", "text root did not run");
        assert_eq!(guard.replay_queue_len(replay_queue_id), Some(1));
        assert_eq!(guard.focus_transitions.len(), 1);
    }

    let resolved_id = {
        let mut guard = session.state.lock();
        let (resolved_id, resolved_replay_queue_id) = guard
            .resolve_pending_leaf(pending_id)
            .expect("resolve pending hex");
        assert_eq!(resolved_replay_queue_id, replay_queue_id);
        assert!(guard.pending_attachments.get(&pending_id).is_none());
        assert_eq!(guard.replay_queue_len(replay_queue_id), Some(1));
        assert_eq!(
            guard.focus_entries()[guard.focus_entries().len() - 1],
            FocusEntry::Resolved(resolved_id)
        );
        assert_eq!(guard.root_attachment_record.children.as_slice(), &[resolved_id]);
        assert_eq!(
            guard
                .attachment(resolved_id)
                .expect("resolved child")
                .parent,
            Some(DomainAttachmentId::new(1))
        );
        assert_eq!(
            guard
                .attachment(resolved_id)
                .expect("resolved child")
                .domain_id,
            hex_domain
        );

        let transitions = guard.focus_transitions.as_slice();
        assert_eq!(transitions.len(), 2);
        assert_eq!(transitions[0].seq, 1);
        assert_eq!(transitions[1].seq, 2);
        assert_eq!(
            transitions[1].before.as_slice()[1],
            reovim_subsys_domain::FocusEntrySnapshot::Pending(pending_id, hex_domain)
        );
        assert_eq!(
            transitions[1].after.as_slice()[1],
            reovim_subsys_domain::FocusEntrySnapshot::Resolved(resolved_id, hex_domain)
        );
        resolved_id
    };

    let projection = session.dispatch_input(b"x", &router).expect("dispatch");
    assert_eq!(projection.content.as_slice(), b"h");
    {
        let guard = session.state.lock();
        assert_eq!(guard.buffer.as_slice(), b"h");
        assert_eq!(guard.cursor, 1);
        assert_eq!(guard.replay_queue_len(replay_queue_id), Some(1));
        assert_eq!(
            guard
                .attachment(second_root)
                .expect("second root")
                .domain_id,
            hex_domain
        );
        assert_eq!(
            guard
                .attachment(resolved_id)
                .expect("resolved child")
                .focus_refs,
            1
        );
    }
});

arch_test!(cc14_cc16_dispatch_probes_session_lock_and_state_hostapi_reverse_flow, {
    let buffer_id = BufferId::new(1);
    let window_id = WindowId::new(1);
    let client_id = ClientId::new(1);
    let session = Session::new(
        SessionId::new(3),
        SessionState::new_with_client(
            domain(1),
            DomainAttachmentId::new(1),
            buffer_id,
            window_id,
            client_id,
        ),
    );
    let state = RwLock::new(StateSubstrate::new());
    let flags = AtomicU32::new(0);
    {
        let mut state_guard = state.write();
        state_guard
            .service_registry
            .register(service_meta(ServiceKey::new(1), owner(9)), 1)
            .expect("service row for handler probe");
    }
    let handler = HostApiProbeHandler {
        session: &session,
        state: &state,
        flags: &flags,
        buffer_id,
        client_id,
        window_id,
        service_key: ServiceKey::new(1),
    };
    let projector = HostApiProbeProjector {
        session: &session,
        state: &state,
        flags: &flags,
        buffer_id,
        client_id,
        window_id,
    };

    let snapshot = session.prepare_dispatch(b"x").expect("prepare dispatch");
    let mut routes = DispatchRoutes::new();
    routes
        .try_push_handler(domain(1), &handler)
        .expect("handler route");
    routes
        .try_push_projector(domain(1), &projector)
        .expect("projector route");
    let projection = session
        .dispatch_prepared(&snapshot, b"x", &routes)
        .expect("prepared dispatch");

    assert_eq!(projection.content.as_slice(), b"p");
    assert_eq!(
        flags.load(Ordering::SeqCst),
        0x0f,
        "handler/projector saw Session.state unlocked and HostApi state calls succeeded"
    );
    let state_guard = state.read();
    assert_eq!(
        state_guard
            .buffer(buffer_id)
            .expect("handler buffer")
            .bytes()
            .as_slice(),
        b"u"
    );
});

// ── Integration smoke (Phase 2 AC) ───────────────────────────────────────────
//
// The kernel-level smoke exercises session + router mechanics without any ext
// dependency. The full end-to-end integration smoke (kernel + text Domain
// handler/projector + dispatch) lives in the kernel-selftest bin's own module
// (`tests/fixtures/kernel-selftest/src/domain_smoke.rs`), where the bin can
// link `reovim-domain-text` without adding an ext dep to the kernel rlib.
//
// This test verifies the dispatch path returns the expected error when no
// handler is registered (the sentinel session state installed by Init::boot).

arch_test!(session_dispatch_without_domain_returns_err, {
    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");
    // No handler registered: dispatch must return an Err.
    let result = kernel.dispatch_input(b"x");
    assert!(result.is_err(), "dispatch without registered handler must return Err");
});

// ── as_u32 body-coverage (session.rs lines 73-74, 113-114) ───────────────────
//
// `SessionId::as_u32` and `DomainAttachmentId::as_u32` are `const fn`. LLVM
// may inline the body at const-eval time and not emit a callable function body,
// leaving the coverage tool with no arc to tick. The tests below call them via
// a non-const path (runtime assignment) so the body receives a runtime-visible
// call site that the profiler can attribute.

arch_test!(session_id_as_u32_runtime_call, {
    // Prevent const-propagation: build the id from a runtime value.
    let v: u32 = reovim_arch::sys::gettid().unsigned_abs() % 65536;
    let id = SessionId::new(v);
    assert_eq!(id.as_u32(), v, "SessionId::as_u32 must return the stored value");
});

arch_test!(domain_attachment_id_as_u32_runtime_call, {
    let v: u32 = (reovim_arch::sys::gettid().unsigned_abs() % 65536) + 1;
    let id = DomainAttachmentId::new(v);
    assert_eq!(id.as_u32(), v, "DomainAttachmentId::as_u32 must return the stored value");
});
