//! Raw hardware-discovery facts for the aarch64 bare-metal floor.
//!
//! The generic system-kernel bridge receives neutral facts. What STAYS here is
//! the target-specific raw mechanism and register-layout interpretation it
//! cannot carry across the floor boundary (invariant #4, asm confinement):
//!
//! - the system-register reads (`MIDR_EL1` / `CTR_EL0` / `CLIDR_EL1` /
//!   `MPIDR_EL1` / `CCSIDR_EL1`), each an `mrs` that must stay in an
//!   `arch-sys-{target}` crate;
//! - `discover_memory`, which queries the `VideoCore` mailbox
//!   (`framebuffer::arm_memory`, raw MMIO) and backs the result with the static
//!   page arena — both raw mechanism that stays below.
//!
//! Boot composition roots read neutral accessors (`cpu_id`,
//! `cache_geometry`, `cpu_affinity`, `discover_memory`) through the crate root
//! and pass them into the system-kernel bridge.

use core::arch::asm;

use reovim_kabi_platform::{MemoryKind, MemoryRange};

use super::{arena, framebuffer};

/// Decoded cache facts from the aarch64 cache-identification registers.
///
/// ```rust,ignore
/// use reovim_arch_sys_none_aarch64::Aarch64CacheGeometry;
///
/// let geometry = Aarch64CacheGeometry {
///     cache_line_bytes: 64,
///     l1d_bytes: 32 * 1024,
///     l1i_bytes: 48 * 1024,
///     l2_bytes: 1024 * 1024,
/// };
/// assert_eq!(geometry.cache_line_bytes, 64);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Aarch64CacheGeometry {
    pub cache_line_bytes: u32,
    pub l1d_bytes: u32,
    pub l1i_bytes: u32,
    pub l2_bytes: u32,
}

/// Reads `MIDR_EL1`, the CPU main-id register.
#[must_use]
pub fn midr() -> u64 {
    let value: u64;
    // SAFETY: MIDR_EL1 is a read-only identification register readable at EL1
    // on this target; the read touches no memory and has no side effects.
    unsafe {
        asm!(
            "mrs {value}, midr_el1",
            value = out(reg) value,
            options(nomem, nostack, preserves_flags),
        );
    }
    value
}

/// Reads `CTR_EL0`, the cache-type register (smallest cache line geometry).
#[must_use]
pub fn ctr() -> u64 {
    let value: u64;
    // SAFETY: CTR_EL0 is a read-only cache-identification register; the read
    // touches no memory and has no side effects.
    unsafe {
        asm!(
            "mrs {value}, ctr_el0",
            value = out(reg) value,
            options(nomem, nostack, preserves_flags),
        );
    }
    value
}

/// Reads `CLIDR_EL1`, the cache-level-id register (which cache types exist at
/// each level).
#[must_use]
pub fn clidr() -> u64 {
    let value: u64;
    // SAFETY: CLIDR_EL1 is a read-only cache-identification register readable at
    // EL1; the read touches no memory and has no side effects.
    unsafe {
        asm!(
            "mrs {value}, clidr_el1",
            value = out(reg) value,
            options(nomem, nostack, preserves_flags),
        );
    }
    value
}

/// Reads `MPIDR_EL1`, the multiprocessor-affinity register.
#[must_use]
pub fn mpidr() -> u64 {
    let value: u64;
    // SAFETY: MPIDR_EL1 is a read-only identification register readable at EL1;
    // the read touches no memory and has no side effects.
    unsafe {
        asm!(
            "mrs {value}, mpidr_el1",
            value = out(reg) value,
            options(nomem, nostack, preserves_flags),
        );
    }
    value
}

/// Selects a cache via `CSSELR_EL1` and reads its `CCSIDR_EL1` size register.
///
/// `level` is one-based (1 = L1); `instruction` selects the instruction cache
/// (`InD = 1`) rather than the data/unified cache (`InD = 0`). The `isb` after
/// the selector write is mandatory: it orders the `CSSELR_EL1` update before the
/// `CCSIDR_EL1` read so the size register reflects the just-selected cache.
///
/// The target-side [`cache_geometry`] helper decodes the returned
/// `CCSIDR_EL1` value before the composition root hands neutral facts to the
/// system-kernel bridge.
#[must_use]
pub fn read_ccsidr(level: u8, instruction: bool) -> u64 {
    // CSSELR_EL1: Level in bits [3:1] (zero-based), InD in bit [0].
    let csselr = (u64::from(level - 1) << 1) | u64::from(instruction);
    let ccsidr: u64;
    // SAFETY: CSSELR_EL1 is the architected cache-size selector and CCSIDR_EL1
    // the read-only size register; both are EL1-accessible. The write only
    // selects which cache CCSIDR_EL1 reports and has no memory side effects; the
    // `isb` orders the selection before the read.
    unsafe {
        asm!(
            "msr csselr_el1, {sel}",
            "isb",
            "mrs {out}, ccsidr_el1",
            sel = in(reg) csselr,
            out = out(reg) ccsidr,
            options(nomem, nostack, preserves_flags),
        );
    }
    ccsidr
}

/// Decodes the [`reovim_kabi_platform::BootInfo::cpu_id`] value from `MIDR_EL1`.
///
/// ```rust,ignore
/// // Target-only: reads MIDR_EL1 through the aarch64 floor.
/// let cpu = reovim_arch_sys_none_aarch64::cpu_id();
/// assert_ne!(cpu, 0);
/// ```
#[must_use]
pub fn cpu_id() -> u32 {
    cpu_id_from_midr(midr())
}

