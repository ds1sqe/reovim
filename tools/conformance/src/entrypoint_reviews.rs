//! Exact repository-entrypoint admission registry.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use reovim_depgraph::{SourceOwner, SourceProvenance, SourceUse};
use serde_json::Value;

use crate::{
    digest,
    finding::{Finding, Findings},
    json::{self, ConfigError},
    policy::relative_path,
    source::{ClassifiedFile, display_relative, read_bounded},
    topology::{RepositoryEntrypoint, Topology},
};

pub const SCHEMA: &str = "reovim-entrypoint-reviews/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    pub reviewed_at: String,
    pub reviews: Vec<Admission>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Admission {
    pub entrypoint: ReviewedEntrypoint,
    pub direct_local_invocations: Vec<Invocation>,
    pub disposition: Disposition,
    pub mechanism: ReviewMechanism,
    pub assumptions: Vec<String>,
    pub review_record: PathBuf,
    pub review_record_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReviewedEntrypoint {
    Script { path: PathBuf, content_digest: String },
    Workflow { path: PathBuf, content_digest: String },
}

impl ReviewedEntrypoint {
    #[must_use]
    pub fn identity(&self) -> EntrypointIdentity {
        match self {
            Self::Script { path, .. } => {
                EntrypointIdentity::Script { path: path.clone() }
            }
            Self::Workflow { path, .. } => {
                EntrypointIdentity::Workflow { path: path.clone() }
            }
        }
    }

    #[must_use]
    pub fn content_digest(&self) -> &str {
        match self {
            Self::Script { content_digest, .. }
            | Self::Workflow { content_digest, .. } => content_digest,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntrypointIdentity {
    Script { path: PathBuf },
    Workflow { path: PathBuf },
    CargoTarget { package: String, target: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Invocation {
    pub target: EntrypointIdentity,
    pub mechanism: InvocationMechanism,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Disposition {
    Admit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReviewMechanism {
    ExactContentRetainedReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InvocationMechanism {
    DirectPathExec,
    HeldCargoTargetExec,
}

impl InvocationMechanism {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectPathExec => "direct-path-exec",
            Self::HeldCargoTargetExec => "held-cargo-target-exec",
        }
    }
}

impl Registry {
    /// Parses a registry value already bounded and duplicate-key checked by the
    /// caller's JSON loader.
    pub fn parse(value: &Value) -> Result<Self, ConfigError> {
        let object = json::object(
            value,
            "$",
            &["schema", "reviewed_at", "reviews"],
            &[],
        )?;
        if json::string(&object["schema"], "$.schema")? != SCHEMA {
            return Err(ConfigError::new(
                "CFG_CONST",
                "$.schema",
                format!("expected {SCHEMA:?}"),
            ));
        }
        let reviewed_at = text(&object["reviewed_at"], "$.reviewed_at")?;
        if !crate::schema::utc_timestamp(&reviewed_at) {
            return Err(ConfigError::new(
                "ENTRYPOINT_REVIEW_TIMESTAMP",
                "$.reviewed_at",
                "expected YYYY-MM-DDTHH:MM:SSZ",
            ));
        }
        let values = json::array(&object["reviews"], "$.reviews")?;
        if values.is_empty() {
            return Err(ConfigError::new(
                "ENTRYPOINT_REVIEW_EMPTY",
                "$.reviews",
                "at least one exact entrypoint review is required",
            ));
        }

        let reviews = parse_reviews(values)?;
        Ok(Self { reviewed_at, reviews })
    }
}

fn parse_reviews(values: &[Value]) -> Result<Vec<Admission>, ConfigError> {
    let mut reviews = Vec::new();
    let mut identities = BTreeSet::new();
    let mut records = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let path = json::index("$.reviews", item);
        let review = parse_review(value, &path)?;
        if !identities.insert(review.entrypoint.identity()) {
            return Err(ConfigError::new(
                "ENTRYPOINT_REVIEW_DUPLICATE_IDENTITY",
                &path,
                "duplicate repository entrypoint review identity",
            ));
        }
        if !records.insert(review.review_record.clone()) {
            return Err(ConfigError::new(
                "ENTRYPOINT_REVIEW_DUPLICATE_RECORD",
                &path,
                "each entrypoint requires its own focused review record",
            ));
        }
        reviews.push(review);
    }
    reviews.sort();
    Ok(reviews)
}

/// Cross-validates exact script/workflow reviews against topology, source
/// inventory, retained records, and the conservative lexical discovery pass.
#[allow(clippy::too_many_lines)]
pub fn validate(
    root: &Path,
    registry: &Registry,
    topology: &Topology,
    inventory: &[ClassifiedFile],
    finding_limit: usize,
    file_byte_limit: u64,
) -> Vec<Finding> {
    let mut findings = Findings::new(finding_limit);
    let topology_reviews = topology
        .repository_entrypoints
        .iter()
        .filter_map(topology_review_identity)
        .collect::<BTreeMap<_, _>>();
    let reviews = registry
        .reviews
        .iter()
        .map(|review| (review.entrypoint.identity(), review))
        .collect::<BTreeMap<_, _>>();

    for (identity, topology_digest) in &topology_reviews {
        match reviews.get(identity) {
            Some(review) if review_digest_matches(review, topology_digest) => {}
            Some(review) => findings.push(Finding::new(
                "entrypoint_review",
                "ENTRYPOINT_REVIEW_CONTENT_DIGEST",
                identity_path(identity),
                format!(
                    concat!(
                        "review digest {:?} differs from topology digest ",
                        "{topology_digest}",
                    ),
                    review.entrypoint.content_digest(),
                    topology_digest = topology_digest,
                ),
            )),
            None => findings.push(Finding::new(
                "entrypoint_review",
                "ENTRYPOINT_REVIEW_MISSING",
                identity_path(identity),
                "topology script/workflow lacks its one exact retained review",
            )),
        }
    }
    for identity in reviews.keys() {
        if !topology_reviews.contains_key(identity) {
            findings.push(Finding::new(
                "entrypoint_review",
                "ENTRYPOINT_REVIEW_STALE",
                identity_path(identity),
                concat!(
                    "review identity is absent from topology ",
                    "script/workflow entrypoints",
                ),
            ));
        }
    }

    let declared_records = registry
        .reviews
        .iter()
        .map(|review| review.review_record.clone())
        .collect::<BTreeSet<_>>();
    let inventoried_records = inventory
        .iter()
        .map(|file| &file.path)
        .filter(|path| focused_record_path(path))
        .cloned()
        .collect::<BTreeSet<_>>();
    for path in inventoried_records.difference(&declared_records) {
        findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_RECORD_UNDECLARED",
            Some(display_relative(path)),
            "focused entrypoint review record has no registry identity",
        ));
    }
    for path in declared_records.difference(&inventoried_records) {
        findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_RECORD_UNINVENTORIED",
            Some(display_relative(path)),
            "registry review record is absent from the focused inventory set",
        ));
    }

    let topology_identities = topology
        .repository_entrypoints
        .iter()
        .map(topology_identity)
        .collect::<BTreeSet<_>>();
    for review in &registry.reviews {
        validate_review_record(
            root,
            review,
            inventory,
            file_byte_limit,
            &mut findings,
        );
        validate_invocations(
            review,
            &reviews,
            &topology_identities,
            topology,
            inventory,
            &mut findings,
        );
        validate_lexical_subset(
            root,
            review,
            inventory,
            file_byte_limit,
            &mut findings,
        );
    }
    findings.finish()
}

fn review_digest_matches(review: &Admission, topology_digest: &str) -> bool {
    review.entrypoint.content_digest() == topology_digest
}

fn validate_review_record(
    root: &Path,
    review: &Admission,
    inventory: &[ClassifiedFile],
    file_byte_limit: u64,
    findings: &mut Findings,
) {
    let record_path = display_relative(&review.review_record);
    let Some(file) =
        inventory.iter().find(|file| file.path == review.review_record)
    else {
        findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_RECORD_MISSING",
            Some(record_path),
            "retained review record is absent from the classified inventory",
        ));
        return;
    };
    if file.classification.owner != SourceOwner::RepositoryTooling
        || file.classification.provenance != SourceProvenance::Authored
        || file.classification.source_use != SourceUse::Tool
    {
        findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_RECORD_CLASSIFICATION",
            Some(record_path.clone()),
            "review record must be RepositoryTooling + Authored + Tool",
        ));
    }
    if file.content_digest != review.review_record_digest {
        findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_RECORD_DIGEST",
            Some(record_path.clone()),
            format!(
                "inventory digest {} differs from reviewed {}",
                file.content_digest, review.review_record_digest
            ),
        ));
    }
    match read_bounded(&root.join(&review.review_record), file_byte_limit) {
        Ok(bytes) => {
            let actual = digest::bytes(&bytes);
            if actual != file.content_digest
                || actual != review.review_record_digest
            {
                findings.push(Finding::new(
                    "entrypoint_review",
                    "ENTRYPOINT_REVIEW_RECORD_BYTES",
                    Some(record_path),
                    format!(
                        concat!(
                            "actual record digest {actual} differs from ",
                            "inventory or review",
                        ),
                        actual = actual
                    ),
                ));
            }
        }
        Err(error) => findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_RECORD_IO",
            Some(record_path),
            error,
        )),
    }
}

