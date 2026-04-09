//! ELF and ZIP structured binary codecs.
//!
//! Produces human-readable summaries of ELF binaries and ZIP archives.
//! ZIP remains a one-way summary codec. ELF gains a narrow Plan 07
//! Phase 2 structural-edit surface for in-place tree edits.

use std::fmt::Write;

use {
    reovim_driver_annotation::{Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget},
    reovim_driver_codec::{
        CodecError, CodecMetadata, ContentCodec, ContentType, DecodeResult, DecodedEdit, ElfTreeOp,
        TranslateEditError, TreeOp, TreePath,
    },
    reovim_driver_vfs::ByteSource,
    reovim_kernel::api::v1::ByteEdit,
};

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
            DecodedEdit::Tree {
                path,
                op: TreeOp::Elf(op),
            } => translate_elf_edit(bytes, path, op),
            DecodedEdit::Tree { .. } => Err(TranslateEditError::UnsupportedEdit {
                reason: "ELF codec only accepts ELF tree operations",
            }),
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

/// Phase 3: ZIP is a transforming-only structured view; decoded text edits cannot
/// be reconstructed from this summary representation, so `translate_edit` remains
/// intentionally read-only.
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
