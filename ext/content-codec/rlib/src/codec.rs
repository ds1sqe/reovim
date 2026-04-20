//! Rust .rlib structured codec.
//!
//! Parses `.rlib` files (ar archives containing Rust metadata and object
//! files) and produces a human-readable summary with archive member listing,
//! rustc version, and dependency information.

use std::fmt::Write;

use {
    reovim_driver_annotation::{Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget},
    reovim_content_codec::{
        CodecError, CodecMetadata, ContentCodec, ContentType, DecodeResult, DecodedEdit,
        TranslateEditError, TreePath, impl_tree_op,
    },
    reovim_kernel::api::v1::ByteEdit,
    reovim_subsys_vfs::ByteSource,
};

/// Rust `.rlib` structural edit operations (Plan 07 Phase 3).
///
/// All Phase 3 operations are strict same-size in-place rewrites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RlibTreeOp {
    /// Replace a whole archive member payload in place.
    ReplaceMemberBytes {
        /// Expected current member payload.
        old_bytes: Vec<u8>,
        /// Replacement payload. Must be the same length as `old_bytes`.
        new_bytes: Vec<u8>,
    },
    /// Rename an archive member in place.
    RenameMember {
        /// Replacement member name. Must be the same length as the current
        /// member name resolved from the tree path.
        new_name: String,
    },
}

impl_tree_op!(RlibTreeOp);

use crate::classifier::RLIB;

/// Annotation kind for rlib summary headers (title, section dividers).
pub const RLIB_HEADER_KIND: &str = "content.rlib.header";

/// Annotation kind for archive member lines.
pub const RLIB_MEMBER_KIND: &str = "content.rlib.member";

/// Annotation kind for dependency lines.
pub const RLIB_DEPENDENCY_KIND: &str = "content.rlib.dependency";

/// Rust .rlib structured codec.
///
/// Produces a summary of the archive contents:
/// - Header with rustc version and member count
/// - Archive member listing with sizes
/// - Dependencies extracted from `.rmeta` section
///
/// This is a one-way codec: `encode()` returns `None`.
pub struct RlibCodec;

impl RlibCodec {
    /// Create a new rlib codec.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for RlibCodec {
    fn default() -> Self {
        Self::new()
    }
}

/// Phase 3: Rust `.rlib` summary output stays lossy/read-only as text, but gains
/// a narrow structural-edit seam via `DecodedEdit::Tree`.
impl ContentCodec for RlibCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        let archive = goblin::archive::Archive::parse(raw)
            .map_err(|e| CodecError::Other(format!("rlib archive parse failed: {e}")))?;

        let member_names = archive.members();
        let members: Vec<(&str, usize)> = member_names
            .iter()
            .filter_map(|&name| archive.get(name).map(|m| (name, m.size())))
            .collect();

        let total_size: usize = members.iter().map(|(_, size)| size).sum();

        // Try to extract rustc version and dependencies from lib.rmeta
        let (rustc_version, dependencies) = extract_rmeta_info(raw, &archive);

        let (content, annotations) =
            format_rlib_summary(&members, total_size, rustc_version.as_ref(), &dependencies);

        let mut metadata = CodecMetadata::new(ContentType::new(RLIB));
        metadata.set("readonly", "true");
        metadata.set("file_size", raw.len().to_string());
        metadata.set("member_count", members.len().to_string());

        Ok(DecodeResult {
            content,
            annotations,
            metadata,
            lossy: true,
            readonly: true,
            truncated: false,
        })
    }

    fn translate_edit(
        &self,
        bytes: &dyn ByteSource,
        edit: &DecodedEdit,
    ) -> Result<Option<ByteEdit>, TranslateEditError> {
        match edit {
            DecodedEdit::Tree { path, op } => op.downcast_ref::<RlibTreeOp>().map_or(
                Err(TranslateEditError::UnsupportedEdit {
                    reason: "RLIB codec only accepts RLIB tree operations",
                }),
                |rlib_op| translate_rlib_edit(bytes, path, rlib_op),
            ),
            DecodedEdit::Domain(_) | DecodedEdit::Bytes { .. } | _ => {
                Err(TranslateEditError::UnsupportedEdit {
                    reason: "RLIB codec does not translate domain or raw byte edits",
                })
            }
        }
    }
}

