//! gRPC v2 TUI application.
//!
//! A clean TUI implementation using only gRPC v2 protocol.
//! Follows mechanism/policy separation - server provides raw data,
//! client renders locally.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  TuiAppV2 (this module)                          POLICY     │
//! │    - Client-side rendering decisions                        │
//! │    - Layout and styling                                     │
//! ├─────────────────────────────────────────────────────────────┤
//! │  TUI Driver (lib/drivers/tui/)                   MECHANISM  │
//! │    - Terminal session (raw mode, alternate screen)          │
//! │    - Input event stream                                     │
//! │    - Screen rendering primitives                            │
//! ├─────────────────────────────────────────────────────────────┤
//! │  TuiGrpcClient (grpc_client.rs)                  TRANSPORT  │
//! │    - gRPC v2 service calls                                  │
//! │    - Streaming notifications                                │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Server (lib/server/)                            MECHANISM  │
//! │    - Raw buffer content                                     │
//! │    - Layout, mode, cursor state                             │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use std::{collections::HashMap, io, time::Duration};

use {
    crossterm::event::{KeyCode, KeyModifiers},
    reovim_arch::Color,
    reovim_driver_display::{
        BuiltinTheme, ThemeLoader, ThemeManager, TokenCacheManager, TokenSpan,
    },
    reovim_driver_tui::{Cursor, CursorStyle, InputEvent, InputReader, Screen, Style, Terminal},
    reovim_protocol::v2::{
        GetLayoutResponse, Notification, WindowInfo, WindowNode, WindowRect, notification::Payload,
        option_changed_payload::Value as OptionValue,
    },
    tokio::{select, time::interval},
    tonic::Streaming,
};

use crate::{
    TuiDebugConfig,
    grpc_client::{TuiGrpcClient, TuiGrpcError},
    layout_mirror::ServerLayoutMirror,
};

/// Line number display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineNumberMode {
    /// No line numbers.
    #[default]
    None,
    /// Absolute line numbers (1, 2, 3...).
    Absolute,
    /// Relative line numbers (distance from cursor).
    Relative,
    /// Hybrid: absolute for cursor line, relative for others.
    Hybrid,
}

/// TUI application error.
#[derive(Debug)]
pub enum TuiAppV2Error {
    /// I/O error.
    Io(io::Error),
    /// gRPC error.
    Grpc(TuiGrpcError),
    /// Server disconnected.
    Disconnected,
    /// Stream ended.
    StreamEnded,
}

impl std::fmt::Display for TuiAppV2Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Grpc(e) => write!(f, "gRPC error: {e}"),
            Self::Disconnected => write!(f, "Server disconnected"),
            Self::StreamEnded => write!(f, "Notification stream ended"),
        }
    }
}

impl std::error::Error for TuiAppV2Error {}

impl From<io::Error> for TuiAppV2Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<TuiGrpcError> for TuiAppV2Error {
    fn from(e: TuiGrpcError) -> Self {
        Self::Grpc(e)
    }
}

/// Client role in a multi-client session (Phase 11.2).
///
/// Determines how this client's input is routed:
/// - `Owner`: Has own independent state (cursor, mode, etc.)
/// - `Follow`: Read-only spectator of another client
/// - `Share`: Bidirectional editing with another client's state
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ClientRole {
    /// Owns editing state (default for new clients).
    #[default]
    Owner,
    /// Read-only spectator of target client.
    /// Constructed via `SetRole` RPC (Phase 9).
    #[allow(dead_code)]
    Follow,
    /// Shares state with owner (pair programming).
    /// Constructed via `SetRole` RPC (Phase 9).
    #[allow(dead_code)]
    Share,
}

impl ClientRole {
    /// Returns the display string for the statusline.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "Owner",
            Self::Follow => "Follow",
            Self::Share => "Share",
        }
    }
}

/// Presence information for a remote client (Phase 11.2).
///
/// Tracks other clients' cursor positions for awareness rendering.
#[derive(Debug, Clone)]
struct RemoteClient {
    /// Client's unique ID.
    /// Reserved for future use (showing client list, identifying for follow/share).
    #[allow(dead_code)]
    client_id: u64,
    /// User-friendly display name (e.g., "TUI@laptop").
    /// Reserved for future use (showing name label next to cursor).
    #[allow(dead_code)]
    display_name: String,
    /// Cursor line (0-indexed).
    cursor_line: u64,
    /// Cursor column (0-indexed).
    cursor_col: u64,
    /// Buffer ID the client is viewing (None if no buffer assigned).
    /// Phase #479: Changed to Option to eliminate ID ambiguity.
    buffer_id: Option<u64>,
    /// Current mode name.
    /// Reserved for future use (showing mode indicator next to cursor).
    #[allow(dead_code)]
    mode: String,
    /// Selection state for visual mode (Phase 14, #471).
    /// Used to render remote clients' selections.
    selection: Option<SelectionState>,
}

/// Cursor position for per-window tracking (Phase 8 #465).
///
/// Uses u64 to match protobuf `CursorMovedPayload` types.
#[derive(Debug, Clone, Copy, Default)]
struct CursorPosition {
    /// Line number (0-indexed).
    line: u64,
    /// Column number (0-indexed).
    column: u64,
}

/// Selection state for per-window tracking (Phase 8 #465).
///
/// Tracks the visual selection range for highlighting in the TUI.
#[derive(Debug, Clone, Default)]
struct SelectionState {
    /// Start position of selection.
    start: CursorPosition,
    /// End position of selection (exclusive).
    end: CursorPosition,
    /// Visual mode type (char, line, block).
    mode: String,
}

/// TUI state tracked locally from server notifications.
#[derive(Debug, Default)]
struct TuiState {
    /// Current mode name (internal).
    mode_name: String,
    /// Current mode display string.
    mode_display: String,
    /// Whether mode accepts text input.
    is_insert_mode: bool,
    /// Cursor line (0-indexed) - legacy global for statusline.
    cursor_line: u64,
    /// Cursor column (0-indexed) - legacy global for statusline.
    cursor_col: u64,
    /// Per-window cursor positions (Phase 8 #465).
    ///
    /// Maps `window_id` -> cursor position. Updated from `CursorMoved`
    /// notifications that include `window_id`.
    window_cursors: HashMap<u64, CursorPosition>,
    /// Per-window selection state (Phase 8 #465).
    ///
    /// Maps `window_id` -> selection. Updated from `SelectionChanged`
    /// notifications. Used to render visual selection highlighting.
    window_selections: HashMap<u64, SelectionState>,
    /// Focused window ID.
    focused_window_id: u64,
    /// Window layout info.
    windows: Vec<WindowInfo>,
    /// Whether screen needs redraw.
    needs_redraw: bool,
    /// Last error message for statusline.
    last_error: Option<String>,
    /// Buffer content cache (`buffer_id` -> lines).
    buffer_cache: HashMap<u64, Vec<String>>,
    /// Line number display mode.
    line_number_mode: LineNumberMode,
    /// Per-window viewport scroll (`window_id` -> `top_line`).
    /// TODO: Implement viewport scrolling per window
    #[allow(dead_code)]
    viewport_scroll: HashMap<u64, usize>,
    /// Whether client needs to create a default window (empty server layout).
    needs_default_window: bool,
    /// This client's unique ID (Phase 11.2 - per-client state).
    ///
    /// Assigned by `presence_join()` on connect. CRITICAL: All `SendKeys`
    /// requests must include this ID, otherwise all clients share state.
    /// Type is `u64` (not `Option`) because clients ALWAYS have an ID after join.
    my_client_id: u64,
    /// This client's role in the session (Phase 11.2).
    my_role: ClientRole,
    /// Other connected clients for awareness rendering (Phase 11.2).
    ///
    /// Maps `client_id` -> `RemoteClient`. Used to render other clients' cursors.
    other_clients: HashMap<u64, RemoteClient>,
}

