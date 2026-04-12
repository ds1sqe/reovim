//! Tests for ELF and ZIP codecs.

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
struct ElfFixture {
    bytes: Vec<u8>,
    text_section_name: String,
    text_section_offset: usize,
    text_patch_offset: usize,
    text_old_bytes: Vec<u8>,
    text_new_bytes: Vec<u8>,
    data_section_name: String,
    data_section_offset: usize,
    section_old_bytes: Vec<u8>,
    section_new_bytes: Vec<u8>,
    symbol_name: String,
    symbol_name_offset: usize,
    symbol_new_name: String,
}

static ELF_FIXTURE: OnceLock<ElfFixture> = OnceLock::new();

fn elf_fixture() -> &'static ElfFixture {
    ELF_FIXTURE.get_or_init(build_elf_fixture)
}

fn build_elf_fixture() -> ElfFixture {
    let fixture_dir = std::env::temp_dir().join(format!(
        "reovim-elf-phase2-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("tests")
    ));
    let _ = std::fs::remove_dir_all(&fixture_dir);
    std::fs::create_dir_all(&fixture_dir).unwrap();

    let source_path = fixture_dir.join("fixture.rs");
    let object_path = fixture_dir.join("fixture.o");
    std::fs::write(
        &source_path,
        r#"
#[no_mangle]
pub static target_old: u8 = 7;

#[link_section = ".text.phase2"]
#[used]
pub static PHASE2_TEXT: [u8; 4] = [0x90, 0x90, 0xC3, 0xCC];

#[link_section = ".phase2"]
#[used]
pub static PHASE2_BYTES: [u8; 8] = *b"ORIGINAL";
"#,
    )
    .unwrap();

    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let output = Command::new(rustc)
        .args([
            "--edition=2021",
            "--crate-type=lib",
            "--emit=obj",
            source_path.to_str().unwrap(),
            "-o",
            object_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fixture compilation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bytes = std::fs::read(&object_path).unwrap();
    let elf = goblin::elf::Elf::parse(&bytes).unwrap();

    let (text_section_name, text_section_offset, text_section_bytes) =
        section_fixture(&elf, &bytes, ".text.phase2");
    let text_old_bytes = text_section_bytes[..4].to_vec();
    let text_new_bytes = text_old_bytes.iter().map(|byte| byte ^ 0x5A).collect();

    let (data_section_name, data_section_offset, section_old_bytes) =
        section_fixture(&elf, &bytes, ".phase2");
    assert!(section_old_bytes.len() >= 8);
    let mut section_new_bytes = section_old_bytes.clone();
    section_new_bytes[..8].copy_from_slice(b"MODIFIED");

    let symbol_name = "target_old".to_string();
    let symbol_name_offset = symbol_name_offset(&elf, &symbol_name);

    let _ = std::fs::remove_dir_all(&fixture_dir);

    ElfFixture {
        bytes,
        text_section_name,
        text_section_offset,
        text_patch_offset: 0,
        text_old_bytes,
        text_new_bytes,
        data_section_name,
        data_section_offset,
        section_old_bytes,
        section_new_bytes,
        symbol_name,
        symbol_name_offset,
        symbol_new_name: "target_new".to_string(),
    }
}

fn section_fixture(
    elf: &goblin::elf::Elf<'_>,
    bytes: &[u8],
    target_name: &str,
) -> (String, usize, Vec<u8>) {
    let section = elf
        .section_headers
        .iter()
        .find(|header| {
            elf.shdr_strtab
                .get_at(header.sh_name)
                .is_some_and(|name| name == target_name)
        })
        .unwrap();
    let offset = usize::try_from(section.sh_offset).unwrap();
    let size = usize::try_from(section.sh_size).unwrap();

    (target_name.to_string(), offset, bytes[offset..offset + size].to_vec())
}

fn symbol_name_offset(elf: &goblin::elf::Elf<'_>, target_name: &str) -> usize {
    let symbol = elf
        .syms
        .iter()
        .find(|sym| {
            elf.strtab
                .get_at(sym.st_name)
                .is_some_and(|name| name == target_name)
        })
        .unwrap();
    let symtab = elf
        .section_headers
        .iter()
        .find(|section| section.sh_type == goblin::elf::section_header::SHT_SYMTAB)
        .unwrap();
    let strtab = elf
        .section_headers
        .get(usize::try_from(symtab.sh_link).unwrap())
        .unwrap();

    usize::try_from(strtab.sh_offset).unwrap() + symbol.st_name
}

fn instruction_patch_edit(fixture: &ElfFixture) -> DecodedEdit {
    DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            fixture.text_section_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::PatchBytes {
            offset: fixture.text_patch_offset,
            old_bytes: fixture.text_old_bytes.clone(),
            new_bytes: fixture.text_new_bytes.clone(),
        }),
    }
}

