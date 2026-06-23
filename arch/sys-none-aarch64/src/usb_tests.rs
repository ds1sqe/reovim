//! Tests for BCM2711 USB raw probe helpers.

use {
    super::{
        BCM2711_DWC2_BUS_BASE, BCM2711_DWC2_MMIO_BASE, BCM2711_XHCI_BUS_BASE,
        BCM2711_XHCI_MMIO_BASE, PcieXhciController, USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES,
        UsbBootKeyboardPending, UsbBootKeyboardPoll, XHCI_CONTROL_ENDPOINT_RING_TRBS,
        XHCI_EP0_CONFIGURATION_DESCRIPTOR_HEADER_TRB_INDEX,
        XHCI_EP0_CONFIGURATION_DESCRIPTOR_TRB_INDEX, XHCI_EP0_DESCRIPTOR_PREFIX_TRB_INDEX,
        XHCI_EP0_DEVICE_DESCRIPTOR_TRB_INDEX, XHCI_EVENT_RING_TRBS, XHCI_STATIC_SCRATCHPAD_BUFFERS,
        XhciAddressDeviceStatus, XhciControllerStartStatus, XhciDeviceDescriptorProbeStatus,
        XhciDriverMemoryStatus, XhciEnableSlotStatus, XhciReadConfigurationDescriptorHeaderStatus,
        XhciReadConfigurationDescriptorStatus, XhciReadDeviceDescriptorStatus,
        XhciSetAddressStatus, XhciSetupPacket, classify_address_device_event,
        classify_device_descriptor_transfer_event, classify_enable_slot_event,
        classify_read_configuration_descriptor_header_transfer_event,
        classify_read_configuration_descriptor_transfer_event,
        classify_read_device_descriptor_transfer_event, classify_set_address_event,
        decode_xhci_capabilities, decode_xhci_command_completion_event,
        decode_xhci_operational_snapshot, decode_xhci_port_snapshot,
        decode_xhci_supported_protocol, decode_xhci_transfer_event,
        parse_usb_configuration_descriptor_header, parse_usb_configuration_descriptor_tree,
        parse_usb_device_descriptor, prepare_xhci_driver_memory, protocol_covers_port,
        usb_descriptor_endpoint0_max_packet_size, xhci_address_device_command_trb,
        xhci_address_device_contexts, xhci_address_device_contexts_with_max_packet_size,
        xhci_controller_start_registers, xhci_data_stage_trb, xhci_enable_slot_command_trb,
        xhci_ep0_control_trb_pointers, xhci_extended_capability_offset, xhci_setup_stage_trb,
        xhci_status_stage_trb,
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

arch_test!(xhci_address_device_command_trb_sets_pointer_bsr_type_and_slot, {
    let blocked = xhci_address_device_command_trb(0x0000_0002_0000_1008, 7, true);
    testrt::check_eq(blocked[0], 0x0000_1000u32);
    testrt::check_eq(blocked[1], 0x0000_0002u32);
    testrt::check_eq(blocked[2], 0u32);
    testrt::check_eq(blocked[3], (7u32 << 24) | (11u32 << 10) | (1u32 << 9) | 1);

    let set_address = xhci_address_device_command_trb(0x0000_1000, 3, false);
    testrt::check_eq(set_address[3], (3u32 << 24) | (11u32 << 10) | 1);
});

arch_test!(xhci_address_device_contexts_prepare_slot_and_endpoint_zero, {
    let caps =
        decode_xhci_capabilities(0x0100_0040, (4 << 24) | (1 << 8) | 8, 0, 0, 0x1000, 0x2000)
            .unwrap_or_else(|| panic!("valid xHCI capability registers decode"));
    let XhciDriverMemoryStatus::Ready(plan) = prepare_xhci_driver_memory(caps) else {
        testrt::check(false, "static xHCI memory is sufficient");
        return;
    };

    let high_speed_port = decode_xhci_port_snapshot(2, (3 << 10) | 1);
    let contexts = xhci_address_device_contexts(plan, high_speed_port);

    testrt::check_eq(contexts.input_context, plan.input_context);
    testrt::check_eq(contexts.output_device_context, plan.output_device_context);
    testrt::check_eq(contexts.control_endpoint_ring, plan.control_endpoint_ring);
    testrt::check_eq(contexts.drop_context_flags, 0u32);
    testrt::check_eq(contexts.add_context_flags, 0x3u32);
    testrt::check_eq(contexts.slot_context[0], (3u32 << 20) | (1u32 << 27));
    testrt::check_eq(contexts.slot_context[1], 2u32 << 16);
    testrt::check_eq(contexts.slot_context[2], 0u32);
    testrt::check_eq(contexts.slot_context[3], 0u32);
    testrt::check_eq(contexts.endpoint0_max_packet_size, 64u16);
    testrt::check_eq(contexts.endpoint0_context[0], 0u32);
    testrt::check_eq(contexts.endpoint0_context[1], (64u32 << 16) | (4u32 << 3) | (3u32 << 1));
    testrt::check_eq(contexts.endpoint0_context[2], (plan.control_endpoint_ring | 1) as u32);
    testrt::check_eq(
        contexts.endpoint0_context[3],
        ((plan.control_endpoint_ring | 1) >> 32) as u32,
    );
    testrt::check_eq(contexts.endpoint0_context[4], 8u32);

    let full_speed =
        xhci_address_device_contexts(plan, decode_xhci_port_snapshot(1, (1 << 10) | 1));
    testrt::check_eq(full_speed.endpoint0_max_packet_size, 8u16);
    let super_speed =
        xhci_address_device_contexts(plan, decode_xhci_port_snapshot(1, (4 << 10) | 1));
    testrt::check_eq(super_speed.endpoint0_max_packet_size, 512u16);
});

arch_test!(xhci_address_device_event_classifier_names_success_and_failures, {
    let success = decode_xhci_command_completion_event([
        0x0000_2000,
        0,
        1u32 << 24,
        (5u32 << 24) | (33u32 << 10) | 1,
    ]);
    match classify_address_device_event(0x2000, 5, success) {
        XhciAddressDeviceStatus::DefaultControlEndpointReady { slot_id } => {
            testrt::check_eq(slot_id, 5u8);
        }
        _ => testrt::check(false, "successful completion prepares endpoint zero"),
    }

    let failed = decode_xhci_command_completion_event([
        0x0000_2000,
        0,
        5u32 << 24,
        (5u32 << 24) | (33u32 << 10) | 1,
    ]);
    match classify_address_device_event(0x2000, 5, failed) {
        XhciAddressDeviceStatus::CommandFailed {
            completion_code,
            slot_id,
        } => {
            testrt::check_eq(completion_code, 5u8);
            testrt::check_eq(slot_id, 5u8);
        }
        _ => testrt::check(false, "failed completion is reported"),
    }

    let wrong_slot = decode_xhci_command_completion_event([
        0x0000_2000,
        0,
        1u32 << 24,
        (4u32 << 24) | (33u32 << 10) | 1,
    ]);
    match classify_address_device_event(0x2000, 5, wrong_slot) {
        XhciAddressDeviceStatus::SlotIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            testrt::check_eq(expected, 5u8);
            testrt::check_eq(actual, 4u8);
            testrt::check_eq(completion_code, 1u8);
        }
        _ => testrt::check(false, "wrong slot ID is reported"),
    }

    let wrong_pointer = decode_xhci_command_completion_event([
        0x0000_3000,
        0,
        1u32 << 24,
        (5u32 << 24) | (33u32 << 10) | 1,
    ]);
    match classify_address_device_event(0x2000, 5, wrong_pointer) {
        XhciAddressDeviceStatus::CommandPointerMismatch {
            expected,
            actual,
            completion_code,
            slot_id,
        } => {
            testrt::check_eq(expected, 0x2000u64);
            testrt::check_eq(actual, 0x3000u64);
            testrt::check_eq(completion_code, 1u8);
            testrt::check_eq(slot_id, 5u8);
        }
        _ => testrt::check(false, "wrong command pointer is reported"),
    }
});

arch_test!(xhci_get_descriptor_control_trbs_encode_setup_data_and_status_ioc, {
    let setup = XhciSetupPacket {
        bm_request_type: 0x80,
        b_request: 6,
        w_value: 1 << 8,
        w_index: 0,
        w_length: 8,
    };

    let setup_trb = xhci_setup_stage_trb(setup);
    testrt::check_eq(setup_trb[0], 0x0100_0680u32);
    testrt::check_eq(setup_trb[1], 0x0008_0000u32);
    testrt::check_eq(setup_trb[2], 8u32);
    testrt::check_eq(setup_trb[3], (3u32 << 16) | (2u32 << 10) | (1u32 << 6) | 1);

    let data_trb = xhci_data_stage_trb(0x0000_0002_0000_0040, 8, true);
    testrt::check_eq(data_trb[0], 0x0000_0040u32);
    testrt::check_eq(data_trb[1], 0x0000_0002u32);
    testrt::check_eq(data_trb[2], 8u32);
    testrt::check_eq(data_trb[3], (1u32 << 16) | (3u32 << 10) | 1);

    let status_trb = xhci_status_stage_trb(false);
    testrt::check_eq(status_trb[0], 0u32);
    testrt::check_eq(status_trb[1], 0u32);
    testrt::check_eq(status_trb[2], 0u32);
    testrt::check_eq(status_trb[3], (4u32 << 10) | (1u32 << 5) | 1);
});

arch_test!(xhci_transfer_event_decode_extracts_pointer_residual_endpoint_and_slot, {
    let raw = [
        0x0000_3008,
        0x0000_0002,
        (1u32 << 24) | 4,
        (6u32 << 24) | (1u32 << 16) | (32u32 << 10) | 1,
    ];

    let event = decode_xhci_transfer_event(raw);

    testrt::check_eq(event.raw, raw);
    testrt::check_eq(event.trb_pointer, 0x0000_0002_0000_3000u64);
    testrt::check_eq(event.transfer_length, 4u32);
    testrt::check_eq(event.completion_code, 1u8);
    testrt::check_eq(event.trb_type, 32u8);
    testrt::check(event.cycle, "event cycle bit is set");
    testrt::check(!event.event_data, "event carries a TRB pointer");
    testrt::check_eq(event.endpoint_id, 1u8);
    testrt::check_eq(event.slot_id, 6u8);
});

arch_test!(xhci_device_descriptor_transfer_classifier_names_success_and_failures, {
    let success = decode_xhci_transfer_event([
        0x0000_3000,
        0,
        1u32 << 24,
        (6u32 << 24) | (1u32 << 16) | (32u32 << 10) | 1,
    ]);
    match classify_device_descriptor_transfer_event(0x3000, 6, 1, success) {
        XhciDeviceDescriptorProbeStatus::DescriptorPrefixReady { slot_id, length } => {
            testrt::check_eq(slot_id, 6u8);
            testrt::check_eq(length, 8u8);
        }
        _ => testrt::check(false, "successful transfer exposes descriptor prefix"),
    }

    let failed = decode_xhci_transfer_event([
        0x0000_3000,
        0,
        (13u32 << 24) | 2,
        (6u32 << 24) | (1u32 << 16) | (32u32 << 10) | 1,
    ]);
    match classify_device_descriptor_transfer_event(0x3000, 6, 1, failed) {
        XhciDeviceDescriptorProbeStatus::TransferFailed {
            completion_code,
            residual_length,
            slot_id,
            endpoint_id,
        } => {
            testrt::check_eq(completion_code, 13u8);
            testrt::check_eq(residual_length, 2u32);
            testrt::check_eq(slot_id, 6u8);
            testrt::check_eq(endpoint_id, 1u8);
        }
        _ => testrt::check(false, "failed transfer is reported"),
    }

    let wrong_pointer = decode_xhci_transfer_event([
        0x0000_4000,
        0,
        1u32 << 24,
        (6u32 << 24) | (1u32 << 16) | (32u32 << 10) | 1,
    ]);
    match classify_device_descriptor_transfer_event(0x3000, 6, 1, wrong_pointer) {
        XhciDeviceDescriptorProbeStatus::TransferPointerMismatch {
            expected,
            actual,
            completion_code,
            slot_id,
            endpoint_id,
        } => {
            testrt::check_eq(expected, 0x3000u64);
            testrt::check_eq(actual, 0x4000u64);
            testrt::check_eq(completion_code, 1u8);
            testrt::check_eq(slot_id, 6u8);
            testrt::check_eq(endpoint_id, 1u8);
        }
        _ => testrt::check(false, "wrong TRB pointer is reported"),
    }

    let wrong_endpoint = decode_xhci_transfer_event([
        0x0000_3000,
        0,
        1u32 << 24,
        (6u32 << 24) | (2u32 << 16) | (32u32 << 10) | 1,
    ]);
    match classify_device_descriptor_transfer_event(0x3000, 6, 1, wrong_endpoint) {
        XhciDeviceDescriptorProbeStatus::EndpointIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            testrt::check_eq(expected, 1u8);
            testrt::check_eq(actual, 2u8);
            testrt::check_eq(completion_code, 1u8);
        }
        _ => testrt::check(false, "wrong endpoint ID is reported"),
    }
});

