//! Tests for `router.rs` — `DomainRouter` SUBSET coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std selftest
//! runner via the `kernel-selftest` fixture bin.

use core::num::NonZeroU32;

use reovim_arch::{arch_test, ds::Bytes};

use crate::{
    projection::{Projection, ProjectionSpan},
    router::{
        CdylibId, DomainApiVersion, DomainId, DomainManifest, DomainRouter, HandlerEntry,
        HandlerId, OnRawInputHandler, PriorityBand, ProjectorEntry, ProjectorId, RawInputResult,
        RegistrationPhase, RenderProjector,
    },
    session::{BufferId, WindowId},
};

struct TestHandler;

impl OnRawInputHandler for TestHandler {
    fn on_raw_input(&self, buffer: Bytes, cursor: usize, _input: &[u8]) -> RawInputResult {
        RawInputResult::ignored(buffer, cursor)
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

static HANDLER: TestHandler = TestHandler;
static PROJECTOR: TestProjector = TestProjector;

fn owner(raw: u32) -> CdylibId {
    CdylibId::new(NonZeroU32::new(raw).unwrap())
}

fn domain(raw: u32) -> DomainId {
    DomainId::new(NonZeroU32::new(raw).unwrap())
}

fn register_owner(r: &mut DomainRouter, id: CdylibId, order: u32) {
    r.register_static_owner(id, order).expect("owner record");
}

fn register_manifest(
    r: &mut DomainRouter,
    id: DomainId,
    manifest_owner: CdylibId,
    handlers: &[HandlerId],
    projectors: &[ProjectorId],
) {
    let mut manifest = DomainManifest::new(
        id,
        Bytes::try_from_slice(b"text").expect("alloc"),
        manifest_owner,
        DomainApiVersion::new(0, 16),
    );
    for handler_id in handlers {
        manifest
            .handler_kinds
            .try_push(*handler_id)
            .expect("handler declaration");
    }
    for projector_id in projectors {
        manifest
            .projector_kinds
            .try_push(*projector_id)
            .expect("projector declaration");
    }
    r.register_manifest(manifest).expect("manifest row");
}

arch_test!(router_new_is_empty, {
    let r = DomainRouter::new();
    assert!(r.name_of_id(1).is_none(), "new router has no names");
});

arch_test!(intern_named_first_call_returns_id, {
    let mut r = DomainRouter::new();
    let id = r.intern_named("text").expect("intern_named succeeds");
    assert_eq!(id.as_u32(), 1, "first interned id is 1");
});

arch_test!(intern_named_idempotent_same_name, {
    // §4.1 §3 intern stability: same name → same DomainId.
    let mut r = DomainRouter::new();
    let id1 = r.intern_named("text").expect("first intern");
    let id2 = r.intern_named("text").expect("second intern");
    assert_eq!(id1, id2, "intern_named is idempotent for the same name");
});

arch_test!(intern_named_different_names_get_different_ids, {
    let mut r = DomainRouter::new();
    let id_text = r.intern_named("text").expect("intern text");
    let id_elf = r.intern_named("elf").expect("intern elf");
    assert_ne!(id_text, id_elf, "different names → different ids");
});

arch_test!(name_of_returns_bytes_for_interned_id, {
    let mut r = DomainRouter::new();
    let id = r.intern_named("text").expect("intern");
    let name = r.name_of(id).expect("name_of returns Some for interned id");
    assert_eq!(name, b"text");
});

arch_test!(name_of_unknown_id_returns_none, {
    use core::num::NonZeroU32;
    let r = DomainRouter::new();
    let fake_id = DomainId::new(NonZeroU32::new(99).unwrap());
    assert!(r.name_of(fake_id).is_none(), "unknown id → None");
});

arch_test!(handler_returns_none_when_not_registered, {
    use core::num::NonZeroU32;
    let r = DomainRouter::new();
    let id = DomainId::new(NonZeroU32::new(1).unwrap());
    assert!(r.handler(id).is_none(), "no handler before registration");
});

arch_test!(projector_returns_none_when_not_registered, {
    let r = DomainRouter::new();
    let id = domain(1);
    assert!(r.projector(id).is_none(), "no projector before registration");
});

arch_test!(default_registration_creates_ordered_on_raw_input_and_render_rows, {
    let mut r = DomainRouter::new();
    let id = domain(1);
    register_owner(&mut r, owner(1), 0);
    register_manifest(&mut r, id, owner(1), &[HandlerId::OnRawInput], &[ProjectorId::Render]);
    r.register_handler(id, &HANDLER).expect("handler row");
    r.register_projector(id, &PROJECTOR).expect("projector row");

    assert!(r.handler(id).is_some(), "handler lookup returns default row");
    assert!(r.projector(id).is_some(), "projector lookup returns default row");

    let h_rows = r
        .handler_rows(id, HandlerId::OnRawInput)
        .expect("handler rows");
    assert_eq!(h_rows.len(), 1);
    assert_eq!(h_rows[0].entry.meta.priority.band, PriorityBand::Base);
    assert_eq!(h_rows[0].entry.meta.priority.priority, 700);

    let p_rows = r
        .projector_rows(id, ProjectorId::Render)
        .expect("projector rows");
    assert_eq!(p_rows.len(), 1);
    assert_eq!(p_rows[0].entry.meta.priority.priority, 700);
});

arch_test!(handler_rows_sort_by_priority_then_owner_then_declaration, {
    let mut r = DomainRouter::new();
    let id = domain(1);
    register_owner(&mut r, owner(1), 9);
    register_owner(&mut r, owner(2), 1);
    register_owner(&mut r, owner(3), 0);
    register_manifest(&mut r, id, owner(1), &[HandlerId::OnRawInput], &[]);
    let low = HandlerEntry::new(
        id,
        HandlerId::OnRawInput,
        owner(2),
        1,
        0,
        PriorityBand::Base,
        700,
        false,
    )
    .expect("low entry");
    let high =
        HandlerEntry::new(id, HandlerId::OnRawInput, owner(1), 9, 0, PriorityBand::Pre, 950, false)
            .expect("high entry");
    let same_priority_first_owner = HandlerEntry::new(
        id,
        HandlerId::OnRawInput,
        owner(3),
        0,
        9,
        PriorityBand::Base,
        700,
        false,
    )
    .expect("owner entry");

    r.register_handler_entry(low, &HANDLER).expect("low row");
    r.register_handler_entry(high, &HANDLER).expect("high row");
    r.register_handler_entry(same_priority_first_owner, &HANDLER)
        .expect("owner row");

    let rows = r
        .handler_rows(id, HandlerId::OnRawInput)
        .expect("handler rows");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].entry.meta.priority.priority, 950);
    assert_eq!(rows[1].entry.meta.owner_order, 0);
    assert_eq!(rows[2].entry.meta.owner_order, 1);
});

