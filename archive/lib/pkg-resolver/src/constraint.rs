//! Thin wrappers around [`semver::Version`] / [`semver::VersionReq`]
//! that preserve the offending package, requirer, and raw string so
//! [`crate::ResolveError`] messages can be shown to a user without
//! further lookup.

use semver::{Version, VersionReq};

use crate::graph::ResolveError;

pub fn parse_version(pkg: &str, raw: &str) -> Result<Version, ResolveError> {
    Version::parse(raw).map_err(|source| ResolveError::InvalidVersion {
        pkg: pkg.to_string(),
        raw: raw.to_string(),
        source,
    })
}

pub fn parse_constraint(pkg: &str, requirer: &str, raw: &str) -> Result<VersionReq, ResolveError> {
    VersionReq::parse(raw).map_err(|source| ResolveError::InvalidConstraint {
        pkg: pkg.to_string(),
        requirer: requirer.to_string(),
        raw: raw.to_string(),
        source,
    })
}

pub fn check(
    pkg: &str,
    requirer: &str,
    actual: &Version,
    req: &VersionReq,
) -> Result<(), ResolveError> {
    if req.matches(actual) {
        Ok(())
    } else {
        Err(ResolveError::VersionMismatch {
            pkg: pkg.to_string(),
            requirer: requirer.to_string(),
            required: req.to_string(),
            actual: actual.to_string(),
        })
    }
}

#[cfg(test)]
#[path = "constraint_tests.rs"]
mod constraint_tests;
