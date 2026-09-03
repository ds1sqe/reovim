//! Exact per-workspace external-Cargo-package admission registry.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use reovim_depgraph::{SourceOwner, SourceProvenance, SourceUse};
use serde_json::Value;

use crate::{
    cargo::Metadata,
    digest,
    finding::{Finding, Findings},
    json::{self, ConfigError},
    policy::relative_path,
    source::ClassifiedFile,
};

pub const SCHEMA: &str = "reovim-dependency-reviews/v2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    pub reviewed_at: String,
    pub workspaces: Vec<WorkspaceReview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceReview {
    pub workspace: String,
    pub lockfile: PathBuf,
    pub lock_digest: String,
    pub admissions: Vec<Admission>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Admission {
    pub name: String,
    pub version: String,
    pub source: String,
    pub checksum: String,
    pub license: String,
    pub resolved_uses: Vec<ResolvedUse>,
    pub review_record: PathBuf,
    pub review_record_digest: String,
}

impl Admission {
    #[must_use]
    pub fn identity(&self) -> PackageIdentity {
        PackageIdentity {
            name: self.name.clone(),
            version: self.version.clone(),
            source: self.source.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageIdentity {
    pub name: String,
    pub version: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResolvedUse {
    pub profile: String,
    pub consumer: Consumer,
    pub dependency_name: String,
    pub kind: DependencyKind,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Consumer {
    WorkspacePackage { workspace: String, package: String },
    ExternalPackage(PackageIdentity),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DependencyKind {
    Normal,
    Development,
    Build,
}

/// Exact topology package identity paired with Cargo's package name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkspacePackage {
    pub id: String,
    pub cargo_name: String,
}

/// One topology profile and its exact Cargo metadata capture.
#[derive(Debug, Clone, Copy)]
pub struct ProfileMetadata<'a> {
    pub id: &'a str,
    pub metadata: &'a Metadata,
}

/// Caller-provided topology and metadata facts for one workspace.
#[derive(Debug)]
pub struct ValidationWorkspace<'a> {
    pub id: &'a str,
    pub lockfile: &'a Path,
    pub packages: &'a [WorkspacePackage],
    pub profiles: &'a [ProfileMetadata<'a>],
}

impl Registry {
    /// Parses a registry value already bounded and duplicate-key checked by the
    /// caller's JSON loader.
    pub fn parse(value: &Value) -> Result<Self, ConfigError> {
        let object = json::object(
            value,
            "$",
            &["schema", "reviewed_at", "workspaces"],
            &[],
        )?;
        exact(&object["schema"], "$.schema", SCHEMA)?;
        let reviewed_at = text(&object["reviewed_at"], "$.reviewed_at")?;
        if !crate::schema::utc_timestamp(&reviewed_at) {
            return Err(ConfigError::new(
                "REVIEW_TIMESTAMP",
                "$.reviewed_at",
                "expected a valid YYYY-MM-DDTHH:MM:SSZ UTC timestamp",
            ));
        }
        let workspaces = parse_workspaces(&object["workspaces"])?;
        Ok(Self { reviewed_at, workspaces })
    }
}

fn parse_workspaces(
    value: &Value,
) -> Result<Vec<WorkspaceReview>, ConfigError> {
    let values = json::array(value, "$.workspaces")?;
    if values.is_empty() {
        return Err(ConfigError::new(
            "REVIEW_EMPTY_WORKSPACES",
            "$.workspaces",
            "at least one workspace review is required",
        ));
    }
    let mut workspaces = Vec::with_capacity(values.len());
    let mut workspace_ids = BTreeSet::new();
    let mut lockfiles = BTreeSet::new();
    let mut record_digests = BTreeMap::new();
    for (item, value) in values.iter().enumerate() {
        let path = json::index("$.workspaces", item);
        let workspace = parse_workspace(value, &path)?;
        validate_workspace_identity(
            &workspace,
            &path,
            &mut workspace_ids,
            &mut lockfiles,
            &mut record_digests,
        )?;
        workspaces.push(workspace);
    }
    workspaces.sort_by(|left, right| left.workspace.cmp(&right.workspace));
    Ok(workspaces)
}

fn validate_workspace_identity(
    workspace: &WorkspaceReview,
    path: &str,
    workspace_ids: &mut BTreeSet<String>,
    lockfiles: &mut BTreeSet<PathBuf>,
    record_digests: &mut BTreeMap<PathBuf, String>,
) -> Result<(), ConfigError> {
    if !workspace_ids.insert(workspace.workspace.clone()) {
        return Err(ConfigError::new(
            "REVIEW_DUPLICATE_WORKSPACE",
            json::child(path, "workspace"),
            "duplicate workspace review",
        ));
    }
    if !lockfiles.insert(workspace.lockfile.clone()) {
        return Err(ConfigError::new(
            "REVIEW_DUPLICATE_LOCKFILE",
            json::child(path, "lockfile"),
            "one lockfile cannot provide authority to multiple workspace \
             reviews",
        ));
    }
    validate_record_digests(workspace, path, record_digests)
}

fn validate_record_digests(
    workspace: &WorkspaceReview,
    path: &str,
    record_digests: &mut BTreeMap<PathBuf, String>,
) -> Result<(), ConfigError> {
    for admission in &workspace.admissions {
        let existing = record_digests.insert(
            admission.review_record.clone(),
            admission.review_record_digest.clone(),
        );
        if existing
            .is_some_and(|digest| digest != admission.review_record_digest)
        {
            return Err(ConfigError::new(
                "REVIEW_RECORD_DIGEST_CONFLICT",
                json::child(path, "admissions"),
                "shared retained review record has inconsistent digests",
            ));
        }
    }
    Ok(())
}

fn parse_workspace(
    value: &Value,
    path: &str,
) -> Result<WorkspaceReview, ConfigError> {
    let object = json::object(
        value,
        path,
        &["workspace", "lockfile", "lock_digest", "admissions"],
        &[],
    )?;
    let workspace_id =
        id(&object["workspace"], &json::child(path, "workspace"))?;
    let values =
        json::array(&object["admissions"], &json::child(path, "admissions"))?;
    let mut admissions = Vec::with_capacity(values.len());
    let mut identities = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let item_path = json::index(&json::child(path, "admissions"), item);
        let admission = parse_admission(value, &item_path, &workspace_id)?;
        if !identities.insert(admission.identity()) {
            return Err(ConfigError::new(
                "REVIEW_DUPLICATE_PACKAGE",
                item_path,
                "duplicate external package admission in one workspace",
            ));
        }
        admissions.push(admission);
    }
    admissions.sort();
    Ok(WorkspaceReview {
        workspace: workspace_id,
        lockfile: relative_path(
            json::string(&object["lockfile"], &json::child(path, "lockfile"))?,
            &json::child(path, "lockfile"),
            false,
        )?,
        lock_digest: tagged_digest(
            &object["lock_digest"],
            &json::child(path, "lock_digest"),
        )?,
        admissions,
    })
}

fn parse_admission(
    value: &Value,
    path: &str,
    workspace: &str,
) -> Result<Admission, ConfigError> {
    let object = json::object(
        value,
        path,
        &[
            "name",
            "version",
            "source",
            "checksum",
            "license",
            "resolved_uses",
            "disposition",
            "review_record",
            "review_record_digest",
        ],
        &[],
    )?;
    let name = text(&object["name"], &json::child(path, "name"))?;
    let version = text(&object["version"], &json::child(path, "version"))?;
    let source = text(&object["source"], &json::child(path, "source"))?;
    let values = json::array(
        &object["resolved_uses"],
        &json::child(path, "resolved_uses"),
    )?;
    let mut resolved_uses = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let item_path = json::index(&json::child(path, "resolved_uses"), item);
        let resolved = parse_resolved_use(value, &item_path)?;
        if let Consumer::WorkspacePackage {
            workspace: consumer_workspace, ..
        } = &resolved.consumer
            && consumer_workspace != workspace
        {
            return Err(ConfigError::new(
                "REVIEW_USE_WORKSPACE",
                json::child(&item_path, "consumer.workspace"),
                concat!(
                    "workspace-package consumer must belong to the reviewed ",
                    "workspace"
                ),
            ));
        }
        if !resolved_uses.insert(resolved) {
            return Err(ConfigError::new(
                "REVIEW_DUPLICATE_USE",
                item_path,
                "duplicate resolved dependency use",
            ));
        }
    }
    exact(&object["disposition"], &json::child(path, "disposition"), "admit")?;
    Ok(Admission {
        name,
        version,
        source,
        checksum: plain_digest(
            &object["checksum"],
            &json::child(path, "checksum"),
        )?,
        license: text(&object["license"], &json::child(path, "license"))?,
        resolved_uses: resolved_uses.into_iter().collect(),
        review_record: relative_path(
            json::string(
                &object["review_record"],
                &json::child(path, "review_record"),
            )?,
            &json::child(path, "review_record"),
            false,
        )?,
        review_record_digest: tagged_digest(
            &object["review_record_digest"],
            &json::child(path, "review_record_digest"),
        )?,
    })
}

fn parse_resolved_use(
    value: &Value,
    path: &str,
) -> Result<ResolvedUse, ConfigError> {
    let object = json::object(
        value,
        path,
        &["profile", "consumer", "dependency_name", "kind", "target"],
        &[],
    )?;
    let kind_path = json::child(path, "kind");
    let kind = match json::string(&object["kind"], &kind_path)? {
        "normal" => DependencyKind::Normal,
        "development" => DependencyKind::Development,
        "build" => DependencyKind::Build,
        other => {
            return Err(ConfigError::new(
                "REVIEW_DEPENDENCY_KIND",
                kind_path,
                format!("unknown dependency kind {other:?}"),
            ));
        }
    };
    let target = if object["target"].is_null() {
        None
    } else {
        Some(text(&object["target"], &json::child(path, "target"))?)
    };
    Ok(ResolvedUse {
        profile: id(&object["profile"], &json::child(path, "profile"))?,
        consumer: parse_consumer(
            &object["consumer"],
            &json::child(path, "consumer"),
        )?,
        dependency_name: text(
            &object["dependency_name"],
            &json::child(path, "dependency_name"),
        )?,
        kind,
        target,
    })
}

fn parse_consumer(value: &Value, path: &str) -> Result<Consumer, ConfigError> {
    let discriminant = json::object(
        value,
        path,
        &["kind"],
        &["workspace", "package", "name", "version", "source"],
    )?;
    let kind_path = json::child(path, "kind");
    match json::string(&discriminant["kind"], &kind_path)? {
        "workspace_package" => {
            let object = json::object(
                value,
                path,
                &["kind", "workspace", "package"],
                &[],
            )?;
            Ok(Consumer::WorkspacePackage {
                workspace: id(
                    &object["workspace"],
                    &json::child(path, "workspace"),
                )?,
                package: id(&object["package"], &json::child(path, "package"))?,
            })
        }
        "external_package" => {
            let object = json::object(
                value,
                path,
                &["kind", "name", "version", "source"],
                &[],
            )?;
            Ok(Consumer::ExternalPackage(PackageIdentity {
                name: text(&object["name"], &json::child(path, "name"))?,
                version: text(
                    &object["version"],
                    &json::child(path, "version"),
                )?,
                source: text(&object["source"], &json::child(path, "source"))?,
            }))
        }
        other => Err(ConfigError::new(
            "REVIEW_CONSUMER_KIND",
            kind_path,
            format!("unknown consumer kind {other:?}"),
        )),
    }
}

/// Cross-validates all v2 admissions against exact lock bytes, retained review
/// records, topology identities, and Cargo metadata's incoming dependency
/// edges for every declared profile.
pub fn validate(
    root: &Path,
    registry: &Registry,
    workspaces: &[ValidationWorkspace<'_>],
    finding_limit: usize,
    file_byte_limit: u64,
    inventory: &[ClassifiedFile],
) -> Vec<Finding> {
    let mut findings = Findings::new(finding_limit);
    let mut contexts = BTreeMap::new();
    for workspace in workspaces {
        if contexts.insert(workspace.id, workspace).is_some() {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_DUPLICATE_VALIDATION_WORKSPACE",
                None,
                format!(
                    "duplicate validation context for workspace {}",
                    workspace.id
                ),
            ));
        }
    }
    let reviews = registry
        .workspaces
        .iter()
        .map(|workspace| (workspace.workspace.as_str(), workspace))
        .collect::<BTreeMap<_, _>>();
    for (id, review) in &reviews {
        let Some(context) = contexts.get(id) else {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_WORKSPACE_UNRESOLVED",
                Some(review.lockfile.to_string_lossy().into_owned()),
                format!(
                    "reviewed workspace {id} is absent from topology evidence"
                ),
            ));
            continue;
        };
        validate_workspace(
            root,
            review,
            context,
            file_byte_limit,
            inventory,
            &mut findings,
        );
    }
    for id in contexts.keys() {
        if !reviews.contains_key(id) {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_WORKSPACE_MISSING",
                None,
                format!(
                    "topology workspace {id} lacks a dependency-review \
                     workspace entry"
                ),
            ));
        }
    }
    findings.finish()
}

