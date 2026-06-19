//! x86-64 `BootInfo` assembly, lifted out of `arch-sys-none-x86-64` (SP04 04a).
//!
//! Like the aarch64 assembly, the kernel is platform-neutral and learns the
//! machine's facts only from the [`BootInfo`] the boot path hands it at entry.
//! x86 differs from aarch64 in *where* the memory map comes from: the Multiboot1
//! bootloader builds an information structure and leaves its pointer in `EBX`,
//! which `_start` stashes into the floor's `MULTIBOOT_INFO_PTR`. This module
//! reads that pointer through the [`backend::multiboot_ptr`] accessor, parses the
//! Multiboot1 mmap into an arena-backed `&'static [MemoryRange]`, and reads the
//! CPU id / clock through the floor's [`backend::cpu_id_native`] /
//! [`backend::timer_frequency`] accessors (the `CPUID`/`RDTSC` asm stays below,
//! invariant #4).
//!
//! Nothing here is compiled in or assumed: a missing/zero pointer, or an
//! information structure without the mmap flag, degrades to an empty map
//! ("unknown") rather than a wrong number.

use {
    reovim_arch_sys_none_x86_64 as backend,
    reovim_kabi_platform::{BootInfo, MemoryKind, MemoryRange},
};

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
/// reports memory as unknown rather than reporting a wrong range. The boot
/// pointer + the arena are the floor's raw facts ([`backend::multiboot_ptr`] /
/// [`backend::arena_alloc_pages`]); the parse is device-neutral and lives here.
fn discover_memory() -> &'static [MemoryRange] {
    let ptr = backend::multiboot_ptr();
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
    let Ok(out_addr) = backend::arena_alloc_pages(count * core::mem::size_of::<MemoryRange>())
    else {
        return &[];
    };
    let out = out_addr as *mut MemoryRange;
    // SAFETY: `out_addr` is a page-aligned hand-out from the floor's static
    // `ARENA`, reserving `count` whole `MemoryRange`s. Page alignment (4096)
    // exceeds `align_of::<MemoryRange>()`, and the monotonic bump never hands the
    // same bytes out twice, so the `'static` slice is honest and unaliased. The
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
/// probe, and the CPU id from `CPUID` — the last two read through the floor's
/// NATIVE-value accessors. The floor parks every secondary core at boot, so the
/// CPU count is the single-core constant `1`.
#[must_use]
pub fn collect_boot_info() -> BootInfo {
    // CPUID leaf 1 eax is the processor version information (family / model /
    // stepping / type) — an identifying value for the banner, not a per-socket
    // unique id. It already fits the `cpu_id: u32` contract, no narrowing.
    let cpu_id = backend::cpu_id_native();
    BootInfo {
        memory: discover_memory(),
        cpu_freq_hz: backend::timer_frequency(),
        cpu_id,
        cpu_count: 1,
        // Cache geometry / affinity / SDRAM clock / heap capacity are not yet
        // discovered on x86 — default them to 0 (honest "unknown") so the target
        // keeps compiling; an x86 discovery backend fills them in future work.
        ..BootInfo::default()
    }
}

/// x86 freestanding has no device tree; the device inventory is always empty.
///
/// Mirror of the aarch64 `collect_device_inventory` so the device-neutral
/// boot surface is symmetric across both freestanding arches.
#[must_use]
pub fn collect_device_inventory() -> reovim_kabi_platform::DeviceInventory {
    reovim_kabi_platform::DeviceInventory::default()
}

// L12 layout: tests live in the sibling file `boot_info_x86_tests.rs`, declared
// as a `#[path]` child so `super::` reaches the private mmap parser, the type
// classifier, and the floor accessors. The pure-parser cases run anywhere; the
// `boot_info_*` live-hardware cases run on the real QEMU q35 machine through the
// floor's asm accessors. The file is only compiled under `selftest`.
#[cfg(feature = "selftest")]
#[path = "boot_info_x86_tests.rs"]
mod tests;
