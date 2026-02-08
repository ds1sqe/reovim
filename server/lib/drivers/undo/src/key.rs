//! Undo provider key - typed key for undo provider lookup.

use reovim_kernel::api::v1::ServiceKey;

/// Typed key for undo provider lookup.
///
/// Currently only one variant, but designed to support future extensions
/// like different undo strategies or scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UndoKey {
    /// Per-buffer undo registry (default strategy).
    Buffer,
}

impl ServiceKey for UndoKey {
    fn service_name() -> &'static str {
        "Undo"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_name() {
        assert_eq!(UndoKey::service_name(), "Undo");
    }

    #[test]
    fn debug() {
        assert_eq!(format!("{:?}", UndoKey::Buffer), "Buffer");
    }

    #[test]
    fn clone_copy_eq() {
        let a = UndoKey::Buffer;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(UndoKey::Buffer);
        set.insert(UndoKey::Buffer);
        assert_eq!(set.len(), 1);
    }
}
