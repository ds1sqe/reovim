use crate::{
    config::{EditorConfig, ProfileConfig},
    highlight::{ColorMode, Theme, ThemeName},
};

use super::Runtime;

impl Runtime {
    /// Builder method to set the profile name to load at startup
    #[must_use]
    pub fn with_profile(mut self, profile_name: Option<String>) -> Self {
        if let Some(name) = profile_name {
            self.current_profile_name = name;
        }
        self
    }

    /// Load and apply a profile by name
    ///
    /// Returns true if the profile was loaded successfully.
    pub fn load_profile(&mut self, name: &str) -> bool {
        match self.profile_manager.load_profile(name) {
            Ok(config) => {
                self.apply_profile(&config);
                self.current_profile_name = name.to_string();
                tracing::info!(profile = %name, "Profile loaded and applied");
                true
            }
            Err(e) => {
                tracing::error!(profile = %name, error = %e, "Failed to load profile");
                false
            }
        }
    }

    /// Apply a profile configuration to the runtime
    pub fn apply_profile(&mut self, config: &ProfileConfig) {
        // Apply theme with overrides
        if let Some(theme_name) = ThemeName::parse(&config.editor.theme) {
            self.theme =
                Theme::from_name_with_overrides(theme_name, &config.editor.theme_overrides);
            // Theme change will trigger rehighlighting via event bus
            self.rehighlight_all_buffers();
            tracing::debug!(
                theme = %config.editor.theme,
                overrides = config.editor.theme_overrides.len(),
                "Applied theme with overrides from profile"
            );
        }

        // Apply color mode
        if let Some(mode) = ColorMode::parse(&config.editor.colormode) {
            self.color_mode = mode;
            tracing::debug!(colormode = %config.editor.colormode, "Applied colormode from profile");
        }

        // Apply editor options
        // Note: These settings need to be applied to windows, which is done at render time
        // or through the :set command mechanism. For now we store them in the profile.

        // Apply indent guide setting
        self.indent_analyzer.set_enabled(config.editor.indentguide);

        tracing::debug!(
            number = config.editor.number,
            relativenumber = config.editor.relativenumber,
            indentguide = config.editor.indentguide,
            scrollbar = config.editor.scrollbar,
            "Applied editor options from profile"
        );

        // Note: Keybinding application is deferred to Phase 5
    }

    /// Save current runtime settings as a profile
    pub fn save_current_as_profile(&self, name: &str) -> bool {
        // Build profile from current state
        // Note: Theme doesn't track its name, so we use a default
        // A future improvement would be to track the current theme name
        let editor_config = EditorConfig {
            theme: "dark".to_string(), // TODO: Track current theme name
            colormode: format!("{:?}", self.color_mode).to_lowercase(),
            number: true,         // TODO: Get from window settings
            relativenumber: true, // TODO: Get from window settings
            indentguide: self.indent_analyzer.is_enabled(),
            scrollbar: true, // TODO: Get from window settings
            ..EditorConfig::default()
        };

        let config = ProfileConfig {
            profile: crate::config::ProfileMeta {
                name: name.to_string(),
                description: String::new(),
                version: "1".to_string(),
            },
            editor: editor_config,
            ..ProfileConfig::default()
        };

        match self.profile_manager.save_profile(name, &config) {
            Ok(()) => {
                tracing::info!(profile = %name, "Profile saved");
                true
            }
            Err(e) => {
                tracing::error!(profile = %name, error = %e, "Failed to save profile");
                false
            }
        }
    }

    /// List available profiles
    #[must_use]
    pub fn list_profiles(&self) -> Vec<String> {
        self.profile_manager.list_profiles()
    }

    /// Initialize the profile system and optionally load the startup profile
    pub fn init_profiles(&mut self) {
        // Ensure default profile exists
        if let Err(e) = self.profile_manager.initialize() {
            tracing::warn!(error = %e, "Failed to initialize profile system");
            return;
        }

        // Load the startup profile
        let profile_name = self.current_profile_name.clone();
        if self.profile_manager.profile_exists(&profile_name) {
            self.load_profile(&profile_name);
        } else {
            tracing::debug!(profile = %profile_name, "Startup profile not found, using defaults");
        }
    }
}
