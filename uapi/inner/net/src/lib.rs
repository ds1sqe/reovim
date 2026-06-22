//! Product-facing local networking and IPC control vocabulary.
//!
//! Upper layers receive these function tables from composition roots. Concrete
//! implementations live below the system-kernel bridge.

#![no_std]

/// Product-facing local socket error.
///
/// The numeric code is diagnostic bridge/provider detail. It is not permission
/// for upper layers to call POSIX directly.
///
/// ```rust
/// use reovim_uapi_net::NetError;
///
/// assert_eq!(NetError::new(9).code(), 9);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NetError(i32);

impl NetError {
    /// Builds a network error from a bridge/provider code.
    ///
    /// ```rust
    /// use reovim_uapi_net::NetError;
    ///
    /// assert_eq!(NetError::new(2).code(), 2);
    /// ```
    #[must_use]
    pub const fn new(code: i32) -> Self {
        Self(code)
    }

    /// The generic unsupported-operation sentinel used by no-op controls.
    ///
    /// ```rust
    /// use reovim_uapi_net::NetError;
    ///
    /// assert_eq!(NetError::unsupported().code(), 38);
    /// ```
    #[must_use]
    pub const fn unsupported() -> Self {
        Self(38)
    }

    /// Returns the diagnostic bridge/provider code.
    ///
    /// ```rust
    /// use reovim_uapi_net::NetError;
    ///
    /// assert_eq!(NetError::new(13).code(), 13);
    /// ```
    #[must_use]
    pub const fn code(self) -> i32 {
        self.0
    }
}

/// "Bad file descriptor" / peer-closed sentinel.
///
/// ```rust
/// use reovim_uapi_net::{EBADF, NetError};
///
/// assert_eq!(EBADF, NetError::new(9));
/// ```
pub const EBADF: NetError = NetError::new(9);

/// "No such file or directory" sentinel.
///
/// ```rust
/// use reovim_uapi_net::{ENOENT, NetError};
///
/// assert_eq!(ENOENT, NetError::new(2));
/// ```
pub const ENOENT: NetError = NetError::new(2);

/// Product-facing local socket handle.
///
/// The bridge owns interpretation of the integer. Current implementations map
/// it to a kernel file descriptor, but upper layers treat it as an opaque
/// socket handle.
///
/// ```rust
/// use reovim_uapi_net::SocketHandle;
///
/// assert_eq!(SocketHandle::new(7).raw(), 7);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SocketHandle(i32);

impl SocketHandle {
    /// Builds an opaque socket handle from a bridge-issued value.
    ///
    /// ```rust
    /// use reovim_uapi_net::SocketHandle;
    ///
    /// assert_eq!(SocketHandle::new(3).raw(), 3);
    /// ```
    #[must_use]
    pub const fn new(raw: i32) -> Self {
        Self(raw)
    }

    /// Returns the bridge-owned handle value.
    ///
    /// ```rust
    /// use reovim_uapi_net::SocketHandle;
    ///
    /// assert_eq!(SocketHandle::new(4).raw(), 4);
    /// ```
    #[must_use]
    pub const fn raw(self) -> i32 {
        self.0
    }
}

/// Function pointer for connecting to a Unix-domain socket.
///
/// ```rust
/// use reovim_uapi_net::{ConnectUnixFn, NetError, SocketHandle};
///
/// fn connect(_: &[u8]) -> Result<SocketHandle, NetError> { Ok(SocketHandle::new(1)) }
/// let f: ConnectUnixFn = connect;
/// assert_eq!(f(b"/tmp/reovim.sock\0").unwrap().raw(), 1);
/// ```
pub type ConnectUnixFn = fn(&[u8]) -> Result<SocketHandle, NetError>;

