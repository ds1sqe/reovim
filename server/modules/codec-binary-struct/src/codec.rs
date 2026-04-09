//! ELF and ZIP structured binary codecs.
//!
//! Produces human-readable summaries of ELF binaries and ZIP archives.
//! Both gain narrow Plan 07 structural-edit surfaces for in-place tree
//! edits: ELF in Phase 2, ZIP in Phase 4.

use std::fmt::Write;

use {
    reovim_driver_annotation::{Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget},
    reovim_driver_codec::{
        CodecError, CodecMetadata, ContentCodec, ContentType, DecodeResult, DecodedEdit,
        TranslateEditError, TreePath, impl_tree_op,
    },
    reovim_driver_vfs::ByteSource,
    reovim_kernel::api::v1::ByteEdit,
};

/// ELF structural edit operations (Plan 07 Phase 2).
///
/// All Phase 2 operations are strict same-size in-place rewrites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElfTreeOp {
    /// Patch a byte span inside an executable section in place.
    PatchBytes {
        /// Section-relative byte offset to replace.
        offset: usize,
        /// Expected current bytes at `offset`.
        old_bytes: Vec<u8>,
        /// Replacement bytes. Must be the same length as `old_bytes`.
        new_bytes: Vec<u8>,
    },
    /// Rename a symbol in place.
    RenameSymbol {
        /// Replacement symbol name. Must be the same length as the
        /// current symbol name resolved from the tree path.
        new_name: String,
    },
    /// Replace an entire named section payload in place.
    ReplaceSectionBytes {
        /// Expected current section payload.
        old_bytes: Vec<u8>,
        /// Replacement section payload. Must be the same length as
        /// `old_bytes`.
        new_bytes: Vec<u8>,
    },
}

impl_tree_op!(ElfTreeOp);

/// ZIP structural edit operations (Plan 07 Phase 4).
///
/// All Phase 4 operations are strict same-size in-place rewrites.
/// No archive rebuild, recompression, or layout change is supported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZipTreeOp {
    /// Rewrite the archive comment in place with same-length bytes.
    ReplaceComment {
        /// Replacement comment bytes. Must be the same length as the
        /// current archive comment.
        new_comment: Vec<u8>,
    },
    /// Replace a STORED entry payload in place with same-size bytes.
    ReplaceEntryBytes {
        /// Expected current entry payload.
        old_bytes: Vec<u8>,
        /// Replacement payload. Must be the same length as `old_bytes`.
        new_bytes: Vec<u8>,
    },
    /// Rename an entry in place (both local header and central directory).
    RenameEntry {
        /// Replacement entry name. Must be the same length as the current
        /// entry name resolved from the tree path.
        new_name: String,
    },
}

impl_tree_op!(ZipTreeOp);

use crate::classifier::{ELF, ZIP};

/// Annotation kind for ELF file header info.
pub const ELF_HEADER_KIND: &str = "content.elf.header";

/// Annotation kind for ELF section entries.
pub const ELF_SECTION_KIND: &str = "content.elf.section";

/// Annotation kind for ZIP archive header info.
pub const ZIP_HEADER_KIND: &str = "content.zip.header";

/// Annotation kind for ZIP file entries.
pub const ZIP_ENTRY_KIND: &str = "content.zip.entry";

/// ELF structured summary codec.
///
/// Produces a summary of the ELF binary including header info,
/// section table, and basic statistics.
pub struct ElfCodec;

impl ElfCodec {
    /// Create a new ELF codec.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for ElfCodec {
    fn default() -> Self {
        Self::new()
    }
}

/// ELF still decodes to a read-only human summary, but Plan 07 Phase 2 adds a
/// narrow structural-edit seam via `DecodedEdit::Tree`.
impl ContentCodec for ElfCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        let elf = goblin::elf::Elf::parse(raw)
            .map_err(|e| CodecError::Other(format!("ELF parse failed: {e}")))?;

        let (content, annotations) = format_elf_summary(&elf, raw.len());

        let mut metadata = CodecMetadata::new(ContentType::new(ELF));
        metadata.set("readonly", "true");
        metadata.set("file_size", raw.len().to_string());

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
            DecodedEdit::Tree { path, op } => op.downcast_ref::<ElfTreeOp>().map_or(
                Err(TranslateEditError::UnsupportedEdit {
                    reason: "ELF codec only accepts ELF tree operations",
                }),
                |elf_op| translate_elf_edit(bytes, path, elf_op),
            ),
            DecodedEdit::Text { .. } | DecodedEdit::Bytes { .. } => {
                Err(TranslateEditError::UnsupportedEdit {
                    reason: "ELF codec does not translate text or raw byte edits",
                })
            }
            _ => Err(TranslateEditError::UnsupportedEdit {
                reason: "ELF codec does not support this decoded edit variant",
            }),
        }
    }
}

