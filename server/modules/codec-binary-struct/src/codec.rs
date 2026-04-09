//! ELF and ZIP structured binary codecs.
//!
//! Produces human-readable summaries of ELF binaries and ZIP archives.
//! Both are one-way (decode-only) codecs — structured views cannot be
//! saved back to binary format.

use std::fmt::Write;

use {
    reovim_driver_annotation::{Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget},
    reovim_driver_codec::{CodecError, CodecMetadata, ContentType, DecodeResult},
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

/// Phase 3: ELF is a transforming-only structured view; byte-level edits cannot be
/// reconstructed from this summary representation, so decoded text edits are
/// intentionally unsupported via `translate_edit`.
impl reovim_driver_codec::ContentCodec for ElfCodec {
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

    fn encode(
        &self,
        _content: &str,
        _metadata: &CodecMetadata,
    ) -> Option<Result<Vec<u8>, CodecError>> {
        None
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

    fn encode(
        &self,
        _content: &str,
        _metadata: &CodecMetadata,
    ) -> Option<Result<Vec<u8>, CodecError>> {
        None
    }
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
