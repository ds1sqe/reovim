//! Core Runtime struct and initialization

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use {
    crate::{
        buffer::{Buffer, TextOps},
        command::CommandRegistry,
        command_line::CommandLine,
        completion::{CompletionContext, CompletionEngine, CompletionItem, CompletionState},
        config::{ProfileConfig, ProfileManager},
        constants::EVENT_CHANNEL_CAPACITY,
        event::{CompletionEvent, InnerEvent},
        explorer::ExplorerState,
        focus::FocusRegistry,
        folding::FoldManager,
        highlight::{ColorMode, HighlightStore, Theme},
        indent::IndentAnalyzer,
        jumplist::JumpList,
        leap::LeapState,
        modd::ModeState,
        modifier::{ModifierContext, ModifierRegistry},
        register::Registers,
        screen::{Screen, WhichKeyPanel},
        settings_menu::SettingsMenuState,
        telescope::{
            TelescopeMatcher, TelescopeState,
            picker::{
                BuffersPicker, CommandsPicker, FilesPicker, GrepPicker, HelpPicker, KeymapsPicker,
                Picker, ProfilesPicker, RecentPicker, ThemesPicker,
            },
        },
        treesitter::TreesitterManager,
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
    /// Currently active buffer ID
    pub active_buffer_id: usize,
    /// Next buffer ID to assign
    next_buffer_id: usize,
    /// File explorer state
    pub explorer_state: Option<ExplorerState>,
    /// Jump list for Ctrl-O/Ctrl-I navigation
    pub jump_list: JumpList,
    /// Which-key popup panel state
    pub which_key_panel: WhichKeyPanel,
    /// Completion engine
    pub completion_engine: Arc<CompletionEngine>,
    /// Current completion state
    pub completion_state: CompletionState,
    /// Cache of unfiltered completion items for re-filtering
    pub(crate) completion_items_cache: Vec<CompletionItem>,
    /// Watch channel sender for broadcasting completion active state
    pub(crate) completion_active_tx: watch::Sender<bool>,
    /// Watch channel receiver for completion active state
    completion_active_rx: watch::Receiver<bool>,
    /// Telescope fuzzy finder state
    pub telescope_state: TelescopeState,
    /// Telescope fuzzy matcher
    pub telescope_matcher: TelescopeMatcher,
    /// Telescope pickers registry
    pub telescope_pickers: HashMap<String, Arc<dyn Picker>>,
    /// Leap motion state
    pub leap_state: LeapState,
    /// Treesitter manager for syntax highlighting
    pub treesitter: TreesitterManager,
    /// Code folding manager
    pub fold_manager: FoldManager,
    /// Indent guide analyzer
    pub indent_analyzer: IndentAnalyzer,
    /// Profile manager for config loading/saving
    pub profile_manager: ProfileManager,
    /// Name of the currently loaded profile
    pub current_profile_name: String,
    /// Settings menu state
    pub settings_menu: SettingsMenuState,
    /// Flag indicating render is needed (for coalescing)
    render_pending: bool,
    /// Focus registry for extensible focus targets
    pub focus_registry: FocusRegistry,
    /// Modifier registry for style and behavior modifiers
    pub modifier_registry: ModifierRegistry,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new(Screen::default())
    }
}

impl Runtime {
    /// Create a new Runtime with the given screen
    #[must_use]
    pub fn new(screen: Screen) -> Self {
        let (tx, rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let (mode_tx, mode_rx) = watch::channel(ModeState::new());
        let (completion_active_tx, completion_active_rx) = watch::channel(false);

        // Initialize profile manager
        let profile_manager = ProfileManager::default();
        let default_profile_name = profile_manager.default_profile_name().to_string();

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
            tx,
            rx,
            initial_file: None,
            showing_landing_page: false,
            mode_tx,
            mode_rx,
            command_registry: Arc::new(CommandRegistry::with_defaults()),
            active_buffer_id: 0,
            next_buffer_id: 1, // Start at 1 since 0 is reserved for initial buffer
            explorer_state: None,
            jump_list: JumpList::new(),
            which_key_panel: WhichKeyPanel::new(),
            completion_engine: Arc::new(CompletionEngine::default()),
            completion_state: CompletionState::new(),
            completion_items_cache: Vec::new(),
            completion_active_tx,
            completion_active_rx,
            telescope_state: TelescopeState::new(),
            telescope_matcher: TelescopeMatcher::new(),
            telescope_pickers: Self::create_telescope_pickers(),
            leap_state: LeapState::new(),
            treesitter: TreesitterManager::new(),
            fold_manager: FoldManager::new(),
            indent_analyzer: IndentAnalyzer::default(),
            profile_manager,
            current_profile_name: default_profile_name,
            settings_menu: SettingsMenuState::new(),
            render_pending: false,
            focus_registry: FocusRegistry::with_defaults(),
            modifier_registry: ModifierRegistry::new(),
        };

        // Enable diff-based rendering by default
        runtime.screen.enable_frame_renderer();

        runtime
    }

