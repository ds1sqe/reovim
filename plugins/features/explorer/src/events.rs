//! `EventBus` events for Explorer
//!
//! These events are emitted by Explorer commands and handled by the runtime.
//! They replace the deprecated `DeferredAction::Explorer` pattern.

use reovim_core::event_bus::Event;

/// Event emitted to move cursor up
#[derive(Debug, Clone)]
pub struct ExplorerCursorUpEvent {
    pub count: usize,
}

impl Event for ExplorerCursorUpEvent {}

/// Event emitted to move cursor down
#[derive(Debug, Clone)]
pub struct ExplorerCursorDownEvent {
    pub count: usize,
}

impl Event for ExplorerCursorDownEvent {}

/// Event emitted to page up
#[derive(Debug, Clone)]
pub struct ExplorerPageUpEvent;

impl Event for ExplorerPageUpEvent {}

/// Event emitted to page down
#[derive(Debug, Clone)]
pub struct ExplorerPageDownEvent;

impl Event for ExplorerPageDownEvent {}

/// Event emitted to go to first item
#[derive(Debug, Clone)]
pub struct ExplorerGotoFirstEvent;

impl Event for ExplorerGotoFirstEvent {}

/// Event emitted to go to last item
#[derive(Debug, Clone)]
pub struct ExplorerGotoLastEvent;

impl Event for ExplorerGotoLastEvent {}

/// Event emitted to toggle expand/collapse on current node
#[derive(Debug, Clone)]
pub struct ExplorerToggleNodeEvent;

impl Event for ExplorerToggleNodeEvent {}

/// Event emitted to open file or toggle directory
#[derive(Debug, Clone)]
pub struct ExplorerOpenNodeEvent;

impl Event for ExplorerOpenNodeEvent {}

/// Event emitted to close parent directory
#[derive(Debug, Clone)]
pub struct ExplorerCloseParentEvent;

impl Event for ExplorerCloseParentEvent {}

/// Event emitted to go to parent directory
#[derive(Debug, Clone)]
pub struct ExplorerGoToParentEvent;

impl Event for ExplorerGoToParentEvent {}

/// Event emitted to refresh tree from filesystem
#[derive(Debug, Clone)]
pub struct ExplorerRefreshEvent;

impl Event for ExplorerRefreshEvent {}

/// Event emitted to toggle showing hidden files
#[derive(Debug, Clone)]
pub struct ExplorerToggleHiddenEvent;

impl Event for ExplorerToggleHiddenEvent {}

/// Event emitted to toggle showing file sizes
#[derive(Debug, Clone)]
pub struct ExplorerToggleSizesEvent;

impl Event for ExplorerToggleSizesEvent {}

/// Event emitted to yank (copy) current item to clipboard
#[derive(Debug, Clone)]
pub struct ExplorerYankEvent;

impl Event for ExplorerYankEvent {}

/// Event emitted to cut current item to clipboard
#[derive(Debug, Clone)]
pub struct ExplorerCutEvent;

impl Event for ExplorerCutEvent {}

/// Event emitted to paste from clipboard
#[derive(Debug, Clone)]
pub struct ExplorerPasteEvent;

impl Event for ExplorerPasteEvent {}

/// Event emitted to close explorer (switch to editor)
#[derive(Debug, Clone)]
pub struct ExplorerCloseEvent;

impl Event for ExplorerCloseEvent {}

/// Event emitted to focus editor window
#[derive(Debug, Clone)]
pub struct ExplorerFocusEditorEvent;

impl Event for ExplorerFocusEditorEvent {}

/// Event emitted to toggle explorer visibility
#[derive(Debug, Clone)]
pub struct ExplorerToggleEvent;

impl Event for ExplorerToggleEvent {}

/// Event emitted to start creating a new file
#[derive(Debug, Clone)]
pub struct ExplorerCreateFileEvent;

impl Event for ExplorerCreateFileEvent {}

/// Event emitted to start creating a new directory
#[derive(Debug, Clone)]
pub struct ExplorerCreateDirEvent;

impl Event for ExplorerCreateDirEvent {}

/// Event emitted to start renaming current item
#[derive(Debug, Clone)]
pub struct ExplorerRenameEvent;

impl Event for ExplorerRenameEvent {}

/// Event emitted to delete current item
#[derive(Debug, Clone)]
pub struct ExplorerDeleteEvent;

impl Event for ExplorerDeleteEvent {}

/// Event emitted to start filtering
#[derive(Debug, Clone)]
pub struct ExplorerStartFilterEvent;

impl Event for ExplorerStartFilterEvent {}

/// Event emitted to clear the current filter
#[derive(Debug, Clone)]
pub struct ExplorerClearFilterEvent;

impl Event for ExplorerClearFilterEvent {}

/// Event emitted to confirm pending operation
#[derive(Debug, Clone)]
pub struct ExplorerConfirmInputEvent {
    pub input: String,
}

impl Event for ExplorerConfirmInputEvent {}

/// Event emitted to cancel pending operation
#[derive(Debug, Clone)]
pub struct ExplorerCancelInputEvent;

impl Event for ExplorerCancelInputEvent {}

/// Event emitted to handle character input during input mode
#[derive(Debug, Clone)]
pub struct ExplorerInputCharEvent {
    pub c: char,
}

impl Event for ExplorerInputCharEvent {}

/// Event emitted to handle backspace during input mode
#[derive(Debug, Clone)]
pub struct ExplorerInputBackspaceEvent;

impl Event for ExplorerInputBackspaceEvent {}

/// Event emitted to enter visual selection mode
#[derive(Debug, Clone)]
pub struct ExplorerVisualModeEvent;

impl Event for ExplorerVisualModeEvent {}

/// Event emitted to toggle selection of current item
#[derive(Debug, Clone)]
pub struct ExplorerToggleSelectEvent;

impl Event for ExplorerToggleSelectEvent {}

/// Event emitted to select all visible items
#[derive(Debug, Clone)]
pub struct ExplorerSelectAllEvent;

impl Event for ExplorerSelectAllEvent {}

/// Event emitted to exit visual selection mode
#[derive(Debug, Clone)]
pub struct ExplorerExitVisualEvent;

impl Event for ExplorerExitVisualEvent {}