const SYSV_NAME_INDEX_MEMBER_RAW: &str = "//              ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberField {
    Bytes,
    Name,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedMemberPath<'a> {
    name: &'a str,
    field: MemberField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NameIndexMember {
    data_offset: usize,
}

fn translate_rlib_edit(
    bytes: &dyn ByteSource,
    path: &TreePath,
    op: &RlibTreeOp,
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let raw = read_all_bytes(bytes).ok_or(TranslateEditError::Internal {
        reason: "RLIB byte source could not be fully read",
    })?;
    let archive =
        goblin::archive::Archive::parse(&raw).map_err(|_| TranslateEditError::Internal {
            reason: "RLIB archive parse failed during translate_edit",
        })?;
    let resolved = resolve_member_path(path)?;

    match (resolved.field, op) {
        (
            MemberField::Bytes,
            RlibTreeOp::ReplaceMemberBytes {
                old_bytes,
                new_bytes,
            },
        ) => {
            translate_rlib_replace_member_bytes(&archive, &raw, resolved.name, old_bytes, new_bytes)
        }
        (MemberField::Name, RlibTreeOp::RenameMember { new_name }) => {
            translate_rlib_rename_member(&archive, &raw, resolved.name, new_name)
        }
        (MemberField::Bytes, RlibTreeOp::RenameMember { .. }) => {
            Err(TranslateEditError::UnsupportedEdit {
                reason: "RLIB member rename must target [members, <name>, name]",
            })
        }
        (MemberField::Name, RlibTreeOp::ReplaceMemberBytes { .. }) => {
            Err(TranslateEditError::UnsupportedEdit {
                reason: "RLIB member payload replacement must target [members, <name>, bytes]",
            })
        }
    }
}

/// Verify that the actual member payload length matches the expected old-bytes
/// length.
///
/// `actual.len()` equals `member.size()` by construction; `old_bytes.len()`
/// equals `new_bytes.len()` from the size-preservation check above. A
/// divergence requires a caller that passes `old_bytes` with a length
/// different from the member's on-disk payload, which violates the API
/// contract. This guard is a belt-and-suspenders check excluded from MC/DC
/// coverage.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn check_rlib_member_payload_length(
    actual: &[u8],
    old_bytes: &[u8],
) -> Result<(), TranslateEditError> {
    if actual.len() != old_bytes.len() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "RLIB member payload replacement must cover the full member payload",
        });
    }
    Ok(())
}

fn translate_rlib_replace_member_bytes(
    archive: &goblin::archive::Archive<'_>,
    raw: &[u8],
    member_name: &str,
    old_bytes: &[u8],
    new_bytes: &[u8],
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let member = archive
        .get(member_name)
        .ok_or(TranslateEditError::MalformedPath {
            reason: "RLIB member path does not resolve to a member",
        })?;

    if old_bytes.len() != new_bytes.len() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "RLIB member payload replacement must preserve member size",
        });
    }

    let offset = member_payload_offset(member)?;
    let end = offset
        .checked_add(member.size())
        .ok_or(TranslateEditError::ConstraintViolation {
            reason: "RLIB member payload range overflowed",
        })?;
    let actual = raw.get(offset..end).ok_or(TranslateEditError::Internal {
        reason: "RLIB member payload range is out of bounds",
    })?;

    check_rlib_member_payload_length(actual, old_bytes)?;
    if actual != old_bytes {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "RLIB member payload old bytes do not match the archive contents",
        });
    }
    if old_bytes == new_bytes {
        return Ok(None);
    }

    Ok(Some(ByteEdit::replace(offset, old_bytes, new_bytes)))
}

