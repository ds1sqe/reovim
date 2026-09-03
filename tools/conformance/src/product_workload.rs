//! Fixed prepared-root workload for the four-stage Product producer.
//!
//! The outer checker supplies one closed, versioned request.  The workload
//! reconstructs the committed Product registry, runs the exact prepared
//! metadata/package/compiler journey, and returns the two package pairs and
//! four compiler stages needed by the next proof adapter.

use std::{
    collections::BTreeSet,
    fmt, fs,
    fs::File,
    io::Read as _,
    path::{Path, PathBuf},
    time::Duration,
};

use serde_json::{Map, Value, json};

use reovim_depgraph::{ArchitecturalRole, SourceUse};

use crate::{
    cargo,
    compiler_capture::{self, BuildObservation},
    compiler_substrate, digest, json as bounded_json, package_capture,
    prepared_root::{self, WorkloadContext},
    process::ProcessLimits,
    product_closures::{Closure, ClosureKind, ParseLimits, Registry},
    product_source,
    topology::Profile,
};

pub const REQUEST_SCHEMA: &str = "reovim-product-workload-request/v2";
pub const RESULT_SCHEMA: &str = "reovim-product-workload-result/v2";
pub const PREPARED_REGISTRY_PATH: &str =
    "/repo/conformance/product-closures.json";

const REQUEST_MAX_BYTES: usize = 4_096;

const SHIPPING_CLOSURE: &str = "foundation-shipping";
const VALIDATION_CLOSURE: &str = "foundation-consumer";
const PUBLIC_RELATION: &str = "foundation-public-api";
const KERNEL_INTERFACE_SHIPPING_CLOSURE: &str = "kernel-interface-shipping";
const KERNEL_INTERFACE_VALIDATION_CLOSURE: &str = "kernel-interface-consumer";
const KERNEL_INTERFACE_PUBLIC_RELATION: &str = "kernel-interface-public-api";
const TARGET: &str = "x86_64-unknown-linux-gnu";

const REPOSITORY: &str = "/repo";
const CARGO: &str = "/compiler/bin/cargo";
const RUSTC: &str = "/compiler/bin/rustc";
const CHECKER: &str = "/checker/reovim-conformance";
const SYSROOT: &str = "/sysroot";
const SCRATCH: &str = "/scratch";
const COMPILER_SUPPORT: &str = "/compiler/lib";
const TARGET_LIBDIR: &str =
    concat!("/sysroot/lib/rustlib/", "x86_64-unknown-linux-gnu/lib",);

const PRODUCT_MANIFEST: &str = "product/Cargo.toml";
const SHIPPING_PACKAGE: &str = "reovim-foundation";
const VALIDATION_PACKAGE: &str = "reovim-foundation-public-consumer";
const KERNEL_INTERFACE_SHIPPING_PACKAGE: &str = "reovim-kernel-interface";
const KERNEL_INTERFACE_VALIDATION_PACKAGE: &str =
    "reovim-kernel-interface-public-consumer";

const SHIPPING_SOURCES: &[&str] = &[
    "kernel/foundation/src/bound.rs",
    "kernel/foundation/src/carrier.rs",
    "kernel/foundation/src/contract.rs",
    "kernel/foundation/src/identity.rs",
    "kernel/foundation/src/lib.rs",
];

const VALIDATION_SOURCES: &[&str] = &[
    "tests/foundation-public-consumer/src/lib.rs",
    "tests/foundation-public-consumer/src/public_items.rs",
];

const KERNEL_INTERFACE_SHIPPING_SOURCES: &[&str] = &[
    "kernel/interface/src/bound.rs",
    "kernel/interface/src/cancellation.rs",
    "kernel/interface/src/carrier.rs",
    "kernel/interface/src/clock.rs",
    "kernel/interface/src/contract.rs",
    "kernel/interface/src/diagnostic.rs",
    "kernel/interface/src/event.rs",
    "kernel/interface/src/event/admission.rs",
    "kernel/interface/src/event/canonical.rs",
    "kernel/interface/src/event/canonical/grammar.rs",
    "kernel/interface/src/event/counts.rs",
    "kernel/interface/src/event/cursor.rs",
    "kernel/interface/src/event/descriptor.rs",
    "kernel/interface/src/event/identity.rs",
    "kernel/interface/src/event/limits.rs",
    "kernel/interface/src/event/proposal.rs",
    "kernel/interface/src/event/record.rs",
    "kernel/interface/src/event/remediation.rs",
    "kernel/interface/src/event/sequence.rs",
    "kernel/interface/src/event/snapshot.rs",
    "kernel/interface/src/event/snapshot_validation.rs",
    "kernel/interface/src/identity.rs",
    "kernel/interface/src/lib.rs",
    "kernel/interface/src/lifecycle.rs",
    "kernel/interface/src/operation.rs",
];

const KERNEL_INTERFACE_VALIDATION_SOURCES: &[&str] = &[
    "tests/kernel-interface-public-consumer/src/clock.rs",
    "tests/kernel-interface-public-consumer/src/diagnostic.rs",
    "tests/kernel-interface-public-consumer/src/event/canonical.rs",
    "tests/kernel-interface-public-consumer/src/event/derives.rs",
    "tests/kernel-interface-public-consumer/src/event/descriptor.rs",
    "tests/kernel-interface-public-consumer/src/event/mod.rs",
    "tests/kernel-interface-public-consumer/src/event/outcomes.rs",
    "tests/kernel-interface-public-consumer/src/event/sequence.rs",
    "tests/kernel-interface-public-consumer/src/event/snapshot.rs",
    "tests/kernel-interface-public-consumer/src/lib.rs",
    "tests/kernel-interface-public-consumer/src/lifecycle.rs",
    "tests/kernel-interface-public-consumer/src/public_items.rs",
    "tests/kernel-interface-public-consumer/src/operation.rs",
];

/// All caller-selected bounds used inside the prepared workload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub registry: ParseLimits,
    pub metadata_process: ProcessLimits,
    pub metadata_json_depth: usize,
    pub metadata_json_items: usize,
    pub package: package_capture::Limits,
    pub compiler: compiler_capture::Limits,
    pub substrate: compiler_substrate::Limits,
    pub source: product_source::AnalysisLimits,
    pub result_bytes: usize,
    pub result_json_depth: usize,
    pub result_json_items: usize,
}

/// Raw identity of the one prepared Product-closure registry file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryInput {
    /// Exact canonical prepared-root path of the registry.
    pub path: PathBuf,
    /// SHA-256 identity of the raw registry bytes.
    pub content_digest: String,
    /// Exact length of the raw registry bytes.
    pub size_bytes: u64,
}

/// Parsed bounded request for one file-backed Product registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub registry: RegistryInput,
    pub limits: Limits,
}

struct LoadedRequest {
    registry: Registry,
    limits: Limits,
}

/// Complete raw result from the fixed four-stage prepared producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    package_ids: PackageIds,
    pub compiler_substrate: compiler_substrate::Observation,
    pub package_before: package_capture::Observation,
    pub package_after: package_capture::Observation,
    pub shipping: BuildObservation,
    pub validation: BuildObservation,
    pub kernel_interface_package_before: package_capture::Observation,
    pub kernel_interface_package_after: package_capture::Observation,
    pub kernel_interface_shipping: BuildObservation,
    pub kernel_interface_validation: BuildObservation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageIds {
    foundation_shipping: String,
    foundation_validation: String,
    kernel_interface_shipping: String,
    kernel_interface_validation: String,
}

/// Typed protocol, setup, execution, or semantic failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: &'static str,
    pub subject: Option<String>,
    pub message: String,
}

impl Error {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, subject: None, message: message.into() }
    }

    fn at(
        code: &'static str,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self { code, subject: Some(subject.into()), message: message.into() }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(subject) = &self.subject {
            return write!(
                formatter,
                "{}: {}: {}",
                self.code, subject, self.message
            );
        }
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

impl From<Error> for prepared_root::Error {
    fn from(failure: Error) -> Self {
        Self {
            code: failure.code,
            subject: failure.subject,
            message: failure.message,
        }
    }
}

/// Constructs the closed prepared-workload request value.
///
/// # Errors
///
/// Returns a typed error when the limits or raw registry identity are invalid.
pub fn request_value(
    registry: &RegistryInput,
    limits: &Limits,
) -> Result<Value, Error> {
    validate_limits(limits)?;
    validate_registry_input(registry, limits)?;
    let value = json!({
        "schema": REQUEST_SCHEMA,
        "registry": registry_input_value(registry)?,
        "limits": limits_value(limits)?,
    });
    validate_request_size(&value)?;
    Ok(value)
}

/// Parses the closed file-backed registry request.
///
/// # Errors
///
/// Returns a typed error for any missing, extra, malformed, unbounded, or
/// unsupported request field.
pub fn parse_request(value: &Value) -> Result<Request, Error> {
    validate_request_size(value)?;
    let object = object(value, "$", &["schema", "registry", "limits"])?;
    exact_text(field(object, "schema")?, "$.schema", REQUEST_SCHEMA)?;
    let limits = parse_limits(field(object, "limits")?)?;
    validate_limits(&limits)?;
    let registry = parse_registry_input(field(object, "registry")?)?;
    validate_registry_input(&registry, &limits)?;
    Ok(Request { registry, limits })
}

fn registry_input_value(input: &RegistryInput) -> Result<Value, Error> {
    Ok(json!({
        "path": path_value(&input.path)?,
        "content_digest": input.content_digest,
        "size_bytes": input.size_bytes,
    }))
}

fn parse_registry_input(value: &Value) -> Result<RegistryInput, Error> {
    let path = "$.registry";
    let object =
        object(value, path, &["path", "content_digest", "size_bytes"])?;
    Ok(RegistryInput {
        path: path_field(object, "path", path)?,
        content_digest: digest_field(object, "content_digest", path)?,
        size_bytes: u64_field(object, "size_bytes", path)?,
    })
}

fn validate_registry_input(
    input: &RegistryInput,
    limits: &Limits,
) -> Result<(), Error> {
    let exact_path = input.path == Path::new(PREPARED_REGISTRY_PATH);
    let bounded_size = input.size_bytes > 0
        && input.size_bytes <= limits.source.source_file_bytes;
    if exact_path && bounded_size && valid_digest(&input.content_digest) {
        return Ok(());
    }
    Err(Error::at(
        "PRODUCT_WORKLOAD_REGISTRY_INPUT",
        input.path.display().to_string(),
        "registry path, size, or digest is outside the exact profile",
    ))
}

fn validate_request_size(value: &Value) -> Result<(), Error> {
    let bytes = serde_json::to_vec(value).map_err(|failure| {
        Error::new("PRODUCT_WORKLOAD_REQUEST_ENCODE", failure.to_string())
    })?;
    if bytes.len() <= REQUEST_MAX_BYTES {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_REQUEST_LIMIT",
        format!("workload request exceeds {REQUEST_MAX_BYTES} bytes"),
    ))
}

fn load_request(request: &Request) -> Result<LoadedRequest, Error> {
    let registry = read_registry_file(&request.registry, &request.limits)?;
    Ok(LoadedRequest { registry, limits: request.limits })
}

fn read_registry_file(
    input: &RegistryInput,
    limits: &Limits,
) -> Result<Registry, Error> {
    let metadata = fs::symlink_metadata(&input.path).map_err(|failure| {
        Error::at(
            "PRODUCT_WORKLOAD_REGISTRY_METADATA",
            input.path.display().to_string(),
            failure.to_string(),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::at(
            "PRODUCT_WORKLOAD_REGISTRY_KIND",
            input.path.display().to_string(),
            "registry is not a regular non-symlink file",
        ));
    }
    let canonical = fs::canonicalize(&input.path).map_err(|failure| {
        Error::at(
            "PRODUCT_WORKLOAD_REGISTRY_CANONICAL",
            input.path.display().to_string(),
            failure.to_string(),
        )
    })?;
    if canonical != input.path {
        return Err(Error::at(
            "PRODUCT_WORKLOAD_REGISTRY_CANONICAL",
            input.path.display().to_string(),
            "registry path differs from its canonical identity",
        ));
    }
    if metadata.len() != input.size_bytes {
        return Err(Error::at(
            "PRODUCT_WORKLOAD_REGISTRY_SIZE",
            input.path.display().to_string(),
            "registry size differs from its raw identity",
        ));
    }
    let read_limit = input.size_bytes.checked_add(1).ok_or_else(|| {
        Error::new(
            "PRODUCT_WORKLOAD_REGISTRY_LIMIT",
            "registry read bound overflow",
        )
    })?;
    if read_limit > limits.source.source_file_bytes.saturating_add(1) {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_REGISTRY_LIMIT",
            "registry read exceeds the existing source-file bound",
        ));
    }
    let file = File::open(&input.path).map_err(|failure| {
        Error::at(
            "PRODUCT_WORKLOAD_REGISTRY_OPEN",
            input.path.display().to_string(),
            failure.to_string(),
        )
    })?;
    let mut bytes = Vec::new();
    file.take(read_limit).read_to_end(&mut bytes).map_err(|failure| {
        Error::at(
            "PRODUCT_WORKLOAD_REGISTRY_READ",
            input.path.display().to_string(),
            failure.to_string(),
        )
    })?;
    if u64::try_from(bytes.len()).ok() != Some(input.size_bytes) {
        return Err(Error::at(
            "PRODUCT_WORKLOAD_REGISTRY_SIZE",
            input.path.display().to_string(),
            "bounded registry read is shorter or longer than expected",
        ));
    }
    if digest::bytes(&bytes) != input.content_digest {
        return Err(Error::at(
            "PRODUCT_WORKLOAD_REGISTRY_DIGEST",
            input.path.display().to_string(),
            "registry bytes differ from their raw digest",
        ));
    }
    let value = bounded_json::parse(
        &bytes,
        limits.result_json_depth,
        limits.result_json_items,
    )
    .map_err(config_error)?;
    let registry =
        Registry::parse(&value, limits.registry).map_err(config_error)?;
    validate_registry(&registry)?;
    Ok(registry)
}

/// Executes the fixed four-stage Product journey inside the prepared root.
///
/// # Errors
///
/// Returns a prepared-root error after closing every local typed failure.
pub fn run(context: &WorkloadContext) -> Result<Value, prepared_root::Error> {
    run_inner(context).map_err(Into::into)
}

fn run_inner(context: &WorkloadContext) -> Result<Value, Error> {
    validate_context(context)?;
    let request = parse_request(&context.workload_request)?;
    let request = load_request(&request)?;
    let paths = WorkloadPaths::create(Path::new(SCRATCH))?;
    let observation = execute_journey(&request, &paths)?;
    encode_observation(&observation, &request)
}

fn execute_journey(
    request: &LoadedRequest,
    paths: &WorkloadPaths,
) -> Result<Observation, Error> {
    let metadata = capture_metadata(request, paths)?;
    validate_metadata(&metadata.metadata, &request.registry)?;
    let package_ids =
        observed_package_ids(&metadata.metadata, &request.registry)?;
    let substrate_request = substrate_request();
    let substrate_before = compiler_substrate::observe(
        substrate_request,
        request.limits.substrate,
    )
    .map_err(substrate_error)?;
    let shipping = closure(&request.registry, SHIPPING_CLOSURE)?;
    let kernel_shipping =
        closure(&request.registry, KERNEL_INTERFACE_SHIPPING_CLOSURE)?;
    let package_before = capture_package(
        paths,
        shipping,
        package_capture::Phase::BeforeCompiler,
        request.limits.package,
    )?;
    let kernel_interface_package_before = capture_package(
        paths,
        kernel_shipping,
        package_capture::Phase::BeforeCompiler,
        request.limits.package,
    )?;
    let stages = capture_four_stages(request, paths, &metadata.metadata)?;
    let package_after = capture_package(
        paths,
        shipping,
        package_capture::Phase::AfterCompiler,
        request.limits.package,
    )?;
    let kernel_interface_package_after = capture_package(
        paths,
        kernel_shipping,
        package_capture::Phase::AfterCompiler,
        request.limits.package,
    )?;
    package_capture::compare(&package_before, &package_after)
        .map_err(package_error)?;
    package_capture::compare(
        &kernel_interface_package_before,
        &kernel_interface_package_after,
    )
    .map_err(package_error)?;
    let substrate_after = compiler_substrate::observe(
        substrate_request,
        request.limits.substrate,
    )
    .map_err(substrate_error)?;
    compiler_substrate::require_unchanged(&substrate_before, &substrate_after)
        .map_err(substrate_error)?;
    Ok(Observation {
        package_ids,
        compiler_substrate: substrate_before,
        package_before,
        package_after,
        shipping: stages.foundation_shipping,
        validation: stages.foundation_validation,
        kernel_interface_package_before,
        kernel_interface_package_after,
        kernel_interface_shipping: stages.kernel_interface_shipping,
        kernel_interface_validation: stages.kernel_interface_validation,
    })
}

fn observed_package_ids(
    metadata: &cargo::Metadata,
    registry: &Registry,
) -> Result<PackageIds, Error> {
    Ok(PackageIds {
        foundation_shipping: package_id(
            metadata,
            closure(registry, SHIPPING_CLOSURE)?,
        )?
        .to_owned(),
        foundation_validation: package_id(
            metadata,
            closure(registry, VALIDATION_CLOSURE)?,
        )?
        .to_owned(),
        kernel_interface_shipping: package_id(
            metadata,
            closure(registry, KERNEL_INTERFACE_SHIPPING_CLOSURE)?,
        )?
        .to_owned(),
        kernel_interface_validation: package_id(
            metadata,
            closure(registry, KERNEL_INTERFACE_VALIDATION_CLOSURE)?,
        )?
        .to_owned(),
    })
}

struct StageObservations {
    foundation_shipping: BuildObservation,
    foundation_validation: BuildObservation,
    kernel_interface_shipping: BuildObservation,
    kernel_interface_validation: BuildObservation,
}

#[derive(Clone, Copy)]
struct StageProviders<'a> {
    direct: Option<&'a compiler_capture::ProviderArtifacts>,
    transitive: Option<&'a compiler_capture::ProviderArtifacts>,
}

fn capture_four_stages(
    request: &LoadedRequest,
    paths: &WorkloadPaths,
    metadata: &cargo::Metadata,
) -> Result<StageObservations, Error> {
    let shipping = closure(&request.registry, SHIPPING_CLOSURE)?;
    let validation = closure(&request.registry, VALIDATION_CLOSURE)?;
    let kernel_shipping =
        closure(&request.registry, KERNEL_INTERFACE_SHIPPING_CLOSURE)?;
    let kernel_validation =
        closure(&request.registry, KERNEL_INTERFACE_VALIDATION_CLOSURE)?;
    let foundation_shipping = capture_stage(
        request,
        paths,
        metadata,
        shipping,
        SHIPPING_SOURCES,
        StageProviders { direct: None, transitive: None },
    )?;
    let foundation_provider = provider(&foundation_shipping)?;
    let foundation_validation = capture_stage(
        request,
        paths,
        metadata,
        validation,
        VALIDATION_SOURCES,
        StageProviders { direct: Some(&foundation_provider), transitive: None },
    )?;
    let kernel_interface_shipping = capture_stage(
        request,
        paths,
        metadata,
        kernel_shipping,
        KERNEL_INTERFACE_SHIPPING_SOURCES,
        StageProviders { direct: Some(&foundation_provider), transitive: None },
    )?;
    let kernel_provider = provider(&kernel_interface_shipping)?;
    let kernel_interface_validation = capture_stage(
        request,
        paths,
        metadata,
        kernel_validation,
        KERNEL_INTERFACE_VALIDATION_SOURCES,
        StageProviders {
            direct: Some(&kernel_provider),
            transitive: Some(&foundation_provider),
        },
    )?;
    Ok(StageObservations {
        foundation_shipping,
        foundation_validation,
        kernel_interface_shipping,
        kernel_interface_validation,
    })
}

fn capture_stage(
    request: &LoadedRequest,
    paths: &WorkloadPaths,
    metadata: &cargo::Metadata,
    closure: &Closure,
    sources: &[&str],
    providers: StageProviders<'_>,
) -> Result<BuildObservation, Error> {
    let package_id = package_id(metadata, closure)?;
    let admitted_sources = source_set(sources);
    compiler_capture::run(
        &compiler_request(
            paths,
            closure,
            package_id,
            metadata,
            &admitted_sources,
            providers.direct,
            providers.transitive,
        )?,
        request.limits.compiler,
    )
    .map(|capture| capture.observation)
    .map_err(compiler_error)
}

fn provider(
    observation: &BuildObservation,
) -> Result<compiler_capture::ProviderArtifacts, Error> {
    observation.provider.clone().ok_or_else(|| {
        Error::new(
            "PRODUCT_WORKLOAD_PROVIDER",
            format!(
                "shipping capture {} omitted provider",
                observation.closure
            ),
        )
    })
}

fn capture_package(
    paths: &WorkloadPaths,
    shipping: &Closure,
    phase: package_capture::Phase,
    limits: package_capture::Limits,
) -> Result<package_capture::Observation, Error> {
    package_capture::run(package_request(paths, shipping, phase), limits)
        .map(|capture| capture.observation)
        .map_err(package_error)
}

fn capture_metadata(
    request: &LoadedRequest,
    paths: &WorkloadPaths,
) -> Result<cargo::Capture, Error> {
    let profile = metadata_profile();
    cargo::metadata_with_library_path(
        Path::new(CARGO),
        Path::new(RUSTC),
        Path::new(REPOSITORY),
        &paths.cargo_home,
        Path::new(PRODUCT_MANIFEST),
        &profile,
        Some(Path::new(CHECKER)),
        Some(Path::new(COMPILER_SUPPORT)),
        request.limits.metadata_process,
        request.limits.metadata_json_depth,
        request.limits.metadata_json_items,
    )
    .map_err(cargo_error)
}

fn substrate_request() -> compiler_substrate::Request<'static> {
    compiler_substrate::Request {
        compiler_support_root: Path::new(COMPILER_SUPPORT),
        target_libdir: Path::new(TARGET_LIBDIR),
    }
}

fn encode_observation(
    observation: &Observation,
    request: &LoadedRequest,
) -> Result<Value, Error> {
    validate_observation(observation, &request.limits, &request.registry)?;
    let value = observation_value(observation)?;
    bound_result(&value, &request.limits)?;
    let parsed = parse_result(&value, &request.limits, &request.registry)?;
    if parsed != *observation {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_RESULT_ROUNDTRIP",
            "workload result changed across its typed protocol",
        ));
    }
    Ok(value)
}

