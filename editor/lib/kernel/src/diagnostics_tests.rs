//! Tests for `diagnostics.rs` — the ping-pong probe verdict formatters and the
//! `health.` render mapping.
//!
//! Registered under the `selftest` feature; runs on the arch no_std selftest
//! runner (arch_test! + testrt::run). The verdict formatters are pure, so the
//! FAIL / edge branches are exercised here without the platform handle. Each
//! probe carries its own `key`, which the formatter echoes back into the line.

use reovim_arch::arch_test;

use crate::diagnostics::{
    clock_freq_msg, clock_monotonic_msg, cpu_affinity_msg, cpu_cache_msg, cpu_cores_msg,
    cpu_id_msg, cpu_vendor_msg, heap_msg, heap_total_msg, implementer_name, log_capacity_msg,
    log_occupancy_msg, mem_freq_msg, mem_map_msg, mem_usable_msg, usable_memory,
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
    use reovim_uapi::system::{BootInfo, MemorySummary};

    let bi = BootInfo {
        memory: MemorySummary::new(2, 1024 * (1 << 20)),
        cpu_freq_hz: 0,
        cpu_id: 0,
        cpu_count: 1,
        ..BootInfo::default()
    };
    let (usable, present) = usable_memory(&bi);
    assert!(present, "a non-empty map is present");
    assert_eq!(usable, 1024 * (1 << 20), "only Usable ranges count toward the total");
});

arch_test!(empty_boot_info_memory_is_absent, {
    use reovim_uapi::system::BootInfo;

    let (usable, present) = usable_memory(&BootInfo::default());
    assert!(!present, "the empty default map is reported absent");
    assert_eq!(usable, 0);
});

// ── 03 deeper-introspection probes ───────────────────────────────────────────

arch_test!(cpu_vendor_decodes_known_implementer, {
    let mut buf = [0u8; 64];
    // MIDR implementer byte 0x41 (ARM) in bits [31:24] of a Cortex-A72 id.
    let line = cpu_vendor_msg(&mut buf, "cpu.vendor", 0x410f_d083).expect("line fits");
    assert!(line.ends_with("ARM"), "0x41 implementer decodes to ARM: {line:?}");
    assert!(!line.contains("unknown"));
});

arch_test!(cpu_vendor_unknown_implementer_is_informational_not_fail, {
    let mut buf = [0u8; 64];
    // 0x99 is not a registered implementer; report it, do not FAIL (the zero-id
    // case is the `cpu.id` probe's job).
    let line = cpu_vendor_msg(&mut buf, "cpu.vendor", 0x990f_0000).expect("line fits");
    assert!(line.contains("unknown (0x99)"), "unrecognized implementer shown raw: {line:?}");
    assert!(!line.contains("FAIL"), "an unknown vendor is informational, not FAIL: {line:?}");
});

arch_test!(implementer_name_table_maps_arm_and_broadcom, {
    assert_eq!(implementer_name(0x41), Some("ARM"));
    assert_eq!(implementer_name(0x42), Some("Broadcom"));
    assert_eq!(implementer_name(0x00), None);
});

arch_test!(cpu_cache_zero_line_renders_fail, {
    let mut buf = [0u8; 96];
    // CTR_EL0 unreadable → zero line size → FAIL, regardless of the size fields.
    let line = cpu_cache_msg(&mut buf, "cpu.cache", 0, 0x8000, 0xC000, 0x10_0000).expect("fits");
    assert!(line.contains("FAIL"), "a zero cache line must render FAIL: {line:?}");
});

arch_test!(cpu_cache_reports_line_and_sizes_in_kib, {
    let mut buf = [0u8; 96];
    // line 64 B, L1d 32 KiB, L1i 48 KiB, L2 1024 KiB (Cortex-A72 raspi4b shape).
    let line =
        cpu_cache_msg(&mut buf, "cpu.cache", 64, 32 * 1024, 48 * 1024, 1 << 20).expect("line fits");
    assert!(line.contains("line 64 B"), "min line size in bytes: {line:?}");
    assert!(line.contains("L1d 32 KiB"), "L1-D in KiB: {line:?}");
    assert!(line.contains("L1i 48 KiB"), "L1-I in KiB: {line:?}");
    assert!(line.contains("L2 1024 KiB"), "L2 in KiB: {line:?}");
    assert!(!line.contains("FAIL"));
});

arch_test!(cpu_affinity_res1_clear_renders_fail, {
    let mut buf = [0u8; 64];
    // Bit 31 (RES1 on ARMv8) clear ⇒ malformed/unreadable MPIDR ⇒ FAIL.
    let line = cpu_affinity_msg(&mut buf, "cpu.affinity", 0x0000_0000).expect("line fits");
    assert!(line.contains("FAIL"), "MPIDR with bit 31 clear must FAIL: {line:?}");
});

arch_test!(cpu_affinity_well_formed_reports_hex, {
    let mut buf = [0u8; 64];
    // Bit 31 set (RES1) with Aff0 = 0x03 (core 3 of a quad cluster).
    let line = cpu_affinity_msg(&mut buf, "cpu.affinity", 0x8000_0003).expect("line fits");
    assert!(line.contains("0x3"), "affinity value reported in hex: {line:?}");
    assert!(!line.contains("FAIL"));
});

arch_test!(mem_freq_zero_renders_unknown_not_fail, {
    let mut buf = [0u8; 64];
    // QEMU's BCM2711 may not implement the SDRAM clock tag → 0 → unknown.
    let line = mem_freq_msg(&mut buf, "mem.freq", 0).expect("line fits");
    assert!(line.contains("unknown"), "an unreported clock is unknown: {line:?}");
    assert!(!line.contains("FAIL"), "unknown is honest, not a FAIL: {line:?}");
});

arch_test!(mem_freq_reports_mhz, {
    let mut buf = [0u8; 64];
    let line = mem_freq_msg(&mut buf, "mem.freq", 1_600_000_000).expect("line fits");
    assert!(line.contains("1600 MHz"), "SDRAM clock reported in MHz: {line:?}");
    assert!(!line.contains("FAIL"));
});

arch_test!(heap_total_zero_renders_fail, {
    let mut buf = [0u8; 64];
    let line = heap_total_msg(&mut buf, "heap.total", 0).expect("line fits");
    assert!(line.contains("FAIL"), "a zero heap capacity must render FAIL: {line:?}");
});

arch_test!(heap_total_reports_kib_then_mib, {
    let mut kib = [0u8; 64];
    // Below a mebibyte renders KiB.
    let kib_line = heap_total_msg(&mut kib, "heap.total", 512 * 1024).expect("fits");
    assert!(kib_line.contains("512 KiB"), "sub-MiB capacity in KiB: {kib_line:?}");
    let mut mib = [0u8; 64];
    // A whole mebibyte or more renders MiB — the floor's 1 MiB arena.
    let mib_line = heap_total_msg(&mut mib, "heap.total", 1 << 20).expect("fits");
    assert!(mib_line.contains("1 MiB"), "the 1 MiB arena reported in MiB: {mib_line:?}");
    assert!(!mib_line.contains("FAIL"));
});

arch_test!(health_event_maps_to_health_subsystem_and_strips_prefix, {
    use {
        crate::event_bus::{BootStageFields, DS12Event},
        reovim_uapi::abi::error::LogLevel,
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
