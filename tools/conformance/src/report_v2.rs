//! Deterministic `reovim-source-role-map/v2` envelope and standalone verifier.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    path::{Path, PathBuf},
};

use reovim_depgraph::{
    ArchitecturalRole, SourceOwner, SourceProvenance, SourceUse,
};
use serde_json::Value;

use crate::{
    compiler_capture, digest, json, package_capture, prepared_root, schema,
    source,
};

/// Exact schema identity accepted by this module.
pub const SCHEMA: &str = "reovim-source-role-map/v2";

const PREPARED_COMPILER_SUPPORT: &str = "/compiler/lib";
const PREPARED_TARGET_LIBDIR: &str =
    concat!("/sysroot/lib/rustlib/", "x86_64-unknown-linux-gnu/lib");

/// A canonical v2 report ready for durable publication.
#[derive(Debug, Clone)]
pub struct BuiltReport {
    pub value: Value,
    pub bytes: Vec<u8>,
    pub identity: String,
}

/// The identity and disposition recovered by standalone verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedReport {
    pub identity: String,
    pub mode: String,
    pub status: String,
}

/// A stable report-envelope or semantic-verification failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportError {
    code: ReportErrorCode,
    message: String,
}

/// Stable closed categories for v2 report failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportErrorCode {
    Envelope,
    Schema,
    Canonical,
    Identity,
    Limit,
    Reference,
    Semantic,
}

impl ReportErrorCode {
    /// Stable lowercase category name for machine-facing diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Envelope => "envelope",
            Self::Schema => "schema",
            Self::Canonical => "canonical",
            Self::Identity => "identity",
            Self::Limit => "limit",
            Self::Reference => "reference",
            Self::Semantic => "semantic",
        }
    }
}

impl fmt::Display for ReportErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl ReportError {
    /// Creates a typed report failure without changing its display message.
    pub fn new(code: ReportErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    /// Returns the stable machine-facing failure category.
    #[must_use]
    pub const fn code(&self) -> ReportErrorCode {
        self.code
    }

    /// Returns the unchanged human-facing failure message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    fn reclassify_semantic(mut self, code: ReportErrorCode) -> Self {
        if self.code() == ReportErrorCode::Semantic {
            self.code = code;
        }
        self
    }
}

impl fmt::Display for ReportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for ReportError {}

/// Canonicalizes, identifies, validates, and serializes one fully populated v2
/// report using the committed embedded v2 schema.
pub fn build(
    value: Value,
    report_byte_limit: usize,
) -> Result<BuiltReport, ReportError> {
    build_with_schema(
        value,
        schema::embedded_report_schema_bytes(),
        report_byte_limit,
    )
}

/// Builds with an explicitly supplied v2 schema preimage. This is intended for
/// exact-schema tests and callers that already hold the committed schema bytes.
pub fn build_with_schema(
    mut value: Value,
    schema_bytes: &[u8],
    report_byte_limit: usize,
) -> Result<BuiltReport, ReportError> {
    let schema_value = parse_schema(schema_bytes)?;
    require_v2_schema(&schema_value)?;
    require_replaceable_identity(&value)?;
    let provisional_identity = report_identity_preimage(&value)?;
    value
        .as_object_mut()
        .ok_or_else(|| envelope_error("report root is not an object"))?
        .insert(
            "report_identity".to_owned(),
            Value::String(provisional_identity),
        );

    schema::validate_instance(&value, &schema_value)
        .map_err(|failure| schema_error(&failure))?;
    schema::canonicalize_unique_arrays(&mut value, &schema_value)
        .map_err(|failure| canonical_error(failure.to_string()))?;
    schema::validate_instance(&value, &schema_value)
        .map_err(|failure| schema_error(&failure))?;
    validate_schema_preimage(&value, schema_bytes)?;
    validate_semantics(&value)?;

    let identity = report_identity_preimage(&value)?;
    value
        .as_object_mut()
        .expect("schema-validated report is an object")
        .insert("report_identity".to_owned(), Value::String(identity.clone()));
    schema::validate_instance(&value, &schema_value)
        .map_err(|failure| schema_error(&failure))?;

    let mut bytes = serde_json::to_vec(&value).map_err(|failure| {
        envelope_error(format!("cannot serialize final report: {failure}"))
    })?;
    bytes.push(b'\n');
    enforce_report_limits(&value, bytes.len(), report_byte_limit)?;
    Ok(BuiltReport { value, bytes, identity })
}

/// Verifies one bounded serialized v2 report using the committed embedded
/// schema and caller-owned pre-parse JSON limits.
pub fn verify(
    bytes: &[u8],
    report_byte_limit: usize,
    json_depth_limit: usize,
    json_container_item_limit: usize,
) -> Result<VerifiedReport, ReportError> {
    verify_with_schema(
        bytes,
        schema::embedded_report_schema_bytes(),
        report_byte_limit,
        json_depth_limit,
        json_container_item_limit,
    )
}

/// Standalone verification with an explicitly supplied v2 schema preimage.
pub fn verify_with_schema(
    bytes: &[u8],
    schema_bytes: &[u8],
    report_byte_limit: usize,
    json_depth_limit: usize,
    json_container_item_limit: usize,
) -> Result<VerifiedReport, ReportError> {
    if bytes.len() > report_byte_limit {
        return Err(limit_error(
            "report exceeds the caller-owned report byte limit",
        ));
    }
    if bytes.last() != Some(&b'\n') {
        return Err(canonical_error(
            "report must end with exactly one canonical LF",
        ));
    }
    let schema_value = parse_schema(schema_bytes)?;
    require_v2_schema(&schema_value)?;
    let value = json::parse(bytes, json_depth_limit, json_container_item_limit)
        .map_err(|failure| envelope_error(failure.to_string()))?;
    schema::validate_instance(&value, &schema_value)
        .map_err(|failure| schema_error(&failure))?;

    let mut canonical_value = value.clone();
    schema::canonicalize_unique_arrays(&mut canonical_value, &schema_value)
        .map_err(|failure| canonical_error(failure.to_string()))?;
    let mut canonical_bytes =
        serde_json::to_vec(&canonical_value).map_err(|failure| {
            canonical_error(format!(
                "cannot serialize canonical report: {failure}"
            ))
        })?;
    canonical_bytes.push(b'\n');
    if canonical_bytes != bytes {
        return Err(canonical_error(concat!(
            "report is not canonical JSON or a schema-set array is not ",
            "in canonical order"
        )));
    }

    enforce_report_limits(&value, bytes.len(), report_byte_limit)?;
    validate_schema_preimage(&value, schema_bytes)?;
    validate_semantics(&value)?;
    let identity = value
        .get("report_identity")
        .and_then(Value::as_str)
        .ok_or_else(|| identity_error("report_identity is absent or invalid"))?
        .to_owned();
    let mut identity_preimage = value.clone();
    identity_preimage
        .as_object_mut()
        .expect("schema-validated report is an object")
        .remove("report_identity");
    let canonical =
        serde_json::to_vec(&identity_preimage).map_err(|failure| {
            identity_error(format!(
                "cannot serialize report identity preimage: {failure}"
            ))
        })?;
    let computed = digest::bytes(&canonical);
    if computed != identity {
        return Err(identity_error(format!(
            "report identity mismatch: expected {identity}, computed {computed}"
        )));
    }

    let mode = required_string(&value, "/mode")?.to_owned();
    let status = required_string(&value, "/result/status")?.to_owned();
    Ok(VerifiedReport { identity, mode, status })
}

fn parse_schema(bytes: &[u8]) -> Result<Value, ReportError> {
    serde_json::from_slice(bytes).map_err(|failure| {
        schema_report_error(format!(
            "v2 report schema is invalid JSON: {failure}"
        ))
    })
}

fn require_v2_schema(schema_value: &Value) -> Result<(), ReportError> {
    if schema_value.pointer("/properties/schema/const").and_then(Value::as_str)
        != Some(SCHEMA)
    {
        return Err(schema_report_error(
            "supplied report schema is not reovim-source-role-map/v2",
        ));
    }
    schema::validate_schema(schema_value)
        .map_err(|failure| schema_error(&failure))
}

fn require_replaceable_identity(value: &Value) -> Result<(), ReportError> {
    match value.get("report_identity") {
        Some(Value::Null) => Ok(()),
        Some(Value::String(identity)) if valid_digest(identity) => Ok(()),
        Some(_) => Err(identity_error(concat!(
            "report_identity input must be null or a sha256 digest that ",
            "the builder can replace"
        ))),
        None => {
            Err(identity_error("fully populated report omits report_identity"))
        }
    }
}

fn report_identity_preimage(value: &Value) -> Result<String, ReportError> {
    let mut preimage = value.clone();
    preimage
        .as_object_mut()
        .ok_or_else(|| envelope_error("report root is not an object"))?
        .remove("report_identity");
    let bytes = serde_json::to_vec(&preimage).map_err(|failure| {
        identity_error(format!(
            "cannot serialize report identity preimage: {failure}"
        ))
    })?;
    Ok(digest::bytes(&bytes))
}

fn validate_schema_preimage(
    value: &Value,
    schema_bytes: &[u8],
) -> Result<(), ReportError> {
    let expected = digest::bytes(schema_bytes);
    let actual = required_string(
        value,
        "/identity_inputs/report_schema/content_digest",
    )?;
    if actual != expected {
        return Err(identity_error(format!(
            "identity_inputs.report_schema.content_digest differs from \
             supplied schema: expected {expected}"
        )));
    }
    Ok(())
}

fn enforce_report_limits(
    value: &Value,
    byte_count: usize,
    caller_limit: usize,
) -> Result<(), ReportError> {
    let declared = value
        .pointer("/limits/configuration/report_bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            limit_error(
                "limits.configuration.report_bytes is absent or invalid",
            )
        })?;
    let byte_count = u64::try_from(byte_count).map_err(|failure| {
        limit_error(format!("report byte count conversion failed: {failure}"))
    })?;
    let caller_limit = u64::try_from(caller_limit).map_err(|failure| {
        limit_error(format!("report limit conversion failed: {failure}"))
    })?;
    if byte_count > caller_limit || byte_count > declared {
        return Err(limit_error(format!(
            "report is {byte_count} bytes; caller limit is {caller_limit} \
             and declared limit is {declared}"
        )));
    }
    Ok(())
}

fn validate_semantics(value: &Value) -> Result<(), ReportError> {
    if required_string(value, "/schema")? != SCHEMA {
        return Err(schema_report_error(
            "report schema identity is not reovim-source-role-map/v2",
        ));
    }
    if required_string(value, "/subject/kind")? != "active_repository" {
        return Err(error(
            "v2 Source Role Map subject is not active_repository",
        ));
    }
    validate_mode(value)?;
    validate_result(value)?;
    validate_graph_and_product_references(value).map_err(|failure| {
        failure.reclassify_semantic(ReportErrorCode::Reference)
    })
}

fn validate_mode(value: &Value) -> Result<(), ReportError> {
    let mode = required_string(value, "/mode")?;
    let revision = value.pointer("/revision").unwrap_or(&Value::Null);
    let git = value.pointer("/identity_inputs/git").unwrap_or(&Value::Null);
    let tree = value
        .pointer("/identity_inputs/git_tree_digest")
        .unwrap_or(&Value::Null);
    match mode {
        "candidate"
            if revision.is_null() && git.is_null() && tree.is_null() =>
        {
            Ok(())
        }
        "exact"
            if revision.as_str().is_some_and(valid_revision)
                && git.is_object()
                && tree.as_str().is_some_and(valid_digest) =>
        {
            Ok(())
        }
        "candidate" | "exact" => {
            Err(identity_error("mode/revision/Git identities are inconsistent"))
        }
        _ => Err(identity_error("report mode is outside the v2 closed set")),
    }
}