fn metadata_profile() -> Profile {
    Profile {
        name: "release".to_owned(),
        target: TARGET.to_owned(),
        all_features: false,
        no_default_features: false,
        features: Vec::new(),
    }
}

struct WorkloadPaths {
    cargo_home: PathBuf,
    temp_dir: PathBuf,
    package_target: PathBuf,
    build_target: PathBuf,
    shipping_trace: PathBuf,
    shipping_dep_info: PathBuf,
    validation_trace: PathBuf,
    validation_dep_info: PathBuf,
    kernel_interface_shipping_trace: PathBuf,
    kernel_interface_shipping_dep_info: PathBuf,
    kernel_interface_validation_trace: PathBuf,
    kernel_interface_validation_dep_info: PathBuf,
}

impl WorkloadPaths {
    fn create(root: &Path) -> Result<Self, Error> {
        canonical_empty_directory(root)?;
        let paths = Self {
            cargo_home: root.join("cargo-home"),
            temp_dir: root.join("temp"),
            package_target: root.join("package-target"),
            build_target: root.join("build-target"),
            shipping_trace: root.join("shipping-trace"),
            shipping_dep_info: root.join("shipping-dep-info"),
            validation_trace: root.join("validation-trace"),
            validation_dep_info: root.join("validation-dep-info"),
            kernel_interface_shipping_trace: root
                .join("kernel-interface-shipping-trace"),
            kernel_interface_shipping_dep_info: root
                .join("kernel-interface-shipping-dep-info"),
            kernel_interface_validation_trace: root
                .join("kernel-interface-validation-trace"),
            kernel_interface_validation_dep_info: root
                .join("kernel-interface-validation-dep-info"),
        };
        for path in paths.all() {
            create_scratch_directory(path)?;
        }
        Ok(paths)
    }

    fn all(&self) -> [&Path; 12] {
        [
            &self.cargo_home,
            &self.temp_dir,
            &self.package_target,
            &self.build_target,
            &self.shipping_trace,
            &self.shipping_dep_info,
            &self.validation_trace,
            &self.validation_dep_info,
            &self.kernel_interface_shipping_trace,
            &self.kernel_interface_shipping_dep_info,
            &self.kernel_interface_validation_trace,
            &self.kernel_interface_validation_dep_info,
        ]
    }
}

fn create_scratch_directory(path: &Path) -> Result<(), Error> {
    fs::create_dir(path).map_err(|failure| {
        Error::at(
            "PRODUCT_WORKLOAD_SCRATCH_CREATE",
            path.display().to_string(),
            failure.to_string(),
        )
    })?;
    canonical_empty_directory(path)
}

fn package_request<'a>(
    paths: &'a WorkloadPaths,
    shipping: &'a Closure,
    phase: package_capture::Phase,
) -> package_capture::Request<'a> {
    package_capture::Request {
        cargo: Path::new(CARGO),
        rustc: Path::new(RUSTC),
        rustc_launcher: Path::new(CHECKER),
        cargo_home: &paths.cargo_home,
        cargo_target_dir: &paths.package_target,
        temp_dir: &paths.temp_dir,
        manifest: Path::new("/repo/product/Cargo.toml"),
        package: &shipping.package.name,
        phase,
        dirty_mode: package_capture::DirtyMode::Reject,
    }
}

fn limits_value(limits: &Limits) -> Result<Value, Error> {
    Ok(json!({
        "registry": {
            "path_bytes": limits.registry.path_bytes,
            "closure_items": limits.registry.closure_items,
            "package_members": limits.registry.package_members,
            "public_items": limits.registry.public_items,
        },
        "metadata": {
            "process": process_limits_value(limits.metadata_process)?,
            "json_depth": limits.metadata_json_depth,
            "json_items": limits.metadata_json_items,
        },
        "package": {
            "process": process_limits_value(limits.package.process)?,
            "output_bytes": limits.package.output_bytes,
            "member_count": limits.package.member_count,
            "path_bytes": limits.package.path_bytes,
        },
        "compiler": {
            "process": process_limits_value(limits.compiler.process)?,
            "path_bytes": limits.compiler.path_bytes,
            "cargo_json_bytes": limits.compiler.cargo_json_bytes,
            "cargo_message_count": limits.compiler.cargo_message_count,
            "wrapper_trace_bytes": limits.compiler.wrapper_trace_bytes,
            "wrapper_argv_items": limits.compiler.wrapper_argv_items,
            "wrapper_argv_bytes": limits.compiler.wrapper_argv_bytes,
            "wrapper_trace_count": limits.compiler.wrapper_trace_count,
            "dep_info_file_bytes": limits.compiler.dep_info_file_bytes,
            "dep_info_total_bytes": limits.compiler.dep_info_total_bytes,
            "compiler_input_count": limits.compiler.compiler_input_count,
            "artifact_count": limits.compiler.artifact_count,
            "artifact_total_bytes": limits.compiler.artifact_total_bytes,
            "json_depth": limits.compiler.json_depth,
            "json_items": limits.compiler.json_items,
        },
        "substrate": {
            "path_bytes": limits.substrate.path_bytes,
            "entry_count": limits.substrate.entry_count,
            "total_bytes": limits.substrate.total_bytes,
        },
        "source": {
            "source_files": limits.source.source_files,
            "source_file_bytes": limits.source.source_file_bytes,
            "source_total_bytes": limits.source.source_total_bytes,
            "syntax_nodes": limits.source.syntax_nodes,
        },
        "result": {
            "bytes": limits.result_bytes,
            "json_depth": limits.result_json_depth,
            "json_items": limits.result_json_items,
        },
    }))
}

fn process_limits_value(limits: ProcessLimits) -> Result<Value, Error> {
    let deadline_ms = duration_millis(limits.deadline)?;
    let supervision_ms = duration_millis(limits.supervision_deadline)?;
    Ok(json!({
        "stdout_bytes": limits.stdout_bytes,
        "stderr_bytes": limits.stderr_bytes,
        "deadline_ms": deadline_ms,
        "supervision_deadline_ms": supervision_ms,
    }))
}

fn duration_millis(value: Duration) -> Result<u64, Error> {
    u64::try_from(value.as_millis()).map_err(|_| {
        Error::new(
            "PRODUCT_WORKLOAD_DURATION",
            "process duration milliseconds do not fit u64",
        )
    })
}

fn parse_limits(value: &Value) -> Result<Limits, Error> {
    let object = object(
        value,
        "$.limits",
        &[
            "registry",
            "metadata",
            "package",
            "compiler",
            "substrate",
            "source",
            "result",
        ],
    )?;
    let metadata = parse_metadata_process(field(object, "metadata")?)?;
    let result = parse_result_limits(field(object, "result")?)?;
    Ok(Limits {
        registry: parse_registry_limits(field(object, "registry")?)?,
        metadata_process: metadata.0,
        metadata_json_depth: metadata.1,
        metadata_json_items: metadata.2,
        package: parse_package_limits(field(object, "package")?)?,
        compiler: parse_compiler_limits(field(object, "compiler")?)?,
        substrate: parse_substrate_limits(field(object, "substrate")?)?,
        source: parse_source_limits(field(object, "source")?)?,
        result_bytes: result.0,
        result_json_depth: result.1,
        result_json_items: result.2,
    })
}

fn parse_registry_limits(value: &Value) -> Result<ParseLimits, Error> {
    let path = "$.limits.registry";
    let object = object(
        value,
        path,
        &["path_bytes", "closure_items", "package_members", "public_items"],
    )?;
    Ok(ParseLimits {
        path_bytes: usize_field(object, "path_bytes", path)?,
        closure_items: usize_field(object, "closure_items", path)?,
        package_members: usize_field(object, "package_members", path)?,
        public_items: usize_field(object, "public_items", path)?,
    })
}

fn parse_metadata_process(
    value: &Value,
) -> Result<(ProcessLimits, usize, usize), Error> {
    let path = "$.limits.metadata";
    let object = object(value, path, &["process", "json_depth", "json_items"])?;
    Ok((
        parse_process_limits(field(object, "process")?, path)?,
        usize_field(object, "json_depth", path)?,
        usize_field(object, "json_items", path)?,
    ))
}

fn parse_package_limits(
    value: &Value,
) -> Result<package_capture::Limits, Error> {
    let path = "$.limits.package";
    let object = object(
        value,
        path,
        &["process", "output_bytes", "member_count", "path_bytes"],
    )?;
    Ok(package_capture::Limits {
        process: parse_process_limits(field(object, "process")?, path)?,
        output_bytes: usize_field(object, "output_bytes", path)?,
        member_count: usize_field(object, "member_count", path)?,
        path_bytes: usize_field(object, "path_bytes", path)?,
    })
}

fn parse_compiler_limits(
    value: &Value,
) -> Result<compiler_capture::Limits, Error> {
    let path = "$.limits.compiler";
    let object = object(
        value,
        path,
        &[
            "process",
            "path_bytes",
            "cargo_json_bytes",
            "cargo_message_count",
            "wrapper_trace_bytes",
            "wrapper_argv_items",
            "wrapper_argv_bytes",
            "wrapper_trace_count",
            "dep_info_file_bytes",
            "dep_info_total_bytes",
            "compiler_input_count",
            "artifact_count",
            "artifact_total_bytes",
            "json_depth",
            "json_items",
        ],
    )?;
    Ok(compiler_capture::Limits {
        process: parse_process_limits(field(object, "process")?, path)?,
        path_bytes: usize_field(object, "path_bytes", path)?,
        cargo_json_bytes: usize_field(object, "cargo_json_bytes", path)?,
        cargo_message_count: usize_field(object, "cargo_message_count", path)?,
        wrapper_trace_bytes: usize_field(object, "wrapper_trace_bytes", path)?,
        wrapper_argv_items: usize_field(object, "wrapper_argv_items", path)?,
        wrapper_argv_bytes: usize_field(object, "wrapper_argv_bytes", path)?,
        wrapper_trace_count: usize_field(object, "wrapper_trace_count", path)?,
        dep_info_file_bytes: usize_field(object, "dep_info_file_bytes", path)?,
        dep_info_total_bytes: usize_field(
            object,
            "dep_info_total_bytes",
            path,
        )?,
        compiler_input_count: usize_field(
            object,
            "compiler_input_count",
            path,
        )?,
        artifact_count: usize_field(object, "artifact_count", path)?,
        artifact_total_bytes: u64_field(object, "artifact_total_bytes", path)?,
        json_depth: usize_field(object, "json_depth", path)?,
        json_items: usize_field(object, "json_items", path)?,
    })
}

fn parse_substrate_limits(
    value: &Value,
) -> Result<compiler_substrate::Limits, Error> {
    let path = "$.limits.substrate";
    let object =
        object(value, path, &["path_bytes", "entry_count", "total_bytes"])?;
    Ok(compiler_substrate::Limits {
        path_bytes: usize_field(object, "path_bytes", path)?,
        entry_count: usize_field(object, "entry_count", path)?,
        total_bytes: u64_field(object, "total_bytes", path)?,
    })
}

fn parse_source_limits(
    value: &Value,
) -> Result<product_source::AnalysisLimits, Error> {
    let path = "$.limits.source";
    let object = object(
        value,
        path,
        &[
            "source_files",
            "source_file_bytes",
            "source_total_bytes",
            "syntax_nodes",
        ],
    )?;
    Ok(product_source::AnalysisLimits {
        source_files: usize_field(object, "source_files", path)?,
        source_file_bytes: u64_field(object, "source_file_bytes", path)?,
        source_total_bytes: u64_field(object, "source_total_bytes", path)?,
        syntax_nodes: usize_field(object, "syntax_nodes", path)?,
    })
}

fn parse_result_limits(value: &Value) -> Result<(usize, usize, usize), Error> {
    let path = "$.limits.result";
    let object = object(value, path, &["bytes", "json_depth", "json_items"])?;
    Ok((
        usize_field(object, "bytes", path)?,
        usize_field(object, "json_depth", path)?,
        usize_field(object, "json_items", path)?,
    ))
}

fn parse_process_limits(
    value: &Value,
    parent: &str,
) -> Result<ProcessLimits, Error> {
    let path = format!("{parent}.process");
    let object = object(
        value,
        &path,
        &[
            "stdout_bytes",
            "stderr_bytes",
            "deadline_ms",
            "supervision_deadline_ms",
        ],
    )?;
    Ok(ProcessLimits {
        stdout_bytes: usize_field(object, "stdout_bytes", &path)?,
        stderr_bytes: usize_field(object, "stderr_bytes", &path)?,
        deadline: Duration::from_millis(u64_field(
            object,
            "deadline_ms",
            &path,
        )?),
        supervision_deadline: Duration::from_millis(u64_field(
            object,
            "supervision_deadline_ms",
            &path,
        )?),
    })
}

fn validate_limits(limits: &Limits) -> Result<(), Error> {
    let positive = [
        limits.registry.path_bytes,
        limits.registry.closure_items,
        limits.registry.package_members,
        limits.registry.public_items,
        limits.metadata_json_depth,
        limits.metadata_json_items,
        limits.package.output_bytes,
        limits.package.member_count,
        limits.package.path_bytes,
        limits.compiler.path_bytes,
        limits.compiler.cargo_json_bytes,
        limits.compiler.cargo_message_count,
        limits.compiler.wrapper_trace_bytes,
        limits.compiler.wrapper_argv_items,
        limits.compiler.wrapper_argv_bytes,
        limits.compiler.wrapper_trace_count,
        limits.compiler.dep_info_file_bytes,
        limits.compiler.dep_info_total_bytes,
        limits.compiler.compiler_input_count,
        limits.compiler.artifact_count,
        limits.compiler.json_depth,
        limits.compiler.json_items,
        limits.substrate.path_bytes,
        limits.substrate.entry_count,
        limits.source.source_files,
        limits.source.syntax_nodes,
        limits.result_bytes,
        limits.result_json_depth,
        limits.result_json_items,
    ];
    let processes = [
        limits.metadata_process,
        limits.package.process,
        limits.compiler.process,
    ];
    let process_valid = processes.iter().all(|process| {
        process.stdout_bytes > 0
            && process.stderr_bytes > 0
            && !process.deadline.is_zero()
            && process.supervision_deadline > process.deadline
    });
    let coherent = limits.package.process.stdout_bytes
        == limits.package.output_bytes
        && limits.compiler.process.stdout_bytes
            == limits.compiler.cargo_json_bytes
        && limits.compiler.dep_info_file_bytes
            <= limits.compiler.dep_info_total_bytes
        && limits.compiler.artifact_total_bytes > 0
        && limits.substrate.total_bytes > 0;
    let source_coherent = limits.source.source_file_bytes > 0
        && limits.source.source_total_bytes >= limits.source.source_file_bytes;
    if positive.into_iter().all(|value| value > 0)
        && process_valid
        && coherent
        && source_coherent
    {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_LIMITS",
        "workload limits are zero, incoherent, or unsupervised",
    ))
}

fn validate_registry(registry: &Registry) -> Result<(), Error> {
    let closure_ids = registry
        .closures
        .iter()
        .map(|closure| closure.id.as_str())
        .collect::<BTreeSet<_>>();
    let expected_closures = BTreeSet::from([
        SHIPPING_CLOSURE,
        VALIDATION_CLOSURE,
        KERNEL_INTERFACE_SHIPPING_CLOSURE,
        KERNEL_INTERFACE_VALIDATION_CLOSURE,
    ]);
    let relation_ids = registry
        .public_consumer_relations
        .iter()
        .map(|relation| relation.id.as_str())
        .collect::<BTreeSet<_>>();
    let expected_relations =
        BTreeSet::from([PUBLIC_RELATION, KERNEL_INTERFACE_PUBLIC_RELATION]);
    if closure_ids != expected_closures || relation_ids != expected_relations {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_REGISTRY_SET",
            "registry is not the exact Foundation and KI closure pairs",
        ));
    }
    for id in [
        SHIPPING_CLOSURE,
        VALIDATION_CLOSURE,
        KERNEL_INTERFACE_SHIPPING_CLOSURE,
        KERNEL_INTERFACE_VALIDATION_CLOSURE,
    ] {
        validate_closure(closure(registry, id)?)?;
    }
    validate_relation(
        registry,
        PUBLIC_RELATION,
        SHIPPING_CLOSURE,
        VALIDATION_CLOSURE,
    )?;
    validate_relation(
        registry,
        KERNEL_INTERFACE_PUBLIC_RELATION,
        KERNEL_INTERFACE_SHIPPING_CLOSURE,
        KERNEL_INTERFACE_VALIDATION_CLOSURE,
    )
}

fn validate_relation(
    registry: &Registry,
    id: &str,
    shipping: &str,
    consumer: &str,
) -> Result<(), Error> {
    let mut relations = registry
        .public_consumer_relations
        .iter()
        .filter(|relation| relation.id == id);
    let relation = relations.next().ok_or_else(|| {
        Error::new(
            "PRODUCT_WORKLOAD_RELATION",
            format!("relation {id} is absent"),
        )
    })?;
    let exact = relations.next().is_none()
        && relation.shipping_closure == shipping
        && relation.consumer_closure == consumer;
    if !exact {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_RELATION",
            format!("relation {id} differs from the exact v2 profile"),
        ));
    }
    Ok(())
}

fn validate_closure(closure: &Closure) -> Result<(), Error> {
    let expected = match closure.id.as_str() {
        SHIPPING_CLOSURE => (
            SHIPPING_CLOSURE,
            ClosureKind::Shipping,
            "foundation",
            SHIPPING_PACKAGE,
            "kernel/foundation/Cargo.toml",
            "reovim_foundation",
            "kernel/foundation/src/lib.rs",
            ArchitecturalRole::Foundation,
            SourceUse::Production,
        ),
        VALIDATION_CLOSURE => (
            VALIDATION_CLOSURE,
            ClosureKind::Validation,
            "foundation-public-consumer",
            VALIDATION_PACKAGE,
            "tests/foundation-public-consumer/Cargo.toml",
            "reovim_foundation_public_consumer",
            "tests/foundation-public-consumer/src/lib.rs",
            ArchitecturalRole::KernelInterface,
            SourceUse::Test,
        ),
        KERNEL_INTERFACE_SHIPPING_CLOSURE => (
            KERNEL_INTERFACE_SHIPPING_CLOSURE,
            ClosureKind::Shipping,
            "kernel-interface",
            KERNEL_INTERFACE_SHIPPING_PACKAGE,
            "kernel/interface/Cargo.toml",
            "reovim_kernel_interface",
            "kernel/interface/src/lib.rs",
            ArchitecturalRole::KernelInterface,
            SourceUse::Production,
        ),
        KERNEL_INTERFACE_VALIDATION_CLOSURE => (
            KERNEL_INTERFACE_VALIDATION_CLOSURE,
            ClosureKind::Validation,
            "kernel-interface-public-consumer",
            KERNEL_INTERFACE_VALIDATION_PACKAGE,
            "tests/kernel-interface-public-consumer/Cargo.toml",
            "reovim_kernel_interface_public_consumer",
            "tests/kernel-interface-public-consumer/src/lib.rs",
            ArchitecturalRole::KernelRuntime,
            SourceUse::Test,
        ),
        _ => {
            return Err(Error::new(
                "PRODUCT_WORKLOAD_CLOSURE",
                "closure is outside the exact four-stage profile",
            ));
        }
    };
    let exact = closure.id == expected.0
        && closure.kind == expected.1
        && closure.workspace.id == "product"
        && closure.workspace.manifest == Path::new(PRODUCT_MANIFEST)
        && closure.workspace.lockfile == Path::new("product/Cargo.lock")
        && closure.package.id == expected.2
        && closure.package.name == expected.3
        && closure.package.manifest == Path::new(expected.4)
        && closure.target.id == "library"
        && closure.target.name == expected.5
        && closure.target.src_path == Path::new(expected.6)
        && closure.target.kind == ["lib"]
        && closure.target.crate_types == ["lib"]
        && closure.architectural_role == expected.7
        && closure.source_use == expected.8
        && closure.cargo.profile == "release"
        && closure.package_membership.is_some()
            == (closure.kind == ClosureKind::Shipping);
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_CLOSURE",
        format!("closure {} differs from its fixed profile", closure.id),
    ))
}

fn closure<'a>(registry: &'a Registry, id: &str) -> Result<&'a Closure, Error> {
    let mut values = registry.closures.iter().filter(|value| value.id == id);
    let value = values.next().ok_or_else(|| {
        Error::new(
            "PRODUCT_WORKLOAD_CLOSURE_MISSING",
            format!("closure {id} is absent"),
        )
    })?;
    if values.next().is_some() {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_CLOSURE_DUPLICATE",
            format!("closure {id} is duplicated"),
        ));
    }
    Ok(value)
}

fn validate_context(context: &WorkloadContext) -> Result<(), Error> {
    if context.scratch_root != Path::new(SCRATCH)
        || context.repository_root.as_deref() != Some(Path::new(REPOSITORY))
    {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_CONTEXT",
            "prepared workload roots differ from the fixed profile",
        ));
    }
    Ok(())
}

