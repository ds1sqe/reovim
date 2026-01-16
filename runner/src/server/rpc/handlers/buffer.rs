//! Buffer operation RPC handlers.
//!
//! Handlers for `buffer/get_content`, `buffer/set_content`, `buffer/list`, `buffer/open_file`.

use {
    reovim_kernel::api::v1::{Buffer, BufferId},
    reovim_protocol::v1::{
        BufferContentResult, BufferGetContentParams, BufferInfo, BufferListResult,
        BufferOpenFileParams, BufferSetContentParams, OkResult, RpcError,
    },
};

use {
    super::super::dispatcher::{HandlerFuture, RpcContext},
    crate::session::{StateSnapshot, emit_state_changes},
};

/// Handler for `buffer/get_content` method.
///
/// Returns the content of a buffer.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "buffer/get_content", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"content": "Hello, World!"}}
/// ```
///
/// # Panics
///
/// This function will not panic as `BufferContentResult` serialization is infallible.
#[must_use]
pub fn buffer_get_content(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: BufferGetContentParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        let content = ctx
            .session
            .with_state(|state| {
                // Use provided buffer_id or fall back to active buffer
                let buffer_id = params
                    .buffer_id
                    .map(BufferId::from_raw)
                    .or(state.app.active_buffer)
                    .ok_or_else(|| RpcError::invalid_params("No active buffer"))?;

                let buffer_arc = state.app.kernel.buffers.get(buffer_id).ok_or_else(|| {
                    RpcError::invalid_params(format!("Buffer {} not found", buffer_id.as_usize()))
                })?;

                Ok(buffer_arc.read().content())
            })
            .await?;

        Ok(serde_json::to_value(BufferContentResult { content })
            .expect("BufferContentResult serialization cannot fail"))
    })
}

/// Handler for `buffer/set_content` method.
///
/// Sets the content of a buffer.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "buffer/set_content", "params": {"content": "New content"}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"ok": true}}
/// ```
///
/// # Panics
///
/// This function will not panic as `OkResult` serialization is infallible.
#[must_use]
pub fn buffer_set_content(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: BufferSetContentParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        // Capture state before modification
        let before = ctx.session.with_state(StateSnapshot::capture).await;

        ctx.session
            .with_state_mut(|state| {
                let buffer_id = params
                    .buffer_id
                    .map(BufferId::from_raw)
                    .or(state.app.active_buffer)
                    .ok_or_else(|| RpcError::invalid_params("No active buffer"))?;

                let buffer_arc = state.app.kernel.buffers.get(buffer_id).ok_or_else(|| {
                    RpcError::invalid_params(format!("Buffer {} not found", buffer_id.as_usize()))
                })?;

                // Replace buffer content
                let mut buffer = buffer_arc.write();
                buffer.set_content(&params.content);
                buffer.set_modified(true);
                drop(buffer);
                Ok(())
            })
            .await?;

        // Capture state after modification and emit changes
        let after = ctx.session.with_state(StateSnapshot::capture).await;
        emit_state_changes(&ctx.session, &before, &after).await;

        Ok(serde_json::to_value(OkResult::new()).expect("OkResult serialization cannot fail"))
    })
}

