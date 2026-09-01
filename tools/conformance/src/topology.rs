//! Parser for the committed exact multi-workspace topology and safety limits.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::Duration,
};

use serde_json::{Map, Value};

use crate::{
    json::{self, ConfigError},
    policy::relative_path,
    process::ProcessLimits,
};

pub const SCHEMA: &str = "reovim-workspace-topology/v2";

/// Bootstrap ceilings enforced before a topology file can select its own
/// equal-or-lower committed maxima.
pub const HARD_LIMITS: Limits = Limits {
    config_bytes: 1_048_576,
    metadata_stdout_bytes: 16_777_216,
    tool_executable_bytes: 100_663_296,
    child_stderr_bytes: 1_048_576,
    child_deadline_ms: 120_000,
    child_supervision_deadline_ms: 130_000,
    report_bytes: 67_108_864,
    source_file_count: 16_384,
    source_entry_count: 32_768,
    directory_entry_count: 16_384,
    source_total_bytes: 134_217_728,
    source_file_bytes: 8_388_608,
    path_bytes: 4_096,
    syntax_item_expr_type_visits: 2_000_000,
    findings: 8_192,
    json_depth: 256,
    json_container_items: 1_000_000,
    product_execution: ProductExecutionLimits {
        scratch_bytes: 536_870_912,
        scratch_inodes: 32_768,
        process_address_space_bytes: 4_294_967_296,
        process_cpu_seconds: 120,
        process_open_files: 256,
        process_output_file_bytes: 268_435_456,
        core_dump_bytes: 0,
        exposure_entry_count: 8_192,
        compiler_substrate_entry_count: 8_192,
        compiler_substrate_total_bytes: 1_073_741_824,
        cargo_json_bytes: 16_777_216,
        cargo_message_count: 4_096,
        wrapper_trace_bytes: 16_777_216,
        wrapper_argv_items: 4_096,
        wrapper_argv_bytes: 1_048_576,
        compiler_unit_count: 256,
        compiler_input_count: 8_192,
        dep_info_file_bytes: 4_194_304,
        dep_info_total_bytes: 67_108_864,
        artifact_count: 4_096,
        artifact_total_bytes: 536_870_912,
        package_list_bytes: 4_194_304,
        package_member_count: 8_192,
        public_item_count: 4_096,
        environment_dependency_count: 1_024,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topology {
    /// Canonical v2 model. Every nested collection is keyed by its declared ID.
    pub workspaces: BTreeMap<String, Workspace>,
    pub limits: Limits,
    pub repository_entrypoints: Vec<RepositoryEntrypoint>,

    // Compatibility projection for the current single root-workspace callers.
    // Multi-workspace execution must select `workspaces` explicitly rather
    // than treating these fields as a complete execution plan.
    pub workspace_manifest: PathBuf,
    pub workspace_members: Vec<PathBuf>,
    pub manifests: Vec<PathBuf>,
    pub lockfiles: Vec<PathBuf>,
    pub cargo_config_files: Vec<PathBuf>,
    pub build_scripts: Vec<PathBuf>,
    pub custom_target_specs: Vec<PathBuf>,
    pub linker_assets: Vec<PathBuf>,
    pub profiles: Vec<Profile>,
    pub packages: Vec<Package>,
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
    pub product_execution: ProductExecutionLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductExecutionLimits {
    pub scratch_bytes: u32,
    pub scratch_inodes: u16,
    pub process_address_space_bytes: u64,
    pub process_cpu_seconds: u8,
    pub process_open_files: u16,
    pub process_output_file_bytes: u32,
    pub core_dump_bytes: u8,
    pub exposure_entry_count: u16,
    pub compiler_substrate_entry_count: u16,
    pub compiler_substrate_total_bytes: u32,
    pub cargo_json_bytes: u32,
    pub cargo_message_count: u16,
    pub wrapper_trace_bytes: u32,
    pub wrapper_argv_items: usize,
    pub wrapper_argv_bytes: u32,
    pub compiler_unit_count: usize,
    pub compiler_input_count: u16,
    pub dep_info_file_bytes: u32,
    pub dep_info_total_bytes: u32,
    pub artifact_count: u16,
    pub artifact_total_bytes: u32,
    pub package_list_bytes: u32,
    pub package_member_count: usize,
    pub public_item_count: usize,
    pub environment_dependency_count: usize,
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

    #[allow(clippy::too_many_lines)]
    pub fn enforce_hard_ceilings(self) -> Result<(), ConfigError> {
        let product = self.product_execution;
        let hard_product = HARD_LIMITS.product_execution;
        let pairs = [
            (
                "$.limits.configuration.config_bytes",
                self.config_bytes as u64,
                HARD_LIMITS.config_bytes as u64,
            ),
            (
                "$.limits.configuration.json_depth",
                self.json_depth as u64,
                HARD_LIMITS.json_depth as u64,
            ),
            (
                "$.limits.configuration.json_container_items",
                self.json_container_items as u64,
                HARD_LIMITS.json_container_items as u64,
            ),
            (
                "$.limits.configuration.report_bytes",
                self.report_bytes as u64,
                HARD_LIMITS.report_bytes as u64,
            ),
            ("$.limits.configuration.findings", self.findings as u64, HARD_LIMITS.findings as u64),
            (
                "$.limits.inventory.source_entry_count",
                self.source_entry_count as u64,
                HARD_LIMITS.source_entry_count as u64,
            ),
            (
                "$.limits.inventory.directory_entry_count",
                self.directory_entry_count as u64,
                HARD_LIMITS.directory_entry_count as u64,
            ),
            (
                "$.limits.inventory.source_file_count",
                self.source_file_count as u64,
                HARD_LIMITS.source_file_count as u64,
            ),
            (
                "$.limits.inventory.source_total_bytes",
                self.source_total_bytes,
                HARD_LIMITS.source_total_bytes,
            ),
            (
                "$.limits.inventory.source_file_bytes",
                self.source_file_bytes,
                HARD_LIMITS.source_file_bytes,
            ),
            (
                "$.limits.inventory.path_bytes",
                self.path_bytes as u64,
                HARD_LIMITS.path_bytes as u64,
            ),
            (
                "$.limits.inventory.syntax_item_expr_type_visits",
                self.syntax_item_expr_type_visits as u64,
                HARD_LIMITS.syntax_item_expr_type_visits as u64,
            ),
            (
                "$.limits.process.stdout_bytes",
                self.metadata_stdout_bytes as u64,
                HARD_LIMITS.metadata_stdout_bytes as u64,
            ),
            (
                "$.limits.process.stderr_bytes",
                self.child_stderr_bytes as u64,
                HARD_LIMITS.child_stderr_bytes as u64,
            ),
            (
                "$.limits.process.execution_deadline_ms",
                self.child_deadline_ms,
                HARD_LIMITS.child_deadline_ms,
            ),
            (
                "$.limits.process.supervision_deadline_ms",
                self.child_supervision_deadline_ms,
                HARD_LIMITS.child_supervision_deadline_ms,
            ),
            (
                "$.limits.process.tool_executable_bytes",
                self.tool_executable_bytes,
                HARD_LIMITS.tool_executable_bytes,
            ),
            (
                "$.limits.product_execution.scratch_bytes",
                u64::from(product.scratch_bytes),
                u64::from(hard_product.scratch_bytes),
            ),
            (
                "$.limits.product_execution.scratch_inodes",
                u64::from(product.scratch_inodes),
                u64::from(hard_product.scratch_inodes),
            ),
            (
                "$.limits.product_execution.process_address_space_bytes",
                product.process_address_space_bytes,
                hard_product.process_address_space_bytes,
            ),
            (
                "$.limits.product_execution.process_cpu_seconds",
                u64::from(product.process_cpu_seconds),
                u64::from(hard_product.process_cpu_seconds),
            ),
            (
                "$.limits.product_execution.process_open_files",
                u64::from(product.process_open_files),
                u64::from(hard_product.process_open_files),
            ),
            (
                "$.limits.product_execution.process_output_file_bytes",
                u64::from(product.process_output_file_bytes),
                u64::from(hard_product.process_output_file_bytes),
            ),
            (
                "$.limits.product_execution.core_dump_bytes",
                u64::from(product.core_dump_bytes),
                u64::from(hard_product.core_dump_bytes),
            ),
            (
                "$.limits.product_execution.exposure_entry_count",
                u64::from(product.exposure_entry_count),
                u64::from(hard_product.exposure_entry_count),
            ),
            (
                "$.limits.product_execution.compiler_substrate_entry_count",
                u64::from(product.compiler_substrate_entry_count),
                u64::from(hard_product.compiler_substrate_entry_count),
            ),
            (
                "$.limits.product_execution.compiler_substrate_total_bytes",
                u64::from(product.compiler_substrate_total_bytes),
                u64::from(hard_product.compiler_substrate_total_bytes),
            ),
            (
                "$.limits.product_execution.cargo_json_bytes",
                u64::from(product.cargo_json_bytes),
                u64::from(hard_product.cargo_json_bytes),
            ),
            (
                "$.limits.product_execution.cargo_message_count",
                u64::from(product.cargo_message_count),
                u64::from(hard_product.cargo_message_count),
            ),
            (
                "$.limits.product_execution.wrapper_trace_bytes",
                u64::from(product.wrapper_trace_bytes),
                u64::from(hard_product.wrapper_trace_bytes),
            ),
            (
                "$.limits.product_execution.wrapper_argv_items",
                product.wrapper_argv_items as u64,
                hard_product.wrapper_argv_items as u64,
            ),
            (
                "$.limits.product_execution.wrapper_argv_bytes",
                u64::from(product.wrapper_argv_bytes),
                u64::from(hard_product.wrapper_argv_bytes),
            ),
            (
                "$.limits.product_execution.compiler_unit_count",
                product.compiler_unit_count as u64,
                hard_product.compiler_unit_count as u64,
            ),
            (
                "$.limits.product_execution.compiler_input_count",
                u64::from(product.compiler_input_count),
                u64::from(hard_product.compiler_input_count),
            ),
            (
                "$.limits.product_execution.dep_info_file_bytes",
                u64::from(product.dep_info_file_bytes),
                u64::from(hard_product.dep_info_file_bytes),
            ),
            (
                "$.limits.product_execution.dep_info_total_bytes",
                u64::from(product.dep_info_total_bytes),
                u64::from(hard_product.dep_info_total_bytes),
            ),
            (
                "$.limits.product_execution.artifact_count",
                u64::from(product.artifact_count),
                u64::from(hard_product.artifact_count),
            ),
            (
                "$.limits.product_execution.artifact_total_bytes",
                u64::from(product.artifact_total_bytes),
                u64::from(hard_product.artifact_total_bytes),
            ),
            (
                "$.limits.product_execution.package_list_bytes",
                u64::from(product.package_list_bytes),
                u64::from(hard_product.package_list_bytes),
            ),
            (
                "$.limits.product_execution.package_member_count",
                product.package_member_count as u64,
                hard_product.package_member_count as u64,
            ),
            (
                "$.limits.product_execution.public_item_count",
                product.public_item_count as u64,
                hard_product.public_item_count as u64,
            ),
            (
                "$.limits.product_execution.environment_dependency_count",
                product.environment_dependency_count as u64,
                hard_product.environment_dependency_count as u64,
            ),
        ];
        for (path, value, hard) in pairs {
            if value > hard {
                return Err(ConfigError::new(
                    "TOPOLOGY_HARD_LIMIT",
                    path,
                    format!("{value} exceeds checker hard ceiling {hard}"),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorkspacePartition {
    RepositoryTooling,
    Product,
}

#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub partition: WorkspacePartition,
    /// Empty internally denotes the normalized repository root (`.`).
    pub root: PathBuf,
    pub workspace_manifest: PathBuf,
    pub lockfile: PathBuf,
    pub members: Vec<PathBuf>,
    pub cargo_config_files: Vec<PathBuf>,
    pub build_scripts: Vec<PathBuf>,
    pub custom_target_specs: Vec<PathBuf>,
    pub linker_assets: Vec<PathBuf>,
    pub metadata_profiles: BTreeMap<String, MetadataProfile>,
    pub packages: BTreeMap<String, WorkspacePackage>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MetadataProfile {
    pub id: String,
    pub target_triple: String,
    pub all_features: bool,
    pub no_default_features: bool,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePackage {
    pub id: String,
    pub name: String,
    pub version: String,
    pub manifest: PathBuf,
    pub targets: BTreeMap<String, WorkspaceTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkspaceTarget {
    pub id: String,
    pub name: String,
    pub kind: Vec<String>,
    pub crate_types: Vec<String>,
    pub src_path: PathBuf,
}

/// Compatibility view used by the current root-workspace metadata caller.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Profile {
    pub name: String,
    pub target: String,
    pub all_features: bool,
    pub no_default_features: bool,
    pub features: Vec<String>,
}

/// Compatibility view used by the current root-workspace graph caller.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Package {
    pub name: String,
    pub manifest: PathBuf,
    pub targets: Vec<Target>,
}

/// Compatibility view used by the current root-workspace graph caller.
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
    /// Parses a closed v2 topology value and validates its semantic indexes.
    pub fn parse(value: &Value) -> Result<Self, ConfigError> {
        let object = json::object(
            value,
            "$",
            &["schema", "limits", "repository_entrypoints", "workspaces"],
            &[],
        )?;
        exact(&object["schema"], "$.schema", SCHEMA)?;
        let limits = parse_limits(&object["limits"])?;
        limits.enforce_hard_ceilings()?;
        let workspaces = parse_workspaces(&object["workspaces"], limits.path_bytes)?;
        let repository_entrypoints =
            parse_entrypoints(&object["repository_entrypoints"], limits.path_bytes)?;

        let root_workspace = workspaces
            .values()
            .find(|workspace| workspace.root.as_os_str().is_empty())
            .ok_or_else(|| {
                ConfigError::new(
                    "TOPOLOGY_ROOT_WORKSPACE",
                    "$.workspaces",
                    "exactly one workspace root must be the repository root '.'",
                )
            })?;

        let mut manifests = BTreeSet::new();
        let mut lockfiles = BTreeSet::new();
        let mut cargo_config_files = BTreeSet::new();
        let mut build_scripts = BTreeSet::new();
        let mut custom_target_specs = BTreeSet::new();
        let mut linker_assets = BTreeSet::new();
        for workspace in workspaces.values() {
            manifests.insert(workspace.workspace_manifest.clone());
            manifests.extend(workspace.packages.values().map(|package| package.manifest.clone()));
            lockfiles.insert(workspace.lockfile.clone());
            cargo_config_files.extend(workspace.cargo_config_files.iter().cloned());
            build_scripts.extend(workspace.build_scripts.iter().cloned());
            custom_target_specs.extend(workspace.custom_target_specs.iter().cloned());
            linker_assets.extend(workspace.linker_assets.iter().cloned());
        }

        let profiles = root_workspace
            .metadata_profiles
            .values()
            .map(|profile| Profile {
                name: profile.id.clone(),
                target: profile.target_triple.clone(),
                all_features: profile.all_features,
                no_default_features: profile.no_default_features,
                features: profile.features.clone(),
            })
            .collect();
        let packages = root_workspace
            .packages
            .values()
            .map(|package| Package {
                name: package.name.clone(),
                manifest: package.manifest.clone(),
                targets: package
                    .targets
                    .values()
                    .map(|target| Target {
                        name: target.name.clone(),
                        kind: target.kind.clone(),
                        crate_types: target.crate_types.clone(),
                        src_path: target.src_path.clone(),
                    })
                    .collect(),
            })
            .collect();

        let topology = Self {
            workspace_manifest: root_workspace.workspace_manifest.clone(),
            workspace_members: root_workspace.members.clone(),
            manifests: manifests.into_iter().collect(),
            lockfiles: lockfiles.into_iter().collect(),
            cargo_config_files: cargo_config_files.into_iter().collect(),
            build_scripts: build_scripts.into_iter().collect(),
            custom_target_specs: custom_target_specs.into_iter().collect(),
            linker_assets: linker_assets.into_iter().collect(),
            profiles,
            packages,
            workspaces,
            limits,
            repository_entrypoints,
        };
        topology.validate_repository_entrypoints()?;
        Ok(topology)
    }

    /// Resolves a Cargo metadata package whose `source` is null against every
    /// declared workspace. Name, version, and normalized manifest must all be
    /// exact; globally unique manifests make the result unambiguous.
    #[must_use]
    pub fn resolve_source_null_package<'a>(
        &'a self,
        name: &str,
        version: &str,
        manifest: &Path,
    ) -> Option<(&'a Workspace, &'a WorkspacePackage)> {
        self.workspaces.values().find_map(|workspace| {
            workspace.packages.values().find_map(|package| {
                (package.name == name && package.version == version && package.manifest == manifest)
                    .then_some((workspace, package))
            })
        })
    }

    fn validate_repository_entrypoints(&self) -> Result<(), ConfigError> {
        let declared = self
            .repository_entrypoints
            .iter()
            .filter_map(|entrypoint| match entrypoint {
                RepositoryEntrypoint::CargoTarget { package, target } => {
                    Some((package.clone(), target.clone()))
                }
                RepositoryEntrypoint::Script { .. } | RepositoryEntrypoint::Workflow { .. } => None,
            })
            .collect::<BTreeSet<_>>();
        let mut expected = BTreeSet::new();
        for workspace in self.workspaces.values() {
            for package in workspace.packages.values() {
                for target in package.targets.values() {
                    if target.kind.iter().any(|kind| kind == "bin")
                        && !expected.insert((package.name.clone(), target.name.clone()))
                    {
                        return Err(ConfigError::new(
                            "TOPOLOGY_CARGO_TARGET_ALIAS",
                            "$.workspaces",
                            format!(
                                "binary identity {}/{} is ambiguous across workspaces",
                                package.name, target.name
                            ),
                        ));
                    }
                }
            }
        }
        if declared != expected {
            return Err(ConfigError::new(
                "TOPOLOGY_CARGO_ENTRYPOINT_SET",
                "$.repository_entrypoints",
                "Cargo target entrypoints must equal every declared binary target",
            ));
        }
        Ok(())
    }
}

fn parse_workspaces(
    value: &Value,
    path_limit: usize,
) -> Result<BTreeMap<String, Workspace>, ConfigError> {
    let values = json::array(value, "$.workspaces")?;
    if values.is_empty() {
        return Err(ConfigError::new(
            "TOPOLOGY_EMPTY_WORKSPACES",
            "$.workspaces",
            "at least one workspace is required",
        ));
    }
    let mut workspaces = BTreeMap::new();
    let mut roots = BTreeSet::new();
    let mut workspace_manifests = BTreeSet::new();
    let mut lockfiles = BTreeSet::new();
    let mut member_paths = BTreeSet::new();
    let mut profile_ids = BTreeSet::new();
    let mut package_ids = BTreeSet::new();
    let mut package_names = BTreeSet::new();
    let mut package_manifests = BTreeSet::new();
    let mut target_sources = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let path = json::index("$.workspaces", item);
        let workspace = parse_workspace(value, &path, path_limit)?;
        if workspaces.contains_key(&workspace.id) {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_WORKSPACE_ID",
                json::child(&path, "id"),
                "duplicate workspace ID",
            ));
        }
        if !roots.insert(workspace.root.clone()) {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_WORKSPACE_ROOT",
                json::child(&path, "root"),
                "workspace root is already owned",
            ));
        }
        if !workspace_manifests.insert(workspace.workspace_manifest.clone()) {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_WORKSPACE_MANIFEST",
                json::child(&path, "workspace_manifest"),
                "workspace manifest is already owned",
            ));
        }
        if !lockfiles.insert(workspace.lockfile.clone()) {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_LOCKFILE",
                json::child(&path, "lockfile"),
                "workspace lockfile is already owned",
            ));
        }
        for member in &workspace.members {
            if !member_paths.insert(member.clone()) {
                return Err(ConfigError::new(
                    "TOPOLOGY_DUPLICATE_MEMBER",
                    json::child(&path, "members"),
                    "workspace member is already owned",
                ));
            }
        }
        for profile in workspace.metadata_profiles.values() {
            if !profile_ids.insert(profile.id.clone()) {
                return Err(ConfigError::new(
                    "TOPOLOGY_DUPLICATE_PROFILE_ID",
                    json::child(&path, "metadata_profiles"),
                    "metadata profile ID must be globally unique",
                ));
            }
        }
        for package in workspace.packages.values() {
            if !package_ids.insert(package.id.clone())
                || !package_names.insert(package.name.clone())
            {
                return Err(ConfigError::new(
                    "TOPOLOGY_DUPLICATE_PACKAGE_IDENTITY",
                    json::child(&path, "packages"),
                    "package ID and Cargo name must be globally unique",
                ));
            }
            if !package_manifests.insert(package.manifest.clone()) {
                return Err(ConfigError::new(
                    "TOPOLOGY_DUPLICATE_PACKAGE_MANIFEST",
                    json::child(&path, "packages"),
                    "package manifest is already owned",
                ));
            }
            for target in package.targets.values() {
                if !target_sources.insert(target.src_path.clone()) {
                    return Err(ConfigError::new(
                        "TOPOLOGY_DUPLICATE_TARGET_SOURCE",
                        json::child(&path, "packages"),
                        "target source is already owned",
                    ));
                }
            }
        }
        workspaces.insert(workspace.id.clone(), workspace);
    }
    Ok(workspaces)
}

