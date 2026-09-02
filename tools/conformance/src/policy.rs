//! Parser for the committed source-classification policy.

use std::{collections::BTreeSet, path::PathBuf};

use reovim_depgraph::{
    ArchitecturalRole, SourceClassification, SourceClassificationMap,
    SourceClassificationRule, SourceOwner, SourceProvenance, SourceUse,
};
use serde_json::Value;

use crate::json::{self, ConfigError};

pub const SCHEMA: &str = "reovim-source-classification/v2";
pub const EVIDENCE_KEY: &str = "conformance/source-role-map.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    pub scope_roots: Vec<PathBuf>,
    pub exclusions: Vec<Exclusion>,
    pub rules: Vec<Rule>,
    pub allowed_product_edges: BTreeSet<(ArchitecturalRole, ArchitecturalRole)>,
    pub generated_evidence_key: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exclusion {
    pub prefix: PathBuf,
    pub kind: ExclusionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExclusionKind {
    VcsMetadata,
    TransientBuildOutput,
    WorkingNotes,
    GeneratedEvidenceOutput,
}

impl ExclusionKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VcsMetadata => "vcs_metadata",
            Self::TransientBuildOutput => "transient_build_output",
            Self::WorkingNotes => "working_notes",
            Self::GeneratedEvidenceOutput => "generated_evidence_output",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub prefix: PathBuf,
    pub precedence: u32,
    pub classification: SourceClassification,
    pub upstream: Option<Upstream>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upstream {
    pub identity: String,
    pub digest: String,
    pub digest_subject: DigestSubject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DigestSubject {
    Paths(Vec<PathBuf>),
    ClassifiedContent,
}

impl Policy {
    pub fn parse(value: &Value) -> Result<Self, ConfigError> {
        let root = json::object(
            value,
            "$",
            &[
                "schema",
                "scope",
                "rules",
                "unmatched",
                "allowed_product_edges",
                "generated_evidence_key",
            ],
            &[],
        )?;
        exact_string(&root["schema"], "$.schema", SCHEMA)?;
        exact_string(&root["unmatched"], "$.unmatched", "deny")?;
        exact_string(
            &root["generated_evidence_key"],
            "$.generated_evidence_key",
            EVIDENCE_KEY,
        )?;

        let scope = json::object(
            &root["scope"],
            "$.scope",
            &["roots", "exclusions"],
            &[],
        )?;
        let scope_roots =
            parse_unique_paths(&scope["roots"], "$.scope.roots", true)?;
        if scope_roots != [PathBuf::new()] {
            return Err(ConfigError::new(
                "POLICY_SCOPE_ROOT",
                "$.scope.roots",
                "v2 requires exactly the repository root '.'",
            ));
        }
        let exclusions = parse_exclusions(&scope["exclusions"])?;
        let actual_exclusions = exclusions
            .iter()
            .map(|exclusion| (exclusion.prefix.clone(), exclusion.kind))
            .collect::<BTreeSet<_>>();
        let required_exclusions = BTreeSet::from([
            (PathBuf::from(".git"), ExclusionKind::VcsMetadata),
            (PathBuf::from("target"), ExclusionKind::TransientBuildOutput),
            (PathBuf::from("tmp"), ExclusionKind::WorkingNotes),
        ]);
        if actual_exclusions != required_exclusions {
            return Err(ConfigError::new(
                "POLICY_EXCLUSION_SET",
                "$.scope.exclusions",
                concat!(
                    "v2 requires exactly .git/vcs_metadata, ",
                    "target/transient_build_output, and tmp/working_notes",
                ),
            ));
        }
        let rules = parse_rules(&root["rules"])?;
        if rules.is_empty() {
            return Err(ConfigError::new(
                "POLICY_EMPTY_RULES",
                "$.rules",
                "at least one rule is required",
            ));
        }
        let allowed_product_edges =
            parse_edges(&root["allowed_product_edges"])?;
        let generated_evidence_key =
            relative_path(EVIDENCE_KEY, "$.generated_evidence_key", false)?;

        let policy = Self {
            scope_roots,
            exclusions,
            rules,
            allowed_product_edges,
            generated_evidence_key,
        };
        policy.validate_result_separation()?;
        // Exercise the shared primitive here so invalid normalized prefixes
        // cannot be accepted by one layer and rejected by another.
        policy.classification_map()?;
        Ok(policy)
    }

    pub fn classification_map(
        &self,
    ) -> Result<SourceClassificationMap, ConfigError> {
        SourceClassificationMap::new(
            2,
            self.rules.iter().map(|rule| {
                SourceClassificationRule::new(
                    rule.prefix.clone(),
                    rule.precedence,
                    rule.classification,
                )
            }),
        )
        .map_err(|error| {
            ConfigError::new("POLICY_RULE_PATH", "$.rules", error.to_string())
        })
    }

    #[must_use]
    pub fn upstream_for(&self, path: &std::path::Path) -> Option<&Upstream> {
        self.winning_rule(path).and_then(|rule| rule.upstream.as_ref())
    }

    #[must_use]
    pub fn winning_rule(&self, path: &std::path::Path) -> Option<&Rule> {
        self.rules
            .iter()
            .filter(|rule| path.starts_with(&rule.prefix))
            .max_by_key(|rule| rule.precedence)
    }

    fn validate_result_separation(&self) -> Result<(), ConfigError> {
        if self
            .rules
            .iter()
            .any(|rule| self.generated_evidence_key.starts_with(&rule.prefix))
        {
            return Err(ConfigError::new(
                "POLICY_RESULT_SELF_REFERENCE",
                "$.rules",
                format!(
                    "generated evidence key {} is classified by a policy rule",
                    self.generated_evidence_key.display()
                ),
            ));
        }
        Ok(())
    }
}

fn parse_exclusions(value: &Value) -> Result<Vec<Exclusion>, ConfigError> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for (item, value) in
        json::array(value, "$.scope.exclusions")?.iter().enumerate()
    {
        let path = json::index("$.scope.exclusions", item);
        let object = json::object(value, &path, &["prefix", "kind"], &[])?;
        let prefix = relative_path(
            json::string(&object["prefix"], &json::child(&path, "prefix"))?,
            &json::child(&path, "prefix"),
            false,
        )?;
        let kind_path = json::child(&path, "kind");
        let kind = match json::string(&object["kind"], &kind_path)? {
            "vcs_metadata" => ExclusionKind::VcsMetadata,
            "transient_build_output" => ExclusionKind::TransientBuildOutput,
            "working_notes" => ExclusionKind::WorkingNotes,
            "generated_evidence_output" => {
                ExclusionKind::GeneratedEvidenceOutput
            }
            other => {
                return Err(ConfigError::new(
                    "POLICY_EXCLUSION_KIND",
                    kind_path,
                    format!("unknown exclusion kind {other:?}"),
                ));
            }
        };
        if !seen.insert((prefix.clone(), kind)) {
            return Err(ConfigError::new(
                "POLICY_DUPLICATE_EXCLUSION",
                path,
                "duplicate exclusion",
            ));
        }
        result.push(Exclusion { prefix, kind });
    }
    result.sort_by(|left, right| {
        (&left.prefix, left.kind).cmp(&(&right.prefix, right.kind))
    });
    Ok(result)
}

#[allow(clippy::too_many_lines)]
fn parse_rules(value: &Value) -> Result<Vec<Rule>, ConfigError> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for (item, value) in json::array(value, "$.rules")?.iter().enumerate() {
        let path = json::index("$.rules", item);
        let object = json::object(
            value,
            &path,
            &[
                "prefix",
                "precedence",
                "source_owner",
                "source_provenance",
                "source_use",
            ],
            &["upstream"],
        )?;
        let prefix_path = json::child(&path, "prefix");
        let prefix = relative_path(
            json::string(&object["prefix"], &prefix_path)?,
            &prefix_path,
            false,
        )?;
        let precedence_path = json::child(&path, "precedence");
        let precedence = u32::try_from(json::unsigned(
            &object["precedence"],
            &precedence_path,
        )?)
        .map_err(|_| {
            ConfigError::new(
                "POLICY_PRECEDENCE",
                precedence_path,
                "value exceeds u32",
            )
        })?;
        let owner = parse_owner(
            &object["source_owner"],
            &json::child(&path, "source_owner"),
        )?;
        let provenance_path = json::child(&path, "source_provenance");
        let provenance =
            match json::string(&object["source_provenance"], &provenance_path)?
            {
                "authored" => SourceProvenance::Authored,
                "generated" => SourceProvenance::Generated,
                "vendored" => SourceProvenance::Vendored,
                other => {
                    return Err(ConfigError::new(
                        "POLICY_PROVENANCE",
                        provenance_path,
                        format!("unknown provenance {other:?}"),
                    ));
                }
            };
        let use_path = json::child(&path, "source_use");
        let source_use = match json::string(&object["source_use"], &use_path)? {
            "production" => SourceUse::Production,
            "test" => SourceUse::Test,
            "example" => SourceUse::Example,
            "tool" => SourceUse::Tool,
            "build_output" => SourceUse::BuildOutput,
            other => {
                return Err(ConfigError::new(
                    "POLICY_SOURCE_USE",
                    use_path,
                    format!("unknown source use {other:?}"),
                ));
            }
        };
        let classification = SourceClassification::new(
            owner, provenance, source_use,
        )
        .map_err(|error| {
            ConfigError::new(
                "POLICY_CLOSED_COMBINATION",
                &path,
                error.to_string(),
            )
        })?;
        let upstream = object
            .get("upstream")
            .map(|value| parse_upstream(value, &json::child(&path, "upstream")))
            .transpose()?;
        match (provenance, upstream.is_some()) {
            (SourceProvenance::Authored, true) => {
                return Err(ConfigError::new(
                    "POLICY_AUTHORED_UPSTREAM",
                    &path,
                    "authored source must not declare upstream",
                ));
            }
            (
                SourceProvenance::Generated | SourceProvenance::Vendored,
                false,
            ) => {
                return Err(ConfigError::new(
                    "POLICY_MISSING_UPSTREAM",
                    &path,
                    concat!(
                        "generated and vendored source require upstream ",
                        "identity and digest",
                    ),
                ));
            }
            _ => {}
        }
        validate_upstream_digest_subject(provenance, upstream.as_ref(), &path)?;
        if !seen.insert((prefix.clone(), precedence)) {
            return Err(ConfigError::new(
                "POLICY_DUPLICATE_RULE",
                path,
                "duplicate prefix and precedence",
            ));
        }
        result.push(Rule { prefix, precedence, classification, upstream });
    }
    Ok(result)
}

