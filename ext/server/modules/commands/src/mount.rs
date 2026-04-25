//! `:mount` ex-command — manually mount a content codec on the active buffer.
//!
//! Bridges [`CodecSessionState::mount_codec`] to the user-typed
//! `:mount <content_type>` ex-command. Resolves a small shorthand table
//! (`hex`, `elf`, `rlib`, `zip`, `pdf`) to canonical content-type strings
//! before consulting the factory store, ensures the buffer's inode has
//! canonical bytes (loading them via VFS if necessary), and registers the
//! mount through the existing session-state API.
//!
//! Phase 5 always mounts in [`MountMode::Summary`]. Structural editing
//! lands in a follow-on phase.

use std::path::Path;

use {
    reovim_content_codec::{ContentCodecFactoryStore, ContentType, MountCodecError, MountMode},
    reovim_content_codec_text::CodecSessionState,
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_text_session::{BufferApi, ExtensionApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, ModuleId},
    reovim_subsys_vfs::VfsInstance,
};

const COMMANDS_MODULE: ModuleId = ModuleId::new("commands");

/// Aliases users can type in place of full content-type strings. The
/// table is walked linearly; stored as a constant so the namespace stays
/// closed (no runtime registration), per the round-1 flight-director
/// `:mount` shorthand finding.
const SHORTHANDS: &[(&str, &str)] = &[
    ("hex", "binary/raw"),
    ("elf", "binary/elf"),
    ("rlib", "binary/rlib"),
    ("zip", "binary/zip"),
    ("pdf", "binary/pdf"),
];

fn resolve_content_type(arg: &str) -> ContentType {
    for (alias, full) in SHORTHANDS {
        if *alias == arg {
            return ContentType::new(*full);
        }
    }
    ContentType::new(arg)
}

/// `:mount <content_type>` — register a codec mount on the active buffer.
#[derive(Debug, Clone, Copy)]
pub struct MountCommand;

impl Command for MountCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "mount")
    }

    fn description(&self) -> &'static str {
        "Mount a content codec on the active buffer"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::required(
            "content_type",
            ArgKind::String,
            "Content type or shorthand (e.g. binary/elf, hex, rlib)",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["mount"]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for MountCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        let Some(content_type_arg) = ctx.string("content_type") else {
            return CommandResult::Error("missing content_type argument".to_string());
        };
        let content_type = resolve_content_type(content_type_arg);

        let Some(buffer_id) = runtime.active_buffer() else {
            return CommandResult::Error("no active buffer".to_string());
        };

        let Some(factories) = runtime.kernel().services.get::<ContentCodecFactoryStore>() else {
            return CommandResult::Error("no factory store".to_string());
        };

        // First mount attempt. If the inode hasn't been populated yet,
        // `mount_codec` returns `NoCanonicalBytes`; we recover by reading
        // the file via VFS, priming the source, and retrying once. This
        // try-recover-retry shape avoids depending on
        // `CodecSessionState::contains` (which checks decoded metadata,
        // not inode presence).
        let first = {
            let Some(state) = runtime.shared_ext_mut::<CodecSessionState>() else {
                return CommandResult::Error("no codec session state".to_string());
            };
            state.mount_codec(
                &factories,
                buffer_id,
                &content_type,
                "default".to_string(),
                MountMode::Summary,
            )
        };
        match first {
            Ok(_) => return CommandResult::Success,
            Err(MountCodecError::NoCodec { content_type }) => {
                return CommandResult::Error(format!(
                    "no codec registered for content type {content_type}"
                ));
            }
            Err(MountCodecError::Mount(e)) => {
                return CommandResult::Error(format!("mount failed: {e}"));
            }
            Err(MountCodecError::NoCanonicalBytes) => {
                // Fall through to VFS-load + retry below.
            }
        }

        // Recovery path: load canonical bytes from VFS, prime the
        // source, retry the mount.
        let Some(file_path) = runtime.buffer_file_path(buffer_id) else {
            return CommandResult::Error("cannot mount on a buffer with no file path".to_string());
        };
        let Some(vfs) = runtime.kernel().services.get::<VfsInstance>() else {
            return CommandResult::Error("no VFS available".to_string());
        };
        let bytes = match vfs.driver().read(Path::new(&file_path)) {
            Ok(b) => b,
            Err(e) => {
                return CommandResult::Error(format!("VFS read failed for {file_path}: {e}"));
            }
        };
        let Some(state) = runtime.shared_ext_mut::<CodecSessionState>() else {
            return CommandResult::Error("no codec session state".to_string());
        };
        state.set_source(buffer_id, bytes);
        match state.mount_codec(
            &factories,
            buffer_id,
            &content_type,
            "default".to_string(),
            MountMode::Summary,
        ) {
            Ok(_) => CommandResult::Success,
            Err(MountCodecError::NoCanonicalBytes) => {
                CommandResult::Error("buffer has no canonical bytes after VFS load".to_string())
            }
            Err(MountCodecError::NoCodec { content_type }) => {
                CommandResult::Error(format!("no codec registered for content type {content_type}"))
            }
            Err(MountCodecError::Mount(e)) => CommandResult::Error(format!("mount failed: {e}")),
        }
    }
}
