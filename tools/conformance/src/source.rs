//! Root-contained source inventory, classification, and topology discovery.

use std::{
    collections::BTreeSet,
    fs, io,
    io::Read as _,
    path::{Path, PathBuf},
};

use reovim_depgraph::{
    SourceClassification, SourceClassificationMap, SourceOwner,
    SourceProvenance, SourceUse,
};

use crate::{
    digest::{self, Sha256},
    finding::{Finding, Findings},
    policy::{DigestSubject, Exclusion, Policy, Upstream},
    topology::{Limits, Topology},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedFile {
    pub path: PathBuf,
    pub content_digest: String,
    pub classification: SourceClassification,
    pub upstream: Option<Upstream>,
    pub rule_prefix: PathBuf,
    pub rule_precedence: u32,
}

#[derive(Debug)]
pub struct Inventory {
    pub files: Vec<ClassifiedFile>,
    pub exclusions: Vec<Exclusion>,
    pub tree_digest: String,
    pub findings: Vec<Finding>,
}

/// One canonical semantic entry in the source-tree digest preimage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceTreeDigestEntry<'a> {
    pub path: &'a Path,
    pub content_digest: &'a str,
    pub owner: SourceOwner,
    pub provenance: SourceProvenance,
    pub source_use: SourceUse,
}

pub fn inventory(
    root: &Path,
    policy: &Policy,
    topology: &Topology,
) -> Inventory {
    let mut findings = Findings::new(topology.limits.findings);
    validate_retired_roots(root, &mut findings);
    let classification_map = match policy.classification_map() {
        Ok(map) => map,
        Err(error) => {
            findings.push(Finding::new(
                "policy",
                error.code,
                Some(error.path),
                error.message,
            ));
            return empty_inventory(policy, findings);
        }
    };
    let paths =
        collect_source_paths(root, policy, topology.limits, &mut findings);
    let mut files = classify_source_paths(
        root,
        policy,
        &classification_map,
        paths,
        topology.limits,
        &mut findings,
    );
    files.sort_by(|left, right| left.path.cmp(&right.path));
    validate_generated_mechanisms(&files, &topology.lockfiles, &mut findings);
    validate_manifests(root, &files, topology.limits, &mut findings);
    validate_product_boundary(&files, topology, &mut findings);
    validate_classification_rules(policy, &files, &mut findings);
    validate_required_checker_inputs(&files, &mut findings);
    validate_entrypoint_declarations(&files, topology, &mut findings);
    validate_entrypoint_sets(&files, topology, &mut findings);
    validate_entrypoint_lexical_warnings(root, &files, topology, &mut findings);
    validate_topology(root, &files, topology, &mut findings);
    validate_upstream_digests(
        root,
        policy,
        &files,
        topology.limits,
        &mut findings,
    );
    let tree_digest = canonical_source_tree_digest(
        files.iter().map(SourceTreeDigestEntry::from),
    );
    Inventory {
        files,
        exclusions: policy.exclusions.clone(),
        tree_digest,
        findings: findings.finish(),
    }
}

fn validate_retired_roots(root: &Path, findings: &mut Findings) {
    for denied in [
        ".cargo",
        "apps",
        "arch",
        "client",
        "editor",
        "kabi",
        "platform",
        "system",
        "uapi",
        "lib/ds",
        "lib/testrt",
    ] {
        match fs::symlink_metadata(root.join(denied)) {
            Ok(_) => findings.push(Finding::new(
                "retired_surface",
                "RETIRED_ROOT_PRESENT",
                Some(denied.to_owned()),
                "retired OS/product root is forbidden by the v1 denylist",
            )),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => findings.push(Finding::new(
                "retired_surface",
                "RETIRED_ROOT_PROBE_IO",
                Some(denied.to_owned()),
                error.to_string(),
            )),
        }
    }
}

fn empty_inventory(policy: &Policy, findings: Findings) -> Inventory {
    Inventory {
        files: Vec::new(),
        exclusions: policy.exclusions.clone(),
        tree_digest: canonical_source_tree_digest(std::iter::empty()),
        findings: findings.finish(),
    }
}

fn collect_source_paths(
    root: &Path,
    policy: &Policy,
    limits: Limits,
    findings: &mut Findings,
) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::new();
    let mut budget = WalkBudget::default();
    for scope_root in &policy.scope_roots {
        let absolute = root.join(scope_root);
        walk(
            root,
            &absolute,
            policy,
            limits,
            &mut budget,
            &mut paths,
            findings,
        );
    }
    paths
}

fn classify_source_paths(
    root: &Path,
    policy: &Policy,
    classification_map: &SourceClassificationMap,
    paths: BTreeSet<PathBuf>,
    limits: Limits,
    findings: &mut Findings,
) -> Vec<ClassifiedFile> {
    let mut files = Vec::new();
    let mut total = 0_u64;
    for (file_index, path) in paths.into_iter().enumerate() {
        if file_index >= limits.source_file_count {
            findings.push(Finding::new(
                "limits",
                "SOURCE_FILE_COUNT_LIMIT",
                None,
                format!(
                    "source file count exceeds {}",
                    limits.source_file_count
                ),
            ));
            break;
        }
        match classify_source_path(
            root,
            policy,
            classification_map,
            &path,
            limits,
            &mut total,
            findings,
        ) {
            ClassifyPath::File(file) => files.push(file),
            ClassifyPath::Skip => {}
            ClassifyPath::Stop => break,
        }
    }
    files
}

enum ClassifyPath {
    File(ClassifiedFile),
    Skip,
    Stop,
}

fn classify_source_path(
    root: &Path,
    policy: &Policy,
    classification_map: &SourceClassificationMap,
    path: &Path,
    limits: Limits,
    total: &mut u64,
    findings: &mut Findings,
) -> ClassifyPath {
    let path_text = display_relative(path);
    if path_text.len() > limits.path_bytes {
        findings.push(Finding::new(
            "path",
            "PATH_BYTE_LIMIT",
            Some(path_text),
            format!("path exceeds {} bytes", limits.path_bytes),
        ));
        return ClassifyPath::Skip;
    }
    let absolute = root.join(path);
    let metadata = match fs::symlink_metadata(&absolute) {
        Ok(metadata) => metadata,
        Err(error) => {
            findings.push(io_finding("SOURCE_METADATA", path, &error));
            return ClassifyPath::Skip;
        }
    };
    if !metadata.file_type().is_file() {
        findings.push(Finding::new(
            "path",
            "SOURCE_NOT_REGULAR",
            Some(path_text),
            "classified path is not a regular file",
        ));
        return ClassifyPath::Skip;
    }
    if metadata.len() > limits.source_file_bytes {
        findings.push(Finding::new(
            "limits",
            "SOURCE_FILE_BYTE_LIMIT",
            Some(path_text),
            format!(
                "{} bytes exceeds per-file limit {}",
                metadata.len(),
                limits.source_file_bytes
            ),
        ));
        return ClassifyPath::Skip;
    }
    let Some(next_total) = total
        .checked_add(metadata.len())
        .filter(|value| *value <= limits.source_total_bytes)
    else {
        findings.push(Finding::new(
            "limits",
            "SOURCE_TOTAL_BYTE_LIMIT",
            Some(path_text),
            format!("source bytes exceed {}", limits.source_total_bytes),
        ));
        return ClassifyPath::Stop;
    };
    *total = next_total;
    let classification = match classification_map.classify(path) {
        Ok(classification) => classification,
        Err(error) => {
            findings.push(Finding::new(
                "classification",
                "SOURCE_CLASSIFICATION",
                Some(path_text),
                error.to_string(),
            ));
            return ClassifyPath::Skip;
        }
    };
    let content_digest =
        match digest::file_bounded(&absolute, limits.source_file_bytes) {
            Ok(digest) => digest,
            Err(error) => {
                findings.push(io_finding("SOURCE_DIGEST", path, &error));
                return ClassifyPath::Skip;
            }
        };
    let winning_rule = policy.winning_rule(path);
    ClassifyPath::File(ClassifiedFile {
        path: path.to_path_buf(),
        content_digest,
        classification,
        upstream: policy.upstream_for(path).cloned(),
        rule_prefix: winning_rule
            .map(|rule| rule.prefix.clone())
            .unwrap_or_default(),
        rule_precedence: winning_rule.map_or(0, |rule| rule.precedence),
    })
}

