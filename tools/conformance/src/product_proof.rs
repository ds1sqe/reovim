//! Real coordinator for the exact four-stage Product producer.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fmt, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use serde_json::Value;

use crate::{
    compiler_capture::{self, BuildObservation, InvocationKind},
    compiler_substrate, package_capture, prepared_root, product_closures,
    product_preflight, product_source, product_workload, report_build_v2,
    source::Inventory,
    topology::Limits,
};

const SHIPPING_CLOSURE: &str = "foundation-shipping";
const CONSUMER_CLOSURE: &str = "foundation-consumer";
const PUBLIC_RELATION: &str = "foundation-public-api";
const KERNEL_INTERFACE_SHIPPING_CLOSURE: &str = "kernel-interface-shipping";
const KERNEL_INTERFACE_CONSUMER_CLOSURE: &str = "kernel-interface-consumer";
const KERNEL_INTERFACE_PUBLIC_RELATION: &str = "kernel-interface-public-api";
const EXECUTION_ID: &str = "kernel-interface-two-pair-proof";
const SUBSTRATE_ID: &str = "foundation-rustc-substrate";
const HOST_TRIPLE: &str = "x86_64-unknown-linux-gnu";
const TARGET_TRIPLE: &str = "x86_64-unknown-linux-gnu";
const PREPARED_SCRATCH: &str = "/scratch";
const MOUNT_PARENT: &str = "/tmp";
const REGISTRY_RELATIVE: &str = "conformance/product-closures.json";
const REGISTRY_PURPOSE: &str = "Product repository input";

/// Exact caller-owned authorities for one four-stage Product execution.
pub struct Request<'a> {
    pub repository_root: &'a Path,
    pub cargo: &'a Path,
    pub rustc: &'a Path,
    pub checker: &'a Path,
    pub inventory: &'a Inventory,
    pub limits: Limits,
    pub registry_bytes: &'a [u8],
    pub registry_value: &'a Value,
    pub registry: &'a product_closures::Registry,
    pub execution_environment: &'a str,
}

/// Owned active-Product evidence and its report adapters.
#[derive(Debug)]
pub struct Observation {
    pub prepared: prepared_root::Execution,
    pub workload: product_workload::Observation,
    pub product_sources: product_source::TwoPairSourceObservation,
    pub checker_runtime_inputs: Vec<report_build_v2::RuntimeInputIdentity>,
    pub cargo_runtime_inputs: Vec<report_build_v2::RuntimeInputIdentity>,
    pub rustc_runtime_inputs: Vec<report_build_v2::RuntimeInputIdentity>,
    cargo_builds: Vec<report_build_v2::ProductCargoBuildEvidence>,
    compiler_units: Vec<report_build_v2::ProductCompilerUnitEvidence>,
    extern_edges: Vec<report_build_v2::ProductExternEvidence>,
    artifacts: Vec<report_build_v2::ProductArtifactEvidence>,
    provider_artifacts: Vec<report_build_v2::ProductProviderArtifactEvidence>,
    execution: report_build_v2::ProductExecutionEvidence,
}

impl Observation {
    /// Borrows the complete active evidence for Source Role Map construction.
    #[must_use]
    pub fn active(&self) -> report_build_v2::ActiveProductEvidence<'_> {
        report_build_v2::ActiveProductEvidence {
            compiler_substrate_id: SUBSTRATE_ID,
            compiler_host_triple: HOST_TRIPLE,
            compiler_target_triple: TARGET_TRIPLE,
            compiler_substrate: &self.workload.compiler_substrate,
            cargo_builds: &self.cargo_builds,
            compiler_units: &self.compiler_units,
            extern_edges: &self.extern_edges,
            artifacts: &self.artifacts,
            provider_artifacts: &self.provider_artifacts,
            foundation_package_before: &self.workload.package_before,
            foundation_package_after: &self.workload.package_after,
            kernel_interface_package_before: &self
                .workload
                .kernel_interface_package_before,
            kernel_interface_package_after: &self
                .workload
                .kernel_interface_package_after,
            product_sources: &self.product_sources,
            execution: &self.execution,
            process: &self.prepared.process,
        }
    }
}

/// Typed preflight, execution, workload, join, or adapter failure.
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

/// Runs and correlates the complete four-stage Product producer.
///
/// # Errors
///
/// Returns a stable typed failure before any partial evidence is exposed.
pub fn run(request: &Request<'_>) -> Result<Observation, Error> {
    require_prepared_capability()?;
    let preflight = product_preflight::observe(&product_preflight::Request {
        repository_root: request.repository_root,
        cargo: request.cargo,
        rustc: request.rustc,
        checker: request.checker,
        inventory: request.inventory,
        limits: request.limits,
    })
    .map_err(preflight_error)?;
    validate_host_registry(request, &preflight.registry)?;
    let workload_limits = workload_limits(request.limits);
    let registry_input = product_workload::RegistryInput {
        path: preflight.registry.destination.clone(),
        content_digest: preflight.registry.content_digest.clone(),
        size_bytes: preflight.registry.size_bytes,
    };
    let workload_request =
        product_workload::request_value(&registry_input, &workload_limits)
            .map_err(workload_error)?;
    validate_registry_join(
        &workload_request,
        &workload_limits,
        &registry_input,
    )?;
    let plan = prepared_plan(request, &preflight, workload_request)?;
    let prepared = prepared_root::execute(&plan).map_err(prepared_error)?;
    validate_prepared_registry_exposure(&preflight.registry, &prepared)?;
    let workload = product_workload::parse_result(
        &prepared.evidence.workload.value,
        &workload_limits,
        request.registry,
    )
    .map_err(workload_error)?;
    validate_substrate_join(&preflight.compiler, &workload.compiler_substrate)?;
    validate_source_join(
        &preflight.compiler_sources,
        &workload.shipping.unit.sources,
        &workload.validation.unit.sources,
        &workload.kernel_interface_shipping.unit.sources,
        &workload.kernel_interface_validation.unit.sources,
    )?;
    let product_sources =
        analyze_two_pair_source(request, &workload, workload_limits.source)?;
    let checker_runtime_inputs = runtime_inputs(&preflight.runtime.checker)?;
    let cargo_runtime_inputs = runtime_inputs(&preflight.runtime.cargo)?;
    let rustc_runtime_inputs = runtime_inputs(&preflight.runtime.rustc)?;
    let cargo_builds = cargo_build_evidence(&workload)?;
    let compiler_units = compiler_unit_evidence(&workload);
    let extern_edges = extern_evidence(&workload)?;
    let artifacts = artifact_evidence(&workload)?;
    let provider_artifacts = provider_artifact_evidence(&workload)?;
    let execution = execution_evidence(&preflight, &prepared)?;
    Ok(Observation {
        prepared,
        workload,
        product_sources,
        checker_runtime_inputs,
        cargo_runtime_inputs,
        rustc_runtime_inputs,
        cargo_builds,
        compiler_units,
        extern_edges,
        artifacts,
        provider_artifacts,
        execution,
    })
}

fn require_prepared_capability() -> Result<(), Error> {
    match prepared_root::capability_probe() {
        prepared_root::Capability::Ready => Ok(()),
        prepared_root::Capability::Unavailable { code, message } => {
            Err(Error::new(code, message))
        }
    }
}

fn validate_host_registry(
    request: &Request<'_>,
    identity: &product_preflight::FileIdentity,
) -> Result<(), Error> {
    let source = request.repository_root.join(REGISTRY_RELATIVE);
    let destination = Path::new(product_workload::PREPARED_REGISTRY_PATH);
    let size = u64::try_from(request.registry_bytes.len()).map_err(|_| {
        Error::new(
            "PRODUCT_PROOF_REGISTRY_SIZE",
            "raw registry byte count does not fit u64",
        )
    })?;
    let bounded = !request.registry_bytes.is_empty()
        && size <= request.limits.source_file_bytes;
    let exact_identity = identity.source == source
        && identity.destination == destination
        && identity.size_bytes == size
        && identity.content_digest
            == crate::digest::bytes(request.registry_bytes);
    if !bounded || !exact_identity {
        return Err(Error::new(
            "PRODUCT_PROOF_REGISTRY_IDENTITY",
            "raw registry bytes differ from preflight identity",
        ));
    }
    let value = crate::json::parse(
        request.registry_bytes,
        request.limits.json_depth,
        request.limits.json_container_items,
    )
    .map_err(|failure| {
        Error::at(failure.code, failure.path, failure.message)
    })?;
    if &value != request.registry_value {
        return Err(Error::new(
            "PRODUCT_PROOF_REGISTRY_VALUE",
            "raw registry bytes differ from the caller's parsed value",
        ));
    }
    let parsed = product_closures::Registry::parse(
        &value,
        workload_limits(request.limits).registry,
    )
    .map_err(|failure| {
        Error::at(failure.code, failure.path, failure.message)
    })?;
    if &parsed == request.registry {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_PROOF_REGISTRY_TYPED",
        "raw registry bytes differ from the caller's typed registry",
    ))
}

