//! `pkg.lock` → [`Lockfile`] deserialization.

use serde::Deserialize;

use crate::{Lockfile, LockfileError, PackageLock};

#[derive(Deserialize)]
struct RawLockfile {
    version: u32,
    #[serde(default, rename = "package")]
    packages: Vec<PackageLock>,
}

impl Lockfile {
    /// Parse a `pkg.lock` lockfile from TOML source.
    ///
    /// # Errors
    ///
    /// Returns [`LockfileError::TomlDe`] for malformed TOML or a
    /// schema mismatch, and [`LockfileError::InvalidSha256`] for any
    /// `[[package]]` with a `sha256` that is not a 64-character
    /// lowercase hex string.
    pub fn from_toml_str(s: &str) -> Result<Self, LockfileError> {
        let raw: RawLockfile = toml::from_str(s)?;
        for pkg in &raw.packages {
            if let Some(sha) = &pkg.sha256
                && !is_valid_sha256(sha)
            {
                return Err(LockfileError::InvalidSha256 {
                    name: pkg.name.clone(),
                    value: sha.clone(),
                });
            }
        }
        Ok(Self {
            version: raw.version,
            packages: raw.packages,
        })
    }
}

fn is_valid_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
