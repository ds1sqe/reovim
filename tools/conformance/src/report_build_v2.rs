//! Active Product-empty `reovim-source-role-map/v2` value construction.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    path::{Path, PathBuf},
};

use reovim_depgraph::SourceOwner;
use serde_json::{Value, json};

use crate::{
    cargo::{Capture, Package as CargoPackage, VersionCapture},
    digest,
    execution_tools::{
        self, Binding as ExecutionBinding, InvocationFamily, LogicalRole,
    },
    finding::Finding,
    git,
    graph::GraphReport,
    policy::{DigestSubject, Policy, Upstream},
    process::{ProcessLimits, ProcessResult},
    product_closures, report_v2,
    source::Inventory,
    topology::{
        Limits, ProductExecutionLimits, Topology, Workspace, WorkspacePackage,
        WorkspacePartition,
    },
};

const REPORT_SCHEMA_PATH: &str = "conformance/source-role-map.schema.json";

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

/// Complete Product-empty inputs. Product compiler/gate evidence is
/// deliberately absent from this type.
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

/// Constructs and seals the active Product-empty v2 Source Role Map.
pub fn build(
    inputs: &BuildInputs<'_>,
) -> Result<report_v2::BuiltReport, BuildError> {
    let workspace = validate_product_empty_scope(inputs)?;
    validate_tool_capture(inputs.checker, inputs.checker_version, "checker")?;
    validate_tool_capture(inputs.cargo, inputs.cargo_version, "Cargo")?;
    validate_tool_capture(inputs.rustc, inputs.rustc_version, "rustc")?;
    let metadata = validate_metadata_completeness(inputs, workspace)?;
    let observations = source_observation_map(inputs)?;
    let mut report = report_value(inputs, workspace, &metadata, &observations)?;
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
    workspace: Value,
    lockfile_digest: String,
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

fn report_value(
    inputs: &BuildInputs<'_>,
    workspace: &Workspace,
    metadata: &[&MetadataEvidence<'_>],
    observations: &BTreeMap<&Path, &SourceFileObservation>,
) -> Result<Value, BuildError> {
    let (mode, revision, git_identity, git_tree_digest, git_processes) =
        mode_values(inputs, workspace)?;
    let source = source_report_values(inputs, workspace, observations)?;
    let evidence =
        evidence_report_values(inputs, workspace, metadata, git_processes)?;
    let findings = finding_report_values(inputs)?;
    let profiles = profile_identity_values(workspace, metadata)?;
    let identity_inputs = identity_inputs_value(
        inputs,
        workspace,
        &source.lockfile_digest,
        &git_identity,
        &git_tree_digest,
        &profiles,
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
            "workspaces": [source.workspace],
            "workspace_packages": evidence.graph.workspace_packages,
            "external_packages": evidence.graph.external_packages,
            "edges": evidence.graph.edges,
            "reachability": evidence.graph.reachability,
        },
        "product_proof": empty_product_proof_value(),
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
    workspace: &Workspace,
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
    let workspace_value = json!({
        "id": workspace.id,
        "partition": "repository_tooling",
        "root": ".",
        "manifest": path_text(
            &workspace.workspace_manifest,
            "workspace manifest",
        )?,
        "lockfile": path_text(&workspace.lockfile, "workspace lockfile")?,
        "members": path_values(&workspace.members, "workspace member")?,
    });
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
    Ok(SourceReportValues {
        classifications,
        exclusions,
        workspace: workspace_value,
        lockfile_digest: lockfile.content_digest.clone(),
    })
}

fn evidence_report_values(
    inputs: &BuildInputs<'_>,
    workspace: &Workspace,
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
    workspace: &Workspace,
    metadata: &[&MetadataEvidence<'_>],
) -> Result<Vec<Value>, BuildError> {
    metadata
        .iter()
        .map(|evidence| {
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
    workspace: &Workspace,
    lockfile_digest: &str,
    git_identity: &Value,
    git_tree_digest: &Value,
    profile_identities: &[Value],
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
        "lockfiles": [{
            "workspace": workspace.id,
            "path": path_text(&workspace.lockfile, "workspace lockfile")?,
            "digest": lockfile_digest,
        }],
        "source_tree_digest": inputs.inventory.tree_digest,
        "checker": tool_value(inputs.checker)?,
        "cargo": tool_value(inputs.cargo)?,
        "rustc": tool_value(inputs.rustc)?,
        "git": git_identity,
        "git_tree_digest": git_tree_digest,
        "cargo_home_config": "absent",
        "cargo_working_directory_config": "absent",
        "compiler_substrates": [],
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
        "package_memberships": [],
        "public_consumers": [],
        "executions": [],
    })
}

fn validate_product_empty_scope<'a>(
    inputs: &'a BuildInputs<'_>,
) -> Result<&'a Workspace, BuildError> {
    ensure_empty_product_registry(inputs.product_closures)?;
    if inputs.topology.workspaces.len() != 1 {
        return Err(BuildError::new(
            "V2_WORKSPACE_SCOPE",
            concat!(
                "active v2 adapter requires exactly one root ",
                "RepositoryTooling ",
                "workspace"
            ),
        ));
    }
    let workspace =
        inputs.topology.workspaces.values().next().expect("length checked");
    if workspace.partition != WorkspacePartition::RepositoryTooling
        || !workspace.root.as_os_str().is_empty()
    {
        return Err(BuildError::new(
            "V2_WORKSPACE_SCOPE",
            "active workspace is not the root RepositoryTooling workspace",
        ));
    }
    if inputs.inventory.files.iter().any(|file| {
        !matches!(file.classification.owner, SourceOwner::RepositoryTooling)
    }) {
        return Err(BuildError::new(
            "V2_PRODUCT_NOT_EMPTY",
            "Product-empty report contains Product-owned source",
        ));
    }
    Ok(workspace)
}

fn ensure_empty_product_registry(
    registry: &product_closures::Registry,
) -> Result<(), BuildError> {
    if registry.closures.is_empty()
        && registry.public_consumer_relations.is_empty()
    {
        Ok(())
    } else {
        Err(BuildError::new(
            "V2_PRODUCT_NOT_EMPTY",
            concat!(
                "active Product-empty adapter refuses Product closures or ",
                "public-consumer relations"
            ),
        ))
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
    workspace: &Workspace,
) -> Result<Vec<&'a MetadataEvidence<'a>>, BuildError> {
    let expected =
        workspace.metadata_profiles.keys().cloned().collect::<BTreeSet<_>>();
    let mut actual = BTreeMap::new();
    for evidence in inputs.metadata {
        if evidence.workspace != workspace.id {
            return Err(BuildError::new(
                "V2_METADATA_WORKSPACE",
                format!(
                    "metadata profile {} names workspace {}, expected {}",
                    evidence.profile, evidence.workspace, workspace.id
                ),
            ));
        }
        if evidence.initial.metadata.workspace_root != inputs.repository_root {
            return Err(BuildError::new(
                "V2_METADATA_ROOT",
                format!(
                    "profile {} metadata root {} differs from {}",
                    evidence.profile,
                    evidence.initial.metadata.workspace_root.display(),
                    inputs.repository_root.display()
                ),
            ));
        }
        if actual.insert(evidence.profile.to_owned(), evidence).is_some() {
            return Err(BuildError::new(
                "V2_METADATA_DUPLICATE_PROFILE",
                format!("duplicate metadata profile {}", evidence.profile),
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
    validate_product_empty_graph(evidence.graph)?;
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

fn validate_product_empty_graph(graph: &GraphReport) -> Result<(), BuildError> {
    for package in &graph.packages {
        if package.owners.is_empty()
            || package
                .owners
                .iter()
                .any(|owner| *owner != SourceOwner::RepositoryTooling)
        {
            return Err(BuildError::new(
                "V2_PRODUCT_GRAPH_OWNER",
                format!(
                    "package {} is not RepositoryTooling-only",
                    package.name
                ),
            ));
        }
    }
    for edge in &graph.edges {
        if edge.decision != "admitted_same_owner"
            || edge.consumer_owners.is_empty()
            || edge.provider_owners.is_empty()
            || edge
                .consumer_owners
                .iter()
                .chain(&edge.provider_owners)
                .any(|owner| *owner != SourceOwner::RepositoryTooling)
        {
            return Err(BuildError::new(
                "V2_PRODUCT_GRAPH_EDGE",
                format!(
                    "{} -> {} is not an all-tooling edge",
                    edge.consumer, edge.provider
                ),
            ));
        }
    }
    Ok(())
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
    Ok(json!({
        "canonical_path": canonical_path,
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
        "closure": null,
        "execution": null,
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
        collections::BTreeMap,
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
    fn product_empty_registry_rejects_even_relation_only_state() {
        let empty = crate::product_closures::Registry {
            closures: Vec::new(),
            public_consumer_relations: Vec::new(),
        };
        super::ensure_empty_product_registry(&empty).unwrap();

        let relation = crate::product_closures::Registry {
            closures: Vec::new(),
            public_consumer_relations: vec![
                crate::product_closures::PublicConsumerRelation {
                    id: "orphan".to_owned(),
                    shipping_closure: "shipping".to_owned(),
                    consumer_closure: "consumer".to_owned(),
                    public_items: Vec::new(),
                    derive_checks: Vec::new(),
                },
            ],
        };
        assert_eq!(
            super::ensure_empty_product_registry(&relation).unwrap_err().code,
            "V2_PRODUCT_NOT_EMPTY"
        );
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
        let executable_digest = format!("sha256:{}", "a".repeat(64));
        assert_ne!(first_process.pgid, second_process.pgid);

        let first = super::process_value(
            &super::ProcessDescription {
                purpose: "fixture",
                workspace: None,
                profile: None,
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

        let mismatched = process(300, 301);
        assert_eq!(
            super::process_value(
                &super::ProcessDescription {
                    purpose: "fixture",
                    workspace: None,
                    profile: None,
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
        assert_eq!(values.len(), 2);
        assert_eq!(values[0]["id"], "library");
        assert!(
            values[1]["id"]
                .as_str()
                .unwrap()
                .starts_with("unregistered-target:sha256:")
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
