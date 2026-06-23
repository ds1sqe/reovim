//! Tests for `editor_core.rs` — `EditorCore`, `EditorCoreAbi` coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (arch_test! + testrt::run). The editor-core-selftest bin in
//! tests/fixtures/ runs these.

use {reovim_arch::arch_test, reovim_uapi::sched::ClockControl};

use crate::{
    EditorInit, LauncherArgs,
    root::{EDITOR_CORE_ABI_VERSION, EditorCoreAbi},
};

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

arch_test!(editor_core_abi_version_constant_major_1, {
    assert_eq!(EDITOR_CORE_ABI_VERSION.major, 1);
    assert_eq!(EDITOR_CORE_ABI_VERSION.minor, 0);
    assert_eq!(EDITOR_CORE_ABI_VERSION.patch, 0);
});

arch_test!(editor_core_abi_new_stores_version, {
    let abi = EditorCoreAbi::new(EDITOR_CORE_ABI_VERSION);
    assert_eq!(abi.version(), EDITOR_CORE_ABI_VERSION);
});

arch_test!(editor_core_boot_anchor_field_accessible, {
    let editor_core = EditorInit::new(launcher_args_with_test_clock())
        .boot()
        .expect("boot succeeds");
    // Boot anchor wall clock is after year 2000 (Unix-epoch nanos).
    assert!(editor_core.boot_anchor.wall_anchor > 946_684_800_000_000_000);
});

arch_test!(editor_core_abi_field_version_is_current, {
    let editor_core = EditorInit::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");
    assert_eq!(editor_core.abi.version(), EDITOR_CORE_ABI_VERSION);
});

arch_test!(editor_core_shared_strong_count_is_1_at_boot, {
    let editor_core = EditorInit::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");
    // Shared<EditorCore> wrapping — each Shared has its own count.
    assert_eq!(editor_core.abi.strong_count(), 1);
});

arch_test!(editor_core_event_bus_field_accessible_after_boot, {
    let editor_core = EditorInit::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");
    // Shared<DS12EventBus>: strong count must be 1 immediately after boot.
    assert_eq!(
        editor_core.event_bus.strong_count(),
        1,
        "event_bus Shared strong count must be 1 at handoff"
    );
});

arch_test!(editor_core_state_substrate_field_accessible_after_boot, {
    let editor_core = EditorInit::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");
    let guard = editor_core.state.read();
    assert_eq!(guard.buffer_count(), 0);
    assert!(guard.limits().max_slots_per_window > 0);
});

arch_test!(editor_core_event_bus_emits_after_boot, {
    use {
        crate::{
            BootClock,
            event_bus::{BootStageFields, DS12Event, EVT_BOOT_STAGE_START},
        },
        core::sync::atomic::{AtomicUsize, Ordering},
        reovim_uapi::abi::error::LogLevel,
    };

    static POST_BOOT_COUNT: AtomicUsize = AtomicUsize::new(0);
    fn post_boot_counter(_e: &DS12Event) {
        POST_BOOT_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    POST_BOOT_COUNT.store(0, Ordering::Relaxed);

    let editor_core = EditorInit::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");

    // Subscribe to the bus post-boot and emit a test event.
    editor_core.event_bus.subscribe(post_boot_counter).unwrap();

    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    editor_core.event_bus.emit(&DS12Event {
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