arch_test!(usb_descriptor_endpoint0_max_packet_size_decodes_usb2_and_usb3_prefixes, {
    testrt::check_eq(
        usb_descriptor_endpoint0_max_packet_size([18, 1, 0x00, 0x02, 0, 0, 0, 64]),
        Some(64u16),
    );
    testrt::check_eq(
        usb_descriptor_endpoint0_max_packet_size([18, 1, 0x00, 0x03, 0, 0, 0, 9]),
        Some(512u16),
    );
    testrt::check_eq(
        usb_descriptor_endpoint0_max_packet_size([7, 1, 0x00, 0x02, 0, 0, 0, 64]),
        None,
    );
    testrt::check_eq(
        usb_descriptor_endpoint0_max_packet_size([18, 2, 0x00, 0x02, 0, 0, 0, 64]),
        None,
    );
    testrt::check_eq(
        usb_descriptor_endpoint0_max_packet_size([18, 1, 0x00, 0x02, 0, 0, 0, 9]),
        None,
    );
});

arch_test!(xhci_address_device_contexts_can_use_descriptor_max_packet_size, {
    let caps =
        decode_xhci_capabilities(0x0100_0040, (4 << 24) | (1 << 8) | 8, 0, 0, 0x1000, 0x2000)
            .unwrap_or_else(|| panic!("valid xHCI capability registers decode"));
    let XhciDriverMemoryStatus::Ready(plan) = prepare_xhci_driver_memory(caps) else {
        testrt::check(false, "static xHCI memory is sufficient");
        return;
    };

    let contexts = xhci_address_device_contexts_with_max_packet_size(
        plan,
        decode_xhci_port_snapshot(2, (3 << 10) | 1),
        16,
    );
    testrt::check_eq(contexts.endpoint0_max_packet_size, 16u16);
    testrt::check_eq(contexts.endpoint0_context[1], (16u32 << 16) | (4u32 << 3) | (3u32 << 1));
});