fn parse_workspace(value: &Value, path: &str, path_limit: usize) -> Result<Workspace, ConfigError> {
    let object = json::object(
        value,
        path,
        &[
            "id",
            "partition",
            "root",
            "workspace_manifest",
            "lockfile",
            "members",
            "cargo_config_files",
            "build_scripts",
            "custom_target_specs",
            "linker_assets",
            "metadata_profiles",
            "packages",
        ],
        &[],
    )?;
    let id = identifier(&object["id"], &json::child(path, "id"))?;
    let partition_path = json::child(path, "partition");
    let partition = match json::string(&object["partition"], &partition_path)? {
        "repository_tooling" => WorkspacePartition::RepositoryTooling,
        "product" => WorkspacePartition::Product,
        other => {
            return Err(ConfigError::new(
                "TOPOLOGY_PARTITION",
                partition_path,
                format!("unknown workspace partition {other:?}"),
            ));
        }
    };
    let root = bounded_path(&object["root"], &json::child(path, "root"), true, path_limit)?;
    let workspace_manifest = bounded_path(
        &object["workspace_manifest"],
        &json::child(path, "workspace_manifest"),
        false,
        path_limit,
    )?;
    let lockfile =
        bounded_path(&object["lockfile"], &json::child(path, "lockfile"), false, path_limit)?;
    let members = paths(&object["members"], &json::child(path, "members"), path_limit)?;
    let cargo_config_files =
        paths(&object["cargo_config_files"], &json::child(path, "cargo_config_files"), path_limit)?;
    let build_scripts =
        paths(&object["build_scripts"], &json::child(path, "build_scripts"), path_limit)?;
    let custom_target_specs = paths(
        &object["custom_target_specs"],
        &json::child(path, "custom_target_specs"),
        path_limit,
    )?;
    let linker_assets =
        paths(&object["linker_assets"], &json::child(path, "linker_assets"), path_limit)?;
    let metadata_profiles =
        parse_profiles(&object["metadata_profiles"], &json::child(path, "metadata_profiles"))?;
    let packages = parse_packages(&object["packages"], &json::child(path, "packages"), path_limit)?;
    let workspace = Workspace {
        id,
        partition,
        root,
        workspace_manifest,
        lockfile,
        members,
        cargo_config_files,
        build_scripts,
        custom_target_specs,
        linker_assets,
        metadata_profiles,
        packages,
    };
    validate_workspace(&workspace, path)?;
    Ok(workspace)
}

