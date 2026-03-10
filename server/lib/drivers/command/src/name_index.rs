//! Name-based command index for ex-command resolution.
//!
//! Maps command names ("w", "write", "q") to `CommandId`s.
//! Built at bootstrap from `CommandRegistry`, stored in `ServiceRegistry`.

use {
    crate::Command,
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
}

impl CommandNameIndex {
    /// Create an empty index.
    #[must_use]
    pub fn new() -> Self {
        Self {
            by_name: HashMap::new(),
        }
    }

    /// Insert a command by name.
    ///
    /// Each name (alias) maps to the same `CommandId` and `Command`.
    /// Last-wins if the same name is inserted twice.
    pub fn insert(&mut self, name: String, id: CommandId, cmd: Arc<dyn Command>) {
        self.by_name.insert(name, (id, cmd));
    }

    /// Resolve a command name to its `CommandId`.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&CommandId> {
        self.by_name.get(name).map(|(id, _)| id)
    }

    /// Resolve a command name to its `CommandId` and `Command` trait object.
    ///
    /// Unlike [`resolve()`](Self::resolve), this returns the full entry so
    /// callers can access `Command::args()` for spec-driven argument binding.
    #[must_use]
    pub fn resolve_entry(&self, name: &str) -> Option<(&CommandId, &dyn Command)> {
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
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    struct TestCmd {
        module: &'static str,
        name: &'static str,
        names: &'static [&'static str],
    }

    impl Command for TestCmd {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new(self.module), self.name)
        }
        fn description(&self) -> &'static str {
            "test command"
        }
        fn names(&self) -> &[&'static str] {
            self.names
        }
    }

    struct CompletingCmd;

    impl Command for CompletingCmd {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "colorscheme")
        }
        fn description(&self) -> &'static str {
            "Set colorscheme"
        }
        fn names(&self) -> &[&'static str] {
            &["colorscheme"]
        }
        fn complete(&self, partial: &str) -> Vec<String> {
            vec![format!("{partial}-dark"), format!("{partial}-light")]
        }
    }

    fn make_index() -> CommandNameIndex {
        let mut idx = CommandNameIndex::new();
        let write_cmd: Arc<dyn Command> = Arc::new(TestCmd {
            module: "commands",
            name: "write",
            names: &["w", "write"],
        });
        let write_id = write_cmd.id();
        idx.insert("w".to_string(), write_id.clone(), Arc::clone(&write_cmd));
        idx.insert("write".to_string(), write_id, write_cmd);

        let quit_cmd: Arc<dyn Command> = Arc::new(TestCmd {
            module: "commands",
            name: "quit",
            names: &["q", "quit"],
        });
        let quit_id = quit_cmd.id();
        idx.insert("q".to_string(), quit_id.clone(), Arc::clone(&quit_cmd));
        idx.insert("quit".to_string(), quit_id, quit_cmd);

        idx
    }

    #[test]
    fn test_new_empty() {
        let idx = CommandNameIndex::new();
        assert_eq!(idx.count(), 0);
    }

    #[test]
    fn test_default_empty() {
        let idx = CommandNameIndex::default();
        assert_eq!(idx.count(), 0);
    }

    #[test]
    fn test_resolve_found() {
        let idx = make_index();
        let id = idx.resolve("w").unwrap();
        assert_eq!(id.name(), "write");
    }

    #[test]
    fn test_resolve_alias() {
        let idx = make_index();
        let id = idx.resolve("write").unwrap();
        assert_eq!(id.name(), "write");
    }

    #[test]
    fn test_resolve_not_found() {
        let idx = make_index();
        assert!(idx.resolve("nonexistent").is_none());
    }

    #[test]
    fn test_count_unique() {
        let idx = make_index();
        // 2 unique commands despite 4 name entries
        assert_eq!(idx.count(), 2);
    }

    #[test]
    fn test_complete_args_delegates() {
        let mut idx = CommandNameIndex::new();
        let cmd: Arc<dyn Command> = Arc::new(CompletingCmd);
        let id = cmd.id();
        idx.insert("colorscheme".to_string(), id, cmd);

        let completions = idx.complete_args("colorscheme", "gru");
        assert_eq!(completions.len(), 2);
        assert_eq!(completions[0], "gru-dark");
        assert_eq!(completions[1], "gru-light");
    }

    #[test]
    fn test_complete_args_not_found() {
        let idx = make_index();
        let completions = idx.complete_args("nonexistent", "");
        assert!(completions.is_empty());
    }

    #[test]
    fn test_complete_args_default_empty() {
        let idx = make_index();
        // Default Command::complete() returns empty
        let completions = idx.complete_args("w", "foo");
        assert!(completions.is_empty());
    }

    #[test]
    fn test_search_by_prefix_found() {
        let idx = make_index();
        let results = idx.search_by_prefix("w");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.name(), "write");
    }

    #[test]
    fn test_search_by_prefix_multiple() {
        let idx = make_index();
        // "q" matches both "q" and "quit" but same command
        let results = idx.search_by_prefix("q");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.name(), "quit");
    }

    #[test]
    fn test_search_by_prefix_empty_matches_all() {
        let idx = make_index();
        let results = idx.search_by_prefix("");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_by_prefix_no_match() {
        let idx = make_index();
        let results = idx.search_by_prefix("z");
        assert!(results.is_empty());
    }

    #[test]
    fn test_list_all() {
        let idx = make_index();
        let all = idx.list_all();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_list_all_empty() {
        let idx = CommandNameIndex::new();
        let all = idx.list_all();
        assert!(all.is_empty());
    }

    #[test]
    fn test_service_impl() {
        use reovim_kernel::api::v1::ServiceRegistry;
        let idx = CommandNameIndex::new();
        let registry = ServiceRegistry::new();
        registry.register(Arc::new(idx));
        assert!(registry.get::<CommandNameIndex>().is_some());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_debug() {
        let idx = make_index();
        let debug_str = format!("{idx:?}");
        assert!(debug_str.contains("CommandNameIndex"));
        assert!(debug_str.contains("unique_commands"));
    }

    #[test]
    fn test_resolve_entry_found() {
        let idx = make_index();
        let (id, cmd) = idx.resolve_entry("w").unwrap();
        assert_eq!(id.name(), "write");
        assert_eq!(cmd.description(), "test command");
    }

    #[test]
    fn test_resolve_entry_alias() {
        let idx = make_index();
        let (id, cmd) = idx.resolve_entry("quit").unwrap();
        assert_eq!(id.name(), "quit");
        assert_eq!(cmd.names(), &["q", "quit"]);
    }

    #[test]
    fn test_resolve_entry_not_found() {
        let idx = make_index();
        assert!(idx.resolve_entry("nonexistent").is_none());
    }

    // === resolve_prefix tests ===

    #[test]
    fn test_resolve_prefix_exact_match() {
        let idx = make_index();
        let result = idx.resolve_prefix("w").unwrap().unwrap();
        assert_eq!(result.0.name(), "write");
    }

    #[test]
    fn test_resolve_prefix_exact_full_name() {
        let idx = make_index();
        let result = idx.resolve_prefix("write").unwrap().unwrap();
        assert_eq!(result.0.name(), "write");
    }

    #[test]
    fn test_resolve_prefix_single_match() {
        let idx = make_index();
        // "wri" matches only "write" (prefix of "write")
        let result = idx.resolve_prefix("wri").unwrap().unwrap();
        assert_eq!(result.0.name(), "write");
    }

    #[test]
    fn test_resolve_prefix_alias_dedup() {
        let idx = make_index();
        // "qu" matches "quit" (prefix of "quit" name). search_by_prefix
        // deduplicates by CommandId, so "q" and "quit" (same command)
        // yield a single match — no ambiguity.
        let result = idx.resolve_prefix("qu").unwrap().unwrap();
        assert_eq!(result.0.name(), "quit");
    }

    #[test]
    fn test_resolve_prefix_no_match() {
        let idx = make_index();
        assert!(idx.resolve_prefix("z").unwrap().is_none());
    }

    #[test]
    fn test_resolve_prefix_empty() {
        let idx = make_index();
        assert!(idx.resolve_prefix("").unwrap().is_none());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_resolve_prefix_ambiguous() {
        // Create an index with two distinct commands sharing a prefix
        let mut idx = CommandNameIndex::new();
        let set_cmd: Arc<dyn Command> = Arc::new(TestCmd {
            module: "commands",
            name: "set",
            names: &["set"],
        });
        let split_cmd: Arc<dyn Command> = Arc::new(TestCmd {
            module: "commands",
            name: "split",
            names: &["split"],
        });
        idx.insert("set".to_string(), set_cmd.id(), set_cmd);
        idx.insert("split".to_string(), split_cmd.id(), split_cmd);

        let Err(err) = idx.resolve_prefix("s") else {
            panic!("expected AmbiguousPrefix error");
        };
        assert_eq!(err.prefix, "s");
        assert_eq!(err.candidates.len(), 2);
        assert!(err.candidates.contains(&"set".to_string()));
        assert!(err.candidates.contains(&"split".to_string()));
    }

    #[test]
    fn test_resolve_prefix_ambiguous_display() {
        let err = AmbiguousPrefix {
            prefix: "s".to_string(),
            candidates: vec!["set".to_string(), "split".to_string()],
        };
        let msg = err.to_string();
        assert!(msg.contains("E464"));
        assert!(msg.contains('s'));
    }

    #[test]
    fn test_resolve_prefix_ambiguous_debug() {
        let err = AmbiguousPrefix {
            prefix: "s".to_string(),
            candidates: vec!["set".to_string()],
        };
        let debug = format!("{err:?}");
        assert!(debug.contains("AmbiguousPrefix"));
    }

    #[test]
    fn test_resolve_prefix_ambiguous_clone() {
        let err = AmbiguousPrefix {
            prefix: "s".to_string(),
            candidates: vec!["set".to_string()],
        };
        #[allow(clippy::redundant_clone)]
        let cloned = err.clone();
        assert_eq!(cloned.prefix, "s");
    }

    #[test]
    fn test_resolve_prefix_ambiguous_error_trait() {
        let err = AmbiguousPrefix {
            prefix: "s".to_string(),
            candidates: vec!["set".to_string()],
        };
        // Verify it implements std::error::Error
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_resolve_prefix_exact_beats_prefix() {
        // "q" is an exact match for the "q" alias, should return quit
        // even though "quit" also starts with "q"
        let idx = make_index();
        let result = idx.resolve_prefix("q").unwrap().unwrap();
        assert_eq!(result.0.name(), "quit");
    }

    #[test]
    fn test_insert_overwrites() {
        let mut idx = CommandNameIndex::new();
        let cmd1: Arc<dyn Command> = Arc::new(TestCmd {
            module: "a",
            name: "first",
            names: &["w"],
        });
        let cmd2: Arc<dyn Command> = Arc::new(TestCmd {
            module: "b",
            name: "second",
            names: &["w"],
        });
        idx.insert("w".to_string(), cmd1.id(), cmd1);
        idx.insert("w".to_string(), cmd2.id(), Arc::clone(&cmd2));
        // Last-wins
        let resolved = idx.resolve("w").unwrap();
        assert_eq!(resolved.name(), "second");
    }
}
