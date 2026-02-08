//! Colorscheme command - switch themes at runtime.
//!
//! This module implements the `:colorscheme` command for changing the editor's
//! color theme.
//!
//! # Usage
//!
//! - `:colorscheme` - Show current theme name
//! - `:colorscheme <name>` - Switch to theme
//! - `:colorscheme invalid` - Error with available themes
//!
//! # Architecture
//!
//! This is a **policy module** - it decides HOW users switch themes.
//! The actual theme definitions and `ThemeManager` are **mechanism** (in the driver layer).

use reovim_driver_display::style::{BuiltinTheme, SharedThemeManager};

use crate::{CommandError, ExCommandContext, ExCommandHandler};

/// Colorscheme command - switch color theme.
///
/// Behavior:
/// - `:colorscheme` - Show current theme
/// - `:colorscheme dark` - Switch to dark theme
/// - `:colorscheme light` - Switch to light theme
/// - `:colorscheme tokyo-night-orange` - Switch to Tokyo Night Orange
///
/// # Example
///
/// ```ignore
/// let cmd = ColorschemeCommand;
/// cmd.execute(&mut ctx, &["dark"])?; // Switch to dark theme
/// cmd.execute(&mut ctx, &[])?;       // Show current theme
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ColorschemeCommand;

impl ColorschemeCommand {
    /// Available theme names.
    const AVAILABLE_THEMES: &'static [&'static str] = &["dark", "light", "tokyo-night-orange"];
}

