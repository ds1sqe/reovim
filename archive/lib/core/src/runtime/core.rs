//! Core Runtime struct and initialization

use std::{collections::BTreeMap, sync::Arc};

/// Function pointer type for focus input handlers (enlist pattern)
pub type FocusInputHandler = fn(&mut Runtime, Option<char>, bool, bool);

use {
    crate::{
        animation::AnimationSystem,
        bind::KeyMap,
        buffer::Buffer,
        command::CommandRegistry,
        command_line::CommandLine,
        config::ProfileManager,
        constants::{EVENT_CHANNEL_CAPACITY, HI_PRIORITY_CHANNEL_CAPACITY},
        decoration::DecorationStore,
        event::RuntimeEvent,
        event_bus::{
            DynEvent, EventBus, EventResult, EventSender, HandlerContext, ViewportScrolled,
            core_events::ModeChanged,
        },
        highlight::{ColorMode, HighlightStore, Theme},
        indent::IndentAnalyzer,
        interactor::InteractorRegistry,
        jumplist::JumpList,
        modd::ModeState,
        modifier::{ModifierContext, ModifierRegistry},
        option::{OptionRegistry, RegisterOption},
        plugin::{Plugin, PluginContext, PluginLoader, PluginStateRegistry, PluginTuple},
        register::Registers,
        runtime::PrioritizedEventSender,
        screen::Screen,
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
    /// High-priority channel sender (user input, mode changes)
    pub hi_tx: mpsc::Sender<RuntimeEvent>,
    /// High-priority channel receiver
    pub(crate) hi_rx: mpsc::Receiver<RuntimeEvent>,
    /// Low-priority channel sender (render signals, background tasks)
    pub lo_tx: mpsc::Sender<RuntimeEvent>,
    /// Low-priority channel receiver
    pub(crate) lo_rx: mpsc::Receiver<RuntimeEvent>,
    /// Prioritized sender wrapper for convenient access to both channels
    pub prioritized_sender: PrioritizedEventSender,
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
    /// Next buffer ID to assign
    pub(super) next_buffer_id: usize,
    /// Jump list for Ctrl-O/Ctrl-I navigation
    pub jump_list: JumpList,
    /// Indent guide analyzer
    pub indent_analyzer: IndentAnalyzer,
    /// Profile manager for config loading/saving
    pub profile_manager: ProfileManager,
    /// Name of the currently loaded profile
    pub current_profile_name: String,
    /// Flag indicating render is needed (for coalescing)
    pub(super) render_pending: bool,
    /// Modifier registry for style and behavior modifiers
    pub modifier_registry: ModifierRegistry,
    /// Decoration store for language-specific visual decorations
    pub decoration_store: DecorationStore,
    /// Event bus for type-erased plugin events
    pub event_bus: Arc<EventBus>,
    /// Plugin state registry for plugin-owned state
    pub plugin_state: Arc<PluginStateRegistry>,
    /// RPC handler registry for plugin-registered RPC methods
    pub rpc_handler_registry: crate::rpc::RpcHandlerRegistry,
    /// Display registry for plugin-provided mode display strings and icons
    pub display_registry: crate::display::DisplayRegistry,
    /// Render stage registry for pipeline transformations
    pub render_stages: Arc<std::sync::RwLock<crate::render::RenderStageRegistry>>,
    /// Option registry for extensible settings
    pub option_registry: Arc<OptionRegistry>,
    /// Ex-command registry for plugin-registered ex-commands
    pub ex_command_registry: Arc<crate::command_line::ExCommandRegistry>,
    /// Profile registry for configurable components
    pub profile_registry: Arc<crate::config::ProfileRegistry>,
    /// Loaded plugins for boot phase execution
    pub(crate) plugins: Vec<Box<dyn Plugin>>,
    /// Timestamp of last user input (for idle detection)
    pub(crate) last_input_at: std::time::Instant,
    /// Whether the idle shimmer effect is currently active
    pub(crate) idle_shimmer_active: bool,
    /// Landing page animation state (when showing landing page)
    pub(crate) landing_state: Option<crate::landing::LandingState>,
    /// Interactor registry for input behavior configuration
    pub interactor_registry: Arc<InteractorRegistry>,
    /// Current event scope for tracking event lifecycle (set during `handle_event`)
    pub(crate) current_scope: Option<crate::event_bus::EventScope>,
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
    #[allow(clippy::too_many_lines)]
    pub fn with_plugins<T: PluginTuple>(screen: Screen, plugins: T) -> Self {
        // Create dual channels for priority-based event processing
        // High-priority: user input, mode changes (smaller capacity, processed first)
        let (hi_tx, hi_rx) = mpsc::channel(HI_PRIORITY_CHANNEL_CAPACITY);
        // Low-priority: render signals, background tasks (larger capacity)
        let (lo_tx, lo_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        // Create prioritized sender wrapper
        let prioritized_sender = PrioritizedEventSender::new(hi_tx.clone(), lo_tx.clone());

        let (mode_tx, mode_rx) = watch::channel(ModeState::new());

        // Initialize event bus and plugin state registry
        let event_bus = Arc::new(EventBus::new(EVENT_CHANNEL_CAPACITY));
        let plugin_state = Arc::new(PluginStateRegistry::new());

        // Initialize option registry EARLY (before plugins subscribe)
        // This allows RegisterOption events emitted during subscribe phase to be processed
        let option_registry = Arc::new(OptionRegistry::new());

        // Add handler for RegisterOption events BEFORE plugins subscribe
        // When event loop starts, queued events will be dispatched to this handler
        {
            let registry = Arc::clone(&option_registry);
            event_bus.subscribe::<RegisterOption, _>(10, move |event, _ctx| {
                if let Err(e) = registry.register(event.spec.clone()) {
                    tracing::warn!("Failed to register option via event: {e}");
                }
                EventResult::Handled
            });
        }

        // Initialize profile manager
        let profile_manager = ProfileManager::default();
        let default_profile_name = profile_manager.default_profile_name().to_string();

        // Load plugins with state registry and event bus integration
        let mut ctx = PluginContext::new();
        let mut loader = PluginLoader::new();
        loader.add_plugins(plugins);
        let loaded_plugins = loader
            .load_with_state(&mut ctx, &plugin_state, &event_bus)
            .expect("Plugin loading failed");

        // Extract components from plugin context
        let (
            command_registry,
            modifier_registry,
            keymap,
            rpc_handler_registry,
            display_registry,
            render_stages,
            option_specs,
            mut interactor_registry,
        ) = ctx.into_parts();

        // Register built-in interactor configs (e.g., Window mode doesn't accept char input)
        interactor_registry.register_builtins();

        // Register build-phase options (backward compatibility with ctx.option())
        for spec in option_specs {
            if let Err(e) = option_registry.register(spec) {
                tracing::warn!("Failed to register option: {e}");
            }
        }

        // Initialize ex-command registry for plugin-registered commands
        let ex_command_registry = Arc::new(crate::command_line::ExCommandRegistry::new());

        // Initialize profile registry for configurable components
        let profile_registry = Arc::new(crate::config::ProfileRegistry::new());

        // Wrap render_stages in Arc<RwLock<>> and inject into plugin_state
        // This allows plugins to register stages from init_state()
        let render_stages = Arc::new(std::sync::RwLock::new(render_stages));
        plugin_state.set_render_stages(Arc::clone(&render_stages));

        // Initialize animation system
        // Frame rate of 30 fps provides smooth transitions without excessive CPU usage
        // Animation sends render signals which are low-priority events
        let animation_system = AnimationSystem::spawn(lo_tx.clone(), 30);
        plugin_state.set_animation_handle(animation_system.handle().clone());
        plugin_state.set_animation_state(animation_system.state());
        // Register the animation render stage
        render_stages
            .write()
            .unwrap()
            .register(std::sync::Arc::new(animation_system.render_stage()));

        let mut runtime = Self {
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
            hi_tx,
            hi_rx,
            lo_tx,
            lo_rx,
            prioritized_sender,
            initial_file: None,
            showing_landing_page: false,
            mode_tx,
            mode_rx,
            command_registry: Arc::new(command_registry),
            keymap,
            next_buffer_id: 0,
            jump_list: JumpList::new(),
            indent_analyzer: IndentAnalyzer::default(),
            profile_manager,
            current_profile_name: default_profile_name,
            render_pending: false,
            modifier_registry,
            decoration_store: DecorationStore::new(),
            event_bus,
            plugin_state,
            rpc_handler_registry,
            display_registry,
            render_stages,
            option_registry,
            ex_command_registry,
            profile_registry,
            plugins: loaded_plugins,
            last_input_at: std::time::Instant::now(),
            idle_shimmer_active: false,
            landing_state: None,
            interactor_registry: Arc::new(interactor_registry),
            current_scope: None,
        };

        // Make keymap and command registry accessible to plugins (e.g., which-key)
        runtime
            .plugin_state
            .set_keymap(Arc::new(runtime.keymap.clone()));
        runtime
            .plugin_state
            .set_command_registry(Arc::clone(&runtime.command_registry));

        // Register built-in display info for status line
        runtime.display_registry.register_builtins(&runtime.theme);

        // Register all event subscriptions from plugins
        runtime.register_event_subscribers();

        // Enable keyboard enhancement protocol if supported
        // This allows terminals (kitty, WezTerm, foot) to disambiguate Ctrl+I from Tab
        if reovim_sys::terminal::supports_keyboard_enhancement().unwrap_or(false) {
            use reovim_sys::{
                ExecutableCommand,
                event::{KeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
            };

            match std::io::stdout().execute(PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES,
            )) {
                Ok(_) => {
                    tracing::info!("Keyboard enhancement protocol enabled");
                }
                Err(e) => {
                    tracing::warn!("Failed to enable keyboard enhancement: {}", e);
                }
            }
        } else {
            tracing::debug!("Keyboard enhancement not supported by terminal");
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

        // Capture old mode description for ModeChanged event
        let from_mode = format!("{:?}", self.mode_state.edit_mode);
        let to_mode = format!("{:?}", mode_state.edit_mode);

        // Handle undo batching on insert mode transitions
        let active_buf_id = self.active_buffer_id();
        if !was_insert && is_insert {
            // Entering insert mode: begin batching
            if let Some(buf) = self.buffers.get_mut(&active_buf_id) {
                buf.begin_batch();
            }
        } else if was_insert && !is_insert {
            // Leaving insert mode: flush batch and record jump position
            if let Some(buf) = self.buffers.get_mut(&active_buf_id) {
                buf.flush_batch();
                // Record current position as a jump point when leaving insert mode
                // This allows navigating back to where editing was completed
                // Use push_current() which doesn't truncate (preserves jump history)
                self.jump_list.push_current(active_buf_id, buf.cur);
            }
        }

        self.mode_state = mode_state.clone();
        let _ = self.mode_tx.send(mode_state);

        // Dispatch ModeChanged event to event bus for plugin subscriptions
        let dyn_event = DynEvent::new(ModeChanged {
            from: from_mode,
            to: to_mode,
        });
        let sender = self.event_bus.sender();
        let mut ctx = HandlerContext::new(&sender);
        let _ = self.event_bus.dispatch(&dyn_event, &mut ctx);
    }

    /// Get current mode state
    #[must_use]
    pub const fn current_mode(&self) -> &ModeState {
        &self.mode_state
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
    /// Emit an event to the `EventBus`, attaching the current scope if present.
    ///
    /// When a scope is present (during `handle_event` processing), this dispatches
    /// the event synchronously to ensure proper scope tracking. When no scope is
    /// present, the event is queued for async processing.
    pub fn emit_event<E: crate::event_bus::Event>(&self, event: E) {
        if let Some(ref scope) = self.current_scope {
            // Synchronous dispatch when scope is present
            // This ensures the scope counter is accurate and wait() works correctly
            scope.increment();
            let dyn_event = crate::event_bus::DynEvent::new(event).with_scope(scope.clone());

            let sender = self.event_bus.sender();
            let mut ctx =
                crate::event_bus::HandlerContext::new(&sender).with_scope(Some(scope.clone()));
            self.event_bus.dispatch(&dyn_event, &mut ctx);

            // Decrement after dispatch completes
            scope.decrement();
        } else {
            // Async queue when no scope tracking needed
            self.event_bus.emit(event);
        }
    }

    /// Open a file, creating a new buffer or switching to existing one
    pub fn open_file(&mut self, path: &str) {
        debug!(path, "open_file: called");

        // Check if this file is already open in a buffer
        for (id, buf) in &self.buffers {
            if buf.file_path.as_deref() == Some(path) {
                debug!(id, path, "open_file: file already open, switching buffer");
                self.screen.set_editor_buffer(*id);
                return;
            }
        }

        // Create new buffer from file
        if let Some(id) = self.create_buffer_from_file(path) {
            debug!(id, path, "open_file: created new buffer");
            self.screen.set_editor_buffer(id);
            self.showing_landing_page = false;
            self.landing_state = None;

            // Emit initial ViewportScrolled to trigger context computation
            if let Some(viewport_info) = self.screen.get_viewport_info(id) {
                self.event_bus.emit(ViewportScrolled {
                    window_id: viewport_info.window_id,
                    buffer_id: viewport_info.buffer_id,
                    top_line: viewport_info.top_line,
                    bottom_line: viewport_info.bottom_line,
                });
                debug!(id, "open_file: emitted initial ViewportScrolled");
            }
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
            // Use emit_event to propagate scope for deterministic completion tracking
            self.emit_event(crate::event_bus::BufferModified {
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
        Self::active_buffer_id(self)
    }

    fn mode(&self) -> &ModeState {
        &self.mode_state
    }

    fn set_mode(&mut self, mode: ModeState) {
        // Inline the mode change logic here to avoid recursion with the inherent method
        let was_insert = self.mode_state.is_insert();
        let is_insert = mode.is_insert();

        // Handle undo batching on insert mode transitions
        let active_buf_id = self.active_buffer_id();
        if !was_insert && is_insert {
            // Entering insert mode: begin batching
            if let Some(buf) = self.buffers.get_mut(&active_buf_id) {
                buf.begin_batch();
            }
        } else if was_insert && !is_insert {
            // Leaving insert mode: flush batch and record jump position
            if let Some(buf) = self.buffers.get_mut(&active_buf_id) {
                buf.flush_batch();
                // Record current position as a jump point when leaving insert mode
                // This allows navigating back to where editing was completed
                // Use push_current() which doesn't truncate (preserves jump history)
                self.jump_list.push_current(active_buf_id, buf.cur);
            }
        }

        self.mode_state = mode.clone();
        let _ = self.mode_tx.send(mode);
    }

    fn screen_size(&self) -> (u16, u16) {
        (self.screen.width(), self.screen.height())
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // Clean up keyboard enhancement protocol
        use reovim_sys::{ExecutableCommand, event::PopKeyboardEnhancementFlags};

        let _ = std::io::stdout().execute(PopKeyboardEnhancementFlags);
    }
}
