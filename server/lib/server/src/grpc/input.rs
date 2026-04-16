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

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use {
    parking_lot::Mutex,
    reovim_protocol::v2::{
        KeyStatus, SendKeysRequest, SendKeysResponse, input_service_server::InputService,
        notification,
    },
    reovim_subsys_input::KeySequence,
    reovim_subsys_session::{change_set::ChangeSet, bridges::BridgeRegistry},
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
        if any_handled
            && let Some(buffer_id) = session.active_buffer_for_client(client_id)
        {
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
            let new_buffer_id =
                session.active_buffer_for_client(client_id).map(|b| b.as_usize());
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

        // Take domain-specific edits for syntax/codec (Phase 5 will move these into driver)
        let pending_text_edits = session.take_pending_text_edits();
        let pending_byte_edits = session.take_pending_byte_edits();

        // Update syntax drivers for modified/deleted buffers (#539, #655)
        if !accumulated_changes.modified_buffers.is_empty()
            || !accumulated_changes.deleted_buffers.is_empty()
        {
            Self::emit_syntax_updates(&session, &accumulated_changes, &pending_text_edits);
        }

        // Notify codec indices of byte-level edits (#740 D.5)
        if !pending_text_edits.is_empty() || !pending_byte_edits.is_empty() {
            Self::notify_codec_indices(&session, &pending_text_edits, &pending_byte_edits);
        }

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

    /// Update syntax drivers and broadcast token updates for modified buffers.
    ///
    /// Called after key processing when `buffer_modified` is true. Uses
    /// incremental `driver.update()` when edit info is available, falls back
    /// to full `driver.parse()` otherwise (#655).
    ///
    /// The function splits mutable borrows to avoid `ExtensionMap` aliasing:
    /// 1. Access `SyntaxSessionState`, update driver, build `TokenUpdate`
    /// 2. Access `SyntaxStreamState`, broadcast the update
    fn emit_syntax_updates(
        session: &Session,
        changes: &ChangeSet,
        text_edits: &[reovim_driver_codec::TextBufferModified],
    ) {
        use {
            crate::session::{SyntaxSessionState, SyntaxStreamState, build_token_update},
            reovim_driver_text_syntax::text_event_to_syntax_edit, // TODO: relocate bridge call (Tier 2 server decoupling)
        };

        // Clean up syntax drivers for deleted buffers (#655 Phase 4)
        if !changes.deleted_buffers.is_empty() {
            session.with_state_mut_sync(|state| {
                let syntax = state.app.extensions.get_or_insert::<SyntaxSessionState>();
                for &buffer_id in &changes.deleted_buffers {
                    syntax.remove(buffer_id);
                }
            });
        }

        if changes.modified_buffers.is_empty() {
            return;
        }

        session.with_state_mut_sync(|state| {
            for &buffer_id in &changes.modified_buffers {
                // Get buffer content and file path
                let Some(buffer_arc) = state.buffer(buffer_id) else {
                    continue;
                };
                let (content, file_path, total_lines) = {
                    let buffer = buffer_arc.read();
                    (
                        buffer.content(),
                        buffer.file_path().map(String::from),
                        buffer.line_count() as u64,
                    )
                };

                // Step 1: Update driver (mutable borrow of SyntaxSessionState)
                let syntax = state.app.extensions.get_or_insert::<SyntaxSessionState>();
                if let Some(ref path) = file_path {
                    syntax.ensure_driver_from_path(buffer_id, path, &content);
                }

                // Try incremental update if text-domain edit available, fall back to full reparse
                // (#655, migrated to pending_text_edits in Phase 2)
                let edit_info = text_edits
                    .iter()
                    .find(|event| event.buffer_id == buffer_id)
                    .map(text_event_to_syntax_edit);

                if let Some(driver) = syntax.get_mut(buffer_id) {
                    match edit_info {
                        Some(ref edit) => driver.update(&content, edit),
                        None => driver.parse(&content),
                    }
                }

                // Build token update from the driver (immutable borrow)
                let full_refresh = edit_info.is_none();
                let update = build_token_update(
                    state.app.extensions.get_or_insert::<SyntaxSessionState>(),
                    buffer_id,
                    total_lines,
                    full_refresh,
                );

                // Step 2: Broadcast to subscribers (mutable borrow of SyntaxStreamState)
                if let Some(update) = update {
                    let stream = state.app.extensions.get_or_insert::<SyntaxStreamState>();
                    stream.broadcast(&update);
                }
            }
        });
    }

    /// Notify codec indices of byte-level edits (#740 D.5).
    ///
    /// Routes `TextBufferModified` and `ByteEdit` records to
    /// `CodecSessionState::notify_index()` for incremental index updates.
    fn notify_codec_indices(
        session: &Session,
        text_edits: &[reovim_driver_codec::TextBufferModified],
        byte_edits: &[(reovim_kernel::api::v1::BufferId, reovim_kernel::api::v1::ByteEdit)],
    ) {
        use reovim_driver_codec::CodecSessionState;

        let mut decoded_buffers = HashSet::new();

        session.with_state_mut_sync(|state| {
            if let Some(codec_state) = state.app.extensions.get_mut::<CodecSessionState>() {
                for event in text_edits {
                    let decoded_edit = reovim_driver_codec::text_edit_to_decoded_edit(event);

                    let view = codec_state
                        .active_view(event.buffer_id)
                        .unwrap_or("default")
                        .to_string();

                    if let Some(byte_edit) =
                        codec_state.apply_decoded_edit(event.buffer_id, &view, &decoded_edit)
                    {
                        decoded_buffers.insert(event.buffer_id);
                        codec_state.notify_index(event.buffer_id, &byte_edit);
                    }
                }

                for (buffer_id, edit) in byte_edits {
                    if decoded_buffers.contains(buffer_id) {
                        continue;
                    }

                    codec_state.apply_byte_edit(*buffer_id, edit);
                    codec_state.notify_index(*buffer_id, edit);
                }
            }
        });
    }

    // NOTE (#471): `fallback_char_insert()` was REMOVED.
    // NOTE (#477): `insert_char_by_target()` removed (server domain decoupling).
    //
    // The fallback was a design mistake that masked configuration bugs by silently
    // inserting characters when no resolver was found. This caused:
    // 1. Characters appearing in Normal mode (should be commands)
    // 2. ModeId mismatches going undetected (e.g., "default/normal" vs "vim/normal")
    //
    // Now, if no resolver is found for a mode, `send_keys()` panics with a clear
    // error message. This is the correct behavior - a missing resolver is a
    // configuration bug that should be fixed, not worked around.
}

/// Test-only re-exposures of helper functions that were inlined into
/// `session/state.rs` dispatch paths during server domain decoupling.
///
/// These are kept in a `#[cfg(test)]` block so the test suite can exercise
/// the logic directly without polluting the production API.
#[cfg(test)]
#[allow(unused_imports)]
use reovim_driver_text_session::api::StateChanges;

#[cfg(test)]
impl InputServiceImpl {
    /// Convert a `ResolveContext` into a `CommandContext`.
    ///
    /// Mirrors `session::state::resolve_to_command_context_inline`.
    pub(crate) fn resolve_to_command_context(
        ctx: &reovim_driver_text_input::ResolveContext,
    ) -> reovim_subsys_command_types::CommandContext {
        use reovim_subsys_command_types::ArgValue;

        let mut cmd_ctx = reovim_subsys_command_types::CommandContext::new();
        if let Some(count) = ctx.count {
            cmd_ctx.set("count", ArgValue::Count(count));
        }
        if let Some(reg) = ctx.register {
            cmd_ctx.set("register", ArgValue::Register(reg));
        }
        for (key, value) in &ctx.metadata {
            let converted = match value {
                reovim_driver_text_input::ArgValue::Bool(b) => Some(ArgValue::Bool(*b)),
                reovim_driver_text_input::ArgValue::String(s) => Some(ArgValue::String(s.clone())),
                reovim_driver_text_input::ArgValue::Char(c) => Some(ArgValue::Char(*c)),
                reovim_driver_text_input::ArgValue::Int(n) => {
                    usize::try_from(*n).ok().map(ArgValue::Count)
                }
                reovim_driver_text_input::ArgValue::Uint(n) => {
                    usize::try_from(*n).ok().map(ArgValue::Count)
                }
                reovim_driver_text_input::ArgValue::Position(p) => {
                    Some(ArgValue::Position(p.line, p.column))
                }
                reovim_driver_text_input::ArgValue::Float(_)
                | reovim_driver_text_input::ArgValue::Range { .. } => None,
            };
            if let Some(arg_value) = converted {
                cmd_ctx.set(key, arg_value);
            }
        }
        cmd_ctx
    }

    /// Handle a single `ResolveResult` against a session, returning
    /// `(handled, StateChanges)`.
    ///
    /// This is a test-only wrapper that dispatches through the session's
    /// execution path.  The `_key` argument is accepted for API compatibility
    /// with historic callers; it is not used.
    pub(crate) async fn handle_resolve_result(
        session: &crate::session::Session,
        result: reovim_driver_text_input::ResolveResult,
        _key: &reovim_driver_text_input::KeyEvent,
        client_id: crate::session::ClientId,
    ) -> (bool, reovim_driver_text_session::api::StateChanges) {
        use {
            reovim_driver_text_input::ResolveResult, reovim_driver_text_session::api::StateChanges,
            reovim_subsys_command_types::RuntimeSignal,
        };

        match result {
            ResolveResult::Completed | ResolveResult::Pending => (true, StateChanges::new()),
            ResolveResult::NotHandled => (false, StateChanges::new()),

            ResolveResult::InsertChar { .. } => {
                tracing::error!(
                    "InsertChar reached test dispatch — resolvers must handle insertion via SessionApi"
                );
                (true, StateChanges::new())
            }

            ResolveResult::ModeTransition(transition) => {
                let mut changes = StateChanges::new();
                changes.mode_changed = true;
                Self::apply_mode_transition_for_client(session, client_id, transition).await;
                (true, changes)
            }

            ResolveResult::Execute(cmd_id, ctx) => {
                let cmd_ctx = Self::resolve_to_command_context(&ctx);
                let mut changes = StateChanges::new();
                if let Some((_, cmd_changes, signals)) =
                    session.execute_command_for_client(client_id, &cmd_id, &cmd_ctx)
                {
                    changes.merge(cmd_changes);
                    for signal in signals {
                        match signal {
                            RuntimeSignal::Quit => changes.record_quit_requested(),
                        }
                    }
                }
                (true, changes)
            }

            ResolveResult::InjectKeys { keys, .. } => {
                let mut changes = StateChanges::new();
                for injected_key in &keys {
                    // Re-dispatch through session's full key dispatch.
                    // dispatch_key_for_client returns ChangeSet; convert back to
                    // StateChanges for the test-only accumulation buffer.
                    if let Some((handled, key_changes)) = session
                        .dispatch_key_for_client(client_id, injected_key)
                        .await
                    {
                        changes.merge(reovim_driver_text_session::state_changes_from_change_set(
                            key_changes,
                        ));
                        let _ = handled;
                    }
                }
                (true, changes)
            }
        }
    }

    /// Apply a `ModeTransition` for a client and run the deferred
    /// `on_command_complete` chain.
    ///
    /// This is a test-only wrapper that mirrors the mode-transition logic
    /// formerly inlined in `InputServiceImpl`.
    pub(crate) async fn apply_mode_transition_for_client(
        session: &crate::session::Session,
        client_id: crate::session::ClientId,
        transition: reovim_subsys_input::ModeTransition,
    ) {
        use reovim_subsys_input::ModeTransition;

        // Apply the initial transition to the client's mode stack.
        session.clients().with_clients_mut(|clients| {
            let Some(client) = clients.get_mut(&client_id) else {
                return;
            };
            match &transition {
                ModeTransition::Push { mode, .. } => {
                    client.state.mode_stack.push(mode.clone());
                }
                ModeTransition::Pop { .. } => {
                    client.state.mode_stack.pop();
                }
                ModeTransition::Set { mode, .. } => {
                    client.state.mode_stack.set(mode.clone());
                }
            }
        });

        // Handle any pop result from the initial transition.
        if let ModeTransition::Pop {
            result: Some(pop_result),
        } = transition
        {
            Self::handle_pop_result_for_client(session, client_id, pop_result);
        }

        // Deferred completion loop: run on_command_complete and apply the
        // returned transition. The loop continues only when the resolver
        // returns `Pop { result: Some(...) }` (i.e. the pop executes a nested
        // command that may trigger further completions).  All other variants
        // (Push, Set, or Pop { result: None }) break after being applied.
        loop {
            let Some(complete_transition) =
                session.try_on_command_complete_for_client(client_id).await
            else {
                break;
            };

            match complete_transition {
                ModeTransition::Push { ref mode, .. } => {
                    let mode = mode.clone();
                    session.clients().with_clients_mut(|clients| {
                        if let Some(client) = clients.get_mut(&client_id) {
                            client.state.mode_stack.push(mode);
                        }
                    });
                    break;
                }
                ModeTransition::Set { ref mode, .. } => {
                    let mode = mode.clone();
                    session.clients().with_clients_mut(|clients| {
                        if let Some(client) = clients.get_mut(&client_id) {
                            // Pop all non-base modes then set.
                            while client.state.mode_stack.depth() > 1 {
                                client.state.mode_stack.pop();
                            }
                            client.state.mode_stack.set(mode);
                        }
                    });
                    break;
                }
                ModeTransition::Pop { result } => {
                    // Apply the pop (no-op if already at base depth).
                    session.clients().with_clients_mut(|clients| {
                        if let Some(client) = clients.get_mut(&client_id) {
                            client.state.mode_stack.pop();
                        }
                    });
                    if let Some(pr) = result {
                        // Execute the nested pop result and continue the loop
                        // so any further completions are processed.
                        Self::handle_pop_result_for_client(session, client_id, pr);
                        // continue
                    } else {
                        // Pop { result: None } — no nested command; break.
                        break;
                    }
                }
            }
        }
    }

    /// Handle a `PopResult` for a client.
    ///
    /// Returns `StateChanges` reflecting any commands executed (including
    /// `RuntimeSignal::Quit` translated to `should_quit`).
    pub(crate) fn handle_pop_result_for_client(
        session: &crate::session::Session,
        client_id: crate::session::ClientId,
        result: reovim_subsys_input::PopResult,
    ) -> reovim_driver_text_session::api::StateChanges {
        use {
            reovim_driver_text_session::api::StateChanges,
            reovim_subsys_command_types::RuntimeSignal, reovim_subsys_input::PopResult,
        };

        match result {
            PopResult::Cancelled | PopResult::Data { .. } => StateChanges::new(),
            PopResult::ExecuteCommand { command, args } => {
                let mut cmd_ctx = reovim_subsys_command_types::CommandContext::new();
                for (key, value) in args {
                    cmd_ctx.set(&key, value);
                }
                let mut changes = StateChanges::new();
                if let Some((result, cmd_changes, signals)) =
                    session.execute_command_for_client(client_id, &command, &cmd_ctx)
                {
                    changes.merge(cmd_changes);
                    for signal in signals {
                        match signal {
                            RuntimeSignal::Quit => changes.record_quit_requested(),
                        }
                    }
                    if let reovim_driver_command::CommandResult::Error(msg) = result {
                        session.with_client_ring_buffer(client_id, |rb| {
                            rb.log_error(&format!("COMMAND_FAILED: {msg}"));
                        });
                    }
                }
                changes
            }
        }
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
