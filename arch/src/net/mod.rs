//! Blocking Unix-domain socket wrappers.
//!
//! [`UnixListener`] binds a pathname socket, accepts connections, and
//! unlinks the socket path on `Drop`. [`UnixStream`] connects to a listener
//! and provides blocking `read`/`write`. Both are thin safe wrappers over
//! the raw socket syscalls in [`crate::sys`]; all `unsafe` lives in `sys/`.
//!
//! ## Design (pin-1 and rule of three)
//!
//! These are **blocking** primitives. The walking skeleton (Phase 3 of #797)
//! is consumer #1. A single skeleton transport does not yet demand readiness
//! multiplexing; `poll`/`epoll` are deferred until a second consumer needs
//! them (rule of three). Concurrency is achieved via
//! [`crate::thread::spawn`]: the server spawns one thread per accepted
//! connection.
//!
//! ## panic = "abort" and `Drop`
//!
//! `Drop` restores the socket and unlinks the path. Under `panic = "abort"`
//! the process terminates before `Drop` runs, so a panicking server leaves
//! the socket path behind. The pre-exit callback seam (gap-7, #797) will
//! address this for Phase 4; Phase 1 documents the gap in [`UnixListener`]'s
//! `Drop` comment.

// Syscall wrappers and constants used in the function bodies below.
// `EBADF`, `Errno`, `AF_UNIX`, `UNIX_PATH_MAX`, `SockaddrUn` are brought
// into scope by the `pub use` re-exports further down in this file.
use crate::sys::{
    AT_FDCWD, accept, bind, close, connect, listen, read, unix_stream_socket, unlinkat,
};

// ---- helpers -----------------------------------------------------------------

/// Fills a [`SockaddrUn`] from a NUL-terminated byte string, returning the
/// populated struct and its `addrlen`. Returns `None` when `path` is too long
/// (> `UNIX_PATH_MAX - 1` bytes before the NUL) or is empty.
fn make_sockaddr(path: &[u8]) -> Option<(SockaddrUn, usize)> {
    // path must include the NUL terminator; the non-NUL content must fit.
    let nul = path.iter().position(|&b| b == 0)?;
    if nul == 0 || nul >= UNIX_PATH_MAX {
        return None;
    }
    let mut sa = SockaddrUn::zeroed();
    sa.sun_family = AF_UNIX;
    sa.sun_path[..=nul].copy_from_slice(&path[..=nul]);
    let addrlen = sa.addrlen();
    Some((sa, addrlen))
}

// ---- UnixStream --------------------------------------------------------------

/// A connected Unix-domain byte-stream socket.
///
/// Obtained via [`UnixListener::accept`] (server side) or
/// [`UnixStream::connect`] (client side). The `Drop` implementation closes
/// the underlying file descriptor.
///
/// ```no_run
/// // A real connect requires a listening server — use the arch selftest
/// // integration smoke instead. This no_run example shows the API shape.
/// use reovim_arch::net::UnixStream;
///
/// // Connect to a server that is listening at this path.
/// let stream = UnixStream::connect(b"/tmp/reovim-doctest.sock\0")
///     .expect("connect to the server");
/// drop(stream); // closes the fd
/// ```
pub struct UnixStream {
    fd: i32,
}

impl UnixStream {
    /// Wraps an already-connected file descriptor.
    ///
    /// The caller transfers ownership; `Drop` will close `fd`.
    pub(crate) const fn from_fd(fd: i32) -> Self {
        Self { fd }
    }

    /// Returns the raw file descriptor.
    ///
    /// The returned fd is valid for the lifetime of the `UnixStream`. Using
    /// it after `Drop` is undefined behaviour at the OS level.
    ///
    /// ```rust
    /// // fd() is used internally by the server accept loop.
    /// // (No runnable example here — a real stream requires a live server.)
    /// ```
    #[must_use]
    pub const fn fd(&self) -> i32 {
        self.fd
    }

    /// Connects to a Unix-domain listener at `path` and returns a connected
    /// `UnixStream`.
    ///
    /// `path` must be a NUL-terminated byte string whose non-NUL prefix fits
    /// in 107 bytes (the kernel's `UNIX_PATH_MAX - 1` limit).
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on failure:
    /// - [`crate::sys::ENOENT`] — no socket file at `path`.
    /// - [`crate::sys::ECONNREFUSED`] — no listener at `path`.
    /// - [`crate::sys::EINVAL`] — `path` is empty, has no NUL, or is too long.
    ///
    /// ```rust
    /// // Connecting to a non-existent path returns ENOENT.
    /// use reovim_arch::net::UnixStream;
    /// use reovim_arch::sys::ENOENT;
    ///
    /// assert_eq!(
    ///     UnixStream::connect(b"/tmp/reovim-no-such-connect-doctest\0").err(),
    ///     Some(ENOENT),
    /// );
    /// ```
    pub fn connect(path: &[u8]) -> Result<Self, Errno> {
        let (sa, addrlen) = make_sockaddr(path).ok_or(crate::sys::EINVAL)?;
        let fd_raw = unix_stream_socket()?;
        // `fd_raw` is a fresh usize; it fits in i32 (a valid fd is < 2^31).
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let fd = fd_raw as i32;
        connect(fd, &sa, addrlen).inspect_err(|_| {
            let _ = close(fd);
        })?;
        Ok(Self { fd })
    }

