//! Closed Cargo and rustc capture for the initial Product proof profile.
//!
//! Cargo selects one exact library build. The same checker binary acts as its
//! private `RUSTC_WRAPPER`, records closed discovery probes separately, and
//! retains one compiler unit with private dep-info evidence.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fmt, fs,
    fs::OpenOptions,
    io::Write as _,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};

use serde_json::{Map, Value, json};

use crate::{
    digest, json as bounded_json,
    process::{
        self, CommandSpec, ProcessLimits, ProcessResult, TerminationReason,
    },
    product_closures::{Closure, ClosureKind, TARGET_TRIPLE},
};

const TRACE_SCHEMA: &str = "reovim-rustc-wrapper-trace/v1";
const WRAPPER_MODE: &str = "REOVIM_CAPTURE_WRAPPER";
const CONTROL_PREFIX: &str = "REOVIM_CAPTURE_";
const EXPECTED_FEATURE: &str = "default";
const COMPILER_LIBRARY_PATH: &str = "/compiler/lib";
pub const RUSTC_LAUNCHER_MODE: &str = "REOVIM_RUSTC_LAUNCHER";
pub const RUSTC_LAUNCHER_RUSTC: &str = "REOVIM_RUSTC_LAUNCHER_RUSTC";
pub const RUSTC_LAUNCHER_LIBRARY: &str = "REOVIM_RUSTC_LAUNCHER_LIBRARY_PATH";

const ENV_RUSTC: &str = "REOVIM_CAPTURE_RUSTC";
const ENV_SYSROOT: &str = "REOVIM_CAPTURE_SYSROOT";
const ENV_CARGO_HOME: &str = "REOVIM_CAPTURE_CARGO_HOME";
const ENV_TEMP: &str = "REOVIM_CAPTURE_TEMP_DIR";
const ENV_BUILD_DIR: &str = "REOVIM_CAPTURE_BUILD_DIR";
const ENV_REPOSITORY: &str = "REOVIM_CAPTURE_REPOSITORY";
const ENV_TARGET: &str = "REOVIM_CAPTURE_TARGET_DIR";
const ENV_TRACE: &str = "REOVIM_CAPTURE_TRACE_DIR";
const ENV_DEP_INFO: &str = "REOVIM_CAPTURE_DEP_INFO_DIR";
const ENV_PATH_BYTES: &str = "REOVIM_CAPTURE_PATH_BYTES";
const ENV_TRACE_BYTES: &str = "REOVIM_CAPTURE_TRACE_BYTES";
const ENV_ARGV_ITEMS: &str = "REOVIM_CAPTURE_ARGV_ITEMS";
const ENV_ARGV_BYTES: &str = "REOVIM_CAPTURE_ARGV_BYTES";
const ENV_TRACE_COUNT: &str = "REOVIM_CAPTURE_TRACE_COUNT";
const ENV_DEP_INFO_BYTES: &str = "REOVIM_CAPTURE_DEP_INFO_BYTES";

/// Hard limits for one Cargo build and its wrapper evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Bounded Cargo child-process limits.
    pub process: ProcessLimits,
    /// Maximum UTF-8 bytes in one retained path.
    pub path_bytes: usize,
    /// Maximum retained Cargo JSON bytes.
    pub cargo_json_bytes: usize,
    /// Maximum Cargo JSON messages.
    pub cargo_message_count: usize,
    /// Maximum aggregate wrapper trace bytes.
    pub wrapper_trace_bytes: usize,
    /// Maximum arguments in one rustc wrapper invocation.
    pub wrapper_argv_items: usize,
    /// Maximum aggregate argument/environment bytes per invocation.
    pub wrapper_argv_bytes: usize,
    /// Maximum discovery probes plus compiler units.
    pub wrapper_trace_count: usize,
    /// Maximum bytes in one dep-info file.
    pub dep_info_file_bytes: usize,
    /// Maximum aggregate dep-info bytes.
    pub dep_info_total_bytes: usize,
    /// Maximum compiler source inputs in one unit.
    pub compiler_input_count: usize,
    /// Maximum retained output artifacts.
    pub artifact_count: usize,
    /// Maximum aggregate retained artifact bytes.
    pub artifact_total_bytes: u64,
    /// JSON nesting bound for Cargo and trace records.
    pub json_depth: usize,
    /// JSON container-item bound for Cargo and trace records.
    pub json_items: usize,
}

/// Exact prepared-root selection for one Cargo build.
pub struct BuildRequest<'a> {
    /// Registered shipping or validation closure.
    pub closure: &'a Closure,
    /// Exact package ID from locked/offline Cargo metadata.
    pub package_id: &'a str,
    /// Locked/offline Cargo metadata preflight for the Product workspace.
    pub metadata: &'a crate::cargo::Metadata,
    /// Canonical repository exposure root.
    pub repository_root: &'a Path,
    /// Canonical Cargo executable.
    pub cargo: &'a Path,
    /// Canonical rustc executable.
    pub rustc: &'a Path,
    /// Canonical same-binary checker executable.
    pub checker: &'a Path,
    /// Prepared target sysroot mounted at the exact `/sysroot` path.
    pub sysroot: &'a Path,
    /// Canonical writable scratch authority.
    pub scratch_root: &'a Path,
    /// Fresh, credential-free Cargo home below scratch.
    pub cargo_home: &'a Path,
    /// Temporary directory below scratch.
    pub temp_dir: &'a Path,
    /// Cargo target authority shared by the related builds.
    pub target_dir: &'a Path,
    /// Empty per-build wrapper trace directory below scratch.
    pub trace_dir: &'a Path,
    /// Empty per-build private dep-info directory below scratch.
    pub dep_info_dir: &'a Path,
    /// Exact compiler source/module paths admitted for this closure.
    pub admitted_sources: &'a BTreeSet<PathBuf>,
    /// Prior shipping provider required by a validation build.
    pub provider: Option<&'a ProviderArtifacts>,
}

/// Exact regular-file identity retained for an output artifact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileIdentity {
    /// Absolute prepared-root path.
    pub path: PathBuf,
    /// SHA-256 content identity.
    pub content_digest: String,
    /// Observed logical size.
    pub size_bytes: u64,
}

/// Cargo's semantic `compiler-artifact` observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoArtifact {
    /// Exact Cargo package ID.
    pub package_id: String,
    /// Exact package manifest path.
    pub manifest_path: PathBuf,
    /// Exact target name.
    pub target_name: String,
    /// Cargo target kinds.
    pub target_kinds: Vec<String>,
    /// Cargo target crate types.
    pub crate_types: Vec<String>,
    /// Exact target source root.
    pub source_path: PathBuf,
    /// Enabled Cargo features.
    pub features: Vec<String>,
    /// Cargo-reported filenames.
    pub filenames: Vec<PathBuf>,
    /// True only for a reused provider event.
    pub fresh: bool,
}

/// Parsed Cargo JSON stream for one exact build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoObservation {
    /// Exact Cargo executable plus build arguments.
    pub command_argv: Vec<String>,
    /// Digest of the complete newline-delimited stream.
    pub raw_digest: String,
    /// Retained stream byte count.
    pub stdout_bytes: u64,
    /// Parsed message count including `build-finished`.
    pub message_count: usize,
    /// Compiler artifact messages in stream order.
    pub artifacts: Vec<CargoArtifact>,
}

/// Closed wrapper invocation classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InvocationKind {
    /// `rustc -vV` compiler identity discovery.
    VersionProbe,
    /// Host capability and filename discovery.
    HostProbe,
    /// Selected-target capability and filename discovery.
    TargetProbe,
    /// Actual Product library compiler unit.
    CompilerUnit,
}

impl InvocationKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::VersionProbe => "version_probe",
            Self::HostProbe => "host_probe",
            Self::TargetProbe => "target_probe",
            Self::CompilerUnit => "compiler_unit",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "version_probe" => Some(Self::VersionProbe),
            "host_probe" => Some(Self::HostProbe),
            "target_probe" => Some(Self::TargetProbe),
            "compiler_unit" => Some(Self::CompilerUnit),
            _ => None,
        }
    }
}

/// One environment entry admitted and forwarded by the wrapper.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EnvironmentEntry {
    /// Exact UTF-8 environment name.
    pub name: String,
    /// Exact UTF-8 environment value.
    pub value: String,
}

/// One paired same-binary wrapper trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperTrace {
    /// Global per-build ordinal.
    pub ordinal: u64,
    /// Closed probe or compiler-unit classification.
    pub kind: InvocationKind,
    /// Canonical rustc path supplied by Cargo.
    pub rustc: PathBuf,
    /// Original rustc argv, including rustc at index zero.
    pub original_argv: Vec<String>,
    /// Effective rustc argv after private dep-info selection.
    pub effective_argv: Vec<String>,
    /// Exact child environment forwarded to rustc.
    pub environment: Vec<EnvironmentEntry>,
    /// Private dep-info path for a compiler unit.
    pub dep_info_path: Option<PathBuf>,
    /// rustc exit status code.
    pub status: i32,
    /// Digest of the paired start and finish trace bytes.
    pub trace_digest: String,
    /// Aggregate paired trace bytes.
    pub trace_bytes: u64,
}

/// Conservative supplied Product extern edge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExternEdge {
    /// Exact rustc extern name.
    pub name: String,
    /// Exact supplied provider path.
    pub path: PathBuf,
}

/// One correlated Product compiler unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerUnit {
    /// Stable unit ID derived from the closure.
    pub id: String,
    /// Closure ID.
    pub closure: String,
    /// Exact crate name.
    pub crate_name: String,
    /// Exact crate types.
    pub crate_types: Vec<String>,
    /// Exact target triple.
    pub target_triple: String,
    /// Cargo profile name.
    pub profile: String,
    /// Enabled feature names from rustc cfgs.
    pub features: Vec<String>,
    /// Exact rustc cfg arguments.
    pub cfgs: Vec<String>,
    /// Original wrapper argv.
    pub original_argv: Vec<String>,
    /// Effective wrapper argv.
    pub effective_argv: Vec<String>,
    /// Private dep-info path.
    pub dep_info_path: PathBuf,
    /// Private dep-info digest.
    pub dep_info_digest: String,
    /// Private dep-info bytes.
    pub dep_info_bytes: u64,
    /// Exact repository-relative compiler sources.
    pub sources: Vec<PathBuf>,
    /// Conservatively participating extern edges.
    pub externs: Vec<ExternEdge>,
    /// Hashed rustc output identities.
    pub outputs: Vec<FileIdentity>,
    /// Paired trace digest.
    pub trace_digest: String,
    /// Paired trace bytes.
    pub trace_bytes: u64,
    /// rustc status code.
    pub status: i32,
}

/// Exact reusable Foundation outputs retained after shipping capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderArtifacts {
    /// Shipping closure ID.
    pub closure: String,
    /// Exact shipping Cargo package ID.
    pub package_id: String,
    /// Shipping compiler unit ID.
    pub unit: String,
    /// Exact Cargo semantic artifact observation.
    pub cargo_artifact: CargoArtifact,
    /// Top-level Cargo shipping rlib.
    pub top_level_rlib: FileIdentity,
    /// Hashed rustc rlib beneath the dependency directory.
    pub hashed_rlib: FileIdentity,
    /// Hashed rustc metadata used by the external consumer.
    pub rmeta: FileIdentity,
    /// Shared target authority.
    pub target_dir: PathBuf,
    /// Shared credential-free Cargo home retaining discovery state.
    pub cargo_home: PathBuf,
}

/// Validated reuse relation from validation back to shipping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderReuse {
    /// Fresh Cargo provider event.
    pub fresh_artifact: CargoArtifact,
    /// Fresh-event hashed rlib identity.
    pub hashed_rlib: FileIdentity,
    /// Fresh-event metadata identity selected by `--extern`.
    pub rmeta: FileIdentity,
    /// Exact consumer extern edge.
    pub extern_edge: ExternEdge,
}

/// Fully correlated semantic result of one Cargo build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildObservation {
    /// Closure ID.
    pub closure: String,
    /// Exact Cargo JSON observation.
    pub cargo: CargoObservation,
    /// Closed discovery probes, ordered by wrapper ordinal.
    pub probes: Vec<WrapperTrace>,
    /// Exactly one nonfresh Product compiler unit.
    pub unit: CompilerUnit,
    /// Shipping provider outputs, when this is the shipping build.
    pub provider: Option<ProviderArtifacts>,
    /// Validated provider reuse, when this is the validation build.
    pub provider_reuse: Option<ProviderReuse>,
}

/// Semantic observation accepted after full Cargo process supervision.
#[derive(Debug)]
pub struct Capture {
    /// Correlated build evidence.
    pub observation: BuildObservation,
}

/// Typed command, wrapper, parser, or correlation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// Stable checker code.
    pub code: &'static str,
    /// Responsible path, when one exists.
    pub path: Option<PathBuf>,
    /// Concise diagnostic detail.
    pub message: String,
}

impl Error {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, path: None, message: message.into() }
    }

    fn at(
        code: &'static str,
        path: impl Into<PathBuf>,
        message: impl Into<String>,
    ) -> Self {
        Self { code, path: Some(path.into()), message: message.into() }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = &self.path {
            return write!(
                formatter,
                "{}: {}: {}",
                self.code,
                path.display(),
                self.message
            );
        }
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

/// True when the current process was invoked by Cargo as the private wrapper.
#[must_use]
pub fn wrapper_requested() -> bool {
    std::env::var_os(WRAPPER_MODE).as_deref() == Some(OsStr::new("1"))
}

/// True when Cargo invoked this image as the closed rustc probe launcher.
#[must_use]
pub fn rustc_launcher_requested() -> bool {
    std::env::var_os(RUSTC_LAUNCHER_MODE).as_deref() == Some(OsStr::new("1"))
}

/// Runs one exact rustc discovery probe with the relocated library authority.
///
/// # Errors
///
/// Returns a typed failure for changed controls, non-probe argv, invalid text,
/// spawn uncertainty, or a signal-terminated compiler.
pub fn rustc_launcher_entry() -> Result<i32, Error> {
    let rustc = launcher_control_path(
        RUSTC_LAUNCHER_RUSTC,
        Path::new("/compiler/bin/rustc"),
    )?;
    let library = launcher_control_path(
        RUSTC_LAUNCHER_LIBRARY,
        Path::new(COMPILER_LIBRARY_PATH),
    )?;
    let arguments = launcher_arguments(&rustc)?;
    validate_launcher_probe(&arguments)?;
    let status = Command::new(&rustc)
        .args(arguments.iter().skip(1))
        .env_clear()
        .env("LD_LIBRARY_PATH", library)
        .env("TERM", "dumb")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|failure| {
            Error::at(
                "COMPILER_LAUNCHER_EXECUTION",
                &rustc,
                failure.to_string(),
            )
        })?;
    status.code().ok_or_else(|| {
        Error::new(
            "COMPILER_LAUNCHER_SIGNAL",
            "rustc probe terminated without an exit code",
        )
    })
}

fn validate_launcher_probe(arguments: &[String]) -> Result<(), Error> {
    if arguments.get(1..).is_some_and(|tail| tail == ["--print=sysroot"]) {
        return Ok(());
    }
    let kind = classify_invocation(arguments)?;
    if kind != InvocationKind::CompilerUnit {
        return validate_probe(arguments, kind);
    }
    Err(Error::new(
        "COMPILER_LAUNCHER_UNIT_UNAVAILABLE",
        "probe launcher cannot execute a compiler unit",
    ))
}

fn launcher_control_path(
    name: &str,
    expected: &Path,
) -> Result<PathBuf, Error> {
    let value = std::env::var_os(name).ok_or_else(|| {
        Error::new(
            "COMPILER_LAUNCHER_CONTROL",
            format!("launcher control {name} is absent"),
        )
    })?;
    let path = PathBuf::from(value);
    if path == expected {
        return Ok(path);
    }
    Err(Error::at(
        "COMPILER_LAUNCHER_CONTROL",
        &path,
        format!("launcher control {name} differs from its exact path"),
    ))
}

fn launcher_arguments(rustc: &Path) -> Result<Vec<String>, Error> {
    let rustc = path_text(rustc)?.to_owned();
    let mut output = vec![rustc];
    let mut bytes = output[0].len();
    for argument in std::env::args_os().skip(1) {
        let text = argument.into_string().map_err(|_| {
            Error::new(
                "COMPILER_LAUNCHER_ARGV_UTF8",
                "rustc probe argument is not UTF-8",
            )
        })?;
        if text.is_empty() || text.contains(['\0', '\r', '\n']) {
            return Err(Error::new(
                "COMPILER_LAUNCHER_ARGV_FORM",
                "rustc probe argument is empty or contains framing bytes",
            ));
        }
        bytes = bytes.checked_add(text.len()).ok_or_else(|| {
            Error::new(
                "COMPILER_LAUNCHER_ARGV_LIMIT",
                "rustc probe argv bytes overflow",
            )
        })?;
        output.push(text);
    }
    if output.len() <= 4_096 && bytes <= 1_048_576 {
        return Ok(output);
    }
    Err(Error::new(
        "COMPILER_LAUNCHER_ARGV_LIMIT",
        "rustc probe argv exceeds its fixed bootstrap bound",
    ))
}

/// Constructs the exact shell-free Cargo build invocation.
///
/// # Errors
///
/// Returns a typed failure for an invalid closure, path authority, writable
/// directory, provider selection, or zero limit.
pub fn command_spec(
    request: &BuildRequest<'_>,
    limits: Limits,
) -> Result<CommandSpec, Error> {
    validate_limits(limits)?;
    validate_build_request(request, limits)?;
    let manifest =
        request.repository_root.join(&request.closure.workspace.manifest);
    let build_dir = request.target_dir.join("build-dir");
    let arguments = build_arguments(request, &manifest);
    let environment = build_environment(request, limits, &build_dir);
    Ok(CommandSpec {
        program: request.cargo.to_path_buf(),
        arguments,
        current_dir: PathBuf::from("/"),
        environment,
        clear_environment: true,
    })
}

fn build_arguments(
    request: &BuildRequest<'_>,
    manifest: &Path,
) -> Vec<OsString> {
    [
        OsString::from("build"),
        OsString::from("--quiet"),
        OsString::from("--locked"),
        OsString::from("--offline"),
        OsString::from("--manifest-path"),
        manifest.as_os_str().to_owned(),
        OsString::from("--package"),
        OsString::from(&request.closure.package.name),
        OsString::from("--lib"),
        OsString::from("--target"),
        OsString::from(TARGET_TRIPLE),
        OsString::from("--profile"),
        OsString::from("release"),
        OsString::from("--jobs"),
        OsString::from("1"),
        OsString::from("--message-format=json"),
        OsString::from("--target-dir"),
        request.target_dir.as_os_str().to_owned(),
    ]
    .into()
}

fn build_environment(
    request: &BuildRequest<'_>,
    limits: Limits,
    build_dir: &Path,
) -> BTreeMap<OsString, OsString> {
    let values = [
        ("CARGO_BUILD_BUILD_DIR", build_dir.as_os_str().to_owned()),
        ("CARGO_HOME", request.cargo_home.as_os_str().to_owned()),
        ("CARGO_INCREMENTAL", OsString::from("0")),
        ("CARGO_NET_OFFLINE", OsString::from("true")),
        ("LD_LIBRARY_PATH", OsString::from(COMPILER_LIBRARY_PATH)),
        ("CARGO_TARGET_DIR", request.target_dir.as_os_str().to_owned()),
        ("RUSTC", request.rustc.as_os_str().to_owned()),
        ("RUSTC_WRAPPER", request.checker.as_os_str().to_owned()),
        ("TEMP", request.temp_dir.as_os_str().to_owned()),
        ("TERM", OsString::from("dumb")),
        ("TMP", request.temp_dir.as_os_str().to_owned()),
        ("TMPDIR", request.temp_dir.as_os_str().to_owned()),
        (WRAPPER_MODE, OsString::from("1")),
        (ENV_RUSTC, request.rustc.as_os_str().to_owned()),
        (ENV_SYSROOT, request.sysroot.as_os_str().to_owned()),
        (ENV_CARGO_HOME, request.cargo_home.as_os_str().to_owned()),
        (ENV_TEMP, request.temp_dir.as_os_str().to_owned()),
        (ENV_BUILD_DIR, build_dir.as_os_str().to_owned()),
        (ENV_REPOSITORY, request.repository_root.as_os_str().to_owned()),
        (ENV_TARGET, request.target_dir.as_os_str().to_owned()),
        (ENV_TRACE, request.trace_dir.as_os_str().to_owned()),
        (ENV_DEP_INFO, request.dep_info_dir.as_os_str().to_owned()),
        (ENV_PATH_BYTES, OsString::from(limits.path_bytes.to_string())),
        (
            ENV_TRACE_BYTES,
            OsString::from(limits.wrapper_trace_bytes.to_string()),
        ),
        (ENV_ARGV_ITEMS, OsString::from(limits.wrapper_argv_items.to_string())),
        (ENV_ARGV_BYTES, OsString::from(limits.wrapper_argv_bytes.to_string())),
        (
            ENV_TRACE_COUNT,
            OsString::from(limits.wrapper_trace_count.to_string()),
        ),
        (
            ENV_DEP_INFO_BYTES,
            OsString::from(limits.dep_info_file_bytes.to_string()),
        ),
    ];
    values
        .into_iter()
        .map(|(name, value)| (OsString::from(name), value))
        .collect()
}

fn validate_limits(limits: Limits) -> Result<(), Error> {
    let positive = limits.path_bytes > 0
        && limits.cargo_json_bytes > 0
        && limits.cargo_message_count > 0
        && limits.wrapper_trace_bytes > 0
        && limits.wrapper_argv_items > 0
        && limits.wrapper_argv_bytes > 0
        && limits.wrapper_trace_count > 0
        && limits.dep_info_file_bytes > 0
        && limits.dep_info_total_bytes > 0
        && limits.compiler_input_count > 0
        && limits.artifact_count > 0
        && limits.artifact_total_bytes > 0
        && limits.json_depth > 0
        && limits.json_items > 0;
    if !positive {
        return Err(Error::new(
            "COMPILER_CAPTURE_ZERO_LIMIT",
            "every compiler capture limit must be positive",
        ));
    }
    Ok(())
}

