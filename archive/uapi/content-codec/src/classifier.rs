//! Content classifier trait.
//!
//! Classifiers examine raw file bytes and optional file path to determine
//! the content type. Multiple classifiers are registered with different
//! priorities; the first one to return `Some` wins.
//!
//! # Priority Convention
//!
//! Higher priority classifiers are tried first:
//! - 50: CJK encodings (most specific byte patterns)
//! - 40: Legacy Western encodings
//! - 20: Binary detection (catch-all for non-text)
//! - 10: UTF-8 (lowest — fallback for valid UTF-8)

use crate::ContentType;

/// Trait for classifying file content type from raw bytes.
///
/// Implementations examine raw bytes (and optionally the file path) to
/// determine whether they can identify the content encoding. Each classifier
/// has a priority that determines evaluation order.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` for use across async tasks.
///
/// # Examples
///
/// ```ignore
/// struct Utf8Classifier;
///
/// impl ContentClassifier for Utf8Classifier {
///     fn classify(&self, raw: &[u8], _path: &str) -> Option<ContentType> {
///         if std::str::from_utf8(raw).is_ok() {
///             Some(ContentType::new("text/utf-8"))
///         } else {
///             None
///         }
///     }
///
///     fn priority(&self) -> u8 { 10 }
///     fn name(&self) -> &str { "utf-8" }
/// }
/// ```
pub trait ContentClassifier: Send + Sync {
    /// Attempt to classify the content type of raw bytes.
    ///
    /// # Arguments
    ///
    /// * `raw` — The raw file bytes (may be truncated for large files)
    /// * `path` — The file path (for extension-based heuristics)
    ///
    /// # Returns
    ///
    /// `Some(content_type)` if this classifier recognizes the content,
    /// `None` to defer to lower-priority classifiers.
    fn classify(&self, raw: &[u8], path: &str) -> Option<ContentType>;

    /// Priority for classifier ordering (higher = tried first).
    ///
    /// Convention:
    /// - 50: CJK encodings
    /// - 40: Legacy Western encodings
    /// - 20: Binary detection
    /// - 10: UTF-8 (fallback)
    fn priority(&self) -> u8 {
        50
    }

    /// Human-readable name for this classifier.
    fn name(&self) -> &'static str;
}

#[cfg(test)]
#[path = "classifier_tests.rs"]
mod tests;
