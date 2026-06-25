//! Selftests for the root daemon shell parser + `/bin` surface.
//!
//! The tests run in the no_std selftest harness and prove the shell can be
//! instantiated from static fixtures with a callback writer.

use {
    super::{RootShellSession, execute_loaded_program_argv},
    crate::{
        block::BlockDevice,
        program::ProgramArgvBuffer,
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
    reovim_uapi_system::{BootInfo, DeviceClass, DeviceEntry},
};

const SINK_CAPACITY: usize = crate::klog::CAPACITY + 4096;

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

fn fixture_source_store() -> ExecutableSourceStore {
    ExecutableSourceStore::new(crate::bin_fixture::program_sources(), sample_payload_sources())
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
const SAMPLE_MEDIA_BIN_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nwrite-stdout-hex 6d656469612d62696e2e6f6b0a\nexit-status ok\n";
const SAMPLE_MEDIA_PAYLOAD_SOURCE_BYTES: &[u8] = b"reovim-payload-source-v1\nexit-status ready\n";
const SAMPLE_BIN_EXIT_CODE_ZERO_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nwrite-stdout-hex 657869742d636f64652d300a\nexit-code 0\n";
const SAMPLE_BIN_EXIT_CODE_SEVEN_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nwrite-stdout-hex 657869742d636f64652d370a\nexit-code 7\n";
const SAMPLE_BIN_INVALID_EXIT_CODE_SOURCE_BYTES: &[u8] = b"reovim-source-v1\nexit-code 256\n";
const SOURCE_MEDIA_BIN: usize = 1;
const SOURCE_MEDIA_PAYLOAD: usize = 2;
const SOURCE_MEDIA_BIN_WRONG_PATH: usize = 3;
const SOURCE_MEDIA_DYNAMIC_BIN: usize = 4;
static SOURCE_MEDIA_KIND: AtomicUsize = AtomicUsize::new(0);

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

fn source_media_read(offset: usize, out: &mut [u8]) -> usize {
    if offset != 0 {
        return 0;
    }
    let (namespace, path, bytes) = match SOURCE_MEDIA_KIND.load(Ordering::Relaxed) {
        SOURCE_MEDIA_BIN => {
            (b"bin".as_slice(), b"/bin/pwd".as_slice(), SAMPLE_MEDIA_BIN_SOURCE_BYTES)
        }
        SOURCE_MEDIA_DYNAMIC_BIN => {
            (b"bin".as_slice(), b"/bin/media-bin".as_slice(), SAMPLE_MEDIA_BIN_SOURCE_BYTES)
        }
        SOURCE_MEDIA_PAYLOAD => (
            b"payload".as_slice(),
            b"/payload/server-smoke".as_slice(),
            SAMPLE_MEDIA_PAYLOAD_SOURCE_BYTES,
        ),
        SOURCE_MEDIA_BIN_WRONG_PATH => {
            (b"bin".as_slice(), b"/bin/help".as_slice(), SAMPLE_MEDIA_BIN_SOURCE_BYTES)
        }
        _ => return 0,
    };
    source_media_encode(out, namespace, path, bytes)
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

fn clear_source_media() {
    SOURCE_MEDIA_KIND.store(0, Ordering::Relaxed);
    crate::block::clear_source_media_device_for_tests();
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
    testrt::check(false, missing_chunk_message(needle));
}

fn assert_syscall_record(
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
            return;
        }
        index += 1;
    }
    testrt::check(false, "missing syscall lifecycle record");
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
    if needle == b"  cat [path...] - print stdin or kernel VFS pseudo files\n" {
        return "missing boot help cat entry";
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
    if needle
        == b"exec.path=/bin/input pid=3 task=3 status=ok loader=source-image entry_fn=bin_input\n"
    {
        return "missing successful source-image /bin/input audit row";
    }
    if needle
        == b"exec.path=/bin/input pid=4 task=4 status=error loader=source-image entry_fn=bin_input\n"
    {
        return "missing rejected source-image /bin/input audit row";
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
        "reovim root shell\n/bin programs: help, init, sh, clear, screentest, pwd, ls, cd, cat, read, mount, input, status, proof, device, dmesg, dump, sched, proc, probe, launch, reovim, halt\nnamespace: /bin\nusage: help [program]\n",
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
        "sched [status|tick|yield] - inspect scheduler state, tick, or yield current task\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help proc\n", None);
    testrt::check_eq(
        sink_str(),
        "proc [processes|execs|media|pending|self|sources|tasks|waits|syscalls|scheduler|exec PROGRAM [ARG...]|spawn PROGRAM [ARG...]|block PROGRAM [ARG...]|wait PID|wake PID|kill PID|install-bin NAME ok|error|install-payload NAME ready|failed|install-bin-media NAME|install-payload-media NAME] - inspect process state\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help device\n", None);
    testrt::check_eq(sink_str(), "device - print boot memory and device inventory\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help launch\n", None);
    testrt::check_eq(sink_str(), "launch [payload] - list or run registered payloads\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help reovim\n", None);
    testrt::check_eq(sink_str(), "reovim - run the default reovim payload alias\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"help halt\n", None);
    testrt::check_eq(sink_str(), "halt - request root daemon shutdown\n");

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

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
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

arch_test!(root_shell_input_runs_as_source_image, {
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
        b"exec.path=/bin/input pid=3 task=3 status=ok loader=source-image entry_fn=bin_input\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"input extra\n", None);
    testrt::check_eq(sink_str(), "input: too many arguments\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/input pid=3 task=3 status=error loader=source-image entry_fn=bin_input\n",
    );
});

arch_test!(root_shell_read_runs_as_source_image, {
    TTY_READ_CALLS.store(0, Ordering::Relaxed);
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"read\n", None);
    testrt::check_eq(sink_str(), "tty fixture input\n");
    assert_syscall_record(
        "/bin/read",
        crate::syscall::SyscallOp::TtyReadLine,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/read pid=3 task=3 status=ok loader=source-image entry_fn=bin_read\n",
    );

    TTY_READ_CALLS.store(0, Ordering::Relaxed);
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"read extra\n", None);
    testrt::check_eq(sink_str(), "read: too many arguments\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/read pid=3 task=3 status=error loader=source-image entry_fn=bin_read\n",
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

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/clear pid=3 task=3 status=ok loader=source-image entry_fn=bin_clear\n",
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

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/screentest pid=3 task=3 status=ok loader=source-image entry_fn=bin_screentest\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"screentest extra\n", None);
    testrt::check_eq(sink_str(), "screentest: too many arguments\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/screentest pid=3 task=3 status=error loader=source-image entry_fn=bin_screentest\n",
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
        b"exec.path=/bin/mount pid=3 task=3 status=ok loader=source-image entry_fn=bin_mount\n",
    );

    run_shell_line_fixture(ProfileSummary::new("appliance", true), b"mount extra\n", None);
    testrt::check_eq(sink_str(), "mount: too many arguments\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/mount pid=3 task=3 status=error loader=source-image entry_fn=bin_mount\n",
    );

    run_shell_line_fixture(ProfileSummary::new("appliance", true), b"reovim\n", None);
    testrt::check_eq(sink_str(), "reovim: payload.ready\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"reovim\n", None);
    testrt::check_eq(sink_str(), "reovim disabled for this profile\n");

    run_shell_line_fixture(ProfileSummary::new("appliance", true), b"reovim extra\n", None);
    testrt::check_eq(sink_str(), "reovim: too many arguments\n");
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

arch_test!(root_shell_proc_installs_payload_source_through_syscall_overlay, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"proc install-payload server-smoke ready\n");
    assert_contains(sink_bytes(), b"proc install-payload:\n");
    assert_contains(sink_bytes(), b"name=server-smoke\nnamespace=payload\n");
    assert_contains(sink_bytes(), b"path=/payload/server-smoke\nstatus=ready\nbytes=");
    assert_contains(sink_bytes(), b"\norigin=installed\n");
    assert_syscall_record(
        "/bin/proc",
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

arch_test!(root_shell_proc_installs_bin_source_through_syscall_overlay, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"proc install-bin pwd ok\n");
    assert_contains(sink_bytes(), b"proc install-bin:\n");
    assert_contains(sink_bytes(), b"name=pwd\nnamespace=bin\n");
    assert_contains(sink_bytes(), b"path=/bin/pwd\nstatus=ok\nbytes=");
    assert_contains(sink_bytes(), b"\norigin=installed\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"proc sources\n");
    assert_contains(sink_bytes(), b"namespace=bin path=/bin/pwd loader=source-image bytes=");
    assert_contains(sink_bytes(), b"origin=installed\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"pwd\n");
    testrt::check_eq(sink_str(), "installed-bin.ok\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
});

arch_test!(root_shell_source_image_exit_code_sets_process_status, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Bin,
            "/bin/pwd",
            SAMPLE_BIN_EXIT_CODE_SEVEN_SOURCE_BYTES,
        ),
        Ok(()),
    );
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let halted = daemon.run_shell_line(&mut session, b"pwd\n");

    testrt::check_eq(halted, false);
    testrt::check_eq(sink_str(), "exit-code-7\n");
    let process = crate::proc::process(3).expect("exit-code process remains retained");
    testrt::check_eq(process.state, crate::proc::ProcessState::Failed);
    testrt::check_eq(process.exit_code, 7);
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Error,
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=3 task=3 status=exit-code loader=source-image entry_fn=bin_pwd\n",
    );

    reset_installed_sources();
});

arch_test!(root_shell_source_image_exit_code_zero_is_pipeline_success, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Bin,
            "/bin/pwd",
            SAMPLE_BIN_EXIT_CODE_ZERO_SOURCE_BYTES,
        ),
        Ok(()),
    );
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let halted = daemon.run_shell_line(&mut session, b"pwd | cat\n");

    testrt::check_eq(halted, false);
    testrt::check_eq(sink_str(), "exit-code-0\n");
    let producer = crate::proc::process(3).expect("pipeline producer remains retained");
    testrt::check_eq(producer.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(producer.exit_code, 0);
    let consumer = crate::proc::process(4).expect("pipeline consumer remains retained");
    testrt::check_eq(consumer.state, crate::proc::ProcessState::Exited);
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
});

arch_test!(root_shell_rejects_invalid_source_image_exit_code_before_spawn, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    testrt::check_eq(
        install_source(
            SourceArtifactNamespace::Bin,
            "/bin/pwd",
            SAMPLE_BIN_INVALID_EXIT_CODE_SOURCE_BYTES,
        ),
        Ok(()),
    );
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let halted = daemon.run_shell_line(&mut session, b"pwd\n");

    testrt::check_eq(halted, false);
    testrt::check_eq(sink_str(), "error: invalid /bin program image\n");
    testrt::check(crate::proc::process(3).is_none(), "invalid image does not spawn a process");
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Error,
    );

    reset_installed_sources();
});

