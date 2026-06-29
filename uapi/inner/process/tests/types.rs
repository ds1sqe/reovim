use {
    core::{
        mem::size_of,
        sync::atomic::{AtomicUsize, Ordering},
    },
    reovim_uapi_process::{
        ExitCode, ProcessArg, ProcessControlReport, ProcessEnv, ProcessError, ProcessId,
        ProcessSleepReport, ProcessSpawnFlags, ProcessSpawnMode, ProcessSpawnRequest,
        ProcessSpawnTarget, ProcessStateCode, ProcessTimedWaitReport, ProcessWaitReport,
        SyscallProcessControl,
    },
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr, SyscallRet},
};

static WAKE_RAW_CALLS: AtomicUsize = AtomicUsize::new(0);

#[test]
fn process_scalars_are_stable() {
    assert_eq!(ProcessId::new(7).raw(), 7);
    assert_eq!(ExitCode::SUCCESS.raw(), 0);
    assert_eq!(ExitCode::FAILURE.raw(), 1);
    assert_eq!(ExitCode::new(42).raw(), 42);
    assert_eq!(ProcessError::new(6).code(), 6);
    assert_eq!(ProcessError::BUSY.code(), SyscallError::BUSY.code());
    assert_eq!(ProcessError::NOT_WAITABLE.code(), SyscallError::NOT_WAITABLE.code());
    assert_eq!(ProcessError::PROTECTED_PROCESS.code(), SyscallError::PROTECTED_PROCESS.code());
    assert_eq!(ProcessSpawnTarget::BIN.raw(), 0);
    assert_eq!(ProcessSpawnTarget::PAYLOAD.raw(), 1);
    assert!(ProcessSpawnTarget::BIN.with_env().carries_env());
    assert_eq!(ProcessSpawnTarget::BIN.with_env().base(), ProcessSpawnTarget::BIN);
    assert!(ProcessSpawnTarget::BIN.with_request().carries_request());
    assert_eq!(ProcessSpawnTarget::BIN.with_request().base(), ProcessSpawnTarget::BIN,);
    assert!(ProcessSpawnFlags::empty().is_known());
    assert!(!ProcessSpawnFlags::from_raw(1).is_known());
    assert_eq!(ProcessSpawnMode::READY.raw(), 0);
    assert_eq!(ProcessSpawnMode::BLOCKED.raw(), 1);
    assert_eq!(ProcessSpawnMode::SLEEPING.raw(), 2);
    assert_eq!(ProcessStateCode::EMPTY.raw(), 0);
    assert_eq!(ProcessStateCode::NEW.raw(), 1);
    assert_eq!(ProcessStateCode::READY.raw(), 2);
    assert_eq!(ProcessStateCode::RUNNING.raw(), 3);
    assert_eq!(ProcessStateCode::BLOCKED.raw(), 4);
    assert_eq!(ProcessStateCode::EXITED.raw(), 5);
    assert_eq!(ProcessStateCode::FAILED.raw(), 6);
    assert_eq!(ProcessStateCode::HALTED.raw(), 7);
    assert_eq!(ProcessStateCode::REAPED.raw(), 8);
    let mut control_report = ProcessControlReport::empty();
    control_report.set_pid(ProcessId::new(10));
    control_report.set_state(ProcessStateCode::READY);
    control_report.set_path("/bin/pwd");
    control_report.set_loader("linked-bin");
    control_report.set_entry_name("bin_pwd");
    control_report.set_body_format("linked-image");
    control_report.set_body_inner("none");
    control_report.set_body_bytes(1234);
    control_report.set_body_checksum(5678);
    assert_eq!(control_report.pid(), ProcessId::new(10));
    assert_eq!(control_report.state(), ProcessStateCode::READY);
    assert_eq!(control_report.path_bytes(), b"/bin/pwd");
    assert!(!control_report.path_truncated());
    assert_eq!(control_report.loader_bytes(), b"linked-bin");
    assert!(!control_report.loader_truncated());
    assert_eq!(control_report.entry_name_bytes(), b"bin_pwd");
    assert!(!control_report.entry_name_truncated());
    assert_eq!(control_report.body_format_bytes(), b"linked-image");
    assert!(!control_report.body_format_truncated());
    assert_eq!(control_report.body_inner_bytes(), b"none");
    assert!(!control_report.body_inner_truncated());
    assert_eq!(control_report.body_bytes(), 1234);
    assert_eq!(control_report.body_checksum(), 5678);
    control_report.clear();
    assert_eq!(control_report, ProcessControlReport::empty());
    let mut sleep_report = ProcessSleepReport::empty();
    sleep_report.set_requested_ticks(3);
    sleep_report.set_pid(ProcessId::new(11));
    sleep_report.set_state(ProcessStateCode::BLOCKED);
    sleep_report.set_wake_tick(14);
    sleep_report.set_path("/bin/pwd");
    assert_eq!(sleep_report.requested_ticks(), 3);
    assert_eq!(sleep_report.pid(), ProcessId::new(11));
    assert_eq!(sleep_report.state(), ProcessStateCode::BLOCKED);
    assert_eq!(sleep_report.wake_tick(), 14);
    assert_eq!(sleep_report.path_bytes(), b"/bin/pwd");
    assert!(!sleep_report.path_truncated());
    sleep_report.clear();
    assert_eq!(sleep_report, ProcessSleepReport::empty());
    let mut wait_report = ProcessWaitReport::empty();
    wait_report.set_child_pid(ProcessId::new(12));
    wait_report.set_child_state(ProcessStateCode::EXITED);
    wait_report.set_exit_code(7);
    wait_report.set_completed(true);
    assert_eq!(wait_report.child_pid(), ProcessId::new(12));
    assert_eq!(wait_report.child_state(), ProcessStateCode::EXITED);
    assert_eq!(wait_report.exit_code(), 7);
    assert!(wait_report.completed());
    wait_report.clear();
    assert_eq!(wait_report, ProcessWaitReport::empty());
    let mut timed_wait = ProcessTimedWaitReport::empty();
    timed_wait.set_requested_ticks(5);
    timed_wait.set_child_pid(ProcessId::new(12));
    timed_wait.set_child_state(ProcessStateCode::EXITED);
    timed_wait.set_exit_code(7);
    timed_wait.set_completed(true);
    timed_wait.set_timed_out(false);
    timed_wait.set_tick_count(9);
    assert_eq!(timed_wait.requested_ticks(), 5);
    assert_eq!(timed_wait.child_pid(), ProcessId::new(12));
    assert_eq!(timed_wait.child_state(), ProcessStateCode::EXITED);
    assert_eq!(timed_wait.exit_code(), 7);
    assert!(timed_wait.completed());
    assert!(!timed_wait.timed_out());
    assert_eq!(timed_wait.tick_count(), 9);
    timed_wait.clear();
    assert_eq!(timed_wait, ProcessTimedWaitReport::empty());
    let text = "/bin/pwd";
    let arg = ProcessArg::from_str(text);
    assert_eq!(arg.len(), 8);
    assert!(!arg.is_empty());
    assert_eq!(arg.ptr(), text.as_ptr());
}

