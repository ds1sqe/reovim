//! Selftests for the root daemon shell parser + `/bin` surface.
//!
//! The tests run in the no_std selftest harness and prove the shell can be
//! instantiated from static fixtures with a callback writer.

use {
    super::{RootShellSession, execute_loaded_program_argv},
    crate::{
        block::BlockDevice,
        program::{
            ProgramArgv, ProgramArgvBuffer, ProgramDescriptor, ProgramImage, ProgramSourceArtifact,
            ProgramStatus,
        },
        rootd::{
            BootCheckState, BootImageSummary, ConsoleInputStatus, ConsoleInputSummary, HaltKernel,
            HardwareProbeResult, PayloadDescriptor, PayloadImage, PayloadImageKind,
            PayloadLaunchResult, PayloadSourceArtifact, ProfileSummary, RootDaemon, WriteFn,
        },
        source_store::{
            ExecutableSourceStore, SourceArtifactNamespace, install_source, reset_installed_sources,
        },
    },
    core::{
        cell::UnsafeCell,
        sync::atomic::{AtomicUsize, Ordering},
    },
    reovim_testrt::{self as testrt, arch_test},
    reovim_uapi_fs::{
        DescriptorFlags, FileStatusFlags, OpenAtDir, OpenFlags, RawFd, SyscallFdControl,
    },
    reovim_uapi_process::{ExitCode, ProcessArg, SyscallProcessControl},
    reovim_uapi_syscall::RawSyscall,
    reovim_uapi_system::{BootInfo, DeviceClass, DeviceEntry},
};

const SINK_CAPACITY: usize = crate::syscall::MAX_PROGRAM_VFS_FILE_BYTES + 4096;

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
static TTY_READ_CALLS: AtomicUsize = AtomicUsize::new(0);

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

fn argv1(arg0: &str) -> ProgramArgvBuffer {
    let mut argv = ProgramArgvBuffer::empty();
    argv.push(arg0).expect("test argv0 fits");
    argv
}

fn argv2(arg0: &str, arg1: &str) -> ProgramArgvBuffer {
    let mut argv = argv1(arg0);
    argv.push(arg1).expect("test argv1 fits");
    argv
}

fn retained_process_context(pid: usize) -> crate::syscall::SyscallContext {
    let process = crate::proc::process(pid).expect("process retained");
    crate::syscall::SyscallContext::from_process(crate::proc::ProcessHandle {
        pid: process.pid,
        task_id: process.task_id,
        program_path: process.program_path,
        loader: process.loader,
        entry_name: process.entry_name,
    })
}

fn bin_fd_carrier(_argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let _ = SyscallProcessControl::new(raw).exit(ExitCode::SUCCESS);
    ProgramStatus::Error
}

fn bin_fd_read(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let target_fd = if argv.arg(1) == Some("4") { 4 } else { 3 };
    let mut bytes = [0u8; 8];
    match fd.read(RawFd::new(target_fd), &mut bytes) {
        Ok(read) => {
            let _ = fd.write(RawFd::stdout(), &bytes[..read]);
            let _ = process.exit(ExitCode::SUCCESS);
        }
        Err(_) => {
            let _ = fd.write(RawFd::stdout(), b"fd-read-error\n");
            let _ = process.exit(ExitCode::FAILURE);
        }
    }
    ProgramStatus::Error
}

const CLOEXEC_TEST_PROGRAMS: [ProgramDescriptor; 2] = [
    ProgramDescriptor {
        id: 90_000,
        name: "carrier",
        path: "/bin/carrier",
        summary: "selftest execve carrier",
        image: ProgramImage::Linked(bin_fd_carrier),
        entry_name: "bin_fd_carrier",
    },
    ProgramDescriptor {
        id: 90_001,
        name: "fdread",
        path: "/bin/fdread",
        summary: "selftest inherited fd reader",
        image: ProgramImage::Linked(bin_fd_read),
        entry_name: "bin_fd_read",
    },
];

const CLOEXEC_TEST_PROGRAM_SOURCES: [ProgramSourceArtifact; 0] = [];

fn fixture_source_store() -> ExecutableSourceStore {
    ExecutableSourceStore::new(crate::bin_fixture::program_sources(), sample_payload_sources())
}

fn cloexec_test_source_store() -> ExecutableSourceStore {
    ExecutableSourceStore::new(&CLOEXEC_TEST_PROGRAM_SOURCES, sample_payload_sources())
}

fn daemon(profile: ProfileSummary, dmesg: Option<fn() -> &'static str>) -> RootDaemon<'static> {
    daemon_with_input_status(profile, dmesg, None)
}

fn daemon_with_programs(
    programs: &'static [ProgramDescriptor],
    program_sources: &'static [ProgramSourceArtifact],
) -> RootDaemon<'static> {
    RootDaemon::new(
        ProfileSummary::new("shell-only", false),
        sample_boot_info(),
        sample_devices(),
        sample_payloads(),
        sample_payload_sources(),
        programs,
        program_sources,
        crate::bin_fixture::write_vfs_file,
        crate::bin_fixture::write_program_help,
        None,
        None,
        Some(probe_fixture),
        None,
        read_line_fixture,
        "reovim-os> ",
        sample_boot_image(),
        ConsoleInputSummary::new(
            "fixture-input",
            "fixture",
            BootCheckState::Ok,
            BootCheckState::Warn,
        ),
        sink_write,
    )
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
        sample_payload_sources(),
        crate::bin_fixture::programs(),
        crate::bin_fixture::program_sources(),
        crate::bin_fixture::write_vfs_file,
        crate::bin_fixture::write_program_help,
        dmesg,
        halt,
        Some(probe_fixture),
        input_status,
        read_line_fixture,
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

fn run_shell_line_fixture(
    profile: ProfileSummary,
    line: &[u8],
    dmesg: Option<fn() -> &'static str>,
) {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    run_shell_line_preserving_log(profile, line, dmesg);
}

fn run_shell_line_preserving_log(
    profile: ProfileSummary,
    line: &[u8],
    dmesg: Option<fn() -> &'static str>,
) {
    sink_clear();
    let daemon = daemon(profile, dmesg);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, line);
}

fn run_session_shell_line(session: &mut RootShellSession, line: &[u8]) {
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), Some(diagnostics));
    let _ = daemon.run_shell_line(session, line);
}

fn run_session_idle_ready_programs(session: &mut RootShellSession) -> ProgramStatus {
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), Some(diagnostics));
    daemon.run_idle_ready_programs(session)
}

fn physical_proof_daemon() -> RootDaemon<'static> {
    RootDaemon::new(
        ProfileSummary::new("shell-only", false),
        sample_boot_info(),
        sample_devices(),
        sample_payloads(),
        sample_payload_sources(),
        crate::bin_fixture::programs(),
        crate::bin_fixture::program_sources(),
        crate::bin_fixture::write_vfs_file,
        crate::bin_fixture::write_program_help,
        Some(diagnostics),
        None,
        Some(probe_budget_fixture),
        Some(ready_input_status),
        read_line_fixture,
        "reovim-os> ",
        sample_boot_image(),
        ConsoleInputSummary::new(
            "usb-keyboard+uart-fallback",
            "live",
            BootCheckState::Ok,
            BootCheckState::Ok,
        ),
        sink_write,
    )
}

const USB_LINE_SOURCE_AUDIT: &[u8] =
    b"input.line_source=usb-keyboard usb_bytes=6 fallback_bytes=0 line_bytes=5\n";

const PHYSICAL_PROOF_COMMANDS: &[&[u8]] = &[
    b"proof\n",
    b"cat /boot/proof\n",
    b"help\n",
    b"help clear\n",
    b"help screentest\n",
    b"help input\n",
    b"help proof\n",
    b"help pwd\n",
    b"help ls\n",
    b"help cd\n",
    b"help cat\n",
    b"help read\n",
    b"help mount\n",
    b"help device\n",
    b"help dmesg\n",
    b"help dump\n",
    b"help sched\n",
    b"help proc\n",
    b"help status\n",
    b"help probe\n",
    b"help launch\n",
    b"help reovim\n",
    b"help hello\n",
    b"hello\n",
    b"help halt\n",
    b"cat /boot/help\n",
    b"clear\n",
    b"screentest\n",
    b"pwd\n",
    b"ls /\n",
    b"ls /boot\n",
    b"ls /dev\n",
    b"ls /log\n",
    b"mount\n",
    b"cat /boot/mounts\n",
    b"device\n",
    b"cat /boot/memory\n",
    b"cat /boot/devices\n",
    b"cd /dev\n",
    b"pwd\n",
    b"ls\n",
    b"cat uart0\n",
    b"cd /\n",
    b"cat /boot/image\n",
    b"status\n",
    b"cat /boot/status\n",
    b"input\n",
    b"cat /boot/input\n",
    b"probe help\n",
    b"cat /boot/probes\n",
    b"probe pcie\n",
    b"probe usb-keyboard\n",
    b"cat /boot/profile\n",
    b"launch\n",
    b"reovim\n",
    b"dmesg --stats\n",
    b"dump status\n",
    b"dump snapshot\n",
    b"dump sync\n",
    b"cat /log/stats\n",
    b"cat /log/events\n",
    b"proc\n",
    b"dmesg\n",
    b"cat /log/dmesg\n",
];

fn run_physical_proof_shell_line(session: &mut RootShellSession, line: &[u8]) {
    sink_clear();
    crate::klog::append_bytes(USB_LINE_SOURCE_AUDIT);
    let daemon = physical_proof_daemon();
    let _ = daemon.run_shell_line(session, line);
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
        path: "/payload/reovim",
        summary: "default reovim payload",
        entry_name: "payload_reovim",
        image: PayloadImage::SourcePath("/payload/reovim"),
    },
    PayloadDescriptor {
        name: "editor-smoke",
        path: "/payload/editor-smoke",
        summary: "editor smoke payload",
        entry_name: "payload_editor_smoke",
        image: PayloadImage::SourcePath("/payload/editor-smoke"),
    },
    PayloadDescriptor {
        name: "server-smoke",
        path: "/payload/server-smoke",
        summary: "server smoke payload",
        entry_name: "payload_server_smoke",
        image: PayloadImage::SourcePath("/payload/server-smoke"),
    },
];

const SAMPLE_PAYLOAD_READY_SOURCE_BYTES: &[u8] = b"reovim-payload-source-v1\nexit-status ready\n";
const SAMPLE_PAYLOAD_FAILED_SOURCE_BYTES: &[u8] = b"reovim-payload-source-v1\nexit-status failed\n";
const SAMPLE_PAYLOAD_SERVICE_READY_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nservice-ready editor\nexit-status ready\n";
const SAMPLE_PAYLOAD_SERVICE_TABLE_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nservice-ready editor\nwrite-service-table\nexit-status ready\n";
const SAMPLE_PAYLOAD_PROCESS_TABLE_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nwrite-process-table\nexit-status ready\n";
const SAMPLE_PAYLOAD_BOOT_PROFILE_DEVICE_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nwrite-boot-profile\nwrite-device-table\nexit-status ready\n";
const SAMPLE_PAYLOAD_EXEC_SOURCE_TABLES_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nwrite-exec-table\nwrite-pending-exec-table\nwrite-source-table\nexit-status ready\n";
const SAMPLE_PAYLOAD_SCHEDULER_DIAGNOSTICS_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nwrite-scheduler-state\nwrite-task-table\nwrite-wait-table\nwrite-syscall-table\nexit-status ready\n";
const SAMPLE_PAYLOAD_SERVICE_RESIDENT_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nservice-ready editor\nservice-hold\n";
const SAMPLE_PAYLOAD_EXEC_PWD_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nexec-bin pwd\nexit-status ready\n";
const SAMPLE_PAYLOAD_EXEC_SOURCE_EXIT_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nexec-bin source-exit\nexit-status ready\n";
const SAMPLE_PAYLOAD_EXEC_CAT_STDIN_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nexec-bin-stdin-hex 7061796c6f61642e6368696c642e737464696e0a cat\nexit-status ready\n";
const SAMPLE_PAYLOAD_PIPE_PWD_CAT_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\npipe-bin pwd -- cat\nexit-status ready\n";
const SAMPLE_PAYLOAD_PIPE_MEDIA_SLEEP_CAT_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\npipe-bin media-sleep -- cat\nwrite-stdout-hex 7061796c6f61642d706970652e61667465720a\nexit-status ready\n";
const SAMPLE_PAYLOAD_SPAWN_PWD_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nspawn-bin pwd\nexit-status ready\n";
const SAMPLE_PAYLOAD_SPAWN_WAIT_PWD_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nspawn-wait-bin pwd\nexit-status ready\n";
const SAMPLE_PAYLOAD_SPAWN_WAIT_MEDIA_SLEEP_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nspawn-wait-bin media-sleep\nwrite-stdout-hex 7061796c6f61642d737061776e2d776169742e61667465720a\nexit-status ready\n";
const SAMPLE_PAYLOAD_SPAWN_WAIT_SOURCE_EXIT_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nspawn-wait-bin source-exit\nexit-status ready\n";
const SAMPLE_PAYLOAD_SPAWN_KILL_PWD_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nspawn-kill-bin pwd\nexit-status ready\n";
const SAMPLE_PAYLOAD_SPAWN_YIELD_PWD_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nspawn-bin pwd\nyield-now\nwrite-stdout-hex 7061796c6f61642e726573756d65640a\nexit-status ready\n";
const SAMPLE_PAYLOAD_SLEEP_TICK_YIELD_LS_BOOT_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nsleep-bin 1 ls /boot\nscheduler-tick\nyield-now\nwrite-stdout-hex 7061796c6f61642e736c6565702e726573756d65640a\nexit-status ready\n";
const SAMPLE_PAYLOAD_SLEEP_WAIT_PWD_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nsleep-wait-bin 2 pwd\nexit-status ready\n";
const SAMPLE_PAYLOAD_TTY_READ_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nread-tty-line\nexit-status ready\n";
const SAMPLE_PAYLOAD_STDOUT_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nwrite-stdout-hex 7061796c6f61642e7374646f75740a\nwrite-process-self\nwrite-vfs-file /boot/profile\nexit-status ready\n";
const SAMPLE_PAYLOAD_STDERR_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nwrite-stderr-hex 7061796c6f61642e7374646572720a\nexit-status ready\n";
const SAMPLE_PAYLOAD_EXIT_CODE_SEVEN_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nwrite-stdout-hex 7061796c6f61642d657869742d370a\nexit-code 7\n";
const SAMPLE_PAYLOAD_EXEC_SERVER_SMOKE_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nexec-payload server-smoke\nexit-status ready\n";
const SAMPLE_PAYLOAD_SPAWN_SERVER_SMOKE_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nspawn-payload server-smoke\nexit-status ready\n";
const SAMPLE_PAYLOAD_SPAWN_WAIT_SERVER_SMOKE_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nspawn-wait-payload server-smoke\nexit-status ready\n";
const SAMPLE_PAYLOAD_SPAWN_WAIT_BLOCKED_PAYLOAD_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nspawn-wait-payload server-smoke\nwrite-stdout-hex 7061796c6f61642d737061776e2d776169742d7061796c6f61642e61667465720a\nexit-status ready\n";
const SAMPLE_PAYLOAD_BLOCKING_BIN_WAIT_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nspawn-wait-bin media-sleep\nwrite-stdout-hex 6368696c642d7061796c6f61642e61667465720a\nexit-status ready\n";
const SAMPLE_PAYLOAD_SPAWN_KILL_SERVER_SMOKE_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nspawn-kill-payload server-smoke\nexit-status ready\n";
const SAMPLE_PAYLOAD_SPAWN_YIELD_SERVER_SMOKE_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nspawn-payload server-smoke\nyield-now\nwrite-stdout-hex 7061796c6f61642e726573756d65640a\nexit-status ready\n";
const SAMPLE_MEDIA_BIN_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nwrite-stdout-hex 6d656469612d62696e2e6f6b0a\nexit-status ok\n";
const SAMPLE_EXEC_BUNDLE_BIN_UAPI_BYTES: &[u8] =
    b"reovim-bin-uapi-v1\nwrite-stdout-hex 657865632d626f64792d756170692e6f6b0a\nopen-readonly-write-stdout-env-or-arg1-or REOVIM_MEDIA_PATH /boot/profile\nspawn-wait-bin-env REOVIM_MEDIA_PATH /boot/status hello\nexit-status ok\n";
const SAMPLE_EXEC_BUNDLE_BIN_UAPI_EXECVE_BYTES: &[u8] =
    b"reovim-bin-uapi-v1\nwrite-stdout-hex 657865632d626f64792d6578656376652e6f6b0a\nexec-bin-env REOVIM_MEDIA_PATH /boot/status hello\nwrite-stdout-hex 657865632d626f64792d6578656376652e7374616c650a\nexit-code 7\n";
const SAMPLE_EXEC_BUNDLE_BIN_UAPI_SPAWN_BYTES: &[u8] =
    b"reovim-bin-uapi-v1\nwrite-stdout-hex 657865632d626f64792d737061776e2e6f6b0a\nspawn-bin-env REOVIM_MEDIA_PATH /boot/status hello\nexit-status ok\n";
const SAMPLE_EXEC_BUNDLE_BIN_UAPI_YIELD_BYTES: &[u8] =
    b"reovim-bin-uapi-v1\nwrite-stdout-hex 657865632d626f64792d7969656c642e6265666f72650a\nspawn-bin-env REOVIM_MEDIA_PATH /boot/status hello\nyield-now\nwrite-stdout-hex 657865632d626f64792d7969656c642e61667465720a\nexit-status ok\n";
const SAMPLE_EXEC_BUNDLE_BIN_UAPI_TICK_BYTES: &[u8] =
    b"reovim-bin-uapi-v1\nwrite-stdout-hex 657865632d626f64792d7469636b2e6265666f72650a\nspawn-sleep-bin-env 1 REOVIM_MEDIA_PATH /boot/status hello\nscheduler-tick\nyield-now\nwrite-stdout-hex 657865632d626f64792d7469636b2e61667465720a\nexit-status ok\n";
const SAMPLE_EXEC_BUNDLE_BIN_UAPI_SLEEP_BYTES: &[u8] =
    b"reovim-bin-uapi-v1\nwrite-stdout-hex 657865632d626f64792d736c6565702e6265666f72650a\nsleep-ticks 1\nwrite-stdout-hex 657865632d626f64792d736c6565702e61667465720a\nexit-status ok\n";
const SAMPLE_EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_BYTES: &[u8] =
    b"reovim-bin-uapi-v1\nwrite-stdout-hex 657865632d626f64792d776169742e6265666f72650a\nspawn-wait-bin media-sleep\nwrite-stdout-hex 657865632d626f64792d776169742e7374616c650a\nexit-code 7\n";
const SAMPLE_MEDIA_PAYLOAD_SOURCE_BYTES: &[u8] =
    b"reovim-payload-source-v1\nexec-bin pwd\nexit-status ready\n";
const SAMPLE_BIN_EXIT_CODE_ZERO_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nwrite-stdout-hex 657869742d636f64652d300a\nexit-code 0\n";
const SAMPLE_BIN_EXIT_CODE_SEVEN_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nwrite-stdout-hex 657869742d636f64652d370a\nexit-code 7\n";
const SAMPLE_BIN_INVALID_EXIT_CODE_SOURCE_BYTES: &[u8] = b"reovim-source-v1\nexit-code 256\n";
const SOURCE_MEDIA_BIN: usize = 1;
const SOURCE_MEDIA_PAYLOAD: usize = 2;
const SOURCE_MEDIA_BIN_WRONG_PATH: usize = 3;
const SOURCE_MEDIA_DYNAMIC_BIN: usize = 4;
const SOURCE_MEDIA_DYNAMIC_PAYLOAD: usize = 5;
static SOURCE_MEDIA_KIND: AtomicUsize = AtomicUsize::new(0);
const EXEC_BUNDLE_NONE: usize = 0;
const EXEC_BUNDLE_BIN_UAPI_DYNAMIC: usize = 1;
const EXEC_BUNDLE_BIN_UAPI_EXECVE_DYNAMIC: usize = 2;
const EXEC_BUNDLE_BIN_UAPI_SPAWN_DYNAMIC: usize = 3;
const EXEC_BUNDLE_BIN_UAPI_YIELD_DYNAMIC: usize = 4;
const EXEC_BUNDLE_BIN_UAPI_TICK_DYNAMIC: usize = 5;
const EXEC_BUNDLE_BIN_UAPI_SLEEP_DYNAMIC: usize = 6;
const EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_DYNAMIC: usize = 7;
const EXEC_BUNDLE_BIN_UAPI_PIPE_WRITE_DYNAMIC: usize = 8;
const EXEC_BUNDLE_BIN_UAPI_FD_COPY_DYNAMIC: usize = 9;
const EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_PARENT_OFFSET: usize =
    crate::source_store::MAX_SOURCE_MEDIA_ARTIFACT_BYTES;
const EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_CHILD_OFFSET: usize =
    crate::source_store::MAX_SOURCE_MEDIA_ARTIFACT_BYTES * 2;
const EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_CAPACITY: usize =
    crate::source_store::MAX_SOURCE_MEDIA_ARTIFACT_BYTES * 3;
static EXEC_BUNDLE_KIND: AtomicUsize = AtomicUsize::new(EXEC_BUNDLE_NONE);

fn source_media_write(_offset: usize, _bytes: &[u8]) -> bool {
    false
}

fn source_media_copy(out: &mut [u8], len: &mut usize, bytes: &[u8]) -> bool {
    if *len + bytes.len() > out.len() {
        return false;
    }
    out[*len..*len + bytes.len()].copy_from_slice(bytes);
    *len += bytes.len();
    true
}

fn source_media_copy_usize(out: &mut [u8], len: &mut usize, mut value: usize) -> bool {
    let mut digits = [0u8; 20];
    let mut digit_count = 0usize;
    if value == 0 {
        digits[0] = b'0';
        digit_count = 1;
    } else {
        while value > 0 {
            digits[digit_count] = b'0' + (value % 10) as u8;
            value /= 10;
            digit_count += 1;
        }
    }

    while digit_count > 0 {
        digit_count -= 1;
        if !source_media_copy(out, len, &digits[digit_count..digit_count + 1]) {
            return false;
        }
    }
    true
}

fn source_media_encode(out: &mut [u8], namespace: &[u8], path: &[u8], source: &[u8]) -> usize {
    let mut len = 0usize;
    let checksum = crate::source_store::source_media_checksum32(source) as usize;
    if !source_media_copy(out, &mut len, b"reovim-source-media-v1\nnamespace=")
        || !source_media_copy(out, &mut len, namespace)
        || !source_media_copy(out, &mut len, b"\npath=")
        || !source_media_copy(out, &mut len, path)
        || !source_media_copy(out, &mut len, b"\nbytes=")
        || !source_media_copy_usize(out, &mut len, source.len())
        || !source_media_copy(out, &mut len, b"\nchecksum=")
        || !source_media_copy_usize(out, &mut len, checksum)
        || !source_media_copy(out, &mut len, b"\n")
        || !source_media_copy(out, &mut len, source)
    {
        return 0;
    }
    len
}

fn exec_body_encode(out: &mut [u8], inner: &[u8], body: &[u8]) -> usize {
    let mut len = 0usize;
    let checksum = crate::source_store::source_media_checksum32(body) as usize;
    if !source_media_copy(out, &mut len, b"reovim-exec-body-v1\ninner=")
        || !source_media_copy(out, &mut len, inner)
        || !source_media_copy(out, &mut len, b"\nbytes=")
        || !source_media_copy_usize(out, &mut len, body.len())
        || !source_media_copy(out, &mut len, b"\nchecksum=")
        || !source_media_copy_usize(out, &mut len, checksum)
        || !source_media_copy(out, &mut len, b"\n")
        || !source_media_copy(out, &mut len, body)
    {
        return 0;
    }
    len
}

fn exec_bundle_encode(out: &mut [u8], namespace: &[u8], path: &[u8], body: &[u8]) -> usize {
    let mut len = 0usize;
    let checksum = crate::source_store::source_media_checksum32(body) as usize;
    if !source_media_copy(out, &mut len, b"reovim-exec-bundle-v1\nnamespace=")
        || !source_media_copy(out, &mut len, namespace)
        || !source_media_copy(out, &mut len, b"\npath=")
        || !source_media_copy(out, &mut len, path)
        || !source_media_copy(out, &mut len, b"\nbytes=")
        || !source_media_copy_usize(out, &mut len, body.len())
        || !source_media_copy(out, &mut len, b"\nchecksum=")
        || !source_media_copy_usize(out, &mut len, checksum)
        || !source_media_copy(out, &mut len, b"\n")
        || !source_media_copy(out, &mut len, body)
    {
        return 0;
    }
    len
}

fn exec_bundle_encode_exec_body(
    out: &mut [u8],
    namespace: &[u8],
    path: &[u8],
    inner: &[u8],
    body: &[u8],
) -> usize {
    let mut exec_body = [0u8; crate::source_store::MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let exec_body_len = exec_body_encode(&mut exec_body, inner, body);
    exec_bundle_encode(out, namespace, path, &exec_body[..exec_body_len])
}

fn exec_bundle_catalog_entry(
    out: &mut [u8],
    len: &mut usize,
    namespace: &[u8],
    path: &[u8],
    offset: usize,
    bytes_len: usize,
    checksum: usize,
) -> bool {
    source_media_copy(out, len, b"entry namespace=")
        && source_media_copy(out, len, namespace)
        && source_media_copy(out, len, b" path=")
        && source_media_copy(out, len, path)
        && source_media_copy(out, len, b" offset=")
        && source_media_copy_usize(out, len, offset)
        && source_media_copy(out, len, b" bytes=")
        && source_media_copy_usize(out, len, bytes_len)
        && source_media_copy(out, len, b" checksum=")
        && source_media_copy_usize(out, len, checksum)
        && source_media_copy(out, len, b"\n")
}

fn exec_bundle_encode_wait_sleep_catalog(out: &mut [u8]) -> usize {
    let mut parent_artifact = [0u8; crate::source_store::MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let parent_len = exec_bundle_encode_exec_body(
        &mut parent_artifact,
        b"bin",
        b"/bin/media-wait-sleep",
        b"bin-uapi-v1",
        SAMPLE_EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_BYTES,
    );
    let parent_checksum =
        crate::source_store::source_media_checksum32(&parent_artifact[..parent_len]) as usize;

    let mut child_artifact = [0u8; crate::source_store::MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let child_len = exec_bundle_encode_exec_body(
        &mut child_artifact,
        b"bin",
        b"/bin/media-sleep",
        b"bin-uapi-v1",
        SAMPLE_EXEC_BUNDLE_BIN_UAPI_SLEEP_BYTES,
    );
    let child_checksum =
        crate::source_store::source_media_checksum32(&child_artifact[..child_len]) as usize;

    let mut body = [0u8; crate::source_store::MAX_SOURCE_MEDIA_CATALOG_BYTES];
    let mut body_len = 0usize;
    if !exec_bundle_catalog_entry(
        &mut body,
        &mut body_len,
        b"bin",
        b"/bin/media-wait-sleep",
        EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_PARENT_OFFSET,
        parent_len,
        parent_checksum,
    ) || !exec_bundle_catalog_entry(
        &mut body,
        &mut body_len,
        b"bin",
        b"/bin/media-sleep",
        EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_CHILD_OFFSET,
        child_len,
        child_checksum,
    ) {
        return 0;
    }

    let checksum = crate::source_store::source_media_checksum32(&body[..body_len]) as usize;
    let mut len = 0usize;
    if !source_media_copy(out, &mut len, b"reovim-exec-bundle-catalog-v1\nbytes=")
        || !source_media_copy_usize(out, &mut len, body_len)
        || !source_media_copy(out, &mut len, b"\nchecksum=")
        || !source_media_copy_usize(out, &mut len, checksum)
        || !source_media_copy(out, &mut len, b"\n")
        || !source_media_copy(out, &mut len, &body[..body_len])
    {
        return 0;
    }
    len
}

fn exec_bundle_encode_pipe_write_body(out: &mut [u8]) -> usize {
    let mut body = [0u8; crate::source_store::MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let mut body_len = 0usize;
    if !source_media_copy(&mut body, &mut body_len, b"reovim-bin-uapi-v1\nwrite-stdout-hex ") {
        return 0;
    }
    let mut index = 0usize;
    while index <= crate::program::MAX_PROGRAM_PIPE_BYTES {
        if !source_media_copy(&mut body, &mut body_len, b"41") {
            return 0;
        }
        index += 1;
    }
    if !source_media_copy(&mut body, &mut body_len, b"\nexit-status ok\n") {
        return 0;
    }

    exec_bundle_encode_exec_body(
        out,
        b"bin",
        b"/bin/media-pipe-write",
        b"bin-uapi-v1",
        &body[..body_len],
    )
}

fn exec_bundle_encode_fd_copy_body(out: &mut [u8]) -> usize {
    let mut body = [0u8; crate::source_store::MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let mut body_len = 0usize;
    if !source_media_copy(&mut body, &mut body_len, b"reovim-bin-uapi-v1\nwrite-stdout-hex ") {
        return 0;
    }
    let mut index = 0usize;
    while index < crate::program::MAX_PROGRAM_PIPE_BYTES {
        if !source_media_copy(&mut body, &mut body_len, b"41") {
            return 0;
        }
        index += 1;
    }
    if !source_media_copy(
        &mut body,
        &mut body_len,
        b"\nopen-readonly-write-stdout /boot/status\nexit-status ok\n",
    ) {
        return 0;
    }

    exec_bundle_encode_exec_body(
        out,
        b"bin",
        b"/bin/media-fd-copy",
        b"bin-uapi-v1",
        &body[..body_len],
    )
}

fn source_media_read(offset: usize, out: &mut [u8]) -> usize {
    if offset != 0 {
        return 0;
    }
    let (namespace, path, bytes) = match SOURCE_MEDIA_KIND.load(Ordering::Relaxed) {
        SOURCE_MEDIA_BIN => {
            (b"bin".as_slice(), b"/bin/proc".as_slice(), SAMPLE_MEDIA_BIN_SOURCE_BYTES)
        }
        SOURCE_MEDIA_DYNAMIC_BIN => {
            (b"bin".as_slice(), b"/bin/media-bin".as_slice(), SAMPLE_MEDIA_BIN_SOURCE_BYTES)
        }
        SOURCE_MEDIA_PAYLOAD => (
            b"payload".as_slice(),
            b"/payload/server-smoke".as_slice(),
            SAMPLE_MEDIA_PAYLOAD_SOURCE_BYTES,
        ),
        SOURCE_MEDIA_DYNAMIC_PAYLOAD => (
            b"payload".as_slice(),
            b"/payload/media-payload".as_slice(),
            SAMPLE_MEDIA_PAYLOAD_SOURCE_BYTES,
        ),
        SOURCE_MEDIA_BIN_WRONG_PATH => {
            (b"bin".as_slice(), b"/bin/ls".as_slice(), SAMPLE_MEDIA_BIN_SOURCE_BYTES)
        }
        _ => return 0,
    };
    source_media_encode(out, namespace, path, bytes)
}

fn exec_bundle_write(_offset: usize, _bytes: &[u8]) -> bool {
    false
}

fn exec_bundle_read(offset: usize, out: &mut [u8]) -> usize {
    match EXEC_BUNDLE_KIND.load(Ordering::Relaxed) {
        EXEC_BUNDLE_BIN_UAPI_DYNAMIC if offset == 0 => exec_bundle_encode_exec_body(
            out,
            b"bin",
            b"/bin/media-bin",
            b"bin-uapi-v1",
            SAMPLE_EXEC_BUNDLE_BIN_UAPI_BYTES,
        ),
        EXEC_BUNDLE_BIN_UAPI_EXECVE_DYNAMIC if offset == 0 => exec_bundle_encode_exec_body(
            out,
            b"bin",
            b"/bin/media-exec",
            b"bin-uapi-v1",
            SAMPLE_EXEC_BUNDLE_BIN_UAPI_EXECVE_BYTES,
        ),
        EXEC_BUNDLE_BIN_UAPI_SPAWN_DYNAMIC if offset == 0 => exec_bundle_encode_exec_body(
            out,
            b"bin",
            b"/bin/media-spawn",
            b"bin-uapi-v1",
            SAMPLE_EXEC_BUNDLE_BIN_UAPI_SPAWN_BYTES,
        ),
        EXEC_BUNDLE_BIN_UAPI_YIELD_DYNAMIC if offset == 0 => exec_bundle_encode_exec_body(
            out,
            b"bin",
            b"/bin/media-yield",
            b"bin-uapi-v1",
            SAMPLE_EXEC_BUNDLE_BIN_UAPI_YIELD_BYTES,
        ),
        EXEC_BUNDLE_BIN_UAPI_TICK_DYNAMIC if offset == 0 => exec_bundle_encode_exec_body(
            out,
            b"bin",
            b"/bin/media-tick",
            b"bin-uapi-v1",
            SAMPLE_EXEC_BUNDLE_BIN_UAPI_TICK_BYTES,
        ),
        EXEC_BUNDLE_BIN_UAPI_SLEEP_DYNAMIC if offset == 0 => exec_bundle_encode_exec_body(
            out,
            b"bin",
            b"/bin/media-sleep",
            b"bin-uapi-v1",
            SAMPLE_EXEC_BUNDLE_BIN_UAPI_SLEEP_BYTES,
        ),
        EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_DYNAMIC if offset == 0 => {
            exec_bundle_encode_wait_sleep_catalog(out)
        }
        EXEC_BUNDLE_BIN_UAPI_PIPE_WRITE_DYNAMIC if offset == 0 => {
            exec_bundle_encode_pipe_write_body(out)
        }
        EXEC_BUNDLE_BIN_UAPI_FD_COPY_DYNAMIC if offset == 0 => exec_bundle_encode_fd_copy_body(out),
        EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_DYNAMIC
            if offset == EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_PARENT_OFFSET =>
        {
            exec_bundle_encode_exec_body(
                out,
                b"bin",
                b"/bin/media-wait-sleep",
                b"bin-uapi-v1",
                SAMPLE_EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_BYTES,
            )
        }
        EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_DYNAMIC
            if offset == EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_CHILD_OFFSET =>
        {
            exec_bundle_encode_exec_body(
                out,
                b"bin",
                b"/bin/media-sleep",
                b"bin-uapi-v1",
                SAMPLE_EXEC_BUNDLE_BIN_UAPI_SLEEP_BYTES,
            )
        }
        _ => 0,
    }
}

fn install_source_media(kind: usize) {
    SOURCE_MEDIA_KIND.store(kind, Ordering::Relaxed);
    crate::block::install_source_media_device(BlockDevice::new(
        "selftest-source-media0",
        crate::source_store::MAX_SOURCE_MEDIA_ARTIFACT_BYTES,
        source_media_write,
        source_media_read,
    ));
}

fn install_exec_bundle(kind: usize) {
    EXEC_BUNDLE_KIND.store(kind, Ordering::Relaxed);
    let capacity = if kind == EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_DYNAMIC {
        EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_CAPACITY
    } else {
        crate::source_store::MAX_SOURCE_MEDIA_ARTIFACT_BYTES
    };
    crate::block::install_exec_bundle_device(BlockDevice::new(
        "selftest-exec-bundle0",
        capacity,
        exec_bundle_write,
        exec_bundle_read,
    ));
}

fn clear_source_media() {
    SOURCE_MEDIA_KIND.store(0, Ordering::Relaxed);
    crate::block::clear_source_media_device_for_tests();
}

fn clear_exec_bundle() {
    EXEC_BUNDLE_KIND.store(EXEC_BUNDLE_NONE, Ordering::Relaxed);
    crate::block::clear_exec_bundle_device_for_tests();
}

const SAMPLE_PAYLOAD_SOURCES: [PayloadSourceArtifact; 3] = [
    PayloadSourceArtifact {
        path: "/payload/reovim",
        kind: PayloadImageKind::SourceImage,
        bytes: SAMPLE_PAYLOAD_READY_SOURCE_BYTES,
    },
    PayloadSourceArtifact {
        path: "/payload/editor-smoke",
        kind: PayloadImageKind::SourceImage,
        bytes: SAMPLE_PAYLOAD_READY_SOURCE_BYTES,
    },
    PayloadSourceArtifact {
        path: "/payload/server-smoke",
        kind: PayloadImageKind::SourceImage,
        bytes: SAMPLE_PAYLOAD_FAILED_SOURCE_BYTES,
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

fn sample_payload_sources() -> &'static [PayloadSourceArtifact] {
    &SAMPLE_PAYLOAD_SOURCES
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

fn probe_budget_fixture(
    target: &str,
    _devices: &[DeviceEntry],
    write: WriteFn,
) -> HardwareProbeResult {
    match target {
        "help" | "list" => {
            probe_budget_emit(write, b"probe targets:\n");
            probe_budget_emit(write, b"  pcie\n");
            probe_budget_emit(write, b"  usb-keyboard (alias: keyboard)\n");
            probe_budget_emit(
                write,
                b"  xhci-read-keyboard-report (alias: usb-keyboard-read-report)\n",
            );
            HardwareProbeResult::Handled
        }
        "pcie" => {
            probe_budget_emit(write, b"probe pcie:\n");
            probe_budget_emit(write, b"state=present\n");
            probe_budget_emit(write, b"raw_status=0x00000000\nrevision=0x00000000\n");
            probe_budget_emit(write, b"root_complex=true\nphy_link_up=true\n");
            probe_budget_emit(write, b"data_link_active=true\nlink_up=true\n");
            probe_budget_emit(write, b"xhci=present\n");
            probe_budget_emit(write, b"xhci.bus=1\nxhci.device=0\nxhci.function=0\n");
            probe_budget_emit(write, b"xhci.vendor=0x00001106\nxhci.device_id=0x00003483\n");
            probe_budget_emit(write, b"xhci.revision=1\nxhci.mmio=0x600000000\n");
            probe_budget_emit(write, b"xhci.cap_length=64\nxhci.hci_version=0x00000100\n");
            probe_budget_emit(write, b"xhci.max_slots=32\nxhci.max_interrupters=8\n");
            probe_budget_emit(write, b"xhci.max_ports=5\nxhci.doorbell_offset=0x00001000\n");
            probe_budget_emit(write, b"xhci.runtime_offset=0x00002000\n");
            probe_budget_emit(write, b"xhci.usbcmd=0x00000001\nxhci.usbsts=0x00000000\n");
            probe_budget_emit(write, b"xhci.pagesize=0x00000001\nxhci.config=0x00000020\n");
            probe_budget_emit(write, b"xhci.enabled_slots=1\nxhci.running=true\n");
            probe_budget_emit(write, b"xhci.halted=false\nxhci.reset_active=false\n");
            probe_budget_emit(write, b"xhci.controller_not_ready=false\n");
            probe_budget_emit(write, b"xhci.host_system_error=false\n");
            probe_budget_emit(write, b"xhci.memory=planned\n");
            probe_budget_emit(write, b"xhci.memory.dcbaa=0x0000000000100000\n");
            probe_budget_emit(write, b"xhci.memory.command_ring=0x0000000000101000\n");
            probe_budget_emit(write, b"xhci.memory.crcr=0x0000000000101001\n");
            probe_budget_emit(write, b"xhci.memory.event_ring=0x0000000000102000\n");
            probe_budget_emit(write, b"xhci.memory.control_endpoint_ring=0x0000000000103000\n");
            probe_budget_emit(
                write,
                b"xhci.memory.interrupt_in_endpoint_ring=0x0000000000104000\n",
            );
            probe_budget_emit(write, b"xhci.memory.erst=0x0000000000105000\n");
            probe_budget_emit(write, b"xhci.memory.erdp=0x0000000000102000\n");
            probe_budget_emit(write, b"xhci.memory.max_slots=32\n");
            probe_budget_emit(write, b"xhci.memory.context_size=32\n");
            probe_budget_emit(write, b"xhci.memory.scratchpads=0\n");
            probe_budget_emit(
                write,
                b"xhci.port1.portsc=0x00000203 connected=true enabled=true powered=true speed=2 link_state=0\n",
            );
            probe_budget_emit(
                write,
                b"xhci.port2.portsc=0x000002a0 connected=false enabled=false powered=true speed=0 link_state=5\n",
            );
            HardwareProbeResult::Handled
        }
        "usb-keyboard" => {
            probe_budget_emit(write, b"probe usb-keyboard:\n");
            probe_budget_emit(write, b"state=report-ready\n");
            probe_budget_emit(write, b"report_bytes=8\n");
            probe_budget_emit(write, b"decoded_bytes=1\n");
            HardwareProbeResult::Handled
        }
        _ => HardwareProbeResult::UnknownTarget,
    }
}

fn probe_budget_emit(write: WriteFn, bytes: &[u8]) {
    write(bytes);
    crate::klog::append_bytes(bytes);
}

fn halt_fixture() {
    HALT_CALLS.fetch_add(1, Ordering::Relaxed);
}

fn read_line_fixture(out: &mut [u8]) -> usize {
    let call = TTY_READ_CALLS.fetch_add(1, Ordering::Relaxed);
    let bytes: &[u8] = if call == 0 { b"tty fixture input" } else { b"" };
    let mut copied = 0usize;
    while copied < bytes.len() && copied < out.len() {
        out[copied] = bytes[copied];
        copied += 1;
    }
    copied
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    let mut i = 0usize;
    while i + needle.len() <= haystack.len() {
        let mut match_len = 0usize;
        while match_len < needle.len() && haystack[i + match_len] == needle[match_len] {
            match_len += 1;
        }
        if match_len == needle.len() {
            return true;
        }
        i += 1;
    }
    false
}

fn assert_contains(haystack: &[u8], needle: &[u8]) {
    if contains_bytes(haystack, needle) {
        return;
    }
    testrt::check(false, missing_chunk_message(needle));
}

fn assert_contains_with_message(haystack: &[u8], needle: &[u8], message: &'static str) {
    testrt::check(contains_bytes(haystack, needle), message);
}

fn assert_contains_prefixed_usize(
    haystack: &[u8],
    prefix: &[u8],
    mut value: usize,
    message: &'static str,
) {
    let mut needle = [0u8; 96];
    testrt::check(prefix.len() < needle.len(), message);
    let mut len = 0usize;
    while len < prefix.len() {
        needle[len] = prefix[len];
        len += 1;
    }

    let mut digits = [0u8; 20];
    let mut digit_count = 0usize;
    if value == 0 {
        digits[0] = b'0';
        digit_count = 1;
    } else {
        while value > 0 && digit_count < digits.len() {
            digits[digit_count] = b'0' + (value % 10) as u8;
            value /= 10;
            digit_count += 1;
        }
    }
    while digit_count > 0 && len < needle.len() {
        digit_count -= 1;
        needle[len] = digits[digit_count];
        len += 1;
    }

    testrt::check(contains_bytes(haystack, &needle[..len]), message);
}

fn assert_syscall_record(
    path: &'static str,
    op: crate::syscall::SyscallOp,
    status: crate::syscall::SyscallStatus,
) {
    assert_syscall_record_with_message(path, op, status, "missing syscall lifecycle record");
}

fn assert_syscall_record_with_message(
    path: &'static str,
    op: crate::syscall::SyscallOp,
    status: crate::syscall::SyscallStatus,
    message: &'static str,
) {
    let mut records = [crate::syscall::EMPTY_SYSCALL_RECORD; crate::syscall::MAX_SYSCALL_RECORDS];
    let count = crate::syscall::snapshot_syscalls(&mut records);
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == path && record.op == op && record.status == status {
            return;
        }
        index += 1;
    }
    testrt::check(false, message);
}

fn syscall_record_count(
    path: &'static str,
    op: crate::syscall::SyscallOp,
    status: crate::syscall::SyscallStatus,
) -> usize {
    let mut records = [crate::syscall::EMPTY_SYSCALL_RECORD; crate::syscall::MAX_SYSCALL_RECORDS];
    let count = crate::syscall::snapshot_syscalls(&mut records);
    let mut found = 0usize;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == path && record.op == op && record.status == status {
            found += 1;
        }
        index += 1;
    }
    found
}

fn assert_event_record(
    component: &'static str,
    severity: &'static str,
    kind: &'static str,
    pid: usize,
    task: usize,
) {
    let mut records = [crate::klog::EMPTY_EVENT_RECORD; crate::klog::MAX_EVENTS];
    let count = crate::klog::snapshot_events(&mut records);
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.source == "process"
            && record.component == component
            && record.severity == severity
            && record.kind == kind
            && record.process_id == pid
            && record.task_id == task
        {
            return;
        }
        index += 1;
    }
    testrt::check(false, "missing structured event record");
}

fn assert_user_resume_blocked(ctx: crate::syscall::SyscallContext) {
    let process = crate::proc::process(ctx.pid).expect("user-resume process retained");
    testrt::check_eq(process.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(process.block_reason, crate::sched::BlockReason::UserResume);
    let task = crate::sched::task(ctx.task_id).expect("user-resume task retained");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Blocked);
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::UserResume);
    assert_syscall_record(
        ctx.program_path,
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Blocked,
    );
    assert_event_record("proc", "info", "process-block", ctx.pid, ctx.task_id);
}

fn assert_blocked_kernel_log_stats(stats: crate::klog::Stats) {
    testrt::check_eq(stats.identity.boot_id, 0usize);
    testrt::check_eq(stats.identity.session_id, 0usize);
    testrt::check_eq(stats.identity.identity_source, "blocked");
    testrt::check_eq(stats.capacity_bytes, 0usize);
    testrt::check_eq(stats.retained_bytes, 0usize);
    testrt::check_eq(stats.dropped_bytes, 0usize);
    testrt::check_eq(stats.next_event_seq, 0usize);
    testrt::check_eq(stats.retained_events, 0usize);
}

fn assert_blocked_dump_sync_status(status: crate::dump::DumpSyncStatus) {
    testrt::check_eq(status.attempted, false);
    testrt::check_eq(status.persistent_available, false);
    testrt::check_eq(status.storage, "blocked");
    testrt::check_eq(status.storage_capacity_bytes, 0usize);
    testrt::check_eq(status.written, false);
    testrt::check_eq(status.bytes_written, 0usize);
    testrt::check_eq(status.checksum, 0u32);
    testrt::check_eq(status.verified, false);
    testrt::check_eq(status.reason, "blocked");
}

fn assert_blocked_dump_status(status: crate::dump::DumpStatus) {
    testrt::check_eq(status.format_version, 0usize);
    testrt::check_eq(status.identity.boot_id, 0usize);
    testrt::check_eq(status.identity.session_id, 0usize);
    testrt::check_eq(status.identity.identity_source, "blocked");
    testrt::check_eq(status.image.package, "blocked");
    testrt::check_eq(status.image.version, "blocked");
    testrt::check_eq(status.image.target, "blocked");
    testrt::check_eq(status.image.selected_profile, "blocked");
    testrt::check_eq(status.image.profile_request, "blocked");
    testrt::check_eq(status.image.bootline, "blocked");
    testrt::check_eq(status.image.launch_profile_feature, "blocked");
    testrt::check_eq(status.boot_info.memory.range_count(), 0usize);
    testrt::check_eq(status.boot_info.memory.usable_bytes(), 0u64);
    testrt::check_eq(status.boot_info.cpu_count, 0u32);
    testrt::check_eq(status.boot_info.heap_total_bytes, 0u64);
    testrt::check_eq(status.device_records, 0usize);
    let mut index = 0usize;
    while index < status.devices.len() {
        let device = status.devices[index];
        testrt::check_eq(device.class, reovim_uapi_system::DeviceClass::Unknown);
        testrt::check_eq(device.mmio_base, 0u64);
        testrt::check_eq(device.mmio_len, 0u64);
        testrt::check_eq(device.irq, u32::MAX);
        testrt::check_eq(device.capacity_bytes, 0u64);
        testrt::check_eq(device.compatible, "");
        index += 1;
    }
    testrt::check_eq(status.proof_state, "blocked");
    testrt::check_eq(status.panic_state, "blocked");
    testrt::check_eq(status.panic_records, 0usize);
    testrt::check(
        status.panic_record.is_none(),
        "blocked dump status should not include panic evidence",
    );
    testrt::check_eq(status.persistent_available, false);
    testrt::check_eq(status.storage, "blocked");
    testrt::check_eq(status.storage_capacity_bytes, 0usize);
    assert_blocked_dump_sync_status(status.last_sync);
    assert_blocked_kernel_log_stats(status.klog);
    testrt::check_eq(status.event_records, 0usize);
    testrt::check_eq(status.process_records, 0usize);
    testrt::check_eq(status.service_records, 0usize);
    testrt::check_eq(status.exec_load_records, 0usize);
    testrt::check_eq(status.pending_exec_records, 0usize);
    testrt::check_eq(status.wait_records, 0usize);
    testrt::check_eq(status.task_records, 0usize);
    testrt::check_eq(status.syscall_records, 0usize);
    testrt::check_eq(status.syscall_continuation_records, 0usize);
}

fn assert_blocked_boot_info(info: BootInfo) {
    testrt::check_eq(info.memory.range_count(), 0usize);
    testrt::check_eq(info.memory.usable_bytes(), 0u64);
    testrt::check_eq(info.cpu_freq_hz, 0u64);
    testrt::check_eq(info.cpu_id, 0u32);
    testrt::check_eq(info.cpu_count, 0u32);
    testrt::check_eq(info.cache_line_bytes, 0u32);
    testrt::check_eq(info.l1d_bytes, 0u32);
    testrt::check_eq(info.l1i_bytes, 0u32);
    testrt::check_eq(info.l2_bytes, 0u32);
    testrt::check_eq(info.cpu_affinity, 0u64);
    testrt::check_eq(info.mem_freq_hz, 0u64);
    testrt::check_eq(info.heap_total_bytes, 0u64);
}

fn assert_blocked_boot_image(image: crate::rootd::BootImageSummary) {
    testrt::check_eq(image.package, "blocked");
    testrt::check_eq(image.version, "blocked");
    testrt::check_eq(image.target, "blocked");
    testrt::check_eq(image.selected_profile, "blocked");
    testrt::check_eq(image.profile_request, "blocked");
    testrt::check_eq(image.bootline, "blocked");
    testrt::check_eq(image.launch_profile_feature, "blocked");
}

fn assert_blocked_console_input(input: crate::rootd::ConsoleInputSummary) {
    testrt::check_eq(input.source, "blocked");
    testrt::check_eq(input.mode, "blocked");
    testrt::check_eq(input.source_state, crate::rootd::BootCheckState::Warn);
    testrt::check_eq(input.usb_keyboard, crate::rootd::BootCheckState::Warn);
    testrt::check_eq(input.usb_keyboard_pending_bytes, 0usize);
    testrt::check_eq(input.usb_keyboard_probe_enabled, false);
    testrt::check_eq(input.usb_keyboard_poll_interval_ms, 0usize);
    testrt::check_eq(input.usb_keyboard_last_poll, "blocked");
}

fn assert_blocked_scheduler_snapshot(snapshot: crate::sched::SchedulerSnapshot) {
    testrt::check_eq(snapshot.current_task_id, usize::MAX);
    testrt::check_eq(snapshot.current_process_id, usize::MAX);
    testrt::check_eq(snapshot.ready_len, 0usize);
    testrt::check_eq(snapshot.next_ready_task_id(), 0usize);
    testrt::check_eq(snapshot.next_ready_process_id(), 0usize);
    testrt::check_eq(snapshot.dispatch_count, usize::MAX);
    testrt::check_eq(snapshot.yield_count, usize::MAX);
    testrt::check_eq(snapshot.tick_count, usize::MAX);
}

fn assert_no_syscall_record(
    path: &'static str,
    op: crate::syscall::SyscallOp,
    status: crate::syscall::SyscallStatus,
) {
    let mut records = [crate::syscall::EMPTY_SYSCALL_RECORD; crate::syscall::MAX_SYSCALL_RECORDS];
    let count = crate::syscall::snapshot_syscalls(&mut records);
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == path && record.op == op && record.status == status {
            testrt::check(false, "unexpected syscall lifecycle record");
            return;
        }
        index += 1;
    }
}

fn missing_chunk_message(needle: &[u8]) -> &'static str {
    if needle == b"reovim root shell\n" {
        return "missing boot help title";
    }
    if needle == b"/bin programs: help, init, sh, clear, screentest" {
        return "missing /bin-backed help program list";
    }
    if needle == b"namespace: /bin\n" {
        return "missing boot help namespace row";
    }
    if needle == b"usage: help [program]\n" {
        return "missing boot help usage row";
    }
    if needle == b"details:\n" {
        return "missing boot help details row";
    }
    if needle == b"bin\n" {
        return "missing root bin directory row";
    }
    if needle == b"exec-body-uapi.ok\n" {
        return "missing executable-body stdout marker";
    }
    if needle == b"profile=shell-only\n" {
        return "missing executable-body profile copy";
    }
    if needle == b"launch=disabled\n" {
        return "missing executable-body launch copy";
    }
    if needle == b"hello from linked bin\n" {
        return "missing executable-body child stdout";
    }
    if needle == b"boot\n" {
        return "missing root boot directory row";
    }
    if needle == b"dev\n" {
        return "missing root dev directory row";
    }
    if needle == b"dump\n" {
        return "missing root dump directory row";
    }
    if needle == b"log\n" {
        return "missing root log directory row";
    }
    if needle == b"proc\n" {
        return "missing root proc directory row";
    }
    if needle == b"devices\n" {
        return "missing boot devices file row";
    }
    if needle == b"help\n" {
        return "missing boot help file row";
    }
    if needle == b"image\n" {
        return "missing boot image file row";
    }
    if needle == b"input\n" {
        return "missing boot input file row";
    }
    if needle == b"memory\n" {
        return "missing boot memory file row";
    }
    if needle == b"mounts\n" {
        return "missing boot mounts file row";
    }
    if needle == b"kernel on / type rootfs (ro,pseudo)\n" {
        return "missing kernel rootfs mount row";
    }
    if needle == b"boot on /boot type bootfs (ro,pseudo)\n" {
        return "missing bootfs mount row";
    }
    if needle == b"devices on /dev type devfs (ro,pseudo)\n" {
        return "missing devfs mount row";
    }
    if needle == b"klog on /log type logfs (ro,pseudo)\n" {
        return "missing logfs mount row";
    }
    if needle
        == b"exec.path=/bin/mount pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_mount\n"
    {
        return "missing linked mount success audit";
    }
    if needle
        == b"exec.path=/bin/mount pid=3 task=3 status=exit-code loader=linked-bin entry_fn=bin_mount\n"
    {
        return "missing linked mount error audit";
    }
    if needle == b"shell: halt\n" {
        return "missing halt shell input audit";
    }
    if needle == b"shell.status=halt\n" {
        return "missing halt shell status audit";
    }
    if needle == b"rootd: halt dump sync\n" {
        return "missing halt dump-sync start audit";
    }
    if needle
        == b"rootd.dump_sync persistent=unavailable storage=none storage_capacity_bytes=0 status=not-written bytes=0 checksum=0 verified=false reason=no-persistent-dump-sink\n"
    {
        return "missing halt dump-sync result audit";
    }
    if needle
        == b"exec.path=/bin/halt pid=3 task=3 status=halt loader=linked-bin entry_fn=bin_halt\n"
    {
        return "missing linked halt success audit";
    }
    if needle == b"shell.status=exit-code\n" {
        return "missing halt exit-code shell status audit";
    }
    if needle
        == b"exec.path=/bin/halt pid=4 task=4 status=exit-code loader=linked-bin entry_fn=bin_halt\n"
    {
        return "missing linked halt exit-code audit";
    }
    if needle == b"probes\n" {
        return "missing boot probes file row";
    }
    if needle == b"proof\n" {
        return "missing boot proof file row";
    }
    if needle == b"profile\n" {
        return "missing boot profile file row";
    }
    if needle == b"status\n" {
        return "missing boot status file row";
    }
    if needle == b"  cat [path...] - print stdin or kernel VFS pseudo files\n" {
        return "missing boot help cat entry";
    }
    if needle == b"  help [program] - show program help\n" {
        return "missing detailed help entry";
    }
    if needle == b"  clear - clear framebuffer console and terminal\n" {
        return "missing detailed clear entry";
    }
    if needle == b"  screentest - print renderer diagnostics\n" {
        return "missing detailed screentest entry";
    }
    if needle == b"  pwd - print current kernel VFS directory\n" {
        return "missing detailed pwd entry";
    }
    if needle == b"  ls [path] - list a kernel VFS directory\n" {
        return "missing detailed ls entry";
    }
    if needle == b"  cd [path] - change current kernel VFS directory\n" {
        return "missing detailed cd entry";
    }
    if needle == b"  read - read one TTY line\n" {
        return "missing detailed read entry";
    }
    if needle == b"    required rows: el: clean, el1: clean-left, el2: clean-all\n" {
        return "missing detailed screentest required rows";
    }
    if needle == b"  mount - print kernel VFS mount table\n" {
        return "missing detailed mount entry";
    }
    if needle == b"  input - print live console input diagnostics\n" {
        return "missing detailed input entry";
    }
    if needle == b"  status - print boot, input, and manual_next summary\n" {
        return "missing detailed status entry";
    }
    if needle == b"  proof - print physical input proof checklist\n" {
        return "missing detailed proof entry";
    }
    if needle == b"  device - print boot memory and device inventory\n" {
        return "missing detailed device entry";
    }
    if needle == b"  dmesg [--stats] - print retained kernel log or ring stats\n" {
        return "missing detailed dmesg entry";
    }
    if needle == b"  dump [status|snapshot|sync] - inspect or flush kernel dump state\n" {
        return "missing detailed dump entry";
    }
    if needle == b"  sched [status|tick|yield|sleep TICKS] - inspect scheduler state, tick, yield, or sleep current task\n" {
        return "missing detailed sched entry";
    }
    if needle == b"  proc [processes|execs|address-spaces|page-tables|pages|memory-objects|media|pending|self|session|services|sources|tasks|waits|syscalls|continuations|scheduler] - inspect process state\n" {
        return "missing detailed proc entry";
    }
    if needle
        == b"  probe target - run lower hardware probe; try `probe help` or `cat /boot/probes`\n"
    {
        return "missing detailed probe entry";
    }
    if needle == b"  launch [NAME=VALUE ...] [payload] [arg...] - list or run registered payloads\n"
    {
        return "missing detailed launch entry";
    }
    if needle == b"  reovim [NAME=VALUE ...] [arg...] - run the default reovim payload alias\n" {
        return "missing detailed reovim entry";
    }
    if needle == b"  hello - print a linked-bin syscall proof\n" {
        return "missing detailed hello entry";
    }
    if needle == b"  halt - request root daemon shutdown\n" {
        return "missing detailed halt entry";
    }
    if needle == b"  ps - print retained process table\n" {
        return "missing detailed ps entry";
    }
    if needle == b"  kill PID - terminate a retained ready or blocked process\n" {
        return "missing detailed kill entry";
    }
    if needle == b"  wake PID - wake an operator-blocked process\n" {
        return "missing detailed wake entry";
    }
    if needle == b"  block [NAME=VALUE ...] PROGRAM [ARG...] - spawn a retained blocked process\n" {
        return "missing detailed block entry";
    }
    if needle == b"  spawn [NAME=VALUE ...] PROGRAM [ARG...] - spawn a retained ready process\n" {
        return "missing detailed spawn entry";
    }
    if needle
        == b"  sleep TICKS [NAME=VALUE ...] PROGRAM [ARG...] - spawn a retained process blocked until scheduler ticks\n"
    {
        return "missing detailed sleep entry";
    }
    if needle == b"  wait PID - wait for a retained process\n" {
        return "missing detailed wait entry";
    }
    if needle == b"  wait-ticks TICKS PID - wait with scheduler ticks for a retained process\n" {
        return "missing detailed wait-ticks entry";
    }
    if needle == b"  exec [NAME=VALUE ...] PROGRAM [ARG...] - replace current process image\n" {
        return "missing detailed exec entry";
    }
    if needle == b"  service-stop NAME - stop a retained resident service\n" {
        return "missing detailed service-stop entry";
    }
    if needle == b"  service-start NAME - start a retained payload service\n" {
        return "missing detailed service-start entry";
    }
    if needle == b"  service-restart NAME - restart a retained payload service\n" {
        return "missing detailed service-restart entry";
    }
    if needle == b"  session - print active shell session state\n" {
        return "missing detailed session entry";
    }
    if needle == b"  services - print retained service table\n" {
        return "missing detailed services entry";
    }
    if needle == b"  tasks - print retained task table\n" {
        return "missing detailed tasks entry";
    }
    if needle == b"  waits - print retained wait table\n" {
        return "missing detailed waits entry";
    }
    if needle == b"  syscalls - print retained syscall trace\n" {
        return "missing detailed syscalls entry";
    }
    if needle == b"  continuations - print active syscall continuations\n" {
        return "missing detailed continuations entry";
    }
    if needle == b"  execs - print executable admission table\n" {
        return "missing detailed execs entry";
    }
    if needle == b"  pending - print pending executable table\n" {
        return "missing detailed pending entry";
    }
    if needle == b"  sources - print executable source table\n" {
        return "missing detailed sources entry";
    }
    if needle == b"  media - print executable media status\n" {
        return "missing detailed media entry";
    }
    if needle == b"  self - print current process state\n" {
        return "missing detailed self entry";
    }
    if needle == b"package=reovim-os\n" {
        return "missing package row";
    }
    if needle == b"version=test\n" {
        return "missing test version row";
    }
    if needle == b"target=fixture-target\n" {
        return "missing fixture target row";
    }
    if needle == b"selected_profile=shell-only\n" {
        return "missing selected profile row";
    }
    if needle == b"profile_request=shell-only\n" {
        return "missing profile request row";
    }
    if needle == b"bootline=absent\n" {
        return "missing absent bootline row";
    }
    if needle == b"launch_profile_feature=disabled\n" {
        return "missing disabled launch-profile feature row";
    }
    if needle == b"source=fixture-input\n" {
        return "missing fixture source row";
    }
    if needle == b"source_state=ready\n" {
        return "missing ready source state row";
    }
    if needle == b"mode=fixture\n" {
        return "missing fixture mode row";
    }
    if needle == b"usb_keyboard_pending_bytes=0\n" {
        return "missing zero pending usb bytes row";
    }
    if needle == b"usb_keyboard_probe=disabled\n" {
        return "missing disabled usb probe row";
    }
    if needle == b"usb_keyboard_poll_interval_ms=0\n" {
        return "missing usb poll interval row";
    }
    if needle == b"usb_keyboard_last_poll=not-polled\n" {
        return "missing not-polled usb row";
    }
    if needle == b"manual_next=probe-help\n" {
        return "missing probe-help manual next row";
    }
    if needle.starts_with(b"exec.path=/bin/cat") {
        return "missing linked cat exec audit row";
    }
    if needle.starts_with(b"path=/bin/cat") {
        return "missing linked cat syscall row";
    }
    if needle.starts_with(b"  ") {
        return "missing indented proof or help row";
    }
    if needle == b"reovim-dump-v1\n" {
        return "missing dump format";
    }
    if needle == b"boot_memory_ranges=1\n" {
        return "missing dump boot memory header row";
    }
    if needle == b"program_stdin_read_bytes=3\n" {
        return "missing /bin/input scheduled stdin read count";
    }
    if needle == b"profile=shell-only\n" {
        return "missing shell-only profile row";
    }
    if needle == b"launch=disabled\n" {
        return "missing disabled launch row";
    }
    if needle == b"payloads=3\n" {
        return "missing payload count row";
    }
    if needle == b"input=fixture-input\n" {
        return "missing fixture input row";
    }
    if needle == b"usb_keyboard=unavailable\n" {
        return "missing unavailable usb keyboard row";
    }
    if needle
        == b"exec.path=/bin/input pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_input\n"
    {
        return "missing successful linked-bin /bin/input audit row";
    }
    if needle
        == b"exec.path=/bin/input pid=4 task=4 status=exit-code loader=linked-bin entry_fn=bin_input\n"
    {
        return "missing rejected linked-bin /bin/input audit row";
    }
    if needle == b"device_records=1\n" {
        return "missing dump device count header row";
    }
    if needle == b"boot:\n" {
        return "missing dump boot section";
    }
    if needle == b"devices:\n" {
        return "missing dump devices section";
    }
    if needle == b"proof:\n" {
        return "missing dump proof section";
    }
    if needle == b"panic:\n" {
        return "missing dump panic section";
    }
    if needle == b"processes:\n" {
        return "missing process table";
    }
    if needle == b"tasks:\n" {
        return "missing task table";
    }
    if needle == b"state=running path=/bin/cat" {
        return "missing running cat process";
    }
    if needle == b"state=running entry=/bin/cat" {
        return "missing running cat task";
    }
    if needle == b"external diagnostics:\nboot diagnostics complete\n" {
        return "missing external diagnostics trailer";
    }
    "expected output chunk"
}

fn find_subslice_from(haystack: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i + needle.len() <= haystack.len() {
        let mut match_len = 0usize;
        while match_len < needle.len() && haystack[i + match_len] == needle[match_len] {
            match_len += 1;
        }
        if match_len == needle.len() {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn assert_proof_commands_match_budget_transcript(output: &[u8]) {
    let Some(mut pos) = find_subslice_from(output, b"/bin programs:\n", 0) else {
        testrt::check(false, "proof output has /bin programs section");
        return;
    };
    pos += b"/bin programs:\n".len();

    let Some(end) = find_subslice_from(output, b"terminal:\n", pos) else {
        testrt::check(false, "proof output has terminal section");
        return;
    };

    let mut index = 0usize;
    while index < PHYSICAL_PROOF_COMMANDS.len() {
        if pos + 2 > end {
            testrt::check(false, "proof output has enough command rows");
            return;
        }
        testrt::check_eq(&output[pos..pos + 2], b"  ");
        pos += 2;

        let command = PHYSICAL_PROOF_COMMANDS[index];
        if pos + command.len() > end {
            testrt::check(false, "proof command row is complete");
            return;
        }
        testrt::check_eq(&output[pos..pos + command.len()], command);
        pos += command.len();
        index += 1;
    }

    testrt::check_eq(pos, end);
}

arch_test!(root_shell_help, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help\n", None);
    testrt::check_eq(
        sink_str(),
        "reovim root shell\n/bin programs: help, init, sh, clear, screentest, pwd, ls, cd, cat, read, mount, input, status, proof, device, dmesg, dump, sched, proc, probe, launch, reovim, hello, halt, ps, kill, wake, block, spawn, sleep, wait, wait-ticks, exec, service-stop, service-start, service-restart, session, services, tasks, waits, syscalls, continuations, execs, pending, sources, media, self, install-bin, install-payload, install-bin-media, install-payload-media\nnamespace: /bin\nusage: help [program]\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help init\n", None);
    testrt::check_eq(sink_str(), "init - start userland session services\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help sh\n", None);
    testrt::check_eq(sink_str(), "sh - run interactive root shell\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help input\n", None);
    testrt::check_eq(sink_str(), "input - print live console input diagnostics\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help read\n", None);
    testrt::check_eq(sink_str(), "read - read one TTY line\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help /bin/input\n", None);
    testrt::check_eq(sink_str(), "input - print live console input diagnostics\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help clear\n", None);
    testrt::check_eq(sink_str(), "clear - clear framebuffer console and terminal\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help screentest\n", None);
    testrt::check_eq(
        sink_str(),
        "screentest - print renderer diagnostics\n  required rows: el: clean, el1: clean-left, el2: clean-all\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help status\n", None);
    testrt::check_eq(sink_str(), "status - print boot, input, and manual_next summary\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help proof\n", None);
    testrt::check_eq(sink_str(), "proof - print physical input proof checklist\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help probe\n", None);
    testrt::check_eq(
        sink_str(),
        "probe target - run lower hardware probe; try `probe help` or `cat /boot/probes`\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help dmesg\n", None);
    testrt::check_eq(sink_str(), "dmesg [--stats] - print retained kernel log or ring stats\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help dump\n", None);
    testrt::check_eq(
        sink_str(),
        "dump [status|snapshot|sync] - inspect or flush kernel dump state\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help sched\n", None);
    testrt::check_eq(
        sink_str(),
        "sched [status|tick|yield|sleep TICKS] - inspect scheduler state, tick, yield, or sleep current task\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help proc\n", None);
    testrt::check_eq(
        sink_str(),
        "proc [processes|execs|address-spaces|page-tables|pages|memory-objects|media|pending|self|session|services|sources|tasks|waits|syscalls|continuations|scheduler] - inspect process state\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help device\n", None);
    testrt::check_eq(sink_str(), "device - print boot memory and device inventory\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help launch\n", None);
    testrt::check_eq(
        sink_str(),
        "launch [NAME=VALUE ...] [payload] [arg...] - list or run registered payloads\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help reovim\n", None);
    testrt::check_eq(
        sink_str(),
        "reovim [NAME=VALUE ...] [arg...] - run the default reovim payload alias\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help hello\n", None);
    testrt::check_eq(sink_str(), "hello - print a linked-bin syscall proof\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help halt\n", None);
    testrt::check_eq(sink_str(), "halt - request root daemon shutdown\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help ps\n", None);
    testrt::check_eq(sink_str(), "ps - print retained process table\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help kill\n", None);
    testrt::check_eq(sink_str(), "kill PID - terminate a retained ready or blocked process\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help wake\n", None);
    testrt::check_eq(sink_str(), "wake PID - wake an operator-blocked process\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help block\n", None);
    testrt::check_eq(
        sink_str(),
        "block [NAME=VALUE ...] PROGRAM [ARG...] - spawn a retained blocked process\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help spawn\n", None);
    testrt::check_eq(
        sink_str(),
        "spawn [NAME=VALUE ...] PROGRAM [ARG...] - spawn a retained ready process\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help sleep\n", None);
    testrt::check_eq(
        sink_str(),
        "sleep TICKS [NAME=VALUE ...] PROGRAM [ARG...] - spawn a retained process blocked until scheduler ticks\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help wait\n", None);
    testrt::check_eq(sink_str(), "wait PID - wait for a retained process\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help wait-ticks\n", None);
    testrt::check_eq(
        sink_str(),
        "wait-ticks TICKS PID - wait with scheduler ticks for a retained process\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help exec\n", None);
    testrt::check_eq(
        sink_str(),
        "exec [NAME=VALUE ...] PROGRAM [ARG...] - replace current process image\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help service-stop\n", None);
    testrt::check_eq(sink_str(), "service-stop NAME - stop a retained resident service\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help service-start\n", None);
    testrt::check_eq(sink_str(), "service-start NAME - start a retained payload service\n");

    run_shell_line_fixture(
        ProfileSummary::new("shell-only", false),
        b"help service-restart\n",
        None,
    );
    testrt::check_eq(sink_str(), "service-restart NAME - restart a retained payload service\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help session\n", None);
    testrt::check_eq(sink_str(), "session - print active shell session state\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help services\n", None);
    testrt::check_eq(sink_str(), "services - print retained service table\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help tasks\n", None);
    testrt::check_eq(sink_str(), "tasks - print retained task table\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help waits\n", None);
    testrt::check_eq(sink_str(), "waits - print retained wait table\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help syscalls\n", None);
    testrt::check_eq(sink_str(), "syscalls - print retained syscall trace\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help continuations\n", None);
    testrt::check_eq(sink_str(), "continuations - print active syscall continuations\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help execs\n", None);
    testrt::check_eq(sink_str(), "execs - print executable admission table\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help pending\n", None);
    testrt::check_eq(sink_str(), "pending - print pending executable table\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help sources\n", None);
    testrt::check_eq(sink_str(), "sources - print executable source table\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help media\n", None);
    testrt::check_eq(sink_str(), "media - print executable media status\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help self\n", None);
    testrt::check_eq(sink_str(), "self - print current process state\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help missing\n", None);
    testrt::check_eq(sink_str(), "help: unknown program: missing\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help a b\n", None);
    testrt::check_eq(sink_str(), "help: too many arguments\n");
});

arch_test!(scheduled_exec_passes_pending_stdin_to_program, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let ctx = crate::syscall::exec_bin_from_shell_argv_with_stdin(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        argv1("input"),
        b"abc",
    )
    .expect("input exec loads with stdin");
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(ctx));
    let pending = crate::syscall::take_pending_exec(ctx.pid).expect("pending input exec exists");
    let pending_ctx = crate::syscall::SyscallContext::from_process(pending.handle());
    let status = execute_loaded_program_argv(
        &daemon,
        &mut session,
        pending.program(),
        pending.argv(),
        pending.env(),
        pending.stdin(),
        None,
        Some(pending_ctx),
    )
    .status();
    crate::syscall::exit_current(ctx, status);

    testrt::check_eq(status, crate::program::ProgramStatus::Ok);
    assert_contains(sink_bytes(), b"program_stdin_read_bytes=3\n");
    assert_syscall_record(
        "/bin/input",
        crate::syscall::SyscallOp::ConsoleInput,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/input",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );

    run_session_shell_line(&mut session, b"pwd | cat\n");
    testrt::check_eq(sink_bytes(), b"/\n");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(root_shell_input_runs_as_linked_bin, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"input\n", None);
    assert_contains(sink_bytes(), b"source=fixture-input\n");
    assert_contains(sink_bytes(), b"program_stdin_read_bytes=0\n");
    assert_syscall_record(
        "/bin/input",
        crate::syscall::SyscallOp::ConsoleInput,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/input",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/input pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_input\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"input extra\n", None);
    testrt::check_eq(sink_str(), "input: too many arguments\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/input pid=3 task=3 status=exit-code loader=linked-bin entry_fn=bin_input\n",
    );
});

arch_test!(root_shell_read_runs_as_linked_bin_over_dev_tty, {
    TTY_READ_CALLS.store(0, Ordering::Relaxed);
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"read\n", None);
    testrt::check_eq(sink_str(), "tty fixture input\n");
    testrt::check_eq(TTY_READ_CALLS.load(Ordering::Relaxed), 1usize);
    assert_syscall_record(
        "/bin/read",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/read",
        crate::syscall::SyscallOp::TtyReadLine,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/read",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/read",
        crate::syscall::SyscallOp::FdClose,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/read pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_read\n",
    );

    TTY_READ_CALLS.store(0, Ordering::Relaxed);
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"cat /dev/tty\n", None);
    testrt::check_eq(sink_str(), "tty fixture input");
    testrt::check_eq(TTY_READ_CALLS.load(Ordering::Relaxed), 2usize);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::TtyReadLine,
        crate::syscall::SyscallStatus::Ok,
    );

    TTY_READ_CALLS.store(0, Ordering::Relaxed);
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"read extra\n", None);
    testrt::check_eq(sink_str(), "read: too many arguments\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/read pid=3 task=3 status=exit-code loader=linked-bin entry_fn=bin_read\n",
    );

    TTY_READ_CALLS.store(1, Ordering::Relaxed);
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"read\n", None);
    testrt::check_eq(sink_str(), "read: input eof\n");
    assert_syscall_record(
        "/bin/read",
        crate::syscall::SyscallOp::TtyReadLine,
        crate::syscall::SyscallStatus::Unavailable,
    );
});

arch_test!(root_shell_pipe_routes_stdout_to_next_program_stdin, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    let mut session = RootShellSession::new();

    run_session_shell_line(&mut session, b"pwd | input\n");

    let output = sink_bytes();
    testrt::check(output.len() >= b"source=".len(), "pipeline emitted consumer output");
    testrt::check_eq(&output[..b"source=".len()], b"source=");
    assert_contains(output, b"program_stdin_read_bytes=2\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::FdDuplicate,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/input",
        crate::syscall::SyscallOp::FdDuplicate,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/input",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(root_shell_clear_and_screentest, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"clear\n", None);
    testrt::check_eq(sink_str(), "\x1b[2J\x1b[H");
    assert_syscall_record(
        "/bin/clear",
        crate::syscall::SyscallOp::TtyClear,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/clear",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/clear",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/clear",
        crate::syscall::SyscallOp::FdClose,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/clear pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_clear\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"clear now\n", None);
    testrt::check_eq(sink_str(), "clear: too many arguments\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"screentest\n", None);
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
    assert_syscall_record(
        "/bin/screentest",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/screentest",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/screentest",
        crate::syscall::SyscallOp::FdClose,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/screentest pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_screentest\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"screentest extra\n", None);
    testrt::check_eq(sink_str(), "screentest: too many arguments\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/screentest pid=3 task=3 status=exit-code loader=linked-bin entry_fn=bin_screentest\n",
    );
});

arch_test!(root_shell_launch_mount_and_reovim, {
    run_shell_line_fixture(ProfileSummary::new("appliance", true), b"launch\n", None);
    assert_contains(
        sink_bytes(),
        b"launch: available payloads:\n  reovim: default reovim payload loader=source-image entry_fn=payload_reovim\n  editor-smoke: editor smoke payload loader=source-image entry_fn=payload_editor_smoke\n  server-smoke: server smoke payload loader=source-image entry_fn=payload_server_smoke\n",
    );

    run_shell_line_fixture(ProfileSummary::new("appliance", true), b"launch editor-smoke\n", None);
    testrt::check_eq(sink_str(), "launch editor-smoke: payload.ready\n");

    run_shell_line_fixture(ProfileSummary::new("appliance", true), b"launch \"\"\n", None);
    testrt::check_eq(sink_str(), "launch: no payload name\n");

    run_shell_line_fixture(
        ProfileSummary::new("shell-only", false),
        b"launch editor-smoke\n",
        None,
    );
    testrt::check_eq(sink_str(), "launch disabled for this profile\n");

    run_shell_line_fixture(ProfileSummary::new("appliance", true), b"mount\n", None);
    assert_contains(sink_bytes(), b"kernel on / type rootfs (ro,pseudo)\n");
    assert_contains(sink_bytes(), b"boot on /boot type bootfs (ro,pseudo)\n");
    assert_contains(sink_bytes(), b"devices on /dev type devfs (ro,pseudo)\n");
    assert_contains(sink_bytes(), b"klog on /log type logfs (ro,pseudo)\n");
    assert_syscall_record(
        "/bin/mount",
        crate::syscall::SyscallOp::VfsMounts,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/mount pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_mount\n",
    );

    run_shell_line_fixture(ProfileSummary::new("appliance", true), b"mount extra\n", None);
    testrt::check_eq(sink_str(), "mount: too many arguments\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/mount pid=3 task=3 status=exit-code loader=linked-bin entry_fn=bin_mount\n",
    );

    run_shell_line_fixture(ProfileSummary::new("appliance", true), b"reovim\n", None);
    testrt::check_eq(sink_str(), "reovim: payload.ready\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"reovim\n", None);
    testrt::check_eq(sink_str(), "reovim disabled for this profile\n");

    run_shell_line_fixture(ProfileSummary::new("appliance", true), b"reovim extra\n", None);
    testrt::check_eq(sink_str(), "reovim: payload.ready\n");
});

arch_test!(root_shell_launch_retains_payload_argv, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_STDOUT_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke --mode smoke\n");
    assert_contains(sink_bytes(), b"payload.stdout\nself:\n");
    assert_contains(
        sink_bytes(),
        b"argc=3 argv0=editor-smoke argv0_truncated=false argv1=--mode argv1_truncated=false argv2=smoke argv2_truncated=false",
    );
    assert_contains(sink_bytes(), b"launch editor-smoke: payload.ready\n");

    let payload = crate::proc::process(4).expect("payload process is retained");
    testrt::check_eq(payload.program_path, "/payload/editor-smoke");
    testrt::check_eq(payload.argc, 3usize);
    testrt::check_eq(payload.argv0(), "editor-smoke");
    testrt::check_eq(payload.argv1(), "--mode");
    testrt::check_eq(payload.argv(2), "smoke");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Reaped);

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ProcessSelf,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
});

arch_test!(root_shell_payload_launch_uses_installed_source_overlay, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_FAILED_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "launch editor-smoke: payload.failed\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"cat /proc/sources\n");
    assert_contains(
        sink_bytes(),
        b"namespace=payload path=/payload/editor-smoke loader=source-image bytes=",
    );
    assert_contains(sink_bytes(), b"origin=installed\n");
    reset_installed_sources();
});

arch_test!(payload_source_exec_bin_runs_child_under_payload_process, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_EXEC_PWD_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "/\nlaunch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut payload_pid = 0usize;
    let mut pwd_parent_pid = 0usize;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == "/payload/editor-smoke" {
            payload_pid = record.pid;
        }
        if record.program_path == "/bin/pwd" {
            pwd_parent_pid = record.parent_pid;
        }
        index += 1;
    }

    testrt::check(payload_pid != 0, "payload process is retained");
    testrt::check_eq(pwd_parent_pid, payload_pid);

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"cat /log/dmesg\n");
    assert_contains(
        sink_bytes(),
        b"payload.start path=/payload/editor-smoke loader=source-image entry_fn=payload_editor_smoke",
    );
    assert_contains(sink_bytes(), b"exec.parent path=/bin/pwd");
    reset_installed_sources();
});

arch_test!(payload_source_exec_bin_stdin_hex_feeds_child_stdin, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_EXEC_CAT_STDIN_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "payload.child.stdin\nlaunch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut payload_pid = 0usize;
    let mut cat = crate::proc::EMPTY_PROCESS_RECORD;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == "/payload/editor-smoke" {
            payload_pid = record.pid;
        }
        if record.program_path == "/bin/cat" {
            cat = record;
        }
        index += 1;
    }

    testrt::check(payload_pid != 0, "payload process is retained");
    testrt::check_eq(cat.program_path, "/bin/cat");
    testrt::check_eq(cat.parent_pid, payload_pid);
    testrt::check_eq(cat.argc, 1usize);
    testrt::check_eq(cat.argv0(), "cat");
    testrt::check_eq(cat.argv_was_truncated(0), false);

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"cat /log/dmesg\n");
    assert_contains(sink_bytes(), b"exec.parent path=/bin/cat");
    reset_installed_sources();
});

arch_test!(payload_source_pipe_bin_feeds_child_stdout_to_child_stdin, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_PIPE_PWD_CAT_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "/\nlaunch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::FdPipe,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::FdDuplicate,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdDuplicate,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut payload_pid = 0usize;
    let mut producer = crate::proc::EMPTY_PROCESS_RECORD;
    let mut consumer = crate::proc::EMPTY_PROCESS_RECORD;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == "/payload/editor-smoke" {
            payload_pid = record.pid;
        }
        if record.program_path == "/bin/pwd" {
            producer = record;
        }
        if record.program_path == "/bin/cat" {
            consumer = record;
        }
        index += 1;
    }

    testrt::check(payload_pid != 0, "payload process is retained");
    testrt::check_eq(producer.program_path, "/bin/pwd");
    testrt::check_eq(producer.parent_pid, payload_pid);
    testrt::check_eq(producer.argc, 1usize);
    testrt::check_eq(producer.argv0(), "pwd");
    testrt::check_eq(consumer.program_path, "/bin/cat");
    testrt::check_eq(consumer.parent_pid, payload_pid);
    testrt::check_eq(consumer.argc, 1usize);
    testrt::check_eq(consumer.argv0(), "cat");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"cat /log/dmesg\n");
    assert_contains(sink_bytes(), b"exec.parent path=/bin/pwd");
    assert_contains(sink_bytes(), b"exec.parent path=/bin/cat");
    reset_installed_sources();
});

arch_test!(payload_source_pipe_bin_resumes_blocked_producer, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    install_exec_bundle(EXEC_BUNDLE_BIN_UAPI_SLEEP_DYNAMIC);
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_PIPE_MEDIA_SLEEP_CAT_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "");

    let launcher = crate::proc::process(3).expect("launch process remains retained");
    testrt::check_eq(launcher.program_path, "/bin/launch");
    testrt::check_eq(launcher.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(launcher.block_reason, crate::sched::BlockReason::WaitChild);
    let payload = crate::proc::process(4).expect("payload process remains retained");
    testrt::check_eq(payload.program_path, "/payload/editor-smoke");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(payload.block_reason, crate::sched::BlockReason::WaitChild);
    let producer = crate::proc::process(5).expect("payload pipe producer remains retained");
    testrt::check_eq(producer.program_path, "/bin/media-sleep");
    testrt::check_eq(producer.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(producer.block_reason, crate::sched::BlockReason::Sleep);
    testrt::check(
        crate::proc::process(6).is_none(),
        "payload pipe consumer is admitted only after producer completion",
    );

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 2usize);
    let mut saw_launch_wait = false;
    let mut saw_producer_sleep = false;
    let mut index = 0usize;
    while index < continuation_count {
        let continuation = continuations[index];
        if continuation.process_id == launcher.pid {
            testrt::check_eq(continuation.nr, reovim_uapi_syscall::SyscallNr::PROCESS_WAIT_READY);
            saw_launch_wait = true;
        }
        if continuation.process_id == producer.pid {
            testrt::check_eq(continuation.nr, reovim_uapi_syscall::SyscallNr::SLEEP);
            saw_producer_sleep = true;
        }
        index += 1;
    }
    testrt::check(saw_launch_wait, "launch retained wait-ready continuation");
    testrt::check(saw_producer_sleep, "producer retained sleep continuation");
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Blocked,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );

    let _ = daemon.run_shell_line(&mut session, b"sched tick\n");
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let idle_status = daemon.run_idle_ready_programs(&mut session);
    testrt::check_eq(idle_status, crate::program::ProgramStatus::Ok);
    testrt::check_eq(
        sink_str(),
        "exec-body-sleep.before\nexec-body-sleep.after\npayload-pipe.after\n",
    );

    let payload = crate::proc::process(4).expect("payload process remains retained");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(payload.exit_code, 0);
    let producer = crate::proc::process(5).expect("producer process remains retained");
    testrt::check_eq(producer.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(producer.exit_code, 0);
    let tick = crate::proc::process(6).expect("sched tick process remains retained");
    testrt::check_eq(tick.program_path, "/bin/sched");
    let consumer = crate::proc::process(7).expect("payload pipe consumer remains retained");
    testrt::check_eq(consumer.program_path, "/bin/cat");
    testrt::check_eq(consumer.parent_pid, payload.pid);
    testrt::check_eq(consumer.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(consumer.exit_code, 0);
    let launcher = crate::proc::process(3).expect("launcher remains retained");
    testrt::check_eq(launcher.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(launcher.block_reason, crate::sched::BlockReason::UserResume);

    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    crate::exec::reset();
});

arch_test!(payload_source_spawn_bin_leaves_adopted_child_ready, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SPAWN_PWD_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "launch editor-smoke: payload.ready\n");

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut payload_pid = 0usize;
    let mut spawned_pwd = crate::proc::EMPTY_PROCESS_RECORD;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == "/payload/editor-smoke" {
            payload_pid = record.pid;
        }
        if record.program_path == "/bin/pwd" {
            spawned_pwd = record;
        }
        index += 1;
    }

    testrt::check(payload_pid != 0, "payload process is retained");
    testrt::check_eq(spawned_pwd.program_path, "/bin/pwd");
    testrt::check_eq(spawned_pwd.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(spawned_pwd.parent_pid, crate::proc::ROOTD_PID);
    testrt::check_eq(spawned_pwd.argc, 1usize);
    testrt::check_eq(spawned_pwd.argv0(), "pwd");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessAdopt,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"process.adopt old_parent_pid=");
    assert_contains(sink_bytes(), b"new_parent_pid=1");
    assert_contains(sink_bytes(), b"path=/bin/pwd");
    reset_installed_sources();
});

arch_test!(payload_source_spawn_payload_leaves_adopted_child_ready, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SPAWN_SERVER_SMOKE_SOURCE_BYTES,
        ),
        Ok(()),
    );
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/server-smoke",
            SAMPLE_PAYLOAD_READY_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "launch editor-smoke: payload.ready\n");

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut editor = crate::proc::EMPTY_PROCESS_RECORD;
    let mut server = crate::proc::EMPTY_PROCESS_RECORD;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == "/payload/editor-smoke" {
            editor = record;
        }
        if record.program_path == "/payload/server-smoke" {
            server = record;
        }
        index += 1;
    }

    testrt::check_eq(editor.program_path, "/payload/editor-smoke");
    testrt::check_eq(editor.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(server.program_path, "/payload/server-smoke");
    testrt::check_eq(server.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(server.parent_pid, crate::proc::ROOTD_PID);
    testrt::check_eq(server.argc, 1usize);
    testrt::check_eq(server.argv0(), "server-smoke");

    let mut pending =
        [crate::exec::EMPTY_PENDING_EXEC_RECORD; crate::exec::MAX_PENDING_EXEC_RECORDS];
    let pending_count = crate::exec::snapshot_pending(&mut pending);
    let mut found_pending_server = false;
    index = 0;
    while index < pending_count {
        if pending[index].path == "/payload/server-smoke" {
            found_pending_server = true;
            testrt::check_eq(pending[index].parent_pid, crate::proc::ROOTD_PID);
            testrt::check_eq(pending[index].kind, crate::exec::ExecLoadKind::Payload);
        }
        index += 1;
    }
    testrt::check(found_pending_server, "spawned payload remains pending after adoption");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadLaunch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::ProcessAdopt,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"process.adopt old_parent_pid=");
    assert_contains(sink_bytes(), b"new_parent_pid=1");
    assert_contains(sink_bytes(), b"path=/payload/server-smoke");
    reset_installed_sources();
});

arch_test!(rootd_idle_dispatch_runs_adopted_pending_payload_child, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SPAWN_SERVER_SMOKE_SOURCE_BYTES,
        ),
        Ok(()),
    );
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/server-smoke",
            SAMPLE_PAYLOAD_READY_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "launch editor-smoke: payload.ready\n");

    let mut pending =
        [crate::exec::EMPTY_PENDING_EXEC_RECORD; crate::exec::MAX_PENDING_EXEC_RECORDS];
    let pending_count = crate::exec::snapshot_pending(&mut pending);
    let mut found_pending_server = false;
    let mut index = 0usize;
    while index < pending_count {
        if pending[index].path == "/payload/server-smoke" {
            found_pending_server = true;
            testrt::check_eq(pending[index].parent_pid, crate::proc::ROOTD_PID);
            testrt::check_eq(pending[index].kind, crate::exec::ExecLoadKind::Payload);
        }
        index += 1;
    }
    testrt::check(found_pending_server, "spawned payload is pending before idle dispatch");

    sink_clear();
    testrt::check_eq(daemon.run_idle_ready_programs(&mut session), ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "");

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut server = crate::proc::EMPTY_PROCESS_RECORD;
    index = 0;
    while index < count {
        let record = records[index];
        if record.program_path == "/payload/server-smoke" {
            server = record;
        }
        index += 1;
    }
    testrt::check_eq(server.program_path, "/payload/server-smoke");
    testrt::check_eq(server.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(server.parent_pid, crate::proc::ROOTD_PID);
    testrt::check_eq(server.exit_code, 0i32);

    testrt::check_eq(crate::exec::snapshot_pending(&mut pending), 0usize);
    testrt::check_eq(crate::dump::status().pending_exec_records, 0usize);
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"payload.start path=/payload/server-smoke loader=source-image entry_fn=payload_server_smoke",
    );
    assert_contains(sink_bytes(), b"payload.exit path=/payload/server-smoke");

    sink_clear();
    let mut inspector = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
    inspector
        .write_vfs_file_path("/dump/status")
        .expect("dump status renders after payload idle dispatch");
    assert_contains(sink_bytes(), b"pending_exec_records=0\n");
    reset_installed_sources();
});

arch_test!(payload_source_spawn_wait_payload_waits_for_child_pid, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SPAWN_WAIT_SERVER_SMOKE_SOURCE_BYTES,
        ),
        Ok(()),
    );
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/server-smoke",
            SAMPLE_PAYLOAD_READY_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "launch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut launch = crate::proc::EMPTY_PROCESS_RECORD;
    let mut editor = crate::proc::EMPTY_PROCESS_RECORD;
    let mut server = crate::proc::EMPTY_PROCESS_RECORD;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == "/bin/launch" {
            launch = record;
        }
        if record.program_path == "/payload/editor-smoke" {
            editor = record;
        }
        if record.program_path == "/payload/server-smoke" {
            server = record;
        }
        index += 1;
    }

    testrt::check_eq(launch.program_path, "/bin/launch");
    testrt::check_eq(launch.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(editor.program_path, "/payload/editor-smoke");
    testrt::check_eq(editor.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(editor.parent_pid, launch.pid);
    testrt::check_eq(server.program_path, "/payload/server-smoke");
    testrt::check_eq(server.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(server.parent_pid, editor.pid);
    testrt::check_eq(server.argc, 1usize);
    testrt::check_eq(server.argv0(), "server-smoke");

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    let mut payload_wait_found = false;
    let mut launch_wait_found = false;
    index = 0;
    while index < wait_count {
        let wait = waits[index];
        if wait.parent_pid == editor.pid && wait.child_pid == server.pid {
            payload_wait_found = true;
            testrt::check_eq(wait.child_state, crate::proc::ProcessState::Exited);
            testrt::check_eq(wait.exit_code, 0);
            testrt::check_eq(wait.completed, true);
        }
        if wait.parent_pid == launch.pid && wait.child_pid == editor.pid {
            launch_wait_found = true;
            testrt::check_eq(wait.child_state, crate::proc::ProcessState::Exited);
            testrt::check_eq(wait.exit_code, 0);
            testrt::check_eq(wait.completed, true);
        }
        index += 1;
    }
    testrt::check(payload_wait_found, "payload wait by payload child PID is retained");
    testrt::check(launch_wait_found, "launch wait on payload is retained");

    let scheduler = crate::sched::snapshot_scheduler();
    testrt::check_eq(scheduler.ready_len, 0usize);
    let mut pending =
        [crate::exec::EMPTY_PENDING_EXEC_RECORD; crate::exec::MAX_PENDING_EXEC_RECORDS];
    testrt::check_eq(crate::exec::snapshot_pending(&mut pending), 0usize);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"wait.end parent_pid=4 child_pid=5 child_state=exited exit=0\n");

    reset_installed_sources();
});

arch_test!(payload_source_spawn_wait_payload_resumes_blocked_child, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    install_exec_bundle(EXEC_BUNDLE_BIN_UAPI_SLEEP_DYNAMIC);
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SPAWN_WAIT_BLOCKED_PAYLOAD_SOURCE_BYTES,
        ),
        Ok(()),
    );
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/server-smoke",
            SAMPLE_PAYLOAD_BLOCKING_BIN_WAIT_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "exec-body-sleep.before\n");

    let launcher = crate::proc::process(3).expect("launch process remains retained");
    testrt::check_eq(launcher.program_path, "/bin/launch");
    testrt::check_eq(launcher.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(launcher.block_reason, crate::sched::BlockReason::WaitChild);
    let parent_payload = crate::proc::process(4).expect("parent payload remains retained");
    testrt::check_eq(parent_payload.program_path, "/payload/editor-smoke");
    testrt::check_eq(parent_payload.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(parent_payload.block_reason, crate::sched::BlockReason::WaitChild);
    let child_payload = crate::proc::process(5).expect("child payload remains retained");
    testrt::check_eq(child_payload.program_path, "/payload/server-smoke");
    testrt::check_eq(child_payload.parent_pid, parent_payload.pid);
    testrt::check_eq(child_payload.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(child_payload.block_reason, crate::sched::BlockReason::WaitChild);
    let bin_child = crate::proc::process(6).expect("nested bin child remains retained");
    testrt::check_eq(bin_child.program_path, "/bin/media-sleep");
    testrt::check_eq(bin_child.parent_pid, child_payload.pid);
    testrt::check_eq(bin_child.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(bin_child.block_reason, crate::sched::BlockReason::Sleep);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 2usize);
    let mut saw_launch_wait = false;
    let mut saw_bin_sleep = false;
    let mut index = 0usize;
    while index < continuation_count {
        let continuation = continuations[index];
        if continuation.process_id == launcher.pid {
            saw_launch_wait = true;
            testrt::check_eq(continuation.nr, reovim_uapi_syscall::SyscallNr::PROCESS_WAIT_READY);
        }
        if continuation.process_id == bin_child.pid {
            saw_bin_sleep = true;
            testrt::check_eq(continuation.nr, reovim_uapi_syscall::SyscallNr::SLEEP);
        }
        index += 1;
    }
    testrt::check(saw_launch_wait, "launch retained wait-ready continuation");
    testrt::check(saw_bin_sleep, "nested bin child retained sleep continuation");
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Blocked,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Blocked,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let _ = daemon.run_shell_line(&mut session, b"sched tick\n");
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let idle_status = daemon.run_idle_ready_programs(&mut session);
    testrt::check_eq(idle_status, crate::program::ProgramStatus::Ok);
    testrt::check_eq(
        sink_str(),
        "exec-body-sleep.after\nchild-payload.after\npayload-spawn-wait-payload.after\n",
    );

    let parent_payload = crate::proc::process(4).expect("parent payload remains retained");
    testrt::check_eq(parent_payload.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(parent_payload.exit_code, 0);
    let child_payload = crate::proc::process(5).expect("child payload remains retained");
    testrt::check_eq(child_payload.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(child_payload.exit_code, 0);
    let bin_child = crate::proc::process(6).expect("nested bin child remains retained");
    testrt::check_eq(bin_child.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(bin_child.exit_code, 0);
    let launcher = crate::proc::process(3).expect("launcher remains retained");
    testrt::check_eq(launcher.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(launcher.block_reason, crate::sched::BlockReason::UserResume);

    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    crate::exec::reset();
});

arch_test!(payload_source_spawn_kill_payload_terminates_child_without_dispatch, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SPAWN_KILL_SERVER_SMOKE_SOURCE_BYTES,
        ),
        Ok(()),
    );
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/server-smoke",
            SAMPLE_PAYLOAD_READY_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "launch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::ProcessKill,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut editor = crate::proc::EMPTY_PROCESS_RECORD;
    let mut server = crate::proc::EMPTY_PROCESS_RECORD;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == "/payload/editor-smoke" {
            editor = record;
        }
        if record.program_path == "/payload/server-smoke" {
            server = record;
        }
        index += 1;
    }

    testrt::check_eq(editor.program_path, "/payload/editor-smoke");
    testrt::check_eq(editor.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(server.program_path, "/payload/server-smoke");
    testrt::check_eq(server.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(server.parent_pid, editor.pid);
    testrt::check_eq(server.exit_code, 1);
    testrt::check_eq(server.argc, 1usize);
    testrt::check_eq(server.argv0(), "server-smoke");

    let scheduler = crate::sched::snapshot_scheduler();
    testrt::check_eq(scheduler.ready_len, 0usize);
    let mut pending =
        [crate::exec::EMPTY_PENDING_EXEC_RECORD; crate::exec::MAX_PENDING_EXEC_RECORDS];
    testrt::check_eq(crate::exec::snapshot_pending(&mut pending), 0usize);

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    let mut payload_wait_found = false;
    index = 0;
    while index < wait_count {
        let wait = waits[index];
        if wait.parent_pid == editor.pid && wait.child_pid == server.pid {
            payload_wait_found = true;
        }
        index += 1;
    }
    testrt::check(!payload_wait_found, "payload kill does not record a child wait");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/payload/server-smoke pid=5 task=5 status=error loader=source-image entry_fn=payload_server_smoke\n",
    );
    assert_contains(
        sink_bytes(),
        b"component=proc severity=info kind=process-kill boot=1 session=1 source=process pid=5 task=5\n",
    );
    reset_installed_sources();
});

arch_test!(payload_source_spawn_wait_bin_waits_for_child_pid, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SPAWN_WAIT_PWD_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "/\nlaunch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut launch = crate::proc::EMPTY_PROCESS_RECORD;
    let mut payload = crate::proc::EMPTY_PROCESS_RECORD;
    let mut pwd = crate::proc::EMPTY_PROCESS_RECORD;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == "/bin/launch" {
            launch = record;
        }
        if record.program_path == "/payload/editor-smoke" {
            payload = record;
        }
        if record.program_path == "/bin/pwd" {
            pwd = record;
        }
        index += 1;
    }

    testrt::check_eq(launch.program_path, "/bin/launch");
    testrt::check_eq(launch.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(payload.program_path, "/payload/editor-smoke");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(payload.parent_pid, launch.pid);
    testrt::check_eq(pwd.program_path, "/bin/pwd");
    testrt::check_eq(pwd.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(pwd.parent_pid, payload.pid);

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    let mut payload_wait_found = false;
    let mut launch_wait_found = false;
    index = 0;
    while index < wait_count {
        let wait = waits[index];
        if wait.parent_pid == payload.pid && wait.child_pid == pwd.pid {
            payload_wait_found = true;
            testrt::check_eq(wait.child_state, crate::proc::ProcessState::Exited);
            testrt::check_eq(wait.exit_code, 0);
            testrt::check_eq(wait.completed, true);
        }
        if wait.parent_pid == launch.pid && wait.child_pid == payload.pid {
            launch_wait_found = true;
            testrt::check_eq(wait.child_state, crate::proc::ProcessState::Exited);
            testrt::check_eq(wait.exit_code, 0);
            testrt::check_eq(wait.completed, true);
        }
        index += 1;
    }
    testrt::check(payload_wait_found, "payload wait by child PID is retained");
    testrt::check(launch_wait_found, "launch wait on payload is retained");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"wait.end parent_pid=4 child_pid=5 child_state=exited exit=0\n");

    reset_installed_sources();
});

arch_test!(payload_source_spawn_kill_bin_terminates_child_without_dispatch, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SPAWN_KILL_PWD_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "launch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessKill,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut payload = crate::proc::EMPTY_PROCESS_RECORD;
    let mut pwd = crate::proc::EMPTY_PROCESS_RECORD;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == "/payload/editor-smoke" {
            payload = record;
        }
        if record.program_path == "/bin/pwd" {
            pwd = record;
        }
        index += 1;
    }

    testrt::check_eq(payload.program_path, "/payload/editor-smoke");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(pwd.program_path, "/bin/pwd");
    testrt::check_eq(pwd.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(pwd.parent_pid, payload.pid);
    testrt::check_eq(pwd.argc, 1usize);
    testrt::check_eq(pwd.argv0(), "pwd");

    let scheduler = crate::sched::snapshot_scheduler();
    testrt::check_eq(scheduler.ready_len, 0usize);
    let mut pending =
        [crate::exec::EMPTY_PENDING_EXEC_RECORD; crate::exec::MAX_PENDING_EXEC_RECORDS];
    testrt::check_eq(crate::exec::snapshot_pending(&mut pending), 0usize);

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    let mut payload_wait_found = false;
    index = 0;
    while index < wait_count {
        let wait = waits[index];
        if wait.parent_pid == payload.pid && wait.child_pid == pwd.pid {
            payload_wait_found = true;
        }
        index += 1;
    }
    testrt::check(!payload_wait_found, "payload kill does not record a child wait");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=5 task=5 status=error loader=linked-bin entry_fn=bin_pwd\n",
    );
    assert_contains(
        sink_bytes(),
        b"component=proc severity=info kind=process-kill boot=1 session=1 source=process pid=5 task=5\n",
    );
    reset_installed_sources();
});

arch_test!(payload_source_yield_runs_spawned_child_then_resumes_payload, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SPAWN_YIELD_PWD_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "/\npayload.resumed\nlaunch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::YieldNow,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut payload_pid = 0usize;
    let mut yielded_pwd = crate::proc::EMPTY_PROCESS_RECORD;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == "/payload/editor-smoke" {
            payload_pid = record.pid;
        }
        if record.program_path == "/bin/pwd" {
            yielded_pwd = record;
        }
        index += 1;
    }

    testrt::check(payload_pid != 0, "payload process is retained");
    testrt::check_eq(yielded_pwd.program_path, "/bin/pwd");
    testrt::check_eq(yielded_pwd.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(yielded_pwd.parent_pid, payload_pid);

    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.yield_count, 1usize);
    reset_installed_sources();
});

arch_test!(payload_source_yield_runs_spawned_payload_then_resumes_payload, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SPAWN_YIELD_SERVER_SMOKE_SOURCE_BYTES,
        ),
        Ok(()),
    );
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/server-smoke",
            SAMPLE_PAYLOAD_READY_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "payload.resumed\nlaunch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::YieldNow,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Error,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut editor = crate::proc::EMPTY_PROCESS_RECORD;
    let mut server = crate::proc::EMPTY_PROCESS_RECORD;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == "/payload/editor-smoke" {
            editor = record;
        }
        if record.program_path == "/payload/server-smoke" {
            server = record;
        }
        index += 1;
    }

    testrt::check_eq(editor.program_path, "/payload/editor-smoke");
    testrt::check_eq(editor.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(server.program_path, "/payload/server-smoke");
    testrt::check_eq(server.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(server.parent_pid, editor.pid);

    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.yield_count, 1usize);
    reset_installed_sources();
});

arch_test!(payload_source_sleep_tick_yield_wakes_child_then_resumes_payload, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SLEEP_TICK_YIELD_LS_BOOT_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    assert_contains(
        sink_bytes(),
        b"devices\nhelp\nimage\ninput\nmemory\nmounts\npayloads\nprobes\nproof\nprofile\nstatus\n",
    );
    assert_contains(sink_bytes(), b"payload.sleep.resumed\nlaunch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/bin/ls",
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/ls",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::YieldNow,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/ls",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/ls",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut payload_pid = 0usize;
    let mut sleeping_ls = crate::proc::EMPTY_PROCESS_RECORD;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == "/payload/editor-smoke" {
            payload_pid = record.pid;
        }
        if record.program_path == "/bin/ls" {
            sleeping_ls = record;
        }
        index += 1;
    }

    testrt::check(payload_pid != 0, "payload process is retained");
    testrt::check_eq(sleeping_ls.program_path, "/bin/ls");
    testrt::check_eq(sleeping_ls.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(sleeping_ls.parent_pid, payload_pid);
    testrt::check_eq(sleeping_ls.argc, 2usize);
    testrt::check_eq(sleeping_ls.argv0(), "ls");
    testrt::check_eq(sleeping_ls.argv_was_truncated(0), false);
    testrt::check_eq(sleeping_ls.argv1(), "/boot");
    testrt::check_eq(sleeping_ls.argv_was_truncated(1), false);

    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.tick_count, 1usize);
    testrt::check_eq(snapshot.yield_count, 1usize);
    reset_installed_sources();
});

arch_test!(payload_source_sleep_wait_bin_waits_with_scheduler_ticks, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SLEEP_WAIT_PWD_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "/\nlaunch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut payload = crate::proc::EMPTY_PROCESS_RECORD;
    let mut pwd = crate::proc::EMPTY_PROCESS_RECORD;
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        if record.program_path == "/payload/editor-smoke" {
            payload = record;
        }
        if record.program_path == "/bin/pwd" {
            pwd = record;
        }
        index += 1;
    }

    testrt::check_eq(payload.program_path, "/payload/editor-smoke");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(pwd.program_path, "/bin/pwd");
    testrt::check_eq(pwd.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(pwd.parent_pid, payload.pid);
    testrt::check_eq(pwd.argc, 1usize);
    testrt::check_eq(pwd.argv0(), "pwd");

    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.tick_count, 2usize);
    testrt::check_eq(snapshot.ready_len, 0usize);

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    let mut payload_wait_found = false;
    index = 0;
    while index < wait_count {
        let wait = waits[index];
        if wait.parent_pid == payload.pid && wait.child_pid == pwd.pid {
            payload_wait_found = true;
            testrt::check_eq(wait.child_state, crate::proc::ProcessState::Exited);
            testrt::check_eq(wait.exit_code, 0);
            testrt::check_eq(wait.completed, true);
        }
        index += 1;
    }
    testrt::check(payload_wait_found, "payload timed wait row is retained");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"wait.start parent_pid=4 parent_task=4 child_pid=5 child_task=5\n",
    );
    assert_contains(sink_bytes(), b"wait.end parent_pid=4 child_pid=5 child_state=exited exit=0\n");
    assert_contains(
        sink_bytes(),
        b"component=proc severity=info kind=process-wake boot=1 session=1 source=process pid=5 task=5\n",
    );
    reset_installed_sources();
});

arch_test!(payload_source_read_tty_line_uses_payload_syscall_context, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    TTY_READ_CALLS.store(0, Ordering::Relaxed);
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_TTY_READ_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "tty fixture input\nlaunch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::TtyReadLine,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );

    let payload = crate::proc::process(4).expect("payload process is retained");
    testrt::check_eq(payload.program_path, "/payload/editor-smoke");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(payload.parent_pid, 3usize);
    TTY_READ_CALLS.store(0, Ordering::Relaxed);
    reset_installed_sources();
});

arch_test!(payload_source_writes_stdout_under_payload_process, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_STDOUT_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    assert_contains(sink_bytes(), b"payload.stdout\nself:\n");
    assert_contains(sink_bytes(), b"state=running path=/payload/editor-smoke");
    assert_contains(sink_bytes(), b"loader=source-image entry_fn=payload_editor_smoke");
    assert_contains(sink_bytes(), b"argc=1 argv0=editor-smoke argv0_truncated=false");
    assert_contains(sink_bytes(), b"profile=appliance\nlaunch=enabled\n");
    assert_contains(sink_bytes(), b"launch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ProcessSelf,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::FdClose,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    reset_installed_sources();
});

arch_test!(payload_source_service_ready_records_exited_payload_service, {
    crate::klog::reset();
    crate::proc::reset();
    crate::service::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SERVICE_READY_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "launch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ServiceReady,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ServiceExited,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut processes = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let process_count = crate::proc::snapshot(&mut processes);
    let mut payload = crate::proc::EMPTY_PROCESS_RECORD;
    let mut index = 0usize;
    while index < process_count {
        let record = processes[index];
        if record.program_path == "/payload/editor-smoke" {
            payload = record;
        }
        index += 1;
    }
    testrt::check_eq(payload.program_path, "/payload/editor-smoke");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Reaped);

    let mut services = [crate::service::EMPTY_SERVICE_RECORD; crate::service::MAX_SERVICES];
    let service_count = crate::service::snapshot(&mut services);
    let mut found = false;
    index = 0;
    while index < service_count {
        let service = services[index];
        if service.name == "editor" {
            found = true;
            testrt::check_eq(service.target, "/payload/editor-smoke");
            testrt::check_eq(service.state, crate::service::ServiceState::Exited);
            testrt::check_eq(service.reason, crate::service::ServiceReason::ProcessExited);
            testrt::check_eq(service.owner_pid, payload.pid);
            testrt::check_eq(service.owner_task_id, payload.task_id);
            testrt::check_eq(service.service_pid, payload.pid);
            testrt::check_eq(service.service_task_id, payload.task_id);
        }
        index += 1;
    }
    testrt::check(found, "payload service-ready row is retained");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"syscall path=/payload/editor-smoke op=service-ready status=ok service=editor target=/payload/editor-smoke",
    );
    assert_contains(
        sink_bytes(),
        b"syscall path=/payload/editor-smoke op=service-exited status=ok service=editor target=/payload/editor-smoke service_pid=4 service_task=4 reason=process-exited state=exited",
    );
    crate::service::reset();
    reset_installed_sources();
});

arch_test!(payload_source_reads_service_table_through_procfs_fd_path, {
    crate::klog::reset();
    crate::proc::reset();
    crate::service::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SERVICE_TABLE_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    assert_contains(sink_bytes(), b"services:\n");
    assert_contains(
        sink_bytes(),
        b"name=editor target=/payload/editor-smoke state=started reason=running",
    );
    assert_contains(sink_bytes(), b"launch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ServiceReady,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::VfsRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::FdClose,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ServiceExited,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut services = [crate::service::EMPTY_SERVICE_RECORD; crate::service::MAX_SERVICES];
    let service_count = crate::service::snapshot(&mut services);
    let mut found = false;
    let mut index = 0usize;
    while index < service_count {
        let service = services[index];
        if service.name == "editor" {
            found = true;
            testrt::check_eq(service.target, "/payload/editor-smoke");
            testrt::check_eq(service.state, crate::service::ServiceState::Exited);
            testrt::check_eq(service.reason, crate::service::ServiceReason::ProcessExited);
        }
        index += 1;
    }
    testrt::check(found, "payload service snapshot kept service row");

    crate::service::reset();
    reset_installed_sources();
});

arch_test!(payload_source_reads_process_table_through_procfs_fd_path, {
    crate::klog::reset();
    crate::proc::reset();
    crate::service::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_PROCESS_TABLE_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    assert_contains(sink_bytes(), b"processes:\n");
    assert_contains(sink_bytes(), b"state=running path=/payload/editor-smoke");
    assert_contains(sink_bytes(), b"loader=source-image entry_fn=payload_editor_smoke");
    assert_contains(sink_bytes(), b"launch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::VfsRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::FdClose,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
});

arch_test!(payload_source_can_snapshot_boot_profile_and_devices_from_payload_context, {
    crate::klog::reset();
    crate::proc::reset();
    crate::service::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_BOOT_PROFILE_DEVICE_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    assert_contains(sink_bytes(), b"profile=appliance\nlaunch=enabled\npayloads=3\n");
    assert_contains(sink_bytes(), b"input=fixture-input\ninput_mode=fixture\n");
    assert_contains(sink_bytes(), b"devices:\n");
    assert_contains(sink_bytes(), b"uart compat=arm,pl011 mmio=0x1000/0x100 irq=12\n");
    assert_contains(sink_bytes(), b"launch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::BootProfile,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadCatalog,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ConsoleInput,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::DeviceCatalog,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
});

arch_test!(payload_source_reads_exec_pending_and_source_tables_through_procfs_fd_path, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::service::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_EXEC_SOURCE_TABLES_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    assert_contains(sink_bytes(), b"execs:\n");
    assert_contains(
        sink_bytes(),
        b"argv0=editor-smoke status=ok reason=loaded path=/payload/editor-smoke",
    );
    assert_contains(sink_bytes(), b"pending:\n");
    assert_contains(sink_bytes(), b"path=/payload/editor-smoke loader=source-image");
    assert_contains(sink_bytes(), b"sources:\n");
    assert_contains(
        sink_bytes(),
        b"namespace=payload path=/payload/editor-smoke loader=source-image",
    );
    assert_contains(sink_bytes(), b"origin=installed\n");
    assert_contains(sink_bytes(), b"launch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::SnapshotExecLoads,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::VfsRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::FdClose,
        crate::syscall::SyscallStatus::Ok,
    );
    testrt::check(
        syscall_record_count(
            "/payload/editor-smoke",
            crate::syscall::SyscallOp::VfsOpen,
            crate::syscall::SyscallStatus::Ok,
        ) >= 3,
        "payload exec, pending, and source report helpers should all open procfs paths",
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::SnapshotPendingExecs,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::SnapshotSourceStore,
        crate::syscall::SyscallStatus::Ok,
    );
    reset_installed_sources();
});

arch_test!(
    payload_source_reads_scheduler_task_wait_and_syscall_tables_through_procfs_fd_path,
    {
        crate::klog::reset();
        crate::proc::reset();
        crate::exec::reset();
        crate::service::reset();
        crate::syscall::reset();
        reset_installed_sources();
        testrt::check_eq(
            install_source(
                SourceArtifactNamespace::Payload,
                "/payload/editor-smoke",
                SAMPLE_PAYLOAD_SCHEDULER_DIAGNOSTICS_SOURCE_BYTES,
            ),
            Ok(()),
        );

        sink_clear();
        let daemon = daemon(ProfileSummary::new("appliance", true), None);
        let mut session = RootShellSession::new();
        let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
        assert_contains(sink_bytes(), b"scheduler:\n");
        assert_contains(sink_bytes(), b"current_task=");
        assert_contains(sink_bytes(), b"current_pid=");
        assert_contains(sink_bytes(), b"tasks:\n");
        assert_contains(sink_bytes(), b"entry=/payload/editor-smoke");
        assert_contains(sink_bytes(), b"waits:\n");
        assert_contains(sink_bytes(), b"syscalls:\n");
        assert_contains(
            sink_bytes(),
            b"path=/payload/editor-smoke op=scheduler-snapshot status=ok",
        );
        assert_contains(sink_bytes(), b"path=/payload/editor-smoke op=snapshot-tasks status=ok");
        assert_contains(sink_bytes(), b"path=/payload/editor-smoke op=snapshot-waits status=ok");
        assert_contains(sink_bytes(), b"path=/payload/editor-smoke op=snapshot-syscalls status=ok");
        assert_contains(sink_bytes(), b"launch editor-smoke: payload.ready\n");

        assert_syscall_record(
            "/payload/editor-smoke",
            crate::syscall::SyscallOp::SchedulerSnapshot,
            crate::syscall::SyscallStatus::Ok,
        );
        assert_syscall_record(
            "/payload/editor-smoke",
            crate::syscall::SyscallOp::SnapshotTasks,
            crate::syscall::SyscallStatus::Ok,
        );
        assert_syscall_record(
            "/payload/editor-smoke",
            crate::syscall::SyscallOp::SnapshotWaits,
            crate::syscall::SyscallStatus::Ok,
        );
        assert_syscall_record(
            "/payload/editor-smoke",
            crate::syscall::SyscallOp::SnapshotSyscalls,
            crate::syscall::SyscallStatus::Ok,
        );
        assert_syscall_record(
            "/payload/editor-smoke",
            crate::syscall::SyscallOp::VfsOpen,
            crate::syscall::SyscallStatus::Ok,
        );
        assert_syscall_record(
            "/payload/editor-smoke",
            crate::syscall::SyscallOp::VfsRead,
            crate::syscall::SyscallStatus::Ok,
        );
        assert_syscall_record(
            "/payload/editor-smoke",
            crate::syscall::SyscallOp::FdRead,
            crate::syscall::SyscallStatus::Ok,
        );
        assert_syscall_record(
            "/payload/editor-smoke",
            crate::syscall::SyscallOp::FdClose,
            crate::syscall::SyscallStatus::Ok,
        );
        testrt::check(
            syscall_record_count(
                "/payload/editor-smoke",
                crate::syscall::SyscallOp::VfsOpen,
                crate::syscall::SyscallStatus::Ok,
            ) >= 4,
            "payload scheduler, task, wait, and syscall report helpers should all open procfs paths",
        );

        reset_installed_sources();
    }
);

arch_test!(payload_source_service_hold_keeps_payload_resident, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::service::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SERVICE_RESIDENT_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "launch editor-smoke: payload.resident\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ServiceReady,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ServiceHold,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/launch",
        crate::syscall::SyscallOp::WaitReady,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut processes = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let process_count = crate::proc::snapshot(&mut processes);
    let mut launch = crate::proc::EMPTY_PROCESS_RECORD;
    let mut payload = crate::proc::EMPTY_PROCESS_RECORD;
    let mut index = 0usize;
    while index < process_count {
        let record = processes[index];
        if record.program_path == "/bin/launch" {
            launch = record;
        }
        if record.program_path == "/payload/editor-smoke" {
            payload = record;
        }
        index += 1;
    }
    testrt::check_eq(launch.program_path, "/bin/launch");
    testrt::check_eq(launch.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(payload.program_path, "/payload/editor-smoke");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(payload.block_reason, crate::sched::BlockReason::Service);
    testrt::check_eq(payload.parent_pid, crate::proc::ROOTD_PID);

    let task = crate::sched::task(payload.task_id).expect("resident service task is retained");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Blocked);
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::Service);

    let mut services = [crate::service::EMPTY_SERVICE_RECORD; crate::service::MAX_SERVICES];
    let service_count = crate::service::snapshot(&mut services);
    let mut found = false;
    index = 0;
    while index < service_count {
        let service = services[index];
        if service.name == "editor" {
            found = true;
            testrt::check_eq(service.target, "/payload/editor-smoke");
            testrt::check_eq(service.state, crate::service::ServiceState::Started);
            testrt::check_eq(service.reason, crate::service::ServiceReason::Running);
            testrt::check_eq(service.owner_pid, payload.pid);
            testrt::check_eq(service.owner_task_id, payload.task_id);
            testrt::check_eq(service.service_pid, payload.pid);
            testrt::check_eq(service.service_task_id, payload.task_id);
        }
        index += 1;
    }
    testrt::check(found, "resident payload service row is retained");

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    testrt::check_eq(crate::proc::snapshot_waits(&mut waits), 0usize);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"syscall path=/payload/editor-smoke op=service-hold status=ok");
    assert_contains(
        sink_bytes(),
        b"wait.ready parent_pid=3 child_pid=4 child_state=blocked block=service\n",
    );
    assert_contains(
        sink_bytes(),
        b"payload.resident path=/payload/editor-smoke pid=4 task=4 status=payload.resident block=service\n",
    );
    assert_contains(sink_bytes(), b"process.adopt old_parent_pid=3 new_parent_pid=1");
    assert_contains(sink_bytes(), b"path=/payload/editor-smoke");

    crate::service::reset();
    reset_installed_sources();
});

arch_test!(linked_kill_resident_payload_marks_service_failed, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::service::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SERVICE_RESIDENT_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "launch editor-smoke: payload.resident\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"kill 4\n");
    assert_contains(
        sink_bytes(),
        b"kill:\npid=4\npath=/payload/editor-smoke\nstate=failed\nloader=source-image\nentry_fn=payload_editor_smoke\nartifact_body_format=source-image\nartifact_body_inner=none\n",
    );
    assert_contains_prefixed_usize(
        sink_bytes(),
        b"artifact_body_bytes=",
        SAMPLE_PAYLOAD_SERVICE_RESIDENT_SOURCE_BYTES.len(),
        "missing service payload body byte count",
    );
    assert_contains_prefixed_usize(
        sink_bytes(),
        b"artifact_checksum=",
        crate::source_store::source_media_checksum32(SAMPLE_PAYLOAD_SERVICE_RESIDENT_SOURCE_BYTES)
            as usize,
        "missing service payload body checksum",
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ProcessKill,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ServiceFailed,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );

    let payload = crate::proc::process(4).expect("killed resident process remains retained");
    testrt::check_eq(payload.program_path, "/payload/editor-smoke");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(payload.block_reason, crate::sched::BlockReason::None);
    testrt::check_eq(payload.exit_code, 1i32);
    testrt::check_eq(payload.parent_pid, crate::proc::ROOTD_PID);

    let task = crate::sched::task(payload.task_id).expect("killed resident task remains retained");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Failed);
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::None);

    let mut services = [crate::service::EMPTY_SERVICE_RECORD; crate::service::MAX_SERVICES];
    let service_count = crate::service::snapshot(&mut services);
    let mut found = false;
    let mut index = 0usize;
    while index < service_count {
        let service = services[index];
        if service.name == "editor" {
            found = true;
            testrt::check_eq(service.target, "/payload/editor-smoke");
            testrt::check_eq(service.state, crate::service::ServiceState::Failed);
            testrt::check_eq(service.reason, crate::service::ServiceReason::ProcessKilled);
            testrt::check_eq(service.service_pid, payload.pid);
            testrt::check_eq(service.service_task_id, payload.task_id);
        }
        index += 1;
    }
    testrt::check(found, "killed resident service row is retained as failed");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"proc services\n");
    assert_contains(
        sink_bytes(),
        b"name=editor target=/payload/editor-smoke state=failed reason=process-killed owner_pid=4 owner_task=4 service_pid=4 service_task=4",
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"syscall path=/payload/editor-smoke op=service-failed status=ok service=editor target=/payload/editor-smoke service_pid=4 service_task=4 reason=process-killed",
    );
    assert_contains(
        sink_bytes(),
        b"component=syscall severity=info kind=service-failed boot=1 session=1 source=process pid=4 task=4\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/payload/editor-smoke pid=4 task=4 status=error loader=source-image entry_fn=payload_editor_smoke\n",
    );

    crate::service::reset();
    reset_installed_sources();
});

arch_test!(linked_service_stop_resident_payload_marks_service_stopped, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::service::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SERVICE_RESIDENT_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "launch editor-smoke: payload.resident\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"service-stop editor\n");
    testrt::check_eq(
        sink_str(),
        "service-stop:\nname=editor\ntarget=/payload/editor-smoke\nservice_pid=4\nservice_task=4\nstate=stopped\nreason=operator-stop\n",
    );
    assert_syscall_record(
        "/bin/service-stop",
        crate::syscall::SyscallOp::ServiceStop,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ProcessKill,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ServiceStop,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ServiceFailed,
        crate::syscall::SyscallStatus::Ok,
    );

    let payload = crate::proc::process(4).expect("stopped resident process remains retained");
    testrt::check_eq(payload.program_path, "/payload/editor-smoke");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(payload.exit_code, 1i32);

    let caller = crate::proc::process(5).expect("service-stop process remains retained");
    testrt::check_eq(caller.program_path, "/bin/service-stop");
    testrt::check_eq(caller.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(caller.exit_code, 0i32);

    let mut services = [crate::service::EMPTY_SERVICE_RECORD; crate::service::MAX_SERVICES];
    let service_count = crate::service::snapshot(&mut services);
    let mut found = false;
    let mut index = 0usize;
    while index < service_count {
        let service = services[index];
        if service.name == "editor" {
            found = true;
            testrt::check_eq(service.target, "/payload/editor-smoke");
            testrt::check_eq(service.state, crate::service::ServiceState::Stopped);
            testrt::check_eq(service.reason, crate::service::ServiceReason::OperatorStop);
            testrt::check_eq(service.service_pid, payload.pid);
            testrt::check_eq(service.service_task_id, payload.task_id);
        }
        index += 1;
    }
    testrt::check(found, "stopped resident service row is retained as stopped");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"syscall path=/bin/service-stop op=service-stop status=ok service=editor target=/payload/editor-smoke service_pid=4 service_task=4 reason=operator-stop state=stopped loader=linked-bin entry_fn=bin_service_stop",
    );
    assert_contains(
        sink_bytes(),
        b"syscall path=/payload/editor-smoke op=service-stop status=ok service=editor target=/payload/editor-smoke service_pid=4 service_task=4 reason=operator-stop state=stopped",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/service-stop pid=5 task=5 status=ok loader=linked-bin entry_fn=bin_service_stop\n",
    );

    crate::service::reset();
    reset_installed_sources();
});

arch_test!(linked_service_start_stopped_payload_service_launches_new_resident_instance, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::service::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SERVICE_RESIDENT_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    let _ = daemon.run_shell_line(&mut session, b"service-stop editor\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"service-start editor\n");
    testrt::check_eq(
        sink_str(),
        "service-start:\nname=editor\ntarget=/payload/editor-smoke\nresult=payload.resident\nservice_pid=7\nservice_task=7\nstate=started\nreason=running\n",
    );
    assert_syscall_record(
        "/bin/service-start",
        crate::syscall::SyscallOp::ServiceStart,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ServiceReady,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ServiceHold,
        crate::syscall::SyscallStatus::Ok,
    );

    let old_payload = crate::proc::process(4).expect("stopped service process remains retained");
    testrt::check_eq(old_payload.state, crate::proc::ProcessState::Failed);
    let caller = crate::proc::process(6).expect("service-start process remains retained");
    testrt::check_eq(caller.program_path, "/bin/service-start");
    testrt::check_eq(caller.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(caller.exit_code, 0i32);
    let new_payload = crate::proc::process(7).expect("restarted service process is retained");
    testrt::check_eq(new_payload.program_path, "/payload/editor-smoke");
    testrt::check_eq(new_payload.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(new_payload.block_reason, crate::sched::BlockReason::Service);
    testrt::check_eq(new_payload.parent_pid, crate::proc::ROOTD_PID);

    let mut services = [crate::service::EMPTY_SERVICE_RECORD; crate::service::MAX_SERVICES];
    let service_count = crate::service::snapshot(&mut services);
    let mut found = false;
    let mut index = 0usize;
    while index < service_count {
        let service = services[index];
        if service.name == "editor" {
            found = true;
            testrt::check_eq(service.state, crate::service::ServiceState::Started);
            testrt::check_eq(service.reason, crate::service::ServiceReason::Running);
            testrt::check_eq(service.service_pid, 7usize);
            testrt::check_eq(service.service_task_id, 7usize);
        }
        index += 1;
    }
    testrt::check(found, "linked service-start records the new resident service instance");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"syscall path=/bin/service-start op=service-start status=ok service=editor target=/payload/editor-smoke service_pid=7 service_task=7 reason=running state=started result=payload.resident loader=linked-bin entry_fn=bin_service_start",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/service-start pid=6 task=6 status=ok loader=linked-bin entry_fn=bin_service_start\n",
    );

    crate::service::reset();
    reset_installed_sources();
});

arch_test!(linked_service_restart_resident_payload_stops_then_starts_new_instance, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::service::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SERVICE_RESIDENT_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"service-restart editor\n");
    testrt::check_eq(
        sink_str(),
        "service-restart:\nname=editor\ntarget=/payload/editor-smoke\nresult=payload.resident\nservice_pid=6\nservice_task=6\nstate=started\nreason=running\n",
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ServiceStop,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/service-restart",
        crate::syscall::SyscallOp::ServiceRestart,
        crate::syscall::SyscallStatus::Ok,
    );

    let old_payload = crate::proc::process(4).expect("old service process remains retained");
    testrt::check_eq(old_payload.state, crate::proc::ProcessState::Failed);
    let caller = crate::proc::process(5).expect("service-restart process remains retained");
    testrt::check_eq(caller.program_path, "/bin/service-restart");
    testrt::check_eq(caller.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(caller.exit_code, 0i32);
    let new_payload = crate::proc::process(6).expect("new service process is retained");
    testrt::check_eq(new_payload.program_path, "/payload/editor-smoke");
    testrt::check_eq(new_payload.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(new_payload.block_reason, crate::sched::BlockReason::Service);
    testrt::check_eq(new_payload.parent_pid, crate::proc::ROOTD_PID);

    let mut services = [crate::service::EMPTY_SERVICE_RECORD; crate::service::MAX_SERVICES];
    let service_count = crate::service::snapshot(&mut services);
    let mut found = false;
    let mut index = 0usize;
    while index < service_count {
        let service = services[index];
        if service.name == "editor" {
            found = true;
            testrt::check_eq(service.state, crate::service::ServiceState::Started);
            testrt::check_eq(service.reason, crate::service::ServiceReason::Running);
            testrt::check_eq(service.service_pid, 6usize);
            testrt::check_eq(service.service_task_id, 6usize);
        }
        index += 1;
    }
    testrt::check(found, "linked service-restart records the replacement service instance");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"syscall path=/payload/editor-smoke op=service-stop status=ok service=editor target=/payload/editor-smoke service_pid=4 service_task=4 reason=operator-stop state=stopped",
    );
    assert_contains(
        sink_bytes(),
        b"syscall path=/bin/service-restart op=service-restart status=ok service=editor target=/payload/editor-smoke service_pid=6 service_task=6 reason=running state=started result=payload.resident loader=linked-bin entry_fn=bin_service_restart",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/service-restart pid=5 task=5 status=ok loader=linked-bin entry_fn=bin_service_restart\n",
    );

    crate::service::reset();
    reset_installed_sources();
});

arch_test!(root_shell_proc_service_compatibility_paths_are_retired, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::service::reset();
    crate::syscall::reset();
    reset_installed_sources();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();

    let _ = daemon.run_shell_line(&mut session, b"proc service-stop editor\n");
    testrt::check_eq(sink_str(), "proc: too many arguments\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"proc service-start editor\n");
    testrt::check_eq(sink_str(), "proc: too many arguments\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"proc service-restart editor\n");
    testrt::check_eq(sink_str(), "proc: too many arguments\n");

    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::ServiceStop,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::ServiceStart,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::ServiceRestart,
        crate::syscall::SyscallStatus::Ok,
    );

    crate::service::reset();
    reset_installed_sources();
});

arch_test!(payload_source_writes_stderr_under_payload_process, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_STDERR_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    assert_contains(sink_bytes(), b"payload.stderr\nlaunch editor-smoke: payload.ready\n");

    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    reset_installed_sources();
});

arch_test!(root_shell_install_payload_command_installs_source_through_syscall_overlay, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"install-payload server-smoke ready\n");
    assert_contains(sink_bytes(), b"install-payload:\n");
    assert_contains(sink_bytes(), b"name=server-smoke\nnamespace=payload\n");
    assert_contains(sink_bytes(), b"path=/payload/server-smoke\nstatus=ready\nbytes=");
    assert_contains(sink_bytes(), b"\norigin=installed\n");
    assert_syscall_record(
        "/bin/install-payload",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"proc sources\n");
    assert_contains(
        sink_bytes(),
        b"namespace=payload path=/payload/server-smoke loader=source-image bytes=",
    );
    assert_contains(sink_bytes(), b"origin=installed\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"launch server-smoke\n");
    testrt::check_eq(sink_str(), "launch server-smoke: payload.ready\n");
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
});

arch_test!(
    root_shell_install_bin_command_rejects_linked_image_bin_without_source_artifact,
    {
        crate::klog::reset();
        crate::proc::reset();
        crate::syscall::reset();
        reset_installed_sources();
        sink_clear();

        let daemon = daemon(ProfileSummary::new("shell-only", false), None);
        let mut session = RootShellSession::new();
        let _ = daemon.run_shell_line(&mut session, b"install-bin proc ok\n");
        testrt::check_eq(sink_str(), "install-bin: unsupported-image\n");
        assert_syscall_record(
            "/bin/install-bin",
            crate::syscall::SyscallOp::SourceInstall,
            crate::syscall::SyscallStatus::Error,
        );

        reset_installed_sources();
    }
);

arch_test!(root_shell_install_source_commands_run_as_linked_bins, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();

    let _ = daemon.run_shell_line(&mut session, b"install-bin proc ok\n");
    testrt::check_eq(sink_str(), "install-bin: unsupported-image\n");
    assert_syscall_record(
        "/bin/install-bin",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Error,
    );
    reset_installed_sources();

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"install-payload server-smoke ready\n");
    assert_contains(sink_bytes(), b"install-payload:\n");
    assert_contains(sink_bytes(), b"name=server-smoke\nnamespace=payload\n");
    assert_contains(sink_bytes(), b"path=/payload/server-smoke\nstatus=ready\nbytes=");
    assert_contains(sink_bytes(), b"\norigin=installed\n");
    assert_syscall_record(
        "/bin/install-payload",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Ok,
    );
    reset_installed_sources();

    install_source_media(SOURCE_MEDIA_BIN);
    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"install-bin-media proc\n");
    testrt::check_eq(sink_str(), "install-bin-media: unsupported-image\n");
    assert_syscall_record(
        "/bin/install-bin-media",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Error,
    );
    reset_installed_sources();
    clear_source_media();

    install_source_media(SOURCE_MEDIA_PAYLOAD);
    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"install-payload-media server-smoke\n");
    assert_contains(sink_bytes(), b"install-payload-media:\n");
    assert_contains(
        sink_bytes(),
        b"name=server-smoke\nnamespace=payload\npath=/payload/server-smoke\n",
    );
    assert_contains(sink_bytes(), b"storage=selftest-source-media0\n");
    assert_contains(sink_bytes(), b"artifact_bytes=");
    assert_contains(sink_bytes(), b"\nbytes=56\nchecksum=2740680926\n");
    assert_contains(sink_bytes(), b"\norigin=installed\nsource=source-media\n");
    assert_syscall_record(
        "/bin/install-payload-media",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Ok,
    );
    reset_installed_sources();
    clear_source_media();

    install_source_media(SOURCE_MEDIA_BIN_WRONG_PATH);
    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"install-bin-media proc\n");
    assert_contains(sink_bytes(), b"install-bin-media: unsupported-image\n");
    assert_syscall_record(
        "/bin/install-bin-media",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Error,
    );

    reset_installed_sources();
    clear_source_media();
    crate::exec::reset();
});

arch_test!(root_shell_proc_install_compatibility_paths_are_retired, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    clear_source_media();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();

    let _ = daemon.run_shell_line(&mut session, b"proc install-bin proc ok\n");
    testrt::check_eq(sink_str(), "proc: too many arguments\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"proc install-payload server-smoke ready\n");
    testrt::check_eq(sink_str(), "proc: too many arguments\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"proc install-bin-media proc\n");
    testrt::check_eq(sink_str(), "proc: too many arguments\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"proc install-payload-media server-smoke\n");
    testrt::check_eq(sink_str(), "proc: too many arguments\n");

    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Error,
    );

    reset_installed_sources();
    clear_source_media();
});

arch_test!(root_shell_source_image_exit_code_sets_process_status, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    crate::program::reset_media_programs();
    let _ = crate::program::install_media_program("/bin/source-exit")
        .expect("media source-exit descriptor installs");
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Bin,
            "/bin/source-exit",
            SAMPLE_BIN_EXIT_CODE_SEVEN_SOURCE_BYTES,
        ),
        Ok(()),
    );
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let halted = daemon.run_shell_line(&mut session, b"source-exit\n");

    testrt::check_eq(halted, false);
    testrt::check_eq(sink_str(), "exit-code-7\n");
    let process = crate::proc::process(3).expect("exit-code process remains retained");
    testrt::check_eq(process.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(process.exit_code, 7);
    assert_syscall_record(
        "/bin/source-exit",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/source-exit pid=3 task=3 status=exit-code loader=source-image entry_fn=bin_source_exit\n",
    );

    reset_installed_sources();
    crate::program::reset_media_programs();
});

arch_test!(linked_exec_preserves_nonzero_replacement_exit_status, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    reset_installed_sources();
    crate::program::reset_media_programs();
    let _ = crate::program::install_media_program("/bin/source-exit")
        .expect("media source-exit descriptor installs");
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Bin,
            "/bin/source-exit",
            SAMPLE_BIN_EXIT_CODE_SEVEN_SOURCE_BYTES,
        ),
        Ok(()),
    );
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let halted = daemon.run_shell_line(&mut session, b"exec source-exit\n");

    testrt::check_eq(halted, false);
    testrt::check_eq(sink_str(), "exit-code-7\n");
    assert_syscall_record(
        "/bin/source-exit",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/source-exit",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/exec",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/exec",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );

    let process = crate::proc::process(3).expect("replacement process remains retained");
    testrt::check_eq(process.program_path, "/bin/source-exit");
    testrt::check_eq(process.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(process.exit_code, 7);
    testrt::check_eq(process.argv0(), "source-exit");

    reset_installed_sources();
    crate::program::reset_media_programs();
});

arch_test!(root_shell_source_image_exit_code_zero_is_pipeline_success, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    crate::program::reset_media_programs();
    let _ = crate::program::install_media_program("/bin/source-zero")
        .expect("media source-zero descriptor installs");
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Bin,
            "/bin/source-zero",
            SAMPLE_BIN_EXIT_CODE_ZERO_SOURCE_BYTES,
        ),
        Ok(()),
    );
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let halted = daemon.run_shell_line(&mut session, b"source-zero | cat\n");

    testrt::check_eq(halted, false);
    testrt::check_eq(sink_str(), "exit-code-0\n");
    let producer = crate::proc::process(3).expect("pipeline producer remains retained");
    testrt::check_eq(producer.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(producer.exit_code, 0);
    let consumer = crate::proc::process(4).expect("pipeline consumer remains retained");
    testrt::check_eq(consumer.state, crate::proc::ProcessState::Exited);
    assert_syscall_record(
        "/bin/source-zero",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
    crate::program::reset_media_programs();
});

arch_test!(root_shell_rejects_invalid_source_image_exit_code_before_spawn, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    crate::program::reset_media_programs();
    let _ = crate::program::install_media_program("/bin/source-invalid")
        .expect("media source-invalid descriptor installs");
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Bin,
            "/bin/source-invalid",
            SAMPLE_BIN_INVALID_EXIT_CODE_SOURCE_BYTES,
        ),
        Ok(()),
    );
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let halted = daemon.run_shell_line(&mut session, b"source-invalid\n");

    testrt::check_eq(halted, false);
    testrt::check_eq(sink_str(), "error: invalid /bin program image\n");
    testrt::check(crate::proc::process(3).is_none(), "invalid image does not spawn a process");
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Error,
    );

    reset_installed_sources();
    crate::program::reset_media_programs();
});

arch_test!(
    root_shell_install_bin_media_command_rejects_linked_image_bin_without_source_artifact,
    {
        crate::klog::reset();
        crate::proc::reset();
        crate::syscall::reset();
        crate::exec::reset();
        reset_installed_sources();
        clear_source_media();
        install_source_media(SOURCE_MEDIA_BIN);
        sink_clear();

        let daemon = daemon(ProfileSummary::new("shell-only", false), None);
        let mut session = RootShellSession::new();
        let _ = daemon.run_shell_line(&mut session, b"install-bin-media proc\n");
        testrt::check_eq(sink_str(), "install-bin-media: unsupported-image\n");
        assert_syscall_record(
            "/bin/install-bin-media",
            crate::syscall::SyscallOp::SourceInstall,
            crate::syscall::SyscallStatus::Error,
        );

        reset_installed_sources();
        clear_source_media();
    }
);

arch_test!(root_shell_proc_reports_source_media_manifest, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    clear_source_media();
    install_source_media(SOURCE_MEDIA_BIN);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"proc media\n");
    assert_contains(sink_bytes(), b"source-media:\n");
    assert_contains(sink_bytes(), b"storage=selftest-source-media0\n");
    assert_contains(sink_bytes(), b"storage_capacity_bytes=512\n");
    assert_contains(sink_bytes(), b"root_bytes=");
    assert_contains(sink_bytes(), b"\nstatus=ok\nformat=artifact\nentries=1\n");
    assert_contains(sink_bytes(), b"truncated=false\n");
    assert_contains(sink_bytes(), b"- namespace=bin path=/bin/proc offset=0 artifact_bytes=");
    assert_contains(sink_bytes(), b" bytes=76 checksum=1469418292\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SourceMediaSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SourceMediaRead,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"cat /proc/media\n");
    assert_contains(sink_bytes(), b"source-media:\n");
    assert_contains(sink_bytes(), b"format=artifact\n");
    assert_contains(sink_bytes(), b"path=/bin/proc");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SourceMediaSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
    clear_source_media();
});

arch_test!(root_shell_executes_media_discovered_bin, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    install_source_media(SOURCE_MEDIA_DYNAMIC_BIN);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"media-bin\n");
    testrt::check_eq(sink_str(), "media-bin.ok\n");

    let mut loads = [crate::exec::EMPTY_EXEC_LOAD_RECORD; crate::exec::MAX_EXEC_LOAD_RECORDS];
    let count = crate::exec::snapshot_loads(&mut loads);
    testrt::check_eq(count, 1usize);
    testrt::check_eq(loads[0].argv0(), "media-bin");
    testrt::check_eq(loads[0].status, crate::exec::ExecLoadStatus::Ok);
    testrt::check_eq(loads[0].reason, crate::exec::ExecLoadReason::LoadedFromSourceMedia);
    testrt::check_eq(loads[0].origin, crate::exec_artifact::ExecArtifactOrigin::SourceMediaSingle);
    testrt::check_eq(loads[0].path, "/bin/media-bin");
    testrt::check_eq(loads[0].source_path, "/bin/media-bin");
    testrt::check_eq(loads[0].entry_name, "bin_media_bin");
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"ls /bin\n");
    assert_contains(sink_bytes(), b"media-bin\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"help\n");
    assert_contains(sink_bytes(), b"/bin programs: ");
    assert_contains(sink_bytes(), b", media-bin\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"help media-bin\n");
    testrt::check_eq(sink_str(), "media-bin - provider-discovered /bin program\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"cat /bin/media-bin\n");
    assert_contains(sink_bytes(), b"program=media-bin\n");
    assert_contains(sink_bytes(), b"path=/bin/media-bin\n");
    assert_contains(sink_bytes(), b"summary=provider-discovered /bin program\n");
    assert_contains(sink_bytes(), b"entry_fn=bin_media_bin\n");

    reset_installed_sources();
    clear_source_media();
    crate::exec::reset();
});

arch_test!(root_shell_executes_bin_uapi_exec_body_from_exec_bundle, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    install_exec_bundle(EXEC_BUNDLE_BIN_UAPI_DYNAMIC);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"media-bin\n");
    assert_contains(sink_bytes(), b"exec-body-uapi.ok\n");
    assert_contains(sink_bytes(), b"profile=shell-only\n");
    assert_contains(sink_bytes(), b"launch=disabled\n");
    assert_contains(sink_bytes(), b"hello from linked bin\n");

    let mut loads = [crate::exec::EMPTY_EXEC_LOAD_RECORD; crate::exec::MAX_EXEC_LOAD_RECORDS];
    let count = crate::exec::snapshot_loads(&mut loads);
    testrt::check_eq(count, 2usize);
    testrt::check_eq(loads[0].argv0(), "media-bin");
    testrt::check_eq(loads[0].status, crate::exec::ExecLoadStatus::Ok);
    testrt::check_eq(loads[0].reason, crate::exec::ExecLoadReason::Loaded);
    testrt::check_eq(loads[0].origin, crate::exec_artifact::ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(loads[0].artifact_body_format.as_str(), "reovim-exec-body");
    testrt::check_eq(loads[0].artifact_body_inner_format.as_str(), "bin-uapi-v1");
    testrt::check_eq(loads[0].path, "/bin/media-bin");
    testrt::check_eq(loads[0].loader, "reovim-exec-body");
    testrt::check_eq(loads[0].entry_name, "bin_media_bin");
    testrt::check_eq(loads[1].argv0(), "hello");
    testrt::check_eq(loads[1].status, crate::exec::ExecLoadStatus::Ok);
    testrt::check_eq(loads[1].reason, crate::exec::ExecLoadReason::Loaded);
    testrt::check_eq(loads[1].origin, crate::exec_artifact::ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(loads[1].artifact_body_format.as_str(), "linked-image");
    testrt::check_eq(loads[1].artifact_body_inner_format.as_str(), "none");
    testrt::check_eq(loads[1].path, "/bin/hello");
    testrt::check_eq(loads[1].loader, "linked-bin");
    testrt::check_eq(loads[1].entry_name, "bin_hello");

    let mut processes = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let process_count = crate::proc::snapshot(&mut processes);
    let mut hello_child = crate::proc::EMPTY_PROCESS_RECORD;
    let mut process_index = 0usize;
    while process_index < process_count {
        let record = processes[process_index];
        if record.program_path == "/bin/hello" && record.parent_pid == 3 {
            hello_child = record;
        }
        process_index += 1;
    }
    testrt::check_eq(hello_child.program_path, "/bin/hello");
    testrt::check_eq(hello_child.parent_pid, 3usize);
    testrt::check_eq(hello_child.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(hello_child.envc, 1usize);
    testrt::check_eq(hello_child.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(hello_child.env_value(0), "/boot/status");
    testrt::check_eq(hello_child.env_was_truncated(0), false);

    testrt::check_eq(
        syscall_record_count(
            "/bin/media-bin",
            crate::syscall::SyscallOp::ExecSpawn,
            crate::syscall::SyscallStatus::Ok,
        ),
        1usize,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/media-bin",
            crate::syscall::SyscallOp::WaitBegin,
            crate::syscall::SyscallStatus::Ok,
        ),
        1usize,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/media-bin",
            crate::syscall::SyscallOp::WaitEnd,
            crate::syscall::SyscallStatus::Ok,
        ),
        1usize,
    );
    testrt::check_eq(loads[1].entry_name, "bin_hello");
    let mut expected_exec_body = [0u8; crate::source_store::MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let expected_exec_body_len = exec_body_encode(
        &mut expected_exec_body,
        b"bin-uapi-v1",
        SAMPLE_EXEC_BUNDLE_BIN_UAPI_BYTES,
    );
    let expected_exec_body_checksum =
        crate::source_store::source_media_checksum32(&expected_exec_body[..expected_exec_body_len]);
    let media_process = crate::proc::process(3).expect("media-bin process is retained");
    let media_space = crate::mm::address_space(media_process.address_space_id)
        .expect("media-bin address space is retained");
    testrt::check_eq(media_space.text_bytes, expected_exec_body_len);
    testrt::check_eq(media_space.text_checksum, expected_exec_body_checksum);

    let media_ctx = retained_process_context(3);
    let mut media_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(media_ctx));
    let mut report = reovim_uapi_process::ProcessControlReport::empty();
    let self_report = media_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROCESS_SELF,
        reovim_uapi_syscall::SyscallArgs::new([
            (&mut report as *mut reovim_uapi_process::ProcessControlReport) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessControlReport>(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(self_report.decode(), Ok(3usize));
    testrt::check_eq(report.path_bytes(), b"/bin/media-bin");
    testrt::check_eq(report.loader_bytes(), b"reovim-exec-body");
    testrt::check_eq(report.entry_name_bytes(), b"bin_media_bin");
    testrt::check_eq(report.body_format_bytes(), b"reovim-exec-body");
    testrt::check_eq(report.body_inner_bytes(), b"bin-uapi-v1");
    testrt::check_eq(report.body_bytes(), expected_exec_body_len);
    testrt::check_eq(report.body_checksum(), expected_exec_body_checksum);
    drop(media_syscalls);
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::FdClose,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_contains(sink_bytes(), b"exec-body-uapi.ok\n");
    assert_contains(sink_bytes(), b"profile=shell-only\n");
    assert_contains(sink_bytes(), b"launch=disabled\n");

    crate::syscall::reset();
    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"media-bin /boot/status\n");
    assert_contains(sink_bytes(), b"exec-body-uapi.ok\n");
    assert_contains(sink_bytes(), b"selected_profile=shell-only\n");
    assert_contains(sink_bytes(), b"launch_profile_feature=disabled\n");
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::FdClose,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    crate::syscall::reset();
    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"REOVIM_MEDIA_PATH=/boot/status media-bin\n");
    assert_contains(sink_bytes(), b"exec-body-uapi.ok\n");
    assert_contains(sink_bytes(), b"selected_profile=shell-only\n");
    assert_contains(sink_bytes(), b"launch_profile_feature=disabled\n");
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::FdClose,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-bin",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"cat /proc/processes\n");
    assert_contains(
        sink_bytes(),
        b"envc=1 env0_name=REOVIM_MEDIA_PATH env0_value=/boot/status env0_truncated=false",
    );

    reset_installed_sources();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    crate::exec::reset();
});

arch_test!(root_shell_executes_bin_uapi_execve_body_from_exec_bundle, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    install_exec_bundle(EXEC_BUNDLE_BIN_UAPI_EXECVE_DYNAMIC);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"media-exec\n");
    assert_contains(sink_bytes(), b"exec-body-execve.ok\n");
    assert_contains(sink_bytes(), b"hello from linked bin\n");
    testrt::check(
        !contains_bytes(sink_bytes(), b"exec-body-execve.stale\n"),
        "exec-bin-env stops old checked body after replacement",
    );

    let mut loads = [crate::exec::EMPTY_EXEC_LOAD_RECORD; crate::exec::MAX_EXEC_LOAD_RECORDS];
    let count = crate::exec::snapshot_loads(&mut loads);
    testrt::check_eq(count, 2usize);
    testrt::check_eq(loads[0].argv0(), "media-exec");
    testrt::check_eq(loads[0].status, crate::exec::ExecLoadStatus::Ok);
    testrt::check_eq(loads[0].reason, crate::exec::ExecLoadReason::Loaded);
    testrt::check_eq(loads[0].origin, crate::exec_artifact::ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(loads[0].artifact_body_format.as_str(), "reovim-exec-body");
    testrt::check_eq(loads[0].artifact_body_inner_format.as_str(), "bin-uapi-v1");
    testrt::check_eq(loads[0].path, "/bin/media-exec");
    testrt::check_eq(loads[0].loader, "reovim-exec-body");
    testrt::check_eq(loads[0].entry_name, "bin_media_exec");
    testrt::check_eq(loads[1].argv0(), "hello");
    testrt::check_eq(loads[1].status, crate::exec::ExecLoadStatus::Ok);
    testrt::check_eq(loads[1].reason, crate::exec::ExecLoadReason::Loaded);
    testrt::check_eq(loads[1].origin, crate::exec_artifact::ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(loads[1].artifact_body_format.as_str(), "linked-image");
    testrt::check_eq(loads[1].artifact_body_inner_format.as_str(), "none");
    testrt::check_eq(loads[1].path, "/bin/hello");
    testrt::check_eq(loads[1].loader, "linked-bin");
    testrt::check_eq(loads[1].entry_name, "bin_hello");

    let process = crate::proc::process(3).expect("replacement process is retained");
    testrt::check_eq(process.pid, 3usize);
    testrt::check_eq(process.parent_pid, 2usize);
    testrt::check_eq(process.task_id, 3usize);
    testrt::check_eq(process.program_path, "/bin/hello");
    testrt::check_eq(process.loader, "linked-bin");
    testrt::check_eq(process.entry_name, "bin_hello");
    testrt::check_eq(process.image_generation, 2usize);
    testrt::check_eq(process.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(process.exit_code, 0);
    testrt::check_eq(process.argc, 1usize);
    testrt::check_eq(process.argv0(), "hello");
    testrt::check_eq(process.envc, 1usize);
    testrt::check_eq(process.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(process.env_value(0), "/boot/status");
    testrt::check_eq(process.env_was_truncated(0), false);

    let task = crate::sched::task(3).expect("replacement task is retained");
    testrt::check_eq(task.process_id, 3usize);
    testrt::check_eq(task.task_id, 3usize);
    testrt::check_eq(task.entry, "bin_hello");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Exited);

    assert_syscall_record(
        "/bin/media-exec",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-exec",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/media-exec",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/media-exec",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"cat /proc/processes\n");
    assert_contains(sink_bytes(), b"pid=3 ppid=2 task=3 state=exited");
    assert_contains(sink_bytes(), b"path=/bin/hello");
    assert_contains(
        sink_bytes(),
        b"envc=1 env0_name=REOVIM_MEDIA_PATH env0_value=/boot/status env0_truncated=false",
    );

    reset_installed_sources();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    crate::exec::reset();
});

arch_test!(root_shell_executes_bin_uapi_spawn_body_from_exec_bundle, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    install_exec_bundle(EXEC_BUNDLE_BIN_UAPI_SPAWN_DYNAMIC);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"media-spawn\n");
    assert_contains(sink_bytes(), b"exec-body-spawn.ok\n");
    testrt::check(
        !contains_bytes(sink_bytes(), b"hello from linked bin\n"),
        "spawn-bin-env leaves child ready instead of waiting",
    );

    let mut loads = [crate::exec::EMPTY_EXEC_LOAD_RECORD; crate::exec::MAX_EXEC_LOAD_RECORDS];
    let load_count = crate::exec::snapshot_loads(&mut loads);
    testrt::check_eq(load_count, 2usize);
    testrt::check_eq(loads[0].argv0(), "media-spawn");
    testrt::check_eq(loads[0].origin, crate::exec_artifact::ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(loads[0].path, "/bin/media-spawn");
    testrt::check_eq(loads[0].loader, "reovim-exec-body");
    testrt::check_eq(loads[0].entry_name, "bin_media_spawn");
    testrt::check_eq(loads[0].artifact_body_inner_format.as_str(), "bin-uapi-v1");
    testrt::check_eq(loads[1].argv0(), "hello");
    testrt::check_eq(loads[1].origin, crate::exec_artifact::ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(loads[1].path, "/bin/hello");
    testrt::check_eq(loads[1].loader, "linked-bin");
    testrt::check_eq(loads[1].entry_name, "bin_hello");

    let parent = crate::proc::process(3).expect("media-spawn process remains retained");
    testrt::check_eq(parent.program_path, "/bin/media-spawn");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(parent.exit_code, 0);

    let child_before = crate::proc::process(4).expect("spawned child remains retained");
    testrt::check_eq(child_before.program_path, "/bin/hello");
    testrt::check_eq(child_before.parent_pid, 1usize);
    testrt::check_eq(child_before.task_id, 4usize);
    testrt::check_eq(child_before.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(child_before.envc, 1usize);
    testrt::check_eq(child_before.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(child_before.env_value(0), "/boot/status");
    testrt::check_eq(child_before.env_was_truncated(0), false);
    assert_syscall_record(
        "/bin/media-spawn",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/media-spawn",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/media-spawn",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    testrt::check_eq(daemon.run_idle_ready_programs(&mut session), ProgramStatus::Ok);
    assert_contains(sink_bytes(), b"hello from linked bin\n");

    let child_after = crate::proc::process(4).expect("spawned child remains after idle dispatch");
    testrt::check_eq(child_after.program_path, "/bin/hello");
    testrt::check_eq(child_after.parent_pid, 1usize);
    testrt::check_eq(child_after.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(child_after.exit_code, 0);
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    crate::exec::reset();
});

arch_test!(root_shell_executes_bin_uapi_yield_body_from_exec_bundle, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    install_exec_bundle(EXEC_BUNDLE_BIN_UAPI_YIELD_DYNAMIC);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"media-yield\n");
    testrt::check_eq(
        sink_str(),
        "exec-body-yield.before\nhello from linked bin\nexec-body-yield.after\n",
    );

    let mut loads = [crate::exec::EMPTY_EXEC_LOAD_RECORD; crate::exec::MAX_EXEC_LOAD_RECORDS];
    let load_count = crate::exec::snapshot_loads(&mut loads);
    testrt::check_eq(load_count, 2usize);
    testrt::check_eq(loads[0].argv0(), "media-yield");
    testrt::check_eq(loads[0].origin, crate::exec_artifact::ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(loads[0].path, "/bin/media-yield");
    testrt::check_eq(loads[0].loader, "reovim-exec-body");
    testrt::check_eq(loads[0].entry_name, "bin_media_yield");
    testrt::check_eq(loads[0].artifact_body_inner_format.as_str(), "bin-uapi-v1");
    testrt::check_eq(loads[1].argv0(), "hello");
    testrt::check_eq(loads[1].origin, crate::exec_artifact::ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(loads[1].path, "/bin/hello");
    testrt::check_eq(loads[1].loader, "linked-bin");
    testrt::check_eq(loads[1].entry_name, "bin_hello");

    let parent = crate::proc::process(3).expect("media-yield process remains retained");
    testrt::check_eq(parent.program_path, "/bin/media-yield");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(parent.exit_code, 0);

    let child = crate::proc::process(4).expect("yielded child remains retained");
    testrt::check_eq(child.program_path, "/bin/hello");
    testrt::check_eq(child.parent_pid, 3usize);
    testrt::check_eq(child.task_id, 4usize);
    testrt::check_eq(child.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(child.exit_code, 0);
    testrt::check_eq(child.envc, 1usize);
    testrt::check_eq(child.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(child.env_value(0), "/boot/status");
    testrt::check_eq(child.env_was_truncated(0), false);

    assert_syscall_record(
        "/bin/media-yield",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-yield",
        crate::syscall::SyscallOp::YieldNow,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/media-yield",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/media-yield",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let scheduler = crate::sched::snapshot_scheduler();
    testrt::check_eq(scheduler.yield_count, 1usize);
    testrt::check_eq(scheduler.ready_len, 0usize);

    reset_installed_sources();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    crate::exec::reset();
});

arch_test!(root_shell_executes_bin_uapi_tick_body_from_exec_bundle, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    install_exec_bundle(EXEC_BUNDLE_BIN_UAPI_TICK_DYNAMIC);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"media-tick\n");
    testrt::check_eq(
        sink_str(),
        "exec-body-tick.before\nhello from linked bin\nexec-body-tick.after\n",
    );

    let mut loads = [crate::exec::EMPTY_EXEC_LOAD_RECORD; crate::exec::MAX_EXEC_LOAD_RECORDS];
    let load_count = crate::exec::snapshot_loads(&mut loads);
    testrt::check_eq(load_count, 2usize);
    testrt::check_eq(loads[0].argv0(), "media-tick");
    testrt::check_eq(loads[0].origin, crate::exec_artifact::ExecArtifactOrigin::BlockBundle);
    testrt::check_eq(loads[0].path, "/bin/media-tick");
    testrt::check_eq(loads[0].loader, "reovim-exec-body");
    testrt::check_eq(loads[0].entry_name, "bin_media_tick");
    testrt::check_eq(loads[0].artifact_body_inner_format.as_str(), "bin-uapi-v1");
    testrt::check_eq(loads[1].argv0(), "hello");
    testrt::check_eq(loads[1].origin, crate::exec_artifact::ExecArtifactOrigin::ImageLinked);
    testrt::check_eq(loads[1].path, "/bin/hello");
    testrt::check_eq(loads[1].loader, "linked-bin");
    testrt::check_eq(loads[1].entry_name, "bin_hello");

    let parent = crate::proc::process(3).expect("media-tick process remains retained");
    testrt::check_eq(parent.program_path, "/bin/media-tick");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(parent.exit_code, 0);

    let child = crate::proc::process(4).expect("tick-woken child remains retained");
    testrt::check_eq(child.program_path, "/bin/hello");
    testrt::check_eq(child.parent_pid, 3usize);
    testrt::check_eq(child.task_id, 4usize);
    testrt::check_eq(child.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(child.exit_code, 0);
    testrt::check_eq(child.envc, 1usize);
    testrt::check_eq(child.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(child.env_value(0), "/boot/status");
    testrt::check_eq(child.env_was_truncated(0), false);

    assert_syscall_record(
        "/bin/media-tick",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-tick",
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-tick",
        crate::syscall::SyscallOp::YieldNow,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/media-tick",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/media-tick",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let scheduler = crate::sched::snapshot_scheduler();
    testrt::check_eq(scheduler.tick_count, 1usize);
    testrt::check_eq(scheduler.yield_count, 1usize);
    testrt::check_eq(scheduler.ready_len, 0usize);

    reset_installed_sources();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    crate::exec::reset();
});

arch_test!(root_shell_resumes_bin_uapi_sleep_body_from_exec_bundle, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    install_exec_bundle(EXEC_BUNDLE_BIN_UAPI_SLEEP_DYNAMIC);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"media-sleep\n");
    testrt::check_eq(sink_str(), "exec-body-sleep.before\n");

    let sleeper = crate::proc::process(3).expect("media-sleep process remains retained");
    testrt::check_eq(sleeper.program_path, "/bin/media-sleep");
    testrt::check_eq(sleeper.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(sleeper.block_reason, crate::sched::BlockReason::Sleep);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 1usize);
    testrt::check_eq(continuations[0].process_id, sleeper.pid);
    testrt::check_eq(continuations[0].task_id, sleeper.task_id);
    testrt::check_eq(continuations[0].program_path, "/bin/media-sleep");
    testrt::check_eq(continuations[0].loader, "reovim-exec-body");
    testrt::check_eq(continuations[0].entry_name, "bin_media_sleep");
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::SLEEP);
    testrt::check_eq(continuations[0].op, crate::syscall::SyscallOp::ProcessSleep);
    testrt::check_eq(continuations[0].args.a0, 1usize);

    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Blocked,
    );
    assert_no_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"sched tick\n");
    assert_contains(sink_bytes(), b"sched tick:\nticked=true\nstatus=ok\ntick_count=1\n");
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let idle_status = daemon.run_idle_ready_programs(&mut session);
    testrt::check_eq(idle_status, crate::program::ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "exec-body-sleep.after\n");

    let resumed = crate::proc::process(3).expect("media-sleep process remains retained");
    testrt::check_eq(resumed.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(resumed.exit_code, 0);

    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);

    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Blocked,
    );
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let scheduler = crate::sched::snapshot_scheduler();
    testrt::check_eq(scheduler.tick_count, 1usize);
    testrt::check_eq(scheduler.ready_len, 0usize);

    reset_installed_sources();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    crate::exec::reset();
});

arch_test!(root_shell_resumes_bin_uapi_spawn_wait_blocked_child, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    install_exec_bundle(EXEC_BUNDLE_BIN_UAPI_WAIT_SLEEP_DYNAMIC);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"media-wait-sleep\n");
    testrt::check_eq(sink_str(), "exec-body-wait.before\nexec-body-sleep.before\n");

    let parent = crate::proc::process(3).expect("media-wait-sleep process remains retained");
    testrt::check_eq(parent.program_path, "/bin/media-wait-sleep");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(parent.block_reason, crate::sched::BlockReason::WaitChild);
    let child = crate::proc::process(4).expect("media-sleep child remains retained");
    testrt::check_eq(child.program_path, "/bin/media-sleep");
    testrt::check_eq(child.parent_pid, parent.pid);
    testrt::check_eq(child.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(child.block_reason, crate::sched::BlockReason::Sleep);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 2usize);
    let mut found_parent_wait = false;
    let mut found_child_sleep = false;
    let mut index = 0usize;
    while index < continuation_count {
        let continuation = continuations[index];
        if continuation.process_id == parent.pid {
            found_parent_wait = true;
            testrt::check_eq(continuation.program_path, "/bin/media-wait-sleep");
            testrt::check_eq(continuation.loader, "reovim-exec-body");
            testrt::check_eq(continuation.entry_name, "bin_media_wait_sleep");
            testrt::check_eq(continuation.nr, reovim_uapi_syscall::SyscallNr::WAIT);
            testrt::check_eq(continuation.op, crate::syscall::SyscallOp::WaitBegin);
            testrt::check_eq(continuation.memory, crate::syscall::SyscallContinuationMemory::None);
            testrt::check_eq(continuation.args.a0, child.pid);
        }
        if continuation.process_id == child.pid {
            found_child_sleep = true;
            testrt::check_eq(continuation.program_path, "/bin/media-sleep");
            testrt::check_eq(continuation.loader, "reovim-exec-body");
            testrt::check_eq(continuation.entry_name, "bin_media_sleep");
            testrt::check_eq(continuation.nr, reovim_uapi_syscall::SyscallNr::SLEEP);
            testrt::check_eq(continuation.op, crate::syscall::SyscallOp::ProcessSleep);
            testrt::check_eq(continuation.args.a0, 1usize);
        }
        index += 1;
    }
    testrt::check(found_parent_wait, "parent wait continuation is retained");
    testrt::check(found_child_sleep, "child sleep continuation is retained");
    assert_syscall_record_with_message(
        "/bin/media-wait-sleep",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
        "parent wait begin is admitted",
    );
    assert_no_syscall_record(
        "/bin/media-wait-sleep",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"sched tick\n");
    assert_syscall_record_with_message(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
        "sleeping child wakes after scheduler tick",
    );

    sink_clear();
    let idle_status = daemon.run_idle_ready_programs(&mut session);
    testrt::check_eq(idle_status, crate::program::ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "exec-body-sleep.after\n");
    testrt::check(
        !contains_bytes(sink_bytes(), b"exec-body-wait.stale\n"),
        "spawn-wait frame exits with child status instead of continuing stale body",
    );

    let parent = crate::proc::process(3).expect("parent remains retained");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(parent.exit_code, 0);
    let child = crate::proc::process(4).expect("child remains retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(child.exit_code, 0);

    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);

    assert_syscall_record_with_message(
        "/bin/media-wait-sleep",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
        "parent wait continuation replays successfully",
    );
    assert_syscall_record_with_message(
        "/bin/media-wait-sleep",
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Blocked,
        "parent records user-resume block after replay",
    );
    assert_syscall_record_with_message(
        "/bin/media-wait-sleep",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
        "parent wakes from child wait",
    );
    assert_syscall_record_with_message(
        "/bin/media-wait-sleep",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
        "parent exits through child wait status frame",
    );
    assert_syscall_record_with_message(
        "/bin/media-wait-sleep",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
        "parent process exit is recorded",
    );
    assert_syscall_record_with_message(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
        "child process exit is recorded",
    );

    reset_installed_sources();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    crate::exec::reset();
});

arch_test!(payload_source_spawn_wait_bin_resumes_blocked_child, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    install_exec_bundle(EXEC_BUNDLE_BIN_UAPI_SLEEP_DYNAMIC);
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_SPAWN_WAIT_MEDIA_SLEEP_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch editor-smoke\n");
    testrt::check_eq(sink_str(), "exec-body-sleep.before\n");

    let launcher = crate::proc::process(3).expect("launch process remains retained");
    testrt::check_eq(launcher.program_path, "/bin/launch");
    testrt::check_eq(launcher.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(launcher.block_reason, crate::sched::BlockReason::WaitChild);
    let payload = crate::proc::process(4).expect("payload process remains retained");
    testrt::check_eq(payload.program_path, "/payload/editor-smoke");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(payload.block_reason, crate::sched::BlockReason::WaitChild);
    let child = crate::proc::process(5).expect("payload spawn-wait child remains retained");
    testrt::check_eq(child.program_path, "/bin/media-sleep");
    testrt::check_eq(child.parent_pid, payload.pid);
    testrt::check_eq(child.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(child.block_reason, crate::sched::BlockReason::Sleep);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 2usize);
    let mut saw_launch_wait = false;
    let mut saw_child_sleep = false;
    let mut index = 0usize;
    while index < continuation_count {
        let continuation = continuations[index];
        if continuation.process_id == launcher.pid {
            saw_launch_wait = true;
            testrt::check_eq(continuation.nr, reovim_uapi_syscall::SyscallNr::PROCESS_WAIT_READY);
        }
        if continuation.process_id == child.pid {
            saw_child_sleep = true;
            testrt::check_eq(continuation.nr, reovim_uapi_syscall::SyscallNr::SLEEP);
        }
        index += 1;
    }
    testrt::check(saw_launch_wait, "launch retained wait-ready continuation");
    testrt::check(saw_child_sleep, "child retained sleep continuation");
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Blocked,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let _ = daemon.run_shell_line(&mut session, b"sched tick\n");
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let idle_status = daemon.run_idle_ready_programs(&mut session);
    testrt::check_eq(idle_status, crate::program::ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "exec-body-sleep.after\npayload-spawn-wait.after\n");

    let payload = crate::proc::process(4).expect("payload remains retained");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(payload.exit_code, 0);
    let child = crate::proc::process(5).expect("child remains retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(child.exit_code, 0);
    let launcher = crate::proc::process(3).expect("launcher remains retained");
    testrt::check_eq(launcher.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(launcher.block_reason, crate::sched::BlockReason::UserResume);

    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    crate::exec::reset();
});

arch_test!(root_shell_resumes_blocked_bin_uapi_pipeline_producer, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    install_exec_bundle(EXEC_BUNDLE_BIN_UAPI_SLEEP_DYNAMIC);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"media-sleep | cat\n");
    testrt::check_eq(sink_str(), "");

    let producer = crate::proc::process(3).expect("pipeline producer remains retained");
    testrt::check_eq(producer.program_path, "/bin/media-sleep");
    testrt::check_eq(producer.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(producer.block_reason, crate::sched::BlockReason::Sleep);
    testrt::check(
        crate::proc::process(4).is_none(),
        "consumer is retained only after producer completion",
    );

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, producer.pid);
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );

    let _ = daemon.run_shell_line(&mut session, b"sched tick\n");
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let idle_status = daemon.run_idle_ready_programs(&mut session);
    testrt::check_eq(idle_status, crate::program::ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "exec-body-sleep.before\nexec-body-sleep.after\n");

    let producer = crate::proc::process(3).expect("producer remains retained");
    testrt::check_eq(producer.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(producer.exit_code, 0);
    let tick = crate::proc::process(4).expect("sched tick process remains retained");
    testrt::check_eq(tick.program_path, "/bin/sched");
    let consumer = crate::proc::process(5).expect("pipeline consumer remains retained");
    testrt::check_eq(consumer.program_path, "/bin/cat");
    testrt::check_eq(consumer.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(consumer.exit_code, 0);

    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);

    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-sleep",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let scheduler = crate::sched::snapshot_scheduler();
    testrt::check_eq(scheduler.tick_count, 1usize);
    testrt::check_eq(scheduler.ready_len, 0usize);

    reset_installed_sources();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    crate::exec::reset();
});

arch_test!(rootd_idle_replays_blocked_bin_uapi_pipe_write_and_continues_body, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    install_exec_bundle(EXEC_BUNDLE_BIN_UAPI_PIPE_WRITE_DYNAMIC);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let writer_ctx = crate::syscall::exec_bin_from_shell_argv0(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "media-pipe-write",
    )
    .expect("media pipe writer loads");
    let (read_fd, write_fd) =
        crate::syscall::create_process_pipe_fds(crate::proc::SHELL_PID).expect("pipe fds");
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            crate::proc::SHELL_PID,
            write_fd,
            writer_ctx.pid,
            1,
        ),
        Ok(()),
    );

    let writer_status = daemon.run_pending_programs_until(&mut session, writer_ctx, None);
    testrt::check_eq(writer_status, crate::program::ProgramStatus::Blocked);
    let _ = crate::syscall::close_process_fd_for_pipeline(crate::proc::SHELL_PID, write_fd);
    testrt::check_eq(sink_str(), "");

    let writer = crate::proc::process(writer_ctx.pid).expect("writer remains retained");
    testrt::check_eq(writer.program_path, "/bin/media-pipe-write");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::PipeWrite);
    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        1usize,
    );

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, writer_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::WRITE);
    testrt::check_eq(continuations[0].op, crate::syscall::SyscallOp::FdWrite);
    testrt::check_eq(
        continuations[0].memory,
        crate::syscall::SyscallContinuationMemory::RawWriteBuffer,
    );

    let reader_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("reader carrier loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("hello"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            crate::proc::SHELL_PID,
            read_fd,
            reader_ctx.pid,
            3,
        ),
        Ok(()),
    );
    let _ = crate::syscall::close_process_fd_for_pipeline(crate::proc::SHELL_PID, read_fd);
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);
    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let mut first = [0u8; 1];
    let first_read = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            3,
            first.as_mut_ptr() as usize,
            first.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(first_read.decode(), Ok(1usize));
    testrt::check_eq(first, [b'A']);
    drop(reader_syscalls);

    sink_clear();
    testrt::check_eq(daemon.run_idle_ready_programs(&mut session), ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "");

    let writer = crate::proc::process(writer_ctx.pid).expect("writer retained after replay");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(writer.exit_code, 0);
    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        0usize,
    );
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);

    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let mut remaining = [0u8; crate::program::MAX_PROGRAM_PIPE_BYTES];
    let remaining_read = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            3,
            remaining.as_mut_ptr() as usize,
            remaining.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(remaining_read.decode(), Ok(crate::program::MAX_PROGRAM_PIPE_BYTES));
    let mut index = 0usize;
    while index < remaining.len() {
        testrt::check_eq(remaining[index], b'A');
        index += 1;
    }
    let eof = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            3,
            remaining.as_mut_ptr() as usize,
            remaining.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(eof.decode(), Ok(0usize));
    drop(reader_syscalls);

    assert_syscall_record(
        "/bin/media-pipe-write",
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-pipe-write",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-pipe-write",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-pipe-write",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    crate::exec::reset();
});

arch_test!(rootd_idle_replays_blocked_bin_uapi_fd_copy_and_continues_body, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::exec::reset();
    reset_installed_sources();
    clear_source_media();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    install_exec_bundle(EXEC_BUNDLE_BIN_UAPI_FD_COPY_DYNAMIC);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();

    let mut expected = [0u8; crate::syscall::MAX_PROGRAM_VFS_FILE_BYTES];
    let expected_len = {
        let mut len = 0usize;
        while len < crate::program::MAX_PROGRAM_PIPE_BYTES {
            expected[len] = b'A';
            len += 1;
        }
        let syscalls = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
        syscalls.write_boot_status();
        let bytes = sink_bytes();
        let mut index = 0usize;
        while index < bytes.len() {
            testrt::check(len < expected.len(), "expected fd-copy output buffer fits");
            expected[len] = bytes[index];
            index += 1;
            len += 1;
        }
        sink_clear();
        crate::klog::reset();
        len
    };
    testrt::check(
        expected_len > crate::program::MAX_PROGRAM_PIPE_BYTES,
        "boot status must force a blocking fd-copy pipe write",
    );

    let writer_ctx = crate::syscall::exec_bin_from_shell_argv0(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "media-fd-copy",
    )
    .expect("media fd-copy writer loads");
    let (read_fd, write_fd) =
        crate::syscall::create_process_pipe_fds(crate::proc::SHELL_PID).expect("pipe fds");
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            crate::proc::SHELL_PID,
            write_fd,
            writer_ctx.pid,
            1,
        ),
        Ok(()),
    );

    let writer_status = daemon.run_pending_programs_until(&mut session, writer_ctx, None);
    testrt::check_eq(writer_status, crate::program::ProgramStatus::Blocked);
    let _ = crate::syscall::close_process_fd_for_pipeline(crate::proc::SHELL_PID, write_fd);
    testrt::check_eq(sink_str(), "");

    let writer = crate::proc::process(writer_ctx.pid).expect("writer remains retained");
    testrt::check_eq(writer.program_path, "/bin/media-fd-copy");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::PipeWrite);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, writer_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::WRITE);
    testrt::check_eq(
        continuations[0].memory,
        crate::syscall::SyscallContinuationMemory::RawWriteBuffer,
    );

    let reader_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("reader carrier loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("hello"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            crate::proc::SHELL_PID,
            read_fd,
            reader_ctx.pid,
            3,
        ),
        Ok(()),
    );
    let _ = crate::syscall::close_process_fd_for_pipeline(crate::proc::SHELL_PID, read_fd);
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);

    let mut actual = [0u8; crate::syscall::MAX_PROGRAM_VFS_FILE_BYTES];
    let mut actual_len = 0usize;
    while actual_len < expected_len {
        let mut byte = [0u8; 1];
        let read = {
            let mut reader_syscalls =
                crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
            reader_syscalls.dispatch_raw_syscall(
                reovim_uapi_syscall::SyscallNr::READ,
                reovim_uapi_syscall::SyscallArgs::new([
                    3,
                    byte.as_mut_ptr() as usize,
                    byte.len(),
                    0,
                    0,
                    0,
                ]),
            )
        };
        testrt::check_eq(read.decode(), Ok(1usize));
        actual[actual_len] = byte[0];
        actual_len += 1;
        testrt::check_eq(daemon.run_idle_ready_programs(&mut session), ProgramStatus::Ok);
    }

    let writer = crate::proc::process(writer_ctx.pid).expect("writer retained after replay");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(writer.exit_code, 0);
    testrt::check_eq(&actual[..actual_len], &expected[..expected_len]);
    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        0usize,
    );
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);

    let mut eof_buf = [0u8; 1];
    let eof = {
        let mut reader_syscalls =
            crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
        reader_syscalls.dispatch_raw_syscall(
            reovim_uapi_syscall::SyscallNr::READ,
            reovim_uapi_syscall::SyscallArgs::new([
                3,
                eof_buf.as_mut_ptr() as usize,
                eof_buf.len(),
                0,
                0,
                0,
            ]),
        )
    };
    testrt::check_eq(eof.decode(), Ok(0usize));

    assert_syscall_record(
        "/bin/media-fd-copy",
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-fd-copy",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-fd-copy",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media-fd-copy",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
    clear_exec_bundle();
    crate::program::reset_media_programs();
    crate::exec::reset();
});

arch_test!(
    root_shell_install_payload_media_command_installs_payload_source_from_source_media,
    {
        crate::klog::reset();
        crate::proc::reset();
        crate::syscall::reset();
        reset_installed_sources();
        clear_source_media();
        install_source_media(SOURCE_MEDIA_PAYLOAD);
        sink_clear();

        let daemon = daemon(ProfileSummary::new("appliance", true), None);
        let mut session = RootShellSession::new();
        let _ = daemon.run_shell_line(&mut session, b"install-payload-media server-smoke\n");
        assert_contains(sink_bytes(), b"install-payload-media:\n");
        assert_contains(
            sink_bytes(),
            b"name=server-smoke\nnamespace=payload\npath=/payload/server-smoke\n",
        );
        assert_contains(sink_bytes(), b"storage=selftest-source-media0\n");
        assert_contains(sink_bytes(), b"artifact_bytes=");
        assert_contains(sink_bytes(), b"\nbytes=56\nchecksum=2740680926\n");
        assert_contains(sink_bytes(), b"\norigin=installed\nsource=source-media\n");
        assert_syscall_record(
            "/bin/install-payload-media",
            crate::syscall::SyscallOp::SourceMediaRead,
            crate::syscall::SyscallStatus::Ok,
        );
        assert_syscall_record(
            "/bin/install-payload-media",
            crate::syscall::SyscallOp::SourceInstall,
            crate::syscall::SyscallStatus::Ok,
        );

        sink_clear();
        let _ = daemon.run_shell_line(&mut session, b"launch server-smoke\n");
        testrt::check_eq(sink_str(), "/\nlaunch server-smoke: payload.ready\n");
        assert_syscall_record(
            "/payload/server-smoke",
            crate::syscall::SyscallOp::PayloadRun,
            crate::syscall::SyscallStatus::Ok,
        );
        assert_syscall_record(
            "/bin/pwd",
            crate::syscall::SyscallOp::ProcessExit,
            crate::syscall::SyscallStatus::Ok,
        );

        reset_installed_sources();
        clear_source_media();
    }
);

arch_test!(root_shell_launch_discovers_payload_source_from_source_media, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    crate::dump::clear_sink_for_tests();
    reset_installed_sources();
    clear_source_media();
    install_source_media(SOURCE_MEDIA_DYNAMIC_PAYLOAD);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch media-payload\n");
    testrt::check_eq(sink_str(), "/\nlaunch media-payload: payload.ready\n");
    assert_syscall_record(
        "/payload/media-payload",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"cat /proc/execs\n");
    assert_contains(
        sink_bytes(),
        b"argv0=media-payload status=ok reason=loaded-source-media path=/payload/media-payload",
    );
    assert_contains(sink_bytes(), b"origin=source-media-single");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"launch\n");
    assert_contains(
        sink_bytes(),
        b"  media-payload: provider-discovered /payload program loader=source-image entry_fn=payload_media_payload\n",
    );

    reset_installed_sources();
    clear_source_media();
});

arch_test!(root_shell_install_bin_media_rejects_source_media_path_mismatch, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    clear_source_media();
    install_source_media(SOURCE_MEDIA_BIN_WRONG_PATH);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"install-bin-media proc\n");
    assert_contains(sink_bytes(), b"install-bin-media: unsupported-image\n");
    assert_syscall_record(
        "/bin/install-bin-media",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Error,
    );

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"proc sources\n");
    assert_contains(
        sink_bytes(),
        b"namespace=payload path=/payload/reovim loader=source-image bytes=",
    );
    assert_contains(sink_bytes(), b"origin=image\n");

    reset_installed_sources();
    clear_source_media();
});

arch_test!(root_shell_device_runs_as_linked_bin, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"device\n", None);
    assert_contains(sink_bytes(), b"boot_info:\n");
    assert_contains(sink_bytes(), b"  ranges=1\n");
    assert_contains(sink_bytes(), b"  usable_bytes=4096\n");
    assert_contains(sink_bytes(), b"devices:\n");
    assert_contains(sink_bytes(), b"- [0] uart compat=arm,pl011 mmio=0x1000/0x100 irq=12\n");
    assert_syscall_record(
        "/bin/device",
        crate::syscall::SyscallOp::BootInfo,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/device",
        crate::syscall::SyscallOp::DeviceCatalog,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/device pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_device\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"device extra\n", None);
    testrt::check_eq(sink_str(), "device: too many arguments\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/device pid=3 task=3 status=exit-code loader=linked-bin entry_fn=bin_device\n",
    );
});

arch_test!(root_shell_reovim_records_payload_process_lifecycle, {
    run_shell_line_fixture(ProfileSummary::new("appliance", true), b"reovim --tui\n", None);
    testrt::check_eq(sink_str(), "reovim: payload.ready\n");
    assert_syscall_record(
        "/bin/reovim",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/reovim",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/reovim",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/reovim",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/reovim",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/reovim",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/reovim",
        crate::syscall::SyscallOp::PayloadLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/reovim",
        crate::syscall::SyscallOp::PayloadLaunch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/reovim",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    testrt::check(count >= 4, "payload process table includes child process");
    testrt::check_eq(records[2].program_path, "/bin/reovim");
    testrt::check_eq(records[2].loader, "linked-bin");
    testrt::check_eq(records[2].entry_name, "bin_reovim");
    testrt::check_eq(records[2].state, crate::proc::ProcessState::Exited);
    testrt::check_eq(records[2].argc, 2usize);
    testrt::check_eq(records[2].argv0(), "reovim");
    testrt::check_eq(records[2].argv1(), "--tui");
    testrt::check_eq(records[3].program_path, "/payload/reovim");
    testrt::check_eq(records[3].loader, "source-image");
    testrt::check_eq(records[3].entry_name, "payload_reovim");
    testrt::check_eq(records[3].parent_pid, records[2].pid);
    testrt::check_eq(records[3].state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(records[3].exit_code, 0);
    testrt::check_eq(records[3].argc, 2usize);
    testrt::check_eq(records[3].argv0(), "reovim");
    testrt::check_eq(records[3].argv1(), "--tui");

    let mut exec_loads = [crate::exec::EMPTY_EXEC_LOAD_RECORD; crate::exec::MAX_EXEC_LOAD_RECORDS];
    let exec_load_count = crate::exec::snapshot_loads(&mut exec_loads);
    testrt::check(exec_load_count >= 2, "payload launch records executable loads");
    testrt::check_eq(exec_loads[0].argv0(), "reovim");
    testrt::check_eq(exec_loads[0].path, "/bin/reovim");
    testrt::check_eq(exec_loads[0].kind, crate::exec::ExecLoadKind::Bin);
    testrt::check_eq(exec_loads[1].argv0(), "reovim");
    testrt::check_eq(exec_loads[1].path, "/payload/reovim");
    testrt::check_eq(exec_loads[1].loader, "source-image");
    testrt::check_eq(exec_loads[1].entry_name, "payload_reovim");
    testrt::check_eq(exec_loads[1].kind, crate::exec::ExecLoadKind::Payload);

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    testrt::check_eq(wait_count, 1usize);
    testrt::check_eq(waits[0].parent_pid, records[2].pid);
    testrt::check_eq(waits[0].child_pid, records[3].pid);
    testrt::check_eq(waits[0].child_state, crate::proc::ProcessState::Exited);
    testrt::check_eq(waits[0].completed, true);

    let mut tasks = [crate::sched::EMPTY_KERNEL_TASK_RECORD; crate::sched::MAX_KERNEL_TASKS];
    let task_count = crate::sched::snapshot_kernel_tasks(&mut tasks);
    testrt::check(task_count >= 4, "payload scheduler table includes child task");
    testrt::check_eq(tasks[2].entry, "/bin/reovim");
    testrt::check_eq(tasks[3].entry, "/payload/reovim");
    testrt::check_eq(tasks[3].parent_task_id, tasks[2].task_id);
    testrt::check_eq(tasks[3].state, crate::sched::KernelTaskState::Reaped);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"payload.start path=/payload/reovim loader=source-image entry_fn=payload_reovim parent_pid=3 parent_task=3 pid=4 task=4\n",
    );
    assert_contains(
        sink_bytes(),
        b"payload.exit path=/payload/reovim pid=4 task=4 status=payload.ready\n",
    );
    assert_contains(
        sink_bytes(),
        b"wait.start parent_pid=3 parent_task=3 child_pid=4 child_task=4\n",
    );
    assert_contains(sink_bytes(), b"wait.end parent_pid=3 child_pid=4 child_state=exited exit=0\n");
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/reovim pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_reovim\n",
    );
});

arch_test!(linked_reovim_and_launch_preserve_payload_env_requests, {
    run_shell_line_fixture(
        ProfileSummary::new("appliance", true),
        b"reovim REOVIM_MEDIA_PATH=/boot/status --tui\n",
        None,
    );
    testrt::check_eq(sink_str(), "reovim: payload.ready\n");

    let payload = crate::proc::process(4).expect("reovim payload process is retained");
    testrt::check_eq(payload.program_path, "/payload/reovim");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(payload.argc, 2usize);
    testrt::check_eq(payload.argv0(), "reovim");
    testrt::check_eq(payload.argv1(), "--tui");
    testrt::check_eq(payload.envc, 1usize);
    testrt::check_eq(payload.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(payload.env_value(0), "/boot/status");
    testrt::check_eq(payload.env_was_truncated(0), false);
    assert_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );

    {
        let daemon = daemon(ProfileSummary::new("appliance", true), None);
        let mut session = RootShellSession::new();
        let mut inspector = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
        sink_clear();
        inspector
            .write_vfs_file_path("/proc/processes")
            .expect("process diagnostics render");
        assert_contains(
            sink_bytes(),
            b"envc=1 env0_name=REOVIM_MEDIA_PATH env0_value=/boot/status env0_truncated=false",
        );
    }

    run_shell_line_fixture(
        ProfileSummary::new("appliance", true),
        b"launch REOVIM_MEDIA_PATH=/boot/status reovim --tui\n",
        None,
    );
    testrt::check_eq(sink_str(), "launch reovim: payload.ready\n");

    let payload = crate::proc::process(4).expect("launch payload process is retained");
    testrt::check_eq(payload.program_path, "/payload/reovim");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(payload.argc, 2usize);
    testrt::check_eq(payload.argv0(), "reovim");
    testrt::check_eq(payload.argv1(), "--tui");
    testrt::check_eq(payload.envc, 1usize);
    testrt::check_eq(payload.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(payload.env_value(0), "/boot/status");
    testrt::check_eq(payload.env_was_truncated(0), false);
    assert_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );

    {
        let daemon = daemon(ProfileSummary::new("appliance", true), None);
        let mut session = RootShellSession::new();
        let mut inspector = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
        sink_clear();
        inspector
            .write_vfs_file_path("/proc/processes")
            .expect("process diagnostics render");
        assert_contains(
            sink_bytes(),
            b"envc=1 env0_name=REOVIM_MEDIA_PATH env0_value=/boot/status env0_truncated=false",
        );
    }
});

arch_test!(payload_launch_dispatches_older_ready_program_before_payload_child, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let parent_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "reovim",
    )
    .expect("reovim loads");
    let parent_ctx = crate::syscall::exec_bin_from_shell(parent_program, argv1("reovim"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(parent_ctx));
    let _ = crate::syscall::take_pending_exec(parent_ctx.pid);

    let older_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "pwd",
    )
    .expect("pwd loads");
    let older = crate::exec::spawn_bin_program(older_program, argv1("pwd"));

    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(parent_ctx));
    let result = syscalls.launch_payload_by_name("reovim");

    testrt::check_eq(result, PayloadLaunchResult::Ready);
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut found_payload = false;
    let mut found_older = false;
    let mut index = 0usize;
    while index < count {
        if records[index].program_path == "/payload/reovim" {
            found_payload = true;
            testrt::check_eq(records[index].state, crate::proc::ProcessState::Reaped);
            testrt::check_eq(records[index].exit_code, 0);
        }
        if records[index].pid == older.pid {
            found_older = true;
            testrt::check_eq(records[index].program_path, "/bin/pwd");
            testrt::check_eq(records[index].state, crate::proc::ProcessState::Exited);
            testrt::check_eq(records[index].exit_code, 0);
        }
        index += 1;
    }
    testrt::check(found_payload, "payload child process is retained");
    testrt::check(found_older, "older ready program ran before payload child");
});

arch_test!(root_shell_launch_records_failed_payload_process_lifecycle, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch server-smoke\n");
    testrt::check_eq(sink_str(), "launch server-smoke: payload.failed\n");
    assert_syscall_record(
        "/bin/launch",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/launch",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/launch",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/launch",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/launch",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/launch",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/launch",
        crate::syscall::SyscallOp::PayloadLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/launch",
        crate::syscall::SyscallOp::PayloadLaunch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/launch",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    testrt::check(count >= 4, "failed payload process table includes child process");
    testrt::check_eq(records[2].program_path, "/bin/launch");
    testrt::check_eq(records[2].loader, "linked-bin");
    testrt::check_eq(records[2].entry_name, "bin_launch");
    testrt::check_eq(records[2].state, crate::proc::ProcessState::Failed);
    testrt::check_eq(records[3].program_path, "/payload/server-smoke");
    testrt::check_eq(records[3].loader, "source-image");
    testrt::check_eq(records[3].entry_name, "payload_server_smoke");
    testrt::check_eq(records[3].parent_pid, records[2].pid);
    testrt::check_eq(records[3].state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(records[3].exit_code, 1);

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    testrt::check_eq(wait_count, 1usize);
    testrt::check_eq(waits[0].parent_pid, records[2].pid);
    testrt::check_eq(waits[0].child_pid, records[3].pid);
    testrt::check_eq(waits[0].child_state, crate::proc::ProcessState::Failed);
    testrt::check_eq(waits[0].exit_code, 1);
    testrt::check_eq(waits[0].completed, true);

    let mut tasks = [crate::sched::EMPTY_KERNEL_TASK_RECORD; crate::sched::MAX_KERNEL_TASKS];
    let task_count = crate::sched::snapshot_kernel_tasks(&mut tasks);
    testrt::check(task_count >= 4, "failed payload scheduler table includes child task");
    testrt::check_eq(tasks[2].entry, "/bin/launch");
    testrt::check_eq(tasks[3].entry, "/payload/server-smoke");
    testrt::check_eq(tasks[3].parent_task_id, tasks[2].task_id);
    testrt::check_eq(tasks[3].state, crate::sched::KernelTaskState::Reaped);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"payload.exit path=/payload/server-smoke");
    assert_contains(sink_bytes(), b"wait.end parent_pid=");
    assert_contains(sink_bytes(), b"child_state=failed");
    assert_contains(sink_bytes(), b"exit=1\n");
    assert_contains(sink_bytes(), b"exec.path=/bin/launch");
    assert_contains(sink_bytes(), b"loader=linked-bin entry_fn=bin_launch\n");
});

arch_test!(root_shell_payload_source_exit_code_sets_payload_process_status, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/server-smoke",
            SAMPLE_PAYLOAD_EXIT_CODE_SEVEN_SOURCE_BYTES,
        ),
        Ok(()),
    );

    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"launch server-smoke\n");
    testrt::check_eq(sink_str(), "payload-exit-7\nlaunch server-smoke: payload.exit_code(7)\n");
    assert_syscall_record(
        "/bin/launch",
        crate::syscall::SyscallOp::PayloadLaunch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/launch",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    testrt::check(count >= 4, "payload exit-code process table includes child process");
    testrt::check_eq(records[2].program_path, "/bin/launch");
    testrt::check_eq(records[2].state, crate::proc::ProcessState::Failed);
    testrt::check_eq(records[2].exit_code, 7);
    testrt::check_eq(records[3].program_path, "/payload/server-smoke");
    testrt::check_eq(records[3].state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(records[3].exit_code, 7);

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    testrt::check_eq(wait_count, 1usize);
    testrt::check_eq(waits[0].parent_pid, records[2].pid);
    testrt::check_eq(waits[0].child_pid, records[3].pid);
    testrt::check_eq(waits[0].child_state, crate::proc::ProcessState::Failed);
    testrt::check_eq(waits[0].exit_code, 7);
    testrt::check_eq(waits[0].completed, true);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"payload.exit path=/payload/server-smoke");
    assert_contains(sink_bytes(), b"status=payload.exit_code exit=7\n");
    assert_contains(sink_bytes(), b"wait.end parent_pid=");
    assert_contains(sink_bytes(), b"child_state=failed exit=7\n");
    assert_contains(sink_bytes(), b"exec.path=/bin/launch");
    assert_contains(sink_bytes(), b"status=exit-code loader=linked-bin entry_fn=bin_launch\n");

    reset_installed_sources();
});

arch_test!(payload_exec_payload_child_exit_code_propagates_to_parent_payload_exit, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/editor-smoke",
            SAMPLE_PAYLOAD_EXEC_SERVER_SMOKE_SOURCE_BYTES,
        ),
        Ok(()),
    );
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/server-smoke",
            SAMPLE_PAYLOAD_EXIT_CODE_SEVEN_SOURCE_BYTES,
        ),
        Ok(()),
    );

    run_shell_line_fixture(ProfileSummary::new("appliance", true), b"launch editor-smoke\n", None);
    testrt::check_eq(sink_str(), "payload-exit-7\nlaunch editor-smoke: payload.exit_code(7)\n");
    assert_syscall_record_with_message(
        "/bin/launch",
        crate::syscall::SyscallOp::PayloadLaunch,
        crate::syscall::SyscallStatus::Ok,
        "missing /bin/launch payload-launch ok",
    );
    assert_syscall_record_with_message(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::PayloadLaunch,
        crate::syscall::SyscallStatus::Error,
        "missing /payload/editor-smoke payload-launch error",
    );
    assert_syscall_record_with_message(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
        "missing /payload/editor-smoke wait-begin ok",
    );
    assert_syscall_record_with_message(
        "/payload/editor-smoke",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
        "missing /payload/editor-smoke wait-end ok",
    );
    assert_syscall_record_with_message(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Error,
        "missing /payload/server-smoke payload-run error",
    );
    assert_syscall_record_with_message(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
        "missing /payload/server-smoke process-exit error",
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    testrt::check(count >= 5, "nested payload process table includes child payload");
    testrt::check_eq(records[2].program_path, "/bin/launch");
    testrt::check_eq(records[2].state, crate::proc::ProcessState::Failed);
    testrt::check_eq(records[2].exit_code, 7);
    testrt::check_eq(records[3].program_path, "/payload/editor-smoke");
    testrt::check_eq(records[3].state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(records[3].exit_code, 7);
    testrt::check_eq(records[4].program_path, "/payload/server-smoke");
    testrt::check_eq(records[4].state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(records[4].exit_code, 7);
    testrt::check_eq(records[4].parent_pid, records[3].pid);

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    testrt::check_eq(wait_count, 2usize);
    let mut found_launch_wait = false;
    let mut found_payload_wait = false;
    let mut index = 0usize;
    while index < wait_count {
        if waits[index].parent_pid == records[2].pid && waits[index].child_pid == records[3].pid {
            found_launch_wait = true;
            testrt::check_eq(waits[index].child_state, crate::proc::ProcessState::Failed);
            testrt::check_eq(waits[index].exit_code, 7);
            testrt::check_eq(waits[index].completed, true);
        }
        if waits[index].parent_pid == records[3].pid && waits[index].child_pid == records[4].pid {
            found_payload_wait = true;
            testrt::check_eq(waits[index].child_state, crate::proc::ProcessState::Failed);
            testrt::check_eq(waits[index].exit_code, 7);
            testrt::check_eq(waits[index].completed, true);
        }
        index += 1;
    }
    testrt::check(found_launch_wait, "launch waits on editor payload exit code");
    testrt::check(found_payload_wait, "editor payload waits on server payload exit code");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"payload.start path=/payload/server-smoke loader=source-image entry_fn=payload_server_smoke parent_pid=4 parent_task=4 pid=5 task=5\n",
    );
    assert_contains(
        sink_bytes(),
        b"payload.exit path=/payload/server-smoke pid=5 task=5 status=payload.exit_code exit=7\n",
    );
    assert_contains(
        sink_bytes(),
        b"payload.exit path=/payload/editor-smoke pid=4 task=4 status=payload.exit_code exit=7\n",
    );
    assert_contains(sink_bytes(), b"wait.end parent_pid=4 child_pid=5 child_state=failed exit=7\n");
    assert_contains(sink_bytes(), b"wait.end parent_pid=3 child_pid=4 child_state=failed exit=7\n");

    reset_installed_sources();
});

arch_test!(payload_exec_bin_child_exit_code_propagates_to_payload_exit, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    reset_installed_sources();
    crate::program::reset_media_programs();
    let _ = crate::program::install_media_program("/bin/source-exit")
        .expect("media source-exit descriptor installs");
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Bin,
            "/bin/source-exit",
            SAMPLE_BIN_EXIT_CODE_SEVEN_SOURCE_BYTES,
        ),
        Ok(()),
    );
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/server-smoke",
            SAMPLE_PAYLOAD_EXEC_SOURCE_EXIT_SOURCE_BYTES,
        ),
        Ok(()),
    );

    run_shell_line_preserving_log(
        ProfileSummary::new("appliance", true),
        b"launch server-smoke\n",
        None,
    );
    testrt::check_eq(sink_str(), "exit-code-7\nlaunch server-smoke: payload.exit_code(7)\n");
    assert_syscall_record(
        "/bin/launch",
        crate::syscall::SyscallOp::PayloadLaunch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/source-exit",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    testrt::check(count >= 5, "payload child exit-code process table includes /bin child");
    testrt::check_eq(records[2].program_path, "/bin/launch");
    testrt::check_eq(records[2].state, crate::proc::ProcessState::Failed);
    testrt::check_eq(records[2].exit_code, 7);
    testrt::check_eq(records[3].program_path, "/payload/server-smoke");
    testrt::check_eq(records[3].state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(records[3].exit_code, 7);
    testrt::check_eq(records[4].program_path, "/bin/source-exit");
    testrt::check_eq(records[4].state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(records[4].exit_code, 7);

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    testrt::check_eq(wait_count, 2usize);
    let mut found_payload_wait = false;
    let mut found_child_wait = false;
    let mut index = 0usize;
    while index < wait_count {
        if waits[index].parent_pid == records[2].pid && waits[index].child_pid == records[3].pid {
            found_payload_wait = true;
            testrt::check_eq(waits[index].child_state, crate::proc::ProcessState::Failed);
            testrt::check_eq(waits[index].exit_code, 7);
            testrt::check_eq(waits[index].completed, true);
        }
        if waits[index].parent_pid == records[3].pid && waits[index].child_pid == records[4].pid {
            found_child_wait = true;
            testrt::check_eq(waits[index].child_state, crate::proc::ProcessState::Failed);
            testrt::check_eq(waits[index].exit_code, 7);
            testrt::check_eq(waits[index].completed, true);
        }
        index += 1;
    }
    testrt::check(found_payload_wait, "launch waits on payload exit code");
    testrt::check(found_child_wait, "payload waits on child exit code");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"payload.exit path=/payload/server-smoke pid=4 task=4 status=payload.exit_code exit=7\n",
    );
    assert_contains(sink_bytes(), b"wait.end parent_pid=4 child_pid=5 child_state=failed exit=7\n");
    assert_contains(sink_bytes(), b"wait.end parent_pid=3 child_pid=4 child_state=failed exit=7\n");

    reset_installed_sources();
    crate::program::reset_media_programs();
});

arch_test!(payload_spawn_wait_bin_child_exit_code_propagates_to_payload_exit, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    reset_installed_sources();
    crate::program::reset_media_programs();
    let _ = crate::program::install_media_program("/bin/source-exit")
        .expect("media source-exit descriptor installs");
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Bin,
            "/bin/source-exit",
            SAMPLE_BIN_EXIT_CODE_SEVEN_SOURCE_BYTES,
        ),
        Ok(()),
    );
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Payload,
            "/payload/server-smoke",
            SAMPLE_PAYLOAD_SPAWN_WAIT_SOURCE_EXIT_SOURCE_BYTES,
        ),
        Ok(()),
    );

    run_shell_line_preserving_log(
        ProfileSummary::new("appliance", true),
        b"launch server-smoke\n",
        None,
    );
    testrt::check_eq(sink_str(), "exit-code-7\nlaunch server-smoke: payload.exit_code(7)\n");
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Error,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    testrt::check(count >= 5, "payload spawn-wait exit-code table includes /bin child");
    testrt::check_eq(records[2].program_path, "/bin/launch");
    testrt::check_eq(records[2].exit_code, 7);
    testrt::check_eq(records[3].program_path, "/payload/server-smoke");
    testrt::check_eq(records[3].state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(records[3].exit_code, 7);
    testrt::check_eq(records[4].program_path, "/bin/source-exit");
    testrt::check_eq(records[4].state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(records[4].exit_code, 7);

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    testrt::check_eq(wait_count, 2usize);
    let mut found_child_wait = false;
    let mut index = 0usize;
    while index < wait_count {
        if waits[index].parent_pid == records[3].pid && waits[index].child_pid == records[4].pid {
            found_child_wait = true;
            testrt::check_eq(waits[index].child_state, crate::proc::ProcessState::Failed);
            testrt::check_eq(waits[index].exit_code, 7);
            testrt::check_eq(waits[index].completed, true);
        }
        index += 1;
    }
    testrt::check(found_child_wait, "spawn-wait observes child exit code");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"payload.exit path=/payload/server-smoke pid=4 task=4 status=payload.exit_code exit=7\n",
    );
    assert_contains(sink_bytes(), b"wait.end parent_pid=4 child_pid=5 child_state=failed exit=7\n");

    reset_installed_sources();
    crate::program::reset_media_programs();
});

arch_test!(root_shell_physical_proof_commands_match_budget_transcript, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proof\n", None);
    assert_proof_commands_match_budget_transcript(sink_bytes());
});

arch_test!(root_shell_physical_proof_klog_budget_keeps_no_wrap, {
    crate::klog::reset();
    crate::klog::append_bytes(
        b"rootd: boot report\n\
          [  OK  ] Initialized framebuffer console.\n\
          geometry=1280x720x32\n\
          [  OK  ] Installed allocator backend.\n\
          [  OK  ] Installed scheduler/sync backend.\n\
          [  OK  ] Discovered 1 memory ranges.\n\
          [  OK  ] Detected 1 CPUs.\n\
          [  OK  ] Enumerated 9 devices.\n\
          [  OK  ] Selected console input source.\n\
          input=usb-keyboard+uart-fallback mode=live usb_keyboard=ready last_poll=report-ready\n\
          [  OK  ] USB keyboard input provider ready.\n\
          [  OK  ] Selected boot profile.\n\
          profile=shell-only launch=disabled\n\
          [  OK  ] Reached target /bin/init.\n\
          init: userland services ready\n\
          [  OK  ] Started /bin/init.\n\
          [  OK  ] Reached target /bin/sh.\n\
          reovim system kernel shell ready\n\
          [  OK  ] Started /bin/sh.\n\
          [  OK  ] Reached target root shell.\n",
    );
    crate::klog::append_bytes(
        b"syscall path=/bin/init op=init-service-start status=ok service=shell target=/bin/sh loader=linked-bin entry_fn=bin_init\n",
    );
    crate::klog::append_bytes(
        b"syscall path=/bin/sh op=session-shell-start status=ok loader=linked-bin entry_fn=bin_sh\n",
    );
    crate::klog::append_bytes(
        b"syscall path=/bin/sh op=session-line-discipline status=ok line_discipline=argv-v1 pipe_mode=single-pipe loader=linked-bin entry_fn=bin_sh\n",
    );
    crate::klog::append_bytes(b"shell_session.owner=/bin/sh loader=linked-bin entry_fn=bin_sh\n");
    crate::klog::append_bytes(
        b"input.usb_keyboard=ready source=usb-keyboard+uart-fallback last_poll=report-ready\n",
    );

    let mut session = RootShellSession::new();
    session.install_shell_owner("/bin/sh", "linked-bin", "bin_sh");
    let mut index = 0usize;
    while index < PHYSICAL_PROOF_COMMANDS.len() {
        run_physical_proof_shell_line(&mut session, PHYSICAL_PROOF_COMMANDS[index]);
        index += 1;
    }

    let retained = crate::klog::len();
    testrt::check_eq(crate::klog::dropped_bytes(), 0usize);
    testrt::check(
        retained < (crate::klog::CAPACITY * 3) / 4,
        "physical proof kernel log keeps headroom",
    );
});

arch_test!(root_shell_vfs_pwd_ls_cd_and_cat, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    let mut session = RootShellSession::new();

    run_session_shell_line(&mut session, b"pwd\n");
    testrt::check_eq(sink_str(), "/\n");

    run_session_shell_line(&mut session, b"ls /\n");
    assert_contains(sink_bytes(), b"bin\n");
    assert_contains(sink_bytes(), b"boot\n");
    assert_contains(sink_bytes(), b"dev\n");
    assert_contains(sink_bytes(), b"dump\n");
    assert_contains(sink_bytes(), b"log\n");
    assert_contains(sink_bytes(), b"proc\n");

    run_session_shell_line(&mut session, b"cat /boot/profile\n");
    assert_contains(sink_bytes(), b"profile=shell-only\n");
    assert_contains(sink_bytes(), b"launch=disabled\n");
    assert_contains(sink_bytes(), b"payloads=3\n");
    assert_contains(sink_bytes(), b"input=fixture-input\n");
    assert_contains(sink_bytes(), b"usb_keyboard=unavailable\n");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdClose,
        crate::syscall::SyscallStatus::Ok,
    );

    run_session_shell_line(&mut session, b"cat \"/boot/profile\"\n");
    assert_contains(sink_bytes(), b"profile=shell-only\n");

    run_session_shell_line(&mut session, b"cat \"\"\n");
    testrt::check_eq(sink_str(), "cat: empty path\n");

    run_session_shell_line(&mut session, b"ls \"\"\n");
    testrt::check_eq(sink_str(), "ls: empty path\n");

    run_session_shell_line(&mut session, b"cd \"\"\n");
    testrt::check_eq(sink_str(), "cd: empty path\n");

    run_session_shell_line(&mut session, b"ls /boot\n");
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

    run_session_shell_line(&mut session, b"ls /proc\n");
    assert_contains(sink_bytes(), b"address-spaces\n");
    assert_contains(sink_bytes(), b"page-tables\n");
    assert_contains(sink_bytes(), b"pages\n");
    assert_contains(sink_bytes(), b"memory-objects\n");
    assert_contains(sink_bytes(), b"processes\n");
    assert_contains(sink_bytes(), b"syscalls\n");

    run_session_shell_line(&mut session, b"cat /boot/help\n");
    assert_contains(sink_bytes(), b"reovim root shell\n");
    assert_contains(sink_bytes(), b"/bin programs: help, init, sh, clear, screentest");
    assert_contains(sink_bytes(), b"namespace: /bin\n");
    assert_contains(sink_bytes(), b"usage: help [program]\n");
    assert_contains(sink_bytes(), b"details:\n");
    assert_contains(sink_bytes(), b"  help [program] - show program help\n");
    assert_contains(sink_bytes(), b"  clear - clear framebuffer console and terminal\n");
    assert_contains(sink_bytes(), b"  screentest - print renderer diagnostics\n");
    assert_contains(
        sink_bytes(),
        b"    required rows: el: clean, el1: clean-left, el2: clean-all\n",
    );
    assert_contains(sink_bytes(), b"  pwd - print current kernel VFS directory\n");
    assert_contains(sink_bytes(), b"  ls [path] - list a kernel VFS directory\n");
    assert_contains(sink_bytes(), b"  cd [path] - change current kernel VFS directory\n");
    assert_contains(sink_bytes(), b"  cat [path...] - print stdin or kernel VFS pseudo files\n");
    assert_contains(sink_bytes(), b"  read - read one TTY line\n");
    assert_contains(sink_bytes(), b"  mount - print kernel VFS mount table\n");
    assert_contains(sink_bytes(), b"  input - print live console input diagnostics\n");
    assert_contains(sink_bytes(), b"  status - print boot, input, and manual_next summary\n");
    assert_contains(sink_bytes(), b"  proof - print physical input proof checklist\n");
    assert_contains(sink_bytes(), b"  device - print boot memory and device inventory\n");
    assert_contains(sink_bytes(), b"  dmesg [--stats] - print retained kernel log or ring stats\n");
    assert_contains(
        sink_bytes(),
        b"  dump [status|snapshot|sync] - inspect or flush kernel dump state\n",
    );
    assert_contains(
        sink_bytes(),
        b"  sched [status|tick|yield|sleep TICKS] - inspect scheduler state, tick, yield, or sleep current task\n",
    );
    assert_contains(
        sink_bytes(),
        b"  proc [processes|execs|address-spaces|page-tables|pages|memory-objects|media|pending|self|session|services|sources|tasks|waits|syscalls|continuations|scheduler] - inspect process state\n",
    );
    assert_contains(
        sink_bytes(),
        b"  probe target - run lower hardware probe; try `probe help` or `cat /boot/probes`\n",
    );
    assert_contains(
        sink_bytes(),
        b"  launch [NAME=VALUE ...] [payload] [arg...] - list or run registered payloads\n",
    );
    assert_contains(
        sink_bytes(),
        b"  reovim [NAME=VALUE ...] [arg...] - run the default reovim payload alias\n",
    );
    assert_contains(sink_bytes(), b"  hello - print a linked-bin syscall proof\n");
    assert_contains(sink_bytes(), b"  halt - request root daemon shutdown\n");
    assert_contains(
        sink_bytes(),
        b"  sleep TICKS [NAME=VALUE ...] PROGRAM [ARG...] - spawn a retained process blocked until scheduler ticks\n",
    );
    assert_contains(sink_bytes(), b"  wait PID - wait for a retained process\n");
    assert_contains(
        sink_bytes(),
        b"  exec [NAME=VALUE ...] PROGRAM [ARG...] - replace current process image\n",
    );
    assert_contains(sink_bytes(), b"  service-stop NAME - stop a retained resident service\n");
    assert_contains(sink_bytes(), b"  service-start NAME - start a retained payload service\n");
    assert_contains(sink_bytes(), b"  service-restart NAME - restart a retained payload service\n");
    assert_contains(sink_bytes(), b"  session - print active shell session state\n");
    assert_contains(sink_bytes(), b"  services - print retained service table\n");
    assert_contains(sink_bytes(), b"  tasks - print retained task table\n");
    assert_contains(sink_bytes(), b"  waits - print retained wait table\n");
    assert_contains(sink_bytes(), b"  syscalls - print retained syscall trace\n");
    assert_contains(sink_bytes(), b"  continuations - print active syscall continuations\n");
    assert_contains(sink_bytes(), b"  execs - print executable admission table\n");
    assert_contains(sink_bytes(), b"  pending - print pending executable table\n");
    assert_contains(sink_bytes(), b"  sources - print executable source table\n");
    assert_contains(sink_bytes(), b"  media - print executable media status\n");
    assert_contains(sink_bytes(), b"  self - print current process state\n");

    run_session_shell_line(&mut session, b"cat /boot/image\n");
    assert_contains(sink_bytes(), b"package=reovim-os\n");
    assert_contains(sink_bytes(), b"version=test\n");
    assert_contains(sink_bytes(), b"target=fixture-target\n");
    assert_contains(sink_bytes(), b"selected_profile=shell-only\n");
    assert_contains(sink_bytes(), b"profile_request=shell-only\n");
    assert_contains(sink_bytes(), b"bootline=absent\n");
    assert_contains(sink_bytes(), b"launch_profile_feature=disabled\n");

    run_session_shell_line(&mut session, b"cat /boot/input\n");
    assert_contains(sink_bytes(), b"source=fixture-input\n");
    assert_contains(sink_bytes(), b"source_state=ready\n");
    assert_contains(sink_bytes(), b"mode=fixture\n");
    assert_contains(sink_bytes(), b"usb_keyboard=unavailable\n");
    assert_contains(sink_bytes(), b"usb_keyboard_pending_bytes=0\n");
    assert_contains(sink_bytes(), b"usb_keyboard_probe=disabled\n");
    assert_contains(sink_bytes(), b"usb_keyboard_poll_interval_ms=0\n");
    assert_contains(sink_bytes(), b"usb_keyboard_last_poll=not-polled\n");

    run_session_shell_line(&mut session, b"cat /boot/status\n");
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

    run_session_shell_line(&mut session, b"cat /boot/proof\n");
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
    assert_contains(sink_bytes(), b"  help read\n");
    assert_contains(sink_bytes(), b"  help mount\n");
    assert_contains(sink_bytes(), b"  help device\n");
    assert_contains(sink_bytes(), b"  help dmesg\n");
    assert_contains(sink_bytes(), b"  help dump\n");
    assert_contains(sink_bytes(), b"  help sched\n");
    assert_contains(sink_bytes(), b"  help proc\n");
    assert_contains(sink_bytes(), b"  help status\n");
    assert_contains(sink_bytes(), b"  help probe\n");
    assert_contains(sink_bytes(), b"  help launch\n");
    assert_contains(sink_bytes(), b"  help reovim\n");
    assert_contains(sink_bytes(), b"  help hello\n");
    assert_contains(sink_bytes(), b"  hello\n");
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
    assert_contains(sink_bytes(), b"  dump status\n");
    assert_contains(sink_bytes(), b"  dump snapshot\n");
    assert_contains(sink_bytes(), b"  dump sync\n");
    assert_contains(sink_bytes(), b"  cat /log/stats\n");
    assert_contains(sink_bytes(), b"  cat /log/events\n");
    assert_contains(sink_bytes(), b"  proc\n");
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
    assert_contains(
        sink_bytes(),
        b"  dmesg contains input.usb_keyboard=ready source=usb-keyboard+uart-fallback last_poll=report-ready\n",
    );
    assert_contains(
        sink_bytes(),
        b"  dmesg pairs input.line_source=usb-keyboard usb_bytes>0 fallback_bytes=0 line_bytes>0 before shell:<command>\n",
    );
    assert_contains(
        sink_bytes(),
        b"  dmesg contains shell.session path=/bin/sh loader=linked-bin entry_fn=bin_sh line=<command>\n",
    );
    assert_contains(
        sink_bytes(),
        b"  dmesg contains exec.parent path=/bin/<command> ppid=2 parent_task=2\n",
    );
    assert_contains(sink_bytes(), b"  manual_next=type-shell-command\n");
    assert_contains(sink_bytes(), b"  probe usb-keyboard reports manual_next=type-shell-command\n");
    assert_contains(sink_bytes(), b"  detailed help catalog available through /boot/help\n");
    assert_contains(sink_bytes(), b"  screentest includes erase-line mode diagnostics\n");
    assert_contains(sink_bytes(), b"  kernel log stats available through /log/stats\n");
    assert_contains(sink_bytes(), b"  kernel structured events available through /log/events\n");
    assert_contains(sink_bytes(), b"  kernel log dropped_bytes=0\n");
    assert_contains(sink_bytes(), b"  retained dmesg has no [klog] dropped_bytes marker\n");
    assert_contains(
        sink_bytes(),
        b"  retained dmesg includes probe usb-keyboard manual_next output\n",
    );
    assert_contains(sink_bytes(), b"  probe catalog available through /boot/probes\n");
    assert_contains(sink_bytes(), b"  probe targets include usb-keyboard\n");
    assert_contains(sink_bytes(), b"  probe targets include xhci-read-keyboard-report\n");
    assert_contains(sink_bytes(), b"  launch/reovim disabled in shell-only profile\n");
    assert_contains(sink_bytes(), b"  shell.status=ok\n");
    assert_contains(sink_bytes(), b"  shell.status=error for disabled payload programs\n");
    assert_contains(sink_bytes(), b"  halt typed last prints halt: ok and stops root daemon\n");

    run_session_shell_line(&mut session, b"proof\n");
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
    assert_contains(sink_bytes(), b"  help read\n");
    assert_contains(sink_bytes(), b"  help mount\n");
    assert_contains(sink_bytes(), b"  help device\n");
    assert_contains(sink_bytes(), b"  help dmesg\n");
    assert_contains(sink_bytes(), b"  help dump\n");
    assert_contains(sink_bytes(), b"  help sched\n");
    assert_contains(sink_bytes(), b"  help proc\n");
    assert_contains(sink_bytes(), b"  help status\n");
    assert_contains(sink_bytes(), b"  help probe\n");
    assert_contains(sink_bytes(), b"  help launch\n");
    assert_contains(sink_bytes(), b"  help reovim\n");
    assert_contains(sink_bytes(), b"  help hello\n");
    assert_contains(sink_bytes(), b"  hello\n");
    assert_contains(sink_bytes(), b"  help halt\n");
    assert_contains(sink_bytes(), b"  cat /boot/probes\n");
    assert_contains(sink_bytes(), b"expected:\n");
    assert_contains(sink_bytes(), b"  probe targets include pcie\n");
    assert_contains(sink_bytes(), b"  probe targets include xhci-read-keyboard-report\n");
    assert_contains(sink_bytes(), b"  detailed help catalog available through /boot/help\n");
    assert_contains(sink_bytes(), b"  kernel log stats available through /log/stats\n");
    assert_contains(sink_bytes(), b"  dump status available through /bin/dump\n");
    assert_contains(
        sink_bytes(),
        b"  dump sync fails closed until persistent storage is available\n",
    );
    assert_contains(sink_bytes(), b"  halt attempts dump sync before root daemon shutdown\n");
    assert_contains(sink_bytes(), b"  kernel structured events available through /log/events\n");
    assert_contains(sink_bytes(), b"  kernel log dropped_bytes=0\n");
    assert_contains(sink_bytes(), b"  retained dmesg has no [klog] dropped_bytes marker\n");
    assert_contains(
        sink_bytes(),
        b"  dmesg contains input.usb_keyboard=ready source=usb-keyboard+uart-fallback last_poll=report-ready\n",
    );
    assert_contains(
        sink_bytes(),
        b"  dmesg pairs input.line_source=usb-keyboard usb_bytes>0 fallback_bytes=0 line_bytes>0 before shell:<command>\n",
    );
    assert_contains(
        sink_bytes(),
        b"  dmesg contains shell.session path=/bin/sh loader=linked-bin entry_fn=bin_sh line=<command>\n",
    );
    assert_contains(
        sink_bytes(),
        b"  dmesg contains exec.parent path=/bin/<command> ppid=2 parent_task=2\n",
    );
    assert_contains(sink_bytes(), b"  probe catalog available through /boot/probes\n");
    assert_contains(sink_bytes(), b"  launch/reovim disabled in shell-only profile\n");
    assert_contains(sink_bytes(), b"  halt typed last prints halt: ok and stops root daemon\n");
    assert_syscall_record(
        "/bin/proof",
        crate::syscall::SyscallOp::BootImage,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/proof",
        crate::syscall::SyscallOp::BootProfile,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/proof",
        crate::syscall::SyscallOp::PayloadCatalog,
        crate::syscall::SyscallStatus::Ok,
    );
    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"exec.path=/bin/proof");
    assert_contains(sink_bytes(), b"status=ok loader=linked-bin entry_fn=bin_proof\n");

    run_session_shell_line(&mut session, b"proof extra\n");
    testrt::check_eq(sink_str(), "proof: too many arguments\n");
    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"exec.path=/bin/proof");
    assert_contains(sink_bytes(), b"status=exit-code loader=linked-bin entry_fn=bin_proof\n");

    run_session_shell_line(&mut session, b"status\n");
    assert_contains(sink_bytes(), b"package=reovim-os\n");
    assert_contains(sink_bytes(), b"version=test\n");
    assert_contains(sink_bytes(), b"selected_profile=shell-only\n");
    assert_contains(sink_bytes(), b"profile_request=shell-only\n");
    assert_contains(sink_bytes(), b"launch_profile_feature=disabled\n");
    assert_contains(sink_bytes(), b"payloads=3\n");
    assert_contains(sink_bytes(), b"manual_next=probe-help\n");
    assert_syscall_record(
        "/bin/status",
        crate::syscall::SyscallOp::BootImage,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/status",
        crate::syscall::SyscallOp::ConsoleInput,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/status",
        crate::syscall::SyscallOp::BootProfile,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/status",
        crate::syscall::SyscallOp::PayloadCatalog,
        crate::syscall::SyscallStatus::Ok,
    );
    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"exec.path=/bin/status");
    assert_contains(sink_bytes(), b"status=ok loader=linked-bin entry_fn=bin_status\n");

    run_session_shell_line(&mut session, b"status extra\n");
    testrt::check_eq(sink_str(), "status: too many arguments\n");

    run_session_shell_line(&mut session, b"input\n");
    assert_contains(sink_bytes(), b"source=fixture-input\n");
    assert_contains(sink_bytes(), b"usb_keyboard_pending_bytes=0\n");
    assert_contains(sink_bytes(), b"usb_keyboard_probe=disabled\n");

    run_session_shell_line(&mut session, b"input extra\n");
    testrt::check_eq(sink_str(), "input: too many arguments\n");

    run_session_shell_line(&mut session, b"cat /boot/mounts\n");
    assert_contains(sink_bytes(), b"kernel on / type rootfs (ro,pseudo)\n");
    assert_contains(sink_bytes(), b"devices on /dev type devfs (ro,pseudo)\n");

    run_session_shell_line(&mut session, b"ls /dev\n");
    testrt::check_eq(sink_str(), "tty\nuart0\n");

    run_session_shell_line(&mut session, b"device\n");
    assert_contains(sink_bytes(), b"boot_info:\n");
    assert_contains(sink_bytes(), b"  ranges=1\n");
    assert_contains(sink_bytes(), b"  usable_bytes=4096\n");
    assert_contains(sink_bytes(), b"devices:\n");
    assert_contains(sink_bytes(), b"- [0] uart compat=arm,pl011 mmio=0x1000/0x100 irq=12\n");

    run_session_shell_line(&mut session, b"device extra\n");
    testrt::check_eq(sink_str(), "device: too many arguments\n");

    run_session_shell_line(&mut session, b"cat /boot/memory\n");
    assert_contains(sink_bytes(), b"ranges=1\n");
    assert_contains(sink_bytes(), b"usable_bytes=4096\n");
    assert_contains(sink_bytes(), b"cpu_count=2\n");

    run_session_shell_line(&mut session, b"cat /boot/devices\n");
    assert_contains(sink_bytes(), b"- [0] uart compat=arm,pl011 mmio=0x1000/0x100 irq=12\n");

    run_session_shell_line(&mut session, b"cat /boot/probes\n");
    assert_contains(sink_bytes(), b"probe targets:\n");
    assert_contains(sink_bytes(), b"  fixture\n");
    assert_contains(sink_bytes(), b"  usb-keyboard\n");
    assert_contains(sink_bytes(), b"  xhci-read-keyboard-report\n");

    run_session_shell_line(&mut session, b"cat /boot/probes/extra\n");
    testrt::check_eq(sink_str(), "cat: /boot/probes/extra: not a directory\n");

    run_session_shell_line(&mut session, b"cd /dev\n");
    testrt::check_eq(sink_str(), "");

    run_session_shell_line(&mut session, b"pwd\n");
    testrt::check_eq(sink_str(), "/dev\n");

    run_session_shell_line(&mut session, b"ls\n");
    testrt::check_eq(sink_str(), "tty\nuart0\n");

    run_session_shell_line(&mut session, b"cat uart0\n");
    assert_contains(sink_bytes(), b"uart compat=arm,pl011");

    run_session_shell_line(&mut session, b"cd /boot/profile\n");
    testrt::check_eq(sink_str(), "cd: /boot/profile: not a directory\n");

    crate::klog::append_line("boot diagnostics complete");
    run_session_shell_line(&mut session, b"cat /log/dmesg\n");
    assert_contains(sink_bytes(), b"boot diagnostics complete\n");
    assert_contains(sink_bytes(), b"external diagnostics:\nboot diagnostics complete\n");

    run_session_shell_line(&mut session, b"ls /log\n");
    testrt::check_eq(sink_str(), "dmesg\nevents\nstats\n");

    run_session_shell_line(&mut session, b"cat /log/stats\n");
    assert_contains(sink_bytes(), b"boot_id=1\n");
    assert_contains(sink_bytes(), b"session_id=1\n");
    assert_contains(sink_bytes(), b"identity_source=volatile-memory\n");
    assert_contains(sink_bytes(), b"capacity_bytes=65536\n");
    assert_contains(sink_bytes(), b"retained_bytes=");
    assert_contains(sink_bytes(), b"dropped_bytes=0\n");
    assert_contains(sink_bytes(), b"next_event_seq=");
    assert_contains(sink_bytes(), b"retained_events=");

    run_session_shell_line(&mut session, b"cat /log/events\n");
    assert_contains(sink_bytes(), b"events:\n");
    assert_contains(sink_bytes(), b"boot=1");
    assert_contains(sink_bytes(), b"session=1");
    assert_contains(sink_bytes(), b"source=process");
    assert_contains(sink_bytes(), b"component=proc");
    assert_contains(sink_bytes(), b"kind=program-exit");
    assert_contains(sink_bytes(), b"pid=");
    assert_contains(sink_bytes(), b"task=");

    run_session_shell_line(&mut session, b"ls /dump\n");
    testrt::check_eq(sink_str(), "snapshot\nstatus\n");

    run_session_shell_line(&mut session, b"cat /dump/status\n");
    assert_contains(sink_bytes(), b"format=reovim-dump-v1\n");
    assert_contains(sink_bytes(), b"boot_id=1\n");
    assert_contains(sink_bytes(), b"session_id=1\n");
    assert_contains(sink_bytes(), b"identity_source=volatile-memory\n");
    assert_contains(sink_bytes(), b"persistent=unavailable\n");
    assert_contains(sink_bytes(), b"storage=none\n");
    assert_contains(sink_bytes(), b"storage_capacity_bytes=0\n");
    assert_contains(sink_bytes(), b"boot_memory_ranges=1\n");
    assert_contains(sink_bytes(), b"boot_memory_usable_bytes=4096\n");
    assert_contains(sink_bytes(), b"device_records=1\n");
    assert_contains(sink_bytes(), b"proof_state=operator-required\n");
    assert_contains(sink_bytes(), b"panic_state=none\n");
    assert_contains(sink_bytes(), b"panic_records=0\n");
    assert_contains(sink_bytes(), b"klog_next_event_seq=");
    assert_contains(sink_bytes(), b"event_records=");
    assert_contains(sink_bytes(), b"process_records=");
    assert_contains(sink_bytes(), b"exec_load_records=");
    assert_contains(sink_bytes(), b"pending_exec_records=");
    assert_contains(sink_bytes(), b"wait_records=");
    assert_contains(sink_bytes(), b"task_records=");
    assert_contains(sink_bytes(), b"syscall_records=");
    assert_contains(sink_bytes(), b"syscall_continuation_records=");

    run_session_shell_line(&mut session, b"cat /dump/snapshot\n");
    assert_contains(sink_bytes(), b"dump:\n");
    assert_contains(sink_bytes(), b"reovim-dump-v1\n");
    assert_contains(sink_bytes(), b"checksum=");
    assert_contains(sink_bytes(), b"boot:\n");
    assert_contains(sink_bytes(), b"memory_ranges=1\n");
    assert_contains(sink_bytes(), b"memory_usable_bytes=4096\n");
    assert_contains(sink_bytes(), b"devices:\n");
    assert_contains(sink_bytes(), b"class=uart compat=arm,pl011");
    assert_contains(sink_bytes(), b"proof:\n");
    assert_contains(sink_bytes(), b"state=operator-required\n");
    assert_contains(sink_bytes(), b"panic:\n");
    assert_contains(sink_bytes(), b"state=none\n");
    assert_contains(sink_bytes(), b"events:\n");
    assert_contains(sink_bytes(), b"processes:\n");
    assert_contains(sink_bytes(), b"execs:\n");
    assert_contains(sink_bytes(), b"pending:\n");
    assert_contains(sink_bytes(), b"scheduler:\n");
    assert_contains(sink_bytes(), b"syscalls:\n");
    assert_contains(sink_bytes(), b"continuations:\n");
    assert_contains(sink_bytes(), b"tasks:\n");
    assert_contains(sink_bytes(), b"waits:\n");

    run_session_shell_line(&mut session, b"ls /bin\n");
    assert_contains(sink_bytes(), b"help\n");
    assert_contains(sink_bytes(), b"pwd\n");
    assert_contains(sink_bytes(), b"reovim\n");
    assert_contains(sink_bytes(), b"halt\n");

    run_session_shell_line(&mut session, b"cat /bin/help\n");
    assert_contains(sink_bytes(), b"program=help\n");
    assert_contains(sink_bytes(), b"path=/bin/help\n");
    assert_contains(sink_bytes(), b"type=bin\n");
    assert_contains(sink_bytes(), b"loader=linked-bin\n");
    assert_contains(sink_bytes(), b"entry_fn=bin_help\n");

    run_session_shell_line(&mut session, b"ls /proc\n");
    testrt::check_eq(
        sink_str(),
        "address-spaces\ncontinuations\nexecs\nmedia\nmemory-objects\npage-tables\npages\npending\nprocesses\nself\nsession\nservices\nscheduler\nsources\nsyscalls\ntasks\nwaits\n",
    );

    run_session_shell_line(&mut session, b"cat /proc/sources\n");
    assert_contains(sink_bytes(), b"sources:\n");
    assert_contains(
        sink_bytes(),
        b"namespace=payload path=/payload/reovim loader=source-image bytes=",
    );

    run_session_shell_line(&mut session, b"cat /proc/execs\n");
    assert_contains(sink_bytes(), b"execs:\n");
    assert_contains(sink_bytes(), b"argv0=cat status=ok reason=loaded path=/bin/cat");
    assert_contains(
        sink_bytes(),
        b"path=/bin/cat source=/bin/cat loader=linked-bin entry_fn=bin_cat",
    );
    assert_contains(sink_bytes(), b"truncated=false kind=bin");

    run_session_shell_line(&mut session, b"cat /proc/pending\n");
    assert_contains(sink_bytes(), b"pending:\n");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SnapshotPendingExecs,
        crate::syscall::SyscallStatus::Ok,
    );

    run_session_shell_line(&mut session, b"cat /proc/self\n");
    assert_contains(sink_bytes(), b"self:\n");
    assert_contains(sink_bytes(), b"state=running path=/bin/cat");
    assert_contains(sink_bytes(), b"loader=linked-bin entry_fn=bin_cat");
    assert_contains(sink_bytes(), b"argc=2 argv0=cat argv0_truncated=false");

    run_session_shell_line(&mut session, b"cat /proc/processes\n");
    assert_contains(sink_bytes(), b"processes:\n");
    assert_contains(sink_bytes(), b"pid=1 ppid=0 task=1 state=running path=rootd");
    assert_contains(sink_bytes(), b"pid=2 ppid=1 task=2 state=running path=root-shell");
    assert_contains(sink_bytes(), b"state=running path=/bin/cat");
    assert_contains(sink_bytes(), b"argc=2 argv0=cat argv0_truncated=false");
    assert_contains(sink_bytes(), b" address_space=");
    assert_contains(sink_bytes(), b" image_generation=");

    run_session_shell_line(&mut session, b"cat /proc/tasks\n");
    assert_contains(sink_bytes(), b"tasks:\n");
    assert_contains(sink_bytes(), b"task=1 pid=1 parent_task=0 state=running entry=rootd");
    assert_contains(sink_bytes(), b"task=2 pid=2 parent_task=1 state=running entry=root-shell");
    assert_contains(sink_bytes(), b"state=running entry=/bin/cat");
    assert_contains(sink_bytes(), b" runs=");
    assert_contains(sink_bytes(), b" ticks=");

    run_session_shell_line(&mut session, b"cat /proc/scheduler\n");
    assert_contains(sink_bytes(), b"scheduler:\n");
    assert_contains(sink_bytes(), b"current_task=");
    assert_contains(sink_bytes(), b"current_pid=");
    assert_contains(sink_bytes(), b"ready_queue_len=0\n");
    assert_contains(sink_bytes(), b"next_ready_pid=0\n");
    assert_contains(sink_bytes(), b"dispatch_count=");
    assert_contains(sink_bytes(), b"yield_count=");
    assert_contains(sink_bytes(), b"tick_count=");

    run_session_shell_line(&mut session, b"cat /proc/syscalls\n");
    assert_contains(sink_bytes(), b"syscalls:\n");
    assert_contains(
        sink_bytes(),
        b"op=snapshot-syscalls status=ok loader=linked-bin entry_fn=bin_cat\n",
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::VfsRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdClose,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
    );

    run_session_shell_line(&mut session, b"cat /proc/continuations\n");
    assert_contains(sink_bytes(), b"continuations:\n");

    run_session_shell_line(&mut session, b"continuations\n");
    assert_contains(sink_bytes(), b"continuations:\n");
    assert_syscall_record(
        "/bin/continuations",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/continuations",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/continuations",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
    );

    run_session_shell_line(&mut session, b"proc continuations\n");
    assert_contains(sink_bytes(), b"continuations:\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );

    run_session_shell_line(&mut session, b"cat /proc/waits\n");
    assert_contains(sink_bytes(), b"waits:\n");
});

arch_test!(root_shell_proc_mm_diagnostics_are_fd_readable, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    let mut session = RootShellSession::new();

    run_session_shell_line(&mut session, b"proc address-spaces\n");
    assert_contains(sink_bytes(), b"address-spaces:\n");
    assert_contains(sink_bytes(), b"path=/bin/proc");
    assert_contains(sink_bytes(), b"loader=linked-bin");
    assert_contains(sink_bytes(), b"entry_fn=bin_proc");
    assert_contains(sink_bytes(), b"page_table=");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SnapshotAddressSpaces,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );

    run_session_shell_line(&mut session, b"cat /proc/page-tables\n");
    assert_contains(sink_bytes(), b"address-space-page-tables:\n");
    assert_contains(sink_bytes(), b"address_space=");
    assert_contains(sink_bytes(), b"root_table=");
    assert_contains(sink_bytes(), b"mapped_pages=");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SnapshotAddressSpacePageTables,
        crate::syscall::SyscallStatus::Ok,
    );

    run_session_shell_line(&mut session, b"cat /proc/pages\n");
    assert_contains(sink_bytes(), b"address-space-pages:\n");
    assert_contains(sink_bytes(), b"page_table=");
    assert_contains(sink_bytes(), b"virtual_start=");
    assert_contains(sink_bytes(), b"flags=");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SnapshotAddressSpacePages,
        crate::syscall::SyscallStatus::Ok,
    );

    run_session_shell_line(&mut session, b"cat /proc/memory-objects\n");
    assert_contains(sink_bytes(), b"address-space-objects:\n");
    assert_contains(sink_bytes(), b"kind=");
    assert_contains(sink_bytes(), b"backing=");
    assert_contains(sink_bytes(), b"page_count=");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SnapshotAddressSpaceObjects,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(root_shell_exec_records_program_process_and_audit, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"pwd\n", None);
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::StdioAttach,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"shell: pwd\n");
    assert_contains(
        sink_bytes(),
        b"shell.session path=root-shell loader=kernel entry_fn=root_shell line=pwd\n",
    );
    assert_contains(sink_bytes(), b"shell.status=ok\n");
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_pwd\n",
    );
    assert_contains(sink_bytes(), b"exec.parent path=/bin/pwd pid=3 ppid=2 task=3 parent_task=2\n");
    assert_contains(
        sink_bytes(),
        b"component=sched severity=info kind=scheduler-dispatch boot=1 session=1 source=process pid=3 task=3\n",
    );
    assert_contains(
        sink_bytes(),
        b"component=proc severity=info kind=program-exit boot=1 session=1 source=process pid=3 task=3\n",
    );

    sink_clear();
    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    testrt::check(count >= 3, "process table includes command process");
    testrt::check_eq(records[0].program_path, "rootd");
    testrt::check_eq(records[0].loader, "kernel");
    testrt::check_eq(records[0].entry_name, "rootd_main");
    testrt::check_eq(records[1].program_path, "root-shell");
    testrt::check_eq(records[1].loader, "kernel");
    testrt::check_eq(records[1].entry_name, "root_shell");
    testrt::check_eq(records[2].program_path, "/bin/pwd");
    testrt::check_eq(records[2].parent_pid, crate::proc::SHELL_PID);
    testrt::check_eq(records[2].loader, "linked-bin");
    testrt::check_eq(records[2].entry_name, "bin_pwd");
    testrt::check_eq(records[2].state, crate::proc::ProcessState::Exited);

    let mut tasks = [crate::sched::EMPTY_KERNEL_TASK_RECORD; crate::sched::MAX_KERNEL_TASKS];
    let task_count = crate::sched::snapshot_kernel_tasks(&mut tasks);
    testrt::check(task_count >= 3, "scheduler table includes command task");
    testrt::check_eq(tasks[2].entry, "/bin/pwd");
    testrt::check_eq(tasks[2].parent_task_id, crate::proc::SHELL_PID);
    testrt::check_eq(tasks[2].state, crate::sched::KernelTaskState::Exited);
});

arch_test!(root_shell_session_audit_records_bin_sh_owner_after_boot_handoff, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    session.install_shell_owner("/bin/sh", "linked-bin", "bin_sh");

    let _ = daemon.run_shell_line(&mut session, b"pwd\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"shell: pwd\n");
    assert_contains(
        sink_bytes(),
        b"shell.session path=/bin/sh loader=linked-bin entry_fn=bin_sh line=pwd\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_pwd\n",
    );
    assert_contains(sink_bytes(), b"exec.parent path=/bin/pwd pid=3 ppid=2 task=3 parent_task=2\n");
});

arch_test!(root_shell_runs_scheduler_selected_pending_program_before_new_command, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let help = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "help",
    )
    .expect("help loads");
    let help_ctx = crate::syscall::exec_bin_from_shell(help, argv1("help"));
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"pwd\n");

    assert_contains(sink_bytes(), b"reovim root shell\n");
    assert_contains(sink_bytes(), b"usage: help [program]\n/\n");
    assert_syscall_record(
        "/bin/help",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/help",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/help",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut found_pwd = false;
    let mut found_help = false;
    let mut index = 0usize;
    while index < count {
        if records[index].program_path == "/bin/pwd" {
            found_pwd = true;
            testrt::check_eq(records[index].state, crate::proc::ProcessState::Exited);
            testrt::check_eq(records[index].exit_code, 0);
        }
        if records[index].pid == help_ctx.pid {
            found_help = true;
            testrt::check_eq(records[index].program_path, "/bin/help");
            testrt::check_eq(records[index].state, crate::proc::ProcessState::Exited);
            testrt::check_eq(records[index].exit_code, 0);
        }
        index += 1;
    }
    testrt::check(found_pwd, "spawned /bin/pwd process is retained");
    testrt::check(found_help, "scheduler-selected pending help process is retained");

    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.current_task_id, crate::proc::SHELL_PID);
    testrt::check_eq(snapshot.current_process_id, crate::proc::SHELL_PID);
});

arch_test!(root_shell_sched_program_reports_and_yields, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"sched\n", None);
    assert_contains(sink_bytes(), b"scheduler:\n");
    assert_contains(sink_bytes(), b"current_pid=3\n");
    assert_contains(sink_bytes(), b"ready_queue_len=0\n");
    assert_contains(sink_bytes(), b"next_ready_pid=0\n");
    assert_contains(sink_bytes(), b"yield_count=0\n");
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::SchedulerSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"sched tick\n", None);
    assert_contains(
        sink_bytes(),
        b"sched tick:\nticked=true\nstatus=ok\ntick_count=1\nscheduler:\n",
    );
    assert_contains(sink_bytes(), b"tick_count=1\n");
    assert_contains(sink_bytes(), b"current_pid=3\n");
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::SchedulerSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"sched yield\n", None);
    assert_contains(sink_bytes(), b"sched yield:\nyielded=false\nstatus=no-peer\nscheduler:\n");
    assert_contains(sink_bytes(), b"yield_count=1\n");
    assert_contains(sink_bytes(), b"current_pid=3\n");
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::YieldNow,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::SchedulerSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"sched sleep 1\n", None);
    testrt::check_eq(sink_str(), "");
    let sleeper = crate::proc::process(3).expect("sched sleep process retained");
    testrt::check_eq(sleeper.program_path, "/bin/sched");
    testrt::check_eq(sleeper.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(sleeper.block_reason, crate::sched::BlockReason::Sleep);
    let sleeper_task = crate::sched::task(sleeper.task_id).expect("sched sleep task retained");
    testrt::check_eq(sleeper_task.state, crate::sched::KernelTaskState::Blocked);
    testrt::check_eq(sleeper_task.block_reason, crate::sched::BlockReason::Sleep);
    testrt::check_eq(sleeper_task.wake_tick, 1usize);
    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, sleeper.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::SLEEP);
    testrt::check_eq(continuations[0].op, crate::syscall::SyscallOp::ProcessSleep);
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Blocked,
    );
    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"shell.status=blocked\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"sched unknown\n", None);
    testrt::check_eq(sink_str(), "sched: unknown subcommand\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"sched status extra\n", None);
    testrt::check_eq(sink_str(), "sched: too many arguments\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"sched sleep\n", None);
    testrt::check_eq(sink_str(), "sched sleep: missing ticks\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"sched sleep 0\n", None);
    testrt::check_eq(sink_str(), "sched sleep: invalid ticks\n");
});

arch_test!(program_yield_without_current_process_is_unavailable, {
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let mut syscalls = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
    let result = syscalls.yield_now_result();

    testrt::check_eq(result.status, crate::syscall::SchedulerYieldStatus::Unavailable);
    testrt::check_eq(result.yielded, false);
    testrt::check_eq(result.selected_pid, 0usize);
    testrt::check_eq(result.selected_task_id, 0usize);
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::YieldNow,
        crate::syscall::SyscallStatus::Unavailable,
    );
});

arch_test!(program_scheduler_tick_without_current_process_is_unavailable, {
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let syscalls = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
    let result = syscalls.scheduler_tick_result();

    testrt::check_eq(result.status, crate::syscall::SchedulerTickStatus::Unavailable);
    testrt::check_eq(result.ticked, false);
    testrt::check_eq(result.task_id, 0usize);
    testrt::check_eq(result.tick_count, 0usize);
    testrt::check_eq(result.task_ticks, 0usize);
    testrt::check_eq(result.woken_count, 0usize);
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallStatus::Unavailable,
    );
});

arch_test!(program_sleep_without_current_process_is_unavailable, {
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let mut syscalls = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
    let result = syscalls.sleep_current_for_ticks_result(1);

    testrt::check_eq(result.status, crate::syscall::SchedulerSleepStatus::Unavailable);
    testrt::check_eq(result.slept, false);
    testrt::check_eq(result.pid, 0usize);
    testrt::check_eq(result.task_id, 0usize);
    testrt::check_eq(result.wake_tick, 0usize);
    testrt::check_eq(result.tick_count, 0usize);
    testrt::check_eq(result.dispatched_count, 0usize);
    testrt::check_eq(result.woken_count, 0usize);
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallStatus::Unavailable,
    );
});

arch_test!(program_yield_runs_older_ready_program_then_resumes_current, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();

    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "sched",
    )
    .expect("sched loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("sched"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid);

    let older_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "pwd",
    )
    .expect("pwd loads");
    let older = crate::exec::spawn_bin_program(older_program, argv1("pwd"));

    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let yield_result = syscalls.yield_now_result();

    testrt::check_eq(yield_result.status, crate::syscall::SchedulerYieldStatus::Yielded);
    testrt::check(yield_result.yielded, "yield switches to older ready program");
    testrt::check_eq(yield_result.selected_pid, older.pid);
    testrt::check_eq(yield_result.selected_task_id, older.task_id);
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::YieldNow,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut found_current = false;
    let mut found_older = false;
    let mut index = 0usize;
    while index < count {
        if records[index].pid == current_ctx.pid {
            found_current = true;
            testrt::check_eq(records[index].state, crate::proc::ProcessState::Running);
        }
        if records[index].pid == older.pid {
            found_older = true;
            testrt::check_eq(records[index].program_path, "/bin/pwd");
            testrt::check_eq(records[index].state, crate::proc::ProcessState::Exited);
            testrt::check_eq(records[index].exit_code, 0);
        }
        index += 1;
    }
    testrt::check(found_current, "yielding process is retained");
    testrt::check(found_older, "older ready program ran during yield");

    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.current_task_id, current_ctx.task_id);
    testrt::check_eq(snapshot.current_process_id, current_ctx.pid);
    testrt::check_eq(snapshot.ready_len, 0usize);
});

arch_test!(program_sleep_dispatches_ready_work_then_resumes_current, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();

    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "sched",
    )
    .expect("sched loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("sched"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid);

    let older_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "pwd",
    )
    .expect("pwd loads");
    let older = crate::exec::spawn_bin_program(older_program, argv1("pwd"));

    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let sleep_result = syscalls.sleep_current_for_ticks_result(1);

    testrt::check_eq(sleep_result.status, crate::syscall::SchedulerSleepStatus::Ok);
    testrt::check(sleep_result.slept, "current process slept and resumed");
    testrt::check_eq(sleep_result.pid, current_ctx.pid);
    testrt::check_eq(sleep_result.task_id, current_ctx.task_id);
    testrt::check_eq(sleep_result.wake_tick, 1usize);
    testrt::check_eq(sleep_result.tick_count, 1usize);
    testrt::check_eq(sleep_result.dispatched_count, 1usize);
    testrt::check_eq(sleep_result.woken_count, 1usize);
    testrt::check_eq(sink_str(), "/\n");

    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let current = crate::proc::process(current_ctx.pid).expect("sleeping process retained");
    testrt::check_eq(current.state, crate::proc::ProcessState::Running);
    let older_record = crate::proc::process(older.pid).expect("older process retained");
    testrt::check_eq(older_record.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(older_record.exit_code, 0);

    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.current_task_id, current_ctx.task_id);
    testrt::check_eq(snapshot.current_process_id, current_ctx.pid);
    testrt::check_eq(snapshot.ready_len, 0usize);
});

arch_test!(program_scheduler_syscalls_dispatch_raw_yield_and_sleep, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();

    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "sched",
    )
    .expect("sched loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("sched"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid);

    let older_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "pwd",
    )
    .expect("pwd loads");
    let older = crate::exec::spawn_bin_program(older_program, argv1("pwd"));

    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let yield_ret = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::YIELD_NOW,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(yield_ret.decode(), Ok(1usize));
    testrt::check_eq(sink_str(), "/\n");

    let older_record = crate::proc::process(older.pid).expect("older process retained");
    testrt::check_eq(older_record.state, crate::proc::ProcessState::Exited);
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::YieldNow,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );

    let bad_sleep = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SLEEP,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(bad_sleep.decode(), Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT));
    let bad_tick = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SCHED_TICK,
        reovim_uapi_syscall::SyscallArgs::new([1, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(bad_tick.decode(), Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT));

    let sleep_ret = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SLEEP,
        reovim_uapi_syscall::SyscallArgs::new([1, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(sleep_ret.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    let current = crate::proc::process(current_ctx.pid).expect("current process retained");
    testrt::check_eq(current.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(current.block_reason, crate::sched::BlockReason::Sleep);
    let task = crate::sched::task(current_ctx.task_id).expect("current task retained");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Blocked);
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::Sleep);
    testrt::check_eq(task.wake_tick, 1usize);
    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, current_ctx.pid);
    testrt::check_eq(continuations[0].task_id, current_ctx.task_id);
    testrt::check_eq(continuations[0].program_path, "/bin/sched");
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::SLEEP);
    testrt::check_eq(continuations[0].op, crate::syscall::SyscallOp::ProcessSleep);
    testrt::check_eq(continuations[0].memory, crate::syscall::SyscallContinuationMemory::None);
    testrt::check_eq(continuations[0].args.a0, 1usize);
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Blocked,
    );
    drop(syscalls);

    {
        let mut inspector = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
        sink_clear();
        inspector
            .write_vfs_file_path("/dump/snapshot")
            .expect("dump snapshot renders retained sleep");
        let dump = sink_bytes();
        assert_contains(dump, b"syscall_continuation_records=1\n");
        assert_contains(dump, b"path=/bin/sched");
        assert_contains(dump, b"state=blocked");
        assert_contains(dump, b"block=sleep");
        assert_contains(dump, b"continuations:\n");
        assert_contains(dump, b"path=/bin/sched nr=11 op=process-sleep");
        assert_contains(dump, b" a0=1 ");
        sink_clear();
    }

    crate::proc::wake_process(current_ctx.pid).expect("test wakes sleeper before deadline");
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let early_replay =
        crate::syscall::resume_syscall_continuation(&daemon, &mut session, current_ctx)
            .expect("early-woken sleeper has retained sleep continuation");
    testrt::check_eq(early_replay.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    let current = crate::proc::process(current_ctx.pid).expect("current process retained");
    testrt::check_eq(current.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(current.block_reason, crate::sched::BlockReason::Sleep);
    let task = crate::sched::task(current_ctx.task_id).expect("current task retained");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Blocked);
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::Sleep);
    testrt::check_eq(task.wake_tick, 1usize);
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, current_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::SLEEP);

    let shell_ctx = crate::syscall::SyscallContext {
        pid: crate::proc::SHELL_PID,
        task_id: crate::proc::SHELL_PID,
        program_path: "/bin/sh",
        loader: "linked-bin",
        entry_name: "bin_sh",
    };
    let mut shell_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(shell_ctx));
    let tick_ret = shell_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SCHED_TICK,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(tick_ret.decode(), Ok(1usize));
    drop(shell_syscalls);

    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let replay_after_tick =
        crate::syscall::resume_syscall_continuation(&daemon, &mut session, current_ctx)
            .expect("due sleeper has retained sleep continuation");
    testrt::check_eq(replay_after_tick.decode(), Ok(1usize));
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    assert_user_resume_blocked(current_ctx);

    {
        let mut inspector = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
        sink_clear();
        inspector
            .write_vfs_file_path("/proc/processes")
            .expect("proc process table renders user-resume block");
        assert_contains(sink_bytes(), b"state=blocked path=/bin/sched");
        assert_contains(sink_bytes(), b"block=user-resume");

        sink_clear();
        inspector
            .write_vfs_file_path("/dump/snapshot")
            .expect("dump snapshot renders user-resume block");
        let dump = sink_bytes();
        assert_contains(dump, b"processes:\n");
        assert_contains(dump, b"state=blocked path=/bin/sched");
        assert_contains(dump, b"block=user-resume");
        assert_contains(dump, b"tasks:\n");
        assert_contains(dump, b"state=blocked entry=/bin/sched block=user-resume");
        sink_clear();
    }

    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let raw_admission_ops = [
        crate::syscall::SyscallOp::ProcessSelf,
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallOp::SchedulerTick,
    ];
    let mut raw_admission_ok_before = [0usize; 3];
    let mut raw_admission_error_before = [0usize; 3];
    let mut raw_index = 0usize;
    while raw_index < raw_admission_ops.len() {
        raw_admission_ok_before[raw_index] = syscall_record_count(
            "/bin/sched",
            raw_admission_ops[raw_index],
            crate::syscall::SyscallStatus::Ok,
        );
        raw_admission_error_before[raw_index] = syscall_record_count(
            "/bin/sched",
            raw_admission_ops[raw_index],
            crate::syscall::SyscallStatus::Error,
        );
        raw_index += 1;
    }
    let continue_errors_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    let stale_get_pid = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::GET_PID,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(stale_get_pid.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    assert_user_resume_blocked(current_ctx);

    let bad_sleep = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SLEEP,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(bad_sleep.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    assert_user_resume_blocked(current_ctx);
    let bad_tick = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SCHED_TICK,
        reovim_uapi_syscall::SyscallArgs::new([1, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(bad_tick.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    assert_user_resume_blocked(current_ctx);
    raw_index = 0;
    while raw_index < raw_admission_ops.len() {
        testrt::check_eq(
            syscall_record_count(
                "/bin/sched",
                raw_admission_ops[raw_index],
                crate::syscall::SyscallStatus::Ok,
            ),
            raw_admission_ok_before[raw_index],
        );
        testrt::check_eq(
            syscall_record_count(
                "/bin/sched",
                raw_admission_ops[raw_index],
                crate::syscall::SyscallStatus::Error,
            ),
            raw_admission_error_before[raw_index] + 1,
        );
        raw_index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before + raw_admission_ops.len(),
    );

    let stdio_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::StdioAttach,
        crate::syscall::SyscallStatus::Ok,
    );
    let stdio_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::StdioAttach,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_stdio = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    let stale_stdio = syscalls.stdio();
    testrt::check_eq(stale_stdio.stdout.fd, 1usize);
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::StdioAttach,
            crate::syscall::SyscallStatus::Ok,
        ),
        stdio_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::StdioAttach,
            crate::syscall::SyscallStatus::Error,
        ),
        stdio_error_before + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_stdio + 1,
    );

    let stdio_error_before_streams = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::StdioAttach,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_streams = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    let stale_stdin = syscalls.stdin();
    let stale_stdout = syscalls.stdout();
    let stale_stderr = syscalls.stderr();
    testrt::check_eq(stale_stdin.fd, 0usize);
    testrt::check_eq(stale_stdout.fd, 1usize);
    testrt::check_eq(stale_stderr.fd, 2usize);
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::StdioAttach,
            crate::syscall::SyscallStatus::Ok,
        ),
        stdio_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::StdioAttach,
            crate::syscall::SyscallStatus::Error,
        ),
        stdio_error_before_streams + 3,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_streams + 3,
    );

    let direct_errors_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    sink_clear();
    testrt::check_eq(
        syscalls.write_fd(1, b"stale-helper-leak\n").err(),
        Some(crate::syscall::ProgramIoError::Busy),
    );
    testrt::check_eq(sink_str(), "");
    assert_user_resume_blocked(current_ctx);

    let mut stale_read = [0x55u8; 4];
    testrt::check_eq(
        syscalls.read_fd(0, &mut stale_read).err(),
        Some(crate::syscall::ProgramIoError::Busy),
    );
    testrt::check_eq(stale_read, [0x55u8; 4]);
    assert_user_resume_blocked(current_ctx);

    testrt::check_eq(
        syscalls.open_vfs_file_path("/boot/profile").err(),
        Some(crate::vfs::VfsError::Busy),
    );
    assert_user_resume_blocked(current_ctx);

    let vfs_normalize_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::VfsNormalize,
        crate::syscall::SyscallStatus::Ok,
    );
    let vfs_normalize_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::VfsNormalize,
        crate::syscall::SyscallStatus::Error,
    );
    let vfs_lookup_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::VfsLookup,
        crate::syscall::SyscallStatus::Ok,
    );
    let vfs_lookup_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::VfsLookup,
        crate::syscall::SyscallStatus::Error,
    );
    let vfs_list_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::VfsList,
        crate::syscall::SyscallStatus::Ok,
    );
    let vfs_list_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::VfsList,
        crate::syscall::SyscallStatus::Error,
    );
    testrt::check_eq(syscalls.normalize_path("/boot").err(), Some(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.lookup_path("/").err(), Some(crate::vfs::VfsError::Busy));
    sink_clear();
    testrt::check_eq(syscalls.write_vfs_listing("/").err(), Some(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.write_vfs_listing("/bin").err(), Some(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.write_vfs_listing("/dev").err(), Some(crate::vfs::VfsError::Busy));
    testrt::check_eq(sink_str(), "");
    testrt::check_eq(syscalls.open_vfs_directory_path("/").err(), Some(crate::vfs::VfsError::Busy));
    testrt::check_eq(
        syscalls.open_vfs_directory_path("/bin").err(),
        Some(crate::vfs::VfsError::Busy),
    );
    testrt::check_eq(
        syscalls.open_vfs_directory_path("/dev").err(),
        Some(crate::vfs::VfsError::Busy),
    );
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::VfsNormalize,
            crate::syscall::SyscallStatus::Ok,
        ),
        vfs_normalize_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::VfsNormalize,
            crate::syscall::SyscallStatus::Error,
        ),
        vfs_normalize_error_before + 4,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::VfsLookup,
            crate::syscall::SyscallStatus::Ok,
        ),
        vfs_lookup_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::VfsLookup,
            crate::syscall::SyscallStatus::Error,
        ),
        vfs_lookup_error_before + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::VfsList,
            crate::syscall::SyscallStatus::Ok,
        ),
        vfs_list_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::VfsList,
            crate::syscall::SyscallStatus::Error,
        ),
        vfs_list_error_before + 3,
    );

    testrt::check_eq(syscalls.pipe_fds().err(), Some(crate::syscall::ProgramIoError::Busy));
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        direct_errors_before + 12,
    );

    let snapshot_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SnapshotProcesses,
        crate::syscall::SyscallStatus::Ok,
    );
    let snapshot_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SnapshotProcesses,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_snapshot = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    let mut stale_processes = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    testrt::check_eq(syscalls.snapshot_processes(&mut stale_processes), 0usize);
    testrt::check_eq(stale_processes[0].pid, 0usize);
    testrt::check_eq(stale_processes[0].program_path, "");
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SnapshotProcesses,
            crate::syscall::SyscallStatus::Ok,
        ),
        snapshot_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SnapshotProcesses,
            crate::syscall::SyscallStatus::Error,
        ),
        snapshot_error_before + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_snapshot + 1,
    );

    sink_clear();
    syscalls.write_process_table();
    testrt::check_eq(sink_str(), "");
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SnapshotProcesses,
            crate::syscall::SyscallStatus::Error,
        ),
        snapshot_error_before + 2,
    );

    let process_self_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::ProcessSelf,
        crate::syscall::SyscallStatus::Ok,
    );
    let process_self_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::ProcessSelf,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_self = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    testrt::check(
        syscalls.process_self().is_none(),
        "stale user-resume process_self should fail closed",
    );
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::ProcessSelf,
            crate::syscall::SyscallStatus::Ok,
        ),
        process_self_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::ProcessSelf,
            crate::syscall::SyscallStatus::Error,
        ),
        process_self_error_before + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_self + 1,
    );

    let process_self_error_before_writer = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::ProcessSelf,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_self_writer = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    sink_clear();
    syscalls.write_current_process();
    testrt::check_eq(sink_str(), "");
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::ProcessSelf,
            crate::syscall::SyscallStatus::Ok,
        ),
        process_self_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::ProcessSelf,
            crate::syscall::SyscallStatus::Error,
        ),
        process_self_error_before_writer + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_self_writer + 1,
    );

    let process_control_ops = [
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallOp::ProcessKill,
    ];
    let mut process_control_ok_before = [0usize; 3];
    let mut process_control_error_before = [0usize; 3];
    let mut process_control_index = 0usize;
    while process_control_index < process_control_ops.len() {
        process_control_ok_before[process_control_index] = syscall_record_count(
            "/bin/sched",
            process_control_ops[process_control_index],
            crate::syscall::SyscallStatus::Ok,
        );
        process_control_error_before[process_control_index] = syscall_record_count(
            "/bin/sched",
            process_control_ops[process_control_index],
            crate::syscall::SyscallStatus::Error,
        );
        process_control_index += 1;
    }
    let continue_errors_before_process_control = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    testrt::check_eq(
        syscalls.wait_process_by_pid(crate::proc::ROOTD_PID),
        Err(crate::syscall::ProgramProcessError::Busy),
    );
    testrt::check_eq(
        syscalls.wake_process_by_pid(crate::proc::ROOTD_PID),
        Err(crate::syscall::ProgramProcessError::Busy),
    );
    testrt::check_eq(
        syscalls.kill_process_by_pid(crate::proc::ROOTD_PID),
        Err(crate::syscall::ProgramProcessError::Busy),
    );
    assert_user_resume_blocked(current_ctx);
    process_control_index = 0;
    while process_control_index < process_control_ops.len() {
        testrt::check_eq(
            syscall_record_count(
                "/bin/sched",
                process_control_ops[process_control_index],
                crate::syscall::SyscallStatus::Ok,
            ),
            process_control_ok_before[process_control_index],
        );
        testrt::check_eq(
            syscall_record_count(
                "/bin/sched",
                process_control_ops[process_control_index],
                crate::syscall::SyscallStatus::Error,
            ),
            process_control_error_before[process_control_index] + 1,
        );
        process_control_index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_process_control + process_control_ops.len(),
    );

    let service_control_ops = [
        crate::syscall::SyscallOp::InitServiceStart,
        crate::syscall::SyscallOp::ServiceReady,
        crate::syscall::SyscallOp::ServiceHold,
        crate::syscall::SyscallOp::ServiceStop,
        crate::syscall::SyscallOp::ServiceStart,
        crate::syscall::SyscallOp::ServiceRestart,
    ];
    let mut service_control_ok_before = [0usize; 6];
    let mut service_control_error_before = [0usize; 6];
    let mut service_control_index = 0usize;
    while service_control_index < service_control_ops.len() {
        service_control_ok_before[service_control_index] = syscall_record_count(
            "/bin/sched",
            service_control_ops[service_control_index],
            crate::syscall::SyscallStatus::Ok,
        );
        service_control_error_before[service_control_index] = syscall_record_count(
            "/bin/sched",
            service_control_ops[service_control_index],
            crate::syscall::SyscallStatus::Error,
        );
        service_control_index += 1;
    }
    let continue_errors_before_service_control = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    let mut before_services = [crate::service::EMPTY_SERVICE_RECORD; crate::service::MAX_SERVICES];
    let before_service_count = crate::service::snapshot(&mut before_services);
    testrt::check_eq(
        syscalls.request_init_service("shell", "/bin/sh"),
        Err(crate::service::ServiceError::Busy),
    );
    testrt::check_eq(syscalls.service_ready("editor"), Err(crate::service::ServiceError::Busy));
    testrt::check_eq(syscalls.service_hold(), Err(crate::syscall::ProgramProcessError::Busy));
    testrt::check_eq(
        syscalls.stop_service_by_name("editor"),
        Err(crate::syscall::ProgramServiceError::Busy),
    );
    testrt::check_eq(
        syscalls.start_service_by_name("editor"),
        Err(crate::syscall::ProgramServiceError::Busy),
    );
    testrt::check_eq(
        syscalls.restart_service_by_name("editor"),
        Err(crate::syscall::ProgramServiceError::Busy),
    );
    assert_user_resume_blocked(current_ctx);
    let mut after_services = [crate::service::EMPTY_SERVICE_RECORD; crate::service::MAX_SERVICES];
    let after_service_count = crate::service::snapshot(&mut after_services);
    testrt::check_eq(after_service_count, before_service_count);
    service_control_index = 0;
    while service_control_index < service_control_ops.len() {
        testrt::check_eq(
            syscall_record_count(
                "/bin/sched",
                service_control_ops[service_control_index],
                crate::syscall::SyscallStatus::Ok,
            ),
            service_control_ok_before[service_control_index],
        );
        testrt::check_eq(
            syscall_record_count(
                "/bin/sched",
                service_control_ops[service_control_index],
                crate::syscall::SyscallStatus::Error,
            ),
            service_control_error_before[service_control_index] + 1,
        );
        service_control_index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_service_control + service_control_ops.len(),
    );

    let source_install_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Ok,
    );
    let source_install_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_source_install = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    install_source_media(SOURCE_MEDIA_PAYLOAD);
    let before_payload_source = fixture_source_store()
        .find_payload("/payload/server-smoke")
        .expect("server-smoke payload source exists");
    testrt::check_eq(before_payload_source.bytes, SAMPLE_PAYLOAD_FAILED_SOURCE_BYTES);
    testrt::check_eq(
        syscalls.install_bin_source_by_name("proc", crate::program::BinSourceInstallStatus::Ok),
        Err(crate::syscall::ProgramSourceInstallError::Busy),
    );
    testrt::check_eq(
        syscalls.install_payload_source_by_name(
            "server-smoke",
            crate::rootd::PayloadSourceInstallStatus::Ready,
        ),
        Err(crate::syscall::ProgramSourceInstallError::Busy),
    );
    testrt::check_eq(
        syscalls.install_bin_source_from_media_by_name("proc"),
        Err(crate::syscall::ProgramSourceInstallError::Busy),
    );
    testrt::check_eq(
        syscalls.install_payload_source_from_media_by_name("server-smoke"),
        Err(crate::syscall::ProgramSourceInstallError::Busy),
    );
    assert_user_resume_blocked(current_ctx);
    let after_payload_source = fixture_source_store()
        .find_payload("/payload/server-smoke")
        .expect("server-smoke payload source remains visible");
    testrt::check_eq(after_payload_source.bytes, SAMPLE_PAYLOAD_FAILED_SOURCE_BYTES);
    clear_source_media();
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SourceInstall,
            crate::syscall::SyscallStatus::Ok,
        ),
        source_install_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SourceInstall,
            crate::syscall::SyscallStatus::Error,
        ),
        source_install_error_before + 4,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_source_install + 4,
    );

    let provider_probe_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::ProviderProbe,
        crate::syscall::SyscallStatus::Ok,
    );
    let provider_probe_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::ProviderProbe,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_probe_catalog = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    sink_clear();
    syscalls.write_probe_catalog();
    testrt::check_eq(sink_str(), "");
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::ProviderProbe,
            crate::syscall::SyscallStatus::Ok,
        ),
        provider_probe_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::ProviderProbe,
            crate::syscall::SyscallStatus::Error,
        ),
        provider_probe_error_before + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_probe_catalog + 1,
    );

    let cwd_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SessionCwdGet,
        crate::syscall::SyscallStatus::Ok,
    );
    let cwd_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SessionCwdGet,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_cwd = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    testrt::check_eq(syscalls.cwd(), "");
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SessionCwdGet,
            crate::syscall::SyscallStatus::Ok,
        ),
        cwd_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SessionCwdGet,
            crate::syscall::SyscallStatus::Error,
        ),
        cwd_error_before + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_cwd + 1,
    );

    let tty_read_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::TtyReadLine,
        crate::syscall::SyscallStatus::Ok,
    );
    let tty_read_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::TtyReadLine,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_tty_read = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    TTY_READ_CALLS.store(0, Ordering::Relaxed);
    let mut tty_line = [0x33u8; crate::rootd::ROOT_LINE_BYTES];
    testrt::check_eq(syscalls.read_tty_line(&mut tty_line), 0usize);
    testrt::check_eq(TTY_READ_CALLS.load(Ordering::Relaxed), 0usize);
    testrt::check_eq(tty_line[0], 0x33u8);
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::TtyReadLine,
            crate::syscall::SyscallStatus::Ok,
        ),
        tty_read_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::TtyReadLine,
            crate::syscall::SyscallStatus::Error,
        ),
        tty_read_error_before + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_tty_read + 1,
    );

    let session_mutation_ops = [
        crate::syscall::SyscallOp::SessionCwdSet,
        crate::syscall::SyscallOp::SessionShellTarget,
        crate::syscall::SyscallOp::SessionShellStart,
        crate::syscall::SyscallOp::SessionLineDiscipline,
    ];
    let mut session_mutation_ok_before = [0usize; 4];
    let mut session_mutation_error_before = [0usize; 4];
    let mut session_mutation_index = 0usize;
    while session_mutation_index < session_mutation_ops.len() {
        session_mutation_ok_before[session_mutation_index] = syscall_record_count(
            "/bin/sched",
            session_mutation_ops[session_mutation_index],
            crate::syscall::SyscallStatus::Ok,
        );
        session_mutation_error_before[session_mutation_index] = syscall_record_count(
            "/bin/sched",
            session_mutation_ops[session_mutation_index],
            crate::syscall::SyscallStatus::Error,
        );
        session_mutation_index += 1;
    }
    let continue_errors_before_session_mutation = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    let stale_cwd = crate::vfs::normalize("/", "/bin").expect("test cwd path normalizes");
    syscalls.set_cwd(stale_cwd);
    syscalls.request_shell_target("/bin/sh");
    syscalls.request_shell_start();
    syscalls.request_shell_line_discipline("argv-v1", "single-pipe");
    assert_user_resume_blocked(current_ctx);
    session_mutation_index = 0;
    while session_mutation_index < session_mutation_ops.len() {
        testrt::check_eq(
            syscall_record_count(
                "/bin/sched",
                session_mutation_ops[session_mutation_index],
                crate::syscall::SyscallStatus::Ok,
            ),
            session_mutation_ok_before[session_mutation_index],
        );
        testrt::check_eq(
            syscall_record_count(
                "/bin/sched",
                session_mutation_ops[session_mutation_index],
                crate::syscall::SyscallStatus::Error,
            ),
            session_mutation_error_before[session_mutation_index] + 1,
        );
        session_mutation_index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_session_mutation + session_mutation_ops.len(),
    );

    let session_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SnapshotSession,
        crate::syscall::SyscallStatus::Ok,
    );
    let session_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SnapshotSession,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_session = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    sink_clear();
    testrt::check_eq(syscalls.prompt(), "blocked");
    syscalls.write_session_state();
    testrt::check_eq(sink_str(), "");
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SnapshotSession,
            crate::syscall::SyscallStatus::Ok,
        ),
        session_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SnapshotSession,
            crate::syscall::SyscallStatus::Error,
        ),
        session_error_before + 2,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_session + 2,
    );

    let boot_info_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::BootInfo,
        crate::syscall::SyscallStatus::Ok,
    );
    let boot_info_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::BootInfo,
        crate::syscall::SyscallStatus::Error,
    );
    let device_catalog_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::DeviceCatalog,
        crate::syscall::SyscallStatus::Ok,
    );
    let device_catalog_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::DeviceCatalog,
        crate::syscall::SyscallStatus::Error,
    );
    let boot_profile_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::BootProfile,
        crate::syscall::SyscallStatus::Ok,
    );
    let boot_profile_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::BootProfile,
        crate::syscall::SyscallStatus::Error,
    );
    let boot_image_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::BootImage,
        crate::syscall::SyscallStatus::Ok,
    );
    let boot_image_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::BootImage,
        crate::syscall::SyscallStatus::Error,
    );
    let console_input_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::ConsoleInput,
        crate::syscall::SyscallStatus::Ok,
    );
    let console_input_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::ConsoleInput,
        crate::syscall::SyscallStatus::Error,
    );
    let payload_catalog_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::PayloadCatalog,
        crate::syscall::SyscallStatus::Ok,
    );
    let payload_catalog_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::PayloadCatalog,
        crate::syscall::SyscallStatus::Error,
    );
    let mounts_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::VfsMounts,
        crate::syscall::SyscallStatus::Ok,
    );
    let mounts_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::VfsMounts,
        crate::syscall::SyscallStatus::Error,
    );
    let help_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::ProgramHelp,
        crate::syscall::SyscallStatus::Ok,
    );
    let help_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::ProgramHelp,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_boot_catalog = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    sink_clear();
    assert_blocked_boot_info(syscalls.boot_info());
    testrt::check_eq(syscalls.devices().len(), 0usize);
    testrt::check_eq(syscalls.profile_name(), "blocked");
    testrt::check_eq(syscalls.launch_enabled(), false);
    assert_blocked_boot_image(syscalls.boot_image());
    assert_blocked_console_input(syscalls.console_input());
    testrt::check_eq(syscalls.payloads().len(), 0usize);
    testrt::check_eq(syscalls.programs().len(), 0usize);
    syscalls.write_boot_info_summary();
    syscalls.write_device_inventory();
    syscalls.write_boot_profile();
    syscalls.write_boot_image();
    syscalls.write_boot_payloads();
    syscalls.write_boot_memory();
    syscalls.write_boot_devices();
    syscalls.write_boot_status();
    syscalls.write_boot_proof();
    syscalls.write_boot_input();
    syscalls.write_mount_table();
    testrt::check_eq(syscalls.write_program_help(None), crate::program::ProgramStatus::Blocked);
    crate::bin_fixture::write_vfs_file(crate::vfs::File::BootHelp, &mut syscalls);
    crate::bin_fixture::write_vfs_file(crate::vfs::File::BinProgram(0), &mut syscalls);
    crate::bin_fixture::write_vfs_file(crate::vfs::File::DevDevice(0), &mut syscalls);
    testrt::check_eq(sink_str(), "");
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::BootInfo,
            crate::syscall::SyscallStatus::Ok,
        ),
        boot_info_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::BootInfo,
            crate::syscall::SyscallStatus::Error,
        ),
        boot_info_error_before + 3,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::DeviceCatalog,
            crate::syscall::SyscallStatus::Ok,
        ),
        device_catalog_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::DeviceCatalog,
            crate::syscall::SyscallStatus::Error,
        ),
        device_catalog_error_before + 4,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::BootProfile,
            crate::syscall::SyscallStatus::Ok,
        ),
        boot_profile_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::BootProfile,
            crate::syscall::SyscallStatus::Error,
        ),
        boot_profile_error_before + 3,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::BootImage,
            crate::syscall::SyscallStatus::Ok,
        ),
        boot_image_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::BootImage,
            crate::syscall::SyscallStatus::Error,
        ),
        boot_image_error_before + 4,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::ConsoleInput,
            crate::syscall::SyscallStatus::Ok,
        ),
        console_input_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::ConsoleInput,
            crate::syscall::SyscallStatus::Error,
        ),
        console_input_error_before + 2,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::PayloadCatalog,
            crate::syscall::SyscallStatus::Ok,
        ),
        payload_catalog_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::PayloadCatalog,
            crate::syscall::SyscallStatus::Error,
        ),
        payload_catalog_error_before + 2,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::VfsMounts,
            crate::syscall::SyscallStatus::Ok,
        ),
        mounts_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::VfsMounts,
            crate::syscall::SyscallStatus::Error,
        ),
        mounts_error_before + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::ProgramHelp,
            crate::syscall::SyscallStatus::Ok,
        ),
        help_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::ProgramHelp,
            crate::syscall::SyscallStatus::Error,
        ),
        help_error_before + 4,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_boot_catalog + 23,
    );

    let source_media_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SourceMediaSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );
    let source_media_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SourceMediaSnapshot,
        crate::syscall::SyscallStatus::Error,
    );
    let source_media_read_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SourceMediaRead,
        crate::syscall::SyscallStatus::Ok,
    );
    let source_media_read_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SourceMediaRead,
        crate::syscall::SyscallStatus::Error,
    );
    let exec_bundle_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::ExecBundleSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );
    let exec_bundle_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::ExecBundleSnapshot,
        crate::syscall::SyscallStatus::Error,
    );
    let exec_bundle_read_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::ExecBundleRead,
        crate::syscall::SyscallStatus::Ok,
    );
    let exec_bundle_read_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::ExecBundleRead,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_media = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    sink_clear();
    syscalls.write_source_media_table();
    testrt::check_eq(sink_str(), "");
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SourceMediaSnapshot,
            crate::syscall::SyscallStatus::Ok,
        ),
        source_media_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SourceMediaSnapshot,
            crate::syscall::SyscallStatus::Error,
        ),
        source_media_error_before + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SourceMediaRead,
            crate::syscall::SyscallStatus::Ok,
        ),
        source_media_read_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SourceMediaRead,
            crate::syscall::SyscallStatus::Error,
        ),
        source_media_read_error_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::ExecBundleSnapshot,
            crate::syscall::SyscallStatus::Ok,
        ),
        exec_bundle_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::ExecBundleSnapshot,
            crate::syscall::SyscallStatus::Error,
        ),
        exec_bundle_error_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::ExecBundleRead,
            crate::syscall::SyscallStatus::Ok,
        ),
        exec_bundle_read_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::ExecBundleRead,
            crate::syscall::SyscallStatus::Error,
        ),
        exec_bundle_read_error_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_media + 1,
    );

    let scheduler_snapshot_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SchedulerSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );
    let scheduler_snapshot_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SchedulerSnapshot,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_scheduler_snapshot = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    sink_clear();
    assert_blocked_scheduler_snapshot(syscalls.scheduler_snapshot());
    syscalls.write_scheduler_state();
    testrt::check_eq(sink_str(), "");
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SchedulerSnapshot,
            crate::syscall::SyscallStatus::Ok,
        ),
        scheduler_snapshot_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SchedulerSnapshot,
            crate::syscall::SyscallStatus::Error,
        ),
        scheduler_snapshot_error_before + 2,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_scheduler_snapshot + 2,
    );

    let scheduler_tick_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallStatus::Ok,
    );
    let scheduler_tick_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_scheduler_tick = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    sink_clear();
    syscalls.write_scheduler_tick();
    testrt::check_eq(sink_str(), "");
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SchedulerTick,
            crate::syscall::SyscallStatus::Ok,
        ),
        scheduler_tick_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SchedulerTick,
            crate::syscall::SyscallStatus::Error,
        ),
        scheduler_tick_error_before + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_scheduler_tick + 1,
    );

    let table_writer_ops = [
        crate::syscall::SyscallOp::SnapshotServices,
        crate::syscall::SyscallOp::SnapshotExecLoads,
        crate::syscall::SyscallOp::SnapshotAddressSpaces,
        crate::syscall::SyscallOp::SnapshotAddressSpacePageTables,
        crate::syscall::SyscallOp::SnapshotAddressSpacePages,
        crate::syscall::SyscallOp::SnapshotAddressSpaceObjects,
        crate::syscall::SyscallOp::SnapshotPendingExecs,
        crate::syscall::SyscallOp::SnapshotSourceStore,
        crate::syscall::SyscallOp::SnapshotTasks,
        crate::syscall::SyscallOp::SnapshotWaits,
        crate::syscall::SyscallOp::SnapshotSyscalls,
        crate::syscall::SyscallOp::SnapshotContinuations,
    ];
    let mut table_writer_ok_before = [0usize; 12];
    let mut table_writer_error_before = [0usize; 12];
    let mut table_index = 0usize;
    while table_index < table_writer_ops.len() {
        table_writer_ok_before[table_index] = syscall_record_count(
            "/bin/sched",
            table_writer_ops[table_index],
            crate::syscall::SyscallStatus::Ok,
        );
        table_writer_error_before[table_index] = syscall_record_count(
            "/bin/sched",
            table_writer_ops[table_index],
            crate::syscall::SyscallStatus::Error,
        );
        table_index += 1;
    }
    let continue_errors_before_table_writers = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    sink_clear();
    syscalls.write_service_table();
    syscalls.write_exec_load_table();
    syscalls.write_address_space_table();
    syscalls.write_address_space_page_table_table();
    syscalls.write_address_space_page_table_entry_table();
    syscalls.write_address_space_object_table();
    syscalls.write_pending_exec_table();
    syscalls.write_source_store_table();
    syscalls.write_task_table();
    syscalls.write_wait_table();
    syscalls.write_syscall_table();
    syscalls.write_syscall_continuation_table();
    testrt::check_eq(sink_str(), "");
    assert_user_resume_blocked(current_ctx);
    table_index = 0;
    while table_index < table_writer_ops.len() {
        testrt::check_eq(
            syscall_record_count(
                "/bin/sched",
                table_writer_ops[table_index],
                crate::syscall::SyscallStatus::Ok,
            ),
            table_writer_ok_before[table_index],
        );
        testrt::check_eq(
            syscall_record_count(
                "/bin/sched",
                table_writer_ops[table_index],
                crate::syscall::SyscallStatus::Error,
            ),
            table_writer_error_before[table_index] + 1,
        );
        table_index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_table_writers + table_writer_ops.len(),
    );

    let log_read_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::KernelLogRead,
        crate::syscall::SyscallStatus::Ok,
    );
    let log_read_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::KernelLogRead,
        crate::syscall::SyscallStatus::Error,
    );
    let log_stats_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::KernelLogStats,
        crate::syscall::SyscallStatus::Ok,
    );
    let log_stats_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::KernelLogStats,
        crate::syscall::SyscallStatus::Error,
    );
    let dump_status_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::DumpStatus,
        crate::syscall::SyscallStatus::Ok,
    );
    let dump_status_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::DumpStatus,
        crate::syscall::SyscallStatus::Error,
    );
    let dump_sync_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::DumpSync,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_diagnostics = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    sink_clear();
    testrt::check_eq(syscalls.write_kernel_log(), false);
    syscalls.write_kernel_log_view();
    testrt::check_eq(syscalls.external_dmesg(), None);
    let stale_stats = syscalls.kernel_log_stats();
    assert_blocked_kernel_log_stats(stale_stats);
    syscalls.write_kernel_log_stats();
    let stale_dump_status = syscalls.dump_status();
    assert_blocked_dump_status(stale_dump_status);
    let stale_dump_sync = syscalls.dump_sync();
    assert_blocked_dump_sync_status(stale_dump_sync);
    syscalls.write_dump_status();
    syscalls.write_dump_snapshot();
    syscalls.write_dump_sync();
    testrt::check_eq(sink_str(), "");
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::KernelLogRead,
            crate::syscall::SyscallStatus::Ok,
        ),
        log_read_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::KernelLogRead,
            crate::syscall::SyscallStatus::Error,
        ),
        log_read_error_before + 3,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::KernelLogStats,
            crate::syscall::SyscallStatus::Ok,
        ),
        log_stats_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::KernelLogStats,
            crate::syscall::SyscallStatus::Error,
        ),
        log_stats_error_before + 2,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::DumpStatus,
            crate::syscall::SyscallStatus::Ok,
        ),
        dump_status_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::DumpStatus,
            crate::syscall::SyscallStatus::Error,
        ),
        dump_status_error_before + 3,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::DumpSync,
            crate::syscall::SyscallStatus::Error,
        ),
        dump_sync_error_before + 2,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_diagnostics + 10,
    );

    let kernel_events_ok_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SnapshotKernelEvents,
        crate::syscall::SyscallStatus::Ok,
    );
    let kernel_events_error_before = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SnapshotKernelEvents,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_kernel_events = syscall_record_count(
        "/bin/sched",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    let mut stale_events = [crate::klog::EMPTY_EVENT_RECORD; crate::klog::MAX_EVENTS];
    testrt::check_eq(syscalls.snapshot_kernel_events(&mut stale_events), 0usize);
    sink_clear();
    syscalls.write_kernel_event_table();
    testrt::check_eq(sink_str(), "");
    assert_user_resume_blocked(current_ctx);
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SnapshotKernelEvents,
            crate::syscall::SyscallStatus::Ok,
        ),
        kernel_events_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SnapshotKernelEvents,
            crate::syscall::SyscallStatus::Error,
        ),
        kernel_events_error_before + 2,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/sched",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_kernel_events + 2,
    );
    drop(syscalls);

    let mut no_current = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
    let unavailable = no_current.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::YIELD_NOW,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(
        unavailable.decode(),
        Err(reovim_uapi_syscall::SyscallError::NO_CURRENT_PROCESS),
    );
    let unavailable_tick = no_current.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SCHED_TICK,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(
        unavailable_tick.decode(),
        Err(reovim_uapi_syscall::SyscallError::NO_CURRENT_PROCESS),
    );
});

arch_test!(root_shell_proc_program_reports_process_state, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"ps\n", None);
    assert_contains(sink_bytes(), b"processes:\n");
    assert_contains(sink_bytes(), b"state=running path=/bin/ps");
    assert_contains(sink_bytes(), b"loader=linked-bin entry_fn=bin_ps");
    assert_syscall_record(
        "/bin/ps",
        crate::syscall::SyscallOp::SnapshotProcesses,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc\n", None);
    assert_contains(sink_bytes(), b"processes:\n");
    assert_contains(sink_bytes(), b"state=running path=/bin/proc");
    assert_contains(sink_bytes(), b"loader=linked-bin entry_fn=bin_proc");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SnapshotProcesses,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc self\n", None);
    assert_contains(sink_bytes(), b"self:\n");
    assert_contains(sink_bytes(), b"path=/bin/proc\nstate=running");
    assert_contains(sink_bytes(), b"loader=linked-bin\nentry_fn=bin_proc\n");
    assert_contains(sink_bytes(), b"artifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n");
    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::ProcessSelf,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"self\n", None);
    assert_contains(sink_bytes(), b"self:\n");
    assert_contains(sink_bytes(), b"path=/bin/self\nstate=running");
    assert_contains(sink_bytes(), b"loader=linked-bin\nentry_fn=bin_self\n");
    assert_contains(sink_bytes(), b"artifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n");
    assert_no_syscall_record(
        "/bin/self",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/self",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/self",
        crate::syscall::SyscallOp::ProcessSelf,
        crate::syscall::SyscallStatus::Ok,
    );

    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::proc::install_shell_session("/bin/sh", "linked-bin", "bin_sh");
    let mut session = RootShellSession::new();
    session.request_shell_target("/bin/sh");
    session.request_shell_start();
    session.request_line_discipline("argv-v1", "single-pipe");
    session.install_shell_owner("/bin/sh", "linked-bin", "bin_sh");
    run_session_shell_line(&mut session, b"proc session\n");
    assert_contains(sink_bytes(), b"session:\n");
    assert_contains(sink_bytes(), b"owner_path=/bin/sh\nowner_loader=linked-bin\n");
    assert_contains(sink_bytes(), b"owner_entry_fn=bin_sh\nowner_pid=2\nowner_task=2\n");
    assert_contains(sink_bytes(), b"cwd=/\nshell_target=/bin/sh\nshell_started=true\n");
    assert_contains(sink_bytes(), b"line_discipline=argv-v1\npipe_mode=single-pipe\n");
    assert_contains(sink_bytes(), b"line_loop_host=rootd\nprompt=reovim-os> \n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SnapshotSession,
        crate::syscall::SyscallStatus::Ok,
    );

    run_session_shell_line(&mut session, b"cat /proc/session\n");
    assert_contains(sink_bytes(), b"session:\n");
    assert_contains(sink_bytes(), b"owner_path=/bin/sh\nowner_loader=linked-bin\n");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SnapshotSession,
        crate::syscall::SyscallStatus::Ok,
    );

    run_session_shell_line(&mut session, b"session\n");
    assert_contains(sink_bytes(), b"session:\n");
    assert_contains(sink_bytes(), b"owner_path=/bin/sh\nowner_loader=linked-bin\n");
    assert_contains(sink_bytes(), b"line_loop_host=rootd\nprompt=reovim-os> \n");
    assert_syscall_record(
        "/bin/session",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/session",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/session",
        crate::syscall::SyscallOp::SnapshotSession,
        crate::syscall::SyscallStatus::Ok,
    );

    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::service::request_service(3, 3, "shell", "/bin/sh").expect("shell service records");
    crate::service::mark_started("shell", 2, 2);
    run_shell_line_preserving_log(
        ProfileSummary::new("shell-only", false),
        b"proc services\n",
        None,
    );
    assert_contains(sink_bytes(), b"services:\n");
    assert_contains(
        sink_bytes(),
        b"name=shell target=/bin/sh state=started reason=running owner_pid=3 owner_task=3 service_pid=2 service_task=2",
    );
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SnapshotServices,
        crate::syscall::SyscallStatus::Ok,
    );

    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::service::request_service(3, 3, "shell", "/bin/sh").expect("shell service records");
    crate::service::mark_started("shell", 2, 2);
    run_shell_line_preserving_log(
        ProfileSummary::new("shell-only", false),
        b"cat /proc/services\n",
        None,
    );
    assert_contains(sink_bytes(), b"services:\n");
    assert_contains(
        sink_bytes(),
        b"name=shell target=/bin/sh state=started reason=running owner_pid=3 owner_task=3 service_pid=2 service_task=2",
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SnapshotServices,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_preserving_log(ProfileSummary::new("shell-only", false), b"services\n", None);
    assert_contains(sink_bytes(), b"services:\n");
    assert_contains(
        sink_bytes(),
        b"name=shell target=/bin/sh state=started reason=running owner_pid=3 owner_task=3 service_pid=2 service_task=2",
    );
    assert_syscall_record(
        "/bin/services",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/services",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/services",
        crate::syscall::SyscallOp::SnapshotServices,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc tasks\n", None);
    assert_contains(sink_bytes(), b"tasks:\n");
    assert_contains(sink_bytes(), b"state=running entry=/bin/proc");
    assert_contains(sink_bytes(), b" runs=");
    assert_contains(sink_bytes(), b" ticks=");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SnapshotTasks,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"tasks\n", None);
    assert_contains(sink_bytes(), b"tasks:\n");
    assert_contains(sink_bytes(), b"state=running entry=/bin/tasks");
    assert_contains(sink_bytes(), b" runs=");
    assert_contains(sink_bytes(), b" ticks=");
    assert_syscall_record(
        "/bin/tasks",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/tasks",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/tasks",
        crate::syscall::SyscallOp::SnapshotTasks,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc waits\n", None);
    assert_contains(sink_bytes(), b"waits:\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SnapshotWaits,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"waits\n", None);
    assert_contains(sink_bytes(), b"waits:\n");
    assert_syscall_record(
        "/bin/waits",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/waits",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/waits",
        crate::syscall::SyscallOp::SnapshotWaits,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc syscalls\n", None);
    assert_contains(sink_bytes(), b"syscalls:\n");
    assert_contains(
        sink_bytes(),
        b"path=/bin/proc op=snapshot-syscalls status=ok loader=linked-bin entry_fn=bin_proc\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"syscalls\n", None);
    assert_contains(sink_bytes(), b"syscalls:\n");
    assert_syscall_record(
        "/bin/syscalls",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/syscalls",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/syscalls",
        crate::syscall::SyscallOp::SnapshotSyscalls,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc execs\n", None);
    assert_contains(sink_bytes(), b"execs:\n");
    assert_contains(sink_bytes(), b"argv0=proc status=ok reason=loaded path=/bin/proc");
    assert_contains(sink_bytes(), b"path=/bin/proc source=/bin/proc");
    assert_contains(sink_bytes(), b"loader=linked-bin entry_fn=bin_proc");
    assert_contains(sink_bytes(), b"truncated=false kind=bin");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SnapshotExecLoads,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"execs\n", None);
    assert_contains(sink_bytes(), b"execs:\n");
    assert_contains(sink_bytes(), b"argv0=execs status=ok reason=loaded path=/bin/execs");
    assert_contains(sink_bytes(), b"path=/bin/execs source=/bin/execs");
    assert_contains(sink_bytes(), b"loader=linked-bin entry_fn=bin_execs");
    assert_syscall_record(
        "/bin/execs",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/execs",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/execs",
        crate::syscall::SyscallOp::SnapshotExecLoads,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc pending\n", None);
    assert_contains(sink_bytes(), b"pending:\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SnapshotPendingExecs,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"pending\n", None);
    assert_contains(sink_bytes(), b"pending:\n");
    assert_syscall_record(
        "/bin/pending",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pending",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pending",
        crate::syscall::SyscallOp::SnapshotPendingExecs,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc sources\n", None);
    assert_contains(sink_bytes(), b"sources:\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SnapshotSourceStore,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"sources\n", None);
    assert_contains(sink_bytes(), b"sources:\n");
    assert_contains(
        sink_bytes(),
        b"namespace=payload path=/payload/reovim loader=source-image bytes=",
    );
    assert_syscall_record(
        "/bin/sources",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/sources",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/sources",
        crate::syscall::SyscallOp::SnapshotSourceStore,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc media\n", None);
    assert_contains(sink_bytes(), b"source-media:\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SourceMediaSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"media\n", None);
    assert_contains(sink_bytes(), b"source-media:\n");
    assert_syscall_record(
        "/bin/media",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/media",
        crate::syscall::SyscallOp::SourceMediaSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc scheduler\n", None);
    assert_contains(sink_bytes(), b"scheduler:\n");
    assert_contains(sink_bytes(), b"tick_count=");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SchedulerSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"exec pwd\n", None);
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record_with_message(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Ok,
        "exec pwd records replacement exec-load",
    );
    assert_syscall_record_with_message(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
        "exec pwd records exec-replace",
    );
    assert_syscall_record_with_message(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
        "exec pwd records replacement process-exit",
    );
    assert_no_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/exec",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    testrt::check_eq(crate::proc::snapshot_waits(&mut waits), 0usize);
    let process = crate::proc::process(3).expect("replaced process remains retained");
    testrt::check_eq(process.program_path, "/bin/pwd");
    testrt::check_eq(process.task_id, 3usize);
    testrt::check_eq(process.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(process.argv0(), "pwd");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_pwd\n",
    );

    run_shell_line_fixture(
        ProfileSummary::new("shell-only", false),
        b"exec REOVIM_MEDIA_PATH=/boot/status pwd\n",
        None,
    );
    testrt::check_eq(sink_str(), "/\n");
    let process = crate::proc::process(3).expect("env replacement process remains retained");
    testrt::check_eq(process.program_path, "/bin/pwd");
    testrt::check_eq(process.task_id, 3usize);
    testrt::check_eq(process.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(process.argv0(), "pwd");
    testrt::check_eq(process.envc, 1usize);
    testrt::check_eq(process.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(process.env_value(0), "/boot/status");
    testrt::check_eq(process.env_was_truncated(0), false);
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );
    {
        sink_clear();
        let daemon = daemon(ProfileSummary::new("shell-only", false), None);
        let mut session = RootShellSession::new();
        let mut inspector = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
        inspector
            .write_vfs_file_path("/dump/snapshot")
            .expect("dump snapshot renders env replacement");
        assert_contains(sink_bytes(), b"processes:\n");
        assert_contains(sink_bytes(), b"path=/bin/pwd");
        assert_contains(
            sink_bytes(),
            b"envc=1 env0_name=REOVIM_MEDIA_PATH env0_value=/boot/status env0_truncated=false",
        );
        sink_clear();
    }

    run_shell_line_fixture(
        ProfileSummary::new("shell-only", false),
        b"exec cat /boot/profile\n",
        None,
    );
    assert_contains(sink_bytes(), b"profile=shell-only\n");
    assert_contains(sink_bytes(), b"launch=disabled\n");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::VfsLookup,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(
        ProfileSummary::new("shell-only", false),
        b"exec cat /boot/profile /boot/status\n",
        None,
    );
    assert_contains(sink_bytes(), b"profile=shell-only\n");
    assert_contains(sink_bytes(), b"input=fixture-input\n");
    let replaced = crate::proc::process(3).expect("cat replacement process remains retained");
    testrt::check_eq(replaced.program_path, "/bin/cat");
    testrt::check_eq(replaced.task_id, 3usize);
    testrt::check_eq(replaced.argc, 3usize);
    testrt::check_eq(replaced.argv0(), "cat");
    testrt::check_eq(replaced.argv_was_truncated(0), false);
    testrt::check_eq(replaced.argv1(), "/boot/profile");
    testrt::check_eq(replaced.argv_was_truncated(1), false);
    testrt::check_eq(replaced.argv(2), "/boot/status");
    testrt::check_eq(replaced.argv_was_truncated(2), false);
    run_shell_line_preserving_log(
        ProfileSummary::new("shell-only", false),
        b"proc processes\n",
        None,
    );
    assert_contains(
        sink_bytes(),
        b"argc=3 argv0=cat argv0_truncated=false argv1=/boot/profile argv1_truncated=false argv2=/boot/status argv2_truncated=false",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc unknown\n", None);
    testrt::check_eq(sink_str(), "proc: unknown subcommand\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"exec missing\n", None);
    testrt::check_eq(sink_str(), "exec: program-not-found\n");
    assert_syscall_record(
        "/bin/exec",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Error,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"exec\n", None);
    testrt::check_eq(sink_str(), "exec: missing program\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"exec pwd extra\n", None);
    testrt::check_eq(sink_str(), "pwd: too many arguments\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc exec\n", None);
    testrt::check_eq(sink_str(), "proc: unknown subcommand\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc exec missing\n", None);
    testrt::check_eq(sink_str(), "proc: too many arguments\n");
    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Error,
    );

    run_shell_line_fixture(
        ProfileSummary::new("shell-only", false),
        b"proc exec pwd extra\n",
        None,
    );
    testrt::check_eq(sink_str(), "proc: too many arguments\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc spawn\n", None);
    testrt::check_eq(sink_str(), "proc: unknown subcommand\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc spawn missing\n", None);
    testrt::check_eq(sink_str(), "proc: too many arguments\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc kill 4\n", None);
    testrt::check_eq(sink_str(), "proc: too many arguments\n");

    run_shell_line_fixture(
        ProfileSummary::new("shell-only", false),
        b"proc processes extra\n",
        None,
    );
    testrt::check_eq(sink_str(), "proc: too many arguments\n");
});

arch_test!(root_shell_proc_process_control_compatibility_paths_are_retired, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc spawn pwd\n", None);
    testrt::check_eq(sink_str(), "proc: too many arguments\n");
    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc block pwd\n", None);
    testrt::check_eq(sink_str(), "proc: too many arguments\n");
    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc sleep 2 pwd\n", None);
    testrt::check_eq(sink_str(), "proc: too many arguments\n");
    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc wait 4\n", None);
    testrt::check_eq(sink_str(), "proc: too many arguments\n");
    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(
        ProfileSummary::new("shell-only", false),
        b"proc wait-ticks 2 4\n",
        None,
    );
    testrt::check_eq(sink_str(), "proc: too many arguments\n");
    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc wake 4\n", None);
    testrt::check_eq(sink_str(), "proc: too many arguments\n");
    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc kill 4\n", None);
    testrt::check_eq(sink_str(), "proc: too many arguments\n");
    assert_no_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::ProcessKill,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(linked_spawn_leaves_child_ready_for_later_scheduler_dispatch, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"spawn pwd\n");
    testrt::check_eq(
        sink_str(),
        "spawn:\npid=4\npath=/bin/pwd\nstate=ready\nloader=linked-bin\nentry_fn=bin_pwd\nartifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n",
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessAdopt,
        crate::syscall::SyscallStatus::Ok,
    );

    let child = crate::proc::process(4).expect("linked spawned child process exists");
    testrt::check_eq(child.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(child.program_path, "/bin/pwd");
    testrt::check_eq(child.parent_pid, crate::proc::ROOTD_PID);
    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.ready_len, 1usize);
    testrt::check_eq(snapshot.next_ready_process_id(), child.pid);

    run_session_shell_line(&mut session, b"pwd\n");
    testrt::check_eq(sink_str(), "/\n/\n");
    let child = crate::proc::process(4).expect("linked spawned child remains retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Exited);
    let later = crate::proc::process(5).expect("later shell pwd process exists");
    testrt::check_eq(later.state, crate::proc::ProcessState::Exited);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/spawn pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_spawn\n",
    );
    assert_contains(
        sink_bytes(),
        b"process.adopt old_parent_pid=3 new_parent_pid=1 child_pid=4 child_task=4 path=/bin/pwd\n",
    );
    assert_contains(
        sink_bytes(),
        b"component=proc severity=info kind=process-adopt boot=1 session=1 source=process pid=4 task=4\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=4 task=4 status=ok loader=linked-bin entry_fn=bin_pwd\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=5 task=5 status=ok loader=linked-bin entry_fn=bin_pwd\n",
    );
});

arch_test!(linked_spawn_and_block_accept_child_env_requests, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"spawn REOVIM_MEDIA_PATH=/boot/status pwd\n");
    testrt::check_eq(
        sink_str(),
        "spawn:\npid=4\npath=/bin/pwd\nstate=ready\nloader=linked-bin\nentry_fn=bin_pwd\nartifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n",
    );
    let child = crate::proc::process(4).expect("linked env spawned child process exists");
    testrt::check_eq(child.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(child.program_path, "/bin/pwd");
    testrt::check_eq(child.envc, 1usize);
    testrt::check_eq(child.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(child.env_value(0), "/boot/status");
    testrt::check_eq(child.env_was_truncated(0), false);
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );

    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"block REOVIM_MEDIA_PATH=/boot/status pwd\n");
    testrt::check_eq(
        sink_str(),
        "block:\npid=4\npath=/bin/pwd\nstate=blocked\nloader=linked-bin\nentry_fn=bin_pwd\nartifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n",
    );
    let child = crate::proc::process(4).expect("linked env blocked child process exists");
    testrt::check_eq(child.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(child.block_reason, crate::sched::BlockReason::Operator);
    testrt::check_eq(child.program_path, "/bin/pwd");
    testrt::check_eq(child.envc, 1usize);
    testrt::check_eq(child.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(child.env_value(0), "/boot/status");
    testrt::check_eq(child.env_was_truncated(0), false);
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(rootd_idle_dispatch_runs_ready_spawned_child_before_next_input, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"spawn pwd\n");
    testrt::check_eq(
        sink_str(),
        "spawn:\npid=4\npath=/bin/pwd\nstate=ready\nloader=linked-bin\nentry_fn=bin_pwd\nartifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n",
    );

    let status = run_session_idle_ready_programs(&mut session);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "/\n");

    let child = crate::proc::process(4).expect("spawned child process remains retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(child.parent_pid, crate::proc::ROOTD_PID);

    let scheduler = crate::sched::snapshot_scheduler();
    testrt::check_eq(scheduler.ready_len, 0usize);

    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=4 task=4 status=ok loader=linked-bin entry_fn=bin_pwd\n",
    );
});

arch_test!(linked_wait_pid_waits_for_retained_ready_child, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"spawn pwd\n");
    testrt::check_eq(
        sink_str(),
        "spawn:\npid=4\npath=/bin/pwd\nstate=ready\nloader=linked-bin\nentry_fn=bin_pwd\nartifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n",
    );

    run_session_shell_line(&mut session, b"wait 4\n");
    testrt::check_eq(sink_str(), "/\nwait:\npid=4\nstate=exited\nexit=0\ncompleted=true\n");
    assert_syscall_record(
        "/bin/wait",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/wait",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let child = crate::proc::process(4).expect("linked waited child process remains retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(child.parent_pid, crate::proc::ROOTD_PID);
    let parent = crate::proc::process(5).expect("linked waiting process remains retained");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Exited);

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    testrt::check_eq(wait_count, 1usize);
    testrt::check_eq(waits[0].parent_pid, 5usize);
    testrt::check_eq(waits[0].child_pid, 4usize);
    testrt::check_eq(waits[0].child_state, crate::proc::ProcessState::Exited);
    testrt::check_eq(waits[0].exit_code, 0);
    testrt::check_eq(waits[0].completed, true);

    sink_clear();
    run_session_shell_line(&mut session, b"wait 4\n");
    testrt::check_eq(sink_str(), "wait: not-waitable\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/wait pid=5 task=5 status=ok loader=linked-bin entry_fn=bin_wait\n",
    );
    assert_contains(
        sink_bytes(),
        b"wait.start parent_pid=5 parent_task=5 child_pid=4 child_task=4\n",
    );
    assert_contains(sink_bytes(), b"wait.end parent_pid=5 child_pid=4 child_state=exited exit=0\n");
});

arch_test!(linked_wait_on_blocked_child_preserves_blocked_process_state, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"block pwd\n");
    testrt::check_eq(
        sink_str(),
        "block:\npid=4\npath=/bin/pwd\nstate=blocked\nloader=linked-bin\nentry_fn=bin_pwd\nartifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n",
    );

    run_session_shell_line(&mut session, b"wait 4\n");
    testrt::check_eq(sink_str(), "");

    let child = crate::proc::process(4).expect("blocked child process remains retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Blocked);
    let waiter = crate::proc::process(5).expect("blocked /bin/wait process remains retained");
    testrt::check_eq(waiter.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(waiter.block_reason, crate::sched::BlockReason::WaitChild);
    let waiter_task = crate::sched::task(waiter.task_id).expect("blocked /bin/wait task exists");
    testrt::check_eq(waiter_task.state, crate::sched::KernelTaskState::Blocked);
    testrt::check_eq(waiter_task.block_reason, crate::sched::BlockReason::WaitChild);

    let wait = crate::proc::wait_record(waiter.pid, child.pid).expect("wait record remains");
    testrt::check(!wait.completed, "blocked wait is not terminal");
    testrt::check_eq(wait.child_state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(wait.exit_code, 0);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 1usize);
    testrt::check_eq(continuations[0].process_id, waiter.pid);
    testrt::check_eq(continuations[0].task_id, waiter.task_id);
    testrt::check_eq(continuations[0].program_path, "/bin/wait");
    testrt::check_eq(continuations[0].loader, "linked-bin");
    testrt::check_eq(continuations[0].entry_name, "bin_wait");
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::WAIT);
    testrt::check_eq(continuations[0].op, crate::syscall::SyscallOp::WaitBegin);
    testrt::check_eq(continuations[0].args.a0, child.pid);

    assert_syscall_record(
        "/bin/wait",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/wait",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Blocked,
    );
    assert_no_syscall_record(
        "/bin/wait",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/wait",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"shell.status=blocked\n");
    testrt::check(
        !contains_bytes(sink_bytes(), b"exec.path=/bin/wait pid=5 task=5 status=error"),
        "blocked /bin/wait must not be logged as an exec error",
    );
});

arch_test!(linked_exec_preserves_blocked_replacement_status, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"block pwd\n");
    testrt::check_eq(
        sink_str(),
        "block:\npid=4\npath=/bin/pwd\nstate=blocked\nloader=linked-bin\nentry_fn=bin_pwd\nartifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n",
    );

    run_session_shell_line(&mut session, b"exec wait 4\n");
    testrt::check_eq(sink_str(), "");

    let replacement = crate::proc::process(5).expect("replacement wait process remains retained");
    testrt::check_eq(replacement.program_path, "/bin/wait");
    testrt::check_eq(replacement.entry_name, "bin_wait");
    testrt::check_eq(replacement.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(replacement.block_reason, crate::sched::BlockReason::WaitChild);

    let wait = crate::proc::wait_record(replacement.pid, 4).expect("wait record remains");
    testrt::check(!wait.completed, "replacement wait is not terminal");
    testrt::check_eq(wait.child_state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(wait.exit_code, 0);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 1usize);
    testrt::check_eq(continuations[0].process_id, replacement.pid);
    testrt::check_eq(continuations[0].program_path, "/bin/wait");
    testrt::check_eq(continuations[0].entry_name, "bin_wait");
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::WAIT);
    testrt::check_eq(continuations[0].op, crate::syscall::SyscallOp::WaitBegin);

    assert_syscall_record(
        "/bin/wait",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/wait",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/wait",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"shell.status=blocked\n");
    testrt::check(
        !contains_bytes(sink_bytes(), b"exec: failed"),
        "blocked replacement must not be reported as exec failure",
    );
});

arch_test!(linked_wake_round_trips_child_through_scheduler_ready, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"block pwd\n");
    testrt::check_eq(
        sink_str(),
        "block:\npid=4\npath=/bin/pwd\nstate=blocked\nloader=linked-bin\nentry_fn=bin_pwd\nartifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n",
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Ok,
    );

    run_session_shell_line(&mut session, b"wake 4\n");
    testrt::check_eq(
        sink_str(),
        "wake:\npid=4\npath=/bin/pwd\nstate=ready\nloader=linked-bin\nentry_fn=bin_pwd\nartifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n",
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );
    let child = crate::proc::process(4).expect("woken child process exists");
    testrt::check_eq(child.state, crate::proc::ProcessState::Ready);
    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.ready_len, 1usize);
    testrt::check_eq(snapshot.next_ready_process_id(), child.pid);

    run_session_shell_line(&mut session, b"pwd\n");
    testrt::check_eq(sink_str(), "/\n/\n");
    let child = crate::proc::process(4).expect("woken child process remains retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Exited);
    let later = crate::proc::process(6).expect("later shell pwd process exists");
    testrt::check_eq(later.state, crate::proc::ProcessState::Exited);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/block pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_block\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/wake pid=5 task=5 status=ok loader=linked-bin entry_fn=bin_wake\n",
    );
    assert_contains(
        sink_bytes(),
        b"component=proc severity=info kind=process-block boot=1 session=1 source=process pid=4 task=4\n",
    );
    assert_contains(
        sink_bytes(),
        b"component=proc severity=info kind=process-wake boot=1 session=1 source=process pid=4 task=4\n",
    );
    assert_contains(
        sink_bytes(),
        b"component=sched severity=info kind=scheduler-dispatch boot=1 session=1 source=process pid=4 task=4\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=6 task=6 status=ok loader=linked-bin entry_fn=bin_pwd\n",
    );
});

arch_test!(linked_sleep_wakes_child_on_scheduler_tick_deadline, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"sleep 2 REOVIM_MEDIA_PATH=/boot/status pwd\n");
    testrt::check_eq(
        sink_str(),
        "sleep:\npid=4\npath=/bin/pwd\nstate=blocked\nblock=sleep\nwake_tick=2\n",
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallStatus::Ok,
    );
    let child = crate::proc::process(4).expect("linked sleeping child process exists");
    testrt::check_eq(child.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(child.block_reason, crate::sched::BlockReason::Sleep);
    testrt::check_eq(child.envc, 1usize);
    testrt::check_eq(child.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(child.env_value(0), "/boot/status");
    testrt::check_eq(child.env_was_truncated(0), false);
    let task = crate::sched::task(child.task_id).expect("linked sleeping child task exists");
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::Sleep);
    testrt::check_eq(task.wake_tick, 2usize);

    {
        sink_clear();
        let daemon = daemon(ProfileSummary::new("shell-only", false), None);
        let mut inspector = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
        inspector
            .write_vfs_file_path("/dump/snapshot")
            .expect("dump snapshot renders sleeping child");
        let dump = sink_bytes();
        assert_contains(dump, b"processes:\n");
        assert_contains(dump, b"path=/bin/pwd");
        assert_contains(dump, b"state=blocked");
        assert_contains(dump, b"block=sleep");
        assert_contains(
            dump,
            b"envc=1 env0_name=REOVIM_MEDIA_PATH env0_value=/boot/status env0_truncated=false",
        );
        assert_contains(dump, b"tasks:\n");
        assert_contains(dump, b"entry=/bin/pwd");
        assert_contains(dump, b"block=sleep");
        assert_contains(dump, b"wake_tick=2");
        sink_clear();
    }

    run_session_shell_line(&mut session, b"sched tick\n");
    assert_contains(sink_bytes(), b"sched tick:\nticked=true\nstatus=ok\n");
    assert_contains(sink_bytes(), b"tick_count=1\n");
    let child = crate::proc::process(4).expect("linked sleeping child still retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Blocked);

    run_session_shell_line(&mut session, b"sched tick\n");
    assert_contains(sink_bytes(), b"tick_count=2\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );
    let child = crate::proc::process(4).expect("linked woken sleeping child process exists");
    testrt::check_eq(child.state, crate::proc::ProcessState::Ready);

    run_session_shell_line(&mut session, b"pwd\n");
    testrt::check_eq(sink_str(), "/\n/\n");
    let child = crate::proc::process(4).expect("linked slept child remains retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Exited);
    let parent = crate::proc::process(3).expect("linked sleep parent remains retained");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Exited);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/sleep pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_sleep\n",
    );
    assert_contains(
        sink_bytes(),
        b"component=proc severity=info kind=process-sleep boot=1 session=1 source=process pid=4 task=4\n",
    );
    assert_contains(
        sink_bytes(),
        b"component=proc severity=info kind=process-wake boot=1 session=1 source=process pid=4 task=4\n",
    );
    assert_contains(
        sink_bytes(),
        b"component=sched severity=info kind=scheduler-dispatch boot=1 session=1 source=process pid=4 task=4\n",
    );
});

arch_test!(linked_kill_sleeping_child_clears_deadline_without_late_wake, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"sleep 2 pwd\n");
    testrt::check_eq(
        sink_str(),
        "sleep:\npid=4\npath=/bin/pwd\nstate=blocked\nblock=sleep\nwake_tick=2\n",
    );
    let child = crate::proc::process(4).expect("sleeping child process exists");
    testrt::check_eq(child.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(child.block_reason, crate::sched::BlockReason::Sleep);
    let task = crate::sched::task(child.task_id).expect("sleeping child task exists");
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::Sleep);
    testrt::check_eq(task.wake_tick, 2usize);

    run_session_shell_line(&mut session, b"kill 4\n");
    testrt::check_eq(
        sink_str(),
        "kill:\npid=4\npath=/bin/pwd\nstate=failed\nloader=linked-bin\nentry_fn=bin_pwd\nartifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n",
    );
    let killed = crate::proc::process(4).expect("killed sleeping child remains retained");
    testrt::check_eq(killed.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(killed.block_reason, crate::sched::BlockReason::None);
    let killed_task = crate::sched::task(killed.task_id).expect("killed sleeping task retained");
    testrt::check_eq(killed_task.state, crate::sched::KernelTaskState::Failed);
    testrt::check_eq(killed_task.block_reason, crate::sched::BlockReason::None);
    testrt::check_eq(killed_task.wake_tick, 0usize);
    assert_no_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    run_session_shell_line(&mut session, b"sched tick\n");
    assert_contains(sink_bytes(), b"tick_count=1\n");
    run_session_shell_line(&mut session, b"sched tick\n");
    assert_contains(sink_bytes(), b"tick_count=2\n");

    let killed = crate::proc::process(4).expect("killed sleeping child remains after deadline");
    testrt::check_eq(killed.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(killed.block_reason, crate::sched::BlockReason::None);
    let killed_task =
        crate::sched::task(killed.task_id).expect("killed sleeping task remains after deadline");
    testrt::check_eq(killed_task.state, crate::sched::KernelTaskState::Failed);
    testrt::check_eq(killed_task.block_reason, crate::sched::BlockReason::None);
    testrt::check_eq(killed_task.wake_tick, 0usize);
    assert_no_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.ready_len, 0usize);
    let mut pending =
        [crate::exec::EMPTY_PENDING_EXEC_RECORD; crate::exec::MAX_PENDING_EXEC_RECORDS];
    testrt::check_eq(crate::exec::snapshot_pending(&mut pending), 0usize);
});

arch_test!(linked_wait_ticks_completes_sleeping_child_after_deadline, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"sleep 2 pwd\n");
    testrt::check_eq(
        sink_str(),
        "sleep:\npid=4\npath=/bin/pwd\nstate=blocked\nblock=sleep\nwake_tick=2\n",
    );

    run_session_shell_line(&mut session, b"wait-ticks 2 4\n");
    testrt::check_eq(
        sink_str(),
        "/\nwait-ticks:\npid=4\nstate=exited\nexit=0\ncompleted=true\ntimed_out=false\ntick_count=2\n",
    );
    assert_syscall_record(
        "/bin/wait-ticks",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/wait-ticks",
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/wait-ticks",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );

    let child = crate::proc::process(4).expect("linked wait-ticks child remains retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Reaped);
    testrt::check_eq(child.exit_code, 0);
    let parent = crate::proc::process(5).expect("linked wait-ticks parent remains retained");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Exited);
    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.tick_count, 2usize);

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    testrt::check_eq(wait_count, 1usize);
    testrt::check_eq(waits[0].parent_pid, 5usize);
    testrt::check_eq(waits[0].child_pid, 4usize);
    testrt::check_eq(waits[0].child_state, crate::proc::ProcessState::Exited);
    testrt::check_eq(waits[0].completed, true);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/wait-ticks pid=5 task=5 status=ok loader=linked-bin entry_fn=bin_wait_ticks\n",
    );
    assert_contains(
        sink_bytes(),
        b"component=proc severity=info kind=process-wake boot=1 session=1 source=process pid=4 task=4\n",
    );
    assert_contains(
        sink_bytes(),
        b"component=sched severity=info kind=scheduler-dispatch boot=1 session=1 source=process pid=4 task=4\n",
    );
    assert_contains(sink_bytes(), b"wait.end parent_pid=5 child_pid=4 child_state=exited exit=0\n");
});

arch_test!(linked_wait_ticks_times_out_operator_blocked_child, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"block pwd\n");
    testrt::check_eq(
        sink_str(),
        "block:\npid=4\npath=/bin/pwd\nstate=blocked\nloader=linked-bin\nentry_fn=bin_pwd\nartifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n",
    );

    run_session_shell_line(&mut session, b"wait-ticks 2 4\n");
    testrt::check_eq(
        sink_str(),
        "wait-ticks:\npid=4\nstate=blocked\nexit=0\ncompleted=false\ntimed_out=true\ntick_count=2\n",
    );
    assert_syscall_record(
        "/bin/wait-ticks",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/wait-ticks",
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/wait-ticks",
        crate::syscall::SyscallOp::WaitTimeout,
        crate::syscall::SyscallStatus::Ok,
    );

    let child = crate::proc::process(4).expect("timed-out child remains retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(child.block_reason, crate::sched::BlockReason::Operator);
    let task = crate::sched::task(child.task_id).expect("timed-out child task remains retained");
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::Operator);
    testrt::check_eq(task.wake_tick, 0usize);
    let parent = crate::proc::process(5).expect("timed-out wait parent remains retained");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Exited);
    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.tick_count, 2usize);
    testrt::check_eq(snapshot.ready_len, 0usize);

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    testrt::check_eq(crate::proc::snapshot_waits(&mut waits), 0usize);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"wait.start parent_pid=5 parent_task=5 child_pid=4 child_task=4\n",
    );
    assert_contains(
        sink_bytes(),
        b"wait.timeout parent_pid=5 child_pid=4 child_state=blocked exit=0 tick_count=2\n",
    );
    assert_contains(
        sink_bytes(),
        b"component=proc severity=info kind=wait-timeout boot=1 session=1 source=process pid=5 task=5\n",
    );
});

arch_test!(proc_tables_expose_block_reason_for_blocked_child, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"block pwd\n");
    testrt::check_eq(
        sink_str(),
        "block:\npid=4\npath=/bin/pwd\nstate=blocked\nloader=linked-bin\nentry_fn=bin_pwd\nartifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n",
    );

    run_session_shell_line(&mut session, b"proc processes\n");
    assert_contains(
        sink_bytes(),
        b"state=blocked path=/bin/pwd exit=0 loader=linked-bin entry_fn=bin_pwd block=operator",
    );

    run_session_shell_line(&mut session, b"proc tasks\n");
    assert_contains(sink_bytes(), b"state=blocked entry=/bin/pwd block=operator");
});

arch_test!(process_block_and_sleep_fail_closed_when_scheduler_task_is_missing, {
    crate::proc::reset();
    crate::syscall::reset();

    let child = crate::proc::spawn_child(
        crate::proc::SHELL_PID,
        crate::proc::SHELL_PID,
        "/bin/pwd",
        "linked-bin",
        "bin_pwd",
    )
    .expect("child process slot available");
    testrt::check(
        crate::sched::task(child.task_id).is_some(),
        "spawned child task exists before scheduler reset",
    );

    crate::sched::reset_kernel_scheduler();
    testrt::check(
        crate::sched::task(child.task_id).is_none(),
        "scheduler reset removes child task",
    );

    testrt::check(
        crate::proc::block_process(child.pid).is_none(),
        "operator block fails without scheduler task",
    );
    let record = crate::proc::process(child.pid).expect("child process remains retained");
    testrt::check_eq(record.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(record.block_reason, crate::sched::BlockReason::None);

    testrt::check(
        crate::proc::sleep_process_until(child.pid, 2).is_none(),
        "sleep block fails without scheduler task",
    );
    let record = crate::proc::process(child.pid).expect("child process remains retained");
    testrt::check_eq(record.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(record.block_reason, crate::sched::BlockReason::None);
});

arch_test!(linked_kill_terminates_blocked_child_without_dispatching_it, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"block pwd\n");
    testrt::check_eq(
        sink_str(),
        "block:\npid=4\npath=/bin/pwd\nstate=blocked\nloader=linked-bin\nentry_fn=bin_pwd\nartifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n",
    );

    run_session_shell_line(&mut session, b"kill 4\n");
    testrt::check_eq(
        sink_str(),
        "kill:\npid=4\npath=/bin/pwd\nstate=failed\nloader=linked-bin\nentry_fn=bin_pwd\nartifact_body_format=linked-image\nartifact_body_inner=none\nartifact_body_bytes=0\nartifact_checksum=0\n",
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessKill,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );

    let child = crate::proc::process(4).expect("killed child process remains retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Failed);
    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.ready_len, 0usize);
    let mut pending =
        [crate::exec::EMPTY_PENDING_EXEC_RECORD; crate::exec::MAX_PENDING_EXEC_RECORDS];
    testrt::check_eq(crate::exec::snapshot_pending(&mut pending), 0usize);

    run_session_shell_line(&mut session, b"pwd\n");
    testrt::check_eq(sink_str(), "/\n");
    let later = crate::proc::process(6).expect("later shell pwd process exists");
    testrt::check_eq(later.state, crate::proc::ProcessState::Exited);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/kill pid=5 task=5 status=ok loader=linked-bin entry_fn=bin_kill\n",
    );
    assert_contains(
        sink_bytes(),
        b"component=proc severity=info kind=process-kill boot=1 session=1 source=process pid=4 task=4\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=6 task=6 status=ok loader=linked-bin entry_fn=bin_pwd\n",
    );
});

arch_test!(program_exec_wait_dispatches_older_ready_payload_before_child_program, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();

    let parent_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "proc",
    )
    .expect("proc loads");
    let parent_ctx = crate::syscall::exec_bin_from_shell(parent_program, argv1("proc"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(parent_ctx));
    let _ = crate::syscall::take_pending_exec(parent_ctx.pid);

    let payload =
        crate::exec::load_payload_by_name(daemon.payloads(), daemon.source_store(), "reovim")
            .expect("payload loads through exec");
    let shell = crate::proc::ProcessHandle {
        pid: crate::proc::SHELL_PID,
        task_id: crate::proc::SHELL_PID,
        program_path: "root-shell",
        loader: "kernel",
        entry_name: "root_shell",
    };
    let older_payload = crate::exec::spawn_payload_child(shell, payload);
    testrt::check(
        crate::proc::begin_wait(shell.pid, older_payload.pid).is_some(),
        "older payload wait starts",
    );

    let mut child_argv = ProgramArgvBuffer::empty();
    child_argv.push("pwd").expect("argv0 fits");
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(parent_ctx));
    let status = syscalls.exec_program_argv_and_wait(child_argv);

    testrt::check_eq(status, Ok(crate::program::ProgramStatus::Ok));
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut records = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let count = crate::proc::snapshot(&mut records);
    let mut found_payload = false;
    let mut found_pwd = false;
    let mut index = 0usize;
    while index < count {
        if records[index].program_path == "/payload/reovim" {
            found_payload = true;
            testrt::check_eq(records[index].state, crate::proc::ProcessState::Reaped);
            testrt::check_eq(records[index].exit_code, 0);
        }
        if records[index].program_path == "/bin/pwd" {
            found_pwd = true;
            testrt::check_eq(records[index].state, crate::proc::ProcessState::Reaped);
            testrt::check_eq(records[index].exit_code, 0);
        }
        index += 1;
    }
    testrt::check(found_payload, "older payload process ran");
    testrt::check(found_pwd, "waited /bin child process ran");
});

arch_test!(program_syscalls_fd_io_uses_standard_descriptor_table, {
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let mut syscalls = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);

    testrt::check_eq(syscalls.write_fd(1, b"stdout"), Ok(6usize));
    testrt::check_eq(sink_str(), "stdout");

    sink_clear();
    testrt::check_eq(syscalls.write_fd_line(2, "stderr"), Ok(7usize));
    testrt::check_eq(sink_str(), "stderr\n");
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );

    let stdout_dup = syscalls
        .duplicate_fd(1)
        .expect("stdout descriptor can be duplicated");
    testrt::check_eq(stdout_dup, crate::syscall::PROGRAM_VFS_FILE_FD);
    sink_clear();
    testrt::check_eq(syscalls.write_fd(stdout_dup, b"dup-stdout"), Ok(10usize));
    testrt::check_eq(sink_str(), "dup-stdout");
    testrt::check_eq(
        syscalls.read_fd(stdout_dup, &mut [0u8; 1]).err(),
        Some(crate::syscall::ProgramIoError::NotReadable),
    );
    testrt::check_eq(
        syscalls
            .seek_fd(stdout_dup, 0, reovim_uapi_fs::SeekWhence::start())
            .err(),
        Some(crate::syscall::ProgramIoError::NotSeekable),
    );
    testrt::check_eq(syscalls.close_fd(1), Ok(()));
    testrt::check_eq(
        syscalls.write_fd(1, b"closed").err(),
        Some(crate::syscall::ProgramIoError::BadFd),
    );
    sink_clear();
    testrt::check_eq(syscalls.write_fd(stdout_dup, b"still-open"), Ok(10usize));
    testrt::check_eq(sink_str(), "still-open");
    testrt::check_eq(syscalls.close_fd(stdout_dup), Ok(()));
    testrt::check_eq(
        syscalls.write_fd(stdout_dup, b"closed").err(),
        Some(crate::syscall::ProgramIoError::BadFd),
    );

    let stdout_capture = crate::syscall::ProgramStdoutCapture::new();
    let mut captured_session = RootShellSession::new();
    let mut captured = crate::syscall::ProgramSyscalls::new_with_stdin_and_stdout_capture(
        &daemon,
        &mut captured_session,
        None,
        &[],
        Some(&stdout_capture),
    );
    let captured_dup = captured
        .duplicate_fd(1)
        .expect("captured stdout descriptor can be duplicated");
    testrt::check_eq(captured_dup, crate::syscall::PROGRAM_VFS_FILE_FD);
    testrt::check_eq(captured.close_fd(1), Ok(()));
    sink_clear();
    testrt::check_eq(captured.write_fd(captured_dup, b"captured"), Ok(8usize));
    testrt::check_eq(sink_str(), "");
    let mut captured_buf = [0u8; 16];
    let captured_len = stdout_capture.copy_into(&mut captured_buf);
    testrt::check_eq(captured_len, 8usize);
    testrt::check_eq(&captured_buf[..captured_len], b"captured");
    testrt::check_eq(captured.close_fd(captured_dup), Ok(()));

    testrt::check_eq(
        syscalls.write_fd(0, b"nope").err(),
        Some(crate::syscall::ProgramIoError::NotWritable),
    );
    testrt::check_eq(
        syscalls.write_fd(99, b"nope").err(),
        Some(crate::syscall::ProgramIoError::BadFd),
    );
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Error,
    );

    let mut read_buf = [0u8; 8];
    testrt::check_eq(syscalls.read_fd(0, &mut read_buf), Ok(0usize));
    assert_syscall_record("", crate::syscall::SyscallOp::FdRead, crate::syscall::SyscallStatus::Ok);

    let mut seeded_session = RootShellSession::new();
    let mut seeded =
        crate::syscall::ProgramSyscalls::new_with_stdin(&daemon, &mut seeded_session, None, b"abc");
    let stdin_dup = seeded
        .duplicate_fd(0)
        .expect("stdin descriptor can be duplicated");
    testrt::check_eq(stdin_dup, crate::syscall::PROGRAM_VFS_FILE_FD);
    testrt::check_eq(seeded.read_fd(0, &mut read_buf[..2]), Ok(2usize));
    testrt::check_eq(&read_buf[..2], b"ab");
    testrt::check_eq(seeded.read_fd(stdin_dup, &mut read_buf), Ok(1usize));
    testrt::check_eq(&read_buf[..1], b"c");
    testrt::check_eq(seeded.read_fd(0, &mut read_buf), Ok(0usize));
    testrt::check_eq(seeded.close_fd(stdin_dup), Ok(()));

    let mut reused_stdio_session = RootShellSession::new();
    let mut reused_stdio = crate::syscall::ProgramSyscalls::new_with_stdin(
        &daemon,
        &mut reused_stdio_session,
        None,
        b"abc",
    );
    testrt::check_eq(reused_stdio.close_fd(0), Ok(()));
    testrt::check_eq(
        reused_stdio.read_fd(0, &mut read_buf).err(),
        Some(crate::syscall::ProgramIoError::BadFd),
    );
    let reused_fd = reused_stdio
        .open_vfs_file_path("/boot/profile")
        .expect("closed stdio descriptor slot can be reused by open");
    testrt::check_eq(reused_fd, 0usize);
    let mut reused_file_buf = [0u8; 8];
    testrt::check_eq(reused_stdio.read_fd(reused_fd, &mut reused_file_buf), Ok(8usize));
    testrt::check_eq(&reused_file_buf[..8], b"profile=");
    testrt::check_eq(reused_stdio.close_fd(reused_fd), Ok(()));

    let fd = seeded
        .open_vfs_file_path("/boot/profile")
        .expect("boot profile opens as a VFS fd");
    testrt::check_eq(fd, crate::syscall::PROGRAM_VFS_FILE_FD);
    let mut file_buf = [0u8; 8];
    let read = seeded
        .read_fd(fd, &mut file_buf)
        .expect("opened VFS fd reads bytes");
    testrt::check_eq(read, 8usize);
    testrt::check_eq(&file_buf[..8], b"profile=");
    let fd2 = seeded
        .open_vfs_file_path("/boot/profile")
        .expect("second boot profile open gets another VFS fd");
    testrt::check_eq(fd2, crate::syscall::PROGRAM_VFS_FILE_FD + 1);
    let read2 = seeded
        .read_fd(fd2, &mut file_buf)
        .expect("second opened VFS fd has an independent cursor");
    testrt::check_eq(read2, 8usize);
    testrt::check_eq(&file_buf[..8], b"profile=");
    let read_again = seeded
        .read_fd(fd, &mut file_buf)
        .expect("first VFS fd remains readable after second open");
    testrt::check_eq(read_again, 8usize);
    testrt::check(&file_buf[..8] != b"profile=", "first VFS fd cursor advances independently");
    testrt::check_eq(seeded.seek_fd(fd, 0, reovim_uapi_fs::SeekWhence::start()), Ok(0usize));
    testrt::check_eq(seeded.read_fd(fd, &mut file_buf), Ok(8usize));
    testrt::check_eq(&file_buf[..8], b"profile=");
    let end_offset = seeded
        .seek_fd(fd, 0, reovim_uapi_fs::SeekWhence::end())
        .expect("VFS fd can seek to end");
    testrt::check(end_offset > 8, "boot profile is longer than the prefix");
    testrt::check_eq(seeded.read_fd(fd, &mut file_buf), Ok(0usize));
    testrt::check_eq(
        seeded
            .seek_fd(fd, -1, reovim_uapi_fs::SeekWhence::start())
            .err(),
        Some(crate::syscall::ProgramIoError::InvalidArgument),
    );
    testrt::check_eq(
        seeded
            .seek_fd(1, 0, reovim_uapi_fs::SeekWhence::start())
            .err(),
        Some(crate::syscall::ProgramIoError::NotSeekable),
    );
    let fd_dup = seeded
        .duplicate_fd(fd)
        .expect("duplicate VFS fd gets a second descriptor");
    testrt::check_eq(fd_dup, crate::syscall::PROGRAM_VFS_FILE_FD + 2);
    testrt::check_eq(seeded.seek_fd(fd_dup, 0, reovim_uapi_fs::SeekWhence::start()), Ok(0usize));
    testrt::check_eq(seeded.read_fd(fd, &mut file_buf), Ok(8usize));
    testrt::check_eq(&file_buf[..8], b"profile=");
    testrt::check_eq(seeded.read_fd(fd_dup, &mut file_buf), Ok(8usize));
    testrt::check(
        &file_buf[..8] != b"profile=",
        "duplicate fd shares the open-file-description cursor",
    );
    testrt::check_eq(seeded.seek_fd(fd_dup, 0, reovim_uapi_fs::SeekWhence::start()), Ok(0usize));
    testrt::check_eq(
        seeded.write_fd(fd, b"nope").err(),
        Some(crate::syscall::ProgramIoError::NotWritable),
    );
    testrt::check_eq(seeded.close_fd(fd), Ok(()));
    testrt::check_eq(
        seeded.read_fd(fd, &mut file_buf).err(),
        Some(crate::syscall::ProgramIoError::BadFd),
    );
    testrt::check_eq(seeded.read_fd(fd_dup, &mut file_buf), Ok(8usize));
    testrt::check_eq(&file_buf[..8], b"profile=");
    testrt::check_eq(seeded.close_fd(fd_dup), Ok(()));
    testrt::check_eq(seeded.read_fd(fd2, &mut file_buf), Ok(8usize));
    let fd3 = seeded
        .open_vfs_file_path("/boot/profile")
        .expect("closed VFS fd slot is reused by the descriptor table");
    testrt::check_eq(fd3, fd);
    testrt::check_eq(seeded.read_fd(fd3, &mut file_buf), Ok(8usize));
    testrt::check_eq(&file_buf[..8], b"profile=");
    testrt::check_eq(seeded.close_fd(fd3), Ok(()));
    testrt::check_eq(seeded.close_fd(fd2), Ok(()));
    testrt::check_eq(seeded.close_fd(fd).err(), Some(crate::syscall::ProgramIoError::BadFd));
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::VfsRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::FdClose,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record("", crate::syscall::SyscallOp::FdSeek, crate::syscall::SyscallStatus::Ok);
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::FdDuplicate,
        crate::syscall::SyscallStatus::Ok,
    );

    testrt::check_eq(
        seeded.read_fd(1, &mut read_buf).err(),
        Some(crate::syscall::ProgramIoError::NotReadable),
    );
    testrt::check_eq(
        seeded.read_fd(99, &mut read_buf).err(),
        Some(crate::syscall::ProgramIoError::BadFd),
    );
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Error,
    );
});

arch_test!(program_syscalls_vfs_fd_table_is_process_owned, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let handle = crate::proc::spawn_child(
        crate::proc::ROOTD_PID,
        crate::proc::ROOTD_PID,
        "/bin/fd-owner",
        "linked-bin",
        "bin_fd_owner",
    )
    .expect("child process slot available");
    let running = crate::proc::run_process(handle.pid).expect("test process can run");
    let current = crate::syscall::SyscallContext {
        pid: running.pid,
        task_id: running.task_id,
        program_path: running.program_path,
        loader: running.loader,
        entry_name: running.entry_name,
    };
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut file_buf = [0u8; 8];
    let fd;

    {
        let stdout_capture = crate::syscall::ProgramStdoutCapture::new();
        let mut session = RootShellSession::new();
        let mut captured = crate::syscall::ProgramSyscalls::new_with_stdin_and_stdout_capture(
            &daemon,
            &mut session,
            Some(current),
            &[],
            Some(&stdout_capture),
        );
        let stdout_dup = captured
            .duplicate_fd(1)
            .expect("process stdout descriptor can be duplicated while captured");
        sink_clear();
        testrt::check_eq(captured.write_fd(stdout_dup, b"proc-cap"), Ok(8usize));
        testrt::check_eq(sink_str(), "");
        let mut captured_buf = [0u8; 16];
        let captured_len = stdout_capture.copy_into(&mut captured_buf);
        testrt::check_eq(captured_len, 8usize);
        testrt::check_eq(&captured_buf[..captured_len], b"proc-cap");
        testrt::check_eq(captured.close_fd(stdout_dup), Ok(()));
    }

    {
        let mut session = RootShellSession::new();
        let second = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current));
        sink_clear();
        testrt::check_eq(second.write_fd(1, b"console"), Ok(7usize));
        testrt::check_eq(sink_str(), "console");
    }

    {
        let mut session = RootShellSession::new();
        let mut first = crate::syscall::ProgramSyscalls::new_with_stdin(
            &daemon,
            &mut session,
            Some(current),
            b"abc",
        );
        let mut stdin_buf = [0u8; 2];
        testrt::check_eq(first.read_fd(0, &mut stdin_buf), Ok(2usize));
        testrt::check_eq(&stdin_buf, b"ab");
        fd = first
            .open_vfs_file_path("/boot/profile")
            .expect("process-owned VFS fd opens");
        testrt::check_eq(fd, crate::syscall::PROGRAM_VFS_FILE_FD);
        testrt::check_eq(first.read_fd(fd, &mut file_buf), Ok(8usize));
        testrt::check_eq(&file_buf[..8], b"profile=");
    }

    {
        let mut session = RootShellSession::new();
        let mut second = crate::syscall::ProgramSyscalls::new_with_stdin(
            &daemon,
            &mut session,
            Some(current),
            b"ignored",
        );
        let mut stdin_buf = [0u8; 2];
        testrt::check_eq(second.read_fd(0, &mut stdin_buf), Ok(1usize));
        testrt::check_eq(&stdin_buf[..1], b"c");
        testrt::check_eq(second.read_fd(0, &mut stdin_buf), Ok(0usize));
        testrt::check_eq(second.read_fd(fd, &mut file_buf), Ok(8usize));
        testrt::check(
            &file_buf[..8] != b"profile=",
            "same process keeps VFS fd cursor across syscall handles",
        );
        testrt::check_eq(second.seek_fd(fd, 0, reovim_uapi_fs::SeekWhence::start()), Ok(0usize));
    }

    {
        let mut session = RootShellSession::new();
        let mut third = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current));
        testrt::check_eq(third.read_fd(fd, &mut file_buf), Ok(8usize));
        testrt::check_eq(&file_buf[..8], b"profile=");
    }

    crate::syscall::exit_current(current, crate::program::ProgramStatus::Ok);

    {
        let mut session = RootShellSession::new();
        let mut after_exit =
            crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current));
        testrt::check_eq(
            after_exit.read_fd(fd, &mut file_buf).err(),
            Some(crate::syscall::ProgramIoError::BadFd),
        );
    }
});

arch_test!(program_syscalls_dispatches_raw_fd_and_process_syscalls, {
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let current = crate::syscall::SyscallContext {
        pid: 42,
        task_id: 7,
        program_path: "/bin/raw-test",
        loader: "linked-bin",
        entry_name: "raw_test",
    };
    let mut syscalls = crate::syscall::ProgramSyscalls::new_with_stdin(
        &daemon,
        &mut session,
        Some(current),
        b"raw",
    );

    let mut read_buf = [0u8; 8];
    let read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            0,
            read_buf.as_mut_ptr() as usize,
            read_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(read.decode(), Ok(3usize));
    testrt::check_eq(&read_buf[..3], b"raw");

    let write = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([1, b"rawout".as_ptr() as usize, 6, 0, 0, 0]),
    );
    testrt::check_eq(write.decode(), Ok(6usize));
    testrt::check_eq(sink_str(), "rawout");

    let get_pid = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::GET_PID,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(get_pid.decode(), Ok(42usize));

    let mut cwd_buf = [0u8; 8];
    let get_cwd = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::GET_CWD,
        reovim_uapi_syscall::SyscallArgs::new([
            cwd_buf.as_mut_ptr() as usize,
            cwd_buf.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(get_cwd.decode(), Ok(1usize));
    testrt::check_eq(&cwd_buf[..1], b"/");
    assert_syscall_record(
        "/bin/raw-test",
        crate::syscall::SyscallOp::SessionCwdGet,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut small_cwd_buf = [0u8; 0];
    let small_cwd = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::GET_CWD,
        reovim_uapi_syscall::SyscallArgs::new([
            small_cwd_buf.as_mut_ptr() as usize,
            small_cwd_buf.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(small_cwd.decode(), Err(reovim_uapi_syscall::SyscallError::FILE_TOO_LARGE));

    let mut pipe_fds = [-1i32; 2];
    let pipe = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));
    testrt::check_eq(pipe_fds, [3, 4]);
    assert_syscall_record(
        "/bin/raw-test",
        crate::syscall::SyscallOp::FdPipe,
        crate::syscall::SyscallStatus::Ok,
    );

    let pipe_dup = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUP,
        reovim_uapi_syscall::SyscallArgs::new([pipe_fds[0] as usize, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(pipe_dup.decode(), Ok(5usize));
    let pipe_status_initial = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_STATUS_GET,
        reovim_uapi_syscall::SyscallArgs::new([pipe_fds[0] as usize, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(pipe_status_initial.decode(), Ok(FileStatusFlags::EMPTY.raw() as usize));
    let pipe_status_set = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_STATUS_SET,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            FileStatusFlags::NONBLOCK.raw() as usize,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe_status_set.decode(), Ok(0usize));
    let pipe_status_after = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_STATUS_GET,
        reovim_uapi_syscall::SyscallArgs::new([5, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(pipe_status_after.decode(), Ok(FileStatusFlags::NONBLOCK.raw() as usize));
    assert_syscall_record(
        "/bin/raw-test",
        crate::syscall::SyscallOp::FdStatusGet,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/raw-test",
        crate::syscall::SyscallOp::FdStatusSet,
        crate::syscall::SyscallStatus::Ok,
    );
    let pipe_write = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            b"abc".as_ptr() as usize,
            3,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe_write.decode(), Ok(3usize));

    let mut pipe_buf = [0u8; 4];
    let pipe_read_dup = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([5, pipe_buf.as_mut_ptr() as usize, 2, 0, 0, 0]),
    );
    testrt::check_eq(pipe_read_dup.decode(), Ok(2usize));
    testrt::check_eq(&pipe_buf[..2], b"ab");
    let pipe_read_original = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe_read_original.decode(), Ok(1usize));
    testrt::check_eq(&pipe_buf[..1], b"c");

    let read_write_end = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(read_write_end.decode(), Err(reovim_uapi_syscall::SyscallError::NOT_READABLE));
    let write_read_end = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            b"x".as_ptr() as usize,
            1,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(write_read_end.decode(), Err(reovim_uapi_syscall::SyscallError::NOT_WRITABLE));
    let pipe_seek = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::LSEEK,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            0,
            reovim_uapi_fs::SeekWhence::start().raw() as usize,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe_seek.decode(), Err(reovim_uapi_syscall::SyscallError::NOT_SEEKABLE));
    let pipe_getdents = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::GETDENTS,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe_getdents.decode(), Err(reovim_uapi_syscall::SyscallError::NOT_DIRECTORY));
    let pipe_extra_arg = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            1,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(
        pipe_extra_arg.decode(),
        Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT),
    );
    let close_pipe_dup = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([5, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(close_pipe_dup.decode(), Ok(0usize));
    let close_pipe_write = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([pipe_fds[1] as usize, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(close_pipe_write.decode(), Ok(0usize));
    let pipe_eof = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe_eof.decode(), Ok(0usize));
    let close_pipe_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([pipe_fds[0] as usize, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(close_pipe_read.decode(), Ok(0usize));

    let open_path = b"/boot/profile";
    let open = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_fs::OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            reovim_uapi_fs::OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(open.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD));
    let open_second = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_fs::OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            reovim_uapi_fs::OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(open_second.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD + 1));
    assert_syscall_record(
        "/bin/raw-test",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    let fd_flags_initial = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_FLAGS_GET,
        reovim_uapi_syscall::SyscallArgs::new([crate::syscall::PROGRAM_VFS_FILE_FD, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(fd_flags_initial.decode(), Ok(DescriptorFlags::EMPTY.raw() as usize));
    let fd_flags_set = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_FLAGS_SET,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            DescriptorFlags::CLOSE_ON_EXEC.raw() as usize,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(fd_flags_set.decode(), Ok(0usize));
    let fd_flags_after = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_FLAGS_GET,
        reovim_uapi_syscall::SyscallArgs::new([crate::syscall::PROGRAM_VFS_FILE_FD, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(fd_flags_after.decode(), Ok(DescriptorFlags::CLOSE_ON_EXEC.raw() as usize));
    assert_syscall_record(
        "/bin/raw-test",
        crate::syscall::SyscallOp::FdFlagsGet,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/raw-test",
        crate::syscall::SyscallOp::FdFlagsSet,
        crate::syscall::SyscallStatus::Ok,
    );
    let fd_flags_dup = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUP,
        reovim_uapi_syscall::SyscallArgs::new([crate::syscall::PROGRAM_VFS_FILE_FD, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(fd_flags_dup.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD + 2));
    let fd_flags_dup_get = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_FLAGS_GET,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 2,
            0,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(fd_flags_dup_get.decode(), Ok(DescriptorFlags::EMPTY.raw() as usize));
    let close_fd_flags_dup = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 2,
            0,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(close_fd_flags_dup.decode(), Ok(0usize));
    let fd_flags_get_extra_arg = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_FLAGS_GET,
        reovim_uapi_syscall::SyscallArgs::new([crate::syscall::PROGRAM_VFS_FILE_FD, 1, 0, 0, 0, 0]),
    );
    testrt::check_eq(
        fd_flags_get_extra_arg.decode(),
        Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT),
    );
    let fd_flags_set_invalid = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_FLAGS_SET,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            DescriptorFlags::new(2).raw() as usize,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(
        fd_flags_set_invalid.decode(),
        Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT),
    );
    let fd_status_set_invalid = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_STATUS_SET,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            FileStatusFlags::NONBLOCK.raw() as usize,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(
        fd_status_set_invalid.decode(),
        Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT),
    );

    let dir_path = b"/boot";
    let open_dir = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_fs::OpenAtDir::session_cwd().raw() as usize,
            dir_path.as_ptr() as usize,
            dir_path.len(),
            reovim_uapi_fs::OpenFlags::READ_DIRECTORY.raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(open_dir.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD + 2));
    let mut dir_buf = [0u8; 16];
    let raw_getdents = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::GETDENTS,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 2,
            dir_buf.as_mut_ptr() as usize,
            dir_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(raw_getdents.decode(), Ok(16usize));
    testrt::check_eq(&dir_buf, b"devices\nhelp\nima");
    let file_getdents = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::GETDENTS,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            dir_buf.as_mut_ptr() as usize,
            dir_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(file_getdents.decode(), Err(reovim_uapi_syscall::SyscallError::NOT_DIRECTORY));
    let dir_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 2,
            read_buf.as_mut_ptr() as usize,
            read_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(dir_read.decode(), Err(reovim_uapi_syscall::SyscallError::NOT_READABLE));
    let relative_profile = b"profile";
    let raw_relative_open = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_fs::OpenAtDir::from_fd(reovim_uapi_fs::RawFd::new(
                (crate::syscall::PROGRAM_VFS_FILE_FD + 2) as i32,
            ))
            .raw() as usize,
            relative_profile.as_ptr() as usize,
            relative_profile.len(),
            reovim_uapi_fs::OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(raw_relative_open.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD + 3));
    let raw_relative_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 3,
            read_buf.as_mut_ptr() as usize,
            read_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(raw_relative_read.decode(), Ok(8usize));
    testrt::check_eq(&read_buf[..8], b"profile=");
    let close_relative = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 3,
            0,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(close_relative.decode(), Ok(0usize));
    let file_anchor = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_fs::OpenAtDir::from_fd(reovim_uapi_fs::RawFd::new(
                crate::syscall::PROGRAM_VFS_FILE_FD as i32,
            ))
            .raw() as usize,
            relative_profile.as_ptr() as usize,
            relative_profile.len(),
            reovim_uapi_fs::OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(file_anchor.decode(), Err(reovim_uapi_syscall::SyscallError::NOT_DIRECTORY));
    let close_dir = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 2,
            0,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(close_dir.decode(), Ok(0usize));

    let raw_vfs_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            read_buf.as_mut_ptr() as usize,
            read_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(raw_vfs_read.decode(), Ok(8usize));
    testrt::check_eq(&read_buf[..8], b"profile=");

    let seek_start = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::LSEEK,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            0,
            reovim_uapi_fs::SeekWhence::start().raw() as usize,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(seek_start.decode(), Ok(0usize));
    let raw_vfs_reread = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            read_buf.as_mut_ptr() as usize,
            read_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(raw_vfs_reread.decode(), Ok(8usize));
    testrt::check_eq(&read_buf[..8], b"profile=");
    let seek_current = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::LSEEK,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            1,
            reovim_uapi_fs::SeekWhence::current().raw() as usize,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(seek_current.decode(), Ok(9usize));
    let seek_end = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::LSEEK,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            0,
            reovim_uapi_fs::SeekWhence::end().raw() as usize,
            0,
            0,
            0,
        ]),
    );
    let end_offset = seek_end.decode().expect("seek end succeeds");
    testrt::check(end_offset > 8, "boot profile has content after prefix");
    let raw_vfs_eof = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            read_buf.as_mut_ptr() as usize,
            read_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(raw_vfs_eof.decode(), Ok(0usize));

    let dup = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUP,
        reovim_uapi_syscall::SyscallArgs::new([crate::syscall::PROGRAM_VFS_FILE_FD, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(dup.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD + 2));
    let seek_dup_start = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::LSEEK,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 2,
            0,
            reovim_uapi_fs::SeekWhence::start().raw() as usize,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(seek_dup_start.decode(), Ok(0usize));
    let close = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([crate::syscall::PROGRAM_VFS_FILE_FD, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(close.decode(), Ok(0usize));
    let raw_vfs_read_dup = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 2,
            read_buf.as_mut_ptr() as usize,
            read_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(raw_vfs_read_dup.decode(), Ok(8usize));
    testrt::check_eq(&read_buf[..8], b"profile=");
    let close_dup = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 2,
            0,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(close_dup.decode(), Ok(0usize));
    let raw_vfs_read_second = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 1,
            read_buf.as_mut_ptr() as usize,
            read_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(raw_vfs_read_second.decode(), Ok(8usize));
    testrt::check_eq(&read_buf[..8], b"profile=");
    let open_reused = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_fs::OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            reovim_uapi_fs::OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(open_reused.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD));
    let dup_to_reused = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUP_TO,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 1,
            crate::syscall::PROGRAM_VFS_FILE_FD,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(dup_to_reused.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD));
    let seek_dup_to_start = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::LSEEK,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            0,
            reovim_uapi_fs::SeekWhence::start().raw() as usize,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(seek_dup_to_start.decode(), Ok(0usize));
    let raw_vfs_read_dup_to_old = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 1,
            read_buf.as_mut_ptr() as usize,
            read_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(raw_vfs_read_dup_to_old.decode(), Ok(8usize));
    testrt::check_eq(&read_buf[..8], b"profile=");
    let seek_dup_to_old_start = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::LSEEK,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 1,
            0,
            reovim_uapi_fs::SeekWhence::start().raw() as usize,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(seek_dup_to_old_start.decode(), Ok(0usize));
    let close_second = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 1,
            0,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(close_second.decode(), Ok(0usize));
    let raw_vfs_read_dup_to_target = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            read_buf.as_mut_ptr() as usize,
            read_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(raw_vfs_read_dup_to_target.decode(), Ok(8usize));
    testrt::check_eq(&read_buf[..8], b"profile=");
    let dup_to_self = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUP_TO,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            crate::syscall::PROGRAM_VFS_FILE_FD,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(dup_to_self.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD));
    let dup_to_extra_arg = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUP_TO,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            crate::syscall::PROGRAM_VFS_FILE_FD + 1,
            1,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(
        dup_to_extra_arg.decode(),
        Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT),
    );
    let close_reused = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([crate::syscall::PROGRAM_VFS_FILE_FD, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(close_reused.decode(), Ok(0usize));
    let bad_dup_to = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUP_TO,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            crate::syscall::PROGRAM_VFS_FILE_FD + 1,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(bad_dup_to.decode(), Err(reovim_uapi_syscall::SyscallError::BAD_DESCRIPTOR));
    let bad_dup = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUP,
        reovim_uapi_syscall::SyscallArgs::new([crate::syscall::PROGRAM_VFS_FILE_FD, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(bad_dup.decode(), Err(reovim_uapi_syscall::SyscallError::BAD_DESCRIPTOR));

    let missing_path = b"/boot/missing";
    let not_found = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_fs::OpenAtDir::session_cwd().raw() as usize,
            missing_path.as_ptr() as usize,
            missing_path.len(),
            reovim_uapi_fs::OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(not_found.decode(), Err(reovim_uapi_syscall::SyscallError::NOT_FOUND));

    let bad_dir = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_fs::OpenAtDir::from_fd(reovim_uapi_fs::RawFd::new(8)).raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            reovim_uapi_fs::OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(bad_dir.decode(), Err(reovim_uapi_syscall::SyscallError::BAD_DESCRIPTOR));

    let bad_flags = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_fs::OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            reovim_uapi_fs::OpenFlags::new(99).raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(bad_flags.decode(), Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT));

    let tty_path = b"/dev/tty";
    let open_tty_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_fs::OpenAtDir::session_cwd().raw() as usize,
            tty_path.as_ptr() as usize,
            tty_path.len(),
            reovim_uapi_fs::OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    let tty_read_fd = open_tty_read.decode().expect("read-only tty opens");
    let write_read_tty = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([tty_read_fd, b"bad".as_ptr() as usize, 3, 0, 0, 0]),
    );
    testrt::check_eq(write_read_tty.decode(), Err(reovim_uapi_syscall::SyscallError::NOT_WRITABLE));
    let close_tty_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([tty_read_fd, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(close_tty_read.decode(), Ok(0usize));

    let open_tty_write = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_fs::OpenAtDir::session_cwd().raw() as usize,
            tty_path.as_ptr() as usize,
            tty_path.len(),
            reovim_uapi_fs::OpenFlags::WRITE_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    let tty_write_fd = open_tty_write.decode().expect("write-only tty opens");
    sink_clear();
    let write_tty = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            tty_write_fd,
            b"ttyout".as_ptr() as usize,
            6,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(write_tty.decode(), Ok(6usize));
    testrt::check_eq(sink_str(), "ttyout");
    assert_syscall_record(
        "/bin/raw-test",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    let read_write_tty = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            tty_write_fd,
            read_buf.as_mut_ptr() as usize,
            read_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(read_write_tty.decode(), Err(reovim_uapi_syscall::SyscallError::NOT_READABLE));
    let close_tty_write = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([tty_write_fd, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(close_tty_write.decode(), Ok(0usize));

    let invalid_raw_mode_enter = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::TERMINAL_RAW_ENTER,
        reovim_uapi_syscall::SyscallArgs::new([99, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(
        invalid_raw_mode_enter.decode(),
        Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT),
    );
    assert_syscall_record(
        "/bin/raw-test",
        crate::syscall::SyscallOp::TtyRawEnter,
        crate::syscall::SyscallStatus::Error,
    );
    let invalid_raw_mode_restore = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::TERMINAL_RAW_RESTORE,
        reovim_uapi_syscall::SyscallArgs::new([2_147_483_648, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(
        invalid_raw_mode_restore.decode(),
        Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT),
    );
    assert_syscall_record(
        "/bin/raw-test",
        crate::syscall::SyscallOp::TtyRawRestore,
        crate::syscall::SyscallStatus::Error,
    );
    let invalid_primary_restore = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::TERMINAL_RAW_RESTORE_PRIMARY,
        reovim_uapi_syscall::SyscallArgs::new([1, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(
        invalid_primary_restore.decode(),
        Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT),
    );
    assert_syscall_record(
        "/bin/raw-test",
        crate::syscall::SyscallOp::TtyRawRestorePrimary,
        crate::syscall::SyscallStatus::Error,
    );

    let write_regular_file = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_fs::OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            reovim_uapi_fs::OpenFlags::WRITE_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(
        write_regular_file.decode(),
        Err(reovim_uapi_syscall::SyscallError::NOT_WRITABLE),
    );

    let invalid_exit = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXIT,
        reovim_uapi_syscall::SyscallArgs::new([256, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(
        invalid_exit.decode(),
        Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT),
    );
    assert_syscall_record(
        "/bin/raw-test",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Error,
    );

    let exit = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXIT,
        reovim_uapi_syscall::SyscallArgs::new([7, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(exit.decode(), Ok(0usize));
    testrt::check_eq(
        syscalls.take_raw_exit_status(),
        Some(crate::program::ProgramStatus::ExitCode(7)),
    );
    assert_syscall_record(
        "/bin/raw-test",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
    );

    let not_readable = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            1,
            read_buf.as_mut_ptr() as usize,
            read_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(not_readable.decode(), Err(reovim_uapi_syscall::SyscallError::NOT_READABLE));

    let not_writable = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([0, b"nope".as_ptr() as usize, 4, 0, 0, 0]),
    );
    testrt::check_eq(not_writable.decode(), Err(reovim_uapi_syscall::SyscallError::NOT_WRITABLE));

    let bad_fd = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([99, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(bad_fd.decode(), Err(reovim_uapi_syscall::SyscallError::BAD_DESCRIPTOR));

    let invalid_ptr = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([0, 0, 1, 0, 0, 0]),
    );
    testrt::check_eq(
        invalid_ptr.decode(),
        Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT),
    );
    let invalid_cwd_ptr = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::GET_CWD,
        reovim_uapi_syscall::SyscallArgs::new([0, 1, 0, 0, 0, 0]),
    );
    testrt::check_eq(
        invalid_cwd_ptr.decode(),
        Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT),
    );

    let not_seekable = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::LSEEK,
        reovim_uapi_syscall::SyscallArgs::new([
            1,
            0,
            reovim_uapi_fs::SeekWhence::start().raw() as usize,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(not_seekable.decode(), Err(reovim_uapi_syscall::SyscallError::NOT_SEEKABLE));
    let bad_seek = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::LSEEK,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 1,
            0,
            reovim_uapi_fs::SeekWhence::start().raw() as usize,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(bad_seek.decode(), Err(reovim_uapi_syscall::SyscallError::BAD_DESCRIPTOR));
    let invalid_seek = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::LSEEK,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD + 1,
            0,
            99,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(
        invalid_seek.decode(),
        Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT),
    );
    let mut no_current_session = RootShellSession::new();
    let mut no_current =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut no_current_session, None);
    let no_pid = no_current.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::GET_PID,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(no_pid.decode(), Err(reovim_uapi_syscall::SyscallError::NO_CURRENT_PROCESS));
    let no_cwd = no_current.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::GET_CWD,
        reovim_uapi_syscall::SyscallArgs::new([
            cwd_buf.as_mut_ptr() as usize,
            cwd_buf.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(no_cwd.decode(), Err(reovim_uapi_syscall::SyscallError::NO_CURRENT_PROCESS));
    let no_exit = no_current.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXIT,
        reovim_uapi_syscall::SyscallArgs::new([0, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(no_exit.decode(), Err(reovim_uapi_syscall::SyscallError::NO_CURRENT_PROCESS));
    let no_open = no_current.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_fs::OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            reovim_uapi_fs::OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(no_open.decode(), Err(reovim_uapi_syscall::SyscallError::NO_CURRENT_PROCESS));
    let mut no_current_pipe_fds = [-1i32; 2];
    let no_pipe = no_current.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            no_current_pipe_fds.as_mut_ptr() as usize,
            no_current_pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(no_pipe.decode(), Err(reovim_uapi_syscall::SyscallError::NO_CURRENT_PROCESS));
    let no_terminal_raw = no_current.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::TERMINAL_RAW_ENTER,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_terminal::PRIMARY_INPUT.raw() as usize,
            0,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(
        no_terminal_raw.decode(),
        Err(reovim_uapi_syscall::SyscallError::NO_CURRENT_PROCESS),
    );
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::TtyRawEnter,
        crate::syscall::SyscallStatus::Unavailable,
    );
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Unavailable,
    );
});

arch_test!(program_pipe_status_flags_are_shared_by_duplicates, {
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let current = crate::syscall::SyscallContext {
        pid: 42,
        task_id: 7,
        program_path: "/bin/status-flags",
        loader: "linked-bin",
        entry_name: "status_flags",
    };
    let mut syscalls = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current));

    let (read_fd, _write_fd) = syscalls.pipe_fds().expect("pipe descriptors");
    let read_dup = syscalls.duplicate_fd(read_fd).expect("read end duplicate");
    testrt::check_eq(syscalls.status_flags(read_fd), Ok(FileStatusFlags::EMPTY));
    testrt::check_eq(syscalls.status_flags(read_dup), Ok(FileStatusFlags::EMPTY));

    testrt::check_eq(syscalls.set_status_flags(read_fd, FileStatusFlags::NONBLOCK), Ok(()));
    testrt::check_eq(syscalls.status_flags(read_dup), Ok(FileStatusFlags::NONBLOCK));

    testrt::check_eq(syscalls.set_status_flags(read_dup, FileStatusFlags::EMPTY), Ok(()));
    testrt::check_eq(syscalls.status_flags(read_fd), Ok(FileStatusFlags::EMPTY));
    assert_syscall_record(
        "/bin/status-flags",
        crate::syscall::SyscallOp::FdStatusGet,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/status-flags",
        crate::syscall::SyscallOp::FdStatusSet,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_pipe_read_empty_live_nonblocking_returns_would_block, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid);

    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let set_nonblock = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_STATUS_SET,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            FileStatusFlags::NONBLOCK.raw() as usize,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(set_nonblock.decode(), Ok(0usize));

    let mut pipe_buf = [0u8; 4];
    let empty_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::WOULD_BLOCK));

    let process = crate::proc::process(current_ctx.pid).expect("process retained");
    testrt::check_eq(process.state, crate::proc::ProcessState::Running);
    testrt::check_eq(process.block_reason, crate::sched::BlockReason::None);
    let task = crate::sched::task(current_ctx.task_id).expect("task retained");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Running);
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::None);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdStatusSet,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Error,
    );
});

arch_test!(program_pipe_write_partial_and_full_nonblocking_returns_would_block, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid);

    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let large = [b'p'; crate::program::MAX_PROGRAM_PIPE_BYTES + 3];
    let partial_write = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            large.as_ptr() as usize,
            large.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(partial_write.decode(), Ok(crate::program::MAX_PROGRAM_PIPE_BYTES));

    let set_write_nonblock = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_STATUS_SET,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            FileStatusFlags::NONBLOCK.raw() as usize,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(set_write_nonblock.decode(), Ok(0usize));

    let full_write = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            b"x".as_ptr() as usize,
            1,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(full_write.decode(), Err(reovim_uapi_syscall::SyscallError::WOULD_BLOCK));

    let process = crate::proc::process(current_ctx.pid).expect("process retained");
    testrt::check_eq(process.state, crate::proc::ProcessState::Running);
    testrt::check_eq(process.block_reason, crate::sched::BlockReason::None);
    let task = crate::sched::task(current_ctx.task_id).expect("task retained");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Running);
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::None);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 0usize);

    let mut drain = [0u8; crate::program::MAX_PROGRAM_PIPE_BYTES];
    let read_full_pipe = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            drain.as_mut_ptr() as usize,
            drain.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(read_full_pipe.decode(), Ok(crate::program::MAX_PROGRAM_PIPE_BYTES));
    testrt::check_eq(&drain[..], &large[..crate::program::MAX_PROGRAM_PIPE_BYTES]);

    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdStatusSet,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Error,
    );
});

arch_test!(program_pipe_write_full_live_pipe_blocks_and_replays_after_read, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let writer_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(writer_ctx));
    let _ = crate::syscall::take_pending_exec(writer_ctx.pid);

    let mut writer_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(writer_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let fill = [b'f'; crate::program::MAX_PROGRAM_PIPE_BYTES];
    let fill_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            fill.as_ptr() as usize,
            fill.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(fill_write.decode(), Ok(crate::program::MAX_PROGRAM_PIPE_BYTES));

    let extra = [b'X'; 1];
    let blocked_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            extra.as_ptr() as usize,
            extra.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(blocked_write.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(writer_syscalls);

    let writer = crate::proc::process(writer_ctx.pid).expect("blocked writer retained");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::PipeWrite);
    let writer_task = crate::sched::task(writer_ctx.task_id).expect("blocked writer task retained");
    testrt::check_eq(writer_task.state, crate::sched::KernelTaskState::Blocked);
    testrt::check_eq(writer_task.block_reason, crate::sched::BlockReason::PipeWrite);
    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        1usize,
    );

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::WRITE);
    testrt::check_eq(continuations[0].op, crate::syscall::SyscallOp::FdWrite);
    testrt::check_eq(
        continuations[0].memory,
        crate::syscall::SyscallContinuationMemory::RawWriteBuffer,
    );
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 1usize);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Blocked,
    );

    let reader_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("hello"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            writer_ctx.pid,
            pipe_fds[0] as usize,
            reader_ctx.pid,
            3,
        ),
        Ok(()),
    );
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);
    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let mut read_one = [0u8; 1];
    let read = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            3,
            read_one.as_mut_ptr() as usize,
            read_one.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(read.decode(), Ok(1usize));
    testrt::check_eq(read_one, [b'f']);
    drop(reader_syscalls);

    let writer = crate::proc::process(writer_ctx.pid).expect("writer wakes after read");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::None);
    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        0usize,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(writer_ctx));
    let replayed = crate::syscall::resume_syscall_continuation(&daemon, &mut session, writer_ctx)
        .expect("woken writer has retained write continuation");
    testrt::check_eq(replayed.decode(), Ok(1usize));
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    let writer = crate::proc::process(writer_ctx.pid).expect("writer retained after replay");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::UserResume);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_pipe_read_empty_live_pipe_blocks_current_process, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid);

    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));
    testrt::check_eq(pipe_fds, [3, 4]);

    let mut pipe_buf = [0u8; 4];
    let empty_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));

    let process = crate::proc::process(current_ctx.pid).expect("blocked process retained");
    testrt::check_eq(process.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(process.block_reason, crate::sched::BlockReason::PipeRead);
    let task = crate::sched::task(current_ctx.task_id).expect("blocked task retained");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Blocked);
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::PipeRead);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Blocked,
    );
    assert_event_record(
        "syscall",
        "info",
        "syscall-continue-blocked",
        current_ctx.pid,
        current_ctx.task_id,
    );
    drop(syscalls);

    let writer_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("hello"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            current_ctx.pid,
            pipe_fds[1] as usize,
            writer_ctx.pid,
            1,
        ),
        Ok(()),
    );
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(writer_ctx));
    let _ = crate::syscall::take_pending_exec(writer_ctx.pid);

    let mut writer_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(writer_ctx));
    let pipe_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([1, b"wake".as_ptr() as usize, 4, 0, 0, 0]),
    );
    testrt::check_eq(pipe_write.decode(), Ok(4usize));
    drop(writer_syscalls);

    let process = crate::proc::process(current_ctx.pid).expect("woken process retained");
    testrt::check_eq(process.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(process.block_reason, crate::sched::BlockReason::None);
    let task = crate::sched::task(current_ctx.task_id).expect("woken task retained");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Ready);
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::None);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let process = crate::proc::process(current_ctx.pid).expect("redispatched reader retained");
    testrt::check_eq(process.state, crate::proc::ProcessState::Running);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    let read_after_wake =
        crate::syscall::resume_syscall_continuation(&daemon, &mut session, current_ctx)
            .expect("woken pipe reader has retained read continuation");
    testrt::check_eq(read_after_wake.decode(), Ok(4usize));
    testrt::check_eq(&pipe_buf, b"wake");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_event_record(
        "syscall",
        "info",
        "syscall-continue-ok",
        current_ctx.pid,
        current_ctx.task_id,
    );
    testrt::check(
        crate::syscall::resume_syscall_continuation(&daemon, &mut session, current_ctx).is_none(),
        "read continuation is consumed after successful replay",
    );
});

arch_test!(program_pipe_last_writer_close_wakes_blocked_reader_with_eof, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let reader_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);

    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let mut pipe_buf = [0u8; 4];
    let empty_read = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(reader_syscalls);

    let reader = crate::proc::process(reader_ctx.pid).expect("blocked reader retained");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::PipeRead);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        1usize,
    );
    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, reader_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::READ);

    testrt::check_eq(
        crate::syscall::close_process_fd_for_pipeline(reader_ctx.pid, pipe_fds[1] as usize),
        Ok(()),
    );
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        0usize,
    );
    let reader = crate::proc::process(reader_ctx.pid).expect("reader wakes after writer close");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::None);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let eof = crate::syscall::resume_syscall_continuation(&daemon, &mut session, reader_ctx)
        .expect("woken reader has retained read continuation");
    testrt::check_eq(eof.decode(), Ok(0usize));
    testrt::check_eq(pipe_buf, [0u8; 4]);
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    assert_user_resume_blocked(reader_ctx);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_pipe_last_reader_close_wakes_blocked_writer_for_error_replay, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let writer_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(writer_ctx));
    let _ = crate::syscall::take_pending_exec(writer_ctx.pid);

    let mut writer_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(writer_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let fill = [b'f'; crate::program::MAX_PROGRAM_PIPE_BYTES];
    let fill_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            fill.as_ptr() as usize,
            fill.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(fill_write.decode(), Ok(crate::program::MAX_PROGRAM_PIPE_BYTES));

    let extra = [b'X'; 1];
    let blocked_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            extra.as_ptr() as usize,
            extra.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(blocked_write.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(writer_syscalls);

    let writer = crate::proc::process(writer_ctx.pid).expect("blocked writer retained");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::PipeWrite);
    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        1usize,
    );
    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, writer_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::WRITE);

    testrt::check_eq(
        crate::syscall::close_process_fd_for_pipeline(writer_ctx.pid, pipe_fds[0] as usize),
        Ok(()),
    );
    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        0usize,
    );
    let writer = crate::proc::process(writer_ctx.pid).expect("writer wakes after reader close");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::None);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    let status = daemon.run_idle_ready_programs(&mut session);
    testrt::check_eq(status, ProgramStatus::Error);
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    let writer = crate::proc::process(writer_ctx.pid).expect("writer retained after failed replay");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(writer.exit_code, 1);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );
});

arch_test!(program_pipe_writer_duplicate_close_waits_for_last_writer_before_eof, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let reader_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);

    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));
    let write_dup = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUP,
        reovim_uapi_syscall::SyscallArgs::new([pipe_fds[1] as usize, 0, 0, 0, 0, 0]),
    );
    let write_dup_fd = write_dup.decode().expect("write endpoint duplicates");

    let mut pipe_buf = [0u8; 4];
    let empty_read = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(reader_syscalls);

    testrt::check_eq(
        crate::syscall::close_process_fd_for_pipeline(reader_ctx.pid, pipe_fds[1] as usize),
        Ok(()),
    );
    let reader = crate::proc::process(reader_ctx.pid).expect("reader remains retained");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::PipeRead);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        1usize,
    );
    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);

    testrt::check_eq(
        crate::syscall::close_process_fd_for_pipeline(reader_ctx.pid, write_dup_fd),
        Ok(()),
    );
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        0usize,
    );
    let reader = crate::proc::process(reader_ctx.pid).expect("reader wakes after last writer");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::None);

    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let eof = crate::syscall::resume_syscall_continuation(&daemon, &mut session, reader_ctx)
        .expect("reader replay after final writer close");
    testrt::check_eq(eof.decode(), Ok(0usize));
    testrt::check_eq(pipe_buf, [0u8; 4]);
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    assert_user_resume_blocked(reader_ctx);
});

arch_test!(program_pipe_reader_duplicate_close_waits_for_last_reader_before_error, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let writer_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(writer_ctx));
    let _ = crate::syscall::take_pending_exec(writer_ctx.pid);

    let mut writer_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(writer_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));
    let read_dup = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUP,
        reovim_uapi_syscall::SyscallArgs::new([pipe_fds[0] as usize, 0, 0, 0, 0, 0]),
    );
    let read_dup_fd = read_dup.decode().expect("read endpoint duplicates");

    let fill = [b'f'; crate::program::MAX_PROGRAM_PIPE_BYTES];
    let fill_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            fill.as_ptr() as usize,
            fill.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(fill_write.decode(), Ok(crate::program::MAX_PROGRAM_PIPE_BYTES));

    let extra = [b'X'; 1];
    let blocked_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            extra.as_ptr() as usize,
            extra.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(blocked_write.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(writer_syscalls);

    testrt::check_eq(
        crate::syscall::close_process_fd_for_pipeline(writer_ctx.pid, pipe_fds[0] as usize),
        Ok(()),
    );
    let writer = crate::proc::process(writer_ctx.pid).expect("writer remains retained");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::PipeWrite);
    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        1usize,
    );
    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);

    testrt::check_eq(
        crate::syscall::close_process_fd_for_pipeline(writer_ctx.pid, read_dup_fd),
        Ok(()),
    );
    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        0usize,
    );
    let writer = crate::proc::process(writer_ctx.pid).expect("writer wakes after last reader");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::None);

    let status = daemon.run_idle_ready_programs(&mut session);
    testrt::check_eq(status, ProgramStatus::Error);
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    let writer = crate::proc::process(writer_ctx.pid).expect("writer retained after failed replay");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(writer.exit_code, 1);
});

arch_test!(program_process_exit_closes_last_writer_and_wakes_reader_with_eof, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon_with_programs(&CLOEXEC_TEST_PROGRAMS, &CLOEXEC_TEST_PROGRAM_SOURCES);
    let mut session = RootShellSession::new();
    let reader_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("reader carrier loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("carrier"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);

    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let mut pipe_buf = [0u8; 4];
    let empty_read = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(reader_syscalls);

    let writer_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("writer carrier loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("carrier"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            reader_ctx.pid,
            pipe_fds[1] as usize,
            writer_ctx.pid,
            1,
        ),
        Ok(()),
    );
    testrt::check_eq(
        crate::syscall::close_process_fd_for_pipeline(reader_ctx.pid, pipe_fds[1] as usize),
        Ok(()),
    );

    let reader = crate::proc::process(reader_ctx.pid).expect("blocked reader retained");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::PipeRead);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        1usize,
    );
    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, reader_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::READ);

    let writer_status = daemon.run_pending_programs_until(&mut session, writer_ctx, None);
    testrt::check_eq(writer_status, ProgramStatus::Ok);
    let writer = crate::proc::process(writer_ctx.pid).expect("writer completed");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(writer.exit_code, 0);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        0usize,
    );
    let reader = crate::proc::process(reader_ctx.pid).expect("reader wakes on writer exit");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::None);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let eof = crate::syscall::resume_syscall_continuation(&daemon, &mut session, reader_ctx)
        .expect("reader replay after writer process exit");
    testrt::check_eq(eof.decode(), Ok(0usize));
    testrt::check_eq(pipe_buf, [0u8; 4]);
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    assert_user_resume_blocked(reader_ctx);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_process_exit_closes_last_reader_and_wakes_writer_error, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon_with_programs(&CLOEXEC_TEST_PROGRAMS, &CLOEXEC_TEST_PROGRAM_SOURCES);
    let mut session = RootShellSession::new();
    let writer_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("writer carrier loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("carrier"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(writer_ctx));
    let _ = crate::syscall::take_pending_exec(writer_ctx.pid);

    let mut writer_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(writer_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let reader_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("reader carrier loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("carrier"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            writer_ctx.pid,
            pipe_fds[0] as usize,
            reader_ctx.pid,
            0,
        ),
        Ok(()),
    );
    testrt::check_eq(
        crate::syscall::close_process_fd_for_pipeline(writer_ctx.pid, pipe_fds[0] as usize),
        Ok(()),
    );

    let fill = [b'f'; crate::program::MAX_PROGRAM_PIPE_BYTES];
    let fill_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            fill.as_ptr() as usize,
            fill.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(fill_write.decode(), Ok(crate::program::MAX_PROGRAM_PIPE_BYTES));

    let extra = [b'X'; 1];
    let blocked_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            extra.as_ptr() as usize,
            extra.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(blocked_write.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(writer_syscalls);

    let writer = crate::proc::process(writer_ctx.pid).expect("blocked writer retained");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::PipeWrite);
    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        1usize,
    );
    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, writer_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::WRITE);

    let reader_status = daemon.run_pending_programs_until(&mut session, reader_ctx, None);
    testrt::check_eq(reader_status, ProgramStatus::Ok);
    let reader = crate::proc::process(reader_ctx.pid).expect("reader completed");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(reader.exit_code, 0);

    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        0usize,
    );
    let writer = crate::proc::process(writer_ctx.pid).expect("writer wakes on reader exit");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::None);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    let status = daemon.run_idle_ready_programs(&mut session);
    testrt::check_eq(status, ProgramStatus::Error);
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    let writer = crate::proc::process(writer_ctx.pid).expect("writer retained after failed replay");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(writer.exit_code, 1);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );
});

arch_test!(program_dup_to_replaces_last_writer_and_wakes_reader_with_eof, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon_with_programs(&CLOEXEC_TEST_PROGRAMS, &CLOEXEC_TEST_PROGRAM_SOURCES);
    let mut session = RootShellSession::new();
    let reader_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("reader carrier loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("carrier"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);

    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let mut pipe_buf = [0u8; 4];
    let empty_read = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(reader_syscalls);

    let writer_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("writer carrier loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("carrier"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            reader_ctx.pid,
            pipe_fds[1] as usize,
            writer_ctx.pid,
            1,
        ),
        Ok(()),
    );
    testrt::check_eq(
        crate::syscall::close_process_fd_for_pipeline(reader_ctx.pid, pipe_fds[1] as usize),
        Ok(()),
    );

    let reader = crate::proc::process(reader_ctx.pid).expect("blocked reader retained");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::PipeRead);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        1usize,
    );
    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);

    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(writer_ctx));
    let _ = crate::syscall::take_pending_exec(writer_ctx.pid);
    let mut writer_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(writer_ctx));
    let dup_to = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUP_TO,
        reovim_uapi_syscall::SyscallArgs::new([0, 1, 0, 0, 0, 0]),
    );
    testrt::check_eq(dup_to.decode(), Ok(1usize));
    drop(writer_syscalls);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::FdDuplicate,
        crate::syscall::SyscallStatus::Ok,
    );

    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        0usize,
    );
    let reader = crate::proc::process(reader_ctx.pid).expect("reader wakes on dup-to close");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::None);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let eof = crate::syscall::resume_syscall_continuation(&daemon, &mut session, reader_ctx)
        .expect("reader replay after dup-to closes last writer");
    testrt::check_eq(eof.decode(), Ok(0usize));
    testrt::check_eq(pipe_buf, [0u8; 4]);
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    assert_user_resume_blocked(reader_ctx);
});

arch_test!(program_dup_to_replaces_last_reader_and_wakes_writer_error, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon_with_programs(&CLOEXEC_TEST_PROGRAMS, &CLOEXEC_TEST_PROGRAM_SOURCES);
    let mut session = RootShellSession::new();
    let writer_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("writer carrier loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("carrier"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(writer_ctx));
    let _ = crate::syscall::take_pending_exec(writer_ctx.pid);

    let mut writer_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(writer_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let reader_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("reader carrier loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("carrier"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            writer_ctx.pid,
            pipe_fds[0] as usize,
            reader_ctx.pid,
            0,
        ),
        Ok(()),
    );
    testrt::check_eq(
        crate::syscall::close_process_fd_for_pipeline(writer_ctx.pid, pipe_fds[0] as usize),
        Ok(()),
    );

    let fill = [b'f'; crate::program::MAX_PROGRAM_PIPE_BYTES];
    let fill_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            fill.as_ptr() as usize,
            fill.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(fill_write.decode(), Ok(crate::program::MAX_PROGRAM_PIPE_BYTES));

    let extra = [b'X'; 1];
    let blocked_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            extra.as_ptr() as usize,
            extra.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(blocked_write.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(writer_syscalls);

    let writer = crate::proc::process(writer_ctx.pid).expect("blocked writer retained");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::PipeWrite);
    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        1usize,
    );
    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);

    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);
    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let dup_to = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUP_TO,
        reovim_uapi_syscall::SyscallArgs::new([1, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(dup_to.decode(), Ok(0usize));
    drop(reader_syscalls);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::FdDuplicate,
        crate::syscall::SyscallStatus::Ok,
    );

    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        0usize,
    );
    let writer = crate::proc::process(writer_ctx.pid).expect("writer wakes on dup-to close");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::None);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    let status = daemon.run_idle_ready_programs(&mut session);
    testrt::check_eq(status, ProgramStatus::Error);
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    let writer = crate::proc::process(writer_ctx.pid).expect("writer retained after failed replay");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(writer.exit_code, 1);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );
});

arch_test!(program_pipe_read_waiter_full_fails_without_continuation, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid);

    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));
    testrt::check_eq(
        crate::syscall::fill_process_pipe_read_waiters_for_tests(
            current_ctx.pid,
            pipe_fds[0] as usize,
            10_000,
        ),
        Ok(crate::proc::MAX_PROCESSES),
    );

    let mut pipe_buf = [0u8; 4];
    let empty_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(syscalls);

    let process = crate::proc::process(current_ctx.pid).expect("process retained");
    testrt::check_eq(process.state, crate::proc::ProcessState::Running);
    testrt::check_eq(process.block_reason, crate::sched::BlockReason::None);
    let task = crate::sched::task(current_ctx.task_id).expect("task retained");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Running);
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::None);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(current_ctx.pid),
        0usize,
    );

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Error,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Blocked,
        ),
        0usize,
    );
});

arch_test!(continuation_replay_fails_closed_when_user_resume_block_cannot_be_retained, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid);

    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let mut pipe_buf = [0u8; 4];
    let empty_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(syscalls);

    let writer_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("hello"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            current_ctx.pid,
            pipe_fds[1] as usize,
            writer_ctx.pid,
            1,
        ),
        Ok(()),
    );
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(writer_ctx));
    let _ = crate::syscall::take_pending_exec(writer_ctx.pid);

    let mut writer_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(writer_ctx));
    let pipe_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([1, b"wake".as_ptr() as usize, 4, 0, 0, 0]),
    );
    testrt::check_eq(pipe_write.decode(), Ok(4usize));
    drop(writer_syscalls);

    let process = crate::proc::process(current_ctx.pid).expect("woken process retained");
    testrt::check_eq(process.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(process.block_reason, crate::sched::BlockReason::None);
    let task = crate::sched::task(current_ctx.task_id).expect("woken task retained");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Ready);
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::None);

    crate::sched::reset_kernel_scheduler();
    testrt::check(
        crate::sched::task(current_ctx.task_id).is_none(),
        "test setup removed selected task before post-replay user-resume block",
    );

    let read_after_wake =
        crate::syscall::resume_syscall_continuation(&daemon, &mut session, current_ctx)
            .expect("woken pipe reader has retained read continuation");
    testrt::check_eq(read_after_wake.decode(), Err(reovim_uapi_syscall::SyscallError::IO));
    testrt::check_eq(&pipe_buf, b"wake");

    let process = crate::proc::process(current_ctx.pid).expect("reader process retained");
    testrt::check_eq(process.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(process.block_reason, crate::sched::BlockReason::None);
    testrt::check_eq(crate::sched::task(current_ctx.task_id), None);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Ok,
        ),
        0usize,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Error,
    );
    assert_event_record(
        "syscall",
        "warn",
        "syscall-continue-error",
        current_ctx.pid,
        current_ctx.task_id,
    );
});

arch_test!(yield_now_replays_selected_retained_pipe_read_continuation, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let reader_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);

    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let mut pipe_buf = [0u8; 4];
    let empty_read = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(reader_syscalls);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, reader_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::READ);

    let yielder_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "sched",
    )
    .expect("sched loads");
    let yielder_ctx = crate::syscall::exec_bin_from_shell(yielder_program, argv1("sched"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            reader_ctx.pid,
            pipe_fds[1] as usize,
            yielder_ctx.pid,
            1,
        ),
        Ok(()),
    );
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(yielder_ctx));
    let _ = crate::syscall::take_pending_exec(yielder_ctx.pid);

    let mut yielder_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(yielder_ctx));
    let pipe_write = yielder_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([1, b"wake".as_ptr() as usize, 4, 0, 0, 0]),
    );
    testrt::check_eq(pipe_write.decode(), Ok(4usize));

    let reader = crate::proc::process(reader_ctx.pid).expect("reader wakes after pipe write");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Ready);
    let yield_ret = yielder_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::YIELD_NOW,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(yield_ret.decode(), Ok(1usize));
    drop(yielder_syscalls);

    testrt::check_eq(&pipe_buf, b"wake");
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    assert_user_resume_blocked(reader_ctx);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );
});

arch_test!(rootd_idle_replay_error_exits_selected_continuation_process, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let reader_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);

    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let mut pipe_buf = [0u8; 4];
    let empty_read = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(reader_syscalls);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, reader_ctx.pid);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        1usize,
    );

    testrt::check_eq(
        crate::syscall::close_process_fd_for_pipeline(reader_ctx.pid, pipe_fds[0] as usize),
        Ok(()),
    );
    crate::proc::wake_process(reader_ctx.pid).expect("test wakes invalid retained reader");

    let status = daemon.run_idle_ready_programs(&mut session);
    testrt::check_eq(status, ProgramStatus::Error);
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        0usize,
    );
    let reader = crate::proc::process(reader_ctx.pid).expect("reader remains retained");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Failed);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );
    assert_event_record(
        "syscall",
        "warn",
        "syscall-continue-error",
        reader_ctx.pid,
        reader_ctx.task_id,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/cat pid=3 task=3 status=error loader=linked-bin entry_fn=bin_cat\n",
    );
});

arch_test!(rootd_foreground_replay_error_for_non_target_continues_to_target, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let reader_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);

    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let mut pipe_buf = [0u8; 4];
    let empty_read = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(reader_syscalls);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, reader_ctx.pid);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        1usize,
    );

    testrt::check_eq(
        crate::syscall::close_process_fd_for_pipeline(reader_ctx.pid, pipe_fds[0] as usize),
        Ok(()),
    );
    crate::proc::wake_process(reader_ctx.pid).expect("test wakes invalid retained reader");

    let target_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "pwd",
    )
    .expect("pwd loads");
    let target_ctx = crate::syscall::exec_bin_from_shell(target_program, argv1("pwd"));

    let status = daemon.run_pending_programs_until(&mut session, target_ctx, None);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "/\n");
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        0usize,
    );
    let reader = crate::proc::process(reader_ctx.pid).expect("reader remains retained");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Failed);
    let target = crate::proc::process(target_ctx.pid).expect("target remains retained");
    testrt::check_eq(target.state, crate::proc::ProcessState::Exited);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/cat pid=3 task=3 status=error loader=linked-bin entry_fn=bin_cat\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=4 task=4 status=ok loader=linked-bin entry_fn=bin_pwd\n",
    );
});

arch_test!(killing_pipe_reader_releases_waiter_and_continuation, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let reader_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);

    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let mut pipe_buf = [0u8; 4];
    let empty_read = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(reader_syscalls);

    let reader = crate::proc::process(reader_ctx.pid).expect("blocked reader retained");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::PipeRead);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        1usize,
    );
    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, reader_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::READ);

    let writer_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("hello"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            reader_ctx.pid,
            pipe_fds[1] as usize,
            writer_ctx.pid,
            1,
        ),
        Ok(()),
    );

    let shell_ctx = crate::syscall::SyscallContext {
        pid: crate::proc::SHELL_PID,
        task_id: crate::proc::SHELL_PID,
        program_path: "/bin/sh",
        loader: "linked-bin",
        entry_name: "bin_sh",
    };
    let mut shell_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(shell_ctx));
    let kill = shell_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROCESS_KILL,
        reovim_uapi_syscall::SyscallArgs::new([reader_ctx.pid, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(kill.decode(), Ok(reader_ctx.pid));
    drop(shell_syscalls);

    let reader = crate::proc::process(reader_ctx.pid).expect("killed reader retained");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::None);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        0usize,
    );
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);

    let reader = crate::proc::process(reader_ctx.pid).expect("reader remains retained");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::None);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        0usize,
    );
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
});

arch_test!(second_blocking_read_keeps_existing_continuation, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let reader_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);

    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let mut first_buf = [0u8; 4];
    let first_ptr = first_buf.as_mut_ptr() as usize;
    let first_read = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            first_ptr,
            first_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(first_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, reader_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::READ);
    testrt::check_eq(continuations[0].args.a1, first_ptr);
    testrt::check_eq(continuations[0].args.a2, first_buf.len());
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        1usize,
    );

    let mut second_buf = [0u8; 4];
    let second_ptr = second_buf.as_mut_ptr() as usize;
    let second_read = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            second_ptr,
            second_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(second_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(reader_syscalls);

    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, reader_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::READ);
    testrt::check_eq(continuations[0].args.a1, first_ptr);
    testrt::check_eq(continuations[0].args.a2, first_buf.len());
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        1usize,
    );

    let reader = crate::proc::process(reader_ctx.pid).expect("blocked reader retained");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::PipeRead);

    let writer_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("hello"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            reader_ctx.pid,
            pipe_fds[1] as usize,
            writer_ctx.pid,
            1,
        ),
        Ok(()),
    );
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(writer_ctx));
    let _ = crate::syscall::take_pending_exec(writer_ctx.pid);
    let mut writer_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(writer_ctx));
    let pipe_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([1, b"root".as_ptr() as usize, 4, 0, 0, 0]),
    );
    testrt::check_eq(pipe_write.decode(), Ok(4usize));
    drop(writer_syscalls);
    crate::syscall::exit_current(writer_ctx, ProgramStatus::Ok);

    testrt::check_eq(daemon.run_idle_ready_programs(&mut session), ProgramStatus::Ok);
    testrt::check_eq(&first_buf, b"root");
    testrt::check_eq(&second_buf, &[0u8; 4]);
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        0usize,
    );
});

arch_test!(rootd_idle_dispatch_replays_ready_pipe_read_continuation, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid);

    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let mut pipe_buf = [0u8; 4];
    let empty_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(syscalls);

    let reader = crate::proc::process(current_ctx.pid).expect("blocked reader retained");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::PipeRead);
    let reader_task =
        crate::sched::task(current_ctx.task_id).expect("blocked reader task retained");
    testrt::check_eq(reader_task.state, crate::sched::KernelTaskState::Blocked);
    testrt::check_eq(reader_task.block_reason, crate::sched::BlockReason::PipeRead);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(current_ctx.pid),
        1usize,
    );
    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 1usize);
    testrt::check_eq(continuations[0].process_id, current_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::READ);
    testrt::check_eq(
        continuations[0].memory,
        crate::syscall::SyscallContinuationMemory::RawReadBuffer,
    );
    testrt::check_eq(continuations[0].args.a1, pipe_buf.as_mut_ptr() as usize);
    testrt::check_eq(continuations[0].args.a2, pipe_buf.len());

    crate::proc::wake_process(current_ctx.pid).expect("test wakes reader before bytes arrive");
    testrt::check_eq(daemon.run_idle_ready_programs(&mut session), ProgramStatus::Ok);
    let reader = crate::proc::process(current_ctx.pid).expect("reader remains retained");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::PipeRead);
    let reader_task =
        crate::sched::task(current_ctx.task_id).expect("reader task remains retained");
    testrt::check_eq(reader_task.state, crate::sched::KernelTaskState::Blocked);
    testrt::check_eq(reader_task.block_reason, crate::sched::BlockReason::PipeRead);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(current_ctx.pid),
        1usize,
    );
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 1usize);
    testrt::check_eq(continuations[0].process_id, current_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::READ);
    testrt::check_eq(
        continuations[0].memory,
        crate::syscall::SyscallContinuationMemory::RawReadBuffer,
    );
    testrt::check_eq(continuations[0].args.a1, pipe_buf.as_mut_ptr() as usize);
    testrt::check_eq(continuations[0].args.a2, pipe_buf.len());
    testrt::check_eq(&pipe_buf, &[0u8; 4]);
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Blocked,
        ),
        3usize,
    );

    {
        let mut inspector = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
        sink_clear();
        inspector
            .write_vfs_file_path("/dump/snapshot")
            .expect("dump snapshot renders retained pipe read");
        let dump = sink_bytes();
        assert_contains(dump, b"syscall_continuation_records=1\n");
        assert_contains(dump, b"processes:\n");
        assert_contains(dump, b"path=/bin/cat");
        assert_contains(dump, b"state=blocked");
        assert_contains(dump, b"block=pipe-read");
        assert_contains(dump, b"continuations:\n");
        assert_contains(dump, b"path=/bin/cat nr=1 op=fd-read");
        assert_contains(dump, b"memory=raw-read-buffer");
        assert_contains(dump, b" a0=3 ");
        assert_contains_prefixed_usize(
            dump,
            b" a1=",
            pipe_buf.as_mut_ptr() as usize,
            "dump snapshot records retained read buffer pointer",
        );
        assert_contains_prefixed_usize(
            dump,
            b" a2=",
            pipe_buf.len(),
            "dump snapshot records retained read length",
        );
        assert_contains(dump, b"loader=linked-bin entry_fn=bin_cat");
        sink_clear();
    }

    let writer_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("hello"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            current_ctx.pid,
            pipe_fds[1] as usize,
            writer_ctx.pid,
            1,
        ),
        Ok(()),
    );
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(writer_ctx));
    let _ = crate::syscall::take_pending_exec(writer_ctx.pid);
    let mut writer_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(writer_ctx));
    let pipe_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([1, b"root".as_ptr() as usize, 4, 0, 0, 0]),
    );
    testrt::check_eq(pipe_write.decode(), Ok(4usize));
    drop(writer_syscalls);
    crate::syscall::exit_current(writer_ctx, ProgramStatus::Ok);

    testrt::check_eq(daemon.run_idle_ready_programs(&mut session), ProgramStatus::Ok);
    testrt::check_eq(&pipe_buf, b"root");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
    );
    testrt::check(
        crate::syscall::resume_syscall_continuation(&daemon, &mut session, current_ctx).is_none(),
        "rootd idle continuation replay consumes retained read frame",
    );
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(current_ctx.pid),
        0usize,
    );
    testrt::check_eq(sink_str(), "");
});

arch_test!(rootd_foreground_dispatch_replays_target_pipe_read_continuation, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid);

    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let mut pipe_buf = [0u8; 4];
    let empty_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(syscalls);

    let writer_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("hello"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            current_ctx.pid,
            pipe_fds[1] as usize,
            writer_ctx.pid,
            1,
        ),
        Ok(()),
    );
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(writer_ctx));
    let _ = crate::syscall::take_pending_exec(writer_ctx.pid);
    let mut writer_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(writer_ctx));
    let pipe_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([1, b"fgnd".as_ptr() as usize, 4, 0, 0, 0]),
    );
    testrt::check_eq(pipe_write.decode(), Ok(4usize));
    drop(writer_syscalls);
    crate::syscall::exit_current(writer_ctx, ProgramStatus::Ok);

    testrt::check_eq(
        daemon.run_pending_programs_until(&mut session, current_ctx, None),
        ProgramStatus::Blocked,
    );
    testrt::check_eq(&pipe_buf, b"fgnd");
    assert_user_resume_blocked(current_ctx);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
    );
    testrt::check(
        crate::syscall::resume_syscall_continuation(&daemon, &mut session, current_ctx).is_none(),
        "rootd foreground continuation replay consumes retained read frame",
    );
    testrt::check_eq(sink_str(), "");
});

arch_test!(rootd_idle_dispatch_replays_ready_wait_continuation, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let parent_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let parent_ctx = crate::syscall::exec_bin_from_shell(parent_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(parent_ctx));
    let _ = crate::syscall::take_pending_exec(parent_ctx.pid);

    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(parent_ctx));
    let argv = [reovim_uapi_process::ProcessArg::from_str("pwd")];
    let spawn = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SPAWN,
        reovim_uapi_syscall::SyscallArgs::new([
            argv.as_ptr() as usize,
            argv.len(),
            reovim_uapi_process::ProcessSpawnTarget::BIN.raw(),
            reovim_uapi_process::ProcessSpawnMode::BLOCKED.raw(),
            0,
            0,
        ]),
    );
    let child_pid = spawn.decode().expect("blocked spawn returns child pid");
    let child = crate::proc::process(child_pid).expect("blocked child retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Blocked);

    let mut wait_report = reovim_uapi_process::ProcessWaitReport::empty();
    let wait = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WAIT,
        reovim_uapi_syscall::SyscallArgs::new([
            child_pid,
            (&mut wait_report as *mut reovim_uapi_process::ProcessWaitReport) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessWaitReport>(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(wait.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(syscalls);

    let parent = crate::proc::process(parent_ctx.pid).expect("waiting parent retained");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(parent.block_reason, crate::sched::BlockReason::WaitChild);
    let parent_task = crate::sched::task(parent_ctx.task_id).expect("waiting task retained");
    testrt::check_eq(parent_task.state, crate::sched::KernelTaskState::Blocked);
    testrt::check_eq(parent_task.block_reason, crate::sched::BlockReason::WaitChild);
    let retained_wait =
        crate::proc::wait_record(parent_ctx.pid, child_pid).expect("wait record retained");
    testrt::check(!retained_wait.completed, "blocked wait is not completed yet");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Blocked,
    );

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 1usize);
    testrt::check_eq(continuations[0].process_id, parent_ctx.pid);
    testrt::check_eq(continuations[0].task_id, parent_ctx.task_id);
    testrt::check_eq(continuations[0].program_path, "/bin/cat");
    testrt::check_eq(continuations[0].loader, "linked-bin");
    testrt::check_eq(continuations[0].entry_name, "bin_cat");
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::WAIT);
    testrt::check_eq(continuations[0].op, crate::syscall::SyscallOp::WaitBegin);
    testrt::check_eq(
        continuations[0].memory,
        crate::syscall::SyscallContinuationMemory::RawWaitReport,
    );
    testrt::check_eq(continuations[0].args.a0, child_pid);

    {
        let mut inspector = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
        sink_clear();
        inspector
            .write_vfs_file_path("/proc/continuations")
            .expect("proc continuation diagnostics render");
        assert_contains(sink_bytes(), b"continuations:\n");
        assert_contains(sink_bytes(), b"path=/bin/cat nr=14 op=wait-begin");
        assert_contains(sink_bytes(), b"memory=raw-wait-report");
        assert_contains(sink_bytes(), b"loader=linked-bin entry_fn=bin_cat\n");

        sink_clear();
        inspector
            .write_vfs_file_path("/dump/status")
            .expect("dump status renders continuation count");
        assert_contains(sink_bytes(), b"syscall_continuation_records=1\n");

        sink_clear();
        inspector
            .write_vfs_file_path("/dump/snapshot")
            .expect("dump snapshot renders continuation table");
        assert_contains(sink_bytes(), b"continuations:\n");
        assert_contains(sink_bytes(), b"path=/bin/cat nr=14 op=wait-begin");
    }

    let waker_ctx = crate::syscall::SyscallContext {
        pid: crate::proc::SHELL_PID,
        task_id: crate::proc::SHELL_PID,
        program_path: "/bin/sh",
        loader: "linked-bin",
        entry_name: "bin_sh",
    };
    let mut waker = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(waker_ctx));
    let mut wake_report = reovim_uapi_process::ProcessControlReport::empty();
    let wake = waker.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROCESS_WAKE,
        reovim_uapi_syscall::SyscallArgs::new([
            parent_ctx.pid,
            (&mut wake_report as *mut reovim_uapi_process::ProcessControlReport) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessControlReport>(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(wake.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    testrt::check_eq(wake_report.pid().raw(), 0usize);
    let parent = crate::proc::process(parent_ctx.pid).expect("waiting parent remains retained");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(parent.block_reason, crate::sched::BlockReason::WaitChild);
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 1usize);
    testrt::check_eq(continuations[0].process_id, parent_ctx.pid);
    assert_syscall_record(
        "/bin/sh",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Error,
    );
    drop(waker);

    crate::proc::wake_process(parent_ctx.pid).expect("test wakes parent before child is ready");
    testrt::check_eq(daemon.run_idle_ready_programs(&mut session), ProgramStatus::Ok);
    let parent =
        crate::proc::process(parent_ctx.pid).expect("parent remains retained after busy replay");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(parent.block_reason, crate::sched::BlockReason::WaitChild);
    let parent_task = crate::sched::task(parent_ctx.task_id).expect("parent task remains retained");
    testrt::check_eq(parent_task.state, crate::sched::KernelTaskState::Blocked);
    testrt::check_eq(parent_task.block_reason, crate::sched::BlockReason::WaitChild);
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 1usize);
    testrt::check_eq(continuations[0].process_id, parent_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::WAIT);
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Blocked,
        ),
        3usize,
    );

    sink_clear();
    crate::proc::wake_process(child_pid).expect("blocked child wakes");
    testrt::check_eq(daemon.run_idle_ready_programs(&mut session), ProgramStatus::Ok);

    testrt::check_eq(sink_str(), "/\n");
    testrt::check_eq(wait_report.child_pid(), reovim_uapi_process::ProcessId::new(child_pid));
    testrt::check_eq(wait_report.child_state(), reovim_uapi_process::ProcessStateCode::EXITED);
    testrt::check_eq(wait_report.exit_code(), 0);
    testrt::check(wait_report.completed(), "replayed wait report marks completion");

    let completed_wait =
        crate::proc::wait_record(parent_ctx.pid, child_pid).expect("completed wait retained");
    testrt::check(completed_wait.completed, "wait completed after child exit");
    let child = crate::proc::process(child_pid).expect("waited child retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Reaped);
    assert_user_resume_blocked(parent_ctx);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    testrt::check(
        crate::syscall::resume_syscall_continuation(&daemon, &mut session, parent_ctx).is_none(),
        "rootd idle wait replay consumes retained wait frame",
    );
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
});

arch_test!(killing_waiting_parent_releases_wait_continuation_and_row, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let parent_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let parent_ctx = crate::syscall::exec_bin_from_shell(parent_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(parent_ctx));
    let _ = crate::syscall::take_pending_exec(parent_ctx.pid);

    let mut parent_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(parent_ctx));
    let argv = [reovim_uapi_process::ProcessArg::from_str("pwd")];
    let spawn = parent_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SPAWN,
        reovim_uapi_syscall::SyscallArgs::new([
            argv.as_ptr() as usize,
            argv.len(),
            reovim_uapi_process::ProcessSpawnTarget::BIN.raw(),
            reovim_uapi_process::ProcessSpawnMode::BLOCKED.raw(),
            0,
            0,
        ]),
    );
    let child_pid = spawn.decode().expect("blocked spawn returns child pid");
    let mut wait_report = reovim_uapi_process::ProcessWaitReport::empty();
    let wait = parent_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WAIT,
        reovim_uapi_syscall::SyscallArgs::new([
            child_pid,
            (&mut wait_report as *mut reovim_uapi_process::ProcessWaitReport) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessWaitReport>(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(wait.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(parent_syscalls);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check(
        crate::proc::wait_record(parent_ctx.pid, child_pid).is_some(),
        "wait row retained",
    );

    let shell_ctx = crate::syscall::SyscallContext {
        pid: crate::proc::SHELL_PID,
        task_id: crate::proc::SHELL_PID,
        program_path: "/bin/sh",
        loader: "linked-bin",
        entry_name: "bin_sh",
    };
    let mut shell_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(shell_ctx));
    let kill = shell_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROCESS_KILL,
        reovim_uapi_syscall::SyscallArgs::new([parent_ctx.pid, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(kill.decode(), Ok(parent_ctx.pid));
    drop(shell_syscalls);

    let parent = crate::proc::process(parent_ctx.pid).expect("killed parent retained");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(parent.block_reason, crate::sched::BlockReason::None);
    let child = crate::proc::process(child_pid).expect("child retained after parent kill");
    testrt::check_eq(child.parent_pid, crate::proc::ROOTD_PID);
    testrt::check_eq(child.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(child.block_reason, crate::sched::BlockReason::Operator);

    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    testrt::check(
        crate::proc::wait_record(parent_ctx.pid, child_pid).is_none(),
        "dead parent wait row is canceled",
    );
    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    testrt::check_eq(crate::proc::snapshot_waits(&mut waits), 0usize);

    sink_clear();
    crate::proc::wake_process(child_pid).expect("adopted child wakes");
    testrt::check_eq(daemon.run_idle_ready_programs(&mut session), ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "/\n");
    testrt::check_eq(crate::proc::snapshot_waits(&mut waits), 0usize);
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
});

arch_test!(rootd_idle_dispatch_replays_ready_wait_ready_continuation, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let parent_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let parent_ctx = crate::syscall::exec_bin_from_shell(parent_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(parent_ctx));
    let _ = crate::syscall::take_pending_exec(parent_ctx.pid);

    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(parent_ctx));
    let argv = [reovim_uapi_process::ProcessArg::from_str("pwd")];
    let spawn = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SPAWN,
        reovim_uapi_syscall::SyscallArgs::new([
            argv.as_ptr() as usize,
            argv.len(),
            reovim_uapi_process::ProcessSpawnTarget::BIN.raw(),
            reovim_uapi_process::ProcessSpawnMode::BLOCKED.raw(),
            0,
            0,
        ]),
    );
    let child_pid = spawn.decode().expect("blocked spawn returns child pid");
    let child = crate::proc::process(child_pid).expect("blocked child retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Blocked);

    let mut wait_report = reovim_uapi_process::ProcessWaitReport::empty();
    wait_report.set_child_pid(reovim_uapi_process::ProcessId::new(child_pid));
    let wait = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROCESS_WAIT_READY,
        reovim_uapi_syscall::SyscallArgs::new([
            child_pid,
            (&mut wait_report as *mut reovim_uapi_process::ProcessWaitReport) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessWaitReport>(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(wait.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(syscalls);

    let parent = crate::proc::process(parent_ctx.pid).expect("waiting parent retained");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(parent.block_reason, crate::sched::BlockReason::WaitChild);
    let retained_wait =
        crate::proc::wait_record(parent_ctx.pid, child_pid).expect("wait-ready record retained");
    testrt::check(!retained_wait.completed, "wait-ready is retained incomplete");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Blocked,
    );

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 1usize);
    testrt::check_eq(continuations[0].process_id, parent_ctx.pid);
    testrt::check_eq(continuations[0].task_id, parent_ctx.task_id);
    testrt::check_eq(continuations[0].program_path, "/bin/cat");
    testrt::check_eq(continuations[0].loader, "linked-bin");
    testrt::check_eq(continuations[0].entry_name, "bin_cat");
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::PROCESS_WAIT_READY);
    testrt::check_eq(continuations[0].op, crate::syscall::SyscallOp::WaitReady);
    testrt::check_eq(
        continuations[0].memory,
        crate::syscall::SyscallContinuationMemory::RawWaitReport,
    );
    testrt::check_eq(continuations[0].args.a0, child_pid);

    {
        let mut inspector = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
        sink_clear();
        inspector
            .write_vfs_file_path("/proc/continuations")
            .expect("proc continuation diagnostics render");
        assert_contains(sink_bytes(), b"continuations:\n");
        assert_contains(sink_bytes(), b"path=/bin/cat nr=28 op=wait-ready");
        assert_contains(sink_bytes(), b"memory=raw-wait-report");
        assert_contains(sink_bytes(), b"loader=linked-bin entry_fn=bin_cat\n");

        sink_clear();
        inspector
            .write_vfs_file_path("/dump/status")
            .expect("dump status renders continuation count");
        assert_contains(sink_bytes(), b"syscall_continuation_records=1\n");

        sink_clear();
        inspector
            .write_vfs_file_path("/dump/snapshot")
            .expect("dump snapshot renders continuation table");
        assert_contains(sink_bytes(), b"continuations:\n");
        assert_contains(sink_bytes(), b"path=/bin/cat nr=28 op=wait-ready");
        assert_contains(sink_bytes(), b"memory=raw-wait-report");
    }

    crate::proc::wake_process(parent_ctx.pid).expect("test wakes wait-ready parent early");
    testrt::check_eq(daemon.run_idle_ready_programs(&mut session), ProgramStatus::Ok);
    let parent = crate::proc::process(parent_ctx.pid).expect("wait-ready parent remains retained");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(parent.block_reason, crate::sched::BlockReason::WaitChild);
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 1usize);
    testrt::check_eq(continuations[0].process_id, parent_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::PROCESS_WAIT_READY);
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Blocked,
        ),
        3usize,
    );

    sink_clear();
    crate::proc::wake_process(child_pid).expect("blocked child wakes");
    testrt::check_eq(daemon.run_idle_ready_programs(&mut session), ProgramStatus::Ok);

    testrt::check_eq(sink_str(), "/\n");
    testrt::check_eq(wait_report.child_pid(), reovim_uapi_process::ProcessId::new(child_pid));
    testrt::check_eq(wait_report.child_state(), reovim_uapi_process::ProcessStateCode::EXITED);
    testrt::check_eq(wait_report.exit_code(), 0);
    testrt::check(wait_report.completed(), "replayed wait-ready report marks completion");

    let completed_wait =
        crate::proc::wait_record(parent_ctx.pid, child_pid).expect("completed wait retained");
    testrt::check(completed_wait.completed, "wait-ready completed after child exit");
    let child = crate::proc::process(child_pid).expect("waited child retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Reaped);
    assert_user_resume_blocked(parent_ctx);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    testrt::check(
        crate::syscall::resume_syscall_continuation(&daemon, &mut session, parent_ctx).is_none(),
        "rootd idle wait-ready replay consumes retained frame",
    );
    let continuation_count = crate::syscall::snapshot_syscall_continuations(&mut continuations);
    testrt::check_eq(continuation_count, 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
});

arch_test!(program_syscalls_dispatches_raw_source_control, {
    crate::syscall::reset();
    reset_installed_sources();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let current = crate::syscall::SyscallContext {
        pid: 42,
        task_id: 7,
        program_path: "/bin/raw-source",
        loader: "linked-bin",
        entry_name: "raw_source",
    };
    let mut syscalls = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current));
    let mut report = reovim_uapi_source::SourceInstallReport::empty();
    let name = "server-smoke";

    let install = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SOURCE_CONTROL,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_source::SourceControlOp::INSTALL_PAYLOAD_STATUS.raw(),
            name.as_ptr() as usize,
            name.len(),
            reovim_uapi_source::SourceInstallStatusCode::READY.raw(),
            (&mut report as *mut reovim_uapi_source::SourceInstallReport) as usize,
            core::mem::size_of::<reovim_uapi_source::SourceInstallReport>(),
        ]),
    );
    testrt::check_eq(install.decode(), Ok(report.bytes_len()));
    testrt::check_eq(report.namespace(), reovim_uapi_source::SourceNamespaceCode::PAYLOAD);
    testrt::check_eq(report.status(), reovim_uapi_source::SourceInstallStatusCode::READY);
    testrt::check_eq(report.origin(), reovim_uapi_source::SourceInstallOriginCode::INSTALLED);
    testrt::check_eq(report.name_bytes(), b"server-smoke");
    testrt::check_eq(report.path_bytes(), b"/payload/server-smoke");
    assert_syscall_record(
        "/bin/raw-source",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Ok,
    );

    let bad_status = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SOURCE_CONTROL,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_source::SourceControlOp::INSTALL_PAYLOAD_STATUS.raw(),
            name.as_ptr() as usize,
            name.len(),
            reovim_uapi_source::SourceInstallStatusCode::OK.raw(),
            (&mut report as *mut reovim_uapi_source::SourceInstallReport) as usize,
            core::mem::size_of::<reovim_uapi_source::SourceInstallReport>(),
        ]),
    );
    testrt::check_eq(bad_status.decode(), Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT));

    let mut no_current_session = RootShellSession::new();
    let mut no_current =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut no_current_session, None);
    let no_current_ret = no_current.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SOURCE_CONTROL,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_source::SourceControlOp::INSTALL_PAYLOAD_STATUS.raw(),
            name.as_ptr() as usize,
            name.len(),
            reovim_uapi_source::SourceInstallStatusCode::READY.raw(),
            (&mut report as *mut reovim_uapi_source::SourceInstallReport) as usize,
            core::mem::size_of::<reovim_uapi_source::SourceInstallReport>(),
        ]),
    );
    testrt::check_eq(
        no_current_ret.decode(),
        Err(reovim_uapi_syscall::SyscallError::NO_CURRENT_PROCESS),
    );
    reset_installed_sources();
});

arch_test!(program_syscalls_dispatches_raw_spawn_and_wait, {
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let current = crate::syscall::SyscallContext {
        pid: crate::proc::SHELL_PID,
        task_id: crate::proc::SHELL_PID,
        program_path: "/bin/sh",
        loader: "source-image",
        entry_name: "bin_sh",
    };
    let mut syscalls = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current));
    let argv = [reovim_uapi_process::ProcessArg::from_str("pwd")];

    let spawn = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SPAWN,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    let child_pid = spawn.decode().expect("raw spawn returns child pid");
    testrt::check_eq(child_pid, 3usize);
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );

    let wait = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WAIT,
        reovim_uapi_syscall::SyscallArgs::new([child_pid, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(wait.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/bin/sh",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );

    let child = crate::proc::process(child_pid).expect("spawned child is retained");
    testrt::check_eq(child.program_path, "/bin/pwd");
    testrt::check_eq(child.state, crate::proc::ProcessState::Reaped);
    let parent = crate::proc::process(crate::proc::SHELL_PID).expect("shell parent retained");
    testrt::check_eq(parent.state, crate::proc::ProcessState::Running);

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    testrt::check_eq(wait_count, 1usize);
    testrt::check_eq(waits[0].parent_pid, crate::proc::SHELL_PID);
    testrt::check_eq(waits[0].child_pid, child_pid);
    testrt::check_eq(waits[0].completed, true);
    testrt::check_eq(waits[0].exit_code, 0);

    let spawn_for_report = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SPAWN,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    let report_child_pid = spawn_for_report
        .decode()
        .expect("raw spawn returns report child pid");
    let mut wait_report = reovim_uapi_process::ProcessWaitReport::empty();
    let wait_with_report = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WAIT,
        reovim_uapi_syscall::SyscallArgs::new([
            report_child_pid,
            (&mut wait_report as *mut reovim_uapi_process::ProcessWaitReport) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessWaitReport>(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(wait_with_report.decode(), Ok(0usize));
    testrt::check_eq(
        wait_report.child_pid(),
        reovim_uapi_process::ProcessId::new(report_child_pid),
    );
    testrt::check_eq(wait_report.child_state(), reovim_uapi_process::ProcessStateCode::EXITED);
    testrt::check_eq(wait_report.exit_code(), 0);
    testrt::check(wait_report.completed(), "raw wait report marks completion");

    drop(syscalls);
    let no_current = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None)
        .dispatch_raw_syscall(
            reovim_uapi_syscall::SyscallNr::SPAWN,
            reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
        );
    testrt::check_eq(
        no_current.decode(),
        Err(reovim_uapi_syscall::SyscallError::NO_CURRENT_PROCESS),
    );
});

arch_test!(program_syscalls_dispatches_raw_spawn_with_env_metadata, {
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let current = crate::syscall::SyscallContext {
        pid: crate::proc::SHELL_PID,
        task_id: crate::proc::SHELL_PID,
        program_path: "/bin/sh",
        loader: "linked-bin",
        entry_name: "bin_sh",
    };
    let mut syscalls = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current));
    let argv = [reovim_uapi_process::ProcessArg::from_str("pwd")];
    let env = [reovim_uapi_process::ProcessEnv::from_pair(
        "REOVIM_MEDIA_PATH",
        "/boot/status",
    )];

    let spawn = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SPAWN,
        reovim_uapi_syscall::SyscallArgs::new([
            argv.as_ptr() as usize,
            argv.len(),
            reovim_uapi_process::ProcessSpawnTarget::BIN
                .with_env()
                .raw(),
            reovim_uapi_process::ProcessSpawnMode::READY.raw(),
            env.as_ptr() as usize,
            env.len(),
        ]),
    );
    let child_pid = spawn.decode().expect("raw env spawn returns child pid");
    let child = crate::proc::process(child_pid).expect("env child is retained");
    testrt::check_eq(child.program_path, "/bin/pwd");
    testrt::check_eq(child.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(child.envc, 1usize);
    testrt::check_eq(child.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(child.env_value(0), "/boot/status");
    testrt::check_eq(child.env_was_truncated(0), false);
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );

    drop(syscalls);
    let mut inspector = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
    sink_clear();
    inspector
        .write_vfs_file_path("/proc/processes")
        .expect("process diagnostics render");
    assert_contains(
        sink_bytes(),
        b"envc=1 env0_name=REOVIM_MEDIA_PATH env0_value=/boot/status env0_truncated=false",
    );
});

arch_test!(program_syscalls_dispatches_raw_process_self_report, {
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("hello"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let mut report = reovim_uapi_process::ProcessControlReport::empty();
    let self_report = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROCESS_SELF,
        reovim_uapi_syscall::SyscallArgs::new([
            (&mut report as *mut reovim_uapi_process::ProcessControlReport) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessControlReport>(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(self_report.decode(), Ok(current_ctx.pid));
    testrt::check_eq(report.pid(), reovim_uapi_process::ProcessId::new(current_ctx.pid));
    testrt::check_eq(report.state(), reovim_uapi_process::ProcessStateCode::RUNNING);
    testrt::check_eq(report.path_bytes(), b"/bin/hello");
    testrt::check_eq(report.loader_bytes(), b"linked-bin");
    testrt::check_eq(report.entry_name_bytes(), b"bin_hello");
    testrt::check_eq(report.body_format_bytes(), b"linked-image");
    testrt::check_eq(report.body_inner_bytes(), b"none");
    testrt::check_eq(report.body_bytes(), 0usize);
    testrt::check_eq(report.body_checksum(), 0u32);
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessSelf,
        crate::syscall::SyscallStatus::Ok,
    );

    let bad_report = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROCESS_SELF,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(bad_report.decode(), Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT));

    drop(syscalls);
    let no_current = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None)
        .dispatch_raw_syscall(
            reovim_uapi_syscall::SyscallNr::PROCESS_SELF,
            reovim_uapi_syscall::SyscallArgs::new([
                (&mut report as *mut reovim_uapi_process::ProcessControlReport) as usize,
                core::mem::size_of::<reovim_uapi_process::ProcessControlReport>(),
                0,
                0,
                0,
                0,
            ]),
        );
    testrt::check_eq(
        no_current.decode(),
        Err(reovim_uapi_syscall::SyscallError::NO_CURRENT_PROCESS),
    );
});

arch_test!(program_syscalls_dispatches_raw_spawn_request_with_env_reports, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let current = crate::syscall::SyscallContext {
        pid: crate::proc::SHELL_PID,
        task_id: crate::proc::SHELL_PID,
        program_path: "/bin/sh",
        loader: "linked-bin",
        entry_name: "bin_sh",
    };
    let mut syscalls = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current));
    let argv = [reovim_uapi_process::ProcessArg::from_str("pwd")];
    let env = [reovim_uapi_process::ProcessEnv::from_pair(
        "REOVIM_MEDIA_PATH",
        "/boot/status",
    )];

    let mut report = reovim_uapi_process::ProcessControlReport::empty();
    let request = reovim_uapi_process::ProcessSpawnRequest::with_process_report(
        reovim_uapi_process::ProcessSpawnTarget::BIN,
        reovim_uapi_process::ProcessSpawnMode::READY,
        &argv,
        &env,
        &mut report,
    );
    let spawn = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SPAWN,
        reovim_uapi_syscall::SyscallArgs::new([
            (&request as *const reovim_uapi_process::ProcessSpawnRequest) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessSpawnRequest>(),
            reovim_uapi_process::ProcessSpawnTarget::BIN
                .with_request()
                .raw(),
            0,
            0,
            0,
        ]),
    );
    let child_pid = spawn.decode().expect("request spawn returns child pid");
    testrt::check_eq(report.pid(), reovim_uapi_process::ProcessId::new(child_pid));
    testrt::check_eq(report.state(), reovim_uapi_process::ProcessStateCode::READY);
    testrt::check_eq(report.path_bytes(), b"/bin/pwd");
    testrt::check_eq(report.body_bytes(), 0usize);
    testrt::check_eq(report.body_checksum(), 0u32);
    let child = crate::proc::process(child_pid).expect("request child retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(child.envc, 1usize);
    testrt::check_eq(child.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(child.env_value(0), "/boot/status");

    let mut sleep_report = reovim_uapi_process::ProcessSleepReport::empty();
    sleep_report.set_requested_ticks(5);
    let sleep_request =
        reovim_uapi_process::ProcessSpawnRequest::with_sleep_report(&argv, &env, &mut sleep_report);
    let sleep_spawn = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SPAWN,
        reovim_uapi_syscall::SyscallArgs::new([
            (&sleep_request as *const reovim_uapi_process::ProcessSpawnRequest) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessSpawnRequest>(),
            reovim_uapi_process::ProcessSpawnTarget::BIN
                .with_request()
                .raw(),
            0,
            0,
            0,
        ]),
    );
    let sleeping_pid = sleep_spawn
        .decode()
        .expect("request sleeping spawn returns child pid");
    testrt::check_eq(sleep_report.pid(), reovim_uapi_process::ProcessId::new(sleeping_pid));
    testrt::check_eq(sleep_report.requested_ticks(), 5usize);
    testrt::check_eq(sleep_report.state(), reovim_uapi_process::ProcessStateCode::BLOCKED);
    let sleeping = crate::proc::process(sleeping_pid).expect("sleeping child retained");
    testrt::check_eq(sleeping.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(sleeping.block_reason, crate::sched::BlockReason::Sleep);
    testrt::check_eq(sleeping.envc, 1usize);
    testrt::check_eq(sleeping.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(sleeping.env_value(0), "/boot/status");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallStatus::Ok,
    );

    let payload_argv = [
        reovim_uapi_process::ProcessArg::from_str("server-smoke"),
        reovim_uapi_process::ProcessArg::from_str("--fixture"),
    ];
    let payload_request = reovim_uapi_process::ProcessSpawnRequest::new(
        reovim_uapi_process::ProcessSpawnTarget::PAYLOAD,
        reovim_uapi_process::ProcessSpawnMode::READY,
        &payload_argv,
        &env,
    );
    let payload_spawn = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SPAWN,
        reovim_uapi_syscall::SyscallArgs::new([
            (&payload_request as *const reovim_uapi_process::ProcessSpawnRequest) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessSpawnRequest>(),
            reovim_uapi_process::ProcessSpawnTarget::PAYLOAD
                .with_request()
                .raw(),
            0,
            0,
            0,
        ]),
    );
    let payload_pid = payload_spawn
        .decode()
        .expect("request payload spawn returns child pid");
    let payload = crate::proc::process(payload_pid).expect("payload child retained");
    testrt::check_eq(payload.program_path, "/payload/server-smoke");
    testrt::check_eq(payload.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(payload.argc, 2usize);
    testrt::check_eq(payload.argv0(), "server-smoke");
    testrt::check_eq(payload.argv1(), "--fixture");
    testrt::check_eq(payload.envc, 1usize);
    testrt::check_eq(payload.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(payload.env_value(0), "/boot/status");

    let mut bad_payload_report = reovim_uapi_process::ProcessControlReport::empty();
    let bad_payload_request = reovim_uapi_process::ProcessSpawnRequest::with_process_report(
        reovim_uapi_process::ProcessSpawnTarget::PAYLOAD,
        reovim_uapi_process::ProcessSpawnMode::READY,
        &payload_argv,
        &env,
        &mut bad_payload_report,
    );
    let rejected_payload = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SPAWN,
        reovim_uapi_syscall::SyscallArgs::new([
            (&bad_payload_request as *const reovim_uapi_process::ProcessSpawnRequest) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessSpawnRequest>(),
            reovim_uapi_process::ProcessSpawnTarget::PAYLOAD
                .with_request()
                .raw(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(
        rejected_payload.decode(),
        Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT),
    );
    testrt::check_eq(bad_payload_report.pid().raw(), 0usize);
    testrt::check(
        crate::proc::process(payload_pid + 1).is_none(),
        "payload request report form does not admit a child",
    );

    let mut bad_report = reovim_uapi_process::ProcessControlReport::empty();
    let mut bad_request = reovim_uapi_process::ProcessSpawnRequest::with_process_report(
        reovim_uapi_process::ProcessSpawnTarget::BIN,
        reovim_uapi_process::ProcessSpawnMode::READY,
        &argv,
        &env,
        &mut bad_report,
    );
    bad_request.set_flags(reovim_uapi_process::ProcessSpawnFlags::from_raw(1));
    let rejected = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SPAWN,
        reovim_uapi_syscall::SyscallArgs::new([
            (&bad_request as *const reovim_uapi_process::ProcessSpawnRequest) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessSpawnRequest>(),
            reovim_uapi_process::ProcessSpawnTarget::BIN
                .with_request()
                .raw(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(rejected.decode(), Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT));
    testrt::check_eq(bad_report.pid().raw(), 0usize);
    testrt::check(
        crate::proc::process(payload_pid + 1).is_none(),
        "unknown request flags do not admit a child",
    );

    drop(syscalls);
    let mut inspector = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
    sink_clear();
    inspector
        .write_vfs_file_path("/proc/processes")
        .expect("process diagnostics render");
    assert_contains(
        sink_bytes(),
        b"envc=1 env0_name=REOVIM_MEDIA_PATH env0_value=/boot/status env0_truncated=false",
    );
});

arch_test!(program_syscalls_dispatches_raw_process_wake_and_kill, {
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let current = crate::syscall::SyscallContext {
        pid: crate::proc::SHELL_PID,
        task_id: crate::proc::SHELL_PID,
        program_path: "/bin/sh",
        loader: "source-image",
        entry_name: "bin_sh",
    };
    let mut syscalls = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current));
    let argv = [reovim_uapi_process::ProcessArg::from_str("pwd")];
    let mut spawn_report = reovim_uapi_process::ProcessControlReport::empty();

    let spawn = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SPAWN,
        reovim_uapi_syscall::SyscallArgs::new([
            argv.as_ptr() as usize,
            argv.len(),
            reovim_uapi_process::ProcessSpawnTarget::BIN.raw(),
            reovim_uapi_process::ProcessSpawnMode::BLOCKED.raw(),
            (&mut spawn_report as *mut reovim_uapi_process::ProcessControlReport) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessControlReport>(),
        ]),
    );
    let child_pid = spawn.decode().expect("raw spawn returns child pid");
    testrt::check_eq(spawn_report.pid().raw(), child_pid);
    testrt::check_eq(spawn_report.state(), reovim_uapi_process::ProcessStateCode::BLOCKED);
    testrt::check_eq(spawn_report.path_bytes(), b"/bin/pwd".as_slice());
    testrt::check_eq(spawn_report.path_truncated(), false);
    testrt::check_eq(spawn_report.body_format_bytes(), b"linked-image");
    testrt::check_eq(spawn_report.body_inner_bytes(), b"none");
    testrt::check_eq(spawn_report.body_bytes(), 0usize);
    testrt::check_eq(spawn_report.body_checksum(), 0u32);
    let blocked = crate::proc::process(child_pid).expect("blocked child is retained");
    testrt::check_eq(blocked.state, crate::proc::ProcessState::Blocked);
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut wake_report = reovim_uapi_process::ProcessControlReport::empty();
    let wake = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROCESS_WAKE,
        reovim_uapi_syscall::SyscallArgs::new([
            child_pid,
            (&mut wake_report as *mut reovim_uapi_process::ProcessControlReport) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessControlReport>(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(wake.decode(), Ok(child_pid));
    testrt::check_eq(wake_report.pid().raw(), child_pid);
    testrt::check_eq(wake_report.state(), reovim_uapi_process::ProcessStateCode::READY);
    testrt::check_eq(wake_report.path_bytes(), b"/bin/pwd".as_slice());
    testrt::check_eq(wake_report.body_format_bytes(), b"linked-image");
    testrt::check_eq(wake_report.body_inner_bytes(), b"none");
    testrt::check_eq(wake_report.body_bytes(), 0usize);
    testrt::check_eq(wake_report.body_checksum(), 0u32);
    let ready = crate::proc::process(child_pid).expect("woken child is retained");
    testrt::check_eq(ready.state, crate::proc::ProcessState::Ready);
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut kill_report = reovim_uapi_process::ProcessControlReport::empty();
    let kill = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROCESS_KILL,
        reovim_uapi_syscall::SyscallArgs::new([
            child_pid,
            (&mut kill_report as *mut reovim_uapi_process::ProcessControlReport) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessControlReport>(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(kill.decode(), Ok(child_pid));
    testrt::check_eq(kill_report.pid().raw(), child_pid);
    testrt::check_eq(kill_report.state(), reovim_uapi_process::ProcessStateCode::FAILED);
    testrt::check_eq(kill_report.path_bytes(), b"/bin/pwd".as_slice());
    testrt::check_eq(kill_report.body_format_bytes(), b"linked-image");
    testrt::check_eq(kill_report.body_inner_bytes(), b"none");
    testrt::check_eq(kill_report.body_bytes(), 0usize);
    testrt::check_eq(kill_report.body_checksum(), 0u32);
    let killed = crate::proc::process(child_pid).expect("killed child is retained");
    testrt::check_eq(killed.state, crate::proc::ProcessState::Failed);
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessKill,
        crate::syscall::SyscallStatus::Ok,
    );

    let protected = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROCESS_KILL,
        reovim_uapi_syscall::SyscallArgs::new([crate::proc::SHELL_PID, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(protected.decode(), Err(reovim_uapi_syscall::SyscallError::PROTECTED_PROCESS));

    drop(syscalls);
    let mut no_current = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
    let no_current_wake = no_current.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROCESS_WAKE,
        reovim_uapi_syscall::SyscallArgs::new([child_pid, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(
        no_current_wake.decode(),
        Err(reovim_uapi_syscall::SyscallError::NO_CURRENT_PROCESS),
    );
    let no_current_kill = no_current.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROCESS_KILL,
        reovim_uapi_syscall::SyscallArgs::new([child_pid, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(
        no_current_kill.decode(),
        Err(reovim_uapi_syscall::SyscallError::NO_CURRENT_PROCESS),
    );
});

arch_test!(program_syscalls_dispatches_raw_sleeping_spawn_and_wait_ticks, {
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let current = crate::syscall::SyscallContext {
        pid: crate::proc::SHELL_PID,
        task_id: crate::proc::SHELL_PID,
        program_path: "/bin/sh",
        loader: "source-image",
        entry_name: "bin_sh",
    };
    let mut syscalls = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current));
    let argv = [reovim_uapi_process::ProcessArg::from_str("pwd")];
    let mut sleep_report = reovim_uapi_process::ProcessSleepReport::empty();
    sleep_report.set_requested_ticks(2);

    let spawn = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SPAWN,
        reovim_uapi_syscall::SyscallArgs::new([
            argv.as_ptr() as usize,
            argv.len(),
            reovim_uapi_process::ProcessSpawnTarget::BIN.raw(),
            reovim_uapi_process::ProcessSpawnMode::SLEEPING.raw(),
            (&mut sleep_report as *mut reovim_uapi_process::ProcessSleepReport) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessSleepReport>(),
        ]),
    );
    let child_pid = spawn
        .decode()
        .expect("raw sleeping spawn returns child pid");
    testrt::check_eq(child_pid, sleep_report.pid().raw());
    testrt::check_eq(sleep_report.requested_ticks(), 2usize);
    testrt::check_eq(sleep_report.state(), reovim_uapi_process::ProcessStateCode::BLOCKED);
    testrt::check_eq(sleep_report.wake_tick(), 2usize);
    testrt::check_eq(sleep_report.path_bytes(), b"/bin/pwd");
    let sleeping = crate::proc::process(child_pid).expect("sleeping child is retained");
    testrt::check_eq(sleeping.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(sleeping.block_reason, crate::sched::BlockReason::Sleep);
    let task = crate::sched::task(sleeping.task_id).expect("sleeping child task is retained");
    testrt::check_eq(task.wake_tick, 2usize);
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut wait_report = reovim_uapi_process::ProcessTimedWaitReport::empty();
    let wait = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROCESS_WAIT_TICKS,
        reovim_uapi_syscall::SyscallArgs::new([
            child_pid,
            2,
            (&mut wait_report as *mut reovim_uapi_process::ProcessTimedWaitReport) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessTimedWaitReport>(),
            0,
            0,
        ]),
    );
    testrt::check_eq(wait.decode(), Ok(child_pid));
    testrt::check_eq(sink_str(), "/\n");
    testrt::check_eq(wait_report.requested_ticks(), 2usize);
    testrt::check_eq(wait_report.child_pid().raw(), child_pid);
    testrt::check_eq(wait_report.child_state(), reovim_uapi_process::ProcessStateCode::EXITED);
    testrt::check_eq(wait_report.exit_code(), 0);
    testrt::check_eq(wait_report.completed(), true);
    testrt::check_eq(wait_report.timed_out(), false);
    testrt::check_eq(wait_report.tick_count(), 2usize);
    let child = crate::proc::process(child_pid).expect("waited child is retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Reaped);
    assert_syscall_record(
        "/bin/sh",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut timeout_report = reovim_uapi_process::ProcessTimedWaitReport::empty();
    let bad_wait = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROCESS_WAIT_TICKS,
        reovim_uapi_syscall::SyscallArgs::new([
            child_pid,
            0,
            (&mut timeout_report as *mut reovim_uapi_process::ProcessTimedWaitReport) as usize,
            core::mem::size_of::<reovim_uapi_process::ProcessTimedWaitReport>(),
            0,
            0,
        ]),
    );
    testrt::check_eq(bad_wait.decode(), Err(reovim_uapi_syscall::SyscallError::INVALID_ARGUMENT));
});

arch_test!(program_syscalls_dispatches_raw_execve_with_env_metadata, {
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("hello"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let argv = [reovim_uapi_process::ProcessArg::from_str("pwd")];
    let env = [reovim_uapi_process::ProcessEnv::from_pair(
        "REOVIM_MEDIA_PATH",
        "/boot/status",
    )];

    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([
            argv.as_ptr() as usize,
            argv.len(),
            env.as_ptr() as usize,
            env.len(),
            0,
            0,
        ]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));

    let process = crate::proc::process(current_ctx.pid).expect("replaced process retained");
    testrt::check_eq(process.program_path, "/bin/pwd");
    testrt::check_eq(process.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(process.envc, 1usize);
    testrt::check_eq(process.env_name(0), "REOVIM_MEDIA_PATH");
    testrt::check_eq(process.env_value(0), "/boot/status");
    testrt::check_eq(process.env_was_truncated(0), false);
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );

    drop(syscalls);
    let mut inspector = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None);
    sink_clear();
    inspector
        .write_vfs_file_path("/proc/processes")
        .expect("process diagnostics render");
    assert_contains(
        sink_bytes(),
        b"envc=1 env0_name=REOVIM_MEDIA_PATH env0_value=/boot/status env0_truncated=false",
    );
});

arch_test!(program_syscalls_dispatches_raw_execve_as_same_process_replacement, {
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("hello"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");
    let before =
        crate::proc::process(current_ctx.pid).expect("current process retained before execve");
    let original_address_space = before.address_space_id;
    let original_generation = before.image_generation;

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let argv = [reovim_uapi_process::ProcessArg::from_str("pwd")];

    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    let stale_get_pid = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::GET_PID,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(stale_get_pid.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));

    let process = crate::proc::process(current_ctx.pid).expect("replaced process retained");
    testrt::check_eq(process.pid, current_ctx.pid);
    testrt::check_eq(process.task_id, current_ctx.task_id);
    testrt::check_eq(process.program_path, "/bin/pwd");
    testrt::check_eq(process.loader, "linked-bin");
    testrt::check_eq(process.entry_name, "bin_pwd");
    testrt::check_eq(process.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(process.argv0(), "pwd");
    testrt::check(
        process.address_space_id > original_address_space,
        "raw execve replacement changes process image identity",
    );
    testrt::check_eq(process.image_generation, original_generation + 1);
    let task = crate::sched::task(current_ctx.task_id).expect("replaced task retained");
    testrt::check_eq(task.process_id, current_ctx.pid);
    testrt::check_eq(task.task_id, current_ctx.task_id);
    testrt::check_eq(task.entry, "bin_pwd");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Ready);
    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.next_ready_process_id(), current_ctx.pid);
    testrt::check_eq(snapshot.next_ready_task_id(), current_ctx.task_id);

    let mut pending =
        [crate::exec::EMPTY_PENDING_EXEC_RECORD; crate::exec::MAX_PENDING_EXEC_RECORDS];
    let pending_count = crate::exec::snapshot_pending(&mut pending);
    let mut found_pending = false;
    let mut index = 0usize;
    while index < pending_count {
        if pending[index].pid == current_ctx.pid && pending[index].path == "/bin/pwd" {
            found_pending = true;
            testrt::check_eq(pending[index].task_id, current_ctx.task_id);
            testrt::check_eq(pending[index].loader, "linked-bin");
            testrt::check_eq(pending[index].entry_name, "bin_pwd");
        }
        index += 1;
    }
    testrt::check(found_pending, "raw execve leaves replacement pending");

    drop(syscalls);
    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    testrt::check_eq(crate::exec::snapshot_pending(&mut pending), 0usize);

    let process = crate::proc::process(current_ctx.pid).expect("replaced process completed");
    testrt::check_eq(process.state, crate::proc::ProcessState::Exited);
    let task = crate::sched::task(current_ctx.task_id).expect("replaced task completed");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Exited);
    testrt::check_eq(crate::sched::snapshot_scheduler().next_ready_process_id(), 0usize);

    let no_current = crate::syscall::ProgramSyscalls::new(&daemon, &mut session, None)
        .dispatch_raw_syscall(
            reovim_uapi_syscall::SyscallNr::EXECVE,
            reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
        );
    testrt::check_eq(
        no_current.decode(),
        Err(reovim_uapi_syscall::SyscallError::NO_CURRENT_PROCESS),
    );
});

arch_test!(program_execve_invalidates_old_direct_fd_helpers, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("hello"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let argv = [ProcessArg::from_str("pwd")];
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );

    let fd_write_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Error,
    );
    let fd_read_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Error,
    );
    let pipe_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::FdPipe,
        crate::syscall::SyscallStatus::Error,
    );
    let stdio_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::StdioAttach,
        crate::syscall::SyscallStatus::Error,
    );

    testrt::check_eq(
        syscalls.write_fd(1, b"stale-leak"),
        Err(crate::syscall::ProgramIoError::BadFd),
    );
    testrt::check_eq(
        syscalls.write_fd_line(1, "stale-line"),
        Err(crate::syscall::ProgramIoError::BadFd),
    );
    syscalls.stdout_bytes(b"ignored-stale-leak");
    let mut stale_read = [0u8; 1];
    testrt::check_eq(
        syscalls.read_fd(0, &mut stale_read),
        Err(crate::syscall::ProgramIoError::BadFd),
    );
    testrt::check_eq(syscalls.pipe_fds(), Err(crate::syscall::ProgramIoError::Busy));
    let stale_stdio = syscalls.stdio();
    testrt::check_eq(stale_stdio.stdout.fd, 1usize);
    let stale_stdout = syscalls.stdout();
    testrt::check_eq(stale_stdout.fd, 1usize);
    testrt::check_eq(sink_str(), "");

    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::FdWrite,
            crate::syscall::SyscallStatus::Error,
        ),
        fd_write_errors + 3,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::FdRead,
            crate::syscall::SyscallStatus::Error,
        ),
        fd_read_errors + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::FdPipe,
            crate::syscall::SyscallStatus::Error,
        ),
        pipe_errors + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::StdioAttach,
            crate::syscall::SyscallStatus::Error,
        ),
        stdio_errors + 3,
    );

    drop(syscalls);
    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_execve_invalidates_old_direct_report_helpers, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("hello"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let argv = [ProcessArg::from_str("pwd")];
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");

    let process_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::SnapshotProcesses,
        crate::syscall::SyscallStatus::Error,
    );
    let scheduler_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::SchedulerSnapshot,
        crate::syscall::SyscallStatus::Error,
    );
    let syscall_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::SnapshotSyscalls,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );

    let mut stale_processes = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    testrt::check_eq(syscalls.snapshot_processes(&mut stale_processes), 0usize);
    testrt::check_eq(stale_processes[0].pid, 0usize);
    testrt::check_eq(stale_processes[0].program_path, "");
    syscalls.write_process_table();
    assert_blocked_scheduler_snapshot(syscalls.scheduler_snapshot());
    syscalls.write_scheduler_state();
    let mut stale_syscalls =
        [crate::syscall::EMPTY_SYSCALL_RECORD; crate::syscall::MAX_SYSCALL_RECORDS];
    testrt::check_eq(syscalls.snapshot_syscalls(&mut stale_syscalls), 0usize);
    testrt::check_eq(stale_syscalls[0].seq, 0usize);
    syscalls.write_syscall_table();
    testrt::check_eq(sink_str(), "");

    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::SnapshotProcesses,
            crate::syscall::SyscallStatus::Error,
        ),
        process_errors + 2,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::SchedulerSnapshot,
            crate::syscall::SyscallStatus::Error,
        ),
        scheduler_errors + 2,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::SnapshotSyscalls,
            crate::syscall::SyscallStatus::Error,
        ),
        syscall_errors + 2,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors,
    );

    drop(syscalls);
    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_execve_invalidates_old_direct_session_helpers, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("hello"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let argv = [ProcessArg::from_str("pwd")];
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");

    let cwd_get_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::SessionCwdGet,
        crate::syscall::SyscallStatus::Error,
    );
    let tty_read_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::TtyReadLine,
        crate::syscall::SyscallStatus::Error,
    );
    let session_mutation_ops = [
        crate::syscall::SyscallOp::SessionCwdSet,
        crate::syscall::SyscallOp::SessionShellTarget,
        crate::syscall::SyscallOp::SessionShellStart,
        crate::syscall::SyscallOp::SessionLineDiscipline,
    ];
    let mut session_mutation_ok_before = [0usize; 4];
    let mut session_mutation_error_before = [0usize; 4];
    let mut session_mutation_index = 0usize;
    while session_mutation_index < session_mutation_ops.len() {
        session_mutation_ok_before[session_mutation_index] = syscall_record_count(
            "",
            session_mutation_ops[session_mutation_index],
            crate::syscall::SyscallStatus::Ok,
        );
        session_mutation_error_before[session_mutation_index] = syscall_record_count(
            "",
            session_mutation_ops[session_mutation_index],
            crate::syscall::SyscallStatus::Error,
        );
        session_mutation_index += 1;
    }
    let continue_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );

    testrt::check_eq(syscalls.cwd(), "");
    TTY_READ_CALLS.store(0, Ordering::Relaxed);
    let mut tty_line = [0x44u8; crate::rootd::ROOT_LINE_BYTES];
    testrt::check_eq(syscalls.read_tty_line(&mut tty_line), 0usize);
    testrt::check_eq(TTY_READ_CALLS.load(Ordering::Relaxed), 0usize);
    testrt::check_eq(tty_line[0], 0x44u8);
    let bin_cwd = crate::vfs::normalize("/", "/bin").expect("test cwd path normalizes");
    syscalls.set_cwd(bin_cwd);
    syscalls.request_shell_target("/bin/sh");
    syscalls.request_shell_start();
    syscalls.request_shell_line_discipline("argv-v1", "single-pipe");
    testrt::check_eq(sink_str(), "");

    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::SessionCwdGet,
            crate::syscall::SyscallStatus::Error,
        ),
        cwd_get_errors + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::TtyReadLine,
            crate::syscall::SyscallStatus::Error,
        ),
        tty_read_errors + 1,
    );
    session_mutation_index = 0;
    while session_mutation_index < session_mutation_ops.len() {
        testrt::check_eq(
            syscall_record_count(
                "",
                session_mutation_ops[session_mutation_index],
                crate::syscall::SyscallStatus::Ok,
            ),
            session_mutation_ok_before[session_mutation_index],
        );
        testrt::check_eq(
            syscall_record_count(
                "",
                session_mutation_ops[session_mutation_index],
                crate::syscall::SyscallStatus::Error,
            ),
            session_mutation_error_before[session_mutation_index] + 1,
        );
        session_mutation_index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors,
    );

    drop(syscalls);
    testrt::check_eq(session.cwd(), "/");
    testrt::check_eq(session.shell_target_requested(), None);
    testrt::check_eq(session.shell_start_requested(), false);
    testrt::check_eq(session.line_discipline_requested(), false);
    testrt::check_eq(session.line_discipline(), "none");
    testrt::check_eq(session.pipe_mode(), "none");

    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_execve_invalidates_old_direct_boot_catalog_helpers, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), Some(diagnostics));
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("hello"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let argv = [ProcessArg::from_str("pwd")];
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");

    let tracked_ops = [
        crate::syscall::SyscallOp::BootInfo,
        crate::syscall::SyscallOp::DeviceCatalog,
        crate::syscall::SyscallOp::BootProfile,
        crate::syscall::SyscallOp::SnapshotSession,
        crate::syscall::SyscallOp::BootImage,
        crate::syscall::SyscallOp::ConsoleInput,
        crate::syscall::SyscallOp::PayloadCatalog,
        crate::syscall::SyscallOp::VfsMounts,
        crate::syscall::SyscallOp::ProgramHelp,
        crate::syscall::SyscallOp::ProviderProbe,
    ];
    let expected_error_increments = [3usize, 4, 3, 1, 4, 2, 2, 1, 3, 2];
    let mut ok_before = [0usize; 10];
    let mut error_before = [0usize; 10];
    let mut index = 0usize;
    while index < tracked_ops.len() {
        ok_before[index] =
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Ok);
        error_before[index] =
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Error);
        index += 1;
    }
    let continue_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );

    assert_blocked_boot_info(syscalls.boot_info());
    testrt::check_eq(syscalls.devices().len(), 0usize);
    testrt::check_eq(syscalls.profile_name(), "blocked");
    testrt::check_eq(syscalls.prompt(), "blocked");
    testrt::check_eq(syscalls.launch_enabled(), false);
    assert_blocked_boot_image(syscalls.boot_image());
    assert_blocked_console_input(syscalls.console_input());
    testrt::check_eq(syscalls.payloads().len(), 0usize);
    testrt::check_eq(syscalls.programs().len(), 0usize);
    syscalls.write_boot_info_summary();
    syscalls.write_device_inventory();
    syscalls.write_boot_profile();
    syscalls.write_boot_image();
    syscalls.write_boot_payloads();
    syscalls.write_boot_memory();
    syscalls.write_boot_devices();
    syscalls.write_boot_status();
    syscalls.write_boot_proof();
    syscalls.write_boot_input();
    syscalls.write_mount_table();
    testrt::check_eq(syscalls.write_program_help(None), crate::program::ProgramStatus::Blocked);
    syscalls.write_program_file_metadata(0);
    syscalls.write_device_file_metadata(0);
    testrt::check(syscalls.run_hardware_probe("help").is_none(), "stale probe denied");
    syscalls.write_probe_catalog();
    testrt::check_eq(sink_str(), "");

    index = 0;
    while index < tracked_ops.len() {
        testrt::check_eq(
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Ok),
            ok_before[index],
        );
        testrt::check_eq(
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Error),
            error_before[index] + expected_error_increments[index],
        );
        index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors,
    );

    drop(syscalls);
    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_execve_invalidates_old_direct_diagnostic_helpers, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    crate::klog::append_line("stale diagnostic leak sentinel");
    let daemon = daemon(ProfileSummary::new("shell-only", false), Some(diagnostics));
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("hello"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let argv = [ProcessArg::from_str("pwd")];
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");

    let tracked_ops = [
        crate::syscall::SyscallOp::KernelLogRead,
        crate::syscall::SyscallOp::KernelLogStats,
        crate::syscall::SyscallOp::DumpStatus,
        crate::syscall::SyscallOp::DumpSync,
    ];
    let expected_error_increments = [3usize, 2, 3, 2];
    let mut ok_before = [0usize; 4];
    let mut error_before = [0usize; 4];
    let mut index = 0usize;
    while index < tracked_ops.len() {
        ok_before[index] =
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Ok);
        error_before[index] =
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Error);
        index += 1;
    }
    let continue_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );

    testrt::check_eq(syscalls.write_kernel_log(), false);
    syscalls.write_kernel_log_view();
    assert_blocked_kernel_log_stats(syscalls.kernel_log_stats());
    syscalls.write_kernel_log_stats();
    testrt::check(syscalls.external_dmesg().is_none(), "stale external dmesg denied");
    assert_blocked_dump_status(syscalls.dump_status());
    assert_blocked_dump_sync_status(syscalls.dump_sync());
    syscalls.write_dump_status();
    syscalls.write_dump_snapshot();
    syscalls.write_dump_sync();
    testrt::check_eq(sink_str(), "");

    index = 0;
    while index < tracked_ops.len() {
        testrt::check_eq(
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Ok),
            ok_before[index],
        );
        testrt::check_eq(
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Error),
            error_before[index] + expected_error_increments[index],
        );
        index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors,
    );

    drop(syscalls);
    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_execve_invalidates_old_direct_vfs_helpers, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("hello"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let argv = [ProcessArg::from_str("pwd")];
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");

    let tracked_ops = [
        crate::syscall::SyscallOp::VfsNormalize,
        crate::syscall::SyscallOp::VfsLookup,
        crate::syscall::SyscallOp::VfsList,
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallOp::VfsRead,
    ];
    let expected_error_increments = [4usize, 1, 3, 5, 0];
    let mut ok_before = [0usize; 5];
    let mut error_before = [0usize; 5];
    let mut index = 0usize;
    while index < tracked_ops.len() {
        ok_before[index] =
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Ok);
        error_before[index] =
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Error);
        index += 1;
    }
    let continue_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );

    testrt::check_eq(syscalls.normalize_path("/boot").err(), Some(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.lookup_path("/").err(), Some(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.write_vfs_listing("/").err(), Some(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.write_vfs_listing("/bin").err(), Some(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.write_vfs_listing("/dev").err(), Some(crate::vfs::VfsError::Busy));
    testrt::check_eq(
        syscalls.open_vfs_file_path("/boot/profile").err(),
        Some(crate::vfs::VfsError::Busy),
    );
    testrt::check_eq(syscalls.open_vfs_directory_path("/").err(), Some(crate::vfs::VfsError::Busy));
    testrt::check_eq(
        syscalls.open_vfs_directory_path("/bin").err(),
        Some(crate::vfs::VfsError::Busy),
    );
    testrt::check_eq(
        syscalls.open_vfs_directory_path("/dev").err(),
        Some(crate::vfs::VfsError::Busy),
    );
    testrt::check_eq(
        syscalls.write_vfs_file_path("/boot/profile").err(),
        Some(crate::vfs::VfsError::Busy),
    );
    testrt::check_eq(sink_str(), "");

    index = 0;
    while index < tracked_ops.len() {
        testrt::check_eq(
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Ok),
            ok_before[index],
        );
        testrt::check_eq(
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Error),
            error_before[index] + expected_error_increments[index],
        );
        index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors,
    );

    drop(syscalls);
    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_execve_invalidates_old_direct_payload_launch_helpers, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("hello"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let argv = [ProcessArg::from_str("pwd")];
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");

    let payload_load_ok = syscall_record_count(
        "",
        crate::syscall::SyscallOp::PayloadLoad,
        crate::syscall::SyscallStatus::Ok,
    );
    let payload_load_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::PayloadLoad,
        crate::syscall::SyscallStatus::Error,
    );
    let payload_launch_ok = syscall_record_count(
        "",
        crate::syscall::SyscallOp::PayloadLaunch,
        crate::syscall::SyscallStatus::Ok,
    );
    let payload_launch_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::PayloadLaunch,
        crate::syscall::SyscallStatus::Error,
    );
    let spawn_child_ok = syscall_record_count(
        "",
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );
    let payload_run_ok = syscall_record_count(
        "",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );
    let continue_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );

    testrt::check_eq(syscalls.launch_payload_by_name("reovim"), PayloadLaunchResult::Failed);
    testrt::check_eq(syscalls.launch_payload_argv(argv1("reovim")), PayloadLaunchResult::Failed);
    testrt::check_eq(sink_str(), "");
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::PayloadLoad,
            crate::syscall::SyscallStatus::Ok,
        ),
        payload_load_ok,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::PayloadLoad,
            crate::syscall::SyscallStatus::Error,
        ),
        payload_load_errors,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::PayloadLaunch,
            crate::syscall::SyscallStatus::Ok,
        ),
        payload_launch_ok,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::PayloadLaunch,
            crate::syscall::SyscallStatus::Error,
        ),
        payload_launch_errors + 2,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::SpawnChild,
            crate::syscall::SyscallStatus::Ok,
        ),
        spawn_child_ok,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::PayloadRun,
            crate::syscall::SyscallStatus::Ok,
        ),
        payload_run_ok,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors,
    );

    let mut pid = 0usize;
    while pid <= crate::proc::MAX_PROCESSES {
        if let Some(process) = crate::proc::process(pid) {
            testrt::check(
                process.program_path != "/payload/reovim",
                "stale execve handle must not launch payload process",
            );
        }
        pid += 1;
    }

    drop(syscalls);
    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_execve_invalidates_old_direct_tty_clear_helper, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("hello"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let argv = [ProcessArg::from_str("pwd")];
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");

    let tty_clear_ok = syscall_record_count(
        "",
        crate::syscall::SyscallOp::TtyClear,
        crate::syscall::SyscallStatus::Ok,
    );
    let tty_clear_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::TtyClear,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );

    syscalls.clear_console();
    testrt::check_eq(sink_str(), "");
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::TtyClear,
            crate::syscall::SyscallStatus::Ok,
        ),
        tty_clear_ok,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::TtyClear,
            crate::syscall::SyscallStatus::Error,
        ),
        tty_clear_errors + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors,
    );

    drop(syscalls);
    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_execve_invalidates_old_direct_process_scheduler_service_helpers, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    crate::service::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("hello"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let argv = [ProcessArg::from_str("pwd")];
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");

    let tracked_ops = [
        crate::syscall::SyscallOp::ProcessSelf,
        crate::syscall::SyscallOp::YieldNow,
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallOp::ProcessKill,
        crate::syscall::SyscallOp::InitServiceStart,
        crate::syscall::SyscallOp::ServiceReady,
        crate::syscall::SyscallOp::ServiceHold,
        crate::syscall::SyscallOp::ServiceStop,
        crate::syscall::SyscallOp::ServiceStart,
        crate::syscall::SyscallOp::ServiceRestart,
    ];
    let mut ok_before = [0usize; 12];
    let mut error_before = [0usize; 12];
    let mut unavailable_before = [0usize; 12];
    let mut index = 0usize;
    while index < tracked_ops.len() {
        ok_before[index] =
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Ok);
        error_before[index] =
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Error);
        unavailable_before[index] = syscall_record_count(
            "",
            tracked_ops[index],
            crate::syscall::SyscallStatus::Unavailable,
        );
        index += 1;
    }
    let continue_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    let before_scheduler = crate::sched::snapshot_scheduler();
    let mut before_services = [crate::service::EMPTY_SERVICE_RECORD; crate::service::MAX_SERVICES];
    let before_service_count = crate::service::snapshot(&mut before_services);

    testrt::check(
        syscalls.process_self().is_none(),
        "stale execve process_self should fail closed",
    );
    let yield_result = syscalls.yield_now_result();
    testrt::check_eq(yield_result.status, crate::syscall::SchedulerYieldStatus::Busy);
    testrt::check_eq(yield_result.yielded, false);
    testrt::check_eq(yield_result.selected_pid, 0usize);
    testrt::check_eq(yield_result.selected_task_id, 0usize);
    let sleep_result = syscalls.sleep_current_for_ticks_result(1);
    testrt::check_eq(sleep_result.status, crate::syscall::SchedulerSleepStatus::Busy);
    testrt::check_eq(sleep_result.slept, false);
    testrt::check_eq(sleep_result.pid, 0usize);
    testrt::check_eq(sleep_result.task_id, 0usize);
    let tick_result = syscalls.scheduler_tick_result();
    testrt::check_eq(tick_result.status, crate::syscall::SchedulerTickStatus::Busy);
    testrt::check_eq(tick_result.ticked, false);
    testrt::check_eq(tick_result.task_id, 0usize);
    testrt::check_eq(
        syscalls.wake_process_by_pid(crate::proc::ROOTD_PID),
        Err(crate::syscall::ProgramProcessError::Busy),
    );
    testrt::check_eq(
        syscalls.kill_process_by_pid(crate::proc::ROOTD_PID),
        Err(crate::syscall::ProgramProcessError::Busy),
    );
    testrt::check_eq(
        syscalls.request_init_service("shell", "/bin/sh"),
        Err(crate::service::ServiceError::Busy),
    );
    testrt::check_eq(syscalls.service_ready("editor"), Err(crate::service::ServiceError::Busy));
    testrt::check_eq(syscalls.service_hold(), Err(crate::syscall::ProgramProcessError::Busy));
    testrt::check_eq(
        syscalls.stop_service_by_name("editor"),
        Err(crate::syscall::ProgramServiceError::Busy),
    );
    testrt::check_eq(
        syscalls.start_service_by_name("editor"),
        Err(crate::syscall::ProgramServiceError::Busy),
    );
    testrt::check_eq(
        syscalls.restart_service_by_name("editor"),
        Err(crate::syscall::ProgramServiceError::Busy),
    );
    testrt::check_eq(sink_str(), "");

    let after_scheduler = crate::sched::snapshot_scheduler();
    testrt::check_eq(after_scheduler.tick_count, before_scheduler.tick_count);
    testrt::check_eq(after_scheduler.yield_count, before_scheduler.yield_count);
    testrt::check_eq(after_scheduler.dispatch_count, before_scheduler.dispatch_count);
    let mut after_services = [crate::service::EMPTY_SERVICE_RECORD; crate::service::MAX_SERVICES];
    let after_service_count = crate::service::snapshot(&mut after_services);
    testrt::check_eq(after_service_count, before_service_count);

    index = 0;
    while index < tracked_ops.len() {
        testrt::check_eq(
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Ok),
            ok_before[index],
        );
        testrt::check_eq(
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Error),
            error_before[index] + 1,
        );
        testrt::check_eq(
            syscall_record_count(
                "",
                tracked_ops[index],
                crate::syscall::SyscallStatus::Unavailable,
            ),
            unavailable_before[index],
        );
        index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors,
    );

    drop(syscalls);
    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_execve_invalidates_old_raw_syscall_dispatch, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    crate::dump::reset_last_sync_for_tests();
    reset_installed_sources();
    clear_source_media();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("appliance", true), Some(diagnostics));
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("hello"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let argv = [ProcessArg::from_str("pwd")];
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");

    let tracked_ops = [
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallOp::ProcessSelf,
        crate::syscall::SyscallOp::SystemHalt,
        crate::syscall::SyscallOp::TtyClear,
        crate::syscall::SyscallOp::SessionShellStart,
        crate::syscall::SyscallOp::DumpSync,
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallOp::ProviderProbe,
        crate::syscall::SyscallOp::ExecReplace,
    ];
    let mut ok_before = [0usize; 11];
    let mut error_before = [0usize; 11];
    let mut unavailable_before = [0usize; 11];
    let mut index = 0usize;
    while index < tracked_ops.len() {
        ok_before[index] =
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Ok);
        error_before[index] =
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Error);
        unavailable_before[index] = syscall_record_count(
            "",
            tracked_ops[index],
            crate::syscall::SyscallStatus::Unavailable,
        );
        index += 1;
    }
    let continue_errors = syscall_record_count(
        "",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );

    let stale_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([0, 0, 1, 0, 0, 0]),
    );
    testrt::check_eq(stale_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    let stale_write = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([1, b"stale-raw".as_ptr() as usize, 9, 0, 0, 0]),
    );
    testrt::check_eq(stale_write.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    let stale_open = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            OpenAtDir::session_cwd().raw() as usize,
            0,
            1,
            OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(stale_open.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    let stale_get_pid = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::GET_PID,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(stale_get_pid.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    let stale_halt = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SYSTEM_HALT,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(stale_halt.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    let stale_clear = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::TERMINAL_CLEAR,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_terminal::PRIMARY_OUTPUT.raw() as usize,
            0,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(stale_clear.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    let stale_session = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SESSION_CONTROL,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_session::SessionControlOp::SHELL_START.raw(),
            0,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(stale_session.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));

    let mut dump_report = reovim_uapi_dump::DumpSyncReport::empty();
    let stale_dump = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUMP_SYNC,
        reovim_uapi_syscall::SyscallArgs::new([
            (&mut dump_report as *mut reovim_uapi_dump::DumpSyncReport) as usize,
            core::mem::size_of::<reovim_uapi_dump::DumpSyncReport>(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(stale_dump.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    testrt::check_eq(dump_report, reovim_uapi_dump::DumpSyncReport::empty());

    let mut source_report = reovim_uapi_source::SourceInstallReport::empty();
    let source_name = "server-smoke";
    let stale_source = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SOURCE_CONTROL,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_source::SourceControlOp::INSTALL_PAYLOAD_STATUS.raw(),
            source_name.as_ptr() as usize,
            source_name.len(),
            reovim_uapi_source::SourceInstallStatusCode::READY.raw(),
            (&mut source_report as *mut reovim_uapi_source::SourceInstallReport) as usize,
            core::mem::size_of::<reovim_uapi_source::SourceInstallReport>(),
        ]),
    );
    testrt::check_eq(stale_source.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    testrt::check_eq(source_report.status(), reovim_uapi_source::SourceInstallStatusCode::NONE);

    let probe_target = "help";
    let stale_probe = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROVIDER_PROBE,
        reovim_uapi_syscall::SyscallArgs::new([
            probe_target.as_ptr() as usize,
            probe_target.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(stale_probe.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    let argv_again = [ProcessArg::from_str("hello")];
    let stale_execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([
            argv_again.as_ptr() as usize,
            argv_again.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(stale_execve.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    testrt::check_eq(sink_str(), "");

    index = 0;
    while index < tracked_ops.len() {
        testrt::check_eq(
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Ok),
            ok_before[index],
        );
        testrt::check_eq(
            syscall_record_count("", tracked_ops[index], crate::syscall::SyscallStatus::Error),
            error_before[index] + 1,
        );
        testrt::check_eq(
            syscall_record_count(
                "",
                tracked_ops[index],
                crate::syscall::SyscallStatus::Unavailable,
            ),
            unavailable_before[index],
        );
        index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors,
    );

    let mut pending =
        [crate::exec::EMPTY_PENDING_EXEC_RECORD; crate::exec::MAX_PENDING_EXEC_RECORDS];
    let pending_count = crate::exec::snapshot_pending(&mut pending);
    let mut found_pwd_pending = false;
    let mut found_hello_pending = false;
    index = 0;
    while index < pending_count {
        if pending[index].pid == current_ctx.pid && pending[index].path == "/bin/pwd" {
            found_pwd_pending = true;
        }
        if pending[index].pid == current_ctx.pid && pending[index].path == "/bin/hello" {
            found_hello_pending = true;
        }
        index += 1;
    }
    testrt::check(found_pwd_pending, "first execve replacement remains pending");
    testrt::check(!found_hello_pending, "stale raw execve must not admit another replacement");

    drop(syscalls);
    testrt::check_eq(session.shell_start_requested(), false);
    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "/\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_execve_admission_readies_replacement_for_rootd_dispatch, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon_with_programs(&CLOEXEC_TEST_PROGRAMS, &CLOEXEC_TEST_PROGRAM_SOURCES);
    let current_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("carrier loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("carrier"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");
    let before =
        crate::proc::process(current_ctx.pid).expect("current process retained before execve");
    let original_address_space = before.address_space_id;
    let original_generation = before.image_generation;

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let open_path = b"/boot/profile";
    let open = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            OpenFlags::READ_ONLY.with_close_on_exec().raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(open.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD));

    let replaced_ctx = syscalls
        .admit_execve_program_argv(argv2("fdread", "3"))
        .expect("execve admission succeeds");
    testrt::check_eq(replaced_ctx.pid, current_ctx.pid);
    testrt::check_eq(replaced_ctx.task_id, current_ctx.task_id);
    testrt::check_eq(replaced_ctx.program_path, "/bin/fdread");
    testrt::check_eq(sink_str(), "");
    assert_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let stale_get_pid = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::GET_PID,
        reovim_uapi_syscall::SyscallArgs::EMPTY,
    );
    testrt::check_eq(stale_get_pid.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(syscalls);

    let process = crate::proc::process(current_ctx.pid).expect("replacement process retained");
    testrt::check_eq(process.pid, current_ctx.pid);
    testrt::check_eq(process.task_id, current_ctx.task_id);
    testrt::check_eq(process.program_path, "/bin/fdread");
    testrt::check_eq(process.loader, "linked-bin");
    testrt::check_eq(process.entry_name, "bin_fd_read");
    testrt::check_eq(process.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(process.argv0(), "fdread");
    testrt::check_eq(process.argv1(), "3");
    testrt::check(
        process.address_space_id > original_address_space,
        "admission changes image identity",
    );
    testrt::check_eq(process.image_generation, original_generation + 1);

    let task = crate::sched::task(current_ctx.task_id).expect("replacement task retained");
    testrt::check_eq(task.process_id, current_ctx.pid);
    testrt::check_eq(task.task_id, current_ctx.task_id);
    testrt::check_eq(task.entry, "bin_fd_read");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Ready);
    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.next_ready_process_id(), current_ctx.pid);
    testrt::check_eq(snapshot.next_ready_task_id(), current_ctx.task_id);

    let mut pending =
        [crate::exec::EMPTY_PENDING_EXEC_RECORD; crate::exec::MAX_PENDING_EXEC_RECORDS];
    let pending_count = crate::exec::snapshot_pending(&mut pending);
    let mut found_pending = false;
    let mut index = 0usize;
    while index < pending_count {
        if pending[index].pid == current_ctx.pid && pending[index].path == "/bin/fdread" {
            found_pending = true;
            testrt::check_eq(pending[index].task_id, current_ctx.task_id);
            testrt::check_eq(pending[index].loader, "linked-bin");
            testrt::check_eq(pending[index].entry_name, "bin_fd_read");
        }
        index += 1;
    }
    testrt::check(found_pending, "execve admission leaves replacement pending");

    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::ExitCode(1));
    testrt::check_eq(sink_str(), "fd-read-error\n");
    assert_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::ProcessRun,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );
    testrt::check_eq(crate::exec::snapshot_pending(&mut pending), 0usize);

    let process = crate::proc::process(current_ctx.pid).expect("replacement process completed");
    testrt::check_eq(process.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(process.exit_code, 1);
    let task = crate::sched::task(current_ctx.task_id).expect("replacement task completed");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Failed);
    testrt::check_eq(crate::sched::snapshot_scheduler().next_ready_process_id(), 0usize);
});

arch_test!(program_execve_fails_busy_with_active_retained_continuation, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    crate::dump::reset_last_sync_for_tests();
    reset_installed_sources();
    clear_source_media();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "cat",
    )
    .expect("cat loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("cat"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));
    let mut pipe_buf = [0u8; 4];
    let blocked_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(blocked_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));

    let fd_write_ok_before = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    let fd_write_error_before = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_raw_write = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    let blocked_write = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([1, b"leak".as_ptr() as usize, 4, 0, 0, 0]),
    );
    testrt::check_eq(blocked_write.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    testrt::check_eq(sink_str(), "");
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::FdWrite,
            crate::syscall::SyscallStatus::Ok,
        ),
        fd_write_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::FdWrite,
            crate::syscall::SyscallStatus::Error,
        ),
        fd_write_error_before + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_raw_write + 1,
    );
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(current_ctx.pid),
        1usize,
    );

    let stdio_ok_before = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::StdioAttach,
        crate::syscall::SyscallStatus::Ok,
    );
    let stdio_error_before = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::StdioAttach,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_stdio = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    let stale_stdio = syscalls.stdio();
    testrt::check_eq(stale_stdio.stdin.fd, 0usize);
    testrt::check_eq(sink_str(), "");
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::StdioAttach,
            crate::syscall::SyscallStatus::Ok,
        ),
        stdio_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::StdioAttach,
            crate::syscall::SyscallStatus::Error,
        ),
        stdio_error_before + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_stdio + 1,
    );
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(current_ctx.pid),
        1usize,
    );

    let stdio_error_before_streams = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::StdioAttach,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_streams = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    let stale_stdin = syscalls.stdin();
    let stale_stdout = syscalls.stdout();
    let stale_stderr = syscalls.stderr();
    testrt::check_eq(stale_stdin.fd, 0usize);
    testrt::check_eq(stale_stdout.fd, 1usize);
    testrt::check_eq(stale_stderr.fd, 2usize);
    testrt::check_eq(sink_str(), "");
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::StdioAttach,
            crate::syscall::SyscallStatus::Ok,
        ),
        stdio_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::StdioAttach,
            crate::syscall::SyscallStatus::Error,
        ),
        stdio_error_before_streams + 3,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_streams + 3,
    );
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(current_ctx.pid),
        1usize,
    );

    let fd_write_ok_before_direct = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    let fd_write_error_before_direct = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Error,
    );
    let fd_read_ok_before_direct = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    let fd_read_error_before_direct = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_direct_fd = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    testrt::check_eq(
        syscalls.write_fd(1, b"helper-leak"),
        Err(crate::syscall::ProgramIoError::Busy),
    );
    syscalls.stdout_bytes(b"ignored-helper-leak");
    testrt::check_eq(sink_str(), "");

    let mut helper_buf = [0u8; 1];
    testrt::check_eq(
        syscalls.read_fd(pipe_fds[0] as usize, &mut helper_buf),
        Err(crate::syscall::ProgramIoError::Busy),
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::FdWrite,
            crate::syscall::SyscallStatus::Ok,
        ),
        fd_write_ok_before_direct,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::FdWrite,
            crate::syscall::SyscallStatus::Error,
        ),
        fd_write_error_before_direct + 2,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::FdRead,
            crate::syscall::SyscallStatus::Ok,
        ),
        fd_read_ok_before_direct,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::FdRead,
            crate::syscall::SyscallStatus::Error,
        ),
        fd_read_error_before_direct + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_direct_fd + 4,
    );
    let vfs_open_ok_before_direct = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    let vfs_open_error_before_direct = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_direct_vfs_open = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    testrt::check_eq(syscalls.open_vfs_file_path("/boot/profile"), Err(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.normalize_path("/boot"), Err(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.lookup_path("/"), Err(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.write_vfs_listing("/"), Err(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.write_vfs_listing("/bin"), Err(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.write_vfs_listing("/dev"), Err(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.open_vfs_directory_path("/"), Err(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.open_vfs_directory_path("/bin"), Err(crate::vfs::VfsError::Busy));
    testrt::check_eq(syscalls.open_vfs_directory_path("/dev"), Err(crate::vfs::VfsError::Busy));
    testrt::check_eq(sink_str(), "");
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::VfsOpen,
            crate::syscall::SyscallStatus::Ok,
        ),
        vfs_open_ok_before_direct,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::VfsOpen,
            crate::syscall::SyscallStatus::Error,
        ),
        vfs_open_error_before_direct + 4,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_direct_vfs_open + 9,
    );
    let fd_pipe_ok_before_direct = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::FdPipe,
        crate::syscall::SyscallStatus::Ok,
    );
    let fd_pipe_error_before_direct = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::FdPipe,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_direct_pipe = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    testrt::check_eq(syscalls.pipe_fds(), Err(crate::syscall::ProgramIoError::Busy));
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::FdPipe,
            crate::syscall::SyscallStatus::Ok,
        ),
        fd_pipe_ok_before_direct,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::FdPipe,
            crate::syscall::SyscallStatus::Error,
        ),
        fd_pipe_error_before_direct + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_direct_pipe + 1,
    );
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(current_ctx.pid),
        1usize,
    );

    let mut before_processes = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let before_process_count = crate::proc::snapshot(&mut before_processes);
    let exec_spawn_ok_before_direct = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Ok,
    );
    let exec_spawn_error_before_direct = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecSpawn,
        crate::syscall::SyscallStatus::Error,
    );
    let exec_replace_ok_before_direct = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );
    let exec_replace_error_before_direct = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_direct_exec = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    testrt::check_eq(
        syscalls.exec_program_argv_and_wait(argv1("pwd")),
        Err(crate::syscall::ProgramExecError::Busy),
    );
    testrt::check_eq(
        syscalls.execve_program_argv(argv1("pwd")),
        Err(crate::syscall::ProgramExecError::Busy),
    );
    testrt::check_eq(
        syscalls.spawn_program_argv(argv1("pwd")),
        Err(crate::syscall::ProgramExecError::Busy),
    );
    testrt::check_eq(sink_str(), "");
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::ExecSpawn,
            crate::syscall::SyscallStatus::Ok,
        ),
        exec_spawn_ok_before_direct,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::ExecSpawn,
            crate::syscall::SyscallStatus::Error,
        ),
        exec_spawn_error_before_direct + 2,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::ExecReplace,
            crate::syscall::SyscallStatus::Ok,
        ),
        exec_replace_ok_before_direct,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::ExecReplace,
            crate::syscall::SyscallStatus::Error,
        ),
        exec_replace_error_before_direct + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_direct_exec + 3,
    );
    testrt::check_eq(
        syscalls.launch_payload_by_name("reovim"),
        crate::rootd::PayloadLaunchResult::Failed,
    );
    testrt::check_eq(
        syscalls.launch_payload_argv(argv1("reovim")),
        crate::rootd::PayloadLaunchResult::Failed,
    );
    let process_control_ops = [
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallOp::ProcessKill,
    ];
    let mut process_control_ok_before = [0usize; 3];
    let mut process_control_error_before = [0usize; 3];
    let mut process_control_index = 0usize;
    while process_control_index < process_control_ops.len() {
        process_control_ok_before[process_control_index] = syscall_record_count(
            "/bin/cat",
            process_control_ops[process_control_index],
            crate::syscall::SyscallStatus::Ok,
        );
        process_control_error_before[process_control_index] = syscall_record_count(
            "/bin/cat",
            process_control_ops[process_control_index],
            crate::syscall::SyscallStatus::Error,
        );
        process_control_index += 1;
    }
    let continue_errors_before_process_control = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    testrt::check_eq(
        syscalls.wait_process_by_pid(crate::proc::ROOTD_PID),
        Err(crate::syscall::ProgramProcessError::Busy),
    );
    testrt::check_eq(
        syscalls.wake_process_by_pid(crate::proc::ROOTD_PID),
        Err(crate::syscall::ProgramProcessError::Busy),
    );
    testrt::check_eq(
        syscalls.kill_process_by_pid(crate::proc::ROOTD_PID),
        Err(crate::syscall::ProgramProcessError::Busy),
    );
    process_control_index = 0;
    while process_control_index < process_control_ops.len() {
        testrt::check_eq(
            syscall_record_count(
                "/bin/cat",
                process_control_ops[process_control_index],
                crate::syscall::SyscallStatus::Ok,
            ),
            process_control_ok_before[process_control_index],
        );
        testrt::check_eq(
            syscall_record_count(
                "/bin/cat",
                process_control_ops[process_control_index],
                crate::syscall::SyscallStatus::Error,
            ),
            process_control_error_before[process_control_index] + 1,
        );
        process_control_index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_process_control + process_control_ops.len(),
    );
    let scheduler_ops = [
        crate::syscall::SyscallOp::YieldNow,
        crate::syscall::SyscallOp::ProcessSleep,
        crate::syscall::SyscallOp::SchedulerTick,
    ];
    let scheduler_error_delta = [1usize, 1usize, 2usize];
    let mut scheduler_ok_before = [0usize; 3];
    let mut scheduler_error_before = [0usize; 3];
    let mut scheduler_index = 0usize;
    while scheduler_index < scheduler_ops.len() {
        scheduler_ok_before[scheduler_index] = syscall_record_count(
            "/bin/cat",
            scheduler_ops[scheduler_index],
            crate::syscall::SyscallStatus::Ok,
        );
        scheduler_error_before[scheduler_index] = syscall_record_count(
            "/bin/cat",
            scheduler_ops[scheduler_index],
            crate::syscall::SyscallStatus::Error,
        );
        scheduler_index += 1;
    }
    let continue_errors_before_scheduler = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    let before_scheduler = crate::sched::snapshot_scheduler();
    let yield_result = syscalls.yield_now_result();
    testrt::check_eq(yield_result.status, crate::syscall::SchedulerYieldStatus::Busy);
    testrt::check_eq(yield_result.yielded, false);
    testrt::check_eq(yield_result.selected_pid, current_ctx.pid);
    testrt::check_eq(yield_result.selected_task_id, current_ctx.task_id);
    let sleep_result = syscalls.sleep_current_for_ticks_result(1);
    testrt::check_eq(sleep_result.status, crate::syscall::SchedulerSleepStatus::Busy);
    testrt::check_eq(sleep_result.slept, false);
    testrt::check_eq(sleep_result.pid, current_ctx.pid);
    testrt::check_eq(sleep_result.task_id, current_ctx.task_id);
    let tick_result = syscalls.scheduler_tick_result();
    testrt::check_eq(tick_result.status, crate::syscall::SchedulerTickStatus::Busy);
    testrt::check_eq(tick_result.ticked, false);
    testrt::check_eq(tick_result.task_id, current_ctx.task_id);
    sink_clear();
    syscalls.write_scheduler_tick();
    testrt::check_eq(sink_str(), "");
    let after_scheduler = crate::sched::snapshot_scheduler();
    testrt::check_eq(after_scheduler.tick_count, before_scheduler.tick_count);
    testrt::check_eq(after_scheduler.yield_count, before_scheduler.yield_count);
    testrt::check_eq(after_scheduler.dispatch_count, before_scheduler.dispatch_count);
    scheduler_index = 0;
    while scheduler_index < scheduler_ops.len() {
        testrt::check_eq(
            syscall_record_count(
                "/bin/cat",
                scheduler_ops[scheduler_index],
                crate::syscall::SyscallStatus::Ok,
            ),
            scheduler_ok_before[scheduler_index],
        );
        testrt::check_eq(
            syscall_record_count(
                "/bin/cat",
                scheduler_ops[scheduler_index],
                crate::syscall::SyscallStatus::Error,
            ),
            scheduler_error_before[scheduler_index] + scheduler_error_delta[scheduler_index],
        );
        scheduler_index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_scheduler + 4,
    );
    let service_control_ops = [
        crate::syscall::SyscallOp::InitServiceStart,
        crate::syscall::SyscallOp::ServiceReady,
        crate::syscall::SyscallOp::ServiceHold,
        crate::syscall::SyscallOp::ServiceStop,
        crate::syscall::SyscallOp::ServiceStart,
        crate::syscall::SyscallOp::ServiceRestart,
    ];
    let mut service_control_ok_before = [0usize; 6];
    let mut service_control_error_before = [0usize; 6];
    let mut service_control_index = 0usize;
    while service_control_index < service_control_ops.len() {
        service_control_ok_before[service_control_index] = syscall_record_count(
            "/bin/cat",
            service_control_ops[service_control_index],
            crate::syscall::SyscallStatus::Ok,
        );
        service_control_error_before[service_control_index] = syscall_record_count(
            "/bin/cat",
            service_control_ops[service_control_index],
            crate::syscall::SyscallStatus::Error,
        );
        service_control_index += 1;
    }
    let continue_errors_before_service_control = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    let mut before_services = [crate::service::EMPTY_SERVICE_RECORD; crate::service::MAX_SERVICES];
    let before_service_count = crate::service::snapshot(&mut before_services);
    testrt::check_eq(
        syscalls.request_init_service("shell", "/bin/sh"),
        Err(crate::service::ServiceError::Busy),
    );
    testrt::check_eq(syscalls.service_ready("editor"), Err(crate::service::ServiceError::Busy));
    testrt::check_eq(syscalls.service_hold(), Err(crate::syscall::ProgramProcessError::Busy));
    testrt::check_eq(
        syscalls.stop_service_by_name("editor"),
        Err(crate::syscall::ProgramServiceError::Busy),
    );
    testrt::check_eq(
        syscalls.start_service_by_name("editor"),
        Err(crate::syscall::ProgramServiceError::Busy),
    );
    testrt::check_eq(
        syscalls.restart_service_by_name("editor"),
        Err(crate::syscall::ProgramServiceError::Busy),
    );
    let mut after_services = [crate::service::EMPTY_SERVICE_RECORD; crate::service::MAX_SERVICES];
    let after_service_count = crate::service::snapshot(&mut after_services);
    testrt::check_eq(after_service_count, before_service_count);
    service_control_index = 0;
    while service_control_index < service_control_ops.len() {
        testrt::check_eq(
            syscall_record_count(
                "/bin/cat",
                service_control_ops[service_control_index],
                crate::syscall::SyscallStatus::Ok,
            ),
            service_control_ok_before[service_control_index],
        );
        testrt::check_eq(
            syscall_record_count(
                "/bin/cat",
                service_control_ops[service_control_index],
                crate::syscall::SyscallStatus::Error,
            ),
            service_control_error_before[service_control_index] + 1,
        );
        service_control_index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_service_control + service_control_ops.len(),
    );

    let source_install_ok_before = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Ok,
    );
    let source_install_error_before = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_source_install = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    install_source_media(SOURCE_MEDIA_PAYLOAD);
    let before_payload_source = fixture_source_store()
        .find_payload("/payload/server-smoke")
        .expect("server-smoke payload source exists");
    testrt::check_eq(before_payload_source.bytes, SAMPLE_PAYLOAD_FAILED_SOURCE_BYTES);
    testrt::check_eq(
        syscalls.install_bin_source_by_name("proc", crate::program::BinSourceInstallStatus::Ok),
        Err(crate::syscall::ProgramSourceInstallError::Busy),
    );
    testrt::check_eq(
        syscalls.install_payload_source_by_name(
            "server-smoke",
            crate::rootd::PayloadSourceInstallStatus::Ready,
        ),
        Err(crate::syscall::ProgramSourceInstallError::Busy),
    );
    testrt::check_eq(
        syscalls.install_bin_source_from_media_by_name("proc"),
        Err(crate::syscall::ProgramSourceInstallError::Busy),
    );
    testrt::check_eq(
        syscalls.install_payload_source_from_media_by_name("server-smoke"),
        Err(crate::syscall::ProgramSourceInstallError::Busy),
    );
    let after_payload_source = fixture_source_store()
        .find_payload("/payload/server-smoke")
        .expect("server-smoke payload source remains visible");
    testrt::check_eq(after_payload_source.bytes, SAMPLE_PAYLOAD_FAILED_SOURCE_BYTES);
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SourceInstall,
            crate::syscall::SyscallStatus::Ok,
        ),
        source_install_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SourceInstall,
            crate::syscall::SyscallStatus::Error,
        ),
        source_install_error_before + 4,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_source_install + 4,
    );

    let mut source_report = reovim_uapi_source::SourceInstallReport::empty();
    let source_name = "server-smoke";
    let raw_source = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::SOURCE_CONTROL,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_source::SourceControlOp::INSTALL_PAYLOAD_STATUS.raw(),
            source_name.as_ptr() as usize,
            source_name.len(),
            reovim_uapi_source::SourceInstallStatusCode::READY.raw(),
            (&mut source_report as *mut reovim_uapi_source::SourceInstallReport) as usize,
            core::mem::size_of::<reovim_uapi_source::SourceInstallReport>(),
        ]),
    );
    testrt::check_eq(raw_source.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    testrt::check_eq(source_report.status(), reovim_uapi_source::SourceInstallStatusCode::NONE);
    clear_source_media();

    let before_dump_sync = crate::dump::status().last_sync;
    testrt::check_eq(before_dump_sync.attempted, false);
    testrt::check_eq(before_dump_sync.reason, "never-synced");
    let stale_dump_status = syscalls.dump_status();
    assert_blocked_dump_status(stale_dump_status);
    let stale_dump_sync = syscalls.dump_sync();
    assert_blocked_dump_sync_status(stale_dump_sync);
    syscalls.write_dump_sync();
    let mut dump_report = reovim_uapi_dump::DumpSyncReport::empty();
    let raw_dump = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUMP_SYNC,
        reovim_uapi_syscall::SyscallArgs::new([
            (&mut dump_report as *mut reovim_uapi_dump::DumpSyncReport) as usize,
            core::mem::size_of::<reovim_uapi_dump::DumpSyncReport>(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(raw_dump.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    testrt::check_eq(dump_report, reovim_uapi_dump::DumpSyncReport::empty());
    testrt::check_eq(crate::dump::status().last_sync, before_dump_sync);
    testrt::check_eq(syscalls.write_kernel_log(), false);
    syscalls.write_kernel_log_view();
    testrt::check_eq(syscalls.external_dmesg(), None);
    let stale_stats = syscalls.kernel_log_stats();
    assert_blocked_kernel_log_stats(stale_stats);
    syscalls.write_kernel_log_stats();
    syscalls.write_dump_status();
    syscalls.write_dump_snapshot();
    syscalls.write_dump_sync();
    testrt::check_eq(sink_str(), "");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::KernelLogRead,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::KernelLogStats,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::DumpStatus,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::DumpSync,
        crate::syscall::SyscallStatus::Error,
    );

    let mut stale_events = [crate::klog::EMPTY_EVENT_RECORD; crate::klog::MAX_EVENTS];
    testrt::check_eq(syscalls.snapshot_kernel_events(&mut stale_events), 0usize);
    sink_clear();
    syscalls.write_kernel_event_table();
    testrt::check_eq(sink_str(), "");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SnapshotKernelEvents,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SnapshotKernelEvents,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    assert_blocked_boot_info(syscalls.boot_info());
    testrt::check_eq(syscalls.devices().len(), 0usize);
    testrt::check_eq(syscalls.profile_name(), "blocked");
    testrt::check_eq(syscalls.launch_enabled(), false);
    assert_blocked_boot_image(syscalls.boot_image());
    assert_blocked_console_input(syscalls.console_input());
    testrt::check_eq(syscalls.payloads().len(), 0usize);
    testrt::check_eq(syscalls.programs().len(), 0usize);
    syscalls.write_boot_info_summary();
    syscalls.write_device_inventory();
    syscalls.write_boot_profile();
    syscalls.write_boot_image();
    syscalls.write_boot_payloads();
    syscalls.write_boot_memory();
    syscalls.write_boot_devices();
    syscalls.write_boot_status();
    syscalls.write_boot_proof();
    syscalls.write_boot_input();
    syscalls.write_mount_table();
    testrt::check_eq(syscalls.write_program_help(None), crate::program::ProgramStatus::Blocked);
    crate::bin_fixture::write_vfs_file(crate::vfs::File::BootHelp, &mut syscalls);
    crate::bin_fixture::write_vfs_file(crate::vfs::File::BinProgram(0), &mut syscalls);
    crate::bin_fixture::write_vfs_file(crate::vfs::File::DevDevice(0), &mut syscalls);
    testrt::check_eq(sink_str(), "");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::BootInfo,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::BootInfo,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::DeviceCatalog,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::DeviceCatalog,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::BootProfile,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::BootProfile,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::BootImage,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::BootImage,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ConsoleInput,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ConsoleInput,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::PayloadCatalog,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::PayloadCatalog,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::VfsMounts,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProgramHelp,
        crate::syscall::SyscallStatus::Error,
    );

    sink_clear();
    syscalls.write_source_media_table();
    testrt::check_eq(sink_str(), "");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SourceMediaSnapshot,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SourceMediaRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecBundleSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecBundleRead,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    assert_blocked_scheduler_snapshot(syscalls.scheduler_snapshot());
    syscalls.write_scheduler_state();
    testrt::check_eq(sink_str(), "");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SchedulerSnapshot,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SchedulerSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SchedulerTick,
        crate::syscall::SyscallStatus::Ok,
    );

    let table_writer_ops = [
        crate::syscall::SyscallOp::SnapshotServices,
        crate::syscall::SyscallOp::SnapshotExecLoads,
        crate::syscall::SyscallOp::SnapshotPendingExecs,
        crate::syscall::SyscallOp::SnapshotSourceStore,
        crate::syscall::SyscallOp::SnapshotTasks,
        crate::syscall::SyscallOp::SnapshotWaits,
        crate::syscall::SyscallOp::SnapshotSyscalls,
        crate::syscall::SyscallOp::SnapshotContinuations,
    ];
    sink_clear();
    syscalls.write_service_table();
    syscalls.write_exec_load_table();
    syscalls.write_pending_exec_table();
    syscalls.write_source_store_table();
    syscalls.write_task_table();
    syscalls.write_wait_table();
    syscalls.write_syscall_table();
    syscalls.write_syscall_continuation_table();
    testrt::check_eq(sink_str(), "");
    let mut table_index = 0usize;
    while table_index < table_writer_ops.len() {
        assert_syscall_record(
            "/bin/cat",
            table_writer_ops[table_index],
            crate::syscall::SyscallStatus::Error,
        );
        assert_no_syscall_record(
            "/bin/cat",
            table_writer_ops[table_index],
            crate::syscall::SyscallStatus::Ok,
        );
        table_index += 1;
    }

    sink_clear();
    syscalls.write_current_process();
    testrt::check_eq(sink_str(), "");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessSelf,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessSelf,
        crate::syscall::SyscallStatus::Ok,
    );

    let provider_probe_ok_before = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::ProviderProbe,
        crate::syscall::SyscallStatus::Ok,
    );
    let provider_probe_error_before = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::ProviderProbe,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_probe_catalog = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    testrt::check_eq(syscalls.run_hardware_probe("fixture"), None);
    let probe_target = "fixture";
    let raw_probe = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PROVIDER_PROBE,
        reovim_uapi_syscall::SyscallArgs::new([
            probe_target.as_ptr() as usize,
            probe_target.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(raw_probe.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    testrt::check_eq(sink_str(), "");
    sink_clear();
    syscalls.write_probe_catalog();
    testrt::check_eq(sink_str(), "");
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::ProviderProbe,
            crate::syscall::SyscallStatus::Ok,
        ),
        provider_probe_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::ProviderProbe,
            crate::syscall::SyscallStatus::Error,
        ),
        provider_probe_error_before + 3,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_probe_catalog + 3,
    );

    syscalls.clear_console();
    let raw_clear = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::TERMINAL_CLEAR,
        reovim_uapi_syscall::SyscallArgs::new([
            reovim_uapi_terminal::PRIMARY_OUTPUT.raw() as usize,
            0,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(raw_clear.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    testrt::check_eq(sink_str(), "");

    let bin_cwd = crate::vfs::normalize("/", "/bin").expect("test cwd path normalizes");
    let session_mutation_ops = [
        crate::syscall::SyscallOp::SessionCwdSet,
        crate::syscall::SyscallOp::SessionShellTarget,
        crate::syscall::SyscallOp::SessionShellStart,
        crate::syscall::SyscallOp::SessionLineDiscipline,
    ];
    let mut session_mutation_ok_before = [0usize; 4];
    let mut session_mutation_error_before = [0usize; 4];
    let mut session_mutation_index = 0usize;
    while session_mutation_index < session_mutation_ops.len() {
        session_mutation_ok_before[session_mutation_index] = syscall_record_count(
            "/bin/cat",
            session_mutation_ops[session_mutation_index],
            crate::syscall::SyscallStatus::Ok,
        );
        session_mutation_error_before[session_mutation_index] = syscall_record_count(
            "/bin/cat",
            session_mutation_ops[session_mutation_index],
            crate::syscall::SyscallStatus::Error,
        );
        session_mutation_index += 1;
    }
    let continue_errors_before_session_mutation = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    syscalls.set_cwd(bin_cwd);
    syscalls.request_shell_target("/bin/sh");
    syscalls.request_shell_start();
    syscalls.request_shell_line_discipline("argv-v1", "single-pipe");
    session_mutation_index = 0;
    while session_mutation_index < session_mutation_ops.len() {
        testrt::check_eq(
            syscall_record_count(
                "/bin/cat",
                session_mutation_ops[session_mutation_index],
                crate::syscall::SyscallStatus::Ok,
            ),
            session_mutation_ok_before[session_mutation_index],
        );
        testrt::check_eq(
            syscall_record_count(
                "/bin/cat",
                session_mutation_ops[session_mutation_index],
                crate::syscall::SyscallStatus::Error,
            ),
            session_mutation_error_before[session_mutation_index] + 1,
        );
        session_mutation_index += 1;
    }
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_session_mutation + session_mutation_ops.len(),
    );
    let tty_read_ok_before = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::TtyReadLine,
        crate::syscall::SyscallStatus::Ok,
    );
    let tty_read_error_before = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::TtyReadLine,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_tty_read = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    TTY_READ_CALLS.store(0, Ordering::Relaxed);
    let mut tty_line = [0x33u8; crate::rootd::ROOT_LINE_BYTES];
    testrt::check_eq(syscalls.read_tty_line(&mut tty_line), 0usize);
    testrt::check_eq(TTY_READ_CALLS.load(Ordering::Relaxed), 0usize);
    testrt::check_eq(tty_line[0], 0x33u8);
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::TtyReadLine,
            crate::syscall::SyscallStatus::Ok,
        ),
        tty_read_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::TtyReadLine,
            crate::syscall::SyscallStatus::Error,
        ),
        tty_read_error_before + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_tty_read + 1,
    );
    testrt::check_eq(syscalls.cwd(), "");
    testrt::check_eq(syscalls.prompt(), "blocked");
    syscalls.write_session_state();
    testrt::check_eq(sink_str(), "");
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SnapshotSession,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SnapshotSession,
        crate::syscall::SyscallStatus::Ok,
    );
    drop(syscalls);
    testrt::check_eq(session.cwd(), "/");
    testrt::check_eq(session.shell_target_requested(), None);
    testrt::check_eq(session.shell_start_requested(), false);
    testrt::check_eq(session.line_discipline_requested(), false);
    testrt::check_eq(session.line_discipline(), "none");
    testrt::check_eq(session.pipe_mode(), "none");
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let mut after_processes = [crate::proc::EMPTY_PROCESS_RECORD; crate::proc::MAX_PROCESSES];
    let after_process_count = crate::proc::snapshot(&mut after_processes);
    testrt::check_eq(after_process_count, before_process_count);
    let mut process_index = 0usize;
    while process_index < after_process_count {
        testrt::check(
            after_processes[process_index].program_path != "/bin/pwd",
            "busy direct helpers do not admit /bin/pwd",
        );
        testrt::check(
            after_processes[process_index].program_path != "/payload/reovim",
            "busy direct helpers do not admit /payload/reovim",
        );
        process_index += 1;
    }
    testrt::check_eq(sink_str(), "");

    let argv = [reovim_uapi_process::ProcessArg::from_str("pwd")];
    let exec_replace_ok_before = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );
    let exec_replace_error_before = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Error,
    );
    let continue_errors_before_raw_execve = syscall_record_count(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    testrt::check_eq(sink_str(), "");
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::ExecReplace,
            crate::syscall::SyscallStatus::Ok,
        ),
        exec_replace_ok_before,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::ExecReplace,
            crate::syscall::SyscallStatus::Error,
        ),
        exec_replace_error_before + 1,
    );
    testrt::check_eq(
        syscall_record_count(
            "/bin/cat",
            crate::syscall::SyscallOp::SyscallContinue,
            crate::syscall::SyscallStatus::Error,
        ),
        continue_errors_before_raw_execve + 1,
    );

    let process = crate::proc::process(current_ctx.pid).expect("current process remains retained");
    testrt::check_eq(process.pid, current_ctx.pid);
    testrt::check_eq(process.task_id, current_ctx.task_id);
    testrt::check_eq(process.program_path, "/bin/cat");
    testrt::check_eq(process.loader, "linked-bin");
    testrt::check_eq(process.entry_name, "bin_cat");
    testrt::check_eq(process.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(process.block_reason, crate::sched::BlockReason::PipeRead);
    testrt::check_eq(process.argv0(), "cat");
    let task = crate::sched::task(current_ctx.task_id).expect("current task remains retained");
    testrt::check_eq(task.entry, "/bin/cat");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Blocked);
    testrt::check_eq(task.block_reason, crate::sched::BlockReason::PipeRead);

    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);
    testrt::check_eq(continuations[0].process_id, current_ctx.pid);
    testrt::check_eq(continuations[0].nr, reovim_uapi_syscall::SyscallNr::READ);
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/payload/reovim",
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::DumpSync,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::DumpSync,
        crate::syscall::SyscallStatus::Unavailable,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProviderProbe,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::TtyClear,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut pending =
        [crate::exec::EMPTY_PENDING_EXEC_RECORD; crate::exec::MAX_PENDING_EXEC_RECORDS];
    let pending_count = crate::exec::snapshot_pending(&mut pending);
    let mut found_pwd_pending = false;
    let mut index = 0usize;
    while index < pending_count {
        if pending[index].pid == current_ctx.pid && pending[index].path == "/bin/pwd" {
            found_pwd_pending = true;
        }
        index += 1;
    }
    testrt::check(!found_pwd_pending, "busy execve does not leave replacement pending");
    reset_installed_sources();
    clear_source_media();
});

arch_test!(program_execve_fails_closed_when_scheduler_task_is_missing, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let current_program = crate::exec::load_bin_program(
        crate::bin_fixture::programs(),
        fixture_source_store(),
        "hello",
    )
    .expect("hello loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("hello"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    crate::sched::reset_kernel_scheduler();
    testrt::check(
        crate::sched::task(current_ctx.task_id).is_none(),
        "scheduler reset removes current task",
    );

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let argv = [reovim_uapi_process::ProcessArg::from_str("pwd")];

    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    testrt::check_eq(sink_str(), "");
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Error,
    );
    assert_no_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );

    let process = crate::proc::process(current_ctx.pid).expect("current process remains retained");
    testrt::check_eq(process.pid, current_ctx.pid);
    testrt::check_eq(process.task_id, current_ctx.task_id);
    testrt::check_eq(process.program_path, "/bin/hello");
    testrt::check_eq(process.loader, "linked-bin");
    testrt::check_eq(process.entry_name, "bin_hello");
    testrt::check_eq(process.state, crate::proc::ProcessState::Running);
    testrt::check_eq(process.argv0(), "hello");

    let mut pending =
        [crate::exec::EMPTY_PENDING_EXEC_RECORD; crate::exec::MAX_PENDING_EXEC_RECORDS];
    let pending_count = crate::exec::snapshot_pending(&mut pending);
    let mut found_pwd_pending = false;
    let mut index = 0usize;
    while index < pending_count {
        if pending[index].pid == current_ctx.pid && pending[index].path == "/bin/pwd" {
            found_pwd_pending = true;
        }
        index += 1;
    }
    testrt::check(!found_pwd_pending, "failed execve does not leave replacement pending");
});

arch_test!(program_execve_closes_close_on_exec_descriptors, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon_with_programs(&CLOEXEC_TEST_PROGRAMS, &CLOEXEC_TEST_PROGRAM_SOURCES);
    let current_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("carrier loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("carrier"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let open_path = b"/boot/profile";
    let open = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            OpenFlags::READ_ONLY.with_close_on_exec().raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(open.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD));

    let argv = [ProcessArg::from_str("fdread"), ProcessArg::from_str("3")];
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");
    assert_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Error,
    );
    drop(syscalls);

    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::ExitCode(1));
    testrt::check_eq(sink_str(), "fd-read-error\n");
    assert_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Error,
    );
});

arch_test!(program_execve_closes_descriptor_after_set_descriptor_flags, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon_with_programs(&CLOEXEC_TEST_PROGRAMS, &CLOEXEC_TEST_PROGRAM_SOURCES);
    let current_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("carrier loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("carrier"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let open_path = b"/boot/profile";
    let open = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(open.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD));
    let flags_before = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_FLAGS_GET,
        reovim_uapi_syscall::SyscallArgs::new([crate::syscall::PROGRAM_VFS_FILE_FD, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(flags_before.decode(), Ok(DescriptorFlags::EMPTY.raw() as usize));
    let set_flags = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_FLAGS_SET,
        reovim_uapi_syscall::SyscallArgs::new([
            crate::syscall::PROGRAM_VFS_FILE_FD,
            DescriptorFlags::CLOSE_ON_EXEC.raw() as usize,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(set_flags.decode(), Ok(0usize));
    let flags_after = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_FLAGS_GET,
        reovim_uapi_syscall::SyscallArgs::new([crate::syscall::PROGRAM_VFS_FILE_FD, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(flags_after.decode(), Ok(DescriptorFlags::CLOSE_ON_EXEC.raw() as usize));

    let argv = [ProcessArg::from_str("fdread"), ProcessArg::from_str("3")];
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::FdFlagsSet,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Error,
    );
    drop(syscalls);

    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::ExitCode(1));
    testrt::check_eq(sink_str(), "fd-read-error\n");
    assert_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Error,
    );
});

arch_test!(program_execve_keeps_dup_descriptor_when_source_was_close_on_exec, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon_with_programs(&CLOEXEC_TEST_PROGRAMS, &CLOEXEC_TEST_PROGRAM_SOURCES);
    let current_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("carrier loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("carrier"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let open_path = b"/boot/profile";
    let open = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            OpenFlags::READ_ONLY.with_close_on_exec().raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(open.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD));
    let dup = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::DUP,
        reovim_uapi_syscall::SyscallArgs::new([crate::syscall::PROGRAM_VFS_FILE_FD, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(dup.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD + 1));

    let argv = [ProcessArg::from_str("fdread"), ProcessArg::from_str("4")];
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");
    assert_no_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    drop(syscalls);

    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_str(), "profile=");
    assert_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_execve_preserves_open_file_description_cursor, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon_with_programs(&CLOEXEC_TEST_PROGRAMS, &CLOEXEC_TEST_PROGRAM_SOURCES);
    let current_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("carrier loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("carrier"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let open_path = b"/boot/profile";
    let open = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    let inherited_fd = open.decode().expect("profile opens");
    testrt::check_eq(inherited_fd, crate::syscall::PROGRAM_VFS_FILE_FD);

    let expected_open = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    let expected_fd = expected_open.decode().expect("second profile opens");
    let mut expected = [0u8; 16];
    let expected_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            expected_fd,
            expected.as_mut_ptr() as usize,
            expected.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(expected_read.decode(), Ok(expected.len()));
    let expected_close = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([expected_fd, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(expected_close.decode(), Ok(0usize));

    let old_image_seek = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::LSEEK,
        reovim_uapi_syscall::SyscallArgs::new([
            inherited_fd,
            8,
            reovim_uapi_fs::SeekWhence::start().raw() as usize,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(old_image_seek.decode(), Ok(8usize));

    let argv = [ProcessArg::from_str("fdread"), ProcessArg::from_str("3")];
    let execve = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    testrt::check_eq(sink_str(), "");
    assert_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    drop(syscalls);

    let replaced_ctx = retained_process_context(current_ctx.pid);
    let status = daemon.run_pending_programs_until(&mut session, replaced_ctx, None);
    testrt::check_eq(status, ProgramStatus::Ok);
    testrt::check_eq(sink_bytes(), &expected[8..16]);
    assert_syscall_record_with_message(
        "/bin/fdread",
        crate::syscall::SyscallOp::SchedulerDispatch,
        crate::syscall::SyscallStatus::Ok,
        "execve replacement scheduler dispatch is recorded",
    );
    assert_syscall_record_with_message(
        "/bin/fdread",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
        "execve replacement fd read is recorded",
    );
    assert_syscall_record_with_message(
        "/bin/fdread",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
        "execve replacement raw exit request is recorded",
    );
    let process = crate::proc::process(current_ctx.pid).expect("replacement process completed");
    testrt::check_eq(process.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(process.exit_code, 0);
    let task = crate::sched::task(current_ctx.task_id).expect("replacement task completed");
    testrt::check_eq(task.state, crate::sched::KernelTaskState::Exited);
});

arch_test!(program_execve_close_on_exec_closes_last_writer_and_wakes_reader_with_eof, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon_with_programs(&CLOEXEC_TEST_PROGRAMS, &CLOEXEC_TEST_PROGRAM_SOURCES);
    let mut session = RootShellSession::new();
    let reader_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("reader carrier loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("carrier"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);

    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let mut pipe_buf = [0u8; 4];
    let empty_read = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[0] as usize,
            pipe_buf.as_mut_ptr() as usize,
            pipe_buf.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(empty_read.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(reader_syscalls);

    let writer_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("writer carrier loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("carrier"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            reader_ctx.pid,
            pipe_fds[1] as usize,
            writer_ctx.pid,
            1,
        ),
        Ok(()),
    );
    testrt::check_eq(
        crate::syscall::close_process_fd_for_pipeline(reader_ctx.pid, pipe_fds[1] as usize),
        Ok(()),
    );

    let reader = crate::proc::process(reader_ctx.pid).expect("blocked reader retained");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::PipeRead);
    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        1usize,
    );
    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);

    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(writer_ctx));
    let _ = crate::syscall::take_pending_exec(writer_ctx.pid);
    let mut writer_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(writer_ctx));
    let set_close_on_exec = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_FLAGS_SET,
        reovim_uapi_syscall::SyscallArgs::new([
            1,
            DescriptorFlags::CLOSE_ON_EXEC.raw() as usize,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(set_close_on_exec.decode(), Ok(0usize));
    let argv = [ProcessArg::from_str("fdread"), ProcessArg::from_str("3")];
    let execve = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    drop(writer_syscalls);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::FdFlagsSet,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );

    testrt::check_eq(
        crate::syscall::program_pipe_read_waiter_count_for_pid(reader_ctx.pid),
        0usize,
    );
    let reader = crate::proc::process(reader_ctx.pid).expect("reader wakes on exec close");
    testrt::check_eq(reader.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(reader.block_reason, crate::sched::BlockReason::None);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let eof = crate::syscall::resume_syscall_continuation(&daemon, &mut session, reader_ctx)
        .expect("reader replay after close-on-exec closes last writer");
    testrt::check_eq(eof.decode(), Ok(0usize));
    testrt::check_eq(pipe_buf, [0u8; 4]);
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    assert_user_resume_blocked(reader_ctx);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Ok,
    );
});

arch_test!(program_execve_close_on_exec_closes_last_reader_and_wakes_writer_error, {
    crate::klog::reset();
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();

    let daemon = daemon_with_programs(&CLOEXEC_TEST_PROGRAMS, &CLOEXEC_TEST_PROGRAM_SOURCES);
    let mut session = RootShellSession::new();
    let writer_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("writer carrier loads");
    let writer_ctx = crate::syscall::exec_bin_from_shell(writer_program, argv1("carrier"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(writer_ctx));
    let _ = crate::syscall::take_pending_exec(writer_ctx.pid);

    let mut writer_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(writer_ctx));
    let mut pipe_fds = [-1i32; 2];
    let pipe = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::PIPE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds.as_mut_ptr() as usize,
            pipe_fds.len(),
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(pipe.decode(), Ok(0usize));

    let reader_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("reader carrier loads");
    let reader_ctx = crate::syscall::exec_bin_from_shell(reader_program, argv1("carrier"));
    testrt::check_eq(
        crate::syscall::duplicate_process_pipe_fd_to_process(
            writer_ctx.pid,
            pipe_fds[0] as usize,
            reader_ctx.pid,
            0,
        ),
        Ok(()),
    );
    testrt::check_eq(
        crate::syscall::close_process_fd_for_pipeline(writer_ctx.pid, pipe_fds[0] as usize),
        Ok(()),
    );

    let fill = [b'f'; crate::program::MAX_PROGRAM_PIPE_BYTES];
    let fill_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            fill.as_ptr() as usize,
            fill.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(fill_write.decode(), Ok(crate::program::MAX_PROGRAM_PIPE_BYTES));

    let extra = [b'X'; 1];
    let blocked_write = writer_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::WRITE,
        reovim_uapi_syscall::SyscallArgs::new([
            pipe_fds[1] as usize,
            extra.as_ptr() as usize,
            extra.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(blocked_write.decode(), Err(reovim_uapi_syscall::SyscallError::BUSY));
    drop(writer_syscalls);

    let writer = crate::proc::process(writer_ctx.pid).expect("blocked writer retained");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::PipeWrite);
    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        1usize,
    );
    let mut continuations = [crate::syscall::EMPTY_SYSCALL_CONTINUATION_RECORD;
        crate::syscall::MAX_SYSCALL_CONTINUATION_RECORDS];
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 1usize);

    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(reader_ctx));
    let _ = crate::syscall::take_pending_exec(reader_ctx.pid);
    let mut reader_syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(reader_ctx));
    let set_close_on_exec = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::FD_FLAGS_SET,
        reovim_uapi_syscall::SyscallArgs::new([
            0,
            DescriptorFlags::CLOSE_ON_EXEC.raw() as usize,
            0,
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(set_close_on_exec.decode(), Ok(0usize));
    let argv = [ProcessArg::from_str("fdread"), ProcessArg::from_str("3")];
    let execve = reader_syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::EXECVE,
        reovim_uapi_syscall::SyscallArgs::new([argv.as_ptr() as usize, argv.len(), 0, 0, 0, 0]),
    );
    testrt::check_eq(execve.decode(), Ok(0usize));
    drop(reader_syscalls);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::FdFlagsSet,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/fdread",
        crate::syscall::SyscallOp::ExecReplace,
        crate::syscall::SyscallStatus::Ok,
    );

    testrt::check_eq(
        crate::syscall::program_pipe_write_waiter_count_for_pid(writer_ctx.pid),
        0usize,
    );
    let writer = crate::proc::process(writer_ctx.pid).expect("writer wakes on exec close");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(writer.block_reason, crate::sched::BlockReason::None);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );

    let status = daemon.run_idle_ready_programs(&mut session);
    testrt::check_eq(status, ProgramStatus::Error);
    testrt::check_eq(crate::syscall::snapshot_syscall_continuations(&mut continuations), 0usize);
    testrt::check_eq(crate::dump::status().syscall_continuation_records, 0usize);
    let writer = crate::proc::process(writer_ctx.pid).expect("writer retained after failed replay");
    testrt::check_eq(writer.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(writer.exit_code, 1);
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::SyscallContinue,
        crate::syscall::SyscallStatus::Error,
    );
    assert_syscall_record(
        "/bin/carrier",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );
});

arch_test!(program_child_spawn_inherits_open_file_description_cursor, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon_with_programs(&CLOEXEC_TEST_PROGRAMS, &CLOEXEC_TEST_PROGRAM_SOURCES);
    let current_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("carrier loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("carrier"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let open_path = b"/boot/profile";
    let open = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    let inherited_fd = open.decode().expect("profile opens");
    testrt::check_eq(inherited_fd, crate::syscall::PROGRAM_VFS_FILE_FD);

    let expected_open = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            OpenFlags::READ_ONLY.raw() as usize,
            0,
            0,
        ]),
    );
    let expected_fd = expected_open.decode().expect("second profile opens");
    let mut expected = [0u8; 24];
    let expected_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            expected_fd,
            expected.as_mut_ptr() as usize,
            expected.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(expected_read.decode(), Ok(expected.len()));
    let expected_close = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::CLOSE,
        reovim_uapi_syscall::SyscallArgs::new([expected_fd, 0, 0, 0, 0, 0]),
    );
    testrt::check_eq(expected_close.decode(), Ok(0usize));

    let mut parent_first = [0u8; 8];
    let parent_first_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            inherited_fd,
            parent_first.as_mut_ptr() as usize,
            parent_first.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(parent_first_read.decode(), Ok(parent_first.len()));
    testrt::check_eq(&parent_first[..], &expected[..8]);

    sink_clear();
    let status = syscalls
        .exec_program_argv_and_wait(argv2("fdread", "3"))
        .expect("child fdread runs");
    testrt::check(status.is_success(), "child fdread exits successfully");
    testrt::check_eq(sink_bytes(), &expected[8..16]);

    let mut parent_after_child = [0u8; 8];
    let parent_after_read = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::READ,
        reovim_uapi_syscall::SyscallArgs::new([
            inherited_fd,
            parent_after_child.as_mut_ptr() as usize,
            parent_after_child.len(),
            0,
            0,
            0,
        ]),
    );
    testrt::check_eq(parent_after_read.decode(), Ok(parent_after_child.len()));
    testrt::check_eq(&parent_after_child[..], &expected[16..24]);
});

arch_test!(program_child_spawn_closes_inherited_close_on_exec_descriptor, {
    crate::proc::reset();
    crate::exec::reset();
    crate::syscall::reset();
    sink_clear();
    let daemon = daemon_with_programs(&CLOEXEC_TEST_PROGRAMS, &CLOEXEC_TEST_PROGRAM_SOURCES);
    let current_program = crate::exec::load_bin_program(
        &CLOEXEC_TEST_PROGRAMS,
        cloexec_test_source_store(),
        "carrier",
    )
    .expect("carrier loads");
    let current_ctx = crate::syscall::exec_bin_from_shell(current_program, argv1("carrier"));
    testrt::check_eq(crate::syscall::dispatch_next_ready_program(), Some(current_ctx));
    let _ = crate::syscall::take_pending_exec(current_ctx.pid).expect("current pending exec");

    let mut session = RootShellSession::new();
    let mut syscalls =
        crate::syscall::ProgramSyscalls::new(&daemon, &mut session, Some(current_ctx));
    let open_path = b"/boot/profile";
    let open = syscalls.dispatch_raw_syscall(
        reovim_uapi_syscall::SyscallNr::OPEN_AT,
        reovim_uapi_syscall::SyscallArgs::new([
            OpenAtDir::session_cwd().raw() as usize,
            open_path.as_ptr() as usize,
            open_path.len(),
            OpenFlags::READ_ONLY.with_close_on_exec().raw() as usize,
            0,
            0,
        ]),
    );
    testrt::check_eq(open.decode(), Ok(crate::syscall::PROGRAM_VFS_FILE_FD));

    let status = syscalls
        .exec_program_argv_and_wait(argv2("fdread", "3"))
        .expect("child fdread dispatches");
    testrt::check(!status.is_success(), "child fdread fails after close-on-exec");
    testrt::check_eq(sink_str(), "fd-read-error\n");
});

arch_test!(root_shell_exec_accepts_absolute_bin_path, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"/bin/pwd\n", None);
    testrt::check_eq(sink_str(), "/\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"shell: /bin/pwd\n");
    assert_contains(sink_bytes(), b"shell.status=ok\n");
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_pwd\n",
    );
});

arch_test!(root_shell_executes_linked_bin_through_raw_syscall_uapi, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"hello\n", None);
    testrt::check_eq(sink_str(), "hello from linked bin\n");
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"shell: hello\n");
    assert_contains(sink_bytes(), b"shell.status=ok\n");
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/hello pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_hello\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"hello extra\n", None);
    testrt::check_eq(sink_str(), "hello: too many arguments\n");
    assert_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_no_syscall_record(
        "/bin/hello",
        crate::syscall::SyscallOp::ProcessExitRequest,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"shell: hello extra\n");
    assert_contains(sink_bytes(), b"shell.status=error\n");
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/hello pid=3 task=3 status=error loader=linked-bin entry_fn=bin_hello\n",
    );
});

arch_test!(root_shell_cwd_programs_run_through_bin_exec, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    let mut session = RootShellSession::new();

    run_session_shell_line(&mut session, b"cd /dev\n");
    testrt::check_eq(sink_str(), "");

    run_session_shell_line(&mut session, b"pwd\n");
    testrt::check_eq(sink_str(), "/dev\n");

    run_session_shell_line(&mut session, b"ls\n");
    testrt::check_eq(sink_str(), "tty\nuart0\n");

    run_session_shell_line(&mut session, b"cat /proc/syscalls\n");
    assert_contains(sink_bytes(), b"path=/bin/cd op=vfs-normalize status=ok");
    assert_contains(sink_bytes(), b"path=/bin/cd op=vfs-lookup status=ok");
    assert_contains(sink_bytes(), b"path=/bin/cd op=session-cwd-set status=ok");
    assert_contains(sink_bytes(), b"path=/bin/pwd op=session-cwd-get status=ok");
    assert_contains(sink_bytes(), b"path=/bin/ls op=vfs-open status=ok");
    assert_contains(sink_bytes(), b"path=/bin/ls op=vfs-lookup status=ok");
    assert_contains(sink_bytes(), b"path=/bin/ls op=vfs-list status=ok");
    assert_contains(
        sink_bytes(),
        b"op=snapshot-syscalls status=ok loader=linked-bin entry_fn=bin_cat\n",
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::VfsOpen,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::FdClose,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/cd pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_cd\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=4 task=4 status=ok loader=linked-bin entry_fn=bin_pwd\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/ls pid=5 task=5 status=ok loader=linked-bin entry_fn=bin_ls\n",
    );
});

arch_test!(root_shell_boot_profile_uses_live_console_input_status, {
    sink_clear();
    let daemon = daemon_with_input_status(
        ProfileSummary::new("shell-only", false),
        Some(diagnostics),
        Some(ready_input_status),
    );
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"cat /boot/profile\n");

    assert_contains(sink_bytes(), b"input=usb-keyboard+uart-fallback\n");
    assert_contains(sink_bytes(), b"input_mode=live\n");
    assert_contains(sink_bytes(), b"usb_keyboard=ready\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"cat /boot/input\n");
    assert_contains(sink_bytes(), b"source=usb-keyboard+uart-fallback\n");
    assert_contains(sink_bytes(), b"source_state=ready\n");
    assert_contains(sink_bytes(), b"mode=live\n");
    assert_contains(sink_bytes(), b"usb_keyboard=ready\n");
    assert_contains(sink_bytes(), b"usb_keyboard_pending_bytes=2\n");
    assert_contains(sink_bytes(), b"usb_keyboard_probe=enabled\n");
    assert_contains(sink_bytes(), b"usb_keyboard_poll_interval_ms=5\n");
    assert_contains(sink_bytes(), b"usb_keyboard_last_poll=report-ready\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"cat /boot/status\n");
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
        (report_pending_input_status, b"manual_next=press-usb-key\n"),
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
        let _ = daemon.run_shell_line(&mut session, b"cat /boot/status\n");
        assert_contains(sink_bytes(), cases[index].1);
        index += 1;
    }
});

arch_test!(root_shell_dmesg_and_unknown_command, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"dmesg\n", None);
    testrt::check_eq(
        sink_str(),
        "dmesg:\nshell: dmesg\nshell.session path=root-shell loader=kernel entry_fn=root_shell line=dmesg\nevent seq=1 component=sched severity=info kind=scheduler-dispatch boot=1 session=1 source=process pid=3 task=3\n",
    );
    assert_syscall_record(
        "/bin/dmesg",
        crate::syscall::SyscallOp::KernelLogRead,
        crate::syscall::SyscallStatus::Ok,
    );
    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/dmesg pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_dmesg\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"dmesg extra\n", None);
    testrt::check_eq(sink_str(), "dmesg: unknown option\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"dmesg a b\n", None);
    testrt::check_eq(sink_str(), "dmesg: too many arguments\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"dmesg --stats\n", None);
    assert_contains(sink_bytes(), b"capacity_bytes=65536\n");
    assert_contains(sink_bytes(), b"retained_bytes=");
    assert_contains(sink_bytes(), b"dropped_bytes=0\n");
    assert_contains(sink_bytes(), b"retained_events=");
    assert_syscall_record(
        "/bin/dmesg",
        crate::syscall::SyscallOp::KernelLogStats,
        crate::syscall::SyscallStatus::Ok,
    );

    crate::klog::reset();
    crate::klog::append_line("rootd: boot report");
    run_shell_line_preserving_log(ProfileSummary::new("shell-only", false), b"dmesg\n", None);
    testrt::check_eq(
        sink_str(),
        "dmesg:\nrootd: boot report\nshell: dmesg\nshell.session path=root-shell loader=kernel entry_fn=root_shell line=dmesg\nevent seq=1 component=sched severity=info kind=scheduler-dispatch boot=1 session=1 source=process pid=4 task=4\n",
    );

    crate::klog::reset();
    crate::klog::append_line("rootd: boot report");
    run_shell_line_preserving_log(
        ProfileSummary::new("shell-only", false),
        b"dmesg\n",
        Some(diagnostics),
    );
    testrt::check_eq(
        sink_str(),
        "dmesg:\nrootd: boot report\nshell: dmesg\nshell.session path=root-shell loader=kernel entry_fn=root_shell line=dmesg\nevent seq=1 component=sched severity=info kind=scheduler-dispatch boot=1 session=1 source=process pid=5 task=5\nexternal diagnostics:\nboot diagnostics complete\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"does-not-exist\n", None);
    testrt::check_eq(sink_str(), "error: unknown /bin program, try `help`\n");
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Error,
    );
    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"cat /log/dmesg\n");
    assert_contains(sink_bytes(), b"shell: does-not-exist\n");
    assert_contains(sink_bytes(), b"shell.status=error\n");
});

arch_test!(root_shell_dump_program_status_snapshot_and_sync, {
    crate::dump::clear_sink_for_tests();
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"dump\n", None);
    assert_contains(sink_bytes(), b"format=reovim-dump-v1\n");
    assert_contains(sink_bytes(), b"persistent=unavailable\n");
    assert_contains(sink_bytes(), b"storage=none\n");
    assert_contains(sink_bytes(), b"storage_capacity_bytes=0\n");
    assert_contains(sink_bytes(), b"last_sync_attempted=false\n");
    assert_contains(sink_bytes(), b"last_sync_reason=never-synced\n");
    assert_contains(sink_bytes(), b"syscall_records=");
    assert_contains(sink_bytes(), b"syscall_continuation_records=");
    assert_syscall_record(
        "/bin/dump",
        crate::syscall::SyscallOp::DumpStatus,
        crate::syscall::SyscallStatus::Ok,
    );
    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/dump pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_dump\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"dump status\n", None);
    assert_contains(sink_bytes(), b"format=reovim-dump-v1\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"dump snapshot\n", None);
    assert_contains(sink_bytes(), b"dump:\n");
    assert_contains(sink_bytes(), b"reovim-dump-v1\n");
    assert_contains(sink_bytes(), b"checksum=");
    assert_contains(sink_bytes(), b"boot:\n");
    assert_contains(sink_bytes(), b"devices:\n");
    assert_contains(sink_bytes(), b"proof:\n");
    assert_contains(sink_bytes(), b"dump-sync:\n");
    assert_contains(sink_bytes(), b"panic:\n");
    assert_contains(sink_bytes(), b"syscalls:\n");
    assert_contains(sink_bytes(), b"continuations:\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"dump sync\n", None);
    testrt::check_eq(
        sink_str(),
        "dump sync:\npersistent=unavailable\nattempted=true\nstorage=none\nstorage_capacity_bytes=0\nstatus=not-written\nreason=no-persistent-dump-sink\n",
    );
    assert_syscall_record(
        "/bin/dump",
        crate::syscall::SyscallOp::DumpSync,
        crate::syscall::SyscallStatus::Unavailable,
    );
    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/dump pid=3 task=3 status=exit-code loader=linked-bin entry_fn=bin_dump\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"dump unknown\n", None);
    testrt::check_eq(sink_str(), "dump: unknown subcommand\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"dump a b\n", None);
    testrt::check_eq(sink_str(), "dump: too many arguments\n");
});

arch_test!(root_shell_rejects_unterminated_quotes, {
    run_shell_line_fixture(
        ProfileSummary::new("shell-only", false),
        b"cat \"/boot/profile\n",
        None,
    );
    testrt::check_eq(sink_str(), "error: unterminated quote\n");

    sink_clear();
    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"cat /log/dmesg\n");
    assert_contains(sink_bytes(), b"shell: cat \"/boot/profile\n");
    assert_contains(sink_bytes(), b"shell.status=error\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"cat '/boot/profile\n", None);
    testrt::check_eq(sink_str(), "error: unterminated quote\n");
});

arch_test!(root_shell_halt_logs_status_before_callback, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    crate::dump::clear_sink_for_tests();
    sink_clear();
    HALT_CALLS.store(0, Ordering::Relaxed);
    let daemon = daemon_with_input_status_and_halt(
        ProfileSummary::new("shell-only", false),
        None,
        None,
        Some(halt_fixture),
    );
    let mut session = RootShellSession::new();

    let should_halt = daemon.run_shell_line(&mut session, b"halt\n");

    testrt::check(should_halt, "halt command requests daemon shutdown");
    testrt::check_eq(sink_str(), "halt: ok\n");
    testrt::check_eq(HALT_CALLS.load(Ordering::Relaxed), 0usize);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains_with_message(sink_bytes(), b"shell: halt\n", "missing halt shell input audit");
    assert_contains_with_message(
        sink_bytes(),
        b"shell.status=halt\n",
        "missing halt shell status audit",
    );
    assert_contains_with_message(
        sink_bytes(),
        b"rootd: halt dump sync\n",
        "missing halt dump-sync start audit",
    );
    assert_contains_with_message(
        sink_bytes(),
        b"rootd.dump_sync persistent=unavailable storage=none storage_capacity_bytes=0 status=not-written bytes=0 checksum=0 verified=false reason=no-persistent-dump-sink\n",
        "missing halt dump-sync result audit",
    );
    assert_contains_with_message(
        sink_bytes(),
        b"exec.path=/bin/halt pid=3 task=3 status=halt loader=linked-bin entry_fn=bin_halt\n",
        "missing linked halt success audit",
    );
    assert_syscall_record(
        "/bin/halt",
        crate::syscall::SyscallOp::SystemHalt,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "rootd",
        crate::syscall::SyscallOp::DumpSync,
        crate::syscall::SyscallStatus::Unavailable,
    );

    crate::klog::reset();
    crate::syscall::reset();
    sink_clear();
    let should_halt = daemon.run_shell_line(&mut session, b"halt now\n");
    testrt::check(!should_halt, "halt with arguments must not stop the daemon");
    testrt::check_eq(sink_str(), "halt: too many arguments\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains_with_message(
        sink_bytes(),
        b"shell: halt now\n",
        "missing halt error shell input audit",
    );
    assert_contains_with_message(
        sink_bytes(),
        b"shell.status=exit-code\n",
        "missing halt exit-code shell status audit",
    );
    assert_contains_with_message(
        sink_bytes(),
        b"exec.path=/bin/halt pid=4 task=4 status=exit-code loader=linked-bin entry_fn=bin_halt\n",
        "missing linked halt exit-code audit",
    );
});

arch_test!(root_shell_probe_uses_lower_provider, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"probe fixture\n", None);
    testrt::check_eq(sink_str(), "probe fixture:\nstate=ready\n");
    assert_syscall_record(
        "/bin/probe",
        crate::syscall::SyscallOp::ProviderProbe,
        crate::syscall::SyscallStatus::Ok,
    );
    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/probe pid=3 task=3 status=ok loader=linked-bin entry_fn=bin_probe\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"probe missing\n", None);
    testrt::check_eq(sink_str(), "probe: unknown target: missing\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"probe\n", None);
    testrt::check_eq(sink_str(), "probe: missing target, try `probe help`\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"probe a b\n", None);
    testrt::check_eq(sink_str(), "probe: too many arguments\n");
});
