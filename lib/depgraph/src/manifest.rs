use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fmt, fs, io,
    path::{Path, PathBuf},
};

/// The Cargo table that declares a dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependencyKind {
    Normal,
    Development,
    Build,
}

impl fmt::Display for DependencyKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Normal => "normal",
            Self::Development => "development",
            Self::Build => "build",
        })
    }
}

/// Where Cargo obtains a dependency.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependencySource {
    /// Registry, Git, or another non-path declaration. The preflight retains
    /// it but does not resolve its identity, lock selection, or review status.
    External,
    /// A path relative to the declaring package manifest.
    Path(PathBuf),
    /// Inheritance through `[workspace.dependencies]`.
    Workspace,
}

/// One dependency declaration retained from a package manifest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Dependency {
    pub alias: String,
    pub package: String,
    pub source: DependencySource,
    pub kind: DependencyKind,
    pub target: Option<String>,
    pub optional: bool,
}

/// One Cargo package discovered below the checked root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub manifest_path: PathBuf,
    pub relative_dir: PathBuf,
    pub dependencies: Vec<Dependency>,
}

/// One locally matched declared `path` edge. Optional and target-specific
/// declarations remain preflight edges because Cargo still reads their
/// manifests.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalDependencyEdge {
    pub consumer: String,
    pub provider: String,
    pub kind: DependencyKind,
    pub target: Option<String>,
    pub optional: bool,
}

/// Recursive manifest-discovery controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryOptions {
    ignored_directory_names: BTreeSet<OsString>,
}

impl DiscoveryOptions {
    /// Starts with only VCS metadata and Cargo build output excluded.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ignored_directory_names: [
                OsString::from(".git"),
                OsString::from("target"),
            ]
            .into_iter()
            .collect(),
        }
    }

    /// Adds a directory basename to the discovery exclusion set.
    #[must_use]
    pub fn ignore_directory(mut self, name: impl Into<OsString>) -> Self {
        self.ignored_directory_names.insert(name.into());
        self
    }

    fn ignores(&self, name: &OsStr) -> bool {
        self.ignored_directory_names.contains(name)
    }
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// A graph derived only from the supported Cargo manifest declarations.
///
/// This is not Cargo's feature/target-resolved package graph. A caller must use
/// Cargo metadata from the exact build graph before making dependency or
/// architecture-conformance claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredManifestGraph {
    root: PathBuf,
    packages: BTreeMap<String, Package>,
    local_edges: Vec<LocalDependencyEdge>,
}

impl DeclaredManifestGraph {
    /// Returns the canonical checked root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns packages keyed by Cargo package name.
    #[must_use]
    pub const fn packages(&self) -> &BTreeMap<String, Package> {
        &self.packages
    }

    /// Returns deterministic, de-duplicated local dependency edges.
    #[must_use]
    pub fn local_edges(&self) -> &[LocalDependencyEdge] {
        &self.local_edges
    }
}

/// Manifest discovery, parsing, or local-edge resolution failure.
#[derive(Debug)]
pub enum GraphError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        line: usize,
        message: String,
    },
    DuplicatePackageName {
        package: String,
        first: PathBuf,
        second: PathBuf,
    },
    PathDependencyOutsideRoot {
        consumer: String,
        path: PathBuf,
    },
    UnresolvedPathDependency {
        consumer: String,
        dependency: String,
        path: PathBuf,
    },
    PathPackageMismatch {
        consumer: String,
        dependency: String,
        expected: String,
        actual: String,
    },
    WorkspaceDependencyUnsupported {
        consumer: String,
        dependency: String,
    },
    UnsupportedResolutionSection {
        path: PathBuf,
        line: usize,
        section: String,
    },
}

