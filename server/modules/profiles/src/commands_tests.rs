use {
    super::*,
    reovim_driver_command_types::ArgValue,
    reovim_driver_session::testing::TestSessionRuntime,
    reovim_driver_vfs::{MockVfs, VfsDriver},
    reovim_kernel::api::v1::{OptionSpec, OptionValue},
    std::sync::Arc,
};

/// Register common test options on a `TestSessionRuntime`'s kernel.
fn register_test_options(test: &TestSessionRuntime) {
    let opts = &test.kernel().options;
    opts.register(OptionSpec::new(
        "number",
        "Show line numbers",
        OptionValue::bool(false),
    ))
    .unwrap();
    opts.register(OptionSpec::new(
        "tabwidth",
        "Tab width",
        OptionValue::int(8),
    ))
    .unwrap();
}

fn make_ctx_with_vfs(vfs: &Arc<MockVfs>) -> CommandContext {
    CommandContext::new().with_vfs(Arc::clone(vfs) as Arc<dyn VfsDriver>)
}

fn make_ctx_with_vfs_and_name(vfs: &Arc<MockVfs>, name: &str) -> CommandContext {
    let mut ctx = make_ctx_with_vfs(vfs);
    ctx.set("name", ArgValue::String(name.to_string()));
    ctx
}

fn profiles_dir() -> PathBuf {
    PathBuf::from("/data/profiles")
}

// ========================================================================
// ProfileSaveCommand
// ========================================================================

#[test]
fn test_save_command_metadata() {
    let cmd = ProfileSaveCommand::new(profiles_dir());
    assert_eq!(cmd.id().name(), "profile-save");
    assert_eq!(cmd.names(), &["profile-save"]);
    assert!(!cmd.description().is_empty());
    assert_eq!(cmd.args().len(), 1);
    assert!(cmd.args()[0].required);
}

#[test]
fn test_save_valid() {
    let mut test = TestSessionRuntime::new();
    register_test_options(&test);
    test.kernel()
        .options
        .set_global("number", OptionValue::bool(true))
        .unwrap();

    let vfs = Arc::new(MockVfs::new());
    vfs.add_dir("/data");
    let args = make_ctx_with_vfs_and_name(&vfs, "my-profile");

    let cmd = ProfileSaveCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_success());

    // Verify file was written
    let written = vfs.read_to_string(std::path::Path::new("/data/profiles/my-profile.toml"));
    assert!(written.is_ok());
    let content = written.unwrap();
    assert!(content.contains("[metadata]"));
    assert!(content.contains("version = 1"));
}

#[test]
fn test_save_missing_name() {
    let mut test = TestSessionRuntime::new();
    register_test_options(&test);
    let vfs = Arc::new(MockVfs::new());
    let args = make_ctx_with_vfs(&vfs);

    let cmd = ProfileSaveCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_error());
}

#[test]
fn test_save_invalid_name() {
    let mut test = TestSessionRuntime::new();
    register_test_options(&test);
    let vfs = Arc::new(MockVfs::new());
    let args = make_ctx_with_vfs_and_name(&vfs, "../evil");

    let cmd = ProfileSaveCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_error());
}

#[test]
fn test_save_no_vfs() {
    let mut test = TestSessionRuntime::new();
    register_test_options(&test);
    let mut args = CommandContext::new();
    args.set("name", ArgValue::String("test".to_string()));

    let cmd = ProfileSaveCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_error());
}

#[test]
fn test_save_vfs_write_error() {
    let mut test = TestSessionRuntime::new();
    register_test_options(&test);
    let vfs = Arc::new(MockVfs::new());
    vfs.add_dir("/data");
    vfs.add_dir("/data/profiles");
    vfs.set_error(
        "/data/profiles/test.toml",
        reovim_driver_vfs::MockErrorKind::PermissionDenied,
    );
    let args = make_ctx_with_vfs_and_name(&vfs, "test");

    let cmd = ProfileSaveCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_error());
}

#[test]
fn test_save_overwrites_existing() {
    let mut test = TestSessionRuntime::new();
    register_test_options(&test);
    let vfs = Arc::new(MockVfs::new());
    vfs.add_dir("/data");
    vfs.add_dir("/data/profiles");
    vfs.add_file_str("/data/profiles/test.toml", "old content");

    let args = make_ctx_with_vfs_and_name(&vfs, "test");
    let cmd = ProfileSaveCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_success());

    let content = vfs
        .read_to_string(std::path::Path::new("/data/profiles/test.toml"))
        .unwrap();
    assert!(content.contains("[metadata]"));
}

// ========================================================================
// ProfileLoadCommand
// ========================================================================

#[test]
fn test_load_command_metadata() {
    let cmd = ProfileLoadCommand::new(profiles_dir());
    assert_eq!(cmd.id().name(), "profile-load");
    assert_eq!(cmd.names(), &["profile-load"]);
    assert!(!cmd.description().is_empty());
    assert_eq!(cmd.args().len(), 1);
}

