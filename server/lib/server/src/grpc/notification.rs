//! `NotificationService` gRPC implementation.
//!
//! Provides server-to-client streaming for real-time notifications.
//! Uses gRPC server streaming to push state changes to connected clients.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::{pin::Pin, sync::Arc};

use {
    futures::Stream,
    reovim_protocol::v2::{
        Notification, SubscribeRequest, notification_service_server::NotificationService,
    },
    tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError},
    tonic::{Request, Response, Status},
};

use crate::session::{Session, SessionId, SessionRegistry};

/// gRPC `NotificationService` implementation.
///
/// Provides server-to-client streaming for real-time notifications.
/// Clients subscribe to receive state changes (mode, cursor, buffer, etc.)
/// as they happen.
pub struct NotificationServiceImpl {
    /// Shared session registry.
    sessions: Arc<SessionRegistry>,
    /// Default session ID to use when not specified.
    default_session_id: SessionId,
}

impl NotificationServiceImpl {
    /// Create a new `NotificationService` with access to the session registry.
    #[must_use]
    pub const fn new(sessions: Arc<SessionRegistry>, default_session_id: SessionId) -> Self {
        Self {
            sessions,
            default_session_id,
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
    /// # Arguments
    ///
    /// * `request` - Contains optional event type filters
    ///
    /// # Returns
    ///
    /// A streaming response of notifications until the client disconnects.
    async fn subscribe(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
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

        Ok(Response::new(Box::pin(output_stream)))
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

    #[tokio::test]
    async fn test_subscribe_no_session() {
        let registry = Arc::new(SessionRegistry::new());
        let service = NotificationServiceImpl::new(registry, SessionId::new("nonexistent"));

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
        let service = NotificationServiceImpl::new(registry, SessionId::new("test"));

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
        let service = NotificationServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(SubscribeRequest {
            event_types: vec!["mode_changed".to_string()],
        });
        let response = service.subscribe(request).await;

        assert!(response.is_ok());
    }
}
