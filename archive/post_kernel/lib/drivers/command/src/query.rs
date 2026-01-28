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

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    struct MockCommand {
        names: &'static [&'static str],
    }

    impl Command for MockCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "mock-cmd")
        }

        fn description(&self) -> &'static str {
            "A mock command"
        }

        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }

        fn names(&self) -> &[&'static str] {
            self.names
        }
    }

    #[test]
    fn test_command_info_from_command() {
        let cmd = MockCommand {
            names: &["write", "w"],
        };
        let info = CommandInfo::from_command(&cmd);

        assert_eq!(info.id.name(), "mock-cmd");
        assert_eq!(info.names, vec!["write", "w"]);
        assert_eq!(info.description, "A mock command");
        assert!(info.args.is_empty());
    }

    #[test]
    fn test_command_info_has_ex_names_true() {
        let cmd = MockCommand {
            names: &["write", "w"],
        };
        let info = CommandInfo::from_command(&cmd);
        assert!(info.has_ex_names());
    }

    #[test]
    fn test_command_info_has_ex_names_false() {
        let cmd = MockCommand { names: &[] };
        let info = CommandInfo::from_command(&cmd);
        assert!(!info.has_ex_names());
    }

    #[test]
    fn test_command_query_service_object_safe() {
        // Verify trait is object-safe
        fn _accepts_dyn(_: &dyn CommandQueryService) {}
    }
}