fn validate_prepared_registry_exposure(
    expected: &product_preflight::FileIdentity,
    execution: &prepared_root::Execution,
) -> Result<(), Error> {
    let mut matches = execution
        .evidence
        .exposures
        .iter()
        .filter(|value| value.destination == expected.destination);
    let observed = matches.next().ok_or_else(|| {
        Error::new(
            "PRODUCT_PROOF_REGISTRY_EXPOSURE",
            "prepared registry exposure is absent",
        )
    })?;
    if matches.next().is_some() {
        return Err(Error::new(
            "PRODUCT_PROOF_REGISTRY_EXPOSURE",
            "prepared registry exposure is duplicated",
        ));
    }
    let view = registry_exposure_view(observed)?;
    validate_registry_exposure_identity(expected, &view)
}

#[derive(Clone, Copy)]
struct RegistryExposureView<'a> {
    id: &'a str,
    destination: &'a Path,
    kind: prepared_root::ExposureKind,
    executable: bool,
    purpose: &'a str,
    device_identity: (Option<u64>, Option<u64>),
    mount_policy: [bool; 4],
    entry_count: usize,
    total_bytes: u64,
    root_relative_path: &'a Path,
    root_kind: prepared_root::InventoryEntryKind,
    root_size_bytes: u64,
    root_content_digest: Option<&'a str>,
    root_device_identity: Option<u64>,
}

fn registry_exposure_view(
    observed: &prepared_root::ExposureObservation,
) -> Result<RegistryExposureView<'_>, Error> {
    prepared_root::validate_inventory_identity(&observed.inventory)
        .map_err(prepared_error)?;
    let [root] = observed.inventory.entries.as_slice() else {
        return Err(Error::new(
            "PRODUCT_PROOF_REGISTRY_EXPOSURE",
            "prepared registry inventory is not one file root",
        ));
    };
    Ok(RegistryExposureView {
        id: &observed.id,
        destination: &observed.destination,
        kind: observed.kind,
        executable: observed.executable,
        purpose: &observed.purpose,
        device_identity: (observed.device_major, observed.device_minor),
        mount_policy: [
            observed.mount_flags.read_only(),
            observed.mount_flags.nosuid(),
            observed.mount_flags.nodev(),
            observed.mount_flags.noexec(),
        ],
        entry_count: observed.inventory.entry_count,
        total_bytes: observed.inventory.total_bytes,
        root_relative_path: &root.relative_path,
        root_kind: root.kind,
        root_size_bytes: root.size_bytes,
        root_content_digest: root.content_digest.as_deref(),
        root_device_identity: root.device_identity,
    })
}

fn validate_registry_exposure_identity(
    expected: &product_preflight::FileIdentity,
    observed: &RegistryExposureView<'_>,
) -> Result<(), Error> {
    let id = format!("file:{}", expected.destination.display());
    let exact = observed.id == id
        && observed.destination
            == Path::new(product_workload::PREPARED_REGISTRY_PATH)
        && observed.kind == prepared_root::ExposureKind::File
        && !observed.executable
        && observed.purpose == REGISTRY_PURPOSE
        && observed.device_identity == (None, None)
        && observed.mount_policy == [true; 4]
        && observed.entry_count == 1
        && observed.total_bytes == expected.size_bytes
        && observed.root_relative_path == Path::new(".")
        && observed.root_kind == prepared_root::InventoryEntryKind::File
        && observed.root_size_bytes == expected.size_bytes
        && observed.root_content_digest
            == Some(expected.content_digest.as_str())
        && observed.root_device_identity.is_none();
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_PROOF_REGISTRY_EXPOSURE",
        "prepared registry exposure differs from preflight identity",
    ))
}

fn validate_registry_join(
    value: &Value,
    limits: &product_workload::Limits,
    expected: &product_workload::RegistryInput,
) -> Result<(), Error> {
    let parsed =
        product_workload::parse_request(value).map_err(workload_error)?;
    if parsed.limits == *limits && parsed.registry == *expected {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_PROOF_REGISTRY_JOIN",
        "workload request differs from the caller's typed registry",
    ))
}

fn prepared_plan(
    request: &Request<'_>,
    preflight: &product_preflight::Observation,
    workload_request: Value,
) -> Result<prepared_root::Plan, Error> {
    let mount_parent = canonical_mount_parent()?;
    let product = request.limits.product_execution;
    let inventory_entries = usize::from(
        product
            .compiler_substrate_entry_count
            .max(product.exposure_entry_count),
    );
    Ok(prepared_root::Plan {
        repository_root: request.repository_root.to_path_buf(),
        mount_parent,
        checker_executable: request.checker.to_path_buf(),
        execution_environment: request.execution_environment.to_owned(),
        workload_request,
        exposures: preflight.exposures.iter().map(prepared_exposure).collect(),
        inventory_limits: prepared_root::InventoryLimits {
            path_bytes: request.limits.path_bytes,
            entry_count: inventory_entries,
            total_bytes: u64::from(product.compiler_substrate_total_bytes),
        },
        scratch_limits: prepared_root::ScratchLimits {
            size_bytes: u64::from(product.scratch_bytes),
            inode_count: u64::from(product.scratch_inodes),
        },
        resource_limits: prepared_root::ResourceLimits {
            address_space_bytes: product.process_address_space_bytes,
            cpu_seconds: u64::from(product.process_cpu_seconds),
            open_files: u64::from(product.process_open_files),
            output_file_bytes: u64::from(product.process_output_file_bytes),
            core_dump_bytes: u64::from(product.core_dump_bytes),
        },
        dispatch_bounds: prepared_root::DispatchBounds {
            config_bytes: request.limits.config_bytes,
            result_bytes: request.limits.metadata_stdout_bytes,
            json_depth: request.limits.json_depth,
            json_items: request.limits.json_container_items,
            execution_deadline: Duration::from_millis(
                request.limits.child_deadline_ms,
            ),
            supervision_deadline: Duration::from_millis(
                request.limits.child_supervision_deadline_ms,
            ),
        },
        workload_result_bytes: request.limits.metadata_stdout_bytes,
        descendant_reap_limit: product.compiler_unit_count,
    })
}

fn canonical_mount_parent() -> Result<PathBuf, Error> {
    let path = Path::new(MOUNT_PARENT);
    let metadata = fs::symlink_metadata(path).map_err(|failure| {
        Error::at(
            "PRODUCT_PROOF_MOUNT_PARENT",
            MOUNT_PARENT,
            failure.to_string(),
        )
    })?;
    let canonical = fs::canonicalize(path).map_err(|failure| {
        Error::at(
            "PRODUCT_PROOF_MOUNT_PARENT",
            MOUNT_PARENT,
            failure.to_string(),
        )
    })?;
    if metadata.is_dir()
        && !metadata.file_type().is_symlink()
        && canonical == path
    {
        return Ok(canonical);
    }
    Err(Error::at(
        "PRODUCT_PROOF_MOUNT_PARENT",
        MOUNT_PARENT,
        "fixed mount parent is not its canonical real directory",
    ))
}

fn prepared_exposure(
    value: &product_preflight::ExposureSpec,
) -> prepared_root::ExposurePlan {
    prepared_root::ExposurePlan {
        id: value.id.clone(),
        source: value.source.clone(),
        destination: value.destination.clone(),
        kind: prepared_kind(value.kind),
        executable: value.executable,
        purpose: value.purpose.clone(),
    }
}

