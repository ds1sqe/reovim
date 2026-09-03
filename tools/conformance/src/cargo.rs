//! Bounded Cargo metadata execution and format-v1 parsing.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ffi::OsString,
    path::{Path, PathBuf},
};

use serde_json::Value;

use crate::{
    compiler_capture::{
        RUSTC_LAUNCHER_LIBRARY, RUSTC_LAUNCHER_MODE, RUSTC_LAUNCHER_RUSTC,
    },
    digest,
    json::{self, ConfigError},
    process::{
        self, CommandSpec, ProcessLimits, ProcessResult, TerminationReason,
    },
    topology::Profile,
};

const METADATA_TARGET_DIRECTORY: &str = "target/cargo-metadata";

#[derive(Debug)]
pub struct Capture {
    pub process: ProcessResult,
    pub raw_digest: String,
    pub metadata: Metadata,
}

#[derive(Clone, Copy)]
struct RustcLaunch<'a> {
    launcher: &'a Path,
    library_path: &'a Path,
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
    pub edition: String,
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
    _root: &Path,
    cargo_home: &Path,
    limits: ProcessLimits,
) -> Result<VersionCapture, CargoError> {
    let spec = cargo_spec(cargo, rustc, cargo_home, ["--version", "--verbose"]);
    let process = process::run(&spec, limits).map_err(|error| CargoError {
        code: "CARGO_VERSION_RUN",
        message: error.to_string(),
    })?;
    validate_process("CARGO_VERSION", &process)?;
    let version = std::str::from_utf8(&process.stdout)
        .map_err(|error| CargoError {
            code: "CARGO_VERSION_UTF8",
            message: error.to_string(),
        })?
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
        environment: BTreeMap::from([(
            OsString::from("TERM"),
            OsString::from("dumb"),
        )]),
        clear_environment: true,
    };
    let process = process::run(&spec, limits).map_err(|error| CargoError {
        code: "RUSTC_VERSION_RUN",
        message: error.to_string(),
    })?;
    validate_process("RUSTC_VERSION", &process)?;
    let version = std::str::from_utf8(&process.stdout)
        .map_err(|error| CargoError {
            code: "RUSTC_VERSION_UTF8",
            message: error.to_string(),
        })?
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
    metadata_with_library_path(
        cargo,
        rustc,
        root,
        cargo_home,
        workspace_manifest,
        profile,
        None,
        None,
        limits,
        json_depth,
        json_items,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn metadata_with_library_path(
    cargo: &Path,
    rustc: &Path,
    root: &Path,
    cargo_home: &Path,
    workspace_manifest: &Path,
    profile: &Profile,
    rustc_launcher: Option<&Path>,
    library_path: Option<&Path>,
    limits: ProcessLimits,
    json_depth: usize,
    json_items: usize,
) -> Result<Capture, CargoError> {
    let launch = rustc_launch(rustc_launcher, library_path)?;
    let spec = metadata_spec(
        cargo,
        rustc,
        root,
        cargo_home,
        workspace_manifest,
        profile,
        launch,
    )?;
    let process = process::run(&spec, limits).map_err(|error| CargoError {
        code: "CARGO_METADATA_RUN",
        message: error.to_string(),
    })?;
    validate_process("CARGO_METADATA", &process)?;
    let raw_digest = digest::bytes(process.stdout.as_slice());
    let value = json::parse(&process.stdout, json_depth, json_items).map_err(
        |error| CargoError {
            code: "CARGO_METADATA_JSON",
            message: error.to_string(),
        },
    )?;
    let metadata = parse_metadata(&value).map_err(|error| CargoError {
        code: "CARGO_METADATA_SHAPE",
        message: error.to_string(),
    })?;
    Ok(Capture { process, raw_digest, metadata })
}

pub fn exact_metadata_arguments(
    repository_root: &Path,
    workspace_manifest: &Path,
    profile: &Profile,
) -> Result<Vec<String>, CargoError> {
    validate_metadata_selection(profile)?;
    let manifest = absolute_manifest_text(repository_root, workspace_manifest)?;
    let mut arguments = vec![
        "metadata".to_owned(),
        "--quiet".to_owned(),
        "--format-version".to_owned(),
        "1".to_owned(),
        "--manifest-path".to_owned(),
        manifest,
        "--locked".to_owned(),
        "--offline".to_owned(),
        "--filter-platform".to_owned(),
        profile.target.clone(),
    ];
    if profile.all_features {
        arguments.push("--all-features".to_owned());
    }
    if profile.no_default_features {
        arguments.push("--no-default-features".to_owned());
    }
    if !profile.features.is_empty() {
        arguments.push("--features".to_owned());
        arguments.push(profile.features.join(","));
    }
    Ok(arguments)
}

fn validate_metadata_selection(profile: &Profile) -> Result<(), CargoError> {
    if profile.target.is_empty()
        || profile.target.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(CargoError {
            code: "CARGO_METADATA_TARGET",
            message: "target triple must be nonempty without ASCII whitespace"
                .to_owned(),
        });
    }
    if profile.all_features
        && (profile.no_default_features || !profile.features.is_empty())
    {
        return Err(CargoError {
            code: "CARGO_METADATA_FEATURE_SELECTION",
            message: concat!(
                "all_features cannot be combined with no_default_features ",
                "or explicit features",
            )
            .to_owned(),
        });
    }
    let features = profile.features.iter().collect::<BTreeSet<_>>();
    let normalized = features.iter().copied().eq(profile.features.iter());
    let valid = profile.features.iter().all(|feature| {
        !feature.is_empty()
            && !feature.contains(',')
            && !feature.bytes().any(|byte| byte.is_ascii_whitespace())
    });
    if !normalized || !valid {
        return Err(CargoError {
            code: "CARGO_METADATA_FEATURE_SELECTION",
            message: concat!(
                "explicit features must be sorted, unique, nonempty, and ",
                "must not contain Cargo list separators",
            )
            .to_owned(),
        });
    }
    Ok(())
}