fn validate_build_request(
    request: &BuildRequest<'_>,
    limits: Limits,
) -> Result<(), Error> {
    validate_closure(request.closure)?;
    validate_nonempty_text(request.package_id, "Cargo package ID")?;
    let readonly = [
        (request.repository_root, PathKind::Directory, "repository"),
        (request.cargo, PathKind::File, "Cargo"),
        (request.rustc, PathKind::File, "rustc"),
        (request.checker, PathKind::File, "checker"),
        (request.sysroot, PathKind::Directory, "sysroot"),
    ];
    for (path, kind, name) in readonly {
        validate_absolute_path(path, name, limits.path_bytes)?;
        validate_canonical_path(path, kind, name)?;
    }
    if request.sysroot != Path::new("/sysroot") {
        return Err(Error::at(
            "COMPILER_SYSROOT_AUTHORITY",
            request.sysroot,
            "prepared compiler sysroot must be mounted at /sysroot",
        ));
    }
    for (path, name) in [
        (request.scratch_root, "scratch root"),
        (request.cargo_home, "Cargo home"),
        (request.temp_dir, "temporary directory"),
        (request.target_dir, "target directory"),
        (request.trace_dir, "trace directory"),
        (request.dep_info_dir, "dep-info directory"),
    ] {
        validate_absolute_path(path, name, limits.path_bytes)?;
    }
    validate_writable_paths(request)?;
    validate_manifest(request)?;
    validate_provider_selection(request)?;
    validate_metadata_preflight(request)?;
    Ok(())
}

