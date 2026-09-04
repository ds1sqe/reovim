//! Read-only host preflight for the exact four-closure Product journey.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    fs::File,
    io::Read as _,
    os::unix::fs::FileTypeExt as _,
    path::{Path, PathBuf},
};

use reovim_depgraph::{
    ArchitecturalRole, SourceOwner, SourceProvenance, SourceUse,
};

use crate::{
    digest, runtime_closure,
    source::{ClassifiedFile, Inventory},
    topology::Limits,
};

const TARGET: &str = "x86_64-unknown-linux-gnu";
const PREPARED_REPOSITORY: &str = "/repo";
const PREPARED_COMPILER: &str = "/compiler";
const PREPARED_CHECKER_ROOT: &str = "/checker";
const PREPARED_CHECKER: &str = "/checker/reovim-conformance";
const PREPARED_SYSROOT: &str = "/sysroot";
const NULL_DEVICE: &str = "/dev/null";
const REGISTRY_RELATIVE: &str = "conformance/product-closures.json";
const PREPARED_REGISTRY: &str = "/repo/conformance/product-closures.json";

const PRODUCT_FILES: &[(&str, FileRole)] = &[
    (REGISTRY_RELATIVE, FileRole::Tool),
    ("product/Cargo.toml", FileRole::Tool),
    ("product/Cargo.lock", FileRole::Tool),
    ("kernel/foundation/Cargo.toml", FileRole::Foundation),
    ("kernel/foundation/src/bound.rs", FileRole::Foundation),
    ("kernel/foundation/src/carrier.rs", FileRole::Foundation),
    ("kernel/foundation/src/contract.rs", FileRole::Foundation),
    ("kernel/foundation/src/identity.rs", FileRole::Foundation),
    ("kernel/foundation/src/lib.rs", FileRole::Foundation),
    ("tests/foundation-public-consumer/Cargo.toml", FileRole::Consumer),
    ("tests/foundation-public-consumer/src/lib.rs", FileRole::Consumer),
    (
        "tests/foundation-public-consumer/src/public_items.rs",
        FileRole::Consumer,
    ),
    ("kernel/interface/Cargo.toml", FileRole::KernelInterface),
    ("kernel/interface/src/bound.rs", FileRole::KernelInterface),
    ("kernel/interface/src/cancellation.rs", FileRole::KernelInterface),
    ("kernel/interface/src/carrier.rs", FileRole::KernelInterface),
    ("kernel/interface/src/clock.rs", FileRole::KernelInterface),
    ("kernel/interface/src/contract.rs", FileRole::KernelInterface),
    ("kernel/interface/src/diagnostic.rs", FileRole::KernelInterface),
    (
        "kernel/interface/src/diagnostic/destination.rs",
        FileRole::KernelInterface,
    ),
    ("kernel/interface/src/diagnostic/shutdown.rs", FileRole::KernelInterface),
    ("kernel/interface/src/effect.rs", FileRole::KernelInterface),
    ("kernel/interface/src/event.rs", FileRole::KernelInterface),
    ("kernel/interface/src/event/admission.rs", FileRole::KernelInterface),
    ("kernel/interface/src/event/canonical.rs", FileRole::KernelInterface),
    (
        "kernel/interface/src/event/canonical/grammar.rs",
        FileRole::KernelInterface,
    ),
    ("kernel/interface/src/event/counts.rs", FileRole::KernelInterface),
    ("kernel/interface/src/event/cursor.rs", FileRole::KernelInterface),
    ("kernel/interface/src/event/descriptor.rs", FileRole::KernelInterface),
    ("kernel/interface/src/event/identity.rs", FileRole::KernelInterface),
    ("kernel/interface/src/event/limits.rs", FileRole::KernelInterface),
    ("kernel/interface/src/event/proposal.rs", FileRole::KernelInterface),
    ("kernel/interface/src/event/record.rs", FileRole::KernelInterface),
    ("kernel/interface/src/event/remediation.rs", FileRole::KernelInterface),
    ("kernel/interface/src/event/sequence.rs", FileRole::KernelInterface),
    ("kernel/interface/src/event/snapshot.rs", FileRole::KernelInterface),
    (
        "kernel/interface/src/event/snapshot_validation.rs",
        FileRole::KernelInterface,
    ),
    ("kernel/interface/src/identity.rs", FileRole::KernelInterface),
    ("kernel/interface/src/lib.rs", FileRole::KernelInterface),
    ("kernel/interface/src/lifecycle.rs", FileRole::KernelInterface),
    ("kernel/interface/src/operation.rs", FileRole::KernelInterface),
    ("kernel/interface/src/output_health.rs", FileRole::KernelInterface),
    (
        "kernel/interface/src/output_health/counters.rs",
        FileRole::KernelInterface,
    ),
    ("kernel/interface/src/output_health/record.rs", FileRole::KernelInterface),
    (
        "kernel/interface/src/output_health/snapshot.rs",
        FileRole::KernelInterface,
    ),
    ("kernel/interface/src/resource.rs", FileRole::KernelInterface),
    (
        "tests/kernel-interface-public-consumer/Cargo.toml",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/clock.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/diagnostic.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        concat!(
            "tests/kernel-interface-public-consumer/",
            "src/diagnostic/destination.rs",
        ),
        FileRole::KernelInterfaceConsumer,
    ),
    (
        concat!(
            "tests/kernel-interface-public-consumer/",
            "src/diagnostic/shutdown.rs",
        ),
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/effect.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/event/canonical.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/event/derives.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/event/descriptor.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/event/mod.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/event/outcomes.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/event/sequence.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/event/snapshot.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/lib.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/lifecycle.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/operation.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/output_health.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/output_health_state.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        concat!(
            "tests/kernel-interface-public-consumer/src/",
            "output_health_state/counters.rs",
        ),
        FileRole::KernelInterfaceConsumer,
    ),
    (
        concat!(
            "tests/kernel-interface-public-consumer/src/",
            "output_health_state/snapshot.rs",
        ),
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/public_items.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/src/resource.rs",
        FileRole::KernelInterfaceConsumer,
    ),
    (
        "tests/kernel-interface-public-consumer/tests/public_contract.rs",
        FileRole::KernelInterfaceConsumer,
    ),
];

pub fn repository_inputs() -> impl Iterator<Item = &'static str> {
    PRODUCT_FILES.iter().map(|(path, _)| *path)
}

