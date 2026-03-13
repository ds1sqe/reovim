//! ELF and ZIP content classifiers.
//!
//! Detects ELF binaries via `\x7fELF` magic and ZIP archives via
//! `PK\x03\x04` magic bytes at offset 0.

use reovim_driver_codec::{ContentClassifier, ContentType};

/// Content type for ELF binaries.
pub const ELF: &str = "binary/elf";

/// Content type for ZIP archives.
pub const ZIP: &str = "binary/zip";

/// ELF magic bytes: `\x7fELF`.
const ELF_MAGIC: &[u8] = b"\x7fELF";

/// ZIP local file header magic: `PK\x03\x04`.
const ZIP_MAGIC: &[u8] = &[0x50, 0x4B, 0x03, 0x04];

/// Known ELF file extensions (fast-path).
const ELF_EXTENSIONS: &[&str] = &["elf"];

/// Known ZIP file extensions (fast-path).
const ZIP_EXTENSIONS: &[&str] = &["zip", "jar", "war", "ear", "apk", "ipa", "xlsx", "docx", "pptx"];

/// ELF content classifier (priority 33).
///
/// Runs after text encoding classifiers (CJK 50, Legacy 40, PDF 35)
/// but before generic binary detection (Hex 20). ELF files contain
/// null bytes, so without this classifier they fall through to hex dump.
pub struct ElfClassifier;

impl ElfClassifier {
    /// Create a new ELF classifier.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ElfClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentClassifier for ElfClassifier {
    fn classify(&self, raw: &[u8], path: &str) -> Option<ContentType> {
        // Fast-path: known ELF extension
        if has_extension(path, ELF_EXTENSIONS) {
            return Some(ContentType::new(ELF));
        }

        // Magic byte detection
        if raw.len() >= ELF_MAGIC.len() && raw[..ELF_MAGIC.len()] == *ELF_MAGIC {
            return Some(ContentType::new(ELF));
        }

        None
    }

    fn priority(&self) -> u8 {
        33
    }

    fn name(&self) -> &'static str {
        "elf"
    }
}

/// ZIP content classifier (priority 31).
///
/// Runs after ELF (33) but before generic binary detection (Hex 20).
/// ZIP archives use `PK\x03\x04` as local file header signature.
pub struct ZipClassifier;

impl ZipClassifier {
    /// Create a new ZIP classifier.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ZipClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentClassifier for ZipClassifier {
    fn classify(&self, raw: &[u8], path: &str) -> Option<ContentType> {
        // Fast-path: known ZIP extension
        if has_extension(path, ZIP_EXTENSIONS) {
            return Some(ContentType::new(ZIP));
        }

        // Magic byte detection
        if raw.len() >= ZIP_MAGIC.len() && raw[..ZIP_MAGIC.len()] == *ZIP_MAGIC {
            return Some(ContentType::new(ZIP));
        }

        None
    }

    fn priority(&self) -> u8 {
        31
    }

    fn name(&self) -> &'static str {
        "zip"
    }
}

/// Check if the file path has one of the given extensions.
fn has_extension(path: &str, extensions: &[&str]) -> bool {
    let Some(ext) = std::path::Path::new(path).extension() else {
        return false;
    };
    let Some(ext_str) = ext.to_str() else {
        return false;
    };
    let lower = ext_str.to_ascii_lowercase();
    extensions.contains(&lower.as_str())
}

#[cfg(test)]
#[path = "classifier_tests.rs"]
mod tests;
