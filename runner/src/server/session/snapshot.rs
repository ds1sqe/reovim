//! State snapshots for change detection.
//!
//! Used to detect state changes before/after command execution
//! and emit appropriate notifications to connected clients.

use reovim_kernel::api::v1::{BufferId, ModeId, Position};

use super::SessionState;

/// Snapshot of session state for change detection.
///
/// Captures the current state that we want to track for changes.
/// After command execution, we compare snapshots to determine
/// which notifications to emit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSnapshot {
    /// Current mode ID.
    pub mode: ModeId,
    /// Active buffer ID.
    pub active_buffer: Option<BufferId>,
    /// Cursor position in active buffer.
    pub cursor: Option<Position>,
    /// Modified flag of active buffer.
    pub modified: Option<bool>,
}

impl StateSnapshot {
    /// Capture current state from session.
    ///
    /// Takes a snapshot of the session state fields we want to
    /// track for change notification purposes.
    #[must_use]
    pub fn capture(state: &SessionState) -> Self {
        let mode = state.app.mode_stack.current().clone();
        let active_buffer = state.app.active_buffer;

        let (cursor, modified) = active_buffer
            .and_then(|id| state.app.kernel.buffers.get(id))
            .map_or((None, None), |arc| {
                let buf = arc.read();
                (Some(buf.position()), Some(buf.is_modified()))
            });

        Self {
            mode,
            active_buffer,
            cursor,
            modified,
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_arch::sync::RwLock,
        reovim_driver_vfs::{MockVfs, VfsDriver},
        reovim_kernel::api::v1::{Buffer, BufferError, BufferManager, KernelContext, ModuleId},
        std::{collections::HashMap, sync::Arc},
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

    fn test_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    #[test]
    fn test_snapshot_capture_empty_state() {
        let kernel = test_kernel();
        let state = SessionState::new(kernel, test_mode_id(), test_vfs());

        let snapshot = StateSnapshot::capture(&state);

        assert_eq!(snapshot.mode, test_mode_id());
        assert!(snapshot.active_buffer.is_none());
        assert!(snapshot.cursor.is_none());
        assert!(snapshot.modified.is_none());
    }

    #[test]
    fn test_snapshot_capture_with_buffer() {
        let kernel = test_kernel();
        let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

        // Create and register a buffer
        let buffer = Buffer::from_string("Hello\nWorld");
        let buffer_id = state.app.kernel.buffers.register(buffer);
        state.app.active_buffer = Some(buffer_id);

        let snapshot = StateSnapshot::capture(&state);

        // Should capture buffer state
        assert_eq!(snapshot.active_buffer, Some(buffer_id));
        assert!(snapshot.cursor.is_some());
        assert!(snapshot.modified.is_some());
        assert!(!snapshot.modified.unwrap()); // New buffer is not modified
    }

    #[test]
    fn test_snapshot_capture_modified_buffer() {
        let kernel = test_kernel();
        let mut state = SessionState::new(kernel, test_mode_id(), test_vfs());

        // Create buffer and mark as modified
        let mut buffer = Buffer::from_string("Test content");
        buffer.set_modified(true);
        let buffer_id = state.app.kernel.buffers.register(buffer);
        state.app.active_buffer = Some(buffer_id);

        let snapshot = StateSnapshot::capture(&state);

        // Modified flag should be captured
        assert_eq!(snapshot.modified, Some(true));
    }

    #[test]
    fn test_snapshot_equality() {
        let snapshot1 = StateSnapshot {
            mode: test_mode_id(),
            active_buffer: None,
            cursor: None,
            modified: None,
        };
        let snapshot2 = StateSnapshot {
            mode: test_mode_id(),
            active_buffer: None,
            cursor: None,
            modified: None,
        };

        assert_eq!(snapshot1, snapshot2);
    }

    #[test]
    fn test_snapshot_mode_difference() {
        let mode1 = ModeId::new(ModuleId::new("test"), "normal");
        let mode2 = ModeId::new(ModuleId::new("test"), "insert");

        let snapshot1 = StateSnapshot {
            mode: mode1,
            active_buffer: None,
            cursor: None,
            modified: None,
        };
        let snapshot2 = StateSnapshot {
            mode: mode2,
            active_buffer: None,
            cursor: None,
            modified: None,
        };

        assert_ne!(snapshot1, snapshot2);
    }

    #[test]
    fn test_snapshot_modified_difference() {
        let buffer_id = BufferId::new();

        let snapshot_unmodified = StateSnapshot {
            mode: test_mode_id(),
            active_buffer: Some(buffer_id),
            cursor: None,
            modified: Some(false),
        };
        let snapshot_modified = StateSnapshot {
            mode: test_mode_id(),
            active_buffer: Some(buffer_id),
            cursor: None,
            modified: Some(true),
        };

        // These should be different (triggers buffer_modified notification)
        assert_ne!(snapshot_unmodified, snapshot_modified);
    }
}