/// Function pointer for binding a Unix-domain listener.
///
/// ```rust
/// use reovim_uapi_net::{ListenUnixFn, NetError, SocketHandle};
///
/// fn listen(_: &[u8]) -> Result<SocketHandle, NetError> { Ok(SocketHandle::new(2)) }
/// let f: ListenUnixFn = listen;
/// assert_eq!(f(b"/tmp/reovim.sock\0").unwrap().raw(), 2);
/// ```
pub type ListenUnixFn = fn(&[u8]) -> Result<SocketHandle, NetError>;

/// Function pointer for accepting a Unix-domain connection.
///
/// ```rust
/// use reovim_uapi_net::{AcceptUnixFn, NetError, SocketHandle};
///
/// fn accept(_: SocketHandle) -> Result<SocketHandle, NetError> { Ok(SocketHandle::new(3)) }
/// let f: AcceptUnixFn = accept;
/// assert_eq!(f(SocketHandle::new(2)).unwrap().raw(), 3);
/// ```
pub type AcceptUnixFn = fn(SocketHandle) -> Result<SocketHandle, NetError>;

/// Function pointer for reading from a local socket.
///
/// ```rust
/// use reovim_uapi_net::{NetError, ReadSocketFn, SocketHandle};
///
/// fn read(_: SocketHandle, _: &mut [u8]) -> Result<usize, NetError> { Ok(0) }
/// let f: ReadSocketFn = read;
/// assert_eq!(f(SocketHandle::new(1), &mut [0; 1]), Ok(0));
/// ```
pub type ReadSocketFn = fn(SocketHandle, &mut [u8]) -> Result<usize, NetError>;

/// Function pointer for writing to a local socket.
///
/// ```rust
/// use reovim_uapi_net::{NetError, SocketHandle, WriteSocketFn};
///
/// fn write(_: SocketHandle, buf: &[u8]) -> Result<usize, NetError> { Ok(buf.len()) }
/// let f: WriteSocketFn = write;
/// assert_eq!(f(SocketHandle::new(1), b"ok"), Ok(2));
/// ```
pub type WriteSocketFn = fn(SocketHandle, &[u8]) -> Result<usize, NetError>;

/// Function pointer for closing a local socket handle.
///
/// ```rust
/// use reovim_uapi_net::{CloseSocketFn, NetError, SocketHandle};
///
/// fn close(_: SocketHandle) -> Result<(), NetError> { Ok(()) }
/// let f: CloseSocketFn = close;
/// assert_eq!(f(SocketHandle::new(1)), Ok(()));
/// ```
pub type CloseSocketFn = fn(SocketHandle) -> Result<(), NetError>;

/// Product-facing local networking control table.
///
/// ```rust
/// use reovim_uapi_net::{NetControl, NetError, SocketHandle};
///
/// fn connect(_: &[u8]) -> Result<SocketHandle, NetError> { Ok(SocketHandle::new(1)) }
/// fn listen(_: &[u8]) -> Result<SocketHandle, NetError> { Ok(SocketHandle::new(2)) }
/// fn accept(_: SocketHandle) -> Result<SocketHandle, NetError> { Ok(SocketHandle::new(3)) }
/// fn read(_: SocketHandle, _: &mut [u8]) -> Result<usize, NetError> { Ok(0) }
/// fn write(_: SocketHandle, buf: &[u8]) -> Result<usize, NetError> { Ok(buf.len()) }
/// fn close(_: SocketHandle) -> Result<(), NetError> { Ok(()) }
///
/// let net = NetControl::new(connect, listen, accept, read, write, close);
/// assert_eq!(net.connect_unix(b"/tmp/reovim.sock\0").unwrap().raw(), 1);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct NetControl {
    /// Connects to a Unix-domain socket path.
    pub connect_unix_fn: ConnectUnixFn,
    /// Binds/listens on a Unix-domain socket path.
    pub listen_unix_fn: ListenUnixFn,
    /// Accepts one connection from a listener handle.
    pub accept_unix_fn: AcceptUnixFn,
    /// Reads from a socket handle.
    pub read_fn: ReadSocketFn,
    /// Writes to a socket handle.
    pub write_fn: WriteSocketFn,
    /// Closes a socket handle.
    pub close_fn: CloseSocketFn,
}