arch_test!(root_shell_proc_installs_bin_source_from_source_media, {
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
    let _ = daemon.run_shell_line(&mut session, b"proc install-bin-media pwd\n");
    assert_contains(sink_bytes(), b"proc install-bin-media:\n");
    assert_contains(sink_bytes(), b"name=pwd\nnamespace=bin\npath=/bin/pwd\n");
    assert_contains(sink_bytes(), b"storage=selftest-source-media0\nstorage_capacity_bytes=512\n");
    assert_contains(sink_bytes(), b"artifact_bytes=");
    assert_contains(sink_bytes(), b"\nbytes=76\nchecksum=1469418292\n");
    assert_contains(sink_bytes(), b"\norigin=installed\nsource=source-media\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SourceMediaRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"proc sources\n");
    assert_contains(sink_bytes(), b"namespace=bin path=/bin/pwd loader=source-image bytes=");
    assert_contains(sink_bytes(), b"origin=installed\n");

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"pwd\n");
    testrt::check_eq(sink_str(), "media-bin.ok\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
    clear_source_media();
});

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
    assert_contains(sink_bytes(), b"- namespace=bin path=/bin/pwd offset=0 artifact_bytes=");
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
    assert_contains(sink_bytes(), b"path=/bin/pwd");
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

arch_test!(root_shell_proc_installs_payload_source_from_source_media, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    clear_source_media();
    install_source_media(SOURCE_MEDIA_PAYLOAD);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("appliance", true), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"proc install-payload-media server-smoke\n");
    assert_contains(sink_bytes(), b"proc install-payload-media:\n");
    assert_contains(
        sink_bytes(),
        b"name=server-smoke\nnamespace=payload\npath=/payload/server-smoke\n",
    );
    assert_contains(sink_bytes(), b"storage=selftest-source-media0\n");
    assert_contains(sink_bytes(), b"artifact_bytes=");
    assert_contains(sink_bytes(), b"\nbytes=43\nchecksum=3609821522\n");
    assert_contains(sink_bytes(), b"\norigin=installed\nsource=source-media\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SourceMediaRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Ok,
    );

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"launch server-smoke\n");
    testrt::check_eq(sink_str(), "launch server-smoke: payload.ready\n");
    assert_syscall_record(
        "/payload/server-smoke",
        crate::syscall::SyscallOp::PayloadRun,
        crate::syscall::SyscallStatus::Ok,
    );

    reset_installed_sources();
    clear_source_media();
});

