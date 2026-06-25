//! Image-packaged `/bin` catalog for `reovim-os`.
//!
//! These are image programs supplied by the OS image. The system kernel owns
//! the `/bin` descriptor ABI and dispatch path; this composition crate supplies
//! the concrete program bodies linked into the current OS image.

use {
    reovim_system_kernel::{
        klog,
        program::{
            self as kernel_program, ProgramDescriptor, ProgramImage, ProgramImageKind,
            ProgramSourceArtifact, ProgramStatus,
        },
        rootd::{BootCheckState, HardwareProbeResult},
        syscall::ProgramSyscalls,
        vfs::{self, File},
    },
    reovim_uapi::system::DeviceEntry,
};

const BIN_HELP: usize = 0;
const BIN_CLEAR: usize = 1;
const BIN_SCREENTEST: usize = 2;
const BIN_PWD: usize = 3;
const BIN_LS: usize = 4;
const BIN_CD: usize = 5;
const BIN_CAT: usize = 6;
const BIN_READ: usize = 7;
const BIN_MOUNT: usize = 8;
const BIN_INPUT: usize = 9;
const BIN_STATUS: usize = 10;
const BIN_PROOF: usize = 11;
const BIN_DEVICE: usize = 12;
const BIN_DMESG: usize = 13;
const BIN_DUMP: usize = 14;
const BIN_SCHED: usize = 15;
const BIN_PROC: usize = 16;
const BIN_PROBE: usize = 17;
const BIN_LAUNCH: usize = 18;
const BIN_REOVIM: usize = 19;
const BIN_HALT: usize = 20;

const BIN_HELP_SOURCE_BYTES: &[u8] = include_bytes!("../bins/help.rvs");
const BIN_PWD_SOURCE_BYTES: &[u8] = include_bytes!("../bins/pwd.rvs");
const BIN_CLEAR_SOURCE_BYTES: &[u8] = include_bytes!("../bins/clear.rvs");
const BIN_SCREENTEST_SOURCE_BYTES: &[u8] = include_bytes!("../bins/screentest.rvs");
const BIN_MOUNT_SOURCE_BYTES: &[u8] = include_bytes!("../bins/mount.rvs");
const BIN_LS_SOURCE_BYTES: &[u8] = include_bytes!("../bins/ls.rvs");
const BIN_CD_SOURCE_BYTES: &[u8] = include_bytes!("../bins/cd.rvs");
const BIN_CAT_SOURCE_BYTES: &[u8] = include_bytes!("../bins/cat.rvs");
const BIN_READ_SOURCE_BYTES: &[u8] = include_bytes!("../bins/read.rvs");
const BIN_DEVICE_SOURCE_BYTES: &[u8] = include_bytes!("../bins/device.rvs");
const BIN_INPUT_SOURCE_BYTES: &[u8] = include_bytes!("../bins/input.rvs");
const BIN_STATUS_SOURCE_BYTES: &[u8] = include_bytes!("../bins/status.rvs");
const BIN_PROOF_SOURCE_BYTES: &[u8] = include_bytes!("../bins/proof.rvs");
const BIN_HALT_SOURCE_BYTES: &[u8] = include_bytes!("../bins/halt.rvs");
const BIN_DMESG_SOURCE_BYTES: &[u8] = include_bytes!("../bins/dmesg.rvs");
const BIN_DUMP_SOURCE_BYTES: &[u8] = include_bytes!("../bins/dump.rvs");
const BIN_SCHED_SOURCE_BYTES: &[u8] = include_bytes!("../bins/sched.rvs");
const BIN_PROC_SOURCE_BYTES: &[u8] = include_bytes!("../bins/proc.rvs");
const BIN_PROBE_SOURCE_BYTES: &[u8] = include_bytes!("../bins/probe.rvs");
const BIN_LAUNCH_SOURCE_BYTES: &[u8] = include_bytes!("../bins/launch.rvs");
const BIN_REOVIM_SOURCE_BYTES: &[u8] = include_bytes!("../bins/reovim.rvs");

const fn source_bytes_bin_program(
    id: usize,
    name: &'static str,
    path: &'static str,
    summary: &'static str,
    entry_name: &'static str,
    _bytes: &'static [u8],
) -> ProgramDescriptor {
    ProgramDescriptor {
        id,
        name,
        path,
        summary,
        image: ProgramImage::SourcePath(path),
        entry_name,
    }
}

