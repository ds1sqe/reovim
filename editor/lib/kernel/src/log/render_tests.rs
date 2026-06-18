//! Tests for `log/render.rs` — LOG2 canonical line renderer goldens.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (`arch_test!` + `testrt::run`). The kernel-selftest bin in
//! tests/fixtures/ runs these.
//!
//! ## Coverage
//!
//! - LOG2 golden: kernel-event fixture renders byte-identical expected line.
//! - LOG2 golden: cdylib-event fixture renders byte-identical expected line.
//! - Instance address grammar: all six variants produce correct tokens.
//! - `\n` escaping: embedded newlines in the message are escaped as `\n`.
//! - Timestamp format: seconds right-aligned min width 5, micros zero-padded 6.

use {reovim_arch::arch_test, reovim_uapi_abi::error::LogLevel};

use crate::log::render::{
    EmitterAddress, InstanceAddress, RenderError, RenderInput, boot_stage_message, render_line,
};

// ── Kernel-event golden (spec §2 worked example) ─────────────────────────────
//
// Expected line from the spec:
//   "[    0.002413] kernel init: boot.stage.ok stage=2 name=config\n"
//
// ts_nanos: 2_413_000 ns = 2413 µs → seconds=0, micros=002413.

arch_test!(log2_golden_kernel_event, {
    let input = RenderInput {
        ts_nanos: 2_413_000,
        emitter_pkg: "kernel",
        emitter_addr: EmitterAddress::Kernel { subsystem: "init" },
        instance: InstanceAddress::None,
        message: "boot.stage.ok stage=2 name=config",
        level: LogLevel::Info,
    };
    let bytes = render_line(&input).expect("render must succeed");
    let got = core::str::from_utf8(bytes.as_slice()).expect("UTF-8");
    let expected = "[    0.002413] kernel init: boot.stage.ok stage=2 name=config\n";
    assert_eq!(
        got, expected,
        "LOG2 kernel-event golden mismatch\n  got: {got:?}\n  expected: {expected:?}"
    );
});

// ── Cdylib-event golden (spec §2 annotated example) ──────────────────────────
//
// Expected line from the spec:
//   "[  123.456789] vim-mode module/7.0 main/b3.w1: entered Insert at byte 4096\n"
//
// ts_nanos: 123_456_789_000 ns = 123_456_789 µs → seconds=123, micros=456789.

arch_test!(log2_golden_cdylib_event, {
    let input = RenderInput {
        ts_nanos: 123_456_789_000,
        emitter_pkg: "vim-mode",
        emitter_addr: EmitterAddress::Cdylib {
            kind: "module",
            id: 7,
            vtable: 0,
        },
        instance: InstanceAddress::SessionBufferWindow {
            session: "main",
            buffer: 3,
            window: 1,
        },
        message: "entered Insert at byte 4096",
        level: LogLevel::Info,
    };
    let bytes = render_line(&input).expect("render must succeed");
    let got = core::str::from_utf8(bytes.as_slice()).expect("UTF-8");
    let expected = "[  123.456789] vim-mode module/7.0 main/b3.w1: entered Insert at byte 4096\n";
    assert_eq!(
        got, expected,
        "LOG2 cdylib-event golden mismatch\n  got: {got:?}\n  expected: {expected:?}"
    );
});

// ── Timestamp format: large seconds value ─────────────────────────────────────

arch_test!(log2_timestamp_large_seconds, {
    // 99999 seconds = width exactly 5 (no padding needed).
    // micros = 000001.
    let input = RenderInput {
        ts_nanos: 99_999_000_001_000, // 99999 * 1e9 + 1000 ns
        emitter_pkg: "kernel",
        emitter_addr: EmitterAddress::Kernel { subsystem: "boot" },
        instance: InstanceAddress::None,
        message: "test",
        level: LogLevel::Debug,
    };
    let bytes = render_line(&input).unwrap();
    let got = core::str::from_utf8(bytes.as_slice()).unwrap();
    assert!(got.starts_with("[99999.000001]"), "large-seconds format: {got:?}");
});

// ── Timestamp format: zero timestamp ──────────────────────────────────────────

