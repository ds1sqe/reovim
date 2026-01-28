//! `InputService` gRPC implementation.
//!
//! Provides key input processing for v2 protocol clients.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_driver_input::{KeyCode, KeySequence, Modifiers},
    reovim_protocol::v2::{
        KeyStatus, SendKeysRequest, SendKeysResponse, input_service_server::InputService,
    },
    tonic::{Request, Response, Status},
};

use crate::session::{Session, SessionId, SessionRegistry};

/// gRPC `InputService` implementation.
///
/// Bridges v2 protocol key input requests to the session system.
/// Currently provides basic character insertion. Full vim-style
/// key resolution requires additional infrastructure.
pub struct InputServiceImpl {
    /// Shared session registry.
    sessions: Arc<SessionRegistry>,
    /// Default session ID to use when not specified.
    default_session_id: SessionId,
}

impl InputServiceImpl {
    /// Create a new `InputService` with access to the session registry.
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
impl InputService for InputServiceImpl {
    /// Send keys to the editor.
    ///
    /// Parses vim notation keys (e.g., "iHello<Esc>", "<C-w>h") and processes them.
    /// Currently supports basic character insertion. Full vim key resolution
    /// will be added as the server infrastructure grows.
    async fn send_keys(
        &self,
        request: Request<SendKeysRequest>,
    ) -> Result<Response<SendKeysResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        // Parse vim notation keys
        let keys = KeySequence::parse(&req.keys).ok_or_else(|| {
            Status::invalid_argument(format!("Invalid key notation: {}", req.keys))
        })?;

        // Process keys
        let mut any_handled = false;
        let mut final_status = KeyStatus::NotFound;

        for key in keys.as_slice() {
            // For now, we only handle simple character insertions
            // Full vim key resolution requires mode resolvers, keymap registry, etc.
            if let KeyCode::Char(ch) = key.code {
                // Only handle plain characters or shift+character (for uppercase)
                if key.modifiers.is_empty() || key.modifiers == Modifiers::SHIFT {
                    // Insert the character into the active buffer
                    let inserted = session
                        .with_state_mut(|state| {
                            // Collapse nested if-let
                            if let Some(buffer_id) = state.active_buffer()
                                && let Some(buffer_arc) = state.buffer(buffer_id)
                            {
                                // Merge write() with insert() to avoid drop timing lint
                                let _ = buffer_arc.write().insert(&ch.to_string());
                                return true;
                            }
                            false
                        })
                        .await;

                    if inserted {
                        any_handled = true;
                        final_status = KeyStatus::Executed;
                    }
                } else {
                    // Character with modifiers (Ctrl, Alt, etc.) - not handled yet
                    tracing::debug!(
                        ?ch,
                        ?key.modifiers,
                        "Key with modifiers not handled in minimal implementation"
                    );
                }
            } else {
                // Non-character keys (arrows, function keys, escape, etc.)
                // These require full vim mode resolution
                tracing::debug!(
                    ?key.code,
                    "Non-character key not handled in minimal implementation"
                );
            }
        }

        // Return result
        Ok(Response::new(SendKeysResponse {
            ok: any_handled,
            status: final_status.into(),
        }))
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
    async fn test_send_keys_valid_notation() {
        let registry = test_registry();
        let service = InputServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(SendKeysRequest {
            keys: "abc".to_string(),
        });
        let response = service.send_keys(request).await;

        // Should parse successfully, but may not execute without active buffer
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_send_keys_invalid_notation() {
        let registry = test_registry();
        let service = InputServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(SendKeysRequest {
            keys: "<Ctrl".to_string(), // Unclosed angle bracket
        });
        let response = service.send_keys(request).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_send_keys_special_keys() {
        let registry = test_registry();
        let service = InputServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(SendKeysRequest {
            keys: "<Esc>".to_string(),
        });
        let response = service.send_keys(request).await;

        // Should parse but not handle (non-character key)
        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(!resp.ok); // Not handled in minimal impl
    }

    #[tokio::test]
    async fn test_send_keys_with_modifiers() {
        let registry = test_registry();
        let service = InputServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(SendKeysRequest {
            keys: "<C-w>".to_string(),
        });
        let response = service.send_keys(request).await;

        // Should parse but not handle (modifier key)
        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(!resp.ok); // Not handled in minimal impl
    }

    #[tokio::test]
    async fn test_send_keys_no_session() {
        let registry = Arc::new(SessionRegistry::new());
        // No session inserted
        let service = InputServiceImpl::new(registry, SessionId::new("nonexistent"));

        let request = Request::new(SendKeysRequest {
            keys: "a".to_string(),
        });
        let response = service.send_keys(request).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::NotFound);
    }
}
