//! Unified TUI application with common input/output bus.
//!
//! This module provides `TuiApp<O: TuiOutput>` — a single implementation
//! for both interactive and headless TUI modes.
//!
//! # Architecture
//!
//! ```text
//! TTY keyboard ──┐
//!                ├──► input_rx ──► TuiApp ──► FrameBuffer ──► output.flush()
//! TuiHandle ─────┘                              ↑              ↓
//!                                          render_frame()   TTY / no-op
//! ```
//!
//! - **Common Input**: Single `mpsc::Receiver<TuiInput>` channel.
//!   Interactive spawns a TTY reader task; headless uses only `TuiHandle`.
//! - **Common Output**: `FrameBuffer` always owned by `TuiApp`.
//!   Capture reads from it for both modes.
//! - **Output Trait**: `TuiOutput` handles only display I/O (flush, cursor).
//!   Headless impl is all no-ops.

use std::{collections::HashMap, io, sync::Arc, time::Duration};

use reovim_client_driver::{ClientModule, ClientModuleLoader, ServerHandle};

use crate::render_backend::RenderBackend as _;

use {
    crossterm::event::{KeyCode, KeyModifiers},
    reovim_driver_display::{
        AnnotationCacheManager, BuiltinTheme, FrameBuffer, ThemeLoader, ThemeManager, TokenSpan,
    },
    reovim_protocol::v2::{
        GetLayoutResponse, Notification, WindowInfo, WindowNode, WindowRect,
        option_changed_payload::Value as OptionValue,
    },
    tokio::{select, sync::mpsc, time::interval},
    tokio_stream::StreamMap,
    tonic::Streaming,
};

use crate::{
    LineNumberMode, RemoteClient, RenderConfig, TuiCoreState, TuiDebugConfig,
    grpc_client::{TuiGrpcClient, TuiGrpcError},
    handle::{ControlRequest, RawInputEvent, TuiHandle, TuiInput},
    layout_mirror::ServerLayoutMirror,
    notification_handler::{
        NotificationContext, NotificationResult, dispatch_buffer_list, dispatch_buffer_metadata,
        handle_notification,
    },
    render_backend::format_frame_buffer,
    render_engine::render_frame,
    tui_output::{CursorStyleHint, TuiOutput},
};

/// TUI application error.
#[derive(Debug)]
pub enum TuiAppError {
    /// I/O error.
    Io(io::Error),
    /// gRPC error.
    Grpc(TuiGrpcError),
    /// Server disconnected.
    Disconnected,
    /// Stream ended.
    StreamEnded,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl std::fmt::Display for TuiAppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Grpc(e) => write!(f, "gRPC error: {e}"),
            Self::Disconnected => write!(f, "Server disconnected"),
            Self::StreamEnded => write!(f, "Notification stream ended"),
        }
    }
}

impl std::error::Error for TuiAppError {}

#[cfg_attr(coverage_nightly, coverage(off))]
impl From<io::Error> for TuiAppError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl From<TuiGrpcError> for TuiAppError {
    fn from(e: TuiGrpcError) -> Self {
        Self::Grpc(e)
    }
}

// =============================================================================
// TuiApp
// =============================================================================

/// Unified TUI application.
///
/// Generic over `TuiOutput` to support both interactive and headless modes.
/// Uses the same event loop, notification handling, and rendering logic.
///
/// - `FrameBuffer` is always owned by `TuiApp` (common output surface)
/// - Input comes from a single `mpsc::Receiver<TuiInput>` channel
/// - The `O: TuiOutput` handles only display I/O (flush to terminal or no-op)
pub struct TuiApp<O: TuiOutput> {
    // === Common (same for both modes) ===
    /// Common output surface — all rendering targets this.
    frame_buffer: FrameBuffer,
    /// Common input channel — both TTY and `TuiHandle` feed here.
    input_rx: mpsc::Receiver<TuiInput>,
    /// gRPC client for server communication.
    client: TuiGrpcClient,
    /// Notification stream from server.
    notification_stream: Streaming<Notification>,
    /// Core TUI state (shared between modes).
    state: TuiCoreState,
    /// Whether the app is running.
    running: bool,
    /// Server address for display.
    server_address: String,
    /// Debug configuration.
    #[allow(dead_code)]
    debug_config: Option<TuiDebugConfig>,
    /// Server layout mirror.
    layout_mirror: ServerLayoutMirror,
    /// Syntax token cache manager.
    token_cache_manager: AnnotationCacheManager,
    /// Theme manager for syntax highlighting.
    theme_manager: ThemeManager,
    /// Theme loader for finding and loading theme files.
    theme_loader: ThemeLoader,
    /// Buffer IDs needing syntax token refresh (fallback for buffers without streams).
    pending_token_refresh: std::collections::HashSet<u64>,
    /// Active token update streams per buffer (#655).
    token_streams: StreamMap<u64, Streaming<reovim_protocol::v2::TokenUpdate>>,
    /// Whether display options need refresh.
    needs_display_options_refresh: bool,
    /// Client module loader — manages lifecycle and dependency order.
    module_loader: ClientModuleLoader,
    /// Server handle adapter for module `init()` calls.
    server_handle: Arc<dyn ServerHandle>,
    /// Platform capabilities (updated on resize, focus changes).
    capabilities: crate::render_engine_bridge::TuiPlatformCapabilities,

