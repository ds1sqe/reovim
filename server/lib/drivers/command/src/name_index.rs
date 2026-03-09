//! Name-based command index for ex-command resolution.
//!
//! Maps command names ("w", "write", "q") to `CommandId`s.
//! Built at bootstrap from `CommandRegistry`, stored in `ServiceRegistry`.

use {
    crate::Command,
    reovim_kernel::api::v1::{CommandId, Service},
    std::{collections::HashMap, sync::Arc},
};

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