fn absolute_manifest_text(
    repository_root: &Path,
    workspace_manifest: &Path,
) -> Result<String, CargoError> {
    validate_repository_root(repository_root)?;
    let manifest = normalized_manifest_text(workspace_manifest)?;
    repository_root.join(manifest).into_os_string().into_string().map_err(
        |_| CargoError {
            code: "CARGO_METADATA_MANIFEST",
            message: "absolute workspace manifest path is not UTF-8".to_owned(),
        },
    )
}

fn validate_repository_root(root: &Path) -> Result<(), CargoError> {
    let text = root.to_str().ok_or_else(|| CargoError {
        code: "CARGO_METADATA_ROOT",
        message: "repository root is not UTF-8".to_owned(),
    })?;
    let valid = root.is_absolute()
        && (text == "/" || !text.ends_with('/'))
        && !text.contains(['\0', '\r', '\n', '\\'])
        && !text.contains("//")
        && !text.split('/').any(|component| matches!(component, "." | ".."));
    if valid {
        return Ok(());
    }
    Err(CargoError {
        code: "CARGO_METADATA_ROOT",
        message: "repository root must be normalized absolute UTF-8".to_owned(),
    })
}

fn normalized_manifest_text(path: &Path) -> Result<&str, CargoError> {
    let text = path.to_str().ok_or_else(|| CargoError {
        code: "CARGO_METADATA_MANIFEST",
        message: "workspace manifest path is not UTF-8".to_owned(),
    })?;
    let parsed =
        crate::policy::relative_path(text, "$.workspace_manifest", false);
    let has_dot_component = text.split('/').any(|part| part == ".");
    if parsed.as_ref().is_ok_and(|parsed| parsed == path) && !has_dot_component
    {
        return Ok(text);
    }
    Err(CargoError {
        code: "CARGO_METADATA_MANIFEST",
        message: concat!(
            "workspace manifest must be a normalized retained ",
            "repository-relative path",
        )
        .to_owned(),
    })
}

