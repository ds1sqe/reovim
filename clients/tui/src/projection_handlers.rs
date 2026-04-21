//! Projection update handlers for TUI client.
//!
//! Dispatches `ProjectionUpdated` notifications by tag. Only `text.mode` has a
//! live server-side emitter today (post-#753 Phase V.B). The remaining tags
//! (`text.cursor`, `text.selection`, `text.buffer_modified`, `options.changed`)
//! have stub handlers that log at trace level and return `Ok(())`.
//!
//! When server-side emitters land, replace the stub bodies with real decoders.
//! The datum encoding for each tag is domain-specific and lives server-side in
//! `ext/server/drivers/text-session/src/text_domain.rs`.

use reovim_protocol::v3::ProjectionUpdatedPayload;

use crate::{grpc_client::TuiGrpcError, notification_handler::NotificationContext};

#[cfg(test)]
use crate::TuiCoreState;

/// Dispatch a `ProjectionUpdated` payload to the per-tag handler.
///
/// Dispatches to the appropriate `handle_*_projection` function based on
/// `p.tag`. Unknown tags are silently traced and ignored — no panic, no error.
///
/// # Errors
///
/// Returns an error if the live `text.mode` handler fails to decode UTF-8.
pub async fn handle_projection_updated<C: NotificationContext>(
    ctx: &mut C,
    p: ProjectionUpdatedPayload,
) -> Result<(), TuiGrpcError> {
    match p.tag.as_str() {
        "text.mode" => handle_mode_projection(ctx, &p),
        "text.cursor" => handle_cursor_projection(ctx, &p),
        "text.selection" => handle_selection_projection(ctx, &p),
        "text.buffer_modified" => handle_buffer_modified_projection(ctx, &p),
        "options.changed" => handle_options_changed_projection(ctx, &p),
        _ => {
            tracing::trace!(tag = %p.tag, "Unknown projection tag, ignoring");
        }
    }
    Ok(())
}

/// Handle `text.mode` projection.
///
/// Datum encoding: raw UTF-8 bytes (`mode_name.as_bytes()`).
/// Emitter: `ext/server/drivers/text-session/src/text_domain.rs`.
///
/// Updates `TuiCoreState.mode_name` for the local client.
fn handle_mode_projection<C: NotificationContext>(ctx: &mut C, p: &ProjectionUpdatedPayload) {
    let bytes = match p.datum.as_ref() {
        Some(d) => &d.content,
        None => {
            tracing::trace!(tag = %p.tag, "text.mode projection missing datum");
            return;
        }
    };

    match std::str::from_utf8(bytes) {
        Ok(mode_name) => {
            let state = ctx.state_mut();
            let is_local = p.client_id == 0 || p.client_id == state.my_client_id;
            if is_local {
                state.mode_name.clone_from(&mode_name.to_string());
                state.mode_display.clone_from(&mode_name.to_string());
                let is_insert = mode_name.contains("insert") || mode_name.contains("INSERT");
                state.set_insert_mode(is_insert);
                tracing::trace!(mode = mode_name, "text.mode projection applied");
            } else if let Some(remote) = state.other_clients.get_mut(&p.client_id) {
                remote.mode = mode_name.to_string();
            }
        }
        Err(e) => {
            tracing::warn!(tag = %p.tag, error = %e, "text.mode datum is not valid UTF-8");
        }
    }
}

/// Handle `text.cursor` projection (stub — server-side emitter not yet wired).
///
// TODO(#757): server-side emitter not yet wired
fn handle_cursor_projection<C: NotificationContext>(_ctx: &mut C, p: &ProjectionUpdatedPayload) {
    tracing::trace!(tag = %p.tag, "text.cursor projection decode not yet implemented");
}

/// Handle `text.selection` projection (stub — server-side emitter not yet wired).
///
// TODO(#757): server-side emitter not yet wired
fn handle_selection_projection<C: NotificationContext>(_ctx: &mut C, p: &ProjectionUpdatedPayload) {
    tracing::trace!(tag = %p.tag, "text.selection projection decode not yet implemented");
}

/// Handle `text.buffer_modified` projection (stub — server-side emitter not yet wired).
///
// TODO(#757): server-side emitter not yet wired
fn handle_buffer_modified_projection<C: NotificationContext>(
    _ctx: &mut C,
    p: &ProjectionUpdatedPayload,
) {
    tracing::trace!(tag = %p.tag, "text.buffer_modified projection decode not yet implemented");
}

/// Handle `options.changed` projection (stub — server-side emitter not yet wired).
///
// TODO(#757): server-side emitter not yet wired
fn handle_options_changed_projection<C: NotificationContext>(
    _ctx: &mut C,
    p: &ProjectionUpdatedPayload,
) {
    tracing::trace!(tag = %p.tag, "options.changed projection decode not yet implemented");
}

// =============================================================================
// Access helpers for tests
// =============================================================================

/// Mode name from `text.mode` datum bytes (test helper).
///
/// Returns `None` if datum is absent or not valid UTF-8.
#[cfg(test)]
pub(crate) fn decode_mode_datum(p: &ProjectionUpdatedPayload) -> Option<String> {
    let bytes = p.datum.as_ref()?.content.as_slice();
    std::str::from_utf8(bytes).ok().map(|s| s.to_string())
}

/// Apply mode update directly to state (test helper).
#[cfg(test)]
pub(crate) fn apply_mode_to_state(state: &mut TuiCoreState, mode_name: &str) {
    state.mode_name = mode_name.to_string();
    state.mode_display = mode_name.to_string();
    let is_insert = mode_name.contains("insert") || mode_name.contains("INSERT");
    state.set_insert_mode(is_insert);
}

#[cfg(test)]
#[path = "projection_handlers_tests.rs"]
mod tests;
