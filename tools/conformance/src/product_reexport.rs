//! Narrow static proof for direct Foundation type re-exports.
//!
//! This resolver deliberately supports only an unconditional direct Cargo
//! dependency, one public external provider module, and direct type names.
//! Every other public `use` remains a typed static-proof gap.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use reovim_depgraph::{
    ArchitecturalRole, SourceOwner, SourceProvenance, SourceUse,
};
use syn::{
    Attribute, File, Item, ItemUse, Meta, Path as SynPath, PathArguments,
    Token, UseTree, Visibility,
    parse::Parser as _,
    punctuated::Punctuated,
    visit::{self, Visit},
};

use crate::{
    cargo::{Metadata, Package, ResolvedDependency, Target},
    finding::{Finding, Findings},
    product_closures::{
        BuiltinDerive, Closure, ClosureKind, PublicConsumerRelation,
        PublicItemKind, PublicReexport, Registry,
    },
    source::{ClassifiedFile, display_relative, read_bounded},
};

const GAP: &str = "UNSUPPORTED_REEXPORT_PROOF_GAP";
const GLOB_GAP: &str = "UNSUPPORTED_GLOB_REEXPORT_PROOF_GAP";
const FOUNDATION: ShippingIdentity = ShippingIdentity {
    package: "reovim-foundation",
    closure: "foundation-shipping",
    relation: "foundation-public-api",
    consumer: "foundation-consumer",
    role: ArchitecturalRole::Foundation,
};
const KERNEL_INTERFACE: ShippingIdentity = ShippingIdentity {
    package: "reovim-kernel-interface",
    closure: "kernel-interface-shipping",
    relation: "kernel-interface-public-api",
    consumer: "kernel-interface-consumer",
    role: ArchitecturalRole::KernelInterface,
};

#[derive(Clone, Copy)]
struct ShippingIdentity {
    package: &'static str,
    closure: &'static str,
    relation: &'static str,
    consumer: &'static str,
    role: ArchitecturalRole,
}

pub struct Request<'a> {
    pub root: &'a Path,
    pub profile: &'a str,
    pub metadata: &'a Metadata,
    pub inventory: &'a BTreeMap<PathBuf, &'a ClassifiedFile>,
    pub registry: &'a Registry,
    pub package_id: &'a str,
    pub source: &'a ClassifiedFile,
    pub syntax: &'a File,
}

#[derive(Clone, Copy)]
pub struct Limits {
    pub imports: usize,
    pub source_files: usize,
    pub source_file_bytes: u64,
    pub source_total_bytes: u64,
    pub syntax_nodes: usize,
}

pub struct Resolver {
    limits: Limits,
    imports: usize,
    source_bytes: u64,
    syntax_nodes: usize,
    sources: BTreeMap<PathBuf, CachedSource>,
    modules: BTreeMap<(PathBuf, String), Result<PathBuf, &'static str>>,
    reexports: BTreeMap<String, PublicReexport>,
}

enum CachedSource {
    Parsed(ParsedSource),
    Rejected(&'static str),
}

struct ParsedSource {
    syntax: File,
    surface: ProviderSurface,
}

impl Resolver {
    pub const fn new(limits: Limits) -> Self {
        Self {
            limits,
            imports: 0,
            source_bytes: 0,
            syntax_nodes: 0,
            sources: BTreeMap::new(),
            modules: BTreeMap::new(),
            reexports: BTreeMap::new(),
        }
    }

    pub fn finish(
        &self,
        metadata: &Metadata,
        registry: &Registry,
        findings: &mut Findings,
    ) {
        if let Err(message) =
            validate_complete_reexports(metadata, registry, &self.reexports)
        {
            findings.push(Finding::new(
                "public_type",
                GAP,
                Some("conformance/product-closures.json".to_owned()),
                message,
            ));
        }
    }

    fn record_reexports(
        &mut self,
        proved: Vec<PublicReexport>,
    ) -> Result<(), &'static str> {
        let mut batch = BTreeSet::new();
        let unique = proved.iter().all(|reexport| {
            batch.insert(reexport.public_type.as_str())
                && !self.reexports.contains_key(&reexport.public_type)
        });
        if !unique {
            return Err("public alias is declared more than once");
        }
        drop(batch);
        for reexport in proved {
            self.reexports.insert(reexport.public_type.clone(), reexport);
        }
        Ok(())
    }

    fn charge_imports(&mut self, count: usize) -> Result<(), &'static str> {
        let imports = self
            .imports
            .checked_add(count)
            .ok_or("direct re-export import count overflowed")?;
        if imports > self.limits.imports {
            return Err("direct re-export import limit is exhausted");
        }
        self.imports = imports;
        Ok(())
    }

    fn charge_source_bytes(
        &mut self,
        bytes: usize,
    ) -> Result<(), &'static str> {
        let bytes = u64::try_from(bytes)
            .map_err(|_| "auxiliary source byte count overflowed")?;
        let total = self
            .source_bytes
            .checked_add(bytes)
            .ok_or("auxiliary source byte count overflowed")?;
        if total > self.limits.source_total_bytes {
            return Err("auxiliary source byte limit is exhausted");
        }
        self.source_bytes = total;
        Ok(())
    }

    fn charge_syntax_nodes(
        &mut self,
        nodes: usize,
    ) -> Result<(), &'static str> {
        let total = self
            .syntax_nodes
            .checked_add(nodes)
            .ok_or("auxiliary syntax node count overflowed")?;
        if total > self.limits.syntax_nodes {
            return Err("aggregate auxiliary syntax node limit is exhausted");
        }
        self.syntax_nodes = total;
        Ok(())
    }

    fn provider_module(
        &mut self,
        request: &Request<'_>,
        root: &Path,
        root_file: &ClassifiedFile,
        name: &str,
    ) -> Result<PathBuf, &'static str> {
        let key = (root.to_path_buf(), name.to_owned());
        if !self.modules.contains_key(&key) {
            let result = resolve_cached_provider_module(
                self, request, root, root_file, name,
            );
            self.modules.insert(key.clone(), result);
        }
        self.modules
            .get(&key)
            .expect("provider module resolution was inserted")
            .clone()
    }

    fn prepare_source(
        &mut self,
        request: &Request<'_>,
        file: &ClassifiedFile,
    ) -> Result<(), &'static str> {
        if !self.sources.contains_key(&file.path) {
            let source = self.read_source(request, file);
            self.sources.insert(file.path.clone(), source);
        }
        match self.sources.get(&file.path) {
            Some(CachedSource::Parsed(_)) => Ok(()),
            Some(CachedSource::Rejected(message)) => Err(message),
            None => Err("auxiliary source cache insertion failed"),
        }
    }

    fn source_syntax<'a>(
        &'a mut self,
        request: &Request<'_>,
        file: &ClassifiedFile,
    ) -> Result<&'a File, &'static str> {
        self.prepare_source(request, file)?;
        match self.sources.get(&file.path) {
            Some(CachedSource::Parsed(source)) => Ok(&source.syntax),
            _ => Err("auxiliary source cache lost parsed syntax"),
        }
    }

    fn provider_facts(
        &self,
        path: &Path,
        name: &str,
    ) -> Result<BTreeSet<BuiltinDerive>, &'static str> {
        let Some(CachedSource::Parsed(source)) = self.sources.get(path) else {
            return Err("provider source facts are absent from the cache");
        };
        source.surface.type_facts(name)
    }

    fn read_source(
        &mut self,
        request: &Request<'_>,
        file: &ClassifiedFile,
    ) -> CachedSource {
        if self.sources.len() >= self.limits.source_files {
            return CachedSource::Rejected(
                "auxiliary source file limit is exhausted",
            );
        }
        let Ok(bytes) = read_bounded(
            &request.root.join(&file.path),
            self.limits.source_file_bytes,
        ) else {
            return CachedSource::Rejected(
                "source could not be read within its bound",
            );
        };
        if let Err(message) = self.charge_source_bytes(bytes.len()) {
            return CachedSource::Rejected(message);
        }
        parse_cached_source(self, file, &bytes)
    }
}

