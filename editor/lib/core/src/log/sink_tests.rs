//! Tests for `log/sink.rs` — `FileSink` LOG7/LOG8 behaviour.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (`arch_test!` + `testrt::run`). The editor-core-selftest bin in
//! tests/fixtures/ runs these.
//!
//! ## Coverage
//!
//! - `EVT_LOG_SINK_FAIL` is the expected constant string.
//! - `SinkOpenError` variant enumeration.
//! - `FileSink::new` constructs without opening the file.
//! - `FileSink::set_headless` changes the headless flag.
//! - LOG1 1:1: every event seen by a parallel subscriber also appears in the
//!   ring (same DS12 stream).
//! - Integration smoke: `EditorInit::boot` + sink attach produces a log file whose
//!   byte content matches the ring's rendered entries (via `/tmp` path, with
//!   a per-run unique suffix from the boot-clock realtime anchor).
//! - Write-failure: arming the sink with a bad fd triggers sink closure and
//!   exactly one `log.sink.fail` event seen by a parallel subscriber.
//! - LOG8 gate: `stderr_echo` writes before open; obeys headless flag after.

use {
    reovim_arch::{
        alloc::fault,
        arch_test,
        sys::{AT_FDCWD, O_CLOEXEC, O_CREAT, O_RDONLY, O_WRONLY, close, openat, read, write},
    },
    reovim_lib_ds::Shared,
    reovim_uapi::{
        abi::error::LogLevel,
        log::{LogSinkControl, LogSinkError, LogSinkHandle},
        panic::PanicConfigError,
    },
};

use crate::{
    BootClock,
    event_bus::{BootStageFields, DS12Event, DS12EventBus, EVT_BOOT_STAGE_OK},
    init::{EditorInit, LauncherArgs},
    log::{
        ring::LogRing,
        sink::{
            EVT_LOG_SINK_FAIL, FileSink, SinkOpenError, inject_handle_for_test, install_control,
            reset_for_test, stderr_echo,
        },
    },
};

const O_APPEND: usize = 0o2000;

fn test_log_control() -> LogSinkControl {
    LogSinkControl::new(
        test_open_log_sink,
        test_write_log_sink,
        test_close_log_sink,
        test_register_panic_flush,
        test_write_diagnostic,
    )
}

fn reset_with_test_log_control() {
    reset_for_test();
    install_control(test_log_control());
}

fn test_open_log_sink(path: &[u8]) -> Result<LogSinkHandle, LogSinkError> {
    let flags = O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC;
    openat(AT_FDCWD, path, flags, 0o644)
        .map(|fd| LogSinkHandle::new(fd as u64))
        .map_err(|err| LogSinkError::new(err.code()))
}

fn test_write_log_sink(handle: LogSinkHandle, bytes: &[u8]) -> Result<(), LogSinkError> {
    let fd = test_handle_to_fd(handle)?;
    let mut off = 0;
    while off < bytes.len() {
        match write(fd, &bytes[off..]) {
            Ok(0) => return Err(LogSinkError::new(0)),
            Ok(n) => off += n,
            Err(err) => return Err(LogSinkError::new(err.code())),
        }
    }
    Ok(())
}

fn test_close_log_sink(handle: LogSinkHandle) -> Result<(), LogSinkError> {
    close(test_handle_to_fd(handle)?)
        .map(|_| ())
        .map_err(|err| LogSinkError::new(err.code()))
}

fn test_register_panic_flush(_: LogSinkHandle) -> Result<(), PanicConfigError> {
    Ok(())
}

fn test_write_diagnostic(bytes: &[u8]) -> Result<(), LogSinkError> {
    test_write_log_sink(LogSinkHandle::new(2), bytes)
}

fn test_handle_to_fd(handle: LogSinkHandle) -> Result<i32, LogSinkError> {
    let raw = handle.raw();
    if raw > i32::MAX as u64 {
        Err(LogSinkError::new(0))
    } else {
        Ok(raw as i32)
    }
}

