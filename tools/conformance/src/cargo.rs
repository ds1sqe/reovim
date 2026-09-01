//! Bounded Cargo metadata execution and format-v1 parsing.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ffi::OsString,
    path::{Path, PathBuf},
};

use serde_json::Value;

use crate::{
    digest,
    json::{self, ConfigError},
    process::{self, CommandSpec, ProcessLimits, ProcessResult, TerminationReason},
    topology::Profile,
};

#[derive(Debug)]
pub struct Capture {
    pub process: ProcessResult,
    pub raw_digest: String,
    pub metadata: Metadata,
}

#[derive(Debug)]
pub struct VersionCapture {
    pub process: ProcessResult,
    pub version: String,
    pub version_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub workspace_root: PathBuf,
    pub workspace_members: BTreeSet<String>,
    pub packages: BTreeMap<String, Package>,
    pub nodes: BTreeMap<String, Node>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: Option<String>,
    pub checksum: Option<String>,
    pub license: Option<String>,
    pub manifest_path: PathBuf,
    pub targets: Vec<Target>,
}

impl Package {
    #[must_use]
    pub fn review_identity(&self) -> String {
        self.source.as_ref().map_or_else(
            || format!("{} {} (workspace)", self.name, self.version),
            |source| format!("{} {} ({source})", self.name, self.version),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
    pub name: String,
    pub kind: Vec<String>,
    pub crate_types: Vec<String>,
    pub src_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub dependencies: Vec<ResolvedDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResolvedDependency {
    pub name: String,
    pub package: String,
    pub kinds: Vec<DependencyKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyKind {
    pub kind: String,
    pub target: Option<String>,
}

#[derive(Debug)]
pub struct CargoError {
    pub code: &'static str,
    pub message: String,
}

impl std::fmt::Display for CargoError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CargoError {}

pub fn version(
    cargo: &Path,
    rustc: &Path,
    root: &Path,
    cargo_home: &Path,
    limits: ProcessLimits,
) -> Result<VersionCapture, CargoError> {
    let spec = cargo_spec(cargo, rustc, root, cargo_home, ["--version", "--verbose"]);
    let process = process::run(&spec, limits)
        .map_err(|error| CargoError { code: "CARGO_VERSION_RUN", message: error.to_string() })?;
    validate_process("CARGO_VERSION", &process)?;
    let version = std::str::from_utf8(&process.stdout)
        .map_err(|error| CargoError { code: "CARGO_VERSION_UTF8", message: error.to_string() })?
        .trim_end()
        .to_owned();
    if version.is_empty() {
        return Err(CargoError {
            code: "CARGO_VERSION_EMPTY",
            message: "Cargo version output was empty".to_owned(),
        });
    }
    let version_digest = digest::bytes(process.stdout.as_slice());
    Ok(VersionCapture { process, version, version_digest })
}

pub fn rustc_version(
    rustc: &Path,
    root: &Path,
    limits: ProcessLimits,
) -> Result<VersionCapture, CargoError> {
    let spec = CommandSpec {
        program: rustc.to_path_buf(),
        arguments: vec!["--version".into(), "--verbose".into()],
        current_dir: root.to_path_buf(),
        environment: BTreeMap::from([(OsString::from("TERM"), OsString::from("dumb"))]),
        clear_environment: true,
    };
    let process = process::run(&spec, limits)
        .map_err(|error| CargoError { code: "RUSTC_VERSION_RUN", message: error.to_string() })?;
    validate_process("RUSTC_VERSION", &process)?;
    let version = std::str::from_utf8(&process.stdout)
        .map_err(|error| CargoError { code: "RUSTC_VERSION_UTF8", message: error.to_string() })?
        .trim_end()
        .to_owned();
    if version.is_empty() {
        return Err(CargoError {
            code: "RUSTC_VERSION_EMPTY",
            message: "Rustc version output was empty".to_owned(),
        });
    }
    let version_digest = digest::bytes(process.stdout.as_slice());
    Ok(VersionCapture { process, version, version_digest })
}

#[allow(clippy::too_many_arguments)]
pub fn metadata(
    cargo: &Path,
    rustc: &Path,
    root: &Path,
    cargo_home: &Path,
    workspace_manifest: &Path,
    profile: &Profile,
    limits: ProcessLimits,
    json_depth: usize,
    json_items: usize,
) -> Result<Capture, CargoError> {
    let manifest = root.join(workspace_manifest);
    let mut arguments = vec![
        "metadata".into(),
        "--quiet".into(),
        "--format-version".into(),
        "1".into(),
        "--manifest-path".into(),
        manifest.into_os_string(),
        "--locked".into(),
        "--offline".into(),
        "--filter-platform".into(),
        profile.target.clone().into(),
    ];
    if profile.all_features {
        arguments.push("--all-features".into());
    }
    if profile.no_default_features {
        arguments.push("--no-default-features".into());
    }
    if !profile.features.is_empty() {
        arguments.push("--features".into());
        arguments.push(profile.features.join(",").into());
    }
    let spec = cargo_spec_os(cargo, rustc, root, cargo_home, arguments);
    let process = process::run(&spec, limits)
        .map_err(|error| CargoError { code: "CARGO_METADATA_RUN", message: error.to_string() })?;
    validate_process("CARGO_METADATA", &process)?;
    let raw_digest = digest::bytes(process.stdout.as_slice());
    let value = json::parse(&process.stdout, json_depth, json_items)
        .map_err(|error| CargoError { code: "CARGO_METADATA_JSON", message: error.to_string() })?;
    let metadata = parse_metadata(&value)
        .map_err(|error| CargoError { code: "CARGO_METADATA_SHAPE", message: error.to_string() })?;
    Ok(Capture { process, raw_digest, metadata })
}

fn cargo_spec<'a>(
    cargo: &Path,
    rustc: &Path,
    root: &Path,
    cargo_home: &Path,
    arguments: impl IntoIterator<Item = &'a str>,
) -> CommandSpec {
    cargo_spec_os(
        cargo,
        rustc,
        root,
        cargo_home,
        arguments.into_iter().map(OsString::from).collect(),
    )
}

fn cargo_spec_os(
    cargo: &Path,
    rustc: &Path,
    _root: &Path,
    cargo_home: &Path,
    arguments: Vec<OsString>,
) -> CommandSpec {
    CommandSpec {
        program: cargo.to_path_buf(),
        arguments,
        current_dir: PathBuf::from("/"),
        environment: BTreeMap::from([
            (OsString::from("CARGO_HOME"), cargo_home.as_os_str().to_owned()),
            (OsString::from("CARGO_NET_OFFLINE"), OsString::from("true")),
            (OsString::from("RUSTC"), rustc.as_os_str().to_owned()),
            (OsString::from("TERM"), OsString::from("dumb")),
        ]),
        clear_environment: true,
    }
}

fn validate_process(prefix: &'static str, process: &ProcessResult) -> Result<(), CargoError> {
    if process.stdout_read_error.is_some() || process.stderr_read_error.is_some() {
        return Err(CargoError {
            code: "CARGO_PIPE_READ",
            message: format!(
                "stdout={:?}, stderr={:?}",
                process.stdout_read_error, process.stderr_read_error
            ),
        });
    }
    if process.termination_reason != TerminationReason::Exited
        || process.stdout_overflow
        || process.stderr_overflow
    {
        return Err(CargoError {
            code: "CARGO_PROCESS_BOUND",
            message: format!(
                "{prefix} stopped by {} (stdout {}, stderr {})",
                process.termination_reason.as_str(),
                process.stdout_bytes,
                process.stderr_bytes
            ),
        });
    }
    if !process.status.success() {
        return Err(CargoError {
            code: "CARGO_NONZERO",
            message: format!("{prefix} exited with {}", process.status),
        });
    }
    if !process.stderr.is_empty() {
        return Err(CargoError {
            code: "CARGO_UNEXPECTED_STDERR",
            message: format!("{prefix} emitted {} stderr bytes", process.stderr.len()),
        });
    }
    Ok(())
}

impl Metadata {
    /// Returns Cargo metadata's conservative resolved closure rooted at every
    /// workspace member. This is not compiler-unit activity evidence.
    #[must_use]
    pub fn resolved_package_ids(&self) -> BTreeSet<String> {
        let mut resolved = BTreeSet::new();
        let mut pending = self.workspace_members.iter().cloned().collect::<VecDeque<_>>();
        while let Some(id) = pending.pop_front() {
            if !resolved.insert(id.clone()) {
                continue;
            }
            if let Some(node) = self.nodes.get(&id) {
                pending
                    .extend(node.dependencies.iter().map(|dependency| dependency.package.clone()));
            }
        }
        resolved
    }
}

fn parse_metadata(value: &Value) -> Result<Metadata, ConfigError> {
    let object = value
        .as_object()
        .ok_or_else(|| ConfigError::new("CARGO_TYPE", "$", "expected metadata object"))?;
    let version = object.get("version").ok_or_else(|| {
        ConfigError::new("CARGO_MISSING", "$.version", "missing metadata version")
    })?;
    if json::unsigned(version, "$.version")? != 1 {
        return Err(ConfigError::new(
            "CARGO_FORMAT_VERSION",
            "$.version",
            "only Cargo metadata format 1 is supported",
        ));
    }
    let workspace_root =
        PathBuf::from(json::string(required(object, "workspace_root", "$")?, "$.workspace_root")?);
    let workspace_members =
        string_set(required(object, "workspace_members", "$")?, "$.workspace_members")?;
    let mut packages = BTreeMap::new();
    for (item, value) in
        json::array(required(object, "packages", "$")?, "$.packages")?.iter().enumerate()
    {
        let package = parse_package(value, &json::index("$.packages", item))?;
        if packages.insert(package.id.clone(), package).is_some() {
            return Err(ConfigError::new(
                "CARGO_DUPLICATE_PACKAGE_ID",
                json::index("$.packages", item),
                "duplicate package id",
            ));
        }
    }
    let resolve = required(object, "resolve", "$")?;
    let resolve = resolve
        .as_object()
        .ok_or_else(|| ConfigError::new("CARGO_TYPE", "$.resolve", "expected resolve object"))?;
    let mut nodes = BTreeMap::new();
    for (item, value) in
        json::array(required(resolve, "nodes", "$.resolve")?, "$.resolve.nodes")?.iter().enumerate()
    {
        let node = parse_node(value, &json::index("$.resolve.nodes", item))?;
        if nodes.insert(node.id.clone(), node).is_some() {
            return Err(ConfigError::new(
                "CARGO_DUPLICATE_NODE_ID",
                json::index("$.resolve.nodes", item),
                "duplicate resolve node id",
            ));
        }
    }
    for id in &workspace_members {
        if !packages.contains_key(id) || !nodes.contains_key(id) {
            return Err(ConfigError::new(
                "CARGO_WORKSPACE_MEMBER",
                "$.workspace_members",
                format!("member {id:?} lacks package or resolve node"),
            ));
        }
    }
    for node in nodes.values() {
        if !packages.contains_key(&node.id) {
            return Err(ConfigError::new(
                "CARGO_NODE_PACKAGE",
                "$.resolve.nodes",
                format!("node {:?} lacks package", node.id),
            ));
        }
        for dependency in &node.dependencies {
            if !packages.contains_key(&dependency.package)
                || !nodes.contains_key(&dependency.package)
            {
                return Err(ConfigError::new(
                    "CARGO_EDGE_PACKAGE",
                    "$.resolve.nodes",
                    format!("edge {} -> {} lacks package/node", node.id, dependency.package),
                ));
            }
        }
    }
    Ok(Metadata { workspace_root, workspace_members, packages, nodes })
}

fn parse_package(value: &Value, path: &str) -> Result<Package, ConfigError> {
    let object = value
        .as_object()
        .ok_or_else(|| ConfigError::new("CARGO_TYPE", path, "expected package object"))?;
    let id = nonempty(required(object, "id", path)?, &json::child(path, "id"))?;
    let name = nonempty(required(object, "name", path)?, &json::child(path, "name"))?;
    let version = nonempty(required(object, "version", path)?, &json::child(path, "version"))?;
    let source = optional_string(object.get("source"), &json::child(path, "source"))?;
    let checksum = optional_string(object.get("checksum"), &json::child(path, "checksum"))?;
    let license = optional_string(object.get("license"), &json::child(path, "license"))?;
    let manifest_path = PathBuf::from(json::string(
        required(object, "manifest_path", path)?,
        &json::child(path, "manifest_path"),
    )?);
    let targets_path = json::child(path, "targets");
    let mut targets = Vec::new();
    for (item, value) in
        json::array(required(object, "targets", path)?, &targets_path)?.iter().enumerate()
    {
        targets.push(parse_target(value, &json::index(&targets_path, item))?);
    }
    targets.sort();
    Ok(Package { id, name, version, source, checksum, license, manifest_path, targets })
}

fn parse_target(value: &Value, path: &str) -> Result<Target, ConfigError> {
    let object = value
        .as_object()
        .ok_or_else(|| ConfigError::new("CARGO_TYPE", path, "expected target object"))?;
    Ok(Target {
        name: nonempty(required(object, "name", path)?, &json::child(path, "name"))?,
        kind: string_set(required(object, "kind", path)?, &json::child(path, "kind"))?
            .into_iter()
            .collect(),
        crate_types: string_set(
            required(object, "crate_types", path)?,
            &json::child(path, "crate_types"),
        )?
        .into_iter()
        .collect(),
        src_path: PathBuf::from(json::string(
            required(object, "src_path", path)?,
            &json::child(path, "src_path"),
        )?),
    })
}

fn parse_node(value: &Value, path: &str) -> Result<Node, ConfigError> {
    let object = value
        .as_object()
        .ok_or_else(|| ConfigError::new("CARGO_TYPE", path, "expected node object"))?;
    let id = nonempty(required(object, "id", path)?, &json::child(path, "id"))?;
    let dependencies_path = json::child(path, "deps");
    let mut dependencies = Vec::new();
    for (item, value) in
        json::array(required(object, "deps", path)?, &dependencies_path)?.iter().enumerate()
    {
        let item_path = json::index(&dependencies_path, item);
        let dependency = value.as_object().ok_or_else(|| {
            ConfigError::new("CARGO_TYPE", &item_path, "expected dependency object")
        })?;
        let name =
            nonempty(required(dependency, "name", &item_path)?, &json::child(&item_path, "name"))?;
        let package =
            nonempty(required(dependency, "pkg", &item_path)?, &json::child(&item_path, "pkg"))?;
        let kinds_path = json::child(&item_path, "dep_kinds");
        let mut kinds = Vec::new();
        for (kind_index, value) in
            json::array(required(dependency, "dep_kinds", &item_path)?, &kinds_path)?
                .iter()
                .enumerate()
        {
            let kind_path = json::index(&kinds_path, kind_index);
            let object = value.as_object().ok_or_else(|| {
                ConfigError::new("CARGO_TYPE", &kind_path, "expected dependency-kind object")
            })?;
            let kind = match object.get("kind") {
                None | Some(Value::Null) => "normal".to_owned(),
                Some(value) => match json::string(value, &json::child(&kind_path, "kind"))? {
                    "dev" => "development".to_owned(),
                    "build" => "build".to_owned(),
                    other => {
                        return Err(ConfigError::new(
                            "CARGO_DEPENDENCY_KIND",
                            kind_path,
                            format!("unknown dependency kind {other:?}"),
                        ));
                    }
                },
            };
            let target = optional_string(object.get("target"), &json::child(&kind_path, "target"))?;
            kinds.push(DependencyKind { kind, target });
        }
        kinds.sort();
        dependencies.push(ResolvedDependency { name, package, kinds });
    }
    dependencies.sort();
    Ok(Node { id, dependencies })
}

fn required<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    path: &str,
) -> Result<&'a Value, ConfigError> {
    object.get(name).ok_or_else(|| {
        ConfigError::new(
            "CARGO_MISSING",
            json::child(path, name),
            "required Cargo metadata field is absent",
        )
    })
}

fn optional_string(value: Option<&Value>, path: &str) -> Result<Option<String>, ConfigError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(nonempty(value, path)?)),
    }
}

fn nonempty(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = json::string(value, path)?;
    if value.is_empty() {
        Err(ConfigError::new("CARGO_EMPTY_STRING", path, "value must not be empty"))
    } else {
        Ok(value.to_owned())
    }
}

fn string_set(value: &Value, path: &str) -> Result<BTreeSet<String>, ConfigError> {
    let mut result = BTreeSet::new();
    for (item, value) in json::array(value, path)?.iter().enumerate() {
        let item_path = json::index(path, item);
        let value = nonempty(value, &item_path)?;
        if !result.insert(value) {
            return Err(ConfigError::new("CARGO_DUPLICATE_VALUE", item_path, "duplicate string"));
        }
    }
    Ok(result)
}
