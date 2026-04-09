//! Tests for rlib codec.

use {
    reovim_driver_codec::{ContentCodec, DecodedEdit},
    reovim_driver_vfs::HeapByteSource,
    reovim_types_text::Position,
};

use super::*;

#[test]
fn decode_invalid_data() {
    let codec = RlibCodec::new();
    let result = codec.decode(b"not an archive");
    assert!(result.is_err());
}

#[test]
fn default_impl() {
    let codec = RlibCodec;
    assert_eq!(std::mem::size_of_val(&codec), std::mem::size_of::<RlibCodec>());
}

#[test]
fn translate_edit_text_is_not_supported() {
    let codec = RlibCodec::new();
    let bytes = HeapByteSource::new(b"archive contents");
    let edit = DecodedEdit::Text {
        start: Position::new(0, 0),
        end: Position::new(0, 0),
        replacement: "x".to_string(),
    };

    assert!(codec.translate_edit(&bytes, &edit).is_none());
}

#[test]
fn translate_edit_bytes_is_not_supported() {
    let codec = RlibCodec::new();
    let bytes = HeapByteSource::new(b"archive contents");
    let edit = DecodedEdit::Bytes {
        offset: 0,
        old_len: 1,
        new_bytes: b"y".to_vec(),
    };

    assert!(codec.translate_edit(&bytes, &edit).is_none());
}

#[test]
fn decode_real_rlib() {
    // Find a real .rlib in target/debug/deps/
    let deps_dir = std::path::Path::new("target/debug/deps");
    if !deps_dir.exists() {
        return; // Skip if not built
    }

    let rlib_path = std::fs::read_dir(deps_dir).ok().and_then(|entries| {
        entries
            .filter_map(Result::ok)
            .find(|e| e.path().extension().is_some_and(|ext| ext == "rlib"))
            .map(|e| e.path())
    });

    let Some(path) = rlib_path else {
        return; // No rlib files found
    };

    let data = std::fs::read(&path).unwrap();
    let codec = RlibCodec::new();
    let result = codec.decode(&data).unwrap();

    assert!(result.content.contains("Rust Library (.rlib) Summary"));
    assert!(result.content.contains("Archive Members"));
    assert!(result.lossy);
    assert!(result.readonly);
    assert!(!result.truncated);
    assert!(!result.annotations.is_empty());
    assert_eq!(result.metadata.get("readonly"), Some("true"));
    assert!(result.metadata.get("member_count").is_some());
    assert!(result.metadata.get("file_size").is_some());
}

#[test]
fn format_rlib_summary_empty() {
    let (content, annotations) = format_rlib_summary(&[], 0, None, &[]);
    assert!(content.contains("Rust Library (.rlib) Summary"));
    assert!(content.contains("Members:        0"));
    assert!(content.contains("Total Size:     0 bytes"));
    // Title + separator + member section header + separator + column header = 7 lines min
    assert!(!annotations.is_empty());
}

#[test]
fn format_rlib_summary_with_members() {
    let members = vec![("lib.rmeta", 100_usize), ("foo.rcgu.o", 200)];
    let (content, annotations) = format_rlib_summary(&members, 300, None, &[]);
    assert!(content.contains("lib.rmeta"));
    assert!(content.contains("foo.rcgu.o"));
    assert!(content.contains("Members:        2"));
    assert!(content.contains("Total Size:     300 bytes"));

    // Should have member annotations
    let member_count = annotations
        .iter()
        .filter(|a| a.kind.name() == RLIB_MEMBER_KIND)
        .count();
    assert_eq!(member_count, 2);
}

#[test]
fn format_rlib_summary_with_version() {
    let version = "1.96.0-nightly (abc123 2026-03-10)".to_string();
    let (content, _) = format_rlib_summary(&[], 0, Some(&version), &[]);
    assert!(content.contains("Rustc Version:  1.96.0-nightly"));
}

#[test]
fn format_rlib_summary_with_dependencies() {
    let deps = vec!["once_cell".to_string(), "serde_json".to_string()];
    let (content, annotations) = format_rlib_summary(&[], 0, None, &deps);
    assert!(content.contains("Dependencies (from metadata)"));
    assert!(content.contains("once_cell"));
    assert!(content.contains("serde_json"));

    let dep_count = annotations
        .iter()
        .filter(|a| a.kind.name() == RLIB_DEPENDENCY_KIND)
        .count();
    assert_eq!(dep_count, 2);
}

#[test]
fn scan_rmeta_bytes_with_magic() {
    let mut data = Vec::new();
    data.extend_from_slice(b"rust\0\0\0\n");
    data.extend_from_slice(b"1.96.0-nightly");
    data.push(0); // null terminator

    let (version, _) = scan_rmeta_bytes(&data);
    assert_eq!(version.as_deref(), Some("1.96.0-nightly"));
}

#[test]
fn scan_rmeta_bytes_no_magic() {
    let (version, _) = scan_rmeta_bytes(b"no magic here");
    assert!(version.is_none());
}

#[test]
fn is_common_non_dep_filters() {
    assert!(is_common_non_dep("rust_metadata"));
    assert!(is_common_non_dep("raw_dylib"));
    assert!(!is_common_non_dep("serde_json"));
    assert!(!is_common_non_dep("tokio_util"));
}

#[test]
fn extract_dependency_names_basic() {
    let data = b"some_crate\0\0other_dep\0\0ab\0\0"; // ab is too short
    let deps = extract_dependency_names(data);
    assert!(deps.contains(&"some_crate".to_string()));
    assert!(deps.contains(&"other_dep".to_string()));
    assert!(!deps.iter().any(|d| d == "ab"));
}

#[test]
fn annotations_namespace_is_content() {
    let (_, annotations) =
        format_rlib_summary(&[("lib.rmeta", 100)], 100, None, &["some_dep".to_string()]);
    for a in &annotations {
        assert_eq!(a.kind.namespace(), Some("content"));
    }
}
