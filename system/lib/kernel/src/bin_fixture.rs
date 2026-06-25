//! Selftest `/bin` catalog for the generic rootd/exec/VFS ABI.
//!
//! These are image programs supplied by the selftest image. The system kernel
//! owns the `/bin` descriptor ABI and dispatch path; this fixture supplies the
//! concrete program bodies linked into the selftest image.

use {
    crate::{
        klog,
        program::{
            self as kernel_program, ProgramDescriptor, ProgramImage, ProgramImageKind,
            ProgramSourceArtifact, ProgramStatus,
        },
        rootd::{BootCheckState, HardwareProbeResult},
        syscall::ProgramSyscalls,
        vfs::{self, File},
    },
    reovim_uapi_system::DeviceEntry,
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

const BIN_HELP_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 2 help: too many arguments\nwrite-help-arg1-or-catalog\n";
const BIN_CLEAR_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 1 clear: too many arguments\nclear-console\nwrite-stdout-hex 1b5b324a1b5b48\n";
const BIN_SCREENTEST_SOURCE_BYTES: &[u8] = concat!(
    "reovim-source-v1\n",
    "reject-argc-greater 1 screentest: too many arguments\n",
    "write-stdout-hex 73637265656e20746573743a0a\n",
    "write-stdout-hex 20207461726765743a206672616d656275666665722f73657269616c207474792072656e6465726572207375627365740a\n",
    "write-stdout-hex 2020666731363a201b5b33303b34376d626c61636b1b5b306d201b5b33316d7265641b5b306d201b5b33326d677265656e1b5b306d201b5b33336d79656c6c6f771b5b306d201b5b33346d626c75651b5b306d201b5b33356d6d6167656e74611b5b306d201b5b33366d6379616e1b5b306d201b5b33376d77686974651b5b306d0a\n",
    "write-stdout-hex 2020666731362b3a201b5b39306d677261791b5b306d201b5b39316d62722d7265641b5b306d201b5b39326d62722d677265656e1b5b306d201b5b39336d62722d79656c6c6f771b5b306d201b5b39346d62722d626c75651b5b306d201b5b39356d62722d6d6167656e74611b5b306d201b5b39366d62722d6379616e1b5b306d201b5b39373b34306d62722d77686974651b5b306d0a\n",
    "write-stdout-hex 2020626731363a201b5b33373b34306d203430201b5b306d201b5b33303b34316d203431201b5b306d201b5b33303b34326d203432201b5b306d201b5b33303b34336d203433201b5b306d201b5b33373b34346d203434201b5b306d201b5b33303b34356d203435201b5b306d201b5b33303b34366d203436201b5b306d201b5b33303b34376d203437201b5b306d0a\n",
    "write-stdout-hex 2020626731362b3a201b5b33303b3130306d20313030201b5b306d201b5b33303b3130316d20313031201b5b306d201b5b33303b3130326d20313032201b5b306d201b5b33303b3130336d20313033201b5b306d201b5b33303b3130346d20313034201b5b306d201b5b33303b3130356d20313035201b5b306d201b5b33303b3130366d20313036201b5b306d201b5b33303b3130376d20313037201b5b306d0a\n",
    "write-stdout-hex 20206964782d66673a201b5b33383b353b32316d69647832311b5b306d201b5b33383b353b34366d69647834361b5b306d201b5b33383b353b35316d69647835311b5b306d201b5b33383b353b39336d69647839331b5b306d201b5b33383b353b3136306d6964783136301b5b306d201b5b33383b353b3139366d6964783139361b5b306d201b5b33383b353b3230316d6964783230311b5b306d201b5b33383b353b3232366d6964783232361b5b306d0a\n",
    "write-stdout-hex 20206964782d62673a201b5b34383b353b32316d203231201b5b306d201b5b34383b353b34366d203436201b5b306d201b5b34383b353b35316d203531201b5b306d201b5b34383b353b39336d203933201b5b306d201b5b34383b353b3136306d20313630201b5b306d201b5b34383b353b3139366d20313936201b5b306d201b5b34383b353b3230316d20323031201b5b306d201b5b34383b353b3232366d20323236201b5b306d0a\n",
    "write-stdout-hex 20207267622d66673a201b5b33383b323b3235353b39323b38376d7761726d1b5b306d201b5b33383b323b3131343b3231343b38366d677265656e1b5b306d201b5b33383b323b33353b3133323b3235356d736b791b5b306d201b5b33383b323b3139303b3132303b3235356d76696f6c65741b5b306d201b5b33383b323b3235353b3235353b3235353b34383b323b303b303b306d77686974652d6f6e2d626c61636b1b5b306d0a\n",
    "write-stdout-hex 20207267622d62673a201b5b34383b323b3235353b39323b38376d20207761726d20201b5b306d201b5b34383b323b3131343b3231343b38366d2020677265656e20201b5b306d201b5b34383b323b33353b3133323b3235356d2020736b7920201b5b306d201b5b34383b323b3139303b3132303b3235356d202076696f6c657420201b5b306d0a\n",
    "write-stdout-hex 202061747472733a201b5b316d626f6c641b5b32326d201b5b326d64696d1b5b32326d201b5b336d6974616c69631b5b32336d201b5b346d756e6465726c696e651b5b32346d201b5b376d726576657273651b5b32376d201b5b313b346d626f6c642b756e6465726c696e651b5b306d206e6f726d616c0a\n",
    "write-stdout-hex 202072657365743a201b5b33316d7265641b5b33396d2064656661756c742d6667201b5b34383b323b34383b34383b34386d677261792d62671b5b34396d2064656661756c742d6267201b5b313b376d626f6c642d7265761b5b306d20706c61696e0a\n",
    "write-stdout-hex 202063723a206c6566742d736964652d73686f756c642d76616e6973680d202063723a206f7665727772697474656e0a\n",
    "write-stdout-hex 2020656c3a207374616c65207375666669782073686f756c642076616e6973680d2020656c3a20636c65616e1b5b4b0a\n",
    "write-stdout-hex 2020656c313a207072656669782073686f756c642076616e6973681b5b314b0d2020656c313a20636c65616e2d6c6566740a\n",
    "write-stdout-hex 2020656c323a2077686f6c65206c696e652073686f756c642076616e6973681b5b324b0d2020656c323a20636c65616e2d616c6c0a\n",
    "write-stdout-hex 202062733a20414208200843202873686f756c642072656164204143290a\n",
    "write-stdout-hex 2020777261703a20303132333435363738396162636465666768696a6b6c6d6e6f707172737475767778797a20303132333435363738396162636465666768696a6b6c6d6e6f707172737475767778797a20303132333435363738396162636465666768696a6b6c6d6e6f707172737475767778797a20303132333435363738396162636465666768696a6b6c6d6e6f707172737475767778797a20303132333435363738396162636465666768696a6b6c6d6e6f707172737475767778797a20656e640a\n",
    "write-stdout-hex 2020646f6e650a\n",
)
.as_bytes();
const BIN_PWD_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 1 pwd: too many arguments\nwrite-cwd-line\n";
const BIN_LS_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 2 ls: too many arguments\nwrite-vfs-listing-arg1-or-cwd\n";
const BIN_CD_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 2 cd: too many arguments\nset-cwd-arg1-or-root\n";
const BIN_CAT_SOURCE_BYTES: &[u8] = b"reovim-source-v1\nwrite-stdin-or-vfs-files-argv-tail\n";
const BIN_READ_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 1 read: too many arguments\nwrite-tty-line\n";
const BIN_MOUNT_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 1 mount: too many arguments\nwrite-mount-table\n";
const BIN_DEVICE_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 1 device: too many arguments\nwrite-boot-info-summary\nwrite-device-inventory\n";
const BIN_INPUT_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 1 input: too many arguments\nwrite-boot-input\n";
const BIN_STATUS_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 1 status: too many arguments\nwrite-boot-status\n";
const BIN_PROOF_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 1 proof: too many arguments\nwrite-boot-proof\n";
const BIN_HALT_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 1 halt: too many arguments\nwrite-stdout-hex 68616c743a206f6b0a\nexit-status halt\n";
const BIN_DMESG_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 2 dmesg: too many arguments\ndispatch-arg1 dmesg: unknown option\ndefault\nwrite-kernel-log-view\ncase --stats\nwrite-kernel-log-stats\nend-dispatch-arg1\n";
const BIN_DUMP_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 2 dump: too many arguments\ndispatch-arg1 dump: unknown subcommand\ndefault\nwrite-dump-status\ncase status\nwrite-dump-status\ncase snapshot\nwrite-dump-snapshot\ncase sync\nwrite-dump-sync\nexit-status error\nend-dispatch-arg1\n";
const BIN_SCHED_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 2 sched: too many arguments\ndispatch-arg1 sched: unknown subcommand\ndefault\nwrite-scheduler-state\ncase status\nwrite-scheduler-state\ncase tick\nwrite-scheduler-tick\nwrite-scheduler-state\ncase yield\nwrite-scheduler-yield\nwrite-scheduler-state\nend-dispatch-arg1\n";
const BIN_PROC_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater-unless-arg1 2 exec,spawn,block,wait,wake,kill,install-bin,install-payload,install-bin-media,install-payload-media proc: too many arguments\ndispatch-arg1 proc: unknown subcommand\ndefault\nwrite-process-table\ncase processes\nwrite-process-table\ncase execs\nwrite-exec-load-table\ncase media\nwrite-source-media-table\ncase pending\nwrite-pending-exec-table\ncase self\nwrite-process-self\ncase sources\nwrite-source-store-table\ncase tasks\nwrite-task-table\ncase waits\nwrite-wait-table\ncase syscalls\nwrite-syscall-table\ncase scheduler\nwrite-scheduler-state\ncase exec\nproc-exec-argv-tail\ncase spawn\nproc-spawn-argv-tail\ncase block\nproc-block-argv-tail\ncase wait\nproc-wait-pid-arg2\ncase wake\nproc-wake-pid-arg2\ncase kill\nproc-kill-pid-arg2\ncase install-bin\nproc-install-bin-arg2-arg3\ncase install-payload\nproc-install-payload-arg2-arg3\ncase install-bin-media\nproc-install-bin-media-arg2\ncase install-payload-media\nproc-install-payload-media-arg2\nend-dispatch-arg1\n";
const BIN_PROBE_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 2 probe: too many arguments\nrun-provider-probe-arg1\n";
const BIN_LAUNCH_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nrequire-launch-enabled launch disabled for this profile\nlaunch-payload-arg1-or-list\n";
const BIN_REOVIM_SOURCE_BYTES: &[u8] =
    b"reovim-source-v1\nreject-argc-greater 1 reovim: too many arguments\nrequire-launch-enabled reovim disabled for this profile\nwrite-stdout-hex 72656f76696d3a20\nlaunch-payload-name reovim\n";

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

/// Static `/bin` program catalog used by system-kernel selftests.
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

/// Static `/bin` source artifact store used by system-kernel selftests.
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

/// Returns the selftest `/bin` source artifact store.
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
        BIN_PWD => write_help_line(syscalls, prefix, "pwd - print current kernel VFS directory"),
        BIN_LS => write_help_line(syscalls, prefix, "ls [path] - list a kernel VFS directory"),
        BIN_CD => {
            write_help_line(syscalls, prefix, "cd [path] - change current kernel VFS directory")
        }
        BIN_CAT => write_help_line(
            syscalls,
            prefix,
            "cat [path...] - print stdin or kernel VFS pseudo files",
        ),
        BIN_READ => write_help_line(syscalls, prefix, "read - read one TTY line"),
        BIN_MOUNT => write_help_line(syscalls, prefix, "mount - print kernel VFS mount table"),
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