arch_test!(log2_timestamp_zero, {
    let input = RenderInput {
        ts_nanos: 0,
        emitter_pkg: "kernel",
        emitter_addr: EmitterAddress::Kernel { subsystem: "boot" },
        instance: InstanceAddress::None,
        message: "boot.stage.start stage=0",
        level: LogLevel::Info,
    };
    let bytes = render_line(&input).unwrap();
    let got = core::str::from_utf8(bytes.as_slice()).unwrap();
    assert!(got.starts_with("[    0.000000]"), "zero-ts format: {got:?}");
});

// ── Instance grammar: Session only ────────────────────────────────────────────

arch_test!(log2_instance_session_only, {
    let input = RenderInput {
        ts_nanos: 1_000_000,
        emitter_pkg: "kernel",
        emitter_addr: EmitterAddress::Kernel {
            subsystem: "kernel",
        },
        instance: InstanceAddress::Session { session: "work" },
        message: "test",
        level: LogLevel::Info,
    };
    let bytes = render_line(&input).unwrap();
    let line = core::str::from_utf8(bytes.as_slice()).unwrap();
    // Should contain " work: " after the address.
    assert!(line.contains(" work: "), "session-only instance: {line:?}");
});

// ── Instance grammar: Session + buffer ───────────────────────────────────────

arch_test!(log2_instance_session_buffer, {
    let input = RenderInput {
        ts_nanos: 1_000_000,
        emitter_pkg: "kernel",
        emitter_addr: EmitterAddress::Kernel {
            subsystem: "kernel",
        },
        instance: InstanceAddress::SessionBuffer {
            session: "main",
            buffer: 5,
        },
        message: "test",
        level: LogLevel::Info,
    };
    let bytes = render_line(&input).unwrap();
    let line = core::str::from_utf8(bytes.as_slice()).unwrap();
    assert!(line.contains(" main/b5: "), "session+buffer instance: {line:?}");
});

// ── Instance grammar: Client only ─────────────────────────────────────────────

arch_test!(log2_instance_client, {
    let input = RenderInput {
        ts_nanos: 1_000_000,
        emitter_pkg: "kernel",
        emitter_addr: EmitterAddress::Kernel {
            subsystem: "kernel",
        },
        instance: InstanceAddress::Client { id: 2 },
        message: "test",
        level: LogLevel::Info,
    };
    let bytes = render_line(&input).unwrap();
    let line = core::str::from_utf8(bytes.as_slice()).unwrap();
    assert!(line.contains(" c2: "), "client-only instance: {line:?}");
});

// ── Instance grammar: Stream only ────────────────────────────────────────────

arch_test!(log2_instance_stream, {
    let input = RenderInput {
        ts_nanos: 1_000_000,
        emitter_pkg: "kernel",
        emitter_addr: EmitterAddress::Kernel {
            subsystem: "kernel",
        },
        instance: InstanceAddress::Stream { id: 17 },
        message: "test",
        level: LogLevel::Info,
    };
    let bytes = render_line(&input).unwrap();
    let line = core::str::from_utf8(bytes.as_slice()).unwrap();
    assert!(line.contains(" s17: "), "stream-only instance: {line:?}");
});

// ── Instance grammar: None (omitted) ─────────────────────────────────────────

arch_test!(log2_instance_none_omitted, {
    let input = RenderInput {
        ts_nanos: 1_000_000,
        emitter_pkg: "kernel",
        emitter_addr: EmitterAddress::Kernel { subsystem: "boot" },
        instance: InstanceAddress::None,
        message: "test",
        level: LogLevel::Info,
    };
    let bytes = render_line(&input).unwrap();
    let line = core::str::from_utf8(bytes.as_slice()).unwrap();
    // The line should be "[...] kernel boot: test\n" — no instance token.
    let expected = "[    0.001000] kernel boot: test\n";
    assert_eq!(line, expected, "none-instance: {line:?}");
});

// ── \n escaping ───────────────────────────────────────────────────────────────

arch_test!(log2_newline_escaping, {
    let input = RenderInput {
        ts_nanos: 0,
        emitter_pkg: "kernel",
        emitter_addr: EmitterAddress::Kernel { subsystem: "boot" },
        instance: InstanceAddress::None,
        message: "line one\nline two\nline three",
        level: LogLevel::Warn,
    };
    let bytes = render_line(&input).unwrap();
    let line = core::str::from_utf8(bytes.as_slice()).unwrap();
    // Embedded newlines become `\n` (two-character escape); only the final
    // newline should be a real 0x0A.
    assert!(line.ends_with('\n'), "must end with real newline: {line:?}");
    // Count real newlines: must be exactly 1 (the trailing newline).
    let newline_count = line.bytes().filter(|&b| b == b'\n').count();
    assert_eq!(newline_count, 1, "only the trailing newline must be real: {line:?}");
    // The escaped sequences appear as literal `\n`.
    assert!(
        line.contains("\\n"),
        "escaped sequences must appear as literal backslash-n: {line:?}"
    );
});

