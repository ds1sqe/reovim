//! Profile command handlers: save, load, list.

use {
    crate::{profile::Profile, validate::validate_profile_name},
    reovim_driver_command::{Command, CommandHandler},
    reovim_driver_command_types::{ArgKind, ArgSpec, CommandContext, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::{CommandId, ModuleId},
    std::path::PathBuf,
};

const PROFILES_MODULE: ModuleId = ModuleId::new("profiles");

// ============================================================================
// ProfileSaveCommand
// ============================================================================

/// Save current option overrides to a named profile.
pub struct ProfileSaveCommand {
    profiles_dir: PathBuf,
}

impl ProfileSaveCommand {
    /// Create a new save command with the given profiles directory.
    #[must_use]
    pub const fn new(profiles_dir: PathBuf) -> Self {
        Self { profiles_dir }
    }
}

impl Command for ProfileSaveCommand {
    fn id(&self) -> CommandId {
        CommandId::new(PROFILES_MODULE, "profile-save")
    }

    fn description(&self) -> &'static str {
        "Save current options to a named profile"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::required(
            "name",
            ArgKind::Rest,
            "Profile name",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["profile-save"]
    }
}

impl CommandHandler for ProfileSaveCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(name) = args.string("name") else {
            return CommandResult::error("profile name required");
        };

        if let Err(e) = validate_profile_name(name) {
            return CommandResult::Error(e);
        }

        let Some(vfs) = args.vfs() else {
            return CommandResult::error("filesystem not available");
        };

        let profile = Profile::from_option_registry(&runtime.kernel().options);
        let toml_content = match profile.to_toml() {
            Ok(s) => s,
            Err(e) => return CommandResult::Error(e),
        };

        if let Err(e) = vfs.create_dir_all(&self.profiles_dir) {
            return CommandResult::Error(format!("failed to create profiles directory: {e}"));
        }

        let path = self.profiles_dir.join(format!("{name}.toml"));
        if let Err(e) = vfs.write_str(&path, &toml_content) {
            return CommandResult::Error(format!("failed to write profile: {e}"));
        }

        CommandResult::Success
    }
}

// ============================================================================
// ProfileLoadCommand
// ============================================================================

/// Load options from a named profile.
pub struct ProfileLoadCommand {
    profiles_dir: PathBuf,
}

impl ProfileLoadCommand {
    /// Create a new load command with the given profiles directory.
    #[must_use]
    pub const fn new(profiles_dir: PathBuf) -> Self {
        Self { profiles_dir }
    }
}

impl Command for ProfileLoadCommand {
    fn id(&self) -> CommandId {
        CommandId::new(PROFILES_MODULE, "profile-load")
    }

    fn description(&self) -> &'static str {
        "Load options from a named profile"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::required(
            "name",
            ArgKind::Rest,
            "Profile name",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["profile-load"]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn complete(&self, partial: &str) -> Vec<String> {
        // Try to list .toml files in profiles dir for tab completion.
        // This is best-effort; no VFS available here, so we use std::fs.
        let Ok(entries) = std::fs::read_dir(&self.profiles_dir) else {
            return vec![];
        };

        entries
            .filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let stem = name.strip_suffix(".toml")?;
                if stem.starts_with(partial) {
                    Some(stem.to_string())
                } else {
                    None
                }
            })
            .collect()
    }
}

impl CommandHandler for ProfileLoadCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(name) = args.string("name") else {
            return CommandResult::error("profile name required");
        };

        if let Err(e) = validate_profile_name(name) {
            return CommandResult::Error(e);
        }

        let Some(vfs) = args.vfs() else {
            return CommandResult::error("filesystem not available");
        };

        let path = self.profiles_dir.join(format!("{name}.toml"));
        let Ok(content) = vfs.read_to_string(&path) else {
            return CommandResult::Error(format!("profile '{name}' not found"));
        };

        let profile = match Profile::from_toml(&content) {
            Ok(p) => p,
            Err(e) => return CommandResult::Error(e),
        };

        let warnings = profile.apply_to_registry(&runtime.kernel().options);
        for w in &warnings {
            tracing::warn!("profile load '{name}': {w}");
        }

        CommandResult::Success
    }
}

// ============================================================================
// ProfileListCommand
// ============================================================================

/// List available profiles.
pub struct ProfileListCommand {
    profiles_dir: PathBuf,
}

impl ProfileListCommand {
    /// Create a new list command with the given profiles directory.
    #[must_use]
    pub const fn new(profiles_dir: PathBuf) -> Self {
        Self { profiles_dir }
    }
}

impl Command for ProfileListCommand {
    fn id(&self) -> CommandId {
        CommandId::new(PROFILES_MODULE, "profile-list")
    }

    fn description(&self) -> &'static str {
        "List available configuration profiles"
    }

    fn names(&self) -> &[&'static str] {
        &["profile-list"]
    }
}

impl CommandHandler for ProfileListCommand {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(vfs) = args.vfs() else {
            return CommandResult::error("filesystem not available");
        };

        if !vfs.exists(&self.profiles_dir) {
            return CommandResult::error("no profiles saved");
        }

        let entries = match vfs.list_dir(&self.profiles_dir) {
            Ok(e) => e,
            Err(e) => {
                return CommandResult::Error(format!("failed to list profiles: {e}"));
            }
        };

        let mut names: Vec<String> = entries
            .iter()
            .filter(|e| e.is_file)
            .filter_map(|e| e.name.strip_suffix(".toml").map(String::from))
            .collect();
        names.sort();

        if names.is_empty() {
            return CommandResult::error("no profiles saved");
        }

        // Output via tracing::info — the runner captures this for display
        tracing::info!("profiles: {}", names.join(", "));

        CommandResult::Success
    }
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
