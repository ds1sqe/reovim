use super::{Entry, Implementation, Registry, reserve};
use alloc::{
    rc::{Rc, Weak},
    vec::Vec,
};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, Pending,
    provider::{Open, ResourceProvider},
};

mod support;

#[cfg(test)]
mod tests;

// The driver owns accepted contexts even when all provisional handles vanish.
// Reservation precedes Open; neither failed cleanup nor repeated close may
// create unbounded orphan contexts outside the configured access capacity.
pub(super) struct Pool {
    slots: Vec<Rc<RefCell<Stage>>>,
    limit: usize,
}

struct Stage {
    implementation: Option<Rc<Implementation>>,
    subject: Option<Rc<Entry>>,
    closed: bool,
    done: bool,
    generation: u64,
}

struct Target {
    owner: Weak<RefCell<Stage>>,
    generation: u64,
}

#[derive(Clone)]
pub(super) struct Reservation {
    stage: Rc<RefCell<Stage>>,
}

impl Pool {
    pub fn reserve_batch(
        &mut self,
        count: usize,
    ) -> Result<Vec<Reservation>, Error> {
        if count > self.limit - self.slots.len() {
            return Err(Error::PRESSURE);
        }
        let mut result = reserve(count)?;
        for _ in 0..count {
            // Capacity is checked for the whole batch before any mutation.
            result.push(self.reserve().unwrap());
        }
        Ok(result)
    }
    pub fn new(limit: usize) -> Result<Self, Error> {
        Ok(Self { slots: reserve(limit)?, limit })
    }

    pub fn reserve(&mut self) -> Result<Reservation, Error> {
        if self.slots.len() == self.limit {
            return Err(Error::PRESSURE);
        }
        let stage = Rc::new(RefCell::new(Stage {
            implementation: None,
            subject: None,
            closed: false,
            done: false,
            generation: 0,
        }));
        self.slots.push(stage.clone());
        Ok(Reservation { stage })
    }

    pub const fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn uses_imports(
        &self,
        scope: &Rc<RefCell<super::access::Scope>>,
    ) -> bool {
        let scope = Rc::downgrade(scope);
        self.slots.iter().any(|slot| uses_imports(&slot.borrow(), &scope))
    }
}

impl Reservation {
    pub fn begin(&self, subject: &Rc<Entry>) -> Result<Option<Pending>, Error> {
        if subject.is_closed() {
            return Err(Error::CLOSED);
        }
        self.stage.borrow_mut().subject = Some(subject.clone());
        if !subject.implementation.opens {
            return Ok(None);
        }
        let target = Open::new(
            Target {
                owner: Rc::downgrade(&self.stage),
                generation: self.stage.borrow().generation,
            },
            stage,
        );
        subject.implementation.provider.borrow_mut().open(&target).map(Some)
    }

    pub fn selected(&self) -> Result<Rc<Implementation>, Error> {
        let mut stage = self.stage.borrow_mut();
        stage.closed = true;
        let subject = stage.subject.as_ref().ok_or(Error::PROVIDER)?;
        if !subject.implementation.opens {
            return Ok(subject.implementation.clone());
        }
        stage.implementation.clone().ok_or(Error::PROVIDER)
    }

    pub fn fence(&self) {
        self.stage.borrow_mut().closed = true;
    }

    // A known failed initialization may be retried after its native request
    // and staged context have drained. The created subject is not recreated.
    pub fn reset(&self) -> Result<(), Error> {
        self.fence();
        let implementation = self.stage.borrow().implementation.clone();
        if let Some(implementation) = implementation {
            implementation.provider.borrow_mut().stop()?;
        }
        let previous = self.stage.borrow_mut().implementation.take();
        drop(previous);
        let mut stage = self.stage.borrow_mut();
        // Reusing a slot must not revive callbacks retained by the old attempt.
        stage.generation =
            stage.generation.checked_add(1).ok_or(Error::PRESSURE)?;
        stage.closed = false;
        Ok(())
    }

    pub fn discard(&self) -> Result<(), Error> {
        self.fence();
        let state = self.stage.borrow();
        if state
            .implementation
            .as_ref()
            .is_some_and(|p| Rc::strong_count(p) != 1)
        {
            return Err(Error::BUSY);
        }
        let implementation = state.implementation.clone();
        drop(state);
        if let Some(implementation) = implementation {
            implementation.provider.borrow_mut().stop()?;
        }
        let implementation = self.stage.borrow_mut().implementation.take();
        drop(implementation);
        self.finish();
        Ok(())
    }

    pub fn finish(&self) {
        let mut stage = self.stage.borrow_mut();
        stage.closed = true;
        stage.done = true;
    }
}

fn stage(
    target: &Target,
    provider: ResourceProvider,
) -> Result<(), (Error, ResourceProvider)> {
    let Some(owner) = target.owner.upgrade() else {
        return Err((Error::CLOSED, provider));
    };
    let mut state = owner.borrow_mut();
    if state.closed || target.generation != state.generation {
        return Err((Error::CLOSED, provider));
    }
    if state.implementation.is_some() {
        return Err((Error::CONFLICT, provider));
    }
    let subject = state.subject.as_ref().unwrap();
    if !support::compatible(&subject.implementation, &provider) {
        return Err((Error::UNSUPPORTED, provider));
    }
    state.implementation = Some(Implementation::new(provider));
    Ok(())
}

pub(super) fn reclaim(registry: &Rc<RefCell<Registry>>) -> Result<bool, Error> {
    let count = registry.borrow().opening.slots.len();
    let mut progress = false;
    for index in (0..count).rev() {
        let stage = registry.borrow().opening.slots[index].clone();
        if cleanup(&stage)? {
            registry.borrow_mut().opening.slots.remove(index);
            progress = true;
        }
    }
    Ok(progress)
}

fn cleanup(owner: &Rc<RefCell<Stage>>) -> Result<bool, Error> {
    let state = owner.borrow();
    if !state.done {
        return Ok(false);
    }
    if state.implementation.as_ref().is_some_and(|p| Rc::strong_count(p) != 1) {
        return Ok(false);
    }
    let implementation = state.implementation.clone();
    drop(state);
    let Some(implementation) = implementation else { return Ok(true) };
    match implementation.provider.borrow_mut().stop() {
        Ok(()) => Ok(true),
        Err(Error::BUSY) => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn close(
    registry: &Rc<RefCell<Registry>>,
    implementation: &Rc<Implementation>,
    aliases: usize,
) -> Result<(), Error> {
    // The caller holds one temporary clone; the pool owns another. Other
    // captures/loans/tasks must remain usable until their own fences drain.
    if Rc::strong_count(implementation) != aliases + 2 {
        return Ok(());
    }
    let registry = registry.borrow();
    let stage =
        registry
            .opening
            .slots
            .iter()
            .find(|stage| {
                stage.borrow().implementation.as_ref().is_some_and(
                    |candidate| Rc::ptr_eq(candidate, implementation),
                )
            })
            .cloned();
    drop(registry);
    let Some(stage) = stage else { return Ok(()) };
    if !stage.borrow().done {
        return Err(Error::BUSY);
    }
    implementation.provider.borrow_mut().stop()
}

fn uses_imports(
    stage: &Stage,
    scope: &alloc::rc::Weak<RefCell<super::access::Scope>>,
) -> bool {
    stage.subject.as_ref().is_some_and(|subject| {
        subject
            .imports
            .as_ref()
            .is_some_and(|imports| alloc::rc::Weak::ptr_eq(imports, scope))
    })
}
