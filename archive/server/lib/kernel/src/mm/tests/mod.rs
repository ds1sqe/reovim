//! Tests for the mm module.

use super::*;

mod buffer_id;
mod saturator;

// LineIndex tests moved to reovim-domain-text (#740)
// PieceTree tests moved to reovim-subsys-vfs (#740)
// BufferSnapshot tests moved to reovim-provider-text (#740)
// VirtualBuffer tests moved to reovim-provider-text (#740)

// === BufferId Tests ===

mod buffer_id_tests {
    use super::*;

    #[test]
    fn test_unique_ids() {
        let id1 = BufferId::new();
        let id2 = BufferId::new();
        let id3 = BufferId::new();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_ordering() {
        let id1 = BufferId::new();
        let id2 = BufferId::new();

        assert!(id1 < id2);
    }

    #[test]
    fn test_from_raw() {
        let id = BufferId::from_raw(42);
        assert_eq!(id.as_usize(), 42);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_display() {
        let id = BufferId::from_raw(123);
        assert_eq!(format!("{id}"), "Buffer(123)");
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let id1 = BufferId::new();
        let id2 = BufferId::new();

        let mut set = HashSet::new();
        set.insert(id1);
        set.insert(id2);
        set.insert(id1); // Duplicate

        assert_eq!(set.len(), 2);
    }
}
