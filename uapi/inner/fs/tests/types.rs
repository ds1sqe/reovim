use reovim_uapi_fs::{FsError, PathControl, RawFd, RawFdControl};

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
fn descriptor_constructors_keep_standard_numbers() {
    assert_eq!(RawFd::stdin().raw(), 0);
    assert_eq!(RawFd::stdout().raw(), 1);
    assert_eq!(RawFd::stderr().raw(), 2);
    assert_eq!(RawFd::new(7).raw(), 7);
}
