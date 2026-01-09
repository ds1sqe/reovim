//! Core Runtime struct and initialization

use std::{collections::BTreeMap, sync::Arc};

/// Function pointer type for focus input handlers (enlist pattern)
pub type FocusInputHandler = fn(&mut Runtime, Option<char>, bool, bool);

use {
    crate::{
        animation::AnimationSystem,
        bind::KeyMap,
        buffer::{Buffer, TextOps},
        command::CommandRegistry,
        command_line::CommandLine,
        component::RenderState,
        config::{ProfileConfig, ProfileManager},
        constants::{EVENT_CHANNEL_CAPACITY, HI_PRIORITY_CHANNEL_CAPACITY},
        decoration::{DecorationStore, LanguageRendererRegistry},
        event::RuntimeEvent,
        event_bus::{
            BufferClosed, DynEvent, EventBus, EventResult, EventSender, FileOpened, HandlerContext,
            ViewportScrolled, core_events::ModeChanged,
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
    /// Modifier registry for style and behavior modifiers
    pub modifier_registry: ModifierRegistry,
    /// Decoration store for language-specific visual decorations
    pub decoration_store: DecorationStore,
    /// Language renderer registry for decoration generation
    pub renderer_registry: LanguageRendererRegistry,
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
            renderer_registry: LanguageRendererRegistry::new(),
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

        // Subscribe to focus change requests from plugins
        // HIGH PRIORITY: Focus changes are user-visible and should be processed immediately
        {
            use crate::event_bus::{EventResult, core_events::RequestFocusChange};
            let mode_tx = runtime.mode_tx.clone();
            let hi_tx = runtime.hi_tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestFocusChange, _>(100, move |event, _ctx| {
                    let current_mode = mode_tx.borrow().clone();
                    let new_mode = current_mode.set_interactor_id(event.target);
                    // Immediately update mode_tx so CommandHandler sees the new mode
                    // before processing subsequent keys (fixes race condition)
                    let _ = mode_tx.send(new_mode.clone());
                    // Also send through event loop to update runtime.mode_state
                    let _ = hi_tx.try_send(RuntimeEvent::mode_change(new_mode));
                    tracing::info!(
                        "Runtime: Requesting focus change to component '{}'",
                        event.target.0
                    );
                    EventResult::Handled
                });
        }

        // Subscribe to mode change requests from plugins
        // HIGH PRIORITY: Mode changes are user-visible and should be processed immediately
        {
            use crate::event_bus::{EventResult, core_events::RequestModeChange};
            let mode_tx = runtime.mode_tx.clone();
            let hi_tx = runtime.hi_tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestModeChange, _>(100, move |event, _ctx| {
                    // Immediately update mode_tx so CommandHandler sees the new mode
                    // before processing subsequent keys (fixes race condition)
                    let _ = mode_tx.send(event.mode.clone());
                    // Also send through event loop to update runtime.mode_state
                    let _ = hi_tx.try_send(RuntimeEvent::mode_change(event.mode.clone()));
                    tracing::info!(
                        "Runtime: Requesting mode change to interactor='{}', edit_mode={:?}, sub_mode={:?}",
                        event.mode.interactor_id.0,
                        event.mode.edit_mode,
                        event.mode.sub_mode
                    );
                    EventResult::Handled
                });
        }

        // Subscribe to file open requests from plugins
        // LOW PRIORITY: File loading is a background operation
        {
            use crate::event_bus::{EventResult, core_events::RequestOpenFile};
            let lo_tx = runtime.lo_tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestOpenFile, _>(100, move |event, _ctx| {
                    tracing::info!("Runtime: Requesting to open file: {:?}", event.path);
                    // Send OpenFileRequest to the runtime event loop
                    let _ = lo_tx.try_send(RuntimeEvent::open_file(event.path.clone()));
                    EventResult::Handled
                });
        }

        // Subscribe to file open at position requests from plugins (LSP navigation)
        // LOW PRIORITY: File loading is a background operation
        {
            use crate::event_bus::{EventResult, core_events::RequestOpenFileAtPosition};
            let lo_tx = runtime.lo_tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestOpenFileAtPosition, _>(100, move |event, _ctx| {
                    tracing::info!(
                        "Runtime: Requesting to open file at position: {:?}:{}:{}",
                        event.path,
                        event.line,
                        event.column
                    );
                    let _ = lo_tx.try_send(RuntimeEvent::open_file_at(
                        event.path.clone(),
                        event.line,
                        event.column,
                    ));
                    EventResult::Handled
                });
        }

        // Subscribe to register set requests from plugins
        // LOW PRIORITY: Register operations are background tasks
        {
            use crate::event_bus::{EventResult, core_events::RequestSetRegister};
            let lo_tx = runtime.lo_tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestSetRegister, _>(100, move |event, _ctx| {
                    tracing::debug!("Runtime: Requesting to set register {:?}", event.register);
                    let _ = lo_tx
                        .try_send(RuntimeEvent::set_register(event.register, event.text.clone()));
                    EventResult::Handled
                });
        }

        // Subscribe to ex-command registration from plugins
        {
            use crate::event_bus::{EventResult, core_events::RegisterExCommand};
            let registry = Arc::clone(&runtime.ex_command_registry);
            runtime
                .event_bus
                .subscribe::<RegisterExCommand, _>(50, move |event, _ctx| {
                    registry.register(event.name.to_string(), event.handler.clone());
                    tracing::debug!("Runtime: Registered ex-command '{}'", event.name);
                    EventResult::Handled
                });
        }

        // Subscribe to configurable component registration from plugins
        {
            use crate::event_bus::{EventResult, core_events::RegisterConfigurable};
            let registry = Arc::clone(&runtime.profile_registry);
            runtime
                .event_bus
                .subscribe::<RegisterConfigurable, _>(40, move |event, _ctx| {
                    registry.register(event.component.clone());
                    EventResult::Handled
                });
        }

        // Subscribe to profile load events
        {
            use crate::event_bus::{EventResult, core_events::ProfileLoadEvent};
            runtime
                .event_bus
                .subscribe::<ProfileLoadEvent, _>(100, move |event, ctx| {
                    // Request render after profile load
                    ctx.request_render();
                    tracing::info!(profile = %event.name, "Profile load requested (full implementation pending)");
                    // TODO: Actually load the profile using profile_registry and profile_manager
                    // This requires Runtime to be mutable, which needs architectural changes
                    EventResult::Handled
                });
        }

        // Subscribe to profile save events
        {
            use crate::event_bus::{EventResult, core_events::ProfileSaveEvent};
            runtime
                .event_bus
                .subscribe::<ProfileSaveEvent, _>(100, move |event, _ctx| {
                    tracing::info!(profile = %event.name, "Profile save requested (full implementation pending)");
                    // TODO: Actually save the profile using profile_registry and profile_manager
                    // This requires Runtime to be mutable, which needs architectural changes
                    EventResult::Handled
                });
        }

        // Subscribe to text insert requests from plugins (for auto-pair insertion)
        // HIGH PRIORITY: Auto-pair and completion insertions must be processed immediately
        // to maintain <16ms latency target for user-initiated text input
        {
            use crate::{
                bind::CommandRef,
                command::{CommandContext, id::builtin},
                event::{CommandEvent, TextInputEvent},
                event_bus::{EventResult, core_events::RequestInsertText},
            };
            let hi_tx = runtime.hi_tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestInsertText, _>(100, move |event, _ctx| {
                    tracing::trace!("Runtime: Inserting text via plugin request: {:?}", event.text);

                    // Delete prefix first (for completion: replace typed prefix with full word)
                    for _ in 0..event.delete_prefix_len {
                        let _ = hi_tx
                            .try_send(RuntimeEvent::text_input(TextInputEvent::DeleteCharBackward));
                    }

                    // Insert each character
                    for c in event.text.chars() {
                        let _ =
                            hi_tx.try_send(RuntimeEvent::text_input(TextInputEvent::InsertChar(c)));
                    }

                    // If requested, move cursor left after insertion
                    if event.move_cursor_left {
                        let _ = hi_tx.try_send(RuntimeEvent::command(CommandEvent {
                            command: CommandRef::Registered(builtin::CURSOR_LEFT),
                            context: CommandContext::default(),
                        }));
                    }

                    EventResult::Handled
                });
        }

        // Subscribe to RequestCursorMove (for plugin-driven jumps)
        // HIGH PRIORITY: Cursor movements are user-visible and should be processed immediately
        {
            use crate::{
                command::traits::OperatorMotionAction,
                event_bus::{EventResult, core_events::RequestCursorMove},
                modd::{OperatorType, SubMode},
                motion::Motion,
            };
            let hi_tx = runtime.hi_tx.clone();
            let mode_tx = runtime.mode_tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestCursorMove, _>(100, move |event, _ctx| {
                    tracing::trace!(
                        "Runtime: RequestCursorMove to line={}, col={} in buffer={}",
                        event.line,
                        event.column,
                        event.buffer_id
                    );

                    // Check for operator from motion_context first, then fallback to mode
                    let operator_info = event.motion_context.as_ref().map_or_else(
                        || {
                            // Fallback: check if we're in operator-pending mode
                            let current_mode = mode_tx.borrow().clone();
                            match current_mode.sub_mode {
                                SubMode::OperatorPending { operator, count } => {
                                    Some((operator, count))
                                }
                                _ => None,
                            }
                        },
                        |motion_ctx| motion_ctx.operator.map(|op| (op, motion_ctx.count)),
                    );

                    match operator_info {
                        Some((operator, op_count)) => {
                            // Create operator motion action for jump
                            tracing::debug!(
                                "Runtime: Jump with operator {:?}, op_count={:?}",
                                operator,
                                op_count
                            );

                            let motion = Motion::JumpTo {
                                line: event.line,
                                column: event.column,
                            };

                            let action = match operator {
                                OperatorType::Delete => OperatorMotionAction::Delete {
                                    motion,
                                    count: op_count.unwrap_or(1),
                                },
                                OperatorType::Yank => OperatorMotionAction::Yank {
                                    motion,
                                    count: op_count.unwrap_or(1),
                                },
                                OperatorType::Change => OperatorMotionAction::Change {
                                    motion,
                                    count: op_count.unwrap_or(1),
                                },
                            };

                            // Send operator motion event (runtime will handle mode change)
                            let _ = hi_tx.try_send(RuntimeEvent::operator_motion(action));
                        }
                        None => {
                            // Normal cursor move (no operator)
                            let _ = hi_tx.try_send(RuntimeEvent::move_cursor(
                                event.buffer_id,
                                event.line,
                                event.column,
                            ));
                        }
                    }

                    EventResult::Handled
                });
        }

        // Subscribe to settings/option capability requests
        // LOW PRIORITY: Settings changes are background operations
        {
            use crate::event_bus::{EventResult, core_events::RequestSetLineNumbers};
            let lo_tx = runtime.lo_tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestSetLineNumbers, _>(100, move |event, _ctx| {
                    let _ = lo_tx.try_send(RuntimeEvent::set_line_numbers(event.enabled));
                    EventResult::Handled
                });
        }
        {
            use crate::event_bus::{EventResult, core_events::RequestSetRelativeLineNumbers};
            let lo_tx = runtime.lo_tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestSetRelativeLineNumbers, _>(100, move |event, _ctx| {
                    let _ = lo_tx.try_send(RuntimeEvent::set_relative_line_numbers(event.enabled));
                    EventResult::Handled
                });
        }
        {
            use crate::event_bus::{EventResult, core_events::RequestSetTheme};
            let lo_tx = runtime.lo_tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestSetTheme, _>(100, move |event, _ctx| {
                    let _ = lo_tx.try_send(RuntimeEvent::set_theme(event.name.clone()));
                    EventResult::Handled
                });
        }
        {
            use crate::event_bus::{EventResult, core_events::RequestSetScrollbar};
            let lo_tx = runtime.lo_tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestSetScrollbar, _>(100, move |event, _ctx| {
                    let _ = lo_tx.try_send(RuntimeEvent::set_scrollbar(event.enabled));
                    EventResult::Handled
                });
        }
        {
            use crate::event_bus::{EventResult, core_events::RequestSetIndentGuide};
            let lo_tx = runtime.lo_tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestSetIndentGuide, _>(100, move |event, _ctx| {
                    let _ = lo_tx.try_send(RuntimeEvent::set_indent_guide(event.enabled));
                    EventResult::Handled
                });
        }
        {
            use crate::event_bus::{EventResult, core_events::RequestSetSignColumn};
            let lo_tx = runtime.lo_tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestSetSignColumn, _>(100, move |event, _ctx| {
                    let _ = lo_tx.try_send(RuntimeEvent::set_sign_column(event.mode));
                    EventResult::Handled
                });
        }

        // Subscribe to command line completion apply requests
        // HIGH PRIORITY: Command line completion is user-initiated and should be processed immediately
        {
            use crate::event_bus::{EventResult, core_events::RequestApplyCmdlineCompletion};
            let hi_tx = runtime.hi_tx.clone();
            runtime
                .event_bus
                .subscribe::<RequestApplyCmdlineCompletion, _>(100, move |event, _ctx| {
                    let _ = hi_tx.try_send(RuntimeEvent::apply_cmdline_completion(
                        event.text.clone(),
                        event.replace_start,
                    ));
                    EventResult::Handled
                });
        }

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
    pub(crate) fn render(&mut self) {
        let render_start = std::time::Instant::now();

        // PHASE 1: Instant render (reads from cache, never waits)
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
            display_registry: &self.display_registry,
        };
        let pre_render = render_start.elapsed();
        self.screen
            .render_with_state(&state)
            .expect("failed to render");
        let post_render = render_start.elapsed();
        self.screen.flush().expect("failed to flush");
        let flush_time = render_start.elapsed();

        // PHASE 2: Cache update (blocking but after user sees response)
        // This ensures next render has fresh data
        self.update_visible_highlights();
        let saturator_time = render_start.elapsed().saturating_sub(flush_time);

        tracing::trace!(
            "[RTT] render: state_build={:?} screen_render={:?} flush={:?} saturator={:?} total={:?}",
            pre_render,
            post_render.saturating_sub(pre_render),
            flush_time.saturating_sub(post_render),
            saturator_time,
            render_start.elapsed()
        );
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

    /// SATURATOR: Request highlight/decoration updates for visible buffers
    ///
    /// This is NON-BLOCKING - sends requests to background saturator tasks.
    /// Render reads from cache immediately (may show stale data on first render).
    /// Saturator sends `RenderSignal` when cache is updated.
    #[allow(clippy::cast_possible_truncation)]
    fn update_visible_highlights(&mut self) {
        use crate::content::WindowContentSource;

        const HIGHLIGHT_PADDING: u16 = 10;

        // Collect (buffer_id, viewport_start, viewport_end) for all visible windows
        let viewports: Vec<(usize, u16, u16)> = self
            .screen
            .windows()
            .iter()
            .filter_map(|win| {
                if let WindowContentSource::FileBuffer { buffer_anchor, .. } = &win.source {
                    let buffer_id = win.buffer_id()?;
                    let line_count = self.buffers.get(&buffer_id)?.contents.len() as u16;
                    let viewport_start = buffer_anchor.y.saturating_sub(HIGHLIGHT_PADDING);
                    let viewport_end =
                        (buffer_anchor.y + win.height + HIGHLIGHT_PADDING).min(line_count);
                    Some((buffer_id, viewport_start, viewport_end))
                } else {
                    None
                }
            })
            .collect();

        // Request saturator updates (non-blocking) or fall back to sync for buffers without saturator
        for (buffer_id, viewport_start, viewport_end) in viewports {
            if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
                if buffer.has_saturator() {
                    // Non-blocking: send request to background task
                    buffer.request_saturator_update(viewport_start, viewport_end);
                } else {
                    // Fallback: sync update for buffers without saturator
                    buffer.update_highlights(viewport_start, viewport_end);
                    buffer.update_decorations(viewport_start, viewport_end);
                }
            }
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

                // Try to create syntax provider from factory (if available)
                if let Some(factory) = self.plugin_state.syntax_factory()
                    && let Some(mut syntax) = factory.create_syntax(id, path, &content)
                {
                    // Do initial parse
                    syntax.parse(&content);
                    buffer.attach_syntax(syntax);
                    debug!(id, path, "create_buffer_from_file: attached syntax provider");
                }

                // Try to create decoration provider from factory (if available)
                if let Some(factory) = self.plugin_state.decoration_factory()
                    && let Some(mut decorator) = factory.create_provider(path, &content)
                {
                    // Initial refresh to parse content
                    decorator.refresh(&content);
                    buffer.attach_decoration_provider(decorator);
                    debug!(id, path, "create_buffer_from_file: attached decoration provider");
                }

                // Start background saturator if syntax/decoration providers exist
                // Saturator sends render signals which are low-priority events
                if buffer.has_syntax() || buffer.has_decoration_provider() {
                    buffer.start_saturator(self.lo_tx.clone());
                    debug!(id, path, "create_buffer_from_file: started saturator");
                }

                self.buffers.insert(id, buffer);

                // Emit FileOpened event for plugins that need to know about new files
                // Use emit_event to propagate scope for deterministic completion tracking
                self.emit_event(FileOpened {
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
            self.screen.set_editor_buffer(buffer_id);
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
            // Use emit_event to propagate scope for deterministic completion tracking
            self.emit_event(BufferClosed { buffer_id });

            // If we closed the active buffer, switch to another one
            if self.active_buffer_id() == buffer_id
                && let Some(&new_id) = self.buffers.keys().next()
            {
                self.screen.set_editor_buffer(new_id);
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
        let current_pos = ids.iter().position(|&id| id == self.active_buffer_id())?;
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
        let current_pos = ids.iter().position(|&id| id == self.active_buffer_id())?;
        let next_pos = (current_pos + 1) % ids.len();
        Some(ids[next_pos])
    }

    /// Get the currently active buffer ID (derived from active window)
    ///
    /// This is the single source of truth for the active buffer, derived from
    /// the screen's active window. Returns 0 if no active window exists.
    #[must_use]
    pub fn active_buffer_id(&self) -> usize {
        self.screen.active_buffer_id().unwrap_or(0)
    }

    /// Get the currently active buffer
    #[must_use]
    pub fn active_buffer(&self) -> Option<&Buffer> {
        self.buffers.get(&self.active_buffer_id())
    }

    /// Get the currently active buffer mutably
    pub fn active_buffer_mut(&mut self) -> Option<&mut Buffer> {
        self.buffers.get_mut(&self.active_buffer_id())
    }

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

        // Apply theme with overrides
        if let Some(theme_name) = ThemeName::parse(&config.editor.theme) {
            self.theme =
                Theme::from_name_with_overrides(theme_name, &config.editor.theme_overrides);
            // Theme change will trigger rehighlighting via event bus
            self.rehighlight_all_buffers();
            debug!(
                theme = %config.editor.theme,
                overrides = config.editor.theme_overrides.len(),
                "Applied theme with overrides from profile"
            );
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
        crate::rpc::ModeSnapshot::from_mode(&self.mode_state, &self.display_registry)
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
            active_buffer_id: self.active_buffer_id(),
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
                .get(&self.active_buffer_id())
                .map(|buf| crate::visual::CursorInfo {
                    x: buf.cur.x,
                    y: buf.cur.y,
                    layer: "editor".to_string(),
                });

        // Get layer info - just base layers for now
        // Plugin windows provide their own z-order during rendering
        let layers = vec![
            crate::visual::LayerInfo {
                name: "base".to_string(),
                z_order: 0,
                visible: true,
                bounds: crate::visual::BoundsInfo::new(0, 0, buffer.width(), buffer.height()),
            },
            crate::visual::LayerInfo {
                name: "editor".to_string(),
                z_order: 2,
                visible: true,
                bounds: crate::visual::BoundsInfo::new(
                    0,
                    1,
                    buffer.width(),
                    buffer.height().saturating_sub(2),
                ),
            },
        ];

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
