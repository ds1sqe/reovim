//! Edit command - open/reload files.
//!
//! Implements the `:e` (edit) command for opening files in buffers.
//!
//! # Behavior
//!
//! - `:e filename` - Open file in current buffer
//! - `:e` - Reload current file (not yet implemented)
//!
//! # Note
//!
//! This is a minimal implementation sufficient for the `vim_commands.rs`
//! integration tests. The `IntegrationTest` harness uses `:e` to open
//! files with specific content for testing.

use std::path::Path;

use {
    crate::{CommandError, ExCommandContext, ExCommandHandler},
    reovim_kernel::api::v1::events::kernel::{FileOpened, FileTypeChanged},
};

/// Edit command - open a file in the current buffer.
///
/// # Example
///
/// ```ignore
/// let edit = EditCommand;
/// edit.execute(&mut ctx, &["file.txt"])?; // :e file.txt
/// ```
#[derive(Debug, Clone, Copy)]
pub struct EditCommand;

impl ExCommandHandler for EditCommand {
    fn id(&self) -> &'static str {
        "edit"
    }

    fn names(&self) -> &[&'static str] {
        &["e", "edit"]
    }

    fn execute(&self, ctx: &mut ExCommandContext<'_>, args: &[&str]) -> Result<(), CommandError> {
        // Get the filename argument
        let filename = match args.first() {
            Some(f) => *f,
            None => {
                // :e without argument - reload current file (not implemented)
                return Err(CommandError::InvalidArguments(
                    "No filename specified. Reload not yet implemented.".to_string(),
                ));
            }
        };

        // Get the buffer to operate on
        let buffer_id = ctx.buffer_id.ok_or(CommandError::NoBuffer)?;

        // Get VFS for file operations
        let vfs = ctx
            .vfs()
            .ok_or_else(|| CommandError::ExecutionFailed("VFS not available".to_string()))?;

        // Read file content
        let content = match vfs.read(Path::new(filename)) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(_) => {
                    return Err(CommandError::ExecutionFailed(
                        "File is not valid UTF-8".to_string(),
                    ));
                }
            },
            Err(e) => {
                return Err(CommandError::ExecutionFailed(format!(
                    "Cannot read file '{filename}': {e}"
                )));
            }
        };

        // Get buffer and set content
        let buffer_arc = ctx.kernel.buffers.get(buffer_id).ok_or_else(|| {
            CommandError::ExecutionFailed(format!("Buffer {} not found", buffer_id.as_usize()))
        })?;

        // Update buffer content
        {
            let mut buffer = buffer_arc.write();

            // Set new content (this also resets cursor to origin)
            buffer.set_content(&content);

            // Set metadata - mark as unmodified since we just loaded
            buffer.set_file_path(Some(filename.to_string()));
            buffer.set_modified(false);
        }

        // Emit FileOpened event for subscribers (LSP, syntax, etc.)
        let buffer_id_raw = buffer_id.as_usize() as u64;
        ctx.kernel.event_bus.emit(FileOpened {
            buffer_id: buffer_id_raw,
            path: filename.to_string(),
        });

        // Emit FileTypeChanged if we can detect the language from the extension
        if let Some(file_type) = file_type_from_extension(filename) {
            ctx.kernel.event_bus.emit(FileTypeChanged {
                buffer_id: buffer_id_raw,
                file_type: file_type.to_string(),
            });
        }

        Ok(())
    }

    fn complete(&self, partial: &str) -> Vec<String> {
        // File path completion would go here
        // For now, return empty - actual implementation uses VFS driver
        let _ = partial;
        vec![]
    }

    fn help(&self) -> &'static str {
        "Edit (open) a file in the current buffer"
    }
}

