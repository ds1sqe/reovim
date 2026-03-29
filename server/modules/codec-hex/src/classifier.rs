//! Binary content classifier.
//!
//! Detects binary files by sampling the first 8192 bytes for null bytes
//! or a high ratio of non-printable characters.

use reovim_driver_codec::{ContentClassifier, ContentType};

/// Maximum bytes to sample for binary detection.
const SAMPLE_SIZE: usize = 8192;

/// Non-printable threshold: numerator for integer ratio check.
/// Equivalent to 30%: `non_printable * 10 > sample.len() * 3`.
const NON_PRINTABLE_NUMERATOR: usize = 10;
const NON_PRINTABLE_DENOMINATOR: usize = 3;

/// Known binary file extensions (fast-path, no content scanning needed).
const BINARY_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "dylib", "o", "a", "lib", // Executables/objects
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", // Images
    "zip", "gz", "bz2", "xz", "tar", "7z", "rar", "zst",   // Archives
    "pdf",   // Documents
    "wasm",  // WebAssembly
    "class", // Java bytecode
    "pyc", "pyo", // Python bytecode
];

/// Binary content classifier (priority 20).
///
/// Runs before the UTF-8 classifier (priority 10). If the content looks
/// binary (null bytes, high non-printable ratio, or known binary extension),
/// returns `ContentType::BINARY_RAW`.
pub struct BinaryClassifier;

impl BinaryClassifier {
    /// Create a new binary classifier.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for BinaryClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentClassifier for BinaryClassifier {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn classify(&self, raw: &[u8], path: &str) -> Option<ContentType> {
        // Fast-path: known binary extensions
        if has_binary_extension(path) {
            return Some(ContentType::new(ContentType::BINARY_RAW));
        }

        // Empty files are not binary
        if raw.is_empty() {
            return None;
        }

        // Sample the first SAMPLE_SIZE bytes
        let sample = if raw.len() > SAMPLE_SIZE {
            &raw[..SAMPLE_SIZE]
        } else {
            raw
        };

        // Null byte detection (immediate binary indicator)
        if sample.contains(&0x00) {
            return Some(ContentType::new(ContentType::BINARY_RAW));
        }

        // Count non-printable bytes — use integer arithmetic to avoid float casts.
        // non_printable * 10 > sample.len() * 3 is equivalent to ratio > 0.30.
        // Max product: SAMPLE_SIZE(8192) * 10 = 81_920, well within usize.
        let non_printable = sample.iter().filter(|&&b| !is_printable(b)).count();

        if non_printable * NON_PRINTABLE_NUMERATOR > sample.len() * NON_PRINTABLE_DENOMINATOR {
            Some(ContentType::new(ContentType::BINARY_RAW))
        } else {
            None
        }
    }

    fn priority(&self) -> u8 {
        20
    }

    fn name(&self) -> &'static str {
        "binary"
    }
}

/// Check if a byte is considered "printable" for binary detection.
///
/// Printable bytes: tab (0x09), newline (0x0A), carriage return (0x0D),
/// and the range 0x20..=0x7E (ASCII printable), plus high bytes 0x80..=0xFF
/// (valid in UTF-8 multi-byte sequences).
const fn is_printable(b: u8) -> bool {
    matches!(b, 0x09 | 0x0A | 0x0D | 0x20..=0x7E | 0x80..=0xFF)
}

/// Check if the file path has a known binary extension.
fn has_binary_extension(path: &str) -> bool {
    let Some(ext) = std::path::Path::new(path).extension() else {
        return false;
    };
    let Some(ext_str) = ext.to_str() else {
        return false;
    };
    let lower = ext_str.to_ascii_lowercase();
    BINARY_EXTENSIONS.contains(&lower.as_str())
}

#[cfg(test)]
#[path = "classifier_tests.rs"]
mod tests;
