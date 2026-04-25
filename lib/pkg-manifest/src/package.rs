//! `[package]` table: manifest identity and reovim version constraint.

use serde::{Deserialize, Serialize};

use crate::kind::PackageKind;

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
    /// that pins an exact version. The resolver enforces the
    /// distinction; the parser accepts either shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Classification that selects the `$REOVIM_LIBRARY_ROOT`
    /// subdir for this package's cdylib.
    ///
    /// Optional with the same distinction as [`Self::version`]:
    /// root user-config manifests may omit it (a setup is not a
    /// cdylib), but path-dep manifests must set it before they can
    /// be installed. The installer enforces the distinction; the
    /// parser accepts either shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<PackageKind>,
    /// Semver constraint on the reovim host version (e.g. `"^0.15"`).
    ///
    /// Carried as a string; the resolver parses it into a
    /// `semver::VersionReq`.
    #[serde(rename = "reovim-version")]
    pub reovim_version: String,
}
