//! Reovim's non-shipping exact repository conformance checker.

#![forbid(unsafe_code)]

pub mod process;

mod cargo;
mod compiler_capture;
mod compiler_substrate;
mod digest;
mod entrypoint_reviews;
mod execution_tools;
mod finding;
mod git;
mod graph;
mod json;
mod package_capture;
mod policy;
mod prepared_root;
mod product_closures;
mod product_preflight;
mod product_proof;
mod product_source;
mod product_workload;
mod report;
mod report_build_v2;
mod report_v2;
mod reviews;
mod runtime_closure;
mod rust_source;
mod schema;
mod source;
mod topology;

use std::{
    collections::BTreeSet,
    fmt, fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use finding::{Finding, Findings};
use topology::{HARD_LIMITS, Limits, Topology};

pub const POLICY_KEY: &str = "conformance/source-classification.json";
pub const TOPOLOGY_KEY: &str = "conformance/workspace-topology.json";
pub const ENTRYPOINT_REVIEWS_KEY: &str = "conformance/entrypoint-reviews.json";
pub const DEPENDENCY_REVIEWS_KEY: &str = "conformance/dependency-reviews.json";
pub const PRODUCT_CLOSURES_KEY: &str = "conformance/product-closures.json";
pub const EXECUTION_TOOL_REVIEWS_KEY: &str =
    "conformance/execution-tool-reviews.json";
pub const POLICY_SCHEMA_KEY: &str =
    "conformance/source-classification.schema.json";
pub const TOPOLOGY_SCHEMA_KEY: &str =
    "conformance/workspace-topology.schema.json";
pub const ENTRYPOINT_REVIEWS_SCHEMA_KEY: &str =
    "conformance/entrypoint-reviews.schema.json";
pub const DEPENDENCY_REVIEWS_SCHEMA_KEY: &str =
    "conformance/dependency-reviews.schema.json";
pub const PRODUCT_CLOSURES_SCHEMA_KEY: &str =
    "conformance/product-closures.schema.json";
pub const EXECUTION_TOOL_REVIEWS_SCHEMA_KEY: &str =
    "conformance/execution-tool-reviews.schema.json";
pub const REPORT_SCHEMA_KEY: &str = "conformance/source-role-map.schema.json";

const EVIDENCE_AUTHORITY_MARKER: &str = ".reovim-evidence-authority-v1";
const EVIDENCE_AUTHORITY_MARKER_BYTES: &[u8] =
    b"reovim-evidence-authority-v1\n";

/// Reports whether Cargo invoked this binary as the private rustc wrapper.
///
/// This is a hidden binary-dispatch seam, not a Product or extension API.
#[doc(hidden)]
#[must_use]
pub fn private_rustc_wrapper_requested() -> bool {
    compiler_capture::wrapper_requested()
}

/// Reports whether Cargo invoked this image as the private rustc launcher.
#[doc(hidden)]
#[must_use]
pub fn private_rustc_launcher_requested() -> bool {
    compiler_capture::rustc_launcher_requested()
}

/// Runs one private same-binary rustc discovery launcher invocation.
///
/// # Errors
///
/// Returns a typed fatal failure when controls, argv, or compiler outcome
/// differ from the closed prepared-root probe profile.
#[doc(hidden)]
pub fn private_rustc_launcher_entry() -> Result<i32, Fatal> {
    compiler_capture::rustc_launcher_entry().map_err(|failure| {
        let code = failure.code;
        Fatal::new(code, failure.to_string())
    })
}

/// Runs one private same-binary rustc-wrapper invocation.
///
/// # Errors
///
/// Returns a typed fatal failure when the invocation, environment, trace, or
/// compiler outcome is outside the exact Product proof profile.
#[doc(hidden)]
pub fn private_rustc_wrapper_entry() -> Result<i32, Fatal> {
    compiler_capture::wrapper_entry().map_err(|failure| {
        let code = failure.code;
        Fatal::new(code, failure.to_string())
    })
}

/// Reports whether this binary received a prepared-root hidden dispatch.
///
/// This is a same-binary implementation seam, not a public Product API.
#[doc(hidden)]
#[must_use]
pub fn private_prepared_root_requested() -> bool {
    let argument = std::env::args_os().nth(1);
    argument.as_deref().is_some_and(|value| {
        value == std::ffi::OsStr::new(prepared_root::SETUP_ARGUMENT)
            || value == std::ffi::OsStr::new(prepared_root::PID1_ARGUMENT)
    })
}

/// Runs the exact prepared-root setup or namespace-PID-1 dispatch.
///
/// # Errors
///
/// Returns a typed fatal failure when the hidden argv, environment, bounded
/// protocol, setup, workload, or cleanup contract fails.
#[doc(hidden)]
pub fn private_prepared_root_entry() -> Result<(), Fatal> {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    let command = arguments.get(1).ok_or_else(|| {
        Fatal::new(
            "PREPARED_DISPATCH_ARGUMENT",
            "prepared-root dispatch argument is absent",
        )
    })?;
    let bounds = private_prepared_dispatch_bounds();
    let result = if command == prepared_root::SETUP_ARGUMENT {
        prepared_root::setup_entry(bounds)
    } else if command == prepared_root::PID1_ARGUMENT {
        let config = arguments.get(2).ok_or_else(|| {
            Fatal::new(
                "PREPARED_PID1_CONFIG_ARGUMENT",
                "namespace PID 1 config argument is absent",
            )
        })?;
        prepared_root::pid1_entry(
            bounds,
            config.as_encoded_bytes(),
            product_workload::run,
        )
    } else {
        return Err(Fatal::new(
            "PREPARED_DISPATCH_ARGUMENT",
            "unknown prepared-root dispatch argument",
        ));
    };
    result.map_err(|failure| Fatal::new(failure.code, failure.to_string()))
}

const fn private_prepared_dispatch_bounds() -> prepared_root::DispatchBounds {
    prepared_root::DispatchBounds {
        config_bytes: HARD_LIMITS.config_bytes,
        result_bytes: HARD_LIMITS.metadata_stdout_bytes,
        json_depth: HARD_LIMITS.json_depth,
        json_items: HARD_LIMITS.json_container_items,
        execution_deadline: std::time::Duration::from_millis(
            HARD_LIMITS.child_deadline_ms,
        ),
        supervision_deadline: std::time::Duration::from_millis(
            HARD_LIMITS.child_supervision_deadline_ms,
        ),
    }
}

#[derive(Debug, Clone)]
pub enum Mode {
    Candidate,
    Exact {
        git: PathBuf,
        expected_revision: String,
        execution_environment: String,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LimitOverrides {
    pub stdout_bytes: Option<usize>,
    pub stderr_bytes: Option<usize>,
    pub deadline_ms: Option<u64>,
    pub supervision_deadline_ms: Option<u64>,
    pub report_bytes: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct CheckOptions {
    pub root: PathBuf,
    pub policy: PathBuf,
    pub topology: PathBuf,
    pub entrypoint_reviews: PathBuf,
    pub dependency_reviews: PathBuf,
    pub product_closures: PathBuf,
    pub execution_tool_reviews: PathBuf,
    pub cargo: PathBuf,
    pub rustc: PathBuf,
    pub cargo_home: PathBuf,
    pub mode: Mode,
    pub limits: LimitOverrides,
}

#[derive(Debug)]
pub struct Outcome {
    report: report_v2::BuiltReport,
    clean: bool,
    root: PathBuf,
    exact_proven: Option<ExactProven>,
}

#[derive(Debug)]
struct ExactProven {
    revision: String,
}

impl Outcome {
    #[must_use]
    pub fn report_bytes(&self) -> &[u8] {
        &self.report.bytes
    }

    #[must_use]
    pub fn report_identity(&self) -> &str {
        &self.report.identity
    }

    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.clean
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedReport {
    pub identity: String,
    pub mode: String,
    pub status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectCertainty {
    NotApplied,
    Applied,
    OutcomeUnknown,
}

impl EffectCertainty {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotApplied => "not_applied",
            Self::Applied => "applied",
            Self::OutcomeUnknown => "outcome_unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupState {
    Complete,
    Incomplete,
}

impl CleanupState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Incomplete => "incomplete",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurabilityState {
    DirectorySynced,
    Unknown,
}

impl DurabilityState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectorySynced => "directory_synced",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug)]
pub struct PublicationOutcome {
    path: PathBuf,
    cleanup: CleanupState,
    durability: DurabilityState,
}

impl PublicationOutcome {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn effect_certainty(&self) -> EffectCertainty {
        EffectCertainty::Applied
    }

    #[must_use]
    pub const fn cleanup_state(&self) -> CleanupState {
        self.cleanup
    }

    #[must_use]
    pub const fn durability_state(&self) -> DurabilityState {
        self.durability
    }
}

#[derive(Debug)]
pub struct PublicationError {
    pub fatal: Fatal,
    pub effect_certainty: EffectCertainty,
    pub intended_path: Option<PathBuf>,
    pub cleanup_state: CleanupState,
    pub durability_state: DurabilityState,
}

impl fmt::Display for PublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fatal.fmt(formatter)
    }
}

impl std::error::Error for PublicationError {}

impl PublicationError {
    const fn not_applied(
        fatal: Fatal,
        intended_path: Option<PathBuf>,
        cleanup_state: CleanupState,
    ) -> Self {
        Self {
            fatal,
            effect_certainty: EffectCertainty::NotApplied,
            intended_path,
            cleanup_state,
            durability_state: DurabilityState::Unknown,
        }
    }

    const fn applied(
        fatal: Fatal,
        intended_path: PathBuf,
        cleanup_state: CleanupState,
        durability_state: DurabilityState,
    ) -> Self {
        Self {
            fatal,
            effect_certainty: EffectCertainty::Applied,
            intended_path: Some(intended_path),
            cleanup_state,
            durability_state,
        }
    }

    const fn outcome_unknown(
        fatal: Fatal,
        intended_path: PathBuf,
        cleanup_state: CleanupState,
    ) -> Self {
        Self {
            fatal,
            effect_certainty: EffectCertainty::OutcomeUnknown,
            intended_path: Some(intended_path),
            cleanup_state,
            durability_state: DurabilityState::Unknown,
        }
    }
}

#[derive(Debug)]
pub struct Fatal {
    pub code: &'static str,
    pub message: String,
}

impl Fatal {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl fmt::Display for Fatal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Fatal {}

/// Runs the bounded checker and constructs a deterministic candidate or exact
/// report.
///
/// # Errors
///
/// Returns a typed fatal error when inputs, tools, subprocess ownership, or
/// report construction cannot be established. Policy violations are retained
/// as report findings rather than returned as errors.
#[allow(clippy::too_many_lines)]
pub fn check(options: &CheckOptions) -> Result<Outcome, Fatal> {
    require_absolute("root", &options.root)?;
    let root = options
        .root
        .canonicalize()
        .map_err(|error| Fatal::new("ROOT_IO", error.to_string()))?;
    if root != options.root || !root.is_dir() {
        return Err(Fatal::new(
            "ROOT_CANONICAL",
            "--root must name the canonical absolute repository directory",
        ));
    }
    let checker_handle = open_executing_image()?;
    let topology_path =
        committed_input(&root, &options.topology, TOPOLOGY_KEY)?;
    let topology_schema_path = committed_input(
        &root,
        &root.join(TOPOLOGY_SCHEMA_KEY),
        TOPOLOGY_SCHEMA_KEY,
    )?;
    let (topology_bytes, topology_value) = json::read(
        &topology_path,
        HARD_LIMITS.config_bytes,
        HARD_LIMITS.json_depth,
        HARD_LIMITS.json_container_items,
    )
    .map_err(config_fatal)?;
    let (topology_schema_bytes, topology_schema_value) = json::read(
        &topology_schema_path,
        HARD_LIMITS.config_bytes,
        HARD_LIMITS.json_depth,
        HARD_LIMITS.json_container_items,
    )
    .map_err(config_fatal)?;
    schema::validate_instance(&topology_value, &topology_schema_value)
        .map_err(|error| Fatal::new("TOPOLOGY_SCHEMA", error.to_string()))?;
    let mut topology =
        Topology::parse(&topology_value).map_err(config_fatal)?;
    apply_limit_overrides(&mut topology.limits, options.limits)?;

    let policy_path = committed_input(&root, &options.policy, POLICY_KEY)?;
    let entrypoint_reviews_path = committed_input(
        &root,
        &options.entrypoint_reviews,
        ENTRYPOINT_REVIEWS_KEY,
    )?;
    let reviews_path = committed_input(
        &root,
        &options.dependency_reviews,
        DEPENDENCY_REVIEWS_KEY,
    )?;
    let product_closures_path = committed_input(
        &root,
        &options.product_closures,
        PRODUCT_CLOSURES_KEY,
    )?;
    let execution_tool_reviews_path = committed_input(
        &root,
        &options.execution_tool_reviews,
        EXECUTION_TOOL_REVIEWS_KEY,
    )?;
    let policy_schema_path = committed_input(
        &root,
        &root.join(POLICY_SCHEMA_KEY),
        POLICY_SCHEMA_KEY,
    )?;
    let reviews_schema_path = committed_input(
        &root,
        &root.join(DEPENDENCY_REVIEWS_SCHEMA_KEY),
        DEPENDENCY_REVIEWS_SCHEMA_KEY,
    )?;
    let entrypoint_reviews_schema_path = committed_input(
        &root,
        &root.join(ENTRYPOINT_REVIEWS_SCHEMA_KEY),
        ENTRYPOINT_REVIEWS_SCHEMA_KEY,
    )?;
    let product_closures_schema_path = committed_input(
        &root,
        &root.join(PRODUCT_CLOSURES_SCHEMA_KEY),
        PRODUCT_CLOSURES_SCHEMA_KEY,
    )?;
    let execution_tool_reviews_schema_path = committed_input(
        &root,
        &root.join(EXECUTION_TOOL_REVIEWS_SCHEMA_KEY),
        EXECUTION_TOOL_REVIEWS_SCHEMA_KEY,
    )?;
    let report_schema_path = committed_input(
        &root,
        &root.join(REPORT_SCHEMA_KEY),
        REPORT_SCHEMA_KEY,
    )?;
    let (policy_bytes, policy_value) = json::read(
        &policy_path,
        topology.limits.config_bytes,
        topology.limits.json_depth,
        topology.limits.json_container_items,
    )
    .map_err(config_fatal)?;
    let (reviews_bytes, reviews_value) = json::read(
        &reviews_path,
        topology.limits.config_bytes,
        topology.limits.json_depth,
        topology.limits.json_container_items,
    )
    .map_err(config_fatal)?;
    let (entrypoint_reviews_bytes, entrypoint_reviews_value) = json::read(
        &entrypoint_reviews_path,
        topology.limits.config_bytes,
        topology.limits.json_depth,
        topology.limits.json_container_items,
    )
    .map_err(config_fatal)?;
    let (product_closures_bytes, product_closures_value) = json::read(
        &product_closures_path,
        topology.limits.config_bytes,
        topology.limits.json_depth,
        topology.limits.json_container_items,
    )
    .map_err(config_fatal)?;
    let (execution_tool_reviews_bytes, execution_tool_reviews_value) =
        json::read(
            &execution_tool_reviews_path,
            topology.limits.config_bytes,
            topology.limits.json_depth,
            topology.limits.json_container_items,
        )
        .map_err(config_fatal)?;
    let (policy_schema_bytes, policy_schema_value) = json::read(
        &policy_schema_path,
        topology.limits.config_bytes,
        topology.limits.json_depth,
        topology.limits.json_container_items,
    )
    .map_err(config_fatal)?;
    let (reviews_schema_bytes, reviews_schema_value) = json::read(
        &reviews_schema_path,
        topology.limits.config_bytes,
        topology.limits.json_depth,
        topology.limits.json_container_items,
    )
    .map_err(config_fatal)?;
    let (entrypoint_reviews_schema_bytes, entrypoint_reviews_schema_value) =
        json::read(
            &entrypoint_reviews_schema_path,
            topology.limits.config_bytes,
            topology.limits.json_depth,
            topology.limits.json_container_items,
        )
        .map_err(config_fatal)?;
    let (product_closures_schema_bytes, product_closures_schema_value) =
        json::read(
            &product_closures_schema_path,
            topology.limits.config_bytes,
            topology.limits.json_depth,
            topology.limits.json_container_items,
        )
        .map_err(config_fatal)?;
    let (
        execution_tool_reviews_schema_bytes,
        execution_tool_reviews_schema_value,
    ) = json::read(
        &execution_tool_reviews_schema_path,
        topology.limits.config_bytes,
        topology.limits.json_depth,
        topology.limits.json_container_items,
    )
    .map_err(config_fatal)?;
    let (report_schema_bytes, report_schema_value) = json::read(
        &report_schema_path,
        topology.limits.config_bytes,
        topology.limits.json_depth,
        topology.limits.json_container_items,
    )
    .map_err(config_fatal)?;
    schema::validate_instance(&policy_value, &policy_schema_value)
        .map_err(|error| Fatal::new("POLICY_SCHEMA", error.to_string()))?;
    schema::validate_instance(&reviews_value, &reviews_schema_value).map_err(
        |error| Fatal::new("DEPENDENCY_REVIEWS_SCHEMA", error.to_string()),
    )?;
    schema::validate_instance(
        &entrypoint_reviews_value,
        &entrypoint_reviews_schema_value,
    )
    .map_err(|error| {
        Fatal::new("ENTRYPOINT_REVIEWS_SCHEMA", error.to_string())
    })?;
    schema::validate_instance(
        &product_closures_value,
        &product_closures_schema_value,
    )
    .map_err(|error| {
        Fatal::new("PRODUCT_CLOSURES_SCHEMA", error.to_string())
    })?;
    schema::validate_instance(
        &execution_tool_reviews_value,
        &execution_tool_reviews_schema_value,
    )
    .map_err(|error| {
        Fatal::new("EXECUTION_TOOL_REVIEWS_SCHEMA", error.to_string())
    })?;
    // Validate the schema itself and require the running checker to embed the
    // exact committed report schema, preventing a stale binary/schema pair.
    schema::validate_schema(&report_schema_value).map_err(|error| {
        Fatal::new("REPORT_SCHEMA_ENGINE", error.to_string())
    })?;
    if digest::bytes(&report_schema_bytes)
        != digest::bytes(schema::embedded_report_schema_bytes())
    {
        return Err(Fatal::new(
            "REPORT_SCHEMA_BINARY_DRIFT",
            "running checker does not embed the committed report schema",
        ));
    }
    let policy = policy::Policy::parse(&policy_value).map_err(config_fatal)?;
    let entrypoint_registry =
        entrypoint_reviews::Registry::parse(&entrypoint_reviews_value)
            .map_err(config_fatal)?;
    let registry =
        reviews::Registry::parse(&reviews_value).map_err(config_fatal)?;
    let product_limits = topology.limits.product_execution;
    let product_registry = product_closures::Registry::parse(
        &product_closures_value,
        product_closures::ParseLimits {
            path_bytes: topology.limits.path_bytes,
            closure_items: product_limits.compiler_unit_count,
            package_members: product_limits.package_member_count,
            public_items: product_limits.public_item_count,
        },
    )
    .map_err(config_fatal)?;
    let execution_tool_registry = execution_tools::Registry::parse(
        &execution_tool_reviews_value,
        execution_tools::ParseLimits {
            path_bytes: topology.limits.path_bytes,
            registry_items: product_limits.environment_dependency_count,
            binding_items: product_limits.environment_dependency_count,
            runtime_inputs: product_limits.environment_dependency_count,
            argv_items: product_limits.wrapper_argv_items,
        },
    )
    .map_err(config_fatal)?;

    let cargo = canonical_executable("cargo", &options.cargo)?;
    let rustc = canonical_executable("rustc", &options.rustc)?;
    let checker_path = std::env::current_exe()
        .map_err(|error| Fatal::new("CHECKER_IDENTITY", error.to_string()))?
        .canonicalize()
        .map_err(|error| Fatal::new("CHECKER_IDENTITY", error.to_string()))?;
    let cargo_home = validate_cargo_home(&options.cargo_home)?;
    validate_fixed_cargo_working_directory()?;
    let process_limits = topology.limits.process();
    let cargo_digest_before =
        digest::file_bounded(&cargo, topology.limits.tool_executable_bytes)
            .map_err(|error| Fatal::new("CARGO_IDENTITY", error.to_string()))?;
    let rustc_digest_before =
        digest::file_bounded(&rustc, topology.limits.tool_executable_bytes)
            .map_err(|error| Fatal::new("RUSTC_IDENTITY", error.to_string()))?;
    let checker_digest_before = digest::file_handle_bounded(
        &checker_handle,
        topology.limits.tool_executable_bytes,
    )
    .map_err(|error| Fatal::new("CHECKER_IDENTITY", error.to_string()))?;
    let checker_path_digest_before = digest::file_bounded(
        &checker_path,
        topology.limits.tool_executable_bytes,
    )
    .map_err(|error| Fatal::new("CHECKER_IDENTITY", error.to_string()))?;
    if checker_path_digest_before != checker_digest_before {
        return Err(Fatal::new(
            "CHECKER_IDENTITY",
            "current executable path and held executing-image bytes differ",
        ));
    }
    let checker_version =
        capture_checker_version(&checker_path, &root, process_limits)?;
    let git_canonical = match &options.mode {
        Mode::Candidate => None,
        Mode::Exact { git, .. } => Some(canonical_executable("git", git)?),
    };
    let git_digest_before = git_canonical
        .as_ref()
        .map(|git| {
            digest::file_bounded(git, topology.limits.tool_executable_bytes)
        })
        .transpose()
        .map_err(|error| Fatal::new("GIT_IDENTITY", error.to_string()))?;

    let inventory = source::inventory(&root, &policy, &topology);
    validate_product_registry(&topology, &inventory, &product_registry)?;
    validate_execution_tool_review_records(
        &root,
        &inventory,
        &execution_tool_registry,
        topology.limits.source_file_bytes,
    )?;
    let selected_git_binding =
        match (&options.mode, &git_canonical, &git_digest_before) {
            (
                Mode::Exact { execution_environment, .. },
                Some(git),
                Some(executable_digest),
            ) => Some(select_git_binding(
                execution_environment,
                git,
                executable_digest,
                &execution_tool_registry,
                topology.limits.tool_executable_bytes,
            )?),
            (Mode::Candidate, None, None) => None,
            _ => {
                return Err(Fatal::new(
                    "EXECUTION_TOOL_STATE",
                    "mode and exact Git execution-tool inputs are inconsistent",
                ));
            }
        };
    if matches!(options.mode, Mode::Exact { .. })
        && inventory
            .findings
            .iter()
            .any(|finding| finding.code == "UNMODELED_GIT_SEMANTICS")
    {
        return Err(Fatal::new(
            "GIT_UNMODELED_SEMANTICS",
            concat!(
                "exact Git commands are not run while an inventoried ",
                "attributes/submodule input exists",
            ),
        ));
    }

    let mut git_evidence = match (&options.mode, &git_canonical) {
        (Mode::Candidate, _) => None,
        (Mode::Exact { expected_revision, .. }, Some(git)) => Some(
            git::prove(git, &root, expected_revision, process_limits)
                .map_err(|error| Fatal::new(error.code, error.message))?,
        ),
        (Mode::Exact { .. }, None) => {
            unreachable!("exact mode has canonical Git path")
        }
    };

    if let Some(evidence) = &git_evidence {
        let binding = selected_git_binding.ok_or_else(|| {
            Fatal::new(
                "EXECUTION_TOOL_STATE",
                "exact Git evidence lacks its selected binding",
            )
        })?;
        if evidence.version != binding.version
            || evidence.version_digest != binding.version_digest
        {
            return Err(Fatal::new(
                "EXECUTION_TOOL_VERSION",
                concat!(
                    "Git version evidence differs from the selected ",
                    "execution-tool binding",
                ),
            ));
        }
        validate_git_inventory(
            evidence,
            &inventory,
            &policy,
            &root,
            topology.limits,
        )?;
    }
    let lockfiles =
        report::lockfile_identities(&topology.lockfiles, &inventory.files)
            .map_err(|error| {
                Fatal::new("LOCKFILE_IDENTITY", error.to_string())
            })?;
    let cargo_version =
        cargo::version(&cargo, &rustc, &root, &cargo_home, process_limits)
            .map_err(cargo_fatal)?;
    let rustc_version = cargo::rustc_version(&rustc, &root, process_limits)
        .map_err(cargo_fatal)?;
    let mut profiles = Vec::new();
    for workspace in topology.workspaces.values() {
        for declaration in workspace.metadata_profiles.values() {
            let profile = execution_profile(declaration);
            let capture = cargo::metadata(
                &cargo,
                &rustc,
                &root,
                &cargo_home,
                &workspace.workspace_manifest,
                &profile,
                process_limits,
                topology.limits.json_depth,
                topology.limits.json_container_items,
            )
            .map_err(cargo_fatal)?;
            profiles.push((workspace.id.clone(), profile.clone(), capture));
        }
    }
    let expected_profile_count = topology
        .workspaces
        .values()
        .map(|workspace| workspace.metadata_profiles.len())
        .sum::<usize>();
    if profiles.len() != expected_profile_count {
        return Err(Fatal::new(
            "PROFILE_COVERAGE",
            "not every committed topology profile was executed exactly once",
        ));
    }

    let mut graphs = Vec::new();
    let mut all_findings = Findings::new(topology.limits.findings);
    all_findings.extend(inventory.findings.clone());
    all_findings.extend(entrypoint_reviews::validate(
        &root,
        &entrypoint_registry,
        &topology,
        &inventory.files,
        topology.limits.findings,
        topology.limits.source_file_bytes,
    ));
    for (_, profile, capture) in &profiles {
        let graph = graph::check(
            &root,
            &profile.name,
            &capture.metadata,
            &inventory.files,
            &policy,
            &topology,
        );
        all_findings.extend(graph.findings.clone());
        all_findings.extend(rust_source::check(
            &root,
            &profile.name,
            &capture.metadata,
            &inventory.files,
            &policy,
            &topology,
            topology.limits,
        ));
        graphs.push(graph);
    }
    let reviewed_packages = topology
        .workspaces
        .values()
        .map(|workspace| {
            workspace
                .packages
                .values()
                .map(|package| reviews::WorkspacePackage {
                    id: package.id.clone(),
                    cargo_name: package.name.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let reviewed_profiles = topology
        .workspaces
        .values()
        .map(|workspace| {
            profiles
                .iter()
                .filter(|(workspace_id, _, _)| workspace_id == &workspace.id)
                .map(|(_, profile, capture)| reviews::ProfileMetadata {
                    id: &profile.name,
                    metadata: &capture.metadata,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let review_workspaces = topology
        .workspaces
        .values()
        .zip(&reviewed_packages)
        .zip(&reviewed_profiles)
        .map(|((workspace, packages), profiles)| reviews::ValidationWorkspace {
            id: &workspace.id,
            lockfile: &workspace.lockfile,
            packages,
            profiles,
        })
        .collect::<Vec<_>>();
    all_findings.extend(reviews::validate(
        &root,
        &registry,
        &review_workspaces,
        topology.limits.findings,
        topology.limits.source_file_bytes,
        &inventory.files,
    ));
    let findings = all_findings.finish();

    let product_evidence = if product_registry.closures.is_empty() {
        None
    } else {
        if !findings.is_empty() {
            return Err(Fatal::new(
                "PRODUCT_STATIC_PROOF_FAILED",
                static_proof_failure(&findings),
            ));
        }
        let Mode::Exact { execution_environment, .. } = &options.mode else {
            return Err(Fatal::new(
                "PRODUCT_EXACT_PROOF_REQUIRED",
                "active Product evidence requires exact clean-revision mode",
            ));
        };
        Some(
            product_proof::run(&product_proof::Request {
                repository_root: &root,
                cargo: &cargo,
                rustc: &rustc,
                checker: &checker_path,
                inventory: &inventory,
                limits: topology.limits,
                registry_value: &product_closures_value,
                registry: &product_registry,
                execution_environment,
            })
            .map_err(|failure| Fatal::new(failure.code, failure.to_string()))?,
        )
    };

    let mut final_profiles = Vec::new();
    if let (Mode::Exact { expected_revision, .. }, Some(git)) =
        (&options.mode, &git_canonical)
    {
        let second_inventory = source::inventory(&root, &policy, &topology);
        if second_inventory.tree_digest != inventory.tree_digest
            || second_inventory.files != inventory.files
            || second_inventory.findings != inventory.findings
        {
            return Err(Fatal::new(
                "SOURCE_TREE_DRIFT",
                "source inventory changed during the conformance run",
            ));
        }
        require_config_unchanged(&policy_path, &policy_bytes, topology.limits)?;
        require_config_unchanged(
            &topology_path,
            &topology_bytes,
            topology.limits,
        )?;
        require_config_unchanged(
            &entrypoint_reviews_path,
            &entrypoint_reviews_bytes,
            topology.limits,
        )?;
        require_config_unchanged(
            &reviews_path,
            &reviews_bytes,
            topology.limits,
        )?;
        require_config_unchanged(
            &product_closures_path,
            &product_closures_bytes,
            topology.limits,
        )?;
        require_config_unchanged(
            &execution_tool_reviews_path,
            &execution_tool_reviews_bytes,
            topology.limits,
        )?;
        require_config_unchanged(
            &policy_schema_path,
            &policy_schema_bytes,
            topology.limits,
        )?;
        require_config_unchanged(
            &topology_schema_path,
            &topology_schema_bytes,
            topology.limits,
        )?;
        require_config_unchanged(
            &entrypoint_reviews_schema_path,
            &entrypoint_reviews_schema_bytes,
            topology.limits,
        )?;
        require_config_unchanged(
            &reviews_schema_path,
            &reviews_schema_bytes,
            topology.limits,
        )?;
        require_config_unchanged(
            &product_closures_schema_path,
            &product_closures_schema_bytes,
            topology.limits,
        )?;
        require_config_unchanged(
            &execution_tool_reviews_schema_path,
            &execution_tool_reviews_schema_bytes,
            topology.limits,
        )?;
        require_config_unchanged(
            &report_schema_path,
            &report_schema_bytes,
            topology.limits,
        )?;
        let final_lockfiles = report::lockfile_identities(
            &topology.lockfiles,
            &second_inventory.files,
        )
        .map_err(|error| Fatal::new("LOCKFILE_IDENTITY", error.to_string()))?;
        if final_lockfiles != lockfiles {
            return Err(Fatal::new(
                "LOCKFILE_DRIFT",
                "lockfile changed during the conformance run",
            ));
        }
        let pre_metadata_git =
            git::prove(git, &root, expected_revision, process_limits)
                .map_err(|error| Fatal::new(error.code, error.message))?;
        validate_git_inventory(
            &pre_metadata_git,
            &second_inventory,
            &policy,
            &root,
            topology.limits,
        )?;
        let Some(evidence) = git_evidence.as_mut() else {
            return Err(Fatal::new(
                "GIT_EVIDENCE_STATE",
                "exact mode lost initial Git evidence",
            ));
        };
        if pre_metadata_git.revision != evidence.revision
            || pre_metadata_git.version_digest != evidence.version_digest
            || pre_metadata_git.tree_digest != evidence.tree_digest
            || pre_metadata_git.tracked_paths != evidence.tracked_paths
            || pre_metadata_git.object_format != evidence.object_format
            || pre_metadata_git.tree_entries != evidence.tree_entries
        {
            return Err(Fatal::new(
                "GIT_DRIFT",
                "Git revision or executable version changed",
            ));
        }
        append_git_phase(evidence, pre_metadata_git, "pre_metadata");
        for (workspace_id, profile, initial) in &profiles {
            let workspace = &topology.workspaces[workspace_id];
            let final_capture = cargo::metadata(
                &cargo,
                &rustc,
                &root,
                &cargo_home,
                &workspace.workspace_manifest,
                profile,
                process_limits,
                topology.limits.json_depth,
                topology.limits.json_container_items,
            )
            .map_err(cargo_fatal)?;
            require_metadata_unchanged(profile, initial, &final_capture)?;
            final_profiles.push((
                workspace_id.clone(),
                profile.clone(),
                final_capture,
            ));
        }
        let post_metadata_inventory =
            source::inventory(&root, &policy, &topology);
        if post_metadata_inventory.tree_digest != inventory.tree_digest
            || post_metadata_inventory.files != inventory.files
            || post_metadata_inventory.findings != inventory.findings
        {
            return Err(Fatal::new(
                "SOURCE_TREE_DRIFT",
                "source inventory changed during final metadata verification",
            ));
        }
        let final_git =
            git::prove(git, &root, expected_revision, process_limits)
                .map_err(|error| Fatal::new(error.code, error.message))?;
        validate_git_inventory(
            &final_git,
            &post_metadata_inventory,
            &policy,
            &root,
            topology.limits,
        )?;
        if final_git.revision != evidence.revision
            || final_git.version_digest != evidence.version_digest
            || final_git.tree_digest != evidence.tree_digest
            || final_git.tracked_paths != evidence.tracked_paths
            || final_git.object_format != evidence.object_format
            || final_git.tree_entries != evidence.tree_entries
        {
            return Err(Fatal::new(
                "GIT_DRIFT",
                "Git revision or executable version changed",
            ));
        }
        append_git_phase(evidence, final_git, "final");
    }

    let cargo_digest_after =
        digest::file_bounded(&cargo, topology.limits.tool_executable_bytes)
            .map_err(|error| Fatal::new("CARGO_IDENTITY", error.to_string()))?;
    let rustc_digest_after =
        digest::file_bounded(&rustc, topology.limits.tool_executable_bytes)
            .map_err(|error| Fatal::new("RUSTC_IDENTITY", error.to_string()))?;
    let checker_digest = digest::file_handle_bounded(
        &checker_handle,
        topology.limits.tool_executable_bytes,
    )
    .map_err(|error| Fatal::new("CHECKER_IDENTITY", error.to_string()))?;
    let checker_path_digest_after = digest::file_bounded(
        &checker_path,
        topology.limits.tool_executable_bytes,
    )
    .map_err(|error| Fatal::new("CHECKER_IDENTITY", error.to_string()))?;
    let git_digest_after = git_canonical
        .as_ref()
        .map(|git| {
            digest::file_bounded(git, topology.limits.tool_executable_bytes)
        })
        .transpose()
        .map_err(|error| Fatal::new("GIT_IDENTITY", error.to_string()))?;
    for (name, before, after) in [
        ("Cargo", Some(&cargo_digest_before), Some(&cargo_digest_after)),
        ("Rustc", Some(&rustc_digest_before), Some(&rustc_digest_after)),
        ("Git", git_digest_before.as_ref(), git_digest_after.as_ref()),
        ("checker", Some(&checker_digest_before), Some(&checker_digest)),
        (
            "checker path",
            Some(&checker_path_digest_before),
            Some(&checker_path_digest_after),
        ),
    ] {
        if before != after {
            return Err(Fatal::new(
                "TOOL_EXECUTABLE_DRIFT",
                format!(
                    "{name} executable bytes changed during the conformance run"
                ),
            ));
        }
    }
    if let (
        Mode::Exact { execution_environment, .. },
        Some(git),
        Some(executable_digest),
    ) = (&options.mode, &git_canonical, &git_digest_after)
    {
        let final_binding = select_git_binding(
            execution_environment,
            git,
            executable_digest,
            &execution_tool_registry,
            topology.limits.tool_executable_bytes,
        )?;
        if selected_git_binding != Some(final_binding) {
            return Err(Fatal::new(
                "EXECUTION_TOOL_BINDING_DRIFT",
                "selected Git execution-tool binding changed during the run",
            ));
        }
    }

    let policy_digest = digest::bytes(&policy_bytes);
    let policy_schema_digest = digest::bytes(&policy_schema_bytes);
    let topology_digest = digest::bytes(&topology_bytes);
    let topology_schema_digest = digest::bytes(&topology_schema_bytes);
    let entrypoint_reviews_digest = digest::bytes(&entrypoint_reviews_bytes);
    let entrypoint_reviews_schema_digest =
        digest::bytes(&entrypoint_reviews_schema_bytes);
    let dependency_reviews_digest = digest::bytes(&reviews_bytes);
    let dependency_reviews_schema_digest = digest::bytes(&reviews_schema_bytes);
    let product_closures_digest = digest::bytes(&product_closures_bytes);
    let product_closures_schema_digest =
        digest::bytes(&product_closures_schema_bytes);
    let execution_tool_reviews_digest =
        digest::bytes(&execution_tool_reviews_bytes);
    let execution_tool_reviews_schema_digest =
        digest::bytes(&execution_tool_reviews_schema_bytes);
    let report_schema_digest = digest::bytes(&report_schema_bytes);
    let mut checker_identity = v2_tool_identity(
        &checker_path,
        &checker_digest,
        &checker_version,
        topology.limits.tool_executable_bytes,
    )?;
    let mut cargo_identity = v2_tool_identity(
        &cargo,
        &cargo_digest_after,
        &cargo_version,
        topology.limits.tool_executable_bytes,
    )?;
    if cargo_identity.executable_digest != cargo_digest_before {
        return Err(Fatal::new(
            "CARGO_IDENTITY_DRIFT",
            "Cargo changed after final identity check",
        ));
    }
    let mut rustc_identity = v2_tool_identity(
        &rustc,
        &rustc_digest_after,
        &rustc_version,
        topology.limits.tool_executable_bytes,
    )?;
    if rustc_identity.executable_digest != rustc_digest_before {
        return Err(Fatal::new(
            "RUSTC_IDENTITY_DRIFT",
            "Rustc changed after final identity check",
        ));
    }
    if let Some(product) = &product_evidence {
        checker_identity
            .runtime_inputs
            .clone_from(&product.checker_runtime_inputs);
        cargo_identity.runtime_inputs.clone_from(&product.cargo_runtime_inputs);
        rustc_identity.runtime_inputs.clone_from(&product.rustc_runtime_inputs);
    }
    let source_observations =
        report_build_v2::observe_source_files(&root, &inventory)
            .map_err(|error| Fatal::new(error.code, error.message))?;
    let metadata_evidence = profiles
        .iter()
        .zip(&graphs)
        .map(|((workspace_id, profile, initial), graph)| {
            report_build_v2::MetadataEvidence {
                workspace: workspace_id,
                profile: &profile.name,
                initial,
                final_capture: final_profiles
                    .iter()
                    .find(|(candidate_workspace, candidate, _)| {
                        candidate_workspace == workspace_id
                            && candidate.name == profile.name
                    })
                    .map(|(_, _, capture)| capture),
                graph,
            }
        })
        .collect::<Vec<_>>();
    let report_mode = match (&options.mode, &git_evidence) {
        (Mode::Candidate, None) => report_build_v2::Mode::Candidate,
        (Mode::Exact { execution_environment, .. }, Some(evidence)) => {
            report_build_v2::Mode::Exact {
                revision: &evidence.revision,
                git: evidence,
                execution_environment,
            }
        }
        _ => unreachable!("mode and Git evidence were validated above"),
    };
    let active_product =
        product_evidence.as_ref().map(product_proof::Observation::active);
    let report = report_build_v2::build(&report_build_v2::BuildInputs {
        repository_root: &root,
        mode: report_mode,
        artifacts: report_build_v2::ArtifactInputs {
            source_classification: report_build_v2::ArtifactIdentity {
                path: Path::new(POLICY_KEY),
                schema: policy::SCHEMA,
                content_digest: &policy_digest,
                schema_path: Path::new(POLICY_SCHEMA_KEY),
                schema_digest: &policy_schema_digest,
            },
            workspace_topology: report_build_v2::ArtifactIdentity {
                path: Path::new(TOPOLOGY_KEY),
                schema: topology::SCHEMA,
                content_digest: &topology_digest,
                schema_path: Path::new(TOPOLOGY_SCHEMA_KEY),
                schema_digest: &topology_schema_digest,
            },
            entrypoint_reviews: report_build_v2::ArtifactIdentity {
                path: Path::new(ENTRYPOINT_REVIEWS_KEY),
                schema: entrypoint_reviews::SCHEMA,
                content_digest: &entrypoint_reviews_digest,
                schema_path: Path::new(ENTRYPOINT_REVIEWS_SCHEMA_KEY),
                schema_digest: &entrypoint_reviews_schema_digest,
            },
            dependency_reviews: report_build_v2::ArtifactIdentity {
                path: Path::new(DEPENDENCY_REVIEWS_KEY),
                schema: reviews::SCHEMA,
                content_digest: &dependency_reviews_digest,
                schema_path: Path::new(DEPENDENCY_REVIEWS_SCHEMA_KEY),
                schema_digest: &dependency_reviews_schema_digest,
            },
            product_closures: report_build_v2::ArtifactIdentity {
                path: Path::new(PRODUCT_CLOSURES_KEY),
                schema: product_closures::SCHEMA,
                content_digest: &product_closures_digest,
                schema_path: Path::new(PRODUCT_CLOSURES_SCHEMA_KEY),
                schema_digest: &product_closures_schema_digest,
            },
            execution_tool_reviews: report_build_v2::ArtifactIdentity {
                path: Path::new(EXECUTION_TOOL_REVIEWS_KEY),
                schema: execution_tools::SCHEMA,
                content_digest: &execution_tool_reviews_digest,
                schema_path: Path::new(EXECUTION_TOOL_REVIEWS_SCHEMA_KEY),
                schema_digest: &execution_tool_reviews_schema_digest,
            },
        },
        report_schema_digest: &report_schema_digest,
        policy: &policy,
        topology: &topology,
        inventory: &inventory,
        source_observations: &source_observations,
        product_closures: &product_registry,
        execution_tools: &execution_tool_registry,
        checker: &checker_identity,
        cargo: &cargo_identity,
        rustc: &rustc_identity,
        checker_version: &checker_version,
        cargo_version: &cargo_version,
        rustc_version: &rustc_version,
        metadata: &metadata_evidence,
        active_product: active_product.as_ref(),
        additional_findings: &findings,
    })
    .map_err(|error| Fatal::new(error.code, error.message))?;
    let verified = report_v2::verify(
        &report.bytes,
        topology.limits.report_bytes,
        topology.limits.json_depth,
        topology.limits.json_container_items,
    )
    .map_err(|error| Fatal::new("REPORT_SELF_VERIFY", error.to_string()))?;
    let expected_mode = if matches!(options.mode, Mode::Candidate) {
        "candidate"
    } else {
        "exact"
    };
    if verified.identity != report.identity
        || verified.mode != expected_mode
        || verified.status != if findings.is_empty() { "pass" } else { "fail" }
    {
        return Err(Fatal::new(
            "REPORT_SELF_VERIFY",
            concat!(
                "constructed report identity, mode, or status disagrees ",
                "with semantic verification",
            ),
        ));
    }
    let exact_proven = git_evidence
        .as_ref()
        .map(|evidence| ExactProven { revision: evidence.revision.clone() });
    Ok(Outcome { clean: findings.is_empty(), report, root, exact_proven })
}

fn static_proof_failure(findings: &[Finding]) -> String {
    let codes = findings
        .iter()
        .map(|finding| finding.code)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(8)
        .collect::<Vec<_>>()
        .join(",");
    let samples = findings
        .iter()
        .take(8)
        .map(finding_sample)
        .collect::<Vec<_>>()
        .join("|");
    format!(
        concat!(
            "active Product execution requires a finding-free static ",
            "source, graph, and review preflight; count={}; codes={}; ",
            "samples={}"
        ),
        findings.len(),
        codes,
        samples
    )
}

fn finding_sample(finding: &Finding) -> String {
    let path = finding.path.as_deref().unwrap_or("-");
    let message = finding.message.chars().take(160).collect::<String>();
    format!("{}@{}:{}", finding.code, path, message)
}

fn require_metadata_unchanged(
    profile: &topology::Profile,
    initial: &cargo::Capture,
    final_capture: &cargo::Capture,
) -> Result<(), Fatal> {
    if final_capture.raw_digest != initial.raw_digest
        || final_capture.metadata != initial.metadata
    {
        return Err(Fatal::new(
            "CARGO_METADATA_DRIFT",
            format!("profile {} metadata changed during the run", profile.name),
        ));
    }
    Ok(())
}

fn execution_profile(profile: &topology::MetadataProfile) -> topology::Profile {
    topology::Profile {
        name: profile.id.clone(),
        target: profile.target_triple.clone(),
        all_features: profile.all_features,
        no_default_features: profile.no_default_features,
        features: profile.features.clone(),
    }
}

fn validate_product_registry(
    topology: &Topology,
    inventory: &source::Inventory,
    registry: &product_closures::Registry,
) -> Result<(), Fatal> {
    let topology_has_product = topology.workspaces.values().any(|workspace| {
        workspace.partition == topology::WorkspacePartition::Product
    });
    let inventory_has_product = inventory.files.iter().any(|file| {
        matches!(
            file.classification.owner,
            reovim_depgraph::SourceOwner::Product(_)
        )
    });
    if registry.closures.is_empty() {
        if topology_has_product || inventory_has_product {
            return Err(Fatal::new(
                "PRODUCT_CLOSURE_MISSING",
                concat!(
                    "Product topology or source exists without an exact ",
                    "Product closure",
                ),
            ));
        }
        if !registry.public_consumer_relations.is_empty() {
            return Err(Fatal::new(
                "PRODUCT_RELATION_WITHOUT_CLOSURE",
                concat!(
                    "public-consumer relations exist while the Product ",
                    "closure set is empty",
                ),
            ));
        }
        return Ok(());
    }
    if !topology_has_product || !inventory_has_product {
        return Err(Fatal::new(
            "PRODUCT_CLOSURE_UNMATCHED",
            concat!(
                "a Product closure exists without matching Product ",
                "topology and source",
            ),
        ));
    }

    for closure in &registry.closures {
        validate_product_closure(topology, inventory, closure)?;
    }
    validate_public_consumer_relations(&registry.public_consumer_relations)?;
    Ok(())
}

fn validate_product_closure(
    topology: &Topology,
    inventory: &source::Inventory,
    closure: &product_closures::Closure,
) -> Result<(), Fatal> {
    let workspace =
        topology.workspaces.get(&closure.workspace.id).ok_or_else(|| {
            Fatal::new(
                "PRODUCT_CLOSURE_WORKSPACE",
                format!("closure {} names an unknown workspace", closure.id),
            )
        })?;
    if workspace.partition != topology::WorkspacePartition::Product
        || workspace.workspace_manifest != closure.workspace.manifest
        || workspace.lockfile != closure.workspace.lockfile
    {
        return Err(Fatal::new(
            "PRODUCT_CLOSURE_WORKSPACE",
            format!(
                "closure {} workspace identity differs from topology",
                closure.id
            ),
        ));
    }
    let package =
        workspace.packages.get(&closure.package.id).ok_or_else(|| {
            Fatal::new(
                "PRODUCT_CLOSURE_PACKAGE",
                format!("closure {} names an unknown package", closure.id),
            )
        })?;
    if package.name != closure.package.name
        || package.manifest != closure.package.manifest
    {
        return Err(Fatal::new(
            "PRODUCT_CLOSURE_PACKAGE",
            format!(
                "closure {} package identity differs from topology",
                closure.id
            ),
        ));
    }
    let target = package.targets.get(&closure.target.id).ok_or_else(|| {
        Fatal::new(
            "PRODUCT_CLOSURE_TARGET",
            format!("closure {} names an unknown target", closure.id),
        )
    })?;
    if target.name != closure.target.name
        || target.src_path != closure.target.src_path
        || target.kind != closure.target.kind
        || target.crate_types != closure.target.crate_types
        || !workspace.metadata_profiles.contains_key(&closure.cargo.profile)
    {
        return Err(Fatal::new(
            "PRODUCT_CLOSURE_TARGET",
            format!(
                "closure {} target/profile differs from topology",
                closure.id
            ),
        ));
    }
    let source = inventory
        .files
        .iter()
        .find(|file| file.path == closure.target.src_path)
        .ok_or_else(|| {
            Fatal::new(
                "PRODUCT_CLOSURE_SOURCE",
                format!(
                    "closure {} target source is absent from inventory",
                    closure.id
                ),
            )
        })?;
    if source.classification.owner
        != reovim_depgraph::SourceOwner::Product(closure.architectural_role)
        || source.classification.source_use != closure.source_use
    {
        return Err(Fatal::new(
            "PRODUCT_CLOSURE_SOURCE",
            format!(
                "closure {} source role/use differs from classification",
                closure.id
            ),
        ));
    }
    validate_package_member_sources(inventory, closure)?;
    let _ = closure.kind;
    Ok(())
}

fn validate_package_member_sources(
    inventory: &source::Inventory,
    closure: &product_closures::Closure,
) -> Result<(), Fatal> {
    let Some(membership) = &closure.package_membership else { return Ok(()) };
    for member in &membership.members {
        if let product_closures::PackageMember::Copied {
            repository_path, ..
        } = member
            && !inventory.files.iter().any(|file| file.path == *repository_path)
        {
            return Err(Fatal::new(
                "PRODUCT_PACKAGE_MEMBER_SOURCE",
                format!(
                    "closure {} copied member {} is absent from inventory",
                    closure.id,
                    repository_path.display()
                ),
            ));
        }
        let _ = member.package_path();
    }
    Ok(())
}

fn validate_public_consumer_relations(
    relations: &[product_closures::PublicConsumerRelation],
) -> Result<(), Fatal> {
    for relation in relations {
        if relation.public_items.is_empty() {
            return Err(Fatal::new(
                "PRODUCT_PUBLIC_ITEMS",
                format!(
                    "relation {} has no declared public items",
                    relation.id
                ),
            ));
        }
        for item in &relation.public_items {
            let _ = (&item.path, item.kind);
        }
        for check in &relation.derive_checks {
            let _ =
                (check.derive, &check.public_type, check.consumer_operation);
        }
        let _ = (&relation.shipping_closure, &relation.consumer_closure);
    }
    Ok(())
}

fn capture_checker_version(
    checker: &Path,
    root: &Path,
    limits: process::ProcessLimits,
) -> Result<cargo::VersionCapture, Fatal> {
    let process = process::run(
        &process::CommandSpec {
            program: checker.to_path_buf(),
            arguments: vec!["--version".into()],
            current_dir: root.to_path_buf(),
            environment: std::collections::BTreeMap::from([(
                std::ffi::OsString::from("TERM"),
                std::ffi::OsString::from("dumb"),
            )]),
            clear_environment: true,
        },
        limits,
    )
    .map_err(|error| Fatal::new("CHECKER_VERSION_RUN", error.to_string()))?;
    if process.termination_reason != process::TerminationReason::Exited
        || !process.status.success()
        || process.stdout_overflow
        || process.stderr_overflow
        || process.stdout_read_error.is_some()
        || process.stderr_read_error.is_some()
        || !process.stderr.is_empty()
        || !process.reaped
        || !process.readers_joined
    {
        return Err(Fatal::new(
            "CHECKER_VERSION_PROCESS",
            concat!(
                "checker --version did not complete through the bounded ",
                "owned process contract",
            ),
        ));
    }
    let raw = std::str::from_utf8(&process.stdout).map_err(|error| {
        Fatal::new("CHECKER_VERSION_UTF8", error.to_string())
    })?;
    if !raw.ends_with('\n') || raw.trim_end_matches('\n').contains(['\r', '\n'])
    {
        return Err(Fatal::new(
            "CHECKER_VERSION_SHAPE",
            concat!(
                "checker --version must emit exactly one nonempty ",
                "LF-terminated line",
            ),
        ));
    }
    let version = raw.trim_end_matches('\n').to_owned();
    if version != concat!("reovim-conformance ", env!("CARGO_PKG_VERSION")) {
        return Err(Fatal::new(
            "CHECKER_VERSION_VALUE",
            "checker --version differs from the compiled package identity",
        ));
    }
    let version_digest = digest::bytes(&process.stdout);
    Ok(cargo::VersionCapture { process, version, version_digest })
}

fn v2_tool_identity(
    executable: &Path,
    executable_digest: &str,
    version: &cargo::VersionCapture,
    executable_byte_limit: u64,
) -> Result<report_build_v2::ToolIdentity, Fatal> {
    let metadata = fs::metadata(executable)
        .map_err(|error| Fatal::new("TOOL_IDENTITY", error.to_string()))?;
    if metadata.len() == 0 || metadata.len() > executable_byte_limit {
        return Err(Fatal::new(
            "TOOL_IDENTITY_SIZE",
            format!(
                "{} has size {} outside 1..={executable_byte_limit}",
                executable.display(),
                metadata.len()
            ),
        ));
    }
    let argv = version
        .process
        .argv
        .iter()
        .map(|argument| {
            argument.to_str().map(str::to_owned).ok_or_else(|| {
                Fatal::new(
                    "TOOL_VERSION_ARGV_UTF8",
                    "tool version argv is not UTF-8",
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let executable_text = executable.to_str().ok_or_else(|| {
        Fatal::new("TOOL_PATH_UTF8", "tool executable path is not UTF-8")
    })?;
    if argv.first().map(String::as_str) != Some(executable_text)
        || argv.len() < 2
    {
        return Err(Fatal::new(
            "TOOL_VERSION_ARGV",
            concat!(
                "tool version capture does not begin with its canonical ",
                "executable and arguments",
            ),
        ));
    }
    Ok(report_build_v2::ToolIdentity {
        canonical_path: executable.to_path_buf(),
        executable_digest: executable_digest.to_owned(),
        executable_size_bytes: metadata.len(),
        version_argv: argv[1..].to_vec(),
        version: version.version.clone(),
        version_digest: version.version_digest.clone(),
        runtime_inputs: Vec::new(),
    })
}

fn validate_execution_tool_review_records(
    root: &Path,
    inventory: &source::Inventory,
    registry: &execution_tools::Registry,
    file_byte_limit: u64,
) -> Result<(), Fatal> {
    if registry.reviewed_at.is_empty() {
        return Err(Fatal::new(
            "EXECUTION_TOOL_REVIEW",
            "review timestamp is empty",
        ));
    }
    for review in &registry.reviews {
        let record = inventory
            .files
            .iter()
            .find(|file| file.path == review.review_record)
            .ok_or_else(|| {
                Fatal::new(
                    "EXECUTION_TOOL_REVIEW_RECORD",
                    format!(
                        "review {} record is absent from source inventory",
                        review.id
                    ),
                )
            })?;
        if record.content_digest != review.review_record_digest
            || record.classification.owner
                != reovim_depgraph::SourceOwner::RepositoryTooling
            || record.classification.provenance
                != reovim_depgraph::SourceProvenance::Authored
            || record.classification.source_use
                != reovim_depgraph::SourceUse::Tool
        {
            return Err(Fatal::new(
                "EXECUTION_TOOL_REVIEW_RECORD",
                format!(
                    "review {} record identity/classification differs",
                    review.id
                ),
            ));
        }
        let actual = digest::file_bounded(
            &root.join(&review.review_record),
            file_byte_limit,
        )
        .map_err(|error| {
            Fatal::new("EXECUTION_TOOL_REVIEW_RECORD", error.to_string())
        })?;
        if actual != review.review_record_digest {
            return Err(Fatal::new(
                "EXECUTION_TOOL_REVIEW_RECORD",
                format!("review {} record bytes changed", review.id),
            ));
        }
        let _ = (
            review.logical_role,
            &review.invocation_families,
            &review.required_semantics,
            &review.consumers,
            &review.product_profiles,
            &review.upstream,
            &review.license,
            &review.assumptions,
            &review.invalidators,
        );
    }
    Ok(())
}

fn select_git_binding<'a>(
    environment_id: &str,
    git: &Path,
    executable_digest: &str,
    registry: &'a execution_tools::Registry,
    executable_byte_limit: u64,
) -> Result<&'a execution_tools::Binding, Fatal> {
    let environment = registry
        .environments
        .iter()
        .find(|environment| environment.id == environment_id)
        .ok_or_else(|| {
            Fatal::new(
                "EXECUTION_TOOL_ENVIRONMENT",
                format!(
                    "unknown execution-tool environment {environment_id:?}"
                ),
            )
        })?;
    let bindings = environment
        .bindings
        .iter()
        .filter(|binding| binding.canonical_path == git)
        .collect::<Vec<_>>();
    let [binding] = bindings.as_slice() else {
        return Err(Fatal::new(
            "EXECUTION_TOOL_BINDING",
            concat!(
                "selected environment must contain exactly one binding ",
                "for canonical Git",
            ),
        ));
    };
    let review = registry
        .reviews
        .iter()
        .find(|review| review.id == binding.review)
        .ok_or_else(|| {
            Fatal::new("EXECUTION_TOOL_REVIEW", "Git binding review is absent")
        })?;
    let expected_families = BTreeSet::from([
        execution_tools::InvocationFamily::GitVersion,
        execution_tools::InvocationFamily::GitRevision,
        execution_tools::InvocationFamily::GitTree,
        execution_tools::InvocationFamily::GitIndex,
        execution_tools::InvocationFamily::GitAttributes,
    ]);
    if review.logical_role != execution_tools::LogicalRole::Git
        || review.invocation_families.iter().copied().collect::<BTreeSet<_>>()
            != expected_families
        || !review.product_profiles.is_empty()
        || binding.prepared_path.is_some()
        || binding.version_argv != ["--version"]
    {
        return Err(Fatal::new(
            "EXECUTION_TOOL_BINDING",
            concat!(
                "Git review families/profile/prepared-path/version argv ",
                "differ from the exact contract",
            ),
        ));
    }
    let metadata = fs::metadata(git).map_err(|error| {
        Fatal::new("EXECUTION_TOOL_BINDING", error.to_string())
    })?;
    if metadata.len() != binding.executable_size_bytes
        || executable_digest != binding.executable_digest
        || digest::file_bounded(git, executable_byte_limit).map_err(
            |error| Fatal::new("EXECUTION_TOOL_BINDING", error.to_string()),
        )? != binding.executable_digest
    {
        return Err(Fatal::new(
            "EXECUTION_TOOL_BINDING",
            "Git executable path/size/digest differs from the selected binding",
        ));
    }
    for runtime in &binding.runtime_inputs {
        let canonical =
            runtime.canonical_path.canonicalize().map_err(|error| {
                Fatal::new("EXECUTION_TOOL_RUNTIME_INPUT", error.to_string())
            })?;
        let metadata = fs::metadata(&canonical).map_err(|error| {
            Fatal::new("EXECUTION_TOOL_RUNTIME_INPUT", error.to_string())
        })?;
        if canonical != runtime.canonical_path
            || metadata.len() != runtime.size_bytes
            || digest::file_bounded(&canonical, executable_byte_limit).map_err(
                |error| {
                    Fatal::new(
                        "EXECUTION_TOOL_RUNTIME_INPUT",
                        error.to_string(),
                    )
                },
            )? != runtime.content_digest
        {
            return Err(Fatal::new(
                "EXECUTION_TOOL_RUNTIME_INPUT",
                format!(
                    "runtime input {} drifted",
                    runtime.canonical_path.display()
                ),
            ));
        }
    }
    Ok(binding)
}

/// Writes an exact passing report into a content-addressed, no-clobber bundle.
///
/// # Errors
///
/// Returns an error unless the outcome is exact, clean, revision-bound, and
/// the canonical out-of-tree destination can be published without clobbering.
/// A post-commit cleanup or durability failure also returns an error:
/// `PublicationError.effect_certainty` distinguishes `NotApplied`, `Applied`,
/// and `OutcomeUnknown`; every post-handoff error retains the intended final
/// path plus cleanup and durability state. Publication owns an initially empty
/// mode-0700 root for a single attempt and permanently consumes it with an
/// authority marker.
/// Same-UID processes that ignore that marker, and host/root authority, are
/// outside this cooperative exclusion contract; detected mutation fails
/// closed.
pub fn write_evidence(
    outcome: &Outcome,
    evidence_root: &Path,
) -> Result<PublicationOutcome, PublicationError> {
    let prepared =
        prepare_evidence(outcome, evidence_root, PublicationFault::None)?;
    publish_prepared(prepared, PublicationFault::None)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum PublicationFault {
    None,
    Create,
    Write,
    FileSync,
    Link,
    Reconcile,
    Unlink,
    DirectorySync,
}

fn publish_prepared(
    prepared: PreparedEvidence,
    fault: PublicationFault,
) -> Result<PublicationOutcome, PublicationError> {
    if let Err(fatal) = prepared.validate_precommit_identity() {
        let cleanup = prepared.cleanup_before_commit();
        return Err(PublicationError::not_applied(
            fatal,
            Some(prepared.final_path),
            cleanup,
        ));
    }
    if fault == PublicationFault::Link {
        let cleanup = prepared.cleanup_before_commit();
        return Err(PublicationError::not_applied(
            Fatal::new("EVIDENCE_PUBLISH", "injected pre-commit link failure"),
            Some(prepared.final_path),
            cleanup,
        ));
    }
    if let Err(error) =
        fs::hard_link(&prepared.temporary_anchored, &prepared.final_anchored)
    {
        match prepared.reconcile_final_link(fault) {
            LinkReconciliation::Applied => {
                let cleanup = prepared.cleanup_temporary_after_commit();
                let durability = sync_publication_directories(&prepared);
                let fatal = reconciled_publication_fatal(&prepared, &error);
                return Err(PublicationError::applied(
                    fatal,
                    prepared.final_path,
                    cleanup,
                    durability,
                ));
            }
            LinkReconciliation::NotApplied => {
                let cleanup = prepared.cleanup_before_commit();
                return Err(PublicationError::not_applied(
                    Fatal::new("EVIDENCE_PUBLISH", error.to_string()),
                    Some(prepared.final_path),
                    cleanup,
                ));
            }
            LinkReconciliation::OutcomeUnknown(fatal) => {
                let cleanup = prepared.cleanup_before_commit();
                return Err(PublicationError::outcome_unknown(
                    fatal,
                    prepared.final_path,
                    cleanup,
                ));
            }
        }
    }
    let cleanup = if fault == PublicationFault::Unlink {
        CleanupState::Incomplete
    } else {
        prepared.cleanup_temporary_after_commit()
    };
    let durability = if fault == PublicationFault::DirectorySync {
        DurabilityState::Unknown
    } else {
        sync_publication_directories(&prepared)
    };
    if let Err(fatal) = prepared.validate_committed_identity() {
        return Err(PublicationError::applied(
            fatal,
            prepared.final_path,
            cleanup,
            durability,
        ));
    }
    if cleanup != CleanupState::Complete {
        return Err(PublicationError::applied(
            Fatal::new(
                "EVIDENCE_POST_COMMIT_CLEANUP",
                concat!(
                    "evidence is published but its owned temporary file ",
                    "could not be removed",
                ),
            ),
            prepared.final_path,
            cleanup,
            durability,
        ));
    }
    if durability != DurabilityState::DirectorySynced {
        return Err(PublicationError::applied(
            Fatal::new(
                "EVIDENCE_POST_COMMIT_SYNC",
                concat!(
                    "evidence is published but directory durability ",
                    "could not be established",
                ),
            ),
            prepared.final_path,
            cleanup,
            durability,
        ));
    }
    Ok(PublicationOutcome { path: prepared.final_path, cleanup, durability })
}

fn reconciled_publication_fatal(
    prepared: &PreparedEvidence,
    link_error: &std::io::Error,
) -> Fatal {
    prepared.validate_committed_identity().err().unwrap_or_else(|| {
        Fatal::new(
            "EVIDENCE_PUBLISH_RECONCILED",
            format!(
                "evidence final link is applied despite hard-link error \
                 response: {link_error}"
            ),
        )
    })
}

struct DirectoryAnchor {
    nominal_path: PathBuf,
    handle: fs::File,
    owned_entry: Option<PathBuf>,
}

impl DirectoryAnchor {
    fn open(
        nominal_path: PathBuf,
        access_path: &Path,
        owned_entry: Option<PathBuf>,
    ) -> Result<Self, Fatal> {
        require_publication_platform()?;
        let before = plain_directory_metadata(access_path)?;
        let handle = fs::File::open(access_path).map_err(|error| {
            Fatal::new("EVIDENCE_DIRECTORY_OPEN", error.to_string())
        })?;
        let anchor = Self { nominal_path, handle, owned_entry };
        if !same_file_identity(
            &before,
            &anchor.handle.metadata().map_err(|error| {
                Fatal::new("EVIDENCE_DIRECTORY_IDENTITY", error.to_string())
            })?,
        ) {
            return Err(Fatal::new(
                "EVIDENCE_DIRECTORY_IDENTITY",
                format!(
                    "{} changed while it was opened",
                    access_path.display()
                ),
            ));
        }
        anchor.validate_nominal_identity()?;
        let anchored = anchor.anchored_path()?;
        let anchored_metadata = fs::metadata(&anchored).map_err(|error| {
            Fatal::new("EVIDENCE_DIRECTORY_ANCHOR", error.to_string())
        })?;
        if !same_file_identity(
            &anchor.handle.metadata().map_err(|error| {
                Fatal::new("EVIDENCE_DIRECTORY_IDENTITY", error.to_string())
            })?,
            &anchored_metadata,
        ) {
            return Err(Fatal::new(
                "EVIDENCE_DIRECTORY_ANCHOR",
                format!(
                    "{} does not resolve to its held directory",
                    anchored.display()
                ),
            ));
        }
        Ok(anchor)
    }

    // Linux sees only the successful arm; non-Linux retains the typed
    // failure.
    #[cfg_attr(target_os = "linux", allow(clippy::unnecessary_wraps))]
    fn anchored_path(&self) -> Result<PathBuf, Fatal> {
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd as _;
            Ok(PathBuf::from(format!(
                "/proc/self/fd/{}",
                self.handle.as_raw_fd()
            )))
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(Fatal::new(
                "EVIDENCE_PLATFORM",
                concat!(
                    "held-directory evidence publication requires Linux ",
                    "/proc/self/fd",
                ),
            ))
        }
    }

    fn child(&self, name: &str) -> Result<PathBuf, Fatal> {
        Ok(self.anchored_path()?.join(name))
    }

    fn validate_nominal_identity(&self) -> Result<(), Fatal> {
        let nominal = plain_directory_metadata(&self.nominal_path)?;
        let held = self.handle.metadata().map_err(|error| {
            Fatal::new("EVIDENCE_DIRECTORY_IDENTITY", error.to_string())
        })?;
        if same_file_identity(&nominal, &held) {
            Ok(())
        } else {
            Err(Fatal::new(
                "EVIDENCE_DIRECTORY_SUBSTITUTED",
                format!(
                    "{} no longer names the held publication directory",
                    self.nominal_path.display()
                ),
            ))
        }
    }

    fn cleanup_owned_entry(&self) -> bool {
        let Some(entry) = &self.owned_entry else { return true };
        if fs::symlink_metadata(entry).is_ok_and(|metadata| {
            !metadata.file_type().is_symlink()
                && metadata.file_type().is_dir()
                && self
                    .handle
                    .metadata()
                    .is_ok_and(|held| same_file_identity(&metadata, &held))
        }) {
            let _ = fs::remove_dir(entry);
        }
        self.handle
            .metadata()
            .is_ok_and(|metadata| file_link_count(&metadata) == 0)
    }
}

#[derive(Debug)]
struct AuthorityMarker {
    anchored_path: PathBuf,
    handle: fs::File,
    owner_uid: u32,
}

impl AuthorityMarker {
    fn validate(&self) -> Result<(), Fatal> {
        let path_metadata =
            fs::symlink_metadata(&self.anchored_path).map_err(|error| {
                Fatal::new("EVIDENCE_AUTHORITY_MARKER", error.to_string())
            })?;
        let held_metadata = self.handle.metadata().map_err(|error| {
            Fatal::new("EVIDENCE_AUTHORITY_MARKER", error.to_string())
        })?;
        let (owner_uid, mode) = publication_owner_and_mode(&path_metadata)?;
        if path_metadata.file_type().is_symlink()
            || !path_metadata.file_type().is_file()
            || !same_file_identity(&path_metadata, &held_metadata)
            || file_link_count(&held_metadata) != 1
            || owner_uid != self.owner_uid
            || mode != 0o600
            || held_metadata.len()
                != EVIDENCE_AUTHORITY_MARKER_BYTES.len() as u64
        {
            return Err(Fatal::new(
                "EVIDENCE_AUTHORITY_MARKER",
                concat!(
                    "the permanent evidence-root authority marker no ",
                    "longer names its held 0600 inode",
                ),
            ));
        }
        Ok(())
    }
}

fn current_publication_uid() -> Result<u32, Fatal> {
    require_publication_platform()?;
    let status = fs::read_to_string("/proc/self/status").map_err(|error| {
        Fatal::new("EVIDENCE_CURRENT_UID", error.to_string())
    })?;
    let uid_line = status
        .lines()
        .find(|line| line.starts_with("Uid:"))
        .ok_or_else(|| {
            Fatal::new(
                "EVIDENCE_CURRENT_UID",
                "/proc/self/status has no Uid field",
            )
        })?;
    let ids = uid_line
        .split_ascii_whitespace()
        .skip(1)
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            Fatal::new("EVIDENCE_CURRENT_UID", error.to_string())
        })?;
    let Some((&first, remaining)) = ids.split_first() else {
        return Err(Fatal::new(
            "EVIDENCE_CURRENT_UID",
            "/proc/self/status has an empty Uid field",
        ));
    };
    if ids.len() != 4 || remaining.iter().any(|uid| *uid != first) {
        return Err(Fatal::new(
            "EVIDENCE_CURRENT_UID",
            concat!(
                "evidence publication requires matching real, effective, ",
                "saved, and filesystem UIDs",
            ),
        ));
    }
    Ok(first)
}

// The Result is load-bearing for the fail-closed non-Linux build.
#[allow(clippy::unnecessary_wraps)]
fn publication_owner_and_mode(
    metadata: &fs::Metadata,
) -> Result<(u32, u32), Fatal> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
        Ok((metadata.uid(), metadata.permissions().mode() & 0o7777))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = metadata;
        Err(Fatal::new(
            "EVIDENCE_PLATFORM",
            "evidence authority metadata requires Linux",
        ))
    }
}

fn validate_evidence_root_authority(
    evidence_root: &Path,
) -> Result<u32, Fatal> {
    let current_uid = current_publication_uid()?;
    let root_metadata = plain_directory_metadata(evidence_root)?;
    let (root_uid, root_mode) = publication_owner_and_mode(&root_metadata)?;
    if root_uid != current_uid || root_mode != 0o700 {
        return Err(Fatal::new(
            "EVIDENCE_ROOT_AUTHORITY",
            format!(
                "{} must be owned by the current UID and have exact mode 0700",
                evidence_root.display()
            ),
        ));
    }
    let parent = evidence_root
        .parent()
        .filter(|parent| *parent != evidence_root)
        .ok_or_else(|| {
            Fatal::new(
                "EVIDENCE_ROOT_PARENT",
                "evidence root must have a protecting parent",
            )
        })?;
    let parent_metadata = plain_directory_metadata(parent)?;
    let (parent_uid, parent_mode) =
        publication_owner_and_mode(&parent_metadata)?;
    let sticky = parent_mode & 0o1000 != 0;
    let sticky_authority =
        sticky && (parent_uid == 0 || parent_uid == current_uid);
    let private_current_uid_authority =
        parent_uid == current_uid && parent_mode & 0o022 == 0;
    if !sticky_authority && !private_current_uid_authority {
        return Err(Fatal::new(
            "EVIDENCE_ROOT_PARENT",
            format!(
                concat!(
                    "{} must be current/root-owned and sticky, or ",
                    "current-UID-owned without group/other write access",
                ),
                parent.display()
            ),
        ));
    }
    Ok(current_uid)
}

fn validate_directory_entries(
    directory: &DirectoryAnchor,
    expected: &[&str],
    code: &'static str,
) -> Result<(), Fatal> {
    let anchored = directory.anchored_path()?;
    let mut seen = vec![false; expected.len()];
    let entries = fs::read_dir(&anchored)
        .map_err(|error| Fatal::new(code, error.to_string()))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| Fatal::new(code, error.to_string()))?;
        let name = entry.file_name();
        let Some(index) =
            expected.iter().position(|expected| name == *expected)
        else {
            return Err(Fatal::new(
                code,
                format!(
                    "{} contains an unowned entry",
                    directory.nominal_path.display()
                ),
            ));
        };
        if seen[index] {
            return Err(Fatal::new(
                code,
                "evidence root contains a duplicate authority entry",
            ));
        }
        seen[index] = true;
    }
    if seen.iter().all(|present| *present) {
        Ok(())
    } else {
        Err(Fatal::new(
            code,
            format!(
                "{} is missing an owned entry",
                directory.nominal_path.display()
            ),
        ))
    }
}

fn validate_initial_evidence_root(
    evidence_root: &DirectoryAnchor,
) -> Result<(), Fatal> {
    let mut entries = fs::read_dir(evidence_root.anchored_path()?)
        .map_err(|error| Fatal::new("EVIDENCE_ROOT_IO", error.to_string()))?;
    match entries.next() {
        None => Ok(()),
        Some(Err(error)) => {
            Err(Fatal::new("EVIDENCE_ROOT_IO", error.to_string()))
        }
        Some(Ok(entry)) => {
            let marker_path = evidence_root.child(EVIDENCE_AUTHORITY_MARKER)?;
            let marker_present = entry.file_name() == EVIDENCE_AUTHORITY_MARKER
                || match fs::symlink_metadata(&marker_path) {
                    Ok(_) => true,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::NotFound =>
                    {
                        false
                    }
                    Err(error) => {
                        return Err(Fatal::new(
                            "EVIDENCE_ROOT_IO",
                            error.to_string(),
                        ));
                    }
                };
            if marker_present {
                Err(Fatal::new(
                    "EVIDENCE_ROOT_CONSUMED",
                    concat!(
                        "evidence root was already consumed; same-UID ",
                        "publishers must honor the permanent authority ",
                        "marker",
                    ),
                ))
            } else {
                Err(Fatal::new(
                    "EVIDENCE_ROOT_NOT_EMPTY",
                    concat!(
                        "evidence root must be exactly empty before its ",
                        "one publication attempt",
                    ),
                ))
            }
        }
    }
}

fn create_authority_marker(
    evidence_root: &DirectoryAnchor,
    owner_uid: u32,
) -> Result<AuthorityMarker, Fatal> {
    require_publication_platform()?;
    let anchored_path = evidence_root.child(EVIDENCE_AUTHORITY_MARKER)?;
    #[cfg(target_os = "linux")]
    let opened = {
        use std::os::unix::fs::OpenOptionsExt as _;
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create_new(true).mode(0o600);
        match options.open(&anchored_path) {
            Ok(handle) => {
                set_authority_marker_permissions(&handle)?;
                handle
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(Fatal::new(
                    "EVIDENCE_ROOT_CONSUMED",
                    concat!(
                        "a competing publication consumed the evidence ",
                        "root authority marker",
                    ),
                ));
            }
            Err(error) => {
                return Err(Fatal::new(
                    "EVIDENCE_AUTHORITY_MARKER",
                    error.to_string(),
                ));
            }
        }
    };
    #[cfg(not(target_os = "linux"))]
    let opened = return Err(Fatal::new(
        "EVIDENCE_PLATFORM",
        "evidence authority marker creation requires Linux",
    ));
    let mut handle = opened;
    handle.write_all(EVIDENCE_AUTHORITY_MARKER_BYTES).map_err(|error| {
        Fatal::new(
            "EVIDENCE_AUTHORITY_MARKER",
            format!(
                "authority marker write failed and consumed the root: {error}"
            ),
        )
    })?;
    handle.sync_all().map_err(|error| {
        Fatal::new(
            "EVIDENCE_AUTHORITY_MARKER",
            format!(
                "authority marker sync failed and consumed the root: {error}"
            ),
        )
    })?;
    evidence_root.handle.sync_all().map_err(|error| {
        Fatal::new(
            "EVIDENCE_AUTHORITY_MARKER",
            format!(
                concat!(
                    "authority marker directory sync failed and consumed ",
                    "the root: {error}",
                ),
                error = error
            ),
        )
    })?;
    let marker = AuthorityMarker { anchored_path, handle, owner_uid };
    marker.validate()?;
    Ok(marker)
}

#[cfg(target_os = "linux")]
fn set_authority_marker_permissions(handle: &fs::File) -> Result<(), Fatal> {
    use std::os::unix::fs::PermissionsExt as _;

    handle.set_permissions(fs::Permissions::from_mode(0o600)).map_err(|error| {
        Fatal::new(
            "EVIDENCE_AUTHORITY_MARKER",
            format!(
                "authority marker permission setup failed and consumed \
                     the root: {error}"
            ),
        )
    })
}

fn claim_evidence_root(
    evidence_root: &DirectoryAnchor,
    owner_uid: u32,
) -> Result<AuthorityMarker, Fatal> {
    validate_initial_evidence_root(evidence_root)?;
    let marker = create_authority_marker(evidence_root, owner_uid)?;
    marker.validate()?;
    validate_directory_entries(
        evidence_root,
        &[EVIDENCE_AUTHORITY_MARKER],
        "EVIDENCE_ROOT_CONSUMED",
    )?;
    Ok(marker)
}

struct PreparedEvidence {
    evidence_root: DirectoryAnchor,
    authority_marker: AuthorityMarker,
    bundle: DirectoryAnchor,
    directory: DirectoryAnchor,
    temporary_anchored: PathBuf,
    temporary_handle: fs::File,
    final_path: PathBuf,
    final_anchored: PathBuf,
}

enum LinkReconciliation {
    Applied,
    NotApplied,
    OutcomeUnknown(Fatal),
}

impl PreparedEvidence {
    fn cleanup_before_commit(&self) -> CleanupState {
        let temporary_clean = cleanup_owned_file(
            &self.temporary_anchored,
            &self.temporary_handle,
            0,
        );
        let directory_clean = self.directory.cleanup_owned_entry();
        let bundle_clean = self.bundle.cleanup_owned_entry();
        if temporary_clean && directory_clean && bundle_clean {
            CleanupState::Complete
        } else {
            CleanupState::Incomplete
        }
    }

    fn cleanup_temporary_after_commit(&self) -> CleanupState {
        if cleanup_owned_file(
            &self.temporary_anchored,
            &self.temporary_handle,
            1,
        ) {
            CleanupState::Complete
        } else {
            CleanupState::Incomplete
        }
    }

    fn validate_directory_identity(&self) -> Result<(), Fatal> {
        self.evidence_root.validate_nominal_identity()?;
        self.authority_marker.validate()?;
        let bundle_name = self
            .bundle
            .nominal_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                Fatal::new(
                    "EVIDENCE_ROOT_CONTENTS",
                    "evidence bundle name is not UTF-8",
                )
            })?;
        validate_directory_entries(
            &self.evidence_root,
            &[EVIDENCE_AUTHORITY_MARKER, bundle_name],
            "EVIDENCE_ROOT_CONTENTS",
        )?;
        self.bundle.validate_nominal_identity()?;
        self.directory.validate_nominal_identity()
    }

    fn validate_committed_identity(&self) -> Result<(), Fatal> {
        self.validate_directory_identity()?;
        let final_metadata = fs::symlink_metadata(&self.final_anchored)
            .map_err(|error| {
                Fatal::new("EVIDENCE_FINAL_IDENTITY", error.to_string())
            })?;
        if final_metadata.file_type().is_symlink()
            || !final_metadata.file_type().is_file()
        {
            return Err(Fatal::new(
                "EVIDENCE_FINAL_IDENTITY",
                "published evidence is not a regular non-symlink file",
            ));
        }
        let temporary_metadata =
            self.temporary_handle.metadata().map_err(|error| {
                Fatal::new("EVIDENCE_FINAL_IDENTITY", error.to_string())
            })?;
        if !same_file_identity(&final_metadata, &temporary_metadata) {
            return Err(Fatal::new(
                "EVIDENCE_FINAL_IDENTITY",
                concat!(
                    "published evidence does not name the synced owned ",
                    "temporary inode",
                ),
            ));
        }
        Ok(())
    }

    fn validate_precommit_identity(&self) -> Result<(), Fatal> {
        self.validate_directory_identity()?;
        let temporary_metadata = fs::symlink_metadata(&self.temporary_anchored)
            .map_err(|error| {
                Fatal::new("EVIDENCE_TEMPORARY_IDENTITY", error.to_string())
            })?;
        if temporary_metadata.file_type().is_symlink()
            || !temporary_metadata.file_type().is_file()
            || !same_file_identity(
                &temporary_metadata,
                &self.temporary_handle.metadata().map_err(|error| {
                    Fatal::new("EVIDENCE_TEMPORARY_IDENTITY", error.to_string())
                })?,
            )
        {
            return Err(Fatal::new(
                "EVIDENCE_TEMPORARY_SUBSTITUTED",
                concat!(
                    "owned temporary evidence path no longer names its ",
                    "held synced inode",
                ),
            ));
        }
        Ok(())
    }

    fn reconcile_final_link(
        &self,
        fault: PublicationFault,
    ) -> LinkReconciliation {
        if fault == PublicationFault::Reconcile {
            return LinkReconciliation::OutcomeUnknown(Fatal::new(
                "EVIDENCE_LINK_RECONCILE",
                "injected final-link reconciliation probe failure",
            ));
        }
        let final_metadata = match fs::symlink_metadata(&self.final_anchored) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return LinkReconciliation::NotApplied;
            }
            Err(error) => {
                return LinkReconciliation::OutcomeUnknown(Fatal::new(
                    "EVIDENCE_LINK_RECONCILE",
                    format!(
                        concat!(
                            "final-link metadata is indeterminate after ",
                            "handoff: {error}",
                        ),
                        error = error
                    ),
                ));
            }
        };
        if final_metadata.file_type().is_symlink()
            || !final_metadata.file_type().is_file()
        {
            return LinkReconciliation::NotApplied;
        }
        match self.temporary_handle.metadata() {
            Ok(temporary)
                if same_file_identity(&final_metadata, &temporary) =>
            {
                LinkReconciliation::Applied
            }
            Ok(_) => LinkReconciliation::NotApplied,
            Err(error) => LinkReconciliation::OutcomeUnknown(Fatal::new(
                "EVIDENCE_LINK_RECONCILE",
                format!(
                    concat!(
                        "held temporary metadata is indeterminate after ",
                        "handoff: {error}",
                    ),
                    error = error
                ),
            )),
        }
    }
}

fn reject_existing_final(
    evidence_root: &DirectoryAnchor,
    identity: &str,
    final_path: &Path,
) -> Result<(), PublicationError> {
    let final_anchored = evidence_root
        .child(identity)
        .map(|bundle| bundle.join("conformance/source-role-map.json"))
        .map_err(|fatal| {
            PublicationError::not_applied(
                fatal,
                Some(final_path.to_path_buf()),
                CleanupState::Complete,
            )
        })?;
    match fs::symlink_metadata(&final_anchored) {
        Ok(_) => Err(PublicationError::not_applied(
            Fatal::new(
                "EVIDENCE_EXISTS",
                format!("{} already exists", final_path.display()),
            ),
            Some(final_path.to_path_buf()),
            CleanupState::Complete,
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(PublicationError::not_applied(
            Fatal::new("EVIDENCE_DESTINATION_IO", error.to_string()),
            Some(final_path.to_path_buf()),
            CleanupState::Complete,
        )),
    }
}

#[allow(clippy::too_many_lines)]
fn prepare_evidence(
    outcome: &Outcome,
    evidence_root: &Path,
    fault: PublicationFault,
) -> Result<PreparedEvidence, PublicationError> {
    let (canonical_evidence_root, bundle, conformance, final_path, temporary) =
        evidence_paths(outcome, evidence_root).map_err(|fatal| {
            PublicationError::not_applied(fatal, None, CleanupState::Complete)
        })?;
    let owner_uid = validate_evidence_root_authority(&canonical_evidence_root)
        .map_err(|fatal| {
            PublicationError::not_applied(fatal, None, CleanupState::Complete)
        })?;
    let evidence_root_anchor = DirectoryAnchor::open(
        canonical_evidence_root.clone(),
        &canonical_evidence_root,
        None,
    )
    .map_err(|fatal| {
        PublicationError::not_applied(fatal, None, CleanupState::Complete)
    })?;
    reject_existing_final(
        &evidence_root_anchor,
        &outcome.report.identity,
        &final_path,
    )?;
    let authority_marker =
        claim_evidence_root(&evidence_root_anchor, owner_uid).map_err(
            |fatal| {
                PublicationError::not_applied(
                    fatal,
                    Some(final_path.clone()),
                    CleanupState::Complete,
                )
            },
        )?;
    let bundle_anchored = evidence_root_anchor
        .child(&outcome.report.identity)
        .map_err(|fatal| {
            PublicationError::not_applied(
                fatal,
                Some(final_path.clone()),
                CleanupState::Complete,
            )
        })?;
    let bundle_created = match create_plain_directory(&bundle_anchored) {
        Ok(created) => created,
        Err(fatal) => {
            return Err(PublicationError::not_applied(
                fatal,
                Some(final_path),
                CleanupState::Complete,
            ));
        }
    };
    if !bundle_created {
        return Err(PublicationError::not_applied(
            Fatal::new(
                "EVIDENCE_ROOT_CONTENTS",
                concat!(
                    "evidence root changed after its permanent authority ",
                    "marker was acquired",
                ),
            ),
            Some(final_path),
            CleanupState::Complete,
        ));
    }
    let bundle_anchor = match DirectoryAnchor::open(
        bundle,
        &bundle_anchored,
        bundle_created.then_some(bundle_anchored.clone()),
    ) {
        Ok(anchor) => anchor,
        Err(fatal) => {
            return Err(PublicationError::not_applied(
                fatal,
                Some(final_path),
                if bundle_created {
                    CleanupState::Incomplete
                } else {
                    CleanupState::Complete
                },
            ));
        }
    };
    if let Err(fatal) = validate_directory_entries(
        &evidence_root_anchor,
        &[EVIDENCE_AUTHORITY_MARKER, &outcome.report.identity],
        "EVIDENCE_ROOT_CONTENTS",
    ) {
        let cleanup =
            cleanup_owned_directories(None, Some(&bundle_anchor), false);
        return Err(PublicationError::not_applied(
            fatal,
            Some(final_path),
            cleanup,
        ));
    }
    let conformance_anchored =
        bundle_anchor.child("conformance").map_err(|fatal| {
            PublicationError::not_applied(
                fatal,
                Some(final_path.clone()),
                CleanupState::Incomplete,
            )
        })?;
    let conformance_created =
        match create_plain_directory(&conformance_anchored) {
            Ok(created) => created,
            Err(fatal) => {
                let cleanup = cleanup_owned_directories(
                    None,
                    Some(&bundle_anchor),
                    false,
                );
                return Err(PublicationError::not_applied(
                    fatal,
                    Some(final_path),
                    cleanup,
                ));
            }
        };
    let directory_anchor = match DirectoryAnchor::open(
        conformance,
        &conformance_anchored,
        conformance_created.then_some(conformance_anchored.clone()),
    ) {
        Ok(anchor) => anchor,
        Err(fatal) => {
            let cleanup = cleanup_owned_directories(
                None,
                Some(&bundle_anchor),
                conformance_created,
            );
            return Err(PublicationError::not_applied(
                fatal,
                Some(final_path),
                cleanup,
            ));
        }
    };
    let final_anchored =
        directory_anchor.child("source-role-map.json").map_err(|fatal| {
            PublicationError::not_applied(
                fatal,
                Some(final_path.clone()),
                CleanupState::Incomplete,
            )
        })?;
    match fs::symlink_metadata(&final_anchored) {
        Ok(_) => {
            let cleanup = cleanup_owned_directories(
                Some(&directory_anchor),
                Some(&bundle_anchor),
                false,
            );
            return Err(PublicationError::not_applied(
                Fatal::new(
                    "EVIDENCE_EXISTS",
                    format!("{} already exists", final_path.display()),
                ),
                Some(final_path),
                cleanup,
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            let cleanup = cleanup_owned_directories(
                Some(&directory_anchor),
                Some(&bundle_anchor),
                false,
            );
            return Err(PublicationError::not_applied(
                Fatal::new("EVIDENCE_DESTINATION_IO", error.to_string()),
                Some(final_path),
                cleanup,
            ));
        }
    }
    let temporary_name = temporary
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
        PublicationError::not_applied(
            Fatal::new(
                "EVIDENCE_TEMPORARY_NAME",
                "temporary filename is not UTF-8",
            ),
            Some(final_path.clone()),
            CleanupState::Incomplete,
        )
    })?;
    let temporary_anchored =
        directory_anchor.child(temporary_name).map_err(|fatal| {
            PublicationError::not_applied(
                fatal,
                Some(final_path.clone()),
                CleanupState::Incomplete,
            )
        })?;
    let temporary_handle = match create_owned_temporary(
        &temporary_anchored,
        &outcome.report.bytes,
        fault,
    ) {
        Ok(handle) => handle,
        Err(error) => {
            let temporary_clean = error.handle.as_ref().is_none_or(|handle| {
                cleanup_owned_file(&temporary_anchored, handle, 0)
            });
            let directory_cleanup = cleanup_owned_directories(
                Some(&directory_anchor),
                Some(&bundle_anchor),
                false,
            );
            let cleanup = if temporary_clean
                && directory_cleanup == CleanupState::Complete
            {
                CleanupState::Complete
            } else {
                CleanupState::Incomplete
            };
            return Err(PublicationError::not_applied(
                error.fatal,
                Some(final_path),
                cleanup,
            ));
        }
    };
    Ok(PreparedEvidence {
        evidence_root: evidence_root_anchor,
        authority_marker,
        bundle: bundle_anchor,
        directory: directory_anchor,
        temporary_anchored,
        temporary_handle,
        final_path,
        final_anchored,
    })
}

#[derive(Debug)]
struct TemporaryError {
    fatal: Fatal,
    handle: Option<fs::File>,
}

fn create_owned_temporary(
    path: &Path,
    bytes: &[u8],
    fault: PublicationFault,
) -> Result<fs::File, TemporaryError> {
    if fault == PublicationFault::Create {
        return Err(TemporaryError {
            fatal: Fatal::new("EVIDENCE_CREATE", "injected create failure"),
            handle: None,
        });
    }
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| TemporaryError {
            fatal: Fatal::new("EVIDENCE_CREATE", error.to_string()),
            handle: None,
        })?;
    if fault == PublicationFault::Write {
        return Err(TemporaryError {
            fatal: Fatal::new("EVIDENCE_WRITE", "injected write failure"),
            handle: Some(output),
        });
    }
    if let Err(error) = output.write_all(bytes) {
        return Err(TemporaryError {
            fatal: Fatal::new("EVIDENCE_WRITE", error.to_string()),
            handle: Some(output),
        });
    }
    if fault == PublicationFault::FileSync {
        return Err(TemporaryError {
            fatal: Fatal::new("EVIDENCE_WRITE", "injected file sync failure"),
            handle: Some(output),
        });
    }
    if let Err(error) = output.sync_all() {
        return Err(TemporaryError {
            fatal: Fatal::new("EVIDENCE_WRITE", error.to_string()),
            handle: Some(output),
        });
    }
    Ok(output)
}

fn evidence_paths(
    outcome: &Outcome,
    evidence_root: &Path,
) -> Result<(PathBuf, PathBuf, PathBuf, PathBuf, PathBuf), Fatal> {
    if !outcome.clean {
        return Err(Fatal::new(
            "EVIDENCE_FINDINGS",
            "exact evidence cannot retain a failing report",
        ));
    }
    let Some(exact_proven) = &outcome.exact_proven else {
        return Err(Fatal::new(
            "EVIDENCE_NOT_EXACT",
            "only a checker-created exact outcome may be retained",
        ));
    };
    let verified = verify_serialized_report(&outcome.report.bytes, HARD_LIMITS)
        .map_err(|error| Fatal::new("EVIDENCE_REPORT_VERIFY", error.message))?;
    if verified.identity != outcome.report.identity
        || verified.mode != "exact"
        || verified.status != "pass"
        || outcome.report.value.get("mode").and_then(serde_json::Value::as_str)
            != Some("exact")
        || !outcome
            .report
            .value
            .get("revision")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|revision| {
                revision == exact_proven.revision && valid_revision(revision)
            })
        || outcome
            .report
            .value
            .pointer("/result/status")
            .and_then(serde_json::Value::as_str)
            != Some("pass")
    {
        return Err(Fatal::new(
            "EVIDENCE_NOT_EXACT",
            "only an exact, revision-bound, passing report may be retained",
        ));
    }
    require_absolute("evidence-root", evidence_root)?;
    let canonical_evidence_root = evidence_root
        .canonicalize()
        .map_err(|error| Fatal::new("EVIDENCE_ROOT_IO", error.to_string()))?;
    if canonical_evidence_root != evidence_root
        || !canonical_evidence_root.is_dir()
        || canonical_evidence_root.starts_with(&outcome.root)
    {
        return Err(Fatal::new(
            "EVIDENCE_ROOT_SCOPE",
            concat!(
                "evidence root must be its canonical existing directory ",
                "path outside the checked root",
            ),
        ));
    }
    reject_symlink_components(&canonical_evidence_root)?;
    let bundle = canonical_evidence_root.join(&outcome.report.identity);
    let conformance = bundle.join("conformance");
    let final_path = conformance.join("source-role-map.json");
    let temporary = conformance
        .join(format!(".source-role-map.{}.tmp", std::process::id()));
    Ok((canonical_evidence_root, bundle, conformance, final_path, temporary))
}

fn cleanup_owned_file(
    path: &Path,
    handle: &fs::File,
    expected_links: u64,
) -> bool {
    if fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.file_type().is_symlink()
            && metadata.file_type().is_file()
            && handle
                .metadata()
                .is_ok_and(|held| same_file_identity(&metadata, &held))
    }) {
        let _ = fs::remove_file(path);
    }
    handle
        .metadata()
        .is_ok_and(|metadata| file_link_count(&metadata) == expected_links)
}

fn cleanup_owned_directories(
    directory: Option<&DirectoryAnchor>,
    bundle: Option<&DirectoryAnchor>,
    unbound_owned_directory: bool,
) -> CleanupState {
    let directory_clean =
        directory.is_none_or(DirectoryAnchor::cleanup_owned_entry);
    let bundle_clean = bundle.is_none_or(DirectoryAnchor::cleanup_owned_entry);
    if !unbound_owned_directory && directory_clean && bundle_clean {
        CleanupState::Complete
    } else {
        CleanupState::Incomplete
    }
}

fn sync_publication_directories(
    prepared: &PreparedEvidence,
) -> DurabilityState {
    for directory in
        [&prepared.directory, &prepared.bundle, &prepared.evidence_root]
    {
        if directory.handle.sync_all().is_err() {
            return DurabilityState::Unknown;
        }
    }
    DurabilityState::DirectorySynced
}

fn plain_directory_metadata(path: &Path) -> Result<fs::Metadata, Fatal> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        Fatal::new("EVIDENCE_DIRECTORY_IO", error.to_string())
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(Fatal::new(
            "EVIDENCE_DIRECTORY_TYPE",
            format!("{} is not a plain directory", path.display()),
        ));
    }
    Ok(metadata)
}

