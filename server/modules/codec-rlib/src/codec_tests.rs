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
