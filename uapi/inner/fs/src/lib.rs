//! Product-facing filesystem and borrowed-fd control vocabulary.
//!
//! Upper layers receive these function tables from composition roots. Concrete
//! implementations live below the system-kernel bridge.

#![no_std]

/// Product-facing borrowed file descriptor.
///
/// This is an opaque handle for process-owned descriptors such as stdin and
/// stdout. The bridge owns interpretation of the numeric value; upper layers
/// use the typed constructors instead of importing POSIX vocabulary directly.
///
/// ```rust
/// use reovim_uapi_fs::RawFd;
///
/// assert_eq!(RawFd::stdin().raw(), 0);
/// assert_eq!(RawFd::stdout().raw(), 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct RawFd(i32);

impl RawFd {
    /// Builds a borrowed descriptor from a bridge-issued value.
    ///
    /// ```rust
    /// use reovim_uapi_fs::RawFd;
    ///
    /// assert_eq!(RawFd::new(7).raw(), 7);
    /// ```
    #[must_use]
    pub const fn new(raw: i32) -> Self {
        Self(raw)
    }

    /// The process standard input descriptor.
    ///
    /// ```rust
    /// use reovim_uapi_fs::RawFd;
    ///
    /// assert_eq!(RawFd::stdin().raw(), 0);
    /// ```
    #[must_use]
    pub const fn stdin() -> Self {
        Self(0)
    }

    /// The process standard output descriptor.
    ///
    /// ```rust
    /// use reovim_uapi_fs::RawFd;
    ///
    /// assert_eq!(RawFd::stdout().raw(), 1);
    /// ```
    #[must_use]
    pub const fn stdout() -> Self {
        Self(1)
    }

    /// The process standard error descriptor.
    ///
    /// ```rust
    /// use reovim_uapi_fs::RawFd;
    ///
    /// assert_eq!(RawFd::stderr().raw(), 2);
    /// ```
    #[must_use]
    pub const fn stderr() -> Self {
        Self(2)
    }

    /// Returns the bridge-owned descriptor value.
    ///
    /// ```rust
    /// use reovim_uapi_fs::RawFd;
    ///
    /// assert_eq!(RawFd::new(4).raw(), 4);
    /// ```
    #[must_use]
    pub const fn raw(self) -> i32 {
        self.0
    }
}

/// Product-facing filesystem I/O error.
///
/// The numeric code is diagnostic bridge/provider detail. It is not permission
/// for upper layers to call POSIX directly.
///
/// ```rust
/// use reovim_uapi_fs::FsError;
///
/// assert_eq!(FsError::new(9).code(), 9);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct FsError(i32);

impl FsError {
    /// Builds an I/O error from a bridge/provider code.
    ///
    /// ```rust
    /// use reovim_uapi_fs::FsError;
    ///
    /// assert_eq!(FsError::new(13).code(), 13);
    /// ```
    #[must_use]
    pub const fn new(code: i32) -> Self {
        Self(code)
    }

    /// Returns the diagnostic bridge/provider code.
    ///
    /// ```rust
    /// use reovim_uapi_fs::FsError;
    ///
    /// assert_eq!(FsError::new(2).code(), 2);
    /// ```
    #[must_use]
    pub const fn code(self) -> i32 {
        self.0
    }
}

/// Function pointer for reading from a borrowed descriptor.
///
/// ```rust
/// use reovim_uapi_fs::{FsError, RawFd, ReadRawFdFn};
///
/// fn read(_: RawFd, _: &mut [u8]) -> Result<usize, FsError> { Ok(0) }
/// let f: ReadRawFdFn = read;
/// assert_eq!(f(RawFd::stdin(), &mut [0; 1]), Ok(0));
/// ```
pub type ReadRawFdFn = fn(RawFd, &mut [u8]) -> Result<usize, FsError>;

/// Function pointer for writing to a borrowed descriptor.
///
/// ```rust
/// use reovim_uapi_fs::{FsError, RawFd, WriteRawFdFn};
///
/// fn write(_: RawFd, buf: &[u8]) -> Result<usize, FsError> { Ok(buf.len()) }
/// let f: WriteRawFdFn = write;
/// assert_eq!(f(RawFd::stdout(), b"ok"), Ok(2));
/// ```
pub type WriteRawFdFn = fn(RawFd, &[u8]) -> Result<usize, FsError>;

/// Function pointer for unlinking a path.
///
/// `path` is the bridge-owned byte form for a NUL-terminated path. The up-face
/// contract intentionally carries no POSIX directory-fd or flag vocabulary.
///
/// ```rust
/// use reovim_uapi_fs::{FsError, UnlinkPathFn};
///
/// fn unlink(path: &[u8]) -> Result<(), FsError> {
///     assert_eq!(path, b"/tmp/reovim.sock\0");
///     Ok(())
/// }
///
/// let f: UnlinkPathFn = unlink;
/// assert_eq!(f(b"/tmp/reovim.sock\0"), Ok(()));
/// ```
pub type UnlinkPathFn = fn(&[u8]) -> Result<(), FsError>;

