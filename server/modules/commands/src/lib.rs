#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Ex-commands module - POLICY.
//!
//! This module implements the standard ex-commands (colon commands):
//! - `:q` / `:quit` - Quit the editor
//! - `:w` / `:write` - Write the buffer to disk
//! - `:wq` - Write and quit
//! - `:e` / `:edit` - Open a file
//! - `:set` - Set editor options
//! - `:colorscheme` - Switch color theme
//! - `:detach` - Detach from server
//! - `:servers` - List server instances
//! - `:kill-server` - Kill the server

mod colorscheme;
mod edit;
mod quit;
mod session;
mod set;
mod write;

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub use {
    colorscheme::ColorschemeCommand,
    edit::EditCommand,
    quit::QuitCommand,
    session::{DetachCommand, KillServerCommand, ServersCommand},
    set::SetCommand,
    write::{WriteCommand, WriteQuitCommand},
};

/// Returns all command handlers provided by this module.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    let mut cmds: Vec<Box<dyn CommandHandler>> = vec![
        Box::new(EditCommand),
        Box::new(QuitCommand),
        Box::new(WriteCommand),
        Box::new(WriteQuitCommand),
        Box::new(ColorschemeCommand),
        Box::new(SetCommand),
    ];
    cmds.extend(session::command_handlers());
    cmds
}

// ============================================================================
// Module trait implementation
// ============================================================================

/// Commands module instance.
pub struct CommandsModule;

impl CommandsModule {
    /// Create a new commands module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CommandsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CommandsModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("commands")
    }

    fn name(&self) -> &'static str {
        "Ex-Commands"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register command handlers (#547)
        let store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in command_handlers() {
            store.add(handler);
        }

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CommandsModule);

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_command::Command};

    // ========================================================================
    // command_handlers() function tests
    // ========================================================================

    #[test]
    fn test_command_handlers_count() {
        let cmds = command_handlers();
        assert_eq!(cmds.len(), 9); // 6 base + 3 session commands
    }

    #[test]
    fn test_command_handlers_contains_all_ids() {
        let cmds = command_handlers();
        let ids: Vec<_> = cmds.iter().map(|c| c.id().name()).collect();
        assert!(ids.contains(&"edit"));
        assert!(ids.contains(&"quit"));
        assert!(ids.contains(&"write"));
        assert!(ids.contains(&"write-quit"));
        assert!(ids.contains(&"colorscheme"));
        assert!(ids.contains(&"set"));
        assert!(ids.contains(&"detach"));
        assert!(ids.contains(&"servers"));
        assert!(ids.contains(&"kill-server"));
    }

    #[test]
    fn test_command_handlers_unique_ids() {
        let cmds = command_handlers();
        let ids: Vec<_> = cmds.iter().map(|c| c.id()).collect();
        for (i, id) in ids.iter().enumerate() {
            for (j, other) in ids.iter().enumerate() {
                if i != j {
                    assert_ne!(id, other, "duplicate command id: {}", id.name());
                }
            }
        }
    }

    // ========================================================================
    // Command type re-export tests
    // ========================================================================

    #[test]
    fn test_quit_command_names() {
        let cmd = QuitCommand;
        let names = cmd.names();
        assert!(names.contains(&"q"));
        assert!(names.contains(&"quit"));
    }

    #[test]
    fn test_write_command_names() {
        let cmd = WriteCommand;
        let names = cmd.names();
        assert!(names.contains(&"w"));
        assert!(names.contains(&"write"));
    }

    #[test]
    fn test_session_commands_re_export() {
        let detach = DetachCommand;
        let servers = ServersCommand;
        let kill = KillServerCommand;
        assert_eq!(detach.id().name(), "detach");
        assert_eq!(servers.id().name(), "servers");
        assert_eq!(kill.id().name(), "kill-server");
    }

    // ========================================================================
    // CommandsModule tests
    // ========================================================================

    #[test]
    fn test_module_id() {
        let module = CommandsModule;
        assert_eq!(module.id().as_str(), "commands");
    }

    #[test]
    fn test_module_name() {
        let module = CommandsModule;
        assert_eq!(module.name(), "Ex-Commands");
    }

    #[test]
    fn test_module_version() {
        let module = CommandsModule;
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_module_new() {
        let module = CommandsModule::new();
        assert_eq!(module.id().as_str(), "commands");
    }

    #[test]
    fn test_module_default() {
        fn accepts_default<T: Default>(val: T) -> T {
            drop(val);
            T::default()
        }
        let module = accepts_default(CommandsModule);
        assert_eq!(module.id().as_str(), "commands");
    }

    #[test]
    fn test_module_exit() {
        let mut module = CommandsModule::new();
        let result = module.exit();
        assert!(result.is_ok());
    }

    #[test]
    fn test_module_init_registers_commands() {
        use reovim_kernel::api::v1::ModuleContext;

        let mut module = CommandsModule::new();
        let ctx = ModuleContext::default();

        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);

        // Verify commands were registered in the CommandHandlerStore
        let store = ctx.services.get::<CommandHandlerStore>().unwrap();
        assert_eq!(store.len(), 9);
    }

    #[test]
    fn test_module_init_idempotent_stores_accumulate() {
        use reovim_kernel::api::v1::ModuleContext;

        let mut module = CommandsModule::new();
        let ctx = ModuleContext::default();

        // Init twice - handlers accumulate in the store
        module.init(&ctx);
        module.init(&ctx);

        let store = ctx.services.get::<CommandHandlerStore>().unwrap();
        assert_eq!(store.len(), 18); // 9 + 9
    }
}