impl ExCommandHandler for ColorschemeCommand {
    fn id(&self) -> &'static str {
        "colorscheme"
    }

    fn names(&self) -> &[&'static str] {
        &["colorscheme", "colors", "colo"]
    }

    fn execute(&self, ctx: &mut ExCommandContext<'_>, args: &[&str]) -> Result<(), CommandError> {
        // Get SharedThemeManager from ServiceRegistry
        let theme_manager = ctx
            .kernel
            .services
            .get::<SharedThemeManager>()
            .ok_or_else(|| {
                CommandError::ExecutionFailed("Theme system not initialized".to_string())
            })?;

        if args.is_empty() {
            // Show current theme
            let name = theme_manager.read().current_theme_name().to_string();
            // TODO: Display via message system when available
            tracing::info!(theme = %name, "current colorscheme");
            return Ok(());
        }

        // Switch to requested theme
        let theme_name = args[0];
        let new_theme = match theme_name {
            "dark" => BuiltinTheme::Dark.load(),
            "light" => BuiltinTheme::Light.load(),
            "tokyo-night-orange" => BuiltinTheme::TokyoNightOrange.load(),
            _ => {
                let available = Self::AVAILABLE_THEMES.join(", ");
                return Err(CommandError::InvalidArguments(format!(
                    "Unknown colorscheme: {theme_name}. Available: {available}"
                )));
            }
        };

        theme_manager.write().set_theme(new_theme);
        tracing::info!(theme = %theme_name, "switched colorscheme");

        // Note: UI refresh is handled by the event loop detecting theme change
        Ok(())
    }

    fn complete(&self, partial: &str) -> Vec<String> {
        Self::AVAILABLE_THEMES
            .iter()
            .filter(|name| name.starts_with(partial))
            .map(|s| (*s).to_string())
            .collect()
    }

    fn help(&self) -> &'static str {
        "Set or show the current color theme. Usage: :colorscheme [dark|light|tokyo-night-orange]"
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::sync::Arc};

    #[test]
    fn test_colorscheme_command_id() {
        let cmd = ColorschemeCommand;
        assert_eq!(cmd.id(), "colorscheme");
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
    fn test_colorscheme_help() {
        let cmd = ColorschemeCommand;
        let help = cmd.help();
        assert!(!help.is_empty());
        assert!(help.contains("colorscheme"));
        assert!(help.contains("dark"));
        assert!(help.contains("light"));
        assert!(help.contains("tokyo-night-orange"));
    }

    #[test]
    fn test_colorscheme_available_themes_constant() {
        assert_eq!(ColorschemeCommand::AVAILABLE_THEMES.len(), 3);
        assert!(ColorschemeCommand::AVAILABLE_THEMES.contains(&"dark"));
        assert!(ColorschemeCommand::AVAILABLE_THEMES.contains(&"light"));
        assert!(ColorschemeCommand::AVAILABLE_THEMES.contains(&"tokyo-night-orange"));
    }

    #[test]
    fn test_colorscheme_execute_no_theme_manager_returns_error() {
        let kernel = reovim_kernel::api::v1::KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = ColorschemeCommand;
        // No SharedThemeManager registered in services
        let result = cmd.execute(&mut ctx, &["dark"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let display = err.to_string();
        assert!(display.contains("Theme system not initialized"));
    }

    #[test]
    fn test_colorscheme_execute_show_current_with_theme_manager() {
        let kernel = reovim_kernel::api::v1::KernelContext::default();
        let theme_manager = SharedThemeManager::new(BuiltinTheme::Dark.load());
        kernel.services.register(Arc::new(theme_manager));

        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = ColorschemeCommand;
        // Empty args = show current theme
        let result = cmd.execute(&mut ctx, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_colorscheme_execute_switch_to_dark() {
        let kernel = reovim_kernel::api::v1::KernelContext::default();
        let theme_manager = SharedThemeManager::new(BuiltinTheme::Light.load());
        kernel.services.register(Arc::new(theme_manager));

        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = ColorschemeCommand;
        let result = cmd.execute(&mut ctx, &["dark"]);
        assert!(result.is_ok());

        // Verify theme was switched
        let tm = kernel.services.get::<SharedThemeManager>().unwrap();
        assert_eq!(tm.read().current_theme_name(), "dark");
    }

    #[test]
    fn test_colorscheme_execute_switch_to_light() {
        let kernel = reovim_kernel::api::v1::KernelContext::default();
        let theme_manager = SharedThemeManager::new(BuiltinTheme::Dark.load());
        kernel.services.register(Arc::new(theme_manager));

        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = ColorschemeCommand;
        let result = cmd.execute(&mut ctx, &["light"]);
        assert!(result.is_ok());

        let tm = kernel.services.get::<SharedThemeManager>().unwrap();
        assert_eq!(tm.read().current_theme_name(), "light");
    }

    #[test]
    fn test_colorscheme_execute_switch_to_tokyo_night_orange() {
        let kernel = reovim_kernel::api::v1::KernelContext::default();
        let theme_manager = SharedThemeManager::new(BuiltinTheme::Dark.load());
        kernel.services.register(Arc::new(theme_manager));

        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = ColorschemeCommand;
        let result = cmd.execute(&mut ctx, &["tokyo-night-orange"]);
        assert!(result.is_ok());

        let tm = kernel.services.get::<SharedThemeManager>().unwrap();
        assert_eq!(tm.read().current_theme_name(), "tokyo-night-orange");
    }

    #[test]
    fn test_colorscheme_execute_invalid_theme_returns_error() {
        let kernel = reovim_kernel::api::v1::KernelContext::default();
        let theme_manager = SharedThemeManager::new(BuiltinTheme::Dark.load());
        kernel.services.register(Arc::new(theme_manager));

        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = ColorschemeCommand;
        let result = cmd.execute(&mut ctx, &["nonexistent"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let display = err.to_string();
        assert!(display.contains("nonexistent"));
        assert!(display.contains("Available"));
    }

    #[test]
    fn test_colorscheme_execute_no_theme_manager_empty_args() {
        let kernel = reovim_kernel::api::v1::KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = ColorschemeCommand;
        // No theme manager, even with empty args the error is the same
        let result = cmd.execute(&mut ctx, &[]);
        assert!(result.is_err());
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
}
