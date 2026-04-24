//! Graph walk + conflict detection.
//!
//! [`resolve`] is the public entry point. It loads the root manifest
//! from `root_dir`, walks every local-path dependency recursively,
//! validates version constraints, and emits a deterministic
//! [`Resolved`] value. Errors surface as [`ResolveError`]; each variant
//! names every piece of context a user needs to locate the problem.
//!
//! The root manifest is a user-config manifest (a "setup") and does
//! not appear in [`Resolved`]; only transitively-reachable path deps
//! become lockfile entries. A path dep's own `[package].version` is
//! required (the root's is not) so the lockfile can pin an exact
//! version.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use {
    reovim_pkg_lockfile::Source,
    reovim_pkg_manifest::{Dependency, DetailedDep, ManifestError},
    semver::{Version, VersionReq},
};

use crate::{
    constraint::{check, parse_constraint, parse_version},
    loader::{LoadedManifest, load},
};

/// The flat, deterministic output of the resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// Resolved packages sorted by `(name, version)`.
    pub packages: Vec<ResolvedPackage>,
}

/// A single resolved package entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPackage {
    /// Package name as declared in its own `[package].name`.
    pub name: String,
    /// Exact version parsed from the package's `[package].version`.
    pub version: Version,
    /// Where the package was found.
    pub source: Source,
    /// Names of resolved direct dependencies, sorted ascending.
    pub dependencies: Vec<String>,
}

/// Resolver error.
///
/// Every variant carries enough context for a plain `eprintln!("{err}")`
/// to locate the faulty file or dependency edge without further lookup.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    /// The manifest directory (or the `pkg.toml` inside it) was not
    /// found or could not be read.
    #[error("manifest not found at `{at}`", at = .at.display())]
    ManifestNotFound {
        /// The directory or `pkg.toml` path that was probed.
        at: PathBuf,
    },
    /// The manifest was found but failed to parse.
    #[error("failed to parse manifest at `{at}`: {source}", at = .at.display())]
    ManifestParse {
        /// Path to the offending manifest file.
        at: PathBuf,
        /// Underlying manifest parse error.
        #[source]
        source: ManifestError,
    },
    /// A path-dep manifest is missing the `[package].version` field.
    ///
    /// Only required on manifests reached through a path dependency;
    /// the root user-config manifest may omit it.
    #[error("path-dep manifest `{pkg}` at `{at}` is missing `[package].version`", at = .at.display())]
    MissingPackageVersion {
        /// Package name declared by the offending manifest.
        pkg: String,
        /// Path to the offending manifest file.
        at: PathBuf,
    },
    /// A package declared a version string that is not valid semver.
    #[error("invalid version `{raw}` declared by `{pkg}`: {source}")]
    InvalidVersion {
        /// Package whose `[package].version` is invalid.
        pkg: String,
        /// The raw string that failed to parse.
        raw: String,
        /// Underlying semver error.
        #[source]
        source: semver::Error,
    },
    /// A dependency constraint is not a valid semver range.
    #[error("invalid constraint `{raw}` on `{pkg}` declared by `{requirer}`: {source}")]
    InvalidConstraint {
        /// Dependency name the constraint targets.
        pkg: String,
        /// Manifest package that declared the constraint.
        requirer: String,
        /// The raw constraint string.
        raw: String,
        /// Underlying semver error.
        #[source]
        source: semver::Error,
    },
    /// A dependency's declared constraint does not accept the
    /// actual version on disk.
    #[error(
        "version mismatch for `{pkg}`: `{requirer}` requires `{required}` but on-disk version is `{actual}`"
    )]
    VersionMismatch {
        /// Dependency whose actual version fails the constraint.
        pkg: String,
        /// Manifest package that declared the failing constraint.
        requirer: String,
        /// The constraint string that rejected the actual version.
        required: String,
        /// The actual version on disk.
        actual: String,
    },
    /// A cycle was detected in the dependency graph. `chain` lists
    /// package names in DFS-visit order from the cycle root to the
    /// offender.
    #[error("dependency cycle detected: {}", .chain.join(" -> "))]
    Cycle {
        /// Package names along the cycle, ancestor first, offender last.
        chain: Vec<String>,
    },
    /// A dependency cannot be resolved in Phase 1 because no
    /// local-path source was declared.
    #[error("cannot resolve `{pkg}` required by `{requirer}`: {reason}")]
    UnresolvableDependency {
        /// Dependency that could not be resolved.
        pkg: String,
        /// Manifest package that declared the dependency.
        requirer: String,
        /// Why the resolver cannot handle it.
        reason: &'static str,
    },
    /// The root manifest requires a reovim version that does not
    /// accept the runtime version passed to [`resolve`].
    #[error(
        "incompatible reovim version: root manifest requires `{required}` but runtime is `{actual}`"
    )]
    IncompatibleReovimVersion {
        /// The constraint from the root manifest.
        required: String,
        /// The runtime reovim version.
        actual: String,
    },
    /// Two dependency edges reached the same package name through
    /// different canonical paths.
    #[error(
        "duplicate package `{pkg}`: reached via `{a}` and `{b}`",
        a = .path_a.display(),
        b = .path_b.display(),
    )]
    DuplicatePackage {
        /// Package name reached twice.
        pkg: String,
        /// First canonical path encountered.
        path_a: PathBuf,
        /// Second canonical path encountered.
        path_b: PathBuf,
    },
}

