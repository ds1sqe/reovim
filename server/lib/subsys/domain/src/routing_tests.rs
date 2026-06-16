//! Tests for `routing.rs` — DT10, DT17, and manifest declaration contracts.

use core::num::NonZeroU32;

use reovim_arch::{arch_test, ds::Bytes};

use crate::{
    id::{CdylibId, DomainId},
    routing::{
        DispatchEntryMeta, DomainApiVersion, DomainManifest, HandlerEntry, HandlerId, PriorityBand,
        ProjectorEntry, ProjectorId, RegistrationError, RegistrationPhase, RowKind,
    },
};

fn owner(raw: u32) -> CdylibId {
    CdylibId::new(NonZeroU32::new(raw).unwrap())
}

fn domain(raw: u32) -> DomainId {
    DomainId::new(NonZeroU32::new(raw).unwrap())
}

arch_test!(priority_bands_validate_ranges, {
    assert!(PriorityBand::Pre.contains(900));
    assert!(PriorityBand::Pre.contains(999));
    assert!(!PriorityBand::Pre.contains(899));
    assert_eq!(PriorityBand::Base.default_priority(), 700);
    assert!(matches!(
        DispatchEntryMeta::new(owner(1), 0, 0, PriorityBand::Decor, 950, false),
        Err(RegistrationError::PriorityOutOfBand {
            band: PriorityBand::Decor,
            priority: 950,
        })
    ));
});

arch_test!(registration_phase_rejects_post_init_rows, {
    assert!(
        RegistrationPhase::Init
            .validate_init_only(RowKind::Handler)
            .is_ok()
    );
    assert_eq!(
        RegistrationPhase::Active.validate_init_only(RowKind::Projector),
        Err(RegistrationError::PostInitRegistration {
            row_kind: RowKind::Projector,
        })
    );
});

arch_test!(dispatch_metadata_orders_by_priority_then_owner_then_declaration, {
    let high = DispatchEntryMeta::new(owner(1), 5, 9, PriorityBand::Pre, 950, false).unwrap();
    let low = DispatchEntryMeta::new(owner(1), 0, 0, PriorityBand::Base, 700, false).unwrap();
    assert!(high.sorts_before(low));

    let first_owner =
        DispatchEntryMeta::new(owner(1), 0, 9, PriorityBand::Base, 700, false).unwrap();
    let second_owner =
        DispatchEntryMeta::new(owner(2), 1, 0, PriorityBand::Base, 700, false).unwrap();
    assert!(first_owner.sorts_before(second_owner));

    let first_decl =
        DispatchEntryMeta::new(owner(1), 0, 0, PriorityBand::Base, 700, false).unwrap();
    let second_decl =
        DispatchEntryMeta::new(owner(1), 0, 1, PriorityBand::Base, 700, false).unwrap();
    assert!(first_decl.sorts_before(second_decl));
});

arch_test!(handler_and_projector_rows_carry_validated_metadata, {
    let h = HandlerEntry::new(
        domain(1),
        HandlerId::OnRawInput,
        owner(1),
        0,
        0,
        PriorityBand::Base,
        700,
        true,
    )
    .unwrap();
    assert_eq!(h.domain_id, domain(1));
    assert!(h.meta.late_registered);

    let p = ProjectorEntry::new(
        domain(1),
        ProjectorId::Render,
        owner(1),
        0,
        1,
        PriorityBand::Decor,
        200,
        false,
    )
    .unwrap();
    assert_eq!(p.projector_id, ProjectorId::Render);
    assert_eq!(p.meta.declaration_order, 1);
});

arch_test!(manifest_declaration_helpers_find_declared_rows, {
    let mut manifest = DomainManifest::new(
        domain(7),
        Bytes::try_from_slice(b"text").expect("alloc"),
        owner(2),
        DomainApiVersion::new(0, 16),
    );

    manifest
        .handler_kinds
        .try_push(HandlerId::OnRawInput)
        .expect("alloc");
    manifest
        .projector_kinds
        .try_push(ProjectorId::Render)
        .expect("alloc");
    manifest.position_codecs.try_push(0x42).expect("alloc");
    manifest.cursor_codecs.try_push(0x24).expect("alloc");

    assert!(manifest.declares_handler(HandlerId::OnRawInput));
    assert!(!manifest.declares_handler(HandlerId::OnAttach));
    assert!(manifest.declares_projector(ProjectorId::Render));
    assert!(manifest.declares_position_codec(0x42));
    assert!(manifest.declares_cursor_codec(0x24));
    assert_eq!(manifest.validate_owner(owner(2)), Ok(()));
    assert!(matches!(
        manifest.validate_owner(owner(3)),
        Err(RegistrationError::OwnerMismatch { .. })
    ));
});
