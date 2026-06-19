//! Runtime hardware discovery for the aarch64 bare-metal floor.
//!
//! The kernel is platform-neutral and learns the machine's facts only from the
//! [`BootInfo`] the floor hands it at entry. This module assembles that struct
//! from what the BCM2711 exposes at runtime:
//!
//! - RAM: the `VideoCore` mailbox "get ARM memory" tag (`framebuffer::arm_memory`).
//! - CPU clock: `CNTFRQ_EL0`, the generic-timer frequency (`timer::frequency`).
//! - CPU model: `MIDR_EL1`, the main-id register.
//! - Cache geometry: `CTR_EL0` (min line), `CLIDR_EL1` + `CCSIDR_EL1` (L1-D/L1-I/L2 sizes).
//! - CPU affinity: `MPIDR_EL1`, the multiprocessor-affinity register.
//! - Memory clock: the mailbox "get clock rate" tag for SDRAM (`framebuffer::sdram_clock_hz`).
//! - Heap capacity: the static page-arena size (`arena::capacity`).
//!
//! Nothing here is compiled in or assumed — every value is read from hardware,
//! and a query that fails degrades to "unknown" (an empty map / zeroed field)
//! rather than reporting a wrong number.

use core::arch::asm;

use reovim_kabi_platform::{BootInfo, MemoryKind, MemoryRange};

use super::{arena, framebuffer, timer};

/// Reads `MIDR_EL1`, the CPU main-id register.
fn midr() -> u64 {
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
fn ctr() -> u64 {
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
fn clidr() -> u64 {
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
fn mpidr() -> u64 {
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
fn read_ccsidr(level: u8, instruction: bool) -> u64 {
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

/// Minimum data cache line size in bytes from `CTR_EL0`.
///
/// `DminLine` (`CTR_EL0[19:16]`) is the log2 of the line size in 4-byte words, so
/// the byte size is `4 << DminLine` (64 on `Cortex-A72`).
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
/// `LineSize`[2:0] = log2(line bytes) − 4, `Associativity`[12:3] = ways − 1,
/// `NumSets`[27:13] = sets − 1. Size = sets × ways × line. Pure over the register
/// value so synthetic values exercise the arithmetic without hardware.
#[allow(clippy::cast_possible_truncation)]
const fn decode_cache_size(ccsidr: u64) -> u32 {
    let line_bytes = 1u32 << (((ccsidr & 0x7) as u32) + 4);
    let assoc = (((ccsidr >> 3) & 0x3FF) as u32) + 1;
    let sets = (((ccsidr >> 13) & 0x7FFF) as u32) + 1;
    sets.saturating_mul(assoc).saturating_mul(line_bytes)
}

/// Size in bytes of the level's cache of the requested kind, or `0` when that
/// cache is absent at that level.
fn cache_size_bytes(clidr: u64, level: u8, instruction: bool) -> u32 {
    if !cache_present(clidr, level, instruction) {
        return 0;
    }
    decode_cache_size(read_ccsidr(level, instruction))
}

/// Queries the mailbox for the ARM RAM region and stores it as a one-element
/// `'static` memory map backed by the boot arena.
///
/// Returns an empty map if the query fails or the arena cannot back it — the
/// kernel then reports memory as unknown rather than reporting a wrong range.
fn discover_memory() -> &'static [MemoryRange] {
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

/// Discovers the machine's hardware facts and assembles them into a [`BootInfo`]
/// for `Init::new`.
///
/// All facts are read from hardware at call time; the floor parks every
/// secondary core at boot, so the CPU count is the single-core constant `1`.
#[must_use]
pub fn collect_boot_info() -> BootInfo {
    // MIDR_EL1's low 32 bits carry the implementer / variant / architecture /
    // part-num / revision fields; bits 63:32 are architecturally RES0, so
    // narrowing to the `cpu_id: u32` contract loses nothing.
    #[allow(clippy::cast_possible_truncation)]
    let cpu_id = midr() as u32;
    let clidr = clidr();
    BootInfo {
        memory: discover_memory(),
        cpu_freq_hz: timer::frequency(),
        cpu_id,
        cpu_count: 1,
        cache_line_bytes: min_cache_line_bytes(ctr()),
        l1d_bytes: cache_size_bytes(clidr, 1, false),
        l1i_bytes: cache_size_bytes(clidr, 1, true),
        l2_bytes: cache_size_bytes(clidr, 2, false),
        cpu_affinity: mpidr(),
        mem_freq_hz: framebuffer::sdram_clock_hz().unwrap_or(0),
        heap_total_bytes: arena::capacity() as u64,
    }
}

// L12 layout: tests live in the sibling file `boot_info_tests.rs`, declared as
// a `#[path]` child so `super::` reaches the private register decoders and the
// collector. The file is only compiled when this module is (the lib.rs target
// gate), so the inner declaration needs only the `selftest` feature gate.
#[cfg(feature = "selftest")]
#[path = "boot_info_tests.rs"]
mod tests;
