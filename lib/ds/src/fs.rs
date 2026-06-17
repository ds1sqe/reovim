//! File + raw-fd Math wrappers over the kabi platform handle's file/sys
//! primitives.
//!
//! [`File`] holds an opaque `i32` fd and reaches its open/read/write/close
//! backend through the boot-installed `kabi` handle — never through `arch`
//! directly. This is the same pattern as [`crate::net`]'s [`UnixStream`]: the
//! algorithm (fd ownership, short-write note) is portable Math; only the
//! injected file/fd backend is World (master invariant 2: `lib/ds ⊄ arch`).
//!
//! The [`read_fd`]/[`write_fd`] free functions operate on a raw fd the caller
//! does NOT own (the tui's stdin/stdout fds, owned by the process): they never
//! close the fd, so they cannot be methods on [`File`] (whose `Drop` closes).
//!
//! ## `Drop` and `panic = "abort"`
//!
//! [`File`]'s `Drop` closes the fd through the handle. Under the workspace
//! `panic = "abort"` profile a panic terminates the process before `Drop` runs,
//! so a panicking holder leaks its fd to process teardown — acceptable for the
//! floor's single log-file consumer.
//!
//! [`UnixStream`]: crate::net::UnixStream

use reovim_kabi_platform::{NetError, handle};

/// A file/sys-call error carrying the platform's positive errno code.
///
/// File-open and fd failures are NOT net failures, so the consumer names
/// `SysError` rather than [`crate::net::Errno`] for an accurate error surface —
/// even though the underlying errno mapping is shared (both alias the kabi
/// [`NetError`] carrier, which is the one positive-errno type the fd-op slots
/// report).
///
/// ```rust
/// use reovim_lib_ds::fs::SysError;
///
/// assert_eq!(SysError::from_code(2).code(), 2);
/// ```
pub type SysError = NetError;

/// `O_WRONLY` — open for writing only (Linux open-flag constant).
///
/// ```rust
/// assert_eq!(reovim_lib_ds::fs::O_WRONLY, 0o1);
/// ```
pub const O_WRONLY: i32 = 0o1;
/// `O_CREAT` — create the file if it does not exist (Linux open-flag constant).
///
/// ```rust
/// assert_eq!(reovim_lib_ds::fs::O_CREAT, 0o100);
/// ```
pub const O_CREAT: i32 = 0o100;
/// `O_CLOEXEC` — close the fd on `exec` (Linux open-flag constant).
///
/// ```rust
/// assert_eq!(reovim_lib_ds::fs::O_CLOEXEC, 0o2_000_000);
/// ```
pub const O_CLOEXEC: i32 = 0o2_000_000;

/// An open file, owning its `i32` fd.
///
/// Obtained via [`File::open`]. `Drop` closes the underlying fd through the
/// handle. Writes use the plain-`write(2)` `file_write` slot (not the socket
/// `send` the net wrappers use); reads and close share the fd-op slots.
///
/// ```no_run
/// // no_run: a real open requires a booted handle.
/// use reovim_lib_ds::fs::{File, O_CREAT, O_WRONLY};
///
/// let file = File::open(b"/tmp/reovim-doctest.log\0", O_WRONLY | O_CREAT, 0o644)
///     .expect("open the file");
/// drop(file); // closes the fd through the handle
/// ```
pub struct File {
    fd: i32,
}

impl File {
    /// Opens `path` (a NUL-terminated byte string) with the Linux open `flags`
    /// and creation `mode` through the handle, returning an owning [`File`].
    ///
    /// # Errors
    ///
    /// Returns [`SysError`] on failure (missing path, permission denied, a
    /// malformed pathname).
    ///
    /// ```no_run
    /// // no_run: requires a booted handle.
    /// use reovim_lib_ds::fs::{File, O_CREAT, O_WRONLY};
    ///
    /// let _ = File::open(b"/tmp/reovim.log\0", O_WRONLY | O_CREAT, 0o644);
    /// ```
    pub fn open(path: &[u8], flags: i32, mode: u32) -> Result<Self, SysError> {
        let fd = handle().file_open(path, flags, mode)?;
        Ok(Self { fd })
    }

