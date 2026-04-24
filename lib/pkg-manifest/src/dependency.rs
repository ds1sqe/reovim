//! `[dependencies]` table entries.
//!
//! A dependency value is either a bare version string
//! (`vim-core = "1.0"`) or an inline table
//! (`vim-text = { version = "1.0", features = ["ripgrep"] }`). Both
//! forms round-trip through [`Dependency`].

use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A `[dependencies]` value.
///
/// The two variants mirror cargo's inline-string vs inline-table
/// forms on disk. [`Dependency::into_detailed`] normalizes both to
/// the uniform [`DetailedDep`] shape that downstream phases (resolver,
/// install) consume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dependency {
    /// Bare version string (`"1.0"`).
    Version(String),
    /// Inline table with version, path, and feature list.
    Detailed(DetailedDep),
}

/// The detailed (inline-table) form of a dependency.
///
/// `version` and `path` are both optional because a path dependency
/// omits `version` and a registry dependency omits `path`. Exact
/// validation (exactly one of version / path present, etc.) happens
/// in Phase 1's resolver.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DetailedDep {
    /// Optional semver constraint (e.g. `"1.0"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Optional filesystem path for local-path dependencies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// Feature flags requested on this dependency.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

impl Dependency {
    /// Normalize to the detailed form.
    ///
    /// Lifts the bare-version sugar into a [`DetailedDep`] with
    /// `path = None` and `features = []`. The detailed variant is
    /// returned unchanged. Phase 1's resolver consumes this uniform
    /// shape without pattern-matching at every call site.
    #[must_use]
    pub fn into_detailed(self) -> DetailedDep {
        match self {
            Self::Version(v) => DetailedDep {
                version: Some(v),
                path: None,
                features: Vec::new(),
            },
            Self::Detailed(d) => d,
        }
    }
}

impl Serialize for Dependency {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Version(s) => serializer.serialize_str(s),
            Self::Detailed(d) => d.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Dependency {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Version(String),
            Detailed(DetailedDep),
        }
        match Raw::deserialize(deserializer)? {
            Raw::Version(s) => Ok(Self::Version(s)),
            Raw::Detailed(d) => Ok(Self::Detailed(d)),
        }
    }
}
