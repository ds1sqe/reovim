//! Exact workspace topology, partition, edge, cycle, and closure checks.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
};

use reovim_depgraph::{SourceOwner, SourceUse};

use crate::{
    cargo::{
        Metadata, Node, Package, ResolvedDependency, Target as CargoTarget,
    },
    finding::{Finding, Findings},
    policy::Policy,
    source::{ClassifiedFile, display_relative},
    topology::{
        Package as ExpectedPackage, Target as ExpectedTarget, Topology,
        Workspace,
    },
};

const RETIRED_NAMES: &[&str] = &[
    "reovim-arch",
    "reovim-uapi",
    "reovim-kabi",
    "reovim-system-kernel",
    "reovim-platform",
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageRecord {
    pub profile: String,
    pub name: String,
    pub manifest: PathBuf,
    pub owners: Vec<SourceOwner>,
    pub targets: Vec<ExpectedTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EdgeRecord {
    pub profile: String,
    pub consumer: String,
    pub provider: String,
    pub kind: String,
    pub target: Option<String>,
    pub consumer_owners: Vec<SourceOwner>,
    pub provider_owners: Vec<SourceOwner>,
    pub decision: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReachabilityRecord {
    pub profile: String,
    pub package: String,
    pub members: Vec<String>,
}

#[derive(Debug)]
pub struct GraphReport {
    pub packages: Vec<PackageRecord>,
    pub edges: Vec<EdgeRecord>,
    pub dependency_reachability: Vec<ReachabilityRecord>,
    pub findings: Vec<Finding>,
}

pub fn check(
    root: &Path,
    profile: &str,
    metadata: &Metadata,
    files: &[ClassifiedFile],
    policy: &Policy,
    topology: &Topology,
) -> GraphReport {
    let mut findings = Findings::new(topology.limits.findings);
    check_workspace_root(root, metadata, topology, &mut findings);
    check_registered_packages(root, profile, metadata, topology, &mut findings);
    let workspace = workspace_for_metadata(root, metadata, topology);
    let expected_packages =
        workspace.map(project_workspace_packages).unwrap_or_default();
    let (mut packages, owners_by_id) = collect_packages(
        root,
        profile,
        metadata,
        files,
        &expected_packages,
        &mut findings,
    );
    let build_scripts: &[PathBuf] =
        workspace.map_or(&[], |selected| selected.build_scripts.as_slice());
    check_build_scripts(profile, &packages, build_scripts, &mut findings);
    let edge_report =
        collect_edges(profile, metadata, &owners_by_id, policy, &mut findings);
    let mut edges = edge_report.edges;
    let concrete_edges = edge_report.concrete;
    let projected_edges = edge_report.projected;
    let package_nodes = metadata
        .workspace_members
        .iter()
        .filter_map(|id| {
            metadata.packages.get(id).map(|package| package.name.clone())
        })
        .collect();
    if let Some(cycle) = find_cycle(&package_nodes, &concrete_edges) {
        findings.push(Finding::new(
            "graph",
            "PACKAGE_CYCLE",
            None,
            format!("profile {profile}: {}", cycle.join(" -> ")),
        ));
    }
    let role_nodes = owners_by_id
        .values()
        .flatten()
        .filter_map(|owner| match owner {
            SourceOwner::Product(role) => Some(*role),
            SourceOwner::RepositoryTooling => None,
        })
        .collect();
    if let Some(cycle) = find_cycle(&role_nodes, &projected_edges) {
        findings.push(Finding::new(
            "graph",
            "PRODUCT_ROLE_CYCLE",
            None,
            format!(
                "profile {profile}: {}",
                cycle
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ),
        ));
    }

    let dependency_reachability = dependency_reachability(profile, metadata);
    packages.sort();
    edges.sort();
    GraphReport {
        packages,
        edges,
        dependency_reachability,
        findings: findings.finish(),
    }
}

fn workspace_for_metadata<'a>(
    root: &Path,
    metadata: &Metadata,
    topology: &'a Topology,
) -> Option<&'a Workspace> {
    let actual = metadata.workspace_root.canonicalize().ok()?;
    topology.workspaces.values().find(|workspace| {
        root.join(&workspace.root)
            .canonicalize()
            .is_ok_and(|candidate| candidate == actual)
    })
}

fn project_workspace_packages(workspace: &Workspace) -> Vec<ExpectedPackage> {
    workspace
        .packages
        .values()
        .map(|package| ExpectedPackage {
            name: package.name.clone(),
            manifest: package.manifest.clone(),
            targets: package
                .targets
                .values()
                .map(|target| ExpectedTarget {
                    name: target.name.clone(),
                    kind: target.kind.clone(),
                    crate_types: target.crate_types.clone(),
                    src_path: target.src_path.clone(),
                })
                .collect(),
        })
        .collect()
}

fn check_workspace_root(
    root: &Path,
    metadata: &Metadata,
    topology: &Topology,
    findings: &mut Findings,
) {
    let canonical = metadata.workspace_root.canonicalize();
    let expected = topology
        .workspaces
        .values()
        .map(|workspace| root.join(&workspace.root))
        .collect::<BTreeSet<_>>();
    if canonical.as_ref().is_ok_and(|path| expected.contains(path)) {
        return;
    }
    findings.push(Finding::new(
        "cargo",
        "CARGO_WORKSPACE_ROOT",
        None,
        format!(
            "metadata workspace root {} is not registered under {}",
            metadata.workspace_root.display(),
            root.display()
        ),
    ));
}

fn check_registered_packages(
    root: &Path,
    profile: &str,
    metadata: &Metadata,
    topology: &Topology,
    findings: &mut Findings,
) {
    for package in metadata.packages.values() {
        check_registered_package(root, profile, package, topology, findings);
    }
}

fn check_registered_package(
    root: &Path,
    profile: &str,
    package: &Package,
    topology: &Topology,
    findings: &mut Findings,
) {
    if RETIRED_NAMES.contains(&package.name.as_str()) {
        findings.push(Finding::new(
            "retired_surface",
            "RETIRED_PACKAGE_NAME",
            None,
            format!(
                "profile {profile}: retired package {} is forbidden",
                package.name
            ),
        ));
    }
    if package.source.is_some() {
        return;
    }
    let manifest = package.manifest_path.strip_prefix(root).ok();
    let resolved = manifest.and_then(|manifest| {
        topology.resolve_source_null_package(
            &package.name,
            &package.version,
            manifest,
        )
    });
    if resolved.is_some() {
        return;
    }
    findings.push(Finding::new(
        "topology",
        "UNREGISTERED_PATH_PACKAGE",
        None,
        format!(
            concat!(
                "profile {}: source-null package {} {} and manifest ",
                "{} do not resolve to one declared workspace package"
            ),
            profile,
            package.name,
            package.version,
            package.manifest_path.display(),
        ),
    ));
}

fn collect_packages(
    root: &Path,
    profile: &str,
    metadata: &Metadata,
    files: &[ClassifiedFile],
    expected_packages: &[ExpectedPackage],
    findings: &mut Findings,
) -> (Vec<PackageRecord>, BTreeMap<String, Vec<SourceOwner>>) {
    let expected = expected_packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let workspace = metadata
        .workspace_members
        .iter()
        .filter_map(|id| metadata.packages.get(id).map(|package| (id, package)))
        .collect::<Vec<_>>();
    check_missing_packages(profile, &expected, &workspace, findings);
    let mut packages = Vec::new();
    let mut owners_by_id = BTreeMap::new();
    for (id, package) in workspace {
        let Some(expected) = expected.get(package.name.as_str()) else {
            report_extra_package(profile, package, findings);
            continue;
        };
        let record =
            check_package(root, profile, package, expected, files, findings);
        owners_by_id.insert((*id).clone(), record.owners.clone());
        packages.push(record);
    }
    (packages, owners_by_id)
}

fn check_missing_packages(
    profile: &str,
    expected: &BTreeMap<&str, &crate::topology::Package>,
    workspace: &[(&String, &Package)],
    findings: &mut Findings,
) {
    let actual = workspace
        .iter()
        .map(|(_, package)| package.name.as_str())
        .collect::<BTreeSet<_>>();
    for name in expected.keys() {
        if !actual.contains(name) {
            findings.push(Finding::new(
                "topology",
                "TOPOLOGY_PACKAGE_MISSING",
                None,
                format!(
                    "profile {profile}: expected package {name} is missing"
                ),
            ));
        }
    }
}

fn report_extra_package(
    profile: &str,
    package: &Package,
    findings: &mut Findings,
) {
    findings.push(Finding::new(
        "topology",
        "TOPOLOGY_PACKAGE_EXTRA",
        None,
        format!(
            "profile {profile}: undeclared workspace package {}",
            package.name
        ),
    ));
}

fn check_package(
    root: &Path,
    profile: &str,
    package: &Package,
    expected: &crate::topology::Package,
    files: &[ClassifiedFile],
    findings: &mut Findings,
) -> PackageRecord {
    let manifest = relative_absolute(
        root,
        &package.manifest_path,
        findings,
        "PACKAGE_MANIFEST",
    )
    .unwrap_or_else(|| expected.manifest.clone());
    check_manifest(profile, &manifest, expected, findings);
    let owners = package_owners(&manifest, files);
    check_package_owners(profile, &manifest, &owners, findings);
    let actual_targets = package
        .targets
        .iter()
        .filter_map(|target| normalize_target(root, target, findings))
        .collect::<BTreeSet<_>>();
    check_targets(
        profile,
        &actual_targets,
        &expected.targets,
        files,
        &owners,
        findings,
    );
    PackageRecord {
        profile: profile.to_owned(),
        name: package.name.clone(),
        manifest,
        owners,
        targets: actual_targets.into_iter().collect(),
    }
}

fn check_manifest(
    profile: &str,
    manifest: &Path,
    expected: &crate::topology::Package,
    findings: &mut Findings,
) {
    if manifest == expected.manifest {
        return;
    }
    findings.push(Finding::new(
        "topology",
        "TOPOLOGY_PACKAGE_MANIFEST",
        Some(display_relative(manifest)),
        format!("profile {profile}: expected {}", expected.manifest.display()),
    ));
}

fn package_owners(
    manifest: &Path,
    files: &[ClassifiedFile],
) -> Vec<SourceOwner> {
    let package_dir = manifest.parent().unwrap_or_else(|| Path::new(""));
    files
        .iter()
        .filter(|file| file.path.starts_with(package_dir))
        .map(|file| file.classification.owner)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn check_package_owners(
    profile: &str,
    manifest: &Path,
    owners: &[SourceOwner],
    findings: &mut Findings,
) {
    if owners.is_empty() {
        findings.push(Finding::new(
            "classification",
            "PACKAGE_OWNER_MISSING",
            Some(display_relative(manifest)),
            format!("profile {profile}: package has no classified files"),
        ));
        return;
    }
    if owners.len() > 1 {
        findings.push(Finding::new(
            "classification",
            "MIXED_OWNER_PACKAGE",
            Some(display_relative(manifest)),
            format!(
                "profile {profile}: package contains {} owners",
                owners.len()
            ),
        ));
    }
}

fn check_targets(
    profile: &str,
    actual: &BTreeSet<ExpectedTarget>,
    expected: &[ExpectedTarget],
    files: &[ClassifiedFile],
    owners: &[SourceOwner],
    findings: &mut Findings,
) {
    let expected = expected.iter().cloned().collect::<BTreeSet<_>>();
    for target in expected.difference(actual) {
        findings.push(Finding::new(
            "topology",
            "TOPOLOGY_TARGET_MISSING",
            Some(display_relative(&target.src_path)),
            format!("profile {profile}: target {} is absent", target.name),
        ));
    }
    for target in actual.difference(&expected) {
        findings.push(Finding::new(
            "topology",
            "TOPOLOGY_TARGET_EXTRA",
            Some(display_relative(&target.src_path)),
            format!("profile {profile}: target {} is undeclared", target.name),
        ));
    }
    for target in actual {
        check_target_classification(profile, target, files, owners, findings);
    }
}

fn check_target_classification(
    profile: &str,
    target: &ExpectedTarget,
    files: &[ClassifiedFile],
    owners: &[SourceOwner],
    findings: &mut Findings,
) {
    let Some(file) = files.iter().find(|file| file.path == target.src_path)
    else {
        findings.push(Finding::new(
            "classification",
            "TARGET_SOURCE_UNCLASSIFIED",
            Some(display_relative(&target.src_path)),
            format!("profile {profile}: target source lacks classification"),
        ));
        return;
    };
    let expected_use = expected_target_use(target, file);
    let classification = &file.classification;
    if classification.source_use == expected_use
        && owners.contains(&classification.owner)
    {
        return;
    }
    findings.push(Finding::new(
        "classification",
        "TARGET_SOURCE_CLASSIFICATION",
        Some(display_relative(&target.src_path)),
        format!(
            concat!(
                "profile {}: target {} requires package owner and ",
                "source use {}"
            ),
            profile,
            target.name,
            expected_use.as_str()
        ),
    ));
}

fn expected_target_use(
    target: &ExpectedTarget,
    file: &ClassifiedFile,
) -> SourceUse {
    if target.kind.iter().any(|kind| matches!(kind.as_str(), "test" | "bench"))
    {
        return SourceUse::Test;
    }
    if target.kind.iter().any(|kind| kind == "example") {
        return SourceUse::Example;
    }
    match file.classification.owner {
        SourceOwner::RepositoryTooling => SourceUse::Tool,
        SourceOwner::Product(_) => file.classification.source_use,
    }
}

fn check_build_scripts(
    profile: &str,
    packages: &[PackageRecord],
    expected_scripts: &[PathBuf],
    findings: &mut Findings,
) {
    let actual = packages
        .iter()
        .flat_map(|package| &package.targets)
        .filter(|target| target.kind.iter().any(|kind| kind == "custom-build"))
        .map(|target| target.src_path.clone())
        .collect::<BTreeSet<_>>();
    let expected = expected_scripts.iter().cloned().collect::<BTreeSet<_>>();
    for path in expected.difference(&actual) {
        findings.push(Finding::new(
            "topology",
            "TOPOLOGY_BUILD_SCRIPT_MISSING",
            Some(display_relative(path)),
            format!(
                concat!(
                    "profile {}: declared build script has no ",
                    "custom-build target"
                ),
                profile
            ),
        ));
    }
    for path in actual.difference(&expected) {
        findings.push(Finding::new(
            "topology",
            "TOPOLOGY_BUILD_SCRIPT_EXTRA",
            Some(display_relative(path)),
            format!(
                concat!(
                    "profile {}: custom-build target is absent from ",
                    "topology"
                ),
                profile
            ),
        ));
    }
}

#[derive(Default)]
struct EdgeReport {
    edges: Vec<EdgeRecord>,
    concrete: BTreeSet<(String, String)>,
    projected: BTreeSet<(
        reovim_depgraph::ArchitecturalRole,
        reovim_depgraph::ArchitecturalRole,
    )>,
}

struct EdgeCollector<'a> {
    profile: &'a str,
    metadata: &'a Metadata,
    owners: &'a BTreeMap<String, Vec<SourceOwner>>,
    policy: &'a Policy,
    findings: &'a mut Findings,
    report: EdgeReport,
}

fn collect_edges(
    profile: &str,
    metadata: &Metadata,
    owners: &BTreeMap<String, Vec<SourceOwner>>,
    policy: &Policy,
    findings: &mut Findings,
) -> EdgeReport {
    let mut collector = EdgeCollector {
        profile,
        metadata,
        owners,
        policy,
        findings,
        report: EdgeReport::default(),
    };
    for (consumer_id, node) in &metadata.nodes {
        collect_node_edges(&mut collector, consumer_id, node);
    }
    collector.report
}

fn collect_node_edges(
    collector: &mut EdgeCollector<'_>,
    consumer_id: &str,
    node: &Node,
) {
    if !collector.metadata.workspace_members.contains(consumer_id) {
        return;
    }
    let Some(consumer) = collector.metadata.packages.get(consumer_id) else {
        return;
    };
    let consumer_owners =
        collector.owners.get(consumer_id).cloned().unwrap_or_default();
    for dependency in &node.dependencies {
        collect_dependency_edge(
            collector,
            consumer,
            &consumer_owners,
            dependency,
        );
    }
}

fn collect_dependency_edge(
    collector: &mut EdgeCollector<'_>,
    consumer: &Package,
    consumer_owners: &[SourceOwner],
    dependency: &ResolvedDependency,
) {
    check_retired_dependency(collector, dependency);
    if !collector.metadata.workspace_members.contains(&dependency.package) {
        return;
    }
    let Some(provider) = collector.metadata.packages.get(&dependency.package)
    else {
        return;
    };
    let provider_owners =
        collector.owners.get(&dependency.package).cloned().unwrap_or_default();
    collector
        .report
        .concrete
        .insert((consumer.name.clone(), provider.name.clone()));
    add_projected_edges(
        &mut collector.report.projected,
        consumer_owners,
        &provider_owners,
    );
    let decision =
        edge_decision(consumer_owners, &provider_owners, collector.policy);
    report_forbidden_edge(collector, consumer, provider, decision);
    let inputs = EdgeInputs {
        profile: collector.profile,
        consumer,
        provider,
        consumer_owners,
        provider_owners: &provider_owners,
        dependency,
        decision,
    };
    push_edge_kinds(&inputs, &mut collector.report.edges);
}

struct EdgeInputs<'a> {
    profile: &'a str,
    consumer: &'a Package,
    provider: &'a Package,
    consumer_owners: &'a [SourceOwner],
    provider_owners: &'a [SourceOwner],
    dependency: &'a ResolvedDependency,
    decision: &'static str,
}

