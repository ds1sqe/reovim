//! Screen content RPC handler.
//!
//! Handler for `state/screen_content` method.
//!
//! Note: The headless server doesn't have an actual terminal screen.
//! This handler returns the content of the active buffer formatted
//! according to the requested format.

use {
    reovim_kernel::api::v1::BufferId,
    reovim_protocol::v1::{RpcError, ScreenContentResult, ScreenFormat, StateScreenContentParams},
};

use super::super::dispatcher::{HandlerFuture, RpcContext};

/// Handler for `state/screen_content` method.
///
/// Returns screen content in requested format (`plain_text`, `raw_ansi`, `cell_grid`).
///
/// Since the headless server doesn't have an actual terminal screen,
/// this returns the active buffer's content formatted according to
/// the requested format.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "state/screen_content", "params": {"format": "plain_text"}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"width": 80, "height": 24, "format": "plain_text", "content": "..."}}
/// ```
///
/// # Panics
///
/// This function will not panic as `ScreenContentResult` serialization is infallible.
#[must_use]
pub fn state_screen_content(ctx: RpcContext, params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let params: StateScreenContentParams =
            serde_json::from_value(params).map_err(|e| RpcError::invalid_params(e.to_string()))?;

        let result = ctx
            .session
            .with_state(|state| {
                // Get active buffer content as screen representation
                let content = state
                    .app
                    .active_buffer
                    .and_then(|id: BufferId| state.app.kernel.buffers.get(id))
                    .map(|arc| {
                        let buf = arc.read();
                        buf.content()
                    })
                    .unwrap_or_default();

                // Calculate dimensions from content
                let lines: Vec<&str> = content.lines().collect();
                #[allow(clippy::cast_possible_truncation)]
                let height = lines.len().min(usize::from(u16::MAX)) as u16;
                #[allow(clippy::cast_possible_truncation)]
                let width = lines
                    .iter()
                    .map(|l| l.len())
                    .max()
                    .unwrap_or(80)
                    .min(usize::from(u16::MAX)) as u16;

                // Format content according to requested format
                let formatted_content = match params.format {
                    ScreenFormat::PlainText => content,
                    ScreenFormat::RawAnsi => {
                        // For headless server, just return plain text
                        // Real ANSI would require a terminal emulator
                        content
                    }
                    ScreenFormat::CellGrid => {
                        // Return JSON cell grid representation
                        let cells: Vec<Vec<serde_json::Value>> = lines
                            .iter()
                            .map(|line| {
                                line.chars()
                                    .map(|c| {
                                        serde_json::json!({
                                            "char": c.to_string(),
                                            "fg": "default",
                                            "bg": "default"
                                        })
                                    })
                                    .collect()
                            })
                            .collect();
                        serde_json::to_string(&cells).unwrap_or_else(|_| "[]".to_string())
                    }
                };

                ScreenContentResult {
                    width: width.max(1),
                    height: height.max(1),
                    format: params.format,
                    content: formatted_content,
                }
            })
            .await;

        Ok(serde_json::to_value(result).expect("ScreenContentResult serialization cannot fail"))
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
        reovim_kernel::api::v1::{
            Buffer, BufferError, BufferManager, KernelContext, ModeId, ModuleId,
        },
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
    async fn test_session_with_buffer(content: &str) -> Arc<Session> {
        let session = test_session();
        let content_owned = content.to_string();
        session
            .with_state_mut(|state| {
                let buffer = Buffer::from_string(&content_owned);
                let id = state.app.kernel.buffers.register(buffer);
                state.app.active_buffer = Some(id);
            })
            .await;
        session
    }

    #[tokio::test]
    async fn test_screen_content_default_format() {
        let session = test_session();
        let ctx = test_ctx_with_session(session);

        // Default format (plain_text)
        let result = state_screen_content(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("width").is_some());
        assert!(value.get("height").is_some());
        assert!(value.get("format").is_some());
        assert!(value.get("content").is_some());
    }

    #[tokio::test]
    async fn test_screen_content_cell_grid() {
        let session = test_session();
        let ctx = test_ctx_with_session(session);

        let result = state_screen_content(ctx, serde_json::json!({"format": "cell_grid"})).await;
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value.get("format").unwrap(), "cell_grid");
    }

    #[tokio::test]
    async fn test_screen_content_with_buffer() {
        let session = test_session_with_buffer("Hello\nWorld\nTest").await;
        let ctx = test_ctx_with_session(session);

        let result = state_screen_content(ctx, serde_json::json!({"format": "plain_text"})).await;
        assert!(result.is_ok());
        let value = result.unwrap();

        // Verify dimensions match buffer content
        assert_eq!(value.get("height").unwrap().as_u64().unwrap(), 3);
        assert_eq!(value.get("width").unwrap().as_u64().unwrap(), 5); // "World" is longest

        // Verify content
        let content = value.get("content").unwrap().as_str().unwrap();
        assert!(content.contains("Hello"));
        assert!(content.contains("World"));
    }

    #[tokio::test]
    async fn test_screen_content_cell_grid_structure() {
        let session = test_session_with_buffer("AB\nCD").await;
        let ctx = test_ctx_with_session(session);

        let result = state_screen_content(ctx, serde_json::json!({"format": "cell_grid"})).await;
        assert!(result.is_ok());
        let value = result.unwrap();

        // Cell grid content should be valid JSON
        let content = value.get("content").unwrap().as_str().unwrap();
        let cells: Vec<Vec<serde_json::Value>> =
            serde_json::from_str(content).expect("Cell grid should be valid JSON");

        // Should have 2 rows
        assert_eq!(cells.len(), 2);

        // First row should have 2 cells (A, B)
        assert_eq!(cells[0].len(), 2);
        assert_eq!(cells[0][0].get("char").unwrap().as_str().unwrap(), "A");
        assert_eq!(cells[0][1].get("char").unwrap().as_str().unwrap(), "B");

        // Second row should have 2 cells (C, D)
        assert_eq!(cells[1].len(), 2);
        assert_eq!(cells[1][0].get("char").unwrap().as_str().unwrap(), "C");
        assert_eq!(cells[1][1].get("char").unwrap().as_str().unwrap(), "D");
    }
}
