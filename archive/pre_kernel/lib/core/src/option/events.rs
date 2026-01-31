//! Events for the extensible option system.
//!
//! These events enable plugins to:
//! - Register settings sections via `RegisterSettingSection`
//! - Register their own options via `RegisterOption`
//! - React to option changes via `OptionChanged`

use std::borrow::Cow;

use crate::event_bus::Event;

use super::spec::{OptionSpec, OptionValue};

/// Event emitted by plugins to register their options.
///
/// Plugins emit this event during the `subscribe()` phase
/// to register their configurable options with the central
/// option registry.
///
/// # Example
///
/// ```ignore
/// bus.emit(RegisterOption {
///     spec: OptionSpec::new(
///         "plugin.treesitter.highlight_timeout_ms",
///         "Timeout for async highlighting",
///         OptionValue::Integer(100),
///     ),
/// });
/// ```
#[derive(Debug, Clone)]
pub struct RegisterOption {
    /// The option specification to register
    pub spec: OptionSpec,
}

impl RegisterOption {
    /// Create a new `RegisterOption` event.
    #[must_use]
    pub const fn new(spec: OptionSpec) -> Self {
        Self { spec }
    }
}

impl Event for RegisterOption {
    fn priority(&self) -> u32 {
        // High priority - run early during plugin initialization
        50
    }
}

/// Event emitted by plugins to register a settings section with UI metadata.
///
/// Plugins emit this event before `RegisterOption` to define section metadata
/// (display name, description, order). If a section is not registered,
/// options will be grouped under a section derived from their category.
///
/// # Example
///
/// ```ignore
/// bus.emit(RegisterSettingSection {
///     id: "treesitter".into(),
///     display_name: "Treesitter".into(),
///     description: Some("Syntax highlighting settings".into()),
///     order: 100,
/// });
/// ```
#[derive(Debug, Clone)]
pub struct RegisterSettingSection {
    /// Section identifier (e.g., "treesitter", "completion")
    ///
    /// This should match the `section` field in `OptionSpec` for options
    /// that belong to this section.
    pub id: Cow<'static, str>,

    /// Display name for the section header in the settings menu
    pub display_name: Cow<'static, str>,

    /// Optional description for the section
    pub description: Option<Cow<'static, str>>,

    /// Display order (lower = earlier, default 100)
    ///
    /// Core sections use low values (0-50), plugin sections use 100+.
    pub order: u32,
}

impl RegisterSettingSection {
    /// Create a new `RegisterSettingSection` event.
    #[must_use]
    pub fn new(
        id: impl Into<Cow<'static, str>>,
        display_name: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            description: None,
            order: 100,
        }
    }

    /// Set the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the display order.
    #[must_use]
    pub const fn with_order(mut self, order: u32) -> Self {
        self.order = order;
        self
    }
}

impl Event for RegisterSettingSection {
    fn priority(&self) -> u32 {
        // Run before RegisterOption (50) so sections exist first
        40
    }
}

/// Source of an option change.
///
/// Identifies how an option was changed, which can be useful
/// for plugins that want to react differently based on the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeSource {
    /// User typed a `:set` command
    UserCommand,

    /// Loaded from a profile file
    ProfileLoad,

    /// Plugin changed programmatically
    Plugin,

    /// Changed via the settings menu UI
    SettingsMenu,

    /// Changed via RPC (server mode)
    Rpc,

    /// Default value applied during initialization
    Default,
}

impl ChangeSource {
    /// Check if this change was initiated by the user.
    #[must_use]
    pub const fn is_user_initiated(self) -> bool {
        matches!(self, Self::UserCommand | Self::SettingsMenu | Self::Rpc)
    }
}

/// Event emitted when an option value changes.
///
/// Plugins can subscribe to this event to react to option
/// changes, whether from user commands, profile loading,
/// or programmatic changes.
///
/// # Example
///
/// ```ignore
/// bus.subscribe::<OptionChanged, _>(100, move |event, ctx| {
///     if event.name == "plugin.treesitter.highlight_timeout_ms" {
///         // Update internal state with new value
///         if let Some(timeout) = event.new_value.as_int() {
///             state.set_timeout(timeout as u64);
///         }
///         ctx.request_render();
///     }
///     EventResult::Handled
/// });
/// ```
#[derive(Debug, Clone)]
pub struct OptionChanged {
    /// Full option name that changed
    pub name: String,

    /// Previous value (before the change)
    pub old_value: OptionValue,

    /// New value (after the change)
    pub new_value: OptionValue,

    /// Source of the change
    pub source: ChangeSource,
}

impl OptionChanged {
    /// Create a new `OptionChanged` event.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        old_value: OptionValue,
        new_value: OptionValue,
        source: ChangeSource,
    ) -> Self {
        Self {
            name: name.into(),
            old_value,
            new_value,
            source,
        }
    }

    /// Check if the new value is different from the old value.
    #[must_use]
    pub fn value_changed(&self) -> bool {
        self.old_value != self.new_value
    }

    /// Get the new value as a boolean, if applicable.
    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        self.new_value.as_bool()
    }

    /// Get the new value as an integer, if applicable.
    #[must_use]
    pub const fn as_int(&self) -> Option<i64> {
        self.new_value.as_int()
    }

    /// Get the new value as a string, if applicable.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        self.new_value.as_str()
    }
}

impl Event for OptionChanged {
    fn priority(&self) -> u32 {
        // Standard priority for notifications
        100
    }
}

/// Event emitted to query an option value.
///
/// This event is used for the `:set option?` command
/// to query the current value of an option.
#[derive(Debug, Clone)]
pub struct QueryOption {
    /// Option name to query
    pub name: String,
}

impl QueryOption {
    /// Create a new `QueryOption` event.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Event for QueryOption {
    fn priority(&self) -> u32 {
        100
    }
}

/// Event emitted to reset an option to its default value.
///
/// This event is used for the `:set option&` command.
#[derive(Debug, Clone)]
pub struct ResetOption {
    /// Option name to reset
    pub name: String,
}

impl ResetOption {
    /// Create a new `ResetOption` event.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Event for ResetOption {
    fn priority(&self) -> u32 {
        100
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_source_user_initiated() {
        assert!(ChangeSource::UserCommand.is_user_initiated());
        assert!(ChangeSource::SettingsMenu.is_user_initiated());
        assert!(ChangeSource::Rpc.is_user_initiated());
        assert!(!ChangeSource::ProfileLoad.is_user_initiated());
        assert!(!ChangeSource::Plugin.is_user_initiated());
        assert!(!ChangeSource::Default.is_user_initiated());
    }

    #[test]
    fn test_option_changed_value_comparison() {
        let event = OptionChanged::new(
            "test",
            OptionValue::Bool(false),
            OptionValue::Bool(true),
            ChangeSource::UserCommand,
        );
        assert!(event.value_changed());

        let event2 = OptionChanged::new(
            "test",
            OptionValue::Bool(true),
            OptionValue::Bool(true),
            ChangeSource::UserCommand,
        );
        assert!(!event2.value_changed());
    }
}