impl fmt::Display for GraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "{}: {source}", path.display())
            }
            Self::Parse { path, line, message } => {
                write!(formatter, "{}:{line}: {message}", path.display())
            }
            Self::DuplicatePackageName { package, first, second } => write!(
                formatter,
                "duplicate package {package}: {} and {}",
                first.display(),
                second.display()
            ),
            Self::PathDependencyOutsideRoot { consumer, path } => write!(
                formatter,
                concat!(
                    "package {consumer} has a path dependency outside ",
                    "the checked root: {path}"
                ),
                consumer = consumer,
                path = path.display(),
            ),
            Self::UnresolvedPathDependency { consumer, dependency, path } => {
                write!(
                    formatter,
                    concat!(
                        "package {consumer} dependency {dependency} does ",
                        "not resolve to a discovered package at {path}"
                    ),
                    consumer = consumer,
                    dependency = dependency,
                    path = path.display(),
                )
            }
            Self::PathPackageMismatch {
                consumer,
                dependency,
                expected,
                actual,
            } => write!(
                formatter,
                concat!(
                    "package {consumer} dependency {dependency} expected ",
                    "package {expected}, found {actual}"
                ),
                consumer = consumer,
                dependency = dependency,
                expected = expected,
                actual = actual,
            ),
            Self::WorkspaceDependencyUnsupported { consumer, dependency } => {
                write!(
                    formatter,
                    concat!(
                        "package {consumer} dependency {dependency} ",
                        "inherits workspace metadata; use Cargo metadata ",
                        "before declared-graph preflight"
                    ),
                    consumer = consumer,
                    dependency = dependency,
                )
            }
            Self::UnsupportedResolutionSection { path, line, section } => {
                write!(
                    formatter,
                    concat!(
                        "{path}:{line}: unsupported Cargo resolution section ",
                        "[{section}]; use Cargo metadata"
                    ),
                    path = path.display(),
                    line = line,
                    section = section,
                )
            }
        }
    }
}

impl std::error::Error for GraphError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Io { source, .. } = self { Some(source) } else { None }
    }
}

/// Recursively discovers manifests and preflights the supported declared local
/// `path` dependency subset.
///
/// This function does not invoke Cargo's resolver. In particular, a returned
/// graph is not sufficient evidence for the exact feature/target-resolved
/// package graph or Reovim `ARCH-*` conformance. Callers must also inspect
/// Cargo metadata and run the separate source/import/public-type checks.
/// Registry and Git dependencies are retained as external declarations, not
/// resolved or dependency-reviewed here.
///
/// # Errors
///
/// Discovery fails on unreadable or unsupported manifests, duplicate package
/// names, unresolved or escaping path dependencies, package-name mismatches,
/// workspace-inherited dependencies, and `[patch]`, `[replace]`, or
/// `[workspace.dependencies]` resolution sections (including equivalent
/// supported inline-table shapes).
pub fn load_declared_manifest_graph(
    root: impl AsRef<Path>,
    options: &DiscoveryOptions,
) -> Result<DeclaredManifestGraph, GraphError> {
    let requested_root = root.as_ref();
    let root = fs::canonicalize(requested_root).map_err(|source| {
        GraphError::Io { path: requested_root.to_path_buf(), source }
    })?;
    let mut manifest_paths = Vec::new();
    walk_manifests(&root, options, &mut manifest_paths)?;
    manifest_paths.sort();

    let mut packages: BTreeMap<String, Package> = BTreeMap::new();
    let mut package_by_dir = BTreeMap::new();
    for manifest_path in manifest_paths {
        let Some(mut package) = parse_manifest(&manifest_path)? else {
            continue;
        };
        let Some(package_dir) = manifest_path.parent() else {
            return Err(parse_error(
                &manifest_path,
                0,
                "manifest path has no package directory",
            ));
        };
        let canonical_dir =
            fs::canonicalize(package_dir).map_err(|source| GraphError::Io {
                path: package_dir.to_path_buf(),
                source,
            })?;
        package.relative_dir = canonical_dir
            .strip_prefix(&root)
            .map_err(|_| GraphError::PathDependencyOutsideRoot {
                consumer: package.name.clone(),
                path: canonical_dir.clone(),
            })?
            .to_path_buf();
        if let Some(first) = packages.get(&package.name) {
            return Err(GraphError::DuplicatePackageName {
                package: package.name,
                first: first.manifest_path.clone(),
                second: package.manifest_path,
            });
        }
        package_by_dir.insert(canonical_dir, package.name.clone());
        packages.insert(package.name.clone(), package);
    }

    let local_edges = resolve_local_edges(&root, &packages, &package_by_dir)?;

    Ok(DeclaredManifestGraph { root, packages, local_edges })
}

