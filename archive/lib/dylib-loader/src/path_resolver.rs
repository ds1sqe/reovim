//! Six-rule search-path resolver for reovim cdylibs.
//!
//! Rules, highest to lowest precedence (MERGE, not replace):
//!
//! | R  | Source                                     | Applies to |
//! |----|--------------------------------------------|------------|
//! | R1 | CLI `--driver <path>` / `-d`               | driver     |
//! | R2 | CLI `--module <path>` / `-m` / `--lib`     | module     |
//! | R3 | env `REOVIM_DRIVER_PATH` (colon-sep)       | driver     |
//! | R4 | env `REOVIM_MODULE_PATH` (colon-sep)       | module     |
//! | R5 | env `REOVIM_LIBRARY_ROOT/<kind>/`          | both       |
//! | R6 | XDG data + system fallback                 | both       |
//!
//! A path in a higher-precedence rule is searched first but does not
//! remove lower-precedence entries. Empty colon-separated segments are
//! filtered; an unset env var contributes zero entries.
//!
//! The shell-`PATH` analogy is the intended mental model: `-d /tmp/x`
//! means *also* look in `/tmp/x`, not *only* there. A replace-mode
//! builder flag exists internally for possible future use but is not
//! exposed in the public API at Phase 1.
//!
//! [`EnvProvider`] is the test seam: tests inject a mock env map
//! instead of touching `std::env`, so the 12-row rule matrix stays
//! hermetic.

use std::{path::PathBuf, sync::Arc};

/// Which flavor of cdylib the resolver is searching for.
///
/// `Driver` and `Module` route to distinct env vars (R3/R4), distinct
/// subdirectories under `REOVIM_LIBRARY_ROOT` (R5), and distinct XDG +
/// system fallbacks (R6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Client driver — `driver/` subdir convention.
    Driver,
    /// Server or client module — `modules/` subdir convention.
    Module,
}

impl Kind {
    /// Subdirectory convention for this kind under
    /// `REOVIM_LIBRARY_ROOT` and under XDG / system data roots.
    #[must_use]
    pub const fn subdir(self) -> &'static str {
        match self {
            Self::Driver => "driver",
            Self::Module => "modules",
        }
    }

    /// Env var consulted for colon-separated path overrides (R3/R4).
    #[must_use]
    pub const fn path_env_var(self) -> &'static str {
        match self {
            Self::Driver => "REOVIM_DRIVER_PATH",
            Self::Module => "REOVIM_MODULE_PATH",
        }
    }
}

impl std::str::FromStr for Kind {
    type Err = UnknownKind;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "driver" => Ok(Self::Driver),
            "module" => Ok(Self::Module),
            other => Err(UnknownKind(other.to_string())),
        }
    }
}

/// String could not be parsed as a [`Kind`].
#[derive(Debug, thiserror::Error)]
#[error("unknown package kind `{0}`: expected `driver` or `module`")]
pub struct UnknownKind(String);

/// Lookup seam for `std::env::var` so tests can inject a mock map.
///
/// Every method takes `&self` so implementations can be cheaply
/// shared behind `Arc` — the resolver clones the provider once per
/// rule lookup.
pub trait EnvProvider: Send + Sync {
    /// Return the value of `key`, or `None` if unset or non-utf8.
    fn get(&self, key: &str) -> Option<String>;
}

/// Reads `std::env::var` directly. Default provider when none is
/// injected.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdEnv;

