//! Product-facing filesystem and borrowed-fd control vocabulary.
//!
//! Upper layers receive these function tables from composition roots. Concrete
//! implementations live below the system-kernel bridge.

#![no_std]

use reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr};

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
    /// The operation would block in the current direct backend.
    pub const BUSY: Self = Self(SyscallError::BUSY.code());

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

/// Directory anchor for Reovim `open_at` operations.
///
/// This is Reovim-owned vocabulary, not POSIX `AT_*` constants. The current OS
/// backend supports [`OpenAtDir::session_cwd`] and descriptors opened as
/// readable directory streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct OpenAtDir(i32);

impl OpenAtDir {
    /// Opens relative to the process/session current directory.
    pub const SESSION_CWD: Self = Self(-1);

    /// Opens relative to the process/session current directory.
    #[must_use]
    pub const fn session_cwd() -> Self {
        Self::SESSION_CWD
    }

    /// Opens relative to an existing directory descriptor.
    #[must_use]
    pub const fn from_fd(fd: RawFd) -> Self {
        Self(fd.raw())
    }

    /// Returns the raw Reovim open-at anchor scalar.
    #[must_use]
    pub const fn raw(self) -> i32 {
        self.0
    }
}

/// Reovim-owned file-open flags.
///
/// These are product-facing semantics, not provider `O_*` constants. The system
/// kernel bridge translates them to whatever lower face is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct OpenFlags(u32);

impl OpenFlags {
    const KIND_MASK: u32 = 0x0000_0001;
    const WRITE_ONLY_BIT: u32 = 0x0000_0002;
    const CLOSE_ON_EXEC_BIT: u32 = 0x0000_0100;
    const VALID_BITS: u32 = Self::KIND_MASK | Self::WRITE_ONLY_BIT | Self::CLOSE_ON_EXEC_BIT;

    /// Open a readable file without write, create, append, or truncate rights.
    pub const READ_ONLY: Self = Self(0);
    /// Open a readable directory stream for directory iteration.
    pub const READ_DIRECTORY: Self = Self(1);
    /// Open a writable device or stream without read, create, append, or truncate rights.
    ///
    /// This is Reovim-owned device/stream semantics, not provider `O_WRONLY`.
    pub const WRITE_ONLY: Self = Self(Self::WRITE_ONLY_BIT);
    /// Close the returned descriptor during successful process image replacement.
    ///
    /// This is Reovim-owned descriptor semantics, not the provider `O_CLOEXEC`
    /// scalar. It is intentionally attached to open so the flag is set
    /// atomically with descriptor allocation.
    pub const CLOSE_ON_EXEC: Self = Self(Self::CLOSE_ON_EXEC_BIT);

    /// Returns the readable-file flag set.
    #[must_use]
    pub const fn read_only() -> Self {
        Self::READ_ONLY
    }

    /// Returns the readable-directory flag set.
    #[must_use]
    pub const fn read_directory() -> Self {
        Self::READ_DIRECTORY
    }

    /// Returns the write-only device/stream flag set.
    #[must_use]
    pub const fn write_only() -> Self {
        Self::WRITE_ONLY
    }

    /// Returns this flag set with close-on-exec enabled.
    #[must_use]
    pub const fn with_close_on_exec(self) -> Self {
        Self(self.0 | Self::CLOSE_ON_EXEC_BIT)
    }

    /// Returns whether this flag set requests close-on-exec.
    #[must_use]
    pub const fn has_close_on_exec(self) -> bool {
        (self.0 & Self::CLOSE_ON_EXEC_BIT) != 0
    }

    /// Returns whether this flag set opens a readable directory stream.
    #[must_use]
    pub const fn is_read_directory(self) -> bool {
        (self.0 & Self::KIND_MASK) == Self::READ_DIRECTORY.0
    }

    /// Returns whether this flag set requests a write-only device/stream.
    #[must_use]
    pub const fn is_write_only(self) -> bool {
        (self.0 & Self::WRITE_ONLY_BIT) != 0
    }

    /// Returns whether the current bootstrap backend supports this flag shape.
    #[must_use]
    pub const fn is_supported_open(self) -> bool {
        (self.0 & !Self::VALID_BITS) == 0 && !(self.is_read_directory() && self.is_write_only())
    }

