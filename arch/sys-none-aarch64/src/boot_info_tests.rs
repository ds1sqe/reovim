//! Tests for aarch64 boot-info register decoding.

use {
    super::{
        cache_present, cache_size_bytes, cache_type, cpu_id_from_midr, decode_cache_size,
        min_cache_line_bytes,
    },
    reovim_testrt::{self as testrt, arch_test},
};

arch_test!(cpu_id_uses_midr_low_bits, {
    testrt::check_eq(cpu_id_from_midr(0x410F_D083), 0x410F_D083u32);
    testrt::check_eq(cpu_id_from_midr(0xFFFF_FFFF_410F_D083), 0x410F_D083u32);
});

arch_test!(min_line_decodes_dminline, {
    // DminLine (CTR_EL0[19:16]) is log2 of the line size in 4-byte words.
    testrt::check_eq(min_cache_line_bytes(0x4 << 16), 64u32); // 4 << 4 = 64
    testrt::check_eq(min_cache_line_bytes(0x0 << 16), 4u32); // 4 << 0 = 4
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
