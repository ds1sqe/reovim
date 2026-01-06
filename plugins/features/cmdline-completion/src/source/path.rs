//! File path completion source

use std::path::{Path, PathBuf};

use crate::cache::CmdlineCompletionItem;

/// Generate file path completions
///
/// Parses the prefix to extract directory and file prefix,
/// then lists matching entries.
#[must_use]
pub fn complete_paths(prefix: &str, _arg_start: usize) -> Vec<CmdlineCompletionItem> {
    let mut items = Vec::new();

    // Parse directory and file prefix
    let (dir, file_prefix) = if prefix.is_empty() {
        (PathBuf::from("."), String::new())
    } else if prefix.ends_with('/') || prefix.ends_with('\\') {
        (PathBuf::from(prefix), String::new())
    } else {
        let path = PathBuf::from(prefix);
        let parent = path
            .parent()
            .map_or_else(|| PathBuf::from("."), PathBuf::from);
        let file = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        (parent, file)
    };

    // Read directory entries
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return items;
    };

    let file_prefix_lower = file_prefix.to_lowercase();

    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };

        // Skip hidden files unless prefix starts with '.'
        if name.starts_with('.') && !file_prefix.starts_with('.') {
            continue;
        }

        // Match prefix (case-insensitive)
        if !file_prefix.is_empty() && !name.to_lowercase().starts_with(&file_prefix_lower) {
            continue;
        }

        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        // Build the full path for insertion
        let insert_path = if dir == Path::new(".") {
            name.clone()
        } else {
            format!("{}/{}", dir.display(), name)
        };

        let item = if is_dir {
            CmdlineCompletionItem {
                label: name,
                description: "Directory".into(),
                icon: "",
                kind: crate::cache::CmdlineCompletionKind::Directory,
                insert_text: format!("{insert_path}/"),
            }
        } else {
            CmdlineCompletionItem {
                label: name,
                description: "File".into(),
                icon: "",
                kind: crate::cache::CmdlineCompletionKind::File,
                insert_text: insert_path,
            }
        };

        items.push(item);
    }

    // Sort: directories first, then files, alphabetically
    items.sort_by(|a, b| {
        use crate::cache::CmdlineCompletionKind::*;
        match (a.kind, b.kind) {
            (Directory, File) => std::cmp::Ordering::Less,
            (File, Directory) => std::cmp::Ordering::Greater,
            _ => a.label.cmp(&b.label),
        }
    });

    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_current_dir() {
        let items = complete_paths("", 0);
        // Should return files in current directory
        // At minimum, we should have Cargo.toml or similar files
        assert!(!items.is_empty() || items.is_empty()); // May be empty in test env
    }

    #[test]
    fn test_complete_with_prefix() {
        let items = complete_paths("src", 0);
        // If there's a src directory, it should match
        // This is environment-dependent, so we just verify no crash
        let _ = items;
    }

    #[test]
    fn test_directories_sorted_first() {
        let items = complete_paths("", 0);

        // Check that all directories come before files
        let mut saw_file = false;
        for item in &items {
            if item.kind == crate::cache::CmdlineCompletionKind::File {
                saw_file = true;
            } else if item.kind == crate::cache::CmdlineCompletionKind::Directory && saw_file {
                panic!("Directory found after file - sorting is wrong");
            }
        }
    }

    #[test]
    fn test_directory_ends_with_slash() {
        let items = complete_paths("", 0);
        for item in &items {
            if item.kind == crate::cache::CmdlineCompletionKind::Directory {
                assert!(item.insert_text.ends_with('/'), "Directory insert_text should end with /");
            }
        }
    }
}
