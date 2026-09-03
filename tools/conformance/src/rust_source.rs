//! Conservative Rust import and public-type boundary analysis.
//!
//! Syn parses syntax; it does not expand macros, evaluate cfg predicates, or
//! resolve names. Product syntax that can conceal a boundary is therefore a
//! typed proof gap rather than an optimistic pass.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use reovim_depgraph::{SourceOwner, SourceProvenance, SourceUse};
use syn::{
    Attribute, File, ImplItem, Item, ReturnType, Type, UseTree, Visibility,
    visit::{self, Visit},
};

use crate::{
    cargo::Metadata,
    finding::{Finding, Findings},
    policy::Policy,
    product_closures, product_reexport,
    source::{ClassifiedFile, display_relative, read_bounded},
    topology::{Limits, Topology},
};

pub struct CheckRequest<'a> {
    pub root: &'a Path,
    pub profile: &'a str,
    pub metadata: &'a Metadata,
    pub files: &'a [ClassifiedFile],
    pub policy: &'a Policy,
    pub topology: &'a Topology,
    pub product_registry: &'a product_closures::Registry,
    pub limits: Limits,
}

pub fn check(request: &CheckRequest<'_>) -> Vec<Finding> {
    let root = request.root;
    let profile = request.profile;
    let metadata = request.metadata;
    let files = request.files;
    let policy = request.policy;
    let topology = request.topology;
    let product_registry = request.product_registry;
    let limits = request.limits;
    let mut findings = Findings::new(limits.findings);
    let packages = workspace_packages(root, metadata, files);
    let target_uses = target_source_uses(root, metadata, files);
    let target_roots = target_uses.keys().cloned().collect::<BTreeSet<_>>();
    let selected_sources = resolve_module_sources(
        root,
        &target_uses,
        files,
        limits,
        profile,
        &mut findings,
    );
    let inventory = files
        .iter()
        .map(|file| (file.path.clone(), file))
        .collect::<BTreeMap<_, _>>();
    let package_directories = topology
        .workspaces
        .values()
        .flat_map(|workspace| workspace.packages.values())
        .filter_map(|package| package.manifest.parent())
        .collect::<BTreeSet<_>>();
    let context = RustCheckContext {
        root,
        profile,
        metadata,
        packages: &packages,
        selected_sources: &selected_sources,
        target_roots: &target_roots,
        inventory: &inventory,
        policy,
        product_registry,
        limits,
    };
    let mut reexports =
        product_reexport::Resolver::new(product_reexport::Limits {
            imports: limits.product_execution.public_item_count,
            source_files: limits.source_file_count,
            source_file_bytes: limits.source_file_bytes,
            source_total_bytes: limits.source_total_bytes,
            syntax_nodes: limits.syntax_item_expr_type_visits,
        });
    for file in files {
        if !rust_source_file(file) {
            continue;
        }
        let current_package = package_for(&file.path, &packages).is_some();
        let known_package = package_directories
            .iter()
            .any(|directory| file.path.starts_with(directory));
        if !current_package && known_package {
            continue;
        }
        context.check_file(file, &mut reexports, &mut findings);
    }
    reexports.finish(metadata, product_registry, &mut findings);
    findings.finish()
}

fn target_source_uses(
    root: &Path,
    metadata: &Metadata,
    files: &[ClassifiedFile],
) -> BTreeMap<PathBuf, BTreeSet<SourceUse>> {
    metadata
        .workspace_members
        .iter()
        .filter_map(|id| metadata.packages.get(id))
        .flat_map(|package| &package.targets)
        .filter_map(|target| {
            let path =
                target.src_path.canonicalize().ok().and_then(|path| {
                    path.strip_prefix(root).ok().map(Path::to_path_buf)
                })?;
            let source_use = files
                .iter()
                .find(|file| file.path == path)?
                .classification
                .source_use;
            Some((path, source_use))
        })
        .fold(
            BTreeMap::<PathBuf, BTreeSet<SourceUse>>::new(),
            |mut uses, (path, source_use)| {
                uses.entry(path).or_default().insert(source_use);
                uses
            },
        )
}

fn rust_source_file(file: &ClassifiedFile) -> bool {
    file.path.extension().is_some_and(|value| value == "rs")
}

struct RustCheckContext<'a> {
    root: &'a Path,
    profile: &'a str,
    metadata: &'a Metadata,
    packages: &'a BTreeMap<String, PackageContext>,
    selected_sources: &'a BTreeMap<PathBuf, BTreeSet<SourceUse>>,
    target_roots: &'a BTreeSet<PathBuf>,
    inventory: &'a BTreeMap<PathBuf, &'a ClassifiedFile>,
    policy: &'a Policy,
    product_registry: &'a product_closures::Registry,
    limits: Limits,
}

impl RustCheckContext<'_> {
    fn check_file(
        &self,
        file: &ClassifiedFile,
        reexports: &mut product_reexport::Resolver,
        findings: &mut Findings,
    ) {
        if reject_build_output(file, self.profile, findings) {
            return;
        }
        let Some((package_id, package_owner)) =
            package_for(&file.path, self.packages)
        else {
            findings.push(Finding::new(
                "source",
                "RUST_SOURCE_PACKAGE",
                Some(display_relative(&file.path)),
                format!(
                    concat!(
                        "profile {}: Rust source is outside every ",
                        "workspace package"
                    ),
                    self.profile
                ),
            ));
            return;
        };
        report_unregistered_source(
            &file.path,
            self.selected_sources,
            self.profile,
            findings,
        );
        let Some(syntax) = self.parse_file(file, findings) else {
            return;
        };
        self.check_syntax(file, &syntax, findings);
        if !syntax_within_limit(
            &syntax,
            &file.path,
            self.limits.syntax_item_expr_type_visits,
            findings,
        ) {
            return;
        }
        let dependencies =
            dependency_owners(package_id, self.metadata, self.packages);
        analyze_file(
            &AnalysisRequest {
                syntax: &syntax,
                file,
                package_id,
                owner: package_owner,
                dependencies: &dependencies,
                policy: self.policy,
                profile: self.profile,
                root: self.root,
                metadata: self.metadata,
                inventory: self.inventory,
                product_registry: self.product_registry,
            },
            reexports,
            findings,
        );
    }

    fn parse_file(
        &self,
        file: &ClassifiedFile,
        findings: &mut Findings,
    ) -> Option<File> {
        let bytes = match read_bounded(
            &self.root.join(&file.path),
            self.limits.source_file_bytes,
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                findings.push(Finding::new(
                    "source",
                    "RUST_SOURCE_READ",
                    Some(display_relative(&file.path)),
                    error,
                ));
                return None;
            }
        };
        let contents = match std::str::from_utf8(&bytes) {
            Ok(contents) => contents,
            Err(error) => {
                findings.push(Finding::new(
                    "source",
                    "RUST_SOURCE_UTF8",
                    Some(display_relative(&file.path)),
                    error.to_string(),
                ));
                return None;
            }
        };
        let analysis_digest = crate::digest::bytes(&bytes);
        if analysis_digest != file.content_digest {
            findings.push(Finding::new(
                "source",
                "RUST_SOURCE_INVENTORY_DRIFT",
                Some(display_relative(&file.path)),
                format!(
                    concat!(
                        "profile {}: parsed bytes digest {analysis_digest} ",
                        "differs from inventory {}"
                    ),
                    self.profile,
                    file.content_digest,
                    analysis_digest = analysis_digest
                ),
            ));
            return None;
        }
        match syn::parse_file(contents) {
            Ok(syntax) => Some(syntax),
            Err(error) => {
                findings.push(Finding::new(
                    "source",
                    "RUST_PARSE",
                    Some(display_relative(&file.path)),
                    error.to_string(),
                ));
                None
            }
        }
    }

    fn check_syntax(
        &self,
        file: &ClassifiedFile,
        syntax: &File,
        findings: &mut Findings,
    ) {
        let mut freestanding = FreestandingVisitor { found: false };
        freestanding.visit_file(syntax);
        if freestanding.found {
            findings.push(Finding::new(
                "retired_surface",
                "FREESTANDING_ATTRIBUTE_FORBIDDEN",
                Some(display_relative(&file.path)),
                "every first-party Rust file forbids no_std and no_main",
            ));
        }
        if self.target_roots.contains(&file.path) {
            validate_crate_attributes(syntax, &file.path, findings);
        }
        let mut unsafe_syntax = UnsafeVisitor { found: false };
        unsafe_syntax.visit_file(syntax);
        if unsafe_syntax.found {
            findings.push(Finding::new(
                "safety",
                "FIRST_PARTY_UNSAFE_SYNTAX",
                Some(display_relative(&file.path)),
                "first-party Rust source contains unsafe syntax",
            ));
        }
        IncludeVisitor {
            root: self.root,
            source_path: &file.path,
            source_classification: file.classification,
            inventory: self.inventory,
            file_byte_limit: self.limits.source_file_bytes,
            findings,
        }
        .visit_file(syntax);
    }
}

