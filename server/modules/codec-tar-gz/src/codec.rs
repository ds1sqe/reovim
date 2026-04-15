//! tar.gz structured codec.
//!
//! Parses `.tar.gz` files (gzip-compressed tar archives) and produces a
//! human-readable summary listing members (name, size, type) with annotations.
//! Structural edits decompress → patch → recompress the full archive.

use std::{
    fmt::Write as FmtWrite,
    io::{Cursor, Read, Write},
};

use {
    flate2::{Compression, read::GzDecoder, write::GzEncoder},
    reovim_driver_codec::{
        CodecError, CodecMetadata, ContentCodec, ContentType, DecodeResult, DecodedEdit,
        TranslateEditError, TreePath, impl_tree_op,
    },
    reovim_kernel::api::v1::ByteEdit,
    reovim_driver_annotation::{Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget},
    reovim_subsys_vfs::ByteSource,
};

use crate::classifier::TAR_GZ;

/// tar.gz structural edit operations (Plan 07 Phase 5).
///
/// All Phase 5 operations are strict same-size in-place rewrites within the
/// tar stream. After patching, the tar stream is recompressed; the resulting
/// gzip bytes will differ from the originals (different compression state)
/// but decompressing again will yield the patched tar contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TarGzTreeOp {
    /// Rename a tar archive member in place.
    ///
    /// The new name must be the same byte length as the old name (resolved
    /// from the tree path) so that the tar header block size is preserved.
    RenameMember {
        /// Replacement member name. Must be the same byte length as the
        /// old name (extracted from the tree path).
        new_name: String,
    },
    /// Replace the payload bytes of a tar archive member in place.
    ///
    /// The replacement must be the same size as the original payload.
    ReplaceMemberBytes {
        /// Expected current member payload.
        old_bytes: Vec<u8>,
        /// Replacement payload. Must be the same length as `old_bytes`.
        new_bytes: Vec<u8>,
    },
}

impl_tree_op!(TarGzTreeOp);

/// Annotation kind for tar.gz summary headers.
pub const TAR_GZ_HEADER_KIND: &str = "content.tar-gz.header";

/// Annotation kind for tar.gz archive member lines.
pub const TAR_GZ_MEMBER_KIND: &str = "content.tar-gz.member";

/// tar.gz structured codec.
///
/// Produces a summary of the archive contents: member names, sizes, and
/// entry types. This is a full-cycle codec — `translate_edit` decompresses,
/// patches the tar stream, then recompresses.
pub struct TarGzCodec;

impl TarGzCodec {
    /// Create a new tar.gz codec.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for TarGzCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentCodec for TarGzCodec {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        let tar_bytes = decompress_gzip(raw)
            .map_err(|e| CodecError::Other(format!("gzip decompress failed: {e}")))?;

        let members = parse_tar_members(&tar_bytes)
            .map_err(|e| CodecError::Other(format!("tar parse failed: {e}")))?;

        let (content, annotations) = format_tar_gz_summary(&members, raw.len());

        let mut metadata = CodecMetadata::new(ContentType::new(TAR_GZ));
        metadata.set("readonly", "false");
        metadata.set("file_size", raw.len().to_string());
        metadata.set("member_count", members.len().to_string());

