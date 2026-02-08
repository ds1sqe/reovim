//! `NotificationService` gRPC implementation.
//!
//! Provides server-to-client streaming for real-time notifications.
//! Uses gRPC server streaming to push state changes to connected clients.
//!
//! # Auto-Cleanup on Disconnect (#483 Phase 4)
//!
//! The notification stream acts as the client's heartbeat. When the stream
//! is dropped (client disconnect, crash, or network failure), a
//! [`CleanupStream`] wrapper triggers automatic cleanup:
//!
//! 1. Revoke the client's session token
//! 2. Remove from presence map
//! 3. Remove from client map (dumps ring buffer for diagnostics)
//! 4. Emit `presence_left` notification to remaining peers

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]
// gRPC protocol uses u64 for IDs; internally we use usize. Equivalent on 64-bit.
#![allow(clippy::cast_possible_truncation)]

use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::SystemTime,
};

use {
    futures::Stream,
    reovim_protocol::v2::{
        Notification, SubscribeRequest, notification::Payload,
        notification_service_server::NotificationService,
    },
    tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError},
    tonic::{Request, Response, Status},
};

use crate::session::{ClientId, Session, SessionId, SessionRegistry, TokenRegistry};

/// Cleanup handler for notification stream disconnection.
///
/// Called when a `CleanupStream` is dropped (client disconnect, crash, or
/// network failure). Only runs as part of gRPC stream lifecycle.
#[cfg_attr(coverage_nightly, coverage(off))]
fn on_notification_stream_dropped(
    client_id: ClientId,
    session: &Session,
    tokens: &TokenRegistry,
) {
    tracing::info!(
        client_id = client_id.as_usize(),
        "Notification stream dropped — auto-cleanup"
    );

    // 1. Revoke session token
    tokens.revoke_by_client(client_id);

    // 2. Remove from presence map and emit notification
    if let Some(presence) = session.presence().leave(client_id) {
        session.emit_notification(build_presence_left_notification(
            client_id,
            &presence.display_name,
        ));
    }

    // 3. Remove from client map (dumps ring buffer for diagnostics)
    session.remove_client(client_id);
}

// ─────────────────────────────────────────────────────────────────────────────
// CleanupStream (#483 Phase 4)
// ─────────────────────────────────────────────────────────────────────────────

/// A stream wrapper that triggers client cleanup when dropped.
///
/// When a notification stream is dropped (client disconnect), the `Drop` impl
/// runs the cleanup closure which revokes the token, removes the client from
/// the session, and emits a `presence_left` notification.
///
/// All cleanup operations are idempotent — if the client already called
/// `Leave()` explicitly, the cleanup closure is a no-op.
struct CleanupStream {
    inner: Pin<Box<dyn Stream<Item = Result<Notification, Status>> + Send>>,
    cleanup: Option<Box<dyn FnOnce() + Send>>,
}

