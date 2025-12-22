//! Telescope fuzzy finder commands (unified command-event types)

use reovim_core::{
    command::traits::*,
    declare_event_command,
    event_bus::{DynEvent, Event},
};

// === Core Events (unified types) ===

/// Open telescope with a specific picker
#[derive(Debug, Clone)]
pub struct TelescopeOpen {
    pub picker: String,
}

impl TelescopeOpen {
    /// Create event for a specific picker
    #[must_use]
    pub fn new(picker: impl Into<String>) -> Self {
        Self {
            picker: picker.into(),
        }
    }
}

impl Event for TelescopeOpen {
    fn priority(&self) -> u32 {
        100
    }
}

/// Insert a character into the query
#[derive(Debug, Clone, Copy)]
pub struct TelescopeInsertChar {
    pub c: char,
}

impl TelescopeInsertChar {
    /// Create event for specific character
    #[must_use]
    pub const fn new(c: char) -> Self {
        Self { c }
    }
}

impl Event for TelescopeInsertChar {
    fn priority(&self) -> u32 {
        100
    }
}

// === Navigation & Control (using macro) ===

declare_event_command! {
    TelescopeBackspace,
    id: "telescope_backspace",
    description: "Delete character from query (backspace)",
}

declare_event_command! {
    TelescopeCursorLeft,
    id: "telescope_cursor_left",
    description: "Move cursor left in query",
}

declare_event_command! {
    TelescopeCursorRight,
    id: "telescope_cursor_right",
    description: "Move cursor right in query",
}

declare_event_command! {
    TelescopeSelectNext,
    id: "telescope_select_next",
    description: "Select next item",
}

declare_event_command! {
    TelescopeSelectPrev,
    id: "telescope_select_prev",
    description: "Select previous item",
}

declare_event_command! {
    TelescopePageDown,
    id: "telescope_page_down",
    description: "Page down",
}

declare_event_command! {
    TelescopePageUp,
    id: "telescope_page_up",
    description: "Page up",
}

declare_event_command! {
    TelescopeGotoFirst,
    id: "telescope_goto_first",
    description: "Go to first item",
}

declare_event_command! {
    TelescopeGotoLast,
    id: "telescope_goto_last",
    description: "Go to last item",
}

declare_event_command! {
    TelescopeConfirm,
    id: "telescope_confirm",
    description: "Confirm selection",
}

declare_event_command! {
    TelescopeClose,
    id: "telescope_close",
    description: "Close telescope",
}

declare_event_command! {
    TelescopeEnterInsert,
    id: "telescope_enter_insert",
    description: "Enter insert mode (for typing query)",
}

declare_event_command! {
    TelescopeEnterNormal,
    id: "telescope_enter_normal",
    description: "Enter normal mode (for j/k navigation)",
}

// === Picker Commands ===
// These are separate command types that emit TelescopeOpen with different picker names

/// Open telescope find files picker
#[derive(Debug, Clone, Copy)]
pub struct TelescopeFindFiles;

impl CommandTrait for TelescopeFindFiles {
    fn name(&self) -> &'static str {
        "telescope_find_files"
    }

    fn description(&self) -> &'static str {
        "Find files with fuzzy search"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpen::new("files")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open telescope buffers picker
#[derive(Debug, Clone, Copy)]
pub struct TelescopeFindBuffers;

impl CommandTrait for TelescopeFindBuffers {
    fn name(&self) -> &'static str {
        "telescope_find_buffers"
    }

    fn description(&self) -> &'static str {
        "Find open buffers"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpen::new("buffers")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open telescope live grep picker
#[derive(Debug, Clone, Copy)]
pub struct TelescopeLiveGrep;

impl CommandTrait for TelescopeLiveGrep {
    fn name(&self) -> &'static str {
        "telescope_live_grep"
    }

    fn description(&self) -> &'static str {
        "Search text in files"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpen::new("grep")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open telescope recent files picker
#[derive(Debug, Clone, Copy)]
pub struct TelescopeFindRecent;

impl CommandTrait for TelescopeFindRecent {
    fn name(&self) -> &'static str {
        "telescope_find_recent"
    }

    fn description(&self) -> &'static str {
        "Find recent files"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpen::new("recent")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open telescope help picker
#[derive(Debug, Clone, Copy)]
pub struct TelescopeHelp;

impl CommandTrait for TelescopeHelp {
    fn name(&self) -> &'static str {
        "telescope_help"
    }

    fn description(&self) -> &'static str {
        "Search help tags"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpen::new("help")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open telescope commands picker
#[derive(Debug, Clone, Copy)]
pub struct TelescopeCommands;

impl CommandTrait for TelescopeCommands {
    fn name(&self) -> &'static str {
        "telescope_commands"
    }

    fn description(&self) -> &'static str {
        "Search available commands"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpen::new("commands")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open telescope keymaps picker
#[derive(Debug, Clone, Copy)]
pub struct TelescopeKeymaps;

impl CommandTrait for TelescopeKeymaps {
    fn name(&self) -> &'static str {
        "telescope_keymaps"
    }

    fn description(&self) -> &'static str {
        "Search keybindings"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpen::new("keymaps")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open telescope themes picker
#[derive(Debug, Clone, Copy)]
pub struct TelescopeThemes;

impl CommandTrait for TelescopeThemes {
    fn name(&self) -> &'static str {
        "telescope_themes"
    }

    fn description(&self) -> &'static str {
        "Search available themes"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpen::new("themes")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open telescope profiles picker  
#[derive(Debug, Clone, Copy)]
pub struct TelescopeProfiles;

impl CommandTrait for TelescopeProfiles {
    fn name(&self) -> &'static str {
        "telescope_profiles"
    }

    fn description(&self) -> &'static str {
        "Search profiles"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(TelescopeOpen::new("profiles")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
