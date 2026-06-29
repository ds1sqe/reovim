//! Image-packaged `/bin` catalog for `reovim-os`.
//!
//! These are image programs supplied by the OS image. The system kernel owns
//! the `/bin` descriptor ABI and dispatch path; this composition crate supplies
//! the concrete program bodies linked into the current OS image.

#[path = "bin_handlers.rs"]
mod handlers;

use reovim_system_kernel::program::{
    LinkedProgramEntry, ProgramDescriptor, ProgramImage, ProgramSourceArtifact,
};


const BIN_HELP: usize = 0;
const BIN_INIT: usize = 1;
const BIN_SH: usize = 2;
const BIN_CLEAR: usize = 3;
const BIN_SCREENTEST: usize = 4;
const BIN_PWD: usize = 5;
const BIN_LS: usize = 6;
const BIN_CD: usize = 7;
const BIN_CAT: usize = 8;
const BIN_READ: usize = 9;
const BIN_MOUNT: usize = 10;
const BIN_INPUT: usize = 11;
const BIN_STATUS: usize = 12;
const BIN_PROOF: usize = 13;
const BIN_DEVICE: usize = 14;
const BIN_DMESG: usize = 15;
const BIN_DUMP: usize = 16;
const BIN_SCHED: usize = 17;
const BIN_PROC: usize = 18;
const BIN_PROBE: usize = 19;
const BIN_LAUNCH: usize = 20;
const BIN_REOVIM: usize = 21;
const BIN_HELLO: usize = 22;
const BIN_HALT: usize = 23;
const BIN_PS: usize = 24;
const BIN_KILL: usize = 25;
const BIN_WAKE: usize = 26;
const BIN_BLOCK: usize = 27;
const BIN_SPAWN: usize = 28;
const BIN_SLEEP: usize = 29;
const BIN_WAIT: usize = 30;
const BIN_WAIT_TICKS: usize = 31;
const BIN_EXEC: usize = 32;
const BIN_SERVICE_STOP: usize = 33;
const BIN_SERVICE_START: usize = 34;
const BIN_SERVICE_RESTART: usize = 35;
const BIN_SESSION: usize = 36;
const BIN_SERVICES: usize = 37;
const BIN_TASKS: usize = 38;
const BIN_WAITS: usize = 39;
const BIN_SYSCALLS: usize = 40;
const BIN_CONTINUATIONS: usize = 41;
const BIN_EXECS: usize = 42;
const BIN_PENDING: usize = 43;
const BIN_SOURCES: usize = 44;
const BIN_MEDIA: usize = 45;
const BIN_SELF: usize = 46;
const BIN_INSTALL_BIN: usize = 47;
const BIN_INSTALL_PAYLOAD: usize = 48;
const BIN_INSTALL_BIN_MEDIA: usize = 49;
const BIN_INSTALL_PAYLOAD_MEDIA: usize = 50;

const fn linked_bin_program(
    id: usize,
    name: &'static str,
    path: &'static str,
    summary: &'static str,
    entry_name: &'static str,
    entry: LinkedProgramEntry,
) -> ProgramDescriptor {
    ProgramDescriptor {
        id,
        name,
        path,
        summary,
        image: ProgramImage::Linked(entry),
        entry_name,
    }
}

