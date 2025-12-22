//! Core Runtime struct and initialization

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

/// Function pointer type for focus input handlers (enlist pattern)
pub type FocusInputHandler = fn(&mut Runtime, Option<char>, bool, bool);

use {
    crate::{
        bind::KeyMap,
        buffer::{Buffer, TextOps},
        command::CommandRegistry,
        command_line::CommandLine,
        component::RenderState,
        config::{ProfileConfig, ProfileManager},
        constants::EVENT_CHANNEL_CAPACITY,
        decoration::{DecorationStore, LanguageRendererRegistry},
        event::InnerEvent,
        event_bus::{BufferClosed, EventBus, EventSender, FileOpened},
        highlight::{ColorMode, HighlightStore, Theme},
        indent::IndentAnalyzer,
        interactor::InteractorRegistry,
        jumplist::JumpList,
        modd::ModeState,
        modifier::{ModifierContext, ModifierRegistry},
        plugin::{PluginContext, PluginLoader, PluginStateRegistry, PluginTuple},
        register::Registers,
        screen::Screen,
        ui_component::ComponentRegistry,
    },
    tracing::debug,
};

use tokio::sync::{mpsc, watch};

/// The main runtime that owns all editor state
pub struct Runtime {
    pub buffers: BTreeMap<usize, Buffer>,
    pub screen: Screen,
    pub highlight_store: HighlightStore,
    pub mode_state: ModeState,
    pub color_mode: ColorMode,
    pub theme: Theme,
    pub registers: Registers,
    pub command_line: CommandLine,
    pub pending_keys: String,
    pub last_command: String,
    pub tx: mpsc::Sender<InnerEvent>,
    pub rx: mpsc::Receiver<InnerEvent>,
    pub initial_file: Option<String>,
    pub(crate) showing_landing_page: bool,
    /// Watch channel sender for broadcasting mode changes
    pub(crate) mode_tx: watch::Sender<ModeState>,
    /// Watch channel receiver (kept to allow subscribing)
    mode_rx: watch::Receiver<ModeState>,
    /// Command registry for trait-based command system
    pub command_registry: Arc<CommandRegistry>,
    /// Keymap with all registered keybindings (built-in + plugins)
    pub keymap: KeyMap,
    /// Currently active buffer ID
    pub active_buffer_id: usize,
    /// Next buffer ID to assign
    next_buffer_id: usize,
    /// Jump list for Ctrl-O/Ctrl-I navigation
    pub jump_list: JumpList,
    /// Indent guide analyzer
    pub indent_analyzer: IndentAnalyzer,
    /// Profile manager for config loading/saving
    pub profile_manager: ProfileManager,
    /// Name of the currently loaded profile
    pub current_profile_name: String,
    /// Flag indicating render is needed (for coalescing)
    render_pending: bool,
    /// Interactor registry for extensible input handlers
    pub interactor_registry: InteractorRegistry,
    /// Modifier registry for style and behavior modifiers
    pub modifier_registry: ModifierRegistry,
    /// Registered focus input handlers per interactor (enlist pattern)
    pub(crate) focus_input_handlers: HashMap<crate::ui_component::ComponentId, FocusInputHandler>,
    /// Component registry for unified UI components
    pub component_registry: ComponentRegistry,
    /// Decoration store for language-specific visual decorations
    pub decoration_store: DecorationStore,
    /// Language renderer registry for decoration generation
    pub renderer_registry: LanguageRendererRegistry,
    /// Event bus for type-erased plugin events
    pub event_bus: Arc<EventBus>,
    /// Plugin state registry for plugin-owned state
    pub plugin_state: Arc<PluginStateRegistry>,
    /// Overlay registry for plugin-based overlays
    pub overlay_registry: crate::overlay::OverlayRegistry,
    /// RPC handler registry for plugin-registered RPC methods
    pub rpc_handler_registry: crate::rpc::RpcHandlerRegistry,
    /// Display registry for plugin-provided mode display strings and icons
    pub display_registry: crate::display::DisplayRegistry,
    /// Render stage registry for pipeline transformations
    pub render_stages: Arc<std::sync::RwLock<crate::render::RenderStageRegistry>>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new(Screen::default())
    }
}

