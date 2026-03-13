//! CJK encoding classifier.
//!
//! Detects CJK-encoded files by attempting to decode with each supported
//! encoding and checking for zero replacement characters.

use reovim_driver_codec::{ContentClassifier, ContentType};

/// Maximum bytes to sample for encoding detection.
const SAMPLE_SIZE: usize = 8192;

/// Supported CJK encodings and their content type identifiers.
const CJK_ENCODINGS: &[(&str, &encoding_rs::Encoding)] = &[
    ("encoding/euc-kr", encoding_rs::EUC_KR),
    ("encoding/shift-jis", encoding_rs::SHIFT_JIS),
    ("encoding/gbk", encoding_rs::GBK),
    ("encoding/big5", encoding_rs::BIG5),
];

/// CJK encoding classifier (priority 50).
///
/// Runs before all other classifiers. Tries each CJK encoding via
/// `encoding_rs` and accepts if the decode produces zero replacement
/// characters and the content contains high bytes (0x80+).
///
/// Known limitation: encodings are tried in a fixed order (EUC-KR,
/// Shift-JIS, GBK, Big5). Some byte sequences are valid in multiple
/// encodings, so Big5 or high-range Shift-JIS content may be
/// misidentified as EUC-KR. File extension hints could improve this
/// but are not implemented yet.
pub struct CjkClassifier;

impl CjkClassifier {
    /// Create a new CJK classifier.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CjkClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentClassifier for CjkClassifier {
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

        // Skip if no high bytes (pure ASCII is handled by UTF-8 classifier)
        if !sample.iter().any(|&b| b >= 0x80) {
            return None;
        }

        // Skip if contains null bytes (binary content)
        if sample.contains(&0x00) {
            return None;
        }

        // Skip if valid UTF-8 (UTF-8 classifier will handle it)
        if std::str::from_utf8(sample).is_ok() {
            return None;
        }

        // Try each CJK encoding
        for &(content_type_str, encoding) in CJK_ENCODINGS {
            let (decoded, _, had_errors) = encoding.decode(sample);
            if !had_errors && !decoded.is_empty() {
                return Some(ContentType::new(content_type_str));
            }
        }

        None
    }

    fn priority(&self) -> u8 {
        50
    }

    fn name(&self) -> &'static str {
        "cjk"
    }
}

#[cfg(test)]
#[path = "classifier_tests.rs"]
mod tests;
