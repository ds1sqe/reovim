//! Semver version range constraints (#619).
//!
//! Provides Cargo-style version matching for module dependency declarations.
//! Uses `(u32, u32, u32)` tuples to keep the depgraph crate zero-dependency.
//!
//! # Supported syntax
//!
//! | Syntax | Meaning |
//! |--------|---------|
//! | `^1.2.3` | Compatible: `>=1.2.3, <2.0.0` |
//! | `^0.2.3` | Compatible (pre-1.0): `>=0.2.3, <0.3.0` |
//! | `^0.0.3` | Compatible (pre-0.1): `>=0.0.3, <0.0.4` |
//! | `=1.2.3` | Exact: only `1.2.3` |
//! | `>=1.2.3` | At least: `>=1.2.3` |
//! | `1.2.3` | Shorthand for `^1.2.3` |

use std::fmt;

/// Semver version tuple: `(major, minor, patch)`.
pub type SemVer = (u32, u32, u32);

/// A version range constraint following Cargo semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionRange {
    /// Compatible range (`^X.Y.Z` or bare `X.Y.Z`).
    ///
    /// - `^1.2.3` matches `>=1.2.3, <2.0.0`
    /// - `^0.2.3` matches `>=0.2.3, <0.3.0`
    /// - `^0.0.3` matches `>=0.0.3, <0.0.4`
    Compatible(SemVer),

    /// Exact match (`=X.Y.Z`).
    Exact(SemVer),

    /// At-least constraint (`>=X.Y.Z`).
    AtLeast(SemVer),
}

impl VersionRange {
    /// Check if a version satisfies this range.
    #[must_use]
    pub fn satisfies(&self, version: SemVer) -> bool {
        match self {
            Self::Compatible(base) => satisfies_compatible(version, *base),
            Self::Exact(exact) => version == *exact,
            Self::AtLeast(min) => version >= *min,
        }
    }

    /// Parse a version range string.
    ///
    /// Returns `None` for invalid syntax.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        if let Some(rest) = s.strip_prefix(">=") {
            let v = parse_version(rest.trim())?;
            Some(Self::AtLeast(v))
        } else if let Some(rest) = s.strip_prefix('^') {
            let v = parse_version(rest.trim())?;
            Some(Self::Compatible(v))
        } else if let Some(rest) = s.strip_prefix('=') {
            let v = parse_version(rest.trim())?;
            Some(Self::Exact(v))
        } else {
            // Bare version string: treat as compatible
            let v = parse_version(s)?;
            Some(Self::Compatible(v))
        }
    }
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compatible((major, minor, patch)) => write!(f, "^{major}.{minor}.{patch}"),
            Self::Exact((major, minor, patch)) => write!(f, "={major}.{minor}.{patch}"),
            Self::AtLeast((major, minor, patch)) => write!(f, ">={major}.{minor}.{patch}"),
        }
    }
}

/// A version constraint: target module + required version range.
#[derive(Debug, Clone)]
pub struct VersionConstraint<K> {
    /// The module this constraint applies to.
    pub target: K,
    /// The version range required.
    pub range: VersionRange,
}

/// A constraint violation found during checking.
#[derive(Debug, Clone)]
pub struct ConstraintViolation<K> {
    /// Module that declared the constraint.
    pub source: K,
    /// Module that failed the constraint.
    pub target: K,
    /// The range that was required.
    pub required: VersionRange,
    /// The actual version found.
    pub actual: SemVer,
}

impl<K: fmt::Debug> fmt::Display for ConstraintViolation<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} requires {:?} {}, but found {}.{}.{}",
            self.source, self.target, self.required, self.actual.0, self.actual.1, self.actual.2
        )
    }
}

/// Check version constraints against resolved module versions.
///
/// `constraints` is a list of `(source_key, target_key, range_string)`.
/// `versions` maps each key to its version tuple.
///
/// Returns all violations found. Empty vec = all constraints satisfied.
pub fn check_version_constraints<K>(
    constraints: &[(K, K, &str)],
    versions: &[(K, SemVer)],
) -> Vec<ConstraintViolation<K>>
where
    K: Eq + std::hash::Hash + Clone + fmt::Debug,
{
    use std::collections::HashMap;

    let version_map: HashMap<&K, SemVer> = versions.iter().map(|(k, v)| (k, *v)).collect();

    let mut violations = Vec::new();

    for (source, target, range_str) in constraints {
        let Some(range) = VersionRange::parse(range_str) else {
            // Malformed constraint string — skip silently (logged by caller)
            continue;
        };

        if let Some(&actual) = version_map.get(target)
            && !range.satisfies(actual)
        {
            violations.push(ConstraintViolation {
                source: source.clone(),
                target: target.clone(),
                required: range,
                actual,
            });
        }
        // If the target isn't in the version map, it's missing entirely —
        // that's caught by the dependency resolver, not the constraint checker.
    }

    violations
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Parse "X.Y.Z" into `(u32, u32, u32)`.
fn parse_version(s: &str) -> Option<SemVer> {
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    // Reject trailing parts (e.g., "1.2.3.4")
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// Check if `version` is in the compatible range of `base` (Cargo ^ semantics).
fn satisfies_compatible(version: SemVer, base: SemVer) -> bool {
    // Must be >= base
    if version < base {
        return false;
    }

    let (major, minor, _patch) = base;

    if major > 0 {
        // ^1.2.3 → same major
        version.0 == major
    } else if minor > 0 {
        // ^0.2.3 → same major.minor
        version.0 == 0 && version.1 == minor
    } else {
        // ^0.0.3 → exact match on all three
        version == base
    }
}

#[cfg(test)]
#[path = "version_tests.rs"]
mod tests;
