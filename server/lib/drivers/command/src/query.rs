//! Command query service for command discovery and completion.
//!
//! Provides a query interface for modules to discover commands without
//! depending on runner internals. Used for cmdline tab-completion,
//! help systems, and command palettes.
//!
//! # Architecture
//!
//! - **Driver layer**: Defines trait contract (`CommandQueryService`)
//! - **Runner layer**: Implements trait (`CommandQuerySnapshot`)
//! - **Modules**: Access via `ServiceRegistry`

use {
    crate::{ArgSpec, Command},
    reovim_kernel::api::v1::{CommandId, Service},
};

/// Command metadata for queries (no execution capability).
///
/// This is a snapshot of command information that can be serialized,
/// cloned, and passed to modules without exposing the handler.
///
/// # Fields
///
/// - `id`: Unique command identifier (module:name)
/// - `names`: Ex-command aliases (e.g., `["w", "write"]`)
/// - `description`: Human-readable description
/// - `args`: Argument specifications
#[derive(Debug, Clone)]
pub struct CommandInfo {
    /// Unique command identifier.
    pub id: CommandId,
    /// Ex-command aliases (e.g., `["w", "write"]`).
    pub names: Vec<String>,
    /// Human-readable description.
    pub description: String,
    /// Argument specifications.
    pub args: Vec<ArgSpec>,
}

impl CommandInfo {
    /// Create from a `Command` trait implementor.
    ///
    /// Extracts all metadata into owned types for safe passing across
    /// module boundaries.
    pub fn from_command<C: Command + ?Sized>(cmd: &C) -> Self {
        Self {
            id: cmd.id(),
            names: cmd.names().iter().map(|s| (*s).to_string()).collect(),
            description: cmd.description().to_string(),
            args: cmd.args(),
        }
    }

    /// Check if this command has any ex-command names.
    ///
    /// Commands with ex-names can be invoked from the command line (`:w`).
    /// Commands without ex-names are internal-only (keybinding commands).
    #[must_use]
    pub const fn has_ex_names(&self) -> bool {
        !self.names.is_empty()
    }
}

/// Query service for command discovery and completion.
///
/// Modules access this via `ServiceRegistry` to implement features
/// like tab-completion without depending on runner internals.
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`).
///
/// # Example
///
/// ```ignore
/// // In cmdline module for tab completion:
/// let query = services.get::<dyn CommandQueryService>()?;
/// let matches = query.search_by_prefix("wri");
/// // matches contains CommandInfo for "write", etc.
/// ```
pub trait CommandQueryService: Service + Send + Sync {
    /// Search commands by name prefix (for tab completion).
    ///
    /// Returns commands where any alias starts with `prefix`.
    /// The search is case-sensitive.
    /// An empty prefix returns all commands (every string starts with `""`).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let matches = query.search_by_prefix("wri");
    /// // Returns CommandInfo for commands with aliases like "write"
    /// ```
    fn search_by_prefix(&self, prefix: &str) -> Vec<CommandInfo>;

    /// Find command by exact name or alias.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let info = query.find_by_name("w");
    /// // Returns Some(CommandInfo) for the :write command
    /// ```
    fn find_by_name(&self, name: &str) -> Option<CommandInfo>;

    /// List all commands that have ex-command names.
    ///
    /// Returns only commands where `names()` is non-empty.
    /// Use this for cmdline completion UI.
    fn list_ex_commands(&self) -> Vec<CommandInfo>;

    /// List all registered commands.
    ///
    /// Includes commands without ex-names (internal commands).
    fn list_all(&self) -> Vec<CommandInfo>;

    /// Get total command count.
    fn count(&self) -> usize;
}

// ============================================================================
// Command query provider for module access (#522)
// ============================================================================

/// Concrete service for module-level command discovery.
///
/// Stores a snapshot of command metadata so modules can list commands
/// without depending on the server crate's `CommandQuerySnapshot`.
/// Registered by the runner during bootstrap.
pub struct CommandQueryProvider {
    commands: Vec<CommandInfo>,
}

impl Service for CommandQueryProvider {}

impl CommandQueryProvider {
    /// Create from a list of command metadata.
    #[must_use]
    pub const fn new(commands: Vec<CommandInfo>) -> Self {
        Self { commands }
    }

    /// List all registered commands.
    #[must_use]
    pub fn list_all(&self) -> &[CommandInfo] {
        &self.commands
    }