fn validate_workspace(
    root: &Path,
    review: &WorkspaceReview,
    context: &ValidationWorkspace<'_>,
    file_byte_limit: u64,
    inventory: &[ClassifiedFile],
    findings: &mut Findings,
) {
    if review.lockfile != context.lockfile {
        findings.push(Finding::new(
            "dependency_review",
            "REVIEW_LOCKFILE_DRIFT",
            Some(review.lockfile.to_string_lossy().into_owned()),
            format!("topology names lockfile {}", context.lockfile.display()),
        ));
    }
    let Some(external_lock) =
        validate_lock(root, review, file_byte_limit, inventory, findings)
    else {
        return;
    };
    validate_review_records(root, review, file_byte_limit, inventory, findings);

    let (package_ids, package_names) =
        validate_topology_packages(context, findings);
    let profile_ids = validate_profile_ids(context, findings);
    validate_declared_uses(review, &profile_ids, &package_ids, findings);

    let actual = collect_actual_uses(context, &package_names, findings);
    let admissions = review
        .admissions
        .iter()
        .map(|admission| (admission.identity(), admission))
        .collect::<BTreeMap<_, _>>();
    validate_lock_admissions(&external_lock, &admissions, findings);
    validate_admissions(&external_lock, &admissions, &actual, findings);
    let admitted_identities =
        admissions.keys().cloned().collect::<BTreeSet<_>>();
    for identity in actual.resolved.difference(&admitted_identities) {
        findings.push(Finding::new(
            "dependency_review",
            "UNREVIEWED_RESOLVED_EXTERNAL_PACKAGE",
            None,
            format!(
                "{} {} ({})",
                identity.name, identity.version, identity.source
            ),
        ));
    }
}

