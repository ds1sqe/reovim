//! Tests for BCM2711 PCIe raw discovery helpers.

use {
    super::{
        BCM2711_PCIE_BUS_BASE, BCM2711_PCIE_MEM_BUS_BASE, BCM2711_PCIE_MEM_CPU_BASE,
        BCM2711_PCIE_MEM_LEN, BCM2711_PCIE_MMIO_BASE, BCM2711_PCIE_MMIO_LEN, PciConfigHeader,
        PciLocation, decode_pcie_link_status, pci_memory_bar_base, pcie_bus_memory_to_cpu,
        pcie_external_config_index,
    },
    reovim_testrt::{self as testrt, arch_test},
};

arch_test!(bcm2711_pcie_addresses_match_firmware_dtb_translation, {
    testrt::check_eq(BCM2711_PCIE_BUS_BASE, 0x7d50_0000u64);
    testrt::check_eq(BCM2711_PCIE_MMIO_BASE, 0xfd50_0000usize);
    testrt::check_eq(BCM2711_PCIE_MMIO_LEN, 0x9310usize);
    testrt::check_eq(BCM2711_PCIE_MEM_BUS_BASE, 0xc000_0000u64);
    testrt::check_eq(BCM2711_PCIE_MEM_CPU_BASE, 0x6_0000_0000u64);
    testrt::check_eq(BCM2711_PCIE_MEM_LEN, 0x4000_0000u64);
});

arch_test!(pcie_status_decode_reports_link_and_mode_bits, {
    let status = decode_pcie_link_status(0x80 | 0x20 | 0x10, 0x0303);

    testrt::check(status.root_complex_mode, "root-complex bit is set");
    testrt::check(status.phy_link_up, "physical link bit is set");
    testrt::check(status.data_link_active, "data link bit is set");
    testrt::check(status.link_up(), "link_up requires phy and data link");
    testrt::check(!status.link_in_l23, "L23 bit is clear");
    testrt::check_eq(status.revision, 0x0303u32);

    let link_down = decode_pcie_link_status(0x80 | 0x10, 0x0303);
    testrt::check(!link_down.link_up(), "one link bit is not enough");
});

arch_test!(pcie_external_config_index_uses_ecam_bus_and_devfn_shape, {
    let location = PciLocation::new(1, 2, 3).unwrap_or_else(|| panic!("valid pci location"));

    testrt::check_eq(location.bus, 1u8);
    testrt::check_eq(location.device, 2u8);
    testrt::check_eq(location.function, 3u8);
    testrt::check_eq(pcie_external_config_index(location), 0x0011_3000u32);
    testrt::check(PciLocation::new(0, 32, 0).is_none(), "device numbers stop before 32");
    testrt::check(PciLocation::new(0, 0, 8).is_none(), "function numbers stop before 8");
});

arch_test!(pci_memory_bars_translate_through_pcie_window, {
    testrt::check_eq(pci_memory_bar_base(0xc000_1000, 0), Some(0xc000_1000u64));
    testrt::check_eq(pcie_bus_memory_to_cpu(0xc000_1000), Some(0x6_0000_1000usize));
    testrt::check_eq(pci_memory_bar_base(0xc000_1004, 0x0000_0006), Some(0x0000_0006_c000_1000u64));
    testrt::check_eq(pci_memory_bar_base(0xc000_1001, 0), None);
    testrt::check_eq(pcie_bus_memory_to_cpu(0xbfff_ffff), None);
    testrt::check_eq(pcie_bus_memory_to_cpu(0x1_0000_0000), None);
});

arch_test!(pci_header_identifies_xhci_and_bar_location, {
    let header = PciConfigHeader {
        location: PciLocation::new(1, 0, 0).unwrap_or_else(|| panic!("valid pci location")),
        vendor_id: 0x1106,
        device_id: 0x3483,
        command: 0,
        status: 0,
        revision_id: 1,
        prog_if: 0x30,
        subclass: 0x03,
        class_code: 0x0c,
        bar0: 0xc001_0000,
        bar1: 0,
    };

    testrt::check(header.is_xhci(), "class/subclass/prog-if names xHCI");
    testrt::check_eq(header.bar0_bus_memory_base(), Some(0xc001_0000u64));
    testrt::check_eq(header.bar0_cpu_memory_base(), Some(0x6_0001_0000usize));

    let not_xhci = PciConfigHeader {
        prog_if: 0x20,
        ..header
    };
    testrt::check(!not_xhci.is_xhci(), "EHCI prog-if is not xHCI");
});
