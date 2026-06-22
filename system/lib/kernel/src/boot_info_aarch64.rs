//! aarch64 `BootInfo` shaping.
//!
//! The raw register/mailbox/arena reads stay below this crate. The composition
//! root gathers those facts and passes them in as [`Aarch64BootFacts`]; this
//! module keeps only the pure cache decode and neutral [`BootInfo`] assembly.

use reovim_kabi_platform::{BootInfo, MemoryRange};

/// Raw aarch64 boot facts gathered by the provider/composition layer.
pub struct Aarch64BootFacts {
    pub memory: &'static [MemoryRange],
    pub timer_frequency_hz: u64,
    pub midr: u64,
    pub ctr: u64,
    pub clidr: u64,
    pub l1d_ccsidr: u64,
    pub l1i_ccsidr: u64,
    pub l2_ccsidr: u64,
    pub mpidr: u64,
    pub sdram_clock_hz: Option<u64>,
    pub heap_total_bytes: u64,
}

impl Default for Aarch64BootFacts {
    fn default() -> Self {
        Self {
            memory: &[],
            timer_frequency_hz: 0,
            midr: 0,
            ctr: 0,
            clidr: 0,
            l1d_ccsidr: 0,
            l1i_ccsidr: 0,
            l2_ccsidr: 0,
            mpidr: 0,
            sdram_clock_hz: None,
            heap_total_bytes: 0,
        }
    }
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
fn cache_size_bytes(clidr: u64, level: u8, instruction: bool, ccsidr: u64) -> u32 {
    if !cache_present(clidr, level, instruction) {
        return 0;
    }
    decode_cache_size(ccsidr)
}

/// Assembles caller-supplied aarch64 boot facts into [`BootInfo`].
#[must_use]
pub fn collect_boot_info(facts: Aarch64BootFacts) -> BootInfo {
    // MIDR_EL1's low 32 bits carry the implementer / variant / architecture /
    // part-num / revision fields; bits 63:32 are architecturally RES0, so
    // narrowing to the `cpu_id: u32` contract loses nothing.
    #[allow(clippy::cast_possible_truncation)]
    let cpu_id = facts.midr as u32;
    BootInfo {
        memory: facts.memory,
        cpu_freq_hz: facts.timer_frequency_hz,
        cpu_id,
        cpu_count: 1,
        cache_line_bytes: min_cache_line_bytes(facts.ctr),
        l1d_bytes: cache_size_bytes(facts.clidr, 1, false, facts.l1d_ccsidr),
        l1i_bytes: cache_size_bytes(facts.clidr, 1, true, facts.l1i_ccsidr),
        l2_bytes: cache_size_bytes(facts.clidr, 2, false, facts.l2_ccsidr),
        cpu_affinity: facts.mpidr,
        mem_freq_hz: facts.sdram_clock_hz.unwrap_or(0),
        heap_total_bytes: facts.heap_total_bytes,
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
