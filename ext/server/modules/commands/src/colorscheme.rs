//! Colorscheme command - switch themes at runtime.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_display_registry::theme::{BuiltinTheme, SharedThemeManager, ThemeLoader},
    reovim_driver_text_session::SessionRuntime,
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

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("theme", ArgKind::Rest, "Theme name")]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        let kernel = runtime.kernel();

        // Get SharedThemeManager from ServiceRegistry
        let Some(theme_manager) = kernel.services.get::<SharedThemeManager>() else {
            return CommandResult::Error("Theme system not initialized".to_string());
        };

        // Check if we got an argument
        let theme_name = ctx.string("theme");

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
#[path = "colorscheme_tests.rs"]
mod tests;