fn symbol_rename_edit(fixture: &ElfFixture, new_name: &str) -> DecodedEdit {
    DecodedEdit::Tree {
        path: TreePath::new(vec![
            "symbols".to_string(),
            fixture.symbol_name.clone(),
            "name".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::RenameSymbol {
            new_name: new_name.to_string(),
        }),
    }
}

fn section_replace_edit(fixture: &ElfFixture) -> DecodedEdit {
    DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            fixture.data_section_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::ReplaceSectionBytes {
            old_bytes: fixture.section_old_bytes.clone(),
            new_bytes: fixture.section_new_bytes.clone(),
        }),
    }
}

// === ELF Codec Tests ===

#[test]
fn elf_decode_invalid() {
    let codec = ElfCodec::new();
    let result = codec.decode(b"not an elf");
    assert!(result.is_err());
}

#[test]
fn elf_default_impl() {
    let codec = ElfCodec;
    assert_eq!(std::mem::size_of_val(&codec), std::mem::size_of::<ElfCodec>());
}

#[test]
fn elf_translate_edit_text_is_not_supported() {
    let codec = ElfCodec::new();
    let bytes = HeapByteSource::new(b"\x7fELF");
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
fn elf_translate_edit_bytes_is_not_supported() {
    let codec = ElfCodec::new();
    let bytes = HeapByteSource::new(vec![0x7f, b'E', b'L', b'F']);
    let edit = DecodedEdit::Bytes {
        offset: 0,
        old_len: 1,
        new_bytes: b"x".to_vec(),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn elf_translate_edit_instruction_patch_returns_in_place_byte_edit() {
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let result = codec
        .translate_edit(&bytes, &instruction_patch_edit(fixture))
        .unwrap();

    assert_eq!(
        result,
        Some(ByteEdit::replace(
            fixture.text_section_offset + fixture.text_patch_offset,
            &fixture.text_old_bytes,
            &fixture.text_new_bytes,
        ))
    );
}

#[test]
fn elf_translate_edit_symbol_rename_returns_in_place_byte_edit() {
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let result = codec
        .translate_edit(&bytes, &symbol_rename_edit(fixture, &fixture.symbol_new_name))
        .unwrap();

    assert_eq!(
        result,
        Some(ByteEdit::replace(
            fixture.symbol_name_offset,
            fixture.symbol_name.as_bytes(),
            fixture.symbol_new_name.as_bytes(),
        ))
    );
}

#[test]
fn elf_translate_edit_section_replace_returns_in_place_byte_edit() {
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let result = codec
        .translate_edit(&bytes, &section_replace_edit(fixture))
        .unwrap();

    assert_eq!(
        result,
        Some(ByteEdit::replace(
            fixture.data_section_offset,
            &fixture.section_old_bytes,
            &fixture.section_new_bytes,
        ))
    );
}

#[test]
fn elf_translate_edit_same_name_symbol_rename_is_noop() {
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    assert_eq!(
        codec.translate_edit(&bytes, &symbol_rename_edit(fixture, &fixture.symbol_name)),
        Ok(None)
    );
}

#[test]
fn elf_translate_edit_malformed_section_path_is_rejected() {
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["sections".to_string(), fixture.text_section_name.clone()]),
        op: TreeOp::new(ElfTreeOp::PatchBytes {
            offset: 0,
            old_bytes: fixture.text_old_bytes.clone(),
            new_bytes: fixture.text_new_bytes.clone(),
        }),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn elf_translate_edit_size_changing_symbol_rename_is_rejected() {
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    assert!(matches!(
        codec.translate_edit(&bytes, &symbol_rename_edit(fixture, "too_long_name")),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}

#[test]
fn elf_translate_edit_patch_requires_executable_section() {
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            fixture.data_section_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::PatchBytes {
            offset: 0,
            old_bytes: fixture.section_old_bytes[..4].to_vec(),
            new_bytes: fixture.section_new_bytes[..4].to_vec(),
        }),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn elf_real_fixture_workflow_reparses_and_restores_original_bytes() {
    let codec = Arc::new(ElfCodec::new());
    let fixture = elf_fixture().clone();
    let mut table = InodeTable::new();
    let inode_id = table.insert(Arc::new(HeapByteSource::new(fixture.bytes.clone())));
    let buffer_id = BufferId::from_raw(1);
    table.bind_file(buffer_id, inode_id);

    let source_mount = table
        .mount(
            inode_id,
            buffer_id,
            Mount::with_mode("elf-source", codec.clone(), MountMode::Structural),
        )
        .unwrap();
    let peer_mount = table
        .mount_additional(inode_id, buffer_id, Mount::new("elf-peer", codec.clone()))
        .unwrap();

    let edits = [
        instruction_patch_edit(&fixture),
        symbol_rename_edit(&fixture, &fixture.symbol_new_name),
        section_replace_edit(&fixture),
    ];
    let mut inverses = Vec::new();

    for edit in &edits {
        let byte_edit = table.apply_edit(source_mount, edit).unwrap().unwrap();
        inverses.push(byte_edit.inverse());

        let current = table.read_bytes(inode_id).unwrap();
        assert_eq!(current.len(), fixture.bytes.len());
        codec.decode(&current).unwrap();
    }

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
fn elf_machine_name_x86_64() {
    assert_eq!(elf_machine_name(goblin::elf::header::EM_X86_64), "x86-64");
}

#[test]
fn elf_machine_name_arm() {
    assert_eq!(elf_machine_name(goblin::elf::header::EM_ARM), "ARM");
}

#[test]
fn elf_machine_name_aarch64() {
    assert_eq!(elf_machine_name(goblin::elf::header::EM_AARCH64), "AArch64");
}

#[test]
fn elf_machine_name_riscv() {
    assert_eq!(elf_machine_name(goblin::elf::header::EM_RISCV), "RISC-V");
}

#[test]
fn elf_machine_name_unknown() {
    assert_eq!(elf_machine_name(0xFFFF), "Unknown");
}

#[test]
fn elf_machine_name_none() {
    assert_eq!(elf_machine_name(goblin::elf::header::EM_NONE), "None");
}

#[test]
fn elf_machine_name_mips() {
    assert_eq!(elf_machine_name(goblin::elf::header::EM_MIPS), "MIPS");
}

#[test]
fn elf_machine_name_ppc() {
    assert_eq!(elf_machine_name(goblin::elf::header::EM_PPC), "PowerPC");
}

#[test]
fn elf_machine_name_ppc64() {
    assert_eq!(elf_machine_name(goblin::elf::header::EM_PPC64), "PowerPC64");
}

#[test]
fn elf_machine_name_s390() {
    assert_eq!(elf_machine_name(goblin::elf::header::EM_S390), "S/390");
}

#[test]
fn elf_machine_name_sparc() {
    assert_eq!(elf_machine_name(goblin::elf::header::EM_SPARC), "SPARC");
}

#[test]
fn elf_machine_name_386() {
    assert_eq!(elf_machine_name(goblin::elf::header::EM_386), "x86");
}

#[test]
fn elf_section_type_progbits() {
    assert_eq!(elf_section_type(goblin::elf::section_header::SHT_PROGBITS), "PROGBITS");
}

#[test]
fn elf_section_type_symtab() {
    assert_eq!(elf_section_type(goblin::elf::section_header::SHT_SYMTAB), "SYMTAB");
}

#[test]
fn elf_section_type_strtab() {
    assert_eq!(elf_section_type(goblin::elf::section_header::SHT_STRTAB), "STRTAB");
}

#[test]
fn elf_section_type_dynamic() {
    assert_eq!(elf_section_type(goblin::elf::section_header::SHT_DYNAMIC), "DYNAMIC");
}

#[test]
fn elf_section_type_null() {
    assert_eq!(elf_section_type(goblin::elf::section_header::SHT_NULL), "NULL");
}

#[test]
fn elf_section_type_note() {
    assert_eq!(elf_section_type(goblin::elf::section_header::SHT_NOTE), "NOTE");
}

#[test]
fn elf_section_type_nobits() {
    assert_eq!(elf_section_type(goblin::elf::section_header::SHT_NOBITS), "NOBITS");
}

#[test]
fn elf_section_type_rela() {
    assert_eq!(elf_section_type(goblin::elf::section_header::SHT_RELA), "RELA");
}

#[test]
fn elf_section_type_rel() {
    assert_eq!(elf_section_type(goblin::elf::section_header::SHT_REL), "REL");
}

#[test]
fn elf_section_type_hash() {
    assert_eq!(elf_section_type(goblin::elf::section_header::SHT_HASH), "HASH");
}

#[test]
fn elf_section_type_dynsym() {
    assert_eq!(elf_section_type(goblin::elf::section_header::SHT_DYNSYM), "DYNSYM");
}

#[test]
fn elf_section_type_init_array() {
    assert_eq!(elf_section_type(goblin::elf::section_header::SHT_INIT_ARRAY), "INIT_ARRAY");
}

#[test]
fn elf_section_type_fini_array() {
    assert_eq!(elf_section_type(goblin::elf::section_header::SHT_FINI_ARRAY), "FINI_ARRAY");
}

#[test]
fn elf_section_type_unknown() {
    assert_eq!(elf_section_type(0xFFFF_FFFF), "OTHER");
}

#[test]
fn elf_decode_real_binary() {
    // Read the actual reovim binary if available, or /bin/ls
    let path = if std::path::Path::new("/bin/ls").exists() {
        "/bin/ls"
    } else {
        return; // Skip on systems without /bin/ls
    };
    let data = std::fs::read(path).unwrap();
    let codec = ElfCodec::new();
    let result = codec.decode(&data).unwrap();

    assert!(result.content.contains("ELF Binary Summary"));
    assert!(result.content.contains("Section Table"));
    assert!(result.lossy);
    assert!(result.readonly);
    assert!(!result.annotations.is_empty());

    // Should have header annotations
    let header_count = result
        .annotations
        .iter()
        .filter(|a| a.kind.name() == ELF_HEADER_KIND)
        .count();
    assert!(header_count >= 2); // Title + Section Table header

    // Should have section annotations
    assert!(
        result
            .annotations
            .iter()
            .any(|a| a.kind.name() == ELF_SECTION_KIND)
    );

    // All annotations in content namespace
    for a in &result.annotations {
        assert_eq!(a.kind.namespace(), Some("content"));
    }
}

#[test]
fn elf_metadata_has_file_size() {
    let path = if std::path::Path::new("/bin/ls").exists() {
        "/bin/ls"
    } else {
        return;
    };
    let data = std::fs::read(path).unwrap();
    let codec = ElfCodec::new();
    let result = codec.decode(&data).unwrap();
    assert_eq!(result.metadata.get("file_size"), Some(data.len().to_string()).as_deref());
}

// === ZIP Codec Tests ===

#[test]
fn zip_decode_invalid() {
    let codec = ZipCodec::new();
    let result = codec.decode(b"not a zip");
    assert!(result.is_err());
}

#[test]
fn zip_default_impl() {
    let codec = ZipCodec;
    assert_eq!(std::mem::size_of_val(&codec), std::mem::size_of::<ZipCodec>());
}

#[test]
fn zip_translate_edit_text_is_not_supported() {
    use reovim_driver_codec::TranslateEditError;
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(b"PK\x03\x04");
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
fn zip_translate_edit_bytes_is_not_supported() {
    use reovim_driver_codec::TranslateEditError;
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(b"PK\x03\x04");
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
fn zip_decode_real_archive() {
    // Create a minimal ZIP in memory
    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let mut writer = zip::ZipWriter::new(cursor);

    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    writer.start_file("hello.txt", options).unwrap();
    std::io::Write::write_all(&mut writer, b"Hello, world!").unwrap();

    writer.start_file("dir/nested.txt", options).unwrap();
    std::io::Write::write_all(&mut writer, b"Nested content").unwrap();

    let result_cursor = writer.finish().unwrap();
    let zip_bytes = result_cursor.into_inner();

    let codec = ZipCodec::new();
    let result = codec.decode(&zip_bytes).unwrap();

    assert!(result.content.contains("ZIP Archive Contents"));
    assert!(result.content.contains("hello.txt"));
    assert!(result.content.contains("dir/nested.txt"));
    assert!(result.content.contains("Entries: 2"));
    assert!(result.lossy);
    assert!(result.readonly);
    assert_eq!(result.metadata.get("entry_count"), Some("2"));

    // Check annotations
    let header_count = result
        .annotations
        .iter()
        .filter(|a| a.kind.name() == ZIP_HEADER_KIND)
        .count();
    assert!(header_count >= 2); // Title + column headers

    let entry_count = result
        .annotations
        .iter()
        .filter(|a| a.kind.name() == ZIP_ENTRY_KIND)
        .count();
    assert_eq!(entry_count, 2);

    // All annotations in content namespace
    for a in &result.annotations {
        assert_eq!(a.kind.namespace(), Some("content"));
    }
}

#[test]
fn zip_decode_empty_archive() {
    // Create an empty ZIP
    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let writer = zip::ZipWriter::new(cursor);
    let result_cursor = writer.finish().unwrap();
    let zip_bytes = result_cursor.into_inner();

    let codec = ZipCodec::new();
    let result = codec.decode(&zip_bytes).unwrap();

    assert!(result.content.contains("Entries: 0"));
    assert!(result.lossy);
    assert!(result.readonly);
}

#[test]
fn zip_metadata_has_file_size() {
    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let writer = zip::ZipWriter::new(cursor);
    let result_cursor = writer.finish().unwrap();
    let zip_bytes = result_cursor.into_inner();

    let codec = ZipCodec::new();
    let result = codec.decode(&zip_bytes).unwrap();
    assert_eq!(result.metadata.get("file_size"), Some(zip_bytes.len().to_string()).as_deref());
}

// ============================================================================
// ZIP structural editing tests (Plan 07 Phase 4)
// ============================================================================

/// Build a real zip archive fixture for structural editing tests.
///
/// The fixture has:
/// - An archive comment "TESTCOMMENT!" (12 bytes, patchable same-length)
/// - A STORED entry "hello.txt" with payload b"Hello World!" (12 bytes)
/// - A DEFLATED entry "compress__.txt" with payload b"Compressed content"
///
/// Entry names are chosen to be exactly the right length for same-length
/// rename tests (e.g. "hello.txt" → "world.txt", both 9 bytes).
fn build_zip_fixture() -> Vec<u8> {
    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let mut writer = zip::ZipWriter::new(cursor);

    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("hello.txt", stored).unwrap();
    std::io::Write::write_all(&mut writer, b"Hello World!").unwrap();

    writer.start_file("compress__.txt", stored).unwrap();
    std::io::Write::write_all(&mut writer, b"Compressed content").unwrap();

    writer.set_comment("TESTCOMMENT!");

    let result_cursor = writer.finish().unwrap();
    result_cursor.into_inner()
}

#[derive(Debug, Clone)]
struct ZipFixture {
    bytes: Vec<u8>,
}

static ZIP_FIXTURE: OnceLock<ZipFixture> = OnceLock::new();

fn zip_fixture() -> &'static ZipFixture {
    ZIP_FIXTURE.get_or_init(|| ZipFixture {
        bytes: build_zip_fixture(),
    })
}

fn zip_comment_edit(new_comment: &[u8]) -> DecodedEdit {
    DecodedEdit::Tree {
        path: TreePath::new(vec!["archive".to_string(), "comment".to_string()]),
        op: TreeOp::new(ZipTreeOp::ReplaceComment {
            new_comment: new_comment.to_vec(),
        }),
    }
}

fn zip_entry_bytes_edit(name: &str, old_bytes: &[u8], new_bytes: &[u8]) -> DecodedEdit {
    DecodedEdit::Tree {
        path: TreePath::new(vec!["entries".to_string(), name.to_string(), "bytes".to_string()]),
        op: TreeOp::new(ZipTreeOp::ReplaceEntryBytes {
            old_bytes: old_bytes.to_vec(),
            new_bytes: new_bytes.to_vec(),
        }),
    }
}

fn zip_rename_edit(name: &str, new_name: &str) -> DecodedEdit {
    DecodedEdit::Tree {
        path: TreePath::new(vec!["entries".to_string(), name.to_string(), "name".to_string()]),
        op: TreeOp::new(ZipTreeOp::RenameEntry {
            new_name: new_name.to_string(),
        }),
    }
}

// --- Positive path tests ---

#[test]
fn zip_noop_round_trip_is_byte_identical() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();

    let result = codec.decode(&fixture.bytes).unwrap();
    assert!(result.content.contains("ZIP Archive Contents"));
    assert!(result.content.contains("hello.txt"));

    let cursor = std::io::Cursor::new(&fixture.bytes);
    let archive = zip::ZipArchive::new(cursor).unwrap();
    assert_eq!(archive.comment(), b"TESTCOMMENT!");
}

#[test]
fn zip_replace_comment_same_length_succeeds() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = zip_comment_edit(b"NEWCOMMENT!!");
    let byte_edit = codec.translate_edit(&bytes, &edit).unwrap().unwrap();

    let mut patched = fixture.bytes.clone();
    patched[byte_edit.offset..byte_edit.offset + byte_edit.old_bytes.len()]
        .copy_from_slice(&byte_edit.new_bytes);

    let cursor = std::io::Cursor::new(&patched);
    let archive = zip::ZipArchive::new(cursor).unwrap();
    assert_eq!(archive.comment(), b"NEWCOMMENT!!");
}

#[test]
fn zip_replace_comment_noop_returns_none() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = zip_comment_edit(b"TESTCOMMENT!");
    let result = codec.translate_edit(&bytes, &edit).unwrap();
    assert!(result.is_none());
}

#[test]
fn zip_replace_stored_entry_bytes_succeeds() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = zip_entry_bytes_edit("hello.txt", b"Hello World!", b"Patched Data");
    let byte_edit = codec.translate_edit(&bytes, &edit).unwrap().unwrap();

    let mut patched = fixture.bytes.clone();
    patched[byte_edit.offset..byte_edit.offset + byte_edit.old_bytes.len()]
        .copy_from_slice(&byte_edit.new_bytes);

    let cursor = std::io::Cursor::new(&patched);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    // Use by_index_raw to read without CRC validation — the in-place
    // data patch leaves the stored CRC stale.
    let mut entry = archive.by_index_raw(0).unwrap();
    assert_eq!(entry.name(), "hello.txt");
    let mut content = Vec::new();
    std::io::Read::read_to_end(&mut entry, &mut content).unwrap();
    assert_eq!(content, b"Patched Data");
}

#[test]
fn zip_replace_entry_bytes_noop_returns_none() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = zip_entry_bytes_edit("hello.txt", b"Hello World!", b"Hello World!");
    let result = codec.translate_edit(&bytes, &edit).unwrap();
    assert!(result.is_none());
}

#[test]
fn zip_rename_entry_same_length_succeeds() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = zip_rename_edit("hello.txt", "world.txt");
    let byte_edit = codec.translate_edit(&bytes, &edit).unwrap().unwrap();

    let mut patched = fixture.bytes.clone();
    patched[byte_edit.offset..byte_edit.offset + byte_edit.old_bytes.len()]
        .copy_from_slice(&byte_edit.new_bytes);

    let cursor = std::io::Cursor::new(&patched);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    assert!(archive.by_name("world.txt").is_ok());
    assert!(archive.by_name("hello.txt").is_err());

    let mut entry = archive.by_name("world.txt").unwrap();
    let mut content = Vec::new();
    std::io::Read::read_to_end(&mut entry, &mut content).unwrap();
    assert_eq!(content, b"Hello World!");
}

#[test]
fn zip_rename_entry_noop_returns_none() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = zip_rename_edit("hello.txt", "hello.txt");
    let result = codec.translate_edit(&bytes, &edit).unwrap();
    assert!(result.is_none());
}

