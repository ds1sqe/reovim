//! PDF structured codec.
//!
//! Extracts text from PDF files page by page, producing a text view
//! with page boundary annotations. Structural editing supports metadata
//! field updates via `lopdf` parse → modify → serialize (Plan 07 Phase 6).

use {
    reovim_driver_annotation::{Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget},
    reovim_content_codec::{
        CodecError, CodecMetadata, ContentCodec, ContentType, DecodeResult, DecodedEdit,
        TranslateEditError, TreePath, impl_tree_op,
    },
    reovim_kernel::api::v1::ByteEdit,
    reovim_subsys_vfs::ByteSource,
};

use crate::classifier::PDF;

/// Maximum PDF input size (100 MB).
///
/// PDFs are compressed, so in-memory expansion is bounded. However,
/// extremely large PDFs can still be slow to parse. This hard limit
/// prevents excessive memory use during extraction.
const MAX_PDF_INPUT_BYTES: usize = 100 * 1024 * 1024;

/// Annotation kind for page separator lines.
pub const PDF_PAGE_KIND: &str = "content.pdf.page";

/// PDF structural edit operations (Plan 07 Phase 6).
///
/// Phase 6 supports metadata-only editing: setting string values in the
/// PDF Info dictionary. The tree path determines which field to edit:
/// `['metadata', '<field>']` where `<field>` is one of Title, Author,
/// Subject, Keywords, Creator, or Producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfTreeOp {
    /// Set a metadata field in the PDF Info dictionary.
    ///
    /// The tree path identifies the field (`['metadata', 'Title']`, etc.).
    /// The `value` replaces the current string content of that field.
    SetMetadata {
        /// New string value for the metadata field.
        value: String,
    },
}

impl_tree_op!(PdfTreeOp);

/// Standard PDF Info dictionary fields supported for structural editing.
const SUPPORTED_METADATA_FIELDS: &[&str] = &[
    "Title", "Author", "Subject", "Keywords", "Creator", "Producer",
];

/// PDF text extraction and metadata editing codec.
///
/// Decodes PDF files into page-by-page text with annotations. Structural
/// editing modifies Info dictionary metadata via `lopdf`
/// parse → modify → serialize, producing a full-file [`ByteEdit`].
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

impl ContentCodec for PdfCodec {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        if raw.len() > MAX_PDF_INPUT_BYTES {
            let size_mb = raw.len() / (1024 * 1024);
            let limit_mb = MAX_PDF_INPUT_BYTES / (1024 * 1024);
            let content = format!(
                "PDF file too large ({size_mb} MB). Maximum supported size is {limit_mb} MB."
            );
            let mut metadata = CodecMetadata::new(ContentType::new(PDF));
            metadata.set("readonly", "true");
            metadata.set("truncated_at", "0");
            metadata.set("total_size", raw.len().to_string());
            return Ok(DecodeResult {
                content,
                annotations: Vec::new(),
                metadata,
                lossy: true,
                readonly: true,
                truncated: true,
            });
        }

        let pages = pdf_extract::extract_text_from_mem_by_pages(raw)
            .map_err(|e| CodecError::Other(format!("PDF extraction failed: {e}")))?;

        let (content, annotations) = format_pdf_pages(&pages);

        let mut metadata = CodecMetadata::new(ContentType::new(PDF));
        metadata.set("readonly", "false");
        metadata.set("page_count", pages.len().to_string());