fn validate_invocations(
    review: &Admission,
    reviews: &BTreeMap<EntrypointIdentity, &Admission>,
    topology_identities: &BTreeSet<EntrypointIdentity>,
    topology: &Topology,
    inventory: &[ClassifiedFile],
    findings: &mut Findings,
) {
    for invocation in &review.direct_local_invocations {
        validate_invocation(
            review,
            invocation,
            reviews,
            topology_identities,
            topology,
            inventory,
            findings,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_invocation(
    review: &Admission,
    invocation: &Invocation,
    reviews: &BTreeMap<EntrypointIdentity, &Admission>,
    topology_identities: &BTreeSet<EntrypointIdentity>,
    topology: &Topology,
    inventory: &[ClassifiedFile],
    findings: &mut Findings,
) {
    if !topology_identities.contains(&invocation.target) {
        findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_INVOCATION_UNRESOLVED",
            identity_path(&invocation.target),
            format!(
                "{} direct invocation is absent from workspace topology",
                identity_label(&review.entrypoint.identity())
            ),
        ));
        return;
    }
    match &invocation.target {
        EntrypointIdentity::Script { .. }
        | EntrypointIdentity::Workflow { .. } => {
            validate_reviewed_target(invocation, reviews, findings);
        }
        EntrypointIdentity::CargoTarget { package, target } => {
            validate_cargo_invocation_source(
                package, target, topology, inventory, findings,
            );
        }
    }
}

fn validate_reviewed_target(
    invocation: &Invocation,
    reviews: &BTreeMap<EntrypointIdentity, &Admission>,
    findings: &mut Findings,
) {
    if !reviews.contains_key(&invocation.target) {
        findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_INVOCATION_UNREVIEWED",
            identity_path(&invocation.target),
            "direct script/workflow target lacks an exact retained review",
        ));
    }
}

