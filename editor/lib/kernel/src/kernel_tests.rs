//! Tests for `kernel.rs` — `Kernel`, `KernelAbi` coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (arch_test! + testrt::run). The kernel-selftest bin in
//! tests/fixtures/ runs these.

use reovim_arch::arch_test;

use crate::{
    Init, LauncherArgs,
    kernel::{KERNEL_ABI_VERSION, KernelAbi},
};

arch_test!(kernel_abi_version_constant_major_1, {
    assert_eq!(KERNEL_ABI_VERSION.major, 1);
    assert_eq!(KERNEL_ABI_VERSION.minor, 0);
    assert_eq!(KERNEL_ABI_VERSION.patch, 0);
});

arch_test!(kernel_abi_new_stores_version, {
    let abi = KernelAbi::new(KERNEL_ABI_VERSION);
    assert_eq!(abi.version(), KERNEL_ABI_VERSION);
});

arch_test!(kernel_boot_anchor_field_accessible, {
    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");
    // Boot anchor wall clock is after year 2000.
    assert!(kernel.boot_anchor.wall_anchor.tv_sec > 946_684_800);
});

arch_test!(kernel_abi_field_version_is_current, {
    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");
    assert_eq!(kernel.abi.version(), KERNEL_ABI_VERSION);
});

arch_test!(kernel_shared_strong_count_is_1_at_boot, {
    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");
    // Shared<Kernel> wrapping — each Shared has its own count.
    assert_eq!(kernel.abi.strong_count(), 1);
});

arch_test!(kernel_event_bus_field_accessible_after_boot, {
    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");
    // Shared<DS12EventBus>: strong count must be 1 immediately after boot.
    assert_eq!(
        kernel.event_bus.strong_count(),
        1,
        "event_bus Shared strong count must be 1 at handoff"
    );
});

arch_test!(kernel_state_substrate_field_accessible_after_boot, {
    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");
    let guard = kernel.state.read();
    assert_eq!(guard.buffer_count(), 0);
    assert!(guard.limits().max_slots_per_window > 0);
});

arch_test!(kernel_event_bus_emits_after_boot, {
    use {
        crate::{
            BootClock,
            event_bus::{BootStageFields, DS12Event, EVT_BOOT_STAGE_START},
        },
        core::sync::atomic::{AtomicUsize, Ordering},
        reovim_uapi_abi::error::LogLevel,
    };

    static POST_BOOT_COUNT: AtomicUsize = AtomicUsize::new(0);
    fn post_boot_counter(_e: &DS12Event) {
        POST_BOOT_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    POST_BOOT_COUNT.store(0, Ordering::Relaxed);

    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");

    // Subscribe to the bus post-boot and emit a test event.
    kernel.event_bus.subscribe(post_boot_counter).unwrap();

    let clock = BootClock::capture();
    kernel.event_bus.emit(&DS12Event {
        ts_nanos: clock.elapsed_nanos(),
        level: LogLevel::Info,
        event: EVT_BOOT_STAGE_START,
        fields: BootStageFields {
            stage: 0,
            error_code: None,
        },
    });

    assert_eq!(
        POST_BOOT_COUNT.load(Ordering::Relaxed),
        1,
        "post-boot emit must reach subscriber"
    );
});