fn check_retired_dependency(
    collector: &mut EdgeCollector<'_>,
    dependency: &ResolvedDependency,
) {
    if !RETIRED_NAMES.contains(&dependency.name.as_str()) {
        return;
    }
    collector.findings.push(Finding::new(
        "retired_surface",
        "RETIRED_DEPENDENCY_NAME",
        None,
        format!(
            "profile {}: retired dependency alias {} is forbidden",
            collector.profile, dependency.name
        ),
    ));
}

fn add_projected_edges(
    edges: &mut BTreeSet<(
        reovim_depgraph::ArchitecturalRole,
        reovim_depgraph::ArchitecturalRole,
    )>,
    consumers: &[SourceOwner],
    providers: &[SourceOwner],
) {
    for consumer in consumers {
        for provider in providers {
            add_projected_edge(edges, *consumer, *provider);
        }
    }
}

fn add_projected_edge(
    edges: &mut BTreeSet<(
        reovim_depgraph::ArchitecturalRole,
        reovim_depgraph::ArchitecturalRole,
    )>,
    consumer: SourceOwner,
    provider: SourceOwner,
) {
    let (SourceOwner::Product(consumer), SourceOwner::Product(provider)) =
        (consumer, provider)
    else {
        return;
    };
    if consumer != provider {
        edges.insert((consumer, provider));
    }
}

