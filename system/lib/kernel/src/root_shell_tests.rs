//! Selftests for the root daemon shell parser + command surface.
//!
//! The tests run in the no_std selftest harness and prove the shell can be
//! instantiated from static fixtures with a callback writer.

use {
    super::{RootShellSession, execute_root_command},
    crate::rootd::{
        BootCheckState, BootImageSummary, ConsoleInputStatus, ConsoleInputSummary, HaltKernel,
        HardwareProbeResult, PayloadDescriptor, PayloadLaunchResult, ProfileSummary, RootDaemon,
        WriteFn,
    },
    core::{
        cell::UnsafeCell,
        sync::atomic::{AtomicUsize, Ordering},
    },
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

static HALT_CALLS: AtomicUsize = AtomicUsize::new(0);

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
    daemon_with_input_status_and_halt(profile, dmesg, input_status, None)
}

fn daemon_with_input_status_and_halt(
    profile: ProfileSummary,
    dmesg: Option<fn() -> &'static str>,
    input_status: Option<ConsoleInputStatus>,
    halt: Option<HaltKernel>,
) -> RootDaemon<'static> {
    let daemon = RootDaemon::new(
        profile,
        sample_boot_info(),
        sample_devices(),
        sample_payloads(),
        dmesg,
        halt,
        Some(probe_fixture),
        input_status,
        "reovim-os> ",
        sample_boot_image(),
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
    ConsoleInputSummary::with_usb_diagnostics(
        "usb-keyboard+uart-fallback",
        "live",
        BootCheckState::Ok,
        BootCheckState::Ok,
        2,
        true,
        5,
        "report-ready",
    )
}

fn usb_probe_status(last_poll: &'static str) -> ConsoleInputSummary {
    ConsoleInputSummary::with_usb_diagnostics(
        "pl011-uart",
        "live",
        BootCheckState::Ok,
        BootCheckState::Warn,
        0,
        true,
        5,
        last_poll,
    )
}

fn pcie_blocker_input_status(_base: ConsoleInputSummary) -> ConsoleInputSummary {
    usb_probe_status("no-connected-root-port")
}

fn controller_init_input_status(_base: ConsoleInputSummary) -> ConsoleInputSummary {
    usb_probe_status("needs-controller-init")
}

fn enumeration_input_status(_base: ConsoleInputSummary) -> ConsoleInputSummary {
    usb_probe_status("needs-enumeration")
}

fn report_pending_input_status(_base: ConsoleInputSummary) -> ConsoleInputSummary {
    usb_probe_status("report-pending")
}

fn decoded_pending_input_status(_base: ConsoleInputSummary) -> ConsoleInputSummary {
    ConsoleInputSummary::with_usb_diagnostics(
        "usb-keyboard+uart-fallback",
        "live",
        BootCheckState::Ok,
        BootCheckState::Warn,
        2,
        true,
        5,
        "decoded-pending",
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
    let _ = daemon.run_command_line(&mut session, line);
}

fn run_session_command(session: &mut RootShellSession, line: &[u8]) {
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), Some(diagnostics));
    let _ = daemon.run_command_line(session, line);
}

const SAMPLE_DEVICES: [DeviceEntry; 1] = [DeviceEntry {
    class: DeviceClass::Uart,
    mmio_base: 0x1000,
    mmio_len: 0x100,
    irq: 12,
    capacity_bytes: 0,
    compatible: "arm,pl011",
}];

const SAMPLE_PAYLOADS: [PayloadDescriptor; 3] = [
    PayloadDescriptor {
        name: "reovim",
        summary: "default reovim payload",
        launch: Some(launch_reovim),
    },
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

fn sample_boot_image() -> BootImageSummary {
    BootImageSummary::new(
        "reovim-os",
        "test",
        "fixture-target",
        "shell-only",
        "shell-only",
        "absent",
        "disabled",
    )
}

fn launch_reovim() -> PayloadLaunchResult {
    PayloadLaunchResult::Ready
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
        "help" | "list" => {
            write(
                b"probe targets:\n  fixture\n  pcie\n  usb-keyboard\n  xhci-read-keyboard-report\n",
            );
            HardwareProbeResult::Handled
        }
        "fixture" => {
            write(b"probe fixture:\nstate=ready\n");
            HardwareProbeResult::Handled
        }
        _ => HardwareProbeResult::UnknownTarget,
    }
}

