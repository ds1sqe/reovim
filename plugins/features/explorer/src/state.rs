use std::{
    fs, io,
    path::{Path, PathBuf},
};

use super::{node::FileNode, tree::FileTree};

/// State of the file explorer
#[derive(Clone, Debug)]
pub struct ExplorerState {
    /// The file tree structure
    pub tree: FileTree,
    /// Index of the currently selected item in the flattened view
    pub cursor_index: usize,
    /// Whether to show hidden files
    pub show_hidden: bool,
    /// Whether to show file sizes
    pub show_sizes: bool,
    /// Current filter text
    pub filter_text: String,
    /// Width of the explorer panel
    pub width: u16,
    /// Scroll offset for the view
    pub scroll_offset: usize,
    /// Visible height of the explorer window (set during render)
    pub visible_height: u16,
    /// Current input mode
    pub input_mode: ExplorerInputMode,
    /// Current input buffer (for create/rename/filter)
    pub input_buffer: String,
    /// Message to display (e.g., error or confirmation prompt)
    pub message: Option<String>,
    /// Clipboard for copy/cut/paste operations
    pub clipboard: ExplorerClipboard,
    /// Multi-file selection state
    pub selection: ExplorerSelection,
    /// Whether the explorer is currently visible
    pub visible: bool,
    /// File details popup state
    pub popup: FileDetailsPopup,
    /// Enable file type syntax coloring
    pub enable_colors: bool,
    /// Tree drawing style for visual hierarchy
    pub tree_style: TreeStyle,
}

/// Tree drawing style for visual hierarchy
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TreeStyle {
    /// No tree lines, just indentation with spaces
    None,
    /// Simple ASCII indentation with ">" character
    Simple,
    /// Box-drawing characters (│, ├, └) for visual hierarchy
    #[default]
    BoxDrawing,
}

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

/// Clipboard operation type
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ClipboardOperation {
    /// Copy operation (keep original)
    #[default]
    Copy,
    /// Cut operation (delete original after paste)
    Cut,
}

/// Explorer clipboard for copy/cut/paste operations
#[derive(Clone, Debug, Default)]
pub struct ExplorerClipboard {
    /// Paths in the clipboard
    pub paths: Vec<PathBuf>,
    /// Operation type
    pub operation: ClipboardOperation,
}

/// Multi-file selection for explorer
#[derive(Clone, Debug, Default)]
pub struct ExplorerSelection {
    /// Set of selected file paths
    pub selected: std::collections::HashSet<PathBuf>,
    /// Whether visual selection mode is active
    pub active: bool,
    /// Anchor index for visual selection (where selection started)
    pub anchor_index: Option<usize>,
}

/// File details popup state
#[derive(Clone, Debug, Default)]
pub struct FileDetailsPopup {
    /// Whether the popup is visible
    pub visible: bool,
    /// File/directory name
    pub name: String,
    /// Full path
    pub path: String,
    /// Type: "file", "directory", "symlink"
    pub file_type: String,
    /// Formatted size (for files)
    pub size: Option<String>,
    /// Formatted creation time
    pub created: Option<String>,
    /// Formatted modification time
    pub modified: Option<String>,
}

impl ExplorerState {
    /// Create a new explorer state from a root path
    pub fn new(root_path: PathBuf) -> io::Result<Self> {
        let tree = FileTree::new(root_path)?;

        Ok(Self {
            tree,
            cursor_index: 0,
            show_hidden: false,
            show_sizes: false,
            filter_text: String::new(),
            width: 30,
            scroll_offset: 0,
            visible_height: 20, // Default height, updated on render
            input_mode: ExplorerInputMode::None,
            input_buffer: String::new(),
            message: None,
            clipboard: ExplorerClipboard::default(),
            selection: ExplorerSelection::default(),
            visible: false,
            popup: FileDetailsPopup::default(),
            enable_colors: true,               // Enable colors by default
            tree_style: TreeStyle::BoxDrawing, // Use box-drawing by default
        })
    }

