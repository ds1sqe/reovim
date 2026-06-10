//! Dependency-graph probe engine for the reovim workspace.
//!
//! The engine enumerates workspace crates, classifies each into the
//! source categories of `Documentation/01-Architecture/02-Project-Layout-and-DAG.md`
//! §1, and verifies every manifest edge against the allowed-edge tables:
//! §2 category edges, §6 foundation sub-DAG, §7 composition catalog, §8
//! transitional allowlist. The probes under `tests/` run as part of plain
//! `cargo test` and fail closed (DAG1..DAG3, DAG5).
//!
//! **DAG4** is spec-asserted: the probe reads manifests only, so runtime
//! install layouts (`$ROOT/{module,driver,capability}/...`) can never grant
//! a Cargo dependency edge. The probe does not reach outside `Cargo.toml`
//! files, so install-path inputs simply do not exist in the probe's data
//! model.

pub mod toml;

use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Path, PathBuf},
};

/// Source categories per spec 1.2 §1. `archive/*` is excluded from
/// enumeration rather than classified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    /// `arch/`, `lib/*`, `uapi/*`
    Foundation,
    /// `server/lib/subsys/*`
    ServerContracts,
    /// `server/lib/kernel/*`
    ServerKernel,
    /// `server/lib/server/*` — Framed-protocol and dispatch glue.
    ServerRuntime,
    /// `clients/lib/subsys/*`
    ClientContracts,
    /// `ext/server/{modules,drivers,providers,domain}/*`
    ServerExt,
    /// `ext/client/{platforms,driver,module,capabilities}/*`
    ClientExt,
    /// `apps/*`
    Apps,
    /// `tools/*`
    Tools,
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Foundation => "foundation",
            Self::ServerContracts => "server-contracts",
            Self::ServerKernel => "server-kernel",
            Self::ServerRuntime => "server-runtime",
            Self::ClientContracts => "client-contracts",
            Self::ServerExt => "server-ext",
            Self::ClientExt => "client-ext",
            Self::Apps => "apps",
            Self::Tools => "tools",
        };
        f.write_str(name)
    }
}

/// The §1 category table. Pattern grammar: `/`-separated path
/// components; a trailing `*` matches one or more further
/// components; a pattern without `*` matches the exact path or any
/// path beneath it.
#[must_use]
pub fn default_category_table() -> Vec<(String, Category)> {
    [
        ("arch", Category::Foundation),
        ("lib/*", Category::Foundation),
        ("uapi/*", Category::Foundation),
        ("server/lib/subsys/*", Category::ServerContracts),
        ("server/lib/kernel/*", Category::ServerKernel),
        ("server/lib/server/*", Category::ServerRuntime),
        ("clients/lib/subsys/*", Category::ClientContracts),
        ("ext/server/modules/*", Category::ServerExt),
        ("ext/server/drivers/*", Category::ServerExt),
        ("ext/server/providers/*", Category::ServerExt),
        ("ext/server/domain/*", Category::ServerExt),
        ("ext/client/platforms/*", Category::ClientExt),
        ("ext/client/driver/*", Category::ClientExt),
        ("ext/client/module/*", Category::ClientExt),
        ("ext/client/capabilities/*", Category::ClientExt),
        ("apps/*", Category::Apps),
        ("tools/*", Category::Tools),
    ]
    .into_iter()
    .map(|(p, c)| (p.to_owned(), c))
    .collect()
}

/// Returns true when `pattern` matches the crate directory `path`
/// (workspace-relative, `/`-separated).
#[must_use]
pub fn pattern_matches(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('/').collect();
    let comps: Vec<&str> = path.split('/').collect();
    let (fixed, wildcard) = match pat.split_last() {
        Some((&"*", head)) => (head, true),
        _ => (pat.as_slice(), false),
    };
    if comps.len() < fixed.len() {
        return false;
    }
    if !fixed.iter().zip(&comps).all(|(a, b)| a == b) {
        return false;
    }
    if wildcard {
        comps.len() > fixed.len()
    } else {
        true
    }
}