    /// Returns whether the current bootstrap kernel backend supports this set.
    #[must_use]
    pub const fn is_supported_read_open(self) -> bool {
        self.is_supported_open() && !self.is_write_only()
    }

    /// Builds flags from a bridge-issued scalar.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw Reovim open flag scalar.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Reovim-owned descriptor-local flags.
///
/// These flags describe the descriptor entry, not the opened object. They are
/// intentionally separate from [`OpenFlags`] so a program can inspect or update
/// descriptor-local state after open without adding a POSIX `fcntl` facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct DescriptorFlags(u32);

impl DescriptorFlags {
    const CLOSE_ON_EXEC_BIT: u32 = 0x0000_0001;
    const VALID_BITS: u32 = Self::CLOSE_ON_EXEC_BIT;

    /// No descriptor-local flags.
    pub const EMPTY: Self = Self(0);
    /// Close the descriptor during successful process image replacement.
    pub const CLOSE_ON_EXEC: Self = Self(Self::CLOSE_ON_EXEC_BIT);

    /// Returns an empty descriptor flag set.
    #[must_use]
    pub const fn empty() -> Self {
        Self::EMPTY
    }

    /// Returns the close-on-exec descriptor flag set.
    #[must_use]
    pub const fn close_on_exec() -> Self {
        Self::CLOSE_ON_EXEC
    }

    /// Returns this flag set with close-on-exec enabled.
    #[must_use]
    pub const fn with_close_on_exec(self) -> Self {
        Self(self.0 | Self::CLOSE_ON_EXEC_BIT)
    }

    /// Returns whether this descriptor will close during image replacement.
    #[must_use]
    pub const fn has_close_on_exec(self) -> bool {
        (self.0 & Self::CLOSE_ON_EXEC_BIT) != 0
    }

    /// Returns whether the current bootstrap kernel supports this flag shape.
    #[must_use]
    pub const fn is_supported(self) -> bool {
        (self.0 & !Self::VALID_BITS) == 0
    }

    /// Builds descriptor flags from a bridge-issued scalar.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw Reovim descriptor flag scalar.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Reovim-owned opened-object status flags.
///
/// These flags describe the stream or opened object reached through a
/// descriptor. They are intentionally separate from [`DescriptorFlags`]:
/// descriptor flags belong to one descriptor entry, while status flags are
/// shared by duplicated descriptors that refer to the same endpoint or opened
/// object. This is not a public POSIX `fcntl` or `O_NONBLOCK` facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct FileStatusFlags(u32);

impl FileStatusFlags {
    const NONBLOCK_BIT: u32 = 0x0000_0001;
    const VALID_BITS: u32 = Self::NONBLOCK_BIT;

    /// No opened-object status flags.
    pub const EMPTY: Self = Self(0);
    /// Empty live reads return immediately instead of blocking the process.
    pub const NONBLOCK: Self = Self(Self::NONBLOCK_BIT);

    /// Returns an empty status flag set.
    #[must_use]
    pub const fn empty() -> Self {
        Self::EMPTY
    }

    /// Returns the nonblocking status flag set.
    #[must_use]
    pub const fn nonblock() -> Self {
        Self::NONBLOCK
    }

    /// Returns this flag set with nonblocking enabled.
    #[must_use]
    pub const fn with_nonblock(self) -> Self {
        Self(self.0 | Self::NONBLOCK_BIT)
    }

    /// Returns whether reads should avoid blocking when no data is ready.
    #[must_use]
    pub const fn is_nonblocking(self) -> bool {
        (self.0 & Self::NONBLOCK_BIT) != 0
    }

    /// Returns whether the current bootstrap kernel supports this flag shape.
    #[must_use]
    pub const fn is_supported(self) -> bool {
        (self.0 & !Self::VALID_BITS) == 0
    }

    /// Builds status flags from a bridge-issued scalar.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw Reovim status flag scalar.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Offset anchor for Reovim fd seek operations.
///
/// This is Reovim-owned vocabulary. It deliberately avoids exposing POSIX
/// `SEEK_*` constants as the product ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SeekWhence(u32);

impl SeekWhence {
    /// Seek relative to the beginning of the opened object.
    pub const START: Self = Self(0);
    /// Seek relative to the current opened-object offset.
    pub const CURRENT: Self = Self(1);
    /// Seek relative to the end of the opened object.
    pub const END: Self = Self(2);

