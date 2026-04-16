//! `BufferService` gRPC implementation.
//!
//! # Architecture (#753 E6)
//!
//! Buffer content access and codec operations are domain-owned. The server
//! no longer imports driver crates directly. Buffer queries route through
//! `DomainDriver` or subsystem contracts.
//!
//! Codec RPCs (mount, unmount, switch_view, list_mounts, list_available_codecs)
//! are stubbed pending domain driver wiring.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_kernel::api::v1::BufferId,
    reovim_protocol::v2::{
        GetAnnotationsRequest, GetAnnotationsResponse, GetCodecViewsRequest,
        GetCodecViewsResponse, GetLineCountRequest, GetLineCountResponse, GetRawContentRequest,
        GetRawContentResponse, LineAnnotation, ListAvailableCodecsRequest,
        ListAvailableCodecsResponse, ListBuffersRequest, ListBuffersResponse, ListMountsRequest,
        ListMountsResponse, MountCodecRequest, MountCodecResponse, OpenFileRequest,
        OpenFileResponse, SetContentRequest, SetContentResponse, SwitchCodecViewRequest,
        SwitchCodecViewResponse, UmountCodecRequest, UmountCodecResponse, WriteFileRequest,
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
    ///
    /// TODO(#753 E6): Route through DomainDriver / BufferContentProvider.
    async fn get_raw_content(
        &self,
        _request: Request<GetRawContentRequest>,
    ) -> Result<Response<GetRawContentResponse>, Status> {
        Err(Status::unimplemented(
            "GetRawContent: pending domain driver wiring (#753 E6)",
        ))
    }

    /// Get the line count of a buffer.
    ///
    /// TODO(#753 E6): Route through DomainDriver / BufferContentProvider.
    async fn get_line_count(
        &self,
        _request: Request<GetLineCountRequest>,
    ) -> Result<Response<GetLineCountResponse>, Status> {
        Err(Status::unimplemented(
            "GetLineCount: pending domain driver wiring (#753 E6)",
        ))
    }

    /// Get annotations for a buffer.
    async fn get_annotations(
        &self,
        request: Request<GetAnnotationsRequest>,
    ) -> Result<Response<GetAnnotationsResponse>, Status> {
        let client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        let client_active = client_id.and_then(|cid| session.active_buffer_for_client(cid));

        session
            .with_state(|state| {
                let buffer_id = req
                    .buffer_id
                    .map(|id| BufferId::from_raw(id as usize))
                    .or(client_active)
                    .or_else(|| state.app.kernel.buffers.list().first().copied())
                    .ok_or_else(|| Status::not_found("No active buffer"))?;

                Ok(Response::new(GetAnnotationsResponse {
                    buffer_id: buffer_id.as_usize() as u64,
                    annotations: Vec::<LineAnnotation>::new(),
                }))
            })
            .await
    }

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

    /// Get available codec views for a buffer.
    ///
    /// TODO(#753 E6): Route through DomainDriver.
    async fn get_codec_views(
        &self,
        _request: Request<GetCodecViewsRequest>,
    ) -> Result<Response<GetCodecViewsResponse>, Status> {
        Err(Status::unimplemented(
            "GetCodecViews: pending domain driver wiring (#753 E6)",
        ))
    }

    /// Switch the active codec view for a buffer.
    ///
    /// TODO(#753 E6): Route through DomainDriver.
    async fn switch_codec_view(
        &self,
        _request: Request<SwitchCodecViewRequest>,
    ) -> Result<Response<SwitchCodecViewResponse>, Status> {
        Err(Status::unimplemented(
            "SwitchCodecView: pending domain driver wiring (#753 E6)",
        ))
    }

    /// Mount a codec on a buffer's inode.
    ///
    /// TODO(#753 E6): Route through DomainDriver.
    async fn mount_codec(
        &self,
        _request: Request<MountCodecRequest>,
    ) -> Result<Response<MountCodecResponse>, Status> {
        Err(Status::unimplemented(
            "MountCodec: pending domain driver wiring (#753 E6)",
        ))
    }

    /// Unmount a previously-registered codec mount.
    ///
    /// TODO(#753 E6): Route through DomainDriver.
    async fn umount_codec(
        &self,
        _request: Request<UmountCodecRequest>,
    ) -> Result<Response<UmountCodecResponse>, Status> {
        Err(Status::unimplemented(
            "UmountCodec: pending domain driver wiring (#753 E6)",
        ))
    }

    /// List every active mount on a buffer's inode.
    ///
    /// TODO(#753 E6): Route through DomainDriver.
    async fn list_mounts(
        &self,
        _request: Request<ListMountsRequest>,
    ) -> Result<Response<ListMountsResponse>, Status> {
        Err(Status::unimplemented(
            "ListMounts: pending domain driver wiring (#753 E6)",
        ))
    }

    /// List every codec factory the server has registered.
    ///
    /// TODO(#753 E6): Route through DomainDriver.
    async fn list_available_codecs(
        &self,
        _request: Request<ListAvailableCodecsRequest>,
    ) -> Result<Response<ListAvailableCodecsResponse>, Status> {
        Err(Status::unimplemented(
            "ListAvailableCodecs: pending domain driver wiring (#753 E6)",
        ))
    }
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
