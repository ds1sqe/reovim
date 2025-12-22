//! `EventBus` events for Settings Menu
//!
//! These events are emitted by Settings Menu commands and handled by the runtime.
//! They replace the deprecated `DeferredAction::SettingsMenu` pattern.

use reovim_core::event_bus::Event;

/// Event emitted to open the settings menu
#[derive(Debug, Clone)]
pub struct SettingsMenuOpenEvent;

impl Event for SettingsMenuOpenEvent {
    fn priority(&self) -> u32 {
        50 // High priority for mode changes
    }
}

/// Event emitted to close the settings menu
#[derive(Debug, Clone)]
pub struct SettingsMenuCloseEvent;

impl Event for SettingsMenuCloseEvent {
    fn priority(&self) -> u32 {
        50
    }
}

/// Event emitted to select the next item
#[derive(Debug, Clone)]
pub struct SettingsMenuSelectNextEvent;

impl Event for SettingsMenuSelectNextEvent {}

/// Event emitted to select the previous item
#[derive(Debug, Clone)]
pub struct SettingsMenuSelectPrevEvent;

impl Event for SettingsMenuSelectPrevEvent {}

/// Event emitted to toggle a boolean setting
#[derive(Debug, Clone)]
pub struct SettingsMenuToggleEvent;

impl Event for SettingsMenuToggleEvent {}

/// Event emitted to cycle to the next choice
#[derive(Debug, Clone)]
pub struct SettingsMenuCycleNextEvent;

impl Event for SettingsMenuCycleNextEvent {}

/// Event emitted to cycle to the previous choice
#[derive(Debug, Clone)]
pub struct SettingsMenuCyclePrevEvent;

impl Event for SettingsMenuCyclePrevEvent {}

/// Event emitted for quick select (1-9)
#[derive(Debug, Clone)]
pub struct SettingsMenuQuickSelectEvent {
    /// The quick select number (1-9)
    pub number: u8,
}

impl Event for SettingsMenuQuickSelectEvent {}

/// Event emitted to increment a number value
#[derive(Debug, Clone)]
pub struct SettingsMenuIncrementEvent;

impl Event for SettingsMenuIncrementEvent {}

/// Event emitted to decrement a number value
#[derive(Debug, Clone)]
pub struct SettingsMenuDecrementEvent;

impl Event for SettingsMenuDecrementEvent {}

/// Event emitted to execute an action item
#[derive(Debug, Clone)]
pub struct SettingsMenuExecuteActionEvent;

impl Event for SettingsMenuExecuteActionEvent {}
