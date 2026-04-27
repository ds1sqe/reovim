//! `:umount` ex-command — remove a content-codec mount from the active buffer.
//!
//! Bridges [`CodecSessionState::unmount_codec`] to the user-typed
//! `:umount [mount_id]` ex-command. With no argument the most-recently
//! registered mount on the active buffer is removed; with an explicit
//! `mount_id` argument the named mount is removed regardless of buffer.
//! Symmetric to the `:mount` command landed in Phase 5.

use {
    reovim_content_codec::{MountId, UmountCodecError},
    reovim_content_codec_text::CodecSessionState,
    reovim_driver_command::{
        ArgKind, ArgSpec, ArgValue, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_text_session::{BufferApi, ExtensionApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

const COMMANDS_MODULE: ModuleId = ModuleId::new("commands");

/// `:umount [mount_id]` — remove a codec mount from the active buffer.
#[derive(Debug, Clone, Copy)]
pub struct UmountCommand;

impl Command for UmountCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "umount")
    }

    fn description(&self) -> &'static str {
        "Unmount a content codec from the active buffer"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "mount_id",
            ArgKind::Count,
            "Mount ID to remove (default: most-recent on active buffer)",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["umount", "unmount"]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for UmountCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        let Some(buffer_id) = runtime.active_buffer() else {
            return CommandResult::Error("no active buffer".to_string());
        };

        // Resolve the target mount_id. Either an explicit argument (with a
        // zero-guard that prevents `MountId::from_raw(0)` from panicking) or
        // the most-recent mount on the active buffer.
        let explicit = match ctx.get("mount_id") {
            Some(ArgValue::Count(n)) => Some(*n),
            _ => None,
        };
        let target_id = if let Some(raw) = explicit {
            if raw == 0 {
                return CommandResult::Error("mount_id must be non-zero".to_string());
            }
            MountId::from_raw(raw)
        } else {
            let Some(state) = runtime.shared_ext::<CodecSessionState>() else {
                return CommandResult::Error("no codec session state".to_string());
            };
            // `list_mounts` returns a snapshot drawn from a `HashMap`, so
            // iteration order is arbitrary. `MountId`s are allocated
            // monotonically by the inode table, so the maximum id is the
            // most-recently registered mount on the buffer.
            let Some(latest) = state
                .list_mounts(buffer_id)
                .iter()
                .max_by_key(|m| m.mount_id)
                .map(|m| m.mount_id)
            else {
                return CommandResult::Error("no mounts on active buffer".to_string());
            };
            latest
        };

        let Some(state) = runtime.shared_ext_mut::<CodecSessionState>() else {
            return CommandResult::Error("no codec session state".to_string());
        };
        match state.unmount_codec(target_id) {
            Ok(()) => CommandResult::Success,
            Err(UmountCodecError::MountNotFound) => {
                CommandResult::Error("mount id not found".to_string())
            }
            Err(UmountCodecError::Umount(e)) => CommandResult::Error(format!("umount failed: {e}")),
        }
    }
}