    /// The raw fd. Valid for the lifetime of the `File`.
    ///
    /// ```no_run
    /// // fd() is used internally; no standalone runnable example (a real File
    /// // needs a booted handle).
    /// ```
    #[must_use]
    pub const fn fd(&self) -> i32 {
        self.fd
    }

    /// Consumes the `File`, returning its raw fd WITHOUT closing it.
    ///
    /// Ownership of the fd transfers to the caller, who is then responsible for
    /// closing it (via [`close_fd`]). Use this when a consumer must drive the
    /// fd through a manual lifecycle that `Drop`-on-scope-exit does not fit —
    /// e.g. a process-global sink that registers the fd elsewhere and closes it
    /// on its own failure path.
    ///
    /// ```no_run
    /// // no_run: requires a booted handle + an open file.
    /// ```
    #[must_use]
    pub const fn into_raw_fd(self) -> i32 {
        let fd = self.fd;
        // Skip `Drop` so the fd is NOT closed: ownership passed to the caller.
        core::mem::forget(self);
        fd
    }

    /// Writes `buf`; returns the count written (may be a short write — the
    /// caller loops on a partial result, matching the net wrappers).
    ///
    /// # Errors
    ///
    /// Returns [`SysError`] on a write failure.
    ///
    /// ```no_run
    /// // no_run: requires a booted handle + an open file.
    /// ```
    pub fn write(&self, buf: &[u8]) -> Result<usize, SysError> {
        handle().file_write(self.fd, buf)
    }

    /// Reads up to `buf.len()` bytes; `0` at end-of-file.
    ///
    /// # Errors
    ///
    /// Returns [`SysError`] on a read failure.
    ///
    /// ```no_run
    /// // no_run: requires a booted handle + an open file.
    /// ```
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, SysError> {
        handle().fd_read(self.fd, buf)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        // Close through the handle; ignore the error (the fd may already be
        // invalid if a prior op closed it).
        let _ = handle().fd_close(self.fd);
    }
}

// SAFETY: `File` wraps a plain `i32` fd; it is not shared by value (no `Clone`)
// and the kernel serializes per-fd operations, so moving the struct across
// threads is sound — the same reasoning as the `net` wrappers.
unsafe impl Send for File {}

/// Reads up to `buf.len()` bytes from a raw `fd` the caller does NOT own, `0`
/// at end-of-file.
///
/// Used for process-owned fds (the tui's stdin) that must NOT be closed by this
/// crate — so this is a free function, not a [`File`] method.
///
/// # Errors
///
/// Returns [`SysError`] on a read failure.
///
/// ```no_run
/// // no_run: requires a booted handle + a live fd.
/// ```
pub fn read_fd(fd: i32, buf: &mut [u8]) -> Result<usize, SysError> {
    handle().fd_read(fd, buf)
}

/// Writes up to `buf.len()` bytes to a raw `fd` the caller does NOT own,
/// returning the count written (may be a short write).
///
/// Used for process-owned fds (the tui's stdout) that must NOT be closed by
/// this crate — so this is a free function, not a [`File`] method.
///
/// # Errors
///
/// Returns [`SysError`] on a write failure.
///
/// ```no_run
/// // no_run: requires a booted handle + a live fd.
/// ```
pub fn write_fd(fd: i32, buf: &[u8]) -> Result<usize, SysError> {
    handle().file_write(fd, buf)
}

/// Closes a raw `fd` the caller owns (e.g. one taken via [`File::into_raw_fd`]).
///
/// The counterpart to [`File::into_raw_fd`] for consumers that drive an fd
/// through a manual lifecycle rather than RAII.
///
/// # Errors
///
/// Returns [`SysError`] when the close fails (an already-closed fd).
///
/// ```no_run
/// // no_run: requires a booted handle + a live fd.
/// ```
pub fn close_fd(fd: i32) -> Result<(), SysError> {
    handle().fd_close(fd)
}