fn halt_fixture() {
    HALT_CALLS.fetch_add(1, Ordering::Relaxed);
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
        "reovim root shell\ncommands: help, clear, screentest, pwd, ls, cd, cat, mount, input, status, proof, device, dmesg, probe, launch, reovim, halt\nusage: help [command]\n",
    );

    run_command(ProfileSummary::new("shell-only", false), b"help input\n", None);
    testrt::check_eq(sink_str(), "input - print live console input diagnostics\n");

    run_command(ProfileSummary::new("shell-only", false), b"help clear\n", None);
    testrt::check_eq(sink_str(), "clear - clear framebuffer console and terminal\n");

    run_command(ProfileSummary::new("shell-only", false), b"help screentest\n", None);
    testrt::check_eq(
        sink_str(),
        "screentest - print renderer diagnostics\n  required rows: el: clean, el1: clean-left, el2: clean-all\n",
    );

    run_command(ProfileSummary::new("shell-only", false), b"help status\n", None);
    testrt::check_eq(sink_str(), "status - print boot, input, and manual_next summary\n");

    run_command(ProfileSummary::new("shell-only", false), b"help proof\n", None);
    testrt::check_eq(sink_str(), "proof - print physical input proof checklist\n");

    run_command(ProfileSummary::new("shell-only", false), b"help probe\n", None);
    testrt::check_eq(
        sink_str(),
        "probe target - run lower hardware probe; try `probe help` or `cat /boot/probes`\n",
    );

    run_command(ProfileSummary::new("shell-only", false), b"help dmesg\n", None);
    testrt::check_eq(sink_str(), "dmesg [--stats] - print retained kernel log or ring stats\n");

    run_command(ProfileSummary::new("shell-only", false), b"help device\n", None);
    testrt::check_eq(sink_str(), "device - print boot memory and device inventory\n");

    run_command(ProfileSummary::new("shell-only", false), b"help launch\n", None);
    testrt::check_eq(sink_str(), "launch [payload] - list or run registered payloads\n");

    run_command(ProfileSummary::new("shell-only", false), b"help reovim\n", None);
    testrt::check_eq(sink_str(), "reovim - run the default reovim payload alias\n");

    run_command(ProfileSummary::new("shell-only", false), b"help halt\n", None);
    testrt::check_eq(sink_str(), "halt - request root daemon shutdown\n");

    run_command(ProfileSummary::new("shell-only", false), b"help missing\n", None);
    testrt::check_eq(sink_str(), "help: unknown command: missing\n");

    run_command(ProfileSummary::new("shell-only", false), b"help a b\n", None);
    testrt::check_eq(sink_str(), "help: too many arguments\n");
});

arch_test!(root_shell_clear_and_screentest, {
    run_command(ProfileSummary::new("shell-only", false), b"clear\n", None);
    testrt::check_eq(sink_str(), "\x1b[2J\x1b[H");

    run_command(ProfileSummary::new("shell-only", false), b"clear now\n", None);
    testrt::check_eq(sink_str(), "clear: too many arguments\n");

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
    assert_contains(sink_bytes(), b"el: clean\x1b[K\n");
    assert_contains(sink_bytes(), b"el1: clean-left\n");
    assert_contains(sink_bytes(), b"\x1b[1K\r");
    assert_contains(sink_bytes(), b"el2: clean-all\n");
    assert_contains(sink_bytes(), b"\x1b[2K\r");
    assert_contains(sink_bytes(), b"bs: AB\x08 \x08C (should read AC)\n");
    assert_contains(sink_bytes(), b"wrap:");
    assert_contains(sink_bytes(), b"  done\n");

    run_command(ProfileSummary::new("shell-only", false), b"screentest extra\n", None);
    testrt::check_eq(sink_str(), "screentest: too many arguments\n");
});

