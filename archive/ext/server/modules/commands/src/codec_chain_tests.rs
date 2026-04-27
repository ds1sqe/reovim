//! Phase 8 of #737: real-codec integration tests for the codec hot-attach chain.
//!
//! Phases 1–7 each shipped module-level unit tests with stub factories that
//! prove the wiring; this file is the one place we exercise the chain with
//! the production [`BinaryStructCodecFactory`] / [`HexCodecFactory`] to
//! prove the orchestration actually classifies and decodes binary content
//! end to end. The four tests cover one decision point each:
//!
//! 1. `e_command_decodes_elf_through_real_codec` — `:e` reads VFS bytes
//!    through `decode_file_bytes` and emits the real ELF summary.
//! 2. `mount_hex_then_umount_round_trips` — `:mount hex` and `:umount`
//!    round-trip cleanly with a real factory.
//! 3. `decode_file_bytes_with_elf_classifier_returns_elf_content_type` —
//!    the helper itself routes through the real classifier+factory.
//! 4. `e_command_falls_back_to_utf8_for_unknown_bytes` — when no codec
//!    applies, `:e` returns literal UTF-8.

use {
    super::{edit::EditCommand, mount::MountCommand, umount::UmountCommand},
    reovim_content_codec::{ContentClassifierStore, ContentCodecFactoryStore, ContentType},
    reovim_content_codec_binary_struct::{BinaryStructCodecFactory, ElfClassifier},
    reovim_content_codec_hex::HexCodecFactory,
    reovim_content_codec_text::CodecSessionState,
    reovim_driver_command::{ArgValue, CommandContext, CommandHandler, CommandResult},
    reovim_driver_text_session::testing::TestSessionRuntime,
    reovim_subsys_content_codec::decode_file_bytes,
    reovim_subsys_vfs::MockVfs,
    std::sync::Arc,
};

/// Minimal valid ELF64 header (64 bytes). Goblin requires a complete
/// header to parse; the classifier matches on the first 4 bytes alone,
/// but the codec calls `goblin::elf::Elf::parse` which reads the full
/// 64-byte header. The fields below describe an `ET_REL` x86-64 little-
/// endian object with no program headers and no section headers — the
/// smallest shape goblin accepts as a valid ELF.
#[rustfmt::skip]
const ELF_BYTES: &[u8] = &[
    // e_ident: magic + class(64) + data(LE) + version(1) + osabi/pad
    0x7f, b'E', b'L', b'F', 0x02, 0x01, 0x01, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00,             // e_type    = ET_REL
    0x3e, 0x00,             // e_machine = EM_X86_64
    0x01, 0x00, 0x00, 0x00, // e_version = 1
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // e_entry = 0
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // e_phoff = 0
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // e_shoff = 0
    0x00, 0x00, 0x00, 0x00, // e_flags = 0
    0x40, 0x00,             // e_ehsize = 64
    0x00, 0x00,             // e_phentsize = 0
    0x00, 0x00,             // e_phnum = 0
    0x00, 0x00,             // e_shentsize = 0
    0x00, 0x00,             // e_shnum = 0
    0x00, 0x00,             // e_shstrndx = 0
];

#[test]
fn e_command_decodes_elf_through_real_codec() {
    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    let buffer_id = harness.active_buffer().expect("active buffer");

    let classifier_store = Arc::new(ContentClassifierStore::new());
    classifier_store.add(Arc::new(ElfClassifier::new()));
    harness.kernel().services.register(classifier_store);

    let factory_store = Arc::new(ContentCodecFactoryStore::new());
    factory_store.add_factory(Arc::new(BinaryStructCodecFactory::new()));
    harness.kernel().services.register(factory_store);

    let mock_vfs = Arc::new(MockVfs::new());
    mock_vfs.add_file("/tmp/test.elf", ELF_BYTES);

    harness.with_runtime(|runtime| {
        let mut ctx = CommandContext::new();
        ctx.set("file", ArgValue::String("/tmp/test.elf".to_string()));
        ctx.set_buffer_id(buffer_id);
        ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_subsys_vfs::VfsDriver>);
        let result = EditCommand.execute(runtime, &ctx);
        assert_eq!(result, CommandResult::Success);

        let content = runtime
            .text_buffer(buffer_id)
            .expect("buffer present")
            .read()
            .content();
        assert!(
            content.starts_with("ELF Binary Summary\n"),
            "expected ELF summary header, got: {content:?}"
        );
    });
}

#[test]
fn mount_hex_then_umount_round_trips() {
    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buf_id = harness.active_buffer().expect("active buffer");

    let factory_store = Arc::new(ContentCodecFactoryStore::new());
    factory_store.add_factory(Arc::new(HexCodecFactory));
    harness
        .kernel()
        .services
        .register(Arc::clone(&factory_store));

    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();
    harness
        .shared_extensions
        .get_mut::<CodecSessionState>()
        .unwrap()
        .set_source(buf_id, b"\x00\x01\x02raw bytes".to_vec());

    let mut mount_ctx = CommandContext::new();
    mount_ctx.set("content_type", ArgValue::String("hex".to_string()));
    harness.with_runtime(|runtime| {
        let result = MountCommand.execute(runtime, &mount_ctx);
        assert_eq!(result, CommandResult::Success);
    });
    assert_eq!(
        harness
            .shared_extensions
            .get::<CodecSessionState>()
            .unwrap()
            .list_mounts(buf_id)
            .len(),
        1,
        "expected one mount after :mount hex"
    );

    let umount_ctx = CommandContext::new();
    harness.with_runtime(|runtime| {
        let result = UmountCommand.execute(runtime, &umount_ctx);
        assert_eq!(result, CommandResult::Success);
    });
    assert!(
        harness
            .shared_extensions
            .get::<CodecSessionState>()
            .unwrap()
            .list_mounts(buf_id)
            .is_empty(),
        "expected no mounts after :umount"
    );
}

#[test]
fn decode_file_bytes_with_elf_classifier_returns_elf_content_type() {
    let classifier_store = ContentClassifierStore::new();
    classifier_store.add(Arc::new(ElfClassifier::new()));

    let factory_store = ContentCodecFactoryStore::new();
    factory_store.add_factory(Arc::new(BinaryStructCodecFactory::new()));

    let (text, content_type) =
        decode_file_bytes(ELF_BYTES, "test.elf", &classifier_store, &factory_store)
            .expect("decode_file_bytes must succeed for ELF magic + classifier + factory");

    let ct = content_type.expect("classifier match should yield Some(content_type)");
    assert_eq!(ct, ContentType::new("binary/elf"));
    assert!(
        text.starts_with("ELF Binary Summary\n"),
        "expected ELF summary header, got: {text:?}"
    );
}

#[test]
fn e_command_falls_back_to_utf8_for_unknown_bytes() {
    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    let buffer_id = harness.active_buffer().expect("active buffer");

    let mock_vfs = Arc::new(MockVfs::new());
    mock_vfs.add_file_str("/tmp/test.txt", "hello world");

    harness.with_runtime(|runtime| {
        let mut ctx = CommandContext::new();
        ctx.set("file", ArgValue::String("/tmp/test.txt".to_string()));
        ctx.set_buffer_id(buffer_id);
        ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_subsys_vfs::VfsDriver>);
        let result = EditCommand.execute(runtime, &ctx);
        assert_eq!(result, CommandResult::Success);

        let content = runtime
            .text_buffer(buffer_id)
            .expect("buffer present")
            .read()
            .content();
        assert_eq!(content, "hello world");
    });
}