arch_test!(xhci_set_address_event_classifier_names_success_and_failures, {
    let success = decode_xhci_command_completion_event([
        0x0000_4000,
        0,
        1u32 << 24,
        (7u32 << 24) | (33u32 << 10) | 1,
    ]);
    match classify_set_address_event(0x4000, 7, 64, success) {
        XhciSetAddressStatus::Addressed {
            slot_id,
            endpoint0_max_packet_size,
        } => {
            testrt::check_eq(slot_id, 7u8);
            testrt::check_eq(endpoint0_max_packet_size, 64u16);
        }
        _ => testrt::check(false, "successful BSR=0 command addresses the slot"),
    }

    let failed = decode_xhci_command_completion_event([
        0x0000_4000,
        0,
        5u32 << 24,
        (7u32 << 24) | (33u32 << 10) | 1,
    ]);
    match classify_set_address_event(0x4000, 7, 64, failed) {
        XhciSetAddressStatus::CommandFailed {
            completion_code,
            slot_id,
        } => {
            testrt::check_eq(completion_code, 5u8);
            testrt::check_eq(slot_id, 7u8);
        }
        _ => testrt::check(false, "failed BSR=0 command is reported"),
    }

    let wrong_slot = decode_xhci_command_completion_event([
        0x0000_4000,
        0,
        1u32 << 24,
        (6u32 << 24) | (33u32 << 10) | 1,
    ]);
    match classify_set_address_event(0x4000, 7, 64, wrong_slot) {
        XhciSetAddressStatus::SlotIdMismatch {
            expected,
            actual,
            completion_code,
        } => {
            testrt::check_eq(expected, 7u8);
            testrt::check_eq(actual, 6u8);
            testrt::check_eq(completion_code, 1u8);
        }
        _ => testrt::check(false, "wrong slot ID is reported"),
    }
});