fn validate_generated_mechanisms(
    files: &[ClassifiedFile],
    lockfiles: &[PathBuf],
    findings: &mut Findings,
) {
    for file in files
        .iter()
        .filter(|file| !generated_mechanism_supported(file, lockfiles))
    {
        findings.push(Finding::new(
            "classification",
            "GENERATED_MECHANISM_PROOF_UNAVAILABLE",
            Some(display_relative(&file.path)),
            "generated source lacks an admitted exact mechanism",
        ));
    }
}

fn validate_manifests(
    root: &Path,
    files: &[ClassifiedFile],
    limits: Limits,
    findings: &mut Findings,
) {
    for manifest in files.iter().filter(|file| {
        file.path.file_name().is_some_and(|name| name == "Cargo.toml")
    }) {
        match read_bounded(&root.join(&manifest.path), limits.source_file_bytes)
        {
            Ok(bytes) if digest::bytes(&bytes) != manifest.content_digest => {
                findings.push(Finding::new(
                    "manifest",
                    "MANIFEST_INVENTORY_DRIFT",
                    Some(display_relative(&manifest.path)),
                    "manifest bytes differ from the inventoried content digest",
                ));
            }
            Ok(bytes) => validate_manifest_bytes(manifest, bytes, findings),
            Err(error) => {
                findings.push(Finding::new(
                    "manifest",
                    "MANIFEST_BOUNDED_READ",
                    Some(display_relative(&manifest.path)),
                    error,
                ));
            }
        }
    }
}

fn validate_manifest_bytes(
    manifest: &ClassifiedFile,
    bytes: Vec<u8>,
    findings: &mut Findings,
) {
    match String::from_utf8(bytes) {
        Ok(contents) if manifest_assigns_panic(&contents) => {
            findings.push(Finding::new(
                "manifest",
                "PANIC_ABORT_FORBIDDEN",
                Some(display_relative(&manifest.path)),
                "first-party Cargo manifests must not assign a panic strategy",
            ));
        }
        Ok(_) => {}
        Err(error) => findings.push(Finding::new(
            "manifest",
            "MANIFEST_UTF8",
            Some(display_relative(&manifest.path)),
            error.to_string(),
        )),
    }
}

fn validate_product_boundary(
    files: &[ClassifiedFile],
    topology: &Topology,
    findings: &mut Findings,
) {
    if topology.workspaces.values().any(|workspace| {
        workspace.partition == crate::topology::WorkspacePartition::Product
    }) {
        return;
    }
    if let Some(file) = files.iter().find(|file| {
        matches!(file.classification.owner, SourceOwner::Product(_))
    }) {
        findings.push(Finding::new(
            "product_boundary",
            "PRODUCT_BOUNDARY_PROOF_UNAVAILABLE",
            Some(display_relative(&file.path)),
            concat!(
                "v1 syntax and Cargo evidence cannot prove Product ",
                "source/public/packaging boundaries"
            ),
        ));
    }
}

fn validate_classification_rules(
    policy: &Policy,
    files: &[ClassifiedFile],
    findings: &mut Findings,
) {
    for rule in &policy.rules {
        if !files.iter().any(|file| {
            file.rule_prefix == rule.prefix
                && file.rule_precedence == rule.precedence
        }) {
            findings.push(Finding::new(
                "classification",
                "DEAD_CLASSIFICATION_RULE",
                Some(display_relative(&rule.prefix)),
                format!(
                    "precedence {} wins for no inventoried file",
                    rule.precedence
                ),
            ));
        }
    }
}

fn validate_required_checker_inputs(
    files: &[ClassifiedFile],
    findings: &mut Findings,
) {
    for required in [
        "conformance/source-classification.json",
        "conformance/source-classification.schema.json",
        "conformance/workspace-topology.json",
        "conformance/workspace-topology.schema.json",
        "conformance/entrypoint-reviews.json",
        "conformance/entrypoint-reviews.schema.json",
        "conformance/dependency-reviews.json",
        "conformance/dependency-reviews.schema.json",
        "conformance/source-role-map.schema.json",
        "tools/conformance/Cargo.toml",
        "tools/conformance/src/lib.rs",
        "tools/conformance/src/main.rs",
    ] {
        if !files.iter().any(|file| file.path == Path::new(required)) {
            findings.push(Finding::new(
                "classification",
                "REQUIRED_CHECKER_INPUT_UNCLASSIFIED",
                Some(required.to_owned()),
                concat!(
                    "required policy/topology/review/checker input is ",
                    "absent or unclassified"
                ),
            ));
        }
    }
}

fn validate_entrypoint_declarations(
    files: &[ClassifiedFile],
    topology: &Topology,
    findings: &mut Findings,
) {
    for entrypoint in &topology.repository_entrypoints {
        let Some((path, content_digest)) = entrypoint_path_digest(entrypoint)
        else {
            continue;
        };
        validate_declared_entrypoint(files, path, content_digest, findings);
    }
}

fn entrypoint_path_digest(
    entrypoint: &crate::topology::RepositoryEntrypoint,
) -> Option<(&Path, &str)> {
    match entrypoint {
        crate::topology::RepositoryEntrypoint::Script {
            path,
            content_digest,
        }
        | crate::topology::RepositoryEntrypoint::Workflow {
            path,
            content_digest,
        } => Some((path, content_digest)),
        crate::topology::RepositoryEntrypoint::CargoTarget { .. } => None,
    }
}

fn validate_declared_entrypoint(
    files: &[ClassifiedFile],
    path: &Path,
    content_digest: &str,
    findings: &mut Findings,
) {
    match files.iter().find(|file| file.path == path) {
        Some(file) if entrypoint_matches(file, content_digest) => {}
        Some(file) if file.content_digest != content_digest => {
            findings.push(Finding::new(
                "topology",
                "ENTRYPOINT_CONTENT_DIGEST",
                Some(display_relative(path)),
                format!(
                    concat!(
                        "entrypoint content digest {} differs from ",
                        "admitted {content_digest}"
                    ),
                    file.content_digest,
                    content_digest = content_digest
                ),
            ));
        }
        Some(_) => findings.push(Finding::new(
            "topology",
            "ENTRYPOINT_CLASSIFICATION",
            Some(display_relative(path)),
            "repository script entrypoint must be RepositoryTooling + Tool",
        )),
        None => findings.push(Finding::new(
            "topology",
            "ENTRYPOINT_UNCLASSIFIED",
            Some(display_relative(path)),
            "repository script entrypoint is absent or unclassified",
        )),
    }
}

fn entrypoint_matches(file: &ClassifiedFile, content_digest: &str) -> bool {
    file.classification.owner == SourceOwner::RepositoryTooling
        && file.classification.source_use == SourceUse::Tool
        && file.content_digest == content_digest
}