fn canonical_empty_directory(path: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path).map_err(|failure| {
        Error::at(
            "PRODUCT_WORKLOAD_SCRATCH_METADATA",
            path.display().to_string(),
            failure.to_string(),
        )
    })?;
    let canonical = fs::canonicalize(path).map_err(|failure| {
        Error::at(
            "PRODUCT_WORKLOAD_SCRATCH_CANONICAL",
            path.display().to_string(),
            failure.to_string(),
        )
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || canonical != path
    {
        return Err(Error::at(
            "PRODUCT_WORKLOAD_SCRATCH_IDENTITY",
            path.display().to_string(),
            "scratch directory is not a canonical real directory",
        ));
    }
    let mut entries = fs::read_dir(path).map_err(|failure| {
        Error::at(
            "PRODUCT_WORKLOAD_SCRATCH_READ",
            path.display().to_string(),
            failure.to_string(),
        )
    })?;
    if entries
        .next()
        .transpose()
        .map_err(|failure| {
            Error::at(
                "PRODUCT_WORKLOAD_SCRATCH_READ",
                path.display().to_string(),
                failure.to_string(),
            )
        })?
        .is_none()
    {
        return Ok(());
    }
    Err(Error::at(
        "PRODUCT_WORKLOAD_SCRATCH_NOT_EMPTY",
        path.display().to_string(),
        "scratch directory must be fresh and empty",
    ))
}

fn validate_metadata(
    metadata: &cargo::Metadata,
    registry: &Registry,
) -> Result<(), Error> {
    if metadata.workspace_root != Path::new("/repo/product")
        || metadata.workspace_members.len() != 4
        || metadata.packages.len() != 4
        || metadata.nodes.len() != 4
        || metadata.workspace_members
            != metadata.packages.keys().cloned().collect()
        || metadata.workspace_members
            != metadata.nodes.keys().cloned().collect()
    {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_METADATA_SET",
            "metadata is not the exact four-package Product workspace",
        ));
    }
    let shipping = closure(registry, SHIPPING_CLOSURE)?;
    let validation = closure(registry, VALIDATION_CLOSURE)?;
    let kernel_shipping = closure(registry, KERNEL_INTERFACE_SHIPPING_CLOSURE)?;
    let kernel_validation =
        closure(registry, KERNEL_INTERFACE_VALIDATION_CLOSURE)?;
    let shipping_id = package_id(metadata, shipping)?;
    let validation_id = package_id(metadata, validation)?;
    let kernel_shipping_id = package_id(metadata, kernel_shipping)?;
    let kernel_validation_id = package_id(metadata, kernel_validation)?;
    if !metadata.nodes[shipping_id].dependencies.is_empty() {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_METADATA_GRAPH",
            "Foundation shipping has a Product dependency",
        ));
    }
    validate_metadata_edge(
        &metadata.nodes[validation_id],
        "reovim_foundation",
        shipping_id,
    )?;
    validate_metadata_edge(
        &metadata.nodes[kernel_shipping_id],
        "reovim_foundation",
        shipping_id,
    )?;
    validate_metadata_edge(
        &metadata.nodes[kernel_validation_id],
        "reovim_kernel_interface",
        kernel_shipping_id,
    )
}

fn validate_metadata_edge(
    node: &cargo::Node,
    name: &str,
    provider: &str,
) -> Result<(), Error> {
    let [edge] = node.dependencies.as_slice() else {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_METADATA_GRAPH",
            "Product node does not have one exact direct dependency",
        ));
    };
    let exact = edge.name == name
        && edge.package == provider
        && edge.kinds.len() == 1
        && edge.kinds[0].kind == "normal"
        && edge.kinds[0].target.is_none();
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_METADATA_GRAPH",
        "Product dependency differs from its exact direct edge",
    ))
}

fn package_id<'a>(
    metadata: &'a cargo::Metadata,
    closure: &Closure,
) -> Result<&'a str, Error> {
    let manifest = Path::new(REPOSITORY).join(&closure.package.manifest);
    let mut matches = metadata.packages.values().filter(|package| {
        package.name == closure.package.name
            && package.manifest_path == manifest
            && package.source.is_none()
            && package.checksum.is_none()
    });
    let package = matches.next().ok_or_else(|| {
        Error::new(
            "PRODUCT_WORKLOAD_METADATA_PACKAGE",
            format!("metadata package {} is absent", closure.package.name),
        )
    })?;
    if matches.next().is_some() {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_METADATA_PACKAGE",
            format!("metadata package {} is ambiguous", closure.package.name),
        ));
    }
    if package.edition != "2024" {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_METADATA_EDITION",
            format!("metadata package {} is not edition 2024", package.name),
        ));
    }
    let source = Path::new(REPOSITORY).join(&closure.target.src_path);
    let target = package.targets.iter().find(|target| {
        target.name == closure.target.name
            && target.kind == ["lib"]
            && target.crate_types == ["lib"]
            && target.src_path == source
    });
    if target.is_some()
        && exact_package_targets(package, closure)
        && metadata.workspace_members.contains(&package.id)
        && metadata.nodes.contains_key(&package.id)
    {
        return Ok(&package.id);
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_METADATA_TARGET",
        format!("metadata targets for {} differ from registry", package.name),
    ))
}

fn exact_package_targets(package: &cargo::Package, closure: &Closure) -> bool {
    if closure.id != KERNEL_INTERFACE_VALIDATION_CLOSURE {
        return package.targets.len() == 1;
    }
    if package.targets.len() != 2 {
        return false;
    }
    package.targets.iter().any(|target| {
        target.name == "kernel-interface-public-contract"
            && target.kind == ["test"]
            && target.crate_types == ["bin"]
            && target.src_path
                == Path::new(REPOSITORY)
                    .join("tests/kernel-interface-public-consumer/")
                    .join("tests/public_contract.rs")
    })
}

fn source_set(values: &[&str]) -> BTreeSet<PathBuf> {
    values.iter().map(PathBuf::from).collect()
}

fn config_error(failure: bounded_json::ConfigError) -> Error {
    Error::at(failure.code, failure.path, failure.message)
}

fn cargo_error(failure: cargo::CargoError) -> Error {
    Error::new(failure.code, failure.message)
}

fn package_error(failure: package_capture::Error) -> Error {
    Error::new(failure.code, failure.message)
}

fn compiler_error(failure: compiler_capture::Error) -> Error {
    match failure.path {
        Some(path) => {
            Error::at(failure.code, path.display().to_string(), failure.message)
        }
        None => Error::new(failure.code, failure.message),
    }
}

fn substrate_error(failure: compiler_substrate::Error) -> Error {
    match failure.path {
        Some(path) => {
            Error::at(failure.code, path.display().to_string(), failure.message)
        }
        None => Error::new(failure.code, failure.message),
    }
}

fn observation_value(observation: &Observation) -> Result<Value, Error> {
    Ok(json!({
        "schema": RESULT_SCHEMA,
        "package_ids": package_ids_value(&observation.package_ids),
        "compiler_substrate": substrate_value(
            &observation.compiler_substrate,
        )?,
        "foundation_package_before": package_value(
            &observation.package_before,
        )?,
        "foundation_package_after": package_value(
            &observation.package_after,
        )?,
        "foundation_shipping": build_value(&observation.shipping)?,
        "foundation_validation": build_value(&observation.validation)?,
        "kernel_interface_package_before": package_value(
            &observation.kernel_interface_package_before,
        )?,
        "kernel_interface_package_after": package_value(
            &observation.kernel_interface_package_after,
        )?,
        "kernel_interface_shipping": build_value(
            &observation.kernel_interface_shipping,
        )?,
        "kernel_interface_validation": build_value(
            &observation.kernel_interface_validation,
        )?,
    }))
}

fn package_ids_value(ids: &PackageIds) -> Value {
    json!({
        "foundation_shipping": ids.foundation_shipping,
        "foundation_validation": ids.foundation_validation,
        "kernel_interface_shipping": ids.kernel_interface_shipping,
        "kernel_interface_validation": ids.kernel_interface_validation,
    })
}

/// Parses and semantically closes one bounded prepared-workload result.
///
/// # Errors
///
/// Returns a typed error for malformed, extra, duplicate, over-bound, or
/// internally inconsistent evidence.
pub fn parse_result(
    value: &Value,
    limits: &Limits,
    registry: &Registry,
) -> Result<Observation, Error> {
    validate_limits(limits)?;
    validate_registry(registry)?;
    bound_result(value, limits)?;
    let object = object(
        value,
        "$",
        &[
            "schema",
            "package_ids",
            "compiler_substrate",
            "foundation_package_before",
            "foundation_package_after",
            "foundation_shipping",
            "foundation_validation",
            "kernel_interface_package_before",
            "kernel_interface_package_after",
            "kernel_interface_shipping",
            "kernel_interface_validation",
        ],
    )?;
    exact_text(field(object, "schema")?, "$.schema", RESULT_SCHEMA)?;
    let observation = Observation {
        package_ids: parse_package_ids(field(object, "package_ids")?)?,
        compiler_substrate: parse_substrate(field(
            object,
            "compiler_substrate",
        )?)?,
        package_before: parse_package(
            field(object, "foundation_package_before")?,
            "$.foundation_package_before",
        )?,
        package_after: parse_package(
            field(object, "foundation_package_after")?,
            "$.foundation_package_after",
        )?,
        shipping: parse_build(
            field(object, "foundation_shipping")?,
            "$.foundation_shipping",
        )?,
        validation: parse_build(
            field(object, "foundation_validation")?,
            "$.foundation_validation",
        )?,
        kernel_interface_package_before: parse_package(
            field(object, "kernel_interface_package_before")?,
            "$.kernel_interface_package_before",
        )?,
        kernel_interface_package_after: parse_package(
            field(object, "kernel_interface_package_after")?,
            "$.kernel_interface_package_after",
        )?,
        kernel_interface_shipping: parse_build(
            field(object, "kernel_interface_shipping")?,
            "$.kernel_interface_shipping",
        )?,
        kernel_interface_validation: parse_build(
            field(object, "kernel_interface_validation")?,
            "$.kernel_interface_validation",
        )?,
    };
    validate_observation(&observation, limits, registry)?;
    Ok(observation)
}

fn parse_package_ids(value: &Value) -> Result<PackageIds, Error> {
    let path = "$.package_ids";
    let object = object(
        value,
        path,
        &[
            "foundation_shipping",
            "foundation_validation",
            "kernel_interface_shipping",
            "kernel_interface_validation",
        ],
    )?;
    Ok(PackageIds {
        foundation_shipping: text_field(object, "foundation_shipping", path)?
            .to_owned(),
        foundation_validation: text_field(
            object,
            "foundation_validation",
            path,
        )?
        .to_owned(),
        kernel_interface_shipping: text_field(
            object,
            "kernel_interface_shipping",
            path,
        )?
        .to_owned(),
        kernel_interface_validation: text_field(
            object,
            "kernel_interface_validation",
            path,
        )?
        .to_owned(),
    })
}

fn bound_result(value: &Value, limits: &Limits) -> Result<(), Error> {
    let bytes = serde_json::to_vec(value).map_err(|failure| {
        Error::new("PRODUCT_WORKLOAD_RESULT_ENCODE", failure.to_string())
    })?;
    if bytes.len() > limits.result_bytes {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_RESULT_LIMIT",
            format!(
                "workload result is {} bytes; limit is {}",
                bytes.len(),
                limits.result_bytes
            ),
        ));
    }
    let parsed = bounded_json::parse(
        &bytes,
        limits.result_json_depth,
        limits.result_json_items,
    )
    .map_err(config_error)?;
    if parsed == *value {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_RESULT_JSON",
        "result changed across bounded JSON serialization",
    ))
}

fn substrate_value(
    observation: &compiler_substrate::Observation,
) -> Result<Value, Error> {
    let entries = observation
        .entries
        .iter()
        .map(|entry| {
            let kind = match entry.kind {
                compiler_substrate::EntryKind::CompilerSupport => {
                    "compiler_support"
                }
                compiler_substrate::EntryKind::TargetLibdir => "target_libdir",
            };
            Ok(json!({
                "kind": kind,
                "canonical_path": path_value(&entry.canonical_path)?,
                "content_digest": entry.content_digest,
                "size_bytes": entry.size_bytes,
            }))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(json!({
        "compiler_support_root": path_value(
            &observation.compiler_support_root,
        )?,
        "target_libdir": path_value(&observation.target_libdir)?,
        "entries": entries,
        "entry_count": observation.entry_count,
        "total_bytes": observation.total_bytes,
        "aggregate_digest": observation.aggregate_digest,
    }))
}

fn parse_substrate(
    value: &Value,
) -> Result<compiler_substrate::Observation, Error> {
    let path = "$.compiler_substrate";
    let object = object(
        value,
        path,
        &[
            "compiler_support_root",
            "target_libdir",
            "entries",
            "entry_count",
            "total_bytes",
            "aggregate_digest",
        ],
    )?;
    let values = array(field(object, "entries")?, &format!("{path}.entries"))?;
    let mut entries = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        entries.push(parse_substrate_entry(
            value,
            &format!("{path}.entries[{index}]"),
        )?);
    }
    Ok(compiler_substrate::Observation {
        compiler_support_root: path_field(
            object,
            "compiler_support_root",
            path,
        )?,
        target_libdir: path_field(object, "target_libdir", path)?,
        entries,
        entry_count: usize_field(object, "entry_count", path)?,
        total_bytes: u64_field(object, "total_bytes", path)?,
        aggregate_digest: digest_field(object, "aggregate_digest", path)?,
    })
}

fn parse_substrate_entry(
    value: &Value,
    path: &str,
) -> Result<compiler_substrate::Entry, Error> {
    let object = object(
        value,
        path,
        &["kind", "canonical_path", "content_digest", "size_bytes"],
    )?;
    let kind = match text_field(object, "kind", path)? {
        "compiler_support" => compiler_substrate::EntryKind::CompilerSupport,
        "target_libdir" => compiler_substrate::EntryKind::TargetLibdir,
        value => {
            return Err(Error::at(
                "PRODUCT_WORKLOAD_SUBSTRATE_KIND",
                format!("{path}.kind"),
                format!("unknown substrate kind {value:?}"),
            ));
        }
    };
    Ok(compiler_substrate::Entry {
        kind,
        canonical_path: path_field(object, "canonical_path", path)?,
        content_digest: digest_field(object, "content_digest", path)?,
        size_bytes: u64_field(object, "size_bytes", path)?,
    })
}

fn package_value(
    observation: &package_capture::Observation,
) -> Result<Value, Error> {
    let members = observation
        .members
        .iter()
        .map(|path| path_value(path.as_path()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "phase": observation.phase.as_str(),
        "dirty_mode": dirty_mode_text(observation.dirty_mode),
        "command_argv": observation.command_argv,
        "raw_output_digest": observation.raw_output_digest,
        "semantic_digest": observation.semantic_digest,
        "stdout_bytes": observation.stdout_bytes,
        "member_path_bytes": observation.member_path_bytes,
        "members": members,
    }))
}

const fn dirty_mode_text(mode: package_capture::DirtyMode) -> &'static str {
    match mode {
        package_capture::DirtyMode::Reject => "reject",
    }
}

fn parse_package(
    value: &Value,
    path: &str,
) -> Result<package_capture::Observation, Error> {
    let object = object(
        value,
        path,
        &[
            "phase",
            "dirty_mode",
            "command_argv",
            "raw_output_digest",
            "semantic_digest",
            "stdout_bytes",
            "member_path_bytes",
            "members",
        ],
    )?;
    let phase = match text_field(object, "phase", path)? {
        "before_compiler" => package_capture::Phase::BeforeCompiler,
        "after_compiler" => package_capture::Phase::AfterCompiler,
        value => {
            return Err(Error::at(
                "PRODUCT_WORKLOAD_PACKAGE_PHASE",
                format!("{path}.phase"),
                format!("unknown package phase {value:?}"),
            ));
        }
    };
    let dirty_mode = match text_field(object, "dirty_mode", path)? {
        "reject" => package_capture::DirtyMode::Reject,
        value => {
            return Err(Error::at(
                "PRODUCT_WORKLOAD_DIRTY_MODE",
                format!("{path}.dirty_mode"),
                format!("unknown dirty mode {value:?}"),
            ));
        }
    };
    Ok(package_capture::Observation {
        phase,
        dirty_mode,
        command_argv: string_array_field(object, "command_argv", path)?,
        raw_output_digest: digest_field(object, "raw_output_digest", path)?,
        semantic_digest: digest_field(object, "semantic_digest", path)?,
        stdout_bytes: u64_field(object, "stdout_bytes", path)?,
        member_path_bytes: u64_field(object, "member_path_bytes", path)?,
        members: path_array_field(object, "members", path)?,
    })
}

fn build_value(observation: &BuildObservation) -> Result<Value, Error> {
    let probes = observation
        .probes
        .iter()
        .map(trace_value)
        .collect::<Result<Vec<_>, _>>()?;
    let provider = observation
        .provider
        .as_ref()
        .map(provider_value)
        .transpose()?
        .unwrap_or(Value::Null);
    let reuse = observation
        .provider_reuse
        .as_ref()
        .map(provider_reuse_value)
        .transpose()?
        .unwrap_or(Value::Null);
    let transitive_reuse = observation
        .transitive_provider_reuse
        .as_ref()
        .map(transitive_provider_reuse_value)
        .transpose()?
        .unwrap_or(Value::Null);
    let consumer_rlib = observation
        .consumer_rlib
        .as_ref()
        .map(file_identity_value)
        .transpose()?
        .unwrap_or(Value::Null);
    Ok(json!({
        "closure": observation.closure,
        "cargo": cargo_observation_value(&observation.cargo)?,
        "probes": probes,
        "unit": compiler_unit_value(&observation.unit)?,
        "provider": provider,
        "provider_reuse": reuse,
        "transitive_provider_reuse": transitive_reuse,
        "consumer_rlib": consumer_rlib,
    }))
}

fn cargo_observation_value(
    observation: &compiler_capture::CargoObservation,
) -> Result<Value, Error> {
    let artifacts = observation
        .artifacts
        .iter()
        .map(cargo_artifact_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "command_argv": observation.command_argv,
        "raw_digest": observation.raw_digest,
        "stdout_bytes": observation.stdout_bytes,
        "message_count": observation.message_count,
        "artifacts": artifacts,
    }))
}

fn cargo_artifact_value(
    artifact: &compiler_capture::CargoArtifact,
) -> Result<Value, Error> {
    let filenames = artifact
        .filenames
        .iter()
        .map(|path| path_value(path.as_path()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "package_id": artifact.package_id,
        "manifest_path": path_value(&artifact.manifest_path)?,
        "target_name": artifact.target_name,
        "target_kinds": artifact.target_kinds,
        "crate_types": artifact.crate_types,
        "source_path": path_value(&artifact.source_path)?,
        "features": artifact.features,
        "filenames": filenames,
        "fresh": artifact.fresh,
    }))
}

fn trace_value(trace: &compiler_capture::WrapperTrace) -> Result<Value, Error> {
    let environment = trace
        .environment
        .iter()
        .map(|entry| {
            json!({
                "name": entry.name,
                "value": entry.value,
            })
        })
        .collect::<Vec<_>>();
    let dep_info_path = trace
        .dep_info_path
        .as_ref()
        .map(|path| path_value(path.as_path()))
        .transpose()?
        .unwrap_or(Value::Null);
    Ok(json!({
        "ordinal": trace.ordinal,
        "kind": invocation_kind_text(trace.kind),
        "rustc": path_value(&trace.rustc)?,
        "original_argv": trace.original_argv,
        "effective_argv": trace.effective_argv,
        "environment": environment,
        "dep_info_path": dep_info_path,
        "status": trace.status,
        "trace_digest": trace.trace_digest,
        "trace_bytes": trace.trace_bytes,
    }))
}

const fn invocation_kind_text(
    kind: compiler_capture::InvocationKind,
) -> &'static str {
    match kind {
        compiler_capture::InvocationKind::VersionProbe => "version_probe",
        compiler_capture::InvocationKind::HostProbe => "host_probe",
        compiler_capture::InvocationKind::TargetProbe => "target_probe",
        compiler_capture::InvocationKind::CompilerUnit => "compiler_unit",
    }
}

fn compiler_unit_value(
    unit: &compiler_capture::CompilerUnit,
) -> Result<Value, Error> {
    let sources = unit
        .sources
        .iter()
        .map(|path| path_value(path.as_path()))
        .collect::<Result<Vec<_>, _>>()?;
    let externs =
        unit.externs.iter().map(extern_value).collect::<Result<Vec<_>, _>>()?;
    let outputs = unit
        .outputs
        .iter()
        .map(file_identity_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "id": unit.id,
        "closure": unit.closure,
        "crate_name": unit.crate_name,
        "crate_types": unit.crate_types,
        "target_triple": unit.target_triple,
        "profile": unit.profile,
        "features": unit.features,
        "cfgs": unit.cfgs,
        "original_argv": unit.original_argv,
        "effective_argv": unit.effective_argv,
        "dep_info_path": path_value(&unit.dep_info_path)?,
        "dep_info_digest": unit.dep_info_digest,
        "dep_info_bytes": unit.dep_info_bytes,
        "dep_info_text": unit.dep_info_text,
        "sources": sources,
        "externs": externs,
        "outputs": outputs,
        "trace_digest": unit.trace_digest,
        "trace_bytes": unit.trace_bytes,
        "status": unit.status,
    }))
}

fn extern_value(edge: &compiler_capture::ExternEdge) -> Result<Value, Error> {
    Ok(json!({
        "name": edge.name,
        "path": path_value(&edge.path)?,
    }))
}

fn file_identity_value(
    identity: &compiler_capture::FileIdentity,
) -> Result<Value, Error> {
    Ok(json!({
        "path": path_value(&identity.path)?,
        "content_digest": identity.content_digest,
        "size_bytes": identity.size_bytes,
    }))
}

fn provider_value(
    provider: &compiler_capture::ProviderArtifacts,
) -> Result<Value, Error> {
    Ok(json!({
        "closure": provider.closure,
        "package_id": provider.package_id,
        "unit": provider.unit,
        "cargo_artifact": cargo_artifact_value(
            &provider.cargo_artifact,
        )?,
        "top_level_rlib": file_identity_value(
            &provider.top_level_rlib,
        )?,
        "hashed_rlib": file_identity_value(&provider.hashed_rlib)?,
        "rmeta": file_identity_value(&provider.rmeta)?,
        "target_dir": path_value(&provider.target_dir)?,
        "cargo_home": path_value(&provider.cargo_home)?,
    }))
}

fn provider_reuse_value(
    reuse: &compiler_capture::ProviderReuse,
) -> Result<Value, Error> {
    Ok(json!({
        "fresh_artifact": cargo_artifact_value(&reuse.fresh_artifact)?,
        "hashed_rlib": file_identity_value(&reuse.hashed_rlib)?,
        "rmeta": file_identity_value(&reuse.rmeta)?,
        "extern_edge": extern_value(&reuse.extern_edge)?,
    }))
}

fn transitive_provider_reuse_value(
    reuse: &compiler_capture::TransitiveProviderReuse,
) -> Result<Value, Error> {
    Ok(json!({
        "fresh_artifact": cargo_artifact_value(&reuse.fresh_artifact)?,
        "hashed_rlib": file_identity_value(&reuse.hashed_rlib)?,
        "rmeta": file_identity_value(&reuse.rmeta)?,
    }))
}