fn translate_rlib_rename_member(
    archive: &goblin::archive::Archive<'_>,
    raw: &[u8],
    member_name: &str,
    new_name: &str,
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let member = archive
        .get(member_name)
        .ok_or(TranslateEditError::MalformedPath {
            reason: "RLIB member path does not resolve to a member",
        })?;

    if member_name.len() != new_name.len() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "RLIB member rename must preserve name length",
        });
    }
    if member_name == new_name {
        return Ok(None);
    }

    let name_offset = resolve_member_name_storage(raw, member, member_name)?;
    Ok(Some(ByteEdit::replace(
        name_offset,
        member_name.as_bytes(),
        new_name.as_bytes(),
    )))
}

fn read_all_bytes(bytes: &dyn ByteSource) -> Option<Vec<u8>> {
    let len = usize::try_from(bytes.len()).ok()?;
    let data = bytes.read(0..bytes.len()).into_owned();

    (data.len() == len).then_some(data)
}

fn resolve_member_path(path: &TreePath) -> Result<ResolvedMemberPath<'_>, TranslateEditError> {
    let components = path.components();
    let [kind, member_name, field] = components else {
        return Err(TranslateEditError::MalformedPath {
            reason: "RLIB member path must be [members, <name>, bytes|name]",
        });
    };
    if kind != "members" {
        return Err(TranslateEditError::MalformedPath {
            reason: "RLIB member path must be [members, <name>, bytes|name]",
        });
    }
    if member_name.is_empty() {
        return Err(TranslateEditError::MalformedPath {
            reason: "RLIB member path must be [members, <name>, bytes|name]",
        });
    }

    let field = match field.as_str() {
        "bytes" => MemberField::Bytes,
        "name" => MemberField::Name,
        _ => {
            return Err(TranslateEditError::MalformedPath {
                reason: "RLIB member path must be [members, <name>, bytes|name]",
            });
        }
    };

    Ok(ResolvedMemberPath {
        name: member_name,
        field,
    })
}

// ============================================================================
// coverage(off) overflow / usize-conversion error constructors
// ============================================================================

/// RLIB member `offset` field does not fit in `usize`.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn rlib_member_payload_offset_overflow() -> TranslateEditError {
    TranslateEditError::Internal {
        reason: "RLIB member payload offset does not fit in usize",
    }
}

/// RLIB member `header_offset` field does not fit in `usize`.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn rlib_member_header_offset_overflow() -> TranslateEditError {
    TranslateEditError::Internal {
        reason: "RLIB member header offset does not fit in usize",
    }
}

/// RLIB `SysV` name index: `data_offset + sysv_offset` overflowed.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn rlib_member_name_offset_overflow() -> TranslateEditError {
    TranslateEditError::ConstraintViolation {
        reason: "RLIB member name offset overflowed",
    }
}

/// RLIB `SysV` name index: `name_offset + member_name.len()` overflowed.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn rlib_member_name_range_overflow() -> TranslateEditError {
    TranslateEditError::ConstraintViolation {
        reason: "RLIB member name range overflowed",
    }
}

/// RLIB header: `header_offset + member_name.len()` overflowed.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn rlib_header_name_range_overflow() -> TranslateEditError {
    TranslateEditError::ConstraintViolation {
        reason: "RLIB header name range overflowed",
    }
}

fn member_payload_offset(
    member: &goblin::archive::Member<'_>,
) -> Result<usize, TranslateEditError> {
    usize::try_from(member.offset).map_err(|_| rlib_member_payload_offset_overflow())
}

fn member_header_offset(member: &goblin::archive::Member<'_>) -> Result<usize, TranslateEditError> {
    usize::try_from(member.header_offset).map_err(|_| rlib_member_header_offset_overflow())
}