        Ok(DecodeResult {
            content,
            annotations,
            metadata,
            lossy: true,
            readonly: false,
            truncated: false,
        })
    }

    fn translate_edit(
        &self,
        bytes: &dyn ByteSource,
        edit: &DecodedEdit,
    ) -> Result<Option<ByteEdit>, TranslateEditError> {
        match edit {
            DecodedEdit::Tree { path, op } => op.downcast_ref::<PdfTreeOp>().map_or(
                Err(TranslateEditError::UnsupportedEdit {
                    reason: "pdf codec only accepts Pdf tree operations",
                }),
                |pdf_op| translate_pdf_edit(bytes, path, pdf_op),
            ),
            // Domain, raw byte, and any future non_exhaustive variants are not
            // supported by the structural PDF codec.
            DecodedEdit::Domain(_) | DecodedEdit::Bytes { .. } | _ => {
                Err(TranslateEditError::UnsupportedEdit {
                    reason: "pdf codec does not translate domain, raw byte, or unknown edits",
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Metadata path resolution

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedMetadataPath {
    field: String,
}

fn resolve_metadata_path(path: &TreePath) -> Result<ResolvedMetadataPath, TranslateEditError> {
    let components = path.components();
    let [kind, field] = components else {
        return Err(TranslateEditError::MalformedPath {
            reason: "pdf metadata path must be [metadata, <field>]",
        });
    };
    if kind != "metadata" || field.is_empty() {
        return Err(TranslateEditError::MalformedPath {
            reason: "pdf metadata path must be [metadata, <field>]",
        });
    }

    if !SUPPORTED_METADATA_FIELDS.contains(&field.as_str()) {
        return Err(TranslateEditError::UnsupportedEdit {
            reason: "pdf metadata field is not supported for editing",
        });
    }

    Ok(ResolvedMetadataPath {
        field: field.clone(),
    })
}

// ---------------------------------------------------------------------------
// Core translate_edit dispatch

fn translate_pdf_edit(
    bytes: &dyn ByteSource,
    path: &TreePath,
    op: &PdfTreeOp,
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let raw = read_all_bytes(bytes).ok_or(TranslateEditError::Internal {
        reason: "pdf byte source could not be fully read",
    })?;

    let mut doc = lopdf::Document::load_mem(&raw).map_err(|_| TranslateEditError::Internal {
        reason: "pdf parse failed during translate_edit",
    })?;

    let resolved = resolve_metadata_path(path)?;

    match op {
        PdfTreeOp::SetMetadata { value } => {
            translate_set_metadata(&raw, &mut doc, &resolved.field, value)
        }
    }
}

// ---------------------------------------------------------------------------
// SetMetadata

fn translate_set_metadata(
    original: &[u8],
    doc: &mut lopdf::Document,
    field: &str,
    value: &str,
) -> Result<Option<ByteEdit>, TranslateEditError> {
    let info_id = doc
        .trailer
        .get(b"Info")
        .map_err(|_| TranslateEditError::ConstraintViolation {
            reason: "pdf has no Info dictionary in trailer",
        })?
        .as_reference()
        .map_err(|_| TranslateEditError::Internal {
            reason: "pdf Info trailer entry is not an object reference",
        })?;

    let info_dict = doc
        .get_dictionary_mut(info_id)
        .map_err(|_| TranslateEditError::Internal {
            reason: "pdf Info object is not a dictionary",
        })?;

    // Check for no-op: if field exists and has the same byte content.
    // Use chained methods instead of a let-chain to avoid an unreachable
    // MC/DC condition on the compound `let Ok(..) && let Ok(..) && ==`.
    let is_noop = info_dict
        .get(field.as_bytes())
        .ok()
        .and_then(|existing| existing.as_str().ok())
        .is_some_and(|existing_bytes| existing_bytes == value.as_bytes());
    if is_noop {
        return Ok(None);
    }

    // Set the metadata field.
    info_dict.set(field, lopdf::Object::string_literal(value));

    // Serialize back.
    let mut output = Vec::new();
    doc.save_to(&mut output)
        .map_err(|_| pdf_serialize_error())?;

    Ok(Some(ByteEdit::replace(0, original, &output)))
}

// ---------------------------------------------------------------------------
// Helpers

/// Error factory for PDF serialization failure in `translate_set_metadata`.
///
/// `lopdf::Document::save_to` writing into a `Vec<u8>` cannot fail at the I/O
/// level.  The only lopdf error paths require a writer that returns an I/O
/// error, which `Vec<u8>` never does.  This path is therefore genuinely
/// unreachable in production; the helper is extracted so that `coverage(off)`
/// applies only to the dead code.
#[cfg_attr(coverage_nightly, coverage(off))]
const fn pdf_serialize_error() -> TranslateEditError {
    TranslateEditError::Internal {
        reason: "pdf serialization failed",
    }
}

fn read_all_bytes(bytes: &dyn ByteSource) -> Option<Vec<u8>> {
    let len = usize::try_from(bytes.len()).ok()?;
    let data = bytes.read(0..bytes.len()).into_owned();
    (data.len() == len).then_some(data)
}

// ---------------------------------------------------------------------------
// Decode output formatting

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
