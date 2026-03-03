//! File tree data structure.
//!
//! Port of `archive/pre_kernel/plugins/features/explorer/src/tree.rs`
//! adapted to use `VfsDriver` instead of `std::fs`.

use {
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

        Ok(Self { root, root_path })
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

        self.root = FileNode::from_path(&self.root_path, vfs, 0)?;
        self.root.set_expanded(true);
        self.root.load_children(vfs)?;

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
    pub fn expand(&mut self, path: &Path, vfs: &dyn VfsDriver) -> Result<(), VfsError> {
        if let Some(node) = self.get_node_mut(path)
            && node.is_dir()
            && !node.is_expanded()
        {
            node.set_expanded(true);
            node.load_children(vfs)?;
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

    /// Flatten the tree with metadata for tree structure rendering.
    ///
    /// Returns nodes with information about their position in the tree,
    /// including vertical line data for box-drawing characters.
    #[must_use]
    pub fn flatten_with_metadata(&self, show_hidden: bool) -> Vec<FlattenedNode<'_>> {
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
        if node.depth > 0 && node.is_hidden && !show_hidden {
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
                .collect();

            for (i, child) in visible_children.iter().enumerate() {
                let is_last_child = i == visible_children.len() - 1;
                vertical_lines.push(!is_last_child);

                Self::flatten_with_metadata_recursive(
                    child,
                    show_hidden,
                    result,
                    vertical_lines,
                    is_last_child,
                );

                vertical_lines.pop();
            }
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
mod tests {
    use {super::*, reovim_driver_vfs::MockVfs, std::path::Path};

    fn setup_mock_vfs() -> MockVfs {
        let vfs = MockVfs::new();
        // /root/
        //   src/
        //     main.rs
        //     lib.rs
        //   .hidden
        //   readme.md
        vfs.add_dir("/root");
        vfs.add_dir("/root/src");
        vfs.add_file("/root/src/main.rs", "fn main() {}");
        vfs.add_file("/root/src/lib.rs", "pub mod foo;");
        vfs.add_file("/root/.hidden", "secret");
        vfs.add_file("/root/readme.md", "# Hello");
        vfs
    }

    #[test]
    fn test_new_tree() {
        let vfs = setup_mock_vfs();
        let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        assert!(tree.root().is_expanded());
        // src (dir), .hidden (file), readme.md (file)
        assert_eq!(tree.root().children().unwrap().len(), 3);
    }

    #[test]
    fn test_root_path() {
        let vfs = setup_mock_vfs();
        let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        assert_eq!(tree.root_path(), Path::new("/root"));
    }

    #[test]
    fn test_flatten_show_all() {
        let vfs = setup_mock_vfs();
        let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        let flat = tree.flatten(true);
        // root + src + .hidden + readme.md = 4 (src is collapsed)
        assert_eq!(flat.len(), 4);
    }

    #[test]
    fn test_flatten_hide_hidden() {
        let vfs = setup_mock_vfs();
        let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        let flat = tree.flatten(false);
        // root + src + readme.md = 3 (no .hidden)
        assert_eq!(flat.len(), 3);
    }

    #[test]
    fn test_expand_and_flatten() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();

        tree.expand(Path::new("/root/src"), &vfs).unwrap();
        let flat = tree.flatten(true);
        // root + src + lib.rs + main.rs + .hidden + readme.md = 6
        assert_eq!(flat.len(), 6);
    }

    #[test]
    fn test_collapse() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();

        tree.expand(Path::new("/root/src"), &vfs).unwrap();
        assert_eq!(tree.flatten(true).len(), 6);

        tree.collapse(Path::new("/root/src"));
        assert_eq!(tree.flatten(true).len(), 4);
    }

    #[test]
    fn test_toggle() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();

        // Toggle expand
        tree.toggle(Path::new("/root/src"), &vfs).unwrap();
        assert_eq!(tree.flatten(true).len(), 6);

        // Toggle collapse
        tree.toggle(Path::new("/root/src"), &vfs).unwrap();
        assert_eq!(tree.flatten(true).len(), 4);
    }

    #[test]
    fn test_get_node() {
        let vfs = setup_mock_vfs();
        let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();

        let node = tree.get_node(Path::new("/root/src"));
        assert!(node.is_some());
        assert_eq!(node.unwrap().name, "src");

        let node = tree.get_node(Path::new("/nonexistent"));
        assert!(node.is_none());
    }

    #[test]
    fn test_get_node_mut() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();

        let node = tree.get_node_mut(Path::new("/root/src"));
        assert!(node.is_some());
        assert_eq!(node.unwrap().name, "src");
    }

    #[test]
    fn test_refresh() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();

        // Expand src
        tree.expand(Path::new("/root/src"), &vfs).unwrap();
        assert_eq!(tree.flatten(true).len(), 6);

        // Refresh should preserve expanded state
        tree.refresh(&vfs).unwrap();
        assert_eq!(tree.flatten(true).len(), 6);
    }

    #[test]
    fn test_expand_nonexistent_path_is_noop() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        // Expanding a nonexistent path does nothing
        tree.expand(Path::new("/nonexistent"), &vfs).unwrap();
        assert_eq!(tree.flatten(true).len(), 4);
    }

    #[test]
    fn test_collapse_nonexistent_path_is_noop() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        tree.collapse(Path::new("/nonexistent"));
        assert_eq!(tree.flatten(true).len(), 4);
    }

    #[test]
    fn test_expand_file_is_noop() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        tree.expand(Path::new("/root/readme.md"), &vfs).unwrap();
        assert_eq!(tree.flatten(true).len(), 4);
    }

    #[test]
    fn test_toggle_file_is_noop() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        tree.toggle(Path::new("/root/readme.md"), &vfs).unwrap();
        assert_eq!(tree.flatten(true).len(), 4);
    }

    #[test]
    fn test_flatten_with_metadata() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        tree.expand(Path::new("/root/src"), &vfs).unwrap();

        let flat = tree.flatten_with_metadata(true);
        assert_eq!(flat.len(), 6);

        // Root is always marked as last child
        assert!(flat[0].is_last_child);
        assert!(flat[0].vertical_lines.is_empty());

        // src/ should not be last (there are files after it)
        let src = flat.iter().find(|f| f.node.name == "src").unwrap();
        assert!(!src.is_last_child);
    }

    #[test]
    fn test_flatten_with_metadata_vertical_lines() {
        let vfs = MockVfs::new();
        // /root/
        //   dir_a/
        //     file1.txt
        //   dir_b/
        //     file2.txt
        vfs.add_dir("/root");
        vfs.add_dir("/root/dir_a");
        vfs.add_file("/root/dir_a/file1.txt", "a");
        vfs.add_dir("/root/dir_b");
        vfs.add_file("/root/dir_b/file2.txt", "b");

        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        tree.expand(Path::new("/root/dir_a"), &vfs).unwrap();
        tree.expand(Path::new("/root/dir_b"), &vfs).unwrap();

        let flat = tree.flatten_with_metadata(true);

        // Find file1.txt — parent dir_a has sibling (vertical_lines[0]=true),
        // file1 is last child of dir_a (vertical_lines[1]=false)
        let file1 = flat.iter().find(|f| f.node.name == "file1.txt").unwrap();
        assert_eq!(file1.vertical_lines, vec![true, false]);
        assert!(file1.is_last_child);

        // Find file2.txt — parent dir_b is last (vertical_lines[0]=false),
        // file2 is last child of dir_b (vertical_lines[1]=false)
        let file2 = flat.iter().find(|f| f.node.name == "file2.txt").unwrap();
        assert_eq!(file2.vertical_lines, vec![false, false]);
        assert!(file2.is_last_child);
    }

    #[test]
    fn test_flatten_with_metadata_hides_hidden() {
        let vfs = setup_mock_vfs();
        let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();

        let flat_all = tree.flatten_with_metadata(true);
        let flat_visible = tree.flatten_with_metadata(false);

        // Hidden files should be excluded when show_hidden=false
        assert!(flat_all.len() > flat_visible.len());
        assert!(!flat_visible.iter().any(|f| f.node.is_hidden));
    }

    #[test]
    fn test_expand_already_expanded_is_noop() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        tree.expand(Path::new("/root/src"), &vfs).unwrap();
        let count = tree.flatten(true).len();
        // Expanding again should not change anything
        tree.expand(Path::new("/root/src"), &vfs).unwrap();
        assert_eq!(tree.flatten(true).len(), count);
    }

    #[test]
    fn test_get_node_mut_not_found() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        let node = tree.get_node_mut(Path::new("/nonexistent"));
        assert!(node.is_none());
    }

    #[test]
    fn test_collapse_file_is_noop() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        let before = tree.flatten(true).len();
        tree.collapse(Path::new("/root/readme.md"));
        assert_eq!(tree.flatten(true).len(), before);
    }

    #[test]
    fn test_toggle_nonexistent_is_noop() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        let before = tree.flatten(true).len();
        tree.toggle(Path::new("/nonexistent"), &vfs).unwrap();
        assert_eq!(tree.flatten(true).len(), before);
    }

    #[test]
    fn test_flattened_node_clone() {
        let vfs = setup_mock_vfs();
        let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        let flat = tree.flatten_with_metadata(true);
        let cloned = flat[0].clone();
        assert_eq!(cloned.node.name, flat[0].node.name);
    }

    #[test]
    fn test_flattened_node_debug() {
        let vfs = setup_mock_vfs();
        let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        let flat = tree.flatten_with_metadata(true);
        let debug = format!("{:?}", flat[0]);
        assert!(debug.contains("FlattenedNode"));
    }

    #[test]
    fn test_tree_clone() {
        let vfs = setup_mock_vfs();
        let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        let cloned = tree.clone();
        assert_eq!(cloned.root_path(), tree.root_path());
        assert_eq!(cloned.flatten(true).len(), tree.flatten(true).len());
    }

    #[test]
    fn test_tree_debug() {
        let vfs = setup_mock_vfs();
        let tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        let debug = format!("{tree:?}");
        assert!(debug.contains("FileTree"));
    }

    #[test]
    fn test_get_node_deep() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        tree.expand(Path::new("/root/src"), &vfs).unwrap();

        // Should find deep node
        let node = tree.get_node(Path::new("/root/src/main.rs"));
        assert!(node.is_some());
        assert_eq!(node.unwrap().name, "main.rs");
    }

    #[test]
    fn test_get_node_mut_deep() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        tree.expand(Path::new("/root/src"), &vfs).unwrap();

        let node = tree.get_node_mut(Path::new("/root/src/lib.rs"));
        assert!(node.is_some());
        assert_eq!(node.unwrap().name, "lib.rs");
    }

    #[test]
    fn test_root_mut() {
        let vfs = setup_mock_vfs();
        let mut tree = FileTree::new(PathBuf::from("/root"), &vfs).unwrap();
        let root = tree.root_mut();
        assert!(root.is_expanded());
    }
}
