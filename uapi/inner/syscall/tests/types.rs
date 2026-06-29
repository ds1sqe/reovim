use {
    core::mem::{align_of, size_of},
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr, SyscallRet},
};

#[test]
fn syscall_numbers_are_stable_transport_scalars() {
    assert_eq!(SyscallNr::INVALID.raw(), 0);
    assert_eq!(SyscallNr::READ.raw(), 1);
    assert_eq!(SyscallNr::WRITE.raw(), 2);
    assert_eq!(SyscallNr::OPEN_AT.raw(), 3);
    assert_eq!(SyscallNr::YIELD_NOW.raw(), 10);
    assert_eq!(SyscallNr::SLEEP.raw(), 11);
    assert_eq!(SyscallNr::SPAWN.raw(), 12);
    assert_eq!(SyscallNr::EXECVE.raw(), 13);
    assert_eq!(SyscallNr::WAIT.raw(), 14);
    assert_eq!(SyscallNr::EXIT.raw(), 15);
    assert_eq!(SyscallNr::DUP.raw(), 16);
    assert_eq!(SyscallNr::SCHED_TICK.raw(), 17);
    assert_eq!(SyscallNr::DUP_TO.raw(), 18);
    assert_eq!(SyscallNr::PIPE.raw(), 19);
    assert_eq!(SyscallNr::TERMINAL_CLEAR.raw(), 20);
    assert_eq!(SyscallNr::SYSTEM_HALT.raw(), 21);
    assert_eq!(SyscallNr::PROVIDER_PROBE.raw(), 22);
    assert_eq!(SyscallNr::DUMP_SYNC.raw(), 23);
    assert_eq!(SyscallNr::PROCESS_WAKE.raw(), 24);
    assert_eq!(SyscallNr::PROCESS_KILL.raw(), 25);
    assert_eq!(SyscallNr::PROCESS_WAIT_TICKS.raw(), 26);
    assert_eq!(SyscallNr::SERVICE_CONTROL.raw(), 27);
    assert_eq!(SyscallNr::PROCESS_WAIT_READY.raw(), 28);
    assert_eq!(SyscallNr::SESSION_CONTROL.raw(), 29);
    assert_eq!(SyscallNr::SOURCE_CONTROL.raw(), 30);
    assert_eq!(SyscallNr::FD_FLAGS_GET.raw(), 31);
    assert_eq!(SyscallNr::FD_FLAGS_SET.raw(), 32);
    assert_eq!(SyscallNr::TERMINAL_RAW_ENTER.raw(), 33);
    assert_eq!(SyscallNr::TERMINAL_RAW_RESTORE.raw(), 34);
    assert_eq!(SyscallNr::TERMINAL_RAW_RESTORE_PRIMARY.raw(), 35);
    assert_eq!(SyscallNr::FD_STATUS_GET.raw(), 36);
    assert_eq!(SyscallNr::FD_STATUS_SET.raw(), 37);
    assert_eq!(SyscallNr::PROCESS_SELF.raw(), 38);
    assert!(SyscallNr::INVALID.is_invalid());
    assert!(!SyscallNr::READ.is_invalid());
}

#[test]
fn syscall_args_are_six_scalar_words() {
    assert_eq!(size_of::<SyscallArgs>(), size_of::<usize>() * SyscallArgs::COUNT);
    assert_eq!(align_of::<SyscallArgs>(), align_of::<usize>());

    let args = SyscallArgs::new([1, 2, 3, 4, 5, 6]);
    assert_eq!(args.get(0), Some(1));
    assert_eq!(args.get(5), Some(6));
    assert_eq!(args.get(6), None);
}

#[test]
fn syscall_ret_decodes_success_and_failure() {
    let ok = SyscallRet::success(42);
    assert!(!ok.is_failure());
    assert_eq!(ok.value(), 42);
    assert_eq!(ok.decode(), Ok(42));

    let err = SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
    assert!(err.is_failure());
    assert_eq!(err.decode(), Err(SyscallError::INVALID_ARGUMENT));
}

#[test]
fn syscall_errors_are_stable_transport_scalars() {
    assert_eq!(SyscallError::UNSUPPORTED.code(), 1);
    assert_eq!(SyscallError::INVALID_ARGUMENT.code(), 2);
    assert_eq!(SyscallError::BAD_DESCRIPTOR.code(), 3);
    assert_eq!(SyscallError::NOT_READABLE.code(), 4);
    assert_eq!(SyscallError::NOT_WRITABLE.code(), 5);
    assert_eq!(SyscallError::NO_CURRENT_PROCESS.code(), 6);
    assert_eq!(SyscallError::NOT_FOUND.code(), 7);
    assert_eq!(SyscallError::NOT_DIRECTORY.code(), 8);
    assert_eq!(SyscallError::BUSY.code(), 9);
    assert_eq!(SyscallError::FILE_TOO_LARGE.code(), 10);
    assert_eq!(SyscallError::IO.code(), 11);
    assert_eq!(SyscallError::NOT_SEEKABLE.code(), 12);
    assert_eq!(SyscallError::PROCESS_NOT_FOUND.code(), 13);
    assert_eq!(SyscallError::NOT_WAITABLE.code(), 14);
    assert_eq!(SyscallError::PROTECTED_PROCESS.code(), 15);
    assert_eq!(SyscallError::INVALID_IMAGE.code(), 16);
    assert_eq!(SyscallError::SOURCE_MEDIA_UNAVAILABLE.code(), 17);
    assert_eq!(SyscallError::SOURCE_MEDIA_READ_FAILED.code(), 18);
    assert_eq!(SyscallError::SOURCE_MEDIA_NAMESPACE_MISMATCH.code(), 19);
    assert_eq!(SyscallError::SOURCE_MEDIA_PATH_MISMATCH.code(), 20);
}

#[test]
fn raw_syscall_hook_carries_transport_only() {
    fn fixture(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::GET_PID);
        SyscallRet::success(args.get(0).unwrap_or_default() + 1)
    }

    let raw = RawSyscall::new(fixture);
    assert_eq!(
        raw.invoke(SyscallNr::GET_PID, SyscallArgs::new([40, 0, 0, 0, 0, 0]))
            .decode(),
        Ok(41),
    );
    assert_eq!(
        RawSyscall::unsupported()
            .invoke(SyscallNr::READ, SyscallArgs::EMPTY)
            .decode(),
        Err(SyscallError::UNSUPPORTED),
    );
}