arch_test!(usb_device_descriptor_parser_extracts_standard_fields, {
    let raw = [
        18, 1, 0x00, 0x02, 0, 0, 0, 64, 0x34, 0x12, 0x78, 0x56, 0x00, 0x01, 1, 2, 3, 1,
    ];

    let fields =
        parse_usb_device_descriptor(raw).unwrap_or_else(|| panic!("valid Device Descriptor"));

    testrt::check_eq(fields.length, 18u8);
    testrt::check_eq(fields.descriptor_type, 1u8);
    testrt::check_eq(fields.bcd_usb, 0x0200u16);
    testrt::check_eq(fields.device_class, 0u8);
    testrt::check_eq(fields.device_subclass, 0u8);
    testrt::check_eq(fields.device_protocol, 0u8);
    testrt::check_eq(fields.max_packet_size0, 64u8);
    testrt::check_eq(fields.vendor_id, 0x1234u16);
    testrt::check_eq(fields.product_id, 0x5678u16);
    testrt::check_eq(fields.bcd_device, 0x0100u16);
    testrt::check_eq(fields.manufacturer_index, 1u8);
    testrt::check_eq(fields.product_index, 2u8);
    testrt::check_eq(fields.serial_number_index, 3u8);
    testrt::check_eq(fields.num_configurations, 1u8);

    let mut wrong_length = raw;
    wrong_length[0] = 17;
    testrt::check(
        parse_usb_device_descriptor(wrong_length).is_none(),
        "wrong Device Descriptor length is rejected",
    );

    let mut wrong_type = raw;
    wrong_type[1] = 2;
    testrt::check(
        parse_usb_device_descriptor(wrong_type).is_none(),
        "wrong descriptor type is rejected",
    );
});