fn resolve_local_edges(
    root: &Path,
    packages: &BTreeMap<String, Package>,
    package_by_dir: &BTreeMap<PathBuf, String>,
) -> Result<Vec<LocalDependencyEdge>, GraphError> {
    let mut edges = BTreeSet::new();
    for package in packages.values() {
        resolve_package_edges(root, package, package_by_dir, &mut edges)?;
    }

    Ok(edges.into_iter().collect())
}

fn resolve_package_edges(
    root: &Path,
    package: &Package,
    package_by_dir: &BTreeMap<PathBuf, String>,
    edges: &mut BTreeSet<LocalDependencyEdge>,
) -> Result<(), GraphError> {
    let Some(package_dir) = package.manifest_path.parent() else {
        return Err(parse_error(
            &package.manifest_path,
            0,
            "manifest path has no package directory",
        ));
    };
    for dependency in &package.dependencies {
        edges.extend(resolve_local_edge(
            root,
            package,
            package_dir,
            dependency,
            package_by_dir,
        )?);
    }
    Ok(())
}

fn resolve_local_edge(
    root: &Path,
    package: &Package,
    package_dir: &Path,
    dependency: &Dependency,
    package_by_dir: &BTreeMap<PathBuf, String>,
) -> Result<Option<LocalDependencyEdge>, GraphError> {
    let declared = match &dependency.source {
        DependencySource::External => return Ok(None),
        DependencySource::Workspace => {
            return Err(GraphError::WorkspaceDependencyUnsupported {
                consumer: package.name.clone(),
                dependency: dependency.alias.clone(),
            });
        }
        DependencySource::Path(declared) => declared,
    };
    let provider = resolve_path_provider(
        root,
        package,
        package_dir,
        dependency,
        declared,
        package_by_dir,
    )?;
    Ok(Some(LocalDependencyEdge {
        consumer: package.name.clone(),
        provider,
        kind: dependency.kind,
        target: dependency.target.clone(),
        optional: dependency.optional,
    }))
}

fn resolve_path_provider(
    root: &Path,
    package: &Package,
    package_dir: &Path,
    dependency: &Dependency,
    declared: &Path,
    package_by_dir: &BTreeMap<PathBuf, String>,
) -> Result<String, GraphError> {
    let joined = package_dir.join(declared);
    let resolved = fs::canonicalize(&joined).map_err(|_| {
        GraphError::UnresolvedPathDependency {
            consumer: package.name.clone(),
            dependency: dependency.alias.clone(),
            path: joined.clone(),
        }
    })?;
    if !resolved.starts_with(root) {
        return Err(GraphError::PathDependencyOutsideRoot {
            consumer: package.name.clone(),
            path: resolved,
        });
    }
    let Some(provider) = package_by_dir.get(&resolved) else {
        return Err(GraphError::UnresolvedPathDependency {
            consumer: package.name.clone(),
            dependency: dependency.alias.clone(),
            path: resolved,
        });
    };
    if provider != &dependency.package {
        return Err(GraphError::PathPackageMismatch {
            consumer: package.name.clone(),
            dependency: dependency.alias.clone(),
            expected: dependency.package.clone(),
            actual: provider.clone(),
        });
    }
    Ok(provider.clone())
}

