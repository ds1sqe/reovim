//! `BufferService` gRPC implementation.
//!
//! Provides raw buffer content access for v2 protocol clients.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::sync::Arc;

use {
    reovim_driver_codec::{
        CodecSessionState, ContentCodecFactoryStore, ContentType, MountCodecError, MountId,
        MountMode, SwitchViewError,
    },
    reovim_kernel::api::v1::BufferId,
    reovim_protocol::v2::{
        AvailableCodec, BufferInfo, CodecMetadata, CodecViewInfo, GetAnnotationsRequest,
        GetAnnotationsResponse, GetCodecViewsRequest, GetCodecViewsResponse, GetLineCountRequest,
        GetLineCountResponse, GetRawContentRequest, GetRawContentResponse, LineAnnotation,
        ListAvailableCodecsRequest, ListAvailableCodecsResponse, ListBuffersRequest,
        ListBuffersResponse, ListMountsRequest, ListMountsResponse, MountCodecRequest,
        MountCodecResponse, MountInfo as ProtoMountInfo, OpenFileRequest, OpenFileResponse,
        SetContentRequest, SetContentResponse, SwitchCodecViewRequest, SwitchCodecViewResponse,
        UmountCodecRequest, UmountCodecResponse, WriteFileRequest, WriteFileResponse,
        buffer_service_server::BufferService,
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

    /// Post-switch buffer lookup error — only reachable if the buffer is
    /// removed from the kernel between the codec `switch_view` call and the
    /// subsequent `state.buffer()` lookup. This is a TOCTOU race that cannot
    /// be reproduced in unit tests.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn switch_view_buffer_not_found(buffer_id: BufferId) -> Status {
        Status::not_found(format!("Buffer {} not found", buffer_id.as_usize()))
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
            session
                .clients()
                .with_clients(|clients| clients.get(&cid).and_then(|c| c.state.active_buffer))
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
            session
                .clients()
                .with_clients(|clients| clients.get(&cid).and_then(|c| c.state.active_buffer))
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
            session
                .clients()
                .with_clients(|clients| clients.get(&cid).and_then(|c| c.state.active_buffer))
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
            session
                .clients()
                .with_clients(|clients| clients.get(&cid).and_then(|c| c.state.active_buffer))
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
    ///
    /// **Deprecated** — scheduled for removal in v0.11.0. Clients should
    /// use `MountCodec` / `UmountCodec` / `ListMounts` /
    /// `ListAvailableCodecs` directly. This handler is retained as a
    /// compatibility shim that still routes through
    /// [`CodecSessionState::switch_view`] so existing test suites keep
    /// passing while the multi-mount wire protocol rolls out.
    ///
    /// The handler is strictly a dispatcher: resolve the buffer, confirm
    /// the codec-state and factory-store extensions exist, delegate the
    /// orchestration to [`CodecSessionState::switch_view`], and write the
    /// decoded content into the buffer. All codec-side mutation lives in
    /// the codec driver (see `#740` Plan 06 Phase 4).
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::significant_drop_tightening)]
    async fn switch_codec_view(
        &self,
        request: Request<SwitchCodecViewRequest>,
    ) -> Result<Response<SwitchCodecViewResponse>, Status> {
        tracing::warn!(
            "SwitchCodecView RPC is deprecated (#740 Plan 06 Phase 5 5c); use MountCodec / UmountCodec / ListMounts / ListAvailableCodecs — removal scheduled for v0.11.0"
        );
        let client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        let client_active = client_id.and_then(|cid| {
            session
                .clients()
                .with_clients(|clients| clients.get(&cid).and_then(|c| c.state.active_buffer))
        });

        session
            .with_state_mut(|state| {
                let buffer_id = req
                    .buffer_id
                    .map(|id| BufferId::from_raw(id as usize))
                    .or(client_active)
                    .or_else(|| state.app.kernel.buffers.list().first().copied())
                    .ok_or_else(|| Status::not_found("No active buffer"))?;

                // Preflight: CodecSessionState extension must exist; the
                // factory store is looked up as an Option and passed into
                // switch_view so the historic bytes-before-codec error
                // order is preserved.
                if state.app.extensions.get::<CodecSessionState>().is_none() {
                    return Err(Status::not_found("No codec state"));
                }
                let factories = state.app.kernel.services.get::<ContentCodecFactoryStore>();

                let content = {
                    let codec_state = state
                        .app
                        .extensions
                        .get_mut::<CodecSessionState>()
                        .ok_or_else(|| Status::not_found("No codec state"))?;

                    match codec_state.switch_view(factories.as_deref(), buffer_id, &req.view_name) {
                        Ok(content) => content,
                        Err(SwitchViewError::NoMetadata) => {
                            return Err(Status::not_found("No codec metadata for buffer"));
                        }
                        Err(SwitchViewError::NoCanonicalBytes) => {
                            return Err(Status::failed_precondition(
                                "No canonical inode bytes for buffer",
                            ));
                        }
                        Err(SwitchViewError::NoCodec) => {
                            return Err(Status::not_found("No codec for content type"));
                        }
                        Err(err @ SwitchViewError::ViewNotAvailable { .. }) => {
                            return Ok(Response::new(SwitchCodecViewResponse {
                                ok: false,
                                error: Some(err.to_string()),
                            }));
                        }
                        Err(SwitchViewError::DecodeFailed { reason }) => {
                            return Err(Status::internal(format!(
                                "Codec decode_view failed: {reason}"
                            )));
                        }
                    }
                };

                let buffer_arc = state
                    .buffer(buffer_id)
                    .ok_or_else(|| Self::switch_view_buffer_not_found(buffer_id))?;
                {
                    let mut buffer = buffer_arc.write();
                    buffer.set_content(&content);
                    buffer.set_modified(false);
                }

                Ok(Response::new(SwitchCodecViewResponse {
                    ok: true,
                    error: None,
                }))
            })
            .await
    }

    // ── Multi-mount RPCs (#740 Plan 06 Phase 5 sub-commit 5c) ─────────────

    /// Mount a codec on a buffer's inode.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::significant_drop_tightening)]
    async fn mount_codec(
        &self,
        request: Request<MountCodecRequest>,
    ) -> Result<Response<MountCodecResponse>, Status> {
        let client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        let client_active = client_id.and_then(|cid| {
            session
                .clients()
                .with_clients(|clients| clients.get(&cid).and_then(|c| c.state.active_buffer))
        });

        session
            .with_state_mut(|state| {
                let buffer_id = req
                    .buffer_id
                    .map(|id| BufferId::from_raw(id as usize))
                    .or(client_active)
                    .or_else(|| state.app.kernel.buffers.list().first().copied())
                    .ok_or_else(|| Status::not_found("No active buffer"))?;

                let factories = state
                    .app
                    .kernel
                    .services
                    .get::<ContentCodecFactoryStore>()
                    .ok_or_else(|| Status::not_found("No codec factory store"))?;

                let codec_state = state
                    .app
                    .extensions
                    .get_mut::<CodecSessionState>()
                    .ok_or_else(|| Status::not_found("No codec state"))?;

                let view_name = req
                    .view_name
                    .clone()
                    .unwrap_or_else(|| "default".to_string());

                // Phase 7 (#740): parse mount mode from proto.
                // 0 = SUMMARY (default), 1 = STRUCTURAL.
                let mode = if req.mount_mode == 1 {
                    MountMode::Structural
                } else {
                    MountMode::Summary
                };

                match codec_state.mount_codec(
                    &factories,
                    buffer_id,
                    &ContentType::new(req.content_type.clone()),
                    view_name,
                    mode,
                ) {
                    Ok(handle) => Ok(Response::new(MountCodecResponse {
                        ok: true,
                        error: None,
                        mount_id: Some(handle.mount_id().as_u64()),
                    })),
                    Err(MountCodecError::NoCanonicalBytes) => {
                        Err(Status::failed_precondition("No canonical inode bytes for buffer"))
                    }
                    // NoCodec + Mount merged: Mount is unreachable (counter
                    // overflow after ~18 quintillion mounts) but the enum is
                    // non-exhaustive, so the wildcard keeps the match exhaustive.
                    Err(err) => Err(Status::not_found(err.to_string())),
                }
            })
            .await
    }

    /// Unmount a previously-registered codec mount.
    async fn umount_codec(
        &self,
        request: Request<UmountCodecRequest>,
    ) -> Result<Response<UmountCodecResponse>, Status> {
        let req = request.into_inner();
        let session = self.get_session()?;

        let mount_id = MountId::from_u64(
            std::num::NonZeroU64::new(req.mount_id)
                .ok_or_else(|| Status::invalid_argument("mount_id must be non-zero"))?
                .get(),
        );

        session
            .with_state_mut(|state| {
                let codec_state = state
                    .app
                    .extensions
                    .get_mut::<CodecSessionState>()
                    .ok_or_else(|| Status::not_found("No codec state"))?;

                match codec_state.unmount_codec(mount_id) {
                    Ok(()) => Ok(Response::new(UmountCodecResponse {
                        ok: true,
                        error: None,
                    })),
                    // MountNotFound + Umount merged: Umount is unreachable
                    // (InodeTable unmount cannot fail once the mount exists).
                    Err(err) => Err(Status::not_found(err.to_string())),
                }
            })
            .await
    }

    /// List every active mount on a buffer's inode.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::significant_drop_tightening)]
    async fn list_mounts(
        &self,
        request: Request<ListMountsRequest>,
    ) -> Result<Response<ListMountsResponse>, Status> {
        let client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        let client_active = client_id.and_then(|cid| {
            session
                .clients()
                .with_clients(|clients| clients.get(&cid).and_then(|c| c.state.active_buffer))
        });

        session
            .with_state(|state| {
                let buffer_id = req
                    .buffer_id
                    .map(|id| BufferId::from_raw(id as usize))
                    .or(client_active)
                    .or_else(|| state.app.kernel.buffers.list().first().copied())
                    .ok_or_else(|| Status::not_found("No active buffer"))?;

                let mounts = state
                    .app
                    .extensions
                    .get::<CodecSessionState>()
                    .map(|cs| cs.list_mounts(buffer_id))
                    .unwrap_or_default()
                    .into_iter()
                    .map(|info| ProtoMountInfo {
                        mount_id: info.mount_id.as_u64(),
                        view_name: info.view_name,
                        content_valid: info.content_valid,
                        mode: match info.mode {
                            MountMode::Summary => 0,
                            MountMode::Structural => 1,
                        },
                    })
                    .collect();

                Ok(Response::new(ListMountsResponse {
                    buffer_id: buffer_id.as_usize() as u64,
                    mounts,
                }))
            })
            .await
    }

    /// List every codec factory the server has registered.
    async fn list_available_codecs(
        &self,
        _request: Request<ListAvailableCodecsRequest>,
    ) -> Result<Response<ListAvailableCodecsResponse>, Status> {
        let session = self.get_session()?;

        session
            .with_state(|state| {
                let codecs = state
                    .app
                    .kernel
                    .services
                    .get::<ContentCodecFactoryStore>()
                    .map(|store| {
                        store
                            .available()
                            .into_iter()
                            .map(|(name, content_types)| AvailableCodec {
                                name: name.to_string(),
                                content_types,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                Ok(Response::new(ListAvailableCodecsResponse { codecs }))
            })
            .await
    }
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