    // === I/O adapter (only thing that differs) ===
    /// Display output adapter (terminal for interactive, no-op for headless).
    output: O,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<O: TuiOutput> TuiApp<O> {
    /// Create a TUI app with the given output adapter and connection details.
    ///
    /// # Panics
    ///
    /// Panics if client module dependency resolution fails (cycle or missing dep).
    #[allow(clippy::too_many_arguments)]
    pub fn new<S: std::hash::BuildHasher>(
        output: O,
        frame_buffer: FrameBuffer,
        input_rx: mpsc::Receiver<TuiInput>,
        client: TuiGrpcClient,
        notification_stream: Streaming<Notification>,
        state: TuiCoreState,
        server_address: String,
        debug_config: Option<TuiDebugConfig>,
        initial_theme: Option<&str>,
        disabled_kinds: &std::collections::HashSet<String, S>,
    ) -> Self {
        let (width, height) = (state.width, state.height);

        // Create server layout mirror
        let layout_mirror = ServerLayoutMirror::new(width, height);

        // Create token cache and theme managers
        let token_cache_manager = AnnotationCacheManager::new();
        let theme_loader = ThemeLoader::new();
        let mut theme_manager = ThemeManager::new(BuiltinTheme::Dark.load());

        // Apply initial theme if specified
        if let Some(theme_name) = initial_theme {
            Self::apply_theme_internal(&theme_loader, &mut theme_manager, theme_name);
        }

        // Load client modules via factory map + dependency-resolving loader
        let factories = crate::static_client_modules::builtin_client_modules();
        let module_loader = ClientModuleLoader::new(factories, disabled_kinds)
            .expect("client module dependency resolution failed");

        // Create server handle adapter for module init() calls.
        // Clone the gRPC client so modules get their own handle while the app
        // keeps its owned client for async event loop calls.
        let server_handle: Arc<dyn ServerHandle> =
            Arc::new(crate::server_handle::TuiServerHandle::new(
                Arc::new(reovim_arch::sync::Mutex::new(client.clone())),
                tokio::runtime::Handle::current(),
            ));

        Self {
            frame_buffer,
            input_rx,
            client,
            notification_stream,
            state,
            running: true,
            server_address,
            debug_config,
            layout_mirror,
            token_cache_manager,
            theme_manager,
            theme_loader,
            pending_token_refresh: std::collections::HashSet::new(),
            token_streams: StreamMap::new(),
            needs_display_options_refresh: false,
            module_loader,
            server_handle,
            capabilities: crate::render_engine_bridge::TuiPlatformCapabilities::new(
                width,
                height,
                reovim_driver_display::DisplayCapabilities::detect(),
            ),
            output,
        }
    }

    /// Apply a theme by name.
    fn apply_theme_internal(loader: &ThemeLoader, manager: &mut ThemeManager, theme_name: &str) {
        // Try user theme first
        if let Ok(theme) = loader.load(theme_name) {
            manager.set_theme(theme);
            tracing::info!(theme = theme_name, "Applied user theme");
            return;
        }

        // Try builtin themes
        let builtin = match theme_name.to_lowercase().as_str() {
            "dark" => Some(BuiltinTheme::Dark),
            "light" => Some(BuiltinTheme::Light),
            "tokyo-night" | "tokyo-night-orange" | "tokyonight" => {
                Some(BuiltinTheme::TokyoNightOrange)
            }
            _ => None,
        };

        if let Some(theme) = builtin {
            manager.set_theme(theme.load());
            tracing::info!(theme = theme_name, "Applied builtin theme");
        } else {
            tracing::warn!(theme = theme_name, "Theme not found");
        }
    }

    /// Run the TUI event loop.
    ///
    /// # Errors
    ///
    /// Returns an error if the event loop fails.
    pub async fn run(&mut self) -> Result<(), TuiAppError> {
        // Initialize client modules before fetching state
        let theme_adapter =
            crate::render_engine_bridge::ThemeProviderAdapter::new(&self.theme_manager);
        let ctx = reovim_client_driver::ModuleContext {
            capabilities: &self.capabilities,
            server: Arc::clone(&self.server_handle),
            theme: &theme_adapter,
            services: None,
            module_registry: None,
        };
        let init_count = self.module_loader.init_all(&ctx);
        tracing::info!(init_count, "Client modules initialized");
        self.module_loader.on_all_loaded(&ctx);

        // Initial state fetch
        self.fetch_initial_state().await?;

        // Initial render
        self.state.set_needs_redraw(true);
        self.render()?;

        // Event loop
        let result = self.event_loop().await;

        // Shutdown modules in reverse dependency order
        self.module_loader.exit_all();

        result
    }

    // =========================================================================
    // Initial State
    // =========================================================================

    /// Fetch initial state from server.
    async fn fetch_initial_state(&mut self) -> Result<(), TuiAppError> {
        let client_id = self.state.my_client_id;

        // Get mode
        let mode_resp = self.client.get_mode_or_panic(client_id).await;
        self.state.mode_name = mode_resp.name;
        self.state.mode_display = mode_resp.display;
        self.state.set_insert_mode(mode_resp.is_insert);

        // Get cursor
        let cursor_resp = self.client.get_cursor_or_panic(None, client_id).await;
        if let Some(pos) = cursor_resp.position {
            self.state
                .update_local_cursor(cursor_resp.window_id, pos.line, pos.column);
        }

        // Get layout
        let layout_resp = self.client.get_layout_or_panic(client_id).await;
        self.apply_layout(&layout_resp);

        // Handle empty layout - create default local window
        if self.state.needs_default_window()
            && let Ok(active_buffer_resp) = self.client.get_active_buffer().await
            && let Some(buffer_id) = active_buffer_resp.buffer_id
        {
            self.create_default_window(buffer_id);
            tracing::info!(buffer_id, "Created default window for empty server layout");
        }

        // Get buffer content for each window
        self.fetch_buffer_contents().await?;

        // Fetch display options (line numbers)
        self.fetch_display_options().await;

        Ok(())
    }

    /// Fetch display-related options from server.
    async fn fetch_display_options(&mut self) {
        match self
            .client
            .get_options(vec!["number".to_string(), "relativenumber".to_string()])
            .await
        {
            Ok(opts) => {
                self.apply_display_options(&opts.options);
            }
            Err(e) => {
                tracing::debug!("Could not fetch display options: {e}");
            }
        }
    }

    /// Apply display options to state.
    fn apply_display_options(
        &mut self,
        options: &std::collections::HashMap<String, reovim_protocol::v2::OptionValue>,
    ) {
        use reovim_protocol::v2::option_value::Value;

        let number = options
            .get("number")
            .and_then(|v| {
                v.value.as_ref().and_then(|val| match val {
                    Value::BoolValue(b) => Some(*b),
                    _ => None,
                })
            })
            .unwrap_or(false);

        let relativenumber = options
            .get("relativenumber")
            .and_then(|v| {
                v.value.as_ref().and_then(|val| match val {
                    Value::BoolValue(b) => Some(*b),
                    _ => None,
                })
            })
            .unwrap_or(false);

        self.state.line_number_mode = match (number, relativenumber) {
            (true, true) => LineNumberMode::Hybrid,
            (true, false) => LineNumberMode::Absolute,
            (false, true) => LineNumberMode::Relative,
            (false, false) => LineNumberMode::None,
        };
    }

    /// Apply layout response to state.
    fn apply_layout(&mut self, layout: &GetLayoutResponse) {
        self.state.focused_window_id = layout.focused_window_id.unwrap_or(0);
        self.state.windows.clear();

        if let Some(root) = &layout.root {
            self.collect_windows(root);
        }

        self.state
            .set_needs_default_window(self.state.windows.is_empty());

        // Update layout mirror
        self.layout_mirror
            .apply_layout_changed(self.state.focused_window_id, &self.state.windows);
    }

    /// Create a default window view for empty layout.
    fn create_default_window(&mut self, buffer_id: u64) {
        let (width, height) = (self.state.width, self.state.height);
        let content_height = height.saturating_sub(1);

        let window = WindowInfo {
            window_id: 1,
            buffer_id: Some(buffer_id),
            rect: Some(WindowRect {
                x: 0,
                y: 0,
                width: u64::from(width),
                height: u64::from(content_height),
            }),
            focused: true,
            opacity: None,
        };

        self.state.windows.push(window);
        self.state.focused_window_id = 1;
        self.state.set_needs_default_window(false);

        // Update layout mirror
        self.layout_mirror
            .apply_layout_changed(1, &self.state.windows);

        tracing::debug!(buffer_id, "Created default window for empty layout");
    }

    /// Recursively collect windows from layout tree.
    fn collect_windows(&mut self, node: &WindowNode) {
        if let Some(n) = &node.node {
            match n {
                reovim_protocol::v2::window_node::Node::Leaf(leaf) => {
                    if leaf.buffer_id.is_some() {
                        self.state.windows.push(WindowInfo {
                            window_id: leaf.window_id,
                            buffer_id: leaf.buffer_id,
                            rect: leaf.rect,
                            focused: leaf.window_id == self.state.focused_window_id,
                            opacity: None,
                        });
                    }
                }
                reovim_protocol::v2::window_node::Node::Split(split) => {
                    for child in &split.children {
                        self.collect_windows(child);
                    }
                }
            }
        }
    }

    /// Fetch buffer content for all visible windows.
    async fn fetch_buffer_contents(&mut self) -> Result<(), TuiAppError> {
        let buffer_ids: Vec<u64> = self
            .state
            .windows
            .iter()
            .filter_map(|w| w.buffer_id)
            .collect();

        for buffer_id in buffer_ids {
            if !self.state.buffer_cache.contains_key(&buffer_id) {
                let content = self
                    .client
                    .get_buffer_content(Some(buffer_id), None, None)
                    .await?;
                self.state.buffer_cache.insert(buffer_id, content.lines);

                // Fetch initial tokens for syntax highlighting
                self.fetch_tokens_for_buffer(buffer_id).await;
            }
        }

        Ok(())
    }

    /// Fetch syntax tokens for a buffer.
    async fn fetch_tokens_for_buffer(&mut self, buffer_id: u64) {
        let content = self
            .state
            .buffer_cache
            .get(&buffer_id)
            .map(|lines| lines.join("\n"))
            .unwrap_or_default();

        match self.client.get_tokens(buffer_id, None, None).await {
            Ok(response) => {
                let token_spans: Vec<TokenSpan> = response
                    .tokens
                    .into_iter()
                    .map(|t| TokenSpan {
                        start_byte: t.start_byte,
                        end_byte: t.end_byte,
                        category: t.category,
                    })
                    .collect();

                self.token_cache_manager.apply_token_update(
                    buffer_id,
                    &token_spans,
                    0,
                    u64::MAX,
                    true,
                    &content,
                    "syntax",
                    0,
                );

                tracing::debug!(buffer_id, token_count = token_spans.len(), "Cached syntax tokens");

                // Subscribe to real-time token updates (#655)
                if !self.token_streams.contains_key(&buffer_id) {
                    match self.client.stream_tokens(buffer_id).await {
                        Ok(stream) => {
                            self.token_streams.insert(buffer_id, stream);
                            tracing::debug!(buffer_id, "Subscribed to token stream");
                        }
                        Err(e) => {
                            tracing::debug!(buffer_id, error = %e, "Could not subscribe to token stream");
                        }
                    }
                }
            }
            Err(e) => {
                tracing::debug!(buffer_id, error = %e, "Could not fetch tokens");
            }
        }
    }

    /// Apply a colorscheme by name.
    fn apply_colorscheme(&mut self, name: &str) {
        Self::apply_theme_internal(&self.theme_loader, &mut self.theme_manager, name);
    }

    // =========================================================================
    // Event Loop
    // =========================================================================

    /// Main event loop — single input source for all events.
    async fn event_loop(&mut self) -> Result<(), TuiAppError> {
        let mut redraw_timer = interval(Duration::from_millis(16)); // 60 FPS max

        while self.running {
            // Biased select: notification stream is prioritized over token
            // stream so BufferModified updates buffer_cache before TokenUpdate
            // tries to convert byte offsets to line/column positions.
            select! {
                biased;

                // Single input source: both TTY and TuiHandle feed here.
                // None means all senders dropped → graceful shutdown.
                result = self.input_rx.recv() => {
                    match result {
                        Some(input) => self.handle_input(input).await?,
                        None => { self.running = false; }
                    }
                }

                // Server notifications (both modes)
                notification = self.notification_stream.message() => {
                    match notification {
                        Ok(Some(notif)) => {
                            self.handle_server_notification(notif).await?;

                            // Process deferred refreshes
                            if !self.pending_token_refresh.is_empty() {
                                let buffer_ids: Vec<u64> =
                                    self.pending_token_refresh.drain().collect();
                                for buffer_id in buffer_ids {
                                    self.fetch_tokens_for_buffer(buffer_id).await;
                                }
                            }
                            if self.needs_display_options_refresh {
                                self.fetch_display_options().await;
                                self.needs_display_options_refresh = false;
                            }
                        }
                        Ok(None) => return Err(TuiAppError::StreamEnded),
                        Err(e) => return Err(TuiAppError::Grpc(e.into())),
                    }
                }

                // Token stream updates (#655) — real-time syntax tokens
                Some((buffer_id, update_result)) = tokio_stream::StreamExt::next(&mut self.token_streams) => {
                    match update_result {
                        Ok(update) => {
                            let content = self
                                .state
                                .buffer_cache
                                .get(&buffer_id)
                                .map(|lines| lines.join("\n"))
                                .unwrap_or_default();

                            let token_spans: Vec<TokenSpan> = update
                                .tokens
                                .into_iter()
                                .map(|t| TokenSpan {
                                    start_byte: t.start_byte,
                                    end_byte: t.end_byte,
                                    category: t.category,
                                })
                                .collect();

                            self.token_cache_manager.apply_token_update(
                                buffer_id,
                                &token_spans,
                                update.start_line,
                                update.end_line,
                                update.full_refresh,
                                &content,
                                &update.layer,
                                update.priority,
                            );

                            self.state.set_needs_redraw(true);
                        }
                        Err(e) => {
                            tracing::debug!(buffer_id, error = %e, "Token stream error, removing");
                        }
                    }
                }

                // Redraw timer (both modes)
                _ = redraw_timer.tick() => {
                    // Tick extensions (e.g., which-key show-delay)
                    for ext in self.module_loader.modules_mut() {
                        if ext.tick() {
                            self.state.set_needs_redraw(true);
                        }
                    }

                    if self.state.needs_redraw() {
                        self.render()?;
                        self.state.set_needs_redraw(false);
                    }
                }
            }
        }
        Ok(())
    }

    // =========================================================================
    // Input Handling (unified — nested dispatch)
    // =========================================================================

    /// Handle any input from the common channel.
    async fn handle_input(&mut self, input: TuiInput) -> Result<(), TuiAppError> {
        match input {
            TuiInput::Raw(raw) => self.handle_raw_input(raw).await,
            TuiInput::Control(ctrl) => self.handle_control(ctrl).await,
        }
    }

    /// Handle raw hardware events (TTY keyboard/mouse).
    async fn handle_raw_input(&mut self, event: RawInputEvent) -> Result<(), TuiAppError> {
        match event {
            RawInputEvent::Key(key_event) => {
                // Ctrl+Q to quit
                if key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && key_event.code == KeyCode::Char('q')
                {
                    self.running = false;
                    return Ok(());
                }

                // Send vim notation to server (identity from token, #483)
                if !key_event.vim_notation.is_empty() {
                    match self.client.send_keys(&key_event.vim_notation).await {
                        Ok(resp) if resp.should_quit => {
                            tracing::info!("Server signaled quit via SendKeysResponse");
                            self.running = false;
                        }
                        Err(e) => {
                            self.state.last_error = Some(format!("Send keys failed: {e}"));
                        }
                        _ => {}
                    }
                }
            }
            RawInputEvent::Resize(width, height) => {
                self.do_resize(width, height).await?;
            }
            RawInputEvent::Mouse(_) => {
                // Mouse support can be added later
            }
        }
        Ok(())
    }

    /// Handle control commands from `TuiHandle` (works for BOTH modes).
    async fn handle_control(&mut self, request: ControlRequest) -> Result<(), TuiAppError> {
        match request {
            ControlRequest::SendKeys { keys, response } => {
                let result = self.client.send_keys(&keys).await;
                match &result {
                    Ok(resp) if resp.should_quit => {
                        self.running = false;
                    }
                    _ => {}
                }
                let _ = response.send(result.is_ok_and(|r| r.ok));
            }
            ControlRequest::Capture { format, response } => {
                let content = self.capture_frame(&format);
                let _ = response.send(content);
            }
            ControlRequest::Resize {
                width,
                height,
                response,
            } => {
                self.do_resize(width, height).await?;
                let _ = response.send(());
            }
            ControlRequest::Stop => {
                self.running = false;
            }
        }
        Ok(())
    }

    /// Shared resize logic (used by both raw input and control requests).
    async fn do_resize(&mut self, width: u16, height: u16) -> Result<(), TuiAppError> {
        self.frame_buffer.resize(width, height);
        self.output.invalidate();
        self.state.width = width;
        self.state.height = height;
        self.layout_mirror.set_screen(width, height);

        // Update platform capabilities and notify all modules
        self.capabilities.update_grid_size(width, height);
        for ext in self.module_loader.modules_mut() {
            ext.on_capabilities_changed(&self.capabilities);
        }

        if let Err(e) = self
            .client
            .resize(u64::from(width), u64::from(height))
            .await
        {
            self.state.last_error = Some(format!("Resize failed: {e}"));
        }

        self.state.set_needs_redraw(true);
        self.render()?;
        Ok(())
    }

    // =========================================================================
    // Notifications
    // =========================================================================

    /// Handle server notification.
    async fn handle_server_notification(&mut self, notif: Notification) -> Result<(), TuiAppError> {
        let needs_metadata = match handle_notification(self, notif).await {
            Ok(NotificationResult::RedrawWithMetadata) => {
                self.state.set_needs_redraw(true);
                true
            }
            Ok(NotificationResult::Redraw) => {
                self.state.set_needs_redraw(true);
                false
            }
            Ok(NotificationResult::NoRedraw) => false,
            Ok(NotificationResult::Stop) => {
                self.running = false;
                false
            }
            Err(e) => {
                tracing::warn!(error = %e, "Notification handling failed");
                false
            }
        };

        // Update layout mirror after any notification
        self.layout_mirror
            .apply_layout_changed(self.state.focused_window_id, &self.state.windows);

        // Dispatch buffer metadata only when buffer/layout changed (#691)
        if needs_metadata {
            let exts = self.module_loader.modules_mut_slice();
            dispatch_buffer_metadata(&self.state, &mut self.client, exts).await;
            dispatch_buffer_list(&self.state, &mut self.client, exts).await;
        }

        Ok(())
    }

    // =========================================================================
    // Rendering (always to FrameBuffer)
    // =========================================================================

    /// Render the current state.
    #[allow(clippy::result_large_err)]
    fn render(&mut self) -> Result<(), TuiAppError> {
        let (width, height) = (self.frame_buffer.width(), self.frame_buffer.height());
        if width == 0 || height == 0 {
            return Ok(());
        }

        // Build render config
        let total_lines = self
            .state
            .windows
            .first()
            .and_then(|w| w.buffer_id)
            .and_then(|bid| self.state.buffer_cache.get(&bid))
            .map_or(0, Vec::len);

        let config = RenderConfig {
            show_line_numbers: self.state.line_number_mode != LineNumberMode::None,
            line_number_mode: self.state.line_number_mode,
            render_self_cursor: !self.output.uses_terminal_cursor(),
            gutter_width: self.calculate_gutter_width(total_lines),
            opacity: 1.0, // TODO(#400): read from server layout when compositor is wired
        };

        // Compute scroll_top for all visible windows using their actual height.
        if self.layout_mirror.has_multiple_windows() {
            for placement in self.layout_mirror.placements() {
                self.state
                    .compute_scroll_top(placement.window_id, placement.height);
            }
        } else {
            let content_height = height.saturating_sub(1);
            self.state
                .compute_scroll_top(self.state.focused_window_id, content_height);
        }

        // Always render to FrameBuffer (common output)
        render_frame(
            &mut self.frame_buffer,
            &self.state,
            &config,
            self.module_loader.modules(),
            &self.token_cache_manager,
            &self.theme_manager,
            &self.layout_mirror,
        );

        // Flush to display (terminal for interactive, no-op for headless)
        self.output.flush(&self.frame_buffer)?;

        // Position hardware cursor (interactive only)
        if self.output.uses_terminal_cursor() {
            self.position_cursor();
        }

        Ok(())
    }

    /// Calculate gutter width for line numbers.
    fn calculate_gutter_width(&self, total_lines: usize) -> u16 {
        if self.state.line_number_mode == LineNumberMode::None {
            return 0;
        }

        let digits = if total_lines == 0 {
            1
        } else {
            #[allow(
                clippy::cast_precision_loss,
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss
            )]
            let d = ((total_lines as f64).log10().floor() as u16) + 1;
            d
        };
        digits + 2
    }

    /// Position cursor (for interactive mode).
    fn position_cursor(&mut self) {
        // Check if any chrome extension wants cursor positioning
        let (width, height) = self.frame_buffer.size();
        for ext in self.module_loader.modules() {
            if ext.has_chrome()
                && let Some((cx, cy)) = ext.cursor_position(width, height)
            {
                self.output.position_cursor(cx, cy);
                self.output.set_cursor_style(CursorStyleHint::Bar);
                self.output.set_cursor_visible(true);
                return;
            }
        }

        let Some(cursor_pos) = self.state.get_focused_cursor() else {
            return;
        };

        let focused_info = self.state.windows.iter().find(|w| w.focused).and_then(|w| {
            w.rect.as_ref().map(|rect| {
                let total_lines = w
                    .buffer_id
                    .and_then(|bid| self.state.buffer_cache.get(&bid))
                    .map_or(0, Vec::len);
                (*rect, total_lines)
            })
        });

        if let Some((rect, total_lines)) = focused_info {
            let gutter_width = self.calculate_gutter_width(total_lines);
            let scroll_top = self.state.get_focused_scroll_top();

            // Count virtual lines from extensions between scroll_top and cursor
            #[allow(clippy::cast_possible_truncation)]
            let cursor_line_idx = cursor_pos.line as usize;
            let virtual_count: usize = self
                .module_loader
                .modules()
                .iter()
                .filter(|e| e.has_buffer_contrib())
                .flat_map(|e| e.virtual_lines())
                .filter(|vl| vl.buffer_line >= scroll_top && vl.buffer_line <= cursor_line_idx)
                .count();

            #[allow(clippy::cast_possible_truncation)]
            let cursor_y = rect.y as u16
                + (cursor_pos.line as usize).saturating_sub(scroll_top) as u16
                + virtual_count as u16;

            // Extension column mapping (e.g., table expanded columns)
            let buffer_id = self.state.get_focused_buffer_id().unwrap_or(0);
            #[allow(clippy::cast_possible_truncation)]
            let bid = reovim_client_driver::BufferId(buffer_id as usize);
            #[allow(clippy::cast_possible_truncation)]
            let cursor_x = self
                .module_loader
                .modules()
                .iter()
                .filter(|e| e.has_buffer_contrib())
                .find_map(|e| {
                    e.map_cursor_column(bid, cursor_pos.line as usize, cursor_pos.column as usize)
                })
                .map_or_else(
                    || rect.x as u16 + gutter_width + cursor_pos.column as u16,
                    |col| rect.x as u16 + gutter_width + col,
                );

            self.output.position_cursor(cursor_x, cursor_y);

            // Set cursor style based on mode
            let style = self.cursor_style_for_mode();
            self.output.set_cursor_style(style);
            self.output.set_cursor_visible(true);
        }
    }

    /// Get cursor style based on current mode.
    fn cursor_style_for_mode(&self) -> CursorStyleHint {
        let mode = self.state.mode_name.to_lowercase();
        match mode.as_str() {
            "insert" | "i" | "command" | ":" | "cmdline" => CursorStyleHint::Bar,
            "replace" | "r" => CursorStyleHint::Underline,
            _ => CursorStyleHint::Block,
        }
    }

    /// Capture current frame — works for BOTH modes.
    fn capture_frame(&self, format: &str) -> String {
        format_frame_buffer(&self.frame_buffer, format)
    }

    // =========================================================================
    // Public accessors
    // =========================================================================

    /// Get the server address.
    #[must_use]
    pub fn server_address(&self) -> &str {
        &self.server_address
    }

    /// Check if the app is still running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Stop the app.
    #[allow(clippy::missing_const_for_fn)] // &mut self cannot be const
    pub fn stop(&mut self) {
        self.running = false;
    }
}