    /// Get total command count.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.commands.len()
    }
}

// ============================================================================
// Ex-command query types (#453)
// ============================================================================

/// Ex-command metadata for queries.
///
/// Lighter than [`CommandInfo`] since ex-commands use `&'static str` identifiers
/// rather than [`CommandId`](reovim_kernel::api::v1::CommandId).
#[derive(Debug, Clone)]
pub struct ExCommandInfo {
    /// Handler identifier (e.g., `"write"`, `"quit"`).
    pub id: String,
    /// Command aliases (e.g., `["w", "write"]`).
    pub names: Vec<String>,
    /// Help text from [`ExCommandHandler::help()`](crate::ExCommandHandler::help).
    pub help: String,
}

/// Query service for ex-command discovery and completion.
///
/// Parallel to [`CommandQueryService`] but for ex-commands (`:w`, `:q`, `:e`).
///
/// # Thread Safety
///
/// Implementations must be thread-safe (`Send + Sync`).
pub trait ExCommandQueryService: Service + Send + Sync {
    /// Search ex-commands by name prefix (for tab completion).
    ///
    /// Returns commands where any alias starts with `prefix`.
    /// Results are deduplicated by handler id.
    /// An empty prefix returns all commands (every string starts with `""`).
    fn search_by_prefix(&self, prefix: &str) -> Vec<ExCommandInfo>;

    /// Find ex-command by exact name or alias.
    fn find_by_name(&self, name: &str) -> Option<ExCommandInfo>;

    /// List all ex-commands.
    fn list_all(&self) -> Vec<ExCommandInfo>;

    /// Get argument completions for a specific command.
    ///
    /// Delegates to the handler's `complete()` method.
    fn complete_args(&self, command: &str, partial: &str) -> Vec<String>;

    /// Get total ex-command count (unique handlers, not aliases).
    fn count(&self) -> usize;
}

#[cfg(test)]
mod tests {
    use {super::*, crate::ArgKind, reovim_kernel::api::v1::ModuleId};

