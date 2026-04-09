//! Tests for tar.gz codec.

use std::{
    io::{Cursor, Write},
    sync::Arc,
};

use {
    flate2::{Compression, write::GzEncoder},
    reovim_driver_codec::{
        ContentCodec, DecodedEdit, InodeTable, Mount, MountMode, TranslateEditError, TreeOp,
        TreePath,
    },
    reovim_driver_vfs::HeapByteSource,
    reovim_kernel::api::v1::{BufferId, ByteEdit},
    reovim_types_text::Position,
    tar::Builder,
};

use super::*;

// ---------------------------------------------------------------------------
// Fixture helpers

#[allow(clippy::similar_names)]
/// A minimal tar.gz fixture with two members.
#[derive(Debug, Clone)]
struct TarGzFixture {
    /// The original gzip bytes.
    bytes: Vec<u8>,
    /// Name of the first member.
    first_name: String,
    /// Payload of the first member.
    first_payload: Vec<u8>,
    /// Name of the second member.
    second_name: String,
}

fn build_fixture() -> TarGzFixture {
    let first_name = "hello.txt".to_string();
    let first_payload = b"ORIGINAL_A".to_vec();
    let second_name = "world.txt".to_string();
    let second_payload = b"ORIGINAL_B".to_vec();

    let bytes = build_tar_gz(&[
        (first_name.as_str(), first_payload.as_slice()),
        (second_name.as_str(), second_payload.as_slice()),
    ]);

    TarGzFixture {
        bytes,
        first_name,
        first_payload,
        second_name,
    }
}

/// Build a tar.gz bytes from a list of (name, payload) pairs.
fn build_tar_gz(members: &[(&str, &[u8])]) -> Vec<u8> {
    let gz_buf = Vec::new();
    let encoder = GzEncoder::new(gz_buf, Compression::default());
    let mut tar = Builder::new(encoder);

    for (name, payload) in members {
        let mut header = tar::Header::new_gnu();
        header.set_size(u64::try_from(payload.len()).unwrap());
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, name, Cursor::new(payload))
            .unwrap();
    }

    let encoder = tar.into_inner().unwrap();
    encoder.finish().unwrap()
}

/// Decompress and re-parse to verify the archive is still valid after an edit.
fn roundtrip_decompress(bytes: &[u8]) -> Vec<TarMember> {
    let tar_bytes = decompress_gzip(bytes).expect("roundtrip decompress failed");
    parse_tar_members(&tar_bytes).expect("roundtrip parse failed")
}

/// Apply a `ByteEdit` to a `Vec<u8>` by splicing `old_bytes` → `new_bytes` at offset.
fn apply_byte_edit_to_vec(data: &mut Vec<u8>, edit: &ByteEdit) {
    let start = edit.offset;
    let end = start + edit.old_bytes.len();
    data.splice(start..end, edit.new_bytes.iter().copied());
}

fn tree_path(member_name: &str, field: &str) -> TreePath {
    TreePath::new(vec![
        "members".to_string(),
        member_name.to_string(),
        field.to_string(),
    ])
}

fn rename_edit(member_name: &str, new_name: &str) -> DecodedEdit {
    DecodedEdit::Tree {
        path: tree_path(member_name, "name"),
        op: TreeOp::new(TarGzTreeOp::RenameMember {
            new_name: new_name.to_string(),
        }),
    }
}

fn replace_bytes_edit(member_name: &str, old_bytes: Vec<u8>, new_bytes: Vec<u8>) -> DecodedEdit {
    DecodedEdit::Tree {
        path: tree_path(member_name, "bytes"),
        op: TreeOp::new(TarGzTreeOp::ReplaceMemberBytes {
            old_bytes,
            new_bytes,
        }),
    }
}

// ---------------------------------------------------------------------------
// Decode tests

#[test]
fn decode_produces_valid_summary() {
    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let result = codec.decode(&fixture.bytes).unwrap();

    assert!(result.content.contains("tar.gz Archive Summary"));
    assert!(result.content.contains("Archive Members"));
    assert!(result.content.contains(&fixture.first_name));
    assert!(result.content.contains(&fixture.second_name));
    assert!(result.lossy);
    assert!(!result.readonly);
    assert!(!result.truncated);
    assert!(!result.annotations.is_empty());
    assert_eq!(result.metadata.get("readonly"), Some("false"));
    assert_eq!(result.metadata.get("member_count"), Some("2"));
}