fn resolve_cached_provider_module(
    resolver: &mut Resolver,
    request: &Request<'_>,
    root: &Path,
    root_file: &ClassifiedFile,
    name: &str,
) -> Result<PathBuf, &'static str> {
    let syntax = resolver.source_syntax(request, root_file)?;
    public_external_module(request.inventory, root, syntax, name)
}

pub fn check(
    request: &Request<'_>,
    resolver: &mut Resolver,
    findings: &mut Findings,
) {
    let has_direct_use = request
        .syntax
        .items
        .iter()
        .any(|item| matches!(item, Item::Use(item) if public(&item.vis)));
    let consumer = has_direct_use.then(|| resolve_consumer(request, resolver));
    for item in &request.syntax.items {
        match item {
            Item::Use(item) if public(&item.vis) => {
                let consumer = consumer
                    .as_ref()
                    .expect("direct public use established above")
                    .as_ref()
                    .map_err(|message| *message);
                check_direct_use(request, resolver, consumer, item, findings);
            }
            Item::Use(_) => {}
            _ => {
                UnsupportedUseVisitor { path: &request.source.path, findings }
                    .visit_item(item);
            }
        }
    }
}

fn check_direct_use(
    request: &Request<'_>,
    resolver: &mut Resolver,
    consumer: Result<&Consumer<'_>, &'static str>,
    item: &ItemUse,
    findings: &mut Findings,
) {
    let result = consumer
        .and_then(|consumer| {
            prove_direct_use(request, resolver, consumer, item)
        })
        .and_then(|proved| resolver.record_reexports(proved));
    if let Err(message) = result {
        push_gap(&request.source.path, &item.tree, message, findings);
    }
}

fn prove_direct_use(
    request: &Request<'_>,
    resolver: &mut Resolver,
    consumer: &Consumer<'_>,
    item: &ItemUse,
) -> Result<Vec<PublicReexport>, &'static str> {
    let import_count = direct_name_count(item)?;
    resolver.charge_imports(import_count)?;
    let direct = parse_direct_use(item)?;
    let provider = resolve_provider(request, resolver, consumer, &direct)?;
    let mut reexports = Vec::with_capacity(direct.names.len());
    for name in direct.names {
        let derives = resolver.provider_facts(&provider.source, &name)?;
        let provider_path = format!("{}::{name}", direct.module);
        let alias_path = qualify(&consumer.module, &name);
        verify_registered_type(provider.relation, &provider_path, &derives)?;
        verify_registered_type(consumer.relation, &alias_path, &derives)?;
        reexports.push(PublicReexport {
            public_type: alias_path,
            provider_relation: provider.relation.id.clone(),
            provider_type: provider_path,
        });
    }
    Ok(reexports)
}

struct Consumer<'a> {
    package: &'a Package,
    relation: &'a PublicConsumerRelation,
    module: Vec<String>,
}

fn resolve_consumer<'a>(
    request: &'a Request<'_>,
    resolver: &mut Resolver,
) -> Result<Consumer<'a>, &'static str> {
    let package = request
        .metadata
        .packages
        .get(request.package_id)
        .ok_or("consumer package is absent from Cargo metadata")?;
    require_workspace_package(request.metadata, package)?;
    require_consumer_edition(package)?;
    require_source(request.source, ArchitecturalRole::KernelInterface)?;
    let target = exact_library_target(package, None)?;
    let root = metadata_path(request.root, &target.src_path)?;
    let module =
        public_source_module(request, resolver, &root, &request.source.path)?;
    let closure = shipping_closure(request, package, target, KERNEL_INTERFACE)?;
    let relation =
        shipping_relation(request.registry, &closure.id, KERNEL_INTERFACE)?;
    Ok(Consumer { package, relation, module })
}

struct Provider<'a> {
    relation: &'a PublicConsumerRelation,
    source: PathBuf,
}

fn resolve_provider<'a>(
    request: &'a Request<'_>,
    resolver: &mut Resolver,
    consumer: &Consumer<'a>,
    import: &DirectUse,
) -> Result<Provider<'a>, &'static str> {
    let dependency = exact_dependency(
        request.metadata,
        &consumer.package.id,
        &import.dependency,
    )?;
    let package = request
        .metadata
        .packages
        .get(&dependency.package)
        .ok_or("provider package is absent from Cargo metadata")?;
    require_workspace_package(request.metadata, package)?;
    let target = exact_library_target(package, Some(&dependency.name))?;
    let root = metadata_path(request.root, &target.src_path)?;
    let root_file = inventory_file(request, &root)?;
    require_source(root_file, ArchitecturalRole::Foundation)?;
    let module =
        resolver.provider_module(request, &root, root_file, &import.module)?;
    let module_file = inventory_file(request, &module)?;
    require_source(module_file, ArchitecturalRole::Foundation)?;
    resolver.prepare_source(request, module_file)?;
    let closure = shipping_closure(request, package, target, FOUNDATION)?;
    let relation =
        shipping_relation(request.registry, &closure.id, FOUNDATION)?;
    Ok(Provider { relation, source: module })
}

fn exact_dependency<'a>(
    metadata: &'a Metadata,
    package_id: &str,
    name: &str,
) -> Result<&'a ResolvedDependency, &'static str> {
    let node = metadata
        .nodes
        .get(package_id)
        .ok_or("consumer dependency node is absent")?;
    let mut matches =
        node.dependencies.iter().filter(|dependency| dependency.name == name);
    let dependency =
        matches.next().ok_or("named direct dependency is absent")?;
    if matches.next().is_some() {
        return Err("named direct dependency is ambiguous");
    }
    let exact_kind = dependency.kinds.len() == 1
        && dependency.kinds[0].kind == "normal"
        && dependency.kinds[0].target.is_none();
    if !exact_kind {
        return Err("dependency is not one unconditional normal edge");
    }
    Ok(dependency)
}

fn require_workspace_package(
    metadata: &Metadata,
    package: &Package,
) -> Result<(), &'static str> {
    if package.source.is_some()
        || !metadata.workspace_members.contains(&package.id)
    {
        return Err("package is not a source-null workspace member");
    }
    Ok(())
}