fn report_forbidden_edge(
    collector: &mut EdgeCollector<'_>,
    consumer: &Package,
    provider: &Package,
    decision: &'static str,
) {
    if !decision.starts_with("forbidden") {
        return;
    }
    let code = if decision == "forbidden_partition" {
        "FORBIDDEN_PARTITION_EDGE"
    } else {
        "FORBIDDEN_PRODUCT_EDGE"
    };
    collector.findings.push(Finding::new(
        "partition",
        code,
        None,
        format!(
            "profile {}: {} -> {} is {decision}",
            collector.profile, consumer.name, provider.name
        ),
    ));
}

fn push_edge_kinds(inputs: &EdgeInputs<'_>, edges: &mut Vec<EdgeRecord>) {
    for kind in &inputs.dependency.kinds {
        edges.push(EdgeRecord {
            profile: inputs.profile.to_owned(),
            consumer: inputs.consumer.name.clone(),
            provider: inputs.provider.name.clone(),
            kind: kind.kind.clone(),
            target: kind.target.clone(),
            consumer_owners: inputs.consumer_owners.to_vec(),
            provider_owners: inputs.provider_owners.to_vec(),
            decision: inputs.decision,
        });
    }
}

fn edge_decision(
    consumers: &[SourceOwner],
    providers: &[SourceOwner],
    policy: &Policy,
) -> &'static str {
    if consumers.is_empty() || providers.is_empty() {
        return "forbidden_partition";
    }
    match edge_decision_result(consumers, providers, policy) {
        Ok(true) => "admitted_product_edge",
        Ok(false) => "admitted_same_owner",
        Err(decision) => decision,
    }
}