const fn prepared_kind(
    value: product_preflight::ExposureKind,
) -> prepared_root::ExposureKind {
    match value {
        product_preflight::ExposureKind::File => {
            prepared_root::ExposureKind::File
        }
        product_preflight::ExposureKind::Directory => {
            prepared_root::ExposureKind::Directory
        }
        product_preflight::ExposureKind::CharacterDevice => {
            prepared_root::ExposureKind::CharacterDevice
        }
    }
}

fn workload_limits(limits: Limits) -> product_workload::Limits {
    let product = limits.product_execution;
    let process = limits.process();
    let package_output =
        process.stdout_bytes.min(product.package_list_bytes as usize);
    let compiler_output =
        process.stdout_bytes.min(product.cargo_json_bytes as usize);
    let package_process = crate::process::ProcessLimits {
        stdout_bytes: package_output,
        ..process
    };
    let compiler_process = crate::process::ProcessLimits {
        stdout_bytes: compiler_output,
        ..process
    };
    product_workload::Limits {
        registry: product_closures::ParseLimits {
            path_bytes: limits.path_bytes,
            closure_items: product.compiler_unit_count,
            package_members: product.package_member_count,
            public_items: product.public_item_count,
        },
        metadata_process: process,
        metadata_json_depth: limits.json_depth,
        metadata_json_items: limits.json_container_items,
        package: package_capture::Limits {
            process: package_process,
            output_bytes: package_output,
            member_count: product.package_member_count,
            path_bytes: limits.path_bytes,
        },
        compiler: compiler_capture::Limits {
            process: compiler_process,
            path_bytes: limits.path_bytes,
            cargo_json_bytes: compiler_output,
            cargo_message_count: usize::from(product.cargo_message_count),
            wrapper_trace_bytes: product.wrapper_trace_bytes as usize,
            wrapper_argv_items: product.wrapper_argv_items,
            wrapper_argv_bytes: product.wrapper_argv_bytes as usize,
            wrapper_trace_count: product.compiler_unit_count,
            dep_info_file_bytes: product.dep_info_file_bytes as usize,
            dep_info_total_bytes: product.dep_info_total_bytes as usize,
            compiler_input_count: usize::from(product.compiler_input_count),
            artifact_count: usize::from(product.artifact_count),
            artifact_total_bytes: u64::from(product.artifact_total_bytes),
            json_depth: limits.json_depth,
            json_items: limits.json_container_items,
        },
        substrate: compiler_substrate::Limits {
            path_bytes: limits.path_bytes,
            entry_count: usize::from(product.compiler_substrate_entry_count),
            total_bytes: u64::from(product.compiler_substrate_total_bytes),
        },
        source: product_source::AnalysisLimits {
            source_files: usize::from(product.compiler_input_count),
            source_file_bytes: limits.source_file_bytes,
            source_total_bytes: limits.source_total_bytes,
            syntax_nodes: limits.syntax_item_expr_type_visits,
        },
        result_bytes: limits.metadata_stdout_bytes,
        result_json_depth: limits.json_depth,
        result_json_items: limits.json_container_items,
    }
}

fn validate_substrate_join(
    expected: &product_preflight::CompilerLayout,
    observed: &compiler_substrate::Observation,
) -> Result<(), Error> {
    let mut entries = expected
        .compiler_support_files
        .iter()
        .map(|file| {
            substrate_entry(
                file,
                compiler_substrate::EntryKind::CompilerSupport,
            )
        })
        .chain(expected.target_libdir_files.iter().map(|file| {
            substrate_entry(file, compiler_substrate::EntryKind::TargetLibdir)
        }))
        .collect::<Vec<_>>();
    entries.sort();
    let mut actual = observed.entries.clone();
    actual.sort();
    let total = entries.iter().try_fold(0_u64, |sum, entry| {
        sum.checked_add(entry.size_bytes).ok_or_else(|| {
            Error::new(
                "PRODUCT_PROOF_SUBSTRATE_JOIN",
                "preflight substrate byte count overflow",
            )
        })
    })?;
    let exact = observed.compiler_support_root
        == expected.prepared_compiler_support_root
        && observed.target_libdir == expected.prepared_target_libdir
        && observed.entry_count == entries.len()
        && observed.total_bytes == total
        && actual == entries;
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_PROOF_SUBSTRATE_JOIN",
        "prepared compiler substrate differs from host preflight",
    ))
}

fn substrate_entry(
    file: &product_preflight::FileIdentity,
    kind: compiler_substrate::EntryKind,
) -> compiler_substrate::Entry {
    compiler_substrate::Entry {
        kind,
        canonical_path: file.destination.clone(),
        content_digest: file.content_digest.clone(),
        size_bytes: file.size_bytes,
    }
}

fn validate_source_join(
    expected: &product_preflight::CompilerSourceSelection,
    foundation_shipping: &[PathBuf],
    foundation_consumer: &[PathBuf],
    kernel_interface_shipping: &[PathBuf],
    kernel_interface_consumer: &[PathBuf],
) -> Result<(), Error> {
    let foundation_shipping_set =
        foundation_shipping.iter().cloned().collect::<BTreeSet<_>>();
    let foundation_consumer_set =
        foundation_consumer.iter().cloned().collect::<BTreeSet<_>>();
    let kernel_shipping_set =
        kernel_interface_shipping.iter().cloned().collect::<BTreeSet<_>>();
    let kernel_consumer_set =
        kernel_interface_consumer.iter().cloned().collect::<BTreeSet<_>>();
    let exact = foundation_shipping.len() == foundation_shipping_set.len()
        && foundation_consumer.len() == foundation_consumer_set.len()
        && kernel_interface_shipping.len() == kernel_shipping_set.len()
        && kernel_interface_consumer.len() == kernel_consumer_set.len()
        && foundation_shipping_set == expected.foundation_shipping
        && foundation_consumer_set == expected.foundation_consumer
        && kernel_shipping_set == expected.kernel_interface_shipping
        && kernel_consumer_set == expected.kernel_interface_consumer;
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_PROOF_SOURCE_JOIN",
        "compiler dep-info sources differ from preflight selection",
    ))
}

fn analyze_two_pair_source(
    request: &Request<'_>,
    workload: &product_workload::Observation,
    limits: product_source::AnalysisLimits,
) -> Result<product_source::TwoPairSourceObservation, Error> {
    let foundation_shipping =
        registry_closure(request.registry, SHIPPING_CLOSURE)?;
    let foundation_consumer =
        registry_closure(request.registry, CONSUMER_CLOSURE)?;
    let foundation_relation =
        registry_relation(request.registry, PUBLIC_RELATION)?;
    let kernel_shipping =
        registry_closure(request.registry, KERNEL_INTERFACE_SHIPPING_CLOSURE)?;
    let kernel_consumer =
        registry_closure(request.registry, KERNEL_INTERFACE_CONSUMER_CLOSURE)?;
    let kernel_relation =
        registry_relation(request.registry, KERNEL_INTERFACE_PUBLIC_RELATION)?;
    let foundation_shipping_sources = source_set(&workload.shipping);
    let foundation_consumer_sources = source_set(&workload.validation);
    let kernel_shipping_sources =
        source_set(&workload.kernel_interface_shipping);
    let kernel_consumer_sources =
        source_set(&workload.kernel_interface_validation);
    let foundation_provider =
        workload.shipping.provider.as_ref().ok_or_else(|| {
            Error::new(
                "PRODUCT_PROOF_COMPILER_JOIN",
                "Foundation shipping provider evidence is absent",
            )
        })?;
    let kernel_reuse =
        workload.kernel_interface_shipping.provider_reuse.as_ref().ok_or_else(
            || {
                Error::new(
                    "PRODUCT_PROOF_COMPILER_JOIN",
                    "Kernel Interface Foundation extern evidence is absent",
                )
            },
        )?;
    product_source::analyze_two_pairs(&product_source::TwoPairAnalysisRequest {
        foundation: product_source::AnalysisRequest {
            root: request.repository_root,
            shipping: foundation_shipping,
            consumer: foundation_consumer,
            relation: foundation_relation,
            shipping_sources: &foundation_shipping_sources,
            consumer_sources: &foundation_consumer_sources,
            inventory: &request.inventory.files,
            limits,
        },
        kernel_interface: product_source::AnalysisRequest {
            root: request.repository_root,
            shipping: kernel_shipping,
            consumer: kernel_consumer,
            relation: kernel_relation,
            shipping_sources: &kernel_shipping_sources,
            consumer_sources: &kernel_consumer_sources,
            inventory: &request.inventory.files,
            limits,
        },
        compiler_join: product_source::CompilerJoin {
            consumer_closure: &workload.kernel_interface_shipping.closure,
            extern_name: &kernel_reuse.extern_edge.name,
            provider_closure: &foundation_provider.closure,
            provider_relation: &foundation_relation.id,
            extern_path: &kernel_reuse.extern_edge.path,
            provider_rmeta_path: &foundation_provider.rmeta.path,
        },
    })
    .map_err(source_error)
}

