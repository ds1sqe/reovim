use reovim_uapi_net::{NetControl, NetError, SocketHandle, UnixListener, UnixStream};

fn connect(_: &[u8]) -> Result<SocketHandle, NetError> {
    Ok(SocketHandle::new(7))
}

fn listen(_: &[u8]) -> Result<SocketHandle, NetError> {
    Ok(SocketHandle::new(8))
}

fn accept(listener: SocketHandle) -> Result<SocketHandle, NetError> {
    assert_eq!(listener.raw(), 8);
    Ok(SocketHandle::new(9))
}

fn read(handle: SocketHandle, buf: &mut [u8]) -> Result<usize, NetError> {
    assert!(matches!(handle.raw(), 7 | 9));
    buf[0] = b'x';
    Ok(1)
}

fn write(handle: SocketHandle, buf: &[u8]) -> Result<usize, NetError> {
    assert!(matches!(handle.raw(), 7 | 9));
    Ok(buf.len())
}

fn close(_: SocketHandle) -> Result<(), NetError> {
    Ok(())
}

#[test]
fn net_control_dispatches_stream_ops() {
    let net = NetControl::new(connect, listen, accept, read, write, close);
    let stream = UnixStream::connect(net, b"/tmp/reovim.sock\0").expect("connect");
    let mut buf = [0u8; 1];

    assert_eq!(stream.read(&mut buf), Ok(1));
    assert_eq!(buf[0], b'x');
    assert_eq!(stream.write(b"ok"), Ok(2));
    assert_eq!(stream.write_all(b"ok"), Ok(()));
}

#[test]
fn net_control_dispatches_listener_accept() {
    let net = NetControl::new(connect, listen, accept, read, write, close);
    let listener = UnixListener::bind(net, b"/tmp/reovim.sock\0").expect("bind");
    let stream = listener.accept().expect("accept");
    let mut buf = [0u8; 1];

    assert_eq!(stream.read_exact(&mut buf), Ok(()));
    assert_eq!(buf[0], b'x');
}

#[test]
fn noop_control_is_stable() {
    let net = NetControl::noop();

    assert_eq!(net.connect_unix(b"/tmp/reovim.sock\0"), Err(NetError::unsupported()));
    assert_eq!(net.listen_unix(b"/tmp/reovim.sock\0"), Err(NetError::unsupported()));
    assert_eq!(net.read(SocketHandle::new(1), &mut [0; 1]), Ok(0));
    assert_eq!(net.write(SocketHandle::new(1), b"ok"), Err(NetError::unsupported()));
    assert_eq!(net.close(SocketHandle::new(1)), Ok(()));
}