fn syntax_within_limit(
    syntax: &File,
    path: &Path,
    limit: usize,
    findings: &mut Findings,
) -> bool {
    let mut counter = NodeCounter { count: 0, limit, exceeded: false };
    counter.visit_file(syntax);
    if !counter.exceeded {
        return true;
    }
    findings.push(Finding::new(
        "limits",
        "AST_NODE_LIMIT",
        Some(display_relative(path)),
        format!("selected item/expression/type visits exceed {limit}"),
    ));
    false
}

fn report_unregistered_source(
    path: &Path,
    selected: &BTreeMap<PathBuf, BTreeSet<SourceUse>>,
    profile: &str,
    findings: &mut Findings,
) {
    if !selected.contains_key(path) {
        findings.push(Finding::new(
            "source",
            "UNREGISTERED_RUST_SOURCE",
            Some(display_relative(path)),
            format!(
                concat!(
                    "profile {profile}: Rust file is not a Cargo target root ",
                    "or reachable module"
                ),
                profile = profile
            ),
        ));
    }
}

fn resolve_module_sources(
    root: &Path,
    target_uses: &BTreeMap<PathBuf, BTreeSet<SourceUse>>,
    files: &[ClassifiedFile],
    limits: Limits,
    profile: &str,
    findings: &mut Findings,
) -> BTreeMap<PathBuf, BTreeSet<SourceUse>> {
    let inventory = files
        .iter()
        .map(|file| (file.path.clone(), file))
        .collect::<BTreeMap<_, _>>();
    let mut selected = BTreeMap::<PathBuf, BTreeSet<SourceUse>>::new();
    let mut processed = BTreeSet::new();
    let mut pending = target_uses
        .iter()
        .flat_map(|(path, uses)| {
            uses.iter().map(|source_use| (path.clone(), *source_use))
        })
        .collect::<Vec<_>>();
    let resolver = ModuleResolver {
        root,
        target_uses,
        inventory: &inventory,
        limits,
        profile,
    };
    while let Some((path, source_use)) = pending.pop() {
        selected.entry(path.clone()).or_default().insert(source_use);
        if !processed.insert((path.clone(), source_use)) {
            continue;
        }
        resolver.resolve(&path, source_use, &mut pending, findings);
    }
    validate_selected_source_uses(&selected, &inventory, profile, findings);
    selected
}

struct ModuleResolver<'a> {
    root: &'a Path,
    target_uses: &'a BTreeMap<PathBuf, BTreeSet<SourceUse>>,
    inventory: &'a BTreeMap<PathBuf, &'a ClassifiedFile>,
    limits: Limits,
    profile: &'a str,
}

impl ModuleResolver<'_> {
    fn resolve(
        &self,
        path: &Path,
        source_use: SourceUse,
        pending: &mut Vec<(PathBuf, SourceUse)>,
        findings: &mut Findings,
    ) {
        let Some(file) = self.inventory.get(path) else {
            findings.push(Finding::new(
                "source",
                "MODULE_SOURCE_UNCLASSIFIED",
                Some(display_relative(path)),
                format!(
                    "profile {}: target/module source is absent from inventory",
                    self.profile
                ),
            ));
            return;
        };
        let bytes = match read_bounded(
            &self.root.join(path),
            self.limits.source_file_bytes,
        ) {
            Ok(bytes)
                if crate::digest::bytes(&bytes) == file.content_digest =>
            {
                bytes
            }
            Ok(_) => {
                findings.push(Finding::new(
                    "source",
                    "MODULE_SOURCE_INVENTORY_DRIFT",
                    Some(display_relative(path)),
                    "module resolver bytes differ from inventory",
                ));
                return;
            }
            Err(error) => {
                findings.push(Finding::new(
                    "source",
                    "MODULE_SOURCE_READ",
                    Some(display_relative(path)),
                    error,
                ));
                return;
            }
        };
        let Some(syntax) = std::str::from_utf8(&bytes)
            .ok()
            .and_then(|text| syn::parse_file(text).ok())
        else {
            return;
        };
        let base = module_base(path, self.target_uses.contains_key(path));
        collect_external_modules(
            &syntax.items,
            &base,
            self.inventory,
            self.profile,
            source_use,
            pending,
            findings,
        );
    }
}

fn module_base(path: &Path, target_root: bool) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    if target_root || path.file_name().is_some_and(|name| name == "mod.rs") {
        parent.to_path_buf()
    } else {
        parent.join(path.file_stem().unwrap_or_default())
    }
}

fn validate_selected_source_uses(
    selected: &BTreeMap<PathBuf, BTreeSet<SourceUse>>,
    inventory: &BTreeMap<PathBuf, &ClassifiedFile>,
    profile: &str,
    findings: &mut Findings,
) {
    for (path, uses) in selected {
        let Some(file) = inventory.get(path) else { continue };
        let expected = if uses.contains(&SourceUse::Tool) {
            Some(SourceUse::Tool)
        } else if uses.len() == 1 {
            uses.first().copied()
        } else {
            findings.push(Finding::new(
                "classification",
                "MODULE_SOURCE_USE_AMBIGUOUS",
                Some(display_relative(path)),
                format!(
                    concat!(
                        "profile {profile}: a module shared only by Test and ",
                        "Example targets has no single effective use"
                    ),
                    profile = profile
                ),
            ));
            None
        };
        if expected
            .is_some_and(|expected| file.classification.source_use != expected)
        {
            findings.push(Finding::new(
                "classification",
                "MODULE_SOURCE_CLASSIFICATION",
                Some(display_relative(path)),
                format!(
                    concat!(
                        "profile {profile}: module reachability requires ",
                        "source use {}"
                    ),
                    expected.expect("checked above").as_str(),
                    profile = profile
                ),
            ));
        }
    }
}

