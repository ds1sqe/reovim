//! PDF content classifier.
//!
//! Detects PDF files by checking for the `%PDF-` magic bytes at offset 0.

use reovim_content_codec::{ContentClassifier, ContentType};

/// Content type for PDF documents.
pub const PDF: &str = "document/pdf";

/// Known PDF file extensions (fast-path, no content scanning needed).
const PDF_EXTENSIONS: &[&str] = &["pdf"];

/// PDF content classifier (priority 35).
///
/// Runs after text encoding classifiers (CJK 50, Legacy 40) but before
/// the generic binary classifier (Hex 20). PDFs contain null bytes, so
/// without this classifier they would fall through to hex dump view.
pub struct PdfClassifier;

impl PdfClassifier {
    /// Create a new PDF classifier.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for PdfClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentClassifier for PdfClassifier {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn classify(&self, raw: &[u8], path: &str) -> Option<ContentType> {
        // Fast-path: known PDF extension
        if has_pdf_extension(path) {
            return Some(ContentType::new(PDF));
        }

        // Magic byte detection: %PDF- at offset 0
        if raw.len() >= 5 && &raw[..5] == b"%PDF-" {
            return Some(ContentType::new(PDF));
        }

        None
    }

    fn priority(&self) -> u8 {
        35
    }

    fn name(&self) -> &'static str {
        "pdf"
    }
}

/// Check if the file path has a PDF extension.
#[cfg_attr(coverage_nightly, coverage(off))]
fn has_pdf_extension(path: &str) -> bool {
    let Some(ext) = std::path::Path::new(path).extension() else {
        return false;
    };
    let Some(ext_str) = ext.to_str() else {
        return false;
    };
    let lower = ext_str.to_ascii_lowercase();
    PDF_EXTENSIONS.contains(&lower.as_str())
}

#[cfg(test)]
#[path = "classifier_tests.rs"]
mod tests;