fn validate_workspace(workspace: &Workspace, path: &str) -> Result<(), ConfigError> {
    if parent(&workspace.workspace_manifest) != workspace.root {
        return Err(ConfigError::new(
            "TOPOLOGY_WORKSPACE_MANIFEST",
            json::child(path, "workspace_manifest"),
            "workspace manifest must be directly under its declared root",
        ));
    }
    if parent(&workspace.lockfile) != workspace.root {
        return Err(ConfigError::new(
            "TOPOLOGY_WORKSPACE_LOCKFILE",
            json::child(path, "lockfile"),
            "workspace lockfile must be directly under its declared root",
        ));
    }
    for (field, paths) in [
        ("members", &workspace.members),
        ("cargo_config_files", &workspace.cargo_config_files),
        ("build_scripts", &workspace.build_scripts),
        ("custom_target_specs", &workspace.custom_target_specs),
        ("linker_assets", &workspace.linker_assets),
    ] {
        for candidate in paths {
            require_under_root(&workspace.root, candidate, &json::child(path, field))?;
        }
    }
    let package_members = workspace
        .packages
        .values()
        .map(|package| parent(&package.manifest))
        .collect::<BTreeSet<_>>();
    if package_members != workspace.members.iter().cloned().collect() {
        return Err(ConfigError::new(
            "TOPOLOGY_MEMBER_SET",
            json::child(path, "members"),
            "members must equal every package manifest directory",
        ));
    }
    let declared_build_scripts = workspace.build_scripts.iter().cloned().collect::<BTreeSet<_>>();
    let target_build_scripts = workspace
        .packages
        .values()
        .flat_map(|package| package.targets.values())
        .filter(|target| target.kind.iter().any(|kind| kind == "custom-build"))
        .map(|target| target.src_path.clone())
        .collect::<BTreeSet<_>>();
    if declared_build_scripts != target_build_scripts {
        return Err(ConfigError::new(
            "TOPOLOGY_BUILD_SCRIPT_SET",
            json::child(path, "build_scripts"),
            "build scripts must equal every custom-build target source",
        ));
    }
    for package in workspace.packages.values() {
        require_under_root(&workspace.root, &package.manifest, &json::child(path, "packages"))?;
        let package_root = parent(&package.manifest);
        for target in package.targets.values() {
            require_under_root(&package_root, &target.src_path, &json::child(path, "packages"))?;
        }
    }
    Ok(())
}

