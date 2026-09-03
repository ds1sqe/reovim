//! Closed source proof for the initial authored Product profile.
//!
//! Rustc dep-info selects the source bytes. This module then proves the
//! directly authored public surface and the external consumer witnesses
//! without attempting general Rust name resolution.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::{Component, Path, PathBuf},
};

use reovim_depgraph::{
    ArchitecturalRole, SourceOwner, SourceProvenance, SourceUse,
};
use syn::{
    Attribute, ExprCall, File, GenericArgument, ImplItem, Item, Meta,
    Path as SynPath, PathArguments, Token, Type, TypeParamBound, UseTree,
    Visibility,
    parse::Parser as _,
    punctuated::Punctuated,
    visit::{self, Visit},
};

use crate::{
    digest,
    product_closures::{
        BuiltinDerive, Closure, ClosureKind, ConsumerOperation, DeriveCheck,
        PublicConsumerRelation, PublicItem, PublicItemKind, PublicReexport,
    },
    source::{ClassifiedFile, read_bounded},
};

const FOUNDATION_CRATE: &str = "reovim_foundation";
const KERNEL_INTERFACE_CRATE: &str = "reovim_kernel_interface";
const DERIVE_WITNESS: &str = "derive_witness";
const CLONE_COPY_WITNESS: &str = "clone_copy_witness";
const CLONE_DEBUG_EQ_WITNESS: &str = "clone_debug_eq_witness";

#[derive(Clone, Copy, PartialEq, Eq)]
enum PairProfile {
    Foundation,
    KernelInterface,
}

struct ClosureProfile {
    id: &'static str,
    kind: ClosureKind,
    package_id: &'static str,
    package_name: &'static str,
    manifest: &'static str,
    target_name: &'static str,
    source: &'static str,
    role: ArchitecturalRole,
    source_use: SourceUse,
}

struct PairProfileSpec {
    relation: &'static str,
    public_crate: &'static str,
    shipping: ClosureProfile,
    consumer: ClosureProfile,
}

const FOUNDATION_PROFILE: PairProfileSpec = PairProfileSpec {
    relation: "foundation-public-api",
    public_crate: FOUNDATION_CRATE,
    shipping: ClosureProfile {
        id: "foundation-shipping",
        kind: ClosureKind::Shipping,
        package_id: "foundation",
        package_name: "reovim-foundation",
        manifest: "kernel/foundation/Cargo.toml",
        target_name: "reovim_foundation",
        source: "kernel/foundation/src/lib.rs",
        role: ArchitecturalRole::Foundation,
        source_use: SourceUse::Production,
    },
    consumer: ClosureProfile {
        id: "foundation-consumer",
        kind: ClosureKind::Validation,
        package_id: "foundation-public-consumer",
        package_name: "reovim-foundation-public-consumer",
        manifest: "tests/foundation-public-consumer/Cargo.toml",
        target_name: "reovim_foundation_public_consumer",
        source: "tests/foundation-public-consumer/src/lib.rs",
        role: ArchitecturalRole::KernelInterface,
        source_use: SourceUse::Test,
    },
};

const KERNEL_INTERFACE_PROFILE: PairProfileSpec = PairProfileSpec {
    relation: "kernel-interface-public-api",
    public_crate: KERNEL_INTERFACE_CRATE,
    shipping: ClosureProfile {
        id: "kernel-interface-shipping",
        kind: ClosureKind::Shipping,
        package_id: "kernel-interface",
        package_name: "reovim-kernel-interface",
        manifest: "kernel/interface/Cargo.toml",
        target_name: "reovim_kernel_interface",
        source: "kernel/interface/src/lib.rs",
        role: ArchitecturalRole::KernelInterface,
        source_use: SourceUse::Production,
    },
    consumer: ClosureProfile {
        id: "kernel-interface-consumer",
        kind: ClosureKind::Validation,
        package_id: "kernel-interface-public-consumer",
        package_name: "reovim-kernel-interface-public-consumer",
        manifest: "tests/kernel-interface-public-consumer/Cargo.toml",
        target_name: "reovim_kernel_interface_public_consumer",
        source: "tests/kernel-interface-public-consumer/src/lib.rs",
        role: ArchitecturalRole::KernelRuntime,
        source_use: SourceUse::Test,
    },
};

/// Caller-owned limits for one Product source analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnalysisLimits {
    /// Maximum compiler-selected source files across both closures.
    pub source_files: usize,
    /// Maximum bytes read from one source file.
    pub source_file_bytes: u64,
    /// Maximum aggregate bytes read across both closures.
    pub source_total_bytes: u64,
    /// Maximum counted syntax nodes in one source file.
    pub syntax_nodes: usize,
}

/// Exact inputs for one shipping/public-consumer relation.
pub struct AnalysisRequest<'a> {
    /// Repository root containing every selected source path.
    pub root: &'a Path,
    /// Registered shipping closure.
    pub shipping: &'a Closure,
    /// Registered validation closure.
    pub consumer: &'a Closure,
    /// Public relation joining the two closures.
    pub relation: &'a PublicConsumerRelation,
    /// Repository-relative shipping paths reported by rustc dep-info.
    pub shipping_sources: &'a BTreeSet<PathBuf>,
    /// Repository-relative consumer paths reported by rustc dep-info.
    pub consumer_sources: &'a BTreeSet<PathBuf>,
    /// Digest-bound repository source inventory.
    pub inventory: &'a [ClassifiedFile],
    /// Explicit source and syntax limits.
    pub limits: AnalysisLimits,
}

/// Compiler-proved Foundation edge required before KI alias resolution.
pub struct CompilerJoin<'a> {
    /// Exact consuming shipping closure.
    pub consumer_closure: &'a str,
    /// Exact rustc extern name supplied to the KI compiler unit.
    pub extern_name: &'a str,
    /// Exact provider shipping closure.
    pub provider_closure: &'a str,
    /// Public relation whose already-proved facts supply alias origins.
    pub provider_relation: &'a str,
    /// Exact extern metadata path observed on the KI compiler unit.
    pub extern_path: &'a Path,
    /// Exact Foundation provider metadata identity path.
    pub provider_rmeta_path: &'a Path,
}

/// Exact Foundation and Kernel Interface source-analysis request.
pub struct TwoPairAnalysisRequest<'a> {
    /// Dependency-free Foundation pair, which is always analyzed first.
    pub foundation: AnalysisRequest<'a>,
    /// Foundation-consuming Kernel Interface pair.
    pub kernel_interface: AnalysisRequest<'a>,
    /// Caller-supplied compiler evidence joining KI to Foundation metadata.
    pub compiler_join: CompilerJoin<'a>,
}

/// Source-derived facts retained after the closed checks pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductSourceObservation {
    /// Exact public-consumer relation proved by this observation.
    pub relation: String,
    /// Exact direct public modules, types, and callable items.
    pub public_items: Vec<PublicItem>,
    /// Exact direct built-in derives and their fixed operations.
    pub derive_checks: Vec<DeriveCheck>,
    /// Exact public types named by external derive witnesses.
    pub witness_types: Vec<String>,
    /// Exact shipping source paths analyzed after the dep-info join.
    pub shipping_sources: Vec<PathBuf>,
    /// Exact consumer source paths analyzed after the dep-info join.
    pub consumer_sources: Vec<PathBuf>,
}

/// Ordered source-derived observations for the exact two-pair profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TwoPairSourceObservation {
    /// Foundation facts, proved without a Product provider.
    pub foundation: ProductSourceObservation,
    /// KI facts, including Foundation-derived public aliases.
    pub kernel_interface: ProductSourceObservation,
}

/// One fail-closed Product source proof error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductSourceError {
    /// Stable checker code.
    pub code: &'static str,
    /// Responsible repository-relative path, when one exists.
    pub path: Option<PathBuf>,
    /// Concise diagnostic detail.
    pub message: String,
}

impl ProductSourceError {
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

impl fmt::Display for ProductSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(path) => write!(
                formatter,
                "{}: {}: {}",
                self.code,
                path.display(),
                self.message
            ),
            None => write!(formatter, "{}: {}", self.code, self.message),
        }
    }
}

impl std::error::Error for ProductSourceError {}

/// Proves the initial authored Foundation surface and external consumer.
///
/// # Errors
///
/// Returns a typed error when compiler-selected source identity, the closed
/// syntax profile, the registered public surface, or consumer witnesses do
/// not match exactly.
#[cfg(test)]
pub fn analyze(
    request: &AnalysisRequest<'_>,
) -> Result<ProductSourceObservation, ProductSourceError> {
    validate_limits(request.limits)?;
    validate_source_count(request)?;
    let inventory = inventory_index(request.inventory)?;
    let mut budget = ReadBudget::new(request.limits.source_total_bytes);
    analyze_pair(
        request,
        PairProfile::Foundation,
        None,
        &inventory,
        &mut budget,
    )
}

/// Proves Foundation first, then the exact Foundation-consuming KI pair.
///
/// The compiler join must be constructed from KI shipping `--extern`
/// evidence and the exact Foundation provider `rmeta` identity. This function
/// validates their path equality before any Foundation fact can authorize a
/// KI public re-export.
///
/// # Errors
///
/// Returns a typed error for a non-exact pair, compiler join, source set,
/// public surface, alias origin, or consumer witness.
pub fn analyze_two_pairs(
    request: &TwoPairAnalysisRequest<'_>,
) -> Result<TwoPairSourceObservation, ProductSourceError> {
    validate_two_pair_request(request)?;
    let inventory = inventory_index(request.foundation.inventory)?;
    let mut budget =
        ReadBudget::new(request.foundation.limits.source_total_bytes);
    let foundation = analyze_pair(
        &request.foundation,
        PairProfile::Foundation,
        None,
        &inventory,
        &mut budget,
    )?;
    validate_compiler_join(request)?;
    let provider = ProvenProvider {
        relation: request.foundation.relation,
        observation: &foundation,
    };
    let kernel_interface = analyze_pair(
        &request.kernel_interface,
        PairProfile::KernelInterface,
        Some(&provider),
        &inventory,
        &mut budget,
    )?;
    Ok(TwoPairSourceObservation { foundation, kernel_interface })
}

struct ProvenProvider<'a> {
    relation: &'a PublicConsumerRelation,
    observation: &'a ProductSourceObservation,
}

fn analyze_pair(
    request: &AnalysisRequest<'_>,
    profile: PairProfile,
    provider: Option<&ProvenProvider<'_>>,
    inventory: &BTreeMap<PathBuf, &ClassifiedFile>,
    budget: &mut ReadBudget,
) -> Result<ProductSourceObservation, ProductSourceError> {
    validate_relation(request, profile)?;
    let shipping = load_source_set(
        request.root,
        request.shipping,
        request.shipping_sources,
        inventory,
        request.limits,
        budget,
    )?;
    let consumer = load_source_set(
        request.root,
        request.consumer,
        request.consumer_sources,
        inventory,
        request.limits,
        budget,
    )?;
    let surface = analyze_shipping(
        &shipping,
        request.shipping,
        request.relation,
        profile,
        provider,
    )?;
    validate_surface(&surface, request.relation)?;
    let witness_types = analyze_consumer(
        &consumer,
        request.consumer,
        request.relation,
        profile,
    )?;

    Ok(ProductSourceObservation {
        relation: request.relation.id.clone(),
        public_items: surface.public_items.into_iter().collect(),
        derive_checks: surface.derive_checks.into_iter().collect(),
        witness_types: witness_types.into_iter().collect(),
        shipping_sources: request.shipping_sources.iter().cloned().collect(),
        consumer_sources: request.consumer_sources.iter().cloned().collect(),
    })
}

fn validate_two_pair_request(
    request: &TwoPairAnalysisRequest<'_>,
) -> Result<(), ProductSourceError> {
    let foundation = &request.foundation;
    let kernel_interface = &request.kernel_interface;
    validate_limits(foundation.limits)?;
    if foundation.root != kernel_interface.root
        || foundation.limits != kernel_interface.limits
        || !std::ptr::eq(foundation.inventory, kernel_interface.inventory)
    {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_TWO_PAIR_AUTHORITY",
            "two-pair source authority, inventory, or limits differ",
        ));
    }
    let counts = [
        foundation.shipping_sources.len(),
        foundation.consumer_sources.len(),
        kernel_interface.shipping_sources.len(),
        kernel_interface.consumer_sources.len(),
    ];
    let total = counts.into_iter().try_fold(0_usize, usize::checked_add);
    if total.is_none_or(|value| value > foundation.limits.source_files) {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_LIMIT",
            "two-pair compiler-selected source count exceeds the limit",
        ));
    }
    Ok(())
}

fn validate_compiler_join(
    request: &TwoPairAnalysisRequest<'_>,
) -> Result<(), ProductSourceError> {
    let join = &request.compiler_join;
    let exact = join.consumer_closure == KERNEL_INTERFACE_PROFILE.shipping.id
        && join.consumer_closure == request.kernel_interface.shipping.id
        && join.extern_name == FOUNDATION_CRATE
        && join.provider_closure == FOUNDATION_PROFILE.shipping.id
        && join.provider_closure == request.foundation.shipping.id
        && join.provider_relation == FOUNDATION_PROFILE.relation
        && join.provider_relation == request.foundation.relation.id
        && join.extern_path == join.provider_rmeta_path
        && exact_rmeta_path(join.extern_path);
    if exact {
        return Ok(());
    }
    Err(ProductSourceError::new(
        "PRODUCT_SOURCE_COMPILER_JOIN",
        "KI compiler extern does not join the exact Foundation rmeta",
    ))
}

fn exact_rmeta_path(path: &Path) -> bool {
    path.is_absolute()
        && path != Path::new("/")
        && path.extension().is_some_and(|value| value == "rmeta")
        && path.components().all(|part| {
            !matches!(part, Component::CurDir | Component::ParentDir)
        })
}

#[cfg(test)]
fn validate_source_count(
    request: &AnalysisRequest<'_>,
) -> Result<(), ProductSourceError> {
    let count = request
        .shipping_sources
        .len()
        .checked_add(request.consumer_sources.len())
        .ok_or_else(|| {
            ProductSourceError::new(
                "PRODUCT_SOURCE_LIMIT",
                "compiler-selected source count overflow",
            )
        })?;
    if count > request.limits.source_files {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_LIMIT",
            "aggregate compiler-selected source count exceeds the limit",
        ));
    }
    Ok(())
}

fn validate_limits(limits: AnalysisLimits) -> Result<(), ProductSourceError> {
    if limits.source_files == 0
        || limits.source_file_bytes == 0
        || limits.source_total_bytes == 0
        || limits.syntax_nodes == 0
    {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_LIMIT",
            "every Product source limit must be positive",
        ));
    }
    Ok(())
}

impl PairProfile {
    const fn spec(self) -> &'static PairProfileSpec {
        match self {
            Self::Foundation => &FOUNDATION_PROFILE,
            Self::KernelInterface => &KERNEL_INTERFACE_PROFILE,
        }
    }

    const fn item_witness_source(self) -> &'static str {
        match self {
            Self::Foundation => {
                "tests/foundation-public-consumer/src/public_items.rs"
            }
            Self::KernelInterface => {
                "tests/kernel-interface-public-consumer/src/public_items.rs"
            }
        }
    }
}

fn validate_relation(
    request: &AnalysisRequest<'_>,
    profile: PairProfile,
) -> Result<(), ProductSourceError> {
    let spec = profile.spec();
    validate_closure(request.shipping, &spec.shipping)?;
    validate_closure(request.consumer, &spec.consumer)?;
    if request.relation.id != spec.relation
        || request.relation.shipping_closure != request.shipping.id
        || request.relation.consumer_closure != request.consumer.id
    {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_RELATION",
            "public relation does not name the supplied exact closures",
        ));
    }
    if profile == PairProfile::Foundation
        && !request.relation.reexports.is_empty()
    {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_REEXPORT_SET",
            "Foundation relation cannot declare public re-exports",
        ));
    }
    Ok(())
}

fn validate_closure(
    closure: &Closure,
    expected: &ClosureProfile,
) -> Result<(), ProductSourceError> {
    if closure.id != expected.id
        || closure.kind != expected.kind
        || closure.workspace.id != "product"
        || closure.workspace.manifest != Path::new("product/Cargo.toml")
        || closure.workspace.lockfile != Path::new("product/Cargo.lock")
        || closure.package.id != expected.package_id
        || closure.package.name != expected.package_name
        || closure.package.manifest != Path::new(expected.manifest)
        || closure.target.id != "library"
        || closure.target.name != expected.target_name
        || closure.target.src_path != Path::new(expected.source)
        || closure.architectural_role != expected.role
        || closure.source_use != expected.source_use
    {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_ROLE",
            format!("closure {} has the wrong role or source use", closure.id),
        ));
    }
    if closure.target.kind.as_slice() != ["lib"]
        || closure.target.crate_types.as_slice() != ["lib"]
        || closure.cargo.profile != "release"
        || closure.package_membership.is_some()
            != (expected.kind == ClosureKind::Shipping)
    {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_TARGET",
            format!("closure {} is not the exact release lib", closure.id),
        ));
    }
    Ok(())
}

fn inventory_index(
    files: &[ClassifiedFile],
) -> Result<BTreeMap<PathBuf, &ClassifiedFile>, ProductSourceError> {
    let mut output = BTreeMap::new();
    for file in files {
        if output.insert(file.path.clone(), file).is_some() {
            return Err(ProductSourceError::at(
                "PRODUCT_SOURCE_INVENTORY",
                &file.path,
                "source inventory repeats one path",
            ));
        }
    }
    Ok(output)
}

struct ReadBudget {
    remaining: u64,
}

impl ReadBudget {
    const fn new(total: u64) -> Self {
        Self { remaining: total }
    }

