//! `InputService` gRPC implementation.
//!
//! Provides key input processing for v2 protocol clients.
//!
//! # Key Resolution
//!
//! When modules are loaded (via `Server::with_session_factory`), keys are resolved
//! through the full resolver system:
//! 1. Parse vim notation keys (e.g., `iHello<Esc>`, `<C-w>h`)
//! 2. For each key, call `SessionState::resolve_key_for_client()` which uses:
//!    - `ResolverRegistry` to find the mode's resolver
//!    - `KeymapRegistry` for keybinding lookup
//!    - `CommandRegistry` for command execution
//! 3. Handle the `ResolveResult` (execute command, insert char, mode transition, etc.)
//! 4. Emit notifications for state changes (mode, cursor, buffer modifications)
//!
//! When modules are NOT loaded (empty registries), the service falls back to
//! basic character insertion for insert-mode-like behavior.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use std::{collections::HashMap, sync::Arc};

use {
    parking_lot::Mutex,
    reovim_protocol::v2::{
        KeyStatus, SendKeysRequest, SendKeysResponse, input_service_server::InputService,
        notification,
    },
    reovim_subsys_input::KeySequence,
    reovim_subsys_session::{bridges::BridgeRegistry, change_set::ChangeSet},
    tonic::{Request, Response, Status},
};

use crate::{
    grpc::{auth::require_client_id, notification_builder},
    session::{ClientId, ClientRingBuffer, Session, SessionId, SessionRegistry},
};

/// gRPC `InputService` implementation.
///
/// Bridges v2 protocol key input requests to the session system.
/// Provides basic character insertion. Full vim-style key resolution
/// (resolvers, operator-pending modes, etc.) requires modules to be
/// loaded by the runner.
pub struct InputServiceImpl {
    /// Shared session registry.
    sessions: Arc<SessionRegistry>,
    /// Default session ID to use when not specified.
    default_session_id: SessionId,
    /// Extension bridge registry for notification emission (#514).
    bridges: Arc<BridgeRegistry>,
    /// Cache of last-emitted extension snapshots for deduplication (#691).
    ///
    /// `detect_bridge_changes` records ALL active bridges as changed on every
    /// keypress, but most bridge states don't change on cursor movement.
    /// This cache suppresses `ExtensionUpdated` when the JSON is identical
    /// to the last emission, eliminating ~5 unnecessary gRPC broadcasts per
    /// j/k press.
    snapshot_cache: Mutex<HashMap<(String, u64), String>>,
}

