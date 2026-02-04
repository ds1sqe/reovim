//! File watching types.
//!
//! Linux equivalent: `inotify`, `fsnotify`

use std::path::PathBuf;

/// Unique identifier for a watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WatchId(u64);

impl WatchId {
    /// Create a new watch ID.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for WatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WatchId({})", self.0)
    }
}

/// Handle to a registered file watch.
///
/// Represents an active watch on a path. The watch remains active
/// until explicitly unwatched via `FileWatcher::unwatch()`.
#[derive(Debug)]
pub struct WatchHandle {
    id: WatchId,
    path: PathBuf,
}

impl WatchHandle {
    /// Create a new watch handle.
    #[must_use]
    pub const fn new(id: WatchId, path: PathBuf) -> Self {
        Self { id, path }
    }

    /// Get the watch ID.
    #[must_use]
    pub const fn id(&self) -> WatchId {
        self.id
    }

    /// Get the watched path.
    #[must_use]
    pub const fn path(&self) -> &PathBuf {
        &self.path
    }
}

/// File system event.
///
/// Emitted when watched files or directories change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    /// A new file or directory was created.
    Created(PathBuf),
    /// An existing file was modified.
    Modified(PathBuf),
    /// A file or directory was deleted.
    Deleted(PathBuf),
    /// A file or directory was renamed.
    Renamed {
        /// Original path.
        from: PathBuf,
        /// New path.
        to: PathBuf,
    },
    /// File metadata changed (permissions, timestamps).
    MetadataChanged(PathBuf),
    /// Watch error occurred.
    Error {
        /// Path that caused the error (if known).
        path: Option<PathBuf>,
        /// Error description.
        message: String,
    },
}

impl WatchEvent {
    /// Get the primary path affected by this event.
    #[must_use]
    pub const fn path(&self) -> Option<&PathBuf> {
        match self {
            Self::Created(p) | Self::Modified(p) | Self::Deleted(p) | Self::MetadataChanged(p) => {
                Some(p)
            }
            Self::Renamed { to, .. } => Some(to),
            Self::Error { path, .. } => path.as_ref(),
        }
    }

    /// Check if this is an error event.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    /// Check if this event indicates a file/directory was created.
    #[must_use]
    pub const fn is_created(&self) -> bool {
        matches!(self, Self::Created(_))
    }

    /// Check if this event indicates a file was modified.
    #[must_use]
    pub const fn is_modified(&self) -> bool {
        matches!(self, Self::Modified(_))
    }

    /// Check if this event indicates a file/directory was deleted.
    #[must_use]
    pub const fn is_deleted(&self) -> bool {
        matches!(self, Self::Deleted(_))
    }

    /// Check if this event indicates a rename.
    #[must_use]
    pub const fn is_renamed(&self) -> bool {
        matches!(self, Self::Renamed { .. })
    }

    /// Check if this event indicates metadata changed.
    #[must_use]
    pub const fn is_metadata_changed(&self) -> bool {
        matches!(self, Self::MetadataChanged(_))
    }
}

impl std::fmt::Display for WatchEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created(p) => write!(f, "created: {}", p.display()),
            Self::Modified(p) => write!(f, "modified: {}", p.display()),
            Self::Deleted(p) => write!(f, "deleted: {}", p.display()),
            Self::Renamed { from, to } => {
                write!(f, "renamed: {} -> {}", from.display(), to.display())
            }
            Self::MetadataChanged(p) => write!(f, "metadata changed: {}", p.display()),
            Self::Error { path, message } => {
                if let Some(p) = path {
                    write!(f, "error at {}: {message}", p.display())
                } else {
                    write!(f, "error: {message}")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_id() {
        let id = WatchId::new(42);
        assert_eq!(id.as_u64(), 42);
        assert_eq!(format!("{id}"), "WatchId(42)");
    }

    #[test]
    fn test_watch_id_equality() {
        let id1 = WatchId::new(1);
        let id2 = WatchId::new(1);
        let id3 = WatchId::new(2);
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_watch_handle() {
        let handle = WatchHandle::new(WatchId::new(1), PathBuf::from("/test"));
        assert_eq!(handle.id().as_u64(), 1);
        assert_eq!(handle.path(), &PathBuf::from("/test"));
    }

    #[test]
    fn test_watch_event_created() {
        let event = WatchEvent::Created(PathBuf::from("/new"));
        assert_eq!(event.path(), Some(&PathBuf::from("/new")));
        assert!(event.is_created());
        assert!(!event.is_modified());
        assert!(!event.is_error());
        assert_eq!(format!("{event}"), "created: /new");
    }

    #[test]
    fn test_watch_event_modified() {
        let event = WatchEvent::Modified(PathBuf::from("/file"));
        assert!(event.is_modified());
        assert_eq!(format!("{event}"), "modified: /file");
    }

    #[test]
    fn test_watch_event_deleted() {
        let event = WatchEvent::Deleted(PathBuf::from("/old"));
        assert!(event.is_deleted());
        assert_eq!(format!("{event}"), "deleted: /old");
    }

    #[test]
    fn test_watch_event_renamed() {
        let event = WatchEvent::Renamed {
            from: PathBuf::from("/old"),
            to: PathBuf::from("/new"),
        };
        assert_eq!(event.path(), Some(&PathBuf::from("/new")));
        assert!(event.is_renamed());
        assert_eq!(format!("{event}"), "renamed: /old -> /new");
    }

    #[test]
    fn test_watch_event_metadata_changed() {
        let event = WatchEvent::MetadataChanged(PathBuf::from("/file"));
        assert!(event.is_metadata_changed());
        assert_eq!(format!("{event}"), "metadata changed: /file");
    }

    #[test]
    fn test_watch_event_error_with_path() {
        let event = WatchEvent::Error {
            path: Some(PathBuf::from("/bad")),
            message: "failed".to_string(),
        };
        assert!(event.is_error());
        assert_eq!(event.path(), Some(&PathBuf::from("/bad")));
        assert_eq!(format!("{event}"), "error at /bad: failed");
    }

    #[test]
    fn test_watch_event_error_without_path() {
        let event = WatchEvent::Error {
            path: None,
            message: "unknown error".to_string(),
        };
        assert!(event.is_error());
        assert_eq!(event.path(), None);
        assert_eq!(format!("{event}"), "error: unknown error");
    }
}