fn validate_closure(closure: &Closure) -> Result<(), Error> {
    if closure.target.kind != ["lib"]
        || closure.target.crate_types != ["lib"]
        || closure.cargo.profile != "release"
    {
        return Err(Error::new(
            "COMPILER_CAPTURE_CLOSURE",
            "closure is outside the exact release library profile",
        ));
    }
    if closure.package.name.is_empty() || closure.target.name.is_empty() {
        return Err(Error::new(
            "COMPILER_CAPTURE_CLOSURE",
            "closure package and target names must be nonempty",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum PathKind {
    File,
    Directory,
}

fn validate_canonical_path(
    path: &Path,
    kind: PathKind,
    name: &str,
) -> Result<(), Error> {
    validate_absolute_path(path, name, usize::MAX)?;
    let metadata = fs::symlink_metadata(path).map_err(|failure| {
        Error::at("COMPILER_CAPTURE_PATH_METADATA", path, failure.to_string())
    })?;
    if metadata.file_type().is_symlink() {
        return Err(Error::at(
            "COMPILER_CAPTURE_PATH_KIND",
            path,
            format!("{name} cannot be a symlink"),
        ));
    }
    let valid_kind = match kind {
        PathKind::File => metadata.is_file(),
        PathKind::Directory => metadata.is_dir(),
    };
    if !valid_kind {
        return Err(Error::at(
            "COMPILER_CAPTURE_PATH_KIND",
            path,
            format!("{name} has the wrong filesystem kind"),
        ));
    }
    let canonical = fs::canonicalize(path).map_err(|failure| {
        Error::at("COMPILER_CAPTURE_PATH_CANONICAL", path, failure.to_string())
    })?;
    if canonical != path {
        return Err(Error::at(
            "COMPILER_CAPTURE_PATH_CANONICAL",
            path,
            format!("{name} path differs from its canonical identity"),
        ));
    }
    Ok(())
}

fn validate_writable_paths(request: &BuildRequest<'_>) -> Result<(), Error> {
    validate_canonical_path(
        request.scratch_root,
        PathKind::Directory,
        "scratch root",
    )?;
    let writable = [
        (request.cargo_home, "Cargo home"),
        (request.temp_dir, "temporary directory"),
        (request.target_dir, "target directory"),
        (request.trace_dir, "trace directory"),
        (request.dep_info_dir, "dep-info directory"),
    ];
    for (path, name) in writable {
        validate_canonical_path(path, PathKind::Directory, name)?;
        if !path.starts_with(request.scratch_root)
            || path == request.scratch_root
        {
            return Err(Error::at(
                "COMPILER_CAPTURE_SCRATCH_AUTHORITY",
                path,
                format!("{name} is outside the scratch authority"),
            ));
        }
    }
    reject_overlapping_writable_paths(&writable)?;
    require_empty_directory(request.trace_dir, "trace directory")?;
    require_empty_directory(request.dep_info_dir, "dep-info directory")?;
    if request.closure.kind == ClosureKind::Shipping {
        require_empty_directory(request.target_dir, "shipping target")?;
    }
    Ok(())
}

fn reject_overlapping_writable_paths(
    paths: &[(&Path, &str)],
) -> Result<(), Error> {
    if let Some((left, right)) = first_path_overlap(paths) {
        return Err(Error::new(
            "COMPILER_CAPTURE_PATH_OVERLAP",
            format!("{left} overlaps {right}"),
        ));
    }
    Ok(())
}

fn first_path_overlap<'a>(
    paths: &[(&Path, &'a str)],
) -> Option<(&'a str, &'a str)> {
    paths.iter().enumerate().find_map(|(index, (left, left_name))| {
        paths.iter().skip(index + 1).find_map(|(right, right_name)| {
            (left.starts_with(right) || right.starts_with(left))
                .then_some((*left_name, *right_name))
        })
    })
}

fn require_empty_directory(path: &Path, name: &str) -> Result<(), Error> {
    let mut entries = fs::read_dir(path).map_err(|failure| {
        Error::at("COMPILER_CAPTURE_DIRECTORY_READ", path, failure.to_string())
    })?;
    if entries
        .next()
        .transpose()
        .map_err(|failure| {
            Error::at(
                "COMPILER_CAPTURE_DIRECTORY_READ",
                path,
                failure.to_string(),
            )
        })?
        .is_some()
    {
        return Err(Error::at(
            "COMPILER_CAPTURE_DIRECTORY_NOT_EMPTY",
            path,
            format!("{name} must be initially empty"),
        ));
    }
    Ok(())
}

fn validate_manifest(request: &BuildRequest<'_>) -> Result<(), Error> {
    let expected =
        request.repository_root.join(&request.closure.workspace.manifest);
    validate_canonical_path(&expected, PathKind::File, "workspace manifest")?;
    if !expected.starts_with(request.repository_root) {
        return Err(Error::at(
            "COMPILER_CAPTURE_MANIFEST",
            expected,
            "workspace manifest escapes the repository root",
        ));
    }
    Ok(())
}

fn validate_metadata_preflight(
    request: &BuildRequest<'_>,
) -> Result<(), Error> {
    let package =
        request.metadata.packages.get(request.package_id).ok_or_else(|| {
            Error::new(
                "COMPILER_METADATA_PACKAGE",
                "selected package ID is absent from Cargo metadata",
            )
        })?;
    let manifest =
        request.repository_root.join(&request.closure.package.manifest);
    if package.name != request.closure.package.name
        || package.manifest_path != manifest
        || package.source.is_some()
        || package.checksum.is_some()
    {
        return Err(Error::new(
            "COMPILER_METADATA_PACKAGE",
            "selected Cargo package differs from its local closure",
        ));
    }
    let reachable = metadata_reachable(request.metadata, request.package_id)?;
    validate_reachable_packages(request.metadata, &reachable)?;
    validate_metadata_targets(package, request)?;
    validate_metadata_dependencies(request, &reachable)
}

fn validate_metadata_targets(
    package: &crate::cargo::Package,
    request: &BuildRequest<'_>,
) -> Result<(), Error> {
    if package.targets.len() != 1 {
        return Err(Error::new(
            "COMPILER_METADATA_TARGET",
            "selected package does not have exactly one target",
        ));
    }
    let target = &package.targets[0];
    let source = request.repository_root.join(&request.closure.target.src_path);
    if target.name != request.closure.target.name
        || target.kind != ["lib"]
        || target.crate_types != ["lib"]
        || target.src_path != source
    {
        return Err(Error::new(
            "COMPILER_METADATA_TARGET",
            "Cargo target differs from the registered library closure",
        ));
    }
    Ok(())
}

fn metadata_reachable(
    metadata: &crate::cargo::Metadata,
    root: &str,
) -> Result<BTreeSet<String>, Error> {
    let mut output = BTreeSet::new();
    let mut pending = vec![root.to_owned()];
    while let Some(package) = pending.pop() {
        if !output.insert(package.clone()) {
            continue;
        }
        let node = metadata.nodes.get(&package).ok_or_else(|| {
            Error::new(
                "COMPILER_METADATA_NODE",
                format!("metadata node {package} is absent"),
            )
        })?;
        pending
            .extend(node.dependencies.iter().map(|edge| edge.package.clone()));
    }
    Ok(output)
}

fn validate_reachable_packages(
    metadata: &crate::cargo::Metadata,
    reachable: &BTreeSet<String>,
) -> Result<(), Error> {
    for package_id in reachable {
        let package = metadata.packages.get(package_id).ok_or_else(|| {
            Error::new(
                "COMPILER_METADATA_PACKAGE",
                format!("reachable package {package_id} is absent"),
            )
        })?;
        if package.source.is_some()
            || !metadata.workspace_members.contains(package_id)
        {
            return Err(Error::new(
                "COMPILER_EXTERNAL_PACKAGE_UNAVAILABLE",
                format!("reachable package {package_id} is not local Product"),
            ));
        }
        if package_has_helper_target(package) {
            return Err(Error::new(
                "PRODUCT_HELPER_PROOF_UNAVAILABLE",
                format!("reachable package {package_id} has a helper target"),
            ));
        }
    }
    Ok(())
}

fn package_has_helper_target(package: &crate::cargo::Package) -> bool {
    package
        .targets
        .iter()
        .flat_map(|target| &target.kind)
        .any(|kind| matches!(kind.as_str(), "custom-build" | "proc-macro"))
}

fn validate_metadata_dependencies(
    request: &BuildRequest<'_>,
    reachable: &BTreeSet<String>,
) -> Result<(), Error> {
    let node = request
        .metadata
        .nodes
        .get(request.package_id)
        .expect("reachable root node checked above");
    if request.closure.kind == ClosureKind::Shipping {
        if reachable != &BTreeSet::from([request.package_id.to_owned()]) {
            return Err(Error::new(
                "COMPILER_SHIPPING_DEPENDENCY",
                "shipping closure has a non-sysroot dependency",
            ));
        }
        return Ok(());
    }
    let provider = request.provider.expect("selection checked before metadata");
    if reachable
        != &BTreeSet::from([
            request.package_id.to_owned(),
            provider.package_id.clone(),
        ])
        || node.dependencies.len() != 1
    {
        return Err(Error::new(
            "COMPILER_VALIDATION_DEPENDENCY",
            "validation dependency closure is not exactly its provider",
        ));
    }
    let edge = &node.dependencies[0];
    let normal = edge.kinds.len() == 1
        && edge.kinds[0].kind == "normal"
        && edge.kinds[0].target.is_none();
    if edge.name != "reovim_foundation"
        || edge.package != provider.package_id
        || !normal
    {
        return Err(Error::new(
            "COMPILER_VALIDATION_DEPENDENCY",
            "validation provider edge has the wrong name, kind, or target",
        ));
    }
    Ok(())
}

fn validate_provider_selection(
    request: &BuildRequest<'_>,
) -> Result<(), Error> {
    match (request.closure.kind, request.provider) {
        (ClosureKind::Shipping, None) => Ok(()),
        (ClosureKind::Validation, Some(provider)) => {
            if provider.target_dir != request.target_dir
                || provider.cargo_home != request.cargo_home
            {
                return Err(Error::new(
                    "COMPILER_PROVIDER_TARGET",
                    "validation does not reuse shipping Cargo/target authority",
                ));
            }
            Ok(())
        }
        (ClosureKind::Shipping, Some(_)) => Err(Error::new(
            "COMPILER_PROVIDER_UNEXPECTED",
            "shipping build cannot consume prior provider evidence",
        )),
        (ClosureKind::Validation, None) => Err(Error::new(
            "COMPILER_PROVIDER_MISSING",
            "validation build requires prior shipping provider evidence",
        )),
    }
}

fn validate_nonempty_text(value: &str, name: &str) -> Result<(), Error> {
    let valid = !value.is_empty()
        && !value.contains(['\0', '\r', '\n'])
        && !value.bytes().any(|byte| byte.is_ascii_control());
    if !valid {
        return Err(Error::new(
            "COMPILER_CAPTURE_TEXT",
            format!("{name} is empty or contains a control character"),
        ));
    }
    Ok(())
}

fn validate_absolute_path(
    path: &Path,
    name: &str,
    byte_limit: usize,
) -> Result<(), Error> {
    let text = path.to_str().ok_or_else(|| {
        Error::at(
            "COMPILER_CAPTURE_PATH_UTF8",
            path,
            format!("{name} path is not UTF-8"),
        )
    })?;
    let normalized = path.is_absolute()
        && text.len() <= byte_limit
        && text != "/"
        && !text.ends_with('/')
        && !text.contains(['\0', '\r', '\n', '\\'])
        && !text.contains("//")
        && path.components().all(|part| {
            matches!(part, Component::RootDir | Component::Normal(_))
        });
    if !normalized {
        return Err(Error::at(
            "COMPILER_CAPTURE_PATH_FORM",
            path,
            format!("{name} path is not normalized absolute UTF-8"),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct WrapperLimits {
    path_bytes: usize,
    trace_bytes: usize,
    argv_items: usize,
    argv_bytes: usize,
    trace_count: usize,
    dep_info_bytes: usize,
}

struct WrapperConfig {
    rustc: PathBuf,
    sysroot: PathBuf,
    cargo_home: PathBuf,
    temp_dir: PathBuf,
    build_dir: PathBuf,
    repository_root: PathBuf,
    target_dir: PathBuf,
    trace_dir: PathBuf,
    dep_info_dir: PathBuf,
    limits: WrapperLimits,
}

#[derive(Debug)]
struct InvocationPlan {
    kind: InvocationKind,
    original: Vec<String>,
    effective: Vec<String>,
    environment: Vec<EnvironmentEntry>,
    dep_info_path: Option<PathBuf>,
    cargo_dep_info_path: Option<PathBuf>,
}

/// Executes one same-binary `RUSTC_WRAPPER` invocation.
///
/// The caller should exit the wrapper process with the returned status. Rustc
/// stdout and stderr remain inherited so Cargo retains its native protocol.
///
/// # Errors
///
/// Returns a typed failure for invalid wrapper control state, unsupported
/// rustc mechanisms, trace publication failure, rustc spawn uncertainty, or
/// dep-info reconciliation failure.
pub fn wrapper_entry() -> Result<i32, Error> {
    let environment = std::env::vars_os().collect::<BTreeMap<_, _>>();
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    wrapper_entry_with(&arguments, &environment)
}

fn wrapper_entry_with(
    arguments: &[OsString],
    environment: &BTreeMap<OsString, OsString>,
) -> Result<i32, Error> {
    let config = wrapper_config(environment)?;
    let child_environment = wrapper_child_environment(environment, &config)?;
    let ordinal = allocate_ordinal(&config)?;
    let mut claim = ClaimGuard::new(&config.trace_dir, ordinal);
    let plan = plan_invocation(arguments, child_environment, &config, ordinal)?;
    publish_start(&config, ordinal, &plan)?;
    let status = execute_rustc(&config, &plan)?;
    let rustc_status = status.code().unwrap_or(1);
    let mut wrapper_status = rustc_status;
    if status.success()
        && let Err(error) = reconcile_dep_info(&config, &plan)
    {
        publish_finish(&config, ordinal, rustc_status, 1)?;
        claim.release()?;
        return Err(error);
    }
    if status.code().is_none() {
        wrapper_status = 1;
    }
    publish_finish(&config, ordinal, rustc_status, wrapper_status)?;
    claim.release()?;
    Ok(wrapper_status)
}

fn wrapper_config(
    environment: &BTreeMap<OsString, OsString>,
) -> Result<WrapperConfig, Error> {
    let limits = WrapperLimits {
        path_bytes: control_usize(environment, ENV_PATH_BYTES, 65_536)?,
        trace_bytes: control_usize(environment, ENV_TRACE_BYTES, 16_777_216)?,
        argv_items: control_usize(environment, ENV_ARGV_ITEMS, 4096)?,
        argv_bytes: control_usize(environment, ENV_ARGV_BYTES, 1_048_576)?,
        trace_count: control_usize(environment, ENV_TRACE_COUNT, 256)?,
        dep_info_bytes: control_usize(
            environment,
            ENV_DEP_INFO_BYTES,
            4_194_304,
        )?,
    };
    let config = WrapperConfig {
        rustc: control_path(environment, ENV_RUSTC, limits.path_bytes)?,
        sysroot: control_path(environment, ENV_SYSROOT, limits.path_bytes)?,
        cargo_home: control_path(
            environment,
            ENV_CARGO_HOME,
            limits.path_bytes,
        )?,
        temp_dir: control_path(environment, ENV_TEMP, limits.path_bytes)?,
        build_dir: control_path(environment, ENV_BUILD_DIR, limits.path_bytes)?,
        repository_root: control_path(
            environment,
            ENV_REPOSITORY,
            limits.path_bytes,
        )?,
        target_dir: control_path(environment, ENV_TARGET, limits.path_bytes)?,
        trace_dir: control_path(environment, ENV_TRACE, limits.path_bytes)?,
        dep_info_dir: control_path(
            environment,
            ENV_DEP_INFO,
            limits.path_bytes,
        )?,
        limits,
    };
    validate_wrapper_config(&config)?;
    Ok(config)
}

fn control_usize(
    environment: &BTreeMap<OsString, OsString>,
    name: &'static str,
    maximum: usize,
) -> Result<usize, Error> {
    let value = required_environment(environment, name)?;
    let text = value.to_str().ok_or_else(|| {
        Error::new("COMPILER_WRAPPER_CONTROL", format!("{name} is not UTF-8"))
    })?;
    let parsed = text.parse::<usize>().map_err(|failure| {
        Error::new("COMPILER_WRAPPER_CONTROL", failure.to_string())
    })?;
    if parsed == 0 || parsed > maximum {
        return Err(Error::new(
            "COMPILER_WRAPPER_CONTROL",
            format!("{name} is outside 1..={maximum}"),
        ));
    }
    Ok(parsed)
}

fn control_path(
    environment: &BTreeMap<OsString, OsString>,
    name: &'static str,
    limit: usize,
) -> Result<PathBuf, Error> {
    let value = required_environment(environment, name)?;
    let path = PathBuf::from(value);
    validate_absolute_path(&path, name, limit)?;
    Ok(path)
}

fn required_environment<'a>(
    environment: &'a BTreeMap<OsString, OsString>,
    name: &'static str,
) -> Result<&'a OsString, Error> {
    environment.get(OsStr::new(name)).ok_or_else(|| {
        Error::new(
            "COMPILER_WRAPPER_CONTROL",
            format!("required wrapper control {name} is absent"),
        )
    })
}

fn validate_wrapper_config(config: &WrapperConfig) -> Result<(), Error> {
    validate_canonical_path(&config.rustc, PathKind::File, "wrapper rustc")?;
    validate_canonical_path(
        &config.sysroot,
        PathKind::Directory,
        "wrapper sysroot",
    )?;
    if config.sysroot != Path::new("/sysroot") {
        return Err(Error::at(
            "COMPILER_SYSROOT_AUTHORITY",
            &config.sysroot,
            "wrapper sysroot must be exactly /sysroot",
        ));
    }
    for (path, name) in [
        (&config.cargo_home, "wrapper Cargo home"),
        (&config.temp_dir, "wrapper temporary directory"),
        (&config.repository_root, "wrapper repository"),
        (&config.target_dir, "wrapper target"),
        (&config.trace_dir, "wrapper trace"),
        (&config.dep_info_dir, "wrapper dep-info"),
    ] {
        validate_canonical_path(path, PathKind::Directory, name)?;
    }
    if config.build_dir != config.target_dir.join("build-dir") {
        return Err(Error::at(
            "COMPILER_BUILD_DIR_AUTHORITY",
            &config.build_dir,
            "wrapper build directory differs from target/build-dir",
        ));
    }
    if config.trace_dir.starts_with(&config.target_dir)
        || config.dep_info_dir.starts_with(&config.target_dir)
        || config.trace_dir.starts_with(&config.dep_info_dir)
        || config.dep_info_dir.starts_with(&config.trace_dir)
    {
        return Err(Error::new(
            "COMPILER_WRAPPER_PATH_OVERLAP",
            "trace, dep-info, and target authorities overlap",
        ));
    }
    Ok(())
}

fn wrapper_child_environment(
    environment: &BTreeMap<OsString, OsString>,
    config: &WrapperConfig,
) -> Result<Vec<EnvironmentEntry>, Error> {
    let mut output = Vec::new();
    let mut total = 0_usize;
    for (name, value) in environment {
        let name = name.to_str().ok_or_else(|| {
            Error::new(
                "COMPILER_WRAPPER_ENVIRONMENT",
                "environment name is not UTF-8",
            )
        })?;
        if wrapper_control_name(name) || name == "LD_LIBRARY_PATH" {
            continue;
        }
        reject_unavailable_environment(name)?;
        if !allowed_wrapper_environment(name) {
            return Err(Error::new(
                "COMPILER_WRAPPER_ENVIRONMENT",
                format!("environment variable {name} is not admitted"),
            ));
        }
        let value = value.to_str().ok_or_else(|| {
            Error::new(
                "COMPILER_WRAPPER_ENVIRONMENT",
                format!("environment value {name} is not UTF-8"),
            )
        })?;
        total = charge_text(total, name, config.limits.argv_bytes)?;
        total = charge_text(total, value, config.limits.argv_bytes)?;
        output.push(EnvironmentEntry {
            name: name.to_owned(),
            value: value.to_owned(),
        });
    }
    output.push(EnvironmentEntry {
        name: "LD_LIBRARY_PATH".to_owned(),
        value: COMPILER_LIBRARY_PATH.to_owned(),
    });
    output.sort();
    validate_required_child_environment(&output, config)?;
    Ok(output)
}

fn reject_unavailable_environment(name: &str) -> Result<(), Error> {
    if name == "OUT_DIR" || name.starts_with("DEP_") {
        return Err(Error::new(
            "PRODUCT_GENERATOR_PROOF_UNAVAILABLE",
            format!("compiler environment {name} implies a helper output"),
        ));
    }
    if matches!(
        name,
        "RUSTFLAGS" | "CARGO_ENCODED_RUSTFLAGS" | "RUSTC_WORKSPACE_WRAPPER"
    ) {
        return Err(Error::new(
            "COMPILER_ENVIRONMENT_INPUT_UNAVAILABLE",
            format!("compiler environment {name} is unavailable"),
        ));
    }
    Ok(())
}

fn wrapper_control_name(name: &str) -> bool {
    name.starts_with(CONTROL_PREFIX) || name == "RUSTC_WRAPPER"
}

fn allowed_wrapper_environment(name: &str) -> bool {
    matches!(
        name,
        "CARGO"
            | "CARGO_BUILD_BUILD_DIR"
            | "CARGO_CRATE_NAME"
            | "CARGO_HOME"
            | "CARGO_INCREMENTAL"
            | "CARGO_MAKEFLAGS"
            | "CARGO_MANIFEST_DIR"
            | "CARGO_MANIFEST_PATH"
            | "CARGO_NET_OFFLINE"
            | "CARGO_PRIMARY_PACKAGE"
            | "CARGO_SBOM_PATH"
            | "CARGO_TARGET_DIR"
            | "LD_LIBRARY_PATH"
            | "RUSTC"
            | "TEMP"
            | "TERM"
            | "TMP"
            | "TMPDIR"
    ) || name.starts_with("CARGO_PKG_")
}

fn validate_required_child_environment(
    environment: &[EnvironmentEntry],
    config: &WrapperConfig,
) -> Result<(), Error> {
    let values = environment
        .iter()
        .map(|entry| (entry.name.as_str(), entry.value.as_str()))
        .collect::<BTreeMap<_, _>>();
    let required = [
        ("CARGO_INCREMENTAL", "0"),
        ("CARGO_NET_OFFLINE", "true"),
        ("LD_LIBRARY_PATH", COMPILER_LIBRARY_PATH),
        ("TERM", "dumb"),
    ];
    if required
        .iter()
        .any(|(name, value)| values.get(name).copied() != Some(*value))
    {
        return Err(Error::new(
            "COMPILER_WRAPPER_ENVIRONMENT",
            "fixed Cargo wrapper environment is absent or changed",
        ));
    }
    let path_required = [
        ("CARGO_BUILD_BUILD_DIR", &config.build_dir),
        ("CARGO_HOME", &config.cargo_home),
        ("CARGO_TARGET_DIR", &config.target_dir),
        ("RUSTC", &config.rustc),
        ("TEMP", &config.temp_dir),
        ("TMP", &config.temp_dir),
        ("TMPDIR", &config.temp_dir),
    ];
    for (name, expected) in path_required {
        if values.get(name).copied() != expected.to_str() {
            return Err(Error::new(
                "COMPILER_WRAPPER_ENVIRONMENT",
                format!("{name} does not match its exact authority"),
            ));
        }
    }
    if values.contains_key("OUT_DIR") {
        return Err(Error::new(
            "PRODUCT_GENERATOR_PROOF_UNAVAILABLE",
            "OUT_DIR is unavailable in the authored profile",
        ));
    }
    Ok(())
}

fn charge_text(
    current: usize,
    value: &str,
    limit: usize,
) -> Result<usize, Error> {
    let total = current.checked_add(value.len()).ok_or_else(|| {
        Error::new("COMPILER_WRAPPER_ARGV_LIMIT", "byte count overflow")
    })?;
    if total > limit {
        return Err(Error::new(
            "COMPILER_WRAPPER_ARGV_LIMIT",
            format!("argument/environment bytes exceed {limit}"),
        ));
    }
    Ok(total)
}

fn allocate_ordinal(config: &WrapperConfig) -> Result<u64, Error> {
    for ordinal in 0..config.limits.trace_count {
        let ordinal = u64::try_from(ordinal).map_err(|failure| {
            Error::new("COMPILER_WRAPPER_TRACE_LIMIT", failure.to_string())
        })?;
        if start_path(&config.trace_dir, ordinal).exists()
            || finish_path(&config.trace_dir, ordinal).exists()
        {
            continue;
        }
        let claim = claim_path(&config.trace_dir, ordinal);
        match OpenOptions::new().write(true).create_new(true).open(&claim) {
            Ok(mut file) => {
                file.write_all(b"reovim-rustc-wrapper-claim/v1\n")
                    .map_err(|failure| trace_io(&claim, &failure))?;
                file.sync_all()
                    .map_err(|failure| trace_io(&claim, &failure))?;
                return Ok(ordinal);
            }
            Err(failure)
                if failure.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(failure) => return Err(trace_io(&claim, &failure)),
        }
    }
    Err(Error::new(
        "COMPILER_WRAPPER_TRACE_LIMIT",
        "wrapper invocation count exhausted its bound",
    ))
}

struct ClaimGuard {
    path: PathBuf,
    released: bool,
}

impl ClaimGuard {
    fn new(trace_dir: &Path, ordinal: u64) -> Self {
        Self { path: claim_path(trace_dir, ordinal), released: false }
    }

    fn release(&mut self) -> Result<(), Error> {
        fs::remove_file(&self.path).map_err(|failure| {
            Error::at(
                "COMPILER_WRAPPER_TRACE_IO",
                &self.path,
                failure.to_string(),
            )
        })?;
        self.released = true;
        Ok(())
    }
}

impl Drop for ClaimGuard {
    fn drop(&mut self) {
        if !self.released {
            let _ = fs::remove_file(&self.path);
        }
    }
}

struct UnitArguments {
    crate_name: String,
    crate_types: BTreeSet<String>,
    target: String,
    cfgs: BTreeSet<String>,
    features: BTreeSet<String>,
    out_dir: PathBuf,
    extra_filename: String,
    source: PathBuf,
    externs: BTreeSet<ExternEdge>,
    emit: EmitLocation,
}

struct EmitLocation {
    flag_index: usize,
    value_index: Option<usize>,
}

fn plan_invocation(
    arguments: &[OsString],
    environment: Vec<EnvironmentEntry>,
    config: &WrapperConfig,
    ordinal: u64,
) -> Result<InvocationPlan, Error> {
    let original = utf8_arguments(arguments, config.limits)?;
    require_wrapper_rustc(&original, config)?;
    reject_dangerous_arguments(&original, &config.target_dir)?;
    let kind = classify_invocation(&original)?;
    if kind != InvocationKind::CompilerUnit {
        validate_probe(&original, kind)?;
        let effective = inject_sysroot(&original, &config.sysroot)?;
        return Ok(InvocationPlan {
            kind,
            original,
            effective,
            environment,
            dep_info_path: None,
            cargo_dep_info_path: None,
        });
    }
    let unit = parse_unit_arguments(&original, config)?;
    let dep_info_path = config.dep_info_dir.join(format!("{ordinal:08}.d"));
    validate_absolute_path(
        &dep_info_path,
        "private dep-info",
        config.limits.path_bytes,
    )?;
    let cargo_dep_info_path = unit
        .out_dir
        .join(format!("{}{}.d", unit.crate_name, unit.extra_filename));
    let rewritten = rewrite_emit(&original, &unit.emit, &dep_info_path)?;
    let effective = inject_sysroot(&rewritten, &config.sysroot)?;
    Ok(InvocationPlan {
        kind,
        original,
        effective,
        environment,
        dep_info_path: Some(dep_info_path),
        cargo_dep_info_path: Some(cargo_dep_info_path),
    })
}

fn utf8_arguments(
    arguments: &[OsString],
    limits: WrapperLimits,
) -> Result<Vec<String>, Error> {
    if arguments.is_empty() || arguments.len() > limits.argv_items {
        return Err(Error::new(
            "COMPILER_WRAPPER_ARGV_LIMIT",
            "wrapper argv count is empty or exceeds its bound",
        ));
    }
    let mut total = 0_usize;
    let mut output = Vec::with_capacity(arguments.len());
    for argument in arguments {
        let text = argument.to_str().ok_or_else(|| {
            Error::new(
                "COMPILER_WRAPPER_ARGV_UTF8",
                "wrapper argument is not UTF-8",
            )
        })?;
        if text.is_empty() || text.contains(['\0', '\r', '\n']) {
            return Err(Error::new(
                "COMPILER_WRAPPER_ARGV_FORM",
                "wrapper argument is empty or contains framing bytes",
            ));
        }
        total = charge_text(total, text, limits.argv_bytes)?;
        output.push(text.to_owned());
    }
    Ok(output)
}

fn require_wrapper_rustc(
    arguments: &[String],
    config: &WrapperConfig,
) -> Result<(), Error> {
    if arguments.first().map(String::as_str) != config.rustc.to_str() {
        return Err(Error::new(
            "COMPILER_WRAPPER_RUSTC",
            "Cargo-supplied rustc differs from the bound compiler",
        ));
    }
    Ok(())
}

fn reject_dangerous_arguments(
    arguments: &[String],
    target_dir: &Path,
) -> Result<(), Error> {
    let mut cursor = 1_usize;
    while cursor < arguments.len() {
        let argument = &arguments[cursor];
        if argument.starts_with('@') {
            return Err(Error::new(
                "COMPILER_RESPONSE_FILE_UNAVAILABLE",
                "rustc response files are unavailable",
            ));
        }
        if argument == "-Z" || short_flag_value(argument, "-Z").is_some() {
            return Err(Error::new(
                "COMPILER_UNSTABLE_FLAG_UNAVAILABLE",
                "unstable rustc options are unavailable",
            ));
        }
        if argument == "--sysroot" || argument.starts_with("--sysroot=") {
            return Err(Error::new(
                "COMPILER_SYSROOT_FLAG_UNAVAILABLE",
                "original Cargo rustc argv cannot select a sysroot",
            ));
        }
        if argument == "-l" || short_flag_value(argument, "-l").is_some() {
            return Err(Error::new(
                "COMPILER_NATIVE_LINK_UNAVAILABLE",
                "native library flags are unavailable",
            ));
        }
        if let Some((value, next)) = option_value(arguments, cursor, "-L")? {
            validate_search_path(value, target_dir)?;
            cursor = next;
            continue;
        }
        if let Some((value, next)) = option_value(arguments, cursor, "-C")? {
            validate_codegen_option(value)?;
            cursor = next;
            continue;
        }
        if argument.starts_with("--link") {
            return Err(Error::new(
                "COMPILER_LINK_FLAG_UNAVAILABLE",
                "explicit linker mechanisms are unavailable",
            ));
        }
        cursor = cursor.saturating_add(1);
    }
    Ok(())
}

fn validate_search_path(value: &str, target_dir: &Path) -> Result<(), Error> {
    let Some(path) = value.strip_prefix("dependency=") else {
        return Err(Error::new(
            "COMPILER_SEARCH_PATH_UNAVAILABLE",
            "only Cargo dependency search paths are admitted",
        ));
    };
    let path = Path::new(path);
    validate_absolute_path(path, "dependency search path", usize::MAX)?;
    if !path.starts_with(target_dir) {
        return Err(Error::at(
            "COMPILER_SEARCH_PATH_UNAVAILABLE",
            path,
            "dependency search path escapes the target authority",
        ));
    }
    Ok(())
}

fn validate_codegen_option(value: &str) -> Result<(), Error> {
    let name = value.split_once('=').map_or(value, |(name, _)| name);
    let forbidden = name == "incremental"
        || name.starts_with("link")
        || name.contains("native")
        || name.contains("framework");
    if forbidden {
        return Err(Error::new(
            "COMPILER_CODEGEN_FLAG_UNAVAILABLE",
            format!("codegen option {name} is unavailable"),
        ));
    }
    Ok(())
}

fn classify_invocation(arguments: &[String]) -> Result<InvocationKind, Error> {
    if arguments.get(1..).is_some_and(|tail| tail == ["-vV"]) {
        return Ok(InvocationKind::VersionProbe);
    }
    let has_print = arguments.iter().any(|argument| {
        argument == "--print" || argument.starts_with("--print=")
    });
    if !has_print {
        return Ok(InvocationKind::CompilerUnit);
    }
    let target = collect_option_values(arguments, "--target")?;
    match target.as_slice() {
        [] => Ok(InvocationKind::HostProbe),
        [target] if target == TARGET_TRIPLE => Ok(InvocationKind::TargetProbe),
        _ => Err(Error::new(
            "COMPILER_DISCOVERY_PROBE_UNAVAILABLE",
            "discovery probe has an unsupported target selection",
        )),
    }
}

fn validate_probe(
    arguments: &[String],
    kind: InvocationKind,
) -> Result<(), Error> {
    let rustc = arguments.first().ok_or_else(|| {
        Error::new(
            "COMPILER_DISCOVERY_PROBE_UNAVAILABLE",
            "probe argv is empty",
        )
    })?;
    let expected = expected_probe_arguments(rustc, kind);
    if arguments != expected {
        return Err(Error::new(
            "COMPILER_DISCOVERY_PROBE_UNAVAILABLE",
            "probe argv differs from the exact Cargo 1.92 grammar",
        ));
    }
    Ok(())
}

fn expected_probe_arguments(rustc: &str, kind: InvocationKind) -> Vec<String> {
    if kind == InvocationKind::VersionProbe {
        return vec![rustc.to_owned(), "-vV".to_owned()];
    }
    let mut arguments = vec![
        rustc.to_owned(),
        "-".to_owned(),
        "--crate-name".to_owned(),
        "___".to_owned(),
        "--print=file-names".to_owned(),
    ];
    if kind == InvocationKind::TargetProbe {
        arguments.extend(["--target".to_owned(), TARGET_TRIPLE.to_owned()]);
    }
    arguments.extend([
        "--crate-type".to_owned(),
        "bin".to_owned(),
        "--crate-type".to_owned(),
        "rlib".to_owned(),
        "--crate-type".to_owned(),
        "dylib".to_owned(),
        "--crate-type".to_owned(),
        "cdylib".to_owned(),
        "--crate-type".to_owned(),
        "staticlib".to_owned(),
        "--crate-type".to_owned(),
        "proc-macro".to_owned(),
        "--print=sysroot".to_owned(),
        "--print=split-debuginfo".to_owned(),
        "--print=crate-name".to_owned(),
        "--print=cfg".to_owned(),
        "-Wwarnings".to_owned(),
    ]);
    arguments
}

fn parse_unit_arguments(
    arguments: &[String],
    config: &WrapperConfig,
) -> Result<UnitArguments, Error> {
    let crate_name = one_option(arguments, "--crate-name")?;
    let crate_types = collect_option_values(arguments, "--crate-type")?
        .into_iter()
        .flat_map(|value| {
            value.split(',').map(str::to_owned).collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>();
    if crate_types != BTreeSet::from(["lib".to_owned()]) {
        return Err(Error::new(
            "COMPILER_CRATE_TYPE_UNAVAILABLE",
            "Product compiler unit must have crate type lib",
        ));
    }
    let target = one_option(arguments, "--target")?;
    if target != TARGET_TRIPLE {
        return Err(Error::new(
            "COMPILER_TARGET_MISMATCH",
            "compiler unit target triple differs from its closure",
        ));
    }
    require_exact_option(arguments, "--edition", "2024")?;
    require_exact_option(arguments, "--error-format", "json")?;
    let cfgs = collect_option_values(arguments, "--cfg")?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let features = feature_cfgs(&cfgs)?;
    let out_dir = PathBuf::from(one_option(arguments, "--out-dir")?);
    validate_unit_directory(&out_dir, config)?;
    validate_exact_search_paths(arguments, config)?;
    let codegen = collect_option_values(arguments, "-C")?;
    let extra_filename = codegen_value(&codegen, "extra-filename")?;
    let metadata = codegen_value(&codegen, "metadata")?;
    require_codegen_value(&codegen, "opt-level", "3")?;
    require_codegen_value(&codegen, "embed-bitcode", "no")?;
    require_codegen_value(&codegen, "strip", "debuginfo")?;
    validate_codegen_hashes(&metadata, &extra_filename)?;
    reject_incremental(&codegen)?;
    let externs = parse_externs(arguments, config)?;
    let emit = emit_location(arguments)?;
    validate_emit(arguments, &emit)?;
    let source = unit_source(arguments, config)?;
    let unit = UnitArguments {
        crate_name,
        crate_types,
        target,
        cfgs,
        features,
        out_dir,
        extra_filename,
        source,
        externs,
        emit,
    };
    validate_unit_grammar(arguments, config, &unit, &metadata)?;
    Ok(unit)
}

fn validate_codegen_hashes(
    metadata: &str,
    extra_filename: &str,
) -> Result<(), Error> {
    let extra = extra_filename.strip_prefix('-').ok_or_else(|| {
        Error::new(
            "COMPILER_CODEGEN_OPTION",
            "extra-filename does not start with one hyphen",
        )
    })?;
    if !closed_hex(metadata) || !closed_hex(extra) {
        return Err(Error::new(
            "COMPILER_CODEGEN_OPTION",
            "metadata and extra-filename are not 16 lowercase hex digits",
        ));
    }
    Ok(())
}

fn closed_hex(value: &str) -> bool {
    value.len() == 16
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn validate_unit_grammar(
    arguments: &[String],
    config: &WrapperConfig,
    unit: &UnitArguments,
    metadata: &str,
) -> Result<(), Error> {
    validate_unit_identity_grammar(unit, config)?;
    let expected = expected_unit_arguments(arguments, config, unit, metadata)?;
    if arguments != expected {
        return Err(Error::new(
            "COMPILER_UNIT_ARGV_GRAMMAR",
            "compiler unit argv differs from the exact Cargo grammar",
        ));
    }
    Ok(())
}

fn validate_unit_identity_grammar(
    unit: &UnitArguments,
    config: &WrapperConfig,
) -> Result<(), Error> {
    let expected_source = match unit.crate_name.as_str() {
        "reovim_foundation" => "kernel/foundation/src/lib.rs",
        "reovim_foundation_public_consumer" => {
            "tests/foundation-public-consumer/src/lib.rs"
        }
        _ => {
            return Err(Error::new(
                "COMPILER_UNIT_IDENTITY",
                "compiler unit crate name is outside the closed pair",
            ));
        }
    };
    if unit.source != config.repository_root.join(expected_source) {
        return Err(Error::new(
            "COMPILER_UNIT_IDENTITY",
            "compiler unit source differs from its closed crate name",
        ));
    }
    let expected_out = config
        .target_dir
        .join("build-dir")
        .join(TARGET_TRIPLE)
        .join("release/deps");
    if unit.out_dir != expected_out {
        return Err(Error::new(
            "COMPILER_OUTPUT_AUTHORITY",
            "compiler output directory differs from the exact build dir",
        ));
    }
    validate_unit_extern_grammar(unit)
}

fn validate_unit_extern_grammar(unit: &UnitArguments) -> Result<(), Error> {
    if unit.crate_name == "reovim_foundation" && unit.externs.is_empty() {
        return Ok(());
    }
    let edges = unit.externs.iter().collect::<Vec<_>>();
    let [edge] = edges.as_slice() else {
        return Err(Error::new(
            "COMPILER_EXTERN_EDGE_SET",
            "consumer unit requires one exact Foundation extern",
        ));
    };
    let filename = edge.path.file_name().and_then(OsStr::to_str).unwrap_or("");
    let valid_filename = filename
        .strip_prefix("libreovim_foundation-")
        .and_then(|value| value.strip_suffix(".rmeta"))
        .is_some_and(closed_hex);
    if unit.crate_name != "reovim_foundation_public_consumer"
        || edge.name != "reovim_foundation"
        || !valid_filename
        || edge.path.parent() != Some(unit.out_dir.as_path())
    {
        return Err(Error::new(
            "COMPILER_EXTERN_EDGE_SET",
            "consumer extern is outside the closed Foundation metadata shape",
        ));
    }
    Ok(())
}

fn expected_unit_arguments(
    arguments: &[String],
    config: &WrapperConfig,
    unit: &UnitArguments,
    metadata: &str,
) -> Result<Vec<String>, Error> {
    let rustc = arguments.first().cloned().ok_or_else(|| {
        Error::new("COMPILER_UNIT_ARGV_GRAMMAR", "rustc argv is empty")
    })?;
    let mut expected = vec![
        rustc,
        "--crate-name".to_owned(),
        unit.crate_name.clone(),
        "--edition=2024".to_owned(),
        path_text(&unit.source)?.to_owned(),
        "--error-format=json".to_owned(),
        "--json=diagnostic-rendered-ansi,artifacts,future-incompat".to_owned(),
        "--crate-type".to_owned(),
        "lib".to_owned(),
        "--emit=dep-info,metadata,link".to_owned(),
        "-C".to_owned(),
        "opt-level=3".to_owned(),
        "-C".to_owned(),
        "embed-bitcode=no".to_owned(),
        "--deny=clippy::pedantic".to_owned(),
        "--deny=clippy::nursery".to_owned(),
        "--deny=clippy::all".to_owned(),
        "--deny=warnings".to_owned(),
        "--forbid=unsafe_code".to_owned(),
        "--forbid=clippy::excessive_nesting".to_owned(),
        "--cfg".to_owned(),
        "feature=\"default\"".to_owned(),
        "--check-cfg".to_owned(),
        "cfg(docsrs,test)".to_owned(),
        "--check-cfg".to_owned(),
        "cfg(feature, values(\"default\"))".to_owned(),
        "-C".to_owned(),
        format!("metadata={metadata}"),
        "-C".to_owned(),
        format!("extra-filename={}", unit.extra_filename),
        "--out-dir".to_owned(),
        path_text(&unit.out_dir)?.to_owned(),
        "--target".to_owned(),
        TARGET_TRIPLE.to_owned(),
        "-C".to_owned(),
        "strip=debuginfo".to_owned(),
    ];
    let build_dir = config.target_dir.join("build-dir");
    expected.extend([
        "-L".to_owned(),
        format!(
            "dependency={}",
            build_dir.join(TARGET_TRIPLE).join("release/deps").display()
        ),
        "-L".to_owned(),
        format!("dependency={}", build_dir.join("release/deps").display()),
    ]);
    if let Some(edge) = unit.externs.iter().next() {
        expected.extend([
            "--extern".to_owned(),
            format!("{}={}", edge.name, edge.path.display()),
        ]);
    }
    Ok(expected)
}

fn validate_exact_search_paths(
    arguments: &[String],
    config: &WrapperConfig,
) -> Result<(), Error> {
    let values = collect_option_values(arguments, "-L")?;
    let mut observed = BTreeSet::new();
    for value in values {
        validate_search_path(&value, &config.target_dir)?;
        let dependency = value.strip_prefix("dependency=").expect("validated");
        if !observed.insert(PathBuf::from(dependency)) {
            return Err(Error::new(
                "COMPILER_SEARCH_PATH_SET",
                "dependency search path is repeated",
            ));
        }
    }
    let build_dir = config.target_dir.join("build-dir");
    let expected = BTreeSet::from([
        build_dir.join(TARGET_TRIPLE).join("release/deps"),
        build_dir.join("release/deps"),
    ]);
    if observed != expected {
        return Err(Error::new(
            "COMPILER_SEARCH_PATH_SET",
            "dependency search paths differ from the exact Cargo set",
        ));
    }
    Ok(())
}

fn feature_cfgs(cfgs: &BTreeSet<String>) -> Result<BTreeSet<String>, Error> {
    let mut features = BTreeSet::new();
    for cfg in cfgs {
        let Some(value) = cfg
            .strip_prefix("feature=\"")
            .and_then(|value| value.strip_suffix('"'))
        else {
            return Err(Error::new(
                "COMPILER_CFG_UNAVAILABLE",
                format!("non-feature cfg {cfg} is unavailable"),
            ));
        };
        features.insert(value.to_owned());
    }
    if features != BTreeSet::from([EXPECTED_FEATURE.to_owned()]) {
        return Err(Error::new(
            "COMPILER_FEATURE_SET",
            "compiler feature cfgs are not exactly default",
        ));
    }
    Ok(features)
}

fn validate_unit_directory(
    out_dir: &Path,
    config: &WrapperConfig,
) -> Result<(), Error> {
    validate_absolute_path(
        out_dir,
        "rustc output directory",
        config.limits.path_bytes,
    )?;
    if !out_dir.starts_with(&config.target_dir) {
        return Err(Error::at(
            "COMPILER_OUTPUT_AUTHORITY",
            out_dir,
            "rustc output directory escapes the target authority",
        ));
    }
    Ok(())
}

fn codegen_value(options: &[String], expected: &str) -> Result<String, Error> {
    let values = options
        .iter()
        .filter_map(|option| option.split_once('='))
        .filter(|(name, _)| *name == expected)
        .map(|(_, value)| value.to_owned())
        .collect::<Vec<_>>();
    match values.as_slice() {
        [value] => Ok(value.clone()),
        _ => Err(Error::new(
            "COMPILER_CODEGEN_OPTION",
            format!("codegen option {expected} is absent or repeated"),
        )),
    }
}

fn require_codegen_value(
    options: &[String],
    name: &str,
    expected: &str,
) -> Result<(), Error> {
    let value = codegen_value(options, name)?;
    if value != expected {
        return Err(Error::new(
            "COMPILER_CODEGEN_OPTION",
            format!("codegen option {name} differs from {expected}"),
        ));
    }
    Ok(())
}

fn reject_incremental(options: &[String]) -> Result<(), Error> {
    if options.iter().any(|option| {
        option.split_once('=').is_some_and(|(name, _)| name == "incremental")
    }) {
        return Err(Error::new(
            "COMPILER_INCREMENTAL_UNAVAILABLE",
            "rustc incremental output is unavailable",
        ));
    }
    Ok(())
}

fn parse_externs(
    arguments: &[String],
    config: &WrapperConfig,
) -> Result<BTreeSet<ExternEdge>, Error> {
    let mut output = BTreeSet::new();
    for value in collect_option_values(arguments, "--extern")? {
        let Some((name, path)) = value.split_once('=') else {
            return Err(Error::new(
                "COMPILER_EXTERN_UNAVAILABLE",
                "extern without an exact provider path is unavailable",
            ));
        };
        validate_rust_name(name, "extern name")?;
        let path = PathBuf::from(path);
        validate_absolute_path(
            &path,
            "extern provider",
            config.limits.path_bytes,
        )?;
        if !path.starts_with(&config.target_dir) {
            return Err(Error::at(
                "COMPILER_EXTERN_UNAVAILABLE",
                &path,
                "extern provider escapes the target authority",
            ));
        }
        let edge = ExternEdge { name: name.to_owned(), path };
        if !output.insert(edge) {
            return Err(Error::new(
                "COMPILER_EXTERN_UNAVAILABLE",
                "extern edge is repeated",
            ));
        }
    }
    Ok(output)
}

fn validate_rust_name(value: &str, name: &str) -> Result<(), Error> {
    let mut bytes = value.bytes();
    let valid = bytes
        .next()
        .is_some_and(|byte| byte == b'_' || byte.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric());
    if !valid {
        return Err(Error::new(
            "COMPILER_RUST_NAME",
            format!("{name} is not a closed Rust identifier"),
        ));
    }
    Ok(())
}

fn option_value<'a>(
    arguments: &'a [String],
    index: usize,
    option: &str,
) -> Result<Option<(&'a str, usize)>, Error> {
    let argument = &arguments[index];
    if argument == option {
        let value = arguments.get(index + 1).ok_or_else(|| {
            Error::new(
                "COMPILER_WRAPPER_ARGV_FORM",
                format!("option {option} has no value"),
            )
        })?;
        return Ok(Some((value, index + 2)));
    }
    if let Some(value) = argument.strip_prefix(&format!("{option}=")) {
        return Ok(Some((value, index + 1)));
    }
    if !option.starts_with("--")
        && let Some(value) = short_flag_value(argument, option)
    {
        return Ok(Some((value, index + 1)));
    }
    Ok(None)
}

fn short_flag_value<'a>(argument: &'a str, option: &str) -> Option<&'a str> {
    argument.strip_prefix(option).filter(|value| !value.is_empty())
}

fn collect_option_values(
    arguments: &[String],
    option: &str,
) -> Result<Vec<String>, Error> {
    let mut output = Vec::new();
    let mut cursor = 1_usize;
    while cursor < arguments.len() {
        if let Some((value, next)) = option_value(arguments, cursor, option)? {
            output.push(value.to_owned());
            cursor = next;
        } else {
            cursor = cursor.saturating_add(1);
        }
    }
    Ok(output)
}

fn one_option(arguments: &[String], option: &str) -> Result<String, Error> {
    let values = collect_option_values(arguments, option)?;
    match values.as_slice() {
        [value] if !value.is_empty() => Ok(value.clone()),
        _ => Err(Error::new(
            "COMPILER_WRAPPER_ARGV_FORM",
            format!("option {option} is absent, empty, or repeated"),
        )),
    }
}

fn require_exact_option(
    arguments: &[String],
    option: &str,
    expected: &str,
) -> Result<(), Error> {
    let value = one_option(arguments, option)?;
    if value != expected {
        return Err(Error::new(
            "COMPILER_WRAPPER_ARGV_FORM",
            format!("option {option} differs from {expected}"),
        ));
    }
    Ok(())
}

fn emit_location(arguments: &[String]) -> Result<EmitLocation, Error> {
    let mut locations = Vec::new();
    for (index, argument) in arguments.iter().enumerate().skip(1) {
        if let Some(location) = emit_candidate(arguments, index, argument)? {
            locations.push(location);
        }
    }
    if locations.len() != 1 {
        return Err(Error::new(
            "COMPILER_EMIT_SET",
            "compiler unit requires exactly one --emit specification",
        ));
    }
    Ok(locations.remove(0))
}

fn emit_candidate(
    arguments: &[String],
    index: usize,
    argument: &str,
) -> Result<Option<EmitLocation>, Error> {
    if argument == "--emit" {
        if arguments.get(index + 1).is_none() {
            return Err(Error::new("COMPILER_EMIT_SET", "--emit has no value"));
        }
        return Ok(Some(EmitLocation {
            flag_index: index,
            value_index: Some(index + 1),
        }));
    }
    Ok(argument
        .starts_with("--emit=")
        .then_some(EmitLocation { flag_index: index, value_index: None }))
}

fn emit_value<'a>(arguments: &'a [String], location: &EmitLocation) -> &'a str {
    if let Some(index) = location.value_index {
        return &arguments[index];
    }
    arguments[location.flag_index]
        .strip_prefix("--emit=")
        .expect("location constructed from --emit=")
}

fn validate_emit(
    arguments: &[String],
    location: &EmitLocation,
) -> Result<(), Error> {
    let parts =
        emit_value(arguments, location).split(',').collect::<BTreeSet<_>>();
    if parts != BTreeSet::from(["dep-info", "link", "metadata"])
        || parts.iter().any(|part| part.contains('='))
    {
        return Err(Error::new(
            "COMPILER_EMIT_SET",
            "emit set must be exactly dep-info, metadata, and link",
        ));
    }
    Ok(())
}

fn rewrite_emit(
    arguments: &[String],
    location: &EmitLocation,
    dep_info: &Path,
) -> Result<Vec<String>, Error> {
    let dep_info = dep_info.to_str().ok_or_else(|| {
        Error::at(
            "COMPILER_DEP_INFO_PATH",
            dep_info,
            "private dep-info path is not UTF-8",
        )
    })?;
    let value = format!("dep-info={dep_info},metadata,link");
    let mut effective = arguments.to_vec();
    if let Some(index) = location.value_index {
        effective[index] = value;
    } else {
        effective[location.flag_index] = format!("--emit={value}");
    }
    Ok(effective)
}

fn inject_sysroot(
    arguments: &[String],
    sysroot: &Path,
) -> Result<Vec<String>, Error> {
    if !collect_option_values(arguments, "--sysroot")?.is_empty() {
        return Err(Error::new(
            "COMPILER_SYSROOT_FLAG_UNAVAILABLE",
            "rustc argv already contains a sysroot selection",
        ));
    }
    let sysroot = path_text(sysroot)?.to_owned();
    let mut effective = arguments.to_vec();
    effective.push("--sysroot".to_owned());
    effective.push(sysroot);
    Ok(effective)
}

fn unit_source(
    arguments: &[String],
    config: &WrapperConfig,
) -> Result<PathBuf, Error> {
    let candidates = arguments
        .iter()
        .skip(1)
        .filter(|argument| {
            Path::new(argument)
                .extension()
                .is_some_and(|value| value.eq_ignore_ascii_case("rs"))
        })
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let [source] = candidates.as_slice() else {
        return Err(Error::new(
            "COMPILER_SOURCE_ROOT",
            "compiler unit does not have exactly one Rust source root",
        ));
    };
    validate_absolute_path(
        source,
        "compiler source root",
        config.limits.path_bytes,
    )?;
    if !source.starts_with(&config.repository_root) {
        return Err(Error::at(
            "COMPILER_SOURCE_ROOT",
            source,
            "compiler source root escapes the repository authority",
        ));
    }
    Ok(source.clone())
}

fn execute_rustc(
    config: &WrapperConfig,
    plan: &InvocationPlan,
) -> Result<std::process::ExitStatus, Error> {
    let mut command = Command::new(&config.rustc);
    command
        .args(plan.effective.iter().skip(1))
        .env_clear()
        .envs(plan.environment.iter().map(|entry| (&entry.name, &entry.value)))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command.status().map_err(|failure| {
        Error::at(
            "COMPILER_RUSTC_EXECUTION",
            &config.rustc,
            failure.to_string(),
        )
    })
}

fn reconcile_dep_info(
    config: &WrapperConfig,
    plan: &InvocationPlan,
) -> Result<(), Error> {
    let (Some(private), Some(cargo)) =
        (plan.dep_info_path.as_deref(), plan.cargo_dep_info_path.as_deref())
    else {
        return Ok(());
    };
    let bytes = read_regular_bounded(
        private,
        config.limits.dep_info_bytes,
        "COMPILER_DEP_INFO_READ",
    )?;
    if cargo.exists() {
        return Err(Error::at(
            "COMPILER_DEP_INFO_COLLISION",
            cargo,
            "Cargo dep-info destination already exists",
        ));
    }
    atomic_write(
        cargo,
        &bytes,
        config.limits.dep_info_bytes,
        "COMPILER_DEP_INFO_WRITE",
    )
}

fn publish_start(
    config: &WrapperConfig,
    ordinal: u64,
    plan: &InvocationPlan,
) -> Result<(), Error> {
    let environment = plan
        .environment
        .iter()
        .map(|entry| json!({"name": entry.name, "value": entry.value}))
        .collect::<Vec<_>>();
    let value = json!({
        "schema": TRACE_SCHEMA,
        "phase": "start",
        "ordinal": ordinal,
        "kind": plan.kind.as_str(),
        "rustc": config.rustc,
        "original_argv": plan.original,
        "effective_argv": plan.effective,
        "environment": environment,
        "dep_info_path": plan.dep_info_path,
    });
    atomic_json(
        &start_path(&config.trace_dir, ordinal),
        &value,
        config.limits.trace_bytes,
    )
}

fn publish_finish(
    config: &WrapperConfig,
    ordinal: u64,
    rustc_status: i32,
    wrapper_status: i32,
) -> Result<(), Error> {
    let value = json!({
        "schema": TRACE_SCHEMA,
        "phase": "finish",
        "ordinal": ordinal,
        "rustc_status": rustc_status,
        "wrapper_status": wrapper_status,
    });
    atomic_json(
        &finish_path(&config.trace_dir, ordinal),
        &value,
        config.limits.trace_bytes,
    )
}

fn atomic_json(path: &Path, value: &Value, limit: usize) -> Result<(), Error> {
    let mut bytes = serde_json::to_vec(value).map_err(|failure| {
        Error::at("COMPILER_WRAPPER_TRACE_JSON", path, failure.to_string())
    })?;
    bytes.push(b'\n');
    atomic_write(path, &bytes, limit, "COMPILER_WRAPPER_TRACE_IO")
}

fn atomic_write(
    path: &Path,
    bytes: &[u8],
    limit: usize,
    code: &'static str,
) -> Result<(), Error> {
    if path.exists() {
        return Err(Error::at(code, path, "output path already exists"));
    }
    if bytes.len() > limit {
        return Err(Error::at(
            code,
            path,
            format!("output exceeds {limit} bytes"),
        ));
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| Error::at(code, path, "output path has no filename"))?;
    let temporary = path.with_file_name(format!(
        ".{}.tmp-{}",
        file_name.to_string_lossy(),
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|failure| Error::at(code, &temporary, failure.to_string()))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|failure| Error::at(code, &temporary, failure.to_string()))?;
    fs::rename(&temporary, path)
        .map_err(|failure| Error::at(code, path, failure.to_string()))?;
    let directory = OpenOptions::new()
        .read(true)
        .open(path.parent().expect("validated output parent"))
        .map_err(|failure| Error::at(code, path, failure.to_string()))?;
    directory
        .sync_all()
        .map_err(|failure| Error::at(code, path, failure.to_string()))
}

fn read_regular_bounded(
    path: &Path,
    limit: usize,
    code: &'static str,
) -> Result<Vec<u8>, Error> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|failure| Error::at(code, path, failure.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::at(code, path, "input is not a regular file"));
    }
    let limit = u64::try_from(limit)
        .map_err(|failure| Error::at(code, path, failure.to_string()))?;
    crate::source::read_bounded(path, limit)
        .map_err(|failure| Error::at(code, path, failure))
}

fn claim_path(trace_dir: &Path, ordinal: u64) -> PathBuf {
    trace_dir.join(format!("{ordinal:08}.claim"))
}

fn start_path(trace_dir: &Path, ordinal: u64) -> PathBuf {
    trace_dir.join(format!("{ordinal:08}.start.json"))
}

fn finish_path(trace_dir: &Path, ordinal: u64) -> PathBuf {
    trace_dir.join(format!("{ordinal:08}.finish.json"))
}

fn trace_io(path: &Path, failure: &std::io::Error) -> Error {
    Error::at("COMPILER_WRAPPER_TRACE_IO", path, failure.to_string())
}

struct TraceFiles {
    starts: BTreeMap<u64, (PathBuf, Vec<u8>)>,
    finishes: BTreeMap<u64, (PathBuf, Vec<u8>)>,
    total_bytes: usize,
}

fn parse_traces(
    request: &BuildRequest<'_>,
    limits: Limits,
) -> Result<Vec<WrapperTrace>, Error> {
    let files = read_trace_files(request.trace_dir, limits)?;
    if files.starts.len() != files.finishes.len() || files.starts.is_empty() {
        return Err(Error::new(
            "COMPILER_TRACE_PAIR",
            "wrapper start and finish trace sets are incomplete",
        ));
    }
    let mut output = Vec::with_capacity(files.starts.len());
    for expected in 0..files.starts.len() {
        let ordinal = u64::try_from(expected).map_err(|failure| {
            Error::new("COMPILER_TRACE_ORDINAL", failure.to_string())
        })?;
        let start = files.starts.get(&ordinal).ok_or_else(|| {
            Error::new(
                "COMPILER_TRACE_ORDINAL",
                format!("wrapper start ordinal {ordinal} is absent"),
            )
        })?;
        let finish = files.finishes.get(&ordinal).ok_or_else(|| {
            Error::new(
                "COMPILER_TRACE_ORDINAL",
                format!("wrapper finish ordinal {ordinal} is absent"),
            )
        })?;
        let trace = parse_trace_pair(ordinal, start, finish, limits)?;
        validate_recorded_trace(&trace, request, limits)?;
        output.push(trace);
    }
    Ok(output)
}

fn read_trace_files(
    trace_dir: &Path,
    limits: Limits,
) -> Result<TraceFiles, Error> {
    let mut paths = fs::read_dir(trace_dir)
        .map_err(|failure| trace_io(trace_dir, &failure))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|failure| trace_io(trace_dir, &failure))?;
    paths.sort();
    let maximum_files =
        limits.wrapper_trace_count.checked_mul(2).ok_or_else(|| {
            Error::new("COMPILER_TRACE_LIMIT", "trace file count overflow")
        })?;
    if paths.len() > maximum_files {
        return Err(Error::new(
            "COMPILER_TRACE_LIMIT",
            "wrapper trace file count exceeds its bound",
        ));
    }
    let mut files = TraceFiles {
        starts: BTreeMap::new(),
        finishes: BTreeMap::new(),
        total_bytes: 0,
    };
    for path in paths {
        read_one_trace_file(&path, limits, &mut files)?;
    }
    Ok(files)
}

fn read_one_trace_file(
    path: &Path,
    limits: Limits,
    files: &mut TraceFiles,
) -> Result<(), Error> {
    let name = path.file_name().and_then(OsStr::to_str).ok_or_else(|| {
        Error::at("COMPILER_TRACE_NAME", path, "trace name is not UTF-8")
    })?;
    let (ordinal, phase) = parse_trace_name(name)?;
    let bytes = read_regular_bounded(
        path,
        limits.wrapper_trace_bytes,
        "COMPILER_TRACE_READ",
    )?;
    files.total_bytes =
        files.total_bytes.checked_add(bytes.len()).ok_or_else(|| {
            Error::new("COMPILER_TRACE_LIMIT", "trace byte count overflow")
        })?;
    if files.total_bytes > limits.wrapper_trace_bytes {
        return Err(Error::new(
            "COMPILER_TRACE_LIMIT",
            "aggregate wrapper trace bytes exceed the limit",
        ));
    }
    let map = match phase {
        TracePhase::Start => &mut files.starts,
        TracePhase::Finish => &mut files.finishes,
    };
    if map.insert(ordinal, (path.to_path_buf(), bytes)).is_some() {
        return Err(Error::new(
            "COMPILER_TRACE_DUPLICATE",
            "wrapper trace ordinal and phase are repeated",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum TracePhase {
    Start,
    Finish,
}

fn parse_trace_name(name: &str) -> Result<(u64, TracePhase), Error> {
    let (ordinal, phase) = if let Some(value) = name.strip_suffix(".start.json")
    {
        (value, TracePhase::Start)
    } else if let Some(value) = name.strip_suffix(".finish.json") {
        (value, TracePhase::Finish)
    } else {
        return Err(Error::new(
            "COMPILER_TRACE_NAME",
            format!("unknown wrapper trace file {name}"),
        ));
    };
    if ordinal.len() != 8 || !ordinal.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Error::new(
            "COMPILER_TRACE_NAME",
            format!("invalid wrapper trace ordinal {ordinal}"),
        ));
    }
    let ordinal = ordinal.parse::<u64>().map_err(|failure| {
        Error::new("COMPILER_TRACE_NAME", failure.to_string())
    })?;
    Ok((ordinal, phase))
}

fn parse_trace_pair(
    ordinal: u64,
    start: &(PathBuf, Vec<u8>),
    finish: &(PathBuf, Vec<u8>),
    limits: Limits,
) -> Result<WrapperTrace, Error> {
    let start_value = parse_trace_json(&start.0, &start.1, limits)?;
    let finish_value = parse_trace_json(&finish.0, &finish.1, limits)?;
    let start_object = closed_object(
        &start_value,
        &[
            "schema",
            "phase",
            "ordinal",
            "kind",
            "rustc",
            "original_argv",
            "effective_argv",
            "environment",
            "dep_info_path",
        ],
        "COMPILER_TRACE_SHAPE",
    )?;
    require_json_text(start_object, "schema", TRACE_SCHEMA)?;
    require_json_text(start_object, "phase", "start")?;
    require_json_ordinal(start_object, ordinal)?;
    let kind = json_text(start_object, "kind").and_then(|value| {
        InvocationKind::parse(value).ok_or_else(|| {
            Error::new(
                "COMPILER_TRACE_KIND",
                format!("unknown trace kind {value}"),
            )
        })
    })?;
    let rustc = PathBuf::from(json_text(start_object, "rustc")?);
    let original_argv = json_string_array(start_object, "original_argv")?;
    let effective_argv = json_string_array(start_object, "effective_argv")?;
    let environment = json_environment(start_object)?;
    let dep_info_path = json_nullable_path(start_object, "dep_info_path")?;
    let finish_object = closed_object(
        &finish_value,
        &["schema", "phase", "ordinal", "rustc_status", "wrapper_status"],
        "COMPILER_TRACE_SHAPE",
    )?;
    require_json_text(finish_object, "schema", TRACE_SCHEMA)?;
    require_json_text(finish_object, "phase", "finish")?;
    require_json_ordinal(finish_object, ordinal)?;
    let rustc_status = json_status(finish_object, "rustc_status")?;
    let wrapper_status = json_status(finish_object, "wrapper_status")?;
    if rustc_status != 0 || wrapper_status != 0 {
        return Err(Error::new(
            "COMPILER_TRACE_STATUS",
            "wrapper trace records a nonzero rustc or wrapper status",
        ));
    }
    let trace_bytes =
        start.1.len().checked_add(finish.1.len()).ok_or_else(|| {
            Error::new("COMPILER_TRACE_LIMIT", "trace byte count overflow")
        })?;
    let trace_digest = paired_trace_digest(&start.1, &finish.1)?;
    Ok(WrapperTrace {
        ordinal,
        kind,
        rustc,
        original_argv,
        effective_argv,
        environment,
        dep_info_path,
        status: wrapper_status,
        trace_digest,
        trace_bytes: u64::try_from(trace_bytes).map_err(|failure| {
            Error::new("COMPILER_TRACE_LIMIT", failure.to_string())
        })?,
    })
}

fn parse_trace_json(
    path: &Path,
    bytes: &[u8],
    limits: Limits,
) -> Result<Value, Error> {
    if !bytes.ends_with(b"\n") || bytes[..bytes.len() - 1].contains(&b'\n') {
        return Err(Error::at(
            "COMPILER_TRACE_FRAMING",
            path,
            "trace must contain one JSON object and final LF",
        ));
    }
    bounded_json::parse(bytes, limits.json_depth, limits.json_items).map_err(
        |failure| Error::at("COMPILER_TRACE_JSON", path, failure.to_string()),
    )
}

fn closed_object<'a>(
    value: &'a Value,
    fields: &[&str],
    code: &'static str,
) -> Result<&'a Map<String, Value>, Error> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::new(code, "expected one JSON object"))?;
    if object.len() != fields.len()
        || object.keys().any(|name| !fields.contains(&name.as_str()))
        || fields.iter().any(|name| !object.contains_key(*name))
    {
        return Err(Error::new(code, "JSON object field set is not exact"));
    }
    Ok(object)
}

fn json_text<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, Error> {
    object.get(name).and_then(Value::as_str).ok_or_else(|| {
        Error::new(
            "COMPILER_TRACE_SHAPE",
            format!("trace field {name} is not a string"),
        )
    })
}

fn require_json_text(
    object: &Map<String, Value>,
    name: &str,
    expected: &str,
) -> Result<(), Error> {
    if json_text(object, name)? != expected {
        return Err(Error::new(
            "COMPILER_TRACE_SHAPE",
            format!("trace field {name} differs from {expected}"),
        ));
    }
    Ok(())
}

fn require_json_ordinal(
    object: &Map<String, Value>,
    expected: u64,
) -> Result<(), Error> {
    if object.get("ordinal").and_then(Value::as_u64) != Some(expected) {
        return Err(Error::new(
            "COMPILER_TRACE_ORDINAL",
            "trace ordinal disagrees with its filename",
        ));
    }
    Ok(())
}

fn json_status(object: &Map<String, Value>, name: &str) -> Result<i32, Error> {
    let value = object.get(name).and_then(Value::as_i64).ok_or_else(|| {
        Error::new(
            "COMPILER_TRACE_SHAPE",
            format!("trace field {name} is not an integer"),
        )
    })?;
    i32::try_from(value).map_err(|failure| {
        Error::new("COMPILER_TRACE_STATUS", failure.to_string())
    })
}

fn json_string_array(
    object: &Map<String, Value>,
    name: &str,
) -> Result<Vec<String>, Error> {
    let values =
        object.get(name).and_then(Value::as_array).ok_or_else(|| {
            Error::new(
                "COMPILER_TRACE_SHAPE",
                format!("trace field {name} is not an array"),
            )
        })?;
    values
        .iter()
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                Error::new(
                    "COMPILER_TRACE_SHAPE",
                    format!("trace array {name} has a non-string"),
                )
            })
        })
        .collect()
}

