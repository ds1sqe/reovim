//! Tests for `init.rs` — `Init`, `LauncherArgs`, `BootError` coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (arch_test! + testrt::run). The kernel-selftest bin in
//! tests/fixtures/ runs these.

use reovim_arch::arch_test;

use crate::init::{BootError, Init, LauncherArgs};

arch_test!(launcher_args_default_disposition_is_recover, {
    use reovim_arch::panic::Disposition;
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
    let init = Init::new(LauncherArgs::default());
    // Wall anchor must be after year 2000 (Unix-epoch nanos).
    assert!(init.boot_anchor().wall_anchor > 946_684_800_000_000_000);
});

arch_test!(init_args_accessor_returns_args, {
    let mut args = LauncherArgs::default();
    args.log_ring_bytes = 512 * 1024;
    let init = Init::new(args);
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

arch_test!(launcher_args_boot_info_round_trips_multi_range, {
    use reovim_kabi_platform::{BootInfo, MemoryKind, MemoryRange};

    // `'static` backing for the slice — the arch payload's arena plays this role
    // on the floor; a `static` array stands in for it under the test runner.
    static RANGES: [MemoryRange; 3] = [
        MemoryRange {
            base: 0x0,
            len: 0x4000_0000,
            kind: MemoryKind::Usable,
        },
        MemoryRange {
            base: 0x4000_0000,
            len: 0x1000_0000,
            kind: MemoryKind::Reserved,
        },
        MemoryRange {
            base: 0x5000_0000,
            len: 0x0010_0000,
            kind: MemoryKind::Acpi,
        },
    ];

    let args = LauncherArgs {
        boot_info: BootInfo {
            memory: &RANGES,
            cpu_freq_hz: 54_000_000,
            cpu_id: 0x410f_d083,
            cpu_count: 1,
        },
        ..LauncherArgs::default()
    };
    let init = Init::new(args);
    let info = init.args().boot_info;

    // The whole multi-range map survives push-at-entry, byte for byte.
    assert_eq!(info.memory.len(), 3, "all three ranges survive the round-trip");
    assert_eq!(info.memory[0].kind, MemoryKind::Usable);
    assert_eq!(info.memory[1].base, 0x4000_0000);
    assert_eq!(info.memory[1].kind, MemoryKind::Reserved);
    assert_eq!(info.memory[2].kind, MemoryKind::Acpi);
    assert_eq!(info.cpu_freq_hz, 54_000_000);
    assert_eq!(info.cpu_id, 0x410f_d083);
    assert_eq!(info.cpu_count, 1);
});

arch_test!(init_boot_returns_shared_kernel, {
    let init = Init::new(LauncherArgs::default());
    let kernel = init.boot().expect("boot must succeed");
    // Verify abi field is accessible and has expected version.
    assert_eq!(kernel.abi.version().major, 1);
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
        let _ = Init::new(LauncherArgs::default()).boot();
        reovim_arch::alloc::fault::reset();
    }
});
