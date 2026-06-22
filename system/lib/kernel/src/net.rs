//! Local networking bridge from up-face controls to the platform handle.
//!
//! This module owns the current Unix-domain socket mapping to
//! `kabi/platform`. Upper layers receive `uapi/net` controls; `lib/ds` does
//! not carry socket wrappers.

use {
    reovim_kabi_platform::{Errno, handle},
    reovim_uapi_net::{NetControl, NetError, SocketHandle},
};

/// Returns the local networking control table backed by the platform handle.
///
/// Composition roots pass this value into runtimes so upper code can use UDS
/// transport without importing lower socket helpers.
///
/// ```rust,no_run
/// use reovim_system_kernel::net::net_control;
/// use reovim_uapi_net::UnixStream;
///
/// let net = net_control();
/// let _ = UnixStream::connect(net, b"/tmp/reovim.sock\0");
/// ```
#[must_use]
pub const fn net_control() -> NetControl {
    NetControl::new(connect_unix, listen_unix, accept_unix, read_socket, write_socket, close_socket)
}

fn connect_unix(path: &[u8]) -> Result<SocketHandle, NetError> {
    handle()
        .unix_connect(path)
        .map(SocketHandle::new)
        .map_err(map_net_error)
}

fn listen_unix(path: &[u8]) -> Result<SocketHandle, NetError> {
    handle()
        .unix_listen(path)
        .map(SocketHandle::new)
        .map_err(map_net_error)
}

fn accept_unix(listener: SocketHandle) -> Result<SocketHandle, NetError> {
    handle()
        .unix_accept(listener.raw())
        .map(SocketHandle::new)
        .map_err(map_net_error)
}

fn read_socket(handle_id: SocketHandle, buf: &mut [u8]) -> Result<usize, NetError> {
    handle()
        .fd_read(handle_id.raw(), buf)
        .map_err(map_net_error)
}

fn write_socket(handle_id: SocketHandle, buf: &[u8]) -> Result<usize, NetError> {
    handle()
        .fd_write(handle_id.raw(), buf)
        .map_err(map_net_error)
}

fn close_socket(handle_id: SocketHandle) -> Result<(), NetError> {
    handle().fd_close(handle_id.raw()).map_err(map_net_error)
}

fn map_net_error(error: Errno) -> NetError {
    NetError::new(error.code())
}
