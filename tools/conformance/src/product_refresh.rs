//! Bounded refresh of one real Product public-contract relation.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    path::{Path, PathBuf},
};

use reovim_depgraph::{ArchitecturalRole, SourceOwner, SourceUse};

use crate::{
    POLICY_KEY, POLICY_SCHEMA_KEY, PRODUCT_CLOSURES_KEY,
    PRODUCT_CLOSURES_SCHEMA_KEY, TOPOLOGY_KEY,
    entrypoint_refresh::{FileUpdate, RefreshEffect, publish_updates},
    json,
    policy::{DigestSubject, Policy},
    product_closures::{
        BuiltinDerive, Closure, ConsumerOperation, DeriveCheck, PackageMember,
        PackageMembership, ParseLimits, PublicConsumerRelation, PublicItem,
        PublicItemKind, PublicReexport, Registry,
    },
    product_source::{
        AnalysisLimits, AnalysisRequest, RelationDerivationRequest,
        derive_kernel_interface_relation,
    },
    schema,
    source::{self, ClassifiedFile, Inventory},
    topology::{HARD_LIMITS, Topology},
};

const RESPONSE_SCHEMA: &str = "reovim-product-contract-refresh/v1";
const RELATION: &str = "kernel-interface-public-api";
const FOUNDATION_SHIPPING: &str = "foundation-shipping";
const FOUNDATION_CONSUMER: &str = "foundation-consumer";
const FOUNDATION_RELATION: &str = "foundation-public-api";
const INTERFACE_SHIPPING: &str = "kernel-interface-shipping";
const INTERFACE_CONSUMER: &str = "kernel-interface-consumer";
const PRODUCT_LOCK: &str = "product/Cargo.lock";

/// Request for one exact Product contract refresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductRefreshRequest {
    /// Absolute canonical repository root.
    pub root: PathBuf,
    /// Exact selected public-consumer relation.
    pub relation: String,
}

/// Applied Product contract refresh summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductRefreshOutcome {
    /// Stable response schema.
    pub schema: &'static str,
    /// Refreshed public-consumer relation.
    pub relation: String,
    /// Derived Product manifest-set identity.
    pub manifest_set_digest: String,
    /// Number of exact shipping package members.
    pub package_members: usize,
    /// Number of observed public items.
    pub public_items: usize,
    /// Number of observed built-in derive checks.
    pub derive_checks: usize,
    /// Number of validated direct re-export origins.
    pub reexports: usize,
}

/// Typed Product refresh failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductRefreshError {
    /// Stable failure category.
    pub code: &'static str,
    /// Human-readable bounded diagnostic.
    pub message: String,
    /// Whether either tracked JSON output may have changed.
    pub effect: RefreshEffect,
}

impl fmt::Display for ProductRefreshError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProductRefreshError {}

/// Refreshes derived Product manifest, membership, and public-surface rows.
///
/// # Errors
///
/// Returns a typed failure for invalid selection, input drift, schema or
/// semantic failure, incomplete external witnesses, candidate drift,
/// publication failure, or incomplete rollback.
pub fn refresh_product_contract(
    request: &ProductRefreshRequest,
) -> Result<ProductRefreshOutcome, ProductRefreshError> {
    validate_request(request)?;
    let topology = read_topology(&request.root)?;
    let policy_path = request.root.join(POLICY_KEY);
    let registry_path = request.root.join(PRODUCT_CLOSURES_KEY);
    let (policy_bytes, policy_value) = read_json(&policy_path)?;
    validate_schema(
        &request.root,
        POLICY_SCHEMA_KEY,
        &policy_value,
        "PRODUCT_REFRESH_POLICY_SCHEMA",
    )?;
    let policy = Policy::parse(&policy_value).map_err(config_error)?;
    let old_inventory = source::inventory(&request.root, &policy, &topology);
    validate_initial_inventory(&old_inventory)?;
    let manifest_set_digest = product_manifest_digest(
        &request.root,
        &policy,
        &old_inventory,
        topology.limits,
    )?;
    let next_policy =
        refresh_policy(&policy_bytes, &policy, &manifest_set_digest)?;
    let next_policy_value = parse_candidate(&next_policy)?;
    validate_schema(
        &request.root,
        POLICY_SCHEMA_KEY,
        &next_policy_value,
        "PRODUCT_REFRESH_POLICY_SCHEMA",
    )?;
    let next_policy_parsed =
        Policy::parse(&next_policy_value).map_err(config_error)?;
    validate_policy_change(&policy, &next_policy_parsed, &manifest_set_digest)?;
    let inventory =
        source::inventory(&request.root, &next_policy_parsed, &topology);
    require_finding_free(&inventory, "refreshed classification policy")?;

    let (registry_bytes, registry_value) = read_json(&registry_path)?;
    validate_schema(
        &request.root,
        PRODUCT_CLOSURES_SCHEMA_KEY,
        &registry_value,
        "PRODUCT_REFRESH_REGISTRY_SCHEMA",
    )?;
    let limits = registry_limits(&topology);
    let registry =
        Registry::parse(&registry_value, limits).map_err(config_error)?;
    let sources = SourceSets::from_inventory(&inventory)?;
    let relation = derive_relation(
        &request.root,
        &registry,
        &sources,
        &inventory.files,
        analysis_limits(&topology),
    )?;
    let membership = derive_membership(&sources.interface_shipping)?;
    let next_registry = refresh_registry(
        &registry_bytes,
        &registry_value,
        &membership,
        &relation,
    )?;
    let next_registry_value = parse_candidate(&next_registry)?;
    validate_schema(
        &request.root,
        PRODUCT_CLOSURES_SCHEMA_KEY,
        &next_registry_value,
        "PRODUCT_REFRESH_REGISTRY_SCHEMA",
    )?;
    let next_registry_parsed =
        Registry::parse(&next_registry_value, limits).map_err(config_error)?;
    validate_registry_change(
        &registry,
        &next_registry_parsed,
        &membership,
        &relation,
    )?;

    let updates = [
        FileUpdate::new(policy_path, policy_bytes, next_policy),
        FileUpdate::new(registry_path, registry_bytes, next_registry),
    ];
    publish_updates(&updates).map_err(publication_error)?;

    Ok(ProductRefreshOutcome {
        schema: RESPONSE_SCHEMA,
        relation: request.relation.clone(),
        manifest_set_digest,
        package_members: membership.members.len(),
        public_items: relation.public_items.len(),
        derive_checks: relation.derive_checks.len(),
        reexports: relation.reexports.len(),
    })
}