/// ZIP structured summary codec.
///
/// Produces a listing of ZIP archive entries with names, sizes,
/// and compression info.
pub struct ZipCodec;

impl ZipCodec {
    /// Create a new ZIP codec.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for ZipCodec {
    fn default() -> Self {
        Self::new()
    }
}

/// ZIP still decodes to a read-only human summary, but Plan 07 Phase 4 adds a
/// narrow structural-edit seam via `DecodedEdit::Tree`.
impl reovim_driver_codec::ContentCodec for ZipCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        let cursor = std::io::Cursor::new(raw);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| CodecError::Other(format!("ZIP parse failed: {e}")))?;

        let entry_count = archive.len();
        let (content, annotations) = format_zip_summary(&mut archive);

        let mut metadata = CodecMetadata::new(ContentType::new(ZIP));
        metadata.set("readonly", "true");
        metadata.set("file_size", raw.len().to_string());
        metadata.set("entry_count", entry_count.to_string());

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
            DecodedEdit::Tree { path, op } => op.downcast_ref::<ZipTreeOp>().map_or(
                Err(TranslateEditError::UnsupportedEdit {
                    reason: "ZIP codec only accepts ZIP tree operations",
                }),
                |zip_op| translate_zip_edit(bytes, path, zip_op),
            ),
            DecodedEdit::Text { .. } | DecodedEdit::Bytes { .. } => {
                Err(TranslateEditError::UnsupportedEdit {
                    reason: "ZIP codec does not translate text or raw byte edits",
                })
            }
            _ => Err(TranslateEditError::UnsupportedEdit {
                reason: "ZIP codec does not support this decoded edit variant",
            }),
        }
    }
}

fn translate_elf_edit(
    bytes: &dyn ByteSource,
    path: &TreePath,
    op: &ElfTreeOp,
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let raw = read_all_bytes(bytes).ok_or(TranslateEditError::Internal {
        reason: "ELF byte source could not be fully read",
    })?;
    let elf = goblin::elf::Elf::parse(&raw).map_err(|_| TranslateEditError::Internal {
        reason: "ELF parse failed during translate_edit",
    })?;

    match op {
        ElfTreeOp::PatchBytes {
            offset,
            old_bytes,
            new_bytes,
        } => translate_elf_patch_bytes(&elf, &raw, path, *offset, old_bytes, new_bytes),
        ElfTreeOp::RenameSymbol { new_name } => {
            translate_elf_rename_symbol(&elf, &raw, path, new_name)
        }
        ElfTreeOp::ReplaceSectionBytes {
            old_bytes,
            new_bytes,
        } => translate_elf_replace_section(&elf, &raw, path, old_bytes, new_bytes),
    }
}

fn translate_elf_patch_bytes(
    elf: &goblin::elf::Elf<'_>,
    raw: &[u8],
    path: &TreePath,
    offset: usize,
    old_bytes: &[u8],
    new_bytes: &[u8],
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let section = resolve_section_path(elf, path)?;
    let section_name = section_name(elf, section)?;
    if (section.sh_flags & u64::from(goblin::elf::section_header::SHF_EXECINSTR)) == 0
        && !section_name.starts_with(".text")
    {
        return Err(TranslateEditError::UnsupportedEdit {
            reason: "ELF instruction patch requires an executable section",
        });
    }

    if old_bytes.len() != new_bytes.len() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "ELF instruction patch must preserve byte length",
        });
    }

    let section_bytes = section_bytes(raw, section)?;
    let end =
        offset
            .checked_add(old_bytes.len())
            .ok_or(TranslateEditError::ConstraintViolation {
                reason: "ELF instruction patch range overflowed",
            })?;
    let actual = section_bytes
        .get(offset..end)
        .ok_or(TranslateEditError::ConstraintViolation {
            reason: "ELF instruction patch exceeds section bounds",
        })?;
    if actual != old_bytes {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "ELF instruction patch old bytes do not match the section contents",
        });
    }
    if old_bytes == new_bytes {
        return Ok(None);
    }

    let file_offset = section_file_offset(section)?.checked_add(offset).ok_or(
        TranslateEditError::ConstraintViolation {
            reason: "ELF instruction patch file offset overflowed",
        },
    )?;
    Ok(Some(ByteEdit::replace(file_offset, old_bytes, new_bytes)))
}