fn require_consumer_edition(package: &Package) -> Result<(), &'static str> {
    if package.edition != "2024" {
        return Err("consumer package is not exact Rust edition 2024");
    }
    Ok(())
}

fn exact_library_target<'a>(
    package: &'a Package,
    required_name: Option<&str>,
) -> Result<&'a Target, &'static str> {
    let mut targets = package.targets.iter().filter(|target| {
        target.kind.as_slice() == ["lib"]
            && target.crate_types.as_slice() == ["lib"]
            && required_name.is_none_or(|name| target.name == name)
    });
    let target = targets.next().ok_or("exact library target is absent")?;
    if targets.next().is_some() {
        return Err("exact library target is ambiguous");
    }
    Ok(target)
}

fn public_source_module(
    request: &Request<'_>,
    resolver: &mut Resolver,
    root: &Path,
    source: &Path,
) -> Result<Vec<String>, &'static str> {
    let root_file = inventory_file(request, root)?;
    require_source(root_file, ArchitecturalRole::KernelInterface)?;
    if source == root {
        return Ok(Vec::new());
    }
    let syntax = resolver.source_syntax(request, root_file)?;
    let mut modules = Vec::new();
    for item in &syntax.items {
        let Item::Mod(module) = item else { continue };
        if module.content.is_some() {
            continue;
        }
        let module_path = resolve_root_module_source(
            request.inventory,
            root,
            &module.ident.to_string(),
        )?;
        if module_path.as_deref() == Some(source) {
            modules.push(module);
        }
    }
    let [module] = modules.as_slice() else {
        return Err("re-export source is not one direct library module");
    };
    if !public(&module.vis) || !module.attrs.is_empty() {
        return Err("re-export source module is not unconditional and public");
    }
    Ok(vec![module.ident.to_string()])
}

fn public_external_module(
    inventory: &BTreeMap<PathBuf, &ClassifiedFile>,
    root: &Path,
    syntax: &File,
    name: &str,
) -> Result<PathBuf, &'static str> {
    let mut modules = syntax.items.iter().filter_map(|item| {
        let Item::Mod(module) = item else { return None };
        (module.ident == name).then_some(module)
    });
    let module =
        modules.next().ok_or("provider module declaration is absent")?;
    if modules.next().is_some() {
        return Err("provider module declaration is ambiguous");
    }
    if !public(&module.vis)
        || !module.attrs.is_empty()
        || module.content.is_some()
    {
        return Err(
            "provider module is not unconditional public external code",
        );
    }
    resolve_root_module_source(inventory, root, name)?
        .ok_or("provider module source is absent")
}

fn resolve_root_module_source(
    inventory: &BTreeMap<PathBuf, &ClassifiedFile>,
    root: &Path,
    name: &str,
) -> Result<Option<PathBuf>, &'static str> {
    let base = root.parent().unwrap_or_else(|| Path::new(""));
    let candidates =
        [base.join(format!("{name}.rs")), base.join(name).join("mod.rs")];
    let matches = candidates
        .into_iter()
        .filter(|path| inventory.contains_key(path))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Ok(None),
        [path] => Ok(Some(path.clone())),
        _ => Err("module source is ambiguous"),
    }
}

fn shipping_closure<'a>(
    request: &'a Request<'_>,
    package: &Package,
    target: &Target,
    identity: ShippingIdentity,
) -> Result<&'a Closure, &'static str> {
    require_package_identity(&package.name, identity)?;
    let manifest = metadata_path(request.root, &package.manifest_path)?;
    let source = metadata_path(request.root, &target.src_path)?;
    let workspace = metadata_path(
        request.root,
        &request.metadata.workspace_root.join("Cargo.toml"),
    )?;
    let mut closures = request.registry.closures.iter().filter(|closure| {
        closure.id == identity.closure
            && closure.kind == ClosureKind::Shipping
            && closure.architectural_role == identity.role
            && closure.source_use == SourceUse::Production
            && closure.workspace.manifest == workspace
            && closure.package.name == identity.package
            && closure.package.manifest == manifest
            && closure.target.name == target.name
            && closure.target.src_path == source
            && closure.target.kind == target.kind
            && closure.target.crate_types == target.crate_types
            && closure.cargo.profile == request.profile
    });
    let closure =
        closures.next().ok_or("source-derived shipping closure is absent")?;
    if closures.next().is_some() {
        return Err("source-derived shipping closure is ambiguous");
    }
    Ok(closure)
}

fn shipping_relation<'a>(
    registry: &'a Registry,
    shipping: &str,
    identity: ShippingIdentity,
) -> Result<&'a PublicConsumerRelation, &'static str> {
    if shipping != identity.closure {
        return Err("shipping closure is outside the admitted identity");
    }
    let mut relations =
        registry.public_consumer_relations.iter().filter(|relation| {
            relation.id == identity.relation
                && relation.shipping_closure == identity.closure
                && relation.consumer_closure == identity.consumer
        });
    let relation = relations
        .next()
        .ok_or("shipping public-consumer relation is absent")?;
    if relations.next().is_some() {
        return Err("shipping public-consumer relation is ambiguous");
    }
    Ok(relation)
}

fn validate_complete_reexports(
    metadata: &Metadata,
    registry: &Registry,
    actual: &BTreeMap<String, PublicReexport>,
) -> Result<(), &'static str> {
    if kernel_interface_package(metadata)?.is_none() {
        return Ok(());
    }
    let foundation =
        shipping_relation(registry, FOUNDATION.closure, FOUNDATION)?;
    if !foundation.reexports.is_empty() {
        return Err("Foundation relation contains a public re-export origin");
    }
    let relation = shipping_relation(
        registry,
        KERNEL_INTERFACE.closure,
        KERNEL_INTERFACE,
    )?;
    exact_reexports(&relation.reexports, actual)
}

fn kernel_interface_package(
    metadata: &Metadata,
) -> Result<Option<&Package>, &'static str> {
    let mut packages = metadata
        .packages
        .values()
        .filter(|package| package.name == KERNEL_INTERFACE.package);
    let Some(package) = packages.next() else {
        return Ok(None);
    };
    if packages.next().is_some() {
        return Err("Kernel Interface package identity is ambiguous");
    }
    require_workspace_package(metadata, package)?;
    require_consumer_edition(package)?;
    Ok(Some(package))
}

fn exact_reexports(
    expected: &[PublicReexport],
    actual: &BTreeMap<String, PublicReexport>,
) -> Result<(), &'static str> {
    let expected_count = expected.len();
    let actual_count = actual.len();
    let expected = expected.iter().cloned().collect::<BTreeSet<_>>();
    let actual = actual.values().cloned().collect::<BTreeSet<_>>();
    if expected.len() != expected_count || actual.len() != actual_count {
        return Err("direct re-export mapping contains a duplicate identity");
    }
    if expected.difference(&actual).next().is_some() {
        return Err(
            "registered direct Foundation re-export is absent or retargeted",
        );
    }
    if actual.difference(&expected).next().is_some() {
        return Err(concat!(
            "source-derived direct Foundation re-export is ",
            "unregistered"
        ));
    }
    Ok(())
}

