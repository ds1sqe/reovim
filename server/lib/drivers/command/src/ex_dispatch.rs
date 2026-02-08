//! Ex-command dispatcher and registry for command-line execution.
//!
//! This module provides the mechanism for ex-command dispatch:
//! - `ExCommandDispatcher` trait defines the interface
//! - `ExCommandRegistry` provides the implementation
//!
//! # Architecture
//!
//! - **Mechanism (this module)**: Registry, dispatch logic
//! - **Policy (modules)**: Actual command implementations
//!
//! # Usage
//!
//! ```ignore
//! use reovim_driver_command::{ExCommandRegistry, ExCommandResult, ExDispatchContext};
//!
//! fn execute_ex(runtime: &mut SessionRuntime<'_>, cmdline: &str) {
//!     if let Some(registry) = runtime.kernel().services.get::<ExCommandRegistry>() {
//!         let ctx = ExDispatchContext::new(args.buffer_id(), None);
//!         let result = registry.dispatch(cmdline, runtime.kernel(), &ctx);
//!         // Handle result...
//!     }
//! }
//! ```

use std::{collections::HashMap, sync::Arc};

use {
    reovim_driver_vfs::VfsInstance,
    reovim_kernel::api::v1::{BufferId, KernelContext, Service, WindowId},
};

use crate::ex_handler::{ExCommandContext, ExCommandHandler};

/// Result of ex-command execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExCommandResult {
    /// Command executed successfully.
    Success,
    /// Command not found.
    NotFound(String),
    /// Command execution error.
    Error(String),
}

/// Context for ex-command execution.
///
/// Provides all information needed to execute an ex-command.
#[derive(Debug, Clone)]
pub struct ExDispatchContext {
    /// Current buffer (if any).
    pub buffer_id: Option<BufferId>,
    /// Current window (if any).
    pub window_id: Option<WindowId>,
    /// Whether command was invoked with ! (e.g., :q!).
    pub bang: bool,
}

impl ExDispatchContext {
    /// Create a new context with buffer and window IDs.
    #[must_use]
    pub const fn new(buffer_id: Option<BufferId>, window_id: Option<WindowId>) -> Self {
        Self {
            buffer_id,
            window_id,
            bang: false,
        }
    }

    /// Set the bang flag.
    #[must_use]
    pub const fn with_bang(mut self, bang: bool) -> Self {
        self.bang = bang;
        self
    }
}

impl Default for ExDispatchContext {
    fn default() -> Self {
        Self::new(None, None)
    }
}

/// Dispatcher for ex-commands (`:w`, `:q`, `:e`, etc.).
///
/// This trait is the mechanism for ex-command execution.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`) since they
/// are stored in `ServiceRegistry`.
pub trait ExCommandDispatcher: Service + Send + Sync {
    /// Dispatch an ex-command.
    ///
    /// Parses the command line and executes the appropriate command.
    ///
    /// # Arguments
    ///
    /// * `cmdline` - The command line content (e.g., "w filename.txt", "q!")
    /// * `kernel` - Kernel context for buffer/window access
    /// * `ctx` - Execution context with buffer/window IDs
    ///
    /// # Returns
    ///
    /// - `ExCommandResult::Success` if command executed successfully
    /// - `ExCommandResult::NotFound` if command name not recognized
    /// - `ExCommandResult::Error` if command execution failed
    fn dispatch(
        &self,
        cmdline: &str,
        kernel: &KernelContext,
        ctx: &ExDispatchContext,
    ) -> ExCommandResult;

    /// Check if a command exists by name.
    ///
    /// Used for validation and completion.
    fn has_command(&self, name: &str) -> bool;

    /// List all available ex-command names.
    ///
    /// Used for completion and help.
    fn list_commands(&self) -> Vec<&str>;
}

// ============================================================================
// ExCommandRegistry - Concrete implementation
// ============================================================================