    fn charge(
        &mut self,
        path: &Path,
        bytes: usize,
    ) -> Result<(), ProductSourceError> {
        let bytes = u64::try_from(bytes).map_err(|error| {
            ProductSourceError::at(
                "PRODUCT_SOURCE_LIMIT",
                path,
                error.to_string(),
            )
        })?;
        let Some(remaining) = self.remaining.checked_sub(bytes) else {
            return Err(ProductSourceError::at(
                "PRODUCT_SOURCE_LIMIT",
                path,
                "aggregate Product source bytes exceed the limit",
            ));
        };
        self.remaining = remaining;
        Ok(())
    }
}

struct ParsedSource {
    syntax: File,
}

struct SourceSet {
    files: BTreeMap<PathBuf, ParsedSource>,
}

impl SourceSet {
    fn syntax(&self, path: &Path) -> Result<&File, ProductSourceError> {
        self.files.get(path).map(|source| &source.syntax).ok_or_else(|| {
            ProductSourceError::at(
                "PRODUCT_COMPILER_SOURCE_SET",
                path,
                "module source is absent from compiler-selected inputs",
            )
        })
    }

    fn paths(&self) -> BTreeSet<PathBuf> {
        self.files.keys().cloned().collect()
    }
}

fn load_source_set(
    root: &Path,
    closure: &Closure,
    selected: &BTreeSet<PathBuf>,
    inventory: &BTreeMap<PathBuf, &ClassifiedFile>,
    limits: AnalysisLimits,
    budget: &mut ReadBudget,
) -> Result<SourceSet, ProductSourceError> {
    if selected.len() > limits.source_files {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_LIMIT",
            "compiler-selected source count exceeds the limit",
        ));
    }
    if !selected.contains(&closure.target.src_path) {
        return Err(ProductSourceError::at(
            "PRODUCT_COMPILER_SOURCE_SET",
            &closure.target.src_path,
            "target root is absent from compiler-selected inputs",
        ));
    }
    let package_root = package_root(closure)?;
    let mut files = BTreeMap::new();
    for path in selected {
        validate_selected_path(path, &package_root)?;
        let source =
            load_source(root, path, closure, inventory, limits, budget)?;
        files.insert(path.clone(), source);
    }
    Ok(SourceSet { files })
}

fn package_root(closure: &Closure) -> Result<PathBuf, ProductSourceError> {
    closure.package.manifest.parent().map(Path::to_path_buf).ok_or_else(|| {
        ProductSourceError::at(
            "PRODUCT_SOURCE_PACKAGE",
            &closure.package.manifest,
            "package manifest has no parent directory",
        )
    })
}

fn validate_selected_path(
    path: &Path,
    package_root: &Path,
) -> Result<(), ProductSourceError> {
    let safe = !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path.components().all(|part| matches!(part, Component::Normal(_)));
    if !safe || !path.starts_with(package_root) {
        return Err(ProductSourceError::at(
            "PRODUCT_COMPILER_SOURCE_SET",
            path,
            "compiler source is outside its exact package root",
        ));
    }
    if path.extension().is_none_or(|extension| extension != "rs") {
        return Err(ProductSourceError::at(
            "PRODUCT_COMPILER_SOURCE_SET",
            path,
            "compiler source is not a Rust file",
        ));
    }
    Ok(())
}

fn load_source(
    root: &Path,
    path: &Path,
    closure: &Closure,
    inventory: &BTreeMap<PathBuf, &ClassifiedFile>,
    limits: AnalysisLimits,
    budget: &mut ReadBudget,
) -> Result<ParsedSource, ProductSourceError> {
    let file = inventory.get(path).ok_or_else(|| {
        ProductSourceError::at(
            "PRODUCT_COMPILER_SOURCE_SET",
            path,
            "compiler source is absent from the repository inventory",
        )
    })?;
    validate_classification(file, closure)?;
    let bytes = read_bounded(&root.join(path), limits.source_file_bytes)
        .map_err(|message| {
            ProductSourceError::at("PRODUCT_SOURCE_READ", path, message)
        })?;
    budget.charge(path, bytes.len())?;
    if digest::bytes(&bytes) != file.content_digest {
        return Err(ProductSourceError::at(
            "PRODUCT_SOURCE_INVENTORY_DRIFT",
            path,
            "parsed source bytes differ from the inventory digest",
        ));
    }
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        ProductSourceError::at("PRODUCT_SOURCE_UTF8", path, error.to_string())
    })?;
    let syntax = syn::parse_file(text).map_err(|error| {
        ProductSourceError::at("PRODUCT_SOURCE_PARSE", path, error.to_string())
    })?;
    enforce_syntax_limit(&syntax, path, limits.syntax_nodes)?;
    Ok(ParsedSource { syntax })
}

fn validate_classification(
    file: &ClassifiedFile,
    closure: &Closure,
) -> Result<(), ProductSourceError> {
    let expected = SourceOwner::Product(closure.architectural_role);
    let classification = file.classification;
    if classification.owner != expected
        || classification.provenance != SourceProvenance::Authored
        || classification.source_use != closure.source_use
    {
        return Err(ProductSourceError::at(
            "PRODUCT_SOURCE_CLASSIFICATION",
            &file.path,
            "compiler source classification disagrees with its closure",
        ));
    }
    Ok(())
}

fn enforce_syntax_limit(
    syntax: &File,
    path: &Path,
    limit: usize,
) -> Result<(), ProductSourceError> {
    let mut counter = NodeCounter::new(limit);
    counter.visit_file(syntax);
    if counter.exceeded {
        return Err(ProductSourceError::at(
            "PRODUCT_SOURCE_SYNTAX_LIMIT",
            path,
            format!("syntax node count exceeds {limit}"),
        ));
    }
    Ok(())
}

struct NodeCounter {
    count: usize,
    limit: usize,
    exceeded: bool,
}

impl NodeCounter {
    const fn new(limit: usize) -> Self {
        Self { count: 0, limit, exceeded: false }
    }

    const fn enter(&mut self) -> bool {
        if self.exceeded {
            return false;
        }
        self.count = self.count.saturating_add(1);
        self.exceeded = self.count > self.limit;
        !self.exceeded
    }
}

impl<'ast> Visit<'ast> for NodeCounter {
    fn visit_item(&mut self, item: &'ast Item) {
        if self.enter() {
            visit::visit_item(self, item);
        }
    }

    fn visit_expr(&mut self, expression: &'ast syn::Expr) {
        if self.enter() {
            visit::visit_expr(self, expression);
        }
    }

    fn visit_type(&mut self, ty: &'ast Type) {
        if self.enter() {
            visit::visit_type(self, ty);
        }
    }

    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        if self.enter() {
            visit::visit_attribute(self, attribute);
        }
    }
}

#[derive(Clone, Copy)]
enum SourceMode {
    Shipping(PairProfile),
    Consumer(PairProfile),
}

impl SourceMode {
    const fn profile(self) -> PairProfile {
        match self {
            Self::Shipping(profile) | Self::Consumer(profile) => profile,
        }
    }
}

fn validate_source_profile(
    sources: &SourceSet,
    closure: &Closure,
    relation: &PublicConsumerRelation,
    mode: SourceMode,
    provider: Option<&ProvenProvider<'_>>,
) -> Result<(), ProductSourceError> {
    let names = declared_names(sources);
    let public_types = registered_public_types(relation);
    let public_traits = registered_public_traits(relation);
    for (path, source) in &sources.files {
        let root = path == &closure.target.src_path;
        validate_file_attributes(&source.syntax, path, root)?;
        validate_unsafe_syntax(&source.syntax, path)?;
        validate_closed_syntax(&source.syntax, path, mode)?;
        if matches!(mode, SourceMode::Shipping(_)) {
            validate_shipping_value_paths(&source.syntax, path)?;
        }
        validate_imports(
            &source.syntax,
            path,
            mode,
            relation,
            &names,
            provider,
        )?;
        validate_type_paths(
            &source.syntax,
            path,
            mode,
            &names,
            TypePathAuthority {
                public_types: &public_types,
                public_traits: &public_traits,
                item_witness: path
                    == Path::new(mode.profile().item_witness_source()),
            },
            provider,
        )?;
    }
    Ok(())
}

fn validate_shipping_value_paths(
    syntax: &File,
    path: &Path,
) -> Result<(), ProductSourceError> {
    let mut visitor = ShippingValuePathGuard { path, error: None };
    visitor.visit_file(syntax);
    visitor.error.map_or(Ok(()), Err)
}

struct ShippingValuePathGuard<'a> {
    path: &'a Path,
    error: Option<ProductSourceError>,
}

impl<'ast> Visit<'ast> for ShippingValuePathGuard<'_> {
    fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
        if self.error.is_some() {
            return;
        }
        let std_value =
            expression.path.segments.first().is_some_and(|segment| {
                matches!(segment.ident.to_string().as_str(), "std" | "r#std")
            });
        if std_value {
            self.error = Some(ProductSourceError::at(
                "PRODUCT_STD_VALUE_PATH_UNAVAILABLE",
                self.path,
                "standard-environment value path is unavailable",
            ));
            return;
        }
        visit::visit_expr_path(self, expression);
    }
}

fn validate_file_attributes(
    syntax: &File,
    path: &Path,
    root: bool,
) -> Result<(), ProductSourceError> {
    let mut forbid_count = 0_usize;
    for attribute in &syntax.attrs {
        if attribute.path().is_ident("doc") {
            continue;
        }
        if root && exact_forbid_unsafe(attribute) {
            forbid_count = forbid_count.saturating_add(1);
            continue;
        }
        return Err(attribute_error(attribute, path));
    }
    if root && forbid_count != 1 {
        return Err(ProductSourceError::at(
            "PRODUCT_FORBID_UNSAFE",
            path,
            "crate root requires exactly one forbid(unsafe_code)",
        ));
    }
    Ok(())
}

fn exact_forbid_unsafe(attribute: &Attribute) -> bool {
    if !attribute.path().is_ident("forbid") {
        return false;
    }
    let Meta::List(list) = &attribute.meta else { return false };
    let parser = Punctuated::<SynPath, Token![,]>::parse_terminated;
    let Ok(paths) = parser.parse2(list.tokens.clone()) else { return false };
    paths.len() == 1 && paths[0].is_ident("unsafe_code")
}

fn attribute_error(attribute: &Attribute, path: &Path) -> ProductSourceError {
    let code = if attribute.path().is_ident("cfg")
        || attribute.path().is_ident("cfg_attr")
    {
        "UNSUPPORTED_CFG_PROOF_GAP"
    } else if attribute.path().is_ident("path") {
        "UNSUPPORTED_PATH_MODULE"
    } else {
        "PRODUCT_ATTRIBUTE_UNAVAILABLE"
    };
    ProductSourceError::at(
        code,
        path,
        "attribute is outside the authored Product profile",
    )
}

fn validate_closed_syntax(
    syntax: &File,
    path: &Path,
    mode: SourceMode,
) -> Result<(), ProductSourceError> {
    let mut guard = SyntaxGuard { path, mode, error: None };
    for item in &syntax.items {
        guard.visit_item(item);
    }
    guard.error.map_or(Ok(()), Err)
}

struct SyntaxGuard<'a> {
    path: &'a Path,
    mode: SourceMode,
    error: Option<ProductSourceError>,
}

impl SyntaxGuard<'_> {
    fn fail(&mut self, code: &'static str, message: &'static str) {
        if self.error.is_none() {
            self.error = Some(ProductSourceError::at(code, self.path, message));
        }
    }
}

impl<'ast> Visit<'ast> for SyntaxGuard<'_> {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        if self.error.is_some() {
            return;
        }
        if let Err(error) = validate_item_attribute(attribute, self.path) {
            self.error = Some(error);
        }
    }

    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        if self.error.is_some() {
            return;
        }
        let name = item.path.segments.last().map(|part| part.ident.to_string());
        let code = match name.as_deref() {
            Some("include" | "include_bytes" | "include_str") => {
                "UNSUPPORTED_INCLUDE_PROOF_GAP"
            }
            Some("env" | "option_env") => "UNSUPPORTED_ENV_PROOF_GAP",
            _ => "UNSUPPORTED_MACRO_PROOF_GAP",
        };
        self.fail(code, "macro invocation is outside the authored profile");
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        let allowed = matches!(
            self.mode,
            SourceMode::Shipping(PairProfile::KernelInterface)
        );
        if public(&item.vis) && !allowed {
            self.fail(
                "UNSUPPORTED_REEXPORT_PROOF_GAP",
                "public re-export is outside the authored profile",
            );
            return;
        }
        visit::visit_item_use(self, item);
    }

    fn visit_item_type(&mut self, _item: &'ast syn::ItemType) {
        self.fail(
            "UNSUPPORTED_PUBLIC_ALIAS_PROOF_GAP",
            "type aliases are outside the authored profile",
        );
    }

    fn visit_item_extern_crate(&mut self, _item: &'ast syn::ItemExternCrate) {
        self.fail(
            "PRODUCT_EXTERN_CRATE_UNAVAILABLE",
            "extern crate syntax is outside the authored profile",
        );
    }

    fn visit_item_foreign_mod(&mut self, _item: &'ast syn::ItemForeignMod) {
        self.fail(
            "PRODUCT_EXTERN_BLOCK_UNAVAILABLE",
            "foreign blocks are outside the authored profile",
        );
    }

    fn visit_item_union(&mut self, _item: &'ast syn::ItemUnion) {
        self.fail(
            "PRODUCT_UNION_UNAVAILABLE",
            "union declarations are outside the authored profile",
        );
    }
}

fn validate_item_attribute(
    attribute: &Attribute,
    path: &Path,
) -> Result<(), ProductSourceError> {
    if attribute.path().is_ident("doc") {
        return Ok(());
    }
    if attribute.path().is_ident("derive")
        && matches!(attribute.meta, Meta::List(_))
    {
        return Ok(());
    }
    if attribute.path().is_ident("must_use")
        && matches!(attribute.meta, Meta::Path(_))
    {
        return Ok(());
    }
    Err(attribute_error(attribute, path))
}

fn validate_unsafe_syntax(
    syntax: &File,
    path: &Path,
) -> Result<(), ProductSourceError> {
    let mut visitor = UnsafeGuard { found: false };
    visitor.visit_file(syntax);
    if visitor.found {
        return Err(ProductSourceError::at(
            "FIRST_PARTY_UNSAFE_SYNTAX",
            path,
            "Product source contains unsafe syntax",
        ));
    }
    Ok(())
}

struct UnsafeGuard {
    found: bool,
}

impl<'ast> Visit<'ast> for UnsafeGuard {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        self.found |= attribute.path().is_ident("unsafe");
        visit::visit_attribute(self, attribute);
    }

    fn visit_safety(&mut self, safety: &'ast syn::Safety) {
        self.found |= matches!(safety, syn::Safety::Unsafe(_));
        visit::visit_safety(self, safety);
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.found |= matches!(item.sig.safety, syn::Safety::Unsafe(_));
        visit::visit_item_fn(self, item);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.found |= matches!(item.sig.safety, syn::Safety::Unsafe(_));
        visit::visit_impl_item_fn(self, item);
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        self.found |= matches!(item.sig.safety, syn::Safety::Unsafe(_));
        visit::visit_trait_item_fn(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        self.found |= item.unsafety.is_some();
        visit::visit_item_impl(self, item);
    }

    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        self.found |= item.unsafety.is_some();
        visit::visit_item_trait(self, item);
    }

    fn visit_item_foreign_mod(&mut self, item: &'ast syn::ItemForeignMod) {
        self.found |= item.unsafety.is_some();
        visit::visit_item_foreign_mod(self, item);
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        self.found |= item.unsafety.is_some();
        visit::visit_item_mod(self, item);
    }

    fn visit_type_fn_ptr(&mut self, ty: &'ast syn::TypeFnPtr) {
        self.found |= ty.unsafety.is_some();
        visit::visit_type_fn_ptr(self, ty);
    }

    fn visit_expr_unsafe(&mut self, expression: &'ast syn::ExprUnsafe) {
        self.found = true;
        visit::visit_expr_unsafe(self, expression);
    }

    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        self.found |= matches!(item.mutability, syn::StaticMutability::Mut(_));
        visit::visit_item_static(self, item);
    }
}

fn validate_imports(
    syntax: &File,
    path: &Path,
    mode: SourceMode,
    relation: &PublicConsumerRelation,
    names: &BTreeSet<String>,
    provider: Option<&ProvenProvider<'_>>,
) -> Result<(), ProductSourceError> {
    let mut visitor =
        ImportVisitor { path, mode, relation, names, provider, error: None };
    visitor.visit_file(syntax);
    visitor.error.map_or(Ok(()), Err)
}

struct ImportVisitor<'a> {
    path: &'a Path,
    mode: SourceMode,
    relation: &'a PublicConsumerRelation,
    names: &'a BTreeSet<String>,
    provider: Option<&'a ProvenProvider<'a>>,
    error: Option<ProductSourceError>,
}

impl<'ast> Visit<'ast> for ImportVisitor<'_> {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if self.error.is_some() {
            return;
        }
        if public(&item.vis) {
            return;
        }
        if item.leading_colon.is_some() {
            self.error = Some(ProductSourceError::at(
                "PRODUCT_IMPORT_UNAVAILABLE",
                self.path,
                "private import cannot use an extern-root marker",
            ));
            return;
        }
        let mut imports = Vec::new();
        if let Err(error) = flatten_use(&item.tree, Vec::new(), &mut imports) {
            self.error = Some(ProductSourceError::at(
                "PRODUCT_IMPORT_UNAVAILABLE",
                self.path,
                error,
            ));
            return;
        }
        if imports.iter().any(|parts| {
            !allowed_import(
                parts,
                self.mode,
                self.relation,
                self.names,
                self.provider,
            )
        }) {
            self.error = Some(ProductSourceError::at(
                "PRODUCT_IMPORT_UNAVAILABLE",
                self.path,
                "import is outside the closed source profile",
            ));
            return;
        }
        visit::visit_item_use(self, item);
    }
}