/// One workspace crate as the probe sees it: manifest-derived only
/// (DAG4 — no install-layout input exists in this model).
#[derive(Debug, Clone)]
pub struct Crate {
    /// Package name from `[package].name`.
    pub name: String,
    /// Workspace-relative directory, `/`-separated.
    pub path: String,
    /// Named dependencies: name + parsed shape for each dep table entry.
    pub deps: Vec<DepEntry>,
}

/// One entry from a dependency table, carrying enough shape for DAG2 + DAG5.
#[derive(Debug, Clone)]
pub struct DepEntry {
    /// Resolved package name (after `package = "..."` renaming).
    pub name: String,
    /// Which dependency table this entry came from.
    pub table: DepTable,
    /// Whether the dep is an in-repo `path = "..."`.
    pub is_path: bool,
    /// Whether the dep has `workspace = true`.
    pub is_workspace_true: bool,
}

/// Which Cargo dependency table a dep entry came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepTable {
    /// `[dependencies]`
    Dependencies,
    /// `[dev-dependencies]`
    DevDependencies,
    /// `[build-dependencies]`
    BuildDependencies,
}

impl fmt::Display for DepTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Dependencies => "dependencies",
            Self::DevDependencies => "dev-dependencies",
            Self::BuildDependencies => "build-dependencies",
        })
    }
}

/// A violation found by the probe. Every variant is fail-closed:
/// the probe reports it unless an §8 allowlist entry covers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    /// DAG1: crate path matched no §1 category pattern.
    UnknownPath { path: String },
    /// DAG1: crate path matched more than one §1 category pattern.
    AmbiguousPath {
        path: String,
        candidates: Vec<String>,
    },
    /// DAG2: category-level edge not granted by §2.
    ForbiddenEdge {
        from: String,
        from_category: Category,
        to: String,
        to_category: Category,
    },
    /// §6: foundation-internal edge without a sub-DAG grant.
    UngrantedFoundationEdge { from: String, to: String },
    /// DAG3: `apps/*`/`tools/*` edge missing from the §7 catalog.
    UncatalogedCompositionEdge { from: String, to: String },
    /// DAG5: third-party or non-path dependency detected in a workspace manifest.
    NonSovereignDep {
        /// Workspace-relative path of the crate's manifest dir.
        crate_path: String,
        /// Dependency table where the violation was found.
        table: DepTable,
        /// Dependency name.
        dep: String,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPath { path } => {
                write!(f, "DAG1: crate at `{path}` matches no source category")
            }
            Self::AmbiguousPath { path, candidates } => write!(
                f,
                "DAG1: crate at `{path}` matches more than one source category pattern: {}",
                candidates.join(", ")
            ),
            Self::ForbiddenEdge {
                from,
                from_category,
                to,
                to_category,
            } => write!(
                f,
                "DAG2: `{from}` ({from_category}) may not depend on `{to}` ({to_category})"
            ),
            Self::UngrantedFoundationEdge { from, to } => {
                write!(f, "DAG2: foundation crate `{from}` has no §6 grant for `{to}`")
            }
            Self::UncatalogedCompositionEdge { from, to } => write!(
                f,
                "DAG3: composition edge `{from}` -> `{to}` is not in the composition catalog"
            ),
            Self::NonSovereignDep {
                crate_path,
                table,
                dep,
            } => write!(
                f,
                "DAG5: `{crate_path}` [{table}]: `{dep}` is not an in-repo path dependency"
            ),
        }
    }
}