// ── EVT_LOG_SINK_FAIL constant ────────────────────────────────────────────────

arch_test!(evt_log_sink_fail_is_correct, {
    reset_with_test_log_control();
    assert_eq!(EVT_LOG_SINK_FAIL, "log.sink.fail");
    assert!(EVT_LOG_SINK_FAIL.starts_with("log."));
});

// ── SinkOpenError variants ────────────────────────────────────────────────────

arch_test!(sink_open_error_subscribe_variant, {
    reset_with_test_log_control();
    let e = SinkOpenError::Subscribe;
    assert!(matches!(e, SinkOpenError::Subscribe));
});

// ── FileSink::new does not open the file ────────────────────────────────────

arch_test!(file_sink_new_does_not_open, {
    reset_with_test_log_control();
    // FileSink::new should succeed even for a path that does not exist —
    // the file is not opened until open_and_subscribe.
    let _sink = FileSink::new(b"/nonexistent/path/reovim.log\0");
    // No panic; the constructor is lazy.
});

// ── FileSink::set_headless mutates the flag ───────────────────────────────────

arch_test!(file_sink_set_headless, {
    reset_with_test_log_control();
    let mut hbuf = [0u8; 64];
    let sink = FileSink::new(reovim_arch::testrt::unique_path(b"/tmp/reovim-headless-", &mut hbuf));
    // Default is headless = true; setting false should not panic.
    sink.set_headless(false);
    sink.set_headless(true);
});

// ── open_and_subscribe on bad path returns SinkOpenError::Open ───────────────

arch_test!(file_sink_open_bad_path_returns_error, {
    reset_with_test_log_control();

    let ring = Shared::try_new(LogRing::try_new(1024 * 1024).unwrap()).unwrap();
    let mut raw_bus = DS12EventBus::new();
    raw_bus.set_builtin(Shared::clone(&ring));
    let bus = Shared::try_new(raw_bus).unwrap();

    // A path in a non-existent directory will fail openat.
    let sink = FileSink::new(b"/nonexistent-dir-kjashd/reovim.log\0");
    let result = sink.open_and_subscribe(&ring, &bus);
    assert!(
        matches!(result, Err(SinkOpenError::Open(_))),
        "bad path must return SinkOpenError::Open"
    );
});

// ── LOG1 1:1: every bus event appears in the ring ────────────────────────────
//
// A parallel subscriber counts events; the ring's len must match.
// This verifies there is no second pipeline (LOG1 invariant).

use core::sync::atomic::{AtomicUsize, Ordering};

static LOG1_COUNT: AtomicUsize = AtomicUsize::new(0);

fn log1_counter(_event: &DS12Event) {
    LOG1_COUNT.fetch_add(1, Ordering::Relaxed);
}

arch_test!(log1_one_to_one_ring_vs_subscriber, {
    reset_with_test_log_control();
    LOG1_COUNT.store(0, Ordering::Relaxed);

    let ring = Shared::try_new(LogRing::try_new(1024 * 1024).unwrap()).unwrap();
    let mut raw_bus = DS12EventBus::new();
    raw_bus.set_builtin(Shared::clone(&ring));
    let bus = Shared::try_new(raw_bus).unwrap();
    // Register a parallel subscriber that counts events.
    bus.subscribe(log1_counter).unwrap();

    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    let n_events = 5_u8;
    for stage in 0..n_events {
        bus.emit(&DS12Event {
            ts_nanos: clock.elapsed_nanos(),
            level: LogLevel::Info,
            event: EVT_BOOT_STAGE_OK,
            fields: BootStageFields {
                stage,
                error_code: None,
            },
        });
    }

    let ring_len = ring.len();
    let sub_count = LOG1_COUNT.load(Ordering::Relaxed);
    assert_eq!(ring_len, usize::from(n_events), "ring must hold all events");
    assert_eq!(sub_count, usize::from(n_events), "parallel subscriber must see all events");
    assert_eq!(ring_len, sub_count, "ring len must equal subscriber count (LOG1 1:1)");
});