fn translate_elf_rename_symbol(
    elf: &goblin::elf::Elf<'_>,
    raw: &[u8],
    path: &TreePath,
    new_name: &str,
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let symbol_name = resolve_symbol_path(path)?;
    let symbol = resolve_named_symbol(elf, symbol_name)?;
    if symbol_name.len() != new_name.len() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "ELF symbol rename must preserve name length",
        });
    }
    if symbol_name == new_name {
        return Ok(None);
    }

    let strtab_offset = find_symbol_strtab_offset(elf)?;
    let name_offset = strtab_offset.checked_add(symbol.name_offset).ok_or(
        TranslateEditError::ConstraintViolation {
            reason: "ELF symbol name offset overflowed",
        },
    )?;
    let actual = raw
        .get(name_offset..name_offset + symbol_name.len())
        .ok_or(TranslateEditError::Internal {
            reason: "ELF symbol name range is out of bounds",
        })?;
    if actual != symbol_name.as_bytes() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "ELF symbol name bytes do not match the string table contents",
        });
    }

    Ok(Some(ByteEdit::replace(
        name_offset,
        symbol_name.as_bytes(),
        new_name.as_bytes(),
    )))
}

fn translate_elf_replace_section(
    elf: &goblin::elf::Elf<'_>,
    raw: &[u8],
    path: &TreePath,
    old_bytes: &[u8],
    new_bytes: &[u8],
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let section = resolve_section_path(elf, path)?;
    if section.sh_type == goblin::elf::section_header::SHT_NOBITS {
        return Err(TranslateEditError::UnsupportedEdit {
            reason: "ELF section replacement does not support NOBITS sections",
        });
    }
    if old_bytes.len() != new_bytes.len() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "ELF section replacement must preserve section size",
        });
    }

    let actual = section_bytes(raw, section)?;
    if actual.len() != old_bytes.len() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "ELF section replacement must cover the full section payload",
        });
    }
    if actual != old_bytes {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "ELF section replacement old bytes do not match the section contents",
        });
    }
    if old_bytes == new_bytes {
        return Ok(None);
    }

    Ok(Some(ByteEdit::replace(section_file_offset(section)?, old_bytes, new_bytes)))
}

fn read_all_bytes(bytes: &dyn ByteSource) -> Option<Vec<u8>> {
    let len = usize::try_from(bytes.len()).ok()?;
    let data = bytes.read(0..bytes.len()).into_owned();

    (data.len() == len).then_some(data)
}

fn resolve_section_path<'a>(
    elf: &'a goblin::elf::Elf<'_>,
    path: &TreePath,
) -> Result<&'a goblin::elf::section_header::SectionHeader, TranslateEditError> {
    let components = path.components();
    let [kind, section_name, field] = components else {
        return Err(TranslateEditError::MalformedPath {
            reason: "ELF section path must be [sections, <name>, bytes]",
        });
    };
    if kind != "sections" || field != "bytes" || section_name.is_empty() {
        return Err(TranslateEditError::MalformedPath {
            reason: "ELF section path must be [sections, <name>, bytes]",
        });
    }

    let mut matches = elf.section_headers.iter().filter(|section| {
        elf.shdr_strtab
            .get_at(section.sh_name)
            .is_some_and(|candidate| candidate == section_name)
    });
    let section = matches.next().ok_or(TranslateEditError::MalformedPath {
        reason: "ELF section path does not resolve to a section",
    })?;
    if matches.next().is_some() {
        return Err(TranslateEditError::MalformedPath {
            reason: "ELF section path is ambiguous",
        });
    }

    Ok(section)
}