/// Errors raised while loading the workspace or probe inputs.
#[derive(Debug)]
pub enum ProbeError {
    /// Filesystem read failure.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A `Cargo.toml`, catalog, or allowlist failed to parse.
    Parse { path: PathBuf, message: String },
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "io error at `{}`: {source}", path.display())
            }
            Self::Parse { path, message } => {
                write!(f, "parse error at `{}`: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for ProbeError {}

/// §7 composition catalog (`composition-edges.toml`).
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    /// Named composition edges.
    pub edge: Vec<CatalogEdge>,
}

impl Catalog {
    /// Validates that every entry has a non-blank `reason` and, if present,
    /// a non-blank `gate`.
    ///
    /// Blank fields turn the catalog into an unaudited pass-list, which is
    /// indistinguishable from a forbidden edge.
    ///
    /// # Errors
    ///
    /// Returns `ProbeError::Parse` when any entry has a blank required field.
    pub fn validate(&self, path: &Path) -> Result<(), ProbeError> {
        for edge in &self.edge {
            if edge.reason.trim().is_empty() {
                return Err(schema_error(
                    path,
                    &format!("edge `{}` -> `{}`: empty reason", edge.from, edge.to),
                ));
            }
            if edge.gate.as_ref().is_some_and(|g| g.trim().is_empty()) {
                return Err(schema_error(
                    path,
                    &format!("edge `{}` -> `{}`: empty gate", edge.from, edge.to),
                ));
            }
        }
        Ok(())
    }
}

/// One named composition edge.
#[derive(Debug, Clone)]
pub struct CatalogEdge {
    /// Depending package name.
    pub from: String,
    /// Depended-on package name.
    pub to: String,
    /// Optional feature gate.
    pub gate: Option<String>,
    /// Mandatory justification.
    pub reason: String,
}

/// §8 transitional allowlist (`transitional-allowlist.toml`).
#[derive(Debug, Clone, Default)]
pub struct Allowlist {
    /// Time-bounded known violations.
    pub entry: Vec<AllowlistEntry>,
}

impl Allowlist {
    /// Validates that every entry has non-blank `reason`, `issue`, and
    /// `expires` fields.
    ///
    /// Blank fields would admit an untracked permanent hole rather than a
    /// transition.
    ///
    /// # Errors
    ///
    /// Returns `ProbeError::Parse` when any entry has a blank required field.
    pub fn validate(&self, path: &Path) -> Result<(), ProbeError> {
        for entry in &self.entry {
            for (field, value) in [
                ("reason", &entry.reason),
                ("issue", &entry.issue),
                ("expires", &entry.expires),
            ] {
                if value.trim().is_empty() {
                    return Err(schema_error(
                        path,
                        &format!("entry `{}` -> `{}`: empty {field}", entry.from, entry.to),
                    ));
                }
            }
        }
        Ok(())
    }
}

fn schema_error(path: &Path, message: &str) -> ProbeError {
    ProbeError::Parse {
        path: path.to_path_buf(),
        message: message.to_owned(),
    }
}

/// One dated allowlist entry. `issue` and `expires` are mandatory:
/// an entry without an expiry is a forbidden edge, not a transition.
#[derive(Debug, Clone)]
pub struct AllowlistEntry {
    /// Depending crate (package name or source path).
    pub from: String,
    /// Depended-on package name.
    pub to: String,
    /// Justification.
    pub reason: String,
    /// Tracking issue reference.
    pub issue: String,
    /// Expiry criterion.
    pub expires: String,
}

/// Probe configuration: the spec tables as data. `default_for` loads the
/// real tables; fixtures may construct custom ones.
#[derive(Debug, Clone)]
pub struct ProbeConfig {
    /// §1 category table.
    pub category_table: Vec<(String, Category)>,
    /// §6 foundation sub-DAG grants: crate name → allowed workspace dep
    /// names. Foundation-internal edges absent from this map are violations.
    pub foundation_grants: BTreeMap<String, Vec<String>>,
    /// §7 composition catalog.
    pub catalog: Catalog,
    /// §8 transitional allowlist.
    pub allowlist: Allowlist,
}

impl ProbeConfig {
    /// Loads the normative configuration for a workspace root:
    /// the §1/§6 tables compiled in, and the §7/§8 TOML files from
    /// `tools/depgraph-probes/` (absent files mean empty tables).
    ///
    /// # Errors
    ///
    /// Returns `ProbeError::Io` / `ProbeError::Parse` when a catalog or
    /// allowlist file exists but cannot be read, parsed, or schema-validated.
    pub fn default_for(root: &Path) -> Result<Self, ProbeError> {
        let catalog_path = root.join("tools/depgraph-probes/composition-edges.toml");
        let allowlist_path = root.join("tools/depgraph-probes/transitional-allowlist.toml");
        let catalog = if catalog_path.exists() {
            let catalog = load_catalog(&catalog_path)?;
            catalog.validate(&catalog_path)?;
            catalog
        } else {
            Catalog::default()
        };
        let allowlist = if allowlist_path.exists() {
            let allowlist = load_allowlist(&allowlist_path)?;
            allowlist.validate(&allowlist_path)?;
            allowlist
        } else {
            Allowlist::default()
        };
        Ok(Self {
            category_table: default_category_table(),
            // §6: every current grant is empty; the table grows only by spec
            // edit + a change here. TODO(#778): populate when the foundation
            // sub-DAG is enumerated.
            foundation_grants: BTreeMap::new(),
            catalog,
            allowlist,
        })
    }
}

fn load_catalog(path: &Path) -> Result<Catalog, ProbeError> {
    let doc = toml::parse_file(path)?;
    let mut edges = Vec::new();
    for record in doc.array("edge") {
        let from = get_required_str(record, "from", path)?;
        let to = get_required_str(record, "to", path)?;
        let reason = get_required_str(record, "reason", path)?;
        let gate = record.get("gate").cloned();
        edges.push(CatalogEdge {
            from: from.to_owned(),
            to: to.to_owned(),
            gate,
            reason: reason.to_owned(),
        });
    }
    Ok(Catalog { edge: edges })
}

fn load_allowlist(path: &Path) -> Result<Allowlist, ProbeError> {
    let doc = toml::parse_file(path)?;
    let mut entries = Vec::new();
    for record in doc.array("entry") {
        let from = get_required_str(record, "from", path)?;
        let to = get_required_str(record, "to", path)?;
        let reason = get_required_str(record, "reason", path)?;
        let issue = get_required_str(record, "issue", path)?;
        let expires = get_required_str(record, "expires", path)?;
        entries.push(AllowlistEntry {
            from: from.to_owned(),
            to: to.to_owned(),
            reason: reason.to_owned(),
            issue: issue.to_owned(),
            expires: expires.to_owned(),
        });
    }
    Ok(Allowlist { entry: entries })
}

fn get_required_str<'a>(
    record: &'a std::collections::BTreeMap<String, String>,
    key: &str,
    path: &Path,
) -> Result<&'a str, ProbeError> {
    record
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| ProbeError::Parse {
            path: path.to_path_buf(),
            message: format!("missing required field `{key}` in array entry"),
        })
}