impl CleanupStream {
    fn new(
        inner: Pin<Box<dyn Stream<Item = Result<Notification, Status>> + Send>>,
        cleanup: impl FnOnce() + Send + 'static,
    ) -> Self {
        Self {
            inner,
            cleanup: Some(Box::new(cleanup)),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Stream for CleanupStream {
    type Item = Result<Notification, Status>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

impl Drop for CleanupStream {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Notification helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build `presence_left` notification for auto-cleanup.
fn build_presence_left_notification(client_id: ClientId, display_name: &str) -> Notification {
    use reovim_protocol::v2::PresenceLeftPayload;

    let timestamp_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time before UNIX_EPOCH")
        .as_millis() as u64;

    Notification {
        event_type: "presence_left".to_string(),
        timestamp_ms,
        payload: Some(Payload::PresenceLeft(PresenceLeftPayload {
            client_id: client_id.as_usize() as u64,
            display_name: display_name.to_string(),
        })),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NotificationServiceImpl
// ─────────────────────────────────────────────────────────────────────────────

/// gRPC `NotificationService` implementation.
///
/// Provides server-to-client streaming for real-time notifications.
/// Clients subscribe to receive state changes (mode, cursor, buffer, etc.)
/// as they happen.
///
/// When the stream is dropped, automatic client cleanup is triggered
/// via [`CleanupStream`] (#483 Phase 4).
pub struct NotificationServiceImpl {
    /// Shared session registry.
    sessions: Arc<SessionRegistry>,
    /// Default session ID to use when not specified.
    default_session_id: SessionId,
    /// Token registry for cleanup on disconnect (#483).
    tokens: Arc<TokenRegistry>,
}

impl NotificationServiceImpl {
    /// Create a new `NotificationService` with access to the session registry.
    #[must_use]
    pub const fn new(
        sessions: Arc<SessionRegistry>,
        default_session_id: SessionId,
        tokens: Arc<TokenRegistry>,
    ) -> Self {
        Self {
            sessions,
            default_session_id,
            tokens,
        }
    }

    /// Get the default session.
    fn get_session(&self) -> Result<Arc<Session>, Status> {
        self.sessions
            .get(&self.default_session_id)
            .ok_or_else(|| Status::not_found("No active session"))
    }
}

#[tonic::async_trait]
impl NotificationService for NotificationServiceImpl {
    /// Stream type for Subscribe RPC.
    type SubscribeStream =
        Pin<Box<dyn Stream<Item = Result<Notification, Status>> + Send + 'static>>;

    /// Subscribe to notifications (server streaming).
    ///
    /// Returns a stream of notifications for state changes. Clients can
    /// optionally filter by event types.
    ///
    /// # Auto-Cleanup (#483 Phase 4)
    ///
    /// If the subscribing client has a token-authenticated identity (from
    /// `x-reovim-token` header), the stream is wrapped in a [`CleanupStream`]
    /// that automatically cleans up the client on disconnect:
    ///
    /// 1. Revoke session token
    /// 2. Remove from presence map (emit `presence_left`)
    /// 3. Remove from client map (dump ring buffer)
    ///
    /// Clients without token authentication (backward compat) get the stream
    /// without auto-cleanup — they must call `Leave()` explicitly.
    async fn subscribe(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        // #483: Extract token-authenticated ClientId before into_inner()
        let client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        // Get the notification receiver from the session
        let rx = session.subscribe_notifications();

        // Convert event_types to a filter set (empty means all events)
        let filter_types: Vec<String> = req.event_types;

        // Create stream from broadcast receiver
        let stream = BroadcastStream::new(rx);

        // Map and filter the stream
        let output_stream = async_stream::stream! {
            let mut stream = stream;
            while let Some(result) = futures::StreamExt::next(&mut stream).await {
                match result {
                    Ok(notification) => {
                        // Apply filter if specified
                        if filter_types.is_empty() || filter_types.contains(&notification.event_type) {
                            yield Ok(notification);
                        }
                    }
                    Err(BroadcastStreamRecvError::Lagged(n)) => {
                        // Client fell behind, log and continue
                        tracing::warn!(lagged = n, "Notification subscriber lagged behind");
                    }
                }
            }
        };

        // #483 Phase 4: Wrap stream with cleanup guard if client is authenticated
        if let Some(cid) = client_id {
            let cleanup_session = Arc::clone(&session);
            let cleanup_tokens = Arc::clone(&self.tokens);

            let guarded = CleanupStream::new(Box::pin(output_stream), move || {
                on_notification_stream_dropped(cid, &cleanup_session, &cleanup_tokens);
            });

            Ok(Response::new(Box::pin(guarded)))
        } else {
            // No token — backward compat, no auto-cleanup
            Ok(Response::new(Box::pin(output_stream)))
        }
    }
}

#[cfg(test)]
mod tests {
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

        assert!(session.has_client(client_id));

        // Drop stream → auto-cleanup
        drop(response);

        assert!(!session.has_client(client_id));
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
        assert!(session.has_client(client_id));
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
        assert!(!session.has_client(client_id));
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

        if let Some(reovim_protocol::v2::notification::Payload::PresenceLeft(payload)) =
            notification.payload
        {
            assert_eq!(payload.client_id, 42);
            assert_eq!(payload.display_name, "test-user");
        } else {
            panic!("Expected PresenceLeft payload");
        }
    }
}
