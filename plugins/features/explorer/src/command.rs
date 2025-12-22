//! Explorer mode commands
//!
//! These commands emit `EventBus` events that are handled by the runtime.
//! The events are defined in `super::events`.

use {
    super::events::{
        ExplorerCancelInputEvent, ExplorerClearFilterEvent, ExplorerCloseEvent,
        ExplorerCloseParentEvent, ExplorerConfirmInputEvent, ExplorerCreateDirEvent,
        ExplorerCreateFileEvent, ExplorerCursorDownEvent, ExplorerCursorUpEvent, ExplorerCutEvent,
        ExplorerDeleteEvent, ExplorerExitVisualEvent, ExplorerFocusEditorEvent,
        ExplorerGoToParentEvent, ExplorerGotoFirstEvent, ExplorerGotoLastEvent,
        ExplorerInputBackspaceEvent, ExplorerInputCharEvent, ExplorerOpenNodeEvent,
        ExplorerPageDownEvent, ExplorerPageUpEvent, ExplorerPasteEvent, ExplorerRefreshEvent,
        ExplorerRenameEvent, ExplorerSelectAllEvent, ExplorerStartFilterEvent, ExplorerToggleEvent,
        ExplorerToggleHiddenEvent, ExplorerToggleNodeEvent, ExplorerToggleSelectEvent,
        ExplorerToggleSizesEvent, ExplorerVisualModeEvent, ExplorerYankEvent,
    },
    reovim_core::{
        command::traits::{CommandResult, CommandTrait, ExecutionContext},
        event_bus::DynEvent,
    },
    std::any::Any,
};

/// Move cursor up in explorer
#[derive(Debug, Clone)]
pub struct ExplorerCursorUpCommand;

