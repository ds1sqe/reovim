//! Client-owned view correspondence and retained input, without hosted I/O.

use crate::{
    context::Window,
    custody::{RetainedInput, Submitted},
    input::{Action, Input},
    viewport,
};
use alloc::{collections::VecDeque, format, string::String};
use reovim_kernel_interface::buffer::BufferState;

pub struct View {
    pub current: Option<Window>,
    pub cursor: usize,
    pub pending_cursor: Option<u64>,
    pub pending: bool,
    pub halted: bool,
    pub status: String,
    pub queued: VecDeque<Input>,
    pub submitted: Option<Submitted>,
    pub receipt: Option<u64>,
    pub top: usize,
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Unavailable,
    InvalidAction,
    InvalidCursor,
}

impl core::fmt::Display for Error {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(out, "Client view: {self:?}")
    }
}
impl core::error::Error for Error {}

impl Default for View {
    fn default() -> Self {
        Self::new()
    }
}

impl View {
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: None,
            cursor: 0,
            pending_cursor: None,
            pending: true,
            halted: false,
            status: "Attaching to Source...".into(),
            queued: VecDeque::new(),
            submitted: None,
            receipt: None,
            top: 0,
            visible: false,
        }
    }

    /// Retains a window-qualified action before handing it to hosted I/O.
    /// # Errors
    /// Fences unresolved input and rejects invalid or foreign window ranges.
    pub fn begin(&mut self, action: &Action) -> Result<(), Error> {
        if self.pending
            || self.halted
            || !self.visible
            || self.submitted.is_some()
            || self.receipt.is_some()
        {
            return Err(Error::Unavailable);
        }
        let window = self.current.as_ref().ok_or(Error::Unavailable)?;
        let cursor = action.cursor_after().ok_or(Error::InvalidAction)?;
        if !qualified(action, window) {
            return Err(Error::InvalidAction);
        }
        if !matches!(action, Action::Observe(_)) {
            self.pending_cursor = Some(cursor);
            self.submitted = Some(Submitted {
                basis: window.observed.buffer,
                action: action.clone(),
            });
        }
        self.pending = true;
        Ok(())
    }

    /// Acceptance settles submission; cursor mapping requires exact agreement.
    pub fn accept(&mut self, window: Window, accepted: Option<BufferState>) {
        self.pending = false;
        if accepted.is_some() {
            self.submitted = None;
        }
        let matching = accepted == Some(window.observed.buffer);
        self.halted =
            self.submitted.is_some() || (!matching && !self.queued.is_empty());
        let proposed = self.pending_cursor.take().unwrap_or(0);
        self.cursor = if matching {
            let relative = proposed
                .checked_sub(window.range.start)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(0);
            viewport::cursor_boundary(&window.units, relative)
        } else {
            0
        };
        self.status = format!(
            "Revision {} | F5 refresh | Ctrl+Q leave",
            window.observed.buffer.revision
        );
        self.current = Some(window);
        if self.halted {
            self.status.push_str(" | Input retained: Ctrl+D discard");
        }
    }

    /// Failure preserves submitted action and cursor intent for resolution.
    pub fn failed(&mut self, error: &dyn core::fmt::Display) {
        self.pending = false;
        self.halted = true;
        self.status = format!("View failed: {error} | F6 recover | F5 retry");
    }

    /// Unresolved submitted work cannot be discarded with the unsent queue.
    pub fn discard_retained(&mut self) {
        self.queued.clear();
        if self.receipt.is_none() {
            self.submitted = None;
            self.pending_cursor = None;
        }
        self.halted = true;
        self.status = if self.receipt.is_some() {
            "Queued input discarded; F6 recover submitted action"
        } else {
            "Retained input discarded; F5 refresh required"
        }
        .into();
    }

    /// Captures authored state independently of rendering and device events.
    /// # Errors
    /// Rejects a cursor without a matching visible-window position.
    pub fn retained_input(&self) -> Result<RetainedInput, Error> {
        let local =
            u64::try_from(self.cursor).map_err(|_| Error::InvalidCursor)?;
        let (basis, cursor) = match &self.current {
            Some(window) if self.cursor <= window.units.len() => (
                Some(window.observed.buffer),
                window
                    .range
                    .start
                    .checked_add(local)
                    .ok_or(Error::InvalidCursor)?,
            ),
            None if local == 0 => (None, 0),
            _ => return Err(Error::InvalidCursor),
        };
        Ok(RetainedInput {
            basis,
            cursor,
            planned: self.pending_cursor,
            pending: self.pending,
            halted: self.halted,
            queued: self.queued.clone(),
            submitted: self.submitted.clone(),
        })
    }
}

fn qualified(action: &Action, window: &Window) -> bool {
    match action {
        Action::Insert(start, end, _, origin) => {
            *origin == window.range.start
                && *start >= window.range.start
                && *end <= window.range.end
        }
        Action::Observe(_) | Action::Undo | Action::Redo => true,
        _ => false,
    }
}