arch_test!(root_shell_proc_rejects_source_media_path_mismatch, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    reset_installed_sources();
    clear_source_media();
    install_source_media(SOURCE_MEDIA_BIN_WRONG_PATH);
    sink_clear();

    let daemon = daemon(ProfileSummary::new("shell-only", false), None);
    let mut session = RootShellSession::new();
    let _ = daemon.run_shell_line(&mut session, b"proc install-bin-media pwd\n");
    assert_contains(sink_bytes(), b"proc install-bin-media: source-media-path-mismatch\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SourceMediaRead,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SourceInstall,
        crate::syscall::SyscallStatus::Error,
    );

    sink_clear();
    let _ = daemon.run_shell_line(&mut session, b"proc sources\n");
    assert_contains(sink_bytes(), b"namespace=bin path=/bin/pwd loader=source-image bytes=");
    assert_contains(sink_bytes(), b"origin=image\n");

    reset_installed_sources();
    clear_source_media();
});

arch_test!(root_shell_device_runs_as_source_image, {
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
        b"exec.path=/bin/device pid=3 task=3 status=ok loader=source-image entry_fn=bin_device\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"device extra\n", None);
    testrt::check_eq(sink_str(), "device: too many arguments\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/device pid=3 task=3 status=error loader=source-image entry_fn=bin_device\n",
    );
});

