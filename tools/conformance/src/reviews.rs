//! Exact external-Cargo-package admission registry.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use reovim_depgraph::{SourceOwner, SourceProvenance, SourceUse};

use serde_json::Value;

use crate::{
    cargo::Capture,
    digest,
    finding::{Finding, Findings},
    json::{self, ConfigError},
    policy::relative_path,
    source::ClassifiedFile,
};

pub const SCHEMA: &str = "reovim-dependency-reviews/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    pub reviewed_at: String,
    pub lock_digest: String,
    pub admissions: Vec<Admission>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Admission {
    pub name: String,
    pub version: String,
    pub source: String,
    pub checksum: String,
    pub license: String,
    pub resolved_consumers: Vec<String>,
    pub resolved_profiles: Vec<String>,
    pub disposition: String,
    pub review_record: PathBuf,
}

impl Registry {
    pub fn parse(value: &Value) -> Result<Self, ConfigError> {
        let object =
            json::object(value, "$", &["schema", "reviewed_at", "lock_digest", "admissions"], &[])?;
        if json::string(&object["schema"], "$.schema")? != SCHEMA {
            return Err(ConfigError::new("CFG_CONST", "$.schema", format!("expected {SCHEMA:?}")));
        }
        let reviewed_at = nonempty(&object["reviewed_at"], "$.reviewed_at")?;
        if !utc_timestamp(&reviewed_at) {
            return Err(ConfigError::new(
                "REVIEW_TIMESTAMP",
                "$.reviewed_at",
                "expected YYYY-MM-DDTHH:MM:SSZ",
            ));
        }
        let lock_digest = nonempty(&object["lock_digest"], "$.lock_digest")?;
        if !tagged_digest(&lock_digest) {
            return Err(ConfigError::new(
                "REVIEW_LOCK_DIGEST",
                "$.lock_digest",
                "expected sha256 plus 64 lowercase hex digits",
            ));
        }
        let mut admissions = Vec::new();
        let mut identities = BTreeSet::new();
        for (item, value) in json::array(&object["admissions"], "$.admissions")?.iter().enumerate()
        {
            let path = json::index("$.admissions", item);
            let object = json::object(
                value,
                &path,
                &[
                    "name",
                    "version",
                    "source",
                    "checksum",
                    "license",
                    "resolved_consumers",
                    "resolved_profiles",
                    "disposition",
                    "review_record",
                ],
                &[],
            )?;
            let name = nonempty(&object["name"], &json::child(&path, "name"))?;
            let version = nonempty(&object["version"], &json::child(&path, "version"))?;
            let source = nonempty(&object["source"], &json::child(&path, "source"))?;
            let checksum = nonempty(&object["checksum"], &json::child(&path, "checksum"))?;
            if !plain_digest(&checksum) {
                return Err(ConfigError::new(
                    "REVIEW_CHECKSUM",
                    json::child(&path, "checksum"),
                    "expected Cargo's 64 lowercase hex checksum",
                ));
            }
            let license = nonempty(&object["license"], &json::child(&path, "license"))?;
            let resolved_consumers =
                strings(&object["resolved_consumers"], &json::child(&path, "resolved_consumers"))?;
            let resolved_profiles =
                strings(&object["resolved_profiles"], &json::child(&path, "resolved_profiles"))?;
            let disposition = nonempty(&object["disposition"], &json::child(&path, "disposition"))?;
            if disposition != "admit" {
                return Err(ConfigError::new(
                    "REVIEW_DISPOSITION",
                    json::child(&path, "disposition"),
                    "only explicit admit records are accepted",
                ));
            }
            let review_record = relative_path(
                json::string(&object["review_record"], &json::child(&path, "review_record"))?,
                &json::child(&path, "review_record"),
                false,
            )?;
            if !identities.insert((name.clone(), version.clone(), source.clone())) {
                return Err(ConfigError::new(
                    "REVIEW_DUPLICATE_PACKAGE",
                    path,
                    "duplicate external package admission",
                ));
            }
            admissions.push(Admission {
                name,
                version,
                source,
                checksum,
                license,
                resolved_consumers,
                resolved_profiles,
                disposition,
                review_record,
            });
        }
        admissions.sort();
        Ok(Self { reviewed_at, lock_digest, admissions })
    }
}

