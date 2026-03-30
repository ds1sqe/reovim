//! File tree data structure.
//!
//! Port of [`archive/pre_kernel/plugins/features/explorer/src/tree.rs`](https://github.com/ds1sqe/reovim/blob/81806439/archive/pre_kernel/plugins/features/explorer/src/tree.rs)
//! adapted to use `VfsDriver` instead of `std::fs`.

use {
    ignore::gitignore::Gitignore,
    reovim_driver_vfs::{VfsDriver, VfsError},
    std::path::{Path, PathBuf},
};

use super::node::FileNode;

/// File tree structure for the explorer.
#[derive(Clone, Debug)]
pub struct FileTree {
    /// Root node of the tree.
    root: FileNode,
    /// Root directory path.
    root_path: PathBuf,
    /// Gitignore matcher built from the root `.gitignore`.
    gitignore: Option<Gitignore>,
}

impl FileTree {
    /// Create a new file tree from a root path.
    ///
    /// Expands the root directory and loads its children.
    ///
    /// # Errors
    ///
    /// Returns `VfsError` if the root path cannot be read or its children
    /// cannot be listed.
    pub fn new(root_path: PathBuf, vfs: &dyn VfsDriver) -> Result<Self, VfsError> {
        let mut root = FileNode::from_path(&root_path, vfs, 0)?;
        root.set_expanded(true);
        root.load_children(vfs)?;

        let gitignore = build_gitignore(&root_path, vfs);
        if let Some(ref gi) = gitignore {
            mark_gitignored_recursive(&mut root, gi);
        }

        Ok(Self {
            root,
            root_path,
            gitignore,
        })
    }

    /// Get the root path.
    #[must_use]
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Get the root node.
    #[must_use]
    pub const fn root(&self) -> &FileNode {
        &self.root
    }

    /// Get the root node mutably.
    pub const fn root_mut(&mut self) -> &mut FileNode {
        &mut self.root
    }

    /// Refresh the entire tree from VFS.
    ///
    /// Remembers which directories were expanded and restores them after reload.
    ///
    /// # Errors
    ///
    /// Returns `VfsError` if the root path or any expanded directory cannot
    /// be re-read.
    pub fn refresh(&mut self, vfs: &dyn VfsDriver) -> Result<(), VfsError> {
        let expanded_paths = self.collect_expanded_paths();

        self.gitignore = build_gitignore(&self.root_path, vfs);
        self.root = FileNode::from_path(&self.root_path, vfs, 0)?;
        self.root.set_expanded(true);
        self.root.load_children(vfs)?;

        if let Some(ref gi) = self.gitignore {
            mark_gitignored_recursive(&mut self.root, gi);
        }

        for path in expanded_paths {
            let _ = self.expand(&path, vfs);
        }

        Ok(())
    }

    /// Collect paths of all expanded directories.
    fn collect_expanded_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        Self::collect_expanded_recursive(&self.root, &mut paths);
        paths
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn collect_expanded_recursive(node: &FileNode, paths: &mut Vec<PathBuf>) {
        if node.is_expanded() {
            paths.push(node.path.clone());
            if let Some(children) = node.children() {
                for child in children {
                    Self::collect_expanded_recursive(child, paths);
                }
            }
        }
    }

    /// Find a node by path.
    #[must_use]
    pub fn get_node(&self, path: &Path) -> Option<&FileNode> {
        Self::find_node_recursive(&self.root, path)
    }

