//! Bridge-level smoke for the device-inventory input validation.
//!
//! L12 layout: declared in `inventory.rs` via
//! `#[cfg(feature = "selftest")] #[path = "inventory_smoke_tests.rs"] mod ...;`,
//! so `super::` reaches `collect_device_inventory`.
//!
//! Raw DTB capture is tested by the composition root below this bridge; this
//! file keeps the bridge honest about malformed/no-DTB inputs.

use {
    super::{DeviceClass, EMPTY_DEVICE_ENTRY, collect_device_inventory},
    core::cell::UnsafeCell,
    reovim_testrt::{self as testrt, arch_test},
};

struct DeviceStore(UnsafeCell<[super::DeviceEntry; 1]>);

// SAFETY: the selftest runner is single-threaded and each test takes the store
// only for the duration of the call.
unsafe impl Sync for DeviceStore {}

static DEVICES: DeviceStore = DeviceStore(UnsafeCell::new([EMPTY_DEVICE_ENTRY; 1]));

fn storage() -> &'static mut [super::DeviceEntry] {
    // SAFETY: single-threaded selftest runner; no stored reference escapes
    // beyond the inventory value checked by this test.
    unsafe { &mut *DEVICES.0.get() }
}

fn classify_unknown(_: &str) -> DeviceClass {
    DeviceClass::Unknown
}

arch_test!(device_inventory_empty_without_dtb, {
    let inventory = collect_device_inventory(&[], storage(), classify_unknown);
    testrt::check(inventory.devices.is_empty(), "empty DTB yields empty inventory");
});
