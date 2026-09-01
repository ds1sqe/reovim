//! Parser for the committed exact workspace topology and safety limits.

use std::{collections::BTreeSet, path::PathBuf, time::Duration};

use serde_json::Value;

use crate::{
    json::{self, ConfigError},
    policy::relative_path,
    process::ProcessLimits,
};

pub const SCHEMA: &str = "reovim-workspace-topology/v1";

/// Bootstrap ceilings enforced before a topology file can select its own
/// equal-or-lower committed maxima.
pub const HARD_LIMITS: Limits = Limits {
    config_bytes: 1_048_576,
    metadata_stdout_bytes: 16_777_216,
    tool_executable_bytes: 67_108_864,
    child_stderr_bytes: 1_048_576,
    child_deadline_ms: 55_000,
    child_supervision_deadline_ms: 60_000,
    report_bytes: 16_777_216,
    source_file_count: 8_192,
    source_entry_count: 16_384,
    directory_entry_count: 8_192,
    source_total_bytes: 67_108_864,
    source_file_bytes: 4_194_304,
    path_bytes: 4_096,
    syntax_item_expr_type_visits: 1_000_000,
    findings: 4_096,
    json_depth: 128,
    json_container_items: 50_000,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topology {
    pub workspace_manifest: PathBuf,
    pub workspace_members: Vec<PathBuf>,
    pub manifests: Vec<PathBuf>,
    pub lockfiles: Vec<PathBuf>,
    pub cargo_config_files: Vec<PathBuf>,
    pub build_scripts: Vec<PathBuf>,
    pub custom_target_specs: Vec<PathBuf>,
    pub linker_assets: Vec<PathBuf>,
    pub limits: Limits,
    pub profiles: Vec<Profile>,
    pub packages: Vec<Package>,
    pub repository_entrypoints: Vec<RepositoryEntrypoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub config_bytes: usize,
    pub metadata_stdout_bytes: usize,
    pub tool_executable_bytes: u64,
    pub child_stderr_bytes: usize,
    pub child_deadline_ms: u64,
    pub child_supervision_deadline_ms: u64,
    pub report_bytes: usize,
    pub source_file_count: usize,
    pub source_entry_count: usize,
    pub directory_entry_count: usize,
    pub source_total_bytes: u64,
    pub source_file_bytes: u64,
    pub path_bytes: usize,
    pub syntax_item_expr_type_visits: usize,
    pub findings: usize,
    pub json_depth: usize,
    pub json_container_items: usize,
}

impl Limits {
    #[must_use]
    pub const fn process(self) -> ProcessLimits {
        ProcessLimits {
            stdout_bytes: self.metadata_stdout_bytes,
            stderr_bytes: self.child_stderr_bytes,
            deadline: Duration::from_millis(self.child_deadline_ms),
            supervision_deadline: Duration::from_millis(self.child_supervision_deadline_ms),
        }
    }

    pub fn enforce_hard_ceilings(self) -> Result<(), ConfigError> {
        let pairs = [
            ("config_bytes", self.config_bytes as u64, HARD_LIMITS.config_bytes as u64),
            (
                "metadata_stdout_bytes",
                self.metadata_stdout_bytes as u64,
                HARD_LIMITS.metadata_stdout_bytes as u64,
            ),
            (
                "tool_executable_bytes",
                self.tool_executable_bytes,
                HARD_LIMITS.tool_executable_bytes,
            ),
            (
                "child_stderr_bytes",
                self.child_stderr_bytes as u64,
                HARD_LIMITS.child_stderr_bytes as u64,
            ),
            ("child_deadline_ms", self.child_deadline_ms, HARD_LIMITS.child_deadline_ms),
            (
                "child_supervision_deadline_ms",
                self.child_supervision_deadline_ms,
                HARD_LIMITS.child_supervision_deadline_ms,
            ),
            ("report_bytes", self.report_bytes as u64, HARD_LIMITS.report_bytes as u64),
            (
                "source_file_count",
                self.source_file_count as u64,
                HARD_LIMITS.source_file_count as u64,
            ),
            (
                "source_entry_count",
                self.source_entry_count as u64,
                HARD_LIMITS.source_entry_count as u64,
            ),
            (
                "directory_entry_count",
                self.directory_entry_count as u64,
                HARD_LIMITS.directory_entry_count as u64,
            ),
            ("source_total_bytes", self.source_total_bytes, HARD_LIMITS.source_total_bytes),
            ("source_file_bytes", self.source_file_bytes, HARD_LIMITS.source_file_bytes),
            ("path_bytes", self.path_bytes as u64, HARD_LIMITS.path_bytes as u64),
            (
                "syntax_item_expr_type_visits",
                self.syntax_item_expr_type_visits as u64,
                HARD_LIMITS.syntax_item_expr_type_visits as u64,
            ),
            ("findings", self.findings as u64, HARD_LIMITS.findings as u64),
            ("json_depth", self.json_depth as u64, HARD_LIMITS.json_depth as u64),
            (
                "json_container_items",
                self.json_container_items as u64,
                HARD_LIMITS.json_container_items as u64,
            ),
        ];
        for (name, value, hard) in pairs {
            if value > hard {
                return Err(ConfigError::new(
                    "TOPOLOGY_HARD_LIMIT",
                    json::child("$.limits", name),
                    format!("{value} exceeds checker hard ceiling {hard}"),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Profile {
    pub name: String,
    pub target: String,
    pub all_features: bool,
    pub no_default_features: bool,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Package {
    pub name: String,
    pub manifest: PathBuf,
    pub targets: Vec<Target>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
    pub name: String,
    pub kind: Vec<String>,
    pub crate_types: Vec<String>,
    pub src_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RepositoryEntrypoint {
    Script { path: PathBuf, content_digest: String },
    Workflow { path: PathBuf, content_digest: String },
    CargoTarget { package: String, target: String },
}

impl Topology {
    pub fn parse(value: &Value) -> Result<Self, ConfigError> {
        let object = json::object(
            value,
            "$",
            &[
                "schema",
                "workspace_manifest",
                "workspace_members",
                "manifests",
                "lockfiles",
                "cargo_config_files",
                "build_scripts",
                "custom_target_specs",
                "linker_assets",
                "limits",
                "profiles",
                "packages",
                "repository_entrypoints",
            ],
            &[],
        )?;
        if json::string(&object["schema"], "$.schema")? != SCHEMA {
            return Err(ConfigError::new("CFG_CONST", "$.schema", format!("expected {SCHEMA:?}")));
        }
        let workspace_manifest = relative_path(
            json::string(&object["workspace_manifest"], "$.workspace_manifest")?,
            "$.workspace_manifest",
            false,
        )?;
        let workspace_members = paths(&object["workspace_members"], "$.workspace_members")?;
        let manifests = paths(&object["manifests"], "$.manifests")?;
        let lockfiles = paths(&object["lockfiles"], "$.lockfiles")?;
        let cargo_config_files = paths(&object["cargo_config_files"], "$.cargo_config_files")?;
        let build_scripts = paths(&object["build_scripts"], "$.build_scripts")?;
        let custom_target_specs = paths(&object["custom_target_specs"], "$.custom_target_specs")?;
        let linker_assets = paths(&object["linker_assets"], "$.linker_assets")?;
        let limits = parse_limits(&object["limits"])?;
        let profiles = parse_profiles(&object["profiles"])?;
        let packages = parse_packages(&object["packages"])?;
        let repository_entrypoints = parse_entrypoints(&object["repository_entrypoints"])?;

        let topology = Self {
            workspace_manifest,
            workspace_members,
            manifests,
            lockfiles,
            cargo_config_files,
            build_scripts,
            custom_target_specs,
            linker_assets,
            limits,
            profiles,
            packages,
            repository_entrypoints,
        };
        topology.limits.enforce_hard_ceilings()?;
        topology.validate_relationships()?;
        Ok(topology)
    }

    fn validate_relationships(&self) -> Result<(), ConfigError> {
        if !self.manifests.contains(&self.workspace_manifest) {
            return Err(ConfigError::new(
                "TOPOLOGY_WORKSPACE_MANIFEST",
                "$.workspace_manifest",
                "workspace manifest is absent from manifests",
            ));
        }
        if !self.cargo_config_files.is_empty() {
            return Err(ConfigError::new(
                "TOPOLOGY_CARGO_CONFIG_SET",
                "$.cargo_config_files",
                "v1 requires no repository Cargo config files",
            ));
        }
        if self.lockfiles != [PathBuf::from("Cargo.lock")] {
            return Err(ConfigError::new(
                "TOPOLOGY_LOCKFILE_SET",
                "$.lockfiles",
                "v1 requires exactly the root Cargo.lock",
            ));
        }
        let package_manifests =
            self.packages.iter().map(|package| package.manifest.clone()).collect::<BTreeSet<_>>();
        let expected_manifests = std::iter::once(self.workspace_manifest.clone())
            .chain(package_manifests.iter().cloned())
            .collect::<BTreeSet<_>>();
        if expected_manifests != self.manifests.iter().cloned().collect() {
            return Err(ConfigError::new(
                "TOPOLOGY_MANIFEST_SET",
                "$.manifests",
                "manifests must equal the workspace manifest plus every package manifest",
            ));
        }
        let expected_members = self
            .packages
            .iter()
            .map(|package| {
                package.manifest.parent().unwrap_or_else(|| std::path::Path::new("")).to_path_buf()
            })
            .collect::<BTreeSet<_>>();
        if expected_members != self.workspace_members.iter().cloned().collect() {
            return Err(ConfigError::new(
                "TOPOLOGY_MEMBER_SET",
                "$.workspace_members",
                "workspace members must equal package manifest directories",
            ));
        }
        for entrypoint in &self.repository_entrypoints {
            if let RepositoryEntrypoint::CargoTarget { package, target } = entrypoint {
                let admitted = self.packages.iter().any(|candidate| {
                    candidate.name == *package
                        && candidate.targets.iter().any(|candidate| {
                            candidate.name == *target
                                && candidate.kind.iter().any(|kind| kind == "bin")
                        })
                });
                if !admitted {
                    return Err(ConfigError::new(
                        "TOPOLOGY_ENTRYPOINT_TARGET",
                        "$.repository_entrypoints",
                        format!(
                            "Cargo entrypoint {package}/{target} is not a declared binary target"
                        ),
                    ));
                }
            }
        }
        let declared_bins = self
            .repository_entrypoints
            .iter()
            .filter_map(|entrypoint| match entrypoint {
                RepositoryEntrypoint::CargoTarget { package, target } => {
                    Some((package.clone(), target.clone()))
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let topology_bins = self
            .packages
            .iter()
            .flat_map(|package| {
                package
                    .targets
                    .iter()
                    .filter(|target| target.kind.iter().any(|kind| kind == "bin"))
                    .map(|target| (package.name.clone(), target.name.clone()))
            })
            .collect::<BTreeSet<_>>();
        if declared_bins != topology_bins {
            return Err(ConfigError::new(
                "TOPOLOGY_CARGO_ENTRYPOINT_SET",
                "$.repository_entrypoints",
                "Cargo target entrypoints must equal every declared binary target",
            ));
        }
        Ok(())
    }
}

fn parse_entrypoints(value: &Value) -> Result<Vec<RepositoryEntrypoint>, ConfigError> {
    let values = json::array(value, "$.repository_entrypoints")?;
    if values.is_empty() {
        return Err(ConfigError::new(
            "TOPOLOGY_EMPTY_ENTRYPOINTS",
            "$.repository_entrypoints",
            "at least one repository entrypoint is required",
        ));
    }
    let mut result = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let path = json::index("$.repository_entrypoints", item);
        let kind = value.as_object().and_then(|object| object.get("kind")).ok_or_else(|| {
            ConfigError::new(
                "CFG_MISSING_FIELD",
                json::child(&path, "kind"),
                "required field is absent",
            )
        })?;
        let entrypoint = match json::string(kind, &json::child(&path, "kind"))? {
            "script" => {
                let object = json::object(value, &path, &["kind", "path", "content_digest"], &[])?;
                RepositoryEntrypoint::Script {
                    path: relative_path(
                        json::string(&object["path"], &json::child(&path, "path"))?,
                        &json::child(&path, "path"),
                        false,
                    )?,
                    content_digest: sha256(
                        json::string(
                            &object["content_digest"],
                            &json::child(&path, "content_digest"),
                        )?,
                        &json::child(&path, "content_digest"),
                    )?,
                }
            }
            "workflow" => {
                let object = json::object(value, &path, &["kind", "path", "content_digest"], &[])?;
                RepositoryEntrypoint::Workflow {
                    path: relative_path(
                        json::string(&object["path"], &json::child(&path, "path"))?,
                        &json::child(&path, "path"),
                        false,
                    )?,
                    content_digest: sha256(
                        json::string(
                            &object["content_digest"],
                            &json::child(&path, "content_digest"),
                        )?,
                        &json::child(&path, "content_digest"),
                    )?,
                }
            }
            "cargo_target" => {
                let object = json::object(value, &path, &["kind", "package", "target"], &[])?;
                RepositoryEntrypoint::CargoTarget {
                    package: nonempty(&object["package"], &json::child(&path, "package"))?,
                    target: nonempty(&object["target"], &json::child(&path, "target"))?,
                }
            }
            other => {
                return Err(ConfigError::new(
                    "TOPOLOGY_ENTRYPOINT_KIND",
                    json::child(&path, "kind"),
                    format!("unknown entrypoint kind {other:?}"),
                ));
            }
        };
        if !result.insert(entrypoint) {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_ENTRYPOINT",
                path,
                "duplicate repository entrypoint",
            ));
        }
    }
    Ok(result.into_iter().collect())
}

fn sha256(value: &str, path: &str) -> Result<String, ConfigError> {
    if value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(value.to_owned())
    } else {
        Err(ConfigError::new("TOPOLOGY_DIGEST", path, "expected sha256:<64 lowercase hex>"))
    }
}

fn parse_limits(value: &Value) -> Result<Limits, ConfigError> {
    const NAMES: &[&str] = &[
        "config_bytes",
        "metadata_stdout_bytes",
        "tool_executable_bytes",
        "child_stderr_bytes",
        "child_deadline_ms",
        "child_supervision_deadline_ms",
        "report_bytes",
        "source_file_count",
        "source_entry_count",
        "directory_entry_count",
        "source_total_bytes",
        "source_file_bytes",
        "path_bytes",
        "syntax_item_expr_type_visits",
        "findings",
        "json_depth",
        "json_container_items",
    ];
    let object = json::object(value, "$.limits", NAMES, &[])?;
    let positive = |name: &str| -> Result<u64, ConfigError> {
        let path = json::child("$.limits", name);
        let value = json::unsigned(&object[name], &path)?;
        if value == 0 {
            Err(ConfigError::new("TOPOLOGY_ZERO_LIMIT", path, "limit must be positive"))
        } else {
            Ok(value)
        }
    };
    let bounded = |name: &str| -> Result<usize, ConfigError> {
        usize::try_from(positive(name)?).map_err(|_| {
            ConfigError::new(
                "TOPOLOGY_LIMIT_RANGE",
                json::child("$.limits", name),
                "limit exceeds this host's usize",
            )
        })
    };
    let limits = Limits {
        config_bytes: bounded("config_bytes")?,
        metadata_stdout_bytes: bounded("metadata_stdout_bytes")?,
        tool_executable_bytes: positive("tool_executable_bytes")?,
        child_stderr_bytes: bounded("child_stderr_bytes")?,
        child_deadline_ms: positive("child_deadline_ms")?,
        child_supervision_deadline_ms: positive("child_supervision_deadline_ms")?,
        report_bytes: bounded("report_bytes")?,
        source_file_count: bounded("source_file_count")?,
        source_entry_count: bounded("source_entry_count")?,
        directory_entry_count: bounded("directory_entry_count")?,
        source_total_bytes: positive("source_total_bytes")?,
        source_file_bytes: positive("source_file_bytes")?,
        path_bytes: bounded("path_bytes")?,
        syntax_item_expr_type_visits: bounded("syntax_item_expr_type_visits")?,
        findings: bounded("findings")?,
        json_depth: bounded("json_depth")?,
        json_container_items: bounded("json_container_items")?,
    };
    if limits.child_supervision_deadline_ms <= limits.child_deadline_ms {
        return Err(ConfigError::new(
            "TOPOLOGY_SUPERVISION_DEADLINE",
            "$.limits.child_supervision_deadline_ms",
            "supervision deadline must be strictly greater than execution deadline",
        ));
    }
    Ok(limits)
}

fn parse_profiles(value: &Value) -> Result<Vec<Profile>, ConfigError> {
    let values = json::array(value, "$.profiles")?;
    if values.is_empty() {
        return Err(ConfigError::new(
            "TOPOLOGY_EMPTY_PROFILES",
            "$.profiles",
            "at least one profile is required",
        ));
    }
    let mut profiles = Vec::new();
    let mut names = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let path = json::index("$.profiles", item);
        let object = json::object(
            value,
            &path,
            &["name", "target", "all_features", "no_default_features", "features"],
            &[],
        )?;
        let name = nonempty(&object["name"], &json::child(&path, "name"))?;
        if !names.insert(name.clone()) {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_PROFILE",
                path,
                "duplicate profile name",
            ));
        }
        let target = nonempty(&object["target"], &json::child(&path, "target"))?;
        if target.bytes().any(|byte| byte.is_ascii_whitespace()) {
            return Err(ConfigError::new(
                "TOPOLOGY_TARGET_FORM",
                json::child(&path, "target"),
                "target must not contain whitespace",
            ));
        }
        let all_features =
            json::boolean(&object["all_features"], &json::child(&path, "all_features"))?;
        let no_default_features = json::boolean(
            &object["no_default_features"],
            &json::child(&path, "no_default_features"),
        )?;
        let features = strings(&object["features"], &json::child(&path, "features"), true)?;
        if all_features && (no_default_features || !features.is_empty()) {
            return Err(ConfigError::new(
                "TOPOLOGY_PROFILE_FEATURES",
                path,
                "all_features cannot be combined with no_default_features or explicit features",
            ));
        }
        profiles.push(Profile { name, target, all_features, no_default_features, features });
    }
    profiles.sort();
    Ok(profiles)
}

fn parse_packages(value: &Value) -> Result<Vec<Package>, ConfigError> {
    let values = json::array(value, "$.packages")?;
    if values.is_empty() {
        return Err(ConfigError::new(
            "TOPOLOGY_EMPTY_PACKAGES",
            "$.packages",
            "at least one package is required",
        ));
    }
    let mut packages = Vec::new();
    let mut names = BTreeSet::new();
    let mut manifests = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let path = json::index("$.packages", item);
        let object = json::object(value, &path, &["name", "manifest", "targets"], &[])?;
        let name = nonempty(&object["name"], &json::child(&path, "name"))?;
        let manifest = relative_path(
            json::string(&object["manifest"], &json::child(&path, "manifest"))?,
            &json::child(&path, "manifest"),
            false,
        )?;
        if !names.insert(name.clone()) || !manifests.insert(manifest.clone()) {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_PACKAGE",
                path,
                "duplicate package name or manifest",
            ));
        }
        let targets_path = json::child(&path, "targets");
        let targets = parse_targets(&object["targets"], &targets_path)?;
        packages.push(Package { name, manifest, targets });
    }
    packages.sort();
    Ok(packages)
}

fn parse_targets(value: &Value, path: &str) -> Result<Vec<Target>, ConfigError> {
    let values = json::array(value, path)?;
    if values.is_empty() {
        return Err(ConfigError::new(
            "TOPOLOGY_EMPTY_TARGETS",
            path,
            "at least one target is required",
        ));
    }
    let mut targets = Vec::new();
    let mut identities = BTreeSet::new();
    let mut sources = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let item_path = json::index(path, item);
        let object =
            json::object(value, &item_path, &["name", "kind", "crate_types", "src_path"], &[])?;
        let name = nonempty(&object["name"], &json::child(&item_path, "name"))?;
        let kind = strings(&object["kind"], &json::child(&item_path, "kind"), false)?;
        let crate_types =
            strings(&object["crate_types"], &json::child(&item_path, "crate_types"), false)?;
        let src_path = relative_path(
            json::string(&object["src_path"], &json::child(&item_path, "src_path"))?,
            &json::child(&item_path, "src_path"),
            false,
        )?;
        if !identities.insert((name.clone(), kind.clone(), crate_types.clone()))
            || !sources.insert(src_path.clone())
        {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_TARGET",
                item_path,
                "duplicate target identity or source path",
            ));
        }
        targets.push(Target { name, kind, crate_types, src_path });
    }
    targets.sort();
    Ok(targets)
}