impl InputServiceImpl {
    /// Create a new `InputService` with access to the session registry.
    #[must_use]
    pub fn new(
        sessions: Arc<SessionRegistry>,
        default_session_id: SessionId,
        bridges: Arc<BridgeRegistry>,
    ) -> Self {
        Self {
            sessions,
            default_session_id,
            bridges,
            snapshot_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Get the default session.
    fn get_session(&self) -> Result<Arc<Session>, Status> {
        self.sessions
            .get(&self.default_session_id)
            .ok_or_else(|| Status::not_found("No active session"))
    }
}

#[tonic::async_trait]
impl InputService for InputServiceImpl {
    /// Send keys to the editor.
    ///
    /// Parses vim notation keys (e.g., `iHello<Esc>`, `<C-w>h`) and processes them.
    ///
    /// # Key Resolution
    ///
    /// When modules are loaded, keys go through the full resolver system:
    /// - `SessionState::resolve_key_for_client()` finds the appropriate mode resolver
    /// - The resolver returns a `ResolveResult` (execute, insert, transition, etc.)
    /// - This method handles each result type appropriately
    /// - State changes are accumulated and emitted as notifications
    ///
    /// When modules are NOT loaded (empty registries), falls back to character insertion.
    #[allow(clippy::too_many_lines)]
    async fn send_keys(
        &self,
        request: Request<SendKeysRequest>,
    ) -> Result<Response<SendKeysResponse>, Status> {
        // #483 Phase 5: Token-only authentication (no body fallback)
        let token_client_id = request.extensions().get::<ClientId>().copied();
        let req = request.into_inner();
        let session = self.get_session()?;

        // #483 Phase 5: caller identity from token
        let client_id = require_client_id(token_client_id)?;

        // Client must exist (created via Join())
        if !session.clients().has_client(client_id) {
            return Err(Status::failed_precondition(format!(
                "Client {client_id} not found — call Join() before sending keys"
            )));
        }

        // Check client relation for input routing
        if let Some(client) = session.clients().get_client(client_id)
            && client.is_following()
        {
            // Following: input is ignored (read-only spectator)
            tracing::debug!(%client_id, "Input ignored for Following client");
            return Ok(Response::new(SendKeysResponse {
                ok: false,
                status: KeyStatus::NotFound.into(),
                should_quit: false,
            }));
        }
        // Independent/Sharing: proceed with normal input processing
        // Note: For Sharing, input goes to target's state - handled by
        // session.resolve_key_for_client() and session.execute_command_for_client()

        // Parse vim notation keys
        let keys = KeySequence::parse(&req.keys).ok_or_else(|| {
            Status::invalid_argument(format!("Invalid key notation: {}", req.keys))
        })?;

        // #514/#468: Snapshot ALL bridge active states before key resolution.
        // Generic detection replaces hardcoded cmdline check.
        let bridge_states_before = Self::snapshot_bridge_states(&session, client_id, &self.bridges);

        // #521/#753 E5: Track mode and compositor generation before key processing
        // for bridge lifecycle hooks and poll-based change detection.
        let mode_before_keys = session.client_current_mode(client_id);
        let compositor_gen_before = session.client_compositor_generation(client_id);

        // Process each key through the resolver system
        let mut any_handled = false;
        let mut final_status = KeyStatus::NotFound;
        let mut accumulated_changes = ChangeSet::new();

        for key in keys.as_slice() {
            // Phase #478: Log key to client ring buffer
            session.with_client_ring_buffer(client_id, |rb: &ClientRingBuffer| {
                rb.log_key(&format!("{:?}", key.code));
            });

            // Debug: Log current mode before resolution
            let current_mode = session
                .client_current_mode(client_id)
                .expect("BUG: client just created but not found - state corruption");
            tracing::debug!(
                ?current_mode,
                %client_id,
                code = ?key.code,
                modifiers = ?key.modifiers,
                "Resolving key"
            );

            // Sub-plan 05 Phase 1: Full dispatch pipeline in one call.
            // resolve + handle result + mode transitions + command execution +
            // pending bindings + on_command_complete — all within the runtime scope.
            let dispatch_result = session.dispatch_key_for_client(client_id, key).await;

            // None means no domain driver active (stub, #753 E3) — key is dropped.
            let Some((handled, changes)) = dispatch_result else {
                tracing::warn!(
                    ?current_mode,
                    %client_id,
                    "dispatch_key_for_client: no domain driver active, key dropped (#753 E3)"
                );
                continue;
            };

            accumulated_changes.merge(changes);

            if handled {
                any_handled = true;
                final_status = KeyStatus::Executed;
            }
        }

        // Record cursor movement for active buffer whenever any key was handled.
        // Most key operations move the cursor (typing, motions, commands like `o`).
        // `record_cursor_move` is idempotent on `affected_buffers`, so calling it
        // even when InsertChar already recorded cursor_move is safe (#505).
        // Per-client active_buffer (#471)
        if any_handled && let Some(buffer_id) = session.active_buffer_for_client(client_id) {
            accumulated_changes.record_cursor_move(buffer_id);
        }

        // #753 E5: Poll-based change detection — compare compositor generation
        // and mode with pre-dispatch values to set ChangeSet flags accurately.
        // The domain driver handles state internally; the server detects changes
        // by comparing before/after snapshots.
        let compositor_gen_after = session.client_compositor_generation(client_id);
        if compositor_gen_after != compositor_gen_before {
            accumulated_changes.layout_changed = true;
        }

        // #664/#753: CursorSnapshot is now an opaque [u8; 8] identity token.
        // Viewport scroll and selection tracking are domain-owned (#753 E3).
        // Bridges read cursor/selection state via projections instead.

        // Auto-emit presence update on buffer/window change (#471).
        // buffer_id comes from domain driver (#753 E3).
        if accumulated_changes.layout_changed || accumulated_changes.focus_changed {
            let new_buffer_id = session
                .active_buffer_for_client(client_id)
                .map(|b| b.as_usize());
            session.presence().update(client_id, |p| {
                p.buffer_id = new_buffer_id;
            });
            accumulated_changes.record_presence_change(client_id.as_usize());
        }

        // #521/#753 E5: Detect mode changes for bridge hooks and ChangeSet flags.
        let mode_after_keys = session.client_current_mode(client_id);
        if let (Some(before), Some(after)) = (&mode_before_keys, &mode_after_keys)
            && before != after
        {
            accumulated_changes.mode_changed = true;
            let from = before.to_string();
            let to = after.to_string();
            Self::notify_bridges_mode_changed(&session, client_id, &self.bridges, &from, &to);
        }

        // #662: Notify bridges of cursor movement so overlays can self-dismiss.
        if accumulated_changes.cursor_moved {
            Self::notify_bridges_cursor_moved(
                &session,
                client_id,
                &self.bridges,
                &mut accumulated_changes,
            );
        }

        // #514/#468/#469: Generic bridge change detection — emit on toggle AND on
        // every key while active (each keystroke may modify extension state).
        Self::detect_bridge_changes(
            &session,
            client_id,
            &self.bridges,
            &bridge_states_before,
            &mut accumulated_changes,
        );

        // Emit notifications BEFORE syntax updates so the TUI receives
        // BufferModified (which refreshes buffer_cache) before TokenUpdate
        // (which needs fresh cache for byte→line conversion).
        // Phase 14 (#471): Pass client_id for cursor/selection filtering
        // Phase #486: emit_notifications is now sync (uses sync per-client state access)
        if accumulated_changes.has_changes() {
            Self::emit_notifications(
                &session,
                &accumulated_changes,
                client_id.as_usize() as u64,
                &self.bridges,
                &self.snapshot_cache,
            );
        }

        // Collect projections from domain driver and emit updates (#753).
        // Called AFTER dispatch releases all Session locks. The driver holds
        // its own state; ProjectionStore acquires its own disjoint lock.
        Self::emit_projection_updates(&session, client_id);

        // #753 E6: Syntax/codec updates route through domain driver post-dispatch hooks.
        // The server no longer touches driver types directly.

        // Return result
        Ok(Response::new(SendKeysResponse {
            ok: any_handled,
            status: final_status.into(),
            should_quit: accumulated_changes.should_quit,
        }))
    }
}

impl InputServiceImpl {
    /// Check bridge `is_active()` for the appropriate scope.
    fn bridge_is_active(
        bridge: &dyn reovim_subsys_session::bridges::ExtensionStateBridge,
        session: &Session,
        client_id: ClientId,
    ) -> bool {
        match bridge.scope() {
            reovim_subsys_session::bridges::ExtensionScope::Client => session
                .clients()
                .with_client_extensions(client_id, |ext| bridge.is_active(ext))
                .unwrap_or(false),
            reovim_subsys_session::bridges::ExtensionScope::Shared => {
                session.with_state_sync(|state| bridge.is_active(&state.app.extensions))
            }
        }
    }

    /// Snapshot the `is_active()` state of all registered bridges for a client.
    ///
    /// Called BEFORE key resolution to detect changes afterwards (#468).
    fn snapshot_bridge_states(
        session: &Session,
        client_id: ClientId,
        bridges: &BridgeRegistry,
    ) -> Vec<(&'static str, bool)> {
        bridges
            .kinds()
            .into_iter()
            .map(|kind| {
                let bridge = bridges.get(kind).expect("bridge kind from kinds()");
                let active = Self::bridge_is_active(bridge, session, client_id);
                (kind, active)
            })
            .collect()
    }

    /// Detect bridge state changes after key resolution (#468).
    ///
    /// For each bridge, emit a change notification if:
    /// - The active state toggled (was inactive, now active, or vice versa)
    /// - The bridge is currently active (content may have changed by key input)
    fn detect_bridge_changes(
        session: &Session,
        client_id: ClientId,
        bridges: &BridgeRegistry,
        before: &[(&str, bool)],
        changes: &mut ChangeSet,
    ) {
        for &(kind, was_active) in before {
            let bridge = bridges.get(kind).expect("bridge kind from snapshot");
            let is_active = Self::bridge_is_active(bridge, session, client_id);
            if was_active != is_active || is_active {
                changes.record_extension_change(kind.into());
            }
        }
    }

    /// Notify all client-scoped bridges of a mode change (#521).
    ///
    /// Iterates all registered bridges and calls `on_mode_changed` for those
    /// with [`ExtensionScope::Client`], giving them a chance to self-dismiss
    /// (e.g., completion popup on leaving insert mode).
    fn notify_bridges_mode_changed(
        session: &Session,
        client_id: ClientId,
        bridges: &BridgeRegistry,
        from: &str,
        to: &str,
    ) {
        session
            .clients()
            .with_client_extensions_mut(client_id, |ext| {
                for bridge in bridges.values() {
                    if bridge.scope() == reovim_subsys_session::bridges::ExtensionScope::Client {
                        bridge.on_mode_changed(from, to, ext);
                    }
                }
            });
    }

    /// Notify all client-scoped bridges of a cursor movement (#662).
    ///
    /// Cursor position is domain-owned (#753 E3); use (0, 0) as placeholder
    /// until projections supply cursor coordinates.
    fn notify_bridges_cursor_moved(
        session: &Session,
        client_id: ClientId,
        bridges: &BridgeRegistry,
        changes: &mut ChangeSet,
    ) {
        // Cursor position is domain-owned. Pass (0, 0) as placeholder (#753 E3).
        let (line, col) = (0usize, 0usize);

        session
            .clients()
            .with_client_extensions_mut(client_id, |ext| {
                for bridge in bridges
                    .values()
                    .filter(|b| b.scope() == reovim_subsys_session::bridges::ExtensionScope::Client)
                {
                    let was_active = bridge.is_active(ext);
                    bridge.on_cursor_moved(line, col, ext);
                    let is_active = bridge.is_active(ext);
                    if was_active && !is_active {
                        changes.record_extension_change(bridge.kind().into());
                    }
                }
            });
    }

    /// Emit notifications for state changes.
    ///
    /// Converts accumulated `StateChanges` to gRPC notifications and emits them
    /// to all subscribed clients.
    ///
    /// # Arguments
    ///
    /// * `session` - The session to emit to
    /// * `changes` - State changes to convert to notifications
    /// * `client_id` - Client ID that originated these changes (for multi-client filtering)
    ///
    /// Phase #486: Now passes `&Session` directly to `build_notifications()` for per-client state.
    fn emit_notifications(
        session: &Session,
        changes: &ChangeSet,
        client_id: u64,
        bridges: &BridgeRegistry,
        snapshot_cache: &Mutex<HashMap<(String, u64), String>>,
    ) {
        let notifications =
            notification_builder::build_notifications(changes, session, client_id, Some(bridges));

        let mut cache = snapshot_cache.lock();
        let mut emitted = 0u32;
        let mut suppressed = 0u32;
        for notif in notifications {
            // Deduplicate extension notifications: skip if JSON is identical
            // to last emission for this (kind, client_id) pair (#691).
            if let Some(notification::Payload::ExtensionUpdated(ref ext)) = notif.payload {
                let key = (ext.kind.clone(), ext.client_id);
                if let Some(prev) = cache.get(&key)
                    && *prev == ext.data
                {
                    suppressed += 1;
                    continue;
                }
                cache.insert(key, ext.data.clone());
            }
            session.emit_notification(notif);
            emitted += 1;
        }
        drop(cache);

        if emitted > 0 || suppressed > 0 {
            tracing::trace!(
                emitted,
                suppressed,
                mode_changed = changes.mode_changed,
                cursor_moved = changes.cursor_moved,
                buffer_modified = !changes.modified_buffers.is_empty(),
                selection_changed = changes.selection_changed,
                "Emitted notifications"
            );
        }
    }

    /// Collect projections from the domain driver and emit updates (#753).
    ///
    /// Called AFTER dispatch releases all `Session` locks. The domain driver
    /// holds its own per-client state; `ProjectionStore` acquires its own
    /// disjoint `RwLock` — no deadlock risk.
    #[allow(clippy::cast_possible_truncation)]
    fn emit_projection_updates(session: &Session, client_id: ClientId) {
        let driver = session.domain_driver();
        let Some(driver) = driver else {
            return; // No domain driver wired — skip projection collection
        };

        let subsys_cid = reovim_subsys_session::ClientId::new(client_id.as_usize());
        let projections = driver.collect_projections(subsys_cid);
        if projections.is_empty() {
            return;
        }

        let result = session
            .projection_store()
            .write()
            .update(subsys_cid, projections);

        let notifications = notification_builder::build_projection_notifications(
            &result,
            client_id.as_usize() as u64,
            notification_builder::current_timestamp_ms(),
        );

        for notif in notifications {
            session.emit_notification(notif);
        }
    }

    // emit_syntax_updates: REMOVED (#753 E6).
    // Syntax updates route through domain driver post-dispatch hooks.

    // notify_codec_indices: REMOVED (#753 E6).
    // Codec index updates route through domain driver post-dispatch hooks.
}

// Test-only helpers (resolve_to_command_context, handle_resolve_result,
// apply_mode_transition_for_client, handle_pop_result_for_client):
// REMOVED (#753 E6). These used driver types (ResolveResult, StateChanges,
// CommandResult) that no longer exist in the server crate. Tests that depend
// on these helpers need to be rewritten to use DomainDriver dispatch.

// All test-only helpers removed (#753 E6). Tests need rewrite.

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
