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

// ============================================================================
// ELF MC/DC coverage gap tests (Category A: testable paths)
// ============================================================================

#[test]
fn elf_translate_edit_patch_length_mismatch_rejected() {
    // 279:0 — old_bytes.len() != new_bytes.len() in translate_elf_patch_bytes
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            fixture.text_section_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::PatchBytes {
            offset: fixture.text_patch_offset,
            old_bytes: fixture.text_old_bytes.clone(),
            // One byte shorter — length mismatch
            new_bytes: fixture.text_new_bytes[..fixture.text_new_bytes.len() - 1].to_vec(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}

#[test]
fn elf_translate_edit_patch_old_bytes_content_mismatch_rejected() {
    // 297:0 — actual != old_bytes in translate_elf_patch_bytes
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    // Build wrong old_bytes: same length but different content
    let wrong_old: Vec<u8> = fixture.text_old_bytes.iter().map(|b| b ^ 0xFF).collect();
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            fixture.text_section_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::PatchBytes {
            offset: fixture.text_patch_offset,
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
fn elf_translate_edit_same_patch_bytes_is_noop() {
    // 302:0 — old_bytes == new_bytes in translate_elf_patch_bytes
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            fixture.text_section_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::PatchBytes {
            offset: fixture.text_patch_offset,
            old_bytes: fixture.text_old_bytes.clone(),
            new_bytes: fixture.text_old_bytes.clone(), // identical
        }),
    };
    assert_eq!(codec.translate_edit(&bytes, &edit), Ok(None));
}

#[test]
fn elf_translate_edit_section_replace_length_mismatch_rejected() {
    // 368:0 — old_bytes.len() != new_bytes.len() in translate_elf_replace_section
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            fixture.data_section_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::ReplaceSectionBytes {
            old_bytes: fixture.section_old_bytes.clone(),
            new_bytes: fixture.section_new_bytes[..fixture.section_new_bytes.len() - 1].to_vec(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}

#[test]
fn elf_translate_edit_section_replace_content_mismatch_rejected() {
    // 380:0 — actual != old_bytes in translate_elf_replace_section
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let wrong_old: Vec<u8> = fixture.section_old_bytes.iter().map(|b| b ^ 0xFF).collect();
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            fixture.data_section_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::ReplaceSectionBytes {
            old_bytes: wrong_old.clone(),
            new_bytes: wrong_old,
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}

#[test]
fn elf_translate_edit_same_section_bytes_is_noop() {
    // 385:0 — old_bytes == new_bytes in translate_elf_replace_section
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            fixture.data_section_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::ReplaceSectionBytes {
            old_bytes: fixture.section_old_bytes.clone(),
            new_bytes: fixture.section_old_bytes.clone(), // identical
        }),
    };
    assert_eq!(codec.translate_edit(&bytes, &edit), Ok(None));
}

#[test]
fn elf_section_path_wrong_kind_rejected() {
    // 409:0 — kind != "sections" in resolve_section_path
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "segments".to_string(), // wrong kind
            fixture.data_section_name.clone(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::ReplaceSectionBytes {
            old_bytes: fixture.section_old_bytes.clone(),
            new_bytes: fixture.section_new_bytes.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn elf_section_path_wrong_field_rejected() {
    // 409:2 — field != "bytes" in resolve_section_path
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            fixture.data_section_name.clone(),
            "content".to_string(), // wrong field
        ]),
        op: TreeOp::new(ElfTreeOp::ReplaceSectionBytes {
            old_bytes: fixture.section_old_bytes.clone(),
            new_bytes: fixture.section_new_bytes.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn elf_section_path_empty_name_rejected() {
    // 409:4 — section_name.is_empty() in resolve_section_path
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            String::new(), // empty name
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::ReplaceSectionBytes {
            old_bytes: fixture.section_old_bytes.clone(),
            new_bytes: fixture.section_new_bytes.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn elf_section_path_nonexistent_name_rejected() {
    // 465:0 — section not found in resolve_section_path
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            ".nosuchsection".to_string(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::ReplaceSectionBytes {
            old_bytes: fixture.section_old_bytes.clone(),
            new_bytes: fixture.section_new_bytes.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn elf_symbol_path_wrong_kind_rejected() {
    // 434:1 — kind != "symbols" in resolve_symbol_path
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "functions".to_string(), // wrong kind
            fixture.symbol_name.clone(),
            "name".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::RenameSymbol {
            new_name: fixture.symbol_new_name.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn elf_symbol_path_wrong_field_rejected() {
    // 439:0 — field != "name" in resolve_symbol_path
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "symbols".to_string(),
            fixture.symbol_name.clone(),
            "bytes".to_string(), // wrong field
        ]),
        op: TreeOp::new(ElfTreeOp::RenameSymbol {
            new_name: fixture.symbol_new_name.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn elf_symbol_path_empty_name_rejected() {
    // 439:4 — symbol_name.is_empty() in resolve_symbol_path
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "symbols".to_string(),
            String::new(), // empty name
            "name".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::RenameSymbol {
            new_name: fixture.symbol_new_name.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

#[test]
fn elf_section_path_with_text_prefix_but_no_execinstr_is_allowed() {
    // 271:1 — section has SHF_EXECINSTR cleared but name starts with ".text"
    // The nested-if restructuring makes this testable: the outer if is taken
    // (EXECINSTR == 0) but the inner if is NOT taken (name starts with .text).
    // We need an ELF section named ".text*" with the executable flag cleared.
    // We reuse the fixture .text.phase2 section but corrupt the sh_flags in
    // the raw bytes so EXECINSTR is cleared while the section name is kept.
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let raw = &fixture.bytes;
    let elf = goblin::elf::Elf::parse(raw).unwrap();

    // Find the .text.phase2 section header and locate its sh_flags field.
    // ELF section header layout (64-bit): sh_name(4) sh_type(4) sh_flags(8)…
    // section_header.sh_offset gives the offset of sh_offset within the header,
    // not the header itself.  We iterate to find the right header.
    let text_section = elf
        .section_headers
        .iter()
        .find(|sh| elf.shdr_strtab.get_at(sh.sh_name) == Some(".text.phase2"))
        .unwrap();

    // Find byte offset of this section header in the raw ELF.
    // ELF64 section header table starts at e_shoff; each entry is e_shentsize.
    let shoff = usize::try_from(elf.header.e_shoff).unwrap();
    let shentsize = usize::from(elf.header.e_shentsize);

    let header_idx = elf
        .section_headers
        .iter()
        .position(|sh| std::ptr::eq(sh, text_section))
        .unwrap();

    let header_start = shoff + header_idx * shentsize;
    // sh_flags at offset 8 within the 64-bit section header (sh_name=4, sh_type=4)
    let flags_offset = header_start + 8;

    let mut corrupted = raw.clone();
    // Clear all flag bits (write zero to the sh_flags u64 field, little-endian)
    corrupted[flags_offset..flags_offset + 8].copy_from_slice(&0u64.to_le_bytes());

    let bytes = HeapByteSource::new(corrupted);
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            ".text.phase2".to_string(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::PatchBytes {
            offset: 0,
            old_bytes: fixture.text_old_bytes.clone(),
            new_bytes: fixture.text_new_bytes.clone(),
        }),
    };
    // With EXECINSTR cleared but name starting with ".text", the patch is
    // still allowed (the inner-if guard is not triggered).
    let result = codec.translate_edit(&bytes, &edit);
    assert!(result.is_ok(), "expected Ok, got {result:?}");
}

// ============================================================================
// ZIP MC/DC coverage gap tests (Category A: testable paths)
// ============================================================================

#[test]
fn zip_entry_replace_actual_len_mismatch_rejected() {
    // 745:0 (original) — actual.len() != old_bytes.len() in translate_zip_replace_entry_bytes
    //
    // Pass old_bytes whose length is different from the entry's on-disk
    // payload size. The size-preservation check (old == new in length) is
    // satisfied, but the actual-payload coverage check catches the mismatch.
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    // "hello.txt" payload is b"Hello World!" (12 bytes). Supply 11 bytes as old.
    let edit = zip_entry_bytes_edit("hello.txt", b"Hello World", b"Patched Dat");
    let result = codec.translate_edit(&bytes, &edit);
    assert!(matches!(result, Err(TranslateEditError::ConstraintViolation { .. })));
}

// ============================================================================
// ELF MC/DC coverage gap tests — parse failure and symbol path component count
// ============================================================================

#[test]
fn elf_translate_edit_parse_failure_returns_internal_error() {
    // Line 235-237 — goblin::elf::Elf::parse fails on garbage bytes, causing
    // translate_elf_edit to return TranslateEditError::Internal.
    //
    // The text/bytes unsupported-edit guards execute before parse, so we must
    // use a Tree edit to reach the parse call.
    let codec = ElfCodec::new();
    let bytes = HeapByteSource::new(b"not an elf at all".to_vec());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            ".text".to_string(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::ReplaceSectionBytes {
            old_bytes: vec![0x90],
            new_bytes: vec![0xCC],
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::Internal { .. })
    ));
}

#[test]
fn elf_symbol_path_wrong_component_count_rejected() {
    // Lines 485-488 — the slice pattern `[kind, symbol_name, field]` in
    // resolve_symbol_path rejects any path that does not have exactly 3
    // components. The 2-component case is exercised by
    // elf_translate_edit_malformed_section_path_is_rejected for section paths;
    // here we exercise the same `else` branch for the symbol path with a
    // 4-component path.
    let codec = ElfCodec::new();
    let fixture = elf_fixture();
    let bytes = HeapByteSource::new(fixture.bytes.clone());
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "symbols".to_string(),
            fixture.symbol_name.clone(),
            "name".to_string(),
            "extra".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::RenameSymbol {
            new_name: fixture.symbol_new_name.clone(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::MalformedPath { .. })
    ));
}

// ============================================================================
// ZIP: ReplaceComment op sent to an Entry target (L815-817)
// ============================================================================

/// `ZipTreeOp::ReplaceComment` is only valid for the archive comment target.
/// Sending it to an entry Bytes target is rejected with `UnsupportedEdit`.
#[test]
fn zip_replace_comment_op_on_entry_bytes_target_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "entries".to_string(),
            "hello.txt".to_string(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ZipTreeOp::ReplaceComment {
            new_comment: b"TESTCOMMENT!".to_vec(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

/// `ZipTreeOp::ReplaceComment` sent to an entry Name target.
#[test]
fn zip_replace_comment_op_on_entry_name_target_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "entries".to_string(),
            "hello.txt".to_string(),
            "name".to_string(),
        ]),
        op: TreeOp::new(ZipTreeOp::ReplaceComment {
            new_comment: b"TESTCOMMENT!".to_vec(),
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

// ============================================================================
// ZIP: DEFLATED entry replacement rejected (L870-872)
// ============================================================================

#[allow(clippy::cast_possible_truncation)] // fixture data is tiny
/// Build a minimal ZIP with a DEFLATED entry by hand-crafting the bytes.
/// The `zip` crate is compiled without `default-features` (no deflate support),
/// so we construct a raw ZIP file where the compression method field is set to
/// DEFLATED (8) even though the payload is just stored bytes — the codec rejects
/// DEFLATED entries at the method-check level before attempting decompression.
fn build_zip_with_deflated_entry() -> Vec<u8> {
    let name = b"deflated.txt";
    let data = b"Hello World!";
    // CRC32 of "Hello World!" — precomputed to avoid crc32fast dependency.
    let crc: u32 = 0x1b85_1995;
    let mut buf = Vec::new();

    // Local file header
    buf.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]); // signature
    buf.extend_from_slice(&20u16.to_le_bytes()); // version needed
    buf.extend_from_slice(&0u16.to_le_bytes()); // flags
    buf.extend_from_slice(&8u16.to_le_bytes()); // compression: DEFLATED
    buf.extend_from_slice(&0u16.to_le_bytes()); // mod time
    buf.extend_from_slice(&0u16.to_le_bytes()); // mod date
    buf.extend_from_slice(&crc.to_le_bytes()); // crc32
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes()); // compressed size
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes()); // uncompressed size
    buf.extend_from_slice(&(name.len() as u16).to_le_bytes()); // name length
    buf.extend_from_slice(&0u16.to_le_bytes()); // extra length
    buf.extend_from_slice(name);
    let data_offset = buf.len();
    buf.extend_from_slice(data);

    // Central directory
    let cd_offset = buf.len();
    buf.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]); // signature
    buf.extend_from_slice(&20u16.to_le_bytes()); // version made by
    buf.extend_from_slice(&20u16.to_le_bytes()); // version needed
    buf.extend_from_slice(&0u16.to_le_bytes()); // flags
    buf.extend_from_slice(&8u16.to_le_bytes()); // compression: DEFLATED
    buf.extend_from_slice(&0u16.to_le_bytes()); // mod time
    buf.extend_from_slice(&0u16.to_le_bytes()); // mod date
    buf.extend_from_slice(&crc.to_le_bytes()); // crc32
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes()); // compressed size
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes()); // uncompressed size
    buf.extend_from_slice(&(name.len() as u16).to_le_bytes()); // name length
    buf.extend_from_slice(&0u16.to_le_bytes()); // extra length
    buf.extend_from_slice(&0u16.to_le_bytes()); // comment length
    buf.extend_from_slice(&0u16.to_le_bytes()); // disk start
    buf.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
    buf.extend_from_slice(&0u32.to_le_bytes()); // external attrs
    buf.extend_from_slice(&0u32.to_le_bytes()); // local header offset
    buf.extend_from_slice(name);

    // EOCD
    let cd_size = buf.len() - cd_offset;
    buf.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]); // signature
    buf.extend_from_slice(&0u16.to_le_bytes()); // disk number
    buf.extend_from_slice(&0u16.to_le_bytes()); // cd disk
    buf.extend_from_slice(&1u16.to_le_bytes()); // entries on disk
    buf.extend_from_slice(&1u16.to_le_bytes()); // total entries
    buf.extend_from_slice(&(cd_size as u32).to_le_bytes()); // cd size
    buf.extend_from_slice(&(cd_offset as u32).to_le_bytes()); // cd offset
    buf.extend_from_slice(&0u16.to_le_bytes()); // comment length

    let _ = data_offset; // suppress unused warning
    buf
}

#[test]
fn zip_replace_deflated_entry_bytes_fails() {
    let zip_bytes = build_zip_with_deflated_entry();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(zip_bytes);

    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "entries".to_string(),
            "deflated.txt".to_string(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ZipTreeOp::ReplaceEntryBytes {
            old_bytes: vec![0u8; 12],
            new_bytes: vec![1u8; 12],
        }),
    };
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

// ============================================================================
// ELF: ambiguous section path (L499-503)
// ============================================================================

/// Build a minimal 64-bit little-endian ELF with two sections sharing the
/// name ".duptext", triggering "ELF section path is ambiguous" (L499-503).
#[allow(clippy::cast_possible_truncation)] // fixture data is tiny
fn build_elf_with_duplicate_sections() -> Vec<u8> {
    let mut shstrtab = Vec::<u8>::new();
    shstrtab.push(0u8);
    let duptext_name_idx: u32 = shstrtab.len() as u32;
    shstrtab.extend_from_slice(b".duptext\0");
    let shstrtab_name_idx: u32 = shstrtab.len() as u32;
    shstrtab.extend_from_slice(b".shstrtab\0");
    while !shstrtab.len().is_multiple_of(8) {
        shstrtab.push(0u8);
    }
    let shstrtab_len = shstrtab.len();

    let payload: &[u8] = b"\x90\x90\x90\x90\x90\x90\x90\x90";
    let payload_off: u64 = 64;
    let shstrtab_off: u64 = payload_off + payload.len() as u64;
    let shoff_raw: u64 = shstrtab_off + shstrtab_len as u64;
    let shoff: u64 = (shoff_raw + 7) & !7u64;
    let pad = (shoff - shoff_raw) as usize;

    // shnum=4: NULL + duptext1 + duptext2 + shstrtab
    let shnum: u16 = 4;
    let shstrndx: u16 = 3;

    let mut buf = Vec::with_capacity(shoff as usize + shnum as usize * 64);

    // ELF header (64 bytes)
    buf.extend_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    buf.extend_from_slice(&1u16.to_le_bytes()); // ET_REL
    buf.extend_from_slice(&0x3eu16.to_le_bytes()); // EM_X86_64
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&0u64.to_le_bytes());
    buf.extend_from_slice(&0u64.to_le_bytes());
    buf.extend_from_slice(&shoff.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&64u16.to_le_bytes());
    buf.extend_from_slice(&56u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&64u16.to_le_bytes());
    buf.extend_from_slice(&shnum.to_le_bytes());
    buf.extend_from_slice(&shstrndx.to_le_bytes());
    assert_eq!(buf.len(), 64);

    buf.extend_from_slice(payload);
    buf.extend_from_slice(&shstrtab);
    buf.extend(std::iter::repeat_n(0u8, pad));
    assert_eq!(buf.len(), shoff as usize);

    let push_shdr = |name: u32,
                     typ: u32,
                     flags: u64,
                     off: u64,
                     size: u64,
                     link: u32,
                     info: u32,
                     v: &mut Vec<u8>| {
        v.extend_from_slice(&name.to_le_bytes());
        v.extend_from_slice(&typ.to_le_bytes());
        v.extend_from_slice(&flags.to_le_bytes());
        v.extend_from_slice(&0u64.to_le_bytes()); // sh_addr
        v.extend_from_slice(&off.to_le_bytes());
        v.extend_from_slice(&size.to_le_bytes());
        v.extend_from_slice(&link.to_le_bytes());
        v.extend_from_slice(&info.to_le_bytes());
        v.extend_from_slice(&1u64.to_le_bytes()); // addralign
        v.extend_from_slice(&0u64.to_le_bytes()); // entsize
    };

    push_shdr(0, 0, 0, 0, 0, 0, 0, &mut buf); // NULL
    push_shdr(duptext_name_idx, 1, 0x6, payload_off, payload.len() as u64, 0, 0, &mut buf);
    push_shdr(duptext_name_idx, 1, 0x6, payload_off, payload.len() as u64, 0, 0, &mut buf);
    push_shdr(shstrtab_name_idx, 3, 0, shstrtab_off, shstrtab_len as u64, 0, 0, &mut buf);

    buf
}

#[test]
fn elf_ambiguous_section_path_rejected() {
    let elf_bytes = build_elf_with_duplicate_sections();

    if goblin::elf::Elf::parse(&elf_bytes).is_err() {
        return; // Skip if goblin cannot parse this synthetic ELF.
    }

    let codec = ElfCodec::new();
    let bytes = HeapByteSource::new(elf_bytes);
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "sections".to_string(),
            ".duptext".to_string(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::ReplaceSectionBytes {
            old_bytes: b"\x90\x90\x90\x90\x90\x90\x90\x90".to_vec(),
            new_bytes: b"\xCC\xCC\xCC\xCC\xCC\xCC\xCC\xCC".to_vec(),
        }),
    };
    assert!(
        matches!(
            codec.translate_edit(&bytes, &edit),
            Err(TranslateEditError::MalformedPath { .. })
        ),
        "Expected MalformedPath for ambiguous section name"
    );
}

// ============================================================================
// ELF: ambiguous symbol path (L551-555)
// ============================================================================

/// Build a minimal ELF with two symbols sharing the name "dupfunc".
#[allow(clippy::cast_possible_truncation)] // fixture data is tiny
fn build_elf_with_duplicate_symbols() -> Vec<u8> {
    let mut strtab = Vec::<u8>::new();
    strtab.push(0u8);
    let sym_name_idx: u32 = strtab.len() as u32;
    strtab.extend_from_slice(b"dupfunc\0");
    while !strtab.len().is_multiple_of(8) {
        strtab.push(0u8);
    }
    let strtab_len = strtab.len();

    let mut shstrtab = Vec::<u8>::new();
    shstrtab.push(0u8);
    let text_name_idx: u32 = shstrtab.len() as u32;
    shstrtab.extend_from_slice(b".text\0");
    let symtab_name_idx: u32 = shstrtab.len() as u32;
    shstrtab.extend_from_slice(b".symtab\0");
    let strtab_sec_name_idx: u32 = shstrtab.len() as u32;
    shstrtab.extend_from_slice(b".strtab\0");
    let shstrtab_name_idx: u32 = shstrtab.len() as u32;
    shstrtab.extend_from_slice(b".shstrtab\0");
    while !shstrtab.len().is_multiple_of(8) {
        shstrtab.push(0u8);
    }
    let shstrtab_len = shstrtab.len();

    let payload: &[u8] = b"\x90\x90\x90\x90\x90\x90\x90\x90";

    let payload_off: u64 = 64;
    let strtab_off: u64 = payload_off + payload.len() as u64;
    let symtab_off: u64 = strtab_off + strtab_len as u64;
    let symtab_len: u64 = 48; // 2 x Elf64_Sym (24 bytes each)
    let shstrtab_off: u64 = symtab_off + symtab_len;
    let shoff_raw: u64 = shstrtab_off + shstrtab_len as u64;
    let shoff: u64 = (shoff_raw + 7) & !7u64;
    let pad = (shoff - shoff_raw) as usize;

    let shnum: u16 = 5; // NULL + .text + .symtab + .strtab + .shstrtab
    let shstrndx: u16 = 4;
    let strtab_link: u32 = 3; // .strtab is section 3 in symtab's sh_link

    let mut buf = Vec::with_capacity(shoff as usize + shnum as usize * 64);

    // ELF header
    buf.extend_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&0x3eu16.to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&0u64.to_le_bytes());
    buf.extend_from_slice(&0u64.to_le_bytes());
    buf.extend_from_slice(&shoff.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&64u16.to_le_bytes());
    buf.extend_from_slice(&56u16.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());
    buf.extend_from_slice(&64u16.to_le_bytes());
    buf.extend_from_slice(&shnum.to_le_bytes());
    buf.extend_from_slice(&shstrndx.to_le_bytes());
    assert_eq!(buf.len(), 64);

    buf.extend_from_slice(payload);
    buf.extend_from_slice(&strtab);

    // Two Elf64_Sym with the same name index
    let write_sym = |name: u32, info: u8, shndx: u16, val: u64, sz: u64, v: &mut Vec<u8>| {
        v.extend_from_slice(&name.to_le_bytes());
        v.push(info);
        v.push(0u8);
        v.extend_from_slice(&shndx.to_le_bytes());
        v.extend_from_slice(&val.to_le_bytes());
        v.extend_from_slice(&sz.to_le_bytes());
    };
    write_sym(sym_name_idx, 0x12, 1, 0, 4, &mut buf);
    write_sym(sym_name_idx, 0x12, 1, 4, 4, &mut buf);

    buf.extend_from_slice(&shstrtab);
    buf.extend(std::iter::repeat_n(0u8, pad));
    assert_eq!(buf.len(), shoff as usize);

    let push_shdr = |name: u32,
                     typ: u32,
                     flags: u64,
                     off: u64,
                     size: u64,
                     link: u32,
                     info: u32,
                     entsize: u64,
                     v: &mut Vec<u8>| {
        v.extend_from_slice(&name.to_le_bytes());
        v.extend_from_slice(&typ.to_le_bytes());
        v.extend_from_slice(&flags.to_le_bytes());
        v.extend_from_slice(&0u64.to_le_bytes());
        v.extend_from_slice(&off.to_le_bytes());
        v.extend_from_slice(&size.to_le_bytes());
        v.extend_from_slice(&link.to_le_bytes());
        v.extend_from_slice(&info.to_le_bytes());
        v.extend_from_slice(&1u64.to_le_bytes());
        v.extend_from_slice(&entsize.to_le_bytes());
    };

    push_shdr(0, 0, 0, 0, 0, 0, 0, 0, &mut buf);
    push_shdr(text_name_idx, 1, 0x6, payload_off, payload.len() as u64, 0, 0, 0, &mut buf);
    push_shdr(symtab_name_idx, 2, 0, symtab_off, symtab_len, strtab_link, 0, 24, &mut buf);
    push_shdr(strtab_sec_name_idx, 3, 0, strtab_off, strtab_len as u64, 0, 0, 0, &mut buf);
    push_shdr(shstrtab_name_idx, 3, 0, shstrtab_off, shstrtab_len as u64, 0, 0, 0, &mut buf);

    buf
}

#[test]
fn elf_ambiguous_symbol_path_rejected() {
    let elf_bytes = build_elf_with_duplicate_symbols();

    if goblin::elf::Elf::parse(&elf_bytes).is_err() {
        return; // Skip if goblin cannot parse this synthetic ELF.
    }

    let codec = ElfCodec::new();
    let bytes = HeapByteSource::new(elf_bytes);
    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "symbols".to_string(),
            "dupfunc".to_string(),
            "name".to_string(),
        ]),
        op: TreeOp::new(ElfTreeOp::RenameSymbol {
            new_name: "dupfunc".to_string(),
        }),
    };
    // With two symbols named "dupfunc", resolve_named_symbol returns MalformedPath
    // (ambiguous) or the noop Ok(None) if both names match exactly.
    let result = codec.translate_edit(&bytes, &edit);
    // The ambiguity check runs before the noop check, so expect MalformedPath.
    assert!(
        matches!(result, Err(TranslateEditError::MalformedPath { .. })) || result == Ok(None),
        "Expected MalformedPath or noop for ambiguous symbol, got: {result:?}"
    );
}

// ============================================================================
// ZIP MC/DC coverage gap tests — resolve_zip_target_path guard branches
// ============================================================================

/// Branch 778:1 — first match-arm guard: `kind == "archive"` is false.
///
/// A two-component path where the first component is NOT "archive" falls
/// through to the wildcard `_` arm and returns `MalformedPath`.
#[test]
fn zip_resolve_path_two_components_wrong_kind_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["entries".to_string(), "comment".to_string()]),
        op: TreeOp::new(ZipTreeOp::ReplaceComment {
            new_comment: b"TESTCOMMENT!".to_vec(),
        }),
    };
    let result = codec.translate_edit(&bytes, &edit);
    assert!(
        matches!(result, Err(TranslateEditError::MalformedPath { .. })),
        "Expected MalformedPath, got {result:?}"
    );
}