/// A violation suppressed by a dated §8 entry, carrying the entry's tracking
/// data so the suppression stays auditable in the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allowlisted {
    /// The suppressed violation.
    pub violation: Violation,
    /// Tracking issue from the allowlist entry.
    pub issue: String,
    /// Expiry criterion from the allowlist entry.
    pub expires: String,
}

/// Probe outcome: per-category counts plus every violation found.
/// `allowlisted` records violations suppressed by §8 entries.
#[derive(Debug, Default)]
pub struct Report {
    /// Crate count per classified category.
    pub category_counts: BTreeMap<Category, usize>,
    /// Violations not covered by the allowlist.
    pub violations: Vec<Violation>,
    /// Violations suppressed by a dated §8 entry.
    pub allowlisted: Vec<Allowlisted>,
}

impl Report {
    /// True when the workspace conforms (no live violations).
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    /// Human-readable summary, one violation per line.
    #[must_use]
    pub fn summary(&self) -> String {
        use fmt::Write as _;
        let mut out = String::new();
        for (category, count) in &self.category_counts {
            // Writing to a String is infallible.
            let _ = writeln!(out, "{category}: {count} crate(s)");
        }
        for a in &self.allowlisted {
            let _ =
                writeln!(out, "allowlisted ({}, expires {}): {}", a.issue, a.expires, a.violation);
        }
        for v in &self.violations {
            let _ = writeln!(out, "VIOLATION: {v}");
        }
        out
    }
}