fn require_package_identity(
    package: &str,
    identity: ShippingIdentity,
) -> Result<(), &'static str> {
    if package != identity.package {
        return Err("package is outside the admitted re-export identity");
    }
    Ok(())
}

fn verify_registered_type(
    relation: &PublicConsumerRelation,
    path: &str,
    expected: &BTreeSet<BuiltinDerive>,
) -> Result<(), &'static str> {
    let mut items =
        relation.public_items.iter().filter(|item| item.path == path);
    let item = items.next().ok_or("registered public type is absent")?;
    if items.next().is_some() || item.kind != PublicItemKind::Type {
        return Err("registered public item identity or kind differs");
    }
    let checks = relation
        .derive_checks
        .iter()
        .filter(|check| check.public_type == path)
        .collect::<Vec<_>>();
    let actual =
        checks.iter().map(|check| check.derive).collect::<BTreeSet<_>>();
    if checks.len() != actual.len() || &actual != expected {
        return Err("registered built-in derive facts differ from source");
    }
    Ok(())
}

struct ProviderSurface {
    types: BTreeMap<String, Result<BTreeSet<BuiltinDerive>, &'static str>>,
}

impl ProviderSurface {
    fn from_syntax(syntax: &File) -> Self {
        let mut types = BTreeMap::new();
        for (name, fact) in syntax.items.iter().filter_map(provider_item_fact) {
            insert_provider_fact(&mut types, name, fact);
        }
        Self { types }
    }

    fn type_facts(
        &self,
        name: &str,
    ) -> Result<BTreeSet<BuiltinDerive>, &'static str> {
        self.types
            .get(name)
            .ok_or("provider public type definition is absent")?
            .clone()
    }
}

type ProviderFact = Result<BTreeSet<BuiltinDerive>, &'static str>;

fn provider_item_fact(item: &Item) -> Option<(String, ProviderFact)> {
    match item {
        Item::Struct(item) => Some((
            item.ident.to_string(),
            provider_definition_facts(&item.vis, &item.attrs),
        )),
        Item::Enum(item) => Some((
            item.ident.to_string(),
            provider_definition_facts(&item.vis, &item.attrs),
        )),
        Item::Type(item) => {
            Some((item.ident.to_string(), Err("provider item is a type alias")))
        }
        Item::Trait(item) => Some((
            item.ident.to_string(),
            Err("provider item kind is not a type"),
        )),
        Item::Union(item) => Some((
            item.ident.to_string(),
            Err("provider item kind is unsupported"),
        )),
        _ => None,
    }
}

fn provider_definition_facts(
    visibility: &Visibility,
    attributes: &[Attribute],
) -> ProviderFact {
    if !public(visibility) {
        return Err("provider type is private");
    }
    parse_derives(attributes)
}

fn insert_provider_fact(
    types: &mut BTreeMap<String, ProviderFact>,
    name: String,
    fact: ProviderFact,
) {
    if types.insert(name.clone(), fact).is_some() {
        types.insert(name, Err("provider public type definition is not exact"));
    }
}

#[cfg(test)]
fn provider_type_facts(
    syntax: &File,
    name: &str,
) -> Result<BTreeSet<BuiltinDerive>, &'static str> {
    ProviderSurface::from_syntax(syntax).type_facts(name)
}

fn parse_derives(
    attributes: &[Attribute],
) -> Result<BTreeSet<BuiltinDerive>, &'static str> {
    let mut derives = BTreeSet::new();
    for attribute in attributes {
        if attribute.path().is_ident("doc") {
            continue;
        }
        if !attribute.path().is_ident("derive") {
            return Err("provider type has an unsupported attribute");
        }
        let Meta::List(list) = &attribute.meta else {
            return Err("provider type has an invalid derive attribute");
        };
        let parser = Punctuated::<SynPath, Token![,]>::parse_terminated;
        let paths = parser
            .parse2(list.tokens.clone())
            .map_err(|_| "provider type has invalid derive syntax")?;
        for path in paths {
            let derive = builtin_derive(&path)?;
            insert_derive(&mut derives, derive)?;
        }
    }
    Ok(derives)
}

fn insert_derive(
    derives: &mut BTreeSet<BuiltinDerive>,
    derive: BuiltinDerive,
) -> Result<(), &'static str> {
    if !derives.insert(derive) {
        return Err("provider type repeats one built-in derive");
    }
    Ok(())
}

fn builtin_derive(path: &SynPath) -> Result<BuiltinDerive, &'static str> {
    if path.leading_colon.is_some()
        || path.segments.len() != 1
        || !matches!(path.segments[0].arguments, PathArguments::None)
    {
        return Err("provider type uses a qualified derive");
    }
    match path.segments[0].ident.to_string().as_str() {
        "Clone" => Ok(BuiltinDerive::Clone),
        "Copy" => Ok(BuiltinDerive::Copy),
        "Debug" => Ok(BuiltinDerive::Debug),
        "PartialEq" => Ok(BuiltinDerive::PartialEq),
        "Eq" => Ok(BuiltinDerive::Eq),
        "Hash" => Ok(BuiltinDerive::Hash),
        "Default" => Ok(BuiltinDerive::Default),
        _ => Err("provider type uses a non-built-in derive"),
    }
}

struct DirectUse {
    dependency: String,
    module: String,
    names: Vec<String>,
}

fn parse_direct_use(item: &ItemUse) -> Result<DirectUse, &'static str> {
    if item.leading_colon.is_none() || !item.attrs.is_empty() {
        return Err("re-export is not extern-prelude anchored direct syntax");
    }
    let UseTree::Path(dependency) = &item.tree else {
        return Err("re-export does not start with a dependency name");
    };
    let UseTree::Path(module) = dependency.tree.as_ref() else {
        return Err("re-export does not name one provider module");
    };
    let dependency = dependency.ident.to_string();
    let module_name = module.ident.to_string();
    if reserved_path(&dependency) || reserved_path(&module_name) {
        return Err("re-export does not name an external provider module");
    }
    let names = direct_names(module.tree.as_ref())?;
    Ok(DirectUse { dependency, module: module_name, names })
}

fn direct_name_count(item: &ItemUse) -> Result<usize, &'static str> {
    if item.leading_colon.is_none() || !item.attrs.is_empty() {
        return Err("re-export is not extern-prelude anchored direct syntax");
    }
    let UseTree::Path(dependency) = &item.tree else {
        return Err("re-export does not start with a dependency name");
    };
    let UseTree::Path(module) = dependency.tree.as_ref() else {
        return Err("re-export does not name one provider module");
    };
    match module.tree.as_ref() {
        UseTree::Name(name) if name.ident != "self" => Ok(1),
        UseTree::Group(group) => Ok(group.items.len()),
        _ => Err("re-export terminal is not a direct type name"),
    }
}