fn validate_request(
    request: &ProductRefreshRequest,
) -> Result<(), ProductRefreshError> {
    if request.relation != RELATION {
        return Err(not_applied(
            "PRODUCT_REFRESH_RELATION",
            "only kernel-interface-public-api is admitted",
        ));
    }
    if !request.root.is_absolute() {
        return Err(not_applied(
            "PRODUCT_REFRESH_ROOT",
            "--root must be absolute",
        ));
    }
    let metadata = fs::symlink_metadata(&request.root).map_err(|error| {
        io_error("PRODUCT_REFRESH_ROOT", &request.root, &error)
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(not_applied(
            "PRODUCT_REFRESH_ROOT",
            "--root must name a non-symlink directory",
        ));
    }
    let canonical = request.root.canonicalize().map_err(|error| {
        io_error("PRODUCT_REFRESH_ROOT", &request.root, &error)
    })?;
    if canonical != request.root {
        return Err(not_applied(
            "PRODUCT_REFRESH_ROOT",
            "--root must use its canonical absolute spelling",
        ));
    }
    Ok(())
}

fn read_topology(root: &Path) -> Result<Topology, ProductRefreshError> {
    let path = root.join(TOPOLOGY_KEY);
    let (_, value) = read_json(&path)?;
    Topology::parse(&value).map_err(config_error)
}

fn read_json(
    path: &Path,
) -> Result<(Vec<u8>, serde_json::Value), ProductRefreshError> {
    require_regular(path)?;
    json::read(
        path,
        HARD_LIMITS.config_bytes,
        HARD_LIMITS.json_depth,
        HARD_LIMITS.json_container_items,
    )
    .map_err(config_error)
}

fn validate_schema(
    root: &Path,
    schema_path: &str,
    value: &serde_json::Value,
    code: &'static str,
) -> Result<(), ProductRefreshError> {
    let (_, schema_value) = read_json(&root.join(schema_path))?;
    schema::validate_instance(value, &schema_value)
        .map_err(|error| not_applied(code, error.to_string()))
}

fn require_regular(path: &Path) -> Result<(), ProductRefreshError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error("PRODUCT_REFRESH_INPUT", path, &error))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(not_applied(
            "PRODUCT_REFRESH_INPUT",
            format!("{} is not a regular non-symlink file", path.display()),
        ));
    }
    Ok(())
}

fn validate_initial_inventory(
    inventory: &Inventory,
) -> Result<(), ProductRefreshError> {
    for finding in &inventory.findings {
        let admitted = finding.code == "UPSTREAM_DIGEST_MISMATCH"
            && finding.path.as_deref() == Some(PRODUCT_LOCK);
        if !admitted {
            return Err(not_applied(
                "PRODUCT_REFRESH_INITIAL_FINDING",
                format!(
                    "{} at {:?}: {}",
                    finding.code, finding.path, finding.message
                ),
            ));
        }
    }
    Ok(())
}

fn require_finding_free(
    inventory: &Inventory,
    subject: &str,
) -> Result<(), ProductRefreshError> {
    let Some(finding) = inventory.findings.first() else {
        return Ok(());
    };
    Err(not_applied(
        "PRODUCT_REFRESH_SOURCE_FINDING",
        format!(
            "{subject}: {} at {:?}: {}",
            finding.code, finding.path, finding.message
        ),
    ))
}