/// Reads `MPIDR_EL1` as the neutral CPU-affinity fact.
///
/// ```rust,ignore
/// // Target-only: reads MPIDR_EL1 through the aarch64 floor.
/// let affinity = reovim_arch_sys_none_aarch64::cpu_affinity();
/// let _cluster = affinity & 0xff;
/// ```
#[must_use]
pub fn cpu_affinity() -> u64 {
    mpidr()
}

/// Reads and decodes the target's cache geometry into neutral facts.
///
/// ```rust,ignore
/// // Target-only: reads CTR_EL0 / CLIDR_EL1 / CCSIDR_EL1.
/// let geometry = reovim_arch_sys_none_aarch64::cache_geometry();
/// assert!(geometry.cache_line_bytes >= 16);
/// ```
#[must_use]
pub fn cache_geometry() -> Aarch64CacheGeometry {
    let clidr = clidr();
    Aarch64CacheGeometry {
        cache_line_bytes: min_cache_line_bytes(ctr()),
        l1d_bytes: cache_size_bytes(clidr, 1, false, read_ccsidr(1, false)),
        l1i_bytes: cache_size_bytes(clidr, 1, true, read_ccsidr(1, true)),
        l2_bytes: cache_size_bytes(clidr, 2, false, read_ccsidr(2, false)),
    }
}

/// `MIDR_EL1` low 32 bits: implementer / variant / architecture / part-num /
/// revision. Bits 63:32 are architecturally RES0 for the ID value used here.
#[allow(clippy::cast_possible_truncation)]
const fn cpu_id_from_midr(midr: u64) -> u32 {
    midr as u32
}

/// Minimum data cache line size in bytes from `CTR_EL0`.
///
/// `DminLine` (`CTR_EL0[19:16]`) is the log2 of the line size in 4-byte words,
/// so the byte size is `4 << DminLine` (64 on Cortex-A72).
#[allow(clippy::cast_possible_truncation)]
const fn min_cache_line_bytes(ctr: u64) -> u32 {
    let dmin_line = (ctr >> 16) & 0xF;
    (4u32) << dmin_line
}

/// The `CtypeN` field for `level` (one-based) in `CLIDR_EL1`: 3 bits per level,
/// L1 at bits [2:0]. Values: 0 none, 1 I-only, 2 D-only, 3 separate I+D, 4
/// unified.
#[allow(clippy::cast_possible_truncation, clippy::cast_lossless)]
const fn cache_type(clidr: u64, level: u8) -> u8 {
    let shift = 3 * (level as u32 - 1);
    ((clidr >> shift) & 0x7) as u8
}

/// Whether `level` (one-based) has a cache of the requested kind, per its
/// `CLIDR_EL1` `CtypeN` field.
const fn cache_present(clidr: u64, level: u8, instruction: bool) -> bool {
    let ctype = cache_type(clidr, level);
    if instruction {
        ctype == 0b001 || ctype == 0b011
    } else {
        // data-only, separate (its D half), or unified all expose a D/unified cache
        ctype == 0b010 || ctype == 0b011 || ctype == 0b100
    }
}

/// Decodes a cache size in bytes from a `CCSIDR_EL1` value.
///
/// Classic, non-`CCIDX` format (`Cortex-A72` does not implement `CCIDX`):
/// `LineSize`[2:0] = log2(line bytes) - 4, `Associativity`[12:3] = ways - 1,
/// `NumSets`[27:13] = sets - 1. Size = sets x ways x line.
#[allow(clippy::cast_possible_truncation)]
const fn decode_cache_size(ccsidr: u64) -> u32 {
    let line_bytes = 1u32 << (((ccsidr & 0x7) as u32) + 4);
    let assoc = (((ccsidr >> 3) & 0x3FF) as u32) + 1;
    let sets = (((ccsidr >> 13) & 0x7FFF) as u32) + 1;
    sets.saturating_mul(assoc).saturating_mul(line_bytes)
}

/// Size in bytes of the level's cache of the requested kind, or `0` when that
/// cache is absent at that level.
fn cache_size_bytes(clidr: u64, level: u8, instruction: bool, ccsidr: u64) -> u32 {
    if !cache_present(clidr, level, instruction) {
        return 0;
    }
    decode_cache_size(ccsidr)
}

/// Queries the mailbox for the ARM RAM region and stores it as a one-element
/// `'static` memory map backed by the boot arena.
///
/// Returns an empty map if the query fails or the arena cannot back it — the
/// kernel then reports memory as unknown rather than reporting a wrong range.
///
/// Stays in the raw-mechanism crate: it drives the `VideoCore` mailbox
/// (`framebuffer::arm_memory`, MMIO) and hands out from the static page arena,
/// neither of which crosses the floor boundary. The boot composition root reads
/// the assembled slice through the [`super::discover_memory`] accessor and
/// passes it into the system-kernel bridge.
#[must_use]
pub fn discover_memory() -> &'static [MemoryRange] {
    let Some((base, len)) = framebuffer::arm_memory() else {
        return &[];
    };
    let range = MemoryRange {
        base,
        len,
        kind: MemoryKind::Usable,
    };

    let Ok(addr) = arena::alloc_pages(core::mem::size_of::<MemoryRange>()) else {
        return &[];
    };
    let ptr = addr as *mut MemoryRange;
    // SAFETY: `addr` is a page-aligned hand-out from the static `ARENA`, whose
    // lifetime equals the process — the monotonic bump allocator never frees or
    // relocates it, so the `'static` lifetime on the returned slice is honest.
    // Page alignment (4096) exceeds `align_of::<MemoryRange>()`, and the
    // hand-out reserved a whole page, so writing one element and reading it back
    // as a one-element slice both stay within owned storage the bump never hands
    // out twice (no aliasing).
    unsafe {
        ptr.write(range);
        core::slice::from_raw_parts(ptr, 1)
    }
}

#[cfg(feature = "selftest")]
#[path = "boot_info_tests.rs"]
mod tests;