#[test]
fn test_load_valid() {
    let mut test = TestSessionRuntime::new();
    register_test_options(&test);

    let toml_content = "\
[metadata]\nversion = 1\n\n\
[options.number]\ntype = \"bool\"\nvalue = true\n\n\
[options.tabwidth]\ntype = \"integer\"\nvalue = 4\n";

    let vfs = Arc::new(MockVfs::new());
    vfs.add_dir("/data/profiles");
    vfs.add_file_str("/data/profiles/test.toml", toml_content);
    let args = make_ctx_with_vfs_and_name(&vfs, "test");

    let cmd = ProfileLoadCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_success());

    assert_eq!(
        test.kernel().options.get_global("number"),
        Some(OptionValue::bool(true))
    );
    assert_eq!(
        test.kernel().options.get_global("tabwidth"),
        Some(OptionValue::int(4))
    );
}

#[test]
fn test_load_missing_profile() {
    let mut test = TestSessionRuntime::new();
    register_test_options(&test);
    let vfs = Arc::new(MockVfs::new());
    let args = make_ctx_with_vfs_and_name(&vfs, "nonexistent");

    let cmd = ProfileLoadCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(matches!(
        result,
        CommandResult::Error(ref msg) if msg.contains("not found")
    ));
}

#[test]
fn test_load_invalid_toml() {
    let mut test = TestSessionRuntime::new();
    register_test_options(&test);
    let vfs = Arc::new(MockVfs::new());
    vfs.add_dir("/data/profiles");
    vfs.add_file_str("/data/profiles/bad.toml", "not valid toml {{{");
    let args = make_ctx_with_vfs_and_name(&vfs, "bad");

    let cmd = ProfileLoadCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_error());
}

#[test]
fn test_load_missing_name() {
    let mut test = TestSessionRuntime::new();
    register_test_options(&test);
    let vfs = Arc::new(MockVfs::new());
    let args = make_ctx_with_vfs(&vfs);

    let cmd = ProfileLoadCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_error());
}

#[test]
fn test_load_invalid_name() {
    let mut test = TestSessionRuntime::new();
    register_test_options(&test);
    let vfs = Arc::new(MockVfs::new());
    let args = make_ctx_with_vfs_and_name(&vfs, "../evil");

    let cmd = ProfileLoadCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_error());
}

#[test]
fn test_load_no_vfs() {
    let mut test = TestSessionRuntime::new();
    register_test_options(&test);
    let mut args = CommandContext::new();
    args.set("name", ArgValue::String("test".to_string()));

    let cmd = ProfileLoadCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_error());
}

#[test]
fn test_load_with_unknown_options_warns() {
    let mut test = TestSessionRuntime::new();
    register_test_options(&test);

    let toml_content = "\
[metadata]\nversion = 1\n\n\
[options.nonexistent]\ntype = \"bool\"\nvalue = true\n";

    let vfs = Arc::new(MockVfs::new());
    vfs.add_dir("/data/profiles");
    vfs.add_file_str("/data/profiles/test.toml", toml_content);
    let args = make_ctx_with_vfs_and_name(&vfs, "test");

    let cmd = ProfileLoadCommand::new(profiles_dir());
    // Succeeds but with warnings (logged via tracing)
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_success());
}

// ========================================================================
// ProfileListCommand
// ========================================================================

#[test]
fn test_list_command_metadata() {
    let cmd = ProfileListCommand::new(profiles_dir());
    assert_eq!(cmd.id().name(), "profile-list");
    assert_eq!(cmd.names(), &["profile-list"]);
    assert!(!cmd.description().is_empty());
    assert!(cmd.args().is_empty());
}

#[test]
fn test_list_with_profiles() {
    let mut test = TestSessionRuntime::new();
    let vfs = Arc::new(MockVfs::new());
    vfs.add_dir("/data/profiles");
    vfs.add_file_str("/data/profiles/alpha.toml", "");
    vfs.add_file_str("/data/profiles/beta.toml", "");
    vfs.add_file_str("/data/profiles/not-toml.txt", "");
    let args = make_ctx_with_vfs(&vfs);

    let cmd = ProfileListCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_success());
}

#[test]
fn test_list_no_profiles_dir() {
    let mut test = TestSessionRuntime::new();
    let vfs = Arc::new(MockVfs::new());
    let args = make_ctx_with_vfs(&vfs);

    let cmd = ProfileListCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_error());
}

#[test]
fn test_list_empty_dir() {
    let mut test = TestSessionRuntime::new();
    let vfs = Arc::new(MockVfs::new());
    vfs.add_dir("/data/profiles");
    let args = make_ctx_with_vfs(&vfs);

    let cmd = ProfileListCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_error()); // "no profiles saved"
}

#[test]
fn test_list_no_vfs() {
    let mut test = TestSessionRuntime::new();
    let args = CommandContext::new();

    let cmd = ProfileListCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_error());
}

#[test]
fn test_list_vfs_error() {
    let mut test = TestSessionRuntime::new();
    let vfs = Arc::new(MockVfs::new());
    vfs.add_dir("/data/profiles");
    vfs.set_error(
        "/data/profiles",
        reovim_driver_vfs::MockErrorKind::PermissionDenied,
    );
    let args = make_ctx_with_vfs(&vfs);

    let cmd = ProfileListCommand::new(profiles_dir());
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_error());
}

// ========================================================================
// ProfileLoadCommand::complete()
// ========================================================================

#[test]
fn test_load_complete_no_dir() {
    let cmd = ProfileLoadCommand::new(PathBuf::from("/nonexistent/path"));
    let completions = cmd.complete("");
    assert!(completions.is_empty());
}
