//! Tests for BCM2711 USB raw probe helpers.

use {
    super::{
        BCM2711_DWC2_BUS_BASE, BCM2711_DWC2_MMIO_BASE, BCM2711_XHCI_BUS_BASE,
        BCM2711_XHCI_MMIO_BASE, PcieXhciController, UsbBootKeyboardPending, UsbBootKeyboardPoll,
        XHCI_EVENT_RING_TRBS, XHCI_STATIC_SCRATCHPAD_BUFFERS, XhciControllerStartStatus,
        XhciDriverMemoryStatus, XhciEnableSlotStatus, classify_enable_slot_event,
        decode_xhci_capabilities, decode_xhci_command_completion_event,
        decode_xhci_operational_snapshot, decode_xhci_port_snapshot,
        decode_xhci_supported_protocol, prepare_xhci_driver_memory, protocol_covers_port,
        xhci_controller_start_registers, xhci_enable_slot_command_trb,
        xhci_extended_capability_offset,
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
        (2 << 27) | (1 << 26) | (3 << 4),
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
    testrt::check_eq(xhci_extended_capability_offset(caps), None);
    testrt::check_eq(caps.hcs_params2, (2 << 27) | (1 << 26) | (3 << 4));
    testrt::check_eq(caps.doorbell_offset, 0x1000u32);
    testrt::check_eq(caps.runtime_register_space_offset, 0x2020u32);
    testrt::check_eq(caps.event_ring_segment_table_max, 8u16);
    testrt::check_eq(caps.max_scratchpad_buffers, 2u16);
    testrt::check(caps.scratchpad_restore, "scratchpad restore bit decodes");
    testrt::check_eq(caps.context_size_bytes, 32u8);

    let wide_contexts = decode_xhci_capabilities(0x0100_0040, (1 << 24) | 1, 0, 1 << 2, 0, 0)
        .unwrap_or_else(|| panic!("valid 64-byte-context controller decodes"));
    testrt::check_eq(wide_contexts.context_size_bytes, 64u8);
});

arch_test!(xhci_extended_capability_offset_decodes_hccparams1_xecp, {
    let caps = decode_xhci_capabilities(
        0x0100_0040,
        (4 << 24) | (1 << 8) | 8,
        0,
        0x0068_0000,
        0x1000,
        0x2000,
    )
    .unwrap_or_else(|| panic!("valid xHCI capability registers decode"));

    testrt::check_eq(xhci_extended_capability_offset(caps), Some(0x1a0usize));
});

arch_test!(xhci_capability_decode_rejects_absent_or_invalid_blocks, {
    testrt::check(
        decode_xhci_capabilities(0, 0, 0, 0, 0, 0).is_none(),
        "zeroed MMIO does not look like xHCI",
    );
    testrt::check(
        decode_xhci_capabilities(0xffff_ffff, 0, 0, 0, 0, 0).is_none(),
        "all-ones MMIO does not look like xHCI",
    );
    testrt::check(
        decode_xhci_capabilities(0x0100_0010, 0, 0, 0, 0, 0).is_none(),
        "too-small capability length is rejected",
    );
    testrt::check(
        decode_xhci_capabilities(0x0100_0040, 0, 0, 0, 0, 0).is_none(),
        "zero MaxSlots is rejected",
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
        0x0000_0011,
        0x0000_0001,
        0x0000_0000,
        0x0000_0001_0000_0041,
        0x0000_0002_0000_1000,
        0x0000_0008,
    );

    testrt::check_eq(snapshot.usb_command, 0x0000_0001u32);
    testrt::check_eq(snapshot.usb_status, 0x0000_0011u32);
    testrt::check_eq(snapshot.page_size, 0x0000_0001u32);
    testrt::check_eq(snapshot.command_ring_control, 0x0000_0001_0000_0041u64);
    testrt::check_eq(snapshot.device_context_base_address_array_pointer, 0x0000_0002_0000_1000u64);
    testrt::check_eq(snapshot.configure, 0x0000_0008u32);
    testrt::check_eq(snapshot.enabled_device_slots, 8u8);
    testrt::check(snapshot.run_stop, "run/stop bit is set");
    testrt::check(!snapshot.reset_active, "controller reset bit is clear");
    testrt::check(snapshot.halted, "halted bit is set");
    testrt::check(snapshot.port_change_detect, "port-change-detect bit is set");
    testrt::check(!snapshot.controller_not_ready, "controller not-ready bit is clear");
    testrt::check(!snapshot.host_system_error, "host-system-error bit is clear");
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

arch_test!(usb_boot_keyboard_poll_names_provider_blockers, {
    let init =
        UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::NeedsControllerInitialization {
            max_slots: 8,
            port: 3,
            speed: 2,
            link_state: 0,
        });

    match init {
        UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::NeedsControllerInitialization {
            max_slots,
            port,
            speed,
            link_state,
        }) => {
            testrt::check_eq(max_slots, 8u8);
            testrt::check_eq(port, 3u8);
            testrt::check_eq(speed, 2u8);
            testrt::check_eq(link_state, 0u8);
        }
        _ => testrt::check(false, "poll reports controller init blocker"),
    }

    let enumerate = UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::NeedsEnumeration {
        port: 4,
        speed: 3,
        link_state: 1,
    });

    match enumerate {
        UsbBootKeyboardPoll::Pending(UsbBootKeyboardPending::NeedsEnumeration {
            port,
            speed,
            link_state,
        }) => {
            testrt::check_eq(port, 4u8);
            testrt::check_eq(speed, 3u8);
            testrt::check_eq(link_state, 1u8);
        }
        _ => testrt::check(false, "poll reports enumeration blocker"),
    }
});

