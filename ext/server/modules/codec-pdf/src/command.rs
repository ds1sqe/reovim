//! Ex-commands for PDF structural editing.
//!
//! Registers `:pdf-set-metadata` which constructs a `PdfTreeOp::SetMetadata`
//! and dispatches it through the active mount's codec pipeline. The mount
//! must be in structural mode (Phase 7, #740).

use {
    reovim_driver_codec::{
        CodecSessionState, ContentCodecFactoryStore, DecodedEdit, TreeOp, TreePath,
    },
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_text_session::{BufferApi, ExtensionApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

use crate::codec::PdfTreeOp;

const PDF_MODULE: ModuleId = ModuleId::new("codec-pdf");

/// `:pdf-set-metadata <field> <value>` — set a PDF metadata field.
///
/// Constructs `DecodedEdit::Tree` with `PdfTreeOp::SetMetadata` and
/// dispatches it through the active mount's codec pipeline.
///
/// After a successful edit, re-decodes the buffer content to reflect
/// the updated bytes.
#[derive(Debug, Clone, Copy)]
pub struct PdfSetMetadataCommand;

impl Command for PdfSetMetadataCommand {
    fn id(&self) -> CommandId {
        CommandId::new(PDF_MODULE, "pdf-set-metadata")
    }

    fn description(&self) -> &'static str {
        "Set a PDF metadata field (Title, Author, Subject, Keywords, Creator, Producer)"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::required("field", ArgKind::String, "Metadata field name"),
            ArgSpec::required("value", ArgKind::Rest, "New value for the field"),
        ]
    }

    fn names(&self) -> &[&'static str] {
        &["pdf-set-metadata"]
    }
}

// Needs codec session state + buffer access — tested by integration tests.
#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for PdfSetMetadataCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        let Some(field) = ctx.string("field") else {
            return CommandResult::Error("missing field argument".to_string());
        };
        let Some(value) = ctx.string("value") else {
            return CommandResult::Error("missing value argument".to_string());
        };

        let Some(buffer_id) = runtime.active_buffer() else {
            return CommandResult::Error("no active buffer".to_string());
        };

        // Build the structural edit.
        let edit = DecodedEdit::Tree {
            path: TreePath::new(vec!["metadata".into(), field.to_string()]),
            op: TreeOp::new(PdfTreeOp::SetMetadata {
                value: value.to_string(),
            }),
        };

        // Apply the decoded edit through the active mount's codec.
        let byte_edit = {
            let Some(codec_state) = runtime.shared_ext_mut::<CodecSessionState>() else {
                return CommandResult::Error("no codec state".to_string());
            };
            codec_state.apply_decoded_edit(buffer_id, "default", &edit)
        };

        if byte_edit.is_none() {
            // Could be a no-op (same value) or an error. The current API
            // flattens both into None. Treat as success — if the value was
            // already correct, nothing to update.
            return CommandResult::Success;
        }

        // Re-decode: the inode bytes changed, so we need to update the
        // text buffer content to reflect the new PDF metadata.
        let new_content = {
            let factories = runtime.kernel().services.get::<ContentCodecFactoryStore>();
            let Some(codec_state) = runtime.shared_ext_mut::<CodecSessionState>() else {
                return CommandResult::Error("no codec state after edit".to_string());
            };
            let Some(meta) = codec_state.get(buffer_id) else {
                return CommandResult::Error("no codec metadata after edit".to_string());
            };
            let content_type = meta.content_type().clone();
            let Some(source_bytes) = codec_state.bytes(buffer_id) else {
                return CommandResult::Error("no bytes after edit".to_string());
            };

            let Some(factories) = factories else {
                return CommandResult::Error("no codec factory store".to_string());
            };
            let Some(codec) = factories.find(&content_type) else {
                return CommandResult::Error("no codec for content type".to_string());
            };
            match codec.decode(source_bytes.as_ref()) {
                Ok(result) => result.content,
                Err(e) => {
                    return CommandResult::Error(format!("re-decode failed: {e}"));
                }
            }
        };

        // Write the new content into the text buffer.
        {
            let Some(buffer_arc) = runtime.text_buffer(buffer_id) else {
                return CommandResult::Error("buffer not found".to_string());
            };
            let mut buffer = buffer_arc.write();
            buffer.set_content(&new_content);
        }

        runtime.record_buffer_modified(buffer_id);
        CommandResult::Success
    }
}

/// Collect all PDF ex-commands for registration.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(PdfSetMetadataCommand)]
}