// --- Shared structural gates ---

#[test]
fn zip_edit_propagates_through_peer_stale() {
    let fixture = zip_fixture();
    let zip_codec: Arc<dyn ContentCodec> = Arc::new(ZipCodec::new());
    let peer_codec: Arc<dyn ContentCodec> = Arc::new(ZipCodec::new());
    let buffer_id = BufferId::from_raw(1);

    let mut table = InodeTable::new();
    let inode_id = table.insert(Arc::new(HeapByteSource::new(fixture.bytes.clone())));
    table.bind_file(buffer_id, inode_id);

    let source_mount = table
        .mount(
            inode_id,
            buffer_id,
            Mount::with_mode("zip-source", zip_codec, MountMode::Structural),
        )
        .unwrap();
    let peer_mount = table
        .mount_additional(inode_id, buffer_id, Mount::new("zip-peer", peer_codec))
        .unwrap();

    let edit = zip_comment_edit(b"NEWCOMMENT!!");
    table.apply_edit(source_mount, &edit).unwrap();

    let inode = table.lookup_inode(inode_id).unwrap();
    let peer = inode.mounts.get(&peer_mount.mount_id()).unwrap();
    assert!(!peer.content_valid);
}

#[test]
fn zip_undo_restores_exact_original_bytes() {
    let fixture = zip_fixture();
    let zip_codec: Arc<dyn ContentCodec> = Arc::new(ZipCodec::new());
    let buffer_id = BufferId::from_raw(1);

    let mut table = InodeTable::new();
    let inode_id = table.insert(Arc::new(HeapByteSource::new(fixture.bytes.clone())));
    table.bind_file(buffer_id, inode_id);

    let mount = table
        .mount(inode_id, buffer_id, Mount::with_mode("zip", zip_codec, MountMode::Structural))
        .unwrap();

    let edit = zip_entry_bytes_edit("hello.txt", b"Hello World!", b"Patched Data");
    let byte_edit = table.apply_edit(mount, &edit).unwrap().unwrap();

    // Undo via inverse ByteEdit.
    table
        .apply_byte_edit(inode_id, &byte_edit.inverse())
        .unwrap();

    let restored = table.read_bytes(inode_id).unwrap();
    assert_eq!(restored, fixture.bytes);
}

