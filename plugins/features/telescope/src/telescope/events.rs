//! `EventBus` events for Telescope
//!
//! These events are emitted by Telescope commands and handled by the runtime.
//! They replace the deprecated `DeferredAction::Telescope` pattern.

use reovim_core::event_bus::Event;

/// Event emitted to open telescope with a specific picker
#[derive(Debug, Clone)]
pub struct TelescopeOpenEvent {
    pub picker: String,
}

impl Event for TelescopeOpenEvent {}

/// Event emitted to insert a character into the query
#[derive(Debug, Clone)]
pub struct TelescopeInsertCharEvent {
    pub c: char,
}

impl Event for TelescopeInsertCharEvent {}

/// Event emitted to delete character from query (backspace)
#[derive(Debug, Clone)]
pub struct TelescopeBackspaceEvent;

impl Event for TelescopeBackspaceEvent {}

/// Event emitted to move cursor left in query
#[derive(Debug, Clone)]
pub struct TelescopeCursorLeftEvent;

impl Event for TelescopeCursorLeftEvent {}

/// Event emitted to move cursor right in query
#[derive(Debug, Clone)]
pub struct TelescopeCursorRightEvent;

impl Event for TelescopeCursorRightEvent {}

/// Event emitted to select next item
#[derive(Debug, Clone)]
pub struct TelescopeSelectNextEvent;

impl Event for TelescopeSelectNextEvent {}

/// Event emitted to select previous item
#[derive(Debug, Clone)]
pub struct TelescopeSelectPrevEvent;

impl Event for TelescopeSelectPrevEvent {}

/// Event emitted to page down
#[derive(Debug, Clone)]
pub struct TelescopePageDownEvent;

impl Event for TelescopePageDownEvent {}

/// Event emitted to page up
#[derive(Debug, Clone)]
pub struct TelescopePageUpEvent;

impl Event for TelescopePageUpEvent {}

/// Event emitted to go to first item
#[derive(Debug, Clone)]
pub struct TelescopeGotoFirstEvent;

impl Event for TelescopeGotoFirstEvent {}

/// Event emitted to go to last item
#[derive(Debug, Clone)]
pub struct TelescopeGotoLastEvent;

impl Event for TelescopeGotoLastEvent {}

/// Event emitted to confirm selection
#[derive(Debug, Clone)]
pub struct TelescopeConfirmEvent;

impl Event for TelescopeConfirmEvent {}

/// Event emitted to close telescope
#[derive(Debug, Clone)]
pub struct TelescopeCloseEvent;

impl Event for TelescopeCloseEvent {}

/// Event emitted to enter insert mode (for typing query)
#[derive(Debug, Clone)]
pub struct TelescopeEnterInsertEvent;

impl Event for TelescopeEnterInsertEvent {}

/// Event emitted to enter normal mode (for j/k navigation)
#[derive(Debug, Clone)]
pub struct TelescopeEnterNormalEvent;

impl Event for TelescopeEnterNormalEvent {}