fn metadata_spec(
    cargo: &Path,
    rustc: &Path,
    root: &Path,
    cargo_home: &Path,
    workspace_manifest: &Path,
    profile: &Profile,
    launch: Option<RustcLaunch<'_>>,
) -> Result<CommandSpec, CargoError> {
    let arguments =
        exact_metadata_arguments(root, workspace_manifest, profile)?
            .into_iter()
            .map(OsString::from)
            .collect();
    let selected_rustc = launch.map_or(rustc, |value| value.launcher);
    let mut spec = cargo_spec_os(cargo, selected_rustc, cargo_home, arguments);
    spec.environment.insert(
        OsString::from("CARGO_TARGET_DIR"),
        root.join(METADATA_TARGET_DIRECTORY).into_os_string(),
    );
    configure_rustc_launcher(&mut spec, rustc, launch)?;
    Ok(spec)
}

fn rustc_launch<'a>(
    launcher: Option<&'a Path>,
    library_path: Option<&'a Path>,
) -> Result<Option<RustcLaunch<'a>>, CargoError> {
    match (launcher, library_path) {
        (None, None) => Ok(None),
        (Some(launcher), Some(library_path)) => {
            Ok(Some(RustcLaunch { launcher, library_path }))
        }
        _ => Err(CargoError {
            code: "CARGO_LIBRARY_PATH",
            message: "rustc launcher and library path must be paired"
                .to_owned(),
        }),
    }
}

fn configure_rustc_launcher(
    spec: &mut CommandSpec,
    rustc: &Path,
    launch: Option<RustcLaunch<'_>>,
) -> Result<(), CargoError> {
    if let Some(launch) = launch {
        validate_library_path(launch.library_path)?;
        validate_library_path(launch.launcher)?;
        spec.environment.insert(
            OsString::from("LD_LIBRARY_PATH"),
            launch.library_path.as_os_str().to_owned(),
        );
        spec.environment
            .insert(OsString::from(RUSTC_LAUNCHER_MODE), OsString::from("1"));
        spec.environment.insert(
            OsString::from(RUSTC_LAUNCHER_RUSTC),
            rustc.as_os_str().to_owned(),
        );
        spec.environment.insert(
            OsString::from(RUSTC_LAUNCHER_LIBRARY),
            launch.library_path.as_os_str().to_owned(),
        );
    }
    Ok(())
}

fn validate_library_path(path: &Path) -> Result<(), CargoError> {
    let text = path.to_str().ok_or_else(|| CargoError {
        code: "CARGO_LIBRARY_PATH",
        message: "compiler library path is not UTF-8".to_owned(),
    })?;
    let normalized = path.is_absolute()
        && path != Path::new("/")
        && !text.ends_with('/')
        && !text.contains(['\0', '\r', '\n', '\\', ':'])
        && !text.contains("//")
        && path.components().all(|part| {
            matches!(
                part,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        });
    if normalized {
        return Ok(());
    }
    Err(CargoError {
        code: "CARGO_LIBRARY_PATH",
        message: "compiler library path is not one normalized directory"
            .to_owned(),
    })
}

fn cargo_spec<'a>(
    cargo: &Path,
    rustc: &Path,
    cargo_home: &Path,
    arguments: impl IntoIterator<Item = &'a str>,
) -> CommandSpec {
    cargo_spec_os(
        cargo,
        rustc,
        cargo_home,
        arguments.into_iter().map(OsString::from).collect(),
    )
}

fn cargo_spec_os(
    cargo: &Path,
    rustc: &Path,
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

fn validate_process(
    prefix: &'static str,
    process: &ProcessResult,
) -> Result<(), CargoError> {
    if process.stdout_read_error.is_some()
        || process.stderr_read_error.is_some()
    {
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
            message: format!(
                "{prefix} exited with {}; {}",
                process.status,
                stderr_summary(process),
            ),
        });
    }
    if !process.stderr.is_empty() {
        return Err(CargoError {
            code: "CARGO_UNEXPECTED_STDERR",
            message: format!(
                "{prefix} emitted {} stderr bytes",
                process.stderr.len()
            ),
        });
    }
    Ok(())
}

fn stderr_summary(process: &ProcessResult) -> String {
    const LIMIT: usize = 512;

    let retained = process.stderr.get(..LIMIT).unwrap_or(&process.stderr);
    format!(
        "stderr_bytes={} stderr_digest={} stderr={}",
        process.stderr.len(),
        crate::digest::bytes(&process.stderr),
        String::from_utf8_lossy(retained),
    )
}

