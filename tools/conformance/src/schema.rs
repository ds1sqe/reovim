//! Small fail-closed validator for the committed generated-report schemas.
//!
//! It implements exactly the JSON Schema keywords used by the frozen v1 and
//! active v2 schemas. Encountering an unsupported assertion keyword is itself
//! an error.

use std::{collections::BTreeSet, fmt};

use serde_json::Value;

const REPORT_SCHEMA_V1: &str =
    include_str!("../../../conformance/compat/v1/source-role-map.schema.json");
const REPORT_SCHEMA_V2: &str =
    include_str!("../../../conformance/source-role-map.schema.json");
const ABSOLUTE_PATH_PATTERN: &str = concat!(
    r"^/(?!.*//)(?!.*(?:^|/)\.{1,2}(?:/|$))",
    r"(?!.*/$)(?!.*\u0000).+$",
);
const RELATIVE_PATH_PATTERN: &str = concat!(
    r"^(?!/)(?![A-Za-z]:)(?!.*\\)(?!.*//)",
    r"(?!.*(?:^|/)\.{1,2}(?:/|$))(?!.*/$)(?!.*\u0000).+$",
);

#[must_use]
pub const fn embedded_report_schema_bytes() -> &'static [u8] {
    REPORT_SCHEMA_V2.as_bytes()
}

#[must_use]
pub const fn embedded_v1_report_schema_bytes() -> &'static [u8] {
    REPORT_SCHEMA_V1.as_bytes()
}

#[derive(Debug, Clone)]
pub struct SchemaError {
    pub path: String,
    pub message: String,
}

impl fmt::Display for SchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for SchemaError {}

pub fn validate_report(instance: &Value) -> Result<(), SchemaError> {
    let identity =
        instance.get("schema").and_then(Value::as_str).ok_or_else(|| {
            schema_error(
                "$.schema",
                "required report schema identity is absent",
            )
        })?;
    let schema_bytes = match identity {
        "reovim-source-role-map/v1" => REPORT_SCHEMA_V1,
        "reovim-source-role-map/v2" => REPORT_SCHEMA_V2,
        other => {
            return Err(schema_error(
                "$.schema",
                &format!("unsupported report schema identity {other:?}"),
            ));
        }
    };
    let schema: Value =
        serde_json::from_str(schema_bytes).map_err(|error| SchemaError {
            path: "$schema".to_owned(),
            message: error.to_string(),
        })?;
    validate_instance(instance, &schema)
}

/// Canonicalizes only arrays whose selected schema explicitly declares
/// `uniqueItems: true`. Ordered arrays such as argv remain byte-order
/// significant.
pub fn canonicalize_unique_arrays(
    instance: &mut Value,
    schema: &Value,
) -> Result<(), SchemaError> {
    validate_schema(schema)?;
    canonicalize(instance, schema, schema, "$")
}

fn canonicalize(
    instance: &mut Value,
    schema: &Value,
    root: &Value,
    path: &str,
) -> Result<(), SchemaError> {
    let object = schema
        .as_object()
        .ok_or_else(|| schema_error(path, "schema node is not an object"))?;
    if let Some(reference) = object.get("$ref") {
        let reference = reference
            .as_str()
            .ok_or_else(|| schema_error(path, "$ref is not a string"))?;
        let pointer = reference.strip_prefix('#').ok_or_else(|| {
            schema_error(path, "only local schema references are supported")
        })?;
        let target = root.pointer(pointer).ok_or_else(|| {
            schema_error(
                path,
                &format!("unresolved schema reference {reference}"),
            )
        })?;
        canonicalize(instance, target, root, path)?;
    }
    if let Some(branches) = object.get("allOf") {
        for branch in branches
            .as_array()
            .ok_or_else(|| schema_error(path, "schema allOf is not an array"))?
        {
            canonicalize(instance, branch, root, path)?;
        }
    }
    if let Some(branches) = object.get("oneOf") {
        let branches = branches.as_array().ok_or_else(|| {
            schema_error(path, "schema oneOf is not an array")
        })?;
        let selected = branches
            .iter()
            .filter(|branch| validate(instance, branch, root, path).is_ok())
            .collect::<Vec<_>>();
        if selected.len() != 1 {
            return Err(schema_error(
                path,
                &format!(
                    "oneOf matched {} branches during canonicalization",
                    selected.len()
                ),
            ));
        }
        canonicalize(instance, selected[0], root, path)?;
    }
    if let Some(condition) = object.get("if") {
        let selected = if validate(instance, condition, root, path).is_ok() {
            object.get("then")
        } else {
            object.get("else")
        };
        if let Some(selected) = selected {
            canonicalize(instance, selected, root, path)?;
        }
    }
    canonicalize_properties(instance, object, root, path)?;
    canonicalize_items(instance, object, root, path)?;
    Ok(())
}

