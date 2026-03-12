use {super::*, crate::session::Session};

fn test_registry() -> Arc<SessionRegistry> {
    let registry = Arc::new(SessionRegistry::new());
    let session = Arc::new(Session::new(SessionId::new("test")));
    registry.insert(&session);
    registry
}

#[tokio::test]
async fn test_ping() {
    let registry = test_registry();
    let service = ServerServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(PingRequest {});
    let response = service.ping(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.pong, "pong");
}

#[tokio::test]
async fn test_info() {
    let registry = test_registry();
    let service = ServerServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(InfoRequest {});
    let response = service.info(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.version, VERSION);
    assert!(resp.uptime_secs == 0 || resp.uptime_secs <= 1); // Just started
    assert_eq!(resp.buffer_count, 0); // No buffers yet
    assert_eq!(resp.client_count, 0);
    assert_eq!(resp.module_count, 0);
}

#[tokio::test]
async fn test_kill_unimplemented() {
    let registry = test_registry();
    let service = ServerServiceImpl::new(registry, SessionId::new("test"));

    let request = Request::new(KillRequest { force: false });
    let response = service.kill(request).await;

    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
}

#[tokio::test]
async fn test_info_no_session() {
    // Create registry without a session
    let registry = Arc::new(SessionRegistry::new());
    let service = ServerServiceImpl::new(registry, SessionId::new("nonexistent"));

    let request = Request::new(InfoRequest {});
    let response = service.info(request).await;

    // Should still work, just with 0 buffer count
    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.buffer_count, 0);
}
