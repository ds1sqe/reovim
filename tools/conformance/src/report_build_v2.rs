//! Active `reovim-source-role-map/v2` value construction.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    path::{Path, PathBuf},
};

use reovim_depgraph::{
    ArchitecturalRole, SourceOwner, SourceProvenance, SourceUse,
};
use serde_json::{Value, json};

use crate::{
    cargo::{Capture, Package as CargoPackage, VersionCapture},
    compiler_capture, compiler_substrate, digest,
    execution_tools::{
        self, Binding as ExecutionBinding, InvocationFamily, LogicalRole,
    },
    finding::Finding,
    git,
    graph::GraphReport,
    package_capture,
    policy::{DigestSubject, Policy, Upstream},
    prepared_root,
    process::{ProcessLimits, ProcessResult},
    product_closures, product_preflight, product_source, report_v2,
    source::Inventory,
    topology::{
        Limits, ProductExecutionLimits, Topology, Workspace, WorkspacePackage,
        WorkspacePartition,
    },
};

const REPORT_SCHEMA_PATH: &str = "conformance/source-role-map.schema.json";
const SHIPPING_CLOSURE: &str = "foundation-shipping";
const CONSUMER_CLOSURE: &str = "foundation-consumer";
const KERNEL_INTERFACE_SHIPPING_CLOSURE: &str = "kernel-interface-shipping";
const KERNEL_INTERFACE_CONSUMER_CLOSURE: &str = "kernel-interface-consumer";
const PRODUCT_WORKSPACE: &str = "product";
const PRODUCT_PROFILE: &str = "release";
const PRODUCT_TARGET: &str = "library";
const PRODUCT_TARGET_TRIPLE: &str = "x86_64-unknown-linux-gnu";
const PUBLIC_RELATION: &str = "foundation-public-api";
const KERNEL_INTERFACE_PUBLIC_RELATION: &str = "kernel-interface-public-api";
const PREPARED_CARGO: &str = "/compiler/bin/cargo";
const PREPARED_RUSTC: &str = "/compiler/bin/rustc";
const PREPARED_CHECKER: &str = "/checker/reovim-conformance";
const PREPARED_MANIFEST: &str = "/repo/product/Cargo.toml";
const PREPARED_TARGET_DIR: &str = "/scratch/build-target";
const PREPARED_COMPILER_SUPPORT: &str = "/compiler/lib";
const PREPARED_TARGET_LIBDIR: &str =
    concat!("/sysroot/lib/rustlib/", "x86_64-unknown-linux-gnu/lib");
const NETWORK_POLICY_BASIS: &str = "bounded-nftables-empty-table-dump";
const NETWORK_POLICY_DOMAIN: &[u8] = b"reovim-nftables-table-set/v1\0";

/// One committed input plus the exact schema that validated it.
#[derive(Debug, Clone, Copy)]
pub struct ArtifactIdentity<'a> {
    pub path: &'a Path,
    pub schema: &'a str,
    pub content_digest: &'a str,
    pub schema_path: &'a Path,
    pub schema_digest: &'a str,
}

/// The six independent committed inputs required by the v2 report.
#[derive(Debug, Clone, Copy)]
pub struct ArtifactInputs<'a> {
    pub source_classification: ArtifactIdentity<'a>,
    pub workspace_topology: ArtifactIdentity<'a>,
    pub entrypoint_reviews: ArtifactIdentity<'a>,
    pub dependency_reviews: ArtifactIdentity<'a>,
    pub product_closures: ArtifactIdentity<'a>,
    pub execution_tool_reviews: ArtifactIdentity<'a>,
}

/// One exact direct runtime input of a checker-owned executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInputIdentity {
    pub canonical_path: PathBuf,
    pub prepared_path: PathBuf,
    pub content_digest: String,
    pub size_bytes: u64,
}

/// Exact identity needed for checker, Cargo, or rustc report entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolIdentity {
    pub canonical_path: PathBuf,
    pub executable_digest: String,
    pub executable_size_bytes: u64,
    /// Arguments only; the executable path remains separately bound.
    pub version_argv: Vec<String>,
    pub version: String,
    pub version_digest: String,
    pub runtime_inputs: Vec<RuntimeInputIdentity>,
}

/// Exact size and Git-style regular-file mode retained for one classified path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceFileObservation {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub mode: String,
}

/// One workspace/profile's initial capture, optional exact final capture, and
/// already evaluated dependency graph.
#[derive(Debug, Clone, Copy)]
pub struct MetadataEvidence<'a> {
    pub workspace: &'a str,
    pub profile: &'a str,
    pub initial: &'a Capture,
    pub final_capture: Option<&'a Capture>,
    pub graph: &'a GraphReport,
}

/// One bounded Cargo JSON capture for a selected Product closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductCargoBuildEvidence {
    pub closure: String,
    pub command_argv: Vec<String>,
    pub raw_output_digest: String,
    pub stdout_bytes: u64,
    pub message_count: u64,
    pub compiled_units: Vec<String>,
    pub fresh_provider_units: Vec<String>,
    pub artifact_events: Vec<ProductCargoArtifactEventEvidence>,
    pub probes: Vec<ProductProbeEvidence>,
}

/// One Cargo `compiler-artifact` event correlated to a wrapper unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductCargoArtifactEventEvidence {
    pub unit: String,
    pub fresh: bool,
    pub filenames: Vec<PathBuf>,
}

/// Closed Cargo discovery-probe kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProductProbeKind {
    Version,
    Host,
    Target,
}

impl ProductProbeKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Version => "version_probe",
            Self::Host => "host_probe",
            Self::Target => "target_probe",
        }
    }
}

/// One ordered, paired rustc discovery-probe trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductProbeEvidence {
    pub kind: ProductProbeKind,
    pub original_argv: Vec<String>,
    pub effective_argv: Vec<String>,
    pub trace_digest: String,
    pub trace_bytes: u64,
    pub status: i32,
}

/// One compiler unit observed by the private Product rustc wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductCompilerUnitEvidence {
    pub id: String,
    pub closure: String,
    pub features: Vec<String>,
    pub cfgs: Vec<String>,
    pub original_argv: Vec<String>,
    pub effective_argv: Vec<String>,
    pub dep_info_path: PathBuf,
    pub dep_info_digest: String,
    pub dep_info_bytes: u64,
    pub trace_digest: String,
    pub trace_bytes: u64,
    pub status: i32,
}

/// One conservative supplied Product-unit extern relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductExternEvidence {
    pub consumer_unit: String,
    pub extern_name: String,
    pub provider_unit: String,
    pub provider_path: PathBuf,
}

/// One Cargo-produced Product rlib or rmeta retained by identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductArtifactEvidence {
    pub unit: String,
    pub path: PathBuf,
    pub content_digest: String,
    pub size_bytes: u64,
    pub crate_type: String,
}

/// One retained file identity used by the Cargo provider relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductFileIdentityEvidence {
    pub path: PathBuf,
    pub content_digest: String,
    pub size_bytes: u64,
}

/// Top-level Cargo output and hashed wrapper outputs for one shipping unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductProviderArtifactEvidence {
    pub unit: String,
    pub top_level_rlib: ProductFileIdentityEvidence,
    pub hashed_rlib: ProductFileIdentityEvidence,
    pub rmeta: ProductFileIdentityEvidence,
}

/// One host path exposed read-only inside the prepared root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProductExposureKind {
    File,
    Directory,
    CharacterDevice,
}

impl ProductExposureKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::CharacterDevice => "character_device",
        }
    }
}

/// One host path exposed read-only inside the prepared root.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ProductExposureEvidence {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub kind: ProductExposureKind,
    pub read_only: bool,
    pub nosuid: bool,
    pub nodev: bool,
    pub noexec: bool,
    pub purpose: String,
    pub inventory_digest: String,
    pub inventory_entry_count: usize,
    pub inventory_total_bytes: u64,
    pub inventory_entries: Vec<prepared_root::InventoryEntry>,
}

/// Stable namespace relations for one Product execution.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ProductNamespaceEvidence {
    pub user: bool,
    pub mount: bool,
    pub network: bool,
    pub pid: bool,
    pub uid_map: String,
    pub gid_map: String,
    pub setgroups_denied: bool,
    pub private_propagation: bool,
    pub pid_one_reaper: bool,
    pub parent_death_signal: bool,
}

/// Closed network observation inside the prepared root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductNetworkEvidence {
    pub interfaces: Vec<String>,
    pub routes: Vec<String>,
    pub kernel_route_count: u64,
    pub socket_count: u64,
    pub policy_observation_digest: String,
    pub policy_basis: String,
    pub external_carrier_absent: bool,
}

/// Bounded tmpfs observation retained after Product execution.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ProductScratchEvidence {
    pub mountpoint: PathBuf,
    pub size_limit_bytes: u64,
    pub inode_limit: u64,
    pub allocated_bytes: u64,
    pub inode_count: u64,
    pub logical_bytes: u64,
    pub owner_only: bool,
    pub nodev: bool,
    pub noexec: bool,
    pub nosuid: bool,
}

/// Terminal cleanup facts for one Product execution.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ProductCleanupEvidence {
    pub processes_absent: bool,
    pub readers_joined: bool,
    pub namespace_absent: bool,
    pub mount_absent: bool,
    pub mountpoint_identity_preserved: bool,
    pub underlying_directory_empty: bool,
    pub source_identity_preserved: bool,
    pub tool_identity_preserved: bool,
}

/// One complete prepared-root execution observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductExecutionEvidence {
    pub id: String,
    pub environment: String,
    pub exposures: Vec<ProductExposureEvidence>,
    pub namespaces: ProductNamespaceEvidence,
    pub network: ProductNetworkEvidence,
    pub scratch: ProductScratchEvidence,
    pub process_group_identity_validated: bool,
    pub process_group_absent_after_reap: bool,
    pub cleanup: ProductCleanupEvidence,
}

/// Complete active Product evidence consumed by the v2 report adapter.
#[derive(Debug)]
pub struct ActiveProductEvidence<'a> {
    pub compiler_substrate_id: &'a str,
    pub compiler_host_triple: &'a str,
    pub compiler_target_triple: &'a str,
    pub compiler_substrate: &'a compiler_substrate::Observation,
    pub cargo_builds: &'a [ProductCargoBuildEvidence],
    pub compiler_units: &'a [ProductCompilerUnitEvidence],
    pub extern_edges: &'a [ProductExternEvidence],
    pub artifacts: &'a [ProductArtifactEvidence],
    pub provider_artifacts: &'a [ProductProviderArtifactEvidence],
    pub foundation_package_before: &'a package_capture::Observation,
    pub foundation_package_after: &'a package_capture::Observation,
    pub kernel_interface_package_before: &'a package_capture::Observation,
    pub kernel_interface_package_after: &'a package_capture::Observation,
    pub product_sources: &'a product_source::TwoPairSourceObservation,
    pub execution: &'a ProductExecutionEvidence,
    pub process: &'a ProcessResult,
}

/// Candidate/exact authority and exact Git evidence when applicable.
#[derive(Debug, Clone, Copy)]
pub enum Mode<'a> {
    Candidate,
    Exact {
        revision: &'a str,
        git: &'a git::Evidence,
        execution_environment: &'a str,
    },
}

/// Complete repository inputs. Active Product evidence is added only through
/// the separately validated Product-proof boundary.
#[derive(Debug)]
pub struct BuildInputs<'a> {
    pub repository_root: &'a Path,
    pub mode: Mode<'a>,
    pub artifacts: ArtifactInputs<'a>,
    pub report_schema_digest: &'a str,
    pub policy: &'a Policy,
    pub topology: &'a Topology,
    pub inventory: &'a Inventory,
    pub source_observations: &'a [SourceFileObservation],
    pub product_closures: &'a product_closures::Registry,
    pub execution_tools: &'a execution_tools::Registry,
    pub checker: &'a ToolIdentity,
    pub cargo: &'a ToolIdentity,
    pub rustc: &'a ToolIdentity,
    pub checker_version: &'a VersionCapture,
    pub cargo_version: &'a VersionCapture,
    pub rustc_version: &'a VersionCapture,
    pub metadata: &'a [MetadataEvidence<'a>],
    pub active_product: Option<&'a ActiveProductEvidence<'a>>,
    /// Findings from checks not already owned by inventory/graph evidence.
    pub additional_findings: &'a [Finding],
}

/// A typed adapter failure suitable for promotion into a fatal checker result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildError {
    pub code: &'static str,
    pub message: String,
}

impl BuildError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for BuildError {}

impl From<report_v2::ReportError> for BuildError {
    fn from(failure: report_v2::ReportError) -> Self {
        let code = match failure.code() {
            report_v2::ReportErrorCode::Envelope => "V2_REPORT_ENVELOPE",
            report_v2::ReportErrorCode::Schema => "V2_REPORT_SCHEMA",
            report_v2::ReportErrorCode::Canonical => "V2_REPORT_CANONICAL",
            report_v2::ReportErrorCode::Identity => "V2_REPORT_IDENTITY",
            report_v2::ReportErrorCode::Limit => "V2_REPORT_LIMIT",
            report_v2::ReportErrorCode::Reference => "V2_REPORT_REFERENCE",
            report_v2::ReportErrorCode::Semantic => "V2_REPORT_SEMANTIC",
        };
        Self::new(code, failure.to_string())
    }
}

/// Captures size and executable-bit mode for the exact classified inventory.
/// Symlinks and non-files fail rather than being normalized into source.
pub fn observe_source_files(
    root: &Path,
    inventory: &Inventory,
) -> Result<Vec<SourceFileObservation>, BuildError> {
    inventory
        .files
        .iter()
        .map(|file| {
            let metadata = fs::symlink_metadata(root.join(&file.path))
                .map_err(|failure| {
                    BuildError::new(
                        "V2_SOURCE_METADATA",
                        format!("{}: {failure}", file.path.display()),
                    )
                })?;
            if !metadata.file_type().is_file()
                || metadata.file_type().is_symlink()
            {
                return Err(BuildError::new(
                    "V2_SOURCE_FILE_TYPE",
                    format!(
                        "{} is not a regular non-symlink file",
                        file.path.display()
                    ),
                ));
            }
            #[cfg(unix)]
            let executable = {
                use std::os::unix::fs::PermissionsExt as _;
                metadata.permissions().mode() & 0o111 != 0
            };
            #[cfg(not(unix))]
            return Err(BuildError::new(
                "V2_SOURCE_MODE_UNAVAILABLE",
                "v2 exact source mode observation requires Unix metadata",
            ));
            #[cfg(unix)]
            Ok(SourceFileObservation {
                path: file.path.clone(),
                size_bytes: metadata.len(),
                mode: if executable { "100755" } else { "100644" }.to_owned(),
            })
        })
        .collect()
}

/// Constructs and seals the active v2 Source Role Map.
pub fn build(
    inputs: &BuildInputs<'_>,
) -> Result<report_v2::BuiltReport, BuildError> {
    validate_product_presence(
        inputs.product_closures,
        inputs.active_product.is_some(),
    )?;
    validate_product_mode(inputs.mode, inputs.active_product.is_some())?;
    let (root_workspace, workspaces) = validate_workspace_scope(inputs)?;
    validate_tool_capture(inputs.checker, inputs.checker_version, "checker")?;
    validate_tool_capture(inputs.cargo, inputs.cargo_version, "Cargo")?;
    validate_tool_capture(inputs.rustc, inputs.rustc_version, "rustc")?;
    let metadata = validate_metadata_completeness(inputs, &workspaces)?;
    let observations = source_observation_map(inputs)?;
    let mut report = report_value(
        inputs,
        root_workspace,
        &workspaces,
        &metadata,
        &observations,
    )?;
    // Make accidental caller reuse after mutation impossible to confuse with
    // the sealed report returned by the v2 envelope.
    report_v2::build(
        std::mem::take(&mut report),
        inputs.topology.limits.report_bytes,
    )
    .map_err(Into::into)
}

struct SourceReportValues {
    classifications: Vec<Value>,
    exclusions: Vec<Value>,
    workspaces: Vec<Value>,
    lockfiles: Vec<Value>,
}

struct EvidenceReportValues {
    metadata_runs: Vec<Value>,
    graph: GraphValues,
    processes: Vec<Value>,
}

struct FindingReportValues {
    values: Vec<Value>,
    status: &'static str,
    count: usize,
}

struct ProductReportValues {
    compiler_substrates: Vec<Value>,
    proof: Value,
    process: Option<Value>,
}

