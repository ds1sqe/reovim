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
        PublicConsumerRelation, PublicItem, PublicItemKind,
    },
    source::{ClassifiedFile, read_bounded},
};

const FOUNDATION_CRATE: &str = "reovim_foundation";
const DERIVE_WITNESS: &str = "derive_witness";

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

/// Source-derived facts retained after the closed checks pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductSourceObservation {
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
pub fn analyze(
    request: &AnalysisRequest<'_>,
) -> Result<ProductSourceObservation, ProductSourceError> {
    validate_limits(request.limits)?;
    validate_source_count(request)?;
    validate_relation(request)?;
    let inventory = inventory_index(request.inventory)?;
    let mut budget = ReadBudget::new(request.limits.source_total_bytes);
    let shipping = load_source_set(
        request.root,
        request.shipping,
        request.shipping_sources,
        &inventory,
        request.limits,
        &mut budget,
    )?;
    let consumer = load_source_set(
        request.root,
        request.consumer,
        request.consumer_sources,
        &inventory,
        request.limits,
        &mut budget,
    )?;
    let surface =
        analyze_shipping(&shipping, request.shipping, request.relation)?;
    validate_surface(&surface, request.relation)?;
    let witness_types =
        analyze_consumer(&consumer, request.consumer, request.relation)?;

    Ok(ProductSourceObservation {
        public_items: surface.public_items.into_iter().collect(),
        derive_checks: surface.derive_checks.into_iter().collect(),
        witness_types: witness_types.into_iter().collect(),
        shipping_sources: request.shipping_sources.iter().cloned().collect(),
        consumer_sources: request.consumer_sources.iter().cloned().collect(),
    })
}

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

fn validate_relation(
    request: &AnalysisRequest<'_>,
) -> Result<(), ProductSourceError> {
    validate_closure(
        request.shipping,
        ClosureKind::Shipping,
        ArchitecturalRole::Foundation,
        SourceUse::Production,
    )?;
    validate_closure(
        request.consumer,
        ClosureKind::Validation,
        ArchitecturalRole::KernelInterface,
        SourceUse::Test,
    )?;
    if request.relation.shipping_closure != request.shipping.id
        || request.relation.consumer_closure != request.consumer.id
    {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_RELATION",
            "public relation does not name the supplied exact closures",
        ));
    }
    Ok(())
}

fn validate_closure(
    closure: &Closure,
    kind: ClosureKind,
    role: ArchitecturalRole,
    source_use: SourceUse,
) -> Result<(), ProductSourceError> {
    if closure.kind != kind
        || closure.architectural_role != role
        || closure.source_use != source_use
    {
        return Err(ProductSourceError::new(
            "PRODUCT_SOURCE_ROLE",
            format!("closure {} has the wrong role or source use", closure.id),
        ));
    }
    if closure.target.kind.as_slice() != ["lib"]
        || closure.target.crate_types.as_slice() != ["lib"]
        || closure.cargo.profile != "release"
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
    Shipping,
    Consumer,
}

fn validate_source_profile(
    sources: &SourceSet,
    closure: &Closure,
    relation: &PublicConsumerRelation,
    mode: SourceMode,
) -> Result<(), ProductSourceError> {
    let names = declared_names(sources);
    let public_types = registered_public_types(relation);
    for (path, source) in &sources.files {
        let root = path == &closure.target.src_path;
        validate_file_attributes(&source.syntax, path, root)?;
        validate_unsafe_syntax(&source.syntax, path)?;
        validate_closed_syntax(&source.syntax, path)?;
        validate_imports(&source.syntax, path, mode)?;
        validate_type_paths(&source.syntax, path, mode, &names, &public_types)?;
    }
    Ok(())
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
) -> Result<(), ProductSourceError> {
    let mut guard = SyntaxGuard { path, error: None };
    for item in &syntax.items {
        guard.visit_item(item);
    }
    guard.error.map_or(Ok(()), Err)
}

