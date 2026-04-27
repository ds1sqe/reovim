use std::fs;

use super::*;

fn services() -> reovim_kernel::api::v1::ServiceRegistry {
    reovim_kernel::api::v1::ServiceRegistry::new()
}

fn temp_dir_with_files(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent dir");
        }
        fs::write(&path, content).expect("Failed to write file");
    }
    dir
}

fn ctx_for(dir: &std::path::Path) -> PickerContext {
    PickerContext {
        cwd: dir.to_path_buf(),
        query: String::new(),
        buffers: vec![],
        commands: vec![],
        options: vec![],
    }
}

// -- Picker metadata tests --

#[test]
fn name_and_title() {
    let picker = FilesPicker::new();
    assert_eq!(picker.name(), "files");
    assert_eq!(picker.title(), "Files");
}

#[test]
fn is_static() {
    let picker = FilesPicker::new();
    assert!(picker.is_static());
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn default_impl() {
    let picker = FilesPicker::default();
    assert_eq!(picker.name(), "files");
}

// -- Items tests --

#[test]
fn items_from_temp_dir() {
    let dir = temp_dir_with_files(&[
        ("main.rs", "fn main() {}"),
        ("lib.rs", "pub mod foo;"),
        ("src/utils.rs", "pub fn helper() {}"),
    ]);
    let picker = FilesPicker::new();
    let ctx = ctx_for(dir.path());
    let items = picker.items(&ctx, &services());

    assert_eq!(items.len(), 3);
    let displays: Vec<&str> = items.iter().map(|i| i.display.as_str()).collect();
    assert!(displays.contains(&"main.rs"));
    assert!(displays.contains(&"lib.rs"));
    assert!(displays.contains(&"src/utils.rs"));
}

#[test]
fn items_returns_relative_paths() {
    let dir = temp_dir_with_files(&[("a/b/c.txt", "hello")]);
    let picker = FilesPicker::new();
    let ctx = ctx_for(dir.path());
    let items = picker.items(&ctx, &services());

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].display, "a/b/c.txt");
}

#[test]
fn gitignore_respected() {
    let dir = temp_dir_with_files(&[
        (".gitignore", "*.log\ntarget/\n"),
        ("main.rs", "fn main() {}"),
        ("debug.log", "log data"),
        ("target/output.bin", "binary"),
    ]);
    // Initialize git repo so .gitignore is respected.
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to run git init");

    let picker = FilesPicker::new();
    let ctx = ctx_for(dir.path());
    let items = picker.items(&ctx, &services());

    let displays: Vec<&str> = items.iter().map(|i| i.display.as_str()).collect();
    assert!(displays.contains(&"main.rs"));
    assert!(!displays.contains(&"debug.log"), "*.log should be gitignored");
    assert!(!displays.iter().any(|d| d.contains("target")), "target/ should be gitignored");
}

// -- on_select tests --

#[test]
fn on_select_file_path() {
    let picker = FilesPicker::new();
    let item = PickerItem {
        display: "main.rs".to_owned(),
        detail: None,
        data: PickerData::FilePath(PathBuf::from("/tmp/main.rs")),
        icon: None,
    };
    let action = picker.on_select(&item);
    assert!(matches!(action, PickerAction::OpenFile(ref p) if p == &PathBuf::from("/tmp/main.rs")));
}

#[test]
fn on_select_wrong_data_closes() {
    let picker = FilesPicker::new();
    let item = PickerItem {
        display: "test".to_owned(),
        detail: None,
        data: PickerData::Text("wrong".to_owned()),
        icon: None,
    };
    let action = picker.on_select(&item);
    assert!(matches!(action, PickerAction::Close));
}

// -- Preview tests --