fn validate_result(value: &Value) -> Result<(), ReportError> {
    let findings = required_array(value, "/findings")?;
    let finding_count = value
        .pointer("/result/finding_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("result.finding_count is absent or invalid"))?;
    if finding_count
        != u64::try_from(findings.len())
            .map_err(|failure| error(failure.to_string()))?
    {
        return Err(error("result.finding_count does not match findings"));
    }
    let status = required_string(value, "/result/status")?;
    if (findings.is_empty() && status != "pass")
        || (!findings.is_empty() && status != "fail")
    {
        return Err(error("result.status disagrees with the finding set"));
    }
    Ok(())
}

#[derive(Debug)]
struct SourceAuthority {
    files: BTreeMap<PathBuf, SourceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceRef {
    path: PathBuf,
    content_digest: String,
    size_bytes: u64,
    owner: SourceOwner,
    provenance: SourceProvenance,
    source_use: SourceUse,
    upstream: Option<UpstreamRef>,
    rule_index: u64,
    rule_prefix: PathBuf,
    rule_precedence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UpstreamRef {
    identity: String,
    digest: String,
    subject: UpstreamSubjectRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum UpstreamSubjectRef {
    Paths(Vec<PathBuf>),
    ClassifiedContent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuleProjection {
    prefix: PathBuf,
    precedence: u64,
    owner: SourceOwner,
    provenance: SourceProvenance,
    source_use: SourceUse,
    upstream: Option<UpstreamRef>,
}

#[derive(Debug, Clone, Copy)]
struct SourceLimits {
    source_entry_count: u64,
    source_file_count: u64,
    source_total_bytes: u64,
    source_file_bytes: u64,
    path_bytes: u64,
}

impl SourceLimits {
    fn from_report(value: &Value) -> Result<Self, ReportError> {
        let prefix = "/limits/inventory";
        Ok(Self {
            source_entry_count: required_count(
                value,
                prefix,
                "source_entry_count",
            )?,
            source_file_count: required_count(
                value,
                prefix,
                "source_file_count",
            )?,
            source_total_bytes: required_count(
                value,
                prefix,
                "source_total_bytes",
            )?,
            source_file_bytes: required_count(
                value,
                prefix,
                "source_file_bytes",
            )?,
            path_bytes: required_count(value, prefix, "path_bytes")?,
        })
    }
}

fn validate_source_authority(
    value: &Value,
) -> Result<SourceAuthority, ReportError> {
    validate_source_exclusions(value)?;
    let limits = SourceLimits::from_report(value)?;
    let classifications =
        required_array(value, "/source_inventory/classifications")?;
    enforce_source_count(classifications, limits)?;
    let authority = collect_source_authority(classifications, limits)?;
    validate_source_upstreams(&authority)?;
    validate_source_tree_digest(value, &authority)?;
    Ok(authority)
}

fn validate_source_exclusions(value: &Value) -> Result<(), ReportError> {
    let mut retained = BTreeMap::new();
    for exclusion in required_array(value, "/source_inventory/exclusions")? {
        let context = "source_inventory.exclusions[]";
        let prefix = field_string(exclusion, "prefix", context)?.to_owned();
        let kind = field_string(exclusion, "kind", context)?.to_owned();
        if retained.insert(prefix, kind).is_some() {
            return Err(reference_error("duplicate source exclusion prefix"));
        }
    }
    let expected = BTreeMap::from([
        (".git".to_owned(), "vcs_metadata".to_owned()),
        ("target".to_owned(), "transient_build_output".to_owned()),
        ("tmp".to_owned(), "working_notes".to_owned()),
    ]);
    if retained != expected {
        return Err(reference_error(
            "source exclusions differ from the current closed profile",
        ));
    }
    Ok(())
}

fn enforce_source_count(
    classifications: &[Value],
    limits: SourceLimits,
) -> Result<(), ReportError> {
    let count = u64::try_from(classifications.len())
        .map_err(|_| limit_error("source file count does not fit u64"))?;
    if count > limits.source_file_count || count > limits.source_entry_count {
        return Err(limit_error(
            "source file count exceeds the retained inventory limits",
        ));
    }
    Ok(())
}

fn collect_source_authority(
    classifications: &[Value],
    limits: SourceLimits,
) -> Result<SourceAuthority, ReportError> {
    let mut files = BTreeMap::new();
    let mut rules = BTreeMap::new();
    let mut identities = BTreeMap::new();
    let mut total_bytes = 0_u64;
    for classification in classifications {
        let source = parse_source_ref(classification, limits)?;
        total_bytes = charge_source_bytes(total_bytes, &source, limits)?;
        validate_rule_identity(&source, &mut rules, &mut identities)?;
        let path = source.path.clone();
        if files.insert(path.clone(), source).is_some() {
            return Err(reference_error(format!(
                "source inventory repeats path {}",
                path.display()
            )));
        }
    }
    Ok(SourceAuthority { files })
}

fn parse_source_ref(
    value: &Value,
    limits: SourceLimits,
) -> Result<SourceRef, ReportError> {
    let context = "source classification";
    let path_text = field_string(value, "path", context)?;
    enforce_path_byte_limit(path_text, limits.path_bytes, context)?;
    let path = parse_relative_path(path_text, context)?;
    let size_bytes = field_count(value, "size_bytes", context)?;
    if size_bytes > limits.source_file_bytes {
        return Err(limit_error(format!(
            "source {} exceeds the per-file byte limit",
            path.display()
        )));
    }
    let owner = parse_source_owner(value)?;
    let provenance = parse_source_provenance(value)?;
    let source_use = parse_source_use(value)?;
    let upstream =
        parse_upstream_ref(value.get("upstream"), context, limits.path_bytes)?;
    let winning = object_field(value, "winning_rule", context)?;
    let prefix_text = field_string(winning, "prefix", "winning rule")?;
    enforce_path_byte_limit(
        prefix_text,
        limits.path_bytes,
        "winning rule prefix",
    )?;
    let rule_prefix = parse_relative_path(prefix_text, "winning rule prefix")?;
    if !path.starts_with(&rule_prefix) {
        return Err(reference_error(format!(
            "winning rule {} is not an ancestor of {}",
            rule_prefix.display(),
            path.display()
        )));
    }
    Ok(SourceRef {
        path,
        content_digest: field_string(value, "content_digest", context)?
            .to_owned(),
        size_bytes,
        owner,
        provenance,
        source_use,
        upstream,
        rule_index: field_count(winning, "index", "winning rule")?,
        rule_prefix,
        rule_precedence: field_count(winning, "precedence", "winning rule")?,
    })
}

fn parse_source_owner(value: &Value) -> Result<SourceOwner, ReportError> {
    let owner = object_field(value, "source_owner", "source classification")?;
    parse_owner_value(owner, "source owner")
}

fn parse_owner_value(
    owner: &Value,
    context: &str,
) -> Result<SourceOwner, ReportError> {
    match field_string(owner, "kind", context)? {
        "repository_tooling" => Ok(SourceOwner::RepositoryTooling),
        "product" => parse_product_owner(owner),
        _ => Err(reference_error("source owner kind is invalid")),
    }
}

fn parse_product_owner(owner: &Value) -> Result<SourceOwner, ReportError> {
    let role = field_string(owner, "role", "Product source owner")?;
    ArchitecturalRole::ALL
        .into_iter()
        .find(|candidate| candidate.as_str() == role)
        .map(SourceOwner::Product)
        .ok_or_else(|| reference_error("Product source role is invalid"))
}

fn parse_source_provenance(
    value: &Value,
) -> Result<SourceProvenance, ReportError> {
    match field_string(value, "source_provenance", "source classification")? {
        "authored" => Ok(SourceProvenance::Authored),
        "generated" => Ok(SourceProvenance::Generated),
        "vendored" => Ok(SourceProvenance::Vendored),
        _ => Err(reference_error("source provenance is invalid")),
    }
}

fn parse_source_use(value: &Value) -> Result<SourceUse, ReportError> {
    match field_string(value, "source_use", "source classification")? {
        "production" => Ok(SourceUse::Production),
        "test" => Ok(SourceUse::Test),
        "example" => Ok(SourceUse::Example),
        "tool" => Ok(SourceUse::Tool),
        "build_output" => Ok(SourceUse::BuildOutput),
        _ => Err(reference_error("source use is invalid")),
    }
}

fn parse_upstream_ref(
    value: Option<&Value>,
    context: &str,
    path_limit: u64,
) -> Result<Option<UpstreamRef>, ReportError> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let subject = object_field(value, "digest_subject", context)?;
    Ok(Some(UpstreamRef {
        identity: field_string(value, "identity", context)?.to_owned(),
        digest: field_string(value, "digest", context)?.to_owned(),
        subject: parse_upstream_subject(subject, path_limit)?,
    }))
}

fn parse_upstream_subject(
    value: &Value,
    path_limit: u64,
) -> Result<UpstreamSubjectRef, ReportError> {
    match field_string(value, "kind", "upstream digest subject")? {
        "paths" => parse_upstream_paths(value, path_limit)
            .map(UpstreamSubjectRef::Paths),
        "classified_content" => Ok(UpstreamSubjectRef::ClassifiedContent),
        _ => Err(reference_error("upstream digest-subject kind is invalid")),
    }
}

fn parse_upstream_paths(
    value: &Value,
    path_limit: u64,
) -> Result<Vec<PathBuf>, ReportError> {
    required_field_array(value, "paths", "digest subject")?
        .iter()
        .map(|path| parse_upstream_path(path, path_limit))
        .collect()
}

fn parse_upstream_path(
    value: &Value,
    path_limit: u64,
) -> Result<PathBuf, ReportError> {
    let text = value
        .as_str()
        .ok_or_else(|| reference_error("upstream subject path is not text"))?;
    enforce_path_byte_limit(text, path_limit, "upstream subject path")?;
    parse_relative_path(text, "upstream subject path")
}

fn charge_source_bytes(
    total: u64,
    source: &SourceRef,
    limits: SourceLimits,
) -> Result<u64, ReportError> {
    let updated = total
        .checked_add(source.size_bytes)
        .ok_or_else(|| limit_error("source total byte count overflow"))?;
    if updated > limits.source_total_bytes {
        return Err(limit_error(
            "source total bytes exceed the retained limit",
        ));
    }
    Ok(updated)
}

fn validate_rule_identity(
    source: &SourceRef,
    rules: &mut BTreeMap<u64, RuleProjection>,
    identities: &mut BTreeMap<(PathBuf, u64), u64>,
) -> Result<(), ReportError> {
    let projection = RuleProjection {
        prefix: source.rule_prefix.clone(),
        precedence: source.rule_precedence,
        owner: source.owner,
        provenance: source.provenance,
        source_use: source.source_use,
        upstream: source.upstream.clone(),
    };
    if let Some(existing) = rules.get(&source.rule_index)
        && existing != &projection
    {
        return Err(reference_error(format!(
            "winning rule index {} has inconsistent projections",
            source.rule_index
        )));
    }
    rules.insert(source.rule_index, projection);
    let key = (source.rule_prefix.clone(), source.rule_precedence);
    if let Some(existing) = identities.insert(key, source.rule_index)
        && existing != source.rule_index
    {
        return Err(reference_error(
            "winning rule prefix/precedence has multiple indices",
        ));
    }
    Ok(())
}

fn validate_source_upstreams(
    authority: &SourceAuthority,
) -> Result<(), ReportError> {
    for source in authority.files.values() {
        validate_source_upstream(source, authority)?;
    }
    Ok(())
}

fn validate_source_upstream(
    source: &SourceRef,
    authority: &SourceAuthority,
) -> Result<(), ReportError> {
    match (source.provenance, source.upstream.as_ref()) {
        (SourceProvenance::Authored, None) => Ok(()),
        (SourceProvenance::Generated, Some(upstream)) => {
            validate_generated_upstream(upstream, authority)
        }
        (SourceProvenance::Vendored, Some(upstream))
            if upstream.subject == UpstreamSubjectRef::ClassifiedContent =>
        {
            Ok(())
        }
        _ => Err(reference_error(format!(
            "source {} has an invalid provenance/upstream subject",
            source.path.display()
        ))),
    }
}

fn validate_generated_upstream(
    upstream: &UpstreamRef,
    authority: &SourceAuthority,
) -> Result<(), ReportError> {
    let UpstreamSubjectRef::Paths(paths) = &upstream.subject else {
        return Err(reference_error(
            "generated source requires an explicit path digest subject",
        ));
    };
    for path in paths {
        validate_upstream_subject_path(path, authority)?;
    }
    Ok(())
}

fn validate_upstream_subject_path(
    path: &Path,
    authority: &SourceAuthority,
) -> Result<(), ReportError> {
    let source = authority.files.get(path).ok_or_else(|| {
        reference_error(format!(
            "upstream subject {} is not classified",
            path.display()
        ))
    })?;
    if source.provenance == SourceProvenance::Generated
        || source.source_use == SourceUse::BuildOutput
    {
        return Err(reference_error(format!(
            "upstream subject {} is generated or BuildOutput",
            path.display()
        )));
    }
    Ok(())
}

fn validate_source_tree_digest(
    value: &Value,
    authority: &SourceAuthority,
) -> Result<(), ReportError> {
    let actual = computed_source_tree_digest(authority);
    let expected =
        required_string(value, "/identity_inputs/source_tree_digest")?;
    if actual != expected {
        return Err(identity_error(format!(
            "source-tree digest mismatch: expected {expected}, \
             computed {actual}"
        )));
    }
    Ok(())
}

fn computed_source_tree_digest(authority: &SourceAuthority) -> String {
    let entries =
        authority.files.values().map(|entry| source::SourceTreeDigestEntry {
            path: &entry.path,
            content_digest: &entry.content_digest,
            owner: entry.owner,
            provenance: entry.provenance,
            source_use: entry.source_use,
        });
    source::canonical_source_tree_digest(entries)
}

fn parse_relative_path(
    value: &str,
    context: &str,
) -> Result<PathBuf, ReportError> {
    crate::policy::relative_path(value, context, false)
        .map_err(|failure| reference_error(failure.to_string()))
}

fn validate_graph_and_product_references(
    value: &Value,
) -> Result<(), ReportError> {
    let source = validate_source_authority(value)?;
    let active_product =
        !required_array(value, "/product_proof/closures")?.is_empty();
    let graph = graph_references(value, &source, active_product)?;
    validate_identity_input_authority(value, &graph, &source)?;
    validate_product_references(value, &graph, &source)
}

struct GraphReferences {
    workspaces: BTreeSet<String>,
    product_workspaces: BTreeSet<String>,
    workspace_roots: BTreeMap<String, PathBuf>,
    workspace_lockfiles: BTreeMap<String, String>,
    workspace_manifests: BTreeMap<String, PathBuf>,
    workspace_members: BTreeMap<String, BTreeSet<String>>,
    profiles: BTreeSet<(String, String)>,
    profile_details: BTreeMap<(String, String), MetadataProfileRef>,
    repository_root: PathBuf,
    packages: BTreeMap<(String, String), GraphPackageRef>,
    external_packages: ExternalPackageReferences,
    edges: EdgeGraph,
    partitions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GraphPackageRef {
    name: String,
    version: String,
    manifest: String,
    owners: BTreeSet<SourceOwner>,
    targets: BTreeMap<String, GraphTargetRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GraphTargetRef {
    name: String,
    src_path: String,
    kind: Vec<String>,
    crate_types: Vec<String>,
}

struct CollectedWorkspaces {
    all: BTreeSet<String>,
    product: BTreeSet<String>,
    roots: BTreeMap<String, PathBuf>,
    lockfiles: BTreeMap<String, String>,
    manifests: BTreeMap<String, PathBuf>,
    members: BTreeMap<String, BTreeSet<String>>,
    partitions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MetadataProfileRef {
    target: String,
    all_features: bool,
    no_default_features: bool,
    features: Vec<String>,
    metadata_digest: String,
}

fn graph_references(
    value: &Value,
    source: &SourceAuthority,
    active_product: bool,
) -> Result<GraphReferences, ReportError> {
    let workspaces = collect_workspaces(value, active_product)?;
    let profiles = collect_profiles(value, &workspaces.all)?;
    let repository_root =
        validate_metadata_runs(value, &workspaces, &profiles)?;
    let profile_ids = profiles.keys().cloned().collect::<BTreeSet<_>>();
    let packages = collect_packages(value, &profile_ids, &workspaces, source)?;
    let external_packages = collect_external_packages(value, &profile_ids)?;
    let edges = validate_cargo_edges(
        value,
        &profile_ids,
        &packages.ids,
        &external_packages,
    )?;
    validate_reachability(value, &packages, &external_packages, &edges)?;
    Ok(GraphReferences {
        workspaces: workspaces.all,
        product_workspaces: workspaces.product,
        workspace_roots: workspaces.roots,
        workspace_lockfiles: workspaces.lockfiles,
        workspace_manifests: workspaces.manifests,
        workspace_members: workspaces.members,
        profiles: profile_ids,
        profile_details: profiles,
        repository_root,
        packages: packages.details,
        external_packages,
        edges,
        partitions: workspaces.partitions,
    })
}

struct ArtifactContract {
    field: &'static str,
    path: &'static str,
    schema: &'static str,
    schema_path: &'static str,
}

fn validate_identity_input_authority(
    value: &Value,
    graph: &GraphReferences,
    source: &SourceAuthority,
) -> Result<(), ReportError> {
    validate_artifact_identities(value, source)?;
    validate_lock_identities(value, graph, source)?;
    let path_limit = required_count(value, "/limits/inventory", "path_bytes")?;
    validate_tool_identities(value, path_limit)
}

fn validate_artifact_identities(
    value: &Value,
    source: &SourceAuthority,
) -> Result<(), ReportError> {
    let contracts = [
        ArtifactContract {
            field: "source_classification",
            path: crate::POLICY_KEY,
            schema: crate::policy::SCHEMA,
            schema_path: crate::POLICY_SCHEMA_KEY,
        },
        ArtifactContract {
            field: "workspace_topology",
            path: crate::TOPOLOGY_KEY,
            schema: crate::topology::SCHEMA,
            schema_path: crate::TOPOLOGY_SCHEMA_KEY,
        },
        ArtifactContract {
            field: "entrypoint_reviews",
            path: crate::ENTRYPOINT_REVIEWS_KEY,
            schema: crate::entrypoint_reviews::SCHEMA,
            schema_path: crate::ENTRYPOINT_REVIEWS_SCHEMA_KEY,
        },
        ArtifactContract {
            field: "dependency_reviews",
            path: crate::DEPENDENCY_REVIEWS_KEY,
            schema: crate::reviews::SCHEMA,
            schema_path: crate::DEPENDENCY_REVIEWS_SCHEMA_KEY,
        },
        ArtifactContract {
            field: "product_closures",
            path: crate::PRODUCT_CLOSURES_KEY,
            schema: crate::product_closures::SCHEMA,
            schema_path: crate::PRODUCT_CLOSURES_SCHEMA_KEY,
        },
        ArtifactContract {
            field: "execution_tool_reviews",
            path: crate::EXECUTION_TOOL_REVIEWS_KEY,
            schema: crate::execution_tools::SCHEMA,
            schema_path: crate::EXECUTION_TOOL_REVIEWS_SCHEMA_KEY,
        },
    ];
    let identity_inputs = value
        .get("identity_inputs")
        .ok_or_else(|| identity_error("identity_inputs is absent"))?;
    for contract in contracts {
        validate_artifact_identity(identity_inputs, &contract, source)?;
    }
    let report = identity_inputs
        .get("report_schema")
        .ok_or_else(|| identity_error("report schema identity is absent"))?;
    if field_string(report, "path", "report schema")?
        != crate::REPORT_SCHEMA_KEY
        || field_string(report, "schema", "report schema")? != SCHEMA
    {
        return Err(identity_error(
            "report schema retained tuple differs from repository authority",
        ));
    }
    require_source_digest(
        source,
        crate::REPORT_SCHEMA_KEY,
        field_string(report, "content_digest", "report schema")?,
        "report schema",
    )?;
    Ok(())
}

fn validate_artifact_identity(
    identity_inputs: &Value,
    contract: &ArtifactContract,
    source: &SourceAuthority,
) -> Result<(), ReportError> {
    let identity = identity_inputs.get(contract.field).ok_or_else(|| {
        identity_error(format!("{} identity is absent", contract.field))
    })?;
    let context = contract.field;
    if field_string(identity, "path", context)? != contract.path
        || field_string(identity, "schema", context)? != contract.schema
        || field_string(identity, "schema_path", context)?
            != contract.schema_path
    {
        return Err(identity_error(format!(
            "{} retained tuple differs from repository authority",
            contract.field
        )));
    }
    require_source_digest(
        source,
        contract.path,
        field_string(identity, "content_digest", context)?,
        context,
    )?;
    require_source_digest(
        source,
        contract.schema_path,
        field_string(identity, "schema_digest", context)?,
        context,
    )?;
    Ok(())
}

fn require_source_digest(
    source: &SourceAuthority,
    path: &str,
    expected: &str,
    context: &str,
) -> Result<(), ReportError> {
    let path = Path::new(path);
    let classified = source.files.get(path).ok_or_else(|| {
        reference_error(format!(
            "{context} path {} is absent from source inventory",
            path.display()
        ))
    })?;
    if classified.content_digest != expected {
        return Err(reference_error(format!(
            "{context} digest differs from classified {}",
            path.display()
        )));
    }
    Ok(())
}

fn validate_lock_identities(
    value: &Value,
    graph: &GraphReferences,
    source: &SourceAuthority,
) -> Result<(), ReportError> {
    let lockfiles = required_array(value, "/identity_inputs/lockfiles")?;
    let mut workspaces = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for lockfile in lockfiles {
        let workspace =
            field_string(lockfile, "workspace", "lockfile identity")?;
        let path = field_string(lockfile, "path", "lockfile identity")?;
        if !workspaces.insert(workspace) || !paths.insert(path) {
            return Err(reference_error(
                "lockfile identities reuse a workspace or path",
            ));
        }
        let expected =
            graph.workspace_lockfiles.get(workspace).ok_or_else(|| {
                reference_error(format!(
                    "lockfile identity references unknown workspace {workspace}"
                ))
            })?;
        if path != expected {
            return Err(reference_error(format!(
                "workspace {workspace} lockfile identity differs from graph"
            )));
        }
        require_source_digest(
            source,
            path,
            field_string(lockfile, "digest", "lockfile identity")?,
            "lockfile identity",
        )?;
    }
    if workspaces != graph.workspaces.iter().map(String::as_str).collect() {
        return Err(reference_error(
            "lockfile identities do not exactly cover retained workspaces",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ToolKind {
    Checker,
    Cargo,
    Rustc,
    Git,
}

impl ToolKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Checker => "checker",
            Self::Cargo => "cargo",
            Self::Rustc => "rustc",
            Self::Git => "git",
        }
    }

    const fn version_arguments(self) -> &'static [&'static str] {
        match self {
            Self::Checker | Self::Git => &["--version"],
            Self::Cargo | Self::Rustc => &["--version", "--verbose"],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileIdentity {
    content_digest: String,
    size_bytes: u64,
}

fn validate_tool_identities(
    value: &Value,
    path_limit: u64,
) -> Result<(), ReportError> {
    let executable_limit =
        required_count(value, "/limits/process", "tool_executable_bytes")?;
    let mut file_paths = BTreeMap::new();
    for kind in [ToolKind::Checker, ToolKind::Cargo, ToolKind::Rustc] {
        let tool = value
            .pointer(&format!("/identity_inputs/{}", kind.name()))
            .ok_or_else(|| {
                identity_error(format!("{} identity is absent", kind.name()))
            })?;
        validate_tool_identity(
            tool,
            kind,
            executable_limit,
            path_limit,
            &mut file_paths,
        )?;
    }
    validate_git_identity(value, executable_limit, path_limit, &mut file_paths)
}

fn validate_git_identity(
    value: &Value,
    executable_limit: u64,
    path_limit: u64,
    file_paths: &mut BTreeMap<String, FileIdentity>,
) -> Result<(), ReportError> {
    let mode = required_string(value, "/mode")?;
    let git = value.pointer("/identity_inputs/git").unwrap_or(&Value::Null);
    match (mode, git) {
        ("candidate", Value::Null) => Ok(()),
        ("exact", Value::Object(_)) => validate_tool_identity(
            git,
            ToolKind::Git,
            executable_limit,
            path_limit,
            file_paths,
        ),
        _ => Err(identity_error(
            "mode and retained Git tool identity are inconsistent",
        )),
    }
}

fn validate_tool_identity(
    tool: &Value,
    kind: ToolKind,
    executable_limit: u64,
    path_limit: u64,
    file_paths: &mut BTreeMap<String, FileIdentity>,
) -> Result<(), ReportError> {
    let context = kind.name();
    let canonical_path = field_string(tool, "canonical_path", context)?;
    if !normalized_file_path(canonical_path) {
        return Err(identity_error(format!(
            "{context} canonical path is not normalized absolute text"
        )));
    }
    enforce_path_byte_limit(canonical_path, path_limit, context)?;
    let size = field_count(tool, "executable_size_bytes", context)?;
    if size == 0 || size > executable_limit {
        return Err(limit_error(format!(
            "{context} executable size is outside the retained limit"
        )));
    }
    register_file_identity(
        canonical_path,
        field_string(tool, "executable_digest", context)?,
        size,
        file_paths,
    )?;
    validate_tool_version(tool, kind)?;
    validate_runtime_inputs(
        tool,
        kind,
        executable_limit,
        path_limit,
        context,
        file_paths,
    )
}

fn validate_tool_version(
    tool: &Value,
    kind: ToolKind,
) -> Result<(), ReportError> {
    let context = kind.name();
    let arguments = string_array(tool, "version_argv", context)?;
    if arguments
        .iter()
        .map(String::as_str)
        .ne(kind.version_arguments().iter().copied())
    {
        return Err(identity_error(format!(
            "{context} version argv differs from its exact profile"
        )));
    }
    let version = field_string(tool, "version", context)?;
    if version.is_empty()
        || version.contains(['\0', '\r'])
        || version.starts_with('\n')
        || version.ends_with('\n')
    {
        return Err(identity_error(format!(
            "{context} version is not normalized retained text"
        )));
    }
    let exact_output = format!("{version}\n");
    let expected = crate::digest::bytes(exact_output.as_bytes());
    if field_string(tool, "version_digest", context)? != expected {
        return Err(identity_error(format!(
            "{context} version digest differs from exact one-LF text"
        )));
    }
    Ok(())
}

fn validate_runtime_inputs(
    tool: &Value,
    kind: ToolKind,
    file_limit: u64,
    path_limit: u64,
    context: &str,
    file_paths: &mut BTreeMap<String, FileIdentity>,
) -> Result<(), ReportError> {
    let mut host_paths = BTreeSet::new();
    let mut mappings = BTreeSet::new();
    let mut prepared_paths = BTreeSet::new();
    for input in required_field_array(tool, "runtime_inputs", context)? {
        let path = field_string(input, "canonical_path", "runtime input")?;
        if !normalized_file_path(path) {
            return Err(identity_error(format!(
                "{context} runtime mappings are invalid or duplicated"
            )));
        }
        enforce_path_byte_limit(path, path_limit, "runtime input")?;
        if matches!(kind, ToolKind::Git) {
            validate_host_runtime_input(input, path, &mut host_paths, context)?;
        } else {
            validate_prepared_runtime_input(
                input,
                path,
                path_limit,
                &mut mappings,
                &mut prepared_paths,
                context,
            )?;
        }
        let size = field_count(input, "size_bytes", "runtime input")?;
        if size > file_limit {
            return Err(limit_error(format!(
                "{context} runtime input exceeds the per-file limit"
            )));
        }
        register_file_identity(
            path,
            field_string(input, "content_digest", "runtime input")?,
            size,
            file_paths,
        )?;
    }
    Ok(())
}

fn validate_host_runtime_input(
    input: &Value,
    path: &str,
    host_paths: &mut BTreeSet<String>,
    context: &str,
) -> Result<(), ReportError> {
    if input.get("prepared_path").is_some()
        || !host_paths.insert(path.to_owned())
    {
        return Err(identity_error(format!(
            "{context} host runtime paths are invalid or duplicated"
        )));
    }
    Ok(())
}

fn validate_prepared_runtime_input(
    input: &Value,
    path: &str,
    path_limit: u64,
    mappings: &mut BTreeSet<(String, String)>,
    prepared_paths: &mut BTreeSet<String>,
    context: &str,
) -> Result<(), ReportError> {
    let prepared = field_string(input, "prepared_path", "runtime input")?;
    let mapping = (path.to_owned(), prepared.to_owned());
    if !normalized_file_path(prepared)
        || !mappings.insert(mapping)
        || !prepared_paths.insert(prepared.to_owned())
    {
        return Err(identity_error(format!(
            "{context} runtime mappings are invalid or duplicated"
        )));
    }
    enforce_path_byte_limit(prepared, path_limit, "prepared runtime input")
}

fn register_file_identity(
    path: &str,
    content_digest: &str,
    size_bytes: u64,
    identities: &mut BTreeMap<String, FileIdentity>,
) -> Result<(), ReportError> {
    let identity =
        FileIdentity { content_digest: content_digest.to_owned(), size_bytes };
    if let Some(existing) = identities.get(path)
        && existing != &identity
    {
        return Err(identity_error(format!(
            "canonical path {path} has conflicting file identities"
        )));
    }
    identities.insert(path.to_owned(), identity);
    Ok(())
}

fn collect_workspaces(
    value: &Value,
    active_product: bool,
) -> Result<CollectedWorkspaces, ReportError> {
    let mut workspaces = BTreeSet::new();
    let mut roots = BTreeSet::new();
    let mut workspace_roots = BTreeMap::new();
    let mut product_workspaces = BTreeSet::new();
    let mut lockfiles = BTreeMap::new();
    let mut manifests = BTreeMap::new();
    let mut members = BTreeMap::new();
    let mut partitions = BTreeMap::new();
    for workspace in required_array(value, "/cargo_graph/workspaces")? {
        let context = "cargo_graph.workspaces[]";
        let id = field_string(workspace, "id", context)?;
        insert_unique(&mut workspaces, id, "workspace")?;
        let root_text = field_string(workspace, "root", context)?;
        let root = parse_workspace_root(root_text)?;
        if !roots.insert(root.clone()) {
            return Err(reference_error("duplicate workspace root"));
        }
        workspace_roots.insert(id.to_owned(), root.clone());
        let lockfile = parse_relative_path(
            field_string(workspace, "lockfile", context)?,
            "workspace lockfile",
        )?;
        let manifest = parse_relative_path(
            field_string(workspace, "manifest", context)?,
            "workspace manifest",
        )?;
        validate_workspace_file(&root, &manifest, "manifest")?;
        validate_workspace_file(&root, &lockfile, "lockfile")?;
        validate_workspace_members(
            workspace,
            &root,
            partition_is_product(workspace)?,
        )?;
        let workspace_members = string_array(workspace, "members", context)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        members.insert(id.to_owned(), workspace_members);
        lockfiles.insert(
            id.to_owned(),
            field_string(workspace, "lockfile", context)?.to_owned(),
        );
        manifests.insert(id.to_owned(), manifest);
        let partition = field_string(workspace, "partition", context)?;
        partitions.insert(id.to_owned(), partition.to_owned());
        if partition == "product" {
            product_workspaces.insert(id.to_owned());
        }
    }
    if !active_product && roots != BTreeSet::from([PathBuf::new()]) {
        return Err(reference_error(
            "current Product-empty profile requires exactly one '.' root",
        ));
    }
    if active_product {
        validate_active_workspace_set(
            &workspaces,
            &product_workspaces,
            &roots,
        )?;
    }

    for lockfile in required_array(value, "/identity_inputs/lockfiles")? {
        require_member(
            &workspaces,
            field_string(lockfile, "workspace", "identity_inputs.lockfiles[]")?,
            "lockfile workspace",
        )?;
    }
    Ok(CollectedWorkspaces {
        all: workspaces,
        product: product_workspaces,
        roots: workspace_roots,
        lockfiles,
        manifests,
        members,
        partitions,
    })
}

fn partition_is_product(workspace: &Value) -> Result<bool, ReportError> {
    let partition =
        field_string(workspace, "partition", "cargo_graph.workspaces[]")?;
    Ok(partition == "product")
}

fn validate_active_workspace_set(
    workspaces: &BTreeSet<String>,
    product: &BTreeSet<String>,
    roots: &BTreeSet<PathBuf>,
) -> Result<(), ReportError> {
    let expected =
        BTreeSet::from(["product".to_owned(), "repository-tooling".to_owned()]);
    if workspaces != &expected
        || product != &BTreeSet::from(["product".to_owned()])
        || !roots.contains(&PathBuf::new())
        || roots.len() != 2
    {
        return Err(reference_error(
            "active Foundation profile requires exact tooling/Product roots",
        ));
    }
    Ok(())
}

fn parse_workspace_root(value: &str) -> Result<PathBuf, ReportError> {
    crate::policy::relative_path(value, "workspace root", true)
        .map_err(|failure| reference_error(failure.to_string()))
}

fn validate_workspace_file(
    root: &Path,
    path: &Path,
    kind: &str,
) -> Result<(), ReportError> {
    if path.parent() == Some(root) {
        return Ok(());
    }
    Err(reference_error(format!(
        "workspace {kind} parent differs from its declared root"
    )))
}

fn validate_workspace_member(
    root: &Path,
    member: &Path,
) -> Result<(), ReportError> {
    if root.as_os_str().is_empty() || member.starts_with(root) {
        return Ok(());
    }
    Err(reference_error("workspace member is outside its declared root"))
}

fn validate_workspace_members(
    workspace: &Value,
    root: &Path,
    product: bool,
) -> Result<(), ReportError> {
    let context = "cargo_graph.workspaces[]";
    for member in required_field_array(workspace, "members", context)? {
        let member = member
            .as_str()
            .ok_or_else(|| reference_error("workspace member is not text"))?;
        let member = parse_relative_path(member, "workspace member")?;
        if !product {
            validate_workspace_member(root, &member)?;
        }
    }
    Ok(())
}

fn collect_profiles(
    value: &Value,
    workspaces: &BTreeSet<String>,
) -> Result<BTreeMap<(String, String), MetadataProfileRef>, ReportError> {
    let mut profiles = BTreeMap::new();
    for profile in required_array(value, "/identity_inputs/metadata_profiles")?
    {
        let context = "metadata_profiles[]";
        let workspace = field_string(profile, "workspace", context)?;
        let profile_id = field_string(profile, "profile", context)?;
        require_member(workspaces, workspace, "metadata profile workspace")?;
        let key = (workspace.to_owned(), profile_id.to_owned());
        let retained = MetadataProfileRef {
            target: field_string(profile, "target_triple", context)?.to_owned(),
            all_features: field_bool(profile, "all_features", context)?,
            no_default_features: field_bool(
                profile,
                "no_default_features",
                context,
            )?,
            features: string_array(profile, "features", context)?,
            metadata_digest: field_string(profile, "metadata_digest", context)?
                .to_owned(),
        };
        if profiles.insert(key, retained).is_some() {
            return Err(error(format!(
                "duplicate metadata profile identity {workspace}/{profile_id}"
            )));
        }
    }
    Ok(profiles)
}

fn validate_metadata_runs(
    value: &Value,
    workspaces: &CollectedWorkspaces,
    profiles: &BTreeMap<(String, String), MetadataProfileRef>,
) -> Result<PathBuf, ReportError> {
    let processes = collect_metadata_processes(value)?;
    let mut runs = BTreeSet::new();
    let mut repository_root = None;
    for run in required_array(value, "/cargo_graph/metadata_runs")? {
        let key = metadata_run_key(run)?;
        if !runs.insert(key.clone()) {
            return Err(reference_error("duplicate metadata-run identity"));
        }
        let process = processes.get(&key).ok_or_else(|| {
            reference_error("metadata run lacks one matching Cargo process")
        })?;
        let root =
            validate_metadata_run(value, run, process, workspaces, profiles)?;
        retain_repository_root(&mut repository_root, root)?;
    }
    let expected = expected_metadata_keys(value, profiles)?;
    let process_keys = processes.keys().cloned().collect::<BTreeSet<_>>();
    if runs != expected || process_keys != expected {
        return Err(reference_error(
            "metadata runs and Cargo processes do not exactly cover profiles",
        ));
    }
    repository_root.ok_or_else(|| {
        reference_error("metadata runs retain no repository root")
    })
}

type MetadataKey = (String, String, String);

fn metadata_run_key(run: &Value) -> Result<MetadataKey, ReportError> {
    let context = "cargo_graph.metadata_runs[]";
    Ok((
        field_string(run, "workspace", context)?.to_owned(),
        field_string(run, "profile", context)?.to_owned(),
        field_string(run, "phase", context)?.to_owned(),
    ))
}

fn collect_metadata_processes(
    value: &Value,
) -> Result<BTreeMap<MetadataKey, &Value>, ReportError> {
    let mut processes = BTreeMap::new();
    for process in required_array(value, "/processes")? {
        let Some(phase) = metadata_process_phase(process)? else {
            continue;
        };
        let workspace = field_string(process, "workspace", "metadata process")?;
        let profile = field_string(process, "profile", "metadata process")?;
        let key = (workspace.to_owned(), profile.to_owned(), phase.to_owned());
        if processes.insert(key, process).is_some() {
            return Err(reference_error("duplicate Cargo metadata process"));
        }
    }
    Ok(processes)
}

fn metadata_process_phase(
    process: &Value,
) -> Result<Option<&'static str>, ReportError> {
    match field_string(process, "purpose", "process")? {
        "cargo_metadata" => Ok(Some("initial")),
        "cargo_metadata_final" => Ok(Some("final")),
        _ => Ok(None),
    }
}

fn validate_metadata_run(
    value: &Value,
    run: &Value,
    process: &Value,
    workspaces: &CollectedWorkspaces,
    profiles: &BTreeMap<(String, String), MetadataProfileRef>,
) -> Result<PathBuf, ReportError> {
    let key = metadata_run_key(run)?;
    let profile =
        profiles.get(&(key.0.clone(), key.1.clone())).ok_or_else(|| {
            reference_error("metadata run references an unknown profile")
        })?;
    let manifest = workspaces.manifests.get(&key.0).ok_or_else(|| {
        reference_error("metadata run references an unknown workspace")
    })?;
    let argv = string_array(run, "command_argv", "metadata run")?;
    let root = metadata_repository_root(&argv, manifest)?;
    validate_metadata_argv(value, &argv, &root, manifest, &key, profile)?;
    validate_metadata_digests(run, process, profile)?;
    validate_metadata_process(value, run, process)?;
    Ok(root)
}

fn metadata_repository_root(
    argv: &[String],
    manifest: &Path,
) -> Result<PathBuf, ReportError> {
    let absolute = argv.get(6).ok_or_else(|| {
        reference_error("Cargo metadata argv omits its manifest path")
    })?;
    if !normalized_file_path(absolute)
        || !Path::new(absolute).ends_with(manifest)
    {
        return Err(reference_error(
            "Cargo metadata manifest is not the retained workspace manifest",
        ));
    }
    let mut root = PathBuf::from(absolute);
    for _ in manifest.components() {
        root.pop();
    }
    let root_text = root.to_str().ok_or_else(|| {
        reference_error("Cargo metadata repository root is not UTF-8")
    })?;
    if !normalized_absolute_text(root_text)
        || root.join(manifest) != Path::new(absolute)
    {
        return Err(reference_error(
            "Cargo metadata repository root is not normalized",
        ));
    }
    Ok(root)
}

fn validate_metadata_argv(
    value: &Value,
    argv: &[String],
    root: &Path,
    manifest: &Path,
    key: &MetadataKey,
    profile: &MetadataProfileRef,
) -> Result<(), ReportError> {
    let cargo = value
        .pointer("/identity_inputs/cargo")
        .ok_or_else(|| identity_error("Cargo identity is absent"))?;
    let executable = field_string(cargo, "canonical_path", "Cargo identity")?;
    let selected = crate::topology::Profile {
        name: key.1.clone(),
        target: profile.target.clone(),
        all_features: profile.all_features,
        no_default_features: profile.no_default_features,
        features: profile.features.clone(),
    };
    let mut expected = vec![executable.to_owned()];
    let arguments =
        crate::cargo::exact_metadata_arguments(root, manifest, &selected)
            .map_err(|failure| reference_error(failure.to_string()))?;
    expected.extend(arguments);
    if argv != expected {
        return Err(reference_error(
            "Cargo metadata argv differs from the closed profile",
        ));
    }
    Ok(())
}

fn validate_metadata_digests(
    run: &Value,
    process: &Value,
    profile: &MetadataProfileRef,
) -> Result<(), ReportError> {
    let context = "metadata run";
    let stdout = field_string(run, "stdout_digest", context)?;
    let metadata = field_string(run, "metadata_digest", context)?;
    let process_stdout =
        process.pointer("/stdout/digest").and_then(Value::as_str).ok_or_else(
            || reference_error("metadata process stdout digest is absent"),
        )?;
    if stdout != metadata
        || metadata != profile.metadata_digest
        || stdout != process_stdout
    {
        return Err(reference_error(
            "metadata/profile/process digests are not identical",
        ));
    }
    Ok(())
}

fn validate_metadata_process(
    value: &Value,
    run: &Value,
    process: &Value,
) -> Result<(), ReportError> {
    let cargo = value
        .pointer("/identity_inputs/cargo")
        .ok_or_else(|| identity_error("Cargo identity is absent"))?;
    let expected_digest =
        field_string(cargo, "executable_digest", "Cargo identity")?;
    let process_digest =
        field_string(process, "executable_digest", "metadata process")?;
    let run_argv = string_array(run, "command_argv", "metadata run")?;
    let process_argv = string_array(process, "argv", "metadata process")?;
    if process_digest != expected_digest || process_argv != run_argv {
        return Err(reference_error(
            "metadata process differs from its Cargo identity or run argv",
        ));
    }
    Ok(())
}

fn retain_repository_root(
    retained: &mut Option<PathBuf>,
    observed: PathBuf,
) -> Result<(), ReportError> {
    if let Some(expected) = retained
        && expected != &observed
    {
        return Err(reference_error(
            "metadata runs disagree on the repository root",
        ));
    }
    *retained = Some(observed);
    Ok(())
}

fn expected_metadata_keys(
    value: &Value,
    profiles: &BTreeMap<(String, String), MetadataProfileRef>,
) -> Result<BTreeSet<MetadataKey>, ReportError> {
    let phases: &[&str] = match required_string(value, "/mode")? {
        "candidate" => &["initial"],
        "exact" => &["initial", "final"],
        _ => return Err(identity_error("metadata mode is invalid")),
    };
    let mut expected = BTreeSet::new();
    for (workspace, profile) in profiles.keys() {
        for phase in phases {
            expected.insert((
                workspace.clone(),
                profile.clone(),
                (*phase).to_owned(),
            ));
        }
    }
    Ok(expected)
}

type PackageReferences = BTreeSet<(String, String, String)>;

struct CollectedPackages {
    ids: PackageReferences,
    details: BTreeMap<(String, String), GraphPackageRef>,
}

fn collect_packages(
    value: &Value,
    profiles: &BTreeSet<(String, String)>,
    workspaces: &CollectedWorkspaces,
    source: &SourceAuthority,
) -> Result<CollectedPackages, ReportError> {
    let mut packages = BTreeSet::new();
    let mut package_targets =
        BTreeMap::<(String, String), BTreeSet<String>>::new();
    let mut package_details = BTreeMap::new();
    for package in required_array(value, "/cargo_graph/workspace_packages")? {
        let workspace =
            field_string(package, "workspace", "workspace_packages[]")?;
        let profile = field_string(package, "profile", "workspace_packages[]")?;
        let id = field_string(package, "id", "workspace_packages[]")?;
        require_profile(profiles, workspace, profile, "workspace package")?;
        if !packages.insert((
            workspace.to_owned(),
            profile.to_owned(),
            id.to_owned(),
        )) {
            return Err(error(format!(
                "duplicate workspace/profile/package identity \
                 {workspace}/{profile}/{id}"
            )));
        }
        let targets =
            required_field_array(package, "targets", "workspace_packages[]")?
                .iter()
                .map(|target| {
                    field_string(target, "id", "workspace_packages[].targets[]")
                        .map(str::to_owned)
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
        if targets.len()
            != required_field_array(package, "targets", "workspace_packages[]")?
                .len()
        {
            return Err(error(format!(
                "package {workspace}/{id} has duplicate target IDs"
            )));
        }
        let package_key = (workspace.to_owned(), id.to_owned());
        if let Some(existing) = package_targets.get(&package_key)
            && existing != &targets
        {
            return Err(error(format!(
                "package {workspace}/{id} changes target identity across \
                 metadata profiles"
            )));
        }
        package_targets.insert(package_key, targets);
        let package_key = (workspace.to_owned(), id.to_owned());
        let details = graph_package_ref(package, workspaces, source)?;
        if let Some(existing) = package_details.get(&package_key)
            && existing != &details
        {
            return Err(error(format!(
                "package {workspace}/{id} changes retained identity across \
                 metadata profiles"
            )));
        }
        package_details.insert(package_key, details);
    }
    Ok(CollectedPackages { ids: packages, details: package_details })
}

fn graph_package_ref(
    package: &Value,
    workspaces: &CollectedWorkspaces,
    source: &SourceAuthority,
) -> Result<GraphPackageRef, ReportError> {
    let context = "workspace_packages[]";
    let mut targets = BTreeMap::new();
    for target in required_field_array(package, "targets", context)? {
        let id = field_string(target, "id", "workspace package target")?;
        targets.insert(id.to_owned(), graph_target_ref(target)?);
    }
    let owners = package_owner_set(package)?;
    validate_package_owners(package, &targets, &owners, workspaces, source)?;
    Ok(GraphPackageRef {
        name: field_string(package, "name", context)?.to_owned(),
        version: field_string(package, "version", context)?.to_owned(),
        manifest: field_string(package, "manifest", context)?.to_owned(),
        owners,
        targets,
    })
}

fn package_owner_set(
    package: &Value,
) -> Result<BTreeSet<SourceOwner>, ReportError> {
    required_field_array(package, "owners", "workspace package")?
        .iter()
        .map(|owner| parse_owner_value(owner, "workspace package owner"))
        .collect()
}

fn validate_package_owners(
    package: &Value,
    targets: &BTreeMap<String, GraphTargetRef>,
    owners: &BTreeSet<SourceOwner>,
    workspaces: &CollectedWorkspaces,
    source: &SourceAuthority,
) -> Result<(), ReportError> {
    let context = "workspace package";
    let workspace = field_string(package, "workspace", context)?;
    let partition =
        workspaces.partitions.get(workspace).map(String::as_str).ok_or_else(
            || reference_error("workspace package partition absent"),
        )?;
    if partition == "repository_tooling"
        && owners != &BTreeSet::from([SourceOwner::RepositoryTooling])
    {
        return Err(reference_error(
            "repository-tooling package is not RepositoryTooling-only",
        ));
    }
    if partition == "product"
        && owners
            .iter()
            .any(|owner| matches!(owner, SourceOwner::RepositoryTooling))
    {
        return Err(reference_error(
            "Product workspace package contains RepositoryTooling ownership",
        ));
    }
    let mut source_owners = BTreeSet::new();
    collect_path_owner(
        field_string(package, "manifest", context)?,
        source,
        &mut source_owners,
    )?;
    for target in targets.values() {
        collect_path_owner(&target.src_path, source, &mut source_owners)?;
    }
    if &source_owners != owners {
        return Err(reference_error(
            "workspace package owners differ from retained source authority",
        ));
    }
    Ok(())
}

fn collect_path_owner(
    path: &str,
    source: &SourceAuthority,
    owners: &mut BTreeSet<SourceOwner>,
) -> Result<(), ReportError> {
    let normalized = parse_relative_path(path, "workspace package source")?;
    let retained = source.files.get(&normalized).ok_or_else(|| {
        reference_error(format!(
            "workspace package source {} is not retained",
            normalized.display()
        ))
    })?;
    owners.insert(retained.owner);
    Ok(())
}

fn graph_target_ref(target: &Value) -> Result<GraphTargetRef, ReportError> {
    let context = "workspace package target";
    Ok(GraphTargetRef {
        name: field_string(target, "name", context)?.to_owned(),
        src_path: field_string(target, "src_path", context)?.to_owned(),
        kind: string_array(target, "kind", context)?,
        crate_types: string_array(target, "crate_types", context)?,
    })
}

fn string_array(
    value: &Value,
    field: &str,
    context: &str,
) -> Result<Vec<String>, ReportError> {
    required_field_array(value, field, context)?
        .iter()
        .map(|item| {
            item.as_str().map(str::to_owned).ok_or_else(|| {
                error(format!("{context}.{field} contains a non-string"))
            })
        })
        .collect()
}

type ExternalPackageReferences =
    BTreeSet<(String, String, String, String, String)>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum PackageEndpointRef {
    Workspace { workspace: String, package: String },
    External { name: String, version: String, source: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct EdgeIdentity {
    workspace: String,
    profile: String,
    consumer: PackageEndpointRef,
    provider: PackageEndpointRef,
    dependency_name: String,
    kind: String,
    target: Option<String>,
}

type EdgeGraph = BTreeMap<
    (String, String, PackageEndpointRef),
    BTreeSet<PackageEndpointRef>,
>;

fn collect_external_packages(
    value: &Value,
    profiles: &BTreeSet<(String, String)>,
) -> Result<ExternalPackageReferences, ReportError> {
    let mut external_packages = BTreeSet::new();
    for package in required_array(value, "/cargo_graph/external_packages")? {
        let workspace =
            field_string(package, "workspace", "external_packages[]")?;
        let profile = field_string(package, "profile", "external_packages[]")?;
        require_profile(profiles, workspace, profile, "external package")?;
        let key = (
            workspace.to_owned(),
            profile.to_owned(),
            field_string(package, "name", "external_packages[]")?.to_owned(),
            field_string(package, "version", "external_packages[]")?.to_owned(),
            field_string(package, "source", "external_packages[]")?.to_owned(),
        );
        if !external_packages.insert(key) {
            return Err(error("duplicate external package identity"));
        }
    }
    Ok(external_packages)
}

fn validate_cargo_edges(
    value: &Value,
    profiles: &BTreeSet<(String, String)>,
    packages: &BTreeSet<(String, String, String)>,
    external_packages: &ExternalPackageReferences,
) -> Result<EdgeGraph, ReportError> {
    let mut identities = BTreeSet::new();
    let mut graph = BTreeMap::<_, BTreeSet<_>>::new();
    for edge in required_array(value, "/cargo_graph/edges")? {
        let context = "cargo_graph.edges[]";
        let workspace = field_string(edge, "workspace", context)?;
        let profile = field_string(edge, "profile", context)?;
        require_profile(profiles, workspace, profile, "Cargo edge")?;
        let consumer = package_endpoint_ref(
            object_field(edge, "consumer", context)?,
            workspace,
            profile,
            packages,
            external_packages,
        )?;
        let provider = package_endpoint_ref(
            object_field(edge, "provider", context)?,
            workspace,
            profile,
            packages,
            external_packages,
        )?;
        validate_edge_authority(edge)?;
        let identity = EdgeIdentity {
            workspace: workspace.to_owned(),
            profile: profile.to_owned(),
            consumer: consumer.clone(),
            provider: provider.clone(),
            dependency_name: field_string(edge, "dependency_name", context)?
                .to_owned(),
            kind: field_string(edge, "kind", context)?.to_owned(),
            target: edge
                .get("target")
                .and_then(Value::as_str)
                .map(str::to_owned),
        };
        if !identities.insert(identity) {
            return Err(reference_error("duplicate Cargo edge identity"));
        }
        graph
            .entry((workspace.to_owned(), profile.to_owned(), consumer))
            .or_default()
            .insert(provider);
    }
    Ok(graph)
}

fn validate_reachability(
    value: &Value,
    packages: &CollectedPackages,
    external_packages: &ExternalPackageReferences,
    edges: &EdgeGraph,
) -> Result<(), ReportError> {
    let mut records = BTreeMap::new();
    for reachability in required_array(value, "/cargo_graph/reachability")? {
        let context = "cargo_graph.reachability[]";
        let workspace = field_string(reachability, "workspace", context)?;
        let profile = field_string(reachability, "profile", context)?;
        let package = field_string(reachability, "package", context)?;
        require_package(
            &packages.ids,
            workspace,
            profile,
            package,
            "reachability root",
        )?;
        let members = string_array(reachability, "members", context)?;
        let member_set = members.iter().cloned().collect::<BTreeSet<_>>();
        if member_set.len() != members.len() {
            return Err(reference_error("duplicate reachability member"));
        }
        let key =
            (workspace.to_owned(), profile.to_owned(), package.to_owned());
        if records.insert(key, member_set).is_some() {
            return Err(reference_error("duplicate reachability root record"));
        }
    }
    if records.keys().cloned().collect::<BTreeSet<_>>() != packages.ids {
        return Err(reference_error(
            "reachability records do not exactly cover workspace packages",
        ));
    }
    for ((workspace, profile, package), members) in records {
        let expected = compute_reachability(
            &workspace,
            &profile,
            &package,
            packages,
            external_packages,
            edges,
        )?;
        if members != expected {
            return Err(reference_error(format!(
                "reachability closure differs for \
                 {workspace}/{profile}/{package}"
            )));
        }
    }
    Ok(())
}

fn validate_edge_authority(edge: &Value) -> Result<(), ReportError> {
    let consumers = edge_owner_set(edge, "consumer_owners")?;
    let providers = edge_owner_set(edge, "provider_owners")?;
    let decision = field_string(edge, "decision", "Cargo edge")?;
    let tooling = BTreeSet::from([SourceOwner::RepositoryTooling]);
    if consumers == tooling
        && providers == tooling
        && decision == "admitted_same_owner"
    {
        return Ok(());
    }
    let kernel_interface = BTreeSet::from([SourceOwner::Product(
        ArchitecturalRole::KernelInterface,
    )]);
    let foundation =
        BTreeSet::from([SourceOwner::Product(ArchitecturalRole::Foundation)]);
    if consumers == kernel_interface
        && providers == foundation
        && decision == "admitted_product_edge"
    {
        return Ok(());
    }
    Err(reference_error(
        "Cargo edge authority differs from the active closed profile",
    ))
}

fn edge_owner_set(
    edge: &Value,
    field: &str,
) -> Result<BTreeSet<SourceOwner>, ReportError> {
    required_field_array(edge, field, "Cargo edge")?
        .iter()
        .map(|owner| parse_owner_value(owner, "Cargo edge owner"))
        .collect()
}

fn compute_reachability(
    workspace: &str,
    profile: &str,
    package: &str,
    packages: &CollectedPackages,
    external_packages: &ExternalPackageReferences,
    edges: &EdgeGraph,
) -> Result<BTreeSet<String>, ReportError> {
    let root = PackageEndpointRef::Workspace {
        workspace: workspace.to_owned(),
        package: package.to_owned(),
    };
    let mut pending = VecDeque::from([root]);
    let mut visited = BTreeSet::new();
    let mut members = BTreeSet::new();
    while let Some(endpoint) = pending.pop_front() {
        if !visited.insert(endpoint.clone()) {
            continue;
        }
        members.insert(endpoint_member_identity(
            &endpoint,
            packages,
            external_packages,
            workspace,
            profile,
        )?);
        enqueue_endpoint_dependencies(
            edges,
            workspace,
            profile,
            &endpoint,
            &mut pending,
        );
    }
    Ok(members)
}

fn enqueue_endpoint_dependencies(
    edges: &EdgeGraph,
    workspace: &str,
    profile: &str,
    endpoint: &PackageEndpointRef,
    pending: &mut VecDeque<PackageEndpointRef>,
) {
    let key = (workspace.to_owned(), profile.to_owned(), endpoint.clone());
    if let Some(providers) = edges.get(&key) {
        pending.extend(providers.iter().cloned());
    }
}

fn endpoint_member_identity(
    endpoint: &PackageEndpointRef,
    packages: &CollectedPackages,
    external_packages: &ExternalPackageReferences,
    workspace: &str,
    profile: &str,
) -> Result<String, ReportError> {
    match endpoint {
        PackageEndpointRef::Workspace { workspace, package } => {
            let details = packages
                .details
                .get(&(workspace.clone(), package.clone()))
                .ok_or_else(|| reference_error("unknown workspace endpoint"))?;
            Ok(format!("{} {} (workspace)", details.name, details.version))
        }
        PackageEndpointRef::External { name, version, source } => {
            let key = (
                workspace.to_owned(),
                profile.to_owned(),
                name.clone(),
                version.clone(),
                source.clone(),
            );
            if !external_packages.contains(&key) {
                return Err(reference_error("unknown external endpoint"));
            }
            Ok(format!("{name} {version} ({source})"))
        }
    }
}

fn validate_product_references(
    value: &Value,
    graph: &GraphReferences,
    source: &SourceAuthority,
) -> Result<(), ReportError> {
    let product = value
        .get("product_proof")
        .ok_or_else(|| error("product_proof is absent"))?;
    let active =
        !required_field_array(product, "closures", "product_proof")?.is_empty();
    let active_refs = if active {
        Some(validate_active_product_state(value, product, graph, source)?)
    } else {
        validate_product_empty_state(value, product, graph, source)?;
        None
    };
    let passing = required_string(value, "/result/status")? == "pass";
    validate_process_references(
        value,
        &graph.workspaces,
        &graph.profiles,
        passing,
        active_refs.as_ref(),
    )?;
    validate_process_purposes(value, graph, active_refs.as_ref())
}

const SHIPPING_CLOSURE: &str = "foundation-shipping";
const CONSUMER_CLOSURE: &str = "foundation-consumer";
const PRODUCT_WORKSPACE: &str = "product";
const PRODUCT_PROFILE: &str = "release";
const SHIPPING_PACKAGE: &str = "foundation";
const CONSUMER_PACKAGE: &str = "foundation-public-consumer";
const SHIPPING_CARGO_NAME: &str = "reovim-foundation";
const CONSUMER_CARGO_NAME: &str = "reovim-foundation-public-consumer";
const SHIPPING_TARGET: &str = "library";
const CONSUMER_TARGET: &str = "library";
const TARGET_TRIPLE: &str = "x86_64-unknown-linux-gnu";
const PREPARED_CARGO: &str = "/compiler/bin/cargo";
const PREPARED_RUSTC: &str = "/compiler/bin/rustc";
const PREPARED_CHECKER: &str = "/checker/reovim-conformance";
const PREPARED_MANIFEST: &str = "/repo/product/Cargo.toml";
const NETWORK_POLICY_BASIS: &str = "bounded-nftables-empty-table-dump";
const NETWORK_POLICY_DOMAIN: &[u8] = b"reovim-nftables-table-set/v1\0";
const PRODUCT_REPOSITORY_INPUTS: [&str; 10] = [
    "product/Cargo.toml",
    "product/Cargo.lock",
    "kernel/foundation/Cargo.toml",
    "kernel/foundation/src/bound.rs",
    "kernel/foundation/src/carrier.rs",
    "kernel/foundation/src/contract.rs",
    "kernel/foundation/src/identity.rs",
    "kernel/foundation/src/lib.rs",
    "tests/foundation-public-consumer/Cargo.toml",
    "tests/foundation-public-consumer/src/lib.rs",
];

struct ActiveProductReferences {
    execution: String,
}

struct ActiveClosureReferences {
    execution: String,
    shipping_unit: String,
    consumer_unit: String,
    shipping_events: BTreeMap<String, ActiveCargoEventRef>,
    consumer_events: BTreeMap<String, ActiveCargoEventRef>,
}

struct ActiveCargoCaptureRef {
    compiled: BTreeSet<String>,
    fresh: BTreeSet<String>,
    events: BTreeMap<String, ActiveCargoEventRef>,
}

struct ActiveCargoEventRef {
    fresh: bool,
    filenames: BTreeSet<String>,
}

struct ActiveUnitRef {
    closure: String,
}

#[derive(Clone)]
struct ActiveArtifactRef {
    closure: String,
    path: String,
    content_digest: String,
    size_bytes: u64,
}

fn validate_active_product_state(
    value: &Value,
    product: &Value,
    graph: &GraphReferences,
    source: &SourceAuthority,
) -> Result<ActiveProductReferences, ReportError> {
    if required_string(value, "/mode")? != "exact"
        || required_string(value, "/result/status")? != "pass"
    {
        return Err(reference_error(
            "active Foundation evidence is not an exact passing proof",
        ));
    }
    validate_active_graph(value, graph)?;
    validate_active_source_set(source)?;
    validate_active_identity_inputs(value, product)?;
    validate_active_runtime_exposures(value, product)?;
    let executions = validate_active_execution(value, product, graph, source)?;
    let closures =
        validate_active_closures(value, product, graph, &executions)?;
    let units = validate_active_units(value, product, graph, &closures)?;
    validate_active_trace_total(value, product)?;
    validate_active_inputs(value, product, source, &units)?;
    let artifacts = validate_active_artifacts(value, product, &units)?;
    validate_active_provider_artifact(value, product, &closures, &artifacts)?;
    validate_product_scratch_paths(product)?;
    validate_active_extern_edges(product, &closures, &units, &artifacts)?;
    validate_active_package_membership(value, product, source, graph)?;
    validate_active_public_consumer(value, product, &closures, &units)?;
    Ok(ActiveProductReferences { execution: executions })
}

fn validate_product_scratch_paths(product: &Value) -> Result<(), ReportError> {
    let executions = required_field_array(product, "executions", "product")?;
    let execution = executions
        .first()
        .ok_or_else(|| reference_error("active Product execution is absent"))?;
    let scratch = object_field(execution, "scratch", "Product execution")?;
    let root = Path::new(field_string(scratch, "mountpoint", "scratch")?);
    if !root.is_absolute() {
        return Err(reference_error("scratch mountpoint is not absolute"));
    }
    for unit in required_field_array(product, "compiler_units", "product")? {
        require_under_scratch(
            field_string(unit, "dep_info_path", "compiler unit")?,
            root,
            "dep-info",
        )?;
    }
    for artifact in required_field_array(product, "artifacts", "product")? {
        require_under_scratch(
            field_string(artifact, "path", "artifact")?,
            root,
            "artifact",
        )?;
    }
    Ok(())
}

fn require_under_scratch(
    value: &str,
    root: &Path,
    context: &str,
) -> Result<(), ReportError> {
    let path = Path::new(value);
    if path != root && path.starts_with(root) {
        Ok(())
    } else {
        Err(reference_error(format!(
            "{context} path is outside Product scratch"
        )))
    }
}

fn validate_active_source_set(
    source: &SourceAuthority,
) -> Result<(), ReportError> {
    let actual = source
        .files
        .iter()
        .filter_map(|(path, item)| {
            matches!(item.owner, SourceOwner::Product(_))
                .then_some(path.clone())
        })
        .collect::<BTreeSet<_>>();
    let expected = [
        "kernel/foundation/Cargo.toml",
        "kernel/foundation/src/bound.rs",
        "kernel/foundation/src/carrier.rs",
        "kernel/foundation/src/contract.rs",
        "kernel/foundation/src/identity.rs",
        "kernel/foundation/src/lib.rs",
        "tests/foundation-public-consumer/Cargo.toml",
        "tests/foundation-public-consumer/src/lib.rs",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    if actual != expected {
        return Err(reference_error(
            "Product source does not exactly match the Foundation journey",
        ));
    }
    Ok(())
}

fn validate_active_graph(
    value: &Value,
    graph: &GraphReferences,
) -> Result<(), ReportError> {
    validate_active_workspace_graph(graph)?;
    validate_active_product_profile(graph)?;
    validate_active_product_packages(graph)?;
    validate_active_cargo_edge(value, graph)
}

fn validate_active_workspace_graph(
    graph: &GraphReferences,
) -> Result<(), ReportError> {
    if graph.product_workspaces
        != BTreeSet::from([PRODUCT_WORKSPACE.to_owned()])
        || graph.partitions.get(PRODUCT_WORKSPACE).map(String::as_str)
            != Some("product")
    {
        return Err(reference_error(
            "active Foundation proof has the wrong Product workspace",
        ));
    }
    if graph.workspace_roots.get("repository-tooling") != Some(&PathBuf::new())
        || graph.workspace_roots.get(PRODUCT_WORKSPACE)
            != Some(&PathBuf::from("product"))
        || graph.workspace_manifests.get(PRODUCT_WORKSPACE)
            != Some(&PathBuf::from("product/Cargo.toml"))
        || graph.workspace_lockfiles.get(PRODUCT_WORKSPACE).map(String::as_str)
            != Some("product/Cargo.lock")
    {
        return Err(reference_error(
            "active Product workspace paths differ from the closed profile",
        ));
    }
    let expected_members =
        ["kernel/foundation", "tests/foundation-public-consumer"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
    if graph.workspace_members.get(PRODUCT_WORKSPACE) != Some(&expected_members)
    {
        return Err(reference_error(
            "active Product workspace members differ from the closed profile",
        ));
    }
    if graph.external_packages.iter().any(|entry| entry.0 == PRODUCT_WORKSPACE)
    {
        return Err(reference_error(
            "active Foundation Product graph contains an external package",
        ));
    }
    Ok(())
}

fn validate_active_product_profile(
    graph: &GraphReferences,
) -> Result<(), ReportError> {
    let product_profiles = graph
        .profiles
        .iter()
        .filter(|(workspace, _)| workspace == PRODUCT_WORKSPACE)
        .cloned()
        .collect::<BTreeSet<_>>();
    let expected_profile = BTreeSet::from([(
        PRODUCT_WORKSPACE.to_owned(),
        PRODUCT_PROFILE.to_owned(),
    )]);
    if product_profiles != expected_profile {
        return Err(reference_error(
            "active Product profile set differs from the closed profile",
        ));
    }
    let profile_key =
        (PRODUCT_WORKSPACE.to_owned(), PRODUCT_PROFILE.to_owned());
    let profile = graph.profile_details.get(&profile_key).ok_or_else(|| {
        reference_error("active Product metadata profile is absent")
    })?;
    if profile.target != TARGET_TRIPLE
        || profile.all_features
        || profile.no_default_features
        || !profile.features.is_empty()
    {
        return Err(reference_error(
            "active Product metadata selection differs from the closed profile",
        ));
    }
    Ok(())
}

fn validate_active_product_packages(
    graph: &GraphReferences,
) -> Result<(), ReportError> {
    let product_packages = graph
        .packages
        .keys()
        .filter(|(workspace, _)| workspace == PRODUCT_WORKSPACE)
        .map(|(_, package)| package.clone())
        .collect::<BTreeSet<_>>();
    let expected_packages = BTreeSet::from([
        SHIPPING_PACKAGE.to_owned(),
        CONSUMER_PACKAGE.to_owned(),
    ]);
    if product_packages != expected_packages {
        return Err(reference_error(
            "active Product package set differs from the closed profile",
        ));
    }
    require_active_package(
        graph,
        SHIPPING_PACKAGE,
        ArchitecturalRole::Foundation,
        SHIPPING_CARGO_NAME,
        "kernel/foundation/Cargo.toml",
        "reovim_foundation",
        "kernel/foundation/src/lib.rs",
    )?;
    require_active_package(
        graph,
        CONSUMER_PACKAGE,
        ArchitecturalRole::KernelInterface,
        CONSUMER_CARGO_NAME,
        "tests/foundation-public-consumer/Cargo.toml",
        "reovim_foundation_public_consumer",
        "tests/foundation-public-consumer/src/lib.rs",
    )?;
    Ok(())
}

fn require_active_package(
    graph: &GraphReferences,
    package: &str,
    role: ArchitecturalRole,
    name: &str,
    manifest: &str,
    target_name: &str,
    target_source: &str,
) -> Result<(), ReportError> {
    let details = graph
        .packages
        .get(&(PRODUCT_WORKSPACE.to_owned(), package.to_owned()))
        .ok_or_else(|| reference_error("active Product package is absent"))?;
    let target = details.targets.get("library");
    if details.name != name
        || details.version != "0.16.0-dev"
        || details.manifest != manifest
        || details.owners != BTreeSet::from([SourceOwner::Product(role)])
        || details.targets.len() != 1
        || target.is_none_or(|target| {
            target.name != target_name
                || target.src_path != target_source
                || target.kind != ["lib"]
                || target.crate_types != ["lib"]
        })
        || !graph.profiles.contains(&(
            PRODUCT_WORKSPACE.to_owned(),
            PRODUCT_PROFILE.to_owned(),
        ))
    {
        return Err(reference_error(
            "active Product package/profile differs from the closed shape",
        ));
    }
    Ok(())
}

fn validate_active_cargo_edge(
    value: &Value,
    graph: &GraphReferences,
) -> Result<(), ReportError> {
    let consumer = PackageEndpointRef::Workspace {
        workspace: PRODUCT_WORKSPACE.to_owned(),
        package: CONSUMER_PACKAGE.to_owned(),
    };
    let provider = PackageEndpointRef::Workspace {
        workspace: PRODUCT_WORKSPACE.to_owned(),
        package: SHIPPING_PACKAGE.to_owned(),
    };
    let key =
        (PRODUCT_WORKSPACE.to_owned(), PRODUCT_PROFILE.to_owned(), consumer);
    if graph.edges.get(&key) != Some(&BTreeSet::from([provider])) {
        return Err(reference_error(
            "active Product graph lacks its one Foundation dependency edge",
        ));
    }
    let product_edges = required_array(value, "/cargo_graph/edges")?
        .iter()
        .filter(|edge| edge["workspace"] == PRODUCT_WORKSPACE)
        .collect::<Vec<_>>();
    if product_edges.len() != 1 || !active_cargo_edge_matches(product_edges[0])?
    {
        return Err(reference_error(
            "active Product graph differs from its one dependency edge",
        ));
    }
    Ok(())
}

fn active_cargo_edge_matches(edge: &Value) -> Result<bool, ReportError> {
    let context = "active Product Cargo edge";
    let consumer = object_field(edge, "consumer", context)?;
    let provider = object_field(edge, "provider", context)?;
    Ok(field_string(edge, "profile", context)? == PRODUCT_PROFILE
        && field_string(edge, "dependency_name", context)?
            == "reovim_foundation"
        && field_string(edge, "kind", context)? == "normal"
        && edge.get("target").is_some_and(Value::is_null)
        && workspace_endpoint_matches(consumer, CONSUMER_PACKAGE)?
        && workspace_endpoint_matches(provider, SHIPPING_PACKAGE)?)
}

fn workspace_endpoint_matches(
    endpoint: &Value,
    package: &str,
) -> Result<bool, ReportError> {
    let context = "active Product Cargo endpoint";
    Ok(field_string(endpoint, "kind", context)? == "workspace_package"
        && field_string(endpoint, "workspace", context)? == PRODUCT_WORKSPACE
        && field_string(endpoint, "package", context)? == package)
}

fn validate_active_identity_inputs(
    value: &Value,
    product: &Value,
) -> Result<(), ReportError> {
    let environment = field_string(
        product,
        "selected_execution_environment",
        "product_proof",
    )?;
    if !value
        .pointer("/identity_inputs/product_proof_gate")
        .is_some_and(Value::is_null)
    {
        return Err(reference_error(
            "active Product proof cannot use fixture-gate authority",
        ));
    }
    let substrates =
        required_array(value, "/identity_inputs/compiler_substrates")?;
    if substrates.len() != 1 {
        return Err(reference_error(
            "active Foundation proof requires one compiler substrate",
        ));
    }
    validate_compiler_substrate(value, &substrates[0], environment)
}

fn validate_active_runtime_exposures(
    value: &Value,
    product: &Value,
) -> Result<(), ReportError> {
    let mut runtime_mappings = BTreeSet::new();
    let mut runtime_identities = BTreeMap::new();
    for tool in ["checker", "cargo", "rustc"] {
        let pointer = format!("/identity_inputs/{tool}/runtime_inputs");
        let inputs = required_array(value, &pointer)?;
        if inputs.is_empty() {
            return Err(reference_error(format!(
                "active Product {tool} runtime-input set is empty"
            )));
        }
        for input in inputs {
            let source =
                field_string(input, "canonical_path", "runtime input")?;
            let destination =
                field_string(input, "prepared_path", "runtime input")?;
            let mapping = (source.to_owned(), destination.to_owned());
            runtime_mappings.insert(mapping.clone());
            runtime_identities.insert(
                mapping,
                (
                    field_string(input, "content_digest", "runtime input")?
                        .to_owned(),
                    field_count(input, "size_bytes", "runtime input")?,
                ),
            );
        }
    }
    let executions = required_field_array(product, "executions", "product")?;
    let execution = executions
        .first()
        .ok_or_else(|| reference_error("active Product execution is absent"))?;
    let exposures = required_field_array(execution, "exposure", "execution")?;
    let exposed = exposures
        .iter()
        .filter(|entry| entry["purpose"] == "system runtime")
        .map(|entry| {
            Ok((
                field_string(entry, "source", "system-runtime exposure")?
                    .to_owned(),
                field_string(entry, "destination", "system-runtime exposure")?
                    .to_owned(),
            ))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if exposed.is_empty() || exposed != runtime_mappings {
        return Err(reference_error(
            "tool runtime mappings and system-runtime exposures differ",
        ));
    }
    for ((_, destination), (digest, size)) in runtime_identities {
        require_file_inventory(exposures, &destination, &digest, size)?;
    }
    Ok(())
}

fn validate_compiler_substrate(
    value: &Value,
    substrate: &Value,
    environment: &str,
) -> Result<(), ReportError> {
    let context = "compiler substrate";
    if field_string(substrate, "environment", context)? != environment
        || field_string(substrate, "host_triple", context)? != TARGET_TRIPLE
        || field_string(substrate, "target_triple", context)? != TARGET_TRIPLE
    {
        return Err(reference_error(
            "compiler substrate environment or target differs",
        ));
    }
    let support = field_string(substrate, "compiler_support_root", context)?;
    let target = field_string(substrate, "target_libdir", context)?;
    if support != PREPARED_COMPILER_SUPPORT || target != PREPARED_TARGET_LIBDIR
    {
        return Err(reference_error(
            "compiler substrate roots differ from the prepared profile",
        ));
    }
    enforce_product_path_limit(value, support, "compiler substrate root")?;
    enforce_product_path_limit(value, target, "compiler substrate root")?;
    let entries = required_field_array(substrate, "entries", context)?;
    let count = u64::try_from(entries.len())
        .map_err(|_| limit_error("compiler substrate count overflow"))?;
    let limit = required_count(
        value,
        "/limits/product_execution",
        "compiler_substrate_entry_count",
    )?;
    if count == 0
        || count > limit
        || field_count(substrate, "entry_count", context)? != count
    {
        return Err(limit_error(
            "compiler substrate entry count differs from its bound",
        ));
    }
    validate_compiler_substrate_entries(value, substrate, entries)
}

fn validate_compiler_substrate_entries(
    value: &Value,
    substrate: &Value,
    entries: &[Value],
) -> Result<(), ReportError> {
    let mut paths = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    let mut total = 0_u64;
    for entry in entries {
        let path = field_string(entry, "canonical_path", "substrate entry")?;
        if !normalized_file_path(path) || !paths.insert(path.to_owned()) {
            return Err(reference_error(
                "compiler substrate entry path is invalid or duplicated",
            ));
        }
        enforce_product_path_limit(value, path, "compiler substrate entry")?;
        let kind = field_string(entry, "kind", "substrate entry")?;
        let root = if kind == "compiler_support" {
            Path::new(PREPARED_COMPILER_SUPPORT)
        } else if kind == "target_libdir" {
            Path::new(PREPARED_TARGET_LIBDIR)
        } else {
            return Err(reference_error(
                "compiler substrate entry has an unknown kind",
            ));
        };
        if Path::new(path).parent() != Some(root) {
            return Err(reference_error(
                "compiler substrate entry escapes its exact root",
            ));
        }
        kinds.insert(kind);
        total = total
            .checked_add(field_count(entry, "size_bytes", "substrate entry")?)
            .ok_or_else(|| limit_error("compiler substrate byte overflow"))?;
    }
    let limit = required_count(
        value,
        "/limits/product_execution",
        "compiler_substrate_total_bytes",
    )?;
    if total == 0
        || total > limit
        || field_count(substrate, "total_bytes", "compiler substrate")? != total
    {
        return Err(limit_error(
            "compiler substrate bytes differ from their bound",
        ));
    }
    if !kinds.contains("compiler_support") || !kinds.contains("target_libdir") {
        return Err(reference_error(
            "compiler substrate omits support or target-libdir evidence",
        ));
    }
    let bytes = serde_json::to_vec(entries).map_err(|failure| {
        identity_error(format!(
            "cannot serialize compiler substrate entries: {failure}"
        ))
    })?;
    if field_string(substrate, "aggregate_digest", "compiler substrate")?
        != digest::bytes(&bytes)
    {
        return Err(identity_error(
            "compiler substrate aggregate digest differs from its entries",
        ));
    }
    Ok(())
}

fn validate_active_execution(
    value: &Value,
    product: &Value,
    graph: &GraphReferences,
    source: &SourceAuthority,
) -> Result<String, ReportError> {
    let executions = required_field_array(product, "executions", "product")?;
    if executions.len() != 1 {
        return Err(reference_error(
            "active Foundation proof requires one execution",
        ));
    }
    let execution = &executions[0];
    let context = "Product execution";
    let id = field_string(execution, "id", context)?.to_owned();
    let environment = field_string(
        product,
        "selected_execution_environment",
        "product_proof",
    )?;
    if field_string(execution, "environment", context)? != environment {
        return Err(reference_error(
            "Product execution environment differs from selection",
        ));
    }
    let closures = string_array(execution, "closures", context)?;
    let expected = BTreeSet::from([
        SHIPPING_CLOSURE.to_owned(),
        CONSUMER_CLOSURE.to_owned(),
    ]);
    if closures.into_iter().collect::<BTreeSet<_>>() != expected {
        return Err(reference_error(
            "Product execution does not select both exact closures",
        ));
    }
    validate_execution_isolation(value, execution, graph, source)?;
    Ok(id)
}

fn validate_execution_isolation(
    value: &Value,
    execution: &Value,
    graph: &GraphReferences,
    source: &SourceAuthority,
) -> Result<(), ReportError> {
    validate_execution_exposure(value, execution)?;
    validate_exposure_manifest(value, execution, graph, source)?;
    require_true_fields(
        object_field(execution, "namespaces", "Product execution")?,
        &[
            "user",
            "mount",
            "network",
            "pid",
            "setgroups_denied",
            "private_propagation",
            "pid_one_reaper",
            "parent_death_signal",
        ],
        "namespace evidence",
    )?;
    validate_namespace_maps(execution)?;
    validate_execution_network(execution)?;
    validate_execution_scratch(value, execution)?;
    require_true_fields(
        object_field(execution, "process_group", "Product execution")?,
        &["identity_validated", "group_absent_after_reap"],
        "process-group evidence",
    )?;
    require_true_fields(
        object_field(execution, "cleanup", "Product execution")?,
        &[
            "processes_absent",
            "readers_joined",
            "namespace_absent",
            "mount_absent",
            "mountpoint_identity_preserved",
            "underlying_directory_empty",
            "source_identity_preserved",
            "tool_identity_preserved",
        ],
        "cleanup evidence",
    )
}

fn require_true_fields(
    value: &Value,
    fields: &[&str],
    context: &str,
) -> Result<(), ReportError> {
    if fields
        .iter()
        .all(|field| value.get(*field).and_then(Value::as_bool) == Some(true))
    {
        Ok(())
    } else {
        Err(reference_error(format!("{context} is not completely proved")))
    }
}

fn validate_namespace_maps(execution: &Value) -> Result<(), ReportError> {
    let namespaces =
        object_field(execution, "namespaces", "Product execution")?;
    for field in ["uid_map", "gid_map"] {
        let map = field_string(namespaces, field, "namespace evidence")?;
        if !valid_single_id_map(map) {
            return Err(reference_error(
                "namespace ID map differs from one root mapping",
            ));
        }
    }
    Ok(())
}

fn valid_single_id_map(value: &str) -> bool {
    let values = value
        .split_ascii_whitespace()
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>();
    values.is_ok_and(|values| {
        values.len() == 3 && values[0] == 0 && values[2] == 1
    })
}

fn validate_execution_exposure(
    value: &Value,
    execution: &Value,
) -> Result<(), ReportError> {
    let entries =
        required_field_array(execution, "exposure", "Product execution")?;
    let limit = required_count(
        value,
        "/limits/product_execution",
        "exposure_entry_count",
    )?;
    let count = u64::try_from(entries.len())
        .map_err(|_| limit_error("exposure count overflow"))?;
    if count == 0 || count > limit {
        return Err(limit_error("Product exposure exceeds its bound"));
    }
    let mut destinations = BTreeSet::new();
    let mut character_devices = 0_u64;
    for entry in entries {
        let source = field_string(entry, "source", "exposure")?;
        let destination = field_string(entry, "destination", "exposure")?;
        if !normalized_absolute_text(source)
            || !normalized_absolute_text(destination)
            || !destinations.insert(destination.to_owned())
        {
            return Err(reference_error(
                "Product exposure destination is invalid or duplicated",
            ));
        }
        character_devices +=
            validate_exposure_kind(entry, source, destination)?;
        let identity = report_inventory_identity(entry)?;
        prepared_root::validate_inventory_identity(&identity)
            .map_err(|failure| reference_error(failure.to_string()))?;
        let count_limit = required_count(
            value,
            "/limits/product_execution",
            "compiler_substrate_entry_count",
        )?;
        let byte_limit = required_count(
            value,
            "/limits/product_execution",
            "compiler_substrate_total_bytes",
        )?;
        if u64::try_from(identity.entry_count)
            .map_err(|_| limit_error("exposure inventory count overflow"))?
            > count_limit
            || identity.total_bytes > byte_limit
        {
            return Err(limit_error(
                "Product exposure inventory exceeds its retained bound",
            ));
        }
        enforce_product_path_limit(value, source, "Product exposure source")?;
        enforce_product_path_limit(
            value,
            destination,
            "Product exposure destination",
        )?;
    }
    if character_devices != 1 {
        return Err(reference_error(
            "Product exposure lacks its one /dev/null character device",
        ));
    }
    Ok(())
}

fn report_inventory_identity(
    exposure: &Value,
) -> Result<prepared_root::InventoryIdentity, ReportError> {
    let mut entries = required_field_array(
        exposure,
        "inventory_entries",
        "Product exposure",
    )?
    .iter()
    .map(report_inventory_entry)
    .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    Ok(prepared_root::InventoryIdentity {
        entry_count: usize::try_from(field_count(
            exposure,
            "inventory_entry_count",
            "Product exposure",
        )?)
        .map_err(|_| limit_error("exposure inventory count overflow"))?,
        total_bytes: field_count(
            exposure,
            "inventory_total_bytes",
            "Product exposure",
        )?,
        aggregate_digest: field_string(
            exposure,
            "inventory_digest",
            "Product exposure",
        )?
        .to_owned(),
        entries,
    })
}

fn report_inventory_entry(
    value: &Value,
) -> Result<prepared_root::InventoryEntry, ReportError> {
    let context = "exposure inventory entry";
    let kind = match field_string(value, "kind", context)? {
        "file" => prepared_root::InventoryEntryKind::File,
        "directory" => prepared_root::InventoryEntryKind::Directory,
        "character_device" => {
            prepared_root::InventoryEntryKind::CharacterDevice
        }
        _ => return Err(reference_error("unknown exposure inventory kind")),
    };
    Ok(prepared_root::InventoryEntry {
        relative_path: PathBuf::from(field_string(
            value,
            "relative_path",
            context,
        )?),
        kind,
        mode: u32::try_from(field_count(value, "mode", context)?)
            .map_err(|_| limit_error("exposure inventory mode overflow"))?,
        size_bytes: field_count(value, "size_bytes", context)?,
        content_digest: optional_report_string(
            value.get("content_digest"),
            "exposure inventory digest",
        )?,
        device_identity: optional_report_count(
            value.get("device_identity"),
            "exposure inventory device",
        )?,
    })
}

fn optional_report_string(
    value: Option<&Value>,
    context: &str,
) -> Result<Option<String>, ReportError> {
    match value {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        _ => Err(error(format!("{context} is not string or null"))),
    }
}

fn optional_report_count(
    value: Option<&Value>,
    context: &str,
) -> Result<Option<u64>, ReportError> {
    match value {
        Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| error(format!("{context} is not unsigned"))),
        _ => Err(error(format!("{context} is not integer or null"))),
    }
}

fn validate_exposure_manifest(
    value: &Value,
    execution: &Value,
    graph: &GraphReferences,
    source: &SourceAuthority,
) -> Result<(), ReportError> {
    let exposures =
        required_field_array(execution, "exposure", "Product execution")?;
    let mut accounted = BTreeSet::new();
    validate_repository_exposure_manifest(
        exposures,
        graph,
        source,
        &mut accounted,
    )?;
    validate_tool_exposure_manifest(value, exposures, &mut accounted)?;
    let (substrate, host_target) =
        validate_compiler_exposure_manifest(value, exposures, &mut accounted)?;
    let target = field_string(substrate, "target_libdir", "substrate")?;
    require_report_exposure(
        exposures,
        Some(host_target.to_str().ok_or_else(|| {
            reference_error("compiler target source path is not UTF-8")
        })?),
        target,
        "directory",
        false,
        "selected Rust target libdir",
        &mut accounted,
    )?;
    require_target_inventory(exposures, target, substrate)?;
    require_report_exposure(
        exposures,
        Some("/dev/null"),
        "/dev/null",
        "character_device",
        false,
        "null standard-input device",
        &mut accounted,
    )?;
    require_null_inventory(exposures)?;
    account_runtime_exposures(exposures, &mut accounted)?;
    if accounted.len() == exposures.len() {
        return Ok(());
    }
    Err(reference_error(
        "Product exposure manifest has a missing or orphan entry",
    ))
}

fn validate_repository_exposure_manifest(
    exposures: &[Value],
    graph: &GraphReferences,
    source: &SourceAuthority,
    accounted: &mut BTreeSet<String>,
) -> Result<(), ReportError> {
    for relative in PRODUCT_REPOSITORY_INPUTS {
        let host_path = graph.repository_root.join(relative);
        let host_text = host_path.to_str().ok_or_else(|| {
            reference_error("repository exposure source is not UTF-8")
        })?;
        let destination = format!("/repo/{relative}");
        require_report_exposure(
            exposures,
            Some(host_text),
            &destination,
            "file",
            false,
            "Product repository input",
            accounted,
        )?;
        let retained =
            source.files.get(Path::new(relative)).ok_or_else(|| {
                reference_error("repository exposure lacks source authority")
            })?;
        require_file_inventory(
            exposures,
            &destination,
            &retained.content_digest,
            retained.size_bytes,
        )?;
    }
    Ok(())
}

fn validate_tool_exposure_manifest(
    value: &Value,
    exposures: &[Value],
    accounted: &mut BTreeSet<String>,
) -> Result<(), ReportError> {
    for (tool, destination, purpose) in [
        ("cargo", PREPARED_CARGO, "Cargo executable"),
        ("rustc", PREPARED_RUSTC, "rustc executable"),
        ("checker", PREPARED_CHECKER, "checker"),
    ] {
        let source = value
            .pointer(&format!("/identity_inputs/{tool}/canonical_path"))
            .and_then(Value::as_str)
            .ok_or_else(|| identity_error("tool canonical path is absent"))?;
        require_report_exposure(
            exposures,
            Some(source),
            destination,
            "file",
            true,
            purpose,
            accounted,
        )?;
        let identity = value
            .pointer(&format!("/identity_inputs/{tool}"))
            .ok_or_else(|| identity_error("tool identity is absent"))?;
        require_file_inventory(
            exposures,
            destination,
            field_string(identity, "executable_digest", "tool identity")?,
            field_count(identity, "executable_size_bytes", "tool identity")?,
        )?;
    }
    Ok(())
}

fn validate_compiler_exposure_manifest<'a>(
    value: &'a Value,
    exposures: &[Value],
    accounted: &mut BTreeSet<String>,
) -> Result<(&'a Value, PathBuf), ReportError> {
    let substrates =
        required_array(value, "/identity_inputs/compiler_substrates")?;
    let [substrate] = substrates else {
        return Err(reference_error(
            "active Product requires one compiler substrate",
        ));
    };
    let rustc = value
        .pointer("/identity_inputs/rustc/canonical_path")
        .and_then(Value::as_str)
        .ok_or_else(|| identity_error("rustc canonical path is absent"))?;
    let cargo = value
        .pointer("/identity_inputs/cargo/canonical_path")
        .and_then(Value::as_str)
        .ok_or_else(|| identity_error("Cargo canonical path is absent"))?;
    let (host_support, host_target) =
        compiler_host_roots(Path::new(cargo), Path::new(rustc))?;
    for entry in required_field_array(substrate, "entries", "substrate")? {
        if field_string(entry, "kind", "substrate entry")? != "compiler_support"
        {
            continue;
        }
        let destination =
            field_string(entry, "canonical_path", "substrate entry")?;
        let relative = Path::new(destination)
            .strip_prefix(PREPARED_COMPILER_SUPPORT)
            .map_err(|failure| reference_error(failure.to_string()))?;
        let source = host_support.join(relative);
        let source = source.to_str().ok_or_else(|| {
            reference_error("compiler support source path is not UTF-8")
        })?;
        require_report_exposure(
            exposures,
            Some(source),
            destination,
            "file",
            true,
            "compiler support",
            accounted,
        )?;
        require_file_inventory(
            exposures,
            destination,
            field_string(entry, "content_digest", "substrate entry")?,
            field_count(entry, "size_bytes", "substrate entry")?,
        )?;
    }
    Ok((substrate, host_target))
}

fn compiler_host_roots(
    cargo: &Path,
    rustc: &Path,
) -> Result<(PathBuf, PathBuf), ReportError> {
    let bin = rustc.parent().ok_or_else(|| {
        reference_error("rustc canonical path has no bin parent")
    })?;
    let sysroot = bin.parent().ok_or_else(|| {
        reference_error("rustc canonical path has no sysroot parent")
    })?;
    if rustc.file_name().and_then(|value| value.to_str()) != Some("rustc")
        || bin.file_name().and_then(|value| value.to_str()) != Some("bin")
        || cargo.file_name().and_then(|value| value.to_str()) != Some("cargo")
        || cargo.parent() != Some(bin)
    {
        return Err(reference_error(
            "Cargo and rustc do not share one sysroot/bin layout",
        ));
    }
    let support = sysroot.join("lib");
    let target = support.join("rustlib").join(TARGET_TRIPLE).join("lib");
    Ok((support, target))
}

#[allow(clippy::too_many_arguments)]
fn require_report_exposure(
    exposures: &[Value],
    source: Option<&str>,
    destination: &str,
    kind: &str,
    executable: bool,
    purpose: &str,
    accounted: &mut BTreeSet<String>,
) -> Result<(), ReportError> {
    let exposure = exposures
        .iter()
        .find(|entry| entry["destination"] == destination)
        .ok_or_else(|| {
            reference_error(format!(
                "required exposure {destination} is absent"
            ))
        })?;
    let retained_source = field_string(exposure, "source", "exposure")?;
    let flags = string_array(exposure, "flags", "exposure")?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let exact = source.is_none_or(|expected| retained_source == expected)
        && field_string(exposure, "kind", "exposure")? == kind
        && field_string(exposure, "purpose", "exposure")? == purpose
        && flags.contains("noexec") != executable
        && accounted.insert(destination.to_owned());
    if exact {
        return Ok(());
    }
    Err(reference_error(format!(
        "exposure {destination} differs from its authority"
    )))
}

fn exposure_by_destination<'a>(
    exposures: &'a [Value],
    destination: &str,
) -> Result<&'a Value, ReportError> {
    exposures
        .iter()
        .find(|entry| entry["destination"] == destination)
        .ok_or_else(|| {
            reference_error(format!(
                "required exposure {destination} is absent"
            ))
        })
}

fn require_file_inventory(
    exposures: &[Value],
    destination: &str,
    content_digest: &str,
    size_bytes: u64,
) -> Result<(), ReportError> {
    let exposure = exposure_by_destination(exposures, destination)?;
    let identity = report_inventory_identity(exposure)?;
    let [entry] = identity.entries.as_slice() else {
        return Err(reference_error(
            "file exposure inventory does not contain one root entry",
        ));
    };
    if entry.relative_path == Path::new(".")
        && entry.kind == prepared_root::InventoryEntryKind::File
        && entry.content_digest.as_deref() == Some(content_digest)
        && entry.size_bytes == size_bytes
        && identity.total_bytes == size_bytes
    {
        return Ok(());
    }
    Err(reference_error(
        "file exposure inventory differs from retained byte identity",
    ))
}

fn require_target_inventory(
    exposures: &[Value],
    destination: &str,
    substrate: &Value,
) -> Result<(), ReportError> {
    let exposure = exposure_by_destination(exposures, destination)?;
    let identity = report_inventory_identity(exposure)?;
    let root = identity.entries.first().ok_or_else(|| {
        reference_error("target-libdir inventory has no root entry")
    })?;
    if root.relative_path != Path::new(".")
        || root.kind != prepared_root::InventoryEntryKind::Directory
    {
        return Err(reference_error(
            "target-libdir inventory root is not a directory",
        ));
    }
    let mut expected = BTreeMap::new();
    for entry in required_field_array(substrate, "entries", "substrate")? {
        if field_string(entry, "kind", "substrate entry")? != "target_libdir" {
            continue;
        }
        let path = field_string(entry, "canonical_path", "substrate entry")?;
        let relative = Path::new(path)
            .strip_prefix(destination)
            .map_err(|failure| reference_error(failure.to_string()))?;
        expected.insert(
            relative.to_path_buf(),
            (
                field_string(entry, "content_digest", "substrate entry")?
                    .to_owned(),
                field_count(entry, "size_bytes", "substrate entry")?,
            ),
        );
    }
    let observed = identity
        .entries
        .iter()
        .skip(1)
        .map(|entry| {
            let digest = entry.content_digest.clone().ok_or_else(|| {
                reference_error("target-libdir file lacks content identity")
            })?;
            if entry.kind != prepared_root::InventoryEntryKind::File {
                return Err(reference_error(
                    "target-libdir contains a non-file child",
                ));
            }
            Ok((entry.relative_path.clone(), (digest, entry.size_bytes)))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    if !expected.is_empty() && observed == expected {
        return Ok(());
    }
    Err(reference_error(
        "target-libdir inventory differs from compiler substrate bytes",
    ))
}

fn require_null_inventory(exposures: &[Value]) -> Result<(), ReportError> {
    let exposure = exposure_by_destination(exposures, "/dev/null")?;
    let identity = report_inventory_identity(exposure)?;
    let [entry] = identity.entries.as_slice() else {
        return Err(reference_error(
            "null-device inventory does not contain one root entry",
        ));
    };
    if entry.relative_path == Path::new(".")
        && entry.kind == prepared_root::InventoryEntryKind::CharacterDevice
        && entry.device_identity == Some(259)
        && identity.total_bytes == 0
    {
        return Ok(());
    }
    Err(reference_error(
        "null-device inventory differs from Linux character device 1:3",
    ))
}

fn account_runtime_exposures(
    exposures: &[Value],
    accounted: &mut BTreeSet<String>,
) -> Result<(), ReportError> {
    for exposure in exposures {
        if exposure["purpose"] != "system runtime" {
            continue;
        }
        let destination =
            field_string(exposure, "destination", "runtime exposure")?;
        let flags = string_array(exposure, "flags", "runtime exposure")?
            .into_iter()
            .collect::<BTreeSet<_>>();
        if field_string(exposure, "kind", "runtime exposure")? != "file"
            || flags.contains("noexec")
            || !accounted.insert(destination.to_owned())
        {
            return Err(reference_error(
                "system-runtime exposure has the wrong shape",
            ));
        }
    }
    Ok(())
}

fn validate_exposure_kind(
    entry: &Value,
    source: &str,
    destination: &str,
) -> Result<u64, ReportError> {
    let context = "Product exposure";
    if field_string(entry, "filesystem", context)? != "bind" {
        return Err(reference_error("Product exposure is not a bind"));
    }
    let flags = string_array(entry, "flags", context)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    match field_string(entry, "kind", context)? {
        "character_device" => validate_null_device(source, destination, &flags),
        "file" | "directory" => {
            validate_regular_exposure(source, destination, &flags)
        }
        _ => Err(reference_error("Product exposure kind is unavailable")),
    }
}

fn validate_null_device(
    source: &str,
    destination: &str,
    flags: &BTreeSet<String>,
) -> Result<u64, ReportError> {
    let expected = ["noexec", "nosuid", "ro"]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if source == "/dev/null" && destination == "/dev/null" && flags == &expected
    {
        Ok(1)
    } else {
        Err(reference_error("/dev/null exposure identity or flags differ"))
    }
}

fn validate_regular_exposure(
    source: &str,
    destination: &str,
    flags: &BTreeSet<String>,
) -> Result<u64, ReportError> {
    let required = ["nodev", "nosuid", "ro"]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let allowed = ["nodev", "noexec", "nosuid", "ro"]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if source != "/dev/null"
        && destination != "/dev/null"
        && required.is_subset(flags)
        && flags.is_subset(&allowed)
    {
        Ok(0)
    } else {
        Err(reference_error("regular Product exposure flags or paths differ"))
    }
}

fn validate_execution_network(execution: &Value) -> Result<(), ReportError> {
    let network = object_field(execution, "network", "Product execution")?;
    let interfaces = string_array(network, "interfaces", "network evidence")?
        .into_iter()
        .collect::<BTreeSet<_>>();
    if field_count(network, "socket_count", "network evidence")? != 0
        || field_count(network, "kernel_route_count", "network evidence")? != 2
        || !field_bool(network, "external_carrier_absent", "network evidence")?
        || !string_array(network, "routes", "network evidence")?.is_empty()
        || interfaces != BTreeSet::from(["lo".to_owned()])
        || field_string(
            network,
            "policy_observation_digest",
            "network evidence",
        )? != crate::digest::bytes(NETWORK_POLICY_DOMAIN)
        || field_string(network, "policy_basis", "network evidence")?
            != NETWORK_POLICY_BASIS
    {
        return Err(reference_error(
            "Product execution retains network authority",
        ));
    }
    Ok(())
}

fn validate_execution_scratch(
    value: &Value,
    execution: &Value,
) -> Result<(), ReportError> {
    let scratch = object_field(execution, "scratch", "Product execution")?;
    enforce_product_path_limit(
        value,
        field_string(scratch, "mountpoint", "scratch evidence")?,
        "scratch mountpoint",
    )?;
    let flags = string_array(scratch, "flags", "scratch evidence")?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let required_flags = ["mode=0700", "nodev", "noexec", "nosuid"]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if flags != required_flags {
        return Err(reference_error(
            "scratch flags differ from the closed prepared-root profile",
        ));
    }
    let retained = [
        ("size_limit_bytes", "scratch_bytes"),
        ("inode_limit", "scratch_inodes"),
    ];
    for (field, limit) in retained {
        if field_count(scratch, field, "scratch evidence")?
            != required_count(value, "/limits/product_execution", limit)?
        {
            return Err(limit_error(
                "scratch hard limit differs from retained topology",
            ));
        }
    }
    let allocated = field_count(scratch, "allocated_bytes", "scratch")?;
    let inodes = field_count(scratch, "inode_count", "scratch")?;
    let logical = field_count(scratch, "logical_bytes", "scratch")?;
    let logical_limit = required_count(
        value,
        "/limits/product_execution",
        "process_output_file_bytes",
    )?;
    if allocated > field_count(scratch, "size_limit_bytes", "scratch")?
        || inodes > field_count(scratch, "inode_limit", "scratch")?
        || logical > logical_limit
    {
        return Err(limit_error("scratch use exceeds its hard bound"));
    }
    Ok(())
}

fn validate_active_closures(
    value: &Value,
    product: &Value,
    graph: &GraphReferences,
    execution: &str,
) -> Result<ActiveClosureReferences, ReportError> {
    let values = required_field_array(product, "closures", "product_proof")?;
    if values.len() != 2 {
        return Err(reference_error(
            "active Foundation proof requires exactly two closures",
        ));
    }
    let mut closures = BTreeMap::new();
    for closure in values {
        let id = field_string(closure, "id", "closure evidence")?;
        if closures.insert(id.to_owned(), closure).is_some() {
            return Err(reference_error("duplicate Product closure evidence"));
        }
    }
    let shipping = closures.get(SHIPPING_CLOSURE).ok_or_else(|| {
        reference_error("Foundation shipping closure evidence is absent")
    })?;
    let consumer = closures.get(CONSUMER_CLOSURE).ok_or_else(|| {
        reference_error("Foundation consumer closure evidence is absent")
    })?;
    validate_closure_identity(
        shipping,
        "shipping",
        SHIPPING_PACKAGE,
        SHIPPING_TARGET,
        execution,
        graph,
    )?;
    validate_closure_identity(
        consumer,
        "validation",
        CONSUMER_PACKAGE,
        CONSUMER_TARGET,
        execution,
        graph,
    )?;
    let shipping_units = validate_cargo_capture(
        value,
        shipping,
        SHIPPING_CARGO_NAME,
        graph,
        true,
    )?;
    let consumer_units = validate_cargo_capture(
        value,
        consumer,
        CONSUMER_CARGO_NAME,
        graph,
        false,
    )?;
    if shipping_units.compiled.len() != 1
        || !shipping_units.fresh.is_empty()
        || consumer_units.compiled.len() != 1
        || consumer_units.fresh != shipping_units.compiled
    {
        return Err(reference_error(
            "Cargo captures disagree on compiled/fresh Foundation units",
        ));
    }
    Ok(ActiveClosureReferences {
        execution: execution.to_owned(),
        shipping_unit: shipping_units.compiled.into_iter().next().unwrap(),
        consumer_unit: consumer_units.compiled.into_iter().next().unwrap(),
        shipping_events: shipping_units.events,
        consumer_events: consumer_units.events,
    })
}

fn validate_closure_identity(
    closure: &Value,
    kind: &str,
    package: &str,
    target: &str,
    execution: &str,
    graph: &GraphReferences,
) -> Result<(), ReportError> {
    let context = "closure evidence";
    if field_string(closure, "kind", context)? != kind
        || field_string(closure, "workspace", context)? != PRODUCT_WORKSPACE
        || field_string(closure, "package", context)? != package
        || field_string(closure, "target", context)? != target
        || field_string(closure, "execution", context)? != execution
        || field_string(closure, "status", context)? != "pass"
    {
        return Err(reference_error(
            "Product closure identity or status differs from declaration",
        ));
    }
    let details = graph
        .packages
        .get(&(PRODUCT_WORKSPACE.to_owned(), package.to_owned()))
        .ok_or_else(|| reference_error("closure package is absent"))?;
    let target = details.targets.get(target).ok_or_else(|| {
        reference_error("closure target is absent from Cargo graph")
    })?;
    if target.kind != ["lib"] || target.crate_types != ["lib"] {
        return Err(reference_error(
            "closure target is not the admitted lib target",
        ));
    }
    Ok(())
}

fn validate_cargo_capture(
    value: &Value,
    closure: &Value,
    package: &str,
    graph: &GraphReferences,
    shipping: bool,
) -> Result<ActiveCargoCaptureRef, ReportError> {
    let capture = object_field(closure, "cargo_capture", "closure")?;
    let argv = string_array(capture, "command_argv", "Cargo capture")?;
    validate_product_build_argv(value, &argv, package, graph)?;
    let bytes = field_count(capture, "stdout_bytes", "Cargo capture")?;
    let messages = field_count(capture, "message_count", "Cargo capture")?;
    if bytes == 0
        || bytes
            > required_count(
                value,
                "/limits/product_execution",
                "cargo_json_bytes",
            )?
        || messages == 0
        || messages
            > required_count(
                value,
                "/limits/product_execution",
                "cargo_message_count",
            )?
    {
        return Err(limit_error("Cargo capture exceeds its retained bounds"));
    }
    let compiled = string_array(capture, "compiled_units", "Cargo capture")?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let fresh = string_array(capture, "fresh_provider_units", "Cargo capture")?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let events = validate_cargo_artifact_events(value, capture)?;
    let expected_units =
        compiled.union(&fresh).cloned().collect::<BTreeSet<_>>();
    let event_units = events.keys().cloned().collect::<BTreeSet<_>>();
    let flags_match =
        events.iter().all(|(unit, event)| event.fresh == fresh.contains(unit));
    if event_units != expected_units
        || !flags_match
        || messages
            != u64::try_from(events.len() + 1)
                .map_err(|_| limit_error("Cargo message count overflow"))?
    {
        return Err(reference_error(
            "Cargo artifact events differ from compiled/fresh units",
        ));
    }
    validate_product_probes(value, capture, shipping)?;
    Ok(ActiveCargoCaptureRef { compiled, fresh, events })
}

fn validate_cargo_artifact_events(
    value: &Value,
    capture: &Value,
) -> Result<BTreeMap<String, ActiveCargoEventRef>, ReportError> {
    let entries =
        required_field_array(capture, "artifact_events", "Cargo capture")?;
    let mut events = BTreeMap::new();
    for entry in entries {
        let context = "Cargo artifact event";
        let unit = field_string(entry, "unit", context)?.to_owned();
        let filenames = string_array(entry, "filenames", context)?;
        if filenames.len() != 2 {
            return Err(reference_error(
                "Cargo artifact event requires rlib and rmeta filenames",
            ));
        }
        let mut retained = BTreeSet::new();
        let mut extensions = BTreeSet::new();
        for path in filenames {
            retain_cargo_artifact_filename(
                value,
                &path,
                &mut retained,
                &mut extensions,
            )?;
        }
        if extensions != BTreeSet::from(["rlib".to_owned(), "rmeta".to_owned()])
            || events
                .insert(
                    unit,
                    ActiveCargoEventRef {
                        fresh: field_bool(entry, "fresh", context)?,
                        filenames: retained,
                    },
                )
                .is_some()
        {
            return Err(reference_error(
                "Cargo artifact event has wrong extensions or duplicate unit",
            ));
        }
    }
    Ok(events)
}

fn retain_cargo_artifact_filename(
    value: &Value,
    path: &str,
    retained: &mut BTreeSet<String>,
    extensions: &mut BTreeSet<String>,
) -> Result<(), ReportError> {
    if !normalized_file_path(path)
        || !Path::new(&path).starts_with("/scratch")
        || !retained.insert(path.to_owned())
    {
        return Err(reference_error(
            "Cargo artifact filename is invalid or duplicated",
        ));
    }
    enforce_product_path_limit(value, path, "Cargo artifact")?;
    let extension = Path::new(&path)
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            reference_error("Cargo artifact filename has no extension")
        })?;
    extensions.insert(extension.to_owned());
    Ok(())
}

fn validate_product_probes(
    value: &Value,
    capture: &Value,
    shipping: bool,
) -> Result<(), ReportError> {
    let probes = required_field_array(capture, "probes", "Cargo capture")?;
    let expected: &[&str] = if shipping {
        &["version_probe", "host_probe", "target_probe", "version_probe"]
    } else {
        &[]
    };
    let kinds = probes
        .iter()
        .map(|probe| field_string(probe, "kind", "compiler probe"))
        .collect::<Result<Vec<_>, _>>()?;
    if kinds != expected {
        return Err(reference_error(
            "compiler discovery-probe sequence differs from Cargo 1.92",
        ));
    }
    for probe in probes {
        validate_product_probe(value, probe)?;
    }
    Ok(())
}

fn validate_product_probe(
    value: &Value,
    probe: &Value,
) -> Result<(), ReportError> {
    let context = "compiler probe";
    let kind = field_string(probe, "kind", context)?;
    let original = string_array(probe, "original_argv", context)?;
    let effective = string_array(probe, "effective_argv", context)?;
    let expected = expected_probe_argv(PREPARED_RUSTC, kind)?;
    let mut expected_effective = expected.clone();
    expected_effective.extend(["--sysroot".to_owned(), "/sysroot".to_owned()]);
    if original != expected
        || effective != expected_effective
        || field_count(probe, "status", context)? != 0
    {
        return Err(reference_error(
            "compiler discovery probe differs from its exact grammar",
        ));
    }
    validate_wrapper_argv_limits(value, &original, "probe original argv")?;
    validate_wrapper_argv_limits(value, &effective, "probe effective argv")
}

fn expected_probe_argv(
    rustc: &str,
    kind: &str,
) -> Result<Vec<String>, ReportError> {
    if kind == "version_probe" {
        return Ok(vec![rustc.to_owned(), "-vV".to_owned()]);
    }
    if !matches!(kind, "host_probe" | "target_probe") {
        return Err(reference_error("compiler probe kind is unavailable"));
    }
    let mut argv = vec![
        rustc.to_owned(),
        "-".to_owned(),
        "--crate-name".to_owned(),
        "___".to_owned(),
        "--print=file-names".to_owned(),
    ];
    if kind == "target_probe" {
        argv.extend(["--target".to_owned(), TARGET_TRIPLE.to_owned()]);
    }
    argv.extend(
        [
            "--crate-type",
            "bin",
            "--crate-type",
            "rlib",
            "--crate-type",
            "dylib",
            "--crate-type",
            "cdylib",
            "--crate-type",
            "staticlib",
            "--crate-type",
            "proc-macro",
            "--print=sysroot",
            "--print=split-debuginfo",
            "--print=crate-name",
            "--print=cfg",
            "-Wwarnings",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    Ok(argv)
}

fn validate_product_build_argv(
    value: &Value,
    argv: &[String],
    package: &str,
    _graph: &GraphReferences,
) -> Result<(), ReportError> {
    let Some(target_dir) = option_value(argv, "--target-dir") else {
        return Err(reference_error("Product build omits --target-dir"));
    };
    if !normalized_file_path(target_dir) {
        return Err(reference_error(
            "Product target directory is not normalized absolute text",
        ));
    }
    enforce_product_path_limit(value, target_dir, "Product target directory")?;
    let expected = [
        PREPARED_CARGO,
        "build",
        "--quiet",
        "--locked",
        "--offline",
        "--manifest-path",
        PREPARED_MANIFEST,
        "--package",
        package,
        "--lib",
        "--target",
        TARGET_TRIPLE,
        "--profile",
        PRODUCT_PROFILE,
        "--jobs",
        "1",
        "--message-format=json",
        "--target-dir",
        target_dir,
    ];
    if argv.iter().map(String::as_str).ne(expected) {
        return Err(reference_error(
            "Product Cargo build argv differs from the closed profile",
        ));
    }
    Ok(())
}

fn option_value<'a>(argv: &'a [String], option: &str) -> Option<&'a str> {
    argv.windows(2)
        .find_map(|pair| (pair[0] == option).then_some(pair[1].as_str()))
}

fn validate_active_units(
    value: &Value,
    product: &Value,
    graph: &GraphReferences,
    closures: &ActiveClosureReferences,
) -> Result<BTreeMap<String, ActiveUnitRef>, ReportError> {
    let values =
        required_field_array(product, "compiler_units", "product_proof")?;
    let count = u64::try_from(values.len())
        .map_err(|_| limit_error("compiler unit count overflow"))?;
    let limit = required_count(
        value,
        "/limits/product_execution",
        "compiler_unit_count",
    )?;
    if count > limit {
        return Err(limit_error("compiler units exceed their retained bound"));
    }
    if values.len() != 2 {
        return Err(reference_error(
            "active Foundation proof requires two compiler units",
        ));
    }
    let mut units = BTreeMap::new();
    let mut dep_info = BTreeSet::new();
    let mut dep_info_total = 0_u64;
    let mut trace_total = 0_u64;
    let consumer_extern = active_consumer_extern_path(product, closures)?;
    for unit in values {
        let id = field_string(unit, "id", "compiler unit")?;
        if units.contains_key(id) {
            return Err(reference_error("duplicate compiler unit identity"));
        }
        let retained = validate_active_unit(
            value,
            unit,
            graph,
            closures,
            &consumer_extern,
            &mut dep_info,
        )?;
        dep_info_total = checked_field_sum(
            dep_info_total,
            unit,
            "dep_info_bytes",
            "dep-info",
        )?;
        trace_total = checked_field_sum(
            trace_total,
            unit,
            "trace_bytes",
            "wrapper trace",
        )?;
        units.insert(id.to_owned(), retained);
    }
    validate_unit_aggregate_limits(value, dep_info_total, trace_total)?;
    Ok(units)
}

fn active_consumer_extern_path(
    product: &Value,
    closures: &ActiveClosureReferences,
) -> Result<String, ReportError> {
    let edges = required_field_array(product, "extern_edges", "product")?;
    let [edge] = edges else {
        return Err(reference_error(
            "active Foundation proof requires one Product extern edge",
        ));
    };
    let provider = object_field(edge, "provider", "Product extern edge")?;
    if field_string(edge, "closure", "Product extern edge")? == CONSUMER_CLOSURE
        && field_string(edge, "unit", "Product extern edge")?
            == closures.consumer_unit
        && field_string(edge, "extern_name", "Product extern edge")?
            == "reovim_foundation"
        && field_string(provider, "unit", "Product extern edge")?
            == closures.shipping_unit
    {
        return Ok(
            field_string(provider, "path", "Product extern edge")?.to_owned()
        );
    }
    Err(reference_error(
        "Product extern edge differs from the closed compiler-unit pair",
    ))
}

fn validate_active_unit(
    value: &Value,
    unit: &Value,
    graph: &GraphReferences,
    closures: &ActiveClosureReferences,
    consumer_extern: &str,
    dep_info_paths: &mut BTreeSet<String>,
) -> Result<ActiveUnitRef, ReportError> {
    let context = "compiler unit";
    let id = field_string(unit, "id", context)?;
    let (closure, package, target, crate_name) = if id == closures.shipping_unit
    {
        (
            SHIPPING_CLOSURE,
            SHIPPING_PACKAGE,
            SHIPPING_TARGET,
            "reovim_foundation",
        )
    } else if id == closures.consumer_unit {
        (
            CONSUMER_CLOSURE,
            CONSUMER_PACKAGE,
            CONSUMER_TARGET,
            "reovim_foundation_public_consumer",
        )
    } else {
        return Err(reference_error("compiler unit is not Cargo-produced"));
    };
    if field_string(unit, "closure", context)? != closure
        || field_string(unit, "execution", context)? != closures.execution
        || field_string(unit, "workspace", context)? != PRODUCT_WORKSPACE
        || field_string(unit, "package", context)? != package
        || field_string(unit, "target", context)? != target
        || field_string(unit, "crate_name", context)? != crate_name
        || field_string(unit, "target_triple", context)? != TARGET_TRIPLE
        || field_string(unit, "profile", context)? != PRODUCT_PROFILE
        || field_count(unit, "status", context)? != 0
    {
        return Err(reference_error(
            "compiler unit identity differs from its closure",
        ));
    }
    if string_array(unit, "crate_types", context)? != ["lib"]
        || string_array(unit, "features", context)? != ["default"]
        || string_array(unit, "cfgs", context)? != ["feature=\"default\""]
    {
        return Err(reference_error(
            "compiler unit crate type, features, or cfgs differ",
        ));
    }
    require_package(
        &graph_package_ids(graph),
        PRODUCT_WORKSPACE,
        PRODUCT_PROFILE,
        package,
        context,
    )?;
    let extern_path =
        (closure == CONSUMER_CLOSURE).then_some(Path::new(consumer_extern));
    validate_unit_paths_and_argv(
        value,
        unit,
        crate_name,
        extern_path,
        dep_info_paths,
    )?;
    Ok(ActiveUnitRef { closure: closure.to_owned() })
}

fn graph_package_ids(graph: &GraphReferences) -> PackageReferences {
    graph
        .packages
        .keys()
        .map(|(workspace, package)| {
            (workspace.clone(), PRODUCT_PROFILE.to_owned(), package.clone())
        })
        .collect()
}

fn validate_unit_paths_and_argv(
    value: &Value,
    unit: &Value,
    crate_name: &str,
    extern_path: Option<&Path>,
    dep_info_paths: &mut BTreeSet<String>,
) -> Result<(), ReportError> {
    let context = "compiler unit";
    let path = field_string(unit, "dep_info_path", context)?;
    if !normalized_file_path(path) || !dep_info_paths.insert(path.to_owned()) {
        return Err(reference_error(
            "compiler dep-info path is invalid or duplicated",
        ));
    }
    enforce_product_path_limit(value, path, "compiler dep-info")?;
    for field in ["original_argv", "effective_argv"] {
        let argv = string_array(unit, field, context)?;
        validate_wrapper_argv_limits(value, &argv, field)?;
    }
    let original = string_array(unit, "original_argv", context)?;
    let effective = string_array(unit, "effective_argv", context)?;
    compiler_capture::validate_reported_unit_argv(
        &original,
        &effective,
        Path::new(path),
        crate_name,
        extern_path,
    )
    .map_err(|failure| reference_error(failure.to_string()))
}

fn validate_wrapper_argv_limits(
    value: &Value,
    argv: &[String],
    context: &str,
) -> Result<(), ReportError> {
    let count = u64::try_from(argv.len())
        .map_err(|_| limit_error("wrapper argv item count overflow"))?;
    let bytes = argv.iter().try_fold(0_u64, |total, argument| {
        let length = u64::try_from(argument.len())
            .map_err(|_| limit_error("wrapper argv byte count overflow"))?;
        total
            .checked_add(length)
            .ok_or_else(|| limit_error("wrapper argv byte count overflow"))
    })?;
    let prefix = "/limits/product_execution";
    if count > required_count(value, prefix, "wrapper_argv_items")?
        || bytes > required_count(value, prefix, "wrapper_argv_bytes")?
    {
        return Err(limit_error(format!(
            "{context} exceeds its retained wrapper argv bound"
        )));
    }
    Ok(())
}

fn checked_field_sum(
    total: u64,
    value: &Value,
    field: &str,
    context: &str,
) -> Result<u64, ReportError> {
    total
        .checked_add(field_count(value, field, context)?)
        .ok_or_else(|| limit_error(format!("{context} aggregate overflow")))
}

fn validate_unit_aggregate_limits(
    value: &Value,
    dep_info: u64,
    trace: u64,
) -> Result<(), ReportError> {
    let prefix = "/limits/product_execution";
    let dep_total = required_count(value, prefix, "dep_info_total_bytes")?;
    let trace_total = required_count(value, prefix, "wrapper_trace_bytes")?;
    let per_file = required_count(value, prefix, "dep_info_file_bytes")?;
    if dep_info == 0
        || dep_info > dep_total
        || trace == 0
        || trace > trace_total
    {
        return Err(limit_error(
            "compiler capture aggregate exceeds its retained bound",
        ));
    }
    for unit in required_array(value, "/product_proof/compiler_units")? {
        if field_count(unit, "dep_info_bytes", "compiler unit")? > per_file {
            return Err(limit_error(
                "compiler dep-info exceeds per-file bound",
            ));
        }
    }
    Ok(())
}

fn validate_active_trace_total(
    value: &Value,
    product: &Value,
) -> Result<(), ReportError> {
    let unit_bytes =
        required_field_array(product, "compiler_units", "product")?
            .iter()
            .map(|unit| field_count(unit, "trace_bytes", "compiler unit"));
    let probe_bytes = required_field_array(product, "closures", "product")?
        .iter()
        .map(|closure| object_field(closure, "cargo_capture", "closure"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|capture| required_field_array(capture, "probes", "capture"))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .map(|probe| field_count(probe, "trace_bytes", "compiler probe"));
    let total =
        unit_bytes.chain(probe_bytes).try_fold(0_u64, |sum, bytes| {
            sum.checked_add(bytes?).ok_or_else(|| {
                limit_error("compiler trace aggregate byte count overflow")
            })
        })?;
    let limit = required_count(
        value,
        "/limits/product_execution",
        "wrapper_trace_bytes",
    )?;
    if total > limit {
        return Err(limit_error(
            "compiler unit and probe traces exceed their aggregate bound",
        ));
    }
    Ok(())
}

fn validate_active_inputs(
    value: &Value,
    product: &Value,
    source: &SourceAuthority,
    units: &BTreeMap<String, ActiveUnitRef>,
) -> Result<(), ReportError> {
    let inputs =
        required_field_array(product, "compiler_inputs", "product_proof")?;
    let limit = required_count(
        value,
        "/limits/product_execution",
        "compiler_input_count",
    )?;
    let count = u64::try_from(inputs.len())
        .map_err(|_| limit_error("compiler input count overflow"))?;
    if count > limit {
        return Err(limit_error("compiler inputs exceed their bound"));
    }
    let shipping_unit = unit_id_for_closure(units, SHIPPING_CLOSURE)?;
    let consumer_unit = unit_id_for_closure(units, CONSUMER_CLOSURE)?;
    let expected = expected_active_inputs(shipping_unit, consumer_unit);
    let mut observed = BTreeSet::new();
    for input in inputs {
        validate_active_input(input, source, units, &mut observed)?;
    }
    if observed != expected {
        return Err(reference_error(
            "compiler inputs do not exactly cover Foundation source",
        ));
    }
    Ok(())
}

fn unit_id_for_closure<'a>(
    units: &'a BTreeMap<String, ActiveUnitRef>,
    closure: &str,
) -> Result<&'a str, ReportError> {
    units
        .iter()
        .find_map(|(id, unit)| (unit.closure == closure).then_some(id.as_str()))
        .ok_or_else(|| reference_error("closure compiler unit is absent"))
}

fn expected_active_inputs(
    shipping_unit: &str,
    consumer_unit: &str,
) -> BTreeSet<(String, String, String, String)> {
    let mut expected = BTreeSet::new();
    for (kind, path) in [
        ("crate_root", "kernel/foundation/src/lib.rs"),
        ("module", "kernel/foundation/src/bound.rs"),
        ("module", "kernel/foundation/src/carrier.rs"),
        ("module", "kernel/foundation/src/contract.rs"),
        ("module", "kernel/foundation/src/identity.rs"),
    ] {
        expected.insert((
            kind.to_owned(),
            SHIPPING_CLOSURE.to_owned(),
            shipping_unit.to_owned(),
            path.to_owned(),
        ));
    }
    expected.insert((
        "crate_root".to_owned(),
        CONSUMER_CLOSURE.to_owned(),
        consumer_unit.to_owned(),
        "tests/foundation-public-consumer/src/lib.rs".to_owned(),
    ));
    expected
}

fn validate_active_input(
    input: &Value,
    source: &SourceAuthority,
    units: &BTreeMap<String, ActiveUnitRef>,
    observed: &mut BTreeSet<(String, String, String, String)>,
) -> Result<(), ReportError> {
    let context = "compiler input";
    let kind = field_string(input, "kind", context)?;
    if !matches!(kind, "crate_root" | "module") {
        return Err(reference_error(
            "authored-std-lib profile contains an unavailable compiler input",
        ));
    }
    let closure = field_string(input, "closure", context)?;
    let unit = field_string(input, "unit", context)?;
    let path = field_string(input, "path", context)?;
    let unit_ref = units.get(unit).ok_or_else(|| {
        reference_error("compiler input references an unknown unit")
    })?;
    if unit_ref.closure != closure {
        return Err(reference_error(
            "compiler input closure differs from its unit",
        ));
    }
    validate_input_source(input, source, path, unit_ref)?;
    let key =
        (kind.to_owned(), closure.to_owned(), unit.to_owned(), path.to_owned());
    if !observed.insert(key) {
        return Err(reference_error("duplicate compiler input identity"));
    }
    Ok(())
}

fn validate_input_source(
    input: &Value,
    source: &SourceAuthority,
    path: &str,
    unit: &ActiveUnitRef,
) -> Result<(), ReportError> {
    let path_buf = parse_relative_path(path, "compiler input")?;
    let retained = source.files.get(&path_buf).ok_or_else(|| {
        reference_error("compiler input is absent from source authority")
    })?;
    let expected_owner = if unit.closure == SHIPPING_CLOSURE {
        SourceOwner::Product(ArchitecturalRole::Foundation)
    } else {
        SourceOwner::Product(ArchitecturalRole::KernelInterface)
    };
    let expected_use = if unit.closure == SHIPPING_CLOSURE {
        SourceUse::Production
    } else {
        SourceUse::Test
    };
    if retained.owner != expected_owner
        || retained.provenance != SourceProvenance::Authored
        || retained.source_use != expected_use
        || retained.content_digest
            != field_string(input, "content_digest", "compiler input")?
        || retained.size_bytes
            != field_count(input, "size_bytes", "compiler input")?
    {
        return Err(reference_error(
            "compiler input differs from classified Product source",
        ));
    }
    Ok(())
}

fn validate_active_artifacts(
    value: &Value,
    product: &Value,
    units: &BTreeMap<String, ActiveUnitRef>,
) -> Result<BTreeMap<(String, String), ActiveArtifactRef>, ReportError> {
    let values = required_field_array(product, "artifacts", "product_proof")?;
    let count = u64::try_from(values.len())
        .map_err(|_| limit_error("artifact count overflow"))?;
    let count_limit =
        required_count(value, "/limits/product_execution", "artifact_count")?;
    if count > count_limit {
        return Err(limit_error("Product artifacts exceed their count bound"));
    }
    if values.len() != 4 {
        return Err(reference_error(
            "active Foundation proof requires rlib and rmeta per unit",
        ));
    }
    let mut artifacts = BTreeMap::new();
    let mut paths = BTreeSet::new();
    let mut total = 0_u64;
    for artifact in values {
        let context = "Product artifact";
        let unit_id = field_string(artifact, "unit", context)?;
        let unit = units.get(unit_id).ok_or_else(|| {
            reference_error("artifact references an unknown compiler unit")
        })?;
        let closure = field_string(artifact, "closure", context)?;
        let path = field_string(artifact, "path", context)?;
        let size = field_count(artifact, "size_bytes", context)?;
        let crate_type = field_string(artifact, "crate_type", context)?;
        if closure != unit.closure
            || !matches!(crate_type, "rlib" | "rmeta")
            || !normalized_file_path(path)
            || !paths.insert(path.to_owned())
            || size == 0
        {
            return Err(reference_error(
                "Product artifact identity differs from its compiler unit",
            ));
        }
        enforce_product_path_limit(value, path, "Product artifact")?;
        total = total
            .checked_add(size)
            .ok_or_else(|| limit_error("artifact byte count overflow"))?;
        let retained = ActiveArtifactRef {
            closure: closure.to_owned(),
            path: path.to_owned(),
            content_digest: field_string(artifact, "content_digest", context)?
                .to_owned(),
            size_bytes: size,
        };
        let key = (unit_id.to_owned(), crate_type.to_owned());
        if artifacts.insert(key, retained).is_some() {
            return Err(reference_error(
                "compiler unit repeats one retained artifact kind",
            ));
        }
    }
    let limit = required_count(
        value,
        "/limits/product_execution",
        "artifact_total_bytes",
    )?;
    if total > limit {
        return Err(limit_error("Product artifact bytes exceed their bound"));
    }
    let expected = artifact_coverage_keys(units.keys());
    if artifacts.keys().cloned().collect::<BTreeSet<_>>() != expected {
        return Err(reference_error(
            "compiler unit lacks rlib or rmeta artifact coverage",
        ));
    }
    Ok(artifacts)
}

fn artifact_coverage_keys<'a>(
    units: impl Iterator<Item = &'a String>,
) -> BTreeSet<(String, String)> {
    units
        .flat_map(|unit| {
            ["rlib", "rmeta"]
                .into_iter()
                .map(|kind| (unit.clone(), kind.to_owned()))
        })
        .collect()
}

fn validate_active_provider_artifact(
    value: &Value,
    product: &Value,
    closures: &ActiveClosureReferences,
    artifacts: &BTreeMap<(String, String), ActiveArtifactRef>,
) -> Result<(), ReportError> {
    let provider = object_field(product, "provider_artifact", "product")?;
    if field_string(provider, "unit", "provider artifact")?
        != closures.shipping_unit
    {
        return Err(reference_error(
            "provider artifact names the wrong shipping unit",
        ));
    }
    let top = provider_file_identity(value, provider, "top_level_rlib")?;
    let hashed = provider_file_identity(value, provider, "hashed_rlib")?;
    let rmeta = provider_file_identity(value, provider, "rmeta")?;
    let shipping_rlib = artifacts
        .get(&(closures.shipping_unit.clone(), "rlib".to_owned()))
        .ok_or_else(|| reference_error("shipping rlib is absent"))?;
    let shipping_rmeta = artifacts
        .get(&(closures.shipping_unit.clone(), "rmeta".to_owned()))
        .ok_or_else(|| reference_error("shipping rmeta is absent"))?;
    require_active_provider_identity(&hashed, shipping_rlib, "hashed rlib")?;
    require_active_provider_identity(&rmeta, shipping_rmeta, "rmeta")?;
    if top.path == hashed.path
        || top.content_digest != hashed.content_digest
        || top.size_bytes != hashed.size_bytes
        || Path::new(&top.path).extension().and_then(|value| value.to_str())
            != Some("rlib")
    {
        return Err(reference_error(
            "top-level and hashed provider rlib relation differs",
        ));
    }
    let shipping_files = BTreeSet::from([top.path, rmeta.path.clone()]);
    require_active_cargo_event(
        &closures.shipping_events,
        &closures.shipping_unit,
        false,
        &shipping_files,
    )?;
    let fresh_files = BTreeSet::from([hashed.path, rmeta.path]);
    require_active_cargo_event(
        &closures.consumer_events,
        &closures.shipping_unit,
        true,
        &fresh_files,
    )?;
    let consumer = closures
        .consumer_events
        .get(&closures.consumer_unit)
        .ok_or_else(|| reference_error("consumer Cargo artifact is absent"))?;
    let consumer_rmeta = artifacts
        .get(&(closures.consumer_unit.clone(), "rmeta".to_owned()))
        .ok_or_else(|| reference_error("consumer rmeta is absent"))?;
    if !consumer.fresh
        && consumer.filenames.contains(&consumer_rmeta.path)
        && closures.shipping_events.len() == 1
        && closures.consumer_events.len() == 2
    {
        return Ok(());
    }
    Err(reference_error("consumer Cargo artifact differs from wrapper output"))
}

#[derive(Clone)]
struct ProviderFileRef {
    path: String,
    content_digest: String,
    size_bytes: u64,
}

fn provider_file_identity(
    value: &Value,
    provider: &Value,
    field: &str,
) -> Result<ProviderFileRef, ReportError> {
    let identity = object_field(provider, field, "provider artifact")?;
    let path = field_string(identity, "path", "provider file")?;
    if !normalized_file_path(path) || !Path::new(path).starts_with("/scratch") {
        return Err(reference_error(
            "provider artifact path is outside scratch",
        ));
    }
    enforce_product_path_limit(value, path, "provider artifact")?;
    Ok(ProviderFileRef {
        path: path.to_owned(),
        content_digest: field_string(
            identity,
            "content_digest",
            "provider file",
        )?
        .to_owned(),
        size_bytes: field_count(identity, "size_bytes", "provider file")?,
    })
}

fn require_active_provider_identity(
    provider: &ProviderFileRef,
    artifact: &ActiveArtifactRef,
    context: &str,
) -> Result<(), ReportError> {
    if provider.path == artifact.path
        && provider.content_digest == artifact.content_digest
        && provider.size_bytes == artifact.size_bytes
    {
        return Ok(());
    }
    Err(reference_error(format!(
        "provider {context} differs from wrapper artifact identity"
    )))
}

fn require_active_cargo_event(
    events: &BTreeMap<String, ActiveCargoEventRef>,
    unit: &str,
    fresh: bool,
    filenames: &BTreeSet<String>,
) -> Result<(), ReportError> {
    let event = events.get(unit).ok_or_else(|| {
        reference_error("Cargo provider artifact event is absent")
    })?;
    if event.fresh == fresh && event.filenames == *filenames {
        Ok(())
    } else {
        Err(reference_error(
            "Cargo provider artifact event differs from retained bytes",
        ))
    }
}

fn validate_active_extern_edges(
    product: &Value,
    closures: &ActiveClosureReferences,
    units: &BTreeMap<String, ActiveUnitRef>,
    artifacts: &BTreeMap<(String, String), ActiveArtifactRef>,
) -> Result<(), ReportError> {
    let edges = required_field_array(product, "extern_edges", "product")?;
    if edges.len() != 1 {
        return Err(reference_error(
            "active Foundation proof requires one Product extern edge",
        ));
    }
    let edge = &edges[0];
    let context = "Product extern edge";
    let provider = object_field(edge, "provider", context)?;
    let artifact = artifacts
        .get(&(closures.shipping_unit.clone(), "rmeta".to_owned()))
        .ok_or_else(|| {
            reference_error("shipping rmeta is absent from extern authority")
        })?;
    if field_string(edge, "closure", context)? != CONSUMER_CLOSURE
        || field_string(edge, "unit", context)? != closures.consumer_unit
        || field_string(edge, "extern_name", context)? != "reovim_foundation"
        || field_string(edge, "decision", context)? != "admitted_product_unit"
        || field_string(provider, "kind", context)? != "compiler_unit"
        || field_string(provider, "unit", context)? != closures.shipping_unit
        || field_string(provider, "path", context)? != artifact.path
        || artifact.closure != SHIPPING_CLOSURE
        || !units.contains_key(&closures.consumer_unit)
    {
        return Err(reference_error(
            "Product extern edge differs from the public-consumer edge",
        ));
    }
    Ok(())
}

fn validate_active_package_membership(
    value: &Value,
    product: &Value,
    source: &SourceAuthority,
    graph: &GraphReferences,
) -> Result<(), ReportError> {
    let values =
        required_field_array(product, "package_memberships", "product_proof")?;
    if values.len() != 1 {
        return Err(reference_error(
            "active Foundation proof requires one package membership",
        ));
    }
    let membership = &values[0];
    let context = "package membership";
    if field_string(membership, "closure", context)? != SHIPPING_CLOSURE
        || field_string(membership, "workspace", context)? != PRODUCT_WORKSPACE
        || field_string(membership, "package", context)? != SHIPPING_PACKAGE
    {
        return Err(reference_error(
            "package membership differs from the shipping closure",
        ));
    }
    let semantic_digest = validate_package_members(value, membership, source)?;
    validate_package_captures(value, membership, graph, &semantic_digest)
}

fn validate_package_captures(
    value: &Value,
    membership: &Value,
    graph: &GraphReferences,
    semantic_digest: &str,
) -> Result<(), ReportError> {
    let captures =
        required_field_array(membership, "captures", "package membership")?;
    if captures.len() != 2 {
        return Err(reference_error(
            "package membership requires its exact capture pair",
        ));
    }
    let mut phases = BTreeMap::new();
    for capture in captures {
        let phase = field_string(capture, "phase", "package capture")?;
        if phases.insert(phase.to_owned(), capture).is_some() {
            return Err(reference_error("duplicate package-capture phase"));
        }
        validate_package_capture_argv(value, capture, graph)?;
        let bytes = field_count(capture, "stdout_bytes", "package capture")?;
        let limit = required_count(
            value,
            "/limits/product_execution",
            "package_list_bytes",
        )?;
        if bytes == 0 || bytes > limit {
            return Err(limit_error("package-list capture exceeds its bound"));
        }
        if field_string(capture, "semantic_digest", "package capture")?
            != semantic_digest
        {
            return Err(reference_error(
                "package capture differs from its semantic member set",
            ));
        }
    }
    phases.get("before_compiler").ok_or_else(|| {
        reference_error("before-compiler package capture is absent")
    })?;
    phases.get("after_compiler").ok_or_else(|| {
        reference_error("after-compiler package capture is absent")
    })?;
    Ok(())
}

fn validate_package_capture_argv(
    value: &Value,
    capture: &Value,
    _graph: &GraphReferences,
) -> Result<(), ReportError> {
    let mut expected = vec![
        PREPARED_CARGO,
        "package",
        "--list",
        "--quiet",
        "--locked",
        "--offline",
        "--no-metadata",
        "--manifest-path",
        PREPARED_MANIFEST,
        "--package",
        SHIPPING_CARGO_NAME,
    ];
    if required_string(value, "/mode")? == "candidate" {
        expected.push("--allow-dirty");
    }
    let argv = string_array(capture, "command_argv", "package capture")?;
    if argv.iter().map(String::as_str).ne(expected) {
        return Err(reference_error(
            "Cargo package-list argv differs from the closed profile",
        ));
    }
    Ok(())
}

fn validate_package_members(
    value: &Value,
    membership: &Value,
    source: &SourceAuthority,
) -> Result<String, ReportError> {
    let members =
        required_field_array(membership, "members", "package membership")?;
    let count = u64::try_from(members.len())
        .map_err(|_| limit_error("package member count overflow"))?;
    let limit = required_count(
        value,
        "/limits/product_execution",
        "package_member_count",
    )?;
    if count > limit {
        return Err(limit_error("package members exceed their bound"));
    }
    let mut copied = BTreeSet::new();
    let mut generated = BTreeSet::new();
    for member in members {
        match field_string(member, "kind", "package member")? {
            "copied" => validate_copied_member(member, source, &mut copied)?,
            "cargo_generated" => {
                insert_generated_member(member, &mut generated)?;
            }
            _ => return Err(reference_error("unknown package-member kind")),
        }
    }
    let expected_copied = [
        "src/bound.rs",
        "src/carrier.rs",
        "src/contract.rs",
        "src/identity.rs",
        "src/lib.rs",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let expected_generated = ["Cargo.lock", "Cargo.toml", "Cargo.toml.orig"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    if copied != expected_copied || generated != expected_generated {
        return Err(reference_error(
            "package members differ from the closed Foundation package",
        ));
    }
    let members = copied
        .into_iter()
        .chain(generated)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    package_capture::semantic_digest(&members).map_err(|failure| {
        identity_error(format!(
            "cannot derive package-member identity: {failure}"
        ))
    })
}

fn insert_generated_member(
    member: &Value,
    generated: &mut BTreeSet<String>,
) -> Result<(), ReportError> {
    let path = field_string(member, "package_path", "member")?;
    if generated.insert(path.to_owned()) {
        Ok(())
    } else {
        Err(reference_error("duplicate generated member"))
    }
}

fn validate_copied_member(
    member: &Value,
    source: &SourceAuthority,
    copied: &mut BTreeSet<String>,
) -> Result<(), ReportError> {
    let package_path = field_string(member, "package_path", "copied member")?;
    let repository_path =
        field_string(member, "repository_path", "copied member")?;
    let retained = source
        .files
        .get(&parse_relative_path(repository_path, "copied member")?)
        .ok_or_else(|| reference_error("copied member source is absent"))?;
    if retained.owner != SourceOwner::Product(ArchitecturalRole::Foundation)
        || retained.provenance != SourceProvenance::Authored
        || retained.source_use != SourceUse::Production
        || retained.content_digest
            != field_string(member, "content_digest", "copied member")?
        || retained.size_bytes
            != field_count(member, "size_bytes", "copied member")?
        || !copied.insert(package_path.to_owned())
    {
        return Err(reference_error(
            "copied package member differs from Product source authority",
        ));
    }
    Ok(())
}

fn validate_active_public_consumer(
    value: &Value,
    product: &Value,
    closures: &ActiveClosureReferences,
    units: &BTreeMap<String, ActiveUnitRef>,
) -> Result<(), ReportError> {
    let values =
        required_field_array(product, "public_consumers", "product_proof")?;
    if values.len() != 1 {
        return Err(reference_error(
            "active Foundation proof requires one public consumer",
        ));
    }
    let consumer = &values[0];
    let context = "public consumer";
    if field_string(consumer, "relation", context)? != "foundation-public-api"
        || field_string(consumer, "shipping_closure", context)?
            != SHIPPING_CLOSURE
        || field_string(consumer, "consumer_closure", context)?
            != CONSUMER_CLOSURE
        || field_string(consumer, "status", context)? != "pass"
        || !units.contains_key(&closures.consumer_unit)
    {
        return Err(reference_error(
            "public-consumer identity differs from the closure pair",
        ));
    }
    let types = validate_public_items(value, consumer)?;
    validate_derive_checks(consumer, &types)
}

fn validate_public_items(
    value: &Value,
    consumer: &Value,
) -> Result<BTreeSet<String>, ReportError> {
    let items = required_field_array(consumer, "public_items", "consumer")?;
    let limit = required_count(
        value,
        "/limits/product_execution",
        "public_item_count",
    )?;
    let count = u64::try_from(items.len())
        .map_err(|_| limit_error("public item count overflow"))?;
    if count > limit {
        return Err(limit_error(
            "public-consumer items exceed their retained bound",
        ));
    }
    if items.is_empty() {
        return Err(reference_error("public-consumer item set is empty"));
    }
    let mut identities = BTreeSet::new();
    let mut modules = BTreeSet::new();
    let mut types = BTreeSet::new();
    for item in items {
        let path = field_string(item, "path", "public item")?;
        let kind = field_string(item, "kind", "public item")?;
        if !valid_public_path(path)
            || !identities.insert((kind.to_owned(), path.to_owned()))
        {
            return Err(reference_error(
                "public item path is invalid or duplicated",
            ));
        }
        if kind == "module" {
            modules.insert(path.to_owned());
        }
        if kind == "type" {
            types.insert(path.to_owned());
        }
    }
    let expected_modules = ["bound", "carrier", "contract", "identity"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    if modules != expected_modules || types.is_empty() {
        return Err(reference_error(
            "public items differ from the direct Foundation module shape",
        ));
    }
    Ok(types)
}

fn valid_public_path(path: &str) -> bool {
    !path.is_empty()
        && path.split("::").all(|part| {
            let mut bytes = part.bytes();
            bytes
                .next()
                .is_some_and(|byte| byte == b'_' || byte.is_ascii_alphabetic())
                && bytes
                    .all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
        })
}

fn validate_derive_checks(
    consumer: &Value,
    public_types: &BTreeSet<String>,
) -> Result<(), ReportError> {
    let checks = required_field_array(consumer, "derive_checks", "consumer")?;
    let mut observed = BTreeMap::<String, BTreeSet<String>>::new();
    for check in checks {
        let derive = field_string(check, "derive", "derive check")?;
        let operation =
            field_string(check, "consumer_operation", "derive check")?;
        let public_type = field_string(check, "public_type", "derive check")?;
        if !public_types.contains(public_type)
            || !derive_operation_matches(derive, operation)
            || !observed
                .entry(public_type.to_owned())
                .or_default()
                .insert(derive.to_owned())
        {
            return Err(reference_error(
                "derive check is unresolved, mismatched, or duplicated",
            ));
        }
    }
    let expected = ["Clone", "Copy", "Debug", "Eq", "PartialEq"]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if observed.len() != public_types.len()
        || observed.values().any(|derives| derives != &expected)
    {
        return Err(reference_error(
            "derive checks do not exactly cover every Foundation type",
        ));
    }
    Ok(())
}

fn derive_operation_matches(derive: &str, operation: &str) -> bool {
    matches!(
        (derive, operation),
        ("Clone", "clone")
            | ("Copy", "copy")
            | ("Debug", "debug")
            | ("PartialEq", "partial_eq")
            | ("Eq", "eq")
    )
}

fn validate_product_empty_state(
    value: &Value,
    product: &Value,
    graph: &GraphReferences,
    source: &SourceAuthority,
) -> Result<(), ReportError> {
    if !product
        .get("selected_execution_environment")
        .is_some_and(Value::is_null)
    {
        return Err(error(
            "Product-empty report selects an execution environment",
        ));
    }
    require_empty_product_arrays(product)?;
    if !required_array(value, "/identity_inputs/compiler_substrates")?
        .is_empty()
    {
        return Err(error(
            "Product-empty report contains compiler-substrate evidence",
        ));
    }
    if !value
        .pointer("/identity_inputs/product_proof_gate")
        .is_some_and(Value::is_null)
    {
        return Err(error(
            "Product-empty report references a Product proof gate",
        ));
    }
    if source.files.values().any(|classification| {
        matches!(classification.owner, SourceOwner::Product(_))
    }) || !graph.product_workspaces.is_empty()
    {
        return Err(error(
            "Product-empty report contains Product ownership or workspace",
        ));
    }
    Ok(())
}

fn require_empty_product_arrays(product: &Value) -> Result<(), ReportError> {
    const REQUIRED: &[&str] = &[
        "closures",
        "compiler_units",
        "compiler_inputs",
        "extern_edges",
        "artifacts",
        "package_memberships",
        "public_consumers",
        "executions",
    ];
    if !product.get("provider_artifact").is_some_and(Value::is_null) {
        return Err(product_evidence_error("provider_artifact"));
    }
    for field in REQUIRED {
        if !required_field_array(product, field, "product_proof")?.is_empty() {
            return Err(product_evidence_error(field));
        }
    }
    Ok(())
}

fn product_evidence_error(field: &str) -> ReportError {
    error(format!("Product-empty report contains Product evidence in {field}"))
}

fn enforce_path_byte_limit(
    path: &str,
    limit: u64,
    context: &str,
) -> Result<(), ReportError> {
    let bytes = u64::try_from(path.len())
        .map_err(|_| limit_error(format!("{context} path length overflow")))?;
    if bytes > limit {
        return Err(limit_error(format!(
            "{context} path is {bytes} bytes; limit is {limit}"
        )));
    }
    Ok(())
}

fn enforce_product_path_limit(
    value: &Value,
    path: &str,
    context: &str,
) -> Result<(), ReportError> {
    let limit = required_count(value, "/limits/inventory", "path_bytes")?;
    enforce_path_byte_limit(path, limit, context)
}

fn required_count(
    value: &Value,
    prefix: &str,
    field: &str,
) -> Result<u64, ReportError> {
    value
        .pointer(&format!("{prefix}/{field}"))
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            limit_error(format!("{prefix}/{field} is absent or invalid"))
        })
}

fn field_count(
    value: &Value,
    field: &str,
    context: &str,
) -> Result<u64, ReportError> {
    value.get(field).and_then(Value::as_u64).ok_or_else(|| {
        limit_error(format!("{context}.{field} is absent or invalid"))
    })
}

fn field_bool(
    value: &Value,
    field: &str,
    context: &str,
) -> Result<bool, ReportError> {
    value
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| error(format!("{context}.{field} is absent or invalid")))
}

fn object_field<'a>(
    value: &'a Value,
    field: &str,
    context: &str,
) -> Result<&'a Value, ReportError> {
    value
        .get(field)
        .ok_or_else(|| error(format!("{context}.{field} is absent")))
}

fn package_endpoint_ref(
    endpoint: &Value,
    workspace: &str,
    profile: &str,
    packages: &BTreeSet<(String, String, String)>,
    external_packages: &BTreeSet<(String, String, String, String, String)>,
) -> Result<PackageEndpointRef, ReportError> {
    match field_string(endpoint, "kind", "Cargo edge endpoint")? {
        "workspace_package" => {
            let retained_workspace =
                field_string(endpoint, "workspace", "Cargo edge endpoint")?;
            let package =
                field_string(endpoint, "package", "Cargo edge endpoint")?;
            if retained_workspace != workspace {
                return Err(reference_error(
                    "Cargo edge workspace endpoint crosses workspace",
                ));
            }
            require_package(
                packages,
                workspace,
                profile,
                package,
                "Cargo edge endpoint",
            )?;
            Ok(PackageEndpointRef::Workspace {
                workspace: workspace.to_owned(),
                package: package.to_owned(),
            })
        }
        "external_package" => {
            let name = field_string(endpoint, "name", "Cargo edge endpoint")?;
            let version =
                field_string(endpoint, "version", "Cargo edge endpoint")?;
            let source =
                field_string(endpoint, "source", "Cargo edge endpoint")?;
            let key = (
                workspace.to_owned(),
                profile.to_owned(),
                name.to_owned(),
                version.to_owned(),
                source.to_owned(),
            );
            if !external_packages.contains(&key) {
                return Err(error(
                    "Cargo edge references an unknown external package",
                ));
            }
            Ok(PackageEndpointRef::External {
                name: name.to_owned(),
                version: version.to_owned(),
                source: source.to_owned(),
            })
        }
        _ => Err(error("Cargo edge endpoint kind is invalid")),
    }
}

fn validate_process_references(
    value: &Value,
    workspaces: &BTreeSet<String>,
    profiles: &BTreeSet<(String, String)>,
    passing: bool,
    active: Option<&ActiveProductReferences>,
) -> Result<(), ReportError> {
    let limits = RetainedProcessLimits::from_report(value)?;
    for process in required_array(value, "/processes")? {
        validate_process_scope(process, workspaces, profiles, active)?;
        validate_process_ownership(process, limits, passing)?;
    }
    Ok(())
}

fn validate_process_purposes(
    value: &Value,
    graph: &GraphReferences,
    active: Option<&ActiveProductReferences>,
) -> Result<(), ReportError> {
    let mut versions = BTreeMap::new();
    let mut git = BTreeMap::new();
    let mut product = BTreeMap::new();
    for process in required_array(value, "/processes")? {
        let purpose = field_string(process, "purpose", "process")?;
        match purpose {
            "checker_version" | "cargo_version" | "rustc_version" => {
                insert_process_purpose(&mut versions, purpose, process)?;
            }
            "cargo_metadata" | "cargo_metadata_final" => {}
            "product_proof_execution" => {
                insert_process_purpose(&mut product, purpose, process)?;
            }
            purpose if git_process_arguments(purpose).is_some() => {
                insert_process_purpose(&mut git, purpose, process)?;
            }
            _ => {
                return Err(reference_error(format!(
                    "unsupported current process purpose {purpose}"
                )));
            }
        }
    }
    validate_version_processes(value, &versions)?;
    validate_product_process(value, active, &product)?;
    match required_string(value, "/mode")? {
        "candidate" if git.is_empty() => Ok(()),
        "exact" => validate_git_processes(value, graph, &git),
        "candidate" => Err(reference_error(
            "candidate report contains exact Git processes",
        )),
        _ => Err(identity_error("process purpose mode is invalid")),
    }
}

fn validate_product_process(
    value: &Value,
    active: Option<&ActiveProductReferences>,
    processes: &BTreeMap<String, &Value>,
) -> Result<(), ReportError> {
    let Some(active) = active else {
        if processes.is_empty() {
            return Ok(());
        }
        return Err(reference_error(
            "Product-empty report contains a Product process",
        ));
    };
    if processes.len() != 1 {
        return Err(reference_error(
            "active Foundation proof requires one Product process",
        ));
    }
    let process = processes
        .get("product_proof_execution")
        .ok_or_else(|| reference_error("Product proof process is absent"))?;
    let checker = value
        .pointer("/identity_inputs/checker")
        .ok_or_else(|| identity_error("checker identity is absent"))?;
    let argv = string_array(process, "argv", "Product process")?;
    let checker_path = field_string(checker, "canonical_path", "checker")?;
    let expected_argv = [checker_path, "product-proof-child"];
    if process.get("workspace").and_then(Value::as_str)
        != Some(PRODUCT_WORKSPACE)
        || process.get("profile").and_then(Value::as_str)
            != Some(PRODUCT_PROFILE)
        || !process.get("closure").is_some_and(Value::is_null)
        || process.get("execution").and_then(Value::as_str)
            != Some(active.execution.as_str())
        || field_string(process, "executable_digest", "Product process")?
            != field_string(checker, "executable_digest", "checker")?
        || argv.iter().map(String::as_str).ne(expected_argv)
        || !process.get("tool_review").is_some_and(Value::is_null)
        || !process.get("tool_environment").is_some_and(Value::is_null)
    {
        return Err(reference_error(
            "Product process differs from its execution/checker authority",
        ));
    }
    Ok(())
}

fn insert_process_purpose<'a>(
    processes: &mut BTreeMap<String, &'a Value>,
    purpose: &str,
    process: &'a Value,
) -> Result<(), ReportError> {
    if processes.insert(purpose.to_owned(), process).is_some() {
        return Err(reference_error(format!(
            "duplicate process purpose {purpose}"
        )));
    }
    Ok(())
}

fn validate_version_processes(
    value: &Value,
    processes: &BTreeMap<String, &Value>,
) -> Result<(), ReportError> {
    let expected = [
        ("checker_version", "checker"),
        ("cargo_version", "cargo"),
        ("rustc_version", "rustc"),
    ];
    if processes.len() != expected.len() {
        return Err(reference_error(
            "version processes do not exactly cover current tools",
        ));
    }
    for (purpose, tool) in expected {
        let process = processes.get(purpose).ok_or_else(|| {
            reference_error(format!("missing {purpose} process"))
        })?;
        validate_version_process(value, process, tool)?;
    }
    Ok(())
}

fn validate_version_process(
    value: &Value,
    process: &Value,
    tool_name: &str,
) -> Result<(), ReportError> {
    let tool =
        value.pointer(&format!("/identity_inputs/{tool_name}")).ok_or_else(
            || identity_error(format!("{tool_name} identity absent")),
        )?;
    let canonical = field_string(tool, "canonical_path", tool_name)?;
    let mut expected_argv = vec![canonical.to_owned()];
    expected_argv.extend(string_array(tool, "version_argv", tool_name)?);
    let actual_argv = string_array(process, "argv", "version process")?;
    let expected_digest = field_string(tool, "executable_digest", tool_name)?;
    let stdout_digest =
        process.pointer("/stdout/digest").and_then(Value::as_str).ok_or_else(
            || reference_error("version process stdout digest absent"),
        )?;
    let version_digest = field_string(tool, "version_digest", tool_name)?;
    let version = field_string(tool, "version", tool_name)?;
    let expected_bytes = u64::try_from(version.len() + 1)
        .map_err(|_| limit_error("version output byte count overflow"))?;
    if actual_argv != expected_argv
        || field_string(process, "executable_digest", "version process")?
            != expected_digest
        || stdout_digest != version_digest
        || process.pointer("/stdout/bytes").and_then(Value::as_u64)
            != Some(expected_bytes)
        || !null_process_scope(process)
    {
        return Err(reference_error(format!(
            "{tool_name} version process differs from retained identity"
        )));
    }
    Ok(())
}

fn null_process_scope(process: &Value) -> bool {
    ["workspace", "profile", "tool_review", "tool_environment"]
        .iter()
        .all(|field| process.get(*field).is_some_and(Value::is_null))
}

fn validate_git_processes(
    value: &Value,
    graph: &GraphReferences,
    processes: &BTreeMap<String, &Value>,
) -> Result<(), ReportError> {
    let expected = GIT_PROCESS_PURPOSES;
    if processes.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != expected.into_iter().collect()
    {
        return Err(reference_error(
            "exact Git processes do not cover the admitted family",
        ));
    }
    let mut authority = None;
    let mut captures = BTreeMap::new();
    for purpose in expected {
        let process =
            processes.get(purpose).expect("checked exact purpose set");
        validate_git_process(value, graph, purpose, process, &mut authority)?;
        retain_git_operation_capture(purpose, process, &mut captures)?;
    }
    Ok(())
}

fn retain_git_operation_capture(
    purpose: &str,
    process: &Value,
    captures: &mut BTreeMap<String, (String, u64)>,
) -> Result<(), ReportError> {
    let operation = git_process_operation(purpose)
        .ok_or_else(|| reference_error("unknown Git process operation"))?;
    let capture = object_field(process, "stdout", "Git process")?;
    let observed = (
        field_string(capture, "digest", "Git stdout")?.to_owned(),
        field_count(capture, "bytes", "Git stdout")?,
    );
    if let Some(expected) = captures.get(operation)
        && expected != &observed
    {
        return Err(reference_error(format!(
            "Git {operation} captures disagree across proof phases"
        )));
    }
    captures.insert(operation.to_owned(), observed);
    Ok(())
}

const GIT_PROCESS_PURPOSES: [&str; 21] = [
    "git_version",
    "git_root",
    "git_head",
    "git_object_format",
    "git_ls_tree",
    "git_index_entries",
    "git_info_attributes_path",
    "git_pre_metadata_version",
    "git_pre_metadata_root",
    "git_pre_metadata_head",
    "git_pre_metadata_object_format",
    "git_pre_metadata_ls_tree",
    "git_pre_metadata_index_entries",
    "git_pre_metadata_info_attributes_path",
    "git_final_version",
    "git_final_root",
    "git_final_head",
    "git_final_object_format",
    "git_final_ls_tree",
    "git_final_index_entries",
    "git_final_info_attributes_path",
];

fn validate_git_process(
    value: &Value,
    graph: &GraphReferences,
    purpose: &str,
    process: &Value,
    authority: &mut Option<(String, String)>,
) -> Result<(), ReportError> {
    let git = value
        .pointer("/identity_inputs/git")
        .ok_or_else(|| identity_error("exact Git identity is absent"))?;
    let canonical = field_string(git, "canonical_path", "Git identity")?;
    let mut expected_argv = vec![canonical.to_owned()];
    expected_argv
        .extend(git_process_arguments(purpose).expect("known purpose"));
    let actual_argv = string_array(process, "argv", "Git process")?;
    if actual_argv != expected_argv
        || field_string(process, "executable_digest", "Git process")?
            != field_string(git, "executable_digest", "Git identity")?
    {
        return Err(reference_error(
            "Git process argv or executable identity differs",
        ));
    }
    validate_git_process_scope(process, graph, authority)?;
    validate_git_process_digest(value, git, graph, purpose, process)
}

fn validate_git_process_scope(
    process: &Value,
    graph: &GraphReferences,
    authority: &mut Option<(String, String)>,
) -> Result<(), ReportError> {
    let workspace = field_string(process, "workspace", "Git process")?;
    if !graph.workspaces.contains(workspace)
        || !process.get("profile").is_some_and(Value::is_null)
    {
        return Err(reference_error("Git process scope is invalid"));
    }
    let review = field_string(process, "tool_review", "Git process")?;
    let environment = field_string(process, "tool_environment", "Git process")?;
    let observed = (review.to_owned(), environment.to_owned());
    if let Some(expected) = authority
        && expected != &observed
    {
        return Err(reference_error(
            "Git processes disagree on review or environment authority",
        ));
    }
    *authority = Some(observed);
    Ok(())
}

fn validate_git_process_digest(
    value: &Value,
    git: &Value,
    graph: &GraphReferences,
    purpose: &str,
    process: &Value,
) -> Result<(), ReportError> {
    let actual = process
        .pointer("/stdout/digest")
        .and_then(Value::as_str)
        .ok_or_else(|| reference_error("Git process stdout digest absent"))?;
    let (expected, expected_bytes) = match git_process_operation(purpose) {
        Some("version") => git_version_output(git)?,
        Some("root") => git_root_output(graph)?,
        Some("head") => git_head_output(value)?,
        Some("object_format") => git_object_format_output(value)?,
        Some("ls_tree") => {
            let digest =
                required_string(value, "/identity_inputs/git_tree_digest")?;
            (digest.to_owned(), None)
        }
        _ => return Ok(()),
    };
    let actual_bytes = process
        .pointer("/stdout/bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| reference_error("Git process stdout bytes absent"))?;
    if actual != expected
        || expected_bytes.is_some_and(|bytes| bytes != actual_bytes)
    {
        return Err(reference_error(format!(
            "{purpose} stdout differs from retained authority"
        )));
    }
    Ok(())
}

fn git_version_output(
    git: &Value,
) -> Result<(String, Option<u64>), ReportError> {
    retained_text_output(field_string(git, "version", "Git identity")?)
}

fn git_root_output(
    graph: &GraphReferences,
) -> Result<(String, Option<u64>), ReportError> {
    let root = graph.repository_root.to_str().ok_or_else(|| {
        reference_error("metadata repository root is not UTF-8")
    })?;
    retained_text_output(root)
}

fn git_head_output(
    value: &Value,
) -> Result<(String, Option<u64>), ReportError> {
    retained_text_output(required_string(value, "/revision")?)
}

fn git_object_format_output(
    value: &Value,
) -> Result<(String, Option<u64>), ReportError> {
    let revision = required_string(value, "/revision")?;
    let format = if revision.len() == 40 { "sha1" } else { "sha256" };
    retained_text_output(format)
}

fn retained_text_output(
    value: &str,
) -> Result<(String, Option<u64>), ReportError> {
    let output = format!("{value}\n");
    let bytes = u64::try_from(output.len())
        .map_err(|_| limit_error("Git output byte count overflow"))?;
    Ok((digest::bytes(output.as_bytes()), Some(bytes)))
}

fn git_process_arguments(purpose: &str) -> Option<Vec<String>> {
    let mut arguments = [
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.untrackedCache=false",
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "core.fileMode=true",
        "-c",
        "core.autocrlf=false",
        "-c",
        "core.eol=lf",
        "-c",
        "core.ignoreStat=false",
        "-c",
        "core.ignoreCase=false",
        "-c",
        "core.attributesFile=/dev/null",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    let suffix: &[&str] = match git_process_operation(purpose)? {
        "version" => &["--version"],
        "root" => &["rev-parse", "--show-toplevel"],
        "head" => &["rev-parse", "HEAD"],
        "object_format" => &["rev-parse", "--show-object-format"],
        "ls_tree" => &["ls-tree", "-r", "-z", "--full-tree", "HEAD"],
        "index_entries" => &["ls-files", "--stage", "-v", "-f", "-z"],
        "info_attributes_path" => {
            &["rev-parse", "--git-path", "info/attributes"]
        }
        _ => return None,
    };
    arguments.extend(suffix.iter().map(|argument| (*argument).to_owned()));
    Some(arguments)
}

fn git_process_operation(purpose: &str) -> Option<&str> {
    purpose
        .strip_prefix("git_pre_metadata_")
        .or_else(|| purpose.strip_prefix("git_final_"))
        .or_else(|| purpose.strip_prefix("git_"))
}

#[derive(Clone, Copy)]
struct RetainedProcessLimits {
    stdout_bytes: u64,
    stderr_bytes: u64,
    execution_ms: u64,
    supervision_ms: u64,
}

impl RetainedProcessLimits {
    fn from_report(value: &Value) -> Result<Self, ReportError> {
        let prefix = "/limits/process";
        let limits = Self {
            stdout_bytes: required_count(value, prefix, "stdout_bytes")?,
            stderr_bytes: required_count(value, prefix, "stderr_bytes")?,
            execution_ms: required_count(
                value,
                prefix,
                "execution_deadline_ms",
            )?,
            supervision_ms: required_count(
                value,
                prefix,
                "supervision_deadline_ms",
            )?,
        };
        if limits.supervision_ms <= limits.execution_ms {
            return Err(limit_error(
                "process supervision deadline does not exceed execution",
            ));
        }
        Ok(limits)
    }
}

fn validate_process_scope(
    process: &Value,
    workspaces: &BTreeSet<String>,
    profiles: &BTreeSet<(String, String)>,
    active: Option<&ActiveProductReferences>,
) -> Result<(), ReportError> {
    let workspace = process.get("workspace").and_then(Value::as_str);
    let profile = process.get("profile").and_then(Value::as_str);
    match (workspace, profile) {
        (Some(workspace), Some(profile)) => {
            require_profile(profiles, workspace, profile, "process")?;
        }
        (Some(workspace), None) => {
            require_member(workspaces, workspace, "process workspace")?;
        }
        (None, Some(_)) => {
            return Err(error("process profile has no workspace identity"));
        }
        (None, None) => {}
    }
    let product_process = field_string(process, "purpose", "process")?
        == "product_proof_execution";
    if active.is_some() && product_process {
        return Ok(());
    }
    if !process.get("closure").is_some_and(Value::is_null)
        || !process.get("execution").is_some_and(Value::is_null)
    {
        return Err(error("non-Product process contains Product references"));
    }
    Ok(())
}

fn validate_process_ownership(
    process: &Value,
    limits: RetainedProcessLimits,
    passing: bool,
) -> Result<(), ReportError> {
    validate_capture(process, "stdout", limits.stdout_bytes, passing)?;
    validate_capture(process, "stderr", limits.stderr_bytes, passing)?;
    validate_group_and_kill(process)?;
    validate_reap_and_readers(process)?;
    validate_process_deadlines(process, limits, passing)?;
    if passing {
        validate_process_success(process)?;
    }
    Ok(())
}

fn validate_capture(
    process: &Value,
    field: &str,
    expected_limit: u64,
    passing: bool,
) -> Result<(), ReportError> {
    let capture = object_field(process, field, "process")?;
    let bytes = field_count(capture, "bytes", field)?;
    let limit = field_count(capture, "limit", field)?;
    let overflow = field_bool(capture, "overflow", field)?;
    field_string(capture, "digest", field)?;
    if limit != expected_limit || bytes > limit {
        return Err(limit_error(format!(
            "process {field} bytes or limit differ from retained bounds"
        )));
    }
    if passing && overflow {
        return Err(error(format!("passing process reports {field} overflow")));
    }
    Ok(())
}

fn validate_group_and_kill(process: &Value) -> Result<(), ReportError> {
    let group = object_field(process, "group", "process")?;
    let identity = field_bool(group, "identity_validated", "process group")?;
    let absent = field_bool(group, "absent_after_reap", "process group")?;
    if !identity || !absent {
        return Err(error(
            "retained process lacks validated and absent group ownership",
        ));
    }
    let kill = object_field(process, "kill", "process")?;
    let attempted = field_bool(kill, "attempted", "process kill")?;
    let raced = field_bool(kill, "raced_with_exit", "process kill")?;
    let killed_group = field_bool(kill, "group_absent", "process kill")?;
    let signal = kill.get("signal").and_then(Value::as_str);
    if killed_group != absent
        || !killed_group
        || raced && !attempted
        || (attempted && signal != Some("SIGKILL"))
        || (!attempted && !kill["signal"].is_null())
    {
        return Err(error("retained process kill evidence is inconsistent"));
    }
    Ok(())
}

fn validate_reap_and_readers(process: &Value) -> Result<(), ReportError> {
    let reap = object_field(process, "reap", "process")?;
    let readers = object_field(process, "readers", "process")?;
    let complete = field_bool(reap, "leader_reaped", "process reap")?
        && field_bool(reap, "descendants_reaped", "process reap")?
        && field_bool(readers, "stdout_joined", "process readers")?
        && field_bool(readers, "stderr_joined", "process readers")?;
    if !complete {
        return Err(error("retained process cleanup is incomplete"));
    }
    Ok(())
}

fn validate_process_deadlines(
    process: &Value,
    limits: RetainedProcessLimits,
    passing: bool,
) -> Result<(), ReportError> {
    let deadlines = object_field(process, "deadlines", "process")?;
    let execution = field_count(deadlines, "execution_ms", "deadlines")?;
    let supervision = field_count(deadlines, "supervision_ms", "deadlines")?;
    let observed = field_bool(deadlines, "observed_after_exit", "deadlines")?;
    if execution != limits.execution_ms
        || supervision != limits.supervision_ms
        || passing && observed
    {
        return Err(limit_error(
            "process deadlines differ from retained passing bounds",
        ));
    }
    Ok(())
}

fn validate_process_success(process: &Value) -> Result<(), ReportError> {
    let successful = field_string(process, "termination_reason", "process")?
        == "exited"
        && process.get("status").and_then(Value::as_i64) == Some(0)
        && process.pointer("/stderr/bytes").and_then(Value::as_u64) == Some(0);
    if !successful {
        return Err(error(format!(
            "passing report contains unsuccessful process {}",
            field_string(process, "purpose", "process")?
        )));
    }
    Ok(())
}

fn require_profile(
    profiles: &BTreeSet<(String, String)>,
    workspace: &str,
    profile: &str,
    context: &str,
) -> Result<(), ReportError> {
    if profiles.contains(&(workspace.to_owned(), profile.to_owned())) {
        Ok(())
    } else {
        Err(error(format!(
            "{context} references unknown workspace/profile \
             {workspace}/{profile}"
        )))
    }
}

fn require_package(
    packages: &BTreeSet<(String, String, String)>,
    workspace: &str,
    profile: &str,
    package: &str,
    context: &str,
) -> Result<(), ReportError> {
    if packages.contains(&(
        workspace.to_owned(),
        profile.to_owned(),
        package.to_owned(),
    )) {
        Ok(())
    } else {
        Err(error(format!(
            "{context} references unknown workspace/profile/package \
             {workspace}/{profile}/{package}"
        )))
    }
}

fn insert_unique(
    set: &mut BTreeSet<String>,
    identity: &str,
    kind: &str,
) -> Result<(), ReportError> {
    if set.insert(identity.to_owned()) {
        Ok(())
    } else {
        Err(error(format!("duplicate {kind} identity {identity}")))
    }
}

fn require_member(
    set: &BTreeSet<String>,
    identity: &str,
    kind: &str,
) -> Result<(), ReportError> {
    if set.contains(identity) {
        Ok(())
    } else {
        Err(error(format!("{kind} references unknown identity {identity}")))
    }
}

fn required_field_array<'a>(
    value: &'a Value,
    field: &str,
    context: &str,
) -> Result<&'a [Value], ReportError> {
    value.get(field).and_then(Value::as_array).map(Vec::as_slice).ok_or_else(
        || error(format!("{context}.{field} is absent or not an array")),
    )
}

fn required_array<'a>(
    value: &'a Value,
    pointer: &str,
) -> Result<&'a [Value], ReportError> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| error(format!("{pointer} is absent or not an array")))
}

fn required_string<'a>(
    value: &'a Value,
    pointer: &str,
) -> Result<&'a str, ReportError> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| error(format!("{pointer} is absent or not a string")))
}

fn field_string<'a>(
    value: &'a Value,
    field: &str,
    context: &str,
) -> Result<&'a str, ReportError> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| {
        error(format!("{context}.{field} is absent or not a string"))
    })
}

