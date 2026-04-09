//! Tests for PDF codec.

use std::sync::Arc;

use {
    reovim_driver_codec::{
        ContentCodec, DecodedEdit, InodeTable, Mount, TranslateEditError, TreeOp, TreePath,
    },
    reovim_driver_vfs::HeapByteSource,
    reovim_kernel::api::v1::BufferId,
    reovim_types_text::Position,
};

use super::*;

// ---------------------------------------------------------------------------
// Fixture helpers

/// Build a minimal valid PDF with the given Info dictionary metadata fields.
///
/// The PDF has no pages (lopdf round-trips it fine for structural editing;
/// pdf-extract requires real pages, but decode tests are coverage-off).
fn build_test_pdf(metadata: &[(&str, &str)]) -> Vec<u8> {
    let mut doc = lopdf::Document::with_version("1.7");

    let mut info = lopdf::Dictionary::new();
    for &(key, value) in metadata {
        info.set(key, lopdf::Object::string_literal(value));
    }

    let info_id = doc.add_object(lopdf::Object::Dictionary(info));
    doc.trailer.set("Info", lopdf::Object::Reference(info_id));

    let mut output = Vec::new();
    doc.save_to(&mut output).unwrap();
    output
}

/// Build a minimal valid PDF with no Info dictionary.
fn build_test_pdf_no_info() -> Vec<u8> {
    let mut doc = lopdf::Document::with_version("1.7");
    let mut output = Vec::new();
    doc.save_to(&mut output).unwrap();
    output
}

/// Convenience: create a metadata set-edit at the given path.
fn metadata_edit(path_field: &str, value: &str) -> DecodedEdit {
    DecodedEdit::Tree {
        path: TreePath::new(vec!["metadata".to_string(), path_field.to_string()]),
        op: TreeOp::new(PdfTreeOp::SetMetadata {
            value: value.to_string(),
        }),
    }
}

/// Read the metadata field from raw PDF bytes via lopdf.
fn read_metadata_field(pdf_bytes: &[u8], field: &str) -> Option<String> {
    let doc = lopdf::Document::load_mem(pdf_bytes).ok()?;
    let info_id = doc.trailer.get(b"Info").ok()?.as_reference().ok()?;
    let info = doc.get_dictionary(info_id).ok()?;
    let value = info.get(field.as_bytes()).ok()?.as_str().ok()?;
    String::from_utf8(value.to_vec()).ok()
}

// ===========================================================================
// Existing format_pdf_pages tests (preserved from Plan 06)
// ===========================================================================

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

// ===========================================================================
// Phase 6: translate_edit refusal paths
// ===========================================================================

#[test]
fn translate_edit_text_is_not_supported() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Title", "Test")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = DecodedEdit::Text {
        start: Position::new(0, 0),
        end: Position::new(0, 0),
        replacement: "x".to_string(),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn translate_edit_bytes_is_not_supported() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Title", "Test")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = DecodedEdit::Bytes {
        offset: 0,
        old_len: 1,
        new_bytes: b"y".to_vec(),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn translate_edit_wrong_tree_op_type() {
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct UnrelatedOp;
    reovim_driver_codec::impl_tree_op!(UnrelatedOp);

    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Title", "Test")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["metadata".to_string(), "Title".to_string()]),
        op: TreeOp::new(UnrelatedOp),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn translate_edit_malformed_path_root() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Title", "Test")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = DecodedEdit::Tree {
        path: TreePath::root(),
        op: TreeOp::new(PdfTreeOp::SetMetadata {
            value: "x".to_string(),
        }),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn translate_edit_malformed_path_single_component() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Title", "Test")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["metadata".to_string()]),
        op: TreeOp::new(PdfTreeOp::SetMetadata {
            value: "x".to_string(),
        }),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn translate_edit_malformed_path_three_components() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Title", "Test")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "metadata".to_string(),
            "Title".to_string(),
            "extra".to_string(),
        ]),
        op: TreeOp::new(PdfTreeOp::SetMetadata {
            value: "x".to_string(),
        }),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn translate_edit_malformed_path_wrong_kind() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Title", "Test")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["pages".to_string(), "1".to_string()]),
        op: TreeOp::new(PdfTreeOp::SetMetadata {
            value: "x".to_string(),
        }),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn translate_edit_malformed_path_empty_field() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Title", "Test")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["metadata".to_string(), String::new()]),
        op: TreeOp::new(PdfTreeOp::SetMetadata {
            value: "x".to_string(),
        }),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn translate_edit_unsupported_metadata_field() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Title", "Test")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = metadata_edit("CreationDate", "2026-01-01");

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn translate_edit_no_info_dictionary() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf_no_info();
    let bytes = HeapByteSource::new(pdf);
    let edit = metadata_edit("Title", "New Title");

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}

