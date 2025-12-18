//! Explorer state management

#![allow(clippy::missing_errors_doc)]

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use super::{node::FileNode, tree::FileTree};

/// Input mode for file operations
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ExplorerInputMode {
    /// Normal navigation mode
    #[default]
    None,
    /// Creating a new file
    CreateFile,
    /// Creating a new directory
    CreateDir,
    /// Renaming current item
    Rename,
    /// Confirming deletion
    ConfirmDelete,
    /// Filtering files
    Filter,
}

/// State of the file explorer
#[derive(Clone, Debug)]
pub struct ExplorerState {
    /// The file tree structure
    pub tree: FileTree,
    /// Index of the currently selected item in the flattened view
    pub cursor_index: usize,
    /// Whether to show hidden files
    pub show_hidden: bool,
    /// Current filter text
    pub filter_text: String,
    /// Width of the explorer panel
    pub width: u16,
    /// Scroll offset for the view
    pub scroll_offset: usize,
    /// Current input mode
    pub input_mode: ExplorerInputMode,
    /// Current input buffer (for create/rename/filter)
    pub input_buffer: String,
    /// Message to display (e.g., error or confirmation prompt)
    pub message: Option<String>,
}

impl ExplorerState {
    /// Create a new explorer state from a root path
    pub fn new(root_path: PathBuf) -> io::Result<Self> {
        let tree = FileTree::new(root_path)?;

        Ok(Self {
            tree,
            cursor_index: 0,
            show_hidden: false,
            filter_text: String::new(),
            width: 30,
            scroll_offset: 0,
            input_mode: ExplorerInputMode::None,
            input_buffer: String::new(),
            message: None,
        })
    }

    /// Check if in input mode
    #[must_use]
    pub fn is_input_mode(&self) -> bool {
        self.input_mode != ExplorerInputMode::None
    }

    /// Start creating a new file
    pub fn start_create_file(&mut self) {
        self.input_mode = ExplorerInputMode::CreateFile;
        self.input_buffer.clear();
        self.message = Some("Create file: ".to_string());
    }

    /// Start creating a new directory
    pub fn start_create_dir(&mut self) {
        self.input_mode = ExplorerInputMode::CreateDir;
        self.input_buffer.clear();
        self.message = Some("Create directory: ".to_string());
    }

    /// Start renaming current item
    pub fn start_rename(&mut self) {
        // Extract name first to avoid borrow issues
        let name = self.current_node().map(|n| n.name.clone());
        if let Some(name) = name {
            self.input_mode = ExplorerInputMode::Rename;
            self.input_buffer = name;
            self.message = Some("Rename to: ".to_string());
        }
    }

    /// Start delete confirmation
    pub fn start_delete(&mut self) {
        // Extract name first to avoid borrow issues
        let name = self.current_node().map(|n| n.name.clone());
        if let Some(name) = name {
            self.input_mode = ExplorerInputMode::ConfirmDelete;
            self.input_buffer.clear();
            self.message = Some(format!("Delete '{name}'? (y/n): "));
        }
    }

    /// Start filter mode
    pub fn start_filter(&mut self) {
        self.input_mode = ExplorerInputMode::Filter;
        self.input_buffer = self.filter_text.clone();
        self.message = Some("Filter: ".to_string());
    }

    /// Cancel current input mode
    pub fn cancel_input(&mut self) {
        self.input_mode = ExplorerInputMode::None;
        self.input_buffer.clear();
        self.message = None;
    }

    /// Add a character to input buffer
    pub fn input_char(&mut self, c: char) {
        self.input_buffer.push(c);
        // For filter mode, apply filter in real-time
        if self.input_mode == ExplorerInputMode::Filter {
            self.filter_text = self.input_buffer.clone();
            self.adjust_cursor_after_filter();
        }
    }

    /// Remove last character from input buffer
    pub fn input_backspace(&mut self) {
        self.input_buffer.pop();
        // For filter mode, apply filter in real-time
        if self.input_mode == ExplorerInputMode::Filter {
            self.filter_text = self.input_buffer.clone();
            self.adjust_cursor_after_filter();
        }
    }

    /// Adjust cursor after filter changes
    fn adjust_cursor_after_filter(&mut self) {
        let len = self.visible_nodes().len();
        if self.cursor_index >= len {
            self.cursor_index = len.saturating_sub(1);
        }
    }

