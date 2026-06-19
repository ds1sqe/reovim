//! aarch64 `BootInfo` assembly: the device-neutral half of the bare-metal
//! hardware discovery, lifted out of `arch-sys-none-aarch64` (SP04 04a).
//!
//! The kernel is platform-neutral and learns the machine's facts only from the
//! [`BootInfo`] the boot path hands it at entry. This module assembles that
//! struct from the raw facts the floor exposes through the
//! `reovim-arch-sys-none-aarch64` accessor surface — the §11
//! system-kernel → arch-sys-none impl edge:
//!
//! - RAM: the `VideoCore` mailbox "get ARM memory" tag, assembled into an
//!   arena-backed map below ([`backend::discover_memory`]).
//! - CPU clock: `CNTFRQ_EL0`, the generic-timer frequency
//!   ([`backend::timer_frequency`]).
//! - CPU model: `MIDR_EL1`, the main-id register ([`backend::midr`]).
//! - Cache geometry: `CTR_EL0` (min line), `CLIDR_EL1` + `CCSIDR_EL1` (L1-D /
//!   L1-I / L2 sizes), decoded by the pure helpers here.
//! - CPU affinity: `MPIDR_EL1`, the multiprocessor-affinity register
//!   ([`backend::mpidr`]).
//! - Memory clock: the mailbox "get clock rate" tag for SDRAM
//!   ([`backend::sdram_clock_hz`]).
//! - Heap capacity: the static page-arena size ([`backend::arena_capacity`]).
//!
//! Nothing here is compiled in or assumed — every value is read from hardware
//! through the accessors, and a query that fails degrades to "unknown" (an empty
//! map / zeroed field) rather than reporting a wrong number. The `asm!` reads
//! that produce these facts stay in the raw-mechanism crate (invariant #4); this
//! module carries only the pure decode + neutral-struct assembly.

use {reovim_arch_sys_none_aarch64 as backend, reovim_kabi_platform::BootInfo};

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
///
/// The presence check + size decode are pure; the `CCSIDR_EL1` read is the
/// floor's raw-fact accessor ([`backend::read_ccsidr`]). Reading only the
/// selected cache's size register (rather than always) keeps the asm reads
/// minimal — an absent level returns `0` without touching the register.
fn cache_size_bytes(clidr: u64, level: u8, instruction: bool) -> u32 {
    if !cache_present(clidr, level, instruction) {
        return 0;
    }
    decode_cache_size(backend::read_ccsidr(level, instruction))
}

/// Discovers the machine's hardware facts and assembles them into a [`BootInfo`]
/// for `Init::new`.
///
/// All facts are read from hardware at call time through the
/// `arch-sys-none-aarch64` raw-fact accessors; the floor parks every secondary
/// core at boot, so the CPU count is the single-core constant `1`.
#[must_use]
pub fn collect_boot_info() -> BootInfo {
    // MIDR_EL1's low 32 bits carry the implementer / variant / architecture /
    // part-num / revision fields; bits 63:32 are architecturally RES0, so
    // narrowing to the `cpu_id: u32` contract loses nothing.
    #[allow(clippy::cast_possible_truncation)]
    let cpu_id = backend::midr() as u32;
    let clidr = backend::clidr();
    BootInfo {
        memory: backend::discover_memory(),
        cpu_freq_hz: backend::timer_frequency(),
        cpu_id,
        cpu_count: 1,
        cache_line_bytes: min_cache_line_bytes(backend::ctr()),
        l1d_bytes: cache_size_bytes(clidr, 1, false),
        l1i_bytes: cache_size_bytes(clidr, 1, true),
        l2_bytes: cache_size_bytes(clidr, 2, false),
        cpu_affinity: backend::mpidr(),
        mem_freq_hz: backend::sdram_clock_hz().unwrap_or(0),
        heap_total_bytes: backend::arena_capacity() as u64,
    }
}

// L12 layout: tests live in the sibling file `boot_info_aarch64_tests.rs`,
// declared as a `#[path]` child so `super::` reaches the private register
// decoders and the collector. The pure-decoder cases run anywhere; the
// `boot_info_*` live-hardware case runs on the real QEMU raspi4b machine through
// the floor's asm accessors. The file is only compiled under `selftest`.
#[cfg(feature = "selftest")]
#[path = "boot_info_aarch64_tests.rs"]
mod tests;