impl Runtime {
    /// Create a new Runtime with the given screen
    ///
    /// This is equivalent to calling `Runtime::with_plugins(screen, DefaultPlugins)`.
    #[must_use]
    pub fn new(screen: Screen) -> Self {
        use crate::plugin::builtin::DefaultPlugins;
        Self::with_plugins(screen, DefaultPlugins)
    }

    /// Create a Runtime with custom plugins
    ///
    /// This allows loading a custom set of plugins instead of the defaults.
    /// Useful for creating minimal runtimes or adding custom plugins.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use reovim_core::plugin::{DefaultPlugins, CorePlugin};
    /// use reovim_core::runtime::Runtime;
    /// use reovim_core::screen::Screen;
    ///
    /// // Use default plugins
    /// let runtime = Runtime::with_plugins(screen, DefaultPlugins);
    ///
    /// // Or use a minimal set
    /// let runtime = Runtime::with_plugins(screen, CorePlugin);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if plugin loading fails (e.g., missing dependencies or
    /// cyclic dependency detected).
    #[must_use]
    pub fn with_plugins<T: PluginTuple>(screen: Screen, plugins: T) -> Self {
        let (tx, rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let (mode_tx, mode_rx) = watch::channel(ModeState::new());

        // Initialize event bus and plugin state registry
        let event_bus = Arc::new(EventBus::new(EVENT_CHANNEL_CAPACITY));
        let plugin_state = Arc::new(PluginStateRegistry::new());

        // Initialize profile manager
        let profile_manager = ProfileManager::default();
        let default_profile_name = profile_manager.default_profile_name().to_string();

        // Load plugins with state registry and event bus integration
        let mut ctx = PluginContext::new();
        let mut loader = PluginLoader::new();
        loader.add_plugins(plugins);
        loader
            .load_with_state(&mut ctx, &plugin_state, &event_bus)
            .expect("Plugin loading failed");

        // Extract components from plugin context
        let (
            command_registry,
            interactor_registry,
            modifier_registry,
            keymap,
            focus_handlers,
            component_registry,
            overlay_registry,
            rpc_handler_registry,
            display_registry,
            render_stages,
        ) = ctx.into_parts();

        // Wrap render_stages in Arc<RwLock<>> and inject into plugin_state
        // This allows plugins to register stages from init_state()
        let render_stages = Arc::new(std::sync::RwLock::new(render_stages));
        plugin_state.set_render_stages(Arc::clone(&render_stages));

        let runtime = Self {
            buffers: BTreeMap::new(),
            screen,
            highlight_store: HighlightStore::new(),
            mode_state: ModeState::new(),
            color_mode: ColorMode::detect(),
            theme: Theme::default(),
            registers: Registers::new(),
            command_line: CommandLine::default(),
            pending_keys: String::new(),
            last_command: String::new(),
            tx,
            rx,
            initial_file: None,
            showing_landing_page: false,
            mode_tx,
            mode_rx,
            command_registry: Arc::new(command_registry),
            keymap,
            active_buffer_id: 0,
            next_buffer_id: 0,
            jump_list: JumpList::new(),
            indent_analyzer: IndentAnalyzer::default(),
            profile_manager,
            current_profile_name: default_profile_name,
            render_pending: false,
            interactor_registry,
            modifier_registry,
            focus_input_handlers: focus_handlers,
            component_registry,
            decoration_store: DecorationStore::new(),
            renderer_registry: LanguageRendererRegistry::new(),
            event_bus,
            plugin_state,
            overlay_registry,
            rpc_handler_registry,
            display_registry,
            render_stages,
        };

        // Subscribe to focus change requests from plugins
        {
            use crate::event_bus::{core_events::RequestFocusChange, EventResult};
            let mode_tx = runtime.mode_tx.clone();
            let tx = runtime.tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestFocusChange, _>(100, move |event, _ctx| {
                    let current_mode = mode_tx.borrow().clone();
                    let new_mode = current_mode.set_interactor_id(event.target);
                    // Send through event loop to properly update runtime.mode_state
                    let _ = tx.try_send(crate::event::InnerEvent::ModeChangeEvent(new_mode));
                    tracing::info!("Runtime: Requesting focus change to component '{}'", event.target.0);
                    EventResult::Handled
                });
        }

        // Subscribe to file open requests from plugins
        {
            use crate::event_bus::{core_events::RequestOpenFile, EventResult};
            let tx = runtime.tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestOpenFile, _>(100, move |event, _ctx| {
                    tracing::info!("Runtime: Requesting to open file: {:?}", event.path);
                    // Send OpenFileRequest to the runtime event loop
                    let _ = tx.try_send(InnerEvent::OpenFileRequest {
                        path: event.path.clone(),
                    });
                    EventResult::Handled
                });
        }

        runtime
    }

    /// Subscribe to mode changes
    #[must_use]
    pub fn subscribe_mode(&self) -> watch::Receiver<ModeState> {
        self.mode_rx.clone()
    }

    /// Get an event sender for emitting events to the event bus
    #[must_use]
    pub fn event_sender(&self) -> EventSender {
        self.event_bus.sender()
    }

    /// Get a reference to the plugin state registry
    #[must_use]
    pub const fn plugin_state(&self) -> &Arc<PluginStateRegistry> {
        &self.plugin_state
    }

    /// Broadcast a mode change
    ///
    /// Handles undo batching: changes made during insert mode are batched
    /// into a single undo unit when leaving insert mode.
    pub(crate) fn set_mode(&mut self, mode_state: ModeState) {
        let was_insert = self.mode_state.is_insert();
        let is_insert = mode_state.is_insert();

        // Handle undo batching on insert mode transitions
        if !was_insert && is_insert {
            // Entering insert mode: begin batching
            if let Some(buf) = self.buffers.get_mut(&self.active_buffer_id) {
                buf.begin_batch();
            }
        } else if was_insert && !is_insert {
            // Leaving insert mode: flush batch
            if let Some(buf) = self.buffers.get_mut(&self.active_buffer_id) {
                buf.flush_batch();
            }
        }

        self.mode_state = mode_state.clone();
        let _ = self.mode_tx.send(mode_state);
    }

    /// Get current mode state
    #[must_use]
    pub const fn current_mode(&self) -> &ModeState {
        &self.mode_state
    }

    /// Register a focus input handler for an interactor (enlist pattern)
    ///
    /// This follows the Bevy/Zed pattern where handlers are registered at
    /// initialization time, enabling fully generic dispatch at runtime.
    pub fn enlist_focus_input_handler(
        &mut self,
        id: crate::ui_component::ComponentId,
        handler: FocusInputHandler,
    ) {
        self.focus_input_handlers.insert(id, handler);
    }

    /// Build a modifier context for the given window
    ///
    /// This creates a context with all the information needed to evaluate
    /// which modifiers should apply to a window.
    #[must_use]
    pub fn build_modifier_context(
        &self,
        window_id: usize,
        buffer_id: usize,
        is_active: bool,
        is_floating: bool,
    ) -> ModifierContext<'_> {
        let filetype = self
            .buffers
            .get(&buffer_id)
            .and_then(|b| b.file_path.as_ref())
            .map(|p| crate::filetype::filetype_id(p));

        let is_modified = self.buffers.get(&buffer_id).is_some_and(|b| b.modified);

        ModifierContext::new(
            self.mode_state.interactor_id,
            &self.mode_state.edit_mode,
            &self.mode_state.sub_mode,
            window_id,
            buffer_id,
        )
        .with_filetype(filetype)
        .with_active(is_active)
        .with_modified(is_modified)
        .with_floating(is_floating)
    }