fn product_manifest_digest(
    root: &Path,
    policy: &Policy,
    inventory: &Inventory,
    limits: crate::topology::Limits,
) -> Result<String, ProductRefreshError> {
    let rule = exact_policy_rule(policy, Path::new(PRODUCT_LOCK))?;
    let upstream = rule.upstream.as_ref().ok_or_else(|| {
        not_applied(
            "PRODUCT_REFRESH_UPSTREAM",
            "Product lock rule has no generated upstream identity",
        )
    })?;
    let DigestSubject::Paths(paths) = &upstream.digest_subject else {
        return Err(not_applied(
            "PRODUCT_REFRESH_UPSTREAM",
            "Product lock must use an exact path-set digest subject",
        ));
    };
    source::aggregate_digest(root, paths, &inventory.files, limits)
        .map_err(|message| not_applied("PRODUCT_REFRESH_UPSTREAM", message))
}

fn exact_policy_rule<'a>(
    policy: &'a Policy,
    prefix: &Path,
) -> Result<&'a crate::policy::Rule, ProductRefreshError> {
    let mut matches = policy.rules.iter().filter(|rule| rule.prefix == prefix);
    let rule = matches.next().ok_or_else(|| {
        not_applied(
            "PRODUCT_REFRESH_UPSTREAM",
            format!("{} has no exact policy rule", prefix.display()),
        )
    })?;
    if matches.next().is_some() {
        return Err(not_applied(
            "PRODUCT_REFRESH_UPSTREAM",
            format!("{} has duplicate policy rules", prefix.display()),
        ));
    }
    Ok(rule)
}

fn refresh_policy(
    bytes: &[u8],
    policy: &Policy,
    digest: &str,
) -> Result<Vec<u8>, ProductRefreshError> {
    let rule = exact_policy_rule(policy, Path::new(PRODUCT_LOCK))?;
    let old = rule.upstream.as_ref().ok_or_else(|| {
        not_applied(
            "PRODUCT_REFRESH_UPSTREAM",
            "Product lock rule has no upstream record",
        )
    })?;
    let needle = format!("\"digest\": \"{}\"", old.digest);
    let replacement = format!("\"digest\": \"{digest}\"");
    replace_once(bytes, &needle, &replacement, "PRODUCT_REFRESH_POLICY")
}

fn validate_policy_change(
    old: &Policy,
    candidate: &Policy,
    digest: &str,
) -> Result<(), ProductRefreshError> {
    let mut expected = old.clone();
    let rule = expected
        .rules
        .iter_mut()
        .find(|rule| rule.prefix == Path::new(PRODUCT_LOCK))
        .ok_or_else(|| {
            not_applied(
                "PRODUCT_REFRESH_POLICY_CANDIDATE",
                "Product lock policy rule disappeared",
            )
        })?;
    let upstream = rule.upstream.as_mut().ok_or_else(|| {
        not_applied(
            "PRODUCT_REFRESH_POLICY_CANDIDATE",
            "Product lock upstream record disappeared",
        )
    })?;
    digest.clone_into(&mut upstream.digest);
    if candidate != &expected {
        return Err(not_applied(
            "PRODUCT_REFRESH_POLICY_CANDIDATE",
            "candidate changed fields outside the derived upstream digest",
        ));
    }
    Ok(())
}

struct SourceSets {
    foundation_shipping: BTreeSet<PathBuf>,
    foundation_consumer: BTreeSet<PathBuf>,
    interface_shipping: BTreeSet<PathBuf>,
    interface_consumer: BTreeSet<PathBuf>,
}

impl SourceSets {
    fn from_inventory(
        inventory: &Inventory,
    ) -> Result<Self, ProductRefreshError> {
        let sets = Self {
            foundation_shipping: select_sources(
                inventory,
                "kernel/foundation/src",
                ArchitecturalRole::Foundation,
                SourceUse::Production,
            ),
            foundation_consumer: select_sources(
                inventory,
                "tests/foundation-public-consumer/src",
                ArchitecturalRole::KernelInterface,
                SourceUse::Test,
            ),
            interface_shipping: select_sources(
                inventory,
                "kernel/interface/src",
                ArchitecturalRole::KernelInterface,
                SourceUse::Production,
            ),
            interface_consumer: select_sources(
                inventory,
                "tests/kernel-interface-public-consumer/src",
                ArchitecturalRole::KernelRuntime,
                SourceUse::Test,
            ),
        };
        if sets.foundation_shipping.is_empty()
            || sets.foundation_consumer.is_empty()
            || sets.interface_shipping.is_empty()
            || sets.interface_consumer.is_empty()
        {
            return Err(not_applied(
                "PRODUCT_REFRESH_SOURCE_SET",
                "one exact Product source set is empty",
            ));
        }
        Ok(sets)
    }
}

fn select_sources(
    inventory: &Inventory,
    prefix: &str,
    role: ArchitecturalRole,
    source_use: SourceUse,
) -> BTreeSet<PathBuf> {
    let prefix = Path::new(prefix);
    inventory
        .files
        .iter()
        .filter(|file| file.path.starts_with(prefix))
        .filter(|file| file.path.extension().is_some_and(|value| value == "rs"))
        .filter(|file| {
            file.classification.owner == SourceOwner::Product(role)
                && file.classification.source_use == source_use
        })
        .map(|file| file.path.clone())
        .collect()
}

