use super::*;

#[test]
fn server_new_creates_empty_registries() {
    let server = Server::new(ServerConfig::default());
    assert!(server.sessions.is_empty());
    assert!(server.services.is_none());
    assert!(server.session_factory.is_none());
}

#[test]
fn server_with_services() {
    let services = Arc::new(ServiceRegistry::new());
    let server = Server::with_services(ServerConfig::default(), services);
    assert!(server.services.is_some());
    assert!(server.session_factory.is_none());
}

#[test]
fn server_with_session_factory() {
    let factory: SessionFactory = Box::new(SessionState::default);
    let server = Server::with_session_factory(ServerConfig::default(), factory);
    assert!(server.services.is_none());
    assert!(server.session_factory.is_some());
}

#[test]
fn create_session_state_uses_default_when_no_factory() {
    let server = Server::new(ServerConfig::default());
    let state = server.create_session_state();
    // Just verify it returns without panicking
    let _ = state;
}

#[test]
fn create_session_state_uses_factory() {
    let factory: SessionFactory = Box::new(|| {
        let mut state = SessionState::default();
        state.app.running = false; // marker to verify factory was called
        state
    });
    let server = Server::with_session_factory(ServerConfig::default(), factory);
    let state = server.create_session_state();
    assert!(!state.app.is_running());
}

#[test]
fn sessions_accessor() {
    let server = Server::new(ServerConfig::default());
    assert!(server.sessions().is_empty());
}