fn collect_external_modules(
    items: &[Item],
    base: &Path,
    inventory: &BTreeMap<PathBuf, &ClassifiedFile>,
    profile: &str,
    source_use: SourceUse,
    pending: &mut Vec<(PathBuf, SourceUse)>,
    findings: &mut Findings,
) {
    for item in items {
        let Item::Mod(module) = item else { continue };
        if module.attrs.iter().any(module_attribute_overrides_path) {
            findings.push(Finding::new(
                "source",
                "UNSUPPORTED_PATH_MODULE",
                None,
                format!(
                    concat!(
                        "profile {profile}: #[path] module resolution is not ",
                        "admitted"
                    ),
                    profile = profile
                ),
            ));
            continue;
        }
        let name = module.ident.to_string();
        if let Some((_, nested)) = &module.content {
            collect_external_modules(
                nested,
                &base.join(name),
                inventory,
                profile,
                source_use,
                pending,
                findings,
            );
            continue;
        }
        let candidates =
            [base.join(format!("{name}.rs")), base.join(&name).join("mod.rs")]
                .into_iter()
                .filter(|candidate| inventory.contains_key(candidate))
                .collect::<Vec<_>>();
        match candidates.as_slice() {
            [candidate] => pending.push((candidate.clone(), source_use)),
            [] => findings.push(Finding::new(
                "source",
                "MODULE_SOURCE_MISSING",
                None,
                format!(
                    concat!(
                        "profile {profile}: external module {name} has no ",
                        "inventoried source"
                    ),
                    profile = profile,
                    name = name
                ),
            )),
            _ => findings.push(Finding::new(
                "source",
                "MODULE_SOURCE_AMBIGUOUS",
                None,
                format!(
                    concat!(
                        "profile {profile}: external module {name} has two ",
                        "candidate sources"
                    ),
                    profile = profile,
                    name = name
                ),
            )),
        }
    }
}

fn module_attribute_overrides_path(attribute: &Attribute) -> bool {
    attribute.path().is_ident("path")
        || (attribute.path().is_ident("cfg_attr")
            && match &attribute.meta {
                syn::Meta::List(list) => {
                    token_display_has_word(&list.tokens.to_string(), "path")
                }
                _ => false,
            })
}

struct FreestandingVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for FreestandingVisitor {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        self.found |= attribute.path().is_ident("no_std")
            || attribute.path().is_ident("no_main");
        if let syn::Meta::List(list) = &attribute.meta {
            let tokens = list.tokens.to_string();
            self.found |= token_display_has_word(&tokens, "no_std")
                || token_display_has_word(&tokens, "no_main");
        }
        visit::visit_attribute(self, attribute);
    }

    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        let tokens = item.tokens.to_string();
        self.found |= token_display_has_word(&tokens, "no_std")
            || token_display_has_word(&tokens, "no_main");
        visit::visit_macro(self, item);
    }
}

fn reject_build_output(
    file: &ClassifiedFile,
    profile: &str,
    findings: &mut Findings,
) -> bool {
    if file.classification.source_use == reovim_depgraph::SourceUse::BuildOutput
    {
        findings.push(Finding::new(
            "source",
            "RUST_SOURCE_BUILD_OUTPUT",
            Some(display_relative(&file.path)),
            format!(
                concat!(
                    "profile {profile}: Rust source cannot be admitted as ",
                    "BuildOutput"
                ),
                profile = profile
            ),
        ));
        true
    } else {
        false
    }
}

fn validate_crate_attributes(
    syntax: &File,
    path: &Path,
    findings: &mut Findings,
) {
    let has_forbid_unsafe = syntax.attrs.iter().any(|attribute| {
        attribute.path().is_ident("forbid")
            && match &attribute.meta {
                syn::Meta::List(list) => list
                    .tokens
                    .to_string()
                    .split(|character: char| {
                        character == ',' || character.is_whitespace()
                    })
                    .any(|token| token == "unsafe_code"),
                _ => false,
            }
    });
    if !has_forbid_unsafe {
        findings.push(Finding::new(
            "safety",
            "TARGET_FORBID_UNSAFE_MISSING",
            Some(display_relative(path)),
            concat!(
                "every first-party Rust target root requires ",
                "#![forbid(unsafe_code)]"
            ),
        ));
    }
}

struct UnsafeVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for UnsafeVisitor {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        self.found |= attribute.path().is_ident("unsafe");
        if let syn::Meta::List(list) = &attribute.meta {
            self.found |=
                token_display_has_word(&list.tokens.to_string(), "unsafe");
        }
        visit::visit_attribute(self, attribute);
    }

    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        self.found |=
            token_display_has_word(&item.tokens.to_string(), "unsafe");
        visit::visit_macro(self, item);
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

struct IncludeVisitor<'a, 'b> {
    root: &'a Path,
    source_path: &'a Path,
    source_classification: reovim_depgraph::SourceClassification,
    inventory: &'a BTreeMap<PathBuf, &'a ClassifiedFile>,
    file_byte_limit: u64,
    findings: &'b mut Findings,
}

impl IncludeVisitor<'_, '_> {
    fn check(&mut self, item: &syn::Macro) {
        let name =
            item.path.segments.last().map(|segment| segment.ident.to_string());
        if name.as_deref().is_some_and(include_name)
            && item.path.segments.len() != 1
        {
            self.findings.push(Finding::new(
                "source",
                "INCLUDE_QUALIFIED_PROOF_UNAVAILABLE",
                Some(display_relative(self.source_path)),
                concat!(
                    "qualified include macro paths are not admitted as Rust ",
                    "built-ins in v1"
                ),
            ));
            return;
        }
        match name.as_deref() {
            Some("include") => self.findings.push(Finding::new(
                "source",
                "INCLUDE_RUST_PROOF_UNAVAILABLE",
                Some(display_relative(self.source_path)),
                concat!(
                    "include! source expansion is not admitted by the v1 ",
                    "syntax checker"
                ),
            )),
            Some("include_str" | "include_bytes") => {
                self.check_literal_include(item);
            }
            _ => self.check_nested_include(item),
        }
    }

    fn check_nested_include(&mut self, item: &syn::Macro) {
        let tokens = item.tokens.to_string();
        let contains_include = ["include", "include_str", "include_bytes"]
            .into_iter()
            .any(|word| token_display_has_word(&tokens, word));
        if contains_include {
            self.findings.push(Finding::new(
                "source",
                "INCLUDE_MACRO_PROOF_UNAVAILABLE",
                Some(display_relative(self.source_path)),
                concat!(
                    "include tokens nested inside another macro cannot be ",
                    "resolved structurally"
                ),
            ));
        }
    }

    fn check_literal_include(&mut self, item: &syn::Macro) {
        let Ok(literal) = item.parse_body::<syn::LitStr>() else {
            self.findings.push(Finding::new(
                "source",
                "INCLUDE_PATH_NOT_LITERAL",
                Some(display_relative(self.source_path)),
                "include_str!/include_bytes! require one literal relative path",
            ));
            return;
        };
        let literal = literal.value();
        let target = match resolve_include_path(self.source_path, &literal) {
            Ok(target) => target,
            Err(message) => {
                self.findings.push(Finding::new(
                    "source",
                    "INCLUDE_PATH_ESCAPE",
                    Some(display_relative(self.source_path)),
                    message,
                ));
                return;
            }
        };
        let Some(classified) = self.inventory.get(&target) else {
            self.findings.push(Finding::new(
                "source",
                "INCLUDE_TARGET_UNCLASSIFIED",
                Some(display_relative(&target)),
                format!(
                    "{} consumes a path absent from the classified inventory",
                    display_relative(self.source_path)
                ),
            ));
            return;
        };
        self.check_include_classification(&target, classified);
        self.check_include_bytes(&target, classified);
    }

