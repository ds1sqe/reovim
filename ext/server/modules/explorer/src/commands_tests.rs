use {super::*, reovim_driver_command::Command};

#[test]
fn toggle_metadata() {
    let cmd = Toggle;
    assert_eq!(cmd.id(), ids::TOGGLE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn close_metadata() {
    let cmd = Close;
    assert_eq!(cmd.id(), ids::CLOSE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn cursor_up_metadata() {
    let cmd = CursorUp;
    assert_eq!(cmd.id(), ids::CURSOR_UP);
    assert!(!cmd.description().is_empty());
}

#[test]
fn cursor_down_metadata() {
    let cmd = CursorDown;
    assert_eq!(cmd.id(), ids::CURSOR_DOWN);
    assert!(!cmd.description().is_empty());
}

#[test]
fn goto_first_metadata() {
    let cmd = GotoFirst;
    assert_eq!(cmd.id(), ids::GOTO_FIRST);
    assert!(!cmd.description().is_empty());
}

#[test]
fn goto_last_metadata() {
    let cmd = GotoLast;
    assert_eq!(cmd.id(), ids::GOTO_LAST);
    assert!(!cmd.description().is_empty());
}

#[test]
fn expand_metadata() {
    let cmd = Expand;
    assert_eq!(cmd.id(), ids::EXPAND);
    assert!(!cmd.description().is_empty());
}

#[test]
fn collapse_metadata() {
    let cmd = Collapse;
    assert_eq!(cmd.id(), ids::COLLAPSE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn open_metadata() {
    let cmd = Open;
    assert_eq!(cmd.id(), ids::OPEN);
    assert!(!cmd.description().is_empty());
}

#[test]
fn goto_parent_metadata() {
    let cmd = GotoParent;
    assert_eq!(cmd.id(), ids::GOTO_PARENT);
    assert!(!cmd.description().is_empty());
}

#[test]
fn toggle_hidden_metadata() {
    let cmd = ToggleHidden;
    assert_eq!(cmd.id(), ids::TOGGLE_HIDDEN);
    assert!(!cmd.description().is_empty());
}

#[test]
fn refresh_metadata() {
    let cmd = Refresh;
    assert_eq!(cmd.id(), ids::REFRESH);
    assert!(!cmd.description().is_empty());
}

#[test]
fn create_file_metadata() {
    let cmd = CreateFile;
    assert_eq!(cmd.id(), ids::CREATE_FILE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn create_dir_metadata() {
    let cmd = CreateDir;
    assert_eq!(cmd.id(), ids::CREATE_DIR);
    assert!(!cmd.description().is_empty());
}

#[test]
fn rename_metadata() {
    let cmd = Rename;
    assert_eq!(cmd.id(), ids::RENAME);
    assert!(!cmd.description().is_empty());
}

#[test]
fn delete_metadata() {
    let cmd = Delete;
    assert_eq!(cmd.id(), ids::DELETE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn confirm_input_metadata() {
    let cmd = ConfirmInput;
    assert_eq!(cmd.id(), ids::CONFIRM_INPUT);
    assert!(!cmd.description().is_empty());
}

#[test]
fn cancel_input_metadata() {
    let cmd = CancelInput;
    assert_eq!(cmd.id(), ids::CANCEL_INPUT);
    assert!(!cmd.description().is_empty());
}

#[test]
fn input_backspace_metadata() {
    let cmd = InputBackspace;
    assert_eq!(cmd.id(), ids::INPUT_BACKSPACE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn yank_path_metadata() {
    let cmd = YankPath;
    assert_eq!(cmd.id(), ids::YANK_PATH);
    assert!(!cmd.description().is_empty());
}

#[test]
fn cut_mark_metadata() {
    let cmd = CutMark;
    assert_eq!(cmd.id(), ids::CUT_MARK);
    assert!(!cmd.description().is_empty());
}

#[test]
fn paste_metadata() {
    let cmd = Paste;
    assert_eq!(cmd.id(), ids::PASTE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn command_handlers_count() {
    let handlers = command_handlers();
    assert_eq!(handlers.len(), 22);
}

#[test]
fn command_handlers_unique_ids() {
    let handlers = command_handlers();
    let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
    let mut deduped = ids.clone();
    deduped.sort_by_key(CommandId::name_owned);
    deduped.dedup_by_key(|id| id.name_owned());
    assert_eq!(ids.len(), deduped.len());
}

#[test]
fn toggle_debug() {
    let debug = format!("{Toggle:?}");
    assert!(debug.contains("Toggle"));
}

#[test]
fn close_debug() {
    let debug = format!("{Close:?}");
    assert!(debug.contains("Close"));
}

#[test]
fn cursor_up_debug() {
    let debug = format!("{CursorUp:?}");
    assert!(debug.contains("CursorUp"));
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn all_default_constructable() {
    let _ = Toggle::default();
    let _ = Close::default();
    let _ = CursorUp::default();
    let _ = CursorDown::default();
    let _ = GotoFirst::default();
    let _ = GotoLast::default();
    let _ = Expand::default();
    let _ = Collapse::default();
    let _ = Open::default();
    let _ = GotoParent::default();
    let _ = ToggleHidden::default();
    let _ = Refresh::default();
    let _ = CreateFile::default();
    let _ = CreateDir::default();
    let _ = Rename::default();
    let _ = Delete::default();
    let _ = ConfirmInput::default();
    let _ = CancelInput::default();
    let _ = InputBackspace::default();
    let _ = YankPath::default();
    let _ = CutMark::default();
    let _ = Paste::default();
}

#[test]
fn cursor_dir_path_on_dir() {
    use {crate::tree::node::FileNode, reovim_subsys_vfs::MockVfs, std::path::Path};

    let vfs = MockVfs::new();
    vfs.add_dir("/root");
    vfs.add_dir("/root/src");
    let mut root = FileNode::from_path(Path::new("/root"), &vfs, 0).unwrap();
    root.set_expanded(true);
    root.load_children(&vfs).unwrap();

    let nodes: Vec<&FileNode> = vec![&root];
    let result = cursor_dir_path(&nodes, 0);
    assert_eq!(result, PathBuf::from("/root"));
}

#[test]
fn cursor_dir_path_on_file() {
    use {
        crate::tree::node::{FileNode, NodeType},
        std::path::PathBuf,
    };

    let file_node = FileNode {
        name: "main.rs".to_string(),
        path: PathBuf::from("/root/main.rs"),
        node_type: NodeType::File { size: 100 },
        depth: 1,
        is_hidden: false,
        is_gitignored: false,
    };

    let nodes: Vec<&FileNode> = vec![&file_node];
    let result = cursor_dir_path(&nodes, 0);
    assert_eq!(result, PathBuf::from("/root"));
}

#[test]
fn cursor_dir_path_out_of_bounds() {
    let nodes: Vec<&crate::tree::node::FileNode> = vec![];
    let result = cursor_dir_path(&nodes, 5);
    assert_eq!(result, PathBuf::from("."));
}

// ============================================================================
// Open command — file-open codec-pipeline path (Phase 2 of #737)
// ============================================================================
//
// Each test wires a single test file into a `MockVfs`, builds an
// `ExplorerState` with a `FileTree` rooted at `/root`, then invokes the
// `Open` command on a `TestSessionRuntime` and asserts on the observable
// outcome (active buffer content, `state.message`, explorer-active flag).
// Codec dispatch is exercised through stub classifiers / codecs registered
// on the kernel `ServiceRegistry`; the four tests cover the four arms the
// rewritten `Open::execute` distinguishes.

mod open_codec {
    use {
        crate::{commands::Open, state::ExplorerState, tree::FileTree},
        reovim_driver_command::CommandHandler,
        reovim_driver_text_session::{
            BufferApi, ExtensionApi, SessionExtension, testing::TestSessionRuntime,
        },
        reovim_subsys_command_types::CommandContext,
        reovim_subsys_content_codec::{
            Annotation, CodecError, CodecMetadata, ContentClassifier, ContentClassifierStore,
            ContentCodec, ContentCodecFactory, ContentCodecFactoryStore, ContentType, DecodeResult,
            MAX_FILE_SIZE,
        },
        reovim_subsys_vfs::MockVfs,
        std::{
            path::PathBuf,
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

    /// Wire `path` → `bytes` into a `MockVfs` and return the populated VFS
    /// alongside an `ExplorerState` whose tree's only entries are the root
    /// directory and `path`. After `flatten`, index 0 is the root directory
    /// and index 1 is the file, so the cursor is set to 1 to select it.
    fn explorer_state_for_file(path: &str, bytes: &[u8]) -> (Arc<MockVfs>, ExplorerState) {
        let vfs = Arc::new(MockVfs::new());
        vfs.add_dir("/root");
        vfs.add_file(path, bytes);
        let tree = FileTree::new(PathBuf::from("/root"), vfs.as_ref()).unwrap();
        let mut state = <ExplorerState as SessionExtension>::create();
        state.tree = Some(tree);
        state.cursor_index = 1;
        state.active = true;
        state.show_hidden = true;
        (vfs, state)
    }

    fn cmd_context(vfs: &Arc<MockVfs>) -> CommandContext {
        let mut ctx = CommandContext::new();
        ctx.set_vfs(Arc::clone(vfs) as Arc<dyn reovim_subsys_vfs::VfsDriver>);
        ctx
    }

    #[test]
    fn open_codec_decoded_buffer_uses_codec_text() {
        let mut harness = TestSessionRuntime::with_buffer("initial");
        let (vfs, state) = explorer_state_for_file("/root/file.bin", b"\x7fELFrest");
        let ctx = cmd_context(&vfs);

        harness.with_runtime(|runtime| {
            *runtime.ext_mut::<ExplorerState>() = state;

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

            assert_eq!(
                Open.execute(runtime, &ctx),
                reovim_subsys_command_types::CommandResult::Success
            );

            let active_id = runtime.active_buffer().expect("buffer must be set");
            let buffer_text = runtime
                .buffer_content(active_id)
                .expect("active buffer must be readable");
            assert_eq!(buffer_text, "[ELF summary]");
            assert!(!runtime.ext::<ExplorerState>().unwrap().active);
            assert!(runtime.ext::<ExplorerState>().unwrap().message.is_none());
        });
    }

    #[test]
    fn open_utf8_fallback_buffer_uses_raw_text() {
        let mut harness = TestSessionRuntime::with_buffer("initial");
        let (vfs, state) = explorer_state_for_file("/root/file.txt", b"hello world");
        let ctx = cmd_context(&vfs);

        harness.with_runtime(|runtime| {
            *runtime.ext_mut::<ExplorerState>() = state;
            runtime
                .kernel()
                .services
                .register(Arc::new(ContentClassifierStore::new()));
            runtime
                .kernel()
                .services
                .register(Arc::new(ContentCodecFactoryStore::new()));

            assert_eq!(
                Open.execute(runtime, &ctx),
                reovim_subsys_command_types::CommandResult::Success
            );

            let active_id = runtime.active_buffer().expect("buffer must be set");
            assert_eq!(runtime.buffer_content(active_id).unwrap(), "hello world");
            assert!(!runtime.ext::<ExplorerState>().unwrap().active);
            assert!(runtime.ext::<ExplorerState>().unwrap().message.is_none());
        });
    }

    #[test]
    fn open_too_large_sets_explorer_message_no_buffer() {
        let mut harness = TestSessionRuntime::with_buffer("initial");
        let initial_buffer_id = harness.active_buffer().expect("initial buffer");
        let big = vec![b'a'; MAX_FILE_SIZE + 1];
        let (vfs, state) = explorer_state_for_file("/root/big.txt", &big);
        let ctx = cmd_context(&vfs);

        harness.with_runtime(|runtime| {
            *runtime.ext_mut::<ExplorerState>() = state;
            runtime
                .kernel()
                .services
                .register(Arc::new(ContentClassifierStore::new()));
            runtime
                .kernel()
                .services
                .register(Arc::new(ContentCodecFactoryStore::new()));

            assert_eq!(
                Open.execute(runtime, &ctx),
                reovim_subsys_command_types::CommandResult::Success
            );

            assert_eq!(runtime.active_buffer(), Some(initial_buffer_id));
            assert!(runtime.ext::<ExplorerState>().unwrap().active);
            let msg = runtime
                .ext::<ExplorerState>()
                .unwrap()
                .message
                .clone()
                .unwrap();
            assert!(msg.starts_with("File too large"), "message was {msg:?}");
            assert!(msg.contains("use :e"));
        });
    }

    #[test]
    fn open_not_utf8_sets_explorer_message_no_buffer() {
        let mut harness = TestSessionRuntime::with_buffer("initial");
        let initial_buffer_id = harness.active_buffer().expect("initial buffer");
        let (vfs, state) = explorer_state_for_file("/root/garbage.bin", b"abc\xFFdef");
        let ctx = cmd_context(&vfs);

        harness.with_runtime(|runtime| {
            *runtime.ext_mut::<ExplorerState>() = state;
            runtime
                .kernel()
                .services
                .register(Arc::new(ContentClassifierStore::new()));
            runtime
                .kernel()
                .services
                .register(Arc::new(ContentCodecFactoryStore::new()));

            assert_eq!(
                Open.execute(runtime, &ctx),
                reovim_subsys_command_types::CommandResult::Success
            );

            assert_eq!(runtime.active_buffer(), Some(initial_buffer_id));
            assert!(runtime.ext::<ExplorerState>().unwrap().active);
            let msg = runtime
                .ext::<ExplorerState>()
                .unwrap()
                .message
                .clone()
                .unwrap();
            assert!(msg.contains("Cannot decode"), "message was {msg:?}");
            assert!(msg.contains("not valid UTF-8"));
            assert!(msg.contains("offset 3"));
        });
    }
}