struct SyntaxGuard<'a> {
    path: &'a Path,
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
        if public(&item.vis) {
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
) -> Result<(), ProductSourceError> {
    let mut visitor = ImportVisitor { path, mode, error: None };
    visitor.visit_file(syntax);
    visitor.error.map_or(Ok(()), Err)
}

struct ImportVisitor<'a> {
    path: &'a Path,
    mode: SourceMode,
    error: Option<ProductSourceError>,
}

impl<'ast> Visit<'ast> for ImportVisitor<'_> {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if self.error.is_some() {
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
        if imports.iter().any(|parts| !allowed_import(parts, self.mode)) {
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

fn allowed_import(parts: &[String], mode: SourceMode) -> bool {
    match mode {
        SourceMode::Shipping => {
            parts == ["core", "fmt"]
                || parts.first().is_some_and(|part| part == "crate")
        }
        SourceMode::Consumer => parts == ["core", "fmt", "Debug"],
    }
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
    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        self.names.insert(item.ident.to_string());
        visit::visit_item_struct(self, item);
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

fn validate_type_paths(
    syntax: &File,
    path: &Path,
    mode: SourceMode,
    names: &BTreeSet<String>,
    public_types: &BTreeSet<String>,
) -> Result<(), ProductSourceError> {
    let mut visitor =
        TypePathGuard { path, mode, names, public_types, error: None };
    visitor.visit_file(syntax);
    visitor.error.map_or(Ok(()), Err)
}

struct TypePathGuard<'a> {
    path: &'a Path,
    mode: SourceMode,
    names: &'a BTreeSet<String>,
    public_types: &'a BTreeSet<String>,
    error: Option<ProductSourceError>,
}

impl<'ast> Visit<'ast> for TypePathGuard<'_> {
    fn visit_type_path(&mut self, ty: &'ast syn::TypePath) {
        if self.error.is_some() {
            return;
        }
        if !allowed_type_path(ty, self.mode, self.names, self.public_types) {
            self.error = Some(ProductSourceError::at(
                "PRODUCT_TYPE_PATH_UNAVAILABLE",
                self.path,
                "type path is outside the closed Product profile",
            ));
            return;
        }
        visit::visit_type_path(self, ty);
    }
}

fn allowed_type_path(
    ty: &syn::TypePath,
    mode: SourceMode,
    names: &BTreeSet<String>,
    public_types: &BTreeSet<String>,
) -> bool {
    if ty.qself.is_some() {
        return false;
    }
    let parts = ty
        .path
        .segments
        .iter()
        .map(|part| part.ident.to_string())
        .collect::<Vec<_>>();
    let Some(first) = parts.first().map(String::as_str) else { return false };
    if simple_type_root(first) || names.contains(first) {
        return true;
    }
    match mode {
        SourceMode::Shipping => allowed_shipping_type(&parts),
        SourceMode::Consumer => allowed_consumer_type(&parts, public_types),
    }
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
    parts.first().is_some_and(|part| part == "crate")
        || parts == ["fmt", "Formatter"]
        || parts == ["fmt", "Result"]
        || parts == ["std", "error", "Error"]
}

fn allowed_consumer_type(
    parts: &[String],
    public_types: &BTreeSet<String>,
) -> bool {
    if parts == ["Debug"] {
        return true;
    }
    if parts.first().is_none_or(|part| part != FOUNDATION_CRATE) {
        return false;
    }
    let public_path = parts[1..].join("::");
    public_types.contains(&public_path)
}

struct Surface {
    public_items: BTreeSet<PublicItem>,
    derive_checks: BTreeSet<DeriveCheck>,
    public_types: BTreeSet<String>,
    local_types: BTreeSet<String>,
    impls: Vec<ImplCandidate>,
}

impl Surface {
    const fn new() -> Self {
        Self {
            public_items: BTreeSet::new(),
            derive_checks: BTreeSet::new(),
            public_types: BTreeSet::new(),
            local_types: BTreeSet::new(),
            impls: Vec::new(),
        }
    }
}

struct ImplCandidate {
    owner: String,
    generic: bool,
    functions: Vec<String>,
    consts: Vec<String>,
}