    /// Set the initial file to open
    #[must_use]
    pub fn with_file(mut self, file_path: Option<String>) -> Self {
        self.initial_file = file_path;
        self
    }

    /// Render the screen with current state
    pub(crate) fn render(&mut self) {
        // Get visibility source from plugin state (fold plugin provides this)
        let visibility_source = self.plugin_state.visibility_source();
        let state = RenderState {
            buffers: &self.buffers,
            highlight_store: &self.highlight_store,
            mode: &self.mode_state,
            command_line: &self.command_line,
            pending_keys: &self.pending_keys,
            last_command: &self.last_command,
            color_mode: self.color_mode,
            theme: &self.theme,
            plugin_state: &self.plugin_state,
            visibility_source: visibility_source.as_ref(),
            indent_analyzer: &self.indent_analyzer,
            modifier_registry: Some(&self.modifier_registry),
            decoration_store: Some(&self.decoration_store),
            renderer_registry: Some(&self.renderer_registry),
            render_stages: &self.render_stages,
        };
        self.screen
            .render_with_state(&state)
            .expect("failed to render");
        self.screen.flush().expect("failed to flush");
    }

    /// Mark that a render is needed (doesn't render immediately)
    ///
    /// Use this instead of `render()` to enable render coalescing.
    /// Call `flush_render()` at the end of event processing to perform
    /// the actual render if needed.
    pub(crate) const fn request_render(&mut self) {
        self.render_pending = true;
    }