#[allow(clippy::too_many_lines)]
pub fn validate(
    root: &Path,
    lockfile: &Path,
    registry: &Registry,
    profiles: &[(crate::topology::Profile, Capture)],
    finding_limit: usize,
    lock_byte_limit: u64,
    inventory: &[ClassifiedFile],
) -> Vec<Finding> {
    let mut findings = Findings::new(finding_limit);
    let absolute_lock = root.join(lockfile);
    let lock_bytes = match crate::source::read_bounded(&absolute_lock, lock_byte_limit) {
        Ok(bytes) => bytes,
        Err(error) => {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_LOCK_IO",
                Some(lockfile.to_string_lossy().into_owned()),
                error,
            ));
            return findings.finish();
        }
    };
    let actual_lock_digest = digest::bytes(&lock_bytes);
    if actual_lock_digest != registry.lock_digest {
        findings.push(Finding::new(
            "dependency_review",
            "REVIEW_LOCK_DIGEST_MISMATCH",
            Some(lockfile.to_string_lossy().into_owned()),
            format!("registry {}, actual {actual_lock_digest}", registry.lock_digest),
        ));
    }
    if !inventory
        .iter()
        .any(|file| file.path == lockfile && file.content_digest == actual_lock_digest)
    {
        findings.push(Finding::new(
            "dependency_review",
            "REVIEW_LOCK_INVENTORY_DRIFT",
            Some(lockfile.to_string_lossy().into_owned()),
            "parsed Cargo.lock bytes differ from the inventoried bytes",
        ));
    }
    let lock_packages = match parse_lock(&lock_bytes) {
        Ok(packages) => packages,
        Err(error) => {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_LOCK_PARSE",
                Some(lockfile.to_string_lossy().into_owned()),
                error,
            ));
            return findings.finish();
        }
    };
    let external_lock = lock_packages
        .into_iter()
        .filter(|package| package.source.is_some())
        .map(|package| {
            (
                (
                    package.name.clone(),
                    package.version.clone(),
                    package.source.clone().expect("filtered"),
                ),
                package,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let admissions = registry
        .admissions
        .iter()
        .map(|admission| {
            (
                (admission.name.clone(), admission.version.clone(), admission.source.clone()),
                admission,
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (identity, package) in &external_lock {
        let Some(admission) = admissions.get(identity) else {
            findings.push(Finding::new(
                "dependency_review",
                "UNREVIEWED_EXTERNAL_PACKAGE",
                None,
                format!(
                    "{} {} ({})",
                    package.name,
                    package.version,
                    package.source.as_deref().unwrap_or("")
                ),
            ));
            continue;
        };
        if package.checksum.as_deref() != Some(admission.checksum.as_str()) {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_CHECKSUM_DRIFT",
                None,
                format!("{} {} checksum differs from Cargo.lock", package.name, package.version),
            ));
        }
    }
    for (identity, admission) in &admissions {
        if !external_lock.contains_key(identity) {
            findings.push(Finding::new(
                "dependency_review",
                "STALE_DEPENDENCY_ADMISSION",
                Some(admission.review_record.to_string_lossy().into_owned()),
                format!("{} {} is absent from Cargo.lock", admission.name, admission.version),
            ));
        }
        let admitted_record = inventory.iter().any(|file| {
            file.path == admission.review_record
                && file.classification.owner == SourceOwner::RepositoryTooling
                && file.classification.provenance == SourceProvenance::Authored
                && file.classification.source_use == SourceUse::Tool
        });
        if !admitted_record {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_RECORD_MISSING",
                Some(admission.review_record.to_string_lossy().into_owned()),
                "admission review record must be inventoried as RepositoryTooling + Authored + Tool",
            ));
        }
    }

    let mut resolved_profiles: BTreeMap<(String, String, String), BTreeSet<String>> =
        BTreeMap::new();
    let mut resolved_consumers: BTreeMap<(String, String, String), BTreeSet<String>> =
        BTreeMap::new();
    let mut metadata_packages = BTreeMap::new();
    for (profile, capture) in profiles {
        let resolved = capture.metadata.resolved_package_ids();
        for id in &resolved {
            let Some(package) = capture.metadata.packages.get(id) else { continue };
            let Some(source) = &package.source else { continue };
            resolved_profiles
                .entry((package.name.clone(), package.version.clone(), source.clone()))
                .or_default()
                .insert(profile.name.clone());
        }
        for (consumer_id, node) in &capture.metadata.nodes {
            if !resolved.contains(consumer_id) {
                continue;
            }
            let Some(consumer) = capture.metadata.packages.get(consumer_id) else { continue };
            for dependency in &node.dependencies {
                if !resolved.contains(&dependency.package) {
                    continue;
                }
                let Some(provider) = capture.metadata.packages.get(&dependency.package) else {
                    continue;
                };
                let Some(source) = &provider.source else { continue };
                resolved_consumers
                    .entry((provider.name.clone(), provider.version.clone(), source.clone()))
                    .or_default()
                    .insert(consumer.review_identity());
            }
        }
        for package in capture.metadata.packages.values() {
            if let Some(source) = &package.source {
                metadata_packages.insert(
                    (package.name.clone(), package.version.clone(), source.clone()),
                    package,
                );
            }
        }
    }
    let topology_profiles =
        profiles.iter().map(|(profile, _)| profile.name.clone()).collect::<BTreeSet<_>>();
    for (identity, admission) in admissions {
        let actual_profiles = resolved_profiles.remove(&identity).unwrap_or_default();
        let declared_profiles =
            admission.resolved_profiles.iter().cloned().collect::<BTreeSet<_>>();
        if !declared_profiles.is_subset(&topology_profiles) {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_UNKNOWN_PROFILE",
                Some(admission.review_record.to_string_lossy().into_owned()),
                "admission references a profile absent from topology",
            ));
        }
        if actual_profiles != declared_profiles {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_PROFILE_DRIFT",
                Some(admission.review_record.to_string_lossy().into_owned()),
                format!("{} {} resolved profile set differs", admission.name, admission.version),
            ));
        }
        let actual_consumers = resolved_consumers.remove(&identity).unwrap_or_default();
        let declared_consumers =
            admission.resolved_consumers.iter().cloned().collect::<BTreeSet<_>>();
        if actual_consumers != declared_consumers {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_CONSUMER_DRIFT",
                Some(admission.review_record.to_string_lossy().into_owned()),
                format!("{} {} resolved consumer set differs", admission.name, admission.version),
            ));
        }
        if let Some(package) = metadata_packages.get(&identity) {
            if package.checksum.as_deref().is_some_and(|checksum| checksum != admission.checksum) {
                findings.push(Finding::new(
                    "dependency_review",
                    "REVIEW_METADATA_CHECKSUM_DRIFT",
                    None,
                    format!(
                        "{} {} checksum differs from Cargo metadata",
                        admission.name, admission.version
                    ),
                ));
            }
            if package.license.as_deref() != Some(admission.license.as_str()) {
                findings.push(Finding::new(
                    "dependency_review",
                    "REVIEW_LICENSE_DRIFT",
                    None,
                    format!(
                        "{} {} license differs from Cargo metadata",
                        admission.name, admission.version
                    ),
                ));
            }
        }
    }
    findings.finish()
}