fn derive_relation(
    root: &Path,
    registry: &Registry,
    sources: &SourceSets,
    inventory: &[ClassifiedFile],
    limits: AnalysisLimits,
) -> Result<PublicConsumerRelation, ProductRefreshError> {
    let foundation_shipping = exact_closure(registry, FOUNDATION_SHIPPING)?;
    let foundation_consumer = exact_closure(registry, FOUNDATION_CONSUMER)?;
    let foundation_relation = exact_relation(registry, FOUNDATION_RELATION)?;
    let interface_shipping = exact_closure(registry, INTERFACE_SHIPPING)?;
    let interface_consumer = exact_closure(registry, INTERFACE_CONSUMER)?;
    let interface_relation = exact_relation(registry, RELATION)?;
    let request = RelationDerivationRequest {
        foundation: AnalysisRequest {
            root,
            shipping: foundation_shipping,
            consumer: foundation_consumer,
            relation: foundation_relation,
            shipping_sources: &sources.foundation_shipping,
            consumer_sources: &sources.foundation_consumer,
            inventory,
            limits,
        },
        kernel_interface: AnalysisRequest {
            root,
            shipping: interface_shipping,
            consumer: interface_consumer,
            relation: interface_relation,
            shipping_sources: &sources.interface_shipping,
            consumer_sources: &sources.interface_consumer,
            inventory,
            limits,
        },
    };
    derive_kernel_interface_relation(&request).map_err(|error| {
        not_applied("PRODUCT_REFRESH_SOURCE", error.to_string())
    })
}

fn exact_closure<'a>(
    registry: &'a Registry,
    id: &str,
) -> Result<&'a Closure, ProductRefreshError> {
    let mut matches =
        registry.closures.iter().filter(|closure| closure.id == id);
    let closure = matches.next().ok_or_else(|| {
        not_applied(
            "PRODUCT_REFRESH_CLOSURE",
            format!("closure {id} is absent"),
        )
    })?;
    if matches.next().is_some() {
        return Err(not_applied(
            "PRODUCT_REFRESH_CLOSURE",
            format!("closure {id} is duplicated"),
        ));
    }
    Ok(closure)
}

fn exact_relation<'a>(
    registry: &'a Registry,
    id: &str,
) -> Result<&'a PublicConsumerRelation, ProductRefreshError> {
    let mut matches = registry
        .public_consumer_relations
        .iter()
        .filter(|relation| relation.id == id);
    let relation = matches.next().ok_or_else(|| {
        not_applied(
            "PRODUCT_REFRESH_RELATION",
            format!("relation {id} is absent"),
        )
    })?;
    if matches.next().is_some() {
        return Err(not_applied(
            "PRODUCT_REFRESH_RELATION",
            format!("relation {id} is duplicated"),
        ));
    }
    Ok(relation)
}

fn derive_membership(
    sources: &BTreeSet<PathBuf>,
) -> Result<PackageMembership, ProductRefreshError> {
    let package_root = Path::new("kernel/interface");
    let mut members = vec![
        PackageMember::CargoGenerated {
            package_path: PathBuf::from("Cargo.lock"),
        },
        PackageMember::CargoGenerated {
            package_path: PathBuf::from("Cargo.toml"),
        },
        PackageMember::CargoGenerated {
            package_path: PathBuf::from("Cargo.toml.orig"),
        },
    ];
    for repository_path in sources {
        let package_path =
            repository_path.strip_prefix(package_root).map_err(|_| {
                not_applied(
                    "PRODUCT_REFRESH_MEMBERSHIP",
                    format!(
                        "{} is outside {}",
                        repository_path.display(),
                        package_root.display()
                    ),
                )
            })?;
        members.push(PackageMember::Copied {
            package_path: package_path.to_path_buf(),
            repository_path: repository_path.clone(),
        });
    }
    members.sort();
    Ok(PackageMembership { members })
}

fn refresh_registry(
    bytes: &[u8],
    value: &serde_json::Value,
    membership: &PackageMembership,
    relation: &PublicConsumerRelation,
) -> Result<Vec<u8>, ProductRefreshError> {
    let shipping_anchor = "    {\n      \"id\": \"kernel-interface-shipping\",";
    let relation_anchor =
        "    {\n      \"id\": \"kernel-interface-public-api\",";
    let old_members = closure_array(value, INTERFACE_SHIPPING, "members")?;
    let old_reexports = relation_array(value, RELATION, "reexports")?;
    let old_items = relation_array(value, RELATION, "public_items")?;
    let old_derives = relation_array(value, RELATION, "derive_checks")?;
    let members = preserve_order(
        old_members,
        &membership.members,
        old_member_key,
        member_key,
    )?;
    let reexports = preserve_order(
        old_reexports,
        &relation.reexports,
        old_reexport_key,
        |value| Ok(reexport_key(value)),
    )?;
    let public_items = preserve_order(
        old_items,
        &relation.public_items,
        old_public_item_key,
        |value| Ok(public_item_key(value)),
    )?;
    let derive_checks = preserve_order(
        old_derives,
        &relation.derive_checks,
        old_derive_key,
        |value| Ok(derive_key(value)),
    )?;
    let members = render_members(&members)?;
    let reexports = render_reexports(&reexports)?;
    let public_items = render_public_items(&public_items)?;
    let derive_checks = render_derive_checks(&derive_checks)?;
    let bytes = replace_array(bytes, shipping_anchor, "members", &members)?;
    let bytes =
        replace_array(&bytes, relation_anchor, "reexports", &reexports)?;
    let bytes =
        replace_array(&bytes, relation_anchor, "public_items", &public_items)?;
    replace_array(&bytes, relation_anchor, "derive_checks", &derive_checks)
}