fn report_value(
    inputs: &BuildInputs<'_>,
    root_workspace: &Workspace,
    workspaces: &[&Workspace],
    metadata: &[&MetadataEvidence<'_>],
    observations: &BTreeMap<&Path, &SourceFileObservation>,
) -> Result<Value, BuildError> {
    let (mode, revision, git_identity, git_tree_digest, git_processes) =
        mode_values(inputs, root_workspace)?;
    let source = source_report_values(inputs, workspaces, observations)?;
    let mut evidence = evidence_report_values(inputs, metadata, git_processes)?;
    let product = product_report_values(inputs, observations)?;
    if let Some(process) = product.process {
        evidence.processes.push(process);
    }
    let findings = finding_report_values(inputs)?;
    let profiles = profile_identity_values(inputs.topology, metadata)?;
    let identity_inputs = identity_inputs_value(
        inputs,
        &source.lockfiles,
        &git_identity,
        &git_tree_digest,
        &profiles,
        &product.compiler_substrates,
    )?;
    Ok(json!({
        "schema": report_v2::SCHEMA,
        "subject": {"kind": "active_repository"},
        "mode": mode,
        "revision": revision,
        "identity_inputs": identity_inputs,
        "limits": limits_value(inputs.topology.limits),
        "source_inventory": {
            "classifications": source.classifications,
            "exclusions": source.exclusions,
        },
        "cargo_graph": {
            "metadata_runs": evidence.metadata_runs,
            "workspaces": source.workspaces,
            "workspace_packages": evidence.graph.workspace_packages,
            "external_packages": evidence.graph.external_packages,
            "edges": evidence.graph.edges,
            "reachability": evidence.graph.reachability,
        },
        "product_proof": product.proof,
        "processes": evidence.processes,
        "findings": findings.values,
        "result": {
            "status": findings.status,
            "finding_count": findings.count,
        },
        "report_identity": null,
    }))
}

fn source_report_values(
    inputs: &BuildInputs<'_>,
    workspaces: &[&Workspace],
    observations: &BTreeMap<&Path, &SourceFileObservation>,
) -> Result<SourceReportValues, BuildError> {
    let classifications = classification_values(inputs, observations)?;
    let exclusions = inputs
        .inventory
        .exclusions
        .iter()
        .map(|exclusion| {
            Ok(json!({
                "prefix": path_text(&exclusion.prefix, "source exclusion")?,
                "kind": exclusion.kind.as_str(),
            }))
        })
        .collect::<Result<Vec<_>, BuildError>>()?;
    let mut workspace_values = Vec::with_capacity(workspaces.len());
    let mut lockfiles = Vec::with_capacity(workspaces.len());
    for workspace in workspaces {
        workspace_values.push(workspace_value(workspace)?);
        lockfiles.push(lockfile_value(inputs, workspace)?);
    }
    Ok(SourceReportValues {
        classifications,
        exclusions,
        workspaces: workspace_values,
        lockfiles,
    })
}

fn workspace_value(workspace: &Workspace) -> Result<Value, BuildError> {
    let partition = match workspace.partition {
        WorkspacePartition::RepositoryTooling => "repository_tooling",
        WorkspacePartition::Product => "product",
    };
    let root = if workspace.root.as_os_str().is_empty() {
        ".".to_owned()
    } else {
        path_text(&workspace.root, "workspace root")?.to_owned()
    };
    Ok(json!({
        "id": workspace.id,
        "partition": partition,
        "root": root,
        "manifest": path_text(
            &workspace.workspace_manifest,
            "workspace manifest",
        )?,
        "lockfile": path_text(&workspace.lockfile, "workspace lockfile")?,
        "members": path_values(&workspace.members, "workspace member")?,
    }))
}

fn lockfile_value(
    inputs: &BuildInputs<'_>,
    workspace: &Workspace,
) -> Result<Value, BuildError> {
    let lockfile = inputs
        .inventory
        .files
        .iter()
        .find(|file| file.path == workspace.lockfile)
        .ok_or_else(|| {
            BuildError::new(
                "V2_LOCKFILE_UNCLASSIFIED",
                format!(
                    "{} is absent from source inventory",
                    workspace.lockfile.display()
                ),
            )
        })?;
    Ok(json!({
        "workspace": workspace.id,
        "path": path_text(&workspace.lockfile, "workspace lockfile")?,
        "digest": lockfile.content_digest,
    }))
}

fn evidence_report_values(
    inputs: &BuildInputs<'_>,
    metadata: &[&MetadataEvidence<'_>],
    git_processes: Vec<Value>,
) -> Result<EvidenceReportValues, BuildError> {
    let mut values = EvidenceReportValues {
        metadata_runs: Vec::new(),
        graph: GraphValues::default(),
        processes: version_process_values(inputs)?,
    };
    values.processes.extend(git_processes);
    for evidence in metadata {
        let workspace =
            inputs.topology.workspaces.get(evidence.workspace).ok_or_else(
                || {
                    BuildError::new(
                        "V2_METADATA_WORKSPACE",
                        format!("unknown workspace {}", evidence.workspace),
                    )
                },
            )?;
        append_metadata_values(inputs, workspace, evidence, &mut values)?;
    }
    Ok(values)
}

fn version_process_values(
    inputs: &BuildInputs<'_>,
) -> Result<Vec<Value>, BuildError> {
    let descriptions = [
        ("checker_version", inputs.checker, &inputs.checker_version.process),
        ("cargo_version", inputs.cargo, &inputs.cargo_version.process),
        ("rustc_version", inputs.rustc, &inputs.rustc_version.process),
    ];
    descriptions
        .into_iter()
        .map(|(purpose, tool, process)| {
            process_value(
                &ProcessDescription {
                    purpose,
                    workspace: None,
                    profile: None,
                    closure: None,
                    execution: None,
                    tool_review: None,
                    tool_environment: None,
                    executable_digest: &tool.executable_digest,
                },
                process,
                &inputs.topology.limits.process(),
            )
        })
        .collect()
}

fn append_metadata_values(
    inputs: &BuildInputs<'_>,
    workspace: &Workspace,
    evidence: &MetadataEvidence<'_>,
    values: &mut EvidenceReportValues,
) -> Result<(), BuildError> {
    append_metadata_capture(
        inputs,
        evidence,
        "initial",
        evidence.initial,
        values,
    )?;
    if let Some(final_capture) = evidence.final_capture {
        append_metadata_capture(
            inputs,
            evidence,
            "final",
            final_capture,
            values,
        )?;
    }
    let graph = graph_values(inputs.repository_root, workspace, evidence)?;
    values.graph.extend(graph);
    Ok(())
}

fn append_metadata_capture(
    inputs: &BuildInputs<'_>,
    evidence: &MetadataEvidence<'_>,
    phase: &str,
    capture: &Capture,
    values: &mut EvidenceReportValues,
) -> Result<(), BuildError> {
    values.metadata_runs.push(metadata_run_value(
        evidence.workspace,
        evidence.profile,
        phase,
        capture,
    )?);
    let purpose = if phase == "initial" {
        "cargo_metadata"
    } else {
        "cargo_metadata_final"
    };
    values.processes.push(process_value(
        &ProcessDescription {
            purpose,
            workspace: Some(evidence.workspace),
            profile: Some(evidence.profile),
            closure: None,
            execution: None,
            tool_review: None,
            tool_environment: None,
            executable_digest: &inputs.cargo.executable_digest,
        },
        &capture.process,
        &inputs.topology.limits.process(),
    )?);
    Ok(())
}

fn finding_report_values(
    inputs: &BuildInputs<'_>,
) -> Result<FindingReportValues, BuildError> {
    let findings = collected_findings(inputs);
    if findings.len() > inputs.topology.limits.findings {
        return Err(BuildError::new(
            "V2_FINDING_LIMIT",
            "combined finding set exceeds the committed limit",
        ));
    }
    let values = findings
        .iter()
        .map(|finding| {
            json!({
                "stage": finding.stage,
                "code": finding.code,
                "path": finding.path,
                "message": finding.message,
            })
        })
        .collect();
    let status = if findings.is_empty() { "pass" } else { "fail" };
    Ok(FindingReportValues { values, status, count: findings.len() })
}

fn profile_identity_values(
    topology: &Topology,
    metadata: &[&MetadataEvidence<'_>],
) -> Result<Vec<Value>, BuildError> {
    metadata
        .iter()
        .map(|evidence| {
            let workspace = topology
                .workspaces
                .get(evidence.workspace)
                .ok_or_else(|| {
                    BuildError::new(
                        "V2_PROFILE_WORKSPACE",
                        format!("unknown workspace {}", evidence.workspace),
                    )
                })?;
            let profile = workspace
                .metadata_profiles
                .get(evidence.profile)
                .ok_or_else(|| {
                    BuildError::new(
                        "V2_PROFILE_UNRESOLVED",
                        format!(
                            "unknown metadata profile {}",
                            evidence.profile
                        ),
                    )
                })?;
            Ok(json!({
                "workspace": evidence.workspace,
                "profile": evidence.profile,
                "target_triple": profile.target_triple,
                "all_features": profile.all_features,
                "no_default_features": profile.no_default_features,
                "features": profile.features,
                "metadata_digest": evidence.initial.raw_digest,
            }))
        })
        .collect()
}

fn identity_inputs_value(
    inputs: &BuildInputs<'_>,
    lockfiles: &[Value],
    git_identity: &Value,
    git_tree_digest: &Value,
    profile_identities: &[Value],
    compiler_substrates: &[Value],
) -> Result<Value, BuildError> {
    Ok(json!({
        "source_classification": artifact_value(
            inputs.artifacts.source_classification,
        )?,
        "workspace_topology": artifact_value(
            inputs.artifacts.workspace_topology,
        )?,
        "entrypoint_reviews": artifact_value(
            inputs.artifacts.entrypoint_reviews,
        )?,
        "dependency_reviews": artifact_value(
            inputs.artifacts.dependency_reviews,
        )?,
        "product_closures": artifact_value(
            inputs.artifacts.product_closures,
        )?,
        "execution_tool_reviews": artifact_value(
            inputs.artifacts.execution_tool_reviews,
        )?,
        "report_schema": {
            "path": REPORT_SCHEMA_PATH,
            "schema": report_v2::SCHEMA,
            "content_digest": inputs.report_schema_digest,
        },
        "lockfiles": lockfiles,
        "source_tree_digest": inputs.inventory.tree_digest,
        "checker": tool_value(inputs.checker)?,
        "cargo": tool_value(inputs.cargo)?,
        "rustc": tool_value(inputs.rustc)?,
        "git": git_identity,
        "git_tree_digest": git_tree_digest,
        "cargo_home_config": "absent",
        "cargo_working_directory_config": "absent",
        "compiler_substrates": compiler_substrates,
        "metadata_profiles": profile_identities,
        "product_proof_gate": null,
    }))
}

fn empty_product_proof_value() -> Value {
    json!({
        "selected_execution_environment": null,
        "closures": [],
        "compiler_units": [],
        "compiler_inputs": [],
        "extern_edges": [],
        "artifacts": [],
        "provider_artifacts": [],
        "package_memberships": [],
        "public_consumers": [],
        "executions": [],
    })
}

#[derive(Clone, Copy)]
struct ActivePair<'a> {
    shipping: &'a product_closures::Closure,
    consumer: &'a product_closures::Closure,
    relation: &'a product_closures::PublicConsumerRelation,
}

struct ActiveRegistry<'a> {
    foundation: ActivePair<'a>,
    kernel_interface: ActivePair<'a>,
}

impl<'a> ActiveRegistry<'a> {
    const fn closures(&self) -> [&'a product_closures::Closure; 4] {
        [
            self.foundation.shipping,
            self.foundation.consumer,
            self.kernel_interface.shipping,
            self.kernel_interface.consumer,
        ]
    }
}

fn product_report_values(
    inputs: &BuildInputs<'_>,
    observations: &BTreeMap<&Path, &SourceFileObservation>,
) -> Result<ProductReportValues, BuildError> {
    let Some(active) = inputs.active_product else {
        return Ok(ProductReportValues {
            compiler_substrates: Vec::new(),
            proof: empty_product_proof_value(),
            process: None,
        });
    };
    let environment = exact_product_environment(inputs.mode)?;
    let registry = active_registry(inputs.product_closures)?;
    if active.execution.environment != environment {
        return Err(BuildError::new(
            "V2_PRODUCT_ENVIRONMENT",
            "Product execution differs from the exact selected environment",
        ));
    }
    validate_product_trace_total(inputs, active)?;
    let substrate = compiler_substrate_value(inputs, active, environment)?;
    let closures = product_closure_values(inputs, active, &registry)?;
    let units = product_unit_values(inputs, active, &registry)?;
    let compiler_inputs =
        product_input_values(inputs, active, &registry, &units, observations)?;
    let artifacts = product_artifact_values(inputs, active, &units)?;
    let extern_edges = product_extern_values(active, &units, &artifacts)?;
    let provider_artifacts =
        product_provider_artifact_values(inputs, active, &units, &artifacts)?;
    let memberships =
        package_membership_values(inputs, active, &registry, observations)?;
    let public_consumers = public_consumer_values(inputs, active, &registry)?;
    validate_active_runtime_exposures(inputs, active.execution)?;
    let execution = product_execution_value(inputs, active)?;
    let process = product_process_value(inputs, active)?;
    Ok(ProductReportValues {
        compiler_substrates: vec![substrate],
        proof: json!({
            "selected_execution_environment": environment,
            "closures": closures.values,
            "compiler_units": units.values,
            "compiler_inputs": compiler_inputs,
            "extern_edges": extern_edges,
            "artifacts": artifacts.values,
            "provider_artifacts": provider_artifacts,
            "package_memberships": memberships,
            "public_consumers": public_consumers,
            "executions": [execution],
        }),
        process: Some(process),
    })
}

fn exact_product_environment(mode: Mode<'_>) -> Result<&str, BuildError> {
    match mode {
        Mode::Exact { execution_environment, .. } => Ok(execution_environment),
        Mode::Candidate => Err(BuildError::new(
            "PRODUCT_EXACT_PROOF_REQUIRED",
            "active Product evidence cannot be emitted in candidate mode",
        )),
    }
}

fn validate_product_mode(
    mode: Mode<'_>,
    active: bool,
) -> Result<(), BuildError> {
    if active && matches!(mode, Mode::Candidate) {
        return Err(BuildError::new(
            "PRODUCT_EXACT_PROOF_REQUIRED",
            "active Product evidence cannot be emitted in candidate mode",
        ));
    }
    Ok(())
}

fn active_registry(
    registry: &product_closures::Registry,
) -> Result<ActiveRegistry<'_>, BuildError> {
    let closure_ids = registry
        .closures
        .iter()
        .map(|closure| closure.id.as_str())
        .collect::<BTreeSet<_>>();
    let expected_closures = BTreeSet::from([
        SHIPPING_CLOSURE,
        CONSUMER_CLOSURE,
        KERNEL_INTERFACE_SHIPPING_CLOSURE,
        KERNEL_INTERFACE_CONSUMER_CLOSURE,
    ]);
    let relation_ids = registry
        .public_consumer_relations
        .iter()
        .map(|relation| relation.id.as_str())
        .collect::<BTreeSet<_>>();
    let expected_relations =
        BTreeSet::from([PUBLIC_RELATION, KERNEL_INTERFACE_PUBLIC_RELATION]);
    if closure_ids != expected_closures || relation_ids != expected_relations {
        return Err(BuildError::new(
            "V2_PRODUCT_REGISTRY_SET",
            "active Product registry differs from its closed identity sets",
        ));
    }
    let foundation = active_pair(
        registry,
        SHIPPING_CLOSURE,
        CONSUMER_CLOSURE,
        PUBLIC_RELATION,
    )?;
    let kernel_interface = active_pair(
        registry,
        KERNEL_INTERFACE_SHIPPING_CLOSURE,
        KERNEL_INTERFACE_CONSUMER_CLOSURE,
        KERNEL_INTERFACE_PUBLIC_RELATION,
    )?;
    validate_foundation_registry(foundation)?;
    validate_kernel_interface_registry(kernel_interface)?;
    Ok(ActiveRegistry { foundation, kernel_interface })
}

fn active_pair<'a>(
    registry: &'a product_closures::Registry,
    shipping_id: &str,
    consumer_id: &str,
    relation_id: &str,
) -> Result<ActivePair<'a>, BuildError> {
    let shipping = active_closure(registry, shipping_id)?;
    let consumer = active_closure(registry, consumer_id)?;
    let relation = registry
        .public_consumer_relations
        .iter()
        .find(|value| value.id == relation_id)
        .ok_or_else(|| missing_product_registry(relation_id))?;
    Ok(ActivePair { shipping, consumer, relation })
}

fn active_closure<'a>(
    registry: &'a product_closures::Registry,
    id: &str,
) -> Result<&'a product_closures::Closure, BuildError> {
    registry
        .closures
        .iter()
        .find(|value| value.id == id)
        .ok_or_else(|| missing_product_registry(id))
}

fn missing_product_registry(subject: &str) -> BuildError {
    BuildError::new(
        "V2_PRODUCT_REGISTRY_SET",
        format!("active Product registry lacks {subject}"),
    )
}

fn validate_foundation_registry(
    pair: ActivePair<'_>,
) -> Result<(), BuildError> {
    validate_registry_closure(
        pair.shipping,
        product_closures::ClosureKind::Shipping,
        "foundation",
        "reovim-foundation",
        ArchitecturalRole::Foundation,
        SourceUse::Production,
    )?;
    validate_registry_closure(
        pair.consumer,
        product_closures::ClosureKind::Validation,
        "foundation-public-consumer",
        "reovim-foundation-public-consumer",
        ArchitecturalRole::KernelInterface,
        SourceUse::Test,
    )?;
    validate_registry_path(
        pair.shipping,
        "kernel/foundation/Cargo.toml",
        "reovim_foundation",
        "kernel/foundation/src/lib.rs",
        true,
    )?;
    validate_registry_path(
        pair.consumer,
        "tests/foundation-public-consumer/Cargo.toml",
        "reovim_foundation_public_consumer",
        "tests/foundation-public-consumer/src/lib.rs",
        false,
    )?;
    validate_registry_relation(pair, PUBLIC_RELATION)?;
    Ok(())
}

fn validate_kernel_interface_registry(
    pair: ActivePair<'_>,
) -> Result<(), BuildError> {
    validate_registry_closure(
        pair.shipping,
        product_closures::ClosureKind::Shipping,
        "kernel-interface",
        "reovim-kernel-interface",
        ArchitecturalRole::KernelInterface,
        SourceUse::Production,
    )?;
    validate_registry_closure(
        pair.consumer,
        product_closures::ClosureKind::Validation,
        "kernel-interface-public-consumer",
        "reovim-kernel-interface-public-consumer",
        ArchitecturalRole::KernelRuntime,
        SourceUse::Test,
    )?;
    validate_registry_path(
        pair.shipping,
        "kernel/interface/Cargo.toml",
        "reovim_kernel_interface",
        "kernel/interface/src/lib.rs",
        true,
    )?;
    validate_registry_path(
        pair.consumer,
        "tests/kernel-interface-public-consumer/Cargo.toml",
        "reovim_kernel_interface_public_consumer",
        "tests/kernel-interface-public-consumer/src/lib.rs",
        false,
    )?;
    validate_registry_relation(pair, KERNEL_INTERFACE_PUBLIC_RELATION)?;
    Ok(())
}

fn validate_registry_path(
    closure: &product_closures::Closure,
    manifest: &str,
    target: &str,
    source: &str,
    has_membership: bool,
) -> Result<(), BuildError> {
    let exact = closure.package.manifest == Path::new(manifest)
        && closure.target.name == target
        && closure.target.src_path == Path::new(source)
        && closure.package_membership.is_some() == has_membership;
    if exact {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_CLOSURE_PATH",
        format!("closure {} has an unexpected package path", closure.id),
    ))
}

fn validate_registry_relation(
    pair: ActivePair<'_>,
    relation_id: &str,
) -> Result<(), BuildError> {
    let exact = pair.relation.id == relation_id
        && pair.relation.shipping_closure == pair.shipping.id
        && pair.relation.consumer_closure == pair.consumer.id;
    if exact {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_RELATION",
        format!("public relation {relation_id} differs from its exact pair"),
    ))
}

fn validate_registry_closure(
    closure: &product_closures::Closure,
    kind: product_closures::ClosureKind,
    package_id: &str,
    package_name: &str,
    role: ArchitecturalRole,
    source_use: SourceUse,
) -> Result<(), BuildError> {
    if closure.kind != kind
        || closure.workspace.id != PRODUCT_WORKSPACE
        || closure.workspace.manifest != Path::new("product/Cargo.toml")
        || closure.workspace.lockfile != Path::new("product/Cargo.lock")
        || closure.package.id != package_id
        || closure.package.name != package_name
        || closure.target.id != PRODUCT_TARGET
        || closure.target.kind != ["lib"]
        || closure.target.crate_types != ["lib"]
        || closure.architectural_role != role
        || closure.source_use != source_use
        || closure.cargo.profile != PRODUCT_PROFILE
    {
        return Err(BuildError::new(
            "V2_PRODUCT_CLOSURE_SHAPE",
            format!("closure {} differs from the active profile", closure.id),
        ));
    }
    Ok(())
}

fn compiler_substrate_value(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
    environment: &str,
) -> Result<Value, BuildError> {
    let limits = inputs.topology.limits.product_execution;
    let observed = active.compiler_substrate;
    if active.compiler_substrate_id.is_empty()
        || active.compiler_host_triple != PRODUCT_TARGET_TRIPLE
        || observed.entries.is_empty()
        || observed.entries.len()
            > usize::from(limits.compiler_substrate_entry_count)
        || observed.total_bytes
            > u64::from(limits.compiler_substrate_total_bytes)
        || active.compiler_target_triple != PRODUCT_TARGET_TRIPLE
    {
        return Err(BuildError::new(
            "V2_PRODUCT_SUBSTRATE_LIMIT",
            "compiler substrate differs from the bounded active profile",
        ));
    }
    validate_substrate_observation(inputs, observed)?;
    let mut entries = observed.entries.iter().collect::<Vec<_>>();
    entries
        .sort_by(|left, right| left.canonical_path.cmp(&right.canonical_path));
    let values = entries
        .into_iter()
        .map(substrate_entry_value)
        .collect::<Result<Vec<_>, _>>()?;
    let aggregate_digest =
        digest::bytes(&serde_json::to_vec(&values).map_err(|failure| {
            BuildError::new(
                "V2_PRODUCT_SUBSTRATE_IDENTITY",
                format!("cannot serialize substrate entries: {failure}"),
            )
        })?);
    Ok(json!({
        "id": active.compiler_substrate_id,
        "environment": environment,
        "host_triple": active.compiler_host_triple,
        "target_triple": active.compiler_target_triple,
        "compiler_support_root": path_text(
            &observed.compiler_support_root,
            "compiler support root",
        )?,
        "target_libdir": path_text(
            &observed.target_libdir,
            "compiler target libdir",
        )?,
        "entries": values,
        "entry_count": observed.entry_count,
        "total_bytes": observed.total_bytes,
        "aggregate_digest": aggregate_digest,
    }))
}

fn validate_substrate_observation(
    inputs: &BuildInputs<'_>,
    observed: &compiler_substrate::Observation,
) -> Result<(), BuildError> {
    validate_product_path(
        inputs,
        &observed.compiler_support_root,
        "compiler support root",
    )?;
    validate_product_path(
        inputs,
        &observed.target_libdir,
        "compiler target libdir",
    )?;
    if observed.compiler_support_root != Path::new(PREPARED_COMPILER_SUPPORT)
        || observed.target_libdir != Path::new(PREPARED_TARGET_LIBDIR)
    {
        return Err(BuildError::new(
            "V2_PRODUCT_SUBSTRATE_ROOT",
            "compiler substrate roots differ from the prepared profile",
        ));
    }
    let mut paths = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    let mut total = 0_u64;
    for entry in &observed.entries {
        let root = match entry.kind {
            compiler_substrate::EntryKind::CompilerSupport => {
                &observed.compiler_support_root
            }
            compiler_substrate::EntryKind::TargetLibdir => {
                &observed.target_libdir
            }
        };
        let text = path_text(&entry.canonical_path, "substrate entry")?;
        if !normalized_absolute_path(text)
            || text.len() > inputs.topology.limits.path_bytes
            || entry.canonical_path == *root
            || !entry.canonical_path.starts_with(root)
            || entry.canonical_path.parent() != Some(root.as_path())
            || !paths.insert(&entry.canonical_path)
        {
            return Err(BuildError::new(
                "V2_PRODUCT_SUBSTRATE_ENTRY",
                format!(
                    "invalid substrate entry {}",
                    entry.canonical_path.display()
                ),
            ));
        }
        kinds.insert(entry.kind);
        total = checked_evidence_sum(total, entry.size_bytes)?;
    }
    let expected_kinds = BTreeSet::from([
        compiler_substrate::EntryKind::CompilerSupport,
        compiler_substrate::EntryKind::TargetLibdir,
    ]);
    if observed.entry_count != observed.entries.len()
        || observed.total_bytes != total
        || kinds != expected_kinds
    {
        return Err(BuildError::new(
            "V2_PRODUCT_SUBSTRATE_IDENTITY",
            "substrate count, bytes, or kind coverage is inconsistent",
        ));
    }
    Ok(())
}

fn normalized_absolute_path(value: &str) -> bool {
    value.starts_with('/')
        && value != "/"
        && !value.ends_with('/')
        && !value.contains(['\0', '\r', '\n', '\\'])
        && !value.contains("//")
        && !value.split('/').any(|part| matches!(part, "." | ".."))
}

fn substrate_entry_value(
    entry: &compiler_substrate::Entry,
) -> Result<Value, BuildError> {
    let kind = match entry.kind {
        compiler_substrate::EntryKind::CompilerSupport => "compiler_support",
        compiler_substrate::EntryKind::TargetLibdir => "target_libdir",
    };
    Ok(json!({
        "canonical_path": path_text(
            &entry.canonical_path,
            "compiler substrate entry",
        )?,
        "kind": kind,
        "content_digest": entry.content_digest,
        "size_bytes": entry.size_bytes,
    }))
}

struct ProductClosureValues {
    values: Vec<Value>,
}

fn product_closure_values(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
    registry: &ActiveRegistry<'_>,
) -> Result<ProductClosureValues, BuildError> {
    if active.cargo_builds.len() != 4 {
        return Err(BuildError::new(
            "V2_PRODUCT_CARGO_CAPTURE_SET",
            "active Product evidence requires four Cargo build captures",
        ));
    }
    let mut builds = BTreeMap::new();
    for build in active.cargo_builds {
        if builds.insert(build.closure.as_str(), build).is_some() {
            return Err(BuildError::new(
                "V2_PRODUCT_CARGO_CAPTURE_DUPLICATE",
                format!("duplicate Cargo capture for {}", build.closure),
            ));
        }
    }
    let mut values = Vec::with_capacity(4);
    for closure in registry.closures() {
        let build = builds.get(closure.id.as_str()).ok_or_else(|| {
            BuildError::new(
                "V2_PRODUCT_CARGO_CAPTURE_MISSING",
                format!("closure {} lacks its Cargo capture", closure.id),
            )
        })?;
        validate_product_cargo_capture(inputs, active, closure, build)?;
        let probes = product_probe_values(inputs, closure, &build.probes)?;
        let artifact_events = cargo_artifact_event_values(inputs, build)?;
        values.push(json!({
            "id": closure.id,
            "kind": closure_kind_text(closure.kind),
            "workspace": closure.workspace.id,
            "package": closure.package.id,
            "target": closure.target.id,
            "execution": active.execution.id,
            "cargo_capture": {
                "command_argv": build.command_argv,
                "raw_output_digest": build.raw_output_digest,
                "stdout_bytes": build.stdout_bytes,
                "message_count": build.message_count,
                "compiled_units": sorted_unique_strings(
                    &build.compiled_units,
                    "Cargo compiled unit",
                )?,
                "fresh_provider_units": sorted_unique_strings(
                    &build.fresh_provider_units,
                    "Cargo fresh provider unit",
                )?,
                "artifact_events": artifact_events,
                "probes": probes,
            },
            "status": "pass",
        }));
    }
    values
        .sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    Ok(ProductClosureValues { values })
}

fn cargo_artifact_event_values(
    inputs: &BuildInputs<'_>,
    build: &ProductCargoBuildEvidence,
) -> Result<Vec<Value>, BuildError> {
    if build.artifact_events.is_empty()
        || build.artifact_events.len()
            > usize::from(
                inputs.topology.limits.product_execution.cargo_message_count,
            )
    {
        return Err(BuildError::new(
            "V2_PRODUCT_CARGO_ARTIFACT_SET",
            "Cargo artifact events are empty or exceed their bound",
        ));
    }
    let mut events = build.artifact_events.iter().collect::<Vec<_>>();
    events.sort_by(|left, right| left.unit.cmp(&right.unit));
    let mut units = BTreeSet::new();
    events
        .into_iter()
        .map(|event| {
            if event.unit.is_empty() || !units.insert(&event.unit) {
                return Err(BuildError::new(
                    "V2_PRODUCT_CARGO_ARTIFACT_UNIT",
                    "Cargo artifact event unit is empty or duplicated",
                ));
            }
            let mut filenames = event.filenames.clone();
            filenames.sort();
            if filenames.len() != 2
                || filenames[0] == filenames[1]
                || filenames.iter().any(|path| {
                    require_under_path(
                        path,
                        Path::new(PREPARED_TARGET_DIR),
                        "Cargo artifact filename",
                    )
                    .is_err()
                        || validate_product_path(
                            inputs,
                            path,
                            "Cargo artifact filename",
                        )
                        .is_err()
                })
            {
                return Err(BuildError::new(
                    "V2_PRODUCT_CARGO_ARTIFACT_PATH",
                    "Cargo artifact filenames differ from the exact pair",
                ));
            }
            let extensions = filenames
                .iter()
                .filter_map(|path| {
                    path.extension().and_then(|value| value.to_str())
                })
                .collect::<BTreeSet<_>>();
            if extensions != BTreeSet::from(["rlib", "rmeta"]) {
                return Err(BuildError::new(
                    "V2_PRODUCT_CARGO_ARTIFACT_PATH",
                    "Cargo artifact event lacks rlib/rmeta coverage",
                ));
            }
            let filenames = filenames
                .iter()
                .map(|path| path_text(path, "Cargo artifact filename"))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(json!({
                "unit": event.unit,
                "fresh": event.fresh,
                "filenames": filenames,
            }))
        })
        .collect()
}

const fn closure_kind_text(
    kind: product_closures::ClosureKind,
) -> &'static str {
    match kind {
        product_closures::ClosureKind::Shipping => "shipping",
        product_closures::ClosureKind::Validation => "validation",
    }
}