// =============================================================================
// NotificationContext Implementation
// =============================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
impl<O: TuiOutput> NotificationContext for TuiApp<O> {
    fn state_mut(&mut self) -> &mut TuiCoreState {
        &mut self.state
    }

    fn client_mut(&mut self) -> &mut TuiGrpcClient {
        &mut self.client
    }

    fn on_buffer_modified(&mut self, buffer_id: u64) {
        // Skip poll-based refresh if we have an active stream for this buffer (#655)
        if !self.token_streams.contains_key(&buffer_id) {
            self.pending_token_refresh.insert(buffer_id);
        }
    }

    fn on_option_changed(&mut self, name: &str, value: Option<OptionValue>) {
        match name {
            "colorscheme" => {
                if let Some(OptionValue::StringValue(theme_name)) = value {
                    self.apply_colorscheme(&theme_name);
                }
            }
            "number" | "relativenumber" => {
                self.needs_display_options_refresh = true;
            }
            _ => {}
        }
    }

    fn on_resize(&mut self, width: u16, height: u16) {
        self.frame_buffer.resize(width, height);
        self.output.invalidate();
    }

    fn extensions_mut(&mut self) -> &mut [Box<dyn ClientModule>] {
        self.module_loader.modules_mut_slice()
    }

    fn on_capture_request(
        &mut self,
        _request_id: u64,
        format: &str,
        target_client_id: u64,
    ) -> Option<String> {
        if target_client_id != self.state.my_client_id {
            return None;
        }
        Some(self.capture_frame(format))
    }
}