fn parse_build(value: &Value, path: &str) -> Result<BuildObservation, Error> {
    let object = object(
        value,
        path,
        &[
            "closure",
            "cargo",
            "probes",
            "unit",
            "provider",
            "provider_reuse",
            "transitive_provider_reuse",
            "consumer_rlib",
        ],
    )?;
    let probe_path = format!("{path}.probes");
    let values = array(field(object, "probes")?, &probe_path)?;
    let mut probes = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        probes.push(parse_trace(value, &format!("{probe_path}[{index}]"))?);
    }
    Ok(BuildObservation {
        closure: text_field(object, "closure", path)?.to_owned(),
        cargo: parse_cargo_observation(
            field(object, "cargo")?,
            &format!("{path}.cargo"),
        )?,
        probes,
        unit: parse_compiler_unit(
            field(object, "unit")?,
            &format!("{path}.unit"),
        )?,
        provider: parse_optional_provider(
            field(object, "provider")?,
            &format!("{path}.provider"),
        )?,
        provider_reuse: parse_optional_reuse(
            field(object, "provider_reuse")?,
            &format!("{path}.provider_reuse"),
        )?,
        transitive_provider_reuse: parse_optional_transitive_reuse(
            field(object, "transitive_provider_reuse")?,
            &format!("{path}.transitive_provider_reuse"),
        )?,
        consumer_rlib: parse_optional_file_identity(
            field(object, "consumer_rlib")?,
            &format!("{path}.consumer_rlib"),
        )?,
    })
}

fn parse_optional_file_identity(
    value: &Value,
    path: &str,
) -> Result<Option<compiler_capture::FileIdentity>, Error> {
    if value.is_null() {
        return Ok(None);
    }
    parse_file_identity(value, path).map(Some)
}

fn parse_cargo_observation(
    value: &Value,
    path: &str,
) -> Result<compiler_capture::CargoObservation, Error> {
    let object = object(
        value,
        path,
        &[
            "command_argv",
            "raw_digest",
            "stdout_bytes",
            "message_count",
            "artifacts",
        ],
    )?;
    let artifact_path = format!("{path}.artifacts");
    let values = array(field(object, "artifacts")?, &artifact_path)?;
    let mut artifacts = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        artifacts.push(parse_cargo_artifact(
            value,
            &format!("{artifact_path}[{index}]"),
        )?);
    }
    Ok(compiler_capture::CargoObservation {
        command_argv: string_array_field(object, "command_argv", path)?,
        raw_digest: digest_field(object, "raw_digest", path)?,
        stdout_bytes: u64_field(object, "stdout_bytes", path)?,
        message_count: usize_field(object, "message_count", path)?,
        artifacts,
    })
}

fn parse_cargo_artifact(
    value: &Value,
    path: &str,
) -> Result<compiler_capture::CargoArtifact, Error> {
    let object = object(
        value,
        path,
        &[
            "package_id",
            "manifest_path",
            "target_name",
            "target_kinds",
            "crate_types",
            "source_path",
            "features",
            "filenames",
            "fresh",
        ],
    )?;
    Ok(compiler_capture::CargoArtifact {
        package_id: text_field(object, "package_id", path)?.to_owned(),
        manifest_path: path_field(object, "manifest_path", path)?,
        target_name: text_field(object, "target_name", path)?.to_owned(),
        target_kinds: string_array_field(object, "target_kinds", path)?,
        crate_types: string_array_field(object, "crate_types", path)?,
        source_path: path_field(object, "source_path", path)?,
        features: string_array_field(object, "features", path)?,
        filenames: path_array_field(object, "filenames", path)?,
        fresh: bool_field(object, "fresh", path)?,
    })
}

fn parse_trace(
    value: &Value,
    path: &str,
) -> Result<compiler_capture::WrapperTrace, Error> {
    let object = object(
        value,
        path,
        &[
            "ordinal",
            "kind",
            "rustc",
            "original_argv",
            "effective_argv",
            "environment",
            "dep_info_path",
            "status",
            "trace_digest",
            "trace_bytes",
        ],
    )?;
    let environment_path = format!("{path}.environment");
    let values = array(field(object, "environment")?, &environment_path)?;
    let mut environment = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        environment.push(parse_environment(
            value,
            &format!("{environment_path}[{index}]"),
        )?);
    }
    Ok(compiler_capture::WrapperTrace {
        ordinal: u64_field(object, "ordinal", path)?,
        kind: parse_invocation_kind(text_field(object, "kind", path)?, path)?,
        rustc: path_field(object, "rustc", path)?,
        original_argv: string_array_field(object, "original_argv", path)?,
        effective_argv: string_array_field(object, "effective_argv", path)?,
        environment,
        dep_info_path: optional_path_field(object, "dep_info_path", path)?,
        status: i32_field(object, "status", path)?,
        trace_digest: digest_field(object, "trace_digest", path)?,
        trace_bytes: u64_field(object, "trace_bytes", path)?,
    })
}

fn parse_environment(
    value: &Value,
    path: &str,
) -> Result<compiler_capture::EnvironmentEntry, Error> {
    let object = object(value, path, &["name", "value"])?;
    Ok(compiler_capture::EnvironmentEntry {
        name: text_field(object, "name", path)?.to_owned(),
        value: text_field(object, "value", path)?.to_owned(),
    })
}

fn parse_invocation_kind(
    value: &str,
    path: &str,
) -> Result<compiler_capture::InvocationKind, Error> {
    match value {
        "version_probe" => Ok(compiler_capture::InvocationKind::VersionProbe),
        "host_probe" => Ok(compiler_capture::InvocationKind::HostProbe),
        "target_probe" => Ok(compiler_capture::InvocationKind::TargetProbe),
        "compiler_unit" => Ok(compiler_capture::InvocationKind::CompilerUnit),
        _ => Err(Error::at(
            "PRODUCT_WORKLOAD_TRACE_KIND",
            format!("{path}.kind"),
            format!("unknown wrapper invocation kind {value:?}"),
        )),
    }
}

fn parse_compiler_unit(
    value: &Value,
    path: &str,
) -> Result<compiler_capture::CompilerUnit, Error> {
    let object = object(
        value,
        path,
        &[
            "id",
            "closure",
            "crate_name",
            "crate_types",
            "target_triple",
            "profile",
            "features",
            "cfgs",
            "original_argv",
            "effective_argv",
            "dep_info_path",
            "dep_info_digest",
            "dep_info_bytes",
            "dep_info_text",
            "sources",
            "externs",
            "outputs",
            "trace_digest",
            "trace_bytes",
            "status",
        ],
    )?;
    let extern_path = format!("{path}.externs");
    let extern_values = array(field(object, "externs")?, &extern_path)?;
    let mut externs = Vec::with_capacity(extern_values.len());
    for (index, value) in extern_values.iter().enumerate() {
        externs.push(parse_extern(value, &format!("{extern_path}[{index}]"))?);
    }
    let output_path = format!("{path}.outputs");
    let output_values = array(field(object, "outputs")?, &output_path)?;
    let mut outputs = Vec::with_capacity(output_values.len());
    for (index, value) in output_values.iter().enumerate() {
        outputs.push(parse_file_identity(
            value,
            &format!("{output_path}[{index}]"),
        )?);
    }
    Ok(compiler_capture::CompilerUnit {
        id: text_field(object, "id", path)?.to_owned(),
        closure: text_field(object, "closure", path)?.to_owned(),
        crate_name: text_field(object, "crate_name", path)?.to_owned(),
        crate_types: string_array_field(object, "crate_types", path)?,
        target_triple: text_field(object, "target_triple", path)?.to_owned(),
        profile: text_field(object, "profile", path)?.to_owned(),
        features: string_array_field(object, "features", path)?,
        cfgs: string_array_field(object, "cfgs", path)?,
        original_argv: string_array_field(object, "original_argv", path)?,
        effective_argv: string_array_field(object, "effective_argv", path)?,
        dep_info_path: path_field(object, "dep_info_path", path)?,
        dep_info_digest: digest_field(object, "dep_info_digest", path)?,
        dep_info_bytes: u64_field(object, "dep_info_bytes", path)?,
        dep_info_text: bounded_json::string(
            field(object, "dep_info_text")?,
            &format!("{path}.dep_info_text"),
        )
        .map_err(config_error)?
        .to_owned(),
        sources: path_array_field(object, "sources", path)?,
        externs,
        outputs,
        trace_digest: digest_field(object, "trace_digest", path)?,
        trace_bytes: u64_field(object, "trace_bytes", path)?,
        status: i32_field(object, "status", path)?,
    })
}

fn parse_extern(
    value: &Value,
    path: &str,
) -> Result<compiler_capture::ExternEdge, Error> {
    let object = object(value, path, &["name", "path"])?;
    Ok(compiler_capture::ExternEdge {
        name: text_field(object, "name", path)?.to_owned(),
        path: path_field(object, "path", path)?,
    })
}

fn parse_file_identity(
    value: &Value,
    path: &str,
) -> Result<compiler_capture::FileIdentity, Error> {
    let object =
        object(value, path, &["path", "content_digest", "size_bytes"])?;
    Ok(compiler_capture::FileIdentity {
        path: path_field(object, "path", path)?,
        content_digest: digest_field(object, "content_digest", path)?,
        size_bytes: u64_field(object, "size_bytes", path)?,
    })
}

fn parse_optional_provider(
    value: &Value,
    path: &str,
) -> Result<Option<compiler_capture::ProviderArtifacts>, Error> {
    if value.is_null() {
        return Ok(None);
    }
    parse_provider(value, path).map(Some)
}

fn parse_provider(
    value: &Value,
    path: &str,
) -> Result<compiler_capture::ProviderArtifacts, Error> {
    let object = object(
        value,
        path,
        &[
            "closure",
            "package_id",
            "unit",
            "cargo_artifact",
            "top_level_rlib",
            "hashed_rlib",
            "rmeta",
            "target_dir",
            "cargo_home",
        ],
    )?;
    Ok(compiler_capture::ProviderArtifacts {
        closure: text_field(object, "closure", path)?.to_owned(),
        package_id: text_field(object, "package_id", path)?.to_owned(),
        unit: text_field(object, "unit", path)?.to_owned(),
        cargo_artifact: parse_cargo_artifact(
            field(object, "cargo_artifact")?,
            &format!("{path}.cargo_artifact"),
        )?,
        top_level_rlib: parse_file_identity(
            field(object, "top_level_rlib")?,
            &format!("{path}.top_level_rlib"),
        )?,
        hashed_rlib: parse_file_identity(
            field(object, "hashed_rlib")?,
            &format!("{path}.hashed_rlib"),
        )?,
        rmeta: parse_file_identity(
            field(object, "rmeta")?,
            &format!("{path}.rmeta"),
        )?,
        target_dir: path_field(object, "target_dir", path)?,
        cargo_home: path_field(object, "cargo_home", path)?,
    })
}

fn parse_optional_reuse(
    value: &Value,
    path: &str,
) -> Result<Option<compiler_capture::ProviderReuse>, Error> {
    if value.is_null() {
        return Ok(None);
    }
    parse_provider_reuse(value, path).map(Some)
}

fn parse_provider_reuse(
    value: &Value,
    path: &str,
) -> Result<compiler_capture::ProviderReuse, Error> {
    let object = object(
        value,
        path,
        &["fresh_artifact", "hashed_rlib", "rmeta", "extern_edge"],
    )?;
    Ok(compiler_capture::ProviderReuse {
        fresh_artifact: parse_cargo_artifact(
            field(object, "fresh_artifact")?,
            &format!("{path}.fresh_artifact"),
        )?,
        hashed_rlib: parse_file_identity(
            field(object, "hashed_rlib")?,
            &format!("{path}.hashed_rlib"),
        )?,
        rmeta: parse_file_identity(
            field(object, "rmeta")?,
            &format!("{path}.rmeta"),
        )?,
        extern_edge: parse_extern(
            field(object, "extern_edge")?,
            &format!("{path}.extern_edge"),
        )?,
    })
}

fn parse_optional_transitive_reuse(
    value: &Value,
    path: &str,
) -> Result<Option<compiler_capture::TransitiveProviderReuse>, Error> {
    if value.is_null() {
        return Ok(None);
    }
    let object =
        object(value, path, &["fresh_artifact", "hashed_rlib", "rmeta"])?;
    Ok(Some(compiler_capture::TransitiveProviderReuse {
        fresh_artifact: parse_cargo_artifact(
            field(object, "fresh_artifact")?,
            &format!("{path}.fresh_artifact"),
        )?,
        hashed_rlib: parse_file_identity(
            field(object, "hashed_rlib")?,
            &format!("{path}.hashed_rlib"),
        )?,
        rmeta: parse_file_identity(
            field(object, "rmeta")?,
            &format!("{path}.rmeta"),
        )?,
    }))
}

fn validate_observation(
    observation: &Observation,
    limits: &Limits,
    registry: &Registry,
) -> Result<(), Error> {
    validate_substrate_observation(
        &observation.compiler_substrate,
        limits.substrate,
    )?;
    validate_package_observations(
        &observation.package_before,
        &observation.package_after,
        closure(registry, SHIPPING_CLOSURE)?,
        &observation.shipping.unit.sources,
        limits.package,
    )?;
    validate_package_observations(
        &observation.kernel_interface_package_before,
        &observation.kernel_interface_package_after,
        closure(registry, KERNEL_INTERFACE_SHIPPING_CLOSURE)?,
        &observation.kernel_interface_shipping.unit.sources,
        limits.package,
    )?;
    validate_build(&observation.shipping, SHIPPING_CLOSURE, limits.compiler)?;
    validate_build(
        &observation.validation,
        VALIDATION_CLOSURE,
        limits.compiler,
    )?;
    validate_build(
        &observation.kernel_interface_shipping,
        KERNEL_INTERFACE_SHIPPING_CLOSURE,
        limits.compiler,
    )?;
    validate_build(
        &observation.kernel_interface_validation,
        KERNEL_INTERFACE_VALIDATION_CLOSURE,
        limits.compiler,
    )?;
    validate_package_id_joins(observation)?;
    validate_build_relation(
        &observation.shipping,
        &observation.validation,
        "reovim_foundation",
        limits.compiler,
    )?;
    validate_build_relation(
        &observation.shipping,
        &observation.kernel_interface_shipping,
        "reovim_foundation",
        limits.compiler,
    )?;
    validate_build_relation(
        &observation.kernel_interface_shipping,
        &observation.kernel_interface_validation,
        "reovim_kernel_interface",
        limits.compiler,
    )?;
    validate_transitive_relation(
        &observation.shipping,
        &observation.kernel_interface_validation,
        limits.compiler,
    )
}

fn validate_package_id_joins(observation: &Observation) -> Result<(), Error> {
    let ids = &observation.package_ids;
    let unique = [
        ids.foundation_shipping.as_str(),
        ids.foundation_validation.as_str(),
        ids.kernel_interface_shipping.as_str(),
        ids.kernel_interface_validation.as_str(),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if unique.len() != 4 {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_PACKAGE_ID_SET",
            "four observed Cargo package IDs are not unique",
        ));
    }
    for build in [
        &observation.shipping,
        &observation.validation,
        &observation.kernel_interface_shipping,
        &observation.kernel_interface_validation,
    ] {
        validate_build_package_ids(build, ids)?;
    }
    Ok(())
}

fn validate_build_package_ids(
    build: &BuildObservation,
    ids: &PackageIds,
) -> Result<(), Error> {
    let own = package_id_for_closure(ids, &build.closure)?;
    if build.cargo.artifacts.iter().any(|artifact| {
        package_id_for_target(ids, &artifact.target_name)
            != Some(artifact.package_id.as_str())
    }) {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_PACKAGE_ID_JOIN",
            "Cargo artifact package ID differs from its observed identity",
        ));
    }
    let own_artifact = build
        .cargo
        .artifacts
        .iter()
        .find(|artifact| artifact.package_id == own);
    if own_artifact.is_none() {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_PACKAGE_ID_JOIN",
            "build lacks its exact observed package ID",
        ));
    }
    if let Some(provider) = &build.provider {
        let provider_id = package_id_for_closure(ids, &provider.closure)?;
        if provider.package_id != provider_id
            || provider.cargo_artifact.package_id != provider_id
        {
            return Err(Error::new(
                "PRODUCT_WORKLOAD_PACKAGE_ID_JOIN",
                "shipping provider package ID differs from metadata",
            ));
        }
    }
    validate_reuse_package_ids(build, ids)
}

fn validate_reuse_package_ids(
    build: &BuildObservation,
    ids: &PackageIds,
) -> Result<(), Error> {
    if let Some(reuse) = &build.provider_reuse {
        let expected = match build.closure.as_str() {
            VALIDATION_CLOSURE | KERNEL_INTERFACE_SHIPPING_CLOSURE => {
                &ids.foundation_shipping
            }
            KERNEL_INTERFACE_VALIDATION_CLOSURE => {
                &ids.kernel_interface_shipping
            }
            _ => {
                return Err(Error::new(
                    "PRODUCT_WORKLOAD_PACKAGE_ID_JOIN",
                    "unexpected direct provider package ID",
                ));
            }
        };
        if &reuse.fresh_artifact.package_id != expected {
            return Err(Error::new(
                "PRODUCT_WORKLOAD_PACKAGE_ID_JOIN",
                "direct provider reuse package ID differs from metadata",
            ));
        }
    }
    if let Some(reuse) = &build.transitive_provider_reuse
        && reuse.fresh_artifact.package_id != ids.foundation_shipping
    {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_PACKAGE_ID_JOIN",
            "transitive provider package ID differs from metadata",
        ));
    }
    Ok(())
}

fn package_id_for_closure<'a>(
    ids: &'a PackageIds,
    closure: &str,
) -> Result<&'a str, Error> {
    match closure {
        SHIPPING_CLOSURE => Ok(&ids.foundation_shipping),
        VALIDATION_CLOSURE => Ok(&ids.foundation_validation),
        KERNEL_INTERFACE_SHIPPING_CLOSURE => Ok(&ids.kernel_interface_shipping),
        KERNEL_INTERFACE_VALIDATION_CLOSURE => {
            Ok(&ids.kernel_interface_validation)
        }
        _ => Err(Error::new(
            "PRODUCT_WORKLOAD_PACKAGE_ID_JOIN",
            "build closure has no observed package ID",
        )),
    }
}

fn package_id_for_target<'a>(
    ids: &'a PackageIds,
    target: &str,
) -> Option<&'a str> {
    match target {
        "reovim_foundation" => Some(&ids.foundation_shipping),
        "reovim_foundation_public_consumer" => Some(&ids.foundation_validation),
        "reovim_kernel_interface" => Some(&ids.kernel_interface_shipping),
        "reovim_kernel_interface_public_consumer" => {
            Some(&ids.kernel_interface_validation)
        }
        _ => None,
    }
}

fn validate_substrate_observation(
    observation: &compiler_substrate::Observation,
    limits: compiler_substrate::Limits,
) -> Result<(), Error> {
    if observation.compiler_support_root != Path::new(COMPILER_SUPPORT)
        || observation.target_libdir != Path::new(TARGET_LIBDIR)
        || observation.entries.is_empty()
        || observation.entries.len() != observation.entry_count
        || observation.entry_count > limits.entry_count
        || observation.total_bytes > limits.total_bytes
        || !valid_digest(&observation.aggregate_digest)
        || !observation.entries.windows(2).all(|pair| pair[0] < pair[1])
    {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_SUBSTRATE",
            "compiler substrate identity or bounds are inconsistent",
        ));
    }
    let mut total = 0_u64;
    let mut kinds = BTreeSet::new();
    for entry in &observation.entries {
        validate_substrate_entry(entry, observation, limits)?;
        kinds.insert(entry.kind);
        total = total.checked_add(entry.size_bytes).ok_or_else(|| {
            Error::new(
                "PRODUCT_WORKLOAD_SUBSTRATE_OVERFLOW",
                "compiler substrate bytes overflow u64",
            )
        })?;
    }
    let expected = BTreeSet::from([
        compiler_substrate::EntryKind::CompilerSupport,
        compiler_substrate::EntryKind::TargetLibdir,
    ]);
    let aggregate = substrate_digest(&observation.entries)?;
    if total == observation.total_bytes
        && kinds == expected
        && aggregate == observation.aggregate_digest
    {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_SUBSTRATE_TOTAL",
        "compiler substrate total or kind coverage is inconsistent",
    ))
}

fn validate_substrate_entry(
    entry: &compiler_substrate::Entry,
    observation: &compiler_substrate::Observation,
    limits: compiler_substrate::Limits,
) -> Result<(), Error> {
    let root = match entry.kind {
        compiler_substrate::EntryKind::CompilerSupport => {
            &observation.compiler_support_root
        }
        compiler_substrate::EntryKind::TargetLibdir => {
            &observation.target_libdir
        }
    };
    let text = normalized_path(&entry.canonical_path)?;
    if entry.canonical_path != *root
        && entry.canonical_path.starts_with(root)
        && text.len() <= limits.path_bytes
        && valid_digest(&entry.content_digest)
    {
        return Ok(());
    }
    Err(Error::at(
        "PRODUCT_WORKLOAD_SUBSTRATE_ENTRY",
        text,
        "substrate entry escapes its root or has an invalid identity",
    ))
}

fn substrate_digest(
    entries: &[compiler_substrate::Entry],
) -> Result<String, Error> {
    let mut input = b"reovim-compiler-substrate/v1\0".to_vec();
    append_length(&mut input, entries.len())?;
    for entry in entries {
        let kind = match entry.kind {
            compiler_substrate::EntryKind::CompilerSupport => 1,
            compiler_substrate::EntryKind::TargetLibdir => 2,
        };
        input.push(kind);
        append_slice(
            &mut input,
            normalized_path(&entry.canonical_path)?.as_bytes(),
        )?;
        input.extend_from_slice(&entry.size_bytes.to_be_bytes());
        append_slice(&mut input, entry.content_digest.as_bytes())?;
    }
    Ok(digest::bytes(&input))
}

fn append_slice(output: &mut Vec<u8>, value: &[u8]) -> Result<(), Error> {
    append_length(output, value.len())?;
    output.extend_from_slice(value);
    Ok(())
}