fn validate_product_cargo_capture(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
    closure: &product_closures::Closure,
    build: &ProductCargoBuildEvidence,
) -> Result<(), BuildError> {
    let limits = inputs.topology.limits.product_execution;
    if build.stdout_bytes == 0
        || build.stdout_bytes > u64::from(limits.cargo_json_bytes)
        || build.message_count == 0
        || build.message_count > u64::from(limits.cargo_message_count)
    {
        return Err(BuildError::new(
            "V2_PRODUCT_CARGO_CAPTURE_LIMIT",
            format!("closure {} exceeds its Cargo capture bound", closure.id),
        ));
    }
    validate_cargo_message_relation(build)?;
    let target_dir = exact_product_target_dir(&build.command_argv)?;
    validate_product_path(inputs, Path::new(target_dir), "Cargo target")?;
    require_under_path(
        Path::new(target_dir),
        &active.execution.scratch.mountpoint,
        "Product Cargo target directory",
    )?;
    let expected = [
        PREPARED_CARGO,
        "build",
        "--quiet",
        "--locked",
        "--offline",
        "--manifest-path",
        PREPARED_MANIFEST,
        "--package",
        closure.package.name.as_str(),
        "--lib",
        "--target",
        PRODUCT_TARGET_TRIPLE,
        "--profile",
        PRODUCT_PROFILE,
        "--jobs",
        "1",
        "--message-format=json",
        "--target-dir",
        target_dir,
    ];
    if build.command_argv.iter().map(String::as_str).ne(expected) {
        return Err(BuildError::new(
            "V2_PRODUCT_CARGO_ARGV",
            format!("closure {} has noncanonical Cargo argv", closure.id),
        ));
    }
    Ok(())
}

fn validate_cargo_message_relation(
    build: &ProductCargoBuildEvidence,
) -> Result<(), BuildError> {
    let expected = u64::try_from(build.artifact_events.len())
        .ok()
        .and_then(|count| count.checked_add(1));
    if expected == Some(build.message_count) {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_CARGO_CAPTURE_SET",
        "Cargo message count differs from artifact events plus finish",
    ))
}

fn exact_product_target_dir(argv: &[String]) -> Result<&str, BuildError> {
    let target = option_value(argv, "--target-dir").ok_or_else(|| {
        BuildError::new(
            "V2_PRODUCT_CARGO_ARGV",
            "Product Cargo build omits --target-dir",
        )
    })?;
    if target == PREPARED_TARGET_DIR {
        return Ok(target);
    }
    Err(BuildError::new(
        "V2_PRODUCT_CARGO_ARGV",
        "Product Cargo target differs from its exact scratch authority",
    ))
}

fn product_probe_values(
    inputs: &BuildInputs<'_>,
    closure: &product_closures::Closure,
    probes: &[ProductProbeEvidence],
) -> Result<Vec<Value>, BuildError> {
    let expected = if closure.id == SHIPPING_CLOSURE {
        vec![
            ProductProbeKind::Version,
            ProductProbeKind::Host,
            ProductProbeKind::Target,
            ProductProbeKind::Version,
        ]
    } else {
        Vec::new()
    };
    if probes.iter().map(|probe| probe.kind).ne(expected) {
        return Err(BuildError::new(
            "V2_PRODUCT_PROBE_SET",
            format!("closure {} has the wrong discovery probes", closure.id),
        ));
    }
    probes.iter().map(|probe| product_probe_value(inputs, probe)).collect()
}

fn product_probe_value(
    inputs: &BuildInputs<'_>,
    probe: &ProductProbeEvidence,
) -> Result<Value, BuildError> {
    validate_product_probe(inputs, probe)?;
    Ok(json!({
        "kind": probe.kind.as_str(),
        "original_argv": probe.original_argv,
        "effective_argv": probe.effective_argv,
        "trace_digest": probe.trace_digest,
        "trace_bytes": probe.trace_bytes,
        "status": probe.status,
    }))
}

fn validate_product_probe(
    inputs: &BuildInputs<'_>,
    probe: &ProductProbeEvidence,
) -> Result<(), BuildError> {
    let expected = expected_probe_argv(PREPARED_RUSTC, probe.kind);
    let mut effective = expected.clone();
    effective.extend(["--sysroot".to_owned(), "/sysroot".to_owned()]);
    if probe.original_argv != expected
        || probe.effective_argv != effective
        || probe.trace_bytes == 0
        || probe.status != 0
    {
        return Err(BuildError::new(
            "V2_PRODUCT_PROBE_SHAPE",
            "discovery probe argv, trace, or status differs",
        ));
    }
    validate_probe_argv_bound(inputs, &probe.original_argv)?;
    validate_probe_argv_bound(inputs, &probe.effective_argv)
}

fn expected_probe_argv(rustc: &str, kind: ProductProbeKind) -> Vec<String> {
    if kind == ProductProbeKind::Version {
        return vec![rustc.to_owned(), "-vV".to_owned()];
    }
    let mut argv = vec![
        rustc.to_owned(),
        "-".to_owned(),
        "--crate-name".to_owned(),
        "___".to_owned(),
        "--print=file-names".to_owned(),
    ];
    if kind == ProductProbeKind::Target {
        argv.extend(["--target".to_owned(), PRODUCT_TARGET_TRIPLE.to_owned()]);
    }
    argv.extend(
        [
            "--crate-type",
            "bin",
            "--crate-type",
            "rlib",
            "--crate-type",
            "dylib",
            "--crate-type",
            "cdylib",
            "--crate-type",
            "staticlib",
            "--crate-type",
            "proc-macro",
            "--print=sysroot",
            "--print=split-debuginfo",
            "--print=crate-name",
            "--print=cfg",
            "-Wwarnings",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    argv
}

fn validate_probe_argv_bound(
    inputs: &BuildInputs<'_>,
    argv: &[String],
) -> Result<(), BuildError> {
    let limits = inputs.topology.limits.product_execution;
    let bytes = argv.iter().try_fold(0_usize, |total, argument| {
        total.checked_add(argument.len()).ok_or_else(|| {
            BuildError::new(
                "V2_PRODUCT_PROBE_LIMIT",
                "probe argv byte count overflow",
            )
        })
    })?;
    if argv.len() > limits.wrapper_argv_items
        || bytes > limits.wrapper_argv_bytes as usize
    {
        return Err(BuildError::new(
            "V2_PRODUCT_PROBE_LIMIT",
            "probe argv exceeds its retained bound",
        ));
    }
    Ok(())
}

fn validate_product_trace_total(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
) -> Result<(), BuildError> {
    let probe_traces = active
        .cargo_builds
        .iter()
        .flat_map(|build| &build.probes)
        .map(|probe| probe.trace_bytes);
    let unit_traces = active.compiler_units.iter().map(|unit| unit.trace_bytes);
    let total = probe_traces
        .chain(unit_traces)
        .try_fold(0_u64, checked_evidence_sum)?;
    let limit =
        u64::from(inputs.topology.limits.product_execution.wrapper_trace_bytes);
    if total > limit {
        return Err(BuildError::new(
            "V2_PRODUCT_TRACE_LIMIT",
            "probe and compiler-unit traces exceed their aggregate bound",
        ));
    }
    Ok(())
}

fn option_value<'a>(arguments: &'a [String], option: &str) -> Option<&'a str> {
    arguments
        .windows(2)
        .find_map(|pair| (pair[0] == option).then_some(pair[1].as_str()))
}

fn sorted_unique_strings(
    values: &[String],
    context: &str,
) -> Result<Vec<String>, BuildError> {
    let sorted = values.iter().cloned().collect::<BTreeSet<_>>();
    if sorted.len() != values.len() || sorted.iter().any(String::is_empty) {
        return Err(BuildError::new(
            "V2_PRODUCT_DUPLICATE_IDENTITY",
            format!("{context} set is empty-valued or duplicated"),
        ));
    }
    Ok(sorted.into_iter().collect())
}

#[derive(Debug, Clone)]
struct ProductUnitRef {
    closure: String,
    original_argv: Vec<String>,
}

struct ProductUnitValues {
    values: Vec<Value>,
    by_id: BTreeMap<String, ProductUnitRef>,
    by_closure: BTreeMap<String, String>,
}

fn product_unit_values(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
    registry: &ActiveRegistry<'_>,
) -> Result<ProductUnitValues, BuildError> {
    let limits = inputs.topology.limits.product_execution;
    if active.compiler_units.len() != 4
        || active.compiler_units.len() > limits.compiler_unit_count
    {
        return Err(BuildError::new(
            "V2_PRODUCT_UNIT_SET",
            "active Product evidence requires four bounded compiler units",
        ));
    }
    let closures = registry
        .closures()
        .into_iter()
        .map(|closure| (closure.id.as_str(), closure))
        .collect::<BTreeMap<_, _>>();
    let mut units = active.compiler_units.iter().collect::<Vec<_>>();
    units.sort_by(|left, right| left.id.cmp(&right.id));
    let mut values = Vec::with_capacity(4);
    let mut by_id = BTreeMap::new();
    let mut by_closure = BTreeMap::new();
    let mut dep_paths = BTreeSet::new();
    let mut dep_total = 0_u64;
    let mut trace_total = 0_u64;
    for unit in units {
        let closure = closures.get(unit.closure.as_str()).ok_or_else(|| {
            BuildError::new(
                "V2_PRODUCT_UNIT_CLOSURE",
                format!("unit {} references an unknown closure", unit.id),
            )
        })?;
        validate_product_unit(
            inputs,
            active,
            unit,
            &mut dep_paths,
            &mut dep_total,
            &mut trace_total,
        )?;
        if by_closure.insert(unit.closure.clone(), unit.id.clone()).is_some()
            || by_id
                .insert(
                    unit.id.clone(),
                    ProductUnitRef {
                        closure: unit.closure.clone(),
                        original_argv: unit.original_argv.clone(),
                    },
                )
                .is_some()
        {
            return Err(BuildError::new(
                "V2_PRODUCT_UNIT_DUPLICATE",
                "compiler unit ID or closure is duplicated",
            ));
        }
        values.push(product_unit_value(active, unit, closure)?);
    }
    validate_cargo_unit_joins(active, &by_closure)?;
    Ok(ProductUnitValues { values, by_id, by_closure })
}

fn validate_product_unit(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
    unit: &ProductCompilerUnitEvidence,
    dep_paths: &mut BTreeSet<PathBuf>,
    dep_total: &mut u64,
    trace_total: &mut u64,
) -> Result<(), BuildError> {
    let limits = inputs.topology.limits.product_execution;
    if unit.id.is_empty()
        || !default_feature_selection(&unit.features, &unit.cfgs)
        || unit.status != 0
        || unit.dep_info_bytes == 0
        || unit.dep_info_bytes > u64::from(limits.dep_info_file_bytes)
        || unit.trace_bytes == 0
        || !dep_paths.insert(unit.dep_info_path.clone())
    {
        return Err(BuildError::new(
            "V2_PRODUCT_UNIT_SHAPE",
            format!(
                "compiler unit {} differs from the active profile",
                unit.id
            ),
        ));
    }
    require_under_path(
        &unit.dep_info_path,
        &active.execution.scratch.mountpoint,
        "compiler dep-info",
    )?;
    validate_product_path(inputs, &unit.dep_info_path, "compiler dep-info")?;
    validate_wrapper_argv(inputs, active, unit)?;
    *dep_total = checked_evidence_sum(*dep_total, unit.dep_info_bytes)?;
    *trace_total = checked_evidence_sum(*trace_total, unit.trace_bytes)?;
    if *dep_total > u64::from(limits.dep_info_total_bytes)
        || *trace_total > u64::from(limits.wrapper_trace_bytes)
    {
        return Err(BuildError::new(
            "V2_PRODUCT_UNIT_LIMIT",
            "compiler dep-info or trace aggregate exceeds its bound",
        ));
    }
    Ok(())
}

fn default_feature_selection(features: &[String], cfgs: &[String]) -> bool {
    features == ["default"] && cfgs == ["feature=\"default\""]
}

fn checked_evidence_sum(total: u64, value: u64) -> Result<u64, BuildError> {
    total.checked_add(value).ok_or_else(|| {
        BuildError::new("V2_PRODUCT_LIMIT_OVERFLOW", "evidence byte overflow")
    })
}

fn validate_wrapper_argv(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
    unit: &ProductCompilerUnitEvidence,
) -> Result<(), BuildError> {
    for (context, argv) in
        [("original", &unit.original_argv), ("effective", &unit.effective_argv)]
    {
        validate_one_wrapper_argv(inputs, unit, context, argv)?;
    }
    let extern_path = expected_unit_extern(active, unit)?;
    compiler_capture::validate_reported_unit_argv(
        &unit.original_argv,
        &unit.effective_argv,
        &unit.dep_info_path,
        crate_name_for_closure(&unit.closure)?,
        extern_path,
    )
    .map_err(|failure| BuildError::new(failure.code, failure.to_string()))
}

fn expected_unit_extern<'a>(
    active: &'a ActiveProductEvidence<'_>,
    unit: &ProductCompilerUnitEvidence,
) -> Result<Option<&'a Path>, BuildError> {
    if unit.closure == SHIPPING_CLOSURE {
        return Ok(None);
    }
    let expected_name = match unit.closure.as_str() {
        CONSUMER_CLOSURE | KERNEL_INTERFACE_SHIPPING_CLOSURE => {
            "reovim_foundation"
        }
        KERNEL_INTERFACE_CONSUMER_CLOSURE => "reovim_kernel_interface",
        _ => {
            return Err(BuildError::new(
                "V2_PRODUCT_UNIT_CLOSURE",
                "compiler unit closure is outside the closed two-pair set",
            ));
        }
    };
    let mut edges =
        active.extern_edges.iter().filter(|edge| edge.consumer_unit == unit.id);
    let Some(edge) = edges.next() else {
        return Err(BuildError::new(
            "V2_PRODUCT_EXTERN_SET",
            "consumer compiler unit lacks one retained extern edge",
        ));
    };
    if edges.next().is_none() && edge.extern_name == expected_name {
        return Ok(Some(&edge.provider_path));
    }
    Err(BuildError::new(
        "V2_PRODUCT_EXTERN_JOIN",
        "consumer compiler argv and retained extern edge differ",
    ))
}

fn crate_name_for_closure(closure: &str) -> Result<&'static str, BuildError> {
    match closure {
        SHIPPING_CLOSURE => Ok("reovim_foundation"),
        CONSUMER_CLOSURE => Ok("reovim_foundation_public_consumer"),
        KERNEL_INTERFACE_SHIPPING_CLOSURE => Ok("reovim_kernel_interface"),
        KERNEL_INTERFACE_CONSUMER_CLOSURE => {
            Ok("reovim_kernel_interface_public_consumer")
        }
        _ => Err(BuildError::new(
            "V2_PRODUCT_UNIT_CLOSURE",
            "compiler unit closure is outside the closed pair",
        )),
    }
}

fn validate_one_wrapper_argv(
    inputs: &BuildInputs<'_>,
    unit: &ProductCompilerUnitEvidence,
    context: &str,
    argv: &[String],
) -> Result<(), BuildError> {
    let limits = inputs.topology.limits.product_execution;
    let bytes = argv.iter().try_fold(0_usize, |total, argument| {
        total.checked_add(argument.len()).ok_or_else(|| {
            BuildError::new(
                "V2_PRODUCT_ARGV_OVERFLOW",
                "wrapper argv byte count overflow",
            )
        })
    })?;
    if argv.is_empty()
        || argv.len() > limits.wrapper_argv_items
        || bytes > limits.wrapper_argv_bytes as usize
        || argv.first().map(String::as_str) != Some(PREPARED_RUSTC)
        || !argv.iter().any(|argument| argument == PRODUCT_TARGET_TRIPLE)
    {
        return Err(BuildError::new(
            "V2_PRODUCT_ARGV",
            format!("unit {} has invalid {context} rustc argv", unit.id),
        ));
    }
    Ok(())
}

fn product_unit_value(
    active: &ActiveProductEvidence<'_>,
    unit: &ProductCompilerUnitEvidence,
    closure: &product_closures::Closure,
) -> Result<Value, BuildError> {
    Ok(json!({
        "id": unit.id,
        "closure": unit.closure,
        "execution": active.execution.id,
        "workspace": closure.workspace.id,
        "package": closure.package.id,
        "target": closure.target.id,
        "crate_name": closure.target.name,
        "crate_types": closure.target.crate_types,
        "target_triple": active.compiler_target_triple,
        "profile": closure.cargo.profile,
        "features": sorted_unique_strings(
            &unit.features,
            "compiler feature",
        )?,
        "cfgs": sorted_unique_strings(&unit.cfgs, "compiler cfg")?,
        "original_argv": unit.original_argv,
        "effective_argv": unit.effective_argv,
        "dep_info_path": path_text(&unit.dep_info_path, "dep-info")?,
        "dep_info_digest": unit.dep_info_digest,
        "dep_info_bytes": unit.dep_info_bytes,
        "trace_digest": unit.trace_digest,
        "trace_bytes": unit.trace_bytes,
        "status": unit.status,
    }))
}

fn validate_cargo_unit_joins(
    active: &ActiveProductEvidence<'_>,
    units: &BTreeMap<String, String>,
) -> Result<(), BuildError> {
    let foundation = required_unit(units, SHIPPING_CLOSURE)?;
    let foundation_consumer = required_unit(units, CONSUMER_CLOSURE)?;
    let kernel = required_unit(units, KERNEL_INTERFACE_SHIPPING_CLOSURE)?;
    let kernel_consumer =
        required_unit(units, KERNEL_INTERFACE_CONSUMER_CLOSURE)?;
    for build in active.cargo_builds {
        let expected = units.get(&build.closure).ok_or_else(|| {
            BuildError::new(
                "V2_PRODUCT_CARGO_UNIT",
                "Cargo capture references an unknown closure",
            )
        })?;
        if build.compiled_units != [expected.clone()] {
            return Err(BuildError::new(
                "V2_PRODUCT_CARGO_UNIT",
                "Cargo compiled-unit set differs from wrapper evidence",
            ));
        }
        let expected_fresh = match build.closure.as_str() {
            SHIPPING_CLOSURE => BTreeSet::new(),
            CONSUMER_CLOSURE | KERNEL_INTERFACE_SHIPPING_CLOSURE => {
                BTreeSet::from([foundation.clone()])
            }
            KERNEL_INTERFACE_CONSUMER_CLOSURE => {
                BTreeSet::from([foundation.clone(), kernel.clone()])
            }
            _ => {
                return Err(BuildError::new(
                    "V2_PRODUCT_CARGO_UNIT",
                    "Cargo capture references an unknown closure",
                ));
            }
        };
        let actual_fresh =
            build.fresh_provider_units.iter().cloned().collect::<BTreeSet<_>>();
        if actual_fresh.len() != build.fresh_provider_units.len()
            || actual_fresh != expected_fresh
        {
            return Err(BuildError::new(
                "V2_PRODUCT_CARGO_FRESH",
                "Cargo fresh-provider set differs from the closure pair",
            ));
        }
        let event_compiled = build
            .artifact_events
            .iter()
            .filter(|event| !event.fresh)
            .map(|event| event.unit.clone())
            .collect::<BTreeSet<_>>();
        let event_fresh = build
            .artifact_events
            .iter()
            .filter(|event| event.fresh)
            .map(|event| event.unit.clone())
            .collect::<BTreeSet<_>>();
        if event_compiled != BTreeSet::from([expected.clone()])
            || event_fresh != expected_fresh
            || build.artifact_events.len()
                != event_compiled.len() + event_fresh.len()
        {
            return Err(BuildError::new(
                "V2_PRODUCT_CARGO_ARTIFACT_SET",
                "Cargo artifact events differ from compiled and fresh units",
            ));
        }
    }
    let distinct = [foundation, foundation_consumer, kernel, kernel_consumer]
        .into_iter()
        .collect::<BTreeSet<_>>();
    if distinct.len() != 4 {
        return Err(BuildError::new(
            "V2_PRODUCT_UNIT_DUPLICATE",
            "the four closures do not use distinct compiler units",
        ));
    }
    Ok(())
}

fn required_unit<'a>(
    units: &'a BTreeMap<String, String>,
    closure: &str,
) -> Result<&'a String, BuildError> {
    units.get(closure).ok_or_else(|| {
        BuildError::new(
            "V2_PRODUCT_UNIT_MISSING",
            format!("closure {closure} has no compiler unit"),
        )
    })
}

fn require_under_path(
    path: &Path,
    root: &Path,
    context: &str,
) -> Result<(), BuildError> {
    if path.is_absolute() && path != root && path.starts_with(root) {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_SCRATCH_PATH",
        format!("{context} {} is outside scratch", path.display()),
    ))
}

fn validate_product_path(
    inputs: &BuildInputs<'_>,
    path: &Path,
    context: &str,
) -> Result<(), BuildError> {
    let text = path_text(path, context)?;
    if normalized_absolute_path(text)
        && text.len() <= inputs.topology.limits.path_bytes
    {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_PATH",
        format!("{context} path is not normalized and bounded"),
    ))
}

fn product_input_values(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
    registry: &ActiveRegistry<'_>,
    units: &ProductUnitValues,
    observations: &BTreeMap<&Path, &SourceFileObservation>,
) -> Result<Vec<Value>, BuildError> {
    validate_exact_compiler_source_sets(active.product_sources)?;
    let sets = [
        (
            registry.foundation.shipping,
            &active.product_sources.foundation.shipping_sources,
        ),
        (
            registry.foundation.consumer,
            &active.product_sources.foundation.consumer_sources,
        ),
        (
            registry.kernel_interface.shipping,
            &active.product_sources.kernel_interface.shipping_sources,
        ),
        (
            registry.kernel_interface.consumer,
            &active.product_sources.kernel_interface.consumer_sources,
        ),
    ];
    let count = sets.iter().try_fold(0_usize, |total, (_, paths)| {
        total.checked_add(paths.len()).ok_or_else(|| {
            BuildError::new(
                "V2_PRODUCT_INPUT_LIMIT",
                "compiler input count overflow",
            )
        })
    })?;
    if count
        > usize::from(
            inputs.topology.limits.product_execution.compiler_input_count,
        )
    {
        return Err(BuildError::new(
            "V2_PRODUCT_INPUT_LIMIT",
            "compiler source inputs differ from the exact retained set",
        ));
    }
    let mut values = Vec::with_capacity(count);
    for (closure, paths) in sets {
        append_product_inputs(
            inputs,
            observations,
            units,
            closure,
            paths,
            &mut values,
        )?;
    }
    values.sort_by(|left, right| {
        let left_key = (left["closure"].as_str(), left["path"].as_str());
        let right_key = (right["closure"].as_str(), right["path"].as_str());
        left_key.cmp(&right_key)
    });
    Ok(values)
}

fn validate_exact_compiler_source_sets(
    observed: &product_source::TwoPairSourceObservation,
) -> Result<(), BuildError> {
    let expected = product_preflight::compiler_source_selection();
    let exact = observed
        .foundation
        .shipping_sources
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        == expected.foundation_shipping
        && observed
            .foundation
            .consumer_sources
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            == expected.foundation_consumer
        && observed
            .kernel_interface
            .shipping_sources
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            == expected.kernel_interface_shipping
        && observed
            .kernel_interface
            .consumer_sources
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            == expected.kernel_interface_consumer;
    if exact {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_INPUT_SET",
        "source analysis differs from exact preflight compiler selection",
    ))
}

fn append_product_inputs(
    inputs: &BuildInputs<'_>,
    observations: &BTreeMap<&Path, &SourceFileObservation>,
    units: &ProductUnitValues,
    closure: &product_closures::Closure,
    paths: &[PathBuf],
    output: &mut Vec<Value>,
) -> Result<(), BuildError> {
    let unit = units.by_closure.get(&closure.id).ok_or_else(|| {
        BuildError::new(
            "V2_PRODUCT_INPUT_UNIT",
            format!("closure {} has no compiler unit", closure.id),
        )
    })?;
    let unique = paths.iter().collect::<BTreeSet<_>>();
    if unique.len() != paths.len() || !unique.contains(&closure.target.src_path)
    {
        return Err(BuildError::new(
            "V2_PRODUCT_INPUT_SET",
            format!("closure {} has an invalid source-input set", closure.id),
        ));
    }
    for path in unique {
        let source = inputs
            .inventory
            .files
            .iter()
            .find(|file| file.path == *path)
            .ok_or_else(|| {
                BuildError::new(
                    "V2_PRODUCT_INPUT_SOURCE",
                    format!(
                        "{} is absent from source inventory",
                        path.display()
                    ),
                )
            })?;
        let observed = observations.get(path.as_path()).ok_or_else(|| {
            BuildError::new(
                "V2_PRODUCT_INPUT_SOURCE",
                format!("{} has no source observation", path.display()),
            )
        })?;
        validate_input_classification(source, closure)?;
        let kind = if path == &closure.target.src_path {
            "crate_root"
        } else {
            "module"
        };
        output.push(json!({
            "kind": kind,
            "closure": closure.id,
            "unit": unit,
            "path": path_text(path, "Product compiler input")?,
            "content_digest": source.content_digest,
            "size_bytes": observed.size_bytes,
        }));
    }
    Ok(())
}

