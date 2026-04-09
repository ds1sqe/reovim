//! Error types for inode and mount operations.

use {
    crate::inode::{InodeId, MountId},
    thiserror::Error,
};

/// Errors while adding a mount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MountError {
    /// Requested inode id is unknown.
    #[error("inode {inode_id} not found")]
    InodeNotFound {
        /// Target inode.
        inode_id: InodeId,
    },

    /// Inode already has an active mount.
    #[error("inode {inode_id} already has an active mount")]
    AlreadyMounted {
        /// Target inode.
        inode_id: InodeId,
    },
}

/// Errors while removing a mount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum UmountError {
    /// Requested inode id is unknown.
    #[error("inode {inode_id} not found")]
    InodeNotFound {
        /// Target inode.
        inode_id: InodeId,
    },

    /// The mount is not present on the inode.
    #[error("mount {mount_id} not found on inode {inode_id}")]
    MountNotFound {
        /// Target inode.
        inode_id: InodeId,
        /// Missing mount.
        mount_id: MountId,
    },
}

/// Errors while applying decoded edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EditError {
    /// Requested inode id is unknown.
    #[error("inode {inode_id} not found")]
    InodeNotFound {
        /// Target inode.
        inode_id: InodeId,
    },

    /// The mount is not present on the inode.
    #[error("mount {mount_id} not found on inode {inode_id}")]
    MountNotFound {
        /// Target inode.
        inode_id: InodeId,
        /// Missing mount.
        mount_id: MountId,
    },

    /// Codec is read-only or cannot translate this edit.
    #[error("codec is read-only")]
    ReadOnly,

    /// An edit variant is not supported in this phase.
    #[error("{reason}")]
    Unsupported {
        /// Explanation.
        reason: &'static str,
    },

    /// The decoded edit does not apply to current bytes.
    #[error("invalid edit: {reason}")]
    InvalidEdit {
        /// Explanation.
        reason: &'static str,
    },

    /// The translated edit could not be applied.
    #[error("failed to apply edit: {reason}")]
    ApplyFailed {
        /// Explanation.
        reason: &'static str,
    },
}

/// Errors surfaced by [`crate::ContentCodec::translate_edit`].
///
/// Plan 07 Phase 1 taxonomy: structural codec translation may accept
/// some decoded-edit operations and reject others. `TranslateEditError`
/// distinguishes rejection reasons so `InodeTable::apply_edit` can map
/// them to concrete [`EditError`] variants and so structural codec
/// authors have a typed vocabulary for "this edit is wrong for this
/// codec" vs "this edit is physically impossible".
///
/// All reason fields are `&'static str` for uniformity with
/// [`EditError`]. Dynamic context (e.g. which ELF section failed to
/// parse) is logged via `tracing` at the codec layer. See Plan 07
/// `~/docs/plans/reovim/740/07-structural-codec-editing.md` §2 for the
/// pinned semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TranslateEditError {
    /// Codec cannot translate any edit. All mounts through this codec
    /// behave read-only by construction (ELF, zip, PDF pre-structural).
    #[error("codec is read-only")]
    ReadOnly,

    /// Codec can translate some operations but not this edit variant.
    ///
    /// Example: a UTF-8 codec receiving [`crate::DecodedEdit::Bytes`]
    /// or [`crate::DecodedEdit::Tree`]. Distinct from `ReadOnly` — the
    /// codec has edit capability, just not for this operation.
    #[error("unsupported edit variant: {reason}")]
    UnsupportedEdit {
        /// Static explanation (e.g. `"utf-8 codec does not accept byte edits"`).
        reason: &'static str,
    },

    /// Edit violates a codec-specific domain constraint.
    ///
    /// Example: ELF section resize that would change the section count,
    /// or a zip STORED-entry rewrite with a different payload length.
    /// The edit is well-formed but domain rules reject it.
    #[error("constraint violation: {reason}")]
    ConstraintViolation {
        /// Static explanation of the violated constraint.
        reason: &'static str,
    },

    /// A [`crate::DecodedEdit::Tree`] `TreePath` did not resolve in the
    /// current tree state.
    ///
    /// Example: structural codec receives an edit targeting a tree node
    /// that has been removed by a prior peer-mount edit, or a path
    /// component that never existed.
    #[error("malformed tree path: {reason}")]
    MalformedPath {
        /// Static explanation (e.g. `"tree path component not found"`).
        reason: &'static str,
    },

    /// Codec hit an I/O, parse, or infrastructure failure while
    /// translating the edit.
    ///
    /// Distinct from `ConstraintViolation` — this is NOT a domain
    /// constraint, it is infrastructure (e.g. byte-source read failure,
    /// decode failure). Dynamic context is logged via `tracing` at the
    /// codec layer; the static reason is enough for callers to
    /// distinguish the class.
    #[error("internal translate_edit failure: {reason}")]
    Internal {
        /// Static explanation of the infrastructure failure class.
        reason: &'static str,
    },
}

/// Errors surfaced by [`crate::CodecSessionState::mount_codec`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MountCodecError {
    /// No canonical inode bytes exist for the buffer. Phase 5 5c callers
    /// must ensure the buffer is bound to an inode (via `set_source` /
    /// `mount_decoded`) before requesting a secondary mount.
    #[error("no canonical inode bytes for buffer")]
    NoCanonicalBytes,

    /// The factory store has no codec registered for the requested
    /// content type.
    #[error("no codec for content type '{content_type}'")]
    NoCodec {
        /// The content type that was requested.
        content_type: String,
    },

    /// The codec driver returned an error while attaching the mount.
    /// This surfaces any `MountError` that leaked out of the underlying
    /// `InodeTable::mount` / `mount_additional` call (for example a
    /// mount id overflow that would only happen after ~18 quintillion
    /// mounts — effectively unreachable, but still surfaced).
    #[error(transparent)]
    Mount(#[from] MountError),
}

/// Errors surfaced by [`crate::CodecSessionState::unmount_codec`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UmountCodecError {
    /// The mount id is not registered on any inode in this session.
    #[error("mount id not found")]
    MountNotFound,

    /// The codec driver returned an error while detaching the mount.
    #[error(transparent)]
    Umount(#[from] UmountError),
}

/// Errors surfaced by orchestration helpers that drive codec view switching.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SwitchViewError {
    /// No codec metadata has been recorded for the buffer.
    #[error("no codec metadata for buffer")]
    NoMetadata,

    /// Canonical inode bytes for the buffer are missing.
    #[error("no canonical inode bytes for buffer")]
    NoCanonicalBytes,

    /// The factory store has no codec registered for the current content type.
    #[error("no codec for content type")]
    NoCodec,

    /// The requested view name is not exposed by the active codec.
    #[error("view '{view_name}' not available")]
    ViewNotAvailable {
        /// The view that was requested but is not offered by the codec.
        view_name: String,
    },

    /// The codec returned an error while decoding the requested view.
    #[error("codec decode_view failed: {reason}")]
    DecodeFailed {
        /// Error message from the codec.
        reason: String,
    },
}

// Compatibility alias used by remaining callsites until all paths migrate.
#[allow(clippy::module_name_repetitions)]
pub type InodeError = MountError;