fn resolve_symbol_path(path: &TreePath) -> Result<&str, TranslateEditError> {
    let components = path.components();
    let [kind, symbol_name, field] = components else {
        return Err(TranslateEditError::MalformedPath {
            reason: "ELF symbol path must be [symbols, <name>, name]",
        });
    };
    if kind != "symbols" || field != "name" || symbol_name.is_empty() {
        return Err(TranslateEditError::MalformedPath {
            reason: "ELF symbol path must be [symbols, <name>, name]",
        });
    }

    Ok(symbol_name)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedSymbol {
    name_offset: usize,
}

fn resolve_named_symbol(
    elf: &goblin::elf::Elf<'_>,
    symbol_name: &str,
) -> Result<ResolvedSymbol, TranslateEditError> {
    let mut matches = elf.syms.iter().filter(|symbol| {
        elf.strtab
            .get_at(symbol.st_name)
            .is_some_and(|candidate| candidate == symbol_name)
    });
    let symbol = matches.next().ok_or(TranslateEditError::MalformedPath {
        reason: "ELF symbol path does not resolve to a symbol",
    })?;
    if matches.next().is_some() {
        return Err(TranslateEditError::MalformedPath {
            reason: "ELF symbol path is ambiguous",
        });
    }

    Ok(ResolvedSymbol {
        name_offset: symbol.st_name,
    })
}

fn find_symbol_strtab_offset(elf: &goblin::elf::Elf<'_>) -> Result<usize, TranslateEditError> {
    let symtab = elf
        .section_headers
        .iter()
        .find(|section| section.sh_type == goblin::elf::section_header::SHT_SYMTAB)
        .ok_or(TranslateEditError::UnsupportedEdit {
            reason: "ELF symbol rename requires a symbol table",
        })?;
    let link_index = usize::try_from(symtab.sh_link).map_err(|_| TranslateEditError::Internal {
        reason: "ELF symbol table link index does not fit in usize",
    })?;
    let strtab = elf
        .section_headers
        .get(link_index)
        .ok_or(TranslateEditError::Internal {
            reason: "ELF symbol table string table is missing",
        })?;

    section_file_offset(strtab)
}

fn section_file_offset(
    section: &goblin::elf::section_header::SectionHeader,
) -> Result<usize, TranslateEditError> {
    usize::try_from(section.sh_offset).map_err(|_| TranslateEditError::Internal {
        reason: "ELF section offset does not fit in usize",
    })
}

fn section_size(
    section: &goblin::elf::section_header::SectionHeader,
) -> Result<usize, TranslateEditError> {
    usize::try_from(section.sh_size).map_err(|_| TranslateEditError::Internal {
        reason: "ELF section size does not fit in usize",
    })
}

fn section_bytes<'a>(
    raw: &'a [u8],
    section: &goblin::elf::section_header::SectionHeader,
) -> Result<&'a [u8], TranslateEditError> {
    let offset = section_file_offset(section)?;
    let size = section_size(section)?;
    let end = offset
        .checked_add(size)
        .ok_or(TranslateEditError::Internal {
            reason: "ELF section range overflowed",
        })?;

    raw.get(offset..end).ok_or(TranslateEditError::Internal {
        reason: "ELF section bytes are out of bounds",
    })
}

fn section_name<'a>(
    elf: &'a goblin::elf::Elf<'_>,
    section: &goblin::elf::section_header::SectionHeader,
) -> Result<&'a str, TranslateEditError> {
    elf.shdr_strtab
        .get_at(section.sh_name)
        .ok_or(TranslateEditError::Internal {
            reason: "ELF section name is missing from the header string table",
        })
}

// ============================================================================
// ZIP structural editing (Plan 07 Phase 4)
// ============================================================================

/// ZIP local file header size (fixed portion before filename).
const ZIP_LOCAL_HEADER_FIXED_SIZE: usize = 30;

/// ZIP central directory header size (fixed portion before filename).
const ZIP_CENTRAL_DIR_HEADER_FIXED_SIZE: usize = 46;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZipEntryField {
    Bytes,
    Name,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZipTargetPath<'a> {
    ArchiveComment,
    Entry { name: &'a str, field: ZipEntryField },
}

fn resolve_zip_target_path(path: &TreePath) -> Result<ZipTargetPath<'_>, TranslateEditError> {
    let components = path.components();
    match components {
        [kind, field] if kind == "archive" && field == "comment" => {
            Ok(ZipTargetPath::ArchiveComment)
        }
        [kind, entry_name, field] if kind == "entries" && !entry_name.is_empty() => {
            let entry_field = match field.as_str() {
                "bytes" => ZipEntryField::Bytes,
                "name" => ZipEntryField::Name,
                _ => {
                    return Err(TranslateEditError::MalformedPath {
                        reason: "ZIP entry path must be [entries, <name>, bytes|name]",
                    });
                }
            };
            Ok(ZipTargetPath::Entry {
                name: entry_name,
                field: entry_field,
            })
        }
        _ => Err(TranslateEditError::MalformedPath {
            reason: "ZIP tree path must be [archive, comment] or [entries, <name>, bytes|name]",
        }),
    }
}

