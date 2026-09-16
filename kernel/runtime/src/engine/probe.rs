use super::{ModuleSelection, Runtime, modules::probe as ownership, reserve};
use alloc::{rc::Rc, vec::Vec};
use core::{cell::Cell, task::Poll};
use reovim_kernel_interface::{
    Error, ResourceId, call::Cancellation, driver::Lifecycle,
    module::ModuleScope,
};

struct State {
    result: Cell<Poll<Result<ResourceId, Error>>>,
    cancelled: Cell<bool>,
}
/// Composition's observation of one driven graph activation. Dropping it
/// requests cancellation; Runtime retains rollback independently of observers.
pub struct ModuleProbe {
    state: Rc<State>,
}
impl ModuleProbe {
    /// Observes the retained result without executing provider code.
    pub fn status(&self) -> Poll<Result<ResourceId, Error>> {
        self.state.result.get()
    }
    /// Requests cancellation; Pending still owns any required rollback.
    #[must_use]
    pub fn cancel(&self) -> Cancellation {
        if self.state.result.get().is_ready() {
            return Cancellation::TooLate;
        }
        self.state.cancelled.set(true);
        Cancellation::Accepted
    }
}
impl Drop for ModuleProbe {
    fn drop(&mut self) {
        self.state.cancelled.set(true);
    }
}
pub(super) struct Work {
    selection: ModuleSelection,
    state: Rc<State>,
    instances: Vec<ResourceId>,
    next: usize,
    error: Option<Error>,
    retiring: Option<ResourceId>,
}
impl Runtime {
    /// Starts a bounded, driven activation of an exact captured graph.
    /// Existing instances are shared; rollback releases only provisional use.
    /// # Errors
    /// Closed, unsupported scope or pressure rejects before construction.
    pub fn probe_modules(
        &mut self,
        selection: ModuleSelection,
    ) -> Result<ModuleProbe, Error> {
        if self.closing {
            return Err(Error::CLOSED);
        }
        self.validate_probe(&selection)?;
        if selection.modules().iter().any(|m| m.scope() != ModuleScope::Global)
        {
            return Err(Error::UNSUPPORTED);
        }
        if self.probes.len() == self.limits.requests {
            return Err(Error::PRESSURE);
        }
        let instances = reserve(selection.modules().len())?;
        let state = Rc::new(State {
            result: Cell::new(Poll::Pending),
            cancelled: Cell::new(false),
        });
        self.probes.push(Work {
            selection,
            state: state.clone(),
            instances,
            next: 0,
            error: None,
            retiring: None,
        });
        Ok(ModuleProbe { state })
    }
}
impl Work {
    pub(super) fn uses_binding(
        &self,
        scope: &Rc<core::cell::RefCell<super::access::Scope>>,
    ) -> bool {
        self.selection
            .modules()
            .iter()
            .any(|selected| selected.uses_binding(scope))
    }

    fn advance(&mut self, runtime: &mut Runtime) {
        if self.state.cancelled.get() {
            self.error.get_or_insert(Error::CANCELLED);
        }
        if runtime.closing {
            self.shutdown(runtime);
            return;
        }
        if self.error.is_some() {
            self.rollback(runtime);
            return;
        }
        if let Err(error) = self.enter(runtime) {
            self.error = Some(error);
            return;
        }
        match runtime.module_status(self.instances[self.next]) {
            Ok(Lifecycle::Ready) => self.next += 1,
            Ok(Lifecycle::Failed(error)) | Err(error) => {
                self.error = Some(error);
            }
            Ok(Lifecycle::Closing | Lifecycle::Closed) => {
                self.error = Some(Error::CLOSED);
            }
            _ => {}
        }
        if self.next == self.selection.modules().len() {
            self.commit(runtime);
        }
    }
    fn shutdown(&mut self, runtime: &mut Runtime) {
        self.error.get_or_insert(Error::CLOSED);
        // Shutdown owns teardown. Wait for actual destruction, not a cached
        // lifecycle report, without waiting for the caller receipt to drain.
        if self.retiring.is_some_and(|id| !ownership::stopped(runtime, id)) {
            return;
        }
        self.retiring = None;
        let Some(&id) = self.instances.last() else {
            self.state.result.set(Poll::Ready(Err(self.error.unwrap())));
            return;
        };
        if !ownership::stopped(runtime, id) {
            return;
        }
        ownership::release(runtime, id);
        self.instances.pop();
    }
    fn enter(&mut self, runtime: &mut Runtime) -> Result<(), Error> {
        if self.next != self.instances.len() {
            return Ok(());
        }
        let selected = &self.selection.modules()[self.next];
        let id = runtime.activate_for_probe(selected)?;
        self.instances.push(id);
        Ok(())
    }
    fn release_next(&mut self, runtime: &mut Runtime) {
        let Some(id) = self.instances.pop() else {
            self.state.result.set(Poll::Ready(Err(self.error.unwrap())));
            return;
        };
        self.retiring = ownership::release(runtime, id).then_some(id);
    }
    fn commit(&self, runtime: &mut Runtime) {
        for &id in &self.instances {
            ownership::commit(runtime, id);
            ownership::release(runtime, id);
        }
        self.state.result.set(Poll::Ready(Ok(*self.instances.last().unwrap())));
    }
    fn rollback(&mut self, runtime: &mut Runtime) {
        if self.retiring.is_none() {
            self.release_next(runtime);
        }
        let Some(id) = self.retiring else { return };
        if !ownership::owned(runtime, id) {
            self.retiring = None;
            return;
        }
        match runtime.unload_module(id) {
            Ok(Lifecycle::Closed) | Err(Error::UNAVAILABLE) => {
                self.retiring = None;
            }
            _ => {}
        }
    }
}
pub(super) fn advance(runtime: &mut Runtime) -> bool {
    let count = runtime.probes.len();
    let mut progress = false;
    for index in (0..count).rev() {
        let mut work = runtime.probes.remove(index);
        let before =
            (work.next, work.instances.len(), work.error, work.retiring);
        work.advance(runtime);
        progress |= before
            != (work.next, work.instances.len(), work.error, work.retiring);
        if work.state.result.get().is_pending() {
            runtime.probes.insert(index, work);
        }
    }
    progress || count != runtime.probes.len()
}
pub(super) fn closed(runtime: &mut Runtime) {
    for mut work in runtime.probes.drain(..) {
        if work.state.cancelled.get() {
            work.error.get_or_insert(Error::CANCELLED);
        }
        work.state
            .result
            .set(Poll::Ready(Err(work.error.unwrap_or(Error::CLOSED))));
    }
}
