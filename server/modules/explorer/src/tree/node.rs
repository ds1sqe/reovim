//! File tree node representation.
//!
//! Port of `archive/pre_kernel/plugins/features/explorer/src/node.rs`
//! adapted to use `VfsDriver` instead of `std::fs`.

use {
    reovim_driver_vfs::{DirEntry, VfsDriver, VfsError},
    std::path::{Path, PathBuf},
};

/// Type of file tree node.
#[derive(Clone, Debug)]
pub enum NodeType {
    /// Regular file with size in bytes.
    File {
        /// File size in bytes.
        size: u64,
    },
    /// Directory that can contain children.
    Directory {
        /// Whether the directory is expanded in the tree view.
        expanded: bool,
        /// Child nodes (populated when expanded).
        children: Vec<FileNode>,
    },
    /// Symbolic link.
    Symlink {
        /// Target of the symlink.
        target: PathBuf,
        /// Whether the symlink target is broken (doesn't exist).
        broken: bool,
    },
}

/// A single node in the file tree.
#[derive(Clone, Debug)]
pub struct FileNode {
    /// Display name (file or directory name).
    pub name: String,
    /// Full path to the file/directory.
    pub path: PathBuf,
    /// Type of node (file, directory, or symlink).
    pub node_type: NodeType,
    /// Depth in the tree (0 = root).
    pub depth: usize,
    /// Whether this is a hidden file (starts with `.`).
    pub is_hidden: bool,
}

impl FileNode {
    /// Create a node from a VFS directory entry.
    ///
    /// Uses `VfsDriver` to determine node type via `symlink_metadata`.
    ///
    /// # Errors
    ///
    /// Returns `VfsError` if the entry's metadata cannot be read.
    pub fn from_entry(
        entry: &DirEntry,
        vfs: &dyn VfsDriver,
        depth: usize,
    ) -> Result<Self, VfsError> {
        let meta = vfs.symlink_metadata(&entry.path)?;
        let is_hidden = entry.name.starts_with('.');

        let node_type = if meta.is_symlink {
            let target = vfs.read_link(&entry.path).unwrap_or_default();
            let broken = !vfs.exists(&entry.path);
            NodeType::Symlink { target, broken }
        } else if meta.is_dir {
            NodeType::Directory {
                expanded: false,
                children: Vec::new(),
            }
        } else {
            NodeType::File { size: meta.size }
        };

        Ok(Self {
            name: entry.name.clone(),
            path: entry.path.clone(),
            node_type,
            depth,
            is_hidden,
        })
    }

    /// Create a root node from a path.
    ///
    /// The root node is always a directory.
    ///
    /// # Errors
    ///
    /// Returns `VfsError` if the path's metadata cannot be read.
    pub fn from_path(path: &Path, vfs: &dyn VfsDriver, depth: usize) -> Result<Self, VfsError> {
        let meta = vfs.symlink_metadata(path)?;
        let name = path.file_name().map_or_else(
            || path.to_string_lossy().to_string(),
            |n| n.to_string_lossy().to_string(),
        );
        let is_hidden = name.starts_with('.');

        let node_type = if meta.is_symlink {
            let target = vfs.read_link(path).unwrap_or_default();
            let broken = !vfs.exists(path);
            NodeType::Symlink { target, broken }
        } else if meta.is_dir {
            NodeType::Directory {
                expanded: false,
                children: Vec::new(),
            }
        } else {
            NodeType::File { size: meta.size }
        };

        Ok(Self {
            name,
            path: path.to_path_buf(),
            node_type,
            depth,
            is_hidden,
        })
    }

    /// Check if this node is a directory.
    #[must_use]
    pub const fn is_dir(&self) -> bool {
        matches!(self.node_type, NodeType::Directory { .. })
    }

    /// Check if this node is a file.
    #[must_use]
    pub const fn is_file(&self) -> bool {
        matches!(self.node_type, NodeType::File { .. })
    }

    /// Check if this node is a symlink.
    #[must_use]
    pub const fn is_symlink(&self) -> bool {
        matches!(self.node_type, NodeType::Symlink { .. })
    }

    /// Check if this symlink is broken (target doesn't exist).
    #[must_use]
    pub const fn is_broken_symlink(&self) -> bool {
        matches!(self.node_type, NodeType::Symlink { broken: true, .. })
    }

    /// Check if this directory is expanded.
    #[must_use]
    pub const fn is_expanded(&self) -> bool {
        matches!(self.node_type, NodeType::Directory { expanded: true, .. })
    }

    /// Toggle the expanded state of a directory.
    pub const fn toggle_expand(&mut self) {
        if let NodeType::Directory { expanded, .. } = &mut self.node_type {
            *expanded = !*expanded;
        }
    }

