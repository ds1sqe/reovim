//! `[package].kind` — classification of a reovim package as a driver
//! or a module.
//!
//! The variant selects which `$REOVIM_LIBRARY_ROOT` subdirectory a
//! cdylib lands in (the convention is owned by
//! [`reovim_dylib_loader::Kind`]). The field is optional on the
//! schema: a root user-config manifest omits it because a setup is
//! not itself a cdylib. Every path-dep manifest reached during
//! install must set it, otherwise the installer errors with a
//! dedicated variant.

use serde::{Deserialize, Serialize};

/// Which `$REOVIM_LIBRARY_ROOT` subdir a package's cdylib belongs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageKind {
    /// Client driver. Installed into `$REOVIM_LIBRARY_ROOT/driver/`.
    Driver,
    /// Server or client module. Installed into
    /// `$REOVIM_LIBRARY_ROOT/modules/`.
    Module,
}

impl PackageKind {
    /// Lowercase string form (`"driver"` or `"module"`). Matches the
    /// TOML-serialized shape and is the single source of truth for
    /// the kind-to-string codec used across the package-manager
    /// stack.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Driver => "driver",
            Self::Module => "module",
        }
    }
}