const fn source_bytes_bin_artifact(
    path: &'static str,
    bytes: &'static [u8],
) -> ProgramSourceArtifact {
    ProgramSourceArtifact {
        path,
        kind: ProgramImageKind::SourceImage,
        bytes,
    }
}

/// Static `/bin` program catalog shipped by the current `reovim-os` image.
pub const BIN_PROGRAMS: [ProgramDescriptor; 21] = [
    source_bytes_bin_program(
        BIN_HELP,
        "help",
        "/bin/help",
        "show /bin program help",
        "bin_help",
        BIN_HELP_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_CLEAR,
        "clear",
        "/bin/clear",
        "clear framebuffer console and terminal",
        "bin_clear",
        BIN_CLEAR_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_SCREENTEST,
        "screentest",
        "/bin/screentest",
        "print renderer diagnostics",
        "bin_screentest",
        BIN_SCREENTEST_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_PWD,
        "pwd",
        "/bin/pwd",
        "print current kernel VFS directory",
        "bin_pwd",
        BIN_PWD_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_LS,
        "ls",
        "/bin/ls",
        "list a kernel VFS directory",
        "bin_ls",
        BIN_LS_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_CD,
        "cd",
        "/bin/cd",
        "change current kernel VFS directory",
        "bin_cd",
        BIN_CD_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_CAT,
        "cat",
        "/bin/cat",
        "print stdin or kernel VFS pseudo files",
        "bin_cat",
        BIN_CAT_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_READ,
        "read",
        "/bin/read",
        "read one TTY line",
        "bin_read",
        BIN_READ_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_MOUNT,
        "mount",
        "/bin/mount",
        "print kernel VFS mount table",
        "bin_mount",
        BIN_MOUNT_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_INPUT,
        "input",
        "/bin/input",
        "print live console input diagnostics",
        "bin_input",
        BIN_INPUT_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_STATUS,
        "status",
        "/bin/status",
        "print boot, input, and manual_next summary",
        "bin_status",
        BIN_STATUS_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_PROOF,
        "proof",
        "/bin/proof",
        "print physical input proof checklist",
        "bin_proof",
        BIN_PROOF_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_DEVICE,
        "device",
        "/bin/device",
        "print boot memory and device inventory",
        "bin_device",
        BIN_DEVICE_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_DMESG,
        "dmesg",
        "/bin/dmesg",
        "print retained kernel log or ring stats",
        "bin_dmesg",
        BIN_DMESG_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_DUMP,
        "dump",
        "/bin/dump",
        "inspect or flush kernel dump state",
        "bin_dump",
        BIN_DUMP_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_SCHED,
        "sched",
        "/bin/sched",
        "inspect scheduler state, tick, or yield",
        "bin_sched",
        BIN_SCHED_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_PROC,
        "proc",
        "/bin/proc",
        "inspect process state",
        "bin_proc",
        BIN_PROC_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_PROBE,
        "probe",
        "/bin/probe",
        "run a lower hardware probe",
        "bin_probe",
        BIN_PROBE_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_LAUNCH,
        "launch",
        "/bin/launch",
        "list or run registered payloads",
        "bin_launch",
        BIN_LAUNCH_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_REOVIM,
        "reovim",
        "/bin/reovim",
        "run the default reovim payload alias",
        "bin_reovim",
        BIN_REOVIM_SOURCE_BYTES,
    ),
    source_bytes_bin_program(
        BIN_HALT,
        "halt",
        "/bin/halt",
        "request root daemon shutdown",
        "bin_halt",
        BIN_HALT_SOURCE_BYTES,
    ),
];

/// Static `/bin` source artifact store shipped by the current `reovim-os` image.
pub const BIN_PROGRAM_SOURCES: [ProgramSourceArtifact; 21] = [
    source_bytes_bin_artifact("/bin/help", BIN_HELP_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/clear", BIN_CLEAR_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/screentest", BIN_SCREENTEST_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/pwd", BIN_PWD_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/ls", BIN_LS_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/cd", BIN_CD_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/cat", BIN_CAT_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/read", BIN_READ_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/mount", BIN_MOUNT_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/input", BIN_INPUT_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/status", BIN_STATUS_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/proof", BIN_PROOF_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/device", BIN_DEVICE_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/dmesg", BIN_DMESG_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/dump", BIN_DUMP_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/sched", BIN_SCHED_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/proc", BIN_PROC_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/probe", BIN_PROBE_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/launch", BIN_LAUNCH_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/reovim", BIN_REOVIM_SOURCE_BYTES),
    source_bytes_bin_artifact("/bin/halt", BIN_HALT_SOURCE_BYTES),
];

/// Returns the `/bin` program catalog for this image.
#[must_use]
pub const fn programs() -> &'static [ProgramDescriptor] {
    &BIN_PROGRAMS
}