fn validate_upstream_digest_subject(
    provenance: SourceProvenance,
    upstream: Option<&Upstream>,
    path: &str,
) -> Result<(), ConfigError> {
    let Some(upstream) = upstream else {
        return Ok(());
    };
    match (provenance, &upstream.digest_subject) {
        (SourceProvenance::Generated, DigestSubject::Paths(_))
        | (SourceProvenance::Vendored, DigestSubject::ClassifiedContent) => {
            Ok(())
        }
        (SourceProvenance::Generated, DigestSubject::ClassifiedContent) => {
            Err(ConfigError::new(
                "POLICY_GENERATED_DIGEST_SUBJECT",
                path,
                "generated source requires a paths digest subject",
            ))
        }
        (SourceProvenance::Vendored, DigestSubject::Paths(_)) => {
            Err(ConfigError::new(
                "POLICY_VENDORED_DIGEST_SUBJECT",
                path,
                "vendored source requires a classified_content digest subject",
            ))
        }
        (SourceProvenance::Authored, _) => {
            unreachable!("authored upstream rejected above")
        }
    }
}

fn parse_owner(value: &Value, path: &str) -> Result<SourceOwner, ConfigError> {
    let object = value
        .as_object()
        .ok_or_else(|| ConfigError::new("CFG_TYPE", path, "expected object"))?;
    let kind = object.get("kind").ok_or_else(|| {
        ConfigError::new(
            "CFG_MISSING_FIELD",
            json::child(path, "kind"),
            "required field is absent",
        )
    })?;
    match json::string(kind, &json::child(path, "kind"))? {
        "repository_tooling" => {
            json::object(value, path, &["kind"], &[])?;
            Ok(SourceOwner::RepositoryTooling)
        }
        "product" => {
            let object = json::object(value, path, &["kind", "role"], &[])?;
            let role_path = json::child(path, "role");
            Ok(SourceOwner::Product(parse_role(
                json::string(&object["role"], &role_path)?,
                &role_path,
            )?))
        }
        other => Err(ConfigError::new(
            "POLICY_OWNER_KIND",
            json::child(path, "kind"),
            format!("unknown source owner kind {other:?}"),
        )),
    }
}