fn direct_names(tree: &UseTree) -> Result<Vec<String>, &'static str> {
    let names = match tree {
        UseTree::Name(name) if name.ident != "self" => {
            vec![name.ident.to_string()]
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .map(|tree| match tree {
                UseTree::Name(name) if name.ident != "self" => {
                    Ok(name.ident.to_string())
                }
                _ => Err("re-export terminal group is not flat direct names"),
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err("re-export terminal is not a direct type name"),
    };
    let unique = names.iter().collect::<BTreeSet<_>>();
    if names.is_empty() || names.len() != unique.len() {
        return Err("re-export repeats a direct type name");
    }
    Ok(names)
}

fn reserved_path(name: &str) -> bool {
    matches!(name, "crate" | "self" | "super")
}

fn qualify(module: &[String], name: &str) -> String {
    if module.is_empty() {
        name.to_owned()
    } else {
        format!("{}::{name}", module.join("::"))
    }
}

fn inventory_file<'a>(
    request: &'a Request<'_>,
    path: &Path,
) -> Result<&'a ClassifiedFile, &'static str> {
    request
        .inventory
        .get(path)
        .copied()
        .ok_or("source is absent from the digest-bound inventory")
}

fn require_source(
    file: &ClassifiedFile,
    role: ArchitecturalRole,
) -> Result<(), &'static str> {
    let expected = SourceOwner::Product(role);
    let exact = file.classification.owner == expected
        && file.classification.provenance == SourceProvenance::Authored
        && file.classification.source_use == SourceUse::Production;
    if !exact {
        return Err("source classification is not exact authored production");
    }
    Ok(())
}

fn parse_cached_source(
    resolver: &mut Resolver,
    file: &ClassifiedFile,
    bytes: &[u8],
) -> CachedSource {
    if crate::digest::bytes(bytes) != file.content_digest {
        return CachedSource::Rejected(
            "source bytes differ from the inventory digest",
        );
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return CachedSource::Rejected("source is not UTF-8");
    };
    let Ok(syntax) = syn::parse_file(text) else {
        return CachedSource::Rejected("source is not parseable Rust");
    };
    let nodes = match syntax_node_count(&syntax, resolver.limits.syntax_nodes) {
        Ok(nodes) => nodes,
        Err(message) => return CachedSource::Rejected(message),
    };
    if let Err(message) = resolver.charge_syntax_nodes(nodes) {
        return CachedSource::Rejected(message);
    }
    CachedSource::Parsed(ParsedSource {
        surface: ProviderSurface::from_syntax(&syntax),
        syntax,
    })
}

fn syntax_node_count(
    syntax: &File,
    limit: usize,
) -> Result<usize, &'static str> {
    let mut counter = SyntaxCounter { count: 0, limit, exceeded: false };
    counter.visit_file(syntax);
    if counter.exceeded {
        Err("auxiliary source exceeds its syntax node limit")
    } else {
        Ok(counter.count)
    }
}

struct SyntaxCounter {
    count: usize,
    limit: usize,
    exceeded: bool,
}

impl SyntaxCounter {
    const fn enter(&mut self) -> bool {
        self.count = self.count.saturating_add(1);
        if self.count > self.limit {
            self.exceeded = true;
            false
        } else {
            true
        }
    }
}

impl<'ast> Visit<'ast> for SyntaxCounter {
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

    fn visit_type(&mut self, ty: &'ast syn::Type) {
        if self.enter() {
            visit::visit_type(self, ty);
        }
    }
}

fn metadata_path(root: &Path, path: &Path) -> Result<PathBuf, &'static str> {
    let canonical = path
        .canonicalize()
        .map_err(|_| "Cargo metadata path is not canonicalizable")?;
    canonical
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .map_err(|_| "Cargo metadata path is outside the repository")
}

const fn public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

fn push_gap(
    path: &Path,
    tree: &UseTree,
    message: &'static str,
    findings: &mut Findings,
) {
    let code = if contains_glob(tree) { GLOB_GAP } else { GAP };
    findings.push(Finding::new(
        "public_type",
        code,
        Some(display_relative(path)),
        message,
    ));
}

fn contains_glob(tree: &UseTree) -> bool {
    match tree {
        UseTree::Glob(_) => true,
        UseTree::Path(path) => contains_glob(&path.tree),
        UseTree::Group(group) => group.items.iter().any(contains_glob),
        UseTree::Name(_) | UseTree::Rename(_) => false,
    }
}

struct UnsupportedUseVisitor<'a, 'b> {
    path: &'a Path,
    findings: &'b mut Findings,
}

impl<'ast> Visit<'ast> for UnsupportedUseVisitor<'_, '_> {
    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        if public(&item.vis) {
            push_gap(
                self.path,
                &item.tree,
                "public re-export is outside the direct Foundation profile",
                self.findings,
            );
        }
        visit::visit_item_use(self, item);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use reovim_depgraph::{
        ArchitecturalRole, SourceClassification, SourceOwner, SourceProvenance,
        SourceUse,
    };
    use syn::{ItemUse, visit::Visit as _};