    fn find_node_recursive<'a>(node: &'a FileNode, path: &Path) -> Option<&'a FileNode> {
        if node.path == path {
            return Some(node);
        }

        if let Some(children) = node.children() {
            for child in children {
                if let Some(found) = Self::find_node_recursive(child, path) {
                    return Some(found);
                }
            }
        }

        None
    }

    /// Find a node by path mutably.
    pub fn get_node_mut(&mut self, path: &Path) -> Option<&mut FileNode> {
        Self::find_node_mut_recursive(&mut self.root, path)
    }

    fn find_node_mut_recursive<'a>(
        node: &'a mut FileNode,
        path: &Path,
    ) -> Option<&'a mut FileNode> {
        if node.path == path {
            return Some(node);
        }

        if let Some(children) = node.children_mut() {
            for child in children {
                if let Some(found) = Self::find_node_mut_recursive(child, path) {
                    return Some(found);
                }
            }
        }

        None
    }

    /// Expand a directory by path.
    ///
    /// # Errors
    ///
    /// Returns `VfsError` if the directory's children cannot be loaded.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn expand(&mut self, path: &Path, vfs: &dyn VfsDriver) -> Result<(), VfsError> {
        if let Some(node) = self.get_node_mut(path)
            && node.is_dir()
            && !node.is_expanded()
        {
            node.set_expanded(true);
            node.load_children(vfs)?;
        }
        // Mark newly loaded children as gitignored.
        if let Some(gi) = self.gitignore.take() {
            if let Some(node) = self.get_node_mut(path) {
                mark_gitignored_recursive(node, &gi);
            }
            self.gitignore = Some(gi);
        }
        Ok(())
    }

    /// Collapse a directory by path.
    pub fn collapse(&mut self, path: &Path) {
        if let Some(node) = self.get_node_mut(path)
            && node.is_dir()
        {
            node.set_expanded(false);
        }
    }

    /// Toggle a directory's expanded state.
    ///
    /// # Errors
    ///
    /// Returns `VfsError` if expanding requires loading children and the
    /// directory cannot be read.
    pub fn toggle(&mut self, path: &Path, vfs: &dyn VfsDriver) -> Result<(), VfsError> {
        if let Some(node) = self.get_node_mut(path)
            && node.is_dir()
        {
            if node.is_expanded() {
                node.set_expanded(false);
            } else {
                node.set_expanded(true);
                node.load_children(vfs)?;
            }
        }
        Ok(())
    }

    /// Flatten the tree into a list of visible nodes.
    ///
    /// Only includes nodes whose parents are expanded.
    /// Skips hidden files unless `show_hidden` is true.
    /// Skips gitignored files unless `show_gitignored` is true.
    #[must_use]
    pub fn flatten(&self, show_hidden: bool, show_gitignored: bool) -> Vec<&FileNode> {
        let mut result = Vec::new();
        Self::flatten_recursive(&self.root, show_hidden, show_gitignored, &mut result);
        result
    }

    // Defensive checks (hidden filter, children None) are structurally
    // unreachable: callers pre-filter hidden nodes and expanded dirs always
    // have a children vec.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn flatten_recursive<'a>(
        node: &'a FileNode,
        show_hidden: bool,
        show_gitignored: bool,
        result: &mut Vec<&'a FileNode>,
    ) {
        if node.depth > 0 && node.is_hidden && !show_hidden {
            return;
        }
        if node.depth > 0 && node.is_gitignored && !show_gitignored {
            return;
        }

        result.push(node);

        if node.is_expanded()
            && let Some(children) = node.children()
        {
            for child in children {
                Self::flatten_recursive(child, show_hidden, show_gitignored, result);
            }
        }
    }

    /// Flatten the tree with metadata for tree structure rendering.
    ///
    /// Returns nodes with information about their position in the tree,
    /// including vertical line data for box-drawing characters.
    #[must_use]
    pub fn flatten_with_metadata(
        &self,
        show_hidden: bool,
        show_gitignored: bool,
    ) -> Vec<FlattenedNode<'_>> {
        let mut result = Vec::new();
        let mut vertical_lines = Vec::new();
        Self::flatten_with_metadata_recursive(
            &self.root,
            show_hidden,
            show_gitignored,
            &mut result,
            &mut vertical_lines,
            true,
        );
        result
    }

    // Defensive checks (hidden filter, children None) are structurally
    // unreachable: callers pre-filter hidden nodes and expanded dirs always
    // have a children vec.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn flatten_with_metadata_recursive<'a>(
        node: &'a FileNode,
        show_hidden: bool,
        show_gitignored: bool,
        result: &mut Vec<FlattenedNode<'a>>,
        vertical_lines: &mut Vec<bool>,
        is_last: bool,
    ) {
        if node.depth > 0 && node.is_hidden && !show_hidden {
            return;
        }
        if node.depth > 0 && node.is_gitignored && !show_gitignored {
            return;
        }

        result.push(FlattenedNode {
            node,
            is_last_child: is_last,
            vertical_lines: vertical_lines.clone(),
        });

        if node.is_expanded()
            && let Some(children) = node.children()
        {
            let visible_children: Vec<_> = children
                .iter()
                .filter(|child| show_hidden || !child.is_hidden)
                .filter(|child| show_gitignored || !child.is_gitignored)
                .collect();

            for (i, child) in visible_children.iter().enumerate() {
                let is_last_child = i == visible_children.len() - 1;
                vertical_lines.push(!is_last_child);

                Self::flatten_with_metadata_recursive(
                    child,
                    show_hidden,
                    show_gitignored,
                    result,
                    vertical_lines,
                    is_last_child,
                );

                vertical_lines.pop();
            }
        }
    }
}

/// Build a gitignore matcher from `.gitignore` in the given directory via VFS.
fn build_gitignore(root: &Path, vfs: &dyn VfsDriver) -> Option<Gitignore> {
    let gitignore_path = root.join(".gitignore");
    let content = vfs.read(&gitignore_path).ok()?;
    let text = std::str::from_utf8(&content).ok()?;

    let mut builder = ignore::gitignore::GitignoreBuilder::new(root);
    for line in text.lines() {
        let _ = builder.add_line(Some(gitignore_path.clone()), line);
    }
    builder.build().ok()
}

/// Recursively mark children as gitignored based on a gitignore matcher.
fn mark_gitignored_recursive(node: &mut FileNode, gitignore: &Gitignore) {
    if let Some(children) = node.children_mut() {
        for child in children.iter_mut() {
            if gitignore
                .matched_path_or_any_parents(&child.path, child.is_dir())
                .is_ignore()
            {
                child.is_gitignored = true;
            }
            mark_gitignored_recursive(child, gitignore);
        }
    }
}

/// A flattened node with metadata for tree structure rendering.
#[derive(Debug, Clone)]
pub struct FlattenedNode<'a> {
    /// The file node.
    pub node: &'a FileNode,

    /// Whether this node is the last child of its parent.
    pub is_last_child: bool,

    /// For each depth level (0..node.depth-1), whether there are more siblings
    /// below at that level that need vertical continuation lines.
    pub vertical_lines: Vec<bool>,
}

#[cfg(test)]
#[path = "file_tree_tests.rs"]
mod tests;