impl NetControl {
    /// Creates a no-op networking control table.
    ///
    /// ```rust
    /// use reovim_uapi_net::{NetControl, NetError};
    ///
    /// assert_eq!(
    ///     NetControl::noop().connect_unix(b"/tmp/reovim.sock\0"),
    ///     Err(NetError::unsupported())
    /// );
    /// ```
    #[must_use]
    pub const fn noop() -> Self {
        Self::new(noop_connect, noop_listen, noop_accept, noop_read, noop_write, noop_close)
    }

    /// Creates a networking control table.
    ///
    /// ```rust
    /// use reovim_uapi_net::{NetControl, NetError, SocketHandle};
    ///
    /// fn connect(_: &[u8]) -> Result<SocketHandle, NetError> { Ok(SocketHandle::new(1)) }
    /// fn listen(_: &[u8]) -> Result<SocketHandle, NetError> { Ok(SocketHandle::new(2)) }
    /// fn accept(_: SocketHandle) -> Result<SocketHandle, NetError> { Ok(SocketHandle::new(3)) }
    /// fn read(_: SocketHandle, _: &mut [u8]) -> Result<usize, NetError> { Ok(0) }
    /// fn write(_: SocketHandle, buf: &[u8]) -> Result<usize, NetError> { Ok(buf.len()) }
    /// fn close(_: SocketHandle) -> Result<(), NetError> { Ok(()) }
    ///
    /// let _net = NetControl::new(connect, listen, accept, read, write, close);
    /// ```
    #[must_use]
    pub const fn new(
        connect_unix_fn: ConnectUnixFn,
        listen_unix_fn: ListenUnixFn,
        accept_unix_fn: AcceptUnixFn,
        read_fn: ReadSocketFn,
        write_fn: WriteSocketFn,
        close_fn: CloseSocketFn,
    ) -> Self {
        Self {
            connect_unix_fn,
            listen_unix_fn,
            accept_unix_fn,
            read_fn,
            write_fn,
            close_fn,
        }
    }

    /// Connects to a Unix-domain socket.
    ///
    /// ```rust
    /// use reovim_uapi_net::{NetControl, NetError};
    ///
    /// assert_eq!(
    ///     NetControl::noop().connect_unix(b"/tmp/reovim.sock\0"),
    ///     Err(NetError::unsupported())
    /// );
    /// ```
    pub fn connect_unix(self, path: &[u8]) -> Result<SocketHandle, NetError> {
        (self.connect_unix_fn)(path)
    }

    /// Binds/listens on a Unix-domain socket.
    ///
    /// ```rust
    /// use reovim_uapi_net::{NetControl, NetError};
    ///
    /// assert_eq!(
    ///     NetControl::noop().listen_unix(b"/tmp/reovim.sock\0"),
    ///     Err(NetError::unsupported())
    /// );
    /// ```
    pub fn listen_unix(self, path: &[u8]) -> Result<SocketHandle, NetError> {
        (self.listen_unix_fn)(path)
    }

    /// Accepts a Unix-domain connection.
    ///
    /// ```rust
    /// use reovim_uapi_net::{NetControl, NetError, SocketHandle};
    ///
    /// assert_eq!(
    ///     NetControl::noop().accept_unix(SocketHandle::new(1)),
    ///     Err(NetError::unsupported())
    /// );
    /// ```
    pub fn accept_unix(self, listener: SocketHandle) -> Result<SocketHandle, NetError> {
        (self.accept_unix_fn)(listener)
    }

    /// Reads from a socket handle.
    ///
    /// ```rust
    /// use reovim_uapi_net::{NetControl, SocketHandle};
    ///
    /// assert_eq!(NetControl::noop().read(SocketHandle::new(1), &mut [0; 1]), Ok(0));
    /// ```
    pub fn read(self, handle: SocketHandle, buf: &mut [u8]) -> Result<usize, NetError> {
        (self.read_fn)(handle, buf)
    }

