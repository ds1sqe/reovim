use {
    core::mem::{align_of, size_of},
    reovim_uapi_dump::{DumpError, DumpSyncReport, SyscallDumpControl},
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr, SyscallRet},
};

#[test]
fn dump_sync_report_is_c_repr_transport_record() {
    assert!(size_of::<DumpSyncReport>() >= 160);
    assert_eq!(align_of::<DumpSyncReport>(), align_of::<usize>());

    let mut report = DumpSyncReport::empty();
    assert!(!report.attempted());
    assert_eq!(report.storage_bytes(), b"");
    assert_eq!(report.reason_bytes(), b"");

    report.set_attempted(true);
    report.set_persistent_available(true);
    report.set_written(true);
    report.set_verified(true);
    report.set_storage_capacity_bytes(4096);
    report.set_bytes_written(1024);
    report.set_checksum(55);
    report.set_storage("dump0");
    report.set_reason("written-readback-ok");

    assert!(report.attempted());
    assert!(report.persistent_available());
    assert!(report.written());
    assert!(report.verified());
    assert_eq!(report.storage_capacity_bytes(), 4096);
    assert_eq!(report.bytes_written(), 1024);
    assert_eq!(report.checksum(), 55);
    assert_eq!(report.storage_bytes(), b"dump0");
    assert_eq!(report.reason_bytes(), b"written-readback-ok");
}

#[test]
fn syscall_dump_control_lowers_sync_to_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        assert_eq!(nr, SyscallNr::DUMP_SYNC);
        assert_ne!(args.a0, 0);
        assert_eq!(args.a1, size_of::<DumpSyncReport>());
        assert_eq!(args.a2, 0);
        assert_eq!(args.a3, 0);
        assert_eq!(args.a4, 0);
        assert_eq!(args.a5, 0);
        SyscallRet::success(0)
    }

    let dump = SyscallDumpControl::new(RawSyscall::new(syscall));
    let mut report = DumpSyncReport::empty();
    assert_eq!(dump.sync(&mut report), Ok(()));
}

#[test]
fn syscall_dump_control_maps_transport_errors() {
    fn no_current(_: SyscallNr, _: SyscallArgs) -> SyscallRet {
        SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS)
    }
    fn io(_: SyscallNr, _: SyscallArgs) -> SyscallRet {
        SyscallRet::failure(SyscallError::IO)
    }

    let mut report = DumpSyncReport::empty();
    assert_eq!(
        SyscallDumpControl::new(RawSyscall::new(no_current)).sync(&mut report),
        Err(DumpError::NoCurrentProcess),
    );
    assert_eq!(
        SyscallDumpControl::new(RawSyscall::new(io)).sync(&mut report),
        Err(DumpError::Io),
    );
}