    fn check_include_classification(
        &mut self,
        target: &Path,
        classified: &ClassifiedFile,
    ) {
        let compatible = classified.classification.owner
            == SourceOwner::RepositoryTooling
            && include_use_compatible(
                self.source_classification.source_use,
                classified.classification.source_use,
            );
        if compatible {
            return;
        }
        self.findings.push(Finding::new(
            "source",
            "INCLUDE_TARGET_CLASSIFICATION",
            Some(display_relative(target)),
            format!(
                concat!(
                    "included input is incompatible with RepositoryTooling ",
                    "consumer use {}"
                ),
                self.source_classification.source_use.as_str()
            ),
        ));
    }

    fn check_include_bytes(
        &mut self,
        target: &Path,
        classified: &ClassifiedFile,
    ) {
        match read_bounded(&self.root.join(target), self.file_byte_limit) {
            Ok(bytes)
                if crate::digest::bytes(&bytes)
                    == classified.content_digest => {}
            Ok(_) => self.findings.push(Finding::new(
                "source",
                "INCLUDE_TARGET_INVENTORY_DRIFT",
                Some(display_relative(target)),
                "included bytes differ from the classified inventory digest",
            )),
            Err(error) => self.findings.push(Finding::new(
                "source",
                "INCLUDE_TARGET_READ",
                Some(display_relative(target)),
                error,
            )),
        }
    }
}

const fn include_use_compatible(consumer: SourceUse, input: SourceUse) -> bool {
    match consumer {
        SourceUse::Tool => matches!(input, SourceUse::Tool),
        SourceUse::Test => matches!(input, SourceUse::Tool | SourceUse::Test),
        SourceUse::Example => {
            matches!(input, SourceUse::Tool | SourceUse::Example)
        }
        SourceUse::Production | SourceUse::BuildOutput => false,
    }
}

impl<'ast> Visit<'ast> for IncludeVisitor<'_, '_> {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        if attribute.path().is_ident("macro_use") {
            self.findings.push(Finding::new(
                "source",
                "INCLUDE_ALIAS_PROOF_UNAVAILABLE",
                Some(display_relative(self.source_path)),
                "#[macro_use] can shadow an admitted include macro name",
            ));
        }
        visit::visit_attribute(self, attribute);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if include_alias(&item.tree) {
            self.findings.push(Finding::new(
                "source",
                "INCLUDE_ALIAS_PROOF_UNAVAILABLE",
                Some(display_relative(self.source_path)),
                concat!(
                    "renamed include macros cannot be proven to use a ",
                    "classified literal input"
                ),
            ));
        }
        visit::visit_item_use(self, item);
    }

    fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
        if item
            .ident
            .as_ref()
            .is_some_and(|ident| include_name(&ident.to_string()))
        {
            self.findings.push(Finding::new(
                "source",
                "INCLUDE_ALIAS_PROOF_UNAVAILABLE",
                Some(display_relative(self.source_path)),
                "a local macro shadows a checked include macro name",
            ));
        }
        visit::visit_item_macro(self, item);
    }

    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        self.check(item);
        visit::visit_macro(self, item);
    }
}

fn include_name(name: &str) -> bool {
    matches!(name, "include" | "include_str" | "include_bytes")
}

fn include_alias(tree: &UseTree) -> bool {
    match tree {
        UseTree::Path(path) => include_alias(&path.tree),
        UseTree::Group(group) => group.items.iter().any(include_alias),
        UseTree::Rename(rename) => {
            include_name(&rename.ident.to_string())
                || include_name(&rename.rename.to_string())
        }
        UseTree::Name(name) => include_name(&name.ident.to_string()),
        UseTree::Glob(_) => true,
    }
}

fn resolve_include_path(
    source_path: &Path,
    literal: &str,
) -> Result<PathBuf, String> {
    if literal.is_empty()
        || literal.contains(['\\', '\0'])
        || literal.as_bytes().get(1) == Some(&b':')
    {
        return Err(
            "include path is empty or uses a non-portable path form".to_owned()
        );
    }
    let literal = Path::new(literal);
    if literal.is_absolute() {
        return Err(
            "absolute include paths are outside the repository contract"
                .to_owned(),
        );
    }
    let mut output = source_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_os_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for component in literal.components() {
        match component {
            std::path::Component::Normal(value) => {
                output.push(value.to_os_string());
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir if output.pop().is_some() => {}
            std::path::Component::ParentDir => {
                return Err(
                    "include path escapes the repository root".to_owned()
                );
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(
                    "include path is not repository-relative".to_owned()
                );
            }
        }
    }
    if output.is_empty() {
        return Err("include path resolves to the repository root".to_owned());
    }
    Ok(output.into_iter().collect())
}

fn token_display_has_word(input: &str, expected: &str) -> bool {
    let bytes = input.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        match scan_display_token(bytes, cursor, expected) {
            DisplayToken::Found => return true,
            DisplayToken::Next(next) => cursor = next,
        }
    }
    false
}

enum DisplayToken {
    Found,
    Next(usize),
}

fn scan_display_token(
    bytes: &[u8],
    cursor: usize,
    expected: &str,
) -> DisplayToken {
    if let Some(end) = raw_string_end(bytes, cursor) {
        return DisplayToken::Next(end);
    }
    if matches!(bytes[cursor], b'"' | b'\'') {
        let next = quoted_display_token_end(bytes, cursor)
            .unwrap_or_else(|| cursor.saturating_add(1));
        return DisplayToken::Next(next);
    }
    if !rust_identifier_start(bytes[cursor]) {
        return DisplayToken::Next(cursor.saturating_add(1));
    }
    let end = rust_identifier_end(bytes, cursor);
    if &bytes[cursor..end] == expected.as_bytes() {
        DisplayToken::Found
    } else {
        DisplayToken::Next(end)
    }
}

fn quoted_display_token_end(bytes: &[u8], start: usize) -> Option<usize> {
    let quote = bytes[start];
    if quote == b'\'' && lifetime_without_closing_quote(bytes, start) {
        return None;
    }
    let mut cursor = start.saturating_add(1);
    while cursor < bytes.len() {
        if bytes[cursor] == b'\\' {
            cursor = cursor.saturating_add(2);
            continue;
        }
        if bytes[cursor] == quote {
            return Some(cursor.saturating_add(1));
        }
        cursor += 1;
    }
    Some(bytes.len())
}

fn lifetime_without_closing_quote(bytes: &[u8], start: usize) -> bool {
    let Some(next) = bytes.get(start.saturating_add(1)).copied() else {
        return false;
    };
    if !rust_identifier_continue(next) {
        return false;
    }
    let end = rust_identifier_end(bytes, start.saturating_add(1));
    bytes.get(end) != Some(&b'\'')
}

const fn rust_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

const fn rust_identifier_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn rust_identifier_end(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start.saturating_add(1);
    while bytes.get(cursor).copied().is_some_and(rust_identifier_continue) {
        cursor += 1;
    }
    cursor
}

fn raw_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut cursor = start;
    if bytes.get(cursor) == Some(&b'b') {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'r') {
        return None;
    }
    cursor += 1;
    let hashes_start = cursor;
    while bytes.get(cursor) == Some(&b'#') {
        cursor += 1;
    }
    let hashes = cursor - hashes_start;
    if bytes.get(cursor) != Some(&b'"') {
        return None;
    }
    cursor += 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'"'
            && (0..hashes)
                .all(|offset| bytes.get(cursor + 1 + offset) == Some(&b'#'))
        {
            return Some(cursor + 1 + hashes);
        }
        cursor += 1;
    }
    Some(bytes.len())
}

