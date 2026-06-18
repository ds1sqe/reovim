//! Tests for `diagnostics.rs` — the ping-pong probe verdict formatters and the
//! `health.` render mapping.
//!
//! Registered under the `selftest` feature; runs on the arch no_std selftest
//! runner (arch_test! + testrt::run). The verdict formatters are pure, so the
//! FAIL / edge branches are exercised here without the platform handle. Each
//! probe carries its own `key`, which the formatter echoes back into the line.

use reovim_arch::arch_test;

use crate::diagnostics::{
    clock_freq_msg, clock_monotonic_msg, cpu_cores_msg, cpu_id_msg, heap_msg, log_capacity_msg,
    log_occupancy_msg, mem_map_msg, mem_usable_msg, usable_memory,
};

arch_test!(cpu_id_zero_renders_fail, {
    let mut buf = [0u8; 64];
    let line = cpu_id_msg(&mut buf, "cpu.id", 0).expect("line fits");
    assert!(line.contains("FAIL"), "a zero id register must render FAIL: {line:?}");
});

arch_test!(cpu_id_nonzero_renders_hex_no_fail, {
    let mut buf = [0u8; 64];
    let line = cpu_id_msg(&mut buf, "cpu.id", 0x410f_d083).expect("line fits");
    assert!(line.contains("cpu.id: 0x410fd083"), "id rendered as keyed hex: {line:?}");
    assert!(!line.contains("FAIL"));
});

arch_test!(cpu_cores_zero_renders_fail, {
    let mut buf = [0u8; 64];
    let line = cpu_cores_msg(&mut buf, "cpu.cores", 0).expect("line fits");
    assert!(line.contains("FAIL"), "zero cores must render FAIL: {line:?}");
});

arch_test!(cpu_cores_singular_vs_plural, {
    let mut one = [0u8; 64];
    let one_line = cpu_cores_msg(&mut one, "cpu.cores", 1).expect("line fits");
    // `ends_with`, not `!contains("cores")` — the key "cpu.cores" itself ends in
    // "cores", so a substring check would false-positive on the singular line.
    assert!(one_line.ends_with("1 core"), "singular: {one_line:?}");
    let mut many = [0u8; 64];
    let many_line = cpu_cores_msg(&mut many, "cpu.cores", 4).expect("line fits");
    assert!(many_line.ends_with("4 cores"), "plural: {many_line:?}");
});

arch_test!(mem_map_empty_renders_fail, {
    let mut buf = [0u8; 64];
    let line = mem_map_msg(&mut buf, "mem.map", 0).expect("line fits");
    assert!(line.contains("FAIL"), "no map ranges must render FAIL: {line:?}");
});

arch_test!(mem_map_reports_range_count_singular_plural, {
    let mut one = [0u8; 64];
    let one_line = mem_map_msg(&mut one, "mem.map", 1).expect("line fits");
    assert!(one_line.ends_with("1 range"), "singular: {one_line:?}");
    let mut many = [0u8; 64];
    let many_line = mem_map_msg(&mut many, "mem.map", 3).expect("line fits");
    assert!(many_line.ends_with("3 ranges"), "plural: {many_line:?}");
});

arch_test!(mem_usable_absent_renders_unknown, {
    let mut buf = [0u8; 64];
    let line = mem_usable_msg(&mut buf, "mem.usable", 0, false).expect("line fits");
    assert!(line.contains("unknown"), "no map → unknown, not a misleading 0 MiB: {line:?}");
    assert!(!line.contains("FAIL"), "absent is unknown, not FAIL: {line:?}");
});

arch_test!(mem_usable_present_but_zero_renders_fail, {
    let mut buf = [0u8; 64];
    let line = mem_usable_msg(&mut buf, "mem.usable", 0, true).expect("line fits");
    assert!(line.contains("FAIL"), "a present map with no usable RAM must FAIL: {line:?}");
});

arch_test!(mem_usable_reports_mib, {
    let mut buf = [0u8; 64];
    let line = mem_usable_msg(&mut buf, "mem.usable", 1024 * (1 << 20), true).expect("line fits");
    assert!(line.contains("1024 MiB"), "usable RAM reported in MiB: {line:?}");
});

arch_test!(heap_probe_failure_renders_fail, {
    let mut fail = [0u8; 64];
    assert!(
        heap_msg(&mut fail, "heap.page", false)
            .expect("fits")
            .contains("FAIL")
    );
    let mut ok = [0u8; 64];
    let ok_line = heap_msg(&mut ok, "heap.small", true).expect("fits");
    assert!(ok_line.contains("heap.small: ok") && !ok_line.contains("FAIL"), "{ok_line:?}");
});

arch_test!(log_capacity_zero_renders_fail, {
    let mut buf = [0u8; 64];
    let line = log_capacity_msg(&mut buf, "log.capacity", 0).expect("line fits");
    assert!(line.contains("FAIL"), "a zero-capacity ring must render FAIL: {line:?}");
});