    struct MockCommand {
        names: &'static [&'static str],
        module: &'static str,
        cmd_name: &'static str,
        desc: &'static str,
        cmd_args: Vec<ArgSpec>,
    }

    impl MockCommand {
        fn simple(names: &'static [&'static str]) -> Self {
            Self {
                names,
                module: "test",
                cmd_name: "mock-cmd",
                desc: "A mock command",
                cmd_args: vec![],
            }
        }
    }

    impl Command for MockCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new(self.module), self.cmd_name)
        }

        fn description(&self) -> &'static str {
            self.desc
        }

        fn args(&self) -> Vec<ArgSpec> {
            self.cmd_args.clone()
        }

        fn names(&self) -> &[&'static str] {
            self.names
        }
    }

    #[test]
    fn test_command_info_from_command() {
        let cmd = MockCommand::simple(&["write", "w"]);
        let info = CommandInfo::from_command(&cmd);

        assert_eq!(info.id.name(), "mock-cmd");
        assert_eq!(info.names, vec!["write", "w"]);
        assert_eq!(info.description, "A mock command");
        assert!(info.args.is_empty());
    }

    #[test]
    fn test_command_info_from_command_with_args() {
        let cmd = MockCommand {
            names: &["test"],
            module: "test",
            cmd_name: "test-cmd",
            desc: "Test with args",
            cmd_args: vec![
                ArgSpec::required("count", ArgKind::Count, "Number of times"),
                ArgSpec::optional("register", ArgKind::Register, "Target register"),
            ],
        };
        let info = CommandInfo::from_command(&cmd);
        assert_eq!(info.args.len(), 2);
        assert!(info.args[0].required);
        assert!(!info.args[1].required);
    }

    #[test]
    fn test_command_info_from_command_no_names() {
        let cmd = MockCommand::simple(&[]);
        let info = CommandInfo::from_command(&cmd);
        assert!(info.names.is_empty());
    }

    #[test]
    fn test_command_info_has_ex_names_true() {
        let cmd = MockCommand::simple(&["write", "w"]);
        let info = CommandInfo::from_command(&cmd);
        assert!(info.has_ex_names());
    }

    #[test]
    fn test_command_info_has_ex_names_false() {
        let cmd = MockCommand::simple(&[]);
        let info = CommandInfo::from_command(&cmd);
        assert!(!info.has_ex_names());
    }

    #[test]
    fn test_command_info_has_ex_names_single() {
        let cmd = MockCommand::simple(&["w"]);
        let info = CommandInfo::from_command(&cmd);
        assert!(info.has_ex_names());
    }

    #[test]
    fn test_command_info_clone() {
        let cmd = MockCommand::simple(&["write", "w"]);
        let info = CommandInfo::from_command(&cmd);
        let cloned = info.clone();

        assert_eq!(info.id.name(), cloned.id.name());
        assert_eq!(info.names, cloned.names);
        assert_eq!(info.description, cloned.description);
        assert_eq!(info.args.len(), cloned.args.len());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_info_debug() {
        let cmd = MockCommand::simple(&["write", "w"]);
        let info = CommandInfo::from_command(&cmd);
        let debug_str = format!("{info:?}");
        assert!(debug_str.contains("CommandInfo"));
        assert!(debug_str.contains("mock-cmd"));
    }

    #[test]
    fn test_command_info_description_owned() {
        let cmd = MockCommand::simple(&[]);
        let info = CommandInfo::from_command(&cmd);
        // description is an owned String, verify it matches
        assert_eq!(info.description, "A mock command");
    }

    #[test]
    fn test_command_info_names_owned() {
        let cmd = MockCommand::simple(&["w", "write"]);
        let info = CommandInfo::from_command(&cmd);
        // names should be owned Strings
        assert_eq!(info.names[0], "w");
        assert_eq!(info.names[1], "write");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_query_service_object_safe() {
        // Inner fn verifies compilation only, never called
        fn _accepts_dyn(_: &dyn CommandQueryService) {}
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_command_query_service_box_object_safe() {
        // Inner fn verifies compilation only, never called
        fn _accepts_box(_: Box<dyn CommandQueryService>) {}
    }

    // === ExCommandInfo tests (#453) ===

    #[test]
    fn test_ex_command_info_clone() {
        let info = ExCommandInfo {
            id: "write".to_string(),
            names: vec!["w".to_string(), "write".to_string()],
            help: "Write buffer".to_string(),
        };
        let cloned = info.clone();
        assert_eq!(cloned.id, info.id);
        assert_eq!(cloned.names, info.names);
        assert_eq!(cloned.help, info.help);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ex_command_info_debug() {
        let info = ExCommandInfo {
            id: "quit".to_string(),
            names: vec!["q".to_string()],
            help: "Quit".to_string(),
        };
        let debug_str = format!("{info:?}");
        assert!(debug_str.contains("ExCommandInfo"));
        assert!(debug_str.contains("quit"));
    }

    #[test]
    fn test_ex_command_info_fields() {
        let info = ExCommandInfo {
            id: "edit".to_string(),
            names: vec!["e".to_string(), "edit".to_string()],
            help: "Edit a file".to_string(),
        };
        assert_eq!(info.id, "edit");
        assert_eq!(info.names.len(), 2);
        assert_eq!(info.names[0], "e");
        assert_eq!(info.names[1], "edit");
        assert_eq!(info.help, "Edit a file");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_ex_command_query_service_object_safe() {
        fn _accepts_dyn(_: &dyn ExCommandQueryService) {}
        fn _accepts_box(_: Box<dyn ExCommandQueryService>) {}
    }

    // === CommandQueryProvider tests (#522) ===

    #[test]
    fn test_command_query_provider_empty() {
        let provider = CommandQueryProvider::new(vec![]);
        assert!(provider.list_all().is_empty());
        assert_eq!(provider.count(), 0);
    }

    #[test]
    fn test_command_query_provider_with_commands() {
        let cmd = MockCommand::simple(&["w", "write"]);
        let info = CommandInfo::from_command(&cmd);
        let provider = CommandQueryProvider::new(vec![info]);
        assert_eq!(provider.count(), 1);
        assert_eq!(provider.list_all()[0].id.name(), "mock-cmd");
    }

    #[test]
    fn test_command_query_provider_is_service() {
        use reovim_kernel::api::v1::ServiceRegistry;
        let provider = CommandQueryProvider::new(vec![]);
        let registry = ServiceRegistry::new();
        registry.register(std::sync::Arc::new(provider));
        assert!(registry.get::<CommandQueryProvider>().is_some());
    }
}