    /// Set the expanded state of a directory.
    pub const fn set_expanded(&mut self, value: bool) {
        if let NodeType::Directory { expanded, .. } = &mut self.node_type {
            *expanded = value;
        }
    }

    /// Get children of a directory.
    #[must_use]
    pub fn children(&self) -> Option<&[Self]> {
        match &self.node_type {
            NodeType::Directory { children, .. } => Some(children),
            _ => None,
        }
    }

    /// Get mutable children of a directory.
    pub const fn children_mut(&mut self) -> Option<&mut Vec<Self>> {
        match &mut self.node_type {
            NodeType::Directory { children, .. } => Some(children),
            _ => None,
        }
    }

    /// Load children for a directory from VFS.
    ///
    /// Clears existing children and reloads from the filesystem.
    /// Sorts: directories first, then case-insensitive alphabetical.
    ///
    /// # Errors
    ///
    /// Returns `VfsError` if the directory listing fails.
    pub fn load_children(&mut self, vfs: &dyn VfsDriver) -> Result<(), VfsError> {
        if let NodeType::Directory { children, .. } = &mut self.node_type {
            children.clear();

            let entries = vfs.list_dir(&self.path)?;
            let mut nodes: Vec<Self> = Vec::new();

            for entry in &entries {
                if let Ok(node) = Self::from_entry(entry, vfs, self.depth + 1) {
                    nodes.push(node);
                }
            }

            // Sort: directories first, then alphabetically (case-insensitive)
            nodes.sort_by(|a, b| match (a.is_dir(), b.is_dir()) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            });

            *children = nodes;
        }
        Ok(())
    }

    /// Get file size in bytes (for files only).
    #[must_use]
    pub const fn size(&self) -> Option<u64> {
        match &self.node_type {
            NodeType::File { size, .. } => Some(*size),
            _ => None,
        }
    }
}