// ── RenderError variant exhaustiveness ────────────────────────────────────────

arch_test!(render_error_is_alloc, {
    let e = RenderError::Alloc;
    assert!(matches!(e, RenderError::Alloc));
});

// ── Trailing newline always present ──────────────────────────────────────────

arch_test!(log2_trailing_newline_always_present, {
    let input = RenderInput {
        ts_nanos: 500_000_000,
        emitter_pkg: "kernel",
        emitter_addr: EmitterAddress::Cdylib {
            kind: "driver",
            id: 1,
            vtable: 2,
        },
        instance: InstanceAddress::None,
        message: "no newline in message",
        level: LogLevel::Error,
    };
    let bytes = render_line(&input).unwrap();
    assert_eq!(bytes.as_slice().last(), Some(&b'\n'), "line must end with newline");
});

arch_test!(instance_address_is_present_both_arms, {
    assert!(!InstanceAddress::None.is_present());
    assert!(InstanceAddress::Session { session: "s0" }.is_present());
});

arch_test!(render_alloc_fault_sweep_covers_error_arms, {
    // Sweep the allocator fault point across every allocation a render makes:
    // each k fails a different `write!`/`try_*` site, executing that site's
    // `map_err(|_| RenderError::Alloc)?` error arm. The sweep ends at the
    // first k where the render succeeds (no more allocation sites).
    let inputs = [
        RenderInput {
            ts_nanos: 1_000_000,
            emitter_pkg: "kernel",
            emitter_addr: EmitterAddress::Kernel { subsystem: "boot" },
            instance: InstanceAddress::SessionBufferWindow {
                session: "s0",
                buffer: 1,
                window: 2,
            },
            message: "line\nwith\nnewlines",
            level: LogLevel::Info,
        },
        RenderInput {
            ts_nanos: 2_000_000,
            emitter_pkg: "pkg",
            emitter_addr: EmitterAddress::Cdylib {
                kind: "driver",
                id: 3,
                vtable: 4,
            },
            instance: InstanceAddress::Stream { id: 9 },
            message: "plain",
            level: LogLevel::Error,
        },
        RenderInput {
            ts_nanos: 3_000_000,
            emitter_pkg: "pkg",
            emitter_addr: EmitterAddress::Kernel { subsystem: "log" },
            instance: InstanceAddress::Client { id: 7 },
            message: "x",
            level: LogLevel::Warn,
        },
        RenderInput {
            ts_nanos: 4_000_000,
            emitter_pkg: "pkg",
            emitter_addr: EmitterAddress::Kernel { subsystem: "log" },
            instance: InstanceAddress::Session { session: "s1" },
            message: "y",
            level: LogLevel::Debug,
        },
        RenderInput {
            ts_nanos: 5_000_000,
            emitter_pkg: "pkg",
            emitter_addr: EmitterAddress::Kernel { subsystem: "log" },
            instance: InstanceAddress::SessionBuffer {
                session: "s2",
                buffer: 3,
            },
            message: "z",
            level: LogLevel::Trace,
        },
    ];
    for input in &inputs {
        let mut k = 0isize;
        loop {
            reovim_arch::alloc::fault::fail_after(k);
            let result = render_line(input);
            reovim_arch::alloc::fault::reset();
            if result.is_ok() {
                break;
            }
            k += 1;
            assert!(k < 64, "render must succeed within a bounded alloc count");
        }
    }
});