fn parse_role(
    value: &str,
    path: &str,
) -> Result<ArchitecturalRole, ConfigError> {
    ArchitecturalRole::ALL
        .into_iter()
        .find(|role| role.as_str() == value)
        .ok_or_else(|| {
            ConfigError::new(
                "POLICY_PRODUCT_ROLE",
                path,
                format!("unknown product role {value:?}"),
            )
        })
}

fn parse_upstream(value: &Value, path: &str) -> Result<Upstream, ConfigError> {
    let object = json::object(
        value,
        path,
        &["identity", "digest", "digest_subject"],
        &[],
    )?;
    let identity =
        json::string(&object["identity"], &json::child(path, "identity"))?;
    if identity.is_empty() {
        return Err(ConfigError::new(
            "POLICY_UPSTREAM_IDENTITY",
            json::child(path, "identity"),
            "identity must not be empty",
        ));
    }
    let digest = json::string(&object["digest"], &json::child(path, "digest"))?;
    if !valid_digest(digest) {
        return Err(ConfigError::new(
            "POLICY_UPSTREAM_DIGEST",
            json::child(path, "digest"),
            "expected sha256 plus 64 lowercase hex digits",
        ));
    }
    let subject_path = json::child(path, "digest_subject");
    let subject = object.get("digest_subject").expect("required field checked");
    let kind = subject
        .as_object()
        .and_then(|subject| subject.get("kind"))
        .ok_or_else(|| {
            ConfigError::new(
                "CFG_MISSING_FIELD",
                json::child(&subject_path, "kind"),
                "required field is absent",
            )
        })?;
    let digest_subject =
        match json::string(kind, &json::child(&subject_path, "kind"))? {
            "paths" => {
                let subject = json::object(
                    subject,
                    &subject_path,
                    &["kind", "paths"],
                    &[],
                )?;
                let paths = parse_unique_paths(
                    &subject["paths"],
                    &json::child(&subject_path, "paths"),
                    false,
                )?;
                if paths.is_empty() {
                    return Err(ConfigError::new(
                        "POLICY_EMPTY_DIGEST_SUBJECT",
                        subject_path,
                        "paths digest subject must not be empty",
                    ));
                }
                DigestSubject::Paths(paths)
            }
            "classified_content" => {
                json::object(subject, &subject_path, &["kind"], &[])?;
                DigestSubject::ClassifiedContent
            }
            other => {
                return Err(ConfigError::new(
                    "POLICY_DIGEST_SUBJECT",
                    subject_path,
                    format!("unknown digest subject {other:?}"),
                ));
            }
        };
    Ok(Upstream {
        identity: identity.to_owned(),
        digest: digest.to_owned(),
        digest_subject,
    })
}