/// Resolve the root manifest at `root_dir` against its local-path
/// dependency tree.
///
/// `reovim_runtime_version` is the version the runtime reports; the
/// root manifest's `[package].reovim-version` constraint is validated
/// against it. Transitive path-dep manifests' `reovim-version` fields
/// are intentionally NOT re-checked — the runtime version is a host
/// property, not a per-manifest property.
///
/// The returned [`Resolved`] lists only the transitively-reachable
/// path deps. The root manifest itself is a user setup and never
/// appears in the lockfile.
///
/// # Errors
///
/// Returns a [`ResolveError`] for any parse, walk, or validation
/// failure. See the enum for the exhaustive list.
pub fn resolve(
    root_dir: &Path,
    reovim_runtime_version: &Version,
) -> Result<Resolved, ResolveError> {
    let root = load(root_dir)?;
    check_reovim_version(&root, reovim_runtime_version)?;

    let mut walker = Walker::new();
    walker.walk_root(&root)?;
    Ok(walker.into_resolved())
}

fn check_reovim_version(root: &LoadedManifest, runtime: &Version) -> Result<(), ResolveError> {
    let raw = &root.manifest.package.reovim_version;
    let req = VersionReq::parse(raw).map_err(|source| ResolveError::InvalidConstraint {
        pkg: root.manifest.package.name.clone(),
        requirer: root.manifest.package.name.clone(),
        raw: raw.clone(),
        source,
    })?;
    if req.matches(runtime) {
        Ok(())
    } else {
        Err(ResolveError::IncompatibleReovimVersion {
            required: raw.clone(),
            actual: runtime.to_string(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Color {
    Gray,
    Black,
}

struct Walker {
    accum: BTreeMap<String, Entry>,
    stack: Vec<String>,
    color: BTreeMap<PathBuf, Color>,
}

struct Entry {
    name: String,
    version: Version,
    source: Source,
    canonical_path: PathBuf,
    dependencies: Vec<String>,
}

impl Walker {
    const fn new() -> Self {
        Self {
            accum: BTreeMap::new(),
            stack: Vec::new(),
            color: BTreeMap::new(),
        }
    }

    /// Walk the root manifest's direct dependencies. The root itself
    /// is not added to `accum`.
    fn walk_root(&mut self, root: &LoadedManifest) -> Result<(), ResolveError> {
        let requirer = root.manifest.package.name.clone();
        self.color.insert(root.canonical_path.clone(), Color::Gray);
        self.stack.push(requirer.clone());

        for (dep_name, dep) in collect_deps(&root.manifest) {
            self.visit_dep(&root.canonical_path, &requirer, &dep_name, &dep)?;
        }

        self.color.insert(root.canonical_path.clone(), Color::Black);
        self.stack.pop();
        Ok(())
    }

    fn visit_dep(
        &mut self,
        base: &Path,
        requirer: &str,
        dep_name: &str,
        dep: &Dependency,
    ) -> Result<(), ResolveError> {
        let detailed = dep.clone().into_detailed();
        let dep_path = require_path(requirer, dep_name, &detailed)?;
        let dep_dir = resolve_relative(base, &dep_path);
        let loaded = load(&dep_dir)?;

        if matches!(self.color.get(&loaded.canonical_path), Some(Color::Gray)) {
            let mut chain = self.stack.clone();
            chain.push(loaded.manifest.package.name.clone());
            return Err(ResolveError::Cycle { chain });
        }

        let name = loaded.manifest.package.name.clone();

        if let Some(existing) = self.accum.get(&name) {
            if existing.canonical_path != loaded.canonical_path {
                return Err(ResolveError::DuplicatePackage {
                    pkg: name,
                    path_a: existing.canonical_path.clone(),
                    path_b: loaded.canonical_path.clone(),
                });
            }
            if let Some(raw) = &detailed.version {
                let req = parse_constraint(dep_name, requirer, raw)?;
                check(dep_name, requirer, &existing.version, &req)?;
            }
            return Ok(());
        }

        let raw_version = loaded.manifest.package.version.as_ref().ok_or_else(|| {
            ResolveError::MissingPackageVersion {
                pkg: name.clone(),
                at: loaded.manifest_path.clone(),
            }
        })?;
        let actual = parse_version(&name, raw_version)?;

        if let Some(raw) = &detailed.version {
            let req = parse_constraint(dep_name, requirer, raw)?;
            check(dep_name, requirer, &actual, &req)?;
        }

        self.color
            .insert(loaded.canonical_path.clone(), Color::Gray);
        self.stack.push(name.clone());

        let mut dep_names: Vec<String> = Vec::new();
        for (child_name, child_dep) in collect_deps(&loaded.manifest) {
            self.visit_dep(&loaded.canonical_path, &name, &child_name, &child_dep)?;
            dep_names.push(child_name);
        }
        // `collect_deps` reads a BTreeMap so names are already unique and
        // sorted; no need to sort or dedup.

        let entry = Entry {
            name: name.clone(),
            version: actual,
            source: Source::LocalPath(loaded.canonical_path.clone()),
            canonical_path: loaded.canonical_path.clone(),
            dependencies: dep_names,
        };
        self.accum.insert(name.clone(), entry);

        self.color
            .insert(loaded.canonical_path.clone(), Color::Black);
        self.stack.pop();
        Ok(())
    }

    fn into_resolved(self) -> Resolved {
        let mut packages: Vec<ResolvedPackage> = self
            .accum
            .into_values()
            .map(|e| ResolvedPackage {
                name: e.name,
                version: e.version,
                source: e.source,
                dependencies: e.dependencies,
            })
            .collect();
        // Names are unique because `accum` keys by package name; sorting
        // by name is sufficient for deterministic output.
        packages.sort_by(|a, b| a.name.cmp(&b.name));
        Resolved { packages }
    }
}

fn require_path(
    requirer: &str,
    dep_name: &str,
    detailed: &DetailedDep,
) -> Result<PathBuf, ResolveError> {
    detailed
        .path
        .clone()
        .ok_or_else(|| ResolveError::UnresolvableDependency {
            pkg: dep_name.to_string(),
            requirer: requirer.to_string(),
            reason: "Phase 1 supports only path-based dependencies (set `path = ...`)",
        })
}

fn resolve_relative(base: &Path, candidate: &Path) -> PathBuf {
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base.join(candidate)
    }
}

fn collect_deps(m: &reovim_pkg_manifest::Manifest) -> Vec<(String, Dependency)> {
    m.dependencies
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod graph_tests;