fn edge_decision_result(
    consumers: &[SourceOwner],
    providers: &[SourceOwner],
    policy: &Policy,
) -> Result<bool, &'static str> {
    let mut saw_product_edge = false;
    for consumer in consumers {
        for provider in providers {
            check_owner_pair(
                *consumer,
                *provider,
                policy,
                &mut saw_product_edge,
            )?;
        }
    }
    Ok(saw_product_edge)
}

fn check_owner_pair(
    consumer: SourceOwner,
    provider: SourceOwner,
    policy: &Policy,
    saw_product_edge: &mut bool,
) -> Result<(), &'static str> {
    match (consumer, provider) {
        (SourceOwner::RepositoryTooling, SourceOwner::RepositoryTooling) => {
            Ok(())
        }
        (SourceOwner::Product(consumer), SourceOwner::Product(provider))
            if consumer == provider =>
        {
            Ok(())
        }
        (SourceOwner::Product(consumer), SourceOwner::Product(provider))
            if policy.allowed_product_edges.contains(&(consumer, provider)) =>
        {
            *saw_product_edge = true;
            Ok(())
        }
        (SourceOwner::Product(_), SourceOwner::Product(_)) => {
            Err("forbidden_product_edge")
        }
        _ => Err("forbidden_partition"),
    }
}

