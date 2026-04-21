//! Text-specific `DecodedEdit` conversion helper.
//!
//! Wraps the `reovim-domain-text-events` event shape into a
//! [`DecodedEdit::Domain`] carrying a `TextEdit` payload. Lives here
//! (not in `uapi/content-codec/`) so the domain-neutral uapi stays free
//! of any text vocabulary. The Plan 14 Phase C.3 text carve-out will
//! relocate this file into `ext/content-codec/text/` alongside the
//! concrete `TextEdit` type.

use {
    reovim_content_codec::{DecodedEdit, DomainEdit, impl_domain_edit},
    reovim_domain_text::Position,
};

/// Concrete domain-edit payload for the text domain.
///
/// Mirrors the pre-Phase-C `DecodedEdit::Text` variant fields. Carried
/// inside [`DecodedEdit::Domain`] via [`DomainEdit::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// Start position in decoded text coordinates.
    pub start: Position,
    /// End position in decoded text coordinates (exclusive).
    pub end: Position,
    /// Replacement text.
    pub replacement: String,
}

impl_domain_edit!(TextEdit);

/// Convert a `TextBufferModified` event into a codec [`DecodedEdit`].
///
/// The domain-text event types stay out of `uapi/content-codec/`; this
/// helper bridges the text-specific shape into the uapi's type-erased
/// [`DecodedEdit::Domain`] variant.
#[must_use]
pub fn text_edit_to_decoded_edit(
    event: &reovim_domain_text_events::TextBufferModified,
) -> DecodedEdit {
    use reovim_domain_text_events::TextEdit as EventTextEdit;

    let payload = match &event.edit {
        EventTextEdit::Insert { position, text } => TextEdit {
            start: Position::new(position.line, position.column),
            end: Position::new(position.line, position.column),
            replacement: text.clone(),
        },
        EventTextEdit::Delete { position, text } => {
            // Compute end position from start + deleted text content
            let mut end_line = position.line;
            let mut end_col = position.column;
            for ch in text.chars() {
                if ch == '\n' {
                    end_line += 1;
                    end_col = 0;
                } else {
                    end_col += 1;
                }
            }
            TextEdit {
                start: Position::new(position.line, position.column),
                end: Position::new(end_line, end_col),
                replacement: String::new(),
            }
        }
    };

    DecodedEdit::Domain(DomainEdit::new(payload))
}