fn validate_entrypoint_sets(
    files: &[ClassifiedFile],
    topology: &Topology,
    findings: &mut Findings,
) {
    let declared_scripts = topology
        .repository_entrypoints
        .iter()
        .filter_map(|entrypoint| match entrypoint {
            crate::topology::RepositoryEntrypoint::Script { path, .. } => {
                Some(path.clone())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let discovered_scripts = files
        .iter()
        .filter(|file| file.path.starts_with("scripts"))
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    compare_path_set(
        "TOPOLOGY_SCRIPT_ENTRYPOINT",
        &declared_scripts,
        &discovered_scripts,
        findings,
    );
    let declared_workflows = topology
        .repository_entrypoints
        .iter()
        .filter_map(|entrypoint| match entrypoint {
            crate::topology::RepositoryEntrypoint::Workflow {
                path, ..
            } => Some(path.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let discovered_workflows = files
        .iter()
        .filter(|file| {
            file.path.starts_with(".github/workflows")
                && file.path.extension().is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("yml")
                        || extension.eq_ignore_ascii_case("yaml")
                })
        })
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    compare_path_set(
        "TOPOLOGY_WORKFLOW_ENTRYPOINT",
        &declared_workflows,
        &discovered_workflows,
        findings,
    );
}

fn validate_entrypoint_lexical_warnings(
    root: &Path,
    files: &[ClassifiedFile],
    topology: &Topology,
    findings: &mut Findings,
) {
    for entrypoint in &topology.repository_entrypoints {
        let Some((path, _)) = entrypoint_path_digest(entrypoint) else {
            continue;
        };
        let Some(caller) = files.iter().find(|file| file.path == *path) else {
            continue;
        };
        validate_entrypoint_lexical_warning(
            root,
            files,
            path,
            caller,
            topology.limits,
            findings,
        );
    }
}

fn validate_entrypoint_lexical_warning(
    root: &Path,
    files: &[ClassifiedFile],
    path: &Path,
    caller: &ClassifiedFile,
    limits: Limits,
    findings: &mut Findings,
) {
    let bytes = match read_bounded(&root.join(path), limits.source_file_bytes) {
        Ok(bytes) if digest::bytes(&bytes) == caller.content_digest => bytes,
        Ok(_) => {
            findings.push(Finding::new(
                "entrypoint",
                "ENTRYPOINT_INPUT_INVENTORY_DRIFT",
                Some(display_relative(path)),
                "entrypoint bytes differ from inventory",
            ));
            return;
        }
        Err(error) => {
            findings.push(Finding::new(
                "entrypoint",
                "ENTRYPOINT_INPUT_READ",
                Some(display_relative(path)),
                error,
            ));
            return;
        }
    };
    let Ok(contents) = std::str::from_utf8(&bytes) else {
        findings.push(Finding::new(
            "entrypoint",
            "ENTRYPOINT_INPUT_UTF8",
            Some(display_relative(path)),
            "v1 script/workflow admission requires UTF-8",
        ));
        return;
    };
    let (inputs, dynamic) = obvious_local_inputs(contents);
    if dynamic {
        findings.push(Finding::new(
            "entrypoint",
            "ENTRYPOINT_LOCAL_INPUT_PROOF_UNAVAILABLE",
            Some(display_relative(path)),
            concat!(
                "defense-in-depth lexical warning detected a dynamic or ",
                "unsupported local invocation forbidden by the exact review ",
                "admission"
            ),
        ));
    }
    for input in inputs {
        validate_local_input(files, path, &input, findings);
    }
}

fn validate_local_input(
    files: &[ClassifiedFile],
    caller_path: &Path,
    input: &Path,
    findings: &mut Findings,
) {
    match files.iter().find(|file| file.path == input) {
        Some(target) if entrypoint_local_input_matches(target) => {}
        Some(_) => findings.push(Finding::new(
            "entrypoint",
            "ENTRYPOINT_LOCAL_INPUT_CLASSIFICATION",
            Some(display_relative(input)),
            "script/workflow local input must be RepositoryTooling + Tool",
        )),
        None => findings.push(Finding::new(
            "entrypoint",
            "ENTRYPOINT_LOCAL_INPUT_UNCLASSIFIED",
            Some(display_relative(input)),
            format!(
                "{} consumes an omitted local input",
                display_relative(caller_path)
            ),
        )),
    }
}

fn entrypoint_local_input_matches(file: &ClassifiedFile) -> bool {
    file.classification.owner == SourceOwner::RepositoryTooling
        && file.classification.source_use == SourceUse::Tool
}

pub fn obvious_local_inputs(contents: &str) -> (BTreeSet<PathBuf>, bool) {
    let mut scan = LocalInputScan::default();
    for line in contents.lines() {
        scan.line(line);
    }
    scan.finish()
}

#[derive(Default)]
struct LocalInputScan {
    inputs: BTreeSet<PathBuf>,
    dynamic: bool,
    single_quoted_block: bool,
    heredoc: Option<(String, bool)>,
    array_assignment: bool,
    logical_command: String,
}

impl LocalInputScan {
    fn line(&mut self, line: &str) {
        let Some(heredoc_body) = self.heredoc_body(line) else {
            return;
        };
        let Some(line) = self.single_quoted_line(line, heredoc_body) else {
            return;
        };
        if !heredoc_body {
            self.start_heredoc(line);
        }
        let starts_array =
            !self.array_assignment && shell_array_assignment_starts(line);
        self.scan_command(line, heredoc_body, starts_array);
        scan_lexical_local_inputs(line, &mut self.inputs, &mut self.dynamic);
        self.update_array_assignment(line, starts_array);
    }

    fn heredoc_body(&mut self, line: &str) -> Option<bool> {
        if self
            .heredoc
            .as_ref()
            .is_some_and(|(delimiter, _)| line.trim() == delimiter)
        {
            self.heredoc = None;
            return None;
        }
        match self.heredoc.as_ref() {
            Some((_, false)) => None,
            Some((_, true)) => Some(true),
            None => Some(false),
        }
    }

    fn single_quoted_line<'a>(
        &mut self,
        line: &'a str,
        heredoc_body: bool,
    ) -> Option<&'a str> {
        if !heredoc_body && self.single_quoted_block {
            self.single_quoted_block = !line.trim_start().starts_with('\'');
            return None;
        }
        if !single_quoted_block_starts(line, heredoc_body) {
            return Some(line);
        }
        self.single_quoted_block = true;
        Some(&line[..line.rfind('\'').expect("odd quote count has a quote")])
    }

    fn start_heredoc(&mut self, line: &str) {
        if let Some(delimiter) = shell_heredoc_delimiter(line) {
            self.heredoc = Some(delimiter);
        }
    }

    fn scan_command(
        &mut self,
        line: &str,
        heredoc_body: bool,
        starts_array: bool,
    ) {
        if heredoc_body || self.array_assignment || starts_array {
            self.dynamic |= command_substitution_has_variable_command(line)
                || has_unquoted_backtick_substitution(line);
            return;
        }
        if !self.logical_command.is_empty() {
            self.logical_command.push(' ');
        }
        self.logical_command.push_str(line.trim_end_matches([' ', '\t', '\\']));
        if line.trim_end().ends_with('\\') {
            return;
        }
        self.dynamic |= variable_expanded_command_path(&self.logical_command);
        self.logical_command.clear();
    }

    fn update_array_assignment(&mut self, line: &str, starts_array: bool) {
        self.array_assignment |= starts_array;
        if self.array_assignment && line.trim() == ")" {
            self.array_assignment = false;
        }
    }

    fn finish(mut self) -> (BTreeSet<PathBuf>, bool) {
        self.dynamic |= !self.logical_command.is_empty();
        (self.inputs, self.dynamic)
    }
}

fn single_quoted_block_starts(line: &str, heredoc_body: bool) -> bool {
    !heredoc_body
        && line.trim_end().ends_with('\'')
        && line.chars().filter(|character| *character == '\'').count() % 2 == 1
        && !line.trim_start().starts_with('#')
}

fn scan_lexical_local_inputs(
    line: &str,
    inputs: &mut BTreeSet<PathBuf>,
    dynamic: &mut bool,
) {
    let words = line
        .split(|character: char| {
            character.is_whitespace() || ";|&".contains(character)
        })
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    for pair in words.windows(2) {
        scan_command_pair(pair, inputs, dynamic);
    }
    scan_dot_paths(line, inputs, dynamic);
}

fn scan_command_pair(
    pair: &[&str],
    inputs: &mut BTreeSet<PathBuf>,
    dynamic: &mut bool,
) {
    let raw = pair[1].trim_matches(['\'', '"']);
    if matches!(pair[0], "source" | ".") {
        record_local_path(raw, inputs, dynamic);
        return;
    }
    if !matches!(
        pair[0],
        "bash" | "sh" | "python" | "python3" | "node" | "exec"
    ) {
        return;
    }
    if raw.contains(['$', '`']) {
        *dynamic = true;
        return;
    }
    if local_script_path(raw) {
        record_local_path(raw, inputs, dynamic);
    }
}

fn record_local_path(
    raw: &str,
    inputs: &mut BTreeSet<PathBuf>,
    dynamic: &mut bool,
) {
    if raw.contains(['$', '`']) {
        *dynamic = true;
        return;
    }
    match crate::policy::relative_path(raw, "entrypoint", false) {
        Ok(path) => {
            inputs.insert(path);
        }
        Err(_) => *dynamic = true,
    }
}

fn local_script_path(raw: &str) -> bool {
    raw.contains('/')
        || Path::new(raw)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(extension, "sh" | "py" | "js" | "ts")
            })
}

