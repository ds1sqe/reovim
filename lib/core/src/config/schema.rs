//! TOML schema definitions for configuration profiles

use {
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
};

/// Global configuration (config.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Name of the default profile to load
    #[serde(default = "default_profile_name")]
    pub default_profile: String,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            default_profile: default_profile_name(),
        }
    }
}

fn default_profile_name() -> String {
    "default".to_string()
}

/// Complete profile configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileConfig {
    /// Profile metadata
    #[serde(default)]
    pub profile: ProfileMeta,

    /// Editor settings
    #[serde(default)]
    pub editor: EditorConfig,

    /// Custom keybindings
    #[serde(default)]
    pub keybindings: KeybindingsConfig,

    /// Completion settings
    #[serde(default)]
    pub completion: CompletionConfig,

    /// Window settings
    #[serde(default)]
    pub window: WindowConfig,

    /// Plugin-specific options
    ///
    /// Structure: `{ "plugin_id": { "option_name": value, ... }, ... }`
    ///
    /// Example TOML:
    /// ```toml
    /// [plugin.treesitter]
    /// highlight_timeout_ms = 100
    /// incremental_parse = true
    ///
    /// [plugin.completion]
    /// auto_trigger = true
    /// ```
    #[serde(default)]
    pub plugin: HashMap<String, toml::Value>,
}

/// Profile metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMeta {
    /// Profile name
    #[serde(default)]
    pub name: String,

    /// Profile description
    #[serde(default)]
    pub description: String,

    /// Schema version for forward compatibility
    #[serde(default = "default_version")]
    pub version: String,
}

impl Default for ProfileMeta {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            description: String::new(),
            version: default_version(),
        }
    }
}

fn default_version() -> String {
    "1".to_string()
}

/// Editor settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Config naturally has many on/off settings
pub struct EditorConfig {
    /// Theme name: dark, light, tokyonight
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Color mode: ansi, 256, truecolor
    #[serde(default = "default_colormode")]
    pub colormode: String,

    /// Show line numbers (:set nu)
    #[serde(default = "default_true")]
    pub number: bool,

    /// Show relative line numbers (:set rnu)
    #[serde(default = "default_true")]
    pub relativenumber: bool,

    /// Show indent guides
    #[serde(default)]
    pub indentguide: bool,

    /// Show scrollbar
    #[serde(default = "default_true")]
    pub scrollbar: bool,

    /// Tab width in spaces
    #[serde(default = "default_tabwidth")]
    pub tabwidth: u8,

    /// Use spaces instead of tabs
    #[serde(default = "default_true")]
    pub expandtab: bool,

    /// Lines to keep above/below cursor
    #[serde(default = "default_scrolloff")]
    pub scrolloff: u16,

    /// Render strategy: `virtual_buffer`, `dirty_region`, `cell_delta`
    #[serde(default = "default_render_strategy")]
    pub render_strategy: String,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            colormode: default_colormode(),
            number: true,
            relativenumber: true,
            indentguide: false,
            scrollbar: true,
            tabwidth: default_tabwidth(),
            expandtab: true,
            scrolloff: default_scrolloff(),
            render_strategy: default_render_strategy(),
        }
    }
}

fn default_render_strategy() -> String {
    "virtual_buffer".to_string()
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_colormode() -> String {
    "truecolor".to_string()
}

const fn default_true() -> bool {
    true
}

const fn default_tabwidth() -> u8 {
    4
}

const fn default_scrolloff() -> u16 {
    5
}

/// Keybindings configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeybindingsConfig {
    /// Key mappings: "mode:key" -> command
    #[serde(flatten)]
    pub mappings: HashMap<String, KeybindingValue>,
}

/// Value for a keybinding
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeybindingValue {
    /// Simple command string
    Command(String),
    /// Extended format with description
    Extended {
        command: String,
        #[serde(default)]
        desc: Option<String>,
    },
}

impl KeybindingValue {
    /// Get the command string
    #[must_use]
    pub fn command(&self) -> &str {
        match self {
            Self::Command(cmd) => cmd,
            Self::Extended { command, .. } => command,
        }
    }

    /// Check if this binding should unset the key
    #[must_use]
    pub fn is_unset(&self) -> bool {
        self.command().is_empty()
    }
}

/// Completion settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionConfig {
    /// Auto-trigger completion while typing
    #[serde(default = "default_true")]
    pub auto_trigger: bool,

    /// Delay before triggering completion (ms)
    #[serde(default = "default_trigger_delay")]
    pub trigger_delay_ms: u32,

    /// Minimum prefix length to trigger
    #[serde(default = "default_min_prefix")]
    pub min_prefix_len: u8,
}

impl Default for CompletionConfig {
    fn default() -> Self {
        Self {
            auto_trigger: true,
            trigger_delay_ms: default_trigger_delay(),
            min_prefix_len: default_min_prefix(),
        }
    }
}

const fn default_trigger_delay() -> u32 {
    50
}

const fn default_min_prefix() -> u8 {
    2
}

/// Window settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    /// Default split direction: horizontal, vertical
    #[serde(default = "default_split")]
    pub default_split: String,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            default_split: default_split(),
        }
    }
}

fn default_split() -> String {
    "vertical".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_profile() {
        let toml = r#"
[profile]
name = "test"
"#;
        let config: ProfileConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.profile.name, "test");
        assert_eq!(config.editor.theme, "dark");
        assert!(config.editor.number);
    }

    #[test]
    fn test_parse_full_profile() {
        let toml = r#"
[profile]
name = "custom"
description = "My custom profile"

[editor]
theme = "tokyonight"
colormode = "256"
number = false
relativenumber = false
tabwidth = 2

[keybindings]
"n:jj" = "enter_normal_mode"
"n:<C-s>" = { command = "save_buffer", desc = "Save file" }
"n:s" = ""
"#;
        let config: ProfileConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.profile.name, "custom");
        assert_eq!(config.editor.theme, "tokyonight");
        assert!(!config.editor.number);
        assert_eq!(config.editor.tabwidth, 2);
        assert_eq!(config.keybindings.mappings.len(), 3);

        let jj = config.keybindings.mappings.get("n:jj").unwrap();
        assert_eq!(jj.command(), "enter_normal_mode");

        let s = config.keybindings.mappings.get("n:s").unwrap();
        assert!(s.is_unset());
    }

    #[test]
    fn test_serialize_profile() {
        let config = ProfileConfig::default();
        let toml = toml::to_string_pretty(&config).unwrap();
        assert!(toml.contains("[profile]"));
        assert!(toml.contains("[editor]"));
    }
}
