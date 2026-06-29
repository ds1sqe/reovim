use {
    reovim_uapi_fs::{
        DescriptorFlags, FileStatusFlags, FsError, OpenAtDir, OpenFlags, PathControl, RawFd,
        RawFdControl, SeekWhence, SyscallFdControl,
    },
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr, SyscallRet},
};

fn read(fd: RawFd, buf: &mut [u8]) -> Result<usize, FsError> {
    assert_eq!(fd, RawFd::stdin());
    buf[0] = b'x';
    Ok(1)
}

fn write(fd: RawFd, buf: &[u8]) -> Result<usize, FsError> {
    assert_eq!(fd, RawFd::stdout());
    Ok(buf.len())
}

fn unlink(path: &[u8]) -> Result<(), FsError> {
    assert_eq!(path, b"/tmp/reovim-uapi-fs.sock\0");
    Ok(())
}

#[test]
fn raw_fd_control_dispatches() {
    let fs = RawFdControl::new(read, write);
    let mut buf = [0u8; 1];

    assert_eq!(fs.read(RawFd::stdin(), &mut buf), Ok(1));
    assert_eq!(buf[0], b'x');
    assert_eq!(fs.write(RawFd::stdout(), b"ok"), Ok(2));
}

#[test]
fn noop_control_is_stable() {
    let fs = RawFdControl::noop();

    assert_eq!(fs.read(RawFd::stdin(), &mut [0; 1]), Ok(0));
    assert_eq!(fs.write(RawFd::stdout(), b"ok"), Ok(2));
}

#[test]
fn path_control_dispatches() {
    let paths = PathControl::new(unlink);

    assert_eq!(paths.unlink(b"/tmp/reovim-uapi-fs.sock\0"), Ok(()));
}

#[test]
fn noop_path_control_reports_unsupported() {
    assert_eq!(PathControl::noop().unlink(b"/tmp/reovim-uapi-fs.sock\0"), Err(FsError::new(38)));
}

#[test]
fn syscall_fd_control_lowers_fd_ops_through_raw_transport() {
    fn syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        match nr {
            SyscallNr::OPEN_AT => {
                assert_eq!(args.get(0), Some(OpenAtDir::session_cwd().raw() as usize));
                assert_ne!(args.get(1), Some(0));
                assert_eq!(args.get(2), Some(13));
                assert_eq!(args.get(3), Some(OpenFlags::READ_ONLY.raw() as usize));
                SyscallRet::success(3)
            }
            SyscallNr::READ => {
                assert_eq!(args.get(0), Some(RawFd::stdin().raw() as usize));
                assert_ne!(args.get(1), Some(0));
                assert_eq!(args.get(2), Some(4));
                SyscallRet::success(1)
            }
            SyscallNr::GETDENTS => {
                assert_eq!(args.get(0), Some(3));
                assert_ne!(args.get(1), Some(0));
                assert_eq!(args.get(2), Some(5));
                SyscallRet::success(3)
            }
            SyscallNr::WRITE => {
                assert_eq!(args.get(0), Some(RawFd::stdout().raw() as usize));
                assert_ne!(args.get(1), Some(0));
                assert_eq!(args.get(2), Some(2));
                SyscallRet::success(2)
            }
            SyscallNr::CLOSE => {
                assert_eq!(args.get(0), Some(3));
                SyscallRet::success(0)
            }
            SyscallNr::DUP => {
                assert_eq!(args.get(0), Some(3));
                SyscallRet::success(4)
            }
            SyscallNr::DUP_TO => {
                assert_eq!(args.get(0), Some(3));
                assert_eq!(args.get(1), Some(6));
                SyscallRet::success(6)
            }
            SyscallNr::FD_FLAGS_GET => {
                assert_eq!(args.get(0), Some(3));
                SyscallRet::success(DescriptorFlags::CLOSE_ON_EXEC.raw() as usize)
            }
            SyscallNr::FD_FLAGS_SET => {
                assert_eq!(args.get(0), Some(3));
                assert_eq!(args.get(1), Some(DescriptorFlags::CLOSE_ON_EXEC.raw() as usize));
                SyscallRet::success(0)
            }
            SyscallNr::FD_STATUS_GET => {
                assert_eq!(args.get(0), Some(3));
                SyscallRet::success(FileStatusFlags::NONBLOCK.raw() as usize)
            }
            SyscallNr::FD_STATUS_SET => {
                assert_eq!(args.get(0), Some(3));
                assert_eq!(args.get(1), Some(FileStatusFlags::NONBLOCK.raw() as usize));
                SyscallRet::success(0)
            }
            SyscallNr::PIPE => {
                assert_ne!(args.get(0), Some(0));
                assert_eq!(args.get(1), Some(2));
                SyscallRet::success(0)
            }
            SyscallNr::LSEEK => {
                assert_eq!(args.get(0), Some(3));
                assert_eq!(args.get(1), Some((-2isize) as usize));
                assert_eq!(args.get(2), Some(SeekWhence::end().raw() as usize));
                SyscallRet::success(11)
            }
            SyscallNr::GET_CWD => {
                assert_ne!(args.get(0), Some(0));
                assert_eq!(args.get(1), Some(8));
                SyscallRet::success(4)
            }
            SyscallNr::CHDIR => {
                assert_ne!(args.get(0), Some(0));
                assert_eq!(args.get(1), Some(5));
                SyscallRet::success(0)
            }
            _ => SyscallRet::failure(SyscallError::INVALID_ARGUMENT),
        }
    }

    let fd = SyscallFdControl::new(RawSyscall::new(syscall));
    let mut buf = [0; 4];
    let mut dir_buf = [0; 5];
    let mut cwd = [0; 8];

    let opened = fd
        .open_at(OpenAtDir::session_cwd(), b"/boot/profile", OpenFlags::READ_ONLY)
        .expect("open-at returns fd");
    assert_eq!(opened, RawFd::new(3));
    assert_eq!(fd.read(RawFd::stdin(), &mut buf), Ok(1));
    assert_eq!(fd.getdents(opened, &mut dir_buf), Ok(3));
    assert_eq!(fd.write(RawFd::stdout(), b"ok"), Ok(2));
    assert_eq!(fd.duplicate(opened), Ok(RawFd::new(4)));
    assert_eq!(fd.duplicate_to(opened, RawFd::new(6)), Ok(RawFd::new(6)));
    assert_eq!(fd.descriptor_flags(opened), Ok(DescriptorFlags::CLOSE_ON_EXEC),);
    assert_eq!(fd.set_descriptor_flags(opened, DescriptorFlags::CLOSE_ON_EXEC), Ok(()),);
    assert_eq!(
        fd.set_descriptor_flags(opened, DescriptorFlags::new(2)),
        Err(FsError::new(SyscallError::INVALID_ARGUMENT.code())),
    );
    assert_eq!(fd.status_flags(opened), Ok(FileStatusFlags::NONBLOCK));
    assert_eq!(fd.set_status_flags(opened, FileStatusFlags::NONBLOCK), Ok(()));
    assert_eq!(
        fd.set_status_flags(opened, FileStatusFlags::new(2)),
        Err(FsError::new(SyscallError::INVALID_ARGUMENT.code())),
    );
    let mut pipe_fds = [RawFd::new(-1), RawFd::new(-1)];
    assert_eq!(fd.pipe(&mut pipe_fds), Ok(()));
    assert_eq!(fd.seek(opened, -2, SeekWhence::end()), Ok(11));
    assert_eq!(fd.get_cwd(&mut cwd), Ok(4));
    assert_eq!(fd.chdir(b"/boot"), Ok(()));
    assert_eq!(fd.close(opened), Ok(()));
}

