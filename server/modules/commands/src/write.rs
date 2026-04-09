//! Write commands.

use std::path::Path;

use {
    reovim_driver_codec::CodecSessionState,
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

        // #740 Plan 06 Phase 5 sub-commit 5d: consolidated `:w` path.
        //
        // Byte truth lives in `InodeTable.inode.bytes`. Every buffer
        // mutation in the runtime flows through `apply_decoded_edit`
        // (faithful codecs) or `apply_byte_edit` (non-codec / STREAMABLE
        // fallback) inside `notify_codec_indices`, so the inode bytes
        // stay in sync with the text buffer at all times. `:w` flushes
        // those bytes directly through `ByteSource::write_to`; there is
        // no `encode` call on the save path.
        //
        // Three paths remain:
        //   1. Buffer has a codec mount in `CodecSessionState` →
        //      delegate to `InodeTable::flush` via the active mount.
        //   2. STREAMABLE buffer without a codec mount (e.g.
        //      `VirtualBuffer` opened via the mmap fast path with no
        //      codec pipeline) → write via `buffer_write_to`.
        //   3. Neither codec mount nor STREAMABLE → materialize buffer
        //      content as UTF-8 and write. This is the scratch-buffer
        //      / plain rope buffer path that never went through the
        //      decode pipeline in the first place.
        let has_active_mount = runtime
            .shared_ext_mut::<CodecSessionState>()
            .and_then(|cs| cs.list_mounts(buffer_id).into_iter().next())
            .is_some();

        let write_result = if has_active_mount {
            flush_via_inode(runtime, buffer_id, &path)
        } else {
            let is_streamable = runtime
                .buffer_capabilities(buffer_id)
                .is_some_and(|c| c.contains(BufferCapabilities::STREAMABLE));
            if is_streamable {
                let mut bytes = Vec::new();
                match runtime.buffer_write_to(buffer_id, &mut bytes) {
                    Ok(()) => vfs
                        .write(Path::new(&path), &bytes)
                        .map_err(|e| e.to_string()),
                    Err(e) => Err(e.to_string()),
                }
            } else {
                let Some(content) = runtime.buffer_content(buffer_id) else {
                    return CommandResult::Error("buffer not found".to_string());
                };
                vfs.write_str(Path::new(&path), &content)
                    .map_err(|e| e.to_string())
            }
        };

        if let Err(e) = write_result {
            return CommandResult::Error(format!("Write failed: {e}"));
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

/// Phase 5 5d flush path: write canonical inode bytes through
/// [`InodeTable::flush`] without invoking any codec encode method.
#[cfg_attr(coverage_nightly, coverage(off))]
fn flush_via_inode(
    runtime: &mut SessionRuntime<'_>,
    buffer_id: reovim_kernel::api::v1::BufferId,
    path: &str,
) -> Result<(), String> {
    use reovim_driver_codec::InodeTable;

    let codec_state = runtime
        .shared_ext_mut::<CodecSessionState>()
        .ok_or_else(|| "codec state missing".to_string())?;

    let Some(mount) = codec_state.list_mounts(buffer_id).into_iter().next() else {
        return Err("buffer has no active codec mount".to_string());
    };

    codec_state
        .inode_table_mut()
        .flush(mount.mount_id, Some(Path::new(path)))
        .map_err(|e| e.to_string())?;

    // Pacify the unused-import lint on the trait — flush is called via
    // the inherent impl, but documenting the InodeTable reference here
    // keeps readers on the right breadcrumb.
    let _ = std::any::type_name::<InodeTable>();
    Ok(())
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