fn parse_edges(
    value: &Value,
) -> Result<BTreeSet<(ArchitecturalRole, ArchitecturalRole)>, ConfigError> {
    let mut result = BTreeSet::new();
    for (item, value) in
        json::array(value, "$.allowed_product_edges")?.iter().enumerate()
    {
        let path = json::index("$.allowed_product_edges", item);
        let object =
            json::object(value, &path, &["consumer", "provider"], &[])?;
        let consumer = parse_role(
            json::string(&object["consumer"], &json::child(&path, "consumer"))?,
            &json::child(&path, "consumer"),
        )?;
        let provider = parse_role(
            json::string(&object["provider"], &json::child(&path, "provider"))?,
            &json::child(&path, "provider"),
        )?;
        if !result.insert((consumer, provider)) {
            return Err(ConfigError::new(
                "POLICY_DUPLICATE_EDGE",
                path,
                "duplicate allowed product edge",
            ));
        }
    }
    Ok(result)
}

fn parse_unique_paths(
    value: &Value,
    path: &str,
    allow_empty: bool,
) -> Result<Vec<PathBuf>, ConfigError> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for (item, value) in json::array(value, path)?.iter().enumerate() {
        let item_path = json::index(path, item);
        let value = relative_path(
            json::string(value, &item_path)?,
            &item_path,
            allow_empty,
        )?;
        if !seen.insert(value.clone()) {
            return Err(ConfigError::new(
                "CFG_DUPLICATE_PATH",
                item_path,
                "duplicate normalized path",
            ));
        }
        result.push(value);
    }
    Ok(result)
}