/// Canonical host inputs for one read-only Product preflight.
pub struct Request<'a> {
    pub repository_root: &'a Path,
    pub cargo: &'a Path,
    pub rustc: &'a Path,
    pub checker: &'a Path,
    pub inventory: &'a Inventory,
    pub limits: Limits,
}

/// Prepared-root exposure type, matching `prepared_root::ExposureKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExposureKind {
    File,
    Directory,
    CharacterDevice,
}

/// One owned host-to-prepared exposure specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExposureSpec {
    pub id: String,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub kind: ExposureKind,
    pub executable: bool,
    pub purpose: String,
}

/// Exact identity of one selected regular file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileIdentity {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub content_digest: String,
    pub size_bytes: u64,
}

/// Exact host and prepared compiler layout selected without executing rustc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerLayout {
    pub host_target_libdir: PathBuf,
    pub prepared_compiler_support_root: PathBuf,
    pub prepared_target_libdir: PathBuf,
    pub compiler_support_files: Vec<FileIdentity>,
    pub target_libdir_files: Vec<FileIdentity>,
    pub provided_shared_objects: Vec<FileIdentity>,
}

/// Exact repository-relative compiler source sets for all four closures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerSourceSelection {
    pub foundation_shipping: BTreeSet<PathBuf>,
    pub foundation_consumer: BTreeSet<PathBuf>,
    pub kernel_interface_shipping: BTreeSet<PathBuf>,
    pub kernel_interface_consumer: BTreeSet<PathBuf>,
}

/// Runtime observations retained separately for each tool identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRuntimeObservations {
    pub cargo: runtime_closure::Observation,
    pub rustc: runtime_closure::Observation,
    pub checker: runtime_closure::Observation,
}

/// Complete owned preflight result for prepared-root plan construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// Raw registry file selected before its prepared exposure is constructed.
    pub registry: FileIdentity,
    pub compiler_sources: CompilerSourceSelection,
    pub cargo: FileIdentity,
    pub rustc: FileIdentity,
    pub checker: FileIdentity,
    pub compiler: CompilerLayout,
    pub runtime: ToolRuntimeObservations,
    pub exposures: Vec<ExposureSpec>,
}

/// Typed path, identity, layout, runtime, or bound failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: &'static str,
    pub path: Option<PathBuf>,
    pub message: String,
}

