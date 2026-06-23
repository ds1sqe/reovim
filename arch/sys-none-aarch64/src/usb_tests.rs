//! Tests for BCM2711 USB raw probe helpers.

use {
    super::{
        BCM2711_DWC2_BUS_BASE, BCM2711_DWC2_MMIO_BASE, BCM2711_XHCI_BUS_BASE,
        BCM2711_XHCI_MMIO_BASE, PcieXhciController, decode_xhci_capabilities,
        decode_xhci_operational_snapshot, decode_xhci_port_snapshot,
    },
    crate::pcie::{PciConfigHeader, PciLocation},
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

arch_test!(pcie_xhci_controller_summary_requires_xhci_class, {
    let header = PciConfigHeader {
        location: PciLocation::new(1, 0, 0).unwrap_or_else(|| panic!("valid pci location")),
        vendor_id: 0x1106,
        device_id: 0x3483,
        command: 0,
        status: 0,
        revision_id: 3,
        prog_if: 0x30,
        subclass: 0x03,
        class_code: 0x0c,
        bar0: 0xc002_0000,
        bar1: 0,
    };

    let controller = PcieXhciController::from_config_header(header)
        .unwrap_or_else(|| panic!("xHCI class header accepted"));
    testrt::check_eq(controller.location.bus, 1u8);
    testrt::check_eq(controller.location.device, 0u8);
    testrt::check_eq(controller.location.function, 0u8);
    testrt::check_eq(controller.vendor_id, 0x1106u16);
    testrt::check_eq(controller.device_id, 0x3483u16);
    testrt::check_eq(controller.revision_id, 3u8);
    testrt::check_eq(controller.mmio_base, Some(0x6_0002_0000usize));

    let ehci_header = PciConfigHeader {
        prog_if: 0x20,
        ..header
    };
    testrt::check(
        PcieXhciController::from_config_header(ehci_header).is_none(),
        "USB EHCI class is not xHCI",
    );
});

arch_test!(xhci_operational_snapshot_decodes_configured_slots, {
    let snapshot = decode_xhci_operational_snapshot(
        0x0000_0001,
        0x0000_0010,
        0x0000_0001,
        0x0000_0000,
        0x0000_0001_0000_0041,
        0x0000_0002_0000_1000,
        0x0000_0008,
    );

    testrt::check_eq(snapshot.usb_command, 0x0000_0001u32);
    testrt::check_eq(snapshot.usb_status, 0x0000_0010u32);
    testrt::check_eq(snapshot.page_size, 0x0000_0001u32);
    testrt::check_eq(snapshot.command_ring_control, 0x0000_0001_0000_0041u64);
    testrt::check_eq(snapshot.device_context_base_address_array_pointer, 0x0000_0002_0000_1000u64);
    testrt::check_eq(snapshot.configure, 0x0000_0008u32);
    testrt::check_eq(snapshot.enabled_device_slots, 8u8);
});

arch_test!(xhci_port_snapshot_decodes_visible_status_bits, {
    let snapshot = decode_xhci_port_snapshot(2, 0x0000_0200 | (3 << 10) | (5 << 5) | 0x3);

    testrt::check_eq(snapshot.port, 2u8);
    testrt::check(snapshot.connected, "connected bit is set");
    testrt::check(snapshot.enabled, "enabled bit is set");
    testrt::check(snapshot.powered, "port power bit is set");
    testrt::check_eq(snapshot.link_state, 5u8);
    testrt::check_eq(snapshot.speed, 3u8);
    testrt::check(!snapshot.reset_active, "port reset bit is clear");

    let resetting = decode_xhci_port_snapshot(1, 1 << 4);
    testrt::check(resetting.reset_active, "port reset bit is set");
});
