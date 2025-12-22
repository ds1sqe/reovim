//! Telescope fuzzy finder commands
//!
//! These commands emit `EventBus` events that are handled by the runtime.
//! The events are defined in `crate::telescope::events`.

use {
    crate::telescope::{
        TelescopeBackspaceEvent, TelescopeCloseEvent, TelescopeConfirmEvent,
        TelescopeEnterInsertEvent, TelescopeEnterNormalEvent, TelescopeGotoFirstEvent,
        TelescopeGotoLastEvent, TelescopeInsertCharEvent, TelescopeOpenEvent,
        TelescopePageDownEvent, TelescopePageUpEvent, TelescopeSelectNextEvent,
        TelescopeSelectPrevEvent,
    },
    reovim_core::{
        command::traits::{CommandResult, CommandTrait, ExecutionContext},
        event_bus::DynEvent,
    },
    std::any::Any,
};

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
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpenEvent {
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
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpenEvent {
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
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpenEvent {
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
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpenEvent {
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
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpenEvent {
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
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpenEvent {
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
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpenEvent {
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

/// Open telescope themes picker
#[derive(Debug, Clone)]
pub struct TelescopeThemesCommand;

impl CommandTrait for TelescopeThemesCommand {
    fn name(&self) -> &'static str {
        "telescope_themes"
    }

    fn description(&self) -> &'static str {
        "Select colorscheme/theme"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpenEvent {
            picker: "themes".to_string(),
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
        CommandResult::EmitEvent(DynEvent::new(TelescopeSelectNextEvent))
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
        CommandResult::EmitEvent(DynEvent::new(TelescopeSelectPrevEvent))
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
        CommandResult::EmitEvent(DynEvent::new(TelescopePageDownEvent))
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
        CommandResult::EmitEvent(DynEvent::new(TelescopePageUpEvent))
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
        CommandResult::EmitEvent(DynEvent::new(TelescopeConfirmEvent))
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
        CommandResult::EmitEvent(DynEvent::new(TelescopeCloseEvent))
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
        CommandResult::EmitEvent(DynEvent::new(TelescopeInsertCharEvent { c: self.c }))
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
        CommandResult::EmitEvent(DynEvent::new(TelescopeBackspaceEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Go to first item in telescope
#[derive(Debug, Clone)]
pub struct TelescopeGotoFirstCommand;

impl CommandTrait for TelescopeGotoFirstCommand {
    fn name(&self) -> &'static str {
        "telescope_goto_first"
    }

    fn description(&self) -> &'static str {
        "Go to first item in telescope"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(TelescopeGotoFirstEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Go to last item in telescope
#[derive(Debug, Clone)]
pub struct TelescopeGotoLastCommand;

impl CommandTrait for TelescopeGotoLastCommand {
    fn name(&self) -> &'static str {
        "telescope_goto_last"
    }

    fn description(&self) -> &'static str {
        "Go to last item in telescope"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(TelescopeGotoLastEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Enter insert mode in telescope (for typing query)
#[derive(Debug, Clone)]
pub struct TelescopeEnterInsertCommand;

impl CommandTrait for TelescopeEnterInsertCommand {
    fn name(&self) -> &'static str {
        "telescope_enter_insert"
    }

    fn description(&self) -> &'static str {
        "Enter insert mode for typing query"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(TelescopeEnterInsertEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Enter normal mode in telescope (for j/k navigation)
#[derive(Debug, Clone)]
pub struct TelescopeEnterNormalCommand;

impl CommandTrait for TelescopeEnterNormalCommand {
    fn name(&self) -> &'static str {
        "telescope_enter_normal"
    }

    fn description(&self) -> &'static str {
        "Enter normal mode for navigation"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(TelescopeEnterNormalEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
