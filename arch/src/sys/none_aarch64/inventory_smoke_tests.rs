//! On-target integration smoke for the device enumerator (04 Phase 2/3 ACs).
//!
//! L12 layout: declared in `mod.rs` via
//! `#[cfg(feature = "selftest")] #[path = "inventory_smoke_tests.rs"] mod ...;`,
//! so `super::` reaches `collect_device_inventory`.
//!
//! Unlike the pure `classify`/reader cases (synthetic blobs), this runs on the
//! real machine: in the aarch64 `arch-selftest` QEMU `raspi4b` pilot the
//! firmware DTB is supplied via `-dtb`, `_start` captures its address, and
//! `collect_device_inventory` walks it. It proves the asm-capture → reader →
//! enumerate path end-to-end on real firmware bytes, not a hand-built fixture.

use {
    super::collect_device_inventory,
    crate::{arch_test, testrt},
    reovim_kabi_platform::DeviceClass,
};

arch_test!(device_inventory_enumerates_real_dtb, {
    let inventory = collect_device_inventory();
    let devices = inventory.devices;
    testrt::check(!devices.is_empty(), "real DTB yields a non-empty inventory");

    // PL011 UART at the known BCM2711 bus address. Its compatible list leads
    // with "arm,pl011-axi" and only carries "arm,pl011" second, so finding it
    // also proves the enumerator matches against the whole compatible list.
    let uart = devices.iter().find(|d| d.class == DeviceClass::Uart);
    testrt::check(uart.is_some(), "inventory contains a PL011 UART");
    if let Some(u) = uart {
        testrt::check_eq(u.mmio_base, 0x7e20_1000u64);
    }

    // Phase 3: the EMMC2 block controller and the USB host controller, each
    // with a reg-derived non-zero MMIO base (enumerated, not driven).
    let block = devices.iter().find(|d| d.class == DeviceClass::Block);
    testrt::check(block.is_some(), "inventory contains an EMMC2 block device");
    testrt::check(block.is_some_and(|b| b.mmio_base != 0), "block device has a non-zero mmio_base");

    let usb = devices.iter().find(|d| d.class == DeviceClass::Usb);
    testrt::check(usb.is_some(), "inventory contains a USB host controller");
    testrt::check(usb.is_some_and(|u| u.mmio_base != 0), "usb host has a non-zero mmio_base");
});