    /// Seek relative to the beginning of the opened object.
    #[must_use]
    pub const fn start() -> Self {
        Self::START
    }

    /// Seek relative to the current opened-object offset.
    #[must_use]
    pub const fn current() -> Self {
        Self::CURRENT
    }

    /// Seek relative to the end of the opened object.
    #[must_use]
    pub const fn end() -> Self {
        Self::END
    }

    /// Builds a whence value from a bridge-issued scalar.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw Reovim whence scalar.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

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

/// Borrowed-fd control backed by the raw Reovim syscall transport.
///
/// This adapter keeps filesystem semantics in `uapi::fs`: callers use fd
/// read/write operations, and only this lowest wrapper layer packs raw syscall
/// numbers and scalar arguments.
#[derive(Debug, Clone, Copy)]
pub struct SyscallFdControl {
    raw: RawSyscall,
}

impl SyscallFdControl {
    /// Creates a borrowed-fd control adapter over the raw syscall transport.
    #[must_use]
    pub const fn new(raw: RawSyscall) -> Self {
        Self { raw }
    }

    /// Reads bytes from a borrowed descriptor through the raw syscall transport.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the raw syscall transport reports failure.
    pub fn read(self, fd: RawFd, buf: &mut [u8]) -> Result<usize, FsError> {
        self.raw
            .invoke(
                SyscallNr::READ,
                SyscallArgs::new([fd_arg(fd), buf.as_mut_ptr().addr(), buf.len(), 0, 0, 0]),
            )
            .decode()
            .map_err(fs_error_from_syscall)
    }

    /// Writes bytes to a borrowed descriptor through the raw syscall transport.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the raw syscall transport reports failure.
    pub fn write(self, fd: RawFd, buf: &[u8]) -> Result<usize, FsError> {
        self.raw
            .invoke(
                SyscallNr::WRITE,
                SyscallArgs::new([fd_arg(fd), buf.as_ptr().addr(), buf.len(), 0, 0, 0]),
            )
            .decode()
            .map_err(fs_error_from_syscall)
    }

    /// Opens a file, directory stream, device, or stream through the raw syscall transport.
    ///
    /// The current kernel backend supports `OpenAtDir::session_cwd()` or an
    /// opened directory descriptor with `OpenFlags::READ_ONLY` /
    /// `OpenFlags::READ_DIRECTORY`, plus `OpenFlags::WRITE_ONLY` for devices
    /// or streams that explicitly support output. Broader create/append/
    /// truncate semantics belong to later fd-table/open-file-description
    /// rollout.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the raw syscall transport reports failure.
    pub fn open_at(self, dir: OpenAtDir, path: &[u8], flags: OpenFlags) -> Result<RawFd, FsError> {
        let fd = self
            .raw
            .invoke(
                SyscallNr::OPEN_AT,
                SyscallArgs::new([
                    dir_arg(dir),
                    path.as_ptr().addr(),
                    path.len(),
                    flags.raw() as usize,
                    0,
                    0,
                ]),
            )
            .decode()
            .map_err(fs_error_from_syscall)?;
        if fd > i32::MAX as usize {
            return Err(fs_error_from_syscall(SyscallError::INVALID_ARGUMENT));
        }
        Ok(RawFd::new(fd as i32))
    }

    /// Reads directory entry bytes from an opened directory stream.
    ///
    /// The returned bytes are a Reovim-owned newline-delimited entry stream for
    /// the current bootstrap VFS. A future record-oriented directory ABI can
    /// replace this buffer contract behind the same domain operation.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the raw syscall transport reports failure.
    pub fn getdents(self, fd: RawFd, buf: &mut [u8]) -> Result<usize, FsError> {
        self.raw
            .invoke(
                SyscallNr::GETDENTS,
                SyscallArgs::new([fd_arg(fd), buf.as_mut_ptr().addr(), buf.len(), 0, 0, 0]),
            )
            .decode()
            .map_err(fs_error_from_syscall)
    }

    /// Closes a descriptor through the raw syscall transport.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the raw syscall transport reports failure.
    pub fn close(self, fd: RawFd) -> Result<(), FsError> {
        self.raw
            .invoke(SyscallNr::CLOSE, SyscallArgs::new([fd_arg(fd), 0, 0, 0, 0, 0]))
            .decode()
            .map(|_| ())
            .map_err(fs_error_from_syscall)
    }