impl EnvProvider for StdEnv {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

/// Split a colon-separated path string into entries, filtering empty
/// segments (so `"a::b"` yields `["a", "b"]` and `""` yields `[]`).
#[must_use]
pub fn split_paths(s: &str) -> Vec<PathBuf> {
    s.split(':')
        .filter(|seg| !seg.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Default XDG + system fallback for a given [`Kind`] (rule R6).
///
/// Honors `$XDG_DATA_HOME` if set, else uses `$HOME/.local/share` per
/// the XDG basedir spec. Always appends `/usr/local/lib/reovim/<kind>`
/// and `/usr/lib/reovim/<kind>` after the user-scoped entry.
fn default_fallback_paths<E: EnvProvider + ?Sized>(env: &E, kind: Kind) -> Vec<PathBuf> {
    let mut out = Vec::new();

    let user_root = env.get("XDG_DATA_HOME").map(PathBuf::from).or_else(|| {
        env.get("HOME")
            .map(|h| PathBuf::from(h).join(".local/share"))
    });

    if let Some(root) = user_root {
        out.push(root.join("reovim").join(kind.subdir()));
    }

    out.push(PathBuf::from("/usr/local/lib/reovim").join(kind.subdir()));
    out.push(PathBuf::from("/usr/lib/reovim").join(kind.subdir()));

    out
}

/// Builder for [`PathResolver`]. Start with
/// [`PathResolverBuilder::for_kind`] and populate inputs.
pub struct PathResolverBuilder {
    kind: Kind,
    cli_paths: Vec<PathBuf>,
    env: Arc<dyn EnvProvider>,
    system_fallback_enabled: bool,
    replace: bool,
}

impl PathResolverBuilder {
    /// Start a builder for the given [`Kind`]. Uses [`StdEnv`]
    /// by default; override with [`Self::with_env`] for tests.
    #[must_use]
    pub fn for_kind(kind: Kind) -> Self {
        Self {
            kind,
            cli_paths: Vec::new(),
            env: Arc::new(StdEnv),
            system_fallback_enabled: true,
            replace: false,
        }
    }

    /// Append a CLI-provided path (rule R1 or R2 depending on
    /// [`Kind`]). Caller may invoke this multiple times; order is
    /// preserved.
    #[must_use]
    pub fn push_cli_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.cli_paths.push(path.into());
        self
    }

    /// Replace the [`EnvProvider`]. Primarily used in tests.
    #[must_use]
    pub fn with_env(mut self, env: impl EnvProvider + 'static) -> Self {
        self.env = Arc::new(env);
        self
    }

    /// Disable rule R6 (XDG + system fallback). Used by
    /// `reovim-dev` which stages into a private `target/reovim-dev/`
    /// root and does not want system installs shadowing its fixtures.
    #[must_use]
    pub const fn without_system_fallback(mut self) -> Self {
        self.system_fallback_enabled = false;
        self
    }

    /// Internal replace-mode switch; not exposed through the reovim
    /// CLI per the locked "merge not replace" decision. Kept so a
    /// future opt-in flag has a home without API churn.
    #[must_use]
    #[doc(hidden)]
    pub const fn replace(mut self, replace: bool) -> Self {
        self.replace = replace;
        self
    }

    /// Build the resolver. Consumes the builder; the resulting
    /// [`PathResolver`] holds the flattened, merged search list.
    #[must_use]
    pub fn build(self) -> PathResolver {
        let Self {
            kind,
            cli_paths,
            env,
            system_fallback_enabled,
            replace,
        } = self;

        let mut search_paths: Vec<PathBuf> = Vec::new();

        // R1 / R2: CLI paths (kind-namespaced by the caller).
        search_paths.extend(cli_paths);

        // Short-circuit "replace" — CLI paths are the entire search
        // list, lower-precedence rules contribute nothing. Exposed
        // only through the private `replace(true)` escape hatch;
        // default is false.
        if replace {
            dedup_preserving_order(&mut search_paths);
            return PathResolver { search_paths };
        }

        // R3 / R4: colon-separated env var, filtered for empty segments.
        if let Some(raw) = env.get(kind.path_env_var()) {
            search_paths.extend(split_paths(&raw));
        }

        // R5: REOVIM_LIBRARY_ROOT/<kind>/
        if let Some(root) = env.get("REOVIM_LIBRARY_ROOT") {
            search_paths.push(PathBuf::from(root).join(kind.subdir()));
        }

        // R6: XDG + system fallback.
        if system_fallback_enabled {
            search_paths.extend(default_fallback_paths(&*env, kind));
        }

        dedup_preserving_order(&mut search_paths);
        PathResolver { search_paths }
    }
}

/// Flattened, ordered search list for a [`Kind`]. Produced by
/// [`PathResolverBuilder::build`] and consumed by the caller
/// (typically passed straight into `scan_paths`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathResolver {
    search_paths: Vec<PathBuf>,
}

impl PathResolver {
    /// Ordered search paths, highest precedence first.
    #[must_use]
    pub fn paths(&self) -> &[PathBuf] {
        &self.search_paths
    }
}

/// Remove duplicates while preserving first-seen order. Two paths are
/// equal when their [`PathBuf`] values are equal (no canonicalization
/// — symlinks and `./` prefixes count as distinct).
fn dedup_preserving_order(paths: &mut Vec<PathBuf>) {
    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| seen.insert(p.clone()));
}

#[cfg(test)]
#[path = "path_resolver_tests.rs"]
mod path_resolver_tests;
