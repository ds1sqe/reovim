//! Event handler dispatch methods for Runtime

use std::sync::Arc;

use crate::{
    bind::CommandRef,
    buffer::TextOps,
    command::{
        CommandTrait,
        traits::{
            BufferAction, CommandLineAction, CommandResult, DeferredAction, ExecutionContext,
            FileAction, OperatorMotionAction, TabAction, WindowAction,
        },
    },
    command_line::{ExCommand, SetOption},
    event::{CommandEvent, VisualTextObjectAction},
    event_bus::BufferModification,
    highlight::Theme,
    modd::{ComponentId, ModeState, SubMode},
    screen::{NavigateDirection, Position, SplitDirection},
    textobject::SemanticTextObjectSpec,
};

use super::Runtime;

impl Runtime {
    /// Schedule a treesitter reparse for the given buffer
    ///
    /// Emits a `BufferModified` event so the treesitter plugin can handle the reparse.
    pub(crate) fn schedule_treesitter_reparse(&self, buffer_id: usize) {
        self.event_bus.emit(crate::event_bus::BufferModified {
            buffer_id,
            modification: BufferModification::FullReplace,
        });
    }

    /// Delete a semantic text object using plugin state's text object source
    ///
    /// Returns the deleted text if successful, None if bounds couldn't be found.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn delete_semantic_text_object(
        &mut self,
        buffer_id: usize,
        spec: SemanticTextObjectSpec,
    ) -> Option<String> {
        // Get buffer content and cursor position
        let (content, cursor_row, cursor_col) = {
            let buffer = self.buffers.get(&buffer_id)?;
            (buffer.content_to_string(), u32::from(buffer.cur.y), u32::from(buffer.cur.x))
        };

        // Find text object bounds using plugin state
        let bounds = self
            .plugin_state
            .text_object_source()?
            .find_bounds(buffer_id, &content, cursor_row, cursor_col, &spec)?;

        // Convert to screen Position
        let start = Position {
            x: bounds.start.col as u16,
            y: bounds.start.row as u16,
        };
        let end = Position {
            x: bounds.end.col as u16,
            y: bounds.end.row as u16,
        };

        // Delete the range
        let buffer = self.buffers.get_mut(&buffer_id)?;
        Some(buffer.delete_range(start, end))
    }

    /// Yank a semantic text object using plugin state's text object source
    ///
    /// Returns the yanked text if successful, None if bounds couldn't be found.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn yank_semantic_text_object(
        &self,
        buffer_id: usize,
        spec: SemanticTextObjectSpec,
    ) -> Option<String> {
        // Get buffer content and cursor position
        let (content, cursor_row, cursor_col) = {
            let buffer = self.buffers.get(&buffer_id)?;
            (buffer.content_to_string(), u32::from(buffer.cur.y), u32::from(buffer.cur.x))
        };

        // Find text object bounds using plugin state
        let bounds = self
            .plugin_state
            .text_object_source()?
            .find_bounds(buffer_id, &content, cursor_row, cursor_col, &spec)?;

        // Convert to screen Position
        let start = Position {
            x: bounds.start.col as u16,
            y: bounds.start.row as u16,
        };
        let end = Position {
            x: bounds.end.col as u16,
            y: bounds.end.row as u16,
        };

        // Yank the range
        let buffer = self.buffers.get(&buffer_id)?;
        Some(buffer.yank_range(start, end))
    }

    /// Find the bounds of a semantic text object using plugin state's text object source
    ///
    /// Returns (start, end) positions if bounds found, None otherwise.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn find_semantic_text_object_bounds(
        &self,
        buffer_id: usize,
        spec: SemanticTextObjectSpec,
    ) -> Option<(Position, Position)> {
        // Get buffer content and cursor position
        let (content, cursor_row, cursor_col) = {
            let buffer = self.buffers.get(&buffer_id)?;
            (buffer.content_to_string(), u32::from(buffer.cur.y), u32::from(buffer.cur.x))
        };

        // Find text object bounds using plugin state
        let bounds = self
            .plugin_state
            .text_object_source()?
            .find_bounds(buffer_id, &content, cursor_row, cursor_col, &spec)?;

        // Convert to screen Position
        let start = Position {
            x: bounds.start.col as u16,
            y: bounds.start.row as u16,
        };
        let end = Position {
            x: bounds.end.col as u16,
            y: bounds.end.row as u16,
        };

        Some((start, end))
    }
}