    /// Writes to a socket handle.
    ///
    /// ```rust
    /// use reovim_uapi_net::{NetControl, NetError, SocketHandle};
    ///
    /// assert_eq!(
    ///     NetControl::noop().write(SocketHandle::new(1), b"ok"),
    ///     Err(NetError::unsupported())
    /// );
    /// ```
    pub fn write(self, handle: SocketHandle, buf: &[u8]) -> Result<usize, NetError> {
        (self.write_fn)(handle, buf)
    }

    /// Closes a socket handle.
    ///
    /// ```rust
    /// use reovim_uapi_net::{NetControl, SocketHandle};
    ///
    /// assert_eq!(NetControl::noop().close(SocketHandle::new(1)), Ok(()));
    /// ```
    pub fn close(self, handle: SocketHandle) -> Result<(), NetError> {
        (self.close_fn)(handle)
    }
}

impl Default for NetControl {
    fn default() -> Self {
        Self::noop()
    }
}

/// A connected Unix-domain byte-stream socket.
///
/// The value owns the underlying socket handle and closes it through its
/// control table on drop.
///
/// ```rust,no_run
/// use reovim_uapi_net::{NetControl, UnixStream};
///
/// let stream = UnixStream::connect(NetControl::noop(), b"/tmp/reovim.sock\0");
/// let _ = stream;
/// ```
pub struct UnixStream {
    handle: SocketHandle,
    net: NetControl,
}

impl UnixStream {
    fn from_handle(net: NetControl, handle: SocketHandle) -> Self {
        Self { handle, net }
    }

    /// Connects to a Unix-domain listener at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`NetError`] when the injected networking service refuses the
    /// connection.
    ///
    /// ```rust,no_run
    /// use reovim_uapi_net::{NetControl, UnixStream};
    ///
    /// let _ = UnixStream::connect(NetControl::noop(), b"/tmp/reovim.sock\0");
    /// ```
    pub fn connect(net: NetControl, path: &[u8]) -> Result<Self, NetError> {
        let handle = net.connect_unix(path)?;
        Ok(Self::from_handle(net, handle))
    }

    /// Reads up to `buf.len()` bytes from the stream.
    ///
    /// # Errors
    ///
    /// Returns [`NetError`] when the injected networking service reports a
    /// read failure.
    ///
    /// ```rust,no_run
    /// use reovim_uapi_net::{NetControl, UnixStream};
    ///
    /// let mut buf = [0u8; 1];
    /// if let Ok(stream) = UnixStream::connect(NetControl::noop(), b"/tmp/x\0") {
    ///     let _ = stream.read(&mut buf);
    /// }
    /// ```
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, NetError> {
        self.net.read(self.handle, buf)
    }

    /// Writes `buf` to the stream.
    ///
    /// # Errors
    ///
    /// Returns [`NetError`] when the injected networking service reports a
    /// write failure.
    ///
    /// ```rust,no_run
    /// use reovim_uapi_net::{NetControl, UnixStream};
    ///
    /// if let Ok(stream) = UnixStream::connect(NetControl::noop(), b"/tmp/x\0") {
    ///     let _ = stream.write(b"x");
    /// }
    /// ```
    pub fn write(&self, buf: &[u8]) -> Result<usize, NetError> {
        self.net.write(self.handle, buf)
    }

    /// Writes all bytes in `buf`, looping over short writes.
    ///
    /// # Errors
    ///
    /// Returns [`NetError`] on the first write failure, or [`EBADF`] when the
    /// service reports a zero-byte write before completion.
    ///
    /// ```rust,no_run
    /// use reovim_uapi_net::{NetControl, UnixStream};
    ///
    /// if let Ok(stream) = UnixStream::connect(NetControl::noop(), b"/tmp/x\0") {
    ///     let _ = stream.write_all(b"x");
    /// }
    /// ```
    pub fn write_all(&self, buf: &[u8]) -> Result<(), NetError> {
        let mut off = 0;
        while off < buf.len() {
            let n = self.write(&buf[off..])?;
            if n == 0 {
                return Err(EBADF);
            }
            off += n;
        }
        Ok(())
    }