fn walk_manifests(
    directory: &Path,
    options: &DiscoveryOptions,
    manifests: &mut Vec<PathBuf>,
) -> Result<(), GraphError> {
    let manifest = directory.join("Cargo.toml");
    if manifest.is_file() {
        manifests.push(manifest);
    }

    let entries = fs::read_dir(directory).map_err(|source| GraphError::Io {
        path: directory.to_path_buf(),
        source,
    })?;
    let mut child_directories = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| GraphError::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source| GraphError::Io { path: entry.path(), source })?;
        if file_type.is_dir() && !options.ignores(&entry.file_name()) {
            child_directories.push(entry.path());
        }
    }
    child_directories.sort();
    for child in child_directories {
        walk_manifests(&child, options, manifests)?;
    }
    Ok(())
}

#[derive(Debug)]
enum Section {
    Root,
    Workspace,
    Package,
    DependencyList { kind: DependencyKind, target: Option<String> },
    DependencyDetail(DependencyBuilder),
    Other,
}

#[derive(Debug)]
struct DependencyBuilder {
    alias: String,
    package: Option<String>,
    path: Option<PathBuf>,
    workspace: bool,
    optional: bool,
    kind: DependencyKind,
    target: Option<String>,
}

impl DependencyBuilder {
    const fn new(
        alias: String,
        kind: DependencyKind,
        target: Option<String>,
    ) -> Self {
        Self {
            alias,
            package: None,
            path: None,
            workspace: false,
            optional: false,
            kind,
            target,
        }
    }

    fn finish(
        self,
        path: &Path,
        line: usize,
    ) -> Result<Dependency, GraphError> {
        if self.workspace && self.path.is_some() {
            return Err(parse_error(
                path,
                line,
                "dependency cannot declare both `workspace = true` and `path`",
            ));
        }
        let source = if self.workspace {
            DependencySource::Workspace
        } else if let Some(path) = self.path {
            DependencySource::Path(path)
        } else {
            DependencySource::External
        };
        let package = self.package.unwrap_or_else(|| self.alias.clone());
        Ok(Dependency {
            alias: self.alias,
            package,
            source,
            kind: self.kind,
            target: self.target,
            optional: self.optional,
        })
    }
}

fn parse_manifest(path: &Path) -> Result<Option<Package>, GraphError> {
    let text = fs::read_to_string(path).map_err(|source| GraphError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut section = Section::Root;
    let mut package_seen = false;
    let mut package_name = None;
    let mut dependencies = Vec::new();
    let mut last_line = 1;

    for (index, raw_line) in text.lines().enumerate() {
        let line_number = index + 1;
        last_line = line_number;
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            parse_manifest_header(
                line,
                path,
                line_number,
                &mut section,
                &mut dependencies,
                &mut package_seen,
            )?;
            continue;
        }

        let Some((raw_key, raw_value)) =
            parse_manifest_assignment(line, &section, path, line_number)?
        else {
            continue;
        };
        let value = raw_value.trim();
        let key =
            parse_assignment_key(&section, raw_key, value, path, line_number)?;
        apply_manifest_assignment(
            &mut section,
            &mut package_name,
            &mut dependencies,
            key,
            value,
            path,
            line_number,
        )?;
    }
    flush_detail(&mut section, &mut dependencies, path, last_line)?;

    if !package_seen {
        return Ok(None);
    }
    let Some(name) = package_name else {
        return Err(parse_error(
            path,
            last_line,
            "[package] is missing string `name`",
        ));
    };
    dependencies.sort();
    Ok(Some(Package {
        name,
        manifest_path: path.to_path_buf(),
        relative_dir: PathBuf::new(),
        dependencies,
    }))
}

fn parse_manifest_header(
    line: &str,
    path: &Path,
    line_number: usize,
    section: &mut Section,
    dependencies: &mut Vec<Dependency>,
    package_seen: &mut bool,
) -> Result<(), GraphError> {
    flush_detail(section, dependencies, path, line_number)?;
    let parts = parse_header(line, path, line_number)?;
    if is_unsupported_resolution_section(&parts) {
        return Err(GraphError::UnsupportedResolutionSection {
            path: path.to_path_buf(),
            line: line_number,
            section: parts.join("."),
        });
    }
    *section = classify_section(&parts);
    *package_seen |= matches!(section, Section::Package);
    Ok(())
}

