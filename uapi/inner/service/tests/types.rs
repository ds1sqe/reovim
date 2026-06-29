use {
    core::mem::{align_of, size_of},
    reovim_uapi_service::{
        ServiceControlOp, ServiceControlReport, ServiceControlResultCode, ServiceError,
        ServiceReasonCode, ServiceStateCode, SyscallServiceControl,
    },
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr, SyscallRet},
};

#[test]
fn service_scalars_and_report_are_stable() {
    assert_eq!(ServiceControlOp::STOP.raw(), 0);
    assert_eq!(ServiceControlOp::START.raw(), 1);
    assert_eq!(ServiceControlOp::RESTART.raw(), 2);
    assert_eq!(ServiceControlOp::REQUEST.raw(), 3);
    assert_eq!(ServiceControlOp::new(0), ServiceControlOp::STOP);
    assert_eq!(ServiceControlOp::new(1), ServiceControlOp::START);
    assert_eq!(ServiceControlOp::new(2), ServiceControlOp::RESTART);
    assert_eq!(ServiceControlOp::new(3), ServiceControlOp::REQUEST);
    assert_eq!(ServiceStateCode::EMPTY.raw(), 0);
    assert_eq!(ServiceStateCode::REQUESTED.raw(), 1);
    assert_eq!(ServiceStateCode::STARTED.raw(), 2);
    assert_eq!(ServiceStateCode::EXITED.raw(), 3);
    assert_eq!(ServiceStateCode::STOPPED.raw(), 4);
    assert_eq!(ServiceStateCode::FAILED.raw(), 5);
    assert_eq!(ServiceReasonCode::NONE.raw(), 0);
    assert_eq!(ServiceReasonCode::REQUESTED.raw(), 1);
    assert_eq!(ServiceReasonCode::RUNNING.raw(), 2);
    assert_eq!(ServiceReasonCode::PROCESS_EXITED.raw(), 3);
    assert_eq!(ServiceReasonCode::PROCESS_FAILED.raw(), 4);
    assert_eq!(ServiceReasonCode::PROCESS_KILLED.raw(), 5);
    assert_eq!(ServiceReasonCode::OPERATOR_STOP.raw(), 6);
    assert_eq!(ServiceReasonCode::EXEC_LOAD_ERROR.raw(), 7);
    assert_eq!(ServiceReasonCode::HALT.raw(), 8);
    assert_eq!(ServiceReasonCode::START_ERROR.raw(), 9);
    assert_eq!(ServiceControlResultCode::NONE.raw(), 0);
    assert_eq!(ServiceControlResultCode::PAYLOAD_READY.raw(), 1);
    assert_eq!(ServiceControlResultCode::PAYLOAD_RESIDENT.raw(), 2);
    assert_eq!(ServiceControlResultCode::PAYLOAD_NOT_CONFIGURED.raw(), 3);
    assert_eq!(ServiceControlResultCode::PAYLOAD_FAILED.raw(), 4);
    assert_eq!(ServiceControlResultCode::PAYLOAD_EXIT_CODE.raw(), 5);
    assert_eq!(ServiceError::NOT_FOUND.code(), SyscallError::NOT_FOUND.code());
    assert_eq!(ServiceError::PROTECTED_SERVICE.code(), SyscallError::PROTECTED_PROCESS.code());

    assert_eq!(align_of::<ServiceControlReport>(), align_of::<usize>());
    assert!(size_of::<ServiceControlReport>() >= 120);

    let mut report = ServiceControlReport::empty();
    report.set_name("editor");
    report.set_target("/payload/editor-smoke");
    report.set_service_pid(4);
    report.set_service_task_id(5);
    report.set_state(ServiceStateCode::STOPPED);
    report.set_reason(ServiceReasonCode::OPERATOR_STOP);
    report.set_result(ServiceControlResultCode::PAYLOAD_RESIDENT);
    report.set_exit_code(7);
    assert_eq!(report.name_bytes(), b"editor");
    assert_eq!(report.target_bytes(), b"/payload/editor-smoke");
    assert_eq!(report.service_pid(), 4);
    assert_eq!(report.service_task_id(), 5);
    assert_eq!(report.state(), ServiceStateCode::STOPPED);
    assert_eq!(report.reason(), ServiceReasonCode::OPERATOR_STOP);
    assert_eq!(report.result(), ServiceControlResultCode::PAYLOAD_RESIDENT);
    assert_eq!(report.exit_code(), 7);
    assert!(!report.name_truncated());
    assert!(!report.target_truncated());
    report.clear();
    assert_eq!(report, ServiceControlReport::empty());
}