fn translate_zip_edit(
    bytes: &dyn ByteSource,
    path: &TreePath,
    op: &ZipTreeOp,
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let raw = read_all_bytes(bytes).ok_or(TranslateEditError::Internal {
        reason: "ZIP byte source could not be fully read",
    })?;
    let cursor = std::io::Cursor::new(&raw[..]);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|_| TranslateEditError::Internal {
        reason: "ZIP parse failed during translate_edit",
    })?;
    let target = resolve_zip_target_path(path)?;

    match (target, op) {
        (ZipTargetPath::ArchiveComment, ZipTreeOp::ReplaceComment { new_comment }) => {
            translate_zip_replace_comment(&archive, &raw, new_comment)
        }
        (
            ZipTargetPath::Entry {
                name,
                field: ZipEntryField::Bytes,
            },
            ZipTreeOp::ReplaceEntryBytes {
                old_bytes,
                new_bytes,
            },
        ) => translate_zip_replace_entry_bytes(&mut archive, &raw, name, old_bytes, new_bytes),
        (
            ZipTargetPath::Entry {
                name,
                field: ZipEntryField::Name,
            },
            ZipTreeOp::RenameEntry { new_name },
        ) => translate_zip_rename_entry(&mut archive, &raw, name, new_name),
        (ZipTargetPath::ArchiveComment, _) => Err(TranslateEditError::UnsupportedEdit {
            reason: "ZIP archive comment only supports ReplaceComment",
        }),
        (
            ZipTargetPath::Entry {
                field: ZipEntryField::Bytes,
                ..
            },
            ZipTreeOp::RenameEntry { .. },
        ) => Err(TranslateEditError::UnsupportedEdit {
            reason: "ZIP entry rename must target [entries, <name>, name]",
        }),
        (
            ZipTargetPath::Entry {
                field: ZipEntryField::Name,
                ..
            },
            ZipTreeOp::ReplaceEntryBytes { .. },
        ) => Err(TranslateEditError::UnsupportedEdit {
            reason: "ZIP entry payload replacement must target [entries, <name>, bytes]",
        }),
        (
            ZipTargetPath::Entry {
                field: ZipEntryField::Bytes,
                ..
            },
            ZipTreeOp::ReplaceComment { .. },
        )
        | (
            ZipTargetPath::Entry {
                field: ZipEntryField::Name,
                ..
            },
            ZipTreeOp::ReplaceComment { .. },
        ) => Err(TranslateEditError::UnsupportedEdit {
            reason: "ZIP ReplaceComment must target [archive, comment]",
        }),
    }
}

fn translate_zip_replace_comment<R: std::io::Read + std::io::Seek>(
    archive: &zip::ZipArchive<R>,
    raw: &[u8],
    new_comment: &[u8],
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let current_comment = archive.comment();
    if new_comment.len() != current_comment.len() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "ZIP archive comment replacement must preserve comment length",
        });
    }
    if current_comment == new_comment {
        return Ok(None);
    }

    // EOCD comment is at the very end of the file, occupying the last
    // `comment.len()` bytes.
    let comment_offset =
        raw.len()
            .checked_sub(current_comment.len())
            .ok_or(TranslateEditError::Internal {
                reason: "ZIP archive comment offset underflowed",
            })?;

    // Verify the bytes at that offset match the current comment.
    let actual = raw
        .get(comment_offset..raw.len())
        .ok_or(TranslateEditError::Internal {
            reason: "ZIP archive comment range is out of bounds",
        })?;
    if actual != current_comment {
        return Err(TranslateEditError::Internal {
            reason: "ZIP archive comment bytes do not match at expected offset",
        });
    }

    Ok(Some(ByteEdit::replace(comment_offset, current_comment, new_comment)))
}

