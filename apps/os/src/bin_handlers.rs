//! Linked `/bin` program implementations for reovim-os.
//!
//! This module owns the concrete program bodies and command-specific helpers for
//! the static catalog defined in `super::bin`.

use {
    reovim_system_kernel::{
        program::{
            self as kernel_program, MAX_PROGRAM_ARGS, MAX_PROGRAM_ENVS, ProgramArgv, ProgramDescriptor,
            ProgramSourceArtifact, ProgramStatus,
        },
        rootd::ROOT_LINE_BYTES,
        syscall::ProgramSyscalls,
        vfs::{self, File},
    },
    reovim_uapi::{
        dump::{DumpSyncReport, SyscallDumpControl},
        fs::{FsError, OpenAtDir, OpenFlags, RawFd, SyscallFdControl},
        process::{
            ExitCode, ProcessArg, ProcessControlReport, ProcessEnv, ProcessError, ProcessId,
            ProcessSleepReport, ProcessStateCode, ProcessTimedWaitReport, ProcessWaitReport,
            SyscallProcessControl,
        },
        sched::{SchedulerTicks, SyscallSchedulerControl},
        service::{
            ServiceControlReport, ServiceControlResultCode, ServiceError, ServiceReasonCode,
            ServiceStateCode, SyscallServiceControl,
        },
        session::SyscallSessionControl,
        source::{
            SourceError, SourceInstallOriginCode, SourceInstallReport, SourceInstallStatusCode,
            SourceNamespaceCode, SyscallSourceControl,
        },
        syscall::RawSyscall,
        system::{SyscallSystemControl, SystemError},
        terminal::SyscallTerminalControl,
    },
};

use super::{
    BIN_HELP, BIN_INIT, BIN_SH, BIN_CLEAR, BIN_SCREENTEST, BIN_PWD, BIN_LS, BIN_CD, BIN_CAT, BIN_READ,
    BIN_MOUNT, BIN_INPUT, BIN_STATUS, BIN_PROOF, BIN_DEVICE, BIN_DMESG, BIN_DUMP, BIN_SCHED, BIN_PROC,
    BIN_PROBE, BIN_LAUNCH, BIN_REOVIM, BIN_HELLO, BIN_HALT, BIN_PS, BIN_KILL, BIN_WAKE, BIN_BLOCK,
    BIN_SPAWN, BIN_SLEEP, BIN_WAIT, BIN_WAIT_TICKS, BIN_EXEC, BIN_SERVICE_STOP, BIN_SERVICE_START,
    BIN_SERVICE_RESTART, BIN_SESSION, BIN_SERVICES, BIN_TASKS, BIN_WAITS, BIN_SYSCALLS,
    BIN_CONTINUATIONS, BIN_EXECS, BIN_PENDING, BIN_SOURCES, BIN_MEDIA, BIN_SELF, BIN_INSTALL_BIN,
    BIN_INSTALL_PAYLOAD, BIN_INSTALL_BIN_MEDIA, BIN_INSTALL_PAYLOAD_MEDIA, BIN_PROGRAMS,
    BIN_PROGRAM_SOURCES, LINKED_HELP_LINE_BYTES,
};

const BIN_SCREENTEST_OUTPUT: &[u8] = concat!(
    "screen test:\n",
    "  target: framebuffer/serial tty renderer subset\n",
    "  fg16: \x1b[30;47mblack\x1b[0m \x1b[31mred\x1b[0m \x1b[32mgreen\x1b[0m \x1b[33myellow\x1b[0m \x1b[34mblue\x1b[0m \x1b[35mmagenta\x1b[0m \x1b[36mcyan\x1b[0m \x1b[37mwhite\x1b[0m\n",
    "  fg16+: \x1b[90mgray\x1b[0m \x1b[91mbr-red\x1b[0m \x1b[92mbr-green\x1b[0m \x1b[93mbr-yellow\x1b[0m \x1b[94mbr-blue\x1b[0m \x1b[95mbr-magenta\x1b[0m \x1b[96mbr-cyan\x1b[0m \x1b[97;40mbr-white\x1b[0m\n",
    "  bg16: \x1b[37;40m 40 \x1b[0m \x1b[30;41m 41 \x1b[0m \x1b[30;42m 42 \x1b[0m \x1b[30;43m 43 \x1b[0m \x1b[37;44m 44 \x1b[0m \x1b[30;45m 45 \x1b[0m \x1b[30;46m 46 \x1b[0m \x1b[30;47m 47 \x1b[0m\n",
    "  bg16+: \x1b[30;100m 100 \x1b[0m \x1b[30;101m 101 \x1b[0m \x1b[30;102m 102 \x1b[0m \x1b[30;103m 103 \x1b[0m \x1b[30;104m 104 \x1b[0m \x1b[30;105m 105 \x1b[0m \x1b[30;106m 106 \x1b[0m \x1b[30;107m 107 \x1b[0m\n",
    "  idx-fg: \x1b[38;5;21midx21\x1b[0m \x1b[38;5;46midx46\x1b[0m \x1b[38;5;51midx51\x1b[0m \x1b[38;5;93midx93\x1b[0m \x1b[38;5;160midx160\x1b[0m \x1b[38;5;196midx196\x1b[0m \x1b[38;5;201midx201\x1b[0m \x1b[38;5;226midx226\x1b[0m\n",
    "  idx-bg: \x1b[48;5;21m 21 \x1b[0m \x1b[48;5;46m 46 \x1b[0m \x1b[48;5;51m 51 \x1b[0m \x1b[48;5;93m 93 \x1b[0m \x1b[48;5;160m 160 \x1b[0m \x1b[48;5;196m 196 \x1b[0m \x1b[48;5;201m 201 \x1b[0m \x1b[48;5;226m 226 \x1b[0m\n",
    "  rgb-fg: \x1b[38;2;255;92;87mwarm\x1b[0m \x1b[38;2;114;214;86mgreen\x1b[0m \x1b[38;2;35;132;255msky\x1b[0m \x1b[38;2;190;120;255mviolet\x1b[0m \x1b[38;2;255;255;255;48;2;0;0;0mwhite-on-black\x1b[0m\n",
    "  rgb-bg: \x1b[48;2;255;92;87m  warm  \x1b[0m \x1b[48;2;114;214;86m  green  \x1b[0m \x1b[48;2;35;132;255m  sky  \x1b[0m \x1b[48;2;190;120;255m  violet  \x1b[0m\n",
    "  attrs: \x1b[1mbold\x1b[22m \x1b[2mdim\x1b[22m \x1b[3mitalic\x1b[23m \x1b[4munderline\x1b[24m \x1b[7mreverse\x1b[27m \x1b[1;4mbold+underline\x1b[0m normal\n",
    "  reset: \x1b[31mred\x1b[39m default-fg \x1b[48;2;48;48;48mgray-bg\x1b[49m default-bg \x1b[1;7mbold-rev\x1b[0m plain\n",
    "  cr: left-side-should-vanish\r  cr: overwritten\n",
    "  el: stale suffix should vanish\r  el: clean\x1b[K\n",
    "  el1: prefix should vanish\x1b[1K\r  el1: clean-left\n",
    "  el2: whole line should vanish\x1b[2K\r  el2: clean-all\n",
    "  bs: AB\x08 \x08C (should read AC)\n",
    "  wrap: 0123456789abcdefghijklmnopqrstuvwxyz 0123456789abcdefghijklmnopqrstuvwxyz 0123456789abcdefghijklmnopqrstuvwxyz 0123456789abcdefghijklmnopqrstuvwxyz 0123456789abcdefghijklmnopqrstuvwxyz end\n",
    "  done\n",
)
    .as_bytes();

