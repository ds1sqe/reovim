//! Command name completion source

use crate::cache::CmdlineCompletionItem;

/// Built-in commands with their abbreviations and descriptions
const BUILTIN_COMMANDS: &[(&str, &str, &str)] = &[
    // File operations
    ("q", "quit", "Quit editor"),
    ("w", "write", "Write buffer to file"),
    ("wq", "wq", "Write and quit"),
    ("x", "x", "Write if modified and quit"),
    ("e", "edit", "Open file for editing"),
    // Window management
    ("sp", "split", "Horizontal split"),
    ("vs", "vsplit", "Vertical split"),
    ("close", "close", "Close current window"),
    ("only", "only", "Close other windows"),
    // Tab management
    ("tabnew", "tabnew", "Open new tab"),
    ("tabclose", "tabclose", "Close current tab"),
    ("tabnext", "tabnext", "Go to next tab"),
    ("tabprev", "tabprev", "Go to previous tab"),
    // Settings
    ("set", "set", "Set option value"),
    ("colorscheme", "colorscheme", "Change color scheme"),
    // Help
    ("help", "help", "Open help"),
    ("h", "help", "Open help"),
];

/// Generate command completions
///
/// Matches both abbreviations and full names against the prefix.
/// Includes built-in commands and registry commands.
#[must_use]
pub fn complete_commands(
    prefix: &str,
    registry_commands: &[(String, String)],
) -> Vec<CmdlineCompletionItem> {
    let prefix_lower = prefix.to_lowercase();
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Match built-in commands
    for (abbrev, full, desc) in BUILTIN_COMMANDS {
        // Match if abbreviation or full name starts with prefix
        if (abbrev.starts_with(&prefix_lower) || full.starts_with(&prefix_lower))
            && seen.insert(*full)
        {
            items.push(CmdlineCompletionItem::command(*full, *desc));
        }
    }

    // Match registry commands (from plugins)
    for (name, desc) in registry_commands {
        let name_lower = name.to_lowercase();
        if name_lower.starts_with(&prefix_lower) && !seen.contains(name.as_str()) {
            items.push(CmdlineCompletionItem::command(name, desc));
        }
    }

    // Sort alphabetically
    items.sort_by(|a, b| a.label.cmp(&b.label));

    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_write() {
        let items = complete_commands("wr", &[]);
        assert!(!items.is_empty());
        assert!(items.iter().any(|i| i.label == "write"));
    }

    #[test]
    fn test_complete_abbreviation() {
        let items = complete_commands("w", &[]);
        assert!(items.iter().any(|i| i.label == "write"));
        assert!(items.iter().any(|i| i.label == "wq"));
    }

    #[test]
    fn test_complete_quit() {
        let items = complete_commands("q", &[]);
        assert!(items.iter().any(|i| i.label == "quit"));
    }

    #[test]
    fn test_complete_empty_prefix() {
        let items = complete_commands("", &[]);
        // All unique commands match empty prefix
        // Note: BUILTIN_COMMANDS has 17 entries but some share the same full name
        // (e.g., "h" and "help" both map to "help"), so we get fewer unique items
        assert!(items.len() > 10); // Sanity check - should have many commands
    }

    #[test]
    fn test_complete_registry_commands() {
        let registry = vec![
            ("settings".to_string(), "Open settings menu".to_string()),
            ("telescope".to_string(), "Open fuzzy finder".to_string()),
        ];
        let items = complete_commands("se", &registry);
        assert!(items.iter().any(|i| i.label == "set"));
        assert!(items.iter().any(|i| i.label == "settings"));
    }

    #[test]
    fn test_complete_case_insensitive() {
        let items = complete_commands("WR", &[]);
        assert!(items.iter().any(|i| i.label == "write"));
    }

    #[test]
    fn test_no_duplicates() {
        let items = complete_commands("h", &[]);
        // "help" should appear only once even though both "h" and "help" match
        let help_count = items.iter().filter(|i| i.label == "help").count();
        assert_eq!(help_count, 1);
    }
}
