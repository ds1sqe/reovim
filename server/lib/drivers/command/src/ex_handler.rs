//! Ex-command handler trait and types.
//!
//! This module defines the mechanism for ex-commands (`:w`, `:q`, `:e`, etc.).
//! Modules implement `ExCommandHandler` to define command behavior.
//!
//! # Architecture
//!
//! - **Mechanism (this module)**: Defines WHAT an ex-command handler is
//! - **Policy (modules)**: Implements HOW specific commands behave
//!
//! # Migrated from `server/modules/commands/src/types.rs`
//!
//! These types were moved here to enable cross-module access. The commands
//! module implements this trait, and the vim module can access handlers via
//! `ExCommandDispatcher` in `ServiceRegistry`.

use std::sync::Arc;

use {
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{BufferId, KernelContext, Position, WindowId},
};

// ============================================================================
// Range Type (for command ranges like :1,5d)
// ============================================================================

/// Text range for command execution (line-based).
///
/// Used for ex-commands that operate on line ranges (e.g., `:1,5d`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExCommandRange {
    /// Start position (inclusive).
    pub start: Position,
    /// End position (exclusive).
    pub end: Position,
}

impl ExCommandRange {
    /// Create a new range.
    #[must_use]
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}

// ============================================================================
// ExCommandHandler Trait
// ============================================================================

/// Handles ex-commands (commands entered via :).
///
/// - **Mechanism (Driver)**: This trait definition
/// - **Policy (Module)**: What `:w`, `:q`, `:e` actually do
///
/// # Implementation
///
/// Modules implement this trait to define command behavior:
///
/// ```ignore
/// use reovim_driver_command::{ExCommandHandler, ExCommandContext, ExCommandError};
///
/// struct WriteCommand;
///
/// impl ExCommandHandler for WriteCommand {
///     fn id(&self) -> &'static str { "write" }
///     fn names(&self) -> &[&'static str] { &["w", "write"] }
///
///     fn execute(&self, ctx: &mut ExCommandContext<'_>, args: &[&str])
///         -> Result<(), ExCommandError>
///     {
///         let buffer_id = ctx.buffer_id
///             .ok_or(ExCommandError::NoBuffer)?;
///         // Write the buffer to disk...
///         Ok(())
///     }
///
///     fn help(&self) -> &'static str {
///         "Write the current buffer to disk"
///     }
/// }
/// ```
pub trait ExCommandHandler: Send + Sync {
    /// Command identifier.
    fn id(&self) -> &'static str;

    /// Command names (e.g., `["w", "write"]`).
    ///
    /// The first name is the canonical name.
    fn names(&self) -> &[&'static str];

    /// Execute the command.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Execution context
    /// * `args` - Command arguments (e.g., `:w foo.txt` has args `["foo.txt"]`)
    ///
    /// # Errors
    ///
    /// Returns `ExCommandError` if the command fails.
    fn execute(&self, ctx: &mut ExCommandContext<'_>, args: &[&str]) -> Result<(), ExCommandError>;

    /// Command completion suggestions.
    ///
    /// Returns possible completions for the given partial input.
    fn complete(&self, _partial: &str) -> Vec<String> {
        vec![]
    }

    /// Help text for the command.
    fn help(&self) -> &'static str {
        ""
    }
}

/// Context passed to ex-command execution.
pub struct ExCommandContext<'a> {
    /// Kernel context for accessing services.
    pub kernel: &'a KernelContext,
    /// Current buffer (if any).
    pub buffer_id: Option<BufferId>,
    /// Current window (if any).
    pub window_id: Option<WindowId>,
    /// Whether command was invoked with ! (e.g., :q!).
    pub bang: bool,
    /// Command range (e.g., :1,5d has range `Some((1,5))`).
    pub range: Option<ExCommandRange>,
    /// VFS driver for file operations.
    pub vfs: Option<Arc<dyn VfsDriver>>,
}

impl<'a> ExCommandContext<'a> {
    /// Create a new context.
    #[must_use]
    pub fn new(kernel: &'a KernelContext) -> Self {
        Self {
            kernel,
            buffer_id: None,
            window_id: None,
            bang: false,
            range: None,
            vfs: None,
        }
    }