fn translate_zip_replace_entry_bytes<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    raw: &[u8],
    entry_name: &str,
    old_bytes: &[u8],
    new_bytes: &[u8],
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let entry = find_zip_entry_by_name(archive, entry_name)?;

    if entry.compression != zip::CompressionMethod::Stored {
        return Err(TranslateEditError::UnsupportedEdit {
            reason: "ZIP entry payload replacement only supports STORED entries",
        });
    }
    if old_bytes.len() != new_bytes.len() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "ZIP entry payload replacement must preserve payload size",
        });
    }

    let data_start = entry.data_start.ok_or(TranslateEditError::Internal {
        reason: "ZIP entry data start offset is not available",
    })?;
    let data_offset = usize::try_from(data_start).map_err(|_| TranslateEditError::Internal {
        reason: "ZIP entry data start offset does not fit in usize",
    })?;
    let data_size = usize::try_from(entry.size).map_err(|_| TranslateEditError::Internal {
        reason: "ZIP entry size does not fit in usize",
    })?;
    let data_end =
        data_offset
            .checked_add(data_size)
            .ok_or(TranslateEditError::ConstraintViolation {
                reason: "ZIP entry data range overflowed",
            })?;
    let actual = raw
        .get(data_offset..data_end)
        .ok_or(TranslateEditError::Internal {
            reason: "ZIP entry data range is out of bounds",
        })?;

    if actual.len() != old_bytes.len() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "ZIP entry payload replacement must cover the full entry payload",
        });
    }
    if actual != old_bytes {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "ZIP entry payload old bytes do not match the archive contents",
        });
    }
    if old_bytes == new_bytes {
        return Ok(None);
    }

    Ok(Some(ByteEdit::replace(data_offset, old_bytes, new_bytes)))
}

fn translate_zip_rename_entry<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    raw: &[u8],
    entry_name: &str,
    new_name: &str,
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let entry = find_zip_entry_by_name(archive, entry_name)?;

    if entry_name.len() != new_name.len() {
        return Err(TranslateEditError::ConstraintViolation {
            reason: "ZIP entry rename must preserve name length",
        });
    }
    if entry_name == new_name {
        return Ok(None);
    }

    let old_name_bytes = entry_name.as_bytes();
    let new_name_bytes = new_name.as_bytes();

    // Locate the name field in the local header (30 bytes into the header).
    let local_name_offset = usize::try_from(entry.header_start)
        .map_err(|_| TranslateEditError::Internal {
            reason: "ZIP local header offset does not fit in usize",
        })?
        .checked_add(ZIP_LOCAL_HEADER_FIXED_SIZE)
        .ok_or(TranslateEditError::ConstraintViolation {
            reason: "ZIP local header name offset overflowed",
        })?;
    verify_name_at_offset(raw, local_name_offset, old_name_bytes, "local header")?;

    // Locate the name field in the central directory (46 bytes into the header).
    let central_name_offset = usize::try_from(entry.central_header_start)
        .map_err(|_| TranslateEditError::Internal {
            reason: "ZIP central directory offset does not fit in usize",
        })?
        .checked_add(ZIP_CENTRAL_DIR_HEADER_FIXED_SIZE)
        .ok_or(TranslateEditError::ConstraintViolation {
            reason: "ZIP central directory name offset overflowed",
        })?;
    verify_name_at_offset(raw, central_name_offset, old_name_bytes, "central directory")?;

    // Emit a single ByteEdit spanning from the earlier name to the end
    // of the later name, with both occurrences patched in place.
    if local_name_offset < central_name_offset {
        emit_dual_name_patch(
            raw,
            local_name_offset,
            central_name_offset,
            old_name_bytes,
            new_name_bytes,
        )
    } else {
        emit_dual_name_patch(
            raw,
            central_name_offset,
            local_name_offset,
            old_name_bytes,
            new_name_bytes,
        )
    }
}

/// Build a single `ByteEdit` that patches two same-length name occurrences
/// within one contiguous span of bytes.
fn emit_dual_name_patch(
    raw: &[u8],
    first_offset: usize,
    second_offset: usize,
    old_name: &[u8],
    new_name: &[u8],
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let span_end = second_offset.checked_add(old_name.len()).ok_or(
        TranslateEditError::ConstraintViolation {
            reason: "ZIP rename span end overflowed",
        },
    )?;
    let old_span = raw
        .get(first_offset..span_end)
        .ok_or(TranslateEditError::Internal {
            reason: "ZIP rename span is out of bounds",
        })?;
    let mut new_span = old_span.to_vec();
    new_span[..new_name.len()].copy_from_slice(new_name);
    let second_rel = second_offset - first_offset;
    new_span[second_rel..second_rel + new_name.len()].copy_from_slice(new_name);
    Ok(Some(ByteEdit::replace(first_offset, old_span, &new_span)))
}

/// Entry metadata resolved from a zip archive by name lookup.
#[derive(Debug, Clone, Copy)]
struct ResolvedZipEntry {
    compression: zip::CompressionMethod,
    size: u64,
    data_start: Option<u64>,
    header_start: u64,
    central_header_start: u64,
}