/// Directories never descended into during enumeration. `archive` is the
/// spec's Archive category (excluded from the depgraph); the rest are
/// build/VCS/scratch artifacts.
const SKIP_DIRS: &[&str] = &["archive", "target", ".git", "tmp", ".github"];

/// Enumerates every workspace crate under `root` by scanning for
/// `Cargo.toml` files with a `[package]` section.
///
/// Scanning the tree (rather than reading `[workspace.members]`) is
/// deliberate: a crate dropped at an unlisted path must still be
/// classified, so DAG1 catches it before anyone wires it into the
/// workspace.
///
/// # Errors
///
/// Returns `ProbeError` on unreadable directories or unparseable manifests.
pub fn enumerate_crates(root: &Path) -> Result<Vec<Crate>, ProbeError> {
    let mut crates = Vec::new();
    walk(root, "", &mut crates)?;
    crates.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(crates)
}

/// Maps an I/O error to `ProbeError::Io` at `path`. Shared by every
/// filesystem touch point so the conversion is a single covered region.
fn io_error(path: &Path) -> impl Fn(std::io::Error) -> ProbeError + '_ {
    move |source| ProbeError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Recursive enumeration step. `rel` is the `/`-separated
/// workspace-relative path of `dir` ("" at the root), threaded down
/// so crate paths never depend on prefix arithmetic.
fn walk(dir: &Path, rel: &str, out: &mut Vec<Crate>) -> Result<(), ProbeError> {
    // Collecting up front folds per-entry iteration errors into the
    // same fallible step as the directory open.
    let entries: Vec<fs::DirEntry> = fs::read_dir(dir)
        .and_then(Iterator::collect)
        .map_err(io_error(dir))?;
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if SKIP_DIRS.contains(&name.as_ref()) {
                continue;
            }
            let child_rel = if rel.is_empty() {
                name.into_owned()
            } else {
                format!("{rel}/{name}")
            };
            walk(&path, &child_rel, out)?;
        } else if name == "Cargo.toml"
            && !rel.is_empty()
            && let Some(krate) = parse_manifest(rel, &path)?
        {
            out.push(krate);
        }
    }
    Ok(())
}

fn parse_manifest(rel: &str, manifest: &Path) -> Result<Option<Crate>, ProbeError> {
    let doc = toml::parse_file(manifest)?;
    let Some(pkg_section) = doc.sections.get("package") else {
        // A nested virtual workspace manifest has no [package] section.
        return Ok(None);
    };
    let name = pkg_section
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ProbeError::Parse {
            path: manifest.to_path_buf(),
            message: "missing [package].name".to_owned(),
        })?
        .to_owned();

    let mut deps = Vec::new();
    for (label, dep_table) in [
        ("dependencies", DepTable::Dependencies),
        ("dev-dependencies", DepTable::DevDependencies),
        ("build-dependencies", DepTable::BuildDependencies),
    ] {
        let Some(section) = doc.sections.get(label) else {
            continue;
        };
        for (key, spec) in section {
            let entry = dep_entry_from_toml_value(key, spec, dep_table, manifest)?;
            deps.push(entry);
        }
    }
    Ok(Some(Crate {
        name,
        path: rel.to_owned(),
        deps,
    }))
}

