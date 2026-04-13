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
    reovim_driver_input::{KeySequence, ModeTransition, PopResult, ResolveContext, ResolveResult},
    reovim_driver_session::{api::StateChanges, bridges::BridgeRegistry},
    reovim_protocol::v2::{
        KeyStatus, SendKeysRequest, SendKeysResponse, input_service_server::InputService,
        notification,
    },
    reovim_subsys_command_types::{ArgValue, CommandContext, CommandResult},
    tonic::{Request, Response, Status},
};

use crate::{
    grpc::{auth::require_client_id, notification_builder},
    session::{ClientEventType, ClientId, ClientRingBuffer, Session, SessionId, SessionRegistry},
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

        // #521: Track mode before key processing for bridge lifecycle hooks.
        let mode_before_keys = session.client_current_mode(client_id);

        // Process each key through the resolver system
        let mut any_handled = false;
        let mut final_status = KeyStatus::NotFound;
        let mut accumulated_changes = StateChanges::new();

        for key in keys.as_slice() {
            // Phase #478: Log key to client ring buffer
            session.with_client_ring_buffer(client_id, |rb: &ClientRingBuffer| {
                rb.log_key(&format!("{:?}", key.code));
            });

            // Debug: Log current mode before resolution
            // Per-client state (#471): Use per-client mode - client was created above
            // Phase #479: KERNEL PANIC if client not found (invariant violation)
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

            // Per-client state (#471): Try to resolve with per-client mode stack first
            // This enables multi-client mode isolation (Client A in INSERT, Client B in NORMAL)
            let resolve_result = session.resolve_key_for_client(client_id, key).await;

            // Debug: Log resolution result
            tracing::debug!(
                resolver_found = resolve_result.is_some(),
                result_type = resolve_result.as_ref().map(|(r, _)| format!("{r:?}")),
                "Key resolution result"
            );

            // Per-client state (#471): Resolver MUST exist for the current mode.
            // If no resolver is found, it's a configuration bug - panic to catch it early.
            let Some((result, changes)) = resolve_result else {
                panic!(
                    "No resolver found for mode {current_mode:?} (client_id={client_id}). \
                     This is a configuration bug - ensure modules are loaded properly \
                     and the client's mode stack is initialized with the correct ModeId."
                );
            };

            // Accumulate changes from resolution
            accumulated_changes.merge(changes);

            // Key was processed by a resolver
            // Per-client state (#471): Pass client_id for per-client mode transitions
            let (handled, result_changes) =
                Self::handle_resolve_result(&session, result, key, client_id).await;
            accumulated_changes.merge(result_changes);

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
            && let Some(buffer_id) = session
                .clients()
                .with_clients(|clients| clients.get(&client_id).and_then(|c| c.state.active_buffer))
        {
            accumulated_changes.record_cursor_move(buffer_id);
        }

        // Viewport scroll tracking: adjust scroll_top so cursor stays visible.
        if any_handled {
            let scrolled_window = session.clients().with_clients_mut(|clients| {
                let client = clients.get_mut(&client_id)?;
                let window = client.state.windows.active_mut()?;
                if window.viewport.ensure_cursor_visible(window.cursor.line) {
                    Some(window.id)
                } else {
                    None
                }
            });
            if let Some(window_id) = scrolled_window {
                accumulated_changes.record_scroll_change(window_id);
            }
        }

        // #664: Update cursor snapshot for bridge tick consumption.
        // Bridges (illuminate, etc.) read this in tick() to detect cursor movement.
        if accumulated_changes.cursor_moved {
            session.clients().with_clients_mut(|clients| {
                let client = clients.get_mut(&client_id)?;
                let window = client.state.windows.active()?;
                let buffer_id = window.buffer_id?;
                #[allow(clippy::cast_possible_truncation)]
                {
                    let snap = client
                        .state
                        .extensions
                        .get_or_insert::<reovim_driver_session::CursorSnapshot>();
                    snap.line = window.cursor.line as u32;
                    snap.col = window.cursor.column as u32;
                    snap.buffer_id = buffer_id.as_usize() as u64;
                }
                Some(())
            });
        }

        // #474: Auto-detect selection changes (defense-in-depth).
        if let Some(state) = session.clients().client_state(client_id) {
            Self::ensure_selection_change_recorded(
                &mut accumulated_changes,
                &state.windows,
                state.active_buffer,
            );
        }

        // Auto-emit presence update on buffer/window change (#471).
        if accumulated_changes.window_changed || accumulated_changes.focus_changed {
            let new_buffer_id = session.clients().with_clients(|clients| {
                let window = clients.get(&client_id)?.state.windows.active()?;
                Some(window.buffer_id?.as_usize())
            });
            session.presence().update(client_id, |p| {
                p.buffer_id = new_buffer_id;
            });
            accumulated_changes.record_presence_change(client_id.as_usize());
        }

        // #521: Notify bridges of mode changes so they can self-dismiss.
        let mode_after_keys = session.client_current_mode(client_id);
        if let (Some(before), Some(after)) = (&mode_before_keys, &mode_after_keys)
            && before != after
        {
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

        // Update syntax drivers for modified/deleted buffers (#539, #655)
        if accumulated_changes.buffer_modified || !accumulated_changes.buffers_deleted.is_empty() {
            Self::emit_syntax_updates(&session, &accumulated_changes);
        }

        // Notify codec indices of byte-level edits (#740 D.5)
        if !accumulated_changes.text_buffer_edits.is_empty()
            || !accumulated_changes.byte_edits.is_empty()
        {
            Self::notify_codec_indices(&session, &accumulated_changes);
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
        bridge: &dyn reovim_driver_session::bridges::ExtensionStateBridge,
        session: &Session,
        client_id: ClientId,
    ) -> bool {
        match bridge.scope() {
            reovim_driver_session::bridges::ExtensionScope::Client => session
                .clients()
                .with_client_extensions(client_id, |ext| bridge.is_active(ext))
                .unwrap_or(false),
            reovim_driver_session::bridges::ExtensionScope::Shared => {
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
        changes: &mut StateChanges,
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
                    if bridge.scope() == reovim_driver_session::bridges::ExtensionScope::Client {
                        bridge.on_mode_changed(from, to, ext);
                    }
                }
            });
    }

    /// Notify all client-scoped bridges of a cursor movement (#662).
    ///
    /// Gives bridges a chance to self-dismiss overlays (e.g., hover popup)
    /// when the cursor moves away from the trigger position.
    fn notify_bridges_cursor_moved(
        session: &Session,
        client_id: ClientId,
        bridges: &BridgeRegistry,
        changes: &mut StateChanges,
    ) {
        let Some((line, col)) = session.clients().with_clients(|clients| {
            let window = clients.get(&client_id)?.state.windows.active()?;
            Some((window.cursor.line, window.cursor.column))
        }) else {
            return;
        };

        session
            .clients()
            .with_client_extensions_mut(client_id, |ext| {
                for bridge in bridges
                    .values()
                    .filter(|b| b.scope() == reovim_driver_session::bridges::ExtensionScope::Client)
                {
                    let was_active = bridge.is_active(ext);
                    bridge.on_cursor_moved(line, col, ext);
                    let is_active = bridge.is_active(ext);
                    // If the bridge deactivated, record an extension change
                    // so the notification pipeline sends the updated state.
                    if was_active && !is_active {
                        changes.record_extension_change(bridge.kind().into());
                    }
                }
            });
    }

    /// Defense-in-depth: if cursor moved but `selection_changed` was not set by
    /// the resolver pipeline, check whether the client has an active selection and
    /// record `selection_changed` so the notification pipeline picks it up.
    ///
    /// This catches commands that set `cursor_moved` on `accumulated_changes`
    /// directly (outside `SessionRuntime::record_cursor_move`).
    fn ensure_selection_change_recorded(
        changes: &mut StateChanges,
        windows: &reovim_driver_session::WindowLayout,
        active_buffer: Option<reovim_kernel::api::v1::BufferId>,
    ) {
        if !changes.cursor_moved || changes.selection_changed {
            return;
        }
        let has_selection = windows.active().is_some_and(|w| w.selection.is_some());
        if has_selection && let Some(buffer_id) = active_buffer {
            changes.record_selection_change(buffer_id);
        }
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
        changes: &StateChanges,
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
                buffer_modified = changes.buffer_modified,
                selection_changed = changes.selection_changed,
                "Emitted notifications"
            );
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
    fn emit_syntax_updates(session: &Session, changes: &StateChanges) {
        use {
            crate::session::{SyntaxSessionState, SyntaxStreamState, build_token_update},
            reovim_driver_syntax::text_event_to_syntax_edit,
        };

        // Clean up syntax drivers for deleted buffers (#655 Phase 4)
        if !changes.buffers_deleted.is_empty() {
            session.with_state_mut_sync(|state| {
                let syntax = state.app.extensions.get_or_insert::<SyntaxSessionState>();
                for &buffer_id in &changes.buffers_deleted {
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
                // (#655, migrated to text_buffer_edits in #740 Plan 09 Phase 7d)
                let edit_info = changes
                    .text_buffer_edits
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
    /// Routes `ByteEdit` records from `StateChanges` to
    /// `CodecSessionState::notify_index()` for incremental index updates.
    fn notify_codec_indices(session: &Session, changes: &reovim_driver_session::StateChanges) {
        use reovim_driver_codec::CodecSessionState;

        let mut decoded_buffers = HashSet::new();

        session.with_state_mut_sync(|state| {
            if let Some(codec_state) = state.app.extensions.get_mut::<CodecSessionState>() {
                for event in &changes.text_buffer_edits {
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

                for (buffer_id, edit) in &changes.byte_edits {
                    if decoded_buffers.contains(buffer_id) {
                        continue;
                    }

                    codec_state.apply_byte_edit(*buffer_id, edit);
                    codec_state.notify_index(*buffer_id, edit);
                }
            }
        });
    }

    /// Convert `ResolveContext` to `CommandContext`.
    fn resolve_to_command_context(ctx: &ResolveContext) -> CommandContext {
        let mut cmd_ctx = CommandContext::new();
        if let Some(count) = ctx.count {
            cmd_ctx.set("count", ArgValue::Count(count));
        }
        if let Some(reg) = ctx.register {
            cmd_ctx.set("register", ArgValue::Register(reg));
        }
        // Transfer metadata (ResolveContext::ArgValue -> CommandContext::ArgValue)
        for (key, value) in &ctx.metadata {
            use reovim_driver_input::ArgValue as InputArgValue;
            let converted = match value {
                InputArgValue::Bool(b) => Some(ArgValue::Bool(*b)),
                InputArgValue::String(s) => Some(ArgValue::String(s.clone())),
                InputArgValue::Char(c) => Some(ArgValue::Char(*c)),
                InputArgValue::Int(n) => usize::try_from(*n).ok().map(ArgValue::Count),
                InputArgValue::Uint(n) => usize::try_from(*n).ok().map(ArgValue::Count),
                InputArgValue::Position(p) => Some(ArgValue::Position(p.line, p.column)),
                InputArgValue::Float(_) | InputArgValue::Range { .. } => {
                    tracing::trace!(key, "Skipping unconvertible metadata");
                    None
                }
            };
            if let Some(arg_value) = converted {
                cmd_ctx.set(key, arg_value);
            }
        }
        cmd_ctx
    }

    /// Handle a `ResolveResult` from the resolver system.
    ///
    /// Returns `(handled, changes)` where:
    /// - `handled` is `true` if the key was processed successfully
    /// - `changes` contains any state changes that need notification (e.g., buffer modifications)
    ///
    /// # Arguments
    ///
    /// * `session` - The session
    /// * `result` - The resolve result to handle
    /// * `key` - The key that was resolved
    /// * `client_id` - Client ID for per-client mode transitions (#471)
    ///
    /// Note: This function uses `Box::pin` for recursive calls to handle
    /// `InjectKeys` (macro playback) without infinite future sizes.
    #[allow(clippy::too_many_lines)] // Per-client state (#471) added per-client mode sync
    fn handle_resolve_result<'a>(
        session: &'a Session,
        result: ResolveResult,
        key: &'a reovim_driver_input::KeyEvent,
        client_id: ClientId,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = (bool, StateChanges)> + Send + 'a>>
    {
        Box::pin(async move {
            let mut changes = StateChanges::new();

            match result {
                ResolveResult::Execute(cmd_id, ctx) => {
                    // Execute the command with per-client state (Phase #471)
                    tracing::debug!(?cmd_id, ?ctx.count, "Executing command from resolver");
                    let cmd_ctx = Self::resolve_to_command_context(&ctx);

                    // Track per-client mode before execution
                    // Phase #479: KERNEL PANIC if client not found (invariant violation)
                    let mode_before = session
                        .client_current_mode(client_id)
                        .expect("BUG: client just created but not found - state corruption");

                    // Phase #471: Execute command with per-client state directly.
                    // No sync bandaids needed - command operates on per-client mode/cursor.
                    let cmd_changes =
                        session.execute_command_for_client(client_id, &cmd_id, &cmd_ctx);

                    // Call per-client on_command_complete for pending operators
                    if let Some(transition) =
                        session.try_on_command_complete_for_client(client_id).await
                    {
                        let pop_changes =
                            Self::apply_mode_transition_for_client(session, client_id, transition)
                                .await;
                        changes.merge(pop_changes);
                    }

                    // Merge command changes and process signals (#547)
                    if let Some((_, cmd_state_changes, signals)) = cmd_changes {
                        changes.merge(cmd_state_changes);
                        for signal in signals {
                            match signal {
                                reovim_subsys_command_types::RuntimeSignal::Quit => {
                                    tracing::info!(
                                        %client_id,
                                        "Client requested quit via RuntimeSignal"
                                    );
                                    changes.record_quit_requested();
                                }
                            }
                        }
                    }

                    // Check if per-client mode changed
                    // Phase #479: KERNEL PANIC if client not found (invariant violation)
                    let mode_after = session
                        .client_current_mode(client_id)
                        .expect("BUG: client just created but not found - state corruption");
                    if mode_before != mode_after {
                        tracing::debug!(
                            ?mode_before,
                            ?mode_after,
                            %client_id,
                            "Mode changed during command execution"
                        );
                        changes.record_mode_change();
                    }

                    (true, changes)
                }

                ResolveResult::InsertChar { .. } => {
                    // InsertChar is handled by resolvers via resolve_with_session.
                    // If we reach here, a resolver returned InsertChar instead of
                    // using the session API directly. Log an error but don't crash —
                    // the character is simply dropped (no buffer mutation).
                    tracing::error!(
                        "InsertChar reached server dispatch — resolvers must handle insertion via SessionApi"
                    );
                    (true, changes)
                }

                ResolveResult::ModeTransition(transition) => {
                    // Per-client state (#471): Apply mode transition to per-client mode stack
                    tracing::debug!(?transition, %client_id, "Applying mode transition");
                    let pop_changes =
                        Self::apply_mode_transition_for_client(session, client_id, transition)
                            .await;
                    changes.merge(pop_changes);
                    // Record mode change for notification
                    changes.record_mode_change();
                    (true, changes)
                }

                ResolveResult::Completed => {
                    // Resolver handled everything via SessionApi
                    tracing::trace!("Resolver completed action via SessionApi");
                    (true, changes)
                }

                ResolveResult::Pending => {
                    // Key accumulated, waiting for more
                    tracing::trace!(?key.code, "Key pending, waiting for more input");
                    (true, changes)
                }

                ResolveResult::NotHandled => {
                    // Resolver didn't handle the key
                    tracing::debug!(?key.code, "Key not handled by resolver");
                    (false, changes)
                }

                ResolveResult::InjectKeys {
                    keys,
                    exit_macro_playback: _,
                } => {
                    // Macro playback - inject keys into session (Epic #465 Phase 8D)
                    tracing::debug!(key_count = keys.len(), "Injecting macro keys");

                    // Process injected keys through the resolver system
                    // Per-client state (#471): Use per-client resolution
                    for injected_key in &keys {
                        let resolve_result = session
                            .resolve_key_for_client(client_id, injected_key)
                            .await;

                        if let Some((result, resolver_changes)) = resolve_result {
                            // Accumulate changes from resolver
                            changes.merge(resolver_changes);

                            // Recursively handle the result
                            // Note: We ignore nested InjectKeys to prevent infinite loops
                            if !matches!(result, ResolveResult::InjectKeys { .. }) {
                                let (_, nested_changes) = Self::handle_resolve_result(
                                    session,
                                    result,
                                    injected_key,
                                    client_id,
                                )
                                .await;
                                changes.merge(nested_changes);
                            }
                        }
                    }

                    // Note: exit_macro_playback handling requires access to VimSessionState
                    // which is a vim module type. For now, macro depth tracking is approximate.
                    // TODO(#465): Add trait-based callback mechanism for cross-module state

                    (true, changes)
                }
            }
        })
    }

    /// Apply a mode transition to per-client mode stack (#471).
    ///
    /// This modifies the per-client mode stack instead of the shared session
    /// mode stack, enabling multi-client mode isolation.
    ///
    /// # Arguments
    ///
    /// * `session` - The session
    /// * `client_id` - Client ID whose mode stack to modify
    /// * `transition` - The mode transition to apply
    async fn apply_mode_transition_for_client(
        session: &Session,
        client_id: ClientId,
        transition: ModeTransition,
    ) -> StateChanges {
        // Update per-client mode stack via session's update_client_state
        let applied = session.clients().update_client_state(client_id, |editing_state| {
            match transition.clone() {
                ModeTransition::Push { mode, context } => {
                    tracing::debug!(?mode, ?context, %client_id, "Pushing mode (per-client)");
                    editing_state.mode_stack.push(mode);
                }

                ModeTransition::Pop { result: _ } => {
                    if editing_state.mode_stack.depth() > 1 {
                        let popped = editing_state.mode_stack.pop();
                        tracing::debug!(?popped, %client_id, "Popped mode (per-client)");
                        // Note: Pop result handling would need command execution
                        // which is done separately after mode transition
                    } else {
                        tracing::warn!(%client_id, "Cannot pop last mode from stack (per-client)");
                    }
                }

                ModeTransition::Set { mode, context } => {
                    tracing::debug!(?mode, ?context, %client_id, "Setting mode (per-client)");
                    // Pop to base then set
                    while editing_state.mode_stack.depth() > 1 {
                        editing_state.mode_stack.pop();
                    }
                    editing_state.mode_stack.set(mode);
                }
            }
        });

        if !applied {
            tracing::warn!(
                %client_id,
                "Failed to apply mode transition (client not found or is follower)"
            );
        }

        // Handle pop result if provided (Phase #471: use per-client state)
        // Returns StateChanges from command execution (e.g., buffer_modified from delete)
        let mut changes = if let ModeTransition::Pop {
            result: Some(pop_result),
        } = transition
        {
            Self::handle_pop_result_for_client(session, client_id, pop_result)
        } else {
            StateChanges::new()
        };

        // Deferred motion completion (#663): after a pop-result command (e.g.,
        // jump-execute) moves the cursor, pending operators (d/c/y) need their
        // on_command_complete callback invoked.  This mirrors the pattern at
        // handle_resolve_result lines 684-691.
        while let Some(complete_transition) =
            session.try_on_command_complete_for_client(client_id).await
        {
            session
                .clients()
                .update_client_state(client_id, |editing_state| match &complete_transition {
                    ModeTransition::Pop { .. } => {
                        if editing_state.mode_stack.depth() > 1 {
                            editing_state.mode_stack.pop();
                        }
                    }
                    ModeTransition::Push { mode, .. } => {
                        editing_state.mode_stack.push(mode.clone());
                    }
                    ModeTransition::Set { mode, .. } => {
                        while editing_state.mode_stack.depth() > 1 {
                            editing_state.mode_stack.pop();
                        }
                        editing_state.mode_stack.set(mode.clone());
                    }
                });

            // Pop with a result (e.g., delete operator returning ExecuteCommand for
            // the actual deletion): execute it and loop to check for further completions.
            // Push/Set means "wait for more input" — apply and exit the loop.
            if let ModeTransition::Pop {
                result: Some(nested_result),
            } = complete_transition
            {
                let nested_changes =
                    Self::handle_pop_result_for_client(session, client_id, nested_result);
                changes.merge(nested_changes);
            } else {
                break;
            }
        }

        changes
    }

    /// Handle a `PopResult` from a mode transition with per-client state (Phase #471).
    ///
    /// This ensures commands executed from pop results (like change operators)
    /// use per-client state for proper multi-client isolation.
    ///
    /// Returns `StateChanges` from command execution so the caller can merge
    /// them into accumulated changes for notification emission.
    fn handle_pop_result_for_client(
        session: &Session,
        client_id: ClientId,
        result: PopResult,
    ) -> StateChanges {
        let mut changes = StateChanges::new();

        match result {
            PopResult::ExecuteCommand { command, args } => {
                tracing::debug!(?command, %client_id, "Executing command from pop result (per-client)");
                let mut cmd_ctx = CommandContext::new();

                // Transfer all arguments directly (same ArgValue type on both sides)
                for (key, value) in args {
                    cmd_ctx.set(&key, value);
                }

                // Set active buffer ID (required for operators like delete/yank)
                // Per-client active_buffer (#471)
                if let Some(buffer_id) = session.clients().with_clients(|clients| {
                    clients.get(&client_id).and_then(|c| c.state.active_buffer)
                }) {
                    cmd_ctx.set_buffer_id(buffer_id);
                }

                // Phase #471/#479/#547: Execute with per-client state, log errors, process signals
                match session.execute_command_for_client(client_id, &command, &cmd_ctx) {
                    Some((CommandResult::Error(ref e), cmd_changes, signals)) => {
                        changes.merge(cmd_changes);
                        // Phase #479: Log command failure to ring buffer (visible, not silent)
                        session.with_client_ring_buffer(client_id, |rb| {
                            rb.log_event(
                                ClientEventType::Error,
                                format!("COMMAND_FAILED: cmd={command:?} error={e}"),
                            );
                        });
                        tracing::warn!(?command, %client_id, error = %e, "Command execution failed");
                        for signal in signals {
                            match signal {
                                reovim_subsys_command_types::RuntimeSignal::Quit => {
                                    tracing::info!(%client_id, "Client requested quit via RuntimeSignal");
                                    changes.record_quit_requested();
                                }
                            }
                        }
                    }
                    Some((_, cmd_changes, signals)) => {
                        changes.merge(cmd_changes);
                        for signal in signals {
                            match signal {
                                reovim_subsys_command_types::RuntimeSignal::Quit => {
                                    tracing::info!(%client_id, "Client requested quit via RuntimeSignal");
                                    changes.record_quit_requested();
                                }
                            }
                        }
                    }
                    None => {}
                }
                // None (client not found or following): already logged in execute_command_for_client
            }

            PopResult::Cancelled => {
                tracing::trace!("Mode transition cancelled");
            }

            PopResult::Data { values } => {
                // Data result from mode - modules handle this via extensions
                tracing::trace!(?values, "Mode returned data");
            }
        }

        changes
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

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
