//! Tests for `boot_info_aarch64.rs`, compiled into the lib under `selftest`.
//!
//! L12 layout: declared in `boot_info_aarch64.rs` via
//! `#[cfg(feature = "selftest")] #[path = "boot_info_aarch64_tests.rs"] mod tests;`,
//! so `super::` reaches the pure register decoders and the collector that lifted
//! up with the assembly (SP04 04a).
//!
//! The cases drive pure register decoders and `BootInfo` shaping over synthetic
//! facts. Raw hardware discovery now lives in the composition root below this
//! crate.

use {
    super::{
        Aarch64BootFacts, cache_present, cache_size_bytes, cache_type, collect_boot_info,
        decode_cache_size, min_cache_line_bytes,
    },
    reovim_kabi_platform::{MemoryKind, MemoryRange},
    reovim_testrt::{self as testrt, arch_test},
};

static MEMORY: [MemoryRange; 1] = [MemoryRange {
    base: 0x80000,
    len: 0x3FF8_0000,
    kind: MemoryKind::Usable,
}];

arch_test!(min_line_decodes_dminline, {
    // DminLine (CTR_EL0[19:16]) is log2 of the line size in 4-byte words.
    testrt::check_eq(min_cache_line_bytes(0x4 << 16), 64u32); // 4 << 4 = 64 (A72)
    testrt::check_eq(min_cache_line_bytes(0x0 << 16), 4u32); // 4 << 0 = 4 (floor)
    testrt::check_eq(min_cache_line_bytes(0x5 << 16), 128u32); // 4 << 5 = 128
    // Only DminLine participates; unrelated CTR bits do not leak in.
    testrt::check_eq(min_cache_line_bytes(0xFFF0_FFFF | (0x4 << 16)), 64u32);
});

arch_test!(cache_size_decodes_sets_ways_line, {
    // 32 KiB: line 64 B (LineSize=2), 2-way (Assoc=1), 256 sets (NumSets=255).
    let ccsidr = 2 | (1 << 3) | (255 << 13);
    testrt::check_eq(decode_cache_size(ccsidr), 32 * 1024u32);
    // 1 MiB L2: line 64 B, 16-way (Assoc=15), 1024 sets (NumSets=1023).
    let l2 = 2 | (15 << 3) | (1023 << 13);
    testrt::check_eq(decode_cache_size(l2), 1024 * 1024u32);
    // Minimum encoding: line 16 B (LineSize=0), direct-mapped, one set.
    testrt::check_eq(decode_cache_size(0), 16u32);
});

arch_test!(cache_type_and_presence_from_clidr, {
    // L1 separate I+D (Ctype1 = 3), L2 unified (Ctype2 = 4).
    let clidr = 3 | (4 << 3);
    testrt::check_eq(cache_type(clidr, 1), 3u8);
    testrt::check_eq(cache_type(clidr, 2), 4u8);
    testrt::check(cache_present(clidr, 1, false), "L1 has a data cache");
    testrt::check(cache_present(clidr, 1, true), "L1 has an instruction cache");
    testrt::check(cache_present(clidr, 2, false), "L2 unified counts as data/unified");
    testrt::check(!cache_present(clidr, 2, true), "L2 unified is not an I-cache");
    // An absent level (Ctype = 0) yields a zero size through the wrapper.
    testrt::check_eq(cache_size_bytes(clidr, 3, false, 0), 0u32);
});

arch_test!(boot_info_shapes_supplied_facts, {
    let clidr = 3 | (4 << 3);
    let l1 = 2 | (1 << 3) | (255 << 13);
    let l2 = 2 | (15 << 3) | (1023 << 13);
    let bi = collect_boot_info(Aarch64BootFacts {
        memory: &MEMORY,
        timer_frequency_hz: 54_000_000,
        midr: 0x410F_D083,
        ctr: 0x4 << 16,
        clidr,
        l1d_ccsidr: l1,
        l1i_ccsidr: l1,
        l2_ccsidr: l2,
        mpidr: 1 << 31,
        sdram_clock_hz: Some(400_000_000),
        heap_total_bytes: 8 * 1024 * 1024,
    });
    testrt::check_eq(bi.memory.len(), 1usize);
    testrt::check_eq(bi.cpu_freq_hz, 54_000_000u64);
    testrt::check_eq(bi.cache_line_bytes, 64u32);
    testrt::check_eq(bi.l1d_bytes, 32 * 1024u32);
    testrt::check_eq(bi.l1i_bytes, 32 * 1024u32);
    testrt::check_eq(bi.l2_bytes, 1024 * 1024u32);
    testrt::check_eq(bi.cpu_affinity, 1u64 << 31);
    testrt::check_eq(bi.mem_freq_hz, 400_000_000u64);
    testrt::check_eq(bi.heap_total_bytes, 8 * 1024 * 1024u64);
});
