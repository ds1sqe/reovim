//! Exact workspace topology, partition, edge, cycle, and closure checks.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
};

use reovim_depgraph::{SourceOwner, SourceUse};

use crate::{
    cargo::{Metadata, Target as CargoTarget},
    finding::{Finding, Findings},
    policy::Policy,
    source::{ClassifiedFile, display_relative},
    topology::{Target as ExpectedTarget, Topology},
};

const RETIRED_NAMES: &[&str] =
    &["reovim-arch", "reovim-uapi", "reovim-kabi", "reovim-system-kernel", "reovim-platform"];

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

#[allow(clippy::too_many_lines)]
pub fn check(
    root: &Path,
    profile: &str,
    metadata: &Metadata,
    files: &[ClassifiedFile],
    policy: &Policy,
    topology: &Topology,
) -> GraphReport {
    let mut findings = Findings::new(topology.limits.findings);
    let canonical_metadata_root = metadata.workspace_root.canonicalize();
    if !matches!(canonical_metadata_root.as_deref(), Ok(path) if path == root) {
        findings.push(Finding::new(
            "cargo",
            "CARGO_WORKSPACE_ROOT",
            None,
            format!(
                "metadata workspace root {} does not equal checked root {}",
                metadata.workspace_root.display(),
                root.display()
            ),
        ));
    }

    let mut packages = Vec::new();
    let mut owners_by_id = BTreeMap::new();
    for package in metadata.packages.values() {
        if RETIRED_NAMES.contains(&package.name.as_str()) {
            findings.push(Finding::new(
                "retired_surface",
                "RETIRED_PACKAGE_NAME",
                None,
                format!("profile {profile}: retired package {} is forbidden", package.name),
            ));
        }
        if package.source.is_none() {
            let resolved = package.manifest_path.strip_prefix(root).ok().and_then(|manifest| {
                topology.resolve_source_null_package(&package.name, &package.version, manifest)
            });
            if resolved.is_none() {
                findings.push(Finding::new(
                    "topology",
                    "UNREGISTERED_PATH_PACKAGE",
                    None,
                    format!(
                        "profile {profile}: source-null package {} {} and manifest {} do not resolve to one declared workspace package",
                        package.name,
                        package.version,
                        package.manifest_path.display(),
                    ),
                ));
            }
        }
    }
    let expected_by_name = topology
        .packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let workspace_packages = metadata
        .workspace_members
        .iter()
        .filter_map(|id| metadata.packages.get(id).map(|package| (id, package)))
        .collect::<Vec<_>>();
    let actual_names = workspace_packages
        .iter()
        .map(|(_, package)| package.name.as_str())
        .collect::<BTreeSet<_>>();
    for expected in expected_by_name.keys() {
        if !actual_names.contains(expected) {
            findings.push(Finding::new(
                "topology",
                "TOPOLOGY_PACKAGE_MISSING",
                None,
                format!("profile {profile}: expected package {expected} is missing"),
            ));
        }
    }
    for (id, package) in workspace_packages {
        let Some(expected) = expected_by_name.get(package.name.as_str()) else {
            findings.push(Finding::new(
                "topology",
                "TOPOLOGY_PACKAGE_EXTRA",
                None,
                format!("profile {profile}: undeclared workspace package {}", package.name),
            ));
            continue;
        };
        let manifest =
            relative_absolute(root, &package.manifest_path, &mut findings, "PACKAGE_MANIFEST");
        let manifest = manifest.unwrap_or_else(|| expected.manifest.clone());
        if manifest != expected.manifest {
            findings.push(Finding::new(
                "topology",
                "TOPOLOGY_PACKAGE_MANIFEST",
                Some(display_relative(&manifest)),
                format!("profile {profile}: expected {}", expected.manifest.display()),
            ));
        }
        let package_dir = manifest.parent().unwrap_or_else(|| Path::new(""));
        let owners = files
            .iter()
            .filter(|file| file.path.starts_with(package_dir))
            .map(|file| file.classification.owner)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if owners.is_empty() {
            findings.push(Finding::new(
                "classification",
                "PACKAGE_OWNER_MISSING",
                Some(display_relative(&manifest)),
                format!("profile {profile}: package has no classified files"),
            ));
        } else if owners.len() > 1 {
            findings.push(Finding::new(
                "classification",
                "MIXED_OWNER_PACKAGE",
                Some(display_relative(&manifest)),
                format!("profile {profile}: package contains {} owners", owners.len()),
            ));
        }
        owners_by_id.insert((*id).clone(), owners.clone());
        let actual_targets = package
            .targets
            .iter()
            .filter_map(|target| normalize_target(root, target, &mut findings))
            .collect::<BTreeSet<_>>();
        let expected_targets = expected.targets.iter().cloned().collect::<BTreeSet<_>>();
        for target in expected_targets.difference(&actual_targets) {
            findings.push(Finding::new(
                "topology",
                "TOPOLOGY_TARGET_MISSING",
                Some(display_relative(&target.src_path)),
                format!("profile {profile}: target {} is absent", target.name),
            ));
        }
        for target in actual_targets.difference(&expected_targets) {
            findings.push(Finding::new(
                "topology",
                "TOPOLOGY_TARGET_EXTRA",
                Some(display_relative(&target.src_path)),
                format!("profile {profile}: target {} is undeclared", target.name),
            ));
        }
        for target in &actual_targets {
            let target_file = files.iter().find(|file| file.path == target.src_path);
            if target_file.is_none() {
                findings.push(Finding::new(
                    "classification",
                    "TARGET_SOURCE_UNCLASSIFIED",
                    Some(display_relative(&target.src_path)),
                    format!("profile {profile}: target source lacks classification"),
                ));
                continue;
            }
            let target_file = target_file.expect("checked above");
            let expected_use =
                if target.kind.iter().any(|kind| matches!(kind.as_str(), "test" | "bench")) {
                    SourceUse::Test
                } else if target.kind.iter().any(|kind| kind == "example") {
                    SourceUse::Example
                } else {
                    match target_file.classification.owner {
                        SourceOwner::RepositoryTooling => SourceUse::Tool,
                        SourceOwner::Product(_) => SourceUse::Production,
                    }
                };
            if target_file.classification.source_use != expected_use
                || !owners.contains(&target_file.classification.owner)
            {
                findings.push(Finding::new(
                    "classification",
                    "TARGET_SOURCE_CLASSIFICATION",
                    Some(display_relative(&target.src_path)),
                    format!(
                        "profile {profile}: target {} requires package owner and source use {}",
                        target.name,
                        expected_use.as_str()
                    ),
                ));
            }
        }
        packages.push(PackageRecord {
            profile: profile.to_owned(),
            name: package.name.clone(),
            manifest,
            owners,
            targets: actual_targets.into_iter().collect(),
        });
    }

    let custom_build_paths = packages
        .iter()
        .flat_map(|package| &package.targets)
        .filter(|target| target.kind.iter().any(|kind| kind == "custom-build"))
        .map(|target| target.src_path.clone())
        .collect::<BTreeSet<_>>();
    let expected_build_paths = topology.build_scripts.iter().cloned().collect::<BTreeSet<_>>();
    for path in expected_build_paths.difference(&custom_build_paths) {
        findings.push(Finding::new(
            "topology",
            "TOPOLOGY_BUILD_SCRIPT_MISSING",
            Some(display_relative(path)),
            format!("profile {profile}: declared build script has no custom-build target"),
        ));
    }
    for path in custom_build_paths.difference(&expected_build_paths) {
        findings.push(Finding::new(
            "topology",
            "TOPOLOGY_BUILD_SCRIPT_EXTRA",
            Some(display_relative(path)),
            format!("profile {profile}: custom-build target is absent from topology"),
        ));
    }

    let mut edges = Vec::new();
    let mut concrete_edges = BTreeSet::new();
    let mut projected_edges = BTreeSet::new();
    for (consumer_id, node) in &metadata.nodes {
        if !metadata.workspace_members.contains(consumer_id) {
            continue;
        }
        let Some(consumer_package) = metadata.packages.get(consumer_id) else { continue };
        let consumer_owners = owners_by_id.get(consumer_id).cloned().unwrap_or_default();
        for dependency in &node.dependencies {
            if RETIRED_NAMES.contains(&dependency.name.as_str()) {
                findings.push(Finding::new(
                    "retired_surface",
                    "RETIRED_DEPENDENCY_NAME",
                    None,
                    format!(
                        "profile {profile}: retired dependency alias {} is forbidden",
                        dependency.name
                    ),
                ));
            }
            if !metadata.workspace_members.contains(&dependency.package) {
                continue;
            }
            let Some(provider_package) = metadata.packages.get(&dependency.package) else {
                continue;
            };
            let provider_owners =
                owners_by_id.get(&dependency.package).cloned().unwrap_or_default();
            concrete_edges.insert((consumer_package.name.clone(), provider_package.name.clone()));
            for consumer in &consumer_owners {
                for provider in &provider_owners {
                    if let (SourceOwner::Product(consumer), SourceOwner::Product(provider)) =
                        (consumer, provider)
                        && consumer != provider
                    {
                        projected_edges.insert((*consumer, *provider));
                    }
                }
            }
            let decision = edge_decision(&consumer_owners, &provider_owners, policy);
            if decision.starts_with("forbidden") {
                findings.push(Finding::new(
                    "partition",
                    if decision == "forbidden_partition" {
                        "FORBIDDEN_PARTITION_EDGE"
                    } else {
                        "FORBIDDEN_PRODUCT_EDGE"
                    },
                    None,
                    format!(
                        "profile {profile}: {} -> {} is {decision}",
                        consumer_package.name, provider_package.name
                    ),
                ));
            }
            for kind in &dependency.kinds {
                edges.push(EdgeRecord {
                    profile: profile.to_owned(),
                    consumer: consumer_package.name.clone(),
                    provider: provider_package.name.clone(),
                    kind: kind.kind.clone(),
                    target: kind.target.clone(),
                    consumer_owners: consumer_owners.clone(),
                    provider_owners: provider_owners.clone(),
                    decision,
                });
            }
        }
    }
    let package_nodes = metadata
        .workspace_members
        .iter()
        .filter_map(|id| metadata.packages.get(id).map(|package| package.name.clone()))
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
                cycle.iter().map(ToString::to_string).collect::<Vec<_>>().join(" -> ")
            ),
        ));
    }

    let dependency_reachability = dependency_reachability(profile, metadata);
    packages.sort();
    edges.sort();
    GraphReport { packages, edges, dependency_reachability, findings: findings.finish() }
}

