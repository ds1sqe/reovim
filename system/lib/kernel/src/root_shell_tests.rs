//! Selftests for the root daemon shell parser + command surface.
//!
//! The tests run in the no_std selftest harness and prove the shell can be
//! instantiated from static fixtures with a callback writer.

use {
    super::{RootShellSession, execute_root_command},
    crate::rootd::{
        BootCheckState, ConsoleInputStatus, ConsoleInputSummary, HardwareProbeResult,
        PayloadDescriptor, PayloadLaunchResult, ProfileSummary, RootDaemon, WriteFn,
    },
    core::cell::UnsafeCell,
    reovim_testrt::{self as testrt, arch_test},
    reovim_uapi_system::{BootInfo, DeviceClass, DeviceEntry},
};

const SINK_CAPACITY: usize = 4096;

struct StaticSink {
    buf: UnsafeCell<[u8; SINK_CAPACITY]>,
    len: UnsafeCell<usize>,
}

// SAFETY: selftest execution is single-threaded; tests never alias mutably across
// threads.
unsafe impl Sync for StaticSink {}

static SINK: StaticSink = StaticSink {
    buf: UnsafeCell::new([0u8; SINK_CAPACITY]),
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
        let ptr = SINK.buf.get() as *const [u8; SINK_CAPACITY];
        let buf = &*ptr;
        &buf[..len]
    }
}

fn sink_str() -> &'static str {
    // SAFETY: tests only emit ASCII diagnostics in this module.
    unsafe { core::str::from_utf8_unchecked(sink_bytes()) }
}

fn daemon(profile: ProfileSummary, dmesg: Option<fn() -> &'static str>) -> RootDaemon<'static> {
    daemon_with_input_status(profile, dmesg, None)
}

fn daemon_with_input_status(
    profile: ProfileSummary,
    dmesg: Option<fn() -> &'static str>,
    input_status: Option<ConsoleInputStatus>,
) -> RootDaemon<'static> {
    let daemon = RootDaemon::new(
        profile,
        sample_boot_info(),
        sample_devices(),
        sample_payloads(),
        dmesg,
        None,
        Some(probe_fixture),
        input_status,
        "reovim-os> ",
        ConsoleInputSummary::new(
            "fixture-input",
            "fixture",
            BootCheckState::Ok,
            BootCheckState::Warn,
        ),
        sink_write,
    );
    daemon
}

fn ready_input_status(_base: ConsoleInputSummary) -> ConsoleInputSummary {
    ConsoleInputSummary::new(
        "usb-keyboard+uart-fallback",
        "live",
        BootCheckState::Ok,
        BootCheckState::Ok,
    )
}

fn run_command(profile: ProfileSummary, line: &[u8], dmesg: Option<fn() -> &'static str>) {
    crate::klog::reset();
    run_command_preserving_log(profile, line, dmesg);
}

fn run_command_preserving_log(
    profile: ProfileSummary,
    line: &[u8],
    dmesg: Option<fn() -> &'static str>,
) {
    sink_clear();
    let daemon = daemon(profile, dmesg);
    let mut session = RootShellSession::new();
    let _ = execute_root_command(&daemon, &mut session, line);
}

fn run_session_command(session: &mut RootShellSession, line: &[u8]) {
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), Some(diagnostics));
    let _ = execute_root_command(&daemon, session, line);
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

fn probe_fixture(target: &str, _devices: &[DeviceEntry], write: WriteFn) -> HardwareProbeResult {
    match target {
        "fixture" => {
            write(b"probe fixture:\nstate=ready\n");
            HardwareProbeResult::Handled
        }
        _ => HardwareProbeResult::UnknownTarget,
    }
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
        "reovim root shell\ncommands: help, clear, screentest, pwd, ls, cd, cat, device, dmesg, probe, launch, halt\nreserved: mount, reovim\n",
    );
});

arch_test!(root_shell_clear_and_screentest, {
    run_command(ProfileSummary::new("shell-only", false), b"clear\n", None);
    testrt::check_eq(sink_str(), "\x1b[2J\x1b[H");

    run_command(ProfileSummary::new("shell-only", false), b"screentest\n", None);
    assert_contains(sink_bytes(), b"screen test:\n");
    assert_contains(sink_bytes(), b"target: framebuffer/serial tty renderer subset");
    assert_contains(sink_bytes(), b"fg16:");
    assert_contains(sink_bytes(), b"fg16+:");
    assert_contains(sink_bytes(), b"bg16:");
    assert_contains(sink_bytes(), b"idx-fg:");
    assert_contains(sink_bytes(), b"\x1b[38;5;196midx196");
    assert_contains(sink_bytes(), b"idx-bg:");
    assert_contains(sink_bytes(), b"rgb-fg:");
    assert_contains(sink_bytes(), b"rgb-bg:");
    assert_contains(sink_bytes(), b"\x1b[48;2;35;132;255m  sky");
    assert_contains(sink_bytes(), b"attrs:");
    assert_contains(sink_bytes(), b"bold");
    assert_contains(sink_bytes(), b"italic");
    assert_contains(sink_bytes(), b"reverse");
    assert_contains(sink_bytes(), b"bold+underline");
    assert_contains(sink_bytes(), b"reset:");
    assert_contains(sink_bytes(), b"cr: overwritten\n");
    assert_contains(sink_bytes(), b"bs: AB\x08 \x08C (should read AC)\n");
    assert_contains(sink_bytes(), b"wrap:");
    assert_contains(sink_bytes(), b"  done\n");
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

    run_command(ProfileSummary::new("appliance", true), b"mount\n", None);
    testrt::check_eq(sink_str(), "reserved: supported after mount/payload-alias rollout\n");
});