#[test]
fn preview_text_file() {
    let dir = temp_dir_with_files(&[("hello.rs", "fn main() {\n    println!(\"hello\");\n}")]);
    let picker = FilesPicker::new();
    let path = dir.path().join("hello.rs");
    let item = PickerItem {
        display: "hello.rs".to_owned(),
        detail: None,
        data: PickerData::FilePath(path.clone()),
        icon: None,
    };
    let preview = picker.preview(&item, &services());
    assert!(preview.is_some());
    let preview = preview.unwrap();
    assert_eq!(preview.lines.len(), 3);
    assert_eq!(preview.lines[0], "fn main() {");
    assert!(preview.highlight_line.is_none());
    assert_eq!(preview.file_path, Some(path));
}

#[test]
fn preview_nonexistent_file() {
    let picker = FilesPicker::new();
    let item = PickerItem {
        display: "nope.rs".to_owned(),
        detail: None,
        data: PickerData::FilePath(PathBuf::from("/tmp/nonexistent_file_12345.rs")),
        icon: None,
    };
    assert!(picker.preview(&item, &services()).is_none());
}

#[test]
fn preview_binary_file() {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let path = dir.path().join("binary.bin");
    fs::write(&path, [0u8, 1, 2, 255, 0, 3]).expect("Failed to write binary");

    let picker = FilesPicker::new();
    let item = PickerItem {
        display: "binary.bin".to_owned(),
        detail: None,
        data: PickerData::FilePath(path),
        icon: None,
    };
    assert!(picker.preview(&item, &services()).is_none());
}

#[test]
fn preview_wrong_data_type() {
    let picker = FilesPicker::new();
    let item = PickerItem {
        display: "x".to_owned(),
        detail: None,
        data: PickerData::Text("y".to_owned()),
        icon: None,
    };
    assert!(picker.preview(&item, &services()).is_none());
}

#[test]
fn preview_empty_file() {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let path = dir.path().join("empty.txt");
    fs::write(&path, "").expect("Failed to write empty file");

    let picker = FilesPicker::new();
    let item = PickerItem {
        display: "empty.txt".to_owned(),
        detail: None,
        data: PickerData::FilePath(path),
        icon: None,
    };
    assert!(picker.preview(&item, &services()).is_none());
}

// -- Module tests --

#[test]
fn module_id() {
    let module = PickerFilesModule::new();
    assert_eq!(module.id().as_str(), "picker-files");
}

#[test]
fn module_name() {
    let module = PickerFilesModule::new();
    assert_eq!(module.name(), "File Picker");
}

