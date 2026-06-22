//! Filesystem bridge from up-face borrowed-fd controls to the platform handle.
//!
//! This module owns the current mapping from product-facing fd controls and
//! system log-file helpers to the down-face `kabi/platform` file slots. Upper
//! layers receive `uapi/fs` controls; `lib/ds` does not carry these file helpers.

use {
    reovim_kabi_platform::{Errno, Mode, OpenFlags, handle},
    reovim_uapi_fs::{FsError, PathControl, RawFd, RawFdControl},
};

pub(crate) type SysError = Errno;

/// Returns the borrowed-fd control table backed by the platform handle.
///
/// Composition roots pass this value into product runtimes so upper code can
/// read stdin/write stdout without importing lower file helpers.
///
/// ```rust,no_run
/// use reovim_system_kernel::fs::raw_fd_control;
/// use reovim_uapi_fs::RawFd;
///
/// let fs = raw_fd_control();
/// let _ = fs.write(RawFd::stderr(), b"diagnostic\n");
/// ```
#[must_use]
pub const fn raw_fd_control() -> RawFdControl {
    RawFdControl::new(read_raw_fd, write_raw_fd)
}

/// Returns the path-operation control table backed by the platform handle.
///
/// Composition roots use this for product/runtime path cleanup without
/// importing syscall-shaped lower APIs.
///
/// ```rust,no_run
/// use reovim_system_kernel::fs::path_control;
///
/// let _ = path_control().unlink(b"/tmp/reovim.sock\0");
/// ```
#[must_use]
pub const fn path_control() -> PathControl {
    PathControl::new(unlink_path)
}

pub(crate) struct File {
    fd: i32,
}

impl File {
    pub(crate) fn open(path: &[u8], flags: OpenFlags, mode: u32) -> Result<Self, SysError> {
        let fd = handle().file_open(path, flags, Mode(mode))?;
        Ok(Self { fd })
    }

    #[must_use]
    pub(crate) const fn into_raw_fd(self) -> i32 {
        let fd = self.fd;
        core::mem::forget(self);
        fd
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let _ = close_fd(self.fd);
    }
}

pub(crate) fn close_fd(fd: i32) -> Result<(), SysError> {
    handle().fd_close(fd)
}

pub(crate) fn write_fd(fd: i32, buf: &[u8]) -> Result<usize, SysError> {
    handle().file_write(fd, buf)
}

fn read_raw_fd(fd: RawFd, buf: &mut [u8]) -> Result<usize, FsError> {
    handle().fd_read(fd.raw(), buf).map_err(map_fs_error)
}

fn write_raw_fd(fd: RawFd, buf: &[u8]) -> Result<usize, FsError> {
    handle().file_write(fd.raw(), buf).map_err(map_fs_error)
}

fn unlink_path(path: &[u8]) -> Result<(), FsError> {
    handle().path_unlink(path).map_err(map_fs_error)
}

fn map_fs_error(error: SysError) -> FsError {
    FsError::new(error.code())
}