arch_test!(root_shell_vfs_pwd_ls_cd_and_cat, {
    crate::klog::reset();
    let mut session = RootShellSession::new();

    run_session_command(&mut session, b"pwd\n");
    testrt::check_eq(sink_str(), "/\n");

    run_session_command(&mut session, b"ls /\n");
    assert_contains(sink_bytes(), b"boot\n");
    assert_contains(sink_bytes(), b"dev\n");
    assert_contains(sink_bytes(), b"log\n");

    run_session_command(&mut session, b"cat /boot/profile\n");
    assert_contains(sink_bytes(), b"profile=shell-only\n");
    assert_contains(sink_bytes(), b"launch=disabled\n");
    assert_contains(sink_bytes(), b"payloads=2\n");
    assert_contains(sink_bytes(), b"input=fixture-input\n");
    assert_contains(sink_bytes(), b"usb_keyboard=unavailable\n");

    run_session_command(&mut session, b"cd /dev\n");
    testrt::check_eq(sink_str(), "");

    run_session_command(&mut session, b"pwd\n");
    testrt::check_eq(sink_str(), "/dev\n");

    run_session_command(&mut session, b"ls\n");
    testrt::check_eq(sink_str(), "uart0\n");

    run_session_command(&mut session, b"cat uart0\n");
    assert_contains(sink_bytes(), b"uart compat=arm,pl011");

    run_session_command(&mut session, b"cd /boot/profile\n");
    testrt::check_eq(sink_str(), "cd: /boot/profile: not a directory\n");

    crate::klog::append_line("boot diagnostics complete");
    run_session_command(&mut session, b"cat /log/dmesg\n");
    assert_contains(sink_bytes(), b"boot diagnostics complete\n");
    assert_contains(sink_bytes(), b"external diagnostics:\nboot diagnostics complete\n");
});

arch_test!(root_shell_boot_profile_uses_live_console_input_status, {
    sink_clear();
    let daemon = daemon_with_input_status(
        ProfileSummary::new("shell-only", false),
        Some(diagnostics),
        Some(ready_input_status),
    );
    let mut session = RootShellSession::new();
    let _ = execute_root_command(&daemon, &mut session, b"cat /boot/profile\n");

    assert_contains(sink_bytes(), b"input=usb-keyboard+uart-fallback\n");
    assert_contains(sink_bytes(), b"input_mode=live\n");
    assert_contains(sink_bytes(), b"usb_keyboard=ready\n");
});

arch_test!(root_shell_dmesg_and_unknown_command, {
    run_command(ProfileSummary::new("shell-only", false), b"dmesg\n", None);
    testrt::check_eq(sink_str(), "dmesg:\n(kernel log empty)\n");

    crate::klog::reset();
    crate::klog::append_line("rootd: boot report");
    run_command_preserving_log(ProfileSummary::new("shell-only", false), b"dmesg\n", None);
    testrt::check_eq(sink_str(), "dmesg:\nrootd: boot report\n");

    crate::klog::reset();
    crate::klog::append_line("rootd: boot report");
    run_command_preserving_log(
        ProfileSummary::new("shell-only", false),
        b"dmesg\n",
        Some(diagnostics),
    );
    testrt::check_eq(
        sink_str(),
        "dmesg:\nrootd: boot report\nexternal diagnostics:\nboot diagnostics complete\n",
    );

    run_command(ProfileSummary::new("shell-only", false), b"does-not-exist\n", None);
    testrt::check_eq(sink_str(), "error: unknown command, try `help`\n");
});

arch_test!(root_shell_probe_uses_lower_provider, {
    run_command(ProfileSummary::new("shell-only", false), b"probe fixture\n", None);
    testrt::check_eq(sink_str(), "probe fixture:\nstate=ready\n");

    run_command(ProfileSummary::new("shell-only", false), b"probe missing\n", None);
    testrt::check_eq(sink_str(), "probe: unknown target: missing\n");

    run_command(ProfileSummary::new("shell-only", false), b"probe\n", None);
    testrt::check_eq(sink_str(), "probe: missing target\n");

    run_command(ProfileSummary::new("shell-only", false), b"probe a b\n", None);
    testrt::check_eq(sink_str(), "probe: too many arguments\n");
});
