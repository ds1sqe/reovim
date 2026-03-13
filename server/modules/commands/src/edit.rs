//! Edit command - open/reload files.
//!
//! Implements the `:e` (edit) command for opening files in buffers.

use std::path::Path;

use {
    reovim_driver_codec::{
        CodecSessionState, ContentClassifierStore, ContentCodecFactoryStore, ContentType,
    },
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{BufferApi, ExtensionApi, SessionRuntime},
    reovim_kernel::api::v1::{
        CommandId, ModuleId,
        events::kernel::{FileOpened, FileTypeChanged},
    },
};

const COMMANDS_MODULE: ModuleId = ModuleId::new("commands");

/// Edit command - open a file in the current buffer.
#[derive(Debug, Clone, Copy)]
pub struct EditCommand;

impl Command for EditCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "edit")
    }

    fn description(&self) -> &'static str {
        "Edit (open) a file in the current buffer"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("file", ArgKind::Rest, "File to edit")]
    }

    fn names(&self) -> &[&'static str] {
        &["e", "edit"]
    }
}

impl CommandHandler for EditCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        // Get the filename argument
        let Some(filename) = ctx.string("file") else {
            return CommandResult::Error(
                "invalid arguments: No filename specified. Reload not yet implemented.".to_string(),
            );
        };

        // Get the buffer to operate on
        let Some(buffer_id) = ctx.buffer_id() else {
            return CommandResult::Error("no buffer".to_string());
        };

        // Get VFS for file operations
        let Some(vfs) = ctx.vfs() else {
            return CommandResult::Error("execution failed: VFS not available".to_string());
        };

        // Read raw bytes from file
        let bytes = match vfs.read(Path::new(filename)) {
            Ok(b) => b,
            Err(e) => {
                return CommandResult::Error(format!(
                    "execution failed: Cannot read file '{filename}': {e}"
                ));
            }
        };

        // Decode via codec pipeline
        let content = match decode_file_content(&bytes, filename, runtime) {
            Ok(text) => text,
            Err(e) => {
                return CommandResult::Error(format!("execution failed: {e}"));
            }
        };

        // Get buffer and set content, then emit events.
        // Scope the immutable `runtime.kernel()` borrow so we can call
        // `record_buffer_modified` (which needs `&mut self`) afterwards.
        {
            let kernel = runtime.kernel();
            let Some(buffer_arc) = kernel.buffers.get(buffer_id) else {
                return CommandResult::Error(format!(
                    "execution failed: Buffer {} not found",
                    buffer_id.as_usize()
                ));
            };

            // Update buffer content
            {
                let mut buffer = buffer_arc.write();
                buffer.set_content(&content);
                buffer.set_file_path(Some(filename.to_string()));
                buffer.set_modified(false);
            }

            // Emit FileOpened event for subscribers (LSP, syntax, etc.)
            #[allow(clippy::cast_possible_truncation)]
            let buffer_id_raw = buffer_id.as_usize() as u64;
            kernel.event_bus.emit(FileOpened {
                buffer_id: buffer_id_raw,
                path: filename.to_string(),
            });

            // Emit FileTypeChanged if we can detect the language from the extension
            if let Some(file_type) = file_type_from_extension(filename) {
                kernel.event_bus.emit(FileTypeChanged {
                    buffer_id: buffer_id_raw,
                    file_type: file_type.to_string(),
                });
            }
        }

        // Record buffer modification so the notification pipeline emits
        // BufferModified to TUI clients, triggering a buffer cache refresh
        // and viewport redraw.
        runtime.record_buffer_modified(buffer_id);

        CommandResult::Success
    }
}

/// Decode file content through the codec pipeline.
///
/// Uses the classifier store to detect content type, then the factory store
/// to find and invoke the appropriate codec. Falls back to `String::from_utf8`
/// if no codec modules are loaded.
///
/// Stores codec metadata in `CodecSessionState` for round-trip save.
fn decode_file_content(
    bytes: &[u8],
    filename: &str,
    runtime: &mut SessionRuntime<'_>,
) -> Result<String, String> {
    // Try to use the codec pipeline
    let services = &runtime.kernel().services;
    let classifier_store = services.get::<ContentClassifierStore>();
    let factory_store = services.get::<ContentCodecFactoryStore>();

    if let (Some(classifiers), Some(factories)) = (classifier_store, factory_store) {
        // Classify the content type (None = treat as UTF-8 fallback)
        let content_type = classifiers
            .classify(bytes, filename)
            .unwrap_or_else(|| ContentType::new(ContentType::UTF8));

        // Find and create the codec
        if let Some(codec) = factories.find(&content_type) {
            match codec.decode(bytes) {
                Ok(result) => {
                    let content = result.content;

                    // Store metadata for round-trip save (includes readonly flag)
                    if let Some(buffer_id) = runtime.active_buffer() {
                        let codec_state = runtime.ext_mut::<CodecSessionState>();
                        codec_state.insert(buffer_id, result.metadata);
                    }

                    return Ok(content);
                }
                Err(e) => {
                    tracing::warn!(
                        "Codec decode failed for {filename}: {e}, falling back to UTF-8"
                    );
                }
            }
        }
    }

    // Graceful fallback: no codec module loaded or codec not found
    String::from_utf8(bytes.to_vec()).map_err(|e| {
        let offset = e.utf8_error().valid_up_to();
        format!("File is not valid UTF-8 (invalid byte at offset {offset})")
    })
}

/// Detect file type from file extension for `FileTypeChanged` events.
fn file_type_from_extension(filename: &str) -> Option<&'static str> {
    let ext_os = Path::new(filename).extension()?.to_str()?;
    let ext = ext_os.to_ascii_lowercase();
    match ext.as_str() {
        "rs" => Some("rust"),
        "py" | "pyi" => Some("python"),
        "ts" => Some("typescript"),
        "tsx" => Some("typescriptreact"),
        "js" => Some("javascript"),
        "jsx" => Some("javascriptreact"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" => Some("cpp"),
        "go" => Some("go"),
        "java" => Some("java"),
        "lua" => Some("lua"),
        "rb" => Some("ruby"),
        "zig" => Some("zig"),
        "toml" => Some("toml"),
        "json" => Some("json"),
        "yaml" | "yml" => Some("yaml"),
        "md" | "markdown" => Some("markdown"),
        _ => None,
    }
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