fn canonicalize_properties(
    instance: &mut Value,
    schema: &serde_json::Map<String, Value>,
    root: &Value,
    path: &str,
) -> Result<(), SchemaError> {
    let (Some(values), Some(properties)) = (
        instance.as_object_mut(),
        schema.get("properties").and_then(Value::as_object),
    ) else {
        return Ok(());
    };
    for (field, subschema) in properties {
        canonicalize_property(
            values.get_mut(field),
            subschema,
            root,
            path,
            field,
        )?;
    }
    Ok(())
}

fn canonicalize_property(
    value: Option<&mut Value>,
    schema: &Value,
    root: &Value,
    path: &str,
    field: &str,
) -> Result<(), SchemaError> {
    let Some(value) = value else {
        return Ok(());
    };
    canonicalize(value, schema, root, &child(path, field))
}

fn canonicalize_items(
    instance: &mut Value,
    schema: &serde_json::Map<String, Value>,
    root: &Value,
    path: &str,
) -> Result<(), SchemaError> {
    let (Some(values), Some(items)) =
        (instance.as_array_mut(), schema.get("items"))
    else {
        return Ok(());
    };
    for (index, value) in values.iter_mut().enumerate() {
        canonicalize(value, items, root, &format!("{path}[{index}]"))?;
    }
    if schema.get("uniqueItems") == Some(&Value::Bool(true)) {
        values.sort_by_cached_key(Value::to_string);
    }
    Ok(())
}

pub fn validate_instance(
    instance: &Value,
    schema: &Value,
) -> Result<(), SchemaError> {
    validate_schema(schema)?;
    validate(instance, schema, schema, "$")
}

pub fn validate_schema(schema: &Value) -> Result<(), SchemaError> {
    preflight_schema(schema, schema, "#", &mut Vec::new(), 0)
}

fn validate(
    instance: &Value,
    schema: &Value,
    root: &Value,
    path: &str,
) -> Result<(), SchemaError> {
    let object = schema.as_object().ok_or_else(|| SchemaError {
        path: path.to_owned(),
        message: "embedded schema node is not an object".to_owned(),
    })?;
    if let Some(reference) = object.get("$ref") {
        let reference = reference
            .as_str()
            .ok_or_else(|| schema_error(path, "$ref is not a string"))?;
        let pointer = reference.strip_prefix('#').ok_or_else(|| {
            schema_error(path, "only local schema references are supported")
        })?;
        let target = root.pointer(pointer).ok_or_else(|| {
            schema_error(
                path,
                &format!("unresolved schema reference {reference}"),
            )
        })?;
        validate(instance, target, root, path)?;
    }
    if let Some(expected) = object.get("const")
        && instance != expected
    {
        return Err(schema_error(path, "value differs from schema const"));
    }
    if let Some(values) = object.get("enum") {
        let values = values
            .as_array()
            .ok_or_else(|| schema_error(path, "schema enum is not an array"))?;
        if !values.contains(instance) {
            return Err(schema_error(path, "value is outside schema enum"));
        }
    }
    if let Some(kind) = object.get("type") {
        let kind = kind
            .as_str()
            .ok_or_else(|| schema_error(path, "schema type is not a string"))?;
        let matches = match kind {
            "object" => instance.is_object(),
            "array" => instance.is_array(),
            "string" => instance.is_string(),
            "integer" => {
                instance.as_i64().is_some() || instance.as_u64().is_some()
            }
            "boolean" => instance.is_boolean(),
            "null" => instance.is_null(),
            other => {
                return Err(schema_error(
                    path,
                    &format!("unsupported schema type {other}"),
                ));
            }
        };
        if !matches {
            return Err(schema_error(path, &format!("expected {kind}")));
        }
    }
    if let Some(branches) = object.get("oneOf") {
        let branches = branches.as_array().ok_or_else(|| {
            schema_error(path, "schema oneOf is not an array")
        })?;
        let matches = branches
            .iter()
            .filter(|branch| validate(instance, branch, root, path).is_ok())
            .count();
        if matches != 1 {
            return Err(schema_error(
                path,
                &format!("oneOf matched {matches} branches"),
            ));
        }
    }
    if let Some(branches) = object.get("allOf") {
        for branch in branches
            .as_array()
            .ok_or_else(|| schema_error(path, "schema allOf is not an array"))?
        {
            validate(instance, branch, root, path)?;
        }
    }
    if let Some(denied) = object.get("not")
        && validate(instance, denied, root, path).is_ok()
    {
        return Err(schema_error(path, "value matches forbidden `not` schema"));
    }
    if let Some(condition) = object.get("if") {
        let selected = if validate(instance, condition, root, path).is_ok() {
            object.get("then")
        } else {
            object.get("else")
        };
        if let Some(selected) = selected {
            validate(instance, selected, root, path)?;
        }
    }
    validate_object(instance, object, root, path)?;
    validate_array(instance, object, root, path)?;
    validate_string(instance, object, path)?;
    validate_number(instance, object, path)?;
    reject_unknown_keywords(object, path)
}