impl Error {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, path: None, message: message.into() }
    }

    fn at(code: &'static str, path: &Path, message: impl Into<String>) -> Self {
        Self { code, path: Some(path.to_path_buf()), message: message.into() }
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

/// Selects and observes the exact minimal host authority for Product proof.
///
/// # Errors
///
/// Returns a typed error for aliases, unsupported layouts, identity drift,
/// unclassified Product inputs, runtime ambiguity, or exhausted bounds.
pub fn observe(request: &Request<'_>) -> Result<Observation, Error> {
    validate_request(request)?;
    let repository_files = observe_repository_files(request)?;
    let registry = registry_identity(&repository_files)?;
    let compiler_sources = compiler_source_selection();
    let compiler = observe_compiler_layout(request)?;
    let cargo = observe_tool(
        request.cargo,
        Path::new("/compiler/bin/cargo"),
        request.limits.path_bytes,
        request.limits.tool_executable_bytes,
    )?;
    let rustc = observe_tool(
        request.rustc,
        Path::new("/compiler/bin/rustc"),
        request.limits.path_bytes,
        request.limits.tool_executable_bytes,
    )?;
    let checker = observe_tool(
        request.checker,
        Path::new(PREPARED_CHECKER),
        request.limits.path_bytes,
        request.limits.tool_executable_bytes,
    )?;
    reject_selected_aliases(
        &repository_files,
        &compiler,
        [&cargo, &rustc, &checker],
    )?;
    let runtime = observe_tool_runtimes(request, &compiler)?;
    let runtime_files = deduplicate_runtime_files(&runtime)?;
    let exposures = exposure_specs(
        request,
        &repository_files,
        &compiler,
        [&cargo, &rustc, &checker],
        &runtime_files,
    )?;
    Ok(Observation {
        registry,
        compiler_sources,
        cargo,
        rustc,
        checker,
        compiler,
        runtime,
        exposures,
    })
}

fn registry_identity(files: &[FileIdentity]) -> Result<FileIdentity, Error> {
    let mut matches = files
        .iter()
        .filter(|file| file.destination == Path::new(PREPARED_REGISTRY));
    let registry = matches.next().ok_or_else(|| {
        Error::new(
            "PRODUCT_PREFLIGHT_REGISTRY",
            "Product closure registry identity is absent",
        )
    })?;
    if matches.next().is_some() {
        return Err(Error::new(
            "PRODUCT_PREFLIGHT_REGISTRY",
            "Product closure registry identity is duplicated",
        ));
    }
    Ok(registry.clone())
}

/// Returns the exact four compiler-selected library source sets.
pub fn compiler_source_selection() -> CompilerSourceSelection {
    let foundation_shipping = [
        "kernel/foundation/src/bound.rs",
        "kernel/foundation/src/carrier.rs",
        "kernel/foundation/src/contract.rs",
        "kernel/foundation/src/identity.rs",
        "kernel/foundation/src/lib.rs",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    let foundation_consumer = [
        "tests/foundation-public-consumer/src/lib.rs",
        "tests/foundation-public-consumer/src/public_items.rs",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    let kernel_interface_shipping = PRODUCT_FILES
        .iter()
        .filter(|(path, role)| {
            *role == FileRole::KernelInterface
                && Path::new(path)
                    .extension()
                    .is_some_and(|value| value.eq_ignore_ascii_case("rs"))
        })
        .map(|(path, _)| PathBuf::from(path))
        .collect();
    let kernel_interface_consumer = PRODUCT_FILES
        .iter()
        .filter(|(path, role)| {
            *role == FileRole::KernelInterfaceConsumer
                && Path::new(path)
                    .starts_with("tests/kernel-interface-public-consumer/src")
        })
        .map(|(path, _)| PathBuf::from(path))
        .collect();
    CompilerSourceSelection {
        foundation_shipping,
        foundation_consumer,
        kernel_interface_shipping,
        kernel_interface_consumer,
    }
}

fn validate_request(request: &Request<'_>) -> Result<(), Error> {
    canonical_directory(request.repository_root, request.limits.path_bytes)?;
    for (path, name) in [
        (request.cargo, "Cargo"),
        (request.rustc, "rustc"),
        (request.checker, "checker"),
    ] {
        canonical_regular_file(
            path,
            request.limits.path_bytes,
            request.limits.tool_executable_bytes,
            name,
        )?;
    }
    if request.inventory.findings.is_empty() {
        Ok(())
    } else {
        Err(Error::new(
            "PRODUCT_PREFLIGHT_INVENTORY_FINDINGS",
            "source inventory contains findings",
        ))
    }
}

fn observe_repository_files(
    request: &Request<'_>,
) -> Result<Vec<FileIdentity>, Error> {
    let mut retained = BTreeMap::new();
    for file in &request.inventory.files {
        if retained.insert(file.path.as_path(), file).is_some() {
            return Err(Error::at(
                "PRODUCT_PREFLIGHT_SOURCE_DUPLICATE",
                &file.path,
                "source inventory repeats one path",
            ));
        }
    }
    let mut values = Vec::with_capacity(PRODUCT_FILES.len());
    let mut total = 0_u64;
    for (path, role) in PRODUCT_FILES {
        let relative = Path::new(path);
        let classified = retained.get(relative).ok_or_else(|| {
            Error::at(
                "PRODUCT_PREFLIGHT_SOURCE_MISSING",
                relative,
                "required Product input is absent from inventory",
            )
        })?;
        validate_source_role(classified, *role)?;
        let source = request.repository_root.join(relative);
        let destination = Path::new(PREPARED_REPOSITORY).join(relative);
        let observed = observe_regular_file(
            &source,
            &destination,
            request.limits.path_bytes,
            request.limits.source_file_bytes,
            "Product source",
        )?;
        if observed.content_digest != classified.content_digest {
            return Err(Error::at(
                "PRODUCT_PREFLIGHT_SOURCE_IDENTITY",
                &source,
                "source bytes differ from exact inventory",
            ));
        }
        total = total.checked_add(observed.size_bytes).ok_or_else(|| {
            Error::new(
                "PRODUCT_PREFLIGHT_SOURCE_LIMIT",
                "selected Product source byte count overflow",
            )
        })?;
        if total > request.limits.source_total_bytes {
            return Err(Error::new(
                "PRODUCT_PREFLIGHT_SOURCE_LIMIT",
                "selected Product source exceeds its aggregate byte bound",
            ));
        }
        values.push(observed);
    }
    Ok(values)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FileRole {
    Tool,
    Foundation,
    Consumer,
    KernelInterface,
    KernelInterfaceConsumer,
}

fn validate_source_role(
    file: &ClassifiedFile,
    role: FileRole,
) -> Result<(), Error> {
    let expected = match role {
        FileRole::Tool => None,
        FileRole::Foundation => {
            Some((ArchitecturalRole::Foundation, SourceUse::Production))
        }
        FileRole::Consumer => {
            Some((ArchitecturalRole::KernelInterface, SourceUse::Test))
        }
        FileRole::KernelInterface => {
            Some((ArchitecturalRole::KernelInterface, SourceUse::Production))
        }
        FileRole::KernelInterfaceConsumer => {
            Some((ArchitecturalRole::KernelRuntime, SourceUse::Test))
        }
    };
    let valid = match expected {
        None => {
            file.classification.owner == SourceOwner::RepositoryTooling
                && file.classification.source_use == SourceUse::Tool
        }
        Some((role, source_use)) => {
            file.classification.owner == SourceOwner::Product(role)
                && file.classification.source_use == source_use
                && file.classification.provenance == SourceProvenance::Authored
        }
    };
    if valid {
        return Ok(());
    }
    Err(Error::at(
        "PRODUCT_PREFLIGHT_SOURCE_ROLE",
        &file.path,
        "required Product input has the wrong classification",
    ))
}

fn observe_compiler_layout(
    request: &Request<'_>,
) -> Result<CompilerLayout, Error> {
    let host_sysroot = derive_sysroot(request.cargo, request.rustc)?;
    let support_root = host_sysroot.join("lib");
    let target_libdir = support_root.join("rustlib").join(TARGET).join("lib");
    canonical_directory(&support_root, request.limits.path_bytes)?;
    canonical_directory(&target_libdir, request.limits.path_bytes)?;
    let prepared_support = PathBuf::from("/compiler/lib");
    let prepared_target = Path::new(PREPARED_SYSROOT)
        .join("lib/rustlib")
        .join(TARGET)
        .join("lib");
    let limit = request.limits.product_execution;
    let support = observe_flat_directory(
        &support_root,
        &prepared_support,
        Some("rustlib"),
        request.limits.path_bytes,
        u64::from(limit.compiler_substrate_total_bytes),
    )?;
    let target = observe_flat_directory(
        &target_libdir,
        &prepared_target,
        None,
        request.limits.path_bytes,
        u64::from(limit.compiler_substrate_total_bytes),
    )?;
    validate_substrate_bounds(&support, &target, request.limits)?;
    let mut provided = Vec::new();
    for file in &support {
        if file_is_elf(&file.source)? {
            provided.push(file.clone());
        }
    }
    if provided.is_empty() {
        return Err(Error::new(
            "PRODUCT_PREFLIGHT_COMPILER_SHARED_OBJECT",
            "compiler support contains no direct ELF shared object",
        ));
    }
    Ok(CompilerLayout {
        host_target_libdir: target_libdir,
        prepared_compiler_support_root: prepared_support,
        prepared_target_libdir: prepared_target,
        compiler_support_files: support,
        target_libdir_files: target,
        provided_shared_objects: provided,
    })
}

fn derive_sysroot(cargo: &Path, rustc: &Path) -> Result<PathBuf, Error> {
    let rustc_bin = rustc.parent().ok_or_else(|| {
        Error::at(
            "PRODUCT_PREFLIGHT_RUSTC_LAYOUT",
            rustc,
            "rustc has no bin parent",
        )
    })?;
    let sysroot = rustc_bin.parent().ok_or_else(|| {
        Error::at(
            "PRODUCT_PREFLIGHT_RUSTC_LAYOUT",
            rustc,
            "rustc has no sysroot parent",
        )
    })?;
    if rustc.file_name().and_then(|name| name.to_str()) != Some("rustc")
        || cargo.file_name().and_then(|name| name.to_str()) != Some("cargo")
        || rustc_bin.file_name().and_then(|name| name.to_str()) != Some("bin")
        || cargo.parent() != Some(rustc_bin)
    {
        return Err(Error::at(
            "PRODUCT_PREFLIGHT_RUSTC_LAYOUT",
            rustc,
            "Cargo and rustc do not share one canonical sysroot/bin layout",
        ));
    }
    Ok(sysroot.to_path_buf())
}

fn observe_flat_directory(
    root: &Path,
    destination: &Path,
    allowed_directory: Option<&str>,
    path_limit: usize,
    file_limit: u64,
) -> Result<Vec<FileIdentity>, Error> {
    let mut entries = fs::read_dir(root)
        .map_err(|failure| {
            Error::at(
                "PRODUCT_PREFLIGHT_DIRECTORY_READ",
                root,
                failure.to_string(),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|failure| {
            Error::at(
                "PRODUCT_PREFLIGHT_DIRECTORY_ENTRY",
                root,
                failure.to_string(),
            )
        })?;
    entries.sort_by_key(fs::DirEntry::file_name);
    let mut values = Vec::new();
    for entry in entries {
        let source = entry.path();
        let metadata = fs::symlink_metadata(&source).map_err(|failure| {
            Error::at(
                "PRODUCT_PREFLIGHT_ENTRY_METADATA",
                &source,
                failure.to_string(),
            )
        })?;
        let name = entry.file_name();
        if metadata.is_dir()
            && allowed_directory.is_some_and(|allowed| name == allowed)
        {
            continue;
        }
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(Error::at(
                "PRODUCT_PREFLIGHT_COMPILER_ENTRY",
                &source,
                "compiler directory contains an unselected entry kind",
            ));
        }
        let destination = destination.join(&name);
        values.push(observe_regular_file(
            &source,
            &destination,
            path_limit,
            file_limit,
            "compiler substrate",
        )?);
    }
    if values.is_empty() {
        return Err(Error::at(
            "PRODUCT_PREFLIGHT_COMPILER_EMPTY",
            root,
            "compiler directory contains no regular files",
        ));
    }
    Ok(values)
}

fn validate_substrate_bounds(
    support: &[FileIdentity],
    target: &[FileIdentity],
    limits: Limits,
) -> Result<(), Error> {
    let product = limits.product_execution;
    let count = support.len().checked_add(target.len()).ok_or_else(|| {
        Error::new(
            "PRODUCT_PREFLIGHT_SUBSTRATE_LIMIT",
            "compiler substrate count overflow",
        )
    })?;
    let total = support.iter().chain(target).try_fold(0_u64, |sum, file| {
        sum.checked_add(file.size_bytes).ok_or_else(|| {
            Error::new(
                "PRODUCT_PREFLIGHT_SUBSTRATE_LIMIT",
                "compiler substrate byte overflow",
            )
        })
    })?;
    if count > usize::from(product.compiler_substrate_entry_count)
        || total > u64::from(product.compiler_substrate_total_bytes)
    {
        return Err(Error::new(
            "PRODUCT_PREFLIGHT_SUBSTRATE_LIMIT",
            "compiler substrate exceeds its committed count or byte bound",
        ));
    }
    Ok(())
}

fn observe_tool(
    source: &Path,
    destination: &Path,
    path_limit: usize,
    limit: u64,
) -> Result<FileIdentity, Error> {
    observe_regular_file(
        source,
        destination,
        path_limit,
        limit,
        "tool executable",
    )
}

fn observe_tool_runtimes(
    request: &Request<'_>,
    compiler: &CompilerLayout,
) -> Result<ToolRuntimeObservations, Error> {
    let limits = runtime_limits(request.limits);
    let cargo = observe_runtime(
        request.cargo,
        Path::new("/compiler/bin/cargo"),
        &[],
        limits,
    )?;
    let checker = observe_runtime(
        request.checker,
        Path::new(PREPARED_CHECKER),
        &[],
        limits,
    )?;
    let provided = compiler
        .provided_shared_objects
        .iter()
        .map(|file| runtime_closure::ObjectRequest {
            source: &file.source,
            destination: &file.destination,
        })
        .collect::<Vec<_>>();
    let rustc = observe_runtime(
        request.rustc,
        Path::new("/compiler/bin/rustc"),
        &provided,
        limits,
    )?;
    Ok(ToolRuntimeObservations { cargo, rustc, checker })
}

fn observe_runtime(
    source: &Path,
    destination: &Path,
    provided: &[runtime_closure::ObjectRequest<'_>],
    limits: runtime_closure::Limits,
) -> Result<runtime_closure::Observation, Error> {
    runtime_closure::observe(
        &[runtime_closure::ObjectRequest { source, destination }],
        provided,
        limits,
    )
    .map_err(|failure| Error {
        code: failure.code,
        path: failure.path,
        message: failure.message,
    })
}

fn runtime_limits(limits: Limits) -> runtime_closure::Limits {
    let product = limits.product_execution;
    runtime_closure::Limits {
        path_bytes: limits.path_bytes,
        object_count: product.environment_dependency_count,
        file_bytes: u64::from(product.compiler_substrate_total_bytes)
            .max(limits.tool_executable_bytes),
        total_bytes: u64::from(product.compiler_substrate_total_bytes),
        dynamic_entries: product.wrapper_argv_items,
        string_bytes: product.wrapper_argv_bytes as usize,
    }
}

fn deduplicate_runtime_files(
    runtime: &ToolRuntimeObservations,
) -> Result<Vec<FileIdentity>, Error> {
    let mut destinations = BTreeMap::<PathBuf, FileIdentity>::new();
    for mapping in runtime
        .cargo
        .mappings
        .iter()
        .chain(&runtime.rustc.mappings)
        .chain(&runtime.checker.mappings)
    {
        let identity = FileIdentity {
            source: mapping.source.clone(),
            destination: mapping.destination.clone(),
            content_digest: mapping.content_digest.clone(),
            size_bytes: mapping.size_bytes,
        };
        insert_runtime_identity(&mut destinations, identity)?;
    }
    Ok(destinations.into_values().collect())
}

fn insert_runtime_identity(
    destinations: &mut BTreeMap<PathBuf, FileIdentity>,
    identity: FileIdentity,
) -> Result<(), Error> {
    if reserved_destination(&identity.destination) {
        return Err(Error::at(
            "PRODUCT_PREFLIGHT_RUNTIME_DESTINATION",
            &identity.destination,
            "system runtime overlaps a fixed prepared authority",
        ));
    }
    if let Some(existing) = destinations.get(&identity.destination)
        && existing != &identity
    {
        return Err(Error::at(
            "PRODUCT_PREFLIGHT_RUNTIME_CONFLICT",
            &identity.destination,
            "prepared runtime path has conflicting identities",
        ));
    }
    destinations.insert(identity.destination.clone(), identity);
    Ok(())
}

fn reserved_destination(path: &Path) -> bool {
    [
        PREPARED_REPOSITORY,
        PREPARED_COMPILER,
        PREPARED_CHECKER_ROOT,
        PREPARED_SYSROOT,
        NULL_DEVICE,
    ]
    .into_iter()
    .any(|reserved| path == Path::new(reserved) || path.starts_with(reserved))
}

fn exposure_specs(
    request: &Request<'_>,
    repository: &[FileIdentity],
    compiler: &CompilerLayout,
    tools: [&FileIdentity; 3],
    runtime: &[FileIdentity],
) -> Result<Vec<ExposureSpec>, Error> {
    let mut values = Vec::new();
    for file in repository {
        values.push(file_exposure(file, false, "Product repository input"));
    }
    values.push(file_exposure(tools[0], true, "Cargo executable"));
    values.push(file_exposure(tools[1], true, "rustc executable"));
    values.push(file_exposure(tools[2], true, "checker"));
    for file in &compiler.compiler_support_files {
        values.push(file_exposure(file, true, "compiler support"));
    }
    values.push(ExposureSpec {
        id: "compiler-target-libdir".to_owned(),
        source: compiler.host_target_libdir.clone(),
        destination: compiler.prepared_target_libdir.clone(),
        kind: ExposureKind::Directory,
        executable: false,
        purpose: "selected Rust target libdir".to_owned(),
    });
    for file in runtime {
        values.push(file_exposure(file, true, "system runtime"));
    }
    values.push(null_exposure(request.limits.path_bytes)?);
    canonicalize_exposures(&mut values, request.limits)?;
    Ok(values)
}

fn file_exposure(
    file: &FileIdentity,
    executable: bool,
    purpose: &str,
) -> ExposureSpec {
    ExposureSpec {
        id: format!("file:{}", file.destination.display()),
        source: file.source.clone(),
        destination: file.destination.clone(),
        kind: ExposureKind::File,
        executable,
        purpose: purpose.to_owned(),
    }
}

fn null_exposure(path_limit: usize) -> Result<ExposureSpec, Error> {
    let path = Path::new(NULL_DEVICE);
    validate_path(path, path_limit, "null device")?;
    let metadata = fs::symlink_metadata(path).map_err(|failure| {
        Error::at("PRODUCT_PREFLIGHT_NULL_METADATA", path, failure.to_string())
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_char_device()
        || fs::canonicalize(path).ok().as_deref() != Some(path)
    {
        return Err(Error::at(
            "PRODUCT_PREFLIGHT_NULL_DEVICE",
            path,
            "/dev/null is not its canonical character device",
        ));
    }
    Ok(ExposureSpec {
        id: "device:/dev/null".to_owned(),
        source: path.to_path_buf(),
        destination: path.to_path_buf(),
        kind: ExposureKind::CharacterDevice,
        executable: false,
        purpose: "null standard-input device".to_owned(),
    })
}

fn canonicalize_exposures(
    values: &mut [ExposureSpec],
    limits: Limits,
) -> Result<(), Error> {
    let maximum = usize::from(limits.product_execution.exposure_entry_count);
    if values.len() > maximum {
        return Err(Error::new(
            "PRODUCT_PREFLIGHT_EXPOSURE_LIMIT",
            "prepared exposures exceed the committed count bound",
        ));
    }
    values.sort_by(|left, right| left.destination.cmp(&right.destination));
    let mut destinations = BTreeSet::new();
    for value in values.iter() {
        validate_path(&value.source, limits.path_bytes, "exposure source")?;
        validate_path(
            &value.destination,
            limits.path_bytes,
            "exposure destination",
        )?;
        if !destinations.insert(&value.destination) {
            return Err(Error::at(
                "PRODUCT_PREFLIGHT_EXPOSURE_DUPLICATE",
                &value.destination,
                "prepared destination is duplicated",
            ));
        }
    }
    reject_destination_overlap(values)
}

fn reject_destination_overlap(values: &[ExposureSpec]) -> Result<(), Error> {
    for window in values.windows(2) {
        let parent = &window[0];
        let child = &window[1];
        if child.destination.starts_with(&parent.destination)
            && parent.kind != ExposureKind::File
        {
            return Err(Error::at(
                "PRODUCT_PREFLIGHT_EXPOSURE_OVERLAP",
                &child.destination,
                "prepared exposures overlap",
            ));
        }
    }
    Ok(())
}

fn reject_selected_aliases(
    repository: &[FileIdentity],
    compiler: &CompilerLayout,
    tools: [&FileIdentity; 3],
) -> Result<(), Error> {
    let mut sources = BTreeSet::new();
    let selected = repository
        .iter()
        .chain(&compiler.compiler_support_files)
        .chain(&compiler.target_libdir_files)
        .chain(tools);
    for file in selected {
        if !sources.insert(&file.source) {
            return Err(Error::at(
                "PRODUCT_PREFLIGHT_SOURCE_ALIAS",
                &file.source,
                "one host file occupies two selected roles",
            ));
        }
    }
    Ok(())
}

fn observe_regular_file(
    source: &Path,
    destination: &Path,
    path_limit: usize,
    file_limit: u64,
    context: &str,
) -> Result<FileIdentity, Error> {
    canonical_regular_file(source, path_limit, file_limit, context)?;
    validate_path(destination, path_limit, context)?;
    let file = File::open(source).map_err(|failure| {
        Error::at("PRODUCT_PREFLIGHT_FILE_OPEN", source, failure.to_string())
    })?;
    let metadata = file.metadata().map_err(|failure| {
        Error::at(
            "PRODUCT_PREFLIGHT_FILE_METADATA",
            source,
            failure.to_string(),
        )
    })?;
    let content_digest = digest::file_handle_bounded(&file, file_limit)
        .map_err(|failure| {
            Error::at(
                "PRODUCT_PREFLIGHT_FILE_DIGEST",
                source,
                failure.to_string(),
            )
        })?;
    Ok(FileIdentity {
        source: source.to_path_buf(),
        destination: destination.to_path_buf(),
        content_digest,
        size_bytes: metadata.len(),
    })
}

fn canonical_regular_file(
    path: &Path,
    path_limit: usize,
    file_limit: u64,
    context: &str,
) -> Result<(), Error> {
    validate_path(path, path_limit, context)?;
    let metadata = fs::symlink_metadata(path).map_err(|failure| {
        Error::at("PRODUCT_PREFLIGHT_FILE_METADATA", path, failure.to_string())
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > file_limit
    {
        return Err(Error::at(
            "PRODUCT_PREFLIGHT_FILE_TYPE",
            path,
            "file is a symlink, nonregular, or over its byte bound",
        ));
    }
    let canonical = fs::canonicalize(path).map_err(|failure| {
        Error::at("PRODUCT_PREFLIGHT_FILE_CANONICAL", path, failure.to_string())
    })?;
    if canonical != path {
        return Err(Error::at(
            "PRODUCT_PREFLIGHT_FILE_ALIAS",
            path,
            "file path differs from its canonical identity",
        ));
    }
    Ok(())
}

fn canonical_directory(path: &Path, path_limit: usize) -> Result<(), Error> {
    validate_path(path, path_limit, "directory")?;
    let metadata = fs::symlink_metadata(path).map_err(|failure| {
        Error::at(
            "PRODUCT_PREFLIGHT_DIRECTORY_METADATA",
            path,
            failure.to_string(),
        )
    })?;
    let canonical = fs::canonicalize(path).map_err(|failure| {
        Error::at(
            "PRODUCT_PREFLIGHT_DIRECTORY_CANONICAL",
            path,
            failure.to_string(),
        )
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || canonical != path
    {
        return Err(Error::at(
            "PRODUCT_PREFLIGHT_DIRECTORY_ALIAS",
            path,
            "directory path differs from its canonical real identity",
        ));
    }
    Ok(())
}

fn validate_path(
    path: &Path,
    limit: usize,
    context: &str,
) -> Result<(), Error> {
    let text = path.to_str().ok_or_else(|| {
        Error::at(
            "PRODUCT_PREFLIGHT_PATH_UTF8",
            path,
            format!("{context} path is not UTF-8"),
        )
    })?;
    if text.len() <= limit && normalized_absolute(text) {
        return Ok(());
    }
    Err(Error::at(
        "PRODUCT_PREFLIGHT_PATH_FORM",
        path,
        format!("{context} path is not normalized and bounded"),
    ))
}

fn normalized_absolute(value: &str) -> bool {
    value.starts_with('/')
        && value != "/"
        && !value.ends_with('/')
        && !value.contains(['\0', '\r', '\n', '\\'])
        && !value.contains("//")
        && !value.split('/').any(|part| matches!(part, "." | ".."))
}

fn file_is_elf(path: &Path) -> Result<bool, Error> {
    let mut file = File::open(path).map_err(|failure| {
        Error::at("PRODUCT_PREFLIGHT_ELF_OPEN", path, failure.to_string())
    })?;
    let mut magic = [0_u8; 4];
    let count = file.read(&mut magic).map_err(|failure| {
        Error::at("PRODUCT_PREFLIGHT_ELF_READ", path, failure.to_string())
    })?;
    Ok(count == magic.len() && magic == *b"\x7fELF")
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        path::{Path, PathBuf},
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
    };

    use reovim_depgraph::{
        ArchitecturalRole, SourceClassification, SourceOwner, SourceProvenance,
        SourceUse,
    };

    use super::{ExposureKind, ExposureSpec, PRODUCT_FILES};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn canonical_toolchain_layout_and_flat_files_are_exact() {
        let tree = ToolchainFixture::new();
        assert_eq!(
            super::derive_sysroot(&tree.cargo, &tree.rustc).unwrap(),
            tree.sysroot
        );
        let observed = super::observe_flat_directory(
            &tree.support,
            Path::new("/compiler/lib"),
            Some("rustlib"),
            4_096,
            1_024,
        )
        .unwrap();
        let destinations = observed
            .iter()
            .map(|file| file.destination.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            destinations,
            BTreeSet::from([
                PathBuf::from("/compiler/lib/empty"),
                PathBuf::from("/compiler/lib/support"),
            ])
        );
        let empty = observed
            .iter()
            .find(|file| file.destination == Path::new("/compiler/lib/empty"))
            .unwrap();
        assert_eq!(empty.size_bytes, 0);
        assert!(
            observed
                .iter()
                .all(|file| file.destination.starts_with("/compiler/lib"))
        );

        let other = tree.root.join("other/cargo");
        tree.write("other/cargo", b"cargo");
        assert_eq!(
            super::derive_sysroot(&other, &tree.rustc).unwrap_err().code,
            "PRODUCT_PREFLIGHT_RUSTC_LAYOUT"
        );
    }

    #[cfg(unix)]
    #[test]
    fn compiler_entry_alias_and_limits_fail_closed() {
        use std::os::unix::fs::symlink;

        let tree = ToolchainFixture::new();
        let alias = tree.support.join("alias");
        symlink(&tree.rustc, &alias).unwrap();
        let failure = super::observe_flat_directory(
            &tree.support,
            Path::new("/compiler/lib"),
            Some("rustlib"),
            4_096,
            1_024,
        )
        .unwrap_err();
        assert_eq!(failure.code, "PRODUCT_PREFLIGHT_COMPILER_ENTRY");
        std::fs::remove_file(alias).unwrap();

        let failure = super::observe_flat_directory(
            &tree.support,
            Path::new("/compiler/lib"),
            Some("rustlib"),
            4_096,
            3,
        )
        .unwrap_err();
        assert_eq!(failure.code, "PRODUCT_PREFLIGHT_FILE_TYPE");
    }

    #[test]
    fn repository_selection_binds_exact_inventory_bytes() {
        let tree = ToolchainFixture::new();
        for (path, _) in PRODUCT_FILES {
            tree.write(path, path.as_bytes());
        }
        let mut inventory = product_inventory(&tree.root);
        let request = preflight_request(&tree, &inventory);
        let observed = super::observe_repository_files(&request).unwrap();
        let destinations = observed
            .iter()
            .map(|file| file.destination.clone())
            .collect::<BTreeSet<_>>();
        let expected = PRODUCT_FILES
            .iter()
            .map(|(path, _)| Path::new("/repo").join(path))
            .collect::<BTreeSet<_>>();
        assert_eq!(destinations, expected);
        assert!(observed.iter().all(|file| {
            file.destination.starts_with("/repo")
                && file.destination != Path::new("/repo")
        }));
        let registry = super::registry_identity(&observed).unwrap();
        assert_eq!(
            registry.destination,
            Path::new("/repo/conformance/product-closures.json")
        );
        assert_eq!(
            registry.source,
            tree.root.join("conformance/product-closures.json")
        );
        assert_eq!(
            registry.content_digest,
            crate::digest::bytes(b"conformance/product-closures.json")
        );
        let exposure =
            super::file_exposure(&registry, false, "Product repository input");
        assert!(!exposure.executable);
        assert_eq!(exposure.purpose, "Product repository input");

        let classified = inventory
            .files
            .iter()
            .find(|file| file.path == Path::new(super::REGISTRY_RELATIVE))
            .unwrap();
        assert_eq!(
            classified.classification.owner,
            SourceOwner::RepositoryTooling
        );
        assert_eq!(classified.classification.source_use, SourceUse::Tool);

        inventory.files[0].content_digest = crate::digest::bytes(b"wrong");
        let request = preflight_request(&tree, &inventory);
        assert_eq!(
            super::observe_repository_files(&request).unwrap_err().code,
            "PRODUCT_PREFLIGHT_SOURCE_IDENTITY"
        );
    }

    #[test]
    fn registry_identity_rejects_omission_and_duplication() {
        assert_eq!(
            super::registry_identity(&[]).unwrap_err().code,
            "PRODUCT_PREFLIGHT_REGISTRY"
        );
        let registry = super::FileIdentity {
            source: PathBuf::from("/host/product-closures.json"),
            destination: PathBuf::from(super::PREPARED_REGISTRY),
            content_digest: crate::digest::bytes(b"registry"),
            size_bytes: 8,
        };
        assert_eq!(
            super::registry_identity(&[registry.clone(), registry])
                .unwrap_err()
                .code,
            "PRODUCT_PREFLIGHT_REGISTRY"
        );
    }

    #[test]
    fn compiler_sources_are_the_exact_four_library_sets() {
        let sources = super::compiler_source_selection();
        assert!(sources.foundation_consumer.contains(Path::new(
            "tests/foundation-public-consumer/src/public_items.rs"
        )));
        assert!(sources.kernel_interface_consumer.contains(Path::new(
            "tests/kernel-interface-public-consumer/src/public_items.rs"
        )));
        assert!(!sources.kernel_interface_consumer.contains(Path::new(
            "tests/kernel-interface-public-consumer/tests/public_contract.rs"
        )));
    }

    #[test]
    fn runtime_deduplication_allows_source_aliases_but_closes_destinations() {
        let shared = mapping("/lib/libc.so.6", "/lib/libc.so.6", '1');
        let runtime =
            runtimes(vec![shared.clone()], vec![shared.clone()], vec![shared]);
        let deduplicated = super::deduplicate_runtime_files(&runtime).unwrap();
        let destinations = deduplicated
            .iter()
            .map(|file| file.destination.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            destinations,
            BTreeSet::from([PathBuf::from("/lib/libc.so.6")])
        );

        let aliases = runtimes(
            vec![mapping("/lib/loader.so", "/lib64/loader.so", '1')],
            vec![mapping("/lib/loader.so", "/usr/lib/loader.so", '1')],
            Vec::new(),
        );
        let destinations = super::deduplicate_runtime_files(&aliases)
            .unwrap()
            .into_iter()
            .map(|file| file.destination)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            destinations,
            BTreeSet::from([
                PathBuf::from("/lib64/loader.so"),
                PathBuf::from("/usr/lib/loader.so"),
            ])
        );
        let conflict = runtimes(
            vec![mapping("/lib/one.so", "/lib/lib.so", '1')],
            vec![mapping("/lib/two.so", "/lib/lib.so", '2')],
            Vec::new(),
        );
        assert_eq!(
            super::deduplicate_runtime_files(&conflict).unwrap_err().code,
            "PRODUCT_PREFLIGHT_RUNTIME_CONFLICT"
        );

        let reserved = runtimes(
            vec![mapping("/lib/one.so", "/checker", '1')],
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(
            super::deduplicate_runtime_files(&reserved).unwrap_err().code,
            "PRODUCT_PREFLIGHT_RUNTIME_DESTINATION"
        );
    }

    #[test]
    fn exposure_overlap_and_count_bounds_fail_closed() {
        let limits = crate::topology::HARD_LIMITS;
        let mut values = vec![
            exposure("/host/root", "/prepared", ExposureKind::Directory),
            exposure("/host/file", "/prepared/file", ExposureKind::File),
        ];
        assert_eq!(
            super::canonicalize_exposures(&mut values, limits)
                .unwrap_err()
                .code,
            "PRODUCT_PREFLIGHT_EXPOSURE_OVERLAP"
        );

        let mut tight = limits;
        tight.product_execution.exposure_entry_count = 1;
        let mut values = vec![
            exposure("/host/one", "/one", ExposureKind::File),
            exposure("/host/two", "/two", ExposureKind::File),
        ];
        assert_eq!(
            super::canonicalize_exposures(&mut values, tight).unwrap_err().code,
            "PRODUCT_PREFLIGHT_EXPOSURE_LIMIT"
        );
    }

    #[test]
    fn real_host_layout_and_runtime_parser_are_compatible() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let cargo = rustup_which("cargo");
        let rustc = rustup_which("rustc");
        let checker = std::env::current_exe().unwrap().canonicalize().unwrap();
        let sysroot = super::derive_sysroot(&cargo, &rustc).unwrap();
        let support = sysroot.join("lib");
        let target = support.join("rustlib").join(super::TARGET).join("lib");
        super::canonical_directory(&support, 4_096).unwrap();
        super::canonical_directory(&target, 4_096).unwrap();
        assert!(PRODUCT_FILES.iter().all(|(path, _)| {
            root.join(path).is_file() && !path.starts_with(".git")
        }));

        let limits = super::runtime_limits(crate::topology::HARD_LIMITS);
        let cargo_runtime = super::observe_runtime(
            &cargo,
            Path::new("/compiler/bin/cargo"),
            &[],
            limits,
        )
        .unwrap();
        assert!(cargo_runtime.mappings.iter().all(|mapping| {
            mapping.destination == Path::new("/lib64/ld-linux-x86-64.so.2")
                || mapping.destination == mapping.source
        }));
        let checker_runtime = super::observe_runtime(
            &checker,
            Path::new(super::PREPARED_CHECKER),
            &[],
            limits,
        )
        .unwrap();
        assert_ne!(cargo_runtime.mappings, Vec::new());
        assert_ne!(checker_runtime.mappings, Vec::new());
    }

    fn mapping(
        source: &str,
        destination: &str,
        marker: char,
    ) -> crate::runtime_closure::Mapping {
        let bytes = marker.to_string();
        crate::runtime_closure::Mapping {
            source: PathBuf::from(source),
            destination: PathBuf::from(destination),
            content_digest: crate::digest::bytes(bytes.as_bytes()),
            size_bytes: 1,
        }
    }

    fn runtimes(
        cargo: Vec<crate::runtime_closure::Mapping>,
        rustc: Vec<crate::runtime_closure::Mapping>,
        checker: Vec<crate::runtime_closure::Mapping>,
    ) -> super::ToolRuntimeObservations {
        super::ToolRuntimeObservations {
            cargo: runtime_observation(cargo),
            rustc: runtime_observation(rustc),
            checker: runtime_observation(checker),
        }
    }

    fn runtime_observation(
        mappings: Vec<crate::runtime_closure::Mapping>,
    ) -> crate::runtime_closure::Observation {
        let total_bytes = u64::try_from(mappings.len()).unwrap();
        crate::runtime_closure::Observation {
            object_count: mappings.len(),
            total_bytes,
            mappings,
            aggregate_digest: crate::digest::bytes(b"runtime observation"),
        }
    }

    fn exposure(
        source: &str,
        destination: &str,
        kind: ExposureKind,
    ) -> ExposureSpec {
        ExposureSpec {
            id: destination.to_owned(),
            source: PathBuf::from(source),
            destination: PathBuf::from(destination),
            kind,
            executable: false,
            purpose: "fixture".to_owned(),
        }
    }

    fn rustup_which(tool: &str) -> PathBuf {
        let output =
            Command::new("rustup").args(["which", tool]).output().unwrap();
        assert!(output.status.success());
        let text = std::str::from_utf8(&output.stdout).unwrap().trim();
        PathBuf::from(text).canonicalize().unwrap()
    }

    fn preflight_request<'a>(
        tree: &'a ToolchainFixture,
        inventory: &'a crate::source::Inventory,
    ) -> super::Request<'a> {
        super::Request {
            repository_root: &tree.root,
            cargo: &tree.cargo,
            rustc: &tree.rustc,
            checker: &tree.rustc,
            inventory,
            limits: crate::topology::HARD_LIMITS,
        }
    }

    fn product_inventory(root: &Path) -> crate::source::Inventory {
        let files = PRODUCT_FILES
            .iter()
            .copied()
            .map(|(path, role)| classified_file(root, path, role))
            .collect();
        crate::source::Inventory {
            files,
            exclusions: Vec::new(),
            tree_digest: crate::digest::bytes(b"product-preflight-fixture"),
            findings: Vec::new(),
        }
    }

    fn classified_file(
        root: &Path,
        path: &str,
        role: super::FileRole,
    ) -> crate::source::ClassifiedFile {
        let (owner, source_use) = match role {
            super::FileRole::Tool => {
                (SourceOwner::RepositoryTooling, SourceUse::Tool)
            }
            super::FileRole::Foundation => (
                SourceOwner::Product(ArchitecturalRole::Foundation),
                SourceUse::Production,
            ),
            super::FileRole::Consumer => (
                SourceOwner::Product(ArchitecturalRole::KernelInterface),
                SourceUse::Test,
            ),
            super::FileRole::KernelInterface => (
                SourceOwner::Product(ArchitecturalRole::KernelInterface),
                SourceUse::Production,
            ),
            super::FileRole::KernelInterfaceConsumer => (
                SourceOwner::Product(ArchitecturalRole::KernelRuntime),
                SourceUse::Test,
            ),
        };
        let relative = PathBuf::from(path);
        crate::source::ClassifiedFile {
            content_digest: crate::digest::file_bounded(
                &root.join(&relative),
                crate::topology::HARD_LIMITS.source_file_bytes,
            )
            .unwrap(),
            path: relative.clone(),
            classification: SourceClassification::new(
                owner,
                SourceProvenance::Authored,
                source_use,
            )
            .unwrap(),
            upstream: None,
            rule_prefix: relative,
            rule_precedence: 1,
        }
    }

    struct ToolchainFixture {
        root: PathBuf,
        sysroot: PathBuf,
        support: PathBuf,
        cargo: PathBuf,
        rustc: PathBuf,
    }

    impl ToolchainFixture {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "reovim-product-preflight-{}-{id}",
                std::process::id()
            ));
            std::fs::create_dir(&root).unwrap();
            let root = root.canonicalize().unwrap();
            let sysroot = root.join("toolchain");
            let support = sysroot.join("lib");
            std::fs::create_dir_all(support.join("rustlib")).unwrap();
            let fixture = Self {
                cargo: sysroot.join("bin/cargo"),
                rustc: sysroot.join("bin/rustc"),
                root,
                sysroot,
                support,
            };
            fixture.write("toolchain/bin/cargo", b"cargo");
            fixture.write("toolchain/bin/rustc", b"rustc");
            fixture.write("toolchain/lib/empty", b"");
            fixture.write("toolchain/lib/support", b"support");
            fixture
        }

        fn write(&self, relative: &str, bytes: &[u8]) {
            let path = self.root.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
    }

    impl Drop for ToolchainFixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).unwrap();
        }
    }
}
