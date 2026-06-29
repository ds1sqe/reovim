use {
    core::mem::{align_of, size_of},
    reovim_uapi_source::{
        SourceControlOp, SourceError, SourceInstallOriginCode, SourceInstallReport,
        SourceInstallStatusCode, SourceNamespaceCode, SyscallSourceControl,
    },
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr, SyscallRet},
};

#[test]
fn source_scalars_and_report_are_stable() {
    assert_eq!(SourceControlOp::INSTALL_BIN_STATUS.raw(), 0);
    assert_eq!(SourceControlOp::INSTALL_PAYLOAD_STATUS.raw(), 1);
    assert_eq!(SourceControlOp::INSTALL_BIN_MEDIA.raw(), 2);
    assert_eq!(SourceControlOp::INSTALL_PAYLOAD_MEDIA.raw(), 3);
    assert_eq!(SourceControlOp::new(0), SourceControlOp::INSTALL_BIN_STATUS);
    assert_eq!(SourceNamespaceCode::NONE.raw(), 0);
    assert_eq!(SourceNamespaceCode::BIN.raw(), 1);
    assert_eq!(SourceNamespaceCode::PAYLOAD.raw(), 2);
    assert_eq!(SourceInstallStatusCode::NONE.raw(), 0);
    assert_eq!(SourceInstallStatusCode::OK.raw(), 1);
    assert_eq!(SourceInstallStatusCode::ERROR.raw(), 2);
    assert_eq!(SourceInstallStatusCode::READY.raw(), 3);
    assert_eq!(SourceInstallStatusCode::FAILED.raw(), 4);
    assert_eq!(SourceInstallOriginCode::NONE.raw(), 0);
    assert_eq!(SourceInstallOriginCode::INSTALLED.raw(), 1);
    assert_eq!(SourceInstallOriginCode::SOURCE_MEDIA.raw(), 2);
    assert_eq!(
        SourceError::SOURCE_MEDIA_UNAVAILABLE.code(),
        SyscallError::SOURCE_MEDIA_UNAVAILABLE.code()
    );
    assert_eq!(
        SourceError::SOURCE_MEDIA_NAMESPACE_MISMATCH.code(),
        SyscallError::SOURCE_MEDIA_NAMESPACE_MISMATCH.code()
    );

    assert_eq!(align_of::<SourceInstallReport>(), align_of::<usize>());
    assert!(size_of::<SourceInstallReport>() >= 160);

    let mut report = SourceInstallReport::empty();
    report.set_namespace(SourceNamespaceCode::BIN);
    report.set_status(SourceInstallStatusCode::OK);
    report.set_origin(SourceInstallOriginCode::SOURCE_MEDIA);
    report.set_bytes_len(21);
    report.set_storage_capacity_bytes(4096);
    report.set_artifact_bytes_len(128);
    report.set_checksum(42);
    report.set_name("hello");
    report.set_path("/bin/hello");
    report.set_storage("source-media");
    assert_eq!(report.namespace(), SourceNamespaceCode::BIN);
    assert_eq!(report.status(), SourceInstallStatusCode::OK);
    assert_eq!(report.origin(), SourceInstallOriginCode::SOURCE_MEDIA);
    assert_eq!(report.bytes_len(), 21);
    assert_eq!(report.storage_capacity_bytes(), 4096);
    assert_eq!(report.artifact_bytes_len(), 128);
    assert_eq!(report.checksum(), 42);
    assert_eq!(report.name_bytes(), b"hello");
    assert_eq!(report.path_bytes(), b"/bin/hello");
    assert_eq!(report.storage_bytes(), b"source-media");
    assert!(!report.name_truncated());
    assert!(!report.path_truncated());
    assert!(!report.storage_truncated());
    report.clear();
    assert_eq!(report, SourceInstallReport::empty());
}