impl Metadata {
    /// Returns Cargo metadata's conservative resolved closure rooted at every
    /// workspace member. This is not compiler-unit activity evidence.
    #[must_use]
    pub fn resolved_package_ids(&self) -> BTreeSet<String> {
        let mut resolved = BTreeSet::new();
        let mut pending =
            self.workspace_members.iter().cloned().collect::<VecDeque<_>>();
        while let Some(id) = pending.pop_front() {
            self.resolve_package_id(&id, &mut resolved, &mut pending);
        }
        resolved
    }

    fn resolve_package_id(
        &self,
        id: &str,
        resolved: &mut BTreeSet<String>,
        pending: &mut VecDeque<String>,
    ) {
        if !resolved.insert(id.to_owned()) {
            return;
        }
        let Some(node) = self.nodes.get(id) else {
            return;
        };
        pending.extend(
            node.dependencies
                .iter()
                .map(|dependency| dependency.package.clone()),
        );
    }
}

fn parse_metadata(value: &Value) -> Result<Metadata, ConfigError> {
    let object = value.as_object().ok_or_else(|| {
        ConfigError::new("CARGO_TYPE", "$", "expected metadata object")
    })?;
    let version = object.get("version").ok_or_else(|| {
        ConfigError::new(
            "CARGO_MISSING",
            "$.version",
            "missing metadata version",
        )
    })?;
    if json::unsigned(version, "$.version")? != 1 {
        return Err(ConfigError::new(
            "CARGO_FORMAT_VERSION",
            "$.version",
            "only Cargo metadata format 1 is supported",
        ));
    }
    let workspace_root = PathBuf::from(json::string(
        required(object, "workspace_root", "$")?,
        "$.workspace_root",
    )?);
    let workspace_members = string_set(
        required(object, "workspace_members", "$")?,
        "$.workspace_members",
    )?;
    let mut packages = BTreeMap::new();
    for (item, value) in
        json::array(required(object, "packages", "$")?, "$.packages")?
            .iter()
            .enumerate()
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
    let resolve = resolve.as_object().ok_or_else(|| {
        ConfigError::new("CARGO_TYPE", "$.resolve", "expected resolve object")
    })?;
    let mut nodes = BTreeMap::new();
    for (item, value) in json::array(
        required(resolve, "nodes", "$.resolve")?,
        "$.resolve.nodes",
    )?
    .iter()
    .enumerate()
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
        validate_resolve_node(node, &packages, &nodes)?;
    }
    Ok(Metadata { workspace_root, workspace_members, packages, nodes })
}

fn validate_resolve_node(
    node: &Node,
    packages: &BTreeMap<String, Package>,
    nodes: &BTreeMap<String, Node>,
) -> Result<(), ConfigError> {
    if !packages.contains_key(&node.id) {
        return Err(ConfigError::new(
            "CARGO_NODE_PACKAGE",
            "$.resolve.nodes",
            format!("node {:?} lacks package", node.id),
        ));
    }
    for dependency in &node.dependencies {
        validate_resolve_edge(node, dependency, packages, nodes)?;
    }
    Ok(())
}

fn validate_resolve_edge(
    node: &Node,
    dependency: &ResolvedDependency,
    packages: &BTreeMap<String, Package>,
    nodes: &BTreeMap<String, Node>,
) -> Result<(), ConfigError> {
    if packages.contains_key(&dependency.package)
        && nodes.contains_key(&dependency.package)
    {
        return Ok(());
    }
    Err(ConfigError::new(
        "CARGO_EDGE_PACKAGE",
        "$.resolve.nodes",
        format!(
            "edge {} -> {} lacks package/node",
            node.id, dependency.package
        ),
    ))
}