#[test]
fn translate_edit_invalid_pdf_bytes() {
    let codec = PdfCodec::new();
    let bytes = HeapByteSource::new(b"not a pdf at all");
    let edit = metadata_edit("Title", "New Title");

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::Internal { .. })
    ));
}

// ===========================================================================
// Phase 6: positive metadata editing
// ===========================================================================

#[test]
fn set_metadata_title() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Title", "Old Title"), ("Author", "Alice")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = metadata_edit("Title", "New Title");

    let result = codec.translate_edit(&bytes, &edit);
    let byte_edit = result.unwrap().expect("should produce a ByteEdit");

    // Apply the byte edit to get new PDF bytes.
    let new_pdf = &byte_edit.new_bytes;

    // Verify the title was updated.
    assert_eq!(read_metadata_field(new_pdf, "Title"), Some("New Title".to_string()));
    // Author should be preserved.
    assert_eq!(read_metadata_field(new_pdf, "Author"), Some("Alice".to_string()));
}

#[test]
fn set_metadata_author() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Title", "My PDF"), ("Author", "Old Author")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = metadata_edit("Author", "New Author");

    let byte_edit = codec.translate_edit(&bytes, &edit).unwrap().unwrap();
    let new_pdf = &byte_edit.new_bytes;

    assert_eq!(read_metadata_field(new_pdf, "Author"), Some("New Author".to_string()));
    // Title should be preserved.
    assert_eq!(read_metadata_field(new_pdf, "Title"), Some("My PDF".to_string()));
}

#[test]
fn set_metadata_subject() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Subject", "Old Subject")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = metadata_edit("Subject", "New Subject");

    let byte_edit = codec.translate_edit(&bytes, &edit).unwrap().unwrap();
    assert_eq!(
        read_metadata_field(&byte_edit.new_bytes, "Subject"),
        Some("New Subject".to_string())
    );
}

#[test]
fn set_metadata_keywords() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Keywords", "rust, editor")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = metadata_edit("Keywords", "rust, editor, pdf");

    let byte_edit = codec.translate_edit(&bytes, &edit).unwrap().unwrap();
    assert_eq!(
        read_metadata_field(&byte_edit.new_bytes, "Keywords"),
        Some("rust, editor, pdf".to_string())
    );
}

#[test]
fn set_metadata_creator() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Creator", "OldTool")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = metadata_edit("Creator", "NewTool");

    let byte_edit = codec.translate_edit(&bytes, &edit).unwrap().unwrap();
    assert_eq!(
        read_metadata_field(&byte_edit.new_bytes, "Creator"),
        Some("NewTool".to_string())
    );
}

#[test]
fn set_metadata_producer() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Producer", "OldLib")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = metadata_edit("Producer", "NewLib");

    let byte_edit = codec.translate_edit(&bytes, &edit).unwrap().unwrap();
    assert_eq!(
        read_metadata_field(&byte_edit.new_bytes, "Producer"),
        Some("NewLib".to_string())
    );
}