fn source_set(build: &BuildObservation) -> BTreeSet<PathBuf> {
    build.unit.sources.iter().cloned().collect()
}

fn registry_closure<'a>(
    registry: &'a product_closures::Registry,
    id: &str,
) -> Result<&'a product_closures::Closure, Error> {
    registry.closures.iter().find(|closure| closure.id == id).ok_or_else(|| {
        Error::at(
            "PRODUCT_PROOF_REGISTRY_JOIN",
            id,
            "required Product closure is absent",
        )
    })
}

fn registry_relation<'a>(
    registry: &'a product_closures::Registry,
    id: &str,
) -> Result<&'a product_closures::PublicConsumerRelation, Error> {
    registry
        .public_consumer_relations
        .iter()
        .find(|relation| relation.id == id)
        .ok_or_else(|| {
            Error::at(
                "PRODUCT_PROOF_REGISTRY_JOIN",
                id,
                "required Product public relation is absent",
            )
        })
}

fn runtime_inputs(
    observed: &crate::runtime_closure::Observation,
) -> Result<Vec<report_build_v2::RuntimeInputIdentity>, Error> {
    if observed.mappings.is_empty() {
        return Err(Error::new(
            "PRODUCT_PROOF_RUNTIME_INPUT_EMPTY",
            "active Product tool has no direct host runtime input",
        ));
    }
    Ok(observed
        .mappings
        .iter()
        .map(|value| report_build_v2::RuntimeInputIdentity {
            canonical_path: value.source.clone(),
            prepared_path: value.destination.clone(),
            content_digest: value.content_digest.clone(),
            size_bytes: value.size_bytes,
        })
        .collect())
}

fn cargo_build_evidence(
    observed: &product_workload::Observation,
) -> Result<Vec<report_build_v2::ProductCargoBuildEvidence>, Error> {
    let foundation = (
        observed.shipping.unit.crate_name.as_str(),
        observed.shipping.unit.id.as_str(),
    );
    let kernel_interface = (
        observed.kernel_interface_shipping.unit.crate_name.as_str(),
        observed.kernel_interface_shipping.unit.id.as_str(),
    );
    Ok(vec![
        cargo_build(&observed.shipping, &[])?,
        cargo_build(&observed.validation, &[foundation])?,
        cargo_build(&observed.kernel_interface_shipping, &[foundation])?,
        cargo_build(
            &observed.kernel_interface_validation,
            &[foundation, kernel_interface],
        )?,
    ])
}

