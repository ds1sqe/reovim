//! Preflight over dependency declarations in Cargo manifests.
//!
//! This crate does not invoke Cargo's resolver and cannot establish Reovim
//! architecture conformance. It discovers manifest declarations, resolves the
//! supported local `path` subset, classifies package directories with a
//! caller-supplied path primitive, and performs conservative coarse-role and
//! cycle preflights. Cargo metadata from the exact build graph plus separate
//! source/import, transitive-closure, public-type, dependency-review, and
//! Source Role Map artifact checks are still required for `ARCH-*` evidence.
//!
//! The crate deliberately contains no repository path catalog or built-in
//! architecture edge matrix. Recognized manifest-level `[patch]`, `[replace]`,
//! and `[workspace.dependencies]` indirection fails closed. This preflight does
//! not inspect `.cargo` configuration, Cargo command-line/environment inputs,
//! or other resolver inputs; exact Cargo metadata remains authoritative.

#![forbid(unsafe_code)]

mod manifest;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::{Component, Path, PathBuf},
};

pub use manifest::{
    DeclaredManifestGraph, Dependency, DependencyKind, DependencySource, DiscoveryOptions,
    GraphError, LocalDependencyEdge, Package, load_declared_manifest_graph,
};

/// The logical product roles defined by Reovim's Layer and Dependency Model.
///
/// These are ownership roles, not crate names or directory conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArchitecturalRole {
    Foundation,
    KernelInterface,
    KernelCore,
    KernelRuntime,
    PublicSdk,
    ConcretePolicy,
    ResourceAdapter,
    HostAdapter,
    ExternalProtocolBinding,
    ProtocolAdapter,
    ExtensionAdapter,
    ExtensionEngineAdapter,
    ClientFamily,
    ClientRuntime,
    PresentationAdapter,
    Assembly,
}

impl ArchitecturalRole {
    /// All ratified roles in inward-to-outward documentation order.
    pub const ALL: [Self; 16] = [
        Self::Foundation,
        Self::KernelInterface,
        Self::KernelCore,
        Self::KernelRuntime,
        Self::PublicSdk,
        Self::ConcretePolicy,
        Self::ResourceAdapter,
        Self::HostAdapter,
        Self::ExternalProtocolBinding,
        Self::ProtocolAdapter,
        Self::ExtensionAdapter,
        Self::ExtensionEngineAdapter,
        Self::ClientFamily,
        Self::ClientRuntime,
        Self::PresentationAdapter,
        Self::Assembly,
    ];

    /// Returns the normative role spelling used in reports and artifacts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Foundation => "Foundation",
            Self::KernelInterface => "Kernel Interface",
            Self::KernelCore => "Kernel Core",
            Self::KernelRuntime => "Kernel Runtime",
            Self::PublicSdk => "Public SDKs",
            Self::ConcretePolicy => "Concrete policy",
            Self::ResourceAdapter => "Resource Adapter",
            Self::HostAdapter => "Host Adapter",
            Self::ExternalProtocolBinding => "External Protocol Binding",
            Self::ProtocolAdapter => "Protocol Adapter",
            Self::ExtensionAdapter => "Extension Adapter",
            Self::ExtensionEngineAdapter => "Extension Engine Adapter",
            Self::ClientFamily => "Client Family",
            Self::ClientRuntime => "Client Runtime",
            Self::PresentationAdapter => "Presentation Adapter",
            Self::Assembly => "Assembly",
        }
    }
}

impl fmt::Display for ArchitecturalRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The authority that owns a classified source path.
///
/// Repository tooling is deliberately outside the product role graph. It is
/// not another [`ArchitecturalRole`] and cannot acquire a product edge by
/// sharing a package or directory with product source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceOwner {
    Product(ArchitecturalRole),
    RepositoryTooling,
}

impl SourceOwner {
    /// Returns the stable owner spelling used by deterministic reports.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Product(role) => role.as_str(),
            Self::RepositoryTooling => "RepositoryTooling",
        }
    }
}

