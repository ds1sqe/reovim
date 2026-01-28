//! Editor options and settings module - POLICY layer.
//!
//! This module provides configurable editor settings like `virtualedit`,
//! following vim's `:set` option pattern.
//!
//! # Design Philosophy
//!
//! Options are POLICY - they define how the editor behaves based on
//! user preferences. The kernel provides mechanisms; this module
//! provides configuration policy.
//!
//! # Example
//!
//! ```
//! use reovim_module_options::{EditorSettings, VirtualEditMode};
//!
//! let mut settings = EditorSettings::default();
//!
//! // Allow virtual edit in block mode only
//! settings.virtual_edit.add_mode(VirtualEditMode::Block);
//!
//! // Check if virtual edit is allowed for a specific mode
//! assert!(!settings.virtual_edit.allows_mode(VirtualEditMode::Insert));
//! assert!(settings.virtual_edit.allows_mode(VirtualEditMode::Block));
//! ```

use reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version};

/// Represents when virtual edit is allowed.
///
/// Virtual edit allows the cursor to move beyond the end of a line,
/// into "virtual" space. This is useful for block selections and
/// certain editing operations.
///
/// Corresponds to vim's `virtualedit` option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VirtualEditMode {
    /// Never allow virtual positions (default behavior).
    #[default]
    None,
    /// Always allow virtual positions in all modes.
    All,
    /// Only allow in visual block mode (Ctrl-V).
    Block,
    /// Only allow in insert mode.
    Insert,
    /// Only allow in one-more mode (cursor can be one past line end).
    OneMore,
}

/// Configuration for virtual edit behavior.
///
/// Stores which modes allow virtual cursor positions.
/// Multiple modes can be enabled simultaneously.
#[derive(Debug, Clone, Default)]
pub struct VirtualEditConfig {
    /// Enabled virtual edit modes.
    modes: Vec<VirtualEditMode>,
}

impl VirtualEditConfig {
    /// Create a new empty virtual edit configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create configuration that allows virtual edit everywhere.
    #[must_use]
    pub fn all() -> Self {
        Self {
            modes: vec![VirtualEditMode::All],
        }
    }

    /// Create configuration for block mode only.
    #[must_use]
    pub fn block_only() -> Self {
        Self {
            modes: vec![VirtualEditMode::Block],
        }
    }

    /// Add a mode to the allowed list.
    pub fn add_mode(&mut self, mode: VirtualEditMode) {
        if !self.modes.contains(&mode) {
            self.modes.push(mode);
        }
    }

    /// Remove a mode from the allowed list.
    pub fn remove_mode(&mut self, mode: VirtualEditMode) {
        self.modes.retain(|m| *m != mode);
    }

    /// Clear all modes (disable virtual edit).
    pub fn clear(&mut self) {
        self.modes.clear();
    }

    /// Check if a specific mode allows virtual edit.
    ///
    /// Returns `true` if `VirtualEditMode::All` is set, or if the
    /// specific mode is in the allowed list.
    #[must_use]
    pub fn allows_mode(&self, mode: VirtualEditMode) -> bool {
        self.modes.contains(&VirtualEditMode::All) || self.modes.contains(&mode)
    }

    /// Check if any virtual edit is enabled.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty() is not const in stable Rust
    pub fn is_enabled(&self) -> bool {
        !self.modes.is_empty()
    }

    /// Check if virtual edit is completely disabled.
    #[must_use]
    pub fn is_disabled(&self) -> bool {
        self.modes.is_empty() || self.modes == vec![VirtualEditMode::None]
    }

    /// Get the list of enabled modes.
    #[must_use]
    pub fn modes(&self) -> &[VirtualEditMode] {
        &self.modes
    }

    /// Set modes from a list (replaces existing).
    pub fn set_modes(&mut self, modes: Vec<VirtualEditMode>) {
        self.modes = modes;
    }
}

/// Editor-wide settings container.
///
/// This struct holds all configurable editor options.
/// It's designed to be extended as more options are added.
#[derive(Debug, Clone, Default)]
pub struct EditorSettings {
    /// Virtual edit configuration.
    pub virtual_edit: VirtualEditConfig,
}

