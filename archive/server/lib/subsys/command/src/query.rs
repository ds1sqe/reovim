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
    crate::{ArgSpec, traits::Command},
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
/// - `names`: User command aliases (e.g., `["w", "write"]`)
/// - `description`: Human-readable description
/// - `args`: Argument specifications
#[derive(Debug, Clone)]
pub struct CommandInfo {
    /// Unique command identifier.
    pub id: CommandId,
    /// User command aliases (e.g., `["w", "write"]`).
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

    /// Check if this command has any user-facing names.
    ///
    /// Commands with names can be invoked from the command line (`:w`).
    /// Commands without names are internal-only (keybinding commands).
    #[must_use]
    pub const fn has_user_names(&self) -> bool {
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

    /// List all commands that have user-facing names.
    ///
    /// Returns only commands where `names()` is non-empty.
    /// Use this for cmdline completion UI.
    fn list_user_commands(&self) -> Vec<CommandInfo>;

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

#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;