impl fmt::Display for SourceOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// How source bytes originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceProvenance {
    Authored,
    Generated,
    Vendored,
}

impl SourceProvenance {
    /// Returns the stable lower-snake-case report spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authored => "authored",
            Self::Generated => "generated",
            Self::Vendored => "vendored",
        }
    }
}

/// How a classified path participates in the repository or product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceUse {
    Production,
    Test,
    Example,
    Tool,
    BuildOutput,
}

impl SourceUse {
    /// Returns the stable lower-snake-case report spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Test => "test",
            Self::Example => "example",
            Self::Tool => "tool",
            Self::BuildOutput => "build_output",
        }
    }
}

/// A source owner/provenance/use combination rejected by the closed model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidSourceClassification {
    pub owner: SourceOwner,
    pub provenance: SourceProvenance,
    pub source_use: SourceUse,
}

impl fmt::Display for InvalidSourceClassification {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "source owner {} with provenance {} cannot have use {}",
            self.owner,
            self.provenance.as_str(),
            self.source_use.as_str()
        )
    }
}

impl std::error::Error for InvalidSourceClassification {}

/// The three independent classifications assigned to one source path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceClassification {
    pub owner: SourceOwner,
    pub provenance: SourceProvenance,
    pub source_use: SourceUse,
}

impl SourceClassification {
    /// Constructs a classification under the closed owner/use law.
    ///
    /// # Errors
    ///
    /// Product owners permit production, test, and example source. Repository
    /// tooling permits tool, test, example, and build-output source. Every
    /// other owner/use combination is rejected.
    pub const fn new(
        owner: SourceOwner,
        provenance: SourceProvenance,
        source_use: SourceUse,
    ) -> Result<Self, InvalidSourceClassification> {
        let owner_use_valid = matches!(
            (owner, source_use),
            (SourceOwner::Product(_), SourceUse::Production | SourceUse::Test | SourceUse::Example)
                | (
                    SourceOwner::RepositoryTooling,
                    SourceUse::Tool | SourceUse::Test | SourceUse::Example | SourceUse::BuildOutput
                )
        );
        let provenance_use_valid = !matches!(source_use, SourceUse::BuildOutput)
            || matches!(provenance, SourceProvenance::Generated);
        if owner_use_valid && provenance_use_valid {
            Ok(Self { owner, provenance, source_use })
        } else {
            Err(InvalidSourceClassification { owner, provenance, source_use })
        }
    }

    /// Constructs authored production source for one product role.
    #[must_use]
    pub const fn product(owner: ArchitecturalRole) -> Self {
        Self {
            owner: SourceOwner::Product(owner),
            provenance: SourceProvenance::Authored,
            source_use: SourceUse::Production,
        }
    }

    /// Constructs authored repository-tool source.
    #[must_use]
    pub const fn repository_tool() -> Self {
        Self {
            owner: SourceOwner::RepositoryTooling,
            provenance: SourceProvenance::Authored,
            source_use: SourceUse::Tool,
        }
    }
}

/// One prefix rule in a versioned path-classification primitive.
///
/// Greater precedence wins. Equal-precedence matches are rejected, even when
/// they carry identical assignments, so overlap cannot become accidental.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceClassificationRule {
    pub prefix: PathBuf,
    pub precedence: u32,
    pub classification: SourceClassification,
}

impl SourceClassificationRule {
    /// Constructs a source-classification rule.
    pub fn new(
        prefix: impl Into<PathBuf>,
        precedence: u32,
        classification: SourceClassification,
    ) -> Self {
        Self { prefix: prefix.into(), precedence, classification }
    }
}

/// A fail-closed path-classification primitive for one source revision.
///
/// This is not the normative Source Role Map evidence artifact. It does not
/// enumerate source files, record source/build graph identity, carry checker
/// evidence, or prove generated/vendored/test/example/tool/build-output
/// coverage. Those obligations belong to the conformance artifact and its
/// source-level checker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceClassificationMap {
    schema_version: u32,
    rules: Vec<SourceClassificationRule>,
}