arch_test!(root_shell_launch_mount_and_reovim, {
    run_command(ProfileSummary::new("appliance", true), b"launch\n", None);
    assert_contains(
        sink_bytes(),
        b"launch: available payloads:\n  reovim: default reovim payload\n  editor-smoke: editor smoke payload\n  server-smoke: server smoke payload\n",
    );

    run_command(ProfileSummary::new("appliance", true), b"launch editor-smoke\n", None);
    testrt::check_eq(sink_str(), "launch editor-smoke: payload.ready\n");

    run_command(ProfileSummary::new("appliance", true), b"launch \"\"\n", None);
    testrt::check_eq(sink_str(), "launch: no payload name\n");

    run_command(ProfileSummary::new("shell-only", false), b"launch editor-smoke\n", None);
    testrt::check_eq(sink_str(), "launch disabled for this profile\n");

    run_command(ProfileSummary::new("appliance", true), b"mount\n", None);
    assert_contains(sink_bytes(), b"kernel on / type rootfs (ro,pseudo)\n");
    assert_contains(sink_bytes(), b"boot on /boot type bootfs (ro,pseudo)\n");
    assert_contains(sink_bytes(), b"devices on /dev type devfs (ro,pseudo)\n");
    assert_contains(sink_bytes(), b"klog on /log type logfs (ro,pseudo)\n");

    run_command(ProfileSummary::new("appliance", true), b"mount extra\n", None);
    testrt::check_eq(sink_str(), "mount: too many arguments\n");

    run_command(ProfileSummary::new("appliance", true), b"reovim\n", None);
    testrt::check_eq(sink_str(), "reovim: payload.ready\n");

    run_command(ProfileSummary::new("shell-only", false), b"reovim\n", None);
    testrt::check_eq(sink_str(), "reovim disabled for this profile\n");

    run_command(ProfileSummary::new("appliance", true), b"reovim extra\n", None);
    testrt::check_eq(sink_str(), "reovim: too many arguments\n");
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
    assert_contains(sink_bytes(), b"payloads=3\n");
    assert_contains(sink_bytes(), b"input=fixture-input\n");
    assert_contains(sink_bytes(), b"usb_keyboard=unavailable\n");

    run_session_command(&mut session, b"cat \"/boot/profile\"\n");
    assert_contains(sink_bytes(), b"profile=shell-only\n");

    run_session_command(&mut session, b"cat \"\"\n");
    testrt::check_eq(sink_str(), "cat: empty path\n");

    run_session_command(&mut session, b"ls \"\"\n");
    testrt::check_eq(sink_str(), "ls: empty path\n");

    run_session_command(&mut session, b"cd \"\"\n");
    testrt::check_eq(sink_str(), "cd: empty path\n");

    run_session_command(&mut session, b"ls /boot\n");
    assert_contains(sink_bytes(), b"devices\n");
    assert_contains(sink_bytes(), b"help\n");
    assert_contains(sink_bytes(), b"image\n");
    assert_contains(sink_bytes(), b"input\n");
    assert_contains(sink_bytes(), b"memory\n");
    assert_contains(sink_bytes(), b"mounts\n");
    assert_contains(sink_bytes(), b"probes\n");
    assert_contains(sink_bytes(), b"proof\n");
    assert_contains(sink_bytes(), b"profile\n");
    assert_contains(sink_bytes(), b"status\n");

    run_session_command(&mut session, b"cat /boot/help\n");
    assert_contains(sink_bytes(), b"reovim root shell\n");
    assert_contains(sink_bytes(), b"commands: help, clear, screentest");
    assert_contains(sink_bytes(), b"usage: help [command]\n");
    assert_contains(sink_bytes(), b"details:\n");
    assert_contains(sink_bytes(), b"  help [command] - show command help\n");
    assert_contains(sink_bytes(), b"  clear - clear framebuffer console and terminal\n");
    assert_contains(sink_bytes(), b"  screentest - print renderer diagnostics\n");
    assert_contains(
        sink_bytes(),
        b"    required rows: el: clean, el1: clean-left, el2: clean-all\n",
    );
    assert_contains(sink_bytes(), b"  pwd - print current kernel VFS directory\n");
    assert_contains(sink_bytes(), b"  ls [path] - list a kernel VFS directory\n");
    assert_contains(sink_bytes(), b"  cd [path] - change current kernel VFS directory\n");
    assert_contains(sink_bytes(), b"  cat path... - print kernel VFS pseudo files\n");
    assert_contains(sink_bytes(), b"  mount - print kernel VFS mount table\n");
    assert_contains(sink_bytes(), b"  input - print live console input diagnostics\n");
    assert_contains(sink_bytes(), b"  status - print boot, input, and manual_next summary\n");
    assert_contains(sink_bytes(), b"  proof - print physical input proof checklist\n");
    assert_contains(sink_bytes(), b"  device - print boot memory and device inventory\n");
    assert_contains(sink_bytes(), b"  dmesg [--stats] - print retained kernel log or ring stats\n");
    assert_contains(
        sink_bytes(),
        b"  probe target - run lower hardware probe; try `probe help` or `cat /boot/probes`\n",
    );
    assert_contains(sink_bytes(), b"  launch [payload] - list or run registered payloads\n");
    assert_contains(sink_bytes(), b"  reovim - run the default reovim payload alias\n");
    assert_contains(sink_bytes(), b"  halt - request root daemon shutdown\n");

    run_session_command(&mut session, b"cat /boot/image\n");
    assert_contains(sink_bytes(), b"package=reovim-os\n");
    assert_contains(sink_bytes(), b"version=test\n");
    assert_contains(sink_bytes(), b"target=fixture-target\n");
    assert_contains(sink_bytes(), b"selected_profile=shell-only\n");
    assert_contains(sink_bytes(), b"profile_request=shell-only\n");
    assert_contains(sink_bytes(), b"bootline=absent\n");
    assert_contains(sink_bytes(), b"launch_profile_feature=disabled\n");

    run_session_command(&mut session, b"cat /boot/input\n");
    assert_contains(sink_bytes(), b"source=fixture-input\n");
    assert_contains(sink_bytes(), b"source_state=ready\n");
    assert_contains(sink_bytes(), b"mode=fixture\n");
    assert_contains(sink_bytes(), b"usb_keyboard=unavailable\n");
    assert_contains(sink_bytes(), b"usb_keyboard_pending_bytes=0\n");
    assert_contains(sink_bytes(), b"usb_keyboard_probe=disabled\n");
    assert_contains(sink_bytes(), b"usb_keyboard_poll_interval_ms=0\n");
    assert_contains(sink_bytes(), b"usb_keyboard_last_poll=not-polled\n");

    run_session_command(&mut session, b"cat /boot/status\n");
    assert_contains(sink_bytes(), b"package=reovim-os\n");
    assert_contains(sink_bytes(), b"target=fixture-target\n");
    assert_contains(sink_bytes(), b"bootline=absent\n");
    assert_contains(sink_bytes(), b"profile=shell-only\n");
    assert_contains(sink_bytes(), b"input=fixture-input\n");
    assert_contains(sink_bytes(), b"source_state=ready\n");
    assert_contains(sink_bytes(), b"usb_keyboard=unavailable\n");
    assert_contains(sink_bytes(), b"usb_keyboard_poll_interval_ms=0\n");
    assert_contains(sink_bytes(), b"usb_keyboard_last_poll=not-polled\n");
    assert_contains(sink_bytes(), b"manual_next=probe-help\n");

    run_session_command(&mut session, b"cat /boot/proof\n");
    assert_contains(sink_bytes(), b"proof:\n");
    assert_contains(sink_bytes(), b"  proof\n");
    assert_contains(sink_bytes(), b"  cat /boot/proof\n");
    assert_contains(sink_bytes(), b"  help\n");
    assert_contains(sink_bytes(), b"  help clear\n");
    assert_contains(sink_bytes(), b"  help screentest\n");
    assert_contains(sink_bytes(), b"  help input\n");
    assert_contains(sink_bytes(), b"  help proof\n");
    assert_contains(sink_bytes(), b"  help pwd\n");
    assert_contains(sink_bytes(), b"  help ls\n");
    assert_contains(sink_bytes(), b"  help cd\n");
    assert_contains(sink_bytes(), b"  help cat\n");
    assert_contains(sink_bytes(), b"  help mount\n");
    assert_contains(sink_bytes(), b"  help device\n");
    assert_contains(sink_bytes(), b"  help dmesg\n");
    assert_contains(sink_bytes(), b"  help status\n");
    assert_contains(sink_bytes(), b"  help probe\n");
    assert_contains(sink_bytes(), b"  help launch\n");
    assert_contains(sink_bytes(), b"  help reovim\n");
    assert_contains(sink_bytes(), b"  help halt\n");
    assert_contains(sink_bytes(), b"  cat /boot/help\n");
    assert_contains(sink_bytes(), b"  clear\n");
    assert_contains(sink_bytes(), b"  screentest\n");
    assert_contains(sink_bytes(), b"  pwd\n");
    assert_contains(sink_bytes(), b"  ls /\n");
    assert_contains(sink_bytes(), b"  ls /boot\n");
    assert_contains(sink_bytes(), b"  ls /dev\n");
    assert_contains(sink_bytes(), b"  ls /log\n");
    assert_contains(sink_bytes(), b"  mount\n");
    assert_contains(sink_bytes(), b"  cat /boot/mounts\n");
    assert_contains(sink_bytes(), b"  device\n");
    assert_contains(sink_bytes(), b"  cat /boot/memory\n");
    assert_contains(sink_bytes(), b"  cat /boot/devices\n");
    assert_contains(sink_bytes(), b"  cd /dev\n  pwd\n  ls\n  cat uart0\n");
    assert_contains(sink_bytes(), b"  cd /\n");
    assert_contains(sink_bytes(), b"  cat /boot/image\n");
    assert_contains(sink_bytes(), b"  status\n");
    assert_contains(sink_bytes(), b"  cat /boot/status\n");
    assert_contains(sink_bytes(), b"  input\n");
    assert_contains(sink_bytes(), b"  cat /boot/input\n");
    assert_contains(sink_bytes(), b"  probe help\n");
    assert_contains(sink_bytes(), b"  cat /boot/probes\n");
    assert_contains(sink_bytes(), b"  probe pcie\n");
    assert_contains(sink_bytes(), b"  probe usb-keyboard\n");
    assert_contains(sink_bytes(), b"  cat /boot/profile\n");
    assert_contains(sink_bytes(), b"  launch\n");
    assert_contains(sink_bytes(), b"  reovim\n");
    assert_contains(sink_bytes(), b"  dmesg --stats\n");
    assert_contains(sink_bytes(), b"  cat /log/stats\n");
    assert_contains(sink_bytes(), b"  dmesg\n");
    assert_contains(sink_bytes(), b"  cat /log/dmesg\n");
    assert_contains(sink_bytes(), b"terminal:\n");
    assert_contains(sink_bytes(), b"  halt\n");
    assert_contains(sink_bytes(), b"  package=reovim-os\n");
    assert_contains(sink_bytes(), b"  version=test\n");
    assert_contains(sink_bytes(), b"  target=fixture-target\n");
    assert_contains(sink_bytes(), b"  selected_profile=shell-only\n");
    assert_contains(sink_bytes(), b"  profile_request=shell-only\n");
    assert_contains(sink_bytes(), b"  launch_profile_feature=disabled\n");
    assert_contains(sink_bytes(), b"  profile=shell-only\n");
    assert_contains(sink_bytes(), b"  launch=disabled\n");
    assert_contains(sink_bytes(), b"  payloads=3\n");
    assert_contains(sink_bytes(), b"  bootline=absent\n");
    assert_contains(sink_bytes(), b"  source=usb-keyboard+uart-fallback\n");
    assert_contains(sink_bytes(), b"  source_state=ready\n");
    assert_contains(sink_bytes(), b"  mode=live\n");
    assert_contains(sink_bytes(), b"  input_mode=live\n");
    assert_contains(sink_bytes(), b"  usb_keyboard=ready\n");
    assert_contains(sink_bytes(), b"  usb_keyboard_probe=enabled\n");
    assert_contains(sink_bytes(), b"  usb_keyboard_last_poll=report-ready\n");
    assert_contains(sink_bytes(), b"  manual_next=type-shell-command\n");
    assert_contains(sink_bytes(), b"  detailed help catalog available through /boot/help\n");
    assert_contains(sink_bytes(), b"  screentest includes erase-line mode diagnostics\n");
    assert_contains(sink_bytes(), b"  kernel log stats available through /log/stats\n");
    assert_contains(sink_bytes(), b"  probe catalog available through /boot/probes\n");
    assert_contains(sink_bytes(), b"  probe targets include usb-keyboard\n");
    assert_contains(sink_bytes(), b"  probe targets include xhci-read-keyboard-report\n");
    assert_contains(sink_bytes(), b"  launch/reovim disabled in shell-only profile\n");
    assert_contains(sink_bytes(), b"  shell.status=ok\n");
    assert_contains(sink_bytes(), b"  shell.status=error for disabled payload commands\n");
    assert_contains(sink_bytes(), b"  halt typed last prints halt: ok and stops root daemon\n");

    run_session_command(&mut session, b"proof\n");
    assert_contains(sink_bytes(), b"proof:\n");
    assert_contains(sink_bytes(), b"  proof\n");
    assert_contains(sink_bytes(), b"  cat /boot/proof\n");
    assert_contains(sink_bytes(), b"  help clear\n");
    assert_contains(sink_bytes(), b"  help screentest\n");
    assert_contains(sink_bytes(), b"  help input\n");
    assert_contains(sink_bytes(), b"  help proof\n");
    assert_contains(sink_bytes(), b"  help pwd\n");
    assert_contains(sink_bytes(), b"  help ls\n");
    assert_contains(sink_bytes(), b"  help cd\n");
    assert_contains(sink_bytes(), b"  help cat\n");
    assert_contains(sink_bytes(), b"  help mount\n");
    assert_contains(sink_bytes(), b"  help device\n");
    assert_contains(sink_bytes(), b"  help dmesg\n");
    assert_contains(sink_bytes(), b"  help status\n");
    assert_contains(sink_bytes(), b"  help probe\n");
    assert_contains(sink_bytes(), b"  help launch\n");
    assert_contains(sink_bytes(), b"  help reovim\n");
    assert_contains(sink_bytes(), b"  help halt\n");
    assert_contains(sink_bytes(), b"  cat /boot/probes\n");
    assert_contains(sink_bytes(), b"expected:\n");
    assert_contains(sink_bytes(), b"  probe targets include pcie\n");
    assert_contains(sink_bytes(), b"  probe targets include xhci-read-keyboard-report\n");
    assert_contains(sink_bytes(), b"  detailed help catalog available through /boot/help\n");
    assert_contains(sink_bytes(), b"  kernel log stats available through /log/stats\n");
    assert_contains(sink_bytes(), b"  probe catalog available through /boot/probes\n");
    assert_contains(sink_bytes(), b"  launch/reovim disabled in shell-only profile\n");
    assert_contains(sink_bytes(), b"  halt typed last prints halt: ok and stops root daemon\n");

    run_session_command(&mut session, b"proof extra\n");
    testrt::check_eq(sink_str(), "proof: too many arguments\n");

    run_session_command(&mut session, b"status\n");
    assert_contains(sink_bytes(), b"package=reovim-os\n");
    assert_contains(sink_bytes(), b"version=test\n");
    assert_contains(sink_bytes(), b"selected_profile=shell-only\n");
    assert_contains(sink_bytes(), b"profile_request=shell-only\n");
    assert_contains(sink_bytes(), b"launch_profile_feature=disabled\n");
    assert_contains(sink_bytes(), b"payloads=3\n");
    assert_contains(sink_bytes(), b"manual_next=probe-help\n");

    run_session_command(&mut session, b"status extra\n");
    testrt::check_eq(sink_str(), "status: too many arguments\n");

    run_session_command(&mut session, b"input\n");
    assert_contains(sink_bytes(), b"source=fixture-input\n");
    assert_contains(sink_bytes(), b"usb_keyboard_pending_bytes=0\n");
    assert_contains(sink_bytes(), b"usb_keyboard_probe=disabled\n");

    run_session_command(&mut session, b"input extra\n");
    testrt::check_eq(sink_str(), "input: too many arguments\n");

    run_session_command(&mut session, b"cat /boot/mounts\n");
    assert_contains(sink_bytes(), b"kernel on / type rootfs (ro,pseudo)\n");
    assert_contains(sink_bytes(), b"devices on /dev type devfs (ro,pseudo)\n");

    run_session_command(&mut session, b"ls /dev\n");
    testrt::check_eq(sink_str(), "uart0\n");

    run_session_command(&mut session, b"device\n");
    assert_contains(sink_bytes(), b"boot_info:\n");
    assert_contains(sink_bytes(), b"  ranges=1\n");
    assert_contains(sink_bytes(), b"  usable_bytes=4096\n");
    assert_contains(sink_bytes(), b"devices:\n");
    assert_contains(sink_bytes(), b"- [0] uart compat=arm,pl011 mmio=0x1000/0x100 irq=12\n");

    run_session_command(&mut session, b"device extra\n");
    testrt::check_eq(sink_str(), "device: too many arguments\n");

    run_session_command(&mut session, b"cat /boot/memory\n");
    assert_contains(sink_bytes(), b"ranges=1\n");
    assert_contains(sink_bytes(), b"usable_bytes=4096\n");
    assert_contains(sink_bytes(), b"cpu_count=2\n");

    run_session_command(&mut session, b"cat /boot/devices\n");
    assert_contains(sink_bytes(), b"- [0] uart compat=arm,pl011 mmio=0x1000/0x100 irq=12\n");

    run_session_command(&mut session, b"cat /boot/probes\n");
    assert_contains(sink_bytes(), b"probe targets:\n");
    assert_contains(sink_bytes(), b"  fixture\n");
    assert_contains(sink_bytes(), b"  usb-keyboard\n");
    assert_contains(sink_bytes(), b"  xhci-read-keyboard-report\n");

    run_session_command(&mut session, b"cat /boot/probes/extra\n");
    testrt::check_eq(sink_str(), "cat: /boot/probes/extra: not a directory\n");

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

    run_session_command(&mut session, b"ls /log\n");
    testrt::check_eq(sink_str(), "dmesg\nstats\n");

    run_session_command(&mut session, b"cat /log/stats\n");
    assert_contains(sink_bytes(), b"capacity_bytes=8192\n");
    assert_contains(sink_bytes(), b"retained_bytes=");
    assert_contains(sink_bytes(), b"dropped_bytes=0\n");
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

    sink_clear();
    let _ = execute_root_command(&daemon, &mut session, b"cat /boot/input\n");
    assert_contains(sink_bytes(), b"source=usb-keyboard+uart-fallback\n");
    assert_contains(sink_bytes(), b"source_state=ready\n");
    assert_contains(sink_bytes(), b"mode=live\n");
    assert_contains(sink_bytes(), b"usb_keyboard=ready\n");
    assert_contains(sink_bytes(), b"usb_keyboard_pending_bytes=2\n");
    assert_contains(sink_bytes(), b"usb_keyboard_probe=enabled\n");
    assert_contains(sink_bytes(), b"usb_keyboard_poll_interval_ms=5\n");
    assert_contains(sink_bytes(), b"usb_keyboard_last_poll=report-ready\n");

    sink_clear();
    let _ = execute_root_command(&daemon, &mut session, b"cat /boot/status\n");
    assert_contains(sink_bytes(), b"package=reovim-os\n");
    assert_contains(sink_bytes(), b"version=test\n");
    assert_contains(sink_bytes(), b"selected_profile=shell-only\n");
    assert_contains(sink_bytes(), b"profile_request=shell-only\n");
    assert_contains(sink_bytes(), b"launch_profile_feature=disabled\n");
    assert_contains(sink_bytes(), b"payloads=3\n");
    assert_contains(sink_bytes(), b"input=usb-keyboard+uart-fallback\n");
    assert_contains(sink_bytes(), b"source_state=ready\n");
    assert_contains(sink_bytes(), b"usb_keyboard=ready\n");
    assert_contains(sink_bytes(), b"usb_keyboard_poll_interval_ms=5\n");
    assert_contains(sink_bytes(), b"usb_keyboard_last_poll=report-ready\n");
    assert_contains(sink_bytes(), b"manual_next=type-shell-command\n");
});