fn validate_object(
    instance: &Value,
    schema: &serde_json::Map<String, Value>,
    root: &Value,
    path: &str,
) -> Result<(), SchemaError> {
    let Some(instance) = instance.as_object() else { return Ok(()) };
    validate_required(instance, schema.get("required"), path)?;
    let properties = schema.get("properties").and_then(Value::as_object);
    validate_additional_properties(instance, schema, properties, path)?;
    validate_properties(instance, properties, root, path)?;
    Ok(())
}

fn validate_required(
    instance: &serde_json::Map<String, Value>,
    required: Option<&Value>,
    path: &str,
) -> Result<(), SchemaError> {
    let Some(required) = required else {
        return Ok(());
    };
    let required = required
        .as_array()
        .ok_or_else(|| schema_error(path, "schema required is not an array"))?;
    for field in required {
        validate_required_field(instance, field, path)?;
    }
    Ok(())
}

fn validate_required_field(
    instance: &serde_json::Map<String, Value>,
    field: &Value,
    path: &str,
) -> Result<(), SchemaError> {
    let field = field.as_str().ok_or_else(|| {
        schema_error(path, "schema required member is not a string")
    })?;
    if instance.contains_key(field) {
        return Ok(());
    }
    Err(schema_error(&child(path, field), "required field is absent"))
}

fn validate_additional_properties(
    instance: &serde_json::Map<String, Value>,
    schema: &serde_json::Map<String, Value>,
    properties: Option<&serde_json::Map<String, Value>>,
    path: &str,
) -> Result<(), SchemaError> {
    if schema.get("additionalProperties") != Some(&Value::Bool(false)) {
        return Ok(());
    }
    for field in instance.keys() {
        validate_known_field(properties, path, field)?;
    }
    Ok(())
}

fn validate_known_field(
    properties: Option<&serde_json::Map<String, Value>>,
    path: &str,
    field: &str,
) -> Result<(), SchemaError> {
    if properties.is_some_and(|properties| properties.contains_key(field)) {
        return Ok(());
    }
    Err(schema_error(&child(path, field), "unknown field"))
}

fn validate_properties(
    instance: &serde_json::Map<String, Value>,
    properties: Option<&serde_json::Map<String, Value>>,
    root: &Value,
    path: &str,
) -> Result<(), SchemaError> {
    let Some(properties) = properties else {
        return Ok(());
    };
    for (field, schema) in properties {
        validate_property(instance.get(field), schema, root, path, field)?;
    }
    Ok(())
}

fn validate_property(
    value: Option<&Value>,
    schema: &Value,
    root: &Value,
    path: &str,
    field: &str,
) -> Result<(), SchemaError> {
    let Some(value) = value else {
        return Ok(());
    };
    validate(value, schema, root, &child(path, field))
}

fn validate_array(
    instance: &Value,
    schema: &serde_json::Map<String, Value>,
    root: &Value,
    path: &str,
) -> Result<(), SchemaError> {
    let Some(instance) = instance.as_array() else { return Ok(()) };
    if let Some(minimum) = schema.get("minItems").and_then(Value::as_u64)
        && u64::try_from(instance.len())
            .map_err(|error| schema_error(path, &error.to_string()))?
            < minimum
    {
        return Err(schema_error(path, "array has fewer than minItems"));
    }
    if let Some(maximum) = schema.get("maxItems").and_then(Value::as_u64)
        && u64::try_from(instance.len())
            .map_err(|error| schema_error(path, &error.to_string()))?
            > maximum
    {
        return Err(schema_error(path, "array has more than maxItems"));
    }
    validate_unique_items(instance, schema, path)?;
    if let Some(items) = schema.get("items") {
        for (index, value) in instance.iter().enumerate() {
            validate(value, items, root, &format!("{path}[{index}]"))?;
        }
    }
    Ok(())
}

