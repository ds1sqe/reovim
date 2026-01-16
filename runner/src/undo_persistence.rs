//! Undo tree persistence.
//!
//! Provides functionality to persist and restore undo trees to/from disk.
//! Uses a centralized undo directory at `~/.local/share/reovim/undo/`.
//!
//! # Architecture
//!
//! This module bridges between:
//! - **Kernel**: `UndoTree` type (the actual undo data structure)
//! - **Protocol**: Serialization types (`SerializableUndoTree`, `UndoFileFormat`)
//! - **VFS**: File system abstraction for reading/writing files
//!
//! # File Format
//!
//! Undo files use `MessagePack` binary format with a 4-byte magic header ("RUND").
//! Files are stored in a centralized directory with percent-encoded paths.

use {
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::UndoTree,
    reovim_protocol::v1::undo::{UndoFileError, UndoFileFormat, from_undo_tree, to_undo_tree},
    std::path::{Path, PathBuf},
};

/// Default undo directory under the data directory.
const UNDO_SUBDIR: &str = "undo";

/// File extension for undo files.
const UNDO_EXTENSION: &str = ".undo";

/// Undo persistence manager.
///
/// Handles serialization and deserialization of undo trees to disk.
#[derive(Debug)]
pub struct UndoPersistence {
    /// Base directory for undo files (e.g., `~/.local/share/reovim/undo/`).
    undo_dir: PathBuf,
}

impl UndoPersistence {
    /// Create a new persistence manager with the given data directory.
    ///
    /// The undo directory will be `{data_dir}/undo/`.
    #[must_use]
    pub fn new(data_dir: &Path) -> Self {
        Self {
            undo_dir: data_dir.join(UNDO_SUBDIR),
        }
    }

    /// Get the undo directory path.
    #[must_use]
    pub fn undo_dir(&self) -> &Path {
        &self.undo_dir
    }

    /// Ensure the undo directory exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    pub fn ensure_dir(&self, vfs: &dyn VfsDriver) -> Result<(), UndoPersistError> {
        if !vfs.exists(&self.undo_dir) {
            vfs.create_dir_all(&self.undo_dir)
                .map_err(|e| UndoPersistError::Io(format!("Failed to create undo dir: {e}")))?;
        }
        Ok(())
    }

    /// Get the undo file path for a buffer's file path.
    #[must_use]
    pub fn undo_file_path(&self, buffer_path: &str) -> PathBuf {
        let encoded = encode_path_component(buffer_path);
        self.undo_dir.join(format!("{encoded}{UNDO_EXTENSION}"))
    }

    /// Persist an undo tree to disk.
    ///
    /// # Arguments
    ///
    /// * `buffer_path` - The original file path of the buffer
    /// * `tree` - The undo tree to persist
    /// * `vfs` - VFS driver for file operations
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The undo directory cannot be created
    /// - Serialization fails
    /// - File write fails
    pub fn persist(
        &self,
        buffer_path: &str,
        tree: &UndoTree,
        vfs: &dyn VfsDriver,
    ) -> Result<(), UndoPersistError> {
        // Ensure directory exists
        self.ensure_dir(vfs)?;

        // Convert to serializable format
        let serializable = from_undo_tree(tree);
        let format = UndoFileFormat::new(buffer_path.to_string(), serializable);

        // Serialize to bytes
        let bytes = format
            .to_bytes()
            .map_err(|e| UndoPersistError::Serialize(e.to_string()))?;

        // Write to file
        let undo_path = self.undo_file_path(buffer_path);
        vfs.write(&undo_path, &bytes)
            .map_err(|e| UndoPersistError::Io(e.to_string()))?;

        tracing::debug!("Persisted undo tree for '{}' ({} bytes)", buffer_path, bytes.len());

        Ok(())
    }

    /// Load an undo tree from disk.
    ///
    /// Returns `None` if no undo file exists for this buffer.
    ///
    /// # Arguments
    ///
    /// * `buffer_path` - The original file path of the buffer
    /// * `vfs` - VFS driver for file operations
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but is corrupt or unreadable.
    pub fn load(
        &self,
        buffer_path: &str,
        vfs: &dyn VfsDriver,
    ) -> Result<Option<UndoTree>, UndoPersistError> {
        let undo_path = self.undo_file_path(buffer_path);

        if !vfs.exists(&undo_path) {
            return Ok(None);
        }

        // Read file
        let bytes = vfs
            .read(&undo_path)
            .map_err(|e| UndoPersistError::Io(e.to_string()))?;

        // Deserialize
        let format = UndoFileFormat::from_bytes(&bytes).map_err(|e| match e {
            UndoFileError::TooShort => UndoPersistError::Deserialize("File too short".to_string()),
            UndoFileError::InvalidMagic => {
                UndoPersistError::Deserialize("Invalid magic bytes".to_string())
            }
            UndoFileError::Deserialize(e) => UndoPersistError::Deserialize(e.to_string()),
            UndoFileError::Io(e) => UndoPersistError::Io(e.to_string()),
        })?;

        // Verify path matches (log warning if mismatch)
        if format.original_path != buffer_path {
            tracing::warn!(
                "Undo file path mismatch: expected '{}', found '{}'",
                buffer_path,
                format.original_path
            );
        }

        // Convert back to kernel type
        let tree = to_undo_tree(&format.tree);

        tracing::debug!("Loaded undo tree for '{}' ({} nodes)", buffer_path, tree.node_count());

        Ok(Some(tree))
    }

    /// Delete the undo file for a buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be deleted.
    pub fn delete(&self, buffer_path: &str, vfs: &dyn VfsDriver) -> Result<(), UndoPersistError> {
        let undo_path = self.undo_file_path(buffer_path);

        if vfs.exists(&undo_path) {
            vfs.delete(&undo_path)
                .map_err(|e| UndoPersistError::Io(e.to_string()))?;
            tracing::debug!("Deleted undo file for '{}'", buffer_path);
        }

        Ok(())
    }

    /// Check if an undo file exists for a buffer.
    #[must_use]
    pub fn exists(&self, buffer_path: &str, vfs: &dyn VfsDriver) -> bool {
        let undo_path = self.undo_file_path(buffer_path);
        vfs.exists(&undo_path)
    }
}