arch_test!(log_capacity_reports_slots, {
    let mut buf = [0u8; 64];
    let line = log_capacity_msg(&mut buf, "log.capacity", 249).expect("line fits");
    assert!(line.contains("249 slots"), "capacity reported in slots: {line:?}");
    assert!(!line.contains("FAIL"));
});

arch_test!(log_occupancy_empty_or_overflow_renders_fail, {
    let mut empty = [0u8; 64];
    assert!(
        log_occupancy_msg(&mut empty, "log.occupancy", 0, 249)
            .expect("fits")
            .contains("FAIL"),
        "an empty ring after boot means the lines never landed",
    );
    let mut over = [0u8; 64];
    assert!(
        log_occupancy_msg(&mut over, "log.occupancy", 250, 249)
            .expect("fits")
            .contains("FAIL"),
        "len exceeding capacity is corrupt accounting",
    );
});

arch_test!(log_occupancy_reports_entries, {
    let mut buf = [0u8; 64];
    let line = log_occupancy_msg(&mut buf, "log.occupancy", 18, 249).expect("line fits");
    assert!(line.contains("18 entries"), "live entries reported: {line:?}");
    assert!(!line.contains("FAIL"));
});

arch_test!(clock_freq_zero_renders_fail, {
    let mut buf = [0u8; 64];
    let line = clock_freq_msg(&mut buf, "clock.freq", 0).expect("line fits");
    assert!(line.contains("FAIL"), "an unreadable timer frequency must FAIL: {line:?}");
});

arch_test!(clock_freq_reports_mhz, {
    let mut buf = [0u8; 64];
    let line = clock_freq_msg(&mut buf, "clock.freq", 54_000_000).expect("line fits");
    assert!(line.contains("54 MHz"), "frequency reported in MHz: {line:?}");
    assert!(!line.contains("FAIL"));
});

arch_test!(clock_monotonic_frozen_renders_fail, {
    let mut buf = [0u8; 64];
    let line = clock_monotonic_msg(&mut buf, "clock.monotonic", false, 0).expect("line fits");
    assert!(line.contains("FAIL"), "a frozen counter must render FAIL: {line:?}");
});

arch_test!(clock_monotonic_advanced_reports_elapsed_no_fail, {
    let mut buf = [0u8; 64];
    let line = clock_monotonic_msg(&mut buf, "clock.monotonic", true, 2_345_000).expect("fits");
    assert!(line.contains("advanced 2345 us"), "elapsed reported in us: {line:?}");
    assert!(!line.contains("FAIL"), "an advancing counter must not FAIL: {line:?}");
});

arch_test!(usable_memory_sums_only_usable_ranges, {
    use reovim_kabi_platform::{BootInfo, MemoryKind, MemoryRange};

    static R: [MemoryRange; 2] = [
        MemoryRange {
            base: 0,
            len: 1024 * (1 << 20),
            kind: MemoryKind::Usable,
        },
        MemoryRange {
            base: 0x4000_0000,
            len: 256 * (1 << 20),
            kind: MemoryKind::Reserved,
        },
    ];
    let bi = BootInfo {
        memory: &R,
        cpu_freq_hz: 0,
        cpu_id: 0,
        cpu_count: 1,
    };
    let (usable, present) = usable_memory(&bi);
    assert!(present, "a non-empty map is present");
    assert_eq!(usable, 1024 * (1 << 20), "only Usable ranges count toward the total");
});

arch_test!(empty_boot_info_memory_is_absent, {
    use reovim_kabi_platform::BootInfo;

    let (usable, present) = usable_memory(&BootInfo::default());
    assert!(!present, "the empty default map is reported absent");
    assert_eq!(usable, 0);
});

arch_test!(health_event_maps_to_health_subsystem_and_strips_prefix, {
    use {
        crate::event_bus::{BootStageFields, DS12Event},
        reovim_uapi_abi::error::LogLevel,
    };

    // A health event carries "health.<metric>" in its id; render_event maps the
    // family to the `health` subsystem and renders the metric as the message.
    let ev = DS12Event {
        ts_nanos: 1_000_000,
        level: LogLevel::Info,
        event: "health.mem.usable: 99 MiB",
        fields: BootStageFields {
            stage: 0,
            error_code: None,
        },
    };
    let bytes = crate::log::render::render_event(&ev).expect("render must succeed");
    let s = core::str::from_utf8(bytes.as_slice()).expect("UTF-8");

    assert!(s.contains(" health: "), "subsystem token is `health`: {s:?}");
    assert!(s.contains("mem.usable: 99 MiB"), "metric is the message body: {s:?}");
    assert!(!s.contains("health.mem"), "the `health.` family prefix is stripped: {s:?}");
});