// --- Explicit refusal tests ---

#[test]
fn zip_replace_comment_wrong_length_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = zip_comment_edit(b"short");
    let result = codec.translate_edit(&bytes, &edit);
    assert!(matches!(result, Err(TranslateEditError::ConstraintViolation { .. })));
}

#[test]
fn zip_replace_entry_bytes_wrong_size_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = zip_entry_bytes_edit("hello.txt", b"Hello World!", b"short");
    let result = codec.translate_edit(&bytes, &edit);
    assert!(matches!(result, Err(TranslateEditError::ConstraintViolation { .. })));
}

#[test]
fn zip_replace_entry_bytes_old_mismatch_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = zip_entry_bytes_edit("hello.txt", b"Wrong data!!", b"Patched Data");
    let result = codec.translate_edit(&bytes, &edit);
    assert!(matches!(result, Err(TranslateEditError::ConstraintViolation { .. })));
}

#[test]
fn zip_rename_entry_wrong_length_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = zip_rename_edit("hello.txt", "toolongname.txt");
    let result = codec.translate_edit(&bytes, &edit);
    assert!(matches!(result, Err(TranslateEditError::ConstraintViolation { .. })));
}

#[test]
fn zip_entry_not_found_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = zip_entry_bytes_edit("missing.txt", b"data", b"data");
    let result = codec.translate_edit(&bytes, &edit);
    assert!(matches!(result, Err(TranslateEditError::MalformedPath { .. })));
}