/// Errors that can occur during undo persistence.
#[derive(Debug)]
pub enum UndoPersistError {
    /// Serialization error.
    Serialize(String),
    /// Deserialization error.
    Deserialize(String),
    /// I/O error.
    Io(String),
}

impl std::fmt::Display for UndoPersistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialize(e) => write!(f, "Serialization error: {e}"),
            Self::Deserialize(e) => write!(f, "Deserialization error: {e}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for UndoPersistError {}

/// Percent-encode a file path for use as a filename.
///
/// Encodes characters that are not safe for filenames:
/// - `/` -> `%2F`
/// - `\` -> `%5C`
/// - `:` -> `%3A`
/// - `%` -> `%25` (escape the escape character)
/// - `<`, `>`, `"`, `|`, `?`, `*` -> encoded (Windows reserved)
///
/// # Example
///
/// ```ignore
/// assert_eq!(
///     encode_path_component("/home/user/file.rs"),
///     "%2Fhome%2Fuser%2Ffile.rs"
/// );
/// ```
#[must_use]
pub fn encode_path_component(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len() * 2);

    for c in path.chars() {
        match c {
            '%' => encoded.push_str("%25"),
            '/' => encoded.push_str("%2F"),
            '\\' => encoded.push_str("%5C"),
            ':' => encoded.push_str("%3A"),
            '<' => encoded.push_str("%3C"),
            '>' => encoded.push_str("%3E"),
            '"' => encoded.push_str("%22"),
            '|' => encoded.push_str("%7C"),
            '?' => encoded.push_str("%3F"),
            '*' => encoded.push_str("%2A"),
            _ => encoded.push(c),
        }
    }

    encoded
}

/// Decode a percent-encoded path component.
///
/// Reverses the encoding done by [`encode_path_component`].
#[must_use]
pub fn decode_path_component(encoded: &str) -> String {
    let mut decoded = String::with_capacity(encoded.len());
    let mut chars = encoded.chars();

    while let Some(c) = chars.next() {
        if c == '%' {
            // Read two hex digits
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2
                && let Ok(byte) = u8::from_str_radix(&hex, 16)
            {
                decoded.push(byte as char);
                continue;
            }
            // Invalid escape sequence, keep as-is
            decoded.push('%');
            decoded.push_str(&hex);
        } else {
            decoded.push(c);
        }
    }

    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_path_component_unix() {
        assert_eq!(encode_path_component("/home/user/file.rs"), "%2Fhome%2Fuser%2Ffile.rs");
    }

    #[test]
    fn test_encode_path_component_windows() {
        assert_eq!(
            encode_path_component("C:\\Users\\Name\\file.txt"),
            "C%3A%5CUsers%5CName%5Cfile.txt"
        );
    }

    #[test]
    fn test_encode_path_component_with_percent() {
        assert_eq!(encode_path_component("/path/100%/file.txt"), "%2Fpath%2F100%25%2Ffile.txt");
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let paths = [
            "/home/user/project/src/main.rs",
            "C:\\Users\\Name\\Documents\\file.txt",
            "/tmp/test%file.txt",
            "/path/with spaces/file.rs",
            "/special<>|?*chars.txt",
        ];

        for path in paths {
            let encoded = encode_path_component(path);
            let decoded = decode_path_component(&encoded);
            assert_eq!(path, decoded, "Round-trip failed for: {path}");
        }
    }

    #[test]
    fn test_undo_file_path() {
        let persistence = UndoPersistence::new(Path::new("/home/user/.local/share/reovim"));
        let undo_path = persistence.undo_file_path("/home/user/project/main.rs");
        assert_eq!(
            undo_path.to_str().unwrap(),
            "/home/user/.local/share/reovim/undo/%2Fhome%2Fuser%2Fproject%2Fmain.rs.undo"
        );
    }

    #[test]
    fn test_undo_dir() {
        let persistence = UndoPersistence::new(Path::new("/data"));
        assert_eq!(persistence.undo_dir(), Path::new("/data/undo"));
    }
}
