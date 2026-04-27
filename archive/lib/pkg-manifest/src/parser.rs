//! `pkg.toml` → [`Manifest`] deserialization.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{Dependency, Manifest, ManifestError, Package, lazy::RawLazy};

#[derive(Deserialize)]
struct RawManifest {
    package: Package,
    #[serde(default)]
    dependencies: BTreeMap<String, Dependency>,
    #[serde(default)]
    lazy: BTreeMap<String, RawLazy>,
}

impl Manifest {
    /// Parse a `pkg.toml` manifest from TOML source.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::TomlDe`] for malformed TOML or a
    /// schema mismatch, and [`ManifestError::InvalidLazyTrigger`] for
    /// a `[lazy.<name>]` entry without exactly one trigger field.
    pub fn from_toml_str(s: &str) -> Result<Self, ManifestError> {
        let raw: RawManifest = toml::from_str(s)?;
        let mut lazy = BTreeMap::new();
        for (name, entry) in raw.lazy {
            let trigger = entry.into_trigger(&name)?;
            lazy.insert(name, trigger);
        }
        Ok(Self {
            package: raw.package,
            dependencies: raw.dependencies,
            lazy,
        })
    }
}
