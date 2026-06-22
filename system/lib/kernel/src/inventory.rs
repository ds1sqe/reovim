//! Device-inventory shaping: parses a caller-supplied firmware DTB and
//! enumerates devices into caller-supplied static storage.

use crate::fdt;

pub use {
    fdt::enumerate::{
        DeviceClassifier, EMPTY_DEVICE_ENTRY, MAX_DEVICE_ENTRIES as DEVICE_STORAGE_ENTRIES,
    },
    reovim_kabi_platform::{DeviceClass, DeviceEntry},
};

/// Maximum DTB size this bridge accepts from the composition root.
pub const MAX_DTB_BYTES: usize = 1 << 20; // 1 MiB

/// Enumerates a caller-supplied firmware DTB into a one-shot static
/// [`reovim_kabi_platform::DeviceInventory`] pushed into the kernel at entry.
/// The caller also supplies the board/chip-specific compatible-string
/// classifier; this bridge only walks and shapes the neutral inventory.
///
/// Returns the empty default when:
/// - no DTB was passed,
/// - the slice is undersized or oversized, or
/// - the FDT parse fails.
///
/// The returned inventory is backed by `storage`, which must live for the
/// process lifetime.
#[must_use]
pub fn collect_device_inventory(
    dtb: &'static [u8],
    storage: &'static mut [DeviceEntry],
    classify: DeviceClassifier,
) -> reovim_kabi_platform::DeviceInventory {
    if !(40..=MAX_DTB_BYTES).contains(&dtb.len()) {
        return reovim_kabi_platform::DeviceInventory::default();
    }
    let Ok(fdt) = fdt::reader::Fdt::parse(dtb) else {
        return reovim_kabi_platform::DeviceInventory::default();
    };
    fdt::enumerate::enumerate_into(&fdt, storage, classify)
}

// Bridge-level smoke: declared here so `super::` reaches
// `collect_device_inventory`; reaches the test runtime through the
// reovim-testrt leaf.
#[cfg(feature = "selftest")]
#[path = "inventory_smoke_tests.rs"]
mod inventory_smoke_tests;