    /// Reads exactly `buf.len()` bytes, looping over short reads.
    ///
    /// # Errors
    ///
    /// Returns [`NetError`] on the first read failure, or [`EBADF`] when the
    /// peer closes before the buffer is full.
    ///
    /// ```rust,no_run
    /// use reovim_uapi_net::{NetControl, UnixStream};
    ///
    /// let mut buf = [0u8; 1];
    /// if let Ok(stream) = UnixStream::connect(NetControl::noop(), b"/tmp/x\0") {
    ///     let _ = stream.read_exact(&mut buf);
    /// }
    /// ```
    pub fn read_exact(&self, buf: &mut [u8]) -> Result<(), NetError> {
        let mut off = 0;
        while off < buf.len() {
            let n = self.read(&mut buf[off..])?;
            if n == 0 {
                return Err(EBADF);
            }
            off += n;
        }
        Ok(())
    }
}

impl Drop for UnixStream {
    fn drop(&mut self) {
        let _ = self.net.close(self.handle);
    }
}

/// A bound Unix-domain listener.
///
/// The value owns the underlying listener handle and closes it through its
/// control table on drop.
///
/// ```rust,no_run
/// use reovim_uapi_net::{NetControl, UnixListener};
///
/// let listener = UnixListener::bind(NetControl::noop(), b"/tmp/reovim.sock\0");
/// let _ = listener;
/// ```
pub struct UnixListener {
    handle: SocketHandle,
    net: NetControl,
}

impl UnixListener {
    /// Binds/listens on a Unix-domain socket path.
    ///
    /// # Errors
    ///
    /// Returns [`NetError`] when the injected networking service refuses the
    /// bind/listen request.
    ///
    /// ```rust,no_run
    /// use reovim_uapi_net::{NetControl, UnixListener};
    ///
    /// let _ = UnixListener::bind(NetControl::noop(), b"/tmp/reovim.sock\0");
    /// ```
    pub fn bind(net: NetControl, path: &[u8]) -> Result<Self, NetError> {
        let handle = net.listen_unix(path)?;
        Ok(Self { handle, net })
    }

    /// Accepts one pending connection.
    ///
    /// # Errors
    ///
    /// Returns [`NetError`] when the injected networking service reports an
    /// accept failure.
    ///
    /// ```rust,no_run
    /// use reovim_uapi_net::{NetControl, UnixListener};
    ///
    /// if let Ok(listener) = UnixListener::bind(NetControl::noop(), b"/tmp/x\0") {
    ///     let _ = listener.accept();
    /// }
    /// ```
    pub fn accept(&self) -> Result<UnixStream, NetError> {
        let handle = self.net.accept_unix(self.handle)?;
        Ok(UnixStream::from_handle(self.net, handle))
    }
}

impl Drop for UnixListener {
    fn drop(&mut self) {
        let _ = self.net.close(self.handle);
    }
}

fn noop_connect(_: &[u8]) -> Result<SocketHandle, NetError> {
    Err(NetError::unsupported())
}

fn noop_listen(_: &[u8]) -> Result<SocketHandle, NetError> {
    Err(NetError::unsupported())
}

fn noop_accept(_: SocketHandle) -> Result<SocketHandle, NetError> {
    Err(NetError::unsupported())
}

fn noop_read(_: SocketHandle, _: &mut [u8]) -> Result<usize, NetError> {
    Ok(0)
}

fn noop_write(_: SocketHandle, _: &[u8]) -> Result<usize, NetError> {
    Err(NetError::unsupported())
}

fn noop_close(_: SocketHandle) -> Result<(), NetError> {
    Ok(())
}
