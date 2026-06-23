//! Tests for DTB enumeration with caller-supplied compatible-string
//! classification.
//!
//! L12 layout: declared in `enumerate.rs` via
//! `#[cfg(feature = "selftest")] #[path = "enumerate_tests.rs"] mod tests;`,
//! so `super::` reaches the private enumerator helpers.
//!
//! These tests cover the bridge behavior only: system-kernel walks the DTB and
//! applies a classifier supplied from below. Board/chip-specific compatible
//! tables do not live in this crate.

use {
    super::{EMPTY_DEVICE_ENTRY, enumerate_into},
    core::cell::UnsafeCell,
    reovim_kabi_platform::DeviceClass,
    reovim_testrt::{self as testrt, arch_test},
    reovim_uapi_system::{DeviceClass as UapiDeviceClass, DeviceEntry},
};

use super::super::reader::Fdt;

static FIXTURE: &[u8] = include_bytes!("testdata/reovim-bcm2711.dtb");

struct DeviceStore(UnsafeCell<[DeviceEntry; 8]>);

// SAFETY: the selftest runner is single-threaded and each test takes the store
// only for the duration of the call.
unsafe impl Sync for DeviceStore {}

static DEVICES: DeviceStore = DeviceStore(UnsafeCell::new([EMPTY_DEVICE_ENTRY; 8]));
static UNKNOWN_DEVICES: DeviceStore = DeviceStore(UnsafeCell::new([EMPTY_DEVICE_ENTRY; 8]));

fn storage() -> &'static mut [DeviceEntry] {
    // SAFETY: single-threaded selftest runner; no stored reference escapes
    // beyond the inventory value checked by this test.
    unsafe { &mut *DEVICES.0.get() }
}

fn unknown_storage() -> &'static mut [DeviceEntry] {
    // SAFETY: same single-threaded selftest runner constraint as `storage`.
    unsafe { &mut *UNKNOWN_DEVICES.0.get() }
}

fn classify_test_compatible(compatible: &str) -> DeviceClass {
    match compatible {
        "arm,primecell" => DeviceClass::Uart,
        _ => DeviceClass::Unknown,
    }
}

fn classify_unknown(_: &str) -> DeviceClass {
    DeviceClass::Unknown
}

fn classify_disabled_xhci(compatible: &str) -> DeviceClass {
    match compatible {
        "generic-xhci" => DeviceClass::Usb,
        _ => DeviceClass::Unknown,
    }
}

arch_test!(enumerate_uses_caller_supplied_classifier, {
    let fdt = Fdt::parse(FIXTURE).unwrap_or_else(|e| panic!("parse: {e:?}"));
    let inventory = enumerate_into(&fdt, storage(), classify_test_compatible);

    let mut has_uart = false;
    for device in inventory.devices {
        has_uart |= device.class == UapiDeviceClass::Uart;
    }

    testrt::check_eq(inventory.devices.len(), 1usize);
    testrt::check(has_uart, "classifier selected UART from compatible list");
});

arch_test!(enumerate_drops_nodes_classified_unknown, {
    let fdt = Fdt::parse(FIXTURE).unwrap_or_else(|e| panic!("parse: {e:?}"));
    let inventory = enumerate_into(&fdt, unknown_storage(), classify_unknown);

    testrt::check(inventory.devices.is_empty(), "unknown-only classifier yields empty inventory");
});

arch_test!(enumerate_skips_disabled_nodes, {
    let fdt = Fdt::parse(FIXTURE).unwrap_or_else(|e| panic!("parse: {e:?}"));
    let inventory = enumerate_into(&fdt, unknown_storage(), classify_disabled_xhci);

    testrt::check(inventory.devices.is_empty(), "disabled nodes are not present devices");
});
