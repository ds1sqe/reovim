//! Same-build execution ownership; ordinary Resource callers receive no driver.
//!
//! ```
//! use reovim_kernel_interface::driver::{Lifecycle, Progress};
//! let lifecycle = Lifecycle::Closing;
//! let progress = Progress::Idle;
//! assert_ne!(lifecycle, Lifecycle::Closed);
//! assert_ne!(progress, Progress::Stopped);
//! ```

use core::task::Context;

use crate::Error;

/// The implementation's lifecycle, observed by its execution owner.
///
/// ```
/// use reovim_kernel_interface::{Error, driver::Lifecycle};
/// let status = Lifecycle::Failed(Error::PROVIDER);
/// assert!(matches!(status, Lifecycle::Failed(Error::PROVIDER)));
/// assert_ne!(status, Lifecycle::Closed);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lifecycle {
    /// Inert state exists; startup has not begun.
    /// ```
    /// use reovim_kernel_interface::driver::Lifecycle;
    /// assert_ne!(Lifecycle::Constructed, Lifecycle::Ready);
    /// ```
    Constructed,
    /// Initialization is progressing; contributions are not ready for use.
    /// ```
    /// use reovim_kernel_interface::driver::Lifecycle;
    /// let status = Lifecycle::Starting;
    /// assert!(!matches!(status, Lifecycle::Ready));
    /// ```
    Starting,
    /// The implementation reports readiness for its declared work.
    /// ```
    /// use reovim_kernel_interface::driver::Lifecycle;
    /// fn ready(status: Lifecycle) -> bool {
    ///     matches!(status, Lifecycle::Ready)
    /// }
    /// ```
    Ready,
    /// New work is fenced while the owner continues cleanup.
    /// ```
    /// use reovim_kernel_interface::driver::Lifecycle;
    /// assert_ne!(Lifecycle::Closing, Lifecycle::Closed);
    /// ```
    Closing,
    /// Lifecycle teardown has completed.
    /// ```
    /// use reovim_kernel_interface::driver::Lifecycle;
    /// fn cleanup_finished(status: Lifecycle) -> bool {
    ///     matches!(status, Lifecycle::Closed)
    /// }
    /// ```
    Closed,
    /// Failure does not prove that cleanup has finished.
    /// ```
    /// use reovim_kernel_interface::{Error, driver::Lifecycle};
    /// let failed = Lifecycle::Failed(Error::UNAVAILABLE);
    /// assert_ne!(failed, Lifecycle::Closed);
    /// ```
    Failed(Error),
}

/// Whether the execution owner should keep driving or wait for a wakeup.
///
/// ```
/// use reovim_kernel_interface::driver::Progress;
/// let progress = Progress::Idle;
/// assert!(!matches!(progress, Progress::Stopped));
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Progress {
    /// Runnable work remains; schedule another bounded turn.
    /// ```
    /// use reovim_kernel_interface::driver::Progress;
    /// fn schedule_again(progress: Progress) -> bool {
    ///     matches!(progress, Progress::More)
    /// }
    /// ```
    More,
    /// No runnable work; owned requests may still be pending.
    /// ```
    /// use reovim_kernel_interface::driver::Progress;
    /// assert_ne!(Progress::Idle, Progress::Stopped);
    /// ```
    Idle,
    /// All work and cleanup have stopped.
    /// ```
    /// use reovim_kernel_interface::driver::Progress;
    /// fn finished(progress: Progress) -> bool {
    ///     matches!(progress, Progress::Stopped)
    /// }
    /// ```
    Stopped,
}

/// Assembly pairs private state with its implementation's execution table.
///
/// Advance performs one bounded turn, including startup and closing progress.
/// Close fences new work; Busy retains cleanup ownership and requires driving.
/// The owner keeps state and executable code alive until Stopped.
///
/// ```
/// use reovim_kernel_interface::driver::{DriverOps, Lifecycle, Progress};
/// let driver = DriverOps {
///     status: |closed: &bool| {
///         if *closed { Lifecycle::Closed } else { Lifecycle::Ready }
///     },
///     advance: |closed, _| {
///         Ok(if *closed { Progress::Stopped } else { Progress::Idle })
///     },
///     close: |closed| { *closed = true; Ok(()) },
/// };
/// let mut closed = false;
/// assert_eq!((driver.status)(&closed), Lifecycle::Ready);
/// (driver.close)(&mut closed).unwrap();
/// assert_eq!((driver.status)(&closed), Lifecycle::Closed);
/// ```
pub struct DriverOps<State> {
    /// Observes lifecycle without transferring or driving state.
    /// ```
    /// use reovim_kernel_interface::driver::{DriverOps, Lifecycle};
    /// fn ready<State>(driver: &DriverOps<State>, state: &State) -> bool {
    ///     (driver.status)(state) == Lifecycle::Ready
    /// }
    /// ```
    pub status: fn(&State) -> Lifecycle,
    /// Runs a bounded turn; an error does not release owned cleanup.
    /// ```
    /// use core::task::Context;
    /// use reovim_kernel_interface::{Error, driver::{DriverOps, Progress}};
    /// fn step<State>(
    ///     driver: &DriverOps<State>,
    ///     state: &mut State,
    ///     wake: &mut Context<'_>,
    /// ) -> Result<Progress, Error> {
    ///     (driver.advance)(state, wake)
    /// }
    /// ```
    pub advance: fn(&mut State, &mut Context<'_>) -> Result<Progress, Error>,
    /// Begins or continues close; Busy requires retaining and driving state.
    /// ```
    /// use reovim_kernel_interface::{Error, driver::DriverOps};
    /// fn close<State>(
    ///     driver: &DriverOps<State>, state: &mut State,
    /// ) -> Result<(), Error> {
    ///     (driver.close)(state)
    /// }
    /// ```
    pub close: fn(&mut State) -> Result<(), Error>,
}