fn parse_profiles(
    value: &Value,
    path: &str,
) -> Result<BTreeMap<String, MetadataProfile>, ConfigError> {
    let values = json::array(value, path)?;
    if values.is_empty() {
        return Err(ConfigError::new(
            "TOPOLOGY_EMPTY_PROFILES",
            path,
            "at least one metadata profile is required",
        ));
    }
    let mut profiles = BTreeMap::new();
    for (item, value) in values.iter().enumerate() {
        let item_path = json::index(path, item);
        let object = json::object(
            value,
            &item_path,
            &["id", "target_triple", "all_features", "no_default_features", "features"],
            &[],
        )?;
        let id = identifier(&object["id"], &json::child(&item_path, "id"))?;
        let target_triple =
            nonempty(&object["target_triple"], &json::child(&item_path, "target_triple"))?;
        if target_triple.bytes().any(|byte| byte.is_ascii_whitespace()) {
            return Err(ConfigError::new(
                "TOPOLOGY_TARGET_FORM",
                json::child(&item_path, "target_triple"),
                "target triple must not contain whitespace",
            ));
        }
        let all_features =
            json::boolean(&object["all_features"], &json::child(&item_path, "all_features"))?;
        let no_default_features = json::boolean(
            &object["no_default_features"],
            &json::child(&item_path, "no_default_features"),
        )?;
        let features = strings(&object["features"], &json::child(&item_path, "features"), true)?;
        if all_features && (no_default_features || !features.is_empty()) {
            return Err(ConfigError::new(
                "TOPOLOGY_PROFILE_FEATURES",
                item_path,
                "all_features cannot be combined with no_default_features or explicit features",
            ));
        }
        let profile = MetadataProfile {
            id: id.clone(),
            target_triple,
            all_features,
            no_default_features,
            features,
        };
        if profiles.insert(id, profile).is_some() {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_PROFILE_ID",
                json::child(&item_path, "id"),
                "duplicate metadata profile ID",
            ));
        }
    }
    Ok(profiles)
}