// =============================================================================
// Connect Functions (Symmetric)
// =============================================================================

use crate::output::{HeadlessOutput, TerminalOutput};

/// Connect to a gRPC server and create an interactive TUI.
///
/// Spawns a TTY reader task that feeds keyboard input to the common channel.
/// Returns the app (caller runs it) and a handle for programmatic control.
///
/// # Errors
///
/// Returns an error if connection or terminal initialization fails.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn connect_interactive<S: std::hash::BuildHasher + Send + Sync>(
    addr: &str,
    debug_config: Option<TuiDebugConfig>,
    theme: Option<&str>,
    disabled_kinds: &std::collections::HashSet<String, S>,
) -> Result<(TuiApp<TerminalOutput>, TuiHandle), TuiAppError> {
    let output = TerminalOutput::new()?;
    let (width, height) = TerminalOutput::terminal_size()?;

    let (input_tx, input_rx) = mpsc::channel(16);

    // Spawn TTY reader task that feeds keyboard input to the common channel
    let _tty_reader = crate::handle::spawn_tty_reader(input_tx.clone());

    let (client, notification_stream, state) = connect_common(addr, width, height).await?;
    let frame_buffer = FrameBuffer::new(width, height);

    let app = TuiApp::new(
        output,
        frame_buffer,
        input_rx,
        client,
        notification_stream,
        state,
        addr.to_string(),
        debug_config,
        theme,
        disabled_kinds,
    );

    let handle = TuiHandle::new(input_tx);
    Ok((app, handle))
}