fn flatten_use(
    tree: &UseTree,
    mut prefix: Vec<String>,
    output: &mut Vec<Vec<String>>,
) -> Result<(), &'static str> {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            flatten_use(&path.tree, prefix, output)
        }
        UseTree::Name(name) => {
            prefix.push(name.ident.to_string());
            output.push(prefix);
            Ok(())
        }
        UseTree::Group(group) => {
            for tree in &group.items {
                flatten_use(tree, prefix.clone(), output)?;
            }
            Ok(())
        }
        UseTree::Rename(_) => Err("renamed imports are unavailable"),
        UseTree::Glob(_) => Err("glob imports are unavailable"),
    }
}

fn allowed_import(
    parts: &[String],
    mode: SourceMode,
    relation: &PublicConsumerRelation,
    names: &BTreeSet<String>,
    provider: Option<&ProvenProvider<'_>>,
) -> bool {
    match mode {
        SourceMode::Shipping(PairProfile::Foundation) => {
            parts == ["core", "fmt"]
                || parts.first().is_some_and(|part| part == "crate")
        }
        SourceMode::Consumer(PairProfile::Foundation) => {
            parts == ["core", "fmt", "Debug"]
        }
        SourceMode::Shipping(PairProfile::KernelInterface) => {
            parts == ["core", "fmt"]
                || local_import(parts, names)
                || provider.is_some_and(|facts| {
                    public_import(parts, FOUNDATION_CRATE, facts.relation)
                })
        }
        SourceMode::Consumer(PairProfile::KernelInterface) => {
            matches!(
                parts,
                [root, module, name]
                    if root == "core"
                        && ((module == "fmt" && name == "Debug")
                            || (module == "error" && name == "Error"))
            ) || local_import(parts, names)
                || public_import(parts, KERNEL_INTERFACE_CRATE, relation)
        }
    }
}

fn local_import(parts: &[String], names: &BTreeSet<String>) -> bool {
    parts.first().is_some_and(|part| {
        matches!(part.as_str(), "crate" | "self" | "super")
            || names.contains(part)
    })
}

fn public_import(
    parts: &[String],
    public_crate: &str,
    relation: &PublicConsumerRelation,
) -> bool {
    if parts.first().map(String::as_str) != Some(public_crate) {
        return false;
    }
    let path = parts[1..].join("::");
    relation.public_items.iter().any(|item| item.path == path)
}

fn declared_names(sources: &SourceSet) -> BTreeSet<String> {
    let mut visitor = NameCollector::default();
    for source in sources.files.values() {
        visitor.visit_file(&source.syntax);
    }
    visitor.names
}

#[derive(Default)]
struct NameCollector {
    names: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for NameCollector {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        let mut imports = Vec::new();
        if flatten_use(&item.tree, Vec::new(), &mut imports).is_ok() {
            self.names.extend(
                imports.into_iter().filter_map(|parts| parts.last().cloned()),
            );
        }
        visit::visit_item_use(self, item);
    }

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        self.names.insert(item.ident.to_string());
        visit::visit_item_struct(self, item);
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        self.names.insert(item.ident.to_string());
        visit::visit_item_mod(self, item);
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        self.names.insert(item.ident.to_string());
        visit::visit_item_enum(self, item);
    }

    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        self.names.insert(item.ident.to_string());
        visit::visit_item_trait(self, item);
    }

    fn visit_generics(&mut self, generics: &'ast syn::Generics) {
        self.names.extend(generics.params.iter().filter_map(generic_type_name));
        visit::visit_generics(self, generics);
    }
}

fn generic_type_name(parameter: &syn::GenericParam) -> Option<String> {
    let syn::GenericParam::Type(parameter) = parameter else { return None };
    Some(parameter.ident.to_string())
}

fn registered_public_types(
    relation: &PublicConsumerRelation,
) -> BTreeSet<String> {
    relation
        .public_items
        .iter()
        .filter(|item| item.kind == PublicItemKind::Type)
        .map(|item| item.path.clone())
        .collect()
}

fn registered_public_traits(
    relation: &PublicConsumerRelation,
) -> BTreeSet<String> {
    relation
        .public_items
        .iter()
        .filter(|item| item.kind == PublicItemKind::Trait)
        .map(|item| item.path.clone())
        .collect()
}

fn registered_public_trait_members(
    relation: &PublicConsumerRelation,
    public_traits: &BTreeSet<String>,
) -> BTreeMap<String, BTreeSet<String>> {
    public_traits
        .iter()
        .map(|public_trait| {
            let prefix = format!("{public_trait}::");
            let members = relation
                .public_items
                .iter()
                .filter(|item| item.kind == PublicItemKind::Function)
                .filter_map(|item| item.path.strip_prefix(&prefix))
                .filter(|member| !member.contains("::"))
                .map(str::to_owned)
                .collect();
            (public_trait.clone(), members)
        })
        .collect()
}

fn validate_type_paths(
    syntax: &File,
    path: &Path,
    mode: SourceMode,
    names: &BTreeSet<String>,
    authority: TypePathAuthority<'_>,
    provider: Option<&ProvenProvider<'_>>,
) -> Result<(), ProductSourceError> {
    let mut visitor = TypePathGuard {
        path,
        mode,
        names,
        authority,
        provider,
        witness_type_depth: 0,
        witness_helper_depth: 0,
        error: None,
    };
    visitor.visit_file(syntax);
    visitor.error.map_or(Ok(()), Err)
}

#[derive(Clone, Copy)]
struct TypePathAuthority<'a> {
    public_types: &'a BTreeSet<String>,
    public_traits: &'a BTreeSet<String>,
    item_witness: bool,
}

struct TypePathGuard<'a> {
    path: &'a Path,
    mode: SourceMode,
    names: &'a BTreeSet<String>,
    authority: TypePathAuthority<'a>,
    provider: Option<&'a ProvenProvider<'a>>,
    witness_type_depth: usize,
    witness_helper_depth: usize,
    error: Option<ProductSourceError>,
}

impl<'ast> Visit<'ast> for TypePathGuard<'_> {
    fn visit_type_path(&mut self, ty: &'ast syn::TypePath) {
        if self.error.is_some() {
            return;
        }
        if !allowed_type_path(
            ty,
            self.mode,
            self.names,
            self.authority.public_types,
            self.provider,
            self.witness_type_depth > 0 || self.authority.item_witness,
        ) {
            self.error = Some(ProductSourceError::at(
                "PRODUCT_TYPE_PATH_UNAVAILABLE",
                self.path,
                "type path is outside the closed Product profile",
            ));
            return;
        }
        visit::visit_type_path(self, ty);
    }

    fn visit_trait_bound(&mut self, bound: &'ast syn::TraitBound) {
        if self.error.is_some() {
            return;
        }
        if !allowed_trait_bound(
            bound,
            self.mode,
            self.names,
            self.authority.public_traits,
            self.authority.item_witness,
            self.witness_helper_depth > 0,
        ) {
            self.error = Some(ProductSourceError::at(
                "PRODUCT_TRAIT_BOUND_UNAVAILABLE",
                self.path,
                "trait bound is outside the closed Product profile",
            ));
            return;
        }
        visit::visit_trait_bound(self, bound);
    }

    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        let witness = is_witness_call(call);
        self.witness_type_depth =
            self.witness_type_depth.saturating_add(usize::from(witness));
        visit::visit_expr_call(self, call);
        self.witness_type_depth =
            self.witness_type_depth.saturating_sub(usize::from(witness));
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let helper = witness_helper_name(&item.sig.ident.to_string()).is_some();
        self.witness_helper_depth =
            self.witness_helper_depth.saturating_add(usize::from(helper));
        visit::visit_item_fn(self, item);
        self.witness_helper_depth =
            self.witness_helper_depth.saturating_sub(usize::from(helper));
    }
}

fn is_witness_call(call: &ExprCall) -> bool {
    let syn::Expr::Path(function) = call.func.as_ref() else { return false };
    function.path.segments.last().is_some_and(|segment| {
        witness_helper_name(&segment.ident.to_string()).is_some()
    })
}

fn allowed_type_path(
    ty: &syn::TypePath,
    mode: SourceMode,
    names: &BTreeSet<String>,
    public_types: &BTreeSet<String>,
    provider: Option<&ProvenProvider<'_>>,
    witness_type: bool,
) -> bool {
    if ty.qself.is_some() || !type_path_arguments_supported(&ty.path) {
        return false;
    }
    let parts = ty
        .path
        .segments
        .iter()
        .map(|part| part.ident.to_string())
        .collect::<Vec<_>>();
    let Some(first) = parts.first().map(String::as_str) else { return false };
    if ty.path.leading_colon.is_some() {
        return witness_type
            && matches!(mode, SourceMode::Consumer(_))
            && allowed_external_type(
                &parts,
                mode.profile().spec().public_crate,
                public_types,
            );
    }
    if simple_type_root(first) || names.contains(first) {
        return true;
    }
    match mode {
        SourceMode::Shipping(PairProfile::Foundation) => {
            allowed_shipping_type(&parts)
        }
        SourceMode::Consumer(PairProfile::Foundation) => {
            allowed_external_type(&parts, FOUNDATION_CRATE, public_types)
        }
        SourceMode::Shipping(PairProfile::KernelInterface) => {
            allowed_shipping_type(&parts)
                || provider.is_some_and(|facts| {
                    allowed_observed_external_type(
                        &parts,
                        FOUNDATION_CRATE,
                        facts.observation,
                    )
                })
        }
        SourceMode::Consumer(PairProfile::KernelInterface) => {
            allowed_external_type(&parts, KERNEL_INTERFACE_CRATE, public_types)
        }
    }
}

fn type_path_arguments_supported(path: &SynPath) -> bool {
    path.segments.iter().enumerate().all(|(index, segment)| {
        if index + 1 != path.segments.len() {
            return matches!(segment.arguments, PathArguments::None);
        }
        match &segment.arguments {
            PathArguments::None => true,
            PathArguments::AngleBracketed(arguments) => {
                arguments.args.iter().all(supported_type_argument)
            }
            PathArguments::Parenthesized(_) => false,
        }
    })
}

const fn supported_type_argument(argument: &GenericArgument) -> bool {
    matches!(argument, GenericArgument::Lifetime(_) | GenericArgument::Type(_))
}

fn simple_type_root(first: &str) -> bool {
    matches!(
        first,
        "Self"
            | "Result"
            | "Option"
            | "bool"
            | "char"
            | "str"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
    )
}

fn allowed_shipping_type(parts: &[String]) -> bool {
    parts
        .first()
        .is_some_and(|part| matches!(part.as_str(), "crate" | "self" | "super"))
        || parts == ["fmt", "Formatter"]
        || parts == ["fmt", "Result"]
        || parts == ["std", "error", "Error"]
}

fn allowed_external_type(
    parts: &[String],
    public_crate: &str,
    public_types: &BTreeSet<String>,
) -> bool {
    if parts == ["Debug"] {
        return true;
    }
    if parts.first().is_none_or(|part| part != public_crate) {
        return false;
    }
    let public_path = parts[1..].join("::");
    public_types.contains(&public_path)
}

fn allowed_observed_external_type(
    parts: &[String],
    public_crate: &str,
    observation: &ProductSourceObservation,
) -> bool {
    if parts.first().map(String::as_str) != Some(public_crate) {
        return false;
    }
    let path = parts[1..].join("::");
    observation
        .public_items
        .iter()
        .any(|item| item.kind == PublicItemKind::Type && item.path == path)
}

fn allowed_trait_bound(
    bound: &syn::TraitBound,
    mode: SourceMode,
    names: &BTreeSet<String>,
    public_traits: &BTreeSet<String>,
    item_witness: bool,
    helper_bound: bool,
) -> bool {
    if helper_bound {
        return exact_builtin_trait_bound(bound).is_some();
    }
    if item_witness
        && matches!(mode, SourceMode::Consumer(_))
        && exact_public_trait_path(bound, mode.profile(), public_traits)
            .is_some()
    {
        return true;
    }
    if bound.lifetimes.is_some()
        || bound.path.leading_colon.is_some()
        || bound.modifiers.require_empty().is_err()
        || bound.path.segments.len() != 1
    {
        return false;
    }
    let segment = &bound.path.segments[0];
    let name = segment.ident.to_string();
    if bound.maybe.is_some() {
        return name == "Sized"
            && matches!(segment.arguments, PathArguments::None);
    }
    if name == "Iterator" {
        return exact_iterator_arguments(&segment.arguments);
    }
    matches!(name.as_str(), "Clone" | "Copy" | "Debug" | "PartialEq" | "Eq")
        || (names.contains(&name)
            && matches!(segment.arguments, PathArguments::None))
}

fn exact_builtin_trait_bound(bound: &syn::TraitBound) -> Option<&'static str> {
    if bound.lifetimes.is_some()
        || bound.path.leading_colon.is_none()
        || bound.modifiers.require_empty().is_err()
        || bound.maybe.is_some()
        || bound.path.segments.len() != 3
        || bound
            .path
            .segments
            .iter()
            .any(|segment| !matches!(segment.arguments, PathArguments::None))
    {
        return None;
    }
    let parts = bound
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    match parts.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["core", "clone", "Clone"] => Some("Clone"),
        ["core", "marker", "Copy"] => Some("Copy"),
        ["core", "fmt", "Debug"] => Some("Debug"),
        ["core", "cmp", "PartialEq"] => Some("PartialEq"),
        ["core", "cmp", "Eq"] => Some("Eq"),
        _ => None,
    }
}

fn exact_public_trait_path(
    bound: &syn::TraitBound,
    profile: PairProfile,
    public_traits: &BTreeSet<String>,
) -> Option<String> {
    if bound.lifetimes.is_some()
        || bound.path.leading_colon.is_none()
        || bound.modifiers.require_empty().is_err()
        || bound.maybe.is_some()
        || bound.path.segments.len() != 3
        || bound
            .path
            .segments
            .iter()
            .any(|segment| !matches!(segment.arguments, PathArguments::None))
    {
        return None;
    }
    let parts = bound
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    if parts[0] != profile.spec().public_crate {
        return None;
    }
    let public_path = parts[1..].join("::");
    public_traits.contains(&public_path).then_some(public_path)
}

fn exact_iterator_arguments(arguments: &PathArguments) -> bool {
    let PathArguments::AngleBracketed(arguments) = arguments else {
        return false;
    };
    if arguments.args.len() != 1 {
        return false;
    }
    let Some(GenericArgument::AssocType(binding)) = arguments.args.first()
    else {
        return false;
    };
    binding.ident == "Item"
        && binding.generics.is_none()
        && matches!(binding.ty, Type::Reference(_))
}

struct Surface {
    public_items: BTreeSet<PublicItem>,
    derive_checks: BTreeSet<DeriveCheck>,
    reexports: BTreeSet<PublicReexport>,
    public_types: BTreeSet<String>,
    local_types: BTreeSet<String>,
    impls: Vec<ImplCandidate>,
}

impl Surface {
    const fn new() -> Self {
        Self {
            public_items: BTreeSet::new(),
            derive_checks: BTreeSet::new(),
            reexports: BTreeSet::new(),
            public_types: BTreeSet::new(),
            local_types: BTreeSet::new(),
            impls: Vec::new(),
        }
    }
}

struct ImplCandidate {
    owner: String,
    functions: Vec<String>,
    consts: Vec<String>,
}

fn analyze_shipping(
    sources: &SourceSet,
    closure: &Closure,
    relation: &PublicConsumerRelation,
    profile: PairProfile,
    provider: Option<&ProvenProvider<'_>>,
) -> Result<Surface, ProductSourceError> {
    validate_provider(profile, provider)?;
    validate_source_profile(
        sources,
        closure,
        relation,
        SourceMode::Shipping(profile),
        provider,
    )?;
    let mut surface = Surface::new();
    let mut visited = BTreeSet::new();
    let mut walk =
        ShippingWalk { visited: &mut visited, surface: &mut surface, provider };
    walk_shipping_source(
        sources,
        &closure.target.src_path,
        &[],
        true,
        true,
        &mut walk,
    )?;
    validate_visited_sources(sources, &visited)?;
    finish_inherent_items(&mut surface)?;
    Ok(surface)
}

struct ShippingWalk<'state, 'provider, 'facts> {
    visited: &'state mut BTreeSet<PathBuf>,
    surface: &'state mut Surface,
    provider: Option<&'provider ProvenProvider<'facts>>,
}

fn validate_provider(
    profile: PairProfile,
    provider: Option<&ProvenProvider<'_>>,
) -> Result<(), ProductSourceError> {
    match (profile, provider) {
        (PairProfile::Foundation, None) => Ok(()),
        (PairProfile::KernelInterface, Some(provider)) => {
            validate_foundation_provider(provider)
        }
        _ => Err(ProductSourceError::new(
            "PRODUCT_SOURCE_REEXPORT_ORIGIN",
            "source pair has the wrong proven provider",
        )),
    }
}

fn validate_foundation_provider(
    provider: &ProvenProvider<'_>,
) -> Result<(), ProductSourceError> {
    let relation = provider.relation;
    let observation = provider.observation;
    let items =
        observation.public_items.iter().cloned().collect::<BTreeSet<_>>();
    let expected_items =
        relation.public_items.iter().cloned().collect::<BTreeSet<_>>();
    let derives =
        observation.derive_checks.iter().cloned().collect::<BTreeSet<_>>();
    let expected_derives =
        relation.derive_checks.iter().cloned().collect::<BTreeSet<_>>();
    if relation.id != FOUNDATION_PROFILE.relation
        || observation.relation != relation.id
        || !relation.reexports.is_empty()
        || items != expected_items
        || derives != expected_derives
    {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_REEXPORT_ORIGIN",
            "KI aliases require the exact proven Foundation observation",
        ));
    }
    Ok(())
}