/// Static `/bin` program catalog shipped by the current `reovim-os` image.
pub const BIN_PROGRAMS: [ProgramDescriptor; 51] = [
    linked_bin_program(
        BIN_HELP,
        "help",
        "/bin/help",
        "show /bin program help",
        "bin_help",
        handlers::bin_help,
    ),
    linked_bin_program(
        BIN_INIT,
        "init",
        "/bin/init",
        "start userland session services",
        "bin_init",
        handlers::bin_init,
    ),
    linked_bin_program(BIN_SH, "sh", "/bin/sh", "run interactive root shell", "bin_sh", handlers::bin_sh),
    linked_bin_program(
        BIN_CLEAR,
        "clear",
        "/bin/clear",
        "clear framebuffer console and terminal",
        "bin_clear",
        handlers::bin_clear,
    ),
    linked_bin_program(
        BIN_SCREENTEST,
        "screentest",
        "/bin/screentest",
        "print renderer diagnostics",
        "bin_screentest",
        handlers::bin_screentest,
    ),
    linked_bin_program(
        BIN_PWD,
        "pwd",
        "/bin/pwd",
        "print current kernel VFS directory",
        "bin_pwd",
        handlers::bin_pwd,
    ),
    linked_bin_program(BIN_LS, "ls", "/bin/ls", "list a kernel VFS directory", "bin_ls", handlers::bin_ls),
    linked_bin_program(
        BIN_CD,
        "cd",
        "/bin/cd",
        "change current kernel VFS directory",
        "bin_cd",
        handlers::bin_cd,
    ),
    linked_bin_program(
        BIN_CAT,
        "cat",
        "/bin/cat",
        "print stdin or kernel VFS pseudo files",
        "bin_cat",
        handlers::bin_cat,
    ),
    linked_bin_program(BIN_READ, "read", "/bin/read", "read one TTY line", "bin_read", handlers::bin_read),
    linked_bin_program(
        BIN_MOUNT,
        "mount",
        "/bin/mount",
        "print kernel VFS mount table",
        "bin_mount",
        handlers::bin_mount,
    ),
    linked_bin_program(
        BIN_INPUT,
        "input",
        "/bin/input",
        "print live console input diagnostics",
        "bin_input",
        handlers::bin_input,
    ),
    linked_bin_program(
        BIN_STATUS,
        "status",
        "/bin/status",
        "print boot, input, and manual_next summary",
        "bin_status",
        handlers::bin_status,
    ),
    linked_bin_program(
        BIN_PROOF,
        "proof",
        "/bin/proof",
        "print physical input proof checklist",
        "bin_proof",
        handlers::bin_proof,
    ),
    linked_bin_program(
        BIN_DEVICE,
        "device",
        "/bin/device",
        "print boot memory and device inventory",
        "bin_device",
        handlers::bin_device,
    ),
    linked_bin_program(
        BIN_DMESG,
        "dmesg",
        "/bin/dmesg",
        "print retained kernel log or ring stats",
        "bin_dmesg",
        handlers::bin_dmesg,
    ),
    linked_bin_program(
        BIN_DUMP,
        "dump",
        "/bin/dump",
        "inspect or flush kernel dump state",
        "bin_dump",
        handlers::bin_dump,
    ),
    linked_bin_program(
        BIN_SCHED,
        "sched",
        "/bin/sched",
        "inspect scheduler state, tick, yield, or sleep",
        "bin_sched",
        handlers::bin_sched,
    ),
    linked_bin_program(
        BIN_PROC,
        "proc",
        "/bin/proc",
        "inspect process state",
        "bin_proc",
        handlers::bin_proc,
    ),
    linked_bin_program(
        BIN_PROBE,
        "probe",
        "/bin/probe",
        "run a lower hardware probe",
        "bin_probe",
        handlers::bin_probe,
    ),
    linked_bin_program(
        BIN_LAUNCH,
        "launch",
        "/bin/launch",
        "list or run registered payloads",
        "bin_launch",
        handlers::bin_launch,
    ),
    linked_bin_program(
        BIN_REOVIM,
        "reovim",
        "/bin/reovim",
        "run the default reovim payload alias",
        "bin_reovim",
        handlers::bin_reovim,
    ),
    linked_bin_program(
        BIN_HELLO,
        "hello",
        "/bin/hello",
        "print a linked-bin syscall proof",
        "bin_hello",
        handlers::bin_hello,
    ),
    linked_bin_program(
        BIN_HALT,
        "halt",
        "/bin/halt",
        "request root daemon shutdown",
        "bin_halt",
        handlers::bin_halt,
    ),
    linked_bin_program(BIN_PS, "ps", "/bin/ps", "print retained process table", "bin_ps", handlers::bin_ps),
    linked_bin_program(
        BIN_KILL,
        "kill",
        "/bin/kill",
        "terminate a retained ready or blocked process",
        "bin_kill",
        handlers::bin_kill,
    ),
    linked_bin_program(
        BIN_WAKE,
        "wake",
        "/bin/wake",
        "wake an operator-blocked process",
        "bin_wake",
        handlers::bin_wake,
    ),
    linked_bin_program(
        BIN_BLOCK,
        "block",
        "/bin/block",
        "spawn a retained blocked process",
        "bin_block",
        handlers::bin_block,
    ),
    linked_bin_program(
        BIN_SPAWN,
        "spawn",
        "/bin/spawn",
        "spawn a retained ready process",
        "bin_spawn",
        handlers::bin_spawn,
    ),
    linked_bin_program(
        BIN_SLEEP,
        "sleep",
        "/bin/sleep",
        "spawn a retained process blocked until scheduler ticks",
        "bin_sleep",
        handlers::bin_sleep,
    ),
    linked_bin_program(
        BIN_WAIT,
        "wait",
        "/bin/wait",
        "wait for a retained process",
        "bin_wait",
        handlers::bin_wait,
    ),
    linked_bin_program(
        BIN_WAIT_TICKS,
        "wait-ticks",
        "/bin/wait-ticks",
        "wait for a retained process with scheduler ticks",
        "bin_wait_ticks",
        handlers::bin_wait_ticks,
    ),
    linked_bin_program(
        BIN_EXEC,
        "exec",
        "/bin/exec",
        "replace current process image",
        "bin_exec",
        handlers::bin_exec,
    ),
    linked_bin_program(
        BIN_SERVICE_STOP,
        "service-stop",
        "/bin/service-stop",
        "stop a retained resident service",
        "bin_service_stop",
        handlers::bin_service_stop,
    ),
    linked_bin_program(
        BIN_SERVICE_START,
        "service-start",
        "/bin/service-start",
        "start a retained payload service",
        "bin_service_start",
        handlers::bin_service_start,
    ),
    linked_bin_program(
        BIN_SERVICE_RESTART,
        "service-restart",
        "/bin/service-restart",
        "restart a retained payload service",
        "bin_service_restart",
        handlers::bin_service_restart,
    ),
    linked_bin_program(
        BIN_SESSION,
        "session",
        "/bin/session",
        "print active shell session state",
        "bin_session",
        handlers::bin_session,
    ),
    linked_bin_program(
        BIN_SERVICES,
        "services",
        "/bin/services",
        "print retained service table",
        "bin_services",
        handlers::bin_services,
    ),
    linked_bin_program(
        BIN_TASKS,
        "tasks",
        "/bin/tasks",
        "print retained task table",
        "bin_tasks",
        handlers::bin_tasks,
    ),
    linked_bin_program(
        BIN_WAITS,
        "waits",
        "/bin/waits",
        "print retained wait table",
        "bin_waits",
        handlers::bin_waits,
    ),
    linked_bin_program(
        BIN_SYSCALLS,
        "syscalls",
        "/bin/syscalls",
        "print retained syscall trace",
        "bin_syscalls",
        handlers::bin_syscalls,
    ),
    linked_bin_program(
        BIN_CONTINUATIONS,
        "continuations",
        "/bin/continuations",
        "print active syscall continuations",
        "bin_continuations",
        handlers::bin_continuations,
    ),
    linked_bin_program(
        BIN_EXECS,
        "execs",
        "/bin/execs",
        "print executable admission table",
        "bin_execs",
        handlers::bin_execs,
    ),
    linked_bin_program(
        BIN_PENDING,
        "pending",
        "/bin/pending",
        "print pending executable table",
        "bin_pending",
        handlers::bin_pending,
    ),
    linked_bin_program(
        BIN_SOURCES,
        "sources",
        "/bin/sources",
        "print executable source table",
        "bin_sources",
        handlers::bin_sources,
    ),
    linked_bin_program(
        BIN_MEDIA,
        "media",
        "/bin/media",
        "print executable media status",
        "bin_media",
        handlers::bin_media,
    ),
    linked_bin_program(
        BIN_SELF,
        "self",
        "/bin/self",
        "print current process state",
        "bin_self",
        handlers::bin_self,
    ),
    linked_bin_program(
        BIN_INSTALL_BIN,
        "install-bin",
        "/bin/install-bin",
        "install a status-only /bin source image",
        "bin_install_bin",
        handlers::bin_install_bin,
    ),
    linked_bin_program(
        BIN_INSTALL_PAYLOAD,
        "install-payload",
        "/bin/install-payload",
        "install a status-only payload source image",
        "bin_install_payload",
        handlers::bin_install_payload,
    ),
    linked_bin_program(
        BIN_INSTALL_BIN_MEDIA,
        "install-bin-media",
        "/bin/install-bin-media",
        "install a /bin source image from source media",
        "bin_install_bin_media",
        handlers::bin_install_bin_media,
    ),
    linked_bin_program(
        BIN_INSTALL_PAYLOAD_MEDIA,
        "install-payload-media",
        "/bin/install-payload-media",
        "install a payload source image from source media",
        "bin_install_payload_media",
        handlers::bin_install_payload_media,
    ),
];

/// Static `/bin` source artifact store shipped by the current `reovim-os` image.
pub const BIN_PROGRAM_SOURCES: [ProgramSourceArtifact; 0] = [];

const LINKED_HELP_LINE_BYTES: usize = 1024;

/// Returns the `/bin` program catalog for this image.
#[must_use]
pub const fn programs() -> &'static [ProgramDescriptor] {
    handlers::programs()
}

/// Returns the `/bin` source artifact store for this image.
#[must_use]
pub const fn program_sources() -> &'static [ProgramSourceArtifact] {
    handlers::program_sources()
}

/// Writes help text for the `/bin` command namespace.
pub fn write_program_help(
    program_name: Option<&str>,
    syscalls: &mut reovim_system_kernel::syscall::ProgramSyscalls<'_, '_, '_>,
) -> reovim_system_kernel::program::ProgramStatus {
    handlers::write_program_help(program_name, syscalls)
}

/// Writes pseudo-file content into `/bin` and `/boot` virtual filesystem views.
pub fn write_vfs_file(
    file: reovim_system_kernel::vfs::File,
    syscalls: &mut reovim_system_kernel::syscall::ProgramSyscalls<'_, '_, '_>,
) {
    handlers::write_vfs_file(file, syscalls)
}