// Linux sees a trivial success; other targets fail closed through this API.
#[cfg_attr(
    target_os = "linux",
    allow(clippy::missing_const_for_fn, clippy::unnecessary_wraps)
)]
fn require_publication_platform() -> Result<(), Fatal> {
    #[cfg(target_os = "linux")]
    {
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(Fatal::new(
            "EVIDENCE_PLATFORM",
            "held-directory evidence publication requires Linux /proc/self/fd",
        ))
    }
}

fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt as _;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (left, right);
        false
    }
}

fn file_link_count(metadata: &fs::Metadata) -> u64 {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt as _;
        metadata.nlink()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = metadata;
        u64::MAX
    }
}

/// Verifies a bounded report's closed schema, semantic invariants, canonical
/// ordering, and self-identity.
///
/// # Errors
///
/// Returns an error for I/O, bound, schema, semantic, or identity failure.
pub fn verify_report(path: &Path) -> Result<VerifiedReport, Fatal> {
    require_absolute("report", path)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| Fatal::new("VERIFY_REPORT_IO", error.to_string()))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(Fatal::new(
            "VERIFY_REPORT_TYPE",
            "report must be a regular non-symlink file",
        ));
    }
    let bytes = source::read_bounded(path, HARD_LIMITS.report_bytes as u64)
        .map_err(|error| Fatal::new("VERIFY_REPORT_READ", error))?;
    verify_serialized_report(&bytes, HARD_LIMITS)
}