        Ok(DecodeResult {
            content,
            annotations,
            metadata,
            lossy: true,
            readonly: false,
            truncated: false,
        })
    }

    fn translate_edit(
        &self,
        bytes: &dyn ByteSource,
        edit: &DecodedEdit,
    ) -> Result<Option<ByteEdit>, TranslateEditError> {
        match edit {
            DecodedEdit::Tree { path, op } => op.downcast_ref::<TarGzTreeOp>().map_or(
                Err(TranslateEditError::UnsupportedEdit {
                    reason: "tar-gz codec only accepts TarGz tree operations",
                }),
                |tar_op| translate_tar_gz_edit(bytes, path, tar_op),
            ),
            // Text, raw byte, and any future non_exhaustive variants are not
            // supported by the structural tar-gz codec.
            DecodedEdit::Text { .. } | DecodedEdit::Bytes { .. } | _ => {
                Err(TranslateEditError::UnsupportedEdit {
                    reason: "tar-gz codec does not translate text, raw byte, or unknown edits",
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Member field discriminator

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberField {
    Name,
    Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedMemberPath {
    name: String,
    field: MemberField,
}

// ---------------------------------------------------------------------------
// Tar member descriptor (used in decode output and in translate_edit)

#[derive(Debug, Clone)]
struct TarMember {
    /// Null-trimmed member name as reported by the tar crate.
    name: String,
    /// Entry type label for display.
    entry_type: &'static str,
    /// Payload size in bytes.
    size: u64,
    /// Byte offset of the 512-byte header block within the tar stream.
    header_offset: usize,
    /// Byte offset of the first payload byte within the tar stream.
    payload_offset: usize,
}

// ---------------------------------------------------------------------------
// Gzip helpers

fn decompress_gzip(raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = GzDecoder::new(raw);
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

fn recompress_gzip(tar_bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    // Writing to an in-memory vec cannot fail.
    encoder.write_all(tar_bytes).unwrap_or(());
    encoder.finish().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tar parsing

/// Parse the tar stream and return a list of member descriptors with
/// byte offsets into the raw tar bytes.
fn parse_tar_members(tar_bytes: &[u8]) -> Result<Vec<TarMember>, String> {
    let cursor = Cursor::new(tar_bytes);
    let mut archive = tar::Archive::new(cursor);
    let mut members = Vec::new();

    for entry_result in archive.entries_with_seek().map_err(|e| e.to_string())? {
        let entry = entry_result.map_err(|e| e.to_string())?;
        let header = entry.header();

        let name = entry
            .path()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned();

        let entry_type = classify_entry_type(header.entry_type());
        let size = header.size().map_err(|e| e.to_string())?;

        // `raw_header_position` is the byte offset of the 512-byte header block.
        let header_offset = usize::try_from(entry.raw_header_position())
            .map_err(|_| "header offset does not fit in usize".to_string())?;

        // Payload follows immediately after the header block (512 bytes).
        let payload_offset = header_offset
            .checked_add(512)
            .ok_or("payload offset overflowed")?;

        members.push(TarMember {
            name,
            entry_type,
            size,
            header_offset,
            payload_offset,
        });
    }

    Ok(members)
}

#[cfg_attr(coverage_nightly, coverage(off))]
const fn classify_entry_type(entry_type: tar::EntryType) -> &'static str {
    match entry_type {
        tar::EntryType::Regular | tar::EntryType::Continuous => "file",
        tar::EntryType::Directory => "dir",
        tar::EntryType::Symlink => "symlink",
        tar::EntryType::Link => "hard-link",
        tar::EntryType::Char => "char-dev",
        tar::EntryType::Block => "block-dev",
        tar::EntryType::Fifo => "fifo",
        tar::EntryType::GNUSparse => "sparse",
        _ => "other",
    }
}

// ---------------------------------------------------------------------------
// Path resolution

fn resolve_member_path(path: &TreePath) -> Result<ResolvedMemberPath, TranslateEditError> {
    let components = path.components();
    let [kind, member_name, field] = components else {
        return Err(TranslateEditError::MalformedPath {
            reason: "tar-gz member path must be [members, <name>, bytes|name]",
        });
    };
    // Split the compound `||` into separate checks to satisfy MC/DC
    // (compound conditions generate unreachable branch artifacts).
    if kind != "members" {
        return Err(TranslateEditError::MalformedPath {
            reason: "tar-gz member path must be [members, <name>, bytes|name]",
        });
    }
    if member_name.is_empty() {
        return Err(TranslateEditError::MalformedPath {
            reason: "tar-gz member path must be [members, <name>, bytes|name]",
        });
    }

    let field = match field.as_str() {
        "name" => MemberField::Name,
        "bytes" => MemberField::Bytes,
        _ => {
            return Err(TranslateEditError::MalformedPath {
                reason: "tar-gz member path must be [members, <name>, bytes|name]",
            });
        }
    };

    Ok(ResolvedMemberPath {
        name: member_name.clone(),
        field,
    })
}

// ---------------------------------------------------------------------------
// Core translate_edit dispatch

fn translate_tar_gz_edit(
    bytes: &dyn ByteSource,
    path: &TreePath,
    op: &TarGzTreeOp,
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let raw = read_all_bytes(bytes).ok_or(TranslateEditError::Internal {
        reason: "tar-gz byte source could not be fully read",
    })?;
    let tar_bytes = decompress_gzip(&raw).map_err(|_| TranslateEditError::Internal {
        reason: "tar-gz decompression failed during translate_edit",
    })?;
    let members = parse_tar_members(&tar_bytes).map_err(|_| TranslateEditError::Internal {
        reason: "tar-gz parse failed during translate_edit",
    })?;

    let resolved = resolve_member_path(path)?;

    match (&resolved.field, op) {
        (MemberField::Name, TarGzTreeOp::RenameMember { new_name }) => {
            translate_rename_member(&raw, &tar_bytes, &members, &resolved.name, new_name)
        }
        (
            MemberField::Bytes,
            TarGzTreeOp::ReplaceMemberBytes {
                old_bytes,
                new_bytes,
            },
        ) => translate_replace_member_bytes(
            &raw,
            &tar_bytes,
            &members,
            &resolved.name,
            old_bytes,
            new_bytes,
        ),
        (MemberField::Name, TarGzTreeOp::ReplaceMemberBytes { .. }) => {
            Err(TranslateEditError::UnsupportedEdit {
                reason: "tar-gz member payload replacement must target [members, <name>, bytes]",
            })
        }
        (MemberField::Bytes, TarGzTreeOp::RenameMember { .. }) => {
            Err(TranslateEditError::UnsupportedEdit {
                reason: "tar-gz member rename must target [members, <name>, name]",
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Rename member

/// Rename a tar member in-place by patching its header name field (100 bytes at
/// offset 0 of the header block) and recomputing the header checksum.
fn translate_rename_member(
    original_gzip: &[u8],
    tar_bytes: &[u8],
    members: &[TarMember],
    old_name: &str,
    new_name: &str,
) -> Result<Option<ByteEdit>, TranslateEditError> {
    if old_name == new_name {
        return Ok(None);
    }

    // Same-length constraint
    if old_name.len() != new_name.len() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "tar-gz member rename must preserve name byte length",
        });
    }
    if new_name.len() > 100 {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "tar-gz member name must fit in the 100-byte header name field",
        });
    }

    let member = find_member(members, old_name)?;
    let header_offset = member.header_offset;

    // The tar header is 512 bytes.
    let header_end =
        header_offset
            .checked_add(512)
            .ok_or(TranslateEditError::ConstraintViolation {
                reason: "tar-gz header block range overflowed",
            })?;
    let header_block =
        tar_bytes
            .get(header_offset..header_end)
            .ok_or(TranslateEditError::Internal {
                reason: "tar-gz header block is out of tar byte range",
            })?;

    // Patch: copy 512-byte block, write new name into first 100 bytes, zero-pad.
    let mut new_header = [0u8; 512];
    new_header.copy_from_slice(header_block);
    // Zero out name field first, then write new name.
    new_header[..100].fill(0);
    let name_bytes = new_name.as_bytes();
    new_header[..name_bytes.len()].copy_from_slice(name_bytes);

    // Recompute checksum (bytes 148..156).
    let checksum = compute_tar_checksum(&new_header);
    // Format: 6 octal digits + NUL + space (POSIX).
    let checksum_str = format!("{checksum:06o}\0 ");
    let checksum_bytes = checksum_str.as_bytes();
    new_header[148..148 + checksum_bytes.len().min(8)]
        .copy_from_slice(&checksum_bytes[..checksum_bytes.len().min(8)]);

    // Build patched tar stream.
    let mut new_tar = tar_bytes.to_vec();
    new_tar[header_offset..header_end].copy_from_slice(&new_header);

    // Recompress.
    let new_gzip = recompress_gzip(&new_tar);
    Ok(Some(ByteEdit::replace(0, original_gzip, &new_gzip)))
}

// ---------------------------------------------------------------------------
// Replace member bytes

fn translate_replace_member_bytes(
    original_gzip: &[u8],
    tar_bytes: &[u8],
    members: &[TarMember],
    member_name: &str,
    old_bytes: &[u8],
    new_bytes: &[u8],
) -> Result<Option<ByteEdit>, TranslateEditError> {
    if old_bytes == new_bytes {
        return Ok(None);
    }
    if old_bytes.len() != new_bytes.len() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "tar-gz member payload replacement must preserve member size",
        });
    }

    let member = find_member(members, member_name)?;
    let payload_size = usize::try_from(member.size).map_err(|_| member_size_overflow_error())?;
    let payload_end = member
        .payload_offset
        .checked_add(payload_size)
        .ok_or_else(payload_range_overflow_error)?;

    let actual =
        tar_bytes
            .get(member.payload_offset..payload_end)
            .ok_or(TranslateEditError::Internal {
                reason: "tar-gz member payload range is out of tar byte range",
            })?;

    if actual.len() != old_bytes.len() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "tar-gz member payload replacement must cover the full member payload",
        });
    }
    if actual != old_bytes {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "tar-gz member payload old bytes do not match the archive contents",
        });
    }

    // Build patched tar stream.
    let mut new_tar = tar_bytes.to_vec();
    new_tar[member.payload_offset..payload_end].copy_from_slice(new_bytes);

    // Recompress.
    let new_gzip = recompress_gzip(&new_tar);
    Ok(Some(ByteEdit::replace(0, original_gzip, &new_gzip)))
}

