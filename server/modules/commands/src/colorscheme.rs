//! Colorscheme command - switch themes at runtime.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_display::style::{BuiltinTheme, SharedThemeManager, ThemeLoader},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

const COMMANDS_MODULE: ModuleId = ModuleId::new("commands");

/// Colorscheme command - switch color theme.
///
/// Behavior:
/// - `:colorscheme` - Show current theme
/// - `:colorscheme dark` - Switch to dark theme
/// - `:colorscheme light` - Switch to light theme
/// - `:colorscheme tokyo-night-orange` - Switch to Tokyo Night Orange
#[derive(Debug, Clone, Copy)]
pub struct ColorschemeCommand;

impl ColorschemeCommand {
    /// Built-in theme names (fallback when `ThemeLoader` is unavailable).
    const BUILTIN_THEMES: &'static [&'static str] = &["dark", "light", "tokyo-night-orange"];

    /// Get available theme names, using `ThemeLoader::discover()` if available,
    /// falling back to hardcoded built-in names.
    fn available_theme_names(services: &reovim_kernel::api::v1::ServiceRegistry) -> String {
        services.get::<ThemeLoader>().map_or_else(
            || Self::BUILTIN_THEMES.join(", "),
            |loader| {
                loader
                    .discover()
                    .into_iter()
                    .map(|t| t.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        )
    }
}

impl Command for ColorschemeCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "colorscheme")
    }

    fn description(&self) -> &'static str {
        "Set or show the current color theme. Usage: :colorscheme [dark|light|tokyo-night-orange]"
    }

    fn names(&self) -> &[&'static str] {
        &["colorscheme", "colors", "colo"]
    }

    fn complete(&self, partial: &str) -> Vec<String> {
        Self::BUILTIN_THEMES
            .iter()
            .filter(|name| name.starts_with(partial))
            .map(|s| (*s).to_string())
            .collect()
    }
}

impl CommandHandler for ColorschemeCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        let kernel = runtime.kernel();

        // Get SharedThemeManager from ServiceRegistry
        let Some(theme_manager) = kernel.services.get::<SharedThemeManager>() else {
            return CommandResult::Error("Theme system not initialized".to_string());
        };

        // Check if we got an argument
        let theme_name = ctx.string("file");

        if theme_name.is_none() {
            // Show current theme
            let name = theme_manager.read().current_theme_name().to_string();
            tracing::info!(theme = %name, "current colorscheme");
            return CommandResult::Success;
        }

        let theme_name = theme_name.expect("checked above");

        // Try ThemeLoader first (finds file themes + built-in fallback)
        if let Some(loader) = kernel.services.get::<ThemeLoader>()
            && let Ok(theme) = loader.load(theme_name)
        {
            theme_manager.write().set_theme(theme);
            tracing::info!(theme = %theme_name, "switched colorscheme");
            return CommandResult::Success;
        }

        // Fall back to hardcoded built-in match
        let new_theme = match theme_name {
            "dark" => BuiltinTheme::Dark.load(),
            "light" => BuiltinTheme::Light.load(),
            "tokyo-night-orange" => BuiltinTheme::TokyoNightOrange.load(),
            _ => {
                let available = Self::available_theme_names(&kernel.services);
                return CommandResult::Error(format!(
                    "invalid arguments: Unknown colorscheme: {theme_name}. Available: {available}"
                ));
            }
        };

        theme_manager.write().set_theme(new_theme);
        tracing::info!(theme = %theme_name, "switched colorscheme");
        CommandResult::Success
    }
}

#[cfg(test)]
mod tests {
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
            ctx.set(
                "file",
                reovim_driver_command::ArgValue::String("dark".to_string()),
            );
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
                "file",
                reovim_driver_command::ArgValue::String("nonexistent-theme".to_string()),
            );
            let result = cmd.execute(runtime, &ctx);
            assert!(result.is_error());
        });
    }
}