fn analyze_shipping(
    sources: &SourceSet,
    closure: &Closure,
    relation: &PublicConsumerRelation,
) -> Result<Surface, ProductSourceError> {
    validate_source_profile(sources, closure, relation, SourceMode::Shipping)?;
    let mut surface = Surface::new();
    let mut visited = BTreeSet::new();
    walk_shipping_source(
        sources,
        &closure.target.src_path,
        &[],
        true,
        true,
        &mut visited,
        &mut surface,
    )?;
    validate_visited_sources(sources, &visited)?;
    finish_inherent_items(&mut surface)?;
    Ok(surface)
}

fn walk_shipping_source(
    sources: &SourceSet,
    path: &Path,
    prefix: &[String],
    exposed: bool,
    target_root: bool,
    visited: &mut BTreeSet<PathBuf>,
    surface: &mut Surface,
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
    walk_shipping_items(
        sources,
        &syntax.items,
        prefix,
        exposed,
        &base,
        visited,
        surface,
    )
}

fn walk_shipping_items(
    sources: &SourceSet,
    items: &[Item],
    prefix: &[String],
    exposed: bool,
    base: &Path,
    visited: &mut BTreeSet<PathBuf>,
    surface: &mut Surface,
) -> Result<(), ProductSourceError> {
    for item in items {
        if let Item::Mod(module) = item {
            walk_shipping_module(
                sources, module, prefix, exposed, base, visited, surface,
            )?;
            continue;
        }
        collect_surface_item(item, prefix, exposed, surface)?;
    }
    Ok(())
}

fn walk_shipping_module(
    sources: &SourceSet,
    module: &syn::ItemMod,
    prefix: &[String],
    exposed: bool,
    base: &Path,
    visited: &mut BTreeSet<PathBuf>,
    surface: &mut Surface,
) -> Result<(), ProductSourceError> {
    let name = module.ident.to_string();
    let module_prefix = append_path(prefix, &name);
    let module_exposed = exposed && public(&module.vis);
    if module_exposed {
        let module_path = qualified(&module_prefix);
        insert_public_item(surface, &module_path, PublicItemKind::Module)?;
    }
    if let Some((_, items)) = &module.content {
        return walk_shipping_items(
            sources,
            items,
            &module_prefix,
            module_exposed,
            &base.join(name),
            visited,
            surface,
        );
    }
    let source = resolve_external_module(sources, base, &name)?;
    walk_shipping_source(
        sources,
        &source,
        &module_prefix,
        module_exposed,
        false,
        visited,
        surface,
    )
}

fn collect_surface_item(
    item: &Item,
    prefix: &[String],
    exposed: bool,
    surface: &mut Surface,
) -> Result<(), ProductSourceError> {
    match item {
        Item::Struct(item) => collect_public_type(
            &item.ident.to_string(),
            &item.attrs,
            &item.vis,
            prefix,
            exposed,
            surface,
        ),
        Item::Enum(item) => collect_public_type(
            &item.ident.to_string(),
            &item.attrs,
            &item.vis,
            prefix,
            exposed,
            surface,
        ),
        Item::Trait(item) => collect_public_trait(
            &item.ident.to_string(),
            &item.vis,
            prefix,
            exposed,
            surface,
        ),
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
        _ => Ok(()),
    }
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
    if !public_type && !derives.is_empty() {
        return Err(ProductSourceError::new(
            "PRODUCT_DERIVE_UNAVAILABLE",
            format!("private type {path} has an unregistrable derive"),
        ));
    }
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
    name: &str,
    visibility: &Visibility,
    prefix: &[String],
    exposed: bool,
    surface: &mut Surface,
) -> Result<(), ProductSourceError> {
    let path = qualified(&append_path(prefix, name));
    surface.local_types.insert(path.clone());
    if exposed && public(visibility) {
        insert_public_item(surface, &path, PublicItemKind::Trait)?;
    }
    Ok(())
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
    let owner = inherent_owner(&item.self_ty, prefix)?;
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
    surface.impls.push(ImplCandidate {
        owner,
        generic: !item.generics.params.is_empty(),
        functions,
        consts,
    });
    Ok(())
}