/// Handler for `buffer/list` method.
///
/// Lists all open buffers.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "buffer/list", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"buffers": [{"id": 1, "modified": false, "line_count": 10}]}}
/// ```
///
/// # Panics
///
/// This function will not panic as `BufferListResult` serialization is infallible.
#[must_use]
pub fn buffer_list(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let buffers = ctx
            .session
            .with_state(|state| {
                state
                    .app
                    .kernel
                    .buffers
                    .list()
                    .iter()
                    .filter_map(|&id| {
                        state.app.kernel.buffers.get(id).map(|arc| {
                            let buf = arc.read();
                            BufferInfo {
                                id: id.as_usize(),
                                file_path: buf.file_path().map(String::from),
                                modified: buf.is_modified(),
                                line_count: buf.line_count(),
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .await;

        Ok(serde_json::to_value(BufferListResult { buffers })
            .expect("BufferListResult serialization cannot fail"))
    })
}

/// Handler for `buffer/open_file` method.
///
/// Opens a file into a new buffer and loads any existing undo history.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "buffer/open_file", "params": {"path": "/tmp/test.txt"}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"buffer_id": 1, "undo_loaded": true}}
/// ```
///
/// # Panics
///
/// This function will not panic as `BufferOpenResult` serialization is infallible.
#[must_use]
pub fn buffer_open_file(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: BufferOpenFileParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        // Read file content
        let content = std::fs::read_to_string(&params.path)
            .map_err(|e| RpcError::internal_error(format!("Failed to read file: {e}")))?;

        // Capture state before modification
        let before = ctx.session.with_state(StateSnapshot::capture).await;

        // Create buffer, register, and load undo history
        let (buffer_id, undo_loaded) = ctx
            .session
            .with_state_mut(|state| {
                let mut buffer = Buffer::from_string(&content);
                buffer.set_file_path(Some(params.path.clone()));
                let id = state.app.kernel.buffers.register(buffer);
                state.app.active_buffer = Some(id);

                // Load undo history from disk (graceful: log errors, don't fail)
                let loaded = state.app.undo_registry.load_graceful(
                    id,
                    &params.path,
                    &state.undo_persistence,
                    state.vfs.as_ref(),
                );

                (id, loaded)
            })
            .await;

        // Capture state after modification and emit changes
        let after = ctx.session.with_state(StateSnapshot::capture).await;
        emit_state_changes(&ctx.session, &before, &after).await;

        Ok(serde_json::json!({
            "buffer_id": buffer_id.as_usize(),
            "undo_loaded": undo_loaded
        }))
    })
}

/// Parameters for `buffer/write_file` method.
#[derive(Debug, serde::Deserialize)]
pub struct BufferWriteFileParams {
    /// Optional buffer ID (defaults to active buffer).
    #[serde(default)]
    pub buffer_id: Option<usize>,
    /// Optional file path to write to (defaults to buffer's path).
    #[serde(default)]
    pub path: Option<String>,
}

/// Handler for `buffer/write_file` method.
///
/// Writes buffer content to disk and persists undo history.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "buffer/write_file", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"ok": true, "path": "/path/to/file", "undo_saved": true}}
/// ```
///
/// # Errors
///
/// Returns `invalid_params` if no active buffer or buffer has no path and none provided.
#[must_use]
pub fn buffer_write_file(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: BufferWriteFileParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        // Get buffer content, path, and buffer ID
        let (buffer_id, content, file_path) = ctx
            .session
            .with_state(|state| {
                let buffer_id = params
                    .buffer_id
                    .map(BufferId::from_raw)
                    .or(state.app.active_buffer)
                    .ok_or_else(|| RpcError::invalid_params("No active buffer"))?;

                let buffer_arc = state.app.kernel.buffers.get(buffer_id).ok_or_else(|| {
                    RpcError::invalid_params(format!("Buffer {} not found", buffer_id.as_usize()))
                })?;

                let (content, buffer_file_path) = {
                    let buffer = buffer_arc.read();
                    (buffer.content(), buffer.file_path().map(String::from))
                };

                // Use provided path or buffer's path
                let file_path = params
                    .path
                    .clone()
                    .or(buffer_file_path)
                    .ok_or_else(|| RpcError::invalid_params("No file path specified"))?;

                Ok((buffer_id, content, file_path))
            })
            .await?;

        // Write file content to disk
        std::fs::write(&file_path, &content)
            .map_err(|e| RpcError::internal_error(format!("Failed to write file: {e}")))?;

        // Persist undo history and clear modified flag
        let undo_saved = ctx
            .session
            .with_state_mut(|state| {
                // Clear modified flag
                if let Some(buffer_arc) = state.app.kernel.buffers.get(buffer_id) {
                    buffer_arc.write().set_modified(false);
                }

                // Persist undo history (log errors, don't fail the write)
                match state.app.undo_registry.persist(
                    buffer_id,
                    &file_path,
                    &state.undo_persistence,
                    state.vfs.as_ref(),
                ) {
                    Ok(()) => true,
                    Err(e) => {
                        tracing::warn!("Failed to persist undo history for '{}': {}", file_path, e);
                        false
                    }
                }
            })
            .await;

        Ok(serde_json::json!({
            "ok": true,
            "path": file_path,
            "undo_saved": undo_saved
        }))
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use {
        super::*,
        crate::{
            server::rpc::handlers::test_utils::test_ctx_with_session,
            session::{Session, SessionId},
        },
        reovim_arch::sync::RwLock,
        reovim_driver_vfs::{MockVfs, VfsDriver},
        reovim_kernel::api::v1::{BufferError, BufferManager, KernelContext, ModeId, ModuleId},
        std::collections::HashMap,
    };

    fn test_vfs() -> Arc<dyn VfsDriver> {
        Arc::new(MockVfs::new())
    }

    /// Test-only buffer manager that actually stores buffers.
    struct TestBufferManager {
        buffers: RwLock<HashMap<BufferId, Arc<RwLock<Buffer>>>>,
    }

    impl TestBufferManager {
        fn new() -> Self {
            Self {
                buffers: RwLock::new(HashMap::new()),
            }
        }
    }

    impl BufferManager for TestBufferManager {
        fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
            self.buffers.read().get(&id).cloned()
        }

        fn create(&self) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(Buffer::new()));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn register(&self, buffer: Buffer) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(buffer));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn unregister(&self, id: BufferId) -> Result<Buffer, BufferError> {
            self.buffers
                .write()
                .remove(&id)
                .map_or(Err(BufferError::NotFound(id)), |arc_buffer| {
                    Arc::try_unwrap(arc_buffer)
                        .map_or_else(|arc| Ok(arc.read().clone()), |rwlock| Ok(rwlock.into_inner()))
                })
        }

        fn list(&self) -> Vec<BufferId> {
            self.buffers.read().keys().copied().collect()
        }

        fn count(&self) -> usize {
            self.buffers.read().len()
        }
    }

    /// Create a `KernelContext` with a real buffer manager for testing.
    fn test_kernel() -> KernelContext {
        use reovim_kernel::api::v1::{
            EventBus, MarkBank, MotionEngine, OptionRegistry, RegisterBank, TextObjectEngine,
        };

        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(RegisterBank::new())),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
        )
    }

    fn test_session() -> Arc<Session> {
        Session::new(
            SessionId::new("test"),
            test_kernel(),
            ModeId::new(ModuleId::new("test"), "normal"),
            test_vfs(),
        )
    }

    /// Create a test session with a buffer containing the given content.
    async fn test_session_with_buffer(content: &str) -> (Arc<Session>, BufferId) {
        let session = test_session();
        let content_owned = content.to_string();
        let buffer_id = session
            .with_state_mut(|state| {
                let buffer = Buffer::from_string(&content_owned);
                let id = state.app.kernel.buffers.register(buffer);
                state.app.active_buffer = Some(id);
                id
            })
            .await;
        (session, buffer_id)
    }

    #[tokio::test]
    async fn test_buffer_list_empty() {
        let session = test_session();
        let ctx = test_ctx_with_session(session);

        let result = buffer_list(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        let buffers = value.get("buffers").unwrap().as_array().unwrap();
        assert!(buffers.is_empty());
    }

    #[tokio::test]
    async fn test_buffer_list_with_buffers() {
        let (session, buffer_id) = test_session_with_buffer("Hello\nWorld").await;
        let ctx = test_ctx_with_session(session);

        let result = buffer_list(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        let buffers = value.get("buffers").unwrap().as_array().unwrap();
        assert_eq!(buffers.len(), 1);

        let buf = &buffers[0];
        assert_eq!(buf.get("id").unwrap().as_u64().unwrap(), buffer_id.as_usize() as u64);
        assert_eq!(buf.get("line_count").unwrap().as_u64().unwrap(), 2);
        assert!(!buf.get("modified").unwrap().as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_buffer_get_content_no_active() {
        let session = test_session();
        let ctx = test_ctx_with_session(session);

        // No active buffer
        let result = buffer_get_content(ctx, serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_buffer_get_content_success() {
        let (session, _buffer_id) = test_session_with_buffer("Hello, World!").await;
        let ctx = test_ctx_with_session(session);

        let result = buffer_get_content(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value.get("content").unwrap().as_str().unwrap(), "Hello, World!");
    }

    #[tokio::test]
    async fn test_buffer_get_content_with_explicit_id() {
        let (session, buffer_id) = test_session_with_buffer("Explicit ID test").await;
        let ctx = test_ctx_with_session(session);

        let result =
            buffer_get_content(ctx, serde_json::json!({"buffer_id": buffer_id.as_usize()})).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value.get("content").unwrap().as_str().unwrap(), "Explicit ID test");
    }

    #[tokio::test]
    async fn test_buffer_get_content_invalid_id() {
        let (session, _buffer_id) = test_session_with_buffer("Test").await;
        let ctx = test_ctx_with_session(session);

        // Use an invalid buffer ID (99999)
        let result = buffer_get_content(ctx, serde_json::json!({"buffer_id": 99999})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_buffer_set_content_no_active() {
        let session = test_session();
        let ctx = test_ctx_with_session(session);

        // No active buffer
        let result = buffer_set_content(ctx, serde_json::json!({"content": "test"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_buffer_set_content_success() {
        let (session, _buffer_id) = test_session_with_buffer("Original content").await;

        // Set content
        let ctx = test_ctx_with_session(Arc::clone(&session));
        let result = buffer_set_content(ctx, serde_json::json!({"content": "New content"})).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("ok").unwrap().as_bool().unwrap());

        // Verify the content was changed
        let ctx2 = test_ctx_with_session(session);
        let result2 = buffer_get_content(ctx2, serde_json::json!({})).await;
        assert!(result2.is_ok());
        let value2 = result2.unwrap();
        assert_eq!(value2.get("content").unwrap().as_str().unwrap(), "New content");
    }

    #[tokio::test]
    async fn test_buffer_set_content_marks_modified() {
        let (session, _buffer_id) = test_session_with_buffer("Original").await;

        // Set content
        let ctx = test_ctx_with_session(Arc::clone(&session));
        let _result = buffer_set_content(ctx, serde_json::json!({"content": "Modified"})).await;

        // Check buffer is marked as modified
        let ctx2 = test_ctx_with_session(session);
        let result = buffer_list(ctx2, serde_json::json!({})).await;
        let value = result.unwrap();
        let buffers = value.get("buffers").unwrap().as_array().unwrap();
        assert!(buffers[0].get("modified").unwrap().as_bool().unwrap());
    }
}
