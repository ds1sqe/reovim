use crate::buffer::Buffer;
use crate::modd::{Mod, ModExtension};

use super::{Command, CommandContext};

/// Result of command execution
#[derive(Debug)]
pub enum CommandResult {
    Success,
    ModeChange(Mod),
    NeedsRender,
    Quit,
    /// Visual delete: returns deleted text to store in clipboard
    VisualDeleteResult(String),
    /// Visual yank: returns yanked text to store in clipboard
    VisualYankResult(String),
    /// Command line commands are handled by Runtime
    CommandLineCommand,
    /// Paste commands need clipboard access from Runtime
    PasteCommand,
    Error(String),
}

/// Executes buffer-related commands
pub struct BufferCommandExecutor;

impl BufferCommandExecutor {
    pub fn execute_on_buffer(
        buffer: &mut Buffer,
        cmd: &Command,
        ctx: &CommandContext,
    ) -> CommandResult {
        let count = ctx.count.unwrap_or(1);

        match cmd {
            // === Cursor Movement ===
            Command::CursorUp => {
                buffer.cur.y = buffer.cur.y.saturating_sub(count as u16);
                CommandResult::NeedsRender
            }
            Command::CursorDown => {
                let max_y = buffer.contents.len().saturating_sub(1) as u16;
                buffer.cur.y = (buffer.cur.y + count as u16).min(max_y);
                CommandResult::NeedsRender
            }
            Command::CursorLeft => {
                buffer.cur.x = buffer.cur.x.saturating_sub(count as u16);
                CommandResult::NeedsRender
            }
            Command::CursorRight => {
                if let Some(line) = buffer.contents.get(buffer.cur.y as usize) {
                    let max_x = line.inner.len().saturating_sub(1).max(0) as u16;
                    buffer.cur.x = (buffer.cur.x + count as u16).min(max_x);
                }
                CommandResult::NeedsRender
            }
            Command::CursorLineStart => {
                buffer.cur.x = 0;
                CommandResult::NeedsRender
            }
            Command::CursorLineEnd => {
                if let Some(line) = buffer.contents.get(buffer.cur.y as usize) {
                    buffer.cur.x = line.inner.len().saturating_sub(1).max(0) as u16;
                }
                CommandResult::NeedsRender
            }
            Command::CursorWordForward => {
                buffer.word_forward();
                CommandResult::NeedsRender
            }
            Command::CursorWordBackward => {
                buffer.word_backward();
                CommandResult::NeedsRender
            }

            // === Mode Switching ===
            Command::EnterNormalMode => {
                // Move cursor back one position (vim behavior when exiting insert mode)
                if buffer.cur.x > 0 {
                    buffer.cur.x -= 1;
                }
                CommandResult::ModeChange(Mod::Normal)
            }
            Command::EnterInsertMode => {
                CommandResult::ModeChange(Mod::Insert(ModExtension::Normal))
            }
            Command::EnterInsertModeAfter => {
                // Move cursor right before entering insert mode
                if let Some(line) = buffer.contents.get(buffer.cur.y as usize) {
                    if (buffer.cur.x as usize) < line.inner.len() {
                        buffer.cur.x += 1;
                    }
                }
                CommandResult::ModeChange(Mod::Insert(ModExtension::Normal))
            }
            Command::EnterInsertModeEndOfLine => {
                // Move cursor to end of line, then enter insert mode
                if let Some(line) = buffer.contents.get(buffer.cur.y as usize) {
                    buffer.cur.x = line.inner.len() as u16;
                }
                CommandResult::ModeChange(Mod::Insert(ModExtension::Normal))
            }
            Command::OpenLineBelow => {
                // Insert new line below current line and enter insert mode
                if buffer.contents.is_empty() {
                    buffer.contents.push(crate::buffer::Line::from(""));
                } else {
                    let insert_at = (buffer.cur.y as usize + 1).min(buffer.contents.len());
                    buffer.contents.insert(insert_at, crate::buffer::Line::from(""));
                    buffer.cur.y += 1;
                }
                buffer.cur.x = 0;
                CommandResult::ModeChange(Mod::Insert(ModExtension::Normal))
            }
            Command::OpenLineAbove => {
                // Insert new line above current line and enter insert mode
                if buffer.contents.is_empty() {
                    buffer.contents.push(crate::buffer::Line::from(""));
                } else {
                    let insert_at = buffer.cur.y as usize;
                    buffer.contents.insert(insert_at, crate::buffer::Line::from(""));
                }
                buffer.cur.x = 0;
                CommandResult::ModeChange(Mod::Insert(ModExtension::Normal))
            }
            Command::EnterVisualMode => {
                CommandResult::ModeChange(Mod::Visual(ModExtension::Normal))
            }
            Command::EnterCommandMode => {
                CommandResult::ModeChange(Mod::Command)
            }

            // === Text Operations ===
            Command::InsertChar(c) => {
                buffer.insert_char(*c);
                CommandResult::NeedsRender
            }
            Command::DeleteCharBackward => {
                buffer.delete_char_backward();
                CommandResult::NeedsRender
            }
            Command::DeleteCharForward => {
                buffer.delete_char_forward();
                CommandResult::NeedsRender
            }
            Command::DeleteLine => {
                buffer.delete_line();
                CommandResult::NeedsRender
            }

            // === Visual Mode ===
            Command::VisualExtendUp => {
                buffer.cur.y = buffer.cur.y.saturating_sub(count as u16);
                CommandResult::NeedsRender
            }
            Command::VisualExtendDown => {
                let max_y = buffer.contents.len().saturating_sub(1) as u16;
                buffer.cur.y = (buffer.cur.y + count as u16).min(max_y);
                CommandResult::NeedsRender
            }
            Command::VisualExtendLeft => {
                buffer.cur.x = buffer.cur.x.saturating_sub(count as u16);
                CommandResult::NeedsRender
            }
            Command::VisualExtendRight => {
                if let Some(line) = buffer.contents.get(buffer.cur.y as usize) {
                    let max_x = line.inner.len().saturating_sub(1).max(0) as u16;
                    buffer.cur.x = (buffer.cur.x + count as u16).min(max_x);
                }
                CommandResult::NeedsRender
            }
            Command::VisualDelete => {
                let text = buffer.delete_selection();
                CommandResult::VisualDeleteResult(text)
            }
            Command::VisualYank => {
                let text = buffer.get_selected_text();
                buffer.clear_selection();
                CommandResult::VisualYankResult(text)
            }

            // === Command Line Mode ===
            Command::CommandLineChar(_)
            | Command::CommandLineBackspace
            | Command::CommandLineExecute
            | Command::CommandLineCancel => CommandResult::CommandLineCommand,

            // === Clipboard ===
            Command::Paste | Command::PasteBefore => CommandResult::PasteCommand,

            // === System ===
            Command::Quit => CommandResult::Quit,
            Command::Noop => CommandResult::Success,
        }
    }
}
