//! tar.gz content classifier.
//!
//! Detects gzip-compressed tar archives via `.tar.gz` or `.tgz` extension
//! combined with gzip magic bytes `[0x1f, 0x8b]` at offset 0.

use reovim_driver_codec::{ContentClassifier, ContentType};

/// Content type for tar.gz archives.
pub const TAR_GZ: &str = "binary/tar-gz";

/// Gzip magic bytes at offset 0.
const GZIP_MAGIC: &[u8; 2] = &[0x1f, 0x8b];

/// tar.gz content classifier (priority 30).
///
/// Below ZIP (31) and ELF (33). Requires `.tar.gz` or `.tgz` extension
/// AND gzip magic bytes to avoid false positives on arbitrary gzip files.
pub struct TarGzClassifier;

impl TarGzClassifier {
    /// Create a new tar.gz classifier.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for TarGzClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentClassifier for TarGzClassifier {
    fn classify(&self, raw: &[u8], path: &str) -> Option<ContentType> {
        // Require .tar.gz or .tgz extension
        if !has_tar_gz_extension(path) {
            return None;
        }

        // Require gzip magic bytes (extension alone is not enough — plain .tar files
        // could be renamed, and we want to be conservative)
        if raw.len() >= GZIP_MAGIC.len() && raw[..GZIP_MAGIC.len()] == *GZIP_MAGIC {
            return Some(ContentType::new(TAR_GZ));
        }

        None
    }

    fn priority(&self) -> u8 {
        30
    }

    fn name(&self) -> &'static str {
        "tar-gz"
    }
}

/// Check if the file path has a `.tar.gz` or `.tgz` extension.
fn has_tar_gz_extension(path: &str) -> bool {
    // Check .tgz via extension comparison (case-insensitive).
    let has_tgz = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("tgz"));

    // Check .tar.gz by lowercasing the full path (extension() only sees ".gz"
    // for double extensions, so we test the whole suffix here).
    let has_tar_gz = path.to_ascii_lowercase().ends_with(".tar.gz");

    has_tgz || has_tar_gz
}

#[cfg(test)]
#[path = "classifier_tests.rs"]
mod tests;
