//! Lock-free completion cache using ArcSwap
//!
//! Follows the completion plugin pattern for non-blocking reads during render.

use std::sync::Arc;

use arc_swap::ArcSwap;

/// Kind of completion item
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmdlineCompletionKind {
    /// Command name (e.g., :write, :edit)
    Command,
    /// File path
    File,
    /// Directory path
    Directory,
    /// Option/setting
    Option,
    /// Subcommand
    Subcommand,
}

/// A single completion item
#[derive(Debug, Clone)]
pub struct CmdlineCompletionItem {
    /// Display label (e.g., "write", "src/main.rs")
    pub label: String,
    /// Short description (e.g., "Write buffer to file")
    pub description: String,
    /// Icon character
    pub icon: &'static str,
    /// Kind for styling
    pub kind: CmdlineCompletionKind,
    /// Text to insert (may differ from label for abbreviations)
    pub insert_text: String,
}

impl CmdlineCompletionItem {
    /// Create a new command completion item
    #[must_use]
    pub fn command(name: impl Into<String>, description: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            label: name.clone(),
            description: description.into(),
            icon: "",
            kind: CmdlineCompletionKind::Command,
            insert_text: name,
        }
    }

    /// Create a new file completion item
    #[must_use]
    pub fn file(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            label: name.clone(),
            description: "File".into(),
            icon: "",
            kind: CmdlineCompletionKind::File,
            insert_text: name,
        }
    }

    /// Create a new directory completion item
    #[must_use]
    pub fn directory(name: impl Into<String>) -> Self {
        let name = name.into();
        let insert_text = format!("{name}/");
        Self {
            label: name,
            description: "Directory".into(),
            icon: "",
            kind: CmdlineCompletionKind::Directory,
            insert_text,
        }
    }
}

/// A snapshot of completion state
#[derive(Debug, Clone, Default)]
pub struct CmdlineCompletionSnapshot {
    /// Available completion items
    pub items: Vec<CmdlineCompletionItem>,
    /// Currently selected index
    pub selected_index: usize,
    /// Whether completion popup is visible
    pub active: bool,
    /// Original prefix for fuzzy highlighting
    pub prefix: String,
    /// Where completion started in input
    pub replace_start: usize,
}

impl CmdlineCompletionSnapshot {
    /// Create an inactive/dismissed snapshot
    #[must_use]
    pub fn dismissed() -> Self {
        Self {
            active: false,
            ..Self::default()
        }
    }

    /// Get the currently selected item
    #[must_use]
    pub fn selected_item(&self) -> Option<&CmdlineCompletionItem> {
        if self.items.is_empty() {
            None
        } else {
            self.items.get(self.selected_index)
        }
    }

    /// Check if there are any items
    #[must_use]
    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }
}

/// Lock-free completion cache
///
/// Uses ArcSwap for atomic snapshot replacement.
pub struct CmdlineCompletionCache {
    current: ArcSwap<CmdlineCompletionSnapshot>,
}

impl CmdlineCompletionCache {
    /// Create a new empty cache
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: ArcSwap::from_pointee(CmdlineCompletionSnapshot::default()),
        }
    }

    /// Store a new snapshot
    pub fn store(&self, snapshot: CmdlineCompletionSnapshot) {
        self.current.store(Arc::new(snapshot));
    }

    /// Load the current snapshot
    #[must_use]
    pub fn load(&self) -> Arc<CmdlineCompletionSnapshot> {
        self.current.load_full()
    }

    /// Update selected index
    pub fn update_selection(&self, new_index: usize) {
        let current = self.load();
        if current.active && new_index < current.items.len() {
            let mut new_snapshot = (*current).clone();
            new_snapshot.selected_index = new_index;
            self.store(new_snapshot);
        }
    }

    /// Select next item (wraps around)
    pub fn select_next(&self) {
        let current = self.load();
        if current.active && !current.items.is_empty() {
            let new_index = (current.selected_index + 1) % current.items.len();
            self.update_selection(new_index);
        }
    }

    /// Select previous item (wraps around)
    pub fn select_prev(&self) {
        let current = self.load();
        if current.active && !current.items.is_empty() {
            let new_index = if current.selected_index == 0 {
                current.items.len() - 1
            } else {
                current.selected_index - 1
            };
            self.update_selection(new_index);
        }
    }

    /// Dismiss completion
    pub fn dismiss(&self) {
        self.store(CmdlineCompletionSnapshot::dismissed());
    }

    /// Check if completion is active
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.load().active
    }
}

impl Default for CmdlineCompletionCache {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CmdlineCompletionCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let snapshot = self.load();
        f.debug_struct("CmdlineCompletionCache")
            .field("active", &snapshot.active)
            .field("item_count", &snapshot.items.len())
            .field("selected_index", &snapshot.selected_index)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_store_load() {
        let cache = CmdlineCompletionCache::new();
        assert!(!cache.is_active());

        let items = vec![
            CmdlineCompletionItem::command("write", "Write buffer"),
            CmdlineCompletionItem::command("quit", "Quit editor"),
        ];
        cache.store(CmdlineCompletionSnapshot {
            items,
            selected_index: 0,
            active: true,
            prefix: "w".into(),
            replace_start: 0,
        });

        assert!(cache.is_active());
        let loaded = cache.load();
        assert_eq!(loaded.items.len(), 2);
        assert_eq!(loaded.prefix, "w");
    }

    #[test]
    fn test_selection_navigation() {
        let cache = CmdlineCompletionCache::new();

        let items = vec![
            CmdlineCompletionItem::command("a", "A"),
            CmdlineCompletionItem::command("b", "B"),
            CmdlineCompletionItem::command("c", "C"),
        ];
        cache.store(CmdlineCompletionSnapshot {
            items,
            selected_index: 0,
            active: true,
            prefix: String::new(),
            replace_start: 0,
        });

        assert_eq!(cache.load().selected_index, 0);

        cache.select_next();
        assert_eq!(cache.load().selected_index, 1);
        cache.select_next();
        assert_eq!(cache.load().selected_index, 2);
        cache.select_next();
        assert_eq!(cache.load().selected_index, 0); // Wrapped

        cache.select_prev();
        assert_eq!(cache.load().selected_index, 2); // Wrapped
    }

    #[test]
    fn test_dismiss() {
        let cache = CmdlineCompletionCache::new();

        let items = vec![CmdlineCompletionItem::command("test", "Test")];
        cache.store(CmdlineCompletionSnapshot {
            items,
            selected_index: 0,
            active: true,
            prefix: "t".into(),
            replace_start: 0,
        });

        assert!(cache.is_active());
        cache.dismiss();
        assert!(!cache.is_active());
        assert!(cache.load().items.is_empty());
    }

    #[test]
    fn test_completion_item_constructors() {
        let cmd = CmdlineCompletionItem::command("write", "Write buffer");
        assert_eq!(cmd.label, "write");
        assert_eq!(cmd.kind, CmdlineCompletionKind::Command);
        assert_eq!(cmd.icon, "");

        let file = CmdlineCompletionItem::file("main.rs");
        assert_eq!(file.label, "main.rs");
        assert_eq!(file.kind, CmdlineCompletionKind::File);
        assert_eq!(file.icon, "");

        let dir = CmdlineCompletionItem::directory("src");
        assert_eq!(dir.label, "src");
        assert_eq!(dir.insert_text, "src/");
        assert_eq!(dir.kind, CmdlineCompletionKind::Directory);
        assert_eq!(dir.icon, "");
    }
}