/// Connect to a gRPC server and create a headless TUI.
///
/// No TTY reader is spawned. The `TuiHandle` is the only input source.
/// The app is NOT spawned — caller decides lifecycle.
///
/// # Errors
///
/// Returns an error if connection fails.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn connect_headless<S: std::hash::BuildHasher + Send + Sync>(
    addr: &str,
    width: u16,
    height: u16,
    debug_config: Option<TuiDebugConfig>,
    theme: Option<&str>,
    disabled_kinds: &std::collections::HashSet<String, S>,
) -> Result<(TuiApp<HeadlessOutput>, TuiHandle), TuiAppError> {
    let output = HeadlessOutput;

    let (input_tx, input_rx) = mpsc::channel(16);

    let (client, notification_stream, state) = connect_common(addr, width, height).await?;
    let frame_buffer = FrameBuffer::new(width, height);

    let app = TuiApp::new(
        output,
        frame_buffer,
        input_rx,
        client,
        notification_stream,
        state,
        addr.to_string(),
        debug_config,
        theme,
        disabled_kinds,
    );

    let handle = TuiHandle::new(input_tx);
    Ok((app, handle))
}

/// Common connection logic for both modes.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn connect_common(
    addr: &str,
    width: u16,
    height: u16,
) -> Result<(TuiGrpcClient, Streaming<Notification>, TuiCoreState), TuiAppError> {
    // Connect gRPC client
    let mut client = TuiGrpcClient::connect(addr).await?;

    // Join presence session FIRST to get session token.
    // subscribe_all() and resize() must come AFTER join so the token is
    // attached to requests. Without the token, the notification stream
    // won't be wrapped in CleanupStream and disconnect cleanup won't run.
    let display_name = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .map_or_else(|_| "TUI Client".to_string(), |user| format!("TUI@{user}"));
    let join_resp = client.presence_join("tui", &display_name).await?;

    // Subscribe to all notifications (token now set by presence_join)
    let notification_stream = client.subscribe_all().await?;

    // Notify server of viewport size (token now attached via make_request)
    client.resize(u64::from(width), u64::from(height)).await?;
    let my_client_id = join_resp.client_id;
    tracing::info!(client_id = my_client_id, display_name, "Joined presence session");

    // Initialize other_clients from JoinResponse.peers_v2
    let other_clients: HashMap<u64, RemoteClient> = join_resp
        .peers_v2
        .into_iter()
        .filter(|peer| peer.id != my_client_id)
        .map(|peer| {
            let (cursor_line, cursor_col) = peer
                .view
                .as_ref()
                .and_then(|v| v.cursor.as_ref())
                .map_or((0, 0), |c| (c.line, c.column));

            let buffer_id = peer.view.as_ref().and_then(|v| v.buffer_id);
            let mode = peer
                .view
                .as_ref()
                .map(|v| v.mode.clone())
                .unwrap_or_default();
            let display_name = peer
                .metadata
                .map_or_else(|| format!("Client {}", peer.id), |m| m.display_name);

            (
                peer.id,
                RemoteClient {
                    client_id: peer.id,
                    display_name,
                    cursor_line,
                    cursor_col,
                    buffer_id,
                    mode,
                    selection: None,
                },
            )
        })
        .collect();

    // Build state with client ID and viewport size
    let mut state = TuiCoreState::new_with_size(my_client_id, width, height);
    state.other_clients = other_clients;

    Ok((client, notification_stream, state))
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