#[test]
fn set_metadata_creates_new_field() {
    let codec = PdfCodec::new();
    // PDF has Title but no Subject.
    let pdf = build_test_pdf(&[("Title", "Test")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = metadata_edit("Subject", "Added Subject");

    let byte_edit = codec.translate_edit(&bytes, &edit).unwrap().unwrap();
    let new_pdf = &byte_edit.new_bytes;

    // Subject should now exist.
    assert_eq!(read_metadata_field(new_pdf, "Subject"), Some("Added Subject".to_string()));
    // Title should be preserved.
    assert_eq!(read_metadata_field(new_pdf, "Title"), Some("Test".to_string()));
}

// ===========================================================================
// Phase 6: no-op detection
// ===========================================================================

#[test]
fn set_metadata_noop_same_value() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Title", "Same Title")]);
    let bytes = HeapByteSource::new(pdf);
    let edit = metadata_edit("Title", "Same Title");

    let result = codec.translate_edit(&bytes, &edit).unwrap();
    assert!(result.is_none(), "same value should be a no-op");
}

// ===========================================================================
// Phase 6: re-parse after edit
// ===========================================================================

#[test]
fn re_parse_after_metadata_edit() {
    let codec = PdfCodec::new();
    let pdf = build_test_pdf(&[("Title", "Before"), ("Author", "Alice")]);
    let bytes = HeapByteSource::new(pdf);

    let edit = metadata_edit("Title", "After");
    let byte_edit = codec.translate_edit(&bytes, &edit).unwrap().unwrap();
    let new_pdf = &byte_edit.new_bytes;

    // The new bytes should be valid PDF loadable by lopdf.
    let doc = lopdf::Document::load_mem(new_pdf).expect("re-parse should succeed");
    let info_id = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
    let info = doc.get_dictionary(info_id).unwrap();

    let title = info.get(b"Title").unwrap().as_str().unwrap();
    assert_eq!(title, b"After");
    let author = info.get(b"Author").unwrap().as_str().unwrap();
    assert_eq!(author, b"Alice");
}

// ===========================================================================
// Phase 6: undo correctness
// ===========================================================================

#[test]
fn undo_restores_original_bytes() {
    let codec = PdfCodec::new();
    let original = build_test_pdf(&[("Title", "Original")]);
    let bytes = HeapByteSource::new(original.clone());

    let edit = metadata_edit("Title", "Changed");
    let byte_edit = codec.translate_edit(&bytes, &edit).unwrap().unwrap();

    // Verify edit changed the bytes.
    assert_ne!(&byte_edit.new_bytes, &original);

    // Apply inverse.
    let inverse = byte_edit.inverse();
    assert_eq!(inverse.new_bytes, original);
}

// ===========================================================================
// Phase 6: InodeTable peer-stale propagation
// ===========================================================================

#[test]
fn peer_stale_propagation_via_inode_table() {
    let codec = Arc::new(PdfCodec::new());
    let pdf = build_test_pdf(&[("Title", "Original"), ("Author", "Test")]);

    let mut table = InodeTable::new();
    let inode_id = table.insert(Arc::new(HeapByteSource::new(pdf.clone())));
    let buffer_id = BufferId::from_raw(1);
    table.bind_file(buffer_id, inode_id);

    let source_mount = table
        .mount(inode_id, buffer_id, Mount::new("pdf-source", codec.clone()))
        .unwrap();
    let peer_mount = table
        .mount_additional(inode_id, buffer_id, Mount::new("pdf-peer", codec))
        .unwrap();

    // Verify initial bytes.
    assert_eq!(table.read_bytes(inode_id).unwrap(), pdf);

    // Apply metadata edit.
    let edit = metadata_edit("Title", "Updated");
    let byte_edit = table.apply_edit(source_mount, &edit).unwrap().unwrap();

    // Bytes should have changed.
    let current = table.read_bytes(inode_id).unwrap();
    assert_ne!(current, pdf);

    // Peer mount should be marked stale.
    let inode = table.lookup_inode(inode_id).unwrap();
    let peer = inode.mounts.get(&peer_mount.mount_id()).unwrap();
    assert!(!peer.content_valid);

    // Apply inverse to restore original.
    let inverse = byte_edit.inverse();
    table.apply_byte_edit(inode_id, &inverse).unwrap();

    let restored = table.read_bytes(inode_id).unwrap();
    assert_eq!(restored, pdf);
}

// ===========================================================================
// Phase 6: resolve_metadata_path unit tests
// ===========================================================================

#[test]
fn resolve_path_all_supported_fields() {
    for &field in SUPPORTED_METADATA_FIELDS {
        let path = TreePath::new(vec!["metadata".to_string(), field.to_string()]);
        let result = resolve_metadata_path(&path);
        assert!(result.is_ok(), "field {field} should be supported");
        assert_eq!(result.unwrap().field, field);
    }
}

#[test]
fn resolve_path_case_sensitive() {
    // Field names are case-sensitive per PDF spec.
    let path = TreePath::new(vec!["metadata".to_string(), "title".to_string()]);
    assert!(matches!(
        resolve_metadata_path(&path),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}