fn validate_topology_packages<'a>(
    context: &'a ValidationWorkspace<'_>,
    findings: &mut Findings,
) -> (BTreeSet<&'a str>, BTreeMap<&'a str, &'a str>) {
    let mut ids = BTreeSet::new();
    let mut names = BTreeMap::new();
    for package in context.packages {
        let duplicate_id = !ids.insert(package.id.as_str());
        let duplicate_name = names
            .insert(package.cargo_name.as_str(), package.id.as_str())
            .is_some();
        if duplicate_id || duplicate_name {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_TOPOLOGY_PACKAGE_ALIAS",
                None,
                "workspace validation package IDs and Cargo names must be \
                 one-to-one",
            ));
        }
    }
    (ids, names)
}

fn validate_profile_ids<'a>(
    context: &'a ValidationWorkspace<'_>,
    findings: &mut Findings,
) -> BTreeSet<&'a str> {
    let mut ids = BTreeSet::new();
    for profile in context.profiles {
        if !ids.insert(profile.id) {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_DUPLICATE_PROFILE_CAPTURE",
                None,
                format!("duplicate metadata profile {}", profile.id),
            ));
        }
    }
    ids
}

fn validate_declared_uses(
    review: &WorkspaceReview,
    profile_ids: &BTreeSet<&str>,
    package_ids: &BTreeSet<&str>,
    findings: &mut Findings,
) {
    for admission in &review.admissions {
        validate_admission_uses(admission, profile_ids, package_ids, findings);
    }
}

fn validate_admission_uses(
    admission: &Admission,
    profile_ids: &BTreeSet<&str>,
    package_ids: &BTreeSet<&str>,
    findings: &mut Findings,
) {
    for resolved in &admission.resolved_uses {
        if !profile_ids.contains(resolved.profile.as_str()) {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_UNKNOWN_PROFILE",
                Some(admission.review_record.to_string_lossy().into_owned()),
                format!(
                    "resolved use names unknown profile {}",
                    resolved.profile
                ),
            ));
        }
        validate_workspace_use(admission, resolved, package_ids, findings);
    }
}

fn validate_workspace_use(
    admission: &Admission,
    resolved: &ResolvedUse,
    package_ids: &BTreeSet<&str>,
    findings: &mut Findings,
) {
    let Consumer::WorkspacePackage { package, .. } = &resolved.consumer else {
        return;
    };
    if !package_ids.contains(package.as_str()) {
        findings.push(Finding::new(
            "dependency_review",
            "REVIEW_UNKNOWN_WORKSPACE_PACKAGE",
            Some(admission.review_record.to_string_lossy().into_owned()),
            format!("resolved use names unknown topology package {package}"),
        ));
    }
}

fn validate_lock_admissions(
    external_lock: &BTreeMap<PackageIdentity, LockPackage>,
    admissions: &BTreeMap<PackageIdentity, &Admission>,
    findings: &mut Findings,
) {
    for (identity, package) in external_lock {
        validate_lock_admission(identity, package, admissions, findings);
    }
}

fn validate_lock_admission(
    identity: &PackageIdentity,
    package: &LockPackage,
    admissions: &BTreeMap<PackageIdentity, &Admission>,
    findings: &mut Findings,
) {
    let Some(admission) = admissions.get(identity) else {
        findings.push(Finding::new(
            "dependency_review",
            "UNREVIEWED_EXTERNAL_PACKAGE",
            None,
            format!(
                "{} {} ({})",
                identity.name, identity.version, identity.source
            ),
        ));
        return;
    };
    if package.checksum.as_deref() != Some(admission.checksum.as_str()) {
        findings.push(Finding::new(
            "dependency_review",
            "REVIEW_CHECKSUM_DRIFT",
            Some(admission.review_record.to_string_lossy().into_owned()),
            format!(
                "{} {} checksum differs from Cargo.lock",
                identity.name, identity.version
            ),
        ));
    }
}

fn validate_admissions(
    external_lock: &BTreeMap<PackageIdentity, LockPackage>,
    admissions: &BTreeMap<PackageIdentity, &Admission>,
    actual: &ActualUses<'_>,
    findings: &mut Findings,
) {
    for (identity, admission) in admissions {
        validate_admission(
            identity,
            admission,
            external_lock,
            actual,
            findings,
        );
    }
}

fn validate_admission(
    identity: &PackageIdentity,
    admission: &Admission,
    external_lock: &BTreeMap<PackageIdentity, LockPackage>,
    actual: &ActualUses<'_>,
    findings: &mut Findings,
) {
    validate_admission_presence(identity, admission, external_lock, findings);
    let declared =
        admission.resolved_uses.iter().cloned().collect::<BTreeSet<_>>();
    let observed = actual.uses.get(identity).cloned().unwrap_or_default();
    validate_use_sets(
        admission, &declared, &observed, actual, identity, findings,
    );
    if let Some(metadata) = actual.packages.get(identity) {
        validate_metadata(admission, metadata, findings);
    }
}

fn validate_admission_presence(
    identity: &PackageIdentity,
    admission: &Admission,
    external_lock: &BTreeMap<PackageIdentity, LockPackage>,
    findings: &mut Findings,
) {
    if !external_lock.contains_key(identity) {
        findings.push(Finding::new(
            "dependency_review",
            "STALE_DEPENDENCY_ADMISSION",
            Some(admission.review_record.to_string_lossy().into_owned()),
            format!(
                "{} {} is absent from Cargo.lock",
                admission.name, admission.version
            ),
        ));
    }
}

