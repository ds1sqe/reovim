//! [`Lockfile`] → `pkg.lock` serialization.
//!
//! The writer sorts `packages` by `(name, version)` before emission
//! so serialized output is deterministic regardless of the input
//! order held in memory.

use serde::Serialize;

use crate::{Lockfile, LockfileError, PackageLock};

#[derive(Serialize)]
struct RawLockfileOut<'a> {
    version: u32,
    #[serde(rename = "package", skip_serializing_if = "Vec::is_empty")]
    packages: Vec<&'a PackageLock>,
}

impl Lockfile {
    /// Serialize this lockfile to a TOML string.
    ///
    /// Output is deterministic: packages are sorted by `(name,
    /// version)` before emission, and every non-trivial field is
    /// backed by an ordered or intrinsically-sorted container.
    ///
    /// # Errors
    ///
    /// Returns [`LockfileError::TomlSer`] if the TOML serializer
    /// rejects the value.
    pub fn to_toml_string(&self) -> Result<String, LockfileError> {
        let mut sorted: Vec<&PackageLock> = self.packages.iter().collect();
        sorted.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.version.cmp(&b.version)));
        let out = RawLockfileOut {
            version: self.version,
            packages: sorted,
        };
        Ok(toml::to_string(&out)?)
    }
}
