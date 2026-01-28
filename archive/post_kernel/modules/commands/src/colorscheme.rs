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
    use super::*;

    #[test]
    fn test_colorscheme_command_id() {
        let cmd = ColorschemeCommand;
        assert_eq!(cmd.id(), "colorscheme");
    }

    #[test]
    fn test_colorscheme_command_names() {
        let cmd = ColorschemeCommand;
        let names = cmd.names();
        assert!(names.contains(&"colorscheme"));
        assert!(names.contains(&"colors"));
        assert!(names.contains(&"colo"));
    }

    #[test]
    fn test_colorscheme_completion() {
        let cmd = ColorschemeCommand;

        // Complete partial "d"
        let completions = cmd.complete("d");
        assert!(completions.contains(&"dark".to_string()));
        assert!(!completions.contains(&"light".to_string()));

        // Complete empty string - should return all
        let completions = cmd.complete("");
        assert_eq!(completions.len(), 3);
        assert!(completions.contains(&"dark".to_string()));
        assert!(completions.contains(&"light".to_string()));
        assert!(completions.contains(&"tokyo-night-orange".to_string()));

        // Complete "t"
        let completions = cmd.complete("t");
        assert!(completions.contains(&"tokyo-night-orange".to_string()));
        assert!(!completions.contains(&"dark".to_string()));
    }

    #[test]
    fn test_colorscheme_help() {
        let cmd = ColorschemeCommand;
        let help = cmd.help();
        assert!(help.contains("colorscheme"));
        assert!(help.contains("dark"));
        assert!(help.contains("light"));
        assert!(help.contains("tokyo-night-orange"));
    }
}