fn validate_use_sets(
    admission: &Admission,
    declared: &BTreeSet<ResolvedUse>,
    observed: &BTreeSet<ResolvedUse>,
    actual: &ActualUses<'_>,
    identity: &PackageIdentity,
    findings: &mut Findings,
) {
    if declared != observed {
        findings.push(Finding::new(
            "dependency_review",
            "REVIEW_USE_DRIFT",
            Some(admission.review_record.to_string_lossy().into_owned()),
            format!(
                "{} {} declared {} exact uses; Cargo metadata reported {}",
                admission.name,
                admission.version,
                declared.len(),
                observed.len()
            ),
        ));
    }
    validate_resolved_use_state(
        admission, declared, actual, identity, findings,
    );
}

fn validate_resolved_use_state(
    admission: &Admission,
    declared: &BTreeSet<ResolvedUse>,
    actual: &ActualUses<'_>,
    identity: &PackageIdentity,
    findings: &mut Findings,
) {
    if actual.resolved.contains(identity) && declared.is_empty() {
        findings.push(Finding::new(
            "dependency_review",
            "REVIEW_RESOLVED_PACKAGE_WITHOUT_USE",
            Some(admission.review_record.to_string_lossy().into_owned()),
            format!(
                "resolved package {} {} has no incoming edge",
                admission.name, admission.version
            ),
        ));
    }
    if !actual.resolved.contains(identity) && !declared.is_empty() {
        findings.push(Finding::new(
            "dependency_review",
            "REVIEW_LOCK_ONLY_HAS_USES",
            Some(admission.review_record.to_string_lossy().into_owned()),
            format!(
                "lock-only package {} {} declares resolved uses",
                admission.name, admission.version
            ),
        ));
    }
}

fn validate_metadata(
    admission: &Admission,
    metadata: &crate::cargo::Package,
    findings: &mut Findings,
) {
    if metadata.license.as_deref() != Some(admission.license.as_str()) {
        findings.push(Finding::new(
            "dependency_review",
            "REVIEW_LICENSE_DRIFT",
            Some(admission.review_record.to_string_lossy().into_owned()),
            format!(
                "{} {} license differs from Cargo metadata",
                admission.name, admission.version
            ),
        ));
    }
    if metadata
        .checksum
        .as_deref()
        .is_some_and(|checksum| checksum != admission.checksum)
    {
        findings.push(Finding::new(
            "dependency_review",
            "REVIEW_METADATA_CHECKSUM_DRIFT",
            Some(admission.review_record.to_string_lossy().into_owned()),
            format!(
                "{} {} checksum differs from Cargo metadata",
                admission.name, admission.version
            ),
        ));
    }
}

fn validate_lock(
    root: &Path,
    review: &WorkspaceReview,
    file_byte_limit: u64,
    inventory: &[ClassifiedFile],
    findings: &mut Findings,
) -> Option<BTreeMap<PackageIdentity, LockPackage>> {
    let lock_bytes = match crate::source::read_bounded(
        &root.join(&review.lockfile),
        file_byte_limit,
    ) {
        Ok(bytes) => bytes,
        Err(error) => {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_LOCK_IO",
                Some(review.lockfile.to_string_lossy().into_owned()),
                error,
            ));
            return None;
        }
    };
    let actual_digest = digest::bytes(&lock_bytes);
    if actual_digest != review.lock_digest {
        findings.push(Finding::new(
            "dependency_review",
            "REVIEW_LOCK_DIGEST_MISMATCH",
            Some(review.lockfile.to_string_lossy().into_owned()),
            format!("registry {}, actual {actual_digest}", review.lock_digest),
        ));
    }
    if !inventory.iter().any(|file| {
        file.path == review.lockfile && file.content_digest == actual_digest
    }) {
        findings.push(Finding::new(
            "dependency_review",
            "REVIEW_LOCK_INVENTORY_DRIFT",
            Some(review.lockfile.to_string_lossy().into_owned()),
            "parsed Cargo.lock bytes differ from the inventoried bytes",
        ));
    }
    match parse_lock(&lock_bytes) {
        Ok(packages) => Some(
            packages
                .into_iter()
                .filter_map(|package| {
                    let source = package.source.clone()?;
                    Some((
                        PackageIdentity {
                            name: package.name.clone(),
                            version: package.version.clone(),
                            source,
                        },
                        package,
                    ))
                })
                .collect(),
        ),
        Err(error) => {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_LOCK_PARSE",
                Some(review.lockfile.to_string_lossy().into_owned()),
                error,
            ));
            None
        }
    }
}

fn validate_review_records(
    root: &Path,
    review: &WorkspaceReview,
    file_byte_limit: u64,
    inventory: &[ClassifiedFile],
    findings: &mut Findings,
) {
    let records = review
        .admissions
        .iter()
        .map(|admission| {
            (&admission.review_record, &admission.review_record_digest)
        })
        .collect::<BTreeMap<_, _>>();
    for (path, expected_digest) in records {
        let display = path.to_string_lossy().into_owned();
        let Some(file) = inventory.iter().find(|file| file.path == *path)
        else {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_RECORD_MISSING",
                Some(display),
                "retained review record is absent from classified inventory",
            ));
            continue;
        };
        if file.classification.owner != SourceOwner::RepositoryTooling
            || file.classification.provenance != SourceProvenance::Authored
            || file.classification.source_use != SourceUse::Tool
        {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_RECORD_CLASSIFICATION",
                Some(display.clone()),
                "review record must be RepositoryTooling + Authored + Tool",
            ));
        }
        if file.content_digest != expected_digest.as_str() {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_RECORD_DIGEST",
                Some(display.clone()),
                "inventory and admission review-record digests differ",
            ));
        }
        match crate::source::read_bounded(&root.join(path), file_byte_limit) {
            Ok(bytes) if digest::bytes(&bytes) == expected_digest.as_str() => {}
            Ok(bytes) => findings.push(Finding::new(
                "dependency_review",
                "REVIEW_RECORD_BYTES",
                Some(display),
                format!(
                    "actual review-record digest is {}",
                    digest::bytes(&bytes)
                ),
            )),
            Err(error) => findings.push(Finding::new(
                "dependency_review",
                "REVIEW_RECORD_IO",
                Some(display),
                error,
            )),
        }
    }
}

#[derive(Debug, Default)]
struct ActualUses<'a> {
    uses: BTreeMap<PackageIdentity, BTreeSet<ResolvedUse>>,
    resolved: BTreeSet<PackageIdentity>,
    packages: BTreeMap<PackageIdentity, &'a crate::cargo::Package>,
}