arch_test!(projector_rows_sort_by_declaration_order_within_owner, {
    let mut r = DomainRouter::new();
    let id = domain(1);
    register_owner(&mut r, owner(1), 0);
    register_manifest(&mut r, id, owner(1), &[], &[ProjectorId::Render]);
    let second_decl = ProjectorEntry::new(
        id,
        ProjectorId::Render,
        owner(1),
        0,
        1,
        PriorityBand::Base,
        700,
        false,
    )
    .expect("second decl");
    let first_decl = ProjectorEntry::new(
        id,
        ProjectorId::Render,
        owner(1),
        0,
        0,
        PriorityBand::Base,
        700,
        false,
    )
    .expect("first decl");

    r.register_projector_entry(second_decl, &PROJECTOR)
        .expect("second row");
    r.register_projector_entry(first_decl, &PROJECTOR)
        .expect("first row");

    let rows = r
        .projector_rows(id, ProjectorId::Render)
        .expect("projector rows");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].entry.meta.declaration_order, 0);
    assert_eq!(rows[1].entry.meta.declaration_order, 1);
});

arch_test!(manifest_registration_round_trips_by_domain_id, {
    let mut r = DomainRouter::new();
    let id = domain(4);
    register_owner(&mut r, owner(1), 0);
    let manifest = DomainManifest::new(
        id,
        Bytes::try_from_slice(b"text").expect("alloc"),
        owner(1),
        DomainApiVersion::new(0, 16),
    );
    r.register_manifest(manifest).expect("manifest row");
    let got = r.manifest(id).expect("manifest exists");
    assert_eq!(got.id, id);
    assert_eq!(got.name.as_slice(), b"text");
    assert_eq!(got.owner_cdylib_id, owner(1));
});