#[test]
#[allow(unsafe_code)]
fn process_env_lowers_through_domain_spawn_and_execve() {
    fn spawn_syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SPAWN);
        assert_ne!(args.get(0), Some(0));
        assert_eq!(args.get(1), Some(2));
        assert_eq!(args.get(2), Some(ProcessSpawnTarget::BIN.with_env().raw()));
        assert_eq!(args.get(3), Some(ProcessSpawnMode::READY.raw()));
        assert_ne!(args.get(4), Some(0));
        assert_eq!(args.get(5), Some(1));
        SyscallRet::success(17)
    }

    fn blocked_spawn_syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SPAWN);
        assert_ne!(args.get(0), Some(0));
        assert_eq!(args.get(1), Some(2));
        assert_eq!(args.get(2), Some(ProcessSpawnTarget::BIN.with_env().raw()));
        assert_eq!(args.get(3), Some(ProcessSpawnMode::BLOCKED.raw()));
        assert_ne!(args.get(4), Some(0));
        assert_eq!(args.get(5), Some(1));
        SyscallRet::success(18)
    }

    fn execve_syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::EXECVE);
        assert_ne!(args.get(0), Some(0));
        assert_eq!(args.get(1), Some(2));
        assert_ne!(args.get(2), Some(0));
        assert_eq!(args.get(3), Some(1));
        assert_eq!(args.get(4), Some(0));
        assert_eq!(args.get(5), Some(0));
        SyscallRet::success(0)
    }

    fn report_request_syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SPAWN);
        assert_ne!(args.get(0), Some(0));
        assert_eq!(args.get(1), Some(size_of::<ProcessSpawnRequest>()));
        assert_eq!(args.get(2), Some(ProcessSpawnTarget::BIN.with_request().raw()));
        assert_eq!(args.get(3), Some(0));
        assert_eq!(args.get(4), Some(0));
        assert_eq!(args.get(5), Some(0));
        // SAFETY: wrapper passes a live request for this synchronous fixture backend.
        let request = unsafe { &*(args.get(0).unwrap() as *const ProcessSpawnRequest) };
        assert_ne!(request.argv_ptr() as usize, 0);
        assert_eq!(request.argc(), 2);
        assert_ne!(request.env_ptr() as usize, 0);
        assert_eq!(request.envc(), 1);
        assert_eq!(request.target(), ProcessSpawnTarget::BIN);
        assert_eq!(request.mode(), ProcessSpawnMode::READY);
        assert_eq!(request.flags(), ProcessSpawnFlags::empty());
        assert_ne!(request.report_ptr(), 0);
        assert_eq!(request.report_len(), size_of::<ProcessControlReport>());
        // SAFETY: request carries a live report for this synchronous fixture backend.
        let report = unsafe { &mut *(request.report_ptr() as *mut ProcessControlReport) };
        report.set_pid(ProcessId::new(19));
        report.set_state(ProcessStateCode::READY);
        report.set_path("/bin/media-bin");
        SyscallRet::success(19)
    }

    fn blocked_report_request_syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SPAWN);
        assert_eq!(args.get(1), Some(size_of::<ProcessSpawnRequest>()));
        assert_eq!(args.get(2), Some(ProcessSpawnTarget::BIN.with_request().raw()));
        // SAFETY: wrapper passes a live request for this synchronous fixture backend.
        let request = unsafe { &*(args.get(0).unwrap() as *const ProcessSpawnRequest) };
        assert_eq!(request.mode(), ProcessSpawnMode::BLOCKED);
        assert_eq!(request.flags(), ProcessSpawnFlags::empty());
        assert_eq!(request.envc(), 1);
        assert_eq!(request.report_len(), size_of::<ProcessControlReport>());
        // SAFETY: request carries a live report for this synchronous fixture backend.
        let report = unsafe { &mut *(request.report_ptr() as *mut ProcessControlReport) };
        report.set_pid(ProcessId::new(20));
        report.set_state(ProcessStateCode::BLOCKED);
        report.set_path("/bin/media-bin");
        SyscallRet::success(20)
    }

    fn sleep_request_syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SPAWN);
        assert_eq!(args.get(1), Some(size_of::<ProcessSpawnRequest>()));
        assert_eq!(args.get(2), Some(ProcessSpawnTarget::BIN.with_request().raw()));
        // SAFETY: wrapper passes a live request for this synchronous fixture backend.
        let request = unsafe { &*(args.get(0).unwrap() as *const ProcessSpawnRequest) };
        assert_eq!(request.mode(), ProcessSpawnMode::SLEEPING);
        assert_eq!(request.flags(), ProcessSpawnFlags::empty());
        assert_eq!(request.envc(), 1);
        assert_eq!(request.report_len(), size_of::<ProcessSleepReport>());
        // SAFETY: request carries a live report for this synchronous fixture backend.
        let report = unsafe { &mut *(request.report_ptr() as *mut ProcessSleepReport) };
        assert_eq!(report.requested_ticks(), 5);
        report.set_pid(ProcessId::new(21));
        report.set_state(ProcessStateCode::BLOCKED);
        report.set_wake_tick(30);
        report.set_path("/bin/media-bin");
        SyscallRet::success(21)
    }

    fn payload_request_syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SPAWN);
        assert_ne!(args.get(0), Some(0));
        assert_eq!(args.get(1), Some(size_of::<ProcessSpawnRequest>()));
        assert_eq!(args.get(2), Some(ProcessSpawnTarget::PAYLOAD.with_request().raw()));
        assert_eq!(args.get(3), Some(0));
        assert_eq!(args.get(4), Some(0));
        assert_eq!(args.get(5), Some(0));
        // SAFETY: wrapper passes a live request for this synchronous fixture backend.
        let request = unsafe { &*(args.get(0).unwrap() as *const ProcessSpawnRequest) };
        assert_ne!(request.argv_ptr() as usize, 0);
        assert_eq!(request.argc(), 2);
        assert_ne!(request.env_ptr() as usize, 0);
        assert_eq!(request.envc(), 1);
        assert_eq!(request.target(), ProcessSpawnTarget::PAYLOAD);
        assert_eq!(request.mode(), ProcessSpawnMode::READY);
        assert_eq!(request.flags(), ProcessSpawnFlags::empty());
        assert_eq!(request.report_ptr(), 0);
        assert_eq!(request.report_len(), 0);
        SyscallRet::success(22)
    }

    let argv = [
        ProcessArg::from_str("media-bin"),
        ProcessArg::from_str("/boot/status"),
    ];
    let env = [ProcessEnv::from_pair("REOVIM_MEDIA_PATH", "/boot/status")];

    let process = SyscallProcessControl::new(RawSyscall::new(spawn_syscall));
    assert_eq!(process.spawn_with_env(&argv, &env).map(ProcessId::raw), Ok(17));

    let process = SyscallProcessControl::new(RawSyscall::new(blocked_spawn_syscall));
    assert_eq!(
        process
            .spawn_blocked_with_env(&argv, &env)
            .map(ProcessId::raw),
        Ok(18),
    );

    let process = SyscallProcessControl::new(RawSyscall::new(execve_syscall));
    assert_eq!(process.execve_with_env(&argv, &env), Ok(ExitCode::SUCCESS));

    let process = SyscallProcessControl::new(RawSyscall::new(report_request_syscall));
    let report = process
        .spawn_report_with_env(&argv, &env)
        .expect("env report spawn succeeds");
    assert_eq!(report.pid(), ProcessId::new(19));
    assert_eq!(report.state(), ProcessStateCode::READY);
    assert_eq!(report.path_bytes(), b"/bin/media-bin");

    let process = SyscallProcessControl::new(RawSyscall::new(blocked_report_request_syscall));
    let blocked_report = process
        .spawn_blocked_report_with_env(&argv, &env)
        .expect("env blocked report spawn succeeds");
    assert_eq!(blocked_report.pid(), ProcessId::new(20));
    assert_eq!(blocked_report.state(), ProcessStateCode::BLOCKED);
    assert_eq!(blocked_report.path_bytes(), b"/bin/media-bin");

    let process = SyscallProcessControl::new(RawSyscall::new(sleep_request_syscall));
    let sleep = process
        .spawn_sleeping_with_env(&argv, &env, 5)
        .expect("env sleeping spawn succeeds");
    assert_eq!(sleep.requested_ticks(), 5);
    assert_eq!(sleep.pid(), ProcessId::new(21));
    assert_eq!(sleep.state(), ProcessStateCode::BLOCKED);
    assert_eq!(sleep.wake_tick(), 30);
    assert_eq!(sleep.path_bytes(), b"/bin/media-bin");

    let process = SyscallProcessControl::new(RawSyscall::new(payload_request_syscall));
    assert_eq!(
        process
            .spawn_payload_with_env(&argv, &env)
            .map(ProcessId::raw),
        Ok(22),
    );
}