fn dependency_reachability(
    profile: &str,
    metadata: &Metadata,
) -> Vec<ReachabilityRecord> {
    let mut reachability = Vec::new();
    for root_id in &metadata.workspace_members {
        let Some(root_package) = metadata.packages.get(root_id) else {
            continue;
        };
        let member_ids = reachable_member_ids(root_id, metadata);
        let members = member_ids
            .iter()
            .filter_map(|id| {
                metadata
                    .packages
                    .get(id)
                    .map(crate::cargo::Package::review_identity)
            })
            .collect();
        reachability.push(ReachabilityRecord {
            profile: profile.to_owned(),
            package: root_package.name.clone(),
            members,
        });
    }
    reachability.sort();
    reachability
}

fn reachable_member_ids(
    root_id: &str,
    metadata: &Metadata,
) -> BTreeSet<String> {
    let mut members = BTreeSet::new();
    let mut pending = VecDeque::from([root_id.to_owned()]);
    while let Some(id) = pending.pop_front() {
        if !members.insert(id.clone()) {
            continue;
        }
        enqueue_dependencies(metadata.nodes.get(&id), &mut pending);
    }
    members
}

fn enqueue_dependencies(node: Option<&Node>, pending: &mut VecDeque<String>) {
    let Some(node) = node else {
        return;
    };
    pending.extend(
        node.dependencies
            .iter()
            .filter(|dependency| {
                dependency
                    .kinds
                    .iter()
                    .any(|kind| kind.kind == "normal" || kind.kind == "build")
            })
            .map(|dependency| dependency.package.clone()),
    );
}

