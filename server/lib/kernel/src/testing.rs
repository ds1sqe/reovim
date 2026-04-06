//! Test utilities for kernel-level testing.
//!
//! Provides shared helpers used by all modules and drivers that need
//! a real in-memory `BufferManager` for testing. This eliminates the
//! need to duplicate `TestBufferManager` and `create_test_context()`
//! across 30+ test modules.
//!
//! # Usage
//!
//! ```ignore
//! use reovim_kernel::testing::create_test_context;
//! use reovim_provider_text::testing::setup_buffer;
//!
//! let ctx = create_test_context();
//! let buffer_id = setup_buffer(&ctx, "hello world");
//! ```
//!
//! # Architecture
//!
//! This module is unconditionally compiled (not `#[cfg(test)]`) so that
//! downstream crates can use it in their test modules. This follows the
//! same pattern as `reovim_driver_session::testing`.

use std::{borrow::Cow, collections::HashMap, sync::Arc};

use {
    crate::api::v1::{
        BufferCapabilities, BufferId, BufferManager, BufferOps, EventBus, KernelBuffer,
        KernelContext, ModeId, ModuleId, OptionRegistry, RwLock, ServiceRegistry,
        StorageCapabilities, StorageError, StorageOps,
    },
    reovim_types_text::{Position, TextGeometry},
};

/// Minimal text buffer implementation for kernel-only tests.
pub struct TestTextBuffer {
    id: BufferId,
    content: String,
    modified: bool,
    file_path: Option<String>,
}

impl TestTextBuffer {
    /// Create a new test buffer from content.
    #[must_use]
    pub fn new(content: &str) -> Self {
        Self {
            id: BufferId::new(),
            content: content.to_owned(),
            modified: false,
            file_path: None,
        }
    }
}

impl StorageOps for TestTextBuffer {
    fn byte_len(&self) -> usize {
        self.content.len()
    }

    fn read_bytes(&self, offset: usize, buf: &mut [u8]) -> usize {
        let bytes = self.content.as_bytes();
        if offset >= bytes.len() {
            return 0;
        }
        let available = &bytes[offset..];
        let count = buf.len().min(available.len());
        buf[..count].copy_from_slice(&available[..count]);
        count
    }

    fn capabilities(&self) -> StorageCapabilities {
        StorageCapabilities::HEAP
    }

    fn insert_bytes(&mut self, offset: usize, data: &[u8]) -> Result<(), StorageError> {
        let text = std::str::from_utf8(data)
            .map_err(|_| StorageError::NotSupported("non-UTF-8 insert into test buffer"))?;
        self.content.insert_str(offset, text);
        self.modified = true;
        Ok(())
    }

    fn delete_bytes(&mut self, offset: usize, len: usize) -> Result<Vec<u8>, StorageError> {
        let total = self.content.len();
        if offset + len > total {
            return Err(StorageError::OffsetOutOfRange { offset, len: total });
        }
        let end = offset + len;
        let deleted = self.content.as_bytes()[offset..end].to_vec();
        self.content.replace_range(offset..end, "");
        self.modified = true;
        Ok(deleted)
    }

    fn append_bytes(&mut self, data: &[u8]) -> Result<(), StorageError> {
        let offset = self.content.len();
        StorageOps::insert_bytes(self, offset, data)
    }

    fn read_chunk(&self, offset: usize, max_len: usize) -> Vec<u8> {
        let bytes = self.content.as_bytes();
        if offset >= bytes.len() {
            return Vec::new();
        }
        let end = (offset + max_len).min(bytes.len());
        bytes[offset..end].to_vec()
    }
}

impl crate::api::v1::BufferMeta for TestTextBuffer {
    fn id(&self) -> BufferId {
        self.id
    }

    fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }

    fn set_file_path(&mut self, path: Option<String>) {
        self.file_path = path;
    }

    fn is_modified(&self) -> bool {
        self.modified
    }

    fn set_modified(&mut self, modified: bool) {
        self.modified = modified;
    }
}

impl TextGeometry for TestTextBuffer {
    fn line_count(&self) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        self.content.lines().count()
    }

    fn line(&self, idx: usize) -> Option<Cow<'_, str>> {
        self.content.lines().nth(idx).map(Cow::Borrowed)
    }

    fn line_len(&self, idx: usize) -> Option<usize> {
        self.content
            .lines()
            .nth(idx)
            .map(|line| line.chars().count())
    }

    fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

impl BufferOps for TestTextBuffer {
    fn buffer_capabilities(&self) -> BufferCapabilities {
        BufferCapabilities::ROPE
    }