    /// Set the buffer ID.
    #[must_use]
    pub const fn with_buffer(mut self, buffer_id: BufferId) -> Self {
        self.buffer_id = Some(buffer_id);
        self
    }

    /// Set the window ID.
    #[must_use]
    pub const fn with_window(mut self, window_id: WindowId) -> Self {
        self.window_id = Some(window_id);
        self
    }

    /// Set the bang flag.
    #[must_use]
    pub const fn with_bang(mut self, bang: bool) -> Self {
        self.bang = bang;
        self
    }

    /// Set the range.
    #[must_use]
    pub const fn with_range(mut self, range: ExCommandRange) -> Self {
        self.range = Some(range);
        self
    }

    /// Set the VFS driver.
    #[must_use]
    pub fn with_vfs(mut self, vfs: Arc<dyn VfsDriver>) -> Self {
        self.vfs = Some(vfs);
        self
    }

    /// Get the VFS driver, if available.
    #[must_use]
    pub fn vfs(&self) -> Option<&dyn VfsDriver> {
        self.vfs.as_deref()
    }
}

/// Ex-command execution errors.
#[derive(Debug, Clone)]
pub enum ExCommandError {
    /// No buffer available.
    NoBuffer,
    /// No window available.
    NoWindow,
    /// Invalid arguments.
    InvalidArguments(String),
    /// Execution failed.
    ExecutionFailed(String),
    /// Unknown command.
    UnknownCommand(String),
}

impl std::fmt::Display for ExCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBuffer => write!(f, "no buffer"),
            Self::NoWindow => write!(f, "no window"),
            Self::InvalidArguments(msg) => write!(f, "invalid arguments: {msg}"),
            Self::ExecutionFailed(msg) => write!(f, "execution failed: {msg}"),
            Self::UnknownCommand(name) => write!(f, "unknown command: {name}"),
        }
    }
}

impl std::error::Error for ExCommandError {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === ExCommandRange tests ===

    #[test]
    fn test_range_new() {
        let range = ExCommandRange::new(Position::new(0, 0), Position::new(5, 0));
        assert_eq!(range.start.line, 0);
        assert_eq!(range.end.line, 5);
    }

    #[test]
    fn test_range_with_columns() {
        let range = ExCommandRange::new(Position::new(3, 7), Position::new(10, 15));
        assert_eq!(range.start.line, 3);
        assert_eq!(range.start.column, 7);
        assert_eq!(range.end.line, 10);
        assert_eq!(range.end.column, 15);
    }

    #[test]
    fn test_range_single_line() {
        let range = ExCommandRange::new(Position::new(5, 0), Position::new(5, 10));
        assert_eq!(range.start.line, range.end.line);
    }

    #[test]
    fn test_range_equality() {
        let r1 = ExCommandRange::new(Position::new(1, 0), Position::new(5, 0));
        let r2 = ExCommandRange::new(Position::new(1, 0), Position::new(5, 0));
        let r3 = ExCommandRange::new(Position::new(2, 0), Position::new(5, 0));

        assert_eq!(r1, r2);
        assert_ne!(r1, r3);
    }