fn validate_input_classification(
    source: &crate::source::ClassifiedFile,
    closure: &product_closures::Closure,
) -> Result<(), BuildError> {
    let expected = SourceOwner::Product(closure.architectural_role);
    if source.classification.owner == expected
        && source.classification.provenance == SourceProvenance::Authored
        && source.classification.source_use == closure.source_use
    {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_INPUT_CLASSIFICATION",
        format!(
            "{} has the wrong Product classification",
            source.path.display()
        ),
    ))
}

struct ProductArtifactValues {
    values: Vec<Value>,
    by_unit_kind: BTreeMap<(String, String), ProductArtifactRef>,
}

struct ProductArtifactRef {
    closure: String,
    path: PathBuf,
    content_digest: String,
    size_bytes: u64,
}

fn product_artifact_values(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
    units: &ProductUnitValues,
) -> Result<ProductArtifactValues, BuildError> {
    let limits = inputs.topology.limits.product_execution;
    if active.artifacts.len() != 8
        || active.artifacts.len() > usize::from(limits.artifact_count)
    {
        return Err(BuildError::new(
            "V2_PRODUCT_ARTIFACT_SET",
            "active Product evidence requires rlib and rmeta per unit",
        ));
    }
    let mut artifacts = active.artifacts.iter().collect::<Vec<_>>();
    artifacts.sort_by(|left, right| {
        (&left.unit, &left.crate_type).cmp(&(&right.unit, &right.crate_type))
    });
    let mut total = 0_u64;
    let mut paths = BTreeSet::new();
    let mut values = Vec::with_capacity(8);
    let mut by_unit_kind = BTreeMap::new();
    for artifact in artifacts {
        let unit = units.by_id.get(&artifact.unit).ok_or_else(|| {
            BuildError::new(
                "V2_PRODUCT_ARTIFACT_UNIT",
                format!("artifact references unknown unit {}", artifact.unit),
            )
        })?;
        if artifact.size_bytes == 0
            || !artifact_extension_matches(artifact)
            || !paths.insert(artifact.path.clone())
        {
            return Err(BuildError::new(
                "V2_PRODUCT_ARTIFACT_SHAPE",
                format!("unit {} has an invalid artifact", artifact.unit),
            ));
        }
        require_under_path(
            &artifact.path,
            Path::new(PREPARED_TARGET_DIR),
            "Product artifact",
        )?;
        validate_product_path(inputs, &artifact.path, "Product artifact")?;
        total = checked_evidence_sum(total, artifact.size_bytes)?;
        if total > u64::from(limits.artifact_total_bytes) {
            return Err(BuildError::new(
                "V2_PRODUCT_ARTIFACT_LIMIT",
                "Product artifact bytes exceed their retained bound",
            ));
        }
        let retained = ProductArtifactRef {
            closure: unit.closure.clone(),
            path: artifact.path.clone(),
            content_digest: artifact.content_digest.clone(),
            size_bytes: artifact.size_bytes,
        };
        let key = (artifact.unit.clone(), artifact.crate_type.clone());
        if by_unit_kind.insert(key, retained).is_some() {
            return Err(BuildError::new(
                "V2_PRODUCT_ARTIFACT_DUPLICATE",
                "compiler unit repeats one artifact kind",
            ));
        }
        values.push(json!({
            "closure": unit.closure,
            "unit": artifact.unit,
            "path": path_text(&artifact.path, "Product artifact")?,
            "content_digest": artifact.content_digest,
            "size_bytes": artifact.size_bytes,
            "crate_type": artifact.crate_type,
        }));
    }
    let expected = artifact_coverage_keys(units.by_id.keys());
    if by_unit_kind.keys().cloned().collect::<BTreeSet<_>>() != expected {
        return Err(BuildError::new(
            "V2_PRODUCT_ARTIFACT_COVERAGE",
            "compiler unit lacks rlib or rmeta artifact coverage",
        ));
    }
    validate_product_artifact_paths(units, &by_unit_kind)?;
    Ok(ProductArtifactValues { values, by_unit_kind })
}

fn artifact_extension_matches(artifact: &ProductArtifactEvidence) -> bool {
    matches!(artifact.crate_type.as_str(), "rlib" | "rmeta")
        && artifact.path.extension().and_then(|value| value.to_str())
            == Some(artifact.crate_type.as_str())
}

fn validate_product_artifact_paths(
    units: &ProductUnitValues,
    artifacts: &BTreeMap<(String, String), ProductArtifactRef>,
) -> Result<(), BuildError> {
    for (unit_id, unit) in &units.by_id {
        let rlib = artifacts
            .get(&(unit_id.clone(), "rlib".to_owned()))
            .ok_or_else(|| {
                BuildError::new(
                    "V2_PRODUCT_ARTIFACT_COVERAGE",
                    "compiler unit rlib is absent",
                )
            })?;
        let rmeta = artifacts
            .get(&(unit_id.clone(), "rmeta".to_owned()))
            .ok_or_else(|| {
                BuildError::new(
                    "V2_PRODUCT_ARTIFACT_COVERAGE",
                    "compiler unit rmeta is absent",
                )
            })?;
        compiler_capture::validate_reported_unit_outputs(
            &unit.original_argv,
            crate_name_for_closure(&unit.closure)?,
            &rlib.path,
            &rmeta.path,
        )
        .map_err(|failure| {
            BuildError::new(failure.code, failure.to_string())
        })?;
    }
    Ok(())
}

fn product_provider_artifact_values(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
    units: &ProductUnitValues,
    artifacts: &ProductArtifactValues,
) -> Result<Vec<Value>, BuildError> {
    if active.provider_artifacts.len() != 2 {
        return Err(BuildError::new(
            "V2_PRODUCT_PROVIDER_SET",
            "active Product evidence requires two provider artifacts",
        ));
    }
    let mut providers = active
        .provider_artifacts
        .iter()
        .map(|provider| (provider.unit.as_str(), provider))
        .collect::<BTreeMap<_, _>>();
    if providers.len() != active.provider_artifacts.len() {
        return Err(BuildError::new(
            "V2_PRODUCT_PROVIDER_SET",
            "provider artifact unit is duplicated",
        ));
    }
    let mut values = Vec::with_capacity(2);
    let mut top_paths = BTreeSet::new();
    for closure in [SHIPPING_CLOSURE, KERNEL_INTERFACE_SHIPPING_CLOSURE] {
        let unit = required_unit(&units.by_closure, closure)?;
        let provider = providers.remove(unit.as_str()).ok_or_else(|| {
            BuildError::new(
                "V2_PRODUCT_PROVIDER_UNIT",
                format!("shipping closure {closure} lacks its provider"),
            )
        })?;
        if !top_paths.insert(provider.top_level_rlib.path.clone())
            || active
                .artifacts
                .iter()
                .any(|artifact| artifact.path == provider.top_level_rlib.path)
        {
            return Err(BuildError::new(
                "V2_PRODUCT_PROVIDER_BYTES",
                "provider top-level rlib path is duplicated or hashed",
            ));
        }
        values.push(product_provider_artifact_value(
            inputs, active, units, artifacts, provider, closure,
        )?);
    }
    if !providers.is_empty() {
        return Err(BuildError::new(
            "V2_PRODUCT_PROVIDER_SET",
            "provider artifact references a nonshipping unit",
        ));
    }
    validate_compiled_consumer_events(active, units, artifacts)?;
    values.sort_by(|left, right| {
        left["unit"].as_str().cmp(&right["unit"].as_str())
    });
    Ok(values)
}

fn product_provider_artifact_value(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
    units: &ProductUnitValues,
    artifacts: &ProductArtifactValues,
    provider: &ProductProviderArtifactEvidence,
    shipping_closure: &str,
) -> Result<Value, BuildError> {
    let shipping = required_unit(&units.by_closure, shipping_closure)?;
    let hashed = artifacts
        .by_unit_kind
        .get(&(shipping.clone(), "rlib".to_owned()))
        .ok_or_else(|| {
            BuildError::new(
                "V2_PRODUCT_PROVIDER_ARTIFACT",
                "shipping hashed rlib is absent",
            )
        })?;
    let rmeta = artifacts
        .by_unit_kind
        .get(&(shipping.clone(), "rmeta".to_owned()))
        .ok_or_else(|| {
            BuildError::new(
                "V2_PRODUCT_PROVIDER_ARTIFACT",
                "shipping rmeta is absent",
            )
        })?;
    require_provider_identity(&provider.hashed_rlib, hashed, "hashed rlib")?;
    require_provider_identity(&provider.rmeta, rmeta, "rmeta")?;
    let top = &provider.top_level_rlib;
    require_under_path(
        &top.path,
        Path::new(PREPARED_TARGET_DIR),
        "top-level Cargo rlib",
    )?;
    validate_product_path(inputs, &top.path, "top-level Cargo rlib")?;
    if top.path
        != product_top_level_rlib_path(crate_name_for_closure(
            shipping_closure,
        )?)
        || top.path == provider.hashed_rlib.path
        || top.path.extension().and_then(|value| value.to_str()) != Some("rlib")
        || provider
            .hashed_rlib
            .path
            .extension()
            .and_then(|value| value.to_str())
            != Some("rlib")
        || provider.rmeta.path.extension().and_then(|value| value.to_str())
            != Some("rmeta")
        || top.content_digest != provider.hashed_rlib.content_digest
        || top.size_bytes == 0
        || top.size_bytes != provider.hashed_rlib.size_bytes
    {
        return Err(BuildError::new(
            "V2_PRODUCT_PROVIDER_BYTES",
            "top-level and hashed provider paths/bytes are not exact",
        ));
    }
    validate_provider_cargo_events(active, provider, shipping_closure)?;
    Ok(json!({
        "unit": provider.unit,
        "top_level_rlib": product_file_identity_value(top)?,
        "hashed_rlib": product_file_identity_value(&provider.hashed_rlib)?,
        "rmeta": product_file_identity_value(&provider.rmeta)?,
    }))
}

fn require_provider_identity(
    observed: &ProductFileIdentityEvidence,
    retained: &ProductArtifactRef,
    context: &str,
) -> Result<(), BuildError> {
    if observed.path == retained.path
        && observed.content_digest == retained.content_digest
        && observed.size_bytes == retained.size_bytes
    {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_PROVIDER_ARTIFACT",
        format!("provider {context} differs from wrapper artifact identity"),
    ))
}

fn validate_provider_cargo_events(
    active: &ActiveProductEvidence<'_>,
    provider: &ProductProviderArtifactEvidence,
    shipping_closure: &str,
) -> Result<(), BuildError> {
    let shipping = active
        .cargo_builds
        .iter()
        .find(|build| build.closure == shipping_closure)
        .ok_or_else(|| {
            BuildError::new(
                "V2_PRODUCT_CARGO_ARTIFACT_SET",
                "shipping Cargo artifact capture is absent",
            )
        })?;
    let shipping_files = BTreeSet::from([
        provider.top_level_rlib.path.clone(),
        provider.rmeta.path.clone(),
    ]);
    require_cargo_event(
        &shipping.artifact_events,
        &provider.unit,
        false,
        &shipping_files,
    )?;
    let fresh_files = BTreeSet::from([
        provider.hashed_rlib.path.clone(),
        provider.rmeta.path.clone(),
    ]);
    let consumers: &[&str] = if shipping_closure == SHIPPING_CLOSURE {
        &[
            CONSUMER_CLOSURE,
            KERNEL_INTERFACE_SHIPPING_CLOSURE,
            KERNEL_INTERFACE_CONSUMER_CLOSURE,
        ]
    } else {
        &[KERNEL_INTERFACE_CONSUMER_CLOSURE]
    };
    for closure in consumers {
        let consumer = active
            .cargo_builds
            .iter()
            .find(|build| build.closure == *closure)
            .ok_or_else(|| {
                BuildError::new(
                    "V2_PRODUCT_CARGO_ARTIFACT_SET",
                    format!("closure {closure} lacks its Cargo capture"),
                )
            })?;
        require_cargo_event(
            &consumer.artifact_events,
            &provider.unit,
            true,
            &fresh_files,
        )?;
    }
    if shipping.artifact_events.iter().filter(|event| !event.fresh).count() == 1
    {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_CARGO_ARTIFACT_SET",
        "shipping Cargo capture has an unexpected compiled event set",
    ))
}

fn validate_compiled_consumer_events(
    active: &ActiveProductEvidence<'_>,
    units: &ProductUnitValues,
    artifacts: &ProductArtifactValues,
) -> Result<(), BuildError> {
    for closure in [CONSUMER_CLOSURE, KERNEL_INTERFACE_CONSUMER_CLOSURE] {
        let unit = required_unit(&units.by_closure, closure)?;
        let build = active
            .cargo_builds
            .iter()
            .find(|build| build.closure == closure)
            .ok_or_else(|| {
                BuildError::new(
                    "V2_PRODUCT_CARGO_ARTIFACT_SET",
                    format!("closure {closure} lacks its Cargo capture"),
                )
            })?;
        let rmeta = artifacts
            .by_unit_kind
            .get(&(unit.clone(), "rmeta".to_owned()))
            .ok_or_else(|| {
                BuildError::new(
                    "V2_PRODUCT_CARGO_ARTIFACT_SET",
                    "consumer compiler rmeta is absent",
                )
            })?;
        let filenames = BTreeSet::from([
            product_top_level_rlib_path(crate_name_for_closure(closure)?),
            rmeta.path.clone(),
        ]);
        require_cargo_event(&build.artifact_events, unit, false, &filenames)?;
    }
    Ok(())
}

fn product_top_level_rlib_path(crate_name: &str) -> PathBuf {
    Path::new(PREPARED_TARGET_DIR)
        .join(PRODUCT_TARGET_TRIPLE)
        .join("release")
        .join(format!("lib{crate_name}.rlib"))
}

fn require_cargo_event(
    events: &[ProductCargoArtifactEventEvidence],
    unit: &str,
    fresh: bool,
    filenames: &BTreeSet<PathBuf>,
) -> Result<(), BuildError> {
    let found = events.iter().any(|event| {
        event.unit == unit
            && event.fresh == fresh
            && event.filenames.iter().cloned().collect::<BTreeSet<_>>()
                == *filenames
    });
    if found {
        Ok(())
    } else {
        Err(BuildError::new(
            "V2_PRODUCT_CARGO_ARTIFACT_JOIN",
            "Cargo artifact event differs from provider evidence",
        ))
    }
}

fn product_file_identity_value(
    value: &ProductFileIdentityEvidence,
) -> Result<Value, BuildError> {
    Ok(json!({
        "path": path_text(&value.path, "provider artifact")?,
        "content_digest": value.content_digest,
        "size_bytes": value.size_bytes,
    }))
}

fn artifact_coverage_keys<'a>(
    units: impl Iterator<Item = &'a String>,
) -> BTreeSet<(String, String)> {
    units
        .flat_map(|unit| {
            ["rlib", "rmeta"]
                .into_iter()
                .map(|kind| (unit.clone(), kind.to_owned()))
        })
        .collect()
}

fn product_extern_values(
    active: &ActiveProductEvidence<'_>,
    units: &ProductUnitValues,
    artifacts: &ProductArtifactValues,
) -> Result<Vec<Value>, BuildError> {
    if active.extern_edges.len() != 3 {
        return Err(BuildError::new(
            "V2_PRODUCT_EXTERN_SET",
            "active Product evidence requires three direct extern edges",
        ));
    }
    let mut edges = active.extern_edges.iter().collect::<Vec<_>>();
    edges.sort_by(|left, right| left.consumer_unit.cmp(&right.consumer_unit));
    let mut consumers = BTreeSet::new();
    let mut values = Vec::with_capacity(3);
    for edge in edges {
        let consumer =
            units.by_id.get(&edge.consumer_unit).ok_or_else(|| {
                BuildError::new(
                    "V2_PRODUCT_EXTERN_CONSUMER",
                    "extern edge references an unknown consumer unit",
                )
            })?;
        let provider =
            units.by_id.get(&edge.provider_unit).ok_or_else(|| {
                BuildError::new(
                    "V2_PRODUCT_EXTERN_PROVIDER",
                    "extern edge references an unknown provider unit",
                )
            })?;
        let artifact = artifacts
            .by_unit_kind
            .get(&(edge.provider_unit.clone(), "rmeta".to_owned()))
            .ok_or_else(|| {
                BuildError::new(
                    "V2_PRODUCT_EXTERN_ARTIFACT",
                    "extern provider has no retained rmeta artifact",
                )
            })?;
        let (provider_closure, extern_name) =
            expected_extern_relation(&consumer.closure)?;
        let exact = consumers.insert(consumer.closure.clone())
            && provider.closure == provider_closure
            && artifact.closure == provider_closure
            && edge.extern_name == extern_name
            && edge.provider_path == artifact.path;
        if !exact {
            return Err(BuildError::new(
                "V2_PRODUCT_EXTERN_JOIN",
                "extern edge differs from the exact direct Product edge",
            ));
        }
        values.push(json!({
            "closure": consumer.closure,
            "unit": edge.consumer_unit,
            "extern_name": edge.extern_name,
            "provider": {
                "kind": "compiler_unit",
                "unit": edge.provider_unit,
                "path": path_text(&edge.provider_path, "extern provider")?,
            },
            "decision": "admitted_product_unit",
        }));
    }
    let expected_consumers = BTreeSet::from([
        CONSUMER_CLOSURE.to_owned(),
        KERNEL_INTERFACE_SHIPPING_CLOSURE.to_owned(),
        KERNEL_INTERFACE_CONSUMER_CLOSURE.to_owned(),
    ]);
    if consumers != expected_consumers {
        return Err(BuildError::new(
            "V2_PRODUCT_EXTERN_SET",
            "direct extern consumer set differs from the exact profile",
        ));
    }
    Ok(values)
}

fn expected_extern_relation(
    consumer_closure: &str,
) -> Result<(&'static str, &'static str), BuildError> {
    match consumer_closure {
        CONSUMER_CLOSURE | KERNEL_INTERFACE_SHIPPING_CLOSURE => {
            Ok((SHIPPING_CLOSURE, "reovim_foundation"))
        }
        KERNEL_INTERFACE_CONSUMER_CLOSURE => {
            Ok((KERNEL_INTERFACE_SHIPPING_CLOSURE, "reovim_kernel_interface"))
        }
        _ => Err(BuildError::new(
            "V2_PRODUCT_EXTERN_JOIN",
            "shipping or unknown closure cannot own a direct extern edge",
        )),
    }
}