#[test]
fn syscall_service_control_lowers_request_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SERVICE_CONTROL);
        assert_eq!(args.a0, ServiceControlOp::REQUEST.raw());
        let service_name = "shell";
        let target = "/bin/sh";
        assert_eq!(args.a1, service_name.as_ptr().addr());
        assert_eq!(args.a2, service_name.len());
        assert_eq!(args.a3, target.as_ptr().addr());
        assert_eq!(args.a4, target.len());
        assert_eq!(args.a5, 0);
        SyscallRet::success(0)
    }

    let service = SyscallServiceControl::new(RawSyscall::new(syscall));
    service
        .request("shell", "/bin/sh")
        .expect("service request succeeds");
}

#[test]
#[allow(unsafe_code)]
fn syscall_service_control_lowers_stop_report_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SERVICE_CONTROL);
        assert_eq!(args.a0, ServiceControlOp::STOP.raw());
        let service_name = "editor";
        assert_eq!(args.a1, service_name.as_ptr().addr());
        assert_eq!(args.a2, service_name.len());
        assert_ne!(args.a3, 0);
        assert_eq!(args.a4, size_of::<ServiceControlReport>());
        assert_eq!(args.a5, 0);
        // SAFETY: the wrapper passes a live report for this synchronous fixture
        // backend.
        let report = unsafe { &mut *(args.a3 as *mut ServiceControlReport) };
        report.set_name("editor");
        report.set_target("/payload/editor-smoke");
        report.set_service_pid(4);
        report.set_service_task_id(4);
        report.set_state(ServiceStateCode::STOPPED);
        report.set_reason(ServiceReasonCode::OPERATOR_STOP);
        SyscallRet::success(4)
    }

    let service = SyscallServiceControl::new(RawSyscall::new(syscall));
    let report = service
        .stop_report("editor")
        .expect("service stop report succeeds");
    assert_eq!(report.name_bytes(), b"editor");
    assert_eq!(report.target_bytes(), b"/payload/editor-smoke");
    assert_eq!(report.service_pid(), 4);
    assert_eq!(report.service_task_id(), 4);
    assert_eq!(report.state(), ServiceStateCode::STOPPED);
    assert_eq!(report.reason(), ServiceReasonCode::OPERATOR_STOP);
}

