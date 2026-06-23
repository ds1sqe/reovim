//! Tests for `init.rs` — `EditorInit`, `LauncherArgs`, `BootError` coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (arch_test! + testrt::run). The editor-core-selftest bin in
//! tests/fixtures/ runs these.

use {reovim_arch::arch_test, reovim_uapi::sched::ClockControl};

use crate::init::{BootError, EditorInit, LauncherArgs};

fn launcher_args_with_test_clock() -> LauncherArgs {
    let mut args = LauncherArgs::default();
    args.clock = ClockControl::new(test_monotonic, test_realtime);
    args
}

fn test_monotonic() -> i64 {
    1_000_000
}

fn test_realtime() -> i64 {
    946_684_800_000_000_001
}

arch_test!(launcher_args_default_disposition_is_recover, {
    use reovim_uapi::panic::Disposition;
    let args = LauncherArgs::default();
    assert!(
        matches!(args.disposition, Disposition::Recover),
        "default disposition must be Recover (6.2 §5)"
    );
});

arch_test!(launcher_args_default_ring_bytes_is_1mib, {
    let args = LauncherArgs::default();
    assert_eq!(args.log_ring_bytes, 1024 * 1024, "default ring = 1 MiB (LOG6)");
});

arch_test!(init_new_captures_boot_anchor, {
    let init = EditorInit::new(launcher_args_with_test_clock());
    // Wall anchor must be after year 2000 (Unix-epoch nanos).
    assert!(init.boot_anchor().wall_anchor > 946_684_800_000_000_000);
});

arch_test!(init_args_accessor_returns_args, {
    let mut args = LauncherArgs::default();
    args.log_ring_bytes = 512 * 1024;
    let init = EditorInit::new(args);
    assert_eq!(init.args().log_ring_bytes, 512 * 1024);
});

arch_test!(launcher_args_default_boot_info_is_empty, {
    let args = LauncherArgs::default();
    assert!(
        args.boot_info.memory.is_empty(),
        "default boot_info carries an empty memory map (no firmware to query)"
    );
    assert_eq!(args.boot_info.cpu_count, 0, "default boot_info has zeroed CPU fields");
});

arch_test!(launcher_args_boot_info_round_trips_diagnostic_view, {
    use reovim_uapi::system::{BootInfo, MemorySummary};

    let args = LauncherArgs {
        boot_info: BootInfo {
            memory: MemorySummary::new(3, 0x4000_0000),
            cpu_freq_hz: 54_000_000,
            cpu_id: 0x410f_d083,
            cpu_count: 1,
            // Deeper static facts (sub-plan 03): every new field carries a
            // distinct non-zero value so the round-trip proves each survives.
            cache_line_bytes: 64,
            l1d_bytes: 0x8000,
            l1i_bytes: 0xC000,
            l2_bytes: 0x10_0000,
            cpu_affinity: 0x8000_0000,
            mem_freq_hz: 1_600_000_000,
            heap_total_bytes: 0x10_0000,
        },
        ..LauncherArgs::default()
    };
    let init = EditorInit::new(args);
    let info = init.args().boot_info;

    // The diagnostic view survives push-at-entry, without exposing raw memory
    // range kinds to the editor core.
    assert_eq!(info.memory.range_count(), 3);
    assert_eq!(info.memory.usable_bytes(), 0x4000_0000);
    assert_eq!(info.cpu_freq_hz, 54_000_000);
    assert_eq!(info.cpu_id, 0x410f_d083);
    assert_eq!(info.cpu_count, 1);

    // Every deeper static fact survives push-at-entry byte for byte.
    assert_eq!(info.cache_line_bytes, 64);
    assert_eq!(info.l1d_bytes, 0x8000);
    assert_eq!(info.l1i_bytes, 0xC000);
    assert_eq!(info.l2_bytes, 0x10_0000);
    assert_eq!(info.cpu_affinity, 0x8000_0000);
    assert_eq!(info.mem_freq_hz, 1_600_000_000);
    assert_eq!(info.heap_total_bytes, 0x10_0000);
});

arch_test!(init_boot_returns_shared_editor_core, {
    let init = EditorInit::new(LauncherArgs::default());
    let editor_core = init.boot().expect("boot must succeed");
    // Verify abi field is accessible and has expected version.
    assert_eq!(editor_core.abi.version().major, 1);
});

arch_test!(boot_error_stage_variant_preserves_stage_number, {
    let e = BootError::Stage {
        stage: 3,
        reason: "test",
    };
    match e {
        BootError::Stage { stage, .. } => assert_eq!(stage, 3),
        BootError::Alloc | BootError::SeamRegistration { .. } => panic!("wrong variant"),
    }
});

arch_test!(boot_error_alloc_variant, {
    let e = BootError::Alloc;
    assert!(matches!(e, BootError::Alloc));
});

arch_test!(boot_alloc_fault_sweep_covers_alloc_error_arms, {
    // Sweep the allocator fault point across boot's allocations (ring, ring
    // Shared, abi Shared, bus Shared): each k executes a different
    // `map_err(|_| BootError::Alloc)?` arm. The sweep runs a fixed range
    // rather than stopping at the first success — a fault landing on a
    // swallowed render allocation inside a boot-stage event still boots OK,
    // which would otherwise end the sweep before the later `Shared` sites.
    for k in 0..96isize {
        reovim_arch::alloc::fault::fail_after(k);
        let _ = EditorInit::new(LauncherArgs::default()).boot();
        reovim_arch::alloc::fault::reset();
    }
});