    /// Reads up to `buf.len()` bytes from the stream.
    ///
    /// Returns the number of bytes placed in `buf`, or `0` at end-of-stream
    /// (peer closed the connection). The bytes beyond the returned count are
    /// uninitialised and must not be read.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on failure (e.g. [`crate::sys::EBADF`] if the fd
    /// was closed prematurely).
    ///
    /// ```no_run
    /// // A real stream requires a live connection — selftest covers this.
    /// // EBADF arm:
    /// use reovim_arch::net::UnixStream;
    /// // UnixStream::from_fd(-1) is crate-private; the EBADF arm is
    /// // covered by the arch selftest suite.
    /// ```
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, Errno> {
        read(self.fd, buf)
    }

    /// Writes `buf` to the stream.
    ///
    /// Returns the number of bytes written, which may be fewer than
    /// `buf.len()` (short write; the caller is responsible for looping).
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on failure.
    ///
    /// ```no_run
    /// // A real stream requires a live connection — selftest covers this.
    /// ```
    pub fn write(&self, buf: &[u8]) -> Result<usize, Errno> {
        // `send(MSG_NOSIGNAL)` rather than `write`: a peer that closed mid-
        // stream must surface as `EPIPE`, not a process-killing `SIGPIPE`.
        crate::sys::send_nosignal(self.fd, buf)
    }

    /// Writes all of `buf` to the stream, looping over short writes.
    ///
    /// Returns `Ok(())` when all bytes are written.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on the first write error, after which the number
    /// of bytes written is unspecified.
    ///
    /// ```no_run
    /// // A real stream requires a live connection — selftest covers this.
    /// ```
    pub fn write_all(&self, buf: &[u8]) -> Result<(), Errno> {
        let mut off = 0;
        while off < buf.len() {
            let n = crate::sys::send_nosignal(self.fd, &buf[off..])?;
            if n == 0 {
                return Err(EBADF); // peer closed
            }
            off += n;
        }
        Ok(())
    }

    /// Reads exactly `buf.len()` bytes from the stream, looping over short reads.
    ///
    /// Returns `Ok(())` when the buffer is full.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on the first read error.  Returns
    /// [`crate::sys::EBADF`] when the peer closes the connection before the
    /// buffer is full (EOF before `buf.len()` bytes).
    ///
    /// ```no_run
    /// // A real stream requires a live connection — selftest covers this.
    /// ```
    pub fn read_exact(&self, buf: &mut [u8]) -> Result<(), Errno> {
        let mut off = 0;
        while off < buf.len() {
            let n = read(self.fd, &mut buf[off..])?;
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
        // Close the fd; ignore the error (the fd may already be invalid if
        // the kernel closed it due to an error earlier in the connection).
        let _ = close(self.fd);
    }
}

// SAFETY: `UnixStream` wraps a plain `i32` fd; the fd is not shared by
// value (no `Clone`) and the kernel serialises per-fd operations, so moving
// the struct across threads is sound.
unsafe impl Send for UnixStream {}

// ---- UnixListener ------------------------------------------------------------

/// A bound and listening Unix-domain socket.
///
/// Created with [`UnixListener::bind`]. The `Drop` implementation closes the
/// listening fd and unlinks the socket path, so the path is removed whether
/// the server exits cleanly or via `drop`. Under `panic = "abort"` `Drop`
/// does not run — see the module-level note on the pre-exit callback seam.
///
/// ```no_run
/// // Binding requires filesystem access — selftest covers this end-to-end.
/// // The API shape:
/// use reovim_arch::net::UnixListener;
///
/// let listener = UnixListener::bind(b"/tmp/reovim-doctest-listener.sock\0")
///     .expect("bind+listen succeed");
/// // listener.accept() blocks until a client connects.
/// drop(listener); // closes fd and unlinks the path
/// ```
pub struct UnixListener {
    fd: i32,
    /// The NUL-terminated socket path, stored so `Drop` can unlink it.
    path: [u8; UNIX_PATH_MAX],
    /// Length of the path in `path` (up to and including the NUL).
    path_len: usize,
}

impl UnixListener {
    /// Binds a new socket to `path` and starts listening with a backlog of 8.
    ///
    /// On success the socket file is created at `path` and connections may be
    /// accepted. `Drop` will unlink the file.
    ///
    /// `path` must be a NUL-terminated byte string whose non-NUL prefix fits
    /// in 107 bytes.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on failure:
    /// - [`crate::sys::EADDRINUSE`] — the path already exists as a socket.
    /// - [`crate::sys::EINVAL`] — `path` is empty, has no NUL, or is too long.
    ///
    /// ```rust
    /// // Binding over an already-existing path returns EADDRINUSE (or ENOENT
    /// // if the path doesn't exist yet but the parent dir is missing — tested
    /// // in the arch selftest suite).
    /// use reovim_arch::net::UnixListener;
    /// use reovim_arch::sys::EINVAL;
    ///
    /// // An empty path slice has no NUL → EINVAL.
    /// assert_eq!(UnixListener::bind(b"").err(), Some(EINVAL));
    /// ```
    pub fn bind(path: &[u8]) -> Result<Self, Errno> {
        let (sa, addrlen) = make_sockaddr(path).ok_or(crate::sys::EINVAL)?;
        let fd_raw = unix_stream_socket()?;
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let fd = fd_raw as i32;

        // bind + listen; on any error close the socket before returning.
        if let Err(e) = bind(fd, &sa, addrlen) {
            let _ = close(fd);
            return Err(e);
        }
        if let Err(e) = listen(fd, 8) {
            let _ = close(fd);
            // Best-effort unlink if we managed to bind.
            let _ = unlinkat(AT_FDCWD, path, 0);
            return Err(e);
        }

        // Store the path for `Drop`.
        let mut stored = [0u8; UNIX_PATH_MAX];
        let nul = path.iter().position(|&b| b == 0).unwrap_or(path.len());
        let copy_len = (nul + 1).min(UNIX_PATH_MAX);
        stored[..copy_len].copy_from_slice(&path[..copy_len]);

        Ok(Self {
            fd,
            path: stored,
            path_len: copy_len,
        })
    }