fn dep_entry_from_toml_value(
    key: &str,
    spec: &toml::TomlValue,
    table: DepTable,
    manifest: &Path,
) -> Result<DepEntry, ProbeError> {
    match spec {
        toml::TomlValue::String(_) => {
            // Bare version string: `dep = "1.0"` — registry dep, not path.
            Ok(DepEntry {
                name: key.to_owned(),
                table,
                is_path: false,
                is_workspace_true: false,
            })
        }
        toml::TomlValue::InlineTable(t) => {
            // `package` key renames the dependency.
            let resolved_name = t
                .get("package")
                .and_then(toml::TomlValue::as_str)
                .unwrap_or(key)
                .to_owned();
            let is_path = t.contains_key("path");
            let is_workspace_true = t
                .get("workspace")
                .and_then(toml::TomlValue::as_bool)
                .unwrap_or(false);
            Ok(DepEntry {
                name: resolved_name,
                table,
                is_path,
                is_workspace_true,
            })
        }
        toml::TomlValue::Bool(_) => Err(ProbeError::Parse {
            path: manifest.to_path_buf(),
            message: format!("unexpected bool value for dependency `{key}`"),
        }),
        toml::TomlValue::Array(_) => Err(ProbeError::Parse {
            path: manifest.to_path_buf(),
            message: format!("unexpected array value for dependency `{key}`"),
        }),
    }
}

/// Classifies one crate path against a category table. Returns the
/// matched category, or the DAG1 violation when zero or multiple
/// patterns match.
///
/// # Errors
///
/// Returns `Violation::UnknownPath` when no pattern matches and
/// `Violation::AmbiguousPath` when more than one matches (DAG1).
pub fn classify(path: &str, table: &[(String, Category)]) -> Result<Category, Violation> {
    let matches: Vec<&(String, Category)> = table
        .iter()
        .filter(|(pattern, _)| pattern_matches(pattern, path))
        .collect();
    match matches.as_slice() {
        [] => Err(Violation::UnknownPath {
            path: path.to_owned(),
        }),
        [(_, category)] => Ok(*category),
        many => {
            let mut candidates: Vec<String> =
                many.iter().map(|(p, c)| format!("`{p}` ({c})")).collect();
            candidates.sort();
            Err(Violation::AmbiguousPath {
                path: path.to_owned(),
                candidates,
            })
        }
    }
}

/// §2 category matrix: which categories `from` may depend on.
///
/// Foundation-internal edges are not listed here — they are governed
/// exclusively by the §6 grant table. Cells the spec conditions on
/// per-chapter lists that do not exist yet (peer subsys edges, ext peer
/// manifests, the client-subsys DAG) are closed; realising chapters open
/// them by spec edit plus a table change here.
const fn allowed_categories(from: Category) -> &'static [Category] {
    match from {
        Category::Foundation | Category::Apps | Category::Tools => &[],
        Category::ServerContracts | Category::ClientContracts => &[Category::Foundation],
        Category::ServerKernel | Category::ServerExt => {
            &[Category::ServerContracts, Category::Foundation]
        }
        Category::ServerRuntime => &[
            Category::ServerKernel,
            Category::ServerContracts,
            Category::Foundation,
        ],
        Category::ClientExt => &[Category::ClientContracts, Category::Foundation],
    }
}