fn walk_shipping_source(
    sources: &SourceSet,
    path: &Path,
    prefix: &[String],
    exposed: bool,
    target_root: bool,
    walk: &mut ShippingWalk<'_, '_, '_>,
) -> Result<(), ProductSourceError> {
    if !walk.visited.insert(path.to_path_buf()) {
        return Err(ProductSourceError::at(
            "PRODUCT_COMPILER_SOURCE_SET",
            path,
            "compiler source is reached by more than one module path",
        ));
    }
    let syntax = sources.syntax(path)?;
    let base = module_base(path, target_root);
    walk_shipping_items(sources, &syntax.items, prefix, exposed, &base, walk)
}

fn walk_shipping_items(
    sources: &SourceSet,
    items: &[Item],
    prefix: &[String],
    exposed: bool,
    base: &Path,
    walk: &mut ShippingWalk<'_, '_, '_>,
) -> Result<(), ProductSourceError> {
    for item in items {
        if let Item::Mod(module) = item {
            walk_shipping_module(sources, module, prefix, exposed, base, walk)?;
            continue;
        }
        collect_surface_item(
            item,
            prefix,
            exposed,
            walk.surface,
            walk.provider,
        )?;
    }
    Ok(())
}

fn walk_shipping_module(
    sources: &SourceSet,
    module: &syn::ItemMod,
    prefix: &[String],
    exposed: bool,
    base: &Path,
    walk: &mut ShippingWalk<'_, '_, '_>,
) -> Result<(), ProductSourceError> {
    let name = module.ident.to_string();
    let module_prefix = append_path(prefix, &name);
    let module_exposed = exposed && public(&module.vis);
    if module_exposed {
        let module_path = qualified(&module_prefix);
        insert_public_item(walk.surface, &module_path, PublicItemKind::Module)?;
    }
    if let Some((_, items)) = &module.content {
        return walk_shipping_items(
            sources,
            items,
            &module_prefix,
            module_exposed,
            &base.join(name),
            walk,
        );
    }
    let source = resolve_external_module(sources, base, &name)?;
    walk_shipping_source(
        sources,
        &source,
        &module_prefix,
        module_exposed,
        false,
        walk,
    )
}

fn collect_surface_item(
    item: &Item,
    prefix: &[String],
    exposed: bool,
    surface: &mut Surface,
    provider: Option<&ProvenProvider<'_>>,
) -> Result<(), ProductSourceError> {
    match item {
        Item::Struct(item) => {
            reject_public_struct_fields(item, exposed)?;
            collect_public_type(
                &item.ident.to_string(),
                &item.attrs,
                &item.vis,
                prefix,
                exposed,
                surface,
            )
        }
        Item::Enum(item) => collect_public_enum(item, prefix, exposed, surface),
        Item::Trait(item) => {
            collect_public_trait(item, prefix, exposed, surface)
        }
        Item::Fn(item) => collect_named_public_item(
            &item.sig.ident.to_string(),
            &item.vis,
            prefix,
            exposed,
            PublicItemKind::Function,
            surface,
        ),
        Item::Const(item) => collect_named_public_item(
            &item.ident.to_string(),
            &item.vis,
            prefix,
            exposed,
            PublicItemKind::Const,
            surface,
        ),
        Item::Static(item) => collect_named_public_item(
            &item.ident.to_string(),
            &item.vis,
            prefix,
            exposed,
            PublicItemKind::Static,
            surface,
        ),
        Item::Impl(item) => collect_impl(item, prefix, surface),
        Item::Use(item) if public(&item.vis) && exposed => {
            collect_reexports(item, prefix, surface, provider)
        }
        Item::Use(item) if public(&item.vis) => Err(ProductSourceError::new(
            "PRODUCT_SOURCE_REEXPORT_SET",
            "public re-export is nested below a private module",
        )),
        _ => Ok(()),
    }
}

fn reject_public_struct_fields(
    item: &syn::ItemStruct,
    exposed: bool,
) -> Result<(), ProductSourceError> {
    let public_type = exposed && public(&item.vis);
    let public_field = item.fields.iter().any(|field| public(&field.vis));
    if !public_type || !public_field {
        return Ok(());
    }
    Err(ProductSourceError::new(
        "PRODUCT_PUBLIC_FIELD_UNAVAILABLE",
        format!("public struct {} exposes a field", item.ident),
    ))
}

fn collect_public_enum(
    item: &syn::ItemEnum,
    prefix: &[String],
    exposed: bool,
    surface: &mut Surface,
) -> Result<(), ProductSourceError> {
    collect_public_type(
        &item.ident.to_string(),
        &item.attrs,
        &item.vis,
        prefix,
        exposed,
        surface,
    )?;
    if !exposed || !public(&item.vis) {
        return Ok(());
    }
    let enum_path = qualified(&append_path(prefix, &item.ident.to_string()));
    for variant in &item.variants {
        let path = format!("{enum_path}::{}", variant.ident);
        insert_public_item(surface, &path, PublicItemKind::Variant)?;
    }
    Ok(())
}

fn collect_reexports(
    item: &syn::ItemUse,
    prefix: &[String],
    surface: &mut Surface,
    provider: Option<&ProvenProvider<'_>>,
) -> Result<(), ProductSourceError> {
    let provider = provider.ok_or_else(|| {
        ProductSourceError::new(
            "PRODUCT_SOURCE_REEXPORT_ORIGIN",
            "public re-export has no proven provider",
        )
    })?;
    let (module, names) = exact_provider_reexport(item)?;
    for name in names {
        collect_reexport(&module, &name, prefix, surface, provider)?;
    }
    Ok(())
}

fn exact_provider_reexport(
    item: &syn::ItemUse,
) -> Result<(String, Vec<String>), ProductSourceError> {
    let Some(_) = item.leading_colon else {
        return Err(reexport_shape_error());
    };
    let UseTree::Path(root) = &item.tree else {
        return Err(reexport_shape_error());
    };
    let UseTree::Path(module) = root.tree.as_ref() else {
        return Err(reexport_shape_error());
    };
    if root.ident != FOUNDATION_CRATE {
        return Err(reexport_shape_error());
    }
    let names = match module.tree.as_ref() {
        UseTree::Name(name) => vec![name.ident.to_string()],
        UseTree::Group(group) if !group.items.is_empty() => group
            .items
            .iter()
            .map(|tree| match tree {
                UseTree::Name(name) => Ok(name.ident.to_string()),
                _ => Err(reexport_shape_error()),
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(reexport_shape_error()),
    };
    Ok((module.ident.to_string(), names))
}

fn reexport_shape_error() -> ProductSourceError {
    ProductSourceError::new(
        "PRODUCT_SOURCE_REEXPORT_ORIGIN",
        "public re-export is not an exact Foundation type path",
    )
}

fn collect_reexport(
    module: &str,
    name: &str,
    prefix: &[String],
    surface: &mut Surface,
    provider: &ProvenProvider<'_>,
) -> Result<(), ProductSourceError> {
    let provider_type = format!("{module}::{name}");
    let provider_item =
        PublicItem { path: provider_type.clone(), kind: PublicItemKind::Type };
    if !provider.observation.public_items.contains(&provider_item) {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_REEXPORT_ORIGIN",
            format!("Foundation does not prove public type {provider_type}"),
        ));
    }
    let public_type = qualified(&append_path(prefix, name));
    let reexport = PublicReexport {
        public_type: public_type.clone(),
        provider_relation: provider.relation.id.clone(),
        provider_type: provider_type.clone(),
    };
    if !surface.reexports.insert(reexport) {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_REEXPORT_SET",
            format!("public alias {public_type} is repeated"),
        ));
    }
    insert_public_item(surface, &public_type, PublicItemKind::Type)?;
    surface.public_types.insert(public_type.clone());
    collect_reexported_variants(
        surface,
        provider.observation,
        &provider_type,
        &public_type,
    )?;
    for check in provider
        .observation
        .derive_checks
        .iter()
        .filter(|check| check.public_type == provider_type)
    {
        let mut alias_check = check.clone();
        alias_check.public_type.clone_from(&public_type);
        if !surface.derive_checks.insert(alias_check) {
            return Err(ProductSourceError::new(
                "PRODUCT_DERIVE_SET",
                format!("public alias {public_type} repeats one derive"),
            ));
        }
    }
    Ok(())
}

fn collect_reexported_variants(
    surface: &mut Surface,
    observation: &ProductSourceObservation,
    provider_type: &str,
    public_type: &str,
) -> Result<(), ProductSourceError> {
    let prefix = format!("{provider_type}::");
    for item in &observation.public_items {
        if item.kind != PublicItemKind::Variant {
            continue;
        }
        let Some(variant) = item.path.strip_prefix(&prefix) else { continue };
        if variant.contains("::") {
            return Err(ProductSourceError::new(
                "PRODUCT_SOURCE_REEXPORT_ORIGIN",
                "provider variant path is not direct",
            ));
        }
        let path = format!("{public_type}::{variant}");
        insert_public_item(surface, &path, PublicItemKind::Variant)?;
    }
    Ok(())
}

fn collect_public_type(
    name: &str,
    attributes: &[Attribute],
    visibility: &Visibility,
    prefix: &[String],
    exposed: bool,
    surface: &mut Surface,
) -> Result<(), ProductSourceError> {
    let path = qualified(&append_path(prefix, name));
    surface.local_types.insert(path.clone());
    let derives = parse_derives(attributes, &path)?;
    let public_type = exposed && public(visibility);
    if !public_type {
        return Ok(());
    }
    insert_public_item(surface, &path, PublicItemKind::Type)?;
    surface.public_types.insert(path.clone());
    for derive in derives {
        let check = derive_check(path.clone(), derive);
        if !surface.derive_checks.insert(check) {
            return Err(ProductSourceError::new(
                "PRODUCT_DERIVE_SET",
                format!("type {path} repeats one derive"),
            ));
        }
    }
    Ok(())
}

fn collect_public_trait(
    item: &syn::ItemTrait,
    prefix: &[String],
    exposed: bool,
    surface: &mut Surface,
) -> Result<(), ProductSourceError> {
    let path = qualified(&append_path(prefix, &item.ident.to_string()));
    surface.local_types.insert(path.clone());
    if !exposed || !public(&item.vis) {
        return Ok(());
    }
    let members = public_trait_members(item, &path)?;
    insert_public_item(surface, &path, PublicItemKind::Trait)?;
    for member in members {
        let member_path = format!("{path}::{member}");
        insert_public_item(surface, &member_path, PublicItemKind::Function)?;
    }
    Ok(())
}

fn public_trait_members(
    item: &syn::ItemTrait,
    path: &str,
) -> Result<BTreeSet<String>, ProductSourceError> {
    if !item.generics.params.is_empty()
        || item.generics.where_clause.is_some()
        || !item.supertraits.is_empty()
    {
        return Err(public_trait_member_error(path));
    }
    let mut observed = BTreeSet::new();
    for member in &item.items {
        let syn::TraitItem::Fn(function) = member else {
            return Err(public_trait_member_error(path));
        };
        if function.default.is_some()
            || !observed.insert(function.sig.ident.to_string())
        {
            return Err(public_trait_member_error(path));
        }
    }
    Ok(observed)
}

fn public_trait_member_error(path: &str) -> ProductSourceError {
    ProductSourceError::new(
        "PRODUCT_PUBLIC_TRAIT_MEMBER_UNAVAILABLE",
        format!("public trait {path} differs from its required methods"),
    )
}

fn collect_named_public_item(
    name: &str,
    visibility: &Visibility,
    prefix: &[String],
    exposed: bool,
    kind: PublicItemKind,
    surface: &mut Surface,
) -> Result<(), ProductSourceError> {
    if exposed && public(visibility) {
        let path = qualified(&append_path(prefix, name));
        insert_public_item(surface, &path, kind)?;
    }
    Ok(())
}

fn insert_public_item(
    surface: &mut Surface,
    path: &str,
    kind: PublicItemKind,
) -> Result<(), ProductSourceError> {
    let item = PublicItem { path: path.to_owned(), kind };
    if !surface.public_items.insert(item) {
        return Err(ProductSourceError::new(
            "PRODUCT_PUBLIC_ITEM_SET",
            format!("public item {path} is declared more than once"),
        ));
    }
    Ok(())
}

fn parse_derives(
    attributes: &[Attribute],
    public_path: &str,
) -> Result<Vec<BuiltinDerive>, ProductSourceError> {
    let mut output = Vec::new();
    for attribute in attributes {
        if !attribute.path().is_ident("derive") {
            continue;
        }
        let Meta::List(list) = &attribute.meta else {
            return Err(ProductSourceError::new(
                "PRODUCT_DERIVE_UNAVAILABLE",
                format!("type {public_path} has an invalid derive"),
            ));
        };
        let parser = Punctuated::<SynPath, Token![,]>::parse_terminated;
        let paths = parser.parse2(list.tokens.clone()).map_err(|error| {
            ProductSourceError::new(
                "PRODUCT_DERIVE_UNAVAILABLE",
                error.to_string(),
            )
        })?;
        for path in paths {
            output.push(parse_builtin_derive(&path, public_path)?);
        }
    }
    Ok(output)
}

fn parse_builtin_derive(
    path: &SynPath,
    public_path: &str,
) -> Result<BuiltinDerive, ProductSourceError> {
    if path.leading_colon.is_some() || path.segments.len() != 1 {
        return Err(ProductSourceError::new(
            "PRODUCT_DERIVE_UNAVAILABLE",
            format!("type {public_path} uses a qualified derive"),
        ));
    }
    let name = path.segments[0].ident.to_string();
    let derive = match name.as_str() {
        "Clone" => BuiltinDerive::Clone,
        "Copy" => BuiltinDerive::Copy,
        "Debug" => BuiltinDerive::Debug,
        "PartialEq" => BuiltinDerive::PartialEq,
        "Eq" => BuiltinDerive::Eq,
        "Hash" => BuiltinDerive::Hash,
        "Default" => BuiltinDerive::Default,
        _ => {
            return Err(ProductSourceError::new(
                "PRODUCT_DERIVE_UNAVAILABLE",
                format!("type {public_path} uses unknown derive {name}"),
            ));
        }
    };
    Ok(derive)
}

const fn derive_check(
    public_type: String,
    derive: BuiltinDerive,
) -> DeriveCheck {
    let consumer_operation = match derive {
        BuiltinDerive::Clone => ConsumerOperation::Clone,
        BuiltinDerive::Copy => ConsumerOperation::Copy,
        BuiltinDerive::Debug => ConsumerOperation::Debug,
        BuiltinDerive::PartialEq => ConsumerOperation::PartialEq,
        BuiltinDerive::Eq => ConsumerOperation::Eq,
        BuiltinDerive::Hash => ConsumerOperation::Hash,
        BuiltinDerive::Default => ConsumerOperation::Default,
    };
    DeriveCheck { derive, public_type, consumer_operation }
}

fn collect_impl(
    item: &syn::ItemImpl,
    prefix: &[String],
    surface: &mut Surface,
) -> Result<(), ProductSourceError> {
    if item.trait_.is_some() {
        return Ok(());
    }
    let owner = inherent_owner(&item.self_ty, &item.generics, prefix)?;
    let mut functions = Vec::new();
    let mut consts = Vec::new();
    for member in &item.items {
        match member {
            ImplItem::Fn(function) if public(&function.vis) => {
                functions.push(function.sig.ident.to_string());
            }
            ImplItem::Const(value) if public(&value.vis) => {
                consts.push(value.ident.to_string());
            }
            ImplItem::Type(value) if public(&value.vis) => {
                return Err(ProductSourceError::new(
                    "PRODUCT_INHERENT_ITEM_UNAVAILABLE",
                    "public inherent associated type is unavailable",
                ));
            }
            _ => {}
        }
    }
    surface.impls.push(ImplCandidate { owner, functions, consts });
    Ok(())
}