fn closure_array<'a>(
    root: &'a serde_json::Value,
    id: &str,
    field: &str,
) -> Result<&'a [serde_json::Value], ProductRefreshError> {
    let closures = root_array(root, "closures")?;
    let selected = exact_object_by_id(closures, id)?;
    let membership = selected
        .get("package_membership")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            not_applied(
                "PRODUCT_REFRESH_ORDER",
                format!("closure {id} has no package membership"),
            )
        })?;
    membership
        .get(field)
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| {
            not_applied(
                "PRODUCT_REFRESH_ORDER",
                format!("closure {id} field {field} is not an array"),
            )
        })
}

fn relation_array<'a>(
    root: &'a serde_json::Value,
    id: &str,
    field: &str,
) -> Result<&'a [serde_json::Value], ProductRefreshError> {
    let relations = root_array(root, "public_consumer_relations")?;
    let selected = exact_object_by_id(relations, id)?;
    selected
        .get(field)
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| {
            not_applied(
                "PRODUCT_REFRESH_ORDER",
                format!("relation {id} field {field} is not an array"),
            )
        })
}

fn root_array<'a>(
    root: &'a serde_json::Value,
    field: &str,
) -> Result<&'a [serde_json::Value], ProductRefreshError> {
    root.get(field)
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| {
            not_applied(
                "PRODUCT_REFRESH_ORDER",
                format!("root field {field} is not an array"),
            )
        })
}

fn exact_object_by_id<'a>(
    values: &'a [serde_json::Value],
    id: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, ProductRefreshError>
{
    let mut matches = values.iter().filter_map(|value| {
        let object = value.as_object()?;
        (object.get("id")?.as_str()? == id).then_some(object)
    });
    let selected = matches.next().ok_or_else(|| {
        not_applied("PRODUCT_REFRESH_ORDER", format!("object {id} is absent"))
    })?;
    if matches.next().is_some() {
        return Err(not_applied(
            "PRODUCT_REFRESH_ORDER",
            format!("object {id} is duplicated"),
        ));
    }
    Ok(selected)
}

fn preserve_order<T, K, O, C>(
    old: &[serde_json::Value],
    candidates: &[T],
    mut old_key: O,
    mut candidate_key: C,
) -> Result<Vec<T>, ProductRefreshError>
where
    T: Clone,
    K: Ord,
    O: FnMut(&serde_json::Value) -> Result<K, ProductRefreshError>,
    C: FnMut(&T) -> Result<K, ProductRefreshError>,
{
    let mut remaining = BTreeMap::new();
    for candidate in candidates {
        let key = candidate_key(candidate)?;
        if remaining.insert(key, candidate.clone()).is_some() {
            return Err(not_applied(
                "PRODUCT_REFRESH_ORDER",
                "derived candidate identity is duplicated",
            ));
        }
    }
    let mut output = Vec::with_capacity(candidates.len());
    for old_value in old {
        let key = old_key(old_value)?;
        if let Some(candidate) = remaining.remove(&key) {
            output.push(candidate);
        }
    }
    output.extend(remaining.into_values());
    Ok(output)
}

fn old_member_key(
    value: &serde_json::Value,
) -> Result<String, ProductRefreshError> {
    string_field(value, "package_path")
}

fn member_key(member: &PackageMember) -> Result<String, ProductRefreshError> {
    path_key(member.package_path())
}

fn old_reexport_key(
    value: &serde_json::Value,
) -> Result<(String, String, String), ProductRefreshError> {
    Ok((
        string_field(value, "public_type")?,
        string_field(value, "provider_relation")?,
        string_field(value, "provider_type")?,
    ))
}

fn reexport_key(value: &PublicReexport) -> (String, String, String) {
    (
        value.public_type.clone(),
        value.provider_relation.clone(),
        value.provider_type.clone(),
    )
}

fn old_public_item_key(
    value: &serde_json::Value,
) -> Result<(String, String), ProductRefreshError> {
    Ok((string_field(value, "path")?, string_field(value, "kind")?))
}

fn public_item_key(value: &PublicItem) -> (String, String) {
    (value.path.clone(), public_item_kind(value.kind).to_owned())
}

fn old_derive_key(
    value: &serde_json::Value,
) -> Result<(String, String, String), ProductRefreshError> {
    Ok((
        string_field(value, "derive")?,
        string_field(value, "public_type")?,
        string_field(value, "consumer_operation")?,
    ))
}

fn derive_key(value: &DeriveCheck) -> (String, String, String) {
    (
        derive_name(value.derive).to_owned(),
        value.public_type.clone(),
        operation_name(value.consumer_operation).to_owned(),
    )
}