/// Registry of ex-command handlers.
///
/// Stores handlers by name for lookup during dispatch.
/// Implements `ExCommandDispatcher` to provide dispatch functionality.
///
/// # Usage
///
/// ```ignore
/// // In server bootstrap:
/// let handlers = ex_handler_store.take_handlers();
/// let registry = ExCommandRegistry::from_handlers(handlers);
/// services.register(Arc::new(registry));
///
/// // In vim module:
/// if let Some(registry) = services.get::<ExCommandRegistry>() {
///     registry.dispatch("w filename.txt", kernel, &ctx);
/// }
/// ```
pub struct ExCommandRegistry {
    /// Handlers indexed by name for fast lookup.
    handlers_by_name: HashMap<String, Arc<dyn ExCommandHandler>>,
    /// All handler names for `list_commands()`.
    all_names: Vec<&'static str>,
}

impl ExCommandRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers_by_name: HashMap::new(),
            all_names: Vec::new(),
        }
    }

    /// Create a registry from a list of handlers.
    ///
    /// Each handler is indexed by all its names.
    #[must_use]
    pub fn from_handlers(handlers: Vec<Arc<dyn ExCommandHandler>>) -> Self {
        let mut registry = Self::new();

        for handler in handlers {
            for &name in handler.names() {
                registry.all_names.push(name);
                registry
                    .handlers_by_name
                    .insert(name.to_string(), Arc::clone(&handler));
            }
        }

        registry
    }

    /// Get a handler by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Arc<dyn ExCommandHandler>> {
        self.handlers_by_name.get(name)
    }

    /// Get the number of registered handlers.
    #[must_use]
    pub fn len(&self) -> usize {
        // Count unique handlers (not aliases)
        let unique: std::collections::HashSet<_> =
            self.handlers_by_name.values().map(|h| h.id()).collect();
        unique.len()
    }

    /// Check if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handlers_by_name.is_empty()
    }
}

impl Default for ExCommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Implement Service so ExCommandRegistry can be stored in ServiceRegistry
impl Service for ExCommandRegistry {}

impl ExCommandDispatcher for ExCommandRegistry {
    fn dispatch(
        &self,
        cmdline: &str,
        kernel: &KernelContext,
        ctx: &ExDispatchContext,
    ) -> ExCommandResult {
        // Parse cmdline: handle bang (!) and split command name from args
        let cmdline = cmdline.trim();
        if cmdline.is_empty() {
            return ExCommandResult::Success;
        }

        // Check for bang at the end of command name
        let (cmd_part, args_part) =
            cmdline
                .find(|c: char| c.is_whitespace())
                .map_or((cmdline, ""), |space_idx| {
                    let cmd = &cmdline[..space_idx];
                    let args = cmdline[space_idx..].trim();
                    (cmd, args)
                });

        // Extract bang if present (e.g., "q!" -> ("q", true))
        let (cmd_name, bang) = cmd_part
            .strip_suffix('!')
            .map_or((cmd_part, false), |stripped| (stripped, true));

        // Lookup handler by command name
        let Some(handler) = self.get(cmd_name) else {
            return ExCommandResult::NotFound(cmd_name.to_string());
        };

        // Parse args into a vector (simple whitespace split for now)
        let args: Vec<&str> = if args_part.is_empty() {
            vec![]
        } else {
            args_part.split_whitespace().collect()
        };

        // Build execution context
        let mut ex_ctx = ExCommandContext::new(kernel);
        if let Some(buffer_id) = ctx.buffer_id {
            ex_ctx = ex_ctx.with_buffer(buffer_id);
        }
        if let Some(window_id) = ctx.window_id {
            ex_ctx = ex_ctx.with_window(window_id);
        }
        ex_ctx = ex_ctx.with_bang(bang || ctx.bang);

        // Look up VFS from ServiceRegistry and attach to context
        if let Some(vfs_instance) = kernel.services.get::<VfsInstance>() {
            ex_ctx = ex_ctx.with_vfs(Arc::clone(vfs_instance.driver()));
        }

        // Execute command
        match handler.execute(&mut ex_ctx, &args) {
            Ok(()) => ExCommandResult::Success,
            Err(e) => ExCommandResult::Error(e.to_string()),
        }
    }