fn parse_packages(
    value: &Value,
    path: &str,
    path_limit: usize,
) -> Result<BTreeMap<String, WorkspacePackage>, ConfigError> {
    let values = json::array(value, path)?;
    if values.is_empty() {
        return Err(ConfigError::new(
            "TOPOLOGY_EMPTY_PACKAGES",
            path,
            "at least one package is required",
        ));
    }
    let mut packages = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut manifests = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let item_path = json::index(path, item);
        let object = json::object(
            value,
            &item_path,
            &["id", "name", "version", "manifest", "targets"],
            &[],
        )?;
        let id = identifier(&object["id"], &json::child(&item_path, "id"))?;
        let name = nonempty(&object["name"], &json::child(&item_path, "name"))?;
        let version = nonempty(&object["version"], &json::child(&item_path, "version"))?;
        let manifest = bounded_path(
            &object["manifest"],
            &json::child(&item_path, "manifest"),
            false,
            path_limit,
        )?;
        let targets =
            parse_targets(&object["targets"], &json::child(&item_path, "targets"), path_limit)?;
        if !names.insert(name.clone()) || !manifests.insert(manifest.clone()) {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_PACKAGE_IDENTITY",
                item_path,
                "duplicate package name or manifest in workspace",
            ));
        }
        let package = WorkspacePackage { id: id.clone(), name, version, manifest, targets };
        if packages.insert(id, package).is_some() {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_PACKAGE_IDENTITY",
                json::child(&item_path, "id"),
                "duplicate package ID",
            ));
        }
    }
    Ok(packages)
}

fn parse_targets(
    value: &Value,
    path: &str,
    path_limit: usize,
) -> Result<BTreeMap<String, WorkspaceTarget>, ConfigError> {
    const KINDS: &[&str] = &[
        "lib",
        "rlib",
        "bin",
        "test",
        "example",
        "bench",
        "custom-build",
        "proc-macro",
        "dylib",
        "cdylib",
        "staticlib",
    ];
    let values = json::array(value, path)?;
    if values.is_empty() {
        return Err(ConfigError::new(
            "TOPOLOGY_EMPTY_TARGETS",
            path,
            "at least one target is required",
        ));
    }
    let mut targets = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut sources = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let item_path = json::index(path, item);
        let object = json::object(
            value,
            &item_path,
            &["id", "name", "kind", "crate_types", "src_path"],
            &[],
        )?;
        let id = identifier(&object["id"], &json::child(&item_path, "id"))?;
        let name = nonempty(&object["name"], &json::child(&item_path, "name"))?;
        let kind = strings(&object["kind"], &json::child(&item_path, "kind"), false)?;
        if let Some(unknown) = kind.iter().find(|kind| !KINDS.contains(&kind.as_str())) {
            return Err(ConfigError::new(
                "TOPOLOGY_TARGET_KIND",
                json::child(&item_path, "kind"),
                format!("unknown target kind {unknown:?}"),
            ));
        }
        let crate_types =
            strings(&object["crate_types"], &json::child(&item_path, "crate_types"), true)?;
        let src_path = bounded_path(
            &object["src_path"],
            &json::child(&item_path, "src_path"),
            false,
            path_limit,
        )?;
        if !names.insert(name.clone()) || !sources.insert(src_path.clone()) {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_TARGET_IDENTITY",
                item_path,
                "duplicate target name or source path in package",
            ));
        }
        let target = WorkspaceTarget { id: id.clone(), name, kind, crate_types, src_path };
        if targets.insert(id, target).is_some() {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_TARGET_IDENTITY",
                json::child(&item_path, "id"),
                "duplicate target ID",
            ));
        }
    }
    Ok(targets)
}