    fn line_count(&self) -> usize {
        TextGeometry::line_count(self)
    }

    fn line(&self, idx: usize) -> Option<Cow<'_, str>> {
        TextGeometry::line(self, idx)
    }

    fn line_len(&self, idx: usize) -> Option<usize> {
        TextGeometry::line_len(self, idx)
    }

    fn content_bytes(&self) -> Vec<u8> {
        self.content.as_bytes().to_vec()
    }

    fn position_to_byte(&self, pos: Position) -> usize {
        let mut offset = 0;
        for (i, line) in self.content.lines().enumerate() {
            if i == pos.line {
                return offset + pos.column.min(line.len());
            }
            offset += line.len() + 1;
        }
        self.content.len()
    }

    fn byte_to_position(&self, byte_offset: usize) -> Position {
        let mut offset = 0;
        for (i, line) in self.content.lines().enumerate() {
            let line_end = offset + line.len();
            if byte_offset <= line_end {
                return Position::new(i, byte_offset - offset);
            }
            offset = line_end + 1;
        }
        Position::origin()
    }

    fn insert_at(&mut self, pos: Position, text: &str) {
        let offset = self.position_to_byte(pos);
        self.content.insert_str(offset, text);
        self.modified = true;
    }

    fn delete_range(&mut self, start: Position, end: Position) -> String {
        let start = self.position_to_byte(start);
        let end = self.position_to_byte(end);
        let deleted = self.content[start..end].to_owned();
        self.content.replace_range(start..end, "");
        self.modified = true;
        deleted
    }

    fn set_content(&mut self, content: &str) {
        content.clone_into(&mut self.content);
        self.modified = true;
    }

    fn content(&self) -> String {
        self.content.clone()
    }

    fn as_text_geometry(&self) -> &dyn TextGeometry {
        self
    }
}

/// In-memory buffer manager for testing.
///
/// Unlike `KernelContext::default()` which uses a `StubBufferManager`
/// (returns `None` for all lookups), this implementation actually stores
/// and retrieves buffers. Use this when tests need real buffer operations.
pub struct TestBufferManager {
    buffers: RwLock<HashMap<BufferId, Arc<RwLock<dyn KernelBuffer>>>>,
}

impl TestBufferManager {
    /// Create a new empty test buffer manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffers: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for TestBufferManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl BufferManager for TestBufferManager {
    fn get(&self, id: BufferId) -> Option<Arc<RwLock<dyn KernelBuffer>>> {
        self.buffers.read().get(&id).cloned()
    }

    fn register(&self, buffer: Arc<RwLock<dyn KernelBuffer>>) -> BufferId {
        let id = buffer.read().id();
        self.buffers.write().insert(id, buffer);
        id
    }

    fn unregister(&self, id: BufferId) -> Option<Arc<RwLock<dyn KernelBuffer>>> {
        self.buffers.write().remove(&id)
    }

    fn list(&self) -> Vec<BufferId> {
        self.buffers.read().keys().copied().collect()
    }

    fn count(&self) -> usize {
        self.buffers.read().len()
    }
}

/// Create a `KernelContext` with a real in-memory buffer manager.
///
/// This is the standard test context used across all modules. It provides:
/// - Real `TestBufferManager` (stores and retrieves buffers)
/// - Fresh `EventBus`
/// - Empty `OptionRegistry` and `ServiceRegistry`
///
/// For tests that need services (undo, search, etc.), create a context
/// with this function and then register services on `ctx.services`.
#[must_use]
pub fn create_test_context() -> KernelContext {
    KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    )
}

/// Standard test mode ID for unit tests.
///
/// Returns `ModeId::new(ModuleId::new("test"), "normal")`.
/// Use this instead of defining `fn test_mode()` locally in test modules.
#[must_use]
pub const fn test_mode() -> ModeId {
    ModeId::new(ModuleId::new("test"), "normal")
}

/// Register a buffer in the context.
///
/// Convenience helper that wraps `ctx.buffers.register()`.
/// Returns the buffer's `BufferId`.
#[must_use]
pub fn register_buffer(ctx: &KernelContext, buffer: Arc<RwLock<dyn KernelBuffer>>) -> BufferId {
    ctx.buffers.register(buffer)
}

/// Create and register a local test text buffer in the context.
#[must_use]
pub fn register_text_buffer(ctx: &KernelContext, content: &str) -> BufferId {
    ctx.buffers
        .register(Arc::new(RwLock::new(TestTextBuffer::new(content))))
}
