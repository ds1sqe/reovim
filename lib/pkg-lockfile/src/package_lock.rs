//! `[[package]]` entry types.

use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A resolved package entry in the lockfile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageLock {
    /// Package name (matches the manifest dependency name).
    pub name: String,
    /// Exact resolved version (no semver ranges; lockfiles pin).
    pub version: String,
    /// Where the package came from on disk (or, in future, from a
    /// registry).
    pub source: Source,
    /// rustc target triple the cdylib was built for (e.g.
    /// `"x86_64-unknown-linux-gnu"`).
    ///
    /// `None` in fixtures that pre-date the install step. Phase 2's
    /// install populates it so the lockfile distinguishes artifacts
    /// across platforms. Declared in Phase 0 to lock the wire shape
    /// before Phase 2 lands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// SHA-256 hex digest of the cdylib file.
    ///
    /// `None` until Phase 2 install computes it. When set, must be a
    /// 64-character lowercase hex string; the parser validates this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Names of the resolved dependencies of this package.
    ///
    /// Version-free: resolution is exact by the time a lockfile is
    /// written, so the name alone identifies the peer entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
}

/// Where a resolved package was obtained from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Local filesystem path. Phase 0 fixtures use only this variant.
    LocalPath(PathBuf),
    /// Remote registry URL. Reserved: no Phase 0 code path constructs
    /// this variant; it is declared now to lock the wire shape before
    /// a future epic adds a registry.
    Registry {
        /// Registry URL (e.g. `"https://registry.reovim.dev"`).
        url: String,
    },
}

impl Serialize for Source {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let raw = match self {
            Self::LocalPath(path) => RawSource {
                kind: SourceKind::LocalPath,
                path: Some(path.clone()),
                url: None,
            },
            Self::Registry { url } => RawSource {
                kind: SourceKind::Registry,
                path: None,
                url: Some(url.clone()),
            },
        };
        raw.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Source {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawSource::deserialize(deserializer)?;
        match (raw.kind, raw.path, raw.url) {
            (SourceKind::LocalPath, Some(path), None) => Ok(Self::LocalPath(path)),
            (SourceKind::Registry, None, Some(url)) => Ok(Self::Registry { url }),
            (SourceKind::LocalPath, _, _) => Err(serde::de::Error::custom(
                "source.kind = \"local-path\" requires `path` and no `url`",
            )),
            (SourceKind::Registry, _, _) => Err(serde::de::Error::custom(
                "source.kind = \"registry\" requires `url` and no `path`",
            )),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct RawSource {
    kind: SourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SourceKind {
    LocalPath,
    Registry,
}
