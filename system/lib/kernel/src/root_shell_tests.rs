//! Selftests for the root daemon shell parser + command surface.
//!
//! The tests run in the no_std selftest harness and prove the shell can be
//! instantiated from static fixtures with a callback writer.

use {
    super::execute_root_command,
    crate::rootd::{PayloadDescriptor, PayloadLaunchResult, ProfileSummary, RootDaemon},
    core::cell::UnsafeCell,
    reovim_testrt::{self as testrt, arch_test},
    reovim_uapi_system::{BootInfo, DeviceClass, DeviceEntry},
};

struct StaticSink {
    buf: UnsafeCell<[u8; 512]>,
    len: UnsafeCell<usize>,
}

// SAFETY: selftest execution is single-threaded; tests never alias mutably across
// threads.
unsafe impl Sync for StaticSink {}

static SINK: StaticSink = StaticSink {
    buf: UnsafeCell::new([0u8; 512]),
    len: UnsafeCell::new(0),
};

fn sink_write(bytes: &[u8]) {
    // SAFETY: The runner is single-threaded and the sink is cleared between
    // test cases.
    unsafe {
        let buf = &mut *SINK.buf.get();
        let len = &mut *SINK.len.get();
        let mut i = 0usize;
        while i < bytes.len() && *len < buf.len() {
            buf[*len] = bytes[i];
            *len += 1;
            i += 1;
        }
    }
}

fn sink_clear() {
    // SAFETY: same single-threaded runner assumptions as `sink_write`.
    unsafe {
        *SINK.len.get() = 0;
        (*SINK.buf.get()).fill(0);
    }
}

fn sink_bytes() -> &'static [u8] {
    // SAFETY: output is only mutated by `sink_write` while the runner is
    // single-threaded; callers consume it synchronously inside the same test.
    unsafe {
        let len = *SINK.len.get();
        let ptr = SINK.buf.get() as *const [u8; 512];
        let buf = &*ptr;
        &buf[..len]
    }
}

fn sink_str() -> &'static str {
    // SAFETY: tests only emit ASCII diagnostics in this module.
    unsafe { core::str::from_utf8_unchecked(sink_bytes()) }
}

fn run_command(profile: ProfileSummary, line: &[u8], dmesg: Option<fn() -> &'static str>) {
    sink_clear();
    let daemon = RootDaemon::new(
        profile,
        sample_boot_info(),
        sample_devices(),
        sample_payloads(),
        dmesg,
        None,
        "reovim-os> ",
        sink_write,
    );
    let _ = execute_root_command(&daemon, line);
}

const SAMPLE_DEVICES: [DeviceEntry; 1] = [DeviceEntry {
    class: DeviceClass::Uart,
    mmio_base: 0x1000,
    mmio_len: 0x100,
    irq: 12,
    capacity_bytes: 0,
    compatible: "arm,pl011",
}];

const SAMPLE_PAYLOADS: [PayloadDescriptor; 2] = [
    PayloadDescriptor {
        name: "editor-smoke",
        summary: "editor smoke payload",
        launch: Some(launch_editor),
    },
    PayloadDescriptor {
        name: "server-smoke",
        summary: "server smoke payload",
        launch: Some(launch_server),
    },
];

fn sample_boot_info() -> BootInfo {
    BootInfo {
        memory: reovim_uapi_system::MemorySummary::new(1, 0x1000),
        cpu_count: 2,
        ..BootInfo::default()
    }
}

fn sample_devices() -> &'static [DeviceEntry] {
    &SAMPLE_DEVICES
}

fn sample_payloads() -> &'static [PayloadDescriptor] {
    &SAMPLE_PAYLOADS
}

fn launch_editor() -> PayloadLaunchResult {
    PayloadLaunchResult::Ready
}

fn launch_server() -> PayloadLaunchResult {
    PayloadLaunchResult::Failed
}

fn diagnostics() -> &'static str {
    "boot diagnostics complete"
}

fn assert_contains(haystack: &[u8], needle: &[u8]) {
    let mut i = 0usize;
    while i + needle.len() <= haystack.len() {
        let mut match_len = 0usize;
        while match_len < needle.len() && haystack[i + match_len] == needle[match_len] {
            match_len += 1;
        }
        if match_len == needle.len() {
            return;
        }
        i += 1;
    }
    testrt::check(false, "expected output chunk");
}

arch_test!(root_shell_help, {
    run_command(ProfileSummary::new("shell-only", false), b"help\n", None);
    testrt::check_eq(
        sink_str(),
        "reovim root shell\ncommands: help, device, dmesg, launch, halt\nreserved: ls, cd, pwd, cat, mount, reovim\n",
    );
});

arch_test!(root_shell_launch_and_reserved, {
    run_command(ProfileSummary::new("appliance", true), b"launch\n", None);
    assert_contains(
        sink_bytes(),
        b"launch: available payloads:\n  editor-smoke: editor smoke payload\n  server-smoke: server smoke payload\n",
    );

    run_command(ProfileSummary::new("appliance", true), b"launch editor-smoke\n", None);
    testrt::check_eq(sink_str(), "launch editor-smoke: payload.ready\n");

    run_command(ProfileSummary::new("shell-only", false), b"launch editor-smoke\n", None);
    testrt::check_eq(sink_str(), "launch disabled for this profile\n");

    run_command(ProfileSummary::new("appliance", true), b"ls\n", None);
    testrt::check_eq(sink_str(), "reserved: supported after VFS/current-directory rollout\n");
});

arch_test!(root_shell_dmesg_and_unknown_command, {
    run_command(ProfileSummary::new("shell-only", false), b"dmesg\n", None);
    testrt::check_eq(sink_str(), "dmesg: (no diagnostics source)\n");

    run_command(ProfileSummary::new("shell-only", false), b"dmesg\n", Some(diagnostics));
    testrt::check_eq(sink_str(), "dmesg:\nboot diagnostics complete\n");

    run_command(ProfileSummary::new("shell-only", false), b"does-not-exist\n", None);
    testrt::check_eq(sink_str(), "error: unknown command, try `help`\n");
});
