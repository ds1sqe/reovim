//! `EventBus` events for Completion
//!
//! These events are emitted by Completion commands and handled by the runtime.
//! They replace the deprecated `DeferredAction::Completion` pattern.

use reovim_core::event_bus::Event;

/// Event emitted to trigger completion at cursor position
#[derive(Debug, Clone)]
pub struct CompletionTriggerEvent {
    /// Buffer ID where completion was triggered
    pub buffer_id: usize,
}

impl Event for CompletionTriggerEvent {
    fn priority(&self) -> u32 {
        50
    }
}

/// Event emitted to select the next completion item
#[derive(Debug, Clone)]
pub struct CompletionSelectNextEvent;

impl Event for CompletionSelectNextEvent {}

/// Event emitted to select the previous completion item
#[derive(Debug, Clone)]
pub struct CompletionSelectPrevEvent;

impl Event for CompletionSelectPrevEvent {}

/// Event emitted to confirm and insert the selected completion
#[derive(Debug, Clone)]
pub struct CompletionConfirmEvent;

impl Event for CompletionConfirmEvent {}

/// Event emitted to dismiss the completion popup
#[derive(Debug, Clone)]
pub struct CompletionDismissEvent;

impl Event for CompletionDismissEvent {}
