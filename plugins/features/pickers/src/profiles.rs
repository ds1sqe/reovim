//! Configuration profile picker implementation

use std::{future::Future, pin::Pin};

use {
    reovim_core::config::ProfileManager,
    reovim_plugin_microscope::{
        MicroscopeAction, MicroscopeData, MicroscopeItem, Picker, PickerContext, PreviewContent,
    },
};

/// Picker for selecting configuration profiles
pub struct ProfilesPicker;

impl ProfilesPicker {
    /// Create a new profiles picker
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ProfilesPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for ProfilesPicker {
    fn name(&self) -> &'static str {
        "profiles"
    }

    fn title(&self) -> &'static str {
        "Select Profile"
    }

    fn prompt(&self) -> &'static str {
        "Profile> "
    }

    fn fetch(
        &self,
        _ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Vec<MicroscopeItem>> + Send + '_>> {
        Box::pin(async move {
            // Get profile list from ProfileManager
            let manager = ProfileManager::default();
            let profiles = manager.list_profiles();

            profiles
                .into_iter()
                .map(|name| {
                    // Try to load profile to get description
                    let description = ProfileManager::default()
                        .load_profile_readonly(&name)
                        .ok()
                        .map(|config| config.profile.description)
                        .filter(|d| !d.is_empty());

                    let mut item = MicroscopeItem::new(
                        name.clone(),
                        name.clone(),
                        MicroscopeData::Profile(name),
                        "profiles",
                    )
                    .with_icon('\u{f013}'); // Gear icon

                    if let Some(desc) = description {
                        item = item.with_detail(desc);
                    }

                    item
                })
                .collect()
        })
    }

    fn on_select(&self, item: &MicroscopeItem) -> MicroscopeAction {
        match &item.data {
            MicroscopeData::Profile(name) => MicroscopeAction::SwitchProfile(name.clone()),
            _ => MicroscopeAction::Nothing,
        }
    }

    fn preview(
        &self,
        item: &MicroscopeItem,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        let name = item.display.clone();

        Box::pin(async move {
            let manager = ProfileManager::default();
            let config = manager.load_profile_readonly(&name).ok()?;

            let mut lines = vec![format!("Profile: {}", config.profile.name), String::new()];

            if !config.profile.description.is_empty() {
                lines.push(format!("Description: {}", config.profile.description));
                lines.push(String::new());
            }

            lines.push("Editor Settings:".to_string());
            lines.push(format!("  Theme: {}", config.editor.theme));
            lines.push(format!("  Color Mode: {}", config.editor.colormode));
            lines.push(format!(
                "  Line Numbers: {}",
                if config.editor.number { "on" } else { "off" }
            ));
            lines.push(format!(
                "  Relative Numbers: {}",
                if config.editor.relativenumber {
                    "on"
                } else {
                    "off"
                }
            ));
            lines.push(format!(
                "  Indent Guides: {}",
                if config.editor.indentguide {
                    "on"
                } else {
                    "off"
                }
            ));
            lines.push(format!(
                "  Scrollbar: {}",
                if config.editor.scrollbar { "on" } else { "off" }
            ));
            lines.push(format!("  Tab Width: {}", config.editor.tabwidth));

            if !config.keybindings.mappings.is_empty() {
                lines.push(String::new());
                lines.push(format!("Custom Keybindings: {}", config.keybindings.mappings.len()));
            }

            Some(PreviewContent::new(lines))
        })
    }
}
