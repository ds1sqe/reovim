//! Clipboard provider key - typed key for clipboard provider lookup.

use reovim_kernel::api::v1::ServiceKey;

/// Typed key for clipboard provider lookup.
///
/// Currently only one variant, but designed to support future extensions
/// like different clipboard backends or scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClipboardKey {
    /// Default clipboard provider (system clipboard + history).
    Default,
}

impl ServiceKey for ClipboardKey {
    fn service_name() -> &'static str {
        "Clipboard"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_name() {
        assert_eq!(ClipboardKey::service_name(), "Clipboard");
    }

    #[test]
    fn debug() {
        let key = ClipboardKey::Default;
        assert_eq!(format!("{key:?}"), "Default");
    }

    #[test]
    fn clone_copy_eq() {
        let a = ClipboardKey::Default;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ClipboardKey::Default);
        set.insert(ClipboardKey::Default);
        assert_eq!(set.len(), 1);
    }
}