fn append_length(output: &mut Vec<u8>, value: usize) -> Result<(), Error> {
    let value = u64::try_from(value).map_err(|_| {
        Error::new(
            "PRODUCT_WORKLOAD_LENGTH",
            "semantic field length does not fit u64",
        )
    })?;
    output.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

fn validate_package_observations(
    before: &package_capture::Observation,
    after: &package_capture::Observation,
    shipping: &Closure,
    sources: &[PathBuf],
    limits: package_capture::Limits,
) -> Result<(), Error> {
    let members = registered_package_members(shipping, sources)?;
    let package = &shipping.package.name;
    if before.dirty_mode != package_capture::DirtyMode::Reject
        || after.dirty_mode != package_capture::DirtyMode::Reject
        || before.stdout_bytes == 0
        || before.stdout_bytes > limits.output_bytes as u64
        || after.stdout_bytes == 0
        || after.stdout_bytes > limits.output_bytes as u64
        || before.members.len() > limits.member_count
        || after.members.len() > limits.member_count
        || before.member_path_bytes > limits.output_bytes as u64
        || after.member_path_bytes > limits.output_bytes as u64
        || before.command_argv != package_argv(package)
        || after.command_argv != package_argv(package)
        || before.members != members
        || after.members != members
    {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_PACKAGE",
            "package observations differ from the exact clean profile",
        ));
    }
    package_capture::compare(before, after).map_err(package_error)?;
    Ok(())
}

fn package_argv(package: &str) -> Vec<String> {
    [
        CARGO,
        "package",
        "--list",
        "--quiet",
        "--locked",
        "--offline",
        "--no-metadata",
        "--manifest-path",
        "/repo/product/Cargo.toml",
        "--package",
        package,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn registered_package_members(
    shipping: &Closure,
    sources: &[PathBuf],
) -> Result<Vec<PathBuf>, Error> {
    let membership = shipping.package_membership.as_ref().ok_or_else(|| {
        Error::new(
            "PRODUCT_WORKLOAD_PACKAGE_MEMBERSHIP",
            format!("shipping closure {} lacks membership", shipping.id),
        )
    })?;
    let source_set = sources.iter().cloned().collect::<BTreeSet<_>>();
    let package_root = shipping.package.manifest.parent().ok_or_else(|| {
        Error::new(
            "PRODUCT_WORKLOAD_PACKAGE_MEMBERSHIP",
            "shipping package manifest has no repository parent",
        )
    })?;
    let mut copied = BTreeSet::new();
    for member in &membership.members {
        let crate::product_closures::PackageMember::Copied {
            package_path,
            repository_path,
        } = member
        else {
            continue;
        };
        validate_copied_member(
            package_root,
            package_path,
            repository_path,
            &source_set,
        )?;
        copied.insert(repository_path.clone());
    }
    if copied != source_set {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_PACKAGE_MEMBERSHIP",
            "copied package members differ from compiler-selected sources",
        ));
    }
    let mut members = membership
        .members
        .iter()
        .map(|member| member.package_path().clone())
        .collect::<Vec<_>>();
    members.sort();
    Ok(members)
}

fn validate_copied_member(
    package_root: &Path,
    package_path: &Path,
    repository_path: &Path,
    sources: &BTreeSet<PathBuf>,
) -> Result<(), Error> {
    let expected = package_root.join(package_path);
    if repository_path == expected && sources.contains(repository_path) {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_PACKAGE_MEMBERSHIP",
        "copied package member does not join its compiler source",
    ))
}

fn validate_build(
    build: &BuildObservation,
    closure: &str,
    limits: compiler_capture::Limits,
) -> Result<(), Error> {
    let initial = closure == SHIPPING_CLOSURE;
    if build.closure != closure
        || build.unit.closure != closure
        || build.unit.id != format!("{closure}:rustc")
        || build.cargo.command_argv != build_argv(closure)?
        || build.cargo.stdout_bytes == 0
        || build.cargo.stdout_bytes > limits.cargo_json_bytes as u64
        || build.cargo.message_count != build.cargo.artifacts.len() + 1
        || build.cargo.message_count > limits.cargo_message_count
        || !valid_digest(&build.cargo.raw_digest)
    {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_BUILD",
            format!("build {closure} differs from its exact Cargo capture"),
        ));
    }
    validate_probes(build, initial, limits)?;
    validate_unit(&build.unit, closure, limits)?;
    validate_cargo_artifacts(build, closure, limits)?;
    let options = (
        build.provider.is_some(),
        build.provider_reuse.is_some(),
        build.transitive_provider_reuse.is_some(),
        build.consumer_rlib.is_some(),
    );
    let expected = match closure {
        SHIPPING_CLOSURE => (true, false, false, false),
        VALIDATION_CLOSURE => (false, true, false, true),
        KERNEL_INTERFACE_SHIPPING_CLOSURE => (true, true, false, false),
        KERNEL_INTERFACE_VALIDATION_CLOSURE => (false, true, true, true),
        _ => {
            return Err(Error::new(
                "PRODUCT_WORKLOAD_BUILD",
                "build closure is outside the four-stage profile",
            ));
        }
    };
    if options == expected {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_PROVIDER_SHAPE",
        format!("build {closure} has the wrong provider evidence shape"),
    ))
}

fn build_argv(closure: &str) -> Result<Vec<String>, Error> {
    let package = match closure {
        SHIPPING_CLOSURE => SHIPPING_PACKAGE,
        VALIDATION_CLOSURE => VALIDATION_PACKAGE,
        KERNEL_INTERFACE_SHIPPING_CLOSURE => KERNEL_INTERFACE_SHIPPING_PACKAGE,
        KERNEL_INTERFACE_VALIDATION_CLOSURE => {
            KERNEL_INTERFACE_VALIDATION_PACKAGE
        }
        _ => {
            return Err(Error::new(
                "PRODUCT_WORKLOAD_BUILD",
                "build argv closure is outside the four-stage profile",
            ));
        }
    };
    Ok([
        CARGO,
        "build",
        "--quiet",
        "--locked",
        "--offline",
        "--manifest-path",
        "/repo/product/Cargo.toml",
        "--package",
        package,
        "--lib",
        "--target",
        TARGET,
        "--profile",
        "release",
        "--jobs",
        "1",
        "--message-format=json",
        "--target-dir",
        "/scratch/build-target",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect())
}

fn validate_probes(
    build: &BuildObservation,
    shipping: bool,
    limits: compiler_capture::Limits,
) -> Result<(), Error> {
    let expected = if shipping {
        vec![
            compiler_capture::InvocationKind::VersionProbe,
            compiler_capture::InvocationKind::HostProbe,
            compiler_capture::InvocationKind::TargetProbe,
            compiler_capture::InvocationKind::VersionProbe,
        ]
    } else {
        Vec::new()
    };
    let kinds = build.probes.iter().map(|probe| probe.kind).collect::<Vec<_>>();
    if kinds != expected || build.probes.len() + 1 > limits.wrapper_trace_count
    {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_PROBES",
            "compiler discovery probes differ from the exact sequence",
        ));
    }
    let mut trace_bytes = build.unit.trace_bytes;
    for (index, probe) in build.probes.iter().enumerate() {
        validate_probe(probe, index, limits)?;
        trace_bytes =
            trace_bytes.checked_add(probe.trace_bytes).ok_or_else(|| {
                Error::new(
                    "PRODUCT_WORKLOAD_TRACE_OVERFLOW",
                    "wrapper trace bytes overflow u64",
                )
            })?;
    }
    if trace_bytes <= limits.wrapper_trace_bytes as u64 {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_TRACE_LIMIT",
        "aggregate wrapper trace bytes exceed their bound",
    ))
}

fn validate_probe(
    probe: &compiler_capture::WrapperTrace,
    index: usize,
    limits: compiler_capture::Limits,
) -> Result<(), Error> {
    let ordinal = u64::try_from(index).map_err(|_| {
        Error::new(
            "PRODUCT_WORKLOAD_PROBE_ORDINAL",
            "probe index does not fit u64",
        )
    })?;
    if probe.ordinal == ordinal
        && probe.rustc == Path::new(RUSTC)
        && probe.dep_info_path.is_none()
        && probe.status == 0
        && probe.trace_bytes > 0
        && valid_digest(&probe.trace_digest)
        && probe.original_argv == expected_probe_argv(index)
        && probe.effective_argv == expected_probe_effective(index)
        && valid_argv(&probe.original_argv, limits)
        && valid_argv(&probe.effective_argv, limits)
        && valid_environment(&probe.environment, limits)
    {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_PROBE",
        format!("compiler discovery probe {index} is inconsistent"),
    ))
}

fn expected_probe_effective(index: usize) -> Vec<String> {
    let mut values = expected_probe_argv(index);
    values.extend(["--sysroot".to_owned(), SYSROOT.to_owned()]);
    values
}

fn expected_probe_argv(index: usize) -> Vec<String> {
    if matches!(index, 0 | 3) {
        return vec![RUSTC.to_owned(), "-vV".to_owned()];
    }
    let mut values = [RUSTC, "-", "--crate-name", "___", "--print=file-names"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if index == 2 {
        values.extend(["--target".to_owned(), TARGET.to_owned()]);
    }
    values.extend(
        [
            "--crate-type",
            "bin",
            "--crate-type",
            "rlib",
            "--crate-type",
            "dylib",
            "--crate-type",
            "cdylib",
            "--crate-type",
            "staticlib",
            "--crate-type",
            "proc-macro",
            "--print=sysroot",
            "--print=split-debuginfo",
            "--print=crate-name",
            "--print=cfg",
            "-Wwarnings",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    values
}

fn validate_unit(
    unit: &compiler_capture::CompilerUnit,
    closure: &str,
    limits: compiler_capture::Limits,
) -> Result<(), Error> {
    let (crate_name, sources, dep_root, extern_name) = match closure {
        SHIPPING_CLOSURE => (
            "reovim_foundation",
            source_set(SHIPPING_SOURCES),
            Path::new("/scratch/shipping-dep-info"),
            None,
        ),
        VALIDATION_CLOSURE => (
            "reovim_foundation_public_consumer",
            source_set(VALIDATION_SOURCES),
            Path::new("/scratch/validation-dep-info"),
            Some("reovim_foundation"),
        ),
        KERNEL_INTERFACE_SHIPPING_CLOSURE => (
            "reovim_kernel_interface",
            source_set(KERNEL_INTERFACE_SHIPPING_SOURCES),
            Path::new("/scratch/kernel-interface-shipping-dep-info"),
            Some("reovim_foundation"),
        ),
        KERNEL_INTERFACE_VALIDATION_CLOSURE => (
            "reovim_kernel_interface_public_consumer",
            source_set(KERNEL_INTERFACE_VALIDATION_SOURCES),
            Path::new("/scratch/kernel-interface-validation-dep-info"),
            Some("reovim_kernel_interface"),
        ),
        _ => {
            return Err(Error::new(
                "PRODUCT_WORKLOAD_UNIT",
                "unit closure is outside the four-stage profile",
            ));
        }
    };
    let source_set = unit.sources.iter().cloned().collect::<BTreeSet<_>>();
    let shape = unit.id == format!("{closure}:rustc")
        && unit.closure == closure
        && unit.crate_name == crate_name
        && unit.crate_types == ["lib"]
        && unit.target_triple == TARGET
        && unit.profile == "release"
        && unit.features == ["default"]
        && unit.cfgs == ["feature=\"default\""]
        && unit.status == 0
        && unit.dep_info_bytes > 0
        && unit.dep_info_bytes <= limits.dep_info_file_bytes as u64
        && unit.trace_bytes > 0
        && valid_digest(&unit.dep_info_digest)
        && valid_digest(&unit.trace_digest)
        && unit.dep_info_path.starts_with(dep_root)
        && unit.dep_info_path != dep_root
        && source_set == sources
        && source_set.len() == unit.sources.len()
        && unit.sources.len() <= limits.compiler_input_count
        && valid_argv(&unit.original_argv, limits)
        && valid_argv(&unit.effective_argv, limits);
    if !shape {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_UNIT",
            format!("compiler unit {closure} differs from the exact profile"),
        ));
    }
    compiler_capture::validate_retained_unit(unit, limits)
        .map_err(compiler_error)?;
    validate_unit_outputs(unit, limits)?;
    let extern_shape = extern_name
        .map_or(unit.externs.is_empty(), |name| valid_unit_extern(unit, name));
    if extern_shape {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_EXTERN",
        "compiler unit extern set differs from the exact closure relation",
    ))
}

fn valid_unit_extern(
    unit: &compiler_capture::CompilerUnit,
    name: &str,
) -> bool {
    unit.externs.len() == 1
        && unit.externs[0].name == name
        && unit.externs[0]
            .path
            .extension()
            .is_some_and(|value| value == "rmeta")
        && unit.externs[0].path.starts_with(
            "/scratch/build-target/build-dir/x86_64-unknown-linux-gnu/\
             release/deps",
        )
}

fn validate_unit_outputs(
    unit: &compiler_capture::CompilerUnit,
    limits: compiler_capture::Limits,
) -> Result<(), Error> {
    if unit.outputs.len() != 2
        || !unit.outputs.windows(2).all(|pair| pair[0] < pair[1])
    {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_UNIT_OUTPUTS",
            "compiler unit does not retain one sorted rlib/rmeta pair",
        ));
    }
    let extensions = unit
        .outputs
        .iter()
        .filter_map(|identity| identity.path.extension())
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        std::ffi::OsStr::new("rlib"),
        std::ffi::OsStr::new("rmeta"),
    ]);
    let mut total = 0_u64;
    for identity in &unit.outputs {
        validate_file_identity(identity, limits)?;
        total = total.checked_add(identity.size_bytes).ok_or_else(|| {
            Error::new(
                "PRODUCT_WORKLOAD_ARTIFACT_OVERFLOW",
                "compiler artifact bytes overflow u64",
            )
        })?;
    }
    if extensions == expected && total <= limits.artifact_total_bytes {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_UNIT_OUTPUTS",
        "compiler unit output extensions or bytes are invalid",
    ))
}

fn validate_file_identity(
    identity: &compiler_capture::FileIdentity,
    limits: compiler_capture::Limits,
) -> Result<(), Error> {
    let text = normalized_path(&identity.path)?;
    if identity.path.starts_with("/scratch/build-target")
        && identity.size_bytes > 0
        && identity.size_bytes <= limits.artifact_total_bytes
        && text.len() <= limits.path_bytes
        && valid_digest(&identity.content_digest)
    {
        return Ok(());
    }
    Err(Error::at(
        "PRODUCT_WORKLOAD_ARTIFACT",
        text,
        "artifact identity escapes target authority or exceeds its bound",
    ))
}

fn validate_cargo_artifacts(
    build: &BuildObservation,
    closure: &str,
    limits: compiler_capture::Limits,
) -> Result<(), Error> {
    let expected = expected_cargo_events(closure)?;
    let expected_count = expected.len();
    if build.cargo.artifacts.len() != expected_count
        || build.cargo.artifacts.len() > limits.artifact_count
    {
        return Err(Error::new(
            "PRODUCT_WORKLOAD_CARGO_ARTIFACT_SET",
            "Cargo artifact event count differs from the closure profile",
        ));
    }
    let mut target_names = BTreeSet::new();
    for artifact in &build.cargo.artifacts {
        validate_cargo_artifact(artifact, limits)?;
        if !target_names.insert(artifact.target_name.as_str()) {
            return Err(Error::new(
                "PRODUCT_WORKLOAD_CARGO_ARTIFACT_DUPLICATE",
                "Cargo artifact target is duplicated",
            ));
        }
    }
    let actual = build
        .cargo
        .artifacts
        .iter()
        .map(|artifact| (artifact.target_name.as_str(), artifact.fresh))
        .collect::<BTreeSet<_>>();
    let exact = actual == expected.into_iter().collect();
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_CARGO_ARTIFACT_SET",
        "Cargo artifacts do not name the expected fresh/nonfresh units",
    ))
}

fn expected_cargo_events(
    closure: &str,
) -> Result<Vec<(&'static str, bool)>, Error> {
    match closure {
        SHIPPING_CLOSURE => Ok(vec![("reovim_foundation", false)]),
        VALIDATION_CLOSURE => Ok(vec![
            ("reovim_foundation", true),
            ("reovim_foundation_public_consumer", false),
        ]),
        KERNEL_INTERFACE_SHIPPING_CLOSURE => Ok(vec![
            ("reovim_foundation", true),
            ("reovim_kernel_interface", false),
        ]),
        KERNEL_INTERFACE_VALIDATION_CLOSURE => Ok(vec![
            ("reovim_foundation", true),
            ("reovim_kernel_interface", true),
            ("reovim_kernel_interface_public_consumer", false),
        ]),
        _ => Err(Error::new(
            "PRODUCT_WORKLOAD_CARGO_ARTIFACT_SET",
            "Cargo event closure is outside the four-stage profile",
        )),
    }
}

fn validate_cargo_artifact(
    artifact: &compiler_capture::CargoArtifact,
    limits: compiler_capture::Limits,
) -> Result<(), Error> {
    let (expected_manifest, expected_source) =
        match artifact.target_name.as_str() {
            "reovim_foundation" => (
                "/repo/kernel/foundation/Cargo.toml",
                "/repo/kernel/foundation/src/lib.rs",
            ),
            "reovim_foundation_public_consumer" => (
                "/repo/tests/foundation-public-consumer/Cargo.toml",
                "/repo/tests/foundation-public-consumer/src/lib.rs",
            ),
            "reovim_kernel_interface" => (
                "/repo/kernel/interface/Cargo.toml",
                "/repo/kernel/interface/src/lib.rs",
            ),
            "reovim_kernel_interface_public_consumer" => (
                "/repo/tests/kernel-interface-public-consumer/Cargo.toml",
                "/repo/tests/kernel-interface-public-consumer/src/lib.rs",
            ),
            _ => {
                return Err(Error::new(
                    "PRODUCT_WORKLOAD_CARGO_ARTIFACT",
                    "Cargo artifact target is outside the four-stage profile",
                ));
            }
        };
    let filenames = artifact
        .filenames
        .iter()
        .filter_map(|path| path.extension())
        .collect::<BTreeSet<_>>();
    let extensions = BTreeSet::from([
        std::ffi::OsStr::new("rlib"),
        std::ffi::OsStr::new("rmeta"),
    ]);
    let path_bounds = artifact.filenames.iter().all(|path| {
        path.starts_with("/scratch/build-target")
            && normalized_path(path)
                .is_ok_and(|value| value.len() <= limits.path_bytes)
    });
    let exact = !artifact.package_id.is_empty()
        && artifact.manifest_path == Path::new(expected_manifest)
        && artifact.target_kinds == ["lib"]
        && artifact.crate_types == ["lib"]
        && artifact.source_path == Path::new(expected_source)
        && artifact.features == ["default"]
        && artifact.filenames.len() == 2
        && filenames == extensions
        && path_bounds;
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_CARGO_ARTIFACT",
        format!(
            "Cargo artifact {} differs from the exact library profile",
            artifact.target_name
        ),
    ))
}

fn validate_build_relation(
    shipping: &BuildObservation,
    validation: &BuildObservation,
    provider_target: &str,
    limits: compiler_capture::Limits,
) -> Result<(), Error> {
    let provider = shipping.provider.as_ref().ok_or_else(|| {
        Error::new(
            "PRODUCT_WORKLOAD_PROVIDER_MISSING",
            "shipping provider evidence is absent",
        )
    })?;
    let reuse = validation.provider_reuse.as_ref().ok_or_else(|| {
        Error::new(
            "PRODUCT_WORKLOAD_REUSE_MISSING",
            "validation provider reuse is absent",
        )
    })?;
    let shipping_artifact = shipping
        .cargo
        .artifacts
        .iter()
        .find(|artifact| artifact.target_name == provider_target)
        .ok_or_else(|| {
            Error::new(
                "PRODUCT_WORKLOAD_PROVIDER_ARTIFACT",
                "shipping Cargo provider artifact is absent",
            )
        })?;
    let fresh_artifact = validation
        .cargo
        .artifacts
        .iter()
        .find(|artifact| artifact.target_name == provider_target)
        .ok_or_else(|| {
            Error::new(
                "PRODUCT_WORKLOAD_REUSE_ARTIFACT",
                "validation fresh provider artifact is absent",
            )
        })?;
    let unit_rlib = extension_identity(&shipping.unit.outputs, "rlib")?;
    let unit_rmeta = extension_identity(&shipping.unit.outputs, "rmeta")?;
    validate_file_identity(&provider.top_level_rlib, limits)?;
    validate_file_identity(&provider.hashed_rlib, limits)?;
    validate_file_identity(&provider.rmeta, limits)?;
    let provider_failure = provider_relation_failure(
        provider,
        shipping,
        shipping_artifact,
        unit_rlib,
        unit_rmeta,
    );
    if let Some(check) = provider_failure {
        return Err(build_relation_error(
            shipping, validation, "provider", check,
        ));
    }
    let reuse_failure = reuse_relation_failure(
        reuse,
        validation,
        provider,
        fresh_artifact,
        shipping_artifact,
        provider_target,
    );
    if let Some(check) = reuse_failure {
        return Err(build_relation_error(shipping, validation, "reuse", check));
    }
    if validation.consumer_rlib.is_some() {
        return validate_consumer_artifact(validation, limits);
    }
    Ok(())
}

fn provider_relation_failure(
    provider: &compiler_capture::ProviderArtifacts,
    shipping: &BuildObservation,
    shipping_artifact: &compiler_capture::CargoArtifact,
    unit_rlib: &compiler_capture::FileIdentity,
    unit_rmeta: &compiler_capture::FileIdentity,
) -> Option<&'static str> {
    first_failed_check(&[
        ("closure", provider.closure == shipping.closure),
        ("unit", provider.unit == shipping.unit.id),
        ("cargo_artifact", provider.cargo_artifact == *shipping_artifact),
        ("package_id", provider.package_id == shipping_artifact.package_id),
        (
            "target_dir",
            provider.target_dir == Path::new("/scratch/build-target"),
        ),
        ("cargo_home", provider.cargo_home == Path::new("/scratch/cargo-home")),
        ("hashed_rlib", provider.hashed_rlib == *unit_rlib),
        ("rmeta", provider.rmeta == *unit_rmeta),
        (
            "distinct_rlib_paths",
            provider.top_level_rlib.path != provider.hashed_rlib.path,
        ),
        (
            "top_level_rlib_path",
            provider.top_level_rlib.path
                == top_level_rlib_path(&shipping_artifact.target_name),
        ),
        (
            "artifact_filenames",
            artifact_filenames_match(
                shipping_artifact,
                &provider.top_level_rlib,
                &provider.rmeta,
            ),
        ),
        (
            "rlib_bytes",
            same_file_bytes(&provider.top_level_rlib, &provider.hashed_rlib),
        ),
    ])
}