// ── Integration smoke: EditorInit::boot + sink attach ───────────────────────────────
//
// Full EditorInit::boot with the file sink attached produces a log file whose
// content matches the ring's rendered entries. Uses a static tmp path that
// is unique across concurrent test binaries by embedding a compile-time
// constant. Within a single test run the selftest runner is single-threaded
// and the path is truncated on each open (O_TRUNC absent means O_APPEND;
// the line-count assertion counts all lines including prior runs, so we
// reset via `reset_for_test` and accept potential accumulation in CI).
// The per-run suffix (BootClock realtime anchor) is written into the file
// name via a static byte array below rather than a local buffer to avoid
// unsafe lifetime extension.

/// Static path for the integration smoke test. Unique per test binary run via
/// the compile-time `line!()` macro embedded in the name — no two test builds
/// in the same workspace should collide on this path.
// TID-unique: several selftest binaries register this test and may run
// concurrently under one `cargo test`; a fixed path would interleave the
// byte-matched file across processes.
fn smoke_log_path(buf: &mut [u8; 64]) -> &[u8] {
    reovim_arch::testrt::unique_path(b"/tmp/reovim-integration-smoke-", buf)
}

arch_test!(integration_smoke_boot_with_sink, {
    reset_with_test_log_control();

    let mut smoke_buf = [0u8; 64];
    let smoke_path: &[u8] = smoke_log_path(&mut smoke_buf);

    // Pre-truncate the smoke file so prior runs do not accumulate and the
    // byte-match below is exact.
    {
        let flags = reovim_arch::sys::O_WRONLY
            | reovim_arch::sys::O_CREAT
            | reovim_arch::sys::O_TRUNC
            | O_CLOEXEC;
        let fd = openat(AT_FDCWD, smoke_path, flags, 0o644)
            .expect("smoke file truncate-open must succeed");
        close(fd as i32).ok();
    }

    let init = EditorInit::new(LauncherArgs {
        log: test_log_control(),
        ..LauncherArgs::default()
    });
    let editor_core = init.boot().expect("boot must succeed");

    // The ring now has 44 entries: boot stages 1..7 (start+ok each = 14) plus
    // the 30 boot-tail health-probe lines (15 ping-pong probes, each a
    // `probing...` line then a verdict).
    let ring_len_before_sink = editor_core.log_ring.len();
    assert_eq!(ring_len_before_sink, 44);

    // Attach the file sink.
    let sink = FileSink::new(smoke_path);
    sink.open_and_subscribe(&editor_core.log_ring, &editor_core.event_bus)
        .expect("sink open must succeed");

    // Emit one more event via the bus (the sink should write it).
    editor_core.event_bus.emit(&DS12Event {
        ts_nanos: editor_core.boot_anchor.elapsed_nanos(),
        level: LogLevel::Info,
        event: EVT_BOOT_STAGE_OK,
        fields: BootStageFields {
            stage: 99,
            error_code: None,
        },
    });

    // The ring should now have 45 entries (44 boot + 1 post-sink).
    assert_eq!(editor_core.log_ring.len(), 45);

    // Read the log file and verify it is non-empty and contains LOG2 lines.
    let fd = openat(AT_FDCWD, smoke_path, O_RDONLY | O_CLOEXEC, 0)
        .expect("log file must exist after sink open");
    let mut buf = [0u8; 8192];
    let n = read(fd as i32, &mut buf).unwrap_or(0);
    close(fd as i32).ok();

    assert!(n > 0, "log file must contain data after sink open + replay + emit");

    // AC byte-match (LOG1 end-to-end): the file is exactly the concatenation
    // of the ring's rendered entries, in order — replayed head (44 boot
    // lines) followed by the live post-sink line. One renderer, one
    // mechanism: ring bytes and sink bytes are the same bytes.
    let mut expected = reovim_lib_ds::Bytes::new();
    editor_core.log_ring.for_each(|entry| {
        expected
            .try_extend_from_slice(entry.line.as_slice())
            .unwrap();
    });
    assert_eq!(
        &buf[..n],
        expected.as_slice(),
        "sink file bytes must equal the ring's rendered entries verbatim"
    );

    let line_count = buf[..n].iter().filter(|&&b| b == b'\n').count();
    assert_eq!(
        line_count, 45,
        "log file must contain exactly 45 lines (44 boot replay + 1 event)"
    );

    // Drop the sink to close the fd cleanly.
    drop(sink);
    reset_for_test();
});