pub fn relative_path(
    value: &str,
    path: &str,
    allow_repository_root: bool,
) -> Result<PathBuf, ConfigError> {
    if value.contains('\\') || value.contains("//") || value.contains('\0') {
        return Err(ConfigError::new(
            "CFG_PATH_FORM",
            path,
            "path must use normalized forward-slash separators",
        ));
    }
    if value.is_empty() || (value != "." && value.ends_with('/')) {
        return Err(ConfigError::new(
            "CFG_PATH_FORM",
            path,
            "path is empty or has a trailing separator",
        ));
    }
    if value == "." && !allow_repository_root {
        return Err(ConfigError::new(
            "CFG_PATH_FORM",
            path,
            "repository root '.' is not admitted in this path field",
        ));
    }
    if value.len() >= 2 && value.as_bytes()[1] == b':' {
        return Err(ConfigError::new(
            "CFG_PATH_FORM",
            path,
            "drive-prefixed paths are forbidden",
        ));
    }
    let raw = PathBuf::from(value);
    if raw.is_absolute() {
        return Err(ConfigError::new(
            "CFG_PATH_FORM",
            path,
            "path must be relative",
        ));
    }
    let mut normalized = PathBuf::new();
    for component in raw.components() {
        match component {
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::CurDir if value == "." => {}
            std::path::Component::CurDir => {
                return Err(ConfigError::new(
                    "CFG_PATH_FORM",
                    path,
                    "embedded '.' is not normalized",
                ));
            }
            std::path::Component::ParentDir => {
                return Err(ConfigError::new(
                    "CFG_PATH_ESCAPE",
                    path,
                    "path contains '..'",
                ));
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(ConfigError::new(
                    "CFG_PATH_FORM",
                    path,
                    "path must be relative",
                ));
            }
        }
    }
    if normalized.as_os_str().is_empty() && !allow_repository_root {
        return Err(ConfigError::new(
            "CFG_PATH_FORM",
            path,
            "path normalizes to repository root",
        ));
    }
    Ok(normalized)
}

fn exact_string(
    value: &Value,
    path: &str,
    expected: &str,
) -> Result<(), ConfigError> {
    let actual = json::string(value, path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(ConfigError::new(
            "CFG_CONST",
            path,
            format!("expected {expected:?}, got {actual:?}"),
        ))
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    #[allow(clippy::needless_pass_by_value)]
    fn policy(rule: Value) -> Value {
        json!({
            "schema": super::SCHEMA,
            "scope": {
                "roots": ["."],
                "exclusions": [
                    {"prefix": ".git", "kind": "vcs_metadata"},
                    {"prefix": "target", "kind": "transient_build_output"},
                    {"prefix": "tmp", "kind": "working_notes"}
                ]
            },
            "rules": [rule],
            "unmatched": "deny",
            "allowed_product_edges": [],
            "generated_evidence_key": super::EVIDENCE_KEY
        })
    }

    fn authored_tool(prefix: &str) -> Value {
        json!({
            "prefix": prefix,
            "precedence": 1,
            "source_owner": {"kind": "repository_tooling"},
            "source_provenance": "authored",
            "source_use": "tool"
        })
    }

    #[test]
    fn scope_and_exclusions_are_exact_closed_sets() {
        let mut narrow = policy(authored_tool("src"));
        narrow.pointer_mut("/scope/roots").unwrap().clone_from(&json!(["src"]));
        assert_eq!(
            super::Policy::parse(&narrow).unwrap_err().code,
            "POLICY_SCOPE_ROOT"
        );

        let mut hidden = policy(authored_tool("src"));
        hidden
            .pointer_mut("/scope/exclusions")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .push(json!({"prefix":"kernel","kind":"working_notes"}));
        assert_eq!(
            super::Policy::parse(&hidden).unwrap_err().code,
            "POLICY_EXCLUSION_SET"
        );
    }

    #[test]
    fn owner_provenance_use_axes_and_upstream_are_fail_closed() {
        let mut invalid_use = authored_tool("src");
        invalid_use["source_use"] = json!("production");
        assert_eq!(
            super::Policy::parse(&policy(invalid_use)).unwrap_err().code,
            "POLICY_CLOSED_COMBINATION"
        );

        let mut generated = authored_tool("generated");
        generated["source_provenance"] = json!("generated");
        assert_eq!(
            super::Policy::parse(&policy(generated)).unwrap_err().code,
            "POLICY_MISSING_UPSTREAM"
        );

        let mut build = authored_tool("build");
        build["source_use"] = json!("build_output");
        assert_eq!(
            super::Policy::parse(&policy(build)).unwrap_err().code,
            "POLICY_CLOSED_COMBINATION"
        );
    }

    #[test]
    fn result_self_reference_and_root_rule_are_rejected() {
        assert_eq!(
            super::Policy::parse(&policy(authored_tool("conformance")))
                .unwrap_err()
                .code,
            "POLICY_RESULT_SELF_REFERENCE"
        );
        assert_eq!(
            super::Policy::parse(&policy(authored_tool("."))).unwrap_err().code,
            "CFG_PATH_FORM"
        );
    }

    #[test]
    fn product_edges_are_parsed_for_closure_validation() {
        let mut value = policy(authored_tool("src"));
        value["allowed_product_edges"] =
            json!([{"consumer":"Kernel Interface","provider":"Foundation"}]);
        let parsed = super::Policy::parse(&value).unwrap();
        assert_eq!(parsed.allowed_product_edges.len(), 1);
    }
}
