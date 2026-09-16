use super::{
    super::{Entry, Registry},
    intern::Key,
};
use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
};
use core::{
    cell::{Cell, RefCell},
    task::{Context, Poll},
};
use reovim_kernel_interface::{
    Error, Outcome, Pending, Value,
    call::Completion,
    provider::{Creation, CreationOps, ResourceProvider},
};

mod delivery;
pub(super) use delivery::pending;

struct Stage {
    owner: Rc<Entry>,
    key: Rc<Key>,
    primary: Option<Rc<Entry>>,
    closed: bool,
}
pub(super) struct Work {
    registry: Weak<RefCell<Registry>>,
    stage: Rc<RefCell<Stage>>,
    native: Option<Pending>,
    outcome: Option<Outcome>,
    generation: u64,
    done: bool,
    quarantine: Option<Completion>,
    pub ready: Option<Rc<Entry>>,
}
impl Work {
    pub fn join(
        registry: Weak<RefCell<Registry>>,
        key: &Rc<Key>,
        owner: Rc<Entry>,
    ) -> Rc<RefCell<Self>> {
        let existing = key.work.borrow().upgrade();
        if let Some(work) = existing {
            return work;
        }
        let work =
            Rc::new(RefCell::new(Self::new(registry, key.clone(), owner)));
        *key.work.borrow_mut() = Rc::downgrade(&work);
        work
    }
    fn new(
        registry: Weak<RefCell<Registry>>,
        key: Rc<Key>,
        owner: Rc<Entry>,
    ) -> Self {
        Self {
            registry,
            stage: Rc::new(RefCell::new(Stage {
                owner,
                key,
                primary: None,
                closed: false,
            })),
            native: None,
            outcome: None,
            generation: 0,
            done: false,
            quarantine: None,
            ready: None,
        }
    }
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        if self.outcome.is_some() {
            return Poll::Ready(());
        }
        let native = self.native.as_mut().unwrap();
        match native.poll(cx) {
            Ok(Poll::Pending) => return Poll::Pending,
            Ok(Poll::Ready(())) => {}
            Err(_) => {
                self.fault(None);
                return Poll::Ready(());
            }
        }
        match native.take() {
            Ok(completion) => self.accept(completion),
            Err(_) => self.fault(None),
        }
        Poll::Ready(())
    }
    fn begin(&mut self) {
        let source = self.stage.borrow();
        let owner = source.owner.clone();
        let key = source.key.clone();
        drop(source);
        if owner.closed.get() {
            self.set(Outcome::Failed(Error::CLOSED));
            return;
        }
        if key.creating.get() {
            self.set(Outcome::Failed(Error::BUSY));
            return;
        }
        let ready = key.ready.borrow().upgrade();
        if let Some(ready) = ready {
            self.ready = Some(ready);
            self.set(Outcome::Ok(Value::unit()));
            return;
        }
        let creation = Creation::new(
            Rc::downgrade(&self.stage),
            CreationOps { resource: stage, keyed: None, auxiliary: None },
        );
        let result = owner
            .implementation
            .provider
            .borrow_mut()
            .materialize(&key.bytes, &creation);
        match result {
            Ok(pending) => self.native = Some(pending),
            Err(error) => self.set(Outcome::Failed(error)),
        }
    }
    fn accept(&mut self, completion: Completion) {
        if completion.activity & !1 != 0 || invalid(&completion.outcome) {
            self.fault(Some(completion));
            return;
        }
        let outcome = match &completion.outcome {
            Outcome::Ok(_) => match self.install() {
                Ok(()) => Outcome::Ok(Value::unit()),
                Err(error) => Outcome::Failed(error),
            },
            other => other.clone(),
        };
        self.set(outcome);
    }
    fn install(&mut self) -> Result<(), Error> {
        let mut stage = self.stage.borrow_mut();
        let entry = stage.primary.as_ref().ok_or(Error::PROVIDER)?.clone();
        if stage.owner.closed.get() {
            return Err(Error::CLOSED);
        }
        let registry = self.registry.upgrade().ok_or(Error::CLOSED)?;
        let mut registry = registry.borrow_mut();
        registry.validate(&[], &entry.implementation.interfaces)?;
        if !stage.key.reserved.get() {
            return Err(Error::CONFLICT);
        }
        registry.install(entry.clone());
        stage.key.reserved.set(false);
        *stage.key.ready.borrow_mut() = Rc::downgrade(&entry);
        stage.primary = None;
        stage.closed = true;
        self.ready = Some(entry);
        Ok(())
    }
    fn set(&mut self, outcome: Outcome) {
        self.generation += 1;
        self.outcome = Some(outcome);
    }
    fn fault(&mut self, completion: Option<Completion>) {
        self.quarantine = completion;
        self.set(Outcome::Unknown { error: Error::PROVIDER, progress: None });
        // Even without a returned Completion, invalid poll/take is fenced.
        self.stage.borrow_mut().closed = true;
    }
    fn finish(&mut self) -> Result<(), Error> {
        if self.done {
            return Ok(());
        }
        if self
            .outcome
            .as_ref()
            .is_none_or(|outcome| matches!(outcome, Outcome::Unknown { .. }))
        {
            return Err(Error::BUSY);
        }
        cleanup(&self.stage)?;
        if let Some(native) = &mut self.native {
            native.finish()?;
        }
        self.native = None;
        self.done = true;
        Ok(())
    }
}
fn invalid(outcome: &Outcome) -> bool {
    match outcome {
        Outcome::Ok(value) => {
            !value.data.is_empty() || !value.references.is_empty()
        }
        Outcome::Unknown { error, progress } => {
            progress.is_some() || bad(error)
        }
        Outcome::Failed(error) => bad(error),
    }
}
fn bad(error: &Error) -> bool {
    error.code.0 == 0 || error.id.is_some_and(|id| id.serial == 0)
}
fn stage(
    owner: &Weak<RefCell<Stage>>,
    provider: ResourceProvider,
) -> Result<(), (Error, ResourceProvider)> {
    let Some(owner) = owner.upgrade() else {
        return Err((Error::CLOSED, provider));
    };
    let mut state = owner.borrow_mut();
    if state.closed {
        return Err((Error::CLOSED, provider));
    }
    if state.primary.is_some() {
        return Err((Error::CONFLICT, provider));
    }
    state.primary = Some(Rc::new(Entry {
        closed: Cell::new(false),
        identity: state.key.identity,
        path: Box::new([]),
        implementation: crate::engine::Implementation::new(provider),
        imports: state.owner.imports.clone(),
        _origin: Some(state.owner.clone()),
        namespace_owner: Some(Rc::downgrade(&state.owner)),
        directory: false,
        parent: None,
        publication: state.owner.publication,
    }));
    Ok(())
}
fn cleanup(stage: &Rc<RefCell<Stage>>) -> Result<(), Error> {
    stage.borrow_mut().closed = true;
    let primary = stage.borrow().primary.clone();
    if let Some(primary) = primary {
        primary.implementation.provider.borrow_mut().stop()?;
        stage.borrow_mut().primary = None;
        drop(primary);
    }
    Ok(())
}