arch_test!(root_shell_status_reports_specific_manual_next_probe, {
    let cases: &[(ConsoleInputStatus, &[u8])] = &[
        (pcie_blocker_input_status, b"manual_next=probe-pcie\n"),
        (controller_init_input_status, b"manual_next=probe-xhci-start\n"),
        (enumeration_input_status, b"manual_next=probe-xhci-read-keyboard-report\n"),
        (report_pending_input_status, b"manual_next=probe-usb-keyboard\n"),
        (decoded_pending_input_status, b"manual_next=type-shell-command\n"),
    ];

    let mut index = 0usize;
    while index < cases.len() {
        sink_clear();
        let daemon = daemon_with_input_status(
            ProfileSummary::new("shell-only", false),
            Some(diagnostics),
            Some(cases[index].0),
        );
        let mut session = RootShellSession::new();
        let _ = execute_root_command(&daemon, &mut session, b"cat /boot/status\n");
        assert_contains(sink_bytes(), cases[index].1);
        index += 1;
    }
});

arch_test!(root_shell_dmesg_and_unknown_command, {
    run_command(ProfileSummary::new("shell-only", false), b"dmesg\n", None);
    testrt::check_eq(sink_str(), "dmesg:\nshell: dmesg\n");

    run_command(ProfileSummary::new("shell-only", false), b"dmesg extra\n", None);
    testrt::check_eq(sink_str(), "dmesg: unknown option\n");

    run_command(ProfileSummary::new("shell-only", false), b"dmesg a b\n", None);
    testrt::check_eq(sink_str(), "dmesg: too many arguments\n");

    run_command(ProfileSummary::new("shell-only", false), b"dmesg --stats\n", None);
    assert_contains(sink_bytes(), b"capacity_bytes=8192\n");
    assert_contains(sink_bytes(), b"retained_bytes=");
    assert_contains(sink_bytes(), b"dropped_bytes=0\n");

    crate::klog::reset();
    crate::klog::append_line("rootd: boot report");
    run_command_preserving_log(ProfileSummary::new("shell-only", false), b"dmesg\n", None);
    testrt::check_eq(sink_str(), "dmesg:\nrootd: boot report\nshell: dmesg\n");

    crate::klog::reset();
    crate::klog::append_line("rootd: boot report");
    run_command_preserving_log(
        ProfileSummary::new("shell-only", false),
        b"dmesg\n",
        Some(diagnostics),
    );
    testrt::check_eq(
        sink_str(),
        "dmesg:\nrootd: boot report\nshell: dmesg\nexternal diagnostics:\nboot diagnostics complete\n",
    );

    run_command(ProfileSummary::new("shell-only", false), b"does-not-exist\n", None);
    testrt::check_eq(sink_str(), "error: unknown command, try `help`\n");
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_command_line(&mut session, b"cat /log/dmesg\n");
    assert_contains(sink_bytes(), b"shell: does-not-exist\n");
    assert_contains(sink_bytes(), b"shell.status=error\n");
});