#[test]
#[allow(unsafe_code)]
fn syscall_source_control_lowers_bin_status_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SOURCE_CONTROL);
        assert_eq!(args.a0, SourceControlOp::INSTALL_BIN_STATUS.raw());
        let name = "proc";
        assert_eq!(args.a1, name.as_ptr().addr());
        assert_eq!(args.a2, name.len());
        assert_eq!(args.a3, SourceInstallStatusCode::OK.raw());
        assert_ne!(args.a4, 0);
        assert_eq!(args.a5, size_of::<SourceInstallReport>());
        // SAFETY: the wrapper passes a live report for this synchronous fixture
        // backend.
        let report = unsafe { &mut *(args.a4 as *mut SourceInstallReport) };
        report.set_namespace(SourceNamespaceCode::BIN);
        report.set_status(SourceInstallStatusCode::OK);
        report.set_origin(SourceInstallOriginCode::INSTALLED);
        report.set_name("proc");
        report.set_path("/bin/proc");
        report.set_bytes_len(21);
        SyscallRet::success(21)
    }

    let source = SyscallSourceControl::new(RawSyscall::new(syscall));
    let report = source
        .install_bin_status("proc", SourceInstallStatusCode::OK)
        .expect("source install succeeds");
    assert_eq!(report.namespace(), SourceNamespaceCode::BIN);
    assert_eq!(report.status(), SourceInstallStatusCode::OK);
    assert_eq!(report.origin(), SourceInstallOriginCode::INSTALLED);
    assert_eq!(report.name_bytes(), b"proc");
    assert_eq!(report.path_bytes(), b"/bin/proc");
    assert_eq!(report.bytes_len(), 21);
}

#[test]
#[allow(unsafe_code)]
fn syscall_source_control_lowers_payload_media_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::SOURCE_CONTROL);
        assert_eq!(args.a0, SourceControlOp::INSTALL_PAYLOAD_MEDIA.raw());
        let name = "server-smoke";
        assert_eq!(args.a1, name.as_ptr().addr());
        assert_eq!(args.a2, name.len());
        assert_eq!(args.a3, SourceInstallStatusCode::NONE.raw());
        assert_ne!(args.a4, 0);
        assert_eq!(args.a5, size_of::<SourceInstallReport>());
        // SAFETY: the wrapper passes a live report for this synchronous fixture
        // backend.
        let report = unsafe { &mut *(args.a4 as *mut SourceInstallReport) };
        report.set_namespace(SourceNamespaceCode::PAYLOAD);
        report.set_origin(SourceInstallOriginCode::SOURCE_MEDIA);
        report.set_name("server-smoke");
        report.set_path("/payload/server-smoke");
        report.set_storage("source-media");
        report.set_storage_capacity_bytes(4096);
        report.set_artifact_bytes_len(128);
        report.set_bytes_len(22);
        report.set_checksum(123);
        SyscallRet::success(22)
    }

    let source = SyscallSourceControl::new(RawSyscall::new(syscall));
    let report = source
        .install_payload_media("server-smoke")
        .expect("source-media install succeeds");
    assert_eq!(report.namespace(), SourceNamespaceCode::PAYLOAD);
    assert_eq!(report.origin(), SourceInstallOriginCode::SOURCE_MEDIA);
    assert_eq!(report.name_bytes(), b"server-smoke");
    assert_eq!(report.path_bytes(), b"/payload/server-smoke");
    assert_eq!(report.storage_bytes(), b"source-media");
    assert_eq!(report.storage_capacity_bytes(), 4096);
    assert_eq!(report.artifact_bytes_len(), 128);
    assert_eq!(report.bytes_len(), 22);
    assert_eq!(report.checksum(), 123);
}

#[test]
fn syscall_source_control_maps_transport_errors_and_rejects_bad_args() {
    fn unavailable(_: SyscallNr, _: SyscallArgs) -> SyscallRet {
        SyscallRet::failure(SyscallError::SOURCE_MEDIA_UNAVAILABLE)
    }

    let source = SyscallSourceControl::new(RawSyscall::new(unavailable));
    assert_eq!(source.install_bin_media("proc"), Err(SourceError::SOURCE_MEDIA_UNAVAILABLE));
    assert_eq!(
        source.install_bin_status("", SourceInstallStatusCode::OK),
        Err(SourceError::INVALID_ARGUMENT)
    );
    assert_eq!(
        source.install_bin_status("proc", SourceInstallStatusCode::READY),
        Err(SourceError::INVALID_ARGUMENT)
    );
    assert_eq!(
        source.install_payload_status("server-smoke", SourceInstallStatusCode::ERROR),
        Err(SourceError::INVALID_ARGUMENT)
    );
}
