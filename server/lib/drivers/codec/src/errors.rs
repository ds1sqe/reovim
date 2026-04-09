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