arch_test!(root_shell_rejects_unterminated_quotes, {
    run_command(ProfileSummary::new("shell-only", false), b"cat \"/boot/profile\n", None);
    testrt::check_eq(sink_str(), "error: unterminated quote\n");

    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_command_line(&mut session, b"cat /log/dmesg\n");
    assert_contains(sink_bytes(), b"shell: cat \"/boot/profile\n");
    assert_contains(sink_bytes(), b"shell.status=error\n");

    run_command(ProfileSummary::new("shell-only", false), b"cat '/boot/profile\n", None);
    testrt::check_eq(sink_str(), "error: unterminated quote\n");
});

arch_test!(root_shell_halt_logs_status_before_callback, {
    crate::klog::reset();
    sink_clear();
    HALT_CALLS.store(0, Ordering::Relaxed);
    let daemon = daemon_with_input_status_and_halt(
        ProfileSummary::new("shell-only", false),
        None,
        None,
        Some(halt_fixture),
    );
    let mut session = RootShellSession::new();

    let should_halt = daemon.run_command_line(&mut session, b"halt\n");

    testrt::check(should_halt, "halt command requests daemon shutdown");
    testrt::check_eq(sink_str(), "halt: ok\n");
    testrt::check_eq(HALT_CALLS.load(Ordering::Relaxed), 0usize);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"shell: halt\n");
    assert_contains(sink_bytes(), b"shell.status=halt\n");

    crate::klog::reset();
    sink_clear();
    let should_halt = daemon.run_command_line(&mut session, b"halt now\n");
    testrt::check(!should_halt, "halt with arguments must not stop the daemon");
    testrt::check_eq(sink_str(), "halt: too many arguments\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"shell: halt now\n");
    assert_contains(sink_bytes(), b"shell.status=error\n");
});

arch_test!(root_shell_probe_uses_lower_provider, {
    run_command(ProfileSummary::new("shell-only", false), b"probe fixture\n", None);
    testrt::check_eq(sink_str(), "probe fixture:\nstate=ready\n");

    run_command(ProfileSummary::new("shell-only", false), b"probe missing\n", None);
    testrt::check_eq(sink_str(), "probe: unknown target: missing\n");

    run_command(ProfileSummary::new("shell-only", false), b"probe\n", None);
    testrt::check_eq(sink_str(), "probe: missing target, try `probe help`\n");

    run_command(ProfileSummary::new("shell-only", false), b"probe a b\n", None);
    testrt::check_eq(sink_str(), "probe: too many arguments\n");
});