/// Product-facing borrowed-fd control table.
///
/// ```rust
/// use reovim_uapi_fs::{FsError, RawFd, RawFdControl};
///
/// fn read(_: RawFd, _: &mut [u8]) -> Result<usize, FsError> { Ok(0) }
/// fn write(_: RawFd, buf: &[u8]) -> Result<usize, FsError> { Ok(buf.len()) }
///
/// let fs = RawFdControl::new(read, write);
/// assert_eq!(fs.read(RawFd::stdin(), &mut [0; 1]), Ok(0));
/// assert_eq!(fs.write(RawFd::stdout(), b"ok"), Ok(2));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct RawFdControl {
    /// Reads bytes from a borrowed descriptor.
    pub read_fn: ReadRawFdFn,
    /// Writes bytes to a borrowed descriptor.
    pub write_fn: WriteRawFdFn,
}

impl RawFdControl {
    /// Creates a no-op borrowed-fd control table.
    ///
    /// Reads return EOF and writes report that all bytes were accepted.
    ///
    /// ```rust
    /// use reovim_uapi_fs::{RawFd, RawFdControl};
    ///
    /// assert_eq!(RawFdControl::noop().read(RawFd::stdin(), &mut [0; 1]), Ok(0));
    /// assert_eq!(RawFdControl::noop().write(RawFd::stdout(), b"ok"), Ok(2));
    /// ```
    #[must_use]
    pub const fn noop() -> Self {
        Self::new(noop_read, noop_write)
    }

    /// Creates a borrowed-fd control table.
    ///
    /// ```rust
    /// use reovim_uapi_fs::{FsError, RawFd, RawFdControl};
    ///
    /// fn read(_: RawFd, _: &mut [u8]) -> Result<usize, FsError> { Ok(0) }
    /// fn write(_: RawFd, buf: &[u8]) -> Result<usize, FsError> { Ok(buf.len()) }
    ///
    /// let _fs = RawFdControl::new(read, write);
    /// ```
    #[must_use]
    pub const fn new(read_fn: ReadRawFdFn, write_fn: WriteRawFdFn) -> Self {
        Self { read_fn, write_fn }
    }

    /// Reads bytes from a borrowed descriptor.
    ///
    /// ```rust
    /// use reovim_uapi_fs::{RawFd, RawFdControl};
    ///
    /// assert_eq!(RawFdControl::noop().read(RawFd::stdin(), &mut [0; 1]), Ok(0));
    /// ```
    pub fn read(self, fd: RawFd, buf: &mut [u8]) -> Result<usize, FsError> {
        (self.read_fn)(fd, buf)
    }

    /// Writes bytes to a borrowed descriptor.
    ///
    /// ```rust
    /// use reovim_uapi_fs::{RawFd, RawFdControl};
    ///
    /// assert_eq!(RawFdControl::noop().write(RawFd::stdout(), b"ok"), Ok(2));
    /// ```
    pub fn write(self, fd: RawFd, buf: &[u8]) -> Result<usize, FsError> {
        (self.write_fn)(fd, buf)
    }
}

impl Default for RawFdControl {
    fn default() -> Self {
        Self::noop()
    }
}

/// Product-facing path-operation control table.
///
/// ```rust
/// use reovim_uapi_fs::{FsError, PathControl};
///
/// fn unlink(path: &[u8]) -> Result<(), FsError> {
///     assert_eq!(path, b"/tmp/reovim.sock\0");
///     Ok(())
/// }
///
/// let paths = PathControl::new(unlink);
/// assert_eq!(paths.unlink(b"/tmp/reovim.sock\0"), Ok(()));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct PathControl {
    /// Unlinks a path.
    pub unlink_fn: UnlinkPathFn,
}

impl PathControl {
    /// Creates a path control table that reports unsupported operations.
    ///
    /// ```rust
    /// use reovim_uapi_fs::{FsError, PathControl};
    ///
    /// assert_eq!(
    ///     PathControl::noop().unlink(b"/tmp/reovim.sock\0"),
    ///     Err(FsError::new(38)),
    /// );
    /// ```
    #[must_use]
    pub const fn noop() -> Self {
        Self::new(noop_unlink)
    }

    /// Creates a path control table.
    ///
    /// ```rust
    /// use reovim_uapi_fs::{FsError, PathControl};
    ///
    /// fn unlink(_: &[u8]) -> Result<(), FsError> { Ok(()) }
    ///
    /// let _paths = PathControl::new(unlink);
    /// ```
    #[must_use]
    pub const fn new(unlink_fn: UnlinkPathFn) -> Self {
        Self { unlink_fn }
    }

    /// Unlinks `path`.
    ///
    /// ```rust
    /// use reovim_uapi_fs::{FsError, PathControl};
    ///
    /// fn unlink(_: &[u8]) -> Result<(), FsError> { Ok(()) }
    ///
    /// assert_eq!(PathControl::new(unlink).unlink(b"/tmp/reovim.sock\0"), Ok(()));
    /// ```
    pub fn unlink(self, path: &[u8]) -> Result<(), FsError> {
        (self.unlink_fn)(path)
    }
}

impl Default for PathControl {
    fn default() -> Self {
        Self::noop()
    }
}

fn noop_read(_: RawFd, _: &mut [u8]) -> Result<usize, FsError> {
    Ok(0)
}

fn noop_write(_: RawFd, buf: &[u8]) -> Result<usize, FsError> {
    Ok(buf.len())
}

fn noop_unlink(_: &[u8]) -> Result<(), FsError> {
    Err(FsError::new(38))
}
