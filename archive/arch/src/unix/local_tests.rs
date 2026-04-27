use super::*;

#[tokio::test]
async fn test_unix_socket_bind_accept() {
    let temp_dir = std::env::temp_dir();
    let socket_path = temp_dir.join(format!("reovim-test-{}.sock", std::process::id()));

    // Clean up any leftover socket
    let _ = std::fs::remove_file(&socket_path);

    // Bind listener
    let listener = UnixLocalListener::bind(&socket_path).await.unwrap();
    assert!(socket_path.exists());

    // Connect from client (in separate task)
    let path = socket_path.clone();
    let client_handle = tokio::spawn(async move { UnixLocalStream::connect(&path).await.unwrap() });

    // Accept connection
    let (server_stream, _) = listener.accept().await.unwrap();
    let _client_stream = client_handle.await.unwrap();

    // Verify streams are connected
    assert!(server_stream.inner().peer_addr().is_ok());
}

#[tokio::test]
async fn test_unix_socket_stale_cleanup() {
    let temp_dir = std::env::temp_dir();
    let socket_path = temp_dir.join(format!("reovim-test-stale-{}.sock", std::process::id()));

    // Create a stale socket file (just an empty file, not a real socket)
    std::fs::write(&socket_path, "").unwrap();
    assert!(socket_path.exists());

    // Binding should clean up the stale file and succeed
    let listener = UnixLocalListener::bind(&socket_path).await.unwrap();
    assert!(socket_path.exists());

    // Clean up
    drop(listener);
    assert!(!socket_path.exists());
}

#[test]
fn test_process_exists_current() {
    // Current process should exist
    let pid = std::process::id();
    assert!(process_exists(pid));
}

#[test]
fn test_process_exists_nonexistent() {
    // PID 1 (init) always exists, but a very high PID likely doesn't
    // Use a PID that's very unlikely to exist
    assert!(!process_exists(u32::MAX - 1));
}

#[tokio::test]
async fn test_unix_socket_path_accessor() {
    let temp_dir = std::env::temp_dir();
    let socket_path = temp_dir.join(format!("reovim-test-path-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixLocalListener::bind(&socket_path).await.unwrap();
    assert_eq!(listener.path(), socket_path.as_path());

    drop(listener);
}

#[tokio::test]
async fn test_unix_stream_inner_mut_and_into() {
    let temp_dir = std::env::temp_dir();
    let socket_path = temp_dir.join(format!("reovim-test-inner-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixLocalListener::bind(&socket_path).await.unwrap();

    let path = socket_path.clone();
    let client_handle = tokio::spawn(async move { UnixLocalStream::connect(&path).await.unwrap() });

    let (server_stream, _) = listener.accept().await.unwrap();
    let mut client_stream = client_handle.await.unwrap();

    // Test inner_mut
    let _inner_ref = client_stream.inner_mut();

    // Test inner (const)
    let _inner = server_stream.inner();

    drop(listener);
}

#[tokio::test]
async fn test_unix_stream_into_inner() {
    let temp_dir = std::env::temp_dir();
    let socket_path = temp_dir.join(format!("reovim-test-into-inner-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixLocalListener::bind(&socket_path).await.unwrap();

    let path = socket_path.clone();
    let client_handle = tokio::spawn(async move { UnixLocalStream::connect(&path).await.unwrap() });

    let (server_stream, _) = listener.accept().await.unwrap();
    let _client_stream = client_handle.await.unwrap();

    // Test into_inner
    let _tokio_stream = server_stream.into_inner();

    drop(listener);
}

#[tokio::test]
async fn test_unix_stream_into_split() {
    let temp_dir = std::env::temp_dir();
    let socket_path = temp_dir.join(format!("reovim-test-split-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixLocalListener::bind(&socket_path).await.unwrap();

    let path = socket_path.clone();
    let client_handle = tokio::spawn(async move { UnixLocalStream::connect(&path).await.unwrap() });

    let (server_stream, _) = listener.accept().await.unwrap();
    let _client_stream = client_handle.await.unwrap();

    // Test into_split
    let (_read_half, _write_half) = server_stream.into_split();

    drop(listener);
}

#[tokio::test]
#[cfg_attr(coverage_nightly, coverage(off))]
async fn test_unix_socket_in_use() {
    let temp_dir = std::env::temp_dir();
    let socket_path = temp_dir.join(format!("reovim-test-inuse-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&socket_path);

    // Bind first listener
    let listener = UnixLocalListener::bind(&socket_path).await.unwrap();

    // Try to bind again - should fail with AddrInUse
    let result = UnixLocalListener::bind(&socket_path).await;
    assert!(result.is_err());
    match result {
        Ok(_) => panic!("Expected bind to fail with AddrInUse"),
        Err(err) => assert_eq!(err.kind(), io::ErrorKind::AddrInUse),
    }

    drop(listener);
}