fn string_field(
    value: &serde_json::Value,
    field: &str,
) -> Result<String, ProductRefreshError> {
    value
        .as_object()
        .and_then(|object| object.get(field))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            not_applied(
                "PRODUCT_REFRESH_ORDER",
                format!("ordered row field {field} is not a string"),
            )
        })
}

fn path_key(path: &Path) -> Result<String, ProductRefreshError> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        not_applied(
            "PRODUCT_REFRESH_ORDER",
            format!("{} is not UTF-8", path.display()),
        )
    })
}

fn validate_registry_change(
    old: &Registry,
    candidate: &Registry,
    membership: &PackageMembership,
    relation: &PublicConsumerRelation,
) -> Result<(), ProductRefreshError> {
    let mut expected = old.clone();
    let closure = expected
        .closures
        .iter_mut()
        .find(|closure| closure.id == INTERFACE_SHIPPING)
        .ok_or_else(|| {
            not_applied(
                "PRODUCT_REFRESH_REGISTRY_CANDIDATE",
                "Kernel Interface shipping closure disappeared",
            )
        })?;
    closure.package_membership = Some(membership.clone());
    let selected = expected
        .public_consumer_relations
        .iter_mut()
        .find(|candidate| candidate.id == RELATION)
        .ok_or_else(|| {
            not_applied(
                "PRODUCT_REFRESH_REGISTRY_CANDIDATE",
                "Kernel Interface relation disappeared",
            )
        })?;
    *selected = relation.clone();
    if candidate != &expected {
        return Err(not_applied(
            "PRODUCT_REFRESH_REGISTRY_CANDIDATE",
            "candidate changed fields outside the selected derived rows",
        ));
    }
    Ok(())
}

const fn registry_limits(topology: &Topology) -> ParseLimits {
    let product = topology.limits.product_execution;
    ParseLimits {
        path_bytes: topology.limits.path_bytes,
        closure_items: product.compiler_unit_count,
        package_members: product.package_member_count,
        public_items: product.public_item_count,
    }
}

fn analysis_limits(topology: &Topology) -> AnalysisLimits {
    let product = topology.limits.product_execution;
    AnalysisLimits {
        source_files: usize::from(product.compiler_input_count),
        source_file_bytes: topology.limits.source_file_bytes,
        source_total_bytes: topology.limits.source_total_bytes,
        syntax_nodes: topology.limits.syntax_item_expr_type_visits,
    }
}

fn parse_candidate(
    bytes: &[u8],
) -> Result<serde_json::Value, ProductRefreshError> {
    json::parse(bytes, HARD_LIMITS.json_depth, HARD_LIMITS.json_container_items)
        .map_err(config_error)
}

fn render_members(
    members: &[PackageMember],
) -> Result<String, ProductRefreshError> {
    render_records(members, 8, |member| match member {
        PackageMember::CargoGenerated { package_path } => Ok(vec![
            ("kind", quote("cargo_generated")?),
            ("package_path", quote_path(package_path)?),
        ]),
        PackageMember::Copied { package_path, repository_path } => Ok(vec![
            ("kind", quote("copied")?),
            ("package_path", quote_path(package_path)?),
            ("repository_path", quote_path(repository_path)?),
        ]),
    })
}

fn render_reexports(
    values: &[PublicReexport],
) -> Result<String, ProductRefreshError> {
    render_records(values, 6, |value| {
        Ok(vec![
            ("public_type", quote(&value.public_type)?),
            ("provider_relation", quote(&value.provider_relation)?),
            ("provider_type", quote(&value.provider_type)?),
        ])
    })
}

fn render_public_items(
    values: &[PublicItem],
) -> Result<String, ProductRefreshError> {
    render_records(values, 6, |value| {
        Ok(vec![
            ("path", quote(&value.path)?),
            ("kind", quote(public_item_kind(value.kind))?),
        ])
    })
}

fn render_derive_checks(
    values: &[DeriveCheck],
) -> Result<String, ProductRefreshError> {
    render_records(values, 6, |value| {
        Ok(vec![
            ("derive", quote(derive_name(value.derive))?),
            ("public_type", quote(&value.public_type)?),
            (
                "consumer_operation",
                quote(operation_name(value.consumer_operation))?,
            ),
        ])
    })
}

fn render_records<T, F>(
    values: &[T],
    indentation: usize,
    mut fields: F,
) -> Result<String, ProductRefreshError>
where
    F: FnMut(&T) -> Result<Vec<(&'static str, String)>, ProductRefreshError>,
{
    if values.is_empty() {
        return Ok("[]".to_owned());
    }
    let base = " ".repeat(indentation);
    let object_indent = " ".repeat(indentation + 2);
    let field_indent = " ".repeat(indentation + 4);
    let mut output = String::from("[");
    for (record_index, value) in values.iter().enumerate() {
        let fields = fields(value)?;
        append_record(
            &mut output,
            &object_indent,
            &field_indent,
            &fields,
            record_index > 0,
        );
    }
    output.push('\n');
    output.push_str(&base);
    output.push(']');
    Ok(output)
}

fn append_record(
    output: &mut String,
    object_indent: &str,
    field_indent: &str,
    fields: &[(&str, String)],
    comma: bool,
) {
    if comma {
        output.push(',');
    }
    output.push('\n');
    output.push_str(object_indent);
    output.push('{');
    append_fields(output, field_indent, fields);
    output.push('\n');
    output.push_str(object_indent);
    output.push('}');
}

fn append_fields(
    output: &mut String,
    indentation: &str,
    fields: &[(&str, String)],
) {
    for (index, (name, value)) in fields.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push('\n');
        output.push_str(indentation);
        output.push('"');
        output.push_str(name);
        output.push_str("\": ");
        output.push_str(value);
    }
}

