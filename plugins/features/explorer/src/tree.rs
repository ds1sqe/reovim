//! File tree data structure

#![allow(clippy::missing_errors_doc)]

use std::{
    io,
    path::{Path, PathBuf},
};

use super::node::FileNode;

/// File tree structure for the explorer
#[derive(Clone, Debug)]
pub struct FileTree {
    /// Root node of the tree
    root: FileNode,
    /// Root directory path
    root_path: PathBuf,
}

impl FileTree {
    /// Create a new file tree from a root path
    pub fn new(root_path: PathBuf) -> io::Result<Self> {
        let mut root = FileNode::from_path(&root_path, 0)?;
        // Expand root by default and load its children
        root.set_expanded(true);
        root.load_children()?;

        Ok(Self { root, root_path })
    }

    /// Get the root path
    #[must_use]
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Get the root node
    #[must_use]
    pub const fn root(&self) -> &FileNode {
        &self.root
    }

    /// Get the root node mutably
    pub const fn root_mut(&mut self) -> &mut FileNode {
        &mut self.root
    }

    /// Refresh the entire tree from the filesystem
    pub fn refresh(&mut self) -> io::Result<()> {
        // Remember which directories were expanded
        let expanded_paths = self.collect_expanded_paths();

        // Recreate root
        self.root = FileNode::from_path(&self.root_path, 0)?;
        self.root.set_expanded(true);
        self.root.load_children()?;

        // Restore expanded state
        for path in expanded_paths {
            let _ = self.expand(&path);
        }

        Ok(())
    }

    /// Collect paths of all expanded directories
    fn collect_expanded_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        Self::collect_expanded_recursive(&self.root, &mut paths);
        paths
    }

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

    /// Find a node by path
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

    /// Find a node by path mutably
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

    /// Expand a directory by path
    pub fn expand(&mut self, path: &Path) -> io::Result<()> {
        if let Some(node) = self.get_node_mut(path)
            && node.is_dir()
            && !node.is_expanded()
        {
            node.set_expanded(true);
            node.load_children()?;
        }
        Ok(())
    }

    /// Collapse a directory by path
    pub fn collapse(&mut self, path: &Path) {
        if let Some(node) = self.get_node_mut(path)
            && node.is_dir()
        {
            node.set_expanded(false);
        }
    }

    /// Toggle a directory's expanded state
    pub fn toggle(&mut self, path: &Path) -> io::Result<()> {
        if let Some(node) = self.get_node_mut(path)
            && node.is_dir()
        {
            if node.is_expanded() {
                node.set_expanded(false);
            } else {
                node.set_expanded(true);
                node.load_children()?;
            }
        }
        Ok(())
    }

    /// Flatten the tree into a list of visible nodes
    /// Only includes nodes whose parents are expanded
    #[must_use]
    pub fn flatten(&self, show_hidden: bool) -> Vec<&FileNode> {
        let mut result = Vec::new();
        Self::flatten_recursive(&self.root, show_hidden, &mut result);
        result
    }

    fn flatten_recursive<'a>(
        node: &'a FileNode,
        show_hidden: bool,
        result: &mut Vec<&'a FileNode>,
    ) {
        // Skip hidden files if not showing them (but always show root)
        if node.depth > 0 && node.is_hidden && !show_hidden {
            return;
        }

        result.push(node);

        if node.is_expanded()
            && let Some(children) = node.children()
        {
            for child in children {
                Self::flatten_recursive(child, show_hidden, result);
            }
        }
    }

    /// Flatten the tree with metadata for tree structure rendering
    /// Returns nodes with information about their position in the tree
    #[must_use]
    pub fn flatten_with_metadata<'a>(&'a self, show_hidden: bool) -> Vec<FlattenedNode<'a>> {
        let mut result = Vec::new();
        let mut vertical_lines = Vec::new();
        Self::flatten_with_metadata_recursive(
            &self.root,
            show_hidden,
            &mut result,
            &mut vertical_lines,
            true,
        );
        result
    }

    fn flatten_with_metadata_recursive<'a>(
        node: &'a FileNode,
        show_hidden: bool,
        result: &mut Vec<FlattenedNode<'a>>,
        vertical_lines: &mut Vec<bool>,
        is_last: bool,
    ) {
        // Skip hidden files if not showing them (but always show root)
        if node.depth > 0 && node.is_hidden && !show_hidden {
            return;
        }

        // Add current node with metadata
        result.push(FlattenedNode {
            node,
            is_last_child: is_last,
            vertical_lines: vertical_lines.clone(),
        });

        // Process children if expanded
        if node.is_expanded()
            && let Some(children) = node.children()
        {
            // Filter visible children
            let visible_children: Vec<_> = children
                .iter()
                .filter(|child| show_hidden || !child.is_hidden)
                .collect();

            // Process each visible child
            for (i, child) in visible_children.iter().enumerate() {
                let is_last_child = i == visible_children.len() - 1;

                // Update vertical line tracking for this depth
                vertical_lines.push(!is_last_child);

                Self::flatten_with_metadata_recursive(
                    child,
                    show_hidden,
                    result,
                    vertical_lines,
                    is_last_child,
                );

                // Pop the vertical line tracking for this depth
                vertical_lines.pop();
            }
        }
    }
}

/// A flattened node with metadata for tree structure rendering
#[derive(Debug, Clone)]
pub struct FlattenedNode<'a> {
    /// The file node
    pub node: &'a FileNode,

    /// Whether this node is the last child of its parent
    pub is_last_child: bool,

    /// For each depth level (0..node.depth-1), whether there are more siblings
    /// below at that level that need vertical continuation lines
    pub vertical_lines: Vec<bool>,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::fs::{self, File},
        tempfile::tempdir,
    };

    #[test]
    fn test_new_tree() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("test.txt")).unwrap();

        let tree = FileTree::new(dir.path().to_path_buf()).unwrap();
        assert!(tree.root().is_expanded());
        assert_eq!(tree.root().children().unwrap().len(), 1);
    }

    #[test]
    fn test_flatten() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("a.txt")).unwrap();
        File::create(dir.path().join("b.txt")).unwrap();

        let tree = FileTree::new(dir.path().to_path_buf()).unwrap();
        let flat = tree.flatten(true);

        // Root + 2 files
        assert_eq!(flat.len(), 3);
    }

    #[test]
    fn test_toggle_expand() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        File::create(subdir.join("file.txt")).unwrap();

        let mut tree = FileTree::new(dir.path().to_path_buf()).unwrap();

        // Initially subdir is collapsed
        let flat = tree.flatten(true);
        assert_eq!(flat.len(), 2); // root + subdir

        // Expand subdir
        tree.toggle(&subdir).unwrap();
        let flat = tree.flatten(true);
        assert_eq!(flat.len(), 3); // root + subdir + file

        // Collapse subdir
        tree.toggle(&subdir).unwrap();
        let flat = tree.flatten(true);
        assert_eq!(flat.len(), 2);
    }

    #[test]
    fn test_hidden_files() {
        let dir = tempdir().unwrap();
        File::create(dir.path().join("visible.txt")).unwrap();
        File::create(dir.path().join(".hidden")).unwrap();

        let tree = FileTree::new(dir.path().to_path_buf()).unwrap();

        // With hidden files
        let flat = tree.flatten(true);
        assert_eq!(flat.len(), 3);

        // Without hidden files
        let flat = tree.flatten(false);
        assert_eq!(flat.len(), 2);
    }
}
