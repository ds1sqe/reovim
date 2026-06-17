//! Unix-domain socket Math wrappers over the kabi platform handle's fd-op
//! primitives.
//!
//! [`UnixStream`] and [`UnixListener`] hold an opaque `i32` fd and reach their
//! connect/read/write/bind/accept/close backend through the boot-installed
//! `kabi` handle — never through `arch` directly. This is the same pattern as
//! the `lib/ds` [`Mutex`](crate::Mutex) over the handle's `park`/`unpark`: the
//! algorithm (framing, short-read/short-write loops, fd ownership) is portable
//! Math; only the injected fd-op backend is World (master invariant 2:
//! `lib/ds ⊄ arch`).
//!
//! ## Blocking, thread-per-connection (pin-1, rule of three)
//!
//! These are blocking primitives. The walking-skeleton server spawns one thread
//! per accepted connection (see [`crate::thread`]); `poll`/`epoll` readiness
//! multiplexing is deferred until a second consumer needs it.
//!
//! ## `Drop` and `panic = "abort"`
//!
//! `Drop` closes the fd through the handle. Under the workspace `panic = "abort"`
//! profile a panic terminates the process before `Drop` runs, so a panicking
//! server leaks its listener fd to process teardown — the listen primitive
//! pre-unlinks any stale socket path before bind, so a restart still succeeds.

use reovim_kabi_platform::handle;
pub use reovim_uapi_posix::Errno;

/// "Bad file descriptor" / peer-closed sentinel (`EBADF`, Linux errno 9).
///
/// The carrier loops treat an `EBADF` on a zero-byte read/write as a clean
/// peer-closed disconnect (the same convention the prior `arch::net` surface
/// used).
///
/// ```rust
/// use reovim_lib_ds::net::{EBADF, Errno};
///
/// assert_eq!(EBADF, Errno::from_code(9));
/// assert_eq!(EBADF.code(), 9);
/// ```
pub const EBADF: Errno = Errno::from_code(9);

/// "No such file or directory" (`ENOENT`, Linux errno 2).
///
/// E.g. a connect to a path with no socket file. Provided so consumers can
/// distinguish a real
/// connect failure from the [`EBADF`] peer-closed sentinel.
///
/// ```rust
/// use reovim_lib_ds::net::{EBADF, ENOENT, Errno};
///
/// assert_eq!(ENOENT, Errno::from_code(2));
/// assert_ne!(ENOENT, EBADF);
/// ```
pub const ENOENT: Errno = Errno::from_code(2);

/// A connected Unix-domain byte-stream socket.
///
/// Obtained via [`UnixListener::accept`] (server side) or [`UnixStream::connect`]
/// (client side). `Drop` closes the underlying fd through the handle.
///
/// ```no_run
/// // no_run: a real connect requires a booted handle + a listening server.
/// use reovim_lib_ds::net::UnixStream;
///
/// let stream = UnixStream::connect(b"/tmp/reovim-doctest.sock\0")
///     .expect("connect to the server");
/// drop(stream); // closes the fd through the handle
/// ```
pub struct UnixStream {
    fd: i32,
}

impl UnixStream {
    /// Wraps an already-connected fd (transfers ownership; `Drop` closes it).
    const fn from_fd(fd: i32) -> Self {
        Self { fd }
    }

    /// The raw fd. Valid for the lifetime of the `UnixStream`.
    ///
    /// ```no_run
    /// // fd() is used internally by the server accept loop; no standalone
    /// // runnable example (a real stream needs a booted handle + server).
    /// ```
    #[must_use]
    pub const fn fd(&self) -> i32 {
        self.fd
    }

    /// Connects to a Unix-domain listener at `path` (a NUL-terminated byte
    /// string) through the handle, returning a connected `UnixStream`.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on failure (no listener, missing path, malformed
    /// address).
    ///
    /// ```no_run
    /// // no_run: requires a booted handle + a listening server.
    /// use reovim_lib_ds::net::UnixStream;
    ///
    /// let _ = UnixStream::connect(b"/tmp/reovim.sock\0");
    /// ```
    pub fn connect(path: &[u8]) -> Result<Self, Errno> {
        let fd = handle().unix_connect(path)?;
        Ok(Self::from_fd(fd))
    }

    /// Reads up to `buf.len()` bytes; `0` at end-of-stream.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on a read failure.
    ///
    /// ```no_run
    /// // no_run: requires a live connection through a booted handle.
    /// ```
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, Errno> {
        handle().fd_read(self.fd, buf)
    }