arch_test!(root_shell_reovim_records_payload_process_lifecycle, {
    run_shell_line_fixture(ProfileSummary::new("appliance", true), b"reovim\n", None);
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
    testrt::check_eq(records[2].loader, "source-image");
    testrt::check_eq(records[2].entry_name, "bin_reovim");
    testrt::check_eq(records[2].state, crate::proc::ProcessState::Exited);
    testrt::check_eq(records[3].program_path, "/payload/reovim");
    testrt::check_eq(records[3].loader, "source-image");
    testrt::check_eq(records[3].entry_name, "payload_reovim");
    testrt::check_eq(records[3].parent_pid, records[2].pid);
    testrt::check_eq(records[3].state, crate::proc::ProcessState::Exited);
    testrt::check_eq(records[3].exit_code, 0);

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
    testrt::check_eq(tasks[3].state, crate::sched::KernelTaskState::Exited);

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
        b"exec.path=/bin/reovim pid=3 task=3 status=ok loader=source-image entry_fn=bin_reovim\n",
    );
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
            testrt::check_eq(records[index].state, crate::proc::ProcessState::Exited);
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
    run_shell_line_fixture(ProfileSummary::new("appliance", true), b"launch server-smoke\n", None);
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
        crate::syscall::SyscallStatus::Error,
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
    testrt::check_eq(records[2].loader, "source-image");
    testrt::check_eq(records[2].entry_name, "bin_launch");
    testrt::check_eq(records[2].state, crate::proc::ProcessState::Failed);
    testrt::check_eq(records[3].program_path, "/payload/server-smoke");
    testrt::check_eq(records[3].loader, "source-image");
    testrt::check_eq(records[3].entry_name, "payload_server_smoke");
    testrt::check_eq(records[3].parent_pid, records[2].pid);
    testrt::check_eq(records[3].state, crate::proc::ProcessState::Failed);
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
    testrt::check_eq(tasks[3].state, crate::sched::KernelTaskState::Failed);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"payload.exit path=/payload/server-smoke pid=4 task=4 status=payload.failed\n",
    );
    assert_contains(sink_bytes(), b"wait.end parent_pid=3 child_pid=4 child_state=failed exit=1\n");
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/launch pid=3 task=3 status=error loader=source-image entry_fn=bin_launch\n",
    );
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
        b"syscall path=/bin/init op=session-shell-target status=ok target=/bin/sh loader=source-image entry_fn=bin_init\n",
    );
    crate::klog::append_bytes(
        b"syscall path=/bin/sh op=session-shell-start status=ok loader=source-image entry_fn=bin_sh\n",
    );
    crate::klog::append_bytes(
        b"input.usb_keyboard=ready source=usb-keyboard+uart-fallback last_poll=report-ready\n",
    );

    let mut session = RootShellSession::new();
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
        b"  sched [status|tick|yield] - inspect scheduler state, tick, or yield current task\n",
    );
    assert_contains(
        sink_bytes(),
        b"  proc [processes|execs|media|pending|self|sources|tasks|waits|syscalls|scheduler|exec PROGRAM [ARG...]|spawn PROGRAM [ARG...]|block PROGRAM [ARG...]|wait PID|wake PID|kill PID|install-bin NAME ok|error|install-payload NAME ready|failed|install-bin-media NAME|install-payload-media NAME] - inspect process state\n",
    );
    assert_contains(
        sink_bytes(),
        b"  probe target - run lower hardware probe; try `probe help` or `cat /boot/probes`\n",
    );
    assert_contains(sink_bytes(), b"  launch [payload] - list or run registered payloads\n");
    assert_contains(sink_bytes(), b"  reovim - run the default reovim payload alias\n");
    assert_contains(sink_bytes(), b"  halt - request root daemon shutdown\n");

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
    assert_contains(sink_bytes(), b"status=ok loader=source-image entry_fn=bin_proof\n");

    run_session_shell_line(&mut session, b"proof extra\n");
    testrt::check_eq(sink_str(), "proof: too many arguments\n");

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
    assert_contains(sink_bytes(), b"status=ok loader=source-image entry_fn=bin_status\n");

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
    testrt::check_eq(sink_str(), "uart0\n");

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
    testrt::check_eq(sink_str(), "uart0\n");

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
    assert_contains(sink_bytes(), b"loader=source-image\n");
    assert_contains(sink_bytes(), b"entry_fn=bin_help\n");

    run_session_shell_line(&mut session, b"ls /proc\n");
    testrt::check_eq(
        sink_str(),
        "execs\nmedia\npending\nprocesses\nself\nscheduler\nsources\nsyscalls\ntasks\nwaits\n",
    );

    run_session_shell_line(&mut session, b"cat /proc/sources\n");
    assert_contains(sink_bytes(), b"sources:\n");
    assert_contains(sink_bytes(), b"namespace=bin path=/bin/help loader=source-image bytes=");
    assert_contains(
        sink_bytes(),
        b"namespace=payload path=/payload/reovim loader=source-image bytes=",
    );

    run_session_shell_line(&mut session, b"cat /proc/execs\n");
    assert_contains(sink_bytes(), b"execs:\n");
    assert_contains(sink_bytes(), b"argv0=cat status=ok reason=loaded path=/bin/cat");
    assert_contains(sink_bytes(), b"path=/bin/cat source=/bin/cat");
    assert_contains(sink_bytes(), b"loader=source-image entry_fn=bin_cat");
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
    assert_contains(sink_bytes(), b"loader=source-image entry_fn=bin_cat");

    run_session_shell_line(&mut session, b"cat /proc/processes\n");
    assert_contains(sink_bytes(), b"processes:\n");
    assert_contains(sink_bytes(), b"pid=1 ppid=0 task=1 state=running path=rootd");
    assert_contains(sink_bytes(), b"pid=2 ppid=1 task=2 state=running path=root-shell");
    assert_contains(sink_bytes(), b"state=running path=/bin/cat");

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
    assert_contains(sink_bytes(), b"ready_queue_len=0\n");
    assert_contains(sink_bytes(), b"dispatch_count=");
    assert_contains(sink_bytes(), b"yield_count=");
    assert_contains(sink_bytes(), b"tick_count=");

    run_session_shell_line(&mut session, b"cat /proc/syscalls\n");
    assert_contains(sink_bytes(), b"syscalls:\n");
    assert_contains(sink_bytes(), b"path=/bin/cat op=scheduler-dispatch status=ok");
    assert_contains(sink_bytes(), b"path=/bin/cat op=vfs-normalize status=ok");
    assert_contains(sink_bytes(), b"path=/bin/cat op=vfs-lookup status=ok");
    assert_contains(sink_bytes(), b"path=/bin/cat op=vfs-open status=ok");
    assert_contains(sink_bytes(), b"path=/bin/cat op=vfs-read status=ok");
    assert_contains(sink_bytes(), b"path=/bin/cat op=process-self status=ok");
    assert_contains(sink_bytes(), b"path=/bin/cat op=dump-status status=ok");
    assert_contains(sink_bytes(), b"path=/bin/cat op=scheduler-snapshot status=ok");
    assert_contains(
        sink_bytes(),
        b"path=/bin/cat op=snapshot-syscalls status=ok loader=source-image entry_fn=bin_cat\n",
    );

    run_session_shell_line(&mut session, b"cat /proc/waits\n");
    assert_contains(sink_bytes(), b"waits:\n");
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
    assert_contains(sink_bytes(), b"shell.status=ok\n");
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=3 task=3 status=ok loader=source-image entry_fn=bin_pwd\n",
    );
    assert_contains(
        sink_bytes(),
        b"event seq=1 component=proc severity=info kind=program-exit boot=1 session=1 source=process pid=3 task=3\n",
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
    testrt::check_eq(records[2].loader, "source-image");
    testrt::check_eq(records[2].entry_name, "bin_pwd");
    testrt::check_eq(records[2].state, crate::proc::ProcessState::Exited);

    let mut tasks = [crate::sched::EMPTY_KERNEL_TASK_RECORD; crate::sched::MAX_KERNEL_TASKS];
    let task_count = crate::sched::snapshot_kernel_tasks(&mut tasks);
    testrt::check(task_count >= 3, "scheduler table includes command task");
    testrt::check_eq(tasks[2].entry, "/bin/pwd");
    testrt::check_eq(tasks[2].state, crate::sched::KernelTaskState::Exited);
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
});