fn package_membership_values(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
    registry: &ActiveRegistry<'_>,
    observations: &BTreeMap<&Path, &SourceFileObservation>,
) -> Result<Vec<Value>, BuildError> {
    let pairs = [
        (
            registry.foundation.shipping,
            active.foundation_package_before,
            active.foundation_package_after,
        ),
        (
            registry.kernel_interface.shipping,
            active.kernel_interface_package_before,
            active.kernel_interface_package_after,
        ),
    ];
    let mut values = pairs
        .into_iter()
        .map(|(shipping, before, after)| {
            package_membership_value(
                inputs,
                shipping,
                before,
                after,
                observations,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    values.sort_by(|left, right| {
        left["closure"].as_str().cmp(&right["closure"].as_str())
    });
    Ok(values)
}

fn package_membership_value(
    inputs: &BuildInputs<'_>,
    shipping: &product_closures::Closure,
    before: &package_capture::Observation,
    after: &package_capture::Observation,
    observations: &BTreeMap<&Path, &SourceFileObservation>,
) -> Result<Value, BuildError> {
    validate_package_observations(inputs, shipping, before, after)?;
    let membership = shipping.package_membership.as_ref().ok_or_else(|| {
        BuildError::new(
            "V2_PRODUCT_PACKAGE_REGISTRY",
            "shipping closure lacks package membership authority",
        )
    })?;
    let observed = before.members.iter().cloned().collect::<BTreeSet<_>>();
    let expected = membership
        .members
        .iter()
        .map(|member| member.package_path().clone())
        .collect::<BTreeSet<_>>();
    if observed != expected {
        return Err(BuildError::new(
            "V2_PRODUCT_PACKAGE_MEMBER_SET",
            "Cargo package-list members differ from registry authority",
        ));
    }
    let mut members = membership.members.iter().collect::<Vec<_>>();
    members
        .sort_by(|left, right| left.package_path().cmp(right.package_path()));
    let members = members
        .into_iter()
        .map(|member| {
            package_member_value(inputs, observations, shipping, member)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut captures = [before, after]
        .into_iter()
        .map(package_capture_value)
        .collect::<Vec<_>>();
    captures.sort_by(|left, right| {
        left["phase"].as_str().cmp(&right["phase"].as_str())
    });
    Ok(json!({
        "closure": shipping.id,
        "workspace": shipping.workspace.id,
        "package": shipping.package.id,
        "captures": captures,
        "members": members,
    }))
}

fn validate_package_observations(
    inputs: &BuildInputs<'_>,
    shipping: &product_closures::Closure,
    before: &package_capture::Observation,
    after: &package_capture::Observation,
) -> Result<(), BuildError> {
    let limit =
        u64::from(inputs.topology.limits.product_execution.package_list_bytes);
    let member_limit =
        inputs.topology.limits.product_execution.package_member_count;
    if before.phase != package_capture::Phase::BeforeCompiler
        || after.phase != package_capture::Phase::AfterCompiler
        || before.dirty_mode != package_capture::DirtyMode::Reject
        || after.dirty_mode != package_capture::DirtyMode::Reject
        || before.stdout_bytes == 0
        || before.stdout_bytes > limit
        || after.stdout_bytes == 0
        || after.stdout_bytes > limit
        || before.member_path_bytes > limit
        || after.member_path_bytes > limit
        || before.members.len() > member_limit
        || after.members.len() > member_limit
    {
        return Err(BuildError::new(
            "V2_PRODUCT_PACKAGE_CAPTURE",
            "before/after package captures exceed the exact profile",
        ));
    }
    package_capture::compare(before, after)
        .map_err(|failure| BuildError::new(failure.code, failure.message))?;
    validate_package_argv(inputs, shipping, before)?;
    validate_package_argv(inputs, shipping, after)
}

fn validate_package_argv(
    _inputs: &BuildInputs<'_>,
    shipping: &product_closures::Closure,
    observed: &package_capture::Observation,
) -> Result<(), BuildError> {
    let expected = [
        PREPARED_CARGO,
        "package",
        "--list",
        "--quiet",
        "--locked",
        "--offline",
        "--no-metadata",
        "--manifest-path",
        PREPARED_MANIFEST,
        "--package",
        shipping.package.name.as_str(),
    ];
    if observed.command_argv.iter().map(String::as_str).ne(expected) {
        return Err(BuildError::new(
            "V2_PRODUCT_PACKAGE_ARGV",
            "Cargo package-list argv differs from the exact selection",
        ));
    }
    Ok(())
}

fn package_capture_value(observed: &package_capture::Observation) -> Value {
    json!({
        "phase": observed.phase.as_str(),
        "command_argv": observed.command_argv,
        "raw_output_digest": observed.raw_output_digest,
        "semantic_digest": observed.semantic_digest,
        "stdout_bytes": observed.stdout_bytes,
    })
}

fn package_member_value(
    inputs: &BuildInputs<'_>,
    observations: &BTreeMap<&Path, &SourceFileObservation>,
    shipping: &product_closures::Closure,
    member: &product_closures::PackageMember,
) -> Result<Value, BuildError> {
    match member {
        product_closures::PackageMember::CargoGenerated { package_path } => {
            Ok(json!({
                "kind": "cargo_generated",
                "package_path": path_text(
                    package_path,
                    "generated package member",
                )?,
            }))
        }
        product_closures::PackageMember::Copied {
            package_path,
            repository_path,
        } => copied_package_member_value(
            inputs,
            observations,
            shipping,
            package_path,
            repository_path,
        ),
    }
}

fn copied_package_member_value(
    inputs: &BuildInputs<'_>,
    observations: &BTreeMap<&Path, &SourceFileObservation>,
    shipping: &product_closures::Closure,
    package_path: &Path,
    repository_path: &Path,
) -> Result<Value, BuildError> {
    let source = inputs
        .inventory
        .files
        .iter()
        .find(|file| file.path == repository_path)
        .ok_or_else(|| {
            BuildError::new(
                "V2_PRODUCT_PACKAGE_SOURCE",
                format!(
                    "{} is absent from inventory",
                    repository_path.display()
                ),
            )
        })?;
    let observed = observations.get(repository_path).ok_or_else(|| {
        BuildError::new(
            "V2_PRODUCT_PACKAGE_SOURCE",
            format!("{} has no source observation", repository_path.display()),
        )
    })?;
    if source.classification.owner
        != SourceOwner::Product(shipping.architectural_role)
        || source.classification.provenance != SourceProvenance::Authored
        || source.classification.source_use != shipping.source_use
    {
        return Err(BuildError::new(
            "V2_PRODUCT_PACKAGE_CLASSIFICATION",
            format!("{} is not Product/Production", repository_path.display()),
        ));
    }
    Ok(json!({
        "kind": "copied",
        "package_path": path_text(package_path, "copied package member")?,
        "repository_path": path_text(
            repository_path,
            "copied repository source",
        )?,
        "content_digest": source.content_digest,
        "size_bytes": observed.size_bytes,
    }))
}

fn public_consumer_values(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
    registry: &ActiveRegistry<'_>,
) -> Result<Vec<Value>, BuildError> {
    let pairs = [
        (registry.foundation, &active.product_sources.foundation),
        (registry.kernel_interface, &active.product_sources.kernel_interface),
    ];
    let mut values = pairs
        .into_iter()
        .map(|(pair, source)| public_consumer_value(inputs, pair, source))
        .collect::<Result<Vec<_>, _>>()?;
    values.sort_by(|left, right| {
        left["relation"].as_str().cmp(&right["relation"].as_str())
    });
    Ok(values)
}

fn public_consumer_value(
    inputs: &BuildInputs<'_>,
    pair: ActivePair<'_>,
    source: &product_source::ProductSourceObservation,
) -> Result<Value, BuildError> {
    validate_product_source_observation(source, pair)?;
    if source.public_items != pair.relation.public_items
        || source.derive_checks != pair.relation.derive_checks
        || source.public_items.is_empty()
        || source.public_items.len()
            > inputs.topology.limits.product_execution.public_item_count
    {
        return Err(BuildError::new(
            "V2_PRODUCT_PUBLIC_SURFACE",
            "analyzed public surface differs from registry authority",
        ));
    }
    let items = source
        .public_items
        .iter()
        .map(|item| {
            json!({
                "path": item.path,
                "kind": public_item_kind_text(item.kind),
            })
        })
        .collect::<Vec<_>>();
    let checks = source
        .derive_checks
        .iter()
        .map(|check| {
            json!({
                "derive": derive_text(check.derive),
                "public_type": check.public_type,
                "consumer_operation": operation_text(
                    check.consumer_operation,
                ),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "relation": pair.relation.id,
        "shipping_closure": pair.relation.shipping_closure,
        "consumer_closure": pair.relation.consumer_closure,
        "public_items": items,
        "derive_checks": checks,
        "status": "pass",
    }))
}

fn validate_product_source_observation(
    source: &product_source::ProductSourceObservation,
    pair: ActivePair<'_>,
) -> Result<(), BuildError> {
    let membership =
        pair.shipping.package_membership.as_ref().ok_or_else(|| {
            BuildError::new(
                "V2_PRODUCT_SOURCE_SET",
                "shipping package membership is absent",
            )
        })?;
    let shipping = membership
        .members
        .iter()
        .filter_map(|member| match member {
            product_closures::PackageMember::Copied {
                repository_path, ..
            } => Some(repository_path.clone()),
            product_closures::PackageMember::CargoGenerated { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    let expected_shipping = shipping.iter().cloned().collect::<Vec<_>>();
    let selections = product_preflight::compiler_source_selection();
    let expected_consumer = match pair.relation.id.as_str() {
        PUBLIC_RELATION => selections.foundation_consumer,
        KERNEL_INTERFACE_PUBLIC_RELATION => {
            selections.kernel_interface_consumer
        }
        _ => {
            return Err(BuildError::new(
                "V2_PRODUCT_SOURCE_SET",
                "source observation references an unknown public relation",
            ));
        }
    }
    .into_iter()
    .collect::<Vec<_>>();
    let expected_witnesses = pair
        .relation
        .public_items
        .iter()
        .filter(|item| item.kind == product_closures::PublicItemKind::Type)
        .map(|item| item.path.clone())
        .collect::<Vec<_>>();
    if source.relation != pair.relation.id
        || source.shipping_sources != expected_shipping
        || source.consumer_sources != expected_consumer
        || source.witness_types != expected_witnesses
    {
        return Err(BuildError::new(
            "V2_PRODUCT_SOURCE_SET",
            "analyzed source or witness set differs from closure authority",
        ));
    }
    Ok(())
}

const fn public_item_kind_text(
    kind: product_closures::PublicItemKind,
) -> &'static str {
    match kind {
        product_closures::PublicItemKind::Module => "module",
        product_closures::PublicItemKind::Type => "type",
        product_closures::PublicItemKind::Variant => "variant",
        product_closures::PublicItemKind::Trait => "trait",
        product_closures::PublicItemKind::Function => "function",
        product_closures::PublicItemKind::Const => "const",
        product_closures::PublicItemKind::Static => "static",
    }
}

const fn derive_text(derive: product_closures::BuiltinDerive) -> &'static str {
    match derive {
        product_closures::BuiltinDerive::Clone => "Clone",
        product_closures::BuiltinDerive::Copy => "Copy",
        product_closures::BuiltinDerive::Debug => "Debug",
        product_closures::BuiltinDerive::PartialEq => "PartialEq",
        product_closures::BuiltinDerive::Eq => "Eq",
        product_closures::BuiltinDerive::Hash => "Hash",
        product_closures::BuiltinDerive::Default => "Default",
    }
}

const fn operation_text(
    operation: product_closures::ConsumerOperation,
) -> &'static str {
    match operation {
        product_closures::ConsumerOperation::Clone => "clone",
        product_closures::ConsumerOperation::Copy => "copy",
        product_closures::ConsumerOperation::Debug => "debug",
        product_closures::ConsumerOperation::PartialEq => "partial_eq",
        product_closures::ConsumerOperation::Eq => "eq",
        product_closures::ConsumerOperation::Hash => "hash",
        product_closures::ConsumerOperation::Default => "default",
    }
}

fn product_execution_value(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
) -> Result<Value, BuildError> {
    validate_product_execution(inputs, active)?;
    let mut exposures = active.execution.exposures.iter().collect::<Vec<_>>();
    exposures.sort_by(|left, right| left.destination.cmp(&right.destination));
    let exposures = exposures
        .into_iter()
        .map(product_exposure_value)
        .collect::<Result<Vec<_>, _>>()?;
    let namespaces = &active.execution.namespaces;
    let network = &active.execution.network;
    let scratch = &active.execution.scratch;
    let cleanup = &active.execution.cleanup;
    Ok(json!({
        "id": active.execution.id,
        "environment": active.execution.environment,
        "closures": [
            SHIPPING_CLOSURE,
            CONSUMER_CLOSURE,
            KERNEL_INTERFACE_SHIPPING_CLOSURE,
            KERNEL_INTERFACE_CONSUMER_CLOSURE,
        ],
        "exposure": exposures,
        "namespaces": {
            "user": namespaces.user,
            "mount": namespaces.mount,
            "network": namespaces.network,
            "pid": namespaces.pid,
            "uid_map": namespaces.uid_map,
            "gid_map": namespaces.gid_map,
            "setgroups_denied": namespaces.setgroups_denied,
            "private_propagation": namespaces.private_propagation,
            "pid_one_reaper": namespaces.pid_one_reaper,
            "parent_death_signal": namespaces.parent_death_signal,
        },
        "network": {
            "interfaces": network.interfaces,
            "routes": network.routes,
            "kernel_route_count": network.kernel_route_count,
            "socket_count": network.socket_count,
            "policy_observation_digest":
                network.policy_observation_digest,
            "policy_basis": network.policy_basis,
            "external_carrier_absent": network.external_carrier_absent,
        },
        "scratch": {
            "mountpoint": path_text(&scratch.mountpoint, "scratch")?,
            "filesystem": "tmpfs",
            "size_limit_bytes": scratch.size_limit_bytes,
            "inode_limit": scratch.inode_limit,
            "allocated_bytes": scratch.allocated_bytes,
            "inode_count": scratch.inode_count,
            "logical_bytes": scratch.logical_bytes,
            "flags": ["mode=0700", "nodev", "noexec", "nosuid"],
        },
        "process_group": {
            "identity_validated":
                active.execution.process_group_identity_validated,
            "group_absent_after_reap":
                active.execution.process_group_absent_after_reap,
        },
        "cleanup": {
            "processes_absent": cleanup.processes_absent,
            "readers_joined": cleanup.readers_joined,
            "namespace_absent": cleanup.namespace_absent,
            "mount_absent": cleanup.mount_absent,
            "mountpoint_identity_preserved":
                cleanup.mountpoint_identity_preserved,
            "underlying_directory_empty": cleanup.underlying_directory_empty,
            "source_identity_preserved": cleanup.source_identity_preserved,
            "tool_identity_preserved": cleanup.tool_identity_preserved,
        },
    }))
}

fn validate_active_runtime_exposures(
    inputs: &BuildInputs<'_>,
    execution: &ProductExecutionEvidence,
) -> Result<(), BuildError> {
    let mut runtime_mappings = BTreeSet::new();
    let mut runtime_identities = BTreeMap::new();
    for (name, tool) in [
        ("checker", inputs.checker),
        ("Cargo", inputs.cargo),
        ("rustc", inputs.rustc),
    ] {
        if tool.runtime_inputs.is_empty() {
            return Err(BuildError::new(
                "V2_PRODUCT_RUNTIME_INPUT",
                format!("active Product {name} runtime inputs are empty"),
            ));
        }
        for input in &tool.runtime_inputs {
            let mapping =
                (input.canonical_path.clone(), input.prepared_path.clone());
            runtime_mappings.insert(mapping.clone());
            let identity = (input.content_digest.clone(), input.size_bytes);
            retain_runtime_identity(
                &mut runtime_identities,
                mapping,
                &identity,
            )?;
        }
    }
    let exposed = execution
        .exposures
        .iter()
        .filter(|exposure| exposure.purpose == "system runtime")
        .map(|exposure| (exposure.source.clone(), exposure.destination.clone()))
        .collect::<BTreeSet<_>>();
    if exposed.is_empty() || exposed != runtime_mappings {
        return Err(BuildError::new(
            "V2_PRODUCT_RUNTIME_EXPOSURE",
            "tool runtime mappings and system-runtime exposures differ",
        ));
    }
    for ((_, destination), (digest, size)) in runtime_identities {
        require_product_file_inventory(
            &execution.exposures,
            &destination,
            &digest,
            size,
        )?;
    }
    Ok(())
}

fn retain_runtime_identity(
    identities: &mut BTreeMap<(PathBuf, PathBuf), (String, u64)>,
    mapping: (PathBuf, PathBuf),
    identity: &(String, u64),
) -> Result<(), BuildError> {
    let prior = identities.insert(mapping, identity.clone());
    if prior.as_ref().is_none_or(|value| value == identity) {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_RUNTIME_INPUT",
        "shared runtime mapping has conflicting file identity",
    ))
}

fn product_exposure_value(
    exposure: &ProductExposureEvidence,
) -> Result<Value, BuildError> {
    let mut flags = Vec::new();
    if exposure.read_only {
        flags.push("ro");
    }
    if exposure.nosuid {
        flags.push("nosuid");
    }
    if exposure.nodev {
        flags.push("nodev");
    }
    if exposure.noexec {
        flags.push("noexec");
    }
    flags.sort_unstable();
    let inventory_entries = exposure
        .inventory_entries
        .iter()
        .map(product_inventory_entry_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "source": path_text(&exposure.source, "exposure source")?,
        "destination": path_text(
            &exposure.destination,
            "exposure destination",
        )?,
        "kind": exposure.kind.as_str(),
        "filesystem": "bind",
        "flags": flags,
        "purpose": exposure.purpose,
        "inventory_digest": exposure.inventory_digest,
        "inventory_entry_count": exposure.inventory_entry_count,
        "inventory_total_bytes": exposure.inventory_total_bytes,
        "inventory_entries": inventory_entries,
    }))
}

fn product_inventory_entry_value(
    entry: &prepared_root::InventoryEntry,
) -> Result<Value, BuildError> {
    Ok(json!({
        "relative_path": path_text(
            &entry.relative_path,
            "exposure inventory entry",
        )?,
        "kind": entry.kind.as_str(),
        "mode": entry.mode,
        "size_bytes": entry.size_bytes,
        "content_digest": entry.content_digest,
        "device_identity": entry.device_identity,
    }))
}

fn validate_product_execution(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
) -> Result<(), BuildError> {
    let execution = active.execution;
    if execution.id.is_empty() || execution.environment.is_empty() {
        return Err(BuildError::new(
            "V2_PRODUCT_EXECUTION_ID",
            "Product execution identity is empty",
        ));
    }
    validate_product_exposures(inputs, &execution.exposures)?;
    validate_product_exposure_manifest(inputs, active)?;
    validate_product_namespace(&execution.namespaces)?;
    validate_product_network(&execution.network)?;
    validate_product_scratch(inputs, &execution.scratch)?;
    if !execution.process_group_identity_validated
        || !execution.process_group_absent_after_reap
        || !complete_product_cleanup(&execution.cleanup)
    {
        return Err(BuildError::new(
            "V2_PRODUCT_EXECUTION_CLEANUP",
            "Product process-group or cleanup evidence is incomplete",
        ));
    }
    Ok(())
}

fn validate_product_exposures(
    inputs: &BuildInputs<'_>,
    exposures: &[ProductExposureEvidence],
) -> Result<(), BuildError> {
    let limit = usize::from(
        inputs.topology.limits.product_execution.exposure_entry_count,
    );
    if exposures.is_empty() || exposures.len() > limit {
        return Err(BuildError::new(
            "V2_PRODUCT_EXPOSURE_LIMIT",
            "Product exposures are empty or exceed their bound",
        ));
    }
    let mut destinations = BTreeSet::new();
    let mut character_devices = 0_usize;
    for exposure in exposures {
        validate_product_path(inputs, &exposure.source, "exposure source")?;
        validate_product_path(
            inputs,
            &exposure.destination,
            "exposure destination",
        )?;
        let identity = prepared_root::InventoryIdentity {
            entry_count: exposure.inventory_entry_count,
            total_bytes: exposure.inventory_total_bytes,
            aggregate_digest: exposure.inventory_digest.clone(),
            entries: exposure.inventory_entries.clone(),
        };
        if !exposure.source.is_absolute()
            || !exposure.destination.is_absolute()
            || exposure.purpose.is_empty()
            || !destinations.insert(&exposure.destination)
            || prepared_root::validate_inventory_identity(&identity).is_err()
        {
            return Err(BuildError::new(
                "V2_PRODUCT_EXPOSURE_SHAPE",
                "Product exposure paths or purpose are invalid",
            ));
        }
        if exposure.inventory_entry_count
            > usize::from(
                inputs
                    .topology
                    .limits
                    .product_execution
                    .compiler_substrate_entry_count,
            )
            || exposure.inventory_total_bytes
                > u64::from(
                    inputs
                        .topology
                        .limits
                        .product_execution
                        .compiler_substrate_total_bytes,
                )
        {
            return Err(BuildError::new(
                "V2_PRODUCT_EXPOSURE_INVENTORY_LIMIT",
                "Product exposure inventory exceeds its retained bound",
            ));
        }
        if exposure.kind == ProductExposureKind::CharacterDevice {
            validate_null_exposure(exposure)?;
            character_devices += 1;
        } else if !exposure.read_only
            || !exposure.nosuid
            || !exposure.nodev
            || exposure.source == Path::new("/dev/null")
            || exposure.destination == Path::new("/dev/null")
        {
            return Err(BuildError::new(
                "V2_PRODUCT_EXPOSURE_FLAGS",
                "regular exposure lacks read-only node-safe flags",
            ));
        }
    }
    if character_devices != 1 {
        return Err(BuildError::new(
            "V2_PRODUCT_NULL_EXPOSURE",
            "Product execution requires one /dev/null character device",
        ));
    }
    Ok(())
}

fn validate_product_exposure_manifest(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
) -> Result<(), BuildError> {
    let exposures = &active.execution.exposures;
    let mut accounted = BTreeSet::new();
    validate_repository_exposures(inputs, exposures, &mut accounted)?;
    validate_tool_exposures(inputs, exposures, &mut accounted)?;
    let host_target =
        validate_compiler_exposures(inputs, active, exposures, &mut accounted)?;
    require_product_exposure(
        exposures,
        Some(&host_target),
        &active.compiler_substrate.target_libdir,
        ProductExposureKind::Directory,
        false,
        "selected Rust target libdir",
        &mut accounted,
    )?;
    require_product_target_inventory(
        exposures,
        &active.compiler_substrate.target_libdir,
        &active.compiler_substrate.entries,
    )?;
    require_product_exposure(
        exposures,
        Some(Path::new("/dev/null")),
        Path::new("/dev/null"),
        ProductExposureKind::CharacterDevice,
        false,
        "null standard-input device",
        &mut accounted,
    )?;
    require_product_null_inventory(exposures)?;
    account_product_runtime_exposures(exposures, &mut accounted)?;
    if accounted.len() == exposures.len() {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_EXPOSURE_MANIFEST",
        "Product exposure manifest has a missing or orphan entry",
    ))
}

fn validate_repository_exposures(
    inputs: &BuildInputs<'_>,
    exposures: &[ProductExposureEvidence],
    accounted: &mut BTreeSet<PathBuf>,
) -> Result<(), BuildError> {
    for relative in product_preflight::repository_inputs() {
        let source = inputs.repository_root.join(relative);
        let destination = Path::new("/repo").join(relative);
        require_product_exposure(
            exposures,
            Some(&source),
            &destination,
            ProductExposureKind::File,
            false,
            "Product repository input",
            accounted,
        )?;
        let retained = inputs
            .inventory
            .files
            .iter()
            .find(|value| value.path == Path::new(relative))
            .ok_or_else(|| {
                BuildError::new(
                    "V2_PRODUCT_EXPOSURE_SOURCE",
                    "repository exposure lacks source authority",
                )
            })?;
        let observed = inputs
            .source_observations
            .iter()
            .find(|value| value.path == Path::new(relative))
            .ok_or_else(|| {
                BuildError::new(
                    "V2_PRODUCT_EXPOSURE_SOURCE",
                    "repository exposure lacks source observation",
                )
            })?;
        require_product_file_inventory(
            exposures,
            &destination,
            &retained.content_digest,
            observed.size_bytes,
        )?;
    }
    Ok(())
}

fn validate_tool_exposures(
    inputs: &BuildInputs<'_>,
    exposures: &[ProductExposureEvidence],
    accounted: &mut BTreeSet<PathBuf>,
) -> Result<(), BuildError> {
    for (tool, destination, purpose) in [
        (inputs.cargo, Path::new(PREPARED_CARGO), "Cargo executable"),
        (inputs.rustc, Path::new(PREPARED_RUSTC), "rustc executable"),
        (inputs.checker, Path::new(PREPARED_CHECKER), "checker"),
    ] {
        require_product_exposure(
            exposures,
            Some(&tool.canonical_path),
            destination,
            ProductExposureKind::File,
            true,
            purpose,
            accounted,
        )?;
        require_product_file_inventory(
            exposures,
            destination,
            &tool.executable_digest,
            tool.executable_size_bytes,
        )?;
    }
    Ok(())
}

fn validate_compiler_exposures(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
    exposures: &[ProductExposureEvidence],
    accounted: &mut BTreeSet<PathBuf>,
) -> Result<PathBuf, BuildError> {
    let (host_support, host_target) = compiler_host_roots(
        &inputs.cargo.canonical_path,
        &inputs.rustc.canonical_path,
    )?;
    for entry in &active.compiler_substrate.entries {
        if entry.kind != compiler_substrate::EntryKind::CompilerSupport {
            continue;
        }
        let relative = entry
            .canonical_path
            .strip_prefix(PREPARED_COMPILER_SUPPORT)
            .map_err(|failure| {
                BuildError::new(
                    "V2_PRODUCT_SUBSTRATE_SOURCE",
                    failure.to_string(),
                )
            })?;
        let source = host_support.join(relative);
        require_product_exposure(
            exposures,
            Some(&source),
            &entry.canonical_path,
            ProductExposureKind::File,
            true,
            "compiler support",
            accounted,
        )?;
        require_product_file_inventory(
            exposures,
            &entry.canonical_path,
            &entry.content_digest,
            entry.size_bytes,
        )?;
    }
    Ok(host_target)
}

fn account_product_runtime_exposures(
    exposures: &[ProductExposureEvidence],
    accounted: &mut BTreeSet<PathBuf>,
) -> Result<(), BuildError> {
    for exposure in exposures {
        if exposure.purpose != "system runtime" {
            continue;
        }
        if exposure.kind != ProductExposureKind::File
            || exposure.noexec
            || !accounted.insert(exposure.destination.clone())
        {
            return Err(BuildError::new(
                "V2_PRODUCT_EXPOSURE_MANIFEST",
                "system-runtime exposure has the wrong shape",
            ));
        }
    }
    Ok(())
}

fn compiler_host_roots(
    cargo: &Path,
    rustc: &Path,
) -> Result<(PathBuf, PathBuf), BuildError> {
    let bin = rustc.parent().ok_or_else(|| {
        BuildError::new(
            "V2_PRODUCT_RUSTC_LAYOUT",
            "rustc canonical path has no bin parent",
        )
    })?;
    let sysroot = bin.parent().ok_or_else(|| {
        BuildError::new(
            "V2_PRODUCT_RUSTC_LAYOUT",
            "rustc canonical path has no sysroot parent",
        )
    })?;
    if rustc.file_name().and_then(|value| value.to_str()) != Some("rustc")
        || bin.file_name().and_then(|value| value.to_str()) != Some("bin")
        || cargo.file_name().and_then(|value| value.to_str()) != Some("cargo")
        || cargo.parent() != Some(bin)
    {
        return Err(BuildError::new(
            "V2_PRODUCT_RUSTC_LAYOUT",
            "Cargo and rustc do not share one sysroot/bin layout",
        ));
    }
    let support = sysroot.join("lib");
    let target =
        support.join("rustlib").join(PRODUCT_TARGET_TRIPLE).join("lib");
    Ok((support, target))
}

#[allow(clippy::too_many_arguments)]
fn require_product_exposure(
    exposures: &[ProductExposureEvidence],
    source: Option<&Path>,
    destination: &Path,
    kind: ProductExposureKind,
    executable: bool,
    purpose: &str,
    accounted: &mut BTreeSet<PathBuf>,
) -> Result<(), BuildError> {
    let Some(exposure) =
        exposures.iter().find(|value| value.destination == destination)
    else {
        return Err(BuildError::new(
            "V2_PRODUCT_EXPOSURE_MANIFEST",
            format!("required exposure {} is absent", destination.display()),
        ));
    };
    let source_matches =
        source.is_none_or(|expected| exposure.source == expected);
    if source_matches
        && exposure.kind == kind
        && exposure.noexec != executable
        && exposure.purpose == purpose
        && accounted.insert(exposure.destination.clone())
    {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_EXPOSURE_MANIFEST",
        format!(
            "exposure {} differs from its authority",
            destination.display()
        ),
    ))
}

fn product_exposure_by_destination<'a>(
    exposures: &'a [ProductExposureEvidence],
    destination: &Path,
) -> Result<&'a ProductExposureEvidence, BuildError> {
    exposures.iter().find(|value| value.destination == destination).ok_or_else(
        || {
            BuildError::new(
                "V2_PRODUCT_EXPOSURE_MANIFEST",
                format!(
                    "required exposure {} is absent",
                    destination.display()
                ),
            )
        },
    )
}