arch_test!(same_owner_late_handler_registration_is_allowed_during_init, {
    let mut r = DomainRouter::new();
    let id = domain(1);
    register_owner(&mut r, owner(1), 0);
    register_manifest(&mut r, id, owner(1), &[HandlerId::OnRawInput], &[]);

    let late =
        HandlerEntry::new(id, HandlerId::OnDetach, owner(1), 0, 1, PriorityBand::Post, 50, true)
            .expect("late entry");

    r.register_handler_entry(late, &HANDLER)
        .expect("same-owner late init row");
    let rows = r.handler_rows(id, HandlerId::OnDetach).expect("late rows");
    assert!(rows[0].entry.meta.late_registered);
});

arch_test!(post_init_late_handler_registration_is_rejected, {
    let mut r = DomainRouter::new();
    let id = domain(1);
    register_owner(&mut r, owner(1), 0);
    register_manifest(&mut r, id, owner(1), &[HandlerId::OnRawInput], &[]);
    r.activate_owner(owner(1)).expect("owner active");

    let late =
        HandlerEntry::new(id, HandlerId::OnDetach, owner(1), 0, 1, PriorityBand::Post, 50, true)
            .expect("late entry");

    assert!(r.register_handler_entry(late, &HANDLER).is_err(), "post-init rows are rejected");
});

arch_test!(undeclared_cross_owner_late_handler_registration_is_rejected, {
    let mut r = DomainRouter::new();
    let id = domain(1);
    register_owner(&mut r, owner(1), 0);
    register_owner(&mut r, owner(2), 1);
    register_manifest(&mut r, id, owner(1), &[HandlerId::OnRawInput], &[]);

    let late =
        HandlerEntry::new(id, HandlerId::OnDetach, owner(2), 1, 0, PriorityBand::Post, 50, true)
            .expect("late entry");

    assert!(
        r.register_handler_entry(late, &HANDLER).is_err(),
        "undeclared late rows must be same-owner"
    );
});

arch_test!(revoke_owner_removes_static_rows_and_marks_owner_failed, {
    let mut r = DomainRouter::new();
    let id = domain(1);
    register_owner(&mut r, owner(1), 0);
    register_owner(&mut r, owner(2), 1);
    register_manifest(&mut r, id, owner(1), &[HandlerId::OnRawInput], &[]);

    let first = HandlerEntry::new(
        id,
        HandlerId::OnRawInput,
        owner(1),
        0,
        0,
        PriorityBand::Base,
        700,
        false,
    )
    .expect("first entry");
    let second = HandlerEntry::new(
        id,
        HandlerId::OnRawInput,
        owner(2),
        1,
        0,
        PriorityBand::Base,
        700,
        false,
    )
    .expect("second entry");
    r.register_handler_entry(first, &HANDLER)
        .expect("first row");
    r.register_handler_entry(second, &HANDLER)
        .expect("second row");

    let removed = r.revoke_owner(owner(2)).expect("revoke owner 2");
    assert_eq!(removed, 1);
    let rows = r
        .handler_rows(id, HandlerId::OnRawInput)
        .expect("remaining rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].entry.meta.owner_cdylib_id, owner(1));
    assert_eq!(r.owner_record(owner(2)).expect("owner record").phase, RegistrationPhase::Failed);
});

// ── Default impl coverage (router.rs line 241-243) ───────────────────────────
//
// `impl Default for DomainRouter` delegates to `Self::new()`. Calling it
// directly exercises lines 241-243 which are otherwise unreachable by the
// `new()` tests (the compiler may or may not inline them into the same
// region, but the coverage tool tracks the explicit `default()` call site).

arch_test!(domain_router_default_equals_new, {
    // `Default::default()` must produce an equivalent empty router.
    let r: DomainRouter = Default::default();
    assert!(r.name_of_id(1).is_none(), "default DomainRouter is empty");
});