impl SourceClassificationMap {
    /// Constructs a map and normalizes every relative rule prefix.
    ///
    /// # Errors
    ///
    /// Returns [`ClassificationMapError`] if a prefix is absolute or escapes
    /// the source root.
    pub fn new(
        schema_version: u32,
        rules: impl IntoIterator<Item = SourceClassificationRule>,
    ) -> Result<Self, ClassificationMapError> {
        if schema_version == 0 {
            return Err(ClassificationMapError::InvalidSchemaVersion);
        }
        let mut normalized = Vec::new();
        for mut rule in rules {
            rule.prefix = normalize_relative(&rule.prefix)?;
            normalized.push(rule);
        }
        Ok(Self { schema_version, rules: normalized })
    }

    /// Returns this classification primitive's schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the ordered path rules retained by the map.
    #[must_use]
    pub fn rules(&self) -> &[SourceClassificationRule] {
        &self.rules
    }

    /// Classifies one path relative to the checked source root.
    ///
    /// # Errors
    ///
    /// Absolute, escaping, unmatched, and equal-precedence multiply matched
    /// paths are errors.
    pub fn classify(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<SourceClassification, ClassificationMapError> {
        let normalized = normalize_relative(path.as_ref())?;
        let mut winners: Vec<&SourceClassificationRule> = Vec::new();
        let mut winning_precedence = None;

        for rule in &self.rules {
            if normalized.starts_with(&rule.prefix) {
                match winning_precedence {
                    None => {
                        winning_precedence = Some(rule.precedence);
                        winners.push(rule);
                    }
                    Some(current) if rule.precedence > current => {
                        winning_precedence = Some(rule.precedence);
                        winners.clear();
                        winners.push(rule);
                    }
                    Some(current) if rule.precedence == current => winners.push(rule),
                    Some(_) => {}
                }
            }
        }

        match winners.as_slice() {
            [] => Err(ClassificationMapError::Unmatched { path: normalized }),
            [winner] => Ok(winner.classification),
            _ => Err(ClassificationMapError::Ambiguous {
                path: normalized,
                precedence: winning_precedence.unwrap_or_default(),
                prefixes: winners.iter().map(|rule| rule.prefix.clone()).collect(),
            }),
        }
    }
}

/// A path-classification primitive construction or lookup failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassificationMapError {
    InvalidSchemaVersion,
    AbsolutePath { path: PathBuf },
    EscapesRoot { path: PathBuf },
    Unmatched { path: PathBuf },
    Ambiguous { path: PathBuf, precedence: u32, prefixes: Vec<PathBuf> },
}

impl fmt::Display for ClassificationMapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchemaVersion => {
                formatter.write_str("source-role schema version must be non-zero")
            }
            Self::AbsolutePath { path } => {
                write!(formatter, "source path must be relative: {}", path.display())
            }
            Self::EscapesRoot { path } => {
                write!(formatter, "source path escapes its root: {}", path.display())
            }
            Self::Unmatched { path } => {
                write!(formatter, "source path has no role: {}", path.display())
            }
            Self::Ambiguous { path, precedence, prefixes } => write!(
                formatter,
                "source path {} has {} matches at precedence {}: {}",
                path.display(),
                prefixes.len(),
                precedence,
                prefixes
                    .iter()
                    .map(|prefix| prefix.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl std::error::Error for ClassificationMapError {}

/// Caller-selected coarse-role edges for declared-manifest preflight.
///
/// Architectural roles compress contract facets and private mechanism edges,
/// so this policy cannot encode or prove the normative Layer Model. Same-role
/// package dependencies are admitted automatically; concrete package cycles
/// are checked separately.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreflightRolePolicy {
    allowed: BTreeSet<(ArchitecturalRole, ArchitecturalRole)>,
}

impl PreflightRolePolicy {
    /// Constructs a policy denying every cross-role edge.
    #[must_use]
    pub const fn new() -> Self {
        Self { allowed: BTreeSet::new() }
    }

    /// Adds one `consumer -> provider` role edge.
    #[must_use]
    pub fn allow(mut self, consumer: ArchitecturalRole, provider: ArchitecturalRole) -> Self {
        self.allowed.insert((consumer, provider));
        self
    }

    /// Returns whether an edge is same-role or explicitly admitted.
    #[must_use]
    pub fn allows(&self, consumer: ArchitecturalRole, provider: ArchitecturalRole) -> bool {
        consumer == provider || self.allowed.contains(&(consumer, provider))
    }
}

/// One conservative declared-manifest preflight finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreflightIssue {
    SourceClassification {
        package: String,
        error: ClassificationMapError,
    },
    ForbiddenPartitionDependency {
        consumer: String,
        consumer_owner: SourceOwner,
        provider: String,
        provider_owner: SourceOwner,
        kind: DependencyKind,
        target: Option<String>,
    },
    ForbiddenRoleDependency {
        consumer: String,
        consumer_role: ArchitecturalRole,
        provider: String,
        provider_role: ArchitecturalRole,
        kind: DependencyKind,
        target: Option<String>,
    },
    PackageCycle {
        packages: Vec<String>,
    },
    RoleCycle {
        roles: Vec<ArchitecturalRole>,
    },
}

