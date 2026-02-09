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
mod tests {
    use super::*;

    #[test]
    fn service_name() {
        assert_eq!(BufferManagerKey::service_name(), "BufferManager");
    }

    #[test]
    fn debug() {
        let key = BufferManagerKey::Simple;
        assert_eq!(format!("{key:?}"), "Simple");
    }

    #[test]
    fn clone_copy_eq() {
        let a = BufferManagerKey::Simple;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(BufferManagerKey::Simple);
        set.insert(BufferManagerKey::Simple);
        assert_eq!(set.len(), 1);
    }
}