fn scan_dot_paths(
    line: &str,
    inputs: &mut BTreeSet<PathBuf>,
    dynamic: &mut bool,
) {
    let mut remaining = line;
    while let Some(start) = remaining.find("./") {
        let suffix = &remaining[start + 2..];
        let end = suffix
            .find(|character: char| {
                !(character.is_ascii_alphanumeric()
                    || "._-/".contains(character))
            })
            .unwrap_or(suffix.len());
        let candidate = &suffix[..end];
        record_local_path(candidate, inputs, dynamic);
        remaining = &suffix[end..];
    }
}

fn shell_heredoc_delimiter(line: &str) -> Option<(String, bool)> {
    let start = line.find("<<")?;
    let mut suffix = line[start + 2..].trim_start();
    suffix = suffix.strip_prefix('-').unwrap_or(suffix).trim_start();
    let quote = suffix
        .chars()
        .next()
        .filter(|character| matches!(character, '\'' | '"'));
    if quote.is_some() {
        suffix = &suffix[1..];
    }
    let end = suffix
        .find(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '_')
        })
        .unwrap_or(suffix.len());
    let delimiter = &suffix[..end];
    (!delimiter.is_empty()).then(|| (delimiter.to_owned(), quote.is_none()))
}

fn shell_array_assignment_starts(line: &str) -> bool {
    let word = line.split_whitespace().next().unwrap_or_default();
    word.split_once('=').is_some_and(|(name, value)| {
        shell_assignment(&format!("{name}="))
            && value == "("
            && !line.trim_end().ends_with(')')
    })
}

fn variable_expanded_command_path(line: &str) -> bool {
    if reviewed_held_cargo_probe(line)
        || reviewed_held_cargo_invocation(line)
        || reviewed_held_checker_invocation(line)
    {
        return false;
    }
    if shell_segments(line)
        .iter()
        .any(|segment| invokes_entrypoint_authority_fd(segment))
    {
        return true;
    }
    if line.trim_start().starts_with('\'') {
        return false;
    }
    if command_substitution_has_variable_command(line)
        || has_unquoted_backtick_substitution(line)
    {
        return true;
    }
    let trimmed = line.trim_start();
    if (trimmed.starts_with("[[") || trimmed.starts_with("if [["))
        && trimmed.contains("]]")
        || trimmed.starts_with("((") && trimmed.contains("))")
    {
        return false;
    }
    shell_segments(line)
        .iter()
        .any(|segment| segment_has_variable_command(segment))
}

fn reviewed_held_cargo_probe(line: &str) -> bool {
    let words = shell_tokens(line);
    words.iter().map(String::as_str).eq([
        "/proc/$$/fd/8",
        "8<&-",
        "--version",
        ">/dev/null",
    ])
}

fn reviewed_held_cargo_invocation(line: &str) -> bool {
    let words = shell_tokens(line);
    words.iter().map(String::as_str).eq([
        "CARGO_HOME=\"$CARGO_HOME_PATH\"",
        "CARGO_TARGET_DIR=\"$ROOT/target\"",
        "RUSTC=\"$RUSTC_BIN\"",
        "/proc/$$/fd/8",
        "8<&-",
        "build",
        "--locked",
        "--offline",
        "--manifest-path",
        "\"$ROOT/Cargo.toml\"",
        "-p",
        "reovim-conformance",
        "--bin",
        "reovim-conformance",
        "--target",
        "\"$CHECKER_TARGET\"",
        "--release",
    ])
}

fn reviewed_held_checker_invocation(line: &str) -> bool {
    let words = shell_tokens(line);
    words.iter().map(String::as_str).eq([
        "CARGO_HOME=\"$CARGO_HOME_PATH\"",
        "CARGO_TARGET_DIR=\"$ROOT/target\"",
        "RUSTC=\"$RUSTC_BIN\"",
        "/proc/$$/fd/9",
        "9<&-",
        "\"$@\"",
    ])
}

fn invokes_entrypoint_authority_fd(segment: &str) -> bool {
    let words = shell_tokens(segment);
    let Some(cursor) = shell_command_start(&words) else {
        return false;
    };
    let cursor = skip_shell_wrappers(&words, cursor);
    words.get(cursor).map(|word| shell_word(word)).is_some_and(|word| {
        let held_fd = word.ends_with("/fd/8") || word.ends_with("/fd/9");
        held_fd && (word.starts_with("/proc/") || word.starts_with("/dev/fd/"))
    })
}

fn segment_has_variable_command(segment: &str) -> bool {
    let words = shell_tokens(segment);
    let Some(cursor) = shell_command_start(&words) else {
        return false;
    };
    if shell_query_command(&words, cursor) {
        return false;
    }
    let cursor = skip_shell_wrappers(&words, cursor);
    words
        .get(cursor)
        .is_some_and(|word| variable_command_word(shell_word(word)))
}

fn shell_command_start(words: &[String]) -> Option<usize> {
    let mut cursor = 0_usize;
    while let Some(word) = words.get(cursor).map(|word| shell_word(word)) {
        if shell_assignment(word) && word.contains("$(") {
            return None;
        }
        if !shell_prefix_word(word) {
            break;
        }
        cursor += 1;
    }
    Some(cursor)
}

fn shell_prefix_word(word: &str) -> bool {
    word.is_empty()
        || matches!(
            word,
            "-" | "!"
                | "{"
                | "}"
                | "("
                | ")"
                | "if"
                | "then"
                | "elif"
                | "else"
                | "while"
                | "until"
                | "do"
                | "done"
                | "run:"
        )
        || shell_assignment(word)
}

fn shell_query_command(words: &[String], cursor: usize) -> bool {
    words.get(cursor).is_some_and(|word| shell_word(word) == "command")
        && words
            .get(cursor + 1)
            .is_some_and(|word| matches!(shell_word(word), "-v" | "-V"))
}

fn skip_shell_wrappers(words: &[String], mut cursor: usize) -> usize {
    while words
        .get(cursor)
        .map(|word| shell_word(word))
        .is_some_and(shell_wrapper)
    {
        cursor += 1;
        cursor = skip_shell_wrapper_options(words, cursor);
    }
    cursor
}

fn skip_shell_wrapper_options(words: &[String], mut cursor: usize) -> usize {
    while words
        .get(cursor)
        .map(|word| shell_word(word))
        .is_some_and(|word| word.starts_with('-') || shell_assignment(word))
    {
        cursor += 1;
    }
    cursor
}

fn shell_wrapper(word: &str) -> bool {
    matches!(word, "command" | "exec" | "env" | "nohup" | "time")
}

fn command_substitution_has_variable_command(line: &str) -> bool {
    let mut substitutions = line;
    while let Some(start) = substitutions.find("$(") {
        substitutions = &substitutions[start + 2..];
        if variable_expanded_command_path(substitutions) {
            return true;
        }
    }
    false
}

fn has_unquoted_backtick_substitution(line: &str) -> bool {
    let mut single_quoted = false;
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            escaped = false;
        } else if character == '\\' && !single_quoted {
            escaped = true;
        } else if character == '\'' {
            single_quoted = !single_quoted;
        } else if character == '`' && !single_quoted {
            return true;
        }
    }
    false
}

fn shell_segments(line: &str) -> Vec<String> {
    split_shell_fields(line, ShellFieldKind::Segment)
}

fn shell_tokens(segment: &str) -> Vec<String> {
    split_shell_fields(segment, ShellFieldKind::Word)
}

#[derive(Clone, Copy)]
enum ShellFieldKind {
    Segment,
    Word,
}

fn split_shell_fields(input: &str, kind: ShellFieldKind) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in input.chars() {
        push_shell_character(
            &mut fields,
            &mut field,
            &mut quote,
            &mut escaped,
            kind,
            character,
        );
    }
    finish_shell_field(&mut fields, &mut field, kind);
    fields
}

fn push_shell_character(
    fields: &mut Vec<String>,
    field: &mut String,
    quote: &mut Option<char>,
    escaped: &mut bool,
    kind: ShellFieldKind,
    character: char,
) {
    if *escaped {
        field.push(character);
        *escaped = false;
        return;
    }
    if character == '\\' && *quote != Some('\'') {
        field.push(character);
        *escaped = true;
        return;
    }
    if let Some(active_quote) = *quote {
        field.push(character);
        if character == active_quote {
            *quote = None;
        }
        return;
    }
    if matches!(character, '\'' | '"') {
        field.push(character);
        *quote = Some(character);
        return;
    }
    if shell_field_separator(kind, character) {
        finish_shell_field(fields, field, kind);
        return;
    }
    field.push(character);
}

