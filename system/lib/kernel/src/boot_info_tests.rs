//! Tests for target-neutral `BootInfo` shaping over neutral facts.

use {
    super::{BootFacts, collect_boot_info},
    reovim_kabi_platform::{MemoryKind, MemoryRange},
    reovim_testrt::{self as testrt, arch_test},
    reovim_uapi_system::MemorySummary,
};

static MEMORY: [MemoryRange; 3] = [
    MemoryRange {
        base: 0,
        len: 0x9_F000,
        kind: MemoryKind::Usable,
    },
    MemoryRange {
        base: 0x10_0000,
        len: 0x700_0000,
        kind: MemoryKind::Reserved,
    },
    MemoryRange {
        base: 0x8000_0000,
        len: 0x3FF8_0000,
        kind: MemoryKind::Usable,
    },
];

arch_test!(boot_info_shapes_supplied_neutral_facts, {
    let bi = collect_boot_info(BootFacts {
        memory: &MEMORY,
        cpu_freq_hz: 54_000_000,
        cpu_id: 0x410F_D083,
        cpu_count: 4,
        cache_line_bytes: 64,
        l1d_bytes: 32 * 1024,
        l1i_bytes: 48 * 1024,
        l2_bytes: 1024 * 1024,
        cpu_affinity: 1 << 31,
        mem_freq_hz: 400_000_000,
        heap_total_bytes: 8 * 1024 * 1024,
    });
    testrt::check_eq(bi.memory, MemorySummary::new(3, 0x4001_F000));
    testrt::check_eq(bi.cpu_freq_hz, 54_000_000u64);
    testrt::check_eq(bi.cpu_id, 0x410F_D083u32);
    testrt::check_eq(bi.cpu_count, 4u32);
    testrt::check_eq(bi.cache_line_bytes, 64u32);
    testrt::check_eq(bi.l1d_bytes, 32 * 1024u32);
    testrt::check_eq(bi.l1i_bytes, 48 * 1024u32);
    testrt::check_eq(bi.l2_bytes, 1024 * 1024u32);
    testrt::check_eq(bi.cpu_affinity, 1u64 << 31);
    testrt::check_eq(bi.mem_freq_hz, 400_000_000u64);
    testrt::check_eq(bi.heap_total_bytes, 8 * 1024 * 1024u64);
});

arch_test!(boot_info_defaults_unknown_facts_to_zero, {
    let bi = collect_boot_info(BootFacts::default());
    testrt::check_eq(bi.memory, MemorySummary::default());
    testrt::check_eq(bi.cpu_freq_hz, 0u64);
    testrt::check_eq(bi.cpu_id, 0u32);
    testrt::check_eq(bi.cpu_count, 0u32);
    testrt::check_eq(bi.cache_line_bytes, 0u32);
    testrt::check_eq(bi.heap_total_bytes, 0u64);
});
