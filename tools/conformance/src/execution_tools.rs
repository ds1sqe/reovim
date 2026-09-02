//! Exact semantic reviews and environment bindings for independently
//! executed tools.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use serde_json::Value;

use crate::{
    json::{self, ConfigError},
    policy::relative_path,
};

pub const SCHEMA: &str = "reovim-execution-tool-reviews/v1";

/// Caller-owned limits applied in addition to the bounded JSON loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    pub path_bytes: usize,
    pub registry_items: usize,
    pub binding_items: usize,
    pub runtime_inputs: usize,
    pub argv_items: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    pub reviewed_at: String,
    pub reviews: Vec<Review>,
    pub environments: Vec<Environment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_field_names)]
pub struct Review {
    pub id: String,
    pub logical_role: LogicalRole,
    pub invocation_families: Vec<InvocationFamily>,
    pub required_semantics: Vec<String>,
    pub consumers: Vec<String>,
    pub product_profiles: Vec<String>,
    pub upstream: String,
    pub license: String,
    pub assumptions: Vec<String>,
    pub invalidators: Vec<String>,
    pub review_record: PathBuf,
    pub review_record_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogicalRole {
    Git,
    LinkerDriver,
    ToolchainResolver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InvocationFamily {
    GitVersion,
    GitRevision,
    GitTree,
    GitIndex,
    GitAttributes,
    LinkerDriver,
    ToolchainWhich,
}

impl InvocationFamily {
    #[must_use]
    pub const fn logical_role(self) -> LogicalRole {
        match self {
            Self::GitVersion
            | Self::GitRevision
            | Self::GitTree
            | Self::GitIndex
            | Self::GitAttributes => LogicalRole::Git,
            Self::LinkerDriver => LogicalRole::LinkerDriver,
            Self::ToolchainWhich => LogicalRole::ToolchainResolver,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    pub id: String,
    pub bindings: Vec<Binding>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Binding {
    pub review: String,
    pub canonical_path: PathBuf,
    pub prepared_path: Option<PathBuf>,
    pub executable_digest: String,
    pub executable_size_bytes: u64,
    pub version_argv: Vec<String>,
    pub version: String,
    pub version_digest: String,
    pub runtime_inputs: Vec<RuntimeInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeInput {
    pub canonical_path: PathBuf,
    pub content_digest: String,
    pub size_bytes: u64,
}

impl Registry {
    /// Parses and semantically closes a registry value already bounded and
    /// duplicate-key checked by the caller's JSON loader.
    pub fn parse(
        value: &Value,
        limits: ParseLimits,
    ) -> Result<Self, ConfigError> {
        validate_limits(limits)?;
        let object = json::object(
            value,
            "$",
            &["schema", "reviewed_at", "reviews", "environments"],
            &[],
        )?;
        exact(&object["schema"], "$.schema", SCHEMA)?;
        let reviewed_at = text(&object["reviewed_at"], "$.reviewed_at")?;
        if !crate::schema::utc_timestamp(&reviewed_at) {
            return Err(ConfigError::new(
                "EXECUTION_TOOL_TIMESTAMP",
                "$.reviewed_at",
                "expected a valid YYYY-MM-DDTHH:MM:SSZ UTC timestamp",
            ));
        }

        let review_values = bounded_array(
            &object["reviews"],
            "$.reviews",
            limits.registry_items,
            "EXECUTION_TOOL_REVIEW_LIMIT",
        )?;
        let (reviews, review_ids) = parse_reviews(review_values, limits)?;

        let environment_values = bounded_array(
            &object["environments"],
            "$.environments",
            limits.registry_items,
            "EXECUTION_TOOL_ENVIRONMENT_LIMIT",
        )?;
        let environments =
            parse_environments(environment_values, limits, &review_ids)?;
        Ok(Self { reviewed_at, reviews, environments })
    }
}

fn parse_reviews(
    values: &[Value],
    limits: ParseLimits,
) -> Result<(Vec<Review>, BTreeSet<String>), ConfigError> {
    let mut reviews = Vec::with_capacity(values.len());
    let mut review_ids = BTreeSet::new();
    let mut review_records = BTreeSet::new();
    let mut family_owners = BTreeMap::new();
    for (item, value) in values.iter().enumerate() {
        let path = json::index("$.reviews", item);
        let review = parse_review(value, &path, limits)?;
        register_review(
            &review,
            &path,
            &mut review_ids,
            &mut review_records,
            &mut family_owners,
        )?;
        reviews.push(review);
    }
    reviews.sort_by(|left, right| left.id.cmp(&right.id));
    Ok((reviews, review_ids))
}

fn register_review(
    review: &Review,
    path: &str,
    review_ids: &mut BTreeSet<String>,
    review_records: &mut BTreeSet<PathBuf>,
    family_owners: &mut BTreeMap<InvocationFamily, String>,
) -> Result<(), ConfigError> {
    if !review_ids.insert(review.id.clone()) {
        return Err(ConfigError::new(
            "EXECUTION_TOOL_DUPLICATE_REVIEW_ID",
            json::child(path, "id"),
            "duplicate execution-tool review ID",
        ));
    }
    if !review_records.insert(review.review_record.clone()) {
        return Err(ConfigError::new(
            "EXECUTION_TOOL_DUPLICATE_REVIEW_RECORD",
            json::child(path, "review_record"),
            "each semantic review requires its own retained record",
        ));
    }
    for family in &review.invocation_families {
        register_family_owner(*family, review, path, family_owners)?;
    }
    Ok(())
}

fn register_family_owner(
    family: InvocationFamily,
    review: &Review,
    path: &str,
    owners: &mut BTreeMap<InvocationFamily, String>,
) -> Result<(), ConfigError> {
    let Some(owner) = owners.insert(family, review.id.clone()) else {
        return Ok(());
    };
    Err(ConfigError::new(
        "EXECUTION_TOOL_DUPLICATE_FAMILY_REVIEW",
        json::child(path, "invocation_families"),
        format!("invocation family is already owned by review {owner}"),
    ))
}

fn parse_environments(
    values: &[Value],
    limits: ParseLimits,
    review_ids: &BTreeSet<String>,
) -> Result<Vec<Environment>, ConfigError> {
    let mut environments = Vec::with_capacity(values.len());
    let mut environment_ids = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let path = json::index("$.environments", item);
        let environment = parse_environment(value, &path, limits, review_ids)?;
        if !environment_ids.insert(environment.id.clone()) {
            return Err(ConfigError::new(
                "EXECUTION_TOOL_DUPLICATE_ENVIRONMENT_ID",
                json::child(&path, "id"),
                "duplicate execution environment ID",
            ));
        }
        environments.push(environment);
    }
    environments.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(environments)
}

fn parse_review(
    value: &Value,
    path: &str,
    limits: ParseLimits,
) -> Result<Review, ConfigError> {
    let object = json::object(
        value,
        path,
        &[
            "id",
            "logical_role",
            "invocation_families",
            "required_semantics",
            "consumers",
            "product_profiles",
            "upstream",
            "license",
            "assumptions",
            "invalidators",
            "disposition",
            "review_record",
            "review_record_digest",
        ],
        &[],
    )?;
    let role_path = json::child(path, "logical_role");
    let logical_role = match json::string(&object["logical_role"], &role_path)?
    {
        "git" => LogicalRole::Git,
        "linker_driver" => LogicalRole::LinkerDriver,
        "toolchain_resolver" => LogicalRole::ToolchainResolver,
        other => {
            return Err(ConfigError::new(
                "EXECUTION_TOOL_LOGICAL_ROLE",
                role_path,
                format!("unknown logical role {other:?}"),
            ));
        }
    };
    let invocation_families = parse_families(
        &object["invocation_families"],
        &json::child(path, "invocation_families"),
        limits,
    )?;
    if invocation_families
        .iter()
        .any(|family| family.logical_role() != logical_role)
    {
        return Err(ConfigError::new(
            "EXECUTION_TOOL_FAMILY_ROLE",
            json::child(path, "invocation_families"),
            "invocation family does not belong to the review's logical role",
        ));
    }
    exact(&object["disposition"], &json::child(path, "disposition"), "admit")?;
    Ok(Review {
        id: id(&object["id"], &json::child(path, "id"))?,
        logical_role,
        invocation_families,
        required_semantics: string_set(
            &object["required_semantics"],
            &json::child(path, "required_semantics"),
            limits.registry_items,
            false,
        )?,
        consumers: string_set(
            &object["consumers"],
            &json::child(path, "consumers"),
            limits.registry_items,
            false,
        )?,
        product_profiles: string_set(
            &object["product_profiles"],
            &json::child(path, "product_profiles"),
            limits.registry_items,
            true,
        )?,
        upstream: text(&object["upstream"], &json::child(path, "upstream"))?,
        license: text(&object["license"], &json::child(path, "license"))?,
        assumptions: string_set(
            &object["assumptions"],
            &json::child(path, "assumptions"),
            limits.registry_items,
            false,
        )?,
        invalidators: string_set(
            &object["invalidators"],
            &json::child(path, "invalidators"),
            limits.registry_items,
            false,
        )?,
        review_record: relative_path_value(
            &object["review_record"],
            &json::child(path, "review_record"),
            limits,
        )?,
        review_record_digest: digest(
            &object["review_record_digest"],
            &json::child(path, "review_record_digest"),
        )?,
    })
}

fn parse_families(
    value: &Value,
    path: &str,
    limits: ParseLimits,
) -> Result<Vec<InvocationFamily>, ConfigError> {
    let values = bounded_array(
        value,
        path,
        limits.registry_items,
        "EXECUTION_TOOL_FAMILY_LIMIT",
    )?;
    if values.is_empty() {
        return Err(ConfigError::new(
            "EXECUTION_TOOL_FAMILIES_EMPTY",
            path,
            "review requires at least one invocation family",
        ));
    }
    let mut result = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let item_path = json::index(path, item);
        let family = match json::string(value, &item_path)? {
            "git-version" => InvocationFamily::GitVersion,
            "git-revision" => InvocationFamily::GitRevision,
            "git-tree" => InvocationFamily::GitTree,
            "git-index" => InvocationFamily::GitIndex,
            "git-attributes" => InvocationFamily::GitAttributes,
            "linker-driver" => InvocationFamily::LinkerDriver,
            "toolchain-which" => InvocationFamily::ToolchainWhich,
            other => {
                return Err(ConfigError::new(
                    "EXECUTION_TOOL_INVOCATION_FAMILY",
                    item_path,
                    format!("unknown invocation family {other:?}"),
                ));
            }
        };
        if !result.insert(family) {
            return Err(ConfigError::new(
                "EXECUTION_TOOL_DUPLICATE_FAMILY",
                item_path,
                "duplicate invocation family",
            ));
        }
    }
    Ok(result.into_iter().collect())
}

fn parse_environment(
    value: &Value,
    path: &str,
    limits: ParseLimits,
    reviews: &BTreeSet<String>,
) -> Result<Environment, ConfigError> {
    let object = json::object(
        value,
        path,
        &["id", "host_os", "host_arch", "bindings"],
        &[],
    )?;
    exact(&object["host_os"], &json::child(path, "host_os"), "linux")?;
    exact(&object["host_arch"], &json::child(path, "host_arch"), "x86_64")?;
    let values = bounded_array(
        &object["bindings"],
        &json::child(path, "bindings"),
        limits.binding_items,
        "EXECUTION_TOOL_BINDING_LIMIT",
    )?;
    let mut bindings = Vec::with_capacity(values.len());
    let mut review_bindings = BTreeSet::new();
    let mut canonical_paths = BTreeSet::new();
    let mut prepared_paths = BTreeSet::new();
    for (item, value) in values.iter().enumerate() {
        let item_path = json::index(&json::child(path, "bindings"), item);
        let binding = parse_binding(value, &item_path, limits)?;
        if !reviews.contains(&binding.review) {
            return Err(ConfigError::new(
                "EXECUTION_TOOL_BINDING_REVIEW_UNRESOLVED",
                json::child(&item_path, "review"),
                format!("binding names absent review {}", binding.review),
            ));
        }
        if !review_bindings.insert(binding.review.clone()) {
            return Err(ConfigError::new(
                "EXECUTION_TOOL_DUPLICATE_REVIEW_BINDING",
                json::child(&item_path, "review"),
                "environment has more than one binding for one review",
            ));
        }
        if !canonical_paths.insert(binding.canonical_path.clone()) {
            return Err(ConfigError::new(
                "EXECUTION_TOOL_DUPLICATE_CANONICAL_PATH",
                json::child(&item_path, "canonical_path"),
                concat!(
                    "environment binds one canonical executable ",
                    "path more than once"
                ),
            ));
        }
        if let Some(prepared_path) = &binding.prepared_path
            && !prepared_paths.insert(prepared_path.clone())
        {
            return Err(ConfigError::new(
                "EXECUTION_TOOL_DUPLICATE_PREPARED_PATH",
                json::child(&item_path, "prepared_path"),
                "environment binds one prepared executable path more than once",
            ));
        }
        bindings.push(binding);
    }
    bindings.sort();
    Ok(Environment {
        id: id(&object["id"], &json::child(path, "id"))?,
        bindings,
    })
}

fn parse_binding(
    value: &Value,
    path: &str,
    limits: ParseLimits,
) -> Result<Binding, ConfigError> {
    let object = json::object(
        value,
        path,
        &[
            "review",
            "canonical_path",
            "prepared_path",
            "executable_digest",
            "executable_size_bytes",
            "version_argv",
            "version",
            "version_digest",
            "runtime_inputs",
        ],
        &[],
    )?;
    let canonical_path = absolute_path(
        &object["canonical_path"],
        &json::child(path, "canonical_path"),
        limits,
    )?;
    let prepared_path = if object["prepared_path"].is_null() {
        None
    } else {
        Some(absolute_path(
            &object["prepared_path"],
            &json::child(path, "prepared_path"),
            limits,
        )?)
    };
    let executable_size_bytes = positive(
        &object["executable_size_bytes"],
        &json::child(path, "executable_size_bytes"),
    )?;
    let argv_values = bounded_array(
        &object["version_argv"],
        &json::child(path, "version_argv"),
        limits.argv_items,
        "EXECUTION_TOOL_ARGV_LIMIT",
    )?;
    if argv_values.is_empty() {
        return Err(ConfigError::new(
            "EXECUTION_TOOL_ARGV_EMPTY",
            json::child(path, "version_argv"),
            "version invocation requires at least one argument",
        ));
    }
    let version_argv = argv_values
        .iter()
        .enumerate()
        .map(|(item, value)| {
            text(value, &json::index(&json::child(path, "version_argv"), item))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let input_values = bounded_array(
        &object["runtime_inputs"],
        &json::child(path, "runtime_inputs"),
        limits.runtime_inputs,
        "EXECUTION_TOOL_RUNTIME_INPUT_LIMIT",
    )?;
    let mut runtime_inputs = Vec::with_capacity(input_values.len());
    let mut input_paths = BTreeSet::new();
    for (item, value) in input_values.iter().enumerate() {
        let item_path = json::index(&json::child(path, "runtime_inputs"), item);
        let input = parse_runtime_input(value, &item_path, limits)?;
        if input.canonical_path == canonical_path {
            return Err(ConfigError::new(
                "EXECUTION_TOOL_RUNTIME_INPUT_EXECUTABLE",
                json::child(&item_path, "canonical_path"),
                "executable cannot also be declared as its own runtime input",
            ));
        }
        if !input_paths.insert(input.canonical_path.clone()) {
            return Err(ConfigError::new(
                "EXECUTION_TOOL_DUPLICATE_RUNTIME_INPUT",
                json::child(&item_path, "canonical_path"),
                "runtime-input path is declared more than once",
            ));
        }
        runtime_inputs.push(input);
    }
    runtime_inputs.sort();
    Ok(Binding {
        review: id(&object["review"], &json::child(path, "review"))?,
        canonical_path,
        prepared_path,
        executable_digest: digest(
            &object["executable_digest"],
            &json::child(path, "executable_digest"),
        )?,
        executable_size_bytes,
        version_argv,
        version: text(&object["version"], &json::child(path, "version"))?,
        version_digest: digest(
            &object["version_digest"],
            &json::child(path, "version_digest"),
        )?,
        runtime_inputs,
    })
}

fn parse_runtime_input(
    value: &Value,
    path: &str,
    limits: ParseLimits,
) -> Result<RuntimeInput, ConfigError> {
    let object = json::object(
        value,
        path,
        &["canonical_path", "content_digest", "size_bytes"],
        &[],
    )?;
    Ok(RuntimeInput {
        canonical_path: absolute_path(
            &object["canonical_path"],
            &json::child(path, "canonical_path"),
            limits,
        )?,
        content_digest: digest(
            &object["content_digest"],
            &json::child(path, "content_digest"),
        )?,
        size_bytes: positive(
            &object["size_bytes"],
            &json::child(path, "size_bytes"),
        )?,
    })
}

fn absolute_path(
    value: &Value,
    path: &str,
    limits: ParseLimits,
) -> Result<PathBuf, ConfigError> {
    let value = json::string(value, path)?;
    if value.len() > limits.path_bytes {
        return Err(ConfigError::new(
            "EXECUTION_TOOL_PATH_LIMIT",
            path,
            format!(
                "path is {} bytes; limit is {}",
                value.len(),
                limits.path_bytes
            ),
        ));
    }
    if value == "/"
        || !value.starts_with('/')
        || value.ends_with('/')
        || value.contains("//")
        || value.contains('\0')
        || value
            .split('/')
            .skip(1)
            .any(|component| matches!(component, "." | ".."))
    {
        return Err(ConfigError::new(
            "EXECUTION_TOOL_ABSOLUTE_PATH",
            path,
            concat!(
                "expected a normalized non-root absolute path ",
                "without dot components"
            ),
        ));
    }
    Ok(PathBuf::from(value))
}

fn relative_path_value(
    value: &Value,
    path: &str,
    limits: ParseLimits,
) -> Result<PathBuf, ConfigError> {
    let value = json::string(value, path)?;
    if value.len() > limits.path_bytes {
        return Err(ConfigError::new(
            "EXECUTION_TOOL_PATH_LIMIT",
            path,
            format!(
                "path is {} bytes; limit is {}",
                value.len(),
                limits.path_bytes
            ),
        ));
    }
    relative_path(value, path, false)
}

fn positive(value: &Value, path: &str) -> Result<u64, ConfigError> {
    let value = json::unsigned(value, path)?;
    if value == 0 {
        Err(ConfigError::new(
            "EXECUTION_TOOL_ZERO_SIZE",
            path,
            "byte size must be positive",
        ))
    } else {
        Ok(value)
    }
}

fn digest(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = json::string(value, path)?;
    if value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(value.to_owned())
    } else {
        Err(ConfigError::new(
            "EXECUTION_TOOL_DIGEST",
            path,
            "expected sha256:<64 lowercase hex>",
        ))
    }
}

fn id(value: &Value, path: &str) -> Result<String, ConfigError> {
    let value = json::string(value, path)?;
    if valid_id(value) {
        Ok(value.to_owned())
    } else {
        Err(ConfigError::new(
            "EXECUTION_TOOL_ID",
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

fn string_set(
    value: &Value,
    path: &str,
    limit: usize,
    allow_empty: bool,
) -> Result<Vec<String>, ConfigError> {
    let values =
        bounded_array(value, path, limit, "EXECUTION_TOOL_STRING_SET_LIMIT")?;
    if values.is_empty() && !allow_empty {
        return Err(ConfigError::new(
            "EXECUTION_TOOL_STRING_SET_EMPTY",
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
                "EXECUTION_TOOL_STRING_SET_DUPLICATE",
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
        Err(ConfigError::new(
            "EXECUTION_TOOL_TEXT_EMPTY",
            path,
            "text must be nonempty",
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
        || limits.registry_items == 0
        || limits.binding_items == 0
        || limits.runtime_inputs == 0
        || limits.argv_items == 0
    {
        Err(ConfigError::new(
            "EXECUTION_TOOL_PARSE_ZERO_LIMIT",
            "$",
            "all execution-tool parser limits must be positive",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    const LIMITS: super::ParseLimits = super::ParseLimits {
        path_bytes: 4096,
        registry_items: 64,
        binding_items: 32,
        runtime_inputs: 64,
        argv_items: 32,
    };

    fn empty_registry() -> Value {
        json!({
            "schema": super::SCHEMA,
            "reviewed_at": "2026-09-01T00:00:00Z",
            "reviews": [],
            "environments": []
        })
    }

    fn review_record(id: &str) -> String {
        format!("conformance/execution-tool-reviews/{id}.md")
    }

    fn review(id: &str, role: &str, family: &str) -> Value {
        json!({
            "id": id,
            "logical_role": role,
            "invocation_families": [family],
            "required_semantics": ["exact stable output"],
            "consumers": ["exact-checker"],
            "product_profiles": ["authored-std-lib/v1"],
            "upstream": "https://example.invalid/upstream",
            "license": "GPL-2.0-only",
            "assumptions": ["reviewed invocation grammar"],
            "invalidators": ["executable or runtime-input drift"],
            "disposition": "admit",
            "review_record": review_record(id),
            "review_record_digest": format!("sha256:{}", "1".repeat(64))
        })
    }

    fn binding(review: &str, executable: &str, prepared: &str) -> Value {
        json!({
            "review": review,
            "canonical_path": executable,
            "prepared_path": prepared,
            "executable_digest": format!("sha256:{}", "2".repeat(64)),
            "executable_size_bytes": 1024,
            "version_argv": ["--version"],
            "version": "tool 1.0\n",
            "version_digest": format!("sha256:{}", "3".repeat(64)),
            "runtime_inputs": [{
                "canonical_path": "/lib64/ld-linux-x86-64.so.2",
                "content_digest": format!("sha256:{}", "4".repeat(64)),
                "size_bytes": 2048
            }]
        })
    }

    fn complete_registry() -> Value {
        let mut value = empty_registry();
        value["reviews"] = json!([review("git-exact", "git", "git-version")]);
        value["environments"] = json!([{
            "id": "linux-x86-64",
            "host_os": "linux",
            "host_arch": "x86_64",
            "bindings": [binding("git-exact", "/usr/bin/git", "/tools/git")]
        }]);
        value
    }

    #[test]
    fn active_empty_registry_is_valid() {
        let registry =
            super::Registry::parse(&empty_registry(), LIMITS).unwrap();
        assert_eq!(registry.reviews.len(), 0);
        assert_eq!(registry.environments.len(), 0);
    }

    #[test]
    fn complete_review_and_environment_binding_parse() {
        let registry =
            super::Registry::parse(&complete_registry(), LIMITS).unwrap();
        assert_eq!(registry.reviews[0].logical_role, super::LogicalRole::Git);
        assert_eq!(registry.environments[0].bindings[0].review, "git-exact");
        assert_eq!(
            registry.environments[0].bindings[0].runtime_inputs.len(),
            1
        );
    }

    #[test]
    fn schema_timestamp_unknown_fields_and_limits_fail_closed() {
        let mut schema = empty_registry();
        schema["schema"] = json!("reovim-execution-tool-reviews/v2");
        assert_eq!(
            super::Registry::parse(&schema, LIMITS).unwrap_err().code,
            "CFG_CONST"
        );

        let mut timestamp = empty_registry();
        timestamp["reviewed_at"] = json!("2026-02-29T25:00:00Z");
        assert_eq!(
            super::Registry::parse(&timestamp, LIMITS).unwrap_err().code,
            "EXECUTION_TOOL_TIMESTAMP"
        );

        let mut unknown = complete_registry();
        unknown["reviews"][0]["surprise"] = json!(true);
        assert_eq!(
            super::Registry::parse(&unknown, LIMITS).unwrap_err().code,
            "CFG_UNKNOWN_FIELD"
        );

        let limits = super::ParseLimits { binding_items: 0, ..LIMITS };
        assert_eq!(
            super::Registry::parse(&empty_registry(), limits).unwrap_err().code,
            "EXECUTION_TOOL_PARSE_ZERO_LIMIT"
        );
    }

    #[test]
    fn ids_relative_and_absolute_paths_and_digests_are_closed() {
        let mut id = complete_registry();
        id["reviews"][0]["id"] = json!("Git_Exact");
        assert_eq!(
            super::Registry::parse(&id, LIMITS).unwrap_err().code,
            "EXECUTION_TOOL_ID"
        );

        let mut relative = complete_registry();
        relative["reviews"][0]["review_record"] = json!("../review.md");
        assert_eq!(
            super::Registry::parse(&relative, LIMITS).unwrap_err().code,
            "CFG_PATH_ESCAPE"
        );

        let mut absolute = complete_registry();
        absolute["environments"][0]["bindings"][0]["canonical_path"] =
            json!("/usr/../bin/git");
        assert_eq!(
            super::Registry::parse(&absolute, LIMITS).unwrap_err().code,
            "EXECUTION_TOOL_ABSOLUTE_PATH"
        );

        let mut digest = complete_registry();
        digest["reviews"][0]["review_record_digest"] = json!("sha256:ABC");
        assert_eq!(
            super::Registry::parse(&digest, LIMITS).unwrap_err().code,
            "EXECUTION_TOOL_DIGEST"
        );
    }

    #[test]
    fn review_sets_roles_and_global_family_ownership_are_exact() {
        let mut duplicate_set = complete_registry();
        duplicate_set["reviews"][0]["assumptions"] = json!(["same", "same"]);
        assert_eq!(
            super::Registry::parse(&duplicate_set, LIMITS).unwrap_err().code,
            "EXECUTION_TOOL_STRING_SET_DUPLICATE"
        );

        let mut wrong_role = complete_registry();
        wrong_role["reviews"][0]["logical_role"] = json!("linker_driver");
        assert_eq!(
            super::Registry::parse(&wrong_role, LIMITS).unwrap_err().code,
            "EXECUTION_TOOL_FAMILY_ROLE"
        );

        let mut duplicate_family = complete_registry();
        duplicate_family["reviews"].as_array_mut().unwrap().push(review(
            "git-second",
            "git",
            "git-version",
        ));
        assert_eq!(
            super::Registry::parse(&duplicate_family, LIMITS).unwrap_err().code,
            "EXECUTION_TOOL_DUPLICATE_FAMILY_REVIEW"
        );
    }

    #[test]
    fn binding_cross_references_and_environment_identities_are_exact() {
        let mut missing = complete_registry();
        missing["environments"][0]["bindings"][0]["review"] = json!("absent");
        assert_eq!(
            super::Registry::parse(&missing, LIMITS).unwrap_err().code,
            "EXECUTION_TOOL_BINDING_REVIEW_UNRESOLVED"
        );

        let mut duplicate_review = complete_registry();
        let duplicate =
            duplicate_review["environments"][0]["bindings"][0].clone();
        duplicate_review["environments"][0]["bindings"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert_eq!(
            super::Registry::parse(&duplicate_review, LIMITS).unwrap_err().code,
            "EXECUTION_TOOL_DUPLICATE_REVIEW_BINDING"
        );

        let mut duplicate_environment = complete_registry();
        let duplicate = duplicate_environment["environments"][0].clone();
        duplicate_environment["environments"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert_eq!(
            super::Registry::parse(&duplicate_environment, LIMITS)
                .unwrap_err()
                .code,
            "EXECUTION_TOOL_DUPLICATE_ENVIRONMENT_ID"
        );
    }

    #[test]
    fn runtime_inputs_sizes_and_argv_are_bounded_and_unique() {
        let mut duplicate = complete_registry();
        let input =
            duplicate["environments"][0]["bindings"][0]["runtime_inputs"][0]
                .clone();
        duplicate["environments"][0]["bindings"][0]["runtime_inputs"]
            .as_array_mut()
            .unwrap()
            .push(input);
        assert_eq!(
            super::Registry::parse(&duplicate, LIMITS).unwrap_err().code,
            "EXECUTION_TOOL_DUPLICATE_RUNTIME_INPUT"
        );

        let mut zero = complete_registry();
        zero["environments"][0]["bindings"][0]["executable_size_bytes"] =
            json!(0);
        assert_eq!(
            super::Registry::parse(&zero, LIMITS).unwrap_err().code,
            "EXECUTION_TOOL_ZERO_SIZE"
        );

        let mut argv = complete_registry();
        argv["environments"][0]["bindings"][0]["version_argv"] = json!([]);
        assert_eq!(
            super::Registry::parse(&argv, LIMITS).unwrap_err().code,
            "EXECUTION_TOOL_ARGV_EMPTY"
        );

        let limits = super::ParseLimits { runtime_inputs: 0, ..LIMITS };
        assert_eq!(
            super::Registry::parse(&empty_registry(), limits).unwrap_err().code,
            "EXECUTION_TOOL_PARSE_ZERO_LIMIT"
        );
    }
}
