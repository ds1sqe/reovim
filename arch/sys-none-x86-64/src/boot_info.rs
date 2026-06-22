//! Multiboot1/E820 boot-memory discovery for the x86_64 bare-metal floor.
//!
//! The system-kernel bridge consumes neutral memory-map facts. The Multiboot
//! pointer walk, E820 type interpretation, and unsafe reads stay below the
//! bridge in this target crate.

use reovim_kabi_platform::MemoryKind;

pub use reovim_kabi_platform::MemoryRange;

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

/// Default number of `MemoryRange` slots a composition root should reserve for
/// Multiboot/E820 parsing.
///
/// ```rust,ignore
/// let mut storage = [reovim_arch_sys_none_x86_64::EMPTY_MEMORY_RANGE;
///     reovim_arch_sys_none_x86_64::MEMORY_STORAGE_ENTRIES];
/// assert_eq!(storage.len(), reovim_arch_sys_none_x86_64::MEMORY_STORAGE_ENTRIES);
/// ```
pub const MEMORY_STORAGE_ENTRIES: usize = 64;

/// Empty `MemoryRange` value for static storage initialization.
///
/// ```rust,ignore
/// use reovim_kabi_platform::MemoryKind;
///
/// assert_eq!(
///     reovim_arch_sys_none_x86_64::EMPTY_MEMORY_RANGE.kind,
///     MemoryKind::Reserved,
/// );
/// ```
pub const EMPTY_MEMORY_RANGE: MemoryRange = MemoryRange {
    base: 0,
    len: 0,
    kind: MemoryKind::Reserved,
};

/// Parses the bootloader memory map into caller-owned static storage.
///
/// Returns an empty map when no Multiboot pointer was stashed, the mmap flag is
/// clear, the map is empty, or the caller supplied no storage.
///
/// # Safety
///
/// The Multiboot information structure pointer stashed by the floor, when
/// nonzero, must point to a readable Multiboot1 information structure whose mmap
/// region is identity-mapped and readable for the duration of this call.
///
/// ```rust,ignore
/// static mut STORAGE: [reovim_arch_sys_none_x86_64::MemoryRange;
///     reovim_arch_sys_none_x86_64::MEMORY_STORAGE_ENTRIES] =
///     [reovim_arch_sys_none_x86_64::EMPTY_MEMORY_RANGE;
///         reovim_arch_sys_none_x86_64::MEMORY_STORAGE_ENTRIES];
///
/// // SAFETY: target boot code supplies the Multiboot pointer and identity map.
/// let memory = unsafe { reovim_arch_sys_none_x86_64::discover_memory(&mut STORAGE) };
/// let _ = memory.len();
/// ```
#[must_use]
pub unsafe fn discover_memory(storage: &'static mut [MemoryRange]) -> &'static [MemoryRange] {
    let ptr = super::multiboot_ptr();
    if ptr == 0 {
        return &[];
    }
    let info = ptr as usize as *const u8;
    // SAFETY: guaranteed by this function's safety contract.
    let Some((addr, len)) = (unsafe { mmap_region(info) }) else {
        return &[];
    };
    let entries = addr as *const u8;
    if storage.is_empty() {
        return &[];
    }
    // SAFETY: guaranteed by this function's safety contract. The parser writes
    // only up to `storage.len()` entries.
    let written = unsafe { parse_entries(entries, len, storage) }.min(storage.len());
    &storage[..written]
}

/// Maps a Multiboot1 memory-type code to the neutral [`MemoryKind`].
///
/// Multiboot1 types: 1 = available, 3 = ACPI reclaimable, 5 = defective; every
/// other code (2 reserved, 4 preserve-on-hibernation, ...) is conservatively
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
    // SAFETY: bit 6 set means the mmap_length/mmap_addr words are present.
    let len = unsafe { rd_u32(info, OFF_MMAP_LENGTH) } as usize;
    // SAFETY: as above.
    let addr = unsafe { rd_u32(info, OFF_MMAP_ADDR) } as usize;
    if addr == 0 || len == 0 {
        return None;
    }
    Some((addr, len))
}

/// Walks the Multiboot1 mmap entry array in `[addr, addr + len)`, writing each
/// range into `out` up to its capacity and returning the total entry count
/// encountered.
///
/// A zero-size entry stops the walk instead of spinning on a stride that never
/// advances. Passing an empty `out` counts entries without writing.
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
            break;
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

#[cfg(feature = "selftest")]
#[path = "boot_info_tests.rs"]
mod tests;