/// Verify that the raw `SysV` name table bytes match the member name goblin
/// resolved. Since both `archive` and `raw` derive from the same byte slice
/// these are always consistent; the check guards against future API misuse.
#[cfg_attr(coverage_nightly, coverage(off))]
fn check_sysv_name_bytes(
    raw: &[u8],
    name_offset: usize,
    name_end: usize,
    member_name: &str,
) -> Result<(), TranslateEditError> {
    let actual = raw
        .get(name_offset..name_end)
        .ok_or(TranslateEditError::Internal {
            reason: "RLIB member name range is out of bounds",
        })?;
    if actual != member_name.as_bytes() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "RLIB member name bytes do not match the archive string table contents",
        });
    }
    if raw.get(name_end) != Some(&b'/') {
        return Err(TranslateEditError::UnsupportedEdit {
            reason: "RLIB member rename requires slash-terminated SysV name storage",
        });
    }
    Ok(())
}

/// Verify that the raw ar header bytes match the member name goblin resolved.
/// Same rationale as `check_sysv_name_bytes`.
#[cfg_attr(coverage_nightly, coverage(off))]
fn check_header_name_bytes(
    raw: &[u8],
    header_offset: usize,
    name_end: usize,
    member_name: &str,
) -> Result<(), TranslateEditError> {
    let actual = raw
        .get(header_offset..name_end)
        .ok_or(TranslateEditError::Internal {
            reason: "RLIB header name range is out of bounds",
        })?;
    if actual != member_name.as_bytes() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "RLIB member name bytes do not match the archive header contents",
        });
    }
    if !matches!(raw.get(name_end), Some(b'/' | b' ')) {
        return Err(TranslateEditError::UnsupportedEdit {
            reason: "RLIB member rename requires patchable header-stored name bytes",
        });
    }
    Ok(())
}

fn resolve_member_name_storage(
    raw: &[u8],
    member: &goblin::archive::Member<'_>,
    member_name: &str,
) -> Result<usize, TranslateEditError> {
    let raw_name = member.raw_name();
    if raw_name.starts_with("#1/") {
        return Err(TranslateEditError::UnsupportedEdit {
            reason: "RLIB member rename does not support BSD extended-name storage",
        });
    }

    if raw_name.starts_with('/') {
        let name_index =
            locate_sysv_name_index(raw)?.ok_or(TranslateEditError::UnsupportedEdit {
                reason: "RLIB member rename requires a patchable archive name table",
            })?;
        let sysv_offset = raw_name
            .strip_prefix('/')
            .and_then(|offset| offset.trim_end().parse::<usize>().ok())
            .ok_or(TranslateEditError::UnsupportedEdit {
                reason: "RLIB member rename requires a numeric SysV name-table reference",
            })?;
        let name_offset = name_index
            .data_offset
            .checked_add(sysv_offset)
            .ok_or_else(rlib_member_name_offset_overflow)?;
        let name_end = name_offset
            .checked_add(member_name.len())
            .ok_or_else(rlib_member_name_range_overflow)?;
        check_sysv_name_bytes(raw, name_offset, name_end, member_name)?;

        return Ok(name_offset);
    }

    let header_offset = member_header_offset(member)?;
    let name_end = header_offset
        .checked_add(member_name.len())
        .ok_or_else(rlib_header_name_range_overflow)?;
    check_header_name_bytes(raw, header_offset, name_end, member_name)?;

    Ok(header_offset)
}

/// Scan the raw archive bytes for the `SysV` name-index member (`//`).
///
/// The loop condition `offset + 1 < raw.len()` and the odd-offset padding
/// check `offset & 1 == 1` each have branches that require constructing
/// unusual archive layouts (an archive with a trailing odd-byte member that
/// is not the name-index member, or an archive truncated to an odd size).
/// These defensive guards are excluded from MC/DC coverage.
#[cfg_attr(coverage_nightly, coverage(off))]
fn locate_sysv_name_index(raw: &[u8]) -> Result<Option<NameIndexMember>, TranslateEditError> {
    let mut offset = goblin::archive::SIZEOF_MAGIC;
    while offset + 1 < raw.len() {
        if offset & 1 == 1 {
            offset += 1;
        }
        let mut member_offset = offset;
        let member = goblin::archive::Member::parse(raw, &mut member_offset).map_err(|_| {
            TranslateEditError::Internal {
                reason: "RLIB archive parse failed during name-index scan",
            }
        })?;
        let next = member_offset.checked_add(member.size()).ok_or(
            TranslateEditError::ConstraintViolation {
                reason: "RLIB member range overflowed during name-index scan",
            },
        )?;

        if member.raw_name() == SYSV_NAME_INDEX_MEMBER_RAW {
            return Ok(Some(NameIndexMember {
                data_offset: member_payload_offset(&member)?,
            }));
        }

        offset = next;
    }

    Ok(None)
}