arch_test!(root_shell_sched_program_reports_and_yields, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"sched\n", None);
    assert_contains(sink_bytes(), b"scheduler:\n");
    assert_contains(sink_bytes(), b"ready_queue_len=0\n");
    assert_contains(sink_bytes(), b"yield_count=0\n");
    assert_syscall_record(
        "/bin/sched",
        crate::syscall::SyscallOp::SchedulerSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"sched tick\n", None);
    assert_contains(
        sink_bytes(),
        b"sched tick:\nticked=true\nstatus=ok\ntask=3\ntick_count=1\ntask_ticks=1\nscheduler:\n",
    );
    assert_contains(sink_bytes(), b"tick_count=1\n");
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
    assert_contains(
        sink_bytes(),
        b"sched yield:\nyielded=false\nstatus=no-peer\nselected_pid=3\nselected_task=3\nscheduler:\n",
    );
    assert_contains(sink_bytes(), b"yield_count=1\n");
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

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"sched unknown\n", None);
    testrt::check_eq(sink_str(), "sched: unknown subcommand\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"sched status extra\n", None);
    testrt::check_eq(sink_str(), "sched: too many arguments\n");
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
    assert_syscall_record(
        "",
        crate::syscall::SyscallOp::SchedulerTick,
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
    testrt::check_eq(snapshot.ready_len, 0usize);
});

