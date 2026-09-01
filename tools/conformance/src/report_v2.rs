//! Deterministic `reovim-source-role-map/v2` envelope and standalone verifier.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use serde_json::Value;

use crate::{digest, json, schema};

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
pub struct ReportError(pub String);

impl fmt::Display for ReportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ReportError {}

/// Canonicalizes, identifies, validates, and serializes one fully populated v2
/// report using the committed embedded v2 schema.
pub fn build(value: Value, report_byte_limit: usize) -> Result<BuiltReport, ReportError> {
    build_with_schema(value, schema::embedded_report_schema_bytes(), report_byte_limit)
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
        .ok_or_else(|| error("report root is not an object"))?
        .insert("report_identity".to_owned(), Value::String(IDENTITY_PLACEHOLDER.to_owned()));

    schema::validate_instance(&value, &schema_value).map_err(|failure| schema_error(&failure))?;
    schema::canonicalize_unique_arrays(&mut value, &schema_value)
        .map_err(|failure| schema_error(&failure))?;
    schema::validate_instance(&value, &schema_value).map_err(|failure| schema_error(&failure))?;
    validate_schema_preimage(&value, schema_bytes)?;
    validate_semantics(&value)?;

    let mut identity_preimage = value.clone();
    identity_preimage
        .as_object_mut()
        .expect("schema-validated report is an object")
        .remove("report_identity");
    let canonical = serde_json::to_vec(&identity_preimage).map_err(|failure| {
        error(format!("cannot serialize report identity preimage: {failure}"))
    })?;
    let identity = digest::bytes(&canonical);
    value
        .as_object_mut()
        .expect("schema-validated report is an object")
        .insert("report_identity".to_owned(), Value::String(identity.clone()));
    schema::validate_instance(&value, &schema_value).map_err(|failure| schema_error(&failure))?;

    let mut bytes = serde_json::to_vec(&value)
        .map_err(|failure| error(format!("cannot serialize final report: {failure}")))?;
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
        return Err(error("report exceeds the caller-owned report byte limit"));
    }
    if bytes.last() != Some(&b'\n') {
        return Err(error("report must end with exactly one canonical LF"));
    }
    let schema_value = parse_schema(schema_bytes)?;
    require_v2_schema(&schema_value)?;
    let value = json::parse(bytes, json_depth_limit, json_container_item_limit)
        .map_err(|failure| error(failure.to_string()))?;
    schema::validate_instance(&value, &schema_value).map_err(|failure| schema_error(&failure))?;

    let mut canonical_value = value.clone();
    schema::canonicalize_unique_arrays(&mut canonical_value, &schema_value)
        .map_err(|failure| schema_error(&failure))?;
    let mut canonical_bytes = serde_json::to_vec(&canonical_value)
        .map_err(|failure| error(format!("cannot serialize canonical report: {failure}")))?;
    canonical_bytes.push(b'\n');
    if canonical_bytes != bytes {
        return Err(error(
            "report is not canonical JSON or a schema-set array is not in canonical order",
        ));
    }

    enforce_report_limits(&value, bytes.len(), report_byte_limit)?;
    validate_schema_preimage(&value, schema_bytes)?;
    validate_semantics(&value)?;
    let identity = value
        .get("report_identity")
        .and_then(Value::as_str)
        .ok_or_else(|| error("report_identity is absent or invalid"))?
        .to_owned();
    let mut identity_preimage = value.clone();
    identity_preimage
        .as_object_mut()
        .expect("schema-validated report is an object")
        .remove("report_identity");
    let canonical = serde_json::to_vec(&identity_preimage).map_err(|failure| {
        error(format!("cannot serialize report identity preimage: {failure}"))
    })?;
    let computed = digest::bytes(&canonical);
    if computed != identity {
        return Err(error(format!(
            "report identity mismatch: expected {identity}, computed {computed}"
        )));
    }

    let mode = required_string(&value, "/mode")?.to_owned();
    let status = required_string(&value, "/result/status")?.to_owned();
    Ok(VerifiedReport { identity, mode, status })
}