    /// Writes `buf`; returns the count written (may be a short write).
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on a write failure.
    ///
    /// ```no_run
    /// // no_run: requires a live connection through a booted handle.
    /// ```
    pub fn write(&self, buf: &[u8]) -> Result<usize, Errno> {
        handle().fd_write(self.fd, buf)
    }

    /// Writes all of `buf`, looping over short writes.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on the first write error; [`EBADF`] when the peer
    /// closed mid-stream (a zero-byte write).
    ///
    /// ```no_run
    /// // no_run: requires a live connection through a booted handle.
    /// ```
    pub fn write_all(&self, buf: &[u8]) -> Result<(), Errno> {
        let mut off = 0;
        while off < buf.len() {
            let n = handle().fd_write(self.fd, &buf[off..])?;
            if n == 0 {
                return Err(EBADF); // peer closed
            }
            off += n;
        }
        Ok(())
    }

    /// Reads exactly `buf.len()` bytes, looping over short reads.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on the first read error; [`EBADF`] when the peer
    /// closes before the buffer is full (EOF before `buf.len()` bytes).
    ///
    /// ```no_run
    /// // no_run: requires a live connection through a booted handle.
    /// ```
    pub fn read_exact(&self, buf: &mut [u8]) -> Result<(), Errno> {
        let mut off = 0;
        while off < buf.len() {
            let n = handle().fd_read(self.fd, &mut buf[off..])?;
            if n == 0 {
                return Err(EBADF); // EOF before buffer full
            }
            off += n;
        }
        Ok(())
    }
}

impl Drop for UnixStream {
    fn drop(&mut self) {
        // Close through the handle; ignore the error (the fd may already be
        // invalid if the kernel closed it on an earlier connection error).
        let _ = handle().fd_close(self.fd);
    }
}

// SAFETY: `UnixStream` wraps a plain `i32` fd; it is not shared by value (no
// `Clone`) and the kernel serializes per-fd operations, so moving the struct
// across threads is sound — the same reasoning as the prior `arch::net` wrapper.
unsafe impl Send for UnixStream {}

/// A bound and listening Unix-domain socket.
///
/// Created with [`UnixListener::bind`], which binds + listens through the
/// handle. `Drop` closes the listening fd through the handle. The handle's
/// listen primitive pre-unlinks a stale socket file before bind, so a restart
/// over the same path succeeds even when a prior `Drop` did not run (the
/// `panic = "abort"` case).
///
/// ```no_run
/// // no_run: binding requires a booted handle + filesystem access.
/// use reovim_lib_ds::net::UnixListener;
///
/// let listener = UnixListener::bind(b"/tmp/reovim-doctest-listener.sock\0")
///     .expect("bind+listen succeed");
/// // listener.accept() blocks until a client connects.
/// drop(listener); // closes the fd through the handle
/// ```
pub struct UnixListener {
    fd: i32,
}

impl UnixListener {
    /// Binds + listens a Unix-domain socket at `path` (NUL-terminated) through
    /// the handle (backlog 8).
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on failure (malformed path, permission denied).
    ///
    /// ```no_run
    /// // no_run: requires a booted handle + filesystem access.
    /// use reovim_lib_ds::net::UnixListener;
    ///
    /// let _ = UnixListener::bind(b"/tmp/reovim.sock\0");
    /// ```
    pub fn bind(path: &[u8]) -> Result<Self, Errno> {
        let fd = handle().unix_listen(path)?;
        Ok(Self { fd })
    }

    /// The raw fd of the listening socket.
    ///
    /// ```no_run
    /// // fd() used internally; no standalone runnable example.
    /// ```
    #[must_use]
    pub const fn fd(&self) -> i32 {
        self.fd
    }

    /// Accepts one pending connection through the handle, returning a connected
    /// [`UnixStream`]. Blocks until a client connects.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on failure (the listener fd was closed, the socket is
    /// not listening).
    ///
    /// ```no_run
    /// // no_run: requires a booted handle + a connecting client.
    /// ```
    pub fn accept(&self) -> Result<UnixStream, Errno> {
        let fd = handle().unix_accept(self.fd)?;
        Ok(UnixStream::from_fd(fd))
    }
}

impl Drop for UnixListener {
    fn drop(&mut self) {
        // Close the listening fd through the handle. The socket-path unlink is
        // owned by the listen primitive's pre-bind cleanup (not a Drop unlink),
        // because under `panic = "abort"` this Drop may not run at all.
        let _ = handle().fd_close(self.fd);
    }
}

// SAFETY: `UnixListener` wraps a plain `i32` fd; no shared mutable state, so
// moving across threads is sound (as the prior `arch::net` wrapper).
unsafe impl Send for UnixListener {}
