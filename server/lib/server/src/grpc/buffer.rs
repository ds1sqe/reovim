//! `BufferService` gRPC implementation.
//!
//! # Architecture (#753 E6)
//!
//! Buffer metadata access is domain-neutral. The server no longer imports
//! driver crates directly. Proto v3 provides: List, OpenFile, WriteFile,
//! SetContent. Content access routes through projections, not typed RPCs.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_protocol::v3::{
        ListBuffersRequest, ListBuffersResponse, OpenFileRequest, OpenFileResponse,
        SetContentRequest, SetContentResponse, WriteFileRequest, WriteFileResponse,
        buffer_service_server::BufferService,
    },
    tonic::{Request, Response, Status},
};

use crate::session::{Session, SessionId, SessionRegistry};

/// gRPC `BufferService` implementation.
///
/// Bridges v3 protocol requests to the session/buffer system.
pub struct BufferServiceImpl {
    /// Shared session registry (used when RPCs are wired to domain driver).
    #[allow(dead_code)]
    sessions: Arc<SessionRegistry>,
    /// Default session ID to use when not specified.
    #[allow(dead_code)]
    default_session_id: SessionId,
}

impl BufferServiceImpl {
    /// Create a new `BufferService` with access to the session registry.
    #[must_use]
    pub const fn new(sessions: Arc<SessionRegistry>, default_session_id: SessionId) -> Self {
        Self {
            sessions,
            default_session_id,
        }
    }

    /// Get the default session.
    #[allow(dead_code)]
    fn get_session(&self) -> Result<Arc<Session>, Status> {
        self.sessions
            .get(&self.default_session_id)
            .ok_or_else(|| Status::not_found("No active session"))
    }
}

#[tonic::async_trait]
impl BufferService for BufferServiceImpl {
    /// List all open buffers.
    ///
    /// TODO(#753 E6): Route buffer metadata through DomainDriver.
    async fn list(
        &self,
        _request: Request<ListBuffersRequest>,
    ) -> Result<Response<ListBuffersResponse>, Status> {
        Err(Status::unimplemented(
            "ListBuffers: pending domain driver wiring (#753 E6)",
        ))
    }

    /// Open a file into a buffer (stub).
    async fn open_file(
        &self,
        _request: Request<OpenFileRequest>,
    ) -> Result<Response<OpenFileResponse>, Status> {
        Err(Status::unimplemented("OpenFile not yet implemented"))
    }

    /// Write buffer to file (stub).
    async fn write_file(
        &self,
        _request: Request<WriteFileRequest>,
    ) -> Result<Response<WriteFileResponse>, Status> {
        Err(Status::unimplemented("WriteFile not yet implemented"))
    }

    /// Set buffer content (stub).
    async fn set_content(
        &self,
        _request: Request<SetContentRequest>,
    ) -> Result<Response<SetContentResponse>, Status> {
        Err(Status::unimplemented("SetContent not yet implemented"))
    }
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