pub(super) fn bin_init(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let service = SyscallServiceControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"init: too many arguments\n");
        false
    } else if service.request("shell", "/bin/sh").is_err() {
        let _ = fd.write(RawFd::stderr(), b"init: service request failed\n");
        false
    } else {
        linked_write_all(fd, RawFd::stdout(), b"init: userland services ready\n")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_sh(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let session = SyscallSessionControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"sh: too many arguments\n");
        false
    } else if session
        .request_line_discipline("argv-v1", "single-pipe")
        .is_err()
    {
        let _ = fd.write(RawFd::stderr(), b"sh: line discipline request failed\n");
        false
    } else if session.request_shell_start().is_err() {
        let _ = fd.write(RawFd::stderr(), b"sh: shell start request failed\n");
        false
    } else {
        linked_write_all(fd, RawFd::stdout(), b"reovim system kernel shell ready\n")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_help(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"help: too many arguments\n");
        false
    } else if argv.argc() == 1 {
        linked_help_catalog(fd)
    } else {
        linked_help_entry(fd, argv.arg(1).unwrap_or(""))
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_clear(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let terminal = SyscallTerminalControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"clear: too many arguments\n");
        false
    } else {
        match terminal.clear_primary_output() {
            Ok(()) => linked_write_tty_output(fd, b"clear", b"\x1b[2J\x1b[H"),
            Err(_) => {
                let _ = fd.write(RawFd::stderr(), b"clear: terminal clear failed\n");
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_hello(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    if argv.argc() > 1 {
        let message = b"hello: too many arguments\n";
        let _ = fd.write(RawFd::stderr(), message);
        return ProgramStatus::Error;
    }

    let message = b"hello from linked bin\n";
    if matches!(fd.write(RawFd::stdout(), message), Ok(written) if written == message.len()) {
        let _ = process.exit(ExitCode::SUCCESS);
    }
    ProgramStatus::Error
}

pub(super) fn bin_halt(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let system = SyscallSystemControl::new(raw);
    if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"halt: too many arguments\n");
        let _ = process.exit(ExitCode::FAILURE);
        return ProgramStatus::Error;
    }

    if !linked_write_all(fd, RawFd::stdout(), b"halt: ok\n") {
        let _ = process.exit(ExitCode::FAILURE);
        return ProgramStatus::Error;
    }
    if system.halt().is_err() {
        let _ = fd.write(RawFd::stderr(), b"halt: request failed\n");
        let _ = process.exit(ExitCode::FAILURE);
    }
    ProgramStatus::Error
}

pub(super) fn bin_probe(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let system = SyscallSystemControl::new(raw);
    if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"probe: too many arguments\n");
        let _ = process.exit(ExitCode::FAILURE);
        return ProgramStatus::Error;
    }

    let Some(target) = argv.arg(1) else {
        let _ = fd.write(RawFd::stderr(), b"probe: missing target, try `probe help`\n");
        let _ = process.exit(ExitCode::FAILURE);
        return ProgramStatus::Error;
    };

    let ok = match system.probe(target) {
        Ok(()) => true,
        Err(SystemError::UnknownTarget) => {
            let _ = fd.write(RawFd::stderr(), b"probe: unknown target: ");
            let _ = fd.write(RawFd::stderr(), target.as_bytes());
            let _ = fd.write(RawFd::stderr(), b"\n");
            false
        }
        Err(SystemError::Unsupported) => {
            let _ = fd.write(RawFd::stderr(), b"probe: no lower probe provider\n");
            false
        }
        Err(_) => {
            let _ = fd.write(RawFd::stderr(), b"probe: request failed\n");
            false
        }
    };
    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_reovim(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let payload = match linked_implicit_child_invocation(argv, 1, "reovim") {
        Ok(payload) => payload,
        Err(LinkedChildInvocationError::TooManyArguments) => {
            let _ = fd.write(RawFd::stderr(), b"reovim: too many arguments\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
        Err(LinkedChildInvocationError::TooManyEnvVars) => {
            let _ = fd.write(RawFd::stderr(), b"reovim: too many env vars\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
        Err(LinkedChildInvocationError::MissingProgram) => unreachable!(),
    };

    let child = match process.spawn_payload_with_env(payload.argv(), payload.env()) {
        Ok(child) => child,
        Err(ProcessError::UNSUPPORTED) => {
            let _ = fd.write(RawFd::stdout(), b"reovim disabled for this profile\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
        Err(ProcessError::NOT_FOUND) => {
            let _ = fd.write(RawFd::stdout(), b"reovim: payload.not_configured\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
        Err(ProcessError::INVALID_IMAGE) => {
            let _ = fd.write(RawFd::stdout(), b"reovim: payload.failed\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
        Err(_) => {
            let _ = fd.write(RawFd::stdout(), b"reovim: payload.failed\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
    };

    if !linked_write_all(fd, RawFd::stdout(), b"reovim: ") {
        let _ = process.exit(ExitCode::FAILURE);
        return ProgramStatus::Error;
    }

    match process.wait(child) {
        Ok(code) => linked_reovim_exit_with_payload_status(fd, process, code),
        Err(ProcessError::BUSY) => ProgramStatus::Blocked,
        Err(_) => {
            let _ = fd.write(RawFd::stdout(), b"payload.failed\n");
            let _ = process.exit(ExitCode::FAILURE);
            ProgramStatus::Error
        }
    }
}

pub(super) fn bin_launch(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    match linked_launch_enabled(fd) {
        Some(true) => {}
        Some(false) => {
            let _ = fd.write(RawFd::stderr(), b"launch disabled for this profile\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
        None => {
            let _ = fd.write(RawFd::stderr(), b"launch: profile read failed\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
    }

    if argv.argc() == 1 {
        let ok = linked_copy_path_to_stdout(fd, b"/boot/payloads", b"launch");
        let _ = process.exit(if ok {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        });
        return ProgramStatus::Error;
    }

    let child = match linked_child_invocation(argv, 1) {
        Ok(child) => child,
        Err(LinkedChildInvocationError::MissingProgram) => {
            let _ = fd.write(RawFd::stderr(), b"launch: no payload name\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
        Err(LinkedChildInvocationError::TooManyArguments) => {
            let _ = fd.write(RawFd::stderr(), b"launch: invalid payload argv\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
        Err(LinkedChildInvocationError::TooManyEnvVars) => {
            let _ = fd.write(RawFd::stderr(), b"launch: too many env vars\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
    };

    let payload = linked_program_name_after_env(argv, 1).unwrap_or("");
    if payload.is_empty() {
        let _ = fd.write(RawFd::stderr(), b"launch: no payload name\n");
        let _ = process.exit(ExitCode::FAILURE);
        return ProgramStatus::Error;
    }

    let child = match process.spawn_payload_with_env(child.argv(), child.env()) {
        Ok(child) => child,
        Err(ProcessError::UNSUPPORTED) => {
            let _ = fd.write(RawFd::stderr(), b"launch disabled for this profile\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
        Err(ProcessError::NOT_FOUND) => {
            let _ = linked_write_launch_payload_status(fd, payload, b"payload.not_configured");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
        Err(ProcessError::INVALID_IMAGE) => {
            let _ = linked_write_launch_payload_status(fd, payload, b"payload.failed");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
        Err(_) => {
            let _ = linked_write_launch_payload_status(fd, payload, b"payload.failed");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
    };

    match process.wait_ready_report(child) {
        Ok(report) => linked_launch_exit_with_payload_report(fd, process, payload, report),
        Err(ProcessError::BUSY) => ProgramStatus::Blocked,
        Err(_) => {
            let _ = linked_write_launch_payload_status(fd, payload, b"payload.failed");
            let _ = process.exit(ExitCode::FAILURE);
            ProgramStatus::Error
        }
    }
}

pub(super) fn bin_pwd(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"pwd: too many arguments\n");
        let _ = process.exit(ExitCode::FAILURE);
        return ProgramStatus::Error;
    }

    let mut cwd = [0u8; vfs::MAX_PATH_BYTES];
    let ok = match fd.get_cwd(&mut cwd) {
        Ok(len) => {
            matches!(fd.write(RawFd::stdout(), &cwd[..len]), Ok(written) if written == len)
                && matches!(fd.write(RawFd::stdout(), b"\n"), Ok(1))
        }
        Err(_) => {
            let _ = fd.write(RawFd::stderr(), b"pwd: getcwd failed\n");
            false
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_cd(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"cd: too many arguments\n");
        let _ = process.exit(ExitCode::FAILURE);
        return ProgramStatus::Error;
    }

    let target = if argv.argc() == 1 {
        "/"
    } else {
        argv.arg(1).unwrap_or("")
    };
    let ok = if target.is_empty() {
        let _ = fd.write(RawFd::stderr(), b"cd: empty path\n");
        false
    } else {
        match fd.chdir(target.as_bytes()) {
            Ok(()) => true,
            Err(error) => {
                linked_write_path_error(fd, b"cd", target, error);
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_ls(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"ls: too many arguments\n");
        let _ = process.exit(ExitCode::FAILURE);
        return ProgramStatus::Error;
    }

    let target = if argv.argc() == 1 {
        "."
    } else {
        argv.arg(1).unwrap_or("")
    };
    let ok = if target.is_empty() {
        let _ = fd.write(RawFd::stderr(), b"ls: empty path\n");
        false
    } else {
        linked_ls_target(fd, target)
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_cat(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let status = if argv.argc() == 1 {
        linked_cat_copy_fd(fd, RawFd::stdin())
    } else {
        let mut ok = true;
        let mut index = 1usize;
        while index < argv.argc() {
            let target = argv.arg(index).unwrap_or("");
            if target.is_empty() {
                let _ = fd.write(RawFd::stderr(), b"cat: empty path\n");
                ok = false;
                index += 1;
                continue;
            }
            match fd.open_at(OpenAtDir::session_cwd(), target.as_bytes(), OpenFlags::READ_ONLY) {
                Ok(opened) => {
                    match linked_cat_copy_fd(fd, opened) {
                        LinkedCopyStatus::Ok => {}
                        LinkedCopyStatus::Blocked => {
                            let _ = fd.close(opened);
                            return ProgramStatus::Blocked;
                        }
                        LinkedCopyStatus::Error => {
                            ok = false;
                        }
                    }
                    let _ = fd.close(opened);
                }
                Err(error) => {
                    linked_write_path_error(fd, b"cat", target, error);
                    ok = false;
                }
            }
            index += 1;
        }
        if ok {
            LinkedCopyStatus::Ok
        } else {
            LinkedCopyStatus::Error
        }
    };

    if status == LinkedCopyStatus::Blocked {
        return ProgramStatus::Blocked;
    }

    let _ = process.exit(if status == LinkedCopyStatus::Ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_read(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"read: too many arguments\n");
        false
    } else {
        linked_read_tty_line(fd)
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_mount(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"mount: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/boot/mounts", b"mount")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_input(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"input: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/boot/input", b"input")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_proof(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"proof: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/boot/proof", b"proof")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_device(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"device: too many arguments\n");
        false
    } else {
        linked_write_all(fd, RawFd::stdout(), b"boot_info:\n")
            && linked_copy_path_to_stdout_with_line_prefix(fd, b"/boot/memory", b"device", b"  ")
            && linked_write_all(fd, RawFd::stdout(), b"devices:\n")
            && linked_copy_path_to_stdout(fd, b"/boot/devices", b"device")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_status(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"status: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/boot/status", b"status")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_dmesg(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"dmesg: too many arguments\n");
        false
    } else {
        match argv.arg(1) {
            None => linked_copy_path_to_stdout(fd, b"/log/dmesg", b"dmesg"),
            Some("--stats") => linked_copy_path_to_stdout(fd, b"/log/stats", b"dmesg"),
            Some(_) => {
                let _ = fd.write(RawFd::stderr(), b"dmesg: unknown option\n");
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_dump(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let dump = SyscallDumpControl::new(raw);
    let process = SyscallProcessControl::new(raw);

    let ok = if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"dump: too many arguments\n");
        false
    } else {
        match argv.arg(1) {
            None | Some("status") => linked_copy_path_to_stdout(fd, b"/dump/status", b"dump"),
            Some("snapshot") => linked_copy_path_to_stdout(fd, b"/dump/snapshot", b"dump"),
            Some("sync") => linked_dump_sync(fd, dump),
            Some(_) => {
                let _ = fd.write(RawFd::stderr(), b"dump: unknown subcommand\n");
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_sched(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let sched = SyscallSchedulerControl::new(raw);
    let process = SyscallProcessControl::new(raw);

    let ok = match argv.arg(1) {
        None | Some("status") => {
            if argv.argc() > 2 {
                let _ = fd.write(RawFd::stderr(), b"sched: too many arguments\n");
                false
            } else {
                linked_sched_write_status(fd)
            }
        }
        Some("tick") => {
            if argv.argc() > 2 {
                let _ = fd.write(RawFd::stderr(), b"sched: too many arguments\n");
                false
            } else {
                linked_sched_tick(fd, sched) && linked_sched_write_status(fd)
            }
        }
        Some("yield") => {
            if argv.argc() > 2 {
                let _ = fd.write(RawFd::stderr(), b"sched: too many arguments\n");
                false
            } else {
                linked_sched_yield(fd, sched) && linked_sched_write_status(fd)
            }
        }
        Some("sleep") => {
            if argv.argc() > 3 {
                let _ = fd.write(RawFd::stderr(), b"sched: too many arguments\n");
                false
            } else {
                let Some(ticks) = argv.arg(2) else {
                    let _ = fd.write(RawFd::stderr(), b"sched sleep: missing ticks\n");
                    let _ = process.exit(ExitCode::FAILURE);
                    return ProgramStatus::Error;
                };
                let Some(ticks) = linked_parse_usize(ticks) else {
                    let _ = fd.write(RawFd::stderr(), b"sched sleep: invalid ticks\n");
                    let _ = process.exit(ExitCode::FAILURE);
                    return ProgramStatus::Error;
                };
                if ticks == 0 {
                    let _ = fd.write(RawFd::stderr(), b"sched sleep: invalid ticks\n");
                    false
                } else {
                    linked_sched_sleep(fd, sched, ticks) && linked_sched_write_status(fd)
                }
            }
        }
        Some(_) => {
            let _ = fd.write(RawFd::stderr(), b"sched: unknown subcommand\n");
            false
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_proc(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);

    let ok = if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"proc: too many arguments\n");
        false
    } else {
        match argv.arg(1) {
            None | Some("processes") => linked_copy_path_to_stdout(fd, b"/proc/processes", b"proc"),
            Some("execs") => linked_copy_path_to_stdout(fd, b"/proc/execs", b"proc"),
            Some("address-spaces") => {
                linked_copy_path_to_stdout(fd, b"/proc/address-spaces", b"proc")
            }
            Some("page-tables") => linked_copy_path_to_stdout(fd, b"/proc/page-tables", b"proc"),
            Some("pages") => linked_copy_path_to_stdout(fd, b"/proc/pages", b"proc"),
            Some("memory-objects") => {
                linked_copy_path_to_stdout(fd, b"/proc/memory-objects", b"proc")
            }
            Some("media") => linked_copy_path_to_stdout(fd, b"/proc/media", b"proc"),
            Some("pending") => linked_copy_path_to_stdout(fd, b"/proc/pending", b"proc"),
            Some("self") => match process.self_report() {
                Ok(report) => linked_write_process_control_report(fd, b"self", &report),
                Err(ProcessError::BUSY) => return ProgramStatus::Blocked,
                Err(_) => {
                    let _ = fd.write(RawFd::stderr(), b"proc self: unavailable\n");
                    false
                }
            },
            Some("session") => linked_copy_path_to_stdout(fd, b"/proc/session", b"proc"),
            Some("services") => linked_copy_path_to_stdout(fd, b"/proc/services", b"proc"),
            Some("sources") => linked_copy_path_to_stdout(fd, b"/proc/sources", b"proc"),
            Some("tasks") => linked_copy_path_to_stdout(fd, b"/proc/tasks", b"proc"),
            Some("waits") => linked_copy_path_to_stdout(fd, b"/proc/waits", b"proc"),
            Some("syscalls") => linked_copy_path_to_stdout(fd, b"/proc/syscalls", b"proc"),
            Some("continuations") => {
                linked_copy_path_to_stdout(fd, b"/proc/continuations", b"proc")
            }
            Some("scheduler") => linked_copy_path_to_stdout(fd, b"/proc/scheduler", b"proc"),
            Some(_) => {
                let _ = fd.write(RawFd::stderr(), b"proc: unknown subcommand\n");
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_ps(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"ps: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/proc/processes", b"ps")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_session(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"session: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/proc/session", b"session")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_services(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"services: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/proc/services", b"services")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_tasks(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"tasks: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/proc/tasks", b"tasks")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_waits(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"waits: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/proc/waits", b"waits")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_syscalls(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"syscalls: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/proc/syscalls", b"syscalls")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_continuations(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"continuations: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/proc/continuations", b"continuations")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_screentest(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"screentest: too many arguments\n");
        false
    } else {
        linked_write_tty_output(fd, b"screentest", BIN_SCREENTEST_OUTPUT)
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_execs(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"execs: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/proc/execs", b"execs")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_pending(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"pending: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/proc/pending", b"pending")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_sources(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"sources: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/proc/sources", b"sources")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_media(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"media: too many arguments\n");
        false
    } else {
        linked_copy_path_to_stdout(fd, b"/proc/media", b"media")
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_self(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 1 {
        let _ = fd.write(RawFd::stderr(), b"self: too many arguments\n");
        false
    } else {
        match process.self_report() {
            Ok(report) => linked_write_process_control_report(fd, b"self", &report),
            Err(ProcessError::BUSY) => return ProgramStatus::Blocked,
            Err(_) => {
                let _ = fd.write(RawFd::stderr(), b"self: unavailable\n");
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_install_bin(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let source = SyscallSourceControl::new(raw);
    let ok = if argv.argc() > 3 {
        let _ = fd.write(RawFd::stderr(), b"install-bin: too many arguments\n");
        false
    } else {
        let Some(name) = argv.arg(1) else {
            let _ = fd.write(RawFd::stderr(), b"install-bin: missing program\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        let Some(status_text) = argv.arg(2) else {
            let _ = fd.write(RawFd::stderr(), b"install-bin: missing status\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        let Some(status) = linked_parse_bin_source_install_status(status_text) else {
            let _ = fd.write(RawFd::stderr(), b"install-bin: invalid status\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        match source.install_bin_status(name, status) {
            Ok(report) => linked_write_source_install_report(fd, b"install-bin", &report),
            Err(error) => {
                linked_write_source_error(fd, b"install-bin", error, b"program-not-found");
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_install_payload(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let source = SyscallSourceControl::new(raw);
    let ok = if argv.argc() > 3 {
        let _ = fd.write(RawFd::stderr(), b"install-payload: too many arguments\n");
        false
    } else {
        let Some(name) = argv.arg(1) else {
            let _ = fd.write(RawFd::stderr(), b"install-payload: missing payload\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        let Some(status_text) = argv.arg(2) else {
            let _ = fd.write(RawFd::stderr(), b"install-payload: missing status\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        let Some(status) = linked_parse_payload_source_install_status(status_text) else {
            let _ = fd.write(RawFd::stderr(), b"install-payload: invalid status\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        match source.install_payload_status(name, status) {
            Ok(report) => linked_write_source_install_report(fd, b"install-payload", &report),
            Err(error) => {
                linked_write_source_error(fd, b"install-payload", error, b"payload-not-found");
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_install_bin_media(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let source = SyscallSourceControl::new(raw);
    let ok = if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"install-bin-media: too many arguments\n");
        false
    } else {
        let Some(name) = argv.arg(1) else {
            let _ = fd.write(RawFd::stderr(), b"install-bin-media: missing program\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        match source.install_bin_media(name) {
            Ok(report) => linked_write_source_install_report(fd, b"install-bin-media", &report),
            Err(error) => {
                linked_write_source_error(fd, b"install-bin-media", error, b"program-not-found");
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_install_payload_media(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let source = SyscallSourceControl::new(raw);
    let ok = if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"install-payload-media: too many arguments\n");
        false
    } else {
        let Some(name) = argv.arg(1) else {
            let _ = fd.write(RawFd::stderr(), b"install-payload-media: missing payload\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        match source.install_payload_media(name) {
            Ok(report) => linked_write_source_install_report(fd, b"install-payload-media", &report),
            Err(error) => {
                linked_write_source_error(
                    fd,
                    b"install-payload-media",
                    error,
                    b"payload-not-found",
                );
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_kill(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"kill: too many arguments\n");
        false
    } else {
        let Some(pid) = argv.arg(1) else {
            let _ = fd.write(RawFd::stderr(), b"kill: missing pid\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        let Some(pid) = linked_parse_usize(pid) else {
            let _ = fd.write(RawFd::stderr(), b"kill: invalid pid\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        match process.kill_report(ProcessId::new(pid)) {
            Ok(report) => linked_write_process_control_report(fd, b"kill", &report),
            Err(_) => {
                let _ = fd.write(RawFd::stderr(), b"kill: failed\n");
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_wake(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"wake: too many arguments\n");
        false
    } else {
        let Some(pid) = argv.arg(1) else {
            let _ = fd.write(RawFd::stderr(), b"wake: missing pid\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        let Some(pid) = linked_parse_usize(pid) else {
            let _ = fd.write(RawFd::stderr(), b"wake: invalid pid\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        match process.wake_report(ProcessId::new(pid)) {
            Ok(report) => linked_write_process_control_report(fd, b"wake", &report),
            Err(_) => {
                let _ = fd.write(RawFd::stderr(), b"wake: failed\n");
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_block(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = match linked_child_invocation(argv, 1) {
        Err(LinkedChildInvocationError::MissingProgram) => {
            let _ = fd.write(RawFd::stderr(), b"block: missing program\n");
            false
        }
        Err(LinkedChildInvocationError::TooManyArguments) => {
            let _ = fd.write(RawFd::stderr(), b"block: too many arguments\n");
            false
        }
        Err(LinkedChildInvocationError::TooManyEnvVars) => {
            let _ = fd.write(RawFd::stderr(), b"block: too many env vars\n");
            false
        }
        Ok(child) => match process.spawn_blocked_report_with_env(child.argv(), child.env()) {
            Ok(report) => linked_write_process_control_report(fd, b"block", &report),
            Err(ProcessError::NOT_FOUND) => {
                let _ = fd.write(RawFd::stderr(), b"block: program-not-found\n");
                false
            }
            Err(_) => {
                let _ = fd.write(RawFd::stderr(), b"block: failed\n");
                false
            }
        },
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_spawn(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = match linked_child_invocation(argv, 1) {
        Err(LinkedChildInvocationError::MissingProgram) => {
            let _ = fd.write(RawFd::stderr(), b"spawn: missing program\n");
            false
        }
        Err(LinkedChildInvocationError::TooManyArguments) => {
            let _ = fd.write(RawFd::stderr(), b"spawn: too many arguments\n");
            false
        }
        Err(LinkedChildInvocationError::TooManyEnvVars) => {
            let _ = fd.write(RawFd::stderr(), b"spawn: too many env vars\n");
            false
        }
        Ok(child) => match process.spawn_report_with_env(child.argv(), child.env()) {
            Ok(report) => linked_write_process_control_report(fd, b"spawn", &report),
            Err(ProcessError::NOT_FOUND) => {
                let _ = fd.write(RawFd::stderr(), b"spawn: program-not-found\n");
                false
            }
            Err(_) => {
                let _ = fd.write(RawFd::stderr(), b"spawn: failed\n");
                false
            }
        },
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_sleep(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > MAX_PROGRAM_ARGS + 2 {
        let _ = fd.write(RawFd::stderr(), b"sleep: too many arguments\n");
        false
    } else {
        let Some(ticks) = argv.arg(1) else {
            let _ = fd.write(RawFd::stderr(), b"sleep: missing ticks\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        let Some(ticks) = linked_parse_usize(ticks) else {
            let _ = fd.write(RawFd::stderr(), b"sleep: invalid ticks\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        if ticks == 0 {
            let _ = fd.write(RawFd::stderr(), b"sleep: invalid ticks\n");
            false
        } else {
            match linked_child_invocation(argv, 2) {
                Err(LinkedChildInvocationError::MissingProgram) => {
                    let _ = fd.write(RawFd::stderr(), b"sleep: missing program\n");
                    false
                }
                Err(LinkedChildInvocationError::TooManyArguments) => {
                    let _ = fd.write(RawFd::stderr(), b"sleep: too many arguments\n");
                    false
                }
                Err(LinkedChildInvocationError::TooManyEnvVars) => {
                    let _ = fd.write(RawFd::stderr(), b"sleep: too many env vars\n");
                    false
                }
                Ok(child) => {
                    match process.spawn_sleeping_with_env(child.argv(), child.env(), ticks) {
                        Ok(report) => linked_write_process_sleep_report(fd, &report),
                        Err(ProcessError::NOT_FOUND) => {
                            let _ = fd.write(RawFd::stderr(), b"sleep: program-not-found\n");
                            false
                        }
                        Err(ProcessError::INVALID_ARGUMENT) => {
                            let _ = fd.write(RawFd::stderr(), b"sleep: invalid argument\n");
                            false
                        }
                        Err(_) => {
                            let _ = fd.write(RawFd::stderr(), b"sleep: failed\n");
                            false
                        }
                    }
                }
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_wait(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"wait: too many arguments\n");
        false
    } else {
        let Some(pid) = argv.arg(1) else {
            let _ = fd.write(RawFd::stderr(), b"wait: missing pid\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        let Some(pid) = linked_parse_usize(pid) else {
            let _ = fd.write(RawFd::stderr(), b"wait: invalid pid\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        match process.wait_report(ProcessId::new(pid)) {
            Ok(report) => linked_write_process_wait_report(fd, &report),
            Err(ProcessError::BUSY) => return ProgramStatus::Blocked,
            Err(ProcessError::NOT_WAITABLE) => {
                let _ = fd.write(RawFd::stderr(), b"wait: not-waitable\n");
                false
            }
            Err(ProcessError::NOT_FOUND) => {
                let _ = fd.write(RawFd::stderr(), b"wait: process-not-found\n");
                false
            }
            Err(ProcessError::PROTECTED_PROCESS) => {
                let _ = fd.write(RawFd::stderr(), b"wait: protected-process\n");
                false
            }
            Err(_) => {
                let _ = fd.write(RawFd::stderr(), b"wait: failed\n");
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_wait_ticks(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let ok = if argv.argc() > 3 {
        let _ = fd.write(RawFd::stderr(), b"wait-ticks: too many arguments\n");
        false
    } else {
        let Some(ticks) = argv.arg(1) else {
            let _ = fd.write(RawFd::stderr(), b"wait-ticks: missing ticks\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        let Some(ticks) = linked_parse_usize(ticks) else {
            let _ = fd.write(RawFd::stderr(), b"wait-ticks: invalid ticks\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        let Some(pid) = argv.arg(2) else {
            let _ = fd.write(RawFd::stderr(), b"wait-ticks: missing pid\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        let Some(pid) = linked_parse_usize(pid) else {
            let _ = fd.write(RawFd::stderr(), b"wait-ticks: invalid pid\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        if ticks == 0 {
            let _ = fd.write(RawFd::stderr(), b"wait-ticks: invalid ticks\n");
            false
        } else {
            match process.wait_for_ticks(ProcessId::new(pid), ticks) {
                Ok(report) => linked_write_process_timed_wait_report(fd, &report),
                Err(_) => {
                    let _ = fd.write(RawFd::stderr(), b"wait-ticks: failed\n");
                    false
                }
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_exec(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let child = match linked_child_invocation(argv, 1) {
        Ok(child) => child,
        Err(LinkedChildInvocationError::MissingProgram) => {
            let _ = fd.write(RawFd::stderr(), b"exec: missing program\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
        Err(LinkedChildInvocationError::TooManyArguments) => {
            let _ = fd.write(RawFd::stderr(), b"exec: too many arguments\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
        Err(LinkedChildInvocationError::TooManyEnvVars) => {
            let _ = fd.write(RawFd::stderr(), b"exec: too many env vars\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        }
    };

    match process.execve_with_env(child.argv(), child.env()) {
        Ok(_replacement_exit) => ProgramStatus::Replaced,
        Err(ProcessError::BUSY) => ProgramStatus::Blocked,
        Err(ProcessError::NOT_FOUND) => {
            let _ = fd.write(RawFd::stderr(), b"exec: program-not-found\n");
            let _ = process.exit(ExitCode::FAILURE);
            ProgramStatus::Error
        }
        Err(ProcessError::INVALID_IMAGE) => {
            let _ = fd.write(RawFd::stderr(), b"exec: invalid-image\n");
            let _ = process.exit(ExitCode::FAILURE);
            ProgramStatus::Error
        }
        Err(_) => {
            let _ = fd.write(RawFd::stderr(), b"exec: failed\n");
            let _ = process.exit(ExitCode::FAILURE);
            ProgramStatus::Error
        }
    }
}

pub(super) fn bin_service_stop(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let service = SyscallServiceControl::new(raw);
    let ok = if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"service-stop: too many arguments\n");
        false
    } else {
        let Some(name) = argv.arg(1) else {
            let _ = fd.write(RawFd::stderr(), b"service-stop: missing service\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        match service.stop_report(name) {
            Ok(report) => linked_write_service_control_report(fd, b"service-stop", &report),
            Err(ServiceError::NOT_FOUND) => {
                let _ = fd.write(RawFd::stderr(), b"service-stop: service-not-found\n");
                false
            }
            Err(ServiceError::PROTECTED_SERVICE) => {
                let _ = fd.write(RawFd::stderr(), b"service-stop: protected-service\n");
                false
            }
            Err(ServiceError::NOT_STOPPABLE) => {
                let _ = fd.write(RawFd::stderr(), b"service-stop: not-stoppable\n");
                false
            }
            Err(ServiceError::INVALID_ARGUMENT) => {
                let _ = fd.write(RawFd::stderr(), b"service-stop: invalid service\n");
                false
            }
            Err(_) => {
                let _ = fd.write(RawFd::stderr(), b"service-stop: failed\n");
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_service_start(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let service = SyscallServiceControl::new(raw);
    let ok = if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"service-start: too many arguments\n");
        false
    } else {
        let Some(name) = argv.arg(1) else {
            let _ = fd.write(RawFd::stderr(), b"service-start: missing service\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        match service.start_report(name) {
            Ok(report) => {
                linked_write_service_control_report(fd, b"service-start", &report)
                    && linked_service_result_success(report.result(), report.exit_code())
            }
            Err(ServiceError::NOT_FOUND) => {
                let _ = fd.write(RawFd::stderr(), b"service-start: service-not-found\n");
                false
            }
            Err(ServiceError::PROTECTED_SERVICE) => {
                let _ = fd.write(RawFd::stderr(), b"service-start: protected-service\n");
                false
            }
            Err(ServiceError::NOT_STOPPABLE) => {
                let _ = fd.write(RawFd::stderr(), b"service-start: already-started\n");
                false
            }
            Err(ServiceError::UNSUPPORTED) => {
                let _ = fd.write(RawFd::stderr(), b"service-start: unsupported-target\n");
                false
            }
            Err(ServiceError::PROCESS_NOT_FOUND) => {
                let _ = fd.write(RawFd::stderr(), b"service-start: process-not-found\n");
                false
            }
            Err(ServiceError::INVALID_ARGUMENT) => {
                let _ = fd.write(RawFd::stderr(), b"service-start: invalid service\n");
                false
            }
            Err(_) => {
                let _ = fd.write(RawFd::stderr(), b"service-start: failed\n");
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

pub(super) fn bin_service_restart(argv: &ProgramArgv<'_>, raw: RawSyscall) -> ProgramStatus {
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let service = SyscallServiceControl::new(raw);
    let ok = if argv.argc() > 2 {
        let _ = fd.write(RawFd::stderr(), b"service-restart: too many arguments\n");
        false
    } else {
        let Some(name) = argv.arg(1) else {
            let _ = fd.write(RawFd::stderr(), b"service-restart: missing service\n");
            let _ = process.exit(ExitCode::FAILURE);
            return ProgramStatus::Error;
        };
        match service.restart_report(name) {
            Ok(report) => {
                linked_write_service_control_report(fd, b"service-restart", &report)
                    && linked_service_result_success(report.result(), report.exit_code())
            }
            Err(ServiceError::NOT_FOUND) => {
                let _ = fd.write(RawFd::stderr(), b"service-restart: service-not-found\n");
                false
            }
            Err(ServiceError::PROTECTED_SERVICE) => {
                let _ = fd.write(RawFd::stderr(), b"service-restart: protected-service\n");
                false
            }
            Err(ServiceError::NOT_STOPPABLE) => {
                let _ = fd.write(RawFd::stderr(), b"service-restart: not-stoppable\n");
                false
            }
            Err(ServiceError::UNSUPPORTED) => {
                let _ = fd.write(RawFd::stderr(), b"service-restart: unsupported-target\n");
                false
            }
            Err(ServiceError::PROCESS_NOT_FOUND) => {
                let _ = fd.write(RawFd::stderr(), b"service-restart: process-not-found\n");
                false
            }
            Err(ServiceError::INVALID_ARGUMENT) => {
                let _ = fd.write(RawFd::stderr(), b"service-restart: invalid service\n");
                false
            }
            Err(_) => {
                let _ = fd.write(RawFd::stderr(), b"service-restart: failed\n");
                false
            }
        }
    };

    let _ = process.exit(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    });
    ProgramStatus::Error
}

fn linked_sched_tick(fd: SyscallFdControl, sched: SyscallSchedulerControl) -> bool {
    match sched.tick_current() {
        Ok(tick_count) => {
            linked_write_all(
                fd,
                RawFd::stdout(),
                b"sched tick:\nticked=true\nstatus=ok\ntick_count=",
            ) && linked_write_usize(fd, RawFd::stdout(), tick_count)
                && linked_write_all(fd, RawFd::stdout(), b"\n")
        }
        Err(_) => {
            let _ = fd.write(RawFd::stderr(), b"sched tick: failed\n");
            false
        }
    }
}

fn linked_sched_yield(fd: SyscallFdControl, sched: SyscallSchedulerControl) -> bool {
    match sched.yield_now() {
        Ok(yielded) => {
            linked_write_all(fd, RawFd::stdout(), b"sched yield:\nyielded=")
                && linked_write_bool(fd, RawFd::stdout(), yielded)
                && linked_write_all(
                    fd,
                    RawFd::stdout(),
                    if yielded {
                        b"\nstatus=ok\n"
                    } else {
                        b"\nstatus=no-peer\n"
                    },
                )
        }
        Err(_) => {
            let _ = fd.write(RawFd::stderr(), b"sched yield: failed\n");
            false
        }
    }
}

fn linked_sched_sleep(fd: SyscallFdControl, sched: SyscallSchedulerControl, ticks: usize) -> bool {
    match sched.sleep_for_ticks(SchedulerTicks::new(ticks)) {
        Ok(tick_count) => {
            linked_write_all(
                fd,
                RawFd::stdout(),
                b"sched sleep:\nslept=true\nstatus=ok\ntick_count=",
            ) && linked_write_usize(fd, RawFd::stdout(), tick_count)
                && linked_write_all(fd, RawFd::stdout(), b"\n")
        }
        Err(_) => {
            let _ = fd.write(RawFd::stderr(), b"sched sleep: failed\n");
            false
        }
    }
}

fn linked_sched_write_status(fd: SyscallFdControl) -> bool {
    linked_copy_path_to_stdout(fd, b"/proc/scheduler", b"sched")
}

fn linked_dump_sync(fd: SyscallFdControl, dump: SyscallDumpControl) -> bool {
    let mut report = DumpSyncReport::empty();
    if dump.sync(&mut report).is_err() {
        let _ = fd.write(RawFd::stderr(), b"dump sync: request failed\n");
        return false;
    }

    let mut ok = linked_write_all(fd, RawFd::stdout(), b"dump sync:\npersistent=")
        && linked_write_all(
            fd,
            RawFd::stdout(),
            if report.persistent_available() {
                b"available"
            } else {
                b"unavailable"
            },
        )
        && linked_write_all(fd, RawFd::stdout(), b"\nattempted=")
        && linked_write_bool(fd, RawFd::stdout(), report.attempted())
        && linked_write_all(fd, RawFd::stdout(), b"\nstorage=")
        && linked_write_all(fd, RawFd::stdout(), report.storage_bytes())
        && linked_write_all(fd, RawFd::stdout(), b"\nstorage_capacity_bytes=")
        && linked_write_usize(fd, RawFd::stdout(), report.storage_capacity_bytes())
        && linked_write_all(fd, RawFd::stdout(), b"\nstatus=")
        && linked_write_all(
            fd,
            RawFd::stdout(),
            if report.written() {
                b"written"
            } else {
                b"not-written"
            },
        );

    if report.persistent_available() {
        ok = ok
            && linked_write_all(fd, RawFd::stdout(), b"\nbytes=")
            && linked_write_usize(fd, RawFd::stdout(), report.bytes_written())
            && linked_write_all(fd, RawFd::stdout(), b"\nchecksum=")
            && linked_write_usize(fd, RawFd::stdout(), report.checksum() as usize)
            && linked_write_all(fd, RawFd::stdout(), b"\nverified=")
            && linked_write_bool(fd, RawFd::stdout(), report.verified());
    }

    ok = ok
        && linked_write_all(fd, RawFd::stdout(), b"\nreason=")
        && linked_write_all(fd, RawFd::stdout(), report.reason_bytes())
        && linked_write_all(fd, RawFd::stdout(), b"\n");
    ok && report.written()
}

fn linked_help_catalog(fd: SyscallFdControl) -> bool {
    let opened = match fd.open_at(OpenAtDir::session_cwd(), b"/boot/help", OpenFlags::READ_ONLY) {
        Ok(opened) => opened,
        Err(_) => {
            linked_write_command_error(fd, b"help", b"open failed");
            return false;
        }
    };

    let mut read_buf = [0u8; 64];
    let mut line = [0u8; LINKED_HELP_LINE_BYTES];
    let mut line_len = 0usize;
    let mut overflow = false;
    let mut ok = true;
    let mut done = false;

    while !done {
        let read = match fd.read(opened, &mut read_buf) {
            Ok(read) => read,
            Err(_) => {
                linked_write_command_error(fd, b"help", b"read failed");
                ok = false;
                break;
            }
        };
        if read == 0 {
            if line_len > 0 && !overflow {
                ok = linked_help_catalog_line(fd, &line[..line_len]);
            }
            break;
        }

        let mut index = 0usize;
        while index < read {
            let byte = read_buf[index];
            if byte == b'\n' {
                if !overflow {
                    if &line[..line_len] == b"details:" {
                        done = true;
                        break;
                    }
                    if !linked_help_catalog_line(fd, &line[..line_len]) {
                        ok = false;
                        done = true;
                        break;
                    }
                }
                line_len = 0;
                overflow = false;
            } else if line_len < line.len() {
                line[line_len] = byte;
                line_len += 1;
            } else {
                overflow = true;
            }
            index += 1;
        }
    }

    let _ = fd.close(opened);
    ok
}

fn linked_help_entry(fd: SyscallFdControl, target: &str) -> bool {
    let opened = match fd.open_at(OpenAtDir::session_cwd(), b"/boot/help", OpenFlags::READ_ONLY) {
        Ok(opened) => opened,
        Err(_) => {
            linked_write_command_error(fd, b"help", b"open failed");
            return false;
        }
    };

    let target_bytes = linked_help_target_name(target);
    let mut read_buf = [0u8; 64];
    let mut line = [0u8; LINKED_HELP_LINE_BYTES];
    let mut line_len = 0usize;
    let mut overflow = false;
    let mut in_details = false;
    let mut copying_match = false;
    let mut ok = true;
    let mut found = false;
    let mut done = false;

    while !done {
        let read = match fd.read(opened, &mut read_buf) {
            Ok(read) => read,
            Err(_) => {
                linked_write_command_error(fd, b"help", b"read failed");
                ok = false;
                break;
            }
        };
        if read == 0 {
            break;
        }

        let mut index = 0usize;
        while index < read {
            let byte = read_buf[index];
            if byte == b'\n' {
                if !overflow {
                    match linked_help_entry_line(
                        fd,
                        &line[..line_len],
                        target_bytes,
                        &mut in_details,
                        &mut copying_match,
                        &mut found,
                    ) {
                        Some(line_ok) => {
                            ok = line_ok;
                            done = true;
                            break;
                        }
                        None => {}
                    }
                }
                line_len = 0;
                overflow = false;
            } else if line_len < line.len() {
                line[line_len] = byte;
                line_len += 1;
            } else {
                overflow = true;
            }
            index += 1;
        }
    }

    if ok && !done && line_len > 0 && !overflow {
        if let Some(line_ok) = linked_help_entry_line(
            fd,
            &line[..line_len],
            target_bytes,
            &mut in_details,
            &mut copying_match,
            &mut found,
        ) {
            ok = line_ok;
        }
    }

    let _ = fd.close(opened);
    if ok && found {
        true
    } else if ok {
        linked_help_unknown(fd, target);
        false
    } else {
        false
    }
}

fn linked_help_entry_line(
    fd: SyscallFdControl,
    line: &[u8],
    target: &[u8],
    in_details: &mut bool,
    copying_match: &mut bool,
    found: &mut bool,
) -> Option<bool> {
    if !*in_details {
        if line == b"details:" {
            *in_details = true;
        }
        return None;
    }

    if *copying_match {
        if line.starts_with(b"    ") {
            return if linked_help_write_line(fd, &line[2..]) {
                None
            } else {
                Some(false)
            };
        }
        return Some(true);
    }

    if let Some(entry) = line.strip_prefix(b"  ") {
        if linked_help_entry_matches(entry, target) {
            *copying_match = true;
            *found = true;
            return if linked_help_write_line(fd, entry) {
                None
            } else {
                Some(false)
            };
        }
    }
    None
}

fn linked_help_catalog_line(fd: SyscallFdControl, line: &[u8]) -> bool {
    linked_help_write_line(fd, line)
}

fn linked_help_write_line(fd: SyscallFdControl, line: &[u8]) -> bool {
    linked_write_all(fd, RawFd::stdout(), line) && linked_write_all(fd, RawFd::stdout(), b"\n")
}

fn linked_help_target_name(target: &str) -> &[u8] {
    let bytes = target.as_bytes();
    if bytes.starts_with(b"/bin/") {
        &bytes[5..]
    } else {
        bytes
    }
}

fn linked_help_entry_matches(entry: &[u8], target: &[u8]) -> bool {
    if target.is_empty() || entry.len() < target.len() || &entry[..target.len()] != target {
        return false;
    }
    match entry.get(target.len()) {
        Some(byte) => matches!(*byte, b' ' | b'['),
        None => true,
    }
}

fn linked_help_unknown(fd: SyscallFdControl, target: &str) {
    let _ = fd.write(RawFd::stderr(), b"help: unknown program: ");
    let _ = fd.write(RawFd::stderr(), target.as_bytes());
    let _ = fd.write(RawFd::stderr(), b"\n");
}

fn linked_copy_path_to_stdout(fd: SyscallFdControl, path: &[u8], program: &[u8]) -> bool {
    match fd.open_at(OpenAtDir::session_cwd(), path, OpenFlags::READ_ONLY) {
        Ok(opened) => {
            let ok = linked_copy_fd_to_stdout(fd, opened, program);
            let _ = fd.close(opened);
            ok
        }
        Err(_) => {
            linked_write_command_error(fd, program, b"open failed");
            false
        }
    }
}

fn linked_copy_path_to_stdout_with_line_prefix(
    fd: SyscallFdControl,
    path: &[u8],
    program: &[u8],
    prefix: &[u8],
) -> bool {
    match fd.open_at(OpenAtDir::session_cwd(), path, OpenFlags::READ_ONLY) {
        Ok(opened) => {
            let ok = linked_copy_fd_to_stdout_with_line_prefix(fd, opened, program, prefix);
            let _ = fd.close(opened);
            ok
        }
        Err(_) => {
            linked_write_command_error(fd, program, b"open failed");
            false
        }
    }
}

fn linked_ls_target(fd: SyscallFdControl, target: &str) -> bool {
    match fd.open_at(OpenAtDir::session_cwd(), target.as_bytes(), OpenFlags::READ_DIRECTORY) {
        Ok(opened) => {
            let ok = linked_ls_copy_dir(fd, opened);
            let _ = fd.close(opened);
            ok
        }
        Err(error) if error.code() == 8 => {
            match fd.open_at(OpenAtDir::session_cwd(), target.as_bytes(), OpenFlags::READ_ONLY) {
                Ok(opened) => {
                    let _ = fd.close(opened);
                    linked_write_basename(fd, target)
                }
                Err(file_error) => {
                    linked_write_path_error(fd, b"ls", target, file_error);
                    false
                }
            }
        }
        Err(error) => {
            linked_write_path_error(fd, b"ls", target, error);
            false
        }
    }
}

fn linked_ls_copy_dir(fd: SyscallFdControl, input: RawFd) -> bool {
    let mut buffer = [0u8; 64];
    loop {
        let read = match fd.getdents(input, &mut buffer) {
            Ok(read) => read,
            Err(_) => {
                let _ = fd.write(RawFd::stderr(), b"ls: read directory failed\n");
                return false;
            }
        };
        if read == 0 {
            return true;
        }
        match fd.write(RawFd::stdout(), &buffer[..read]) {
            Ok(written) if written == read => {}
            _ => {
                let _ = fd.write(RawFd::stderr(), b"ls: write failed\n");
                return false;
            }
        }
    }
}

fn linked_write_basename(fd: SyscallFdControl, target: &str) -> bool {
    let name = linked_path_basename(target);
    matches!(fd.write(RawFd::stdout(), name), Ok(written) if written == name.len())
        && matches!(fd.write(RawFd::stdout(), b"\n"), Ok(1))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LinkedCopyStatus {
    Ok,
    Blocked,
    Error,
}

impl LinkedCopyStatus {
    const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }
}

fn linked_cat_copy_fd(fd: SyscallFdControl, input: RawFd) -> LinkedCopyStatus {
    linked_copy_fd_to_stdout_status(fd, input, b"cat")
}

fn linked_read_tty_line(fd: SyscallFdControl) -> bool {
    let tty = match fd.open_at(OpenAtDir::session_cwd(), b"/dev/tty", OpenFlags::READ_ONLY) {
        Ok(tty) => tty,
        Err(error) => {
            linked_write_path_error(fd, b"read", "/dev/tty", error);
            return false;
        }
    };

    let mut line = [0u8; ROOT_LINE_BYTES];
    let read = match fd.read(tty, &mut line) {
        Ok(read) => read,
        Err(_) => {
            let _ = fd.close(tty);
            linked_write_command_error(fd, b"read", b"read failed");
            return false;
        }
    };
    let _ = fd.close(tty);

    if read == 0 {
        let _ = fd.write(RawFd::stderr(), b"read: input eof\n");
        return false;
    }

    linked_write_all(fd, RawFd::stdout(), &line[..read])
        && linked_write_all(fd, RawFd::stdout(), b"\n")
}

fn linked_write_tty_output(fd: SyscallFdControl, program: &[u8], bytes: &[u8]) -> bool {
    let tty = match fd.open_at(OpenAtDir::session_cwd(), b"/dev/tty", OpenFlags::WRITE_ONLY) {
        Ok(tty) => tty,
        Err(error) => {
            linked_write_path_error(fd, program, "/dev/tty", error);
            return false;
        }
    };

    let wrote = linked_write_all(fd, tty, bytes);
    let closed = fd.close(tty).is_ok();
    if wrote && closed {
        true
    } else {
        linked_write_command_error(fd, program, b"tty write failed");
        false
    }
}

fn linked_copy_fd_to_stdout(fd: SyscallFdControl, input: RawFd, program: &[u8]) -> bool {
    linked_copy_fd_to_stdout_status(fd, input, program).is_ok()
}

fn linked_copy_fd_to_stdout_status(
    fd: SyscallFdControl,
    input: RawFd,
    program: &[u8],
) -> LinkedCopyStatus {
    let mut buffer = [0u8; 64];
    loop {
        let read = match fd.read(input, &mut buffer) {
            Ok(read) => read,
            Err(FsError::BUSY) => return LinkedCopyStatus::Blocked,
            Err(_) => {
                linked_write_command_error(fd, program, b"read failed");
                return LinkedCopyStatus::Error;
            }
        };
        if read == 0 {
            return LinkedCopyStatus::Ok;
        }
        match fd.write(RawFd::stdout(), &buffer[..read]) {
            Ok(written) if written == read => {}
            _ => {
                linked_write_command_error(fd, program, b"write failed");
                return LinkedCopyStatus::Error;
            }
        }
    }
}

fn linked_copy_fd_to_stdout_with_line_prefix(
    fd: SyscallFdControl,
    input: RawFd,
    program: &[u8],
    prefix: &[u8],
) -> bool {
    let mut buffer = [0u8; 64];
    let mut at_line_start = true;
    loop {
        let read = match fd.read(input, &mut buffer) {
            Ok(read) => read,
            Err(_) => {
                linked_write_command_error(fd, program, b"read failed");
                return false;
            }
        };
        if read == 0 {
            return true;
        }

        let mut start = 0usize;
        while start < read {
            if at_line_start && !linked_write_all(fd, RawFd::stdout(), prefix) {
                linked_write_command_error(fd, program, b"write failed");
                return false;
            }
            at_line_start = false;

            let mut end = start;
            while end < read && buffer[end] != b'\n' {
                end += 1;
            }
            if end < read {
                end += 1;
                at_line_start = true;
            }

            if !linked_write_all(fd, RawFd::stdout(), &buffer[start..end]) {
                linked_write_command_error(fd, program, b"write failed");
                return false;
            }
            start = end;
        }
    }
}

fn linked_path_basename(path: &str) -> &[u8] {
    let bytes = path.as_bytes();
    if bytes == b"/" {
        return b"/";
    }
    let mut end = bytes.len();
    while end > 1 && bytes[end - 1] == b'/' {
        end -= 1;
    }
    let mut start = end;
    while start > 0 {
        if bytes[start - 1] == b'/' {
            break;
        }
        start -= 1;
    }
    &bytes[start..end]
}

fn linked_write_path_error(fd: SyscallFdControl, program: &[u8], target: &str, error: FsError) {
    let _ = fd.write(RawFd::stderr(), program);
    let _ = fd.write(RawFd::stderr(), b": ");
    let _ = fd.write(RawFd::stderr(), target.as_bytes());
    let _ = fd.write(RawFd::stderr(), b": ");
    let _ = fd.write(RawFd::stderr(), linked_fs_error_word(error));
    let _ = fd.write(RawFd::stderr(), b"\n");
}

fn linked_write_command_error(fd: SyscallFdControl, program: &[u8], message: &[u8]) {
    let _ = fd.write(RawFd::stderr(), program);
    let _ = fd.write(RawFd::stderr(), b": ");
    let _ = fd.write(RawFd::stderr(), message);
    let _ = fd.write(RawFd::stderr(), b"\n");
}

fn linked_write_all(fd: SyscallFdControl, target: RawFd, bytes: &[u8]) -> bool {
    matches!(fd.write(target, bytes), Ok(written) if written == bytes.len())
}

fn linked_write_bool(fd: SyscallFdControl, target: RawFd, value: bool) -> bool {
    linked_write_all(fd, target, if value { b"true" } else { b"false" })
}

fn linked_write_usize(fd: SyscallFdControl, target: RawFd, mut value: usize) -> bool {
    let mut buf = [0u8; 20];
    let mut index = buf.len();
    if value == 0 {
        index -= 1;
        buf[index] = b'0';
    } else {
        while value > 0 {
            index -= 1;
            buf[index] = b'0' + (value % 10) as u8;
            value /= 10;
        }
    }
    linked_write_all(fd, target, &buf[index..])
}

fn linked_write_i32(fd: SyscallFdControl, target: RawFd, value: i32) -> bool {
    if value < 0 {
        linked_write_all(fd, target, b"-")
            && linked_write_usize(fd, target, value.unsigned_abs() as usize)
    } else {
        linked_write_usize(fd, target, value as usize)
    }
}

fn linked_write_process_control_report(
    fd: SyscallFdControl,
    label: &[u8],
    report: &ProcessControlReport,
) -> bool {
    linked_write_all(fd, RawFd::stdout(), label)
        && linked_write_all(fd, RawFd::stdout(), b":\npid=")
        && linked_write_usize(fd, RawFd::stdout(), report.pid().raw())
        && linked_write_all(fd, RawFd::stdout(), b"\npath=")
        && linked_write_all(fd, RawFd::stdout(), report.path_bytes())
        && (!report.path_truncated() || linked_write_all(fd, RawFd::stdout(), b"..."))
        && linked_write_all(fd, RawFd::stdout(), b"\nstate=")
        && linked_write_all(fd, RawFd::stdout(), linked_process_state_word(report.state()))
        && linked_write_all(fd, RawFd::stdout(), b"\nloader=")
        && linked_write_all(fd, RawFd::stdout(), report.loader_bytes())
        && (!report.loader_truncated() || linked_write_all(fd, RawFd::stdout(), b"..."))
        && linked_write_all(fd, RawFd::stdout(), b"\nentry_fn=")
        && linked_write_all(fd, RawFd::stdout(), report.entry_name_bytes())
        && (!report.entry_name_truncated() || linked_write_all(fd, RawFd::stdout(), b"..."))
        && linked_write_all(fd, RawFd::stdout(), b"\nartifact_body_format=")
        && linked_write_all(fd, RawFd::stdout(), report.body_format_bytes())
        && (!report.body_format_truncated() || linked_write_all(fd, RawFd::stdout(), b"..."))
        && linked_write_all(fd, RawFd::stdout(), b"\nartifact_body_inner=")
        && linked_write_all(fd, RawFd::stdout(), report.body_inner_bytes())
        && (!report.body_inner_truncated() || linked_write_all(fd, RawFd::stdout(), b"..."))
        && linked_write_all(fd, RawFd::stdout(), b"\nartifact_body_bytes=")
        && linked_write_usize(fd, RawFd::stdout(), report.body_bytes())
        && linked_write_all(fd, RawFd::stdout(), b"\nartifact_checksum=")
        && linked_write_usize(fd, RawFd::stdout(), report.body_checksum() as usize)
        && linked_write_all(fd, RawFd::stdout(), b"\n")
}

fn linked_write_process_wait_report(fd: SyscallFdControl, report: &ProcessWaitReport) -> bool {
    linked_write_all(fd, RawFd::stdout(), b"wait:\npid=")
        && linked_write_usize(fd, RawFd::stdout(), report.child_pid().raw())
        && linked_write_all(fd, RawFd::stdout(), b"\nstate=")
        && linked_write_all(fd, RawFd::stdout(), linked_process_state_word(report.child_state()))
        && linked_write_all(fd, RawFd::stdout(), b"\nexit=")
        && linked_write_i32(fd, RawFd::stdout(), report.exit_code())
        && linked_write_all(fd, RawFd::stdout(), b"\ncompleted=")
        && linked_write_bool(fd, RawFd::stdout(), report.completed())
        && linked_write_all(fd, RawFd::stdout(), b"\n")
}

fn linked_write_process_sleep_report(fd: SyscallFdControl, report: &ProcessSleepReport) -> bool {
    linked_write_all(fd, RawFd::stdout(), b"sleep:\npid=")
        && linked_write_usize(fd, RawFd::stdout(), report.pid().raw())
        && linked_write_all(fd, RawFd::stdout(), b"\npath=")
        && linked_write_all(fd, RawFd::stdout(), report.path_bytes())
        && (!report.path_truncated() || linked_write_all(fd, RawFd::stdout(), b"..."))
        && linked_write_all(fd, RawFd::stdout(), b"\nstate=")
        && linked_write_all(fd, RawFd::stdout(), linked_process_state_word(report.state()))
        && linked_write_all(fd, RawFd::stdout(), b"\nblock=sleep\nwake_tick=")
        && linked_write_usize(fd, RawFd::stdout(), report.wake_tick())
        && linked_write_all(fd, RawFd::stdout(), b"\n")
}

fn linked_write_process_timed_wait_report(
    fd: SyscallFdControl,
    report: &ProcessTimedWaitReport,
) -> bool {
    linked_write_all(fd, RawFd::stdout(), b"wait-ticks:\npid=")
        && linked_write_usize(fd, RawFd::stdout(), report.child_pid().raw())
        && linked_write_all(fd, RawFd::stdout(), b"\nstate=")
        && linked_write_all(fd, RawFd::stdout(), linked_process_state_word(report.child_state()))
        && linked_write_all(fd, RawFd::stdout(), b"\nexit=")
        && linked_write_i32(fd, RawFd::stdout(), report.exit_code())
        && linked_write_all(fd, RawFd::stdout(), b"\ncompleted=")
        && linked_write_bool(fd, RawFd::stdout(), report.completed())
        && linked_write_all(fd, RawFd::stdout(), b"\ntimed_out=")
        && linked_write_bool(fd, RawFd::stdout(), report.timed_out())
        && linked_write_all(fd, RawFd::stdout(), b"\ntick_count=")
        && linked_write_usize(fd, RawFd::stdout(), report.tick_count())
        && linked_write_all(fd, RawFd::stdout(), b"\n")
}

fn linked_write_service_control_report(
    fd: SyscallFdControl,
    label: &[u8],
    report: &ServiceControlReport,
) -> bool {
    linked_write_all(fd, RawFd::stdout(), label)
        && linked_write_all(fd, RawFd::stdout(), b":\nname=")
        && linked_write_all(fd, RawFd::stdout(), report.name_bytes())
        && (!report.name_truncated() || linked_write_all(fd, RawFd::stdout(), b"..."))
        && linked_write_all(fd, RawFd::stdout(), b"\ntarget=")
        && linked_write_all(fd, RawFd::stdout(), report.target_bytes())
        && (!report.target_truncated() || linked_write_all(fd, RawFd::stdout(), b"..."))
        && linked_write_service_control_result(fd, report)
        && linked_write_all(fd, RawFd::stdout(), b"\nservice_pid=")
        && linked_write_usize(fd, RawFd::stdout(), report.service_pid())
        && linked_write_all(fd, RawFd::stdout(), b"\nservice_task=")
        && linked_write_usize(fd, RawFd::stdout(), report.service_task_id())
        && linked_write_all(fd, RawFd::stdout(), b"\nstate=")
        && linked_write_all(fd, RawFd::stdout(), linked_service_state_word(report.state()))
        && linked_write_all(fd, RawFd::stdout(), b"\nreason=")
        && linked_write_all(fd, RawFd::stdout(), linked_service_reason_word(report.reason()))
        && linked_write_all(fd, RawFd::stdout(), b"\n")
}

fn linked_write_service_control_result(
    fd: SyscallFdControl,
    report: &ServiceControlReport,
) -> bool {
    if report.result() == ServiceControlResultCode::NONE {
        return true;
    }
    let mut ok = linked_write_all(fd, RawFd::stdout(), b"\nresult=")
        && linked_write_all(fd, RawFd::stdout(), linked_service_result_word(report.result()));
    if report.result() == ServiceControlResultCode::PAYLOAD_EXIT_CODE {
        ok = ok
            && linked_write_all(fd, RawFd::stdout(), b"\nexit_code=")
            && linked_write_i32(fd, RawFd::stdout(), report.exit_code());
    }
    ok
}

fn linked_service_result_success(result: ServiceControlResultCode, exit_code: i32) -> bool {
    result == ServiceControlResultCode::NONE
        || result == ServiceControlResultCode::PAYLOAD_READY
        || result == ServiceControlResultCode::PAYLOAD_RESIDENT
        || (result == ServiceControlResultCode::PAYLOAD_EXIT_CODE && exit_code == 0)
}

fn linked_write_source_install_report(
    fd: SyscallFdControl,
    label: &[u8],
    report: &SourceInstallReport,
) -> bool {
    let mut ok = linked_write_all(fd, RawFd::stdout(), label)
        && linked_write_all(fd, RawFd::stdout(), b":\nname=")
        && linked_write_all(fd, RawFd::stdout(), report.name_bytes())
        && (!report.name_truncated() || linked_write_all(fd, RawFd::stdout(), b"..."))
        && linked_write_all(fd, RawFd::stdout(), b"\nnamespace=")
        && linked_write_all(fd, RawFd::stdout(), linked_source_namespace_word(report.namespace()))
        && linked_write_all(fd, RawFd::stdout(), b"\npath=")
        && linked_write_all(fd, RawFd::stdout(), report.path_bytes())
        && (!report.path_truncated() || linked_write_all(fd, RawFd::stdout(), b"..."));

    if report.status() != SourceInstallStatusCode::NONE {
        ok = ok
            && linked_write_all(fd, RawFd::stdout(), b"\nstatus=")
            && linked_write_all(fd, RawFd::stdout(), linked_source_status_word(report.status()));
    }

    if report.origin() == SourceInstallOriginCode::SOURCE_MEDIA {
        ok = ok
            && linked_write_all(fd, RawFd::stdout(), b"\nstorage=")
            && linked_write_all(fd, RawFd::stdout(), report.storage_bytes())
            && (!report.storage_truncated() || linked_write_all(fd, RawFd::stdout(), b"..."))
            && linked_write_all(fd, RawFd::stdout(), b"\nstorage_capacity_bytes=")
            && linked_write_usize(fd, RawFd::stdout(), report.storage_capacity_bytes())
            && linked_write_all(fd, RawFd::stdout(), b"\nartifact_bytes=")
            && linked_write_usize(fd, RawFd::stdout(), report.artifact_bytes_len());
    }

    ok = ok
        && linked_write_all(fd, RawFd::stdout(), b"\nbytes=")
        && linked_write_usize(fd, RawFd::stdout(), report.bytes_len());

    if report.origin() == SourceInstallOriginCode::SOURCE_MEDIA {
        ok = ok
            && linked_write_all(fd, RawFd::stdout(), b"\nchecksum=")
            && linked_write_usize(fd, RawFd::stdout(), report.checksum() as usize)
            && linked_write_all(fd, RawFd::stdout(), b"\norigin=installed\nsource=source-media\n");
    } else {
        ok = ok
            && linked_write_all(fd, RawFd::stdout(), b"\norigin=")
            && linked_write_all(fd, RawFd::stdout(), linked_source_origin_word(report.origin()))
            && linked_write_all(fd, RawFd::stdout(), b"\n");
    }
    ok
}

fn linked_write_source_error(
    fd: SyscallFdControl,
    label: &[u8],
    error: SourceError,
    not_found_word: &[u8],
) {
    let _ = fd.write(RawFd::stderr(), label);
    let _ = fd.write(RawFd::stderr(), b": ");
    let _ = fd.write(RawFd::stderr(), linked_source_error_word(error, not_found_word));
    let _ = fd.write(RawFd::stderr(), b"\n");
}

fn linked_parse_bin_source_install_status(value: &str) -> Option<SourceInstallStatusCode> {
    match value.as_bytes() {
        b"ok" => Some(SourceInstallStatusCode::OK),
        b"error" => Some(SourceInstallStatusCode::ERROR),
        _ => None,
    }
}

fn linked_parse_payload_source_install_status(value: &str) -> Option<SourceInstallStatusCode> {
    match value.as_bytes() {
        b"ready" => Some(SourceInstallStatusCode::READY),
        b"failed" => Some(SourceInstallStatusCode::FAILED),
        _ => None,
    }
}

fn linked_source_namespace_word(namespace: SourceNamespaceCode) -> &'static [u8] {
    if namespace == SourceNamespaceCode::BIN {
        b"bin"
    } else if namespace == SourceNamespaceCode::PAYLOAD {
        b"payload"
    } else {
        b"none"
    }
}

fn linked_source_status_word(status: SourceInstallStatusCode) -> &'static [u8] {
    if status == SourceInstallStatusCode::OK {
        b"ok"
    } else if status == SourceInstallStatusCode::ERROR {
        b"error"
    } else if status == SourceInstallStatusCode::READY {
        b"ready"
    } else if status == SourceInstallStatusCode::FAILED {
        b"failed"
    } else {
        b"none"
    }
}

fn linked_source_origin_word(origin: SourceInstallOriginCode) -> &'static [u8] {
    if origin == SourceInstallOriginCode::INSTALLED {
        b"installed"
    } else if origin == SourceInstallOriginCode::SOURCE_MEDIA {
        b"source-media"
    } else {
        b"none"
    }
}

fn linked_source_error_word(error: SourceError, not_found_word: &[u8]) -> &'static [u8] {
    if error == SourceError::NOT_FOUND {
        return if not_found_word == b"payload-not-found" {
            b"payload-not-found"
        } else {
            b"program-not-found"
        };
    }
    if error == SourceError::NO_CURRENT_PROCESS {
        b"no-current-process"
    } else if error == SourceError::SOURCE_MEDIA_UNAVAILABLE {
        b"source-media-unavailable"
    } else if error == SourceError::SOURCE_MEDIA_READ_FAILED {
        b"source-media-read-failed"
    } else if error == SourceError::SOURCE_MEDIA_NAMESPACE_MISMATCH {
        b"source-media-namespace-mismatch"
    } else if error == SourceError::SOURCE_MEDIA_PATH_MISMATCH {
        b"source-media-path-mismatch"
    } else if error == SourceError::INVALID_IMAGE {
        b"source-media-invalid"
    } else if error == SourceError::UNSUPPORTED {
        b"unsupported-image"
    } else if error == SourceError::FILE_TOO_LARGE {
        b"too-large"
    } else if error == SourceError::BUSY {
        b"no-slot"
    } else if error == SourceError::INVALID_ARGUMENT {
        b"invalid-argument"
    } else {
        b"failed"
    }
}

fn linked_process_state_word(state: ProcessStateCode) -> &'static [u8] {
    if state == ProcessStateCode::EMPTY {
        b"empty"
    } else if state == ProcessStateCode::NEW {
        b"new"
    } else if state == ProcessStateCode::READY {
        b"ready"
    } else if state == ProcessStateCode::RUNNING {
        b"running"
    } else if state == ProcessStateCode::BLOCKED {
        b"blocked"
    } else if state == ProcessStateCode::EXITED {
        b"exited"
    } else if state == ProcessStateCode::FAILED {
        b"failed"
    } else if state == ProcessStateCode::HALTED {
        b"halted"
    } else if state == ProcessStateCode::REAPED {
        b"reaped"
    } else {
        b"unknown"
    }
}

fn linked_service_result_word(result: ServiceControlResultCode) -> &'static [u8] {
    if result == ServiceControlResultCode::NONE {
        b"none"
    } else if result == ServiceControlResultCode::PAYLOAD_READY {
        b"payload.ready"
    } else if result == ServiceControlResultCode::PAYLOAD_RESIDENT {
        b"payload.resident"
    } else if result == ServiceControlResultCode::PAYLOAD_NOT_CONFIGURED {
        b"payload.not_configured"
    } else if result == ServiceControlResultCode::PAYLOAD_FAILED {
        b"payload.failed"
    } else if result == ServiceControlResultCode::PAYLOAD_EXIT_CODE {
        b"payload.exit_code"
    } else {
        b"unknown"
    }
}

fn linked_service_state_word(state: ServiceStateCode) -> &'static [u8] {
    if state == ServiceStateCode::EMPTY {
        b"empty"
    } else if state == ServiceStateCode::REQUESTED {
        b"requested"
    } else if state == ServiceStateCode::STARTED {
        b"started"
    } else if state == ServiceStateCode::EXITED {
        b"exited"
    } else if state == ServiceStateCode::STOPPED {
        b"stopped"
    } else if state == ServiceStateCode::FAILED {
        b"failed"
    } else {
        b"unknown"
    }
}

fn linked_service_reason_word(reason: ServiceReasonCode) -> &'static [u8] {
    if reason == ServiceReasonCode::NONE {
        b"none"
    } else if reason == ServiceReasonCode::REQUESTED {
        b"requested"
    } else if reason == ServiceReasonCode::RUNNING {
        b"running"
    } else if reason == ServiceReasonCode::PROCESS_EXITED {
        b"process-exited"
    } else if reason == ServiceReasonCode::PROCESS_FAILED {
        b"process-failed"
    } else if reason == ServiceReasonCode::PROCESS_KILLED {
        b"process-killed"
    } else if reason == ServiceReasonCode::OPERATOR_STOP {
        b"operator-stop"
    } else if reason == ServiceReasonCode::EXEC_LOAD_ERROR {
        b"exec-load-error"
    } else if reason == ServiceReasonCode::HALT {
        b"halt"
    } else if reason == ServiceReasonCode::START_ERROR {
        b"start-error"
    } else {
        b"unknown"
    }
}

fn linked_reovim_exit_with_payload_status(
    fd: SyscallFdControl,
    process: SyscallProcessControl,
    code: ExitCode,
) -> ProgramStatus {
    let raw = code.raw();
    let ok = match raw {
        0 => linked_write_all(fd, RawFd::stdout(), b"payload.ready\n"),
        1 => linked_write_all(fd, RawFd::stdout(), b"payload.failed\n"),
        _ => {
            linked_write_all(fd, RawFd::stdout(), b"payload.exit_code(")
                && linked_write_usize(fd, RawFd::stdout(), raw as usize)
                && linked_write_all(fd, RawFd::stdout(), b")\n")
        }
    };
    let exit = if ok { code } else { ExitCode::FAILURE };
    let _ = process.exit(exit);
    ProgramStatus::Error
}

fn linked_launch_exit_with_payload_report(
    fd: SyscallFdControl,
    process: SyscallProcessControl,
    payload: &str,
    report: ProcessWaitReport,
) -> ProgramStatus {
    if !(linked_write_all(fd, RawFd::stdout(), b"launch ")
        && linked_write_all(fd, RawFd::stdout(), payload.as_bytes())
        && linked_write_all(fd, RawFd::stdout(), b": "))
    {
        let _ = process.exit(ExitCode::FAILURE);
        return ProgramStatus::Error;
    }
    if !report.completed() {
        let ok = report.child_state() == ProcessStateCode::BLOCKED
            && linked_write_all(fd, RawFd::stdout(), b"payload.resident\n");
        let _ = process.exit(if ok {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        });
        return if ok {
            ProgramStatus::Ok
        } else {
            ProgramStatus::Error
        };
    }
    let code = report.exit_code();
    if code < 0 || code > u8::MAX as i32 {
        let _ = linked_write_all(fd, RawFd::stdout(), b"payload.failed\n");
        let _ = process.exit(ExitCode::FAILURE);
        return ProgramStatus::Error;
    }
    linked_reovim_exit_with_payload_status(fd, process, ExitCode::new(code as u8))
}

fn linked_write_launch_payload_status(fd: SyscallFdControl, payload: &str, status: &[u8]) -> bool {
    linked_write_all(fd, RawFd::stdout(), b"launch ")
        && linked_write_all(fd, RawFd::stdout(), payload.as_bytes())
        && linked_write_all(fd, RawFd::stdout(), b": ")
        && linked_write_all(fd, RawFd::stdout(), status)
        && linked_write_all(fd, RawFd::stdout(), b"\n")
}

fn linked_launch_enabled(fd: SyscallFdControl) -> Option<bool> {
    let profile = fd
        .open_at(OpenAtDir::session_cwd(), b"/boot/profile", OpenFlags::READ_ONLY)
        .ok()?;
    let mut buffer = [0u8; 512];
    let read = match fd.read(profile, &mut buffer) {
        Ok(read) => read,
        Err(_) => {
            let _ = fd.close(profile);
            return None;
        }
    };
    let _ = fd.close(profile);
    Some(linked_bytes_contains(&buffer[..read], b"launch=enabled\n"))
}

fn linked_bytes_contains(bytes: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.len() > bytes.len() {
        return false;
    }
    let mut index = 0usize;
    while index + needle.len() <= bytes.len() {
        if &bytes[index..index + needle.len()] == needle {
            return true;
        }
        index += 1;
    }
    false
}

fn linked_parse_usize(value: &str) -> Option<usize> {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let mut parsed = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if !byte.is_ascii_digit() {
            return None;
        }
        parsed = parsed.checked_mul(10)?;
        parsed = parsed.checked_add((byte - b'0') as usize)?;
        index += 1;
    }
    Some(parsed)
}

struct LinkedChildInvocation {
    argv: [ProcessArg; MAX_PROGRAM_ARGS],
    argc: usize,
    env: [ProcessEnv; MAX_PROGRAM_ENVS],
    envc: usize,
}

impl LinkedChildInvocation {
    fn empty() -> Self {
        Self {
            argv: [ProcessArg::from_str(""); MAX_PROGRAM_ARGS],
            argc: 0,
            env: [ProcessEnv::from_pair("", ""); MAX_PROGRAM_ENVS],
            envc: 0,
        }
    }

    fn argv(&self) -> &[ProcessArg] {
        &self.argv[..self.argc]
    }

    fn env(&self) -> &[ProcessEnv] {
        &self.env[..self.envc]
    }
}

enum LinkedChildInvocationError {
    MissingProgram,
    TooManyArguments,
    TooManyEnvVars,
}

fn linked_split_env_assignment(token: &str) -> Option<(&str, &str)> {
    let bytes = token.as_bytes();
    let mut split = 0usize;
    while split < bytes.len() && bytes[split] != b'=' {
        split += 1;
    }
    if split == 0 || split >= bytes.len() {
        return None;
    }
    let name = &bytes[..split];
    if !kernel_program::program_env_name_is_valid(name) {
        return None;
    }
    Some((&token[..split], &token[split + 1..]))
}

fn linked_program_name_after_env<'a>(argv: &ProgramArgv<'a>, start: usize) -> Option<&'a str> {
    let mut source = start;
    while source < argv.argc() {
        let token = argv.arg(source).unwrap_or("");
        if linked_split_env_assignment(token).is_none() {
            return Some(token);
        }
        source += 1;
    }
    None
}

fn linked_child_invocation(
    argv: &ProgramArgv<'_>,
    start: usize,
) -> Result<LinkedChildInvocation, LinkedChildInvocationError> {
    let mut child = LinkedChildInvocation::empty();
    let mut source = start;
    while source < argv.argc() {
        let token = argv.arg(source).unwrap_or("");
        let Some((name, value)) = linked_split_env_assignment(token) else {
            break;
        };
        if child.envc >= child.env.len() {
            return Err(LinkedChildInvocationError::TooManyEnvVars);
        }
        child.env[child.envc] = ProcessEnv::from_pair(name, value);
        child.envc += 1;
        source += 1;
    }

    if source >= argv.argc() {
        return Err(LinkedChildInvocationError::MissingProgram);
    }

    while source < argv.argc() {
        if child.argc >= child.argv.len() {
            return Err(LinkedChildInvocationError::TooManyArguments);
        }
        child.argv[child.argc] = ProcessArg::from_str(argv.arg(source).unwrap_or(""));
        child.argc += 1;
        source += 1;
    }

    Ok(child)
}

fn linked_implicit_child_invocation(
    argv: &ProgramArgv<'_>,
    start: usize,
    program: &'static str,
) -> Result<LinkedChildInvocation, LinkedChildInvocationError> {
    let mut child = LinkedChildInvocation::empty();
    child.argv[0] = ProcessArg::from_str(program);
    child.argc = 1;

    let mut source = start;
    while source < argv.argc() {
        let token = argv.arg(source).unwrap_or("");
        let Some((name, value)) = linked_split_env_assignment(token) else {
            break;
        };
        if child.envc >= child.env.len() {
            return Err(LinkedChildInvocationError::TooManyEnvVars);
        }
        child.env[child.envc] = ProcessEnv::from_pair(name, value);
        child.envc += 1;
        source += 1;
    }

    while source < argv.argc() {
        if child.argc >= child.argv.len() {
            return Err(LinkedChildInvocationError::TooManyArguments);
        }
        child.argv[child.argc] = ProcessArg::from_str(argv.arg(source).unwrap_or(""));
        child.argc += 1;
        source += 1;
    }

    Ok(child)
}

fn linked_fs_error_word(error: FsError) -> &'static [u8] {
    match error.code() {
        7 => b"not found",
        8 => b"not a directory",
        9 => b"busy",
        10 => b"file too large",
        11 => b"io error",
        _ => b"open failed",
    }
}

/// Returns the `/bin` program catalog for this image.
#[must_use]
pub(super) const fn programs() -> &'static [ProgramDescriptor] {
    &BIN_PROGRAMS
}

/// Returns the `/bin` source artifact store for this image.
#[must_use]
pub(super) const fn program_sources() -> &'static [ProgramSourceArtifact] {
    &BIN_PROGRAM_SOURCES
}

pub(super) fn write_program_help(
    program_name: Option<&str>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some(program_name) = program_name else {
        write_detailed_help_catalog(syscalls);
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

pub(super) fn write_vfs_file(file: File, syscalls: &mut ProgramSyscalls<'_, '_, '_>) {
    match file {
        File::BinProgram(index) => syscalls.write_program_file_metadata(index),
        File::BootHelp => {
            let _ = syscalls.write_program_help(None);
        }
        File::BootProfile => syscalls.write_boot_profile(),
        File::BootImage => syscalls.write_boot_image(),
        File::BootInput => syscalls.write_boot_input(),
        File::BootProof => syscalls.write_boot_proof(),
        File::BootProbes => syscalls.write_probe_catalog(),
        File::BootPayloads => syscalls.write_boot_payloads(),
        File::BootStatus => syscalls.write_boot_status(),
        File::BootMemory => syscalls.write_boot_memory(),
        File::BootDevices => syscalls.write_boot_devices(),
        File::BootMounts => syscalls.write_mount_table(),
        File::DumpStatus => syscalls.write_dump_status(),
        File::DumpSnapshot => syscalls.write_dump_snapshot(),
        File::LogDmesg => syscalls.write_kernel_log_view(),
        File::LogEvents => syscalls.write_kernel_event_table(),
        File::LogStats => syscalls.write_kernel_log_stats(),
        File::ProcExecs => syscalls.write_exec_load_table(),
        File::ProcAddressSpaces => syscalls.write_address_space_table(),
        File::ProcPageTables => syscalls.write_address_space_page_table_table(),
        File::ProcPages => syscalls.write_address_space_page_table_entry_table(),
        File::ProcMemoryObjects => syscalls.write_address_space_object_table(),
        File::ProcMedia => syscalls.write_source_media_table(),
        File::ProcPending => syscalls.write_pending_exec_table(),
        File::ProcProcesses => syscalls.write_process_table(),
        File::ProcContinuations => syscalls.write_syscall_continuation_table(),
        File::ProcSelf => syscalls.write_current_process(),
        File::ProcSession => syscalls.write_session_state(),
        File::ProcServices => syscalls.write_service_table(),
        File::ProcScheduler => syscalls.write_scheduler_state(),
        File::ProcSources => syscalls.write_source_store_table(),
        File::ProcSyscalls => syscalls.write_syscall_table(),
        File::ProcTasks => syscalls.write_task_table(),
        File::ProcWaits => syscalls.write_wait_table(),
        File::DevTty => {}
        File::DevDevice(index) => syscalls.write_device_file_metadata(index),
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
        BIN_INIT => write_help_line(syscalls, prefix, "init - start userland session services"),
        BIN_SH => write_help_line(syscalls, prefix, "sh - run interactive root shell"),
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
            "sched [status|tick|yield|sleep TICKS] - inspect scheduler state, tick, yield, or sleep current task",
        ),
        BIN_PROC => write_help_line(
            syscalls,
            prefix,
            "proc [processes|execs|address-spaces|page-tables|pages|memory-objects|media|pending|self|session|services|sources|tasks|waits|syscalls|continuations|scheduler] - inspect process state",
        ),
        BIN_PROBE => write_help_line(
            syscalls,
            prefix,
            "probe target - run lower hardware probe; try `probe help` or `cat /boot/probes`",
        ),
        BIN_LAUNCH => write_help_line(
            syscalls,
            prefix,
            "launch [NAME=VALUE ...] [payload] [arg...] - list or run registered payloads",
        ),
        BIN_REOVIM => write_help_line(
            syscalls,
            prefix,
            "reovim [NAME=VALUE ...] [arg...] - run the default reovim payload alias",
        ),
        BIN_HELLO => write_help_line(syscalls, prefix, "hello - print a linked-bin syscall proof"),
        BIN_HALT => write_help_line(syscalls, prefix, "halt - request root daemon shutdown"),
        BIN_PS => write_help_line(syscalls, prefix, "ps - print retained process table"),
        BIN_KILL => write_help_line(
            syscalls,
            prefix,
            "kill PID - terminate a retained ready or blocked process",
        ),
        BIN_WAKE => {
            write_help_line(syscalls, prefix, "wake PID - wake an operator-blocked process")
        }
        BIN_BLOCK => write_help_line(
            syscalls,
            prefix,
            "block [NAME=VALUE ...] PROGRAM [ARG...] - spawn a retained blocked process",
        ),
        BIN_SPAWN => write_help_line(
            syscalls,
            prefix,
            "spawn [NAME=VALUE ...] PROGRAM [ARG...] - spawn a retained ready process",
        ),
        BIN_SLEEP => write_help_line(
            syscalls,
            prefix,
            "sleep TICKS [NAME=VALUE ...] PROGRAM [ARG...] - spawn a retained process blocked until scheduler ticks",
        ),
        BIN_WAIT => write_help_line(syscalls, prefix, "wait PID - wait for a retained process"),
        BIN_WAIT_TICKS => write_help_line(
            syscalls,
            prefix,
            "wait-ticks TICKS PID - wait with scheduler ticks for a retained process",
        ),
        BIN_EXEC => write_help_line(
            syscalls,
            prefix,
            "exec [NAME=VALUE ...] PROGRAM [ARG...] - replace current process image",
        ),
        BIN_SERVICE_STOP => write_help_line(
            syscalls,
            prefix,
            "service-stop NAME - stop a retained resident service",
        ),
        BIN_SERVICE_START => write_help_line(
            syscalls,
            prefix,
            "service-start NAME - start a retained payload service",
        ),
        BIN_SERVICE_RESTART => write_help_line(
            syscalls,
            prefix,
            "service-restart NAME - restart a retained payload service",
        ),
        BIN_SESSION => {
            write_help_line(syscalls, prefix, "session - print active shell session state")
        }
        BIN_SERVICES => {
            write_help_line(syscalls, prefix, "services - print retained service table")
        }
        BIN_TASKS => write_help_line(syscalls, prefix, "tasks - print retained task table"),
        BIN_WAITS => write_help_line(syscalls, prefix, "waits - print retained wait table"),
        BIN_SYSCALLS => {
            write_help_line(syscalls, prefix, "syscalls - print retained syscall trace")
        }
        BIN_CONTINUATIONS => {
            write_help_line(syscalls, prefix, "continuations - print active syscall continuations")
        }
        BIN_EXECS => write_help_line(syscalls, prefix, "execs - print executable admission table"),
        BIN_PENDING => {
            write_help_line(syscalls, prefix, "pending - print pending executable table")
        }
        BIN_SOURCES => write_help_line(syscalls, prefix, "sources - print executable source table"),
        BIN_MEDIA => write_help_line(syscalls, prefix, "media - print executable media status"),
        BIN_SELF => write_help_line(syscalls, prefix, "self - print current process state"),
        BIN_INSTALL_BIN => write_help_line(
            syscalls,
            prefix,
            "install-bin NAME ok|error - install a status-only /bin source image",
        ),
        BIN_INSTALL_PAYLOAD => write_help_line(
            syscalls,
            prefix,
            "install-payload NAME ready|failed - install a status-only payload source image",
        ),
        BIN_INSTALL_BIN_MEDIA => write_help_line(
            syscalls,
            prefix,
            "install-bin-media NAME - install a /bin source image from source media",
        ),
        BIN_INSTALL_PAYLOAD_MEDIA => write_help_line(
            syscalls,
            prefix,
            "install-payload-media NAME - install a payload source image from source media",
        ),
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