    /// Create the telescope picker registry
    fn create_telescope_pickers() -> HashMap<String, Arc<dyn Picker>> {
        let mut pickers: HashMap<String, Arc<dyn Picker>> = HashMap::new();
        pickers.insert("files".to_string(), Arc::new(FilesPicker::new()));
        pickers.insert("buffers".to_string(), Arc::new(BuffersPicker::new()));
        pickers.insert("commands".to_string(), Arc::new(CommandsPicker::new()));
        pickers.insert("keymaps".to_string(), Arc::new(KeymapsPicker::new()));
        pickers.insert("grep".to_string(), Arc::new(GrepPicker::new()));
        pickers.insert("recent".to_string(), Arc::new(RecentPicker::new()));
        pickers.insert("help".to_string(), Arc::new(HelpPicker::new()));
        pickers.insert("themes".to_string(), Arc::new(ThemesPicker::new()));
        pickers.insert("profiles".to_string(), Arc::new(ProfilesPicker::new()));
        pickers
    }

    /// Subscribe to mode changes
    #[must_use]
    pub fn subscribe_mode(&self) -> watch::Receiver<ModeState> {
        self.mode_rx.clone()
    }

    /// Subscribe to completion active state changes
    #[must_use]
    pub fn subscribe_completion_active(&self) -> watch::Receiver<bool> {
        self.completion_active_rx.clone()
    }