impl CommandTrait for ExplorerCursorUpCommand {
    fn name(&self) -> &'static str {
        "explorer_cursor_up"
    }

    fn description(&self) -> &'static str {
        "Move cursor up in explorer"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let count = ctx.count.unwrap_or(1);
        CommandResult::EmitEvent(DynEvent::new(ExplorerCursorUpEvent { count }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Move cursor down in explorer
#[derive(Debug, Clone)]
pub struct ExplorerCursorDownCommand;

impl CommandTrait for ExplorerCursorDownCommand {
    fn name(&self) -> &'static str {
        "explorer_cursor_down"
    }

    fn description(&self) -> &'static str {
        "Move cursor down in explorer"
    }

    fn execute(&self, ctx: &mut ExecutionContext) -> CommandResult {
        let count = ctx.count.unwrap_or(1);
        CommandResult::EmitEvent(DynEvent::new(ExplorerCursorDownEvent { count }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Page up in explorer
#[derive(Debug, Clone)]
pub struct ExplorerPageUpCommand;

impl CommandTrait for ExplorerPageUpCommand {
    fn name(&self) -> &'static str {
        "explorer_page_up"
    }

    fn description(&self) -> &'static str {
        "Page up in explorer"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerPageUpEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Page down in explorer
#[derive(Debug, Clone)]
pub struct ExplorerPageDownCommand;

impl CommandTrait for ExplorerPageDownCommand {
    fn name(&self) -> &'static str {
        "explorer_page_down"
    }

    fn description(&self) -> &'static str {
        "Page down in explorer"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerPageDownEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Go to first item in explorer
#[derive(Debug, Clone)]
pub struct ExplorerGotoFirstCommand;

impl CommandTrait for ExplorerGotoFirstCommand {
    fn name(&self) -> &'static str {
        "explorer_goto_first"
    }

    fn description(&self) -> &'static str {
        "Go to first item in explorer"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerGotoFirstEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Go to last item in explorer
#[derive(Debug, Clone)]
pub struct ExplorerGotoLastCommand;

impl CommandTrait for ExplorerGotoLastCommand {
    fn name(&self) -> &'static str {
        "explorer_goto_last"
    }

    fn description(&self) -> &'static str {
        "Go to last item in explorer"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerGotoLastEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Toggle expand/collapse on current node
#[derive(Debug, Clone)]
pub struct ExplorerToggleNodeCommand;

impl CommandTrait for ExplorerToggleNodeCommand {
    fn name(&self) -> &'static str {
        "explorer_toggle_node"
    }

    fn description(&self) -> &'static str {
        "Toggle expand/collapse on current directory"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerToggleNodeEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Open file or toggle directory
#[derive(Debug, Clone)]
pub struct ExplorerOpenNodeCommand;

impl CommandTrait for ExplorerOpenNodeCommand {
    fn name(&self) -> &'static str {
        "explorer_open_node"
    }

    fn description(&self) -> &'static str {
        "Open file or toggle directory"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        tracing::info!("ExplorerOpenNodeCommand::execute() called");
        CommandResult::EmitEvent(DynEvent::new(ExplorerOpenNodeEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Close parent directory
#[derive(Debug, Clone)]
pub struct ExplorerCloseParentCommand;

impl CommandTrait for ExplorerCloseParentCommand {
    fn name(&self) -> &'static str {
        "explorer_close_parent"
    }

    fn description(&self) -> &'static str {
        "Close parent directory"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerCloseParentEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Go to parent directory
#[derive(Debug, Clone)]
pub struct ExplorerGoToParentCommand;

impl CommandTrait for ExplorerGoToParentCommand {
    fn name(&self) -> &'static str {
        "explorer_go_to_parent"
    }

    fn description(&self) -> &'static str {
        "Go to parent directory"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerGoToParentEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Refresh tree from filesystem
#[derive(Debug, Clone)]
pub struct ExplorerRefreshCommand;

impl CommandTrait for ExplorerRefreshCommand {
    fn name(&self) -> &'static str {
        "explorer_refresh"
    }

    fn description(&self) -> &'static str {
        "Refresh tree from filesystem"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerRefreshEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Toggle showing hidden files
#[derive(Debug, Clone)]
pub struct ExplorerToggleHiddenCommand;

impl CommandTrait for ExplorerToggleHiddenCommand {
    fn name(&self) -> &'static str {
        "explorer_toggle_hidden"
    }

    fn description(&self) -> &'static str {
        "Toggle showing hidden files"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerToggleHiddenEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Toggle showing file sizes
#[derive(Debug, Clone)]
pub struct ExplorerToggleSizesCommand;

impl CommandTrait for ExplorerToggleSizesCommand {
    fn name(&self) -> &'static str {
        "explorer_toggle_sizes"
    }

    fn description(&self) -> &'static str {
        "Toggle showing file sizes"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerToggleSizesEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Yank (copy) current item to clipboard
#[derive(Debug, Clone)]
pub struct ExplorerYankCommand;

impl CommandTrait for ExplorerYankCommand {
    fn name(&self) -> &'static str {
        "explorer_yank"
    }

    fn description(&self) -> &'static str {
        "Yank (copy) current item"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerYankEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Cut current item to clipboard
#[derive(Debug, Clone)]
pub struct ExplorerCutCommand;

impl CommandTrait for ExplorerCutCommand {
    fn name(&self) -> &'static str {
        "explorer_cut"
    }

    fn description(&self) -> &'static str {
        "Cut current item"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerCutEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Paste from clipboard
#[derive(Debug, Clone)]
pub struct ExplorerPasteCommand;

impl CommandTrait for ExplorerPasteCommand {
    fn name(&self) -> &'static str {
        "explorer_paste"
    }

    fn description(&self) -> &'static str {
        "Paste from clipboard"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerPasteEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Enter visual selection mode
#[derive(Debug, Clone)]
pub struct ExplorerVisualModeCommand;

impl CommandTrait for ExplorerVisualModeCommand {
    fn name(&self) -> &'static str {
        "explorer_visual_mode"
    }

    fn description(&self) -> &'static str {
        "Enter visual selection mode"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerVisualModeEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Toggle selection of current item
#[derive(Debug, Clone)]
pub struct ExplorerToggleSelectCommand;

impl CommandTrait for ExplorerToggleSelectCommand {
    fn name(&self) -> &'static str {
        "explorer_toggle_select"
    }

    fn description(&self) -> &'static str {
        "Toggle selection"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerToggleSelectEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Select all visible items
#[derive(Debug, Clone)]
pub struct ExplorerSelectAllCommand;

impl CommandTrait for ExplorerSelectAllCommand {
    fn name(&self) -> &'static str {
        "explorer_select_all"
    }

    fn description(&self) -> &'static str {
        "Select all items"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerSelectAllEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Exit visual selection mode
#[derive(Debug, Clone)]
pub struct ExplorerExitVisualCommand;

impl CommandTrait for ExplorerExitVisualCommand {
    fn name(&self) -> &'static str {
        "explorer_exit_visual"
    }

    fn description(&self) -> &'static str {
        "Exit visual mode"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerExitVisualEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Close explorer
#[derive(Debug, Clone)]
pub struct ExplorerCloseCommand;

impl CommandTrait for ExplorerCloseCommand {
    fn name(&self) -> &'static str {
        "explorer_close"
    }

    fn description(&self) -> &'static str {
        "Close explorer"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerCloseEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Focus editor window
#[derive(Debug, Clone)]
pub struct ExplorerFocusEditorCommand;

impl CommandTrait for ExplorerFocusEditorCommand {
    fn name(&self) -> &'static str {
        "explorer_focus_editor"
    }

    fn description(&self) -> &'static str {
        "Focus editor window"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerFocusEditorEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Toggle explorer visibility
#[derive(Debug, Clone)]
pub struct ToggleExplorerCommand;

impl CommandTrait for ToggleExplorerCommand {
    fn name(&self) -> &'static str {
        "toggle_explorer"
    }

    fn description(&self) -> &'static str {
        "Toggle explorer visibility"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        tracing::info!("ToggleExplorerCommand::execute() called");
        CommandResult::EmitEvent(DynEvent::new(ExplorerToggleEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Create a new file
#[derive(Debug, Clone)]
pub struct ExplorerCreateFileCommand;

impl CommandTrait for ExplorerCreateFileCommand {
    fn name(&self) -> &'static str {
        "explorer_create_file"
    }

    fn description(&self) -> &'static str {
        "Create a new file"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerCreateFileEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Create a new directory
#[derive(Debug, Clone)]
pub struct ExplorerCreateDirCommand;

impl CommandTrait for ExplorerCreateDirCommand {
    fn name(&self) -> &'static str {
        "explorer_create_dir"
    }

    fn description(&self) -> &'static str {
        "Create a new directory"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerCreateDirEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Rename current item
#[derive(Debug, Clone)]
pub struct ExplorerRenameCommand;

impl CommandTrait for ExplorerRenameCommand {
    fn name(&self) -> &'static str {
        "explorer_rename"
    }

    fn description(&self) -> &'static str {
        "Rename current file or directory"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerRenameEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Delete current item
#[derive(Debug, Clone)]
pub struct ExplorerDeleteCommand;

impl CommandTrait for ExplorerDeleteCommand {
    fn name(&self) -> &'static str {
        "explorer_delete"
    }

    fn description(&self) -> &'static str {
        "Delete current file or directory"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerDeleteEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Start filter mode
#[derive(Debug, Clone)]
pub struct ExplorerFilterCommand;

impl CommandTrait for ExplorerFilterCommand {
    fn name(&self) -> &'static str {
        "explorer_filter"
    }

    fn description(&self) -> &'static str {
        "Filter files by name"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerStartFilterEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Clear filter
#[derive(Debug, Clone)]
pub struct ExplorerClearFilterCommand;

impl CommandTrait for ExplorerClearFilterCommand {
    fn name(&self) -> &'static str {
        "explorer_clear_filter"
    }

    fn description(&self) -> &'static str {
        "Clear file filter"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerClearFilterEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Input character in explorer input mode
///
/// Used dynamically when a character is typed in explorer input mode.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ExplorerInputCharCommand {
    c: char,
}

#[allow(dead_code)]
impl ExplorerInputCharCommand {
    #[must_use]
    pub const fn new(c: char) -> Self {
        Self { c }
    }
}

impl CommandTrait for ExplorerInputCharCommand {
    fn name(&self) -> &'static str {
        "explorer_input_char"
    }

    fn description(&self) -> &'static str {
        "Input character in explorer"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerInputCharEvent { c: self.c }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Confirm input in explorer input mode
#[derive(Debug, Clone)]
pub struct ExplorerConfirmInputCommand;

impl CommandTrait for ExplorerConfirmInputCommand {
    fn name(&self) -> &'static str {
        "explorer_confirm_input"
    }

    fn description(&self) -> &'static str {
        "Confirm input in explorer"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        // Input will be retrieved from explorer state by the handler
        CommandResult::EmitEvent(DynEvent::new(ExplorerConfirmInputEvent {
            input: String::new(),
        }))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Cancel input in explorer input mode
#[derive(Debug, Clone)]
pub struct ExplorerCancelInputCommand;

impl CommandTrait for ExplorerCancelInputCommand {
    fn name(&self) -> &'static str {
        "explorer_cancel_input"
    }

    fn description(&self) -> &'static str {
        "Cancel input in explorer"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerCancelInputEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Backspace in explorer input mode
#[derive(Debug, Clone)]
pub struct ExplorerInputBackspaceCommand;

impl CommandTrait for ExplorerInputBackspaceCommand {
    fn name(&self) -> &'static str {
        "explorer_input_backspace"
    }

    fn description(&self) -> &'static str {
        "Backspace in explorer input"
    }

    fn execute(&self, _ctx: &mut ExecutionContext) -> CommandResult {
        CommandResult::EmitEvent(DynEvent::new(ExplorerInputBackspaceEvent))
    }

    fn clone_box(&self) -> Box<dyn CommandTrait> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