    #[test]
    fn test_range_copy_clone() {
        let range = ExCommandRange::new(Position::new(1, 0), Position::new(5, 0));
        let copied = range;
        #[allow(clippy::clone_on_copy)]
        let cloned = range.clone();
        assert_eq!(range, copied);
        assert_eq!(range, cloned);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_range_debug() {
        let range = ExCommandRange::new(Position::new(1, 0), Position::new(5, 0));
        let debug_str = format!("{range:?}");
        assert!(debug_str.contains("ExCommandRange"));
    }

    // === ExCommandError tests ===

    #[test]
    fn test_command_error_display_no_buffer() {
        let err = ExCommandError::NoBuffer;
        assert_eq!(err.to_string(), "no buffer");
    }

    #[test]
    fn test_command_error_display_no_window() {
        let err = ExCommandError::NoWindow;
        assert_eq!(err.to_string(), "no window");
    }

    #[test]
    fn test_command_error_display_invalid_arguments() {
        let err = ExCommandError::InvalidArguments("too many args".to_string());
        let display = err.to_string();
        assert!(display.contains("invalid arguments"));
        assert!(display.contains("too many args"));
    }

    #[test]
    fn test_command_error_display_execution_failed() {
        let err = ExCommandError::ExecutionFailed("disk full".to_string());
        let display = err.to_string();
        assert!(display.contains("execution failed"));
        assert!(display.contains("disk full"));
    }

    #[test]
    fn test_command_error_display_unknown_command() {
        let err = ExCommandError::UnknownCommand("foo".to_string());
        let display = err.to_string();
        assert!(display.contains("unknown command"));
        assert!(display.contains("foo"));
    }

    #[test]
    fn test_command_error_clone() {
        let errors = [
            ExCommandError::NoBuffer,
            ExCommandError::NoWindow,
            ExCommandError::InvalidArguments("args".to_string()),
            ExCommandError::ExecutionFailed("fail".to_string()),
            ExCommandError::UnknownCommand("cmd".to_string()),
        ];

        for err in &errors {
            let cloned = err.clone();
            assert_eq!(err.to_string(), cloned.to_string());
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_error_debug() {
        let err = ExCommandError::NoBuffer;
        let debug_str = format!("{err:?}");
        assert!(debug_str.contains("NoBuffer"));

        let err = ExCommandError::NoWindow;
        let debug_str = format!("{err:?}");
        assert!(debug_str.contains("NoWindow"));
    }

    #[test]
    fn test_command_error_is_std_error() {
        fn accepts_error(_: &dyn std::error::Error) {}
        let err = ExCommandError::NoBuffer;
        // Verify it implements std::error::Error
        accepts_error(&err);
    }

    #[test]
    fn test_command_error_source_is_none() {
        // std::error::Error default source() returns None
        use std::error::Error;
        let err = ExCommandError::NoBuffer;
        assert!(err.source().is_none());
    }

    // === ExCommandHandler trait tests ===

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ex_command_handler_object_safe() {
        // Inner fns verify compilation only, never called
        fn _accepts_ref(_: &dyn ExCommandHandler) {}
        fn _accepts_box(_: Box<dyn ExCommandHandler>) {}
    }

    struct TestExHandler;

    impl ExCommandHandler for TestExHandler {
        fn id(&self) -> &'static str {
            "test-handler"
        }

        fn names(&self) -> &[&'static str] {
            &["test", "t"]
        }

        fn execute(
            &self,
            _ctx: &mut ExCommandContext<'_>,
            _args: &[&str],
        ) -> Result<(), ExCommandError> {
            Ok(())
        }
    }

    #[test]
    fn test_ex_command_handler_default_complete() {
        let handler = TestExHandler;
        let completions = handler.complete("te");
        assert!(completions.is_empty());
    }

    #[test]
    fn test_ex_command_handler_default_help() {
        let handler = TestExHandler;
        assert_eq!(handler.help(), "");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ex_command_handler_custom_help() {
        struct HelpHandler;

        impl ExCommandHandler for HelpHandler {
            fn id(&self) -> &'static str {
                "help-handler"
            }
            fn names(&self) -> &[&'static str] {
                &["help"]
            }
            fn execute(
                &self,
                _ctx: &mut ExCommandContext<'_>,
                _args: &[&str],
            ) -> Result<(), ExCommandError> {
                Ok(())
            }
            fn help(&self) -> &'static str {
                "Show help information"
            }
        }

        let handler = HelpHandler;
        assert_eq!(handler.help(), "Show help information");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ex_command_handler_custom_complete() {
        struct CompleteHandler;

        impl ExCommandHandler for CompleteHandler {
            fn id(&self) -> &'static str {
                "complete-handler"
            }
            fn names(&self) -> &[&'static str] {
                &["edit"]
            }
            fn execute(
                &self,
                _ctx: &mut ExCommandContext<'_>,
                _args: &[&str],
            ) -> Result<(), ExCommandError> {
                Ok(())
            }
            fn complete(&self, partial: &str) -> Vec<String> {
                vec![format!("{partial}.rs"), format!("{partial}.txt")]
            }
        }

        let handler = CompleteHandler;
        let completions = handler.complete("main");
        assert_eq!(completions.len(), 2);
        assert_eq!(completions[0], "main.rs");
        assert_eq!(completions[1], "main.txt");
    }

    // === ExCommandContext tests ===

    #[test]
    fn test_ex_command_context_new() {
        let kernel = KernelContext::default();
        let ctx = ExCommandContext::new(&kernel);
        assert!(ctx.buffer_id.is_none());
        assert!(ctx.window_id.is_none());
        assert!(!ctx.bang);
        assert!(ctx.range.is_none());
        assert!(ctx.vfs.is_none());
    }

    #[test]
    fn test_ex_command_context_with_buffer() {
        let kernel = KernelContext::default();
        let ctx = ExCommandContext::new(&kernel).with_buffer(BufferId::from_raw(5));
        assert_eq!(ctx.buffer_id, Some(BufferId::from_raw(5)));
    }

    #[test]
    fn test_ex_command_context_with_window() {
        let kernel = KernelContext::default();
        let ctx = ExCommandContext::new(&kernel).with_window(WindowId::from_raw(3));
        assert_eq!(ctx.window_id, Some(WindowId::from_raw(3)));
    }

    #[test]
    fn test_ex_command_context_with_bang() {
        let kernel = KernelContext::default();
        let ctx = ExCommandContext::new(&kernel).with_bang(true);
        assert!(ctx.bang);
    }

    #[test]
    fn test_ex_command_context_with_bang_false() {
        let kernel = KernelContext::default();
        let ctx = ExCommandContext::new(&kernel).with_bang(false);
        assert!(!ctx.bang);
    }

    #[test]
    fn test_ex_command_context_with_range() {
        let kernel = KernelContext::default();
        let range = ExCommandRange::new(Position::new(1, 0), Position::new(5, 0));
        let ctx = ExCommandContext::new(&kernel).with_range(range);
        assert!(ctx.range.is_some());
        let r = ctx.range.unwrap();
        assert_eq!(r.start.line, 1);
        assert_eq!(r.end.line, 5);
    }

    #[test]
    fn test_ex_command_context_builder_chain() {
        let kernel = KernelContext::default();
        let range = ExCommandRange::new(Position::new(0, 0), Position::new(10, 0));
        let ctx = ExCommandContext::new(&kernel)
            .with_buffer(BufferId::from_raw(1))
            .with_window(WindowId::from_raw(2))
            .with_bang(true)
            .with_range(range);

        assert_eq!(ctx.buffer_id, Some(BufferId::from_raw(1)));
        assert_eq!(ctx.window_id, Some(WindowId::from_raw(2)));
        assert!(ctx.bang);
        assert!(ctx.range.is_some());
    }

    #[test]
    fn test_ex_command_context_vfs_none_by_default() {
        let kernel = KernelContext::default();
        let ctx = ExCommandContext::new(&kernel);
        assert!(ctx.vfs().is_none());
    }

    #[test]
    fn test_ex_command_context_with_vfs() {
        use reovim_driver_vfs::MockVfs;

        let kernel = KernelContext::default();
        let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
        let ctx = ExCommandContext::new(&kernel).with_vfs(vfs);
        assert!(ctx.vfs().is_some());
        assert!(ctx.vfs.is_some());
    }

    #[test]
    fn test_ex_command_context_kernel_access() {
        let kernel = KernelContext::default();
        let ctx = ExCommandContext::new(&kernel);
        // Just verify we can access the kernel reference
        let _kernel_ref = ctx.kernel;
    }

    // ========================================================================
    // ExCommandHandler trait - default methods and execute
    // ========================================================================

    #[test]
    fn test_ex_command_handler_id_and_names() {
        let handler = TestExHandler;
        assert_eq!(handler.id(), "test-handler");
        assert_eq!(handler.names(), &["test", "t"]);
    }

    #[test]
    fn test_ex_command_handler_execute_success() {
        let handler = TestExHandler;
        let kernel = KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);
        assert!(handler.execute(&mut ctx, &[]).is_ok());
    }

