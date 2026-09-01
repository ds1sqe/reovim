//! Deterministic `reovim-source-role-map/v1` report construction.

use std::{fmt, path::Path};

use reovim_depgraph::SourceOwner;
use serde_json::{Value, json};

use crate::{
    cargo::{Capture, VersionCapture},
    digest,
    finding::Finding,
    git,
    graph::GraphReport,
    policy::{DigestSubject, Upstream},
    process::{ProcessLimits, ProcessResult},
    source::{Inventory, display_relative},
    topology::{Limits, Profile},
};

pub const SCHEMA: &str = "reovim-source-role-map/v1";

#[derive(Debug, Clone)]
pub struct ToolIdentity {
    pub canonical_path: String,
    pub executable_digest: String,
    pub version: String,
    pub version_digest: String,
}

#[derive(Debug)]
pub struct BuildInputs<'a> {
    pub mode: &'static str,
    pub revision: Option<&'a str>,
    pub policy_digest: &'a str,
    pub policy_schema_digest: &'a str,
    pub topology_digest: &'a str,
    pub topology_schema_digest: &'a str,
    pub entrypoint_reviews_digest: &'a str,
    pub entrypoint_reviews_schema_digest: &'a str,
    pub dependency_reviews_digest: &'a str,
    pub dependency_reviews_schema_digest: &'a str,
    pub report_schema_digest: &'a str,
    pub lockfiles: &'a [(String, String)],
    pub checker_digest: &'a str,
    pub cargo_identity: &'a ToolIdentity,
    pub rustc_identity: &'a ToolIdentity,
    pub git_identity: Option<&'a ToolIdentity>,
    pub profiles: &'a [(Profile, Capture)],
    pub final_profiles: &'a [(Profile, Capture)],
    pub cargo_version: &'a VersionCapture,
    pub rustc_version: &'a VersionCapture,
    pub git_evidence: Option<&'a git::Evidence>,
    pub inventory: &'a Inventory,
    pub graphs: &'a [GraphReport],
    pub findings: &'a [Finding],
    pub limits: Limits,
}

#[derive(Debug)]
pub struct BuiltReport {
    pub(crate) value: Value,
    pub(crate) bytes: Vec<u8>,
    pub(crate) identity: String,
}