arch_test!(xhci_read_device_descriptor_classifier_names_success_and_failures, {
    let descriptor = [
        18, 1, 0x00, 0x02, 0, 0, 0, 64, 0x34, 0x12, 0x78, 0x56, 0x00, 0x01, 1, 2, 3, 1,
    ];
    let success = decode_xhci_transfer_event([
        0x0000_5000,
        0,
        1u32 << 24,
        (8u32 << 24) | (1u32 << 16) | (32u32 << 10) | 1,
    ]);
    match classify_read_device_descriptor_transfer_event(0x5000, 8, 1, descriptor, success) {
        XhciReadDeviceDescriptorStatus::DeviceDescriptorReady {
            slot_id,
            vendor_id,
            product_id,
            num_configurations,
        } => {
            testrt::check_eq(slot_id, 8u8);
            testrt::check_eq(vendor_id, 0x1234u16);
            testrt::check_eq(product_id, 0x5678u16);
            testrt::check_eq(num_configurations, 1u8);
        }
        _ => testrt::check(false, "successful transfer exposes full Device Descriptor"),
    }

    let mut invalid = descriptor;
    invalid[0] = 0;
    match classify_read_device_descriptor_transfer_event(0x5000, 8, 1, invalid, success) {
        XhciReadDeviceDescriptorStatus::InvalidDeviceDescriptor {
            length,
            descriptor_type,
        } => {
            testrt::check_eq(length, 0u8);
            testrt::check_eq(descriptor_type, 1u8);
        }
        _ => testrt::check(false, "malformed descriptor is reported"),
    }

    let failed = decode_xhci_transfer_event([
        0x0000_5000,
        0,
        (13u32 << 24) | 2,
        (8u32 << 24) | (1u32 << 16) | (32u32 << 10) | 1,
    ]);
    match classify_read_device_descriptor_transfer_event(0x5000, 8, 1, descriptor, failed) {
        XhciReadDeviceDescriptorStatus::TransferFailed {
            completion_code,
            residual_length,
            slot_id,
            endpoint_id,
        } => {
            testrt::check_eq(completion_code, 13u8);
            testrt::check_eq(residual_length, 2u32);
            testrt::check_eq(slot_id, 8u8);
            testrt::check_eq(endpoint_id, 1u8);
        }
        _ => testrt::check(false, "failed transfer is reported"),
    }
});