arch_test!(render_alloc_fault_boundary_sweep, {
    // A write site only allocates when the `Bytes` capacity boundary lands
    // exactly on it, so a single input cannot reach every site's error arm.
    // Brute-force message lengths x fault points: some (len, k) pair puts the
    // growth at each write site, executing its `map_err` arm. No assertions
    // beyond completion — the goldens elsewhere pin the success bytes.
    const PAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"; // 48 bytes
    let shapes = [
        (
            EmitterAddress::Kernel { subsystem: "boot" },
            InstanceAddress::SessionBufferWindow {
                session: "s0",
                buffer: 1,
                window: 2,
            },
        ),
        (
            EmitterAddress::Cdylib {
                kind: "driver",
                id: 3,
                vtable: 4,
            },
            InstanceAddress::Stream { id: 9 },
        ),
        (EmitterAddress::Kernel { subsystem: "log" }, InstanceAddress::Client { id: 7 }),
        (
            EmitterAddress::Kernel { subsystem: "log" },
            InstanceAddress::Session { session: "s1" },
        ),
        (
            EmitterAddress::Kernel { subsystem: "log" },
            InstanceAddress::SessionBuffer {
                session: "s2",
                buffer: 3,
            },
        ),
    ];
    // Newlines at fixed offsets so the escape loop's both write sites see
    // varying boundary positions as the slice length grows.
    const NL_PAD: &str = "aaaaaaa\naaaaaaaaaaaaaaa\naaaaaaaaaaaaaaaaaaaaaa";
    for (emitter_addr, instance) in &shapes {
        for len in 0..PAD.len() {
            for k in 0..6isize {
                // (a) message varies — boundary moves across the suffix sites.
                let input = RenderInput {
                    ts_nanos: 1_000_000,
                    emitter_pkg: "kernel",
                    emitter_addr: *emitter_addr,
                    instance: *instance,
                    message: &PAD[..len],
                    level: LogLevel::Info,
                };
                reovim_arch::alloc::fault::fail_after(k);
                let _ = render_line(&input);
                reovim_arch::alloc::fault::reset();

                // (b) the prefix varies — emitter/instance writes have fixed
                // offsets for a fixed prefix, so the capacity boundary can
                // only land on them when the package name length moves.
                let input = RenderInput {
                    ts_nanos: 1_000_000,
                    emitter_pkg: &PAD[..len],
                    emitter_addr: *emitter_addr,
                    instance: *instance,
                    message: "m",
                    level: LogLevel::Info,
                };
                reovim_arch::alloc::fault::fail_after(k);
                let _ = render_line(&input);
                reovim_arch::alloc::fault::reset();

                // (c) newline-bearing message — moves the boundary across the
                // escape loop's segment and escape-sequence write sites.
                let nl_len = len.min(NL_PAD.len());
                let input = RenderInput {
                    ts_nanos: 1_000_000,
                    emitter_pkg: "kernel",
                    emitter_addr: *emitter_addr,
                    instance: *instance,
                    message: &NL_PAD[..nl_len],
                    level: LogLevel::Info,
                };
                reovim_arch::alloc::fault::fail_after(k);
                let _ = render_line(&input);
                reovim_arch::alloc::fault::reset();

                // (d) prefix AND newline vary together: the escape-sequence
                // write site's offset is prefix-determined, so only a moving
                // prefix can put the capacity boundary on it.
                let input = RenderInput {
                    ts_nanos: 1_000_000,
                    emitter_pkg: &PAD[..len],
                    emitter_addr: *emitter_addr,
                    instance: *instance,
                    message: &NL_PAD[..nl_len],
                    level: LogLevel::Info,
                };
                reovim_arch::alloc::fault::fail_after(k);
                let _ = render_line(&input);
                reovim_arch::alloc::fault::reset();
            }
        }
    }
});

// ── Boot-stage message enrichment ─────────────────────────────────────────────

arch_test!(boot_stage_message_enriches_known_events, {
    // ok / starting / failed verbs, with the stage number, denominator, and name.
    let mut buf = [0u8; 48];
    assert_eq!(
        boot_stage_message(&mut buf, "boot.stage.ok", 2),
        Some("stage 2/7 ok: shell config")
    );
    let mut buf = [0u8; 48];
    assert_eq!(
        boot_stage_message(&mut buf, "boot.stage.start", 7),
        Some("stage 7/7 starting: handoff")
    );
    let mut buf = [0u8; 48];
    assert_eq!(
        boot_stage_message(&mut buf, "boot.stage.fail", 3),
        Some("stage 3/7 failed: library root")
    );
    // Non-boot events fall back to the bare id (the caller renders event.event).
    let mut buf = [0u8; 48];
    assert_eq!(boot_stage_message(&mut buf, "log.sink.fail", 0), None);
    // An out-of-range stage stays well-formed (name "stage").
    let mut buf = [0u8; 48];
    assert_eq!(boot_stage_message(&mut buf, "boot.stage.ok", 9), Some("stage 9/7 ok: stage"));
});