fn parse_package(value: &Value, path: &str) -> Result<Package, ConfigError> {
    let object = value.as_object().ok_or_else(|| {
        ConfigError::new("CARGO_TYPE", path, "expected package object")
    })?;
    let id = nonempty(required(object, "id", path)?, &json::child(path, "id"))?;
    let name =
        nonempty(required(object, "name", path)?, &json::child(path, "name"))?;
    let version = nonempty(
        required(object, "version", path)?,
        &json::child(path, "version"),
    )?;
    let edition = nonempty(
        required(object, "edition", path)?,
        &json::child(path, "edition"),
    )?;
    let source =
        optional_string(object.get("source"), &json::child(path, "source"))?;
    let checksum = optional_string(
        object.get("checksum"),
        &json::child(path, "checksum"),
    )?;
    let license =
        optional_string(object.get("license"), &json::child(path, "license"))?;
    let manifest_path = PathBuf::from(json::string(
        required(object, "manifest_path", path)?,
        &json::child(path, "manifest_path"),
    )?);
    let targets_path = json::child(path, "targets");
    let mut targets = Vec::new();
    for (item, value) in
        json::array(required(object, "targets", path)?, &targets_path)?
            .iter()
            .enumerate()
    {
        targets.push(parse_target(value, &json::index(&targets_path, item))?);
    }
    targets.sort();
    Ok(Package {
        id,
        name,
        version,
        edition,
        source,
        checksum,
        license,
        manifest_path,
        targets,
    })
}

fn parse_target(value: &Value, path: &str) -> Result<Target, ConfigError> {
    let object = value.as_object().ok_or_else(|| {
        ConfigError::new("CARGO_TYPE", path, "expected target object")
    })?;
    Ok(Target {
        name: nonempty(
            required(object, "name", path)?,
            &json::child(path, "name"),
        )?,
        kind: string_set(
            required(object, "kind", path)?,
            &json::child(path, "kind"),
        )?
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
    let object = value.as_object().ok_or_else(|| {
        ConfigError::new("CARGO_TYPE", path, "expected node object")
    })?;
    let id = nonempty(required(object, "id", path)?, &json::child(path, "id"))?;
    let dependencies_path = json::child(path, "deps");
    let mut dependencies = Vec::new();
    for (item, value) in
        json::array(required(object, "deps", path)?, &dependencies_path)?
            .iter()
            .enumerate()
    {
        let item_path = json::index(&dependencies_path, item);
        let dependency = value.as_object().ok_or_else(|| {
            ConfigError::new(
                "CARGO_TYPE",
                &item_path,
                "expected dependency object",
            )
        })?;
        let name = nonempty(
            required(dependency, "name", &item_path)?,
            &json::child(&item_path, "name"),
        )?;
        let package = nonempty(
            required(dependency, "pkg", &item_path)?,
            &json::child(&item_path, "pkg"),
        )?;
        let kinds_path = json::child(&item_path, "dep_kinds");
        let mut kinds = Vec::new();
        for (kind_index, value) in json::array(
            required(dependency, "dep_kinds", &item_path)?,
            &kinds_path,
        )?
        .iter()
        .enumerate()
        {
            let kind_path = json::index(&kinds_path, kind_index);
            kinds.push(parse_dependency_kind(value, &kind_path)?);
        }
        kinds.sort();
        dependencies.push(ResolvedDependency { name, package, kinds });
    }
    dependencies.sort();
    Ok(Node { id, dependencies })
}

fn parse_dependency_kind(
    value: &Value,
    path: &str,
) -> Result<DependencyKind, ConfigError> {
    let object = value.as_object().ok_or_else(|| {
        ConfigError::new("CARGO_TYPE", path, "expected dependency-kind object")
    })?;
    let kind = normalized_dependency_kind(object.get("kind"), path)?;
    let target =
        optional_string(object.get("target"), &json::child(path, "target"))?;
    Ok(DependencyKind { kind, target })
}

fn normalized_dependency_kind(
    value: Option<&Value>,
    path: &str,
) -> Result<String, ConfigError> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok("normal".to_owned());
    };
    match json::string(value, &json::child(path, "kind"))? {
        "dev" => Ok("development".to_owned()),
        "build" => Ok("build".to_owned()),
        other => Err(ConfigError::new(
            "CARGO_DEPENDENCY_KIND",
            path,
            format!("unknown dependency kind {other:?}"),
        )),
    }
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

fn optional_string(
    value: Option<&Value>,
    path: &str,
) -> Result<Option<String>, ConfigError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(nonempty(value, path)?)),
    }
}

