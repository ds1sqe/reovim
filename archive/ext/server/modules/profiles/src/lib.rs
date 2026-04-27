#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Configuration profiles module - POLICY.
//!
//! Implements `:profile-save`, `:profile-load`, `:profile-list` commands
//! for saving and loading named editor option snapshots as TOML files.

mod commands;
mod profile;
mod validate;

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    std::path::PathBuf,
};

pub use {
    commands::{ProfileListCommand, ProfileLoadCommand, ProfileSaveCommand},
    profile::{Profile, ProfileMetadata, ProfileOption},
    validate::validate_profile_name,
};

/// Returns all command handlers provided by this module.
#[must_use]
pub fn command_handlers(profiles_dir: PathBuf) -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(ProfileSaveCommand::new(profiles_dir.clone())),
        Box::new(ProfileLoadCommand::new(profiles_dir.clone())),
        Box::new(ProfileListCommand::new(profiles_dir)),
    ]
}

/// Configuration profiles module instance.
pub struct ProfilesModule;

impl ProfilesModule {
    /// Create a new profiles module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ProfilesModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ProfilesModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("profiles")
    }

    fn name(&self) -> &'static str {
        "Configuration Profiles"
    }

    fn version(&self) -> Version {
        Version::new(0, 10, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let profiles_dir = ctx.data_dir.clone();
        let store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in command_handlers(profiles_dir) {
            store.add(handler);
        }
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(ProfilesModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
