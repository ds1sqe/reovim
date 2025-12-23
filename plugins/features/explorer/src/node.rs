//! File tree node representation

#![allow(clippy::missing_errors_doc)]

use std::{
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Represents a single node in the file tree
#[derive(Clone, Debug)]
pub struct FileNode {
    /// Display name (file or directory name)
    pub name: String,
    /// Full path to the file/directory
    pub path: PathBuf,
    /// Type of node (file or directory)
    pub node_type: NodeType,
    /// Depth in the tree (0 = root)
    pub depth: usize,
    /// Whether this is a hidden file (starts with .)
    pub is_hidden: bool,
}

/// Type of file node
#[derive(Clone, Debug)]
pub enum NodeType {
    /// Regular file with metadata
    File {
        /// File size in bytes
        size: u64,
        /// Creation time
        created: Option<SystemTime>,
        /// Last modified time
        modified: Option<SystemTime>,
    },
    /// Directory that can contain children
    Directory {
        /// Whether the directory is expanded in the tree view
        expanded: bool,
        /// Child nodes (populated when expanded)
        children: Vec<FileNode>,
    },
    /// Symbolic link
    Symlink {
        /// Target of the symlink
        target: PathBuf,
        /// Whether the symlink target is broken (doesn't exist)
        broken: bool,
    },
}

impl FileNode {
    /// Create a new file node from a path
    pub fn from_path(path: &Path, depth: usize) -> io::Result<Self> {
        let metadata = path.symlink_metadata()?;
        let name = path.file_name().map_or_else(
            || path.to_string_lossy().to_string(),
            |n| n.to_string_lossy().to_string(),
        );
        let is_hidden = name.starts_with('.');

        let node_type = if metadata.is_symlink() {
            let target = std::fs::read_link(path).unwrap_or_default();
            // Check if the symlink target exists (resolved path)
            let broken = !path.exists();
            NodeType::Symlink { target, broken }
        } else if metadata.is_dir() {
            NodeType::Directory {
                expanded: false,
                children: Vec::new(),
            }
        } else {
            NodeType::File {
                size: metadata.len(),
                created: metadata.created().ok(),
                modified: metadata.modified().ok(),
            }
        };

        Ok(Self {
            name,
            path: path.to_path_buf(),
            node_type,
            depth,
            is_hidden,
        })
    }

    /// Check if this node is a directory
    #[must_use]
    pub const fn is_dir(&self) -> bool {
        matches!(self.node_type, NodeType::Directory { .. })
    }

    /// Check if this node is a file
    #[must_use]
    pub const fn is_file(&self) -> bool {
        matches!(self.node_type, NodeType::File { .. })
    }

    /// Check if this node is a symlink
    #[must_use]
    pub const fn is_symlink(&self) -> bool {
        matches!(self.node_type, NodeType::Symlink { .. })
    }

    /// Check if this symlink is broken (target doesn't exist)
    #[must_use]
    pub const fn is_broken_symlink(&self) -> bool {
        matches!(self.node_type, NodeType::Symlink { broken: true, .. })
    }

    /// Check if this directory is expanded
    #[must_use]
    pub const fn is_expanded(&self) -> bool {
        matches!(self.node_type, NodeType::Directory { expanded: true, .. })
    }

    /// Toggle the expanded state of a directory
    pub const fn toggle_expand(&mut self) {
        if let NodeType::Directory { expanded, .. } = &mut self.node_type {
            *expanded = !*expanded;
        }
    }

    /// Set the expanded state of a directory
    pub const fn set_expanded(&mut self, value: bool) {
        if let NodeType::Directory { expanded, .. } = &mut self.node_type {
            *expanded = value;
        }
    }

    /// Get children of a directory
    #[must_use]
    pub fn children(&self) -> Option<&[Self]> {
        match &self.node_type {
            NodeType::Directory { children, .. } => Some(children),
            _ => None,
        }
    }

    /// Get mutable children of a directory
    pub const fn children_mut(&mut self) -> Option<&mut Vec<Self>> {
        match &mut self.node_type {
            NodeType::Directory { children, .. } => Some(children),
            _ => None,
        }
    }

    /// Load children for a directory from the filesystem
    pub fn load_children(&mut self) -> io::Result<()> {
        if let NodeType::Directory { children, .. } = &mut self.node_type {
            children.clear();

            let entries = std::fs::read_dir(&self.path)?;
            let mut nodes: Vec<Self> = Vec::new();

            for entry in entries.flatten() {
                if let Ok(node) = Self::from_path(&entry.path(), self.depth + 1) {
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

    /// Get file size in bytes (for files only)
    #[must_use]
    pub const fn size(&self) -> Option<u64> {
        match &self.node_type {
            NodeType::File { size, .. } => Some(*size),
            _ => None,
        }
    }

    /// Get creation time (for files only)
    #[must_use]
    pub const fn created(&self) -> Option<SystemTime> {
        match &self.node_type {
            NodeType::File { created, .. } => *created,
            _ => None,
        }
    }

    /// Get modification time (for files only)
    #[must_use]
    pub const fn modified(&self) -> Option<SystemTime> {
        match &self.node_type {
            NodeType::File { modified, .. } => *modified,
            _ => None,
        }
    }

    /// Get an icon/prefix for the node type
    ///
    /// Uses the global icon registry to get the appropriate icon based on:
    /// - File extension for files
    /// - Expanded state for directories
    /// - Broken state for symlinks
    #[must_use]
    pub fn icon(&self) -> &'static str {
        use reovim_core::style::icons::{file_icons::symlink_icon, registry};

        let reg = registry().read().unwrap();
        match &self.node_type {
            NodeType::Directory { expanded, .. } => reg.dir_icon(*expanded),
            NodeType::Symlink { broken, .. } => symlink_icon(*broken, reg.icon_set()),
            NodeType::File { .. } => {
                // Get file extension
                let ext = self.path.extension().and_then(|e| e.to_str()).unwrap_or("");
                reg.file_icon(ext)
            }
        }
    }
}

/// Format a file size in bytes to human-readable format
///
/// Uses SI prefixes (K, M, G, T) with one decimal place for sizes >= 1K.
/// Returns exact byte count for sizes < 1K.
#[must_use]
#[allow(clippy::cast_precision_loss, dead_code)]
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

/// Format a `SystemTime` to human-readable format: "YYYY-MM-DD HH:MM AM/PM"
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn format_datetime(time: SystemTime) -> String {
    use std::time::UNIX_EPOCH;

    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs() as i64;

    // Convert to local time components (simplified calculation)
    // Note: This is a basic implementation without timezone library
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;

    // Calculate year, month, day (simplified - doesn't handle all edge cases perfectly)
    let mut year = 1970i32;
    let mut remaining_days = days_since_epoch;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let days_in_months: [i64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for days in days_in_months {
        if remaining_days < days {
            break;
        }
        remaining_days -= days;
        month += 1;
    }
    let day = remaining_days + 1;

    // Calculate time components
    let mut hour = (time_of_day / 3600) as u32;
    let minute = ((time_of_day % 3600) / 60) as u32;

    // Convert to 12-hour format
    let am_pm = if hour < 12 { "AM" } else { "PM" };
    if hour == 0 {
        hour = 12;
    } else if hour > 12 {
        hour -= 12;
    }

    format!("{year}-{month:02}-{day:02} {hour:02}:{minute:02} {am_pm}")
}

/// Check if a year is a leap year
const fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::fs::{self, File},
        tempfile::tempdir,
    };

    #[test]
    fn test_file_icon_by_extension() {
        let dir = tempdir().unwrap();

        // Create test files with various extensions (including realistic filenames)
        let test_cases = [
            ("main.rs", " "),       // Rust icon
            ("lib.rs", " "),        // Rust icon
            ("Cargo.toml", " "),    // TOML icon
            ("config.toml", " "),   // TOML icon
            ("README.md", " "),     // Markdown icon
            ("CHANGELOG.md", " "),  // Markdown icon
            ("package.json", " "),  // JSON icon
            ("tsconfig.json", " "), // JSON icon
            ("script.py", " "),     // Python icon
            ("Cargo.lock", " "),    // Lock icon
            ("unknown.xyz", " "),   // Unknown -> default icon
            ("LICENSE", " "),       // No extension -> default icon
        ];

        for (filename, expected_icon) in test_cases {
            let file_path = dir.path().join(filename);
            File::create(&file_path).unwrap();

            let node = FileNode::from_path(&file_path, 0).unwrap();
            let icon = node.icon();
            assert_eq!(
                icon, expected_icon,
                "File '{}' should have icon '{}' but got '{}'",
                filename, expected_icon, icon
            );
        }
    }

    #[test]
    fn test_directory_icon() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("testdir");
        fs::create_dir(&subdir).unwrap();

        let mut node = FileNode::from_path(&subdir, 0).unwrap();

        // Collapsed directory
        assert_eq!(node.icon(), "󰉋 ", "Collapsed dir should have closed folder icon");

        // Expanded directory
        node.set_expanded(true);
        assert_eq!(node.icon(), "󰝰 ", "Expanded dir should have open folder icon");
    }

    #[test]
    fn test_file_node_from_path() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        File::create(&file_path).unwrap();

        let node = FileNode::from_path(&file_path, 0).unwrap();
        assert_eq!(node.name, "test.txt");
        assert!(node.is_file());
        assert!(!node.is_hidden);
    }

    #[test]
    fn test_directory_node() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();

        let node = FileNode::from_path(&subdir, 0).unwrap();
        assert!(node.is_dir());
        assert!(!node.is_expanded());
    }

    #[test]
    fn test_hidden_file() {
        let dir = tempdir().unwrap();
        let hidden = dir.path().join(".hidden");
        File::create(&hidden).unwrap();

        let node = FileNode::from_path(&hidden, 0).unwrap();
        assert!(node.is_hidden);
    }

    #[test]
    fn test_toggle_expand() {
        let dir = tempdir().unwrap();
        let mut node = FileNode::from_path(dir.path(), 0).unwrap();

        assert!(!node.is_expanded());
        node.toggle_expand();
        assert!(node.is_expanded());
        node.toggle_expand();
        assert!(!node.is_expanded());
    }

    #[test]
    fn test_load_children() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("a.txt");
        let file2 = dir.path().join("b.txt");
        let subdir = dir.path().join("subdir");

        File::create(&file1).unwrap();
        File::create(&file2).unwrap();
        fs::create_dir(&subdir).unwrap();

        let mut node = FileNode::from_path(dir.path(), 0).unwrap();
        node.load_children().unwrap();

        let children = node.children().unwrap();
        assert_eq!(children.len(), 3);
        // Directory should come first
        assert_eq!(children[0].name, "subdir");
        assert!(children[0].is_dir());
    }
}