fn quote(value: &str) -> Result<String, ProductRefreshError> {
    serde_json::to_string(value).map_err(|error| {
        not_applied("PRODUCT_REFRESH_JSON_STRING", error.to_string())
    })
}

fn quote_path(path: &Path) -> Result<String, ProductRefreshError> {
    let value = path.to_str().ok_or_else(|| {
        not_applied(
            "PRODUCT_REFRESH_PATH",
            format!("{} is not UTF-8", path.display()),
        )
    })?;
    quote(value)
}

const fn public_item_kind(kind: PublicItemKind) -> &'static str {
    match kind {
        PublicItemKind::Module => "module",
        PublicItemKind::Type => "type",
        PublicItemKind::Variant => "variant",
        PublicItemKind::Trait => "trait",
        PublicItemKind::Function => "function",
        PublicItemKind::Const => "const",
        PublicItemKind::Static => "static",
    }
}

const fn derive_name(derive: BuiltinDerive) -> &'static str {
    match derive {
        BuiltinDerive::Clone => "Clone",
        BuiltinDerive::Copy => "Copy",
        BuiltinDerive::Debug => "Debug",
        BuiltinDerive::PartialEq => "PartialEq",
        BuiltinDerive::Eq => "Eq",
        BuiltinDerive::Hash => "Hash",
        BuiltinDerive::Default => "Default",
    }
}

const fn operation_name(operation: ConsumerOperation) -> &'static str {
    match operation {
        ConsumerOperation::Clone => "clone",
        ConsumerOperation::Copy => "copy",
        ConsumerOperation::Debug => "debug",
        ConsumerOperation::PartialEq => "partial_eq",
        ConsumerOperation::Eq => "eq",
        ConsumerOperation::Hash => "hash",
        ConsumerOperation::Default => "default",
    }
}

fn replace_array(
    bytes: &[u8],
    object_anchor: &str,
    field: &str,
    replacement: &str,
) -> Result<Vec<u8>, ProductRefreshError> {
    let text = std::str::from_utf8(bytes).map_err(|error| {
        not_applied("PRODUCT_REFRESH_UTF8", error.to_string())
    })?;
    let anchor = unique_offset(text, object_anchor, "PRODUCT_REFRESH_ANCHOR")?;
    let object_start = anchor + object_anchor.find('{').unwrap_or(0);
    let object_end = matching_delimiter(bytes, object_start, b'{', b'}')?;
    let field_token = format!("\"{field}\": [");
    let object = &text[object_start..=object_end];
    let field_offset =
        unique_offset(object, &field_token, "PRODUCT_REFRESH_FIELD")?;
    let array_start = object_start + field_offset + field_token.len() - 1;
    let array_end = matching_delimiter(bytes, array_start, b'[', b']')?;
    if array_end > object_end {
        return Err(not_applied(
            "PRODUCT_REFRESH_FIELD",
            format!("field {field} escapes its selected object"),
        ));
    }
    let mut output = Vec::with_capacity(
        bytes.len() - (array_end + 1 - array_start) + replacement.len(),
    );
    output.extend_from_slice(&bytes[..array_start]);
    output.extend_from_slice(replacement.as_bytes());
    output.extend_from_slice(&bytes[array_end + 1..]);
    Ok(output)
}

fn matching_delimiter(
    bytes: &[u8],
    start: usize,
    open: u8,
    close: u8,
) -> Result<usize, ProductRefreshError> {
    if bytes.get(start) != Some(&open) {
        return Err(not_applied(
            "PRODUCT_REFRESH_DELIMITER",
            "selected field has the wrong opening delimiter",
        ));
    }
    let mut scan = DelimiterScan::new();
    for (offset, byte) in bytes[start..].iter().copied().enumerate() {
        if scan.consume(byte, open, close)? {
            return Ok(start + offset);
        }
    }
    Err(not_applied(
        "PRODUCT_REFRESH_DELIMITER",
        "selected JSON value has no closing delimiter",
    ))
}

struct DelimiterScan {
    depth: usize,
    string: bool,
    escaped: bool,
}

impl DelimiterScan {
    const fn new() -> Self {
        Self { depth: 0, string: false, escaped: false }
    }

    fn consume(
        &mut self,
        byte: u8,
        open: u8,
        close: u8,
    ) -> Result<bool, ProductRefreshError> {
        if self.string {
            self.consume_string(byte);
            return Ok(false);
        }
        if byte == b'"' {
            self.string = true;
        } else if byte == open {
            self.depth = self.depth.saturating_add(1);
        } else if byte == close {
            return self.consume_close();
        }
        Ok(false)
    }

