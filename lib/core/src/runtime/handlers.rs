//! Event handler dispatch methods for Runtime

use std::sync::Arc;

use crate::{
    bind::CommandRef,
    buffer::TextOps,
    command::{
        CommandTrait,
        traits::{
            BufferAction, CommandLineAction, CommandResult, CompletionAction, DeferredAction,
            ExecutionContext, ExplorerAction, FoldAction, LeapAction, OperatorMotionAction,
            SettingsMenuAction, TabAction, TelescopeAction, WindowAction,
        },
    },
    command_line::{ExCommand, SetOption},
    event::{CommandEvent, VisualTextObjectAction},
    explorer::ExplorerState,
    highlight::Theme,
    modd::ModeState,
    screen::{NavigateDirection, Position, SplitDirection},
    textobject::SemanticTextObjectSpec,
    treesitter::TreesitterTheme,
};

use super::Runtime;

impl Runtime {
    /// Schedule a treesitter reparse for the given buffer if it has a parser
    pub(crate) fn schedule_treesitter_reparse(&mut self, buffer_id: usize) {
        if self.treesitter.has_parser(buffer_id) {
            self.treesitter.schedule_reparse(buffer_id);
        }
    }

    /// Delete a semantic text object using treesitter
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

        // Find text object bounds using treesitter
        let bounds = self.treesitter.find_text_object_bounds(
            buffer_id, &content, cursor_row, cursor_col, spec.kind, spec.scope,
        )?;

        // Convert treesitter Position to screen Position
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

