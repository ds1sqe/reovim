//! Configuration profile system
//!
//! Provides TOML-based configuration profiles that can be loaded, saved,
//! and switched at runtime. Profiles control theme, keybindings, editor
//! options, and other settings.
//!
//! # File Structure
//!
//! ```text
//! ~/.config/reovim/
//!   config.toml              # Global config + default profile name
//!   profiles/
//!     default.toml           # Default profile
//!     <custom>.toml          # User profiles
//! ```

mod loader;
mod profile;
mod schema;

pub use {
    loader::{ConfigError, get_config_dir, load_toml, save_toml},
    profile::ProfileManager,
    schema::{
        CompletionConfig, EditorConfig, GlobalConfig, KeybindingValue, KeybindingsConfig,
        ProfileConfig, ProfileMeta, WindowConfig,
    },
};
