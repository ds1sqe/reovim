//! Device-inventory assembly: reads the firmware DTB and enumerates its devices,
//! lifted out of `arch-sys-none-aarch64` (SP04 04a).
//!
//! The DTB physical address is a raw boot fact captured by the floor `_start`
//! asm into the arch-sys-none-aarch64 `DTB_PTR` static; this module reads it
//! through the [`backend::dtb_ptr`] accessor (NOT by naming the static across
//! the upward impl edge), bounds the firmware blob, parses it with the migrated
//! [`crate::fdt`] reader, and enumerates the device tree into a neutral
//! [`reovim_kabi_platform::DeviceInventory`]. The asm capture stays below
//! (invariant #4); the parse + enumerate are device-neutral and live here.

use reovim_arch_sys_none_aarch64 as backend;

use crate::fdt;

/// Reads the firmware DTB captured at boot and enumerates its devices into a
/// one-shot static [`reovim_kabi_platform::DeviceInventory`] pushed into the
/// kernel at entry.
///
/// Returns the empty default when:
/// - no DTB was passed (e.g. QEMU without `-dtb`),
/// - the pointer is zero or the FDT header is unreadable / oversized, or
/// - the FDT parse fails.
///
/// The returned inventory is backed by the boot arena and valid for the
/// process lifetime.
#[must_use]
pub fn collect_device_inventory() -> reovim_kabi_platform::DeviceInventory {
    const MAX_DTB_BYTES: usize = 1 << 20; // 1 MiB
    let p = backend::dtb_ptr();
    if p == 0 {
        return reovim_kabi_platform::DeviceInventory::default();
    }
    let ptr = p as *const u8;
    // Read totalsize from the FDT header (big-endian u32 at byte offset 4) to
    // bound the slice before handing it to the parser.
    // SAFETY: the firmware DTB lives in identity-mapped RAM for the whole
    // process; the 8-byte header read is within it.
    let total = unsafe {
        let hdr = core::slice::from_raw_parts(ptr, 8);
        u32::from_be_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize
    };
    // Reject absurd sizes before forming the full slice. A valid FDT header is
    // at least 40 bytes; we cap well above any real DTB but within mapped RAM.
    if !(40..=MAX_DTB_BYTES).contains(&total) {
        return reovim_kabi_platform::DeviceInventory::default();
    }
    // SAFETY: as above — `total` bytes from the DTB base are within the
    // process-lifetime, identity-mapped firmware region. The `'static` lifetime
    // is honest: the firmware RAM is never unmapped and we never write through
    // this pointer. The enumerator's `compatible: &'static str` slices point
    // into this same byte range and inherit the same lifetime guarantee.
    let dtb: &'static [u8] = unsafe { core::slice::from_raw_parts(ptr, total) };
    let Ok(fdt) = fdt::reader::Fdt::parse(dtb) else {
        return reovim_kabi_platform::DeviceInventory::default();
    };
    fdt::enumerate::enumerate(&fdt)
}

// On-target device-inventory smoke (04 Phase 2/3 ACs): declared here so
// `super::` reaches `collect_device_inventory`. Runs on the real QEMU raspi4b
// machine with a `-dtb` blob, exercising the asm-capture → reader → enumerate
// path end-to-end; reaches the test runtime through the reovim-testrt leaf.
#[cfg(feature = "selftest")]
#[path = "inventory_smoke_tests.rs"]
mod inventory_smoke_tests;