fn parse_schema(bytes: &[u8]) -> Result<Value, ReportError> {
    serde_json::from_slice(bytes)
        .map_err(|failure| error(format!("v2 report schema is invalid JSON: {failure}")))
}

fn require_v2_schema(schema_value: &Value) -> Result<(), ReportError> {
    if schema_value.pointer("/properties/schema/const").and_then(Value::as_str) != Some(SCHEMA) {
        return Err(error("supplied report schema is not reovim-source-role-map/v2"));
    }
    schema::validate_schema(schema_value).map_err(|failure| schema_error(&failure))
}

fn require_identity_placeholder(value: &Value) -> Result<(), ReportError> {
    match value.get("report_identity") {
        Some(Value::Null) => Ok(()),
        Some(Value::String(identity)) if valid_digest(identity) => Ok(()),
        Some(_) => Err(error(
            "report_identity input must be null or a sha256 placeholder that will be replaced",
        )),
        None => Err(error("fully populated report omits report_identity")),
    }
}

fn validate_schema_preimage(value: &Value, schema_bytes: &[u8]) -> Result<(), ReportError> {
    let expected = digest::bytes(schema_bytes);
    let actual = required_string(value, "/identity_inputs/report_schema/content_digest")?;
    if actual != expected {
        return Err(error(format!(
            "identity_inputs.report_schema.content_digest differs from supplied schema: expected {expected}"
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
        .ok_or_else(|| error("limits.configuration.report_bytes is absent or invalid"))?;
    let byte_count = u64::try_from(byte_count)
        .map_err(|failure| error(format!("report byte count conversion failed: {failure}")))?;
    let caller_limit = u64::try_from(caller_limit)
        .map_err(|failure| error(format!("report limit conversion failed: {failure}")))?;
    if byte_count > caller_limit || byte_count > declared {
        return Err(error(format!(
            "report is {byte_count} bytes; caller limit is {caller_limit} and declared limit is {declared}"
        )));
    }
    Ok(())
}

fn validate_semantics(value: &Value) -> Result<(), ReportError> {
    if required_string(value, "/schema")? != SCHEMA {
        return Err(error("report schema identity is not reovim-source-role-map/v2"));
    }
    if required_string(value, "/subject/kind")? != "active_repository" {
        return Err(error("v2 Source Role Map subject is not active_repository"));
    }
    validate_mode(value)?;
    validate_result(value)?;
    validate_graph_and_product_references(value)
}

fn validate_mode(value: &Value) -> Result<(), ReportError> {
    let mode = required_string(value, "/mode")?;
    let revision = value.pointer("/revision").unwrap_or(&Value::Null);
    let git = value.pointer("/identity_inputs/git").unwrap_or(&Value::Null);
    let tree = value.pointer("/identity_inputs/git_tree_digest").unwrap_or(&Value::Null);
    match mode {
        "candidate" if revision.is_null() && git.is_null() && tree.is_null() => Ok(()),
        "exact"
            if revision.as_str().is_some_and(valid_revision)
                && git.is_object()
                && tree.as_str().is_some_and(valid_digest) =>
        {
            Ok(())
        }
        "candidate" | "exact" => Err(error("mode/revision/Git identities are inconsistent")),
        _ => Err(error("report mode is outside the v2 closed set")),
    }
}

fn validate_result(value: &Value) -> Result<(), ReportError> {
    let findings = required_array(value, "/findings")?;
    let finding_count = value
        .pointer("/result/finding_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("result.finding_count is absent or invalid"))?;
    if finding_count
        != u64::try_from(findings.len()).map_err(|failure| error(failure.to_string()))?
    {
        return Err(error("result.finding_count does not match findings"));
    }
    let status = required_string(value, "/result/status")?;
    if (findings.is_empty() && status != "pass") || (!findings.is_empty() && status != "fail") {
        return Err(error("result.status disagrees with the finding set"));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct ClosureRef {
    kind: String,
    workspace: String,
    package: String,
    target: String,
    execution: String,
}

#[derive(Debug, Clone)]
struct UnitRef {
    closure: String,
}

#[allow(clippy::too_many_lines)]
fn validate_graph_and_product_references(value: &Value) -> Result<(), ReportError> {
    let mut workspaces = BTreeSet::new();
    let mut product_workspaces = BTreeSet::new();
    for workspace in required_array(value, "/cargo_graph/workspaces")? {
        let id = field_string(workspace, "id", "cargo_graph.workspaces[]")?;
        insert_unique(&mut workspaces, id, "workspace")?;
        if field_string(workspace, "partition", "cargo_graph.workspaces[]")? == "product" {
            product_workspaces.insert(id.to_owned());
        }
    }

    for lockfile in required_array(value, "/identity_inputs/lockfiles")? {
        require_member(
            &workspaces,
            field_string(lockfile, "workspace", "identity_inputs.lockfiles[]")?,
            "lockfile workspace",
        )?;
    }

    let mut profiles = BTreeSet::new();
    for profile in required_array(value, "/identity_inputs/metadata_profiles")? {
        let workspace = field_string(profile, "workspace", "metadata_profiles[]")?;
        let profile_id = field_string(profile, "profile", "metadata_profiles[]")?;
        require_member(&workspaces, workspace, "metadata profile workspace")?;
        if !profiles.insert((workspace.to_owned(), profile_id.to_owned())) {
            return Err(error(format!(
                "duplicate metadata profile identity {workspace}/{profile_id}"
            )));
        }
    }

    for run in required_array(value, "/cargo_graph/metadata_runs")? {
        require_profile(
            &profiles,
            field_string(run, "workspace", "cargo_graph.metadata_runs[]")?,
            field_string(run, "profile", "cargo_graph.metadata_runs[]")?,
            "metadata run",
        )?;
    }

    let mut packages = BTreeSet::new();
    let mut package_targets = BTreeMap::<(String, String), BTreeSet<String>>::new();
    for package in required_array(value, "/cargo_graph/workspace_packages")? {
        let workspace = field_string(package, "workspace", "workspace_packages[]")?;
        let profile = field_string(package, "profile", "workspace_packages[]")?;
        let id = field_string(package, "id", "workspace_packages[]")?;
        require_profile(&profiles, workspace, profile, "workspace package")?;
        if !packages.insert((workspace.to_owned(), profile.to_owned(), id.to_owned())) {
            return Err(error(format!(
                "duplicate workspace/profile/package identity {workspace}/{profile}/{id}"
            )));
        }
        let targets = required_field_array(package, "targets", "workspace_packages[]")?
            .iter()
            .map(|target| {
                field_string(target, "id", "workspace_packages[].targets[]").map(str::to_owned)
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if targets.len() != required_field_array(package, "targets", "workspace_packages[]")?.len()
        {
            return Err(error(format!("package {workspace}/{id} has duplicate target IDs")));
        }
        let package_key = (workspace.to_owned(), id.to_owned());
        if let Some(existing) = package_targets.get(&package_key)
            && existing != &targets
        {
            return Err(error(format!(
                "package {workspace}/{id} changes target identity across metadata profiles"
            )));
        }
        package_targets.insert(package_key, targets);
    }

    let mut external_packages = BTreeSet::new();
    for package in required_array(value, "/cargo_graph/external_packages")? {
        let workspace = field_string(package, "workspace", "external_packages[]")?;
        let profile = field_string(package, "profile", "external_packages[]")?;
        require_profile(&profiles, workspace, profile, "external package")?;
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

    for edge in required_array(value, "/cargo_graph/edges")? {
        let workspace = field_string(edge, "workspace", "cargo_graph.edges[]")?;
        let profile = field_string(edge, "profile", "cargo_graph.edges[]")?;
        require_profile(&profiles, workspace, profile, "Cargo edge")?;
        validate_package_endpoint(
            edge.get("consumer").ok_or_else(|| error("Cargo edge consumer is absent"))?,
            profile,
            &packages,
            &external_packages,
        )?;
        validate_package_endpoint(
            edge.get("provider").ok_or_else(|| error("Cargo edge provider is absent"))?,
            profile,
            &packages,
            &external_packages,
        )?;
    }

    for reachability in required_array(value, "/cargo_graph/reachability")? {
        let workspace = field_string(reachability, "workspace", "cargo_graph.reachability[]")?;
        let profile = field_string(reachability, "profile", "cargo_graph.reachability[]")?;
        let package = field_string(reachability, "package", "cargo_graph.reachability[]")?;
        require_package(&packages, workspace, profile, package, "reachability root")?;
    }

    let product = value.get("product_proof").ok_or_else(|| error("product_proof is absent"))?;
    let closures = required_field_array(product, "closures", "product_proof")?;
    let product_arrays = [
        "compiler_units",
        "compiler_inputs",
        "extern_edges",
        "artifacts",
        "package_memberships",
        "public_consumers",
        "executions",
    ];
    let has_product_classification =
        required_array(value, "/source_inventory/classifications")?.iter().any(|classification| {
            classification.pointer("/source_owner/kind").and_then(Value::as_str) == Some("product")
        });
    if closures.is_empty() {
        if !product.get("selected_execution_environment").is_some_and(Value::is_null)
            || product_arrays.iter().any(|field| {
                product.get(*field).and_then(Value::as_array).is_none_or(|array| !array.is_empty())
            })
            || has_product_classification
            || !product_workspaces.is_empty()
        {
            return Err(error(
                "Product-empty report contains Product ownership, workspace, environment, or evidence",
            ));
        }
        validate_process_references(
            value,
            &workspaces,
            &profiles,
            &BTreeMap::new(),
            &BTreeMap::new(),
        )?;
        return Ok(());
    }
    if !has_product_classification || product_workspaces.is_empty() {
        return Err(error("Product closures lack Product source/workspace ownership"));
    }
    let selected_environment = product
        .get("selected_execution_environment")
        .and_then(Value::as_str)
        .ok_or_else(|| error("Product closures require a selected execution environment"))?;
    let substrate_environments = required_array(value, "/identity_inputs/compiler_substrates")?
        .iter()
        .map(|substrate| {
            field_string(substrate, "environment", "compiler_substrates[]").map(str::to_owned)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    require_member(
        &substrate_environments,
        selected_environment,
        "selected compiler-substrate environment",
    )?;

    let mut closure_refs = BTreeMap::new();
    for closure in closures {
        let id = field_string(closure, "id", "product_proof.closures[]")?;
        let reference = ClosureRef {
            kind: field_string(closure, "kind", "product_proof.closures[]")?.to_owned(),
            workspace: field_string(closure, "workspace", "product_proof.closures[]")?.to_owned(),
            package: field_string(closure, "package", "product_proof.closures[]")?.to_owned(),
            target: field_string(closure, "target", "product_proof.closures[]")?.to_owned(),
            execution: field_string(closure, "execution", "product_proof.closures[]")?.to_owned(),
        };
        if closure_refs.insert(id.to_owned(), reference).is_some() {
            return Err(error(format!("duplicate closure ID {id}")));
        }
    }

    let mut execution_refs = BTreeMap::<String, BTreeSet<String>>::new();
    for execution in required_field_array(product, "executions", "product_proof")? {
        let id = field_string(execution, "id", "product_proof.executions[]")?;
        if field_string(execution, "environment", "product_proof.executions[]")?
            != selected_environment
        {
            return Err(error(format!("execution {id} uses an unselected environment")));
        }
        let selected = required_field_array(execution, "closures", "product_proof.executions[]")?
            .iter()
            .map(|closure| {
                closure
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| error("execution closure reference is not a string"))
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if execution_refs.insert(id.to_owned(), selected).is_some() {
            return Err(error(format!("duplicate execution ID {id}")));
        }
    }
    for (id, closure) in &closure_refs {
        require_member(&product_workspaces, &closure.workspace, "closure Product workspace")?;
        require_package_target(
            &package_targets,
            &closure.workspace,
            &closure.package,
            &closure.target,
            "closure",
        )?;
        let execution_closures = execution_refs
            .get(&closure.execution)
            .ok_or_else(|| error(format!("closure {id} references unknown execution")))?;
        require_member(execution_closures, id, "execution closure")?;
    }
    for (execution, selected) in &execution_refs {
        if selected.is_empty() {
            return Err(error(format!("execution {execution} selects no closure")));
        }
        for closure in selected {
            let reference = closure_refs.get(closure).ok_or_else(|| {
                error(format!("execution {execution} references unknown closure {closure}"))
            })?;
            if reference.execution != *execution {
                return Err(error(format!(
                    "execution {execution} and closure {closure} disagree about ownership"
                )));
            }
        }
    }

    let mut unit_refs = BTreeMap::new();
    for unit in required_field_array(product, "compiler_units", "product_proof")? {
        let id = field_string(unit, "id", "product_proof.compiler_units[]")?;
        let workspace = field_string(unit, "workspace", "compiler_units[]")?;
        let profile = field_string(unit, "profile", "compiler_units[]")?;
        let package = field_string(unit, "package", "compiler_units[]")?;
        let target = field_string(unit, "target", "compiler_units[]")?;
        let closure = field_string(unit, "closure", "compiler_units[]")?;
        let execution = field_string(unit, "execution", "compiler_units[]")?;
        require_profile(&profiles, workspace, profile, "compiler unit")?;
        require_package_target(&package_targets, workspace, package, target, "compiler unit")?;
        let closure_ref = closure_refs
            .get(closure)
            .ok_or_else(|| error(format!("compiler unit {id} references unknown closure")))?;
        if closure_ref.workspace != workspace
            || closure_ref.package != package
            || closure_ref.target != target
            || closure_ref.execution != execution
        {
            return Err(error(format!("compiler unit {id} disagrees with its closure identity")));
        }
        if !execution_refs.contains_key(execution) {
            return Err(error(format!("compiler unit {id} references unknown execution")));
        }
        let reference = UnitRef { closure: closure.to_owned() };
        if unit_refs.insert(id.to_owned(), reference).is_some() {
            return Err(error(format!("duplicate compiler unit ID {id}")));
        }
    }

    for input in required_field_array(product, "compiler_inputs", "product_proof")? {
        validate_unit_closure_reference(input, "compiler input", &unit_refs, &closure_refs)?;
    }
    for edge in required_field_array(product, "extern_edges", "product_proof")? {
        validate_unit_closure_reference(edge, "extern edge", &unit_refs, &closure_refs)?;
        let provider =
            edge.get("provider").ok_or_else(|| error("extern edge provider is absent"))?;
        match field_string(provider, "kind", "extern edge provider")? {
            "compiler_unit" => {
                require_map_key(
                    &unit_refs,
                    field_string(provider, "unit", "extern edge provider")?,
                    "extern provider unit",
                )?;
            }
            "sysroot" => {
                let substrates = required_array(value, "/identity_inputs/compiler_substrates")?
                    .iter()
                    .map(|substrate| {
                        field_string(substrate, "id", "compiler_substrates[]").map(str::to_owned)
                    })
                    .collect::<Result<BTreeSet<_>, _>>()?;
                require_member(
                    &substrates,
                    field_string(provider, "substrate", "extern edge provider")?,
                    "extern sysroot substrate",
                )?;
            }
            _ => return Err(error("extern provider kind is invalid")),
        }
    }
    for artifact in required_field_array(product, "artifacts", "product_proof")? {
        validate_unit_closure_reference(artifact, "artifact", &unit_refs, &closure_refs)?;
    }
    for membership in required_field_array(product, "package_memberships", "product_proof")? {
        let closure = field_string(membership, "closure", "package membership")?;
        let reference = closure_refs.get(closure).ok_or_else(|| {
            error(format!("package membership references unknown closure {closure}"))
        })?;
        if reference.kind != "shipping"
            || field_string(membership, "workspace", "package membership")? != reference.workspace
            || field_string(membership, "package", "package membership")? != reference.package
        {
            return Err(error(format!(
                "package membership {closure} disagrees with its shipping closure"
            )));
        }
    }

    let mut relations = BTreeSet::new();
    for consumer in required_field_array(product, "public_consumers", "product_proof")? {
        let relation = field_string(consumer, "relation", "public consumer")?;
        insert_unique(&mut relations, relation, "public-consumer relation")?;
        let shipping = field_string(consumer, "shipping_closure", "public consumer")?;
        let validation = field_string(consumer, "consumer_closure", "public consumer")?;
        if closure_refs.get(shipping).is_none_or(|closure| closure.kind != "shipping")
            || closure_refs.get(validation).is_none_or(|closure| closure.kind != "validation")
        {
            return Err(error(format!(
                "public-consumer relation {relation} references the wrong closure kinds"
            )));
        }
    }
    validate_process_references(value, &workspaces, &profiles, &closure_refs, &execution_refs)
}

fn validate_package_endpoint(
    endpoint: &Value,
    profile: &str,
    packages: &BTreeSet<(String, String, String)>,
    external_packages: &BTreeSet<(String, String, String, String, String)>,
) -> Result<(), ReportError> {
    match field_string(endpoint, "kind", "Cargo edge endpoint")? {
        "workspace_package" => require_package(
            packages,
            field_string(endpoint, "workspace", "Cargo edge endpoint")?,
            profile,
            field_string(endpoint, "package", "Cargo edge endpoint")?,
            "Cargo edge endpoint",
        ),
        "external_package" => {
            let name = field_string(endpoint, "name", "Cargo edge endpoint")?;
            let version = field_string(endpoint, "version", "Cargo edge endpoint")?;
            let source = field_string(endpoint, "source", "Cargo edge endpoint")?;
            if external_packages.iter().any(
                |(_, candidate_profile, candidate_name, candidate_version, candidate_source)| {
                    candidate_profile == profile
                        && candidate_name == name
                        && candidate_version == version
                        && candidate_source == source
                },
            ) {
                Ok(())
            } else {
                Err(error("Cargo edge references an unknown external package"))
            }
        }
        _ => Err(error("Cargo edge endpoint kind is invalid")),
    }
}

fn validate_unit_closure_reference(
    value: &Value,
    context: &str,
    units: &BTreeMap<String, UnitRef>,
    closures: &BTreeMap<String, ClosureRef>,
) -> Result<(), ReportError> {
    let unit = field_string(value, "unit", context)?;
    let closure = field_string(value, "closure", context)?;
    let unit_ref = units
        .get(unit)
        .ok_or_else(|| error(format!("{context} references unknown compiler unit {unit}")))?;
    if !closures.contains_key(closure) || unit_ref.closure != closure {
        return Err(error(format!("{context} unit and closure identities disagree")));
    }
    Ok(())
}

fn validate_process_references(
    value: &Value,
    workspaces: &BTreeSet<String>,
    profiles: &BTreeSet<(String, String)>,
    closures: &BTreeMap<String, ClosureRef>,
    executions: &BTreeMap<String, BTreeSet<String>>,
) -> Result<(), ReportError> {
    for process in required_array(value, "/processes")? {
        let workspace = process.get("workspace").and_then(Value::as_str);
        let profile = process.get("profile").and_then(Value::as_str);
        match (workspace, profile) {
            (Some(workspace), Some(profile)) => {
                require_profile(profiles, workspace, profile, "process")?;
            }
            (Some(workspace), None) => require_member(workspaces, workspace, "process workspace")?,
            (None, Some(_)) => return Err(error("process profile has no workspace identity")),
            (None, None) => {}
        }
        if let Some(closure) = process.get("closure").and_then(Value::as_str) {
            require_map_key(closures, closure, "process closure")?;
        }
        if let Some(execution) = process.get("execution").and_then(Value::as_str) {
            require_map_key(executions, execution, "process execution")?;
        }
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
        Err(error(format!("{context} references unknown workspace/profile {workspace}/{profile}")))
    }
}

fn require_package(
    packages: &BTreeSet<(String, String, String)>,
    workspace: &str,
    profile: &str,
    package: &str,
    context: &str,
) -> Result<(), ReportError> {
    if packages.contains(&(workspace.to_owned(), profile.to_owned(), package.to_owned())) {
        Ok(())
    } else {
        Err(error(format!(
            "{context} references unknown workspace/profile/package {workspace}/{profile}/{package}"
        )))
    }
}

fn require_package_target(
    packages: &BTreeMap<(String, String), BTreeSet<String>>,
    workspace: &str,
    package: &str,
    target: &str,
    context: &str,
) -> Result<(), ReportError> {
    if packages
        .get(&(workspace.to_owned(), package.to_owned()))
        .is_some_and(|targets| targets.contains(target))
    {
        Ok(())
    } else {
        Err(error(format!(
            "{context} references unknown workspace/package/target {workspace}/{package}/{target}"
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

fn require_member(set: &BTreeSet<String>, identity: &str, kind: &str) -> Result<(), ReportError> {
    if set.contains(identity) {
        Ok(())
    } else {
        Err(error(format!("{kind} references unknown identity {identity}")))
    }
}

fn require_map_key<T>(
    map: &BTreeMap<String, T>,
    identity: &str,
    kind: &str,
) -> Result<(), ReportError> {
    if map.contains_key(identity) {
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
    value
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| error(format!("{context}.{field} is absent or not an array")))
}

fn required_array<'a>(value: &'a Value, pointer: &str) -> Result<&'a [Value], ReportError> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| error(format!("{pointer} is absent or not an array")))
}

fn required_string<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, ReportError> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| error(format!("{pointer} is absent or not a string")))
}

fn field_string<'a>(value: &'a Value, field: &str, context: &str) -> Result<&'a str, ReportError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| error(format!("{context}.{field} is absent or not a string")))
}

fn valid_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn schema_error(failure: &schema::SchemaError) -> ReportError {
    error(failure.to_string())
}

fn error(message: impl Into<String>) -> ReportError {
    ReportError(message.into())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{BuiltReport, SCHEMA};

    const REPORT_LIMIT: usize = 4 * 1024 * 1024;
    const JSON_DEPTH: usize = 256;
    const JSON_ITEMS: usize = 1_000_000;

    #[test]
    fn candidate_product_empty_round_trip_preserves_ordered_argv() {
        let original = candidate_report();
        let ordered = original["identity_inputs"]["cargo"]["version_argv"].clone();
        let original_exclusions = original["source_inventory"]["exclusions"].clone();
        let built = super::build(original, REPORT_LIMIT).unwrap();

        assert_eq!(built.value["identity_inputs"]["cargo"]["version_argv"], ordered);
        assert_ne!(built.value["source_inventory"]["exclusions"], original_exclusions);
        assert_eq!(built.bytes.last(), Some(&b'\n'));
        let verified = super::verify(&built.bytes, REPORT_LIMIT, JSON_DEPTH, JSON_ITEMS).unwrap();
        assert_eq!(verified.identity, built.identity);
        assert_eq!(verified.mode, "candidate");
        assert_eq!(verified.status, "pass");
    }

    #[test]
    fn ordered_argv_changes_identity_and_is_never_set_sorted() {
        let mut left = candidate_report();
        left["identity_inputs"]["cargo"]["version_argv"] =
            json!(["cargo", "--version", "--verbose"]);
        let mut right = left.clone();
        right["identity_inputs"]["cargo"]["version_argv"] =
            json!(["cargo", "--verbose", "--version"]);

        let left = super::build(left, REPORT_LIMIT).unwrap();
        let right = super::build(right, REPORT_LIMIT).unwrap();
        assert_eq!(
            left.value["identity_inputs"]["cargo"]["version_argv"],
            json!(["cargo", "--version", "--verbose"])
        );
        assert_eq!(
            right.value["identity_inputs"]["cargo"]["version_argv"],
            json!(["cargo", "--verbose", "--version"])
        );
        assert_ne!(left.identity, right.identity);
    }

    #[test]
    fn standalone_verifier_rejects_order_identity_and_final_lf_mutants() {
        let built = super::build(candidate_report(), REPORT_LIMIT).unwrap();

        let mut unordered = built.value.clone();
        unordered["source_inventory"]["exclusions"].as_array_mut().unwrap().reverse();
        let unordered = serialized(&unordered);
        assert!(
            super::verify(&unordered, REPORT_LIMIT, JSON_DEPTH, JSON_ITEMS)
                .unwrap_err()
                .0
                .contains("canonical")
        );

        let mut identity = built.value.clone();
        identity["report_identity"] = json!(digest('9'));
        let identity = serialized(&identity);
        assert!(
            super::verify(&identity, REPORT_LIMIT, JSON_DEPTH, JSON_ITEMS)
                .unwrap_err()
                .0
                .contains("identity mismatch")
        );

        let without_lf = &built.bytes[..built.bytes.len() - 1];
        assert!(
            super::verify(without_lf, REPORT_LIMIT, JSON_DEPTH, JSON_ITEMS)
                .unwrap_err()
                .0
                .contains("canonical LF")
        );
    }

    #[test]
    fn semantic_verifier_rejects_product_empty_and_reference_mutants() {
        let mut product = candidate_report();
        product["source_inventory"]["classifications"][0]["source_owner"] =
            json!({"kind": "product", "role": "Foundation"});
        product["source_inventory"]["classifications"][0]["source_use"] = json!("production");
        assert!(super::build(product, REPORT_LIMIT).unwrap_err().0.contains("Product-empty"));

        let mut duplicate_workspace = candidate_report();
        duplicate_workspace["cargo_graph"]["workspaces"].as_array_mut().unwrap().push(json!({
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
                .0
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
                .0
                .contains("unknown workspace/profile")
        );
    }

    #[test]
    fn build_and_verify_enforce_both_report_byte_limits() {
        assert!(super::build(candidate_report(), 32).unwrap_err().0.contains("caller limit"));
        let built = super::build(candidate_report(), REPORT_LIMIT).unwrap();
        assert!(
            super::verify(&built.bytes, 32, JSON_DEPTH, JSON_ITEMS)
                .unwrap_err()
                .0
                .contains("caller-owned")
        );
    }

    fn serialized(value: &Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(value).unwrap();
        bytes.push(b'\n');
        bytes
    }

    #[allow(clippy::too_many_lines)]
    fn candidate_report() -> Value {
        let report_schema_digest =
            crate::digest::bytes(crate::schema::embedded_report_schema_bytes());
        json!({
            "schema": SCHEMA,
            "subject": {"kind": "active_repository"},
            "mode": "candidate",
            "revision": null,
            "identity_inputs": {
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
            },
            "limits": limits(),
            "source_inventory": {
                "classifications": [{
                    "path": "Cargo.toml",
                    "content_digest": digest('4'),
                    "size_bytes": 1,
                    "mode": "100644",
                    "source_owner": {"kind": "repository_tooling"},
                    "source_provenance": "authored",
                    "source_use": "tool",
                    "upstream": null,
                    "winning_rule": {"index": 0, "prefix": "Cargo.toml", "precedence": 1}
                }],
                "exclusions": [
                    {"prefix": ".git", "kind": "vcs_metadata"},
                    {"prefix": "tmp", "kind": "working_notes"},
                    {"prefix": "target", "kind": "transient_build_output"}
                ]
            },
            "cargo_graph": {
                "metadata_runs": [],
                "workspaces": [{
                    "id": "root",
                    "partition": "repository_tooling",
                    "root": ".",
                    "manifest": "Cargo.toml",
                    "lockfile": "Cargo.lock",
                    "members": []
                }],
                "workspace_packages": [],
                "external_packages": [],
                "edges": [],
                "reachability": []
            },
            "product_proof": {
                "selected_execution_environment": null,
                "closures": [],
                "compiler_units": [],
                "compiler_inputs": [],
                "extern_edges": [],
                "artifacts": [],
                "package_memberships": [],
                "public_consumers": [],
                "executions": []
            },
            "processes": [],
            "findings": [],
            "result": {"status": "pass", "finding_count": 0},
            "report_identity": null
        })
    }

    fn artifact(path: &str, schema: &str) -> Value {
        json!({
            "path": path,
            "schema": schema,
            "content_digest": digest('5'),
            "schema_path": "conformance/source-classification.schema.json",
            "schema_digest": digest('6')
        })
    }

    fn tool(path: &str) -> Value {
        json!({
            "canonical_path": path,
            "executable_digest": digest('a'),
            "executable_size_bytes": 1,
            "version_argv": [path, "--version", "--version"],
            "version": "tool 1",
            "version_digest": digest('b'),
            "runtime_inputs": []
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