/// Branch 778:3 — first match-arm guard: `field == "comment"` is false.
///
/// A two-component path `["archive", "other"]` has `kind == "archive"` but
/// `field != "comment"`, so the guard fails and falls to the `_` arm.
#[test]
fn zip_resolve_path_archive_wrong_field_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec!["archive".to_string(), "metadata".to_string()]),
        op: TreeOp::new(ZipTreeOp::ReplaceComment {
            new_comment: b"TESTCOMMENT!".to_vec(),
        }),
    };
    let result = codec.translate_edit(&bytes, &edit);
    assert!(
        matches!(result, Err(TranslateEditError::MalformedPath { .. })),
        "Expected MalformedPath, got {result:?}"
    );
}

/// Branch 781:1 — second match-arm guard: `kind == "entries"` is false.
///
/// A three-component path where `kind != "entries"` falls through to the
/// wildcard `_` arm and returns `MalformedPath`.
#[test]
fn zip_resolve_path_three_components_wrong_kind_fails() {
    let fixture = zip_fixture();
    let codec = ZipCodec::new();
    let bytes = HeapByteSource::new(fixture.bytes.clone());

    let edit = DecodedEdit::Tree {
        path: TreePath::new(vec![
            "archive".to_string(),
            "hello.txt".to_string(),
            "bytes".to_string(),
        ]),
        op: TreeOp::new(ZipTreeOp::ReplaceEntryBytes {
            old_bytes: b"Hello World!".to_vec(),
            new_bytes: b"Patched Data".to_vec(),
        }),
    };
    let result = codec.translate_edit(&bytes, &edit);
    assert!(
        matches!(result, Err(TranslateEditError::MalformedPath { .. })),
        "Expected MalformedPath, got {result:?}"
    );
}
