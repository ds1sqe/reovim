//! Tests for `service.rs` — flags, row state, descriptor metadata, and leases.

use core::num::NonZeroU32;

use {reovim_arch::arch_test, reovim_lib_ds::Bytes};

use crate::{
    id::{CdylibId, ServiceKey, ServiceLeaseId},
    routing::DomainApiVersion,
    service::{ServiceBorrow, ServiceDescriptorMeta, ServiceFlags, ServiceLease, ServiceRowState},
};

fn owner(raw: u32) -> CdylibId {
    CdylibId::new(NonZeroU32::new(raw).unwrap())
}

arch_test!(service_flags_report_send_and_sync_safety, {
    let flags = ServiceFlags::new(ServiceFlags::SEND_SAFE | ServiceFlags::HOSTAPI_REENTRANT);
    assert!(flags.send_safe());
    assert!(!flags.sync_safe());
    assert!(flags.contains(ServiceFlags::HOSTAPI_REENTRANT));
});

arch_test!(service_row_state_hides_draining_and_revoked_rows, {
    assert!(ServiceRowState::Visible.is_visible());
    assert!(!ServiceRowState::DrainingHidden.is_visible());
    assert!(!ServiceRowState::Revoked.is_visible());
});

arch_test!(service_descriptor_metadata_preserves_diagnostic_fields, {
    let meta = ServiceDescriptorMeta::new(
        ServiceKey::new(1),
        owner(2),
        DomainApiVersion::new(0, 16),
        DomainApiVersion::new(1, 0),
        Bytes::try_from_slice(b"svc.example").expect("alloc"),
        64,
        ServiceFlags::new(ServiceFlags::SEND_SAFE | ServiceFlags::SYNC_SAFE),
    );
    assert_eq!(meta.key, ServiceKey::new(1));
    assert_eq!(meta.owner_cdylib_id, owner(2));
    assert_eq!(meta.type_name.as_slice(), b"svc.example");
    assert!(meta.flags.send_safe());
    assert!(meta.flags.sync_safe());
});

arch_test!(service_borrow_captures_owner_generation, {
    let borrow = ServiceBorrow::new(ServiceKey::new(7), owner(3), 99);
    assert_eq!(borrow.key, ServiceKey::new(7));
    assert_eq!(borrow.owner_cdylib_id, owner(3));
    assert_eq!(borrow.owner_generation, 99);
});

arch_test!(service_lease_checks_timeout_and_generation, {
    let lease =
        ServiceLease::new(ServiceLeaseId::new(1), ServiceKey::new(2), owner(4), 10, 100, 50);
    assert!(!lease.is_expired(150));
    assert!(lease.is_expired(151));
    assert!(lease.matches_generation(10));
    assert!(!lease.matches_generation(11));
});
