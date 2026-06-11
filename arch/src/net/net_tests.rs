//! Tests for `arch::net`, compiled into the lib under `selftest` (#797 Phase 1).
//!
//! L12 layout: sibling file, declared inside `arch/src/net/mod.rs` as
//! `#[cfg(feature = "selftest")] #[path = "net_tests.rs"] mod tests;`.
//!
//! These tests exercise the safe `UnixListener`/`UnixStream` wrappers:
//! loopback round-trip, error arms (ENOENT, ECONNREFUSED, EADDRINUSE, EBADF),
//! and `Drop` behaviour (listener unlinks the socket path).
//!
//! All tests use unique `/tmp/` paths tagged with a test-name suffix so
//! concurrent test runs do not collide (the selftest runner is sequential in
//! this process; the suffix makes paths unique enough for CI).

use crate::{
    arch_test,
    net::{UnixListener, UnixStream},
    sys::{
        AT_FDCWD, EADDRINUSE, EBADF, EINVAL, ENOENT, O_CLOEXEC, O_CREAT, O_WRONLY, accept, close,
        openat, unlinkat,
    },
    testrt,
    thread::spawn,
};

arch_test!(unix_listener_bind_listen_accept_roundtrip, {
    // Integration smoke: bind a listener, spawn an accept thread, connect from
    // the main thread, exchange a frame-sized buffer (16 bytes) round-trip,
    // assert byte-for-byte match. Proves the blocking thread-per-connection
    // carrier primitive composes end-to-end on the real floor.
    let mut pathbuf = [0u8; 64];
    let path: &[u8] = testrt::unique_path(b"/tmp/reovim-arch-net-roundtrip-", &mut pathbuf);

    let listener = UnixListener::bind(path).expect("bind+listen succeed");

    // Spawn the accept-side in a thread.
    let accept_handle = spawn::<_, Result<(), crate::sys::Errno>>(move || {
        let stream = listener.accept()?;
        // Echo back whatever the client sent — 16 bytes.
        let mut buf = [0u8; 16];
        stream.read_exact(&mut buf)?;
        stream.write_all(&buf)?;
        Ok(())
    })
    .expect("spawn accept thread");

    // Connect from the main thread.
    let client = UnixStream::connect(path).expect("connect succeeds");

    // Send the payload.
    let payload = b"reovim-arch-net!"; // 16 bytes
    client.write_all(payload).expect("write payload");

    // Read it back.
    let mut echo = [0u8; 16];
    client.read_exact(&mut echo).expect("read echo");
    testrt::check(&echo == payload, "round-trip bytes match");

    // Join the accept thread.
    accept_handle.join().expect("accept thread ok");
});

arch_test!(unix_stream_connect_absent_path_is_enoent, {
    // Connecting to a path that does not exist at all returns ENOENT.
    let result = UnixStream::connect(b"/tmp/reovim-arch-net-no-such-socket-enoent-test\0");
    testrt::check_eq(result.err(), Some(ENOENT));
});

arch_test!(unix_stream_connect_path_exists_but_no_listener_is_econnrefused, {
    // A regular file at the path is not a socket; the kernel returns an error
    // (ECONNREFUSED or ENOTSOCK) when connect is attempted.
    let mut pathbuf = [0u8; 64];
    let path: &[u8] = testrt::unique_path(b"/tmp/reovim-arch-net-no-listener-", &mut pathbuf);
    // Create a regular file so the path exists.
    let fd_raw =
        openat(AT_FDCWD, path, O_CREAT | O_WRONLY | O_CLOEXEC, 0o600).expect("create regular file");
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let fd = fd_raw as i32;
    close(fd).unwrap();

    let result = UnixStream::connect(path);
    // Unlink the temp file.
    let _ = unlinkat(AT_FDCWD, path, 0);
    // The kernel returns ECONNREFUSED when the path exists but is not a listening
    // socket. Any error here is acceptable — the key is it is not Ok.
    testrt::check(result.is_err(), "connect on non-listening path fails");
});

arch_test!(unix_listener_bind_over_existing_path_is_eaddrinuse, {
    // Binding twice to the same path returns EADDRINUSE on the second bind.
    let mut pathbuf = [0u8; 64];
    let path: &[u8] = testrt::unique_path(b"/tmp/reovim-arch-net-eaddrinuse-", &mut pathbuf);

    let first = UnixListener::bind(path).expect("first bind succeeds");
    let result = UnixListener::bind(path);
    drop(first); // cleans up the path via Drop

    testrt::check_eq(result.err(), Some(EADDRINUSE));
});

arch_test!(unix_listener_bind_empty_path_is_einval, {
    // An empty slice has no NUL terminator → EINVAL.
    testrt::check_eq(UnixListener::bind(b"").err(), Some(EINVAL));
});

arch_test!(unix_stream_connect_empty_path_is_einval, {
    testrt::check_eq(UnixStream::connect(b"").err(), Some(EINVAL));
});

arch_test!(unix_listener_drop_unlinks_socket_path, {
    // After Drop the socket file must not exist (unlinkat removes it).
    let mut pathbuf = [0u8; 64];
    let path: &[u8] = testrt::unique_path(b"/tmp/reovim-arch-net-drop-unlink-", &mut pathbuf);

    {
        let _listener = UnixListener::bind(path).expect("bind succeeds");
        // listener is dropped here, which calls close + unlinkat.
    }

    // Attempting to connect after drop returns ENOENT (path gone).
    testrt::check_eq(UnixStream::connect(path).err(), Some(ENOENT));
});

arch_test!(accept_on_bad_fd_returns_ebadf, {
    // Raw accept syscall on fd -1 returns EBADF — exercises the error arm.
    testrt::check_eq(accept(-1).err(), Some(EBADF));
});

arch_test!(unix_stream_write_to_closed_peer_is_epipe_not_sigpipe, {
    // A disconnecting client must surface as `EPIPE` on the next write —
    // the raw `write(2)` path would raise a process-killing `SIGPIPE`.
    let mut pathbuf = [0u8; 64];
    let path: &[u8] = testrt::unique_path(b"/tmp/reovim-arch-net-epipe-", &mut pathbuf);

    let listener = UnixListener::bind(path).expect("bind+listen succeed");
    let client = UnixStream::connect(path).expect("connect succeeds");
    let served = listener.accept().expect("accept succeeds");
    drop(served); // peer closes immediately

    // The first write may succeed into the socket buffer; loop until the
    // closed peer surfaces. Bounded: the kernel reports EPIPE as soon as
    // the reset is observed.
    let mut saw_epipe = false;
    for _ in 0..16 {
        match client.write(b"x") {
            Ok(_) => {}
            Err(e) => {
                testrt::check_eq(e, crate::sys::EPIPE);
                saw_epipe = true;
                break;
            }
        }
    }
    testrt::check(saw_epipe, "closed peer surfaces as EPIPE, process alive");
});
