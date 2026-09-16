use super::super::{
    access::{Handle, Opened, Ownership, Scope},
    codec,
    custody::Custody,
    execution::Task,
};
use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, Outcome, Pending, Value,
    call::Completion,
    caller::Call,
    provider::{Creation, CreationOps, ResourceProvider},
    resource::AccessId,
};
mod driving;
mod staging;
use staging::Stage;

pub(in crate::engine) struct Create {
    stage: Rc<RefCell<Stage>>,
    opening: super::super::opening::Reservation,
    creation: Creation,
    name: Box<[u8]>,
    target: Weak<RefCell<Scope>>,
    token: AccessId,
    opened: Rc<RefCell<Opened>>,
    request: u64,
    output: Option<Value>,
    reserved: bool,
}
pub(in crate::engine) fn admit(
    scope: &mut Scope,
    target: &Rc<RefCell<Scope>>,
    call: &Call,
) -> Result<Task, Error> {
    let parent = scope.live(call.subject)?;
    let opened = parent.opened.borrow();
    if opened.held & 2 == 0 || opened.create_access & 3 != 3 {
        return Err(Error::DENIED);
    }
    let support = parent
        .implementation
        .provider
        .try_borrow()
        .map_err(|_| Error::BUSY)?
        .namespace()
        .ok_or(Error::UNSUPPORTED)?;
    if !support.create {
        return Err(Error::UNSUPPORTED);
    }
    let name = codec::bytes(codec::no_refs(&call.input)?, 4096)?;
    codec::name(name, scope.non_ascii_names)?;
    let name = Box::from(name);
    let token = scope.token()?;
    let mut output =
        codec::acquired(parent.entry.identity, token, scope.limits.values)?;
    let held = Rc::new(RefCell::new(Opened {
        held: 3,
        ceiling: opened.create_access,
        create_access: 0,
        closed: false,
        ownership: Ownership::Owned {
            retain_loans: opened.ownership.retains_loans(),
        },
    }));
    drop(opened);
    let owner = parent.entry.clone();
    let registry = scope.registry.upgrade().ok_or(Error::CLOSED)?;
    let identity = registry.borrow_mut().reserve()?;
    let reserved = registry.borrow_mut().opening.reserve();
    let opening = match reserved {
        Ok(opening) => opening,
        Err(error) => {
            registry.borrow_mut().release();
            return Err(error);
        }
    };
    output.data[..16].copy_from_slice(&identity.epoch);
    output.data[16..24].copy_from_slice(&identity.serial.to_le_bytes());
    let stage = Rc::new(RefCell::new(Stage::new(
        scope.registry.clone(),
        owner.clone(),
        parent.implementation.clone(),
        identity,
        scope.limits.values.bytes,
    )));
    let creation = Creation::new(
        Rc::downgrade(&stage),
        CreationOps {
            resource: |_, provider| Err((Error::UNSUPPORTED, provider)),
            keyed: Some(staging::stage),
            auxiliary: None,
        },
    );
    scope.next_handle += 1;
    scope.reserved_handles += 1;
    Ok(Task::creation(
        owner,
        Create {
            stage,
            opening,
            creation,
            name,
            target: Rc::downgrade(target),
            token,
            opened: held,
            request: call.request,
            output: Some(output),
            reserved: true,
        },
    ))
}
impl Create {
    pub fn begin(
        &self,
        provider: &mut ResourceProvider,
    ) -> Result<Pending, Error> {
        let native = provider.create_child(&self.name, &self.creation)?;
        Ok(driving::begin(native, self.stage.clone(), self.opening.clone()))
    }
    pub fn validate(
        &mut self,
        completion: &mut Completion,
    ) -> Result<Option<Error>, Error> {
        if completion.activity & !2 != 0 {
            return Err(Error::PROVIDER);
        }
        match &completion.outcome {
            Outcome::Ok(value)
                if value.data.is_empty() && value.references.is_empty() => {}
            Outcome::Failed(_) | Outcome::Unknown { progress: None, .. } => {
                return Ok(None);
            }
            _ => return Err(Error::PROVIDER),
        }
        let target = self.target.upgrade().ok_or(Error::PROVIDER)?;
        let implementation = self.opening.selected()?;
        let mut scope = target.borrow_mut();
        // Key/schema/capacity reservations precede publication. Accepted work
        // can settle during shutdown; no new admission is performed here.
        let entry = self.stage.borrow_mut().commit()?;
        self.release_in(&mut scope);
        if scope.records.iter().any(|record| {
            let record = record.borrow();
            record.id == self.request && record.custody != Custody::Released
        }) {
            scope.handles.push(Handle {
                token: self.token,
                implementation,
                entry,
                opened: self.opened.clone(),
                interface: None,
                provisional: Some(self.request),
            });
        }
        completion.outcome = Outcome::Ok(self.output.take().unwrap());
        Ok(None)
    }
    pub fn finish(&self) -> Result<(), Error> {
        staging::finish(&self.stage)
    }
    pub fn release(&mut self) {
        self.opening.finish();
        if let Some(scope) = self.target.upgrade() {
            self.release_in(&mut scope.borrow_mut());
        }
    }
    const fn release_in(&mut self, scope: &mut Scope) {
        if self.reserved {
            scope.reserved_handles -= 1;
            self.reserved = false;
        }
    }
}
