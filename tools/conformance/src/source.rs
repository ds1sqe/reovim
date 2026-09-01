//! Root-contained source inventory, classification, and topology discovery.

use std::{
    collections::BTreeSet,
    fs, io,
    io::Read as _,
    path::{Path, PathBuf},
};

use reovim_depgraph::{SourceClassification, SourceOwner, SourceProvenance, SourceUse};

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

#[allow(clippy::too_many_lines)]
pub fn inventory(root: &Path, policy: &Policy, topology: &Topology) -> Inventory {
    let mut findings = Findings::new(topology.limits.findings);
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
    let classification_map = match policy.classification_map() {
        Ok(map) => map,
        Err(error) => {
            findings.push(Finding::new("policy", error.code, Some(error.path), error.message));
            return Inventory {
                files: Vec::new(),
                exclusions: policy.exclusions.clone(),
                tree_digest: digest::bytes(b""),
                findings: findings.finish(),
            };
        }
    };
    let mut paths = BTreeSet::new();
    let mut budget = WalkBudget::default();
    for scope_root in &policy.scope_roots {
        let absolute = root.join(scope_root);
        walk(root, &absolute, policy, topology.limits, &mut budget, &mut paths, &mut findings);
    }

    let mut files = Vec::new();
    let mut total = 0_u64;
    for (file_index, path) in paths.into_iter().enumerate() {
        if file_index >= topology.limits.source_file_count {
            findings.push(Finding::new(
                "limits",
                "SOURCE_FILE_COUNT_LIMIT",
                None,
                format!("source file count exceeds {}", topology.limits.source_file_count),
            ));
            break;
        }
        let path_text = display_relative(&path);
        if path_text.len() > topology.limits.path_bytes {
            findings.push(Finding::new(
                "path",
                "PATH_BYTE_LIMIT",
                Some(path_text),
                format!("path exceeds {} bytes", topology.limits.path_bytes),
            ));
            continue;
        }
        let absolute = root.join(&path);
        let metadata = match fs::symlink_metadata(&absolute) {
            Ok(metadata) => metadata,
            Err(error) => {
                findings.push(io_finding("SOURCE_METADATA", &path, error));
                continue;
            }
        };
        if !metadata.file_type().is_file() {
            findings.push(Finding::new(
                "path",
                "SOURCE_NOT_REGULAR",
                Some(path_text),
                "classified path is not a regular file",
            ));
            continue;
        }
        if metadata.len() > topology.limits.source_file_bytes {
            findings.push(Finding::new(
                "limits",
                "SOURCE_FILE_BYTE_LIMIT",
                Some(path_text),
                format!(
                    "{} bytes exceeds per-file limit {}",
                    metadata.len(),
                    topology.limits.source_file_bytes
                ),
            ));
            continue;
        }
        total = match total.checked_add(metadata.len()) {
            Some(value) if value <= topology.limits.source_total_bytes => value,
            _ => {
                findings.push(Finding::new(
                    "limits",
                    "SOURCE_TOTAL_BYTE_LIMIT",
                    Some(path_text),
                    format!("source bytes exceed {}", topology.limits.source_total_bytes),
                ));
                break;
            }
        };
        let classification = match classification_map.classify(&path) {
            Ok(classification) => classification,
            Err(error) => {
                findings.push(Finding::new(
                    "classification",
                    "SOURCE_CLASSIFICATION",
                    Some(path_text),
                    error.to_string(),
                ));
                continue;
            }
        };
        let content_digest =
            match digest::file_bounded(&absolute, topology.limits.source_file_bytes) {
                Ok(digest) => digest,
                Err(error) => {
                    findings.push(io_finding("SOURCE_DIGEST", &path, error));
                    continue;
                }
            };
        files.push(ClassifiedFile {
            path: path.clone(),
            content_digest,
            classification,
            upstream: policy.upstream_for(&path).cloned(),
            rule_prefix: policy
                .winning_rule(&path)
                .map(|rule| rule.prefix.clone())
                .unwrap_or_default(),
            rule_precedence: policy.winning_rule(&path).map_or(0, |rule| rule.precedence),
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    for file in files.iter().filter(|file| !generated_mechanism_supported(file)) {
        findings.push(Finding::new(
            "classification",
            "GENERATED_MECHANISM_PROOF_UNAVAILABLE",
            Some(display_relative(&file.path)),
            "v1 validates regeneration only for the exact root Cargo.lock",
        ));
    }
    for manifest in
        files.iter().filter(|file| file.path.file_name().is_some_and(|name| name == "Cargo.toml"))
    {
        match read_bounded(&root.join(&manifest.path), topology.limits.source_file_bytes) {
            Ok(bytes) if digest::bytes(&bytes) != manifest.content_digest => {
                findings.push(Finding::new(
                    "manifest",
                    "MANIFEST_INVENTORY_DRIFT",
                    Some(display_relative(&manifest.path)),
                    "manifest bytes differ from the inventoried content digest",
                ));
            }
            Ok(bytes) => match String::from_utf8(bytes) {
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
            },
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
    if let Some(file) =
        files.iter().find(|file| matches!(file.classification.owner, SourceOwner::Product(_)))
    {
        findings.push(Finding::new(
            "product_boundary",
            "PRODUCT_BOUNDARY_PROOF_UNAVAILABLE",
            Some(display_relative(&file.path)),
            "v1 syntax and Cargo evidence cannot prove Product source/public/packaging boundaries",
        ));
    }
    for rule in &policy.rules {
        if !files
            .iter()
            .any(|file| file.rule_prefix == rule.prefix && file.rule_precedence == rule.precedence)
        {
            findings.push(Finding::new(
                "classification",
                "DEAD_CLASSIFICATION_RULE",
                Some(display_relative(&rule.prefix)),
                format!("precedence {} wins for no inventoried file", rule.precedence),
            ));
        }
    }
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
                "required policy/topology/review/checker input is absent or unclassified",
            ));
        }
    }
    for entrypoint in &topology.repository_entrypoints {
        if let crate::topology::RepositoryEntrypoint::Script { path, content_digest }
        | crate::topology::RepositoryEntrypoint::Workflow { path, content_digest } = entrypoint
        {
            match files.iter().find(|file| file.path == *path) {
                Some(file)
                    if file.classification.owner == SourceOwner::RepositoryTooling
                        && file.classification.source_use == reovim_depgraph::SourceUse::Tool
                        && file.content_digest == *content_digest => {}
                Some(file) if file.content_digest != *content_digest => {
                    findings.push(Finding::new(
                        "topology",
                        "ENTRYPOINT_CONTENT_DIGEST",
                        Some(display_relative(path)),
                        format!(
                            "entrypoint content digest {} differs from admitted {content_digest}",
                            file.content_digest
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
    }
    let declared_scripts = topology
        .repository_entrypoints
        .iter()
        .filter_map(|entrypoint| match entrypoint {
            crate::topology::RepositoryEntrypoint::Script { path, .. } => Some(path.clone()),
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
        &mut findings,
    );
    let declared_workflows = topology
        .repository_entrypoints
        .iter()
        .filter_map(|entrypoint| match entrypoint {
            crate::topology::RepositoryEntrypoint::Workflow { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let discovered_workflows = files
        .iter()
        .filter(|file| {
            file.path.starts_with(".github/workflows")
                && file.path.extension().is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
                })
        })
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    compare_path_set(
        "TOPOLOGY_WORKFLOW_ENTRYPOINT",
        &declared_workflows,
        &discovered_workflows,
        &mut findings,
    );
    validate_entrypoint_lexical_warnings(root, &files, topology, &mut findings);
    validate_topology(root, &files, topology, &mut findings);
    validate_upstream_digests(root, policy, &files, topology.limits, &mut findings);
    let tree_digest = tree_digest(&files);
    Inventory {
        files,
        exclusions: policy.exclusions.clone(),
        tree_digest,
        findings: findings.finish(),
    }
}

fn validate_entrypoint_lexical_warnings(
    root: &Path,
    files: &[ClassifiedFile],
    topology: &Topology,
    findings: &mut Findings,
) {
    for entrypoint in &topology.repository_entrypoints {
        let path = match entrypoint {
            crate::topology::RepositoryEntrypoint::Script { path, .. }
            | crate::topology::RepositoryEntrypoint::Workflow { path, .. } => path,
            crate::topology::RepositoryEntrypoint::CargoTarget { .. } => continue,
        };
        let Some(caller) = files.iter().find(|file| file.path == *path) else { continue };
        let bytes = match read_bounded(&root.join(path), topology.limits.source_file_bytes) {
            Ok(bytes) if digest::bytes(&bytes) == caller.content_digest => bytes,
            Ok(_) => {
                findings.push(Finding::new(
                    "entrypoint",
                    "ENTRYPOINT_INPUT_INVENTORY_DRIFT",
                    Some(display_relative(path)),
                    "entrypoint bytes differ from inventory",
                ));
                continue;
            }
            Err(error) => {
                findings.push(Finding::new(
                    "entrypoint",
                    "ENTRYPOINT_INPUT_READ",
                    Some(display_relative(path)),
                    error,
                ));
                continue;
            }
        };
        let Ok(contents) = std::str::from_utf8(&bytes) else {
            findings.push(Finding::new(
                "entrypoint",
                "ENTRYPOINT_INPUT_UTF8",
                Some(display_relative(path)),
                "v1 script/workflow admission requires UTF-8",
            ));
            continue;
        };
        let (inputs, dynamic) = obvious_local_inputs(contents);
        if dynamic {
            findings.push(Finding::new(
                "entrypoint",
                "ENTRYPOINT_LOCAL_INPUT_PROOF_UNAVAILABLE",
                Some(display_relative(path)),
                "defense-in-depth lexical warning detected a dynamic or unsupported local invocation forbidden by the exact review admission",
            ));
        }
        for input in inputs {
            let target = files.iter().find(|file| file.path == input);
            match target {
                Some(target)
                    if target.classification.owner == SourceOwner::RepositoryTooling
                        && target.classification.source_use == SourceUse::Tool => {}
                Some(_) => findings.push(Finding::new(
                    "entrypoint",
                    "ENTRYPOINT_LOCAL_INPUT_CLASSIFICATION",
                    Some(display_relative(&input)),
                    "script/workflow local input must be RepositoryTooling + Tool",
                )),
                None => findings.push(Finding::new(
                    "entrypoint",
                    "ENTRYPOINT_LOCAL_INPUT_UNCLASSIFIED",
                    Some(display_relative(&input)),
                    format!("{} consumes an omitted local input", display_relative(path)),
                )),
            }
        }
    }
}

pub fn obvious_local_inputs(contents: &str) -> (BTreeSet<PathBuf>, bool) {
    let mut inputs = BTreeSet::new();
    let mut dynamic = false;
    let mut single_quoted_block = false;
    let mut heredoc = None::<(String, bool)>;
    let mut array_assignment = false;
    let mut logical_command = String::new();
    for line in contents.lines() {
        let heredoc_body = if let Some((delimiter, expands)) = heredoc.as_ref() {
            if line.trim() == delimiter {
                heredoc = None;
                continue;
            }
            if !expands {
                continue;
            }
            true
        } else {
            false
        };
        if !heredoc_body && single_quoted_block {
            if line.trim_start().starts_with('\'') {
                single_quoted_block = false;
            }
            continue;
        }
        if !heredoc_body && let Some(delimiter) = shell_heredoc_delimiter(line) {
            heredoc = Some(delimiter);
        }
        let line = if !heredoc_body
            && line.trim_end().ends_with('\'')
            && line.chars().filter(|character| *character == '\'').count() % 2 == 1
            && !line.trim_start().starts_with('#')
        {
            single_quoted_block = true;
            &line[..line.rfind('\'').expect("odd quote count has a quote")]
        } else {
            line
        };
        let starts_array = !array_assignment && shell_array_assignment_starts(line);
        if heredoc_body || array_assignment || starts_array {
            if command_substitution_has_variable_command(line)
                || has_unquoted_backtick_substitution(line)
            {
                dynamic = true;
            }
        } else {
            if !logical_command.is_empty() {
                logical_command.push(' ');
            }
            logical_command.push_str(line.trim_end_matches([' ', '\t', '\\']));
            if !line.trim_end().ends_with('\\') {
                if variable_expanded_command_path(&logical_command) {
                    dynamic = true;
                }
                logical_command.clear();
            }
        }
        scan_lexical_local_inputs(line, &mut inputs, &mut dynamic);
        if starts_array {
            array_assignment = true;
        }
        if array_assignment && line.trim() == ")" {
            array_assignment = false;
        }
    }
    dynamic |= !logical_command.is_empty();
    (inputs, dynamic)
}

fn scan_lexical_local_inputs(line: &str, inputs: &mut BTreeSet<PathBuf>, dynamic: &mut bool) {
    let words = line
        .split(|character: char| character.is_whitespace() || ";|&".contains(character))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    for pair in words.windows(2) {
        if matches!(pair[0], "source" | ".") {
            let raw = pair[1].trim_matches(['\'', '"']);
            if raw.contains(['$', '`']) {
                *dynamic = true;
            } else if let Ok(path) = crate::policy::relative_path(raw, "entrypoint", false) {
                inputs.insert(path);
            } else {
                *dynamic = true;
            }
        } else if matches!(pair[0], "bash" | "sh" | "python" | "python3" | "node" | "exec") {
            let raw = pair[1].trim_matches(['\'', '"']);
            if raw.contains(['$', '`']) {
                *dynamic = true;
                continue;
            }
            let local_looking = raw.contains('/')
                || Path::new(raw).extension().is_some_and(|extension| {
                    matches!(extension.to_str(), Some("sh" | "py" | "js" | "ts"))
                });
            if local_looking {
                if let Ok(path) = crate::policy::relative_path(raw, "entrypoint", false) {
                    inputs.insert(path);
                } else {
                    *dynamic = true;
                }
            }
        }
    }
    let mut remaining = line;
    while let Some(start) = remaining.find("./") {
        let suffix = &remaining[start + 2..];
        let end = suffix
            .find(|character: char| {
                !(character.is_ascii_alphanumeric() || "._-/".contains(character))
            })
            .unwrap_or(suffix.len());
        let candidate = &suffix[..end];
        if candidate.is_empty() {
            *dynamic = true;
        } else if let Ok(path) = crate::policy::relative_path(candidate, "entrypoint", false) {
            inputs.insert(path);
        } else {
            *dynamic = true;
        }
        remaining = &suffix[end..];
    }
}

fn shell_heredoc_delimiter(line: &str) -> Option<(String, bool)> {
    let start = line.find("<<")?;
    let mut suffix = line[start + 2..].trim_start();
    suffix = suffix.strip_prefix('-').unwrap_or(suffix).trim_start();
    let quote = suffix.chars().next().filter(|character| matches!(character, '\'' | '"'));
    if quote.is_some() {
        suffix = &suffix[1..];
    }
    let end = suffix
        .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .unwrap_or(suffix.len());
    let delimiter = &suffix[..end];
    (!delimiter.is_empty()).then(|| (delimiter.to_owned(), quote.is_none()))
}

fn shell_array_assignment_starts(line: &str) -> bool {
    let word = line.split_whitespace().next().unwrap_or_default();
    word.split_once('=').is_some_and(|(name, value)| {
        shell_assignment(&format!("{name}=")) && value == "(" && !line.trim_end().ends_with(')')
    })
}

fn variable_expanded_command_path(line: &str) -> bool {
    if line.trim_start().starts_with('\'') {
        return false;
    }
    if command_substitution_has_variable_command(line) || has_unquoted_backtick_substitution(line) {
        return true;
    }
    let trimmed = line.trim_start();
    if (trimmed.starts_with("[[") || trimmed.starts_with("if [[")) && trimmed.contains("]]")
        || trimmed.starts_with("((") && trimmed.contains("))")
    {
        return false;
    }
    shell_segments(line).iter().any(|segment| {
        let words = shell_tokens(segment);
        let mut cursor = 0_usize;
        while let Some(word) = words.get(cursor).map(|word| shell_word(word)) {
            if shell_assignment(word) && word.contains("$(") {
                return false;
            }
            if word.is_empty()
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
            {
                cursor += 1;
                continue;
            }
            break;
        }

        if words.get(cursor).is_some_and(|word| shell_word(word) == "command")
            && words.get(cursor + 1).is_some_and(|word| matches!(shell_word(word), "-v" | "-V"))
        {
            return false;
        }

        while let Some(wrapper) = words.get(cursor).map(|word| shell_word(word))
            && matches!(wrapper, "command" | "exec" | "env" | "nohup" | "time")
        {
            cursor += 1;
            while let Some(word) = words.get(cursor).map(|word| shell_word(word))
                && (word.starts_with('-') || shell_assignment(word))
            {
                cursor += 1;
            }
        }

        words.get(cursor).is_some_and(|word| variable_command_word(shell_word(word)))
    })
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
    let mut segments = Vec::new();
    let mut segment = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            segment.push(character);
            escaped = false;
        } else if character == '\\' && quote != Some('\'') {
            segment.push(character);
            escaped = true;
        } else if let Some(active_quote) = quote {
            segment.push(character);
            if character == active_quote {
                quote = None;
            }
        } else if matches!(character, '\'' | '"') {
            segment.push(character);
            quote = Some(character);
        } else if ";|&".contains(character) {
            if !segment.trim().is_empty() {
                segments.push(std::mem::take(&mut segment));
            }
        } else {
            segment.push(character);
        }
    }
    if !segment.trim().is_empty() {
        segments.push(segment);
    }
    segments
}

fn shell_tokens(segment: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in segment.chars() {
        if escaped {
            word.push(character);
            escaped = false;
        } else if character == '\\' && quote != Some('\'') {
            word.push(character);
            escaped = true;
        } else if let Some(active_quote) = quote {
            word.push(character);
            if character == active_quote {
                quote = None;
            }
        } else if matches!(character, '\'' | '"') {
            word.push(character);
            quote = Some(character);
        } else if character.is_whitespace() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        } else {
            word.push(character);
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

fn shell_word(word: &str) -> &str {
    word.trim_matches(|character| matches!(character, '\'' | '"' | '\\' | '(' | ')'))
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

fn generated_mechanism_supported(file: &ClassifiedFile) -> bool {
    file.classification.provenance != SourceProvenance::Generated
        || file.path == Path::new("Cargo.lock")
}

fn manifest_assigns_panic(input: &str) -> bool {
    let bytes = input.as_bytes();
    let mut cursor = 0_usize;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'#' => {
                cursor += 1;
                while cursor < bytes.len() && bytes[cursor] != b'\n' {
                    cursor += 1;
                }
            }
            quote @ (b'\'' | b'"') => {
                let (value, next) = quoted_token(bytes, cursor, quote);
                cursor = next;
                if value == b"panic" || value.contains(&b'\\') {
                    skip_toml_space_and_comments(bytes, &mut cursor);
                    if bytes.get(cursor) == Some(&b'=') {
                        return true;
                    }
                }
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' || byte == b'-' => {
                let start = cursor;
                cursor += 1;
                while cursor < bytes.len()
                    && (bytes[cursor].is_ascii_alphanumeric()
                        || matches!(bytes[cursor], b'_' | b'-'))
                {
                    cursor += 1;
                }
                if &bytes[start..cursor] == b"panic" {
                    skip_toml_space_and_comments(bytes, &mut cursor);
                    if bytes.get(cursor) == Some(&b'=') {
                        return true;
                    }
                }
            }
            _ => cursor += 1,
        }
    }
    false
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

#[allow(clippy::too_many_lines)]
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
            findings.push(io_finding("SOURCE_WALK_METADATA", relative, error));
            return;
        }
    };
    if reserved_git_control(relative) {
        findings.push(Finding::new(
            "git",
            "UNMODELED_GIT_SEMANTICS",
            Some(display_relative(relative)),
            "v1 does not admit hierarchical Git attributes or root submodule semantics",
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
    if excluded(relative, &policy.exclusions) {
        if !metadata.is_dir() && !metadata.is_file() {
            findings.push(Finding::new(
                "path",
                "SPECIAL_EXCLUSION_ROOT",
                Some(display_relative(relative)),
                "excluded path is not a regular file or directory",
            ));
        }
        return;
    }
    if metadata.is_file() {
        budget.files = budget.files.saturating_add(1);
        if budget.files > limits.source_file_count {
            findings.push(Finding::new(
                "limits",
                "SOURCE_FILE_COUNT_LIMIT",
                Some(display_relative(relative)),
                format!("source file count exceeds {}", limits.source_file_count),
            ));
            budget.saturated = true;
            return;
        }
        paths.insert(relative.to_path_buf());
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
    let entries = match fs::read_dir(absolute) {
        Ok(entries) => entries,
        Err(error) => {
            findings.push(io_finding("SOURCE_WALK_READ", relative, error));
            return;
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
                format!("directory entries exceed {}", limits.directory_entry_count),
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
            Err(error) => findings.push(io_finding("SOURCE_WALK_ENTRY", relative, error)),
        }
    }
    children.sort();
    for child in children {
        let relative = child.strip_prefix(root).unwrap_or(&child);
        let Some(text) = relative.to_str() else {
            findings.push(Finding::new(
                "path",
                "NON_UTF8_PATH",
                None,
                "source path is not valid UTF-8",
            ));
            continue;
        };
        if text.len() > limits.path_bytes {
            findings.push(Finding::new(
                "path",
                "PATH_BYTE_LIMIT",
                Some(text.to_owned()),
                format!("path exceeds {} bytes", limits.path_bytes),
            ));
            continue;
        }
        walk(root, &child, policy, limits, budget, paths, findings);
    }
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
    let actual = files.iter().map(|file| file.path.clone()).collect::<BTreeSet<_>>();
    let discovered_manifests = actual
        .iter()
        .filter(|path| path.file_name().is_some_and(|name| name == "Cargo.toml"))
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
        .filter(|path| path.file_name().is_some_and(|name| name == "Cargo.lock"))
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
            ) && path.parent().and_then(Path::file_name).is_some_and(|name| name == ".cargo")
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
            path.extension()
                .is_some_and(|extension| matches!(extension.to_str(), Some("lds" | "x")))
                || (path.extension().is_some_and(|extension| extension == "ld")
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
    // Canonicalize every explicitly consumed path to catch mount/symlink
    // aliases even after the walk's component-level checks.
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
                format!("canonical path escapes root to {}", canonical.display()),
            )),
            Err(error) => findings.push(io_finding("TOPOLOGY_PATH_IO", path, error)),
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
        let Some(classified_file) = files.iter().find(|file| file.path == path) else {
            return Err(format!("digest subject {} is not classified", path.display()));
        };
        if classified_file.classification.provenance == SourceProvenance::Generated
            || classified_file.classification.source_use == reovim_depgraph::SourceUse::BuildOutput
        {
            return Err(format!(
                "digest subject {} is generated/build output rather than an authored or vendored input",
                path.display()
            ));
        }
        let absolute = root.join(&path);
        let metadata = fs::symlink_metadata(&absolute).map_err(|error| error.to_string())?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(format!(
                "digest subject {} is not a regular non-symlink file",
                path.display()
            ));
        }
        if metadata.len() > limits.source_file_bytes {
            return Err(format!("digest subject {} exceeds per-file limit", path.display()));
        }
        total = total
            .checked_add(metadata.len())
            .ok_or_else(|| "digest subject byte count overflow".to_owned())?;
        if total > limits.source_total_bytes {
            return Err("digest subject exceeds aggregate source-byte limit".to_owned());
        }
        let bytes = read_bounded(&absolute, limits.source_file_bytes)?;
        digest.update(display_relative(&path).as_bytes());
        digest.update(b"\n");
        digest.update(&bytes);
        digest.update(b"\n");
    }
    Ok(hex_digest(digest.finish()))
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn read_bounded(path: &Path, byte_limit: u64) -> Result<Vec<u8>, String> {
    let before = fs::metadata(path).map_err(|error| error.to_string())?;
    if before.len() > byte_limit {
        return Err("file exceeds per-file byte limit".to_owned());
    }
    let capacity = usize::try_from(before.len()).map_err(|error| error.to_string())?;
    let mut bytes = Vec::with_capacity(capacity);
    fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(byte_limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let count = u64::try_from(bytes.len()).map_err(|error| error.to_string())?;
    let after = fs::metadata(path).map_err(|error| error.to_string())?;
    if count > byte_limit {
        return Err("file grew beyond per-file byte limit".to_owned());
    }
    if before.len() != count || after.len() != count {
        return Err("file changed while being read".to_owned());
    }
    Ok(bytes)
}

fn tree_digest(files: &[ClassifiedFile]) -> String {
    let mut digest = Sha256::default();
    for file in files {
        digest.update(display_relative(&file.path).as_bytes());
        digest.update(b"\0");
        digest.update(file.content_digest.as_bytes());
        digest.update(b"\0");
        match file.classification.owner {
            SourceOwner::Product(role) => {
                digest.update(b"product:");
                digest.update(role.as_str().as_bytes());
            }
            SourceOwner::RepositoryTooling => digest.update(b"repository_tooling"),
        }
        digest.update(b"\0");
        digest.update(file.classification.provenance.as_str().as_bytes());
        digest.update(b"\0");
        digest.update(file.classification.source_use.as_str().as_bytes());
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

#[allow(clippy::needless_pass_by_value)]
fn io_finding(code: &'static str, path: &Path, error: io::Error) -> Finding {
    Finding::new("path", code, Some(display_relative(path)), error.to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use reovim_depgraph::{SourceClassification, SourceOwner, SourceProvenance, SourceUse};

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
    fn generated_mechanism_is_closed_to_root_lockfile() {
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
        assert!(super::generated_mechanism_supported(&generated("Cargo.lock")));
        assert!(!super::generated_mechanism_supported(&generated("src/generated.json")));
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
        let (_, dynamic) = super::obvious_local_inputs("CARGO_HOME=\"$ROOT/tmp\" cargo --version");
        assert!(!dynamic);
        let (_, dynamic) = super::obvious_local_inputs(
            "ROOT=\"$(cd \"$(dirname \"${BASH_SOURCE[0]}\")/..\" && pwd)\"",
        );
        assert!(!dynamic);

        let (_, dynamic) =
            super::obvious_local_inputs("VALUES=(\n  \"$(\"$ROOT/tmp/hidden\")\"\n)\n");
        assert!(dynamic);
        let (_, dynamic) = super::obvious_local_inputs("cat <<EOF\n$(\"$ROOT/tmp/hidden\")\nEOF\n");
        assert!(dynamic);
        let (_, dynamic) =
            super::obvious_local_inputs("cat <<'EOF'\n$(\"$ROOT/tmp/hidden\")\nEOF\n");
        assert!(!dynamic);
    }

    #[test]
    fn current_entrypoint_command_positions_use_supported_forms() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        for path in [
            "scripts/check.sh",
            "scripts/check-documentation.sh",
            "scripts/check-conformance.sh",
            ".github/workflows/ci.yml",
        ] {
            let contents = std::fs::read_to_string(root.join(path)).unwrap();
            let (_, dynamic) = super::obvious_local_inputs(&contents);
            assert!(!dynamic, "{path}");
        }
    }
}
