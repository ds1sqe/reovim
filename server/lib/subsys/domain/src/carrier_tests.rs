//! Tests for `carrier.rs` — carrier header layout and structural status.

use core::num::NonZeroU32;

use reovim_arch::{arch_test, ds::Bytes};

use crate::{
    carrier::{
        CarrierStatus, CursorCarrier, CursorHeader, KERNEL_FLAG, PositionCarrier, PositionHeader,
    },
    id::DomainId,
};

arch_test!(position_header_little_endian_layout, {
    let h = PositionHeader::from_raw_parts(0x0102_0304, 0x0506, 0x0708);
    assert_eq!(h.bytes, [0x04, 0x03, 0x02, 0x01, 0x06, 0x05, 0x08, 0x07]);
    assert_eq!(h.domain_raw(), 0x0102_0304);
    assert_eq!(h.inner_id(), 0x0506);
    assert_eq!(h.flags(), 0x0708);
});

arch_test!(cursor_header_detects_kernel_flag, {
    let h = CursorHeader::from_raw_parts(1, 0, KERNEL_FLAG);
    assert!(h.has_kernel_flag());
});

arch_test!(typed_header_constructors_use_domain_id, {
    let id = DomainId::new(NonZeroU32::new(7).unwrap());
    let p = PositionHeader::new(id, 9, 0);
    let c = CursorHeader::new(id, 10, 1);
    assert_eq!(p.domain_raw(), 7);
    assert_eq!(p.inner_id(), 9);
    assert_eq!(c.domain_raw(), 7);
    assert_eq!(c.inner_id(), 10);
});

arch_test!(position_carrier_zero_domain_non_empty_is_invalid, {
    let mut content = Bytes::new();
    content.try_push(1).expect("alloc");
    let carrier = PositionCarrier::new(PositionHeader::from_raw_parts(0, 1, 0), content);
    assert_eq!(carrier.structural_status(), CarrierStatus::Invalid);
});

arch_test!(cursor_carrier_empty_without_domain_is_valid_opaque, {
    let carrier = CursorCarrier::new(CursorHeader::from_raw_parts(0, 1, 0), Bytes::new());
    assert_eq!(carrier.structural_status(), CarrierStatus::ValidOpaque);
});