fn validate_unique_items(
    instance: &[Value],
    schema: &serde_json::Map<String, Value>,
    path: &str,
) -> Result<(), SchemaError> {
    if schema.get("uniqueItems") != Some(&Value::Bool(true)) {
        return Ok(());
    }
    let mut seen = BTreeSet::new();
    for value in instance {
        let encoded = serde_json::to_string(value)
            .map_err(|error| schema_error(path, &error.to_string()))?;
        if !seen.insert(encoded) {
            return Err(schema_error(path, "array items are not unique"));
        }
    }
    Ok(())
}

fn validate_string(
    instance: &Value,
    schema: &serde_json::Map<String, Value>,
    path: &str,
) -> Result<(), SchemaError> {
    let Some(instance) = instance.as_str() else { return Ok(()) };
    if let Some(minimum) = schema.get("minLength").and_then(Value::as_u64)
        && u64::try_from(instance.chars().count())
            .map_err(|error| schema_error(path, &error.to_string()))?
            < minimum
    {
        return Err(schema_error(path, "string is shorter than minLength"));
    }
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str)
        && !pattern_matches(pattern, instance)?
    {
        return Err(schema_error(path, "string does not match schema pattern"));
    }
    if let Some(format) = schema.get("format").and_then(Value::as_str) {
        match format {
            "date-time" if utc_timestamp(instance) => {}
            "date-time" => {
                return Err(schema_error(
                    path,
                    "string is not the admitted UTC date-time form",
                ));
            }
            other => {
                return Err(schema_error(
                    path,
                    &format!("unsupported schema format {other}"),
                ));
            }
        }
    }
    Ok(())
}

fn validate_number(
    instance: &Value,
    schema: &serde_json::Map<String, Value>,
    path: &str,
) -> Result<(), SchemaError> {
    let Some(instance) = instance
        .as_i64()
        .map(i128::from)
        .or_else(|| instance.as_u64().map(i128::from))
    else {
        return Ok(());
    };
    if let Some(minimum) = schema.get("minimum").and_then(Value::as_u64)
        && instance < i128::from(minimum)
    {
        return Err(schema_error(path, "integer is less than minimum"));
    }
    if let Some(maximum) = schema.get("maximum").and_then(Value::as_u64)
        && instance > i128::from(maximum)
    {
        return Err(schema_error(path, "integer exceeds maximum"));
    }
    Ok(())
}

fn pattern_matches(pattern: &str, value: &str) -> Result<bool, SchemaError> {
    match pattern {
        "^sha256:[0-9a-f]{64}$" => Ok(value.len() == 71
            && value.starts_with("sha256:")
            && lower_hex(&value[7..])),
        "^(?:[0-9a-f]{40}|[0-9a-f]{64})$" => {
            Ok(matches!(value.len(), 40 | 64) && lower_hex(value))
        }
        "^[0-9a-f]{64}$" => Ok(value.len() == 64 && lower_hex(value)),
        "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$" => {
            Ok(utc_timestamp(value))
        }
        "^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$" => Ok(kebab_identifier(value)),
        "^[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*$" => {
            Ok(rust_public_path(value))
        }
        ABSOLUTE_PATH_PATTERN => Ok(value.starts_with('/')
            && value.len() > 1
            && !value.ends_with('/')
            && !value.contains(['\0'])
            && !value.contains("//")
            && !value
                .split('/')
                .any(|component| matches!(component, "." | ".."))),
        RELATIVE_PATH_PATTERN => Ok(!(value.is_empty()
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains(['\\', '\0'])
            || value.contains("//")
            || value.len() >= 2 && value.as_bytes()[1] == b':'
            || value
                .split('/')
                .any(|component| matches!(component, "." | "..")))),
        other => Err(SchemaError {
            path: "$schema.pattern".to_owned(),
            message: format!("unsupported embedded schema pattern {other:?}"),
        }),
    }
}