    /// Broadcast completion active state change
    pub(crate) fn set_completion_active(&self, active: bool) {
        let _ = self.completion_active_tx.send(active);
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
            self.mode_state.focus_id,
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
        self.screen
            .render(
                &self.buffers,
                &self.highlight_store,
                &self.mode_state,
                &self.command_line,
                &self.pending_keys,
                &self.last_command,
                self.color_mode,
                &self.theme,
                self.explorer_state.as_ref(),
                &self.which_key_panel,
                &self.completion_state,
                &self.telescope_state,
                &self.leap_state,
                &self.fold_manager,
                &self.indent_analyzer,
                &self.settings_menu,
            )
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
                self.buffers.insert(id, buffer.clone());

                // Initialize treesitter for this buffer
                let language_id = self.treesitter.init_buffer(id, Some(path));
                debug!(id, path, ?language_id, "create_buffer_from_file: treesitter initialized");

                // Generate initial syntax highlights and fold ranges
                #[allow(clippy::cast_possible_truncation)]
                if self.treesitter.has_parser(id) {
                    let line_count = buffer.contents.len() as u32;
                    let highlights = self.treesitter.parse_and_highlight(
                        id,
                        &content,
                        0,
                        line_count.saturating_sub(1),
                    );
                    if !highlights.is_empty() {
                        self.highlight_store.add(id, highlights);
                        debug!(id, "create_buffer_from_file: syntax highlights added");
                    }

                    // Compute fold ranges
                    let fold_ranges = self.treesitter.compute_fold_ranges(id, &content);
                    if !fold_ranges.is_empty() {
                        self.fold_manager.set_ranges(id, fold_ranges);
                        debug!(id, "create_buffer_from_file: fold ranges computed");
                    }
                }

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
            // Clean up treesitter state
            self.treesitter.remove_buffer(buffer_id);
            // Clean up highlights
            self.highlight_store.clear_all(buffer_id);
            // Clean up fold state
            self.fold_manager.remove_buffer(buffer_id);

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

    /// Trigger completion at current cursor position
    pub(crate) fn trigger_completion(&self, buffer_id: usize) {
        let Some(buffer) = self.buffers.get(&buffer_id) else {
            return;
        };

        // Build completion context
        let ctx = Self::build_completion_context(buffer);

        // Don't trigger if prefix is empty
        if ctx.prefix.is_empty() {
            return;
        }

        let content = buffer.contents.clone();
        let engine = self.completion_engine.clone();
        let tx = self.tx.clone();

        // Spawn async completion fetch
        tokio::spawn(async move {
            let items = engine.complete(&ctx, &content).await;
            if !items.is_empty() {
                let _ = tx
                    .send(InnerEvent::CompletionEvent(CompletionEvent::Update {
                        items,
                        prefix: ctx.prefix,
                        start_col: ctx.word_start_col,
                        start_row: ctx.position.y,
                    }))
                    .await;
            }
        });
    }

    /// Build completion context from buffer state
    #[allow(clippy::cast_possible_truncation)]
    fn build_completion_context(buffer: &Buffer) -> CompletionContext {
        let position = buffer.cur;
        let line = buffer
            .contents
            .get(position.y as usize)
            .map(|l| l.inner.clone())
            .unwrap_or_default();

        // Find word start by walking backward
        let chars: Vec<char> = line.chars().collect();
        let mut word_start = position.x as usize;
        while word_start > 0 {
            let ch = chars.get(word_start - 1).copied().unwrap_or(' ');
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            word_start -= 1;
        }

        let prefix = if word_start < position.x as usize {
            chars[word_start..position.x as usize].iter().collect()
        } else {
            String::new()
        };

        CompletionContext::new(buffer.id, position, line, word_start as u16, prefix)
    }

    /// Insert a completion item at current cursor position
    pub(crate) fn insert_completion(&mut self, item: &CompletionItem) {
        let Some(buffer) = self.buffers.get_mut(&0) else {
            return;
        };

        // Delete the prefix (characters from start_col to cursor)
        let prefix_len = self.completion_state.prefix.len();
        for _ in 0..prefix_len {
            buffer.delete_char_backward();
        }

        // Insert the completion text
        for ch in item.insert_text.chars() {
            buffer.insert_char(ch);
        }
    }

    /// Re-highlight all buffers after theme change
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn rehighlight_all_buffers(&mut self) {
        let buffer_ids: Vec<usize> = self.buffers.keys().copied().collect();
        for buffer_id in buffer_ids {
            if self.treesitter.has_parser(buffer_id) {
                let Some(buffer) = self.buffers.get(&buffer_id) else {
                    continue;
                };
                let content: String = buffer
                    .contents
                    .iter()
                    .map(|l| l.inner.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                let line_count = buffer.contents.len() as u32;
                let highlights = self.treesitter.parse_and_highlight(
                    buffer_id,
                    &content,
                    0,
                    line_count.saturating_sub(1),
                );
                self.highlight_store.clear_all(buffer_id);
                if !highlights.is_empty() {
                    self.highlight_store.add(buffer_id, highlights);
                }
            }
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
        use crate::{highlight::ThemeName, treesitter::TreesitterTheme};

        // Apply theme
        if let Some(theme_name) = ThemeName::parse(&config.editor.theme) {
            self.theme = Theme::from_name(theme_name);
            self.treesitter
                .set_theme(TreesitterTheme::from_theme_name(theme_name));
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
                buffer_id: w.buffer_id,
                buffer_anchor_x: w.buffer_anchor.x,
                buffer_anchor_y: w.buffer_anchor.y,
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
                    layer: if self.telescope_state.active {
                        "telescope"
                    } else if self.settings_menu.visible {
                        "settings"
                    } else {
                        "editor"
                    }
                    .to_string(),
                });

        // Get layer info
        let layers = self.screen.layer_info(
            self.explorer_state.is_some(),
            self.which_key_panel.visible,
            self.completion_state.active,
            self.telescope_state.active,
            self.leap_state.is_active(),
            self.settings_menu.visible,
        );

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