#[derive(Debug)]
struct PackageContext {
    directory: PathBuf,
    owners: Vec<SourceOwner>,
}

fn workspace_packages(
    root: &Path,
    metadata: &Metadata,
    files: &[ClassifiedFile],
) -> BTreeMap<String, PackageContext> {
    metadata
        .workspace_members
        .iter()
        .filter_map(|id| {
            let package = metadata.packages.get(id)?;
            let manifest = package.manifest_path.canonicalize().ok()?;
            let relative = manifest.strip_prefix(root).ok()?;
            let directory = relative
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_path_buf();
            let owners = files
                .iter()
                .filter(|file| file.path.starts_with(&directory))
                .map(|file| file.classification.owner)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            Some((id.clone(), PackageContext { directory, owners }))
        })
        .collect()
}

fn package_for<'a>(
    path: &Path,
    packages: &'a BTreeMap<String, PackageContext>,
) -> Option<(&'a str, SourceOwner)> {
    packages
        .iter()
        .filter(|(_, package)| {
            path.starts_with(&package.directory) && package.owners.len() == 1
        })
        .max_by_key(|(_, package)| package.directory.components().count())
        .map(|(id, package)| (id.as_str(), package.owners[0]))
}

fn dependency_owners(
    package_id: &str,
    metadata: &Metadata,
    packages: &BTreeMap<String, PackageContext>,
) -> BTreeMap<String, Vec<SourceOwner>> {
    metadata
        .nodes
        .get(package_id)
        .into_iter()
        .flat_map(|node| &node.dependencies)
        .filter_map(|dependency| {
            packages.get(&dependency.package).map(|package| {
                (dependency.name.clone(), package.owners.clone())
            })
        })
        .collect()
}

struct AnalysisRequest<'a> {
    syntax: &'a File,
    file: &'a ClassifiedFile,
    package_id: &'a str,
    owner: SourceOwner,
    dependencies: &'a BTreeMap<String, Vec<SourceOwner>>,
    policy: &'a Policy,
    profile: &'a str,
    root: &'a Path,
    metadata: &'a Metadata,
    inventory: &'a BTreeMap<PathBuf, &'a ClassifiedFile>,
    product_registry: &'a product_closures::Registry,
}

fn analyze_file(
    request: &AnalysisRequest<'_>,
    reexports: &mut product_reexport::Resolver,
    findings: &mut Findings,
) {
    let syntax = request.syntax;
    let file = request.file;
    let package_id = request.package_id;
    let owner = request.owner;
    let dependencies = request.dependencies;
    let policy = request.policy;
    let profile = request.profile;
    let root = request.root;
    let metadata = request.metadata;
    let inventory = request.inventory;
    let product_registry = request.product_registry;
    let product = matches!(owner, SourceOwner::Product(_));
    let path = display_relative(&file.path);
    let boundary = BoundaryContext {
        consumer: owner,
        dependencies,
        policy,
        profile,
        path: &path,
    };
    let mut imports = Vec::new();
    let mut aliases = BTreeMap::new();
    collect_imports(&syntax.items, &mut imports, &mut aliases);
    for import in imports {
        check_boundary(&boundary, "SOURCE_IMPORT", &import, findings);
    }
    if product {
        product_reexport::check(
            &product_reexport::Request {
                root,
                profile,
                metadata,
                inventory,
                registry: product_registry,
                package_id,
                source: file,
                syntax,
            },
            reexports,
            findings,
        );
        let mut gaps = GapVisitor { path: &path, findings };
        gaps.visit_file(syntax);
    }
    let mut public_types = Vec::new();
    collect_public_types(&syntax.items, &mut public_types);
    if file.classification.provenance == SourceProvenance::Generated
        && !public_types.is_empty()
    {
        findings.push(Finding::new(
            "public_type",
            "GENERATED_PUBLIC_TYPE_PROOF_GAP",
            Some(path.clone()),
            concat!(
                "generated source contributes public type syntax that is not ",
                "resolved after expansion"
            ),
        ));
    }
    for mut first in public_types {
        if let Some(original) = aliases.get(&first) {
            first.clone_from(original);
        }
        check_boundary(&boundary, "PUBLIC_TYPE", &first, findings);
    }
}

struct BoundaryContext<'a> {
    consumer: SourceOwner,
    dependencies: &'a BTreeMap<String, Vec<SourceOwner>>,
    policy: &'a Policy,
    profile: &'a str,
    path: &'a str,
}

fn check_boundary(
    context: &BoundaryContext<'_>,
    prefix: &'static str,
    first_segment: &str,
    findings: &mut Findings,
) {
    let Some(providers) = context.dependencies.get(first_segment) else {
        return;
    };
    for provider in providers {
        let Some(code) =
            boundary_code(prefix, context.consumer, *provider, context.policy)
        else {
            continue;
        };
        findings.push(Finding::new(
            boundary_category(prefix),
            code,
            Some(context.path.to_owned()),
            format!(
                "profile {}: {first_segment} crosses {} -> {provider}",
                context.profile, context.consumer
            ),
        ));
    }
}

fn boundary_code(
    prefix: &str,
    consumer: SourceOwner,
    provider: SourceOwner,
    policy: &Policy,
) -> Option<&'static str> {
    match (consumer, provider) {
        (SourceOwner::RepositoryTooling, SourceOwner::RepositoryTooling) => {
            None
        }
        (SourceOwner::Product(left), SourceOwner::Product(right))
            if left == right =>
        {
            None
        }
        (SourceOwner::Product(left), SourceOwner::Product(right))
            if policy.allowed_product_edges.contains(&(left, right)) =>
        {
            None
        }
        (SourceOwner::Product(_), SourceOwner::Product(_)) => {
            Some(product_edge_code(prefix))
        }
        _ => Some(partition_code(prefix)),
    }
}

fn boundary_category(prefix: &str) -> &'static str {
    if prefix == "PUBLIC_TYPE" { "public_type" } else { "source" }
}

fn product_edge_code(prefix: &str) -> &'static str {
    if prefix == "PUBLIC_TYPE" {
        "PUBLIC_TYPE_PRODUCT_EDGE"
    } else {
        "SOURCE_IMPORT_PRODUCT_EDGE"
    }
}

fn partition_code(prefix: &str) -> &'static str {
    if prefix == "PUBLIC_TYPE" {
        "PUBLIC_TYPE_PARTITION"
    } else {
        "SOURCE_IMPORT_PARTITION"
    }
}

fn collect_imports(
    items: &[Item],
    imports: &mut Vec<String>,
    aliases: &mut BTreeMap<String, String>,
) {
    for item in items {
        collect_item_imports(item, imports, aliases);
    }
}

fn collect_item_imports(
    item: &Item,
    imports: &mut Vec<String>,
    aliases: &mut BTreeMap<String, String>,
) {
    match item {
        Item::Use(item) => flatten_use(&item.tree, None, imports, aliases),
        Item::ExternCrate(item) => {
            let original = item.ident.to_string();
            imports.push(original.clone());
            if let Some((_, rename)) = &item.rename {
                aliases.insert(rename.to_string(), original);
            }
        }
        Item::Mod(item) => {
            if let Some((_, nested)) = &item.content {
                collect_imports(nested, imports, aliases);
            }
        }
        _ => {}
    }
}

