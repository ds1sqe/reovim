//! `:codecs` ex-command — list available codec factories and active mounts.
//!
//! `:codecs` is a discovery command. It walks the
//! [`ContentCodecFactoryStore`] for registered factory names and
//! supported content types, plus [`CodecSessionState::list_mounts`] for
//! the active mounts on the current buffer, and emits the listing via
//! `tracing::info!` on the stable target `codecs`.
//!
//! User-visible TUI display is deferred — `CommandResult` and
//! `RuntimeSignal` have no notification variant in the current kernel.
//! The `reovim cli log-tail --target codecs` path picks up the listing
//! today.

use {
    reovim_content_codec::ContentCodecFactoryStore,
    reovim_content_codec_text::{CodecSessionState, MountInfo},
    reovim_driver_command::{ArgSpec, Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_text_session::{BufferApi, ExtensionApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

const COMMANDS_MODULE: ModuleId = ModuleId::new("commands");

/// Stable tracing target for `:codecs` output. Downstream consumers
/// (e.g. `reovim cli log-tail --target codecs`) depend on this string;
/// changing it is a user-visible breaking change.
const CODECS_TRACING_TARGET: &str = "codecs";

/// Format the codec listing into a `Vec<String>` of human-readable lines.
///
/// Two sections: "Available codecs:" and "Active mounts:". A missing
/// or empty `factories` argument both render as a `(none)` placeholder
/// — production code never distinguishes the two cases at the user
/// surface, and neither does this formatter.
pub fn format_codecs_listing(
    factories: Option<&ContentCodecFactoryStore>,
    mounts: &[MountInfo],
) -> Vec<String> {
    let mut lines = Vec::new();

    lines.push("Available codecs:".to_string());
    let codec_lines = factories.map_or_else(Vec::new, |store| {
        store
            .available()
            .into_iter()
            .map(|(name, types)| format!("  {name}: {}", types.join(" ")))
            .collect::<Vec<_>>()
    });
    if codec_lines.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        lines.extend(codec_lines);
    }

    lines.push("Active mounts:".to_string());
    if mounts.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for m in mounts {
            lines.push(format!(
                "  mount_id={} view={} mode={:?} content_valid={}",
                m.mount_id.as_usize(),
                m.view_name,
                m.mode,
                m.content_valid,
            ));
        }
    }

    lines
}

/// `:codecs` — list available codecs and active mounts on the active
/// buffer. Output is emitted via `tracing::info!(target: "codecs")`.
#[derive(Debug, Clone, Copy)]
pub struct CodecsCommand;

impl Command for CodecsCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "codecs")
    }

    fn description(&self) -> &'static str {
        "List available codecs and active mounts"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![]
    }

    fn names(&self) -> &[&'static str] {
        &["codecs"]
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for CodecsCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
        let factories = runtime.kernel().services.get::<ContentCodecFactoryStore>();
        let mounts = runtime
            .active_buffer()
            .and_then(|buffer_id| {
                runtime
                    .shared_ext::<CodecSessionState>()
                    .map(|s| s.list_mounts(buffer_id))
            })
            .unwrap_or_default();

        for line in format_codecs_listing(factories.as_deref(), &mounts) {
            tracing::info!(target: CODECS_TRACING_TARGET, "{line}");
        }

        CommandResult::Success
    }
}