fn parse_manifest_assignment<'a>(
    line: &'a str,
    section: &Section,
    path: &Path,
    line_number: usize,
) -> Result<Option<(&'a str, &'a str)>, GraphError> {
    let Some(assignment) = split_assignment(line) else {
        if section_requires_assignment(section) {
            return Err(parse_error(
                path,
                line_number,
                "expected `key = value`",
            ));
        }
        return Ok(None);
    };
    Ok(Some(assignment))
}

const fn section_requires_assignment(section: &Section) -> bool {
    matches!(
        section,
        Section::Package
            | Section::DependencyList { .. }
            | Section::DependencyDetail(_)
    )
}

fn apply_manifest_assignment(
    section: &mut Section,
    package_name: &mut Option<String>,
    dependencies: &mut Vec<Dependency>,
    key: String,
    value: &str,
    path: &Path,
    line_number: usize,
) -> Result<(), GraphError> {
    match section {
        Section::Package if key == "name" => {
            set_package_name(package_name, value, path, line_number)
        }
        Section::DependencyList { kind, target } => {
            dependencies.push(parse_dependency(
                key,
                value,
                *kind,
                target.clone(),
                path,
                line_number,
            )?);
            Ok(())
        }
        Section::DependencyDetail(builder) => {
            apply_detail(builder, &key, value, path, line_number)
        }
        Section::Root
        | Section::Workspace
        | Section::Package
        | Section::Other => Ok(()),
    }
}

fn set_package_name(
    package_name: &mut Option<String>,
    value: &str,
    path: &Path,
    line_number: usize,
) -> Result<(), GraphError> {
    if package_name.is_some() {
        return Err(parse_error(path, line_number, "duplicate package name"));
    }
    *package_name = Some(parse_string(value, path, line_number)?);
    Ok(())
}

fn parse_assignment_key(
    section: &Section,
    raw_key: &str,
    value: &str,
    path: &Path,
    line: usize,
) -> Result<String, GraphError> {
    let key_parts = parse_dotted_key(raw_key.trim(), path, line)?;
    let unsupported_section = match section {
        Section::Root
            if matches!(
                key_parts.first().map(String::as_str),
                Some("patch" | "replace")
            ) =>
        {
            Some(key_parts.join("."))
        }
        Section::Root if begins_workspace_dependencies(&key_parts) => {
            Some(key_parts.join("."))
        }
        Section::Root
            if key_parts == ["workspace"]
                && inline_table_declares_key(
                    value,
                    "dependencies",
                    path,
                    line,
                )? =>
        {
            Some("workspace.dependencies".to_owned())
        }
        Section::Workspace
            if key_parts.first().map(String::as_str)
                == Some("dependencies") =>
        {
            Some(format!("workspace.{}", key_parts.join(".")))
        }
        _ => None,
    };
    if let Some(section) = unsupported_section {
        return Err(GraphError::UnsupportedResolutionSection {
            path: path.to_path_buf(),
            line,
            section,
        });
    }
    let [key] = key_parts.as_slice() else {
        return Err(parse_error(
            path,
            line,
            "dotted assignment keys are outside the supported manifest subset",
        ));
    };
    Ok(key.clone())
}

fn flush_detail(
    section: &mut Section,
    dependencies: &mut Vec<Dependency>,
    path: &Path,
    line: usize,
) -> Result<(), GraphError> {
    let previous = std::mem::replace(section, Section::Other);
    if let Section::DependencyDetail(builder) = previous {
        dependencies.push(builder.finish(path, line)?);
    }
    Ok(())
}