impl EditorSettings {
    /// Create new default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Options module for managing editor settings.
///
/// This module provides the infrastructure for editor configuration.
/// Settings can be queried and modified at runtime.
pub struct OptionsModule {
    settings: EditorSettings,
}

impl OptionsModule {
    /// Create a new options module with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            settings: EditorSettings::default(),
        }
    }

    /// Get a reference to the current settings.
    #[must_use]
    pub const fn settings(&self) -> &EditorSettings {
        &self.settings
    }

    /// Get a mutable reference to the settings.
    pub const fn settings_mut(&mut self) -> &mut EditorSettings {
        &mut self.settings
    }
}

impl Default for OptionsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for OptionsModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("options")
    }

    fn name(&self) -> &'static str {
        "Editor Options"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(OptionsModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_virtual_edit_mode_default() {
        let mode = VirtualEditMode::default();
        assert_eq!(mode, VirtualEditMode::None);
    }

    #[test]
    fn test_virtual_edit_config_new() {
        let config = VirtualEditConfig::new();
        assert!(config.is_disabled());
        assert!(!config.is_enabled());
    }

    #[test]
    fn test_virtual_edit_config_all() {
        let config = VirtualEditConfig::all();
        assert!(config.is_enabled());
        assert!(config.allows_mode(VirtualEditMode::Block));
        assert!(config.allows_mode(VirtualEditMode::Insert));
        assert!(config.allows_mode(VirtualEditMode::OneMore));
    }

    #[test]
    fn test_virtual_edit_config_block_only() {
        let config = VirtualEditConfig::block_only();
        assert!(config.allows_mode(VirtualEditMode::Block));
        assert!(!config.allows_mode(VirtualEditMode::Insert));
    }

    #[test]
    fn test_virtual_edit_config_add_mode() {
        let mut config = VirtualEditConfig::new();
        config.add_mode(VirtualEditMode::Insert);
        config.add_mode(VirtualEditMode::Block);

        assert!(config.allows_mode(VirtualEditMode::Insert));
        assert!(config.allows_mode(VirtualEditMode::Block));
        assert!(!config.allows_mode(VirtualEditMode::OneMore));
    }

    #[test]
    fn test_virtual_edit_config_remove_mode() {
        let mut config = VirtualEditConfig::new();
        config.add_mode(VirtualEditMode::Insert);
        config.add_mode(VirtualEditMode::Block);
        config.remove_mode(VirtualEditMode::Insert);

        assert!(!config.allows_mode(VirtualEditMode::Insert));
        assert!(config.allows_mode(VirtualEditMode::Block));
    }

    #[test]
    fn test_virtual_edit_config_clear() {
        let mut config = VirtualEditConfig::all();
        config.clear();
        assert!(config.is_disabled());
    }

    #[test]
    fn test_virtual_edit_config_no_duplicates() {
        let mut config = VirtualEditConfig::new();
        config.add_mode(VirtualEditMode::Block);
        config.add_mode(VirtualEditMode::Block);
        config.add_mode(VirtualEditMode::Block);

        assert_eq!(config.modes().len(), 1);
    }

    #[test]
    fn test_editor_settings_default() {
        let settings = EditorSettings::default();
        assert!(settings.virtual_edit.is_disabled());
    }

    #[test]
    fn test_options_module_id() {
        let module = OptionsModule::new();
        assert_eq!(module.id().as_str(), "options");
    }

    #[test]
    fn test_options_module_name() {
        let module = OptionsModule::new();
        assert_eq!(module.name(), "Editor Options");
    }

    #[test]
    fn test_options_module_settings_access() {
        let mut module = OptionsModule::new();

        // Modify settings
        module
            .settings_mut()
            .virtual_edit
            .add_mode(VirtualEditMode::Block);

        // Read settings
        assert!(
            module
                .settings()
                .virtual_edit
                .allows_mode(VirtualEditMode::Block)
        );
    }

    #[test]
    fn test_set_modes() {
        let mut config = VirtualEditConfig::new();
        config.set_modes(vec![VirtualEditMode::Insert, VirtualEditMode::OneMore]);

        assert!(config.allows_mode(VirtualEditMode::Insert));
        assert!(config.allows_mode(VirtualEditMode::OneMore));
        assert!(!config.allows_mode(VirtualEditMode::Block));
    }
}
