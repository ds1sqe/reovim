//! Core primitives for the kernel.
//!
//! Linux equivalent: Core kernel functionality
//!
//! This module provides kernel-owned abstractions:
//!
//! - **Mode**: Mode identity and mode stack
//! - **Option**: Editor option registry
//! - **Config**: Configuration system
//!
//! Text-specific algorithms (`Motion`, `TextObject`, `Register`) live in
//! `reovim-domain-text`. Mark types (`Mark`, `MarkBank`, `SpecialMark`) live
//! in `reovim-driver-session`. The kernel has zero domain dependencies.

mod config;
mod mode;
mod option;

// Re-export option types
pub use option::{
    ConstraintError, OptionConstraint, OptionError, OptionRegistry, OptionScope, OptionScopeId,
    OptionSpec, OptionValue, SetResult,
};

// Re-export config types
pub use config::{Config, ConfigError, ConfigPaths, ConfigValue};

// Re-export mode types
pub use mode::{CommandId, CursorStyle, Mode, ModeId, ModeStack};

#[cfg(test)]
mod tests;