fn collect_actual_uses<'a>(
    context: &ValidationWorkspace<'a>,
    package_names: &BTreeMap<&str, &str>,
    findings: &mut Findings,
) -> ActualUses<'a> {
    let mut result = ActualUses::default();
    let expected_workspace_packages =
        package_names.keys().copied().collect::<BTreeSet<_>>();
    for profile in context.profiles {
        collect_profile_uses(
            context,
            profile,
            package_names,
            &expected_workspace_packages,
            &mut result,
            findings,
        );
    }
    result
}

fn collect_profile_uses<'a>(
    context: &ValidationWorkspace<'a>,
    profile: &ProfileMetadata<'a>,
    package_names: &BTreeMap<&str, &str>,
    expected_workspace_packages: &BTreeSet<&str>,
    result: &mut ActualUses<'a>,
    findings: &mut Findings,
) {
    validate_workspace_package_set(
        profile,
        expected_workspace_packages,
        findings,
    );
    let resolved = profile.metadata.resolved_package_ids();
    collect_resolved_packages(profile.metadata, &resolved, result);
    collect_dependency_uses(
        context,
        profile,
        package_names,
        &resolved,
        result,
        findings,
    );
}

fn validate_workspace_package_set(
    profile: &ProfileMetadata<'_>,
    expected: &BTreeSet<&str>,
    findings: &mut Findings,
) {
    let actual = profile
        .metadata
        .workspace_members
        .iter()
        .filter_map(|id| profile.metadata.packages.get(id))
        .map(|package| package.name.as_str())
        .collect::<BTreeSet<_>>();
    if actual != *expected {
        findings.push(Finding::new(
            "dependency_review",
            "REVIEW_WORKSPACE_PACKAGE_DRIFT",
            None,
            format!(
                "profile {} workspace package set differs from topology",
                profile.id
            ),
        ));
    }
}

fn collect_resolved_packages<'a>(
    metadata: &'a Metadata,
    resolved: &BTreeSet<String>,
    result: &mut ActualUses<'a>,
) {
    for id in resolved {
        let Some(package) = metadata.packages.get(id) else {
            continue;
        };
        let Some(identity) = package_identity(package) else {
            continue;
        };
        result.resolved.insert(identity.clone());
        result.packages.entry(identity).or_insert(package);
    }
}

fn package_identity(
    package: &crate::cargo::Package,
) -> Option<PackageIdentity> {
    Some(PackageIdentity {
        name: package.name.clone(),
        version: package.version.clone(),
        source: package.source.clone()?,
    })
}

fn collect_dependency_uses<'a>(
    context: &ValidationWorkspace<'a>,
    profile: &ProfileMetadata<'a>,
    package_names: &BTreeMap<&str, &str>,
    resolved: &BTreeSet<String>,
    result: &mut ActualUses<'a>,
    findings: &mut Findings,
) {
    for (consumer_id, node) in &profile.metadata.nodes {
        collect_consumer_uses(
            context,
            profile,
            package_names,
            resolved,
            (consumer_id, node),
            result,
            findings,
        );
    }
}

fn collect_consumer_uses<'a>(
    context: &ValidationWorkspace<'a>,
    profile: &ProfileMetadata<'a>,
    package_names: &BTreeMap<&str, &str>,
    resolved: &BTreeSet<String>,
    consumer_entry: (&str, &crate::cargo::Node),
    result: &mut ActualUses<'a>,
    findings: &mut Findings,
) {
    let (consumer_id, node) = consumer_entry;
    if !resolved.contains(consumer_id) {
        return;
    }
    let Some(package) = profile.metadata.packages.get(consumer_id) else {
        return;
    };
    let Some(consumer) =
        resolve_consumer(context, profile, package_names, package, findings)
    else {
        return;
    };
    for dependency in &node.dependencies {
        collect_provider_uses(
            profile, resolved, &consumer, dependency, result, findings,
        );
    }
}

fn resolve_consumer(
    context: &ValidationWorkspace<'_>,
    profile: &ProfileMetadata<'_>,
    package_names: &BTreeMap<&str, &str>,
    package: &crate::cargo::Package,
    findings: &mut Findings,
) -> Option<Consumer> {
    if let Some(identity) = package_identity(package) {
        return Some(Consumer::ExternalPackage(identity));
    }
    if let Some(package_id) = package_names.get(package.name.as_str()) {
        return Some(Consumer::WorkspacePackage {
            workspace: context.id.to_owned(),
            package: (*package_id).to_owned(),
        });
    }
    findings.push(Finding::new(
        "dependency_review",
        "REVIEW_METADATA_CONSUMER_UNMAPPED",
        None,
        format!(
            "profile {} workspace package {} lacks topology identity",
            profile.id, package.name
        ),
    ));
    None
}

fn collect_provider_uses(
    profile: &ProfileMetadata<'_>,
    resolved: &BTreeSet<String>,
    consumer: &Consumer,
    dependency: &crate::cargo::ResolvedDependency,
    result: &mut ActualUses<'_>,
    findings: &mut Findings,
) {
    if !resolved.contains(&dependency.package) {
        return;
    }
    let Some(provider) = profile.metadata.packages.get(&dependency.package)
    else {
        return;
    };
    let Some(identity) = package_identity(provider) else {
        return;
    };
    for cargo_kind in &dependency.kinds {
        collect_dependency_kind(
            profile, consumer, dependency, cargo_kind, &identity, result,
            findings,
        );
    }
}

fn collect_dependency_kind(
    profile: &ProfileMetadata<'_>,
    consumer: &Consumer,
    dependency: &crate::cargo::ResolvedDependency,
    cargo_kind: &crate::cargo::DependencyKind,
    provider: &PackageIdentity,
    result: &mut ActualUses<'_>,
    findings: &mut Findings,
) {
    let Some(kind) = dependency_kind(cargo_kind, findings) else {
        return;
    };
    result.uses.entry(provider.clone()).or_default().insert(ResolvedUse {
        profile: profile.id.to_owned(),
        consumer: consumer.clone(),
        dependency_name: dependency.name.clone(),
        kind,
        target: cargo_kind.target.clone(),
    });
}

fn dependency_kind(
    cargo_kind: &crate::cargo::DependencyKind,
    findings: &mut Findings,
) -> Option<DependencyKind> {
    let kind = match cargo_kind.kind.as_str() {
        "normal" => DependencyKind::Normal,
        "development" => DependencyKind::Development,
        "build" => DependencyKind::Build,
        other => {
            findings.push(Finding::new(
                "dependency_review",
                "REVIEW_METADATA_DEPENDENCY_KIND",
                None,
                format!(
                    "Cargo parser supplied unknown dependency kind {other:?}"
                ),
            ));
            return None;
        }
    };
    Some(kind)
}

#[derive(Debug)]
struct LockPackage {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
}