fn json_environment(
    object: &Map<String, Value>,
) -> Result<Vec<EnvironmentEntry>, Error> {
    let values = object
        .get("environment")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            Error::new(
                "COMPILER_TRACE_SHAPE",
                "trace environment is not an array",
            )
        })?;
    let mut output = BTreeSet::new();
    for value in values {
        let entry =
            closed_object(value, &["name", "value"], "COMPILER_TRACE_SHAPE")?;
        let item = EnvironmentEntry {
            name: json_text(entry, "name")?.to_owned(),
            value: json_text(entry, "value")?.to_owned(),
        };
        if !output.insert(item) {
            return Err(Error::new(
                "COMPILER_TRACE_ENVIRONMENT",
                "trace environment repeats one entry",
            ));
        }
    }
    Ok(output.into_iter().collect())
}

fn json_nullable_path(
    object: &Map<String, Value>,
    name: &str,
) -> Result<Option<PathBuf>, Error> {
    let value = object.get(name).ok_or_else(|| {
        Error::new(
            "COMPILER_TRACE_SHAPE",
            format!("trace field {name} is absent"),
        )
    })?;
    if value.is_null() {
        return Ok(None);
    }
    value.as_str().map(PathBuf::from).map(Some).ok_or_else(|| {
        Error::new(
            "COMPILER_TRACE_SHAPE",
            format!("trace field {name} is not null or a string"),
        )
    })
}