arch_test!(xhci_ep0_control_transfer_indices_advance_by_triplet, {
    let (prefix_setup, prefix_data, prefix_status) =
        xhci_ep0_control_trb_pointers(XHCI_EP0_DESCRIPTOR_PREFIX_TRB_INDEX);
    let (device_setup, device_data, device_status) =
        xhci_ep0_control_trb_pointers(XHCI_EP0_DEVICE_DESCRIPTOR_TRB_INDEX);
    let (config_setup, config_data, config_status) =
        xhci_ep0_control_trb_pointers(XHCI_EP0_CONFIGURATION_DESCRIPTOR_HEADER_TRB_INDEX);
    let (config_tree_setup, config_tree_data, config_tree_status) =
        xhci_ep0_control_trb_pointers(XHCI_EP0_CONFIGURATION_DESCRIPTOR_TRB_INDEX);

    testrt::check_eq(device_setup - prefix_setup, 3u64 * 16);
    testrt::check_eq(device_data - prefix_data, 3u64 * 16);
    testrt::check_eq(device_status - prefix_status, 3u64 * 16);
    testrt::check_eq(config_setup - device_setup, 3u64 * 16);
    testrt::check_eq(config_data - device_data, 3u64 * 16);
    testrt::check_eq(config_status - device_status, 3u64 * 16);
    testrt::check_eq(config_tree_setup - config_setup, 3u64 * 16);
    testrt::check_eq(config_tree_data - config_data, 3u64 * 16);
    testrt::check_eq(config_tree_status - config_status, 3u64 * 16);
});

arch_test!(usb_configuration_descriptor_header_parser_extracts_standard_fields, {
    let raw = [9, 2, 34, 0, 1, 1, 0, 0x80, 50];

    let fields = parse_usb_configuration_descriptor_header(raw)
        .unwrap_or_else(|| panic!("valid Configuration Descriptor header"));

    testrt::check_eq(fields.length, 9u8);
    testrt::check_eq(fields.descriptor_type, 2u8);
    testrt::check_eq(fields.total_length, 34u16);
    testrt::check_eq(fields.num_interfaces, 1u8);
    testrt::check_eq(fields.configuration_value, 1u8);
    testrt::check_eq(fields.configuration_index, 0u8);
    testrt::check_eq(fields.attributes, 0x80u8);
    testrt::check_eq(fields.max_power, 50u8);

    let mut wrong_length = raw;
    wrong_length[0] = 8;
    testrt::check(
        parse_usb_configuration_descriptor_header(wrong_length).is_none(),
        "wrong Configuration Descriptor header length is rejected",
    );

    let mut wrong_type = raw;
    wrong_type[1] = 1;
    testrt::check(
        parse_usb_configuration_descriptor_header(wrong_type).is_none(),
        "wrong descriptor type is rejected",
    );

    let too_short_tree = [9, 2, 8, 0, 1, 1, 0, 0x80, 50];
    testrt::check(
        parse_usb_configuration_descriptor_header(too_short_tree).is_none(),
        "wTotalLength smaller than header is rejected",
    );
});

