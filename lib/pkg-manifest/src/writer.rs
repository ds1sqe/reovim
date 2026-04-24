//! [`Manifest`] → `pkg.toml` serialization.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::{Dependency, Manifest, ManifestError, Package, lazy::RawLazy};

#[derive(Serialize)]
struct RawManifestOut<'a> {
    package: &'a Package,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    dependencies: &'a BTreeMap<String, Dependency>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    lazy: BTreeMap<String, RawLazy>,
}

impl Manifest {
    /// Serialize this manifest to a TOML string.
    ///
    /// Output is deterministic: [`Self::dependencies`] and
    /// [`Self::lazy`] are [`BTreeMap`]-backed so key order is stable
    /// across runs.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::TomlSer`] if the TOML serializer
    /// rejects the value.
    pub fn to_toml_string(&self) -> Result<String, ManifestError> {
        let lazy: BTreeMap<String, RawLazy> = self
            .lazy
            .iter()
            .map(|(k, v)| (k.clone(), v.into()))
            .collect();
        let out = RawManifestOut {
            package: &self.package,
            dependencies: &self.dependencies,
            lazy,
        };
        Ok(toml::to_string(&out)?)
    }
}