fn reuse_relation_failure(
    reuse: &compiler_capture::ProviderReuse,
    validation: &BuildObservation,
    provider: &compiler_capture::ProviderArtifacts,
    fresh_artifact: &compiler_capture::CargoArtifact,
    shipping_artifact: &compiler_capture::CargoArtifact,
    provider_target: &str,
) -> Option<&'static str> {
    first_failed_check(&[
        ("fresh_artifact", reuse.fresh_artifact == *fresh_artifact),
        (
            "artifact_semantics",
            same_artifact_semantics(fresh_artifact, shipping_artifact),
        ),
        ("hashed_rlib", reuse.hashed_rlib == provider.hashed_rlib),
        ("rmeta", reuse.rmeta == provider.rmeta),
        (
            "artifact_filenames",
            artifact_filenames_match(
                fresh_artifact,
                &reuse.hashed_rlib,
                &reuse.rmeta,
            ),
        ),
        (
            "consumer_edge_set",
            validation.unit.externs == [reuse.extern_edge.clone()],
        ),
        ("extern_name", reuse.extern_edge.name == provider_target),
        ("extern_path", reuse.extern_edge.path == provider.rmeta.path),
    ])
}

fn first_failed_check(checks: &[(&'static str, bool)]) -> Option<&'static str> {
    checks.iter().find_map(|(name, passed)| (!passed).then_some(*name))
}

fn build_relation_error(
    shipping: &BuildObservation,
    validation: &BuildObservation,
    group: &str,
    check: &str,
) -> Error {
    Error::at(
        "PRODUCT_WORKLOAD_PROVIDER_RELATION",
        format!("{} -> {}", shipping.closure, validation.closure),
        format!("{group} check failed: {check}"),
    )
}

fn validate_consumer_artifact(
    validation: &BuildObservation,
    limits: compiler_capture::Limits,
) -> Result<(), Error> {
    let target = validation.unit.crate_name.as_str();
    let artifact = validation
        .cargo
        .artifacts
        .iter()
        .find(|value| value.target_name == target)
        .expect("validation artifact set checked before relation");
    let cargo_rlib = validation.consumer_rlib.as_ref().ok_or_else(|| {
        Error::new(
            "PRODUCT_WORKLOAD_CONSUMER_ARTIFACT",
            "consumer Cargo rlib identity is absent",
        )
    })?;
    validate_file_identity(cargo_rlib, limits)?;
    let unit_rlib = extension_identity(&validation.unit.outputs, "rlib")?;
    let unit_rmeta = extension_identity(&validation.unit.outputs, "rmeta")?;
    let failure = first_failed_check(&[
        ("top_level_rlib_path", cargo_rlib.path == top_level_rlib_path(target)),
        ("distinct_rlib_paths", cargo_rlib.path != unit_rlib.path),
        ("rlib_bytes", same_file_bytes(cargo_rlib, unit_rlib)),
        (
            "artifact_filenames",
            artifact_filenames_match(artifact, cargo_rlib, unit_rmeta),
        ),
    ]);
    let Some(check) = failure else { return Ok(()) };
    Err(Error::at(
        "PRODUCT_WORKLOAD_CONSUMER_ARTIFACT",
        validation.closure.clone(),
        format!("consumer check failed: {check}"),
    ))
}

fn top_level_rlib_path(crate_name: &str) -> PathBuf {
    Path::new("/scratch/build-target")
        .join(TARGET)
        .join("release")
        .join(format!("lib{crate_name}.rlib"))
}

fn validate_transitive_relation(
    foundation: &BuildObservation,
    validation: &BuildObservation,
    limits: compiler_capture::Limits,
) -> Result<(), Error> {
    let provider = foundation.provider.as_ref().ok_or_else(|| {
        Error::new(
            "PRODUCT_WORKLOAD_PROVIDER_MISSING",
            "Foundation provider evidence is absent",
        )
    })?;
    let reuse =
        validation.transitive_provider_reuse.as_ref().ok_or_else(|| {
            Error::new(
                "PRODUCT_WORKLOAD_REUSE_MISSING",
                "transitive Foundation reuse is absent",
            )
        })?;
    validate_file_identity(&reuse.hashed_rlib, limits)?;
    validate_file_identity(&reuse.rmeta, limits)?;
    let cargo_artifact = validation
        .cargo
        .artifacts
        .iter()
        .find(|artifact| artifact.target_name == "reovim_foundation")
        .ok_or_else(|| {
            Error::new(
                "PRODUCT_WORKLOAD_TRANSITIVE_PROVIDER_RELATION",
                "Kernel Interface validation lacks Foundation reuse",
            )
        })?;
    let exact = reuse.fresh_artifact == *cargo_artifact
        && reuse.fresh_artifact.target_name == "reovim_foundation"
        && reuse.fresh_artifact.fresh
        && reuse.hashed_rlib == provider.hashed_rlib
        && reuse.rmeta == provider.rmeta
        && artifact_filenames_match(
            cargo_artifact,
            &reuse.hashed_rlib,
            &reuse.rmeta,
        )
        && same_artifact_semantics(
            &reuse.fresh_artifact,
            &provider.cargo_artifact,
        );
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_TRANSITIVE_PROVIDER_RELATION",
        "transitive Foundation artifact identity drifted",
    ))
}

fn artifact_filenames_match(
    artifact: &compiler_capture::CargoArtifact,
    rlib: &compiler_capture::FileIdentity,
    rmeta: &compiler_capture::FileIdentity,
) -> bool {
    let actual = artifact.filenames.iter().collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([&rlib.path, &rmeta.path]);
    actual.len() == artifact.filenames.len() && actual == expected
}

fn extension_identity<'a>(
    values: &'a [compiler_capture::FileIdentity],
    extension: &str,
) -> Result<&'a compiler_capture::FileIdentity, Error> {
    let mut matches = values.iter().filter(|identity| {
        identity.path.extension().and_then(std::ffi::OsStr::to_str)
            == Some(extension)
    });
    let value = matches.next().ok_or_else(|| {
        Error::new(
            "PRODUCT_WORKLOAD_ARTIFACT_EXTENSION",
            format!("artifact set lacks one {extension}"),
        )
    })?;
    if matches.next().is_none() {
        return Ok(value);
    }
    Err(Error::new(
        "PRODUCT_WORKLOAD_ARTIFACT_EXTENSION",
        format!("artifact set duplicates {extension}"),
    ))
}

fn same_file_bytes(
    left: &compiler_capture::FileIdentity,
    right: &compiler_capture::FileIdentity,
) -> bool {
    left.content_digest == right.content_digest
        && left.size_bytes == right.size_bytes
}

fn same_artifact_semantics(
    left: &compiler_capture::CargoArtifact,
    right: &compiler_capture::CargoArtifact,
) -> bool {
    left.package_id == right.package_id
        && left.manifest_path == right.manifest_path
        && left.target_name == right.target_name
        && left.target_kinds == right.target_kinds
        && left.crate_types == right.crate_types
        && left.source_path == right.source_path
        && left.features == right.features
}

fn valid_argv(argv: &[String], limits: compiler_capture::Limits) -> bool {
    if argv.is_empty()
        || argv.len() > limits.wrapper_argv_items
        || argv.first().map(String::as_str) != Some(RUSTC)
    {
        return false;
    }
    argv.iter()
        .try_fold(0_usize, |total, value| {
            if value.is_empty() || value.contains(['\0', '\r', '\n']) {
                return None;
            }
            total.checked_add(value.len())
        })
        .is_some_and(|bytes| bytes <= limits.wrapper_argv_bytes)
}

fn valid_environment(
    values: &[compiler_capture::EnvironmentEntry],
    limits: compiler_capture::Limits,
) -> bool {
    if !values.windows(2).all(|pair| pair[0] < pair[1])
        || values.iter().any(|entry| !allowed_environment_name(&entry.name))
        || !required_environment_matches(values)
    {
        return false;
    }
    values
        .iter()
        .try_fold(0_usize, |total, entry| {
            if entry.name.is_empty()
                || entry.name.contains(['\0', '\r', '\n', '='])
                || entry.value.contains(['\0', '\r', '\n'])
            {
                return None;
            }
            total.checked_add(entry.name.len())?.checked_add(entry.value.len())
        })
        .is_some_and(|bytes| bytes <= limits.wrapper_argv_bytes)
}

fn allowed_environment_name(name: &str) -> bool {
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

fn required_environment_matches(
    values: &[compiler_capture::EnvironmentEntry],
) -> bool {
    let expected = [
        ("CARGO_BUILD_BUILD_DIR", "/scratch/build-target/build-dir"),
        ("CARGO_HOME", "/scratch/cargo-home"),
        ("CARGO_INCREMENTAL", "0"),
        ("CARGO_NET_OFFLINE", "true"),
        ("LD_LIBRARY_PATH", COMPILER_SUPPORT),
        ("CARGO_TARGET_DIR", "/scratch/build-target"),
        ("RUSTC", RUSTC),
        ("TEMP", "/scratch/temp"),
        ("TERM", "dumb"),
        ("TMP", "/scratch/temp"),
        ("TMPDIR", "/scratch/temp"),
    ];
    expected
        .iter()
        .all(|(name, value)| environment_value(values, name) == Some(*value))
}

fn environment_value<'a>(
    values: &'a [compiler_capture::EnvironmentEntry],
    name: &str,
) -> Option<&'a str> {
    values
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.value.as_str())
}

fn object<'a>(
    value: &'a Value,
    path: &str,
    required: &[&str],
) -> Result<&'a Map<String, Value>, Error> {
    bounded_json::object(value, path, required, &[]).map_err(config_error)
}

fn field<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a Value, Error> {
    object.get(name).ok_or_else(|| {
        Error::new(
            "PRODUCT_WORKLOAD_INTERNAL_FIELD",
            format!("validated object omitted field {name}"),
        )
    })
}

fn array<'a>(value: &'a Value, path: &str) -> Result<&'a [Value], Error> {
    bounded_json::array(value, path).map_err(config_error)
}

fn text_field<'a>(
    object: &'a Map<String, Value>,
    name: &str,
    parent: &str,
) -> Result<&'a str, Error> {
    let path = format!("{parent}.{name}");
    let value = bounded_json::string(field(object, name)?, &path)
        .map_err(config_error)?;
    if !value.is_empty() && !value.contains(['\0', '\r', '\n']) {
        return Ok(value);
    }
    Err(Error::at(
        "PRODUCT_WORKLOAD_TEXT",
        path,
        "text is empty or contains a framing character",
    ))
}

fn exact_text(value: &Value, path: &str, expected: &str) -> Result<(), Error> {
    let observed = bounded_json::string(value, path).map_err(config_error)?;
    if observed == expected {
        return Ok(());
    }
    Err(Error::at(
        "PRODUCT_WORKLOAD_SCHEMA",
        path,
        format!("expected {expected:?}, found {observed:?}"),
    ))
}

fn u64_field(
    object: &Map<String, Value>,
    name: &str,
    parent: &str,
) -> Result<u64, Error> {
    bounded_json::unsigned(field(object, name)?, &format!("{parent}.{name}"))
        .map_err(config_error)
}

fn usize_field(
    object: &Map<String, Value>,
    name: &str,
    parent: &str,
) -> Result<usize, Error> {
    usize::try_from(u64_field(object, name, parent)?).map_err(|_| {
        Error::at(
            "PRODUCT_WORKLOAD_INTEGER",
            format!("{parent}.{name}"),
            "unsigned integer does not fit usize",
        )
    })
}

fn i32_field(
    object: &Map<String, Value>,
    name: &str,
    parent: &str,
) -> Result<i32, Error> {
    let path = format!("{parent}.{name}");
    let value = field(object, name)?.as_i64().ok_or_else(|| {
        Error::at("PRODUCT_WORKLOAD_INTEGER", &path, "expected signed integer")
    })?;
    i32::try_from(value).map_err(|_| {
        Error::at(
            "PRODUCT_WORKLOAD_INTEGER",
            path,
            "signed integer does not fit i32",
        )
    })
}

fn bool_field(
    object: &Map<String, Value>,
    name: &str,
    parent: &str,
) -> Result<bool, Error> {
    bounded_json::boolean(field(object, name)?, &format!("{parent}.{name}"))
        .map_err(config_error)
}

fn digest_field(
    object: &Map<String, Value>,
    name: &str,
    parent: &str,
) -> Result<String, Error> {
    let value = text_field(object, name, parent)?;
    if valid_digest(value) {
        return Ok(value.to_owned());
    }
    Err(Error::at(
        "PRODUCT_WORKLOAD_DIGEST",
        format!("{parent}.{name}"),
        "digest is not lowercase SHA-256 hex",
    ))
}

fn valid_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn string_array_field(
    object: &Map<String, Value>,
    name: &str,
    parent: &str,
) -> Result<Vec<String>, Error> {
    let path = format!("{parent}.{name}");
    let values = array(field(object, name)?, &path)?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let item_path = format!("{path}[{index}]");
            let value = bounded_json::string(value, &item_path)
                .map_err(config_error)?;
            if !value.is_empty() && !value.contains(['\0', '\r', '\n']) {
                return Ok(value.to_owned());
            }
            Err(Error::at(
                "PRODUCT_WORKLOAD_TEXT",
                item_path,
                "array text is empty or contains a framing character",
            ))
        })
        .collect()
}

fn path_array_field(
    object: &Map<String, Value>,
    name: &str,
    parent: &str,
) -> Result<Vec<PathBuf>, Error> {
    let path = format!("{parent}.{name}");
    let values = array(field(object, name)?, &path)?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| parse_path(value, &format!("{path}[{index}]")))
        .collect()
}

fn path_field(
    object: &Map<String, Value>,
    name: &str,
    parent: &str,
) -> Result<PathBuf, Error> {
    parse_path(field(object, name)?, &format!("{parent}.{name}"))
}

fn optional_path_field(
    object: &Map<String, Value>,
    name: &str,
    parent: &str,
) -> Result<Option<PathBuf>, Error> {
    let value = field(object, name)?;
    if value.is_null() {
        return Ok(None);
    }
    parse_path(value, &format!("{parent}.{name}")).map(Some)
}

fn parse_path(value: &Value, path: &str) -> Result<PathBuf, Error> {
    let text = bounded_json::string(value, path).map_err(config_error)?;
    normalized_path(Path::new(text))
        .map(PathBuf::from)
        .map_err(|failure| Error::at(failure.code, path, failure.message))
}

fn path_value(path: &Path) -> Result<Value, Error> {
    normalized_path(path).map(|value| Value::String(value.to_owned()))
}

fn normalized_path(path: &Path) -> Result<&str, Error> {
    let text = path.to_str().ok_or_else(|| {
        Error::at(
            "PRODUCT_WORKLOAD_PATH_UTF8",
            path.display().to_string(),
            "path is not UTF-8",
        )
    })?;
    let valid = !text.is_empty()
        && (text == "/" || !text.ends_with('/'))
        && !text.contains(['\0', '\r', '\n', '\\'])
        && !text.contains("//")
        && !text.split('/').any(|part| matches!(part, "." | ".."));
    if valid {
        return Ok(text);
    }
    Err(Error::at(
        "PRODUCT_WORKLOAD_PATH_FORM",
        text,
        "path is not normalized UTF-8",
    ))
}

