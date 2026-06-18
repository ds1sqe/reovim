//! Runtime hardware discovery for the `x86_64` bare-metal floor.
//!
//! Like the aarch64 backend, the kernel is platform-neutral and learns the
//! machine's facts only from the [`BootInfo`] the floor hands it at entry. x86
//! differs from aarch64 in *where* the memory map comes from: the Multiboot1
//! bootloader builds an information structure and leaves its pointer in `EBX`,
//! which `_start` stashes into [`crate::start::MULTIBOOT_INFO_PTR`]. This module
//! reads that pointer, parses the Multiboot1 mmap into an arena-backed
//! `&'static [MemoryRange]`, and reads the CPU id/clock from `CPUID`/`RDTSC`.
//!
//! Nothing here is compiled in or assumed: a missing/zero pointer, or an
//! information structure without the mmap flag, degrades to an empty map
//! ("unknown") rather than a wrong number.

use core::sync::atomic::Ordering;

use reovim_kabi_platform::{BootInfo, MemoryKind, MemoryRange};

use super::{arena, timer};

/// Multiboot1 `flags` bit 6: the `mmap_length`/`mmap_addr` fields are valid.
const FLAG_MMAP: u32 = 1 << 6;

/// Byte offsets into the Multiboot1 information structure.
const OFF_FLAGS: usize = 0;
const OFF_MMAP_LENGTH: usize = 44;
const OFF_MMAP_ADDR: usize = 48;

/// Byte offsets into a Multiboot1 mmap entry, relative to its `size` field. The
/// `size` field (offset 0) excludes itself, so the stride to the next entry is
/// `size + 4`.
const ENTRY_OFF_BASE: usize = 4;
const ENTRY_OFF_LEN: usize = 12;
const ENTRY_OFF_TYPE: usize = 20;
/// The bytes needed to read one full mmap entry (`type` field at 20, +4).
const ENTRY_MIN_BYTES: usize = 24;

/// Returns the stashed Multiboot1 info-structure pointer, or 0 when `_start`
/// did not run on a Multiboot entry.
///
/// The storage cell lives in [`crate::start::MULTIBOOT_INFO_PTR`] (beside the
/// other boot statics the `_start` asm touches); this is the single read API.
#[must_use]
fn multiboot_ptr() -> u32 {
    crate::start::MULTIBOOT_INFO_PTR.load(Ordering::Relaxed)
}

/// Maps a Multiboot1 memory-type code to the neutral [`MemoryKind`].
///
/// Multiboot1 types: 1 = available, 3 = ACPI reclaimable, 5 = defective; every
/// other code (2 reserved, 4 preserve-on-hibernation, …) is conservatively
/// reported as reserved.
const fn kind_of(ty: u32) -> MemoryKind {
    match ty {
        1 => MemoryKind::Usable,
        3 => MemoryKind::Acpi,
        5 => MemoryKind::Unusable,
        _ => MemoryKind::Reserved,
    }
}

/// Reads a little-endian `u32` at `p + off`.
///
/// # Safety
/// `p + off .. p + off + 4` must be a readable, in-bounds range.
const unsafe fn rd_u32(p: *const u8, off: usize) -> u32 {
    // SAFETY: the caller guarantees the 4-byte range is readable; an unaligned
    // read is well-defined here.
    unsafe { p.add(off).cast::<u32>().read_unaligned() }
}

/// Reads a little-endian `u64` at `p + off`.
///
/// # Safety
/// `p + off .. p + off + 8` must be a readable, in-bounds range.
const unsafe fn rd_u64(p: *const u8, off: usize) -> u64 {
    // SAFETY: the caller guarantees the 8-byte range is readable; an unaligned
    // read is well-defined here.
    unsafe { p.add(off).cast::<u64>().read_unaligned() }
}

/// Reads `(mmap_addr, mmap_length)` from a Multiboot1 information structure, or
/// `None` when the mmap flag is clear or the region is empty.
///
/// # Safety
/// `info` must point to a readable Multiboot1 information structure (at least
/// `OFF_MMAP_ADDR + 4` bytes).
const unsafe fn mmap_region(info: *const u8) -> Option<(usize, usize)> {
    // SAFETY: the caller guarantees the header is readable.
    let flags = unsafe { rd_u32(info, OFF_FLAGS) };
    if flags & FLAG_MMAP == 0 {
        return None;
    }
    // SAFETY: bit 6 set ⇒ the mmap_length/mmap_addr words are present.
    let len = unsafe { rd_u32(info, OFF_MMAP_LENGTH) } as usize;
    // SAFETY: as above.
    let addr = unsafe { rd_u32(info, OFF_MMAP_ADDR) } as usize;
    if addr == 0 || len == 0 {
        return None;
    }
    Some((addr, len))
}

