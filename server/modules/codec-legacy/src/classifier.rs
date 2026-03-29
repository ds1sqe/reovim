//! Legacy encoding classifier.
//!
//! Detects Latin-1 (ISO-8859-1) and Windows-1252 encoded files by
//! looking for high bytes (0x80+) that are invalid UTF-8 but valid
//! in these legacy encodings.

use reovim_driver_codec::{ContentClassifier, ContentType};

/// Maximum bytes to sample for encoding detection.
const SAMPLE_SIZE: usize = 8192;

/// Bytes in the 0x80..0x9F range that are valid in Windows-1252 but
/// undefined in ISO-8859-1. Their presence indicates Windows-1252.
const CP1252_SPECIFIC: &[u8] = &[
    0x80, // Euro sign
    0x82, // Single low-9 quotation mark
    0x83, // Latin small letter f with hook
    0x84, // Double low-9 quotation mark
    0x85, // Horizontal ellipsis
    0x86, // Dagger
    0x87, // Double dagger
    0x88, // Modifier letter circumflex accent
    0x89, // Per mille sign
    0x8A, // Latin capital letter S with caron
    0x8B, // Single left-pointing angle quotation mark
    0x8C, // Latin capital ligature OE
    0x8E, // Latin capital letter Z with caron
    0x91, // Left single quotation mark
    0x92, // Right single quotation mark
    0x93, // Left double quotation mark
    0x94, // Right double quotation mark
    0x95, // Bullet
    0x96, // En dash
    0x97, // Em dash
    0x98, // Small tilde
    0x99, // Trade mark sign
    0x9A, // Latin small letter s with caron
    0x9B, // Single right-pointing angle quotation mark
    0x9C, // Latin small ligature oe
    0x9E, // Latin small letter z with caron
    0x9F, // Latin capital letter Y with diaeresis
];

/// Content type for Windows-1252 encoding.
pub const WINDOWS_1252: &str = "encoding/windows-1252";

/// Content type for Latin-1 (ISO-8859-1) encoding.
pub const LATIN_1: &str = "encoding/latin-1";

/// Legacy encoding classifier (priority 40).
///
/// Runs after CJK (50) but before binary (20) and UTF-8 (10).
/// Detects Latin-1 and Windows-1252 by checking for high bytes
/// (0x80+) that are invalid UTF-8.
pub struct LegacyClassifier;

impl LegacyClassifier {
    /// Create a new legacy classifier.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for LegacyClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentClassifier for LegacyClassifier {
    fn classify(&self, raw: &[u8], _path: &str) -> Option<ContentType> {
        if raw.is_empty() {
            return None;
        }

        // Sample the first SAMPLE_SIZE bytes, snapping back past any byte
        // matching the UTF-8 continuation pattern (10xxxxxx) to avoid
        // splitting a multi-byte sequence at the sample boundary.
        let sample = if raw.len() > SAMPLE_SIZE {
            let mut end = SAMPLE_SIZE;
            while end > 0 && (raw[end] & 0xC0) == 0x80 {
                end -= 1;
            }
            &raw[..end]
        } else {
            raw
        };

        // Skip if no high bytes (pure ASCII handled by UTF-8)
        if !sample.iter().any(|&b| b >= 0x80) {
            return None;
        }

        // Skip if contains null bytes (binary content)
        if sample.contains(&0x00) {
            return None;
        }

        // Skip if valid UTF-8
        if std::str::from_utf8(sample).is_ok() {
            return None;
        }

        // Check for Windows-1252 specific bytes (0x80..0x9F range)
        let has_cp1252_bytes = sample.iter().any(|b| CP1252_SPECIFIC.contains(b));

        if has_cp1252_bytes {
            Some(ContentType::new(WINDOWS_1252))
        } else {
            // All bytes 0x80..0xFF are valid in Latin-1
            // (Latin-1 maps 1:1 to Unicode code points 0x00..0xFF)
            Some(ContentType::new(LATIN_1))
        }
    }

    fn priority(&self) -> u8 {
        40
    }

    fn name(&self) -> &'static str {
        "legacy"
    }
}

#[cfg(test)]
#[path = "classifier_tests.rs"]
mod tests;