fn parse_lock(bytes: &[u8]) -> Result<Vec<LockPackage>, String> {
    let input =
        std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let mut parser = LockParser::default();
    for (line_number, raw) in input.lines().enumerate() {
        parser.consume_line(raw.trim(), line_number)?;
    }
    let packages = parser.finish(input.lines().count())?;
    validate_lock_identities(&packages)?;
    Ok(packages)
}

#[derive(Default)]
struct LockParser {
    packages: Vec<LockPackage>,
    fields: BTreeMap<String, String>,
    in_package: bool,
    format_version: Option<String>,
}

impl LockParser {
    fn consume_line(
        &mut self,
        line: &str,
        line_number: usize,
    ) -> Result<(), String> {
        if line == "[[package]]" {
            self.start_package(line_number)?;
            return Ok(());
        }
        if line.starts_with("[[") {
            self.leave_package(line_number)?;
            return Ok(());
        }
        if !self.in_package && line.starts_with("version") {
            return self.parse_format_version(line, line_number);
        }
        if self.skip_line(line) {
            return Ok(());
        }
        self.parse_package_field(line, line_number)
    }

    fn start_package(&mut self, line_number: usize) -> Result<(), String> {
        if self.in_package {
            self.finish_package(line_number)?;
        }
        self.in_package = true;
        Ok(())
    }

    fn leave_package(&mut self, line_number: usize) -> Result<(), String> {
        if self.in_package {
            self.finish_package(line_number)?;
            self.in_package = false;
        }
        Ok(())
    }

    fn finish_package(&mut self, line_number: usize) -> Result<(), String> {
        self.packages.push(lock_package(&self.fields, line_number)?);
        self.fields.clear();
        Ok(())
    }

    fn parse_format_version(
        &mut self,
        line: &str,
        line_number: usize,
    ) -> Result<(), String> {
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!(
                "malformed Cargo.lock version at line {}",
                line_number + 1
            ));
        };
        if key.trim() != "version"
            || self.format_version.replace(value.trim().to_owned()).is_some()
        {
            return Err(
                "duplicate or unsupported Cargo.lock format version".to_owned()
            );
        }
        Ok(())
    }

    fn skip_line(&self, line: &str) -> bool {
        !self.in_package
            || line.is_empty()
            || line.starts_with('#')
            || line.starts_with('[')
    }

    fn parse_package_field(
        &mut self,
        line: &str,
        line_number: usize,
    ) -> Result<(), String> {
        let Some((key, value)) = line.split_once('=') else {
            return Ok(());
        };
        let key = key.trim();
        if key == "replace" {
            return Err(format!(
                "unsupported Cargo.lock replace identity at line {}",
                line_number + 1
            ));
        }
        if !matches!(key, "name" | "version" | "source" | "checksum") {
            return Ok(());
        }
        let value = parse_quoted(value.trim())?;
        if self.fields.insert(key.to_owned(), value).is_some() {
            return Err(format!(
                "duplicate {key} in Cargo.lock package near line {}",
                line_number + 1
            ));
        }
        Ok(())
    }

    fn finish(mut self, line_count: usize) -> Result<Vec<LockPackage>, String> {
        if self.in_package {
            self.finish_package(line_count)?;
        }
        if self.format_version.as_deref() != Some("4") {
            return Err(format!(
                "unsupported Cargo.lock format version {:?}; expected 4",
                self.format_version
            ));
        }
        Ok(self.packages)
    }
}

fn validate_lock_identities(packages: &[LockPackage]) -> Result<(), String> {
    let mut identities = BTreeSet::new();
    for package in packages {
        let identity = (
            package.name.clone(),
            package.version.clone(),
            package.source.clone(),
        );
        if !identities.insert(identity) {
            return Err(format!(
                "duplicate Cargo.lock package {} {}",
                package.name, package.version
            ));
        }
    }
    Ok(())
}

fn lock_package(
    fields: &BTreeMap<String, String>,
    line: usize,
) -> Result<LockPackage, String> {
    let name = fields
        .get("name")
        .ok_or_else(|| {
            format!("Cargo.lock package near line {line} lacks name")
        })?
        .clone();
    let version = fields
        .get("version")
        .ok_or_else(|| format!("Cargo.lock package {name} lacks version"))?
        .clone();
    let source = fields.get("source").cloned();
    let checksum = fields.get("checksum").cloned();
    if source.is_some() && checksum.is_none() {
        return Err(format!(
            "external Cargo.lock package {name} {version} lacks checksum"
        ));
    }
    if source.is_none() && checksum.is_some() {
        return Err(format!(
            "workspace Cargo.lock package {name} {version} has checksum"
        ));
    }
    Ok(LockPackage { name, version, source, checksum })
}

fn parse_quoted(value: &str) -> Result<String, String> {
    serde_json::from_str::<String>(value).map_err(|error| {
        format!("unsupported Cargo.lock string {value:?}: {error}")
    })
}

