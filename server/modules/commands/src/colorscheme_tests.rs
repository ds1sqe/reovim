use {super::*, std::sync::Arc};

#[test]
fn test_colorscheme_command_id() {
    let cmd = ColorschemeCommand;
    assert_eq!(cmd.id().name(), "colorscheme");
    assert_eq!(cmd.id().module().as_str(), "commands");
}

#[test]
fn test_colorscheme_command_names() {
    let cmd = ColorschemeCommand;
    let names = cmd.names();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"colorscheme"));
    assert!(names.contains(&"colors"));
    assert!(names.contains(&"colo"));
}

#[test]
fn test_colorscheme_command_args() {
    let cmd = ColorschemeCommand;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "theme");
    assert_eq!(args[0].kind, ArgKind::Rest);
    assert!(!args[0].required);
}

#[test]
fn test_colorscheme_completion_partial_d() {
    let cmd = ColorschemeCommand;
    let completions = cmd.complete("d");
    assert_eq!(completions.len(), 1);
    assert!(completions.contains(&"dark".to_string()));
}

#[test]
fn test_colorscheme_completion_partial_l() {
    let cmd = ColorschemeCommand;
    let completions = cmd.complete("l");
    assert_eq!(completions.len(), 1);
    assert!(completions.contains(&"light".to_string()));
}

#[test]
fn test_colorscheme_completion_partial_t() {
    let cmd = ColorschemeCommand;
    let completions = cmd.complete("t");
    assert_eq!(completions.len(), 1);
    assert!(completions.contains(&"tokyo-night-orange".to_string()));
}

#[test]
fn test_colorscheme_completion_empty_returns_all() {
    let cmd = ColorschemeCommand;
    let completions = cmd.complete("");
    assert_eq!(completions.len(), 3);
    assert!(completions.contains(&"dark".to_string()));
    assert!(completions.contains(&"light".to_string()));
    assert!(completions.contains(&"tokyo-night-orange".to_string()));
}

#[test]
fn test_colorscheme_completion_no_match() {
    let cmd = ColorschemeCommand;
    let completions = cmd.complete("xyz");
    assert!(completions.is_empty());
}

#[test]
fn test_colorscheme_completion_full_name() {
    let cmd = ColorschemeCommand;
    let completions = cmd.complete("dark");
    assert_eq!(completions.len(), 1);
    assert!(completions.contains(&"dark".to_string()));
}

#[test]
fn test_colorscheme_description() {
    let cmd = ColorschemeCommand;
    let desc = cmd.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("colorscheme"));
    assert!(desc.contains("dark"));
}

#[test]
fn test_colorscheme_builtin_themes_constant() {
    assert_eq!(ColorschemeCommand::BUILTIN_THEMES.len(), 3);
    assert!(ColorschemeCommand::BUILTIN_THEMES.contains(&"dark"));
    assert!(ColorschemeCommand::BUILTIN_THEMES.contains(&"light"));
    assert!(ColorschemeCommand::BUILTIN_THEMES.contains(&"tokyo-night-orange"));
}

#[test]
fn test_colorscheme_command_debug() {
    let cmd = ColorschemeCommand;
    let debug = format!("{cmd:?}");
    assert!(debug.contains("ColorschemeCommand"));
}

#[test]
fn test_colorscheme_command_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ColorschemeCommand>();
}

#[test]
fn test_available_theme_names_without_loader() {
    let services = reovim_kernel::api::v1::ServiceRegistry::new();
    let names = ColorschemeCommand::available_theme_names(&services);
    assert!(names.contains("dark"));
    assert!(names.contains("light"));
    assert!(names.contains("tokyo-night-orange"));
}

#[test]
fn test_available_theme_names_with_loader() {
    use {std::io::Write, tempfile::TempDir};

    let services = reovim_kernel::api::v1::ServiceRegistry::new();

    let temp_dir = TempDir::new().unwrap();
    let theme_path = temp_dir.path().join("custom.toml");
    let mut f = std::fs::File::create(&theme_path).unwrap();
    f.write_all(b"[meta]\nname = \"Custom\"\n").unwrap();

    let loader = ThemeLoader::with_paths(vec![temp_dir.path().to_path_buf()]);
    services.register(Arc::new(loader));

    let names = ColorschemeCommand::available_theme_names(&services);
    assert!(names.contains("custom"));
    assert!(names.contains("dark"));
}

// ========================================================================
// Execute tests
// ========================================================================

#[test]
fn test_colorscheme_execute_no_theme_manager() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness.with_runtime(|runtime| {
        let cmd = ColorschemeCommand;
        let ctx = CommandContext::new();
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_colorscheme_execute_show_current() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    // Register SharedThemeManager
    harness
        .kernel()
        .services
        .register(Arc::new(SharedThemeManager::new(BuiltinTheme::Dark.load())));

    harness.with_runtime(|runtime| {
        let cmd = ColorschemeCommand;
        // No "file" argument → show current theme
        let ctx = CommandContext::new();
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });
}

#[test]
fn test_colorscheme_execute_switch_builtin_dark() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness
        .kernel()
        .services
        .register(Arc::new(SharedThemeManager::new(BuiltinTheme::Dark.load())));

    harness.with_runtime(|runtime| {
        let cmd = ColorschemeCommand;
        let mut ctx = CommandContext::new();
        ctx.set("theme", reovim_driver_command::ArgValue::String("dark".to_string()));
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });
}

#[test]
fn test_colorscheme_execute_unknown_theme() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness
        .kernel()
        .services
        .register(Arc::new(SharedThemeManager::new(BuiltinTheme::Dark.load())));

    harness.with_runtime(|runtime| {
        let cmd = ColorschemeCommand;
        let mut ctx = CommandContext::new();
        ctx.set(
            "theme",
            reovim_driver_command::ArgValue::String("nonexistent-theme".to_string()),
        );
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}