arch_test!(xhci_enable_slot_command_trb_sets_cycle_type_and_slot_type, {
    let default_slot = xhci_enable_slot_command_trb(0);
    testrt::check_eq(default_slot[0], 0u32);
    testrt::check_eq(default_slot[1], 0u32);
    testrt::check_eq(default_slot[2], 0u32);
    testrt::check_eq(default_slot[3], (9u32 << 10) | 1);

    let typed_slot = xhci_enable_slot_command_trb(0x23);
    testrt::check_eq(typed_slot[3], (3u32 << 16) | (9u32 << 10) | 1);
});

arch_test!(xhci_supported_protocol_decode_extracts_port_range_and_slot_type, {
    let protocol = decode_xhci_supported_protocol(
        0x1a0,
        (3u32 << 24) | (0x10u32 << 16) | 2,
        u32::from_le_bytes(*b"USB "),
        (4u32 << 28) | (0x012u32 << 16) | (3u32 << 8) | 2,
        0x1f,
    );

    testrt::check_eq(protocol.offset, 0x1a0u32);
    testrt::check_eq(protocol.name, *b"USB ");
    testrt::check_eq(protocol.major_revision, 3u8);
    testrt::check_eq(protocol.minor_revision, 0x10u8);
    testrt::check_eq(protocol.compatible_port_offset, 2u8);
    testrt::check_eq(protocol.compatible_port_count, 3u8);
    testrt::check_eq(protocol.protocol_defined, 0x012u16);
    testrt::check_eq(protocol.protocol_speed_id_count, 4u8);
    testrt::check_eq(protocol.protocol_slot_type, 31u8);
    testrt::check(!protocol_covers_port(protocol, 1), "port before range is rejected");
    testrt::check(protocol_covers_port(protocol, 2), "first compatible port is covered");
    testrt::check(protocol_covers_port(protocol, 4), "last compatible port is covered");
    testrt::check(!protocol_covers_port(protocol, 5), "port after range is rejected");

    let trb = xhci_enable_slot_command_trb(protocol.protocol_slot_type);
    testrt::check_eq(trb[3], (31u32 << 16) | (9u32 << 10) | 1);
});

arch_test!(xhci_command_completion_event_decode_extracts_pointer_status_and_slot, {
    let raw = [
        0x0000_1008,
        0x0000_0002,
        1u32 << 24,
        (7u32 << 24) | (33u32 << 10) | 1,
    ];

    let event = decode_xhci_command_completion_event(raw);

    testrt::check_eq(event.raw, raw);
    testrt::check_eq(event.command_trb_pointer, 0x0000_0002_0000_1000u64);
    testrt::check_eq(event.completion_code, 1u8);
    testrt::check_eq(event.trb_type, 33u8);
    testrt::check(event.cycle, "event cycle bit is set");
    testrt::check_eq(event.slot_id, 7u8);
});

arch_test!(xhci_enable_slot_event_classifier_names_success_and_failures, {
    let success = decode_xhci_command_completion_event([
        0x0000_1000,
        0,
        1u32 << 24,
        (4u32 << 24) | (33u32 << 10) | 1,
    ]);
    match classify_enable_slot_event(0x1000, success) {
        XhciEnableSlotStatus::SlotEnabled { slot_id } => {
            testrt::check_eq(slot_id, 4u8);
        }
        _ => testrt::check(false, "successful completion enables a slot"),
    }

    let failed =
        decode_xhci_command_completion_event([0x0000_1000, 0, 5u32 << 24, (33u32 << 10) | 1]);
    match classify_enable_slot_event(0x1000, failed) {
        XhciEnableSlotStatus::CommandFailed {
            completion_code,
            slot_id,
        } => {
            testrt::check_eq(completion_code, 5u8);
            testrt::check_eq(slot_id, 0u8);
        }
        _ => testrt::check(false, "failed completion keeps slot unavailable"),
    }

    let wrong_type =
        decode_xhci_command_completion_event([0x0000_1000, 0, 1u32 << 24, (32u32 << 10) | 1]);
    match classify_enable_slot_event(0x1000, wrong_type) {
        XhciEnableSlotStatus::UnexpectedEventType {
            trb_type,
            completion_code,
        } => {
            testrt::check_eq(trb_type, 32u8);
            testrt::check_eq(completion_code, 1u8);
        }
        _ => testrt::check(false, "wrong event type is reported"),
    }

    let wrong_pointer = decode_xhci_command_completion_event([
        0x0000_2000,
        0,
        1u32 << 24,
        (2u32 << 24) | (33u32 << 10) | 1,
    ]);
    match classify_enable_slot_event(0x1000, wrong_pointer) {
        XhciEnableSlotStatus::CommandPointerMismatch {
            expected,
            actual,
            completion_code,
            slot_id,
        } => {
            testrt::check_eq(expected, 0x1000u64);
            testrt::check_eq(actual, 0x2000u64);
            testrt::check_eq(completion_code, 1u8);
            testrt::check_eq(slot_id, 2u8);
        }
        _ => testrt::check(false, "wrong command pointer is reported"),
    }
});