    /// Duplicates a descriptor through the raw syscall transport.
    ///
    /// The returned descriptor refers to the same open file description as
    /// `fd`, so offset movement is shared. This is a filesystem-domain
    /// operation; raw `DUP` remains only the transport number.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the raw syscall transport reports failure.
    pub fn duplicate(self, fd: RawFd) -> Result<RawFd, FsError> {
        let fd = self
            .raw
            .invoke(SyscallNr::DUP, SyscallArgs::new([fd_arg(fd), 0, 0, 0, 0, 0]))
            .decode()
            .map_err(fs_error_from_syscall)?;
        if fd > i32::MAX as usize {
            return Err(fs_error_from_syscall(SyscallError::INVALID_ARGUMENT));
        }
        Ok(RawFd::new(fd as i32))
    }

    /// Duplicates `old_fd` into the requested `new_fd` descriptor slot.
    ///
    /// If `new_fd` is already open, the kernel closes it before installing the
    /// duplicate. The installed descriptor refers to the same open file
    /// description as `old_fd`, so offset movement is shared. Descriptor-local
    /// close-on-exec state is cleared on the installed descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when either descriptor number is invalid, `old_fd` is
    /// not open, or the raw syscall transport reports failure.
    pub fn duplicate_to(self, old_fd: RawFd, new_fd: RawFd) -> Result<RawFd, FsError> {
        let fd = self
            .raw
            .invoke(
                SyscallNr::DUP_TO,
                SyscallArgs::new([fd_arg(old_fd), fd_arg(new_fd), 0, 0, 0, 0]),
            )
            .decode()
            .map_err(fs_error_from_syscall)?;
        if fd > i32::MAX as usize {
            return Err(fs_error_from_syscall(SyscallError::INVALID_ARGUMENT));
        }
        Ok(RawFd::new(fd as i32))
    }

    /// Reads descriptor-local flags for `fd`.
    ///
    /// This is Reovim-owned descriptor vocabulary, not a public POSIX
    /// `fcntl(F_GETFD)` facade. The current supported flag is
    /// [`DescriptorFlags::CLOSE_ON_EXEC`].
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when `fd` is invalid, the returned flag set is not
    /// supported by this uapi version, or the raw syscall transport reports
    /// failure.
    pub fn descriptor_flags(self, fd: RawFd) -> Result<DescriptorFlags, FsError> {
        let raw = self
            .raw
            .invoke(SyscallNr::FD_FLAGS_GET, SyscallArgs::new([fd_arg(fd), 0, 0, 0, 0, 0]))
            .decode()
            .map_err(fs_error_from_syscall)?;
        if raw > u32::MAX as usize {
            return Err(fs_error_from_syscall(SyscallError::INVALID_ARGUMENT));
        }
        let flags = DescriptorFlags::new(raw as u32);
        if !flags.is_supported() {
            return Err(fs_error_from_syscall(SyscallError::INVALID_ARGUMENT));
        }
        Ok(flags)
    }

    /// Replaces descriptor-local flags for `fd`.
    ///
    /// This is Reovim-owned descriptor vocabulary, not a public POSIX
    /// `fcntl(F_SETFD)` facade.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when `fd` is invalid, `flags` contains unsupported
    /// bits, or the raw syscall transport reports failure.
    pub fn set_descriptor_flags(self, fd: RawFd, flags: DescriptorFlags) -> Result<(), FsError> {
        if !flags.is_supported() {
            return Err(fs_error_from_syscall(SyscallError::INVALID_ARGUMENT));
        }
        self.raw
            .invoke(
                SyscallNr::FD_FLAGS_SET,
                SyscallArgs::new([fd_arg(fd), flags.raw() as usize, 0, 0, 0, 0]),
            )
            .decode()
            .map(|_| ())
            .map_err(fs_error_from_syscall)
    }