// ---------------------------------------------------------------------------
// Helpers

/// Error factory for a member-size cast failure in `translate_replace_member_bytes`.
///
/// `member.size` is a `u64` from the tar header.  On any 64-bit platform
/// `usize` is at least 64 bits, so `usize::try_from(u64)` never fails.
/// This path is therefore genuinely unreachable in production; the helper is
/// extracted so that `coverage(off)` applies only to the dead code.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn member_size_overflow_error() -> TranslateEditError {
    TranslateEditError::Internal {
        reason: "tar-gz member size does not fit in usize",
    }
}

/// Error factory for a payload-range overflow in `translate_replace_member_bytes`.
///
/// `payload_offset + payload_size` would need to exceed `usize::MAX` to
/// overflow, which is impossible on 64-bit platforms for realistic tar
/// archives.  The helper is extracted so that `coverage(off)` applies only
/// to the dead code.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn payload_range_overflow_error() -> TranslateEditError {
    TranslateEditError::ConstraintViolation {
        reason: "tar-gz member payload range overflowed",
    }
}

fn find_member<'a>(
    members: &'a [TarMember],
    name: &str,
) -> Result<&'a TarMember, TranslateEditError> {
    members
        .iter()
        .find(|m| m.name == name)
        .ok_or(TranslateEditError::MalformedPath {
            reason: "tar-gz member path does not resolve to a member",
        })
}