fn verify_serialized_report(
    bytes: &[u8],
    limits: Limits,
) -> Result<VerifiedReport, Fatal> {
    let value =
        json::parse(bytes, limits.json_depth, limits.json_container_items)
            .map_err(|error| {
                Fatal::new("VERIFY_REPORT_JSON", error.to_string())
            })?;
    match value.get("schema").and_then(serde_json::Value::as_str) {
        Some("reovim-source-role-map/v1") => report::verify(bytes, limits)
            .map(|verified| VerifiedReport {
                identity: verified.identity,
                mode: verified.mode,
                status: verified.status,
            })
            .map_err(|error| Fatal::new("VERIFY_REPORT_V1", error.to_string())),
        Some("reovim-source-role-map/v2") => report_v2::verify(
            bytes,
            limits.report_bytes,
            limits.json_depth,
            limits.json_container_items,
        )
        .map(|verified| VerifiedReport {
            identity: verified.identity,
            mode: verified.mode,
            status: verified.status,
        })
        .map_err(|error| v2_report_fatal(&error)),
        Some(other) => Err(Fatal::new(
            "VERIFY_REPORT_SCHEMA",
            format!("unsupported Source Role Map schema {other:?}"),
        )),
        None => Err(Fatal::new(
            "VERIFY_REPORT_SCHEMA",
            "Source Role Map schema identity is absent",
        )),
    }
}