    /// Reads opened-object status flags for `fd`.
    ///
    /// This is Reovim-owned stream/object vocabulary, not a public POSIX
    /// `fcntl(F_GETFL)` facade. The current supported flag is
    /// [`FileStatusFlags::NONBLOCK`].
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when `fd` is invalid, the returned flag set is not
    /// supported by this uapi version, or the raw syscall transport reports
    /// failure.
    pub fn status_flags(self, fd: RawFd) -> Result<FileStatusFlags, FsError> {
        let raw = self
            .raw
            .invoke(SyscallNr::FD_STATUS_GET, SyscallArgs::new([fd_arg(fd), 0, 0, 0, 0, 0]))
            .decode()
            .map_err(fs_error_from_syscall)?;
        if raw > u32::MAX as usize {
            return Err(fs_error_from_syscall(SyscallError::INVALID_ARGUMENT));
        }
        let flags = FileStatusFlags::new(raw as u32);
        if !flags.is_supported() {
            return Err(fs_error_from_syscall(SyscallError::INVALID_ARGUMENT));
        }
        Ok(flags)
    }

    /// Replaces opened-object status flags for `fd`.
    ///
    /// This is Reovim-owned status vocabulary, not a public POSIX
    /// `fcntl(F_SETFL)` facade.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when `fd` is invalid, `flags` contains unsupported
    /// bits, or the raw syscall transport reports failure.
    pub fn set_status_flags(self, fd: RawFd, flags: FileStatusFlags) -> Result<(), FsError> {
        if !flags.is_supported() {
            return Err(fs_error_from_syscall(SyscallError::INVALID_ARGUMENT));
        }
        self.raw
            .invoke(
                SyscallNr::FD_STATUS_SET,
                SyscallArgs::new([fd_arg(fd), flags.raw() as usize, 0, 0, 0, 0]),
            )
            .decode()
            .map(|_| ())
            .map_err(fs_error_from_syscall)
    }

    /// Creates a pipe and writes the read and write descriptors into `fds`.
    ///
    /// `fds[0]` is the read end and `fds[1]` is the write end. This is
    /// Reovim-owned fd vocabulary; the raw `PIPE` number is only the transport
    /// used by this filesystem-domain operation.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the current process has no descriptor capacity
    /// or the raw syscall transport reports failure.
    pub fn pipe(self, fds: &mut [RawFd; 2]) -> Result<(), FsError> {
        self.raw
            .invoke(
                SyscallNr::PIPE,
                SyscallArgs::new([fds.as_mut_ptr().addr(), fds.len(), 0, 0, 0, 0]),
            )
            .decode()
            .map(|_| ())
            .map_err(fs_error_from_syscall)
    }

    /// Seeks the opened-object offset for a descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the raw syscall transport reports failure.
    pub fn seek(self, fd: RawFd, offset: isize, whence: SeekWhence) -> Result<usize, FsError> {
        self.raw
            .invoke(
                SyscallNr::LSEEK,
                SyscallArgs::new([fd_arg(fd), offset as usize, whence.raw() as usize, 0, 0, 0]),
            )
            .decode()
            .map_err(fs_error_from_syscall)
    }

    /// Reads the process current-directory path into `buf`.
    ///
    /// The returned value is the number of path bytes written. The path is not
    /// NUL terminated; callers that need a text value should use exactly the
    /// returned prefix of `buf`.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the raw syscall transport reports failure,
    /// including when `buf` is too small for the current path.
    pub fn get_cwd(self, buf: &mut [u8]) -> Result<usize, FsError> {
        self.raw
            .invoke(
                SyscallNr::GET_CWD,
                SyscallArgs::new([buf.as_mut_ptr().addr(), buf.len(), 0, 0, 0, 0]),
            )
            .decode()
            .map_err(fs_error_from_syscall)
    }

    /// Changes the process current-directory path.
    ///
    /// The path bytes are interpreted by the filesystem domain; callers should
    /// pass UTF-8 shell path bytes without a trailing NUL.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when the raw syscall transport reports failure,
    /// including invalid, missing, or non-directory targets.
    pub fn chdir(self, path: &[u8]) -> Result<(), FsError> {
        self.raw
            .invoke(
                SyscallNr::CHDIR,
                SyscallArgs::new([path.as_ptr().addr(), path.len(), 0, 0, 0, 0]),
            )
            .decode()
            .map(|_| ())
            .map_err(fs_error_from_syscall)
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

fn fd_arg(fd: RawFd) -> usize {
    fd.raw() as usize
}

fn dir_arg(dir: OpenAtDir) -> usize {
    dir.raw() as usize
}

fn fs_error_from_syscall(error: SyscallError) -> FsError {
    FsError::new(error.code())
}
