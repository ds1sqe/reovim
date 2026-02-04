//! Microscope fuzzy finder commands (unified command-event types)

use reovim_core::{
    command::traits::*,
    declare_event_command,
    event_bus::{DynEvent, Event},
};

// === Core Events (unified types) ===

/// Open microscope with a specific picker
#[derive(Debug, Clone)]
pub struct MicroscopeOpen {
    pub picker: String,
}

impl MicroscopeOpen {
    /// Create event for a specific picker
    #[must_use]
    pub fn new(picker: impl Into<String>) -> Self {
        Self {
            picker: picker.into(),
        }
    }
}

impl Event for MicroscopeOpen {
    fn priority(&self) -> u32 {
        100
    }
}

/// Insert a character into the query
#[derive(Debug, Clone, Copy)]
pub struct MicroscopeInsertChar {
    pub c: char,
}

impl MicroscopeInsertChar {
    /// Create event for specific character
    #[must_use]
    pub const fn new(c: char) -> Self {
        Self { c }
    }
}

impl Event for MicroscopeInsertChar {
    fn priority(&self) -> u32 {
        100
    }
}

// === Navigation & Control (using macro) ===

declare_event_command! {
    MicroscopeBackspace,
    id: "microscope_backspace",
    description: "Delete character from query (backspace)",
}

declare_event_command! {
    MicroscopeCursorLeft,
    id: "microscope_cursor_left",
    description: "Move cursor left in query",
}

declare_event_command! {
    MicroscopeCursorRight,
    id: "microscope_cursor_right",
    description: "Move cursor right in query",
}

declare_event_command! {
    MicroscopeSelectNext,
    id: "microscope_select_next",
    description: "Select next item",
}

declare_event_command! {
    MicroscopeSelectPrev,
    id: "microscope_select_prev",
    description: "Select previous item",
}

declare_event_command! {
    MicroscopePageDown,
    id: "microscope_page_down",
    description: "Page down",
}

declare_event_command! {
    MicroscopePageUp,
    id: "microscope_page_up",
    description: "Page up",
}

declare_event_command! {
    MicroscopeGotoFirst,
    id: "microscope_goto_first",
    description: "Go to first item",
}

declare_event_command! {
    MicroscopeGotoLast,
    id: "microscope_goto_last",
    description: "Go to last item",
}

declare_event_command! {
    MicroscopeConfirm,
    id: "microscope_confirm",
    description: "Confirm selection",
}

declare_event_command! {
    MicroscopeClose,
    id: "microscope_close",
    description: "Close microscope",
}

declare_event_command! {
    MicroscopeEnterInsert,
    id: "microscope_enter_insert",
    description: "Enter insert mode (for typing query)",
}

declare_event_command! {
    MicroscopeEnterNormal,
    id: "microscope_enter_normal",
    description: "Enter normal mode (for j/k navigation)",
}

// === Normal Mode Prompt Commands ===

declare_event_command! {
    MicroscopeWordForward,
    id: "microscope_word_forward",
    description: "Move cursor forward one word in query",
}

declare_event_command! {
    MicroscopeWordBackward,
    id: "microscope_word_backward",
    description: "Move cursor backward one word in query",
}

declare_event_command! {
    MicroscopeCursorStart,
    id: "microscope_cursor_start",
    description: "Move cursor to start of query",
}

declare_event_command! {
    MicroscopeCursorEnd,
    id: "microscope_cursor_end",
    description: "Move cursor to end of query",
}

declare_event_command! {
    MicroscopeClearQuery,
    id: "microscope_clear_query",
    description: "Clear the search query",
}

declare_event_command! {
    MicroscopeDeleteWord,
    id: "microscope_delete_word",
    description: "Delete word before cursor",
}

// === Picker Commands ===
// These are separate command types that emit MicroscopeOpen with different picker names

/// Open microscope find files picker
#[derive(Debug, Clone, Copy)]
pub struct MicroscopeFindFiles;

impl CommandTrait for MicroscopeFindFiles {
    fn name(&self) -> &'static str {
        "microscope_find_files"
    }

    fn description(&self) -> &'static str {
        "Find files with fuzzy search"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(MicroscopeOpen::new("files")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open microscope buffers picker
#[derive(Debug, Clone, Copy)]
pub struct MicroscopeFindBuffers;

impl CommandTrait for MicroscopeFindBuffers {
    fn name(&self) -> &'static str {
        "microscope_find_buffers"
    }

    fn description(&self) -> &'static str {
        "Find open buffers"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(MicroscopeOpen::new("buffers")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open microscope live grep picker
#[derive(Debug, Clone, Copy)]
pub struct MicroscopeLiveGrep;

impl CommandTrait for MicroscopeLiveGrep {
    fn name(&self) -> &'static str {
        "microscope_live_grep"
    }

    fn description(&self) -> &'static str {
        "Search text in files"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(MicroscopeOpen::new("grep")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open microscope recent files picker
#[derive(Debug, Clone, Copy)]
pub struct MicroscopeFindRecent;

impl CommandTrait for MicroscopeFindRecent {
    fn name(&self) -> &'static str {
        "microscope_find_recent"
    }

    fn description(&self) -> &'static str {
        "Find recent files"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(MicroscopeOpen::new("recent")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open microscope help picker
#[derive(Debug, Clone, Copy)]
pub struct MicroscopeHelp;

impl CommandTrait for MicroscopeHelp {
    fn name(&self) -> &'static str {
        "microscope_help"
    }

    fn description(&self) -> &'static str {
        "Search help tags"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(MicroscopeOpen::new("help")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open microscope commands picker
#[derive(Debug, Clone, Copy)]
pub struct MicroscopeCommands;

impl CommandTrait for MicroscopeCommands {
    fn name(&self) -> &'static str {
        "microscope_commands"
    }

    fn description(&self) -> &'static str {
        "Search available commands"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(MicroscopeOpen::new("commands")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open microscope keymaps picker
#[derive(Debug, Clone, Copy)]
pub struct MicroscopeKeymaps;

impl CommandTrait for MicroscopeKeymaps {
    fn name(&self) -> &'static str {
        "microscope_keymaps"
    }

    fn description(&self) -> &'static str {
        "Search keybindings"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(MicroscopeOpen::new("keymaps")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open microscope themes picker
#[derive(Debug, Clone, Copy)]
pub struct MicroscopeThemes;

impl CommandTrait for MicroscopeThemes {
    fn name(&self) -> &'static str {
        "microscope_themes"
    }

    fn description(&self) -> &'static str {
        "Search available themes"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(MicroscopeOpen::new("themes")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Open microscope profiles picker  
#[derive(Debug, Clone, Copy)]
pub struct MicroscopeProfiles;

impl CommandTrait for MicroscopeProfiles {
    fn name(&self) -> &'static str {
        "microscope_profiles"
    }

    fn description(&self) -> &'static str {
        "Search profiles"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(MicroscopeOpen::new("profiles")))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(*self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
