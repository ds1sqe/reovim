//! Device-tree node enumerator: walks the DTB and produces a [`DeviceInventory`].
//!
//! The enumerator performs a shallow two-level walk of the FDT — root children
//! and their children — which is sufficient for the `BCM2711` (and most SoCs):
//! peripherals live directly under `/` or under `/soc`. It builds a
//! [`DeviceEntry`] for every node whose first `compatible` string maps to a
//! non-[`Unknown`] class and stores the entries in a `&'static [DeviceEntry]`
//! backed by the boot arena.
//!
//! This is one-shot *static* discovery. The enumerator is called once at boot
//! and the result is pushed into the kernel as part of [`DeviceInventory`]; no
//! re-enumeration or mutation occurs after that point.
//!
//! [`Unknown`]: reovim_kabi_platform::DeviceClass::Unknown

use core::mem::size_of;

use reovim_kabi_platform::{DeviceClass, DeviceEntry, DeviceInventory};

use reovim_arch_sys_none_aarch64 as backend;

use super::reader::{Fdt, Node};

/// Maximum number of device entries the enumerator will collect.
///
/// Sized for the `BCM2711` `SoC` — `BCM2711` has well under 32 uniquely-classified
/// devices at the two levels this walk covers. Phase 3 may raise the cap when
/// richer enumeration is required.
const MAX_DEVICE_ENTRIES: usize = 32;

/// Maps the first `compatible` string of a DTB node to a [`DeviceClass`].
///
/// The mapping is based on well-known compatible strings for the `BCM2711` and
/// ARM common devices. Strings not listed here map to [`DeviceClass::Unknown`].
///
/// The EMMC2 block controller and the (DWC2) USB host controller are
/// enumerated, not driven: 04 records their presence and MMIO window; reading a
/// block's capacity or walking USB descriptors is later driver work.
pub fn classify(compatible: &str) -> DeviceClass {
    match compatible {
        "arm,pl011" => DeviceClass::Uart,
        "arm,gic-400" => DeviceClass::Interrupt,
        "brcm,bcm2835-mbox" => DeviceClass::Mailbox,
        "brcm,bcm2711-emmc2" => DeviceClass::Block,
        "brcm,bcm2708-usb" => DeviceClass::Usb,
        _ => DeviceClass::Unknown,
    }
}

/// Classifies a node by its full `compatible` list: the first non-[`Unknown`]
/// class among the NUL-separated compatible strings.
///
/// DTBs list compatibles most-specific-first, and the generic string we match
/// on may not be first — the real Pi 4 UART leads with `"arm,pl011-axi"` and
/// only carries `"arm,pl011"` as its second entry — so every string is checked,
/// not just [`Node::first_compatible`].
fn classify_node(node: &Node) -> DeviceClass {
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
fn entry_from_node(node: &Node<'static>) -> Option<DeviceEntry> {
    let class = classify_node(node);
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
/// [`MAX_DEVICE_ENTRIES`] are silently dropped (the cap is generous enough
/// for the expected `BCM2711` inventory).
///
/// The returned slice is backed by the boot arena (process lifetime, no
/// reallocation, no aliasing). If the arena is exhausted or no entries were
/// collected, the empty default is returned.
///
/// `fdt` is borrowed as `&Fdt<'static>` so the `&'static str` slices produced
/// from property bytes carry an honest `'static` lifetime (they point into the
/// DTB byte slice, which lives in identity-mapped RAM for the process lifetime).
pub fn enumerate(fdt: &Fdt<'static>) -> DeviceInventory {
    let mut buf = [DeviceEntry {
        class: DeviceClass::Unknown,
        mmio_base: 0,
        mmio_len: 0,
        irq: u32::MAX,
        capacity_bytes: 0,
        compatible: "",
    }; MAX_DEVICE_ENTRIES];
    let mut count = 0usize;

    let Ok(root) = fdt.root() else {
        return DeviceInventory::default();
    };

    // First level: direct children of root.
    for child in root.children() {
        if count >= MAX_DEVICE_ENTRIES {
            break;
        }
        if let Some(entry) = entry_from_node(&child) {
            buf[count] = entry;
            count += 1;
        }

        // Second level: children of this root-level child (e.g. /soc/* devices).
        for grandchild in child.children() {
            if count >= MAX_DEVICE_ENTRIES {
                break;
            }
            if let Some(entry) = entry_from_node(&grandchild) {
                buf[count] = entry;
                count += 1;
            }
        }
    }

    if count == 0 {
        return DeviceInventory::default();
    }

    // Allocate arena storage (the floor's static boot arena, reached through the
    // arch-sys-none-aarch64 accessor — the arena mechanism stays below) and copy
    // the entries into it.
    let alloc_size = count * size_of::<DeviceEntry>();
    let Ok(addr) = backend::arena_alloc_pages(alloc_size) else {
        return DeviceInventory::default();
    };
    let ptr = addr as *mut DeviceEntry;
    // SAFETY: `addr` is a page-aligned hand-out from the static `ARENA`, whose
    // lifetime equals the process — the monotonic bump allocator never frees or
    // relocates it, so the `'static` lifetime on the returned slice is honest.
    // Page alignment (4096) exceeds `align_of::<DeviceEntry>()`, and the
    // hand-out reserved a whole page (rounded up), so writing `count` elements
    // and reading them back as a slice both stay within owned storage the bump
    // never hands out twice (no aliasing).
    unsafe {
        core::ptr::copy_nonoverlapping(buf.as_ptr(), ptr, count);
        DeviceInventory {
            devices: core::slice::from_raw_parts(ptr, count),
        }
    }
}

// L12 layout: tests live in the sibling file `enumerate_tests.rs`, declared as
// a `#[path]` child so `super::` reaches the private `classify` function. The
// file is only compiled when this module is (the lib.rs target gate), so the
// inner declaration needs only the `selftest` feature gate.
#[cfg(feature = "selftest")]
#[path = "enumerate_tests.rs"]
mod tests;
