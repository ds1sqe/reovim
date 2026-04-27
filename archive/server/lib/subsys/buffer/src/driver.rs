//! Factory trait for buffer instances.

use {
    crate::{buffer::Buffer, error::BufferError},
    reovim_kernel::api::v1::BufferId,
    std::sync::Arc,
};

/// Factory and registry for [`Buffer`] instances.
///
/// `BufferDriver` is the entry point the composition root holds.
/// The safe host-side wrapper `LoadedBuffer` (in
/// `server/lib/subsys/driver-loader/`) implements this trait by
/// routing each method through the `BufferVTable`.
pub trait BufferDriver: Send + Sync + 'static {
    /// Discriminant string identifying this driver family.
    ///
    /// Must equal `"buffer"`.
    fn kind(&self) -> &'static str;

    /// Create a new buffer pre-populated with `initial_bytes`.
    ///
    /// If `file_path` is `Some`, the buffer is considered associated
    /// with that path (but not yet marked modified — it was just opened).
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::Driver`] if the driver-side allocator
    /// fails to provision the new buffer.
    fn create_buffer(
        &self,
        initial_bytes: &[u8],
        file_path: Option<String>,
    ) -> Result<Arc<dyn Buffer>, BufferError>;

    /// Open the file at `file_path` and return a buffer over its contents.
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::Io`] if the file cannot be read; returns
    /// [`BufferError::Driver`] for allocator-side failures.
    fn open_buffer(&self, file_path: &str) -> Result<Arc<dyn Buffer>, BufferError>;

    /// List the ids of all live buffers owned by this driver.
    fn list_buffers(&self) -> Vec<BufferId>;

    /// Return the buffer with `id`, or `None` if it is not tracked.
    fn get_buffer(&self, id: BufferId) -> Option<Arc<dyn Buffer>>;

    /// Close (and release) the buffer with `id`.
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::NotFound`] if no buffer is tracked at `id`.
    fn close_buffer(&self, id: BufferId) -> Result<(), BufferError>;
}

#[cfg(test)]
#[path = "driver_tests.rs"]
mod tests;
