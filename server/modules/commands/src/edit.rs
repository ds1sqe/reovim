//! Edit command - open/reload files.
//!
//! Implements the `:e` (edit) command for opening files in buffers.

use std::path::Path;

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
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
mod tests {
    use super::*;

    #[test]
    fn test_edit_command_id() {
        let cmd = EditCommand;
        assert_eq!(cmd.id().name(), "edit");
        assert_eq!(cmd.id().module().as_str(), "commands");
    }

    #[test]
    fn test_edit_command_names() {
        let cmd = EditCommand;
        let names = cmd.names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"e"));
        assert!(names.contains(&"edit"));
    }

    #[test]
    fn test_edit_command_description() {
        let cmd = EditCommand;
        let desc = cmd.description();
        assert!(!desc.is_empty());
        assert!(desc.contains("Edit"));
    }

    #[test]
    fn test_edit_command_complete_returns_empty() {
        let cmd = EditCommand;
        let completions = cmd.complete("some_path");
        assert!(completions.is_empty());
    }

    #[test]
    fn test_edit_command_debug() {
        let cmd = EditCommand;
        let debug = format!("{cmd:?}");
        assert!(debug.contains("EditCommand"));
    }

    #[test]
    fn test_edit_command_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<EditCommand>();
    }

    #[test]
    fn test_edit_command_clone_copy() {
        let cmd = EditCommand;
        let copy = cmd;
        assert_eq!(cmd.id(), copy.id());
    }

    // ========================================================================
    // file_type_from_extension tests
    // ========================================================================

    #[test]
    fn test_file_type_rust() {
        assert_eq!(file_type_from_extension("main.rs"), Some("rust"));
    }

    #[test]
    fn test_file_type_python() {
        assert_eq!(file_type_from_extension("app.py"), Some("python"));
        assert_eq!(file_type_from_extension("types.pyi"), Some("python"));
    }

    #[test]
    fn test_file_type_typescript() {
        assert_eq!(file_type_from_extension("index.ts"), Some("typescript"));
        assert_eq!(file_type_from_extension("App.tsx"), Some("typescriptreact"));
    }

    #[test]
    fn test_file_type_javascript() {
        assert_eq!(file_type_from_extension("app.js"), Some("javascript"));
        assert_eq!(file_type_from_extension("App.jsx"), Some("javascriptreact"));
    }

    #[test]
    fn test_file_type_c_family() {
        assert_eq!(file_type_from_extension("main.c"), Some("c"));
        assert_eq!(file_type_from_extension("header.h"), Some("c"));
        assert_eq!(file_type_from_extension("main.cpp"), Some("cpp"));
        assert_eq!(file_type_from_extension("main.cc"), Some("cpp"));
        assert_eq!(file_type_from_extension("main.cxx"), Some("cpp"));
        assert_eq!(file_type_from_extension("header.hpp"), Some("cpp"));
    }

    #[test]
    fn test_file_type_other_languages() {
        assert_eq!(file_type_from_extension("main.go"), Some("go"));
        assert_eq!(file_type_from_extension("Main.java"), Some("java"));
        assert_eq!(file_type_from_extension("init.lua"), Some("lua"));
        assert_eq!(file_type_from_extension("app.rb"), Some("ruby"));
        assert_eq!(file_type_from_extension("main.zig"), Some("zig"));
    }

    #[test]
    fn test_file_type_config_formats() {
        assert_eq!(file_type_from_extension("Cargo.toml"), Some("toml"));
        assert_eq!(file_type_from_extension("data.json"), Some("json"));
        assert_eq!(file_type_from_extension("config.yaml"), Some("yaml"));
        assert_eq!(file_type_from_extension("config.yml"), Some("yaml"));
    }

    #[test]
    fn test_file_type_markdown() {
        assert_eq!(file_type_from_extension("README.md"), Some("markdown"));
        assert_eq!(file_type_from_extension("doc.markdown"), Some("markdown"));
    }

    #[test]
    fn test_file_type_case_insensitive() {
        assert_eq!(file_type_from_extension("MAIN.RS"), Some("rust"));
        assert_eq!(file_type_from_extension("App.Py"), Some("python"));
        assert_eq!(file_type_from_extension("index.TS"), Some("typescript"));
        assert_eq!(file_type_from_extension("main.CPP"), Some("cpp"));
    }

    #[test]
    fn test_file_type_unknown_extension() {
        assert_eq!(file_type_from_extension("data.xyz"), None);
        assert_eq!(file_type_from_extension("binary.bin"), None);
    }

    #[test]
    fn test_file_type_no_extension() {
        assert_eq!(file_type_from_extension("Makefile"), None);
        assert_eq!(file_type_from_extension("Dockerfile"), None);
    }

    // ========================================================================
    // Execute tests (using TestSessionRuntime + MockVfs)
    // ========================================================================

    #[test]
    fn test_edit_execute_no_filename() {
        use reovim_driver_session::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");
        harness.with_runtime(|runtime| {
            let cmd = EditCommand;
            let ctx = CommandContext::new();
            let result = cmd.execute(runtime, &ctx);
            assert!(result.is_error());
        });
    }

    #[test]
    fn test_edit_execute_no_buffer_id() {
        use reovim_driver_session::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");
        harness.with_runtime(|runtime| {
            let cmd = EditCommand;
            let mut ctx = CommandContext::new();
            ctx.set(
                "file",
                reovim_driver_command::ArgValue::String("test.rs".to_string()),
            );
            // No buffer_id set
            let result = cmd.execute(runtime, &ctx);
            assert!(result.is_error());
        });
    }

    #[test]
    fn test_edit_execute_no_vfs() {
        use reovim_driver_session::testing::TestSessionRuntime;

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let buffer_id = harness.active_buffer().unwrap();
        harness.with_runtime(|runtime| {
            let cmd = EditCommand;
            let mut ctx = CommandContext::new();
            ctx.set(
                "file",
                reovim_driver_command::ArgValue::String("test.rs".to_string()),
            );
            ctx.set_buffer_id(buffer_id);
            // No VFS set
            let result = cmd.execute(runtime, &ctx);
            assert!(result.is_error());
        });
    }

    #[test]
    fn test_edit_execute_file_not_found() {
        use {
            reovim_driver_session::testing::TestSessionRuntime, reovim_driver_vfs::MockVfs,
            std::sync::Arc,
        };

        let mut harness = TestSessionRuntime::with_buffer("hello");
        let buffer_id = harness.active_buffer().unwrap();
        let mock_vfs = Arc::new(MockVfs::new());
        // No files added → read will fail

        harness.with_runtime(|runtime| {
            let cmd = EditCommand;
            let mut ctx = CommandContext::new();
            ctx.set(
                "file",
                reovim_driver_command::ArgValue::String("nonexistent.rs".to_string()),
            );
            ctx.set_buffer_id(buffer_id);
            ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_driver_vfs::VfsDriver>);
            let result = cmd.execute(runtime, &ctx);
            assert!(result.is_error());
        });
    }

    #[test]
    fn test_edit_execute_success() {
        use {
            reovim_driver_session::testing::TestSessionRuntime, reovim_driver_vfs::MockVfs,
            std::sync::Arc,
        };

        let mut harness = TestSessionRuntime::with_buffer("original");
        let buffer_id = harness.active_buffer().unwrap();
        let mock_vfs = Arc::new(MockVfs::new());
        mock_vfs.add_file_str("/tmp/test.rs", "fn main() {}");

        harness.with_runtime(|runtime| {
            let cmd = EditCommand;
            let mut ctx = CommandContext::new();
            ctx.set(
                "file",
                reovim_driver_command::ArgValue::String("/tmp/test.rs".to_string()),
            );
            ctx.set_buffer_id(buffer_id);
            ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_driver_vfs::VfsDriver>);
            let result = cmd.execute(runtime, &ctx);
            assert!(result.is_success());
        });
    }

    #[test]
    fn test_edit_execute_invalid_utf8() {
        use {
            reovim_driver_session::testing::TestSessionRuntime, reovim_driver_vfs::MockVfs,
            std::sync::Arc,
        };

        let mut harness = TestSessionRuntime::with_buffer("original");
        let buffer_id = harness.active_buffer().unwrap();
        let mock_vfs = Arc::new(MockVfs::new());
        mock_vfs.add_file("/tmp/binary.bin", &[0xFF, 0xFE, 0x80, 0x90]);

        harness.with_runtime(|runtime| {
            let cmd = EditCommand;
            let mut ctx = CommandContext::new();
            ctx.set(
                "file",
                reovim_driver_command::ArgValue::String("/tmp/binary.bin".to_string()),
            );
            ctx.set_buffer_id(buffer_id);
            ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_driver_vfs::VfsDriver>);
            let result = cmd.execute(runtime, &ctx);
            assert!(result.is_error());
        });
    }
}
