//! Device-tree node enumerator: walks the DTB and produces a [`DeviceInventory`].
//!
//! The enumerator performs a shallow two-level walk of the FDT — root children
//! and their children — the common layout where peripherals live directly under
//! `/` or under `/soc`. It builds a [`DeviceEntry`] for every node whose
//! `compatible` list maps to a non-[`Unknown`] class through the caller-supplied
//! classifier and stores the entries in a `&'static [DeviceEntry]` backed by
//! caller-provided static storage.
//!
//! This is one-shot *static* discovery. The enumerator is called once at boot
//! and the result is pushed into the kernel as part of [`DeviceInventory`]; no
//! re-enumeration or mutation occurs after that point.
//!
//! [`Unknown`]: reovim_kabi_platform::DeviceClass::Unknown

use reovim_kabi_platform::{DeviceClass, DeviceEntry, DeviceInventory};

use super::reader::{Fdt, Node};

/// Maps one DTB `compatible` string into the coarse KABI device class.
///
/// Board/chip-specific compatible-string tables live below this bridge and are
/// passed in by the composition root. Returning [`DeviceClass::Unknown`] keeps
/// the node out of the resulting inventory.
pub type DeviceClassifier = fn(&str) -> DeviceClass;

/// Maximum number of device entries the enumerator will collect.
///
/// A conservative cap for early static discovery. The caller supplies storage,
/// so targets that need fewer entries can hand in a smaller slice; entries past
/// this bridge cap are ignored.
pub const MAX_DEVICE_ENTRIES: usize = 32;

/// Empty `DeviceEntry` value for static storage initialization.
pub const EMPTY_DEVICE_ENTRY: DeviceEntry = DeviceEntry {
    class: DeviceClass::Unknown,
    mmio_base: 0,
    mmio_len: 0,
    irq: u32::MAX,
    capacity_bytes: 0,
    compatible: "",
};

/// Classifies a node by its full `compatible` list: the first non-[`Unknown`]
/// class among the NUL-separated compatible strings.
///
/// DTBs list compatibles most-specific-first, and a classifier may intentionally
/// match a later generic string rather than the first board-specific string, so
/// every string is checked, not just [`Node::first_compatible`].
fn classify_node(node: &Node, classify: DeviceClassifier) -> DeviceClass {
    let Some(bytes) = node.prop_bytes("compatible") else {
        return DeviceClass::Unknown;
    };
    for s in bytes.split(|&b| b == 0) {
        if let Ok(text) = core::str::from_utf8(s) {
            let class = classify(text);
            if !matches!(class, DeviceClass::Unknown) {
                return class;
            }
        }
    }
    DeviceClass::Unknown
}

/// Attempts to build a [`DeviceEntry`] from a node. Returns `None` when the
/// node has no recognisable compatible string (class == Unknown).
fn entry_from_node(node: &Node<'static>, classify: DeviceClassifier) -> Option<DeviceEntry> {
    let class = classify_node(node, classify);
    if matches!(class, DeviceClass::Unknown) {
        return None;
    }
    let compatible = node.first_compatible().unwrap_or("");
    let (mmio_base, mmio_len) = node.reg().unwrap_or((0, 0));
    let irq = node.first_interrupt_cell().unwrap_or(u32::MAX);
    Some(DeviceEntry {
        class,
        mmio_base,
        mmio_len,
        irq,
        capacity_bytes: 0,
        compatible,
    })
}

/// Walks the firmware device tree and assembles a static [`DeviceInventory`].
///
/// Performs a shallow two-level walk (root → children, and each child's
/// children). Every node whose `compatible` property classifies to a
/// non-[`Unknown`] class contributes one [`DeviceEntry`]. Entries beyond
/// [`MAX_DEVICE_ENTRIES`] or the caller-provided storage length are silently
/// dropped.
///
/// The returned slice is backed by caller-provided process-lifetime storage.
/// If no entries were collected, the empty default is returned.
///
/// `fdt` is borrowed as `&Fdt<'static>` so the `&'static str` slices produced
/// from property bytes carry an honest `'static` lifetime (they point into the
/// DTB byte slice, which lives in identity-mapped RAM for the process lifetime).
pub fn enumerate_into(
    fdt: &Fdt<'static>,
    storage: &'static mut [DeviceEntry],
    classify: DeviceClassifier,
) -> DeviceInventory {
    let mut count = 0usize;
    let cap = storage.len().min(MAX_DEVICE_ENTRIES);

    let Ok(root) = fdt.root() else {
        return DeviceInventory::default();
    };

    // First level: direct children of root.
    for child in root.children() {
        if count >= cap {
            break;
        }
        if let Some(entry) = entry_from_node(&child, classify) {
            storage[count] = entry;
            count += 1;
        }

        // Second level: children of this root-level child (e.g. /soc/* devices).
        for grandchild in child.children() {
            if count >= cap {
                break;
            }
            if let Some(entry) = entry_from_node(&grandchild, classify) {
                storage[count] = entry;
                count += 1;
            }
        }
    }

    if count == 0 {
        return DeviceInventory::default();
    }

    DeviceInventory {
        devices: &storage[..count],
    }
}

// L12 layout: tests live in the sibling file `enumerate_tests.rs`, declared as
// a `#[path]` child so `super::` reaches private helpers. The file is only
// compiled when this module is (the lib.rs target gate), so the inner
// declaration needs only the `selftest` feature gate.
#[cfg(feature = "selftest")]
#[path = "enumerate_tests.rs"]
mod tests;