fn inherent_owner(
    ty: &Type,
    generics: &syn::Generics,
    prefix: &[String],
) -> Result<String, ProductSourceError> {
    let Type::Path(path) = ty else {
        return Err(ProductSourceError::new(
            "PRODUCT_INHERENT_IMPL_UNAVAILABLE",
            "inherent impl target is not a direct path",
        ));
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() {
        return Err(ProductSourceError::new(
            "PRODUCT_INHERENT_IMPL_UNAVAILABLE",
            "inherent impl target requires name resolution",
        ));
    }
    let lifetime = exact_impl_lifetime(generics)?;
    let segment_count = path.path.segments.len();
    let mut parts = Vec::with_capacity(segment_count);
    for (index, part) in path.path.segments.iter().enumerate() {
        let final_segment = index + 1 == segment_count;
        if !exact_impl_segment_arguments(
            &part.arguments,
            final_segment,
            lifetime.as_deref(),
        ) {
            return Err(ProductSourceError::new(
                "PRODUCT_INHERENT_IMPL_UNAVAILABLE",
                "inherent impl has non-exact lifetime arguments",
            ));
        }
        parts.push(part.ident.to_string());
    }
    if parts.first().is_some_and(|part| part == "crate") {
        parts.remove(0);
        return Ok(qualified(&parts));
    }
    if parts.first().is_some_and(|part| part == "self") {
        parts.remove(0);
    } else if parts.len() != 1 {
        return Err(ProductSourceError::new(
            "PRODUCT_INHERENT_IMPL_UNAVAILABLE",
            "inherent impl target is not a direct own type",
        ));
    }
    let mut owner = prefix.to_vec();
    owner.extend(parts);
    Ok(qualified(&owner))
}

fn exact_impl_lifetime(
    generics: &syn::Generics,
) -> Result<Option<String>, ProductSourceError> {
    if generics.where_clause.is_some() {
        return Err(ProductSourceError::new(
            "PRODUCT_INHERENT_IMPL_UNAVAILABLE",
            "inherent impl cannot have a where clause",
        ));
    }
    if generics.params.is_empty() {
        return Ok(None);
    }
    let parameters = generics.params.iter().collect::<Vec<_>>();
    let [syn::GenericParam::Lifetime(parameter)] = parameters.as_slice() else {
        return Err(ProductSourceError::new(
            "PRODUCT_INHERENT_IMPL_UNAVAILABLE",
            "inherent impl supports one lifetime binder only",
        ));
    };
    if !parameter.attrs.is_empty() || !parameter.bounds.is_empty() {
        return Err(ProductSourceError::new(
            "PRODUCT_INHERENT_IMPL_UNAVAILABLE",
            "inherent impl lifetime binder must be unbounded",
        ));
    }
    Ok(Some(parameter.lifetime.to_string()))
}

fn exact_impl_segment_arguments(
    arguments: &PathArguments,
    final_segment: bool,
    lifetime: Option<&str>,
) -> bool {
    if !final_segment {
        return matches!(arguments, PathArguments::None);
    }
    match (lifetime, arguments) {
        (None, PathArguments::None) => true,
        (Some(expected), PathArguments::AngleBracketed(arguments))
            if arguments.args.len() == 1 =>
        {
            matches!(
                arguments.args.first(),
                Some(GenericArgument::Lifetime(actual))
                    if actual.to_string() == expected
            )
        }
        _ => false,
    }
}

fn finish_inherent_items(
    surface: &mut Surface,
) -> Result<(), ProductSourceError> {
    for implementation in std::mem::take(&mut surface.impls) {
        if !surface.local_types.contains(&implementation.owner) {
            return Err(ProductSourceError::new(
                "PRODUCT_INHERENT_IMPL_UNAVAILABLE",
                format!(
                    "impl target {} is not a direct own type",
                    implementation.owner
                ),
            ));
        }
        if !surface.public_types.contains(&implementation.owner) {
            continue;
        }
        insert_impl_items(surface, implementation)?;
    }
    Ok(())
}

fn insert_impl_items(
    surface: &mut Surface,
    implementation: ImplCandidate,
) -> Result<(), ProductSourceError> {
    for name in implementation.functions {
        let path = format!("{}::{name}", implementation.owner);
        insert_public_item(surface, &path, PublicItemKind::Function)?;
    }
    for name in implementation.consts {
        let path = format!("{}::{name}", implementation.owner);
        insert_public_item(surface, &path, PublicItemKind::Const)?;
    }
    Ok(())
}

fn module_base(path: &Path, target_root: bool) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    if target_root || path.file_name().is_some_and(|name| name == "mod.rs") {
        return parent.to_path_buf();
    }
    parent.join(path.file_stem().unwrap_or_default())
}

fn resolve_external_module(
    sources: &SourceSet,
    base: &Path,
    name: &str,
) -> Result<PathBuf, ProductSourceError> {
    let flat = base.join(format!("{name}.rs"));
    let nested = base.join(name).join("mod.rs");
    let candidates = [flat, nested]
        .into_iter()
        .filter(|path| sources.files.contains_key(path))
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [path] => Ok(path.clone()),
        [] => Err(ProductSourceError::at(
            "PRODUCT_MODULE_SOURCE_MISSING",
            base,
            format!("module {name} has no compiler-selected source"),
        )),
        _ => Err(ProductSourceError::at(
            "PRODUCT_MODULE_SOURCE_AMBIGUOUS",
            base,
            format!("module {name} has two compiler-selected sources"),
        )),
    }
}

fn validate_visited_sources(
    sources: &SourceSet,
    visited: &BTreeSet<PathBuf>,
) -> Result<(), ProductSourceError> {
    let selected = sources.paths();
    if selected != *visited {
        return Err(ProductSourceError::new(
            "PRODUCT_COMPILER_SOURCE_SET",
            set_difference_message("compiler source", &selected, visited),
        ));
    }
    Ok(())
}

fn validate_surface(
    surface: &Surface,
    relation: &PublicConsumerRelation,
) -> Result<(), ProductSourceError> {
    let expected_items = relation.public_items.iter().cloned().collect();
    if surface.public_items != expected_items {
        return Err(ProductSourceError::new(
            "PRODUCT_PUBLIC_ITEM_SET",
            set_difference_message(
                "public item",
                &expected_items,
                &surface.public_items,
            ),
        ));
    }
    let expected_derives = relation.derive_checks.iter().cloned().collect();
    if surface.derive_checks != expected_derives {
        return Err(ProductSourceError::new(
            "PRODUCT_DERIVE_SET",
            set_difference_message(
                "derive",
                &expected_derives,
                &surface.derive_checks,
            ),
        ));
    }
    let expected_reexports = relation.reexports.iter().cloned().collect();
    if surface.reexports != expected_reexports {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_REEXPORT_SET",
            set_difference_message(
                "public re-export",
                &expected_reexports,
                &surface.reexports,
            ),
        ));
    }
    Ok(())
}

fn set_difference_message<T>(
    label: &str,
    expected: &BTreeSet<T>,
    observed: &BTreeSet<T>,
) -> String
where
    T: Ord + fmt::Debug,
{
    if let Some(missing) = expected.difference(observed).next() {
        return format!("{label} is missing: {missing:?}");
    }
    if let Some(extra) = observed.difference(expected).next() {
        return format!("unexpected {label}: {extra:?}");
    }
    format!("{label} sets differ")
}

fn append_path(prefix: &[String], name: &str) -> Vec<String> {
    let mut output = prefix.to_vec();
    output.push(name.to_owned());
    output
}

fn qualified(parts: &[String]) -> String {
    parts.join("::")
}

const fn public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

fn analyze_consumer(
    sources: &SourceSet,
    closure: &Closure,
    relation: &PublicConsumerRelation,
    profile: PairProfile,
) -> Result<BTreeSet<String>, ProductSourceError> {
    validate_source_profile(
        sources,
        closure,
        relation,
        SourceMode::Consumer(profile),
        None,
    )?;
    validate_module_tree(sources, &closure.target.src_path)?;
    let mut visitor = WitnessVisitor::new(profile, relation)?;
    for (path, source) in &sources.files {
        visitor.path = path;
        visitor.visit_file(&source.syntax);
    }
    let witness_types = visitor.finish()?;
    validate_public_item_witnesses(sources, relation, profile, &witness_types)?;
    validate_trait_impl_witnesses(sources, relation, profile)?;
    Ok(witness_types)
}

fn validate_trait_impl_witnesses(
    sources: &SourceSet,
    relation: &PublicConsumerRelation,
    profile: PairProfile,
) -> Result<(), ProductSourceError> {
    let public_traits = registered_public_traits(relation);
    let trait_members =
        registered_public_trait_members(relation, &public_traits);
    let mut visitor = TraitImplWitnessVisitor {
        profile,
        public_traits: &public_traits,
        trait_members: &trait_members,
        observed: BTreeSet::new(),
        path: Path::new(""),
        error: None,
    };
    for (path, source) in &sources.files {
        visitor.path = path;
        visitor.visit_file(&source.syntax);
    }
    visitor.finish()
}

struct TraitImplWitnessVisitor<'a> {
    profile: PairProfile,
    public_traits: &'a BTreeSet<String>,
    trait_members: &'a BTreeMap<String, BTreeSet<String>>,
    observed: BTreeSet<String>,
    path: &'a Path,
    error: Option<ProductSourceError>,
}

impl TraitImplWitnessVisitor<'_> {
    fn fail(&mut self, message: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(ProductSourceError::at(
                "PRODUCT_CONSUMER_TRAIT_IMPL_SET",
                self.path,
                message,
            ));
        }
    }

    fn observe(&mut self, item: &syn::ItemImpl, public_trait: &str) {
        let members = item
            .items
            .iter()
            .filter_map(|member| match member {
                ImplItem::Fn(function) => Some(function.sig.ident.to_string()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let functions_only = members.len() == item.items.len();
        let expected = self.trait_members.get(public_trait);
        if !functions_only || expected != Some(&members) {
            self.fail(format!(
                "trait impl {public_trait} has the wrong required methods"
            ));
            return;
        }
        if !self.observed.insert(public_trait.to_owned()) {
            self.fail(format!("trait impl {public_trait} is repeated"));
        }
    }

    fn finish(self) -> Result<(), ProductSourceError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if self.observed == *self.public_traits {
            return Ok(());
        }
        Err(ProductSourceError::new(
            "PRODUCT_CONSUMER_TRAIT_IMPL_SET",
            set_difference_message(
                "public trait impl",
                self.public_traits,
                &self.observed,
            ),
        ))
    }
}

impl<'ast> Visit<'ast> for TraitImplWitnessVisitor<'_> {
    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if self.error.is_some() {
            return;
        }
        let Some((path, _)) = &item.trait_ else {
            visit::visit_item_impl(self, item);
            return;
        };
        let root = path.segments.first().map(|part| part.ident.to_string());
        if root.as_deref() != Some(self.profile.spec().public_crate) {
            visit::visit_item_impl(self, item);
            return;
        }
        let public_trait = exact_trait_impl_path(
            item.modifiers.require_empty().is_err(),
            path,
            self.profile,
            self.public_traits,
        );
        match public_trait {
            Some(public_trait) => self.observe(item, &public_trait),
            None => self.fail("public trait impl path is not exact"),
        }
        visit::visit_item_impl(self, item);
    }
}

fn exact_trait_impl_path(
    modified: bool,
    path: &SynPath,
    profile: PairProfile,
    public_traits: &BTreeSet<String>,
) -> Option<String> {
    if modified
        || path.leading_colon.is_none()
        || path.segments.len() != 3
        || path
            .segments
            .iter()
            .any(|segment| !matches!(segment.arguments, PathArguments::None))
    {
        return None;
    }
    let parts = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    if parts[0] != profile.spec().public_crate {
        return None;
    }
    let public_trait = parts[1..].join("::");
    public_traits.contains(&public_trait).then_some(public_trait)
}

fn validate_public_item_witnesses(
    sources: &SourceSet,
    relation: &PublicConsumerRelation,
    profile: PairProfile,
    witness_types: &BTreeSet<String>,
) -> Result<(), ProductSourceError> {
    let path = Path::new(profile.item_witness_source());
    let source = sources.files.get(path).ok_or_else(|| {
        ProductSourceError::at(
            "PRODUCT_CONSUMER_PUBLIC_ITEM_SET",
            path,
            "dedicated public-item witness source is absent",
        )
    })?;
    let mut visitor = PublicItemWitnessVisitor::new(profile, relation, path);
    visitor.visit_file(&source.syntax);
    visitor.finish(witness_types)
}

struct PublicItemWitnessVisitor<'a> {
    profile: PairProfile,
    expected: BTreeMap<String, PublicItemKind>,
    public_traits: BTreeSet<String>,
    observed: BTreeSet<PublicItem>,
    path: &'a Path,
    error: Option<ProductSourceError>,
}

impl<'a> PublicItemWitnessVisitor<'a> {
    fn new(
        profile: PairProfile,
        relation: &PublicConsumerRelation,
        path: &'a Path,
    ) -> Self {
        let expected = relation
            .public_items
            .iter()
            .map(|item| (item.path.clone(), item.kind))
            .collect();
        Self {
            profile,
            expected,
            public_traits: registered_public_traits(relation),
            observed: BTreeSet::new(),
            path,
            error: None,
        }
    }

    fn record(&mut self, public_path: &str, kind: PublicItemKind) {
        if self.error.is_some() {
            return;
        }
        if self.expected.get(public_path) != Some(&kind) {
            self.fail(format!(
                "external witness path is unregistered: {public_path}"
            ));
            return;
        }
        let item = PublicItem { path: public_path.to_owned(), kind };
        if !self.observed.insert(item) {
            self.fail(format!(
                "public item witness is repeated: {public_path}"
            ));
        }
    }

    fn fail(&mut self, message: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(ProductSourceError::at(
                "PRODUCT_CONSUMER_PUBLIC_ITEM_SET",
                self.path,
                message,
            ));
        }
    }

    fn observe_public_value(&mut self, public_path: &str) {
        let kind = self.expected.get(public_path).copied();
        if let Some(kind @ (PublicItemKind::Function | PublicItemKind::Const)) =
            kind
        {
            self.record(public_path, kind);
            return;
        }
        let message = "external value witness is not registered";
        self.fail(format!("{message}: {public_path}"));
    }

    fn finish(
        mut self,
        witness_types: &BTreeSet<String>,
    ) -> Result<(), ProductSourceError> {
        if let Some(error) = self.error.take() {
            return Err(error);
        }
        for public_type in witness_types {
            self.observed.insert(PublicItem {
                path: public_type.clone(),
                kind: PublicItemKind::Type,
            });
        }
        self.insert_module_witnesses();
        let expected = self
            .expected
            .into_iter()
            .map(|(path, kind)| PublicItem { path, kind })
            .collect();
        if self.observed == expected {
            return Ok(());
        }
        Err(ProductSourceError::at(
            "PRODUCT_CONSUMER_PUBLIC_ITEM_SET",
            self.path,
            set_difference_message(
                "public item witness",
                &expected,
                &self.observed,
            ),
        ))
    }

    fn insert_module_witnesses(&mut self) {
        let modules = self
            .observed
            .iter()
            .filter_map(|item| item.path.split_once("::"))
            .map(|(module, _)| module.to_owned())
            .filter(|module| {
                self.expected.get(module) == Some(&PublicItemKind::Module)
            })
            .collect::<BTreeSet<_>>();
        for module in modules {
            self.observed.insert(PublicItem {
                path: module,
                kind: PublicItemKind::Module,
            });
        }
    }
}

impl<'ast> Visit<'ast> for PublicItemWitnessVisitor<'_> {
    fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
        if self.error.is_some() {
            return;
        }
        match exact_public_value_path(expression, self.profile) {
            Ok(Some(public_path)) => {
                self.observe_public_value(&public_path);
            }
            Ok(None) => visit::visit_expr_path(self, expression),
            Err(message) => self.fail(message),
        }
    }

    fn visit_trait_bound(&mut self, bound: &'ast syn::TraitBound) {
        if self.error.is_some() {
            return;
        }
        let public_path =
            exact_public_trait_path(bound, self.profile, &self.public_traits);
        if let Some(public_path) = public_path {
            self.record(&public_path, PublicItemKind::Trait);
        }
        visit::visit_trait_bound(self, bound);
    }

    fn visit_pat(&mut self, pattern: &'ast syn::Pat) {
        if self.error.is_some() {
            return;
        }
        let path = match pattern {
            syn::Pat::Path(pattern) => {
                Some((pattern.qself.as_ref(), &pattern.path))
            }
            syn::Pat::Struct(pattern) => {
                Some((pattern.qself.as_ref(), &pattern.path))
            }
            syn::Pat::TupleStruct(pattern) => {
                Some((pattern.qself.as_ref(), &pattern.path))
            }
            _ => None,
        };
        let Some((qself, path)) = path else {
            visit::visit_pat(self, pattern);
            return;
        };
        match exact_public_variant_path(qself, path, self.profile) {
            Ok(Some(public_path)) => {
                self.record(&public_path, PublicItemKind::Variant);
            }
            Ok(None) => visit::visit_pat(self, pattern),
            Err(message) => self.fail(message),
        }
    }
}

fn exact_public_variant_path(
    qself: Option<&syn::QSelf>,
    path: &SynPath,
    profile: PairProfile,
) -> Result<Option<String>, String> {
    let root = path.segments.first().map(|segment| segment.ident.to_string());
    if root.as_deref() != Some(profile.spec().public_crate) {
        return Ok(None);
    }
    let exact = qself.is_none()
        && path.leading_colon.is_some()
        && path.segments.len() == 4
        && path
            .segments
            .iter()
            .all(|segment| matches!(segment.arguments, PathArguments::None));
    if !exact {
        return Err("public variant witness path is not exact".to_owned());
    }
    let parts = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    Ok(Some(parts[1..].join("::")))
}

fn exact_public_value_path(
    expression: &syn::ExprPath,
    profile: PairProfile,
) -> Result<Option<String>, String> {
    let path = &expression.path;
    if expression.qself.is_some() {
        return exact_public_trait_method_path(expression).map(Some);
    }
    let root = path.segments.first().map(|segment| segment.ident.to_string());
    if root.as_deref() != Some(profile.spec().public_crate) {
        return Ok(None);
    }
    if path.leading_colon.is_none() || path.segments.len() < 3 {
        return Err("external value witness path is not exact".to_owned());
    }
    let mut parts = Vec::with_capacity(path.segments.len());
    for (index, segment) in path.segments.iter().enumerate() {
        let final_segment = index + 1 == path.segments.len();
        if !exact_value_segment_arguments(
            &segment.arguments,
            final_segment,
            profile,
            &parts,
            &segment.ident.to_string(),
        ) {
            return Err(
                "external value witness has unavailable arguments".to_owned()
            );
        }
        parts.push(segment.ident.to_string());
    }
    Ok(Some(parts[1..].join("::")))
}