fn require_product_file_inventory(
    exposures: &[ProductExposureEvidence],
    destination: &Path,
    content_digest: &str,
    size_bytes: u64,
) -> Result<(), BuildError> {
    let exposure = product_exposure_by_destination(exposures, destination)?;
    let [entry] = exposure.inventory_entries.as_slice() else {
        return Err(BuildError::new(
            "V2_PRODUCT_EXPOSURE_INVENTORY",
            "file exposure inventory does not contain one root entry",
        ));
    };
    if entry.relative_path == Path::new(".")
        && entry.kind == prepared_root::InventoryEntryKind::File
        && entry.content_digest.as_deref() == Some(content_digest)
        && entry.size_bytes == size_bytes
        && exposure.inventory_total_bytes == size_bytes
    {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_EXPOSURE_INVENTORY",
        "file exposure inventory differs from retained byte identity",
    ))
}

fn require_product_target_inventory(
    exposures: &[ProductExposureEvidence],
    destination: &Path,
    substrate: &[compiler_substrate::Entry],
) -> Result<(), BuildError> {
    let exposure = product_exposure_by_destination(exposures, destination)?;
    let root = exposure.inventory_entries.first().ok_or_else(|| {
        BuildError::new(
            "V2_PRODUCT_EXPOSURE_INVENTORY",
            "target-libdir inventory has no root entry",
        )
    })?;
    if root.relative_path != Path::new(".")
        || root.kind != prepared_root::InventoryEntryKind::Directory
    {
        return Err(BuildError::new(
            "V2_PRODUCT_EXPOSURE_INVENTORY",
            "target-libdir inventory root is not a directory",
        ));
    }
    let expected = substrate
        .iter()
        .filter(|entry| {
            entry.kind == compiler_substrate::EntryKind::TargetLibdir
        })
        .map(|entry| {
            let relative = entry
                .canonical_path
                .strip_prefix(destination)
                .map_err(|failure| {
                    BuildError::new(
                        "V2_PRODUCT_EXPOSURE_INVENTORY",
                        failure.to_string(),
                    )
                })?;
            Ok((
                relative.to_path_buf(),
                (entry.content_digest.clone(), entry.size_bytes),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, BuildError>>()?;
    let observed = exposure
        .inventory_entries
        .iter()
        .skip(1)
        .map(|entry| {
            if entry.kind != prepared_root::InventoryEntryKind::File {
                return Err(BuildError::new(
                    "V2_PRODUCT_EXPOSURE_INVENTORY",
                    "target-libdir contains a non-file child",
                ));
            }
            let digest = entry.content_digest.clone().ok_or_else(|| {
                BuildError::new(
                    "V2_PRODUCT_EXPOSURE_INVENTORY",
                    "target-libdir file lacks content identity",
                )
            })?;
            Ok((entry.relative_path.clone(), (digest, entry.size_bytes)))
        })
        .collect::<Result<BTreeMap<_, _>, BuildError>>()?;
    if !expected.is_empty() && observed == expected {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_EXPOSURE_INVENTORY",
        "target-libdir inventory differs from compiler substrate bytes",
    ))
}

fn require_product_null_inventory(
    exposures: &[ProductExposureEvidence],
) -> Result<(), BuildError> {
    let exposure =
        product_exposure_by_destination(exposures, Path::new("/dev/null"))?;
    let [entry] = exposure.inventory_entries.as_slice() else {
        return Err(BuildError::new(
            "V2_PRODUCT_EXPOSURE_INVENTORY",
            "null-device inventory does not contain one root entry",
        ));
    };
    if entry.relative_path == Path::new(".")
        && entry.kind == prepared_root::InventoryEntryKind::CharacterDevice
        && entry.device_identity == Some(259)
        && exposure.inventory_total_bytes == 0
    {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_EXPOSURE_INVENTORY",
        "null-device inventory differs from Linux character device 1:3",
    ))
}

fn validate_null_exposure(
    exposure: &ProductExposureEvidence,
) -> Result<(), BuildError> {
    if exposure.source == Path::new("/dev/null")
        && exposure.destination == Path::new("/dev/null")
        && exposure.read_only
        && exposure.nosuid
        && !exposure.nodev
        && exposure.noexec
    {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_NULL_EXPOSURE",
        "/dev/null character-device flags or paths differ",
    ))
}

fn validate_product_namespace(
    namespace: &ProductNamespaceEvidence,
) -> Result<(), BuildError> {
    if namespace.user
        && namespace.mount
        && namespace.network
        && namespace.pid
        && namespace.setgroups_denied
        && namespace.private_propagation
        && namespace.pid_one_reaper
        && namespace.parent_death_signal
        && valid_single_id_map(&namespace.uid_map)
        && valid_single_id_map(&namespace.gid_map)
    {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_NAMESPACE",
        "Product namespace evidence is incomplete",
    ))
}

fn valid_single_id_map(value: &str) -> bool {
    let values = value
        .split_ascii_whitespace()
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>();
    values.is_ok_and(|values| {
        values.len() == 3 && values[0] == 0 && values[2] == 1
    })
}

fn validate_product_network(
    network: &ProductNetworkEvidence,
) -> Result<(), BuildError> {
    let interfaces =
        network.interfaces.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if interfaces == BTreeSet::from(["lo"])
        && network.interfaces.len() == 1
        && network.routes.is_empty()
        && network.kernel_route_count == 2
        && network.socket_count == 0
        && network.policy_observation_digest
            == digest::bytes(NETWORK_POLICY_DOMAIN)
        && network.policy_basis == NETWORK_POLICY_BASIS
        && network.external_carrier_absent
    {
        return Ok(());
    }
    Err(BuildError::new(
        "V2_PRODUCT_NETWORK",
        "Product network evidence retains external authority",
    ))
}

fn validate_product_scratch(
    inputs: &BuildInputs<'_>,
    scratch: &ProductScratchEvidence,
) -> Result<(), BuildError> {
    let limits = inputs.topology.limits.product_execution;
    validate_product_path(inputs, &scratch.mountpoint, "scratch")?;
    if !scratch.mountpoint.is_absolute()
        || scratch.size_limit_bytes != u64::from(limits.scratch_bytes)
        || scratch.inode_limit != u64::from(limits.scratch_inodes)
        || scratch.allocated_bytes > scratch.size_limit_bytes
        || scratch.inode_count > scratch.inode_limit
        || scratch.logical_bytes > u64::from(limits.process_output_file_bytes)
        || !scratch.owner_only
        || !scratch.nodev
        || !scratch.noexec
        || !scratch.nosuid
    {
        return Err(BuildError::new(
            "V2_PRODUCT_SCRATCH",
            "Product scratch evidence differs from committed hard bounds",
        ));
    }
    Ok(())
}

const fn complete_product_cleanup(cleanup: &ProductCleanupEvidence) -> bool {
    cleanup.processes_absent
        && cleanup.readers_joined
        && cleanup.namespace_absent
        && cleanup.mount_absent
        && cleanup.mountpoint_identity_preserved
        && cleanup.underlying_directory_empty
        && cleanup.source_identity_preserved
        && cleanup.tool_identity_preserved
}

fn product_process_value(
    inputs: &BuildInputs<'_>,
    active: &ActiveProductEvidence<'_>,
) -> Result<Value, BuildError> {
    let argv = process_argv(active.process, "Product proof process")?;
    let checker =
        path_text(&inputs.checker.canonical_path, "Product proof checker")?;
    let limits = inputs.topology.limits.process();
    let stdout_len =
        u64::try_from(active.process.stdout.len()).map_err(|_| {
            BuildError::new(
                "V2_PRODUCT_PROCESS_LIMIT",
                "Product stdout length does not fit u64",
            )
        })?;
    let stderr_len =
        u64::try_from(active.process.stderr.len()).map_err(|_| {
            BuildError::new(
                "V2_PRODUCT_PROCESS_LIMIT",
                "Product stderr length does not fit u64",
            )
        })?;
    if argv.iter().map(String::as_str).ne([checker, "product-proof-child"])
        || active.process.status.code() != Some(0)
        || active.process.termination_reason
            != crate::process::TerminationReason::Exited
        || active.process.stderr_bytes != 0
        || active.process.stdout_bytes != stdout_len
        || active.process.stderr_bytes != stderr_len
        || active.process.stdout_bytes > limits.stdout_bytes as u64
        || active.process.stderr_bytes > limits.stderr_bytes as u64
        || active.process.stdout_overflow
        || active.process.stderr_overflow
        || active.process.limit_observed_after_exit
    {
        return Err(BuildError::new(
            "V2_PRODUCT_PROCESS",
            "Product proof process is not the exact successful checker child",
        ));
    }
    process_value(
        &ProcessDescription {
            purpose: "product_proof_execution",
            workspace: Some(PRODUCT_WORKSPACE),
            profile: Some(PRODUCT_PROFILE),
            closure: None,
            execution: Some(&active.execution.id),
            tool_review: None,
            tool_environment: None,
            executable_digest: &inputs.checker.executable_digest,
        },
        active.process,
        &limits,
    )
}

fn validate_workspace_scope<'a>(
    inputs: &'a BuildInputs<'_>,
) -> Result<(&'a Workspace, Vec<&'a Workspace>), BuildError> {
    let roots = inputs
        .topology
        .workspaces
        .values()
        .filter(|workspace| workspace.root.as_os_str().is_empty())
        .collect::<Vec<_>>();
    let [root] = roots.as_slice() else {
        return Err(BuildError::new(
            "V2_WORKSPACE_SCOPE",
            "topology must contain exactly one repository-root workspace",
        ));
    };
    if root.partition != WorkspacePartition::RepositoryTooling {
        return Err(BuildError::new(
            "V2_WORKSPACE_SCOPE",
            "repository-root workspace is not RepositoryTooling",
        ));
    }
    let workspaces = inputs.topology.workspaces.values().collect::<Vec<_>>();
    for workspace in &workspaces {
        if workspace.id != root.id
            && workspace.partition != WorkspacePartition::Product
        {
            return Err(BuildError::new(
                "V2_WORKSPACE_PARTITION",
                "a non-root workspace is not Product-owned",
            ));
        }
    }
    Ok((root, workspaces))
}

fn validate_product_presence(
    registry: &product_closures::Registry,
    active: bool,
) -> Result<(), BuildError> {
    let empty = registry.closures.is_empty()
        && registry.public_consumer_relations.is_empty();
    let complete = !registry.closures.is_empty()
        && !registry.public_consumer_relations.is_empty();
    if !empty && !complete {
        return Err(BuildError::new(
            "V2_PRODUCT_REGISTRY_SHAPE",
            "Product closure and relation registries are only partly active",
        ));
    }
    match (empty, active) {
        (true, false) | (false, true) => Ok(()),
        (true, true) => Err(BuildError::new(
            "V2_PRODUCT_EVIDENCE_UNDECLARED",
            "active Product evidence has no declared registry authority",
        )),
        (false, false) => Err(BuildError::new(
            "V2_PRODUCT_EVIDENCE_MISSING",
            "active Product registry has no complete execution evidence",
        )),
    }
}

fn validate_tool_capture(
    identity: &ToolIdentity,
    capture: &VersionCapture,
    label: &str,
) -> Result<(), BuildError> {
    if identity.version != capture.version
        || identity.version_digest != capture.version_digest
    {
        return Err(BuildError::new(
            "V2_TOOL_VERSION_MISMATCH",
            format!(
                "{label} identity disagrees with its bounded version capture"
            ),
        ));
    }
    let argv = process_argv(&capture.process, &format!("{label} version"))?;
    let executable =
        path_text(&identity.canonical_path, &format!("{label} executable"))?;
    if argv.first().map(String::as_str) != Some(executable)
        || argv.get(1..) != Some(identity.version_argv.as_slice())
    {
        return Err(BuildError::new(
            "V2_TOOL_VERSION_ARGV",
            format!("{label} version capture argv differs from its identity"),
        ));
    }
    Ok(())
}

fn validate_metadata_completeness<'a>(
    inputs: &'a BuildInputs<'a>,
    workspaces: &[&Workspace],
) -> Result<Vec<&'a MetadataEvidence<'a>>, BuildError> {
    let expected = workspaces
        .iter()
        .flat_map(|workspace| {
            workspace
                .metadata_profiles
                .keys()
                .map(|profile| (workspace.id.clone(), profile.clone()))
        })
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeMap::new();
    for evidence in inputs.metadata {
        let workspace =
            inputs.topology.workspaces.get(evidence.workspace).ok_or_else(
                || {
                    BuildError::new(
                        "V2_METADATA_WORKSPACE",
                        format!("unknown workspace {}", evidence.workspace),
                    )
                },
            )?;
        let expected_root = inputs.repository_root.join(&workspace.root);
        if evidence.initial.metadata.workspace_root != expected_root {
            return Err(BuildError::new(
                "V2_METADATA_ROOT",
                format!(
                    "profile {} metadata root {} differs from {}",
                    evidence.profile,
                    evidence.initial.metadata.workspace_root.display(),
                    expected_root.display()
                ),
            ));
        }
        let key = (evidence.workspace.to_owned(), evidence.profile.to_owned());
        if actual.insert(key, evidence).is_some() {
            return Err(BuildError::new(
                "V2_METADATA_DUPLICATE_PROFILE",
                format!(
                    "duplicate metadata profile {}/{}",
                    evidence.workspace, evidence.profile
                ),
            ));
        }
        validate_graph_profile(evidence)?;
        validate_final_capture(inputs.mode, evidence)?;
    }
    if actual.keys().cloned().collect::<BTreeSet<_>>() != expected {
        return Err(BuildError::new(
            "V2_METADATA_PROFILE_SET",
            "metadata evidence does not exactly cover topology profiles",
        ));
    }
    Ok(actual.into_values().collect())
}

fn validate_graph_profile(
    evidence: &MetadataEvidence<'_>,
) -> Result<(), BuildError> {
    let wrong_package = evidence
        .graph
        .packages
        .iter()
        .any(|record| record.profile != evidence.profile);
    let wrong_edge = evidence
        .graph
        .edges
        .iter()
        .any(|record| record.profile != evidence.profile);
    let wrong_reachability = evidence
        .graph
        .dependency_reachability
        .iter()
        .any(|record| record.profile != evidence.profile);
    if wrong_package || wrong_edge || wrong_reachability {
        return Err(BuildError::new(
            "V2_GRAPH_PROFILE",
            format!(
                "graph contains a record outside profile {}",
                evidence.profile
            ),
        ));
    }
    Ok(())
}

fn validate_final_capture(
    mode: Mode<'_>,
    evidence: &MetadataEvidence<'_>,
) -> Result<(), BuildError> {
    match mode {
        Mode::Candidate if evidence.final_capture.is_some() => {
            Err(BuildError::new(
                "V2_CANDIDATE_FINAL_METADATA",
                concat!(
                    "candidate report must not claim an exact final metadata ",
                    "capture"
                ),
            ))
        }
        Mode::Exact { .. } => validate_exact_final_capture(evidence),
        Mode::Candidate => Ok(()),
    }
}

fn validate_exact_final_capture(
    evidence: &MetadataEvidence<'_>,
) -> Result<(), BuildError> {
    let final_capture = evidence.final_capture.ok_or_else(|| {
        BuildError::new(
            "V2_EXACT_FINAL_METADATA",
            format!("exact profile {} lacks final metadata", evidence.profile),
        )
    })?;
    if final_capture.raw_digest != evidence.initial.raw_digest
        || final_capture.metadata != evidence.initial.metadata
    {
        return Err(BuildError::new(
            "V2_METADATA_DRIFT",
            format!("profile {} metadata changed", evidence.profile),
        ));
    }
    Ok(())
}

fn source_observation_map<'a>(
    inputs: &'a BuildInputs<'_>,
) -> Result<BTreeMap<&'a Path, &'a SourceFileObservation>, BuildError> {
    let mut values = BTreeMap::new();
    for observation in inputs.source_observations {
        if values.insert(observation.path.as_path(), observation).is_some() {
            return Err(BuildError::new(
                "V2_SOURCE_OBSERVATION_DUPLICATE",
                format!(
                    "duplicate observation for {}",
                    observation.path.display()
                ),
            ));
        }
        if !matches!(observation.mode.as_str(), "100644" | "100755") {
            return Err(BuildError::new(
                "V2_SOURCE_MODE",
                format!(
                    "{} has unadmitted mode {}",
                    observation.path.display(),
                    observation.mode
                ),
            ));
        }
    }
    let expected = inputs
        .inventory
        .files
        .iter()
        .map(|file| file.path.as_path())
        .collect::<BTreeSet<_>>();
    if values.keys().copied().collect::<BTreeSet<_>>() != expected {
        return Err(BuildError::new(
            "V2_SOURCE_OBSERVATION_SET",
            "source observations do not exactly cover classified inventory",
        ));
    }
    Ok(values)
}

type ModeValues = (String, Option<String>, Value, Value, Vec<Value>);

fn mode_values(
    inputs: &BuildInputs<'_>,
    workspace: &Workspace,
) -> Result<ModeValues, BuildError> {
    match inputs.mode {
        Mode::Candidate => Ok((
            "candidate".to_owned(),
            None,
            Value::Null,
            Value::Null,
            Vec::new(),
        )),
        Mode::Exact { revision, git, execution_environment } => {
            exact_mode_values(
                inputs,
                workspace,
                revision,
                git,
                execution_environment,
            )
        }
    }
}

fn exact_mode_values(
    inputs: &BuildInputs<'_>,
    workspace: &Workspace,
    revision: &str,
    git: &git::Evidence,
    execution_environment: &str,
) -> Result<ModeValues, BuildError> {
    if git.revision != revision {
        return Err(BuildError::new(
            "V2_GIT_REVISION",
            "exact Git evidence revision differs from requested revision",
        ));
    }
    let (binding, review) = git_binding(
        inputs.execution_tools,
        execution_environment,
        &git.version,
        &git.version_digest,
    )?;
    let identity = execution_binding_value(binding)?;
    let processes = git_process_values(
        inputs,
        workspace,
        git,
        binding,
        review,
        execution_environment,
    )?;
    Ok((
        "exact".to_owned(),
        Some(revision.to_owned()),
        identity,
        Value::String(git.tree_digest.clone()),
        processes,
    ))
}

fn git_process_values(
    inputs: &BuildInputs<'_>,
    workspace: &Workspace,
    git: &git::Evidence,
    binding: &ExecutionBinding,
    review: &execution_tools::Review,
    execution_environment: &str,
) -> Result<Vec<Value>, BuildError> {
    let mut values = Vec::with_capacity(git.processes.len());
    for (purpose, process) in &git.processes {
        let argv = process_argv(process, purpose)?;
        let executable = path_text(&binding.canonical_path, "Git executable")?;
        if argv.first().map(String::as_str) != Some(executable) {
            return Err(BuildError::new(
                "V2_GIT_PROCESS_EXECUTABLE",
                format!("{purpose} did not invoke the reviewed Git executable"),
            ));
        }
        values.push(process_value(
            &ProcessDescription {
                purpose,
                workspace: Some(&workspace.id),
                profile: None,
                closure: None,
                execution: None,
                tool_review: Some(&review.id),
                tool_environment: Some(execution_environment),
                executable_digest: &binding.executable_digest,
            },
            process,
            &inputs.topology.limits.process(),
        )?);
    }
    Ok(values)
}

fn git_binding<'a>(
    registry: &'a execution_tools::Registry,
    environment: &str,
    version: &str,
    version_digest: &str,
) -> Result<(&'a ExecutionBinding, &'a execution_tools::Review), BuildError> {
    let environment = registry
        .environments
        .iter()
        .find(|candidate| candidate.id == environment)
        .ok_or_else(|| {
            BuildError::new(
                "V2_GIT_ENVIRONMENT",
                format!("unknown Git execution environment {environment}"),
            )
        })?;
    let required = BTreeSet::from([
        InvocationFamily::GitVersion,
        InvocationFamily::GitRevision,
        InvocationFamily::GitTree,
        InvocationFamily::GitIndex,
        InvocationFamily::GitAttributes,
    ]);
    let mut matches = environment.bindings.iter().filter_map(|binding| {
        registry
            .reviews
            .iter()
            .find(|review| review.id == binding.review)
            .filter(|review| {
                review.logical_role == LogicalRole::Git
                    && review
                        .invocation_families
                        .iter()
                        .copied()
                        .collect::<BTreeSet<_>>()
                        == required
                    && review.product_profiles.is_empty()
            })
            .map(|review| (binding, review))
    });
    let selected = matches.next().ok_or_else(|| {
        BuildError::new(
            "V2_GIT_BINDING",
            concat!(
                "environment has no RepositoryTooling-only binding for all ",
                "Git proof families"
            ),
        )
    })?;
    if matches.next().is_some() {
        return Err(BuildError::new(
            "V2_GIT_BINDING",
            "environment has more than one complete Git proof binding",
        ));
    }
    if selected.0.version_digest != version_digest
        || selected.0.version.trim_end_matches(['\r', '\n']) != version
    {
        return Err(BuildError::new(
            "V2_GIT_VERSION",
            "Git execution binding disagrees with exact Git version evidence",
        ));
    }
    Ok(selected)
}

fn classification_values(
    inputs: &BuildInputs<'_>,
    observations: &BTreeMap<&Path, &SourceFileObservation>,
) -> Result<Vec<Value>, BuildError> {
    inputs
        .inventory
        .files
        .iter()
        .map(|file| classification_value(inputs, observations, file))
        .collect()
}

fn classification_value(
    inputs: &BuildInputs<'_>,
    observations: &BTreeMap<&Path, &SourceFileObservation>,
    file: &crate::source::ClassifiedFile,
) -> Result<Value, BuildError> {
    let observation =
        observations.get(file.path.as_path()).ok_or_else(|| {
            BuildError::new(
                "V2_SOURCE_OBSERVATION_MISSING",
                format!("missing observation for {}", file.path.display()),
            )
        })?;
    let matches = inputs
        .policy
        .rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| {
            rule.prefix == file.rule_prefix
                && rule.precedence == file.rule_precedence
                && rule.classification == file.classification
        })
        .collect::<Vec<_>>();
    let [(index, rule)] = matches.as_slice() else {
        return Err(BuildError::new(
            "V2_WINNING_RULE",
            format!(
                "{} does not resolve to one exact policy rule",
                file.path.display()
            ),
        ));
    };
    Ok(json!({
        "path": path_text(&file.path, "classified source")?,
        "content_digest": file.content_digest,
        "size_bytes": observation.size_bytes,
        "mode": observation.mode,
        "source_owner": owner_value(file.classification.owner),
        "source_provenance": file.classification.provenance.as_str(),
        "source_use": file.classification.source_use.as_str(),
        "upstream": file.upstream.as_ref().map(upstream_value).transpose()?,
        "winning_rule": {
            "index": index,
            "prefix": path_text(&rule.prefix, "winning rule prefix")?,
            "precedence": rule.precedence,
        },
    }))
}

