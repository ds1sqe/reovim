//! Buffer manager key - typed key for buffer manager lookup.

use reovim_kernel::api::v1::ServiceKey;

/// Typed key for buffer manager lookup.
///
/// Different variants can represent different buffer management strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferManagerKey {
    /// Simple in-memory buffer manager (HashMap-based).
    Simple,
}

impl ServiceKey for BufferManagerKey {
    fn service_name() -> &'static str {
        "BufferManager"
    }
}

#[cfg(test)]
#[path = "key_tests.rs"]
mod tests;