fn read_all_bytes(bytes: &dyn ByteSource) -> Option<Vec<u8>> {
    let len = usize::try_from(bytes.len()).ok()?;
    let data = bytes.read(0..bytes.len()).into_owned();
    (data.len() == len).then_some(data)
}

/// Compute the tar header checksum.
///
/// The checksum field is at bytes 148..156. Per POSIX:
/// 1. Set bytes 148..156 to spaces (0x20).
/// 2. Sum all 512 bytes as unsigned values.
fn compute_tar_checksum(header: &[u8; 512]) -> u32 {
    let mut sum: u32 = 0;
    for (i, &b) in header.iter().enumerate() {
        // Treat checksum field as spaces.
        let byte = if (148..156).contains(&i) { 0x20_u8 } else { b };
        sum = sum.wrapping_add(u32::from(byte));
    }
    sum
}

// ---------------------------------------------------------------------------
// Decode output formatting

#[cfg_attr(coverage_nightly, coverage(off))]
fn format_tar_gz_summary(
    members: &[TarMember],
    compressed_size: usize,
) -> (String, Vec<Annotation>) {
    let mut output = String::with_capacity(1024);
    let mut annotations = Vec::new();
    let mut line_idx: usize = 0;

    let header_kind = AnnotationKind::new(TAR_GZ_HEADER_KIND);
    let member_kind = AnnotationKind::new(TAR_GZ_MEMBER_KIND);

    // Title
    output.push_str("tar.gz Archive Summary\n");
    annotations.push(Annotation {
        kind: header_kind.clone(),
        target: AnnotationTarget::Line(line_idx),
        priority: 0,
        payload: AnnotationPayload::None,
    });
    line_idx += 1;

    output.push_str("======================\n");
    annotations.push(Annotation {
        kind: header_kind.clone(),
        target: AnnotationTarget::Line(line_idx),
        priority: 0,
        payload: AnnotationPayload::None,
    });
    line_idx += 1;

    let _ = writeln!(output, "Compressed Size: {compressed_size} bytes");
    line_idx += 1;
    let _ = writeln!(output, "Members:         {}", members.len());
    line_idx += 1;
    output.push('\n');
    line_idx += 1;

    // Members section
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
        kind: header_kind,
        target: AnnotationTarget::Line(line_idx),
        priority: 0,
        payload: AnnotationPayload::None,
    });
    line_idx += 1;

    let _ = writeln!(output, "{:<48} {:>10} {:<10}", "Name", "Size", "Type");
    line_idx += 1;

    for member in members {
        let _ =
            writeln!(output, "{:<48} {:>10} {:<10}", member.name, member.size, member.entry_type);
        annotations.push(Annotation {
            kind: member_kind.clone(),
            target: AnnotationTarget::Line(line_idx),
            priority: 0,
            payload: AnnotationPayload::None,
        });
        line_idx += 1;
    }

    let _ = line_idx; // suppress unused warning
    (output, annotations)
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