/// gRPC v2 TUI application.
///
/// Uses streaming notifications for real-time updates and
/// client-side rendering for full policy control.
pub struct TuiAppV2 {
    /// gRPC client for server communication.
    client: TuiGrpcClient,
    /// Notification stream from server.
    notification_stream: Streaming<Notification>,
    /// Terminal session.
    terminal: Terminal,
    /// Screen buffer and renderer.
    screen: Screen,
    /// Cursor manager.
    cursor: Cursor,
    /// Input event reader.
    input: InputReader,
    /// Current TUI state.
    state: TuiState,
    /// Whether the app is running.
    running: bool,
    /// Server address for display.
    server_address: String,
    /// Debug configuration.
    #[allow(dead_code)]
    debug_config: Option<TuiDebugConfig>,
    /// Server layout mirror (Phase 11.2).
    ///
    /// Passive data structure that mirrors server-managed window layout.
    /// Updated from `layout_changed` notifications, used for rendering.
    layout_mirror: ServerLayoutMirror,
    /// Syntax token cache manager (Phase 13.0).
    ///
    /// Caches syntax tokens from `GetTokens` RPC for syntax highlighting.
    /// Per-buffer caches with byte-to-position conversion.
    /// TODO: Add real-time streaming via `StreamTokens` in Phase 13.1
    token_cache_manager: TokenCacheManager,
    /// Theme manager for syntax highlighting (Phase 13.0).
    ///
    /// Maps token categories to styles using the current theme.
    /// Supports hierarchical fallback (e.g., `keyword.control` → `keyword`).
    theme_manager: ThemeManager,
    /// Theme loader for finding and loading theme files (Phase 13.0).
    ///
    /// Searches `~/.config/reovim/themes/` and system paths for TOML theme files.
    theme_loader: ThemeLoader,
}

impl TuiAppV2 {
    /// Connect to a gRPC server and create a new TUI app.
    ///
    /// # Arguments
    ///
    /// * `addr` - Server address in `host:port` format.
    /// * `debug_config` - Optional debug configuration.
    /// * `initial_theme` - Optional initial theme name.
    ///
    /// # Errors
    ///
    /// Returns an error if connection fails.
    pub async fn connect(
        addr: &str,
        debug_config: Option<TuiDebugConfig>,
        initial_theme: Option<&str>,
    ) -> Result<Self, TuiAppV2Error> {
        // Connect gRPC client
        let mut client = TuiGrpcClient::connect(addr).await?;

        // Subscribe to all notifications
        let notification_stream = client.subscribe_all().await?;

        // Enter terminal session
        let terminal = Terminal::enter()?;

        // Get terminal size
        let (width, height) = Terminal::size()?;

        // Notify server of viewport size
        client.resize(u64::from(width), u64::from(height)).await?;

        // Join presence session to get unique client ID (Phase 11.2)
        // CRITICAL: Without this, all clients share state as ClientId(0)
        let display_name = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .map_or_else(|_| "TUI Client".to_string(), |user| format!("TUI@{user}"));
        let join_resp = client.presence_join("tui", &display_name).await?;
        let my_client_id = join_resp.client_id;
        tracing::info!(client_id = my_client_id, display_name, "Joined presence session");

        // Create screen and cursor
        let screen = Screen::new(width, height);
        let cursor = Cursor::new();
        let input = InputReader::new();

        // Create server layout mirror (Phase 11.2)
        let layout_mirror = ServerLayoutMirror::new(width, height);

        // Create token cache manager (Phase 13.0)
        let token_cache_manager = TokenCacheManager::new();

        // Create theme loader and manager (Phase 13.0)
        let theme_loader = ThemeLoader::new();
        let mut theme_manager = ThemeManager::new(BuiltinTheme::Dark.load());

        // Apply initial theme if specified
        if let Some(theme_name) = initial_theme {
            // Try user theme first
            if let Ok(theme) = theme_loader.load(theme_name) {
                theme_manager.set_theme(theme);
                tracing::info!(theme = theme_name, "Applied initial user theme");
            } else {
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
                    theme_manager.set_theme(theme.load());
                    tracing::info!(theme = theme_name, "Applied initial builtin theme");
                } else {
                    tracing::warn!(theme = theme_name, "Initial theme not found, using dark");
                }
            }
        }

        // Build state with client ID
        let state = TuiState {
            my_client_id,
            ..TuiState::default()
        };