fn compiler_request<'a>(
    paths: &'a WorkloadPaths,
    closure: &'a Closure,
    package_id: &'a str,
    metadata: &'a cargo::Metadata,
    admitted_sources: &'a BTreeSet<PathBuf>,
    provider: Option<&'a compiler_capture::ProviderArtifacts>,
    transitive_provider: Option<&'a compiler_capture::ProviderArtifacts>,
) -> Result<compiler_capture::BuildRequest<'a>, Error> {
    let (trace_dir, dep_info_dir) = match closure.id.as_str() {
        SHIPPING_CLOSURE => (&paths.shipping_trace, &paths.shipping_dep_info),
        VALIDATION_CLOSURE => {
            (&paths.validation_trace, &paths.validation_dep_info)
        }
        KERNEL_INTERFACE_SHIPPING_CLOSURE => (
            &paths.kernel_interface_shipping_trace,
            &paths.kernel_interface_shipping_dep_info,
        ),
        KERNEL_INTERFACE_VALIDATION_CLOSURE => (
            &paths.kernel_interface_validation_trace,
            &paths.kernel_interface_validation_dep_info,
        ),
        _ => {
            return Err(Error::new(
                "PRODUCT_WORKLOAD_CLOSURE",
                "compiler request closure is outside the four-stage profile",
            ));
        }
    };
    Ok(compiler_capture::BuildRequest {
        closure,
        package_id,
        metadata,
        repository_root: Path::new(REPOSITORY),
        cargo: Path::new(CARGO),
        rustc: Path::new(RUSTC),
        checker: Path::new(CHECKER),
        sysroot: Path::new(SYSROOT),
        scratch_root: Path::new(SCRATCH),
        cargo_home: &paths.cargo_home,
        temp_dir: &paths.temp_dir,
        target_dir: &paths.build_target,
        trace_dir,
        dep_info_dir,
        admitted_sources,
        provider,
        transitive_provider,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::Duration,
    };

    use serde_json::{Value, json};

    use super::{
        CARGO, CHECKER, KERNEL_INTERFACE_SHIPPING_CLOSURE,
        KERNEL_INTERFACE_SHIPPING_PACKAGE, KERNEL_INTERFACE_SHIPPING_SOURCES,
        KERNEL_INTERFACE_VALIDATION_CLOSURE,
        KERNEL_INTERFACE_VALIDATION_SOURCES, Limits, Observation,
        PREPARED_REGISTRY_PATH, REQUEST_MAX_BYTES, REQUEST_SCHEMA, RUSTC,
        RegistryInput, SHIPPING_CLOSURE, SHIPPING_PACKAGE, SHIPPING_SOURCES,
        SYSROOT, TARGET, VALIDATION_CLOSURE, VALIDATION_SOURCES, build_argv,
        expected_probe_argv, expected_probe_effective, observation_value,
        package_argv, parse_request,
        parse_result as parse_result_with_registry, read_registry_file,
        request_value, source_set, substrate_digest,
    };
    use crate::{
        cargo::{Metadata, Node, Package, Target},
        compiler_capture::{
            BuildObservation, CargoArtifact, CargoObservation, CompilerUnit,
            EnvironmentEntry, ExternEdge, FileIdentity, InvocationKind,
            ProviderArtifacts, ProviderReuse, TransitiveProviderReuse,
            WrapperTrace,
        },
        compiler_substrate::{self, Entry, EntryKind},
        package_capture::{self, DirtyMode, Phase},
        process::ProcessLimits,
        product_closures::{ParseLimits, Registry},
        topology::HARD_LIMITS,
    };

    const PROCESS: ProcessLimits = ProcessLimits {
        stdout_bytes: 4_096,
        stderr_bytes: 4_096,
        deadline: Duration::from_secs(10),
        supervision_deadline: Duration::from_secs(11),
    };

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn limits() -> Limits {
        Limits {
            registry: ParseLimits {
                path_bytes: 4_096,
                closure_items: 16,
                package_members: 32,
                public_items: HARD_LIMITS.product_execution.public_item_count,
            },
            metadata_process: PROCESS,
            metadata_json_depth: 64,
            metadata_json_items: 10_000,
            package: package_capture::Limits {
                process: PROCESS,
                output_bytes: 4_096,
                member_count: 32,
                path_bytes: 4_096,
            },
            compiler: crate::compiler_capture::Limits {
                process: PROCESS,
                path_bytes: 4_096,
                cargo_json_bytes: 4_096,
                cargo_message_count: 16,
                wrapper_trace_bytes: 1_048_576,
                wrapper_argv_items: 512,
                wrapper_argv_bytes: 65_536,
                wrapper_trace_count: 8,
                dep_info_file_bytes: 4_096,
                dep_info_total_bytes: 8_192,
                compiler_input_count: 32,
                artifact_count: 16,
                artifact_total_bytes: 1_048_576,
                json_depth: 64,
                json_items: 10_000,
            },
            substrate: compiler_substrate::Limits {
                path_bytes: 4_096,
                entry_count: 16,
                total_bytes: 1_048_576,
            },
            source: crate::product_source::AnalysisLimits {
                source_files: 16,
                source_file_bytes: 1_048_576,
                source_total_bytes: 2_097_152,
                syntax_nodes: 100_000,
            },
            result_bytes: 2_097_152,
            result_json_depth: 64,
            result_json_items: 100_000,
        }
    }

    fn registry_value() -> Value {
        serde_json::from_str(include_str!(
            "../../../conformance/product-closures.json"
        ))
        .unwrap()
    }

    fn registry_bytes() -> &'static [u8] {
        include_bytes!("../../../conformance/product-closures.json")
    }

    fn prepared_registry_input() -> RegistryInput {
        registry_input(Path::new(PREPARED_REGISTRY_PATH), registry_bytes())
    }

    fn registry_input(path: &Path, bytes: &[u8]) -> RegistryInput {
        RegistryInput {
            path: path.to_path_buf(),
            content_digest: crate::digest::bytes(bytes),
            size_bytes: u64::try_from(bytes.len()).unwrap(),
        }
    }

    fn registry() -> Registry {
        let value = registry_value();
        Registry::parse(&value, limits().registry).unwrap()
    }

    fn parse_result(
        value: &Value,
        limits: &Limits,
    ) -> Result<Observation, super::Error> {
        parse_result_with_registry(value, limits, &registry())
    }

    fn registry_from_value(value: &Value) -> Registry {
        Registry::parse(value, limits().registry).unwrap()
    }

    #[test]
    fn request_roundtrip_preserves_bounded_registry_reference() {
        let input = prepared_registry_input();
        let value = request_value(&input, &limits()).unwrap();
        let parsed = parse_request(&value).unwrap();
        assert_eq!(parsed.registry, input);
        assert_eq!(parsed.limits, limits());
        assert_eq!(value["schema"], REQUEST_SCHEMA);
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(bytes.len() <= REQUEST_MAX_BYTES);
    }

    #[test]
    fn request_rejects_protocol_and_reference_mutants() {
        let input = prepared_registry_input();
        let mut value = request_value(&input, &limits()).unwrap();
        value["extra"] = json!(true);
        assert_eq!(
            parse_request(&value).unwrap_err().code,
            "CFG_UNKNOWN_FIELD"
        );

        let mut value = request_value(&input, &limits()).unwrap();
        value.as_object_mut().unwrap().remove("registry");
        assert_eq!(
            parse_request(&value).unwrap_err().code,
            "CFG_MISSING_FIELD"
        );

        let mut value = request_value(&input, &limits()).unwrap();
        value["product_closures"] = registry_value();
        assert_eq!(
            parse_request(&value).unwrap_err().code,
            "PRODUCT_WORKLOAD_REQUEST_LIMIT"
        );

        for field in ["path", "content_digest", "size_bytes"] {
            let mut value = request_value(&input, &limits()).unwrap();
            value["registry"].as_object_mut().unwrap().remove(field);
            assert_eq!(
                parse_request(&value).unwrap_err().code,
                "CFG_MISSING_FIELD"
            );
        }

        let mut changed = input.clone();
        changed.path = PathBuf::from("/repo/conformance/other.json");
        assert_eq!(
            request_value(&changed, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_REGISTRY_INPUT"
        );

        let mut changed = input.clone();
        changed.content_digest = "sha256:wrong".to_owned();
        assert_eq!(
            request_value(&changed, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_REGISTRY_INPUT"
        );

        let mut changed = input.clone();
        changed.size_bytes = 0;
        assert_eq!(
            request_value(&changed, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_REGISTRY_INPUT"
        );

        let mut invalid = limits();
        invalid.compiler.wrapper_trace_count = 0;
        assert_eq!(
            request_value(&input, &invalid).unwrap_err().code,
            "PRODUCT_WORKLOAD_LIMITS"
        );
    }

    #[test]
    fn registry_file_loads_exact_current_bytes() {
        let fixture = RegistryFixture::new(registry_bytes());
        let input = fixture.input(registry_bytes());
        assert_eq!(read_registry_file(&input, &limits()).unwrap(), registry());
    }

    #[test]
    fn registry_file_rejects_kind_size_and_digest_mutants() {
        let fixture = RegistryFixture::new(registry_bytes());
        let input = fixture.input(registry_bytes());

        let directory = registry_input(&fixture.root, registry_bytes());
        assert_eq!(
            read_registry_file(&directory, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_REGISTRY_KIND"
        );

        let mut short = input.clone();
        short.size_bytes -= 1;
        assert_eq!(
            read_registry_file(&short, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_REGISTRY_SIZE"
        );

        let mut long = input.clone();
        long.size_bytes += 1;
        assert_eq!(
            read_registry_file(&long, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_REGISTRY_SIZE"
        );

        let mut digest = input;
        digest.content_digest = crate::digest::bytes(b"different");
        assert_eq!(
            read_registry_file(&digest, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_REGISTRY_DIGEST"
        );
    }

    #[cfg(unix)]
    #[test]
    fn registry_file_rejects_symlink() {
        use std::os::unix::fs::symlink;

        let fixture = RegistryFixture::new(registry_bytes());
        let alias = fixture.root.join("registry-alias.json");
        symlink(&fixture.path, &alias).unwrap();
        let input = registry_input(&alias, registry_bytes());
        assert_eq!(
            read_registry_file(&input, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_REGISTRY_KIND"
        );
    }

    #[test]
    fn registry_file_rejects_raw_and_semantic_mutants() {
        assert_registry_parse_failure(b"{", "CFG_JSON_SYNTAX");
        assert_registry_parse_failure(
            br#"{"schema":1,"schema":2}"#,
            "CFG_DUPLICATE_FIELD",
        );

        let mut semantic = registry_value();
        semantic["closures"].as_array_mut().unwrap().pop();
        let bytes = serde_json::to_vec(&semantic).unwrap();
        let fixture = RegistryFixture::new(&bytes);
        let failure =
            read_registry_file(&fixture.input(&bytes), &limits()).unwrap_err();
        assert_eq!(failure.code, "PRODUCT_RELATION_CONSUMER_UNRESOLVED");
    }

    #[test]
    fn registry_file_rejects_identity_drift() {
        let fixture = RegistryFixture::new(registry_bytes());
        let input = fixture.input(registry_bytes());
        let mut changed = registry_bytes().to_vec();
        changed[0] = if changed[0] == b'{' { b' ' } else { b'{' };
        fixture.rewrite(&changed);
        assert_eq!(
            read_registry_file(&input, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_REGISTRY_DIGEST"
        );
    }

    fn assert_registry_parse_failure(bytes: &[u8], code: &'static str) {
        let fixture = RegistryFixture::new(bytes);
        let input = fixture.input(bytes);
        assert_eq!(
            read_registry_file(&input, &limits()).unwrap_err().code,
            code
        );
    }

    struct RegistryFixture {
        root: PathBuf,
        path: PathBuf,
    }

    impl RegistryFixture {
        fn new(bytes: &[u8]) -> Self {
            let ordinal = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "reovim-product-registry-{}-{ordinal}",
                std::process::id(),
            ));
            fs::create_dir(&root).unwrap();
            let root = root.canonicalize().unwrap();
            let path = root.join("product-closures.json");
            fs::write(&path, bytes).unwrap();
            Self { root, path }
        }

        fn input(&self, bytes: &[u8]) -> RegistryInput {
            registry_input(&self.path, bytes)
        }

        fn rewrite(&self, bytes: &[u8]) {
            fs::write(&self.path, bytes).unwrap();
        }
    }

    impl Drop for RegistryFixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn result_roundtrip_preserves_full_typed_observation() {
        let observation = observation();
        let value = observation_value(&observation).unwrap();
        assert_eq!(parse_result(&value, &limits()).unwrap(), observation);
        let shipping_rmeta =
            value.pointer("/foundation_shipping/provider/rmeta/path").unwrap();
        let consumer_extern = value
            .pointer("/foundation_validation/provider_reuse/extern_edge/path")
            .unwrap();
        assert_eq!(shipping_rmeta, consumer_extern);
    }

    #[test]
    fn result_rejects_closed_protocol_and_probe_mutants() {
        let mut value = observation_value(&observation()).unwrap();
        value["unknown"] = json!(0);
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "CFG_UNKNOWN_FIELD"
        );

        let mut value = observation_value(&observation()).unwrap();
        value["foundation_shipping"]["probes"].as_array_mut().unwrap().pop();
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_PROBES"
        );

        let mut value = observation_value(&observation()).unwrap();
        let probe = value["foundation_shipping"]["probes"][0].clone();
        value["foundation_validation"]["probes"] = json!([probe]);
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_PROBES"
        );

        let mut value = observation_value(&observation()).unwrap();
        let environment =
            value["foundation_shipping"]["probes"][0]["environment"]
                .as_array_mut()
                .unwrap();
        let term = environment
            .iter_mut()
            .find(|entry| entry["name"] == "TERM")
            .unwrap();
        term["value"] = json!("xterm");
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_PROBE"
        );
    }

    #[test]
    fn result_rejects_package_substrate_and_provider_drift() {
        let mut value = observation_value(&observation()).unwrap();
        value["foundation_package_after"]["dirty_mode"] =
            json!("candidate_allow");
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_DIRTY_MODE"
        );

        let mut value = observation_value(&observation()).unwrap();
        value["compiler_substrate"]["aggregate_digest"] = json!(digest('f'));
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_SUBSTRATE_TOTAL"
        );

        let mut value = observation_value(&observation()).unwrap();
        let path = "/foundation_shipping/provider/top_level_rlib/\
                    content_digest";
        *value.pointer_mut(path).unwrap() = json!(digest('9'));
        let error = parse_result(&value, &limits()).unwrap_err();
        assert_eq!(error.code, "PRODUCT_WORKLOAD_PROVIDER_RELATION");
        assert_eq!(
            error.subject.as_deref(),
            Some("foundation-shipping -> foundation-consumer")
        );
        assert_eq!(error.message, "provider check failed: rlib_bytes");

        let mut value = observation_value(&observation()).unwrap();
        let path = "/foundation_validation/provider_reuse/extern_edge/path";
        *value.pointer_mut(path).unwrap() =
            json!("/scratch/build-target/wrong.rmeta");
        let error = parse_result(&value, &limits()).unwrap_err();
        assert_eq!(
            error.subject.as_deref(),
            Some("foundation-shipping -> foundation-consumer")
        );
        assert_eq!(error.code, "PRODUCT_WORKLOAD_PROVIDER_RELATION");
        assert_eq!(error.message, "reuse check failed: consumer_edge_set");
    }

    #[test]
    fn result_rejects_build_dir_as_final_artifact_root() {
        let mut value = observation_value(&observation()).unwrap();
        let wrong = json!(concat!(
            "/scratch/build-target/build-dir/",
            "x86_64-unknown-linux-gnu/release/",
            "libreovim_foundation.rlib",
        ));
        value["foundation_shipping"]["cargo"]["artifacts"][0]["filenames"][0] =
            wrong.clone();
        value["foundation_shipping"]["provider"]["cargo_artifact"]["filenames"]
            [0] = wrong.clone();
        value["foundation_shipping"]["provider"]["top_level_rlib"]["path"] =
            wrong;
        let error = parse_result(&value, &limits()).unwrap_err();
        assert_eq!(error.code, "PRODUCT_WORKLOAD_PROVIDER_RELATION");
        assert_eq!(
            error.subject.as_deref(),
            Some("foundation-shipping -> foundation-consumer")
        );
        assert_eq!(error.message, "provider check failed: top_level_rlib_path");
    }

    #[test]
    fn result_joins_package_membership_to_registry_and_sources() {
        let result = observation_value(&observation()).unwrap();
        let mut value = registry_value();
        shipping_closure_value(&mut value, KERNEL_INTERFACE_SHIPPING_CLOSURE)
            ["package_membership"]["members"]
            .as_array_mut()
            .unwrap()
            .pop();
        let changed = registry_from_value(&value);
        assert_eq!(
            parse_result_with_registry(&result, &limits(), &changed)
                .unwrap_err()
                .code,
            "PRODUCT_WORKLOAD_PACKAGE_MEMBERSHIP"
        );

        let mut value = registry_value();
        let members = shipping_closure_value(
            &mut value,
            KERNEL_INTERFACE_SHIPPING_CLOSURE,
        )["package_membership"]["members"]
            .as_array_mut()
            .unwrap();
        let copied = members
            .iter_mut()
            .find(|member| member["package_path"] == "src/clock.rs")
            .unwrap();
        copied["repository_path"] = json!("kernel/interface/src/clock2.rs");
        let changed = registry_from_value(&value);
        assert_eq!(
            parse_result_with_registry(&result, &limits(), &changed)
                .unwrap_err()
                .code,
            "PRODUCT_WORKLOAD_PACKAGE_MEMBERSHIP"
        );

        let mut value = registry_value();
        let members = shipping_closure_value(
            &mut value,
            KERNEL_INTERFACE_SHIPPING_CLOSURE,
        )["package_membership"]["members"]
            .as_array_mut()
            .unwrap();
        let copied = members
            .iter_mut()
            .find(|member| member["package_path"] == "src/clock.rs")
            .unwrap();
        copied["package_path"] = json!("src/clock2.rs");
        let changed = registry_from_value(&value);
        assert_eq!(
            parse_result_with_registry(&result, &limits(), &changed)
                .unwrap_err()
                .code,
            "PRODUCT_WORKLOAD_PACKAGE_MEMBERSHIP"
        );
    }

    #[test]
    fn metadata_requires_edition_2024_for_the_fixed_packages() {
        let registry = registry();
        let shipping = registry
            .closures
            .iter()
            .find(|closure| closure.id == SHIPPING_CLOSURE)
            .unwrap();
        let mut metadata = shipping_metadata(shipping);
        assert_eq!(super::package_id(&metadata, shipping).unwrap(), "shipping");
        metadata.packages.get_mut("shipping").unwrap().edition =
            "2015".to_owned();
        assert_eq!(
            super::package_id(&metadata, shipping).unwrap_err().code,
            "PRODUCT_WORKLOAD_METADATA_EDITION"
        );
    }

    #[test]
    fn result_rejects_package_id_retargeting() {
        let mut value = observation_value(&observation()).unwrap();
        value["package_ids"]["foundation_shipping"] =
            json!("retargeted-package-id");
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_PACKAGE_ID_JOIN"
        );

        let mut value = observation_value(&observation()).unwrap();
        value["foundation_shipping"]["provider"]["package_id"] =
            json!("retargeted-package-id");
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_PACKAGE_ID_JOIN"
        );
    }

    #[test]
    fn result_rejects_retained_unit_and_dep_info_mutants() {
        let mut value = observation_value(&observation()).unwrap();
        replace_argv_text(
            &mut value,
            "/foundation_shipping/unit/original_argv",
            "--edition=2024",
            "--edition=2015",
        );
        assert_result_code(&value, "COMPILER_WRAPPER_ARGV_FORM");

        let mut value = observation_value(&observation()).unwrap();
        replace_argv_text(
            &mut value,
            "/foundation_shipping/unit/original_argv",
            "reovim_foundation",
            "retargeted_foundation",
        );
        assert_result_code(&value, "COMPILER_UNIT_IDENTITY");

        let mut value = observation_value(&observation()).unwrap();
        replace_argv_prefix(
            &mut value,
            "/foundation_validation/unit/original_argv",
            "reovim_foundation=",
            "retargeted_foundation=",
        );
        assert_result_code(&value, "COMPILER_EXTERN_EDGE_SET");

        let mut value = observation_value(&observation()).unwrap();
        replace_argv_text(
            &mut value,
            "/foundation_shipping/unit/original_argv",
            "extra-filename=-1111111111111111",
            "extra-filename=-9999999999999999",
        );
        assert_result_code(&value, "COMPILER_EFFECTIVE_ARGV");

        let mut value = observation_value(&observation()).unwrap();
        replace_argv_text(
            &mut value,
            "/foundation_shipping/unit/original_argv",
            "feature=\"default\"",
            "feature=\"changed\"",
        );
        assert_result_code(&value, "COMPILER_FEATURE_SET");

        let mut value = observation_value(&observation()).unwrap();
        replace_argv_text(
            &mut value,
            "/foundation_shipping/unit/original_argv",
            "/repo/kernel/foundation/src/lib.rs",
            "/repo/kernel/foundation/src/bound.rs",
        );
        assert_result_code(&value, "COMPILER_UNIT_IDENTITY");

        let mut value = observation_value(&observation()).unwrap();
        value["foundation_shipping"]["unit"]["effective_argv"]
            .as_array_mut()
            .unwrap()
            .push(json!("--unknown"));
        assert_result_code(&value, "COMPILER_EFFECTIVE_ARGV");

        let mut value = observation_value(&observation()).unwrap();
        value["foundation_shipping"]["unit"]["dep_info_digest"] =
            json!(digest('0'));
        assert_result_code(&value, "COMPILER_RETAINED_DEP_INFO");

        let mut value = observation_value(&observation()).unwrap();
        mutate_dep_info_source(&mut value);
        assert_result_code(&value, "COMPILER_RETAINED_DEP_INFO");

        let mut value = observation_value(&observation()).unwrap();
        let path = "/foundation_shipping/unit/outputs/0/path";
        *value.pointer_mut(path).unwrap() = json!(concat!(
            "/scratch/build-target/build-dir/",
            "x86_64-unknown-linux-gnu/release/deps/",
            "libreovim_foundation-9999999999999999.rlib",
        ));
        assert_result_code(&value, "COMPILER_RETAINED_UNIT_OUTPUTS");
    }

    #[test]
    fn result_rejects_detached_consumer_rlibs() {
        assert_detached_consumer_rlib(
            "foundation_validation",
            "foundation-consumer",
            1,
            "libreovim_foundation_public_consumer-detached.rlib",
        );
        assert_detached_consumer_rlib(
            "kernel_interface_validation",
            "kernel-interface-consumer",
            2,
            "libreovim_kernel_interface_public_consumer-detached.rlib",
        );
    }

    #[test]
    fn result_rejects_kernel_stage_and_transitive_mutants() {
        let mut value = observation_value(&observation()).unwrap();
        value["kernel_interface_shipping"]["cargo"]["artifacts"][0]["fresh"] =
            json!(false);
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_CARGO_ARTIFACT_SET"
        );

        let mut value = observation_value(&observation()).unwrap();
        let probes = value["foundation_shipping"]["probes"].clone();
        value["kernel_interface_shipping"]["probes"] = probes;
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_PROBES"
        );

        let mut value = observation_value(&observation()).unwrap();
        value["kernel_interface_validation"]["transitive_provider_reuse"] =
            Value::Null;
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_PROVIDER_SHAPE"
        );

        let mut value = observation_value(&observation()).unwrap();
        let path = "/kernel_interface_validation/\
                    transitive_provider_reuse/hashed_rlib/content_digest";
        *value.pointer_mut(path).unwrap() = json!(digest('0'));
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_TRANSITIVE_PROVIDER_RELATION"
        );

        let mut value = observation_value(&observation()).unwrap();
        let detached = json!(concat!(
            "/scratch/build-target/",
            "x86_64-unknown-linux-gnu/release/",
            "libreovim_foundation-detached.rlib",
        ));
        value["foundation_shipping"]["cargo"]["artifacts"][0]["filenames"][0] =
            detached.clone();
        value["foundation_shipping"]["provider"]["cargo_artifact"]["filenames"]
            [0] = detached;
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_PROVIDER_RELATION"
        );

        let mut value = observation_value(&observation()).unwrap();
        let detached = json!(concat!(
            "/scratch/build-target/build-dir/",
            "x86_64-unknown-linux-gnu/release/deps/",
            "libreovim_foundation-detached.rlib",
        ));
        value["foundation_validation"]["cargo"]["artifacts"][0]["filenames"]
            [0] = detached.clone();
        let path = "/foundation_validation/provider_reuse/\
                    fresh_artifact/filenames/0";
        *value.pointer_mut(path).unwrap() = detached;
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_PROVIDER_RELATION"
        );

        let mut value = observation_value(&observation()).unwrap();
        let path = "/kernel_interface_validation/\
                    transitive_provider_reuse/fresh_artifact/filenames/0";
        *value.pointer_mut(path).unwrap() = json!(concat!(
            "/scratch/build-target/build-dir/",
            "x86_64-unknown-linux-gnu/release/deps/",
            "libreovim_foundation-detached.rlib",
        ));
        assert_eq!(
            parse_result(&value, &limits()).unwrap_err().code,
            "PRODUCT_WORKLOAD_TRANSITIVE_PROVIDER_RELATION"
        );
    }

    fn assert_result_code(value: &Value, code: &'static str) {
        assert_eq!(parse_result(value, &limits()).unwrap_err().code, code);
    }

    fn replace_argv_text(value: &mut Value, path: &str, old: &str, new: &str) {
        let arguments =
            value.pointer_mut(path).unwrap().as_array_mut().unwrap();
        let argument = arguments
            .iter_mut()
            .find(|argument| argument.as_str() == Some(old))
            .unwrap();
        *argument = json!(new);
    }

    fn replace_argv_prefix(
        value: &mut Value,
        path: &str,
        old: &str,
        new: &str,
    ) {
        let arguments =
            value.pointer_mut(path).unwrap().as_array_mut().unwrap();
        let argument = arguments
            .iter_mut()
            .find(|argument| {
                argument.as_str().is_some_and(|text| text.starts_with(old))
            })
            .unwrap();
        let changed = argument.as_str().unwrap().replacen(old, new, 1);
        *argument = json!(changed);
    }

    fn mutate_dep_info_source(value: &mut Value) {
        let path = "/foundation_shipping/unit/dep_info_text";
        let changed = value.pointer(path).unwrap().as_str().unwrap().replace(
            "/repo/kernel/foundation/src/bound.rs",
            "/repo/kernel/foundation/src/unknown.rs",
        );
        let digest = crate::digest::bytes(changed.as_bytes());
        let bytes = changed.len();
        *value.pointer_mut(path).unwrap() = json!(changed);
        value["foundation_shipping"]["unit"]["dep_info_digest"] = json!(digest);
        value["foundation_shipping"]["unit"]["dep_info_bytes"] = json!(bytes);
    }

    fn assert_detached_consumer_rlib(
        stage: &str,
        closure: &str,
        cargo_index: usize,
        filename: &str,
    ) {
        let mut value = observation_value(&observation()).unwrap();
        let detached =
            format!("/scratch/build-target/{TARGET}/release/{filename}");
        value[stage]["cargo"]["artifacts"][cargo_index]["filenames"][0] =
            json!(detached);
        value[stage]["consumer_rlib"]["path"] = json!(detached);
        let error = parse_result(&value, &limits()).unwrap_err();
        assert_eq!(error.code, "PRODUCT_WORKLOAD_CONSUMER_ARTIFACT");
        assert_eq!(error.subject.as_deref(), Some(closure));
        assert_eq!(error.message, "consumer check failed: top_level_rlib_path");
    }

    fn observation() -> Observation {
        let compiler_substrate = substrate();
        let package_before = package(Phase::BeforeCompiler, SHIPPING_PACKAGE);
        let mut package_after = package(Phase::AfterCompiler, SHIPPING_PACKAGE);
        package_after.raw_output_digest = digest('8');
        let shipping = shipping_build();
        let validation = validation_build(&shipping);
        let kernel_interface_package_before =
            package(Phase::BeforeCompiler, KERNEL_INTERFACE_SHIPPING_PACKAGE);
        let mut kernel_interface_package_after =
            package(Phase::AfterCompiler, KERNEL_INTERFACE_SHIPPING_PACKAGE);
        kernel_interface_package_after.raw_output_digest = digest('9');
        let kernel_interface_shipping = kernel_shipping_build(&shipping);
        let kernel_interface_validation =
            kernel_validation_build(&shipping, &kernel_interface_shipping);
        Observation {
            package_ids: super::PackageIds {
                foundation_shipping: "shipping-package-id".to_owned(),
                foundation_validation: "validation-package-id".to_owned(),
                kernel_interface_shipping: "kernel-shipping-package-id"
                    .to_owned(),
                kernel_interface_validation: "kernel-validation-package-id"
                    .to_owned(),
            },
            compiler_substrate,
            package_before,
            package_after,
            shipping,
            validation,
            kernel_interface_package_before,
            kernel_interface_package_after,
            kernel_interface_shipping,
            kernel_interface_validation,
        }
    }

    fn substrate() -> compiler_substrate::Observation {
        let entries = vec![
            Entry {
                kind: EntryKind::CompilerSupport,
                canonical_path: PathBuf::from(
                    "/compiler/lib/librustc_driver.so",
                ),
                content_digest: digest('1'),
                size_bytes: 10,
            },
            Entry {
                kind: EntryKind::TargetLibdir,
                canonical_path: PathBuf::from(concat!(
                    "/sysroot/lib/rustlib/",
                    "x86_64-unknown-linux-gnu/lib/libstd.rlib",
                )),
                content_digest: digest('2'),
                size_bytes: 20,
            },
        ];
        compiler_substrate::Observation {
            compiler_support_root: PathBuf::from("/compiler/lib"),
            target_libdir: PathBuf::from(concat!(
                "/sysroot/lib/rustlib/",
                "x86_64-unknown-linux-gnu/lib",
            )),
            aggregate_digest: substrate_digest(&entries).unwrap(),
            entries,
            entry_count: 2,
            total_bytes: 30,
        }
    }

    fn package(phase: Phase, package: &str) -> package_capture::Observation {
        let registry = registry();
        let shipping = registry
            .closures
            .iter()
            .find(|closure| closure.package.name == package)
            .unwrap();
        let sources = if package == SHIPPING_PACKAGE {
            source_set(SHIPPING_SOURCES)
        } else {
            source_set(KERNEL_INTERFACE_SHIPPING_SOURCES)
        }
        .into_iter()
        .collect::<Vec<_>>();
        let members =
            super::registered_package_members(shipping, &sources).unwrap();
        let bytes = format!(
            "{}\n",
            members
                .iter()
                .map(|path| path.to_str().unwrap())
                .collect::<Vec<_>>()
                .join("\n")
        );
        package_capture::parse_output(
            phase,
            DirtyMode::Reject,
            package_argv(package),
            bytes.as_bytes(),
            limits().package,
        )
        .unwrap()
    }

    fn shipping_closure_value<'a>(
        registry: &'a mut Value,
        id: &str,
    ) -> &'a mut Value {
        registry["closures"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|closure| closure["id"] == id)
            .unwrap()
    }

    fn shipping_metadata(
        closure: &crate::product_closures::Closure,
    ) -> Metadata {
        let id = "shipping".to_owned();
        let package = Package {
            id: id.clone(),
            name: closure.package.name.clone(),
            version: "0.16.0-dev".to_owned(),
            edition: "2024".to_owned(),
            source: None,
            checksum: None,
            license: Some("AGPL-3.0-only".to_owned()),
            manifest_path: Path::new("/repo").join(&closure.package.manifest),
            targets: vec![Target {
                name: closure.target.name.clone(),
                kind: vec!["lib".to_owned()],
                crate_types: vec!["lib".to_owned()],
                src_path: Path::new("/repo").join(&closure.target.src_path),
            }],
        };
        Metadata {
            workspace_root: PathBuf::from("/repo/product"),
            workspace_members: BTreeSet::from([id.clone()]),
            packages: BTreeMap::from([(id.clone(), package)]),
            nodes: BTreeMap::from([(
                id.clone(),
                Node { id, dependencies: Vec::new() },
            )]),
        }
    }

    fn shipping_build() -> BuildObservation {
        let rlib = file(
            concat!(
                "/scratch/build-target/build-dir/",
                "x86_64-unknown-linux-gnu/release/deps/",
                "libreovim_foundation-1111111111111111.rlib",
            ),
            '3',
            100,
        );
        let rmeta = file(
            concat!(
                "/scratch/build-target/build-dir/",
                "x86_64-unknown-linux-gnu/release/deps/",
                "libreovim_foundation-1111111111111111.rmeta",
            ),
            '4',
            40,
        );
        let top = file(
            concat!(
                "/scratch/build-target/",
                "x86_64-unknown-linux-gnu/release/",
                "libreovim_foundation.rlib",
            ),
            '3',
            100,
        );
        let artifact = foundation_artifact(false, &top.path, &rmeta.path);
        let unit = unit(
            SHIPPING_CLOSURE,
            vec![rlib.clone(), rmeta.clone()],
            Vec::new(),
        );
        let provider = ProviderArtifacts {
            closure: SHIPPING_CLOSURE.to_owned(),
            package_id: "shipping-package-id".to_owned(),
            unit: unit.id.clone(),
            cargo_artifact: artifact.clone(),
            top_level_rlib: top,
            hashed_rlib: rlib,
            rmeta,
            target_dir: PathBuf::from("/scratch/build-target"),
            cargo_home: PathBuf::from("/scratch/cargo-home"),
        };
        BuildObservation {
            closure: SHIPPING_CLOSURE.to_owned(),
            cargo: CargoObservation {
                command_argv: build_argv(SHIPPING_CLOSURE).unwrap(),
                raw_digest: digest('5'),
                stdout_bytes: 100,
                message_count: 2,
                artifacts: vec![artifact],
            },
            probes: (0..4).map(probe).collect(),
            unit,
            provider: Some(provider),
            provider_reuse: None,
            transitive_provider_reuse: None,
            consumer_rlib: None,
        }
    }

    fn validation_build(shipping: &BuildObservation) -> BuildObservation {
        let provider = shipping.provider.as_ref().unwrap();
        let rlib = file(
            concat!(
                "/scratch/build-target/build-dir/",
                "x86_64-unknown-linux-gnu/release/deps/",
                "libreovim_foundation_public_consumer-2222222222222222.rlib",
            ),
            '6',
            80,
        );
        let rmeta = file(
            concat!(
                "/scratch/build-target/build-dir/",
                "x86_64-unknown-linux-gnu/release/deps/",
                "libreovim_foundation_public_consumer-2222222222222222.rmeta",
            ),
            '7',
            30,
        );
        let top = file(
            concat!(
                "/scratch/build-target/",
                "x86_64-unknown-linux-gnu/release/",
                "libreovim_foundation_public_consumer.rlib",
            ),
            '6',
            80,
        );
        let edge = ExternEdge {
            name: "reovim_foundation".to_owned(),
            path: provider.rmeta.path.clone(),
        };
        let unit = unit(
            VALIDATION_CLOSURE,
            vec![rlib, rmeta.clone()],
            vec![edge.clone()],
        );
        let fresh = foundation_artifact(
            true,
            &provider.hashed_rlib.path,
            &provider.rmeta.path,
        );
        let consumer = consumer_artifact(&top.path, &rmeta.path);
        BuildObservation {
            closure: VALIDATION_CLOSURE.to_owned(),
            cargo: CargoObservation {
                command_argv: build_argv(VALIDATION_CLOSURE).unwrap(),
                raw_digest: digest('a'),
                stdout_bytes: 200,
                message_count: 3,
                artifacts: vec![fresh.clone(), consumer],
            },
            probes: Vec::new(),
            unit,
            provider: None,
            provider_reuse: Some(ProviderReuse {
                fresh_artifact: fresh,
                hashed_rlib: provider.hashed_rlib.clone(),
                rmeta: provider.rmeta.clone(),
                extern_edge: edge,
            }),
            transitive_provider_reuse: None,
            consumer_rlib: Some(top),
        }
    }

    fn kernel_shipping_build(
        foundation: &BuildObservation,
    ) -> BuildObservation {
        let foundation_provider = foundation.provider.as_ref().unwrap();
        let rlib = file(
            concat!(
                "/scratch/build-target/build-dir/",
                "x86_64-unknown-linux-gnu/release/deps/",
                "libreovim_kernel_interface-3333333333333333.rlib",
            ),
            'e',
            140,
        );
        let rmeta = file(
            concat!(
                "/scratch/build-target/build-dir/",
                "x86_64-unknown-linux-gnu/release/deps/",
                "libreovim_kernel_interface-3333333333333333.rmeta",
            ),
            'f',
            60,
        );
        let top = file(
            concat!(
                "/scratch/build-target/",
                "x86_64-unknown-linux-gnu/release/",
                "libreovim_kernel_interface.rlib",
            ),
            'e',
            140,
        );
        let edge = ExternEdge {
            name: "reovim_foundation".to_owned(),
            path: foundation_provider.rmeta.path.clone(),
        };
        let unit = unit(
            KERNEL_INTERFACE_SHIPPING_CLOSURE,
            vec![rlib.clone(), rmeta.clone()],
            vec![edge.clone()],
        );
        let fresh_foundation = foundation_artifact(
            true,
            &foundation_provider.hashed_rlib.path,
            &foundation_provider.rmeta.path,
        );
        let own = kernel_artifact(false, &top.path, &rmeta.path);
        let provider = ProviderArtifacts {
            closure: KERNEL_INTERFACE_SHIPPING_CLOSURE.to_owned(),
            package_id: "kernel-shipping-package-id".to_owned(),
            unit: unit.id.clone(),
            cargo_artifact: own.clone(),
            top_level_rlib: top,
            hashed_rlib: rlib,
            rmeta,
            target_dir: PathBuf::from("/scratch/build-target"),
            cargo_home: PathBuf::from("/scratch/cargo-home"),
        };
        BuildObservation {
            closure: KERNEL_INTERFACE_SHIPPING_CLOSURE.to_owned(),
            cargo: CargoObservation {
                command_argv: build_argv(KERNEL_INTERFACE_SHIPPING_CLOSURE)
                    .unwrap(),
                raw_digest: digest('1'),
                stdout_bytes: 300,
                message_count: 3,
                artifacts: vec![fresh_foundation.clone(), own],
            },
            probes: Vec::new(),
            unit,
            provider: Some(provider),
            provider_reuse: Some(ProviderReuse {
                fresh_artifact: fresh_foundation,
                hashed_rlib: foundation_provider.hashed_rlib.clone(),
                rmeta: foundation_provider.rmeta.clone(),
                extern_edge: edge,
            }),
            transitive_provider_reuse: None,
            consumer_rlib: None,
        }
    }

    fn kernel_validation_build(
        foundation: &BuildObservation,
        kernel: &BuildObservation,
    ) -> BuildObservation {
        let foundation_provider = foundation.provider.as_ref().unwrap();
        let kernel_provider = kernel.provider.as_ref().unwrap();
        let rlib = file(
            concat!(
                "/scratch/build-target/build-dir/",
                "x86_64-unknown-linux-gnu/release/deps/",
                "libreovim_kernel_interface_public_consumer-",
                "4444444444444444.rlib",
            ),
            '2',
            90,
        );
        let rmeta = file(
            concat!(
                "/scratch/build-target/build-dir/",
                "x86_64-unknown-linux-gnu/release/deps/",
                "libreovim_kernel_interface_public_consumer-",
                "4444444444444444.rmeta",
            ),
            '3',
            35,
        );
        let top = file(
            concat!(
                "/scratch/build-target/",
                "x86_64-unknown-linux-gnu/release/",
                "libreovim_kernel_interface_public_consumer.rlib",
            ),
            '2',
            90,
        );
        let edge = ExternEdge {
            name: "reovim_kernel_interface".to_owned(),
            path: kernel_provider.rmeta.path.clone(),
        };
        let unit = unit(
            KERNEL_INTERFACE_VALIDATION_CLOSURE,
            vec![rlib, rmeta.clone()],
            vec![edge.clone()],
        );
        let fresh_foundation = foundation_artifact(
            true,
            &foundation_provider.hashed_rlib.path,
            &foundation_provider.rmeta.path,
        );
        let fresh_kernel = kernel_artifact(
            true,
            &kernel_provider.hashed_rlib.path,
            &kernel_provider.rmeta.path,
        );
        let consumer = kernel_consumer_artifact(&top.path, &rmeta.path);
        BuildObservation {
            closure: KERNEL_INTERFACE_VALIDATION_CLOSURE.to_owned(),
            cargo: CargoObservation {
                command_argv: build_argv(KERNEL_INTERFACE_VALIDATION_CLOSURE)
                    .unwrap(),
                raw_digest: digest('4'),
                stdout_bytes: 400,
                message_count: 4,
                artifacts: vec![
                    fresh_foundation.clone(),
                    fresh_kernel.clone(),
                    consumer,
                ],
            },
            probes: Vec::new(),
            unit,
            provider: None,
            provider_reuse: Some(ProviderReuse {
                fresh_artifact: fresh_kernel,
                hashed_rlib: kernel_provider.hashed_rlib.clone(),
                rmeta: kernel_provider.rmeta.clone(),
                extern_edge: edge,
            }),
            transitive_provider_reuse: Some(TransitiveProviderReuse {
                fresh_artifact: fresh_foundation,
                hashed_rlib: foundation_provider.hashed_rlib.clone(),
                rmeta: foundation_provider.rmeta.clone(),
            }),
            consumer_rlib: Some(top),
        }
    }

    fn probe(index: usize) -> WrapperTrace {
        let kind = match index {
            0 | 3 => InvocationKind::VersionProbe,
            1 => InvocationKind::HostProbe,
            2 => InvocationKind::TargetProbe,
            _ => unreachable!(),
        };
        WrapperTrace {
            ordinal: index as u64,
            kind,
            rustc: PathBuf::from(RUSTC),
            original_argv: expected_probe_argv(index),
            effective_argv: expected_probe_effective(index),
            environment: probe_environment(),
            dep_info_path: None,
            status: 0,
            trace_digest: digest('b'),
            trace_bytes: 10,
        }
    }

    fn probe_environment() -> Vec<EnvironmentEntry> {
        let mut values = [
            ("CARGO_BUILD_BUILD_DIR", "/scratch/build-target/build-dir"),
            ("CARGO_HOME", "/scratch/cargo-home"),
            ("CARGO_INCREMENTAL", "0"),
            ("CARGO_NET_OFFLINE", "true"),
            ("LD_LIBRARY_PATH", super::COMPILER_SUPPORT),
            ("CARGO_TARGET_DIR", "/scratch/build-target"),
            ("RUSTC", RUSTC),
            ("TEMP", "/scratch/temp"),
            ("TERM", "dumb"),
            ("TMP", "/scratch/temp"),
            ("TMPDIR", "/scratch/temp"),
        ]
        .into_iter()
        .map(|(name, value)| EnvironmentEntry {
            name: name.to_owned(),
            value: value.to_owned(),
        })
        .collect::<Vec<_>>();
        values.sort();
        values
    }

    fn unit(
        closure: &str,
        outputs: Vec<FileIdentity>,
        externs: Vec<ExternEdge>,
    ) -> CompilerUnit {
        let profile = unit_profile(closure);
        let (original_argv, effective_argv) = unit_argv(&profile, &externs);
        let sources = profile.sources.into_iter().collect::<Vec<_>>();
        let dependencies = sources
            .iter()
            .map(|source| Path::new("/repo").join(source).display().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let dep_info_text =
            format!("{}: {dependencies}\n", outputs[0].path.display());
        CompilerUnit {
            id: format!("{closure}:rustc"),
            closure: closure.to_owned(),
            crate_name: profile.crate_name.to_owned(),
            crate_types: vec!["lib".to_owned()],
            target_triple: TARGET.to_owned(),
            profile: "release".to_owned(),
            features: vec!["default".to_owned()],
            cfgs: vec!["feature=\"default\"".to_owned()],
            original_argv,
            effective_argv,
            dep_info_path: PathBuf::from(profile.dep_root),
            dep_info_digest: crate::digest::bytes(dep_info_text.as_bytes()),
            dep_info_bytes: u64::try_from(dep_info_text.len()).unwrap(),
            dep_info_text,
            sources,
            externs,
            outputs,
            trace_digest: digest('d'),
            trace_bytes: 20,
            status: 0,
        }
    }

    struct UnitFixtureProfile {
        crate_name: &'static str,
        dep_root: &'static str,
        root_source: &'static str,
        sources: BTreeSet<PathBuf>,
        hash: &'static str,
    }

    fn unit_profile(closure: &str) -> UnitFixtureProfile {
        let values = match closure {
            SHIPPING_CLOSURE => (
                "reovim_foundation",
                "/scratch/shipping-dep-info/00000004.d",
                "kernel/foundation/src/lib.rs",
                source_set(SHIPPING_SOURCES),
                "1111111111111111",
            ),
            VALIDATION_CLOSURE => (
                "reovim_foundation_public_consumer",
                "/scratch/validation-dep-info/00000000.d",
                "tests/foundation-public-consumer/src/lib.rs",
                source_set(VALIDATION_SOURCES),
                "2222222222222222",
            ),
            KERNEL_INTERFACE_SHIPPING_CLOSURE => (
                "reovim_kernel_interface",
                concat!(
                    "/scratch/kernel-interface-shipping-dep-info/",
                    "00000000.d"
                ),
                "kernel/interface/src/lib.rs",
                source_set(KERNEL_INTERFACE_SHIPPING_SOURCES),
                "3333333333333333",
            ),
            KERNEL_INTERFACE_VALIDATION_CLOSURE => (
                "reovim_kernel_interface_public_consumer",
                concat!(
                    "/scratch/kernel-interface-validation-dep-info/",
                    "00000000.d"
                ),
                "tests/kernel-interface-public-consumer/src/lib.rs",
                source_set(KERNEL_INTERFACE_VALIDATION_SOURCES),
                "4444444444444444",
            ),
            _ => unreachable!(),
        };
        UnitFixtureProfile {
            crate_name: values.0,
            dep_root: values.1,
            root_source: values.2,
            sources: values.3,
            hash: values.4,
        }
    }

    fn unit_argv(
        profile: &UnitFixtureProfile,
        externs: &[ExternEdge],
    ) -> (Vec<String>, Vec<String>) {
        let out_dir = Path::new("/scratch/build-target/build-dir")
            .join(TARGET)
            .join("release/deps");
        let mut original_argv = vec![
            RUSTC.to_owned(),
            "--crate-name".to_owned(),
            profile.crate_name.to_owned(),
            "--edition=2024".to_owned(),
            Path::new("/repo").join(profile.root_source).display().to_string(),
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
            format!("extra-filename=-{}", profile.hash),
            "--out-dir".to_owned(),
            out_dir.display().to_string(),
            "--target".to_owned(),
            TARGET.to_owned(),
            "-C".to_owned(),
            "strip=debuginfo".to_owned(),
            "-L".to_owned(),
            format!("dependency={}", out_dir.display()),
            "-L".to_owned(),
            "dependency=/scratch/build-target/build-dir/release/deps"
                .to_owned(),
        ];
        if let [edge] = externs {
            original_argv.extend([
                "--extern".to_owned(),
                format!("{}={}", edge.name, edge.path.display()),
            ]);
        }
        let mut effective_argv = original_argv.clone();
        let emit = effective_argv
            .iter()
            .position(|value| value == "--emit=dep-info,metadata,link")
            .unwrap();
        effective_argv[emit] =
            format!("--emit=dep-info={},metadata,link", profile.dep_root);
        effective_argv.extend(["--sysroot".to_owned(), SYSROOT.to_owned()]);
        (original_argv, effective_argv)
    }

    fn foundation_artifact(
        fresh: bool,
        rlib: &std::path::Path,
        rmeta: &std::path::Path,
    ) -> CargoArtifact {
        artifact(
            "shipping-package-id",
            "/repo/kernel/foundation/Cargo.toml",
            "reovim_foundation",
            "/repo/kernel/foundation/src/lib.rs",
            fresh,
            rlib,
            rmeta,
        )
    }

    fn consumer_artifact(
        rlib: &std::path::Path,
        rmeta: &std::path::Path,
    ) -> CargoArtifact {
        artifact(
            "validation-package-id",
            "/repo/tests/foundation-public-consumer/Cargo.toml",
            "reovim_foundation_public_consumer",
            "/repo/tests/foundation-public-consumer/src/lib.rs",
            false,
            rlib,
            rmeta,
        )
    }

    fn kernel_artifact(
        fresh: bool,
        rlib: &std::path::Path,
        rmeta: &std::path::Path,
    ) -> CargoArtifact {
        artifact(
            "kernel-shipping-package-id",
            "/repo/kernel/interface/Cargo.toml",
            "reovim_kernel_interface",
            "/repo/kernel/interface/src/lib.rs",
            fresh,
            rlib,
            rmeta,
        )
    }

    fn kernel_consumer_artifact(
        rlib: &std::path::Path,
        rmeta: &std::path::Path,
    ) -> CargoArtifact {
        artifact(
            "kernel-validation-package-id",
            "/repo/tests/kernel-interface-public-consumer/Cargo.toml",
            "reovim_kernel_interface_public_consumer",
            "/repo/tests/kernel-interface-public-consumer/src/lib.rs",
            false,
            rlib,
            rmeta,
        )
    }

    fn artifact(
        package_id: &str,
        manifest: &str,
        target: &str,
        source: &str,
        fresh: bool,
        rlib: &std::path::Path,
        rmeta: &std::path::Path,
    ) -> CargoArtifact {
        CargoArtifact {
            package_id: package_id.to_owned(),
            manifest_path: PathBuf::from(manifest),
            target_name: target.to_owned(),
            target_kinds: vec!["lib".to_owned()],
            crate_types: vec!["lib".to_owned()],
            source_path: PathBuf::from(source),
            features: vec!["default".to_owned()],
            filenames: vec![rlib.to_path_buf(), rmeta.to_path_buf()],
            fresh,
        }
    }

    fn file(path: &str, digest_char: char, size_bytes: u64) -> FileIdentity {
        FileIdentity {
            path: PathBuf::from(path),
            content_digest: digest(digest_char),
            size_bytes,
        }
    }

    fn digest(value: char) -> String {
        crate::digest::bytes(value.to_string().as_bytes())
    }

    #[test]
    fn fixed_tool_paths_remain_explicit() {
        assert_eq!(CARGO, "/compiler/bin/cargo");
        assert_eq!(RUSTC, "/compiler/bin/rustc");
        assert_eq!(CHECKER, "/checker/reovim-conformance");
        assert_eq!(
            package_argv(SHIPPING_PACKAGE),
            [
                CARGO,
                "package",
                "--list",
                "--quiet",
                "--locked",
                "--offline",
                "--no-metadata",
                "--manifest-path",
                "/repo/product/Cargo.toml",
                "--package",
                SHIPPING_PACKAGE,
            ]
            .map(str::to_owned)
        );
    }
}
