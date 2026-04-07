//! Write commands.

use std::path::Path;

use {
    reovim_driver_codec::{CodecSessionState, ContentCodecFactoryStore},
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult, RuntimeSignal,
    },
    reovim_driver_session::{BufferApi, CommandApi, ExtensionApi, SessionRuntime},
    reovim_kernel::api::v1::{
        CommandId, ModuleId,
        events::kernel::{BufferSaved, BufferWillSave},
    },
    reovim_provider_text::BufferCapabilities,
};

const COMMANDS_MODULE: ModuleId = ModuleId::new("commands");

/// Write command - save the buffer to disk.
///
/// Behavior:
/// - `:w` - Write current buffer to its file
/// - `:w filename` - Write current buffer to specified file
#[derive(Debug, Clone, Copy)]
pub struct WriteCommand;

/// Command ID for the write command (used by `WriteQuitCommand` for re-entrant call).
pub const WRITE_CMD_ID: CommandId = CommandId::new(COMMANDS_MODULE, "write");

impl Command for WriteCommand {
    fn id(&self) -> CommandId {
        WRITE_CMD_ID
    }

    fn description(&self) -> &'static str {
        "Write the current buffer to disk. Use :w filename to save to a specific file."
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("file", ArgKind::Rest, "File to write")]
    }

    fn names(&self) -> &[&'static str] {
        &["w", "write"]
    }
}

// Needs VFS + codec factories — tested by integration tests.
#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for WriteCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        let Some(buffer_id) = ctx.buffer_id() else {
            return CommandResult::Error("no buffer".to_string());
        };

        // Determine target path: explicit argument or buffer's existing path
        let explicit_file = ctx.string("file");
        let path = if let Some(file) = explicit_file {
            file.to_string()
        } else if let Some(existing) = runtime.buffer_file_path(buffer_id) {
            existing
        } else {
            return CommandResult::Error("No file name".to_string());
        };

        // Emit BufferWillSave so pre-save hooks (format-on-save) can modify content
        #[allow(clippy::cast_possible_truncation)]
        runtime.kernel().event_bus.emit(BufferWillSave {
            buffer_id: buffer_id.as_usize() as u64,
            path: path.clone(),
        });

        let Some(vfs) = ctx.vfs() else {
            return CommandResult::Error("VFS not available".to_string());
        };

        // Streaming write for STREAMABLE buffers (VirtualBuffer / mmap-backed)
        // when no codec re-encoding is needed. Avoids full String materialization
        // for large files.
        let is_streamable = runtime
            .buffer_capabilities(buffer_id)
            .is_some_and(|c| c.contains(BufferCapabilities::STREAMABLE));
        let has_codec = runtime
            .shared_ext_mut::<CodecSessionState>()
            .and_then(|cs| cs.get(buffer_id))
            .is_some();

        if is_streamable && !has_codec {
            let mut bytes = Vec::new();
            if let Err(e) = runtime.buffer_write_to(buffer_id, &mut bytes) {
                return CommandResult::Error(format!("Write failed: {e}"));
            }
            if let Err(e) = vfs.write(Path::new(&path), &bytes) {
                return CommandResult::Error(format!("Write failed: {e}"));
            }
        } else {
            // Get buffer content AFTER pre-save hooks (formatters may have modified it)
            let Some(content) = runtime.buffer_content(buffer_id) else {
                return CommandResult::Error("buffer not found".to_string());
            };
            // Encode through codec pipeline if metadata exists
            if let Err(e) = encode_and_write(runtime, &path, &content, vfs.as_ref()) {
                return CommandResult::Error(format!("Write failed: {e}"));
            }
        }

        // If saving to a new filename, update the buffer's file path
        if explicit_file.is_some() {
            runtime.rename_buffer(buffer_id, &path);
        }

        // Clear modified flag
        runtime.set_buffer_modified(buffer_id, false);

        // Emit BufferSaved event for subscribers (LSP DidSave, etc.)
        #[allow(clippy::cast_possible_truncation)]
        let buffer_id_raw = buffer_id.as_usize() as u64;
        runtime.kernel().event_bus.emit(BufferSaved {
            buffer_id: buffer_id_raw,
            path,
        });

        CommandResult::Success
    }
}

/// Encode content through codec pipeline and write to VFS.
///
/// If codec metadata exists for the buffer, uses the codec to encode
/// back to the original format. Otherwise falls back to writing UTF-8 bytes.
#[cfg_attr(coverage_nightly, coverage(off))]
fn encode_and_write(
    runtime: &mut SessionRuntime<'_>,
    path: &str,
    content: &str,
    vfs: &dyn reovim_driver_vfs::VfsDriver,
) -> Result<(), String> {
    // Check if we have codec metadata for this buffer (shared extensions = per-buffer)
    if let Some(buffer_id) = runtime.active_buffer() {
        let codec_state = runtime.shared_ext_mut::<CodecSessionState>();
        if let Some(metadata) = codec_state.and_then(|cs| cs.get(buffer_id)) {
            // Check readonly (lossy decode means writing back would corrupt data)
            if metadata.get("readonly") == Some("true") {
                return Err("buffer is read-only (lossy codec decode)".to_string());
            }

            // Try to encode via codec
            let content_type = metadata.content_type().clone();
            let metadata_clone = metadata.clone();

            let services = &runtime.kernel().services;
            if let Some(factory_store) = services.get::<ContentCodecFactoryStore>()
                && let Some(codec) = factory_store.find(&content_type)
            {
                match codec.encode(content, &metadata_clone) {
                    Some(Ok(bytes)) => {
                        return vfs
                            .write(Path::new(path), &bytes)
                            .map_err(|e| e.to_string());
                    }
                    Some(Err(e)) => {
                        return Err(format!("codec encode failed: {e}"));
                    }
                    None => {
                        return Err("buffer is read-only (one-way codec)".to_string());
                    }
                }
            }
        }
    }

    // Fallback: write as UTF-8
    vfs.write_str(Path::new(path), content)
        .map_err(|e| e.to_string())
}

/// Write and quit command - save and exit.
///
/// Uses re-entrant execution: calls `:write` first, then signals quit.
#[derive(Debug, Clone, Copy)]
pub struct WriteQuitCommand;

impl Command for WriteQuitCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "write-quit")
    }

    fn description(&self) -> &'static str {
        "Write the current buffer and quit the editor."
    }

    fn names(&self) -> &[&'static str] {
        &["wq"]
    }
}

// Re-entrant command execution — tested by integration tests.
#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for WriteQuitCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        // Write first via re-entrant execution
        let result = runtime.execute_command(WRITE_CMD_ID, ctx.clone());
        if result.is_error() {
            return result;
        }

        // Then signal quit
        runtime.signal(RuntimeSignal::Quit);
        CommandResult::Success
    }
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
