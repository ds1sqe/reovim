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

use crate::session::{Session, SessionId, SessionRegistry};

/// gRPC `BufferService` implementation.
///
/// Bridges v2 protocol requests to the session/buffer system.
pub struct BufferServiceImpl {
    /// Shared session registry.
    sessions: Arc<SessionRegistry>,
    /// Default session ID to use when not specified.
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
    fn get_session(&self) -> Result<Arc<Session>, Status> {
        self.sessions
            .get(&self.default_session_id)
            .ok_or_else(|| Status::not_found("No active session"))
    }
}

#[tonic::async_trait]
impl BufferService for BufferServiceImpl {
    /// Get raw content lines from a buffer.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::significant_drop_tightening)]
    async fn get_raw_content(
        &self,
        request: Request<GetRawContentRequest>,
    ) -> Result<Response<GetRawContentResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        session
            .with_state(|state| {
                let buffer_id = req
                    .buffer_id
                    .map(|id| BufferId::from_raw(id as usize))
                    .or_else(|| state.active_buffer())
                    .ok_or_else(|| Status::not_found("No active buffer"))?;

                let buffer_arc = state.buffer(buffer_id).ok_or_else(|| {
                    Status::not_found(format!("Buffer {} not found", buffer_id.as_usize()))
                })?;

                let buffer = buffer_arc.read();
                let total_lines = buffer.line_count();
                let start = req.start_line.unwrap_or(0) as usize;
                let end = req
                    .end_line
                    .map_or(total_lines, |e| e as usize)
                    .min(total_lines);

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
    #[allow(clippy::cast_possible_truncation)]
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
                    .or_else(|| state.active_buffer())
                    .ok_or_else(|| Status::not_found("No active buffer"))?;

                let buffer_arc = state.buffer(buffer_id).ok_or_else(|| {
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

    /// Get annotations for a buffer.
    #[allow(clippy::cast_possible_truncation)]
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
                    .or_else(|| state.active_buffer())
                    .ok_or_else(|| Status::not_found("No active buffer"))?;

                // Return empty annotations for now
                Ok(Response::new(GetAnnotationsResponse {
                    buffer_id: buffer_id.as_usize() as u64,
                    annotations: Vec::<LineAnnotation>::new(),
                }))
            })
            .await
    }

    /// List all open buffers.
    #[allow(clippy::significant_drop_tightening)]
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
mod tests {
    use super::*;

    fn test_registry() -> Arc<SessionRegistry> {
        let registry = Arc::new(SessionRegistry::new());
        let session = Arc::new(Session::new(SessionId::new("test")));
        registry.insert(&session);
        registry
    }

    /// Create a registry with a session that has a real buffer manager.
    fn test_registry_with_buffer_manager() -> (Arc<SessionRegistry>, Arc<Session>) {
        use {
            parking_lot::RwLock as ParkingLotRwLock,
            reovim_driver_buffer::TestBufferManager,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, RegisterBank,
                ServiceRegistry, TextObjectEngine,
            },
        };

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(ParkingLotRwLock::new(RegisterBank::new())),
            Arc::new(ParkingLotRwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            Arc::new(ServiceRegistry::new()),
        );

        let state = crate::session::SessionState::with_kernel(kernel);
        let session = Arc::new(Session::from_state(SessionId::new("test"), state));

        let registry = Arc::new(SessionRegistry::new());
        registry.insert(&session);

        (registry, session)
    }

    #[tokio::test]
    async fn test_get_raw_content_no_session() {
        let registry = Arc::new(SessionRegistry::new());
        let service = BufferServiceImpl::new(registry, SessionId::new("nonexistent"));

        let request = Request::new(GetRawContentRequest {
            buffer_id: None,
            start_line: None,
            end_line: None,
        });
        let response = service.get_raw_content(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_get_raw_content_no_buffer() {
        let registry = test_registry();
        let service = BufferServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetRawContentRequest {
            buffer_id: None,
            start_line: None,
            end_line: None,
        });
        let response = service.get_raw_content(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_get_raw_content_with_buffer() {
        let (registry, session) = test_registry_with_buffer_manager();

        session
            .with_state_mut(|state| {
                state.create_buffer("hello\nworld");
            })
            .await;

        let service = BufferServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetRawContentRequest {
            buffer_id: None,
            start_line: None,
            end_line: None,
        });
        let response = service.get_raw_content(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.lines.len(), 2);
        assert_eq!(resp.lines[0], "hello");
        assert_eq!(resp.lines[1], "world");
    }

    #[tokio::test]
    async fn test_get_line_count_no_buffer() {
        let registry = test_registry();
        let service = BufferServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetLineCountRequest { buffer_id: None });
        let response = service.get_line_count(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_get_line_count_with_buffer() {
        let (registry, session) = test_registry_with_buffer_manager();

        session
            .with_state_mut(|state| {
                state.create_buffer("line1\nline2\nline3");
            })
            .await;

        let service = BufferServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(GetLineCountRequest { buffer_id: None });
        let response = service.get_line_count(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.line_count, 3);
    }

    #[tokio::test]
    async fn test_list_buffers_empty() {
        let registry = test_registry();
        let service = BufferServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(ListBuffersRequest {});
        let response = service.list(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(resp.buffers.is_empty());
    }

    #[tokio::test]
    async fn test_list_buffers_with_buffer() {
        let (registry, session) = test_registry_with_buffer_manager();

        session
            .with_state_mut(|state| {
                state.create_buffer("content");
            })
            .await;

        let service = BufferServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(ListBuffersRequest {});
        let response = service.list(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert_eq!(resp.buffers.len(), 1);
        assert_eq!(resp.buffers[0].line_count, 1);
    }

    #[tokio::test]
    async fn test_open_file_unimplemented() {
        let registry = test_registry();
        let service = BufferServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(OpenFileRequest {
            path: "test.txt".to_string(),
        });
        let response = service.open_file(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
    }

    #[tokio::test]
    async fn test_write_file_unimplemented() {
        let registry = test_registry();
        let service = BufferServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(WriteFileRequest {
            buffer_id: Some(0),
            path: None,
        });
        let response = service.write_file(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
    }

    #[tokio::test]
    async fn test_set_content_unimplemented() {
        let registry = test_registry();
        let service = BufferServiceImpl::new(registry, SessionId::new("test"));

        let request = Request::new(SetContentRequest {
            buffer_id: Some(0),
            content: "test".to_string(),
        });
        let response = service.set_content(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
    }
}