fn valid_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn normalized_absolute_text(value: &str) -> bool {
    value.starts_with('/')
        && (value == "/" || !value.ends_with('/'))
        && !value.contains(['\0', '\r', '\n', '\\'])
        && !value.contains("//")
        && !value.split('/').any(|component| matches!(component, "." | ".."))
}

fn normalized_file_path(value: &str) -> bool {
    value != "/" && normalized_absolute_text(value)
}

fn schema_error(failure: &schema::SchemaError) -> ReportError {
    schema_report_error(failure.to_string())
}

fn error(message: impl Into<String>) -> ReportError {
    ReportError::new(ReportErrorCode::Semantic, message)
}

fn envelope_error(message: impl Into<String>) -> ReportError {
    ReportError::new(ReportErrorCode::Envelope, message)
}

fn schema_report_error(message: impl Into<String>) -> ReportError {
    ReportError::new(ReportErrorCode::Schema, message)
}

fn canonical_error(message: impl Into<String>) -> ReportError {
    ReportError::new(ReportErrorCode::Canonical, message)
}

fn identity_error(message: impl Into<String>) -> ReportError {
    ReportError::new(ReportErrorCode::Identity, message)
}

fn limit_error(message: impl Into<String>) -> ReportError {
    ReportError::new(ReportErrorCode::Limit, message)
}