impl fmt::Display for PreflightIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceClassification { package, error } => {
                write!(formatter, "package {package}: {error}")
            }
            Self::ForbiddenPartitionDependency {
                consumer,
                consumer_owner,
                provider,
                provider_owner,
                kind,
                target,
            } => {
                write!(
                    formatter,
                    "forbidden {kind} partition dependency: {consumer} ({consumer_owner}) -> {provider} ({provider_owner})"
                )?;
                if let Some(target) = target {
                    write!(formatter, " for target {target}")?;
                }
                Ok(())
            }
            Self::ForbiddenRoleDependency {
                consumer,
                consumer_role,
                provider,
                provider_role,
                kind,
                target,
            } => {
                write!(
                    formatter,
                    "forbidden {kind} dependency: {consumer} ({consumer_role}) -> {provider} ({provider_role})"
                )?;
                if let Some(target) = target {
                    write!(formatter, " for target {target}")?;
                }
                Ok(())
            }
            Self::PackageCycle { packages } => {
                write!(formatter, "package dependency cycle: {}", packages.join(" -> "))
            }
            Self::RoleCycle { roles } => write!(
                formatter,
                "architectural role cycle: {}",
                roles.iter().map(ToString::to_string).collect::<Vec<_>>().join(" -> ")
            ),
        }
    }
}

/// Deterministic declared-manifest preflight output.
///
/// `package_classifications` contains classifications of package directories only. It is
/// not source-file coverage or a normative Source Role Map artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightReport {
    pub package_classifications: BTreeMap<String, SourceClassification>,
    pub issues: Vec<PreflightIssue>,
}

impl PreflightReport {
    /// Returns true only when no finding was recorded.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
}

