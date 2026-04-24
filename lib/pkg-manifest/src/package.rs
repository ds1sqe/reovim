//! `[package]` table: manifest identity and reovim version constraint.

use serde::{Deserialize, Serialize};

/// The `[package]` table of a `pkg.toml` manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    /// Manifest name; a local identifier for the reovim setup.
    pub name: String,
    /// Semver constraint on the reovim host version (e.g. `"^0.15"`).
    ///
    /// Carried as a string in Phase 0; Phase 1's resolver parses it
    /// into a `semver::VersionReq`.
    #[serde(rename = "reovim-version")]
    pub reovim_version: String,
}
