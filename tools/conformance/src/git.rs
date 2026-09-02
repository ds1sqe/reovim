//! Exact-revision and clean-tree proof through the bounded process runner.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    path::{Path, PathBuf},
};

use crate::{
    digest,
    process::{
        self, CommandSpec, ProcessLimits, ProcessResult, TerminationReason,
    },
};

#[derive(Debug)]
pub struct Evidence {
    pub version: String,
    pub version_digest: String,
    pub revision: String,
    pub tracked_paths: BTreeSet<PathBuf>,
    pub tree_digest: String,
    pub object_format: String,
    pub tree_entries: BTreeMap<PathBuf, TreeEntry>,
    pub processes: Vec<(&'static str, ProcessResult)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: String,
    pub kind: String,
    pub object_id: String,
}

#[derive(Debug)]
pub struct GitError {
    pub code: &'static str,
    pub message: String,
}

impl std::fmt::Display for GitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for GitError {}

#[allow(clippy::too_many_lines)]
pub fn prove(
    git: &Path,
    root: &Path,
    expected_revision: &str,
    limits: ProcessLimits,
) -> Result<Evidence, GitError> {
    if !valid_revision(expected_revision) {
        return Err(GitError {
            code: "GIT_EXPECTED_REVISION",
            message: "expected revision must be 40 or 64 lowercase hex digits"
                .to_owned(),
        });
    }
    let mut processes = Vec::new();
    let version_process = invoke(git, root, ["--version"], limits)?;
    require_success("GIT_VERSION", &version_process)?;
    let version = output_line("GIT_VERSION", &version_process)?;
    let version_digest = digest::bytes(&version_process.stdout);
    processes.push(("git_version", version_process));

    let top_process =
        invoke(git, root, ["rev-parse", "--show-toplevel"], limits)?;
    require_success("GIT_ROOT", &top_process)?;
    let top = std::path::PathBuf::from(output_line("GIT_ROOT", &top_process)?);
    let canonical_top = top.canonicalize().map_err(|error| GitError {
        code: "GIT_ROOT_CANONICAL",
        message: error.to_string(),
    })?;
    if canonical_top != root {
        return Err(GitError {
            code: "GIT_ROOT_MISMATCH",
            message: format!(
                "Git root {} differs from {}",
                canonical_top.display(),
                root.display()
            ),
        });
    }
    processes.push(("git_root", top_process));

    let head_process = invoke(git, root, ["rev-parse", "HEAD"], limits)?;
    require_success("GIT_HEAD", &head_process)?;
    let revision = output_line("GIT_HEAD", &head_process)?;
    if revision != expected_revision {
        return Err(GitError {
            code: "GIT_REVISION_MISMATCH",
            message: format!("expected {expected_revision}, got {revision}"),
        });
    }
    processes.push(("git_head", head_process));

    let object_format_process =
        invoke(git, root, ["rev-parse", "--show-object-format"], limits)?;
    require_success("GIT_OBJECT_FORMAT", &object_format_process)?;
    let object_format =
        output_line("GIT_OBJECT_FORMAT", &object_format_process)?;
    if !matches!(object_format.as_str(), "sha1" | "sha256") {
        return Err(GitError {
            code: "GIT_OBJECT_FORMAT",
            message: format!("unsupported object format {object_format:?}"),
        });
    }
    processes.push(("git_object_format", object_format_process));

    let tree_process = invoke(
        git,
        root,
        ["ls-tree", "-r", "-z", "--full-tree", "HEAD"],
        limits,
    )?;
    require_success("GIT_LS_TREE", &tree_process)?;
    let tree_entries = parse_tree(&tree_process.stdout, &object_format)?;
    if let Some(forbidden) = tree_entries.keys().find(|path| {
        path.file_name().is_some_and(|name| name == ".gitattributes")
    }) {
        return Err(GitError {
            code: "GIT_UNMODELED_SEMANTICS",
            message: format!("HEAD contains unmodeled {}", forbidden.display()),
        });
    }
    for forbidden in [Path::new(".gitattributes"), Path::new(".gitmodules")] {
        if forbidden == Path::new(".gitmodules")
            && tree_entries.contains_key(forbidden)
        {
            return Err(GitError {
                code: "GIT_UNMODELED_SEMANTICS",
                message: format!(
                    "HEAD contains unmodeled {}",
                    forbidden.display()
                ),
            });
        }
        match std::fs::symlink_metadata(root.join(forbidden)) {
            Ok(_) => {
                return Err(GitError {
                    code: "GIT_UNMODELED_SEMANTICS",
                    message: format!(
                        "worktree contains unmodeled {}",
                        forbidden.display()
                    ),
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(GitError {
                    code: "GIT_UNMODELED_SEMANTICS_IO",
                    message: error.to_string(),
                });
            }
        }
    }
    let tracked_paths = tree_entries.keys().cloned().collect();
    let tree_digest = digest::bytes(&tree_process.stdout);
    processes.push(("git_ls_tree", tree_process));

    let index_flags_process =
        invoke(git, root, ["ls-files", "--stage", "-v", "-f", "-z"], limits)?;
    require_success("GIT_INDEX_FLAGS", &index_flags_process)?;
    if parse_index_entries(&index_flags_process.stdout, &object_format)?
        != tree_entries
    {
        return Err(GitError {
            code: "GIT_INDEX_TREE_MISMATCH",
            message:
                "raw stage-0 index modes/object IDs/paths differ from HEAD tree"
                    .to_owned(),
        });
    }
    processes.push(("git_index_entries", index_flags_process));

    let info_attributes_process = invoke(
        git,
        root,
        ["rev-parse", "--git-path", "info/attributes"],
        limits,
    )?;
    require_success("GIT_INFO_ATTRIBUTES_PATH", &info_attributes_process)?;
    let info_attributes = PathBuf::from(output_line(
        "GIT_INFO_ATTRIBUTES_PATH",
        &info_attributes_process,
    )?);
    let info_attributes = if info_attributes.is_absolute() {
        info_attributes
    } else {
        root.join(info_attributes)
    };
    match std::fs::symlink_metadata(&info_attributes) {
        Ok(_) => {
            return Err(GitError {
                code: "GIT_INFO_ATTRIBUTES",
                message: "repository info/attributes is not admitted"
                    .to_owned(),
            });
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(GitError {
                code: "GIT_INFO_ATTRIBUTES_IO",
                message: error.to_string(),
            });
        }
    }
    processes.push(("git_info_attributes_path", info_attributes_process));

    Ok(Evidence {
        version,
        version_digest,
        revision,
        tracked_paths,
        tree_digest,
        object_format,
        tree_entries,
        processes,
    })
}

fn parse_tree(
    bytes: &[u8],
    object_format: &str,
) -> Result<BTreeMap<PathBuf, TreeEntry>, GitError> {
    if !bytes.is_empty() && bytes.last() != Some(&0) {
        return Err(GitError {
            code: "GIT_LS_TREE_SHAPE",
            message: "NUL-delimited path output lacks its final delimiter"
                .to_owned(),
        });
    }
    let mut entries = BTreeMap::new();
    let object_id_len = if object_format == "sha1" { 40 } else { 64 };
    for raw in
        bytes.split(|byte| *byte == 0).filter(|record| !record.is_empty())
    {
        let record = std::str::from_utf8(raw).map_err(|error| GitError {
            code: "GIT_LS_TREE_UTF8",
            message: error.to_string(),
        })?;
        let (header, path) =
            record.split_once('\t').ok_or_else(|| GitError {
                code: "GIT_LS_TREE_SHAPE",
                message: "tree record lacks tab separator".to_owned(),
            })?;
        let mut header = header.split(' ');
        let (Some(mode), Some(kind), Some(object_id), None) =
            (header.next(), header.next(), header.next(), header.next())
        else {
            return Err(GitError {
                code: "GIT_LS_TREE_SHAPE",
                message: "tree record header is malformed".to_owned(),
            });
        };
        if !matches!(mode, "100644" | "100755") || kind != "blob" {
            return Err(GitError {
                code: "GIT_LS_TREE_KIND",
                message: format!("unadmitted tree entry {mode} {kind} {path}"),
            });
        }
        if object_id.len() != object_id_len
            || !object_id.bytes().all(|byte| {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            })
        {
            return Err(GitError {
                code: "GIT_LS_TREE_OBJECT_ID",
                message: format!("invalid object id for {path}"),
            });
        }
        let path = crate::policy::relative_path(path, "$git.ls_tree", false)
            .map_err(|error| GitError {
                code: "GIT_LS_TREE_PATH",
                message: error.to_string(),
            })?;
        if entries
            .insert(
                path,
                TreeEntry {
                    mode: mode.to_owned(),
                    kind: kind.to_owned(),
                    object_id: object_id.to_owned(),
                },
            )
            .is_some()
        {
            return Err(GitError {
                code: "GIT_LS_TREE_DUPLICATE",
                message: "duplicate tracked path".to_owned(),
            });
        }
    }
    Ok(entries)
}

fn parse_index_entries(
    bytes: &[u8],
    object_format: &str,
) -> Result<BTreeMap<PathBuf, TreeEntry>, GitError> {
    if !bytes.is_empty() && bytes.last() != Some(&0) {
        return Err(GitError {
            code: "GIT_INDEX_FLAGS_SHAPE",
            message: "NUL-delimited index output lacks its final delimiter"
                .to_owned(),
        });
    }
    let mut entries = BTreeMap::new();
    let object_id_len = if object_format == "sha1" { 40 } else { 64 };
    for raw in
        bytes.split(|byte| *byte == 0).filter(|record| !record.is_empty())
    {
        if raw.len() < 3 || raw[0] != b'H' || raw[1] != b' ' {
            return Err(GitError {
                code: "GIT_INDEX_FLAGS",
                message: concat!(
                    "index contains assume-unchanged, skip-worktree, ",
                    "conflict, or unknown flag",
                )
                .to_owned(),
            });
        }
        let value =
            std::str::from_utf8(&raw[2..]).map_err(|error| GitError {
                code: "GIT_INDEX_FLAGS_UTF8",
                message: error.to_string(),
            })?;
        let (header, path) =
            value.split_once('\t').ok_or_else(|| GitError {
                code: "GIT_INDEX_FLAGS_SHAPE",
                message: "index record lacks tab separator".to_owned(),
            })?;
        let mut fields = header.split(' ');
        let (Some(mode), Some(object_id), Some(stage), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(GitError {
                code: "GIT_INDEX_FLAGS_SHAPE",
                message: "index record header is malformed".to_owned(),
            });
        };
        if !matches!(mode, "100644" | "100755") || stage != "0" {
            return Err(GitError {
                code: "GIT_INDEX_FLAGS",
                message: concat!(
                    "index contains an unadmitted mode or nonzero ",
                    "conflict stage",
                )
                .to_owned(),
            });
        }
        if object_id.len() != object_id_len
            || !object_id.bytes().all(|byte| {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            })
        {
            return Err(GitError {
                code: "GIT_INDEX_FLAGS_OBJECT_ID",
                message: format!("invalid index object ID for {path}"),
            });
        }
        let path = crate::policy::relative_path(path, "$git.index", false)
            .map_err(|error| GitError {
                code: "GIT_INDEX_FLAGS_PATH",
                message: error.to_string(),
            })?;
        if entries
            .insert(
                path,
                TreeEntry {
                    mode: mode.to_owned(),
                    kind: "blob".to_owned(),
                    object_id: object_id.to_owned(),
                },
            )
            .is_some()
        {
            return Err(GitError {
                code: "GIT_INDEX_FLAGS_DUPLICATE",
                message: "duplicate index path".to_owned(),
            });
        }
    }
    Ok(entries)
}

fn invoke<'a>(
    git: &Path,
    root: &Path,
    arguments: impl IntoIterator<Item = &'a str>,
    limits: ProcessLimits,
) -> Result<ProcessResult, GitError> {
    let spec = CommandSpec {
        program: git.to_path_buf(),
        arguments: [
            OsString::from("-c"),
            OsString::from("core.fsmonitor=false"),
            OsString::from("-c"),
            OsString::from("core.untrackedCache=false"),
            OsString::from("-c"),
            OsString::from("core.hooksPath=/dev/null"),
            OsString::from("-c"),
            OsString::from("core.fileMode=true"),
            OsString::from("-c"),
            OsString::from("core.autocrlf=false"),
            OsString::from("-c"),
            OsString::from("core.eol=lf"),
            OsString::from("-c"),
            OsString::from("core.ignoreStat=false"),
            OsString::from("-c"),
            OsString::from("core.ignoreCase=false"),
            OsString::from("-c"),
            OsString::from("core.attributesFile=/dev/null"),
        ]
        .into_iter()
        .chain(arguments.into_iter().map(OsString::from))
        .collect(),
        current_dir: root.to_path_buf(),
        environment: BTreeMap::from([
            (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
            (OsString::from("GIT_CONFIG_GLOBAL"), OsString::from("/dev/null")),
            (OsString::from("GIT_ATTR_NOSYSTEM"), OsString::from("1")),
            (OsString::from("GIT_NO_REPLACE_OBJECTS"), OsString::from("1")),
            (OsString::from("GIT_OPTIONAL_LOCKS"), OsString::from("0")),
            (OsString::from("LC_ALL"), OsString::from("C")),
        ]),
        clear_environment: true,
    };
    process::run(&spec, limits).map_err(|error| GitError {
        code: "GIT_RUN",
        message: error.to_string(),
    })
}

fn require_success(
    prefix: &'static str,
    process: &ProcessResult,
) -> Result<(), GitError> {
    require_bounded(prefix, process)?;
    if process.status.success() {
        Ok(())
    } else {
        Err(GitError {
            code: "GIT_NONZERO",
            message: format!("{prefix} exited with {}", process.status),
        })
    }
}

fn require_bounded(
    prefix: &'static str,
    process: &ProcessResult,
) -> Result<(), GitError> {
    if process.termination_reason != TerminationReason::Exited
        || process.stdout_overflow
        || process.stderr_overflow
        || process.stdout_read_error.is_some()
        || process.stderr_read_error.is_some()
    {
        Err(GitError {
            code: "GIT_PROCESS_BOUND",
            message: format!(
                "{prefix} failed bounded capture: {}",
                process.termination_reason.as_str()
            ),
        })
    } else if !process.stderr.is_empty() {
        Err(GitError {
            code: "GIT_UNEXPECTED_STDERR",
            message: format!(
                "{prefix} emitted {} stderr bytes",
                process.stderr.len()
            ),
        })
    } else {
        Ok(())
    }
}

fn output_line(
    prefix: &'static str,
    process: &ProcessResult,
) -> Result<String, GitError> {
    let value = std::str::from_utf8(&process.stdout).map_err(|error| {
        GitError { code: "GIT_UTF8", message: format!("{prefix}: {error}") }
    })?;
    let value = value.trim_end_matches(['\r', '\n']);
    if value.is_empty() || value.contains(['\r', '\n']) {
        Err(GitError {
            code: "GIT_OUTPUT_SHAPE",
            message: format!("{prefix} expected exactly one nonempty line"),
        })
    } else {
        Ok(value.to_owned())
    }
}

fn valid_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    const SHA1_OID: &str =
        concat!("01234567890123456789", "01234567890123456789",);
    const SHA256_OID: &str = concat!(
        "01234567890123456789012345678901",
        "23456789012345678901234567890123",
    );

    fn tree_entry(
        mode: &str,
        kind: &str,
        object_id: &str,
        path: &str,
    ) -> Vec<u8> {
        format!("{mode} {kind} {object_id}\t{path}\0").into_bytes()
    }

    #[test]
    fn full_tree_parser_checks_modes_kinds_and_object_lengths() {
        let sha1 = tree_entry("100644", "blob", SHA1_OID, "file.txt");
        let parsed = super::parse_tree(&sha1, "sha1").unwrap();
        assert_eq!(parsed[std::path::Path::new("file.txt")].mode, "100644");

        let sha256 = tree_entry("100755", "blob", SHA256_OID, "script");
        assert_eq!(super::parse_tree(&sha256, "sha256").unwrap().len(), 1);

        for invalid in [
            tree_entry("120000", "blob", SHA1_OID, "link"),
            tree_entry("160000", "commit", SHA1_OID, "submodule"),
            tree_entry("100664", "blob", SHA1_OID, "mode"),
        ] {
            let error = super::parse_tree(&invalid, "sha1").unwrap_err();
            assert_eq!(error.code, "GIT_LS_TREE_KIND");
        }
    }

    #[test]
    fn index_flag_parser_rejects_hidden_and_unknown_states() {
        let oid = "0123456789012345678901234567890123456789";
        let valid = format!("H 100644 {oid} 0\tfile\0");
        assert_eq!(
            super::parse_index_entries(valid.as_bytes(), "sha1").unwrap().len(),
            1
        );
        for flag in ['h', 'S', '?'] {
            let invalid = format!("{flag} 100644 {oid} 0\tfile\0");
            assert_eq!(
                super::parse_index_entries(invalid.as_bytes(), "sha1")
                    .unwrap_err()
                    .code,
                "GIT_INDEX_FLAGS"
            );
        }
        let staged = format!("H 100644 {oid} 2\tfile\0");
        assert_eq!(
            super::parse_index_entries(staged.as_bytes(), "sha1")
                .unwrap_err()
                .code,
            "GIT_INDEX_FLAGS"
        );
    }
}
