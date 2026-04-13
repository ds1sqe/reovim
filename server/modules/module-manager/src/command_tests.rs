use {
    super::*,
    reovim_driver_command::Command,
    reovim_driver_session::{BufferApi, testing::TestSessionRuntime},
    reovim_kernel::api::v1::ModuleId,
    reovim_subsys_command_types::CommandContext,
    reovim_subsys_module_loader::report::ModuleLoadReport,
    std::sync::Arc,
};

// ========================================================================
// Command metadata
// ========================================================================

#[test]
fn test_command_id() {
    let cmd = ModulesCommand::new();
    assert_eq!(cmd.id().module().as_str(), "module-manager");
    assert_eq!(cmd.id().name(), "modules");
}

#[test]
fn test_command_description() {
    let cmd = ModulesCommand::new();
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_command_names() {
    let cmd = ModulesCommand::new();
    assert_eq!(cmd.names(), &["Modules"]);
}

#[test]
fn test_command_no_args() {
    let cmd = ModulesCommand::new();
    assert!(cmd.args().is_empty());
}

#[test]
fn test_command_default() {
    fn create_default<T: Default>() -> T {
        T::default()
    }
    let cmd: ModulesCommand = create_default();
    assert_eq!(cmd.id().name(), "modules");
}

// ========================================================================
// Command execution
// ========================================================================

#[test]
fn test_execute_creates_buffer() {
    let mut test = TestSessionRuntime::new();
    let args = CommandContext::new();

    let cmd = ModulesCommand::new();
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_success());
}

#[test]
fn test_execute_sets_active_buffer() {
    let mut test = TestSessionRuntime::new();
    let args = CommandContext::new();

    let cmd = ModulesCommand::new();
    test.with_runtime(|runtime| {
        cmd.execute(runtime, &args);
        assert!(runtime.active_buffer().is_some());
    });
}

#[test]
fn test_execute_buffer_not_modified() {
    let mut test = TestSessionRuntime::new();
    let args = CommandContext::new();

    let cmd = ModulesCommand::new();
    test.with_runtime(|runtime| {
        cmd.execute(runtime, &args);
        let buf_id = runtime.active_buffer().unwrap();
        assert_eq!(runtime.is_buffer_modified(buf_id), Some(false));
    });
}

#[test]
fn test_execute_no_report_shows_message() {
    let mut test = TestSessionRuntime::new();
    let args = CommandContext::new();

    let cmd = ModulesCommand::new();
    test.with_runtime(|runtime| {
        cmd.execute(runtime, &args);
        let buf_id = runtime.active_buffer().unwrap();
        let content = runtime.buffer_content(buf_id).unwrap();
        assert!(content.contains("No module load report available"));
    });
}

#[test]
fn test_execute_with_report() {
    let mut test = TestSessionRuntime::new();
    let args = CommandContext::new();

    // Register a ModuleLoadReport via kernel services (interior mutability)
    {
        let mut report = ModuleLoadReport::new();
        report.loaded.push(ModuleId::new("vim"));
        report.loaded.push(ModuleId::new("editor"));
        report.disabled.push(ModuleId::new("treesitter-rust"));
        report
            .failed
            .push((ModuleId::new("broken"), "init failed".to_string()));
        test.kernel().services.register(Arc::new(report));
    }

    let cmd = ModulesCommand::new();
    test.with_runtime(|runtime| {
        cmd.execute(runtime, &args);
        let buf_id = runtime.active_buffer().unwrap();
        let content = runtime.buffer_content(buf_id).unwrap();
        assert!(content.contains("=== Modules ==="));
        assert!(content.contains("[OK] vim"));
        assert!(content.contains("[OK] editor"));
        assert!(content.contains("[--] treesitter-rust"));
        assert!(content.contains("[!!] broken"));
        assert!(content.contains("init failed"));
    });
}

#[test]
fn test_execute_with_missing_deps() {
    let mut test = TestSessionRuntime::new();
    let args = CommandContext::new();

    {
        let mut report = ModuleLoadReport::new();
        report.loaded.push(ModuleId::new("completion"));
        report
            .missing_deps
            .push((ModuleId::new("completion"), ModuleId::new("lsp")));
        test.kernel().services.register(Arc::new(report));
    }

    let cmd = ModulesCommand::new();
    test.with_runtime(|runtime| {
        cmd.execute(runtime, &args);
        let buf_id = runtime.active_buffer().unwrap();
        let content = runtime.buffer_content(buf_id).unwrap();
        assert!(content.contains("Dependency Issues"));
        assert!(content.contains("completion requires 'lsp'"));
    });
}

#[test]
fn test_execute_always_succeeds() {
    let mut test = TestSessionRuntime::new();
    let args = CommandContext::new();

    let cmd = ModulesCommand::new();
    let result = test.with_runtime(|runtime| cmd.execute(runtime, &args));
    assert!(result.is_success());
}
