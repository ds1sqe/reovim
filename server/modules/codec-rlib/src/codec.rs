//! Rust .rlib structured codec.
//!
//! Parses `.rlib` files (ar archives containing Rust metadata and object
//! files) and produces a human-readable summary with archive member listing,
//! rustc version, and dependency information.

use std::fmt::Write;

use {
    reovim_driver_annotation::{Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget},
    reovim_driver_codec::{CodecError, CodecMetadata, ContentType, DecodeResult},
};

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

/// Phase 3: Rust `.rlib` summary output is a transform-only representation; byte
/// edits in this decoded view cannot be mapped back faithfully, so
/// `translate_edit` stays read-only.
impl reovim_driver_codec::ContentCodec for RlibCodec {
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
            // Crate names are typically 3-64 chars, contain underscore/hyphen
            if (3..=64).contains(&len)
                && (data[start..i].contains(&b'_') || data[start..i].contains(&b'-'))
                && let Ok(name) = std::str::from_utf8(&data[start..i])
                && !is_common_non_dep(name)
                && seen.insert(name.to_string())
            {
                deps.push(name.to_string());
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