fn nonempty(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = json::string(value, path)?;
    if value.is_empty() {
        Err(ConfigError::new(
            "CARGO_EMPTY_STRING",
            path,
            "value must not be empty",
        ))
    } else {
        Ok(value.to_owned())
    }
}

fn string_set(
    value: &Value,
    path: &str,
) -> Result<BTreeSet<String>, ConfigError> {
    let mut result = BTreeSet::new();
    for (item, value) in json::array(value, path)?.iter().enumerate() {
        let item_path = json::index(path, item);
        let value = nonempty(value, &item_path)?;
        if !result.insert(value) {
            return Err(ConfigError::new(
                "CARGO_DUPLICATE_VALUE",
                item_path,
                "duplicate string",
            ));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::{OsStr, OsString},
        path::Path,
    };

    use super::{RustcLaunch, exact_metadata_arguments, metadata_spec};
    use crate::topology::Profile;

    #[test]
    fn package_edition_is_retained_and_required() {
        let mut value = serde_json::json!({
            "id": "path+file:///fixture#package@0.1.0",
            "name": "package",
            "version": "0.1.0",
            "edition": "2024",
            "source": null,
            "checksum": null,
            "license": null,
            "manifest_path": "/fixture/Cargo.toml",
            "targets": []
        });
        let package = super::parse_package(&value, "$.packages[0]").unwrap();
        assert_eq!(package.edition, "2024");

        value.as_object_mut().unwrap().remove("edition");
        let error = super::parse_package(&value, "$.packages[0]").unwrap_err();
        assert_eq!(error.code, "CARGO_MISSING");
    }

    #[test]
    fn metadata_arguments_use_retained_relative_manifest() {
        let profile = profile(false, false, &[]);
        let arguments = exact_metadata_arguments(
            Path::new("/checkout/reovim"),
            Path::new("nested/Cargo.toml"),
            &profile,
        )
        .unwrap();
        assert_eq!(
            arguments,
            [
                "metadata",
                "--quiet",
                "--format-version",
                "1",
                "--manifest-path",
                "/checkout/reovim/nested/Cargo.toml",
                "--locked",
                "--offline",
                "--filter-platform",
                "x86_64-unknown-linux-gnu",
            ]
        );
    }

    #[test]
    fn metadata_arguments_preserve_closed_feature_order() {
        let profile = profile(false, true, &["alloc", "std"]);
        let arguments = exact_metadata_arguments(
            Path::new("/checkout/reovim"),
            Path::new("Cargo.toml"),
            &profile,
        )
        .unwrap();
        assert_eq!(
            &arguments[10..],
            ["--no-default-features", "--features", "alloc,std"]
        );
    }

    #[test]
    fn metadata_arguments_reject_non_normal_manifest_paths() {
        let profile = profile(false, false, &[]);
        for path in [
            "",
            ".",
            "/Cargo.toml",
            "../Cargo.toml",
            "nested/../Cargo.toml",
            "./Cargo.toml",
            "nested/./Cargo.toml",
            "nested//Cargo.toml",
            "nested\\Cargo.toml",
            "C:Cargo.toml",
            "nested/Cargo.toml/",
        ] {
            let failure = exact_metadata_arguments(
                Path::new("/checkout/reovim"),
                Path::new(path),
                &profile,
            )
            .unwrap_err();
            assert_eq!(failure.code, "CARGO_METADATA_MANIFEST", "{path}");
        }
    }

    #[test]
    fn metadata_arguments_reject_invalid_roots_and_feature_combinations() {
        let baseline_profile = profile(false, false, &[]);
        for root in [
            "",
            ".",
            "checkout/reovim",
            "/checkout/../reovim",
            "/checkout/./reovim",
            "/checkout//reovim",
            "/checkout/reovim/",
        ] {
            let failure = exact_metadata_arguments(
                Path::new(root),
                Path::new("Cargo.toml"),
                &baseline_profile,
            )
            .unwrap_err();
            assert_eq!(failure.code, "CARGO_METADATA_ROOT", "{root}");
        }

        for profile in [
            profile(true, true, &[]),
            profile(true, false, &["alloc"]),
            profile(false, false, &[""]),
            profile(false, false, &["a,b"]),
            profile(false, false, &["a b"]),
            profile(false, false, &["std", "alloc"]),
            profile(false, false, &["std", "std"]),
        ] {
            let failure = exact_metadata_arguments(
                Path::new("/checkout/reovim"),
                Path::new("Cargo.toml"),
                &profile,
            )
            .unwrap_err();
            assert_eq!(failure.code, "CARGO_METADATA_FEATURE_SELECTION");
        }

        for target in ["", "x86_64 unknown", "x86_64\tunknown"] {
            let mut invalid = profile(false, false, &[]);
            invalid.target = target.to_owned();
            let failure = exact_metadata_arguments(
                Path::new("/checkout/reovim"),
                Path::new("Cargo.toml"),
                &invalid,
            )
            .unwrap_err();
            assert_eq!(failure.code, "CARGO_METADATA_TARGET", "{target}");
        }

        let supported = profile(
            false,
            true,
            &["_internal", "a-b", "dep:serde", "foo/bar", "foo?/bar"],
        );
        let arguments = exact_metadata_arguments(
            Path::new("/checkout/reovim"),
            Path::new("Cargo.toml"),
            &supported,
        )
        .unwrap();
        assert_eq!(
            arguments.last().map(String::as_str),
            Some("_internal,a-b,dep:serde,foo/bar,foo?/bar")
        );
    }

    #[test]
    fn metadata_spec_uses_fixed_root_for_config_search() {
        let root = Path::new("/checkout/reovim");
        let spec = metadata_spec(
            Path::new("/tools/cargo"),
            Path::new("/tools/rustc"),
            root,
            Path::new("/cargo-home"),
            Path::new("nested/Cargo.toml"),
            &profile(false, false, &[]),
            None,
        )
        .unwrap();
        assert_eq!(spec.current_dir, Path::new("/"));
        assert_eq!(
            spec.arguments[5],
            OsString::from("/checkout/reovim/nested/Cargo.toml")
        );
        assert_ne!(spec.current_dir, root);
        assert_eq!(
            spec.environment.get(OsStr::new("CARGO_TARGET_DIR")),
            Some(&OsString::from("/checkout/reovim/target/cargo-metadata"))
        );

        let prepared = metadata_spec(
            Path::new("/tools/cargo"),
            Path::new("/tools/rustc"),
            root,
            Path::new("/cargo-home"),
            Path::new("nested/Cargo.toml"),
            &profile(false, false, &[]),
            Some(RustcLaunch {
                launcher: Path::new("/checker/reovim-conformance"),
                library_path: Path::new("/compiler/lib"),
            }),
        )
        .unwrap();
        assert_eq!(
            prepared.environment.get(OsStr::new("LD_LIBRARY_PATH")),
            Some(&OsString::from("/compiler/lib"))
        );
        assert_eq!(
            prepared.environment.get(OsStr::new("RUSTC")),
            Some(&OsString::from("/checker/reovim-conformance"))
        );
        let failure = metadata_spec(
            Path::new("/tools/cargo"),
            Path::new("/tools/rustc"),
            root,
            Path::new("/cargo-home"),
            Path::new("nested/Cargo.toml"),
            &profile(false, false, &[]),
            Some(RustcLaunch {
                launcher: Path::new("/checker/reovim-conformance"),
                library_path: Path::new("/compiler/lib:/other"),
            }),
        )
        .unwrap_err();
        assert_eq!(failure.code, "CARGO_LIBRARY_PATH");
    }

    fn profile(
        all_features: bool,
        no_default_features: bool,
        features: &[&str],
    ) -> Profile {
        Profile {
            name: "stable".to_owned(),
            target: "x86_64-unknown-linux-gnu".to_owned(),
            all_features,
            no_default_features,
            features: features
                .iter()
                .map(|feature| (*feature).to_owned())
                .collect(),
        }
    }
}
