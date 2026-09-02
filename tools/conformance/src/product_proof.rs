//! Real coordinator for the initial Foundation Product proof.

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
const EXECUTION_ID: &str = "foundation-proof";
const SUBSTRATE_ID: &str = "foundation-rustc-substrate";
const HOST_TRIPLE: &str = "x86_64-unknown-linux-gnu";
const TARGET_TRIPLE: &str = "x86_64-unknown-linux-gnu";
const PREPARED_SCRATCH: &str = "/scratch";
const MOUNT_PARENT: &str = "/tmp";

/// Exact caller-owned authorities for one active Foundation proof.
pub struct Request<'a> {
    pub repository_root: &'a Path,
    pub cargo: &'a Path,
    pub rustc: &'a Path,
    pub checker: &'a Path,
    pub inventory: &'a Inventory,
    pub limits: Limits,
    pub registry_value: &'a Value,
    pub registry: &'a product_closures::Registry,
    pub execution_environment: &'a str,
}

/// Owned active-Product evidence and its report adapters.
#[derive(Debug)]
pub struct Observation {
    pub prepared: prepared_root::Execution,
    pub workload: product_workload::Observation,
    pub product_source: product_source::ProductSourceObservation,
    pub checker_runtime_inputs: Vec<report_build_v2::RuntimeInputIdentity>,
    pub cargo_runtime_inputs: Vec<report_build_v2::RuntimeInputIdentity>,
    pub rustc_runtime_inputs: Vec<report_build_v2::RuntimeInputIdentity>,
    cargo_builds: Vec<report_build_v2::ProductCargoBuildEvidence>,
    compiler_units: Vec<report_build_v2::ProductCompilerUnitEvidence>,
    extern_edges: Vec<report_build_v2::ProductExternEvidence>,
    artifacts: Vec<report_build_v2::ProductArtifactEvidence>,
    provider_artifact: report_build_v2::ProductProviderArtifactEvidence,
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
            provider_artifact: &self.provider_artifact,
            package_before: &self.workload.package_before,
            package_after: &self.workload.package_after,
            product_source: &self.product_source,
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

/// Runs and correlates the complete initial Foundation Product proof.
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
    let workload_limits = workload_limits(request.limits);
    let workload_request = product_workload::request_value(
        request.registry_value,
        &workload_limits,
    )
    .map_err(workload_error)?;
    validate_registry_join(
        &workload_request,
        &workload_limits,
        request.registry,
    )?;
    let plan = prepared_plan(request, &preflight, workload_request)?;
    let prepared = prepared_root::execute(&plan).map_err(prepared_error)?;
    let workload = product_workload::parse_result(
        &prepared.evidence.workload.value,
        &workload_limits,
    )
    .map_err(workload_error)?;
    validate_substrate_join(&preflight.compiler, &workload.compiler_substrate)?;
    validate_source_join(
        &preflight.compiler_sources,
        &workload.shipping.unit.sources,
        &workload.validation.unit.sources,
    )?;
    let product_source =
        analyze_product_source(request, &workload, workload_limits.source)?;
    let checker_runtime_inputs = runtime_inputs(&preflight.runtime.checker)?;
    let cargo_runtime_inputs = runtime_inputs(&preflight.runtime.cargo)?;
    let rustc_runtime_inputs = runtime_inputs(&preflight.runtime.rustc)?;
    let cargo_builds = cargo_build_evidence(&workload)?;
    let compiler_units = compiler_unit_evidence(&workload);
    let extern_edges = extern_evidence(&workload)?;
    let artifacts = artifact_evidence(&workload)?;
    let provider_artifact = provider_artifact_evidence(&workload)?;
    let execution = execution_evidence(&preflight, &prepared)?;
    Ok(Observation {
        prepared,
        workload,
        product_source,
        checker_runtime_inputs,
        cargo_runtime_inputs,
        rustc_runtime_inputs,
        cargo_builds,
        compiler_units,
        extern_edges,
        artifacts,
        provider_artifact,
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

fn validate_registry_join(
    value: &Value,
    limits: &product_workload::Limits,
    expected: &product_closures::Registry,
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
    shipping: &[PathBuf],
    consumer: &[PathBuf],
) -> Result<(), Error> {
    let shipping_set = shipping.iter().cloned().collect::<BTreeSet<_>>();
    let consumer_set = consumer.iter().cloned().collect::<BTreeSet<_>>();
    let exact = shipping.len() == shipping_set.len()
        && consumer.len() == consumer_set.len()
        && shipping_set == expected.shipping
        && consumer_set == expected.consumer;
    if exact {
        return Ok(());
    }
    Err(Error::new(
        "PRODUCT_PROOF_SOURCE_JOIN",
        "compiler dep-info sources differ from preflight selection",
    ))
}

fn analyze_product_source(
    request: &Request<'_>,
    workload: &product_workload::Observation,
    limits: product_source::AnalysisLimits,
) -> Result<product_source::ProductSourceObservation, Error> {
    let shipping = registry_closure(request.registry, SHIPPING_CLOSURE)?;
    let consumer = registry_closure(request.registry, CONSUMER_CLOSURE)?;
    let relation = registry_relation(request.registry)?;
    let shipping_sources =
        workload.shipping.unit.sources.iter().cloned().collect::<BTreeSet<_>>();
    let consumer_sources = workload
        .validation
        .unit
        .sources
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    product_source::analyze(&product_source::AnalysisRequest {
        root: request.repository_root,
        shipping,
        consumer,
        relation,
        shipping_sources: &shipping_sources,
        consumer_sources: &consumer_sources,
        inventory: &request.inventory.files,
        limits,
    })
    .map_err(source_error)
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

fn registry_relation(
    registry: &product_closures::Registry,
) -> Result<&product_closures::PublicConsumerRelation, Error> {
    let [relation] = registry.public_consumer_relations.as_slice() else {
        return Err(Error::new(
            "PRODUCT_PROOF_REGISTRY_JOIN",
            "active Product registry lacks its one public relation",
        ));
    };
    if relation.id == PUBLIC_RELATION {
        return Ok(relation);
    }
    Err(Error::new(
        "PRODUCT_PROOF_REGISTRY_JOIN",
        "active Product registry names an unexpected public relation",
    ))
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
    let shipping = cargo_build(&observed.shipping, None)?;
    let validation =
        cargo_build(&observed.validation, Some(&observed.shipping.unit.id))?;
    Ok(vec![shipping, validation])
}

fn cargo_build(
    observed: &BuildObservation,
    provider_unit: Option<&String>,
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
        .map(|artifact| cargo_artifact_event(observed, artifact, provider_unit))
        .collect::<Result<Vec<_>, Error>>()?;
    let fresh_provider_units = provider_unit.cloned().into_iter().collect();
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
    provider_unit: Option<&String>,
) -> Result<report_build_v2::ProductCargoArtifactEventEvidence, Error> {
    let unit = if artifact.target_name == observed.unit.crate_name {
        observed.unit.id.clone()
    } else {
        provider_unit.cloned().ok_or_else(|| {
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
) -> Result<report_build_v2::ProductProviderArtifactEvidence, Error> {
    let provider = observed.shipping.provider.as_ref().ok_or_else(|| {
        Error::new(
            "PRODUCT_PROOF_PROVIDER_ARTIFACT",
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
    [&observed.shipping.unit, &observed.validation.unit]
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
    let reuse =
        observed.validation.provider_reuse.as_ref().ok_or_else(|| {
            Error::new(
                "PRODUCT_PROOF_EXTERN_JOIN",
                "validation build lacks its provider reuse",
            )
        })?;
    Ok(vec![report_build_v2::ProductExternEvidence {
        consumer_unit: observed.validation.unit.id.clone(),
        extern_name: reuse.extern_edge.name.clone(),
        provider_unit: observed.shipping.unit.id.clone(),
        provider_path: reuse.extern_edge.path.clone(),
    }])
}

fn artifact_evidence(
    observed: &product_workload::Observation,
) -> Result<Vec<report_build_v2::ProductArtifactEvidence>, Error> {
    let mut values = Vec::new();
    for unit in [&observed.shipping.unit, &observed.validation.unit] {
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
        env, fs,
        path::{Path, PathBuf},
        process::Command,
    };

    use super::{
        ExposureJoin, artifact_kind, join_exposures, runtime_inputs,
        validate_source_join, validate_substrate_join, workload_limits,
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
            shipping: std::iter::once(PathBuf::from(
                "kernel/foundation/src/lib.rs",
            ))
            .collect(),
            consumer: std::iter::once(PathBuf::from(
                "tests/foundation-public-consumer/src/lib.rs",
            ))
            .collect(),
        };
        let shipping = vec![PathBuf::from("kernel/foundation/src/lib.rs")];
        let consumer =
            vec![PathBuf::from("tests/foundation-public-consumer/src/lib.rs")];
        validate_source_join(&expected, &shipping, &consumer).unwrap();
        let duplicate = vec![shipping[0].clone(), shipping[0].clone()];
        assert_eq!(
            validate_source_join(&expected, &duplicate, &consumer)
                .unwrap_err()
                .code,
            "PRODUCT_PROOF_SOURCE_JOIN"
        );
    }

    #[test]
    fn exposure_join_closes_missing_unknown_and_changed_ids() {
        let plans = vec![exposure("one", "/one"), exposure("two", "/two")];
        let exact = vec![observed("one", "/one"), observed("two", "/two")];
        assert_eq!(join_exposures(&plans, &exact).unwrap().len(), 2);
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
        assert_eq!(retained.len(), 2);
        assert_ne!(retained[0].prepared_path, retained[1].prepared_path);
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
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let checker = live_checker();
        let cargo = rustup_which("cargo");
        let rustc = rustup_which("rustc");
        let topology_value = config_value(&root, crate::TOPOLOGY_KEY);
        let topology =
            crate::topology::Topology::parse(&topology_value).unwrap();
        let policy_value = config_value(&root, crate::POLICY_KEY);
        let policy = crate::policy::Policy::parse(&policy_value).unwrap();
        let inventory = crate::source::inventory(&root, &policy, &topology);
        assert!(inventory.findings.is_empty(), "inventory has findings");
        let registry_value = config_value(&root, crate::PRODUCT_CLOSURES_KEY);
        let limits = workload_limits(topology.limits);
        let registry = crate::product_closures::Registry::parse(
            &registry_value,
            limits.registry,
        )
        .unwrap();
        let observed = super::run(&super::Request {
            repository_root: &root,
            cargo: &cargo,
            rustc: &rustc,
            checker: &checker,
            inventory: &inventory,
            limits: topology.limits,
            registry_value: &registry_value,
            registry: &registry,
            execution_environment: "live-foundation-proof",
        })
        .unwrap();
        assert_ne!(observed.checker_runtime_inputs, Vec::new());
        assert_ne!(observed.cargo_runtime_inputs, Vec::new());
        assert_ne!(observed.rustc_runtime_inputs, Vec::new());
        let active = observed.active();
        assert_eq!(active.cargo_builds.len(), 2);
        assert_eq!(active.compiler_units.len(), 2);
        assert_eq!(active.artifacts.len(), 4);
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
        format!("sha256:{}", byte.to_string().repeat(64))
    }
}
