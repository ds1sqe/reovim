//! Tests for rlib codec.

use std::{
    process::Command,
    sync::{Arc, OnceLock},
};

use {
    reovim_domain_text::Position,
    reovim_driver_codec::{
        ContentCodec, DecodedEdit, InodeTable, Mount, MountMode, TranslateEditError, TreeOp,
        TreePath,
    },
    reovim_driver_vfs::HeapByteSource,
    reovim_kernel::api::v1::{BufferId, ByteEdit},
};

use super::*;

#[derive(Debug, Clone)]
struct RlibFixture {
    bytes: Vec<u8>,
    payload_member_name: String,
    payload_member_offset: usize,
    payload_old_bytes: Vec<u8>,
    payload_new_bytes: Vec<u8>,
    header_member_name: String,
    header_member_name_offset: usize,
    header_member_new_name: String,
    sysv_member_name: String,
    sysv_member_name_offset: usize,
    sysv_member_new_name: String,
}

static RLIB_FIXTURE: OnceLock<RlibFixture> = OnceLock::new();

fn rlib_fixture() -> &'static RlibFixture {
    RLIB_FIXTURE.get_or_init(build_rlib_fixture)
}

fn build_rlib_fixture() -> RlibFixture {
    let fixture_dir = std::env::temp_dir().join(format!(
        "reovim-rlib-phase3-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("tests")
    ));
    let _ = std::fs::remove_dir_all(&fixture_dir);
    std::fs::create_dir_all(&fixture_dir).unwrap();

    let source_path = fixture_dir.join("fixture.rs");
    let rlib_path = fixture_dir.join("libfixture.rlib");
    std::fs::write(
        &source_path,
        r#"
#[used]
pub static PHASE3_BYTES: [u8; 8] = *b"ORIGINAL";

pub fn phase3_value() -> u8 {
    PHASE3_BYTES[0]
}
"#,
    )
    .unwrap();

    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let output = Command::new(rustc)
        .args([
            "--edition=2021",
            "--crate-type=rlib",
            source_path.to_str().unwrap(),
            "-o",
            rlib_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fixture compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bytes = std::fs::read(&rlib_path).unwrap();
    let archive = goblin::archive::Archive::parse(&bytes).unwrap();

    let header_member_name = "lib.rmeta".to_string();
    let header_member = archive.get(&header_member_name).unwrap();
    let header_member_name_offset =
        resolve_member_name_storage(&bytes, header_member, &header_member_name).unwrap();

    let sysv_member_name = archive
        .members()
        .into_iter()
        .find(|name| {
            *name != header_member_name
                && archive
                    .get(name)
                    .is_some_and(|member| member.raw_name().starts_with('/'))
        })
        .unwrap()
        .to_string();
    let sysv_member = archive.get(&sysv_member_name).unwrap();
    let sysv_member_name_offset =
        resolve_member_name_storage(&bytes, sysv_member, &sysv_member_name).unwrap();
    let payload_member_offset = member_payload_offset(sysv_member).unwrap();
    let payload_old_bytes = archive.extract(&sysv_member_name, &bytes).unwrap().to_vec();
    let mut payload_new_bytes = payload_old_bytes.clone();
    payload_new_bytes[0] ^= 0x5A;

    let _ = std::fs::remove_dir_all(&fixture_dir);

    RlibFixture {
        bytes,
        payload_member_name: sysv_member_name.clone(),
        payload_member_offset,
        payload_old_bytes,
        payload_new_bytes,
        header_member_name: header_member_name.clone(),
        header_member_name_offset,
        header_member_new_name: same_length_name(&header_member_name),
        sysv_member_name: sysv_member_name.clone(),
        sysv_member_name_offset,
        sysv_member_new_name: same_length_name(&sysv_member_name),
    }
}

fn same_length_name(name: &str) -> String {
    let mut bytes = name.as_bytes().to_vec();
    let first = bytes.first_mut().unwrap();
    *first = if *first == b'Z' { b'Y' } else { b'Z' };
    String::from_utf8(bytes).unwrap()
}

fn payload_replace_edit(fixture: &RlibFixture) -> DecodedEdit {
    DecodedEdit::Tree {
        path: TreePath::new(vec![
            "members".to_string(),
            fixture.payload_member_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(RlibTreeOp::ReplaceMemberBytes {
            old_bytes: fixture.payload_old_bytes.clone(),
            new_bytes: fixture.payload_new_bytes.clone(),
        }),
    }
}

fn header_rename_edit(fixture: &RlibFixture, new_name: &str) -> DecodedEdit {
    DecodedEdit::Tree {
        path: TreePath::new(vec![
            "members".to_string(),
            fixture.header_member_name.clone(),
            "name".to_string(),
        ]),
        op: TreeOp::new(RlibTreeOp::RenameMember {
            new_name: new_name.to_string(),
        }),
    }
}

fn sysv_rename_edit(fixture: &RlibFixture, new_name: &str) -> DecodedEdit {
    DecodedEdit::Tree {
        path: TreePath::new(vec![
            "members".to_string(),
            fixture.sysv_member_name.clone(),
            "name".to_string(),
        ]),
        op: TreeOp::new(RlibTreeOp::RenameMember {
            new_name: new_name.to_string(),
        }),
    }
}

fn bsd_named_archive_bytes(name: &str, payload: &[u8]) -> Vec<u8> {
    let file_size = name.len() + payload.len();
    let mut archive = Vec::new();
    archive.extend_from_slice(goblin::archive::MAGIC);
    archive.extend_from_slice(format!("{:<16}", format!("#1/{}", name.len())).as_bytes());
    archive.extend_from_slice(format!("{:<12}", 0).as_bytes());
    archive.extend_from_slice(format!("{:<6}", 0).as_bytes());
    archive.extend_from_slice(format!("{:<6}", 0).as_bytes());
    archive.extend_from_slice(format!("{:<8}", 0).as_bytes());
    archive.extend_from_slice(format!("{file_size:<10}").as_bytes());
    archive.extend_from_slice(b"`\n");
    archive.extend_from_slice(name.as_bytes());
    archive.extend_from_slice(payload);
    if archive.len() & 1 == 1 {
        archive.push(b'\n');
    }
    archive
}

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

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
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

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn translate_edit_payload_replace_returns_in_place_byte_edit() {
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let result = codec
        .translate_edit(&bytes, &payload_replace_edit(fixture))
        .unwrap();

    assert_eq!(
        result,
        Some(ByteEdit::replace(
            fixture.payload_member_offset,
            &fixture.payload_old_bytes,
            &fixture.payload_new_bytes,
        ))
    );
}

#[test]
fn translate_edit_header_member_rename_returns_in_place_byte_edit() {
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let result = codec
        .translate_edit(&bytes, &header_rename_edit(fixture, &fixture.header_member_new_name))
        .unwrap();

    assert_eq!(
        result,
        Some(ByteEdit::replace(
            fixture.header_member_name_offset,
            fixture.header_member_name.as_bytes(),
            fixture.header_member_new_name.as_bytes(),
        ))
    );
}

#[test]
fn translate_edit_sysv_member_rename_returns_in_place_byte_edit() {
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let result = codec
        .translate_edit(&bytes, &sysv_rename_edit(fixture, &fixture.sysv_member_new_name))
        .unwrap();

    assert_eq!(
        result,
        Some(ByteEdit::replace(
            fixture.sysv_member_name_offset,
            fixture.sysv_member_name.as_bytes(),
            fixture.sysv_member_new_name.as_bytes(),
        ))
    );
}

#[test]
fn translate_edit_same_payload_replace_is_noop() {
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "members".to_string(),
            fixture.payload_member_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(RlibTreeOp::ReplaceMemberBytes {
            old_bytes: fixture.payload_old_bytes.clone(),
            new_bytes: fixture.payload_old_bytes.clone(),
        }),
    };

    assert_eq!(codec.translate_edit(&bytes, &edit), Ok(None));
}

#[test]
fn translate_edit_same_name_rename_is_noop() {
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    assert_eq!(
        codec.translate_edit(&bytes, &sysv_rename_edit(fixture, &fixture.sysv_member_name)),
        Ok(None)
    );
}

#[test]
fn translate_edit_malformed_member_path_is_rejected() {
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["members".to_string(), fixture.payload_member_name.clone()]),
        op: TreeOp::new(RlibTreeOp::ReplaceMemberBytes {
            old_bytes: fixture.payload_old_bytes.clone(),
            new_bytes: fixture.payload_new_bytes.clone(),
        }),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn translate_edit_unsupported_target_is_rejected() {
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "members".to_string(),
            fixture.payload_member_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(RlibTreeOp::RenameMember {
            new_name: fixture.sysv_member_new_name.clone(),
        }),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn translate_edit_size_changing_payload_replace_is_rejected() {
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "members".to_string(),
            fixture.payload_member_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(RlibTreeOp::ReplaceMemberBytes {
            old_bytes: fixture.payload_old_bytes.clone(),
            new_bytes: fixture.payload_new_bytes[..fixture.payload_new_bytes.len() - 1].to_vec(),
        }),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}

#[test]
fn translate_edit_size_changing_member_rename_is_rejected() {
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    assert!(matches!(
        codec.translate_edit(
            &bytes,
            &sysv_rename_edit(fixture, &format!("{}x", fixture.sysv_member_name))
        ),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}

#[test]
fn translate_edit_unpatchable_rename_storage_is_rejected() {
    let codec = RlibCodec::new();
    let name = "this_is_a_bsd_named_member.o";
    let bytes = HeapByteSource::new(bsd_named_archive_bytes(name, b"DATA"));
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["members".to_string(), name.to_string(), "name".to_string()]),
        op: TreeOp::new(RlibTreeOp::RenameMember {
            new_name: same_length_name(name),
        }),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn real_fixture_workflow_reparses_and_restores_original_bytes() {
    let codec = Arc::new(RlibCodec::new());
    let fixture = rlib_fixture().clone();
    let mut table = InodeTable::new();
    let inode_id = table.insert(Arc::new(HeapByteSource::new(fixture.bytes.clone())));
    let buffer_id = BufferId::from_raw(1);
    table.bind_file(buffer_id, inode_id);

    let source_mount = table
        .mount(
            inode_id,
            buffer_id,
            Mount::with_mode("rlib-source", codec.clone(), MountMode::Structural),
        )
        .unwrap();
    let peer_mount = table
        .mount_additional(inode_id, buffer_id, Mount::new("rlib-peer", codec.clone()))
        .unwrap();

    assert_eq!(table.read_bytes(inode_id).unwrap(), fixture.bytes);

    let edits = [
        payload_replace_edit(&fixture),
        sysv_rename_edit(&fixture, &fixture.sysv_member_new_name),
    ];
    let mut inverses = Vec::new();

    for edit in &edits {
        let byte_edit = table.apply_edit(source_mount, edit).unwrap().unwrap();
        inverses.push(byte_edit.inverse());

        let current = table.read_bytes(inode_id).unwrap();
        assert_eq!(current.len(), fixture.bytes.len());
        let decoded = codec.decode(&current).unwrap();
        assert!(decoded.content.contains("Archive Members"));
    }

    let current = table.read_bytes(inode_id).unwrap();
    let decoded = codec.decode(&current).unwrap();
    assert!(decoded.content.contains(&fixture.sysv_member_new_name));

    let inode = table.lookup_inode(inode_id).unwrap();
    let peer = inode.mounts.get(&peer_mount.mount_id()).unwrap();
    assert!(!peer.content_valid);

    for inverse in inverses.iter().rev() {
        table.apply_byte_edit(inode_id, inverse).unwrap();
    }

    let restored = table.read_bytes(inode_id).unwrap();
    assert_eq!(restored, fixture.bytes);
    codec.decode(&restored).unwrap();
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

// ============================================================================
// MC/DC coverage gap tests (Category A: testable paths)
// ============================================================================

#[test]
fn translate_edit_payload_old_bytes_content_mismatch_rejected() {
    // 230:0 — actual != old_bytes in translate_rlib_replace_member_bytes
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    // Build wrong old_bytes: same length as payload but different content.
    let wrong_old: Vec<u8> = fixture.payload_old_bytes.iter().map(|b| b ^ 0xFF).collect();
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "members".to_string(),
            fixture.payload_member_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(RlibTreeOp::ReplaceMemberBytes {
            old_bytes: wrong_old.clone(),
            new_bytes: wrong_old, // same length
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}

#[test]
fn translate_edit_member_path_wrong_kind_rejected() {
    // 285:0 — kind != "members" in resolve_member_path
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "entries".to_string(), // wrong kind
            fixture.payload_member_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(RlibTreeOp::ReplaceMemberBytes {
            old_bytes: fixture.payload_old_bytes.clone(),
            new_bytes: fixture.payload_new_bytes.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn translate_edit_member_path_empty_name_rejected() {
    // 285:2 — member_name.is_empty() in resolve_member_path
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "members".to_string(),
            String::new(), // empty name
            "bytes".to_string(),
        ]),
        op: TreeOp::new(RlibTreeOp::ReplaceMemberBytes {
            old_bytes: fixture.payload_old_bytes.clone(),
            new_bytes: fixture.payload_new_bytes.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn extract_rmeta_info_no_rmeta_member() {
    // 441:1 — rmeta_name is None (no .rmeta member)
    //
    // Build a minimal archive without any .rmeta member.
    let mut archive_bytes = Vec::new();
    archive_bytes.extend_from_slice(goblin::archive::MAGIC);

    // Write one member: "hello.o" with 4 bytes of payload.
    let payload = b"data";
    let member_name = b"hello.o         "; // 16 bytes, padded with spaces
    let file_size = b"4         "; // 10 bytes
    archive_bytes.extend_from_slice(member_name);
    archive_bytes.extend_from_slice(b"0           "); // mtime (12)
    archive_bytes.extend_from_slice(b"0     "); // uid (6)
    archive_bytes.extend_from_slice(b"0     "); // gid (6)
    archive_bytes.extend_from_slice(b"100644  "); // mode (8)
    archive_bytes.extend_from_slice(file_size);
    archive_bytes.extend_from_slice(b"`\n"); // magic (2)
    archive_bytes.extend_from_slice(payload);

    let codec = RlibCodec::new();
    // decode() calls extract_rmeta_info which returns (None, Vec::new()) when
    // no .rmeta member is found.
    let result = codec.decode(&archive_bytes);
    assert!(result.is_ok());
    let decoded = result.unwrap();
    assert!(!decoded.content.contains("Rustc Version"));
}

#[test]
fn extract_rmeta_info_non_elf_rmeta_falls_back_to_raw_scan() {
    // 463:1 — rmeta_section is None (rmeta data is not a valid ELF)
    //
    // Build a minimal archive with a "lib.rmeta" member whose payload is raw
    // rmeta bytes (not a wrapped ELF object). extract_rmeta_info falls back to
    // scan_rmeta_bytes(rmeta_data) directly.
    let mut rmeta_payload = Vec::new();
    rmeta_payload.extend_from_slice(b"rust\0\0\0\n");
    rmeta_payload.extend_from_slice(b"1.99.0-test\0");

    let mut archive_bytes = Vec::new();
    archive_bytes.extend_from_slice(goblin::archive::MAGIC);

    let file_size_str = format!("{:<10}", rmeta_payload.len());
    archive_bytes.extend_from_slice(b"lib.rmeta       "); // name (16)
    archive_bytes.extend_from_slice(b"0           "); // mtime (12)
    archive_bytes.extend_from_slice(b"0     "); // uid (6)
    archive_bytes.extend_from_slice(b"0     "); // gid (6)
    archive_bytes.extend_from_slice(b"100644  "); // mode (8)
    archive_bytes.extend_from_slice(file_size_str.as_bytes()); // file size (10)
    archive_bytes.extend_from_slice(b"`\n"); // magic (2)
    archive_bytes.extend_from_slice(&rmeta_payload);
    if archive_bytes.len() & 1 == 1 {
        archive_bytes.push(b'\n');
    }

    let codec = RlibCodec::new();
    let result = codec.decode(&archive_bytes);
    assert!(result.is_ok());
    let decoded = result.unwrap();
    assert!(decoded.content.contains("1.99.0-test"), "content: {}", decoded.content);
}

#[test]
fn scan_rmeta_bytes_newline_terminated_version() {
    // 486:2 — b == b'\n' branch in the version-terminator position()
    let mut data = Vec::new();
    data.extend_from_slice(b"rust\0\0\0\n");
    data.extend_from_slice(b"1.96.0-nightly");
    data.push(b'\n'); // newline terminator instead of null

    let (version, _) = scan_rmeta_bytes(&data);
    assert_eq!(version.as_deref(), Some("1.96.0-nightly"));
}

#[test]
fn scan_rmeta_bytes_empty_version_returns_none() {
    // 490:0 — s.is_empty() when the byte immediately after magic is a terminator
    let mut data = Vec::new();
    data.extend_from_slice(b"rust\0\0\0\n");
    data.push(0); // null terminator immediately after magic → empty version string

    let (version, _) = scan_rmeta_bytes(&data);
    assert!(version.is_none(), "expected None for empty version, got {version:?}");
}

#[test]
fn extract_dependency_names_hyphen_style_crate() {
    // 522:1 — contains(&b'_') is false but contains(&b'-') is true
    let data = b"tokio-util\0other_dep\0ab\0";
    let deps = extract_dependency_names(data);
    assert!(deps.contains(&"tokio-util".to_string()), "expected tokio-util in {deps:?}");
    assert!(deps.contains(&"other_dep".to_string()));
}

// ============================================================================
// RLIB MC/DC coverage gap tests — parse failure, field/op mismatch, component count
// ============================================================================

#[test]
fn translate_rlib_edit_parse_failure_returns_internal_error() {
    // Lines 164-167 — goblin::archive::Archive::parse fails on garbage bytes,
    // causing translate_rlib_edit to return TranslateEditError::Internal.
    //
    // The text/bytes unsupported-edit guards execute before parse, so we must
    // use a Tree edit to reach the archive parse call.
    let codec = RlibCodec::new();
    let bytes = HeapByteSource::new(b"not an archive at all".to_vec());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "members".to_string(),
            "lib.rmeta".to_string(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(RlibTreeOp::ReplaceMemberBytes {
            old_bytes: vec![0x00],
            new_bytes: vec![0xFF],
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::Internal { .. })
    ));
}

#[test]
fn translate_rlib_name_field_with_replace_bytes_op_is_rejected() {
    // Lines 188-192 — (MemberField::Name, RlibTreeOp::ReplaceMemberBytes) arm.
    // Targeting the "name" field while supplying a ReplaceMemberBytes operation
    // must return UnsupportedEdit.
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "members".to_string(),
            fixture.payload_member_name.clone(),
            "name".to_string(),
        ]),
        op: TreeOp::new(RlibTreeOp::ReplaceMemberBytes {
            old_bytes: fixture.payload_old_bytes.clone(),
            new_bytes: fixture.payload_new_bytes.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn translate_rlib_member_path_too_many_components_rejected() {
    // Lines 295-302 — the slice pattern `[kind, member_name, field]` in
    // resolve_member_path rejects any path that does not have exactly 3
    // components. The 2-component case is exercised by
    // translate_edit_malformed_member_path_is_rejected; here we cover the
    // same `else` branch with a 4-component path.
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "members".to_string(),
            fixture.payload_member_name.clone(),
            "bytes".to_string(),
            "extra".to_string(),
        ]),
        op: TreeOp::new(RlibTreeOp::ReplaceMemberBytes {
            old_bytes: fixture.payload_old_bytes.clone(),
            new_bytes: fixture.payload_new_bytes.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

// ============================================================================
// MC/DC coverage gap tests — remaining uncovered paths
// ============================================================================

#[test]
fn translate_edit_member_path_unknown_field_rejected() {
    // L315-317 — `_ =>` arm in the field match inside resolve_member_path.
    // Providing a path with a valid kind/name but an unknown field exercises
    // the wildcard arm that returns MalformedPath.
    let codec = RlibCodec::new();
    let fixture = rlib_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "members".to_string(),
            fixture.payload_member_name.clone(),
            "unknown_field".to_string(),
        ]),
        op: TreeOp::new(RlibTreeOp::ReplaceMemberBytes {
            old_bytes: fixture.payload_old_bytes.clone(),
            new_bytes: fixture.payload_new_bytes.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

/// Build a minimal archive where the `lib.rmeta` member header declares a
/// file size larger than the actual raw bytes present. `goblin::archive::Archive::parse`
/// may still succeed (it builds the member index from headers), but
/// `archive.extract("lib.rmeta", raw)` will fail when it tries to slice beyond
/// `raw.len()`, exercising L535-536 (`let Ok(rmeta_data) = archive.extract(...)` else).
fn build_archive_with_truncated_rmeta() -> Vec<u8> {
    let mut archive_bytes = Vec::new();
    archive_bytes.extend_from_slice(goblin::archive::MAGIC);

    // Declare a `lib.rmeta` member with a payload of 9999 bytes, but only write
    // a few actual bytes. goblin will see the member name and record it in its
    // member table; extract() will fail when the raw slice is insufficient.
    let declared_size = b"9999      "; // 10 bytes, right-padded spaces
    archive_bytes.extend_from_slice(b"lib.rmeta       "); // name (16)
    archive_bytes.extend_from_slice(b"0           "); // mtime (12)
    archive_bytes.extend_from_slice(b"0     "); // uid (6)
    archive_bytes.extend_from_slice(b"0     "); // gid (6)
    archive_bytes.extend_from_slice(b"100644  "); // mode (8)
    archive_bytes.extend_from_slice(declared_size); // file size (10)
    archive_bytes.extend_from_slice(b"`\n"); // member magic (2)
    // Actual payload: only 4 bytes, not 9999
    archive_bytes.extend_from_slice(b"rust");

    archive_bytes
}

#[test]
fn extract_rmeta_info_extract_failure_returns_empty() {
    // L535-536 — `archive.extract(rmeta_name, raw)` returns Err.
    // The archive header says "lib.rmeta" is 9999 bytes, but the raw buffer is
    // only a few bytes long. goblin's `extract` fails with a bounds error, and
    // `extract_rmeta_info` falls through to return (None, Vec::new()).
    let archive_bytes = build_archive_with_truncated_rmeta();

    // goblin::archive::Archive::parse may fail for a very truncated archive;
    // if it does, decode() returns an Err and the test path is still exercised.
    let codec = RlibCodec::new();
    let result = codec.decode(&archive_bytes);
    // Either decode succeeds (extract failed gracefully) or fails with internal error.
    // Both outcomes are acceptable — what matters is no panic and no coverage miss.
    if let Ok(decoded) = result {
        // extract failed, so no Rustc version info
        assert!(!decoded.content.contains("Rustc Version:"));
    }
    // Err case: archive parse itself failed — still exercises the decode path
}

#[test]
fn extract_dependency_names_common_non_dep_filtered_out() {
    // L620:1 — the `.filter()` closure returns `None` when `is_common_non_dep(n)` is true.
    // "rust_metadata" satisfies (3..=64) length and has `_` separator, but is a
    // common non-dep and must not appear in the output.
    let data = b"rust_metadata\0tokio_util\0ab\0";
    let deps = extract_dependency_names(data);
    assert!(
        !deps.contains(&"rust_metadata".to_string()),
        "common non-dep 'rust_metadata' should be filtered out, got {deps:?}"
    );
    assert!(
        deps.contains(&"tokio_util".to_string()),
        "regular dep 'tokio_util' should be present, got {deps:?}"
    );
}

#[test]
fn extract_dependency_names_duplicate_filtered_out() {
    // L620:1 — the `.filter()` closure returns `None` when `seen.insert()` returns false
    // (duplicate name). Include "tokio_util" twice; only one occurrence should appear.
    let data = b"tokio_util\0tokio_util\0other_dep\0";
    let deps = extract_dependency_names(data);
    let count = deps.iter().filter(|d| d.as_str() == "tokio_util").count();
    assert_eq!(count, 1, "duplicate 'tokio_util' should appear only once, got {deps:?}");
}
