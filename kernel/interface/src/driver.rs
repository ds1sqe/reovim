//! Same-build execution ownership; ordinary Resource callers receive no driver.

use core::task::Context;

use crate::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lifecycle {
    Constructed,
    Starting,
    Ready,
    Closing,
    Closed,
    /// Failure does not prove that cleanup has finished.
    Failed(Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Progress {
    More,
    /// No runnable work; owned requests may still be pending.
    Idle,
    /// All work and cleanup have stopped.
    Stopped,
}

/// Assembly pairs private state with its implementation's execution table.
///
/// Advance performs one bounded turn, including startup and closing progress.
/// Close fences new work; Busy retains cleanup ownership and requires driving.
/// The owner keeps state and executable code alive until Stopped.
pub struct DriverOps<State> {
    pub status: fn(&State) -> Lifecycle,
    pub advance: fn(&mut State, &mut Context<'_>) -> Result<Progress, Error>,
    pub close: fn(&mut State) -> Result<(), Error>,
}