impl Runtime {
    /// Handle command line actions. Returns true if editor should quit.
    #[allow(clippy::match_same_arms, clippy::too_many_lines)]
    pub(crate) fn handle_command_line_action(&mut self, action: &CommandLineAction) -> bool {
        match action {
            CommandLineAction::InsertChar(c) => {
                self.command_line.insert_char(*c);
            }
            CommandLineAction::Backspace => {
                self.command_line.delete_char();
            }
            CommandLineAction::Execute => {
                if let Some(ex_cmd) = self.command_line.execute() {
                    match ex_cmd {
                        ExCommand::Quit => {
                            self.command_line.clear();
                            return true;
                        }
                        ExCommand::Write { filename } => {
                            // Determine file path: use provided filename or buffer's file_path
                            let path = filename
                                .or_else(|| self.buffers.get(&0).and_then(|b| b.file_path.clone()));

                            if let Some(path) = path
                                && let Some(buffer) = self.buffers.get_mut(&0)
                            {
                                let content = buffer.content_to_string();
                                match std::fs::write(&path, &content) {
                                    Ok(()) => {
                                        tracing::info!(path = %path, bytes = content.len(), "File saved");
                                        buffer.file_path = Some(path);
                                        buffer.modified = false;
                                    }
                                    Err(e) => {
                                        tracing::error!(path = %path, error = %e, "Failed to write file");
                                    }
                                }
                            }
                        }
                        ExCommand::WriteQuit => {
                            // Write file then quit
                            let path = self.buffers.get(&0).and_then(|b| b.file_path.clone());
                            if let Some(path) = path
                                && let Some(buffer) = self.buffers.get_mut(&0)
                            {
                                let content = buffer.content_to_string();
                                match std::fs::write(&path, &content) {
                                    Ok(()) => {
                                        tracing::info!(path = %path, bytes = content.len(), "File saved before quit");
                                        buffer.modified = false;
                                    }
                                    Err(e) => {
                                        tracing::error!(path = %path, error = %e, "Failed to write file before quit");
                                    }
                                }
                            }
                            self.command_line.clear();
                            return true;
                        }
                        ExCommand::Set { option } => match option {
                            SetOption::Number(enabled) => {
                                self.screen.set_number(enabled);
                            }
                            SetOption::RelativeNumber(enabled) => {
                                self.screen.set_relative_number(enabled);
                            }
                            SetOption::ColorMode(mode) => {
                                self.set_color_mode(mode);
                            }
                            SetOption::ColorScheme(name) => {
                                self.theme = Theme::from_name(name);
                                // Theme change triggers rehighlighting via event bus
                                self.rehighlight_all_buffers();
                                tracing::info!(theme = ?name, "Colorscheme changed");
                            }
                            SetOption::IndentGuide(enabled) => {
                                self.indent_analyzer.set_enabled(enabled);
                                tracing::info!(enabled, "Indent guides toggled");
                            }
                            SetOption::Scrollbar(enabled) => {
                                self.screen.set_scrollbar(enabled);
                                tracing::info!(enabled, "Scrollbar toggled");
                            }
                            // Dynamic options from OptionRegistry
                            SetOption::DynamicEnable(name) => {
                                self.handle_dynamic_option_enable(&name);
                            }
                            SetOption::DynamicDisable(name) => {
                                self.handle_dynamic_option_disable(&name);
                            }
                            SetOption::DynamicSet { name, value } => {
                                self.handle_dynamic_option_set(&name, &value);
                            }
                            SetOption::DynamicQuery(name) => {
                                self.handle_dynamic_option_query(&name);
                            }
                            SetOption::DynamicReset(name) => {
                                self.handle_dynamic_option_reset(&name);
                            }
                        },
                        ExCommand::Colorscheme { name } => {
                            self.theme = Theme::from_name(name);
                            // Theme change triggers rehighlighting via event bus
                            self.rehighlight_all_buffers();
                            tracing::info!(theme = ?name, "Colorscheme changed");
                        }
                        ExCommand::Edit { filename } => {
                            self.open_file(&filename);
                            self.screen.set_editor_buffer(self.active_buffer_id);
                        }
                        // Window management
                        ExCommand::Split { filename } => {
                            self.handle_window_split(false, filename.as_ref());
                        }
                        ExCommand::VSplit { filename } => {
                            self.handle_window_split(true, filename.as_ref());
                        }
                        ExCommand::Close => {
                            if self.handle_window_close(false) {
                                return true;
                            }
                        }
                        ExCommand::Only => {
                            self.handle_window_only();
                        }
                        // Tab management
                        ExCommand::TabNew { filename } => {
                            self.handle_tab_new(filename.as_ref());
                        }
                        ExCommand::TabClose => {
                            if self.handle_tab_close() {
                                return true;
                            }
                        }
                        ExCommand::TabNext => {
                            self.handle_tab_next();
                        }
                        ExCommand::TabPrev => {
                            self.handle_tab_prev();
                        }
                        // Profile management
                        ExCommand::ProfileLoad { name } => {
                            if self.load_profile(&name) {
                                tracing::info!(profile = %name, "Profile loaded");
                            }
                        }
                        ExCommand::ProfileSave { name } => {
                            if self.save_current_as_profile(&name) {
                                tracing::info!(profile = %name, "Profile saved");
                            }
                        }
                        ExCommand::ProfileList => {
                            // Telescope handled by plugin via EventBus
                            tracing::debug!("Profile list command - handled by telescope plugin");
                        }
                        ExCommand::Settings => {
                            // Settings menu handled by plugin via EventBus
                            tracing::debug!("Settings command - handled by settings-menu plugin");
                        }
                        ExCommand::Unknown(cmd) => {
                            tracing::warn!(command = %cmd, "Unknown ex-command");
                        }
                    }
                }
                // Clear command line after execution (render handled by handle_command)
                self.command_line.clear();
            }
            CommandLineAction::Cancel => {
                // Just clear the command line, mode change handled by CommandHandler
                self.command_line.clear();
            }
        }
        false
    }