#[test]
fn wake_lowers_to_exactly_one_raw_syscall() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::PROCESS_WAKE);
        assert_eq!(args.get(0), Some(10));
        assert_eq!(args.get(1), Some(0));
        assert_eq!(args.get(2), Some(0));
        assert_eq!(args.get(3), Some(0));
        assert_eq!(args.get(4), Some(0));
        assert_eq!(args.get(5), Some(0));
        WAKE_RAW_CALLS.fetch_add(1, Ordering::SeqCst);
        SyscallRet::success(10)
    }

    WAKE_RAW_CALLS.store(0, Ordering::SeqCst);
    let process = SyscallProcessControl::new(RawSyscall::new(syscall));

    assert_eq!(process.wake(ProcessId::new(10)), Ok(ProcessId::new(10)));
    assert_eq!(WAKE_RAW_CALLS.load(Ordering::SeqCst), 1);
}

#[test]
#[allow(unsafe_code)]
fn syscall_process_control_lowers_identity_spawn_execve_wait_kill_and_exit_through_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        match nr {
            SyscallNr::GET_PID => SyscallRet::success(42),
            SyscallNr::PROCESS_SELF => {
                assert_ne!(args.get(0), Some(0));
                assert_eq!(args.get(1), Some(size_of::<ProcessControlReport>()));
                assert_eq!(args.get(2), Some(0));
                assert_eq!(args.get(3), Some(0));
                assert_eq!(args.get(4), Some(0));
                assert_eq!(args.get(5), Some(0));
                // SAFETY: the wrapper passes a live report for this
                // synchronous fixture backend.
                let report = unsafe { &mut *(args.get(0).unwrap() as *mut ProcessControlReport) };
                report.set_pid(ProcessId::new(42));
                report.set_state(ProcessStateCode::RUNNING);
                report.set_path("/bin/self-test");
                report.set_loader("linked-bin");
                report.set_entry_name("bin_self_test");
                report.set_body_format("linked-image");
                report.set_body_inner("none");
                SyscallRet::success(42)
            }
            SyscallNr::SPAWN => {
                assert_ne!(args.get(0), Some(0));
                assert_eq!(args.get(1), Some(2));
                match (args.get(2), args.get(3)) {
                    (Some(0), Some(0)) => {
                        if args.get(4) == Some(0) {
                            assert_eq!(args.get(5), Some(0));
                        } else {
                            assert_eq!(args.get(5), Some(size_of::<ProcessControlReport>()));
                            // SAFETY: the wrapper passes a live report for this
                            // synchronous fixture backend.
                            let report = unsafe {
                                &mut *(args.get(4).unwrap() as *mut ProcessControlReport)
                            };
                            report.set_pid(ProcessId::new(7));
                            report.set_state(ProcessStateCode::READY);
                            report.set_path("/bin/pwd");
                            report.set_loader("linked-bin");
                            report.set_entry_name("bin_pwd");
                            report.set_body_format("linked-image");
                            report.set_body_inner("none");
                        }
                        SyscallRet::success(7)
                    }
                    (Some(0), Some(1)) => {
                        if args.get(4) == Some(0) {
                            assert_eq!(args.get(5), Some(0));
                        } else {
                            assert_eq!(args.get(5), Some(size_of::<ProcessControlReport>()));
                            // SAFETY: the wrapper passes a live report for this
                            // synchronous fixture backend.
                            let report = unsafe {
                                &mut *(args.get(4).unwrap() as *mut ProcessControlReport)
                            };
                            report.set_pid(ProcessId::new(9));
                            report.set_state(ProcessStateCode::BLOCKED);
                            report.set_path("/bin/pwd");
                            report.set_loader("linked-bin");
                            report.set_entry_name("bin_pwd");
                            report.set_body_format("linked-image");
                            report.set_body_inner("none");
                        }
                        SyscallRet::success(9)
                    }
                    (Some(0), Some(2)) => {
                        assert_ne!(args.get(4), Some(0));
                        assert_eq!(args.get(5), Some(size_of::<ProcessSleepReport>()));
                        // SAFETY: the wrapper passes a live report for this
                        // synchronous fixture backend.
                        let report =
                            unsafe { &mut *(args.get(4).unwrap() as *mut ProcessSleepReport) };
                        assert_eq!(report.requested_ticks(), 3);
                        report.set_pid(ProcessId::new(10));
                        report.set_state(ProcessStateCode::BLOCKED);
                        report.set_wake_tick(13);
                        report.set_path("/bin/pwd");
                        SyscallRet::success(10)
                    }
                    (Some(1), Some(0)) => {
                        assert_eq!(args.get(4), Some(0));
                        assert_eq!(args.get(5), Some(0));
                        SyscallRet::success(8)
                    }
                    _ => SyscallRet::failure(SyscallError::INVALID_ARGUMENT),
                }
            }
            SyscallNr::EXECVE => {
                assert_ne!(args.get(0), Some(0));
                assert_eq!(args.get(1), Some(2));
                SyscallRet::success(0)
            }
            SyscallNr::WAIT => {
                assert_eq!(args.get(0), Some(7));
                if args.get(1) == Some(0) {
                    assert_eq!(args.get(2), Some(0));
                } else {
                    assert_eq!(args.get(2), Some(size_of::<ProcessWaitReport>()));
                    // SAFETY: the wrapper passes a live report for this
                    // synchronous fixture backend.
                    let report = unsafe { &mut *(args.get(1).unwrap() as *mut ProcessWaitReport) };
                    report.set_child_pid(ProcessId::new(7));
                    report.set_child_state(ProcessStateCode::EXITED);
                    report.set_exit_code(0);
                    report.set_completed(true);
                }
                assert_eq!(args.get(3), Some(0));
                assert_eq!(args.get(4), Some(0));
                assert_eq!(args.get(5), Some(0));
                SyscallRet::success(0)
            }
            SyscallNr::PROCESS_WAIT_TICKS => {
                assert_eq!(args.get(0), Some(10));
                assert_eq!(args.get(1), Some(3));
                assert_ne!(args.get(2), Some(0));
                assert_eq!(args.get(3), Some(size_of::<ProcessTimedWaitReport>()));
                assert_eq!(args.get(4), Some(0));
                assert_eq!(args.get(5), Some(0));
                // SAFETY: the wrapper passes a live report for this synchronous
                // fixture backend.
                let report = unsafe { &mut *(args.get(2).unwrap() as *mut ProcessTimedWaitReport) };
                assert_eq!(report.child_pid(), ProcessId::new(10));
                assert_eq!(report.requested_ticks(), 3);
                report.set_child_state(ProcessStateCode::EXITED);
                report.set_exit_code(0);
                report.set_completed(true);
                report.set_timed_out(false);
                report.set_tick_count(3);
                SyscallRet::success(10)
            }
            SyscallNr::PROCESS_WAIT_READY => {
                assert_eq!(args.get(0), Some(8));
                assert_ne!(args.get(1), Some(0));
                assert_eq!(args.get(2), Some(size_of::<ProcessWaitReport>()));
                assert_eq!(args.get(3), Some(0));
                assert_eq!(args.get(4), Some(0));
                assert_eq!(args.get(5), Some(0));
                // SAFETY: the wrapper passes a live report for this synchronous
                // fixture backend.
                let report = unsafe { &mut *(args.get(1).unwrap() as *mut ProcessWaitReport) };
                assert_eq!(report.child_pid(), ProcessId::new(8));
                report.set_child_state(ProcessStateCode::BLOCKED);
                report.set_exit_code(0);
                report.set_completed(false);
                SyscallRet::success(0)
            }
            SyscallNr::PROCESS_WAKE => {
                assert_eq!(args.get(0), Some(10));
                if args.get(1) == Some(0) {
                    assert_eq!(args.get(2), Some(0));
                } else {
                    assert_eq!(args.get(2), Some(size_of::<ProcessControlReport>()));
                    // SAFETY: the wrapper passes a live report for this
                    // synchronous fixture backend.
                    let report =
                        unsafe { &mut *(args.get(1).unwrap() as *mut ProcessControlReport) };
                    report.set_pid(ProcessId::new(10));
                    report.set_state(ProcessStateCode::READY);
                    report.set_path("/bin/pwd");
                    report.set_loader("linked-bin");
                    report.set_entry_name("bin_pwd");
                    report.set_body_format("linked-image");
                    report.set_body_inner("none");
                }
                assert_eq!(args.get(3), Some(0));
                assert_eq!(args.get(4), Some(0));
                assert_eq!(args.get(5), Some(0));
                SyscallRet::success(10)
            }
            SyscallNr::PROCESS_KILL => {
                assert_eq!(args.get(0), Some(11));
                if args.get(1) == Some(0) {
                    assert_eq!(args.get(2), Some(0));
                } else {
                    assert_eq!(args.get(2), Some(size_of::<ProcessControlReport>()));
                    // SAFETY: the wrapper passes a live report for this
                    // synchronous fixture backend.
                    let report =
                        unsafe { &mut *(args.get(1).unwrap() as *mut ProcessControlReport) };
                    report.set_pid(ProcessId::new(11));
                    report.set_state(ProcessStateCode::FAILED);
                    report.set_path("/bin/pwd");
                    report.set_loader("linked-bin");
                    report.set_entry_name("bin_pwd");
                    report.set_body_format("linked-image");
                    report.set_body_inner("none");
                }
                assert_eq!(args.get(3), Some(0));
                assert_eq!(args.get(4), Some(0));
                assert_eq!(args.get(5), Some(0));
                SyscallRet::success(11)
            }
            SyscallNr::EXIT => {
                assert_eq!(args.get(0), Some(ExitCode::SUCCESS.raw() as usize));
                SyscallRet::success(0)
            }
            _ => SyscallRet::failure(SyscallError::INVALID_ARGUMENT),
        }
    }

    let process = SyscallProcessControl::new(RawSyscall::new(syscall));
    let argv = [
        ProcessArg::from_str("pwd"),
        ProcessArg::from_str("--logical"),
    ];

    assert_eq!(process.get_pid().map(ProcessId::raw), Ok(42));
    let self_report = process.self_report().expect("self report succeeds");
    assert_eq!(self_report.pid(), ProcessId::new(42));
    assert_eq!(self_report.state(), ProcessStateCode::RUNNING);
    assert_eq!(self_report.path_bytes(), b"/bin/self-test");
    assert_eq!(self_report.loader_bytes(), b"linked-bin");
    assert_eq!(self_report.entry_name_bytes(), b"bin_self_test");
    assert_eq!(self_report.body_format_bytes(), b"linked-image");
    assert_eq!(self_report.body_inner_bytes(), b"none");
    assert_eq!(self_report.body_bytes(), 0);
    assert_eq!(self_report.body_checksum(), 0);
    assert_eq!(process.spawn(&argv).map(ProcessId::raw), Ok(7));
    let spawn_report = process.spawn_report(&argv).expect("spawn report succeeds");
    assert_eq!(spawn_report.pid(), ProcessId::new(7));
    assert_eq!(spawn_report.state(), ProcessStateCode::READY);
    assert_eq!(spawn_report.path_bytes(), b"/bin/pwd");
    assert_eq!(spawn_report.loader_bytes(), b"linked-bin");
    assert_eq!(spawn_report.entry_name_bytes(), b"bin_pwd");
    assert_eq!(spawn_report.body_format_bytes(), b"linked-image");
    assert_eq!(spawn_report.body_inner_bytes(), b"none");
    assert_eq!(spawn_report.body_bytes(), 0);
    assert_eq!(spawn_report.body_checksum(), 0);
    assert_eq!(process.spawn_payload(&argv).map(ProcessId::raw), Ok(8));
    assert_eq!(process.spawn_blocked(&argv).map(ProcessId::raw), Ok(9));
    let blocked_report = process
        .spawn_blocked_report(&argv)
        .expect("blocked spawn report succeeds");
    assert_eq!(blocked_report.pid(), ProcessId::new(9));
    assert_eq!(blocked_report.state(), ProcessStateCode::BLOCKED);
    assert_eq!(blocked_report.path_bytes(), b"/bin/pwd");
    assert_eq!(blocked_report.loader_bytes(), b"linked-bin");
    assert_eq!(blocked_report.entry_name_bytes(), b"bin_pwd");
    assert_eq!(blocked_report.body_format_bytes(), b"linked-image");
    assert_eq!(blocked_report.body_inner_bytes(), b"none");
    assert_eq!(blocked_report.body_bytes(), 0);
    assert_eq!(blocked_report.body_checksum(), 0);
    let sleep = process
        .spawn_sleeping(&argv, 3)
        .expect("sleeping spawn succeeds");
    assert_eq!(sleep.requested_ticks(), 3);
    assert_eq!(sleep.pid(), ProcessId::new(10));
    assert_eq!(sleep.state(), ProcessStateCode::BLOCKED);
    assert_eq!(sleep.wake_tick(), 13);
    assert_eq!(sleep.path_bytes(), b"/bin/pwd");
    assert_eq!(process.execve(&argv), Ok(ExitCode::SUCCESS));
    assert_eq!(process.wait(ProcessId::new(7)), Ok(ExitCode::SUCCESS));
    let wait_report = process
        .wait_report(ProcessId::new(7))
        .expect("wait report succeeds");
    assert_eq!(wait_report.child_pid(), ProcessId::new(7));
    assert_eq!(wait_report.child_state(), ProcessStateCode::EXITED);
    assert_eq!(wait_report.exit_code(), 0);
    assert!(wait_report.completed());
    let timed_wait = process
        .wait_for_ticks(ProcessId::new(10), 3)
        .expect("timed wait succeeds");
    assert_eq!(timed_wait.child_pid(), ProcessId::new(10));
    assert_eq!(timed_wait.child_state(), ProcessStateCode::EXITED);
    assert_eq!(timed_wait.exit_code(), 0);
    assert!(timed_wait.completed());
    assert!(!timed_wait.timed_out());
    assert_eq!(timed_wait.tick_count(), 3);
    let ready_wait = process
        .wait_ready_report(ProcessId::new(8))
        .expect("ready wait succeeds");
    assert_eq!(ready_wait.child_pid(), ProcessId::new(8));
    assert_eq!(ready_wait.child_state(), ProcessStateCode::BLOCKED);
    assert_eq!(ready_wait.exit_code(), 0);
    assert!(!ready_wait.completed());
    assert_eq!(process.wake(ProcessId::new(10)).map(ProcessId::raw), Ok(10));
    let wake_report = process
        .wake_report(ProcessId::new(10))
        .expect("wake report succeeds");
    assert_eq!(wake_report.pid(), ProcessId::new(10));
    assert_eq!(wake_report.state(), ProcessStateCode::READY);
    assert_eq!(wake_report.path_bytes(), b"/bin/pwd");
    assert_eq!(wake_report.loader_bytes(), b"linked-bin");
    assert_eq!(wake_report.entry_name_bytes(), b"bin_pwd");
    assert_eq!(wake_report.body_format_bytes(), b"linked-image");
    assert_eq!(wake_report.body_inner_bytes(), b"none");
    assert_eq!(wake_report.body_bytes(), 0);
    assert_eq!(wake_report.body_checksum(), 0);
    assert_eq!(process.kill(ProcessId::new(11)).map(ProcessId::raw), Ok(11));
    let kill_report = process
        .kill_report(ProcessId::new(11))
        .expect("kill report succeeds");
    assert_eq!(kill_report.pid(), ProcessId::new(11));
    assert_eq!(kill_report.state(), ProcessStateCode::FAILED);
    assert_eq!(kill_report.path_bytes(), b"/bin/pwd");
    assert_eq!(kill_report.loader_bytes(), b"linked-bin");
    assert_eq!(kill_report.entry_name_bytes(), b"bin_pwd");
    assert_eq!(kill_report.body_format_bytes(), b"linked-image");
    assert_eq!(kill_report.body_inner_bytes(), b"none");
    assert_eq!(kill_report.body_bytes(), 0);
    assert_eq!(kill_report.body_checksum(), 0);
    assert_eq!(process.exit(ExitCode::SUCCESS), Ok(()));
    assert_eq!(process.spawn_sleeping(&argv, 0), Err(ProcessError::INVALID_ARGUMENT));
    assert_eq!(
        process.wait_for_ticks(ProcessId::new(10), 0),
        Err(ProcessError::INVALID_ARGUMENT),
    );
}