arch_test!(root_shell_proc_program_reports_process_state, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc\n", None);
    assert_contains(sink_bytes(), b"processes:\n");
    assert_contains(sink_bytes(), b"state=running path=/bin/proc");
    assert_contains(sink_bytes(), b"loader=source-image entry_fn=bin_proc");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SnapshotProcesses,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc self\n", None);
    assert_contains(sink_bytes(), b"self:\n");
    assert_contains(sink_bytes(), b"state=running path=/bin/proc");
    assert_contains(sink_bytes(), b"loader=source-image entry_fn=bin_proc");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::ProcessSelf,
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

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc waits\n", None);
    assert_contains(sink_bytes(), b"waits:\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SnapshotWaits,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc syscalls\n", None);
    assert_contains(sink_bytes(), b"syscalls:\n");
    assert_contains(
        sink_bytes(),
        b"path=/bin/proc op=snapshot-syscalls status=ok loader=source-image entry_fn=bin_proc\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc execs\n", None);
    assert_contains(sink_bytes(), b"execs:\n");
    assert_contains(sink_bytes(), b"argv0=proc status=ok reason=loaded path=/bin/proc");
    assert_contains(sink_bytes(), b"path=/bin/proc source=/bin/proc");
    assert_contains(sink_bytes(), b"loader=source-image entry_fn=bin_proc");
    assert_contains(sink_bytes(), b"truncated=false kind=bin");
    assert_syscall_record(
        "/bin/proc",
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

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc scheduler\n", None);
    assert_contains(sink_bytes(), b"scheduler:\n");
    assert_contains(sink_bytes(), b"tick_count=");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::SchedulerSnapshot,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc exec pwd\n", None);
    testrt::check_eq(sink_str(), "/\n");
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
        "/bin/proc",
        crate::syscall::SyscallOp::WaitBegin,
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
        "/bin/proc",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );

    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    let wait_count = crate::proc::snapshot_waits(&mut waits);
    testrt::check(wait_count >= 1, "program child exec records a wait");
    testrt::check_eq(waits[0].parent_pid, 3);
    testrt::check_eq(waits[0].child_pid, 4);
    testrt::check_eq(waits[0].child_state, crate::proc::ProcessState::Exited);
    testrt::check_eq(waits[0].completed, true);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"wait.start parent_pid=3 parent_task=3 child_pid=4 child_task=4\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=4 task=4 status=ok loader=source-image entry_fn=bin_pwd\n",
    );
    assert_contains(sink_bytes(), b"wait.end parent_pid=3 child_pid=4 child_state=exited exit=0\n");

    run_shell_line_fixture(
        ProfileSummary::new("shell-only", false),
        b"proc exec cat /boot/profile\n",
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
        crate::syscall::SyscallOp::SpawnChild,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/cat",
        crate::syscall::SyscallOp::VfsLookup,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc unknown\n", None);
    testrt::check_eq(sink_str(), "proc: unknown subcommand\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::FdWrite,
        crate::syscall::SyscallStatus::Ok,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc exec missing\n", None);
    testrt::check_eq(sink_str(), "proc exec: program-not-found\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::ExecLoad,
        crate::syscall::SyscallStatus::Error,
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc exec\n", None);
    testrt::check_eq(sink_str(), "proc exec: missing program\n");

    run_shell_line_fixture(
        ProfileSummary::new("shell-only", false),
        b"proc exec pwd extra\n",
        None,
    );
    testrt::check_eq(sink_str(), "pwd: too many arguments\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc spawn missing\n", None);
    testrt::check_eq(sink_str(), "proc spawn: program-not-found\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"proc spawn\n", None);
    testrt::check_eq(sink_str(), "proc spawn: missing program\n");

    run_shell_line_fixture(
        ProfileSummary::new("shell-only", false),
        b"proc processes extra\n",
        None,
    );
    testrt::check_eq(sink_str(), "proc: too many arguments\n");
});

arch_test!(proc_spawn_leaves_child_ready_for_later_scheduler_dispatch, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"proc spawn pwd\n");
    testrt::check_eq(sink_str(), "proc spawn:\npid=4\npath=/bin/pwd\nstate=ready\n");
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

    let child = crate::proc::process(4).expect("spawned child process exists");
    testrt::check_eq(child.state, crate::proc::ProcessState::Ready);
    testrt::check_eq(child.program_path, "/bin/pwd");
    testrt::check_eq(child.parent_pid, crate::proc::ROOTD_PID);
    let mut waits = [crate::proc::EMPTY_WAIT_RECORD; crate::proc::MAX_WAITS];
    testrt::check_eq(crate::proc::snapshot_waits(&mut waits), 0usize);
    let mut tasks = [crate::sched::EMPTY_KERNEL_TASK_RECORD; crate::sched::MAX_KERNEL_TASKS];
    let task_count = crate::sched::snapshot_kernel_tasks(&mut tasks);
    let mut spawned_task_ready = false;
    let mut spawned_task_parent = 0usize;
    let mut index = 0usize;
    while index < task_count {
        if tasks[index].process_id == 4 {
            spawned_task_ready = tasks[index].state == crate::sched::KernelTaskState::Ready;
            spawned_task_parent = tasks[index].parent_task_id;
        }
        index += 1;
    }
    testrt::check(spawned_task_ready, "spawned child task remains scheduler-ready");
    testrt::check_eq(spawned_task_parent, crate::proc::ROOTD_PID);

    run_session_shell_line(&mut session, b"pwd\n");
    testrt::check_eq(sink_str(), "/\n/\n");
    let child = crate::proc::process(4).expect("spawned child process remains retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Exited);
    let later = crate::proc::process(5).expect("later shell pwd process exists");
    testrt::check_eq(later.state, crate::proc::ProcessState::Exited);

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=4 task=4 status=ok loader=source-image entry_fn=bin_pwd\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=5 task=5 status=ok loader=source-image entry_fn=bin_pwd\n",
    );
});

arch_test!(proc_wait_pid_waits_for_retained_ready_child, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"proc spawn pwd\n");
    testrt::check_eq(sink_str(), "proc spawn:\npid=4\npath=/bin/pwd\nstate=ready\n");

    run_session_shell_line(&mut session, b"proc wait 4\n");
    testrt::check_eq(sink_str(), "/\nproc wait:\npid=4\nstate=exited\nexit=0\ncompleted=true\n");
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::WaitBegin,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/proc",
        crate::syscall::SyscallOp::WaitEnd,
        crate::syscall::SyscallStatus::Ok,
    );
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessExit,
        crate::syscall::SyscallStatus::Ok,
    );

    let child = crate::proc::process(4).expect("waited child process remains retained");
    testrt::check_eq(child.state, crate::proc::ProcessState::Exited);
    testrt::check_eq(child.parent_pid, crate::proc::ROOTD_PID);
    let parent = crate::proc::process(5).expect("waiting proc process remains retained");
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
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=4 task=4 status=ok loader=source-image entry_fn=bin_pwd\n",
    );
    assert_contains(
        sink_bytes(),
        b"wait.start parent_pid=5 parent_task=5 child_pid=4 child_task=4\n",
    );
    assert_contains(sink_bytes(), b"wait.end parent_pid=5 child_pid=4 child_state=exited exit=0\n");
});

