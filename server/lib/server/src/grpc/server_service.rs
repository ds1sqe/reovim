//! `ServerService` gRPC implementation.
//!
//! Provides server management operations for v2 protocol clients.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_protocol::v2::{
        InfoRequest, InfoResponse, KillRequest, KillResponse, PingRequest, PingResponse,
        server_service_server::ServerService,
    },
    tonic::{Request, Response, Status},
};

use crate::session::{SessionId, SessionRegistry};

/// Server version (from Cargo.toml).
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// gRPC `ServerService` implementation.
///
/// Bridges v2 protocol server management requests to the server system.
pub struct ServerServiceImpl {
    /// Shared session registry.
    sessions: Arc<SessionRegistry>,
    /// Default session ID to use when not specified.
    default_session_id: SessionId,
    /// Server start time (for uptime calculation).
    start_time: std::time::Instant,
}

impl ServerServiceImpl {
    /// Create a new `ServerService` with access to the session registry.
    #[must_use]
    pub fn new(sessions: Arc<SessionRegistry>, default_session_id: SessionId) -> Self {
        Self {
            sessions,
            default_session_id,
            start_time: std::time::Instant::now(),
        }
    }
}

#[tonic::async_trait]
impl ServerService for ServerServiceImpl {
    /// Kill the server (graceful shutdown).
    ///
    /// Note: Requires server-level shutdown signaling which isn't implemented
    /// in the minimal lib/server crate. Returns unimplemented for now.
    async fn kill(&self, _request: Request<KillRequest>) -> Result<Response<KillResponse>, Status> {
        // Server shutdown requires a shutdown channel or signal mechanism
        // that's wired through the Server struct. This needs architectural
        // work to implement properly.
        //
        // For now, return unimplemented. The CLI can use `pkill` or the
        // runner's existing JSON-RPC kill endpoint.
        Err(Status::unimplemented("Kill not yet implemented"))
    }

    /// Get server info.
    #[allow(clippy::cast_possible_truncation)]
    async fn info(&self, _request: Request<InfoRequest>) -> Result<Response<InfoResponse>, Status> {
        // Calculate uptime
        let uptime_secs = self.start_time.elapsed().as_secs();

        // Get buffer count from default session
        let buffer_count = if let Some(session) = self.sessions.get(&self.default_session_id) {
            session
                .with_state(|state| state.app.kernel.buffers.list().len())
                .await
        } else {
            0
        } as u64;

        // Client count - we don't track this yet in lib/server
        let client_count = 0u64;

        // Module count - modules are in the runner, not lib/server
        let module_count = 0u64;

        Ok(Response::new(InfoResponse {
            version: VERSION.to_string(),
            uptime_secs,
            client_count,
            buffer_count,
            module_count,
        }))
    }

    /// Ping (health check).
    async fn ping(&self, _request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        Ok(Response::new(PingResponse {
            pong: "pong".to_string(),
        }))
    }
}

#[cfg(test)]
#[path = "server_service_tests.rs"]
mod tests;