#[derive(Debug)]
struct LockPackage {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
}

fn parse_lock(bytes: &[u8]) -> Result<Vec<LockPackage>, String> {
    let input = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let mut packages = Vec::new();
    let mut fields = BTreeMap::new();
    let mut in_package = false;
    let mut format_version = None;
    for (line_number, raw) in input.lines().enumerate() {
        let line = raw.trim();
        if line == "[[package]]" {
            if in_package {
                packages.push(lock_package(&fields, line_number)?);
                fields.clear();
            }
            in_package = true;
            continue;
        }
        if line.starts_with("[[") {
            if in_package {
                packages.push(lock_package(&fields, line_number)?);
                fields.clear();
                in_package = false;
            }
            continue;
        }
        if !in_package && line.starts_with("version") {
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!("malformed Cargo.lock version at line {}", line_number + 1));
            };
            if key.trim() != "version" || format_version.replace(value.trim().to_owned()).is_some()
            {
                return Err("duplicate or unsupported Cargo.lock format version".to_owned());
            }
            continue;
        }
        if !in_package || line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim();
        if key == "replace" {
            return Err(format!(
                "unsupported Cargo.lock replace identity at line {}",
                line_number + 1
            ));
        }
        if matches!(key, "name" | "version" | "source" | "checksum") {
            let value = parse_quoted(value.trim())?;
            if fields.insert(key.to_owned(), value).is_some() {
                return Err(format!(
                    "duplicate {key} in Cargo.lock package near line {}",
                    line_number + 1
                ));
            }
        }
    }
    if in_package {
        packages.push(lock_package(&fields, input.lines().count())?);
    }
    if format_version.as_deref() != Some("4") {
        return Err(format!(
            "unsupported Cargo.lock format version {format_version:?}; expected 4"
        ));
    }
    let mut identities = BTreeSet::new();
    for package in &packages {
        if !identities.insert((
            package.name.clone(),
            package.version.clone(),
            package.source.clone(),
        )) {
            return Err(format!(
                "duplicate Cargo.lock package {} {}",
                package.name, package.version
            ));
        }
    }
    Ok(packages)
}

