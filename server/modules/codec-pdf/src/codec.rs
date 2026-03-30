//! PDF text extraction codec.
//!
//! Extracts text from PDF files page by page, producing a text view
//! with page boundary annotations. This is a one-way (decode-only)
//! codec — PDF documents cannot be saved back from extracted text.

use {
    reovim_driver_annotation::{Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget},
    reovim_driver_codec::{CodecError, CodecMetadata, ContentType, DecodeResult},
};

use crate::classifier::PDF;

/// Annotation kind for page separator lines.
pub const PDF_PAGE_KIND: &str = "content.pdf.page";

/// PDF text extraction codec.
///
/// Extracts text from PDF pages using `pdf-extract`. Each page is
/// separated by a header line with the page number. Annotations mark
/// page boundaries and metadata.
///
/// This is a one-way codec: `encode()` returns `None` because PDF
/// content cannot be reconstructed from extracted text.
pub struct PdfCodec;

#[cfg_attr(coverage_nightly, coverage(off))]
impl PdfCodec {
    /// Create a new PDF codec.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for PdfCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_codec::ContentCodec for PdfCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        let pages = pdf_extract::extract_text_from_mem_by_pages(raw)
            .map_err(|e| CodecError::Other(format!("PDF extraction failed: {e}")))?;

        let (content, annotations) = format_pdf_pages(&pages);

        let mut metadata = CodecMetadata::new(ContentType::new(PDF));
        metadata.set("readonly", "true");
        metadata.set("page_count", pages.len().to_string());

        Ok(DecodeResult {
            content,
            annotations,
            metadata,
            lossy: true,
            readonly: true,
        })
    }

    fn encode(
        &self,
        _content: &str,
        _metadata: &CodecMetadata,
    ) -> Option<Result<Vec<u8>, CodecError>> {
        // One-way codec: PDF content cannot be reconstructed from text
        None
    }
}

/// Format extracted PDF pages into text content with annotations.
///
/// Each page gets a separator line (`--- Page N ---`) followed by the
/// extracted text. Annotations mark each page separator line.
fn format_pdf_pages(pages: &[String]) -> (String, Vec<Annotation>) {
    if pages.is_empty() {
        return (String::new(), Vec::new());
    }

    // Estimate capacity: page separator + content per page
    let total_content_len: usize = pages.iter().map(String::len).sum();
    let mut output = String::with_capacity(total_content_len + pages.len() * 20);
    let mut annotations = Vec::with_capacity(pages.len());

    let page_kind = AnnotationKind::new(PDF_PAGE_KIND);
    let mut line_idx = 0;

    for (page_num, page_text) in pages.iter().enumerate() {
        // Page separator line
        let page_label = format!("--- Page {} ---", page_num + 1);
        output.push_str(&page_label);
        output.push('\n');

        annotations.push(Annotation {
            kind: page_kind.clone(),
            target: AnnotationTarget::Line(line_idx),
            priority: 0,
            payload: AnnotationPayload::Number(page_num + 1),
        });

        line_idx += 1;

        // Page content
        let trimmed = page_text.trim();
        if !trimmed.is_empty() {
            output.push_str(trimmed);
            output.push('\n');
            line_idx += trimmed.lines().count();
        }

        // Blank line between pages (except after last page)
        if page_num + 1 < pages.len() {
            output.push('\n');
            line_idx += 1;
        }
    }

    (output, annotations)
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