/// Preflights package-directory classifications, declared local role edges,
/// and concrete and role-projected cycles.
///
/// Only package directories are passed to `classification_map`; source files
/// are not enumerated or classified. A clean report is therefore not `ARCH-*`
/// conformance. The exact Cargo-resolved graph and the independent source,
/// import, transitive-closure, public-type, dependency-review, and normative
/// Source Role Map evidence checks remain required.
#[must_use]
pub fn preflight_declared_graph(
    graph: &DeclaredManifestGraph,
    classification_map: &SourceClassificationMap,
    policy: &PreflightRolePolicy,
) -> PreflightReport {
    let mut package_classifications = BTreeMap::new();
    let mut issues = Vec::new();

    for package in graph.packages().values() {
        match classification_map.classify(&package.relative_dir) {
            Ok(classification) => {
                package_classifications.insert(package.name.clone(), classification);
            }
            Err(error) => {
                issues.push(PreflightIssue::SourceClassification {
                    package: package.name.clone(),
                    error,
                });
            }
        }
    }

    for edge in graph.local_edges() {
        let Some(consumer) = package_classifications.get(&edge.consumer).copied() else {
            continue;
        };
        let Some(provider) = package_classifications.get(&edge.provider).copied() else {
            continue;
        };
        match (consumer.owner, provider.owner) {
            (SourceOwner::Product(consumer_role), SourceOwner::Product(provider_role)) => {
                if !policy.allows(consumer_role, provider_role) {
                    issues.push(PreflightIssue::ForbiddenRoleDependency {
                        consumer: edge.consumer.clone(),
                        consumer_role,
                        provider: edge.provider.clone(),
                        provider_role,
                        kind: edge.kind,
                        target: edge.target.clone(),
                    });
                }
            }
            (SourceOwner::RepositoryTooling, SourceOwner::RepositoryTooling) => {}
            (consumer_owner, provider_owner) => {
                issues.push(PreflightIssue::ForbiddenPartitionDependency {
                    consumer: edge.consumer.clone(),
                    consumer_owner,
                    provider: edge.provider.clone(),
                    provider_owner,
                    kind: edge.kind,
                    target: edge.target.clone(),
                });
            }
        }
    }

    let package_nodes = graph.packages().keys().cloned().collect::<BTreeSet<_>>();
    let package_edges = graph
        .local_edges()
        .iter()
        .map(|edge| (edge.consumer.clone(), edge.provider.clone()))
        .collect::<BTreeSet<_>>();
    if let Some(packages) = find_cycle(&package_nodes, &package_edges) {
        issues.push(PreflightIssue::PackageCycle { packages });
    }

    let mut role_nodes = BTreeSet::new();
    let mut role_edges = BTreeSet::new();
    for classification in package_classifications.values() {
        if let SourceOwner::Product(role) = classification.owner {
            role_nodes.insert(role);
        }
    }
    for edge in graph.local_edges() {
        if let (Some(consumer), Some(provider)) = (
            package_classifications.get(&edge.consumer),
            package_classifications.get(&edge.provider),
        ) && let (SourceOwner::Product(consumer), SourceOwner::Product(provider)) =
            (consumer.owner, provider.owner)
            && consumer != provider
        {
            role_edges.insert((consumer, provider));
        }
    }
    if let Some(roles) = find_cycle(&role_nodes, &role_edges) {
        issues.push(PreflightIssue::RoleCycle { roles });
    }

    PreflightReport { package_classifications, issues }
}

fn normalize_relative(path: &Path) -> Result<PathBuf, ClassificationMapError> {
    let original = path.to_path_buf();
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(ClassificationMapError::EscapesRoot { path: original });
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ClassificationMapError::AbsolutePath { path: original });
            }
        }
    }
    Ok(normalized)
}

fn find_cycle<T>(nodes: &BTreeSet<T>, edges: &BTreeSet<(T, T)>) -> Option<Vec<T>>
where
    T: Clone + Ord,
{
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        Active,
        Complete,
    }

    fn visit<T>(
        node: &T,
        edges: &BTreeSet<(T, T)>,
        marks: &mut BTreeMap<T, Mark>,
        stack: &mut Vec<T>,
    ) -> Option<Vec<T>>
    where
        T: Clone + Ord,
    {
        match marks.get(node) {
            Some(Mark::Complete) => return None,
            Some(Mark::Active) => {
                let start = stack.iter().position(|entry| entry == node).unwrap_or(0);
                let mut cycle = stack[start..].to_vec();
                cycle.push(node.clone());
                return Some(cycle);
            }
            None => {}
        }

        marks.insert(node.clone(), Mark::Active);
        stack.push(node.clone());
        for (_, provider) in edges.iter().filter(|(consumer, _)| consumer == node) {
            if let Some(cycle) = visit(provider, edges, marks, stack) {
                return Some(cycle);
            }
        }
        stack.pop();
        marks.insert(node.clone(), Mark::Complete);
        None
    }

    let mut marks = BTreeMap::new();
    let mut stack = Vec::new();
    for node in nodes {
        if let Some(cycle) = visit(node, edges, &mut marks, &mut stack) {
            return Some(cycle);
        }
    }
    None
}