    use super::{
        FOUNDATION, Limits, Resolver, builtin_derive, direct_name_count,
        exact_dependency, exact_reexports, parse_direct_use,
        provider_type_facts, require_consumer_edition,
        require_package_identity, shipping_relation, syntax_node_count,
        validate_complete_reexports, verify_registered_type,
    };
    use crate::cargo::{
        DependencyKind, Metadata, Node, Package, ResolvedDependency,
    };
    use crate::finding::Findings;
    use crate::product_closures::{
        BuiltinDerive, CargoSelection, Closure, ClosureKind, ConsumerOperation,
        DeriveCheck, PackageRef, PublicConsumerRelation, PublicItem,
        PublicItemKind, PublicReexport, Registry, TargetRef, WorkspaceRef,
    };
    use crate::source::ClassifiedFile;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "reovim-product-reexport-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn write(&self, relative: &str, bytes: &[u8]) {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn item(source: &str) -> ItemUse {
        syn::parse_str(source).unwrap()
    }

    fn names(source: &str) -> Vec<String> {
        parse_direct_use(&item(source)).unwrap().names
    }

    #[test]
    fn direct_name_and_flat_group_are_the_only_supported_forms() {
        assert_eq!(
            names("pub use ::reovim_foundation::bound::CheckedCount;"),
            ["CheckedCount"]
        );
        assert_eq!(
            names(concat!(
                "pub use ::reovim_foundation::bound::{",
                "BoundError, CheckedCount};"
            )),
            ["BoundError", "CheckedCount"]
        );
        for source in [
            "pub use ::reovim_foundation::bound::*;",
            "pub use ::reovim_foundation::bound::CheckedCount as Count;",
            "pub use reovim_foundation::bound::CheckedCount;",
            "pub use ::reovim_foundation::bound::self;",
            "pub use ::reovim_foundation::bound::{self, CheckedCount};",
            "pub use ::reovim_foundation::bound::{checked::CheckedCount};",
            "pub use ::reovim_foundation::bound::{CheckedCount, CheckedCount};",
        ] {
            assert!(parse_direct_use(&item(source)).is_err(), "{source}");
        }
    }

    #[test]
    fn unqualified_shadowable_dependency_paths_fail_closed() {
        for source in [
            concat!(
                "mod reovim_foundation {}\n",
                "pub use reovim_foundation::bound::CheckedCount;"
            ),
            concat!(
                "use other::Thing as reovim_foundation;\n",
                "pub use reovim_foundation::bound::CheckedCount;"
            ),
            concat!(
                "use other::reovim_foundation;\n",
                "pub use reovim_foundation::bound::CheckedCount;"
            ),
        ] {
            let syntax = syn::parse_file(source).unwrap();
            let public_use = syntax.items.iter().find_map(public_use).unwrap();
            assert!(parse_direct_use(public_use).is_err(), "{source}");
        }
    }

    #[test]
    fn edition_2015_crate_root_shadow_mutant_fails_closed() {
        let syntax = syn::parse_file(concat!(
            "mod reovim_foundation { pub mod bound { pub struct ",
            "CheckedCount; } }\n",
            "mod consumer { pub use ::reovim_foundation::bound::",
            "CheckedCount; }"
        ))
        .unwrap();
        assert!(syntax.items.iter().any(|item| {
            matches!(item, syn::Item::Mod(module)
                if module.ident == "reovim_foundation")
        }));
        assert!(
            parse_direct_use(&item(
                "pub use ::reovim_foundation::bound::CheckedCount;"
            ))
            .is_ok()
        );
        let package = cargo_package("reovim-kernel-interface", "2015");
        assert!(require_consumer_edition(&package).is_err());
    }

    #[test]
    fn alternate_foundation_package_and_relation_are_not_admitted() {
        let package = cargo_package("alternate-foundation", "2024");
        assert!(require_package_identity(&package.name, FOUNDATION).is_err());

        let mut alternate = relation(PublicItemKind::Type, all_copy_derives());
        alternate.id = "alternate-foundation-public-api".to_owned();
        let registry = crate::product_closures::Registry {
            closures: Vec::new(),
            public_consumer_relations: vec![alternate],
        };
        assert!(
            shipping_relation(&registry, "foundation-shipping", FOUNDATION,)
                .is_err()
        );

        let mut retargeted = relation(PublicItemKind::Type, all_copy_derives());
        retargeted.consumer_closure =
            "alternate-foundation-consumer".to_owned();
        let registry = crate::product_closures::Registry {
            closures: Vec::new(),
            public_consumer_relations: vec![retargeted],
        };
        assert!(
            shipping_relation(&registry, "foundation-shipping", FOUNDATION,)
                .is_err()
        );
    }

    #[test]
    fn provider_facts_come_from_one_public_direct_definition() {
        let syntax = syn::parse_file(concat!(
            "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n",
            "pub struct CheckedCount(u64);"
        ))
        .unwrap();
        let derives = provider_type_facts(&syntax, "CheckedCount").unwrap();
        assert_eq!(derives, all_copy_derives());

        for source in [
            "struct CheckedCount(u64);",
            "pub type CheckedCount = u64;",
            "pub trait CheckedCount {}",
            "#[derive(core::fmt::Debug)] pub struct CheckedCount;",
            "#[derive(Clone, Clone)] pub struct CheckedCount;",
            "#[repr(C)] pub struct CheckedCount;",
        ] {
            let syntax = syn::parse_file(source).unwrap();
            assert!(
                provider_type_facts(&syntax, "CheckedCount").is_err(),
                "{source}"
            );
        }
    }

    #[test]
    fn registry_identity_kind_and_derives_are_equality_targets() {
        let mut relation = relation(PublicItemKind::Type, all_copy_derives());
        assert!(
            verify_registered_type(
                &relation,
                "bound::CheckedCount",
                &all_copy_derives(),
            )
            .is_ok()
        );

        relation.public_items[0].kind = PublicItemKind::Trait;
        assert!(
            verify_registered_type(
                &relation,
                "bound::CheckedCount",
                &all_copy_derives(),
            )
            .is_err()
        );
        relation.public_items[0].kind = PublicItemKind::Type;
        relation.derive_checks.pop();
        assert!(
            verify_registered_type(
                &relation,
                "bound::CheckedCount",
                &all_copy_derives(),
            )
            .is_err()
        );
    }

    fn all_copy_derives() -> BTreeSet<BuiltinDerive> {
        [
            BuiltinDerive::Clone,
            BuiltinDerive::Copy,
            BuiltinDerive::Debug,
            BuiltinDerive::PartialEq,
            BuiltinDerive::Eq,
        ]
        .into_iter()
        .collect()
    }

    fn relation(
        kind: PublicItemKind,
        derives: BTreeSet<BuiltinDerive>,
    ) -> PublicConsumerRelation {
        let public_type = "bound::CheckedCount".to_owned();
        let derive_checks = derives
            .into_iter()
            .map(|derive| DeriveCheck {
                derive,
                public_type: public_type.clone(),
                consumer_operation: operation(derive),
            })
            .collect();
        PublicConsumerRelation {
            id: "foundation-public-api".to_owned(),
            shipping_closure: "foundation-shipping".to_owned(),
            consumer_closure: "foundation-consumer".to_owned(),
            public_items: vec![PublicItem { path: public_type, kind }],
            derive_checks,
            reexports: Vec::new(),
        }
    }

    fn reexport(provider_type: &str) -> PublicReexport {
        PublicReexport {
            public_type: "clock::CheckedCount".to_owned(),
            provider_relation: "foundation-public-api".to_owned(),
            provider_type: provider_type.to_owned(),
        }
    }

    fn alias_findings(alias_source: &str) -> Vec<crate::finding::Finding> {
        let root = TempRoot::new();
        for (path, bytes) in [
            ("product/Cargo.toml", b"".as_slice()),
            ("kernel/foundation/Cargo.toml", b"".as_slice()),
            ("kernel/interface/Cargo.toml", b"".as_slice()),
            ("kernel/foundation/src/lib.rs", b"pub mod bound;".as_slice()),
            (
                "kernel/foundation/src/bound.rs",
                b"pub struct CheckedCount;".as_slice(),
            ),
            ("kernel/interface/src/lib.rs", b"pub mod clock;".as_slice()),
            ("kernel/interface/src/clock.rs", alias_source.as_bytes()),
        ] {
            root.write(path, bytes);
        }

        let foundation_root = classified(
            &root,
            "kernel/foundation/src/lib.rs",
            ArchitecturalRole::Foundation,
        );
        let foundation_module = classified(
            &root,
            "kernel/foundation/src/bound.rs",
            ArchitecturalRole::Foundation,
        );
        let kernel_root = classified(
            &root,
            "kernel/interface/src/lib.rs",
            ArchitecturalRole::KernelInterface,
        );
        let kernel_module = classified(
            &root,
            "kernel/interface/src/clock.rs",
            ArchitecturalRole::KernelInterface,
        );
        let files =
            [foundation_root, foundation_module, kernel_root, kernel_module];
        let inventory = files
            .iter()
            .map(|file| (file.path.clone(), file))
            .collect::<BTreeMap<_, _>>();
        let metadata = fixture_metadata(&root);
        let registry = fixture_registry();
        let syntax = syn::parse_file(alias_source).unwrap();
        let mut resolver = Resolver::new(limits(8, 4096, 4096));
        let mut findings = Findings::new(16);
        super::check(
            &super::Request {
                root: &root.0,
                profile: "release",
                metadata: &metadata,
                inventory: &inventory,
                registry: &registry,
                package_id: "kernel-id",
                source: &files[3],
                syntax: &syntax,
            },
            &mut resolver,
            &mut findings,
        );
        resolver.finish(&metadata, &registry, &mut findings);
        findings.finish()
    }

    fn classified(
        root: &TempRoot,
        path: &str,
        role: ArchitecturalRole,
    ) -> ClassifiedFile {
        let bytes = fs::read(root.0.join(path)).unwrap();
        let path = PathBuf::from(path);
        ClassifiedFile {
            content_digest: crate::digest::bytes(&bytes),
            classification: SourceClassification::new(
                SourceOwner::Product(role),
                SourceProvenance::Authored,
                SourceUse::Production,
            )
            .unwrap(),
            upstream: None,
            rule_prefix: path.clone(),
            rule_precedence: 1,
            path,
        }
    }

    fn fixture_metadata(root: &TempRoot) -> Metadata {
        let foundation = fixture_package(
            root,
            "foundation-id",
            "reovim-foundation",
            "kernel/foundation",
        );
        let kernel = fixture_package(
            root,
            "kernel-id",
            "reovim-kernel-interface",
            "kernel/interface",
        );
        let dependency = ResolvedDependency {
            name: "reovim_foundation".to_owned(),
            package: foundation.id.clone(),
            kinds: vec![DependencyKind {
                kind: "normal".to_owned(),
                target: None,
            }],
        };
        Metadata {
            workspace_root: root.0.join("product"),
            workspace_members: BTreeSet::from([
                foundation.id.clone(),
                kernel.id.clone(),
            ]),
            packages: BTreeMap::from([
                (foundation.id.clone(), foundation),
                (kernel.id.clone(), kernel),
            ]),
            nodes: BTreeMap::from([(
                "kernel-id".to_owned(),
                Node {
                    id: "kernel-id".to_owned(),
                    dependencies: vec![dependency],
                },
            )]),
        }
    }

    fn fixture_package(
        root: &TempRoot,
        id: &str,
        name: &str,
        directory: &str,
    ) -> Package {
        Package {
            id: id.to_owned(),
            name: name.to_owned(),
            version: "0.16.0-dev".to_owned(),
            edition: "2024".to_owned(),
            source: None,
            checksum: None,
            license: None,
            manifest_path: root.0.join(directory).join("Cargo.toml"),
            targets: vec![crate::cargo::Target {
                name: name.replace('-', "_"),
                kind: vec!["lib".to_owned()],
                crate_types: vec!["lib".to_owned()],
                src_path: root.0.join(directory).join("src/lib.rs"),
            }],
        }
    }

    fn fixture_registry() -> Registry {
        let foundation = relation(PublicItemKind::Type, BTreeSet::new());
        let mut kernel = relation(PublicItemKind::Type, BTreeSet::new());
        kernel.id = "kernel-interface-public-api".to_owned();
        kernel.shipping_closure = "kernel-interface-shipping".to_owned();
        kernel.consumer_closure = "kernel-interface-consumer".to_owned();
        kernel.public_items[0].path = "clock::CheckedCount".to_owned();
        kernel.reexports = vec![reexport("bound::CheckedCount")];
        Registry {
            closures: vec![
                fixture_closure(
                    super::FOUNDATION,
                    "foundation",
                    "kernel/foundation",
                ),
                fixture_closure(
                    super::KERNEL_INTERFACE,
                    "kernel-interface",
                    "kernel/interface",
                ),
            ],
            public_consumer_relations: vec![foundation, kernel],
        }
    }

    fn fixture_closure(
        identity: super::ShippingIdentity,
        package_id: &str,
        directory: &str,
    ) -> Closure {
        Closure {
            id: identity.closure.to_owned(),
            kind: ClosureKind::Shipping,
            workspace: WorkspaceRef {
                id: "product".to_owned(),
                manifest: PathBuf::from("product/Cargo.toml"),
                lockfile: PathBuf::from("product/Cargo.lock"),
            },
            package: PackageRef {
                id: package_id.to_owned(),
                name: identity.package.to_owned(),
                manifest: PathBuf::from(directory).join("Cargo.toml"),
            },
            target: TargetRef {
                id: "library".to_owned(),
                name: identity.package.replace('-', "_"),
                src_path: PathBuf::from(directory).join("src/lib.rs"),
                kind: vec!["lib".to_owned()],
                crate_types: vec!["lib".to_owned()],
            },
            architectural_role: identity.role,
            source_use: SourceUse::Production,
            cargo: CargoSelection { profile: "release".to_owned() },
            package_membership: None,
        }
    }

    fn static_registry(
        expected: PublicReexport,
    ) -> crate::product_closures::Registry {
        let foundation = relation(PublicItemKind::Type, all_copy_derives());
        let mut kernel = relation(PublicItemKind::Type, all_copy_derives());
        kernel.id = "kernel-interface-public-api".to_owned();
        kernel.shipping_closure = "kernel-interface-shipping".to_owned();
        kernel.consumer_closure = "kernel-interface-consumer".to_owned();
        kernel.public_items[0].path = "clock::CheckedCount".to_owned();
        for check in &mut kernel.derive_checks {
            check.public_type = "clock::CheckedCount".to_owned();
        }
        kernel.reexports = vec![expected];
        crate::product_closures::Registry {
            closures: Vec::new(),
            public_consumer_relations: vec![foundation, kernel],
        }
    }

    fn kernel_metadata() -> Metadata {
        let package = cargo_package("reovim-kernel-interface", "2024");
        let id = package.id.clone();
        Metadata {
            workspace_root: std::path::PathBuf::new(),
            workspace_members: BTreeSet::from([id.clone()]),
            packages: BTreeMap::from([(id, package)]),
            nodes: BTreeMap::new(),
        }
    }

    #[test]
    fn complete_reexport_mapping_is_an_exact_equality_target() {
        let expected = vec![reexport("bound::CheckedCount")];
        let actual = BTreeMap::from([(
            "clock::CheckedCount".to_owned(),
            expected[0].clone(),
        )]);
        assert!(exact_reexports(&expected, &actual).is_ok());

        let absent = BTreeMap::new();
        assert!(exact_reexports(&expected, &absent).is_err());

        let replaced = BTreeMap::from([(
            "clock::CheckedCount".to_owned(),
            reexport("bound::CheckedByteCount"),
        )]);
        assert!(exact_reexports(&expected, &replaced).is_err());

        let unregistered = vec![reexport("bound::CheckedByteCount")];
        assert!(exact_reexports(&unregistered, &actual).is_err());
    }

    #[test]
    fn deleted_or_locally_replaced_alias_cannot_satisfy_registry_origin() {
        let expected = vec![reexport("bound::CheckedCount")];
        let deleted_source = BTreeMap::new();
        let local_replacement = BTreeMap::new();
        assert!(exact_reexports(&expected, &deleted_source).is_err());
        assert!(exact_reexports(&expected, &local_replacement).is_err());
    }

    #[test]
    fn source_alias_deletion_and_local_replacement_fail_end_to_end() {
        let valid =
            alias_findings("pub use ::reovim_foundation::bound::CheckedCount;");
        assert_eq!(valid, Vec::new());

        let deleted = alias_findings("");
        let replaced = alias_findings("pub struct CheckedCount;");
        assert!(has_reexport_gap(&deleted));
        assert!(has_reexport_gap(&replaced));
    }

    #[test]
    fn finalizer_requires_complete_kernel_mapping_and_empty_foundation() {
        let expected = reexport("bound::CheckedCount");
        let metadata = kernel_metadata();
        let mut registry = static_registry(expected.clone());
        let actual = BTreeMap::from([(
            "clock::CheckedCount".to_owned(),
            expected.clone(),
        )]);
        assert!(
            validate_complete_reexports(&metadata, &registry, &actual).is_ok()
        );
        assert!(
            validate_complete_reexports(&metadata, &registry, &BTreeMap::new())
                .is_err()
        );

        registry.public_consumer_relations[0].reexports.push(expected);
        assert!(
            validate_complete_reexports(&metadata, &registry, &actual).is_err()
        );
    }

    #[test]
    fn duplicate_source_alias_is_rejected_transactionally() {
        let mut resolver = Resolver::new(limits(4, 64, 64));
        let first = reexport("bound::CheckedCount");
        resolver.record_reexports(vec![first.clone()]).unwrap();
        let before = resolver.reexports.clone();
        assert!(resolver.record_reexports(vec![first]).is_err());
        assert_eq!(resolver.reexports, before);

        let mut resolver = Resolver::new(limits(4, 64, 64));
        let duplicate = reexport("bound::CheckedCount");
        assert!(
            resolver
                .record_reexports(vec![duplicate.clone(), duplicate])
                .is_err()
        );
        assert_eq!(resolver.reexports, BTreeMap::new());
    }

    fn has_reexport_gap(findings: &[crate::finding::Finding]) -> bool {
        findings
            .iter()
            .any(|finding| finding.code == "UNSUPPORTED_REEXPORT_PROOF_GAP")
    }

    fn operation(derive: BuiltinDerive) -> ConsumerOperation {
        match derive {
            BuiltinDerive::Clone => ConsumerOperation::Clone,
            BuiltinDerive::Copy => ConsumerOperation::Copy,
            BuiltinDerive::Debug => ConsumerOperation::Debug,
            BuiltinDerive::PartialEq => ConsumerOperation::PartialEq,
            BuiltinDerive::Eq => ConsumerOperation::Eq,
            BuiltinDerive::Hash => ConsumerOperation::Hash,
            BuiltinDerive::Default => ConsumerOperation::Default,
        }
    }

    #[test]
    fn qualified_and_unknown_derives_fail_closed() {
        for source in ["core::clone::Clone", "::Debug", "Arbitrary"] {
            let path = syn::parse_str(source).unwrap();
            assert!(builtin_derive(&path).is_err(), "{source}");
        }
    }

    #[test]
    fn unsupported_public_use_retains_its_typed_gap() {
        let syntax = syn::parse_file(
            "mod nested { pub use reovim_foundation::bound::*; }",
        )
        .unwrap();
        let mut findings = Findings::new(4);
        super::UnsupportedUseVisitor {
            path: std::path::Path::new("src/lib.rs"),
            findings: &mut findings,
        }
        .visit_file(&syntax);
        let findings = findings.finish();
        let [finding] = findings.as_slice() else {
            panic!("expected one typed public-use gap");
        };
        assert_eq!(finding.code, "UNSUPPORTED_GLOB_REEXPORT_PROOF_GAP");
    }

    #[test]
    fn dependency_must_be_one_unconditional_normal_direct_edge() {
        let mut metadata = dependency_metadata(DependencyKind {
            kind: "normal".to_owned(),
            target: None,
        });
        assert!(
            exact_dependency(&metadata, "consumer", "reovim_foundation",)
                .is_ok()
        );
        for kind in [
            DependencyKind { kind: "development".to_owned(), target: None },
            DependencyKind { kind: "build".to_owned(), target: None },
            DependencyKind {
                kind: "normal".to_owned(),
                target: Some("cfg(unix)".to_owned()),
            },
        ] {
            metadata.nodes = dependency_metadata(kind).nodes;
            assert!(
                exact_dependency(&metadata, "consumer", "reovim_foundation",)
                    .is_err()
            );
        }
    }

    #[test]
    fn import_byte_and_node_amplification_exhausts_explicit_limits() {
        let mut resolver = Resolver::new(limits(2, 4, 3));
        assert!(resolver.charge_imports(2).is_ok());
        assert!(resolver.charge_imports(1).is_err());
        assert!(resolver.charge_source_bytes(4).is_ok());
        assert!(resolver.charge_source_bytes(1).is_err());
        assert!(resolver.charge_syntax_nodes(3).is_ok());
        assert!(resolver.charge_syntax_nodes(1).is_err());

        let many = item("pub use ::provider::types::{A, B, C};");
        let mut resolver = Resolver::new(limits(2, 64, 64));
        let count = direct_name_count(&many).unwrap();
        assert_eq!(count, 3);
        assert!(resolver.charge_imports(count).is_err());

        let large = syn::parse_file(
            "pub struct A; pub struct B; pub struct C; pub struct D;",
        )
        .unwrap();
        assert!(syntax_node_count(&large, 3).is_err());
    }

    const fn limits(
        imports: usize,
        source_total_bytes: u64,
        syntax_nodes: usize,
    ) -> Limits {
        Limits {
            imports,
            source_files: 4,
            source_file_bytes: source_total_bytes,
            source_total_bytes,
            syntax_nodes,
        }
    }

    fn public_use(item: &syn::Item) -> Option<&ItemUse> {
        let syn::Item::Use(item) = item else { return None };
        matches!(item.vis, syn::Visibility::Public(_)).then_some(item)
    }

    fn dependency_metadata(kind: DependencyKind) -> Metadata {
        let dependency = ResolvedDependency {
            name: "reovim_foundation".to_owned(),
            package: "provider".to_owned(),
            kinds: vec![kind],
        };
        Metadata {
            workspace_root: std::path::PathBuf::new(),
            workspace_members: BTreeSet::new(),
            packages: std::collections::BTreeMap::new(),
            nodes: std::collections::BTreeMap::from([(
                "consumer".to_owned(),
                Node {
                    id: "consumer".to_owned(),
                    dependencies: vec![dependency],
                },
            )]),
        }
    }

    fn cargo_package(name: &str, edition: &str) -> Package {
        Package {
            id: name.to_owned(),
            name: name.to_owned(),
            version: "0.16.0-dev".to_owned(),
            edition: edition.to_owned(),
            source: None,
            checksum: None,
            license: None,
            manifest_path: std::path::PathBuf::from("Cargo.toml"),
            targets: Vec::new(),
        }
    }
}