/// Returns the `/bin` source artifact store for this image.
#[must_use]
pub const fn program_sources() -> &'static [ProgramSourceArtifact] {
    &BIN_PROGRAM_SOURCES
}

pub fn write_program_help(
    program_name: Option<&str>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some(program_name) = program_name else {
        write_help_catalog(syscalls);
        return ProgramStatus::Ok;
    };
    let Some((_, program)) = kernel_program::resolve_argv0(syscalls.programs(), program_name)
    else {
        syscalls.stderr_bytes(b"help: unknown program: ");
        syscalls.stderr_bytes(program_name.as_bytes());
        syscalls.stderr_bytes(b"\n");
        return ProgramStatus::Error;
    };
    write_program_help_entry(syscalls, program, b"");
    ProgramStatus::Ok
}

pub fn write_vfs_file(file: File, syscalls: &mut ProgramSyscalls<'_, '_, '_>) {
    match file {
        File::BinProgram(index) => write_program_file(syscalls, index),
        File::BootHelp => write_detailed_help_catalog(syscalls),
        File::BootProfile => write_boot_profile(syscalls),
        File::BootImage => write_boot_image(syscalls),
        File::BootInput => syscalls.write_boot_input(),
        File::BootProof => syscalls.write_boot_proof(),
        File::BootProbes => write_probe_catalog(syscalls),
        File::BootStatus => syscalls.write_boot_status(),
        File::BootMemory => write_boot_memory(syscalls),
        File::BootDevices => write_boot_devices(syscalls),
        File::BootMounts => syscalls.write_mount_table(),
        File::DumpStatus => syscalls.write_dump_status(),
        File::DumpSnapshot => syscalls.write_dump_snapshot(),
        File::LogDmesg => syscalls.write_kernel_log_view(),
        File::LogEvents => write_event_table(syscalls),
        File::LogStats => syscalls.write_kernel_log_stats(),
        File::ProcExecs => syscalls.write_exec_load_table(),
        File::ProcMedia => syscalls.write_source_media_table(),
        File::ProcPending => syscalls.write_pending_exec_table(),
        File::ProcProcesses => syscalls.write_process_table(),
        File::ProcSelf => syscalls.write_current_process(),
        File::ProcScheduler => syscalls.write_scheduler_state(),
        File::ProcSources => syscalls.write_source_store_table(),
        File::ProcSyscalls => syscalls.write_syscall_table(),
        File::ProcTasks => syscalls.write_task_table(),
        File::ProcWaits => syscalls.write_wait_table(),
        File::DevDevice(index) => {
            if let Some(device) = syscalls.devices().get(index) {
                write_device_row(syscalls, device, index);
            }
        }
    }
}

fn write_program_file(syscalls: &ProgramSyscalls<'_, '_, '_>, id: usize) {
    let Some((_, entry)) = kernel_program::find_by_id(syscalls.programs(), id) else {
        syscalls.stdout_line("program metadata unavailable");
        return;
    };
    syscalls.stdout_bytes(b"program=");
    syscalls.stdout_bytes(entry.name.as_bytes());
    syscalls.stdout_bytes(b"\npath=");
    syscalls.stdout_bytes(entry.path.as_bytes());
    syscalls.stdout_bytes(b"\nsummary=");
    syscalls.stdout_bytes(entry.summary.as_bytes());
    syscalls.stdout_bytes(b"\ntype=bin\nloader=");
    syscalls.stdout_bytes(entry.image_kind().as_str().as_bytes());
    syscalls.stdout_bytes(b"\nentry_fn=");
    syscalls.stdout_bytes(entry.entry_name.as_bytes());
    syscalls.stdout_bytes(b"\n");
}