fn id(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = json::string(value, path)?;
    if valid_id(value) {
        Ok(value.to_owned())
    } else {
        Err(ConfigError::new(
            "REVIEW_ID",
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

fn text(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = json::string(value, path)?;
    if value.is_empty() {
        Err(ConfigError::new(
            "REVIEW_EMPTY_STRING",
            path,
            "value must not be empty",
        ))
    } else {
        Ok(value.to_owned())
    }
}

fn exact(value: &Value, path: &str, expected: &str) -> Result<(), ConfigError> {
    let actual = json::string(value, path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(ConfigError::new(
            "CFG_CONST",
            path,
            format!("expected {expected:?}"),
        ))
    }
}

fn tagged_digest(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = json::string(value, path)?;
    if value.len() == 71
        && value.starts_with("sha256:")
        && lower_hex(&value[7..])
    {
        Ok(value.to_owned())
    } else {
        Err(ConfigError::new(
            "REVIEW_DIGEST",
            path,
            "expected sha256:<64 lowercase hex>",
        ))
    }
}

fn plain_digest(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = json::string(value, path)?;
    if value.len() == 64 && lower_hex(value) {
        Ok(value.to_owned())
    } else {
        Err(ConfigError::new(
            "REVIEW_CHECKSUM",
            path,
            "expected Cargo's 64 lowercase hex checksum",
        ))
    }
}

fn lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
        SourceClassification, SourceOwner, SourceProvenance, SourceUse,
    };
    use serde_json::{Value, json};

    use crate::{
        cargo::{DependencyKind, Metadata, Node, Package, ResolvedDependency},
        source::ClassifiedFile,
    };

    static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

    fn registry() -> Value {
        serde_json::from_str(include_str!(
            "../../../conformance/dependency-reviews.json"
        ))
        .unwrap()
    }

    fn one_admission_registry(record_digest: &str, lock_digest: &str) -> Value {
        let checksum = fixture_checksum();
        let review_record = "conformance/dependency-reviews/external.md";
        json!({
            "schema": super::SCHEMA,
            "reviewed_at": "2026-09-01T00:00:00Z",
            "workspaces": [{
                "workspace": "repository-tooling",
                "lockfile": "Cargo.lock",
                "lock_digest": lock_digest,
                "admissions": [{
                    "name": "external",
                    "version": "1.0.0",
                    "source": "registry+exact",
                    "checksum": checksum,
                    "license": "MIT",
                    "resolved_uses": [{
                        "profile": "hosted-default",
                        "consumer": {
                            "kind": "workspace_package",
                            "workspace": "repository-tooling",
                            "package": "tool"
                        },
                        "dependency_name": "external_alias",
                        "kind": "normal",
                        "target": "cfg(target_os = \"linux\")"
                    }],
                    "disposition": "admit",
                    "review_record": review_record,
                    "review_record_digest": record_digest
                }]
            }]
        })
    }

    #[test]
    fn current_registry_has_exact_workspace_packages_profiles_and_records() {
        let registry = super::Registry::parse(&registry()).unwrap();
        let workspace_ids = registry
            .workspaces
            .iter()
            .map(|item| item.workspace.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            workspace_ids,
            BTreeSet::from(["product", "repository-tooling"])
        );
        let workspace = registry
            .workspaces
            .iter()
            .find(|item| item.workspace == "repository-tooling")
            .unwrap();
        let lock_only = workspace
            .admissions
            .iter()
            .filter(|item| item.resolved_uses.is_empty())
            .map(|item| item.name.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(lock_only, BTreeSet::from(["serde", "serde_derive"]));
        let nix = workspace
            .admissions
            .iter()
            .find(|item| item.name == "nix")
            .unwrap();
        assert!(nix.resolved_uses.iter().all(|item| {
            item.target.as_deref() == Some("cfg(target_os = \"linux\")")
                && item.dependency_name == "nix"
        }));
        let product = registry
            .workspaces
            .iter()
            .find(|item| item.workspace == "product")
            .unwrap();
        assert_eq!(product.admissions, Vec::new());
    }

    #[test]
    fn closed_fields_timestamp_ids_paths_digests_and_disposition_fail() {
        let mut unknown = registry();
        unknown["workspaces"][0]["surprise"] = json!(true);
        assert_eq!(
            super::Registry::parse(&unknown).unwrap_err().code,
            "CFG_UNKNOWN_FIELD"
        );

        let mut timestamp = registry();
        timestamp["reviewed_at"] = json!("2026-02-29T25:00:00Z");
        assert_eq!(
            super::Registry::parse(&timestamp).unwrap_err().code,
            "REVIEW_TIMESTAMP"
        );

        let mut id = registry();
        id["workspaces"][0]["workspace"] = json!("Bad_ID");
        assert_eq!(super::Registry::parse(&id).unwrap_err().code, "REVIEW_ID");

        let mut path = registry();
        path["workspaces"][0]["lockfile"] = json!("../Cargo.lock");
        assert_eq!(
            super::Registry::parse(&path).unwrap_err().code,
            "CFG_PATH_ESCAPE"
        );

        let mut digest = registry();
        digest["workspaces"][0]["admissions"][0]["checksum"] = json!("ABC");
        assert_eq!(
            super::Registry::parse(&digest).unwrap_err().code,
            "REVIEW_CHECKSUM"
        );

        let mut disposition = registry();
        disposition["workspaces"][0]["admissions"][0]["disposition"] =
            json!("observe");
        assert_eq!(
            super::Registry::parse(&disposition).unwrap_err().code,
            "CFG_CONST"
        );
    }

    #[test]
    fn duplicate_workspace_lock_package_use_and_record_conflicts_fail() {
        let mut workspace = registry();
        let duplicate = workspace["workspaces"][0].clone();
        workspace["workspaces"].as_array_mut().unwrap().push(duplicate);
        assert_eq!(
            super::Registry::parse(&workspace).unwrap_err().code,
            "REVIEW_DUPLICATE_WORKSPACE"
        );

        let mut package = registry();
        let duplicate = package["workspaces"][0]["admissions"][0].clone();
        package["workspaces"][0]["admissions"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert_eq!(
            super::Registry::parse(&package).unwrap_err().code,
            "REVIEW_DUPLICATE_PACKAGE"
        );

        let mut resolved = registry();
        let duplicate =
            resolved["workspaces"][0]["admissions"][0]["resolved_uses"][0]
                .clone();
        resolved["workspaces"][0]["admissions"][0]["resolved_uses"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert_eq!(
            super::Registry::parse(&resolved).unwrap_err().code,
            "REVIEW_DUPLICATE_USE"
        );

        let mut record = registry();
        record["workspaces"][0]["admissions"][1]["review_record"] =
            record["workspaces"][0]["admissions"][0]["review_record"].clone();
        record["workspaces"][0]["admissions"][1]["review_record_digest"] =
            json!(crate::digest::bytes(b"conflicting review record"));
        assert_eq!(
            super::Registry::parse(&record).unwrap_err().code,
            "REVIEW_RECORD_DIGEST_CONFLICT"
        );
    }

    #[test]
    fn consumer_shape_workspace_and_dependency_kind_are_closed() {
        let mut workspace = registry();
        let nix = workspace["workspaces"][0]["admissions"]
            .as_array()
            .unwrap()
            .iter()
            .position(|admission| admission["name"] == "nix")
            .unwrap();
        let admission = &mut workspace["workspaces"][0]["admissions"][nix];
        let consumer = &mut admission["resolved_uses"][0]["consumer"];
        consumer["workspace"] = json!("other");
        assert_eq!(
            super::Registry::parse(&workspace).unwrap_err().code,
            "REVIEW_USE_WORKSPACE"
        );

        let mut kind = registry();
        kind["workspaces"][0]["admissions"][nix]["resolved_uses"][0]["kind"] =
            json!("optional");
        assert_eq!(
            super::Registry::parse(&kind).unwrap_err().code,
            "REVIEW_DEPENDENCY_KIND"
        );

        let mut mixed = registry();
        let admission = &mut mixed["workspaces"][0]["admissions"][nix];
        let consumer = &mut admission["resolved_uses"][0]["consumer"];
        consumer["name"] = json!("smuggled");
        assert_eq!(
            super::Registry::parse(&mixed).unwrap_err().code,
            "CFG_UNKNOWN_FIELD"
        );
    }

    #[test]
    fn lock_parser_rejects_format_duplicate_identity_and_missing_checksum() {
        assert!(
            super::parse_lock(b"version = 3\n")
                .unwrap_err()
                .contains("expected 4")
        );
        let duplicate = br#"version = 4
[[package]]
name = "x"
version = "1"
source = "registry+x"
checksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
[[package]]
name = "x"
version = "1"
source = "registry+x"
checksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
"#;
        assert!(
            super::parse_lock(duplicate)
                .unwrap_err()
                .contains("duplicate Cargo.lock package")
        );
        let missing = br#"version = 4
[[package]]
name = "x"
version = "1"
source = "registry+x"
"#;
        assert!(
            super::parse_lock(missing).unwrap_err().contains("lacks checksum")
        );
    }

    #[test]
    fn exact_metadata_edge_and_lock_only_rules_validate() {
        let fixture = Fixture::new();
        let registry = super::Registry::parse(&one_admission_registry(
            &fixture.record_digest,
            &fixture.lock_digest,
        ))
        .unwrap();
        let metadata = metadata();
        let packages = [super::WorkspacePackage {
            id: "tool".to_owned(),
            cargo_name: "tool".to_owned(),
        }];
        let profiles = [super::ProfileMetadata {
            id: "hosted-default",
            metadata: &metadata,
        }];
        let contexts = [super::ValidationWorkspace {
            id: "repository-tooling",
            lockfile: std::path::Path::new("Cargo.lock"),
            packages: &packages,
            profiles: &profiles,
        }];
        let findings = super::validate(
            &fixture.root,
            &registry,
            &contexts,
            64,
            1_048_576,
            &fixture.inventory,
        );
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn metadata_use_target_and_missing_use_mutants_fail() {
        let fixture = Fixture::new();
        let mut value = one_admission_registry(
            &fixture.record_digest,
            &fixture.lock_digest,
        );
        value["workspaces"][0]["admissions"][0]["resolved_uses"][0]["target"] =
            Value::Null;
        let registry = super::Registry::parse(&value).unwrap();
        let metadata = metadata();
        let packages = [super::WorkspacePackage {
            id: "tool".to_owned(),
            cargo_name: "tool".to_owned(),
        }];
        let profiles = [super::ProfileMetadata {
            id: "hosted-default",
            metadata: &metadata,
        }];
        let contexts = [super::ValidationWorkspace {
            id: "repository-tooling",
            lockfile: std::path::Path::new("Cargo.lock"),
            packages: &packages,
            profiles: &profiles,
        }];
        let findings = super::validate(
            &fixture.root,
            &registry,
            &contexts,
            64,
            1_048_576,
            &fixture.inventory,
        );
        assert!(
            findings.iter().any(|finding| finding.code == "REVIEW_USE_DRIFT")
        );

        let mut value = one_admission_registry(
            &fixture.record_digest,
            &fixture.lock_digest,
        );
        value["workspaces"][0]["admissions"][0]["resolved_uses"] = json!([]);
        let registry = super::Registry::parse(&value).unwrap();
        let findings = super::validate(
            &fixture.root,
            &registry,
            &contexts,
            64,
            1_048_576,
            &fixture.inventory,
        );
        assert!(findings.iter().any(
            |finding| finding.code == "REVIEW_RESOLVED_PACKAGE_WITHOUT_USE"
        ));
    }

    fn metadata() -> Metadata {
        let workspace_id = "path+file:///fixture/tool#0.1.0".to_owned();
        let external_id = "registry+exact#external@1.0.0".to_owned();
        let workspace = Package {
            id: workspace_id.clone(),
            name: "tool".to_owned(),
            version: "0.1.0".to_owned(),
            edition: "2024".to_owned(),
            source: None,
            checksum: None,
            license: Some("AGPL-3.0-only".to_owned()),
            manifest_path: PathBuf::from("/fixture/tool/Cargo.toml"),
            targets: Vec::new(),
        };
        let external = Package {
            id: external_id.clone(),
            name: "external".to_owned(),
            version: "1.0.0".to_owned(),
            edition: "2024".to_owned(),
            source: Some("registry+exact".to_owned()),
            checksum: Some(fixture_checksum()),
            license: Some("MIT".to_owned()),
            manifest_path: PathBuf::from("/cargo/external/Cargo.toml"),
            targets: Vec::new(),
        };
        Metadata {
            workspace_root: PathBuf::from("/fixture"),
            workspace_members: BTreeSet::from([workspace_id.clone()]),
            packages: BTreeMap::from([
                (workspace_id.clone(), workspace),
                (external_id.clone(), external),
            ]),
            nodes: BTreeMap::from([
                (
                    workspace_id.clone(),
                    Node {
                        id: workspace_id,
                        dependencies: vec![ResolvedDependency {
                            name: "external_alias".to_owned(),
                            package: external_id.clone(),
                            kinds: vec![DependencyKind {
                                kind: "normal".to_owned(),
                                target: Some(
                                    "cfg(target_os = \"linux\")".to_owned(),
                                ),
                            }],
                        }],
                    },
                ),
                (
                    external_id.clone(),
                    Node { id: external_id, dependencies: Vec::new() },
                ),
            ]),
        }
    }

    struct Fixture {
        root: PathBuf,
        lock_digest: String,
        record_digest: String,
        inventory: Vec<ClassifiedFile>,
    }

    impl Fixture {
        fn new() -> Self {
            let serial = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "reovim-review-test-{}-{serial}",
                std::process::id()
            ));
            fs::create_dir_all(root.join("conformance/dependency-reviews"))
                .unwrap();
            let checksum = fixture_checksum();
            let lock = format!(
                r#"version = 4
[[package]]
name = "external"
version = "1.0.0"
source = "registry+exact"
checksum = "{checksum}"
[[package]]
name = "tool"
version = "0.1.0"
"#
            );
            let record = b"# External review\n";
            fs::write(root.join("Cargo.lock"), lock.as_bytes()).unwrap();
            fs::write(
                root.join("conformance/dependency-reviews/external.md"),
                record,
            )
            .unwrap();
            let lock_digest = crate::digest::bytes(lock.as_bytes());
            let record_digest = crate::digest::bytes(record);
            let classification = SourceClassification::new(
                SourceOwner::RepositoryTooling,
                SourceProvenance::Authored,
                SourceUse::Tool,
            )
            .unwrap();
            let inventory = vec![
                ClassifiedFile {
                    path: PathBuf::from("Cargo.lock"),
                    content_digest: lock_digest.clone(),
                    classification,
                    upstream: None,
                    rule_prefix: PathBuf::from("Cargo.lock"),
                    rule_precedence: 1,
                },
                ClassifiedFile {
                    path: PathBuf::from(
                        "conformance/dependency-reviews/external.md",
                    ),
                    content_digest: record_digest.clone(),
                    classification,
                    upstream: None,
                    rule_prefix: PathBuf::from(
                        "conformance/dependency-reviews",
                    ),
                    rule_precedence: 1,
                },
            ];
            Self { root, lock_digest, record_digest, inventory }
        }
    }

    fn fixture_checksum() -> String {
        crate::digest::bytes(b"external package fixture")
            .trim_start_matches("sha256:")
            .to_owned()
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
}