fn paired_trace_digest(start: &[u8], finish: &[u8]) -> Result<String, Error> {
    let mut input =
        Vec::with_capacity(start.len().checked_add(finish.len()).ok_or_else(
            || Error::new("COMPILER_TRACE_LIMIT", "trace digest size overflow"),
        )?);
    input.extend_from_slice(start);
    input.extend_from_slice(finish);
    Ok(digest::bytes(&input))
}

fn validate_recorded_trace(
    trace: &WrapperTrace,
    request: &BuildRequest<'_>,
    limits: Limits,
) -> Result<(), Error> {
    if trace.rustc != request.rustc {
        return Err(Error::new(
            "COMPILER_TRACE_RUSTC",
            "trace rustc differs from the selected compiler",
        ));
    }
    validate_recorded_argv(trace, limits)?;
    let config = recorded_wrapper_config(request, limits);
    validate_recorded_environment(&trace.environment, &config)?;
    reject_dangerous_arguments(&trace.original_argv, request.target_dir)?;
    let observed_kind = classify_invocation(&trace.original_argv)?;
    if observed_kind != trace.kind {
        return Err(Error::new(
            "COMPILER_TRACE_KIND",
            "recorded trace kind disagrees with its argv",
        ));
    }
    if trace.kind != InvocationKind::CompilerUnit {
        return validate_recorded_probe(trace, &config);
    }
    validate_recorded_unit(trace, request, &config)
}

fn recorded_wrapper_config(
    request: &BuildRequest<'_>,
    limits: Limits,
) -> WrapperConfig {
    WrapperConfig {
        rustc: request.rustc.to_path_buf(),
        sysroot: request.sysroot.to_path_buf(),
        cargo_home: request.cargo_home.to_path_buf(),
        temp_dir: request.temp_dir.to_path_buf(),
        build_dir: request.target_dir.join("build-dir"),
        repository_root: request.repository_root.to_path_buf(),
        target_dir: request.target_dir.to_path_buf(),
        trace_dir: request.trace_dir.to_path_buf(),
        dep_info_dir: request.dep_info_dir.to_path_buf(),
        limits: WrapperLimits {
            path_bytes: limits.path_bytes,
            trace_bytes: limits.wrapper_trace_bytes,
            argv_items: limits.wrapper_argv_items,
            argv_bytes: limits.wrapper_argv_bytes,
            trace_count: limits.wrapper_trace_count,
            dep_info_bytes: limits.dep_info_file_bytes,
        },
    }
}

/// Revalidates one report-retained compiler unit against the exact wrapper
/// grammar without trusting the earlier workload parser.
pub fn validate_reported_unit_argv(
    original: &[String],
    effective: &[String],
    dep_info_path: &Path,
    crate_name: &str,
    extern_path: Option<&Path>,
) -> Result<(), Error> {
    let (dep_info_dir, ordinal) = match crate_name {
        "reovim_foundation" => ("/scratch/shipping-dep-info", 4_u64),
        "reovim_foundation_public_consumer" => {
            ("/scratch/validation-dep-info", 0_u64)
        }
        _ => {
            return Err(Error::new(
                "COMPILER_UNIT_IDENTITY",
                "reported compiler unit is outside the closed pair",
            ));
        }
    };
    let expected_dep_info =
        Path::new(dep_info_dir).join(format!("{ordinal:08}.d"));
    if dep_info_path != expected_dep_info {
        return Err(Error::at(
            "COMPILER_DEP_INFO_PATH",
            dep_info_path,
            "reported dep-info path differs from the exact unit ordinal",
        ));
    }
    let config = report_wrapper_config(Path::new(dep_info_dir));
    require_wrapper_rustc(original, &config)?;
    reject_dangerous_arguments(original, &config.target_dir)?;
    let unit = parse_unit_arguments(original, &config)?;
    if unit.crate_name != crate_name {
        return Err(Error::new(
            "COMPILER_UNIT_IDENTITY",
            "reported compiler argv names the wrong closed crate",
        ));
    }
    validate_reported_extern(&unit, extern_path)?;
    let rewritten = rewrite_emit(original, &unit.emit, dep_info_path)?;
    let expected = inject_sysroot(&rewritten, &config.sysroot)?;
    if effective == expected {
        return Ok(());
    }
    Err(Error::new(
        "COMPILER_EFFECTIVE_ARGV",
        "reported effective argv differs from the exact wrapper rewrite",
    ))
}

fn report_wrapper_config(dep_info_dir: &Path) -> WrapperConfig {
    let target_dir = PathBuf::from("/scratch/build-target");
    WrapperConfig {
        rustc: PathBuf::from("/compiler/bin/rustc"),
        sysroot: PathBuf::from("/sysroot"),
        cargo_home: PathBuf::from("/scratch/cargo-home"),
        temp_dir: PathBuf::from("/scratch/temp"),
        build_dir: target_dir.join("build-dir"),
        repository_root: PathBuf::from("/repo"),
        target_dir,
        trace_dir: PathBuf::from("/scratch/report-trace"),
        dep_info_dir: dep_info_dir.to_path_buf(),
        limits: WrapperLimits {
            path_bytes: usize::MAX,
            trace_bytes: usize::MAX,
            argv_items: usize::MAX,
            argv_bytes: usize::MAX,
            trace_count: usize::MAX,
            dep_info_bytes: usize::MAX,
        },
    }
}

fn validate_reported_extern(
    unit: &UnitArguments,
    expected_path: Option<&Path>,
) -> Result<(), Error> {
    let values = unit.externs.iter().collect::<Vec<_>>();
    match (values.as_slice(), expected_path) {
        ([], None) => Ok(()),
        ([edge], Some(path))
            if edge.name == "reovim_foundation" && edge.path == path =>
        {
            Ok(())
        }
        _ => Err(Error::new(
            "COMPILER_EXTERN_EDGE_SET",
            "reported compiler argv differs from its retained extern edge",
        )),
    }
}

fn validate_recorded_argv(
    trace: &WrapperTrace,
    limits: Limits,
) -> Result<(), Error> {
    if trace.original_argv.is_empty()
        || trace.original_argv.len() > limits.wrapper_argv_items
        || trace.effective_argv.is_empty()
        || trace.effective_argv.len() > limits.wrapper_argv_items
    {
        return Err(Error::new(
            "COMPILER_TRACE_ARGV_LIMIT",
            "recorded argv count is empty or exceeds its bound",
        ));
    }
    let mut bytes = 0_usize;
    for value in trace.original_argv.iter().chain(&trace.effective_argv) {
        bytes = charge_text(bytes, value, limits.wrapper_argv_bytes)?;
    }
    Ok(())
}

fn validate_recorded_environment(
    environment: &[EnvironmentEntry],
    config: &WrapperConfig,
) -> Result<(), Error> {
    let normalized = environment.iter().cloned().collect::<BTreeSet<_>>();
    if normalized.len() != environment.len()
        || !normalized.iter().eq(environment.iter())
    {
        return Err(Error::new(
            "COMPILER_TRACE_ENVIRONMENT",
            "recorded environment is not sorted and unique",
        ));
    }
    let mut bytes = 0_usize;
    for entry in environment {
        if wrapper_control_name(&entry.name)
            || reject_unavailable_environment(&entry.name).is_err()
            || !allowed_wrapper_environment(&entry.name)
        {
            return Err(Error::new(
                "COMPILER_TRACE_ENVIRONMENT",
                format!(
                    "recorded environment name {} is unavailable",
                    entry.name
                ),
            ));
        }
        bytes = charge_text(bytes, &entry.name, config.limits.argv_bytes)?;
        bytes = charge_text(bytes, &entry.value, config.limits.argv_bytes)?;
    }
    validate_required_child_environment(environment, config)
}

fn validate_recorded_probe(
    trace: &WrapperTrace,
    config: &WrapperConfig,
) -> Result<(), Error> {
    validate_probe(&trace.original_argv, trace.kind)?;
    let effective = inject_sysroot(&trace.original_argv, &config.sysroot)?;
    if trace.effective_argv != effective || trace.dep_info_path.is_some() {
        return Err(Error::new(
            "COMPILER_TRACE_PROBE",
            "probe effective argv differs from exact sysroot injection",
        ));
    }
    Ok(())
}

fn validate_recorded_unit(
    trace: &WrapperTrace,
    request: &BuildRequest<'_>,
    config: &WrapperConfig,
) -> Result<(), Error> {
    let unit = parse_unit_arguments(&trace.original_argv, config)?;
    let expected_source =
        request.repository_root.join(&request.closure.target.src_path);
    if unit.crate_name != request.closure.target.name
        || unit.source != expected_source
    {
        return Err(Error::new(
            "COMPILER_UNIT_IDENTITY",
            "compiler unit crate name or source root differs from its closure",
        ));
    }
    let expected_dep_info =
        request.dep_info_dir.join(format!("{:08}.d", trace.ordinal));
    if trace.dep_info_path.as_ref() != Some(&expected_dep_info) {
        return Err(Error::new(
            "COMPILER_DEP_INFO_PATH",
            "trace dep-info path differs from its unique ordinal path",
        ));
    }
    let rewritten =
        rewrite_emit(&trace.original_argv, &unit.emit, &expected_dep_info)?;
    let effective = inject_sysroot(&rewritten, &config.sysroot)?;
    if trace.effective_argv != effective {
        return Err(Error::new(
            "COMPILER_EFFECTIVE_ARGV",
            "effective argv changes more than the dep-info selection",
        ));
    }
    validate_compiler_environment(trace, request)
}

fn validate_compiler_environment(
    trace: &WrapperTrace,
    request: &BuildRequest<'_>,
) -> Result<(), Error> {
    let environment = trace
        .environment
        .iter()
        .map(|entry| (entry.name.as_str(), entry.value.as_str()))
        .collect::<BTreeMap<_, _>>();
    let package_manifest =
        request.repository_root.join(&request.closure.package.manifest);
    let package_root = package_manifest
        .parent()
        .expect("registered package manifest has a parent");
    let expected = [
        ("CARGO_CRATE_NAME", request.closure.target.name.as_str()),
        ("CARGO_MANIFEST_DIR", path_text(package_root)?),
        ("CARGO_MANIFEST_PATH", path_text(&package_manifest)?),
        ("CARGO_PKG_NAME", request.closure.package.name.as_str()),
        ("CARGO_PRIMARY_PACKAGE", "1"),
    ];
    if expected
        .iter()
        .any(|(name, value)| environment.get(name).copied() != Some(*value))
    {
        return Err(Error::new(
            "COMPILER_UNIT_ENVIRONMENT",
            "Cargo compiler-unit environment differs from its closure",
        ));
    }
    Ok(())
}

fn path_text(path: &Path) -> Result<&str, Error> {
    path.to_str().ok_or_else(|| {
        Error::at("COMPILER_CAPTURE_PATH_UTF8", path, "path is not UTF-8")
    })
}

/// Parses one complete bounded Cargo JSON message stream.
///
/// # Errors
///
/// Returns a typed failure for malformed framing, unknown messages, build
/// scripts, unsuccessful completion, profile drift, duplicate filenames, or
/// exhausted bounds.
pub fn parse_cargo_output(
    command_argv: Vec<String>,
    output: &[u8],
    limits: Limits,
) -> Result<CargoObservation, Error> {
    validate_limits(limits)?;
    if output.is_empty() || output.len() > limits.cargo_json_bytes {
        return Err(Error::new(
            "COMPILER_CARGO_JSON_LIMIT",
            "Cargo JSON stream is empty or exceeds its byte bound",
        ));
    }
    if !output.ends_with(b"\n")
        || output.contains(&b'\r')
        || output.contains(&0)
    {
        return Err(Error::new(
            "COMPILER_CARGO_JSON_FRAMING",
            "Cargo JSON stream requires LF framing without CR or NUL",
        ));
    }
    let mut artifacts = Vec::new();
    let mut finished = false;
    let mut messages = 0_usize;
    for line in output.split_inclusive(|byte| *byte == b'\n') {
        messages = messages.checked_add(1).ok_or_else(|| {
            Error::new("COMPILER_CARGO_JSON_LIMIT", "message count overflow")
        })?;
        if messages > limits.cargo_message_count {
            return Err(Error::new(
                "COMPILER_CARGO_JSON_LIMIT",
                "Cargo JSON message count exceeds its bound",
            ));
        }
        let line = &line[..line.len() - 1];
        if line.is_empty() || finished {
            return Err(Error::new(
                "COMPILER_CARGO_JSON_FRAMING",
                "Cargo JSON contains an empty or post-finish message",
            ));
        }
        let value =
            bounded_json::parse(line, limits.json_depth, limits.json_items)
                .map_err(|failure| {
                    Error::new("COMPILER_CARGO_JSON", failure.to_string())
                })?;
        match cargo_reason(&value)? {
            "compiler-artifact" => {
                artifacts.push(parse_cargo_artifact(&value)?);
            }
            "build-finished" => {
                parse_build_finished(&value)?;
                finished = true;
            }
            "build-script-executed" => {
                return Err(Error::new(
                    "PRODUCT_BUILD_SCRIPT_PROOF_UNAVAILABLE",
                    "Cargo emitted a build-script event",
                ));
            }
            "compiler-message" => {
                return Err(Error::new(
                    "COMPILER_DIAGNOSTIC_UNAVAILABLE",
                    "successful Product capture cannot contain diagnostics",
                ));
            }
            other => {
                return Err(Error::new(
                    "COMPILER_CARGO_MESSAGE_UNAVAILABLE",
                    format!("unknown Cargo message reason {other}"),
                ));
            }
        }
    }
    if !finished
        || artifacts.is_empty()
        || artifacts.len() > limits.artifact_count
    {
        return Err(Error::new(
            "COMPILER_CARGO_COMPLETION",
            "Cargo stream lacks exact artifacts or successful completion",
        ));
    }
    Ok(CargoObservation {
        command_argv,
        raw_digest: digest::bytes(output),
        stdout_bytes: u64::try_from(output.len()).map_err(|failure| {
            Error::new("COMPILER_CARGO_JSON_LIMIT", failure.to_string())
        })?,
        message_count: messages,
        artifacts,
    })
}

fn cargo_reason(value: &Value) -> Result<&str, Error> {
    value
        .as_object()
        .and_then(|object| object.get("reason"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            Error::new(
                "COMPILER_CARGO_MESSAGE_SHAPE",
                "Cargo message reason is absent or invalid",
            )
        })
}

fn parse_cargo_artifact(value: &Value) -> Result<CargoArtifact, Error> {
    let object = closed_object(
        value,
        &[
            "reason",
            "package_id",
            "manifest_path",
            "target",
            "profile",
            "features",
            "filenames",
            "executable",
            "fresh",
        ],
        "COMPILER_CARGO_ARTIFACT_SHAPE",
    )?;
    require_json_text(object, "reason", "compiler-artifact")?;
    if !object.get("executable").is_some_and(Value::is_null) {
        return Err(Error::new(
            "COMPILER_CARGO_ARTIFACT_SHAPE",
            "library artifact unexpectedly names an executable",
        ));
    }
    let target = object.get("target").ok_or_else(|| {
        Error::new(
            "COMPILER_CARGO_ARTIFACT_SHAPE",
            "Cargo artifact target is absent",
        )
    })?;
    let target = parse_cargo_target(target)?;
    parse_cargo_profile(object.get("profile").ok_or_else(|| {
        Error::new(
            "COMPILER_CARGO_ARTIFACT_SHAPE",
            "Cargo artifact profile is absent",
        )
    })?)?;
    let features = value_string_set(object, "features")?;
    let filenames = value_path_set(object, "filenames")?;
    let fresh =
        object.get("fresh").and_then(Value::as_bool).ok_or_else(|| {
            Error::new(
                "COMPILER_CARGO_ARTIFACT_SHAPE",
                "Cargo artifact fresh is not a boolean",
            )
        })?;
    Ok(CargoArtifact {
        package_id: json_text(object, "package_id")?.to_owned(),
        manifest_path: PathBuf::from(json_text(object, "manifest_path")?),
        target_name: target.name,
        target_kinds: target.kinds,
        crate_types: target.crate_types,
        source_path: target.source_path,
        features,
        filenames,
        fresh,
    })
}