fn v2_report_fatal(error: &report_v2::ReportError) -> Fatal {
    let code = match error.code() {
        report_v2::ReportErrorCode::Envelope => "VERIFY_REPORT_V2_ENVELOPE",
        report_v2::ReportErrorCode::Schema => "VERIFY_REPORT_V2_SCHEMA",
        report_v2::ReportErrorCode::Canonical => "VERIFY_REPORT_V2_CANONICAL",
        report_v2::ReportErrorCode::Identity => "VERIFY_REPORT_V2_IDENTITY",
        report_v2::ReportErrorCode::Limit => "VERIFY_REPORT_V2_LIMIT",
        report_v2::ReportErrorCode::Reference => "VERIFY_REPORT_V2_REFERENCE",
        report_v2::ReportErrorCode::Semantic => "VERIFY_REPORT_V2_SEMANTIC",
    };
    Fatal::new(code, error.to_string())
}

fn committed_input(
    root: &Path,
    supplied: &Path,
    key: &str,
) -> Result<PathBuf, Fatal> {
    require_absolute(key, supplied)?;
    let expected = root.join(key);
    let canonical = supplied.canonicalize().map_err(|error| {
        Fatal::new("INPUT_IO", format!("{}: {error}", supplied.display()))
    })?;
    let canonical_expected = expected.canonicalize().map_err(|error| {
        Fatal::new("INPUT_IO", format!("{}: {error}", expected.display()))
    })?;
    if canonical != canonical_expected || supplied != canonical {
        return Err(Fatal::new(
            "INPUT_IDENTITY",
            format!("{key} must be supplied by its canonical committed path"),
        ));
    }
    Ok(canonical)
}