arch_test!(xhci_driver_memory_plan_prepares_aligned_static_tables, {
    let caps =
        decode_xhci_capabilities(0x0100_0040, (4 << 24) | (1 << 8) | 8, 2 << 27, 0, 0x1000, 0x2000)
            .unwrap_or_else(|| panic!("valid xHCI capability registers decode"));

    let status = prepare_xhci_driver_memory(caps);
    let XhciDriverMemoryStatus::Ready(plan) = status else {
        testrt::check(false, "static xHCI memory is sufficient");
        return;
    };

    testrt::check_eq(plan.max_slots_enabled, 8u8);
    testrt::check_eq(plan.context_size_bytes, 32u8);
    testrt::check_eq(plan.scratchpad_buffers, 2u16);
    testrt::check_eq(plan.event_ring_segment_table_entries, 1u16);
    testrt::check_eq(plan.event_ring_trbs, XHCI_EVENT_RING_TRBS as u16);
    testrt::check_eq(plan.command_ring_control, plan.command_ring | 1);
    testrt::check_eq(plan.event_ring_dequeue_pointer, plan.event_ring);
    testrt::check_eq(plan.dcbaa & 0x3f, 0u64);
    testrt::check_eq(plan.command_ring & 0x3f, 0u64);
    testrt::check_eq(plan.event_ring & 0x3f, 0u64);
    testrt::check_eq(plan.event_ring_segment_table & 0x3f, 0u64);
    testrt::check_eq(plan.scratchpad_array & 0x3f, 0u64);

    let too_many = decode_xhci_capabilities(
        0x0100_0040,
        (1 << 24) | 8,
        (XHCI_STATIC_SCRATCHPAD_BUFFERS as u32 + 1) << 27,
        0,
        0,
        0,
    )
    .unwrap_or_else(|| panic!("valid xHCI capability registers decode"));

    match prepare_xhci_driver_memory(too_many) {
        XhciDriverMemoryStatus::TooManyScratchpads {
            requested,
            supported,
        } => {
            testrt::check_eq(requested, XHCI_STATIC_SCRATCHPAD_BUFFERS as u16 + 1);
            testrt::check_eq(supported, XHCI_STATIC_SCRATCHPAD_BUFFERS as u16);
        }
        _ => testrt::check(false, "too many scratchpads is refused"),
    }
});

arch_test!(xhci_controller_start_registers_map_memory_plan_to_mmio_values, {
    let caps =
        decode_xhci_capabilities(0x0100_0040, (4 << 24) | (1 << 8) | 8, 0, 0, 0x1000, 0x2000)
            .unwrap_or_else(|| panic!("valid xHCI capability registers decode"));
    let XhciDriverMemoryStatus::Ready(plan) = prepare_xhci_driver_memory(caps) else {
        testrt::check(false, "static xHCI memory is sufficient");
        return;
    };

    let registers = xhci_controller_start_registers(plan);

    testrt::check_eq(registers.device_context_base_address_array_pointer, plan.dcbaa);
    testrt::check_eq(registers.command_ring_control, plan.command_ring_control);
    testrt::check_eq(registers.configure, plan.max_slots_enabled as u32);
    testrt::check_eq(registers.interrupter_management, 0u32);
    testrt::check_eq(registers.interrupter_moderation, 0u32);
    testrt::check_eq(
        registers.event_ring_segment_table_size,
        plan.event_ring_segment_table_entries as u32,
    );
    testrt::check_eq(
        registers.event_ring_segment_table_base_address,
        plan.event_ring_segment_table,
    );
    testrt::check_eq(registers.event_ring_dequeue_pointer, plan.event_ring_dequeue_pointer);
    testrt::check_eq(registers.usb_status_clear, (1 << 2) | (1 << 3) | (1 << 4));
    testrt::check_eq(registers.usb_command, 1u32);

    let unavailable = XhciControllerStartStatus::DriverMemoryUnavailable {
        requested: 9,
        supported: XHCI_STATIC_SCRATCHPAD_BUFFERS as u16,
    };
    match unavailable {
        XhciControllerStartStatus::DriverMemoryUnavailable {
            requested,
            supported,
        } => {
            testrt::check_eq(requested, 9u16);
            testrt::check_eq(supported, XHCI_STATIC_SCRATCHPAD_BUFFERS as u16);
        }
        _ => testrt::check(false, "driver memory blocker carries scratchpad counts"),
    }
});