    /// Confirm current input operation
    pub fn confirm_input(&mut self) -> io::Result<()> {
        match self.input_mode {
            ExplorerInputMode::CreateFile => {
                self.do_create_file()?;
            }
            ExplorerInputMode::CreateDir => {
                self.do_create_dir()?;
            }
            ExplorerInputMode::Rename => {
                self.do_rename()?;
            }
            ExplorerInputMode::ConfirmDelete => {
                if self.input_buffer.to_lowercase() == "y" {
                    self.do_delete()?;
                }
            }
            ExplorerInputMode::Filter | ExplorerInputMode::None => {
                // Filter is already applied, just exit input mode
            }
        }
        self.input_mode = ExplorerInputMode::None;
        self.input_buffer.clear();
        self.message = None;
        Ok(())
    }

    /// Get the parent directory for new file/directory creation
    fn get_creation_parent(&self) -> Option<PathBuf> {
        self.current_node().map_or_else(
            || Some(self.tree.root_path().to_path_buf()),
            |node| {
                if node.is_dir() {
                    Some(node.path.clone())
                } else {
                    node.path.parent().map(Path::to_path_buf)
                }
            },
        )
    }

    /// Create a new file
    fn do_create_file(&mut self) -> io::Result<()> {
        if self.input_buffer.is_empty() {
            return Ok(());
        }
        if let Some(parent) = self.get_creation_parent() {
            let path = parent.join(&self.input_buffer);
            fs::File::create(&path)?;
            self.refresh()?;
        }
        Ok(())
    }

    /// Create a new directory
    fn do_create_dir(&mut self) -> io::Result<()> {
        if self.input_buffer.is_empty() {
            return Ok(());
        }
        if let Some(parent) = self.get_creation_parent() {
            let path = parent.join(&self.input_buffer);
            fs::create_dir(&path)?;
            self.refresh()?;
        }
        Ok(())
    }

    /// Rename current item
    fn do_rename(&mut self) -> io::Result<()> {
        if self.input_buffer.is_empty() {
            return Ok(());
        }
        if let Some(node) = self.current_node() {
            let old_path = node.path.clone();
            if let Some(parent) = old_path.parent() {
                let new_path = parent.join(&self.input_buffer);
                fs::rename(&old_path, &new_path)?;
                self.refresh()?;
            }
        }
        Ok(())
    }

    /// Delete current item
    fn do_delete(&mut self) -> io::Result<()> {
        if let Some(node) = self.current_node() {
            let path = node.path.clone();
            if node.is_dir() {
                fs::remove_dir_all(&path)?;
            } else {
                fs::remove_file(&path)?;
            }
            self.refresh()?;
        }
        Ok(())
    }

    /// Get all visible nodes (respecting hidden files and filter)
    #[must_use]
    pub fn visible_nodes(&self) -> Vec<&FileNode> {
        let all_nodes = self.tree.flatten(self.show_hidden);

        if self.filter_text.is_empty() {
            return all_nodes;
        }

        // Filter nodes by name
        let filter_lower = self.filter_text.to_lowercase();
        all_nodes
            .into_iter()
            .filter(|node| node.name.to_lowercase().contains(&filter_lower))
            .collect()
    }

    /// Get the currently selected node
    #[must_use]
    pub fn current_node(&self) -> Option<&FileNode> {
        let nodes = self.visible_nodes();
        nodes.get(self.cursor_index).copied()
    }

    /// Get the path of the currently selected node
    #[must_use]
    pub fn current_path(&self) -> Option<&Path> {
        self.current_node().map(|n| n.path.as_path())
    }

    /// Move the cursor by a delta amount
    #[allow(clippy::cast_sign_loss)]
    pub fn move_cursor(&mut self, delta: isize) {
        let nodes = self.visible_nodes();
        let len = nodes.len();

        if len == 0 {
            self.cursor_index = 0;
            return;
        }

        let new_index = if delta < 0 {
            self.cursor_index.saturating_sub(delta.unsigned_abs())
        } else {
            self.cursor_index
                .saturating_add(delta as usize)
                .min(len.saturating_sub(1))
        };

        self.cursor_index = new_index;
    }

    /// Move cursor to the first item
    pub const fn move_to_first(&mut self) {
        self.cursor_index = 0;
    }

    /// Move cursor to the last item
    pub fn move_to_last(&mut self) {
        let len = self.visible_nodes().len();
        self.cursor_index = len.saturating_sub(1);
    }

    /// Move cursor by a page
    #[allow(clippy::cast_possible_wrap)]
    pub fn move_page(&mut self, height: u16, down: bool) {
        let page_size = height.saturating_sub(1) as isize;
        let delta = if down { page_size } else { -page_size };
        self.move_cursor(delta);
    }

    /// Toggle expand/collapse on the current directory
    pub fn toggle_current(&mut self) -> io::Result<()> {
        if let Some(path) = self.current_path().map(Path::to_path_buf) {
            self.tree.toggle(&path)?;
        }
        Ok(())
    }