fn edge_decision(
    consumers: &[SourceOwner],
    providers: &[SourceOwner],
    policy: &Policy,
) -> &'static str {
    if consumers.is_empty() || providers.is_empty() {
        return "forbidden_partition";
    }
    let mut saw_product_edge = false;
    for consumer in consumers {
        for provider in providers {
            match (consumer, provider) {
                (SourceOwner::RepositoryTooling, SourceOwner::RepositoryTooling) => {}
                (SourceOwner::Product(consumer), SourceOwner::Product(provider))
                    if consumer == provider => {}
                (SourceOwner::Product(consumer), SourceOwner::Product(provider))
                    if policy.allowed_product_edges.contains(&(*consumer, *provider)) =>
                {
                    saw_product_edge = true;
                }
                (SourceOwner::Product(_), SourceOwner::Product(_)) => {
                    return "forbidden_product_edge";
                }
                _ => return "forbidden_partition",
            }
        }
    }
    if saw_product_edge { "admitted_product_edge" } else { "admitted_same_owner" }
}

fn dependency_reachability(profile: &str, metadata: &Metadata) -> Vec<ReachabilityRecord> {
    let mut reachability = Vec::new();
    for root_id in &metadata.workspace_members {
        let Some(root_package) = metadata.packages.get(root_id) else { continue };
        let mut member_ids = BTreeSet::new();
        let mut pending = VecDeque::from([root_id.clone()]);
        while let Some(id) = pending.pop_front() {
            if !member_ids.insert(id.clone()) {
                continue;
            }
            if let Some(node) = metadata.nodes.get(&id) {
                for dependency in &node.dependencies {
                    if dependency
                        .kinds
                        .iter()
                        .any(|kind| kind.kind == "normal" || kind.kind == "build")
                    {
                        pending.push_back(dependency.package.clone());
                    }
                }
            }
        }
        let members = member_ids
            .iter()
            .filter_map(|id| metadata.packages.get(id).map(crate::cargo::Package::review_identity))
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

fn normalize_target(
    root: &Path,
    target: &CargoTarget,
    findings: &mut Findings,
) -> Option<ExpectedTarget> {
    let src_path = relative_absolute(root, &target.src_path, findings, "TARGET_SOURCE")?;
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
            findings.push(Finding::new("path", code, None, format!("{}: {error}", path.display())));
            return None;
        }
    };
    canonical.strip_prefix(root).map_or_else(
        |_| {
            findings.push(Finding::new(
                "path",
                "ROOT_ESCAPE",
                None,
                format!("Cargo path {} escapes checked root", canonical.display()),
            ));
            None
        },
        |relative| Some(relative.to_path_buf()),
    )
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

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, path::PathBuf};

    use reovim_depgraph::{ArchitecturalRole, SourceOwner};

    use crate::policy::Policy;

    fn policy() -> Policy {
        Policy {
            scope_roots: vec![PathBuf::new()],
            exclusions: Vec::new(),
            rules: Vec::new(),
            allowed_product_edges: BTreeSet::new(),
            generated_evidence_key: PathBuf::from("conformance/source-role-map.json"),
        }
    }

    #[test]
    fn partition_edges_fail_in_both_directions_and_mixed_sets_fail() {
        let product = SourceOwner::Product(ArchitecturalRole::Foundation);
        let tooling = SourceOwner::RepositoryTooling;
        assert_eq!(super::edge_decision(&[product], &[tooling], &policy()), "forbidden_partition");
        assert_eq!(super::edge_decision(&[tooling], &[product], &policy()), "forbidden_partition");
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
        assert_eq!(super::find_cycle(&nodes, &edges), Some(vec!["a", "b", "a"]));
    }
}
