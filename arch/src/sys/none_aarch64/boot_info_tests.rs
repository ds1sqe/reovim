//! Tests for `boot_info.rs`, compiled into the lib under `selftest`.
//!
//! L12 layout: declared in `boot_info.rs` via
//! `#[cfg(feature = "selftest")] #[path = "boot_info_tests.rs"] mod tests;`, so
//! `super::` reaches the private register decoders and the collector.
//!
//! Two tiers: the `cache_*`/`min_line_*` cases drive the pure register
//! decoders over synthetic `CTR`/`CLIDR`/`CCSIDR` values — deterministic, no
//! hardware. The `boot_info_*` cases run on the real QEMU raspi4b machine to
//! prove the live register/mailbox discovery end to end.

use {
    super::{
        arena, cache_present, cache_size_bytes, cache_type, collect_boot_info, decode_cache_size,
        min_cache_line_bytes,
    },
    crate::{arch_test, testrt},
};

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
    testrt::check_eq(cache_size_bytes(clidr, 3, false), 0u32);
});

arch_test!(boot_info_discovers_real_cache_and_affinity, {
    // End-to-end on the real raspi4b machine: live register reads yield a real
    // cache line, non-zero L1 sizes, a well-formed affinity (MPIDR_EL1[31] is
    // RES1 on ARMv8), and the static arena capacity.
    let bi = collect_boot_info();
    testrt::check_eq(bi.cache_line_bytes, 64u32); // Cortex-A72 minimum line
    testrt::check(bi.l1d_bytes != 0, "L1-D size discovered");
    testrt::check(bi.l1i_bytes != 0, "L1-I size discovered");
    testrt::check((bi.cpu_affinity >> 31) & 1 == 1, "MPIDR_EL1[31] RES1 set");
    testrt::check_eq(bi.heap_total_bytes, arena::capacity() as u64);
});
