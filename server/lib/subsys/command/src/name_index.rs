//! Name-based command index for ex-command resolution.
//!
//! Maps command names ("w", "write", "q") to `CommandId`s.
//! Built at bootstrap from `CommandRegistry`, stored in `ServiceRegistry`.

use {
    crate::{resolution_listener::CommandResolutionListener, traits::Command},
    parking_lot::RwLock,
    reovim_kernel::api::v1::{CommandId, Service},
    std::{collections::HashMap, fmt, sync::Arc},
};

/// Error returned when a prefix matches multiple distinct commands.
///
/// For example, if `:s` could match both `:set` and `:split`, the user
/// must type more characters to disambiguate.
#[derive(Debug, Clone)]
pub struct AmbiguousPrefix {
    /// The prefix that was searched for.
    pub prefix: String,
    /// The candidate command names that matched.
    pub candidates: Vec<String>,
}

impl fmt::Display for AmbiguousPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "E464: Ambiguous use of user-defined command: {}", self.prefix)
    }
}

impl std::error::Error for AmbiguousPrefix {}

/// Name-based command index for ex-command resolution.
///
/// Maps command names to their `CommandId` and the underlying `Command`
/// trait object (for `complete()` delegation).
///
/// # Lifecycle
///
/// 1. Server builds this at bootstrap from `CommandRegistry`
/// 2. Stored in `ServiceRegistry` as `Arc<CommandNameIndex>`
/// 3. Modules query it for name→id resolution and tab-completion
pub struct CommandNameIndex {
    /// Name → (`CommandId`, `Command` trait object) for lookup and completion.
    by_name: HashMap<String, (CommandId, Arc<dyn Command>)>,
    /// Side-effect listeners fired before every `resolve*` call.
    ///
    /// Used by the lazy-load runtime to observe command-name
    /// resolution and dlopen packages whose `on-event` trigger
    /// matches the queried name. Listeners may not influence the
    /// lookup result on this call — see
    /// [`CommandResolutionListener`].
    listeners: RwLock<Vec<Arc<dyn CommandResolutionListener>>>,
}

impl CommandNameIndex {
    /// Create an empty index.
    #[must_use]
    pub fn new() -> Self {
        Self {
            by_name: HashMap::new(),
            listeners: RwLock::new(Vec::new()),
        }
    }

    /// Insert a command by name.
    ///
    /// Each name (alias) maps to the same `CommandId` and `Command`.
    /// Last-wins if the same name is inserted twice.
    pub fn insert(&mut self, name: String, id: CommandId, cmd: Arc<dyn Command>) {
        self.by_name.insert(name, (id, cmd));
    }

    /// Register a side-effect listener fired before every `resolve*`
    /// lookup.
    ///
    /// Listeners run in registration order. They may not influence
    /// the lookup result on the same call — see
    /// [`CommandResolutionListener`].
    pub fn add_listener(&self, listener: Arc<dyn CommandResolutionListener>) {
        self.listeners.write().push(listener);
    }

    /// Fire every registered listener with `name`.
    ///
    /// Called from each `resolve*` entry point. Listeners are invoked
    /// in registration order; the index never branches on their
    /// behavior.
    fn fire_listeners(&self, name: &str) {
        for listener in self.listeners.read().iter() {
            listener.on_resolve(name);
        }
    }

    /// Resolve a command name to its `CommandId`.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&CommandId> {
        self.fire_listeners(name);
        self.by_name.get(name).map(|(id, _)| id)
    }

    /// Resolve a command name to its `CommandId` and `Command` trait object.
    ///
    /// Unlike [`resolve()`](Self::resolve), this returns the full entry so
    /// callers can access `Command::args()` for spec-driven argument binding.
    #[must_use]
    pub fn resolve_entry(&self, name: &str) -> Option<(&CommandId, &dyn Command)> {
        self.fire_listeners(name);
        self.by_name.get(name).map(|(id, cmd)| (id, cmd.as_ref()))
    }

    /// Resolve a command by exact name or unambiguous prefix.
    ///
    /// Resolution order:
    /// 1. Exact match — returned immediately (highest priority)
    /// 2. Prefix search — if exactly one distinct command matches, return it
    /// 3. Multiple aliases of the same command are deduplicated (not ambiguous)
    /// 4. Multiple distinct commands — return [`AmbiguousPrefix`] error
    ///
    /// Returns `Ok(None)` for empty input or no matches.
    ///
    /// # Errors
    ///
    /// Returns [`AmbiguousPrefix`] when the prefix matches two or more
    /// distinct commands (e.g., `:s` matching both `:set` and `:split`).
    pub fn resolve_prefix(
        &self,
        name: &str,
    ) -> Result<Option<(&CommandId, &dyn Command)>, AmbiguousPrefix> {
        self.fire_listeners(name);
        if name.is_empty() {
            return Ok(None);
        }

        // Exact match has highest priority
        if let Some(entry) = self.by_name.get(name) {
            return Ok(Some((&entry.0, entry.1.as_ref())));
        }

        // Prefix search with deduplication by CommandId
        let matches = self.search_by_prefix(name);
        match matches.len() {
            0 => Ok(None),
            1 => Ok(Some(matches[0])),
            _ => {
                // search_by_prefix() already deduplicates by CommandId,
                // so 2+ results means genuinely distinct commands.
                let mut candidates: Vec<String> = self
                    .by_name
                    .iter()
                    .filter(|(n, _)| n.starts_with(name))
                    .map(|(n, _)| n.clone())
                    .collect();
                candidates.sort();
                candidates.dedup();
                Err(AmbiguousPrefix {
                    prefix: name.to_string(),
                    candidates,
                })
            }
        }
    }

    /// Get argument completions for a named command.
    ///
    /// Delegates to the command's `complete()` method.
    #[must_use]
    pub fn complete_args(&self, name: &str, partial: &str) -> Vec<String> {
        self.by_name
            .get(name)
            .map_or_else(Vec::new, |(_, cmd)| cmd.complete(partial))
    }

    /// Search for commands whose names start with `prefix`.
    ///
    /// Returns deduplicated results (one entry per unique `CommandId`).
    #[must_use]
    pub fn search_by_prefix(&self, prefix: &str) -> Vec<(&CommandId, &dyn Command)> {
        let mut seen = std::collections::HashSet::new();
        let mut results = Vec::new();
        for (name, (id, cmd)) in &self.by_name {
            if name.starts_with(prefix) && seen.insert(id) {
                results.push((id, cmd.as_ref()));
            }
        }
        results
    }

    /// List all unique commands in the index.
    #[must_use]
    pub fn list_all(&self) -> Vec<(&CommandId, &dyn Command)> {
        let mut seen = std::collections::HashSet::new();
        self.by_name
            .values()
            .filter(|(id, _)| seen.insert(id))
            .map(|(id, cmd)| (id, cmd.as_ref()))
            .collect()
    }

    /// Get the number of unique commands (not aliases).
    #[must_use]
    pub fn count(&self) -> usize {
        let seen: std::collections::HashSet<_> = self.by_name.values().map(|(id, _)| id).collect();
        seen.len()
    }
}

impl Default for CommandNameIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for CommandNameIndex {}

impl std::fmt::Debug for CommandNameIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandNameIndex")
            .field("name_count", &self.by_name.len())
            .field("unique_commands", &self.count())
            .field("listener_count", &self.listeners.read().len())
            .finish()
    }
}

#[cfg(test)]
#[path = "name_index_tests.rs"]
mod tests;