fn write_boot_profile(syscalls: &ProgramSyscalls<'_, '_, '_>) {
    syscalls.stdout_bytes(b"profile=");
    syscalls.stdout_bytes(syscalls.profile_name().as_bytes());
    syscalls.stdout_bytes(b"\nlaunch=");
    write_bool_word(syscalls, syscalls.launch_enabled());
    syscalls.stdout_bytes(b"\npayloads=");
    write_u64_dec(syscalls, syscalls.payloads().len() as u64);
    syscalls.stdout_bytes(b"\nprompt=");
    syscalls.stdout_bytes(syscalls.prompt().as_bytes());
    let input = syscalls.console_input();
    syscalls.stdout_bytes(b"\ninput=");
    syscalls.stdout_bytes(input.source.as_bytes());
    syscalls.stdout_bytes(b"\ninput_mode=");
    syscalls.stdout_bytes(input.mode.as_bytes());
    syscalls.stdout_bytes(b"\nusb_keyboard=");
    syscalls.stdout_bytes(input_state_word(input.usb_keyboard));
    syscalls.stdout_bytes(b"\n");
}

fn write_boot_image(syscalls: &ProgramSyscalls<'_, '_, '_>) {
    let image = syscalls.boot_image();
    syscalls.stdout_bytes(b"package=");
    syscalls.stdout_bytes(image.package.as_bytes());
    syscalls.stdout_bytes(b"\nversion=");
    syscalls.stdout_bytes(image.version.as_bytes());
    syscalls.stdout_bytes(b"\ntarget=");
    syscalls.stdout_bytes(image.target.as_bytes());
    syscalls.stdout_bytes(b"\nselected_profile=");
    syscalls.stdout_bytes(image.selected_profile.as_bytes());
    syscalls.stdout_bytes(b"\nprofile_request=");
    syscalls.stdout_bytes(image.profile_request.as_bytes());
    syscalls.stdout_bytes(b"\nbootline=");
    syscalls.stdout_bytes(image.bootline.as_bytes());
    syscalls.stdout_bytes(b"\nlaunch_profile_feature=");
    syscalls.stdout_bytes(image.launch_profile_feature.as_bytes());
    syscalls.stdout_bytes(b"\n");
}

fn write_boot_memory(syscalls: &ProgramSyscalls<'_, '_, '_>) {
    let info = syscalls.boot_info();
    write_kv_num(syscalls, "ranges", info.memory.range_count() as u64);
    write_kv_num(syscalls, "usable_bytes", info.memory.usable_bytes());
    write_kv_num(syscalls, "cpu_count", info.cpu_count as u64);
    write_kv_num(syscalls, "heap_total_bytes", info.heap_total_bytes);
    write_kv_num(syscalls, "cache_line_bytes", info.cache_line_bytes as u64);
}

fn write_boot_devices(syscalls: &ProgramSyscalls<'_, '_, '_>) {
    let devices = syscalls.devices();
    let mut index = 0usize;
    while index < devices.len() {
        write_device_row(syscalls, &devices[index], index);
        index += 1;
    }
}

fn write_event_table(syscalls: &ProgramSyscalls<'_, '_, '_>) {
    let mut records = [klog::EMPTY_EVENT_RECORD; klog::MAX_EVENTS];
    let count = syscalls.snapshot_kernel_events(&mut records);
    syscalls.stdout_line("events:");
    let mut index = 0usize;
    while index < count {
        let record = records[index];
        syscalls.stdout_bytes(b"- seq=");
        write_u64_dec(syscalls, record.seq as u64);
        syscalls.stdout_bytes(b" boot=");
        write_u64_dec(syscalls, record.boot_id as u64);
        syscalls.stdout_bytes(b" session=");
        write_u64_dec(syscalls, record.session_id as u64);
        syscalls.stdout_bytes(b" source=");
        syscalls.stdout_bytes(record.source.as_bytes());
        syscalls.stdout_bytes(b" component=");
        syscalls.stdout_bytes(record.component.as_bytes());
        syscalls.stdout_bytes(b" severity=");
        syscalls.stdout_bytes(record.severity.as_bytes());
        syscalls.stdout_bytes(b" kind=");
        syscalls.stdout_bytes(record.kind.as_bytes());
        syscalls.stdout_bytes(b" pid=");
        write_u64_dec(syscalls, record.process_id as u64);
        syscalls.stdout_bytes(b" task=");
        write_u64_dec(syscalls, record.task_id as u64);
        syscalls.stdout_bytes(b"\n");
        index += 1;
    }
}

fn write_device_row(syscalls: &ProgramSyscalls<'_, '_, '_>, device: &DeviceEntry, index: usize) {
    syscalls.stdout_bytes(b"- [");
    write_u64_dec(syscalls, index as u64);
    syscalls.stdout_bytes(b"] ");
    syscalls.stdout_bytes(vfs::device_class_name(device.class).as_bytes());
    syscalls.stdout_bytes(b" compat=");
    syscalls.stdout_bytes(device.compatible.as_bytes());
    syscalls.stdout_bytes(b" mmio=");
    write_u64_hex(syscalls, device.mmio_base);
    syscalls.stdout_bytes(b"/");
    write_u64_hex(syscalls, device.mmio_len);
    syscalls.stdout_bytes(b" irq=");
    write_u64_dec(syscalls, device.irq as u64);
    syscalls.stdout_bytes(b"\n");
}