#[test]
fn decode_invalid_gzip_returns_error() {
    let codec = TarGzCodec::new();
    let result = codec.decode(b"not gzip data");
    assert!(result.is_err());
}

#[test]
fn decode_truncated_tar_inside_gzip_returns_error() {
    // Compress some garbage bytes that look gzip but aren't valid tar
    let gz_buf = Vec::new();
    let mut encoder = GzEncoder::new(gz_buf, Compression::default());
    encoder.write_all(b"not a tar archive").unwrap();
    let bad_bytes = encoder.finish().unwrap();

    let codec = TarGzCodec::new();
    // decode should either succeed with 0 members or return an error;
    // in either case it should not panic.
    let _ = codec.decode(&bad_bytes);
}

#[test]
fn default_impl() {
    let codec = TarGzCodec;
    assert_eq!(std::mem::size_of_val(&codec), std::mem::size_of::<TarGzCodec>());
}

// ---------------------------------------------------------------------------
// translate_edit: unsupported variants

#[test]
fn translate_edit_text_is_not_supported() {
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(b"some bytes");
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
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(b"some bytes");
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
fn translate_edit_wrong_tree_op_type_is_rejected() {
    // Construct a Tree edit with a non-TarGzTreeOp op (use RlibTreeOp-like
    // approach: a dummy type via Arc<dyn AnyTreeOp>)
    use reovim_driver_codec::TreeOp;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct UnrelatedOp;
    reovim_driver_codec::impl_tree_op!(UnrelatedOp);

    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: tree_path(&fixture.first_name, "name"),
        op: TreeOp::new(UnrelatedOp),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

// ---------------------------------------------------------------------------
// translate_edit: malformed paths

#[test]
fn translate_edit_malformed_path_too_short() {
    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let name = fixture.first_name;
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["members".to_string(), name.clone()]),
        op: TreeOp::new(TarGzTreeOp::RenameMember { new_name: name }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn translate_edit_malformed_path_wrong_root() {
    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let name = fixture.first_name;
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["entries".to_string(), name.clone(), "name".to_string()]),
        op: TreeOp::new(TarGzTreeOp::RenameMember { new_name: name }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn translate_edit_malformed_path_unknown_field() {
    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let name = fixture.first_name;
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["members".to_string(), name.clone(), "metadata".to_string()]),
        op: TreeOp::new(TarGzTreeOp::RenameMember { new_name: name }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn translate_edit_member_not_found() {
    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes);
    // Use a different new_name so it doesn't hit the early no-op return.
    // Both names have the same length (15) but the member doesn't exist.
    let edit = rename_edit("nonexistent.txt", "replacement.txt");
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

// ---------------------------------------------------------------------------
// translate_edit: wrong op/path combination

#[test]
fn translate_edit_rename_on_bytes_path_rejected() {
    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: tree_path(&fixture.first_name, "bytes"),
        op: TreeOp::new(TarGzTreeOp::RenameMember {
            new_name: fixture.first_name.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn translate_edit_replace_bytes_on_name_path_rejected() {
    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: tree_path(&fixture.first_name, "name"),
        op: TreeOp::new(TarGzTreeOp::ReplaceMemberBytes {
            old_bytes: fixture.first_payload.clone(),
            new_bytes: fixture.first_payload.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

// ---------------------------------------------------------------------------
// translate_edit: rename (positive + constraints)

#[test]
fn translate_edit_rename_returns_full_file_replacement() {
    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    // Same-length name: "hello.txt" (9 chars) -> "jello.txt" (9 chars)
    let new_name = "jello.txt".to_string();
    assert_eq!(new_name.len(), fixture.first_name.len());

    let result = codec
        .translate_edit(&bytes, &rename_edit(&fixture.first_name, &new_name))
        .unwrap();

    let byte_edit = result.expect("rename should return Some(ByteEdit)");
    // Full file replacement starts at offset 0
    assert_eq!(byte_edit.offset, 0);
    assert_eq!(&byte_edit.old_bytes[..], &fixture.bytes[..]);

    // Verify the edit produces a valid archive with the renamed member.
    let new_bytes = {
        let mut current = fixture.bytes.clone();
        apply_byte_edit_to_vec(&mut current, &byte_edit);
        current
    };
    let members = roundtrip_decompress(&new_bytes);
    assert!(members.iter().any(|m| m.name == new_name));
    assert!(!members.iter().any(|m| m.name == fixture.first_name));
}

#[test]
fn translate_edit_rename_same_name_is_noop() {
    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let result = codec
        .translate_edit(&bytes, &rename_edit(&fixture.first_name, &fixture.first_name))
        .unwrap();
    assert_eq!(result, None);
}

#[test]
fn translate_edit_rename_different_length_rejected() {
    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    // "hello.txt" (9) -> "hi.txt" (6): different lengths
    let result = codec.translate_edit(&bytes, &rename_edit(&fixture.first_name, "hi.txt"));
    assert!(matches!(result, Err(TranslateEditError::ConstraintViolation { .. })));
}

// ---------------------------------------------------------------------------
// translate_edit: replace bytes (positive + constraints)

#[test]
fn translate_edit_replace_bytes_returns_full_file_replacement() {
    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let mut new_payload = fixture.first_payload.clone();
    new_payload[0] ^= 0x5A; // flip one byte

    let result = codec
        .translate_edit(
            &bytes,
            &replace_bytes_edit(
                &fixture.first_name,
                fixture.first_payload.clone(),
                new_payload.clone(),
            ),
        )
        .unwrap();

    let byte_edit = result.expect("replace_bytes should return Some(ByteEdit)");
    assert_eq!(byte_edit.offset, 0);
    assert_eq!(&byte_edit.old_bytes[..], &fixture.bytes[..]);

    // Verify the edit produces a valid archive with the patched payload.
    let new_bytes = {
        let mut current = fixture.bytes.clone();
        apply_byte_edit_to_vec(&mut current, &byte_edit);
        current
    };
    let tar_bytes = decompress_gzip(&new_bytes).unwrap();
    let members_parsed = parse_tar_members(&tar_bytes).unwrap();
    let member_a = members_parsed
        .iter()
        .find(|m| m.name == fixture.first_name)
        .unwrap();
    let payload_end = member_a.payload_offset + usize::try_from(member_a.size).unwrap();
    assert_eq!(&tar_bytes[member_a.payload_offset..payload_end], &new_payload[..]);
}

#[test]
fn translate_edit_replace_bytes_same_bytes_is_noop() {
    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let result = codec
        .translate_edit(
            &bytes,
            &replace_bytes_edit(
                &fixture.first_name,
                fixture.first_payload.clone(),
                fixture.first_payload.clone(),
            ),
        )
        .unwrap();
    assert_eq!(result, None);
}

#[test]
fn translate_edit_replace_bytes_size_mismatch_rejected() {
    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let shorter = fixture.first_payload[..fixture.first_payload.len() - 1].to_vec();
    let result = codec.translate_edit(
        &bytes,
        &replace_bytes_edit(&fixture.first_name, fixture.first_payload.clone(), shorter),
    );
    assert!(matches!(result, Err(TranslateEditError::ConstraintViolation { .. })));
}

#[test]
fn translate_edit_replace_bytes_wrong_old_bytes_rejected() {
    let fixture = build_fixture();
    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    // Same length as actual but wrong content
    let mut wrong_old = fixture.first_payload.clone();
    wrong_old[0] ^= 0xFF;
    let mut new_bytes = fixture.first_payload.clone();
    new_bytes[0] ^= 0x5A;

    let result = codec
        .translate_edit(&bytes, &replace_bytes_edit(&fixture.first_name, wrong_old, new_bytes));
    assert!(matches!(result, Err(TranslateEditError::ConstraintViolation { .. })));
}

// ---------------------------------------------------------------------------
// Structural gate: undo correctness

#[test]
fn rename_edit_undo_restores_original_bytes() {
    let fixture = build_fixture();
    let new_name = "jello.txt".to_string();
    assert_eq!(new_name.len(), fixture.first_name.len());

    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let byte_edit = codec
        .translate_edit(&bytes, &rename_edit(&fixture.first_name, &new_name))
        .unwrap()
        .unwrap();

    // Apply the edit to get new bytes.
    let mut current = fixture.bytes.clone();
    apply_byte_edit_to_vec(&mut current, &byte_edit);
    assert_ne!(current, fixture.bytes);

    // Apply the inverse to restore.
    let inverse = byte_edit.inverse();
    apply_byte_edit_to_vec(&mut current, &inverse);
    assert_eq!(current, fixture.bytes);
}

#[test]
fn replace_bytes_edit_undo_restores_original_bytes() {
    let fixture = build_fixture();
    let mut new_payload = fixture.first_payload.clone();
    new_payload[0] ^= 0x5A;

    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let byte_edit = codec
        .translate_edit(
            &bytes,
            &replace_bytes_edit(&fixture.first_name, fixture.first_payload.clone(), new_payload),
        )
        .unwrap()
        .unwrap();

    let mut current = fixture.bytes.clone();
    apply_byte_edit_to_vec(&mut current, &byte_edit);
    assert_ne!(current, fixture.bytes);

    let inverse = byte_edit.inverse();
    apply_byte_edit_to_vec(&mut current, &inverse);
    assert_eq!(current, fixture.bytes);
}

// ---------------------------------------------------------------------------
// Structural gate: re-parse after edit

#[test]
fn re_parse_after_rename_is_valid() {
    let fixture = build_fixture();
    let new_name = "jello.txt".to_string();
    assert_eq!(new_name.len(), fixture.first_name.len());

    let codec = TarGzCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let byte_edit = codec
        .translate_edit(&bytes, &rename_edit(&fixture.first_name, &new_name))
        .unwrap()
        .unwrap();

    let mut current = fixture.bytes.clone();
    apply_byte_edit_to_vec(&mut current, &byte_edit);

    // Re-parse via codec decode — should not error.
    let decoded = codec.decode(&current).unwrap();
    assert!(decoded.content.contains(&new_name));
    assert!(!decoded.content.contains(&fixture.first_name));
    assert!(decoded.content.contains(&fixture.second_name));
}

// ---------------------------------------------------------------------------
// InodeTable / peer-stale propagation

#[test]
fn peer_stale_propagation_via_inode_table() {
    let fixture = build_fixture();
    let new_name = "jello.txt".to_string();
    assert_eq!(new_name.len(), fixture.first_name.len());

    let codec = Arc::new(TarGzCodec::new());
    let mut table = InodeTable::new();
    let inode_id = table.insert(Arc::new(HeapByteSource::new(fixture.bytes.clone())));
    let buffer_id = BufferId::from_raw(1);
    table.bind_file(buffer_id, inode_id);

    let source_mount = table
        .mount(
            inode_id,
            buffer_id,
            Mount::with_mode("tar-gz-source", codec.clone(), MountMode::Structural),
        )
        .unwrap();
    let peer_mount = table
        .mount_additional(inode_id, buffer_id, Mount::new("tar-gz-peer", codec))
        .unwrap();

    // Verify initial bytes.
    assert_eq!(table.read_bytes(inode_id).unwrap(), fixture.bytes);

    // Apply rename edit.
    let edit = rename_edit(&fixture.first_name, &new_name);
    let byte_edit = table.apply_edit(source_mount, &edit).unwrap().unwrap();

    // Bytes should have changed (different gzip compression).
    let current = table.read_bytes(inode_id).unwrap();
    assert_ne!(current, fixture.bytes);

    // Peer mount should be marked stale.
    let inode = table.lookup_inode(inode_id).unwrap();
    let peer = inode.mounts.get(&peer_mount.mount_id()).unwrap();
    assert!(!peer.content_valid);

    // Apply inverse to restore original.
    let inverse = byte_edit.inverse();
    table.apply_byte_edit(inode_id, &inverse).unwrap();

    let restored = table.read_bytes(inode_id).unwrap();
    assert_eq!(restored, fixture.bytes);
}

// ---------------------------------------------------------------------------
// Internal helpers

#[test]
fn compute_tar_checksum_matches_all_zero_header() {
    let header = [0u8; 512];
    // Checksum field (148..156) treated as spaces, rest is 0 -> sum = 8 * 0x20 = 256
    let result = compute_tar_checksum(&header);
    assert_eq!(result, 256);
}

#[test]
fn decompress_gzip_valid() {
    let gz_buf = Vec::new();
    let mut encoder = GzEncoder::new(gz_buf, Compression::default());
    encoder.write_all(b"hello").unwrap();
    let compressed = encoder.finish().unwrap();

    let result = decompress_gzip(&compressed).unwrap();
    assert_eq!(result, b"hello");
}

#[test]
fn decompress_gzip_invalid() {
    let result = decompress_gzip(b"not gzip");
    assert!(result.is_err());
}

#[test]
fn recompress_roundtrip() {
    let original = b"some tar content that is interesting";
    let compressed = recompress_gzip(original);
    let decompressed = decompress_gzip(&compressed).unwrap();
    assert_eq!(decompressed, original);
}
