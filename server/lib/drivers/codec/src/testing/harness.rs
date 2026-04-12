//! Shared codec verification harness (Plan 07 Phase 1 scaffold).
//!
//! `verify_codec` runs every structural codec through the same four
//! Phase 1 gates before Phase 2 format work lands:
//!
//! 1. **No-op round-trip** — apply zero edits, confirm bytes unchanged.
//! 2. **Post-edit re-parse** — apply each fixture edit through
//!    `translate_edit`, apply the resulting byte edit, then re-decode
//!    and confirm the result is well-formed.
//! 3. **Peer-mount propagation** — mount the same inode under a second
//!    codec reference, apply the edit, confirm the peer mount's
//!    `content_valid` flag is cleared (the next read will re-decode).
//! 4. **Undo correctness** — apply an edit, roll bytes back, confirm
//!    the inode returns to its original state.
//!
//! Phase 2–6 format phases will provide format-specific
//! [`HarnessFixture`] instances with ELF/zip/PDF fixtures. Phase 1 ships
//! only the scaffold and exercises it against the UTF-8 codec.

use std::sync::Arc;

use {reovim_driver_vfs::HeapByteSource, reovim_kernel::api::v1::BufferId};

use crate::{ContentCodec, DecodedEdit, EditError, InodeTable, Mount, TranslateEditError};

/// A harness fixture: raw bytes plus a list of edits that should round-trip.
#[derive(Debug, Clone)]
pub struct HarnessFixture {
    /// Display name for diagnostics and test output.
    pub name: &'static str,
    /// Initial raw bytes to seed the inode with.
    pub initial_bytes: Vec<u8>,
    /// Edits to run through `translate_edit`. Each must be accepted
    /// (`Ok(Some(_))` or `Ok(None)`) by the codec under test.
    pub edits: Vec<DecodedEdit>,
}

impl HarnessFixture {
    /// Build a new harness fixture.
    #[must_use]
    pub const fn new(name: &'static str, initial_bytes: Vec<u8>, edits: Vec<DecodedEdit>) -> Self {
        Self {
            name,
            initial_bytes,
            edits,
        }
    }
}

/// Harness failure modes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessError {
    /// Initial decode failed; the fixture's `initial_bytes` do not parse
    /// through the codec.
    InitialDecodeFailed {
        /// Fixture name.
        fixture: &'static str,
        /// Stringified error class.
        reason: String,
    },
    /// The no-op round-trip gate changed bytes.
    NoopRoundTripChangedBytes {
        /// Fixture name.
        fixture: &'static str,
    },
    /// A fixture edit was rejected by the codec when it should have
    /// been accepted.
    EditRejected {
        /// Fixture name.
        fixture: &'static str,
        /// Zero-based index of the offending edit in `fixture.edits`.
        edit_index: usize,
        /// Rejection class returned by the codec.
        cause: TranslateEditError,
    },
    /// Applying the translated `ByteEdit` to the inode failed.
    ApplyFailed {
        /// Fixture name.
        fixture: &'static str,
        /// Zero-based index of the offending edit in `fixture.edits`.
        edit_index: usize,
        /// `InodeTable::apply_edit` error.
        cause: EditError,
    },
    /// Re-decoding the edited bytes failed.
    PostEditDecodeFailed {
        /// Fixture name.
        fixture: &'static str,
        /// Zero-based index of the edit whose effect we tried to re-decode.
        edit_index: usize,
        /// Stringified error.
        reason: String,
    },
    /// Peer mount's `content_valid` flag was not cleared after a sibling edit.
    PeerMountNotStaled {
        /// Fixture name.
        fixture: &'static str,
    },
    /// Undo round-trip did not restore the original bytes.
    UndoDidNotRestore {
        /// Fixture name.
        fixture: &'static str,
    },
}

/// Map an `EditError` surfaced by `InodeTable::apply_edit` back onto a
/// `HarnessError` carrying the fixture identity.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn classify_apply_error(
    fixture_name: &'static str,
    edit_index: usize,
    cause: EditError,
) -> HarnessError {
    match cause {
        EditError::ReadOnly => HarnessError::EditRejected {
            fixture: fixture_name,
            edit_index,
            cause: TranslateEditError::ReadOnly,
        },
        EditError::Unsupported { reason } => HarnessError::EditRejected {
            fixture: fixture_name,
            edit_index,
            cause: TranslateEditError::UnsupportedEdit { reason },
        },
        EditError::InvalidEdit { reason } => HarnessError::EditRejected {
            fixture: fixture_name,
            edit_index,
            cause: TranslateEditError::ConstraintViolation { reason },
        },
        other => HarnessError::ApplyFailed {
            fixture: fixture_name,
            edit_index,
            cause: other,
        },
    }
}