fn shell_field_separator(kind: ShellFieldKind, character: char) -> bool {
    match kind {
        ShellFieldKind::Segment => ";|&".contains(character),
        ShellFieldKind::Word => character.is_whitespace(),
    }
}

fn finish_shell_field(
    fields: &mut Vec<String>,
    field: &mut String,
    kind: ShellFieldKind,
) {
    let populated = match kind {
        ShellFieldKind::Segment => !field.trim().is_empty(),
        ShellFieldKind::Word => !field.is_empty(),
    };
    if populated {
        fields.push(std::mem::take(field));
    }
}

fn shell_word(word: &str) -> &str {
    word.trim_matches(|character| {
        matches!(character, '\'' | '"' | '\\' | '(' | ')')
    })
}

fn shell_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else { return false };
    let name = name.strip_suffix('+').unwrap_or(name);
    !name.is_empty()
        && name.bytes().enumerate().all(|(index, byte)| {
            byte == b'_'
                || if index == 0 {
                    byte.is_ascii_alphabetic()
                } else {
                    byte.is_ascii_alphanumeric()
                }
        })
}

fn variable_command_word(word: &str) -> bool {
    word.contains('$') || word.contains('`')
}

fn generated_mechanism_supported(
    file: &ClassifiedFile,
    lockfiles: &[PathBuf],
) -> bool {
    file.classification.provenance != SourceProvenance::Generated
        || lockfiles.iter().any(|path| path == &file.path)
}

fn manifest_assigns_panic(input: &str) -> bool {
    let bytes = input.as_bytes();
    let mut cursor = 0_usize;
    while cursor < bytes.len() {
        let assigns_panic = match bytes[cursor] {
            b'#' => {
                skip_toml_comment(bytes, &mut cursor);
                false
            }
            quote @ (b'\'' | b'"') => {
                quoted_key_assigns_panic(bytes, &mut cursor, quote)
            }
            byte if byte.is_ascii_alphabetic()
                || byte == b'_'
                || byte == b'-' =>
            {
                bare_key_assigns_panic(bytes, &mut cursor)
            }
            _ => {
                cursor += 1;
                false
            }
        };
        if assigns_panic {
            return true;
        }
    }
    false
}

const fn skip_toml_comment(bytes: &[u8], cursor: &mut usize) {
    *cursor += 1;
    while *cursor < bytes.len() && bytes[*cursor] != b'\n' {
        *cursor += 1;
    }
}

fn quoted_key_assigns_panic(
    bytes: &[u8],
    cursor: &mut usize,
    quote: u8,
) -> bool {
    let (value, next) = quoted_token(bytes, *cursor, quote);
    *cursor = next;
    if value != b"panic" && !value.contains(&b'\\') {
        return false;
    }
    skip_toml_space_and_comments(bytes, cursor);
    bytes.get(*cursor) == Some(&b'=')
}

fn bare_key_assigns_panic(bytes: &[u8], cursor: &mut usize) -> bool {
    let start = *cursor;
    *cursor += 1;
    while bytes.get(*cursor).copied().is_some_and(toml_bare_key_byte) {
        *cursor += 1;
    }
    if &bytes[start..*cursor] != b"panic" {
        return false;
    }
    skip_toml_space_and_comments(bytes, cursor);
    bytes.get(*cursor) == Some(&b'=')
}

const fn toml_bare_key_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn quoted_token(bytes: &[u8], start: usize, quote: u8) -> (&[u8], usize) {
    let triple = bytes.get(start..start + 3) == Some(&[quote, quote, quote]);
    let delimiter = if triple { 3 } else { 1 };
    let content_start = start + delimiter;
    let mut cursor = content_start;
    while cursor < bytes.len() {
        let closes = if triple {
            bytes.get(cursor) == Some(&quote)
                && bytes.get(cursor + 1) == Some(&quote)
                && bytes.get(cursor + 2) == Some(&quote)
        } else {
            bytes.get(cursor) == Some(&quote)
        };
        if closes {
            return (&bytes[content_start..cursor], cursor + delimiter);
        }
        if quote == b'"' && bytes[cursor] == b'\\' {
            cursor = cursor.saturating_add(2);
        } else {
            cursor += 1;
        }
    }
    (&bytes[content_start..], bytes.len())
}

fn skip_toml_space_and_comments(bytes: &[u8], cursor: &mut usize) {
    loop {
        while bytes.get(*cursor).is_some_and(u8::is_ascii_whitespace) {
            *cursor += 1;
        }
        if bytes.get(*cursor) != Some(&b'#') {
            return;
        }
        while *cursor < bytes.len() && bytes[*cursor] != b'\n' {
            *cursor += 1;
        }
    }
}

fn walk(
    root: &Path,
    absolute: &Path,
    policy: &Policy,
    limits: Limits,
    budget: &mut WalkBudget,
    paths: &mut BTreeSet<PathBuf>,
    findings: &mut Findings,
) {
    if budget.saturated {
        return;
    }
    let Ok(relative) = absolute.strip_prefix(root) else {
        findings.push(Finding::new(
            "path",
            "ROOT_ESCAPE",
            Some(absolute.display().to_string()),
            "scope path is outside the canonical root",
        ));
        return;
    };
    let metadata = match fs::symlink_metadata(absolute) {
        Ok(metadata) => metadata,
        Err(error) => {
            findings.push(io_finding("SOURCE_WALK_METADATA", relative, &error));
            return;
        }
    };
    if reserved_git_control(relative) {
        findings.push(Finding::new(
            "git",
            "UNMODELED_GIT_SEMANTICS",
            Some(display_relative(relative)),
            concat!(
                "v1 does not admit hierarchical Git attributes or root ",
                "submodule semantics"
            ),
        ));
    }
    if metadata.file_type().is_symlink() {
        findings.push(Finding::new(
            "path",
            "SYMLINK_IN_SCOPE",
            Some(display_relative(relative)),
            "symlinks are not admitted in checked scope",
        ));
        return;
    }
    if handle_excluded_path(relative, &metadata, &policy.exclusions, findings) {
        return;
    }
    if handle_walk_file(relative, &metadata, limits, budget, paths, findings) {
        return;
    }
    if !metadata.is_dir() {
        findings.push(Finding::new(
            "path",
            "SPECIAL_FILE_IN_SCOPE",
            Some(display_relative(relative)),
            "only regular files and directories are admitted",
        ));
        return;
    }
    let Some(children) =
        collect_walk_children(absolute, relative, limits, budget, findings)
    else {
        return;
    };
    walk_children(root, policy, limits, budget, paths, findings, children);
}

fn handle_excluded_path(
    relative: &Path,
    metadata: &fs::Metadata,
    exclusions: &[Exclusion],
    findings: &mut Findings,
) -> bool {
    if !excluded(relative, exclusions) {
        return false;
    }
    if !metadata.is_dir() && !metadata.is_file() {
        findings.push(Finding::new(
            "path",
            "SPECIAL_EXCLUSION_ROOT",
            Some(display_relative(relative)),
            "excluded path is not a regular file or directory",
        ));
    }
    true
}

fn handle_walk_file(
    relative: &Path,
    metadata: &fs::Metadata,
    limits: Limits,
    budget: &mut WalkBudget,
    paths: &mut BTreeSet<PathBuf>,
    findings: &mut Findings,
) -> bool {
    if !metadata.is_file() {
        return false;
    }
    budget.files = budget.files.saturating_add(1);
    if budget.files > limits.source_file_count {
        findings.push(Finding::new(
            "limits",
            "SOURCE_FILE_COUNT_LIMIT",
            Some(display_relative(relative)),
            format!("source file count exceeds {}", limits.source_file_count),
        ));
        budget.saturated = true;
        return true;
    }
    paths.insert(relative.to_path_buf());
    true
}