fn paths(value: &Value, path: &str) -> Result<Vec<PathBuf>, ConfigError> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for (item, value) in json::array(value, path)?.iter().enumerate() {
        let item_path = json::index(path, item);
        let value = relative_path(json::string(value, &item_path)?, &item_path, false)?;
        if !seen.insert(value.clone()) {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_PATH",
                item_path,
                "duplicate normalized path",
            ));
        }
        result.push(value);
    }
    result.sort();
    Ok(result)
}

fn strings(value: &Value, path: &str, allow_empty_array: bool) -> Result<Vec<String>, ConfigError> {
    let values = json::array(value, path)?;
    if !allow_empty_array && values.is_empty() {
        return Err(ConfigError::new("TOPOLOGY_EMPTY_SET", path, "at least one value is required"));
    }
    let mut result = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let item_path = json::index(path, item);
        let value = nonempty(value, &item_path)?;
        if !result.insert(value) {
            return Err(ConfigError::new("TOPOLOGY_DUPLICATE_VALUE", item_path, "duplicate value"));
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

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    fn topology() -> Value {
        serde_json::from_str(include_str!("../../../conformance/workspace-topology.json")).unwrap()
    }

    #[test]
    fn profiles_are_closed_unique_and_feature_consistent() {
        let mut duplicate = topology();
        let first = duplicate["profiles"][0].clone();
        duplicate["profiles"].as_array_mut().unwrap().push(first);
        assert_eq!(
            super::Topology::parse(&duplicate).unwrap_err().code,
            "TOPOLOGY_DUPLICATE_PROFILE"
        );
        let mut invalid = topology();
        invalid["profiles"][0]["all_features"] = json!(true);
        invalid["profiles"][0]["features"] = json!(["x"]);
        assert_eq!(super::Topology::parse(&invalid).unwrap_err().code, "TOPOLOGY_PROFILE_FEATURES");
    }

    #[test]
    fn package_target_and_path_alias_mutants_fail() {
        let mut duplicate = topology();
        let target = duplicate["packages"][0]["targets"][0].clone();
        duplicate["packages"][0]["targets"].as_array_mut().unwrap().push(target);
        assert_eq!(
            super::Topology::parse(&duplicate).unwrap_err().code,
            "TOPOLOGY_DUPLICATE_TARGET"
        );
        let mut root_manifest = topology();
        root_manifest["packages"][0]["manifest"] = json!(".");
        assert_eq!(super::Topology::parse(&root_manifest).unwrap_err().code, "CFG_PATH_FORM");
    }

    #[test]
    fn lock_and_hard_limit_changes_fail_closed() {
        let mut nested_lock = topology();
        nested_lock["lockfiles"] = json!(["nested/Cargo.lock"]);
        assert_eq!(super::Topology::parse(&nested_lock).unwrap_err().code, "TOPOLOGY_LOCKFILE_SET");
        let mut oversized = topology();
        oversized["limits"]["source_file_count"] = json!(8193);
        assert_eq!(super::Topology::parse(&oversized).unwrap_err().code, "TOPOLOGY_HARD_LIMIT");
    }

    #[test]
    fn unknown_fields_are_rejected_by_hand_parser_too() {
        let mut value = topology();
        value.as_object_mut().unwrap().insert("surprise".to_owned(), json!(true));
        assert_eq!(super::Topology::parse(&value).unwrap_err().code, "CFG_UNKNOWN_FIELD");
    }

    #[test]
    fn binary_entrypoint_set_is_exact() {
        let mut missing = topology();
        missing["repository_entrypoints"]
            .as_array_mut()
            .unwrap()
            .retain(|entry| entry["kind"] != "cargo_target");
        assert_eq!(
            super::Topology::parse(&missing).unwrap_err().code,
            "TOPOLOGY_CARGO_ENTRYPOINT_SET"
        );
        let mut extra = topology();
        extra["repository_entrypoints"].as_array_mut().unwrap().push(json!({
            "kind":"cargo_target", "package":"reovim-depgraph", "target":"not-a-bin"
        }));
        assert_eq!(super::Topology::parse(&extra).unwrap_err().code, "TOPOLOGY_ENTRYPOINT_TARGET");
    }
}
