//! Main event loop for the editor

use crate::buffer::{Buffer, SelectionOps, TextOps};
use crate::config::ProfileConfig;
use crate::event::{
    BufferEvent, CommandHandler, CompletionEvent, CompletionHandler, ExplorerEvent,
    HighlightEvent, InnerEvent, InputEventBroker, SettingsMenuEvent, TerminateHandler,
    TreesitterEvent, WindowEvent,
};
use crate::highlight::{HighlightGroup, Theme};
use crate::treesitter::TreesitterTheme;
use crate::modd::{EditMode, ModExtension, ModeState, SubMode};

use super::Runtime;

impl Runtime {
    /// Initialize and run the editor event loop
    #[allow(clippy::missing_panics_doc)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::future_not_send)]
    #[allow(clippy::single_match_else)]
    #[allow(clippy::collapsible_if)]
    #[allow(clippy::match_same_arms)]
    pub async fn init(mut self) {
        tracing::info!("Runtime initializing");

        let mut buffer = Buffer::empty(0);

        // Load file if provided, otherwise show landing page
        if let Some(ref path) = self.initial_file {
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    let line_count = content.lines().count();
                    buffer.set_content(&content);
                    tracing::info!(path = %path, lines = line_count, "File loaded");
                }
                Err(e) => {
                    tracing::warn!(path = %path, error = %e, "Failed to load file");
                }
            }
            buffer.file_path = Some(path.clone());
        } else {
            // Show landing page when no file is opened
            let landing_content = crate::landing::generate(
                self.screen.width(),
                self.screen.height().saturating_sub(1), // Reserve status line
            );
            buffer.set_content(&landing_content);
            self.showing_landing_page = true;
        }

        self.buffers.insert(0, buffer);
        let input_broker = InputEventBroker::with_event_sender(self.tx.clone());

        // Command handler for key-to-command translation
        // Pass mode receiver so CommandHandler can read mode from Runtime (single source of truth)
        let mode_rx = self.subscribe_mode();
        let completion_active_rx = self.subscribe_completion_active();
        let mut command_hdr = CommandHandler::new(
            self.tx.clone(),
            mode_rx,
            completion_active_rx,
            self.command_registry.clone(),
        );
        let mut terminate_hdr = TerminateHandler::new(self.tx.clone());

        // Completion handler for auto-triggering completion on typing
        let completion_mode_rx = self.subscribe_mode();
        let mut completion_hdr =
            CompletionHandler::with_defaults(self.tx.clone(), completion_mode_rx);

        input_broker.key_broker.enlist(&mut command_hdr);
        input_broker.key_broker.enlist(&mut terminate_hdr);
        input_broker.key_broker.enlist(&mut completion_hdr);

        tokio::spawn(async move { command_hdr.run().await });
        tokio::spawn(async move { terminate_hdr.run().await });
        tokio::spawn(async move { completion_hdr.run().await });
        tokio::spawn(async move { input_broker.subscribe().await });

        // Initial render to show content immediately
        self.render();

        tracing::debug!("Entering event loop");
        self.run_event_loop().await;

        tracing::debug!("Event loop ended, finalizing screen");
        let _ = self.screen.finalize();
    }

    /// The main event processing loop
    #[allow(clippy::collapsible_if)]
    #[allow(clippy::match_same_arms)]
    #[allow(clippy::future_not_send)]
    async fn run_event_loop(&mut self) {
        use std::time::Duration;
        use tokio::time::interval;

        // Interval for checking pending treesitter parses
        let mut treesitter_check_interval =
            interval(Duration::from_millis(crate::treesitter::TreesitterManager::DEBOUNCE_MS));

        loop {
            tokio::select! {
                // Check for incoming events
                next = self.rx.recv() => {
                    if let Some(ev) = next {
                        if self.handle_event(ev) {
                            break;
                        }
                    } else {
                        self.tx
                            .send(InnerEvent::KillSignal)
                            .await
                            .expect("cannot broadcast kill signal");
                        break;
                    }
                }

                // Periodically check for pending treesitter parses
                _ = treesitter_check_interval.tick() => {
                    self.process_pending_treesitter_parses();
                }
            }
        }
    }

    /// Process any pending treesitter parses that have passed the debounce threshold
    #[allow(clippy::cast_possible_truncation)]
    fn process_pending_treesitter_parses(&mut self) {
        let ready_buffer_ids = self.treesitter.get_ready_parses();

        for buffer_id in ready_buffer_ids {
            if let Some(buffer) = self.buffers.get(&buffer_id) {
                let content = buffer.content_to_string();
                let line_count = buffer.contents.len() as u32;

                // Clear old syntax highlights
                self.highlight_store
                    .clear_group(buffer_id, HighlightGroup::Syntax);

                // Perform full reparse (since we don't have edit info at this point)
                let highlights = self.treesitter.parse_and_highlight(
                    buffer_id,
                    &content,
                    0,
                    line_count.saturating_sub(1),
                );

                if !highlights.is_empty() {
                    self.highlight_store.add(buffer_id, highlights);
                }
                self.render();
            }
        }
    }

    /// Handle a single event. Returns true if the editor should quit.
    #[allow(clippy::collapsible_if)]
    #[allow(clippy::match_same_arms)]
    #[allow(clippy::too_many_lines)]
    pub(crate) fn handle_event(&mut self, ev: InnerEvent) -> bool {
        match ev {
            InnerEvent::BufferEvent(buffer_event) => match buffer_event {
                BufferEvent::SetContent { buffer_id, content } => {
                    if let Some(b) = self.buffers.get_mut(&buffer_id) {
                        b.set_content(&content);
                        self.render();
                    }
                }
                BufferEvent::LoadFile { buffer_id, path } => {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Some(b) = self.buffers.get_mut(&buffer_id) {
                            b.set_content(&content);
                            b.file_path = Some(path.to_string_lossy().to_string());
                            self.render();
                        }
                    }
                }
                BufferEvent::Create { buffer_id } => {
                    let buffer = Buffer::empty(buffer_id);
                    self.buffers.insert(buffer_id, buffer);
                }
                BufferEvent::Close { buffer_id } => {
                    self.close_buffer(buffer_id);
                    self.render();
                }
                BufferEvent::Switch { buffer_id } => {
                    self.switch_buffer(buffer_id);
                    self.render();
                }
            },
            InnerEvent::CommandEvent(cmd_event) => {
                if self.handle_command(cmd_event) {
                    return true;
                }
            }
            InnerEvent::ModeChangeEvent(new_mode) => {
                self.handle_mode_change(new_mode);
            }
            InnerEvent::PendingKeysEvent(keys) => {
                // If pending_keys is being cleared and had content, save as last_command
                if keys.is_empty() && !self.pending_keys.is_empty() {
                    self.last_command = self.pending_keys.clone();
                }
                self.pending_keys = keys;
                self.render();
            }
            InnerEvent::WindowEvent(window_event) => match window_event {
                WindowEvent::ToggleExplorer => {
                    self.screen.toggle_explorer();
                    self.render();
                }
                WindowEvent::FocusExplorer => {
                    self.screen.focus_explorer();
                    self.render();
                }
                WindowEvent::FocusEditor => {
                    self.screen.focus_editor();
                    self.render();
                }
                // TODO: Implement in Phase 7
                WindowEvent::SplitHorizontal { .. }
                | WindowEvent::SplitVertical { .. }
                | WindowEvent::Close { .. }
                | WindowEvent::CloseOthers
                | WindowEvent::FocusDirection { .. }
                | WindowEvent::MoveWindow { .. }
                | WindowEvent::Resize { .. }
                | WindowEvent::Equalize
                | WindowEvent::TabNew { .. }
                | WindowEvent::TabClose
                | WindowEvent::TabNext
                | WindowEvent::TabPrev
                | WindowEvent::TabGoto { .. } => {
                    // Window management events - to be implemented
                }
            },
            InnerEvent::ExplorerEvent(explorer_event) => match explorer_event {
                ExplorerEvent::Toggle => {
                    self.screen.toggle_explorer();
                    self.render();
                }
                ExplorerEvent::OpenFile { path } => {
                    self.open_file(&path.to_string_lossy());
                    self.screen.focus_editor();
                    self.render();
                }
                ExplorerEvent::Refresh => {
                    // TODO: Refresh explorer tree when explorer module is implemented
                    self.render();
                }
                ExplorerEvent::SetRoot { path: _ } => {
                    // TODO: Set explorer root when explorer module is implemented
                    self.render();
                }
            },
            InnerEvent::HighlightEvent(hl_event) => match hl_event {
                HighlightEvent::Add {
                    buffer_id,
                    highlights,
                } => {
                    self.highlight_store.add(buffer_id, highlights);
                    self.render();
                }
                HighlightEvent::ClearGroup { buffer_id, group } => {
                    self.highlight_store.clear_group(buffer_id, group);
                    self.render();
                }
                HighlightEvent::ClearAll { buffer_id } => {
                    self.highlight_store.clear_all(buffer_id);
                    self.render();
                }
            },
            InnerEvent::RenderSignal => {
                self.render();
            }
            InnerEvent::CompletionEvent(comp_event) => {
                self.handle_completion_event(comp_event);
            }
            InnerEvent::KillSignal => {
                return true;
            }
            InnerEvent::WhichKeyShow { prefix, bindings } => {
                self.which_key_panel.show(prefix, bindings);
                self.render();
            }
            InnerEvent::WhichKeyHide => {
                self.which_key_panel.hide();
                self.render();
            }
            InnerEvent::OperatorMotionEvent(ref action) => {
                self.handle_operator_motion(action);
            }
            InnerEvent::TelescopeEvent(telescope_event) => {
                self.handle_telescope_event(telescope_event);
            }
            InnerEvent::LeapEvent(leap_event) => {
                self.handle_leap_event(leap_event);
            }
            InnerEvent::TreesitterEvent(ts_event) => {
                self.handle_treesitter_event(ts_event);
            }
            InnerEvent::VisualTextObjectEvent(ref action) => {
                self.handle_visual_text_object(action);
            }
            InnerEvent::ScreenResizeEvent { width, height } => {
                tracing::debug!("Screen resize: {}x{}", width, height);
                self.screen.resize(width, height);
                self.render();
            }
            InnerEvent::SettingsMenuEvent(ref settings_event) => {
                self.handle_settings_menu_event(settings_event);
            }
        }
        false
    }

    /// Handle treesitter-related events
    #[allow(clippy::cast_possible_truncation)]
    fn handle_treesitter_event(&mut self, event: TreesitterEvent) {
        match event {
            TreesitterEvent::ScheduleReparse { buffer_id } => {
                // Schedule a debounced reparse
                if self.treesitter.has_parser(buffer_id) {
                    self.treesitter.schedule_reparse(buffer_id);
                }
            }
            TreesitterEvent::IncrementalParse { buffer_id, edit } => {
                // Perform incremental parse with edit information
                if let Some(buffer) = self.buffers.get(&buffer_id) {
                    let content = buffer.content_to_string();
                    let line_count = buffer.contents.len() as u32;

                    // Clear old syntax highlights
                    self.highlight_store
                        .clear_group(buffer_id, HighlightGroup::Syntax);

                    // Generate new highlights via incremental parse
                    let highlights = self.treesitter.parse_incremental_and_highlight(
                        buffer_id,
                        &content,
                        &edit,
                        0,
                        line_count.saturating_sub(1),
                    );

                    if !highlights.is_empty() {
                        self.highlight_store.add(buffer_id, highlights);
                    }
                    self.render();
                }
            }
            TreesitterEvent::FullReparse { buffer_id } => {
                // Perform a full reparse
                if let Some(buffer) = self.buffers.get(&buffer_id) {
                    let content = buffer.content_to_string();
                    let line_count = buffer.contents.len() as u32;

                    // Clear old syntax highlights
                    self.highlight_store
                        .clear_group(buffer_id, HighlightGroup::Syntax);

                    // Generate new highlights via full parse
                    let highlights = self.treesitter.parse_and_highlight(
                        buffer_id,
                        &content,
                        0,
                        line_count.saturating_sub(1),
                    );

                    if !highlights.is_empty() {
                        self.highlight_store.add(buffer_id, highlights);
                    }
                    self.render();
                }
            }
        }
    }

    /// Handle telescope-related events
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cast_possible_truncation)]
    fn handle_telescope_event(&mut self, event: crate::event::TelescopeEvent) {
        use crate::event::TelescopeEvent;
        use crate::telescope::TelescopeData;
        use crate::telescope::picker::PickerContext;
        use crate::command::traits::ExecutionContext;

        match event {
            TelescopeEvent::Open { picker } => {
                tracing::debug!(?picker, "Telescope open requested");
                // Calculate layout based on screen size
                self.telescope_state.calculate_layout(
                    self.screen.width(),
                    self.screen.height(),
                );

                // Get title and prompt from picker registry
                let (title, prompt) = self
                    .telescope_pickers
                    .get(&picker)
                    .map_or(("Telescope", "> "), |p| (p.title(), p.prompt()));

                self.telescope_state.open(&picker, title, prompt);
                self.set_mode(ModeState::telescope());
                self.render();

                // Fetch items from picker asynchronously
                if let Some(picker_impl) = self.telescope_pickers.get(&picker).cloned() {
                    let tx = self.tx.clone();
                    let cwd = std::env::current_dir().unwrap_or_default();

                    tokio::spawn(async move {
                        let ctx = PickerContext {
                            query: String::new(),
                            cwd,
                            max_items: 1000,
                        };
                        let items = picker_impl.fetch(&ctx).await;
                        tracing::debug!(count = items.len(), "Fetched telescope items");
                        let _ = tx.send(InnerEvent::TelescopeEvent(
                            TelescopeEvent::UpdateItems { items }
                        )).await;
                    });
                }
            }
            TelescopeEvent::UpdateQuery { query } => {
                tracing::debug!(?query, "Telescope query updated");
                self.telescope_state.query.clone_from(&query);
                self.telescope_state.cursor_pos = self.telescope_state.query.len();

                // Filter items using nucleo matcher
                if query.is_empty() {
                    // Empty query - show all items
                    self.telescope_state.items = self.telescope_state.all_items.clone();
                } else if !self.telescope_state.all_items.is_empty() {
                    self.telescope_matcher.set_pattern(&query);
                    let filtered = self.telescope_matcher.match_items(
                        self.telescope_state.all_items.clone(),
                    );
                    self.telescope_state.update_filtered_items(filtered);
                }
                self.render_telescope_only();
            }
            TelescopeEvent::UpdateItems { items } => {
                tracing::debug!(count = items.len(), "Telescope items updated");
                self.telescope_state.update_items(items);
                self.render();
            }
            TelescopeEvent::SelectNext => {
                self.telescope_state.select_next();
                self.render_telescope_only();
            }
            TelescopeEvent::SelectPrev => {
                self.telescope_state.select_prev();
                self.render_telescope_only();
            }
            TelescopeEvent::PageDown => {
                self.telescope_state.page_down();
                self.render_telescope_only();
            }
            TelescopeEvent::PageUp => {
                self.telescope_state.page_up();
                self.render_telescope_only();
            }
            TelescopeEvent::Confirm => {
                // Get selected item before closing
                if let Some(item) = self.telescope_state.selected_item().cloned() {
                    tracing::debug!(?item.display, "Telescope selection confirmed");
                    // Handle action based on item data
                    match &item.data {
                        TelescopeData::FilePath(path) => {
                            let path_str = path.to_string_lossy().to_string();
                            self.telescope_state.close();
                            self.set_mode(ModeState::normal());
                            self.open_file(&path_str);
                        }
                        TelescopeData::BufferId(buf_id) => {
                            let buf_id = *buf_id;
                            self.telescope_state.close();
                            self.set_mode(ModeState::normal());
                            self.switch_buffer(buf_id);
                        }
                        TelescopeData::GrepMatch { path, line, col: _ } => {
                            let path_str = path.to_string_lossy().to_string();
                            let target_line = *line;
                            self.telescope_state.close();
                            self.set_mode(ModeState::normal());
                            self.open_file(&path_str);
                            // Move cursor to line
                            if let Some(buffer) = self.buffers.get_mut(&self.active_buffer_id) {
                                buffer.cur.y = target_line.saturating_sub(1) as u16;
                                buffer.cur.x = 0;
                            }
                        }
                        TelescopeData::Command(cmd_id) => {
                            let cmd_id = cmd_id.clone();
                            self.telescope_state.close();
                            self.set_mode(ModeState::normal());
                            // Execute the command
                            if let Some(cmd) = self.command_registry.get(&cmd_id)
                                && let Some(buffer) = self.buffers.get_mut(&self.active_buffer_id)
                            {
                                let mut ctx = ExecutionContext {
                                    buffer,
                                    count: Some(1),
                                    buffer_id: self.active_buffer_id,
                                    window_id: 0,
                                };
                                let _ = cmd.execute(&mut ctx);
                            }
                        }
                        TelescopeData::Keymap { .. } | TelescopeData::HelpTag { .. } => {
                            // Informational, just close
                            self.telescope_state.close();
                            self.set_mode(ModeState::normal());
                        }
                        TelescopeData::Theme(name) => {
                            let name = *name;
                            self.telescope_state.close();
                            self.set_mode(ModeState::normal());
                            // Apply the theme
                            self.theme = Theme::from_name(name);
                            self.treesitter.set_theme(TreesitterTheme::from_theme_name(name));
                            self.rehighlight_all_buffers();
                            tracing::info!(theme = ?name, "Theme applied via telescope");
                        }
                        TelescopeData::Profile(name) => {
                            let name = name.clone();
                            self.telescope_state.close();
                            self.set_mode(ModeState::normal());
                            // Load and apply the profile
                            self.load_profile(&name);
                        }
                    }
                } else {
                    self.telescope_state.close();
                    self.set_mode(ModeState::normal());
                }
                self.render();
            }
            TelescopeEvent::Close => {
                self.telescope_state.close();
                self.set_mode(ModeState::normal());
                self.render();
            }
            TelescopeEvent::UpdatePreview { content } => {
                self.telescope_state.set_preview(Some(content));
                self.render_telescope_only();
            }
        }
    }

    /// Handle completion-related events
    fn handle_completion_event(&mut self, event: CompletionEvent) {
        match event {
            CompletionEvent::Trigger { buffer_id } => {
                self.trigger_completion(buffer_id);
            }
            CompletionEvent::Update {
                items,
                prefix,
                start_col,
                start_row,
            } => {
                self.completion_items_cache.clone_from(&items);
                self.completion_state.activate(items, prefix, start_col, start_row);
                self.set_completion_active(true);
                self.render();
            }
            CompletionEvent::SelectNext => {
                self.completion_state.select_next();
                self.render();
            }
            CompletionEvent::SelectPrev => {
                self.completion_state.select_prev();
                self.render();
            }
            CompletionEvent::Confirm => {
                if let Some(item) = self.completion_state.selected_item().cloned() {
                    self.insert_completion(&item);
                }
                self.completion_state.dismiss();
                self.set_completion_active(false);
                self.render();
            }
            CompletionEvent::Dismiss => {
                self.completion_state.dismiss();
                self.set_completion_active(false);
                self.render();
            }
            CompletionEvent::UpdateFilter { new_prefix } => {
                self.completion_state
                    .update_prefix(&new_prefix, &self.completion_items_cache);
                self.render();
            }
        }
    }

    /// Handle mode change events
    #[allow(clippy::collapsible_if)]
    fn handle_mode_change(&mut self, new_mode: ModeState) {
        tracing::debug!(?new_mode, "Mode changed");

        // Hide which-key panel on mode change
        self.which_key_panel.hide();

        // Handle insert mode
        if new_mode.is_insert() {
            // Clear landing page content when entering insert mode (only once)
            if self.showing_landing_page {
                if let Some(buffer) = self.buffers.get_mut(&0) {
                    buffer.contents.clear();
                    buffer.cur.x = 0;
                    buffer.cur.y = 0;
                }
                self.showing_landing_page = false;
            }
        }

        // Handle visual mode
        if let EditMode::Visual(ext) = &new_mode.edit_mode {
            // Start selection when entering visual mode
            if let Some(buffer) = self.buffers.get_mut(&0) {
                match ext {
                    ModExtension::Block => buffer.start_block_selection(),
                    _ => buffer.start_selection(),
                }
            }
        }

        // Handle normal mode
        if new_mode.is_normal() && matches!(new_mode.sub_mode, SubMode::None) {
            // Clear selection when returning to normal mode
            if let Some(buffer) = self.buffers.get_mut(&0) {
                buffer.clear_selection();
            }
            // Note: command line is cleared in handle_command_line_command
            // after the command is executed, not here (to avoid race condition)
        }

        // Handle command mode
        if new_mode.is_command() {
            // Activate command line when entering command mode
            self.command_line.activate();
        }

        // Explorer, ExplorerInput, OperatorPending, Telescope modes are handled elsewhere:
        // - Explorer mode is handled via window focus
        // - ExplorerInput mode is for file operations and filter
        // - OperatorPending mode just waits for a motion key (handled by CommandHandler)
        // - Telescope mode is handled separately when opening a picker

        // Use set_mode to broadcast via watch channel
        self.set_mode(new_mode);
        self.render();
    }

    /// Handle leap-related events
    #[allow(clippy::cast_possible_truncation)]
    fn handle_leap_event(&mut self, event: crate::event::LeapEvent) {
        use crate::event::LeapEvent;
        use crate::leap::{find_matches, generate_labels};

        match event {
            LeapEvent::Start {
                direction,
                operator,
                count,
            } => {
                tracing::debug!(?direction, ?operator, ?count, "Leap mode started");
                self.leap_state.start(direction, operator, count);
                self.set_mode(ModeState::leap(direction, operator, count));
                self.render();
            }
            LeapEvent::FirstChar { char } => {
                tracing::debug!(?char, "Leap first char");
                self.leap_state.set_first_char(char);
                self.render();
            }
            LeapEvent::SecondChar { char } => {
                tracing::debug!(?char, "Leap second char");
                self.leap_state.set_second_char(char);

                // Find all matches in the visible buffer area
                if let Some(buffer) = self.buffers.get(&self.active_buffer_id) {
                    let pattern = self.leap_state.search_chars.clone();
                    let cursor_line = buffer.cur.y;
                    let cursor_col = buffer.cur.x;

                    // Use full buffer for now (TODO: limit to visible area)
                    let start_line = 0;
                    let end_line = buffer.contents.len() as u16;

                    // Build lines for pattern matching
                    let lines: Vec<&str> = buffer
                        .contents
                        .iter()
                        .map(|line| line.inner.as_str())
                        .collect();

                    let matches = find_matches(
                        &lines,
                        &pattern,
                        self.leap_state.direction,
                        cursor_line,
                        cursor_col,
                        start_line,
                        end_line,
                    );

                    if matches.is_empty() {
                        // No matches, cancel leap mode
                        self.leap_state.reset();
                        self.set_mode(ModeState::normal());
                    } else {
                        // Generate labels and assign to matches
                        let labels = generate_labels(matches.len());
                        self.leap_state.set_matches_with_labels(matches, labels);
                    }
                }
                self.render();
            }
            LeapEvent::SelectLabel { label } => {
                tracing::debug!(?label, "Leap label selected");

                if let Some(target) = self.leap_state.find_match_by_label(&label) {
                    let target_line = target.line;
                    let target_col = target.col;

                    // For now, just jump to the target (operator support TBD)
                    // TODO: Add operator support via OperatorMotionEvent
                    if let Some(buffer) = self.buffers.get_mut(&self.active_buffer_id) {
                        buffer.cur.y = target_line;
                        buffer.cur.x = target_col;
                    }
                }

                self.leap_state.reset();
                self.set_mode(ModeState::normal());
                self.render();
            }
            LeapEvent::Cancel => {
                tracing::debug!("Leap cancelled");
                self.leap_state.reset();
                self.set_mode(ModeState::normal());
                self.render();
            }
        }
    }

    /// Handle settings menu events
    #[allow(clippy::too_many_lines)]
    fn handle_settings_menu_event(&mut self, event: &SettingsMenuEvent) {
        match event {
            SettingsMenuEvent::Open => {
                // Use current profile or fall back to default
                let profile = self
                    .profile_manager
                    .current_profile()
                    .cloned()
                    .unwrap_or_default();
                self.settings_menu.open(&profile, &self.current_profile_name);
                let (w, h) = self.screen.size();
                self.settings_menu.calculate_layout(w, h);
                self.set_mode(ModeState::settings_menu());
                self.render();
            }
            SettingsMenuEvent::Close => {
                // If in text input mode, cancel input instead of closing menu
                if self.settings_menu.is_text_input_mode() {
                    self.settings_menu.cancel_text_input();
                } else {
                    self.settings_menu.close();
                    self.set_mode(ModeState::normal());
                }
                self.render();
            }
            SettingsMenuEvent::SelectNext => {
                self.settings_menu.select_next();
                self.render();
            }
            SettingsMenuEvent::SelectPrev => {
                self.settings_menu.select_prev();
                self.render();
            }
            SettingsMenuEvent::Toggle => {
                if self.settings_menu.toggle_selected() {
                    self.apply_settings_from_menu();
                }
                self.render();
            }
            SettingsMenuEvent::CycleNext => {
                if self.settings_menu.cycle_next_selected() {
                    self.apply_settings_from_menu();
                }
                self.render();
            }
            SettingsMenuEvent::CyclePrev => {
                if self.settings_menu.cycle_prev_selected() {
                    self.apply_settings_from_menu();
                }
                self.render();
            }
            SettingsMenuEvent::QuickSelect(n) => {
                if self.settings_menu.quick_select(*n) {
                    self.apply_settings_from_menu();
                }
                self.render();
            }
            SettingsMenuEvent::Increment => {
                if self.settings_menu.increment_selected() {
                    self.apply_settings_from_menu();
                }
                self.render();
            }
            SettingsMenuEvent::Decrement => {
                if self.settings_menu.decrement_selected() {
                    self.apply_settings_from_menu();
                }
                self.render();
            }
            SettingsMenuEvent::ExecuteAction => {
                // If in text input mode, Enter confirms the input
                if self.settings_menu.is_text_input_mode() {
                    let profile_name = self.settings_menu.get_input_value().to_string();
                    let action = self.settings_menu.take_pending_action();
                    self.settings_menu.cancel_text_input();

                    if action == Some(crate::settings_menu::ActionType::SaveProfile)
                        && !profile_name.is_empty()
                    {
                        self.save_settings_to_disk_with_name(&profile_name);
                        self.current_profile_name.clone_from(&profile_name);
                    }
                } else if let Some(action) = self.settings_menu.get_selected_action() {
                    match action {
                        crate::settings_menu::ActionType::SaveProfile => {
                            // Enter text input mode for profile name
                            self.settings_menu.enter_text_input(
                                crate::settings_menu::ActionType::SaveProfile,
                                "Profile name",
                                &self.current_profile_name,
                            );
                        }
                        crate::settings_menu::ActionType::LoadProfile => {
                            // Close settings menu and open telescope profile picker
                            self.settings_menu.close();
                            self.set_mode(ModeState::normal());
                            self.handle_profile_list();
                        }
                        crate::settings_menu::ActionType::ResetToDefault => {
                            // Reset to default profile
                            let default_profile = ProfileConfig::default();
                            self.settings_menu
                                .open(&default_profile, &self.current_profile_name);
                            let (w, h) = self.screen.size();
                            self.settings_menu.calculate_layout(w, h);
                            self.apply_settings_from_menu();
                        }
                    }
                }
                self.render();
            }
            SettingsMenuEvent::InputChar(c) => {
                self.settings_menu.input_char(*c);
                self.render();
            }
            SettingsMenuEvent::InputBackspace => {
                self.settings_menu.input_backspace();
                self.render();
            }
            SettingsMenuEvent::InputConfirm => {
                // Get the input value and pending action before canceling input mode
                let profile_name = self.settings_menu.get_input_value().to_string();
                let action = self.settings_menu.take_pending_action();
                self.settings_menu.cancel_text_input();

                if action == Some(crate::settings_menu::ActionType::SaveProfile)
                    && !profile_name.is_empty()
                {
                    self.save_settings_to_disk_with_name(&profile_name);
                    // Update current profile name
                    self.current_profile_name.clone_from(&profile_name);
                }
                self.render();
            }
            SettingsMenuEvent::InputCancel => {
                self.settings_menu.cancel_text_input();
                self.render();
            }
        }
    }

    /// Apply settings from the menu to the runtime (live preview, no disk save)
    fn apply_settings_from_menu(&mut self) {
        let profile = self.settings_menu.to_profile_config();

        // Apply theme
        if let Some(theme_name) = crate::highlight::ThemeName::parse(&profile.editor.theme) {
            self.theme = Theme::from_name(theme_name);
            self.treesitter
                .set_theme(TreesitterTheme::from_theme_name(theme_name));
            self.rehighlight_all_buffers();
        }

        // Apply color mode
        if let Some(mode) = crate::highlight::ColorMode::parse(&profile.editor.colormode) {
            self.color_mode = mode;
        }

        // Apply screen settings
        self.screen.set_number(profile.editor.number);
        self.screen.set_relative_number(profile.editor.relativenumber);
        self.screen.set_scrollbar(profile.editor.scrollbar);

        // Apply indent guide setting
        self.indent_analyzer.set_enabled(profile.editor.indentguide);

        // Store in profile manager (memory only, no disk save)
        self.profile_manager.set_current_profile(profile);
    }

    /// Save current settings to disk with a specific profile name
    fn save_settings_to_disk_with_name(&self, name: &str) {
        // Get current profile from settings menu state
        let profile = self.settings_menu.to_profile_config();

        match self.profile_manager.save_profile(name, &profile) {
            Ok(()) => {
                tracing::info!(profile = %name, "Settings saved");
            }
            Err(e) => {
                tracing::warn!(error = %e, profile = %name, "Failed to save settings");
            }
        }
    }
}