#[derive(Default)]
struct GraphValues {
    workspace_packages: Vec<Value>,
    external_packages: Vec<Value>,
    edges: Vec<Value>,
    reachability: Vec<Value>,
}

impl GraphValues {
    fn extend(&mut self, values: Self) {
        self.workspace_packages.extend(values.workspace_packages);
        self.external_packages.extend(values.external_packages);
        self.edges.extend(values.edges);
        self.reachability.extend(values.reachability);
    }
}

fn graph_values(
    root: &Path,
    workspace: &Workspace,
    evidence: &MetadataEvidence<'_>,
) -> Result<GraphValues, BuildError> {
    let workspace_packages =
        workspace_package_values(root, workspace, evidence)?;
    let external_packages = external_package_values(workspace, evidence)?;
    let edges = edge_values(root, workspace, evidence)?;
    let reachability = reachability_values(workspace, evidence)?;
    Ok(GraphValues {
        workspace_packages,
        external_packages,
        edges,
        reachability,
    })
}

fn workspace_package_values(
    root: &Path,
    workspace: &Workspace,
    evidence: &MetadataEvidence<'_>,
) -> Result<Vec<Value>, BuildError> {
    let metadata = &evidence.initial.metadata;
    let mut workspace_packages = Vec::new();
    let mut resolved_workspace_ids = BTreeSet::new();
    for cargo_id in &metadata.workspace_members {
        let (value, package_id) =
            workspace_package_value(root, workspace, evidence, cargo_id)?;
        if !resolved_workspace_ids.insert(package_id.clone()) {
            return Err(BuildError::new(
                "V2_METADATA_PACKAGE_DUPLICATE",
                format!(
                    "topology package {package_id} resolves more than once"
                ),
            ));
        }
        workspace_packages.push(value);
    }
    if resolved_workspace_ids
        != workspace.packages.keys().cloned().collect::<BTreeSet<_>>()
    {
        return Err(BuildError::new(
            "V2_METADATA_PACKAGE_SET",
            format!(
                "profile {} does not exactly cover topology packages",
                evidence.profile
            ),
        ));
    }

    Ok(workspace_packages)
}

fn workspace_package_value(
    root: &Path,
    workspace: &Workspace,
    evidence: &MetadataEvidence<'_>,
    cargo_id: &str,
) -> Result<(Value, String), BuildError> {
    let package =
        evidence.initial.metadata.packages.get(cargo_id).ok_or_else(|| {
            BuildError::new(
                "V2_METADATA_MEMBER_UNRESOLVED",
                format!("workspace member {cargo_id} has no package"),
            )
        })?;
    let (resolved_workspace, expected) =
        resolve_workspace_package(root, package, workspace)?;
    if resolved_workspace.id != workspace.id {
        return Err(BuildError::new(
            "V2_METADATA_WORKSPACE_CROSSING",
            format!(
                "package {} resolved outside active workspace",
                package.name
            ),
        ));
    }
    let manifest =
        relative_repo_path(root, &package.manifest_path, "workspace manifest")?;
    let graph_package = evidence
        .graph
        .packages
        .iter()
        .find(|record| {
            record.profile == evidence.profile
                && record.name == package.name
                && record.manifest == manifest
        })
        .ok_or_else(|| {
            BuildError::new(
                "V2_GRAPH_PACKAGE_UNRESOLVED",
                format!("{} is absent from graph evidence", package.name),
            )
        })?;
    let targets = package_target_values(
        root,
        package,
        expected,
        evidence.graph,
        evidence.profile,
    )?;
    let owners = graph_package
        .owners
        .iter()
        .copied()
        .map(owner_value)
        .collect::<Vec<_>>();
    let value = json!({
        "workspace": workspace.id,
        "profile": evidence.profile,
        "id": expected.id,
        "name": package.name,
        "version": package.version,
        "manifest": path_text(&manifest, "workspace package manifest")?,
        "owners": owners,
        "targets": targets,
    });
    Ok((value, expected.id.clone()))
}

fn external_package_values(
    workspace: &Workspace,
    evidence: &MetadataEvidence<'_>,
) -> Result<Vec<Value>, BuildError> {
    let mut values = Vec::new();
    for package in evidence
        .initial
        .metadata
        .packages
        .values()
        .filter(|package| package.source.is_some())
    {
        let checksum =
            package.checksum.as_deref().map(checksum_digest).transpose()?;
        values.push(json!({
            "workspace": workspace.id,
            "profile": evidence.profile,
            "name": package.name,
            "version": package.version,
            "source": package.source,
            "checksum": checksum,
        }));
    }
    Ok(values)
}

fn edge_values(
    root: &Path,
    workspace: &Workspace,
    evidence: &MetadataEvidence<'_>,
) -> Result<Vec<Value>, BuildError> {
    let metadata = &evidence.initial.metadata;
    let mut edges = Vec::new();
    for node in metadata.nodes.values() {
        let consumer = metadata.packages.get(&node.id).ok_or_else(|| {
            BuildError::new(
                "V2_METADATA_NODE_UNRESOLVED",
                format!("resolve node {} has no package", node.id),
            )
        })?;
        for dependency in &node.dependencies {
            edges.extend(dependency_edge_values(
                root, workspace, evidence, consumer, dependency,
            )?);
        }
    }
    Ok(edges)
}

fn dependency_edge_values(
    root: &Path,
    workspace: &Workspace,
    evidence: &MetadataEvidence<'_>,
    consumer: &CargoPackage,
    dependency: &crate::cargo::ResolvedDependency,
) -> Result<Vec<Value>, BuildError> {
    let provider = evidence
        .initial
        .metadata
        .packages
        .get(&dependency.package)
        .ok_or_else(|| {
            BuildError::new(
                "V2_METADATA_EDGE_UNRESOLVED",
                format!(
                    "{} dependency alias {} has no provider {}",
                    consumer.name, dependency.name, dependency.package
                ),
            )
        })?;
    if dependency.kinds.is_empty() {
        return Err(BuildError::new(
            "V2_METADATA_EDGE_KIND",
            format!(
                "{} -> {} has no dependency kind",
                consumer.name, provider.name
            ),
        ));
    }
    let consumer_endpoint = package_endpoint(root, workspace, consumer)?;
    let provider_endpoint = package_endpoint(root, workspace, provider)?;
    let mut values = Vec::new();
    for kind in &dependency.kinds {
        let (consumer_owners, provider_owners, decision) =
            resolved_edge_authority(
                evidence,
                consumer,
                provider,
                &kind.kind,
                kind.target.as_deref(),
            )?;
        let consumer_owners =
            consumer_owners.into_iter().map(owner_value).collect::<Vec<_>>();
        let provider_owners =
            provider_owners.into_iter().map(owner_value).collect::<Vec<_>>();
        values.push(json!({
            "workspace": workspace.id,
            "profile": evidence.profile,
            "consumer": consumer_endpoint,
            "provider": provider_endpoint,
            "dependency_name": dependency.name,
            "kind": kind.kind,
            "target": kind.target,
            "consumer_owners": consumer_owners,
            "provider_owners": provider_owners,
            "decision": decision,
        }));
    }
    Ok(values)
}

fn reachability_values(
    workspace: &Workspace,
    evidence: &MetadataEvidence<'_>,
) -> Result<Vec<Value>, BuildError> {
    let package_ids_by_name = workspace
        .packages
        .values()
        .map(|package| (package.name.as_str(), package.id.as_str()))
        .collect::<BTreeMap<_, _>>();
    evidence
        .graph
        .dependency_reachability
        .iter()
        .map(|record| {
            let package = package_ids_by_name
                .get(record.package.as_str())
                .ok_or_else(|| {
                    BuildError::new(
                        "V2_REACHABILITY_PACKAGE",
                        format!("unknown reachability root {}", record.package),
                    )
                })?;
            Ok(json!({
                "workspace": workspace.id,
                "profile": evidence.profile,
                "package": package,
                "members": record.members,
            }))
        })
        .collect()
}

fn resolved_edge_authority<'a>(
    evidence: &'a MetadataEvidence<'_>,
    consumer: &CargoPackage,
    provider: &CargoPackage,
    kind: &str,
    target: Option<&str>,
) -> Result<(Vec<SourceOwner>, Vec<SourceOwner>, &'a str), BuildError> {
    if consumer.source.is_some() || provider.source.is_some() {
        return Ok((
            vec![SourceOwner::RepositoryTooling],
            vec![SourceOwner::RepositoryTooling],
            "admitted_same_owner",
        ));
    }
    let record = evidence
        .graph
        .edges
        .iter()
        .find(|record| {
            let endpoints_match = record.consumer == consumer.name
                && record.provider == provider.name;
            record.profile == evidence.profile
                && endpoints_match
                && record.kind == kind
                && record.target.as_deref() == target
        })
        .ok_or_else(|| {
            BuildError::new(
                "V2_GRAPH_EDGE_UNRESOLVED",
                format!(
                    "{} -> {} alias edge is absent from graph evidence",
                    consumer.name, provider.name
                ),
            )
        })?;
    Ok((
        record.consumer_owners.clone(),
        record.provider_owners.clone(),
        record.decision,
    ))
}

fn resolve_workspace_package<'a>(
    root: &Path,
    package: &CargoPackage,
    active: &'a Workspace,
) -> Result<(&'a Workspace, &'a WorkspacePackage), BuildError> {
    if package.source.is_some() {
        return Err(BuildError::new(
            "V2_TOPOLOGY_PACKAGE_EXTERNAL",
            format!(
                "external package {} cannot resolve as workspace source",
                package.name
            ),
        ));
    }
    let manifest = relative_repo_path(
        root,
        &package.manifest_path,
        "Cargo package manifest",
    )?;
    active
        .packages
        .values()
        .find(|expected| {
            expected.name == package.name
                && expected.version == package.version
                && expected.manifest == manifest
        })
        .map(|package| (active, package))
        .ok_or_else(|| {
            BuildError::new(
                "V2_TOPOLOGY_PACKAGE_UNRESOLVED",
                format!(
                    "{} {} at {} does not resolve to the active topology",
                    package.name,
                    package.version,
                    manifest.display()
                ),
            )
        })
}

fn package_target_values(
    root: &Path,
    package: &CargoPackage,
    expected: &WorkspacePackage,
    graph: &GraphReport,
    profile: &str,
) -> Result<Vec<Value>, BuildError> {
    let mut resolved = BTreeSet::new();
    let mut emitted = BTreeSet::new();
    let mut values = Vec::new();
    for target in &package.targets {
        let src_path =
            relative_repo_path(root, &target.src_path, "Cargo target source")?;
        let expected_target = expected.targets.values().find(|candidate| {
            candidate.name == target.name
                && candidate.kind == target.kind
                && candidate.crate_types == target.crate_types
                && candidate.src_path == src_path
        });
        let target_id = resolved_target_id(
            package,
            target,
            &src_path,
            expected_target,
            graph,
            profile,
            &mut resolved,
        )?;
        if !emitted.insert(target_id.clone()) {
            return Err(BuildError::new(
                "V2_TOPOLOGY_TARGET_DUPLICATE",
                format!(
                    "{} target identity {target_id} resolves twice",
                    package.name
                ),
            ));
        }
        values.push(json!({
            "id": target_id,
            "name": target.name,
            "kind": target.kind,
            "crate_types": target.crate_types,
            "src_path": path_text(&src_path, "Cargo target source")?,
        }));
    }
    for missing in expected
        .targets
        .values()
        .filter(|target| !resolved.contains(&target.id))
    {
        require_graph_finding(
            graph,
            profile,
            "TOPOLOGY_TARGET_MISSING",
            &missing.src_path,
        )?;
    }
    Ok(values)
}

fn resolved_target_id(
    package: &CargoPackage,
    target: &crate::cargo::Target,
    src_path: &Path,
    expected: Option<&crate::topology::WorkspaceTarget>,
    graph: &GraphReport,
    profile: &str,
    resolved: &mut BTreeSet<String>,
) -> Result<String, BuildError> {
    let Some(expected) = expected else {
        require_graph_finding(
            graph,
            profile,
            "TOPOLOGY_TARGET_EXTRA",
            src_path,
        )?;
        return diagnostic_target_id(package, target, src_path);
    };
    if !resolved.insert(expected.id.clone()) {
        return Err(BuildError::new(
            "V2_TOPOLOGY_TARGET_DUPLICATE",
            format!("{} target {} resolves twice", package.name, expected.id),
        ));
    }
    Ok(expected.id.clone())
}

fn require_graph_finding(
    graph: &GraphReport,
    profile: &str,
    code: &str,
    path: &Path,
) -> Result<(), BuildError> {
    let path = path_text(path, "topology finding path")?;
    if graph.findings.iter().any(|finding| {
        finding.code == code
            && finding.path.as_deref() == Some(path)
            && finding.message.contains(&format!("profile {profile}:"))
    }) {
        Ok(())
    } else {
        Err(BuildError::new(
            "V2_TOPOLOGY_TARGET_UNRESOLVED",
            format!("target {path} has no matching {code} graph finding"),
        ))
    }
}

fn diagnostic_target_id(
    package: &CargoPackage,
    target: &crate::cargo::Target,
    src_path: &Path,
) -> Result<String, BuildError> {
    let identity = json!({
        "package_name": package.name,
        "package_version": package.version,
        "target_name": target.name,
        "kind": target.kind,
        "crate_types": target.crate_types,
        "src_path": path_text(src_path, "diagnostic target source")?,
    });
    let bytes = serde_json::to_vec(&identity).map_err(|failure| {
        BuildError::new(
            "V2_DIAGNOSTIC_TARGET_ID",
            format!("cannot serialize diagnostic target identity: {failure}"),
        )
    })?;
    Ok(format!("unregistered-target:{}", digest::bytes(&bytes)))
}

fn package_endpoint(
    root: &Path,
    workspace: &Workspace,
    package: &CargoPackage,
) -> Result<Value, BuildError> {
    if let Some(source) = &package.source {
        Ok(json!({
            "kind": "external_package",
            "name": package.name,
            "version": package.version,
            "source": source,
        }))
    } else {
        let (_, resolved) =
            resolve_workspace_package(root, package, workspace)?;
        Ok(json!({
            "kind": "workspace_package",
            "workspace": workspace.id,
            "package": resolved.id,
        }))
    }
}

fn checksum_digest(value: &str) -> Result<String, BuildError> {
    let raw = value.strip_prefix("sha256:").unwrap_or(value);
    if raw.len() == 64
        && raw
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(format!("sha256:{raw}"))
    } else {
        Err(BuildError::new(
            "V2_PACKAGE_CHECKSUM",
            "Cargo checksum is not 64 lowercase hexadecimal digits",
        ))
    }
}

fn collected_findings(inputs: &BuildInputs<'_>) -> Vec<Finding> {
    let mut findings = inputs.inventory.findings.clone();
    for evidence in inputs.metadata {
        findings.extend(evidence.graph.findings.clone());
    }
    findings.extend(inputs.additional_findings.iter().cloned());
    findings.sort();
    findings.dedup();
    findings
}

fn artifact_value(identity: ArtifactIdentity<'_>) -> Result<Value, BuildError> {
    Ok(json!({
        "path": path_text(identity.path, "artifact path")?,
        "schema": identity.schema,
        "content_digest": identity.content_digest,
        "schema_path": path_text(identity.schema_path, "artifact schema path")?,
        "schema_digest": identity.schema_digest,
    }))
}

fn tool_value(identity: &ToolIdentity) -> Result<Value, BuildError> {
    let canonical_path =
        path_text(&identity.canonical_path, "tool executable")?;
    let runtime_inputs = identity
        .runtime_inputs
        .iter()
        .map(runtime_input_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "canonical_path": canonical_path,
        "executable_digest": identity.executable_digest,
        "executable_size_bytes": identity.executable_size_bytes,
        "version_argv": identity.version_argv,
        "version": identity.version,
        "version_digest": identity.version_digest,
        "runtime_inputs": runtime_inputs,
    }))
}

fn execution_binding_value(
    binding: &ExecutionBinding,
) -> Result<Value, BuildError> {
    let canonical_path =
        path_text(&binding.canonical_path, "execution binding")?;
    let runtime_inputs = binding
        .runtime_inputs
        .iter()
        .map(execution_runtime_input_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "canonical_path": canonical_path,
        "executable_digest": binding.executable_digest,
        "executable_size_bytes": binding.executable_size_bytes,
        "version_argv": binding.version_argv,
        "version": binding.version,
        "version_digest": binding.version_digest,
        "runtime_inputs": runtime_inputs,
    }))
}

fn execution_runtime_input_value(
    input: &execution_tools::RuntimeInput,
) -> Result<Value, BuildError> {
    let canonical_path =
        path_text(&input.canonical_path, "execution runtime input")?;
    Ok(json!({
        "canonical_path": canonical_path,
        "content_digest": input.content_digest,
        "size_bytes": input.size_bytes,
    }))
}

fn runtime_input_value(
    input: &RuntimeInputIdentity,
) -> Result<Value, BuildError> {
    let canonical_path =
        path_text(&input.canonical_path, "tool runtime input")?;
    let prepared_path =
        path_text(&input.prepared_path, "prepared runtime input")?;
    Ok(json!({
        "canonical_path": canonical_path,
        "prepared_path": prepared_path,
        "content_digest": input.content_digest,
        "size_bytes": input.size_bytes,
    }))
}

fn metadata_run_value(
    workspace: &str,
    profile: &str,
    phase: &str,
    capture: &Capture,
) -> Result<Value, BuildError> {
    Ok(json!({
        "workspace": workspace,
        "profile": profile,
        "phase": phase,
        "command_argv": process_argv(&capture.process, "Cargo metadata")?,
        "stdout_digest": digest::bytes(&capture.process.stdout),
        "metadata_digest": capture.raw_digest,
    }))
}

struct ProcessDescription<'a> {
    purpose: &'a str,
    workspace: Option<&'a str>,
    profile: Option<&'a str>,
    closure: Option<&'a str>,
    execution: Option<&'a str>,
    tool_review: Option<&'a str>,
    tool_environment: Option<&'a str>,
    executable_digest: &'a str,
}

fn process_value(
    description: &ProcessDescription<'_>,
    process: &ProcessResult,
    limits: &ProcessLimits,
) -> Result<Value, BuildError> {
    let purpose = description.purpose;
    if process.stdout_read_error.is_some()
        || process.stderr_read_error.is_some()
    {
        return Err(BuildError::new(
            "V2_PROCESS_CAPTURE",
            format!("{purpose} has an unrepresentable pipe read failure"),
        ));
    }
    #[cfg(target_os = "linux")]
    if process.pgid != Some(process.leader_pid)
        || !process.group_identity_validated
        || !process.group_absent_after_reap
        || !process.descendants_owned
        || !process.descendants_reaped
        || !process.reaped
        || !process.readers_joined
    {
        return Err(BuildError::new(
            "V2_PROCESS_GROUP_EVIDENCE",
            format!(
                "{purpose} lacks complete Linux process-group ownership or \
                 cleanup proof"
            ),
        ));
    }
    let signal = process.kill_attempted.then_some("SIGKILL");
    let execution_deadline =
        duration_millis(limits.deadline, "execution deadline")?;
    let supervision_deadline =
        duration_millis(limits.supervision_deadline, "supervision deadline")?;
    Ok(json!({
        "purpose": purpose,
        "workspace": description.workspace,
        "profile": description.profile,
        "closure": description.closure,
        "execution": description.execution,
        "tool_review": description.tool_review,
        "tool_environment": description.tool_environment,
        "executable_digest": description.executable_digest,
        "argv": process_argv(process, purpose)?,
        "termination_reason": process.termination_reason.as_str(),
        "status": process.status.code(),
        "stdout": {
            "bytes": process.stdout_bytes,
            "limit": limits.stdout_bytes,
            "overflow": process.stdout_overflow,
            "digest": digest::bytes(&process.stdout),
        },
        "stderr": {
            "bytes": process.stderr_bytes,
            "limit": limits.stderr_bytes,
            "overflow": process.stderr_overflow,
            "digest": digest::bytes(&process.stderr),
        },
        "kill": {
            "attempted": process.kill_attempted,
            "signal": signal,
            "raced_with_exit": process.kill_raced_with_exit,
            "group_absent": process.group_absent_after_reap,
        },
        "reap": {
            "leader_reaped": process.reaped,
            "descendants_reaped": process.descendants_reaped,
        },
        "group": {
            // Kernel-assigned process IDs are ephemeral observations. The
            // durable proof is the checked relation and terminal absence, not
            // the numeric value that happened to realize them in this run.
            "identity_validated": process.group_identity_validated,
            "absent_after_reap": process.group_absent_after_reap,
        },
        "readers": {
            "stdout_joined": process.readers_joined,
            "stderr_joined": process.readers_joined,
        },
        "deadlines": {
            "execution_ms": execution_deadline,
            "supervision_ms": supervision_deadline,
            "observed_after_exit": process.limit_observed_after_exit,
        },
    }))
}

fn process_argv(
    process: &ProcessResult,
    context: &str,
) -> Result<Vec<String>, BuildError> {
    process
        .argv
        .iter()
        .map(|argument| {
            argument.to_str().map(str::to_owned).ok_or_else(|| {
                BuildError::new(
                    "V2_NON_UTF8_ARGV",
                    format!("{context} contains a non-UTF-8 argv element"),
                )
            })
        })
        .collect()
}

fn duration_millis(
    duration: std::time::Duration,
    context: &str,
) -> Result<u64, BuildError> {
    u64::try_from(duration.as_millis()).map_err(|failure| {
        BuildError::new("V2_DURATION_OVERFLOW", format!("{context}: {failure}"))
    })
}

fn owner_value(owner: SourceOwner) -> Value {
    match owner {
        SourceOwner::Product(role) => {
            json!({"kind": "product", "role": role.as_str()})
        }
        SourceOwner::RepositoryTooling => json!({"kind": "repository_tooling"}),
    }
}

fn upstream_value(upstream: &Upstream) -> Result<Value, BuildError> {
    let subject = match &upstream.digest_subject {
        DigestSubject::Paths(paths) => json!({
            "kind": "paths",
            "paths": path_values(paths, "upstream digest subject")?,
        }),
        DigestSubject::ClassifiedContent => {
            json!({"kind": "classified_content"})
        }
    };
    Ok(json!({
        "identity": upstream.identity,
        "digest": upstream.digest,
        "digest_subject": subject,
    }))
}

fn limits_value(limits: Limits) -> Value {
    let product = limits.product_execution;
    json!({
        "configuration": {
            "config_bytes": limits.config_bytes,
            "json_depth": limits.json_depth,
            "json_container_items": limits.json_container_items,
            "report_bytes": limits.report_bytes,
            "findings": limits.findings,
        },
        "inventory": {
            "source_entry_count": limits.source_entry_count,
            "directory_entry_count": limits.directory_entry_count,
            "source_file_count": limits.source_file_count,
            "source_total_bytes": limits.source_total_bytes,
            "source_file_bytes": limits.source_file_bytes,
            "path_bytes": limits.path_bytes,
            "syntax_item_expr_type_visits": limits.syntax_item_expr_type_visits,
        },
        "process": {
            "stdout_bytes": limits.metadata_stdout_bytes,
            "stderr_bytes": limits.child_stderr_bytes,
            "execution_deadline_ms": limits.child_deadline_ms,
            "supervision_deadline_ms": limits.child_supervision_deadline_ms,
            "tool_executable_bytes": limits.tool_executable_bytes,
        },
        "product_execution": product_limits_value(product),
    })
}

