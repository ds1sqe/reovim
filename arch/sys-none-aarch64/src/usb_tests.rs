//! Tests for BCM2711 USB raw probe helpers.

use {
    super::{
        BCM2711_DWC2_BUS_BASE, BCM2711_DWC2_MMIO_BASE, BCM2711_XHCI_BUS_BASE,
        BCM2711_XHCI_MMIO_BASE, decode_xhci_capabilities,
    },
    reovim_testrt::{self as testrt, arch_test},
};

arch_test!(bcm2711_usb_bus_addresses_translate_to_arm_physical_mmio, {
    testrt::check_eq(BCM2711_DWC2_BUS_BASE, 0x7e98_0000u64);
    testrt::check_eq(BCM2711_DWC2_MMIO_BASE, 0xfe98_0000usize);
    testrt::check_eq(BCM2711_XHCI_BUS_BASE, 0x7e9c_0000u64);
    testrt::check_eq(BCM2711_XHCI_MMIO_BASE, 0xfe9c_0000usize);
});

arch_test!(xhci_capability_decode_extracts_controller_shape, {
    let caps = decode_xhci_capabilities(
        0x0100_0040, // HCIVERSION=1.0, CAPLENGTH=0x40
        (4 << 24) | (2 << 8) | 8,
        0x0000_0001,
        0x0000_1003,
        0x0000_2020,
    )
    .unwrap_or_else(|| panic!("valid xHCI capability registers decode"));

    testrt::check_eq(caps.cap_length, 0x40u8);
    testrt::check_eq(caps.hci_version, 0x0100u16);
    testrt::check_eq(caps.max_device_slots, 8u8);
    testrt::check_eq(caps.max_interrupters, 2u16);
    testrt::check_eq(caps.max_ports, 4u8);
    testrt::check_eq(caps.hcc_params1, 0x0000_0001u32);
    testrt::check_eq(caps.doorbell_offset, 0x1000u32);
    testrt::check_eq(caps.runtime_register_space_offset, 0x2020u32);
});

arch_test!(xhci_capability_decode_rejects_absent_or_invalid_blocks, {
    testrt::check(
        decode_xhci_capabilities(0, 0, 0, 0, 0).is_none(),
        "zeroed MMIO does not look like xHCI",
    );
    testrt::check(
        decode_xhci_capabilities(0xffff_ffff, 0, 0, 0, 0).is_none(),
        "all-ones MMIO does not look like xHCI",
    );
    testrt::check(
        decode_xhci_capabilities(0x0100_0010, 0, 0, 0, 0).is_none(),
        "too-small capability length is rejected",
    );
});