fn reject_unknown_keywords(
    schema: &serde_json::Map<String, Value>,
    path: &str,
) -> Result<(), SchemaError> {
    const SUPPORTED: &[&str] = &[
        "$schema",
        "$id",
        "$ref",
        "$defs",
        "title",
        "type",
        "const",
        "enum",
        "oneOf",
        "allOf",
        "if",
        "then",
        "else",
        "not",
        "required",
        "properties",
        "additionalProperties",
        "items",
        "minItems",
        "maxItems",
        "uniqueItems",
        "minLength",
        "pattern",
        "format",
        "minimum",
        "maximum",
    ];
    for keyword in schema.keys() {
        if !SUPPORTED.contains(&keyword.as_str()) {
            return Err(schema_error(
                path,
                &format!("unsupported embedded schema keyword {keyword}"),
            ));
        }
    }
    Ok(())
}

fn preflight_schema(
    schema: &Value,
    root: &Value,
    path: &str,
    references: &mut Vec<String>,
    depth: usize,
) -> Result<(), SchemaError> {
    if depth > 256 {
        return Err(schema_error(path, "schema validation depth exceeds 256"));
    }
    let object = schema
        .as_object()
        .ok_or_else(|| schema_error(path, "schema node is not an object"))?;
    reject_unknown_keywords(object, path)?;
    validate_keyword_shapes(object, path)?;
    preflight_reference(object.get("$ref"), root, path, references, depth)?;
    for map_name in ["$defs", "properties"] {
        preflight_map(
            object.get(map_name),
            map_name,
            root,
            path,
            references,
            depth,
        )?;
    }
    for array_name in ["oneOf", "allOf"] {
        preflight_array(
            object.get(array_name),
            array_name,
            root,
            path,
            references,
            depth,
        )?;
    }
    for child_name in ["items", "if", "then", "else", "not"] {
        if let Some(child) = object.get(child_name) {
            preflight_schema(
                child,
                root,
                &format!("{path}/{child_name}"),
                references,
                depth + 1,
            )?;
        }
    }
    Ok(())
}

fn preflight_reference(
    reference: Option<&Value>,
    root: &Value,
    path: &str,
    references: &mut Vec<String>,
    depth: usize,
) -> Result<(), SchemaError> {
    let Some(reference) = reference else {
        return Ok(());
    };
    let reference = reference
        .as_str()
        .ok_or_else(|| schema_error(path, "$ref is not a string"))?;
    let pointer = reference.strip_prefix('#').ok_or_else(|| {
        schema_error(path, "only local schema references are supported")
    })?;
    if references.iter().any(|active| active == reference) {
        return Err(schema_error(
            path,
            &format!("cyclic schema reference {reference}"),
        ));
    }
    let target = root.pointer(pointer).ok_or_else(|| {
        schema_error(path, &format!("unresolved schema reference {reference}"))
    })?;
    references.push(reference.to_owned());
    let result =
        preflight_schema(target, root, reference, references, depth + 1);
    references.pop();
    result
}

fn preflight_map(
    children: Option<&Value>,
    map_name: &str,
    root: &Value,
    path: &str,
    references: &mut Vec<String>,
    depth: usize,
) -> Result<(), SchemaError> {
    let Some(children) = children else {
        return Ok(());
    };
    let children = children.as_object().ok_or_else(|| {
        schema_error(path, &format!("{map_name} is not an object"))
    })?;
    for (name, child) in children {
        preflight_schema(
            child,
            root,
            &format!("{path}/{map_name}/{name}"),
            references,
            depth + 1,
        )?;
    }
    Ok(())
}

fn preflight_array(
    children: Option<&Value>,
    array_name: &str,
    root: &Value,
    path: &str,
    references: &mut Vec<String>,
    depth: usize,
) -> Result<(), SchemaError> {
    let Some(children) = children else {
        return Ok(());
    };
    let children = children.as_array().ok_or_else(|| {
        schema_error(path, &format!("{array_name} is not an array"))
    })?;
    for (index, child) in children.iter().enumerate() {
        preflight_schema(
            child,
            root,
            &format!("{path}/{array_name}/{index}"),
            references,
            depth + 1,
        )?;
    }
    Ok(())
}