#[test]
#[allow(unsafe_code)]
fn syscall_service_control_lowers_start_report_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SERVICE_CONTROL);
        assert_eq!(args.a0, ServiceControlOp::START.raw());
        let service_name = "editor";
        assert_eq!(args.a1, service_name.as_ptr().addr());
        assert_eq!(args.a2, service_name.len());
        assert_ne!(args.a3, 0);
        assert_eq!(args.a4, size_of::<ServiceControlReport>());
        assert_eq!(args.a5, 0);
        // SAFETY: the wrapper passes a live report for this synchronous fixture
        // backend.
        let report = unsafe { &mut *(args.a3 as *mut ServiceControlReport) };
        report.set_name("editor");
        report.set_target("/payload/editor-smoke");
        report.set_service_pid(7);
        report.set_service_task_id(7);
        report.set_state(ServiceStateCode::STARTED);
        report.set_reason(ServiceReasonCode::RUNNING);
        report.set_result(ServiceControlResultCode::PAYLOAD_RESIDENT);
        report.set_exit_code(0);
        SyscallRet::success(7)
    }

    let service = SyscallServiceControl::new(RawSyscall::new(syscall));
    let report = service
        .start_report("editor")
        .expect("service start report succeeds");
    assert_eq!(report.name_bytes(), b"editor");
    assert_eq!(report.target_bytes(), b"/payload/editor-smoke");
    assert_eq!(report.service_pid(), 7);
    assert_eq!(report.service_task_id(), 7);
    assert_eq!(report.state(), ServiceStateCode::STARTED);
    assert_eq!(report.reason(), ServiceReasonCode::RUNNING);
    assert_eq!(report.result(), ServiceControlResultCode::PAYLOAD_RESIDENT);
    assert_eq!(report.exit_code(), 0);
}

#[test]
#[allow(unsafe_code)]
fn syscall_service_control_lowers_restart_report_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SERVICE_CONTROL);
        assert_eq!(args.a0, ServiceControlOp::RESTART.raw());
        let service_name = "editor";
        assert_eq!(args.a1, service_name.as_ptr().addr());
        assert_eq!(args.a2, service_name.len());
        assert_ne!(args.a3, 0);
        assert_eq!(args.a4, size_of::<ServiceControlReport>());
        assert_eq!(args.a5, 0);
        // SAFETY: the wrapper passes a live report for this synchronous fixture
        // backend.
        let report = unsafe { &mut *(args.a3 as *mut ServiceControlReport) };
        report.set_name("editor");
        report.set_target("/payload/editor-smoke");
        report.set_service_pid(8);
        report.set_service_task_id(8);
        report.set_state(ServiceStateCode::STARTED);
        report.set_reason(ServiceReasonCode::RUNNING);
        report.set_result(ServiceControlResultCode::PAYLOAD_RESIDENT);
        report.set_exit_code(0);
        SyscallRet::success(8)
    }

    let service = SyscallServiceControl::new(RawSyscall::new(syscall));
    let report = service
        .restart_report("editor")
        .expect("service restart report succeeds");
    assert_eq!(report.name_bytes(), b"editor");
    assert_eq!(report.target_bytes(), b"/payload/editor-smoke");
    assert_eq!(report.service_pid(), 8);
    assert_eq!(report.service_task_id(), 8);
    assert_eq!(report.state(), ServiceStateCode::STARTED);
    assert_eq!(report.reason(), ServiceReasonCode::RUNNING);
    assert_eq!(report.result(), ServiceControlResultCode::PAYLOAD_RESIDENT);
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn syscall_service_control_maps_transport_errors() {
    fn no_current(_: SyscallNr, _: SyscallArgs) -> SyscallRet {
        SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS)
    }

    let service = SyscallServiceControl::new(RawSyscall::new(no_current));
    assert_eq!(service.stop_report("editor"), Err(ServiceError::NO_CURRENT_PROCESS),);
    assert_eq!(service.start_report("editor"), Err(ServiceError::NO_CURRENT_PROCESS),);
    assert_eq!(service.restart_report("editor"), Err(ServiceError::NO_CURRENT_PROCESS),);
    assert_eq!(service.request("shell", "/bin/sh"), Err(ServiceError::NO_CURRENT_PROCESS),);
    assert_eq!(service.stop_report(""), Err(ServiceError::INVALID_ARGUMENT));
    assert_eq!(service.start_report(""), Err(ServiceError::INVALID_ARGUMENT));
    assert_eq!(service.restart_report(""), Err(ServiceError::INVALID_ARGUMENT));
    assert_eq!(service.request("", "/bin/sh"), Err(ServiceError::INVALID_ARGUMENT));
    assert_eq!(service.request("shell", ""), Err(ServiceError::INVALID_ARGUMENT));
}