        Ok(Self {
            client,
            notification_stream,
            terminal,
            screen,
            cursor,
            input,
            state,
            running: true,
            server_address: addr.to_string(),
            debug_config,
            layout_mirror,
            token_cache_manager,
            theme_manager,
            theme_loader,
        })
    }

    /// Run the TUI event loop.
    ///
    /// # Errors
    ///
    /// Returns an error if the event loop fails.
    pub async fn run(&mut self) -> Result<(), TuiAppV2Error> {
        // Initial state fetch
        self.fetch_initial_state().await?;

        // Initial render
        self.state.needs_redraw = true;
        self.render()?;

        // Event loop
        self.event_loop().await
    }

    /// Fetch initial state from server.
    ///
    /// # Phase #479: Fail-Loud Policy
    ///
    /// Uses `*_or_panic()` methods for critical state lookups. After
    /// `presence_join()` succeeds (which assigns `my_client_id`), these
    /// lookups should always work - failure indicates a bug or misconfiguration.
    async fn fetch_initial_state(&mut self) -> Result<(), TuiAppV2Error> {
        // Phase #479: Use my_client_id (assigned by presence_join in connect())
        let client_id = self.state.my_client_id;

        // Get mode - panic if fails (should never happen after join)
        let mode_resp = self.client.get_mode_or_panic(client_id).await;
        self.state.mode_name = mode_resp.name;
        self.state.mode_display = mode_resp.display;
        self.state.is_insert_mode = mode_resp.is_insert;

        // Get cursor - panic if fails
        let cursor_resp = self.client.get_cursor_or_panic(None, client_id).await;
        if let Some(pos) = cursor_resp.position {
            self.state.cursor_line = pos.line;
            self.state.cursor_col = pos.column;
        }

        // Get layout - panic if fails
        let layout_resp = self.client.get_layout_or_panic(client_id).await;
        self.apply_layout(&layout_resp);

        // Handle empty layout - create default local window
        if self.state.needs_default_window
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
        // Try to fetch number/relativenumber options
        // Server may return unimplemented if options not supported yet
        match self
            .client
            .get_options(vec!["number".to_string(), "relativenumber".to_string()])
            .await
        {
            Ok(opts) => {
                self.apply_display_options(&opts.options);
            }
            Err(e) => {
                // Log but don't fail - options are optional
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
    ///
    /// If server returns empty layout (no windows), client creates a default
    /// local view using the active buffer. This follows the principle that
    /// clients own window/layout decisions.
    fn apply_layout(&mut self, layout: &GetLayoutResponse) {
        // Phase #479: focused_window_id is now Option<u64> to eliminate ID ambiguity
        // None means "no windows" or "no focus", which we treat as window 0 for compatibility
        self.state.focused_window_id = layout.focused_window_id.unwrap_or(0);

        // Flatten window tree to list
        self.state.windows.clear();
        if let Some(root) = &layout.root {
            self.collect_windows(root);
        }

        // If no windows from server, mark that we need to create a default view
        // The actual creation happens after we have the active buffer
        self.state.needs_default_window = self.state.windows.is_empty();

        // Update layout mirror (Phase 11.2)
        self.layout_mirror
            .apply_layout_changed(self.state.focused_window_id, &self.state.windows);
    }

    /// Create a default window view for empty layout.
    ///
    /// Called when server returns no windows. Client creates a local window
    /// viewing the active buffer with full terminal dimensions.
    fn create_default_window(&mut self, buffer_id: u64) {
        let (width, height) = Terminal::size().unwrap_or((80, 24));
        let content_height = height.saturating_sub(1); // Reserve statusline

        let window = WindowInfo {
            window_id: 1, // Local ID
            // Phase #479: buffer_id is now Option<u64>
            buffer_id: Some(buffer_id),
            rect: Some(WindowRect {
                x: 0,
                y: 0,
                width: u64::from(width),
                height: u64::from(content_height),
            }),
            focused: true,
        };

        self.state.windows.push(window);
        self.state.focused_window_id = 1;
        self.state.needs_default_window = false;

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
                    // Phase #479: buffer_id is now Optional to eliminate ID ambiguity
                    // Skip windows with no buffer (they can't be displayed anyway)
                    if leaf.buffer_id.is_some() {
                        self.state.windows.push(WindowInfo {
                            window_id: leaf.window_id,
                            buffer_id: leaf.buffer_id,
                            rect: leaf.rect,
                            focused: leaf.window_id == self.state.focused_window_id,
                        });
                    } else {
                        tracing::debug!(
                            window_id = leaf.window_id,
                            "Skipping window with no buffer"
                        );
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
    async fn fetch_buffer_contents(&mut self) -> Result<(), TuiAppV2Error> {
        // Phase #479: buffer_id is now Option<u64>, filter out None values
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

                // Fetch initial tokens for syntax highlighting (Phase 13.0)
                self.fetch_tokens_for_buffer(buffer_id).await;
            }
        }

        Ok(())
    }

    /// Fetch syntax tokens for a buffer.
    ///
    /// Uses one-shot `GetTokens` RPC. Tokens are cached in `token_cache_manager`.
    /// Called on initial content load and after buffer modifications.
    async fn fetch_tokens_for_buffer(&mut self, buffer_id: u64) {
        // Get buffer content for byte-to-position conversion
        let content = self
            .state
            .buffer_cache
            .get(&buffer_id)
            .map(|lines| lines.join("\n"))
            .unwrap_or_default();

        // Request tokens from server
        match self.client.get_tokens(buffer_id, None, None).await {
            Ok(response) => {
                // Convert protocol TokenSpan to our local TokenSpan type
                let token_spans: Vec<TokenSpan> = response
                    .tokens
                    .into_iter()
                    .map(|t| TokenSpan {
                        start_byte: t.start_byte,
                        end_byte: t.end_byte,
                        category: t.category,
                    })
                    .collect();

                // Apply tokens to cache (full refresh)
                self.token_cache_manager.apply_token_update(
                    buffer_id,
                    &token_spans,
                    0,
                    u64::MAX, // Full buffer range
                    true,     // Full refresh
                    &content,
                );

                tracing::debug!(buffer_id, token_count = token_spans.len(), "Cached syntax tokens");
            }
            Err(e) => {
                // Log but don't fail - syntax highlighting is optional
                tracing::debug!(buffer_id, error = %e, "Could not fetch tokens");
            }
        }
    }

    /// Apply a colorscheme by name (Phase 13.0).
    ///
    /// Attempts to load the theme in this order:
    /// 1. User theme file (e.g., `~/.config/reovim/themes/name.toml`)
    /// 2. Builtin theme (dark, light, tokyo-night-orange)
    fn apply_colorscheme(&mut self, name: &str) {
        // Try user theme first
        match self.theme_loader.load(name) {
            Ok(theme) => {
                self.theme_manager.set_theme(theme);
                tracing::info!(theme = name, "Applied user theme");
                return;
            }
            Err(e) => {
                tracing::debug!(theme = name, error = %e, "User theme not found, trying builtin");
            }
        }

        // Try builtin themes
        let builtin = match name.to_lowercase().as_str() {
            "dark" => Some(BuiltinTheme::Dark),
            "light" => Some(BuiltinTheme::Light),
            "tokyo-night" | "tokyo-night-orange" | "tokyonight" => {
                Some(BuiltinTheme::TokyoNightOrange)
            }
            _ => None,
        };

        if let Some(theme) = builtin {
            self.theme_manager.set_theme(theme.load());
            tracing::info!(theme = name, "Applied builtin theme");
        } else {
            tracing::warn!(theme = name, "Theme not found");
            self.state.last_error = Some(format!("Theme not found: {name}"));
        }
    }

    /// Main event loop.
    async fn event_loop(&mut self) -> Result<(), TuiAppV2Error> {
        let mut redraw_timer = interval(Duration::from_millis(16)); // 60 FPS max

        while self.running {
            select! {
                // Terminal input events
                event = self.input.next_event() => {
                    match event {
                        Some(event) => self.handle_input_event(event).await?,
                        None => return Err(TuiAppV2Error::Disconnected),
                    }
                }

                // Server notifications
                notification = self.notification_stream.message() => {
                    match notification {
                        Ok(Some(notif)) => self.handle_notification(notif).await?,
                        Ok(None) => return Err(TuiAppV2Error::StreamEnded),
                        Err(e) => return Err(TuiAppV2Error::Grpc(e.into())),
                    }
                }

                // Redraw timer
                _ = redraw_timer.tick() => {
                    if self.state.needs_redraw {
                        self.render()?;
                        self.state.needs_redraw = false;
                    }
                }
            }
        }
        Ok(())
    }

    /// Handle terminal input event.
    async fn handle_input_event(&mut self, event: InputEvent) -> Result<(), TuiAppV2Error> {
        match event {
            InputEvent::Key(key_event) => {
                // Ctrl+Q to quit
                if key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && key_event.code == KeyCode::Char('q')
                {
                    self.running = false;
                    return Ok(());
                }

                // Send vim notation to server with client ID (Phase 11.2)
                // Note: my_client_id is always valid after presence_join()
                if !key_event.vim_notation.is_empty() {
                    let result = self
                        .client
                        .send_keys_with_client(&key_event.vim_notation, self.state.my_client_id)
                        .await;
                    if let Err(e) = result {
                        self.state.last_error = Some(format!("Send keys failed: {e}"));
                    }
                }
            }
            InputEvent::Resize(resize) => {
                self.screen.resize(resize.width, resize.height);

                // Update layout mirror dimensions (Phase 11.2)
                self.layout_mirror.set_screen(resize.width, resize.height);

                // Notify server
                if let Err(e) = self
                    .client
                    .resize(u64::from(resize.width), u64::from(resize.height))
                    .await
                {
                    self.state.last_error = Some(format!("Resize failed: {e}"));
                }

                self.state.needs_redraw = true;
            }
            InputEvent::Mouse(_) | InputEvent::FocusGained | InputEvent::FocusLost => {
                // Mouse support and focus tracking can be added later
            }
            InputEvent::Paste(text) => {
                // Handle paste by sending as keys with client ID (Phase 11.2)
                let result = self
                    .client
                    .send_keys_with_client(&text, self.state.my_client_id)
                    .await;
                if let Err(e) = result {
                    self.state.last_error = Some(format!("Paste failed: {e}"));
                }
            }
        }
        Ok(())
    }

    /// Handle server notification.
    #[allow(clippy::too_many_lines)]
    async fn handle_notification(&mut self, notif: Notification) -> Result<(), TuiAppV2Error> {
        if let Some(payload) = notif.payload {
            match payload {
                Payload::ModeChanged(mode) => {
                    // Phase 14 (#471): Filter by client_id for multi-client mode isolation
                    let is_local_mode = mode.client_id == self.state.my_client_id;

                    if is_local_mode {
                        // Local mode update
                        self.state.mode_name = mode.name;
                        self.state.mode_display = mode.display;
                        self.state.is_insert_mode = mode.is_insert;
                    } else {
                        // Remote mode update - update other_clients map
                        if let Some(remote) = self.state.other_clients.get_mut(&mode.client_id) {
                            remote.mode.clone_from(&mode.display);
                        }
                    }
                    self.state.needs_redraw = true;
                }
                Payload::CursorMoved(cursor) => {
                    // Phase 14 (#471): Filter by client_id for multi-client cursor isolation
                    let is_local_cursor = cursor.client_id == self.state.my_client_id;

                    if let Some(pos) = cursor.position {
                        if is_local_cursor {
                            // Local cursor update
                            // Store per-window cursor position (Phase 8 #465)
                            self.state.window_cursors.insert(
                                cursor.window_id,
                                CursorPosition {
                                    line: pos.line,
                                    column: pos.column,
                                },
                            );

                            // Update legacy globals for focused window (statusline compatibility)
                            if cursor.window_id == self.state.focused_window_id {
                                self.state.cursor_line = pos.line;
                                self.state.cursor_col = pos.column;
                            }
                        } else {
                            // Remote cursor update - update other_clients map
                            if let Some(remote) =
                                self.state.other_clients.get_mut(&cursor.client_id)
                            {
                                remote.cursor_line = pos.line;
                                remote.cursor_col = pos.column;
                            }
                        }
                    }
                    self.state.needs_redraw = true;
                }
                Payload::BufferModified(buf) => {
                    // Invalidate cache and refetch
                    self.state.buffer_cache.remove(&buf.buffer_id);
                    // Refetch content
                    if let Ok(content) = self
                        .client
                        .get_buffer_content(Some(buf.buffer_id), None, None)
                        .await
                    {
                        self.state.buffer_cache.insert(buf.buffer_id, content.lines);

                        // Refresh syntax tokens (Phase 13.0)
                        self.fetch_tokens_for_buffer(buf.buffer_id).await;
                    }
                    self.state.needs_redraw = true;
                }
                Payload::LayoutChanged(layout) => {
                    // Phase #479: focused_window_id is now Option<u64> to eliminate ID ambiguity
                    // None means "no focus", which we handle by using first window or 0
                    let effective_focused_id = match layout.focused_window_id {
                        Some(id) => id,
                        None if !layout.windows.is_empty() => {
                            tracing::debug!(
                                "Server sent no focused_window_id with {} windows, using first",
                                layout.windows.len()
                            );
                            layout.windows.first().map_or(0, |w| w.window_id)
                        }
                        None => 0,
                    };

                    // Update layout mirror (Phase 11.2)
                    self.layout_mirror
                        .apply_layout_changed(effective_focused_id, &layout.windows);

                    // Keep legacy state for statusline compatibility
                    self.state.focused_window_id = effective_focused_id;

                    // Phase 11.2 Fix: Set focused flag on the matching window
                    // The server sends window geometry but doesn't set the focused field.
                    // position_cursor() requires focused=true to locate the cursor window.
                    self.state.windows = layout
                        .windows
                        .into_iter()
                        .map(|mut w| {
                            w.focused = w.window_id == effective_focused_id;
                            w
                        })
                        .collect();

                    // Phase 8 (#465): Clean up stale cursor entries for deleted windows
                    let current_window_ids: std::collections::HashSet<u64> =
                        self.state.windows.iter().map(|w| w.window_id).collect();
                    self.state
                        .window_cursors
                        .retain(|id, _| current_window_ids.contains(id));

                    self.state.needs_redraw = true;
                }
                Payload::RenderComplete(_) => {
                    // Server signals a frame is ready - refresh content
                    self.state.needs_redraw = true;
                }
                Payload::Detach(_detach) => {
                    self.running = false;
                }
                Payload::OptionChanged(opt) => {
                    // Handle option changes (Phase 13.0)
                    match opt.name.as_str() {
                        "number" | "relativenumber" => {
                            // Refetch display options asynchronously
                            self.fetch_display_options().await;
                            self.state.needs_redraw = true;
                        }
                        "colorscheme" => {
                            // Load and apply new theme
                            if let Some(OptionValue::StringValue(theme_name)) = opt.value {
                                self.apply_colorscheme(&theme_name);
                            }
                            self.state.needs_redraw = true;
                        }
                        _ => {
                            self.state.needs_redraw = true;
                        }
                    }
                }
                Payload::PresenceJoined(p) => {
                    // Phase 11.2: Track other clients for awareness rendering
                    // Phase 14 (#471): cursor no longer in presence - uses CursorMoved with client_id
                    if let Some(client) = p.client {
                        // Skip self
                        if client.client_id != self.state.my_client_id {
                            tracing::info!(
                                client_id = client.client_id,
                                display_name = %client.display_name,
                                buffer_id = ?client.buffer_id,
                                "PresenceJoined: Adding remote client"
                            );
                            self.state.other_clients.insert(
                                client.client_id,
                                RemoteClient {
                                    client_id: client.client_id,
                                    display_name: client.display_name,
                                    cursor_line: 0, // Updated via CursorMoved notification
                                    cursor_col: 0,
                                    buffer_id: client.buffer_id,
                                    mode: client.mode,
                                    selection: None, // Updated via SelectionChanged notification
                                },
                            );
                            self.state.needs_redraw = true;
                        }
                    }
                }
                Payload::PresenceUpdated(p) => {
                    // Phase 11.2: Update remote client's state (viewport, mode)
                    // Phase 14 (#471): cursor no longer in presence - uses CursorMoved with client_id
                    if let Some(client) = p.client
                        && client.client_id != self.state.my_client_id
                    {
                        // Preserve existing cursor position and selection (updated via CursorMoved/SelectionChanged)
                        let old = self.state.other_clients.get(&client.client_id);
                        let cursor_line = old.map_or(0, |c| c.cursor_line);
                        let cursor_col = old.map_or(0, |c| c.cursor_col);
                        let selection = old.and_then(|c| c.selection.clone());

                        self.state.other_clients.insert(
                            client.client_id,
                            RemoteClient {
                                client_id: client.client_id,
                                display_name: client.display_name,
                                cursor_line,
                                cursor_col,
                                buffer_id: client.buffer_id,
                                mode: client.mode,
                                selection,
                            },
                        );
                        self.state.needs_redraw = true;
                    }
                }
                Payload::PresenceLeft(p) => {
                    // Phase 11.2: Remove departed client
                    self.state.other_clients.remove(&p.client_id);
                    self.state.needs_redraw = true;
                }
                Payload::SelectionChanged(sel) => {
                    // Phase 14 (#471): Filter by client_id for multi-client selection isolation
                    let is_local_selection = sel.client_id == self.state.my_client_id;

                    if is_local_selection {
                        // Local selection update
                        // Phase 8 (#465): Track selection for visual mode highlighting
                        if sel.has_selection {
                            if let Some(selection) = sel.selection {
                                let start =
                                    selection.start.map_or_else(CursorPosition::default, |p| {
                                        CursorPosition {
                                            line: p.line,
                                            column: p.column,
                                        }
                                    });
                                let end = selection.end.map_or_else(CursorPosition::default, |p| {
                                    CursorPosition {
                                        line: p.line,
                                        column: p.column,
                                    }
                                });
                                let mode = sel.visual_mode.unwrap_or_default();
                                self.state
                                    .window_selections
                                    .insert(sel.window_id, SelectionState { start, end, mode });
                            }
                        } else {
                            // Clear selection for this window
                            self.state.window_selections.remove(&sel.window_id);
                        }
                    } else {
                        // Remote selection update - update other_clients map
                        if let Some(remote) = self.state.other_clients.get_mut(&sel.client_id) {
                            if sel.has_selection {
                                if let Some(selection) = sel.selection {
                                    let start =
                                        selection.start.map_or_else(CursorPosition::default, |p| {
                                            CursorPosition {
                                                line: p.line,
                                                column: p.column,
                                            }
                                        });
                                    let end =
                                        selection.end.map_or_else(CursorPosition::default, |p| {
                                            CursorPosition {
                                                line: p.line,
                                                column: p.column,
                                            }
                                        });
                                    let mode = sel.visual_mode.unwrap_or_default();
                                    remote.selection = Some(SelectionState { start, end, mode });
                                }
                            } else {
                                remote.selection = None;
                            }
                        }
                    }
                    self.state.needs_redraw = true;
                }
                _ => {
                    // Other notifications
                    self.state.needs_redraw = true;
                }
            }
        }
        Ok(())
    }

    /// Render the current state to terminal.
    #[allow(clippy::result_large_err)]
    fn render(&mut self) -> Result<(), TuiAppV2Error> {
        let (width, height) = (self.screen.width(), self.screen.height());
        if width == 0 || height == 0 {
            return Ok(());
        }

        // Clear screen buffer
        self.screen.clear();

        // Render each window
        self.render_windows();

        // Render statusline
        self.render_statusline();

        // Apply screen to terminal
        self.screen.render(&mut self.terminal)?;

        // Position cursor
        self.position_cursor()?;

        Ok(())
    }

    /// Calculate gutter width for line numbers.
    fn calculate_gutter_width(&self, total_lines: usize) -> u16 {
        if self.state.line_number_mode == LineNumberMode::None {
            return 0;
        }

        // Calculate width needed: digits + 1 space padding
        let digits = if total_lines == 0 {
            1
        } else {
            // Safe: using f64 for log10 calculation, result is small
            #[allow(
                clippy::cast_precision_loss,
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss
            )]
            let d = ((total_lines as f64).log10().floor() as u16) + 1;
            d
        };
        digits + 2 // digits + space before + space after
    }

    /// Render a line number at the given position.
    #[allow(clippy::too_many_arguments)]
    fn render_line_number(&mut self, x: u16, y: u16, width: u16, line: usize, cursor_line: usize) {
        #[allow(clippy::cast_possible_truncation)]
        let fmt_width = (width as usize).saturating_sub(2);

        let (number_str, is_cursor_line) = match self.state.line_number_mode {
            LineNumberMode::None => return,
            LineNumberMode::Absolute => {
                let num = line + 1;
                let s = format!("{num:>fmt_width$} ");
                (s, line == cursor_line)
            }
            LineNumberMode::Relative => {
                let rel = line.abs_diff(cursor_line);
                let s = format!("{rel:>fmt_width$} ");
                (s, line == cursor_line)
            }
            LineNumberMode::Hybrid => {
                let num = if line == cursor_line {
                    line + 1
                } else {
                    line.abs_diff(cursor_line)
                };
                let s = format!("{num:>fmt_width$} ");
                (s, line == cursor_line)
            }
        };

        // Style: cursor line number is brighter
        let style = if is_cursor_line {
            Style::default().with_fg(Color::White)
        } else {
            Style::default().with_fg(Color::DarkGrey)
        };

        self.screen.write_str(x, y, &number_str, &style);
    }

    /// Convert display driver Style to TUI driver Style.
    ///
    /// Both types are structurally similar with fg, bg, and attributes.
    /// The attribute bits are compatible (BOLD, ITALIC, etc. at same positions).
    fn convert_style(display_style: &reovim_driver_display::Style) -> Style {
        use {
            reovim_driver_display::Attributes as DisplayAttrs,
            reovim_driver_tui::Attributes as TuiAttrs,
        };

        let mut tui_style = Style::default();

        if let Some(fg) = display_style.fg {
            tui_style = tui_style.with_fg(fg);
        }
        if let Some(bg) = display_style.bg {
            tui_style = tui_style.with_bg(bg);
        }

        // Convert attributes (same bit positions)
        if display_style.attributes.contains(DisplayAttrs::BOLD) {
            tui_style.attrs.set(TuiAttrs::BOLD);
        }
        if display_style.attributes.contains(DisplayAttrs::ITALIC) {
            tui_style.attrs.set(TuiAttrs::ITALIC);
        }
        if display_style.attributes.contains(DisplayAttrs::UNDERLINE) {
            tui_style.attrs.set(TuiAttrs::UNDERLINE);
        }
        if display_style
            .attributes
            .contains(DisplayAttrs::STRIKETHROUGH)
        {
            tui_style.attrs.set(TuiAttrs::STRIKETHROUGH);
        }

        tui_style
    }

    /// Check if a character at (line, col) is within the selection.
    ///
    /// Handles character, line, and block modes with exclusive end semantics.
    fn is_in_selection(selection: &SelectionState, line: u64, col: u64) -> bool {
        let (start_line, end_line) = (selection.start.line, selection.end.line);
        let (start_col, end_col) = (selection.start.column, selection.end.column);

        match selection.mode.as_str() {
            "line" => {
                // Line mode: entire lines selected, end line is exclusive
                line >= start_line && line < end_line
            }
            "block" => {
                // Block mode: rectangular selection
                // Line range (end line exclusive)
                let in_line_range = line >= start_line && line < end_line;
                // Column range (end col exclusive)
                let in_col_range = col >= start_col && col < end_col;
                in_line_range && in_col_range
            }
            _ => {
                // Character mode (default): contiguous range
                if start_line == end_line.saturating_sub(1) && start_line == line {
                    // Single line selection (but end may be on next line)
                    col >= start_col && col < end_col
                } else if line == start_line {
                    // First line of multi-line selection
                    col >= start_col
                } else if line == end_line.saturating_sub(1) {
                    // Last line of multi-line selection (end line exclusive)
                    col < end_col
                } else if start_line == end_line {
                    // Selection on a single line
                    line == start_line && col >= start_col && col < end_col
                } else {
                    // Middle lines are fully selected
                    line > start_line && line < end_line
                }
            }
        }
    }

    /// Render a line with syntax highlighting and optional selection.
    ///
    /// Gets tokens from the cache and applies styles from the theme manager.
    /// Falls back to default style for regions without tokens.
    /// Applies inverse video for selected characters.
    #[allow(clippy::too_many_arguments)]
    fn render_line_with_syntax(
        &mut self,
        x: u16,
        y: u16,
        max_width: u16,
        line: &str,
        line_number: u32,
        buffer_id: u64,
        selection: Option<&SelectionState>,
    ) {
        let default_style = Style::default();
        let selection_style = Style::default()
            .with_fg(Color::Black)
            .with_bg(Color::Magenta);

        // Get tokens for this line
        let tokens: Vec<_> = self
            .token_cache_manager
            .tokens_for_line(buffer_id, line_number)
            .collect();

        // Render line with syntax + selection highlighting
        let chars: Vec<char> = line.chars().take(max_width as usize).collect();

        for (col, ch) in (0_u32..).zip(chars.iter()) {
            // Check if this character is selected
            #[allow(clippy::cast_lossless)]
            let is_selected = selection
                .is_some_and(|sel| Self::is_in_selection(sel, line_number as u64, col as u64));

            // Determine base style (syntax highlighting or default)
            let base_style = if tokens.is_empty() {
                default_style.clone()
            } else {
                tokens
                    .iter()
                    .find(|t| col >= t.start_col && col < t.end_col)
                    .map_or_else(
                        || default_style.clone(),
                        |t| {
                            let display_style = self.theme_manager.get_style(&t.category);
                            Self::convert_style(&display_style)
                        },
                    )
            };

            // Apply selection highlighting if selected
            let final_style = if is_selected {
                selection_style.clone()
            } else {
                base_style
            };

            #[allow(clippy::cast_possible_truncation)]
            let screen_x = x + col as u16;
            self.screen.put_char(screen_x, y, *ch, &final_style);
        }

        // If line is empty but selected (line mode), show at least one character of selection
        if chars.is_empty()
            && let Some(sel) = selection
            && sel.mode == "line"
        {
            #[allow(clippy::cast_lossless)]
            let is_line_selected = Self::is_in_selection(sel, line_number as u64, 0);
            if is_line_selected {
                self.screen.put_char(x, y, ' ', &selection_style);
            }
        }
    }

    /// Render all windows to screen buffer.
    ///
    /// Uses `ServerLayoutMirror` (Phase 11.2) for placement data.
    fn render_windows(&mut self) {
        let tilde_style = Style::default().with_fg(Color::DarkBlue);

        // Get placements from layout mirror (Phase 11.2)
        let placements = self.layout_mirror.placements().to_vec();
        let has_multiple = self.layout_mirror.has_multiple_windows();
        let focused_id = self.layout_mirror.focused_id();

        // Clone caches to avoid borrow issues
        let buffer_cache = self.state.buffer_cache.clone();
        let selection_cache = self.state.window_selections.clone();

        // Get cursor line (for line number rendering)
        #[allow(clippy::cast_possible_truncation)]
        let cursor_line = self.state.cursor_line as usize;

        for placement in &placements {
            let x = placement.x;
            let y = placement.y;
            let w = placement.width;
            let h = placement.height;

            // Get buffer content (Phase #479: buffer_id is now Option<u64>)
            let lines = placement.buffer_id.and_then(|bid| buffer_cache.get(&bid));
            let total_lines = lines.map_or(0, Vec::len);

            // Get selection for this window (Phase 8 #465)
            let selection = selection_cache.get(&placement.window_id);

            // Calculate gutter width for line numbers
            let gutter_width = self.calculate_gutter_width(total_lines);
            let content_x = x + gutter_width;
            let content_width = w.saturating_sub(gutter_width);

            // Render content
            if let Some(lines) = lines {
                for (row, line) in lines.iter().enumerate().take(h as usize) {
                    #[allow(clippy::cast_possible_truncation)]
                    let screen_y = y + row as u16;

                    // Render line number if enabled
                    if gutter_width > 0 {
                        self.render_line_number(x, screen_y, gutter_width, row, cursor_line);
                    }

                    // Render line with syntax highlighting + selection (Phase 8 #465)
                    #[allow(clippy::cast_possible_truncation)]
                    let line_num = row as u32;
                    // Phase #479: buffer_id is Option - unwrap is safe here since we have lines
                    if let Some(buffer_id) = placement.buffer_id {
                        self.render_line_with_syntax(
                            content_x,
                            screen_y,
                            content_width,
                            line,
                            line_num,
                            buffer_id,
                            selection,
                        );
                    }
                }

                // Render empty lines past EOF with tilde (~)
                for row in lines.len()..(h as usize) {
                    #[allow(clippy::cast_possible_truncation)]
                    let screen_y = y + row as u16;

                    // Render tilde at content position (or gutter if no line numbers)
                    let tilde_x = if gutter_width > 0 {
                        x + gutter_width.saturating_sub(2)
                    } else {
                        x
                    };
                    self.screen.put_char(tilde_x, screen_y, '~', &tilde_style);
                }
            } else {
                // No buffer - fill with tildes
                for row in 0..(h as usize) {
                    #[allow(clippy::cast_possible_truncation)]
                    let screen_y = y + row as u16;
                    self.screen.put_char(x, screen_y, '~', &tilde_style);
                }
            }

            // Draw focus indicator if multiple windows (Phase 11.2)
            if has_multiple && focused_id == Some(placement.window_id) {
                let border_style = Style::default();
                self.screen.put_char(x, y, '▪', &border_style);
            }

            // Issue #474: Render other clients' selections (behind cursors)
            self.render_remote_selections(placement, gutter_width);

            // Phase 11.2: Render other clients' cursors for awareness
            self.render_remote_cursors(placement, gutter_width);
        }
    }

    /// Render other clients' cursors within a window (Phase 11.2, Issue #474).
    ///
    /// Shows each remote client's cursor as a colored marker.
    /// Each client gets a distinct color from the CBF-8 colorblind-friendly palette.
    fn render_remote_cursors(
        &mut self,
        placement: &crate::layout_mirror::WindowPlacement,
        gutter_width: u16,
    ) {
        // Collect remote clients viewing this buffer (Phase #479: buffer_id is Option)
        // Issue #474 fix: Show remote cursor if:
        // 1. Buffer IDs match (both have same buffer open), OR
        // 2. Remote buffer_id is None (client hasn't sent presence update yet)
        // This handles the common case where presence updates aren't sent on buffer change.
        let remotes: Vec<_> = self
            .state
            .other_clients
            .iter()
            .filter(|(_, r)| {
                match (placement.buffer_id, r.buffer_id) {
                    // Both have buffer_id - must match
                    (Some(local_bid), Some(remote_bid)) => local_bid == remote_bid,
                    // Remote doesn't have buffer_id yet - show cursor (assume same buffer)
                    (Some(_), None) => true,
                    // Local doesn't have buffer_id - don't show (shouldn't happen)
                    (None, _) => false,
                }
            })
            .map(|(&client_id, remote)| (client_id, remote.clone()))
            .collect();

        for (client_id, remote) in &remotes {
            // Get deterministic color for this client from CBF-8 palette
            let fg_color = reovim_arch::palette::color_for_client(*client_id);
            let bg_color = reovim_arch::palette::dark_color_for_client(*client_id);
            let cursor_style = Style::default().with_fg(fg_color).with_bg(bg_color);

            // Render remote cursor
            #[allow(clippy::cast_possible_truncation)]
            let cursor_row = remote.cursor_line as u16;
            #[allow(clippy::cast_possible_truncation)]
            let cursor_col = remote.cursor_col as u16;

            if cursor_row < placement.height {
                let screen_x = placement.x + gutter_width + cursor_col;
                let screen_y = placement.y + cursor_row;

                // Only render if within window bounds
                if screen_x < placement.x + placement.width
                    && screen_y < placement.y + placement.height
                {
                    // Use a thin vertical bar to indicate remote cursor
                    self.screen.put_char(screen_x, screen_y, '▎', &cursor_style);
                }
            }
        }
    }

    /// Render other clients' selections within a window (Issue #474).
    ///
    /// Shows visual mode selections from remote clients as colored backgrounds.
    /// Must be called BEFORE `render_remote_cursors` so cursors appear on top.
    fn render_remote_selections(
        &mut self,
        placement: &crate::layout_mirror::WindowPlacement,
        gutter_width: u16,
    ) {
        // Collect remote clients viewing this buffer with active selections
        // Issue #474 fix: Same buffer matching logic as render_remote_cursors
        let remotes: Vec<_> = self
            .state
            .other_clients
            .iter()
            .filter(|(_, r)| {
                r.selection.is_some()
                    && match (placement.buffer_id, r.buffer_id) {
                        (Some(local_bid), Some(remote_bid)) => local_bid == remote_bid,
                        (Some(_), None) => true, // Remote hasn't sent presence update yet
                        (None, _) => false,
                    }
            })
            .map(|(&client_id, remote)| (client_id, remote.clone()))
            .collect();

        for (client_id, remote) in &remotes {
            let Some(selection) = &remote.selection else {
                continue;
            };

            // Get dimmed color for selection background
            let bg_color = reovim_arch::palette::dimmed_color_for_client(*client_id);

            // Calculate selection range (normalize start/end)
            let start_line = selection.start.line.min(selection.end.line);
            let end_line = selection.start.line.max(selection.end.line);

            for line in start_line..=end_line {
                #[allow(clippy::cast_possible_truncation)]
                let line_u16 = line as u16;

                // Skip lines outside visible window
                if line_u16 >= placement.height {
                    continue;
                }

                let screen_y = placement.y + line_u16;

                // Calculate column range for this line based on selection mode
                let (start_col, end_col) = match selection.mode.as_str() {
                    "line" => {
                        // Line mode: entire line
                        (0u16, placement.width.saturating_sub(gutter_width))
                    }
                    "block" => {
                        // Block mode: same columns on every line
                        #[allow(clippy::cast_possible_truncation)]
                        let sc = selection.start.column.min(selection.end.column) as u16;
                        #[allow(clippy::cast_possible_truncation)]
                        let ec = selection.start.column.max(selection.end.column) as u16;
                        (sc, ec.saturating_add(1))
                    }
                    _ => {
                        // Char mode: depends on line position
                        #[allow(clippy::cast_possible_truncation)]
                        if line == start_line && line == end_line {
                            // Single line selection
                            let sc = selection.start.column.min(selection.end.column) as u16;
                            let ec = selection.start.column.max(selection.end.column) as u16;
                            (sc, ec.saturating_add(1))
                        } else if line == start_line {
                            // First line: from start column to end of line
                            #[allow(clippy::cast_possible_truncation)]
                            let sc = if selection.start.line < selection.end.line {
                                selection.start.column as u16
                            } else {
                                selection.end.column as u16
                            };
                            (sc, placement.width.saturating_sub(gutter_width))
                        } else if line == end_line {
                            // Last line: from start of line to end column
                            #[allow(clippy::cast_possible_truncation)]
                            let ec = if selection.start.line < selection.end.line {
                                selection.end.column as u16
                            } else {
                                selection.start.column as u16
                            };
                            (0, ec.saturating_add(1))
                        } else {
                            // Middle lines: entire line
                            (0, placement.width.saturating_sub(gutter_width))
                        }
                    }
                };

                // Apply selection background to each cell in range
                for col in start_col..end_col {
                    let screen_x = placement.x + gutter_width + col;
                    if screen_x < placement.x + placement.width {
                        self.screen.overlay_bg(screen_x, screen_y, bg_color);
                    }
                }
            }
        }
    }

    /// Get mode-specific style for the statusline mode section.
    fn mode_statusline_style(&self) -> Style {
        let mode = self.state.mode_name.to_lowercase();
        let (fg, bg) = match mode.as_str() {
            "insert" | "i" => (Color::Black, Color::Green),
            "visual" | "v" | "visual-line" | "visual-block" => (Color::Black, Color::Magenta),
            "replace" | "r" => (Color::Black, Color::Red),
            "command" | ":" | "cmdline" => (Color::Black, Color::Yellow),
            "delete" | "d" => (Color::Black, Color::DarkRed),
            "yank" | "y" => (Color::Black, Color::DarkYellow),
            "change" | "c" => (Color::Black, Color::DarkCyan),
            _ => (Color::Black, Color::Blue), // Normal mode and others
        };
        Style::default().with_fg(fg).with_bg(bg).bold()
    }

    /// Render statusline at bottom of screen.
    fn render_statusline(&mut self) {
        let height = self.screen.height();
        let width = self.screen.width();
        if height == 0 {
            return;
        }

        let status_y = height - 1;
        let default_style = Style::default().reverse();

        // Copy values to avoid borrow issues
        let mode_display = self.state.mode_display.clone();
        let cursor_line = self.state.cursor_line;
        let cursor_col = self.state.cursor_col;
        let server = self.server_address.clone();
        let role = self.state.my_role.as_str();

        // Phase 14 (#471): Include client_id in statusline for multi-client awareness
        // my_client_id is always valid after presence_join()
        let client_id_str = format!("#{}", self.state.my_client_id);

        // Get mode-specific style
        let mode_style = self.mode_statusline_style();

        // Build statusline with role and client_id indicator (Phase 14, #471)
        let left = format!(" {mode_display} [{role}{client_id_str}] ");
        let cursor_str = format!("{}:{}", cursor_line + 1, cursor_col + 1);
        let right = format!(" {cursor_str} | {server} ");

        // Fill statusline with default style
        self.screen
            .fill_horizontal(0, status_y, width, ' ', &default_style);

        // Write mode section with mode-specific colors
        self.screen.write_str(0, status_y, &left, &mode_style);

        // Write right part with default style
        #[allow(clippy::cast_possible_truncation)]
        let right_x = width.saturating_sub(right.len() as u16);
        self.screen
            .write_str(right_x, status_y, &right, &default_style);
    }

    /// Get cursor style based on current mode.
    fn cursor_style_for_mode(&self) -> CursorStyle {
        let mode = self.state.mode_name.to_lowercase();
        match mode.as_str() {
            // Insert mode and command line: blinking bar
            "insert" | "i" | "command" | ":" | "cmdline" => CursorStyle::BlinkingBar,
            // Replace mode: underline
            "replace" | "r" => CursorStyle::SteadyUnderline,
            // Normal mode, visual modes, and others: steady block
            _ => CursorStyle::SteadyBlock,
        }
    }

    /// Position terminal cursor at editor cursor location.
    #[allow(clippy::result_large_err)]
    fn position_cursor(&mut self) -> Result<(), TuiAppV2Error> {
        // Find focused window and copy necessary info to avoid borrow issues
        let focused_info = self.state.windows.iter().find(|w| w.focused).and_then(|w| {
            w.rect.as_ref().map(|rect| {
                // Phase 8 (#465): Get cursor for THIS window from per-window storage
                let cursor_pos = self
                    .state
                    .window_cursors
                    .get(&w.window_id)
                    .copied()
                    .unwrap_or(CursorPosition {
                        line: self.state.cursor_line,
                        column: self.state.cursor_col,
                    });

                // Phase #479: buffer_id is now Option<u64>
                let total_lines = w
                    .buffer_id
                    .and_then(|bid| self.state.buffer_cache.get(&bid))
                    .map_or(0, Vec::len);
                (*rect, total_lines, cursor_pos)
            })
        });

        if let Some((rect, total_lines, cursor_pos)) = focused_info {
            // Calculate gutter width for proper cursor offset
            let gutter_width = self.calculate_gutter_width(total_lines);

            #[allow(clippy::cast_possible_truncation)]
            let cursor_x = rect.x as u16 + gutter_width + cursor_pos.column as u16;
            #[allow(clippy::cast_possible_truncation)]
            let cursor_y = rect.y as u16 + cursor_pos.line as u16;

            // Set cursor style based on mode
            let cursor_style = self.cursor_style_for_mode();
            self.cursor.set_style(cursor_style);
            self.cursor.move_to(cursor_x, cursor_y);
            self.cursor.show();
            self.cursor.apply()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_state_default() {
        let state = TuiState::default();
        assert!(state.mode_name.is_empty());
        assert!(state.mode_display.is_empty());
        assert!(!state.is_insert_mode);
        assert_eq!(state.cursor_line, 0);
        assert_eq!(state.cursor_col, 0);
        assert_eq!(state.line_number_mode, LineNumberMode::None);
    }

    #[test]
    fn test_error_display() {
        let err = TuiAppV2Error::Disconnected;
        assert_eq!(err.to_string(), "Server disconnected");

        let err = TuiAppV2Error::StreamEnded;
        assert_eq!(err.to_string(), "Notification stream ended");
    }

    #[test]
    fn test_line_number_mode_default() {
        assert_eq!(LineNumberMode::default(), LineNumberMode::None);
    }

    #[test]
    fn test_line_number_mode_variants() {
        // Test all variants exist
        let _ = LineNumberMode::None;
        let _ = LineNumberMode::Absolute;
        let _ = LineNumberMode::Relative;
        let _ = LineNumberMode::Hybrid;
    }
}