#[derive(Debug)]
pub struct ReportError(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedReport {
    pub identity: String,
    pub mode: String,
    pub status: String,
}

impl fmt::Display for ReportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ReportError {}

#[allow(clippy::too_many_lines)]
pub fn build(inputs: &BuildInputs<'_>) -> Result<BuiltReport, ReportError> {
    let process_limits = inputs.limits.process();
    let mut processes = vec![process_value(
        "cargo_version",
        None,
        &inputs.cargo_identity.executable_digest,
        &inputs.cargo_version.process,
        process_limits,
    )];
    processes.push(process_value(
        "rustc_version",
        None,
        &inputs.rustc_identity.executable_digest,
        &inputs.rustc_version.process,
        process_limits,
    ));
    for (profile, capture) in inputs.profiles {
        processes.push(process_value(
            "cargo_metadata",
            Some(&profile.name),
            &inputs.cargo_identity.executable_digest,
            &capture.process,
            process_limits,
        ));
    }
    for (profile, capture) in inputs.final_profiles {
        processes.push(process_value(
            "cargo_metadata_final",
            Some(&profile.name),
            &inputs.cargo_identity.executable_digest,
            &capture.process,
            process_limits,
        ));
    }
    if let (Some(identity), Some(evidence)) = (inputs.git_identity, inputs.git_evidence) {
        for (purpose, process) in &evidence.processes {
            processes.push(process_value(
                purpose,
                None,
                &identity.executable_digest,
                process,
                process_limits,
            ));
        }
    }

    let classifications = inputs
        .inventory
        .files
        .iter()
        .map(|file| {
            json!({
                "path": display_relative(&file.path),
                "content_digest": file.content_digest,
                "source_owner": owner_value(file.classification.owner),
                "source_provenance": file.classification.provenance.as_str(),
                "source_use": file.classification.source_use.as_str(),
                "upstream": file.upstream.as_ref().map(upstream_value),
            })
        })
        .collect::<Vec<_>>();
    let exclusions = inputs
        .inventory
        .exclusions
        .iter()
        .map(|exclusion| {
            json!({
                "prefix": display_relative(&exclusion.prefix),
                "kind": exclusion.kind.as_str(),
            })
        })
        .collect::<Vec<_>>();
    let mut packages = inputs
        .graphs
        .iter()
        .flat_map(|graph| &graph.packages)
        .map(|package| {
            json!({
                "profile": package.profile,
                "name": package.name,
                "manifest": display_relative(&package.manifest),
                "owners": package.owners.iter().copied().map(owner_value).collect::<Vec<_>>(),
                "targets": package.targets.iter().map(|target| json!({
                    "name": target.name,
                    "kind": target.kind,
                    "crate_types": target.crate_types,
                    "src_path": display_relative(&target.src_path),
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    packages.sort_by_key(Value::to_string);
    let mut edges = inputs
        .graphs
        .iter()
        .flat_map(|graph| &graph.edges)
        .map(|edge| {
            json!({
                "profile": edge.profile,
                "consumer": edge.consumer,
                "provider": edge.provider,
                "kind": edge.kind,
                "target": edge.target,
                "consumer_owners": edge.consumer_owners.iter().copied().map(owner_value).collect::<Vec<_>>(),
                "provider_owners": edge.provider_owners.iter().copied().map(owner_value).collect::<Vec<_>>(),
                "decision": edge.decision,
            })
        })
        .collect::<Vec<_>>();
    edges.sort_by_key(Value::to_string);
    let mut dependency_reachability = inputs
        .graphs
        .iter()
        .flat_map(|graph| &graph.dependency_reachability)
        .map(|reachability| {
            json!({
                "profile": reachability.profile,
                "package": reachability.package,
                "members": reachability.members,
            })
        })
        .collect::<Vec<_>>();
    dependency_reachability.sort_by_key(Value::to_string);
    let findings = inputs
        .findings
        .iter()
        .map(|finding| {
            json!({
                "stage": finding.stage,
                "code": finding.code,
                "path": finding.path,
                "message": finding.message,
            })
        })
        .collect::<Vec<_>>();
    let profile_identities = inputs
        .profiles
        .iter()
        .map(|(profile, capture)| {
            json!({
                "name": profile.name,
                "target": profile.target,
                "all_features": profile.all_features,
                "no_default_features": profile.no_default_features,
                "features": profile.features,
                "metadata_digest": capture.raw_digest,
            })
        })
        .collect::<Vec<_>>();
    let lockfiles = inputs
        .lockfiles
        .iter()
        .map(|(path, digest)| json!({"path": path, "digest": digest}))
        .collect::<Vec<_>>();
    let status = if findings.is_empty() { "pass" } else { "fail" };
    let mut report = json!({
        "schema": SCHEMA,
        "mode": inputs.mode,
        "revision": inputs.revision,
        "identity_inputs": {
            "policy_digest": inputs.policy_digest,
            "policy_schema_digest": inputs.policy_schema_digest,
            "topology_digest": inputs.topology_digest,
            "topology_schema_digest": inputs.topology_schema_digest,
            "entrypoint_reviews_digest": inputs.entrypoint_reviews_digest,
            "entrypoint_reviews_schema_digest": inputs.entrypoint_reviews_schema_digest,
            "dependency_reviews_digest": inputs.dependency_reviews_digest,
            "dependency_reviews_schema_digest": inputs.dependency_reviews_schema_digest,
            "report_schema_digest": inputs.report_schema_digest,
            "lockfiles": lockfiles,
            "source_tree_digest": inputs.inventory.tree_digest,
            "checker_digest": inputs.checker_digest,
            "cargo": tool_value(inputs.cargo_identity),
            "rustc": tool_value(inputs.rustc_identity),
            "git": inputs.git_identity.map(tool_value),
            "git_tree_digest": inputs.git_evidence.map(|evidence| evidence.tree_digest.as_str()),
            "cargo_home_config": "absent",
            "cargo_working_directory": "/",
            "cargo_working_directory_config": "absent",
            "profiles": profile_identities,
        },
        "limits": limits_value(inputs.limits),
        "classifications": classifications,
        "exclusions": exclusions,
        "packages": packages,
        "edges": edges,
        "dependency_reachability": dependency_reachability,
        "processes": processes,
        "findings": findings,
        "result": {"status": status, "finding_count": inputs.findings.len()},
    });
    canonicalize_arrays(&mut report);
    let canonical = serde_json::to_vec(&report).map_err(|error| ReportError(error.to_string()))?;
    let identity = digest::bytes(&canonical);
    report
        .as_object_mut()
        .expect("report constructed as object")
        .insert("report_identity".to_owned(), Value::String(identity.clone()));
    crate::schema::validate_report(&report).map_err(|error| ReportError(error.to_string()))?;
    let mut bytes = serde_json::to_vec(&report).map_err(|error| ReportError(error.to_string()))?;
    bytes.push(b'\n');
    if bytes.len() > inputs.limits.report_bytes {
        return Err(ReportError(format!(
            "report is {} bytes; committed limit is {}",
            bytes.len(),
            inputs.limits.report_bytes
        )));
    }
    Ok(BuiltReport { value: report, bytes, identity })
}

pub fn verify(bytes: &[u8], limits: Limits) -> Result<VerifiedReport, ReportError> {
    if bytes.len() > limits.report_bytes {
        return Err(ReportError("report exceeds committed report byte limit".to_owned()));
    }
    let mut value = crate::json::parse(bytes, limits.json_depth, limits.json_container_items)
        .map_err(|error| ReportError(error.to_string()))?;
    crate::schema::validate_report(&value).map_err(|error| ReportError(error.to_string()))?;
    let mut ordered = value.clone();
    canonicalize_arrays(&mut ordered);
    if ordered != value {
        return Err(ReportError(
            "report arrays are not in canonical deterministic order".to_owned(),
        ));
    }
    validate_unique_keys(&value)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| ReportError("report root is not an object".to_owned()))?;
    if object.get("schema").and_then(Value::as_str) != Some(SCHEMA) {
        return Err(ReportError("report schema is not reovim-source-role-map/v1".to_owned()));
    }
    let identity = object
        .remove("report_identity")
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or_else(|| ReportError("report_identity is absent or not a string".to_owned()))?;
    let canonical = serde_json::to_vec(&value).map_err(|error| ReportError(error.to_string()))?;
    let actual = digest::bytes(&canonical);
    if actual != identity {
        return Err(ReportError(format!(
            "report identity mismatch: expected {identity}, computed {actual}"
        )));
    }
    let finding_count = value
        .pointer("/result/finding_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| ReportError("result.finding_count is absent or invalid".to_owned()))?;
    let actual_findings = value
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| ReportError("findings is absent or invalid".to_owned()))?
        .len();
    if finding_count
        != u64::try_from(actual_findings).map_err(|error| ReportError(error.to_string()))?
    {
        return Err(ReportError("result.finding_count does not match findings".to_owned()));
    }
    let status = value
        .pointer("/result/status")
        .and_then(Value::as_str)
        .ok_or_else(|| ReportError("result.status is absent or invalid".to_owned()))?;
    if (actual_findings == 0 && status != "pass") || (actual_findings != 0 && status != "fail") {
        return Err(ReportError("result.status disagrees with finding_count".to_owned()));
    }
    let mode = value
        .get("mode")
        .and_then(Value::as_str)
        .ok_or_else(|| ReportError("mode is absent or invalid".to_owned()))?;
    let revision = value.get("revision").unwrap_or(&Value::Null);
    let git = value.pointer("/identity_inputs/git").unwrap_or(&Value::Null);
    match mode {
        "candidate" if revision.is_null() && git.is_null() => {}
        "exact" if revision.as_str().is_some_and(valid_revision) && git.is_object() => {}
        _ => return Err(ReportError("mode/revision/Git identity are inconsistent".to_owned())),
    }
    let embedded_schema_digest = digest::bytes(crate::schema::embedded_report_schema_bytes());
    if value.pointer("/identity_inputs/report_schema_digest").and_then(Value::as_str)
        != Some(embedded_schema_digest.as_str())
    {
        return Err(ReportError(
            "identity_inputs.report_schema_digest differs from the embedded schema".to_owned(),
        ));
    }
    let execution_deadline = value
        .pointer("/limits/child_deadline_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| ReportError("execution deadline is absent".to_owned()))?;
    let supervision_deadline = value
        .pointer("/limits/child_supervision_deadline_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| ReportError("supervision deadline is absent".to_owned()))?;
    if supervision_deadline <= execution_deadline {
        return Err(ReportError("supervision deadline must exceed execution deadline".to_owned()));
    }
    if status == "pass" {
        validate_passing_v1_semantics(&value)?;
        validate_passing_processes(&value, mode)?;
    }
    Ok(VerifiedReport { identity, mode: mode.to_owned(), status: status.to_owned() })
}

#[allow(clippy::case_sensitive_file_extension_comparisons, clippy::too_many_lines)]
fn validate_passing_v1_semantics(value: &Value) -> Result<(), ReportError> {
    let profile_names = value
        .pointer("/identity_inputs/profiles")
        .and_then(Value::as_array)
        .ok_or_else(|| ReportError("identity profile list is absent".to_owned()))?
        .iter()
        .map(|profile| {
            profile
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| ReportError("identity profile name is invalid".to_owned()))
        })
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    if profile_names.is_empty() {
        return Err(ReportError("passing v1 report has no committed profiles".to_owned()));
    }
    let classifications = value
        .get("classifications")
        .and_then(Value::as_array)
        .ok_or_else(|| ReportError("classifications are absent".to_owned()))?;
    let mut classifications_by_path = std::collections::BTreeMap::new();
    for classification in classifications {
        let path = classification
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| ReportError("classification path is invalid".to_owned()))?;
        if !repository_tooling_owner(classification.get("source_owner")) {
            return Err(ReportError(
                "passing Phase-B v1 report contains a Product classification".to_owned(),
            ));
        }
        let source_use = classification
            .get("source_use")
            .and_then(Value::as_str)
            .ok_or_else(|| ReportError("classification source_use is invalid".to_owned()))?;
        let provenance = classification
            .get("source_provenance")
            .and_then(Value::as_str)
            .ok_or_else(|| ReportError("classification provenance is invalid".to_owned()))?;
        if source_use == "production"
            || source_use == "build_output"
            || (path.ends_with(".rs") && !matches!(source_use, "tool" | "test" | "example"))
        {
            return Err(ReportError(
                "passing Phase-B v1 report contains an inadmissible source use".to_owned(),
            ));
        }
        if provenance == "generated"
            && !(path == "Cargo.lock"
                && source_use == "tool"
                && classification.pointer("/upstream/digest_subject/kind").and_then(Value::as_str)
                    == Some("paths")
                && classification
                    .pointer("/upstream/digest_subject/paths")
                    .and_then(Value::as_array)
                    .is_some_and(|paths| !paths.is_empty()))
        {
            return Err(ReportError(
                "passing Phase-B v1 report admits generated provenance only for bound Cargo.lock"
                    .to_owned(),
            ));
        }
        classifications_by_path.insert(path, source_use);
    }

    let packages = value
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| ReportError("packages are absent".to_owned()))?;
    let mut packages_by_profile =
        std::collections::BTreeMap::<&str, std::collections::BTreeSet<&str>>::new();
    for package in packages {
        let profile = package.get("profile").and_then(Value::as_str).unwrap_or_default();
        let name = package.get("name").and_then(Value::as_str).unwrap_or_default();
        if !profile_names.contains(profile) || profile.is_empty() || name.is_empty() {
            return Err(ReportError("package uses an unknown profile or identity".to_owned()));
        }
        let owners = package
            .get("owners")
            .and_then(Value::as_array)
            .ok_or_else(|| ReportError("package owners are absent".to_owned()))?;
        if owners.is_empty() || !owners.iter().all(|owner| repository_tooling_owner(Some(owner))) {
            return Err(ReportError(
                "passing Phase-B v1 package ownership is not RepositoryTooling-only".to_owned(),
            ));
        }
        packages_by_profile.entry(profile).or_default().insert(name);
        for target in package
            .get("targets")
            .and_then(Value::as_array)
            .ok_or_else(|| ReportError("package targets are absent".to_owned()))?
        {
            let source = target.get("src_path").and_then(Value::as_str).unwrap_or_default();
            let kinds = target
                .get("kind")
                .and_then(Value::as_array)
                .ok_or_else(|| ReportError("target kinds are absent".to_owned()))?;
            let expected_use = if kinds
                .iter()
                .any(|kind| kind.as_str().is_some_and(|kind| matches!(kind, "test" | "bench")))
            {
                "test"
            } else if kinds.iter().any(|kind| kind.as_str() == Some("example")) {
                "example"
            } else {
                "tool"
            };
            if classifications_by_path.get(source).copied() != Some(expected_use) {
                return Err(ReportError(
                    "passing report target source classification disagrees with target use"
                        .to_owned(),
                ));
            }
        }
    }
    let Some(reference_packages) = packages_by_profile.values().next().cloned() else {
        return Err(ReportError("passing v1 report has no workspace packages".to_owned()));
    };
    if packages_by_profile.len() != profile_names.len()
        || packages_by_profile.values().any(|packages| *packages != reference_packages)
    {
        return Err(ReportError(
            "workspace package set is not exact and equal across profiles".to_owned(),
        ));
    }

    for edge in value
        .get("edges")
        .and_then(Value::as_array)
        .ok_or_else(|| ReportError("edges are absent".to_owned()))?
    {
        let profile = edge.get("profile").and_then(Value::as_str).unwrap_or_default();
        let consumer = edge.get("consumer").and_then(Value::as_str).unwrap_or_default();
        let provider = edge.get("provider").and_then(Value::as_str).unwrap_or_default();
        let packages = packages_by_profile.get(profile).ok_or_else(|| {
            ReportError("edge uses a profile absent from exact identity inputs".to_owned())
        })?;
        if !packages.contains(consumer)
            || !packages.contains(provider)
            || edge.get("decision").and_then(Value::as_str) != Some("admitted_same_owner")
            || !["consumer_owners", "provider_owners"].into_iter().all(|field| {
                edge.get(field).and_then(Value::as_array).is_some_and(|owners| {
                    !owners.is_empty()
                        && owners.iter().all(|owner| repository_tooling_owner(Some(owner)))
                })
            })
        {
            return Err(ReportError(
                "passing Phase-B v1 edge is not an admitted RepositoryTooling edge".to_owned(),
            ));
        }
    }

    let mut reachability_by_profile =
        std::collections::BTreeMap::<&str, std::collections::BTreeSet<&str>>::new();
    for reachability in value
        .get("dependency_reachability")
        .and_then(Value::as_array)
        .ok_or_else(|| ReportError("dependency_reachability is absent".to_owned()))?
    {
        let profile = reachability.get("profile").and_then(Value::as_str).unwrap_or_default();
        let package = reachability.get("package").and_then(Value::as_str).unwrap_or_default();
        if !packages_by_profile.get(profile).is_some_and(|packages| packages.contains(package)) {
            return Err(ReportError(
                "dependency reachability uses an unknown profile/package".to_owned(),
            ));
        }
        reachability_by_profile.entry(profile).or_default().insert(package);
    }
    if profile_names.iter().any(|profile| {
        reachability_by_profile.get(profile.as_str()) != packages_by_profile.get(profile.as_str())
    }) {
        return Err(ReportError(
            "dependency reachability does not cover every exact profile/package".to_owned(),
        ));
    }
    Ok(())
}

fn repository_tooling_owner(value: Option<&Value>) -> bool {
    value.and_then(Value::as_object).is_some_and(|owner| {
        owner.get("kind").and_then(Value::as_str) == Some("repository_tooling")
    })
}

fn validate_unique_keys(value: &Value) -> Result<(), ReportError> {
    unique_by(value, "/classifications", |item| string_key(item, "path"))?;
    unique_by(value, "/exclusions", |item| string_key(item, "prefix"))?;
    unique_by(value, "/identity_inputs/lockfiles", |item| string_key(item, "path"))?;
    unique_by(value, "/identity_inputs/profiles", |item| string_key(item, "name"))?;
    unique_by(value, "/packages", |item| pair_key(item, "profile", "name"))?;
    unique_by(value, "/dependency_reachability", |item| pair_key(item, "profile", "package"))?;
    unique_by(value, "/edges", |item| {
        let mut key = pair_key(item, "profile", "consumer")?;
        for field in ["provider", "kind"] {
            key.push('\0');
            key.push_str(item.get(field)?.as_str()?);
        }
        key.push('\0');
        if let Some(target) = item.get("target")?.as_str() {
            key.push_str(target);
        }
        Some(key)
    })?;
    unique_by(value, "/findings", |item| serde_json::to_string(item).ok())?;
    let packages = value
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| ReportError("packages is absent".to_owned()))?;
    for package in packages {
        let package_name = package.get("name").and_then(Value::as_str).unwrap_or("?");
        let targets = package
            .get("targets")
            .and_then(Value::as_array)
            .ok_or_else(|| ReportError("package targets are absent".to_owned()))?;
        let mut names = std::collections::BTreeSet::new();
        let mut sources = std::collections::BTreeSet::new();
        for target in targets {
            let name = target.get("name").and_then(Value::as_str).unwrap_or_default();
            let source = target.get("src_path").and_then(Value::as_str).unwrap_or_default();
            if !names.insert(name) || !sources.insert(source) {
                return Err(ReportError(format!(
                    "package {package_name} has duplicate target name or source"
                )));
            }
        }
    }
    Ok(())
}

fn unique_by(
    value: &Value,
    pointer: &str,
    key: impl Fn(&Value) -> Option<String>,
) -> Result<(), ReportError> {
    let values = value
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or_else(|| ReportError(format!("{pointer} is absent or invalid")))?;
    let mut keys = std::collections::BTreeSet::new();
    for item in values {
        let key = key(item).ok_or_else(|| ReportError(format!("{pointer} key is invalid")))?;
        if !keys.insert(key) {
            return Err(ReportError(format!("{pointer} contains a duplicate semantic key")));
        }
    }
    Ok(())
}

fn string_key(value: &Value, field: &str) -> Option<String> {
    value.get(field)?.as_str().map(str::to_owned)
}

fn pair_key(value: &Value, first: &str, second: &str) -> Option<String> {
    Some(format!("{}\0{}", value.get(first)?.as_str()?, value.get(second)?.as_str()?))
}

fn canonicalize_arrays(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in &mut *values {
                canonicalize_arrays(value);
            }
            values.sort_by_key(Value::to_string);
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                canonicalize_arrays(value);
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_lines)]
fn validate_passing_processes(value: &Value, mode: &str) -> Result<(), ReportError> {
    let processes = value
        .get("processes")
        .and_then(Value::as_array)
        .ok_or_else(|| ReportError("processes is absent or invalid".to_owned()))?;
    let stdout_limit = value
        .pointer("/limits/metadata_stdout_bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| ReportError("metadata stdout limit is absent".to_owned()))?;
    let stderr_limit = value
        .pointer("/limits/child_stderr_bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| ReportError("child stderr limit is absent".to_owned()))?;
    let cargo_digest = value
        .pointer("/identity_inputs/cargo/executable_digest")
        .and_then(Value::as_str)
        .ok_or_else(|| ReportError("Cargo executable digest is absent".to_owned()))?;
    let rustc_digest = value
        .pointer("/identity_inputs/rustc/executable_digest")
        .and_then(Value::as_str)
        .ok_or_else(|| ReportError("Rustc executable digest is absent".to_owned()))?;
    let git_digest =
        value.pointer("/identity_inputs/git/executable_digest").and_then(Value::as_str);
    let mut actual = std::collections::BTreeMap::new();
    for process in processes {
        let purpose = process
            .get("purpose")
            .and_then(Value::as_str)
            .ok_or_else(|| ReportError("process purpose is absent".to_owned()))?;
        let profile = process.get("profile").and_then(Value::as_str).map(str::to_owned);
        *actual.entry((purpose.to_owned(), profile)).or_insert(0_usize) += 1;
        let success = process.get("termination_reason").and_then(Value::as_str) == Some("exited")
            && process.get("status").and_then(Value::as_i64) == Some(0)
            && process.get("stdout_overflow").and_then(Value::as_bool) == Some(false)
            && process.get("stderr_overflow").and_then(Value::as_bool) == Some(false)
            && process.get("reaped").and_then(Value::as_bool) == Some(true)
            && process.get("readers_joined").and_then(Value::as_bool) == Some(true)
            && process.get("kill_attempted").and_then(Value::as_bool) == Some(false)
            && process.get("kill_raced_with_exit").and_then(Value::as_bool) == Some(false)
            && process.get("limit_observed_after_exit").and_then(Value::as_bool) == Some(false)
            && process.get("stdout_limit").and_then(Value::as_u64) == Some(stdout_limit)
            && process.get("stderr_limit").and_then(Value::as_u64) == Some(stderr_limit)
            && process
                .get("stdout_bytes")
                .and_then(Value::as_u64)
                .is_some_and(|count| count <= stdout_limit)
            && process.get("stderr_bytes").and_then(Value::as_u64) == Some(0);
        if !success {
            return Err(ReportError("passing report contains a failed/unowned process".to_owned()));
        }
        let expected_digest = if purpose.starts_with("cargo_") {
            Some(cargo_digest)
        } else if purpose == "rustc_version" {
            Some(rustc_digest)
        } else if purpose.starts_with("git_") {
            git_digest
        } else {
            None
        };
        if process.get("executable_digest").and_then(Value::as_str) != expected_digest {
            return Err(ReportError(format!("{purpose} has the wrong executable digest")));
        }
    }
    let profile_names = value
        .pointer("/identity_inputs/profiles")
        .and_then(Value::as_array)
        .ok_or_else(|| ReportError("identity profile list is absent".to_owned()))?
        .iter()
        .filter_map(|profile| profile.get("name").and_then(Value::as_str))
        .collect::<std::collections::BTreeSet<_>>();
    let mut expected = std::collections::BTreeMap::from([
        (("cargo_version".to_owned(), None), 1_usize),
        (("rustc_version".to_owned(), None), 1_usize),
    ]);
    for profile in &profile_names {
        expected.insert(("cargo_metadata".to_owned(), Some((*profile).to_owned())), 1);
        if mode == "exact" {
            expected.insert(("cargo_metadata_final".to_owned(), Some((*profile).to_owned())), 1);
        }
    }
    if mode == "exact" {
        for purpose in [
            "git_version",
            "git_root",
            "git_head",
            "git_object_format",
            "git_ls_tree",
            "git_index_entries",
            "git_info_attributes_path",
            "git_pre_metadata_version",
            "git_pre_metadata_root",
            "git_pre_metadata_head",
            "git_pre_metadata_object_format",
            "git_pre_metadata_ls_tree",
            "git_pre_metadata_index_entries",
            "git_pre_metadata_info_attributes_path",
            "git_final_version",
            "git_final_root",
            "git_final_head",
            "git_final_object_format",
            "git_final_ls_tree",
            "git_final_index_entries",
            "git_final_info_attributes_path",
        ] {
            expected.insert((purpose.to_owned(), None), 1);
        }
    }
    if actual != expected {
        return Err(ReportError(
            "process purpose/profile multiset is incomplete or duplicated".to_owned(),
        ));
    }
    Ok(())
}

fn valid_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use serde_json::{Value, json};

    fn passing_v1() -> Value {
        json!({
            "identity_inputs": {"profiles": [{"name": "p"}]},
            "classifications": [{
                "path": "tools/checker/src/lib.rs",
                "source_owner": {"kind": "repository_tooling"},
                "source_provenance": "authored",
                "source_use": "tool"
            }],
            "packages": [{
                "profile": "p",
                "name": "checker",
                "owners": [{"kind": "repository_tooling"}],
                "targets": [{"kind": ["lib"], "src_path": "tools/checker/src/lib.rs"}]
            }],
            "edges": [],
            "dependency_reachability": [{"profile": "p", "package": "checker", "members": []}]
        })
    }

    #[test]
    fn passing_v1_semantics_reject_product_use_profile_and_edge_mutants() {
        assert!(super::validate_passing_v1_semantics(&passing_v1()).is_ok());

        let mut product = passing_v1();
        product["classifications"][0]["source_owner"] =
            json!({"kind": "product", "role": "foundation"});
        assert!(super::validate_passing_v1_semantics(&product).is_err());

        let mut build_output = passing_v1();
        build_output["classifications"][0]["source_use"] = json!("build_output");
        assert!(super::validate_passing_v1_semantics(&build_output).is_err());

        let mut missing_profile = passing_v1();
        missing_profile["dependency_reachability"][0]["profile"] = json!("other");
        assert!(super::validate_passing_v1_semantics(&missing_profile).is_err());

        let mut edge = passing_v1();
        edge["edges"] = json!([{
            "profile": "p",
            "consumer": "checker",
            "provider": "checker",
            "decision": "admitted_product_policy",
            "consumer_owners": [{"kind": "repository_tooling"}],
            "provider_owners": [{"kind": "repository_tooling"}]
        }]);
        assert!(super::validate_passing_v1_semantics(&edge).is_err());
    }
}

fn owner_value(owner: SourceOwner) -> Value {
    match owner {
        SourceOwner::Product(role) => json!({"kind": "product", "role": role.as_str()}),
        SourceOwner::RepositoryTooling => json!({"kind": "repository_tooling"}),
    }
}

fn upstream_value(upstream: &Upstream) -> Value {
    let subject = match &upstream.digest_subject {
        DigestSubject::Paths(paths) => json!({
            "kind": "paths",
            "paths": paths.iter().map(|path| display_relative(path)).collect::<Vec<_>>(),
        }),
        DigestSubject::ClassifiedContent => json!({"kind": "classified_content"}),
    };
    json!({
        "identity": upstream.identity,
        "digest": upstream.digest,
        "digest_subject": subject,
    })
}

fn tool_value(tool: &ToolIdentity) -> Value {
    json!({
        "canonical_path": tool.canonical_path,
        "executable_digest": tool.executable_digest,
        "version": tool.version,
        "version_digest": tool.version_digest,
    })
}

fn process_value(
    purpose: &str,
    profile: Option<&str>,
    executable_digest: &str,
    process: &ProcessResult,
    limits: ProcessLimits,
) -> Value {
    json!({
        "purpose": purpose,
        "profile": profile,
        "executable_digest": executable_digest,
        "termination_reason": process.termination_reason.as_str(),
        "status": process.status.code(),
        "stdout_bytes": process.stdout_bytes,
        "stderr_bytes": process.stderr_bytes,
        "stdout_limit": limits.stdout_bytes,
        "stderr_limit": limits.stderr_bytes,
        "stdout_overflow": process.stdout_overflow,
        "stderr_overflow": process.stderr_overflow,
        "kill_attempted": process.kill_attempted,
        "kill_raced_with_exit": process.kill_raced_with_exit,
        "limit_observed_after_exit": process.limit_observed_after_exit,
        "reaped": process.reaped,
        "readers_joined": process.readers_joined,
    })
}

fn limits_value(limits: Limits) -> Value {
    json!({
        "config_bytes": limits.config_bytes,
        "json_depth": limits.json_depth,
        "json_container_items": limits.json_container_items,
        "metadata_stdout_bytes": limits.metadata_stdout_bytes,
        "tool_executable_bytes": limits.tool_executable_bytes,
        "child_stderr_bytes": limits.child_stderr_bytes,
        "child_deadline_ms": limits.child_deadline_ms,
        "child_supervision_deadline_ms": limits.child_supervision_deadline_ms,
        "report_bytes": limits.report_bytes,
        "source_file_count": limits.source_file_count,
        "source_entry_count": limits.source_entry_count,
        "directory_entry_count": limits.directory_entry_count,
        "source_total_bytes": limits.source_total_bytes,
        "source_file_bytes": limits.source_file_bytes,
        "path_bytes": limits.path_bytes,
        "syntax_item_expr_type_visits": limits.syntax_item_expr_type_visits,
        "findings": limits.findings,
    })
}

pub fn absolute_tool_identity(
    path: &Path,
    version: &str,
    version_digest: &str,
    byte_limit: u64,
) -> Result<ToolIdentity, ReportError> {
    let canonical = path.canonicalize().map_err(|error| ReportError(error.to_string()))?;
    let executable_digest = digest::file_bounded(&canonical, byte_limit)
        .map_err(|error| ReportError(format!("{}: {error}", canonical.display())))?;
    Ok(ToolIdentity {
        canonical_path: canonical.display().to_string(),
        executable_digest,
        version: version.to_owned(),
        version_digest: version_digest.to_owned(),
    })
}

pub fn lockfile_identities(
    paths: &[std::path::PathBuf],
    inventory: &[crate::source::ClassifiedFile],
) -> Result<Vec<(String, String)>, ReportError> {
    paths
        .iter()
        .map(|path| {
            let file = inventory.iter().find(|file| file.path == *path).ok_or_else(|| {
                ReportError(format!("{} is absent from source inventory", path.display()))
            })?;
            Ok((display_relative(path), file.content_digest.clone()))
        })
        .collect()
}