fn collect_walk_children(
    absolute: &Path,
    relative: &Path,
    limits: Limits,
    budget: &mut WalkBudget,
    findings: &mut Findings,
) -> Option<Vec<PathBuf>> {
    let entries = match fs::read_dir(absolute) {
        Ok(entries) => entries,
        Err(error) => {
            findings.push(io_finding("SOURCE_WALK_READ", relative, &error));
            return None;
        }
    };
    let mut children = Vec::new();
    let mut directory_entries = 0_usize;
    for entry in entries {
        directory_entries = directory_entries.saturating_add(1);
        budget.entries = budget.entries.saturating_add(1);
        if directory_entries > limits.directory_entry_count {
            findings.push(Finding::new(
                "limits",
                "DIRECTORY_ENTRY_COUNT_LIMIT",
                Some(display_relative(relative)),
                format!(
                    "directory entries exceed {}",
                    limits.directory_entry_count
                ),
            ));
            budget.saturated = true;
            break;
        }
        if budget.entries > limits.source_entry_count {
            findings.push(Finding::new(
                "limits",
                "SOURCE_ENTRY_COUNT_LIMIT",
                Some(display_relative(relative)),
                format!("source entries exceed {}", limits.source_entry_count),
            ));
            budget.saturated = true;
            break;
        }
        match entry {
            Ok(entry) => children.push(entry.path()),
            Err(error) => {
                findings.push(io_finding(
                    "SOURCE_WALK_ENTRY",
                    relative,
                    &error,
                ));
            }
        }
    }
    children.sort();
    Some(children)
}

fn walk_children(
    root: &Path,
    policy: &Policy,
    limits: Limits,
    budget: &mut WalkBudget,
    paths: &mut BTreeSet<PathBuf>,
    findings: &mut Findings,
    children: Vec<PathBuf>,
) {
    for child in children {
        if !walk_child_path_admitted(root, &child, limits, findings) {
            continue;
        }
        walk(root, &child, policy, limits, budget, paths, findings);
    }
}

fn walk_child_path_admitted(
    root: &Path,
    child: &Path,
    limits: Limits,
    findings: &mut Findings,
) -> bool {
    let relative = child.strip_prefix(root).unwrap_or(child);
    let Some(text) = relative.to_str() else {
        findings.push(Finding::new(
            "path",
            "NON_UTF8_PATH",
            None,
            "source path is not valid UTF-8",
        ));
        return false;
    };
    if text.len() <= limits.path_bytes {
        return true;
    }
    findings.push(Finding::new(
        "path",
        "PATH_BYTE_LIMIT",
        Some(text.to_owned()),
        format!("path exceeds {} bytes", limits.path_bytes),
    ));
    false
}

fn reserved_git_control(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == ".gitattributes")
        || path == Path::new(".gitmodules")
}

#[derive(Default)]
struct WalkBudget {
    entries: usize,
    files: usize,
    saturated: bool,
}

fn excluded(path: &Path, exclusions: &[Exclusion]) -> bool {
    exclusions.iter().any(|exclusion| path.starts_with(&exclusion.prefix))
}

fn validate_topology(
    root: &Path,
    files: &[ClassifiedFile],
    topology: &Topology,
    findings: &mut Findings,
) {
    let actual =
        files.iter().map(|file| file.path.clone()).collect::<BTreeSet<_>>();
    let discovered_manifests = actual
        .iter()
        .filter(|path| {
            path.file_name().is_some_and(|name| name == "Cargo.toml")
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    compare_path_set(
        "TOPOLOGY_MANIFEST",
        &topology.manifests.iter().cloned().collect(),
        &discovered_manifests,
        findings,
    );
    let discovered_locks = actual
        .iter()
        .filter(|path| {
            path.file_name().is_some_and(|name| name == "Cargo.lock")
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    compare_path_set(
        "TOPOLOGY_LOCKFILE",
        &topology.lockfiles.iter().cloned().collect(),
        &discovered_locks,
        findings,
    );
    let discovered_configs = actual
        .iter()
        .filter(|path| {
            matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("config" | "config.toml")
            ) && path
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name == ".cargo")
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    compare_path_set(
        "TOPOLOGY_CARGO_CONFIG",
        &topology.cargo_config_files.iter().cloned().collect(),
        &discovered_configs,
        findings,
    );
    let discovered_linker_assets = actual
        .iter()
        .filter(|path| {
            path.extension().is_some_and(|extension| {
                matches!(extension.to_str(), Some("lds" | "x"))
            }) || (path.extension().is_some_and(|extension| extension == "ld")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("link-")))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    compare_path_set(
        "TOPOLOGY_LINKER_ASSET",
        &topology.linker_assets.iter().cloned().collect(),
        &discovered_linker_assets,
        findings,
    );
    let discovered_target_specs = topology
        .profiles
        .iter()
        .filter(|profile| {
            Path::new(&profile.target)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        })
        .map(|profile| PathBuf::from(&profile.target))
        .collect::<BTreeSet<_>>();
    compare_path_set(
        "TOPOLOGY_CUSTOM_TARGET",
        &topology.custom_target_specs.iter().cloned().collect(),
        &discovered_target_specs,
        findings,
    );
    validate_consumed_paths(root, topology, findings);
}

fn validate_consumed_paths(
    root: &Path,
    topology: &Topology,
    findings: &mut Findings,
) {
    // Catch mount and symlink aliases after component-level walk checks.
    for path in topology
        .manifests
        .iter()
        .chain(&topology.lockfiles)
        .chain(&topology.cargo_config_files)
        .chain(&topology.build_scripts)
        .chain(&topology.custom_target_specs)
        .chain(&topology.linker_assets)
    {
        match root.join(path).canonicalize() {
            Ok(canonical) if canonical.starts_with(root) => {}
            Ok(canonical) => findings.push(Finding::new(
                "path",
                "ROOT_ESCAPE",
                Some(display_relative(path)),
                format!(
                    "canonical path escapes root to {}",
                    canonical.display()
                ),
            )),
            Err(error) => {
                findings.push(io_finding("TOPOLOGY_PATH_IO", path, &error));
            }
        }
    }
}

fn compare_path_set(
    code: &'static str,
    expected: &BTreeSet<PathBuf>,
    actual: &BTreeSet<PathBuf>,
    findings: &mut Findings,
) {
    for path in expected.difference(actual) {
        findings.push(Finding::new(
            "topology",
            code,
            Some(display_relative(path)),
            "declared path is missing",
        ));
    }
    for path in actual.difference(expected) {
        findings.push(Finding::new(
            "topology",
            code,
            Some(display_relative(path)),
            "discovered path is not declared",
        ));
    }
}

fn validate_upstream_digests(
    root: &Path,
    policy: &Policy,
    files: &[ClassifiedFile],
    limits: Limits,
    findings: &mut Findings,
) {
    for rule in &policy.rules {
        let Some(upstream) = &rule.upstream else { continue };
        let subject_paths = match &upstream.digest_subject {
            DigestSubject::Paths(paths) => paths.clone(),
            DigestSubject::ClassifiedContent => files
                .iter()
                .filter(|file| {
                    file.classification.provenance == SourceProvenance::Vendored
                        && file.rule_prefix == rule.prefix
                })
                .map(|file| file.path.clone())
                .collect(),
        };
        match aggregate_digest(root, &subject_paths, files, limits) {
            Ok(actual) if actual == upstream.digest => {}
            Ok(actual) => findings.push(Finding::new(
                "classification",
                "UPSTREAM_DIGEST_MISMATCH",
                Some(display_relative(&rule.prefix)),
                format!("expected {}, computed {actual}", upstream.digest),
            )),
            Err(message) => findings.push(Finding::new(
                "classification",
                "UPSTREAM_DIGEST_SUBJECT",
                Some(display_relative(&rule.prefix)),
                message,
            )),
        }
    }
}

fn aggregate_digest(
    root: &Path,
    paths: &[PathBuf],
    files: &[ClassifiedFile],
    limits: Limits,
) -> Result<String, String> {
    let mut paths = paths.to_vec();
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        return Err("digest subject resolved to no classified files".to_owned());
    }
    let mut digest = Sha256::default();
    let mut total = 0_u64;
    for path in paths {
        let Some(classified_file) = files.iter().find(|file| file.path == path)
        else {
            return Err(format!(
                "digest subject {} is not classified",
                path.display()
            ));
        };
        if classified_file.classification.provenance
            == SourceProvenance::Generated
            || classified_file.classification.source_use
                == reovim_depgraph::SourceUse::BuildOutput
        {
            return Err(format!(
                concat!(
                    "digest subject {} is generated/build output rather than ",
                    "an authored or vendored input"
                ),
                path.display()
            ));
        }
        let absolute = root.join(&path);
        let metadata = fs::symlink_metadata(&absolute)
            .map_err(|error| error.to_string())?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink()
        {
            return Err(format!(
                "digest subject {} is not a regular non-symlink file",
                path.display()
            ));
        }
        if metadata.len() > limits.source_file_bytes {
            return Err(format!(
                "digest subject {} exceeds per-file limit",
                path.display()
            ));
        }
        total = total
            .checked_add(metadata.len())
            .ok_or_else(|| "digest subject byte count overflow".to_owned())?;
        if total > limits.source_total_bytes {
            return Err(
                "digest subject exceeds aggregate source-byte limit".to_owned()
            );
        }
        let bytes = read_bounded(&absolute, limits.source_file_bytes)?;
        digest.update(display_relative(&path).as_bytes());
        digest.update(b"\n");
        digest.update(&bytes);
        digest.update(b"\n");
    }
    Ok(hex_digest(digest.finish()))
}

pub fn read_bounded(path: &Path, byte_limit: u64) -> Result<Vec<u8>, String> {
    let before = fs::metadata(path).map_err(|error| error.to_string())?;
    if before.len() > byte_limit {
        return Err("file exceeds per-file byte limit".to_owned());
    }
    let capacity =
        usize::try_from(before.len()).map_err(|error| error.to_string())?;
    let mut bytes = Vec::with_capacity(capacity);
    fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(byte_limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let count =
        u64::try_from(bytes.len()).map_err(|error| error.to_string())?;
    let after = fs::metadata(path).map_err(|error| error.to_string())?;
    if count > byte_limit {
        return Err("file grew beyond per-file byte limit".to_owned());
    }
    if before.len() != count || after.len() != count {
        return Err("file changed while being read".to_owned());
    }
    Ok(bytes)
}

impl<'a> From<&'a ClassifiedFile> for SourceTreeDigestEntry<'a> {
    fn from(file: &'a ClassifiedFile) -> Self {
        Self {
            path: &file.path,
            content_digest: &file.content_digest,
            owner: file.classification.owner,
            provenance: file.classification.provenance,
            source_use: file.classification.source_use,
        }
    }
}

/// Computes the canonical source-tree digest from typed semantic entries.
///
/// Entry order is normalized by path. Callers must reject duplicate paths
/// before using this digest as evidence.
pub fn canonical_source_tree_digest<'a>(
    entries: impl IntoIterator<Item = SourceTreeDigestEntry<'a>>,
) -> String {
    let mut entries = entries.into_iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| left.path.cmp(right.path));
    let mut digest = Sha256::default();
    for entry in entries {
        digest.update(display_relative(entry.path).as_bytes());
        digest.update(b"\0");
        digest.update(entry.content_digest.as_bytes());
        digest.update(b"\0");
        match entry.owner {
            SourceOwner::Product(role) => {
                digest.update(b"product:");
                digest.update(role.as_str().as_bytes());
            }
            SourceOwner::RepositoryTooling => {
                digest.update(b"repository_tooling");
            }
        }
        digest.update(b"\0");
        digest.update(entry.provenance.as_str().as_bytes());
        digest.update(b"\0");
        digest.update(entry.source_use.as_str().as_bytes());
        digest.update(b"\n");
    }
    hex_digest(digest.finish())
}

fn hex_digest(bytes: [u8; 32]) -> String {
    let mut result = String::from("sha256:");
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut result, "{byte:02x}").expect("write to String");
    }
    result
}