fn product_limits_value(limits: ProductExecutionLimits) -> Value {
    json!({
        "scratch_bytes": limits.scratch_bytes,
        "scratch_inodes": limits.scratch_inodes,
        "process_address_space_bytes": limits.process_address_space_bytes,
        "process_cpu_seconds": limits.process_cpu_seconds,
        "process_open_files": limits.process_open_files,
        "process_output_file_bytes": limits.process_output_file_bytes,
        "core_dump_bytes": limits.core_dump_bytes,
        "exposure_entry_count": limits.exposure_entry_count,
        "compiler_substrate_entry_count": limits.compiler_substrate_entry_count,
        "compiler_substrate_total_bytes": limits.compiler_substrate_total_bytes,
        "cargo_json_bytes": limits.cargo_json_bytes,
        "cargo_message_count": limits.cargo_message_count,
        "wrapper_trace_bytes": limits.wrapper_trace_bytes,
        "wrapper_argv_items": limits.wrapper_argv_items,
        "wrapper_argv_bytes": limits.wrapper_argv_bytes,
        "compiler_unit_count": limits.compiler_unit_count,
        "compiler_input_count": limits.compiler_input_count,
        "dep_info_file_bytes": limits.dep_info_file_bytes,
        "dep_info_total_bytes": limits.dep_info_total_bytes,
        "artifact_count": limits.artifact_count,
        "artifact_total_bytes": limits.artifact_total_bytes,
        "package_list_bytes": limits.package_list_bytes,
        "package_member_count": limits.package_member_count,
        "public_item_count": limits.public_item_count,
        "environment_dependency_count": limits.environment_dependency_count,
    })
}

fn path_values(
    paths: &[PathBuf],
    context: &str,
) -> Result<Vec<String>, BuildError> {
    paths
        .iter()
        .map(|path| path_text(path, context).map(str::to_owned))
        .collect()
}

fn path_text<'a>(path: &'a Path, context: &str) -> Result<&'a str, BuildError> {
    path.to_str().ok_or_else(|| {
        BuildError::new(
            "V2_NON_UTF8_PATH",
            format!("{context} {} is not UTF-8", path.display()),
        )
    })
}

fn relative_repo_path(
    root: &Path,
    path: &Path,
    context: &str,
) -> Result<PathBuf, BuildError> {
    let relative = if path.is_absolute() {
        path.strip_prefix(root).map_err(|_| {
            BuildError::new(
                "V2_ROOT_ESCAPE",
                format!(
                    "{context} {} escapes {}",
                    path.display(),
                    root.display()
                ),
            )
        })?
    } else {
        path
    };
    if relative.as_os_str().is_empty() {
        return Err(BuildError::new(
            "V2_EMPTY_RELATIVE_PATH",
            format!("{context} resolves to repository root"),
        ));
    }
    path_text(relative, context)?;
    Ok(relative.to_path_buf())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        ffi::OsString,
        path::{Path, PathBuf},
        time::Duration,
    };

    #[test]
    fn report_error_conversion_preserves_category_and_message() {
        use crate::report_v2::ReportErrorCode;

        let cases = [
            (ReportErrorCode::Envelope, "V2_REPORT_ENVELOPE"),
            (ReportErrorCode::Schema, "V2_REPORT_SCHEMA"),
            (ReportErrorCode::Canonical, "V2_REPORT_CANONICAL"),
            (ReportErrorCode::Identity, "V2_REPORT_IDENTITY"),
            (ReportErrorCode::Limit, "V2_REPORT_LIMIT"),
            (ReportErrorCode::Reference, "V2_REPORT_REFERENCE"),
            (ReportErrorCode::Semantic, "V2_REPORT_SEMANTIC"),
        ];
        for (category, code) in cases {
            let failure = crate::report_v2::ReportError::new(
                category,
                "exact report failed",
            );
            let converted = super::BuildError::from(failure);
            assert_eq!(converted.code, code);
            assert_eq!(converted.message, "exact report failed");
        }
    }

    #[test]
    fn cargo_checksums_are_tagged_once_and_fail_closed() {
        let raw = "a".repeat(64);
        assert_eq!(
            super::checksum_digest(&raw).unwrap(),
            format!("sha256:{raw}")
        );
        assert_eq!(
            super::checksum_digest(&format!("sha256:{raw}")).unwrap(),
            format!("sha256:{raw}")
        );
        assert_eq!(
            super::checksum_digest("ABC").unwrap_err().code,
            "V2_PACKAGE_CHECKSUM"
        );
    }

    #[test]
    fn product_presence_is_exactly_empty_or_active() {
        let empty = crate::product_closures::Registry {
            closures: Vec::new(),
            public_consumer_relations: Vec::new(),
        };
        super::validate_product_presence(&empty, false).unwrap();
        assert_eq!(
            super::validate_product_presence(&empty, true).unwrap_err().code,
            "V2_PRODUCT_EVIDENCE_UNDECLARED"
        );

        let relation = crate::product_closures::Registry {
            closures: Vec::new(),
            public_consumer_relations: vec![
                crate::product_closures::PublicConsumerRelation {
                    id: "orphan".to_owned(),
                    shipping_closure: "shipping".to_owned(),
                    consumer_closure: "consumer".to_owned(),
                    public_items: Vec::new(),
                    derive_checks: Vec::new(),
                    reexports: Vec::new(),
                },
            ],
        };
        assert_eq!(
            super::validate_product_presence(&relation, false)
                .unwrap_err()
                .code,
            "V2_PRODUCT_REGISTRY_SHAPE"
        );

        let active = active_registry_fixture();
        assert_eq!(
            super::validate_product_presence(&active, false).unwrap_err().code,
            "V2_PRODUCT_EVIDENCE_MISSING"
        );
        super::validate_product_presence(&active, true).unwrap();
    }

    #[test]
    fn active_registry_adapter_is_exact_and_ordered() {
        let registry = active_registry_fixture();
        let selected = super::active_registry(&registry).unwrap();
        assert_eq!(selected.foundation.shipping.id, super::SHIPPING_CLOSURE);
        assert_eq!(selected.foundation.consumer.id, super::CONSUMER_CLOSURE);
        assert_eq!(selected.foundation.relation.id, super::PUBLIC_RELATION);
        assert_eq!(
            selected.kernel_interface.shipping.id,
            super::KERNEL_INTERFACE_SHIPPING_CLOSURE
        );
        assert_eq!(
            selected.kernel_interface.consumer.id,
            super::KERNEL_INTERFACE_CONSUMER_CLOSURE
        );
        assert_eq!(
            selected.kernel_interface.relation.id,
            super::KERNEL_INTERFACE_PUBLIC_RELATION
        );

        let sorted = super::sorted_unique_strings(
            &["z".to_owned(), "a".to_owned()],
            "fixture",
        )
        .unwrap();
        assert_eq!(sorted, ["a", "z"]);
        assert_eq!(
            super::sorted_unique_strings(
                &["same".to_owned(), "same".to_owned()],
                "fixture",
            )
            .unwrap_err()
            .code,
            "V2_PRODUCT_DUPLICATE_IDENTITY"
        );
    }

    #[test]
    fn typed_product_observations_are_closed_and_zero_preserving() {
        let features = vec!["default".to_owned()];
        let cfgs = vec!["feature=\"default\"".to_owned()];
        assert!(super::default_feature_selection(&features, &cfgs));
        assert!(!super::default_feature_selection(&[], &cfgs));
        assert!(!super::default_feature_selection(&features, &[]));
        assert_eq!(
            super::expected_probe_argv(
                super::PREPARED_RUSTC,
                super::ProductProbeKind::Version,
            ),
            ["/compiler/bin/rustc", "-vV"]
        );
        let target = super::expected_probe_argv(
            super::PREPARED_RUSTC,
            super::ProductProbeKind::Target,
        );
        assert!(
            target.windows(2).any(|pair| {
                pair == ["--target", "x86_64-unknown-linux-gnu"]
            })
        );
        let identity = crate::prepared_root::inventory_identity(vec![
            crate::prepared_root::InventoryEntry {
                relative_path: PathBuf::from("."),
                kind: crate::prepared_root::InventoryEntryKind::CharacterDevice,
                mode: 0o20666,
                size_bytes: 0,
                content_digest: None,
                device_identity: Some(259),
            },
        ])
        .unwrap();
        let mut null = super::ProductExposureEvidence {
            source: PathBuf::from("/dev/null"),
            destination: PathBuf::from("/dev/null"),
            kind: super::ProductExposureKind::CharacterDevice,
            read_only: true,
            nosuid: true,
            nodev: false,
            noexec: true,
            purpose: "null".to_owned(),
            inventory_digest: identity.aggregate_digest,
            inventory_entry_count: identity.entry_count,
            inventory_total_bytes: identity.total_bytes,
            inventory_entries: identity.entries,
        };
        super::validate_null_exposure(&null).unwrap();
        null.nodev = true;
        assert_eq!(
            super::validate_null_exposure(&null).unwrap_err().code,
            "V2_PRODUCT_NULL_EXPOSURE"
        );
        assert_eq!(
            super::validate_product_mode(super::Mode::Candidate, true)
                .unwrap_err()
                .code,
            "PRODUCT_EXACT_PROOF_REQUIRED"
        );
        super::validate_product_mode(super::Mode::Candidate, false).unwrap();
        assert_eq!(
            super::exact_product_environment(super::Mode::Candidate)
                .unwrap_err()
                .code,
            "PRODUCT_EXACT_PROOF_REQUIRED"
        );
        assert!(super::valid_single_id_map("0 1000 1"));
        assert!(!super::valid_single_id_map("0 1000 2"));

        assert_duplicate_network_rejected();

        let entry = crate::compiler_substrate::Entry {
            kind: crate::compiler_substrate::EntryKind::CompilerSupport,
            canonical_path: PathBuf::from("/toolchain/empty"),
            content_digest: crate::digest::bytes(b"empty substrate fixture"),
            size_bytes: 0,
        };
        let retained = super::substrate_entry_value(&entry).unwrap();
        assert_eq!(retained["size_bytes"], 0);
        assert_eq!(retained["canonical_path"], "/toolchain/empty");
        let schema: serde_json::Value = serde_json::from_slice(
            crate::schema::embedded_report_schema_bytes(),
        )
        .unwrap();
        assert_eq!(
            schema.pointer(
                "/$defs/compiler_substrate_entry/properties/size_bytes/$ref"
            ),
            Some(&serde_json::json!("#/$defs/count"))
        );
    }

    #[test]
    fn plural_producer_helpers_reject_identity_drift() {
        let mut build = cargo_build_evidence();
        super::validate_cargo_message_relation(&build).unwrap();
        build.message_count += 1;
        assert_eq!(
            super::validate_cargo_message_relation(&build).unwrap_err().code,
            "V2_PRODUCT_CARGO_CAPTURE_SET"
        );

        let exact = vec![
            "--target-dir".to_owned(),
            super::PREPARED_TARGET_DIR.to_owned(),
        ];
        assert_eq!(
            super::exact_product_target_dir(&exact).unwrap(),
            super::PREPARED_TARGET_DIR
        );
        let changed =
            vec!["--target-dir".to_owned(), "/scratch/other".to_owned()];
        assert_eq!(
            super::exact_product_target_dir(&changed).unwrap_err().code,
            "V2_PRODUCT_CARGO_ARGV"
        );

        let mut artifact = super::ProductArtifactEvidence {
            unit: "unit".to_owned(),
            path: PathBuf::from("/scratch/build-target/value.rlib"),
            content_digest: crate::digest::bytes(b"artifact"),
            size_bytes: 1,
            crate_type: "rlib".to_owned(),
        };
        assert!(super::artifact_extension_matches(&artifact));
        artifact.crate_type = "rmeta".to_owned();
        assert!(!super::artifact_extension_matches(&artifact));

        assert_runtime_identity_conflict();
        assert_source_selection_substitution();
    }

    fn cargo_build_evidence() -> super::ProductCargoBuildEvidence {
        super::ProductCargoBuildEvidence {
            closure: "foundation-shipping".to_owned(),
            command_argv: Vec::new(),
            raw_output_digest: crate::digest::bytes(b"cargo"),
            stdout_bytes: 1,
            message_count: 2,
            compiled_units: vec!["unit".to_owned()],
            fresh_provider_units: Vec::new(),
            artifact_events: vec![super::ProductCargoArtifactEventEvidence {
                unit: "unit".to_owned(),
                fresh: false,
                filenames: Vec::new(),
            }],
            probes: Vec::new(),
        }
    }

    fn assert_runtime_identity_conflict() {
        let mapping =
            (PathBuf::from("/host/lib.so"), PathBuf::from("/lib/lib.so"));
        let mut identities = BTreeMap::new();
        let first = (crate::digest::bytes(b"first"), 1);
        super::retain_runtime_identity(
            &mut identities,
            mapping.clone(),
            &first,
        )
        .unwrap();
        let changed = (crate::digest::bytes(b"changed"), 1);
        assert_eq!(
            super::retain_runtime_identity(&mut identities, mapping, &changed,)
                .unwrap_err()
                .code,
            "V2_PRODUCT_RUNTIME_INPUT"
        );
    }

    fn assert_source_selection_substitution() {
        let expected = crate::product_preflight::compiler_source_selection();
        let foundation = source_observation(
            "foundation-public-api",
            expected.foundation_shipping,
            expected.foundation_consumer,
        );
        let kernel_interface = source_observation(
            "kernel-interface-public-api",
            expected.kernel_interface_shipping,
            expected.kernel_interface_consumer,
        );
        let mut observed = crate::product_source::TwoPairSourceObservation {
            foundation,
            kernel_interface,
        };
        super::validate_exact_compiler_source_sets(&observed).unwrap();
        observed.kernel_interface.consumer_sources.pop();
        observed.kernel_interface.consumer_sources.push(PathBuf::from(
            "tests/kernel-interface-public-consumer/tests/public_contract.rs",
        ));
        assert_eq!(
            super::validate_exact_compiler_source_sets(&observed)
                .unwrap_err()
                .code,
            "V2_PRODUCT_INPUT_SET"
        );
    }

    fn source_observation(
        relation: &str,
        shipping: BTreeSet<PathBuf>,
        consumer: BTreeSet<PathBuf>,
    ) -> crate::product_source::ProductSourceObservation {
        crate::product_source::ProductSourceObservation {
            relation: relation.to_owned(),
            public_items: Vec::new(),
            derive_checks: Vec::new(),
            witness_types: Vec::new(),
            shipping_sources: shipping.into_iter().collect(),
            consumer_sources: consumer.into_iter().collect(),
        }
    }

    fn assert_duplicate_network_rejected() {
        let network = super::ProductNetworkEvidence {
            interfaces: vec!["lo".to_owned(), "lo".to_owned()],
            routes: Vec::new(),
            kernel_route_count: 2,
            socket_count: 0,
            policy_observation_digest: crate::digest::bytes(
                super::NETWORK_POLICY_DOMAIN,
            ),
            policy_basis: super::NETWORK_POLICY_BASIS.to_owned(),
            external_carrier_absent: true,
        };
        assert_eq!(
            super::validate_product_network(&network).unwrap_err().code,
            "V2_PRODUCT_NETWORK"
        );
    }

    #[test]
    fn product_executable_exposure_joins_host_identity() {
        let identity = crate::prepared_root::inventory_identity(vec![
            crate::prepared_root::InventoryEntry {
                relative_path: PathBuf::from("."),
                kind: crate::prepared_root::InventoryEntryKind::File,
                mode: 0o100_755,
                size_bytes: 1,
                content_digest: Some(crate::digest::bytes(b"cargo fixture")),
                device_identity: None,
            },
        ])
        .unwrap();
        let mut tool = super::ProductExposureEvidence {
            source: PathBuf::from("/host/cargo"),
            destination: PathBuf::from(super::PREPARED_CARGO),
            kind: super::ProductExposureKind::File,
            read_only: true,
            nosuid: true,
            nodev: true,
            noexec: false,
            purpose: "Cargo executable".to_owned(),
            inventory_digest: identity.aggregate_digest,
            inventory_entry_count: identity.entry_count,
            inventory_total_bytes: identity.total_bytes,
            inventory_entries: identity.entries,
        };
        let mut accounted = BTreeSet::new();
        require_tool_exposure(&tool, &mut accounted).unwrap();
        tool.source = PathBuf::from("/host/other");
        assert_eq!(
            require_tool_exposure(&tool, &mut BTreeSet::new())
                .unwrap_err()
                .code,
            "V2_PRODUCT_EXPOSURE_MANIFEST"
        );
    }

    fn require_tool_exposure(
        tool: &super::ProductExposureEvidence,
        accounted: &mut BTreeSet<PathBuf>,
    ) -> Result<(), super::BuildError> {
        super::require_product_exposure(
            std::slice::from_ref(tool),
            Some(Path::new("/host/cargo")),
            Path::new(super::PREPARED_CARGO),
            super::ProductExposureKind::File,
            true,
            "Cargo executable",
            accounted,
        )
    }

    fn active_registry_fixture() -> crate::product_closures::Registry {
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/product-closures.json"
        ))
        .unwrap();
        crate::product_closures::Registry::parse(
            &value,
            crate::product_closures::ParseLimits {
                path_bytes: 4_096,
                closure_items: 256,
                package_members: 8_192,
                public_items: 4_096,
            },
        )
        .unwrap()
    }

    #[test]
    fn repository_relative_paths_reject_root_and_escape() {
        let root = Path::new("/repo");
        assert_eq!(
            super::relative_repo_path(root, root, "fixture").unwrap_err().code,
            "V2_EMPTY_RELATIVE_PATH"
        );
        assert_eq!(
            super::relative_repo_path(
                root,
                Path::new("/elsewhere/file"),
                "fixture"
            )
            .unwrap_err()
            .code,
            "V2_ROOT_ESCAPE"
        );
        assert_eq!(
            super::relative_repo_path(root, Path::new("/repo/file"), "fixture")
                .unwrap(),
            PathBuf::from("file")
        );
    }

    #[cfg(unix)]
    #[test]
    fn ephemeral_process_ids_are_redacted_from_deterministic_records() {
        use std::os::unix::process::ExitStatusExt as _;

        let process = |leader_pid, pgid| crate::process::ProcessResult {
            argv: vec![OsString::from("/tool"), OsString::from("--version")],
            status: std::process::ExitStatus::from_raw(0),
            stdout: b"tool 1\n".to_vec(),
            stderr: Vec::new(),
            stdout_bytes: 7,
            stderr_bytes: 0,
            stdout_overflow: false,
            stderr_overflow: false,
            stdout_read_error: None,
            stderr_read_error: None,
            termination_reason: crate::process::TerminationReason::Exited,
            kill_attempted: false,
            kill_raced_with_exit: false,
            limit_observed_after_exit: false,
            leader_pid,
            pgid: Some(pgid),
            group_identity_validated: true,
            group_absent_after_reap: true,
            descendants_owned: true,
            descendants_reaped: true,
            descendant_residue_observed: false,
            reaped: true,
            readers_joined: true,
        };
        let limits = crate::process::ProcessLimits {
            stdout_bytes: 1024,
            stderr_bytes: 1024,
            deadline: Duration::from_secs(1),
            supervision_deadline: Duration::from_secs(2),
        };
        let first_process = process(100, 100);
        let second_process = process(200, 200);
        let executable_digest = crate::digest::bytes(b"executable fixture");
        assert_ne!(first_process.pgid, second_process.pgid);

        let first = super::process_value(
            &super::ProcessDescription {
                purpose: "fixture",
                workspace: None,
                profile: None,
                closure: None,
                execution: None,
                tool_review: None,
                tool_environment: None,
                executable_digest: &executable_digest,
            },
            &first_process,
            &limits,
        )
        .unwrap();
        let second = super::process_value(
            &super::ProcessDescription {
                purpose: "fixture",
                workspace: None,
                profile: None,
                closure: None,
                execution: None,
                tool_review: None,
                tool_environment: None,
                executable_digest: &executable_digest,
            },
            &second_process,
            &limits,
        )
        .unwrap();

        assert!(first["group"].get("pgid").is_none());
        assert_eq!(first["group"]["identity_validated"], true);
        assert_eq!(first["group"]["absent_after_reap"], true);
        assert_eq!(first, second);

        assert_product_process_scope(
            &second_process,
            &limits,
            &executable_digest,
        );

        let mismatched = process(300, 301);
        assert_eq!(
            super::process_value(
                &super::ProcessDescription {
                    purpose: "fixture",
                    workspace: None,
                    profile: None,
                    closure: None,
                    execution: None,
                    tool_review: None,
                    tool_environment: None,
                    executable_digest: &executable_digest,
                },
                &mismatched,
                &limits,
            )
            .unwrap_err()
            .code,
            "V2_PROCESS_GROUP_EVIDENCE"
        );
    }

    fn assert_product_process_scope(
        process: &crate::process::ProcessResult,
        limits: &crate::process::ProcessLimits,
        executable_digest: &str,
    ) {
        let scoped = super::process_value(
            &super::ProcessDescription {
                purpose: "product_proof_execution",
                workspace: Some("product"),
                profile: Some("release"),
                closure: None,
                execution: Some("kernel-interface-two-pair-proof"),
                tool_review: None,
                tool_environment: None,
                executable_digest,
            },
            process,
            limits,
        )
        .unwrap();
        assert_eq!(scoped["execution"], "kernel-interface-two-pair-proof");
        assert!(scoped["closure"].is_null());
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_are_typed_failures() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt as _};

        let path = PathBuf::from(OsString::from_vec(vec![b'a', 0xff]));
        assert_eq!(
            super::path_text(&path, "fixture").unwrap_err().code,
            "V2_NON_UTF8_PATH"
        );
    }

    #[test]
    fn graph_known_extra_target_is_retained_with_diagnostic_identity() {
        let package = crate::cargo::Package {
            id: "cargo-package-id".to_owned(),
            name: "fixture".to_owned(),
            version: "0.1.0".to_owned(),
            edition: "2024".to_owned(),
            source: None,
            checksum: None,
            license: None,
            manifest_path: PathBuf::from("/repo/fixture/Cargo.toml"),
            targets: vec![
                crate::cargo::Target {
                    name: "fixture".to_owned(),
                    kind: vec!["lib".to_owned()],
                    crate_types: vec!["lib".to_owned()],
                    src_path: PathBuf::from("/repo/fixture/src/lib.rs"),
                },
                crate::cargo::Target {
                    name: "build-script-build".to_owned(),
                    kind: vec!["custom-build".to_owned()],
                    crate_types: vec!["bin".to_owned()],
                    src_path: PathBuf::from("/repo/fixture/build.rs"),
                },
            ],
        };
        let expected = crate::topology::WorkspacePackage {
            id: "fixture".to_owned(),
            name: "fixture".to_owned(),
            version: "0.1.0".to_owned(),
            manifest: PathBuf::from("fixture/Cargo.toml"),
            targets: BTreeMap::from([(
                "library".to_owned(),
                crate::topology::WorkspaceTarget {
                    id: "library".to_owned(),
                    name: "fixture".to_owned(),
                    kind: vec!["lib".to_owned()],
                    crate_types: vec!["lib".to_owned()],
                    src_path: PathBuf::from("fixture/src/lib.rs"),
                },
            )]),
        };
        let graph = crate::graph::GraphReport {
            packages: Vec::new(),
            edges: Vec::new(),
            dependency_reachability: Vec::new(),
            findings: vec![crate::finding::Finding::new(
                "topology",
                "TOPOLOGY_TARGET_EXTRA",
                Some("fixture/build.rs".to_owned()),
                "profile p: target build-script-build is undeclared",
            )],
        };

        let values = super::package_target_values(
            Path::new("/repo"),
            &package,
            &expected,
            &graph,
            "p",
        )
        .unwrap();
        let diagnostic_id = super::diagnostic_target_id(
            &package,
            &package.targets[1],
            Path::new("fixture/build.rs"),
        )
        .unwrap();
        let identities = values
            .iter()
            .map(|value| value["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            identities,
            BTreeSet::from(["library", diagnostic_id.as_str()])
        );

        let absent = crate::graph::GraphReport {
            packages: Vec::new(),
            edges: Vec::new(),
            dependency_reachability: Vec::new(),
            findings: Vec::new(),
        };
        assert_eq!(
            super::package_target_values(
                Path::new("/repo"),
                &package,
                &expected,
                &absent,
                "p"
            )
            .unwrap_err()
            .code,
            "V2_TOPOLOGY_TARGET_UNRESOLVED"
        );
    }
}
