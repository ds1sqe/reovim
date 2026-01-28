//! `BufferService` gRPC implementation.
//!
//! Provides raw buffer content access for v2 protocol clients.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_kernel::api::v1::BufferId,
    reovim_protocol::v2::{
        BufferInfo, GetAnnotationsRequest, GetAnnotationsResponse, GetLineCountRequest,
        GetLineCountResponse, GetRawContentRequest, GetRawContentResponse, LineAnnotation,
        ListBuffersRequest, ListBuffersResponse, OpenFileRequest, OpenFileResponse,
        SetContentRequest, SetContentResponse, WriteFileRequest, WriteFileResponse,
        buffer_service_server::BufferService,
    },
    tonic::{Request, Response, Status},
};

use crate::server::session::{SessionId, SessionRegistry};

/// gRPC `BufferService` implementation.
///
/// Bridges v2 protocol requests to the session/buffer system.
pub struct BufferServiceImpl {
    /// Shared session registry.
    pub sessions: Arc<SessionRegistry>,
    /// Default session ID to use when not specified.
    pub default_session_id: SessionId,
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
    fn get_session(&self) -> Result<Arc<crate::server::session::Session>, Status> {
        self.sessions
            .get(&self.default_session_id)
            .ok_or_else(|| Status::not_found("No active session"))
    }
}