fn cargo_build(
    observed: &BuildObservation,
    provider_units: &[(&str, &str)],
) -> Result<report_build_v2::ProductCargoBuildEvidence, Error> {
    let message_count =
        u64::try_from(observed.cargo.message_count).map_err(|_| {
            Error::new(
                "PRODUCT_PROOF_CARGO_COUNT",
                "Cargo message count does not fit u64",
            )
        })?;
    let probes = observed
        .probes
        .iter()
        .map(probe_evidence)
        .collect::<Result<Vec<_>, _>>()?;
    let artifact_events = observed
        .cargo
        .artifacts
        .iter()
        .map(|artifact| {
            cargo_artifact_event(observed, artifact, provider_units)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let fresh_provider_units =
        provider_units.iter().map(|(_, unit)| (*unit).to_owned()).collect();
    Ok(report_build_v2::ProductCargoBuildEvidence {
        closure: observed.closure.clone(),
        command_argv: observed.cargo.command_argv.clone(),
        raw_output_digest: observed.cargo.raw_digest.clone(),
        stdout_bytes: observed.cargo.stdout_bytes,
        message_count,
        compiled_units: vec![observed.unit.id.clone()],
        fresh_provider_units,
        artifact_events,
        probes,
    })
}

fn cargo_artifact_event(
    observed: &BuildObservation,
    artifact: &compiler_capture::CargoArtifact,
    provider_units: &[(&str, &str)],
) -> Result<report_build_v2::ProductCargoArtifactEventEvidence, Error> {
    let unit = if artifact.target_name == observed.unit.crate_name {
        observed.unit.id.clone()
    } else {
        provider_units
            .iter()
            .find(|(target, _)| *target == artifact.target_name)
            .map(|(_, unit)| (*unit).to_owned())
            .ok_or_else(|| {
                Error::new(
                    "PRODUCT_PROOF_CARGO_ARTIFACT_UNIT",
                    "Cargo event does not map to its compiler unit",
                )
            })?
    };
    Ok(report_build_v2::ProductCargoArtifactEventEvidence {
        unit,
        fresh: artifact.fresh,
        filenames: artifact.filenames.clone(),
    })
}

fn provider_artifact_evidence(
    observed: &product_workload::Observation,
) -> Result<Vec<report_build_v2::ProductProviderArtifactEvidence>, Error> {
    let foundation = provider_artifact(&observed.shipping)?;
    let kernel_interface =
        provider_artifact(&observed.kernel_interface_shipping)?;
    Ok(vec![foundation, kernel_interface])
}

fn provider_artifact(
    observed: &BuildObservation,
) -> Result<report_build_v2::ProductProviderArtifactEvidence, Error> {
    let provider = observed.provider.as_ref().ok_or_else(|| {
        Error::at(
            "PRODUCT_PROOF_PROVIDER_ARTIFACT",
            &observed.closure,
            "shipping workload omitted its provider artifact relation",
        )
    })?;
    Ok(report_build_v2::ProductProviderArtifactEvidence {
        unit: provider.unit.clone(),
        top_level_rlib: product_file_identity(&provider.top_level_rlib),
        hashed_rlib: product_file_identity(&provider.hashed_rlib),
        rmeta: product_file_identity(&provider.rmeta),
    })
}

fn product_file_identity(
    value: &compiler_capture::FileIdentity,
) -> report_build_v2::ProductFileIdentityEvidence {
    report_build_v2::ProductFileIdentityEvidence {
        path: value.path.clone(),
        content_digest: value.content_digest.clone(),
        size_bytes: value.size_bytes,
    }
}

fn probe_evidence(
    observed: &compiler_capture::WrapperTrace,
) -> Result<report_build_v2::ProductProbeEvidence, Error> {
    let kind = match observed.kind {
        InvocationKind::VersionProbe => {
            report_build_v2::ProductProbeKind::Version
        }
        InvocationKind::HostProbe => report_build_v2::ProductProbeKind::Host,
        InvocationKind::TargetProbe => {
            report_build_v2::ProductProbeKind::Target
        }
        InvocationKind::CompilerUnit => {
            return Err(Error::new(
                "PRODUCT_PROOF_PROBE_KIND",
                "Cargo probe set contains a compiler unit",
            ));
        }
    };
    Ok(report_build_v2::ProductProbeEvidence {
        kind,
        original_argv: observed.original_argv.clone(),
        effective_argv: observed.effective_argv.clone(),
        trace_digest: observed.trace_digest.clone(),
        trace_bytes: observed.trace_bytes,
        status: observed.status,
    })
}

fn compiler_unit_evidence(
    observed: &product_workload::Observation,
) -> Vec<report_build_v2::ProductCompilerUnitEvidence> {
    [
        &observed.shipping.unit,
        &observed.validation.unit,
        &observed.kernel_interface_shipping.unit,
        &observed.kernel_interface_validation.unit,
    ]
    .into_iter()
    .map(|unit| report_build_v2::ProductCompilerUnitEvidence {
        id: unit.id.clone(),
        closure: unit.closure.clone(),
        features: unit.features.clone(),
        cfgs: unit.cfgs.clone(),
        original_argv: unit.original_argv.clone(),
        effective_argv: unit.effective_argv.clone(),
        dep_info_path: unit.dep_info_path.clone(),
        dep_info_digest: unit.dep_info_digest.clone(),
        dep_info_bytes: unit.dep_info_bytes,
        trace_digest: unit.trace_digest.clone(),
        trace_bytes: unit.trace_bytes,
        status: unit.status,
    })
    .collect()
}

fn extern_evidence(
    observed: &product_workload::Observation,
) -> Result<Vec<report_build_v2::ProductExternEvidence>, Error> {
    Ok(vec![
        extern_edge(&observed.validation, &observed.shipping)?,
        extern_edge(&observed.kernel_interface_shipping, &observed.shipping)?,
        extern_edge(
            &observed.kernel_interface_validation,
            &observed.kernel_interface_shipping,
        )?,
    ])
}

fn extern_edge(
    consumer: &BuildObservation,
    provider: &BuildObservation,
) -> Result<report_build_v2::ProductExternEvidence, Error> {
    let reuse = consumer.provider_reuse.as_ref().ok_or_else(|| {
        Error::at(
            "PRODUCT_PROOF_EXTERN_JOIN",
            &consumer.closure,
            "consumer build lacks its direct provider reuse",
        )
    })?;
    Ok(report_build_v2::ProductExternEvidence {
        consumer_unit: consumer.unit.id.clone(),
        extern_name: reuse.extern_edge.name.clone(),
        provider_unit: provider.unit.id.clone(),
        provider_path: reuse.extern_edge.path.clone(),
    })
}

fn artifact_evidence(
    observed: &product_workload::Observation,
) -> Result<Vec<report_build_v2::ProductArtifactEvidence>, Error> {
    let mut values = Vec::new();
    for unit in [
        &observed.shipping.unit,
        &observed.validation.unit,
        &observed.kernel_interface_shipping.unit,
        &observed.kernel_interface_validation.unit,
    ] {
        for artifact in &unit.outputs {
            values.push(report_build_v2::ProductArtifactEvidence {
                unit: unit.id.clone(),
                path: artifact.path.clone(),
                content_digest: artifact.content_digest.clone(),
                size_bytes: artifact.size_bytes,
                crate_type: artifact_kind(&artifact.path)?.to_owned(),
            });
        }
    }
    values.sort_by(|left, right| {
        (&left.unit, &left.crate_type).cmp(&(&right.unit, &right.crate_type))
    });
    Ok(values)
}

fn artifact_kind(path: &Path) -> Result<&'static str, Error> {
    match path.extension().and_then(OsStr::to_str) {
        Some("rlib") => Ok("rlib"),
        Some("rmeta") => Ok("rmeta"),
        _ => Err(Error::at(
            "PRODUCT_PROOF_ARTIFACT_KIND",
            path.display().to_string(),
            "compiler artifact is not rlib or rmeta",
        )),
    }
}

fn execution_evidence(
    preflight: &product_preflight::Observation,
    prepared: &prepared_root::Execution,
) -> Result<report_build_v2::ProductExecutionEvidence, Error> {
    let evidence = &prepared.evidence;
    validate_prepared_evidence(evidence)?;
    let exposures = exposure_evidence(&preflight.exposures, evidence)?;
    let network = &evidence.network_after;
    let sockets = network
        .non_packet_socket_count
        .checked_add(network.packet_socket_count)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| {
            Error::new(
                "PRODUCT_PROOF_NETWORK_COUNT",
                "prepared network socket count overflow",
            )
        })?;
    if network.ipv4_route_count != 0 || network.ipv6_route_count != 0 {
        return Err(Error::new(
            "PRODUCT_PROOF_NETWORK_JOIN",
            "prepared Product network contains a route",
        ));
    }
    let namespaces = &evidence.namespaces;
    let scratch = &evidence.scratch;
    let cleanup = &evidence.cleanup;
    Ok(report_build_v2::ProductExecutionEvidence {
        id: EXECUTION_ID.to_owned(),
        environment: evidence.execution_environment.clone(),
        exposures,
        namespaces: report_build_v2::ProductNamespaceEvidence {
            user: namespaces.user_private(),
            mount: namespaces.mount_private(),
            network: namespaces.network_private(),
            pid: namespaces.pid_private(),
            uid_map: namespaces.uid_map.clone(),
            gid_map: namespaces.gid_map.clone(),
            setgroups_denied: namespaces.setgroups_denied(),
            private_propagation: namespaces.private_propagation(),
            pid_one_reaper: namespaces.pid_one(),
            parent_death_signal: namespaces.parent_death_signal(),
        },
        network: report_build_v2::ProductNetworkEvidence {
            interfaces: network.interfaces.clone(),
            routes: Vec::new(),
            kernel_route_count: u64::try_from(network.kernel_route_count)
                .map_err(|failure| {
                    Error::new(
                        "PRODUCT_PROOF_NETWORK_COUNT",
                        failure.to_string(),
                    )
                })?,
            socket_count: sockets,
            policy_observation_digest: network
                .policy_observation_digest
                .clone(),
            policy_basis: network.policy_basis.clone(),
            external_carrier_absent: network.external_carrier_absent,
        },
        scratch: report_build_v2::ProductScratchEvidence {
            mountpoint: PathBuf::from(PREPARED_SCRATCH),
            size_limit_bytes: scratch.size_limit_bytes,
            inode_limit: scratch.inode_limit,
            allocated_bytes: scratch.allocated_bytes,
            inode_count: scratch.inode_count,
            logical_bytes: scratch.logical_bytes,
            owner_only: scratch.mode_owner_only,
            nodev: scratch.mount_flags.nodev(),
            noexec: scratch.mount_flags.noexec(),
            nosuid: scratch.mount_flags.nosuid(),
        },
        process_group_identity_validated: prepared
            .process
            .group_identity_validated,
        process_group_absent_after_reap: prepared
            .process
            .group_absent_after_reap,
        cleanup: report_build_v2::ProductCleanupEvidence {
            processes_absent: cleanup.descendants_reaped(),
            readers_joined: cleanup.readers_joined(),
            namespace_absent: cleanup.namespace_absent(),
            mount_absent: cleanup.mount_absent(),
            mountpoint_identity_preserved: cleanup
                .mountpoint_identity_preserved(),
            underlying_directory_empty: cleanup.underlying_directory_empty(),
            source_identity_preserved: cleanup.source_identity_preserved(),
            tool_identity_preserved: cleanup.tool_identity_preserved(),
        },
    })
}

fn validate_prepared_evidence(
    value: &prepared_root::Evidence,
) -> Result<(), Error> {
    let cleanup = &value.cleanup;
    let complete = value.namespaces.complete()
        && value.network_before == value.network_after
        && value.workload.status == "passed"
        && cleanup.descendants_reaped()
        && cleanup.readers_joined()
        && cleanup.proc_unmounted()
        && cleanup.exposures_unmounted()
        && cleanup.scratch_unmounted()
        && cleanup.source_identity_preserved()
        && cleanup.tool_identity_preserved()
        && cleanup.namespace_absent()
        && cleanup.mount_absent()
        && cleanup.mountpoint_identity_preserved()
        && cleanup.underlying_directory_empty();
    if complete {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_PROOF_PREPARED_EVIDENCE",
        "prepared-root evidence is incomplete or changed",
    ))
}

fn exposure_evidence(
    planned: &[product_preflight::ExposureSpec],
    prepared: &prepared_root::Evidence,
) -> Result<Vec<report_build_v2::ProductExposureEvidence>, Error> {
    let joins = prepared
        .exposures
        .iter()
        .map(|value| ExposureJoin {
            id: &value.id,
            destination: &value.destination,
            kind: value.kind,
            executable: value.executable,
            purpose: &value.purpose,
        })
        .collect::<Vec<_>>();
    let matched = join_exposures(planned, &joins)?;
    let mut output = Vec::with_capacity(matched.len());
    for (value, plan) in prepared.exposures.iter().zip(matched) {
        output.push(report_build_v2::ProductExposureEvidence {
            source: plan.source.clone(),
            destination: value.destination.clone(),
            kind: report_exposure_kind(value.kind),
            read_only: value.mount_flags.read_only(),
            nosuid: value.mount_flags.nosuid(),
            nodev: value.mount_flags.nodev(),
            noexec: value.mount_flags.noexec(),
            purpose: value.purpose.clone(),
            inventory_digest: value.inventory.aggregate_digest.clone(),
            inventory_entry_count: value.inventory.entry_count,
            inventory_total_bytes: value.inventory.total_bytes,
            inventory_entries: value.inventory.entries.clone(),
        });
    }
    output.sort_by(|left, right| left.destination.cmp(&right.destination));
    Ok(output)
}