fn write_probe_catalog(syscalls: &ProgramSyscalls<'_, '_, '_>) {
    match syscalls.run_hardware_probe("help") {
        Some(HardwareProbeResult::Handled) => {}
        Some(HardwareProbeResult::UnknownTarget) => {
            syscalls.stdout_line("probe targets:");
            syscalls.stdout_line("  unknown");
        }
        None => {
            syscalls.stdout_line("probe targets:");
            syscalls.stdout_line("  none");
        }
    }
}

fn write_help_catalog(syscalls: &ProgramSyscalls<'_, '_, '_>) {
    syscalls.stdout_line("reovim root shell");
    write_program_names(syscalls);
    syscalls.stdout_line("namespace: /bin");
    syscalls.stdout_line("usage: help [program]");
}

fn write_program_names(syscalls: &ProgramSyscalls<'_, '_, '_>) {
    syscalls.stdout_bytes(b"/bin programs: ");
    let programs = syscalls.programs();
    let mut index = 0usize;
    while index < programs.len() {
        if index > 0 {
            syscalls.stdout_bytes(b", ");
        }
        syscalls.stdout_bytes(programs[index].name.as_bytes());
        index += 1;
    }
    let mut media_programs = [None; kernel_program::MAX_MEDIA_PROGRAMS];
    let count = kernel_program::snapshot_media_programs(&mut media_programs);
    index = 0;
    while index < count {
        if let Some(program) = media_programs[index] {
            if !programs.is_empty() || index > 0 {
                syscalls.stdout_bytes(b", ");
            }
            syscalls.stdout_bytes(program.name.as_bytes());
        }
        index += 1;
    }
    syscalls.stdout_bytes(b"\n");
}

fn write_detailed_help_catalog(syscalls: &ProgramSyscalls<'_, '_, '_>) {
    write_help_catalog(syscalls);
    syscalls.stdout_line("details:");
    let mut index = 0usize;
    let programs = syscalls.programs();
    while index < programs.len() {
        write_program_help_entry(syscalls, &programs[index], b"  ");
        index += 1;
    }
    let mut media_programs = [None; kernel_program::MAX_MEDIA_PROGRAMS];
    let count = kernel_program::snapshot_media_programs(&mut media_programs);
    index = 0;
    while index < count {
        if let Some(program) = media_programs[index] {
            write_program_help_entry(syscalls, program, b"  ");
        }
        index += 1;
    }
}

