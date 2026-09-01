//! Exact Product shipping, validation, package, and public-consumer closures.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use reovim_depgraph::{ArchitecturalRole, SourceUse};
use serde_json::Value;

use crate::{
    json::{self, ConfigError},
    policy::relative_path,
};

pub const SCHEMA: &str = "reovim-product-closures/v1";
pub const PRODUCT_PROOF_PROFILE: &str = "authored-std-lib/v1";
pub const TARGET_TRIPLE: &str = "x86_64-unknown-linux-gnu";

/// Caller-owned limits applied after the bounded JSON loader has constructed a
/// value. Keeping them explicit prevents direct parser use from bypassing the
/// topology's tighter Product limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    pub path_bytes: usize,
    pub closure_items: usize,
    pub package_members: usize,
    pub public_items: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    pub closures: Vec<Closure>,
    pub public_consumer_relations: Vec<PublicConsumerRelation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Closure {
    pub id: String,
    pub kind: ClosureKind,
    pub workspace: WorkspaceRef,
    pub package: PackageRef,
    pub target: TargetRef,
    pub architectural_role: ArchitecturalRole,
    pub source_use: SourceUse,
    pub cargo: CargoSelection,
    pub package_membership: Option<PackageMembership>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClosureKind {
    Shipping,
    Validation,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkspaceRef {
    pub id: String,
    pub manifest: PathBuf,
    pub lockfile: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageRef {
    pub id: String,
    pub name: String,
    pub manifest: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TargetRef {
    pub id: String,
    pub name: String,
    pub src_path: PathBuf,
    pub kind: Vec<String>,
    pub crate_types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CargoSelection {
    pub profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageMembership {
    pub members: Vec<PackageMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageMember {
    Copied { package_path: PathBuf, repository_path: PathBuf },
    CargoGenerated { package_path: PathBuf },
}

impl PackageMember {
    #[must_use]
    pub const fn package_path(&self) -> &PathBuf {
        match self {
            Self::Copied { package_path, .. } | Self::CargoGenerated { package_path } => {
                package_path
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicConsumerRelation {
    pub id: String,
    pub shipping_closure: String,
    pub consumer_closure: String,
    pub public_items: Vec<PublicItem>,
    pub derive_checks: Vec<DeriveCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PublicItem {
    pub path: String,
    pub kind: PublicItemKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PublicItemKind {
    Module,
    Type,
    Trait,
    Function,
    Const,
    Static,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeriveCheck {
    pub derive: BuiltinDerive,
    pub public_type: String,
    pub consumer_operation: ConsumerOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BuiltinDerive {
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConsumerOperation {
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Default,
}

impl Registry {
    /// Parses and semantically closes a registry value already bounded and
    /// duplicate-key checked by the caller's JSON loader.
    pub fn parse(value: &Value, limits: ParseLimits) -> Result<Self, ConfigError> {
        validate_limits(limits)?;
        let object = json::object(
            value,
            "$",
            &["schema", "product_proof_profile", "closures", "public_consumer_relations"],
            &[],
        )?;
        exact(&object["schema"], "$.schema", SCHEMA)?;
        exact(&object["product_proof_profile"], "$.product_proof_profile", PRODUCT_PROOF_PROFILE)?;

        let closure_values = bounded_array(
            &object["closures"],
            "$.closures",
            limits.closure_items,
            "PRODUCT_CLOSURE_LIMIT",
        )?;
        let mut closures = Vec::with_capacity(closure_values.len());
        let mut closure_ids = BTreeSet::new();
        for (item, value) in closure_values.iter().enumerate() {
            let path = json::index("$.closures", item);
            let closure = parse_closure(value, &path, limits)?;
            if !closure_ids.insert(closure.id.clone()) {
                return Err(ConfigError::new(
                    "PRODUCT_CLOSURE_DUPLICATE_ID",
                    json::child(&path, "id"),
                    "duplicate closure ID",
                ));
            }
            closures.push(closure);
        }
        closures.sort_by(|left, right| left.id.cmp(&right.id));

        let relation_values = bounded_array(
            &object["public_consumer_relations"],
            "$.public_consumer_relations",
            limits.closure_items,
            "PRODUCT_RELATION_LIMIT",
        )?;
        let mut public_consumer_relations = Vec::with_capacity(relation_values.len());
        let mut relation_ids = BTreeSet::new();
        for (item, value) in relation_values.iter().enumerate() {
            let path = json::index("$.public_consumer_relations", item);
            let relation = parse_relation(value, &path, limits)?;
            if !relation_ids.insert(relation.id.clone()) {
                return Err(ConfigError::new(
                    "PRODUCT_RELATION_DUPLICATE_ID",
                    json::child(&path, "id"),
                    "duplicate public-consumer relation ID",
                ));
            }
            public_consumer_relations.push(relation);
        }
        public_consumer_relations.sort_by(|left, right| left.id.cmp(&right.id));

        let registry = Self { closures, public_consumer_relations };
        registry.validate_cross_references()?;
        Ok(registry)
    }

    #[allow(clippy::too_many_lines)]
    fn validate_cross_references(&self) -> Result<(), ConfigError> {
        let closures = self
            .closures
            .iter()
            .map(|closure| (closure.id.as_str(), closure))
            .collect::<BTreeMap<_, _>>();

        let mut workspaces = BTreeMap::<&str, &WorkspaceRef>::new();
        let mut workspace_manifests = BTreeMap::<&PathBuf, &str>::new();
        let mut workspace_locks = BTreeMap::<&PathBuf, &str>::new();
        let mut packages = BTreeMap::<&str, &PackageRef>::new();
        let mut package_names = BTreeMap::<&str, &str>::new();
        let mut package_manifests = BTreeMap::<&PathBuf, &str>::new();
        let mut targets = BTreeMap::<(&str, &str), &TargetRef>::new();
        let mut target_sources = BTreeMap::<&PathBuf, (&str, &str)>::new();
        let mut target_closures = BTreeMap::<(&str, &str), &str>::new();
        let mut package_memberships = BTreeMap::<&str, &PackageMembership>::new();

        for closure in &self.closures {
            insert_consistent(
                &mut workspaces,
                closure.workspace.id.as_str(),
                &closure.workspace,
                "PRODUCT_CLOSURE_WORKSPACE_IDENTITY",
                "$.closures",
            )?;
            insert_reverse_unique(
                &mut workspace_manifests,
                &closure.workspace.manifest,
                closure.workspace.id.as_str(),
                "PRODUCT_CLOSURE_WORKSPACE_MANIFEST_ALIAS",
                "$.closures",
            )?;
            insert_reverse_unique(
                &mut workspace_locks,
                &closure.workspace.lockfile,
                closure.workspace.id.as_str(),
                "PRODUCT_CLOSURE_WORKSPACE_LOCK_ALIAS",
                "$.closures",
            )?;
            insert_consistent(
                &mut packages,
                closure.package.id.as_str(),
                &closure.package,
                "PRODUCT_CLOSURE_PACKAGE_IDENTITY",
                "$.closures",
            )?;
            insert_reverse_unique(
                &mut package_names,
                closure.package.name.as_str(),
                closure.package.id.as_str(),
                "PRODUCT_CLOSURE_PACKAGE_NAME_ALIAS",
                "$.closures",
            )?;
            insert_reverse_unique(
                &mut package_manifests,
                &closure.package.manifest,
                closure.package.id.as_str(),
                "PRODUCT_CLOSURE_PACKAGE_MANIFEST_ALIAS",
                "$.closures",
            )?;
            let target_key = (closure.package.id.as_str(), closure.target.id.as_str());
            insert_consistent(
                &mut targets,
                target_key,
                &closure.target,
                "PRODUCT_CLOSURE_TARGET_IDENTITY",
                "$.closures",
            )?;
            insert_reverse_unique(
                &mut target_sources,
                &closure.target.src_path,
                target_key,
                "PRODUCT_CLOSURE_TARGET_SOURCE_ALIAS",
                "$.closures",
            )?;
            insert_reverse_unique(
                &mut target_closures,
                target_key,
                closure.id.as_str(),
                "PRODUCT_CLOSURE_TARGET_REUSED",
                "$.closures",
            )?;
            if let Some(membership) = &closure.package_membership {
                insert_consistent(
                    &mut package_memberships,
                    closure.package.id.as_str(),
                    membership,
                    "PRODUCT_CLOSURE_PACKAGE_MEMBERSHIP_IDENTITY",
                    "$.closures",
                )?;
            }
        }

        let mut shipping_relations = BTreeMap::<&str, &str>::new();
        let mut consumer_relations = BTreeMap::<&str, &str>::new();
        let mut relation_pairs = BTreeSet::new();
        for relation in &self.public_consumer_relations {
            let shipping = closures.get(relation.shipping_closure.as_str()).ok_or_else(|| {
                ConfigError::new(
                    "PRODUCT_RELATION_SHIPPING_UNRESOLVED",
                    "$.public_consumer_relations",
                    format!(
                        "relation {} names absent shipping closure {}",
                        relation.id, relation.shipping_closure
                    ),
                )
            })?;
            if shipping.kind != ClosureKind::Shipping {
                return Err(ConfigError::new(
                    "PRODUCT_RELATION_SHIPPING_KIND",
                    "$.public_consumer_relations",
                    format!("closure {} is not shipping", shipping.id),
                ));
            }
            let consumer = closures.get(relation.consumer_closure.as_str()).ok_or_else(|| {
                ConfigError::new(
                    "PRODUCT_RELATION_CONSUMER_UNRESOLVED",
                    "$.public_consumer_relations",
                    format!(
                        "relation {} names absent consumer closure {}",
                        relation.id, relation.consumer_closure
                    ),
                )
            })?;
            if consumer.kind != ClosureKind::Validation {
                return Err(ConfigError::new(
                    "PRODUCT_RELATION_CONSUMER_KIND",
                    "$.public_consumer_relations",
                    format!("closure {} is not validation", consumer.id),
                ));
            }
            if shipping.workspace != consumer.workspace {
                return Err(ConfigError::new(
                    "PRODUCT_RELATION_WORKSPACE",
                    "$.public_consumer_relations",
                    "shipping and public consumer must belong to the same exact Product workspace",
                ));
            }
            if shipping.package.id == consumer.package.id {
                return Err(ConfigError::new(
                    "PRODUCT_RELATION_EXTERNAL_PACKAGE",
                    "$.public_consumer_relations",
                    "public consumer must cross a distinct package boundary",
                ));
            }
            if !relation_pairs.insert((shipping.id.as_str(), consumer.id.as_str())) {
                return Err(ConfigError::new(
                    "PRODUCT_RELATION_DUPLICATE_PAIR",
                    "$.public_consumer_relations",
                    "duplicate shipping/public-consumer pair",
                ));
            }
            insert_reverse_unique(
                &mut shipping_relations,
                shipping.id.as_str(),
                relation.id.as_str(),
                "PRODUCT_RELATION_MULTIPLE_SHIPPING",
                "$.public_consumer_relations",
            )?;
            insert_reverse_unique(
                &mut consumer_relations,
                consumer.id.as_str(),
                relation.id.as_str(),
                "PRODUCT_RELATION_MULTIPLE_CONSUMER",
                "$.public_consumer_relations",
            )?;
        }

        for closure in &self.closures {
            let present = match closure.kind {
                ClosureKind::Shipping => shipping_relations.contains_key(closure.id.as_str()),
                ClosureKind::Validation => consumer_relations.contains_key(closure.id.as_str()),
            };
            if !present {
                return Err(ConfigError::new(
                    "PRODUCT_CLOSURE_UNRELATED",
                    "$.closures",
                    format!("closure {} has no exact public-consumer relation", closure.id),
                ));
            }
        }
        Ok(())
    }
}

#[allow(clippy::too_many_lines)]
fn parse_closure(value: &Value, path: &str, limits: ParseLimits) -> Result<Closure, ConfigError> {
    let object = json::object(
        value,
        path,
        &[
            "id",
            "kind",
            "workspace",
            "package",
            "target",
            "architectural_role",
            "source_use",
            "cargo",
            "mechanism_profile",
            "package_membership",
        ],
        &[],
    )?;
    let id = id(&object["id"], &json::child(path, "id"))?;
    let kind_path = json::child(path, "kind");
    let kind = match json::string(&object["kind"], &kind_path)? {
        "shipping" => ClosureKind::Shipping,
        "validation" => ClosureKind::Validation,
        other => {
            return Err(ConfigError::new(
                "PRODUCT_CLOSURE_KIND",
                kind_path,
                format!("unknown closure kind {other:?}"),
            ));
        }
    };
    let workspace = parse_workspace(&object["workspace"], &json::child(path, "workspace"), limits)?;
    let package = parse_package(&object["package"], &json::child(path, "package"), limits)?;
    let target = parse_target(&object["target"], &json::child(path, "target"), limits)?;
    if target.kind != ["lib"] || target.crate_types != ["lib"] {
        return Err(ConfigError::new(
            "PRODUCT_CLOSURE_TARGET_PROFILE",
            json::child(path, "target"),
            "authored-std-lib/v1 requires kind [\"lib\"] and crate_types [\"lib\"]",
        ));
    }
    let architectural_role =
        role(&object["architectural_role"], &json::child(path, "architectural_role"))?;
    let source_use_path = json::child(path, "source_use");
    let source_use = match json::string(&object["source_use"], &source_use_path)? {
        "production" => SourceUse::Production,
        "test" => SourceUse::Test,
        other => {
            return Err(ConfigError::new(
                "PRODUCT_CLOSURE_SOURCE_USE",
                source_use_path,
                format!("unsupported Product closure source use {other:?}"),
            ));
        }
    };
    let cargo = parse_cargo(&object["cargo"], &json::child(path, "cargo"), limits)?;
    exact(
        &object["mechanism_profile"],
        &json::child(path, "mechanism_profile"),
        PRODUCT_PROOF_PROFILE,
    )?;
    let package_membership = if object["package_membership"].is_null() {
        None
    } else {
        Some(parse_membership(
            &object["package_membership"],
            &json::child(path, "package_membership"),
            limits,
        )?)
    };
    match kind {
        ClosureKind::Shipping => {
            if source_use != SourceUse::Production {
                return Err(ConfigError::new(
                    "PRODUCT_CLOSURE_SHIPPING_USE",
                    json::child(path, "source_use"),
                    "shipping closure must use production source",
                ));
            }
            if cargo.profile != "release" {
                return Err(ConfigError::new(
                    "PRODUCT_CLOSURE_SHIPPING_PROFILE",
                    json::child(path, "cargo.profile"),
                    "shipping closure must select Cargo release profile",
                ));
            }
            if package_membership.is_none() {
                return Err(ConfigError::new(
                    "PRODUCT_CLOSURE_SHIPPING_MEMBERSHIP",
                    json::child(path, "package_membership"),
                    "shipping closure requires exact Cargo package membership",
                ));
            }
        }
        ClosureKind::Validation => {
            if source_use != SourceUse::Test {
                return Err(ConfigError::new(
                    "PRODUCT_CLOSURE_VALIDATION_USE",
                    json::child(path, "source_use"),
                    "validation closure must use test source",
                ));
            }
            if package_membership.is_some() {
                return Err(ConfigError::new(
                    "PRODUCT_CLOSURE_VALIDATION_MEMBERSHIP",
                    json::child(path, "package_membership"),
                    "validation closure cannot enter shipping package membership",
                ));
            }
        }
    }
    Ok(Closure {
        id,
        kind,
        workspace,
        package,
        target,
        architectural_role,
        source_use,
        cargo,
        package_membership,
    })
}

fn parse_workspace(
    value: &Value,
    path: &str,
    limits: ParseLimits,
) -> Result<WorkspaceRef, ConfigError> {
    let object = json::object(value, path, &["id", "manifest", "lockfile"], &[])?;
    Ok(WorkspaceRef {
        id: id(&object["id"], &json::child(path, "id"))?,
        manifest: path_value(&object["manifest"], &json::child(path, "manifest"), limits)?,
        lockfile: path_value(&object["lockfile"], &json::child(path, "lockfile"), limits)?,
    })
}

fn parse_package(
    value: &Value,
    path: &str,
    limits: ParseLimits,
) -> Result<PackageRef, ConfigError> {
    let object = json::object(value, path, &["id", "name", "manifest"], &[])?;
    Ok(PackageRef {
        id: id(&object["id"], &json::child(path, "id"))?,
        name: text(&object["name"], &json::child(path, "name"))?,
        manifest: path_value(&object["manifest"], &json::child(path, "manifest"), limits)?,
    })
}

fn parse_target(value: &Value, path: &str, limits: ParseLimits) -> Result<TargetRef, ConfigError> {
    let object =
        json::object(value, path, &["id", "name", "src_path", "kind", "crate_types"], &[])?;
    Ok(TargetRef {
        id: id(&object["id"], &json::child(path, "id"))?,
        name: text(&object["name"], &json::child(path, "name"))?,
        src_path: path_value(&object["src_path"], &json::child(path, "src_path"), limits)?,
        kind: string_set(&object["kind"], &json::child(path, "kind"), limits.closure_items, false)?,
        crate_types: string_set(
            &object["crate_types"],
            &json::child(path, "crate_types"),
            limits.closure_items,
            false,
        )?,
    })
}

fn parse_cargo(
    value: &Value,
    path: &str,
    limits: ParseLimits,
) -> Result<CargoSelection, ConfigError> {
    let object = json::object(
        value,
        path,
        &[
            "command",
            "target_triple",
            "profile",
            "all_features",
            "no_default_features",
            "features",
            "expected_artifact_crate_types",
        ],
        &[],
    )?;
    exact(&object["command"], &json::child(path, "command"), "build")?;
    exact(&object["target_triple"], &json::child(path, "target_triple"), TARGET_TRIPLE)?;
    if json::boolean(&object["all_features"], &json::child(path, "all_features"))? {
        return Err(ConfigError::new(
            "PRODUCT_CLOSURE_FEATURE_SELECTION",
            json::child(path, "all_features"),
            "all_features must be false",
        ));
    }
    if json::boolean(&object["no_default_features"], &json::child(path, "no_default_features"))? {
        return Err(ConfigError::new(
            "PRODUCT_CLOSURE_FEATURE_SELECTION",
            json::child(path, "no_default_features"),
            "no_default_features must be false for the default-empty feature profile",
        ));
    }
    let features = string_set(
        &object["features"],
        &json::child(path, "features"),
        limits.closure_items,
        true,
    )?;
    if !features.is_empty() {
        return Err(ConfigError::new(
            "PRODUCT_CLOSURE_FEATURE_SELECTION",
            json::child(path, "features"),
            "authored-std-lib/v1 admits no selected features",
        ));
    }
    let expected = string_set(
        &object["expected_artifact_crate_types"],
        &json::child(path, "expected_artifact_crate_types"),
        limits.closure_items,
        false,
    )?;
    if expected != ["rlib"] {
        return Err(ConfigError::new(
            "PRODUCT_CLOSURE_ARTIFACT_TYPE",
            json::child(path, "expected_artifact_crate_types"),
            "authored-std-lib/v1 requires exactly rlib",
        ));
    }
    Ok(CargoSelection { profile: id(&object["profile"], &json::child(path, "profile"))? })
}

fn parse_membership(
    value: &Value,
    path: &str,
    limits: ParseLimits,
) -> Result<PackageMembership, ConfigError> {
    let object = json::object(value, path, &["mode", "members"], &[])?;
    exact(&object["mode"], &json::child(path, "mode"), "cargo_package_list")?;
    let values = bounded_array(
        &object["members"],
        &json::child(path, "members"),
        limits.package_members,
        "PRODUCT_PACKAGE_MEMBER_LIMIT",
    )?;
    if values.is_empty() {
        return Err(ConfigError::new(
            "PRODUCT_PACKAGE_MEMBERS_EMPTY",
            json::child(path, "members"),
            "shipping package requires at least one expected member",
        ));
    }
    let mut members = Vec::with_capacity(values.len());
    let mut package_paths = BTreeSet::new();
    let mut repository_paths = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let item_path = json::index(&json::child(path, "members"), item);
        let member = parse_member(value, &item_path, limits)?;
        if !package_paths.insert(member.package_path().clone()) {
            return Err(ConfigError::new(
                "PRODUCT_PACKAGE_MEMBER_DUPLICATE_PATH",
                item_path,
                "package path has more than one membership mapping",
            ));
        }
        if let PackageMember::Copied { repository_path, .. } = &member
            && !repository_paths.insert(repository_path.clone())
        {
            return Err(ConfigError::new(
                "PRODUCT_PACKAGE_MEMBER_DUPLICATE_SOURCE",
                item_path,
                "repository source is copied to more than one package path",
            ));
        }
        members.push(member);
    }
    members.sort();
    Ok(PackageMembership { members })
}

fn parse_member(
    value: &Value,
    path: &str,
    limits: ParseLimits,
) -> Result<PackageMember, ConfigError> {
    let discriminant = json::object(value, path, &["kind", "package_path"], &["repository_path"])?;
    let kind_path = json::child(path, "kind");
    match json::string(&discriminant["kind"], &kind_path)? {
        "copied" => {
            let object =
                json::object(value, path, &["kind", "package_path", "repository_path"], &[])?;
            Ok(PackageMember::Copied {
                package_path: path_value(
                    &object["package_path"],
                    &json::child(path, "package_path"),
                    limits,
                )?,
                repository_path: path_value(
                    &object["repository_path"],
                    &json::child(path, "repository_path"),
                    limits,
                )?,
            })
        }
        "cargo_generated" => {
            let object = json::object(value, path, &["kind", "package_path"], &[])?;
            let package_path =
                path_value(&object["package_path"], &json::child(path, "package_path"), limits)?;
            if !matches!(
                package_path.to_str(),
                Some(".cargo_vcs_info.json" | "Cargo.lock" | "Cargo.toml" | "Cargo.toml.orig")
            ) {
                return Err(ConfigError::new(
                    "PRODUCT_PACKAGE_GENERATED_MEMBER",
                    json::child(path, "package_path"),
                    "path is outside the closed Cargo-generated member set",
                ));
            }
            Ok(PackageMember::CargoGenerated { package_path })
        }
        other => Err(ConfigError::new(
            "PRODUCT_PACKAGE_MEMBER_KIND",
            kind_path,
            format!("unknown package-member kind {other:?}"),
        )),
    }
}

fn parse_relation(
    value: &Value,
    path: &str,
    limits: ParseLimits,
) -> Result<PublicConsumerRelation, ConfigError> {
    let object = json::object(
        value,
        path,
        &["id", "shipping_closure", "consumer_closure", "public_items", "derive_checks"],
        &[],
    )?;
    let values = bounded_array(
        &object["public_items"],
        &json::child(path, "public_items"),
        limits.public_items,
        "PRODUCT_PUBLIC_ITEM_LIMIT",
    )?;
    if values.is_empty() {
        return Err(ConfigError::new(
            "PRODUCT_PUBLIC_ITEMS_EMPTY",
            json::child(path, "public_items"),
            "public-consumer relation requires at least one public item",
        ));
    }
    let mut public_items = Vec::with_capacity(values.len());
    let mut item_paths = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let item_path = json::index(&json::child(path, "public_items"), item);
        let public_item = parse_public_item(value, &item_path)?;
        if !item_paths.insert(public_item.path.clone()) {
            return Err(ConfigError::new(
                "PRODUCT_PUBLIC_ITEM_DUPLICATE_PATH",
                item_path,
                "public path is declared more than once",
            ));
        }
        public_items.push(public_item);
    }
    public_items.sort();

    let values = bounded_array(
        &object["derive_checks"],
        &json::child(path, "derive_checks"),
        limits.public_items,
        "PRODUCT_DERIVE_CHECK_LIMIT",
    )?;
    let public_types = public_items
        .iter()
        .filter(|item| item.kind == PublicItemKind::Type)
        .map(|item| item.path.as_str())
        .collect::<BTreeSet<_>>();
    let mut derive_checks = Vec::with_capacity(values.len());
    let mut derive_identities = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let item_path = json::index(&json::child(path, "derive_checks"), item);
        let check = parse_derive_check(value, &item_path)?;
        if !public_types.contains(check.public_type.as_str()) {
            return Err(ConfigError::new(
                "PRODUCT_DERIVE_PUBLIC_TYPE_UNRESOLVED",
                json::child(&item_path, "public_type"),
                "derive check must name a declared public item of kind type",
            ));
        }
        if !derive_identities.insert((check.derive, check.public_type.clone())) {
            return Err(ConfigError::new(
                "PRODUCT_DERIVE_CHECK_DUPLICATE",
                item_path,
                "derive/public-type pair is declared more than once",
            ));
        }
        derive_checks.push(check);
    }
    derive_checks.sort();
    Ok(PublicConsumerRelation {
        id: id(&object["id"], &json::child(path, "id"))?,
        shipping_closure: id(&object["shipping_closure"], &json::child(path, "shipping_closure"))?,
        consumer_closure: id(&object["consumer_closure"], &json::child(path, "consumer_closure"))?,
        public_items,
        derive_checks,
    })
}

fn parse_public_item(value: &Value, path: &str) -> Result<PublicItem, ConfigError> {
    let object = json::object(value, path, &["path", "kind"], &[])?;
    let item_path = public_path(&object["path"], &json::child(path, "path"))?;
    let kind_path = json::child(path, "kind");
    let kind = match json::string(&object["kind"], &kind_path)? {
        "module" => PublicItemKind::Module,
        "type" => PublicItemKind::Type,
        "trait" => PublicItemKind::Trait,
        "function" => PublicItemKind::Function,
        "const" => PublicItemKind::Const,
        "static" => PublicItemKind::Static,
        other => {
            return Err(ConfigError::new(
                "PRODUCT_PUBLIC_ITEM_KIND",
                kind_path,
                format!("unknown public-item kind {other:?}"),
            ));
        }
    };
    Ok(PublicItem { path: item_path, kind })
}

fn parse_derive_check(value: &Value, path: &str) -> Result<DeriveCheck, ConfigError> {
    let object = json::object(value, path, &["derive", "public_type", "consumer_operation"], &[])?;
    let derive_path = json::child(path, "derive");
    let derive = match json::string(&object["derive"], &derive_path)? {
        "Clone" => BuiltinDerive::Clone,
        "Copy" => BuiltinDerive::Copy,
        "Debug" => BuiltinDerive::Debug,
        "PartialEq" => BuiltinDerive::PartialEq,
        "Eq" => BuiltinDerive::Eq,
        "Hash" => BuiltinDerive::Hash,
        "Default" => BuiltinDerive::Default,
        other => {
            return Err(ConfigError::new(
                "PRODUCT_DERIVE_KIND",
                derive_path,
                format!("unsupported derive {other:?}"),
            ));
        }
    };
    let operation_path = json::child(path, "consumer_operation");
    let consumer_operation = match json::string(&object["consumer_operation"], &operation_path)? {
        "clone" => ConsumerOperation::Clone,
        "copy" => ConsumerOperation::Copy,
        "debug" => ConsumerOperation::Debug,
        "partial_eq" => ConsumerOperation::PartialEq,
        "eq" => ConsumerOperation::Eq,
        "hash" => ConsumerOperation::Hash,
        "default" => ConsumerOperation::Default,
        other => {
            return Err(ConfigError::new(
                "PRODUCT_DERIVE_OPERATION",
                operation_path,
                format!("unsupported consumer operation {other:?}"),
            ));
        }
    };
    if !derive_matches_operation(derive, consumer_operation) {
        return Err(ConfigError::new(
            "PRODUCT_DERIVE_OPERATION_MISMATCH",
            json::child(path, "consumer_operation"),
            "consumer operation does not exercise the named derive",
        ));
    }
    Ok(DeriveCheck {
        derive,
        public_type: public_path(&object["public_type"], &json::child(path, "public_type"))?,
        consumer_operation,
    })
}

const fn derive_matches_operation(derive: BuiltinDerive, operation: ConsumerOperation) -> bool {
    matches!(
        (derive, operation),
        (BuiltinDerive::Clone, ConsumerOperation::Clone)
            | (BuiltinDerive::Copy, ConsumerOperation::Copy)
            | (BuiltinDerive::Debug, ConsumerOperation::Debug)
            | (BuiltinDerive::PartialEq, ConsumerOperation::PartialEq)
            | (BuiltinDerive::Eq, ConsumerOperation::Eq)
            | (BuiltinDerive::Hash, ConsumerOperation::Hash)
            | (BuiltinDerive::Default, ConsumerOperation::Default)
    )
}

fn role(value: &Value, path: &str) -> Result<ArchitecturalRole, ConfigError> {
    let value = json::string(value, path)?;
    ArchitecturalRole::ALL.into_iter().find(|role| role.as_str() == value).ok_or_else(|| {
        ConfigError::new(
            "PRODUCT_CLOSURE_ROLE",
            path,
            format!("unknown Product architectural role {value:?}"),
        )
    })
}

fn path_value(value: &Value, path: &str, limits: ParseLimits) -> Result<PathBuf, ConfigError> {
    let value = json::string(value, path)?;
    if value.len() > limits.path_bytes {
        return Err(ConfigError::new(
            "PRODUCT_PATH_LIMIT",
            path,
            format!("path is {} bytes; limit is {}", value.len(), limits.path_bytes),
        ));
    }
    relative_path(value, path, false)
}

fn id(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = json::string(value, path)?;
    if valid_id(value) {
        Ok(value.to_owned())
    } else {
        Err(ConfigError::new(
            "PRODUCT_ID",
            path,
            "expected lower-kebab ID beginning with an ASCII lowercase letter",
        ))
    }
}

fn valid_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.try_fold(false, |previous_hyphen, byte| match byte {
            b'a'..=b'z' | b'0'..=b'9' => Some(false),
            b'-' if !previous_hyphen => Some(true),
            _ => None,
        }) == Some(false)
}

fn public_path(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = json::string(value, path)?;
    if !value.is_empty()
        && value.split("::").all(|segment| {
            segment.bytes().next().is_some_and(|byte| byte == b'_' || byte.is_ascii_alphabetic())
                && segment.bytes().all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
        })
    {
        Ok(value.to_owned())
    } else {
        Err(ConfigError::new(
            "PRODUCT_PUBLIC_PATH",
            path,
            "expected a nonempty ASCII Rust-style path without empty segments",
        ))
    }
}

fn string_set(
    value: &Value,
    path: &str,
    limit: usize,
    allow_empty: bool,
) -> Result<Vec<String>, ConfigError> {
    let values = bounded_array(value, path, limit, "PRODUCT_STRING_SET_LIMIT")?;
    if values.is_empty() && !allow_empty {
        return Err(ConfigError::new(
            "PRODUCT_STRING_SET_EMPTY",
            path,
            "at least one value is required",
        ));
    }
    let mut result = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let item_path = json::index(path, item);
        let value = text(value, &item_path)?;
        if !result.insert(value) {
            return Err(ConfigError::new(
                "PRODUCT_STRING_SET_DUPLICATE",
                item_path,
                "duplicate value",
            ));
        }
    }
    Ok(result.into_iter().collect())
}

fn text(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = json::string(value, path)?;
    if value.is_empty() {
        Err(ConfigError::new("PRODUCT_TEXT_EMPTY", path, "text must be nonempty"))
    } else {
        Ok(value.to_owned())
    }
}

fn exact(value: &Value, path: &str, expected: &str) -> Result<(), ConfigError> {
    let actual = json::string(value, path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(ConfigError::new("CFG_CONST", path, format!("expected {expected:?}")))
    }
}

fn bounded_array<'a>(
    value: &'a Value,
    path: &str,
    limit: usize,
    code: &'static str,
) -> Result<&'a [Value], ConfigError> {
    let values = json::array(value, path)?;
    if values.len() > limit {
        Err(ConfigError::new(
            code,
            path,
            format!("array has {} items; limit is {limit}", values.len()),
        ))
    } else {
        Ok(values)
    }
}

fn validate_limits(limits: ParseLimits) -> Result<(), ConfigError> {
    if limits.path_bytes == 0
        || limits.closure_items == 0
        || limits.package_members == 0
        || limits.public_items == 0
    {
        Err(ConfigError::new(
            "PRODUCT_PARSE_ZERO_LIMIT",
            "$",
            "all Product registry parser limits must be positive",
        ))
    } else {
        Ok(())
    }
}

fn insert_consistent<'a, K, V>(
    map: &mut BTreeMap<K, &'a V>,
    key: K,
    value: &'a V,
    code: &'static str,
    path: &str,
) -> Result<(), ConfigError>
where
    K: Ord + Copy,
    V: PartialEq,
{
    if let Some(existing) = map.get(&key)
        && *existing != value
    {
        return Err(ConfigError::new(code, path, "identity has inconsistent exact definitions"));
    }
    map.insert(key, value);
    Ok(())
}

fn insert_reverse_unique<K, V>(
    map: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    code: &'static str,
    path: &str,
) -> Result<(), ConfigError>
where
    K: Ord,
    V: Ord + Copy,
{
    if let Some(existing) = map.get(&key)
        && *existing != value
    {
        return Err(ConfigError::new(code, path, "one exact identity has multiple aliases"));
    }
    map.insert(key, value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    const LIMITS: super::ParseLimits = super::ParseLimits {
        path_bytes: 4096,
        closure_items: 32,
        package_members: 64,
        public_items: 64,
    };

    fn empty_registry() -> Value {
        json!({
            "schema": super::SCHEMA,
            "product_proof_profile": super::PRODUCT_PROOF_PROFILE,
            "closures": [],
            "public_consumer_relations": []
        })
    }

    fn closure(id: &str, kind: &str, package: &str, target: &str) -> Value {
        let shipping = kind == "shipping";
        json!({
            "id": id,
            "kind": kind,
            "workspace": {
                "id": "product",
                "manifest": "product/Cargo.toml",
                "lockfile": "product/Cargo.lock"
            },
            "package": {
                "id": package,
                "name": package,
                "manifest": format!("{package}/Cargo.toml")
            },
            "target": {
                "id": target,
                "name": target.replace('-', "_"),
                "src_path": format!("{package}/src/lib.rs"),
                "kind": ["lib"],
                "crate_types": ["lib"]
            },
            "architectural_role": if shipping { "Foundation" } else { "Kernel Interface" },
            "source_use": if shipping { "production" } else { "test" },
            "cargo": {
                "command": "build",
                "target_triple": super::TARGET_TRIPLE,
                "profile": "release",
                "all_features": false,
                "no_default_features": false,
                "features": [],
                "expected_artifact_crate_types": ["rlib"]
            },
            "mechanism_profile": super::PRODUCT_PROOF_PROFILE,
            "package_membership": if shipping {
                json!({
                    "mode": "cargo_package_list",
                    "members": [
                        {
                            "kind": "copied",
                            "package_path": "src/lib.rs",
                            "repository_path": format!("{package}/src/lib.rs")
                        },
                        {"kind": "cargo_generated", "package_path": "Cargo.toml"}
                    ]
                })
            } else {
                Value::Null
            }
        })
    }

    fn complete_registry() -> Value {
        let mut value = empty_registry();
        value["closures"] = json!([
            closure("foundation-shipping", "shipping", "kernel-foundation", "library"),
            closure("foundation-consumer", "validation", "foundation-consumer", "library")
        ]);
        value["public_consumer_relations"] = json!([{
            "id": "foundation-public-api",
            "shipping_closure": "foundation-shipping",
            "consumer_closure": "foundation-consumer",
            "public_items": [
                {"path": "Revision", "kind": "type"},
                {"path": "transaction::begin", "kind": "function"}
            ],
            "derive_checks": [
                {"derive": "Copy", "public_type": "Revision", "consumer_operation": "copy"},
                {"derive": "Debug", "public_type": "Revision", "consumer_operation": "debug"}
            ]
        }]);
        value
    }

    #[test]
    fn active_empty_registry_is_valid() {
        let registry = super::Registry::parse(&empty_registry(), LIMITS).unwrap();
        assert!(registry.closures.is_empty());
        assert!(registry.public_consumer_relations.is_empty());
    }

    #[test]
    fn complete_pair_parses_and_sorts_semantic_sets() {
        let registry = super::Registry::parse(&complete_registry(), LIMITS).unwrap();
        assert_eq!(registry.closures.len(), 2);
        assert_eq!(registry.closures[0].id, "foundation-consumer");
        let relation = &registry.public_consumer_relations[0];
        assert_eq!(relation.public_items[0].path, "Revision");
        assert_eq!(relation.derive_checks[0].derive, super::BuiltinDerive::Copy);
    }

    #[test]
    fn schema_unknown_fields_ids_paths_and_limits_fail_closed() {
        let mut schema = empty_registry();
        schema["schema"] = json!("reovim-product-closures/v2");
        assert_eq!(super::Registry::parse(&schema, LIMITS).unwrap_err().code, "CFG_CONST");

        let mut unknown = empty_registry();
        unknown["surprise"] = json!(true);
        assert_eq!(super::Registry::parse(&unknown, LIMITS).unwrap_err().code, "CFG_UNKNOWN_FIELD");

        let mut bad_id = complete_registry();
        bad_id["closures"][0]["id"] = json!("Bad_ID");
        assert_eq!(super::Registry::parse(&bad_id, LIMITS).unwrap_err().code, "PRODUCT_ID");

        let mut bad_path = complete_registry();
        bad_path["closures"][0]["workspace"]["manifest"] = json!("../Cargo.toml");
        assert_eq!(super::Registry::parse(&bad_path, LIMITS).unwrap_err().code, "CFG_PATH_ESCAPE");

        let limits = super::ParseLimits { closure_items: 1, ..LIMITS };
        assert_eq!(
            super::Registry::parse(&complete_registry(), limits).unwrap_err().code,
            "PRODUCT_CLOSURE_LIMIT"
        );
    }

    #[test]
    fn closure_profile_and_kind_laws_fail_closed() {
        let mut target = complete_registry();
        target["closures"][0]["target"]["kind"] = json!(["bin"]);
        assert_eq!(
            super::Registry::parse(&target, LIMITS).unwrap_err().code,
            "PRODUCT_CLOSURE_TARGET_PROFILE"
        );

        let mut shipping_use = complete_registry();
        shipping_use["closures"][0]["source_use"] = json!("test");
        assert_eq!(
            super::Registry::parse(&shipping_use, LIMITS).unwrap_err().code,
            "PRODUCT_CLOSURE_SHIPPING_USE"
        );

        let mut validation_membership = complete_registry();
        validation_membership["closures"][1]["package_membership"] =
            validation_membership["closures"][0]["package_membership"].clone();
        assert_eq!(
            super::Registry::parse(&validation_membership, LIMITS).unwrap_err().code,
            "PRODUCT_CLOSURE_VALIDATION_MEMBERSHIP"
        );

        let mut feature = complete_registry();
        feature["closures"][0]["cargo"]["features"] = json!(["std"]);
        assert_eq!(
            super::Registry::parse(&feature, LIMITS).unwrap_err().code,
            "PRODUCT_CLOSURE_FEATURE_SELECTION"
        );
    }

    #[test]
    fn member_paths_are_one_to_one_and_generated_names_are_closed() {
        let mut duplicate = complete_registry();
        duplicate["closures"][0]["package_membership"]["members"][0]["package_path"] =
            json!("Cargo.toml");
        assert_eq!(
            super::Registry::parse(&duplicate, LIMITS).unwrap_err().code,
            "PRODUCT_PACKAGE_MEMBER_DUPLICATE_PATH"
        );

        let mut generated = complete_registry();
        generated["closures"][0]["package_membership"]["members"][1]["package_path"] =
            json!("README.md");
        assert_eq!(
            super::Registry::parse(&generated, LIMITS).unwrap_err().code,
            "PRODUCT_PACKAGE_GENERATED_MEMBER"
        );
    }

    #[test]
    fn repeated_identities_and_aliases_fail() {
        let mut duplicate = complete_registry();
        let repeated = duplicate["closures"][0].clone();
        duplicate["closures"].as_array_mut().unwrap().push(repeated);
        assert_eq!(
            super::Registry::parse(&duplicate, LIMITS).unwrap_err().code,
            "PRODUCT_CLOSURE_DUPLICATE_ID"
        );

        let mut inconsistent_workspace = complete_registry();
        inconsistent_workspace["closures"][1]["workspace"]["lockfile"] =
            json!("product/Other.lock");
        assert_eq!(
            super::Registry::parse(&inconsistent_workspace, LIMITS).unwrap_err().code,
            "PRODUCT_CLOSURE_WORKSPACE_IDENTITY"
        );

        let mut target_alias = complete_registry();
        target_alias["closures"][0]["target"]["src_path"] =
            target_alias["closures"][1]["target"]["src_path"].clone();
        assert_eq!(
            super::Registry::parse(&target_alias, LIMITS).unwrap_err().code,
            "PRODUCT_CLOSURE_TARGET_SOURCE_ALIAS"
        );
    }

    #[test]
    fn public_relation_references_and_derive_checks_are_exact() {
        let mut missing = complete_registry();
        missing["public_consumer_relations"][0]["shipping_closure"] = json!("absent");
        assert_eq!(
            super::Registry::parse(&missing, LIMITS).unwrap_err().code,
            "PRODUCT_RELATION_SHIPPING_UNRESOLVED"
        );

        let mut same_package = complete_registry();
        same_package["closures"][0]["package"] = same_package["closures"][1]["package"].clone();
        same_package["closures"][0]["target"]["id"] = json!("shipping-library");
        assert_eq!(
            super::Registry::parse(&same_package, LIMITS).unwrap_err().code,
            "PRODUCT_RELATION_EXTERNAL_PACKAGE"
        );

        let mut mismatch = complete_registry();
        mismatch["public_consumer_relations"][0]["derive_checks"][0]["consumer_operation"] =
            json!("clone");
        assert_eq!(
            super::Registry::parse(&mismatch, LIMITS).unwrap_err().code,
            "PRODUCT_DERIVE_OPERATION_MISMATCH"
        );

        let mut unresolved = complete_registry();
        unresolved["public_consumer_relations"][0]["derive_checks"][0]["public_type"] =
            json!("Missing");
        assert_eq!(
            super::Registry::parse(&unresolved, LIMITS).unwrap_err().code,
            "PRODUCT_DERIVE_PUBLIC_TYPE_UNRESOLVED"
        );

        let mut unrelated = complete_registry();
        unrelated["public_consumer_relations"] = json!([]);
        assert_eq!(
            super::Registry::parse(&unrelated, LIMITS).unwrap_err().code,
            "PRODUCT_CLOSURE_UNRELATED"
        );
    }
}