    /// Expand the current directory
    pub fn expand_current(&mut self) -> io::Result<()> {
        if let Some(path) = self.current_path().map(Path::to_path_buf) {
            self.tree.expand(&path)?;
        }
        Ok(())
    }

    /// Collapse the current directory
    pub fn collapse_current(&mut self) {
        if let Some(path) = self.current_path().map(Path::to_path_buf) {
            self.tree.collapse(&path);
        }
    }

    /// Go to parent directory of current selection
    pub fn go_to_parent(&mut self) {
        if let Some(current) = self.current_node()
            && let Some(parent_path) = current.path.parent()
        {
            // Find the parent in the visible nodes and move cursor to it
            let nodes = self.visible_nodes();
            for (i, node) in nodes.iter().enumerate() {
                if node.path == parent_path {
                    self.cursor_index = i;
                    return;
                }
            }
        }
    }

    /// Set the filter text
    pub fn set_filter(&mut self, text: String) {
        self.filter_text = text;
        // Reset cursor to ensure it's within bounds
        let len = self.visible_nodes().len();
        if self.cursor_index >= len {
            self.cursor_index = len.saturating_sub(1);
        }
    }

    /// Clear the filter
    pub fn clear_filter(&mut self) {
        self.filter_text.clear();
    }

    /// Toggle showing hidden files
    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        // Reset cursor to ensure it's within bounds
        let len = self.visible_nodes().len();
        if self.cursor_index >= len {
            self.cursor_index = len.saturating_sub(1);
        }
    }

    /// Refresh the tree from the filesystem
    pub fn refresh(&mut self) -> io::Result<()> {
        self.tree.refresh()?;
        // Ensure cursor is within bounds
        let len = self.visible_nodes().len();
        if self.cursor_index >= len {
            self.cursor_index = len.saturating_sub(1);
        }
        Ok(())
    }

    /// Set a new root path
    pub fn set_root(&mut self, path: PathBuf) -> io::Result<()> {
        self.tree = FileTree::new(path)?;
        self.cursor_index = 0;
        self.scroll_offset = 0;
        Ok(())
    }

    /// Update scroll offset to keep cursor visible
    pub const fn update_scroll(&mut self, visible_height: u16) {
        let height = visible_height as usize;

        // Ensure cursor is visible
        if self.cursor_index < self.scroll_offset {
            self.scroll_offset = self.cursor_index;
        } else if self.cursor_index >= self.scroll_offset + height {
            self.scroll_offset = self.cursor_index.saturating_sub(height) + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::fs::File, tempfile::tempdir};

    #[test]
    fn test_new_state() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("test.txt")).unwrap();

        let state = ExplorerState::new(dir.path().to_path_buf()).unwrap();
        assert_eq!(state.cursor_index, 0);
        assert!(!state.show_hidden);
    }

    #[test]
    fn test_move_cursor() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("a.txt")).unwrap();
        File::create(dir.path().join("b.txt")).unwrap();
        File::create(dir.path().join("c.txt")).unwrap();

        let mut state = ExplorerState::new(dir.path().to_path_buf()).unwrap();

        // Move down
        state.move_cursor(1);
        assert_eq!(state.cursor_index, 1);

        state.move_cursor(1);
        assert_eq!(state.cursor_index, 2);

        // Move past end (should clamp)
        state.move_cursor(10);
        assert_eq!(state.cursor_index, 3); // root + 3 files - 1

        // Move up
        state.move_cursor(-1);
        assert_eq!(state.cursor_index, 2);

        // Move past start (should clamp)
        state.move_cursor(-10);
        assert_eq!(state.cursor_index, 0);
    }

    #[test]
    fn test_filter() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("apple.txt")).unwrap();
        File::create(dir.path().join("banana.txt")).unwrap();
        File::create(dir.path().join("cherry.txt")).unwrap();

        let mut state = ExplorerState::new(dir.path().to_path_buf()).unwrap();

        // No filter - all visible
        assert_eq!(state.visible_nodes().len(), 4); // root + 3 files

        // Filter for "an"
        state.set_filter("an".to_string());
        let nodes = state.visible_nodes();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "banana.txt");

        // Clear filter
        state.clear_filter();
        assert_eq!(state.visible_nodes().len(), 4);
    }

    #[test]
    fn test_toggle_hidden() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("visible.txt")).unwrap();
        File::create(dir.path().join(".hidden")).unwrap();

        let mut state = ExplorerState::new(dir.path().to_path_buf()).unwrap();

        // Hidden files not shown by default
        assert_eq!(state.visible_nodes().len(), 2); // root + visible

        // Toggle to show hidden
        state.toggle_hidden();
        assert_eq!(state.visible_nodes().len(), 3); // root + visible + hidden
    }
}
