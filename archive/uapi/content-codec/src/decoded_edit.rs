//! Decoded-edit abstraction for codec-local mutation pipelines.
//!
//! `DecodedEdit` represents a user-level edit in the decoded domain.
//! The domain-specific edit shape is carried inside the [`DecodedEdit::Domain`]
//! variant as a type-erased [`DomainEdit`] trait object. Each codec concern
//! defines its own concrete domain-edit type (e.g. `TextEdit` for the text
//! domain, `AudioSampleEdit` for an audio domain) and wraps it via
//! [`DomainEdit::new`]. Codecs recover the concrete type with
//! [`DomainEdit::downcast_ref`].
//!
//! The type-erased design keeps [`crate::ContentCodec`] object-safe
//! (`Arc<dyn ContentCodec>` continues to work for factories and registries)
//! while keeping uapi free of any domain vocabulary.
//!
//! - [`DecodedEdit::Domain`] — domain-shaped edit wrapping a concrete domain
//!   edit payload.
//! - [`DecodedEdit::Bytes`] — direct byte-shaped edit in decoded output
//!   space. Used by hex/byte codecs that expose raw bytes as the decoded
//!   view.
//! - [`DecodedEdit::Tree`] — structural edit targeting a tree-shaped decoded
//!   view. A [`TreePath`] identifies a node in the tree, and a [`TreeOp`]
//!   describes the change. Structural codecs (ELF, .rlib, zip, tar.gz, PDF)
//!   parse a mount-local tree cache and translate `Tree` edits back into
//!   canonical byte edits immediately — trees are never a parallel source
//!   of truth.

use std::any::Any;

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

// ── Tree ops ────────────────────────────────────────────────────────────────

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
/// use reovim_content_codec::impl_tree_op;
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

// ── Domain edits (type-erased) ──────────────────────────────────────────────

/// Object-safe trait for domain-specific edit payloads.
///
/// Each codec concern defines its own concrete edit type (e.g. `TextEdit`
/// for the text domain) and wraps it in [`DomainEdit::new`] when producing
/// a [`DecodedEdit::Domain`] value. Codecs recover the concrete type with
/// [`DomainEdit::downcast_ref`].
///
/// The same rationale as [`AnyTreeOp`] applies: keep uapi out of every
/// domain's vocabulary while letting concrete codecs round-trip their
/// real edit types.
///
/// Use the [`impl_domain_edit!`] macro for ergonomic blanket implementation
/// on any `Debug + Clone + PartialEq + Send + Sync + 'static` type.
pub trait AnyDomainEdit: Any + std::fmt::Debug + Send + Sync {
    /// Clone into a new boxed trait object.
    fn clone_box(&self) -> Box<dyn AnyDomainEdit>;
    /// Value-equality against an erased peer.
    fn eq_any(&self, other: &dyn Any) -> bool;
    /// Upcast to `&dyn Any` for downcasting.
    fn as_any(&self) -> &dyn Any;
}

/// Implement [`AnyDomainEdit`] for a concrete type.
///
/// The type must derive or implement `Debug`, `Clone`, `PartialEq`, and be
/// `Send + Sync + 'static`.
///
/// ```ignore
/// use reovim_content_codec::impl_domain_edit;
///
/// #[derive(Debug, Clone, PartialEq, Eq)]
/// pub struct MyEdit { pub tag: &'static str }
///
/// impl_domain_edit!(MyEdit);
/// ```
#[macro_export]
macro_rules! impl_domain_edit {
    ($ty:ty) => {
        impl $crate::AnyDomainEdit for $ty {
            fn clone_box(&self) -> Box<dyn $crate::AnyDomainEdit> {
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

/// Opaque type-erased domain-specific edit payload.
///
/// See [`DecodedEdit::Domain`]. Mirrors [`TreeOp`] but for domain-shaped
/// edits (rather than tree-shaped structural edits).
pub struct DomainEdit {
    inner: Box<dyn AnyDomainEdit>,
}

impl DomainEdit {
    /// Wrap a concrete domain-edit value.
    #[must_use]
    pub fn new<T: AnyDomainEdit + 'static>(edit: T) -> Self {
        Self {
            inner: Box::new(edit),
        }
    }

    /// Attempt to downcast to a concrete domain-edit type.
    #[must_use]
    pub fn downcast_ref<T: AnyDomainEdit + 'static>(&self) -> Option<&T> {
        self.inner.as_any().downcast_ref::<T>()
    }
}

impl Clone for DomainEdit {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone_box(),
        }
    }
}

impl PartialEq for DomainEdit {
    fn eq(&self, other: &Self) -> bool {
        self.inner.eq_any(other.inner.as_any())
    }
}

impl Eq for DomainEdit {}

impl std::fmt::Debug for DomainEdit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

// ── DecodedEdit enum ────────────────────────────────────────────────────────

/// Domain-agnostic decoded edit representation.
///
/// The `Domain` variant carries a type-erased [`DomainEdit`] so that the
/// uapi stays free of any domain's vocabulary while codec concerns
/// (text, audio, etc.) can round-trip their concrete edit payloads via
/// [`DomainEdit::new`] / [`DomainEdit::downcast_ref`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodedEdit {
    /// A domain-shaped edit from a decoded domain buffer.
    ///
    /// The concrete shape of the inner value is domain-specific. Text
    /// codecs wrap a text-edit type (see `ext/content-codec/text/`); audio
    /// or video domains plug in their own.
    Domain(DomainEdit),

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
    /// Whether this edit replaces no old bytes in the byte-shaped view.
    ///
    /// `Domain` and `Tree` variants return `false`: callers must inspect
    /// the concrete edit (via [`DomainEdit::downcast_ref`] or tree-op
    /// dispatch) for domain-level insertion semantics.
    #[must_use]
    pub const fn is_byte_insertion(&self) -> bool {
        match self {
            Self::Bytes { old_len, .. } => *old_len == 0,
            Self::Domain(_) | Self::Tree { .. } => false,
        }
    }

    /// Whether this edit deletes old bytes without adding new bytes in
    /// the byte-shaped view.
    ///
    /// As with [`Self::is_byte_insertion`], `Domain` and `Tree` variants
    /// return `false` at this layer — inspect the concrete edit for
    /// domain-level deletion semantics.
    #[must_use]
    pub const fn is_byte_deletion(&self) -> bool {
        match self {
            Self::Bytes {
                old_len, new_bytes, ..
            } => *old_len > 0 && new_bytes.is_empty(),
            Self::Domain(_) | Self::Tree { .. } => false,
        }
    }
}

#[cfg(test)]
#[path = "decoded_edit_tests.rs"]
mod tests;