fn find_zip_entry_by_name<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<ResolvedZipEntry, TranslateEditError> {
    for i in 0..archive.len() {
        let entry = archive
            .by_index_raw(i)
            .map_err(|_| TranslateEditError::Internal {
                reason: "ZIP entry access failed during name lookup",
            })?;
        if entry.name() == name {
            return Ok(ResolvedZipEntry {
                compression: entry.compression(),
                size: entry.size(),
                data_start: entry.data_start(),
                header_start: entry.header_start(),
                central_header_start: entry.central_header_start(),
            });
        }
    }
    Err(TranslateEditError::MalformedPath {
        reason: "ZIP entry name does not resolve to an entry",
    })
}

fn verify_name_at_offset(
    raw: &[u8],
    offset: usize,
    expected: &[u8],
    location: &'static str,
) -> Result<(), TranslateEditError> {
    let end =
        offset
            .checked_add(expected.len())
            .ok_or(TranslateEditError::ConstraintViolation {
                reason: "ZIP name verification range overflowed",
            })?;
    let actual = raw.get(offset..end).ok_or(TranslateEditError::Internal {
        reason: if location == "local header" {
            "ZIP local header name range is out of bounds"
        } else {
            "ZIP central directory name range is out of bounds"
        },
    })?;
    if actual != expected {
        return Err(TranslateEditError::ConstraintViolation {
            reason: if location == "local header" {
                "ZIP local header name bytes do not match"
            } else {
                "ZIP central directory name bytes do not match"
            },
        });
    }
    Ok(())
}

/// Format an ELF binary into a human-readable summary.
#[cfg_attr(coverage_nightly, coverage(off))]
fn format_elf_summary(elf: &goblin::elf::Elf<'_>, file_size: usize) -> (String, Vec<Annotation>) {
    let mut output = String::with_capacity(2048);
    let mut annotations = Vec::new();

    let header_kind = AnnotationKind::new(ELF_HEADER_KIND);
    let section_kind = AnnotationKind::new(ELF_SECTION_KIND);

    // Header section
    let mut line_idx = 0;

    output.push_str("ELF Binary Summary\n");
    annotations.push(Annotation {
        kind: header_kind.clone(),
        target: AnnotationTarget::Line(line_idx),
        priority: 0,
        payload: AnnotationPayload::None,
    });
    line_idx += 1;

    let _ = writeln!(output, "==================");
    line_idx += 1;

    // File type
    let elf_type = match elf.header.e_type {
        goblin::elf::header::ET_NONE => "None",
        goblin::elf::header::ET_REL => "Relocatable",
        goblin::elf::header::ET_EXEC => "Executable",
        goblin::elf::header::ET_DYN => "Shared Object",
        goblin::elf::header::ET_CORE => "Core Dump",
        _ => "Unknown",
    };
    let _ = writeln!(output, "Type:         {elf_type}");
    line_idx += 1;

    // Machine type
    let machine = elf_machine_name(elf.header.e_machine);
    let _ = writeln!(output, "Machine:      {machine}");
    line_idx += 1;

    // Entry point
    let _ = writeln!(output, "Entry Point:  {:#x}", elf.entry);
    line_idx += 1;

    // File size
    let _ = writeln!(output, "File Size:    {file_size} bytes");
    line_idx += 1;

    // Program headers
    let _ = writeln!(output, "Prog Headers: {}", elf.program_headers.len());
    line_idx += 1;

    // Section count
    let _ = writeln!(output, "Sections:     {}", elf.section_headers.len());
    line_idx += 1;

    // Blank separator
    output.push('\n');
    line_idx += 1;

    // Section table
    if !elf.section_headers.is_empty() {
        output.push_str("Section Table\n");
        annotations.push(Annotation {
            kind: header_kind,
            target: AnnotationTarget::Line(line_idx),
            priority: 0,
            payload: AnnotationPayload::None,
        });
        line_idx += 1;

        let _ = writeln!(output, "-------------");
        line_idx += 1;

        let _ = writeln!(
            output,
            "{name:<30} {size:>12} {offset:>12}  Type",
            name = "Name",
            size = "Size",
            offset = "Offset"
        );
        line_idx += 1;

        for sh in &elf.section_headers {
            let name = elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("<unknown>");
            let section_type = elf_section_type(sh.sh_type);

            let _ = writeln!(
                output,
                "{name:<30} {size:>12} {offset:>12}  {stype}",
                size = sh.sh_size,
                offset = sh.sh_offset,
                stype = section_type,
            );

            annotations.push(Annotation {
                kind: section_kind.clone(),
                target: AnnotationTarget::Line(line_idx),
                priority: 0,
                payload: AnnotationPayload::Text(name.to_string()),
            });

            line_idx += 1;
        }
    }

    (output, annotations)
}