    /// Flush pending render if needed
    ///
    /// Should be called once at the end of each event loop iteration.
    /// Only renders if `request_render()` was called since the last flush.
    pub(crate) fn flush_render(&mut self) {
        if self.render_pending {
            self.render();
            self.render_pending = false;
        }
    }

    /// Set color mode (for :set colormode command)
    #[allow(clippy::missing_const_for_fn)]
    pub fn set_color_mode(&mut self, mode: ColorMode) {
        self.color_mode = mode;
    }

    /// Create a new empty buffer and return its ID
    pub fn create_buffer(&mut self) -> usize {
        let id = self.next_buffer_id;
        self.next_buffer_id += 1;
        let buffer = Buffer::empty(id);
        self.buffers.insert(id, buffer);
        id
    }

    /// Create a new buffer from a file and return its ID
    /// Returns None if the file cannot be read
    pub fn create_buffer_from_file(&mut self, path: &str) -> Option<usize> {
        debug!(path, "create_buffer_from_file: attempting to read");
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let id = self.next_buffer_id;
                self.next_buffer_id += 1;
                let mut buffer = Buffer::empty(id);
                buffer.set_content(&content);
                buffer.file_path = Some(path.to_string());
                self.buffers.insert(id, buffer);

                // Emit FileOpened event for treesitter plugin to handle syntax highlighting
                self.event_bus.emit(FileOpened {
                    buffer_id: id,
                    path: path.to_string(),
                });
                debug!(id, path, "create_buffer_from_file: emitted FileOpened event");

                debug!(id, path, "create_buffer_from_file: success");
                Some(id)
            }
            Err(e) => {
                debug!(path, error = %e, "create_buffer_from_file: failed to read file");
                None
            }
        }
    }

    /// Switch to a different buffer by ID
    pub fn switch_buffer(&mut self, buffer_id: usize) {
        if self.buffers.contains_key(&buffer_id) {
            self.active_buffer_id = buffer_id;
        }
    }

    /// Close a buffer by ID
    /// Returns true if buffer was closed, false if it was the last buffer
    pub fn close_buffer(&mut self, buffer_id: usize) -> bool {
        // Don't close the last buffer
        if self.buffers.len() <= 1 {
            return false;
        }

        if self.buffers.remove(&buffer_id).is_some() {
            // Clean up highlights
            self.highlight_store.clear_all(buffer_id);

            // Emit BufferClosed event for plugins to clean up their state
            self.event_bus.emit(BufferClosed { buffer_id });

            // If we closed the active buffer, switch to another one
            if self.active_buffer_id == buffer_id
                && let Some(&new_id) = self.buffers.keys().next()
            {
                self.active_buffer_id = new_id;
            }
            true
        } else {
            false
        }
    }

    /// Get the previous buffer ID (wraps around)
    #[must_use]
    pub fn prev_buffer_id(&self) -> Option<usize> {
        let ids: Vec<usize> = self.buffers.keys().copied().collect();
        if ids.len() <= 1 {
            return None;
        }
        let current_pos = ids.iter().position(|&id| id == self.active_buffer_id)?;
        let prev_pos = if current_pos == 0 {
            ids.len() - 1
        } else {
            current_pos - 1
        };
        Some(ids[prev_pos])
    }

    /// Get the next buffer ID (wraps around)
    #[must_use]
    pub fn next_buffer_id(&self) -> Option<usize> {
        let ids: Vec<usize> = self.buffers.keys().copied().collect();
        if ids.len() <= 1 {
            return None;
        }
        let current_pos = ids.iter().position(|&id| id == self.active_buffer_id)?;
        let next_pos = (current_pos + 1) % ids.len();
        Some(ids[next_pos])
    }

    /// Get the currently active buffer
    #[must_use]
    pub fn active_buffer(&self) -> Option<&Buffer> {
        self.buffers.get(&self.active_buffer_id)
    }

    /// Get the currently active buffer mutably
    pub fn active_buffer_mut(&mut self) -> Option<&mut Buffer> {
        self.buffers.get_mut(&self.active_buffer_id)
    }

    /// Open a file, creating a new buffer or switching to existing one
    pub fn open_file(&mut self, path: &str) {
        debug!(path, "open_file: called");

        // Check if this file is already open in a buffer
        for (id, buf) in &self.buffers {
            if buf.file_path.as_deref() == Some(path) {
                debug!(id, path, "open_file: file already open, switching buffer");
                self.active_buffer_id = *id;
                return;
            }
        }

        // Create new buffer from file
        if let Some(id) = self.create_buffer_from_file(path) {
            debug!(id, path, "open_file: created new buffer");
            self.active_buffer_id = id;
            self.showing_landing_page = false;
        } else {
            debug!(path, "open_file: failed to create buffer");
        }
    }

    /// Re-highlight all buffers after theme change
    ///
    /// Emits parse requests for each buffer so the treesitter plugin can
    /// regenerate highlights with the new theme.
    pub(crate) fn rehighlight_all_buffers(&mut self) {
        use crate::event_bus::BufferModification;

        for (&buffer_id, buffer) in &self.buffers {
            // Clear existing highlights
            self.highlight_store.clear_all(buffer_id);

            // Emit BufferModified event to trigger reparse by treesitter plugin
            self.event_bus.emit(crate::event_bus::BufferModified {
                buffer_id,
                modification: BufferModification::FullReplace,
            });
            debug!(buffer_id, "rehighlight_all_buffers: requested reparse");
            let _ = buffer; // Suppress unused warning
        }
    }

    // ========================================================================
    // Profile Management
    // ========================================================================

    /// Set a specific profile to load at startup
    #[must_use]
    pub fn with_profile(mut self, profile_name: Option<String>) -> Self {
        if let Some(name) = profile_name {
            self.current_profile_name = name;
        }
        self
    }

    /// Load and apply a profile by name
    ///
    /// Returns true if the profile was loaded successfully.
    pub fn load_profile(&mut self, name: &str) -> bool {
        match self.profile_manager.load_profile(name) {
            Ok(config) => {
                self.apply_profile(&config);
                self.current_profile_name = name.to_string();
                tracing::info!(profile = %name, "Profile loaded and applied");
                true
            }
            Err(e) => {
                tracing::error!(profile = %name, error = %e, "Failed to load profile");
                false
            }
        }
    }

    /// Apply a profile configuration to the runtime
    pub fn apply_profile(&mut self, config: &ProfileConfig) {
        use crate::highlight::ThemeName;

        // Apply theme
        if let Some(theme_name) = ThemeName::parse(&config.editor.theme) {
            self.theme = Theme::from_name(theme_name);
            // Theme change will trigger rehighlighting via event bus
            self.rehighlight_all_buffers();
            debug!(theme = %config.editor.theme, "Applied theme from profile");
        }

        // Apply color mode
        if let Some(mode) = ColorMode::parse(&config.editor.colormode) {
            self.color_mode = mode;
            debug!(colormode = %config.editor.colormode, "Applied colormode from profile");
        }

        // Apply editor options
        // Note: These settings need to be applied to windows, which is done at render time
        // or through the :set command mechanism. For now we store them in the profile.

        // Apply indent guide setting
        self.indent_analyzer.set_enabled(config.editor.indentguide);

        debug!(
            number = config.editor.number,
            relativenumber = config.editor.relativenumber,
            indentguide = config.editor.indentguide,
            scrollbar = config.editor.scrollbar,
            "Applied editor options from profile"
        );

        // Note: Keybinding application is deferred to Phase 5
    }

    /// Save current runtime settings as a profile
    pub fn save_current_as_profile(&self, name: &str) -> bool {
        use crate::config::EditorConfig;

        // Build profile from current state
        // Note: Theme doesn't track its name, so we use a default
        // A future improvement would be to track the current theme name
        let editor_config = EditorConfig {
            theme: "dark".to_string(), // TODO: Track current theme name
            colormode: format!("{:?}", self.color_mode).to_lowercase(),
            number: true,         // TODO: Get from window settings
            relativenumber: true, // TODO: Get from window settings
            indentguide: self.indent_analyzer.is_enabled(),
            scrollbar: true, // TODO: Get from window settings
            ..EditorConfig::default()
        };

        let config = ProfileConfig {
            profile: crate::config::ProfileMeta {
                name: name.to_string(),
                description: String::new(),
                version: "1".to_string(),
            },
            editor: editor_config,
            ..ProfileConfig::default()
        };

        match self.profile_manager.save_profile(name, &config) {
            Ok(()) => {
                tracing::info!(profile = %name, "Profile saved");
                true
            }
            Err(e) => {
                tracing::error!(profile = %name, error = %e, "Failed to save profile");
                false
            }
        }
    }

    /// List available profiles
    #[must_use]
    pub fn list_profiles(&self) -> Vec<String> {
        self.profile_manager.list_profiles()
    }

    /// Initialize the profile system and optionally load the startup profile
    pub fn init_profiles(&mut self) {
        // Ensure default profile exists
        if let Err(e) = self.profile_manager.initialize() {
            tracing::warn!(error = %e, "Failed to initialize profile system");
            return;
        }

        // Load the startup profile
        let profile_name = self.current_profile_name.clone();
        if self.profile_manager.profile_exists(&profile_name) {
            self.load_profile(&profile_name);
        } else {
            debug!(profile = %profile_name, "Startup profile not found, using defaults");
        }
    }

    // === RPC State Snapshot Methods ===

    /// Get a snapshot of the current mode state for RPC
    #[must_use]
    pub fn mode_snapshot(&self) -> crate::rpc::ModeSnapshot {
        crate::rpc::ModeSnapshot::from(&self.mode_state)
    }

    /// Get a snapshot of cursor position for a buffer
    #[must_use]
    pub fn cursor_snapshot(&self, buffer_id: usize) -> Option<crate::rpc::CursorSnapshot> {
        self.buffers
            .get(&buffer_id)
            .map(|buf| crate::rpc::CursorSnapshot::from(&buf.cur))
    }

    /// Get a snapshot of buffer metadata
    #[must_use]
    pub fn buffer_snapshot(&self, buffer_id: usize) -> Option<crate::rpc::BufferSnapshot> {
        self.buffers
            .get(&buffer_id)
            .map(crate::rpc::BufferSnapshot::from)
    }

    /// Get a list of all buffer snapshots
    #[must_use]
    pub fn buffer_list_snapshot(&self) -> Vec<crate::rpc::BufferSnapshot> {
        self.buffers
            .values()
            .map(crate::rpc::BufferSnapshot::from)
            .collect()
    }

    /// Get a snapshot of selection state for a buffer
    #[must_use]
    pub fn selection_snapshot(&self, buffer_id: usize) -> Option<crate::rpc::SelectionSnapshot> {
        self.buffers.get(&buffer_id).map(|buf| {
            let mut snapshot = crate::rpc::SelectionSnapshot::from(&buf.selection);
            // Fill in the actual cursor position
            snapshot.cursor = crate::rpc::CursorSnapshot::from(&buf.cur);
            snapshot
        })
    }

    /// Get a snapshot of screen dimensions and state
    #[must_use]
    pub fn screen_snapshot(&self) -> crate::rpc::ScreenSnapshot {
        crate::rpc::ScreenSnapshot {
            width: self.screen.width(),
            height: self.screen.height(),
            active_buffer_id: self.active_buffer_id,
            active_window_id: self.screen.active_window_id(),
            window_count: self.screen.window_count(),
        }
    }

    /// Get a snapshot of all windows and their states
    #[must_use]
    pub fn windows_snapshot(&self) -> Vec<crate::rpc::WindowSnapshot> {
        self.screen
            .windows()
            .iter()
            .map(|w| crate::rpc::WindowSnapshot {
                id: w.id,
                buffer_id: w.buffer_id().unwrap_or(0),
                buffer_anchor_x: w.buffer_anchor().map_or(0, |a| a.x),
                buffer_anchor_y: w.buffer_anchor().map_or(0, |a| a.y),
                is_active: w.is_active,
                cursor_x: w.cursor.x,
                cursor_y: w.cursor.y,
            })
            .collect()
    }

    /// Get the content of a buffer as a string
    #[must_use]
    pub fn buffer_content(&self, buffer_id: usize) -> Option<String> {
        self.buffers.get(&buffer_id).map(|buf| {
            buf.contents
                .iter()
                .map(|line| line.inner.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
    }

    /// Get a visual snapshot of the screen for debugging/AI understanding
    ///
    /// Returns None if the frame renderer is not enabled.
    #[must_use]
    pub fn visual_snapshot(&self) -> Option<crate::visual::VisualSnapshot> {
        let buffer = self.screen.frame_buffer()?;

        // Build cell grid
        let mut cells = Vec::with_capacity(buffer.height() as usize);
        for y in 0..buffer.height() {
            let mut row = Vec::with_capacity(buffer.width() as usize);
            if let Some(buffer_row) = buffer.row(y) {
                for cell in buffer_row {
                    row.push(crate::rpc::CellSnapshot::from(cell));
                }
            }
            cells.push(row);
        }

        // Get cursor info
        let cursor =
            self.buffers
                .get(&self.active_buffer_id)
                .map(|buf| crate::visual::CursorInfo {
                    x: buf.cur.x,
                    y: buf.cur.y,
                    layer: "editor".to_string(),
                });

        // Get layer info - dynamically query overlay registry
        let ctx = crate::component::RenderContext::new(
            self.screen.width(),
            self.screen.height(),
            &self.theme,
            self.color_mode,
        );
        let layers = self.screen.layer_info(&self.overlay_registry, &ctx);

        // Build plain text
        let plain_text = buffer.to_ascii();

        Some(crate::visual::VisualSnapshot {
            width: buffer.width(),
            height: buffer.height(),
            cells,
            cursor,
            layers,
            plain_text,
        })
    }
}

impl super::RuntimeContext for Runtime {
    fn plugin_state(&self) -> &Arc<PluginStateRegistry> {
        &self.plugin_state
    }

    fn event_bus(&self) -> &Arc<crate::event_bus::EventBus> {
        &self.event_bus
    }

    fn buffer(&self, id: usize) -> Option<&Buffer> {
        self.buffers.get(&id)
    }

    fn buffer_mut(&mut self, id: usize) -> Option<&mut Buffer> {
        self.buffers.get_mut(&id)
    }

    fn active_buffer_id(&self) -> usize {
        self.active_buffer_id
    }

    fn mode(&self) -> &ModeState {
        &self.mode_state
    }

    fn set_mode(&mut self, mode: ModeState) {
        // Inline the mode change logic here to avoid recursion with the inherent method
        let was_insert = self.mode_state.is_insert();
        let is_insert = mode.is_insert();

        // Handle undo batching on insert mode transitions
        if !was_insert && is_insert {
            // Entering insert mode: begin batching
            if let Some(buf) = self.buffers.get_mut(&self.active_buffer_id) {
                buf.begin_batch();
            }
        } else if was_insert && !is_insert {
            // Leaving insert mode: flush batch
            if let Some(buf) = self.buffers.get_mut(&self.active_buffer_id) {
                buf.flush_batch();
            }
        }

        self.mode_state = mode.clone();
        let _ = self.mode_tx.send(mode);
    }

    fn screen_size(&self) -> (u16, u16) {
        (self.screen.width(), self.screen.height())
    }
}
