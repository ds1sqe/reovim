//! Event handler dispatch methods for Runtime

use std::sync::Arc;

use crate::bind::CommandRef;
use crate::buffer::TextOps;
use crate::command::{
    traits::{
        CommandLineAction, CommandResult, CompletionAction, DeferredAction, ExecutionContext,
        ExplorerAction,
    },
    CommandTrait,
};
use crate::explorer::ExplorerState;
use crate::modd::Mod;
use crate::command_line::{ExCommand, SetOption};
use crate::event::CommandEvent;

use super::Runtime;

impl Runtime {
    /// Handle command line actions. Returns true if editor should quit.
    #[allow(clippy::match_same_arms)]
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
                            let path = filename.or_else(|| {
                                self.buffers.get(&0).and_then(|b| b.file_path.clone())
                            });

                            if let Some(path) = path
                                && let Some(buffer) = self.buffers.get_mut(&0)
                            {
                                let content = buffer.content_to_string();
                                match std::fs::write(&path, &content) {
                                    Ok(()) => {
                                        tracing::info!(path = %path, bytes = content.len(), "File saved");
                                        buffer.file_path = Some(path);
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
                                && let Some(buffer) = self.buffers.get(&0)
                            {
                                let content = buffer.content_to_string();
                                match std::fs::write(&path, &content) {
                                    Ok(()) => {
                                        tracing::info!(path = %path, bytes = content.len(), "File saved before quit");
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
                        },
                        ExCommand::Edit { filename } => {
                            self.open_file(&filename);
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

            match result {
                CommandResult::NeedsRender => {
                    self.render();
                }
                CommandResult::ModeChange(new_mode) => {
                    // Actually change the mode for commands like o, O
                    self.set_mode(new_mode);
                    self.render();
                }
                CommandResult::Quit => {
                    return true;
                }
                CommandResult::ClipboardWrite(text) => {
                    self.clipboard = text;
                    self.render();
                }
                CommandResult::DeferToRuntime(action) => {
                    match action {
                        DeferredAction::Paste { before: _before } => {
                            // Handle paste
                            // TODO: implement proper PasteBefore (paste at cursor vs before cursor)
                            if let Some(buf) = self.buffers.get_mut(&context.buffer_id)
                                && !self.clipboard.is_empty()
                            {
                                for c in self.clipboard.chars() {
                                    buf.insert_char(c);
                                }
                            }
                            self.render();
                        }
                        DeferredAction::CommandLine(cl_action) => {
                            let should_quit = self.handle_command_line_action(&cl_action);
                            self.render();
                            return should_quit;
                        }
                        DeferredAction::Completion(comp_action) => {
                            self.handle_completion_action(&comp_action, context.buffer_id);
                        }
                        DeferredAction::Explorer(explorer_action) => {
                            self.handle_explorer_action(&explorer_action);
                            self.render();
                        }
                        DeferredAction::JumpOlder => {
                            if let Some(entry) = self.jump_list.jump_older() {
                                let target_pos = entry.position;
                                let target_buf_id = entry.buffer_id;
                                if let Some(buf) = self.buffers.get_mut(&target_buf_id) {
                                    buf.cur = target_pos;
                                }
                            }
                            self.render();
                        }
                        DeferredAction::JumpNewer => {
                            if let Some(entry) = self.jump_list.jump_newer() {
                                let target_pos = entry.position;
                                let target_buf_id = entry.buffer_id;
                                if let Some(buf) = self.buffers.get_mut(&target_buf_id) {
                                    buf.cur = target_pos;
                                }
                            }
                            self.render();
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
    pub(crate) fn handle_explorer_action(&mut self, action: &ExplorerAction) {
        // Explorer actions require explorer_state which will be integrated in Phase 5
        // For now, we handle the basic mode transitions
        match action {
            ExplorerAction::Toggle => {
                // Toggle explorer visibility and mode
                self.screen.toggle_explorer();
                if self.screen.layout().is_explorer_visible() {
                    self.set_mode(Mod::Explorer);
                    // Initialize explorer state if needed
                    if self.explorer_state.is_none()
                        && let Ok(cwd) = std::env::current_dir()
                    {
                        self.explorer_state = ExplorerState::new(cwd).ok();
                    }
                } else {
                    self.set_mode(Mod::Normal);
                }
            }
            ExplorerAction::Close | ExplorerAction::FocusEditor => {
                // Switch focus back to editor
                self.screen.focus_editor();
                self.set_mode(Mod::Normal);
            }
            ExplorerAction::CursorUp { count } => {
                if let Some(ref mut state) = self.explorer_state {
                    state.move_cursor(-(*count as isize));
                }
            }
            ExplorerAction::CursorDown { count } => {
                if let Some(ref mut state) = self.explorer_state {
                    state.move_cursor(*count as isize);
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
                    .map(|node| (node.is_dir(), node.is_file(), node.is_symlink(), node.path.clone()));

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
                        self.set_mode(Mod::Normal);
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
            ExplorerAction::CreateFile => {
                if let Some(ref mut state) = self.explorer_state {
                    state.start_create_file();
                    self.set_mode(Mod::ExplorerInput);
                }
            }
            ExplorerAction::CreateDir => {
                if let Some(ref mut state) = self.explorer_state {
                    state.start_create_dir();
                    self.set_mode(Mod::ExplorerInput);
                }
            }
            ExplorerAction::Rename => {
                if let Some(ref mut state) = self.explorer_state {
                    state.start_rename();
                    // Only switch mode if rename actually started (node exists)
                    if state.is_input_mode() {
                        self.set_mode(Mod::ExplorerInput);
                    }
                }
            }
            ExplorerAction::Delete => {
                if let Some(ref mut state) = self.explorer_state {
                    state.start_delete();
                    // Only switch mode if delete actually started (node exists)
                    if state.is_input_mode() {
                        self.set_mode(Mod::ExplorerInput);
                    }
                }
            }
            ExplorerAction::StartFilter => {
                if let Some(ref mut state) = self.explorer_state {
                    state.start_filter();
                    self.set_mode(Mod::ExplorerInput);
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
                    self.set_mode(Mod::Explorer);
                }
            }
            ExplorerAction::CancelInput => {
                if let Some(ref mut state) = self.explorer_state {
                    state.cancel_input();
                    self.set_mode(Mod::Explorer);
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
                self.render();
            }
            CompletionAction::SelectPrev => {
                self.completion_state.select_prev();
                self.render();
            }
            CompletionAction::Confirm => {
                if let Some(item) = self.completion_state.selected_item().cloned() {
                    self.insert_completion(&item);
                }
                self.completion_state.dismiss();
                self.render();
            }
            CompletionAction::Dismiss => {
                self.completion_state.dismiss();
                self.render();
            }
        }
    }
}
