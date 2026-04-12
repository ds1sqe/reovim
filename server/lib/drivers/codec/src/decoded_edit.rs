//! Decoded-edit abstraction for codec-local mutation pipelines.
//!
//! `DecodedEdit` represents a user-level edit in the decoded domain.
//!
//! - [`DecodedEdit::Text`] — text-shaped edit with document coordinates.
//!   A faithful text codec translates these coordinates to byte offsets
//!   when mutating canonical bytes.
//! - [`DecodedEdit::Bytes`] — direct byte-shaped edit in decoded output
//!   space. Used by hex/byte codecs that expose raw bytes as the
//!   decoded view.
//! - [`DecodedEdit::Tree`] — structural edit targeting a tree-shaped
//!   decoded view (Plan 07 Phase 1+). A [`TreePath`] identifies a node
//!   in the tree, and a [`TreeOp`] describes the change. Structural
//!   codecs (ELF, .rlib, zip, tar.gz, PDF) parse a mount-local tree
//!   cache and translate `Tree` edits back into canonical byte edits
//!   immediately — trees are never a parallel source of truth.

use std::any::Any;

use reovim_domain_text::Position;

/// Path to a node inside a tree-shaped decoded view.
///
/// `TreePath` is format-agnostic: each component is a `String` identifier
/// interpreted by the codec. Typical patterns:
///
/// - ELF: `["sections", "text", "bytes"]`
/// - zip: `["entries", "README.txt", "data"]`
/// - PDF: `["metadata", "title"]`
///
/// The empty path (`TreePath::root()`) refers to the root of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreePath {
    components: Vec<String>,
}

impl TreePath {
    /// Construct a path from an explicit component list.
    #[must_use]
    pub const fn new(components: Vec<String>) -> Self {
        Self { components }
    }

    /// Construct the root path (empty component list).
    #[must_use]
    pub const fn root() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    /// Whether this is the root path.
    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.components.is_empty()
    }

    /// Borrow the component list.
    #[must_use]
    pub fn components(&self) -> &[String] {
        &self.components
    }
}

/// Object-safe trait for format-specific tree operations.
///
/// Implementors provide format-specific structural edit semantics
/// (e.g. ELF instruction patch, ZIP entry rename). The driver layer
/// wraps them in an opaque [`TreeOp`] via [`TreeOp::new`]; codec
/// `translate_edit` implementations recover the concrete type with
/// [`TreeOp::downcast_ref`].
///
/// Use the [`impl_tree_op!`] macro for ergonomic blanket implementation
/// on any `Debug + Clone + PartialEq + Send + Sync + 'static` type.
pub trait AnyTreeOp: Any + std::fmt::Debug + Send + Sync {
    /// Clone into a new boxed trait object.
    fn clone_box(&self) -> Box<dyn AnyTreeOp>;
    /// Value-equality against an erased peer.
    fn eq_any(&self, other: &dyn Any) -> bool;
    /// Upcast to `&dyn Any` for downcasting.
    fn as_any(&self) -> &dyn Any;
}

/// Implement [`AnyTreeOp`] for a concrete type.
///
/// The type must derive or implement `Debug`, `Clone`, `PartialEq`,
/// and be `Send + Sync + 'static`.
///
/// ```ignore
/// use reovim_driver_codec::impl_tree_op;
///
/// #[derive(Debug, Clone, PartialEq, Eq)]
/// pub enum MyFormatOp { Rename { new_name: String } }
///
/// impl_tree_op!(MyFormatOp);
/// ```
#[macro_export]
macro_rules! impl_tree_op {
    ($ty:ty) => {
        impl $crate::AnyTreeOp for $ty {
            fn clone_box(&self) -> Box<dyn $crate::AnyTreeOp> {
                Box::new(self.clone())
            }

            fn eq_any(&self, other: &dyn std::any::Any) -> bool {
                other.downcast_ref::<Self>().is_some_and(|o| self == o)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    };
}

/// Opaque type-erased structural tree operation.
///
/// Wraps a `Box<dyn AnyTreeOp>` so the driver layer never depends on
/// format-specific operation types. Each codec module defines its own
/// concrete op enum (e.g. `ElfTreeOp`, `ZipTreeOp`) and wraps it with
/// [`TreeOp::new`]. The codec's `translate_edit` recovers the concrete
/// type via [`TreeOp::downcast_ref`].
pub struct TreeOp {
    inner: Box<dyn AnyTreeOp>,
}

impl TreeOp {
    /// Wrap a concrete tree-op value.
    #[must_use]
    pub fn new<T: AnyTreeOp + 'static>(op: T) -> Self {
        Self {
            inner: Box::new(op),
        }
    }

    /// Attempt to downcast to a concrete tree-op type.
    #[must_use]
    pub fn downcast_ref<T: AnyTreeOp + 'static>(&self) -> Option<&T> {
        self.inner.as_any().downcast_ref::<T>()
    }
}

impl Clone for TreeOp {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone_box(),
        }
    }
}

impl PartialEq for TreeOp {
    fn eq(&self, other: &Self) -> bool {
        self.inner.eq_any(other.inner.as_any())
    }
}

impl Eq for TreeOp {}

impl std::fmt::Debug for TreeOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

/// Domain-agnostic decoded edit representation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodedEdit {
    /// A text-shaped edit from a decoded text buffer.
    Text {
        /// Start position in decoded text coordinates.
        start: Position,
        /// End position in decoded text coordinates (exclusive).
        end: Position,
        /// Replacement text.
        replacement: String,
    },

    /// A direct byte-shaped edit in decoded output space.
    Bytes {
        /// Start offset in bytes.
        offset: usize,
        /// Number of old bytes to replace.
        old_len: usize,
        /// Replacement bytes.
        new_bytes: Vec<u8>,
    },

    /// A structural edit on a tree-shaped decoded view.
    ///
    /// Used by structural codecs (ELF, .rlib, zip, tar.gz, PDF). The
    /// codec's [`crate::ContentCodec::translate_edit`] implementation
    /// resolves `path` in its parsed tree cache and applies `op`,
    /// emitting an equivalent byte-level edit immediately. Bytes remain
    /// the source of truth; the tree is mount-local cache only.
    Tree {
        /// Path of the target node in the codec's tree view.
        path: TreePath,
        /// Operation to apply at that node.
        op: TreeOp,
    },
}

impl DecodedEdit {
    /// Convenience: whether this edit replaces no old bytes.
    #[must_use]
    pub fn is_insertion(&self) -> bool {
        match self {
            Self::Text { start, end, .. } => start == end,
            Self::Bytes { old_len, .. } => *old_len == 0,
            Self::Tree { .. } => false,
        }
    }

    /// Convenience: whether this edit deletes old bytes and inserts no new bytes.
    #[must_use]
    pub fn is_deletion(&self) -> bool {
        match self {
            Self::Text {
                start,
                end,
                replacement,
                ..
            } => *start != *end && replacement.is_empty(),
            Self::Bytes {
                old_len, new_bytes, ..
            } => *old_len > 0 && new_bytes.is_empty(),
            Self::Tree { .. } => false,
        }
    }
}

#[cfg(test)]
#[path = "decoded_edit_tests.rs"]
mod tests;