fn inherent_owner(
    ty: &Type,
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
    let mut parts = path
        .path
        .segments
        .iter()
        .map(|part| {
            if matches!(part.arguments, PathArguments::None) {
                Ok(part.ident.to_string())
            } else {
                Err(ProductSourceError::new(
                    "PRODUCT_INHERENT_IMPL_UNAVAILABLE",
                    "generic inherent impl target is unavailable",
                ))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
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
        if implementation.generic {
            return Err(ProductSourceError::new(
                "PRODUCT_INHERENT_IMPL_UNAVAILABLE",
                "generic public inherent impl is unavailable",
            ));
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
) -> Result<BTreeSet<String>, ProductSourceError> {
    validate_source_profile(sources, closure, relation, SourceMode::Consumer)?;
    validate_module_tree(sources, &closure.target.src_path)?;
    let mut visitor = WitnessVisitor::new(relation);
    for (path, source) in &sources.files {
        visitor.path = path;
        visitor.visit_file(&source.syntax);
    }
    visitor.finish()
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
    expected: BTreeSet<String>,
    witnesses: BTreeSet<String>,
    call_count: usize,
    helper_count: usize,
    error: Option<ProductSourceError>,
}

impl WitnessVisitor<'_> {
    fn new(relation: &PublicConsumerRelation) -> Self {
        Self {
            path: Path::new(""),
            expected: registered_public_types(relation),
            witnesses: BTreeSet::new(),
            call_count: 0,
            helper_count: 0,
            error: None,
        }
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
        if self.helper_count != 1 {
            return Err(ProductSourceError::new(
                "PRODUCT_CONSUMER_DERIVE_SET",
                "consumer requires exactly one derive_witness helper",
            ));
        }
        if self.call_count != self.witnesses.len() {
            return Err(ProductSourceError::new(
                "PRODUCT_CONSUMER_DERIVE_SET",
                "consumer repeats one derive witness",
            ));
        }
        if self.witnesses != self.expected {
            return Err(ProductSourceError::new(
                "PRODUCT_CONSUMER_DERIVE_SET",
                set_difference_message(
                    "derive witness",
                    &self.expected,
                    &self.witnesses,
                ),
            ));
        }
        Ok(self.witnesses)
    }
}

impl<'ast> Visit<'ast> for WitnessVisitor<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if item.sig.ident == DERIVE_WITNESS {
            self.observe_helper(item);
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
        if last.ident != DERIVE_WITNESS {
            visit::visit_expr_call(self, call);
            return;
        }
        self.call_count = self.call_count.saturating_add(1);
        match explicit_witness_type(&function.path) {
            Ok(public_type) => {
                self.witnesses.insert(public_type);
            }
            Err(message) => self.fail(message),
        }
        visit::visit_expr_call(self, call);
    }
}

impl WitnessVisitor<'_> {
    fn observe_helper(&mut self, item: &syn::ItemFn) {
        self.helper_count = self.helper_count.saturating_add(1);
        if let Err(message) = validate_witness_helper(item) {
            self.fail(message);
        }
    }
}

fn validate_witness_helper(item: &syn::ItemFn) -> Result<(), String> {
    let signature = &item.sig;
    if signature.constness.is_none()
        || signature.asyncness.is_some()
        || !matches!(signature.safety, syn::Safety::Default)
        || signature.abi.is_some()
        || signature.variadic.is_some()
        || !signature.inputs.is_empty()
        || !returns_bool(&signature.output)
    {
        return Err("derive_witness has the wrong function shape".to_owned());
    }
    let parameters = signature.generics.params.iter().collect::<Vec<_>>();
    let [syn::GenericParam::Type(parameter)] = parameters.as_slice() else {
        return Err("derive_witness requires one type parameter".to_owned());
    };
    if parameter.ident != "T" || parameter.default.is_some() {
        return Err("derive_witness type parameter must be T".to_owned());
    }
    let mut bounds = parse_trait_bounds(&parameter.bounds)?;
    collect_where_bounds(&signature.generics, &mut bounds)?;
    let expected = ["Clone", "Copy", "Debug", "Eq", "PartialEq"]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if bounds != expected {
        return Err("derive_witness requires the exact five bounds".to_owned());
    }
    Ok(())
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
        if bound.path.leading_colon.is_some()
            || bound.path.segments.len() != 1
            || bound.modifiers.require_empty().is_err()
            || bound.maybe.is_some()
            || bound.lifetimes.is_some()
        {
            return Err("derive_witness has a qualified bound".to_owned());
        }
        let name = bound.path.segments[0].ident.to_string();
        if !output.insert(name) {
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

fn explicit_witness_type(path: &SynPath) -> Result<String, String> {
    if path.leading_colon.is_some() || path.segments.len() != 1 {
        return Err(
            "derive_witness call must use its local exact name".to_owned()
        );
    }
    let segment = &path.segments[0];
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(
            "derive_witness call requires explicit turbofish".to_owned()
        );
    };
    if arguments.colon2_token.is_none() || arguments.args.len() != 1 {
        return Err(
            "derive_witness call requires one turbofish type".to_owned()
        );
    }
    let Some(GenericArgument::Type(Type::Path(ty))) = arguments.args.first()
    else {
        return Err("derive_witness argument is not a type path".to_owned());
    };
    external_public_type(ty)
}

fn external_public_type(ty: &syn::TypePath) -> Result<String, String> {
    if ty.qself.is_some() || ty.path.leading_colon.is_some() {
        return Err("derive witness type must be an ordinary path".to_owned());
    }
    let parts = ty
        .path
        .segments
        .iter()
        .map(|part| {
            if matches!(part.arguments, PathArguments::None) {
                Ok(part.ident.to_string())
            } else {
                Err("derive witness type cannot have arguments".to_owned())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parts.len() != 3 || parts[0] != FOUNDATION_CRATE {
        return Err(
            "derive witness must use a full Foundation public path".to_owned()
        );
    }
    Ok(parts[1..].join("::"))
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

    use super::{AnalysisLimits, AnalysisRequest, ProductSourceError, analyze};
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
    const LIMITS: AnalysisLimits = AnalysisLimits {
        source_files: 16,
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
        assert_eq!(observation.public_items.len(), 96);
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
            "derive_witness::<reovim_foundation::bound::ArithmeticRule>()",
            "true",
            "PRODUCT_CONSUMER_DERIVE_SET",
        );
        source_mutant(
            CONSUMER,
            "T: Clone + Copy + Debug + PartialEq + Eq,",
            "T: Clone + Copy + Debug + Eq,",
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
            "derive_witness::<reovim_foundation::bound::ArithmeticRule>()",
            "derive_witness::<reovim_foundation::bound::BoundError>()",
            "PRODUCT_CONSUMER_DERIVE_SET",
        );
    }

    fn fixture() -> Fixture {
        let repository = repository_root();
        let root = fixture_root();
        let registry = read_registry(&repository);
        let shipping =
            SHIPPING.iter().map(PathBuf::from).collect::<BTreeSet<_>>();
        let consumer = BTreeSet::from([PathBuf::from(CONSUMER)]);
        let mut fixture = Fixture {
            root,
            registry,
            inventory: Vec::new(),
            shipping,
            consumer,
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
        let (role, source_use) = if path.starts_with("kernel/foundation") {
            (ArchitecturalRole::Foundation, SourceUse::Production)
        } else {
            (ArchitecturalRole::KernelInterface, SourceUse::Test)
        };
        fixture.inventory.push(classified_file(path, bytes, role, source_use));
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
            relation: &fixture.registry.public_consumer_relations[0],
            shipping_sources: &fixture.shipping,
            consumer_sources: &fixture.consumer,
            inventory: &fixture.inventory,
            limits: LIMITS,
        })
    }

    fn source_mutant(path: &str, old: &str, new: &str, code: &'static str) {
        let mut fixture = fixture();
        replace_source(&mut fixture, path, old, new);
        assert_code(&fixture, code);
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
}