fn validate_keyword_shapes(
    schema: &serde_json::Map<String, Value>,
    path: &str,
) -> Result<(), SchemaError> {
    for name in ["$schema", "$id", "$ref", "title", "type", "pattern", "format"]
    {
        if schema.get(name).is_some_and(|value| !value.is_string()) {
            return Err(schema_error(
                path,
                &format!("schema {name} must be a string"),
            ));
        }
    }
    if let Some(kind) = schema.get("type").and_then(Value::as_str)
        && !matches!(
            kind,
            "object" | "array" | "string" | "integer" | "boolean" | "null"
        )
    {
        return Err(schema_error(
            path,
            &format!("unsupported schema type {kind}"),
        ));
    }
    for name in ["$defs", "properties", "items", "if", "then", "else", "not"] {
        if schema.get(name).is_some_and(|value| !value.is_object()) {
            return Err(schema_error(
                path,
                &format!("schema {name} must be an object"),
            ));
        }
    }
    for name in ["oneOf", "allOf", "required", "enum"] {
        validate_array_keyword(name, schema.get(name), path)?;
    }
    for name in ["additionalProperties", "uniqueItems"] {
        if schema.get(name).is_some_and(|value| !value.is_boolean()) {
            return Err(schema_error(
                path,
                &format!("schema {name} must be boolean"),
            ));
        }
    }
    for name in ["minItems", "maxItems", "minLength", "minimum", "maximum"] {
        if schema.get(name).is_some_and(|value| value.as_u64().is_none()) {
            return Err(schema_error(
                path,
                &format!("schema {name} must be a nonnegative integer"),
            ));
        }
    }
    for (minimum, maximum) in [("minItems", "maxItems"), ("minimum", "maximum")]
    {
        if let (Some(minimum), Some(maximum)) = (
            schema.get(minimum).and_then(Value::as_u64),
            schema.get(maximum).and_then(Value::as_u64),
        ) && minimum > maximum
        {
            return Err(schema_error(path, "schema minimum exceeds maximum"));
        }
    }
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        let _ = pattern_matches(pattern, "")?;
    }
    if let Some(format) = schema.get("format").and_then(Value::as_str)
        && format != "date-time"
    {
        return Err(schema_error(
            path,
            &format!("unsupported schema format {format}"),
        ));
    }
    Ok(())
}

fn validate_array_keyword(
    name: &str,
    value: Option<&Value>,
    path: &str,
) -> Result<(), SchemaError> {
    let Some(value) = value else {
        return Ok(());
    };
    let values = value.as_array().ok_or_else(|| {
        schema_error(path, &format!("schema {name} must be an array"))
    })?;
    if matches!(name, "oneOf" | "allOf" | "enum") && values.is_empty() {
        return Err(schema_error(
            path,
            &format!("schema {name} must not be empty"),
        ));
    }
    if name == "required" && values.iter().any(|value| !value.is_string()) {
        return Err(schema_error(
            path,
            "schema required entries must be strings",
        ));
    }
    let encoded = values.iter().map(Value::to_string).collect::<BTreeSet<_>>();
    if encoded.len() != values.len() {
        return Err(schema_error(
            path,
            &format!("schema {name} contains duplicates"),
        ));
    }
    Ok(())
}

pub fn utc_timestamp(value: &str) -> bool {
    if !(value.len() == 20
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value.as_bytes()[10] == b'T'
        && value.as_bytes()[13] == b':'
        && value.as_bytes()[16] == b':'
        && value.as_bytes()[19] == b'Z'
        && value.bytes().enumerate().all(|(index, byte)| {
            matches!(index, 4 | 7 | 10 | 13 | 16 | 19) || byte.is_ascii_digit()
        }))
    {
        return false;
    }
    let number =
        |range: std::ops::Range<usize>| value.get(range)?.parse::<u32>().ok();
    let (
        Some(year),
        Some(month),
        Some(day),
        Some(hour),
        Some(minute),
        Some(second),
    ) = (
        number(0..4),
        number(5..7),
        number(8..10),
        number(11..13),
        number(14..16),
        number(17..19),
    )
    else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=max_day).contains(&day) && hour < 24 && minute < 60 && second < 60
}

fn lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn kebab_identifier(value: &str) -> bool {
    let mut segments = value.split('-');
    let Some(first) = segments.next() else { return false };
    !first.is_empty()
        && first.as_bytes()[0].is_ascii_lowercase()
        && first
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && segments.all(|segment| {
            !segment.is_empty()
                && segment.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit()
                })
        })
}

fn rust_public_path(value: &str) -> bool {
    !value.is_empty() && value.split("::").all(rust_identifier)
}

