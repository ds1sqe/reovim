//! Rust .rlib content classifier.
//!
//! Detects Rust library archives via `.rlib` extension combined with
//! `!<arch>\n` (ar archive) magic bytes at offset 0.

use reovim_driver_codec::{ContentClassifier, ContentType};

/// Content type for Rust .rlib archives.
pub const RLIB: &str = "binary/rlib";

/// AR archive magic bytes: `!<arch>\n`.
const AR_MAGIC: &[u8] = b"!<arch>\n";

/// Rlib content classifier (priority 32).
///
/// Between ELF (33) and ZIP (31). Requires `.rlib` extension because
/// the ar magic `!<arch>\n` is shared with generic `.a` archives.
/// Plain `.a` files without `.rlib` extension are not classified here
/// and fall through to hex dump.
pub struct RlibClassifier;

impl RlibClassifier {
    /// Create a new rlib classifier.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for RlibClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentClassifier for RlibClassifier {
    fn classify(&self, raw: &[u8], path: &str) -> Option<ContentType> {
        // Require .rlib extension (ar magic alone is too generic)
        if !has_rlib_extension(path) {
            return None;
        }

        // Extension fast-path: .rlib extension alone is sufficient
        // (no other format uses this extension)
        if raw.len() >= AR_MAGIC.len() && raw[..AR_MAGIC.len()] == *AR_MAGIC {
            return Some(ContentType::new(RLIB));
        }

        // Extension present but no ar magic — still classify as rlib
        // (file may be truncated or have unusual header)
        Some(ContentType::new(RLIB))
    }

    fn priority(&self) -> u8 {
        32
    }

    fn name(&self) -> &'static str {
        "rlib"
    }
}

/// Check if the file path has `.rlib` extension.
fn has_rlib_extension(path: &str) -> bool {
    // Input is `&str` (valid UTF-8), so `extension().to_str()` always
    // succeeds.  Chain with `and_then` to avoid an untestable branch.
    std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("rlib"))
}

#[cfg(test)]
#[path = "classifier_tests.rs"]
mod tests;