struct CargoTarget {
    name: String,
    kinds: Vec<String>,
    crate_types: Vec<String>,
    source_path: PathBuf,
}

fn parse_cargo_target(value: &Value) -> Result<CargoTarget, Error> {
    let object = closed_object(
        value,
        &[
            "kind",
            "crate_types",
            "name",
            "src_path",
            "edition",
            "doc",
            "doctest",
            "test",
        ],
        "COMPILER_CARGO_TARGET_SHAPE",
    )?;
    require_json_text(object, "edition", "2024")?;
    require_json_bool(object, "doc", true)?;
    require_json_bool(object, "doctest", false)?;
    require_json_bool(object, "test", false)?;
    Ok(CargoTarget {
        name: json_text(object, "name")?.to_owned(),
        kinds: value_string_set(object, "kind")?,
        crate_types: value_string_set(object, "crate_types")?,
        source_path: PathBuf::from(json_text(object, "src_path")?),
    })
}

fn parse_cargo_profile(value: &Value) -> Result<(), Error> {
    let object = closed_object(
        value,
        &[
            "opt_level",
            "debuginfo",
            "debug_assertions",
            "overflow_checks",
            "test",
        ],
        "COMPILER_CARGO_PROFILE_SHAPE",
    )?;
    require_json_text(object, "opt_level", "3")?;
    if object.get("debuginfo").and_then(Value::as_u64) != Some(0) {
        return Err(Error::new(
            "COMPILER_CARGO_PROFILE_SHAPE",
            "Cargo profile debuginfo is not zero",
        ));
    }
    require_json_bool(object, "debug_assertions", false)?;
    require_json_bool(object, "overflow_checks", false)?;
    require_json_bool(object, "test", false)
}

fn require_json_bool(
    object: &Map<String, Value>,
    name: &str,
    expected: bool,
) -> Result<(), Error> {
    if object.get(name).and_then(Value::as_bool) != Some(expected) {
        return Err(Error::new(
            "COMPILER_CARGO_MESSAGE_SHAPE",
            format!("Cargo field {name} differs from {expected}"),
        ));
    }
    Ok(())
}

fn value_string_set(
    object: &Map<String, Value>,
    name: &str,
) -> Result<Vec<String>, Error> {
    let values = json_string_array(object, name)?;
    let set = values.iter().collect::<BTreeSet<_>>();
    if set.len() != values.len() || !set.iter().copied().eq(values.iter()) {
        return Err(Error::new(
            "COMPILER_CARGO_MESSAGE_SHAPE",
            format!("Cargo field {name} is not sorted and unique"),
        ));
    }
    Ok(values)
}

fn value_path_set(
    object: &Map<String, Value>,
    name: &str,
) -> Result<Vec<PathBuf>, Error> {
    let values = json_string_array(object, name)?;
    if values.iter().collect::<BTreeSet<_>>().len() != values.len() {
        return Err(Error::new(
            "COMPILER_CARGO_MESSAGE_SHAPE",
            format!("Cargo field {name} contains duplicate paths"),
        ));
    }
    Ok(values.into_iter().map(PathBuf::from).collect())
}

fn parse_build_finished(value: &Value) -> Result<(), Error> {
    let object = closed_object(
        value,
        &["reason", "success"],
        "COMPILER_CARGO_FINISH_SHAPE",
    )?;
    require_json_text(object, "reason", "build-finished")?;
    require_json_bool(object, "success", true)
}

#[derive(Debug)]
struct DepInfoObservation {
    digest: String,
    bytes: u64,
    sources: Vec<PathBuf>,
}

fn parse_dep_info(
    bytes: &[u8],
    request: &BuildRequest<'_>,
    limits: Limits,
) -> Result<DepInfoObservation, Error> {
    if bytes.is_empty() || bytes.len() > limits.dep_info_file_bytes {
        return Err(Error::new(
            "COMPILER_DEP_INFO_LIMIT",
            "dep-info is empty or exceeds its byte bound",
        ));
    }
    if !bytes.ends_with(b"\n") || bytes.contains(&b'\r') || bytes.contains(&0) {
        return Err(Error::new(
            "COMPILER_DEP_INFO_FRAMING",
            "dep-info requires final LF without CR or NUL",
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|failure| {
        Error::new("COMPILER_DEP_INFO_UTF8", failure.to_string())
    })?;
    let logical = join_make_continuations(text)?;
    let dependencies = dependency_rule_set(&logical)?;
    let sources = relative_dep_info_sources(&dependencies, request, limits)?;
    if sources.iter().cloned().collect::<BTreeSet<_>>()
        != *request.admitted_sources
    {
        return Err(Error::new(
            "COMPILER_INPUT_SET",
            "compiler dep-info differs from the admitted module source set",
        ));
    }
    Ok(DepInfoObservation {
        digest: digest::bytes(bytes),
        bytes: u64::try_from(bytes.len()).map_err(|failure| {
            Error::new("COMPILER_DEP_INFO_LIMIT", failure.to_string())
        })?,
        sources,
    })
}

fn join_make_continuations(text: &str) -> Result<String, Error> {
    let mut output = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\\' && characters.peek() == Some(&'\n') {
            characters.next();
            output.push(' ');
            continue;
        }
        output.push(character);
    }
    if output.ends_with("\\\n") {
        return Err(Error::new(
            "COMPILER_DEP_INFO_FRAMING",
            "dep-info ends inside a continuation",
        ));
    }
    Ok(output)
}

fn dependency_rule_set(text: &str) -> Result<BTreeSet<String>, Error> {
    let mut expected = None;
    for line in text.lines() {
        if line.starts_with("# env-dep:") {
            return Err(Error::new(
                "COMPILER_ENV_DEP_UNAVAILABLE",
                "rustc dep-info contains an environment dependency",
            ));
        }
        if line.starts_with('#') {
            return Err(Error::new(
                "COMPILER_DEP_INFO_COMMENT_UNAVAILABLE",
                "unknown dep-info comment is unavailable",
            ));
        }
        if line.is_empty() {
            continue;
        }
        let Some(dependencies) = dependency_rule(line)? else { continue };
        let dependencies = parse_make_words(dependencies)?;
        if dependencies.is_empty() {
            continue;
        }
        let dependencies = dependencies.into_iter().collect::<BTreeSet<_>>();
        merge_dependency_set(&mut expected, dependencies)?;
    }
    expected.ok_or_else(|| {
        Error::new(
            "COMPILER_DEP_INFO_SHAPE",
            "dep-info contains no dependency-bearing output rule",
        )
    })
}

fn dependency_rule(line: &str) -> Result<Option<&str>, Error> {
    if let Some((_, dependencies)) = line.split_once(": ") {
        return Ok(Some(dependencies));
    }
    if line.ends_with(':') {
        return Ok(None);
    }
    Err(Error::new(
        "COMPILER_DEP_INFO_SHAPE",
        "dep-info line is not a Make rule",
    ))
}

fn merge_dependency_set(
    expected: &mut Option<BTreeSet<String>>,
    dependencies: BTreeSet<String>,
) -> Result<(), Error> {
    if expected.as_ref().is_some_and(|value| value != &dependencies) {
        return Err(Error::new(
            "COMPILER_DEP_INFO_SHAPE",
            "dep-info output rules disagree on their dependencies",
        ));
    }
    if expected.is_none() {
        *expected = Some(dependencies);
    }
    Ok(())
}

fn parse_make_words(value: &str) -> Result<Vec<String>, Error> {
    let mut output = Vec::new();
    let mut word = String::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            word.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character.is_ascii_whitespace() {
            push_make_word(&mut output, &mut word);
            continue;
        }
        word.push(character);
    }
    if escaped {
        return Err(Error::new(
            "COMPILER_DEP_INFO_ESCAPE",
            "dep-info ends inside a Make escape",
        ));
    }
    push_make_word(&mut output, &mut word);
    Ok(output)
}

fn push_make_word(output: &mut Vec<String>, word: &mut String) {
    if !word.is_empty() {
        output.push(std::mem::take(word));
    }
}

fn relative_dep_info_sources(
    dependencies: &BTreeSet<String>,
    request: &BuildRequest<'_>,
    limits: Limits,
) -> Result<Vec<PathBuf>, Error> {
    if dependencies.len() > limits.compiler_input_count {
        return Err(Error::new(
            "COMPILER_INPUT_LIMIT",
            "dep-info source count exceeds its conservative bound",
        ));
    }
    let mut output = Vec::with_capacity(dependencies.len());
    for dependency in dependencies {
        let absolute = Path::new(dependency);
        validate_absolute_path(absolute, "compiler input", limits.path_bytes)?;
        let relative = absolute
            .strip_prefix(request.repository_root)
            .map_err(|_| {
                Error::at(
                    "COMPILER_INPUT_AUTHORITY",
                    absolute,
                    "compiler input escapes the repository root",
                )
            })?
            .to_path_buf();
        output.push(relative);
    }
    output.sort();
    output.dedup();
    Ok(output)
}

/// Runs and correlates one exact fresh Product Cargo build.
///
/// Shipping requires an empty target authority. Validation reuses only the
/// supplied shipping provider and proves its retained bytes before and after
/// the build.
///
/// # Errors
///
/// Returns a typed error for command construction, process uncertainty,
/// malformed Cargo or wrapper evidence, source/output drift, unexpected
/// units, or invalid provider reuse.
pub fn run(
    request: &BuildRequest<'_>,
    limits: Limits,
) -> Result<Capture, Error> {
    let provider_before = request
        .provider
        .map(|provider| snapshot_provider(provider, limits))
        .transpose()?;
    let spec = command_spec(request, limits)?;
    let process = process::run(&spec, limits.process).map_err(|failure| {
        Error::new("COMPILER_CARGO_RUN", failure.to_string())
    })?;
    validate_cargo_process(&spec, &process)?;
    let command_argv = utf8_process_argv(&process.argv)?;
    let cargo = parse_cargo_output(command_argv, &process.stdout, limits)?;
    let traces = parse_traces(request, limits)?;
    let observation = correlate_build(request, cargo, traces, limits)?;
    if let (Some(provider), Some(before)) =
        (request.provider, provider_before.as_ref())
    {
        let after = snapshot_provider(provider, limits)?;
        if &after != before {
            return Err(Error::new(
                "COMPILER_PROVIDER_PRE_POST_DRIFT",
                "shipping provider bytes changed during validation",
            ));
        }
    }
    Ok(Capture { observation })
}

fn validate_cargo_process(
    spec: &CommandSpec,
    process: &ProcessResult,
) -> Result<(), Error> {
    if process.stdout_read_error.is_some()
        || process.stderr_read_error.is_some()
        || process.stdout_overflow
        || process.stderr_overflow
        || process.termination_reason != TerminationReason::Exited
    {
        return Err(Error::new(
            "COMPILER_CARGO_PROCESS_BOUND",
            "Cargo process or pipe capture is uncertain",
        ));
    }
    if !process.status.success() || !process.stderr.is_empty() {
        return Err(Error::new(
            "COMPILER_CARGO_PROCESS_STATUS",
            format!(
                "Cargo status {}; {}",
                process.status,
                cargo_stderr_summary(process),
            ),
        ));
    }
    if !process.reaped
        || !process.readers_joined
        || !process.group_identity_validated
        || !process.group_absent_after_reap
        || !process.descendants_owned
        || !process.descendants_reaped
        || process.descendant_residue_observed
    {
        return Err(Error::new(
            "COMPILER_CARGO_PROCESS_OWNERSHIP",
            "Cargo descendant ownership or cleanup is incomplete",
        ));
    }
    let expected = std::iter::once(spec.program.as_os_str().to_owned())
        .chain(spec.arguments.iter().cloned())
        .collect::<Vec<_>>();
    if process.argv != expected {
        return Err(Error::new(
            "COMPILER_CARGO_ARGV",
            "observed Cargo argv differs from the exact command",
        ));
    }
    Ok(())
}

fn cargo_stderr_summary(process: &ProcessResult) -> String {
    const LIMIT: usize = 512;

    let retained = process.stderr.get(..LIMIT).unwrap_or(&process.stderr);
    format!(
        "stderr_bytes={} stderr_digest={} stderr={}",
        process.stderr.len(),
        digest::bytes(&process.stderr),
        String::from_utf8_lossy(retained),
    )
}

fn utf8_process_argv(arguments: &[OsString]) -> Result<Vec<String>, Error> {
    arguments
        .iter()
        .map(|argument| {
            argument.to_str().map(str::to_owned).ok_or_else(|| {
                Error::new(
                    "COMPILER_CARGO_ARGV",
                    "observed Cargo argv is not UTF-8",
                )
            })
        })
        .collect()
}

fn correlate_build(
    request: &BuildRequest<'_>,
    cargo: CargoObservation,
    traces: Vec<WrapperTrace>,
    limits: Limits,
) -> Result<BuildObservation, Error> {
    let (unit_trace, probes) = split_traces(traces, request.closure.kind)?;
    let mut budget = ArtifactBudget::new(limits);
    let unit = compiler_unit(request, unit_trace, limits, &mut budget)?;
    match request.closure.kind {
        ClosureKind::Shipping => {
            correlate_shipping(request, cargo, probes, unit, &mut budget)
        }
        ClosureKind::Validation => {
            correlate_validation(request, cargo, probes, unit, &mut budget)
        }
    }
}

fn split_traces(
    traces: Vec<WrapperTrace>,
    closure_kind: ClosureKind,
) -> Result<(WrapperTrace, Vec<WrapperTrace>), Error> {
    let mut units = Vec::new();
    let mut probes = Vec::new();
    for trace in traces {
        if trace.kind == InvocationKind::CompilerUnit {
            units.push(trace);
        } else {
            probes.push(trace);
        }
    }
    let [unit] = units.as_slice() else {
        return Err(Error::new(
            "COMPILER_UNIT_SET",
            "build requires exactly one nonfresh compiler unit",
        ));
    };
    validate_probe_set(&probes, closure_kind)?;
    validate_trace_ordinals(unit, &probes, closure_kind)?;
    Ok((unit.clone(), probes))
}

fn validate_probe_set(
    probes: &[WrapperTrace],
    closure_kind: ClosureKind,
) -> Result<(), Error> {
    let observed = probes.iter().map(|probe| probe.kind).collect::<Vec<_>>();
    let expected = match closure_kind {
        ClosureKind::Shipping => vec![
            InvocationKind::VersionProbe,
            InvocationKind::HostProbe,
            InvocationKind::TargetProbe,
            InvocationKind::VersionProbe,
        ],
        ClosureKind::Validation => Vec::new(),
    };
    if observed != expected {
        return Err(Error::new(
            "COMPILER_DISCOVERY_PROBE_SET",
            "wrapper probes differ from the closure-specific exact sequence",
        ));
    }
    Ok(())
}

fn validate_trace_ordinals(
    unit: &WrapperTrace,
    probes: &[WrapperTrace],
    closure_kind: ClosureKind,
) -> Result<(), Error> {
    let expected_unit = match closure_kind {
        ClosureKind::Shipping => 4,
        ClosureKind::Validation => 0,
    };
    let probe_ordinals = probes.iter().map(|probe| probe.ordinal);
    let expected_probes =
        0..u64::try_from(probes.len()).map_err(|failure| {
            Error::new("COMPILER_TRACE_ORDINAL", failure.to_string())
        })?;
    if unit.ordinal != expected_unit || !probe_ordinals.eq(expected_probes) {
        return Err(Error::new(
            "COMPILER_TRACE_ORDINAL",
            "probe and compiler-unit ordinals are not exact",
        ));
    }
    Ok(())
}

fn compiler_unit(
    request: &BuildRequest<'_>,
    trace: WrapperTrace,
    limits: Limits,
    budget: &mut ArtifactBudget,
) -> Result<CompilerUnit, Error> {
    let config = recorded_wrapper_config(request, limits);
    let arguments = parse_unit_arguments(&trace.original_argv, &config)?;
    let dep_info_path = trace.dep_info_path.clone().ok_or_else(|| {
        Error::new(
            "COMPILER_DEP_INFO_PATH",
            "compiler unit trace has no private dep-info path",
        )
    })?;
    let dep_info_bytes = read_regular_bounded(
        &dep_info_path,
        limits.dep_info_file_bytes,
        "COMPILER_DEP_INFO_READ",
    )?;
    if dep_info_bytes.len() > limits.dep_info_total_bytes {
        return Err(Error::new(
            "COMPILER_DEP_INFO_LIMIT",
            "aggregate dep-info bytes exceed the limit",
        ));
    }
    let dep_info = parse_dep_info(&dep_info_bytes, request, limits)?;
    let outputs = identify_unit_outputs(&arguments, request, budget)?;
    Ok(CompilerUnit {
        id: format!("{}:rustc", request.closure.id),
        closure: request.closure.id.clone(),
        crate_name: arguments.crate_name,
        crate_types: arguments.crate_types.into_iter().collect(),
        target_triple: arguments.target,
        profile: request.closure.cargo.profile.clone(),
        features: arguments.features.into_iter().collect(),
        cfgs: arguments.cfgs.into_iter().collect(),
        original_argv: trace.original_argv,
        effective_argv: trace.effective_argv,
        dep_info_path,
        dep_info_digest: dep_info.digest,
        dep_info_bytes: dep_info.bytes,
        sources: dep_info.sources,
        externs: arguments.externs.into_iter().collect(),
        outputs,
        trace_digest: trace.trace_digest,
        trace_bytes: trace.trace_bytes,
        status: trace.status,
    })
}

struct ArtifactBudget {
    count: usize,
    bytes: u64,
    limits: Limits,
}

impl ArtifactBudget {
    const fn new(limits: Limits) -> Self {
        Self { count: 0, bytes: 0, limits }
    }

    fn identify(&mut self, path: &Path) -> Result<FileIdentity, Error> {
        self.count = self.count.checked_add(1).ok_or_else(|| {
            Error::new("COMPILER_ARTIFACT_LIMIT", "artifact count overflow")
        })?;
        if self.count > self.limits.artifact_count {
            return Err(Error::new(
                "COMPILER_ARTIFACT_LIMIT",
                "artifact count exceeds its bound",
            ));
        }
        let identity = identify_file(path, self.limits)?;
        self.bytes =
            self.bytes.checked_add(identity.size_bytes).ok_or_else(|| {
                Error::new("COMPILER_ARTIFACT_LIMIT", "artifact bytes overflow")
            })?;
        if self.bytes > self.limits.artifact_total_bytes {
            return Err(Error::new(
                "COMPILER_ARTIFACT_LIMIT",
                "artifact bytes exceed their aggregate bound",
            ));
        }
        Ok(identity)
    }
}

fn identify_file(path: &Path, limits: Limits) -> Result<FileIdentity, Error> {
    validate_absolute_path(path, "compiler artifact", limits.path_bytes)?;
    let metadata = fs::symlink_metadata(path).map_err(|failure| {
        Error::at("COMPILER_ARTIFACT_METADATA", path, failure.to_string())
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::at(
            "COMPILER_ARTIFACT_KIND",
            path,
            "artifact is not a regular non-symlink file",
        ));
    }
    if metadata.len() == 0 || metadata.len() > limits.artifact_total_bytes {
        return Err(Error::at(
            "COMPILER_ARTIFACT_LIMIT",
            path,
            "artifact is empty or exceeds the byte bound",
        ));
    }
    let content_digest =
        digest::file_bounded(path, limits.artifact_total_bytes).map_err(
            |failure| {
                Error::at("COMPILER_ARTIFACT_READ", path, failure.to_string())
            },
        )?;
    Ok(FileIdentity {
        path: path.to_path_buf(),
        content_digest,
        size_bytes: metadata.len(),
    })
}

fn identify_unit_outputs(
    arguments: &UnitArguments,
    request: &BuildRequest<'_>,
    budget: &mut ArtifactBudget,
) -> Result<Vec<FileIdentity>, Error> {
    let stem = format!("{}{}", arguments.crate_name, arguments.extra_filename);
    let paths = [
        arguments.out_dir.join(format!("lib{stem}.rlib")),
        arguments.out_dir.join(format!("lib{stem}.rmeta")),
    ];
    let mut output = Vec::new();
    for path in paths {
        if !path.starts_with(request.target_dir) {
            return Err(Error::at(
                "COMPILER_OUTPUT_AUTHORITY",
                path,
                "compiler output escapes the target authority",
            ));
        }
        output.push(budget.identify(&path)?);
    }
    output.sort();
    Ok(output)
}

fn correlate_shipping(
    request: &BuildRequest<'_>,
    cargo: CargoObservation,
    probes: Vec<WrapperTrace>,
    unit: CompilerUnit,
    budget: &mut ArtifactBudget,
) -> Result<BuildObservation, Error> {
    let [artifact] = cargo.artifacts.as_slice() else {
        return Err(Error::new(
            "COMPILER_CARGO_ARTIFACT_SET",
            "shipping build requires exactly one Cargo artifact",
        ));
    };
    validate_closure_artifact(artifact, request, false)?;
    if !unit.externs.is_empty() {
        return Err(Error::new(
            "COMPILER_SHIPPING_EXTERN",
            "shipping compiler unit supplies a non-sysroot extern",
        ));
    }
    let cargo_files = identify_cargo_files(artifact, request, budget)?;
    let top_level_rlib = extension_identity(&cargo_files, "rlib")?;
    let cargo_rmeta = extension_identity(&cargo_files, "rmeta")?;
    let hashed_rlib = extension_identity(&unit.outputs, "rlib")?;
    let unit_rmeta = extension_identity(&unit.outputs, "rmeta")?;
    require_same_file_identity(cargo_rmeta, unit_rmeta, true)?;
    require_same_file_identity(top_level_rlib, hashed_rlib, false)?;
    let provider = ProviderArtifacts {
        closure: request.closure.id.clone(),
        package_id: request.package_id.to_owned(),
        unit: unit.id.clone(),
        cargo_artifact: artifact.clone(),
        top_level_rlib: top_level_rlib.clone(),
        hashed_rlib: hashed_rlib.clone(),
        rmeta: unit_rmeta.clone(),
        target_dir: request.target_dir.to_path_buf(),
        cargo_home: request.cargo_home.to_path_buf(),
    };
    Ok(BuildObservation {
        closure: request.closure.id.clone(),
        cargo,
        probes,
        unit,
        provider: Some(provider),
        provider_reuse: None,
    })
}