    #[test]
    fn test_ex_command_handler_execute_with_args() {
        let handler = TestExHandler;
        let kernel = KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);
        assert!(handler.execute(&mut ctx, &["arg1", "arg2"]).is_ok());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ex_command_handler_failing_execute() {
        struct FailHandler;
        impl ExCommandHandler for FailHandler {
            fn id(&self) -> &'static str {
                "fail"
            }
            fn names(&self) -> &[&'static str] {
                &["fail"]
            }
            fn execute(
                &self,
                _ctx: &mut ExCommandContext<'_>,
                _args: &[&str],
            ) -> Result<(), ExCommandError> {
                Err(ExCommandError::ExecutionFailed("intentional".to_string()))
            }
        }

        let handler = FailHandler;
        let kernel = KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);
        let result = handler.execute(&mut ctx, &[]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("intentional"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ex_command_handler_no_buffer_error() {
        struct NeedBufferHandler;
        impl ExCommandHandler for NeedBufferHandler {
            fn id(&self) -> &'static str {
                "need-buffer"
            }
            fn names(&self) -> &[&'static str] {
                &["nb"]
            }
            fn execute(
                &self,
                ctx: &mut ExCommandContext<'_>,
                _args: &[&str],
            ) -> Result<(), ExCommandError> {
                ctx.buffer_id.ok_or(ExCommandError::NoBuffer)?;
                Ok(())
            }
        }

        let handler = NeedBufferHandler;
        let kernel = KernelContext::default();

        // Without buffer -> error
        let mut ctx = ExCommandContext::new(&kernel);
        let result = handler.execute(&mut ctx, &[]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "no buffer");

        // With buffer -> success
        let mut ctx = ExCommandContext::new(&kernel).with_buffer(BufferId::from_raw(1));
        assert!(handler.execute(&mut ctx, &[]).is_ok());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ex_command_handler_no_window_error() {
        struct NeedWindowHandler;
        impl ExCommandHandler for NeedWindowHandler {
            fn id(&self) -> &'static str {
                "need-window"
            }
            fn names(&self) -> &[&'static str] {
                &["nw"]
            }
            fn execute(
                &self,
                ctx: &mut ExCommandContext<'_>,
                _args: &[&str],
            ) -> Result<(), ExCommandError> {
                ctx.window_id.ok_or(ExCommandError::NoWindow)?;
                Ok(())
            }
        }

        let handler = NeedWindowHandler;
        let kernel = KernelContext::default();

        let mut ctx = ExCommandContext::new(&kernel);
        let result = handler.execute(&mut ctx, &[]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "no window");

        let mut ctx = ExCommandContext::new(&kernel).with_window(WindowId::from_raw(1));
        assert!(handler.execute(&mut ctx, &[]).is_ok());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ex_command_handler_invalid_arguments_error() {
        struct ValidatingHandler;
        impl ExCommandHandler for ValidatingHandler {
            fn id(&self) -> &'static str {
                "validate"
            }
            fn names(&self) -> &[&'static str] {
                &["val"]
            }
            fn execute(
                &self,
                _ctx: &mut ExCommandContext<'_>,
                args: &[&str],
            ) -> Result<(), ExCommandError> {
                if args.is_empty() {
                    return Err(ExCommandError::InvalidArguments(
                        "at least one argument required".to_string(),
                    ));
                }
                Ok(())
            }
        }

        let handler = ValidatingHandler;
        let kernel = KernelContext::default();

        let mut ctx = ExCommandContext::new(&kernel);
        let result = handler.execute(&mut ctx, &[]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("invalid arguments"));

        let mut ctx = ExCommandContext::new(&kernel);
        assert!(handler.execute(&mut ctx, &["arg"]).is_ok());
    }

    #[test]
    fn test_ex_command_handler_unknown_command_error() {
        let err = ExCommandError::UnknownCommand("foobar".to_string());
        assert!(err.to_string().contains("unknown command"));
        assert!(err.to_string().contains("foobar"));
    }

    // ========================================================================
    // ExCommandHandler default complete and help with custom complete
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ex_command_handler_custom_complete_with_context() {
        struct ContextCompleteHandler;
        impl ExCommandHandler for ContextCompleteHandler {
            fn id(&self) -> &'static str {
                "context-complete"
            }
            fn names(&self) -> &[&'static str] {
                &["cc"]
            }
            fn execute(
                &self,
                _ctx: &mut ExCommandContext<'_>,
                _args: &[&str],
            ) -> Result<(), ExCommandError> {
                Ok(())
            }
            fn complete(&self, partial: &str) -> Vec<String> {
                if partial.is_empty() {
                    vec!["all".to_string(), "none".to_string()]
                } else {
                    vec![format!("{partial}-complete")]
                }
            }
            fn help(&self) -> &'static str {
                "Context-aware completion handler"
            }
        }

        let handler = ContextCompleteHandler;
        let empty = handler.complete("");
        assert_eq!(empty.len(), 2);

        let with_prefix = handler.complete("foo");
        assert_eq!(with_prefix.len(), 1);
        assert_eq!(with_prefix[0], "foo-complete");

        assert_eq!(handler.help(), "Context-aware completion handler");
    }

    // ========================================================================
    // ExCommandContext - full builder chain with all fields
    // ========================================================================

    #[test]
    fn test_ex_command_context_full_builder_chain() {
        let kernel = KernelContext::default();
        let range = ExCommandRange::new(Position::new(0, 0), Position::new(10, 0));
        let ctx = ExCommandContext::new(&kernel)
            .with_buffer(BufferId::from_raw(1))
            .with_window(WindowId::from_raw(2))
            .with_bang(true)
            .with_range(range);

        assert_eq!(ctx.buffer_id, Some(BufferId::from_raw(1)));
        assert_eq!(ctx.window_id, Some(WindowId::from_raw(2)));
        assert!(ctx.bang);
        assert!(ctx.range.is_some());
        let r = ctx.range.unwrap();
        assert_eq!(r.start, Position::new(0, 0));
        assert_eq!(r.end, Position::new(10, 0));
    }

    #[test]
    fn test_ex_command_context_with_vfs_provides_access() {
        use reovim_driver_vfs::MockVfs;

        let kernel = KernelContext::default();
        let mock_vfs = MockVfs::new();
        let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> = Arc::new(mock_vfs);
        let ctx = ExCommandContext::new(&kernel).with_vfs(vfs);

        assert!(ctx.vfs().is_some());
        assert!(ctx.vfs.is_some());
    }

    #[test]
    fn test_ex_command_context_vfs_deref() {
        use reovim_driver_vfs::MockVfs;

        let kernel = KernelContext::default();
        let mock_vfs = MockVfs::new();
        let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> = Arc::new(mock_vfs);
        let ctx = ExCommandContext::new(&kernel).with_vfs(vfs);

        // Verify vfs() returns a dyn VfsDriver reference
        let _vfs_ref = ctx.vfs().unwrap();
    }

    // ========================================================================
    // ExCommandError additional coverage
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_error_debug_invalid_arguments() {
        let err = ExCommandError::InvalidArguments("bad arg".to_string());
        let debug = format!("{err:?}");
        assert!(debug.contains("InvalidArguments"));
        assert!(debug.contains("bad arg"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_error_debug_execution_failed() {
        let err = ExCommandError::ExecutionFailed("crash".to_string());
        let debug = format!("{err:?}");
        assert!(debug.contains("ExecutionFailed"));
        assert!(debug.contains("crash"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_error_debug_unknown_command() {
        let err = ExCommandError::UnknownCommand("xyz".to_string());
        let debug = format!("{err:?}");
        assert!(debug.contains("UnknownCommand"));
        assert!(debug.contains("xyz"));
    }

    #[test]
    fn test_command_error_clone_and_display_all_variants() {
        let variants = vec![
            ExCommandError::NoBuffer,
            ExCommandError::NoWindow,
            ExCommandError::InvalidArguments("ia".to_string()),
            ExCommandError::ExecutionFailed("ef".to_string()),
            ExCommandError::UnknownCommand("uc".to_string()),
        ];

        for err in &variants {
            let cloned = err.clone();
            assert_eq!(err.to_string(), cloned.to_string());
        }
    }

    #[test]
    fn test_command_error_source_is_none_all_variants() {
        use std::error::Error;

        let variants: Vec<ExCommandError> = vec![
            ExCommandError::NoBuffer,
            ExCommandError::NoWindow,
            ExCommandError::InvalidArguments("ia".to_string()),
            ExCommandError::ExecutionFailed("ef".to_string()),
            ExCommandError::UnknownCommand("uc".to_string()),
        ];

        for err in &variants {
            assert!(err.source().is_none());
        }
    }

    // ========================================================================
    // ExCommandRange additional tests
    // ========================================================================

    #[test]
    fn test_range_zero_length() {
        let range = ExCommandRange::new(Position::new(3, 5), Position::new(3, 5));
        assert_eq!(range.start, range.end);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_range_debug_contains_positions() {
        let range = ExCommandRange::new(Position::new(1, 2), Position::new(3, 4));
        let debug = format!("{range:?}");
        assert!(debug.contains("ExCommandRange"));
        assert!(debug.contains("start"));
        assert!(debug.contains("end"));
    }

    // ========================================================================
    // ExCommandHandler as trait object
    // ========================================================================

    #[test]
    fn test_ex_command_handler_as_boxed_trait_object() {
        let handler: Box<dyn ExCommandHandler> = Box::new(TestExHandler);
        assert_eq!(handler.id(), "test-handler");
        assert_eq!(handler.names(), &["test", "t"]);
        assert!(handler.complete("").is_empty());
        assert_eq!(handler.help(), "");
    }

    #[test]
    fn test_ex_command_handler_as_arc_trait_object() {
        let handler: Arc<dyn ExCommandHandler> = Arc::new(TestExHandler);
        assert_eq!(handler.id(), "test-handler");
        assert_eq!(handler.names(), &["test", "t"]);
    }

    // ========================================================================
    // ExCommandContext - execute with bang flag in context
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ex_command_context_bang_accessible_in_handler() {
        struct BangHandler;
        impl ExCommandHandler for BangHandler {
            fn id(&self) -> &'static str {
                "bang"
            }
            fn names(&self) -> &[&'static str] {
                &["bang"]
            }
            fn execute(
                &self,
                ctx: &mut ExCommandContext<'_>,
                _args: &[&str],
            ) -> Result<(), ExCommandError> {
                if ctx.bang {
                    Ok(())
                } else {
                    Err(ExCommandError::ExecutionFailed("need bang".to_string()))
                }
            }
        }

        let handler = BangHandler;
        let kernel = KernelContext::default();

        let mut ctx = ExCommandContext::new(&kernel).with_bang(false);
        assert!(handler.execute(&mut ctx, &[]).is_err());

        let mut ctx = ExCommandContext::new(&kernel).with_bang(true);
        assert!(handler.execute(&mut ctx, &[]).is_ok());
    }

    // ========================================================================
    // ExCommandContext - range accessible in handler
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ex_command_context_range_accessible_in_handler() {
        struct RangeHandler;
        impl ExCommandHandler for RangeHandler {
            fn id(&self) -> &'static str {
                "range"
            }
            fn names(&self) -> &[&'static str] {
                &["range"]
            }
            fn execute(
                &self,
                ctx: &mut ExCommandContext<'_>,
                _args: &[&str],
            ) -> Result<(), ExCommandError> {
                if ctx.range.is_some() {
                    Ok(())
                } else {
                    Err(ExCommandError::InvalidArguments("no range".to_string()))
                }
            }
        }

        let handler = RangeHandler;
        let kernel = KernelContext::default();

        let mut ctx = ExCommandContext::new(&kernel);
        assert!(handler.execute(&mut ctx, &[]).is_err());

        let range = ExCommandRange::new(Position::new(1, 0), Position::new(5, 0));
        let mut ctx = ExCommandContext::new(&kernel).with_range(range);
        assert!(handler.execute(&mut ctx, &[]).is_ok());
    }
}