/// Format a file size in bytes to human-readable format.
///
/// Uses SI prefixes (K, M, G, T) with one decimal place for sizes >= 1K.
/// Returns exact byte count for sizes < 1K.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1}T", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{bytes}B")
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_vfs::MockVfs, std::path::Path};

    fn setup_mock_vfs() -> MockVfs {
        let vfs = MockVfs::new();
        // Create a directory structure:
        // /root/
        //   src/        (dir)
        //   .hidden     (hidden file)
        //   main.rs     (file, 100 bytes)
        //   readme.md   (file, 50 bytes)
        vfs.add_dir("/root");
        vfs.add_dir("/root/src");
        vfs.add_file("/root/.hidden", "secret");
        vfs.add_file("/root/main.rs", "x".repeat(100));
        vfs.add_file("/root/readme.md", "y".repeat(50));
        vfs
    }

    #[test]
    fn test_from_path_directory() {
        let vfs = setup_mock_vfs();
        let node = FileNode::from_path(Path::new("/root"), &vfs, 0).unwrap();
        assert_eq!(node.name, "root");
        assert!(node.is_dir());
        assert!(!node.is_expanded());
        assert_eq!(node.depth, 0);
        assert!(!node.is_hidden);
    }

    #[test]
    fn test_from_entry_file() {
        let vfs = setup_mock_vfs();
        let entry = DirEntry::file(PathBuf::from("/root/main.rs"));
        let node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
        assert_eq!(node.name, "main.rs");
        assert!(node.is_file());
        assert!(!node.is_dir());
        assert_eq!(node.depth, 1);
        assert_eq!(node.size(), Some(100));
    }

    #[test]
    fn test_from_entry_directory() {
        let vfs = setup_mock_vfs();
        let entry = DirEntry::directory(PathBuf::from("/root/src"));
        let node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
        assert_eq!(node.name, "src");
        assert!(node.is_dir());
        assert!(!node.is_file());
    }

    #[test]
    fn test_hidden_file() {
        let vfs = setup_mock_vfs();
        let entry = DirEntry::file(PathBuf::from("/root/.hidden"));
        let node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
        assert!(node.is_hidden);
    }

    #[test]
    fn test_toggle_expand() {
        let vfs = setup_mock_vfs();
        let mut node = FileNode::from_path(Path::new("/root"), &vfs, 0).unwrap();
        assert!(!node.is_expanded());
        node.toggle_expand();
        assert!(node.is_expanded());
        node.toggle_expand();
        assert!(!node.is_expanded());
    }

    #[test]
    fn test_set_expanded() {
        let vfs = setup_mock_vfs();
        let mut node = FileNode::from_path(Path::new("/root"), &vfs, 0).unwrap();
        node.set_expanded(true);
        assert!(node.is_expanded());
        node.set_expanded(false);
        assert!(!node.is_expanded());
    }

    #[test]
    fn test_load_children() {
        let vfs = setup_mock_vfs();
        let mut node = FileNode::from_path(Path::new("/root"), &vfs, 0).unwrap();
        node.load_children(&vfs).unwrap();

        let children = node.children().unwrap();
        assert_eq!(children.len(), 4);
        // Directories first, then alphabetical
        assert_eq!(children[0].name, "src");
        assert!(children[0].is_dir());
    }

    #[test]
    fn test_children_empty_for_file() {
        let vfs = setup_mock_vfs();
        let entry = DirEntry::file(PathBuf::from("/root/main.rs"));
        let node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
        assert!(node.children().is_none());
    }

    #[test]
    fn test_children_mut_empty_for_file() {
        let vfs = setup_mock_vfs();
        let entry = DirEntry::file(PathBuf::from("/root/main.rs"));
        let mut node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
        assert!(node.children_mut().is_none());
    }

    #[test]
    fn test_is_symlink() {
        let _vfs = MockVfs::new();
        // MockVfs doesn't support symlinks natively, but we can construct one manually
        let node = FileNode {
            name: "link".to_string(),
            path: PathBuf::from("/link"),
            node_type: NodeType::Symlink {
                target: PathBuf::from("/target"),
                broken: false,
            },
            depth: 0,
            is_hidden: false,
        };
        assert!(node.is_symlink());
        assert!(!node.is_broken_symlink());
        assert!(!node.is_dir());
        assert!(!node.is_file());
    }

    #[test]
    fn test_broken_symlink() {
        let node = FileNode {
            name: "broken".to_string(),
            path: PathBuf::from("/broken"),
            node_type: NodeType::Symlink {
                target: PathBuf::from("/nonexistent"),
                broken: true,
            },
            depth: 0,
            is_hidden: false,
        };
        assert!(node.is_symlink());
        assert!(node.is_broken_symlink());
    }

    #[test]
    fn test_file_size_none_for_dir() {
        let vfs = setup_mock_vfs();
        let node = FileNode::from_path(Path::new("/root"), &vfs, 0).unwrap();
        assert!(node.size().is_none());
    }

    #[test]
    fn test_toggle_expand_on_file_is_noop() {
        let vfs = setup_mock_vfs();
        let entry = DirEntry::file(PathBuf::from("/root/main.rs"));
        let mut node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
        assert!(!node.is_expanded());
        node.toggle_expand(); // Should be a no-op
        assert!(!node.is_expanded());
    }

    #[test]
    fn test_set_expanded_on_file_is_noop() {
        let vfs = setup_mock_vfs();
        let entry = DirEntry::file(PathBuf::from("/root/main.rs"));
        let mut node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
        node.set_expanded(true); // Should be a no-op
        assert!(!node.is_expanded());
    }

    #[test]
    fn test_load_children_on_file_is_noop() {
        let vfs = setup_mock_vfs();
        let entry = DirEntry::file(PathBuf::from("/root/main.rs"));
        let mut node = FileNode::from_entry(&entry, &vfs, 1).unwrap();
        node.load_children(&vfs).unwrap(); // Should be a no-op
        assert!(node.children().is_none());
    }

    #[test]
    fn test_load_children_sorts_dirs_first() {
        let vfs = MockVfs::new();
        vfs.add_dir("/d");
        vfs.add_file("/d/zebra.txt", "z");
        vfs.add_dir("/d/alpha");
        vfs.add_file("/d/beta.txt", "b");
        vfs.add_dir("/d/gamma");

        let mut node = FileNode::from_path(Path::new("/d"), &vfs, 0).unwrap();
        node.load_children(&vfs).unwrap();

        let children = node.children().unwrap();
        let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
        // Dirs first (alpha, gamma), then files alphabetical (beta.txt, zebra.txt)
        assert_eq!(names, &["alpha", "gamma", "beta.txt", "zebra.txt"]);
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(0), "0B");
        assert_eq!(format_size(100), "100B");
        assert_eq!(format_size(1023), "1023B");
    }

    #[test]
    fn test_format_size_kb() {
        assert_eq!(format_size(1024), "1.0K");
        assert_eq!(format_size(1536), "1.5K");
    }

    #[test]
    fn test_format_size_mb() {
        assert_eq!(format_size(1_048_576), "1.0M");
    }

    #[test]
    fn test_format_size_gb() {
        assert_eq!(format_size(1_073_741_824), "1.0G");
    }

    #[test]
    fn test_format_size_tb() {
        assert_eq!(format_size(1_099_511_627_776), "1.0T");
    }

    #[test]
    fn test_from_path_name_extraction() {
        let vfs = MockVfs::new();
        vfs.add_dir("/my-project");
        let node = FileNode::from_path(Path::new("/my-project"), &vfs, 0).unwrap();
        assert_eq!(node.name, "my-project");
    }
}
