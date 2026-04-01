//! Command handlers for the format module.
//!
//! - `FormatDocument`: Format the entire current document.
//! - `FormatSelection`: Format the selected range (falls back to full format).

use {
    reovim_driver_command::CommandHandler,
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_session::{BufferApi, SessionRuntime},
    reovim_kernel::api::v1::CommandId,
    tracing::debug,
};

use crate::{ids, resolver};

// ============================================================================
// FormatDocument command
// ============================================================================

/// Format the current document.
pub struct FormatDocument;

impl reovim_driver_command::Command for FormatDocument {
    fn id(&self) -> CommandId {
        ids::FORMAT_DOCUMENT
    }

    fn description(&self) -> &'static str {
        "Format current document"
    }
}

impl CommandHandler for FormatDocument {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(buf_id) = runtime.active_buffer() else {
            debug!("format: no active buffer");
            return CommandResult::Success;
        };
        if runtime
            .buffer_capabilities(buf_id)
            .is_some_and(|caps| {
                !caps.contains(reovim_kernel::api::v1::BufferCapabilities::CONTENT_MATERIALIZABLE)
            })
        {
            debug!("format: skipping non-materializable buffer (file too large)");
            return CommandResult::Success;
        }
        let Some(file_path) = runtime.buffer_file_path(buf_id) else {
            debug!("format: no file path for buffer");
            return CommandResult::Success;
        };
        let Some(content) = runtime.buffer_content(buf_id) else {
            debug!("format: could not read buffer content");
            return CommandResult::Success;
        };

        let filetype = resolver::detect_filetype(&file_path);
        let services = runtime.kernel().services.clone();

        let Some(formatted) =
            resolver::resolve_and_format(&content, &file_path, &filetype, &services)
        else {
            debug!(filetype = %filetype, "format: no formatter available");
            return CommandResult::Success;
        };

        if formatted != content {
            runtime.replace_content(buf_id, &formatted);
        }

        CommandResult::Success
    }
}

// ============================================================================
// FormatSelection command
// ============================================================================

/// Format the selected range.
pub struct FormatSelection;

impl reovim_driver_command::Command for FormatSelection {
    fn id(&self) -> CommandId {
        ids::FORMAT_SELECTION
    }

    fn description(&self) -> &'static str {
        "Format selected range"
    }
}

impl CommandHandler for FormatSelection {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // For now, delegate to full document format.
        // Range formatting requires visual selection tracking and is deferred.
        let Some(buf_id) = runtime.active_buffer() else {
            return CommandResult::Success;
        };
        if runtime
            .buffer_capabilities(buf_id)
            .is_some_and(|caps| {
                !caps.contains(reovim_kernel::api::v1::BufferCapabilities::CONTENT_MATERIALIZABLE)
            })
        {
            return CommandResult::Success;
        }
        let Some(file_path) = runtime.buffer_file_path(buf_id) else {
            return CommandResult::Success;
        };
        let Some(content) = runtime.buffer_content(buf_id) else {
            return CommandResult::Success;
        };

        let filetype = resolver::detect_filetype(&file_path);
        let services = runtime.kernel().services.clone();

        let Some(formatted) =
            resolver::resolve_and_format(&content, &file_path, &filetype, &services)
        else {
            return CommandResult::Success;
        };

        if formatted != content {
            runtime.replace_content(buf_id, &formatted);
        }

        CommandResult::Success
    }
}

/// Collect all command handlers for registration.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(FormatDocument), Box::new(FormatSelection)]
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
