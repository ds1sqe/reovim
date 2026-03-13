//! Tests for PDF codec.

use reovim_driver_codec::{CodecMetadata, ContentCodec, ContentType};

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
fn encode_returns_none() {
    let codec = PdfCodec::new();
    let metadata = CodecMetadata::new(ContentType::new(PDF));
    assert!(codec.encode("any content", &metadata).is_none());
}

#[test]
fn default_impl() {
    let codec = PdfCodec;
    let metadata = CodecMetadata::new(ContentType::new(PDF));
    assert!(codec.encode("x", &metadata).is_none());
}

#[test]
fn decode_invalid_pdf() {
    let codec = PdfCodec::new();
    let result = codec.decode(b"not a pdf");
    assert!(result.is_err());
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