fn normalize_target(
    root: &Path,
    target: &CargoTarget,
    findings: &mut Findings,
) -> Option<ExpectedTarget> {
    let src_path =
        relative_absolute(root, &target.src_path, findings, "TARGET_SOURCE")?;
    Some(ExpectedTarget {
        name: target.name.clone(),
        kind: target.kind.clone(),
        crate_types: target.crate_types.clone(),
        src_path,
    })
}

fn relative_absolute(
    root: &Path,
    path: &Path,
    findings: &mut Findings,
    code: &'static str,
) -> Option<PathBuf> {
    if !path.is_absolute() {
        findings.push(Finding::new(
            "path",
            code,
            None,
            format!("Cargo path {} is not absolute", path.display()),
        ));
        return None;
    }
    let canonical = match path.canonicalize() {
        Ok(path) => path,
        Err(error) => {
            findings.push(Finding::new(
                "path",
                code,
                None,
                format!("{}: {error}", path.display()),
            ));
            return None;
        }
    };
    canonical.strip_prefix(root).map_or_else(
        |_| {
            findings.push(Finding::new(
                "path",
                "ROOT_ESCAPE",
                None,
                format!(
                    "Cargo path {} escapes checked root",
                    canonical.display()
                ),
            ));
            None
        },
        |relative| Some(relative.to_path_buf()),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    Active,
    Complete,
}

fn visit_cycle<T>(
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
            let start =
                stack.iter().position(|entry| entry == node).unwrap_or(0);
            let mut cycle = stack[start..].to_vec();
            cycle.push(node.clone());
            return Some(cycle);
        }
        None => {}
    }
    marks.insert(node.clone(), Mark::Active);
    stack.push(node.clone());
    for (_, provider) in edges.iter().filter(|(consumer, _)| consumer == node) {
        if let Some(cycle) = visit_cycle(provider, edges, marks, stack) {
            return Some(cycle);
        }
    }
    stack.pop();
    marks.insert(node.clone(), Mark::Complete);
    None
}

