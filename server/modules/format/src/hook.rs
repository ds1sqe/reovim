//! Format-on-save hook.
//!
//! Handles `BufferWillSave` events by resolving and applying the appropriate
//! formatter before the buffer is written to disk.

use std::sync::Arc;

use {
    reovim_kernel::api::v1::{
        BufferManager, OptionRegistry, OptionScopeId, OptionValue, ServiceRegistry,
        events::kernel::BufferWillSave,
    },
    tracing::{debug, warn},
};

use crate::resolver;

/// Handle a `BufferWillSave` event by formatting the buffer content.
///
/// This function is called synchronously from the event bus. It:
/// 1. Checks the `autoformat` option (skips if disabled)
/// 2. Reads buffer content from the kernel
/// 3. Resolves a formatter (external > LSP > no-op)
/// 4. Writes formatted content back to the buffer
///
/// Formatting failures are logged but never block the save.
pub fn format_on_save(
    event: &BufferWillSave,
    buffers: &Arc<dyn BufferManager>,
    options: &Arc<OptionRegistry>,
    services: &Arc<ServiceRegistry>,
) {
    // Check autoformat option
    if options.get("autoformat", OptionScopeId::Global) == Some(OptionValue::Bool(false)) {
        debug!("format-on-save: autoformat disabled, skipping");
        return;
    }

    // Get buffer
    // Buffer IDs are bounded well within usize range.
    #[allow(clippy::cast_possible_truncation)]
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(event.buffer_id as usize);
    let Some(buf) = buffers.get(buffer_id) else {
        warn!(buffer_id = event.buffer_id, "format-on-save: buffer not found");
        return;
    };

    // Read content
    let content = buf.read().content();
    if content.is_empty() {
        return;
    }

    // Detect filetype from path
    let filetype = resolver::detect_filetype(&event.path);

    // Resolve and format
    let Some(formatted) = resolver::resolve_and_format(&content, &event.path, &filetype, services)
    else {
        debug!(path = %event.path, filetype = %filetype, "format-on-save: no formatter available");
        return;
    };

    // Write back only if content changed
    if formatted != content {
        debug!(path = %event.path, "format-on-save: applying formatted content");
        buf.write().set_content(&formatted);
    }
}

#[cfg(test)]
#[path = "hook_tests.rs"]
mod tests;
