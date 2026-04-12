//! Codec driver events.
//!
//! Events emitted by the codec driver for codec-selection and
//! content-type lifecycle.  These are NOT domain events — they belong
//! to the codec driver because file type is a codec-factory concern.

use reovim_kernel::api::v1::{events::kernel::priority, BufferId, Event};

/// A buffer's detected content type changed.
///
/// Emitted when the codec classifier (re-)identifies the content type
/// of a buffer, e.g., after open or on codec switch.  The `file_type`
/// string matches the classifier's identifier (e.g., `"rust"`,
/// `"python"`, `"elf"`, `"pdf"`).
///
/// Type refinement to a structured [`ContentType`](crate::ContentType)
/// is a follow-on concern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTypeChanged {
    /// Buffer whose content type changed.
    pub buffer_id: BufferId,
    /// New content type identifier.
    pub file_type: String,
}

impl Event for FileTypeChanged {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