fn correlate_validation(
    request: &BuildRequest<'_>,
    cargo: CargoObservation,
    probes: Vec<WrapperTrace>,
    unit: CompilerUnit,
    budget: &mut ArtifactBudget,
) -> Result<BuildObservation, Error> {
    let provider = request.provider.expect("request validation checked");
    if cargo.artifacts.len() != 2 {
        return Err(Error::new(
            "COMPILER_CARGO_ARTIFACT_SET",
            "validation requires one fresh provider and one consumer artifact",
        ));
    }
    let fresh = cargo
        .artifacts
        .iter()
        .find(|artifact| artifact.package_id == provider.package_id)
        .ok_or_else(|| {
            Error::new(
                "COMPILER_PROVIDER_EVENT",
                "validation Cargo stream lacks its provider event",
            )
        })?;
    let consumer = cargo
        .artifacts
        .iter()
        .find(|artifact| artifact.package_id == request.package_id)
        .ok_or_else(|| {
            Error::new(
                "COMPILER_CARGO_ARTIFACT_SET",
                "validation Cargo stream lacks its consumer event",
            )
        })?;
    validate_closure_artifact(consumer, request, false)?;
    validate_provider_event(fresh, provider)?;
    let fresh_files = identify_cargo_files(fresh, request, budget)?;
    let fresh_rlib = extension_identity(&fresh_files, "rlib")?;
    let fresh_rmeta = extension_identity(&fresh_files, "rmeta")?;
    require_same_file_identity(fresh_rlib, &provider.hashed_rlib, true)?;
    require_same_file_identity(fresh_rmeta, &provider.rmeta, true)?;
    require_same_file_identity(fresh_rlib, &provider.top_level_rlib, false)?;
    let extern_edge = exact_provider_extern(&unit, provider)?;
    correlate_consumer_artifact(consumer, request, &unit, budget)?;
    let provider_reuse = ProviderReuse {
        fresh_artifact: fresh.clone(),
        hashed_rlib: fresh_rlib.clone(),
        rmeta: fresh_rmeta.clone(),
        extern_edge,
    };
    Ok(BuildObservation {
        closure: request.closure.id.clone(),
        cargo,
        probes,
        unit,
        provider: None,
        provider_reuse: Some(provider_reuse),
    })
}

fn validate_closure_artifact(
    artifact: &CargoArtifact,
    request: &BuildRequest<'_>,
    fresh: bool,
) -> Result<(), Error> {
    let manifest =
        request.repository_root.join(&request.closure.package.manifest);
    let source = request.repository_root.join(&request.closure.target.src_path);
    let identity_matches = artifact.package_id == request.package_id
        && artifact.manifest_path == manifest
        && artifact.target_name == request.closure.target.name
        && artifact.target_kinds == ["lib"]
        && artifact.crate_types == ["lib"]
        && artifact.source_path == source
        && artifact.features == [EXPECTED_FEATURE]
        && artifact.fresh == fresh;
    if !identity_matches {
        return Err(Error::new(
            "COMPILER_CARGO_ARTIFACT_IDENTITY",
            "Cargo artifact differs from its exact closure",
        ));
    }
    validate_artifact_filenames(artifact, request)
}

fn validate_artifact_filenames(
    artifact: &CargoArtifact,
    request: &BuildRequest<'_>,
) -> Result<(), Error> {
    if artifact.filenames.len() != 2 {
        return Err(Error::new(
            "COMPILER_CARGO_ARTIFACT_FILENAMES",
            "library artifact requires exactly rlib and rmeta filenames",
        ));
    }
    let mut extensions = BTreeSet::new();
    for path in &artifact.filenames {
        validate_absolute_path(path, "Cargo artifact", usize::MAX)?;
        if !path.starts_with(request.target_dir) {
            return Err(Error::at(
                "COMPILER_CARGO_ARTIFACT_FILENAMES",
                path,
                "Cargo artifact escapes the target authority",
            ));
        }
        let extension =
            path.extension().and_then(OsStr::to_str).ok_or_else(|| {
                Error::at(
                    "COMPILER_CARGO_ARTIFACT_FILENAMES",
                    path,
                    "Cargo artifact has no UTF-8 extension",
                )
            })?;
        extensions.insert(extension.to_owned());
    }
    if extensions != BTreeSet::from(["rlib".to_owned(), "rmeta".to_owned()]) {
        return Err(Error::new(
            "COMPILER_CARGO_ARTIFACT_FILENAMES",
            "Cargo artifact extensions are not exactly rlib and rmeta",
        ));
    }
    Ok(())
}

fn identify_cargo_files(
    artifact: &CargoArtifact,
    request: &BuildRequest<'_>,
    budget: &mut ArtifactBudget,
) -> Result<Vec<FileIdentity>, Error> {
    validate_artifact_filenames(artifact, request)?;
    let mut output = artifact
        .filenames
        .iter()
        .map(|path| budget.identify(path))
        .collect::<Result<Vec<_>, _>>()?;
    output.sort();
    Ok(output)
}

fn extension_identity<'a>(
    identities: &'a [FileIdentity],
    extension: &str,
) -> Result<&'a FileIdentity, Error> {
    let matches = identities
        .iter()
        .filter(|identity| {
            identity.path.extension().and_then(OsStr::to_str) == Some(extension)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [identity] => Ok(identity),
        _ => Err(Error::new(
            "COMPILER_ARTIFACT_SET",
            format!("artifact set lacks one exact {extension}"),
        )),
    }
}

fn require_same_file_identity(
    left: &FileIdentity,
    right: &FileIdentity,
    require_path: bool,
) -> Result<(), Error> {
    let bytes_match = left.content_digest == right.content_digest
        && left.size_bytes == right.size_bytes;
    let path_matches = !require_path || left.path == right.path;
    if !bytes_match || !path_matches {
        return Err(Error::new(
            "COMPILER_PROVIDER_ARTIFACT_MISMATCH",
            "provider artifact path or bytes do not correlate",
        ));
    }
    Ok(())
}

fn validate_provider_event(
    fresh: &CargoArtifact,
    provider: &ProviderArtifacts,
) -> Result<(), Error> {
    if !fresh.fresh || !same_artifact_semantics(fresh, &provider.cargo_artifact)
    {
        return Err(Error::new(
            "COMPILER_PROVIDER_EVENT",
            "fresh provider semantic identity differs from shipping",
        ));
    }
    Ok(())
}

fn same_artifact_semantics(
    left: &CargoArtifact,
    right: &CargoArtifact,
) -> bool {
    left.package_id == right.package_id
        && left.manifest_path == right.manifest_path
        && left.target_name == right.target_name
        && left.target_kinds == right.target_kinds
        && left.crate_types == right.crate_types
        && left.source_path == right.source_path
        && left.features == right.features
}

fn exact_provider_extern(
    unit: &CompilerUnit,
    provider: &ProviderArtifacts,
) -> Result<ExternEdge, Error> {
    let [edge] = unit.externs.as_slice() else {
        return Err(Error::new(
            "COMPILER_EXTERN_EDGE_SET",
            "consumer requires exactly one provider extern edge",
        ));
    };
    if edge.name != "reovim_foundation" || edge.path != provider.rmeta.path {
        return Err(Error::new(
            "COMPILER_EXTERN_EDGE_SET",
            "consumer extern does not select exact provider metadata",
        ));
    }
    Ok(edge.clone())
}

fn correlate_consumer_artifact(
    artifact: &CargoArtifact,
    request: &BuildRequest<'_>,
    unit: &CompilerUnit,
    budget: &mut ArtifactBudget,
) -> Result<(), Error> {
    let cargo_files = identify_cargo_files(artifact, request, budget)?;
    let cargo_rlib = extension_identity(&cargo_files, "rlib")?;
    let cargo_rmeta = extension_identity(&cargo_files, "rmeta")?;
    let unit_rlib = extension_identity(&unit.outputs, "rlib")?;
    let unit_rmeta = extension_identity(&unit.outputs, "rmeta")?;
    require_same_file_identity(cargo_rlib, unit_rlib, false)?;
    require_same_file_identity(cargo_rmeta, unit_rmeta, true)
}

