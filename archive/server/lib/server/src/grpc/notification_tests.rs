use super::*;

fn test_registry() -> Arc<SessionRegistry> {
    let registry = Arc::new(SessionRegistry::new());
    let session = Arc::new(Session::new(SessionId::new("test")));
    registry.insert(&session);
    registry
}

fn test_tokens() -> Arc<TokenRegistry> {
    Arc::new(TokenRegistry::new())
}

#[tokio::test]
async fn test_subscribe_no_session() {
    let registry = Arc::new(SessionRegistry::new());
    let service =
        NotificationServiceImpl::new(registry, SessionId::new("nonexistent"), test_tokens());

    let request = Request::new(SubscribeRequest {
        event_types: vec![],
    });
    let response = service.subscribe(request).await;

    assert!(response.is_err());
    let err = response.err().unwrap();
    assert_eq!(err.code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_subscribe_returns_stream() {
    let registry = test_registry();
    let service = NotificationServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let request = Request::new(SubscribeRequest {
        event_types: vec![],
    });
    let response = service.subscribe(request).await;

    // Should successfully return a stream
    assert!(response.is_ok());
}

#[tokio::test]
async fn test_subscribe_with_filter() {
    let registry = test_registry();
    let service = NotificationServiceImpl::new(registry, SessionId::new("test"), test_tokens());

    let request = Request::new(SubscribeRequest {
        event_types: vec!["mode_changed".to_string()],
    });
    let response = service.subscribe(request).await;

    assert!(response.is_ok());
}

// ── Auto-cleanup tests (#483 Phase 4) ────────────────────────────

#[tokio::test]
async fn test_stream_drop_revokes_token() {
    let tokens = test_tokens();
    let registry = test_registry();
    let service = NotificationServiceImpl::new(
        Arc::clone(&registry),
        SessionId::new("test"),
        Arc::clone(&tokens),
    );

    // Simulate a joined client with a token
    let client_id = registry.next_client_id();
    let token = tokens.register(client_id);
    let session = registry.get(&SessionId::new("test")).unwrap();
    session.add_client(client_id);

    // Subscribe with token in extensions
    let mut request = Request::new(SubscribeRequest {
        event_types: vec![],
    });
    request.extensions_mut().insert(client_id);
    let response = service.subscribe(request).await.unwrap();

    // Token is still valid while stream is alive
    assert!(tokens.resolve(&token).is_some());

    // Drop the stream — should trigger cleanup
    drop(response);

    // Token should be revoked
    assert!(tokens.resolve(&token).is_none());
}

#[tokio::test]
async fn test_stream_drop_removes_client() {
    let tokens = test_tokens();
    let registry = test_registry();
    let service = NotificationServiceImpl::new(
        Arc::clone(&registry),
        SessionId::new("test"),
        Arc::clone(&tokens),
    );

    let client_id = registry.next_client_id();
    tokens.register(client_id);
    let session = registry.get(&SessionId::new("test")).unwrap();
    session.add_client(client_id);

    // Subscribe with token
    let mut request = Request::new(SubscribeRequest {
        event_types: vec![],
    });
    request.extensions_mut().insert(client_id);
    let response = service.subscribe(request).await.unwrap();

    assert!(session.clients().has_client(client_id));

    // Drop stream → auto-cleanup
    drop(response);

    assert!(!session.clients().has_client(client_id));
}

#[tokio::test]
async fn test_stream_drop_removes_from_presence() {
    use crate::session::ClientPresence;

    let tokens = test_tokens();
    let registry = test_registry();
    let service = NotificationServiceImpl::new(
        Arc::clone(&registry),
        SessionId::new("test"),
        Arc::clone(&tokens),
    );

    let client_id = registry.next_client_id();
    tokens.register(client_id);
    let session = registry.get(&SessionId::new("test")).unwrap();
    session.add_client(client_id);
    session
        .presence()
        .join(ClientPresence::new(client_id, "test", "cleanup-test"));

    // Subscribe with token
    let mut request = Request::new(SubscribeRequest {
        event_types: vec![],
    });
    request.extensions_mut().insert(client_id);
    let response = service.subscribe(request).await.unwrap();

    assert!(session.presence().contains(client_id));

    // Drop stream → auto-cleanup
    drop(response);

    assert!(!session.presence().contains(client_id));
}

#[tokio::test]
async fn test_stream_drop_without_token_no_cleanup() {
    let tokens = test_tokens();
    let registry = test_registry();
    let service = NotificationServiceImpl::new(
        Arc::clone(&registry),
        SessionId::new("test"),
        Arc::clone(&tokens),
    );

    let session = registry.get(&SessionId::new("test")).unwrap();
    let client_id = ClientId::new(99);
    session.add_client(client_id);

    // Subscribe WITHOUT token in extensions (backward compat)
    let request = Request::new(SubscribeRequest {
        event_types: vec![],
    });
    let response = service.subscribe(request).await.unwrap();

    // Drop stream — should NOT cleanup (no token identity)
    drop(response);

    // Client should still be present
    assert!(session.clients().has_client(client_id));
}

#[tokio::test]
async fn test_cleanup_idempotent_with_explicit_leave() {
    use crate::session::ClientPresence;

    let tokens = test_tokens();
    let registry = test_registry();
    let service = NotificationServiceImpl::new(
        Arc::clone(&registry),
        SessionId::new("test"),
        Arc::clone(&tokens),
    );

    let client_id = registry.next_client_id();
    tokens.register(client_id);
    let session = registry.get(&SessionId::new("test")).unwrap();
    session.add_client(client_id);
    session
        .presence()
        .join(ClientPresence::new(client_id, "test", "idem-test"));

    // Subscribe with token
    let mut request = Request::new(SubscribeRequest {
        event_types: vec![],
    });
    request.extensions_mut().insert(client_id);
    let response = service.subscribe(request).await.unwrap();

    // Simulate explicit Leave() before stream drop
    tokens.revoke_by_client(client_id);
    session.presence().leave(client_id);
    session.remove_client(client_id);

    // Drop stream — cleanup closure runs but all ops are no-ops
    drop(response); // Should not panic

    // Everything already cleaned up
    assert!(!session.clients().has_client(client_id));
    assert!(!session.presence().contains(client_id));
    assert!(tokens.is_empty());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_presence_left_notification() {
    let client_id = ClientId::new(42);
    let notification = build_presence_left_notification(client_id, "test-user");

    assert_eq!(notification.event_type, "presence_left");
    assert!(notification.timestamp_ms > 0);

    if let Some(reovim_protocol::v3::notification::Payload::PresenceLeft(payload)) =
        notification.payload
    {
        assert_eq!(payload.client_id, 42);
        assert_eq!(payload.display_name, "test-user");
    } else {
        panic!("Expected PresenceLeft payload");
    }
}