arch_test!(
    xhci_read_configuration_descriptor_header_classifier_names_success_and_failures,
    {
        let descriptor = [9, 2, 34, 0, 1, 1, 0, 0x80, 50];
        let success = decode_xhci_transfer_event([
            0x0000_6000,
            0,
            1u32 << 24,
            (9u32 << 24) | (1u32 << 16) | (32u32 << 10) | 1,
        ]);
        match classify_read_configuration_descriptor_header_transfer_event(
            0x6000, 9, 1, descriptor, success,
        ) {
            XhciReadConfigurationDescriptorHeaderStatus::ConfigurationDescriptorHeaderReady {
                slot_id,
                total_length,
                num_interfaces,
                configuration_value,
            } => {
                testrt::check_eq(slot_id, 9u8);
                testrt::check_eq(total_length, 34u16);
                testrt::check_eq(num_interfaces, 1u8);
                testrt::check_eq(configuration_value, 1u8);
            }
            _ => {
                testrt::check(false, "successful transfer exposes Configuration Descriptor header")
            }
        }

        let invalid = [9, 2, 8, 0, 1, 1, 0, 0x80, 50];
        match classify_read_configuration_descriptor_header_transfer_event(
            0x6000, 9, 1, invalid, success,
        ) {
            XhciReadConfigurationDescriptorHeaderStatus::InvalidConfigurationDescriptorHeader {
                length,
                descriptor_type,
                total_length,
            } => {
                testrt::check_eq(length, 9u8);
                testrt::check_eq(descriptor_type, 2u8);
                testrt::check_eq(total_length, 8u16);
            }
            _ => testrt::check(false, "malformed configuration header is reported"),
        }

        let failed = decode_xhci_transfer_event([
            0x0000_6000,
            0,
            (13u32 << 24) | 3,
            (9u32 << 24) | (1u32 << 16) | (32u32 << 10) | 1,
        ]);
        match classify_read_configuration_descriptor_header_transfer_event(
            0x6000, 9, 1, descriptor, failed,
        ) {
            XhciReadConfigurationDescriptorHeaderStatus::TransferFailed {
                completion_code,
                residual_length,
                slot_id,
                endpoint_id,
            } => {
                testrt::check_eq(completion_code, 13u8);
                testrt::check_eq(residual_length, 3u32);
                testrt::check_eq(slot_id, 9u8);
                testrt::check_eq(endpoint_id, 1u8);
            }
            _ => testrt::check(false, "failed transfer is reported"),
        }
    }
);

arch_test!(usb_configuration_descriptor_tree_finds_boot_keyboard_endpoint, {
    let raw = [
        9, 2, 34, 0, 1, 1, 0, 0x80, 50, 9, 4, 0, 0, 1, 3, 1, 1, 0, 9, 0x21, 0x11, 0x01, 0, 1, 0x22,
        63, 0, 7, 5, 0x81, 3, 8, 0, 10,
    ];
    let mut descriptor = [0u8; USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES];
    let mut index = 0usize;
    while index < raw.len() {
        descriptor[index] = raw[index];
        index += 1;
    }

    let tree = parse_usb_configuration_descriptor_tree(descriptor, raw.len() as u16)
        .unwrap_or_else(|| panic!("valid HID boot keyboard descriptor tree"));

    testrt::check_eq(tree.header.total_length, 34u16);
    testrt::check_eq(tree.header.num_interfaces, 1u8);
    testrt::check_eq(tree.descriptor_count, 4u8);
    let keyboard = tree
        .boot_keyboard
        .unwrap_or_else(|| panic!("boot keyboard interface present"));
    testrt::check_eq(keyboard.interface_number, 0u8);
    testrt::check_eq(keyboard.alternate_setting, 0u8);
    testrt::check_eq(keyboard.endpoint_count, 1u8);
    let endpoint = keyboard
        .interrupt_in_endpoint
        .unwrap_or_else(|| panic!("interrupt-IN endpoint present"));
    testrt::check_eq(endpoint.address, 0x81u8);
    testrt::check_eq(endpoint.endpoint_number, 1u8);
    testrt::check(endpoint.direction_in, "endpoint direction is IN");
    testrt::check_eq(endpoint.transfer_type, 3u8);
    testrt::check_eq(endpoint.max_packet_size, 8u16);
    testrt::check_eq(endpoint.interval, 10u8);

    descriptor[9] = 0;
    testrt::check(
        parse_usb_configuration_descriptor_tree(descriptor, raw.len() as u16).is_none(),
        "zero-length child descriptor is rejected",
    );
});