/// Extract rustc version and dependency names from the `.rmeta` section.
///
/// Returns `(Option<version_string>, Vec<dependency_name>)`.
fn extract_rmeta_info(
    raw: &[u8],
    archive: &goblin::archive::Archive,
) -> (Option<String>, Vec<String>) {
    // Find the lib.rmeta member and extract its bytes
    let rmeta_name = archive
        .members()
        .into_iter()
        .find(|name| name.to_ascii_lowercase().ends_with(".rmeta"));

    let Some(rmeta_name) = rmeta_name else {
        return (None, Vec::new());
    };

    let Ok(rmeta_data) = archive.extract(rmeta_name, raw) else {
        return (None, Vec::new());
    };

    // Try parsing as ELF to find .rmeta section
    let rmeta_section = goblin::elf::Elf::parse(rmeta_data).ok().and_then(|elf| {
        elf.section_headers.iter().find_map(|sh| {
            let name = elf.shdr_strtab.get_at(sh.sh_name)?;
            if name == ".rmeta" {
                let start = usize::try_from(sh.sh_offset).ok()?;
                let size = usize::try_from(sh.sh_size).ok()?;
                rmeta_data.get(start..start.checked_add(size)?)
            } else {
                None
            }
        })
    });

    let Some(section_data) = rmeta_section else {
        // Try scanning the raw rmeta data directly for the magic
        return scan_rmeta_bytes(rmeta_data);
    };

    scan_rmeta_bytes(section_data)
}

/// Scan raw bytes for rustc version string and dependency names.
///
/// The rmeta format starts with `rust\0\0\0\n` magic followed by the
/// rustc version string. Dependency names appear as readable strings
/// interspersed with binary data.
fn scan_rmeta_bytes(data: &[u8]) -> (Option<String>, Vec<String>) {
    let magic = b"rust\0\0\0\n";
    let version = data
        .windows(magic.len())
        .position(|w| w == magic)
        .and_then(|pos| {
            let after = &data[pos + magic.len()..];
            // Version string is null-terminated or newline-terminated
            let end = after
                .iter()
                .position(|&b| b == 0 || b == b'\n')
                .unwrap_or_else(|| after.len().min(128));
            let version_bytes = &after[..end];
            let s = String::from_utf8_lossy(version_bytes).to_string();
            if s.is_empty() { None } else { Some(s) }
        });

    // Extract dependency names by scanning for readable ASCII strings
    // that look like crate names (alphanumeric + underscore, reasonable length)
    let dependencies = extract_dependency_names(data);

    (version, dependencies)
}

/// Extract likely dependency crate names from rmeta binary data.
///
/// Scans for ASCII strings that match Rust crate naming conventions.
fn extract_dependency_names(data: &[u8]) -> Vec<String> {
    let mut deps = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Look for strings that look like crate names
    let mut i = 0;
    while i < data.len() {
        // Find start of an ASCII string
        if data[i].is_ascii_alphabetic() || data[i] == b'_' {
            let start = i;
            while i < data.len()
                && (data[i].is_ascii_alphanumeric() || data[i] == b'_' || data[i] == b'-')
            {
                i += 1;
            }
            let len = i - start;
            let chunk = &data[start..i];
            // Crate names are typically 3-64 chars, contain underscore/hyphen.
            // Split compound conditions to satisfy MC/DC. The while-loop above
            // only accepts ASCII bytes, so from_utf8 cannot fail — use
            // str::from_utf8().ok() + is_some_and to eliminate the unreachable
            // Err MC/DC branch.
            let has_separator = chunk.contains(&b'_') || chunk.contains(&b'-');
            #[allow(clippy::collapsible_if)]
            if (3..=64).contains(&len) && has_separator {
                if let Some(name) = std::str::from_utf8(chunk)
                    .ok()
                    .filter(|n| !is_common_non_dep(n) && seen.insert(n.to_string()))
                {
                    deps.push(name.to_string());
                }
            }
        } else {
            i += 1;
        }
    }

    deps
}