fn rust_identifier(value: &str) -> bool {
    let Some(first) = value.bytes().next() else { return false };
    (first.is_ascii_alphabetic() || first == b'_')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn child(parent: &str, field: &str) -> String {
    format!("{parent}.{field}")
}

fn schema_error(path: &str, message: &str) -> SchemaError {
    SchemaError { path: path.to_owned(), message: message.to_owned() }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn closed_report_rejects_unknown_and_missing_fields() {
        let mut value = json!({});
        let error = super::validate_report(&value).unwrap_err();
        assert!(error.message.contains("required"));
        value
            .as_object_mut()
            .expect("object")
            .insert("unknown".to_owned(), json!(true));
        assert!(super::validate_report(&value).is_err());
    }

    #[test]
    fn schema_preflight_rejects_unresolved_and_cyclic_refs() {
        let unresolved = json!({"$ref": "#/$defs/missing", "$defs": {}});
        assert!(
            super::validate_schema(&unresolved)
                .unwrap_err()
                .message
                .contains("unresolved")
        );
        let cyclic = json!({
            "$ref": "#/$defs/self",
            "$defs": {"self": {"$ref": "#/$defs/self"}}
        });
        assert!(
            super::validate_schema(&cyclic)
                .unwrap_err()
                .message
                .contains("cyclic")
        );
    }

    #[test]
    fn normalized_relative_path_pattern_rejects_alias_forms() {
        let schema = json!({
            "type": "string",
            "pattern": super::RELATIVE_PATH_PATTERN
        });
        for invalid in [".", "a/./b", "a//b", "a\\b", "C:/b", "a/"] {
            assert!(
                super::validate_instance(&json!(invalid), &schema).is_err(),
                "{invalid}"
            );
        }
        super::validate_instance(&json!("a/b"), &schema).unwrap();
    }

    #[test]
    fn date_time_is_range_checked() {
        let schema = json!({"type": "string", "format": "date-time"});
        super::validate_instance(&json!("2024-02-29T23:59:59Z"), &schema)
            .unwrap();
        assert!(
            super::validate_instance(&json!("2025-02-29T25:60:60Z"), &schema)
                .is_err()
        );
    }

    #[test]
    fn signed_integer_still_obeys_nonnegative_minimum() {
        let schema = json!({"type": "integer", "minimum": 0});
        assert!(super::validate_instance(&json!(-1), &schema).is_err());
        super::validate_instance(&json!(0), &schema).unwrap();
    }

    #[test]
    fn v2_identifier_absolute_and_public_path_patterns_are_closed() {
        let identifier = json!({
            "type": "string",
            "pattern": "^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$"
        });
        for valid in ["a", "source-role-map-v2", "tool9"] {
            super::validate_instance(&json!(valid), &identifier).unwrap();
        }
        for invalid in ["", "9tool", "Tool", "tool_1", "tool--one", "tool-"] {
            assert!(
                super::validate_instance(&json!(invalid), &identifier).is_err()
            );
        }

        let absolute = json!({
            "type": "string",
            "pattern": super::ABSOLUTE_PATH_PATTERN
        });
        super::validate_instance(&json!("/usr/bin/git"), &absolute).unwrap();
        for invalid in ["/", "relative", "/a//b", "/a/../b", "/a/"] {
            assert!(
                super::validate_instance(&json!(invalid), &absolute).is_err()
            );
        }

        let public_path = json!({
            "type": "string",
            "pattern": "^[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*$"
        });
        super::validate_instance(
            &json!("foundation::ResourceId"),
            &public_path,
        )
        .unwrap();
        for invalid in
            ["::Resource", "resource::", "resource::::Id", "resource::9Id"]
        {
            assert!(
                super::validate_instance(&json!(invalid), &public_path)
                    .is_err()
            );
        }
    }

    #[test]
    fn canonicalization_sorts_only_schema_sets() {
        let schema = json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["set", "argv"],
            "properties": {
                "set": {
                    "type": "array",
                    "uniqueItems": true,
                    "items": {"type": "string"}
                },
                "argv": {"type": "array", "items": {"type": "string"}}
            }
        });
        let mut value = json!({"set": ["z", "a"], "argv": ["z", "a"]});
        super::canonicalize_unique_arrays(&mut value, &schema).unwrap();
        assert_eq!(value, json!({"set": ["a", "z"], "argv": ["z", "a"]}));
    }
}