/// Walks the Multiboot1 mmap entry array in `[addr, addr + len)`, writing each
/// range into `out` (up to its capacity) and returning the total entry count
/// encountered.
///
/// Pure over the `(addr, len)` buffer and `out`, so a synthetic entry array
/// drives it in tests without a bootloader. A zero-size entry (malformed) stops
/// the walk instead of spinning on a stride that never advances. Passing an
/// empty `out` counts the entries without writing — the two-pass sizing path.
///
/// # Safety
/// `[addr, addr + len)` must be a readable byte range.
unsafe fn parse_entries(addr: *const u8, len: usize, out: &mut [MemoryRange]) -> usize {
    let mut offset = 0usize;
    let mut count = 0usize;
    while offset + ENTRY_MIN_BYTES <= len {
        // SAFETY: `offset + ENTRY_MIN_BYTES <= len`, so the 24-byte entry at
        // `addr + offset` is inside the caller's readable range.
        let entry = unsafe { addr.add(offset) };
        // SAFETY: the size field is the first 4 bytes of the in-bounds entry.
        let size = unsafe { rd_u32(entry, 0) } as usize;
        if size == 0 {
            break; // malformed: a zero stride would never advance the walk
        }
        // SAFETY: base/len/type lie within the 24 in-bounds bytes.
        let base = unsafe { rd_u64(entry, ENTRY_OFF_BASE) };
        let length = unsafe { rd_u64(entry, ENTRY_OFF_LEN) };
        let ty = unsafe { rd_u32(entry, ENTRY_OFF_TYPE) };
        if let Some(slot) = out.get_mut(count) {
            *slot = MemoryRange {
                base,
                len: length,
                kind: kind_of(ty),
            };
        }
        count += 1;
        offset += size + 4;
    }
    count
}

/// Parses the firmware memory map into an arena-backed `'static` slice.
///
/// Returns an empty map when no Multiboot pointer was stashed, the mmap flag is
/// clear, the map is empty, or the arena cannot back it — the kernel then
/// reports memory as unknown rather than reporting a wrong range.
fn discover_memory() -> &'static [MemoryRange] {
    let ptr = multiboot_ptr();
    if ptr == 0 {
        return &[];
    }
    let info = ptr as usize as *const u8;
    // SAFETY: the Multiboot info structure sits in low memory, inside the low-1
    // GiB the `_start` identity map covers, so the header is readable here.
    let Some((addr, len)) = (unsafe { mmap_region(info) }) else {
        return &[];
    };
    let entries = addr as *const u8;
    // First pass: count entries to size the arena hand-out.
    // SAFETY: the bootloader's mmap_addr/mmap_length describe a readable entry
    // array (same identity-map precondition as the header above).
    let count = unsafe { parse_entries(entries, len, &mut []) };
    if count == 0 {
        return &[];
    }
    let Ok(out_addr) = arena::alloc_pages(count * core::mem::size_of::<MemoryRange>()) else {
        return &[];
    };
    let out = out_addr as *mut MemoryRange;
    // SAFETY: `out_addr` is a page-aligned hand-out from the static `ARENA`,
    // reserving `count` whole `MemoryRange`s. Page alignment (4096) exceeds
    // `align_of::<MemoryRange>()`, and the monotonic bump never hands the same
    // bytes out twice, so the `'static` slice is honest and unaliased. The
    // second walk writes exactly the `count` slots the first pass counted.
    unsafe {
        let slots = core::slice::from_raw_parts_mut(out, count);
        let written = parse_entries(entries, len, slots);
        core::slice::from_raw_parts(out.cast_const(), written)
    }
}

/// Discovers the machine's hardware facts and assembles them into a [`BootInfo`]
/// for `Init::new`.
///
/// RAM comes from the Multiboot1 mmap, the CPU clock from the TSC frequency
/// probe, and the CPU id from `CPUID`. The floor parks every secondary core at
/// boot, so the CPU count is the single-core constant `1`.
#[must_use]
pub fn collect_boot_info() -> BootInfo {
    // CPUID leaf 1 eax is the processor version information (family / model /
    // stepping / type) — an identifying value for the banner, not a per-socket
    // unique id. It already fits the `cpu_id: u32` contract, no narrowing.
    let cpu_id = timer::cpuid(1).0;
    BootInfo {
        memory: discover_memory(),
        cpu_freq_hz: timer::frequency(),
        cpu_id,
        cpu_count: 1,
    }
}

#[cfg(feature = "selftest")]
#[path = "boot_info_tests.rs"]
mod tests;