arch_test!(proc_block_and_wake_round_trips_child_through_scheduler_ready, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"proc block pwd\n");
    testrt::check_eq(sink_str(), "proc block:\npid=4\npath=/bin/pwd\nstate=blocked\n");
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
        crate::syscall::SyscallOp::ProcessBlock,
        crate::syscall::SyscallStatus::Ok,
    );

    let child = crate::proc::process(4).expect("blocked child process exists");
    testrt::check_eq(child.state, crate::proc::ProcessState::Blocked);
    testrt::check_eq(child.program_path, "/bin/pwd");
    testrt::check_eq(child.parent_pid, crate::proc::ROOTD_PID);
    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.ready_len, 0usize);
    let mut pending =
        [crate::exec::EMPTY_PENDING_EXEC_RECORD; crate::exec::MAX_PENDING_EXEC_RECORDS];
    testrt::check_eq(crate::exec::snapshot_pending(&mut pending), 1usize);
    testrt::check_eq(pending[0].pid, 4usize);
    testrt::check_eq(pending[0].parent_pid, crate::proc::ROOTD_PID);
    testrt::check_eq(pending[0].path, "/bin/pwd");
    testrt::check_eq(pending[0].kind, crate::exec::ExecLoadKind::Bin);

    run_session_shell_line(&mut session, b"proc wake 4\n");
    testrt::check_eq(sink_str(), "proc wake:\npid=4\npath=/bin/pwd\nstate=ready\n");
    assert_syscall_record(
        "/bin/pwd",
        crate::syscall::SyscallOp::ProcessWake,
        crate::syscall::SyscallStatus::Ok,
    );
    let child = crate::proc::process(4).expect("woken child process exists");
    testrt::check_eq(child.state, crate::proc::ProcessState::Ready);
    let snapshot = crate::sched::snapshot_scheduler();
    testrt::check_eq(snapshot.ready_len, 1usize);
    testrt::check_eq(snapshot.next_ready_task_id(), child.task_id);

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
        b"exec.path=/bin/pwd pid=4 task=4 status=ok loader=source-image entry_fn=bin_pwd\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=6 task=6 status=ok loader=source-image entry_fn=bin_pwd\n",
    );
});

arch_test!(proc_tables_expose_block_reason_for_blocked_child, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"proc block pwd\n");
    testrt::check_eq(sink_str(), "proc block:\npid=4\npath=/bin/pwd\nstate=blocked\n");

    run_session_shell_line(&mut session, b"proc processes\n");
    assert_contains(
        sink_bytes(),
        b"state=blocked path=/bin/pwd exit=0 loader=source-image entry_fn=bin_pwd block=operator",
    );

    run_session_shell_line(&mut session, b"proc tasks\n");
    assert_contains(sink_bytes(), b"state=blocked entry=/bin/pwd block=operator");
});