fn reference_error(message: impl Into<String>) -> ReportError {
    ReportError::new(ReportErrorCode::Reference, message)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use serde_json::{Value, json};

    use crate::{package_capture, prepared_root};

    use super::{
        BuiltReport, NETWORK_POLICY_BASIS, NETWORK_POLICY_DOMAIN,
        PREPARED_CARGO, PREPARED_CHECKER, PREPARED_RUSTC,
        PRODUCT_REPOSITORY_INPUTS, ReportError, ReportErrorCode, SCHEMA,
    };

    const REPORT_LIMIT: usize = 4 * 1024 * 1024;
    const JSON_DEPTH: usize = 256;
    const JSON_ITEMS: usize = 1_000_000;

    fn refresh_source_tree_digest(report: &mut Value) {
        let digest = {
            let limits = super::SourceLimits::from_report(report).unwrap();
            let values = super::required_array(
                report,
                "/source_inventory/classifications",
            )
            .unwrap();
            let authority =
                super::collect_source_authority(values, limits).unwrap();
            super::computed_source_tree_digest(&authority)
        };
        report["identity_inputs"]["source_tree_digest"] = json!(digest);
    }

    fn classification<'a>(report: &'a Value, path: &str) -> &'a Value {
        report["source_inventory"]["classifications"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["path"] == path)
            .unwrap()
    }

    fn classification_mut<'a>(
        report: &'a mut Value,
        path: &str,
    ) -> &'a mut Value {
        report["source_inventory"]["classifications"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|entry| entry["path"] == path)
            .unwrap()
    }

    fn forbidden_upstream_subject(source_use: &str) -> ReportError {
        verify_reidentified_mutant(|report| {
            let subject = classification_mut(report, "Cargo.toml");
            subject["source_provenance"] = json!("generated");
            subject["source_use"] = json!(source_use);
            subject["upstream"] = generated_upstream([crate::POLICY_KEY]);
            classification_mut(report, "Cargo.lock")["upstream"] =
                generated_upstream(["Cargo.toml"]);
        })
    }

    fn assert_source_count_limit(field: &str) {
        let failure = verify_reidentified_mutant(|report| {
            report["limits"]["inventory"][field] = json!(1);
        });
        assert_eq!(failure.code(), ReportErrorCode::Limit, "{field}");
    }

    fn reuse_winning_projection(report: &mut Value) {
        for path in [crate::POLICY_KEY, crate::TOPOLOGY_KEY] {
            classification_mut(report, path)["winning_rule"]["prefix"] =
                json!("conformance");
        }
    }

    fn assert_artifact_digest_crosslink(field: &str, digest_field: &str) {
        let failure = verify_reidentified_mutant(|report| {
            report["identity_inputs"][field][digest_field] = json!(digest('0'));
        });
        assert_eq!(
            failure.code(),
            ReportErrorCode::Reference,
            "{field}.{digest_field}"
        );
    }

    fn assert_runtime_identity_conflict(field: &str) {
        let replacement = if field == "content_digest" {
            json!(digest('2'))
        } else {
            json!(2)
        };
        let conflict = verify_reidentified_mutant(move |report| {
            let first = runtime_input("/runtime/lib.so", "/lib/lib.so", '1', 1);
            let mut second = first.clone();
            second[field] = replacement;
            report["identity_inputs"]["checker"]["runtime_inputs"] =
                json!([first]);
            report["identity_inputs"]["cargo"]["runtime_inputs"] =
                json!([second]);
        });
        assert_eq!(conflict.code(), ReportErrorCode::Identity, "{field}");
    }

    fn assert_executable_runtime_conflict(digest_byte: char, size_bytes: u64) {
        let conflict = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["checker"]["runtime_inputs"] =
                json!([runtime_input(
                    "/cargo",
                    "/lib/cargo.so",
                    digest_byte,
                    size_bytes,
                )]);
        });
        assert_eq!(conflict.code(), ReportErrorCode::Identity);
    }

    fn assert_invalid_retained_version(version: &str) {
        let failure = verify_reidentified_mutant(|report| {
            let output = format!("{version}\n");
            report["identity_inputs"]["cargo"]["version"] = json!(version);
            report["identity_inputs"]["cargo"]["version_digest"] =
                json!(crate::digest::bytes(output.as_bytes()));
        });
        assert_eq!(failure.code(), ReportErrorCode::Identity, "{version:?}");
    }

    fn assert_artifact_crosslinks(field: &str) {
        for digest_field in ["content_digest", "schema_digest"] {
            assert_artifact_digest_crosslink(field, digest_field);
        }
    }

    fn assert_false_namespace_fact(field: &str) {
        let failure = active_product_failure(|report| {
            report["product_proof"]["executions"][0]["namespaces"][field] =
                json!(false);
        });
        assert_eq!(failure.code(), ReportErrorCode::Reference, "{field}");
    }

    fn assert_missing_execution_field(group: &str, field: &str) {
        let failure = active_product_failure(|report| {
            report["product_proof"]["executions"][0][group]
                .as_object_mut()
                .unwrap()
                .remove(field);
        });
        assert_eq!(failure.code(), ReportErrorCode::Schema, "{field}");
    }

    fn remove_product_manifest_exposure(report: &mut Value) {
        let values = report["product_proof"]["executions"][0]["exposure"]
            .as_array_mut()
            .unwrap();
        let position = values
            .iter()
            .position(|entry| {
                entry["destination"] == "/repo/product/Cargo.toml"
            })
            .unwrap();
        values.remove(position);
    }

    #[test]
    fn active_product_round_trip_closes_foundation_evidence() {
        let fixture = active_product_report();
        let captures =
            fixture["product_proof"]["package_memberships"][0]["captures"]
                .as_array()
                .unwrap();
        assert_ne!(
            captures[0]["raw_output_digest"],
            captures[1]["raw_output_digest"]
        );
        let built = super::build(fixture, REPORT_LIMIT).unwrap();
        let verified =
            super::verify(&built.bytes, REPORT_LIMIT, JSON_DEPTH, JSON_ITEMS)
                .unwrap();
        assert_eq!(verified.identity, built.identity);
        assert_eq!(verified.mode, "exact");
        assert_eq!(verified.status, "pass");
    }

    #[test]
    fn active_product_argv_separates_inner_and_outer_tool_paths() {
        let mut report = active_product_report();
        assert_eq!(
            report["identity_inputs"]["cargo"]["canonical_path"],
            "/toolchain/bin/cargo"
        );
        let capture = &report["product_proof"]["closures"][0]["cargo_capture"];
        assert_eq!(capture["command_argv"][0], PREPARED_CARGO);
        assert_eq!(
            process_by_purpose(&mut report, "product_proof_execution")["argv"]
                [0],
            "/checker"
        );

        let cargo = active_product_failure(|report| {
            let capture =
                &mut report["product_proof"]["closures"][0]["cargo_capture"];
            capture["command_argv"][0] = json!("/cargo");
        });
        assert_eq!(cargo.code(), ReportErrorCode::Reference);

        let rustc = active_product_failure(|report| {
            let unit = &mut report["product_proof"]["compiler_units"][0];
            unit["original_argv"][0] = json!("/rustc");
            unit["effective_argv"][0] = json!("/rustc");
        });
        assert_eq!(rustc.code(), ReportErrorCode::Reference);

        let probe = active_product_failure(|report| {
            let capture =
                &mut report["product_proof"]["closures"][0]["cargo_capture"];
            capture["probes"][0]["original_argv"][0] = json!("/rustc");
            capture["probes"][0]["effective_argv"][0] = json!("/rustc");
        });
        assert_eq!(probe.code(), ReportErrorCode::Reference);

        let package = active_product_failure(|report| {
            report["product_proof"]["package_memberships"][0]["captures"][0]
                ["command_argv"][0] = json!("/cargo");
        });
        assert_eq!(package.code(), ReportErrorCode::Reference);

        let checker = active_product_failure(|report| {
            process_by_purpose(report, "product_proof_execution")["argv"][0] =
                json!("/other-checker");
        });
        assert_eq!(checker.code(), ReportErrorCode::Reference);
    }

    #[test]
    fn active_product_crosslinks_reject_identity_correct_mutants() {
        let input = active_product_failure(|report| {
            report["product_proof"]["compiler_inputs"][0]["content_digest"] =
                json!(digest('0'));
        });
        assert_eq!(input.code(), ReportErrorCode::Reference);

        let provider = active_product_failure(|report| {
            report["product_proof"]["extern_edges"][0]["provider"]["path"] =
                json!("/scratch/target/wrong.rlib");
        });
        assert_eq!(provider.code(), ReportErrorCode::Reference);

        let missing_rmeta = active_product_failure(|report| {
            report["product_proof"]["artifacts"]
                .as_array_mut()
                .unwrap()
                .remove(1);
        });
        assert_eq!(missing_rmeta.code(), ReportErrorCode::Reference);

        let empty_runtime = active_product_failure(|report| {
            report["identity_inputs"]["cargo"]["runtime_inputs"] = json!([]);
        });
        assert_eq!(empty_runtime.code(), ReportErrorCode::Reference);

        let orphan_runtime = active_product_failure(|report| {
            let values = report["product_proof"]["executions"][0]["exposure"]
                .as_array_mut()
                .unwrap();
            let runtime = values
                .iter_mut()
                .find(|entry| entry["purpose"] == "system runtime")
                .unwrap();
            runtime["source"] = json!("/runtime/orphan.so");
        });
        assert_eq!(orphan_runtime.code(), ReportErrorCode::Reference);

        let package = active_product_failure(|report| {
            report["product_proof"]["package_memberships"][0]["captures"][1]
                ["semantic_digest"] = json!(digest('0'));
        });
        assert_eq!(package.code(), ReportErrorCode::Reference);

        let public = active_product_failure(|report| {
            report["product_proof"]["public_consumers"][0]["derive_checks"]
                [0]["consumer_operation"] = json!("copy");
        });
        assert_eq!(public.code(), ReportErrorCode::Reference);

        let process = active_product_failure(|report| {
            process_by_purpose(report, "product_proof_execution")["execution"] =
                json!("other-execution");
        });
        assert_eq!(process.code(), ReportErrorCode::Reference);

        let feature = active_product_failure(|report| {
            report["product_proof"]["compiler_units"][0]["features"] =
                json!([]);
        });
        assert_eq!(feature.code(), ReportErrorCode::Reference);

        let cfg = active_product_failure(|report| {
            report["product_proof"]["compiler_units"][0]["cfgs"] =
                json!(["feature=\"other\""]);
        });
        assert_eq!(cfg.code(), ReportErrorCode::Reference);

        let original_sysroot = active_product_failure(|report| {
            let argv =
                report["product_proof"]["compiler_units"][0]["original_argv"]
                    .as_array_mut()
                    .unwrap();
            argv.extend([json!("--sysroot"), json!("/sysroot")]);
        });
        assert_eq!(original_sysroot.code(), ReportErrorCode::Reference);

        let missing_sysroot = active_product_failure(|report| {
            report["product_proof"]["compiler_units"][0]["effective_argv"] =
                json!([
                    PREPARED_RUSTC,
                    "--target",
                    "x86_64-unknown-linux-gnu",
                    concat!("--emit=link,dep-info=", "/scratch/dep/shipping.d")
                ]);
        });
        assert_eq!(missing_sysroot.code(), ReportErrorCode::Reference);

        for argument in [
            "-lfoo",
            "-Lnative=/scratch/native",
            "/repo/kernel/foundation/src/other.rs",
            "--cap-lints=allow",
        ] {
            assert_extra_unit_argument_rejected(argument);
        }
    }

    #[test]
    fn active_product_mapping_and_artifact_mutants_fail() {
        let runtime_destination = active_product_failure(|report| {
            let input =
                &mut report["identity_inputs"]["cargo"]["runtime_inputs"][0];
            input["prepared_path"] = json!("/lib/other.so");
        });
        assert_eq!(runtime_destination.code(), ReportErrorCode::Reference);

        let compiler_root = active_product_failure(|report| {
            let substrate =
                &mut report["identity_inputs"]["compiler_substrates"][0];
            substrate["compiler_support_root"] = json!("/compiler/other");
        });
        assert_eq!(compiler_root.code(), ReportErrorCode::Reference);

        let provider_bytes = active_product_failure(|report| {
            let provider = &mut report["product_proof"]["provider_artifact"];
            provider["top_level_rlib"]["content_digest"] = json!(digest('0'));
        });
        assert_eq!(provider_bytes.code(), ReportErrorCode::Reference);

        let cargo_artifact = active_product_failure(|report| {
            let closure = &mut report["product_proof"]["closures"][0];
            let events = &mut closure["cargo_capture"]["artifact_events"];
            events[0]["filenames"][0] = json!("/scratch/other.rlib");
        });
        assert_eq!(cargo_artifact.code(), ReportErrorCode::Reference);

        let exposure_digest = active_product_failure(|report| {
            let exposure =
                &mut report["product_proof"]["executions"][0]["exposure"][0];
            exposure["inventory_digest"] = json!(digest('0'));
        });
        assert_eq!(exposure_digest.code(), ReportErrorCode::Reference);
    }

    fn assert_extra_unit_argument_rejected(argument: &str) {
        let failure = active_product_failure(|report| {
            append_unit_argument(report, 0, argument);
        });
        assert_eq!(failure.code(), ReportErrorCode::Reference, "{argument}");
    }

    fn append_unit_argument(report: &mut Value, index: usize, value: &str) {
        let unit = &mut report["product_proof"]["compiler_units"][index];
        unit["original_argv"].as_array_mut().unwrap().push(json!(value));
        let effective = unit["effective_argv"].as_array_mut().unwrap();
        let sysroot = effective.len() - 2;
        effective.insert(sysroot, json!(value));
    }

    #[test]
    fn active_product_limits_reject_identity_correct_mutants() {
        let cargo = active_product_failure(|report| {
            let capture =
                &mut report["product_proof"]["closures"][0]["cargo_capture"];
            capture["stdout_bytes"] = json!(16_777_217);
        });
        assert_eq!(cargo.code(), ReportErrorCode::Limit);

        let dep_info = active_product_failure(|report| {
            report["product_proof"]["compiler_units"][0]["dep_info_bytes"] =
                json!(4_194_305);
        });
        assert_eq!(dep_info.code(), ReportErrorCode::Limit);

        let argv = active_product_failure(|report| {
            report["limits"]["product_execution"]["wrapper_argv_bytes"] =
                json!(1);
        });
        assert_eq!(argv.code(), ReportErrorCode::Limit);

        let public = active_product_failure(|report| {
            report["limits"]["product_execution"]["public_item_count"] =
                json!(4);
        });
        assert_eq!(public.code(), ReportErrorCode::Limit);
    }

    #[test]
    fn active_product_isolation_rejects_identity_correct_mutants() {
        let cleanup = active_product_failure(|report| {
            let cleanup =
                &mut report["product_proof"]["executions"][0]["cleanup"];
            cleanup["processes_absent"] = json!(false);
        });
        assert_eq!(cleanup.code(), ReportErrorCode::Reference);

        let namespace = active_product_failure(|report| {
            report["product_proof"]["executions"][0]["namespaces"]["uid_map"] =
                json!("unproved");
        });
        assert_eq!(namespace.code(), ReportErrorCode::Reference);

        for field in ["setgroups_denied", "parent_death_signal"] {
            assert_false_namespace_fact(field);
        }

        let network = active_product_failure(|report| {
            report["product_proof"]["executions"][0]["network"]["interfaces"] =
                json!(["eth0"]);
        });
        assert_eq!(network.code(), ReportErrorCode::Reference);

        let policy_digest = active_product_failure(|report| {
            let network =
                &mut report["product_proof"]["executions"][0]["network"];
            network["policy_observation_digest"] = json!(digest('0'));
        });
        assert_eq!(policy_digest.code(), ReportErrorCode::Reference);

        let policy_basis = active_product_failure(|report| {
            let network =
                &mut report["product_proof"]["executions"][0]["network"];
            network["policy_basis"] = json!("unbounded-assumption");
        });
        assert_eq!(policy_basis.code(), ReportErrorCode::Reference);

        let missing_null = active_product_failure(|report| {
            report["product_proof"]["executions"][0]["exposure"]
                .as_array_mut()
                .unwrap()
                .retain(|entry| entry["kind"] != "character_device");
        });
        assert_eq!(missing_null.code(), ReportErrorCode::Reference);

        let null_nodev = active_product_failure(|report| {
            let values = report["product_proof"]["executions"][0]["exposure"]
                .as_array_mut()
                .unwrap();
            let null = values
                .iter_mut()
                .find(|entry| entry["kind"] == "character_device")
                .unwrap();
            let flags = &mut null["flags"];
            flags.as_array_mut().unwrap().push(json!("nodev"));
        });
        assert_eq!(null_nodev.code(), ReportErrorCode::Reference);

        let tool_source = active_product_failure(|report| {
            let values = report["product_proof"]["executions"][0]["exposure"]
                .as_array_mut()
                .unwrap();
            let cargo = values
                .iter_mut()
                .find(|entry| entry["purpose"] == "Cargo executable")
                .unwrap();
            cargo["source"] = json!("/other-cargo");
        });
        assert_eq!(tool_source.code(), ReportErrorCode::Reference);

        let missing_repository_input =
            active_product_failure(remove_product_manifest_exposure);
        assert_eq!(missing_repository_input.code(), ReportErrorCode::Reference);
    }

    #[test]
    fn active_product_schema_requires_capture_identity_without_pids() {
        let cargo = active_product_failure(|report| {
            report["product_proof"]["closures"][0]
                .as_object_mut()
                .unwrap()
                .remove("cargo_capture");
        });
        assert_eq!(cargo.code(), ReportErrorCode::Schema);

        for field in
            ["dep_info_digest", "dep_info_bytes", "trace_digest", "trace_bytes"]
        {
            assert_missing_active_unit_field(field);
        }

        for field in ["setgroups_denied", "parent_death_signal"] {
            assert_missing_execution_field("namespaces", field);
        }
        for field in ["policy_observation_digest", "policy_basis"] {
            assert_missing_execution_field("network", field);
        }

        let pid = active_product_failure(|report| {
            let process_group =
                &mut report["product_proof"]["executions"][0]["process_group"];
            process_group["leader_pid"] = json!(1234);
        });
        assert_eq!(pid.code(), ReportErrorCode::Schema);

        let report_schema: Value = serde_json::from_slice(
            crate::schema::embedded_report_schema_bytes(),
        )
        .unwrap();
        assert_closed_group_shape(
            &report_schema,
            "/$defs/process_group_evidence",
        );
    }

    #[test]
    fn active_product_probe_mutants_fail_closed() {
        let missing = active_product_failure(|report| {
            report["product_proof"]["closures"][0]["cargo_capture"]["probes"]
                .as_array_mut()
                .unwrap()
                .pop();
        });
        assert_eq!(missing.code(), ReportErrorCode::Reference);

        let extra = active_product_failure(|report| {
            report["product_proof"]["closures"][1]["cargo_capture"]["probes"]
                .as_array_mut()
                .unwrap()
                .push(active_probe("version_probe"));
        });
        assert_eq!(extra.code(), ReportErrorCode::Reference);

        let order = active_product_failure(|report| {
            report["product_proof"]["closures"][0]["cargo_capture"]["probes"]
                .as_array_mut()
                .unwrap()
                .swap(0, 1);
        });
        assert_eq!(order.code(), ReportErrorCode::Reference);

        let argv = active_product_failure(|report| {
            report["product_proof"]["closures"][0]["cargo_capture"]["probes"]
                [0]["effective_argv"][3] = json!("/other");
        });
        assert_eq!(argv.code(), ReportErrorCode::Reference);

        let digest = active_product_failure(|report| {
            report["product_proof"]["closures"][0]["cargo_capture"]["probes"]
                [0]["trace_digest"] = json!("not-a-digest");
        });
        assert_eq!(digest.code(), ReportErrorCode::Schema);

        let limit = active_product_failure(|report| {
            report["product_proof"]["closures"][0]["cargo_capture"]["probes"]
                [0]["trace_bytes"] = json!(16_777_217);
        });
        assert_eq!(limit.code(), ReportErrorCode::Limit);
    }

    fn assert_missing_active_unit_field(field: &str) {
        let failure = active_product_failure(|report| {
            report["product_proof"]["compiler_units"][0]
                .as_object_mut()
                .unwrap()
                .remove(field);
        });
        assert_eq!(failure.code(), ReportErrorCode::Schema, "{field}");
    }

    #[test]
    fn candidate_product_empty_round_trip_preserves_ordered_argv() {
        let original = candidate_report();
        let ordered =
            original["identity_inputs"]["cargo"]["version_argv"].clone();
        let original_exclusions =
            original["source_inventory"]["exclusions"].clone();
        let built = super::build(original, REPORT_LIMIT).unwrap();

        assert_eq!(
            built.value["identity_inputs"]["cargo"]["version_argv"],
            ordered
        );
        assert_ne!(
            built.value["source_inventory"]["exclusions"],
            original_exclusions
        );
        assert_eq!(built.bytes.last(), Some(&b'\n'));
        let verified =
            super::verify(&built.bytes, REPORT_LIMIT, JSON_DEPTH, JSON_ITEMS)
                .unwrap();
        assert_eq!(verified.identity, built.identity);
        assert_eq!(verified.mode, "candidate");
        assert_eq!(verified.status, "pass");
    }

    #[test]
    fn metadata_argv_requires_closed_profile_and_matching_process() {
        let run_only = verify_reidentified_mutant(|report| {
            report["cargo_graph"]["metadata_runs"][0]["command_argv"][0] =
                json!("/bin/false");
        });
        assert_eq!(run_only.code(), ReportErrorCode::Reference);

        let run_and_process = verify_reidentified_mutant(|report| {
            report["cargo_graph"]["metadata_runs"][0]["command_argv"][0] =
                json!("/bin/false");
            report["processes"][0]["argv"][0] = json!("/bin/false");
        });
        assert_eq!(run_and_process.code(), ReportErrorCode::Reference);
    }

    #[test]
    fn workspace_paths_must_match_the_single_root() {
        let wrong_root = verify_reidentified_mutant(|report| {
            report["cargo_graph"]["workspaces"][0]["root"] = json!("nested");
        });
        assert_eq!(wrong_root.code(), ReportErrorCode::Reference);
        assert!(wrong_root.message().contains("manifest parent"));

        let wrong_manifest = verify_reidentified_mutant(|report| {
            report["cargo_graph"]["workspaces"][0]["manifest"] =
                json!("nested/Cargo.toml");
        });
        assert_eq!(wrong_manifest.code(), ReportErrorCode::Reference);
        assert!(wrong_manifest.message().contains("manifest parent"));

        let wrong_lockfile = verify_reidentified_mutant(|report| {
            report["cargo_graph"]["workspaces"][0]["lockfile"] =
                json!("nested/Cargo.lock");
        });
        assert_eq!(wrong_lockfile.code(), ReportErrorCode::Reference);
        assert!(wrong_lockfile.message().contains("lockfile parent"));

        let duplicate_root = verify_reidentified_mutant(|report| {
            let mut workspace = report["cargo_graph"]["workspaces"][0].clone();
            workspace["id"] = json!("other");
            report["cargo_graph"]["workspaces"]
                .as_array_mut()
                .unwrap()
                .push(workspace);
        });
        assert_eq!(duplicate_root.code(), ReportErrorCode::Reference);
        assert!(duplicate_root.message().contains("duplicate workspace root"));

        let non_root = verify_reidentified_mutant(|report| {
            let workspace = &mut report["cargo_graph"]["workspaces"][0];
            workspace["root"] = json!("nested");
            workspace["manifest"] = json!("nested/Cargo.toml");
            workspace["lockfile"] = json!("nested/Cargo.lock");
            workspace["members"] = json!(["nested/lib/fixture"]);
        });
        assert_eq!(non_root.code(), ReportErrorCode::Reference);
        assert!(non_root.message().contains("exactly one '.' root"));
    }

    #[test]
    fn standalone_verifier_rejects_order_identity_and_final_lf_mutants() {
        let built = super::build(candidate_report(), REPORT_LIMIT).unwrap();

        let mut unordered = built.value.clone();
        unordered["source_inventory"]["exclusions"]
            .as_array_mut()
            .unwrap()
            .reverse();
        let unordered = serialized(&unordered);
        let failure =
            super::verify(&unordered, REPORT_LIMIT, JSON_DEPTH, JSON_ITEMS)
                .unwrap_err();
        assert_eq!(failure.code(), ReportErrorCode::Canonical);
        assert!(failure.message().contains("canonical"));

        let mut identity = built.value.clone();
        identity["report_identity"] = json!(digest('9'));
        let identity = serialized(&identity);
        let failure =
            super::verify(&identity, REPORT_LIMIT, JSON_DEPTH, JSON_ITEMS)
                .unwrap_err();
        assert_eq!(failure.code(), ReportErrorCode::Identity);
        assert!(failure.message().contains("identity mismatch"));

        let without_lf = &built.bytes[..built.bytes.len() - 1];
        let failure =
            super::verify(without_lf, REPORT_LIMIT, JSON_DEPTH, JSON_ITEMS)
                .unwrap_err();
        assert_eq!(failure.code(), ReportErrorCode::Canonical);
        assert!(failure.message().contains("canonical LF"));
    }

    #[test]
    fn stable_error_categories_cover_envelope_schema_and_semantics() {
        let envelope =
            super::verify(b"{\n", REPORT_LIMIT, JSON_DEPTH, JSON_ITEMS)
                .unwrap_err();
        assert_eq!(envelope.code(), ReportErrorCode::Envelope);
        assert_eq!(envelope.code().as_str(), "envelope");

        let mut schema = candidate_report();
        schema.as_object_mut().unwrap().remove("subject");
        let schema = super::build(schema, REPORT_LIMIT).unwrap_err();
        assert_eq!(schema.code(), ReportErrorCode::Schema);

        let semantic = verify_reidentified_mutant(|report| {
            report["result"]["status"] = json!("fail");
        });
        assert_eq!(semantic.code(), ReportErrorCode::Semantic);
    }

    #[test]
    fn retained_source_inventory_limits_and_tree_are_typed() {
        let duplicate = verify_reidentified_mutant(|report| {
            let mut extra = classification(report, "Cargo.toml").clone();
            extra["mode"] = json!("100755");
            report["source_inventory"]["classifications"]
                .as_array_mut()
                .unwrap()
                .push(extra);
        });
        assert_eq!(duplicate.code(), ReportErrorCode::Reference);

        for field in ["source_entry_count", "source_file_count"] {
            assert_source_count_limit(field);
        }

        let file_bytes = verify_reidentified_mutant(|report| {
            report["limits"]["inventory"]["source_file_bytes"] = json!(1);
            classification_mut(report, "Cargo.toml")["size_bytes"] = json!(2);
        });
        assert_eq!(file_bytes.code(), ReportErrorCode::Limit);

        let total_bytes = verify_reidentified_mutant(|report| {
            report["limits"]["inventory"]["source_total_bytes"] = json!(1);
        });
        assert_eq!(total_bytes.code(), ReportErrorCode::Limit);

        let path_bytes = verify_reidentified_mutant(|report| {
            report["limits"]["inventory"]["path_bytes"] = json!(8);
        });
        assert_eq!(path_bytes.code(), ReportErrorCode::Limit);

        let tree = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["source_tree_digest"] =
                json!(digest('0'));
        });
        assert_eq!(tree.code(), ReportErrorCode::Identity);
    }

    #[test]
    fn retained_winning_rule_and_upstream_laws_are_typed() {
        let ancestor = verify_reidentified_mutant(|report| {
            classification_mut(report, "Cargo.toml")["winning_rule"]["prefix"] =
                json!("other");
        });
        assert_eq!(ancestor.code(), ReportErrorCode::Reference);

        let inconsistent_index = verify_reidentified_mutant(|report| {
            let index =
                classification(report, "Cargo.lock")["winning_rule"]["index"]
                    .clone();
            classification_mut(report, "Cargo.toml")["winning_rule"]["index"] =
                index;
        });
        assert_eq!(inconsistent_index.code(), ReportErrorCode::Reference);

        let reused_projection = verify_reidentified_mutant(|report| {
            reuse_winning_projection(report);
        });
        assert_eq!(reused_projection.code(), ReportErrorCode::Reference);

        let wrong_subject = verify_reidentified_mutant(|report| {
            let classification = classification_mut(report, "Cargo.lock");
            classification["upstream"]["digest_subject"] =
                json!({"kind": "classified_content"});
        });
        assert_eq!(wrong_subject.code(), ReportErrorCode::Reference);

        let missing_subject = verify_reidentified_mutant(|report| {
            classification_mut(report, "Cargo.lock")["upstream"] =
                generated_upstream(["missing.txt"]);
        });
        assert_eq!(missing_subject.code(), ReportErrorCode::Reference);

        let generated_subject = forbidden_upstream_subject("tool");
        assert_eq!(generated_subject.code(), ReportErrorCode::Reference);
        let build_output_subject = forbidden_upstream_subject("build_output");
        assert_eq!(build_output_subject.code(), ReportErrorCode::Reference);
    }

    #[test]
    fn retained_source_path_limits_cover_rule_and_upstream_paths() {
        let long_path = "x".repeat(4_097);
        let rule = verify_reidentified_mutant(|report| {
            classification_mut(report, "Cargo.toml")["winning_rule"]["prefix"] =
                json!(long_path);
        });
        assert_eq!(rule.code(), ReportErrorCode::Limit);

        let long_path = "x".repeat(4_097);
        let upstream = verify_reidentified_mutant(|report| {
            classification_mut(report, "Cargo.lock")["upstream"] =
                generated_upstream([long_path.as_str()]);
        });
        assert_eq!(upstream.code(), ReportErrorCode::Limit);
    }

    #[test]
    fn retained_artifact_and_lock_identity_mutants_fail_typed() {
        for field in [
            "source_classification",
            "workspace_topology",
            "entrypoint_reviews",
            "dependency_reviews",
            "product_closures",
            "execution_tool_reviews",
        ] {
            let failure = mutated_artifact_path(field);
            assert_eq!(failure.code(), ReportErrorCode::Identity, "{field}");
            assert_artifact_crosslinks(field);
        }
        let report_schema = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["report_schema"]["path"] =
                json!("conformance/wrong.schema.json");
        });
        assert_eq!(report_schema.code(), ReportErrorCode::Identity);

        let lock = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["lockfiles"][0]["path"] =
                json!("other/Cargo.lock");
        });
        assert_eq!(lock.code(), ReportErrorCode::Reference);

        let lock_digest = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["lockfiles"][0]["digest"] =
                json!(digest('0'));
        });
        assert_eq!(lock_digest.code(), ReportErrorCode::Reference);

        let artifact_source = verify_reidentified_source_mutant(|report| {
            classification_mut(report, crate::POLICY_KEY)["content_digest"] =
                json!(digest('0'));
        });
        assert_eq!(artifact_source.code(), ReportErrorCode::Reference);

        let schema_source = verify_reidentified_source_mutant(|report| {
            let classification =
                classification_mut(report, crate::POLICY_SCHEMA_KEY);
            classification["content_digest"] = json!(digest('0'));
        });
        assert_eq!(schema_source.code(), ReportErrorCode::Reference);

        let report_source = verify_reidentified_source_mutant(|report| {
            let classification =
                classification_mut(report, crate::REPORT_SCHEMA_KEY);
            classification["content_digest"] = json!(digest('0'));
        });
        assert_eq!(report_source.code(), ReportErrorCode::Reference);

        let lock_source = verify_reidentified_source_mutant(|report| {
            classification_mut(report, "Cargo.lock")["content_digest"] =
                json!(digest('0'));
        });
        assert_eq!(lock_source.code(), ReportErrorCode::Reference);
    }

    #[test]
    fn retained_tool_identity_mutants_fail_typed() {
        let baseline = candidate_report();
        assert!(
            baseline["identity_inputs"]["cargo"]["version"]
                .as_str()
                .unwrap()
                .contains('\n')
        );
        assert!(
            baseline["identity_inputs"]["rustc"]["version"]
                .as_str()
                .unwrap()
                .contains('\n')
        );

        let argv = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["cargo"]["version_argv"] =
                json!(["--verbose", "--version"]);
        });
        assert_eq!(argv.code(), ReportErrorCode::Identity);

        let size = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["checker"]["executable_size_bytes"] =
                json!(100_663_297_u64);
        });
        assert_eq!(size.code(), ReportErrorCode::Limit);

        let version_digest = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["rustc"]["version_digest"] =
                json!(digest('f'));
        });
        assert_eq!(version_digest.code(), ReportErrorCode::Identity);

        let runtime = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["cargo"]["runtime_inputs"] = json!([
                runtime_input("/runtime/lib.so", "/lib/lib.so", '1', 1),
                runtime_input("/runtime/lib.so", "/lib/lib.so", '2', 1)
            ]);
        });
        assert_eq!(runtime.code(), ReportErrorCode::Identity);

        let runtime_limit = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["cargo"]["runtime_inputs"] =
                json!([runtime_input(
                    "/runtime/lib.so",
                    "/lib/lib.so",
                    '1',
                    100_663_297,
                )]);
        });
        assert_eq!(runtime_limit.code(), ReportErrorCode::Limit);

        for version in [
            "cargo 1.92.0\rrelease: 1.92.0",
            "cargo 1.92.0\0release: 1.92.0",
            "\ncargo 1.92.0",
            "cargo 1.92.0\n",
        ] {
            assert_invalid_retained_version(version);
        }

        let mut shared = candidate_report();
        let input = runtime_input("/runtime/lib.so", "/lib/lib.so", '1', 1);
        shared["identity_inputs"]["checker"]["runtime_inputs"] = json!([input]);
        shared["identity_inputs"]["cargo"]["runtime_inputs"] = json!([input]);
        super::build(shared, REPORT_LIMIT).unwrap();

        for field in ["content_digest", "size_bytes"] {
            assert_runtime_identity_conflict(field);
        }

        let mut shared_executable = candidate_report();
        set_cargo_executable(&mut shared_executable, "/checker");
        super::build(shared_executable, REPORT_LIMIT).unwrap();

        let mut shared_runtime = candidate_report();
        shared_runtime["identity_inputs"]["checker"]["runtime_inputs"] =
            json!([runtime_input("/cargo", "/lib/cargo.so", 'a', 1)]);
        super::build(shared_runtime, REPORT_LIMIT).unwrap();

        let executable_conflict = verify_reidentified_mutant(|report| {
            set_cargo_executable(report, "/checker");
            set_cargo_digest(report, &digest('b'));
        });
        assert_eq!(executable_conflict.code(), ReportErrorCode::Identity);

        assert_executable_runtime_conflict('1', 1);
        assert_executable_runtime_conflict('a', 2);

        let long_path = format!("/{}", "x".repeat(4_097));
        let tool_path = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["checker"]["canonical_path"] =
                json!(long_path);
        });
        assert_eq!(tool_path.code(), ReportErrorCode::Limit);

        let long_path = format!("/{}", "x".repeat(4_097));
        let runtime_path = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["cargo"]["runtime_inputs"] =
                json!([runtime_input(&long_path, "/lib/long.so", '1', 1)]);
        });
        assert_eq!(runtime_path.code(), ReportErrorCode::Limit);
    }

    #[test]
    fn exact_git_mutant_fails_typed() {
        super::build(exact_report(), REPORT_LIMIT).unwrap();
        let git = verify_reidentified_report_mutant(exact_report(), |report| {
            report["identity_inputs"]["git"]["version_argv"] =
                json!(["--verbose"]);
        });
        assert_eq!(git.code(), ReportErrorCode::Identity);
    }

    #[test]
    fn exact_git_captures_require_authority_and_phase_agreement() {
        let phase =
            verify_reidentified_report_mutant(exact_report(), |report| {
                let process =
                    process_by_purpose(report, "git_final_index_entries");
                process["stdout"]["digest"] = json!(digest('0'));
            });
        assert_eq!(phase.code(), ReportErrorCode::Reference);
        assert!(phase.message().contains("proof phases"));

        let phase_bytes =
            verify_reidentified_report_mutant(exact_report(), |report| {
                let process = process_by_purpose(
                    report,
                    "git_final_info_attributes_path",
                );
                process["stdout"]["bytes"] = json!(1);
            });
        assert_eq!(phase_bytes.code(), ReportErrorCode::Reference);
        assert!(phase_bytes.message().contains("proof phases"));

        let authority =
            verify_reidentified_report_mutant(exact_report(), |report| {
                set_process_digests(
                    report,
                    &["git_root", "git_pre_metadata_root", "git_final_root"],
                    &digest('0'),
                );
            });
        assert_eq!(authority.code(), ReportErrorCode::Reference);
        assert!(authority.message().contains("retained authority"));
    }

    #[test]
    fn semantic_verifier_rejects_product_empty_and_reference_mutants() {
        let failure = verify_reidentified_source_mutant(|product| {
            let classification =
                &mut product["source_inventory"]["classifications"][0];
            classification["source_owner"] =
                json!({"kind": "product", "role": "Foundation"});
            classification["source_use"] = json!("production");
        });
        assert_eq!(failure.code(), ReportErrorCode::Reference);
        assert!(failure.message().contains("Product-empty"));
        assert!(!failure.message().contains("identity mismatch"));

        let mut duplicate_workspace = candidate_report();
        duplicate_workspace["cargo_graph"]["workspaces"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id": "root",
                "partition": "repository_tooling",
                "root": "nested",
                "manifest": "nested/Cargo.toml",
                "lockfile": "nested/Cargo.lock",
                "members": []
            }));
        assert!(
            super::build(duplicate_workspace, REPORT_LIMIT)
                .unwrap_err()
                .message()
                .contains("duplicate workspace")
        );

        let mut missing_profile = candidate_report();
        missing_profile["cargo_graph"]["metadata_runs"] = json!([{
            "workspace": "root",
            "profile": "missing",
            "phase": "initial",
            "command_argv": ["cargo", "metadata"],
            "stdout_digest": digest('8'),
            "metadata_digest": digest('7')
        }]);
        assert!(
            super::build(missing_profile, REPORT_LIMIT)
                .unwrap_err()
                .message()
                .contains("matching Cargo process")
        );
    }

    #[test]
    fn product_empty_rejects_unproduced_identity_evidence() {
        let substrate = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["compiler_substrates"] = json!([{
                "id": "unused-substrate",
                "environment": "unused-environment",
                "host_triple": "x86_64-unknown-linux-gnu",
                "target_triple": "x86_64-unknown-linux-gnu",
                "compiler_support_root": "/toolchain/lib",
                "target_libdir": "/toolchain/lib/target",
                "entries": [{
                    "canonical_path": "/toolchain/lib/libstd.rlib",
                    "kind": "compiler_support",
                    "content_digest": digest('1'),
                    "size_bytes": 1
                }],
                "entry_count": 1,
                "total_bytes": 1,
                "aggregate_digest": digest('2')
            }]);
        });
        assert_eq!(substrate.code(), ReportErrorCode::Reference);
        assert!(substrate.message().contains("compiler-substrate"));

        let gate = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["product_proof_gate"] = json!({
                "schema": "reovim-product-proof-gate/v1",
                "revision": "a".repeat(40),
                "report_identity": digest('3'),
                "artifact_digest": digest('4')
            });
        });
        assert_eq!(gate.code(), ReportErrorCode::Reference);
        assert!(gate.message().contains("proof gate"));
    }

    #[test]
    fn product_empty_rejects_process_product_references() {
        let failure = verify_reidentified_mutant(|report| {
            report["processes"][0]["closure"] = json!("shipping");
        });
        assert_eq!(failure.code(), ReportErrorCode::Reference);
        assert!(failure.message().contains("Product references"));
    }

    #[test]
    fn passing_process_requires_complete_owned_cleanup() {
        let group = verify_reidentified_mutant(|report| {
            report["processes"][0]["group"]["identity_validated"] =
                json!(false);
        });
        assert_eq!(group.code(), ReportErrorCode::Reference);

        let readers = verify_reidentified_mutant(|report| {
            report["processes"][0]["readers"]["stderr_joined"] = json!(false);
        });
        assert_eq!(readers.code(), ReportErrorCode::Reference);

        let bytes = verify_reidentified_mutant(|report| {
            report["processes"][0]["stdout"]["bytes"] = json!(16_777_217);
        });
        assert_eq!(bytes.code(), ReportErrorCode::Limit);

        let kill = verify_reidentified_mutant(|report| {
            report["processes"][0]["kill"]["attempted"] = json!(true);
        });
        assert_eq!(kill.code(), ReportErrorCode::Reference);
    }

    #[test]
    fn current_process_purpose_set_and_identity_are_exact() {
        let missing = verify_reidentified_mutant(|report| {
            remove_process(report, "checker_version");
        });
        assert_eq!(missing.code(), ReportErrorCode::Reference);

        let duplicate = verify_reidentified_mutant(|report| {
            let process = process_by_purpose(report, "checker_version").clone();
            report["processes"].as_array_mut().unwrap().push(process);
        });
        assert!(matches!(
            duplicate.code(),
            ReportErrorCode::Schema | ReportErrorCode::Reference
        ));

        let unknown = verify_reidentified_mutant(|report| {
            process_by_purpose(report, "checker_version")["purpose"] =
                json!("arbitrary_process");
        });
        assert_eq!(unknown.code(), ReportErrorCode::Reference);

        let digest = verify_reidentified_mutant(|report| {
            process_by_purpose(report, "cargo_version")["stdout"]["digest"] =
                json!(digest('0'));
        });
        assert_eq!(digest.code(), ReportErrorCode::Reference);

        let candidate_git = verify_reidentified_mutant(|report| {
            report["processes"]
                .as_array_mut()
                .unwrap()
                .push(git_process("git_version"));
        });
        assert_eq!(candidate_git.code(), ReportErrorCode::Reference);

        let exact_git =
            verify_reidentified_report_mutant(exact_report(), |report| {
                process_by_purpose(report, "git_head")["argv"] =
                    json!(["/git", "rev-parse", "HEAD"]);
            });
        assert_eq!(exact_git.code(), ReportErrorCode::Reference);

        let final_missing =
            verify_reidentified_report_mutant(exact_report(), |report| {
                remove_process(report, "git_final_version");
            });
        assert_eq!(final_missing.code(), ReportErrorCode::Reference);
    }

    #[test]
    fn workspace_package_owner_and_version_drift_fail_typed() {
        let owner = verify_reidentified_mutant(|report| {
            report["cargo_graph"]["workspace_packages"][0]["owners"] =
                json!([{"kind": "product", "role": "Foundation"}]);
        });
        assert_eq!(owner.code(), ReportErrorCode::Reference);

        let mut two_profiles = candidate_report();
        add_alternate_profile(&mut two_profiles);
        let version =
            verify_reidentified_report_mutant(two_profiles, |report| {
                let packages = report["cargo_graph"]["workspace_packages"]
                    .as_array_mut()
                    .unwrap();
                packages[1]["version"] = json!("0.2.0");
            });
        assert_eq!(version.code(), ReportErrorCode::Reference);
    }

    #[test]
    fn cargo_edge_authority_and_identity_mutants_fail_closed() {
        let owner = verify_reidentified_mutant(|report| {
            report["cargo_graph"]["edges"][0]["consumer_owners"] =
                json!([{"kind": "product", "role": "Foundation"}]);
        });
        assert_eq!(owner.code(), ReportErrorCode::Reference);

        let decision = verify_reidentified_mutant(|report| {
            report["cargo_graph"]["edges"][0]["decision"] =
                json!("forbidden_partition");
        });
        assert_eq!(decision.code(), ReportErrorCode::Reference);

        let duplicate = verify_reidentified_mutant(|report| {
            let edge = report["cargo_graph"]["edges"][0].clone();
            report["cargo_graph"]["edges"].as_array_mut().unwrap().push(edge);
        });
        assert!(matches!(
            duplicate.code(),
            ReportErrorCode::Schema | ReportErrorCode::Reference
        ));
    }

    #[test]
    fn reachability_requires_exact_recomputed_closure() {
        let erased = verify_reidentified_mutant(|report| {
            report["cargo_graph"]["reachability"][0]["members"] =
                json!(["fixture 0.1.0 (workspace)"]);
        });
        assert_eq!(erased.code(), ReportErrorCode::Reference);

        let invented = verify_reidentified_mutant(|report| {
            report["cargo_graph"]["reachability"][0]["members"]
                .as_array_mut()
                .unwrap()
                .push(json!("invented 9.9.9 (workspace)"));
        });
        assert_eq!(invented.code(), ReportErrorCode::Reference);

        let missing = verify_reidentified_mutant(|report| {
            report["cargo_graph"]["reachability"] = json!([]);
        });
        assert_eq!(missing.code(), ReportErrorCode::Reference);

        let duplicate = verify_reidentified_mutant(|report| {
            let root = report["cargo_graph"]["reachability"][0].clone();
            report["cargo_graph"]["reachability"]
                .as_array_mut()
                .unwrap()
                .push(root);
        });
        assert!(matches!(
            duplicate.code(),
            ReportErrorCode::Schema | ReportErrorCode::Reference
        ));
    }

    #[test]
    fn source_exclusion_mutants_fail_closed() {
        for field in ["prefix", "kind"] {
            assert_exclusion_schema_mutant(field);
        }

        let missing = verify_schema_mutant(|report| {
            report["source_inventory"]["exclusions"]
                .as_array_mut()
                .unwrap()
                .pop();
        });
        assert_eq!(missing.code(), ReportErrorCode::Schema);

        let duplicate = verify_schema_mutant(|report| {
            let exclusion = report["source_inventory"]["exclusions"][0].clone();
            report["source_inventory"]["exclusions"].as_array_mut().unwrap()
                [1] = exclusion;
        });
        assert_eq!(duplicate.code(), ReportErrorCode::Schema);

        let extra = verify_schema_mutant(|report| {
            let exclusion = report["source_inventory"]["exclusions"][0].clone();
            report["source_inventory"]["exclusions"]
                .as_array_mut()
                .unwrap()
                .push(exclusion);
        });
        assert_eq!(extra.code(), ReportErrorCode::Schema);

        let mut direct = candidate_report();
        direct["source_inventory"]["exclusions"][0]["prefix"] =
            json!("broadened");
        let failure = super::validate_source_exclusions(&direct).unwrap_err();
        assert_eq!(failure.code(), ReportErrorCode::Reference);
    }

    fn assert_exclusion_schema_mutant(field: &str) {
        let failure = verify_schema_mutant(|report| {
            report["source_inventory"]["exclusions"][0][field] =
                json!("broadened");
        });
        assert_eq!(failure.code(), ReportErrorCode::Schema, "{field}");
    }

    #[test]
    fn product_empty_schema_omits_ephemeral_process_group_id() {
        let report_schema: Value = serde_json::from_slice(
            crate::schema::embedded_report_schema_bytes(),
        )
        .unwrap();
        assert_closed_group_shape(&report_schema, "/$defs/group_evidence");
    }

    #[test]
    fn build_and_verify_enforce_both_report_byte_limits() {
        let failure = super::build(candidate_report(), 32).unwrap_err();
        assert_eq!(failure.code(), ReportErrorCode::Limit);
        assert!(failure.message().contains("caller limit"));
        let built = super::build(candidate_report(), REPORT_LIMIT).unwrap();
        let failure = super::verify(&built.bytes, 32, JSON_DEPTH, JSON_ITEMS)
            .unwrap_err();
        assert_eq!(failure.code(), ReportErrorCode::Limit);
        assert!(failure.message().contains("caller-owned"));
    }

    fn serialized(value: &Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(value).unwrap();
        bytes.push(b'\n');
        bytes
    }

    fn mutated_artifact_path(field: &str) -> ReportError {
        verify_reidentified_report_mutant(candidate_report(), |report| {
            report["identity_inputs"][field]["path"] =
                json!("conformance/wrong.json");
        })
    }

    fn verify_reidentified_mutant(
        mutate: impl FnOnce(&mut Value),
    ) -> ReportError {
        verify_reidentified_report_mutant(candidate_report(), mutate)
    }

    fn active_product_failure(mutate: impl FnOnce(&mut Value)) -> ReportError {
        verify_reidentified_report_mutant(active_product_report(), mutate)
    }

    fn verify_reidentified_source_mutant(
        mutate: impl FnOnce(&mut Value),
    ) -> ReportError {
        let mut report = candidate_report();
        mutate(&mut report);
        refresh_source_tree_digest(&mut report);
        verify_reidentified_report_mutant(report, |_| {})
    }

    fn verify_schema_mutant(mutate: impl FnOnce(&mut Value)) -> ReportError {
        let mut report =
            super::build(candidate_report(), REPORT_LIMIT).unwrap().value;
        mutate(&mut report);
        let mut preimage = report.clone();
        preimage.as_object_mut().unwrap().remove("report_identity");
        let identity =
            crate::digest::bytes(&serde_json::to_vec(&preimage).unwrap());
        report["report_identity"] = json!(identity);
        super::verify(
            &serialized(&report),
            REPORT_LIMIT,
            JSON_DEPTH,
            JSON_ITEMS,
        )
        .unwrap_err()
    }

    fn verify_reidentified_report_mutant(
        mut report: Value,
        mutate: impl FnOnce(&mut Value),
    ) -> ReportError {
        mutate(&mut report);
        let schema_bytes = crate::schema::embedded_report_schema_bytes();
        let schema: Value = serde_json::from_slice(schema_bytes).unwrap();
        crate::schema::canonicalize_unique_arrays(&mut report, &schema)
            .unwrap();
        let mut preimage = report.clone();
        preimage.as_object_mut().unwrap().remove("report_identity");
        let identity =
            crate::digest::bytes(&serde_json::to_vec(&preimage).unwrap());
        report["report_identity"] = json!(identity);
        super::verify(
            &serialized(&report),
            REPORT_LIMIT,
            JSON_DEPTH,
            JSON_ITEMS,
        )
        .unwrap_err()
    }

    fn assert_closed_group_shape(schema: &Value, pointer: &str) {
        let group = schema.pointer(pointer).unwrap();
        assert!(group["properties"].get("leader_pid").is_none());
        assert!(group["properties"].get("pgid").is_none());
        assert!(
            group["required"]
                .as_array()
                .unwrap()
                .iter()
                .all(|field| field != "leader_pid" && field != "pgid")
        );
    }

    fn candidate_report() -> Value {
        let report_schema_digest =
            crate::digest::bytes(crate::schema::embedded_report_schema_bytes());
        let identity_inputs = candidate_identity_inputs(&report_schema_digest);
        let source_inventory = candidate_source_inventory(&identity_inputs);
        let mut report = json!({
            "schema": SCHEMA,
            "subject": {"kind": "active_repository"},
            "mode": "candidate",
            "revision": null,
            "identity_inputs": identity_inputs,
            "limits": limits(),
            "source_inventory": source_inventory,
            "cargo_graph": candidate_cargo_graph(),
            "product_proof": candidate_product_proof(),
            "processes": candidate_processes(),
            "findings": [],
            "result": {"status": "pass", "finding_count": 0},
            "report_identity": null
        });
        refresh_source_tree_digest(&mut report);
        report
    }

    fn exact_report() -> Value {
        let mut report = candidate_report();
        report["mode"] = json!("exact");
        report["revision"] = json!("a".repeat(40));
        report["identity_inputs"]["git"] = tool("/git");
        report["identity_inputs"]["git_tree_digest"] = json!(digest('9'));
        report["cargo_graph"]["metadata_runs"]
            .as_array_mut()
            .unwrap()
            .push(metadata_run("stable", "final"));
        report["processes"]
            .as_array_mut()
            .unwrap()
            .push(metadata_process("stable", "final"));
        report["processes"].as_array_mut().unwrap().extend(git_processes());
        report
    }

    fn active_product_report() -> Value {
        let mut report = exact_report();
        rename_fixture_workspace(&mut report);
        add_product_source(&mut report);
        add_product_graph(&mut report);
        add_product_identity(&mut report);
        report["product_proof"] = active_product_proof(&report);
        report["processes"].as_array_mut().unwrap().push(product_process());
        refresh_source_tree_digest(&mut report);
        report
    }

    fn rename_fixture_workspace(report: &mut Value) {
        replace_exact_string(report, "root", "repository-tooling");
    }

    fn replace_exact_string(value: &mut Value, from: &str, to: &str) {
        match value {
            Value::String(text) if text == from => *text = to.to_owned(),
            Value::Array(values) => replace_array_strings(values, from, to),
            Value::Object(values) => {
                replace_object_strings(values, from, to);
            }
            _ => {}
        }
    }

    fn replace_array_strings(values: &mut [Value], from: &str, to: &str) {
        for value in values {
            replace_exact_string(value, from, to);
        }
    }

    fn replace_object_strings(
        values: &mut serde_json::Map<String, Value>,
        from: &str,
        to: &str,
    ) {
        for value in values.values_mut() {
            replace_exact_string(value, from, to);
        }
    }

    fn add_product_source(report: &mut Value) {
        let files = report["source_inventory"]["classifications"]
            .as_array_mut()
            .unwrap();
        for path in ["product/Cargo.toml", "product/Cargo.lock"] {
            files.push(authority_classification(
                path,
                &digest('e'),
                files.len(),
            ));
        }
        for path in [
            "kernel/foundation/Cargo.toml",
            "kernel/foundation/src/bound.rs",
            "kernel/foundation/src/carrier.rs",
            "kernel/foundation/src/contract.rs",
            "kernel/foundation/src/identity.rs",
            "kernel/foundation/src/lib.rs",
        ] {
            let mut item =
                authority_classification(path, &digest('e'), files.len());
            set_product_classification(&mut item, "Foundation", "production");
            files.push(item);
        }
        for path in [
            "tests/foundation-public-consumer/Cargo.toml",
            "tests/foundation-public-consumer/src/lib.rs",
        ] {
            let mut item =
                authority_classification(path, &digest('f'), files.len());
            set_product_classification(&mut item, "Kernel Interface", "test");
            files.push(item);
        }
    }

    fn set_product_classification(
        item: &mut Value,
        role: &str,
        use_name: &str,
    ) {
        item["source_owner"] = json!({"kind": "product", "role": role});
        item["source_use"] = json!(use_name);
    }

    fn add_product_graph(report: &mut Value) {
        report["identity_inputs"]["metadata_profiles"]
            .as_array_mut()
            .unwrap()
            .push(product_metadata_profile());
        report["identity_inputs"]["lockfiles"].as_array_mut().unwrap().push(
            json!({
                "workspace": "product",
                "path": "product/Cargo.lock",
                "digest": digest('e')
            }),
        );
        let graph = &mut report["cargo_graph"];
        graph["metadata_runs"]
            .as_array_mut()
            .unwrap()
            .push(product_metadata_run());
        graph["metadata_runs"]
            .as_array_mut()
            .unwrap()
            .push(product_final_metadata_run());
        graph["workspaces"].as_array_mut().unwrap().push(product_workspace());
        graph["workspace_packages"]
            .as_array_mut()
            .unwrap()
            .extend(product_workspace_packages());
        graph["edges"].as_array_mut().unwrap().push(product_cargo_edge());
        graph["reachability"]
            .as_array_mut()
            .unwrap()
            .extend(product_reachability());
        report["processes"]
            .as_array_mut()
            .unwrap()
            .push(product_metadata_process());
        report["processes"]
            .as_array_mut()
            .unwrap()
            .push(product_final_metadata_process());
    }

    fn product_metadata_profile() -> Value {
        json!({
            "workspace": "product",
            "profile": "release",
            "target_triple": "x86_64-unknown-linux-gnu",
            "all_features": false,
            "no_default_features": false,
            "features": [],
            "metadata_digest": digest('b')
        })
    }

    fn product_metadata_run() -> Value {
        json!({
            "workspace": "product",
            "profile": "release",
            "phase": "initial",
            "command_argv": product_metadata_argv(),
            "stdout_digest": digest('b'),
            "metadata_digest": digest('b')
        })
    }

    fn product_final_metadata_run() -> Value {
        let mut run = product_metadata_run();
        run["phase"] = json!("final");
        run
    }

    fn product_metadata_argv() -> Vec<&'static str> {
        vec![
            "/toolchain/bin/cargo",
            "metadata",
            "--quiet",
            "--format-version",
            "1",
            "--manifest-path",
            "/repo/product/Cargo.toml",
            "--locked",
            "--offline",
            "--filter-platform",
            "x86_64-unknown-linux-gnu",
        ]
    }

    fn product_metadata_process() -> Value {
        let mut process = passing_process();
        process["purpose"] = json!("cargo_metadata");
        process["workspace"] = json!("product");
        process["profile"] = json!("release");
        process["executable_digest"] = json!(digest('a'));
        process["argv"] = json!(product_metadata_argv());
        process["stdout"]["digest"] = json!(digest('b'));
        process
    }

    fn product_final_metadata_process() -> Value {
        let mut process = product_metadata_process();
        process["purpose"] = json!("cargo_metadata_final");
        process
    }

    fn product_workspace() -> Value {
        json!({
            "id": "product",
            "partition": "product",
            "root": "product",
            "manifest": "product/Cargo.toml",
            "lockfile": "product/Cargo.lock",
            "members": [
                "kernel/foundation",
                "tests/foundation-public-consumer"
            ]
        })
    }

    fn product_workspace_packages() -> Vec<Value> {
        vec![
            json!({
                "workspace": "product",
                "profile": "release",
                "id": "foundation",
                "name": "reovim-foundation",
                "version": "0.16.0-dev",
                "manifest": "kernel/foundation/Cargo.toml",
                "owners": [{"kind": "product", "role": "Foundation"}],
                "targets": [{
                    "id": "library",
                    "name": "reovim_foundation",
                    "kind": ["lib"],
                    "crate_types": ["lib"],
                    "src_path": "kernel/foundation/src/lib.rs"
                }]
            }),
            json!({
                "workspace": "product",
                "profile": "release",
                "id": "foundation-public-consumer",
                "name": "reovim-foundation-public-consumer",
                "version": "0.16.0-dev",
                "manifest": "tests/foundation-public-consumer/Cargo.toml",
                "owners": [{
                    "kind": "product",
                    "role": "Kernel Interface"
                }],
                "targets": [{
                    "id": "library",
                    "name": "reovim_foundation_public_consumer",
                    "kind": ["lib"],
                    "crate_types": ["lib"],
                    "src_path": "tests/foundation-public-consumer/src/lib.rs"
                }]
            }),
        ]
    }

    fn product_cargo_edge() -> Value {
        json!({
            "workspace": "product",
            "profile": "release",
            "consumer": {
                "kind": "workspace_package",
                "workspace": "product",
                "package": "foundation-public-consumer"
            },
            "provider": {
                "kind": "workspace_package",
                "workspace": "product",
                "package": "foundation"
            },
            "dependency_name": "reovim_foundation",
            "kind": "normal",
            "target": null,
            "consumer_owners": [{
                "kind": "product",
                "role": "Kernel Interface"
            }],
            "provider_owners": [{
                "kind": "product",
                "role": "Foundation"
            }],
            "decision": "admitted_product_edge"
        })
    }

    fn product_reachability() -> Vec<Value> {
        vec![
            json!({
                "workspace": "product",
                "profile": "release",
                "package": "foundation",
                "members": ["reovim-foundation 0.16.0-dev (workspace)"]
            }),
            json!({
                "workspace": "product",
                "profile": "release",
                "package": "foundation-public-consumer",
                "members": [
                    "reovim-foundation 0.16.0-dev (workspace)",
                    concat!(
                        "reovim-foundation-public-consumer 0.16.0-dev ",
                        "(workspace)"
                    )
                ]
            }),
        ]
    }

    fn add_product_identity(report: &mut Value) {
        replace_exact_string(report, "/cargo", "/toolchain/bin/cargo");
        replace_exact_string(report, "/rustc", "/toolchain/bin/rustc");
        let runtime = runtime_input("/runtime/libc.so", "/lib/libc.so", '7', 1);
        for tool in ["checker", "cargo", "rustc"] {
            report["identity_inputs"][tool]["runtime_inputs"] =
                json!([runtime.clone()]);
        }
        let entries = json!([
            {
                "canonical_path": "/compiler/lib/librustc_driver.so",
                "kind": "compiler_support",
                "content_digest": digest('8'),
                "size_bytes": 1
            },
            {
                "canonical_path": concat!(
                    "/sysroot/lib/rustlib/",
                    "x86_64-unknown-linux-gnu/lib/libstd.rlib"
                ),
                "kind": "target_libdir",
                "content_digest": digest('9'),
                "size_bytes": 1
            }
        ]);
        let aggregate = crate::digest::bytes(
            &serde_json::to_vec(entries.as_array().unwrap()).unwrap(),
        );
        report["identity_inputs"]["compiler_substrates"] = json!([{
            "id": "rust-test-substrate",
            "environment": "linux-product-test",
            "host_triple": "x86_64-unknown-linux-gnu",
            "target_triple": "x86_64-unknown-linux-gnu",
            "compiler_support_root": super::PREPARED_COMPILER_SUPPORT,
            "target_libdir": super::PREPARED_TARGET_LIBDIR,
            "entries": entries,
            "entry_count": 2,
            "total_bytes": 2,
            "aggregate_digest": aggregate
        }]);
    }

    fn active_product_proof(report: &Value) -> Value {
        json!({
            "selected_execution_environment": "linux-product-test",
            "closures": active_closures(),
            "compiler_units": active_compiler_units(),
            "compiler_inputs": active_compiler_inputs(report),
            "extern_edges": [{
                "closure": "foundation-consumer",
                "unit": "consumer-unit",
                "extern_name": "reovim_foundation",
                "provider": {
                    "kind": "compiler_unit",
                    "unit": "shipping-unit",
                    "path": concat!(
                        "/scratch/build-target/build-dir/",
                        "x86_64-unknown-linux-gnu/release/deps/",
                        "libreovim_foundation-2222222222222222.rmeta"
                    )
                },
                "decision": "admitted_product_unit"
            }],
            "artifacts": active_artifacts(),
            "provider_artifact": active_provider_artifact(),
            "package_memberships": [active_package_membership(report)],
            "public_consumers": [active_public_consumer()],
            "executions": [active_execution(report)]
        })
    }

    fn active_closures() -> Vec<Value> {
        vec![
            closure_evidence(
                "foundation-shipping",
                "shipping",
                "foundation",
                "shipping-unit",
                &[],
            ),
            closure_evidence(
                "foundation-consumer",
                "validation",
                "foundation-public-consumer",
                "consumer-unit",
                &["shipping-unit"],
            ),
        ]
    }

    fn closure_evidence(
        id: &str,
        kind: &str,
        package_id: &str,
        unit: &str,
        fresh: &[&str],
    ) -> Value {
        let package_name = if kind == "shipping" {
            "reovim-foundation"
        } else {
            "reovim-foundation-public-consumer"
        };
        let artifact_events = active_cargo_artifact_events(kind);
        json!({
            "id": id,
            "kind": kind,
            "workspace": "product",
            "package": package_id,
            "target": "library",
            "execution": "foundation-proof",
            "cargo_capture": {
                "command_argv": product_build_argv(package_name),
                "raw_output_digest": digest('a'),
                "stdout_bytes": 1,
                "message_count": artifact_events.len() + 1,
                "compiled_units": [unit],
                "fresh_provider_units": fresh,
                "artifact_events": artifact_events,
                "probes": active_probes(kind)
            },
            "status": "pass"
        })
    }

    fn active_cargo_artifact_events(kind: &str) -> Vec<Value> {
        let provider_rlib = concat!(
            "/scratch/build-target/build-dir/",
            "x86_64-unknown-linux-gnu/release/deps/",
            "libreovim_foundation-2222222222222222.rlib"
        );
        let provider_rmeta = concat!(
            "/scratch/build-target/build-dir/",
            "x86_64-unknown-linux-gnu/release/deps/",
            "libreovim_foundation-2222222222222222.rmeta"
        );
        if kind == "shipping" {
            return vec![json!({
                "unit": "shipping-unit",
                "fresh": false,
                "filenames": [
                    concat!(
                        "/scratch/build-target/",
                        "x86_64-unknown-linux-gnu/release/",
                        "libreovim_foundation.rlib"
                    ),
                    provider_rmeta,
                ]
            })];
        }
        vec![
            json!({
                "unit": "shipping-unit",
                "fresh": true,
                "filenames": [provider_rlib, provider_rmeta]
            }),
            json!({
                "unit": "consumer-unit",
                "fresh": false,
                "filenames": [
                    concat!(
                        "/scratch/build-target/",
                        "x86_64-unknown-linux-gnu/release/",
                        "libreovim_foundation_public_consumer.rlib"
                    ),
                    concat!(
                        "/scratch/build-target/build-dir/",
                        "x86_64-unknown-linux-gnu/release/deps/",
                        "libreovim_foundation_public_consumer-",
                        "2222222222222222.rmeta"
                    )
                ]
            }),
        ]
    }

    fn active_provider_artifact() -> Value {
        json!({
            "unit": "shipping-unit",
            "top_level_rlib": {
                "path": concat!(
                    "/scratch/build-target/",
                    "x86_64-unknown-linux-gnu/release/",
                    "libreovim_foundation.rlib"
                ),
                "content_digest": digest('d'),
                "size_bytes": 1
            },
            "hashed_rlib": {
                "path": concat!(
                    "/scratch/build-target/build-dir/",
                    "x86_64-unknown-linux-gnu/release/deps/",
                    "libreovim_foundation-2222222222222222.rlib"
                ),
                "content_digest": digest('d'),
                "size_bytes": 1
            },
            "rmeta": {
                "path": concat!(
                    "/scratch/build-target/build-dir/",
                    "x86_64-unknown-linux-gnu/release/deps/",
                    "libreovim_foundation-2222222222222222.rmeta"
                ),
                "content_digest": digest('f'),
                "size_bytes": 1
            }
        })
    }

    fn active_probes(closure_kind: &str) -> Vec<Value> {
        if closure_kind != "shipping" {
            return Vec::new();
        }
        ["version_probe", "host_probe", "target_probe", "version_probe"]
            .into_iter()
            .map(active_probe)
            .collect()
    }

    fn active_probe(kind: &str) -> Value {
        let original =
            super::expected_probe_argv(PREPARED_RUSTC, kind).unwrap();
        let mut effective = original.clone();
        effective.extend(["--sysroot".to_owned(), "/sysroot".to_owned()]);
        json!({
            "kind": kind,
            "original_argv": original,
            "effective_argv": effective,
            "trace_digest": digest('1'),
            "trace_bytes": 1,
            "status": 0
        })
    }

    fn product_build_argv(package: &str) -> Vec<String> {
        [
            PREPARED_CARGO,
            "build",
            "--quiet",
            "--locked",
            "--offline",
            "--manifest-path",
            "/repo/product/Cargo.toml",
            "--package",
            package,
            "--lib",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--profile",
            "release",
            "--jobs",
            "1",
            "--message-format=json",
            "--target-dir",
            "/scratch/build-target",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    fn active_compiler_units() -> Vec<Value> {
        vec![
            compiler_unit(
                "shipping-unit",
                "foundation-shipping",
                "foundation",
                "reovim_foundation",
                "/scratch/shipping-dep-info/00000004.d",
            ),
            compiler_unit(
                "consumer-unit",
                "foundation-consumer",
                "foundation-public-consumer",
                "reovim_foundation_public_consumer",
                "/scratch/validation-dep-info/00000000.d",
            ),
        ]
    }

    fn compiler_unit(
        id: &str,
        closure: &str,
        package: &str,
        crate_name: &str,
        dep_info: &str,
    ) -> Value {
        let source = if closure == "foundation-shipping" {
            "/repo/kernel/foundation/src/lib.rs"
        } else {
            "/repo/tests/foundation-public-consumer/src/lib.rs"
        };
        let extern_path =
            (closure == "foundation-consumer").then_some(concat!(
                "/scratch/build-target/build-dir/",
                "x86_64-unknown-linux-gnu/release/deps/",
                "libreovim_foundation-2222222222222222.rmeta"
            ));
        let original = exact_unit_argv(crate_name, source, extern_path);
        let mut effective = original.clone();
        let emit = effective
            .iter_mut()
            .find(|value| value.as_str() == "--emit=dep-info,metadata,link")
            .unwrap();
        *emit = format!("--emit=dep-info={dep_info},metadata,link");
        effective.extend(["--sysroot".to_owned(), "/sysroot".to_owned()]);
        json!({
            "id": id,
            "closure": closure,
            "execution": "foundation-proof",
            "workspace": "product",
            "package": package,
            "target": "library",
            "crate_name": crate_name,
            "crate_types": ["lib"],
            "target_triple": "x86_64-unknown-linux-gnu",
            "profile": "release",
            "features": ["default"],
            "cfgs": ["feature=\"default\""],
            "original_argv": original,
            "effective_argv": effective,
            "dep_info_path": dep_info,
            "dep_info_digest": digest('b'),
            "dep_info_bytes": 1,
            "trace_digest": digest('c'),
            "trace_bytes": 1,
            "status": 0
        })
    }

    fn exact_unit_argv(
        crate_name: &str,
        source: &str,
        extern_path: Option<&str>,
    ) -> Vec<String> {
        let mut argv = [
            PREPARED_RUSTC,
            "--crate-name",
            crate_name,
            "--edition=2024",
            source,
            "--error-format=json",
            "--json=diagnostic-rendered-ansi,artifacts,future-incompat",
            "--crate-type",
            "lib",
            "--emit=dep-info,metadata,link",
            "-C",
            "opt-level=3",
            "-C",
            "embed-bitcode=no",
            "--deny=clippy::pedantic",
            "--deny=clippy::nursery",
            "--deny=clippy::all",
            "--deny=warnings",
            "--forbid=unsafe_code",
            "--forbid=clippy::excessive_nesting",
            "--cfg",
            "feature=\"default\"",
            "--check-cfg",
            "cfg(docsrs,test)",
            "--check-cfg",
            "cfg(feature, values(\"default\"))",
            "-C",
            "metadata=1111111111111111",
            "-C",
            "extra-filename=-2222222222222222",
            "--out-dir",
            concat!(
                "/scratch/build-target/build-dir/",
                "x86_64-unknown-linux-gnu/release/deps"
            ),
            "--target",
            "x86_64-unknown-linux-gnu",
            "-C",
            "strip=debuginfo",
            "-L",
            concat!(
                "dependency=/scratch/build-target/build-dir/",
                "x86_64-unknown-linux-gnu/release/deps"
            ),
            "-L",
            "dependency=/scratch/build-target/build-dir/release/deps",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        if let Some(path) = extern_path {
            argv.extend([
                "--extern".to_owned(),
                format!("reovim_foundation={path}"),
            ]);
        }
        argv
    }

    fn active_compiler_inputs(report: &Value) -> Vec<Value> {
        let mut inputs = Vec::new();
        for (kind, path) in [
            ("crate_root", "kernel/foundation/src/lib.rs"),
            ("module", "kernel/foundation/src/bound.rs"),
            ("module", "kernel/foundation/src/carrier.rs"),
            ("module", "kernel/foundation/src/contract.rs"),
            ("module", "kernel/foundation/src/identity.rs"),
        ] {
            inputs.push(compiler_input(
                report,
                kind,
                "foundation-shipping",
                "shipping-unit",
                path,
            ));
        }
        inputs.push(compiler_input(
            report,
            "crate_root",
            "foundation-consumer",
            "consumer-unit",
            "tests/foundation-public-consumer/src/lib.rs",
        ));
        inputs
    }

    fn compiler_input(
        report: &Value,
        kind: &str,
        closure: &str,
        unit: &str,
        path: &str,
    ) -> Value {
        let source = classification(report, path);
        json!({
            "kind": kind,
            "closure": closure,
            "unit": unit,
            "path": path,
            "content_digest": source["content_digest"],
            "size_bytes": source["size_bytes"]
        })
    }

    fn active_artifacts() -> Vec<Value> {
        vec![
            json!({
                "closure": "foundation-shipping",
                "unit": "shipping-unit",
                "path": concat!(
                    "/scratch/build-target/build-dir/",
                    "x86_64-unknown-linux-gnu/release/deps/",
                    "libreovim_foundation-2222222222222222.rlib"
                ),
                "content_digest": digest('d'),
                "size_bytes": 1,
                "crate_type": "rlib"
            }),
            json!({
                "closure": "foundation-shipping",
                "unit": "shipping-unit",
                "path": concat!(
                    "/scratch/build-target/build-dir/",
                    "x86_64-unknown-linux-gnu/release/deps/",
                    "libreovim_foundation-2222222222222222.rmeta"
                ),
                "content_digest": digest('f'),
                "size_bytes": 1,
                "crate_type": "rmeta"
            }),
            json!({
                "closure": "foundation-consumer",
                "unit": "consumer-unit",
                "path": concat!(
                    "/scratch/build-target/build-dir/",
                    "x86_64-unknown-linux-gnu/release/deps/",
                    "libreovim_foundation_public_consumer-",
                    "2222222222222222.rlib"
                ),
                "content_digest": digest('e'),
                "size_bytes": 1,
                "crate_type": "rlib"
            }),
            json!({
                "closure": "foundation-consumer",
                "unit": "consumer-unit",
                "path": concat!(
                    "/scratch/build-target/build-dir/",
                    "x86_64-unknown-linux-gnu/release/deps/",
                    "libreovim_foundation_public_consumer-",
                    "2222222222222222.rmeta"
                ),
                "content_digest": digest('a'),
                "size_bytes": 1,
                "crate_type": "rmeta"
            }),
        ]
    }

    fn active_package_membership(report: &Value) -> Value {
        let captures = ["before_compiler", "after_compiler"]
            .into_iter()
            .map(package_capture)
            .collect::<Vec<_>>();
        let mut members = Vec::new();
        for name in ["bound.rs", "carrier.rs", "contract.rs", "identity.rs"] {
            members.push(copied_package_member(report, name));
        }
        members.push(copied_package_member(report, "lib.rs"));
        for path in ["Cargo.lock", "Cargo.toml", "Cargo.toml.orig"] {
            members.push(json!({
                "kind": "cargo_generated",
                "package_path": path
            }));
        }
        json!({
            "closure": "foundation-shipping",
            "workspace": "product",
            "package": "foundation",
            "captures": captures,
            "members": members
        })
    }

    fn package_capture(phase: &str) -> Value {
        let members = [
            "Cargo.lock",
            "Cargo.toml",
            "Cargo.toml.orig",
            "src/bound.rs",
            "src/carrier.rs",
            "src/contract.rs",
            "src/identity.rs",
            "src/lib.rs",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
        let semantic_digest =
            package_capture::semantic_digest(&members).unwrap();
        let (raw_output_digest, stdout_bytes) = if phase == "before_compiler" {
            (digest('f'), 1)
        } else {
            (digest('e'), 2)
        };
        json!({
            "phase": phase,
            "command_argv": [
                PREPARED_CARGO,
                "package",
                "--list",
                "--quiet",
                "--locked",
                "--offline",
                "--no-metadata",
                "--manifest-path",
                "/repo/product/Cargo.toml",
                "--package",
                "reovim-foundation"
            ],
            "raw_output_digest": raw_output_digest,
            "semantic_digest": semantic_digest,
            "stdout_bytes": stdout_bytes
        })
    }

    fn copied_package_member(report: &Value, name: &str) -> Value {
        let repository_path = format!("kernel/foundation/src/{name}");
        let source = classification(report, &repository_path);
        json!({
            "kind": "copied",
            "package_path": format!("src/{name}"),
            "repository_path": repository_path,
            "content_digest": source["content_digest"],
            "size_bytes": source["size_bytes"]
        })
    }

    fn active_public_consumer() -> Value {
        let public_type = "bound::Bound";
        let derive_checks = [
            ("Clone", "clone"),
            ("Copy", "copy"),
            ("Debug", "debug"),
            ("Eq", "eq"),
            ("PartialEq", "partial_eq"),
        ]
        .into_iter()
        .map(|(derive, operation)| {
            json!({
                "derive": derive,
                "public_type": public_type,
                "consumer_operation": operation
            })
        })
        .collect::<Vec<_>>();
        json!({
            "relation": "foundation-public-api",
            "shipping_closure": "foundation-shipping",
            "consumer_closure": "foundation-consumer",
            "public_items": [
                {"path": "bound", "kind": "module"},
                {"path": "carrier", "kind": "module"},
                {"path": "contract", "kind": "module"},
                {"path": "identity", "kind": "module"},
                {"path": public_type, "kind": "type"}
            ],
            "derive_checks": derive_checks,
            "status": "pass"
        })
    }

    fn active_execution(report: &Value) -> Value {
        json!({
            "id": "foundation-proof",
            "environment": "linux-product-test",
            "closures": ["foundation-shipping", "foundation-consumer"],
            "exposure": active_exposures(report),
            "namespaces": {
                "user": true,
                "mount": true,
                "network": true,
                "pid": true,
                "uid_map": "0 1000 1",
                "gid_map": "0 1000 1",
                "setgroups_denied": true,
                "private_propagation": true,
                "pid_one_reaper": true,
                "parent_death_signal": true
            },
            "network": {
                "interfaces": ["lo"],
                "routes": [],
                "kernel_route_count": 2,
                "socket_count": 0,
                "policy_observation_digest": crate::digest::bytes(
                    NETWORK_POLICY_DOMAIN,
                ),
                "policy_basis": NETWORK_POLICY_BASIS,
                "external_carrier_absent": true
            },
            "scratch": {
                "mountpoint": "/scratch",
                "filesystem": "tmpfs",
                "size_limit_bytes": 536_870_912,
                "inode_limit": 32_768,
                "allocated_bytes": 2,
                "inode_count": 2,
                "logical_bytes": 2,
                "flags": ["mode=0700", "nodev", "noexec", "nosuid"]
            },
            "process_group": {
                "identity_validated": true,
                "group_absent_after_reap": true
            },
            "cleanup": {
                "processes_absent": true,
                "readers_joined": true,
                "namespace_absent": true,
                "mount_absent": true,
                "mountpoint_identity_preserved": true,
                "underlying_directory_empty": true,
                "source_identity_preserved": true,
                "tool_identity_preserved": true
            }
        })
    }

    fn active_exposures(report: &Value) -> Vec<Value> {
        let mut values = PRODUCT_REPOSITORY_INPUTS
            .into_iter()
            .map(|relative| {
                let path = format!("/repo/{relative}");
                let retained = classification(report, relative);
                exposure_value(
                    &path,
                    &path,
                    "file",
                    ["nodev", "noexec", "nosuid", "ro"],
                    "Product repository input",
                    &file_inventory(
                        retained["content_digest"].as_str().unwrap(),
                        retained["size_bytes"].as_u64().unwrap(),
                        0o100_644,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (source, destination, purpose, content_digest) in [
            (
                "/toolchain/bin/cargo",
                PREPARED_CARGO,
                "Cargo executable",
                digest('a'),
            ),
            (
                "/toolchain/bin/rustc",
                PREPARED_RUSTC,
                "rustc executable",
                digest('a'),
            ),
            ("/checker", PREPARED_CHECKER, "checker", digest('a')),
            (
                "/toolchain/lib/librustc_driver.so",
                "/compiler/lib/librustc_driver.so",
                "compiler support",
                digest('8'),
            ),
        ] {
            values.push(exposure_value(
                source,
                destination,
                "file",
                ["nodev", "nosuid", "ro", ""],
                purpose,
                &file_inventory(&content_digest, 1, 0o100_755),
            ));
        }
        values.push(exposure_value(
            concat!("/toolchain/lib/rustlib/", "x86_64-unknown-linux-gnu/lib"),
            super::PREPARED_TARGET_LIBDIR,
            "directory",
            ["nodev", "noexec", "nosuid", "ro"],
            "selected Rust target libdir",
            &target_inventory(),
        ));
        values.push(exposure_value(
            "/runtime/libc.so",
            "/lib/libc.so",
            "file",
            ["nodev", "nosuid", "ro", ""],
            "system runtime",
            &file_inventory(&digest('7'), 1, 0o100_755),
        ));
        values.push(exposure_value(
            "/dev/null",
            "/dev/null",
            "character_device",
            ["noexec", "nosuid", "ro", ""],
            "null standard-input device",
            &null_inventory(),
        ));
        values.sort_by(|left, right| {
            left["destination"].as_str().cmp(&right["destination"].as_str())
        });
        values
    }

    fn exposure_value<const N: usize>(
        source: &str,
        destination: &str,
        kind: &str,
        flags: [&str; N],
        purpose: &str,
        inventory: &prepared_root::InventoryIdentity,
    ) -> Value {
        let flags = flags
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        let entries = inventory
            .entries
            .iter()
            .map(|entry| {
                json!({
                    "relative_path": entry.relative_path,
                    "kind": entry.kind.as_str(),
                    "mode": entry.mode,
                    "size_bytes": entry.size_bytes,
                    "content_digest": entry.content_digest,
                    "device_identity": entry.device_identity,
                })
            })
            .collect::<Vec<_>>();
        json!({
            "source": source,
            "destination": destination,
            "kind": kind,
            "filesystem": "bind",
            "flags": flags,
            "purpose": purpose,
            "inventory_digest": inventory.aggregate_digest,
            "inventory_entry_count": inventory.entry_count,
            "inventory_total_bytes": inventory.total_bytes,
            "inventory_entries": entries,
        })
    }

    fn file_inventory(
        content_digest: &str,
        size_bytes: u64,
        mode: u32,
    ) -> prepared_root::InventoryIdentity {
        prepared_root::inventory_identity(vec![prepared_root::InventoryEntry {
            relative_path: PathBuf::from("."),
            kind: prepared_root::InventoryEntryKind::File,
            mode,
            size_bytes,
            content_digest: Some(content_digest.to_owned()),
            device_identity: None,
        }])
        .unwrap()
    }

    fn target_inventory() -> prepared_root::InventoryIdentity {
        prepared_root::inventory_identity(vec![
            prepared_root::InventoryEntry {
                relative_path: PathBuf::from("."),
                kind: prepared_root::InventoryEntryKind::Directory,
                mode: 0o40755,
                size_bytes: 0,
                content_digest: None,
                device_identity: None,
            },
            prepared_root::InventoryEntry {
                relative_path: PathBuf::from("libstd.rlib"),
                kind: prepared_root::InventoryEntryKind::File,
                mode: 0o100_644,
                size_bytes: 1,
                content_digest: Some(digest('9')),
                device_identity: None,
            },
        ])
        .unwrap()
    }

    fn null_inventory() -> prepared_root::InventoryIdentity {
        prepared_root::inventory_identity(vec![prepared_root::InventoryEntry {
            relative_path: PathBuf::from("."),
            kind: prepared_root::InventoryEntryKind::CharacterDevice,
            mode: 0o20666,
            size_bytes: 0,
            content_digest: None,
            device_identity: Some(259),
        }])
        .unwrap()
    }

    fn product_process() -> Value {
        let mut process = passing_process();
        process["purpose"] = json!("product_proof_execution");
        process["workspace"] = json!("product");
        process["profile"] = json!("release");
        process["execution"] = json!("foundation-proof");
        process["executable_digest"] = json!(digest('a'));
        process["argv"] = json!(["/checker", "product-proof-child"]);
        process
    }

    fn candidate_identity_inputs(report_schema_digest: &str) -> Value {
        json!({
            "source_classification": artifact(
                "conformance/source-classification.json",
                "reovim-source-classification/v2"
            ),
            "workspace_topology": artifact(
                "conformance/workspace-topology.json",
                "reovim-workspace-topology/v2"
            ),
            "entrypoint_reviews": artifact(
                "conformance/entrypoint-reviews.json",
                "reovim-entrypoint-reviews/v1"
            ),
            "dependency_reviews": artifact(
                "conformance/dependency-reviews.json",
                "reovim-dependency-reviews/v2"
            ),
            "product_closures": artifact(
                "conformance/product-closures.json",
                "reovim-product-closures/v1"
            ),
            "execution_tool_reviews": artifact(
                "conformance/execution-tool-reviews.json",
                "reovim-execution-tool-reviews/v1"
            ),
            "report_schema": {
                "path": "conformance/source-role-map.schema.json",
                "schema": SCHEMA,
                "content_digest": report_schema_digest
            },
            "lockfiles": [{
                "workspace": "root",
                "path": "Cargo.lock",
                "digest": digest('1')
            }],
            "source_tree_digest": digest('2'),
            "checker": tool("/checker"),
            "cargo": tool("/cargo"),
            "rustc": tool("/rustc"),
            "git": null,
            "git_tree_digest": null,
            "cargo_home_config": "absent",
            "cargo_working_directory_config": "absent",
            "compiler_substrates": [],
            "metadata_profiles": [{
                "workspace": "root",
                "profile": "stable",
                "target_triple": "x86_64-unknown-linux-gnu",
                "all_features": false,
                "no_default_features": false,
                "features": [],
                "metadata_digest": digest('3')
            }],
            "product_proof_gate": null
        })
    }

    fn candidate_source_inventory(identity_inputs: &Value) -> Value {
        json!({
            "classifications": base_authority_classifications(identity_inputs),
            "exclusions": [
                {"prefix": ".git", "kind": "vcs_metadata"},
                {"prefix": "tmp", "kind": "working_notes"},
                {"prefix": "target", "kind": "transient_build_output"}
            ]
        })
    }

    fn base_authority_classifications(identity_inputs: &Value) -> Vec<Value> {
        let mut paths = BTreeMap::from([
            ("Cargo.toml".to_owned(), digest('4')),
            ("Cargo.lock".to_owned(), digest('1')),
            ("lib/fixture/Cargo.toml".to_owned(), digest('c')),
            ("lib/fixture/src/lib.rs".to_owned(), digest('d')),
        ]);
        for field in [
            "source_classification",
            "workspace_topology",
            "entrypoint_reviews",
            "dependency_reviews",
            "product_closures",
            "execution_tool_reviews",
        ] {
            let artifact = &identity_inputs[field];
            insert_fixture_source(
                &mut paths,
                artifact["path"].as_str().unwrap(),
                artifact["content_digest"].as_str().unwrap(),
            );
            insert_fixture_source(
                &mut paths,
                artifact["schema_path"].as_str().unwrap(),
                artifact["schema_digest"].as_str().unwrap(),
            );
        }
        let report = &identity_inputs["report_schema"];
        insert_fixture_source(
            &mut paths,
            report["path"].as_str().unwrap(),
            report["content_digest"].as_str().unwrap(),
        );
        paths
            .into_iter()
            .enumerate()
            .map(|(index, (path, digest))| {
                authority_classification(&path, &digest, index)
            })
            .collect()
    }

    fn insert_fixture_source(
        paths: &mut BTreeMap<String, String>,
        path: &str,
        digest: &str,
    ) {
        let previous = paths.insert(path.to_owned(), digest.to_owned());
        assert!(previous.is_none_or(|value| value == digest));
    }

    fn authority_classification(
        path: &str,
        content_digest: &str,
        index: usize,
    ) -> Value {
        let mut classification = json!({
            "path": path,
            "content_digest": content_digest,
            "size_bytes": 1,
            "mode": "100644",
            "source_owner": {"kind": "repository_tooling"},
            "source_provenance": "authored",
            "source_use": "tool",
            "upstream": null,
            "winning_rule": {
                "index": index,
                "prefix": path,
                "precedence": 1
            }
        });
        if path == "Cargo.lock" {
            classification["source_provenance"] = json!("generated");
            classification["upstream"] = generated_upstream(["Cargo.toml"]);
        }
        classification
    }

    fn generated_upstream<const N: usize>(paths: [&str; N]) -> Value {
        let paths = paths.into_iter().collect::<Vec<_>>();
        json!({
            "identity": "fixture-generator/v1",
            "digest": digest('7'),
            "digest_subject": {"kind": "paths", "paths": paths}
        })
    }

    fn candidate_cargo_graph() -> Value {
        json!({
            "metadata_runs": [metadata_run("stable", "initial")],
            "workspaces": [{
                "id": "root",
                "partition": "repository_tooling",
                "root": ".",
                "manifest": "Cargo.toml",
                "lockfile": "Cargo.lock",
                "members": ["lib/fixture"]
            }],
            "workspace_packages": [workspace_package("stable")],
            "external_packages": [external_package("stable")],
            "edges": [cargo_edge("stable")],
            "reachability": [reachability("stable")]
        })
    }

    fn external_package(profile: &str) -> Value {
        json!({
            "workspace": "root",
            "profile": profile,
            "name": "dependency",
            "version": "1.0.0",
            "source": "registry+https://example.invalid/index",
            "checksum": null
        })
    }

    fn cargo_edge(profile: &str) -> Value {
        json!({
            "workspace": "root",
            "profile": profile,
            "consumer": {
                "kind": "workspace_package",
                "workspace": "root",
                "package": "fixture"
            },
            "provider": {
                "kind": "external_package",
                "name": "dependency",
                "version": "1.0.0",
                "source": "registry+https://example.invalid/index"
            },
            "dependency_name": "dependency",
            "kind": "normal",
            "target": null,
            "consumer_owners": [{"kind": "repository_tooling"}],
            "provider_owners": [{"kind": "repository_tooling"}],
            "decision": "admitted_same_owner"
        })
    }

    fn reachability(profile: &str) -> Value {
        json!({
            "workspace": "root",
            "profile": profile,
            "package": "fixture",
            "members": [
                "dependency 1.0.0 (registry+https://example.invalid/index)",
                "fixture 0.1.0 (workspace)"
            ]
        })
    }

    fn candidate_product_proof() -> Value {
        json!({
            "selected_execution_environment": null,
            "closures": [],
            "compiler_units": [],
            "compiler_inputs": [],
            "extern_edges": [],
            "artifacts": [],
            "provider_artifact": null,
            "package_memberships": [],
            "public_consumers": [],
            "executions": []
        })
    }

    fn metadata_run(profile: &str, phase: &str) -> Value {
        json!({
            "workspace": "root",
            "profile": profile,
            "phase": phase,
            "command_argv": metadata_argv(),
            "stdout_digest": digest('3'),
            "metadata_digest": digest('3')
        })
    }

    fn metadata_argv() -> Vec<&'static str> {
        vec![
            "/cargo",
            "metadata",
            "--quiet",
            "--format-version",
            "1",
            "--manifest-path",
            "/repo/Cargo.toml",
            "--locked",
            "--offline",
            "--filter-platform",
            "x86_64-unknown-linux-gnu",
        ]
    }

    fn metadata_process(profile: &str, phase: &str) -> Value {
        let mut process = passing_process();
        process["purpose"] = json!(if phase == "initial" {
            "cargo_metadata"
        } else {
            "cargo_metadata_final"
        });
        process["workspace"] = json!("root");
        process["profile"] = json!(profile);
        process["executable_digest"] = json!(digest('a'));
        process["argv"] = json!(metadata_argv());
        process["stdout"]["digest"] = json!(digest('3'));
        process
    }

    fn candidate_processes() -> Vec<Value> {
        vec![
            metadata_process("stable", "initial"),
            version_process("checker_version", "/checker"),
            version_process("cargo_version", "/cargo"),
            version_process("rustc_version", "/rustc"),
        ]
    }

    fn version_process(purpose: &str, path: &str) -> Value {
        let identity = tool(path);
        let version = identity["version"].as_str().unwrap();
        let mut argv = vec![path.to_owned()];
        argv.extend(
            identity["version_argv"]
                .as_array()
                .unwrap()
                .iter()
                .map(|argument| argument.as_str().unwrap().to_owned()),
        );
        let mut process = passing_process();
        process["purpose"] = json!(purpose);
        process["executable_digest"] = identity["executable_digest"].clone();
        process["argv"] = json!(argv);
        process["stdout"]["bytes"] = json!(version.len() + 1);
        process["stdout"]["digest"] = identity["version_digest"].clone();
        process
    }

    fn git_processes() -> Vec<Value> {
        super::GIT_PROCESS_PURPOSES.into_iter().map(git_process).collect()
    }

    fn git_process(purpose: &str) -> Value {
        let identity = tool("/git");
        let mut process = passing_process();
        let mut argv = vec!["/git".to_owned()];
        argv.extend(super::git_process_arguments(purpose).unwrap());
        process["purpose"] = json!(purpose);
        process["workspace"] = json!("root");
        process["tool_review"] = json!("git-repository-proof");
        process["tool_environment"] = json!("linux-test");
        process["executable_digest"] = identity["executable_digest"].clone();
        process["argv"] = json!(argv);
        match super::git_process_operation(purpose) {
            Some("version") => {
                let version = identity["version"].as_str().unwrap();
                process["stdout"]["bytes"] = json!(version.len() + 1);
                process["stdout"]["digest"] =
                    identity["version_digest"].clone();
            }
            Some("head") => {
                process["stdout"]["bytes"] = json!(41);
                process["stdout"]["digest"] = json!(crate::digest::bytes(
                    format!("{}\n", "a".repeat(40)).as_bytes()
                ));
            }
            Some("ls_tree") => {
                process["stdout"]["digest"] = json!(digest('9'));
            }
            Some("root") => {
                let output = "/repo\n";
                process["stdout"]["bytes"] = json!(output.len());
                process["stdout"]["digest"] =
                    json!(crate::digest::bytes(output.as_bytes()));
            }
            Some("object_format") => {
                let output = "sha1\n";
                process["stdout"]["bytes"] = json!(output.len());
                process["stdout"]["digest"] =
                    json!(crate::digest::bytes(output.as_bytes()));
            }
            _ => {}
        }
        process
    }

    fn workspace_package(profile: &str) -> Value {
        json!({
            "workspace": "root",
            "profile": profile,
            "id": "fixture",
            "name": "fixture",
            "version": "0.1.0",
            "manifest": "lib/fixture/Cargo.toml",
            "owners": [{"kind": "repository_tooling"}],
            "targets": [{
                "id": "library",
                "name": "fixture",
                "kind": ["lib"],
                "crate_types": ["lib"],
                "src_path": "lib/fixture/src/lib.rs"
            }]
        })
    }

    fn passing_process() -> Value {
        json!({
            "purpose": "fixture",
            "workspace": null,
            "profile": null,
            "closure": null,
            "execution": null,
            "tool_review": null,
            "tool_environment": null,
            "executable_digest": digest('5'),
            "argv": ["/fixture"],
            "termination_reason": "exited",
            "status": 0,
            "stdout": {
                "bytes": 0,
                "limit": 16_777_216,
                "overflow": false,
                "digest": digest('6')
            },
            "stderr": {
                "bytes": 0,
                "limit": 1_048_576,
                "overflow": false,
                "digest": digest('7')
            },
            "kill": {
                "attempted": false,
                "signal": null,
                "raced_with_exit": false,
                "group_absent": true
            },
            "reap": {
                "leader_reaped": true,
                "descendants_reaped": true
            },
            "group": {
                "identity_validated": true,
                "absent_after_reap": true
            },
            "readers": {
                "stdout_joined": true,
                "stderr_joined": true
            },
            "deadlines": {
                "execution_ms": 120_000,
                "supervision_ms": 130_000,
                "observed_after_exit": false
            }
        })
    }

    fn add_alternate_profile(report: &mut Value) {
        let mut profile =
            report["identity_inputs"]["metadata_profiles"][0].clone();
        profile["profile"] = json!("alternate");
        report["identity_inputs"]["metadata_profiles"]
            .as_array_mut()
            .unwrap()
            .push(profile);
        report["cargo_graph"]["metadata_runs"]
            .as_array_mut()
            .unwrap()
            .push(metadata_run("alternate", "initial"));
        report["cargo_graph"]["workspace_packages"]
            .as_array_mut()
            .unwrap()
            .push(workspace_package("alternate"));
        report["cargo_graph"]["external_packages"]
            .as_array_mut()
            .unwrap()
            .push(external_package("alternate"));
        report["cargo_graph"]["edges"]
            .as_array_mut()
            .unwrap()
            .push(cargo_edge("alternate"));
        report["cargo_graph"]["reachability"]
            .as_array_mut()
            .unwrap()
            .push(reachability("alternate"));
        report["processes"]
            .as_array_mut()
            .unwrap()
            .push(metadata_process("alternate", "initial"));
    }

    fn process_by_purpose<'a>(
        report: &'a mut Value,
        purpose: &str,
    ) -> &'a mut Value {
        report["processes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|process| process["purpose"] == purpose)
            .unwrap()
    }

    fn remove_process(report: &mut Value, purpose: &str) {
        report["processes"]
            .as_array_mut()
            .unwrap()
            .retain(|process| process["purpose"] != purpose);
    }

    fn set_process_digests(
        report: &mut Value,
        purposes: &[&str],
        digest: &str,
    ) {
        for purpose in purposes {
            process_by_purpose(report, purpose)["stdout"]["digest"] =
                json!(digest);
        }
    }

    fn set_cargo_executable(report: &mut Value, path: &str) {
        report["identity_inputs"]["cargo"]["canonical_path"] = json!(path);
        for run in
            report["cargo_graph"]["metadata_runs"].as_array_mut().unwrap()
        {
            run["command_argv"][0] = json!(path);
        }
        for process in report["processes"].as_array_mut().unwrap() {
            set_cargo_process_path(process, path);
        }
    }

    fn set_cargo_process_path(process: &mut Value, path: &str) {
        if current_cargo_process(process) {
            process["argv"][0] = json!(path);
        }
    }

    fn set_cargo_digest(report: &mut Value, digest: &str) {
        report["identity_inputs"]["cargo"]["executable_digest"] = json!(digest);
        for process in report["processes"].as_array_mut().unwrap() {
            set_cargo_process_digest(process, digest);
        }
    }

    fn set_cargo_process_digest(process: &mut Value, digest: &str) {
        if current_cargo_process(process) {
            process["executable_digest"] = json!(digest);
        }
    }

    fn current_cargo_process(process: &Value) -> bool {
        matches!(
            process["purpose"].as_str(),
            Some("cargo_version" | "cargo_metadata" | "cargo_metadata_final")
        )
    }

    fn artifact(path: &str, schema: &str) -> Value {
        let schema_path = match path {
            crate::POLICY_KEY => crate::POLICY_SCHEMA_KEY,
            crate::TOPOLOGY_KEY => crate::TOPOLOGY_SCHEMA_KEY,
            crate::ENTRYPOINT_REVIEWS_KEY => {
                crate::ENTRYPOINT_REVIEWS_SCHEMA_KEY
            }
            crate::DEPENDENCY_REVIEWS_KEY => {
                crate::DEPENDENCY_REVIEWS_SCHEMA_KEY
            }
            crate::PRODUCT_CLOSURES_KEY => crate::PRODUCT_CLOSURES_SCHEMA_KEY,
            crate::EXECUTION_TOOL_REVIEWS_KEY => {
                crate::EXECUTION_TOOL_REVIEWS_SCHEMA_KEY
            }
            _ => panic!("unknown fixture artifact path {path}"),
        };
        json!({
            "path": path,
            "schema": schema,
            "content_digest": digest('5'),
            "schema_path": schema_path,
            "schema_digest": digest('6')
        })
    }

    fn tool(path: &str) -> Value {
        let (version_argv, version) = match path {
            "/checker" => (json!(["--version"]), "reovim 0.16.0"),
            "/cargo" => (
                json!(["--version", "--verbose"]),
                "cargo 1.92.0\nrelease: 1.92.0\nhost: x86_64-unknown-linux-gnu",
            ),
            "/rustc" => (
                json!(["--version", "--verbose"]),
                "rustc 1.92.0\nrelease: 1.92.0\nhost: x86_64-unknown-linux-gnu",
            ),
            "/git" => (json!(["--version"]), "git version 2.55.0"),
            _ => panic!("unknown fixture tool path {path}"),
        };
        let exact_output = format!("{version}\n");
        json!({
            "canonical_path": path,
            "executable_digest": digest('a'),
            "executable_size_bytes": 1,
            "version_argv": version_argv,
            "version": version,
            "version_digest": crate::digest::bytes(exact_output.as_bytes()),
            "runtime_inputs": []
        })
    }

    fn runtime_input(
        path: &str,
        prepared_path: &str,
        digest_byte: char,
        size_bytes: u64,
    ) -> Value {
        json!({
            "canonical_path": path,
            "prepared_path": prepared_path,
            "content_digest": digest(digest_byte),
            "size_bytes": size_bytes
        })
    }

    fn limits() -> Value {
        json!({
            "configuration": {
                "config_bytes": 1_048_576,
                "json_depth": JSON_DEPTH,
                "json_container_items": JSON_ITEMS,
                "report_bytes": REPORT_LIMIT,
                "findings": 8_192
            },
            "inventory": {
                "source_entry_count": 32_768,
                "directory_entry_count": 16_384,
                "source_file_count": 16_384,
                "source_total_bytes": 134_217_728,
                "source_file_bytes": 8_388_608,
                "path_bytes": 4_096,
                "syntax_item_expr_type_visits": 2_000_000
            },
            "process": {
                "stdout_bytes": 16_777_216,
                "stderr_bytes": 1_048_576,
                "execution_deadline_ms": 120_000,
                "supervision_deadline_ms": 130_000,
                "tool_executable_bytes": 100_663_296
            },
            "product_execution": {
                "scratch_bytes": 536_870_912,
                "scratch_inodes": 32_768,
                "process_address_space_bytes": 4_294_967_296_u64,
                "process_cpu_seconds": 120,
                "process_open_files": 256,
                "process_output_file_bytes": 268_435_456,
                "core_dump_bytes": 0,
                "exposure_entry_count": 8_192,
                "compiler_substrate_entry_count": 8_192,
                "compiler_substrate_total_bytes": 1_073_741_824,
                "cargo_json_bytes": 16_777_216,
                "cargo_message_count": 4_096,
                "wrapper_trace_bytes": 16_777_216,
                "wrapper_argv_items": 4_096,
                "wrapper_argv_bytes": 1_048_576,
                "compiler_unit_count": 256,
                "compiler_input_count": 8_192,
                "dep_info_file_bytes": 4_194_304,
                "dep_info_total_bytes": 67_108_864,
                "artifact_count": 4_096,
                "artifact_total_bytes": 536_870_912,
                "package_list_bytes": 4_194_304,
                "package_member_count": 8_192,
                "public_item_count": 4_096,
                "environment_dependency_count": 1_024
            }
        })
    }

    fn digest(hex: char) -> String {
        format!("sha256:{}", hex.to_string().repeat(64))
    }

    fn _assert_built_type(_: BuiltReport) {}
}