fn snapshot_provider(
    provider: &ProviderArtifacts,
    limits: Limits,
) -> Result<Vec<FileIdentity>, Error> {
    let mut budget = ArtifactBudget::new(limits);
    let expected =
        [&provider.top_level_rlib, &provider.hashed_rlib, &provider.rmeta];
    let mut observed = Vec::new();
    for identity in expected {
        if !identity.path.starts_with(&provider.target_dir) {
            return Err(Error::at(
                "COMPILER_PROVIDER_TARGET",
                &identity.path,
                "provider artifact escapes its target authority",
            ));
        }
        let current = budget.identify(&identity.path)?;
        require_same_file_identity(&current, identity, true).map_err(|_| {
            Error::new(
                "COMPILER_PROVIDER_PRE_DRIFT",
                "provider artifact changed before validation",
            )
        })?;
        observed.push(current);
    }
    observed.sort();
    Ok(observed)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        ffi::{OsStr, OsString},
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::Duration,
    };

    use serde_json::{Value, json};

    use super::{
        BuildRequest, CargoArtifact, EnvironmentEntry, FileIdentity,
        InvocationKind, Limits, ProviderArtifacts, WrapperConfig,
        WrapperLimits, WrapperTrace, build_arguments, build_environment,
        classify_invocation, parse_cargo_output, parse_dep_info,
        parse_trace_pair, plan_invocation, require_same_file_identity,
        same_artifact_semantics, validate_metadata_preflight, validate_probe,
        validate_probe_set, validate_provider_event, validate_recorded_probe,
        wrapper_child_environment,
    };
    use crate::{
        cargo::{
            DependencyKind, Metadata, Node, Package, ResolvedDependency, Target,
        },
        process::ProcessLimits,
        product_closures::{ClosureKind, ParseLimits, Registry, TARGET_TRIPLE},
    };

    const LIMITS: Limits = Limits {
        process: ProcessLimits {
            stdout_bytes: 16_777_216,
            stderr_bytes: 1_048_576,
            deadline: Duration::from_secs(120),
            supervision_deadline: Duration::from_secs(130),
        },
        path_bytes: 4096,
        cargo_json_bytes: 16_777_216,
        cargo_message_count: 4096,
        wrapper_trace_bytes: 16_777_216,
        wrapper_argv_items: 4096,
        wrapper_argv_bytes: 1_048_576,
        wrapper_trace_count: 256,
        dep_info_file_bytes: 4_194_304,
        dep_info_total_bytes: 67_108_864,
        compiler_input_count: 8192,
        artifact_count: 4096,
        artifact_total_bytes: 536_870_912,
        json_depth: 128,
        json_items: 1_000_000,
    };

    const SHIPPING_ID: &str = "foundation-package-id";
    const CONSUMER_ID: &str = "foundation-consumer-package-id";
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        scratch: PathBuf,
        cargo_home: PathBuf,
        temp_dir: PathBuf,
        target_dir: PathBuf,
        trace_dir: PathBuf,
        dep_info_dir: PathBuf,
        repository: PathBuf,
        executable: PathBuf,
        registry: Registry,
        metadata: Metadata,
        shipping_sources: BTreeSet<PathBuf>,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.scratch).expect("remove capture fixture");
        }
    }

    #[test]
    fn build_arguments_and_environment_are_exact() {
        let fixture = fixture();
        let request = shipping_request(&fixture);
        let manifest = fixture.repository.join("product/Cargo.toml");
        let arguments = build_arguments(&request, &manifest);
        let text = arguments
            .iter()
            .map(|value| value.to_str().expect("UTF-8 argument"))
            .collect::<Vec<_>>();
        assert_eq!(
            text,
            [
                "build",
                "--quiet",
                "--locked",
                "--offline",
                "--manifest-path",
                manifest.to_str().expect("manifest text"),
                "--package",
                "reovim-foundation",
                "--lib",
                "--target",
                TARGET_TRIPLE,
                "--profile",
                "release",
                "--jobs",
                "1",
                "--message-format=json",
                "--target-dir",
                request.target_dir.to_str().expect("target text"),
            ]
        );
        let build_dir = request.target_dir.join("build-dir");
        let environment = build_environment(&request, LIMITS, &build_dir);
        assert_eq!(environment[OsStr::new("CARGO_INCREMENTAL")], "0");
        assert_eq!(environment[OsStr::new("CARGO_NET_OFFLINE")], "true");
        assert_eq!(environment[OsStr::new("LD_LIBRARY_PATH")], "/compiler/lib");
        assert_eq!(
            environment[OsStr::new("REOVIM_CAPTURE_SYSROOT")],
            "/sysroot"
        );
        assert_eq!(
            environment[OsStr::new("RUSTC_WRAPPER")],
            fixture.executable
        );
    }

    #[test]
    fn observed_unit_argv_rewrites_only_dep_info_and_sysroot() {
        let fixture = fixture();
        let config = wrapper_config(&fixture);
        let arguments = unit_arguments(&fixture, false);
        let plan = plan_invocation(
            &to_os(&arguments),
            compiler_environment(&fixture),
            &config,
            4,
        )
        .expect("observed Rust 1.92 unit argv");
        assert_eq!(plan.kind, InvocationKind::CompilerUnit);
        assert_eq!(plan.original, arguments);
        assert_eq!(
            &plan.effective[plan.effective.len() - 2..],
            ["--sysroot", "/sysroot"]
        );
        assert!(plan.effective.iter().any(|argument| {
            argument.starts_with("--emit=dep-info=")
                && argument.ends_with(",metadata,link")
        }));
        assert_eq!(
            plan.dep_info_path,
            Some(fixture.scratch.join("dep-info/00000004.d"))
        );
    }

    #[test]
    fn dangerous_and_search_path_argv_mutants_fail_closed() {
        let cases = [
            (
                ArgMutation::Append("@response"),
                "COMPILER_RESPONSE_FILE_UNAVAILABLE",
            ),
            (ArgMutation::Append("-lfoo"), "COMPILER_NATIVE_LINK_UNAVAILABLE"),
            (
                ArgMutation::AppendPair("-C", "link-arg=-Wl,x"),
                "COMPILER_CODEGEN_FLAG_UNAVAILABLE",
            ),
            (
                ArgMutation::AppendPair("-C", "incremental=/tmp/cache"),
                "COMPILER_CODEGEN_FLAG_UNAVAILABLE",
            ),
            (
                ArgMutation::AppendPair("-C", "target-cpu=native"),
                "COMPILER_UNIT_ARGV_GRAMMAR",
            ),
            (
                ArgMutation::AppendPair("-Z", "unstable-options"),
                "COMPILER_UNSTABLE_FLAG_UNAVAILABLE",
            ),
            (
                ArgMutation::Append("--unknown-option"),
                "COMPILER_UNIT_ARGV_GRAMMAR",
            ),
            (
                ArgMutation::AppendPair("--sysroot", "/other"),
                "COMPILER_SYSROOT_FLAG_UNAVAILABLE",
            ),
            (
                ArgMutation::ReplaceFirst("dependency=", "native="),
                "COMPILER_SEARCH_PATH_UNAVAILABLE",
            ),
            (ArgMutation::RemoveFirstPair("-L"), "COMPILER_SEARCH_PATH_SET"),
            (ArgMutation::DuplicateFirstPair("-L"), "COMPILER_SEARCH_PATH_SET"),
        ];
        for (mutation, code) in cases {
            assert_argv_mutant(mutation, code);
        }
    }

    #[test]
    fn probe_classification_and_exact_counts_are_closed() {
        assert_eq!(
            classify_invocation(&["rustc".to_owned(), "-vV".to_owned()])
                .expect("version probe"),
            InvocationKind::VersionProbe
        );
        let host = probe_arguments(false);
        let target = probe_arguments(true);
        assert_eq!(
            classify_invocation(&host).expect("host probe"),
            InvocationKind::HostProbe
        );
        assert_eq!(
            classify_invocation(&target).expect("target probe"),
            InvocationKind::TargetProbe
        );
        let mut expanded = host;
        expanded.push("--unknown".to_owned());
        assert_eq!(
            validate_probe(&expanded, InvocationKind::HostProbe)
                .expect_err("expanded probe")
                .code,
            "COMPILER_DISCOVERY_PROBE_UNAVAILABLE"
        );
        let exact = traces_for_kinds(&[
            InvocationKind::VersionProbe,
            InvocationKind::HostProbe,
            InvocationKind::TargetProbe,
            InvocationKind::VersionProbe,
        ]);
        validate_probe_set(&exact, ClosureKind::Shipping)
            .expect("exact shipping probe sequence");
        assert_eq!(
            validate_probe_set(&exact[..3], ClosureKind::Shipping)
                .expect_err("missing version probe")
                .code,
            "COMPILER_DISCOVERY_PROBE_SET"
        );
        let mut extra = exact;
        extra.push(dummy_trace(4, InvocationKind::VersionProbe));
        assert_eq!(
            validate_probe_set(&extra, ClosureKind::Shipping)
                .expect_err("duplicate version probe")
                .code,
            "COMPILER_DISCOVERY_PROBE_SET"
        );
        validate_probe_set(&[], ClosureKind::Validation)
            .expect("validation reuses compiler discovery");
        assert_eq!(
            validate_probe_set(&extra[..1], ClosureKind::Validation)
                .expect_err("validation probe is unexpected")
                .code,
            "COMPILER_DISCOVERY_PROBE_SET"
        );
    }

    #[test]
    fn cargo_json_parser_accepts_real_shape_and_rejects_helpers() {
        let fixture = fixture();
        let request = shipping_request(&fixture);
        let artifact = cargo_artifact_json(&request, false);
        let output = cargo_stream(&[
            artifact,
            json!({"reason": "build-finished", "success": true}),
        ]);
        let observation = parse_cargo_output(
            vec!["cargo".to_owned(), "build".to_owned()],
            &output,
            LIMITS,
        )
        .expect("real Cargo artifact shape");
        assert_eq!(observation.artifacts.len(), 1);
        assert_eq!(observation.artifacts[0].features, ["default"]);

        let helper = cargo_stream(&[
            json!({"reason": "build-script-executed"}),
            json!({"reason": "build-finished", "success": true}),
        ]);
        assert_eq!(
            parse_cargo_output(Vec::new(), &helper, LIMITS)
                .expect_err("build script unavailable")
                .code,
            "PRODUCT_BUILD_SCRIPT_PROOF_UNAVAILABLE"
        );
    }

    #[test]
    fn cargo_json_framing_and_filename_mutants_fail() {
        let fixture = fixture();
        let request = shipping_request(&fixture);
        let mut artifact = cargo_artifact_json(&request, false);
        let first = artifact["filenames"][0].clone();
        artifact["filenames"][1] = first;
        let duplicate = cargo_stream(&[
            artifact,
            json!({"reason": "build-finished", "success": true}),
        ]);
        assert_eq!(
            parse_cargo_output(Vec::new(), &duplicate, LIMITS)
                .expect_err("duplicate filename")
                .code,
            "COMPILER_CARGO_MESSAGE_SHAPE"
        );
        let mut unterminated = cargo_stream(&[
            json!({"reason": "build-finished", "success": true}),
        ]);
        unterminated.pop();
        assert_eq!(
            parse_cargo_output(Vec::new(), &unterminated, LIMITS)
                .expect_err("missing final LF")
                .code,
            "COMPILER_CARGO_JSON_FRAMING"
        );
    }

    #[test]
    fn dep_info_parser_joins_exact_real_module_sources() {
        let fixture = fixture();
        let request = shipping_request(&fixture);
        let dependencies = fixture
            .shipping_sources
            .iter()
            .map(|source| fixture.repository.join(source))
            .map(|source| source.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        let bytes = format!("/scratch/unit.d: {dependencies}\n").into_bytes();
        let observation =
            parse_dep_info(&bytes, &request, LIMITS).expect("exact dep-info");
        assert_eq!(
            observation.sources,
            fixture.shipping_sources.iter().cloned().collect::<Vec<_>>()
        );

        let mut environment = bytes;
        environment.extend_from_slice(b"# env-dep:HOME=/secret\n");
        assert_eq!(
            parse_dep_info(&environment, &request, LIMITS)
                .expect_err("environment dependency")
                .code,
            "COMPILER_ENV_DEP_UNAVAILABLE"
        );
    }

    #[test]
    fn trace_pair_requires_matching_identity_and_status() {
        let start = trace_start_json(2);
        let finish = trace_finish_json(2, 0);
        let start_file = (PathBuf::from("2.start.json"), start);
        let finish_file = (PathBuf::from("2.finish.json"), finish);
        let trace = parse_trace_pair(2, &start_file, &finish_file, LIMITS)
            .expect("paired trace");
        assert_eq!(trace.kind, InvocationKind::VersionProbe);

        let failed = (PathBuf::from("2.finish.json"), trace_finish_json(2, 1));
        assert_eq!(
            parse_trace_pair(2, &start_file, &failed, LIMITS)
                .expect_err("nonzero trace")
                .code,
            "COMPILER_TRACE_STATUS"
        );
    }

    #[test]
    fn probe_effective_argv_allows_only_exact_sysroot_injection() {
        let fixture = fixture();
        let config = wrapper_config(&fixture);
        let mut trace = dummy_trace(0, InvocationKind::VersionProbe);
        trace.original_argv[0] =
            fixture.executable.to_string_lossy().into_owned();
        trace.effective_argv[0] = trace.original_argv[0].clone();
        validate_recorded_probe(&trace, &config).expect("exact sysroot delta");
        trace.effective_argv.push("--unknown".to_owned());
        assert_eq!(
            validate_recorded_probe(&trace, &config)
                .expect_err("effective argv drift")
                .code,
            "COMPILER_TRACE_PROBE"
        );
    }

    #[test]
    fn metadata_helper_target_fails_before_compiler_execution() {
        assert_helper_target("custom-build", "build-script-build");
        assert_helper_target("proc-macro", "foundation_macros");
    }

    fn assert_helper_target(kind: &str, name: &str) {
        let mut fixture = fixture();
        let source = fixture.repository.join("kernel/foundation/build.rs");
        let package = fixture
            .metadata
            .packages
            .get_mut(SHIPPING_ID)
            .expect("shipping package");
        package.targets.push(Target {
            name: name.to_owned(),
            kind: vec![kind.to_owned()],
            crate_types: vec!["bin".to_owned()],
            src_path: source,
        });
        let request = shipping_request(&fixture);
        assert_eq!(
            validate_metadata_preflight(&request)
                .expect_err("build helper unavailable")
                .code,
            "PRODUCT_HELPER_PROOF_UNAVAILABLE"
        );
    }

    #[test]
    fn wrapper_environment_rejects_out_dir_and_unknown_values() {
        let fixture = fixture();
        let config = wrapper_config(&fixture);
        let mut environment = wrapper_environment_map(&fixture);
        environment.insert(OsString::from("OUT_DIR"), OsString::from("/tmp"));
        assert_eq!(
            wrapper_child_environment(&environment, &config)
                .expect_err("OUT_DIR unavailable")
                .code,
            "PRODUCT_GENERATOR_PROOF_UNAVAILABLE"
        );
    }

    #[test]
    fn equal_provider_semantics_do_not_waive_byte_mismatch() {
        let fixture = fixture();
        let request = shipping_request(&fixture);
        let shipping = cargo_artifact_value(&request, false);
        let mut fresh = shipping.clone();
        fresh.fresh = true;
        let identity = FileIdentity {
            path: fixture.scratch.join("target/provider.rlib"),
            content_digest: digest('a'),
            size_bytes: 10,
        };
        let provider = provider_artifacts(shipping, &identity);
        validate_provider_event(&fresh, &provider)
            .expect("semantic identity remains equal");
        assert!(same_artifact_semantics(&fresh, &provider.cargo_artifact));
        let changed = FileIdentity { content_digest: digest('b'), ..identity };
        assert_eq!(
            require_same_file_identity(&changed, &provider.hashed_rlib, true,)
                .expect_err("equal semantics cannot waive changed bytes")
                .code,
            "COMPILER_PROVIDER_ARTIFACT_MISMATCH"
        );
    }

    #[derive(Clone, Copy)]
    enum ArgMutation {
        Append(&'static str),
        AppendPair(&'static str, &'static str),
        ReplaceFirst(&'static str, &'static str),
        RemoveFirstPair(&'static str),
        DuplicateFirstPair(&'static str),
    }

    fn assert_argv_mutant(mutation: ArgMutation, expected: &'static str) {
        let fixture = fixture();
        let config = wrapper_config(&fixture);
        let mut arguments = unit_arguments(&fixture, false);
        mutate_arguments(&mut arguments, mutation);
        let error = plan_invocation(
            &to_os(&arguments),
            compiler_environment(&fixture),
            &config,
            4,
        )
        .expect_err("argv mutant must fail");
        assert_eq!(error.code, expected, "{error}");
    }

    fn mutate_arguments(arguments: &mut Vec<String>, mutation: ArgMutation) {
        match mutation {
            ArgMutation::Append(value) => arguments.push(value.to_owned()),
            ArgMutation::AppendPair(left, right) => {
                arguments.extend([left.to_owned(), right.to_owned()]);
            }
            ArgMutation::ReplaceFirst(old, new) => {
                let value = arguments
                    .iter_mut()
                    .find(|value| value.contains(old))
                    .expect("replace mutation anchor");
                *value = value.replacen(old, new, 1);
            }
            ArgMutation::RemoveFirstPair(prefix) => {
                let index = arguments
                    .iter()
                    .position(|value| value.starts_with(prefix))
                    .expect("remove mutation anchor");
                arguments.remove(index);
                arguments.remove(index);
            }
            ArgMutation::DuplicateFirstPair(prefix) => {
                let index = arguments
                    .iter()
                    .position(|value| value.starts_with(prefix))
                    .expect("duplicate mutation anchor");
                let pair = arguments[index..=index + 1].to_vec();
                arguments.extend(pair);
            }
        }
    }

    fn fixture() -> Fixture {
        let serial = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let scratch = std::env::temp_dir().join(format!(
            "reovim-compiler-capture-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&scratch).expect("create capture fixture");
        let cargo_home = scratch.join("cargo-home");
        let temp_dir = scratch.join("temp");
        let target_dir = scratch.join("target");
        let trace_dir = scratch.join("trace");
        let dep_info_dir = scratch.join("dep-info");
        for directory in
            [&cargo_home, &temp_dir, &target_dir, &trace_dir, &dep_info_dir]
        {
            fs::create_dir(directory).expect("create fixture dir");
        }
        let repository = fs::canonicalize(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .expect("repository root"),
        )
        .expect("canonical repository");
        let executable = fs::canonicalize(
            std::env::current_exe().expect("current test executable"),
        )
        .expect("canonical test executable");
        let registry = read_registry(&repository);
        let metadata = metadata(&repository, &registry);
        let shipping_sources = [
            "kernel/foundation/src/bound.rs",
            "kernel/foundation/src/carrier.rs",
            "kernel/foundation/src/contract.rs",
            "kernel/foundation/src/identity.rs",
            "kernel/foundation/src/lib.rs",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect();
        Fixture {
            scratch,
            cargo_home,
            temp_dir,
            target_dir,
            trace_dir,
            dep_info_dir,
            repository,
            executable,
            registry,
            metadata,
            shipping_sources,
        }
    }

    fn read_registry(repository: &Path) -> Registry {
        let bytes =
            fs::read(repository.join("conformance/product-closures.json"))
                .expect("read closure registry");
        let value: Value =
            serde_json::from_slice(&bytes).expect("parse registry");
        Registry::parse(
            &value,
            ParseLimits {
                path_bytes: 4096,
                closure_items: 1024,
                package_members: 8192,
                public_items: 8192,
            },
        )
        .expect("validate closure registry")
    }

    fn metadata(repository: &Path, registry: &Registry) -> Metadata {
        let shipping = &registry.closures[1];
        let consumer = &registry.closures[0];
        let packages = BTreeMap::from([
            (
                SHIPPING_ID.to_owned(),
                package(repository, shipping, SHIPPING_ID),
            ),
            (
                CONSUMER_ID.to_owned(),
                package(repository, consumer, CONSUMER_ID),
            ),
        ]);
        let nodes = BTreeMap::from([
            (
                SHIPPING_ID.to_owned(),
                Node { id: SHIPPING_ID.to_owned(), dependencies: Vec::new() },
            ),
            (
                CONSUMER_ID.to_owned(),
                Node {
                    id: CONSUMER_ID.to_owned(),
                    dependencies: vec![ResolvedDependency {
                        name: "reovim_foundation".to_owned(),
                        package: SHIPPING_ID.to_owned(),
                        kinds: vec![DependencyKind {
                            kind: "normal".to_owned(),
                            target: None,
                        }],
                    }],
                },
            ),
        ]);
        Metadata {
            workspace_root: repository.join("product"),
            workspace_members: BTreeSet::from([
                SHIPPING_ID.to_owned(),
                CONSUMER_ID.to_owned(),
            ]),
            packages,
            nodes,
        }
    }

    fn package(
        repository: &Path,
        closure: &crate::product_closures::Closure,
        id: &str,
    ) -> Package {
        Package {
            id: id.to_owned(),
            name: closure.package.name.clone(),
            version: "0.16.0-dev".to_owned(),
            source: None,
            checksum: None,
            license: Some("AGPL-3.0-only".to_owned()),
            manifest_path: repository.join(&closure.package.manifest),
            targets: vec![Target {
                name: closure.target.name.clone(),
                kind: vec!["lib".to_owned()],
                crate_types: vec!["lib".to_owned()],
                src_path: repository.join(&closure.target.src_path),
            }],
        }
    }

    fn shipping_request(fixture: &Fixture) -> BuildRequest<'_> {
        let closure = fixture
            .registry
            .closures
            .iter()
            .find(|closure| closure.id == "foundation-shipping")
            .expect("shipping closure");
        BuildRequest {
            closure,
            package_id: SHIPPING_ID,
            metadata: &fixture.metadata,
            repository_root: &fixture.repository,
            cargo: &fixture.executable,
            rustc: &fixture.executable,
            checker: &fixture.executable,
            sysroot: Path::new("/sysroot"),
            scratch_root: &fixture.scratch,
            cargo_home: &fixture.cargo_home,
            temp_dir: &fixture.temp_dir,
            target_dir: &fixture.target_dir,
            trace_dir: &fixture.trace_dir,
            dep_info_dir: &fixture.dep_info_dir,
            admitted_sources: &fixture.shipping_sources,
            provider: None,
        }
    }

    fn wrapper_config(fixture: &Fixture) -> WrapperConfig {
        WrapperConfig {
            rustc: fixture.executable.clone(),
            sysroot: PathBuf::from("/sysroot"),
            cargo_home: fixture.cargo_home.clone(),
            temp_dir: fixture.temp_dir.clone(),
            build_dir: fixture.target_dir.join("build-dir"),
            repository_root: fixture.repository.clone(),
            target_dir: fixture.target_dir.clone(),
            trace_dir: fixture.trace_dir.clone(),
            dep_info_dir: fixture.dep_info_dir.clone(),
            limits: WrapperLimits {
                path_bytes: LIMITS.path_bytes,
                trace_bytes: LIMITS.wrapper_trace_bytes,
                argv_items: LIMITS.wrapper_argv_items,
                argv_bytes: LIMITS.wrapper_argv_bytes,
                trace_count: LIMITS.wrapper_trace_count,
                dep_info_bytes: LIMITS.dep_info_file_bytes,
            },
        }
    }

    fn unit_arguments(fixture: &Fixture, consumer: bool) -> Vec<String> {
        let crate_name = if consumer {
            "reovim_foundation_public_consumer"
        } else {
            "reovim_foundation"
        };
        let source = if consumer {
            "tests/foundation-public-consumer/src/lib.rs"
        } else {
            "kernel/foundation/src/lib.rs"
        };
        let build_dir = fixture.scratch.join("target/build-dir");
        let out_dir = build_dir.join(TARGET_TRIPLE).join("release/deps");
        let target_search = out_dir.clone();
        let host_search = build_dir.join("release/deps");
        vec![
            fixture.executable.to_string_lossy().into_owned(),
            "--crate-name".to_owned(),
            crate_name.to_owned(),
            "--edition=2024".to_owned(),
            fixture.repository.join(source).to_string_lossy().into_owned(),
            "--error-format=json".to_owned(),
            "--json=diagnostic-rendered-ansi,artifacts,future-incompat"
                .to_owned(),
            "--crate-type".to_owned(),
            "lib".to_owned(),
            "--emit=dep-info,metadata,link".to_owned(),
            "-C".to_owned(),
            "opt-level=3".to_owned(),
            "-C".to_owned(),
            "embed-bitcode=no".to_owned(),
            "--deny=clippy::pedantic".to_owned(),
            "--deny=clippy::nursery".to_owned(),
            "--deny=clippy::all".to_owned(),
            "--deny=warnings".to_owned(),
            "--forbid=unsafe_code".to_owned(),
            "--forbid=clippy::excessive_nesting".to_owned(),
            "--cfg".to_owned(),
            "feature=\"default\"".to_owned(),
            "--check-cfg".to_owned(),
            "cfg(docsrs,test)".to_owned(),
            "--check-cfg".to_owned(),
            "cfg(feature, values(\"default\"))".to_owned(),
            "-C".to_owned(),
            "metadata=0123456789abcdef".to_owned(),
            "-C".to_owned(),
            "extra-filename=-0123456789abcdef".to_owned(),
            "--out-dir".to_owned(),
            out_dir.to_string_lossy().into_owned(),
            "--target".to_owned(),
            TARGET_TRIPLE.to_owned(),
            "-C".to_owned(),
            "strip=debuginfo".to_owned(),
            "-L".to_owned(),
            format!("dependency={}", target_search.display()),
            "-L".to_owned(),
            format!("dependency={}", host_search.display()),
        ]
    }

    fn compiler_environment(fixture: &Fixture) -> Vec<EnvironmentEntry> {
        let request = shipping_request(fixture);
        vec![
            entry(
                "CARGO_BUILD_BUILD_DIR",
                &fixture.target_dir.join("build-dir").to_string_lossy(),
            ),
            entry("CARGO_CRATE_NAME", "reovim_foundation"),
            entry("CARGO_HOME", &fixture.cargo_home.to_string_lossy()),
            entry("CARGO_INCREMENTAL", "0"),
            entry(
                "CARGO_MANIFEST_DIR",
                &fixture.repository.join("kernel/foundation").to_string_lossy(),
            ),
            entry(
                "CARGO_MANIFEST_PATH",
                &fixture
                    .repository
                    .join("kernel/foundation/Cargo.toml")
                    .to_string_lossy(),
            ),
            entry("CARGO_NET_OFFLINE", "true"),
            entry("LD_LIBRARY_PATH", "/compiler/lib"),
            entry("CARGO_PKG_NAME", "reovim-foundation"),
            entry("CARGO_PRIMARY_PACKAGE", "1"),
            entry("CARGO_TARGET_DIR", &request.target_dir.to_string_lossy()),
            entry("RUSTC", &fixture.executable.to_string_lossy()),
            entry("TEMP", &fixture.temp_dir.to_string_lossy()),
            entry("TERM", "dumb"),
            entry("TMP", &fixture.temp_dir.to_string_lossy()),
            entry("TMPDIR", &fixture.temp_dir.to_string_lossy()),
        ]
    }

    fn wrapper_environment_map(
        fixture: &Fixture,
    ) -> BTreeMap<OsString, OsString> {
        let request = shipping_request(fixture);
        let mut environment = BTreeMap::new();
        for entry in compiler_environment(fixture) {
            environment.insert(entry.name.into(), entry.value.into());
        }
        environment.insert("REOVIM_CAPTURE_WRAPPER".into(), "1".into());
        environment.insert(
            "REOVIM_CAPTURE_RUSTC".into(),
            fixture.executable.as_os_str().to_owned(),
        );
        environment.insert("REOVIM_CAPTURE_SYSROOT".into(), "/sysroot".into());
        environment.insert(
            "REOVIM_CAPTURE_CARGO_HOME".into(),
            fixture.cargo_home.as_os_str().to_owned(),
        );
        environment.insert(
            "REOVIM_CAPTURE_TEMP_DIR".into(),
            fixture.temp_dir.as_os_str().to_owned(),
        );
        environment.insert(
            "REOVIM_CAPTURE_BUILD_DIR".into(),
            fixture.target_dir.join("build-dir").into_os_string(),
        );
        environment.insert(
            "REOVIM_CAPTURE_REPOSITORY".into(),
            fixture.repository.as_os_str().to_owned(),
        );
        environment.insert(
            "REOVIM_CAPTURE_TARGET_DIR".into(),
            request.target_dir.as_os_str().to_owned(),
        );
        environment.insert(
            "REOVIM_CAPTURE_TRACE_DIR".into(),
            request.trace_dir.as_os_str().to_owned(),
        );
        environment.insert(
            "REOVIM_CAPTURE_DEP_INFO_DIR".into(),
            request.dep_info_dir.as_os_str().to_owned(),
        );
        for (name, value) in [
            ("REOVIM_CAPTURE_PATH_BYTES", LIMITS.path_bytes),
            ("REOVIM_CAPTURE_TRACE_BYTES", LIMITS.wrapper_trace_bytes),
            ("REOVIM_CAPTURE_ARGV_ITEMS", LIMITS.wrapper_argv_items),
            ("REOVIM_CAPTURE_ARGV_BYTES", LIMITS.wrapper_argv_bytes),
            ("REOVIM_CAPTURE_TRACE_COUNT", LIMITS.wrapper_trace_count),
            ("REOVIM_CAPTURE_DEP_INFO_BYTES", LIMITS.dep_info_file_bytes),
        ] {
            environment.insert(name.into(), value.to_string().into());
        }
        environment
    }

    fn entry(name: &str, value: &str) -> EnvironmentEntry {
        EnvironmentEntry { name: name.to_owned(), value: value.to_owned() }
    }

    fn to_os(arguments: &[String]) -> Vec<OsString> {
        arguments.iter().map(OsString::from).collect()
    }

    fn probe_arguments(target: bool) -> Vec<String> {
        let mut values = vec![
            "rustc".to_owned(),
            "-".to_owned(),
            "--crate-name".to_owned(),
            "___".to_owned(),
            "--print=file-names".to_owned(),
        ];
        if target {
            values.extend(["--target".to_owned(), TARGET_TRIPLE.to_owned()]);
        }
        values.extend([
            "--crate-type".to_owned(),
            "bin".to_owned(),
            "--crate-type".to_owned(),
            "rlib".to_owned(),
            "--crate-type".to_owned(),
            "dylib".to_owned(),
            "--crate-type".to_owned(),
            "cdylib".to_owned(),
            "--crate-type".to_owned(),
            "staticlib".to_owned(),
            "--crate-type".to_owned(),
            "proc-macro".to_owned(),
            "--print=sysroot".to_owned(),
            "--print=split-debuginfo".to_owned(),
            "--print=crate-name".to_owned(),
            "--print=cfg".to_owned(),
            "-Wwarnings".to_owned(),
        ]);
        values
    }

    fn traces_for_kinds(kinds: &[InvocationKind]) -> Vec<WrapperTrace> {
        kinds
            .iter()
            .copied()
            .enumerate()
            .map(|(ordinal, kind)| dummy_trace(ordinal, kind))
            .collect()
    }

    fn dummy_trace(ordinal: usize, kind: InvocationKind) -> WrapperTrace {
        WrapperTrace {
            ordinal: u64::try_from(ordinal).expect("ordinal"),
            kind,
            rustc: PathBuf::from("/compiler/bin/rustc"),
            original_argv: vec!["rustc".to_owned(), "-vV".to_owned()],
            effective_argv: vec![
                "rustc".to_owned(),
                "-vV".to_owned(),
                "--sysroot".to_owned(),
                "/sysroot".to_owned(),
            ],
            environment: Vec::new(),
            dep_info_path: None,
            status: 0,
            trace_digest: digest('0'),
            trace_bytes: 1,
        }
    }

    fn cargo_artifact_json(request: &BuildRequest<'_>, fresh: bool) -> Value {
        let target = request.target_dir;
        let manifest =
            request.repository_root.join(&request.closure.package.manifest);
        let source =
            request.repository_root.join(&request.closure.target.src_path);
        let rmeta = target.join(concat!(
            "build-dir/x86_64-unknown-linux-gnu/release/deps/",
            "libreovim_foundation-hash.rmeta"
        ));
        json!({
            "reason": "compiler-artifact",
            "package_id": request.package_id,
            "manifest_path": manifest,
            "target": {
                "kind": ["lib"],
                "crate_types": ["lib"],
                "name": request.closure.target.name,
                "src_path": source,
                "edition": "2024",
                "doc": true,
                "doctest": false,
                "test": false,
            },
            "profile": {
                "opt_level": "3",
                "debuginfo": 0,
                "debug_assertions": false,
                "overflow_checks": false,
                "test": false,
            },
            "features": ["default"],
            "filenames": [
                target.join("release/libreovim_foundation.rlib"),
                rmeta,
            ],
            "executable": null,
            "fresh": fresh,
        })
    }

    fn cargo_stream(values: &[Value]) -> Vec<u8> {
        let mut output = Vec::new();
        for value in values {
            output.extend(
                serde_json::to_vec(value).expect("serialize Cargo JSON"),
            );
            output.push(b'\n');
        }
        output
    }

    fn trace_start_json(ordinal: u64) -> Vec<u8> {
        json_line(&json!({
            "schema": "reovim-rustc-wrapper-trace/v1",
            "phase": "start",
            "ordinal": ordinal,
            "kind": "version_probe",
            "rustc": "/compiler/bin/rustc",
            "original_argv": ["/compiler/bin/rustc", "-vV"],
            "effective_argv": [
                "/compiler/bin/rustc", "-vV", "--sysroot", "/sysroot"
            ],
            "environment": [],
            "dep_info_path": null,
        }))
    }

    fn trace_finish_json(ordinal: u64, status: i32) -> Vec<u8> {
        json_line(&json!({
            "schema": "reovim-rustc-wrapper-trace/v1",
            "phase": "finish",
            "ordinal": ordinal,
            "rustc_status": status,
            "wrapper_status": status,
        }))
    }

    fn json_line(value: &Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(value).expect("serialize trace");
        bytes.push(b'\n');
        bytes
    }

    fn cargo_artifact_value(
        request: &BuildRequest<'_>,
        fresh: bool,
    ) -> CargoArtifact {
        CargoArtifact {
            package_id: request.package_id.to_owned(),
            manifest_path: request
                .repository_root
                .join(&request.closure.package.manifest),
            target_name: request.closure.target.name.clone(),
            target_kinds: vec!["lib".to_owned()],
            crate_types: vec!["lib".to_owned()],
            source_path: request
                .repository_root
                .join(&request.closure.target.src_path),
            features: vec!["default".to_owned()],
            filenames: Vec::new(),
            fresh,
        }
    }

    fn provider_artifacts(
        cargo_artifact: CargoArtifact,
        identity: &FileIdentity,
    ) -> ProviderArtifacts {
        ProviderArtifacts {
            closure: "foundation-shipping".to_owned(),
            package_id: SHIPPING_ID.to_owned(),
            unit: "foundation-shipping:rustc".to_owned(),
            cargo_artifact,
            top_level_rlib: identity.clone(),
            hashed_rlib: identity.clone(),
            rmeta: FileIdentity {
                path: identity.path.with_extension("rmeta"),
                ..identity.clone()
            },
            target_dir: identity
                .path
                .parent()
                .expect("target parent")
                .to_path_buf(),
            cargo_home: PathBuf::from("/scratch/cargo-home"),
        }
    }

    fn digest(value: char) -> String {
        format!("sha256:{}", value.to_string().repeat(64))
    }
}