struct ExposureJoin<'a> {
    id: &'a str,
    destination: &'a Path,
    kind: prepared_root::ExposureKind,
    executable: bool,
    purpose: &'a str,
}

fn join_exposures<'a>(
    planned: &'a [product_preflight::ExposureSpec],
    observed: &[ExposureJoin<'_>],
) -> Result<Vec<&'a product_preflight::ExposureSpec>, Error> {
    let mut by_id = BTreeMap::new();
    for value in planned {
        if by_id.insert(value.id.as_str(), value).is_some() {
            return Err(Error::at(
                "PRODUCT_PROOF_EXPOSURE_JOIN",
                &value.id,
                "preflight exposure ID is duplicated",
            ));
        }
    }
    let mut matched = Vec::with_capacity(observed.len());
    for value in observed {
        let plan = by_id.remove(value.id).ok_or_else(|| {
            Error::at(
                "PRODUCT_PROOF_EXPOSURE_JOIN",
                value.id,
                "prepared exposure is absent from preflight",
            )
        })?;
        let exact = value.destination == plan.destination
            && value.kind == prepared_kind(plan.kind)
            && value.executable == plan.executable
            && value.purpose == plan.purpose;
        if !exact {
            return Err(Error::at(
                "PRODUCT_PROOF_EXPOSURE_JOIN",
                value.id,
                "prepared exposure differs from preflight",
            ));
        }
        matched.push(plan);
    }
    if by_id.is_empty() {
        return Ok(matched);
    }
    Err(Error::new(
        "PRODUCT_PROOF_EXPOSURE_JOIN",
        "preflight exposure is absent from prepared evidence",
    ))
}

const fn report_exposure_kind(
    value: prepared_root::ExposureKind,
) -> report_build_v2::ProductExposureKind {
    match value {
        prepared_root::ExposureKind::File => {
            report_build_v2::ProductExposureKind::File
        }
        prepared_root::ExposureKind::Directory => {
            report_build_v2::ProductExposureKind::Directory
        }
        prepared_root::ExposureKind::CharacterDevice => {
            report_build_v2::ProductExposureKind::CharacterDevice
        }
    }
}

fn preflight_error(failure: product_preflight::Error) -> Error {
    Error {
        code: failure.code,
        subject: failure.path.map(|path| path.display().to_string()),
        message: failure.message,
    }
}

fn prepared_error(failure: prepared_root::Error) -> Error {
    Error {
        code: failure.code,
        subject: failure.subject,
        message: failure.message,
    }
}

fn workload_error(failure: product_workload::Error) -> Error {
    Error {
        code: failure.code,
        subject: failure.subject,
        message: failure.message,
    }
}

