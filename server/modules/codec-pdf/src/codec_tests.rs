//! Tests for PDF codec.

use {
    reovim_driver_codec::{ContentCodec, DecodedEdit},
    reovim_driver_vfs::HeapByteSource,
    reovim_types_text::Position,
};

use super::*;

#[test]
fn format_empty_pages() {
    let (content, annotations) = format_pdf_pages(&[]);
    assert_eq!(content, "");
    assert!(annotations.is_empty());
}

#[test]
fn format_single_page() {
    let pages = vec!["Hello, world!".to_string()];
    let (content, annotations) = format_pdf_pages(&pages);
    assert!(content.starts_with("--- Page 1 ---\n"));
    assert!(content.contains("Hello, world!"));
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0].kind.name(), PDF_PAGE_KIND);
}

#[test]
fn format_multiple_pages() {
    let pages = vec![
        "First page content".to_string(),
        "Second page content".to_string(),
        "Third page content".to_string(),
    ];
    let (content, annotations) = format_pdf_pages(&pages);

    assert!(content.contains("--- Page 1 ---"));
    assert!(content.contains("--- Page 2 ---"));
    assert!(content.contains("--- Page 3 ---"));
    assert!(content.contains("First page content"));
    assert!(content.contains("Second page content"));
    assert!(content.contains("Third page content"));
    assert_eq!(annotations.len(), 3);
}

#[test]
fn format_page_annotations_have_page_numbers() {
    let pages = vec!["A".to_string(), "B".to_string()];
    let (_, annotations) = format_pdf_pages(&pages);

    assert_eq!(annotations.len(), 2);
    // Page numbers in payload (1-indexed)
    assert!(matches!(annotations[0].payload, AnnotationPayload::Number(1)));
    assert!(matches!(annotations[1].payload, AnnotationPayload::Number(2)));
}

#[test]
fn format_page_annotations_target_separator_lines() {
    let pages = vec!["Line one\nLine two".to_string(), "Page two".to_string()];
    let (_, annotations) = format_pdf_pages(&pages);

    // First page separator at line 0
    assert!(annotations[0].target.affects_line(0));
    // Second page separator: line 0 (separator) + 2 (content lines) + 1 (blank) = line 4
    assert!(annotations[1].target.affects_line(4));
}

#[test]
fn format_whitespace_only_page() {
    let pages = vec!["  \n  \t  \n  ".to_string()];
    let (content, annotations) = format_pdf_pages(&pages);

    // Should have separator but no content (whitespace trimmed to empty)
    assert!(content.starts_with("--- Page 1 ---\n"));
    assert_eq!(annotations.len(), 1);
}

#[test]
fn format_trims_page_content() {
    let pages = vec!["\n\n  Hello  \n\n".to_string()];
    let (content, _) = format_pdf_pages(&pages);

    assert!(content.contains("Hello"));
    // Leading/trailing whitespace should be trimmed
    assert!(!content.contains("\n\n  Hello"));
}

#[test]
fn format_namespace_is_content() {
    let pages = vec!["text".to_string()];
    let (_, annotations) = format_pdf_pages(&pages);

    for annotation in &annotations {
        assert_eq!(annotation.kind.namespace(), Some("content"));
    }
}

#[test]
fn default_impl() {
    let codec = PdfCodec;
    assert_eq!(std::mem::size_of_val(&codec), std::mem::size_of::<PdfCodec>());
}

#[test]
fn decode_invalid_pdf() {
    let codec = PdfCodec::new();
    let result = codec.decode(b"not a pdf");
    assert!(result.is_err());
}

#[test]
fn translate_edit_text_is_not_supported() {
    use reovim_driver_codec::TranslateEditError;
    let codec = PdfCodec::new();
    let bytes = HeapByteSource::new(b"%PDF-1.7");
    let edit = DecodedEdit::Text {
        start: Position::new(0, 0),
        end: Position::new(0, 0),
        replacement: "x".to_string(),
    };

    assert!(matches!(codec.translate_edit(&bytes, &edit), Err(TranslateEditError::ReadOnly)));
}

#[test]
fn translate_edit_bytes_is_not_supported() {
    use reovim_driver_codec::TranslateEditError;
    let codec = PdfCodec::new();
    let bytes = HeapByteSource::new(b"%PDF-1.7");
    let edit = DecodedEdit::Bytes {
        offset: 0,
        old_len: 1,
        new_bytes: b"y".to_vec(),
    };

    assert!(matches!(codec.translate_edit(&bytes, &edit), Err(TranslateEditError::ReadOnly)));
}

#[test]
fn format_blank_line_between_pages_not_after_last() {
    let pages = vec!["A".to_string(), "B".to_string()];
    let (content, _) = format_pdf_pages(&pages);

    // Should have blank line between pages
    assert!(content.contains("A\n\n--- Page 2 ---"));
    // Should NOT end with double newline
    assert!(content.ends_with("B\n"));
}

#[test]
fn oversized_pdf_returns_truncated() {
    let codec = PdfCodec::new();
    // Create a byte slice just over the limit (we don't need real PDF data
    // because the size check happens before parsing)
    let data = vec![0u8; MAX_PDF_INPUT_BYTES + 1];
    let result = codec.decode(&data).unwrap();
    assert!(result.truncated);
    assert!(result.content.contains("too large"));
    assert!(result.content.contains("100 MB"));
    assert_eq!(result.metadata.get("truncated_at"), Some("0"));
    assert_eq!(
        result.metadata.get("total_size"),
        Some((MAX_PDF_INPUT_BYTES + 1).to_string()).as_deref()
    );
}