#[tonic::async_trait]
impl BufferService for BufferServiceImpl {
    /// Get raw content lines from a buffer.
    ///
    /// Returns lines without any rendering (no gutter, no styling).
    /// Clients are responsible for rendering the content.
    #[allow(clippy::cast_possible_truncation)] // Buffer IDs fit in usize on all supported platforms
    #[allow(clippy::significant_drop_tightening)] // RwLock guard needed for multiple field accesses
    async fn get_raw_content(
        &self,
        request: Request<GetRawContentRequest>,
    ) -> Result<Response<GetRawContentResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        session
            .with_state(|state| {
                // Determine which buffer to query
                let buffer_id = req
                    .buffer_id
                    .map(|id| BufferId::from_raw(id as usize))
                    .or_else(|| state.session_active_buffer())
                    .ok_or_else(|| Status::not_found("No active buffer"))?;

                let buffer_arc = state.app.kernel.buffers.get(buffer_id).ok_or_else(|| {
                    Status::not_found(format!("Buffer {} not found", buffer_id.as_usize()))
                })?;

                let buffer = buffer_arc.read();
                let total_lines = buffer.line_count();
                let start = req.start_line.unwrap_or(0) as usize;
                let end = req
                    .end_line
                    .map_or(total_lines, |e| e as usize)
                    .min(total_lines);

                // Collect lines in the requested range
                let lines: Vec<String> = (start..end)
                    .filter_map(|i| buffer.line(i).map(ToString::to_string))
                    .collect();

                Ok(Response::new(GetRawContentResponse {
                    buffer_id: buffer_id.as_usize() as u64,
                    lines,
                    start_line: start as u64,
                    total_lines: total_lines as u64,
                }))
            })
            .await
    }

    /// Get the line count of a buffer.
    #[allow(clippy::cast_possible_truncation)] // Buffer IDs fit in usize on all supported platforms
    async fn get_line_count(
        &self,
        request: Request<GetLineCountRequest>,
    ) -> Result<Response<GetLineCountResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        session
            .with_state(|state| {
                let buffer_id = req
                    .buffer_id
                    .map(|id| BufferId::from_raw(id as usize))
                    .or_else(|| state.session_active_buffer())
                    .ok_or_else(|| Status::not_found("No active buffer"))?;

                let buffer_arc = state.app.kernel.buffers.get(buffer_id).ok_or_else(|| {
                    Status::not_found(format!("Buffer {} not found", buffer_id.as_usize()))
                })?;

                let buffer = buffer_arc.read();
                Ok(Response::new(GetLineCountResponse {
                    buffer_id: buffer_id.as_usize() as u64,
                    line_count: buffer.line_count() as u64,
                }))
            })
            .await
    }

    /// Get annotations for a buffer (diagnostics, git signs, etc.).
    ///
    /// Phase 2: Returns empty - annotation system integration deferred.
    #[allow(clippy::cast_possible_truncation)] // Buffer IDs fit in usize on all supported platforms
    async fn get_annotations(
        &self,
        request: Request<GetAnnotationsRequest>,
    ) -> Result<Response<GetAnnotationsResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        session
            .with_state(|state| {
                let buffer_id = req
                    .buffer_id
                    .map(|id| BufferId::from_raw(id as usize))
                    .or_else(|| state.session_active_buffer())
                    .ok_or_else(|| Status::not_found("No active buffer"))?;

                // Phase 2: Return empty annotations
                // Full annotation support will come when we integrate with the annotation system
                Ok(Response::new(GetAnnotationsResponse {
                    buffer_id: buffer_id.as_usize() as u64,
                    annotations: Vec::<LineAnnotation>::new(),
                }))
            })
            .await
    }

    /// List all open buffers.
    #[allow(clippy::significant_drop_tightening)] // RwLock guard needed for multiple field accesses
    async fn list(
        &self,
        _request: Request<ListBuffersRequest>,
    ) -> Result<Response<ListBuffersResponse>, Status> {
        let session = self.get_session()?;

        session
            .with_state(|state| {
                let buffers: Vec<BufferInfo> = state
                    .app
                    .kernel
                    .buffers
                    .list()
                    .iter()
                    .filter_map(|&id| {
                        state.app.kernel.buffers.get(id).map(|arc| {
                            let buf = arc.read();
                            // Derive display name from file path or use placeholder
                            let name = buf
                                .file_path()
                                .and_then(|p| std::path::Path::new(p).file_name())
                                .map_or_else(
                                    || format!("[Buffer {}]", id.as_usize()),
                                    |n| n.to_string_lossy().into_owned(),
                                );
                            BufferInfo {
                                id: id.as_usize() as u64,
                                name,
                                path: buf.file_path().map(String::from),
                                line_count: buf.line_count() as u64,
                                modified: buf.is_modified(),
                            }
                        })
                    })
                    .collect();

                Ok(Response::new(ListBuffersResponse { buffers }))
            })
            .await
    }

    /// Open a file into a buffer.
    ///
    /// Phase 2: Stubbed - file operations deferred.
    async fn open_file(
        &self,
        _request: Request<OpenFileRequest>,
    ) -> Result<Response<OpenFileResponse>, Status> {
        // Phase 2: Stub - full file operations deferred to Phase 3+
        Err(Status::unimplemented(
            "OpenFile not implemented in Phase 2 - use JSON-RPC buffer/open",
        ))
    }

    /// Write buffer to file.
    ///
    /// Phase 2: Stubbed - file operations deferred.
    async fn write_file(
        &self,
        _request: Request<WriteFileRequest>,
    ) -> Result<Response<WriteFileResponse>, Status> {
        // Phase 2: Stub - full file operations deferred to Phase 3+
        Err(Status::unimplemented(
            "WriteFile not implemented in Phase 2 - use JSON-RPC buffer/write",
        ))
    }

    /// Set buffer content.
    ///
    /// Phase 2: Stubbed - content modification deferred.
    async fn set_content(
        &self,
        _request: Request<SetContentRequest>,
    ) -> Result<Response<SetContentResponse>, Status> {
        // Phase 2: Stub - content modification deferred to Phase 3+
        Err(Status::unimplemented(
            "SetContent not implemented in Phase 2 - use JSON-RPC buffer/set_content",
        ))
    }
}

#[cfg(test)]
mod tests {
    // Tests will require a mock SessionRegistry
    // Deferred to integration tests where we can spin up a real session
}