fn exact_public_trait_method_path(
    expression: &syn::ExprPath,
) -> Result<String, String> {
    let path = &expression.path;
    let qself = expression.qself.as_ref().ok_or_else(|| {
        "public trait method witness has no qualified self".to_owned()
    })?;
    let Type::TraitObject(object) = qself.ty.as_ref() else {
        return Err(
            "public trait method witness self is not dyn Trait".to_owned()
        );
    };
    let bounds = object.bounds.iter().collect::<Vec<_>>();
    let [TypeParamBound::Trait(bound)] = bounds.as_slice() else {
        return Err(
            "public trait method witness has the wrong dyn bounds".to_owned()
        );
    };
    let segments = path.segments.iter().collect::<Vec<_>>();
    let trait_segments = bound.path.segments.iter().collect::<Vec<_>>();
    let exact = object.dyn_token.is_some()
        && qself.as_token.is_none()
        && qself.position == 0
        && segments.len() == 1
        && path.leading_colon.is_some()
        && bound.path.leading_colon.is_some()
        && segments
            .iter()
            .all(|segment| matches!(segment.arguments, PathArguments::None))
        && trait_segments.len() == 3
        && trait_segments
            .iter()
            .all(|segment| matches!(segment.arguments, PathArguments::None));
    if !exact {
        return Err("public trait method witness path is not exact".to_owned());
    }
    let mut parts = trait_segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    parts.push(segments[0].ident.to_string());
    Ok(parts[1..].join("::"))
}

fn exact_value_segment_arguments(
    arguments: &PathArguments,
    final_segment: bool,
    profile: PairProfile,
    owner: &[String],
    name: &str,
) -> bool {
    if !final_segment {
        return matches!(arguments, PathArguments::None);
    }
    if matches!(arguments, PathArguments::None) {
        return true;
    }
    profile == PairProfile::KernelInterface
        && owner == [KERNEL_INTERFACE_CRATE, "event", "StartupEventDescriptor"]
        && name == "observe_timestamp"
        && exact_clock_witness_argument(arguments)
}

fn exact_clock_witness_argument(arguments: &PathArguments) -> bool {
    let PathArguments::AngleBracketed(arguments) = arguments else {
        return false;
    };
    if arguments.colon2_token.is_none() || arguments.args.len() != 1 {
        return false;
    }
    let Some(GenericArgument::Type(Type::TraitObject(object))) =
        arguments.args.first()
    else {
        return false;
    };
    let bounds = object.bounds.iter().collect::<Vec<_>>();
    let [TypeParamBound::Trait(bound)] = bounds.as_slice() else {
        return false;
    };
    let expected = BTreeSet::from(["clock::MonotonicClock".to_owned()]);
    exact_public_trait_path(bound, PairProfile::KernelInterface, &expected)
        .is_some()
}

fn validate_module_tree(
    sources: &SourceSet,
    target: &Path,
) -> Result<(), ProductSourceError> {
    let mut visited = BTreeSet::new();
    walk_module_source(sources, target, true, &mut visited)?;
    validate_visited_sources(sources, &visited)
}

fn walk_module_source(
    sources: &SourceSet,
    path: &Path,
    target_root: bool,
    visited: &mut BTreeSet<PathBuf>,
) -> Result<(), ProductSourceError> {
    if !visited.insert(path.to_path_buf()) {
        return Err(ProductSourceError::at(
            "PRODUCT_COMPILER_SOURCE_SET",
            path,
            "compiler source is reached by more than one module path",
        ));
    }
    let syntax = sources.syntax(path)?;
    let base = module_base(path, target_root);
    walk_module_items(sources, &syntax.items, &base, visited)
}

fn walk_module_items(
    sources: &SourceSet,
    items: &[Item],
    base: &Path,
    visited: &mut BTreeSet<PathBuf>,
) -> Result<(), ProductSourceError> {
    for item in items {
        let Item::Mod(module) = item else { continue };
        let name = module.ident.to_string();
        if let Some((_, nested)) = &module.content {
            walk_module_items(sources, nested, &base.join(name), visited)?;
            continue;
        }
        let source = resolve_external_module(sources, base, &name)?;
        walk_module_source(sources, &source, false, visited)?;
    }
    Ok(())
}

struct WitnessVisitor<'a> {
    path: &'a Path,
    profile: PairProfile,
    expected: BTreeMap<String, &'static str>,
    witnesses: BTreeMap<String, &'static str>,
    helpers: BTreeMap<&'static str, usize>,
    error: Option<ProductSourceError>,
}

impl WitnessVisitor<'_> {
    fn new(
        profile: PairProfile,
        relation: &PublicConsumerRelation,
    ) -> Result<Self, ProductSourceError> {
        Ok(Self {
            path: Path::new(""),
            profile,
            expected: expected_witnesses(profile, relation)?,
            witnesses: BTreeMap::new(),
            helpers: BTreeMap::new(),
            error: None,
        })
    }

    fn fail(&mut self, message: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(ProductSourceError::at(
                "PRODUCT_CONSUMER_DERIVE_SET",
                self.path,
                message,
            ));
        }
    }

    fn finish(self) -> Result<BTreeSet<String>, ProductSourceError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        validate_helper_set(self.profile, &self.helpers)?;
        if self.witnesses != self.expected {
            return Err(ProductSourceError::new(
                "PRODUCT_CONSUMER_DERIVE_SET",
                map_difference_message(&self.expected, &self.witnesses),
            ));
        }
        Ok(self.witnesses.into_keys().collect())
    }
}

fn expected_witnesses(
    profile: PairProfile,
    relation: &PublicConsumerRelation,
) -> Result<BTreeMap<String, &'static str>, ProductSourceError> {
    let mut derives = BTreeMap::<String, BTreeSet<BuiltinDerive>>::new();
    for check in &relation.derive_checks {
        derives
            .entry(check.public_type.clone())
            .or_default()
            .insert(check.derive);
    }
    let mut output = BTreeMap::new();
    for public_type in registered_public_types(relation) {
        let helper = derives
            .get(&public_type)
            .and_then(|set| helper_for_derives(profile, set))
            .ok_or_else(|| {
                ProductSourceError::new(
                    "PRODUCT_CONSUMER_DERIVE_SET",
                    format!("type {public_type} has no exact witness helper"),
                )
            })?;
        output.insert(public_type, helper);
    }
    Ok(output)
}

fn helper_for_derives(
    profile: PairProfile,
    derives: &BTreeSet<BuiltinDerive>,
) -> Option<&'static str> {
    let full = BTreeSet::from([
        BuiltinDerive::Clone,
        BuiltinDerive::Copy,
        BuiltinDerive::Debug,
        BuiltinDerive::PartialEq,
        BuiltinDerive::Eq,
    ]);
    if derives == &full {
        return Some(DERIVE_WITNESS);
    }
    if profile == PairProfile::Foundation {
        return None;
    }
    let clone_copy =
        BTreeSet::from([BuiltinDerive::Clone, BuiltinDerive::Copy]);
    if derives == &clone_copy {
        return Some(CLONE_COPY_WITNESS);
    }
    let clone_debug_eq = BTreeSet::from([
        BuiltinDerive::Clone,
        BuiltinDerive::Debug,
        BuiltinDerive::PartialEq,
        BuiltinDerive::Eq,
    ]);
    (derives == &clone_debug_eq).then_some(CLONE_DEBUG_EQ_WITNESS)
}

fn validate_helper_set(
    profile: PairProfile,
    helpers: &BTreeMap<&'static str, usize>,
) -> Result<(), ProductSourceError> {
    let expected = match profile {
        PairProfile::Foundation => vec![DERIVE_WITNESS],
        PairProfile::KernelInterface => {
            vec![DERIVE_WITNESS, CLONE_COPY_WITNESS, CLONE_DEBUG_EQ_WITNESS]
        }
    };
    let exact = helpers.len() == expected.len()
        && expected.into_iter().all(|name| helpers.get(name) == Some(&1));
    if exact {
        return Ok(());
    }
    Err(ProductSourceError::new(
        "PRODUCT_CONSUMER_DERIVE_SET",
        "consumer does not define each exact witness helper once",
    ))
}

fn map_difference_message(
    expected: &BTreeMap<String, &'static str>,
    observed: &BTreeMap<String, &'static str>,
) -> String {
    for (public_type, helper) in expected {
        if observed.get(public_type) != Some(helper) {
            return format!(
                "derive witness is missing or wrong: {public_type}"
            );
        }
    }
    if let Some(public_type) =
        observed.keys().find(|key| !expected.contains_key(*key))
    {
        return format!("unexpected derive witness: {public_type}");
    }
    "derive witness maps differ".to_owned()
}

impl<'ast> Visit<'ast> for WitnessVisitor<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if let Some(helper) = witness_helper_name(&item.sig.ident.to_string()) {
            self.observe_helper(helper, item);
        }
        visit::visit_item_fn(self, item);
    }

    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        if self.error.is_some() {
            return;
        }
        let syn::Expr::Path(function) = call.func.as_ref() else {
            visit::visit_expr_call(self, call);
            return;
        };
        let Some(last) = function.path.segments.last() else {
            visit::visit_expr_call(self, call);
            return;
        };
        let Some(helper) = witness_helper_name(&last.ident.to_string()) else {
            visit::visit_expr_call(self, call);
            return;
        };
        match explicit_witness_call(function, call, self.profile, helper) {
            Ok(public_type) => self.observe_witness(public_type, helper),
            Err(message) => self.fail(message),
        }
        visit::visit_expr_call(self, call);
    }
}

impl WitnessVisitor<'_> {
    fn observe_witness(&mut self, public_type: String, helper: &'static str) {
        if self.expected.get(&public_type).copied() != Some(helper) {
            self.fail(format!(
                "type {public_type} uses the wrong witness helper"
            ));
            return;
        }
        if self.witnesses.insert(public_type, helper).is_some() {
            self.fail("consumer repeats one derive witness");
        }
    }

    fn observe_helper(&mut self, helper: &'static str, item: &syn::ItemFn) {
        let count = self.helpers.entry(helper).or_default();
        *count = count.saturating_add(1);
        if !helper_allowed(self.profile, helper) {
            self.fail(format!("helper {helper} is unavailable for this pair"));
            return;
        }
        if let Err(message) = validate_witness_helper(item, helper) {
            self.fail(message);
        }
    }
}

fn witness_helper_name(name: &str) -> Option<&'static str> {
    match name {
        DERIVE_WITNESS => Some(DERIVE_WITNESS),
        CLONE_COPY_WITNESS => Some(CLONE_COPY_WITNESS),
        CLONE_DEBUG_EQ_WITNESS => Some(CLONE_DEBUG_EQ_WITNESS),
        _ => None,
    }
}

fn helper_allowed(profile: PairProfile, helper: &str) -> bool {
    match profile {
        PairProfile::Foundation => matches!(helper, DERIVE_WITNESS),
        PairProfile::KernelInterface => true,
    }
}

fn validate_witness_helper(
    item: &syn::ItemFn,
    helper: &'static str,
) -> Result<(), String> {
    let signature = &item.sig;
    if !matches!(item.vis, Visibility::Inherited)
        || !item.attrs.is_empty()
        || signature.constness.is_none()
        || signature.asyncness.is_some()
        || !matches!(signature.safety, syn::Safety::Default)
        || signature.abi.is_some()
        || signature.variadic.is_some()
        || !signature.inputs.is_empty()
        || !returns_bool(&signature.output)
    {
        return Err(format!("{helper} has the wrong function shape"));
    }
    let parameters = signature.generics.params.iter().collect::<Vec<_>>();
    let [syn::GenericParam::Type(parameter)] = parameters.as_slice() else {
        return Err(format!("{helper} requires one type parameter"));
    };
    if parameter.ident != "T"
        || !parameter.attrs.is_empty()
        || parameter.default.is_some()
    {
        return Err(format!("{helper} type parameter must be T"));
    }
    let mut bounds = parse_trait_bounds(&parameter.bounds)?;
    collect_where_bounds(&signature.generics, &mut bounds)?;
    let expected = witness_helper_bounds(helper);
    if bounds != expected {
        return Err(format!("{helper} has the wrong exact bounds"));
    }
    Ok(())
}

fn witness_helper_bounds(helper: &str) -> BTreeSet<String> {
    let names: &[&str] = match helper {
        DERIVE_WITNESS => &["Clone", "Copy", "Debug", "PartialEq", "Eq"],
        CLONE_COPY_WITNESS => &["Clone", "Copy"],
        CLONE_DEBUG_EQ_WITNESS => &["Clone", "Debug", "PartialEq", "Eq"],
        _ => &[],
    };
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn returns_bool(output: &syn::ReturnType) -> bool {
    let syn::ReturnType::Type(_, ty) = output else { return false };
    let Type::Path(path) = ty.as_ref() else { return false };
    path.qself.is_none() && path.path.is_ident("bool")
}

fn parse_trait_bounds(
    bounds: &Punctuated<TypeParamBound, Token![+]>,
) -> Result<BTreeSet<String>, String> {
    let mut output = BTreeSet::new();
    for bound in bounds {
        let TypeParamBound::Trait(bound) = bound else {
            return Err("derive_witness has a non-trait bound".to_owned());
        };
        let name = exact_builtin_trait_bound(bound).ok_or_else(|| {
            "derive_witness bound is not an exact core trait".to_owned()
        })?;
        if !output.insert(name.to_owned()) {
            return Err("derive_witness repeats one bound".to_owned());
        }
    }
    Ok(output)
}

fn collect_where_bounds(
    generics: &syn::Generics,
    output: &mut BTreeSet<String>,
) -> Result<(), String> {
    let Some(clause) = &generics.where_clause else { return Ok(()) };
    for predicate in &clause.predicates {
        let syn::WherePredicate::Type(predicate) = predicate else {
            return Err("derive_witness has a non-type predicate".to_owned());
        };
        if !type_is_t(&predicate.bounded_ty) || predicate.lifetimes.is_some() {
            return Err("derive_witness predicate does not bind T".to_owned());
        }
        let bounds = parse_trait_bounds(&predicate.bounds)?;
        for bound in bounds {
            insert_witness_bound(output, bound)?;
        }
    }
    Ok(())
}

fn insert_witness_bound(
    output: &mut BTreeSet<String>,
    bound: String,
) -> Result<(), String> {
    if !output.insert(bound) {
        return Err("derive_witness repeats one bound".to_owned());
    }
    Ok(())
}

fn type_is_t(ty: &Type) -> bool {
    let Type::Path(path) = ty else { return false };
    path.qself.is_none() && path.path.is_ident("T")
}

fn explicit_witness_call(
    function: &syn::ExprPath,
    call: &ExprCall,
    profile: PairProfile,
    helper: &'static str,
) -> Result<String, String> {
    if function.qself.is_some()
        || function.path.leading_colon.is_some()
        || !call.args.is_empty()
        || !exact_helper_path(&function.path, profile, helper)
    {
        return Err(format!("{helper} call does not use its exact path"));
    }
    let segment = function
        .path
        .segments
        .last()
        .ok_or_else(|| format!("{helper} call path is empty"))?;
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(format!("{helper} call requires explicit turbofish"));
    };
    if arguments.colon2_token.is_none() || arguments.args.len() != 1 {
        return Err(format!("{helper} call requires one turbofish type"));
    }
    let Some(GenericArgument::Type(Type::Path(ty))) = arguments.args.first()
    else {
        return Err(format!("{helper} argument is not a type path"));
    };
    external_public_type(ty, profile)
}

fn exact_helper_path(
    path: &SynPath,
    profile: PairProfile,
    helper: &str,
) -> bool {
    let parts = path.segments.iter().collect::<Vec<_>>();
    let exact_prefix = match profile {
        PairProfile::Foundation => parts.len() == 1,
        PairProfile::KernelInterface => {
            parts.len() == 2
                && parts[0].ident == "crate"
                && matches!(parts[0].arguments, PathArguments::None)
        }
    };
    exact_prefix && parts.last().is_some_and(|part| part.ident == helper)
}

fn external_public_type(
    ty: &syn::TypePath,
    profile: PairProfile,
) -> Result<String, String> {
    if ty.qself.is_some() || ty.path.leading_colon.is_none() {
        return Err(
            "derive witness type must use the extern-prelude root".to_owned()
        );
    }
    let segments = ty.path.segments.iter().collect::<Vec<_>>();
    let public_crate = profile.spec().public_crate;
    if segments.len() != 3
        || segments[0].ident != public_crate
        || !matches!(segments[0].arguments, PathArguments::None)
        || !matches!(segments[1].arguments, PathArguments::None)
    {
        return Err(
            "derive witness must use its exact full public path".to_owned()
        );
    }
    let public_type = format!("{}::{}", segments[1].ident, segments[2].ident);
    if !exact_witness_type_arguments(
        &segments[2].arguments,
        profile,
        &public_type,
    ) {
        return Err(
            "derive witness type has unavailable generic arguments".to_owned()
        );
    }
    Ok(public_type)
}