#[test]
fn zip_malformed_path_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["invalid".to_string()]),
        op: TreeOp::new(ZipTreeOp::ReplaceComment {
            new_comment: b"x".to_vec(),
        }),
    };
    let result = codec.translate_edit(&bytes, &edit);
    assert!(matches!(result, Err(TranslateEditError::MalformedPath { .. })));
}

#[test]
fn zip_malformed_path_unknown_field_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "entries".to_string(),
            "hello.txt".to_string(),
            "metadata".to_string(),
        ]),
        op: TreeOp::new(ZipTreeOp::ReplaceComment {
            new_comment: b"x".to_vec(),
        }),
    };
    let result = codec.translate_edit(&bytes, &edit);
    assert!(matches!(result, Err(TranslateEditError::MalformedPath { .. })));
}

#[test]
fn zip_wrong_op_for_comment_path_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["archive".to_string(), "comment".to_string()]),
        op: TreeOp::new(ZipTreeOp::RenameEntry {
            new_name: "x".to_string(),
        }),
    };
    let result = codec.translate_edit(&bytes, &edit);
    assert!(matches!(result, Err(TranslateEditError::UnsupportedEdit { .. })));
}

#[test]
fn zip_wrong_op_for_entry_bytes_path_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "entries".to_string(),
            "hello.txt".to_string(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ZipTreeOp::RenameEntry {
            new_name: "world.txt".to_string(),
        }),
    };
    let result = codec.translate_edit(&bytes, &edit);
    assert!(matches!(result, Err(TranslateEditError::UnsupportedEdit { .. })));
}

