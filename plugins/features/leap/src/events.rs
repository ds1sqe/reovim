//! Leap events for the event bus
//!
//! These events are used by the leap plugin to communicate through
//! the type-erased event bus system.

use reovim_core::{event_bus::Event, modd::OperatorType, screen::Position};

use super::state::LeapDirection;

/// Event emitted when leap mode starts
#[derive(Debug, Clone)]
pub struct LeapStartEvent {
    /// Search direction
    pub direction: LeapDirection,
    /// Pending operator (if from operator-pending mode)
    pub operator: Option<OperatorType>,
    /// Operator count
    pub count: Option<usize>,
}

impl Event for LeapStartEvent {
    fn priority(&self) -> u32 {
        50 // High priority for mode changes
    }
}

/// Event emitted when first character is entered
#[derive(Debug, Clone)]
pub struct LeapFirstCharEvent {
    /// The first character entered
    pub char: char,
}

impl Event for LeapFirstCharEvent {}

/// Event emitted when second character is entered
#[derive(Debug, Clone)]
pub struct LeapSecondCharEvent {
    /// The second character entered
    pub char: char,
}

impl Event for LeapSecondCharEvent {}

/// Event emitted when a label is selected to jump
#[derive(Debug, Clone)]
pub struct LeapSelectLabelEvent {
    /// The selected label
    pub label: String,
}

impl Event for LeapSelectLabelEvent {}

/// Event emitted when leap mode is cancelled
#[derive(Debug, Clone)]
pub struct LeapCancelEvent;

impl Event for LeapCancelEvent {}

/// Event emitted when a leap jump occurs
///
/// This is emitted after a successful jump so other plugins
/// can react to the cursor movement.
#[derive(Debug, Clone)]
pub struct LeapJumpEvent {
    /// Source position before jump
    pub from: Position,
    /// Target position after jump
    pub to: Position,
    /// Direction of the leap
    pub direction: LeapDirection,
}

impl Event for LeapJumpEvent {}

/// Event emitted when matches are found
#[derive(Debug, Clone)]
pub struct LeapMatchesFoundEvent {
    /// Number of matches found
    pub match_count: usize,
    /// The search pattern
    pub pattern: String,
}

impl Event for LeapMatchesFoundEvent {}
