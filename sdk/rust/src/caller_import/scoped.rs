use super::{Caller, Error, Rc, State, ValueLimits, abi};
use core::any::Any;

/// Fences safe Caller clones before a borrowed native scope expires.
///
/// Close drains local delivery views. Original request custody still belongs
/// to the native Caller; the module must settle its requests before closing.
pub struct ImportedCaller {
    state: Rc<State>,
}

impl ImportedCaller {
    /// Borrows the Caller supplied to a native module instance.
    /// # Errors
    /// Rejects invalid table extents, missing callbacks or unusable bounds.
    /// # Safety
    /// Native storage and callbacks must obey the Caller ABI and remain valid
    /// until close succeeds. Busy or failed cleanup does not end that promise.
    /// Code retains executable dependencies. Calls are serialized on the
    /// native executor thread and callbacks never unwind. Successful view
    /// finish must disconnect its retained wake callbacks before returning.
    pub unsafe fn new(
        raw: abi::Caller,
        code: Rc<dyn Any>,
        limits: ValueLimits,
        capacity: usize,
    ) -> Result<Self, Error> {
        // SAFETY: Construction's native lifetime promise covers the checked
        // table and every delivery view until explicit close succeeds.
        let state = unsafe { super::state(raw, code, limits, capacity)? };
        Ok(Self { state })
    }

    /// Returns a cloneable capability that is fenced when close begins.
    #[must_use]
    pub fn caller(&self) -> Caller {
        super::caller(self.state.clone())
    }

    /// Fences further calls and drains retained native delivery views.
    /// # Errors
    /// Busy or failed view cleanup retains ownership for another close call.
    pub fn close(&mut self) -> Result<(), Error> {
        self.state.closed.set(true);
        if self.state.active.get() != 0 {
            return Err(Error::BUSY);
        }
        drain(&self.state)
    }
}

impl Drop for ImportedCaller {
    fn drop(&mut self) {
        if self.close().is_err() {
            // Abandoning cleanup cannot free code or native delivery owners.
            // The unsafe admission promise still requires the borrowed parent
            // context to survive; Drop cannot authorize its destruction.
            core::mem::forget(self.state.clone());
        }
    }
}

fn drain(state: &State) -> Result<(), Error> {
    loop {
        let record = state.records.borrow().last().cloned();
        let Some(record) = record else { return Ok(()) };
        if record.borrow().busy {
            return Err(Error::BUSY);
        }
        super::close_view(&record)?;
        super::remove(state, &record);
    }
}
