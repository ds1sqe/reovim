//! Raw hardware-discovery facts for the aarch64 bare-metal floor.
//!
//! The `BootInfo` *assembly* (decoding these facts into the neutral
//! `reovim_kabi_platform::BootInfo` the kernel reads at entry) lifted UP into
//! `reovim-system-kernel` (SP04 04a). What STAYS here is the raw mechanism it
//! cannot carry across the floor boundary (invariant #4, asm confinement):
//!
//! - the system-register reads (`MIDR_EL1` / `CTR_EL0` / `CLIDR_EL1` /
//!   `MPIDR_EL1` / `CCSIDR_EL1`), each an `mrs` that must stay in an
//!   `arch-sys-{target}` crate;
//! - `discover_memory`, which queries the `VideoCore` mailbox
//!   (`framebuffer::arm_memory`, raw MMIO) and backs the result with the static
//!   page arena — both raw mechanism that stays below.
//!
//! Boot composition roots read these through the crate-root raw-fact accessors
//! (`crate::midr` … `crate::discover_memory`) and pass them into the
//! system-kernel bridge; the pure register-decode arithmetic (line size, cache
//! geometry, type presence) is device-neutral and lives up there with the
//! assembly.

use core::arch::asm;

use reovim_kabi_platform::{MemoryKind, MemoryRange};

use super::{arena, framebuffer};

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
/// The system kernel decodes the returned `CCSIDR_EL1` value (the pure
/// `decode_cache_size` arithmetic lives up there); only the register selection +
/// read stay here behind the asm boundary.
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