    /// Resolve a `CommandRef` to an executable command trait object
    fn resolve_command(&self, cmd_ref: &CommandRef) -> Option<Arc<dyn CommandTrait>> {
        match cmd_ref {
            CommandRef::Registered(id) => self.command_registry.get(id),
            CommandRef::Inline(cmd) => Some(cmd.clone()),
        }
    }

    /// Handle a command event. Returns true if the editor should quit.
    #[allow(clippy::match_same_arms)]
    #[allow(clippy::too_many_lines)]
    pub(crate) fn handle_command(&mut self, cmd_event: CommandEvent) -> bool {
        let start = std::time::Instant::now();
        let CommandEvent { command, context } = cmd_event;

        // Resolve the command reference to a trait object
        let Some(cmd) = self.resolve_command(&command) else {
            // Command not found in registry
            tracing::warn!("Command not found in registry: {:?}", command);
            return false;
        };

        tracing::debug!("[CMD] handle_command START: name={} at {:?}", cmd.name(), start.elapsed());

        // Use active_buffer_id instead of context.buffer_id since the dispatcher
        // doesn't track buffer changes. In a single-window editor, active_buffer_id
        // is the correct buffer to operate on.
        let buffer_id = self.active_buffer_id;

        // Get the buffer and execute the command
        if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
            // Record position BEFORE executing jump commands
            if cmd.is_jump() {
                self.jump_list.push(buffer_id, buffer.cur);
            }

            let mut exec_ctx = ExecutionContext {
                buffer,
                count: context.count,
                buffer_id,
                window_id: context.window_id,
            };

            let result = cmd.execute(&mut exec_ctx);
            tracing::debug!(
                "[CMD] executed: name={} result={:?} at {:?}",
                cmd.name(),
                std::mem::discriminant(&result),
                start.elapsed()
            );

            // Check if command is text-modifying for treesitter reparse
            let is_text_modifying = cmd.is_text_modifying();

            match result {
                CommandResult::NeedsRender => {
                    // Schedule treesitter reparse if this command modifies text
                    if is_text_modifying {
                        self.schedule_treesitter_reparse(buffer_id);
                    }
                    self.request_render();
                }
                CommandResult::ModeChange(new_mode) => {
                    // Actually change the mode for commands like o, O
                    // Commands that change mode to Insert often modify buffer (o, O, etc.)
                    if is_text_modifying {
                        self.schedule_treesitter_reparse(buffer_id);
                    }
                    self.set_mode(new_mode);
                    self.request_render();
                }
                CommandResult::Quit => {
                    return true;
                }
                CommandResult::ClipboardWrite {
                    text,
                    register,
                    mode_change,
                } => {
                    // Trigger yank animation for visual mode selections
                    // Must capture selection bounds BEFORE mode change clears the selection
                    if let Some(buf) = self.buffers.get(&buffer_id)
                        && buf.selection.active
                    {
                        use crate::buffer::SelectionOps;
                        let (start, end) = buf.selection_bounds();
                        self.trigger_yank_range_animation(buffer_id, start, end);
                    }

                    self.registers.set_by_name(register, text);
                    if let Some(new_mode) = mode_change {
                        self.handle_mode_change(new_mode);
                    }
                    self.request_render();
                }
                CommandResult::DeferToRuntime(action) => {
                    match action {
                        DeferredAction::Paste {
                            before: _before,
                            register,
                        } => {
                            // Handle paste from specified register
                            // TODO: implement proper PasteBefore (paste at cursor vs before cursor)
                            // Use active_buffer_id, not context.buffer_id (which is hardcoded to 0 in dispatcher)
                            let paste_buffer_id = self.active_buffer_id;
                            if let Some(text) = self.registers.get_by_name(register)
                                && let Some(buf) = self.buffers.get_mut(&paste_buffer_id)
                            {
                                buf.insert_text(&text);
                            }
                            // Schedule treesitter reparse after paste
                            self.schedule_treesitter_reparse(paste_buffer_id);
                            self.request_render();
                        }
                        DeferredAction::CommandLine(cl_action) => {
                            let should_quit = self.handle_command_line_action(&cl_action);
                            self.request_render();
                            return should_quit;
                        }
                        DeferredAction::JumpOlder => {
                            if let Some(entry) = self.jump_list.jump_older() {
                                let target_pos = entry.position;
                                let target_buf_id = entry.buffer_id;
                                if let Some(buf) = self.buffers.get_mut(&target_buf_id) {
                                    buf.cur = target_pos;
                                }
                            }
                            self.request_render();
                        }
                        DeferredAction::JumpNewer => {
                            if let Some(entry) = self.jump_list.jump_newer() {
                                let target_pos = entry.position;
                                let target_buf_id = entry.buffer_id;
                                if let Some(buf) = self.buffers.get_mut(&target_buf_id) {
                                    buf.cur = target_pos;
                                }
                            }
                            self.request_render();
                        }
                        DeferredAction::OperatorMotion(ref op_action) => {
                            self.handle_operator_motion(op_action);
                        }
                        DeferredAction::Window(ref action) => {
                            if self.handle_window_action(action) {
                                return true;
                            }
                            self.sync_mode_with_screen_focus();
                        }
                        DeferredAction::Tab(ref action) => {
                            if self.handle_tab_action(action) {
                                return true;
                            }
                        }
                        DeferredAction::Buffer(ref action) => {
                            self.handle_buffer_action(action);
                        }
                        DeferredAction::File(ref action) => {
                            self.handle_file_action(action);
                        }
                    }
                }
                CommandResult::EmitEvent(event) => {
                    tracing::info!("Command result: EmitEvent, type={}", event.type_name());
                    // Dispatch the event to the event bus for plugin handling
                    let sender = self.event_bus.sender();
                    let mut ctx = crate::event_bus::HandlerContext::new(&sender);
                    let result = self.event_bus.dispatch(&event, &mut ctx);
                    tracing::info!(
                        "Event dispatched: type={}, result={:?}",
                        event.type_name(),
                        result
                    );

                    // Check if any handler requested render or quit
                    if ctx.render_requested() {
                        self.request_render();
                    }
                    if ctx.quit_requested() {
                        // Emit quit - the event loop will handle it
                        let tx = self.tx.clone();
                        tokio::spawn(async move {
                            let _ = tx.send(crate::event::InnerEvent::KillSignal).await;
                        });
                    }

                    // Events should be handled by plugin EventBus subscriptions
                    // If not handled, log for debugging during migration
                    if matches!(result, crate::event_bus::EventResult::NotHandled) {
                        tracing::trace!("Unhandled event: {:?}", event.type_name());
                    }
                }
                CommandResult::Error(msg) => {
                    tracing::warn!(error = %msg, "Command execution failed");
                }
                CommandResult::Deferred(handler) => {
                    // Trait-based deferred action handler
                    // The handler receives RuntimeContext and can perform any runtime operation
                    handler.handle(self);
                    self.request_render();
                }
                CommandResult::Success => {}
            }
            tracing::debug!("[CMD] handle_command DONE at {:?}", start.elapsed());
        }
        false
    }

    /// Handle operator + motion action (d/y/c + motion)
    #[allow(clippy::too_many_lines)]
    pub(crate) fn handle_operator_motion(&mut self, action: &OperatorMotionAction) {
        let buffer_id = self.active_buffer_id;
        let mut text_modified = false;

        if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
            match *action {
                OperatorMotionAction::Delete { motion, count } => {
                    let deleted = buffer.delete_to_motion(motion, count);
                    if !deleted.is_empty() {
                        // Store in unnamed register
                        self.registers.set(deleted);
                        text_modified = true;
                    }
                }
                OperatorMotionAction::Yank { motion, count } => {
                    // Calculate range before yanking to trigger visual feedback
                    let start_pos = buffer.cur;
                    let yanked = buffer.yank_to_motion(motion, count);
                    if !yanked.is_empty() {
                        // Get line count before moving yanked
                        let line_count = yanked.lines().count();

                        // Trigger yank blink animation
                        self.trigger_yank_animation(
                            buffer_id, start_pos, motion, count, line_count,
                        );

                        // Store in unnamed register
                        self.registers.set(yanked);
                    }
                    // Yank doesn't modify text
                }
                OperatorMotionAction::Change { motion, count } => {
                    let deleted = buffer.delete_to_motion(motion, count);
                    if !deleted.is_empty() {
                        // Store in unnamed register
                        self.registers.set(deleted);
                        text_modified = true;
                    }
                    // Enter insert mode after change
                    self.set_mode(ModeState::insert());
                }
                OperatorMotionAction::DeleteTextObject { text_object } => {
                    let deleted = buffer.delete_text_object(text_object);
                    if !deleted.is_empty() {
                        self.registers.set(deleted);
                        text_modified = true;
                    }
                }
                OperatorMotionAction::YankTextObject { text_object } => {
                    // Get range for visual feedback
                    let range = buffer.find_text_object_bounds(text_object);
                    let yanked = buffer.yank_text_object(text_object);
                    if !yanked.is_empty() {
                        // Trigger yank blink animation on text object range
                        if let Some((start, end)) = range {
                            self.trigger_yank_range_animation(buffer_id, start, end);
                        }

                        // Store in register after animation trigger
                        self.registers.set(yanked);
                    }
                    // Yank doesn't modify text
                }
                OperatorMotionAction::ChangeTextObject { text_object } => {
                    let deleted = buffer.delete_text_object(text_object);
                    if !deleted.is_empty() {
                        self.registers.set(deleted);
                        text_modified = true;
                    }
                    // Enter insert mode after change
                    self.set_mode(ModeState::insert());
                }
                OperatorMotionAction::DeleteSemanticTextObject { text_object } => {
                    if let Some(deleted) = self.delete_semantic_text_object(buffer_id, text_object)
                        && !deleted.is_empty()
                    {
                        self.registers.set(deleted);
                        text_modified = true;
                    }
                }
                OperatorMotionAction::YankSemanticTextObject { text_object } => {
                    if let Some(yanked) = self.yank_semantic_text_object(buffer_id, text_object)
                        && !yanked.is_empty()
                    {
                        self.registers.set(yanked);
                    }
                }
                OperatorMotionAction::ChangeSemanticTextObject { text_object } => {
                    if let Some(deleted) = self.delete_semantic_text_object(buffer_id, text_object)
                        && !deleted.is_empty()
                    {
                        self.registers.set(deleted);
                        text_modified = true;
                    }
                    // Enter insert mode after change
                    self.set_mode(ModeState::insert());
                }
                OperatorMotionAction::DeleteWordTextObject { text_object } => {
                    let deleted = buffer.delete_word_text_object(text_object);
                    if !deleted.is_empty() {
                        self.registers.set(deleted);
                        text_modified = true;
                    }
                }
                OperatorMotionAction::YankWordTextObject { text_object } => {
                    let yanked = buffer.yank_word_text_object(text_object);
                    if !yanked.is_empty() {
                        self.registers.set(yanked);
                    }
                    // Yank doesn't modify text
                }
                OperatorMotionAction::ChangeWordTextObject { text_object } => {
                    let deleted = buffer.delete_word_text_object(text_object);
                    if !deleted.is_empty() {
                        self.registers.set(deleted);
                        text_modified = true;
                    }
                    // Enter insert mode after change
                    self.set_mode(ModeState::insert());
                }
                OperatorMotionAction::ChangeLine => {
                    // Get current line content
                    let y = buffer.cur.y as usize;
                    if let Some(line) = buffer.contents.get_mut(y) {
                        // Store line content in register
                        if !line.inner.is_empty() {
                            self.registers.set(line.inner.clone());
                            // Clear the line content
                            line.inner.clear();
                            text_modified = true;
                        }
                    }
                    // Move cursor to start of line
                    buffer.cur.x = 0;
                    // Enter insert mode after change
                    self.set_mode(ModeState::insert());
                }
            }
        }

        // Schedule treesitter reparse if text was modified
        if text_modified {
            self.schedule_treesitter_reparse(buffer_id);
        }
        self.request_render();
    }

    /// Handle visual mode text object selection (viw, vi(, vif, etc.)
    pub(crate) fn handle_visual_text_object(&mut self, action: &VisualTextObjectAction) {
        // Get the primary buffer (buffer 0 for now)
        let buffer_id = 0;

        if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
            match *action {
                VisualTextObjectAction::SelectDelimiter { text_object } => {
                    // Find text object bounds using existing buffer method
                    if let Some((start, end)) = buffer.find_text_object_bounds(text_object) {
                        // Set selection: anchor at start, cursor at end
                        buffer.selection.anchor = start;
                        buffer.selection.active = true;
                        buffer.cur = end;
                    }
                }
                VisualTextObjectAction::SelectWord { text_object } => {
                    // Find word text object bounds
                    if let Some((start, end)) = buffer.find_word_text_object_bounds(text_object) {
                        buffer.selection.anchor = start;
                        buffer.selection.active = true;
                        buffer.cur = end;
                    }
                }
                VisualTextObjectAction::SelectSemantic { text_object } => {
                    // Find semantic text object bounds using treesitter
                    if let Some((start, end)) =
                        self.find_semantic_text_object_bounds(buffer_id, text_object)
                        && let Some(buf) = self.buffers.get_mut(&buffer_id)
                    {
                        buf.selection.anchor = start;
                        buf.selection.active = true;
                        buf.cur = end;
                    }
                }
            }
        }
        self.request_render();
    }

    // === Window Management Handlers ===

    /// Synchronize mode state with screen focus
    ///
    /// The screen is the source of truth for focus (plugin vs editor).
    /// This method updates the mode state to match.
    pub(crate) fn sync_mode_with_screen_focus(&mut self) {
        use crate::modd::ComponentId;

        // Check if any plugin window has focus
        let focused_plugin = self.screen.focused_plugin();
        let mode_interactor = self.mode_state.interactor_id;

        match (focused_plugin, mode_interactor) {
            // Screen has plugin focus, but mode doesn't match
            (Some(plugin_id), interactor_id) if interactor_id != plugin_id => {
                self.set_mode(ModeState::with_interactor_id_and_mode(
                    plugin_id,
                    crate::modd::EditMode::Normal,
                ));
            }
            // Screen has no plugin focus, but mode has non-editor focus
            (None, interactor_id) if interactor_id != ComponentId::EDITOR => {
                self.set_mode(ModeState::normal());
            }
            // Already in sync
            _ => {}
        }
    }

    /// Handle window-related deferred actions. Returns true if editor should quit.
    pub(crate) fn handle_window_action(&mut self, action: &WindowAction) -> bool {
        match action {
            WindowAction::SplitHorizontal { filename } => {
                self.handle_window_split(false, filename.as_ref());
            }
            WindowAction::SplitVertical { filename } => {
                self.handle_window_split(true, filename.as_ref());
            }
            WindowAction::Close { force } => {
                if self.handle_window_close(*force) {
                    return true;
                }
            }
            WindowAction::CloseOthers => {
                self.handle_window_only();
            }
            WindowAction::FocusDirection { direction } => {
                self.handle_window_navigate(*direction);
            }
            WindowAction::MoveDirection { direction } => {
                // Window movement (swap windows) - not yet implemented
                tracing::info!(direction = ?direction, "Window move direction (not yet implemented)");
            }
            WindowAction::Resize { direction, delta } => {
                tracing::info!(direction = ?direction, delta = %delta, "Window resize (not yet implemented)");
            }
            WindowAction::Equalize => {
                self.handle_window_equalize();
            }
            WindowAction::SwapDirection { direction } => {
                self.screen.swap_window(*direction);
            }
        }

        // Exit window mode if we're in it
        self.exit_window_mode_if_active();

        false
    }

    /// Exit window mode if currently active
    fn exit_window_mode_if_active(&mut self) {
        if matches!(
            self.mode_state.sub_mode,
            SubMode::Interactor(id) if id == ComponentId::WINDOW
        ) {
            self.handle_mode_change(ModeState::normal());
        }
    }

    /// Handle tab-related deferred actions. Returns true if editor should quit.
    pub(crate) fn handle_tab_action(&mut self, action: &TabAction) -> bool {
        match action {
            TabAction::New { filename } => {
                self.handle_tab_new(filename.as_ref());
            }
            TabAction::Close => {
                if self.handle_tab_close() {
                    return true;
                }
            }
            TabAction::Next => {
                self.handle_tab_next();
            }
            TabAction::Prev => {
                self.handle_tab_prev();
            }
            TabAction::Goto { index } => {
                self.screen.goto_tab(*index);
            }
        }
        false
    }

    /// Handle window split command
    ///
    /// Saves the current window's cursor before splitting so the new window
    /// inherits the correct cursor position. See docs/window-buffer.md.
    pub(crate) fn handle_window_split(&mut self, vertical: bool, filename: Option<&String>) {
        let start = std::time::Instant::now();
        tracing::debug!("[SPLIT] handle_window_split START vertical={}", vertical);

        let direction = if vertical {
            SplitDirection::Vertical
        } else {
            SplitDirection::Horizontal
        };

        // If a filename is provided, open it
        if let Some(path) = filename {
            self.open_file(path);
        }

        // Get the current buffer ID (either the newly opened file or existing buffer)
        // Use self.active_buffer_id directly since open_file updates it
        let buffer_id = self.active_buffer_id;

        // CRITICAL: Save the buffer's live cursor to the current window BEFORE splitting.
        // split_window() reads window.cursor to copy to the new window, so we must
        // ensure it reflects the current buffer cursor, not a stale saved value.
        if let Some(window) = self.screen.active_window_mut()
            && let Some(buffer) = self.buffers.get(&buffer_id)
        {
            tracing::debug!(
                "[SPLIT] SAVE cursor: win={} buffer.cur=({},{}) -> window.cursor at {:?}",
                window.id,
                buffer.cur.x,
                buffer.cur.y,
                start.elapsed()
            );
            window.cursor = buffer.cur;
            window.desired_col = buffer.desired_col;
        } else {
            tracing::warn!("[SPLIT] failed to get active window or buffer!");
        }

        // Split the window
        if let Some(new_window_id) = self.screen.split_window(direction) {
            // Set the buffer for the new window
            self.screen.set_window_buffer(new_window_id, buffer_id);
            tracing::debug!(
                "[SPLIT] created new_win={} buffer={} at {:?}",
                new_window_id,
                buffer_id,
                start.elapsed()
            );
        }
        tracing::debug!("[SPLIT] handle_window_split DONE at {:?}", start.elapsed());
    }

    /// Handle window close command. Returns true if editor should quit.
    pub(crate) fn handle_window_close(&mut self, _force: bool) -> bool {
        // TODO: Check for unsaved changes if force is false
        self.screen.close_window()
    }

    /// Handle close all other windows (:only)
    pub(crate) fn handle_window_only(&mut self) {
        self.screen.close_other_windows();
    }

    /// Handle window navigation (focus direction)
    pub(crate) fn handle_window_navigate(&mut self, direction: NavigateDirection) {
        let start = std::time::Instant::now();
        let before_window_id = self.screen.active_window_id();

        // Before navigation: save current buffer cursor to current window
        if let Some(window) = self.screen.active_window_mut()
            && let Some(buffer_id) = window.buffer_id()
            && let Some(buffer) = self.buffers.get(&buffer_id)
        {
            tracing::debug!(
                "[NAV] SAVE cursor: win={} buffer.cur=({},{}) -> window.cursor at {:?}",
                window.id,
                buffer.cur.x,
                buffer.cur.y,
                start.elapsed()
            );
            window.cursor = buffer.cur;
            window.desired_col = buffer.desired_col;
        }

        // Navigate to new window
        self.screen.navigate_window(direction);

        let after_window_id = self.screen.active_window_id();
        tracing::debug!(
            "[NAV] navigate {:?}: win {} -> win {:?} at {:?}",
            direction,
            before_window_id.unwrap_or(999),
            after_window_id,
            start.elapsed()
        );

        // After navigation: load new window's cursor into buffer and update active_buffer_id
        if let Some(window) = self.screen.active_window()
            && let Some(new_buffer_id) = window.buffer_id()
        {
            let window_cursor = window.cursor;
            let window_desired_col = window.desired_col;

            tracing::debug!(
                "[NAV] LOAD cursor: win={} window.cursor=({},{}) -> buffer.cur at {:?}",
                window.id,
                window_cursor.x,
                window_cursor.y,
                start.elapsed()
            );

            // Update active_buffer_id to match the new window's buffer
            self.active_buffer_id = new_buffer_id;

            // Load window's cursor into buffer
            if let Some(buffer) = self.buffers.get_mut(&new_buffer_id) {
                buffer.cur = window_cursor;
                buffer.desired_col = window_desired_col;
            }
        }
        tracing::debug!("[NAV] handle_window_navigate DONE at {:?}", start.elapsed());
    }

    /// Handle window equalize
    pub(crate) fn handle_window_equalize(&mut self) {
        self.screen.equalize_windows();
    }

    // === Tab Management Handlers ===

    /// Handle new tab command
    pub(crate) fn handle_tab_new(&mut self, filename: Option<&String>) {
        // If a filename is provided, open it
        if let Some(path) = filename {
            self.open_file(path);
        }

        // Get the current buffer ID
        // Use self.active_buffer_id directly since open_file updates it
        let buffer_id = self.active_buffer_id;

        let tab_id = self.screen.new_tab(buffer_id);
        tracing::info!(tab_id = tab_id, buffer_id = buffer_id, "New tab created");
    }

    /// Handle tab close command. Returns true if editor should quit.
    pub(crate) fn handle_tab_close(&mut self) -> bool {
        self.screen.close_tab()
    }

    /// Handle next tab command (gt)
    pub(crate) fn handle_tab_next(&mut self) {
        self.screen.next_tab();
    }

    /// Handle previous tab command (gT)
    pub(crate) fn handle_tab_prev(&mut self) {
        self.screen.prev_tab();
    }

    // === Buffer Navigation Handlers ===

    /// Handle buffer-related deferred actions
    pub(crate) fn handle_buffer_action(&mut self, action: &BufferAction) {
        match action {
            BufferAction::Prev => {
                if let Some(prev_id) = self.prev_buffer_id() {
                    self.switch_buffer(prev_id);
                    // Update window's buffer
                    if let Some(window_id) = self.screen.active_window_id() {
                        self.screen.set_window_buffer(window_id, prev_id);
                    }
                }
            }
            BufferAction::Next => {
                if let Some(next_id) = self.next_buffer_id() {
                    self.switch_buffer(next_id);
                    if let Some(window_id) = self.screen.active_window_id() {
                        self.screen.set_window_buffer(window_id, next_id);
                    }
                }
            }
            BufferAction::Delete { force: _ } => {
                // TODO: Check modified state if !force
                let buffer_id = self.active_buffer_id;
                self.close_buffer(buffer_id);
                // Update window to show the new active buffer
                if let Some(window_id) = self.screen.active_window_id() {
                    self.screen
                        .set_window_buffer(window_id, self.active_buffer_id);
                }
            }
        }
        self.request_render();
    }

    // ========================================================================
    // File Operations
    // ========================================================================

    /// Handle file operations from plugins
    ///
    /// This provides a way for plugins to request file operations that need
    /// Runtime access (e.g., opening files into buffers, creating files).
    pub(crate) fn handle_file_action(&mut self, action: &FileAction) {
        match action {
            FileAction::Open { path } => {
                self.open_file(path);
                self.screen.set_editor_buffer(self.active_buffer_id);
                // Switch focus to editor if a plugin has focus
                if self.screen.has_plugin_focus() {
                    self.screen.focus_editor();
                    self.set_mode(ModeState::normal());
                }
            }
            FileAction::Create { path, is_dir } => {
                let result = if *is_dir {
                    std::fs::create_dir_all(path)
                } else {
                    // Create parent directories if needed, then create file
                    if let Some(parent) = std::path::Path::new(path).parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    std::fs::File::create(path).map(|_| ())
                };
                match result {
                    Ok(()) => {
                        tracing::info!(path = %path, is_dir = is_dir, "File/directory created");
                    }
                    Err(e) => {
                        tracing::error!(path = %path, error = %e, "Failed to create file/directory");
                    }
                }
            }
            FileAction::Delete { path } => {
                let path = std::path::Path::new(path);
                let result = if path.is_dir() {
                    std::fs::remove_dir_all(path)
                } else {
                    std::fs::remove_file(path)
                };
                match result {
                    Ok(()) => {
                        tracing::info!(path = %path.display(), "File/directory deleted");
                    }
                    Err(e) => {
                        tracing::error!(path = %path.display(), error = %e, "Failed to delete");
                    }
                }
            }
            FileAction::Rename { from, to } => match std::fs::rename(from, to) {
                Ok(()) => {
                    tracing::info!(from = %from, to = %to, "File/directory renamed");
                }
                Err(e) => {
                    tracing::error!(from = %from, to = %to, error = %e, "Failed to rename");
                }
            },
        }
        self.request_render();
    }

    // === Dynamic Option Handlers ===

    /// Handle dynamic option enable (`:set optionname`)
    fn handle_dynamic_option_enable(&self, name: &str) {
        use crate::option::{ChangeSource, OptionChanged, OptionValue};

        if let Some(old_value) = self.option_registry.get(name) {
            let new_value = OptionValue::Bool(true);
            match self.option_registry.set(name, new_value.clone()) {
                Ok(_) => {
                    tracing::info!(option = %name, "Option enabled");
                    // Emit change event
                    self.event_bus.emit(OptionChanged::new(
                        name,
                        old_value,
                        new_value,
                        ChangeSource::UserCommand,
                    ));
                }
                Err(e) => {
                    tracing::warn!(option = %name, error = %e, "Failed to enable option");
                }
            }
        } else {
            tracing::warn!(option = %name, "E518: Unknown option");
        }
    }

    /// Handle dynamic option disable (`:set nooptionname`)
    fn handle_dynamic_option_disable(&self, name: &str) {
        use crate::option::{ChangeSource, OptionChanged, OptionValue};

        if let Some(old_value) = self.option_registry.get(name) {
            let new_value = OptionValue::Bool(false);
            match self.option_registry.set(name, new_value.clone()) {
                Ok(_) => {
                    tracing::info!(option = %name, "Option disabled");
                    // Emit change event
                    self.event_bus.emit(OptionChanged::new(
                        name,
                        old_value,
                        new_value,
                        ChangeSource::UserCommand,
                    ));
                }
                Err(e) => {
                    tracing::warn!(option = %name, error = %e, "Failed to disable option");
                }
            }
        } else {
            tracing::warn!(option = %name, "E518: Unknown option");
        }
    }

    /// Handle dynamic option set (`:set optionname=value`)
    fn handle_dynamic_option_set(&self, name: &str, value_str: &str) {
        use crate::option::{ChangeSource, OptionChanged, OptionValue};

        // Get the spec to determine the expected type
        let Some(spec) = self.option_registry.get_spec(name) else {
            tracing::warn!(option = %name, "E518: Unknown option");
            return;
        };

        // Parse value based on expected type
        let new_value = match &spec.default {
            OptionValue::Bool(_) => {
                // For booleans, accept "true"/"false", "1"/"0", "yes"/"no"
                match value_str.to_lowercase().as_str() {
                    "true" | "1" | "yes" | "on" => OptionValue::Bool(true),
                    "false" | "0" | "no" | "off" => OptionValue::Bool(false),
                    _ => {
                        tracing::warn!(option = %name, value = %value_str, "E474: Invalid boolean argument");
                        return;
                    }
                }
            }
            OptionValue::Integer(_) => {
                if let Ok(i) = value_str.parse::<i64>() {
                    OptionValue::Integer(i)
                } else {
                    tracing::warn!(option = %name, value = %value_str, "E521: Number required");
                    return;
                }
            }
            OptionValue::String(_) => OptionValue::String(value_str.to_string()),
            OptionValue::Choice { choices, .. } => {
                if choices.contains(&value_str.to_string()) {
                    OptionValue::Choice {
                        value: value_str.to_string(),
                        choices: choices.clone(),
                    }
                } else {
                    tracing::warn!(
                        option = %name,
                        value = %value_str,
                        valid = %choices.join(", "),
                        "E474: Invalid choice"
                    );
                    return;
                }
            }
        };

        // Get old value for change event
        let old_value = self
            .option_registry
            .get(name)
            .unwrap_or_else(|| spec.default.clone());

        // Set the value
        match self.option_registry.set(name, new_value.clone()) {
            Ok(_) => {
                tracing::info!(option = %name, value = %value_str, "Option set");
                // Emit change event
                self.event_bus.emit(OptionChanged::new(
                    name,
                    old_value,
                    new_value,
                    ChangeSource::UserCommand,
                ));
            }
            Err(e) => {
                tracing::warn!(option = %name, value = %value_str, error = %e, "E474: Invalid value");
            }
        }
    }

    /// Handle dynamic option query (`:set optionname?`)
    fn handle_dynamic_option_query(&self, name: &str) {
        if let Some(value) = self.option_registry.get(name) {
            tracing::info!(option = %name, value = %value, "Option value: {name}={value}");
        } else {
            tracing::warn!(option = %name, "E518: Unknown option");
        }
    }

    /// Handle dynamic option reset (`:set optionname&`)
    fn handle_dynamic_option_reset(&self, name: &str) {
        use crate::option::{ChangeSource, OptionChanged};

        // Get the spec to find the default value
        let Some(spec) = self.option_registry.get_spec(name) else {
            tracing::warn!(option = %name, "E518: Unknown option");
            return;
        };

        let old_value = self
            .option_registry
            .get(name)
            .unwrap_or_else(|| spec.default.clone());
        let default_value = spec.default;

        match self.option_registry.reset(name) {
            Ok(_) => {
                tracing::info!(option = %name, "Option reset to default");
                // Emit change event
                self.event_bus.emit(OptionChanged::new(
                    name,
                    old_value,
                    default_value,
                    ChangeSource::UserCommand,
                ));
            }
            Err(e) => {
                tracing::warn!(option = %name, error = %e, "Failed to reset option");
            }
        }
    }
}