#[test]
fn module_version() {
    let module = PickerFilesModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn module_default() {
    let module = PickerFilesModule::default();
    assert_eq!(module.id().as_str(), "picker-files");
}

#[test]
fn module_exit() {
    let mut module = PickerFilesModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn module_init_registers_picker() {
    let services = Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
    let ctx = ModuleContext::new(
        reovim_kernel::api::v1::KernelContext::default(),
        services.clone(),
        PathBuf::from("/tmp"),
        PathBuf::from("/tmp"),
    );

    let mut module = PickerFilesModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    let registry = services.get::<PickerRegistry>();
    assert!(registry.is_some());
    let reg = registry.unwrap();
    assert!(reg.get("files").is_some());
}

// ============================================================================
// open_file — codec-pipeline path (Phase 3 of #737)
// ============================================================================
//
// Each test wires a single file into a `MockVfs`, registers a
// `VfsInstance`, optionally registers `ContentClassifierStore` and
// `ContentCodecFactoryStore` with stub classifiers / codecs, then
// invokes `open_file` on a `TestSessionRuntime` and asserts on
// observable runtime state. Five tests cover the four return shapes of
// `decode_file_bytes` plus the buffer-reuse short-circuit invariant.

mod open_file_codec {
    use {
        super::super::open_file,
        reovim_driver_text_session::{BufferApi, testing::TestSessionRuntime},
        reovim_subsys_content_codec::{
            Annotation, CodecError, CodecMetadata, ContentClassifier, ContentClassifierStore,
            ContentCodec, ContentCodecFactory, ContentCodecFactoryStore, ContentType, DecodeResult,
            MAX_FILE_SIZE,
        },
        reovim_subsys_vfs::{MockVfs, VfsDriver, VfsInstance},
        std::{
            path::Path,
            sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            },
        },
    };

    const STUB_TYPE: &str = "test/stub";

    struct PrefixClassifier {
        prefix: Vec<u8>,
        content_type: ContentType,
    }
    impl ContentClassifier for PrefixClassifier {
        fn classify(&self, raw: &[u8], _path: &str) -> Option<ContentType> {
            if raw.starts_with(&self.prefix) {
                Some(self.content_type.clone())
            } else {
                None
            }
        }
        fn name(&self) -> &'static str {
            "test-prefix-classifier"
        }
    }

    struct PanickingClassifier;
    impl ContentClassifier for PanickingClassifier {
        fn classify(&self, _raw: &[u8], _path: &str) -> Option<ContentType> {
            panic!("classifier must not run when buffer is reused");
        }
        fn name(&self) -> &'static str {
            "test-panicking-classifier"
        }
    }

    struct EchoCodec {
        output: &'static str,
        fail_count: AtomicUsize,
    }
    impl ContentCodec for EchoCodec {
        fn decode(&self, _raw: &[u8]) -> Result<DecodeResult, CodecError> {
            if self.fail_count.fetch_sub(1, Ordering::SeqCst) > 0 {
                return Err(CodecError::InvalidSequence {
                    offset: 0,
                    detail: "test-injected".into(),
                });
            }
            Ok(DecodeResult {
                content: self.output.to_string(),
                annotations: Vec::<Annotation>::new(),
                metadata: CodecMetadata::new(ContentType::new(STUB_TYPE)),
                lossy: false,
                readonly: false,
                truncated: false,
            })
        }
    }

    struct EchoFactory {
        codec: Arc<EchoCodec>,
        matches_type: ContentType,
    }
    impl ContentCodecFactory for EchoFactory {
        fn create(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
            if content_type == &self.matches_type {
                Some(self.codec.clone() as Arc<dyn ContentCodec>)
            } else {
                None
            }
        }
        fn supported_content_types(&self) -> Vec<&str> {
            vec![STUB_TYPE]
        }
        fn name(&self) -> &'static str {
            "test-echo-factory"
        }
    }

    fn vfs_with_file(path: &str, bytes: &[u8]) -> Arc<MockVfs> {
        let vfs = Arc::new(MockVfs::new());
        vfs.add_file(path, bytes);
        vfs
    }

    fn register_vfs(runtime: &reovim_driver_text_session::SessionRuntime<'_>, vfs: Arc<MockVfs>) {
        let instance = Arc::new(VfsInstance::new(vfs as Arc<dyn VfsDriver>));
        runtime.kernel().services.register(instance);
    }

    #[test]
    fn picker_open_codec_decoded_buffer_uses_codec_text() {
        let mut harness = TestSessionRuntime::with_buffer("initial");
        let vfs = vfs_with_file("/root/file.bin", b"\x7fELFrest");
        harness.with_runtime(|runtime| {
            register_vfs(runtime, vfs);

            let stub_type = ContentType::new(STUB_TYPE);
            let classifier_store = Arc::new(ContentClassifierStore::new());
            classifier_store.add(Arc::new(PrefixClassifier {
                prefix: b"\x7fELF".to_vec(),
                content_type: stub_type.clone(),
            }));
            let factory_store = Arc::new(ContentCodecFactoryStore::new());
            factory_store.add_factory(Arc::new(EchoFactory {
                codec: Arc::new(EchoCodec {
                    output: "[ELF summary]",
                    fail_count: AtomicUsize::new(0),
                }),
                matches_type: stub_type,
            }));
            runtime.kernel().services.register(classifier_store);
            runtime.kernel().services.register(factory_store);

            open_file(runtime, Path::new("/root/file.bin"));

            let active_id = runtime.active_buffer().expect("buffer must be set");
            assert_eq!(runtime.buffer_content(active_id).unwrap(), "[ELF summary]");
        });
    }

    #[test]
    fn picker_open_utf8_fallback_buffer_uses_raw_text() {
        let mut harness = TestSessionRuntime::with_buffer("initial");
        let vfs = vfs_with_file("/root/file.txt", b"hello world");
        harness.with_runtime(|runtime| {
            register_vfs(runtime, vfs);
            runtime
                .kernel()
                .services
                .register(Arc::new(ContentClassifierStore::new()));
            runtime
                .kernel()
                .services
                .register(Arc::new(ContentCodecFactoryStore::new()));

            open_file(runtime, Path::new("/root/file.txt"));

            let active_id = runtime.active_buffer().expect("buffer must be set");
            assert_eq!(runtime.buffer_content(active_id).unwrap(), "hello world");
        });
    }

    #[test]
    fn picker_open_too_large_skips_buffer_create() {
        let mut harness = TestSessionRuntime::with_buffer("initial");
        let initial_id = harness.active_buffer().expect("initial buffer");
        let big = vec![b'a'; MAX_FILE_SIZE + 1];
        let vfs = vfs_with_file("/root/big.txt", &big);
        harness.with_runtime(|runtime| {
            let initial_count = runtime.kernel().buffers.list().len();
            register_vfs(runtime, vfs);
            runtime
                .kernel()
                .services
                .register(Arc::new(ContentClassifierStore::new()));
            runtime
                .kernel()
                .services
                .register(Arc::new(ContentCodecFactoryStore::new()));

            open_file(runtime, Path::new("/root/big.txt"));

            assert_eq!(
                runtime.kernel().buffers.list().len(),
                initial_count,
                "no new buffer should be created on TooLarge"
            );
            assert_eq!(runtime.active_buffer(), Some(initial_id));
        });
    }

    #[test]
    fn picker_open_not_utf8_skips_buffer_create() {
        let mut harness = TestSessionRuntime::with_buffer("initial");
        let initial_id = harness.active_buffer().expect("initial buffer");
        let vfs = vfs_with_file("/root/garbage.bin", b"abc\xFFdef");
        harness.with_runtime(|runtime| {
            let initial_count = runtime.kernel().buffers.list().len();
            register_vfs(runtime, vfs);
            runtime
                .kernel()
                .services
                .register(Arc::new(ContentClassifierStore::new()));
            runtime
                .kernel()
                .services
                .register(Arc::new(ContentCodecFactoryStore::new()));

            open_file(runtime, Path::new("/root/garbage.bin"));

            assert_eq!(
                runtime.kernel().buffers.list().len(),
                initial_count,
                "no new buffer should be created on NotUtf8"
            );
            assert_eq!(runtime.active_buffer(), Some(initial_id));
        });
    }

    #[test]
    fn picker_open_existing_buffer_reused_no_codec_call() {
        let mut harness = TestSessionRuntime::with_buffer("initial");
        let vfs = vfs_with_file("/root/reused.txt", b"will not be read");
        harness.with_runtime(|runtime| {
            register_vfs(runtime, vfs);
            // Pre-create a buffer matching the canonical path so the
            // reuse short-circuit triggers and the panicking classifier
            // (registered below) is never reached.
            let canonical = Path::new("/root/reused.txt")
                .canonicalize()
                .unwrap_or_else(|_| Path::new("/root/reused.txt").to_path_buf());
            let existing_id =
                runtime.create_buffer(Some(&canonical.to_string_lossy()), "pre-existing content");

            let classifier_store = Arc::new(ContentClassifierStore::new());
            classifier_store.add(Arc::new(PanickingClassifier));
            runtime.kernel().services.register(classifier_store);
            runtime
                .kernel()
                .services
                .register(Arc::new(ContentCodecFactoryStore::new()));

            open_file(runtime, Path::new("/root/reused.txt"));

            assert_eq!(runtime.active_buffer(), Some(existing_id));
            assert_eq!(runtime.buffer_content(existing_id).unwrap(), "pre-existing content");
        });
    }
}