/// Format a ZIP archive into a human-readable entry listing.
#[cfg_attr(coverage_nightly, coverage(off))]
fn format_zip_summary<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> (String, Vec<Annotation>) {
    let mut output = String::with_capacity(2048);
    let mut annotations = Vec::new();

    let header_kind = AnnotationKind::new(ZIP_HEADER_KIND);
    let entry_kind = AnnotationKind::new(ZIP_ENTRY_KIND);

    let entry_count = archive.len();
    let mut line_idx = 0;

    // Header
    output.push_str("ZIP Archive Contents\n");
    annotations.push(Annotation {
        kind: header_kind.clone(),
        target: AnnotationTarget::Line(line_idx),
        priority: 0,
        payload: AnnotationPayload::None,
    });
    line_idx += 1;

    let _ = writeln!(output, "====================");
    line_idx += 1;

    let _ = writeln!(output, "Entries: {entry_count}");
    line_idx += 1;

    // Blank separator
    output.push('\n');
    line_idx += 1;

    // Column headers
    let _ = writeln!(
        output,
        "{name:<50} {size:>12} {comp:>12}  Method",
        name = "Name",
        size = "Size",
        comp = "Compressed"
    );
    annotations.push(Annotation {
        kind: header_kind,
        target: AnnotationTarget::Line(line_idx),
        priority: 0,
        payload: AnnotationPayload::None,
    });
    line_idx += 1;

    let _ = writeln!(output, "{}", "-".repeat(90));
    line_idx += 1;

    // Entries with metadata
    for i in 0..entry_count {
        // Capture name fallback before the mutable borrow.
        let fallback_name = archive.name_for_index(i).unwrap_or("<unknown>").to_string();

        let (name, size, compressed, method) = archive.by_index_raw(i).map_or_else(
            |_| (fallback_name, 0, 0, "?".to_string()),
            |entry| {
                let n = entry.name().to_string();
                let s = entry.size();
                let c = entry.compressed_size();
                let m = format!("{:?}", entry.compression());
                (n, s, c, m)
            },
        );

        let _ = writeln!(output, "{name:<50} {size:>12} {compressed:>12}  {method}");

        annotations.push(Annotation {
            kind: entry_kind.clone(),
            target: AnnotationTarget::Line(line_idx),
            priority: 0,
            payload: AnnotationPayload::Text(name),
        });

        line_idx += 1;
    }

    (output, annotations)
}

/// Map ELF `e_machine` to a human-readable name.
const fn elf_machine_name(machine: u16) -> &'static str {
    match machine {
        goblin::elf::header::EM_NONE => "None",
        goblin::elf::header::EM_386 => "x86",
        goblin::elf::header::EM_ARM => "ARM",
        goblin::elf::header::EM_X86_64 => "x86-64",
        goblin::elf::header::EM_AARCH64 => "AArch64",
        goblin::elf::header::EM_RISCV => "RISC-V",
        goblin::elf::header::EM_MIPS => "MIPS",
        goblin::elf::header::EM_PPC => "PowerPC",
        goblin::elf::header::EM_PPC64 => "PowerPC64",
        goblin::elf::header::EM_S390 => "S/390",
        goblin::elf::header::EM_SPARC => "SPARC",
        _ => "Unknown",
    }
}

/// Map ELF section type to a human-readable name.
const fn elf_section_type(stype: u32) -> &'static str {
    match stype {
        goblin::elf::section_header::SHT_NULL => "NULL",
        goblin::elf::section_header::SHT_PROGBITS => "PROGBITS",
        goblin::elf::section_header::SHT_SYMTAB => "SYMTAB",
        goblin::elf::section_header::SHT_STRTAB => "STRTAB",
        goblin::elf::section_header::SHT_RELA => "RELA",
        goblin::elf::section_header::SHT_HASH => "HASH",
        goblin::elf::section_header::SHT_DYNAMIC => "DYNAMIC",
        goblin::elf::section_header::SHT_NOTE => "NOTE",
        goblin::elf::section_header::SHT_NOBITS => "NOBITS",
        goblin::elf::section_header::SHT_REL => "REL",
        goblin::elf::section_header::SHT_DYNSYM => "DYNSYM",
        goblin::elf::section_header::SHT_INIT_ARRAY => "INIT_ARRAY",
        goblin::elf::section_header::SHT_FINI_ARRAY => "FINI_ARRAY",
        _ => "OTHER",
    }
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