#[test]
fn descriptor_constructors_keep_standard_numbers() {
    assert_eq!(RawFd::stdin().raw(), 0);
    assert_eq!(RawFd::stdout().raw(), 1);
    assert_eq!(RawFd::stderr().raw(), 2);
    assert_eq!(RawFd::new(7).raw(), 7);
    assert_eq!(OpenAtDir::session_cwd().raw(), -1);
    assert_eq!(OpenAtDir::from_fd(RawFd::new(9)).raw(), 9);
    assert_eq!(OpenFlags::READ_ONLY.raw(), 0);
    assert_eq!(OpenFlags::READ_DIRECTORY.raw(), 1);
    assert_eq!(OpenFlags::WRITE_ONLY.raw(), 2);
    assert_eq!(OpenFlags::CLOSE_ON_EXEC.raw(), 256);
    assert_eq!(OpenFlags::READ_ONLY.with_close_on_exec().raw(), 256);
    assert_eq!(OpenFlags::READ_DIRECTORY.with_close_on_exec().raw(), 257);
    assert_eq!(OpenFlags::WRITE_ONLY.with_close_on_exec().raw(), 258);
    assert!(
        OpenFlags::READ_ONLY
            .with_close_on_exec()
            .has_close_on_exec()
    );
    assert!(
        OpenFlags::READ_DIRECTORY
            .with_close_on_exec()
            .is_read_directory()
    );
    assert!(
        OpenFlags::READ_DIRECTORY
            .with_close_on_exec()
            .is_supported_read_open()
    );
    assert!(
        OpenFlags::WRITE_ONLY
            .with_close_on_exec()
            .is_supported_open()
    );
    assert!(OpenFlags::WRITE_ONLY.with_close_on_exec().is_write_only());
    assert!(
        !OpenFlags::WRITE_ONLY
            .with_close_on_exec()
            .is_supported_read_open()
    );
    assert!(!OpenFlags::new(3).is_supported_open());
    assert!(!OpenFlags::new(99).is_supported_read_open());
    assert_eq!(OpenFlags::new(5).raw(), 5);
    assert_eq!(FsError::BUSY.code(), SyscallError::BUSY.code());
    assert_eq!(DescriptorFlags::EMPTY.raw(), 0);
    assert_eq!(DescriptorFlags::empty().raw(), 0);
    assert_eq!(DescriptorFlags::CLOSE_ON_EXEC.raw(), 1);
    assert_eq!(DescriptorFlags::close_on_exec().raw(), 1);
    assert_eq!(DescriptorFlags::empty().with_close_on_exec(), DescriptorFlags::CLOSE_ON_EXEC,);
    assert!(DescriptorFlags::CLOSE_ON_EXEC.has_close_on_exec());
    assert!(DescriptorFlags::CLOSE_ON_EXEC.is_supported());
    assert!(!DescriptorFlags::new(2).is_supported());
    assert_eq!(FileStatusFlags::EMPTY.raw(), 0);
    assert_eq!(FileStatusFlags::empty().raw(), 0);
    assert_eq!(FileStatusFlags::NONBLOCK.raw(), 1);
    assert_eq!(FileStatusFlags::nonblock().raw(), 1);
    assert_eq!(FileStatusFlags::empty().with_nonblock(), FileStatusFlags::NONBLOCK,);
    assert!(FileStatusFlags::NONBLOCK.is_nonblocking());
    assert!(FileStatusFlags::NONBLOCK.is_supported());
    assert!(!FileStatusFlags::new(2).is_supported());
    assert_eq!(SeekWhence::start().raw(), 0);
    assert_eq!(SeekWhence::current().raw(), 1);
    assert_eq!(SeekWhence::end().raw(), 2);
    assert_eq!(SeekWhence::new(9).raw(), 9);
}