    /// Check if in input mode
    #[must_use]
    pub fn is_input_mode(&self) -> bool {
        self.input_mode != ExplorerInputMode::None
    }

    /// Toggle explorer visibility
    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }

    /// Show the explorer
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the explorer
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Start creating a new file
    pub fn start_create_file(&mut self) {
        self.input_mode = ExplorerInputMode::CreateFile;
        self.input_buffer.clear();
        self.message = Some("Create file: ".to_string());
        tracing::info!(
            "ExplorerState: start_create_file() called, message set to: {:?}",
            self.message
        );
    }

    /// Start creating a new directory
    pub fn start_create_dir(&mut self) {
        self.input_mode = ExplorerInputMode::CreateDir;
        self.input_buffer.clear();
        self.message = Some("Create directory: ".to_string());
    }

    /// Start renaming current item
    pub fn start_rename(&mut self) {
        // Extract info first to avoid borrow issues
        let info = self.current_node().map(|n| (n.name.clone(), n.depth));
        if let Some((name, depth)) = info {
            // Protect root directory from being renamed
            if depth == 0 {
                self.message = Some("Cannot rename root directory".to_string());
                return;
            }
            self.input_mode = ExplorerInputMode::Rename;
            self.input_buffer = name;
            self.message = Some("Rename to: ".to_string());
        }
    }

    /// Start delete confirmation
    pub fn start_delete(&mut self) {
        // Extract info first to avoid borrow issues
        let info = self.current_node().map(|n| (n.name.clone(), n.depth));
        if let Some((name, depth)) = info {
            // Protect root directory from being deleted
            if depth == 0 {
                self.message = Some("Cannot delete root directory".to_string());
                return;
            }
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

    /// Ensure cursor is within valid bounds
    ///
    /// Call this after any operation that may change the number of visible nodes.
    fn validate_cursor_bounds(&mut self) {
        let len = self.visible_nodes().len();
        if len == 0 {
            self.cursor_index = 0;
        } else if self.cursor_index >= len {
            self.cursor_index = len.saturating_sub(1);
        }
    }

    /// Adjust cursor after filter changes
    fn adjust_cursor_after_filter(&mut self) {
        self.validate_cursor_bounds();
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
        self.validate_cursor_bounds();
    }

    /// Clear the filter
    pub fn clear_filter(&mut self) {
        self.filter_text.clear();
    }

    /// Toggle showing hidden files
    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.validate_cursor_bounds();
    }

    /// Toggle showing file sizes
    pub const fn toggle_sizes(&mut self) {
        self.show_sizes = !self.show_sizes;
    }

    /// Refresh the tree from the filesystem
    pub fn refresh(&mut self) -> io::Result<()> {
        self.tree.refresh()?;
        self.validate_cursor_bounds();
        Ok(())
    }

    /// Set a new root path
    pub fn set_root(&mut self, path: PathBuf) -> io::Result<()> {
        self.tree = FileTree::new(path)?;
        self.cursor_index = 0;
        self.scroll_offset = 0;
        Ok(())
    }

    /// Set visible height (called from render)
    pub const fn set_visible_height(&mut self, height: u16) {
        self.visible_height = height;
    }

    /// Update scroll offset to keep cursor visible
    pub const fn update_scroll(&mut self) {
        let height = self.visible_height as usize;
        if height == 0 {
            return;
        }

        // Ensure cursor is visible
        if self.cursor_index < self.scroll_offset {
            self.scroll_offset = self.cursor_index;
        } else if self.cursor_index >= self.scroll_offset + height {
            self.scroll_offset = self.cursor_index.saturating_sub(height) + 1;
        }
    }

    /// Show file details popup for the current node
    pub fn show_file_details(&mut self) {
        use super::node::{format_datetime, format_size};

        // Extract node data first to avoid borrow conflict
        let node_info = self.current_node().map(|node| {
            (
                node.name.clone(),
                node.path.display().to_string(),
                node.is_file(),
                node.is_dir(),
                node.is_symlink(),
                node.size(),
                node.created(),
                node.modified(),
            )
        });

        if let Some((name, path, is_file, is_dir, is_symlink, size, created, modified)) = node_info
        {
            self.popup.visible = true;
            self.popup.name = name;
            self.popup.path = path;

            if is_file {
                self.popup.file_type = "file".to_string();
                self.popup.size = size.map(format_size);
                self.popup.created = created.map(format_datetime);
                self.popup.modified = modified.map(format_datetime);
            } else if is_dir {
                self.popup.file_type = "directory".to_string();
                self.popup.size = None;
                self.popup.created = None;
                self.popup.modified = None;
            } else if is_symlink {
                self.popup.file_type = "symlink".to_string();
                self.popup.size = None;
                self.popup.created = None;
                self.popup.modified = None;
            }
        }
    }

    /// Close the file details popup
    pub fn close_popup(&mut self) {
        self.popup.visible = false;
    }

    /// Check if popup is visible
    #[must_use]
    pub const fn is_popup_visible(&self) -> bool {
        self.popup.visible
    }

    /// Sync popup with current cursor position (update content if visible)
    pub fn sync_popup(&mut self) {
        if self.popup.visible {
            self.show_file_details();
        }
    }

    /// Yank (copy) current item to clipboard
    pub fn yank_current(&mut self) {
        // Extract data first to avoid borrow conflicts
        let node_info = self
            .current_node()
            .map(|node| (node.path.clone(), node.name.clone(), node.depth));

        if let Some((path, name, depth)) = node_info {
            // Don't allow yanking root
            if depth == 0 {
                self.message = Some("Cannot yank root directory".to_string());
                return;
            }
            self.clipboard.paths = vec![path];
            self.clipboard.operation = ClipboardOperation::Copy;
            self.message = Some(format!("Yanked: {name}"));
        }
    }

    /// Cut current item to clipboard
    pub fn cut_current(&mut self) {
        // Extract data first to avoid borrow conflicts
        let node_info = self
            .current_node()
            .map(|node| (node.path.clone(), node.name.clone(), node.depth));

        if let Some((path, name, depth)) = node_info {
            // Don't allow cutting root
            if depth == 0 {
                self.message = Some("Cannot cut root directory".to_string());
                return;
            }
            self.clipboard.paths = vec![path];
            self.clipboard.operation = ClipboardOperation::Cut;
            self.message = Some(format!("Cut: {name}"));
        }
    }

    /// Paste from clipboard to current directory
    pub fn paste(&mut self) -> io::Result<()> {
        if self.clipboard.paths.is_empty() {
            self.message = Some("Clipboard is empty".to_string());
            return Ok(());
        }

        // Get target directory
        let target_dir = self
            .get_creation_parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No target directory"))?;

        let mut success_count = 0;
        let operation = self.clipboard.operation.clone();

        for source_path in self.clipboard.paths.clone() {
            let file_name = source_path
                .file_name()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid path"))?;
            let mut dest_path = target_dir.join(file_name);

            // Handle name conflicts by appending _copy
            if dest_path.exists() && dest_path != source_path {
                let stem = dest_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file");
                let ext = dest_path.extension().and_then(|s| s.to_str());
                let new_name =
                    ext.map_or_else(|| format!("{stem}_copy"), |ext| format!("{stem}_copy.{ext}"));
                dest_path = target_dir.join(new_name);
            }

            // Skip if source and dest are the same
            if dest_path == source_path {
                continue;
            }

            // Perform the operation
            let result = if source_path.is_dir() {
                copy_dir_recursive(&source_path, &dest_path)
            } else {
                fs::copy(&source_path, &dest_path).map(|_| ())
            };

            if result.is_ok() {
                success_count += 1;
                // For cut operation, delete the source after successful copy
                if operation == ClipboardOperation::Cut {
                    if source_path.is_dir() {
                        let _ = fs::remove_dir_all(&source_path);
                    } else {
                        let _ = fs::remove_file(&source_path);
                    }
                }
            }
        }

        // Clear clipboard after cut operation
        if operation == ClipboardOperation::Cut {
            self.clipboard.paths.clear();
        }

        self.message = Some(format!("Pasted {success_count} item(s)"));
        self.refresh()?;
        Ok(())
    }

    /// Enter visual selection mode
    pub fn enter_visual_mode(&mut self) {
        self.selection.active = true;
        self.selection.anchor_index = Some(self.cursor_index);
        self.selection.selected.clear();

        // Select current item
        if let Some(node) = self.current_node()
            && node.depth > 0
        {
            self.selection.selected.insert(node.path.clone());
        }
    }

    /// Exit visual selection mode
    pub fn exit_visual_mode(&mut self) {
        self.selection.active = false;
        self.selection.anchor_index = None;
        self.selection.selected.clear();
    }

    /// Toggle selection of current item
    pub fn toggle_select_current(&mut self) {
        let path = self
            .current_node()
            .filter(|n| n.depth > 0)
            .map(|n| n.path.clone());

        if let Some(path) = path {
            if self.selection.selected.contains(&path) {
                self.selection.selected.remove(&path);
            } else {
                self.selection.selected.insert(path);
            }
        }
    }

    /// Select all visible items
    pub fn select_all(&mut self) {
        // Collect paths first to avoid borrow conflict
        let paths: Vec<PathBuf> = self
            .visible_nodes()
            .iter()
            .filter(|node| node.depth > 0)
            .map(|node| node.path.clone())
            .collect();

        self.selection.active = true;
        self.selection.selected.clear();
        for path in paths {
            self.selection.selected.insert(path);
        }

        let count = self.selection.selected.len();
        self.message = Some(format!("Selected {count} item(s)"));
    }

    /// Update visual selection when cursor moves
    pub fn update_visual_selection(&mut self) {
        if !self.selection.active {
            return;
        }

        let Some(anchor) = self.selection.anchor_index else {
            return;
        };

        let start = anchor.min(self.cursor_index);
        let end = anchor.max(self.cursor_index);

        // Collect paths first to avoid borrow conflict
        let paths: Vec<PathBuf> = self
            .visible_nodes()
            .iter()
            .enumerate()
            .filter(|(i, node)| *i >= start && *i <= end && node.depth > 0)
            .map(|(_, node)| node.path.clone())
            .collect();

        self.selection.selected.clear();
        for path in paths {
            self.selection.selected.insert(path);
        }
    }

    /// Check if a path is selected
    #[must_use]
    pub fn is_selected(&self, path: &Path) -> bool {
        self.selection.selected.contains(path)
    }

    /// Yank selected items to clipboard (for multi-selection)
    pub fn yank_selected(&mut self) {
        if self.selection.selected.is_empty() {
            self.yank_current();
            return;
        }

        let paths: Vec<PathBuf> = self.selection.selected.iter().cloned().collect();
        let count = paths.len();
        self.clipboard.paths = paths;
        self.clipboard.operation = ClipboardOperation::Copy;
        self.message = Some(format!("Yanked {count} item(s)"));
        self.exit_visual_mode();
    }

    /// Cut selected items (for multi-selection)
    pub fn cut_selected(&mut self) {
        if self.selection.selected.is_empty() {
            self.cut_current();
            return;
        }

        let paths: Vec<PathBuf> = self.selection.selected.iter().cloned().collect();
        let count = paths.len();
        self.clipboard.paths = paths;
        self.clipboard.operation = ClipboardOperation::Cut;
        self.message = Some(format!("Cut {count} item(s)"));
        self.exit_visual_mode();
    }
}

/// Recursively copy a directory and its contents
fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
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