fn source_error(failure: product_source::ProductSourceError) -> Error {
    Error {
        code: failure.code,
        subject: failure.path.map(|path| path.display().to_string()),
        message: failure.message,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        env, fs,
        path::{Path, PathBuf},
        process::Command,
    };

    use super::{
        ExposureJoin, REGISTRY_PURPOSE, RegistryExposureView, artifact_kind,
        join_exposures, runtime_inputs, validate_host_registry,
        validate_registry_exposure_identity, validate_source_join,
        validate_substrate_join, workload_limits,
    };

    #[test]
    fn committed_limits_map_without_expansion() {
        let source = crate::topology::HARD_LIMITS;
        let mapped = workload_limits(source);
        let product = source.product_execution;
        assert_eq!(mapped.metadata_process, source.process());
        assert_eq!(
            mapped.package.output_bytes,
            product.package_list_bytes as usize
        );
        assert_eq!(
            mapped.compiler.wrapper_trace_bytes,
            product.wrapper_trace_bytes as usize
        );
        assert_eq!(
            mapped.substrate.total_bytes,
            u64::from(product.compiler_substrate_total_bytes)
        );
        assert_eq!(
            mapped.source.source_files,
            usize::from(product.compiler_input_count)
        );
        assert_eq!(mapped.result_bytes, source.metadata_stdout_bytes);
        let raw = include_bytes!("../../../conformance/product-closures.json");
        let registry = crate::product_workload::RegistryInput {
            path: PathBuf::from(
                crate::product_workload::PREPARED_REGISTRY_PATH,
            ),
            content_digest: crate::digest::bytes(raw),
            size_bytes: u64::try_from(raw.len()).unwrap(),
        };
        let value =
            crate::product_workload::request_value(&registry, &mapped).unwrap();
        let parsed = crate::product_workload::parse_request(&value).unwrap();
        assert_eq!(parsed.registry, registry);
        assert_eq!(parsed.limits, mapped);
    }

    #[test]
    fn substrate_join_rejects_identity_mutant() {
        let support = file("/host/support.so", "/compiler/lib/support.so", '1');
        let target = file(
            "/host/libstd.rlib",
            "/sysroot/lib/rustlib/x86_64-unknown-linux-gnu/lib/libstd.rlib",
            '2',
        );
        let layout = compiler_layout(support.clone(), target.clone());
        let mut observation = compiler_observation(&support, &target);
        validate_substrate_join(&layout, &observation).unwrap();
        observation.entries[0].content_digest = digest('3');
        assert_eq!(
            validate_substrate_join(&layout, &observation).unwrap_err().code,
            "PRODUCT_PROOF_SUBSTRATE_JOIN"
        );
    }

    #[test]
    fn compiler_source_join_is_exact_and_duplicate_closed() {
        let expected = crate::product_preflight::CompilerSourceSelection {
            foundation_shipping: std::iter::once(PathBuf::from(
                "kernel/foundation/src/lib.rs",
            ))
            .collect(),
            foundation_consumer: std::iter::once(PathBuf::from(
                "tests/foundation-public-consumer/src/lib.rs",
            ))
            .collect(),
            kernel_interface_shipping: std::iter::once(PathBuf::from(
                "kernel/interface/src/lib.rs",
            ))
            .collect(),
            kernel_interface_consumer: std::iter::once(PathBuf::from(
                "tests/kernel-interface-public-consumer/src/lib.rs",
            ))
            .collect(),
        };
        let shipping = vec![PathBuf::from("kernel/foundation/src/lib.rs")];
        let consumer =
            vec![PathBuf::from("tests/foundation-public-consumer/src/lib.rs")];
        let kernel_shipping =
            vec![PathBuf::from("kernel/interface/src/lib.rs")];
        let kernel_consumer = vec![PathBuf::from(
            "tests/kernel-interface-public-consumer/src/lib.rs",
        )];
        validate_source_join(
            &expected,
            &shipping,
            &consumer,
            &kernel_shipping,
            &kernel_consumer,
        )
        .unwrap();
        let duplicate = vec![shipping[0].clone(), shipping[0].clone()];
        assert_eq!(
            validate_source_join(
                &expected,
                &duplicate,
                &consumer,
                &kernel_shipping,
                &kernel_consumer,
            )
            .unwrap_err()
            .code,
            "PRODUCT_PROOF_SOURCE_JOIN"
        );
    }

    #[test]
    fn exposure_join_closes_missing_unknown_and_changed_ids() {
        let plans = vec![exposure("one", "/one"), exposure("two", "/two")];
        let exact = vec![observed("one", "/one"), observed("two", "/two")];
        let joined = join_exposures(&plans, &exact)
            .unwrap()
            .into_iter()
            .map(|value| value.id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(joined, BTreeSet::from(["one", "two"]));
        assert_eq!(
            join_exposures(&plans, &exact[..1]).unwrap_err().code,
            "PRODUCT_PROOF_EXPOSURE_JOIN"
        );
        let unknown = vec![observed("unknown", "/one")];
        assert_eq!(
            join_exposures(&plans, &unknown).unwrap_err().code,
            "PRODUCT_PROOF_EXPOSURE_JOIN"
        );
        let changed =
            vec![observed("one", "/changed"), observed("two", "/two")];
        assert_eq!(
            join_exposures(&plans, &changed).unwrap_err().code,
            "PRODUCT_PROOF_EXPOSURE_JOIN"
        );
    }

    #[test]
    fn host_registry_join_rejects_all_caller_authority_drift() {
        let fixture = HostRegistryFixture::new();
        let request =
            fixture.request(&fixture.bytes, &fixture.value, &fixture.registry);
        let identity = fixture.identity();
        validate_host_registry(&request, &identity).unwrap();

        let mut changed = identity.clone();
        changed.source = fixture.root.join("conformance/other.json");
        assert_host_registry_code(&request, &changed, "IDENTITY");

        let mut changed = identity.clone();
        changed.destination = PathBuf::from("/repo/conformance/other.json");
        assert_host_registry_code(&request, &changed, "IDENTITY");

        let mut changed = identity.clone();
        changed.size_bytes += 1;
        assert_host_registry_code(&request, &changed, "IDENTITY");

        let mut changed = identity.clone();
        changed.content_digest = digest('0');
        assert_host_registry_code(&request, &changed, "IDENTITY");

        let omitted = fixture.request(&[], &fixture.value, &fixture.registry);
        assert_host_registry_code(&omitted, &identity, "IDENTITY");

        let changed_value = serde_json::Value::Null;
        let changed =
            fixture.request(&fixture.bytes, &changed_value, &fixture.registry);
        assert_host_registry_code(&changed, &identity, "VALUE");

        let mut changed_registry = fixture.registry.clone();
        changed_registry.closures.pop();
        let changed =
            fixture.request(&fixture.bytes, &fixture.value, &changed_registry);
        assert_host_registry_code(&changed, &identity, "TYPED");
    }

    #[test]
    fn prepared_registry_join_rejects_identity_and_policy_drift() {
        let expected = registry_identity();
        let exact = registry_exposure(&expected);
        validate_registry_exposure_identity(&expected, &exact).unwrap();

        let mut changed = exact;
        changed.destination = Path::new("/repo/conformance/other.json");
        assert_registry_exposure_failure(&expected, &changed);

        let mut changed = exact;
        changed.kind = crate::prepared_root::ExposureKind::Directory;
        assert_registry_exposure_failure(&expected, &changed);

        let mut changed = exact;
        changed.executable = true;
        assert_registry_exposure_failure(&expected, &changed);

        let mut changed = exact;
        changed.purpose = "wrong purpose";
        assert_registry_exposure_failure(&expected, &changed);

        for flag in 0..4 {
            let mut changed = exact;
            changed.mount_policy[flag] = false;
            assert_registry_exposure_failure(&expected, &changed);
        }

        let mut changed = exact;
        changed.root_size_bytes += 1;
        assert_registry_exposure_failure(&expected, &changed);

        let mut changed = exact;
        changed.root_content_digest = Some("sha256:wrong");
        assert_registry_exposure_failure(&expected, &changed);
    }

    #[test]
    fn runtime_adapter_preserves_destinations_and_rejects_empty() {
        let observation = crate::runtime_closure::Observation {
            mappings: vec![
                mapping("/lib/loader.so", "/lib64/loader.so"),
                mapping("/lib/loader.so", "/usr/lib/loader.so"),
            ],
            object_count: 2,
            total_bytes: 2,
            aggregate_digest: digest('2'),
        };
        let retained = runtime_inputs(&observation).unwrap();
        let prepared = retained
            .iter()
            .map(|value| value.prepared_path.as_path())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            prepared,
            BTreeSet::from([
                Path::new("/lib64/loader.so"),
                Path::new("/usr/lib/loader.so"),
            ])
        );
        let empty = crate::runtime_closure::Observation {
            mappings: Vec::new(),
            object_count: 0,
            total_bytes: 0,
            aggregate_digest: digest('0'),
        };
        assert_eq!(
            runtime_inputs(&empty).unwrap_err().code,
            "PRODUCT_PROOF_RUNTIME_INPUT_EMPTY"
        );
    }

    #[test]
    fn artifact_adapter_rejects_unavailable_output_kind() {
        assert_eq!(
            artifact_kind(Path::new("/scratch/value.rmeta")).unwrap(),
            "rmeta"
        );
        assert!(artifact_kind(Path::new("/scratch/value.so")).is_err());
    }

    #[test]
    #[ignore = "requires live Linux namespace/mount capability"]
    fn live_current_repository_prepared_journey() {
        let live = LiveFixture::new();
        let observed = super::run(&live.request()).unwrap();
        let active = observed.active();
        assert_eq!(
            active.foundation_package_before.members,
            active.foundation_package_after.members,
        );
        assert_eq!(
            active.kernel_interface_package_before.members,
            active.kernel_interface_package_after.members,
        );
    }

    #[test]
    #[ignore = "explicit config-transport acceptance measurement"]
    fn live_current_repository_config_transport_inspection() {
        let live = LiveFixture::new();
        let request = live.request();
        let preflight = crate::product_preflight::observe(
            &crate::product_preflight::Request {
                repository_root: request.repository_root,
                cargo: request.cargo,
                rustc: request.rustc,
                checker: request.checker,
                inventory: request.inventory,
                limits: request.limits,
            },
        )
        .unwrap();
        validate_host_registry(&request, &preflight.registry).unwrap();
        let limits = workload_limits(request.limits);
        let registry = crate::product_workload::RegistryInput {
            path: preflight.registry.destination.clone(),
            content_digest: preflight.registry.content_digest.clone(),
            size_bytes: preflight.registry.size_bytes,
        };
        let value =
            crate::product_workload::request_value(&registry, &limits).unwrap();
        let request_bytes = serde_json::to_vec(&value).unwrap().len();
        assert!(request_bytes <= 4_096);
        let plan = super::prepared_plan(&request, &preflight, value).unwrap();
        let measured =
            crate::prepared_root::inspect_config_transport(&plan).unwrap();
        assert_eq!(measured.limit_bytes, 96 * 1024);
        assert!(measured.headroom_bytes > 0);
        assert_eq!(
            measured.encoded_bytes + measured.headroom_bytes,
            measured.limit_bytes
        );
        eprintln!(
            "request={} encoded={} limit={} headroom={}",
            request_bytes,
            measured.encoded_bytes,
            measured.limit_bytes,
            measured.headroom_bytes,
        );
    }

    struct HostRegistryFixture {
        root: PathBuf,
        bytes: Vec<u8>,
        value: serde_json::Value,
        registry: crate::product_closures::Registry,
        inventory: crate::source::Inventory,
    }

    impl HostRegistryFixture {
        fn new() -> Self {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap();
            let bytes =
                fs::read(root.join(crate::PRODUCT_CLOSURES_KEY)).unwrap();
            let limits = crate::topology::HARD_LIMITS;
            let value = crate::json::parse(
                &bytes,
                limits.json_depth,
                limits.json_container_items,
            )
            .unwrap();
            let registry = crate::product_closures::Registry::parse(
                &value,
                workload_limits(limits).registry,
            )
            .unwrap();
            let inventory = crate::source::Inventory {
                files: Vec::new(),
                exclusions: Vec::new(),
                tree_digest: digest('0'),
                findings: Vec::new(),
            };
            Self { root, bytes, value, registry, inventory }
        }

        fn request<'a>(
            &'a self,
            bytes: &'a [u8],
            value: &'a serde_json::Value,
            registry: &'a crate::product_closures::Registry,
        ) -> super::Request<'a> {
            super::Request {
                repository_root: &self.root,
                cargo: Path::new("/tool/cargo"),
                rustc: Path::new("/tool/rustc"),
                checker: Path::new("/tool/checker"),
                inventory: &self.inventory,
                limits: crate::topology::HARD_LIMITS,
                registry_bytes: bytes,
                registry_value: value,
                registry,
                execution_environment: "registry-unit-test",
            }
        }

        fn identity(&self) -> crate::product_preflight::FileIdentity {
            crate::product_preflight::FileIdentity {
                source: self.root.join(crate::PRODUCT_CLOSURES_KEY),
                destination: PathBuf::from(
                    crate::product_workload::PREPARED_REGISTRY_PATH,
                ),
                content_digest: crate::digest::bytes(&self.bytes),
                size_bytes: u64::try_from(self.bytes.len()).unwrap(),
            }
        }
    }

    struct LiveFixture {
        root: PathBuf,
        checker: PathBuf,
        cargo: PathBuf,
        rustc: PathBuf,
        inventory: crate::source::Inventory,
        limits: crate::topology::Limits,
        registry_bytes: Vec<u8>,
        registry_value: serde_json::Value,
        registry: crate::product_closures::Registry,
    }

    impl LiveFixture {
        fn new() -> Self {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap();
            let topology_value = config_value(&root, crate::TOPOLOGY_KEY);
            let topology =
                crate::topology::Topology::parse(&topology_value).unwrap();
            let policy_value = config_value(&root, crate::POLICY_KEY);
            let policy = crate::policy::Policy::parse(&policy_value).unwrap();
            let inventory = crate::source::inventory(&root, &policy, &topology);
            assert!(inventory.findings.is_empty(), "inventory has findings");
            let registry_bytes =
                fs::read(root.join(crate::PRODUCT_CLOSURES_KEY)).unwrap();
            let registry_value = crate::json::parse(
                &registry_bytes,
                topology.limits.json_depth,
                topology.limits.json_container_items,
            )
            .unwrap();
            let registry = crate::product_closures::Registry::parse(
                &registry_value,
                workload_limits(topology.limits).registry,
            )
            .unwrap();
            Self {
                root,
                checker: live_checker(),
                cargo: rustup_which("cargo"),
                rustc: rustup_which("rustc"),
                inventory,
                limits: topology.limits,
                registry_bytes,
                registry_value,
                registry,
            }
        }

        fn request(&self) -> super::Request<'_> {
            super::Request {
                repository_root: &self.root,
                cargo: &self.cargo,
                rustc: &self.rustc,
                checker: &self.checker,
                inventory: &self.inventory,
                limits: self.limits,
                registry_bytes: &self.registry_bytes,
                registry_value: &self.registry_value,
                registry: &self.registry,
                execution_environment: "live-kernel-interface-two-pair-proof",
            }
        }
    }

    fn assert_host_registry_code(
        request: &super::Request<'_>,
        identity: &crate::product_preflight::FileIdentity,
        suffix: &str,
    ) {
        let expected = format!("PRODUCT_PROOF_REGISTRY_{suffix}");
        assert_eq!(
            validate_host_registry(request, identity).unwrap_err().code,
            expected
        );
    }

    fn registry_identity() -> crate::product_preflight::FileIdentity {
        crate::product_preflight::FileIdentity {
            source: PathBuf::from(
                "/host/repo/conformance/product-closures.json",
            ),
            destination: PathBuf::from(
                crate::product_workload::PREPARED_REGISTRY_PATH,
            ),
            content_digest: digest('1'),
            size_bytes: 100,
        }
    }

    fn registry_exposure(
        expected: &crate::product_preflight::FileIdentity,
    ) -> RegistryExposureView<'_> {
        RegistryExposureView {
            id: "file:/repo/conformance/product-closures.json",
            destination: &expected.destination,
            kind: crate::prepared_root::ExposureKind::File,
            executable: false,
            purpose: REGISTRY_PURPOSE,
            device_identity: (None, None),
            mount_policy: [true; 4],
            entry_count: 1,
            total_bytes: expected.size_bytes,
            root_relative_path: Path::new("."),
            root_kind: crate::prepared_root::InventoryEntryKind::File,
            root_size_bytes: expected.size_bytes,
            root_content_digest: Some(&expected.content_digest),
            root_device_identity: None,
        }
    }

    fn assert_registry_exposure_failure(
        expected: &crate::product_preflight::FileIdentity,
        observed: &RegistryExposureView<'_>,
    ) {
        assert_eq!(
            validate_registry_exposure_identity(expected, observed)
                .unwrap_err()
                .code,
            "PRODUCT_PROOF_REGISTRY_EXPOSURE"
        );
    }

    fn live_checker() -> PathBuf {
        let value = env::var_os("REOVIM_LIVE_CHECKER").expect(concat!(
            "REOVIM_LIVE_CHECKER must name the absolute built ",
            "reovim-conformance binary"
        ));
        let path = PathBuf::from(value);
        assert!(path.is_absolute(), "REOVIM_LIVE_CHECKER is not absolute");
        let canonical = path.canonicalize().unwrap();
        assert_eq!(canonical, path, "REOVIM_LIVE_CHECKER is not canonical");
        canonical
    }

    fn rustup_which(tool: &str) -> PathBuf {
        let output =
            Command::new("rustup").args(["which", tool]).output().unwrap();
        assert!(output.status.success(), "rustup which {tool} failed");
        assert!(output.stderr.is_empty(), "rustup which emitted stderr");
        let value = std::str::from_utf8(&output.stdout).unwrap().trim_end();
        assert!(!value.is_empty() && !value.contains(['\r', '\n']));
        PathBuf::from(value).canonicalize().unwrap()
    }

    fn config_value(root: &Path, relative: &str) -> serde_json::Value {
        let bytes = fs::read(root.join(relative)).unwrap();
        crate::json::parse(
            &bytes,
            crate::topology::HARD_LIMITS.json_depth,
            crate::topology::HARD_LIMITS.json_container_items,
        )
        .unwrap()
    }

    fn file(
        source: &str,
        destination: &str,
        byte: char,
    ) -> crate::product_preflight::FileIdentity {
        crate::product_preflight::FileIdentity {
            source: PathBuf::from(source),
            destination: PathBuf::from(destination),
            content_digest: digest(byte),
            size_bytes: 1,
        }
    }

    fn compiler_layout(
        support: crate::product_preflight::FileIdentity,
        target: crate::product_preflight::FileIdentity,
    ) -> crate::product_preflight::CompilerLayout {
        crate::product_preflight::CompilerLayout {
            host_target_libdir: PathBuf::from("/host/target"),
            prepared_compiler_support_root: PathBuf::from("/compiler/lib"),
            prepared_target_libdir: PathBuf::from(concat!(
                "/sysroot/lib/rustlib/",
                "x86_64-unknown-linux-gnu/lib"
            )),
            compiler_support_files: vec![support.clone()],
            target_libdir_files: vec![target],
            provided_shared_objects: vec![support],
        }
    }

    fn compiler_observation(
        support: &crate::product_preflight::FileIdentity,
        target: &crate::product_preflight::FileIdentity,
    ) -> crate::compiler_substrate::Observation {
        let mut entries = vec![
            super::substrate_entry(
                support,
                crate::compiler_substrate::EntryKind::CompilerSupport,
            ),
            super::substrate_entry(
                target,
                crate::compiler_substrate::EntryKind::TargetLibdir,
            ),
        ];
        entries.sort();
        crate::compiler_substrate::Observation {
            compiler_support_root: PathBuf::from("/compiler/lib"),
            target_libdir: PathBuf::from(concat!(
                "/sysroot/lib/rustlib/",
                "x86_64-unknown-linux-gnu/lib"
            )),
            entries,
            entry_count: 2,
            total_bytes: 2,
            aggregate_digest: digest('4'),
        }
    }

    fn exposure(
        id: &str,
        destination: &str,
    ) -> crate::product_preflight::ExposureSpec {
        crate::product_preflight::ExposureSpec {
            id: id.to_owned(),
            source: PathBuf::from(format!("/host{destination}")),
            destination: PathBuf::from(destination),
            kind: crate::product_preflight::ExposureKind::File,
            executable: false,
            purpose: "fixture".to_owned(),
        }
    }

    fn observed<'a>(id: &'a str, destination: &'a str) -> ExposureJoin<'a> {
        ExposureJoin {
            id,
            destination: Path::new(destination),
            kind: crate::prepared_root::ExposureKind::File,
            executable: false,
            purpose: "fixture",
        }
    }

    fn mapping(
        source: &str,
        destination: &str,
    ) -> crate::runtime_closure::Mapping {
        crate::runtime_closure::Mapping {
            source: PathBuf::from(source),
            destination: PathBuf::from(destination),
            content_digest: digest('1'),
            size_bytes: 1,
        }
    }

    fn digest(byte: char) -> String {
        crate::digest::bytes(byte.to_string().as_bytes())
    }
}
