//! Edit command - open/reload files.
//!
//! Implements the `:e` (edit) command for opening files in buffers.

use std::path::Path;

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::SessionRuntime,
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

        // Read file content
        let content = match vfs.read(Path::new(filename)) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(_) => {
                    return CommandResult::Error(
                        "execution failed: File is not valid UTF-8".to_string(),
                    );
                }
            },
            Err(e) => {
                return CommandResult::Error(format!(
                    "execution failed: Cannot read file '{filename}': {e}"
                ));
            }
        };

        // Get buffer and set content
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

        CommandResult::Success
    }
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
