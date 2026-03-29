//! UTF-8 content classifier.
//!
//! Lowest priority (10) — acts as the fallback classifier when no
//! higher-priority classifier recognizes the content.

use reovim_driver_codec::{ContentClassifier, ContentType};

/// UTF-8 content classifier.
///
/// Returns `Some(ContentType("text/utf-8"))` if the raw bytes are valid
/// UTF-8, `None` otherwise. Priority 10 (lowest) — this is the fallback
/// when no other classifier matches.
pub struct Utf8Classifier;

impl Utf8Classifier {
    /// Create a new UTF-8 classifier.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for Utf8Classifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentClassifier for Utf8Classifier {
    fn classify(&self, raw: &[u8], _path: &str) -> Option<ContentType> {
        if std::str::from_utf8(raw).is_ok() {
            Some(ContentType::new(ContentType::UTF8))
        } else {
            None
        }
    }

    fn priority(&self) -> u8 {
        10
    }

    fn name(&self) -> &'static str {
        "utf-8"
    }
}

#[cfg(test)]
#[path = "classifier_tests.rs"]
mod tests;
