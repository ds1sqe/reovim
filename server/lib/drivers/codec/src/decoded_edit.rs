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

use reovim_types_text::Position;

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

/// Structural tree operation.
///
/// Plan 07 Phase 1: `TreeOp` is an **enum**, not a trait object. The
/// fixed 5-format target ladder (ELF, .rlib, zip, tar.gz, PDF) makes an
/// enum strictly better than `Box<dyn TreeOp>` / `Arc<dyn TreeOp>`:
///
/// - `#[derive(Debug, Clone, PartialEq, Eq)]` works without manual
///   impls; no Arc-identity-equality hazard.
/// - Each Phase 2–6 lands a new variant here, gated only by the
///   `#[non_exhaustive]` attribute.
///
/// Phase 1 ships exactly one variant — the test/feature-gated
/// [`TreeOp::Synthetic`] placeholder — so the verification harness can
/// construct and match `TreeOp` values before any real format is wired up.
/// Phase 2 adds the first real format variant: [`TreeOp::Elf`].
/// Phase 3 adds [`TreeOp::Rlib`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElfTreeOp {
    /// Patch a byte span inside an executable section in place.
    PatchBytes {
        /// Section-relative byte offset to replace.
        offset: usize,
        /// Expected current bytes at `offset`.
        old_bytes: Vec<u8>,
        /// Replacement bytes. Must be the same length as `old_bytes`.
        new_bytes: Vec<u8>,
    },
    /// Rename a symbol in place.
    RenameSymbol {
        /// Replacement symbol name. Must be the same length as the
        /// current symbol name resolved from the tree path.
        new_name: String,
    },
    /// Replace an entire named section payload in place.
    ReplaceSectionBytes {
        /// Expected current section payload.
        old_bytes: Vec<u8>,
        /// Replacement section payload. Must be the same length as
        /// `old_bytes`.
        new_bytes: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RlibTreeOp {
    /// Replace a whole archive member payload in place.
    ReplaceMemberBytes {
        /// Expected current member payload.
        old_bytes: Vec<u8>,
        /// Replacement payload. Must be the same length as `old_bytes`.
        new_bytes: Vec<u8>,
    },
    /// Rename an archive member in place.
    RenameMember {
        /// Replacement member name. Must be the same length as the current
        /// member name resolved from the tree path.
        new_name: String,
    },
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeOp {
    /// ELF structural edit (Plan 07 Phase 2).
    Elf(ElfTreeOp),

    /// Rust `.rlib` structural edit (Plan 07 Phase 3).
    Rlib(RlibTreeOp),

    /// Test-only placeholder variant.
    ///
    /// Compiled into the crate under `#[cfg(any(test, feature =
    /// "testing"))]` so the Phase 1 verification harness and unit tests
    /// can construct a `TreeOp` without needing a real format
    /// implementation.
    #[cfg(any(test, feature = "testing"))]
    Synthetic {
        /// Human-readable name for diagnostics.
        name: String,
    },
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
