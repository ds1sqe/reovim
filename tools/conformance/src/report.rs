//! Frozen standalone verification for `reovim-source-role-map/v1` reports.

use std::fmt;

use serde_json::Value;

use crate::{digest, source::display_relative, topology::Limits};

pub const SCHEMA: &str = "reovim-source-role-map/v1";

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
    let embedded_schema_digest = digest::bytes(crate::schema::embedded_v1_report_schema_bytes());
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