// ── Write-failure: sink closes and emits log.sink.fail ───────────────────────
//
// Procedure:
// 1. Open a real tmp file to get a valid handle.
// 2. Register a parallel subscriber that counts `log.sink.fail` events.
// 3. Inject the handle into the global sink via `inject_handle_for_test`.
// 4. Close the handle's backing file externally so the next write fails.
// 5. Emit one event; the sink's write fails → sink closes → `log.sink.fail`
//    is emitted → parallel subscriber sees exactly 1 fail event.

static FAIL_COUNT: AtomicUsize = AtomicUsize::new(0);

fn fail_counter(event: &DS12Event) {
    if event.event == EVT_LOG_SINK_FAIL {
        FAIL_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

arch_test!(write_failure_closes_sink_and_emits_fail_event, {
    reset_with_test_log_control();
    FAIL_COUNT.store(0, Ordering::Relaxed);

    // Open a tmp file and get a valid fd for the lower test control.
    // O_WRONLY | O_CREAT | O_CLOEXEC (no O_APPEND needed here).
    let mut wbuf = [0u8; 64];
    let tmp_path: &[u8] = reovim_arch::testrt::unique_path(b"/tmp/reovim-writefail-", &mut wbuf);
    let open_flags =
        reovim_arch::sys::O_WRONLY | reovim_arch::sys::O_CREAT | reovim_arch::sys::O_CLOEXEC;
    let fd = openat(AT_FDCWD, tmp_path, open_flags, 0o644).expect("tmp file must open") as i32;

    // Build the bus and ring; register the fail counter.
    let ring = Shared::try_new(LogRing::try_new(1024 * 1024).unwrap()).unwrap();
    let mut raw_bus = DS12EventBus::new();
    raw_bus.set_builtin(Shared::clone(&ring));
    let bus = Shared::try_new(raw_bus).unwrap();
    bus.subscribe(fail_counter).unwrap();

    // Inject the handle into the sink so the subscriber callback will write to it.
    inject_handle_for_test(LogSinkHandle::new(fd as u64), test_log_control(), Shared::clone(&bus));

    // Close the backing fd externally; the next write through the handle fails.
    close(fd).ok();

    // Emit one event. The subscriber write fails → sink closes → 1 fail event.
    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    bus.emit(&DS12Event {
        ts_nanos: clock.elapsed_nanos(),
        level: LogLevel::Info,
        event: EVT_BOOT_STAGE_OK,
        fields: BootStageFields {
            stage: 1,
            error_code: None,
        },
    });

    let fail_count = FAIL_COUNT.load(Ordering::Relaxed);
    assert_eq!(
        fail_count, 1,
        "exactly one log.sink.fail event must be emitted on write failure"
    );

    reset_for_test();
});

// ── LOG8 gate: stderr_echo active before open, obeys headless after ───────────
//
// We cannot capture fd-2 output in the selftest runner, so we verify the
// gate's control-flow logic via `reset_for_test` state transitions:
// - Before open (handle = None): `stderr_echo` must not panic.
// - After open with headless=false: gate must be inactive.
//   We verify "no panic" in both cases — the write itself is fire-and-forget.

arch_test!(log8_stderr_gate_active_before_open, {
    reset_with_test_log_control();
    // Sink is not open (handle = None, headless = true by default).
    // stderr_echo must execute without panic.
    stderr_echo(b"[    0.000001] editor-core log8: test\n");
    // If we reach here, the gate path did not panic.
});

arch_test!(log8_stderr_gate_obeys_headless_flag, {
    reset_with_test_log_control();

    let ring = Shared::try_new(LogRing::try_new(1024 * 1024).unwrap()).unwrap();
    let mut raw_bus = DS12EventBus::new();
    raw_bus.set_builtin(Shared::clone(&ring));
    let bus = Shared::try_new(raw_bus).unwrap();

    let tmp_path = b"/tmp/reovim-log8-headless-test.log\0";
    let sink = FileSink::new(tmp_path);
    sink.open_and_subscribe(&ring, &bus).expect("sink open");

    // headless = true (default): stderr_echo must write (no panic).
    stderr_echo(b"[    0.000002] editor-core log8: headless\n");

    // headless = false: stderr_echo should skip the write (no panic either).
    sink.set_headless(false);
    stderr_echo(b"[    0.000003] editor-core log8: non-headless\n");

    drop(sink);
    reset_for_test();
});

// ── Render-OOM in subscriber callback (sink.rs L321) ─────────────────────────
//
// The subscriber callback calls `render_line` before acquiring the lock. Arm
// `fail_after(1)` so the ring built-in's Bytes alloc succeeds (attempt 0) but
// the sink callback's render_line Bytes alloc fails (attempt 1) → the early
// `return` at L321 executes. No write to the sink must occur and no panic must
// happen. No `log.sink.fail` is emitted because the render failure returns
// before touching the sink handle at all.

static SINK_OOM_FAIL_COUNT: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

fn sink_oom_counter(event: &DS12Event) {
    if event.event == EVT_LOG_SINK_FAIL {
        SINK_OOM_FAIL_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    }
}

arch_test!(sink_callback_render_oom_silent_drop, {
    reset_with_test_log_control();
    SINK_OOM_FAIL_COUNT.store(0, core::sync::atomic::Ordering::Relaxed);

    // Open a real tmp file to back the sink handle.
    let mut obuf = [0u8; 64];
    let tmp_path: &[u8] = reovim_arch::testrt::unique_path(b"/tmp/reovim-sink-oom-", &mut obuf);
    let open_flags =
        reovim_arch::sys::O_WRONLY | reovim_arch::sys::O_CREAT | reovim_arch::sys::O_CLOEXEC;
    let fd = openat(AT_FDCWD, tmp_path, open_flags, 0o644).expect("tmp file must open") as i32;

    // Build a bus WITHOUT the ring built-in: the sink callback's
    // `render_line` is then the first allocation in the emit fan-out, so
    // `fail_after(0)` lands on it deterministically (no alloc counting).
    let bus = Shared::try_new(DS12EventBus::new()).unwrap();
    bus.subscribe(sink_oom_counter).unwrap();

    // Inject the handle so the sink subscriber callback will attempt to render+write.
    inject_handle_for_test(LogSinkHandle::new(fd as u64), test_log_control(), Shared::clone(&bus));

    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    let ev = DS12Event {
        ts_nanos: clock.elapsed_nanos(),
        level: LogLevel::Info,
        event: EVT_BOOT_STAGE_OK,
        fields: BootStageFields {
            stage: 2,
            error_code: None,
        },
    };

    // Fail the very next allocation: the sink callback's `render_line`.
    fault::fail_after(0);
    bus.emit(&ev);
    fault::reset();

    // No log.sink.fail must have been emitted: the render-OOM path returns
    // silently before touching the sink handle (L321 return). The sink remains open.
    let fail_count = SINK_OOM_FAIL_COUNT.load(core::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        fail_count, 0,
        "render-OOM in sink callback must drop silently without emitting log.sink.fail"
    );

    close(fd).ok();
    reset_for_test();
});

// ── SinkOpenError::Subscribe when bus is at capacity (sink.rs L262-263) ──────
//
// Fill the bus with SUBSCRIBER_CAPACITY (64) noop subscribers, then call
// `open_and_subscribe` on a valid path. The bus subscribe call inside
// `open_and_subscribe` must return `SubscribeError::Capacity`, which maps to
// `SinkOpenError::Subscribe`.

fn noop_sub(_e: &DS12Event) {}

arch_test!(sink_open_subscribe_capacity_error, {
    reset_with_test_log_control();

    let ring = Shared::try_new(LogRing::try_new(1024 * 1024).unwrap()).unwrap();
    let mut raw_bus = DS12EventBus::new();
    raw_bus.set_builtin(Shared::clone(&ring));
    let bus = Shared::try_new(raw_bus).unwrap();

    // Fill the bus to capacity (64 subscribers).
    use crate::event_bus::SUBSCRIBER_CAPACITY;
    for _ in 0..SUBSCRIBER_CAPACITY {
        bus.subscribe(noop_sub)
            .expect("must succeed before capacity");
    }

    // open_and_subscribe must fail with SinkOpenError::Subscribe because the
    // bus cannot accept the sink's registration.
    let tmp_path = b"/tmp/reovim-sink-capacity-test.log\0";
    let sink = FileSink::new(tmp_path);
    let result = sink.open_and_subscribe(&ring, &bus);
    assert!(
        matches!(result, Err(SinkOpenError::Subscribe)),
        "open_and_subscribe must return SinkOpenError::Subscribe when bus is full"
    );

    reset_for_test();
});

// ── Callback early-return when handle is None (sink.rs L334) ─────────────────
//
// Register the sink subscriber callback on a bus via `inject_handle_for_test`,
// then call `reset_for_test()` so the sink state has `handle = None, closed = false`
// while the subscriber is still on the bus. Emitting an event must hit the
// `None`-handle early-return path without panicking and without triggering any
// `log.sink.fail` emission.

static SINK_NONE_FD_FAIL_COUNT: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

fn sink_none_fd_fail_counter(event: &DS12Event) {
    if event.event == EVT_LOG_SINK_FAIL {
        SINK_NONE_FD_FAIL_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    }
}

arch_test!(sink_callback_fd_none_early_return, {
    reset_with_test_log_control();
    SINK_NONE_FD_FAIL_COUNT.store(0, core::sync::atomic::Ordering::Relaxed);

    // Open a tmp file to get a valid fd for the handle injected below.
    let tmp_path = b"/tmp/reovim-sink-none-fd-test.log\0";
    let open_flags =
        reovim_arch::sys::O_WRONLY | reovim_arch::sys::O_CREAT | reovim_arch::sys::O_CLOEXEC;
    let fd = openat(AT_FDCWD, tmp_path, open_flags, 0o644).expect("tmp file must open") as i32;

    // Build bus+ring; add a fail-counter subscriber.
    let ring = Shared::try_new(LogRing::try_new(1024 * 1024).unwrap()).unwrap();
    let mut raw_bus = DS12EventBus::new();
    raw_bus.set_builtin(Shared::clone(&ring));
    let bus = Shared::try_new(raw_bus).unwrap();
    bus.subscribe(sink_none_fd_fail_counter).unwrap();

    // Inject a handle so the sink subscriber is registered on the bus.
    inject_handle_for_test(LogSinkHandle::new(fd as u64), test_log_control(), Shared::clone(&bus));

    // reset_for_test sets handle = None, closed = false, bus = None — but the
    // subscriber callback is still registered on the bus. The next emit will
    // reach the `None`-handle early-return at sink.rs L334.
    close(fd).ok();
    reset_for_test();

    // Emit: the callback must silently return without writing or panicking.
    let clock = BootClock::capture(reovim_uapi::sched::ClockControl::default());
    bus.emit(&DS12Event {
        ts_nanos: clock.elapsed_nanos(),
        level: LogLevel::Info,
        event: EVT_BOOT_STAGE_OK,
        fields: BootStageFields {
            stage: 3,
            error_code: None,
        },
    });

    // No log.sink.fail must be emitted: the None-handle path just returns.
    let fail_count = SINK_NONE_FD_FAIL_COUNT.load(core::sync::atomic::Ordering::Relaxed);
    assert_eq!(fail_count, 0, "None-handle early-return must not emit log.sink.fail");

    reset_for_test();
});