    /// Yank a semantic text object using treesitter
    ///
    /// Returns the yanked text if successful, None if bounds couldn't be found.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn yank_semantic_text_object(
        &mut self,
        buffer_id: usize,
        spec: SemanticTextObjectSpec,
    ) -> Option<String> {
        // Get buffer content and cursor position
        let (content, cursor_row, cursor_col) = {
            let buffer = self.buffers.get(&buffer_id)?;
            (buffer.content_to_string(), u32::from(buffer.cur.y), u32::from(buffer.cur.x))
        };

        // Find text object bounds using treesitter
        let bounds = self.treesitter.find_text_object_bounds(
            buffer_id, &content, cursor_row, cursor_col, spec.kind, spec.scope,
        )?;

        // Convert treesitter Position to screen Position
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

    /// Find the bounds of a semantic text object using treesitter
    ///
    /// Returns (start, end) positions if bounds found, None otherwise.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn find_semantic_text_object_bounds(
        &mut self,
        buffer_id: usize,
        spec: SemanticTextObjectSpec,
    ) -> Option<(Position, Position)> {
        // Get buffer content and cursor position
        let (content, cursor_row, cursor_col) = {
            let buffer = self.buffers.get(&buffer_id)?;
            (buffer.content_to_string(), u32::from(buffer.cur.y), u32::from(buffer.cur.x))
        };

        // Find text object bounds using treesitter
        let bounds = self.treesitter.find_text_object_bounds(
            buffer_id, &content, cursor_row, cursor_col, spec.kind, spec.scope,
        )?;

        // Convert treesitter Position to screen Position
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
                                self.treesitter
                                    .set_theme(TreesitterTheme::from_theme_name(name));
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
                        },
                        ExCommand::Colorscheme { name } => {
                            self.theme = Theme::from_name(name);
                            self.treesitter
                                .set_theme(TreesitterTheme::from_theme_name(name));
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
                            // Open telescope with profiles picker
                            self.handle_profile_list();
                        }
                        ExCommand::Settings => {
                            // Open settings menu via event
                            use crate::event::{InnerEvent, SettingsMenuEvent};
                            let tx = self.tx.clone();
                            tokio::spawn(async move {
                                let _ = tx
                                    .send(InnerEvent::SettingsMenuEvent(SettingsMenuEvent::Open))
                                    .await;
                            });
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
        let CommandEvent { command, context } = cmd_event;

        // Resolve the command reference to a trait object
        let Some(cmd) = self.resolve_command(&command) else {
            // Command not found in registry
            return false;
        };

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
                CommandResult::ClipboardWrite { text, register } => {
                    self.registers.set_by_name(register, text);
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
                        DeferredAction::Completion(comp_action) => {
                            self.handle_completion_action(&comp_action, context.buffer_id);
                        }
                        DeferredAction::Explorer(explorer_action) => {
                            self.handle_explorer_action(&explorer_action);
                            self.request_render();
                        }
                        DeferredAction::Telescope(telescope_action) => {
                            self.handle_telescope_action(&telescope_action);
                        }
                        DeferredAction::Fold(fold_action) => {
                            self.handle_fold_action(&fold_action, buffer_id);
                            self.request_render();
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
                        DeferredAction::Leap(ref leap_action) => {
                            self.handle_leap_action(leap_action);
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
                        DeferredAction::SettingsMenu(ref action) => {
                            self.handle_settings_menu_action(action);
                        }
                    }
                }
                CommandResult::Error(msg) => {
                    tracing::warn!(error = %msg, "Command execution failed");
                }
                CommandResult::Success => {}
            }
        }
        false
    }

    /// Handle explorer actions
    #[allow(clippy::cast_possible_wrap)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cognitive_complexity)]
    pub(crate) fn handle_explorer_action(&mut self, action: &ExplorerAction) {
        // Explorer actions require explorer_state which will be integrated in Phase 5
        // For now, we handle the basic mode transitions
        match action {
            ExplorerAction::Toggle => {
                // Toggle explorer visibility and mode
                self.screen.toggle_explorer();
                if self.screen.layout().is_explorer_visible() {
                    self.set_mode(ModeState::explorer());
                    // Initialize explorer state if needed
                    if self.explorer_state.is_none()
                        && let Ok(cwd) = std::env::current_dir()
                    {
                        self.explorer_state = ExplorerState::new(cwd).ok();
                    }
                } else {
                    self.set_mode(ModeState::normal());
                }
            }
            ExplorerAction::Close | ExplorerAction::FocusEditor => {
                // Switch focus back to editor
                self.screen.focus_editor();
                self.set_mode(ModeState::normal());
            }
            ExplorerAction::CursorUp { count } => {
                if let Some(ref mut state) = self.explorer_state {
                    state.move_cursor(-(*count as isize));
                    state.update_visual_selection();
                }
            }
            ExplorerAction::CursorDown { count } => {
                if let Some(ref mut state) = self.explorer_state {
                    state.move_cursor(*count as isize);
                    state.update_visual_selection();
                }
            }
            ExplorerAction::PageUp => {
                if let Some(ref mut state) = self.explorer_state {
                    let height = self.screen.layout().explorer_height();
                    state.move_page(height, false);
                }
            }
            ExplorerAction::PageDown => {
                if let Some(ref mut state) = self.explorer_state {
                    let height = self.screen.layout().explorer_height();
                    state.move_page(height, true);
                }
            }
            ExplorerAction::GotoFirst => {
                if let Some(ref mut state) = self.explorer_state {
                    state.move_to_first();
                }
            }
            ExplorerAction::GotoLast => {
                if let Some(ref mut state) = self.explorer_state {
                    state.move_to_last();
                }
            }
            ExplorerAction::ToggleNode => {
                if let Some(ref mut state) = self.explorer_state {
                    let _ = state.toggle_current();
                }
            }
            ExplorerAction::OpenNode => {
                // Open file or toggle directory
                // First extract info from current node without holding mutable borrow
                let node_info = self
                    .explorer_state
                    .as_ref()
                    .and_then(|state| state.current_node())
                    .map(|node| {
                        (node.is_dir(), node.is_file(), node.is_symlink(), node.path.clone())
                    });

                if let Some((is_dir, is_file, is_symlink, path)) = node_info {
                    if is_dir {
                        if let Some(ref mut state) = self.explorer_state {
                            let _ = state.toggle_current();
                        }
                    } else if is_file || is_symlink {
                        // Open file in a new buffer (symlinks are opened as target file)
                        self.open_file(&path.to_string_lossy());
                        // Update the editor window to show the new buffer
                        self.screen.set_editor_buffer(self.active_buffer_id);
                        // Switch focus to editor
                        self.screen.focus_editor();
                        self.set_mode(ModeState::normal());
                    }
                }
            }
            ExplorerAction::CloseParent => {
                if let Some(ref mut state) = self.explorer_state {
                    state.collapse_current();
                }
            }
            ExplorerAction::GoToParent => {
                if let Some(ref mut state) = self.explorer_state {
                    state.go_to_parent();
                }
            }
            ExplorerAction::Refresh => {
                if let Some(ref mut state) = self.explorer_state {
                    let _ = state.refresh();
                }
            }
            ExplorerAction::ToggleHidden => {
                if let Some(ref mut state) = self.explorer_state {
                    state.toggle_hidden();
                }
            }
            ExplorerAction::ToggleSizes => {
                if let Some(ref mut state) = self.explorer_state {
                    state.toggle_sizes();
                }
            }
            ExplorerAction::Yank => {
                if let Some(ref mut state) = self.explorer_state {
                    state.yank_current();
                }
            }
            ExplorerAction::Cut => {
                if let Some(ref mut state) = self.explorer_state {
                    state.cut_current();
                }
            }
            ExplorerAction::Paste => {
                if let Some(ref mut state) = self.explorer_state {
                    let _ = state.paste();
                }
            }
            ExplorerAction::VisualMode => {
                if let Some(ref mut state) = self.explorer_state {
                    state.enter_visual_mode();
                }
            }
            ExplorerAction::ToggleSelect => {
                if let Some(ref mut state) = self.explorer_state {
                    state.toggle_select_current();
                }
            }
            ExplorerAction::SelectAll => {
                if let Some(ref mut state) = self.explorer_state {
                    state.select_all();
                }
            }
            ExplorerAction::ExitVisual => {
                if let Some(ref mut state) = self.explorer_state {
                    state.exit_visual_mode();
                }
            }
            ExplorerAction::CreateFile => {
                if let Some(ref mut state) = self.explorer_state {
                    state.start_create_file();
                    self.set_mode(ModeState::explorer_input());
                }
            }
            ExplorerAction::CreateDir => {
                if let Some(ref mut state) = self.explorer_state {
                    state.start_create_dir();
                    self.set_mode(ModeState::explorer_input());
                }
            }
            ExplorerAction::Rename => {
                if let Some(ref mut state) = self.explorer_state {
                    state.start_rename();
                    // Only switch mode if rename actually started (node exists)
                    if state.is_input_mode() {
                        self.set_mode(ModeState::explorer_input());
                    }
                }
            }
            ExplorerAction::Delete => {
                if let Some(ref mut state) = self.explorer_state {
                    state.start_delete();
                    // Only switch mode if delete actually started (node exists)
                    if state.is_input_mode() {
                        self.set_mode(ModeState::explorer_input());
                    }
                }
            }
            ExplorerAction::StartFilter => {
                if let Some(ref mut state) = self.explorer_state {
                    state.start_filter();
                    self.set_mode(ModeState::explorer_input());
                }
            }
            ExplorerAction::ClearFilter => {
                if let Some(ref mut state) = self.explorer_state {
                    state.clear_filter();
                }
            }
            ExplorerAction::ConfirmInput { input: _ } => {
                if let Some(ref mut state) = self.explorer_state {
                    let _ = state.confirm_input();
                    self.set_mode(ModeState::explorer());
                }
            }
            ExplorerAction::CancelInput => {
                if let Some(ref mut state) = self.explorer_state {
                    state.cancel_input();
                    self.set_mode(ModeState::explorer());
                }
            }
            ExplorerAction::InputChar { c } => {
                if let Some(ref mut state) = self.explorer_state {
                    state.input_char(*c);
                }
            }
            ExplorerAction::InputBackspace => {
                if let Some(ref mut state) = self.explorer_state {
                    state.input_backspace();
                }
            }
        }
    }

    /// Handle completion actions from keybindings
    pub(crate) fn handle_completion_action(&mut self, action: &CompletionAction, buffer_id: usize) {
        match action {
            CompletionAction::Trigger => {
                self.trigger_completion(buffer_id);
            }
            CompletionAction::SelectNext => {
                self.completion_state.select_next();
                self.request_render();
            }
            CompletionAction::SelectPrev => {
                self.completion_state.select_prev();
                self.request_render();
            }
            CompletionAction::Confirm => {
                if let Some(item) = self.completion_state.selected_item().cloned() {
                    self.insert_completion(&item);
                }
                self.completion_state.dismiss();
                self.request_render();
            }
            CompletionAction::Dismiss => {
                self.completion_state.dismiss();
                self.request_render();
            }
        }
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
                    let yanked = buffer.yank_to_motion(motion, count);
                    if !yanked.is_empty() {
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
                    let yanked = buffer.yank_text_object(text_object);
                    if !yanked.is_empty() {
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

    /// Handle telescope actions from keybindings
    pub(crate) fn handle_telescope_action(&mut self, action: &TelescopeAction) {
        use crate::event::{InnerEvent, TelescopeEvent};

        match action {
            TelescopeAction::Open { picker } => {
                // Send TelescopeEvent::Open to the event loop
                let tx = self.tx.clone();
                let picker_name = picker.clone();
                tokio::spawn(async move {
                    let _ = tx
                        .send(InnerEvent::TelescopeEvent(TelescopeEvent::Open {
                            picker: picker_name,
                        }))
                        .await;
                });
            }
            TelescopeAction::InsertChar(c) => {
                self.telescope_state.insert_char(*c);
                // Trigger filtering via UpdateQuery event
                let tx = self.tx.clone();
                let query = self.telescope_state.query.clone();
                tokio::spawn(async move {
                    let _ = tx
                        .send(InnerEvent::TelescopeEvent(TelescopeEvent::UpdateQuery { query }))
                        .await;
                });
            }
            TelescopeAction::Backspace => {
                self.telescope_state.delete_char();
                // Trigger filtering via UpdateQuery event
                let tx = self.tx.clone();
                let query = self.telescope_state.query.clone();
                tokio::spawn(async move {
                    let _ = tx
                        .send(InnerEvent::TelescopeEvent(TelescopeEvent::UpdateQuery { query }))
                        .await;
                });
            }
            TelescopeAction::CursorLeft => {
                self.telescope_state.cursor_left();
                self.request_render();
            }
            TelescopeAction::CursorRight => {
                self.telescope_state.cursor_right();
                self.request_render();
            }
            TelescopeAction::SelectNext => {
                self.telescope_state.select_next();
                self.request_render();
            }
            TelescopeAction::SelectPrev => {
                self.telescope_state.select_prev();
                self.request_render();
            }
            TelescopeAction::PageDown => {
                self.telescope_state.page_down();
                self.request_render();
            }
            TelescopeAction::PageUp => {
                self.telescope_state.page_up();
                self.request_render();
            }
            TelescopeAction::GotoFirst => {
                self.telescope_state.move_to_first();
                self.request_render();
            }
            TelescopeAction::GotoLast => {
                self.telescope_state.move_to_last();
                self.request_render();
            }
            TelescopeAction::Confirm => {
                // Send TelescopeEvent::Confirm to the event loop
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    let _ = tx
                        .send(InnerEvent::TelescopeEvent(TelescopeEvent::Confirm))
                        .await;
                });
            }
            TelescopeAction::Close => {
                // Close telescope and return to normal mode (synchronous)
                use crate::modd::ModeState;
                self.telescope_state.close();
                self.set_mode(ModeState::normal());
                self.render();
            }
            TelescopeAction::EnterInsert => {
                // Switch telescope to insert mode (for typing query)
                use crate::modd::{EditMode, Focus, ModExtension, ModeState};
                let mode = ModeState::with_focus_and_mode(
                    Focus::Telescope,
                    EditMode::Insert(ModExtension::Normal),
                );
                self.mode_state = mode.clone();
                let _ = self.mode_tx.send(mode);
                self.request_render();
            }
            TelescopeAction::EnterNormal => {
                // Switch telescope to normal mode (for j/k navigation)
                use crate::modd::{EditMode, Focus, ModeState};
                let mode = ModeState::with_focus_and_mode(Focus::Telescope, EditMode::Normal);
                self.mode_state = mode.clone();
                let _ = self.mode_tx.send(mode);
                self.request_render();
            }
        }
    }

    /// Handle leap motion actions
    pub(crate) fn handle_leap_action(&mut self, action: &LeapAction) {
        match action {
            LeapAction::Start {
                direction,
                operator,
                count,
            } => {
                // Activate leap state
                if let Some(op) = operator {
                    self.leap_state
                        .activate_with_operator(*direction, *op, *count);
                } else {
                    self.leap_state.activate(*direction);
                }

                // Change to leap mode
                self.set_mode(ModeState::leap(*direction, *operator, *count));
                self.request_render();
            }
        }
    }

    /// Handle fold actions from keybindings
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn handle_fold_action(&mut self, action: &FoldAction, buffer_id: usize) {
        // Get the cursor line from the buffer
        let cursor_line = self
            .buffers
            .get(&buffer_id)
            .map_or(0, |buf| u32::from(buf.cur.y));

        match action {
            FoldAction::Toggle => {
                self.fold_manager.toggle(buffer_id, cursor_line);
            }
            FoldAction::Open => {
                self.fold_manager.open(buffer_id, cursor_line);
            }
            FoldAction::Close => {
                self.fold_manager.close(buffer_id, cursor_line);
            }
            FoldAction::OpenAll => {
                self.fold_manager.open_all(buffer_id);
            }
            FoldAction::CloseAll => {
                self.fold_manager.close_all(buffer_id);
            }
        }
    }

    // === Window Management Handlers ===

    /// Synchronize mode state with screen focus
    ///
    /// The screen is the source of truth for focus (explorer vs editor).
    /// This method updates the mode state to match.
    pub(crate) fn sync_mode_with_screen_focus(&mut self) {
        use crate::modd::Focus;

        let screen_has_explorer_focus = self.screen.is_explorer_focused();
        let mode_has_explorer_focus = self.mode_state.focus == Focus::Explorer;

        if screen_has_explorer_focus && !mode_has_explorer_focus {
            self.set_mode(ModeState::explorer());
        } else if !screen_has_explorer_focus && mode_has_explorer_focus {
            self.set_mode(ModeState::normal());
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
            WindowAction::FocusOrSplitLeft => {
                self.handle_focus_or_split(NavigateDirection::Left, SplitDirection::Vertical);
            }
            WindowAction::FocusOrSplitDown => {
                self.handle_focus_or_split(NavigateDirection::Down, SplitDirection::Horizontal);
            }
            WindowAction::FocusOrSplitUp => {
                self.handle_focus_or_split(NavigateDirection::Up, SplitDirection::Horizontal);
            }
            WindowAction::FocusOrSplitRight => {
                self.handle_focus_or_split(NavigateDirection::Right, SplitDirection::Vertical);
            }
        }
        false
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
                window_id = window.id,
                "SPLIT: saving buffer.cur=({},{}) to window.cursor before split",
                buffer.cur.x,
                buffer.cur.y
            );
            window.cursor = buffer.cur;
            window.desired_col = buffer.desired_col;
        } else {
            tracing::warn!("SPLIT: failed to get active window or buffer!");
        }

        // Split the window
        if let Some(new_window_id) = self.screen.split_window(direction) {
            // Set the buffer for the new window
            self.screen.set_window_buffer(new_window_id, buffer_id);
            tracing::info!(
                window_id = new_window_id,
                buffer_id = buffer_id,
                vertical = vertical,
                "Window split created"
            );
        }
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
        let before_window_id = self.screen.active_window_id();

        // Before navigation: save current buffer cursor to current window
        if let Some(window) = self.screen.active_window_mut()
            && let Some(buffer) = self.buffers.get(&window.buffer_id)
        {
            tracing::debug!(
                window_id = window.id,
                "SAVE cursor: buffer.cur=({},{}) -> window.cursor",
                buffer.cur.x,
                buffer.cur.y
            );
            window.cursor = buffer.cur;
            window.desired_col = buffer.desired_col;
        }

        // Navigate to new window
        self.screen.navigate_window(direction);

        let after_window_id = self.screen.active_window_id();
        tracing::debug!(?before_window_id, ?after_window_id, ?direction, "Window navigation");

        // After navigation: load new window's cursor into buffer and update active_buffer_id
        if let Some(window) = self.screen.active_window() {
            let new_buffer_id = window.buffer_id;
            let window_cursor = window.cursor;
            let window_desired_col = window.desired_col;

            tracing::debug!(
                window_id = window.id,
                "LOAD cursor: window.cursor=({},{}) -> buffer.cur",
                window_cursor.x,
                window_cursor.y
            );

            // Update active_buffer_id to match the new window's buffer
            self.active_buffer_id = new_buffer_id;

            // Load window's cursor into buffer
            if let Some(buffer) = self.buffers.get_mut(&new_buffer_id) {
                buffer.cur = window_cursor;
                buffer.desired_col = window_desired_col;
            }
        }
    }

    /// Handle window equalize
    pub(crate) fn handle_window_equalize(&mut self) {
        self.screen.equalize_windows();
    }

    /// Handle smart focus: focus existing window or create split if none exists
    ///
    /// Uses `handle_window_navigate` for proper cursor save/restore.
    /// See docs/window-buffer.md for the window-buffer architecture.
    pub(crate) fn handle_focus_or_split(
        &mut self,
        direction: NavigateDirection,
        split_direction: SplitDirection,
    ) {
        // Get current window ID before navigation attempt
        let before_id = self.screen.active_window_id();

        // Try to navigate using the proper handler (with cursor save/restore)
        self.handle_window_navigate(direction);

        // Check if navigation succeeded by comparing window IDs
        let after_id = self.screen.active_window_id();

        // If window ID didn't change, no adjacent window exists - create a split
        if before_id == after_id {
            // Create a split with the current buffer
            let buffer_id = self.active_buffer_id;
            if let Some(new_window_id) = self.screen.split_window(split_direction) {
                self.screen.set_window_buffer(new_window_id, buffer_id);
                // Navigate to the new window (with cursor save/restore)
                self.handle_window_navigate(direction);
                tracing::info!(
                    direction = ?direction,
                    new_window_id = new_window_id,
                    "Smart focus: created split"
                );
            }
        } else {
            tracing::info!(direction = ?direction, "Smart focus: navigated to existing window");
        }
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
    // Profile Management
    // ========================================================================

    /// Handle :profile list command - opens telescope profiles picker
    pub(crate) fn handle_profile_list(&self) {
        use crate::event::{InnerEvent, TelescopeEvent};

        // Open telescope with profiles picker
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx
                .send(InnerEvent::TelescopeEvent(TelescopeEvent::Open {
                    picker: "profiles".to_string(),
                }))
                .await;
        });
    }

    // ========================================================================
    // Settings Menu Handlers
    // ========================================================================

    /// Handle settings menu actions from deferred commands
    pub(crate) fn handle_settings_menu_action(&self, action: &SettingsMenuAction) {
        use crate::event::{InnerEvent, SettingsMenuEvent};

        let event = match *action {
            SettingsMenuAction::Open => SettingsMenuEvent::Open,
            SettingsMenuAction::Close => SettingsMenuEvent::Close,
            SettingsMenuAction::SelectNext => SettingsMenuEvent::SelectNext,
            SettingsMenuAction::SelectPrev => SettingsMenuEvent::SelectPrev,
            SettingsMenuAction::Toggle => SettingsMenuEvent::Toggle,
            SettingsMenuAction::CycleNext => SettingsMenuEvent::CycleNext,
            SettingsMenuAction::CyclePrev => SettingsMenuEvent::CyclePrev,
            SettingsMenuAction::QuickSelect(n) => SettingsMenuEvent::QuickSelect(n),
            SettingsMenuAction::Increment => SettingsMenuEvent::Increment,
            SettingsMenuAction::Decrement => SettingsMenuEvent::Decrement,
            SettingsMenuAction::ExecuteAction => SettingsMenuEvent::ExecuteAction,
        };

        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(InnerEvent::SettingsMenuEvent(event)).await;
        });
    }
}