fn classify_section(parts: &[String]) -> Section {
    if parts == ["workspace"] {
        return Section::Workspace;
    }
    if parts == ["package"] {
        return Section::Package;
    }

    let dependency_index =
        parts.iter().position(|part| dependency_kind(part).is_some());
    let Some(index) = dependency_index else {
        return Section::Other;
    };
    if index != 0 && parts.first().map(String::as_str) != Some("target") {
        return Section::Other;
    }
    let kind = dependency_kind(&parts[index])
        .expect("index was selected by dependency_kind");
    let target = (index > 0).then(|| parts[1..index].join("."));
    match parts.len().checked_sub(index) {
        Some(1) => Section::DependencyList { kind, target },
        Some(2) => Section::DependencyDetail(DependencyBuilder::new(
            parts[index + 1].clone(),
            kind,
            target,
        )),
        _ => Section::Other,
    }
}

fn is_unsupported_resolution_section(parts: &[String]) -> bool {
    matches!(parts.first().map(String::as_str), Some("patch" | "replace"))
        || begins_workspace_dependencies(parts)
}

fn begins_workspace_dependencies(parts: &[String]) -> bool {
    parts.first().map(String::as_str) == Some("workspace")
        && parts.get(1).map(String::as_str) == Some("dependencies")
}

fn dependency_kind(name: &str) -> Option<DependencyKind> {
    match name {
        "dependencies" => Some(DependencyKind::Normal),
        "dev-dependencies" => Some(DependencyKind::Development),
        "build-dependencies" => Some(DependencyKind::Build),
        _ => None,
    }
}

fn parse_dependency(
    alias: String,
    value: &str,
    kind: DependencyKind,
    target: Option<String>,
    path: &Path,
    line: usize,
) -> Result<Dependency, GraphError> {
    if value.starts_with('"') || value.starts_with('\'') {
        parse_string(value, path, line)?;
        return DependencyBuilder::new(alias, kind, target).finish(path, line);
    }
    let inner = value
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .ok_or_else(|| {
            parse_error(
                path,
                line,
                "dependency must be a string or inline table",
            )
        })?;
    let mut builder = DependencyBuilder::new(alias, kind, target);
    for entry in split_top_level(inner, ',') {
        if entry.trim().is_empty() {
            continue;
        }
        let Some((raw_key, raw_value)) = split_assignment(entry) else {
            return Err(parse_error(
                path,
                line,
                "invalid dependency inline-table entry",
            ));
        };
        let key = parse_key(raw_key.trim(), path, line)?;
        apply_detail(&mut builder, &key, raw_value.trim(), path, line)?;
    }
    builder.finish(path, line)
}

fn apply_detail(
    builder: &mut DependencyBuilder,
    key: &str,
    value: &str,
    path: &Path,
    line: usize,
) -> Result<(), GraphError> {
    match key {
        "path" => {
            builder.path =
                Some(PathBuf::from(parse_string(value, path, line)?));
        }
        "package" => builder.package = Some(parse_string(value, path, line)?),
        "workspace" => builder.workspace = parse_bool(value, path, line)?,
        "optional" => builder.optional = parse_bool(value, path, line)?,
        _ => {}
    }
    Ok(())
}

fn parse_header(
    line: &str,
    path: &Path,
    line_number: usize,
) -> Result<Vec<String>, GraphError> {
    if line.starts_with("[[") {
        return Ok(Vec::new());
    }
    let inner = line
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .ok_or_else(|| {
            parse_error(path, line_number, "malformed table header")
        })?;
    parse_dotted_key(inner, path, line_number)
}

fn parse_dotted_key(
    raw: &str,
    path: &Path,
    line: usize,
) -> Result<Vec<String>, GraphError> {
    split_top_level(raw, '.')
        .into_iter()
        .map(|part| parse_key(part.trim(), path, line))
        .collect()
}