/// Detect file type from file extension for `FileTypeChanged` events.
///
/// Returns the LSP-compatible language identifier, or `None` for unknown extensions.
/// This will be unified into a shared `FiletypeRegistry` per #530.
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
    use {
        super::*,
        reovim_kernel::api::v1::{BufferId, KernelContext},
    };

    #[test]
    fn test_edit_command_id() {
        let cmd = EditCommand;
        assert_eq!(cmd.id(), "edit");
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
    fn test_edit_command_help() {
        let cmd = EditCommand;
        let help = cmd.help();
        assert!(!help.is_empty());
        assert!(help.contains("Edit"));
    }

    #[test]
    fn test_edit_command_complete_returns_empty() {
        let cmd = EditCommand;
        let completions = cmd.complete("some_path");
        assert!(completions.is_empty());
    }

    #[test]
    fn test_edit_command_complete_empty_input() {
        let cmd = EditCommand;
        let completions = cmd.complete("");
        assert!(completions.is_empty());
    }

    #[test]
    fn test_edit_command_execute_no_args_returns_error() {
        let kernel = KernelContext::default();
        let buffer_id = BufferId::from_raw(1);
        let mut ctx = ExCommandContext::new(&kernel).with_buffer(buffer_id);

        let cmd = EditCommand;
        let result = cmd.execute(&mut ctx, &[]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let display = err.to_string();
        assert!(display.contains("invalid arguments"));
        assert!(display.contains("No filename"));
    }

    #[test]
    fn test_edit_command_execute_no_buffer_returns_error() {
        let kernel = KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);
        // No buffer_id, no VFS

        let cmd = EditCommand;
        // With a filename arg, it will try to get buffer_id which is None
        let result = cmd.execute(&mut ctx, &["file.txt"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "no buffer");
    }

    #[test]
    fn test_edit_command_execute_no_vfs_returns_error() {
        let kernel = KernelContext::default();
        let buffer_id = BufferId::from_raw(1);
        let mut ctx = ExCommandContext::new(&kernel).with_buffer(buffer_id);
        // ctx.vfs is None by default

        let cmd = EditCommand;
        let result = cmd.execute(&mut ctx, &["file.txt"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let display = err.to_string();
        assert!(display.contains("VFS not available"));
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

    /// Simple in-memory buffer manager for testing.
    struct TestBufferManager {
        buffers: reovim_arch::sync::RwLock<
            std::collections::HashMap<
                BufferId,
                std::sync::Arc<reovim_arch::sync::RwLock<reovim_kernel::api::v1::Buffer>>,
            >,
        >,
    }

    impl TestBufferManager {
        fn new() -> Self {
            Self {
                buffers: reovim_arch::sync::RwLock::new(std::collections::HashMap::new()),
            }
        }
    }

    impl reovim_kernel::api::v1::BufferManager for TestBufferManager {
        fn get(
            &self,
            id: BufferId,
        ) -> Option<std::sync::Arc<reovim_arch::sync::RwLock<reovim_kernel::api::v1::Buffer>>>
        {
            self.buffers.read().get(&id).cloned()
        }

        fn create(&self) -> BufferId {
            let id = BufferId::new();
            let buffer = std::sync::Arc::new(reovim_arch::sync::RwLock::new(
                reovim_kernel::api::v1::Buffer::new(),
            ));
            self.buffers.write().insert(id, buffer);
            id
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        fn register(&self, buffer: reovim_kernel::api::v1::Buffer) -> BufferId {
            let id = BufferId::new();
            let buffer = std::sync::Arc::new(reovim_arch::sync::RwLock::new(buffer));
            self.buffers.write().insert(id, buffer);
            id
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        fn unregister(
            &self,
            id: BufferId,
        ) -> Result<reovim_kernel::api::v1::Buffer, reovim_kernel::api::v1::BufferError> {
            self.buffers.write().remove(&id).map_or(
                Err(reovim_kernel::api::v1::BufferError::NotFound(id)),
                |arc_buffer| {
                    std::sync::Arc::try_unwrap(arc_buffer)
                        .map_or_else(|arc| Ok(arc.read().clone()), |rwlock| Ok(rwlock.into_inner()))
                },
            )
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        fn list(&self) -> Vec<BufferId> {
            self.buffers.read().keys().copied().collect()
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        fn count(&self) -> usize {
            self.buffers.read().len()
        }
    }

    /// Create a `KernelContext` with a functional buffer manager.
    fn make_kernel_with_buffers() -> KernelContext {
        use std::sync::Arc;

        KernelContext {
            buffers: Arc::new(TestBufferManager::new()),
            ..KernelContext::default()
        }
    }

    #[test]
    fn test_edit_command_execute_file_not_found() {
        use {reovim_driver_vfs::MockVfs, std::sync::Arc};

        let kernel = make_kernel_with_buffers();
        let buffer_id = kernel.buffers.create();
        let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> = Arc::new(MockVfs::new());
        let mut ctx = ExCommandContext::new(&kernel)
            .with_buffer(buffer_id)
            .with_vfs(vfs);

        let cmd = EditCommand;
        let result = cmd.execute(&mut ctx, &["/nonexistent/file.txt"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let display = err.to_string();
        assert!(display.contains("Cannot read file"), "got: {display}");
    }

    #[test]
    fn test_edit_command_execute_non_utf8() {
        use {reovim_driver_vfs::MockVfs, std::sync::Arc};

        let kernel = make_kernel_with_buffers();
        let buffer_id = kernel.buffers.create();
        let mock_vfs = MockVfs::new();
        // Add a file with invalid UTF-8 bytes
        mock_vfs.add_file("/binary.bin", [0xFF, 0xFE, 0x80, 0x81]);
        let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> = Arc::new(mock_vfs);
        let mut ctx = ExCommandContext::new(&kernel)
            .with_buffer(buffer_id)
            .with_vfs(vfs);

        let cmd = EditCommand;
        let result = cmd.execute(&mut ctx, &["/binary.bin"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let display = err.to_string();
        assert!(display.contains("not valid UTF-8"), "got: {display}");
    }

    #[test]
    fn test_edit_command_execute_buffer_not_in_kernel() {
        use {reovim_driver_vfs::MockVfs, std::sync::Arc};

        let kernel = make_kernel_with_buffers();
        // Use a raw buffer ID that isn't registered in the kernel
        let buffer_id = BufferId::from_raw(999);
        let mock_vfs = MockVfs::new();
        mock_vfs.add_file_str("/test.txt", "hello world");
        let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> = Arc::new(mock_vfs);
        let mut ctx = ExCommandContext::new(&kernel)
            .with_buffer(buffer_id)
            .with_vfs(vfs);

        let cmd = EditCommand;
        let result = cmd.execute(&mut ctx, &["/test.txt"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let display = err.to_string();
        assert!(display.contains("not found"), "got: {display}");
    }

    #[test]
    fn test_edit_command_execute_success() {
        use {reovim_driver_vfs::MockVfs, std::sync::Arc};

        let kernel = make_kernel_with_buffers();
        let buffer_id = kernel.buffers.create();
        let mock_vfs = MockVfs::new();
        mock_vfs.add_file_str("/test.txt", "hello world");
        let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> = Arc::new(mock_vfs);
        let mut ctx = ExCommandContext::new(&kernel)
            .with_buffer(buffer_id)
            .with_vfs(vfs);

        let cmd = EditCommand;
        let result = cmd.execute(&mut ctx, &["/test.txt"]);
        assert!(result.is_ok(), "edit command should succeed, got: {:?}", result.err());

        // Verify buffer content was updated
        let buffer_arc = kernel.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.content(), "hello world");
        assert_eq!(buffer.file_path(), Some("/test.txt"));
        assert!(!buffer.is_modified());
        drop(buffer);
    }

    #[test]
    fn test_edit_command_execute_empty_file() {
        use {reovim_driver_vfs::MockVfs, std::sync::Arc};

        let kernel = make_kernel_with_buffers();
        let buffer_id = kernel.buffers.create();
        let mock_vfs = MockVfs::new();
        mock_vfs.add_file_str("/empty.txt", "");
        let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> = Arc::new(mock_vfs);
        let mut ctx = ExCommandContext::new(&kernel)
            .with_buffer(buffer_id)
            .with_vfs(vfs);

        let cmd = EditCommand;
        let result = cmd.execute(&mut ctx, &["/empty.txt"]);
        assert!(result.is_ok());

        let buffer_arc = kernel.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.content(), "");
        assert!(!buffer.is_modified());
        drop(buffer);
    }

    #[test]
    fn test_edit_command_execute_multiline_file() {
        use {reovim_driver_vfs::MockVfs, std::sync::Arc};

        let kernel = make_kernel_with_buffers();
        let buffer_id = kernel.buffers.create();
        let mock_vfs = MockVfs::new();
        mock_vfs.add_file_str("/multi.txt", "line1\nline2\nline3");
        let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> = Arc::new(mock_vfs);
        let mut ctx = ExCommandContext::new(&kernel)
            .with_buffer(buffer_id)
            .with_vfs(vfs);

        let cmd = EditCommand;
        let result = cmd.execute(&mut ctx, &["/multi.txt"]);
        assert!(result.is_ok());

        let buffer_arc = kernel.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert!(buffer.content().contains("line1"));
        assert!(buffer.content().contains("line3"));
        assert_eq!(buffer.file_path(), Some("/multi.txt"));
        drop(buffer);
    }

    #[test]
    fn test_edit_command_sets_file_path() {
        use {reovim_driver_vfs::MockVfs, std::sync::Arc};

        let kernel = make_kernel_with_buffers();
        let buffer_id = kernel.buffers.create();
        let mock_vfs = MockVfs::new();
        mock_vfs.add_file_str("/path/to/file.rs", "fn main() {}");
        let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> = Arc::new(mock_vfs);
        let mut ctx = ExCommandContext::new(&kernel)
            .with_buffer(buffer_id)
            .with_vfs(vfs);

        let cmd = EditCommand;
        let result = cmd.execute(&mut ctx, &["/path/to/file.rs"]);
        assert!(result.is_ok());

        let buffer_arc = kernel.buffers.get(buffer_id).unwrap();
        let buffer = buffer_arc.read();
        assert_eq!(buffer.file_path(), Some("/path/to/file.rs"));
        drop(buffer);
    }

    #[test]
    fn test_edit_command_clone_copy() {
        let cmd = EditCommand;
        let copy = cmd;
        // Both should be valid since it's Copy
        assert_eq!(cmd.id(), "edit");
        assert_eq!(copy.id(), "edit");
    }

    // ========================================================================
    // Event emission tests
    // ========================================================================

    #[test]
    fn test_edit_emits_file_opened_event() {
        use {
            reovim_driver_vfs::MockVfs,
            reovim_kernel::api::v1::{EventResult, events::kernel::FileOpened},
            std::sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
        };

        let kernel = make_kernel_with_buffers();
        let buffer_id = kernel.buffers.create();
        let mock_vfs = MockVfs::new();
        mock_vfs.add_file_str("/test.rs", "fn main() {}");
        let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> = Arc::new(mock_vfs);

        let received = Arc::new(AtomicBool::new(false));
        let received_clone = Arc::clone(&received);
        let _sub = kernel
            .event_bus
            .subscribe::<FileOpened, _>(100, move |event| {
                assert_eq!(event.path, "/test.rs");
                assert_eq!(event.buffer_id, buffer_id.as_usize() as u64);
                received_clone.store(true, Ordering::Relaxed);
                EventResult::Handled
            });

        let mut ctx = ExCommandContext::new(&kernel)
            .with_buffer(buffer_id)
            .with_vfs(vfs);
        EditCommand.execute(&mut ctx, &["/test.rs"]).unwrap();
        assert!(received.load(Ordering::Relaxed), "FileOpened event not emitted");
    }

    #[test]
    fn test_edit_emits_file_type_changed_for_known_extension() {
        use {
            reovim_driver_vfs::MockVfs,
            reovim_kernel::api::v1::{EventResult, events::kernel::FileTypeChanged},
            std::sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
        };

        let kernel = make_kernel_with_buffers();
        let buffer_id = kernel.buffers.create();
        let mock_vfs = MockVfs::new();
        mock_vfs.add_file_str("/main.rs", "fn main() {}");
        let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> = Arc::new(mock_vfs);

        let received = Arc::new(AtomicBool::new(false));
        let received_clone = Arc::clone(&received);
        let expected_bid = buffer_id.as_usize() as u64;
        let _sub = kernel
            .event_bus
            .subscribe::<FileTypeChanged, _>(100, move |event| {
                assert_eq!(event.file_type, "rust");
                assert_eq!(event.buffer_id, expected_bid);
                received_clone.store(true, Ordering::Relaxed);
                EventResult::Handled
            });

        let mut ctx = ExCommandContext::new(&kernel)
            .with_buffer(buffer_id)
            .with_vfs(vfs);
        EditCommand.execute(&mut ctx, &["/main.rs"]).unwrap();
        assert!(received.load(Ordering::Relaxed), "FileTypeChanged event not emitted for .rs");
    }

    #[test]
    fn test_edit_no_file_type_changed_for_unknown_extension() {
        use {
            reovim_driver_vfs::MockVfs,
            reovim_kernel::api::v1::{EventResult, events::kernel::FileTypeChanged},
            std::sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
        };

        let kernel = make_kernel_with_buffers();
        let buffer_id = kernel.buffers.create();
        let mock_vfs = MockVfs::new();
        mock_vfs.add_file_str("/data.xyz", "some content");
        let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> = Arc::new(mock_vfs);

        let received = Arc::new(AtomicBool::new(false));
        let received_clone = Arc::clone(&received);
        let _sub = kernel
            .event_bus
            .subscribe::<FileTypeChanged, _>(100, move |event| {
                let _ = event;
                received_clone.store(true, Ordering::Relaxed);
                EventResult::Handled
            });

        let mut ctx = ExCommandContext::new(&kernel)
            .with_buffer(buffer_id)
            .with_vfs(vfs);
        EditCommand.execute(&mut ctx, &["/data.xyz"]).unwrap();
        assert!(
            !received.load(Ordering::Relaxed),
            "FileTypeChanged should NOT be emitted for unknown extension"
        );
    }

    #[test]
    fn test_edit_no_events_on_failure() {
        use {
            reovim_kernel::api::v1::{EventResult, events::kernel::FileOpened},
            std::sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
        };

        let kernel = KernelContext::default();
        let buffer_id = BufferId::from_raw(1);

        let received = Arc::new(AtomicBool::new(false));
        let received_clone = Arc::clone(&received);
        let _sub = kernel.event_bus.subscribe::<FileOpened, _>(100, move |_| {
            received_clone.store(true, Ordering::Relaxed);
            EventResult::Handled
        });

        // No VFS -> should fail before emitting events
        let mut ctx = ExCommandContext::new(&kernel).with_buffer(buffer_id);
        let result = EditCommand.execute(&mut ctx, &["file.txt"]);
        assert!(result.is_err());
        assert!(!received.load(Ordering::Relaxed), "FileOpened should NOT be emitted on failure");
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
}
