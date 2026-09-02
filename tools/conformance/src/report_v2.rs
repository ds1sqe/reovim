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

use crate::{digest, json, schema, source};

/// Exact schema identity accepted by this module.
pub const SCHEMA: &str = "reovim-source-role-map/v2";

const IDENTITY_PLACEHOLDER: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

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
    require_identity_placeholder(&value)?;
    value
        .as_object_mut()
        .ok_or_else(|| envelope_error("report root is not an object"))?
        .insert(
            "report_identity".to_owned(),
            Value::String(IDENTITY_PLACEHOLDER.to_owned()),
        );

    schema::validate_instance(&value, &schema_value)
        .map_err(|failure| schema_error(&failure))?;
    schema::canonicalize_unique_arrays(&mut value, &schema_value)
        .map_err(|failure| canonical_error(failure.to_string()))?;
    schema::validate_instance(&value, &schema_value)
        .map_err(|failure| schema_error(&failure))?;
    validate_schema_preimage(&value, schema_bytes)?;
    validate_semantics(&value)?;

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
    let identity = digest::bytes(&canonical);
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

fn require_identity_placeholder(value: &Value) -> Result<(), ReportError> {
    match value.get("report_identity") {
        Some(Value::Null) => Ok(()),
        Some(Value::String(identity)) if valid_digest(identity) => Ok(()),
        Some(_) => Err(identity_error(concat!(
            "report_identity input must be null or a sha256 placeholder ",
            "that will be replaced"
        ))),
        None => {
            Err(identity_error("fully populated report omits report_identity"))
        }
    }
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
    let graph = graph_references(value, &source)?;
    validate_identity_input_authority(value, &graph, &source)?;
    validate_product_references(value, &graph, &source)
}

struct GraphReferences {
    workspaces: BTreeSet<String>,
    product_workspaces: BTreeSet<String>,
    workspace_lockfiles: BTreeMap<String, String>,
    profiles: BTreeSet<(String, String)>,
    repository_root: PathBuf,
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
    lockfiles: BTreeMap<String, String>,
    manifests: BTreeMap<String, PathBuf>,
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
) -> Result<GraphReferences, ReportError> {
    let workspaces = collect_workspaces(value)?;
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
        workspace_lockfiles: workspaces.lockfiles,
        profiles: profile_ids,
        repository_root,
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
    file_limit: u64,
    path_limit: u64,
    context: &str,
    file_paths: &mut BTreeMap<String, FileIdentity>,
) -> Result<(), ReportError> {
    let mut paths = BTreeSet::new();
    for input in required_field_array(tool, "runtime_inputs", context)? {
        let path = field_string(input, "canonical_path", "runtime input")?;
        if !normalized_file_path(path) || !paths.insert(path) {
            return Err(identity_error(format!(
                "{context} runtime-input paths are invalid or duplicated"
            )));
        }
        enforce_path_byte_limit(path, path_limit, "runtime input")?;
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
) -> Result<CollectedWorkspaces, ReportError> {
    let mut workspaces = BTreeSet::new();
    let mut roots = BTreeSet::new();
    let mut product_workspaces = BTreeSet::new();
    let mut lockfiles = BTreeMap::new();
    let mut manifests = BTreeMap::new();
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
        validate_workspace_members(workspace, &root)?;
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
    if roots != BTreeSet::from([PathBuf::new()]) {
        return Err(reference_error(
            "current Product-empty profile requires exactly one '.' root",
        ));
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
        lockfiles,
        manifests,
        partitions,
    })
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
) -> Result<(), ReportError> {
    let context = "cargo_graph.workspaces[]";
    for member in required_field_array(workspace, "members", context)? {
        let member = member
            .as_str()
            .ok_or_else(|| reference_error("workspace member is not text"))?;
        let member = parse_relative_path(member, "workspace member")?;
        validate_workspace_member(root, &member)?;
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
    if workspaces.partitions.get(workspace).map(String::as_str)
        != Some("repository_tooling")
        || owners != &BTreeSet::from([SourceOwner::RepositoryTooling])
    {
        return Err(reference_error(
            "Product-empty workspace package is not RepositoryTooling-only",
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
    let expected = BTreeSet::from([SourceOwner::RepositoryTooling]);
    let consumers = edge_owner_set(edge, "consumer_owners")?;
    let providers = edge_owner_set(edge, "provider_owners")?;
    let decision = field_string(edge, "decision", "Cargo edge")?;
    if consumers != expected
        || providers != expected
        || decision != "admitted_same_owner"
    {
        return Err(reference_error(
            "Product-empty Cargo edge is not admitted RepositoryTooling-only",
        ));
    }
    Ok(())
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
    validate_product_empty_state(value, product, graph, source)?;
    let passing = required_string(value, "/result/status")? == "pass";
    validate_process_references(
        value,
        &graph.workspaces,
        &graph.profiles,
        passing,
    )?;
    validate_process_purposes(value, graph)
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
) -> Result<(), ReportError> {
    let limits = RetainedProcessLimits::from_report(value)?;
    for process in required_array(value, "/processes")? {
        validate_process_scope(process, workspaces, profiles)?;
        validate_process_ownership(process, limits, passing)?;
    }
    Ok(())
}

fn validate_process_purposes(
    value: &Value,
    graph: &GraphReferences,
) -> Result<(), ReportError> {
    let mut versions = BTreeMap::new();
    let mut git = BTreeMap::new();
    for process in required_array(value, "/processes")? {
        let purpose = field_string(process, "purpose", "process")?;
        match purpose {
            "checker_version" | "cargo_version" | "rustc_version" => {
                insert_process_purpose(&mut versions, purpose, process)?;
            }
            "cargo_metadata" | "cargo_metadata_final" => {}
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
    match required_string(value, "/mode")? {
        "candidate" if git.is_empty() => Ok(()),
        "exact" => validate_git_processes(value, graph, &git),
        "candidate" => Err(reference_error(
            "candidate report contains exact Git processes",
        )),
        _ => Err(identity_error("process purpose mode is invalid")),
    }
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
    if !process.get("closure").is_some_and(Value::is_null)
        || !process.get("execution").is_some_and(Value::is_null)
    {
        return Err(error("Product-empty process contains Product references"));
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
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use super::{BuiltReport, ReportError, ReportErrorCode, SCHEMA};

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
            let first = runtime_input("/runtime/lib.so", '1', 1);
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
                json!([runtime_input("/cargo", digest_byte, size_bytes)]);
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
                runtime_input("/runtime/lib.so", '1', 1),
                runtime_input("/runtime/lib.so", '2', 1)
            ]);
        });
        assert_eq!(runtime.code(), ReportErrorCode::Identity);

        let runtime_limit = verify_reidentified_mutant(|report| {
            report["identity_inputs"]["cargo"]["runtime_inputs"] =
                json!([runtime_input("/runtime/lib.so", '1', 100_663_297)]);
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
        let input = runtime_input("/runtime/lib.so", '1', 1);
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
            json!([runtime_input("/cargo", 'a', 1)]);
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
                json!([runtime_input(&long_path, '1', 1)]);
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

    fn runtime_input(path: &str, digest_byte: char, size_bytes: u64) -> Value {
        json!({
            "canonical_path": path,
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