fn flatten_use(
    tree: &UseTree,
    first: Option<String>,
    imports: &mut Vec<String>,
    aliases: &mut BTreeMap<String, String>,
) {
    match tree {
        UseTree::Path(path) => {
            let first = first.unwrap_or_else(|| path.ident.to_string());
            flatten_use(&path.tree, Some(first), imports, aliases);
        }
        UseTree::Name(name) => {
            imports.push(first.unwrap_or_else(|| name.ident.to_string()));
        }
        UseTree::Rename(rename) => {
            let original = first.unwrap_or_else(|| rename.ident.to_string());
            aliases.insert(rename.rename.to_string(), original.clone());
            imports.push(original);
        }
        UseTree::Glob(_) => {
            if let Some(first) = first {
                imports.push(first);
            }
        }
        UseTree::Group(group) => {
            for tree in &group.items {
                flatten_use(tree, first.clone(), imports, aliases);
            }
        }
    }
}

fn collect_public_types(items: &[Item], output: &mut Vec<String>) {
    for item in items {
        collect_public_item_types(item, output);
    }
}

fn collect_public_item_types(item: &Item, output: &mut Vec<String>) {
    match item {
        Item::Fn(item) if public(&item.vis) => {
            collect_signature(&item.sig, output);
        }
        Item::Struct(item) if public(&item.vis) => {
            collect_fields(&item.fields, true, output);
        }
        Item::Enum(item) if public(&item.vis) => {
            for variant in &item.variants {
                collect_fields(&variant.fields, false, output);
            }
        }
        Item::Type(item) if public(&item.vis) => {
            collect_type(&item.ty, output);
        }
        Item::Const(item) if public(&item.vis) => {
            collect_type(&item.ty, output);
        }
        Item::Static(item) if public(&item.vis) => {
            collect_type(&item.ty, output);
        }
        Item::Trait(item) if public(&item.vis) => {
            collect_trait_types(item, output);
        }
        Item::Impl(item) => collect_impl_types(item, output),
        Item::Mod(item) if public(&item.vis) => {
            if let Some((_, nested)) = &item.content {
                collect_public_types(nested, output);
            }
        }
        _ => {}
    }
}

fn collect_signature(signature: &syn::Signature, output: &mut Vec<String>) {
    for argument in &signature.inputs {
        if let syn::FnArg::Typed(argument) = argument {
            collect_type(&argument.ty, output);
        }
    }
    if let ReturnType::Type(_, ty) = &signature.output {
        collect_type(ty, output);
    }
}

fn collect_fields(
    fields: &syn::Fields,
    public_only: bool,
    output: &mut Vec<String>,
) {
    for field in fields {
        if !public_only || public(&field.vis) {
            collect_type(&field.ty, output);
        }
    }
}

fn collect_trait_types(item: &syn::ItemTrait, output: &mut Vec<String>) {
    for supertrait in &item.supertraits {
        collect_trait_bound(supertrait, output);
    }
    for item in &item.items {
        collect_trait_item_types(item, output);
    }
}

fn collect_trait_item_types(item: &syn::TraitItem, output: &mut Vec<String>) {
    match item {
        syn::TraitItem::Fn(item) => collect_signature(&item.sig, output),
        syn::TraitItem::Type(item) => {
            for bound in &item.bounds {
                collect_trait_bound(bound, output);
            }
        }
        _ => {}
    }
}

fn collect_trait_bound(bound: &syn::TypeParamBound, output: &mut Vec<String>) {
    let syn::TypeParamBound::Trait(bound) = bound else {
        return;
    };
    if let Some(segment) = bound.path.segments.first() {
        output.push(segment.ident.to_string());
    }
}

fn collect_impl_types(item: &syn::ItemImpl, output: &mut Vec<String>) {
    for item in &item.items {
        let ImplItem::Fn(function) = item else {
            continue;
        };
        if public(&function.vis) {
            collect_signature(&function.sig, output);
        }
    }
}

fn collect_type(ty: &Type, output: &mut Vec<String>) {
    TypeCollector(output).visit_type(ty);
}

struct TypeCollector<'a>(&'a mut Vec<String>);

impl<'ast> Visit<'ast> for TypeCollector<'_> {
    fn visit_type_path(&mut self, ty: &'ast syn::TypePath) {
        if ty.qself.is_none()
            && let Some(segment) = ty.path.segments.first()
        {
            self.0.push(segment.ident.to_string());
        }
        visit::visit_type_path(self, ty);
    }
}

const fn public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

struct GapVisitor<'a, 'b> {
    path: &'a str,
    findings: &'b mut Findings,
}

impl GapVisitor<'_, '_> {
    fn attributes(&mut self, attributes: &[Attribute]) {
        for _ in attributes.iter().filter(|attribute| cfg_attribute(attribute))
        {
            self.findings.push(Finding::new(
                "source",
                "UNSUPPORTED_CFG_PROOF_GAP",
                Some(self.path.to_owned()),
                concat!(
                    "cfg-dependent product syntax requires expanded source ",
                    "evidence"
                ),
            ));
        }
    }
}

fn cfg_attribute(attribute: &Attribute) -> bool {
    attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr")
}

impl<'ast> Visit<'ast> for GapVisitor<'_, '_> {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        self.attributes(std::slice::from_ref(attribute));
    }

    fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
        let code = if item.mac.path.is_ident("include") {
            "UNSUPPORTED_INCLUDE_PROOF_GAP"
        } else {
            "UNSUPPORTED_MACRO_PROOF_GAP"
        };
        self.findings.push(Finding::new(
            "source",
            code,
            Some(self.path.to_owned()),
            "product item macro cannot be resolved by syntax-only analysis",
        ));
        visit::visit_item_macro(self, item);
    }

    fn visit_expr_macro(&mut self, expression: &'ast syn::ExprMacro) {
        if expression.mac.path.is_ident("include")
            || expression.mac.path.is_ident("include_bytes")
            || expression.mac.path.is_ident("include_str")
        {
            self.findings.push(Finding::new(
                "source",
                "UNSUPPORTED_INCLUDE_PROOF_GAP",
                Some(self.path.to_owned()),
                "include macro cannot be resolved by syntax-only analysis",
            ));
        }
        visit::visit_expr_macro(self, expression);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        self.findings.push(Finding::new(
            "public_type",
            "UNSUPPORTED_EXTERN_CRATE_PROOF_GAP",
            Some(self.path.to_owned()),
            concat!(
                "extern crate can shadow an extern-prelude dependency name ",
                "and is outside the Product source profile"
            ),
        ));
        visit::visit_item_extern_crate(self, item);
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        if public(&item.vis) {
            self.findings.push(Finding::new(
                "public_type",
                "UNSUPPORTED_PUBLIC_ALIAS_PROOF_GAP",
                Some(self.path.to_owned()),
                "public type alias requires resolved-name evidence",
            ));
        }
        visit::visit_item_type(self, item);
    }
}

struct NodeCounter {
    count: usize,
    limit: usize,
    exceeded: bool,
}

impl NodeCounter {
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
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use reovim_depgraph::{
        SourceClassification, SourceOwner, SourceProvenance, SourceUse,
    };
    use syn::visit::Visit as _;

