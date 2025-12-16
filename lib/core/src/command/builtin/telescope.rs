//! Telescope fuzzy finder commands

use crate::command::traits::{
    CommandResult, CommandTrait, DeferredAction, ExecutionContext, TelescopeAction,
};
use std::any::Any;

/// Open telescope find files picker
#[derive(Debug, Clone)]
pub struct TelescopeFindFilesCommand;

impl CommandTrait for TelescopeFindFilesCommand {
    fn name(&self) -> &'static str {
        "telescope_find_files"
    }

    fn description(&self) -> &'static str {
        "Find files with fuzzy search"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::Open {
            picker: "files".to_string(),
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Open telescope buffers picker
#[derive(Debug, Clone)]
pub struct TelescopeFindBuffersCommand;

impl CommandTrait for TelescopeFindBuffersCommand {
    fn name(&self) -> &'static str {
        "telescope_find_buffers"
    }

    fn description(&self) -> &'static str {
        "Find open buffers"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::Open {
            picker: "buffers".to_string(),
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Open telescope live grep picker
#[derive(Debug, Clone)]
pub struct TelescopeLiveGrepCommand;

impl CommandTrait for TelescopeLiveGrepCommand {
    fn name(&self) -> &'static str {
        "telescope_live_grep"
    }

    fn description(&self) -> &'static str {
        "Search text in files"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::Open {
            picker: "grep".to_string(),
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Open telescope recent files picker
#[derive(Debug, Clone)]
pub struct TelescopeRecentFilesCommand;

impl CommandTrait for TelescopeRecentFilesCommand {
    fn name(&self) -> &'static str {
        "telescope_recent_files"
    }

    fn description(&self) -> &'static str {
        "Find recently opened files"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::Open {
            picker: "recent".to_string(),
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Open telescope commands picker (command palette)
#[derive(Debug, Clone)]
pub struct TelescopeCommandsCommand;

impl CommandTrait for TelescopeCommandsCommand {
    fn name(&self) -> &'static str {
        "telescope_commands"
    }

    fn description(&self) -> &'static str {
        "Open command palette"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::Open {
            picker: "commands".to_string(),
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Open telescope help tags picker
#[derive(Debug, Clone)]
pub struct TelescopeHelpTagsCommand;

impl CommandTrait for TelescopeHelpTagsCommand {
    fn name(&self) -> &'static str {
        "telescope_help_tags"
    }

    fn description(&self) -> &'static str {
        "Search help tags"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::Open {
            picker: "help".to_string(),
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Open telescope keymaps picker
#[derive(Debug, Clone)]
pub struct TelescopeKeymapsCommand;

impl CommandTrait for TelescopeKeymapsCommand {
    fn name(&self) -> &'static str {
        "telescope_keymaps"
    }

    fn description(&self) -> &'static str {
        "Search keybindings"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::Open {
            picker: "keymaps".to_string(),
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Select next item in telescope
#[derive(Debug, Clone)]
pub struct TelescopeSelectNextCommand;

impl CommandTrait for TelescopeSelectNextCommand {
    fn name(&self) -> &'static str {
        "telescope_select_next"
    }

    fn description(&self) -> &'static str {
        "Select next item in telescope"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::SelectNext))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Select previous item in telescope
#[derive(Debug, Clone)]
pub struct TelescopeSelectPrevCommand;

impl CommandTrait for TelescopeSelectPrevCommand {
    fn name(&self) -> &'static str {
        "telescope_select_prev"
    }

    fn description(&self) -> &'static str {
        "Select previous item in telescope"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::SelectPrev))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Page down in telescope
#[derive(Debug, Clone)]
pub struct TelescopePageDownCommand;

impl CommandTrait for TelescopePageDownCommand {
    fn name(&self) -> &'static str {
        "telescope_page_down"
    }

    fn description(&self) -> &'static str {
        "Page down in telescope"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::PageDown))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Page up in telescope
#[derive(Debug, Clone)]
pub struct TelescopePageUpCommand;

impl CommandTrait for TelescopePageUpCommand {
    fn name(&self) -> &'static str {
        "telescope_page_up"
    }

    fn description(&self) -> &'static str {
        "Page up in telescope"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::PageUp))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Confirm selection in telescope
#[derive(Debug, Clone)]
pub struct TelescopeConfirmCommand;

impl CommandTrait for TelescopeConfirmCommand {
    fn name(&self) -> &'static str {
        "telescope_confirm"
    }

    fn description(&self) -> &'static str {
        "Confirm selection in telescope"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::Confirm))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Close telescope
#[derive(Debug, Clone)]
pub struct TelescopeCloseCommand;

impl CommandTrait for TelescopeCloseCommand {
    fn name(&self) -> &'static str {
        "telescope_close"
    }

    fn description(&self) -> &'static str {
        "Close telescope"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::Close))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Insert character in telescope query
#[derive(Debug, Clone)]
pub struct TelescopeInsertCharCommand {
    c: char,
}

impl TelescopeInsertCharCommand {
    #[must_use]
    pub const fn new(c: char) -> Self {
        Self { c }
    }
}

impl CommandTrait for TelescopeInsertCharCommand {
    fn name(&self) -> &'static str {
        "telescope_insert_char"
    }

    fn description(&self) -> &'static str {
        "Insert character in telescope query"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::InsertChar(
            self.c,
        )))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Backspace in telescope query
#[derive(Debug, Clone)]
pub struct TelescopeBackspaceCommand;

impl CommandTrait for TelescopeBackspaceCommand {
    fn name(&self) -> &'static str {
        "telescope_backspace"
    }

    fn description(&self) -> &'static str {
        "Delete character in telescope query"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::DeferToRuntime(DeferredAction::Telescope(TelescopeAction::Backspace))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
