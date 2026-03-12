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
#[path = "node_tests.rs"]
mod tests;