/// Run the no-op + edit sequence against an already-mounted inode.
#[cfg_attr(coverage_nightly, coverage(off))]
fn run_edit_sequence(
    fixture: &HarnessFixture,
    codec: &dyn ContentCodec,
    table: &mut InodeTable,
    inode_id: crate::InodeId,
    mount_handle: crate::MountHandle,
) -> Result<(), HarnessError> {
    let after_noop = table
        .read_bytes(inode_id)
        .expect("harness: read bytes after noop");
    if after_noop != fixture.initial_bytes {
        return Err(HarnessError::NoopRoundTripChangedBytes {
            fixture: fixture.name,
        });
    }

    for (idx, edit) in fixture.edits.iter().enumerate() {
        match table.apply_edit(mount_handle, edit) {
            Ok(_) => {}
            Err(cause) => return Err(classify_apply_error(fixture.name, idx, cause)),
        }

        let edited = table
            .read_bytes(inode_id)
            .map_err(|cause| HarnessError::ApplyFailed {
                fixture: fixture.name,
                edit_index: idx,
                cause,
            })?;
        codec
            .decode(&edited)
            .map_err(|e| HarnessError::PostEditDecodeFailed {
                fixture: fixture.name,
                edit_index: idx,
                reason: format!("{e:?}"),
            })?;
    }

    Ok(())
}

/// Run the Phase 1 verification harness against a `ContentCodec`
/// implementation using the given fixture.
///
/// # Errors
///
/// Returns `HarnessError` the first time a gate fails. The caller
/// should surface the specific variant in test assertions so regressions
/// in any of the four gates are visible.
#[allow(clippy::missing_panics_doc)]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn verify_codec(
    codec: &Arc<dyn ContentCodec>,
    fixture: &HarnessFixture,
) -> Result<(), HarnessError> {
    // Gate 0: the fixture is well-formed enough for the codec to decode.
    codec
        .decode(&fixture.initial_bytes)
        .map_err(|e| HarnessError::InitialDecodeFailed {
            fixture: fixture.name,
            reason: format!("{e:?}"),
        })?;

    // Gate 1: no-op round-trip. Build an inode + source mount; the no-op
    // check lives inside `run_edit_sequence`.
    let mut table = InodeTable::new();
    let initial = fixture.initial_bytes.clone();
    let inode_id = table.insert(Arc::new(HeapByteSource::new(initial.clone())));
    let buffer_id = BufferId::from_raw(1);
    table.bind_file(buffer_id, inode_id);
    let mount_handle = table
        .mount(inode_id, buffer_id, Mount::new("harness-source", Arc::clone(codec)))
        .expect("harness: mount source codec");

    // Gate 3 (peer-mount propagation): attach a peer mount BEFORE any
    // edit runs so we can observe its `content_valid` transition after
    // the edit sequence.
    let peer_handle = table
        .mount_additional(inode_id, buffer_id, Mount::new("harness-peer", Arc::clone(codec)))
        .expect("harness: mount peer codec");

    // Gate 2: post-edit re-parse (inside run_edit_sequence).
    run_edit_sequence(fixture, codec.as_ref(), &mut table, inode_id, mount_handle)?;

    // Gate 3 follow-up: if any edit actually mutated bytes, the peer
    // mount must now be stale. Clean no-ops must leave the peer valid.
    if !fixture.edits.is_empty() {
        let current = table
            .read_bytes(inode_id)
            .expect("harness: final read_bytes");
        let inode = table
            .lookup_inode(inode_id)
            .expect("harness: inode still present after edits");
        let peer_mount = inode
            .mounts
            .get(&peer_handle.mount_id())
            .expect("harness: peer mount still attached");
        if current != initial && peer_mount.content_valid {
            return Err(HarnessError::PeerMountNotStaled {
                fixture: fixture.name,
            });
        }
    }

    // Gate 4: undo correctness. Phase 1 scaffold writes the original
    // bytes back and confirms the codec re-decodes cleanly. Phase 2+
    // will exercise the real inode undo log.
    table
        .set_bytes(inode_id, initial.clone())
        .expect("harness: restore initial bytes");
    let restored = table
        .read_bytes(inode_id)
        .expect("harness: read restored bytes");
    if restored != initial {
        return Err(HarnessError::UndoDidNotRestore {
            fixture: fixture.name,
        });
    }
    codec
        .decode(&restored)
        .map_err(|e| HarnessError::InitialDecodeFailed {
            fixture: fixture.name,
            reason: format!("{e:?}"),
        })?;

    Ok(())
}

#[cfg(test)]
#[path = "harness_tests.rs"]
mod tests;