fn lock_package(fields: &BTreeMap<String, String>, line: usize) -> Result<LockPackage, String> {
    let name = fields
        .get("name")
        .ok_or_else(|| format!("Cargo.lock package near line {line} lacks name"))?
        .clone();
    let version = fields
        .get("version")
        .ok_or_else(|| format!("Cargo.lock package {name} lacks version"))?
        .clone();
    let source = fields.get("source").cloned();
    let checksum = fields.get("checksum").cloned();
    if source.is_some() && checksum.is_none() {
        return Err(format!("external Cargo.lock package {name} {version} lacks checksum"));
    }
    Ok(LockPackage { name, version, source, checksum })
}

fn parse_quoted(value: &str) -> Result<String, String> {
    serde_json::from_str::<String>(value)
        .map_err(|error| format!("unsupported Cargo.lock string {value:?}: {error}"))
}

fn strings(value: &Value, path: &str) -> Result<Vec<String>, ConfigError> {
    let mut result = BTreeSet::new();
    for (item, value) in json::array(value, path)?.iter().enumerate() {
        let item_path = json::index(path, item);
        let value = nonempty(value, &item_path)?;
        if !result.insert(value) {
            return Err(ConfigError::new("REVIEW_DUPLICATE_VALUE", item_path, "duplicate value"));
        }
    }
    Ok(result.into_iter().collect())
}

fn nonempty(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = json::string(value, path)?;
    if value.is_empty() {
        Err(ConfigError::new("CFG_EMPTY_STRING", path, "value must not be empty"))
    } else {
        Ok(value.to_owned())
    }
}

fn utc_timestamp(value: &str) -> bool {
    value.len() == 20
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value.as_bytes()[10] == b'T'
        && value.as_bytes()[13] == b':'
        && value.as_bytes()[16] == b':'
        && value.as_bytes()[19] == b'Z'
        && value.bytes().enumerate().all(|(index, byte)| {
            matches!(index, 4 | 7 | 10 | 13 | 16 | 19) || byte.is_ascii_digit()
        })
}

fn tagged_digest(value: &str) -> bool {
    value.len() == 71 && value.starts_with("sha256:") && plain_digest(&value[7..])
}

fn plain_digest(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    fn registry() -> serde_json::Value {
        serde_json::from_str(include_str!("../../../conformance/dependency-reviews.json")).unwrap()
    }

    #[test]
    fn review_record_root_and_unknown_fields_fail() {
        let mut root = registry();
        root["admissions"][0]["review_record"] = json!(".");
        assert_eq!(super::Registry::parse(&root).unwrap_err().code, "CFG_PATH_FORM");

        let mut unknown = registry();
        unknown["admissions"][0]
            .as_object_mut()
            .unwrap()
            .insert("active_profiles".to_owned(), json!([]));
        assert_eq!(super::Registry::parse(&unknown).unwrap_err().code, "CFG_UNKNOWN_FIELD");
    }

    #[test]
    fn lock_parser_rejects_format_and_duplicate_identity_mutants() {
        assert!(super::parse_lock(b"version = 3\n").unwrap_err().contains("expected 4"));
        let duplicate = br#"version = 4
[[package]]
name = "x"
version = "1"
source = "registry+x"
checksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
[[package]]
name = "x"
version = "1"
source = "registry+x"
checksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
"#;
        assert!(super::parse_lock(duplicate).unwrap_err().contains("duplicate Cargo.lock package"));
    }
}
