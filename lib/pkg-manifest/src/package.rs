//! `[package]` table: manifest identity and reovim version constraint.

use serde::{Deserialize, Serialize};

/// The `[package]` table of a `pkg.toml` manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    /// Manifest name; a local identifier for the reovim setup.
    pub name: String,
    /// Package's own semver version (e.g. `"1.0.0"`).
    ///
    /// Optional on a root user-config manifest (a setup has no
    /// semantic version). Required on any manifest reached as a
    /// path dependency so the resolver can write a lockfile entry
    /// that pins an exact version. The resolver (#771 Phase 1) is
    /// what enforces the distinction; the parser accepts either
    /// shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Semver constraint on the reovim host version (e.g. `"^0.15"`).
    ///
    /// Carried as a string in Phase 0; Phase 1's resolver parses it
    /// into a `semver::VersionReq`.
    #[serde(rename = "reovim-version")]
    pub reovim_version: String,
}