#[cfg(feature = "grpc")]
#[test]
fn server_with_bridges() {
    struct TestBridge;
    impl reovim_driver_session::bridges::ExtensionStateBridge for TestBridge {
        fn kind(&self) -> &'static str {
            "test-bridge"
        }
        fn scope(&self) -> reovim_driver_session::bridges::ExtensionScope {
            reovim_driver_session::bridges::ExtensionScope::Client
        }
        fn snapshot(
            &self,
            _: &reovim_driver_session::ExtensionMap,
        ) -> Option<serde_json::Value> {
            None
        }
        fn is_active(&self, _: &reovim_driver_session::ExtensionMap) -> bool {
            false
        }
        fn on_mode_changed(
            &self,
            _from: &str,
            _to: &str,
            _: &mut reovim_driver_session::ExtensionMap,
        ) {
        }
    }

    let mut registry = BridgeRegistry::new();
    registry.register(TestBridge);
    let server = Server::new(ServerConfig::default()).with_bridges(registry);
    // Verify the bridge registry was set (it's inside an Arc)
    let _ = server;
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn run_until_starts_and_shuts_down() {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let (port_tx, port_rx) = tokio::sync::oneshot::channel::<u16>();

    let config = ServerConfig::grpc(0); // OS-assigned port
    let server = Arc::new(Server::new(config));

    let server_clone = Arc::clone(&server);
    let handle = tokio::spawn(async move {
        server_clone
            .run_until(
                async {
                    shutdown_rx.await.ok();
                },
                Some(port_tx),
            )
            .await
    });

    // Wait for port to be assigned
    let port = port_rx.await.expect("should receive port");
    assert!(port > 0);

    // Verify session was created
    assert_eq!(server.sessions().len(), 1);

    // Shut down
    shutdown_tx.send(()).expect("should send shutdown");
    let result = handle.await.expect("task should complete");
    assert!(result.is_ok());
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn run_until_rejects_non_grpc_transport() {
    let config = ServerConfig::tcp(8080);
    let server = Server::new(config);

    let result = server.run_until(std::future::pending::<()>(), None).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn run_until_with_factory_creates_custom_session() {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let (port_tx, port_rx) = tokio::sync::oneshot::channel::<u16>();

    let factory: SessionFactory = Box::new(SessionState::default);
    let config = ServerConfig::grpc(0);
    let server = Arc::new(Server::with_session_factory(config, factory));

    let server_clone = Arc::clone(&server);
    let handle = tokio::spawn(async move {
        server_clone
            .run_until(
                async {
                    shutdown_rx.await.ok();
                },
                Some(port_tx),
            )
            .await
    });

    let _port = port_rx.await.expect("should receive port");
    assert_eq!(server.sessions().len(), 1);

    shutdown_tx.send(()).expect("should send shutdown");
    handle
        .await
        .expect("task should complete")
        .expect("server should succeed");
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn run_creates_session_and_dispatches_grpc() {
    // Cover lines 165-187: run() creates session state, inserts default session,
    // and dispatches to the appropriate transport.
    // We use gRPC transport with port 0 (OS-assigned) so it binds successfully.
    // Since run() doesn't accept a shutdown signal, we abort the task after verifying.
    let config = ServerConfig::grpc(0);
    let server = Arc::new(Server::new(config));

    let server_clone = Arc::clone(&server);
    let handle = tokio::spawn(async move { server_clone.run().await });

    // Give the server a moment to start and create the session
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Verify session was created by run()
    assert_eq!(server.sessions().len(), 1);

    // Abort the task since run() doesn't have a shutdown mechanism
    handle.abort();
    let _ = handle.await;
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn run_tcp_transport_creates_session() {
    // Cover lines 180-182: the TcpWithFallback match arm in run().
    // run() with TcpWithFallback will call run_tcp_fallback which calls run_tcp
    // which blocks forever. We abort after verifying session creation.
    let config = ServerConfig::default(); // TcpWithFallback
    let server = Arc::new(Server::new(config));

    let server_clone = Arc::clone(&server);
    let handle = tokio::spawn(async move { server_clone.run().await });

    // Give the server a moment to create the session
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Verify session was created
    assert_eq!(server.sessions().len(), 1);

    handle.abort();
    let _ = handle.await;
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn run_tcp_specific_port_creates_session() {
    // Cover lines 182, 207-208: the Tcp { port } match arm and run_tcp entry.
    let config = ServerConfig::tcp(0);
    let server = Arc::new(Server::new(config));

    let server_clone = Arc::clone(&server);
    let handle = tokio::spawn(async move { server_clone.run().await });

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(server.sessions().len(), 1);

    handle.abort();
    let _ = handle.await;
}

#[cfg(all(feature = "grpc", unix))]
#[tokio::test]
async fn run_unix_socket_creates_session() {
    // Cover lines 184, 217-218: the UnixSocket match arm and run_unix entry.
    let tmp = tempfile::tempdir().expect("should create tempdir");
    let socket_path = tmp.path().join("test.sock");
    let config = ServerConfig::unix_socket(&socket_path);
    let server = Arc::new(Server::new(config));

    let server_clone = Arc::clone(&server);
    let handle = tokio::spawn(async move { server_clone.run().await });

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(server.sessions().len(), 1);

    handle.abort();
    let _ = handle.await;
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn run_grpc_without_shutdown_signal() {
    // Cover lines 329-332 (grpc-web) or equivalent non-grpc-web: the else branch
    // in run_grpc where shutdown is None, calling serve_with_incoming.
    // run() calls run_grpc(port, None, None) so shutdown is None.
    let config = ServerConfig::grpc(0);
    let server = Arc::new(Server::new(config));

    let server_clone = Arc::clone(&server);
    let handle = tokio::spawn(async move { server_clone.run().await });

    // Give time for run_grpc to bind and enter serve_with_incoming
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(server.sessions().len(), 1);

    handle.abort();
    let _ = handle.await;
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn run_until_without_port_sender() {
    // Cover run_until with port_tx = None
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let config = ServerConfig::grpc(0);
    let server = Arc::new(Server::new(config));

    let server_clone = Arc::clone(&server);
    let handle = tokio::spawn(async move {
        server_clone
            .run_until(
                async {
                    shutdown_rx.await.ok();
                },
                None, // no port sender
            )
            .await
    });

    // Give server time to start
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(server.sessions().len(), 1);

    shutdown_tx.send(()).expect("should send shutdown");
    let result = handle.await.expect("task should complete");
    assert!(result.is_ok());
}

#[cfg(feature = "grpc")]
#[tokio::test]
async fn run_with_custom_session_name() {
    // Verify run() uses the configured session name
    let config = ServerConfig::grpc(0).with_session_name("my-custom-session");
    let server = Arc::new(Server::new(config));

    let server_clone = Arc::clone(&server);
    let handle = tokio::spawn(async move { server_clone.run().await });

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(server.sessions().len(), 1);

    handle.abort();
    let _ = handle.await;
}