#[test]
fn zip_wrong_op_for_entry_name_path_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "entries".to_string(),
            "hello.txt".to_string(),
            "name".to_string(),
        ]),
        op: TreeOp::new(ZipTreeOp::ReplaceEntryBytes {
            old_bytes: b"Hello World!".to_vec(),
            new_bytes: b"Patched Data".to_vec(),
        }),
    };
    let result = codec.translate_edit(&bytes, &edit);
    assert!(matches!(result, Err(TranslateEditError::UnsupportedEdit { .. })));
}

#[test]
fn zip_non_zip_tree_op_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "entries".to_string(),
            "hello.txt".to_string(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::PatchBytes {
            offset: 0,
            old_bytes: vec![0],
            new_bytes: vec![1],
        }),
    };
    let result = codec.translate_edit(&bytes, &edit);
    assert!(matches!(result, Err(TranslateEditError::UnsupportedEdit { .. })));
}

#[test]
fn zip_malformed_bytes_fails_on_translate() {
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(b"not a zip at all".to_vec());

    let edit = zip_comment_edit(b"test");
    let result = codec.translate_edit(&bytes, &edit);
    assert!(matches!(result, Err(TranslateEditError::Internal { .. })));
}

#[test]
fn zip_empty_entry_name_in_path_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["entries".to_string(), String::new(), "bytes".to_string()]),
        op: TreeOp::new(ZipTreeOp::ReplaceEntryBytes {
            old_bytes: vec![0],
            new_bytes: vec![1],
        }),
    };
    let result = codec.translate_edit(&bytes, &edit);
    assert!(matches!(result, Err(TranslateEditError::MalformedPath { .. })));
}