fn parse_entrypoints(
    value: &Value,
    path_limit: usize,
) -> Result<Vec<RepositoryEntrypoint>, ConfigError> {
    let values = json::array(value, "$.repository_entrypoints")?;
    let mut result = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let path = json::index("$.repository_entrypoints", item);
        let kind_path = json::child(&path, "kind");
        let raw = value.as_object().ok_or_else(|| {
            ConfigError::new("CFG_TYPE", &path, "expected repository entrypoint object")
        })?;
        let kind = raw.get("kind").ok_or_else(|| {
            ConfigError::new("CFG_MISSING_FIELD", &kind_path, "required field is absent")
        })?;
        let kind = json::string(kind, &kind_path)?;
        let entrypoint = match kind {
            "script" | "workflow" => {
                let object = json::object(value, &path, &["kind", "path", "content_digest"], &[])?;
                let entrypoint_path =
                    bounded_path(&object["path"], &json::child(&path, "path"), false, path_limit)?;
                if !paths.insert(entrypoint_path.clone()) {
                    return Err(ConfigError::new(
                        "TOPOLOGY_ENTRYPOINT_PATH_ALIAS",
                        json::child(&path, "path"),
                        "repository entrypoint path is declared more than once",
                    ));
                }
                let content_digest = sha256(
                    json::string(&object["content_digest"], &json::child(&path, "content_digest"))?,
                    &json::child(&path, "content_digest"),
                )?;
                if kind == "script" {
                    RepositoryEntrypoint::Script { path: entrypoint_path, content_digest }
                } else {
                    RepositoryEntrypoint::Workflow { path: entrypoint_path, content_digest }
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
                    kind_path,
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

#[allow(clippy::too_many_lines)]
fn parse_limits(value: &Value) -> Result<Limits, ConfigError> {
    let object = json::object(
        value,
        "$.limits",
        &["configuration", "inventory", "process", "product_execution"],
        &[],
    )?;
    let configuration = json::object(
        &object["configuration"],
        "$.limits.configuration",
        &["config_bytes", "json_depth", "json_container_items", "report_bytes", "findings"],
        &[],
    )?;
    let inventory = json::object(
        &object["inventory"],
        "$.limits.inventory",
        &[
            "source_entry_count",
            "directory_entry_count",
            "source_file_count",
            "source_total_bytes",
            "source_file_bytes",
            "path_bytes",
            "syntax_item_expr_type_visits",
        ],
        &[],
    )?;
    let process = json::object(
        &object["process"],
        "$.limits.process",
        &[
            "stdout_bytes",
            "stderr_bytes",
            "execution_deadline_ms",
            "supervision_deadline_ms",
            "tool_executable_bytes",
        ],
        &[],
    )?;
    let product = json::object(
        &object["product_execution"],
        "$.limits.product_execution",
        &[
            "scratch_bytes",
            "scratch_inodes",
            "process_address_space_bytes",
            "process_cpu_seconds",
            "process_open_files",
            "process_output_file_bytes",
            "core_dump_bytes",
            "exposure_entry_count",
            "compiler_substrate_entry_count",
            "compiler_substrate_total_bytes",
            "cargo_json_bytes",
            "cargo_message_count",
            "wrapper_trace_bytes",
            "wrapper_argv_items",
            "wrapper_argv_bytes",
            "compiler_unit_count",
            "compiler_input_count",
            "dep_info_file_bytes",
            "dep_info_total_bytes",
            "artifact_count",
            "artifact_total_bytes",
            "package_list_bytes",
            "package_member_count",
            "public_item_count",
            "environment_dependency_count",
        ],
        &[],
    )?;
    let core_dump_bytes = u8::try_from(json::unsigned(
        &product["core_dump_bytes"],
        "$.limits.product_execution.core_dump_bytes",
    )?)
    .map_err(|_| {
        ConfigError::new(
            "TOPOLOGY_LIMIT_RANGE",
            "$.limits.product_execution.core_dump_bytes",
            "limit exceeds u8",
        )
    })?;
    if core_dump_bytes != 0 {
        return Err(ConfigError::new(
            "TOPOLOGY_CORE_DUMP_LIMIT",
            "$.limits.product_execution.core_dump_bytes",
            "core dump limit must be exactly zero",
        ));
    }
    let limits = Limits {
        config_bytes: positive_usize(configuration, "$.limits.configuration", "config_bytes")?,
        json_depth: positive_usize(configuration, "$.limits.configuration", "json_depth")?,
        json_container_items: positive_usize(
            configuration,
            "$.limits.configuration",
            "json_container_items",
        )?,
        report_bytes: positive_usize(configuration, "$.limits.configuration", "report_bytes")?,
        findings: positive_usize(configuration, "$.limits.configuration", "findings")?,
        source_entry_count: positive_usize(inventory, "$.limits.inventory", "source_entry_count")?,
        directory_entry_count: positive_usize(
            inventory,
            "$.limits.inventory",
            "directory_entry_count",
        )?,
        source_file_count: positive_usize(inventory, "$.limits.inventory", "source_file_count")?,
        source_total_bytes: positive_u64(inventory, "$.limits.inventory", "source_total_bytes")?,
        source_file_bytes: positive_u64(inventory, "$.limits.inventory", "source_file_bytes")?,
        path_bytes: positive_usize(inventory, "$.limits.inventory", "path_bytes")?,
        syntax_item_expr_type_visits: positive_usize(
            inventory,
            "$.limits.inventory",
            "syntax_item_expr_type_visits",
        )?,
        metadata_stdout_bytes: positive_usize(process, "$.limits.process", "stdout_bytes")?,
        child_stderr_bytes: positive_usize(process, "$.limits.process", "stderr_bytes")?,
        child_deadline_ms: positive_u64(process, "$.limits.process", "execution_deadline_ms")?,
        child_supervision_deadline_ms: positive_u64(
            process,
            "$.limits.process",
            "supervision_deadline_ms",
        )?,
        tool_executable_bytes: positive_u64(process, "$.limits.process", "tool_executable_bytes")?,
        product_execution: ProductExecutionLimits {
            scratch_bytes: positive_u32(product, "$.limits.product_execution", "scratch_bytes")?,
            scratch_inodes: positive_u16(product, "$.limits.product_execution", "scratch_inodes")?,
            process_address_space_bytes: positive_u64(
                product,
                "$.limits.product_execution",
                "process_address_space_bytes",
            )?,
            process_cpu_seconds: positive_u8(
                product,
                "$.limits.product_execution",
                "process_cpu_seconds",
            )?,
            process_open_files: positive_u16(
                product,
                "$.limits.product_execution",
                "process_open_files",
            )?,
            process_output_file_bytes: positive_u32(
                product,
                "$.limits.product_execution",
                "process_output_file_bytes",
            )?,
            core_dump_bytes,
            exposure_entry_count: positive_u16(
                product,
                "$.limits.product_execution",
                "exposure_entry_count",
            )?,
            compiler_substrate_entry_count: positive_u16(
                product,
                "$.limits.product_execution",
                "compiler_substrate_entry_count",
            )?,
            compiler_substrate_total_bytes: positive_u32(
                product,
                "$.limits.product_execution",
                "compiler_substrate_total_bytes",
            )?,
            cargo_json_bytes: positive_u32(
                product,
                "$.limits.product_execution",
                "cargo_json_bytes",
            )?,
            cargo_message_count: positive_u16(
                product,
                "$.limits.product_execution",
                "cargo_message_count",
            )?,
            wrapper_trace_bytes: positive_u32(
                product,
                "$.limits.product_execution",
                "wrapper_trace_bytes",
            )?,
            wrapper_argv_items: positive_usize(
                product,
                "$.limits.product_execution",
                "wrapper_argv_items",
            )?,
            wrapper_argv_bytes: positive_u32(
                product,
                "$.limits.product_execution",
                "wrapper_argv_bytes",
            )?,
            compiler_unit_count: positive_usize(
                product,
                "$.limits.product_execution",
                "compiler_unit_count",
            )?,
            compiler_input_count: positive_u16(
                product,
                "$.limits.product_execution",
                "compiler_input_count",
            )?,
            dep_info_file_bytes: positive_u32(
                product,
                "$.limits.product_execution",
                "dep_info_file_bytes",
            )?,
            dep_info_total_bytes: positive_u32(
                product,
                "$.limits.product_execution",
                "dep_info_total_bytes",
            )?,
            artifact_count: positive_u16(product, "$.limits.product_execution", "artifact_count")?,
            artifact_total_bytes: positive_u32(
                product,
                "$.limits.product_execution",
                "artifact_total_bytes",
            )?,
            package_list_bytes: positive_u32(
                product,
                "$.limits.product_execution",
                "package_list_bytes",
            )?,
            package_member_count: positive_usize(
                product,
                "$.limits.product_execution",
                "package_member_count",
            )?,
            public_item_count: positive_usize(
                product,
                "$.limits.product_execution",
                "public_item_count",
            )?,
            environment_dependency_count: positive_usize(
                product,
                "$.limits.product_execution",
                "environment_dependency_count",
            )?,
        },
    };
    if limits.child_supervision_deadline_ms <= limits.child_deadline_ms {
        return Err(ConfigError::new(
            "TOPOLOGY_SUPERVISION_DEADLINE",
            "$.limits.process.supervision_deadline_ms",
            "supervision deadline must be strictly greater than execution deadline",
        ));
    }
    Ok(limits)
}

fn positive_u64(object: &Map<String, Value>, path: &str, name: &str) -> Result<u64, ConfigError> {
    let value_path = json::child(path, name);
    let value = json::unsigned(&object[name], &value_path)?;
    if value == 0 {
        Err(ConfigError::new("TOPOLOGY_ZERO_LIMIT", value_path, "limit must be positive"))
    } else {
        Ok(value)
    }
}

fn positive_usize(
    object: &Map<String, Value>,
    path: &str,
    name: &str,
) -> Result<usize, ConfigError> {
    usize::try_from(positive_u64(object, path, name)?).map_err(|_| {
        ConfigError::new(
            "TOPOLOGY_LIMIT_RANGE",
            json::child(path, name),
            "limit exceeds this host's usize",
        )
    })
}

fn positive_u32(object: &Map<String, Value>, path: &str, name: &str) -> Result<u32, ConfigError> {
    u32::try_from(positive_u64(object, path, name)?).map_err(|_| {
        ConfigError::new("TOPOLOGY_LIMIT_RANGE", json::child(path, name), "limit exceeds u32")
    })
}

fn positive_u16(object: &Map<String, Value>, path: &str, name: &str) -> Result<u16, ConfigError> {
    u16::try_from(positive_u64(object, path, name)?).map_err(|_| {
        ConfigError::new("TOPOLOGY_LIMIT_RANGE", json::child(path, name), "limit exceeds u16")
    })
}

fn positive_u8(object: &Map<String, Value>, path: &str, name: &str) -> Result<u8, ConfigError> {
    u8::try_from(positive_u64(object, path, name)?).map_err(|_| {
        ConfigError::new("TOPOLOGY_LIMIT_RANGE", json::child(path, name), "limit exceeds u8")
    })
}

fn paths(value: &Value, path: &str, path_limit: usize) -> Result<Vec<PathBuf>, ConfigError> {
    let mut result = BTreeSet::new();
    for (item, value) in json::array(value, path)?.iter().enumerate() {
        let item_path = json::index(path, item);
        let value = bounded_path(value, &item_path, false, path_limit)?;
        if !result.insert(value) {
            return Err(ConfigError::new(
                "TOPOLOGY_DUPLICATE_PATH",
                item_path,
                "duplicate normalized path",
            ));
        }
    }
    Ok(result.into_iter().collect())
}

fn bounded_path(
    value: &Value,
    path: &str,
    allow_repository_root: bool,
    path_limit: usize,
) -> Result<PathBuf, ConfigError> {
    let raw = json::string(value, path)?;
    if raw.len() > path_limit {
        return Err(ConfigError::new(
            "TOPOLOGY_PATH_LIMIT",
            path,
            format!("path is {} bytes; limit is {path_limit}", raw.len()),
        ));
    }
    if raw != "." && raw.split('/').any(|component| component == ".") {
        return Err(ConfigError::new(
            "CFG_PATH_FORM",
            path,
            "embedded '.' path components are not normalized",
        ));
    }
    relative_path(raw, path, allow_repository_root)
}

fn require_under_root(root: &Path, candidate: &Path, path: &str) -> Result<(), ConfigError> {
    if root.as_os_str().is_empty() || candidate.starts_with(root) {
        Ok(())
    } else {
        Err(ConfigError::new(
            "TOPOLOGY_WORKSPACE_ESCAPE",
            path,
            format!("{} is outside workspace root {}", candidate.display(), root.display()),
        ))
    }
}

fn parent(path: &Path) -> PathBuf {
    path.parent().unwrap_or_else(|| Path::new("")).to_path_buf()
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

fn identifier(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = nonempty(value, path)?;
    let bytes = value.as_bytes();
    let valid = bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !value.ends_with('-')
        && !value.contains("--");
    if valid {
        Ok(value)
    } else {
        Err(ConfigError::new("TOPOLOGY_ID", path, "expected lowercase kebab-case ID"))
    }
}

fn nonempty(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = json::string(value, path)?;
    if value.is_empty() {
        Err(ConfigError::new("CFG_EMPTY_STRING", path, "value must not be empty"))
    } else {
        Ok(value.to_owned())
    }
}

fn exact(value: &Value, path: &str, expected: &str) -> Result<(), ConfigError> {
    let actual = json::string(value, path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(ConfigError::new("CFG_CONST", path, format!("expected {expected:?}, got {actual:?}")))
    }
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::{Value, json};

    fn topology() -> Value {
        serde_json::from_str(include_str!("../../../conformance/workspace-topology.json")).unwrap()
    }

    fn add_product_workspace(value: &mut Value) {
        value["workspaces"].as_array_mut().unwrap().push(json!({
            "id": "product",
            "partition": "product",
            "root": "product",
            "workspace_manifest": "product/Cargo.toml",
            "lockfile": "product/Cargo.lock",
            "members": ["product/foundation"],
            "cargo_config_files": [],
            "build_scripts": [],
            "custom_target_specs": [],
            "linker_assets": [],
            "metadata_profiles": [{
                "id": "product-release",
                "target_triple": "x86_64-unknown-linux-gnu",
                "all_features": false,
                "no_default_features": true,
                "features": []
            }],
            "packages": [{
                "id": "foundation",
                "name": "reovim-foundation",
                "version": "0.16.0-dev",
                "manifest": "product/foundation/Cargo.toml",
                "targets": [{
                    "id": "library",
                    "name": "reovim_foundation",
                    "kind": ["lib"],
                    "crate_types": ["rlib"],
                    "src_path": "product/foundation/src/lib.rs"
                }]
            }]
        }));
    }

    #[test]
    fn current_product_empty_topology_parses_as_keyed_v2() {
        let topology = super::Topology::parse(&topology()).unwrap();
        assert_eq!(topology.workspaces.len(), 1);
        let tooling = &topology.workspaces["repository-tooling"];
        assert_eq!(tooling.partition, super::WorkspacePartition::RepositoryTooling);
        assert!(tooling.root.as_os_str().is_empty());
        assert_eq!(tooling.metadata_profiles.len(), 3);
        assert_eq!(tooling.packages.len(), 2);
        assert!(
            topology
                .workspaces
                .values()
                .all(|workspace| { workspace.partition != super::WorkspacePartition::Product })
        );
        assert_eq!(topology.workspace_manifest, Path::new("Cargo.toml"));
        assert_eq!(topology.profiles.len(), 3);
        assert_eq!(topology.packages.len(), 2);
    }

    #[test]
    fn dual_workspace_package_resolves_globally_and_exactly() {
        let mut value = topology();
        add_product_workspace(&mut value);
        let topology = super::Topology::parse(&value).unwrap();
        assert_eq!(topology.workspaces.len(), 2);
        let (workspace, package) = topology
            .resolve_source_null_package(
                "reovim-foundation",
                "0.16.0-dev",
                Path::new("product/foundation/Cargo.toml"),
            )
            .unwrap();
        assert_eq!(workspace.id, "product");
        assert_eq!(package.id, "foundation");
        assert!(
            topology
                .resolve_source_null_package(
                    "reovim-foundation",
                    "0.16.1",
                    Path::new("product/foundation/Cargo.toml"),
                )
                .is_none()
        );
        assert!(topology.manifests.contains(&Path::new("product/Cargo.toml").to_path_buf()));
        assert!(topology.lockfiles.contains(&Path::new("product/Cargo.lock").to_path_buf()));
    }

    #[test]
    fn schema_unknown_fields_and_ids_fail_closed() {
        let mut legacy = topology();
        legacy["schema"] = json!("reovim-workspace-topology/v1");
        assert_eq!(super::Topology::parse(&legacy).unwrap_err().code, "CFG_CONST");

        let mut unknown = topology();
        unknown["workspaces"][0]
            .as_object_mut()
            .unwrap()
            .insert("surprise".to_owned(), json!(true));
        assert_eq!(super::Topology::parse(&unknown).unwrap_err().code, "CFG_UNKNOWN_FIELD");

        let mut bad_id = topology();
        bad_id["workspaces"][0]["packages"][0]["id"] = json!("Bad_ID");
        assert_eq!(super::Topology::parse(&bad_id).unwrap_err().code, "TOPOLOGY_ID");
    }

    #[test]
    fn duplicate_workspace_profile_package_and_target_ids_fail() {
        let mut workspace = topology();
        let duplicate = workspace["workspaces"][0].clone();
        workspace["workspaces"].as_array_mut().unwrap().push(duplicate);
        assert_eq!(
            super::Topology::parse(&workspace).unwrap_err().code,
            "TOPOLOGY_DUPLICATE_WORKSPACE_ID"
        );

        let mut profile = topology();
        let duplicate = profile["workspaces"][0]["metadata_profiles"][0].clone();
        profile["workspaces"][0]["metadata_profiles"].as_array_mut().unwrap().push(duplicate);
        assert_eq!(
            super::Topology::parse(&profile).unwrap_err().code,
            "TOPOLOGY_DUPLICATE_PROFILE_ID"
        );

        let mut package = topology();
        let duplicate_id = package["workspaces"][0]["packages"][0]["id"].clone();
        package["workspaces"][0]["packages"][1]["id"] = duplicate_id;
        assert_eq!(
            super::Topology::parse(&package).unwrap_err().code,
            "TOPOLOGY_DUPLICATE_PACKAGE_IDENTITY"
        );

        let mut target = topology();
        let duplicate = target["workspaces"][0]["packages"][0]["targets"][0].clone();
        target["workspaces"][0]["packages"][0]["targets"].as_array_mut().unwrap().push(duplicate);
        assert_eq!(
            super::Topology::parse(&target).unwrap_err().code,
            "TOPOLOGY_DUPLICATE_TARGET_IDENTITY"
        );
    }

    #[test]
    fn normalized_bounded_and_contained_paths_are_required() {
        let mut alias = topology();
        alias["workspaces"][0]["packages"][0]["manifest"] = json!("lib/depgraph/./Cargo.toml");
        assert_eq!(super::Topology::parse(&alias).unwrap_err().code, "CFG_PATH_FORM");

        let mut escape = topology();
        add_product_workspace(&mut escape);
        escape["workspaces"][1]["packages"][0]["manifest"] = json!("other/Cargo.toml");
        escape["workspaces"][1]["members"] = json!(["other"]);
        assert_eq!(super::Topology::parse(&escape).unwrap_err().code, "TOPOLOGY_WORKSPACE_ESCAPE");

        let mut bounded = topology();
        bounded["limits"]["inventory"]["path_bytes"] = json!(10);
        assert_eq!(super::Topology::parse(&bounded).unwrap_err().code, "TOPOLOGY_PATH_LIMIT");
    }

    #[test]
    fn workspace_members_and_build_scripts_are_exact() {
        let mut members = topology();
        members["workspaces"][0]["members"] = json!(["lib/depgraph"]);
        assert_eq!(super::Topology::parse(&members).unwrap_err().code, "TOPOLOGY_MEMBER_SET");

        let mut build = topology();
        build["workspaces"][0]["build_scripts"] = json!(["tools/conformance/build.rs"]);
        assert_eq!(super::Topology::parse(&build).unwrap_err().code, "TOPOLOGY_BUILD_SCRIPT_SET");
    }

    #[test]
    fn profiles_targets_and_entrypoints_are_semantically_closed() {
        let mut profile = topology();
        profile["workspaces"][0]["metadata_profiles"][0]["all_features"] = json!(true);
        profile["workspaces"][0]["metadata_profiles"][0]["features"] = json!(["x"]);
        assert_eq!(super::Topology::parse(&profile).unwrap_err().code, "TOPOLOGY_PROFILE_FEATURES");

        let mut kind = topology();
        kind["workspaces"][0]["packages"][0]["targets"][0]["kind"] = json!(["mystery"]);
        assert_eq!(super::Topology::parse(&kind).unwrap_err().code, "TOPOLOGY_TARGET_KIND");

        let mut missing = topology();
        missing["repository_entrypoints"]
            .as_array_mut()
            .unwrap()
            .retain(|entrypoint| entrypoint["kind"] != "cargo_target");
        assert_eq!(
            super::Topology::parse(&missing).unwrap_err().code,
            "TOPOLOGY_CARGO_ENTRYPOINT_SET"
        );

        let mut alias = topology();
        alias["repository_entrypoints"].as_array_mut().unwrap().push(json!({
            "kind": "workflow",
            "path": "scripts/check.sh",
            "content_digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000"
        }));
        assert_eq!(
            super::Topology::parse(&alias).unwrap_err().code,
            "TOPOLOGY_ENTRYPOINT_PATH_ALIAS"
        );
    }

    #[test]
    fn nested_limits_are_closed_bounded_and_ordered() {
        let mut oversized = topology();
        oversized["limits"]["configuration"]["json_depth"] = json!(257);
        assert_eq!(super::Topology::parse(&oversized).unwrap_err().code, "TOPOLOGY_HARD_LIMIT");

        let mut deadline = topology();
        deadline["limits"]["process"]["supervision_deadline_ms"] = json!(120_000);
        assert_eq!(
            super::Topology::parse(&deadline).unwrap_err().code,
            "TOPOLOGY_SUPERVISION_DEADLINE"
        );

        let mut core = topology();
        core["limits"]["product_execution"]["core_dump_bytes"] = json!(1);
        assert_eq!(super::Topology::parse(&core).unwrap_err().code, "TOPOLOGY_CORE_DUMP_LIMIT");

        let mut unknown = topology();
        unknown["limits"]["process"].as_object_mut().unwrap().insert("forks".to_owned(), json!(1));
        assert_eq!(super::Topology::parse(&unknown).unwrap_err().code, "CFG_UNKNOWN_FIELD");
    }
}