    use crate::{
        finding::Findings, source::ClassifiedFile, topology::HARD_LIMITS,
    };

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "reovim-conformance-rust-source-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn add(
            &self,
            relative: &str,
            contents: &[u8],
            source_use: SourceUse,
        ) -> ClassifiedFile {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
            ClassifiedFile {
                path: PathBuf::from(relative),
                content_digest: crate::digest::bytes(contents),
                classification: classification(source_use),
                upstream: None,
                rule_prefix: PathBuf::from(relative),
                rule_precedence: 1,
            }
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn classification(source_use: SourceUse) -> SourceClassification {
        SourceClassification::new(
            SourceOwner::RepositoryTooling,
            SourceProvenance::Authored,
            source_use,
        )
        .unwrap()
    }

    #[test]
    fn product_syntax_proof_gaps_are_typed() {
        let syntax = syn::parse_file(
            r#"
#[cfg(unix)]
pub fn selected() {}
pub use crate::*;
pub type Alias = crate::Thing;
extern crate evil as reovim_foundation;
include!("generated.rs");
"#,
        )
        .unwrap();
        let mut findings = Findings::new(32);
        super::GapVisitor { path: "src/lib.rs", findings: &mut findings }
            .visit_file(&syntax);
        let codes = findings
            .finish()
            .into_iter()
            .map(|finding| finding.code)
            .collect::<std::collections::BTreeSet<_>>();
        for code in [
            "UNSUPPORTED_CFG_PROOF_GAP",
            "UNSUPPORTED_EXTERN_CRATE_PROOF_GAP",
            "UNSUPPORTED_PUBLIC_ALIAS_PROOF_GAP",
            "UNSUPPORTED_INCLUDE_PROOF_GAP",
        ] {
            assert!(codes.contains(code), "{code}");
        }
    }

    #[test]
    fn unsafe_attributes_functions_blocks_and_macro_tokens_are_detected() {
        for source in [
            "unsafe fn f() {}",
            "fn f() { unsafe {} }",
            "#[unsafe(no_mangle)] pub fn f() {}",
            "#[cfg_attr(any(), unsafe(no_mangle))] pub fn f() {}",
            "macro_rules! x { () => { unsafe { call() } } }",
            "macro_rules! x { () => { let _: &'a (); unsafe {} } }",
            "type F = unsafe fn();",
            "unsafe extern \"C\" { fn f(); }",
            "extern \"C\" { unsafe fn f(); }",
        ] {
            let syntax = syn::parse_file(source).unwrap();
            let mut visitor = super::UnsafeVisitor { found: false };
            visitor.visit_file(&syntax);
            assert!(visitor.found, "{source}");
        }
        for source in [
            "fn f() { println!(\"unsafe\"); }",
            "fn f() { println!(r#\"text \\\" unsafe\"#); }",
        ] {
            let syntax = syn::parse_file(source).unwrap();
            let mut visitor = super::UnsafeVisitor { found: false };
            visitor.visit_file(&syntax);
            assert!(!visitor.found, "{source}");
        }
    }

    #[test]
    fn target_attributes_and_build_output_are_fail_closed() {
        let syntax = syn::parse_file("#![no_std]\npub fn f() {}").unwrap();
        let mut freestanding = super::FreestandingVisitor { found: false };
        freestanding.visit_file(&syntax);
        assert!(freestanding.found);
        let nested = syn::parse_file("mod stale { #![no_main] }").unwrap();
        let mut freestanding = super::FreestandingVisitor { found: false };
        freestanding.visit_file(&nested);
        assert!(freestanding.found);
        let macro_hidden =
            syn::parse_file("macro_rules! stale { () => { #![no_std] } }")
                .unwrap();
        let mut freestanding = super::FreestandingVisitor { found: false };
        freestanding.visit_file(&macro_hidden);
        assert!(freestanding.found);
        for source in
            ["#![cfg_attr(any(), no_std)]", "#![cfg_attr(any(), no_main)]"]
        {
            let syntax = syn::parse_file(source).unwrap();
            let mut freestanding = super::FreestandingVisitor { found: false };
            freestanding.visit_file(&syntax);
            assert!(freestanding.found, "{source}");
        }
        let mut findings = Findings::new(8);
        super::validate_crate_attributes(
            &syntax,
            std::path::Path::new("src/lib.rs"),
            &mut findings,
        );
        assert!(
            findings
                .finish()
                .iter()
                .any(|finding| finding.code == "TARGET_FORBID_UNSAFE_MISSING")
        );

        let file = ClassifiedFile {
            path: PathBuf::from("src/generated.rs"),
            content_digest: crate::digest::bytes(b""),
            classification: SourceClassification::new(
                SourceOwner::RepositoryTooling,
                SourceProvenance::Generated,
                SourceUse::BuildOutput,
            )
            .unwrap(),
            upstream: None,
            rule_prefix: PathBuf::from("src/generated.rs"),
            rule_precedence: 1,
        };
        let mut findings = Findings::new(8);
        assert!(super::reject_build_output(&file, "test", &mut findings));
        assert_eq!(findings.finish()[0].code, "RUST_SOURCE_BUILD_OUTPUT");
    }

    #[test]
    fn module_resolution_is_recursive_conservative_and_rejects_orphans() {
        let root = TempRoot::new();
        let files = vec![
            root.add(
                "src/entry.rs",
                concat!(
                    "mod foo; mod nested; mod inline { mod bar; } ",
                    "#[cfg(any())] mod cfg_only;"
                )
                .as_bytes(),
                SourceUse::Tool,
            ),
            root.add("src/foo.rs", b"mod child;", SourceUse::Tool),
            root.add("src/foo/child.rs", b"", SourceUse::Tool),
            root.add("src/nested/mod.rs", b"", SourceUse::Tool),
            root.add("src/inline/bar.rs", b"", SourceUse::Tool),
            root.add("src/cfg_only.rs", b"", SourceUse::Tool),
            root.add("src/orphan.rs", b"", SourceUse::Tool),
            root.add("tests/checker.rs", b"mod helper;", SourceUse::Test),
            root.add("tests/helper.rs", b"", SourceUse::Test),
        ];
        let target_uses = BTreeMap::from([
            (PathBuf::from("src/entry.rs"), BTreeSet::from([SourceUse::Tool])),
            (
                PathBuf::from("tests/checker.rs"),
                BTreeSet::from([SourceUse::Test]),
            ),
        ]);
        let mut findings = Findings::new(64);
        let selected = super::resolve_module_sources(
            &root.0,
            &target_uses,
            &files,
            HARD_LIMITS,
            "default",
            &mut findings,
        );
        for expected in [
            "src/entry.rs",
            "src/foo.rs",
            "src/foo/child.rs",
            "src/nested/mod.rs",
            "src/inline/bar.rs",
            "src/cfg_only.rs",
            "tests/checker.rs",
            "tests/helper.rs",
        ] {
            assert!(selected.contains_key(Path::new(expected)), "{expected}");
        }
        assert!(!selected.contains_key(Path::new("src/orphan.rs")));
        super::report_unregistered_source(
            Path::new("src/orphan.rs"),
            &selected,
            "default",
            &mut findings,
        );
        let values = findings.finish();
        assert_eq!(
            values,
            [crate::finding::Finding::new(
                "source",
                "UNREGISTERED_RUST_SOURCE",
                Some("src/orphan.rs".to_owned()),
                concat!(
                    "profile default: Rust file is not a Cargo target root ",
                    "or reachable module",
                ),
            )]
        );
    }

    #[test]
    fn module_resolution_rejects_path_ambiguity_and_use_drift() {
        let root = TempRoot::new();
        let files = vec![
            root.add(
                "src/lib.rs",
                br#"mod missing; mod duplicate;
#[path = "../tmp/hidden.rs"] mod hidden;
#[cfg_attr(any(), path = "../tmp/other.rs")] mod conditional;
mod wrong_use;"#,
                SourceUse::Tool,
            ),
            root.add("src/duplicate.rs", b"", SourceUse::Tool),
            root.add("src/duplicate/mod.rs", b"", SourceUse::Tool),
            root.add("src/wrong_use.rs", b"", SourceUse::Test),
        ];
        let targets = BTreeMap::from([(
            PathBuf::from("src/lib.rs"),
            BTreeSet::from([SourceUse::Tool]),
        )]);
        let mut findings = Findings::new(64);
        let _ = super::resolve_module_sources(
            &root.0,
            &targets,
            &files,
            HARD_LIMITS,
            "default",
            &mut findings,
        );
        let codes = findings
            .finish()
            .into_iter()
            .map(|finding| finding.code)
            .collect::<BTreeSet<_>>();
        for expected in [
            "MODULE_SOURCE_MISSING",
            "MODULE_SOURCE_AMBIGUOUS",
            "UNSUPPORTED_PATH_MODULE",
            "MODULE_SOURCE_CLASSIFICATION",
        ] {
            assert!(codes.contains(expected), "{expected}");
        }
    }

    #[test]
    fn module_use_dominance_is_order_independent_and_nonshipping_mix_fails() {
        for (tool_root, test_root) in
            [("src/a.rs", "src/z.rs"), ("src/z.rs", "src/a.rs")]
        {
            let root = TempRoot::new();
            let files = vec![
                root.add(tool_root, b"mod shared;", SourceUse::Tool),
                root.add(test_root, b"mod shared;", SourceUse::Test),
                root.add("src/shared.rs", b"", SourceUse::Tool),
            ];
            let targets = BTreeMap::from([
                (PathBuf::from(tool_root), BTreeSet::from([SourceUse::Tool])),
                (PathBuf::from(test_root), BTreeSet::from([SourceUse::Test])),
            ]);
            let mut findings = Findings::new(64);
            let selected = super::resolve_module_sources(
                &root.0,
                &targets,
                &files,
                HARD_LIMITS,
                "default",
                &mut findings,
            );
            assert_eq!(
                selected[Path::new("src/shared.rs")],
                BTreeSet::from([SourceUse::Tool, SourceUse::Test])
            );
            assert_eq!(findings.finish(), Vec::new());
        }

        let root = TempRoot::new();
        let files = vec![
            root.add("src/test.rs", b"mod shared;", SourceUse::Test),
            root.add("src/example.rs", b"mod shared;", SourceUse::Example),
            root.add("src/shared.rs", b"", SourceUse::Test),
        ];
        let targets = BTreeMap::from([
            (PathBuf::from("src/test.rs"), BTreeSet::from([SourceUse::Test])),
            (
                PathBuf::from("src/example.rs"),
                BTreeSet::from([SourceUse::Example]),
            ),
        ]);
        let mut findings = Findings::new(64);
        let _ = super::resolve_module_sources(
            &root.0,
            &targets,
            &files,
            HARD_LIMITS,
            "default",
            &mut findings,
        );
        assert!(
            findings
                .finish()
                .iter()
                .any(|finding| finding.code == "MODULE_SOURCE_USE_AMBIGUOUS")
        );
    }

    #[test]
    fn include_use_compatibility_follows_tool_precedence() {
        for (consumer, input, accepted) in [
            (SourceUse::Tool, SourceUse::Tool, true),
            (SourceUse::Tool, SourceUse::Test, false),
            (SourceUse::Test, SourceUse::Tool, true),
            (SourceUse::Test, SourceUse::Test, true),
            (SourceUse::Example, SourceUse::Tool, true),
            (SourceUse::Example, SourceUse::Example, true),
            (SourceUse::Test, SourceUse::Example, false),
            (SourceUse::Example, SourceUse::Test, false),
            (SourceUse::Tool, SourceUse::BuildOutput, false),
            (SourceUse::Test, SourceUse::BuildOutput, false),
            (SourceUse::Example, SourceUse::BuildOutput, false),
        ] {
            assert_eq!(
                super::include_use_compatible(consumer, input),
                accepted
            );
        }
    }

    #[test]
    fn literal_includes_bind_classified_inputs_and_mutants_fail_closed() {
        let root = TempRoot::new();
        let source = root.add("src/lib.rs", b"", SourceUse::Tool);
        let tool = root.add("data.txt", b"retained", SourceUse::Tool);
        let test = root.add("test-only.txt", b"test", SourceUse::Test);
        let files = [source, tool, test];
        let inventory =
            files.iter().map(|file| (file.path.clone(), file)).collect();

        let positive = syn::parse_file(
            r#"const A: &str = include_str!("../data.txt");
const B: &[u8] = include_bytes!("../data.txt");"#,
        )
        .unwrap();
        let mut findings = Findings::new(64);
        super::IncludeVisitor {
            root: &root.0,
            source_path: Path::new("src/lib.rs"),
            source_classification: classification(SourceUse::Tool),
            inventory: &inventory,
            file_byte_limit: HARD_LIMITS.source_file_bytes,
            findings: &mut findings,
        }
        .visit_file(&positive);
        assert_eq!(findings.finish(), Vec::new());

        let negative = syn::parse_file(
            r#"const A: &str = include_str!("../../escape");
const B: &str = include_str!("../missing");
const C: &str = include_str!(concat!("../", "data.txt"));
const D: &str = include_str!("../test-only.txt");
include!("../code.rs");
macro_rules! hidden { () => { include_bytes!("../data.txt") } }
use std::include_str as load;"#,
        )
        .unwrap();
        let mut findings = Findings::new(64);
        super::IncludeVisitor {
            root: &root.0,
            source_path: Path::new("src/lib.rs"),
            source_classification: classification(SourceUse::Tool),
            inventory: &inventory,
            file_byte_limit: HARD_LIMITS.source_file_bytes,
            findings: &mut findings,
        }
        .visit_file(&negative);
        let codes = findings
            .finish()
            .into_iter()
            .map(|finding| finding.code)
            .collect::<BTreeSet<_>>();
        for expected in [
            "INCLUDE_PATH_ESCAPE",
            "INCLUDE_TARGET_UNCLASSIFIED",
            "INCLUDE_PATH_NOT_LITERAL",
            "INCLUDE_TARGET_CLASSIFICATION",
            "INCLUDE_RUST_PROOF_UNAVAILABLE",
            "INCLUDE_MACRO_PROOF_UNAVAILABLE",
            "INCLUDE_ALIAS_PROOF_UNAVAILABLE",
        ] {
            assert!(codes.contains(expected), "{expected}");
        }

        let shadowed = syn::parse_file(
            r#"const A: &str = arbitrary::include_str!("../data.txt");
use arbitrary::include_str;
use arbitrary::*;
#[macro_use] extern crate arbitrary;"#,
        )
        .unwrap();
        let mut findings = Findings::new(64);
        super::IncludeVisitor {
            root: &root.0,
            source_path: Path::new("src/lib.rs"),
            source_classification: classification(SourceUse::Tool),
            inventory: &inventory,
            file_byte_limit: HARD_LIMITS.source_file_bytes,
            findings: &mut findings,
        }
        .visit_file(&shadowed);
        let codes = findings
            .finish()
            .into_iter()
            .map(|finding| finding.code)
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("INCLUDE_QUALIFIED_PROOF_UNAVAILABLE"));
        assert!(codes.contains("INCLUDE_ALIAS_PROOF_UNAVAILABLE"));
    }
}
