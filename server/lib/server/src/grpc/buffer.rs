//! `BufferService` gRPC implementation.
//!
//! Provides raw buffer content access for v2 protocol clients.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_driver_codec::{CodecSessionState, ContentCodec, ContentCodecFactoryStore},
    reovim_kernel::api::v1::BufferId,
    reovim_protocol::v2::{
        BufferInfo, CodecMetadata, CodecViewInfo, GetAnnotationsRequest, GetAnnotationsResponse,
        GetCodecViewsRequest, GetCodecViewsResponse, GetLineCountRequest, GetLineCountResponse,
        GetRawContentRequest, GetRawContentResponse, LineAnnotation, ListBuffersRequest,
        ListBuffersResponse, OpenFileRequest, OpenFileResponse, SetContentRequest,
        SetContentResponse, SwitchCodecViewRequest, SwitchCodecViewResponse, WriteFileRequest,
        WriteFileResponse, buffer_service_server::BufferService,
    },
    tonic::{Request, Response, Status},
};

use crate::session::{ClientId, Session, SessionId, SessionRegistry};

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
        // Per-client active_buffer (#471): extract client ID before consuming request.
        let client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        // Resolve buffer_id: explicit > per-client active > any in list
        let client_active = client_id.and_then(|cid| {
            session.with_clients(|clients| clients.get(&cid).and_then(|c| c.state.active_buffer))
        });

        session
            .with_state(|state| {
                let buffer_id = req
                    .buffer_id
                    .map(|id| BufferId::from_raw(id as usize))
                    .or(client_active)
                    .or_else(|| state.app.kernel.buffers.list().first().copied())
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
                    .filter_map(|i| buffer.line(i).map(std::borrow::Cow::into_owned))
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
        let client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        let client_active = client_id.and_then(|cid| {
            session.with_clients(|clients| clients.get(&cid).and_then(|c| c.state.active_buffer))
        });

        session
            .with_state(|state| {
                let buffer_id = req
                    .buffer_id
                    .map(|id| BufferId::from_raw(id as usize))
                    .or(client_active)
                    .or_else(|| state.app.kernel.buffers.list().first().copied())
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
        let client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        let client_active = client_id.and_then(|cid| {
            session.with_clients(|clients| clients.get(&cid).and_then(|c| c.state.active_buffer))
        });

        session
            .with_state(|state| {
                let buffer_id = req
                    .buffer_id
                    .map(|id| BufferId::from_raw(id as usize))
                    .or(client_active)
                    .or_else(|| state.app.kernel.buffers.list().first().copied())
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
                // Collect buffer IDs from unified manager
                let all_ids: Vec<BufferId> = state.app.kernel.buffers.list();

                let buffers: Vec<BufferInfo> = all_ids
                    .iter()
                    .filter_map(|&id| {
                        let buffer_arc = state.buffer(id)?;
                        let buffer = buffer_arc.read();
                        let file_path = buffer.file_path().map(String::from);
                        let name = file_path
                            .as_deref()
                            .and_then(|p| std::path::Path::new(p).file_name())
                            .map_or_else(
                                || format!("[Buffer {}]", id.as_usize()),
                                |n| n.to_string_lossy().into_owned(),
                            );
                        let codec_meta = state
                            .app
                            .extensions
                            .get::<CodecSessionState>()
                            .and_then(|css| css.get(id))
                            .map(|m| CodecMetadata {
                                codec_name: m
                                    .content_type()
                                    .as_str()
                                    .strip_prefix("text/")
                                    .unwrap_or_else(|| m.content_type().as_str())
                                    .to_string(),
                                line_ending: m.get("line_ending").map(String::from),
                                has_bom: m.get("bom") == Some("true"),
                            });
                        Some(BufferInfo {
                            id: id.as_usize() as u64,
                            name,
                            path: file_path,
                            line_count: buffer.line_count() as u64,
                            modified: buffer.is_modified(),
                            content_type: None,
                            readonly: None,
                            codec_metadata: codec_meta,
                            capabilities: buffer.buffer_capabilities().bits(),
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

    /// Get available codec views for a buffer.
    #[allow(clippy::cast_possible_truncation)]
    async fn get_codec_views(
        &self,
        request: Request<GetCodecViewsRequest>,
    ) -> Result<Response<GetCodecViewsResponse>, Status> {
        let client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        let client_active = client_id.and_then(|cid| {
            session.with_clients(|clients| clients.get(&cid).and_then(|c| c.state.active_buffer))
        });

        session
            .with_state(|state| {
                let buffer_id = req
                    .buffer_id
                    .map(|id| BufferId::from_raw(id as usize))
                    .or(client_active)
                    .or_else(|| state.app.kernel.buffers.list().first().copied())
                    .ok_or_else(|| Status::not_found("No active buffer"))?;

                let codec_state = state
                    .app
                    .extensions
                    .get::<CodecSessionState>()
                    .ok_or_else(|| Status::not_found("No codec state"))?;

                let metadata = codec_state
                    .get(buffer_id)
                    .ok_or_else(|| Status::not_found("No codec metadata for buffer"))?;

                let content_type = metadata.content_type().clone();
                let active_view = codec_state
                    .active_view(buffer_id)
                    .unwrap_or("default")
                    .to_string();

                // Find the codec factory and get views
                let factory_store = state.app.kernel.services.get::<ContentCodecFactoryStore>();
                let views = factory_store
                    .and_then(|store| store.find(&content_type))
                    .map(|codec| {
                        codec
                            .views()
                            .iter()
                            .map(|v| CodecViewInfo {
                                name: v.name.to_string(),
                                display: v.display.to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                Ok(Response::new(GetCodecViewsResponse {
                    buffer_id: buffer_id.as_usize() as u64,
                    views,
                    active_view,
                }))
            })
            .await
    }

    /// Switch the active codec view for a buffer.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::significant_drop_tightening)]
    async fn switch_codec_view(
        &self,
        request: Request<SwitchCodecViewRequest>,
    ) -> Result<Response<SwitchCodecViewResponse>, Status> {
        let client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        let client_active = client_id.and_then(|cid| {
            session.with_clients(|clients| clients.get(&cid).and_then(|c| c.state.active_buffer))
        });

        session
            .with_state_mut(|state| {
                let buffer_id = req
                    .buffer_id
                    .map(|id| BufferId::from_raw(id as usize))
                    .or(client_active)
                    .or_else(|| state.app.kernel.buffers.list().first().copied())
                    .ok_or_else(|| Status::not_found("No active buffer"))?;

                // Get canonical inode bytes and content type from codec state
                let codec_state = state
                    .app
                    .extensions
                    .get::<CodecSessionState>()
                    .ok_or_else(|| Status::not_found("No codec state"))?;

                let source_bytes = codec_state.bytes(buffer_id).ok_or_else(|| {
                    Status::failed_precondition("No canonical inode bytes for buffer")
                })?;

                let content_type = codec_state
                    .get(buffer_id)
                    .ok_or_else(|| Status::not_found("No codec metadata for buffer"))?
                    .content_type()
                    .clone();

                // Find the codec and validate the view name
                let factory_store = state.app.kernel.services.get::<ContentCodecFactoryStore>();
                let codec = factory_store
                    .and_then(|store| store.find(&content_type))
                    .ok_or_else(|| Status::not_found("No codec for content type"))?;
                let codec: Arc<dyn ContentCodec> = codec.into();

                let view_name = &req.view_name;
                if !codec.views().iter().any(|v| v.name == view_name) {
                    return Ok(Response::new(SwitchCodecViewResponse {
                        ok: false,
                        error: Some(format!("View '{view_name}' not available")),
                    }));
                }

                // Decode with the requested view
                let result = codec
                    .decode_view(&source_bytes, view_name)
                    .map_err(|e| Status::internal(format!("Codec decode_view failed: {e}")))?;

                // Update buffer content (Rope buffers only — codec views are text)
                let buffer_arc = state.buffer(buffer_id).ok_or_else(|| {
                    Status::not_found(format!("Buffer {} not found", buffer_id.as_usize()))
                })?;

                {
                    let mut buffer = buffer_arc.write();
                    buffer.set_content(&result.content);
                    buffer.set_modified(false);
                }

                // Update codec state and rebuild index from new raw bytes (#740 D.3)
                if let Some(codec_state) = state.app.extensions.get_mut::<CodecSessionState>() {
                    codec_state.insert(buffer_id, result.metadata);
                    codec_state.set_active_view(buffer_id, view_name.clone());
                    codec_state.set_source_with_codec(buffer_id, source_bytes, codec);
                    // Rebuild the codec index from the raw bytes for the new view.
                    // The old index is stale after a view switch — re-build rather
                    // than incremental update since the entire content changed.
                    if codec_state.has_index(buffer_id) {
                        codec_state.remove_index(buffer_id);
                    }
                }

                Ok(Response::new(SwitchCodecViewResponse {
                    ok: true,
                    error: None,
                }))
            })
            .await
    }
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