// --- Real fixture workflow: full sequence ---

#[test]
fn zip_real_fixture_workflow_reparses_and_restores_original_bytes() {
    let fixture = zip_fixture();
    let codec = Arc::new(ZipCodec::new());
    let buffer_id = BufferId::from_raw(1);

    let mut table = InodeTable::new();
    let inode_id = table.insert(Arc::new(HeapByteSource::new(fixture.bytes.clone())));
    table.bind_file(buffer_id, inode_id);

    let mount = table
        .mount(
            inode_id,
            buffer_id,
            Mount::with_mode("zip", codec.clone(), MountMode::Structural),
        )
        .unwrap();

    let edits = [
        zip_comment_edit(b"NEWCOMMENT!!"),
        zip_entry_bytes_edit("hello.txt", b"Hello World!", b"Patched Data"),
        zip_rename_edit("hello.txt", "world.txt"),
    ];
    let mut inverses = Vec::new();

    for edit in &edits {
        let byte_edit = table.apply_edit(mount, edit).unwrap().unwrap();
        inverses.push(byte_edit.inverse());

        let current = table.read_bytes(inode_id).unwrap();
        assert_eq!(current.len(), fixture.bytes.len());
        let decoded = codec.decode(&current).unwrap();
        assert!(decoded.content.contains("ZIP Archive Contents"));
    }

    // Verify final state has all three edits applied.
    let current = table.read_bytes(inode_id).unwrap();
    let cursor = std::io::Cursor::new(&current);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    assert_eq!(archive.comment(), b"NEWCOMMENT!!");
    assert!(archive.by_name("world.txt").is_ok());
    assert!(archive.by_name("hello.txt").is_err());

    // Undo all three edits via inverse ByteEdits.
    for inverse in inverses.iter().rev() {
        table.apply_byte_edit(inode_id, inverse).unwrap();
    }

    let restored = table.read_bytes(inode_id).unwrap();
    assert_eq!(restored, fixture.bytes);
    codec.decode(&restored).unwrap();
}

#[test]
fn zip_accepted_edit_preserves_archive_length() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = zip_entry_bytes_edit("hello.txt", b"Hello World!", b"Patched Data");
    let result = codec.translate_edit(&bytes, &edit).unwrap().unwrap();
    assert_eq!(result.old_bytes.len(), result.new_bytes.len());
}

#[test]
fn zip_rename_preserves_archive_length() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = zip_rename_edit("hello.txt", "world.txt");
    let result = codec.translate_edit(&bytes, &edit).unwrap().unwrap();
    assert_eq!(result.old_bytes.len(), result.new_bytes.len());
}