/// Runs the full §5 probe over a workspace.
///
/// # Errors
///
/// Returns `ProbeError` when the workspace cannot be enumerated;
/// rule violations are reported in the `Report`, not as errors.
pub fn run_probe(root: &Path, config: &ProbeConfig) -> Result<Report, ProbeError> {
    let crates = enumerate_crates(root)?;
    let mut report = Report::default();
    let mut classified: BTreeMap<&str, Category> = BTreeMap::new();

    // DAG1: classify every crate.
    for krate in &crates {
        match classify(&krate.path, &config.category_table) {
            Ok(category) => {
                *report.category_counts.entry(category).or_insert(0) += 1;
                classified.insert(&krate.name, category);
            }
            Err(violation) => report.violations.push(violation),
        }
    }

    let mut raw_violations = Vec::new();

    // DAG2/DAG3: category-edge and composition checks.
    for krate in &crates {
        let Some(&from_category) = classified.get(krate.name.as_str()) else {
            // Unclassified crates already carry their DAG1 violation.
            continue;
        };
        for dep in &krate.deps {
            let Some(&to_category) = classified.get(dep.name.as_str()) else {
                // External dependency: outside the workspace graph — DAG5 handles it below.
                continue;
            };
            if let Some(v) = check_edge(krate, from_category, &dep.name, to_category, config) {
                raw_violations.push(v);
            }
        }
    }

    // DAG5: sovereignty walk — every dep must be an in-repo path dep.
    for krate in &crates {
        for dep in &krate.deps {
            if !dep.is_path && !dep.is_workspace_true {
                // Plain string version or unrecognised inline-table shape
                // without `path` or `workspace = true` → non-sovereign.
                raw_violations.push(Violation::NonSovereignDep {
                    crate_path: krate.path.clone(),
                    table: dep.table,
                    dep: dep.name.clone(),
                });
            } else if dep.is_workspace_true {
                // `{ workspace = true }` — the workspace dep table is intentionally
                // empty (L9 marker); any workspace = true dep is a violation.
                raw_violations.push(Violation::NonSovereignDep {
                    crate_path: krate.path.clone(),
                    table: dep.table,
                    dep: dep.name.clone(),
                });
            }
        }
    }

    // Apply allowlist to DAG2/DAG3 violations (DAG5 has no allowlist, §9).
    for violation in raw_violations {
        match &violation {
            Violation::NonSovereignDep { .. } => {
                // DAG5 violations are never allowlisted.
                report.violations.push(violation);
            }
            _ => {
                if let Some(entry) = allowlist_match(&violation, &config.allowlist) {
                    report.allowlisted.push(Allowlisted {
                        violation,
                        issue: entry.issue.clone(),
                        expires: entry.expires.clone(),
                    });
                } else {
                    report.violations.push(violation);
                }
            }
        }
    }

    Ok(report)
}

#[must_use]
pub(crate) fn check_edge(
    krate: &Crate,
    from_category: Category,
    dep: &str,
    to_category: Category,
    config: &ProbeConfig,
) -> Option<Violation> {
    match from_category {
        Category::Foundation => {
            let granted = config
                .foundation_grants
                .get(&krate.name)
                .is_some_and(|grants| grants.iter().any(|g| g == dep));
            if granted {
                None
            } else {
                Some(Violation::UngrantedFoundationEdge {
                    from: krate.name.clone(),
                    to: dep.to_owned(),
                })
            }
        }
        Category::Apps | Category::Tools => {
            let cataloged = config
                .catalog
                .edge
                .iter()
                .any(|e| e.from == krate.name && e.to == dep);
            if cataloged {
                None
            } else {
                Some(Violation::UncatalogedCompositionEdge {
                    from: krate.name.clone(),
                    to: dep.to_owned(),
                })
            }
        }
        _ => {
            if allowed_categories(from_category).contains(&to_category) {
                None
            } else {
                Some(Violation::ForbiddenEdge {
                    from: krate.name.clone(),
                    from_category,
                    to: dep.to_owned(),
                    to_category,
                })
            }
        }
    }
}

#[must_use]
pub(crate) fn allowlist_match<'a>(
    violation: &Violation,
    allowlist: &'a Allowlist,
) -> Option<&'a AllowlistEntry> {
    let (from, to) = match violation {
        Violation::ForbiddenEdge { from, to, .. }
        | Violation::UngrantedFoundationEdge { from, to }
        | Violation::UncatalogedCompositionEdge { from, to } => (from, to),
        Violation::UnknownPath { .. }
        | Violation::AmbiguousPath { .. }
        | Violation::NonSovereignDep { .. } => return None,
    };
    allowlist
        .entry
        .iter()
        .find(|e| &e.from == from && &e.to == to)
}

#[cfg(test)]
mod tests;
