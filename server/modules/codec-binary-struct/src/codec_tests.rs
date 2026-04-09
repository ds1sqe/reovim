//! Tests for ELF and ZIP codecs.

use {
    reovim_driver_codec::{ContentCodec, DecodedEdit},
    reovim_driver_vfs::HeapByteSource,
    reovim_types_text::Position,
};

use super::*;

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
    use reovim_driver_codec::TranslateEditError;
    let codec = ElfCodec::new();
    let bytes = HeapByteSource::new(b"\x7fELF");
    let edit = DecodedEdit::Text {
        start: Position::new(0, 0),
        end: Position::new(0, 0),
        replacement: "x".to_string(),
    };

    assert!(matches!(codec.translate_edit(&bytes, &edit), Err(TranslateEditError::ReadOnly)));
}

#[test]
fn elf_translate_edit_bytes_is_not_supported() {
    use reovim_driver_codec::TranslateEditError;
    let codec = ElfCodec::new();
    let bytes = HeapByteSource::new(vec![0x7f, b'E', b'L', b'F']);
    let edit = DecodedEdit::Bytes {
        offset: 0,
        old_len: 1,
        new_bytes: b"x".to_vec(),
    };

    assert!(matches!(codec.translate_edit(&bytes, &edit), Err(TranslateEditError::ReadOnly)));
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

    assert!(matches!(codec.translate_edit(&bytes, &edit), Err(TranslateEditError::ReadOnly)));
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

    assert!(matches!(codec.translate_edit(&bytes, &edit), Err(TranslateEditError::ReadOnly)));
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