#[test]
fn execve_reports_admission_success() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::EXECVE);
        assert_ne!(args.get(0), Some(0));
        assert_eq!(args.get(1), Some(1));
        assert_eq!(args.get(2), Some(0));
        assert_eq!(args.get(3), Some(0));
        assert_eq!(args.get(4), Some(0));
        assert_eq!(args.get(5), Some(0));
        SyscallRet::success(0)
    }

    let process = SyscallProcessControl::new(RawSyscall::new(syscall));

    assert_eq!(process.execve(&[ProcessArg::from_str("source-exit")]), Ok(ExitCode::SUCCESS));
}

#[test]
fn execve_rejects_invalid_success_scalar() {
    fn syscall(nr: SyscallNr, _: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::EXECVE);
        SyscallRet::success(usize::from(u8::MAX) + 1)
    }

    let process = SyscallProcessControl::new(RawSyscall::new(syscall));

    assert_eq!(
        process.execve(&[ProcessArg::from_str("bad-exit")]),
        Err(ProcessError::INVALID_ARGUMENT),
    );
}

#[test]
fn syscall_process_control_maps_transport_errors() {
    fn syscall(_: SyscallNr, _: SyscallArgs) -> SyscallRet {
        SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS)
    }

    let process = SyscallProcessControl::new(RawSyscall::new(syscall));

    assert_eq!(
        process.get_pid(),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.self_report(),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.exit(ExitCode::FAILURE),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.spawn(&[ProcessArg::from_str("pwd")]),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.spawn_report(&[ProcessArg::from_str("pwd")]),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.spawn_payload(&[ProcessArg::from_str("reovim")]),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.spawn_blocked(&[ProcessArg::from_str("pwd")]),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.spawn_blocked_report(&[ProcessArg::from_str("pwd")]),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.spawn_sleeping(&[ProcessArg::from_str("pwd")], 1),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.execve(&[ProcessArg::from_str("pwd")]),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.wait(ProcessId::new(9)),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.wait_for_ticks(ProcessId::new(9), 1),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.wake(ProcessId::new(9)),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.wake_report(ProcessId::new(9)),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.kill(ProcessId::new(9)),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
    assert_eq!(
        process.kill_report(ProcessId::new(9)),
        Err(ProcessError::new(SyscallError::NO_CURRENT_PROCESS.code())),
    );
}