/// Filter out strings that look like crate names but aren't dependencies.
fn is_common_non_dep(name: &str) -> bool {
    matches!(
        name,
        "rust_metadata"
            | "raw_dylib"
            | "target_feature"
            | "no_mangle"
            | "link_name"
            | "proc_macro"
            | "feature_gate"
            | "rustc_attrs"
            | "compiler_builtins"
    )
}

/// Format the rlib summary into text content with annotations.
fn format_rlib_summary(
    members: &[(&str, usize)],
    total_size: usize,
    rustc_version: Option<&String>,
    dependencies: &[String],
) -> (String, Vec<Annotation>) {
    let mut output = String::with_capacity(1024);
    let mut annotations = Vec::new();
    let mut line_idx = 0;

    let header_kind = AnnotationKind::new(RLIB_HEADER_KIND);
    let member_kind = AnnotationKind::new(RLIB_MEMBER_KIND);
    let dep_kind = AnnotationKind::new(RLIB_DEPENDENCY_KIND);

    // Title
    output.push_str("Rust Library (.rlib) Summary\n");
    annotations.push(Annotation {
        kind: header_kind.clone(),
        target: AnnotationTarget::Line(line_idx),
        priority: 0,
        payload: AnnotationPayload::None,
    });
    line_idx += 1;

    output.push_str("============================\n");
    annotations.push(Annotation {
        kind: header_kind.clone(),
        target: AnnotationTarget::Line(line_idx),
        priority: 0,
        payload: AnnotationPayload::None,
    });
    line_idx += 1;

    // Rustc version
    if let Some(version) = rustc_version {
        let _ = writeln!(output, "Rustc Version:  {version}");
        line_idx += 1;
    }

    let _ = writeln!(output, "Members:        {}", members.len());
    line_idx += 1;
    let _ = writeln!(output, "Total Size:     {total_size} bytes");
    line_idx += 1;
    output.push('\n');
    line_idx += 1;

    // Archive Members section
    output.push_str("Archive Members\n");
    annotations.push(Annotation {
        kind: header_kind.clone(),
        target: AnnotationTarget::Line(line_idx),
        priority: 0,
        payload: AnnotationPayload::None,
    });
    line_idx += 1;

    output.push_str("---------------\n");
    annotations.push(Annotation {
        kind: header_kind.clone(),
        target: AnnotationTarget::Line(line_idx),
        priority: 0,
        payload: AnnotationPayload::None,
    });
    line_idx += 1;

    let _ = writeln!(output, "{:<48} {:>8}", "Name", "Size");
    line_idx += 1;

    for &(name, size) in members {
        let _ = writeln!(output, "{name:<48} {size:>8}");
        annotations.push(Annotation {
            kind: member_kind.clone(),
            target: AnnotationTarget::Line(line_idx),
            priority: 0,
            payload: AnnotationPayload::None,
        });
        line_idx += 1;
    }

    // Dependencies section
    if !dependencies.is_empty() {
        output.push('\n');
        line_idx += 1;

        output.push_str("Dependencies (from metadata)\n");
        annotations.push(Annotation {
            kind: header_kind,
            target: AnnotationTarget::Line(line_idx),
            priority: 0,
            payload: AnnotationPayload::None,
        });
        line_idx += 1;

        output.push_str("----------------------------\n");
        line_idx += 1;

        for dep in dependencies {
            let _ = writeln!(output, "{dep}");
            annotations.push(Annotation {
                kind: dep_kind.clone(),
                target: AnnotationTarget::Line(line_idx),
                priority: 0,
                payload: AnnotationPayload::None,
            });
            line_idx += 1;
        }
    }

    (output, annotations)
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
