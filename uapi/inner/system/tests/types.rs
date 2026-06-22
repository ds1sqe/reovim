//! Behavioral tests for `uapi/system` data wrappers.

use reovim_uapi_system::{BootInfo, DeviceClass, DeviceEntry, DeviceInventory, MemorySummary};

#[test]
fn memory_summary_reports_count_and_usable_bytes() {
    let summary = MemorySummary::new(2, 4096);
    assert_eq!(summary.range_count(), 2);
    assert_eq!(summary.usable_bytes(), 4096);
    assert!(!summary.is_empty());
}

#[test]
fn boot_info_default_is_empty_diagnostic_view() {
    let info = BootInfo::default();
    assert!(info.memory.is_empty());
    assert_eq!(info.cpu_count, 0);
}

#[test]
fn device_inventory_carries_static_entries() {
    static DEVICES: [DeviceEntry; 1] = [DeviceEntry {
        class: DeviceClass::Uart,
        mmio_base: 0x1000,
        mmio_len: 0x100,
        irq: 1,
        capacity_bytes: 0,
        compatible: "arm,pl011",
    }];

    let inventory = DeviceInventory { devices: &DEVICES };
    assert_eq!(inventory.devices[0].class, DeviceClass::Uart);
    assert_eq!(inventory.devices[0].compatible, "arm,pl011");
}