fn write_program_help_entry(
    syscalls: &ProgramSyscalls<'_, '_, '_>,
    program: &ProgramDescriptor,
    prefix: &[u8],
) {
    match program.id {
        BIN_HELP => write_help_line(syscalls, prefix, "help [program] - show program help"),
        BIN_CLEAR => {
            write_help_line(syscalls, prefix, "clear - clear framebuffer console and terminal")
        }
        BIN_SCREENTEST => {
            write_help_line(syscalls, prefix, "screentest - print renderer diagnostics");
            syscalls.stdout_bytes(if prefix.is_empty() { b"  " } else { b"    " });
            syscalls.stdout_line("required rows: el: clean, el1: clean-left, el2: clean-all");
        }
        BIN_PWD => {
            write_help_line(syscalls, prefix, "pwd - print current kernel VFS directory")
        }
        BIN_LS => {
            write_help_line(syscalls, prefix, "ls [path] - list a kernel VFS directory")
        }
        BIN_CD => {
            write_help_line(syscalls, prefix, "cd [path] - change current kernel VFS directory")
        }
        BIN_CAT => write_help_line(
            syscalls,
            prefix,
            "cat [path...] - print stdin or kernel VFS pseudo files",
        ),
        BIN_READ => write_help_line(syscalls, prefix, "read - read one TTY line"),
        BIN_MOUNT => {
            write_help_line(syscalls, prefix, "mount - print kernel VFS mount table")
        }
        BIN_INPUT => {
            write_help_line(syscalls, prefix, "input - print live console input diagnostics")
        }
        BIN_STATUS => {
            write_help_line(syscalls, prefix, "status - print boot, input, and manual_next summary")
        }
        BIN_PROOF => {
            write_help_line(syscalls, prefix, "proof - print physical input proof checklist")
        }
        BIN_DEVICE => {
            write_help_line(syscalls, prefix, "device - print boot memory and device inventory")
        }
        BIN_DMESG => write_help_line(
            syscalls,
            prefix,
            "dmesg [--stats] - print retained kernel log or ring stats",
        ),
        BIN_DUMP => write_help_line(
            syscalls,
            prefix,
            "dump [status|snapshot|sync] - inspect or flush kernel dump state",
        ),
        BIN_SCHED => write_help_line(
            syscalls,
            prefix,
            "sched [status|tick|yield] - inspect scheduler state, tick, or yield current task",
        ),
        BIN_PROC => write_help_line(
            syscalls,
            prefix,
            "proc [processes|execs|media|pending|self|sources|tasks|waits|syscalls|scheduler|exec PROGRAM [ARG...]|spawn PROGRAM [ARG...]|block PROGRAM [ARG...]|wait PID|wake PID|kill PID|install-bin NAME ok|error|install-payload NAME ready|failed|install-bin-media NAME|install-payload-media NAME] - inspect process state",
        ),
        BIN_PROBE => write_help_line(
            syscalls,
            prefix,
            "probe target - run lower hardware probe; try `probe help` or `cat /boot/probes`",
        ),
        BIN_LAUNCH => {
            write_help_line(syscalls, prefix, "launch [payload] - list or run registered payloads")
        }
        BIN_REOVIM => {
            write_help_line(syscalls, prefix, "reovim - run the default reovim payload alias")
        }
        BIN_HALT => write_help_line(syscalls, prefix, "halt - request root daemon shutdown"),
        _ => {
            syscalls.stdout_bytes(prefix);
            syscalls.stdout_bytes(program.name.as_bytes());
            syscalls.stdout_bytes(b" - ");
            syscalls.stdout_line(program.summary);
        }
    }
}

fn write_help_line(syscalls: &ProgramSyscalls<'_, '_, '_>, prefix: &[u8], line: &str) {
    syscalls.stdout_bytes(prefix);
    syscalls.stdout_line(line);
}

fn input_state_word(state: BootCheckState) -> &'static [u8] {
    match state {
        BootCheckState::Ok => b"ready",
        BootCheckState::Warn => b"unavailable",
    }
}

fn write_bool_word(syscalls: &ProgramSyscalls<'_, '_, '_>, value: bool) {
    syscalls.stdout_bytes(if value { b"enabled" } else { b"disabled" });
}

fn write_kv_num(syscalls: &ProgramSyscalls<'_, '_, '_>, key: &str, value: u64) {
    syscalls.stdout_bytes(key.as_bytes());
    syscalls.stdout_bytes(b"=");
    write_u64_dec(syscalls, value);
    syscalls.stdout_bytes(b"\n");
}

fn write_u64_dec(syscalls: &ProgramSyscalls<'_, '_, '_>, value: u64) {
    let mut buf = [0u8; 24];
    let len = u64_to_dec(value, &mut buf);
    syscalls.stdout_bytes(&buf[..len]);
}

fn write_u64_hex(syscalls: &ProgramSyscalls<'_, '_, '_>, value: u64) {
    let mut buf = [0u8; 18];
    let len = u64_to_hex(value, &mut buf[2..]);
    buf[0] = b'0';
    buf[1] = b'x';
    syscalls.stdout_bytes(&buf[..2 + len]);
}

fn u64_to_dec(mut value: u64, out: &mut [u8]) -> usize {
    if value == 0 {
        out[0] = b'0';
        return 1;
    }

    let mut tmp = [0u8; 20];
    let mut len = 0usize;
    while value > 0 {
        tmp[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
    }

    let mut i = 0usize;
    while len > 0 {
        len -= 1;
        out[i] = tmp[len];
        i += 1;
    }
    i
}

fn u64_to_hex(mut value: u64, out: &mut [u8]) -> usize {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    if value == 0 {
        out[0] = b'0';
        return 1;
    }

    let mut tmp = [0u8; 16];
    let mut len = 0usize;
    while value > 0 {
        tmp[len] = HEX[(value & 0xf) as usize];
        len += 1;
        value >>= 4;
    }

    let mut i = 0usize;
    while len > 0 {
        len -= 1;
        out[i] = tmp[len];
        i += 1;
    }
    i
}
