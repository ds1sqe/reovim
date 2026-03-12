//! Module source types — where a third-party module comes from.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Where a third-party module is sourced from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ModuleSource {
    /// Git repository (cloned and built locally).
    #[serde(rename = "git")]
    Git {
        /// Repository URL (HTTPS or SSH).
        url: String,
        /// Optional branch, tag, or commit to pin.
        #[serde(default)]
        rev: Option<String>,
    },
    /// Local filesystem path (for development).
    #[serde(rename = "path")]
    Path {
        /// Absolute or relative path to the module crate root.
        path: String,
    },
}

impl ModuleSource {
    /// Create a git source with no specific revision.
    #[must_use]
    pub fn git(url: impl Into<String>) -> Self {
        Self::Git {
            url: url.into(),
            rev: None,
        }
    }

    /// Create a git source pinned to a revision.
    #[must_use]
    pub fn git_rev(url: impl Into<String>, rev: impl Into<String>) -> Self {
        Self::Git {
            url: url.into(),
            rev: Some(rev.into()),
        }
    }

    /// Create a local path source.
    #[must_use]
    pub fn path(path: impl Into<String>) -> Self {
        Self::Path { path: path.into() }
    }

    /// Check if this is a git source.
    #[must_use]
    pub const fn is_git(&self) -> bool {
        matches!(self, Self::Git { .. })
    }

    /// Check if this is a local path source.
    #[must_use]
    pub const fn is_path(&self) -> bool {
        matches!(self, Self::Path { .. })
    }
}

impl fmt::Display for ModuleSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Git { url, rev: None } => write!(f, "git({url})"),
            Self::Git {
                url,
                rev: Some(rev),
            } => write!(f, "git({url}@{rev})"),
            Self::Path { path } => write!(f, "path({path})"),
        }
    }
}