    /// Returns the raw file descriptor of the listening socket.
    ///
    /// ```no_run
    /// // fd() used internally; no standalone runnable example.
    /// ```
    #[must_use]
    pub const fn fd(&self) -> i32 {
        self.fd
    }

    /// Accepts one pending connection, returning a [`UnixStream`].
    ///
    /// Blocks until a client connects. The accepted stream is a fresh
    /// connected socket the caller owns.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] on failure (e.g. [`crate::sys::EBADF`] if the
    /// listener fd was closed, [`crate::sys::EINVAL`] if the socket is
    /// not listening).
    ///
    /// ```rust
    /// // Accepting without a client returns EBADF on a closed fd:
    /// use reovim_arch::sys::EBADF;
    /// use reovim_arch::sys::{accept};
    /// assert_eq!(accept(-1), Err(EBADF));
    /// ```
    pub fn accept(&self) -> Result<UnixStream, Errno> {
        let fd_raw = accept(self.fd)?;
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        Ok(UnixStream::from_fd(fd_raw as i32))
    }
}

impl Drop for UnixListener {
    /// Closes the listening fd and unlinks the socket path.
    ///
    /// The unlink is best-effort: errors are silently ignored (the file may
    /// already have been removed, or the process may have lost the path).
    ///
    /// Note: under `panic = "abort"` (the workspace `panic` profile) `Drop`
    /// does not run on a panic. The pre-exit callback seam (gap-7, #797,
    /// `arch::panic::register_pre_exit_hook`) addresses this; it lands before
    /// Phase 4 of the walking skeleton.
    fn drop(&mut self) {
        let _ = close(self.fd);
        // Build the NUL-terminated path slice from the stored bytes.
        let _ = unlinkat(AT_FDCWD, &self.path[..self.path_len], 0);
    }
}

// SAFETY: `UnixListener` wraps a plain `i32` fd and a byte array; no shared
// mutable state. Moving across threads is sound.
unsafe impl Send for UnixListener {}

// ---- re-export the errno constants the arch::net public API surface uses ----
// (so callers can `use reovim_arch::net::*; ... Err(ENOENT)` without a
// separate `crate::sys` import).
pub use crate::sys::{EADDRINUSE, EBADF, ECONNREFUSED, ENOENT, EOPNOTSUPP, Errno};

// L12 layout (#797 Phase 1): tests live in the sibling file `net_tests.rs`,
// declared in `arch/src/lib.rs` under `#[cfg(feature = "selftest")]`.
#[cfg(feature = "selftest")]
#[path = "net_tests.rs"]
mod tests;

// Export the AF_UNIX / SOCK_STREAM constants so callers using `arch::net`
// do not need to reach into `arch::sys::net`.
pub use crate::sys::net::{AF_UNIX, SOCK_CLOEXEC, SOCK_STREAM, SockaddrUn, UNIX_PATH_MAX};
