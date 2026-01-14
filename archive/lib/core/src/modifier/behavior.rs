//! Behavior modifiers for keybinding overrides and feature flags

use std::collections::{HashMap, HashSet};

use crate::bind::CommandRef;

/// Unique identifier for a command (for disable lists)
pub type CommandId = &'static str;

/// Keybinding override action
#[derive(Debug, Clone)]
pub enum KeyBindingAction {
    /// Bind the key to a specific command
    Bind(CommandRef),
    /// Unbind the key (no action)
    Unbind,
}

/// Feature flags that can be toggled by modifiers
#[derive(Debug, Clone, Default)]
pub struct FeatureFlags {
    /// Disable completion popup
    pub disable_completion: Option<bool>,
    /// Disable auto-indent
    pub disable_auto_indent: Option<bool>,
    /// Disable syntax highlighting
    pub disable_syntax: Option<bool>,
    /// Custom feature flags (for plugins)
    pub custom: HashMap<String, bool>,
}

impl FeatureFlags {
    /// Create empty feature flags
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Merge with another feature flags, preferring other's values when set
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let mut custom = self.custom.clone();
        custom.extend(other.custom.clone());

        Self {
            disable_completion: other.disable_completion.or(self.disable_completion),
            disable_auto_indent: other.disable_auto_indent.or(self.disable_auto_indent),
            disable_syntax: other.disable_syntax.or(self.disable_syntax),
            custom,
        }
    }

    /// Check if completion is disabled
    #[must_use]
    pub fn is_completion_disabled(&self) -> bool {
        self.disable_completion.unwrap_or(false)
    }
}

/// Behavior modifiers for keybindings and commands
#[derive(Debug, Clone, Default)]
pub struct BehaviorModifiers {
    /// Keybinding overrides: (`key_sequence`) -> action
    ///
    /// Key sequences are mode-independent here; the modifier's `matches()`
    /// determines when these apply.
    pub keybinding_overrides: HashMap<String, KeyBindingAction>,
    /// Commands to disable in this context
    pub disabled_commands: HashSet<CommandId>,
    /// Feature flags
    pub features: FeatureFlags,
}

impl BehaviorModifiers {
    /// Create empty behavior modifiers
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Check if any behavior is set
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keybinding_overrides.is_empty()
            && self.disabled_commands.is_empty()
            && self.features.disable_completion.is_none()
            && self.features.disable_auto_indent.is_none()
            && self.features.disable_syntax.is_none()
            && self.features.custom.is_empty()
    }

    /// Merge with another behavior modifiers, preferring other's values
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let mut keybinding_overrides = self.keybinding_overrides.clone();
        keybinding_overrides.extend(other.keybinding_overrides.clone());

        let mut disabled_commands = self.disabled_commands.clone();
        disabled_commands.extend(other.disabled_commands.clone());

        Self {
            keybinding_overrides,
            disabled_commands,
            features: self.features.merge(&other.features),
        }
    }

    /// Add a keybinding override
    #[must_use]
    pub fn with_keybinding(mut self, key: impl Into<String>, action: KeyBindingAction) -> Self {
        self.keybinding_overrides.insert(key.into(), action);
        self
    }

    /// Add a disabled command
    #[must_use]
    pub fn with_disabled_command(mut self, cmd_id: CommandId) -> Self {
        self.disabled_commands.insert(cmd_id);
        self
    }

    /// Disable completion
    #[must_use]
    pub const fn with_completion_disabled(mut self, disabled: bool) -> Self {
        self.features.disable_completion = Some(disabled);
        self
    }
}

/// Computed behavior state for a window after applying all modifiers
#[derive(Debug, Clone, Default)]
pub struct WindowBehaviorState {
    /// Effective behavior modifiers
    pub behavior: BehaviorModifiers,
    /// IDs of modifiers that contributed to this state
    pub applied_modifiers: Vec<&'static str>,
}

impl WindowBehaviorState {
    /// Create empty state
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get keybinding override for a key sequence
    #[must_use]
    pub fn get_keybinding_override(&self, key: &str) -> Option<&KeyBindingAction> {
        self.behavior.keybinding_overrides.get(key)
    }

    /// Check if a command is disabled
    #[must_use]
    pub fn is_command_disabled(&self, cmd_id: CommandId) -> bool {
        self.behavior.disabled_commands.contains(cmd_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_behavior_modifiers_empty() {
        let behavior = BehaviorModifiers::none();
        assert!(behavior.is_empty());
    }

    #[test]
    fn test_behavior_modifiers_merge() {
        let base = BehaviorModifiers::none()
            .with_keybinding("j", KeyBindingAction::Unbind)
            .with_disabled_command("some_cmd");

        let overlay = BehaviorModifiers::none()
            .with_keybinding("k", KeyBindingAction::Unbind)
            .with_completion_disabled(true);

        let merged = base.merge(&overlay);

        assert!(merged.keybinding_overrides.contains_key("j"));
        assert!(merged.keybinding_overrides.contains_key("k"));
        assert!(merged.disabled_commands.contains("some_cmd"));
        assert!(merged.features.is_completion_disabled());
    }

    #[test]
    fn test_feature_flags_merge() {
        let base = FeatureFlags {
            disable_completion: Some(true),
            disable_auto_indent: None,
            ..Default::default()
        };

        let overlay = FeatureFlags {
            disable_completion: None,
            disable_auto_indent: Some(true),
            ..Default::default()
        };

        let merged = base.merge(&overlay);

        assert_eq!(merged.disable_completion, Some(true)); // From base
        assert_eq!(merged.disable_auto_indent, Some(true)); // From overlay
    }
}
