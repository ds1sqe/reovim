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
    // yield a single match -- no ambiguity.
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