arch_test!(proc_kill_terminates_blocked_child_without_dispatching_it, {
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();
    sink_clear();

    let mut session = RootShellSession::new();
    run_session_shell_line(&mut session, b"proc block pwd\n");
    testrt::check_eq(sink_str(), "proc block:\npid=4\npath=/bin/pwd\nstate=blocked\n");

    run_session_shell_line(&mut session, b"proc kill 4\n");
    testrt::check_eq(sink_str(), "proc kill:\npid=4\npath=/bin/pwd\nstate=failed\n");
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
    testrt::check_eq(child.parent_pid, crate::proc::ROOTD_PID);
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
        b"exec.path=/bin/pwd pid=4 task=4 status=error loader=source-image entry_fn=bin_pwd\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=6 task=6 status=ok loader=source-image entry_fn=bin_pwd\n",
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
            testrt::check_eq(records[index].state, crate::proc::ProcessState::Exited);
            testrt::check_eq(records[index].exit_code, 0);
        }
        if records[index].program_path == "/bin/pwd" {
            found_pwd = true;
            testrt::check_eq(records[index].state, crate::proc::ProcessState::Exited);
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
    testrt::check_eq(seeded.read_fd(0, &mut read_buf), Ok(3usize));
    testrt::check_eq(&read_buf[..3], b"abc");
    testrt::check_eq(seeded.read_fd(0, &mut read_buf), Ok(0usize));

    let fd = seeded
        .open_vfs_file_path("/boot/profile")
        .expect("boot profile opens as a VFS fd");
    testrt::check_eq(fd, crate::syscall::PROGRAM_VFS_FILE_FD);
    let mut file_buf = [0u8; 16];
    let read = seeded
        .read_fd(fd, &mut file_buf)
        .expect("opened VFS fd reads bytes");
    testrt::check_eq(read, 16usize);
    testrt::check_eq(&file_buf[..8], b"profile=");
    testrt::check_eq(
        seeded.write_fd(fd, b"nope").err(),
        Some(crate::syscall::ProgramIoError::NotWritable),
    );
    testrt::check_eq(seeded.close_fd(fd), Ok(()));
    testrt::check_eq(
        seeded.read_fd(fd, &mut file_buf).err(),
        Some(crate::syscall::ProgramIoError::BadFd),
    );
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

arch_test!(root_shell_exec_accepts_absolute_bin_path, {
    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"/bin/pwd\n", None);
    testrt::check_eq(sink_str(), "/\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"shell: /bin/pwd\n");
    assert_contains(sink_bytes(), b"shell.status=ok\n");
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=3 task=3 status=ok loader=source-image entry_fn=bin_pwd\n",
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
    testrt::check_eq(sink_str(), "uart0\n");

    run_session_shell_line(&mut session, b"cat /proc/syscalls\n");
    assert_contains(sink_bytes(), b"path=/bin/cd op=vfs-normalize status=ok");
    assert_contains(sink_bytes(), b"path=/bin/cd op=vfs-lookup status=ok");
    assert_contains(sink_bytes(), b"path=/bin/cd op=session-cwd-set status=ok");
    assert_contains(sink_bytes(), b"path=/bin/pwd op=session-cwd-get status=ok");
    assert_contains(sink_bytes(), b"path=/bin/ls op=session-cwd-get status=ok");
    assert_contains(sink_bytes(), b"path=/bin/ls op=vfs-lookup status=ok");
    assert_contains(sink_bytes(), b"path=/bin/ls op=vfs-list status=ok");
    assert_contains(
        sink_bytes(),
        b"path=/bin/cat op=snapshot-syscalls status=ok loader=source-image entry_fn=bin_cat\n",
    );

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/cd pid=3 task=3 status=ok loader=source-image entry_fn=bin_cd\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/pwd pid=4 task=4 status=ok loader=source-image entry_fn=bin_pwd\n",
    );
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/ls pid=5 task=5 status=ok loader=source-image entry_fn=bin_ls\n",
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
    testrt::check_eq(sink_str(), "dmesg:\nshell: dmesg\n");
    assert_syscall_record(
        "/bin/dmesg",
        crate::syscall::SyscallOp::KernelLogRead,
        crate::syscall::SyscallStatus::Ok,
    );
    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/dmesg pid=3 task=3 status=ok loader=source-image entry_fn=bin_dmesg\n",
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
    testrt::check_eq(sink_str(), "dmesg:\nrootd: boot report\nshell: dmesg\n");

    crate::klog::reset();
    crate::klog::append_line("rootd: boot report");
    run_shell_line_preserving_log(
        ProfileSummary::new("shell-only", false),
        b"dmesg\n",
        Some(diagnostics),
    );
    testrt::check_eq(
        sink_str(),
        "dmesg:\nrootd: boot report\nshell: dmesg\nexternal diagnostics:\nboot diagnostics complete\n",
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
    crate::klog::reset();
    crate::proc::reset();
    crate::syscall::reset();

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"dump\n", None);
    assert_contains(sink_bytes(), b"format=reovim-dump-v1\n");
    assert_contains(sink_bytes(), b"persistent=unavailable\n");
    assert_contains(sink_bytes(), b"storage=none\n");
    assert_contains(sink_bytes(), b"storage_capacity_bytes=0\n");
    assert_contains(sink_bytes(), b"syscall_records=");
    assert_syscall_record(
        "/bin/dump",
        crate::syscall::SyscallOp::DumpStatus,
        crate::syscall::SyscallStatus::Ok,
    );
    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/dump pid=3 task=3 status=ok loader=source-image entry_fn=bin_dump\n",
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
    assert_contains(sink_bytes(), b"panic:\n");
    assert_contains(sink_bytes(), b"syscalls:\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"dump sync\n", None);
    testrt::check_eq(
        sink_str(),
        "dump sync:\npersistent=unavailable\nstorage=none\nstorage_capacity_bytes=0\nstatus=not-written\nreason=no-persistent-dump-sink\n",
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
        b"exec.path=/bin/dump pid=3 task=3 status=error loader=source-image entry_fn=bin_dump\n",
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
    assert_contains(sink_bytes(), b"shell: halt\n");
    assert_contains(sink_bytes(), b"shell.status=halt\n");
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/halt pid=3 task=3 status=halt loader=source-image entry_fn=bin_halt\n",
    );

    crate::klog::reset();
    sink_clear();
    let should_halt = daemon.run_shell_line(&mut session, b"halt now\n");
    testrt::check(!should_halt, "halt with arguments must not stop the daemon");
    testrt::check_eq(sink_str(), "halt: too many arguments\n");

    sink_clear();
    let _ = crate::klog::write_to(sink_write);
    assert_contains(sink_bytes(), b"shell: halt now\n");
    assert_contains(sink_bytes(), b"shell.status=error\n");
    assert_contains(
        sink_bytes(),
        b"exec.path=/bin/halt pid=4 task=4 status=error loader=source-image entry_fn=bin_halt\n",
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
        b"exec.path=/bin/probe pid=3 task=3 status=ok loader=source-image entry_fn=bin_probe\n",
    );

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"probe missing\n", None);
    testrt::check_eq(sink_str(), "probe: unknown target: missing\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"probe\n", None);
    testrt::check_eq(sink_str(), "probe: missing target, try `probe help`\n");

    run_shell_line_fixture(ProfileSummary::new("shell-only", false), b"probe a b\n", None);
    testrt::check_eq(sink_str(), "probe: too many arguments\n");
});