#[must_use]
pub fn display_relative(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn io_finding(code: &'static str, path: &Path, error: &io::Error) -> Finding {
    Finding::new("path", code, Some(display_relative(path)), error.to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use reovim_depgraph::{
        ArchitecturalRole, SourceClassification, SourceOwner, SourceProvenance,
        SourceUse,
    };

    use super::SourceTreeDigestEntry;

    const FIRST_DIGEST: &str = concat!(
        "sha256:",
        "1111111111111111111111111111111111111111111111111111111111111111"
    );
    const SECOND_DIGEST: &str = concat!(
        "sha256:",
        "2222222222222222222222222222222222222222222222222222222222222222"
    );

    fn digest_entry<'a>(
        path: &'a str,
        content_digest: &'a str,
    ) -> SourceTreeDigestEntry<'a> {
        SourceTreeDigestEntry {
            path: std::path::Path::new(path),
            content_digest,
            owner: SourceOwner::Product(ArchitecturalRole::Foundation),
            provenance: SourceProvenance::Authored,
            source_use: SourceUse::Production,
        }
    }

    #[test]
    fn canonical_tree_digest_preserves_exact_byte_framing() {
        let entries = [
            digest_entry("src/lib.rs", SECOND_DIGEST),
            SourceTreeDigestEntry {
                path: std::path::Path::new("Cargo.toml"),
                content_digest: FIRST_DIGEST,
                owner: SourceOwner::RepositoryTooling,
                provenance: SourceProvenance::Authored,
                source_use: SourceUse::Tool,
            },
        ];
        let expected = concat!(
            "Cargo.toml\0",
            "sha256:",
            "1111111111111111111111111111111111111111111111111111111111111111",
            "\0repository_tooling\0authored\0tool\n",
            "src/lib.rs\0",
            "sha256:",
            "2222222222222222222222222222222222222222222222222222222222222222",
            "\0product:Foundation\0authored\0production\n",
        );

        assert_eq!(
            super::canonical_source_tree_digest(entries),
            crate::digest::bytes(expected.as_bytes())
        );
    }

    #[test]
    fn canonical_tree_digest_is_input_order_independent() {
        let first = digest_entry("a.rs", FIRST_DIGEST);
        let second = digest_entry("z.rs", SECOND_DIGEST);

        assert_eq!(
            super::canonical_source_tree_digest([first, second]),
            super::canonical_source_tree_digest([second, first])
        );
    }

    #[test]
    fn canonical_tree_digest_is_sensitive_to_every_semantic_field() {
        let baseline = digest_entry("src/lib.rs", FIRST_DIGEST);
        let expected = super::canonical_source_tree_digest([baseline]);
        let mutants = [
            digest_entry("src/main.rs", FIRST_DIGEST),
            digest_entry("src/lib.rs", SECOND_DIGEST),
            SourceTreeDigestEntry {
                owner: SourceOwner::Product(ArchitecturalRole::KernelInterface),
                ..baseline
            },
            SourceTreeDigestEntry {
                provenance: SourceProvenance::Vendored,
                ..baseline
            },
            SourceTreeDigestEntry { source_use: SourceUse::Test, ..baseline },
        ];

        for mutant in mutants {
            assert_ne!(super::canonical_source_tree_digest([mutant]), expected);
        }
    }

    #[test]
    fn canonical_empty_tree_digest_matches_empty_sha256() {
        assert_eq!(
            super::canonical_source_tree_digest(std::iter::empty()),
            crate::digest::bytes(b"")
        );
    }

    #[test]
    fn panic_assignment_scanner_covers_toml_key_forms() {
        for input in [
            "panic = \"abort\"",
            "panic = 'abort'",
            "'panic'\n =\n 'abort'",
            "profile.release.panic = \"abort\"",
            "profile = { release = { panic = 'abort' } }",
            "\"panic\" = \"unwind\"",
            "\"pan\\u0069c\" = \"abort\"",
        ] {
            assert!(super::manifest_assigns_panic(input), "{input}");
        }
    }

    #[test]
    fn panic_assignment_scanner_ignores_comments_and_string_values() {
        for input in [
            "# panic = 'abort'",
            "description = \"panic = 'abort'\"",
            "description = '''panic = 'abort' '''",
            "key = \"panic\"",
        ] {
            assert!(!super::manifest_assigns_panic(input), "{input}");
        }
    }

    #[test]
    fn generated_mechanism_is_closed_to_declared_lockfiles() {
        let generated = |path: &str| super::ClassifiedFile {
            path: PathBuf::from(path),
            content_digest: crate::digest::bytes(b"x"),
            classification: SourceClassification::new(
                SourceOwner::RepositoryTooling,
                SourceProvenance::Generated,
                SourceUse::Tool,
            )
            .unwrap(),
            upstream: None,
            rule_prefix: PathBuf::from(path),
            rule_precedence: 1,
        };
        let lockfiles =
            [PathBuf::from("Cargo.lock"), PathBuf::from("product/Cargo.lock")];
        assert!(super::generated_mechanism_supported(
            &generated("Cargo.lock"),
            &lockfiles,
        ));
        assert!(super::generated_mechanism_supported(
            &generated("product/Cargo.lock"),
            &lockfiles,
        ));
        assert!(!super::generated_mechanism_supported(
            &generated("src/generated.json"),
            &lockfiles
        ));
    }

    #[test]
    fn entrypoint_scanner_visits_every_direct_local_path() {
        let (inputs, dynamic) = super::obvious_local_inputs(
            "./scripts/first.sh && ./scripts/second.sh; ./scripts/third.sh",
        );
        assert_eq!(
            inputs,
            [
                PathBuf::from("scripts/first.sh"),
                PathBuf::from("scripts/second.sh"),
                PathBuf::from("scripts/third.sh"),
            ]
            .into_iter()
            .collect()
        );
        assert!(!dynamic);

        let (_, dynamic) = super::obvious_local_inputs("./ && ./../outside");
        assert!(dynamic);
    }

    #[test]
    fn entrypoint_scanner_rejects_variable_expanded_command_paths() {
        for input in [
            "\"$ROOT/tmp/hidden\"",
            "if \"${ROOT}/tmp/hidden\"; then :; fi",
            "env MODE=test \"$ROOT\"/tmp/hidden",
            "command \"${ROOT}/tmp/hidden\"",
            "HIDDEN=\"$ROOT/tmp/hidden\"; \"$HIDDEN\"",
            "HIDDEN=\"path with spaces\" \"$HIDDEN\"",
            "HIDDEN=\"$ROOT/tmp/hidden\"; sh \"$HIDDEN\"",
            "HIDDEN=\"$ROOT/tmp/hidden\"; exec \\\n  \"$HIDDEN\"",
            "OUT=\"$(\"$ROOT/tmp/hidden\")\"",
            "OUT=`\"$ROOT/tmp/hidden\"`",
        ] {
            let (_, dynamic) = super::obvious_local_inputs(input);
            assert!(dynamic, "{input}");
        }
        let (_, dynamic) = super::obvious_local_inputs(
            "CARGO_HOME=\"$ROOT/tmp\" cargo --version",
        );
        assert!(!dynamic);
        let (_, dynamic) = super::obvious_local_inputs(
            "ROOT=\"$(cd \"$(dirname \"${BASH_SOURCE[0]}\")/..\" && pwd)\"",
        );
        assert!(!dynamic);

        let (_, dynamic) = super::obvious_local_inputs(
            "VALUES=(\n  \"$(\"$ROOT/tmp/hidden\")\"\n)\n",
        );
        assert!(dynamic);
        let (_, dynamic) = super::obvious_local_inputs(
            "cat <<EOF\n$(\"$ROOT/tmp/hidden\")\nEOF\n",
        );
        assert!(dynamic);
        let (_, dynamic) = super::obvious_local_inputs(
            "cat <<'EOF'\n$(\"$ROOT/tmp/hidden\")\nEOF\n",
        );
        assert!(!dynamic);
    }

    #[test]
    fn entrypoint_scanner_closes_held_execution_forms_exactly() {
        let probe = "/proc/$$/fd/8 8<&- --version >/dev/null";
        let (_, dynamic) = super::obvious_local_inputs(probe);
        assert!(!dynamic);
        for mutant in [
            probe.replace(" 8<&-", ""),
            probe.replace("/proc/$$", "/proc/self"),
            probe.replace("/proc/$$/fd/8", "/dev/fd/8"),
            format!("exec {probe}"),
            probe.replace("--version", "--help"),
        ] {
            let (_, dynamic) = super::obvious_local_inputs(&mutant);
            assert!(dynamic, "{mutant}");
        }

        let cargo = concat!(
            "CARGO_HOME=\"$CARGO_HOME_PATH\" \\\n",
            "    CARGO_TARGET_DIR=\"$ROOT/target\" \\\n",
            "    RUSTC=\"$RUSTC_BIN\" \\\n",
            "    /proc/$$/fd/8 8<&- build --locked --offline \\\n",
            "    --manifest-path \"$ROOT/Cargo.toml\" \\\n",
            "    -p reovim-conformance --bin reovim-conformance \\\n",
            "    --target \"$CHECKER_TARGET\" --release",
        );
        let (_, dynamic) = super::obvious_local_inputs(cargo);
        assert!(!dynamic);

        for mutant in [
            cargo.replace(" 8<&-", ""),
            cargo.replace("/proc/$$", "/proc/self"),
            cargo.replace("    /proc/$$", "    exec /proc/$$"),
            cargo.replace("--offline ", ""),
            cargo.replace("--manifest-path \"$ROOT/Cargo.toml\" ", ""),
            cargo.replace("--bin reovim-conformance", "--bin other"),
            cargo.replace("--target \"$CHECKER_TARGET\"", "--target other"),
            cargo.replace(" --release", ""),
        ] {
            let (_, dynamic) = super::obvious_local_inputs(&mutant);
            assert!(dynamic, "{mutant}");
        }

        let reviewed = concat!(
            "CARGO_HOME=\"$CARGO_HOME_PATH\" \\\n",
            "    CARGO_TARGET_DIR=\"$ROOT/target\" \\\n",
            "    RUSTC=\"$RUSTC_BIN\" \\\n",
            "    /proc/$$/fd/9 9<&- \"$@\"",
        );
        let (_, dynamic) = super::obvious_local_inputs(reviewed);
        assert!(!dynamic);

        for mutant in [
            reviewed.replace(" 9<&-", ""),
            reviewed.replace("/proc/$$", "/proc/self"),
            reviewed.replace("/proc/$$/fd/9", "/dev/fd/9"),
            reviewed.replace("\"$@\"", "\"$*\""),
            reviewed.replace("RUSTC=\"$RUSTC_BIN\"", "RUSTC=$RUSTC_BIN"),
            format!("{reviewed}; true"),
        ] {
            let (_, dynamic) = super::obvious_local_inputs(&mutant);
            assert!(dynamic, "{mutant}");
        }

        let (_, dynamic) = super::obvious_local_inputs(
            "[[ -f /proc/self/fd/8 && -x /proc/self/fd/9 ]]",
        );
        assert!(!dynamic);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn held_checker_form_executes_one_image_without_fd_leak() {
        use std::process::Command;

        let shell = r#"
set -euo pipefail
fixture="$(mktemp -d)"
trap 'find "$fixture" -depth -delete' EXIT
bash_bin="$(command -v bash)"
false_bin="$(command -v false)"
exec 8< "$bash_bin"
if /proc/self/fd/8 -c '[[ ! -e /proc/self/fd/8 ]]'; then
    exit 90
fi
/proc/$$/fd/8 8<&- -c '[[ ! -e /proc/self/fd/8 ]]'
[[ -e /proc/$$/fd/8 ]]
exec 8<&-
[[ ! -e /proc/$$/fd/8 ]]
ln -s "$bash_bin" "$fixture/held"
exec 9< "$fixture/held"
ln -sfn "$false_bin" "$fixture/held"
for round in 1 2; do
    /proc/$$/fd/9 9<&- -c '
        [[ ! -e /proc/self/fd/9 ]]
        bash -c "[[ ! -e /proc/self/fd/9 ]]"
    '
done
[[ -e /proc/$$/fd/9 ]]
if /proc/$$/fd/9 -c '[[ ! -e /proc/self/fd/9 ]]'; then
    exit 91
fi
exec 9<&-
[[ ! -e /proc/$$/fd/9 ]]
"#;
        let output =
            Command::new("bash").arg("-c").arg(shell).output().unwrap();
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    #[test]
    fn current_entrypoint_command_positions_use_supported_forms() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        for path in [
            "scripts/check.sh",
            "scripts/check-documentation.sh",
            "scripts/check-conformance.sh",
            "scripts/check-style.sh",
            ".github/workflows/ci.yml",
        ] {
            let contents = std::fs::read_to_string(root.join(path)).unwrap();
            assert_static_entrypoint(path, &contents);
        }
    }

    fn assert_static_entrypoint(path: &str, contents: &str) {
        let mut scan = super::LocalInputScan::default();
        for (index, line) in contents.lines().enumerate() {
            scan.line(line);
            assert!(!scan.dynamic, "{path}:{}: {line}", index + 1);
        }
        let (_, dynamic) = scan.finish();
        assert!(!dynamic, "{path}");
    }
}