fn exact_witness_type_arguments(
    arguments: &PathArguments,
    profile: PairProfile,
    public_type: &str,
) -> bool {
    let lifetime_type = profile == PairProfile::KernelInterface
        && matches!(
            public_type,
            "event::CanonicalEventBytes"
                | "event::StartupEventRecord"
                | "event::StartupSnapshot"
        );
    if !lifetime_type {
        return matches!(arguments, PathArguments::None);
    }
    let PathArguments::AngleBracketed(arguments) = arguments else {
        return false;
    };
    arguments.args.len() == 1
        && matches!(
            arguments.args.first(),
            Some(GenericArgument::Lifetime(lifetime))
                if lifetime.ident == "static"
        )
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use reovim_depgraph::{
        ArchitecturalRole, SourceClassification, SourceOwner, SourceProvenance,
        SourceUse,
    };

    use super::{
        AnalysisLimits, AnalysisRequest, CompilerJoin, ProductSourceError,
        TwoPairAnalysisRequest, analyze, analyze_two_pairs,
    };
    use crate::{
        digest,
        product_closures::{ParseLimits, PublicItemKind, Registry},
        source::ClassifiedFile,
    };

    const SHIPPING: &[&str] = &[
        "kernel/foundation/src/bound.rs",
        "kernel/foundation/src/carrier.rs",
        "kernel/foundation/src/contract.rs",
        "kernel/foundation/src/identity.rs",
        "kernel/foundation/src/lib.rs",
    ];
    const CONSUMER: &str = "tests/foundation-public-consumer/src/lib.rs";
    const CONSUMER_ITEMS: &str =
        "tests/foundation-public-consumer/src/public_items.rs";
    const KI_SHIPPING: &[&str] = &[
        "kernel/interface/src/cancellation.rs",
        "kernel/interface/src/clock.rs",
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
        "kernel/interface/src/lib.rs",
        "kernel/interface/src/lifecycle.rs",
        "kernel/interface/src/operation.rs",
    ];
    const KI_CONSUMER: &[&str] = &[
        "tests/kernel-interface-public-consumer/src/clock.rs",
        "tests/kernel-interface-public-consumer/src/event/canonical.rs",
        "tests/kernel-interface-public-consumer/src/event/derives.rs",
        "tests/kernel-interface-public-consumer/src/event/descriptor.rs",
        "tests/kernel-interface-public-consumer/src/event/mod.rs",
        "tests/kernel-interface-public-consumer/src/event/outcomes.rs",
        "tests/kernel-interface-public-consumer/src/event/sequence.rs",
        "tests/kernel-interface-public-consumer/src/event/snapshot.rs",
        "tests/kernel-interface-public-consumer/src/lib.rs",
        "tests/kernel-interface-public-consumer/src/lifecycle.rs",
        "tests/kernel-interface-public-consumer/src/operation.rs",
        "tests/kernel-interface-public-consumer/src/public_items.rs",
    ];
    const LIMITS: AnalysisLimits = AnalysisLimits {
        source_files: 64,
        source_file_bytes: 1_048_576,
        source_total_bytes: 8_388_608,
        syntax_nodes: 100_000,
    };

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        registry: Registry,
        inventory: Vec<ClassifiedFile>,
        shipping: BTreeSet<PathBuf>,
        consumer: BTreeSet<PathBuf>,
        ki_shipping: BTreeSet<PathBuf>,
        ki_consumer: BTreeSet<PathBuf>,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove Product fixture");
        }
    }

    use std::collections::BTreeSet;

    #[test]
    fn real_surface_and_witnesses_are_exact() {
        let fixture = fixture();
        let observation = run(&fixture).expect("real Product source passes");
        assert_eq!(observation.public_items.len(), 143);
        assert_eq!(observation.derive_checks.len(), 165);
        assert_eq!(observation.witness_types.len(), 33);
        assert!(observation.public_items.iter().any(|item| {
            item.path == "carrier::CarrierDescriptorError::invocation"
                && item.kind == PublicItemKind::Function
        }));
        assert!(observation.public_items.iter().all(|item| {
            !item.path.ends_with("::fmt")
                && !item.path.starts_with("std::error")
        }));
    }

    #[test]
    fn exact_two_pair_surface_aliases_and_witnesses_pass() {
        let fixture = fixture();
        let observation =
            run_two_pairs(&fixture).expect("real two-pair source passes");
        assert_eq!(observation.foundation.public_items.len(), 143);
        assert_eq!(observation.foundation.derive_checks.len(), 165);
        assert_eq!(observation.foundation.witness_types.len(), 33);
        assert_eq!(observation.kernel_interface.public_items.len(), 356);
        assert_eq!(observation.kernel_interface.derive_checks.len(), 306);
        assert_eq!(observation.kernel_interface.witness_types.len(), 64);
        let relation = relation(&fixture, "kernel-interface-public-api");
        assert_eq!(relation.reexports.len(), 6);
        assert!(relation.reexports.iter().all(|item| {
            has_public_type(
                &observation.kernel_interface.public_items,
                &item.public_type,
            )
        }));
    }

    #[test]
    fn compiler_join_requires_one_exact_rmeta_identity() {
        let fixture = fixture();
        let extern_path = fixture.root.join("compiler/foundation.rmeta");
        let provider_path = fixture.root.join("provider/foundation.rmeta");
        let error = run_two_pairs_join(&fixture, &extern_path, &provider_path)
            .expect_err("different metadata paths must fail");
        assert_eq!(error.code, "PRODUCT_SOURCE_COMPILER_JOIN", "{error}");
    }

    #[test]
    fn reexport_registry_mutants_fail_exactly() {
        let mut missing = fixture();
        ki_relation_mut(&mut missing).reexports.pop();
        assert_two_pair_code(&missing, "PRODUCT_SOURCE_REEXPORT_SET");

        let mut extra = fixture();
        let mut reexport = ki_relation_mut(&mut extra).reexports[0].clone();
        reexport.public_type = "clock::ClockDescriptor".to_owned();
        ki_relation_mut(&mut extra).reexports.push(reexport);
        assert_two_pair_code(&extra, "PRODUCT_SOURCE_REEXPORT_SET");

        let mut retargeted = fixture();
        ki_relation_mut(&mut retargeted).reexports[0].provider_type =
            "bound::BoundError".to_owned();
        assert_two_pair_code(&retargeted, "PRODUCT_SOURCE_REEXPORT_SET");
    }

    #[test]
    fn local_same_shaped_type_cannot_replace_a_reexport() {
        ki_source_mutant(
            "kernel/interface/src/clock.rs",
            "pub use ::reovim_foundation::bound::CheckedDuration;",
            concat!(
                "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n",
                "pub struct CheckedDuration;",
            ),
            "PRODUCT_SOURCE_REEXPORT_SET",
        );
    }

    #[test]
    fn inherent_impl_accepts_only_matching_lifetime_binders() {
        let path = "kernel/interface/src/event/canonical.rs";
        let header = "impl<'a> crate::event::CanonicalEventBytes<'a> {";
        ki_source_mutant(
            path,
            header,
            "impl<T> crate::event::CanonicalEventBytes<T> {",
            "PRODUCT_INHERENT_IMPL_UNAVAILABLE",
        );
        ki_source_mutant(
            path,
            header,
            "impl<'a> crate::event::CanonicalEventBytes<'static> {",
            "PRODUCT_INHERENT_IMPL_UNAVAILABLE",
        );
        ki_source_mutant(
            path,
            header,
            "impl<'a> crate::event::CanonicalEventBytes {",
            "PRODUCT_INHERENT_IMPL_UNAVAILABLE",
        );
    }

    #[test]
    fn generic_trait_bound_escapes_fail_closed() {
        let path = "kernel/interface/src/event/admission.rs";
        let bound = "C: MonotonicClock + ?Sized,";
        ki_source_mutant(
            path,
            bound,
            "C: crate::clock::MonotonicClock + ?Sized,",
            "PRODUCT_TRAIT_BOUND_UNAVAILABLE",
        );
        ki_source_mutant(
            path,
            bound,
            "C: for<'a> MonotonicClock + ?Sized,",
            "PRODUCT_TRAIT_BOUND_UNAVAILABLE",
        );
    }

    #[test]
    fn kernel_interface_witness_paths_and_helpers_are_exact() {
        let path = "tests/kernel-interface-public-consumer/src/lifecycle.rs";
        let full = concat!(
            "crate::derive_witness::<",
            "::reovim_kernel_interface::lifecycle::Generation>()",
        );
        ki_source_mutant(
            path,
            full,
            "crate::derive_witness::<Generation>()",
            "PRODUCT_CONSUMER_DERIVE_SET",
        );
        ki_source_mutant(
            path,
            full,
            concat!(
                "crate::clone_copy_witness::<",
                "::reovim_kernel_interface::lifecycle::Generation>()",
            ),
            "PRODUCT_CONSUMER_DERIVE_SET",
        );
        ki_source_mutant(
            "tests/kernel-interface-public-consumer/src/lib.rs",
            "const fn clone_copy_witness<T>() -> bool",
            "const fn renamed_clone_copy_witness<T>() -> bool",
            "PRODUCT_TRAIT_BOUND_UNAVAILABLE",
        );
    }

    #[test]
    fn private_builtin_derives_are_valid_but_not_published() {
        let mut fixture = fixture();
        append_source(
            &mut fixture,
            "kernel/interface/src/cancellation.rs",
            concat!(
                "\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n",
                "struct PrivateDerived;\n",
            ),
        );
        let observation =
            run_two_pairs(&fixture).expect("private built-in derive passes");
        assert!(
            observation
                .kernel_interface
                .public_items
                .iter()
                .all(|item| item.path != "cancellation::PrivateDerived")
        );
        assert!(
            observation.kernel_interface.derive_checks.iter().all(
                |check| check.public_type != "cancellation::PrivateDerived"
            )
        );
    }

    #[test]
    fn trait_impl_methods_are_excluded_but_inherent_methods_are_exact() {
        let mut accepted = fixture();
        append_source(
            &mut accepted,
            "kernel/foundation/src/carrier.rs",
            concat!(
                "\ntrait HiddenTrait { fn hidden(self); }\n",
                "impl HiddenTrait for MemorySafety {\n",
                "    fn hidden(self) {}\n",
                "}\n",
            ),
        );
        run(&accepted).expect("trait-impl method is not inherent surface");

        source_mutant(
            "kernel/foundation/src/carrier.rs",
            "impl MemorySafety {",
            concat!(
                "impl MemorySafety {\n",
                "    pub const fn extra(self) -> Self { self }\n",
            ),
            "PRODUCT_PUBLIC_ITEM_SET",
        );
    }

    #[test]
    fn compiler_source_tree_mutants_fail_with_current_digests() {
        let mut missing = fixture();
        missing.shipping.remove(Path::new("kernel/foundation/src/carrier.rs"));
        assert_code(&missing, "PRODUCT_MODULE_SOURCE_MISSING");

        let mut extra = fixture();
        add_source(
            &mut extra,
            "tests/foundation-public-consumer/src/extra.rs",
            "fn unused() {}\n",
            ArchitecturalRole::KernelInterface,
            SourceUse::Test,
        );
        extra.consumer.insert(PathBuf::from(
            "tests/foundation-public-consumer/src/extra.rs",
        ));
        assert_code(&extra, "PRODUCT_COMPILER_SOURCE_SET");
    }

    #[test]
    fn classification_identity_mutants_fail_closed() {
        let mut role = fixture();
        let file =
            inventory_file_mut(&mut role, "kernel/foundation/src/carrier.rs");
        file.classification = classification(
            ArchitecturalRole::KernelInterface,
            SourceUse::Production,
        );
        assert_code(&role, "PRODUCT_SOURCE_CLASSIFICATION");

        let mut generated = fixture();
        let file = inventory_file_mut(
            &mut generated,
            "kernel/foundation/src/carrier.rs",
        );
        file.classification.provenance = SourceProvenance::Generated;
        assert_code(&generated, "PRODUCT_SOURCE_CLASSIFICATION");
    }

    #[test]
    fn public_item_and_method_mutants_fail_exactly() {
        source_mutant(
            "kernel/foundation/src/carrier.rs",
            "    pub const fn invocation(self) -> InvocationModel {",
            "    const fn invocation(self) -> InvocationModel {",
            "PRODUCT_PUBLIC_ITEM_SET",
        );

        let mut registry = fixture();
        let item = registry.registry.public_consumer_relations[0]
            .public_items
            .iter_mut()
            .find(|item| {
                item.path == "carrier::CarrierDescriptorError::invocation"
            })
            .expect("registered method");
        item.kind = PublicItemKind::Const;
        assert_code(&registry, "PRODUCT_PUBLIC_ITEM_SET");
    }

    #[test]
    fn derive_mutants_fail_with_current_source_identity() {
        source_mutant(
            "kernel/foundation/src/identity.rs",
            "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
            "#[derive(Clone, Debug, PartialEq, Eq)]",
            "PRODUCT_DERIVE_SET",
        );
        source_mutant(
            "kernel/foundation/src/identity.rs",
            "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
            "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]",
            "PRODUCT_DERIVE_SET",
        );
        source_mutant(
            "kernel/foundation/src/identity.rs",
            "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
            "#[derive(core::clone::Clone, Copy, Debug, PartialEq, Eq)]",
            "PRODUCT_DERIVE_UNAVAILABLE",
        );
    }

    #[test]
    fn attributes_aliases_macros_and_unsafe_are_unavailable() {
        let cases = [
            (
                "pub enum OwnershipModel {",
                "#[repr(C)]\npub enum OwnershipModel {",
                "PRODUCT_ATTRIBUTE_UNAVAILABLE",
            ),
            (
                "pub enum OwnershipModel {",
                "#[cfg(test)]\npub enum OwnershipModel {",
                "UNSUPPORTED_CFG_PROOF_GAP",
            ),
            (
                "use core::fmt;",
                "use core::fmt;\nuse std::path::PathBuf as Hidden;",
                "PRODUCT_IMPORT_UNAVAILABLE",
            ),
            (
                "use core::fmt;",
                "use core::fmt;\nconst HIDDEN: &str = env!(\"HOME\");",
                "UNSUPPORTED_ENV_PROOF_GAP",
            ),
            (
                "use core::fmt;",
                "use core::fmt;\nconst HIDDEN: &str = include_str!(\"x\");",
                "UNSUPPORTED_INCLUDE_PROOF_GAP",
            ),
            (
                "pub enum OwnershipModel {",
                "pub unsafe fn hidden() {}\npub enum OwnershipModel {",
                "FIRST_PARTY_UNSAFE_SYNTAX",
            ),
        ];
        for (old, new, code) in cases {
            source_mutant("kernel/foundation/src/carrier.rs", old, new, code);
        }
    }

    #[test]
    fn reexports_type_aliases_and_module_paths_are_unavailable() {
        source_mutant(
            "kernel/foundation/src/lib.rs",
            "pub mod bound;",
            "pub use bound::BoundError;\npub mod bound;",
            "UNSUPPORTED_REEXPORT_PROOF_GAP",
        );
        source_mutant(
            "kernel/foundation/src/lib.rs",
            "pub mod bound;",
            "pub type Hidden = u64;\npub mod bound;",
            "UNSUPPORTED_PUBLIC_ALIAS_PROOF_GAP",
        );
        source_mutant(
            "kernel/foundation/src/lib.rs",
            "pub mod bound;",
            "#[path = \"bound.rs\"]\npub mod bound;",
            "UNSUPPORTED_PATH_MODULE",
        );
    }

    #[test]
    fn forbidden_private_type_paths_are_not_hidden_from_analysis() {
        source_mutant(
            "kernel/foundation/src/carrier.rs",
            "use core::fmt;",
            concat!(
                "use core::fmt;\n",
                "struct Hidden { value: std::path::PathBuf }",
            ),
            "PRODUCT_TYPE_PATH_UNAVAILABLE",
        );
    }

    #[test]
    fn consumer_witness_set_and_helper_shape_are_exact() {
        source_mutant(
            CONSUMER,
            "derive_witness::<::reovim_foundation::bound::ArithmeticRule>()",
            "true",
            "PRODUCT_CONSUMER_DERIVE_SET",
        );
        source_mutant(
            CONSUMER,
            "        + ::core::cmp::PartialEq\n",
            "",
            "PRODUCT_CONSUMER_DERIVE_SET",
        );
        source_mutant(
            CONSUMER,
            "const fn derive_witness<T>() -> bool",
            "fn derive_witness<T>() -> bool",
            "PRODUCT_CONSUMER_DERIVE_SET",
        );
        source_mutant(
            CONSUMER,
            "derive_witness::<::reovim_foundation::bound::ArithmeticRule>()",
            "derive_witness::<::reovim_foundation::bound::BoundError>()",
            "PRODUCT_CONSUMER_DERIVE_SET",
        );
    }

    #[test]
    fn foundation_public_item_witnesses_are_complete_and_exact() {
        let path = CONSUMER_ITEMS;
        source_mutant(
            path,
            "        ::reovim_foundation::identity::InstanceId::value,\n",
            "",
            "PRODUCT_CONSUMER_PUBLIC_ITEM_SET",
        );
        source_mutant(
            path,
            "::reovim_foundation::identity::InstanceId::value",
            "::reovim_foundation::identity::InstanceId::new",
            "PRODUCT_CONSUMER_PUBLIC_ITEM_SET",
        );
    }

    #[test]
    fn kernel_interface_public_item_witnesses_are_complete_and_exact() {
        let path = "tests/kernel-interface-public-consumer/src/public_items.rs";
        ki_source_mutant(
            path,
            concat!(
                "        ::reovim_kernel_interface::event::",
                "EVENT_RING_MAX_BYTES,\n",
            ),
            "",
            "PRODUCT_CONSUMER_PUBLIC_ITEM_SET",
        );
        ki_source_mutant(
            path,
            concat!(
                "::reovim_kernel_interface::cancellation::",
                "CancellationOutcome::cause",
            ),
            concat!(
                "::reovim_kernel_interface::cancellation::",
                "CancellationOutcome::execution",
            ),
            "PRODUCT_CONSUMER_PUBLIC_ITEM_SET",
        );
    }

    #[test]
    fn foundation_variant_add_delete_and_retarget_mutants_fail() {
        let shipping = "kernel/foundation/src/bound.rs";
        let declaration = concat!(
            "pub enum ArithmeticRule {\n",
            "    /// All arithmetic is checked and ",
            "overflow is a typed failure.\n",
            "    Checked,\n",
            "}",
        );
        source_mutant(
            shipping,
            declaration,
            concat!(
                "pub enum ArithmeticRule {\n",
                "    /// All arithmetic is checked and ",
                "overflow is a typed failure.\n",
                "    Checked,\n",
                "    AuditAdded,\n",
                "}",
            ),
            "PRODUCT_PUBLIC_ITEM_SET",
        );
        source_mutant(
            shipping,
            declaration,
            "pub enum ArithmeticRule {}",
            "PRODUCT_PUBLIC_ITEM_SET",
        );
        source_mutant(
            CONSUMER_ITEMS,
            concat!(
                "::reovim_foundation::bound::ZeroPolicy::Allow\n",
                "        |",
            ),
            concat!(
                "::reovim_foundation::bound::ZeroPolicy::Reject\n",
                "        |",
            ),
            "PRODUCT_CONSUMER_PUBLIC_ITEM_SET",
        );
    }

    #[test]
    fn kernel_variant_add_delete_and_retarget_mutants_fail() {
        let shipping = "kernel/interface/src/cancellation.rs";
        let declaration = concat!(
            "pub enum CancellationState {\n",
            "    /// No cancellation request has been observed.\n",
            "    Open,\n",
            "    /// Cancellation was requested for the stated cause.\n",
            "    Requested(CancellationCause),\n",
            "}",
        );
        ki_source_mutant(
            shipping,
            declaration,
            concat!(
                "pub enum CancellationState {\n",
                "    /// No cancellation request has been observed.\n",
                "    Open,\n",
                "    /// Cancellation was requested for the stated cause.\n",
                "    Requested(CancellationCause),\n",
                "    AuditAdded,\n",
                "}",
            ),
            "PRODUCT_PUBLIC_ITEM_SET",
        );
        ki_source_mutant(
            shipping,
            declaration,
            concat!(
                "pub enum CancellationState {\n",
                "    /// Cancellation was requested for the stated cause.\n",
                "    Requested(CancellationCause),\n",
                "}",
            ),
            "PRODUCT_PUBLIC_ITEM_SET",
        );
        ki_source_mutant(
            "tests/kernel-interface-public-consumer/src/public_items.rs",
            concat!(
                "::reovim_kernel_interface::cancellation::",
                "CancellationCause::\n",
                "            ExplicitRequest\n",
                "        |",
            ),
            concat!(
                "::reovim_kernel_interface::cancellation::",
                "CancellationCause::\n",
                "            Replacement\n",
                "        |",
            ),
            "PRODUCT_CONSUMER_PUBLIC_ITEM_SET",
        );
    }

    #[test]
    fn public_fields_and_default_trait_methods_are_unavailable() {
        source_mutant(
            "kernel/foundation/src/contract.rs",
            "pub struct ContractId(u128);",
            "pub struct ContractId(pub u128);",
            "PRODUCT_PUBLIC_FIELD_UNAVAILABLE",
        );
        ki_source_mutant(
            "kernel/interface/src/clock.rs",
            "pub trait MonotonicClock {",
            concat!(
                "pub trait MonotonicClock {\n",
                "    fn audit_default(&self) {}",
            ),
            "PRODUCT_PUBLIC_TRAIT_MEMBER_UNAVAILABLE",
        );
    }

    #[test]
    fn helper_bounds_reject_compiling_local_trait_substitutes() {
        source_mutant(
            CONSUMER,
            concat!(
                "const fn derive_witness<T>() -> bool\n",
                "where\n",
                "    T: ::core::clone::Clone",
            ),
            concat!(
                "mod fake {\n",
                "    pub trait Clone {}\n",
                "    impl<T> Clone for T {}\n",
                "}\n\n",
                "const fn derive_witness<T>() -> bool\n",
                "where\n",
                "    T: fake::Clone",
            ),
            "PRODUCT_TRAIT_BOUND_UNAVAILABLE",
        );
    }

    #[test]
    fn public_trait_impl_paths_require_the_extern_root() {
        ki_source_mutant(
            "tests/kernel-interface-public-consumer/src/clock.rs",
            concat!(
                "impl ::reovim_kernel_interface::clock::MonotonicClock ",
                "for FixedClock",
            ),
            concat!(
                "impl reovim_kernel_interface::clock::MonotonicClock ",
                "for FixedClock",
            ),
            "PRODUCT_CONSUMER_TRAIT_IMPL_SET",
        );
    }

    #[test]
    fn standard_environment_value_paths_are_unavailable_in_shipping() {
        let mut foundation = fixture();
        append_source(
            &mut foundation,
            "kernel/foundation/src/carrier.rs",
            "\nfn audit_environment() { let _ = std::fs::read(\"x\"); }\n",
        );
        assert_code(&foundation, "PRODUCT_STD_VALUE_PATH_UNAVAILABLE");

        let mut kernel = fixture();
        append_source(
            &mut kernel,
            "kernel/interface/src/cancellation.rs",
            "\nfn audit_environment() { let _ = ::std::fs::read(\"x\"); }\n",
        );
        assert_two_pair_code(&kernel, "PRODUCT_STD_VALUE_PATH_UNAVAILABLE");

        let mut raw_identifier = fixture();
        append_source(
            &mut raw_identifier,
            "kernel/foundation/src/carrier.rs",
            concat!(
                "\nfn audit_raw_environment() { ",
                "let _ = ::r#std::fs::read(\"x\"); }\n",
            ),
        );
        assert_code(&raw_identifier, "PRODUCT_STD_VALUE_PATH_UNAVAILABLE");

        let mut raw_kernel = fixture();
        append_source(
            &mut raw_kernel,
            "kernel/interface/src/cancellation.rs",
            concat!(
                "\nfn audit_raw_environment() { ",
                "let _ = r#std::fs::read(\"x\"); }\n",
            ),
        );
        assert_two_pair_code(&raw_kernel, "PRODUCT_STD_VALUE_PATH_UNAVAILABLE");
    }

    #[test]
    fn lexical_shadow_modules_cannot_satisfy_foundation_witnesses() {
        source_mutant(
            CONSUMER_ITEMS,
            "::reovim_foundation::identity::InstanceId::value",
            concat!(
                "{ mod reovim_foundation { pub mod identity { ",
                "pub struct InstanceId; impl InstanceId { ",
                "pub const fn value() {} } } } ",
                "reovim_foundation::identity::InstanceId::value }",
            ),
            "PRODUCT_CONSUMER_PUBLIC_ITEM_SET",
        );
        source_mutant(
            CONSUMER,
            concat!(
                "derive_witness::<::reovim_foundation::identity::",
                "InstanceId>()",
            ),
            concat!(
                "{ mod reovim_foundation { pub mod identity { ",
                "#[derive(Clone, Copy, Debug, PartialEq, Eq)] ",
                "pub struct InstanceId; } } ",
                "derive_witness::<reovim_foundation::identity::",
                "InstanceId>() }",
            ),
            "PRODUCT_CONSUMER_DERIVE_SET",
        );
    }

    #[test]
    fn lexical_shadow_modules_cannot_satisfy_kernel_witnesses() {
        let items =
            "tests/kernel-interface-public-consumer/src/public_items.rs";
        ki_source_mutant(
            items,
            concat!(
                "::reovim_kernel_interface::cancellation::",
                "CancellationOutcome::cause",
            ),
            concat!(
                "{ mod reovim_kernel_interface { pub mod cancellation { ",
                "pub struct CancellationOutcome; impl CancellationOutcome { ",
                "pub const fn cause() {} } } } ",
                "reovim_kernel_interface::cancellation::",
                "CancellationOutcome::cause }",
            ),
            "PRODUCT_CONSUMER_PUBLIC_ITEM_SET",
        );
        ki_source_mutant(
            "tests/kernel-interface-public-consumer/src/lifecycle.rs",
            concat!(
                "crate::derive_witness::<::reovim_kernel_interface::",
                "lifecycle::Generation>()",
            ),
            concat!(
                "{ mod reovim_kernel_interface { pub mod lifecycle { ",
                "#[derive(Clone, Copy, Debug, PartialEq, Eq)] ",
                "pub struct Generation; } } ",
                "crate::derive_witness::<reovim_kernel_interface::",
                "lifecycle::Generation>() }",
            ),
            "PRODUCT_CONSUMER_DERIVE_SET",
        );
        ki_source_mutant(
            items,
            concat!(
                "let _: Option<&dyn ::reovim_kernel_interface::clock::",
                "MonotonicClock> = None;",
            ),
            concat!(
                "mod reovim_kernel_interface { pub mod clock { ",
                "pub trait MonotonicClock {} } } ",
                "let _: Option<&dyn reovim_kernel_interface::clock::",
                "MonotonicClock> = None;",
            ),
            "PRODUCT_TRAIT_BOUND_UNAVAILABLE",
        );
    }

    fn fixture() -> Fixture {
        let repository = repository_root();
        let root = fixture_root();
        let registry = read_registry(&repository);
        let shipping =
            SHIPPING.iter().map(PathBuf::from).collect::<BTreeSet<_>>();
        let consumer =
            [CONSUMER, CONSUMER_ITEMS].into_iter().map(PathBuf::from).collect();
        let ki_shipping =
            KI_SHIPPING.iter().map(PathBuf::from).collect::<BTreeSet<_>>();
        let ki_consumer =
            KI_CONSUMER.iter().map(PathBuf::from).collect::<BTreeSet<_>>();
        let mut fixture = Fixture {
            root,
            registry,
            inventory: Vec::new(),
            shipping,
            consumer,
            ki_shipping,
            ki_consumer,
        };
        copy_sources(&repository, &mut fixture);
        fixture
    }

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("conformance crate below repository root")
            .to_path_buf()
    }

    fn fixture_root() -> PathBuf {
        let serial = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "reovim-product-source-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create Product fixture root");
        root
    }

    fn read_registry(repository: &Path) -> Registry {
        let bytes =
            fs::read(repository.join("conformance/product-closures.json"))
                .expect("read Product registry");
        let value =
            serde_json::from_slice(&bytes).expect("parse registry JSON");
        Registry::parse(
            &value,
            ParseLimits {
                path_bytes: 4096,
                closure_items: 1024,
                package_members: 8192,
                public_items: 8192,
            },
        )
        .expect("validate Product registry")
    }

    fn copy_sources(repository: &Path, fixture: &mut Fixture) {
        let paths = fixture
            .shipping
            .iter()
            .chain(&fixture.consumer)
            .chain(&fixture.ki_shipping)
            .chain(&fixture.ki_consumer)
            .cloned()
            .collect::<Vec<_>>();
        for path in paths {
            let bytes = fs::read(repository.join(&path)).expect("read source");
            write_source(fixture, &path, &bytes);
        }
    }

    fn write_source(fixture: &mut Fixture, path: &Path, bytes: &[u8]) {
        let absolute = fixture.root.join(path);
        fs::create_dir_all(absolute.parent().expect("source parent"))
            .expect("create source parent");
        fs::write(&absolute, bytes).expect("write source");
        let (role, source_use) = source_classification(path);
        fixture.inventory.push(classified_file(path, bytes, role, source_use));
    }

    fn source_classification(path: &Path) -> (ArchitecturalRole, SourceUse) {
        if path.starts_with("kernel/foundation") {
            return (ArchitecturalRole::Foundation, SourceUse::Production);
        }
        if path.starts_with("kernel/interface") {
            return (ArchitecturalRole::KernelInterface, SourceUse::Production);
        }
        if path.starts_with("tests/foundation-public-consumer") {
            return (ArchitecturalRole::KernelInterface, SourceUse::Test);
        }
        (ArchitecturalRole::KernelRuntime, SourceUse::Test)
    }

    fn classified_file(
        path: &Path,
        bytes: &[u8],
        role: ArchitecturalRole,
        source_use: SourceUse,
    ) -> ClassifiedFile {
        ClassifiedFile {
            path: path.to_path_buf(),
            content_digest: digest::bytes(bytes),
            classification: classification(role, source_use),
            upstream: None,
            rule_prefix: path.to_path_buf(),
            rule_precedence: 10,
        }
    }

    fn classification(
        role: ArchitecturalRole,
        source_use: SourceUse,
    ) -> SourceClassification {
        SourceClassification {
            owner: SourceOwner::Product(role),
            provenance: SourceProvenance::Authored,
            source_use,
        }
    }

    fn run(
        fixture: &Fixture,
    ) -> Result<super::ProductSourceObservation, ProductSourceError> {
        let shipping = fixture
            .registry
            .closures
            .iter()
            .find(|closure| closure.id == "foundation-shipping")
            .expect("shipping closure");
        let consumer = fixture
            .registry
            .closures
            .iter()
            .find(|closure| closure.id == "foundation-consumer")
            .expect("consumer closure");
        analyze(&AnalysisRequest {
            root: &fixture.root,
            shipping,
            consumer,
            relation: relation(fixture, "foundation-public-api"),
            shipping_sources: &fixture.shipping,
            consumer_sources: &fixture.consumer,
            inventory: &fixture.inventory,
            limits: LIMITS,
        })
    }

    fn run_two_pairs(
        fixture: &Fixture,
    ) -> Result<super::TwoPairSourceObservation, ProductSourceError> {
        let rmeta = fixture.root.join("compiler/libreovim_foundation.rmeta");
        run_two_pairs_join(fixture, &rmeta, &rmeta)
    }

    fn run_two_pairs_join(
        fixture: &Fixture,
        extern_path: &Path,
        provider_rmeta_path: &Path,
    ) -> Result<super::TwoPairSourceObservation, ProductSourceError> {
        let foundation = pair_request(
            fixture,
            "foundation-shipping",
            "foundation-consumer",
            "foundation-public-api",
            &fixture.shipping,
            &fixture.consumer,
        );
        let kernel_interface = pair_request(
            fixture,
            "kernel-interface-shipping",
            "kernel-interface-consumer",
            "kernel-interface-public-api",
            &fixture.ki_shipping,
            &fixture.ki_consumer,
        );
        analyze_two_pairs(&TwoPairAnalysisRequest {
            foundation,
            kernel_interface,
            compiler_join: CompilerJoin {
                consumer_closure: "kernel-interface-shipping",
                extern_name: "reovim_foundation",
                provider_closure: "foundation-shipping",
                provider_relation: "foundation-public-api",
                extern_path,
                provider_rmeta_path,
            },
        })
    }

    fn pair_request<'a>(
        fixture: &'a Fixture,
        shipping: &str,
        consumer: &str,
        relation_id: &str,
        shipping_sources: &'a BTreeSet<PathBuf>,
        consumer_sources: &'a BTreeSet<PathBuf>,
    ) -> AnalysisRequest<'a> {
        AnalysisRequest {
            root: &fixture.root,
            shipping: closure(fixture, shipping),
            consumer: closure(fixture, consumer),
            relation: relation(fixture, relation_id),
            shipping_sources,
            consumer_sources,
            inventory: &fixture.inventory,
            limits: LIMITS,
        }
    }

    fn closure<'a>(fixture: &'a Fixture, id: &str) -> &'a super::Closure {
        fixture
            .registry
            .closures
            .iter()
            .find(|closure| closure.id == id)
            .expect("registered closure")
    }

    fn relation<'a>(
        fixture: &'a Fixture,
        id: &str,
    ) -> &'a super::PublicConsumerRelation {
        fixture
            .registry
            .public_consumer_relations
            .iter()
            .find(|relation| relation.id == id)
            .expect("registered relation")
    }

    fn has_public_type(items: &[super::PublicItem], path: &str) -> bool {
        items.contains(&super::PublicItem {
            path: path.to_owned(),
            kind: PublicItemKind::Type,
        })
    }

    fn ki_relation_mut(
        fixture: &mut Fixture,
    ) -> &mut super::PublicConsumerRelation {
        fixture
            .registry
            .public_consumer_relations
            .iter_mut()
            .find(|relation| relation.id == "kernel-interface-public-api")
            .expect("registered KI relation")
    }

    fn source_mutant(path: &str, old: &str, new: &str, code: &'static str) {
        let mut fixture = fixture();
        replace_source(&mut fixture, path, old, new);
        assert_code(&fixture, code);
    }

    fn ki_source_mutant(path: &str, old: &str, new: &str, code: &'static str) {
        let mut fixture = fixture();
        replace_source(&mut fixture, path, old, new);
        assert_two_pair_code(&fixture, code);
    }

    fn replace_source(fixture: &mut Fixture, path: &str, old: &str, new: &str) {
        let absolute = fixture.root.join(path);
        let original = fs::read_to_string(&absolute).expect("read mutant");
        assert!(original.contains(old), "mutation anchor is absent");
        let changed = original.replacen(old, new, 1);
        fs::write(&absolute, changed.as_bytes()).expect("write mutant");
        refresh_digest(fixture, path, changed.as_bytes());
    }

    fn append_source(fixture: &mut Fixture, path: &str, suffix: &str) {
        let absolute = fixture.root.join(path);
        let mut changed = fs::read_to_string(&absolute).expect("read append");
        changed.push_str(suffix);
        fs::write(&absolute, changed.as_bytes()).expect("write append");
        refresh_digest(fixture, path, changed.as_bytes());
    }

    fn refresh_digest(fixture: &mut Fixture, path: &str, bytes: &[u8]) {
        inventory_file_mut(fixture, path).content_digest = digest::bytes(bytes);
    }

    fn inventory_file_mut<'a>(
        fixture: &'a mut Fixture,
        path: &str,
    ) -> &'a mut ClassifiedFile {
        fixture
            .inventory
            .iter_mut()
            .find(|file| file.path == Path::new(path))
            .expect("inventoried source")
    }

    fn add_source(
        fixture: &mut Fixture,
        path: &str,
        text: &str,
        role: ArchitecturalRole,
        source_use: SourceUse,
    ) {
        let path = Path::new(path);
        let absolute = fixture.root.join(path);
        fs::create_dir_all(absolute.parent().expect("extra source parent"))
            .expect("create extra source parent");
        fs::write(absolute, text).expect("write extra source");
        fixture.inventory.push(classified_file(
            path,
            text.as_bytes(),
            role,
            source_use,
        ));
    }

    fn assert_code(fixture: &Fixture, expected: &'static str) {
        let error = run(fixture).expect_err("mutant must fail");
        assert_eq!(error.code, expected, "{error}");
    }

    fn assert_two_pair_code(fixture: &Fixture, expected: &'static str) {
        let error = run_two_pairs(fixture).expect_err("mutant must fail");
        assert_eq!(error.code, expected, "{error}");
    }
}
