use {
    reovim_uapi_session::{SessionControlOp, SessionError, SyscallSessionControl},
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr, SyscallRet},
};

#[test]
fn session_scalars_are_stable() {
    assert_eq!(SessionControlOp::SHELL_START.raw(), 0);
    assert_eq!(SessionControlOp::LINE_DISCIPLINE.raw(), 1);
    assert_eq!(SessionControlOp::new(0), SessionControlOp::SHELL_START);
    assert_eq!(SessionControlOp::new(1), SessionControlOp::LINE_DISCIPLINE);
    assert_eq!(SessionError::NO_CURRENT_PROCESS.code(), SyscallError::NO_CURRENT_PROCESS.code());
}

#[test]
fn syscall_session_control_lowers_shell_start_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SESSION_CONTROL);
        assert_eq!(args.a0, SessionControlOp::SHELL_START.raw());
        assert_eq!(args.a1, 0);
        assert_eq!(args.a2, 0);
        assert_eq!(args.a3, 0);
        assert_eq!(args.a4, 0);
        assert_eq!(args.a5, 0);
        SyscallRet::success(0)
    }

    let session = SyscallSessionControl::new(RawSyscall::new(syscall));
    session
        .request_shell_start()
        .expect("session start succeeds");
}

#[test]
fn syscall_session_control_lowers_line_discipline_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SESSION_CONTROL);
        assert_eq!(args.a0, SessionControlOp::LINE_DISCIPLINE.raw());
        let line_discipline = "argv-v1";
        let pipe_mode = "single-pipe";
        assert_eq!(args.a1, line_discipline.as_ptr().addr());
        assert_eq!(args.a2, line_discipline.len());
        assert_eq!(args.a3, pipe_mode.as_ptr().addr());
        assert_eq!(args.a4, pipe_mode.len());
        assert_eq!(args.a5, 0);
        SyscallRet::success(0)
    }

    let session = SyscallSessionControl::new(RawSyscall::new(syscall));
    session
        .request_line_discipline("argv-v1", "single-pipe")
        .expect("line discipline request succeeds");
}

#[test]
fn syscall_session_control_maps_transport_errors() {
    fn no_current(_: SyscallNr, _: SyscallArgs) -> SyscallRet {
        SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS)
    }

    let session = SyscallSessionControl::new(RawSyscall::new(no_current));
    assert_eq!(session.request_shell_start(), Err(SessionError::NO_CURRENT_PROCESS));
    assert_eq!(
        session.request_line_discipline("argv-v1", "single-pipe"),
        Err(SessionError::NO_CURRENT_PROCESS)
    );
    assert_eq!(
        session.request_line_discipline("", "single-pipe"),
        Err(SessionError::INVALID_ARGUMENT)
    );
    assert_eq!(
        session.request_line_discipline("argv-v1", ""),
        Err(SessionError::INVALID_ARGUMENT)
    );
}