arch_test!(xhci_read_configuration_descriptor_classifier_names_success_and_failures, {
    let raw = [
        9, 2, 34, 0, 1, 1, 0, 0x80, 50, 9, 4, 0, 0, 1, 3, 1, 1, 0, 9, 0x21, 0x11, 0x01, 0, 1, 0x22,
        63, 0, 7, 5, 0x81, 3, 8, 0, 10,
    ];
    let mut descriptor = [0u8; USB_CONFIGURATION_DESCRIPTOR_MAX_BYTES];
    let mut index = 0usize;
    while index < raw.len() {
        descriptor[index] = raw[index];
        index += 1;
    }

    let success = decode_xhci_transfer_event([
        0x0000_7000,
        0,
        1u32 << 24,
        (10u32 << 24) | (1u32 << 16) | (32u32 << 10) | 1,
    ]);
    match classify_read_configuration_descriptor_transfer_event(
        0x7000,
        10,
        1,
        descriptor,
        raw.len() as u16,
        success,
    ) {
        XhciReadConfigurationDescriptorStatus::ConfigurationDescriptorReady {
            slot_id,
            total_length,
            num_interfaces,
            boot_keyboard_ready_to_configure,
        } => {
            testrt::check_eq(slot_id, 10u8);
            testrt::check_eq(total_length, 34u16);
            testrt::check_eq(num_interfaces, 1u8);
            testrt::check(
                boot_keyboard_ready_to_configure,
                "boot keyboard interface has interrupt-IN endpoint",
            );
        }
        _ => testrt::check(false, "successful transfer exposes Configuration descriptor tree"),
    }

    let failed = decode_xhci_transfer_event([
        0x0000_7000,
        0,
        (13u32 << 24) | 4,
        (10u32 << 24) | (1u32 << 16) | (32u32 << 10) | 1,
    ]);
    match classify_read_configuration_descriptor_transfer_event(
        0x7000,
        10,
        1,
        descriptor,
        raw.len() as u16,
        failed,
    ) {
        XhciReadConfigurationDescriptorStatus::TransferFailed {
            completion_code,
            residual_length,
            slot_id,
            endpoint_id,
        } => {
            testrt::check_eq(completion_code, 13u8);
            testrt::check_eq(residual_length, 4u32);
            testrt::check_eq(slot_id, 10u8);
            testrt::check_eq(endpoint_id, 1u8);
        }
        _ => testrt::check(false, "failed transfer is reported"),
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
    testrt::check_eq(plan.command_ring_trbs, 64u16);
    testrt::check_eq(plan.command_ring_control, plan.command_ring | 1);
    testrt::check_eq(plan.event_ring_dequeue_pointer, plan.event_ring);
    testrt::check_eq(plan.dcbaa & 0x3f, 0u64);
    testrt::check_eq(plan.command_ring & 0x3f, 0u64);
    testrt::check_eq(plan.event_ring & 0x3f, 0u64);
    testrt::check_eq(plan.event_ring_segment_table & 0x3f, 0u64);
    testrt::check_eq(plan.scratchpad_array & 0x3f, 0u64);
    testrt::check_eq(plan.input_context & 0xfff, 0u64);
    testrt::check_eq(plan.output_device_context & 0xfff, 0u64);
    testrt::check_eq(plan.control_endpoint_ring & 0x3f, 0u64);
    testrt::check_eq(XHCI_CONTROL_ENDPOINT_RING_TRBS, 64usize);

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