fn inline_table_declares_key(
    raw: &str,
    target: &str,
    path: &Path,
    line: usize,
) -> Result<bool, GraphError> {
    let Some(inner) =
        raw.strip_prefix('{').and_then(|rest| rest.strip_suffix('}'))
    else {
        return Ok(false);
    };
    for entry in split_top_level(inner, ',') {
        if entry.trim().is_empty() {
            continue;
        }
        let Some((raw_key, _)) = split_assignment(entry) else {
            return Err(parse_error(path, line, "invalid inline-table entry"));
        };
        let key = parse_dotted_key(raw_key.trim(), path, line)?;
        if key.first().map(String::as_str) == Some(target) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parse_key(
    raw: &str,
    path: &Path,
    line: usize,
) -> Result<String, GraphError> {
    if raw.starts_with('"') || raw.starts_with('\'') {
        parse_string(raw, path, line)
    } else if !raw.is_empty()
        && raw.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
        })
    {
        Ok(raw.to_owned())
    } else {
        Err(parse_error(path, line, "invalid bare or quoted key"))
    }
}

fn parse_string(
    raw: &str,
    path: &Path,
    line: usize,
) -> Result<String, GraphError> {
    let Some(quote) =
        raw.chars().next().filter(|quote| matches!(quote, '"' | '\''))
    else {
        return Err(parse_error(path, line, "expected quoted string"));
    };
    if raw.len() < 2 || !raw.ends_with(quote) {
        return Err(parse_error(path, line, "unterminated quoted string"));
    }
    let inner = &raw[1..raw.len() - 1];
    if quote == '\'' {
        return Ok(inner.to_owned());
    }

    let mut decoded = String::new();
    let mut characters = inner.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        let Some(escaped) = characters.next() else {
            return Err(parse_error(path, line, "unterminated string escape"));
        };
        decoded.push(match escaped {
            '"' => '"',
            '\\' => '\\',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            _ => {
                return Err(parse_error(
                    path,
                    line,
                    "unsupported string escape",
                ));
            }
        });
    }
    Ok(decoded)
}

fn parse_bool(raw: &str, path: &Path, line: usize) -> Result<bool, GraphError> {
    match raw {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(parse_error(path, line, "expected boolean")),
    }
}

fn strip_comment(line: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match (quote, character) {
            (Some('"'), '\\') => escaped = true,
            (Some(active), current) if active == current => quote = None,
            (None, '"' | '\'') => quote = Some(character),
            (None, '#') => return &line[..index],
            _ => {}
        }
    }
    line
}

fn split_assignment(line: &str) -> Option<(&str, &str)> {
    let mut quote = None;
    let mut escaped = false;
    let mut brace_depth = 0_u32;
    let mut bracket_depth = 0_u32;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match (quote, character) {
            (Some('"'), '\\') => escaped = true,
            (Some(active), current) if active == current => quote = None,
            (None, '"' | '\'') => quote = Some(character),
            (None, '{') => brace_depth += 1,
            (None, '}') => brace_depth = brace_depth.saturating_sub(1),
            (None, '[') => bracket_depth += 1,
            (None, ']') => bracket_depth = bracket_depth.saturating_sub(1),
            (None, '=') if brace_depth == 0 && bracket_depth == 0 => {
                return Some((&line[..index], &line[index + 1..]));
            }
            _ => {}
        }
    }
    None
}

fn split_top_level(value: &str, delimiter: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut brace_depth = 0_u32;
    let mut bracket_depth = 0_u32;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match (quote, character) {
            (Some('"'), '\\') => escaped = true,
            (Some(active), current) if active == current => quote = None,
            (None, '"' | '\'') => quote = Some(character),
            (None, '{') => brace_depth += 1,
            (None, '}') => brace_depth = brace_depth.saturating_sub(1),
            (None, '[') => bracket_depth += 1,
            (None, ']') => bracket_depth = bracket_depth.saturating_sub(1),
            (None, current)
                if current == delimiter
                    && brace_depth == 0
                    && bracket_depth == 0 =>
            {
                parts.push(&value[start..index]);
                start = index + current.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&value[start..]);
    parts
}

fn parse_error(
    path: &Path,
    line: usize,
    message: impl Into<String>,
) -> GraphError {
    GraphError::Parse {
        path: path.to_path_buf(),
        line,
        message: message.into(),
    }
}