    const fn consume_string(&mut self, byte: u8) {
        match (self.escaped, byte) {
            (true, _) => self.escaped = false,
            (false, b'\\') => self.escaped = true,
            (false, b'"') => self.string = false,
            (false, _) => {}
        }
    }

    fn consume_close(&mut self) -> Result<bool, ProductRefreshError> {
        let Some(depth) = self.depth.checked_sub(1) else {
            return Err(not_applied(
                "PRODUCT_REFRESH_DELIMITER",
                "selected JSON delimiter underflowed",
            ));
        };
        self.depth = depth;
        Ok(depth == 0)
    }
}

fn replace_once(
    bytes: &[u8],
    old: &str,
    new: &str,
    code: &'static str,
) -> Result<Vec<u8>, ProductRefreshError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| not_applied(code, error.to_string()))?;
    let offset = unique_offset(text, old, code)?;
    if old == new {
        return Ok(bytes.to_vec());
    }
    let mut output = Vec::with_capacity(bytes.len() - old.len() + new.len());
    output.extend_from_slice(&bytes[..offset]);
    output.extend_from_slice(new.as_bytes());
    output.extend_from_slice(&bytes[offset + old.len()..]);
    Ok(output)
}

fn unique_offset(
    text: &str,
    needle: &str,
    code: &'static str,
) -> Result<usize, ProductRefreshError> {
    let mut matches = text.match_indices(needle);
    let offset = matches.next().map(|(offset, _)| offset).ok_or_else(|| {
        not_applied(code, format!("canonical token is absent: {needle}"))
    })?;
    if matches.next().is_some() {
        return Err(not_applied(
            code,
            format!("canonical token is duplicated: {needle}"),
        ));
    }
    Ok(offset)
}

fn config_error(error: json::ConfigError) -> ProductRefreshError {
    let json::ConfigError { code, path, message } = error;
    not_applied(
        "PRODUCT_REFRESH_CONFIG",
        format!("{code} at {path}: {message}"),
    )
}

fn publication_error(
    error: crate::entrypoint_refresh::EntrypointRefreshError,
) -> ProductRefreshError {
    let crate::entrypoint_refresh::EntrypointRefreshError {
        code,
        message,
        effect,
    } = error;
    ProductRefreshError {
        code: "PRODUCT_REFRESH_PUBLISH",
        message: format!("{code}: {message}"),
        effect,
    }
}

fn io_error(
    code: &'static str,
    path: &Path,
    error: &std::io::Error,
) -> ProductRefreshError {
    not_applied(code, format!("{}: {error}", path.display()))
}

fn not_applied(
    code: &'static str,
    message: impl Into<String>,
) -> ProductRefreshError {
    ProductRefreshError {
        code,
        message: message.into(),
        effect: RefreshEffect::DefinitelyNotApplied,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::{
        ProductRefreshRequest, matching_delimiter, preserve_order,
        replace_array, replace_once, string_field, validate_request,
    };
    use crate::entrypoint_refresh::RefreshEffect;

    #[test]
    fn exact_replacement_rejects_absent_and_duplicate_tokens() {
        assert!(replace_once(b"alpha", "beta", "gamma", "TEST").is_err());
        assert!(replace_once(b"alpha alpha", "alpha", "beta", "TEST").is_err());
        assert_eq!(
            replace_once(b"alpha", "alpha", "beta", "TEST").unwrap(),
            b"beta",
        );
    }

    #[test]
    fn array_replacement_is_scoped_and_string_aware() {
        let source = br#"{
  "objects": [
    {
      "id": "selected",
      "values": ["[", {"nested": [1]}]
    },
    {
      "id": "other",
      "values": [9]
    }
  ]
}
"#;
        let anchor = "    {\n      \"id\": \"selected\",";
        let next = replace_array(source, anchor, "values", "[2, 3]").unwrap();
        let text = String::from_utf8(next).unwrap();
        assert!(text.contains("\"values\": [2, 3]"));
        assert!(text.contains("\"values\": [9]"));
    }

    #[test]
    fn delimiter_matching_rejects_unclosed_input() {
        assert!(matching_delimiter(b"[1", 0, b'[', b']').is_err());
        assert_eq!(matching_delimiter(br#"["["]"#, 0, b'[', b']').unwrap(), 4,);
    }

    #[test]
    fn retained_order_precedes_new_derived_rows() {
        let old = vec![json!({"id": "beta"}), json!({"id": "alpha"})];
        let candidates =
            vec!["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()];
        let ordered = preserve_order(
            &old,
            &candidates,
            |value| string_field(value, "id"),
            |value| Ok(value.clone()),
        )
        .unwrap();
        assert_eq!(ordered, ["beta", "alpha", "gamma"]);
    }

    #[test]
    fn unselected_relation_fails_before_repository_access() {
        let request = ProductRefreshRequest {
            root: PathBuf::from("/not-inspected"),
            relation: "foundation-public-api".to_owned(),
        };
        let error = validate_request(&request).unwrap_err();
        assert_eq!(error.code, "PRODUCT_REFRESH_RELATION");
        assert_eq!(error.effect, RefreshEffect::DefinitelyNotApplied);
    }
}