fn find_cycle<T>(
    nodes: &BTreeSet<T>,
    edges: &BTreeSet<(T, T)>,
) -> Option<Vec<T>>
where
    T: Clone + Ord,
{
    let mut marks = BTreeMap::new();
    let mut stack = Vec::new();
    for node in nodes {
        if let Some(cycle) = visit_cycle(node, edges, &mut marks, &mut stack) {
            return Some(cycle);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        path::{Path, PathBuf},
    };

    use reovim_depgraph::{ArchitecturalRole, SourceOwner};

    use crate::{cargo::Metadata, policy::Policy, topology::Topology};

    fn policy() -> Policy {
        Policy {
            scope_roots: vec![PathBuf::new()],
            exclusions: Vec::new(),
            rules: Vec::new(),
            allowed_product_edges: BTreeSet::new(),
            generated_evidence_key: PathBuf::from(
                "conformance/source-role-map.json",
            ),
        }
    }

    #[test]
    fn partition_edges_fail_in_both_directions_and_mixed_sets_fail() {
        let product = SourceOwner::Product(ArchitecturalRole::Foundation);
        let tooling = SourceOwner::RepositoryTooling;
        assert_eq!(
            super::edge_decision(&[product], &[tooling], &policy()),
            "forbidden_partition"
        );
        assert_eq!(
            super::edge_decision(&[tooling], &[product], &policy()),
            "forbidden_partition"
        );
        assert_eq!(
            super::edge_decision(&[tooling, product], &[tooling], &policy()),
            "forbidden_partition"
        );
    }

    #[test]
    fn unallowed_product_edge_and_cycle_are_detected() {
        let foundation = SourceOwner::Product(ArchitecturalRole::Foundation);
        let kernel = SourceOwner::Product(ArchitecturalRole::KernelInterface);
        assert_eq!(
            super::edge_decision(&[foundation], &[kernel], &policy()),
            "forbidden_product_edge"
        );
        let nodes = BTreeSet::from(["a", "b"]);
        let edges = BTreeSet::from([("a", "b"), ("b", "a")]);
        assert_eq!(
            super::find_cycle(&nodes, &edges),
            Some(vec!["a", "b", "a"])
        );
    }

    #[test]
    fn dual_workspace_selection_uses_the_matching_package_set() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("repository root");
        let bytes = fs::read(root.join("conformance/workspace-topology.json"))
            .expect("topology bytes");
        let value = serde_json::from_slice(&bytes).expect("topology JSON");
        let topology = Topology::parse(&value).expect("topology");
        let metadata = Metadata {
            workspace_root: root.join("product"),
            workspace_members: BTreeSet::new(),
            packages: BTreeMap::new(),
            nodes: BTreeMap::new(),
        };
        let workspace =
            super::workspace_for_metadata(&root, &metadata, &topology)
                .expect("product workspace");
        assert_eq!(workspace.id, "product");
        let names = super::project_workspace_packages(workspace)
            .into_iter()
            .map(|package| package.name)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            names,
            BTreeSet::from([
                "reovim-foundation".to_owned(),
                "reovim-foundation-public-consumer".to_owned(),
                "reovim-kernel-interface".to_owned(),
                "reovim-kernel-interface-public-consumer".to_owned(),
            ])
        );
    }
}