    fn has_command(&self, name: &str) -> bool {
        self.handlers_by_name.contains_key(name)
    }

    fn list_commands(&self) -> Vec<&str> {
        self.all_names.clone()
    }
}

impl std::fmt::Debug for ExCommandRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExCommandRegistry")
            .field("handler_count", &self.len())
            .field("commands", &self.all_names)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use crate::ex_handler::ExCommandError;

    use super::*;

    // === ExDispatchContext tests ===

    #[test]
    fn test_ex_dispatch_context_default() {
        let ctx = ExDispatchContext::default();
        assert!(ctx.buffer_id.is_none());
        assert!(ctx.window_id.is_none());
        assert!(!ctx.bang);
    }

    #[test]
    fn test_ex_dispatch_context_new() {
        let ctx = ExDispatchContext::new(None, None);
        assert!(ctx.buffer_id.is_none());
        assert!(ctx.window_id.is_none());
        assert!(!ctx.bang);
    }

    #[test]
    fn test_ex_dispatch_context_new_with_buffer() {
        let ctx = ExDispatchContext::new(Some(BufferId::from_raw(5)), None);
        assert_eq!(ctx.buffer_id, Some(BufferId::from_raw(5)));
        assert!(ctx.window_id.is_none());
    }

    #[test]
    fn test_ex_dispatch_context_new_with_window() {
        let ctx = ExDispatchContext::new(None, Some(WindowId::from_raw(3)));
        assert!(ctx.buffer_id.is_none());
        assert_eq!(ctx.window_id, Some(WindowId::from_raw(3)));
    }

    #[test]
    fn test_ex_dispatch_context_new_with_both() {
        let ctx = ExDispatchContext::new(Some(BufferId::from_raw(1)), Some(WindowId::from_raw(2)));
        assert_eq!(ctx.buffer_id, Some(BufferId::from_raw(1)));
        assert_eq!(ctx.window_id, Some(WindowId::from_raw(2)));
    }

    #[test]
    fn test_ex_dispatch_context_with_bang() {
        let ctx = ExDispatchContext::default().with_bang(true);
        assert!(ctx.bang);
    }

    #[test]
    fn test_ex_dispatch_context_with_bang_false() {
        let ctx = ExDispatchContext::default().with_bang(false);
        assert!(!ctx.bang);
    }

    #[test]
    fn test_ex_dispatch_context_clone() {
        let ctx = ExDispatchContext::new(Some(BufferId::from_raw(1)), Some(WindowId::from_raw(2)))
            .with_bang(true);
        #[allow(clippy::redundant_clone)]
        let cloned = ctx.clone();
        assert_eq!(cloned.buffer_id, Some(BufferId::from_raw(1)));
        assert_eq!(cloned.window_id, Some(WindowId::from_raw(2)));
        assert!(cloned.bang);
    }

    #[test]
    fn test_ex_dispatch_context_debug() {
        let ctx = ExDispatchContext::default();
        let debug_str = format!("{ctx:?}");
        assert!(debug_str.contains("ExDispatchContext"));
    }

    // === ExCommandResult tests ===

    #[test]
    fn test_ex_command_result_variants() {
        let success = ExCommandResult::Success;
        let not_found = ExCommandResult::NotFound("foo".to_string());
        let error = ExCommandResult::Error("failed".to_string());

        assert_eq!(success, ExCommandResult::Success);
        assert!(matches!(not_found, ExCommandResult::NotFound(_)));
        assert!(matches!(error, ExCommandResult::Error(_)));
    }

    #[test]
    fn test_ex_command_result_equality() {
        assert_eq!(ExCommandResult::Success, ExCommandResult::Success);
        assert_eq!(
            ExCommandResult::NotFound("x".to_string()),
            ExCommandResult::NotFound("x".to_string())
        );
        assert_eq!(
            ExCommandResult::Error("y".to_string()),
            ExCommandResult::Error("y".to_string())
        );

        assert_ne!(ExCommandResult::Success, ExCommandResult::Error("e".to_string()));
        assert_ne!(
            ExCommandResult::NotFound("a".to_string()),
            ExCommandResult::NotFound("b".to_string())
        );
    }

    #[test]
    fn test_ex_command_result_clone() {
        let results = [
            ExCommandResult::Success,
            ExCommandResult::NotFound("cmd".to_string()),
            ExCommandResult::Error("err".to_string()),
        ];
        for result in &results {
            let cloned = result.clone();
            assert_eq!(result, &cloned);
        }
    }

    #[test]
    fn test_ex_command_result_debug() {
        let debug_str = format!("{:?}", ExCommandResult::Success);
        assert_eq!(debug_str, "Success");

        let debug_str = format!("{:?}", ExCommandResult::NotFound("foo".to_string()));
        assert!(debug_str.contains("NotFound"));
        assert!(debug_str.contains("foo"));

        let debug_str = format!("{:?}", ExCommandResult::Error("bar".to_string()));
        assert!(debug_str.contains("Error"));
        assert!(debug_str.contains("bar"));
    }

    #[test]
    fn test_ex_command_dispatcher_object_safe() {
        fn _accepts_ref(_: &dyn ExCommandDispatcher) {}
        fn _accepts_box(_: Box<dyn ExCommandDispatcher>) {}
    }

    // === Test helpers ===

    struct TestExCommand {
        id: &'static str,
        names: &'static [&'static str],
        execute_fn: fn(&mut ExCommandContext<'_>, &[&str]) -> Result<(), ExCommandError>,
    }

    impl ExCommandHandler for TestExCommand {
        fn id(&self) -> &'static str {
            self.id
        }

        fn names(&self) -> &[&'static str] {
            self.names
        }

        fn execute(
            &self,
            ctx: &mut ExCommandContext<'_>,
            args: &[&str],
        ) -> Result<(), ExCommandError> {
            (self.execute_fn)(ctx, args)
        }
    }

    #[allow(clippy::unnecessary_wraps)] // Matches ExCommandHandler::execute signature
    fn success_cmd(_ctx: &mut ExCommandContext<'_>, _args: &[&str]) -> Result<(), ExCommandError> {
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)] // Matches ExCommandHandler::execute signature
    fn error_cmd(_ctx: &mut ExCommandContext<'_>, _args: &[&str]) -> Result<(), ExCommandError> {
        Err(ExCommandError::ExecutionFailed("test error".to_string()))
    }

    // === ExCommandRegistry tests ===

    #[test]
    fn test_registry_new() {
        let registry = ExCommandRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_default() {
        let registry = ExCommandRegistry::default();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_from_handlers() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![
            Arc::new(TestExCommand {
                id: "write",
                names: &["w", "write"],
                execute_fn: success_cmd,
            }),
            Arc::new(TestExCommand {
                id: "quit",
                names: &["q", "quit"],
                execute_fn: success_cmd,
            }),
        ];

        let registry = ExCommandRegistry::from_handlers(handlers);
        assert_eq!(registry.len(), 2);
        assert!(registry.has_command("w"));
        assert!(registry.has_command("write"));
        assert!(registry.has_command("q"));
        assert!(registry.has_command("quit"));
        assert!(!registry.has_command("foo"));
    }

    #[test]
    fn test_registry_from_handlers_empty() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![];
        let registry = ExCommandRegistry::from_handlers(handlers);
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_get() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "write",
            names: &["w", "write"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        assert!(registry.get("w").is_some());
        assert!(registry.get("write").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_get_handler_metadata() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "write",
            names: &["w", "write"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        let handler = registry.get("w").unwrap();
        assert_eq!(handler.id(), "write");
        assert_eq!(handler.names(), &["w", "write"]);
    }

    #[test]
    fn test_registry_len_counts_unique_handlers() {
        // Two names pointing to one handler should count as 1
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "write",
            names: &["w", "write"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        assert_eq!(registry.len(), 1); // One unique handler despite two names
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_registry_is_empty_false_after_add() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "test",
            names: &["t"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_registry_debug() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "write",
            names: &["w", "write"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        let debug_str = format!("{registry:?}");
        assert!(debug_str.contains("ExCommandRegistry"));
        assert!(debug_str.contains("handler_count"));
    }

    #[test]
    fn test_registry_debug_empty() {
        let registry = ExCommandRegistry::new();
        let debug_str = format!("{registry:?}");
        assert!(debug_str.contains("ExCommandRegistry"));
    }

    #[test]
    fn test_registry_service_impl() {
        fn accepts_service(_: &dyn Service) {}
        let registry = ExCommandRegistry::new();
        accepts_service(&registry);
    }

    // === Dispatch tests ===

    #[test]
    fn test_dispatch_success() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "test",
            names: &["t", "test"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        let kernel = KernelContext::default();
        let ctx = ExDispatchContext::default();

        let result = registry.dispatch("t", &kernel, &ctx);
        assert_eq!(result, ExCommandResult::Success);

        let result = registry.dispatch("test arg1 arg2", &kernel, &ctx);
        assert_eq!(result, ExCommandResult::Success);
    }

    #[test]
    fn test_dispatch_not_found() {
        let registry = ExCommandRegistry::new();
        let kernel = KernelContext::default();
        let ctx = ExDispatchContext::default();

        let result = registry.dispatch("unknown", &kernel, &ctx);
        assert_eq!(result, ExCommandResult::NotFound("unknown".to_string()));
    }

    #[test]
    fn test_dispatch_not_found_contains_name() {
        let registry = ExCommandRegistry::new();
        let kernel = KernelContext::default();
        let ctx = ExDispatchContext::default();

        let result = registry.dispatch("foobar", &kernel, &ctx);
        if let ExCommandResult::NotFound(name) = result {
            assert_eq!(name, "foobar");
        } else {
            panic!("Expected NotFound");
        }
    }

    #[test]
    fn test_dispatch_error() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "fail",
            names: &["fail"],
            execute_fn: error_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        let kernel = KernelContext::default();
        let ctx = ExDispatchContext::default();

        let result = registry.dispatch("fail", &kernel, &ctx);
        if let ExCommandResult::Error(msg) = result {
            assert!(msg.contains("test error"));
        } else {
            panic!("Expected Error");
        }
    }

    #[test]
    fn test_dispatch_with_bang() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "quit",
            names: &["q"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        let kernel = KernelContext::default();
        let ctx = ExDispatchContext::default();

        // q! should be parsed as q with bang=true
        let result = registry.dispatch("q!", &kernel, &ctx);
        assert_eq!(result, ExCommandResult::Success);
    }

    #[test]
    fn test_dispatch_with_context_bang() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "quit",
            names: &["q"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        let kernel = KernelContext::default();
        let ctx = ExDispatchContext::default().with_bang(true);

        let result = registry.dispatch("q", &kernel, &ctx);
        assert_eq!(result, ExCommandResult::Success);
    }

    #[test]
    fn test_dispatch_empty_cmdline() {
        let registry = ExCommandRegistry::new();
        let kernel = KernelContext::default();
        let ctx = ExDispatchContext::default();

        let result = registry.dispatch("", &kernel, &ctx);
        assert_eq!(result, ExCommandResult::Success);

        let result = registry.dispatch("   ", &kernel, &ctx);
        assert_eq!(result, ExCommandResult::Success);
    }

    #[test]
    fn test_dispatch_with_leading_trailing_whitespace() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "test",
            names: &["t"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        let kernel = KernelContext::default();
        let ctx = ExDispatchContext::default();

        let result = registry.dispatch("  t  ", &kernel, &ctx);
        assert_eq!(result, ExCommandResult::Success);
    }

    #[test]
    fn test_dispatch_with_args() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "write",
            names: &["w"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        let kernel = KernelContext::default();
        let ctx = ExDispatchContext::default();

        let result = registry.dispatch("w filename.txt", &kernel, &ctx);
        assert_eq!(result, ExCommandResult::Success);
    }

    #[test]
    fn test_dispatch_with_multiple_args() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "test",
            names: &["t"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        let kernel = KernelContext::default();
        let ctx = ExDispatchContext::default();

        let result = registry.dispatch("t arg1 arg2 arg3", &kernel, &ctx);
        assert_eq!(result, ExCommandResult::Success);
    }

    #[test]
    fn test_dispatch_with_buffer_context() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "test",
            names: &["t"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        let kernel = KernelContext::default();
        let ctx = ExDispatchContext::new(Some(BufferId::from_raw(5)), None);

        let result = registry.dispatch("t", &kernel, &ctx);
        assert_eq!(result, ExCommandResult::Success);
    }

    #[test]
    fn test_dispatch_with_window_context() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "test",
            names: &["t"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        let kernel = KernelContext::default();
        let ctx = ExDispatchContext::new(None, Some(WindowId::from_raw(3)));

        let result = registry.dispatch("t", &kernel, &ctx);
        assert_eq!(result, ExCommandResult::Success);
    }

    #[test]
    fn test_dispatch_bang_not_found_strips_bang() {
        let registry = ExCommandRegistry::new();
        let kernel = KernelContext::default();
        let ctx = ExDispatchContext::default();

        // "unknown!" should strip the bang and report "unknown" as not found
        let result = registry.dispatch("unknown!", &kernel, &ctx);
        assert_eq!(result, ExCommandResult::NotFound("unknown".to_string()));
    }

    // === list_commands tests ===

    #[test]
    fn test_list_commands() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "write",
            names: &["w", "write"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        let commands = registry.list_commands();
        assert!(commands.contains(&"w"));
        assert!(commands.contains(&"write"));
    }

    #[test]
    fn test_list_commands_empty() {
        let registry = ExCommandRegistry::new();
        let commands = registry.list_commands();
        assert!(commands.is_empty());
    }

    #[test]
    fn test_list_commands_multiple_handlers() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![
            Arc::new(TestExCommand {
                id: "write",
                names: &["w", "write"],
                execute_fn: success_cmd,
            }),
            Arc::new(TestExCommand {
                id: "quit",
                names: &["q"],
                execute_fn: success_cmd,
            }),
        ];

        let registry = ExCommandRegistry::from_handlers(handlers);
        let commands = registry.list_commands();
        assert_eq!(commands.len(), 3);
        assert!(commands.contains(&"w"));
        assert!(commands.contains(&"write"));
        assert!(commands.contains(&"q"));
    }

    // === has_command tests ===

    #[test]
    fn test_has_command() {
        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "write",
            names: &["w", "write"],
            execute_fn: success_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);
        assert!(registry.has_command("w"));
        assert!(registry.has_command("write"));
        assert!(!registry.has_command("x"));
        assert!(!registry.has_command(""));
    }

    #[test]
    fn test_dispatch_with_vfs_in_service_registry() {
        use reovim_driver_vfs::{MockVfs, VfsInstance};

        // Create a command that verifies VFS is available in the context
        fn vfs_check_cmd(
            ctx: &mut ExCommandContext<'_>,
            _args: &[&str],
        ) -> Result<(), ExCommandError> {
            if ctx.vfs().is_some() {
                Ok(())
            } else {
                Err(ExCommandError::ExecutionFailed("VFS not available".to_string()))
            }
        }

        let handlers: Vec<Arc<dyn ExCommandHandler>> = vec![Arc::new(TestExCommand {
            id: "check-vfs",
            names: &["vfscheck"],
            execute_fn: vfs_check_cmd,
        })];

        let registry = ExCommandRegistry::from_handlers(handlers);

        // Create kernel with VFS registered in ServiceRegistry
        let kernel = KernelContext::default();
        let mock_vfs = MockVfs::new();
        let vfs: Arc<dyn reovim_driver_vfs::VfsDriver> = Arc::new(mock_vfs);
        let vfs_instance = Arc::new(VfsInstance::new(vfs));
        kernel.services.register(vfs_instance);

        let ctx = ExDispatchContext::default();
        let result = registry.dispatch("vfscheck", &kernel, &ctx);
        assert_eq!(result, ExCommandResult::Success);
    }
}