fn canonical_executable(
    name: &'static str,
    path: &Path,
) -> Result<PathBuf, Fatal> {
    require_absolute(name, path)?;
    let canonical = path.canonicalize().map_err(|error| {
        Fatal::new("EXECUTABLE_IO", format!("{}: {error}", path.display()))
    })?;
    if !canonical.is_file() {
        return Err(Fatal::new(
            "EXECUTABLE_TYPE",
            format!("{name} is not a regular file"),
        ));
    }
    if canonical != path {
        return Err(Fatal::new(
            "EXECUTABLE_PROXY",
            format!(
                concat!(
                    "--{name} must name its canonical real executable ",
                    "(resolve Rust proxies with `rustup which {name}`)",
                ),
                name = name
            ),
        ));
    }
    Ok(canonical)
}

fn validate_cargo_home(path: &Path) -> Result<PathBuf, Fatal> {
    require_absolute("cargo-home", path)?;
    let canonical = path
        .canonicalize()
        .map_err(|error| Fatal::new("CARGO_HOME_IO", error.to_string()))?;
    if canonical != path || !canonical.is_dir() {
        return Err(Fatal::new(
            "CARGO_HOME_CANONICAL",
            "--cargo-home must name a canonical absolute directory",
        ));
    }
    for name in ["config", "config.toml"] {
        match fs::symlink_metadata(canonical.join(name)) {
            Ok(_) => {
                return Err(Fatal::new(
                    "CARGO_HOME_CONFIG",
                    format!("CARGO_HOME/{name} is not admitted"),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(Fatal::new(
                    "CARGO_HOME_CONFIG_IO",
                    error.to_string(),
                ));
            }
        }
    }
    Ok(canonical)
}

fn validate_fixed_cargo_working_directory() -> Result<(), Fatal> {
    for path in [Path::new("/.cargo/config"), Path::new("/.cargo/config.toml")]
    {
        match fs::symlink_metadata(path) {
            Ok(_) => {
                return Err(Fatal::new(
                    "SYSTEM_CARGO_CONFIG",
                    format!(
                        concat!(
                            "{} must be absent for the fixed Cargo working ",
                            "directory",
                        ),
                        path.display()
                    ),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(Fatal::new(
                    "SYSTEM_CARGO_CONFIG_IO",
                    error.to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn apply_limit_overrides(
    limits: &mut Limits,
    overrides: LimitOverrides,
) -> Result<(), Fatal> {
    tighten(
        "stdout-limit",
        &mut limits.metadata_stdout_bytes,
        overrides.stdout_bytes,
    )?;
    tighten(
        "stderr-limit",
        &mut limits.child_stderr_bytes,
        overrides.stderr_bytes,
    )?;
    tighten("report-limit", &mut limits.report_bytes, overrides.report_bytes)?;
    if let Some(value) = overrides.deadline_ms {
        if value == 0 || value > limits.child_deadline_ms {
            return Err(Fatal::new(
                "LIMIT_OVERRIDE",
                concat!(
                    "--deadline-ms must be positive and no greater than ",
                    "the committed limit",
                ),
            ));
        }
        limits.child_deadline_ms = value;
    }
    if let Some(value) = overrides.supervision_deadline_ms {
        if value == 0 || value > limits.child_supervision_deadline_ms {
            return Err(Fatal::new(
                "LIMIT_OVERRIDE",
                concat!(
                    "--supervision-deadline-ms must be positive and no ",
                    "greater than the committed limit",
                ),
            ));
        }
        limits.child_supervision_deadline_ms = value;
    }
    if limits.child_supervision_deadline_ms <= limits.child_deadline_ms {
        return Err(Fatal::new(
            "LIMIT_OVERRIDE",
            "effective supervision deadline must exceed the execution deadline",
        ));
    }
    Ok(())
}

fn tighten(
    name: &str,
    target: &mut usize,
    value: Option<usize>,
) -> Result<(), Fatal> {
    if let Some(value) = value {
        if value == 0 || value > *target {
            return Err(Fatal::new(
                "LIMIT_OVERRIDE",
                format!(
                    concat!(
                        "--{name} must be positive and no greater than the ",
                        "committed limit",
                    ),
                    name = name
                ),
            ));
        }
        *target = value;
    }
    Ok(())
}

fn require_absolute(name: &str, path: &Path) -> Result<(), Fatal> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(Fatal::new(
            "CLI_ABSOLUTE_PATH",
            format!("--{name} must be absolute"),
        ))
    }
}

fn reject_symlink_components(path: &Path) -> Result<(), Fatal> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            Fatal::new("EVIDENCE_COMPONENT_IO", error.to_string())
        })?;
        if metadata.file_type().is_symlink() {
            return Err(Fatal::new(
                "EVIDENCE_SYMLINK",
                format!("{} is a symlink", current.display()),
            ));
        }
    }
    Ok(())
}

fn create_plain_directory(path: &Path) -> Result<bool, Fatal> {
    match fs::create_dir(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path).map_err(|error| {
                Fatal::new("EVIDENCE_DIRECTORY_IO", error.to_string())
            })?;
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                Ok(false)
            } else {
                Err(Fatal::new(
                    "EVIDENCE_DIRECTORY_TYPE",
                    format!("{} is not a plain directory", path.display()),
                ))
            }
        }
        Err(error) => {
            Err(Fatal::new("EVIDENCE_DIRECTORY_CREATE", error.to_string()))
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn config_fatal(error: json::ConfigError) -> Fatal {
    Fatal::new(error.code, format!("{}: {}", error.path, error.message))
}

fn cargo_fatal(error: cargo::CargoError) -> Fatal {
    Fatal::new(error.code, error.message)
}

fn append_git_phase(
    base: &mut git::Evidence,
    phase: git::Evidence,
    label: &str,
) {
    base.processes.extend(phase.processes.into_iter().map(
        |(purpose, process)| {
            let purpose = match (label, purpose) {
                ("pre_metadata", "git_version") => "git_pre_metadata_version",
                ("pre_metadata", "git_root") => "git_pre_metadata_root",
                ("pre_metadata", "git_head") => "git_pre_metadata_head",
                ("pre_metadata", "git_object_format") => {
                    "git_pre_metadata_object_format"
                }
                ("pre_metadata", "git_ls_tree") => "git_pre_metadata_ls_tree",
                ("pre_metadata", "git_index_entries") => {
                    "git_pre_metadata_index_entries"
                }
                ("pre_metadata", "git_info_attributes_path") => {
                    "git_pre_metadata_info_attributes_path"
                }
                ("final", "git_version") => "git_final_version",
                ("final", "git_root") => "git_final_root",
                ("final", "git_head") => "git_final_head",
                ("final", "git_object_format") => "git_final_object_format",
                ("final", "git_ls_tree") => "git_final_ls_tree",
                ("final", "git_index_entries") => "git_final_index_entries",
                ("final", "git_info_attributes_path") => {
                    "git_final_info_attributes_path"
                }
                _ => "git_final_unknown",
            };
            (purpose, process)
        },
    ));
}

fn validate_git_inventory(
    evidence: &git::Evidence,
    inventory: &source::Inventory,
    policy: &policy::Policy,
    root: &Path,
    limits: Limits,
) -> Result<(), Fatal> {
    for path in &evidence.tracked_paths {
        if policy
            .exclusions
            .iter()
            .any(|exclusion| path.starts_with(&exclusion.prefix))
        {
            return Err(Fatal::new(
                "TRACKED_EXCLUSION",
                format!(
                    "tracked path {} is hidden by a closed exclusion",
                    path.display()
                ),
            ));
        }
    }
    let inventoried = inventory
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<std::collections::BTreeSet<_>>();
    if inventoried != evidence.tracked_paths {
        let missing = evidence.tracked_paths.difference(&inventoried).next();
        let extra = inventoried.difference(&evidence.tracked_paths).next();
        return Err(Fatal::new(
            "REVISION_INVENTORY_MISMATCH",
            format!(
                concat!(
                    "tracked/inventoried path sets differ; ",
                    "missing={missing:?}, extra={extra:?}",
                ),
                missing = missing,
                extra = extra
            ),
        ));
    }
    for file in &inventory.files {
        let entry = evidence.tree_entries.get(&file.path).ok_or_else(|| {
            Fatal::new(
                "GIT_TREE_ENTRY_MISSING",
                format!(
                    "{} is absent from full HEAD tree",
                    file.path.display()
                ),
            )
        })?;
        let bytes = source::read_bounded(
            &root.join(&file.path),
            limits.source_file_bytes,
        )
        .map_err(|error| Fatal::new("GIT_BLOB_READ", error))?;
        if digest::bytes(&bytes) != file.content_digest {
            return Err(Fatal::new(
                "GIT_BLOB_INVENTORY_DRIFT",
                format!("{} changed after inventory", file.path.display()),
            ));
        }
        let object_id = digest::git_blob(&bytes, &evidence.object_format)
            .map_err(|error| Fatal::new("GIT_BLOB_HASH", error.to_string()))?;
        if object_id != entry.object_id {
            return Err(Fatal::new(
                "GIT_BLOB_MISMATCH",
                format!("{} bytes do not match HEAD blob", file.path.display()),
            ));
        }
        let mode = git_file_mode(&root.join(&file.path))?;
        if mode != entry.mode {
            return Err(Fatal::new(
                "GIT_MODE_MISMATCH",
                format!(
                    "{} mode {mode} differs from HEAD {}",
                    file.path.display(),
                    entry.mode
                ),
            ));
        }
    }
    Ok(())
}

fn git_file_mode(path: &Path) -> Result<String, Fatal> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let metadata = fs::metadata(path)
            .map_err(|error| Fatal::new("GIT_MODE_IO", error.to_string()))?;
        Ok(if metadata.permissions().mode() & 0o111 == 0 {
            "100644".to_owned()
        } else {
            "100755".to_owned()
        })
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(Fatal::new(
            "GIT_MODE_PLATFORM",
            "v1 Git mode proof requires the declared Unix target",
        ))
    }
}

fn require_config_unchanged(
    path: &Path,
    initial: &[u8],
    limits: Limits,
) -> Result<(), Fatal> {
    let (final_bytes, _) = json::read(
        path,
        limits.config_bytes,
        limits.json_depth,
        limits.json_container_items,
    )
    .map_err(config_fatal)?;
    if digest::bytes(initial) != digest::bytes(&final_bytes) {
        return Err(Fatal::new(
            "CONFIG_INPUT_DRIFT",
            format!("{} changed during the conformance run", path.display()),
        ));
    }
    Ok(())
}

fn open_executing_image() -> Result<fs::File, Fatal> {
    #[cfg(target_os = "linux")]
    let path = Path::new("/proc/self/exe");
    #[cfg(not(target_os = "linux"))]
    let path = std::env::current_exe()
        .map_err(|error| Fatal::new("CHECKER_IDENTITY", error.to_string()))?;
    #[cfg(not(target_os = "linux"))]
    let path = path.as_path();
    fs::File::open(path)
        .map_err(|error| Fatal::new("CHECKER_IDENTITY", error.to_string()))
}

fn valid_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{
        CleanupState, DirectoryAnchor, DurabilityState,
        EVIDENCE_AUTHORITY_MARKER, EffectCertainty, Outcome, PreparedEvidence,
        PublicationFault, claim_evidence_root, create_authority_marker,
        create_owned_temporary, create_plain_directory,
        current_publication_uid, fs, publish_prepared, reject_existing_final,
        report_v2, v2_report_fatal, validate_directory_entries,
        validate_evidence_root_authority, validate_initial_evidence_root,
        write_evidence,
    };

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn v2_verifier_failure_categories_remain_distinct() {
        use report_v2::ReportErrorCode;

        let cases = [
            (ReportErrorCode::Envelope, "VERIFY_REPORT_V2_ENVELOPE"),
            (ReportErrorCode::Schema, "VERIFY_REPORT_V2_SCHEMA"),
            (ReportErrorCode::Canonical, "VERIFY_REPORT_V2_CANONICAL"),
            (ReportErrorCode::Identity, "VERIFY_REPORT_V2_IDENTITY"),
            (ReportErrorCode::Limit, "VERIFY_REPORT_V2_LIMIT"),
            (ReportErrorCode::Reference, "VERIFY_REPORT_V2_REFERENCE"),
            (ReportErrorCode::Semantic, "VERIFY_REPORT_V2_SEMANTIC"),
        ];
        for (category, code) in cases {
            let error = report_v2::ReportError::new(category, "failure");
            let fatal = v2_report_fatal(&error);
            assert_eq!(fatal.code, code);
            assert_eq!(fatal.message, "failure");
        }
    }

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new(label: &str) -> Self {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "reovim-conformance-publication-{label}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            #[cfg(target_os = "linux")]
            set_owner_only(&path);
            Self(path)
        }

        fn prepared(&self, label: &str) -> PreparedEvidence {
            let bundle = self.0.join(label);
            let directory = bundle.join("conformance");
            let owner_uid = validate_evidence_root_authority(&self.0).unwrap();
            let evidence_root =
                DirectoryAnchor::open(self.0.clone(), &self.0, None).unwrap();
            let authority_marker =
                claim_evidence_root(&evidence_root, owner_uid).unwrap();
            let bundle_anchored = evidence_root.child(label).unwrap();
            assert!(create_plain_directory(&bundle_anchored).unwrap());
            let bundle_anchor = DirectoryAnchor::open(
                bundle,
                &bundle_anchored,
                Some(bundle_anchored.clone()),
            )
            .unwrap();
            let directory_anchored =
                bundle_anchor.child("conformance").unwrap();
            assert!(create_plain_directory(&directory_anchored).unwrap());
            let directory_anchor = DirectoryAnchor::open(
                directory.clone(),
                &directory_anchored,
                Some(directory_anchored.clone()),
            )
            .unwrap();
            let temporary_anchored =
                directory_anchor.child(".report.tmp").unwrap();
            let temporary_handle = create_owned_temporary(
                &temporary_anchored,
                b"evidence",
                PublicationFault::None,
            )
            .unwrap();
            let final_anchored =
                directory_anchor.child("source-role-map.json").unwrap();
            PreparedEvidence {
                evidence_root,
                authority_marker,
                bundle: bundle_anchor,
                directory: directory_anchor,
                temporary_anchored,
                temporary_handle,
                final_path: directory.join("source-role-map.json"),
                final_anchored,
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn set_owner_only(path: &Path) {
        use std::os::unix::fs::PermissionsExt as _;

        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn evidence_root_requires_exact_owner_only_mode_and_safe_parent() {
        use std::os::unix::fs::PermissionsExt as _;

        let wrong_mode = TempRoot::new("wrong-root-mode");
        fs::set_permissions(&wrong_mode.0, fs::Permissions::from_mode(0o755))
            .unwrap();
        let error =
            validate_evidence_root_authority(&wrong_mode.0).unwrap_err();
        assert_eq!(error.code, "EVIDENCE_ROOT_AUTHORITY");

        let unsafe_parent = TempRoot::new("unsafe-parent");
        fs::set_permissions(
            &unsafe_parent.0,
            fs::Permissions::from_mode(0o777),
        )
        .unwrap();
        let unsafe_child = unsafe_parent.0.join("evidence");
        fs::create_dir(&unsafe_child).unwrap();
        fs::set_permissions(&unsafe_child, fs::Permissions::from_mode(0o700))
            .unwrap();
        let error =
            validate_evidence_root_authority(&unsafe_child).unwrap_err();
        assert_eq!(error.code, "EVIDENCE_ROOT_PARENT");

        let sticky_parent = TempRoot::new("sticky-parent");
        fs::set_permissions(
            &sticky_parent.0,
            fs::Permissions::from_mode(0o1777),
        )
        .unwrap();
        let sticky_child = sticky_parent.0.join("evidence");
        fs::create_dir(&sticky_child).unwrap();
        fs::set_permissions(&sticky_child, fs::Permissions::from_mode(0o700))
            .unwrap();
        assert_eq!(
            validate_evidence_root_authority(&sticky_child).unwrap(),
            current_publication_uid().unwrap()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn permanent_marker_consumes_root_and_is_held_with_exact_identity() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = TempRoot::new("permanent-marker");
        let owner_uid = validate_evidence_root_authority(&root.0).unwrap();
        let anchor =
            DirectoryAnchor::open(root.0.clone(), &root.0, None).unwrap();
        let marker = claim_evidence_root(&anchor, owner_uid).unwrap();
        marker.validate().unwrap();
        validate_directory_entries(
            &anchor,
            &[EVIDENCE_AUTHORITY_MARKER],
            "EVIDENCE_ROOT_CONTENTS",
        )
        .unwrap();
        let metadata =
            fs::symlink_metadata(root.0.join(EVIDENCE_AUTHORITY_MARKER))
                .unwrap();
        assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);
        drop(marker);

        let error = claim_evidence_root(&anchor, owner_uid).unwrap_err();
        assert_eq!(error.code, "EVIDENCE_ROOT_CONSUMED");
        assert!(root.0.join(EVIDENCE_AUTHORITY_MARKER).is_file());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn competing_stale_claim_loses_to_create_new_marker() {
        let root = TempRoot::new("competing-marker");
        let owner_uid = validate_evidence_root_authority(&root.0).unwrap();
        let first_anchor =
            DirectoryAnchor::open(root.0.clone(), &root.0, None).unwrap();
        let second_anchor =
            DirectoryAnchor::open(root.0.clone(), &root.0, None).unwrap();
        validate_initial_evidence_root(&first_anchor).unwrap();
        validate_initial_evidence_root(&second_anchor).unwrap();

        let marker = create_authority_marker(&first_anchor, owner_uid).unwrap();
        let error =
            create_authority_marker(&second_anchor, owner_uid).unwrap_err();
        assert_eq!(error.code, "EVIDENCE_ROOT_CONSUMED");
        marker.validate().unwrap();
        validate_directory_entries(
            &first_anchor,
            &[EVIDENCE_AUTHORITY_MARKER],
            "EVIDENCE_ROOT_CONTENTS",
        )
        .unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn failed_publication_attempt_permanently_consumes_root() {
        let root = TempRoot::new("failed-attempt-consumes");
        let prepared = root.prepared("bundle");
        let error =
            publish_prepared(prepared, PublicationFault::Link).unwrap_err();
        assert_eq!(error.effect_certainty, EffectCertainty::NotApplied);
        assert_eq!(error.cleanup_state, CleanupState::Complete);
        assert!(root.0.join(EVIDENCE_AUTHORITY_MARKER).is_file());

        let owner_uid = validate_evidence_root_authority(&root.0).unwrap();
        let anchor =
            DirectoryAnchor::open(root.0.clone(), &root.0, None).unwrap();
        let error = claim_evidence_root(&anchor, owner_uid).unwrap_err();
        assert_eq!(error.code, "EVIDENCE_ROOT_CONSUMED");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn authority_marker_replacement_is_detected_without_deleting_replacement() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = TempRoot::new("marker-replacement");
        let displaced_root = TempRoot::new("marker-replacement-displaced");
        let prepared = root.prepared("bundle");
        let marker_path = root.0.join(EVIDENCE_AUTHORITY_MARKER);
        let displaced_marker = displaced_root.0.join("original-marker");
        fs::rename(&marker_path, &displaced_marker).unwrap();
        fs::write(&marker_path, super::EVIDENCE_AUTHORITY_MARKER_BYTES)
            .unwrap();
        fs::set_permissions(&marker_path, fs::Permissions::from_mode(0o600))
            .unwrap();

        let error =
            publish_prepared(prepared, PublicationFault::None).unwrap_err();
        assert_eq!(error.effect_certainty, EffectCertainty::NotApplied);
        assert_eq!(error.fatal.code, "EVIDENCE_AUTHORITY_MARKER");
        assert_eq!(
            fs::read(&marker_path).unwrap(),
            super::EVIDENCE_AUTHORITY_MARKER_BYTES
        );
        assert_eq!(
            fs::read(&displaced_marker).unwrap(),
            super::EVIDENCE_AUTHORITY_MARKER_BYTES
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn existing_final_wins_over_consumed_root_error() {
        let root = TempRoot::new("existing-final");
        let owner_uid = validate_evidence_root_authority(&root.0).unwrap();
        let anchor =
            DirectoryAnchor::open(root.0.clone(), &root.0, None).unwrap();
        let _marker = claim_evidence_root(&anchor, owner_uid).unwrap();
        let identity = "sha256:existing";
        let bundle = root.0.join(identity);
        let conformance = bundle.join("conformance");
        fs::create_dir(&bundle).unwrap();
        fs::create_dir(&conformance).unwrap();
        let final_path = conformance.join("source-role-map.json");
        fs::write(&final_path, b"existing").unwrap();

        let error =
            reject_existing_final(&anchor, identity, &final_path).unwrap_err();
        assert_eq!(error.fatal.code, "EVIDENCE_EXISTS");
        assert_eq!(error.effect_certainty, EffectCertainty::NotApplied);
        assert_eq!(error.intended_path.as_deref(), Some(final_path.as_path()));
    }

    #[test]
    fn candidate_outcome_cannot_be_written_as_exact_evidence() {
        let outcome = Outcome {
            report: report_v2::BuiltReport {
                value: serde_json::json!({
                    "mode": "candidate",
                    "revision": null,
                    "result": {"status": "pass"}
                }),
                bytes: Vec::new(),
                identity: concat!(
                    "sha256:",
                    "00000000000000000000000000000000",
                    "00000000000000000000000000000000",
                )
                .to_owned(),
            },
            clean: true,
            root: PathBuf::from("/checked-root"),
            exact_proven: None,
        };
        let error =
            write_evidence(&outcome, Path::new("/unused-evidence-root"))
                .unwrap_err();
        assert_eq!(error.fatal.code, "EVIDENCE_NOT_EXACT");
        assert_eq!(error.effect_certainty, EffectCertainty::NotApplied);
    }

    #[test]
    fn temporary_file_faults_preserve_exact_ownership() {
        let root = TempRoot::new("temporary");
        let path = root.0.join("report.tmp");
        fs::write(&path, b"preexisting").unwrap();
        let error =
            create_owned_temporary(&path, b"new", PublicationFault::None)
                .unwrap_err();
        assert!(error.handle.is_none());
        assert_eq!(fs::read(&path).unwrap(), b"preexisting");
        fs::remove_file(&path).unwrap();

        let error =
            create_owned_temporary(&path, b"new", PublicationFault::Create)
                .unwrap_err();
        assert!(error.handle.is_none());
        assert!(!path.exists());
        for fault in [PublicationFault::Write, PublicationFault::FileSync] {
            let error =
                create_owned_temporary(&path, b"new", fault).unwrap_err();
            assert!(error.handle.is_some());
            assert!(path.exists());
            fs::remove_file(&path).unwrap();
        }
        let _ =
            create_owned_temporary(&path, b"retained", PublicationFault::None)
                .unwrap();
        assert_eq!(fs::read(path).unwrap(), b"retained");
    }

    #[test]
    fn publication_faults_expose_pre_and_post_commit_effect_certainty() {
        let link_root = TempRoot::new("effects-link");
        let prepared = link_root.prepared("link");
        let final_path = prepared.final_path.clone();
        let error =
            publish_prepared(prepared, PublicationFault::Link).unwrap_err();
        assert_eq!(error.effect_certainty, EffectCertainty::NotApplied);
        assert_eq!(error.cleanup_state, CleanupState::Complete);
        assert!(!final_path.exists());

        let unlink_root = TempRoot::new("effects-unlink");
        let prepared = unlink_root.prepared("unlink");
        let final_path = prepared.final_path.clone();
        let temporary = prepared.directory.nominal_path.join(".report.tmp");
        let error =
            publish_prepared(prepared, PublicationFault::Unlink).unwrap_err();
        assert_eq!(error.effect_certainty, EffectCertainty::Applied);
        assert_eq!(error.cleanup_state, CleanupState::Incomplete);
        assert_eq!(error.intended_path.as_deref(), Some(final_path.as_path()));
        assert!(final_path.exists() && temporary.exists());

        let sync_root = TempRoot::new("effects-sync");
        let prepared = sync_root.prepared("sync");
        let final_path = prepared.final_path.clone();
        let error = publish_prepared(prepared, PublicationFault::DirectorySync)
            .unwrap_err();
        assert_eq!(error.effect_certainty, EffectCertainty::Applied);
        assert_eq!(error.cleanup_state, CleanupState::Complete);
        assert_eq!(error.durability_state, DurabilityState::Unknown);
        assert!(final_path.exists());

        let success_root = TempRoot::new("effects-success");
        let publication = publish_prepared(
            success_root.prepared("success"),
            PublicationFault::None,
        )
        .expect("fully applied durable publication");
        assert_eq!(publication.effect_certainty(), EffectCertainty::Applied);
        assert_eq!(publication.cleanup_state(), CleanupState::Complete);
        assert_eq!(
            publication.durability_state(),
            DurabilityState::DirectorySynced
        );
        assert!(publication.path().exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn directory_substitution_cannot_redirect_publication() {
        use std::os::unix::fs::symlink;

        let root = TempRoot::new("directory-substitution");
        let attacker_root = TempRoot::new("directory-substitution-attacker");
        let prepared = root.prepared("bundle");
        let nominal_directory = prepared.directory.nominal_path.clone();
        let displaced_directory =
            prepared.bundle.nominal_path.join("displaced-conformance");
        let attacker_directory = attacker_root.0.join("attacker");
        fs::create_dir(&attacker_directory).unwrap();
        fs::rename(&nominal_directory, &displaced_directory).unwrap();
        symlink(&attacker_directory, &nominal_directory).unwrap();

        let error =
            publish_prepared(prepared, PublicationFault::None).unwrap_err();
        assert_eq!(error.effect_certainty, EffectCertainty::NotApplied);
        assert_eq!(error.cleanup_state, CleanupState::Incomplete);
        assert!(matches!(
            error.fatal.code,
            "EVIDENCE_DIRECTORY_TYPE" | "EVIDENCE_DIRECTORY_SUBSTITUTED"
        ));
        assert!(!attacker_directory.join("source-role-map.json").exists());
        assert!(!displaced_directory.join("source-role-map.json").exists());
        assert!(!displaced_directory.join(".report.tmp").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn displaced_temporary_and_replacement_are_not_misreported_or_deleted() {
        let root = TempRoot::new("temporary-replacement");
        let displaced_root = TempRoot::new("temporary-displaced");
        let prepared = root.prepared("bundle");
        let nominal_temporary =
            prepared.directory.nominal_path.join(".report.tmp");
        let displaced_temporary = displaced_root.0.join("displaced-report");
        fs::rename(&nominal_temporary, &displaced_temporary).unwrap();
        fs::write(&nominal_temporary, b"replacement").unwrap();

        let error =
            publish_prepared(prepared, PublicationFault::None).unwrap_err();
        assert_eq!(error.effect_certainty, EffectCertainty::NotApplied);
        assert_eq!(error.cleanup_state, CleanupState::Incomplete);
        assert_eq!(error.fatal.code, "EVIDENCE_TEMPORARY_SUBSTITUTED");
        assert_eq!(fs::read(&nominal_temporary).unwrap(), b"replacement");
        assert_eq!(fs::read(&displaced_temporary).unwrap(), b"evidence");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn displaced_directory_and_replacement_remain_cleanup_incomplete() {
        let root = TempRoot::new("directory-replacement");
        let displaced_root = TempRoot::new("directory-displaced");
        let prepared = root.prepared("bundle");
        let nominal_directory = prepared.directory.nominal_path.clone();
        let displaced_directory =
            displaced_root.0.join("displaced-conformance");
        fs::rename(&nominal_directory, &displaced_directory).unwrap();
        fs::create_dir(&nominal_directory).unwrap();

        let error =
            publish_prepared(prepared, PublicationFault::None).unwrap_err();
        assert_eq!(error.effect_certainty, EffectCertainty::NotApplied);
        assert_eq!(error.cleanup_state, CleanupState::Incomplete);
        assert_eq!(error.fatal.code, "EVIDENCE_DIRECTORY_SUBSTITUTED");
        assert!(nominal_directory.is_dir());
        assert!(displaced_directory.is_dir());
        assert!(!displaced_directory.join(".report.tmp").exists());
    }

    #[test]
    fn existing_owned_final_link_is_reconciled_as_applied() {
        let root = TempRoot::new("reconciled-link");
        let prepared = root.prepared("bundle");
        let final_path = prepared.final_path.clone();
        let nominal_temporary =
            prepared.directory.nominal_path.join(".report.tmp");
        fs::hard_link(&prepared.temporary_anchored, &prepared.final_anchored)
            .unwrap();

        let error =
            publish_prepared(prepared, PublicationFault::None).unwrap_err();
        assert_eq!(error.effect_certainty, EffectCertainty::Applied);
        assert_eq!(error.cleanup_state, CleanupState::Complete);
        assert_eq!(error.durability_state, DurabilityState::DirectorySynced);
        assert_eq!(error.fatal.code, "EVIDENCE_PUBLISH_RECONCILED");
        assert_eq!(error.intended_path.as_deref(), Some(final_path.as_path()));
        assert_eq!(fs::read(final_path).unwrap(), b"evidence");
        assert!(!nominal_temporary.exists());
    }

    #[test]
    fn failed_final_link_reconciliation_preserves_outcome_unknown() {
        let root = TempRoot::new("reconcile-unknown");
        let prepared = root.prepared("bundle");
        let final_path = prepared.final_path.clone();
        let nominal_temporary =
            prepared.directory.nominal_path.join(".report.tmp");
        fs::hard_link(&prepared.temporary_anchored, &prepared.final_anchored)
            .unwrap();

        let error = publish_prepared(prepared, PublicationFault::Reconcile)
            .unwrap_err();
        assert_eq!(error.effect_certainty, EffectCertainty::OutcomeUnknown);
        assert_eq!(error.effect_certainty.as_str(), "outcome_unknown");
        assert_eq!(error.cleanup_state, CleanupState::Incomplete);
        assert_eq!(error.durability_state, DurabilityState::Unknown);
        assert_eq!(error.fatal.code, "EVIDENCE_LINK_RECONCILE");
        assert_eq!(error.intended_path.as_deref(), Some(final_path.as_path()));
        assert_eq!(fs::read(final_path).unwrap(), b"evidence");
        assert!(!nominal_temporary.exists());
    }
}