fn validate_cargo_invocation_source(
    package: &str,
    target: &str,
    topology: &Topology,
    inventory: &[ClassifiedFile],
    findings: &mut Findings,
) {
    let source = topology
        .packages
        .iter()
        .find(|candidate| candidate.name == package)
        .and_then(|package| {
            package.targets.iter().find(|candidate| {
                candidate.name == target
                    && candidate.kind.iter().any(|kind| kind == "bin")
            })
        })
        .map(|target| &target.src_path);
    let Some(source) = source else {
        findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_CARGO_TARGET_SOURCE",
            None,
            format!(
                concat!(
                    "Cargo invocation {package}/{target} has no topology ",
                    "binary source",
                ),
                package = package,
                target = target
            ),
        ));
        return;
    };
    let admitted = inventory.iter().any(|file| {
        file.path == *source
            && file.classification.owner == SourceOwner::RepositoryTooling
            && file.classification.provenance == SourceProvenance::Authored
            && file.classification.source_use == SourceUse::Tool
    });
    if !admitted {
        findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_CARGO_SOURCE_CLASSIFICATION",
            Some(display_relative(source)),
            concat!(
                "Cargo invocation source must be ",
                "RepositoryTooling + Authored + Tool",
            ),
        ));
    }
}

fn validate_lexical_subset(
    root: &Path,
    review: &Admission,
    inventory: &[ClassifiedFile],
    file_byte_limit: u64,
    findings: &mut Findings,
) {
    let (path, reviewed_digest) = match &review.entrypoint {
        ReviewedEntrypoint::Script { path, content_digest }
        | ReviewedEntrypoint::Workflow { path, content_digest } => {
            (path, content_digest)
        }
    };
    let path_text = display_relative(path);
    let Some(file) = inventory.iter().find(|file| file.path == *path) else {
        findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_SOURCE_MISSING",
            Some(path_text),
            "reviewed entrypoint is absent from the classified inventory",
        ));
        return;
    };
    if file.classification.owner != SourceOwner::RepositoryTooling
        || file.classification.provenance != SourceProvenance::Authored
        || file.classification.source_use != SourceUse::Tool
    {
        findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_SOURCE_CLASSIFICATION",
            Some(path_text.clone()),
            "reviewed entrypoint must be RepositoryTooling + Authored + Tool",
        ));
    }
    let bytes = match read_bounded(&root.join(path), file_byte_limit) {
        Ok(bytes) => bytes,
        Err(error) => {
            findings.push(Finding::new(
                "entrypoint_review",
                "ENTRYPOINT_REVIEW_SOURCE_IO",
                Some(path_text),
                error,
            ));
            return;
        }
    };
    let actual_digest = digest::bytes(&bytes);
    if actual_digest != *reviewed_digest || actual_digest != file.content_digest
    {
        findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_SOURCE_BYTES",
            Some(path_text),
            format!(
                "actual digest {actual_digest} differs from inventory or review"
            ),
        ));
        return;
    }
    let Ok(contents) = std::str::from_utf8(&bytes) else {
        findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_SOURCE_UTF8",
            Some(path_text),
            "reviewed script/workflow must be UTF-8",
        ));
        return;
    };
    let (discovered, _dynamic) = crate::source::obvious_local_inputs(contents);
    let declared = review
        .direct_local_invocations
        .iter()
        .filter_map(|invocation| match &invocation.target {
            EntrypointIdentity::Script { path }
            | EntrypointIdentity::Workflow { path } => Some(path.clone()),
            EntrypointIdentity::CargoTarget { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    for missing in discovered.difference(&declared) {
        findings.push(Finding::new(
            "entrypoint_review",
            "ENTRYPOINT_REVIEW_LEXICAL_EDGE_MISSING",
            Some(display_relative(missing)),
            format!(
                concat!(
                    "lexically discovered direct input is not reviewed ",
                    "for {path_text}",
                ),
                path_text = path_text
            ),
        ));
    }
}

fn topology_review_identity(
    entrypoint: &RepositoryEntrypoint,
) -> Option<(EntrypointIdentity, &str)> {
    match entrypoint {
        RepositoryEntrypoint::Script { path, content_digest } => Some((
            EntrypointIdentity::Script { path: path.clone() },
            content_digest.as_str(),
        )),
        RepositoryEntrypoint::Workflow { path, content_digest } => Some((
            EntrypointIdentity::Workflow { path: path.clone() },
            content_digest.as_str(),
        )),
        RepositoryEntrypoint::CargoTarget { .. } => None,
    }
}

fn topology_identity(entrypoint: &RepositoryEntrypoint) -> EntrypointIdentity {
    match entrypoint {
        RepositoryEntrypoint::Script { path, .. } => {
            EntrypointIdentity::Script { path: path.clone() }
        }
        RepositoryEntrypoint::Workflow { path, .. } => {
            EntrypointIdentity::Workflow { path: path.clone() }
        }
        RepositoryEntrypoint::CargoTarget { package, target } => {
            EntrypointIdentity::CargoTarget {
                package: package.clone(),
                target: target.clone(),
            }
        }
    }
}

fn identity_path(identity: &EntrypointIdentity) -> Option<String> {
    match identity {
        EntrypointIdentity::Script { path }
        | EntrypointIdentity::Workflow { path } => Some(display_relative(path)),
        EntrypointIdentity::CargoTarget { .. } => None,
    }
}

fn identity_label(identity: &EntrypointIdentity) -> String {
    match identity {
        EntrypointIdentity::Script { path } => {
            format!("script {}", display_relative(path))
        }
        EntrypointIdentity::Workflow { path } => {
            format!("workflow {}", display_relative(path))
        }
        EntrypointIdentity::CargoTarget { package, target } => {
            format!("Cargo target {package}/{target}")
        }
    }
}

#[allow(clippy::too_many_lines)]
fn parse_review(value: &Value, path: &str) -> Result<Admission, ConfigError> {
    const COMMON: &[&str] = &[
        "kind",
        "direct_local_invocations",
        "disposition",
        "mechanism",
        "assumptions",
        "review_record",
        "review_record_digest",
    ];
    let raw = value
        .as_object()
        .ok_or_else(|| ConfigError::new("CFG_TYPE", path, "expected object"))?;
    let kind_path = json::child(path, "kind");
    let kind = raw
        .get("kind")
        .ok_or_else(|| {
            ConfigError::new(
                "CFG_MISSING_FIELD",
                &kind_path,
                "required field is absent",
            )
        })
        .and_then(|value| json::string(value, &kind_path))?;
    let (object, entrypoint, required_mechanism) = match kind {
        "script" => {
            let required =
                with_identity_fields(COMMON, &["path", "content_digest"]);
            let object = json::object(value, path, &required, &[])?;
            let path_field = json::child(path, "path");
            let digest_field = json::child(path, "content_digest");
            let entrypoint = ReviewedEntrypoint::Script {
                path: relative_path(
                    json::string(&object["path"], &path_field)?,
                    &path_field,
                    false,
                )?,
                content_digest: digest(
                    json::string(&object["content_digest"], &digest_field)?,
                    &digest_field,
                )?,
            };
            (object, entrypoint, ReviewMechanism::ExactContentRetainedReview)
        }
        "workflow" => {
            let required =
                with_identity_fields(COMMON, &["path", "content_digest"]);
            let object = json::object(value, path, &required, &[])?;
            let path_field = json::child(path, "path");
            let digest_field = json::child(path, "content_digest");
            let entrypoint = ReviewedEntrypoint::Workflow {
                path: relative_path(
                    json::string(&object["path"], &path_field)?,
                    &path_field,
                    false,
                )?,
                content_digest: digest(
                    json::string(&object["content_digest"], &digest_field)?,
                    &digest_field,
                )?,
            };
            (object, entrypoint, ReviewMechanism::ExactContentRetainedReview)
        }
        other => {
            return Err(ConfigError::new(
                "ENTRYPOINT_REVIEW_KIND",
                kind_path,
                format!("unknown entrypoint kind {other:?}"),
            ));
        }
    };

    let invocations_path = json::child(path, "direct_local_invocations");
    let direct_local_invocations = parse_invocations(
        &object["direct_local_invocations"],
        &invocations_path,
    )?;
    let disposition_path = json::child(path, "disposition");
    if json::string(&object["disposition"], &disposition_path)? != "admit" {
        return Err(ConfigError::new(
            "ENTRYPOINT_REVIEW_DISPOSITION",
            disposition_path,
            "only explicit admit records are accepted",
        ));
    }
    let mechanism_path = json::child(path, "mechanism");
    let mechanism = parse_review_mechanism(
        json::string(&object["mechanism"], &mechanism_path)?,
        &mechanism_path,
    )?;
    if mechanism != required_mechanism {
        return Err(ConfigError::new(
            "ENTRYPOINT_REVIEW_MECHANISM_KIND",
            mechanism_path,
            "review mechanism does not match the entrypoint kind",
        ));
    }
    let assumptions_path = json::child(path, "assumptions");
    let assumptions =
        nonempty_unique_texts(&object["assumptions"], &assumptions_path)?;
    let record_path = json::child(path, "review_record");
    let review_record = relative_path(
        json::string(&object["review_record"], &record_path)?,
        &record_path,
        false,
    )?;
    if !focused_record_path(&review_record) {
        return Err(ConfigError::new(
            "ENTRYPOINT_REVIEW_RECORD_PATH",
            record_path,
            concat!(
                "record must be a focused Markdown file under ",
                "conformance/entrypoint-reviews",
            ),
        ));
    }
    let record_digest_path = json::child(path, "review_record_digest");
    let review_record_digest = digest(
        json::string(&object["review_record_digest"], &record_digest_path)?,
        &record_digest_path,
    )?;

    Ok(Admission {
        entrypoint,
        direct_local_invocations,
        disposition: Disposition::Admit,
        mechanism,
        assumptions,
        review_record,
        review_record_digest,
    })
}

fn parse_invocations(
    value: &Value,
    path: &str,
) -> Result<Vec<Invocation>, ConfigError> {
    let mut invocations = Vec::new();
    let mut identities = BTreeSet::new();
    for (item, value) in json::array(value, path)?.iter().enumerate() {
        let item_path = json::index(path, item);
        let invocation = parse_invocation(value, &item_path)?;
        if !identities.insert(invocation.target.clone()) {
            return Err(ConfigError::new(
                "ENTRYPOINT_REVIEW_DUPLICATE_INVOCATION",
                item_path,
                "duplicate direct local invocation identity",
            ));
        }
        invocations.push(invocation);
    }
    invocations.sort();
    Ok(invocations)
}

fn parse_invocation(
    value: &Value,
    path: &str,
) -> Result<Invocation, ConfigError> {
    let raw = value
        .as_object()
        .ok_or_else(|| ConfigError::new("CFG_TYPE", path, "expected object"))?;
    let kind_path = json::child(path, "kind");
    let kind = raw
        .get("kind")
        .ok_or_else(|| {
            ConfigError::new(
                "CFG_MISSING_FIELD",
                &kind_path,
                "required field is absent",
            )
        })
        .and_then(|value| json::string(value, &kind_path))?;
    let (target, mechanism, raw_mechanism) = match kind {
        "script" | "workflow" => {
            let object =
                json::object(value, path, &["kind", "path", "mechanism"], &[])?;
            let target_path = json::child(path, "path");
            let target_path = relative_path(
                json::string(&object["path"], &target_path)?,
                &target_path,
                false,
            )?;
            let target = if kind == "script" {
                EntrypointIdentity::Script { path: target_path }
            } else {
                EntrypointIdentity::Workflow { path: target_path }
            };
            (
                target,
                InvocationMechanism::DirectPathExec,
                json::string(
                    &object["mechanism"],
                    &json::child(path, "mechanism"),
                )?,
            )
        }
        "cargo_target" => {
            let object = json::object(
                value,
                path,
                &["kind", "package", "target", "mechanism"],
                &[],
            )?;
            let target = EntrypointIdentity::CargoTarget {
                package: cargo_identity(
                    &object["package"],
                    &json::child(path, "package"),
                )?,
                target: cargo_identity(
                    &object["target"],
                    &json::child(path, "target"),
                )?,
            };
            (
                target,
                InvocationMechanism::HeldCargoTargetExec,
                json::string(
                    &object["mechanism"],
                    &json::child(path, "mechanism"),
                )?,
            )
        }
        other => {
            return Err(ConfigError::new(
                "ENTRYPOINT_REVIEW_INVOCATION_KIND",
                kind_path,
                format!("unknown invocation kind {other:?}"),
            ));
        }
    };
    if raw_mechanism != mechanism.as_str() {
        return Err(ConfigError::new(
            "ENTRYPOINT_REVIEW_INVOCATION_MECHANISM",
            json::child(path, "mechanism"),
            "invocation mechanism does not match the target kind",
        ));
    }
    Ok(Invocation { target, mechanism })
}

fn with_identity_fields(
    common: &[&'static str],
    identity: &[&'static str],
) -> Vec<&'static str> {
    common.iter().chain(identity).copied().collect()
}

fn parse_review_mechanism(
    value: &str,
    path: &str,
) -> Result<ReviewMechanism, ConfigError> {
    match value {
        "exact-content-retained-review" => {
            Ok(ReviewMechanism::ExactContentRetainedReview)
        }
        other => Err(ConfigError::new(
            "ENTRYPOINT_REVIEW_MECHANISM",
            path,
            format!("unknown review mechanism {other:?}"),
        )),
    }
}

fn nonempty_unique_texts(
    value: &Value,
    path: &str,
) -> Result<Vec<String>, ConfigError> {
    let values = json::array(value, path)?;
    if values.is_empty() {
        return Err(ConfigError::new(
            "ENTRYPOINT_REVIEW_EMPTY_ASSUMPTIONS",
            path,
            "at least one explicit execution assumption is required",
        ));
    }
    let mut result = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let item_path = json::index(path, item);
        let value = text(value, &item_path)?;
        if !result.insert(value) {
            return Err(ConfigError::new(
                "ENTRYPOINT_REVIEW_DUPLICATE_ASSUMPTION",
                item_path,
                "duplicate execution assumption",
            ));
        }
    }
    Ok(result.into_iter().collect())
}

fn cargo_identity(value: &Value, path: &str) -> Result<String, ConfigError> {
    text(value, path)
}

fn text(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = json::string(value, path)?;
    if value.is_empty() {
        Err(ConfigError::new(
            "ENTRYPOINT_REVIEW_TEXT",
            path,
            "text must be nonempty",
        ))
    } else {
        Ok(value.to_owned())
    }
}

fn digest(value: &str, path: &str) -> Result<String, ConfigError> {
    if value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(value.to_owned())
    } else {
        Err(ConfigError::new(
            "ENTRYPOINT_REVIEW_DIGEST",
            path,
            "expected sha256:<64 lowercase hex>",
        ))
    }
}

fn focused_record_path(path: &Path) -> bool {
    path.parent() == Some(Path::new("conformance/entrypoint-reviews"))
        && path.extension().and_then(|extension| extension.to_str())
            == Some("md")
        && path.file_name().and_then(|name| name.to_str()) != Some("README.md")
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        fs,
        path::{Path, PathBuf},
    };

    use reovim_depgraph::{
        SourceClassification, SourceOwner, SourceProvenance, SourceUse,
    };
    use serde_json::{Value, json};

    use crate::{source::ClassifiedFile, topology::RepositoryEntrypoint};

    fn registry() -> Value {
        serde_json::from_str(include_str!(
            "../../../conformance/entrypoint-reviews.json"
        ))
        .unwrap()
    }

    fn registry_schema() -> Value {
        serde_json::from_str(include_str!(
            "../../../conformance/entrypoint-reviews.schema.json"
        ))
        .unwrap()
    }

    fn topology() -> crate::topology::Topology {
        let value: Value = serde_json::from_str(include_str!(
            "../../../conformance/workspace-topology.json"
        ))
        .unwrap();
        crate::topology::Topology::parse(&value).unwrap()
    }

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    fn review_inventory_paths(review: &super::Admission) -> [PathBuf; 2] {
        let entrypoint = match &review.entrypoint {
            super::ReviewedEntrypoint::Script { path, .. }
            | super::ReviewedEntrypoint::Workflow { path, .. } => path.clone(),
        };
        [entrypoint, review.review_record.clone()]
    }

    fn cargo_entrypoint_source(
        topology: &crate::topology::Topology,
        entrypoint: &RepositoryEntrypoint,
    ) -> Option<PathBuf> {
        match entrypoint {
            RepositoryEntrypoint::CargoTarget { package, target } => {
                Some(package_source(topology, package, target))
            }
            _ => None,
        }
    }

    fn package_source(
        topology: &crate::topology::Topology,
        package: &str,
        target: &str,
    ) -> PathBuf {
        let package = topology
            .packages
            .iter()
            .find(|candidate| candidate.name == package)
            .unwrap();
        package
            .targets
            .iter()
            .find(|candidate| candidate.name == target)
            .unwrap()
            .src_path
            .clone()
    }

    fn current_inventory(
        registry: &super::Registry,
        topology: &crate::topology::Topology,
    ) -> Vec<ClassifiedFile> {
        let mut paths = registry
            .reviews
            .iter()
            .flat_map(review_inventory_paths)
            .collect::<BTreeSet<_>>();
        paths.insert(PathBuf::from("conformance/entrypoint-reviews/README.md"));
        paths.extend(topology.repository_entrypoints.iter().filter_map(
            |entrypoint| cargo_entrypoint_source(topology, entrypoint),
        ));
        paths
            .into_iter()
            .map(|path| {
                let bytes = fs::read(root().join(&path)).unwrap();
                ClassifiedFile {
                    path: path.clone(),
                    content_digest: crate::digest::bytes(&bytes),
                    classification: SourceClassification::new(
                        SourceOwner::RepositoryTooling,
                        SourceProvenance::Authored,
                        SourceUse::Tool,
                    )
                    .unwrap(),
                    upstream: None,
                    rule_prefix: path,
                    rule_precedence: 1,
                }
            })
            .collect()
    }

    #[test]
    fn current_registry_exposes_exact_identities_and_edges() {
        crate::schema::validate_instance(&registry(), &registry_schema())
            .unwrap();
        let registry = super::Registry::parse(&registry()).unwrap();
        let identities = registry
            .reviews
            .iter()
            .map(|review| review.entrypoint.identity())
            .collect::<BTreeSet<_>>();
        let expected = BTreeSet::from([
            super::EntrypointIdentity::Script {
                path: PathBuf::from("scripts/check.sh"),
            },
            super::EntrypointIdentity::Script {
                path: PathBuf::from("scripts/check-cache.sh"),
            },
            super::EntrypointIdentity::Script {
                path: PathBuf::from("scripts/check-style.sh"),
            },
            super::EntrypointIdentity::Script {
                path: PathBuf::from("scripts/check-documentation.sh"),
            },
            super::EntrypointIdentity::Script {
                path: PathBuf::from("scripts/check-conformance.sh"),
            },
            super::EntrypointIdentity::Workflow {
                path: PathBuf::from(".github/workflows/ci.yml"),
            },
        ]);
        assert_eq!(identities, expected);
        let check = registry
            .reviews
            .iter()
            .find(|review| {
                matches!(
                    &review.entrypoint,
                    super::ReviewedEntrypoint::Script { path, .. }
                        if path == std::path::Path::new("scripts/check.sh")
                )
            })
            .unwrap();
        let invocations = check
            .direct_local_invocations
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let expected = BTreeSet::from([
            super::Invocation {
                target: super::EntrypointIdentity::Script {
                    path: PathBuf::from("scripts/check-style.sh"),
                },
                mechanism: super::InvocationMechanism::DirectPathExec,
            },
            super::Invocation {
                target: super::EntrypointIdentity::Script {
                    path: PathBuf::from("scripts/check-documentation.sh"),
                },
                mechanism: super::InvocationMechanism::DirectPathExec,
            },
            super::Invocation {
                target: super::EntrypointIdentity::Script {
                    path: PathBuf::from("scripts/check-cache.sh"),
                },
                mechanism: super::InvocationMechanism::DirectPathExec,
            },
        ]);
        assert_eq!(invocations, expected);
        assert!(check.entrypoint.content_digest().starts_with("sha256:"));
        let conformance = registry
            .reviews
            .iter()
            .find(|review| {
                matches!(
                    &review.entrypoint,
                    super::ReviewedEntrypoint::Script { path, .. }
                        if path == Path::new("scripts/check-conformance.sh")
                )
            })
            .unwrap();
        assert!(matches!(
            conformance.direct_local_invocations.as_slice(),
            [super::Invocation {
                target: super::EntrypointIdentity::CargoTarget { .. },
                mechanism: super::InvocationMechanism::HeldCargoTargetExec,
            }]
        ));
        assert_eq!(
            check.mechanism,
            super::ReviewMechanism::ExactContentRetainedReview
        );
        assert_eq!(check.disposition, super::Disposition::Admit);
    }

    #[test]
    fn unknown_fields_and_kind_specific_fields_fail() {
        let mut unknown = registry();
        unknown["reviews"][0]["surprise"] = json!(true);
        assert_eq!(
            super::Registry::parse(&unknown).unwrap_err().code,
            "CFG_UNKNOWN_FIELD"
        );

        let mut cargo_review = registry();
        cargo_review["reviews"][0]["kind"] = json!("cargo_target");
        assert_eq!(
            super::Registry::parse(&cargo_review).unwrap_err().code,
            "ENTRYPOINT_REVIEW_KIND"
        );
    }

    #[test]
    fn duplicate_entrypoint_record_and_invocation_identities_fail() {
        let mut duplicate_entrypoint = registry();
        let duplicate = duplicate_entrypoint["reviews"][0].clone();
        duplicate_entrypoint["reviews"].as_array_mut().unwrap().push(duplicate);
        assert_eq!(
            super::Registry::parse(&duplicate_entrypoint).unwrap_err().code,
            "ENTRYPOINT_REVIEW_DUPLICATE_IDENTITY"
        );

        let mut duplicate_record = registry();
        duplicate_record["reviews"][1]["review_record"] =
            duplicate_record["reviews"][0]["review_record"].clone();
        assert_eq!(
            super::Registry::parse(&duplicate_record).unwrap_err().code,
            "ENTRYPOINT_REVIEW_DUPLICATE_RECORD"
        );

        let mut duplicate_invocation = registry();
        let duplicate =
            duplicate_invocation["reviews"][0]["direct_local_invocations"][0]
                .clone();
        duplicate_invocation["reviews"][0]["direct_local_invocations"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert_eq!(
            super::Registry::parse(&duplicate_invocation).unwrap_err().code,
            "ENTRYPOINT_REVIEW_DUPLICATE_INVOCATION"
        );
    }

    #[test]
    fn disposition_mechanism_path_and_digests_fail_closed() {
        let mut timestamp = registry();
        timestamp["reviewed_at"] = json!("2025-02-29T25:60:60Z");
        assert_eq!(
            super::Registry::parse(&timestamp).unwrap_err().code,
            "ENTRYPOINT_REVIEW_TIMESTAMP"
        );

        let mut disposition = registry();
        disposition["reviews"][0]["disposition"] = json!("observe");
        assert_eq!(
            super::Registry::parse(&disposition).unwrap_err().code,
            "ENTRYPOINT_REVIEW_DISPOSITION"
        );

        let mut mechanism = registry();
        mechanism["reviews"][0]["mechanism"] =
            json!("cargo-manifest-source-retained-review");
        assert_eq!(
            super::Registry::parse(&mechanism).unwrap_err().code,
            "ENTRYPOINT_REVIEW_MECHANISM"
        );

        let mut path = registry();
        path["reviews"][0]["review_record"] = json!("../outside.md");
        assert_eq!(
            super::Registry::parse(&path).unwrap_err().code,
            "CFG_PATH_ESCAPE"
        );

        let mut digest = registry();
        digest["reviews"][0]["review_record_digest"] = json!("sha256:ABC");
        assert_eq!(
            super::Registry::parse(&digest).unwrap_err().code,
            "ENTRYPOINT_REVIEW_DIGEST"
        );
    }

    #[test]
    fn assumptions_and_invocation_mechanisms_are_closed() {
        let mut empty = registry();
        empty["reviews"][0]["assumptions"] = json!([]);
        assert_eq!(
            super::Registry::parse(&empty).unwrap_err().code,
            "ENTRYPOINT_REVIEW_EMPTY_ASSUMPTIONS"
        );

        let mut duplicate = registry();
        let assumption = duplicate["reviews"][0]["assumptions"][0].clone();
        duplicate["reviews"][0]["assumptions"]
            .as_array_mut()
            .unwrap()
            .push(assumption);
        assert_eq!(
            super::Registry::parse(&duplicate).unwrap_err().code,
            "ENTRYPOINT_REVIEW_DUPLICATE_ASSUMPTION"
        );

        let mut wrong_invocation = registry();
        let review = wrong_invocation["reviews"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|review| review["path"] == "scripts/check-conformance.sh")
            .unwrap();
        let invocation = &mut review["direct_local_invocations"][0];
        invocation["mechanism"] = json!("direct-path-exec");
        assert_eq!(
            super::Registry::parse(&wrong_invocation).unwrap_err().code,
            "ENTRYPOINT_REVIEW_INVOCATION_MECHANISM"
        );
    }

    #[test]
    fn current_registry_cross_validates_exact_topology_records_and_edges() {
        let registry = super::Registry::parse(&registry()).unwrap();
        let topology = topology();
        let inventory = current_inventory(&registry, &topology);
        let findings = super::validate(
            &root(),
            &registry,
            &topology,
            &inventory,
            128,
            4_194_304,
        );
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn validation_rejects_missing_reviews_stale_records_and_unresolved_edges() {
        let mut registry = super::Registry::parse(&registry()).unwrap();
        let topology = topology();
        let mut inventory = current_inventory(&registry, &topology);

        registry.reviews.remove(0);
        inventory.push(ClassifiedFile {
            path: PathBuf::from("conformance/entrypoint-reviews/stale.md"),
            content_digest: crate::digest::bytes(b"stale"),
            classification: SourceClassification::new(
                SourceOwner::RepositoryTooling,
                SourceProvenance::Authored,
                SourceUse::Tool,
            )
            .unwrap(),
            upstream: None,
            rule_prefix: PathBuf::from("conformance/entrypoint-reviews"),
            rule_precedence: 1,
        });
        registry.reviews[0].direct_local_invocations.push(super::Invocation {
            target: super::EntrypointIdentity::CargoTarget {
                package: "missing".to_owned(),
                target: "missing".to_owned(),
            },
            mechanism: super::InvocationMechanism::HeldCargoTargetExec,
        });

        let findings = super::validate(
            &root(),
            &registry,
            &topology,
            &inventory,
            128,
            4_194_304,
        );
        let codes = findings
            .iter()
            .map(|finding| finding.code)
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("ENTRYPOINT_REVIEW_MISSING"));
        assert!(codes.contains("ENTRYPOINT_REVIEW_RECORD_UNDECLARED"));
        assert!(codes.contains("ENTRYPOINT_REVIEW_INVOCATION_UNRESOLVED"));
    }

    #[test]
    fn validation_rejects_digest_classification_and_lexical_edge_drift() {
        let topology = topology();

        let mut lexical_registry = super::Registry::parse(&registry()).unwrap();
        let lexical_inventory = current_inventory(&lexical_registry, &topology);
        let check = lexical_registry
            .reviews
            .iter_mut()
            .find(|review| {
                matches!(
                    &review.entrypoint,
                    super::ReviewedEntrypoint::Script { path, .. }
                        if path == std::path::Path::new("scripts/check.sh")
                )
            })
            .unwrap();
        check.direct_local_invocations.retain(|invocation| {
            !matches!(
                &invocation.target,
                super::EntrypointIdentity::Script { path }
                    if path == Path::new("scripts/check-style.sh")
            )
        });
        let lexical_findings = super::validate(
            &root(),
            &lexical_registry,
            &topology,
            &lexical_inventory,
            128,
            4_194_304,
        );
        assert!(
            lexical_findings.iter().any(|finding| finding.code
                == "ENTRYPOINT_REVIEW_LEXICAL_EDGE_MISSING")
        );

        let mut registry = super::Registry::parse(&registry()).unwrap();
        let mut inventory = current_inventory(&registry, &topology);
        let check = registry
            .reviews
            .iter_mut()
            .find(|review| {
                matches!(
                    &review.entrypoint,
                    super::ReviewedEntrypoint::Script { path, .. }
                        if path == std::path::Path::new("scripts/check.sh")
                )
            })
            .unwrap();
        if let super::ReviewedEntrypoint::Script { content_digest, .. } =
            &mut check.entrypoint
        {
            *content_digest = crate::digest::bytes(b"different entrypoint");
        }
        check.review_record_digest = crate::digest::bytes(b"different review");
        let record = inventory
            .iter_mut()
            .find(|file| file.path == check.review_record)
            .unwrap();
        record.classification = SourceClassification::new(
            SourceOwner::RepositoryTooling,
            SourceProvenance::Authored,
            SourceUse::Test,
        )
        .unwrap();

        let findings = super::validate(
            &root(),
            &registry,
            &topology,
            &inventory,
            128,
            4_194_304,
        );
        let codes = findings
            .iter()
            .map(|finding| finding.code)
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("ENTRYPOINT_REVIEW_CONTENT_DIGEST"));
        assert!(codes.contains("ENTRYPOINT_REVIEW_RECORD_DIGEST"));
        assert!(codes.contains("ENTRYPOINT_REVIEW_RECORD_CLASSIFICATION"));
    }
}
