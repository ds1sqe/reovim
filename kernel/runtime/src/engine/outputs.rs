use alloc::{
    rc::{Rc, Weak},
    vec::Vec,
};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, Value,
    declaration::{Method, ValueLimits, ValueType},
    provider::Receive,
    resource::AccessId,
};

use super::{
    Entry,
    access::{Handle, Opened, Ownership, Scope},
    custody::Custody,
    factory::Factory,
    reserve,
};

#[derive(Clone, Copy)]
pub(super) enum Schema<'a> {
    Value(&'a ValueType),
    Receive(&'a ValueType, Receive),
}

impl Schema<'_> {
    fn validate(
        self,
        value: &Value,
        limits: ValueLimits,
        check: impl FnMut(u32, Option<[u8; 16]>, u32) -> Result<(), Error>,
    ) -> Result<(), Error> {
        match self {
            Self::Value(schema) => schema.validate(
                &value.data,
                value.references.len(),
                limits,
                check,
            ),
            Self::Receive(packet, request) => {
                request.validate(packet, value, limits, check)
            }
        }
    }
}

struct Slot {
    handle: Option<Handle>,
    origin: Option<Rc<RefCell<Opened>>>,
    rights: u32,
}

pub(super) struct Outputs {
    target: Weak<RefCell<Scope>>,
    source: Option<Weak<RefCell<Scope>>>,
    request: u64,
    slots: Vec<Slot>,
    reserved: usize,
    prepared: Option<Value>,
}

impl Outputs {
    pub fn reserve(
        scope: &mut Scope,
        target: &Rc<RefCell<Scope>>,
        entry: &Rc<Entry>,
        method: &Method,
        request: u64,
    ) -> Result<Self, Error> {
        let primary = usize::from(method.created().is_some());
        let references = scope.limits.values.references;
        if references < primary || scope.limits.values.bytes < primary * 4 {
            return Err(Error::PRESSURE);
        }
        let count = method
            .output()
            .reference_bound(references - primary)
            .checked_add(primary)
            .ok_or(Error::PRESSURE)?;
        Self::reserve_count(scope, target, entry, count, request)
    }

    pub fn reserve_count(
        scope: &mut Scope,
        target: &Rc<RefCell<Scope>>,
        entry: &Rc<Entry>,
        count: usize,
        request: u64,
    ) -> Result<Self, Error> {
        if count
            > scope.limits.handles
                - scope.handles.len()
                - scope.reserved_handles
        {
            return Err(Error::PRESSURE);
        }
        let end = scope
            .next_handle
            .checked_add(u64::try_from(count).map_err(|_| Error::PRESSURE)?)
            .ok_or(Error::PRESSURE)?;
        let mut slots = reserve(count)?;
        for index in 0..count {
            slots.push(Slot {
                handle: Some(Handle {
                    token: AccessId(scope.next_handle + index as u64),
                    entry: entry.clone(),
                    implementation: entry.implementation.clone(),
                    interface: None,
                    provisional: Some(request),
                    opened: Rc::new(RefCell::new(Opened {
                        held: 0,
                        ceiling: 0,
                        create_access: 0,
                        closed: false,
                        ownership: Ownership::Owned { retain_loans: false },
                    })),
                }),
                origin: None,
                rights: 0,
            });
        }
        scope.next_handle = end;
        scope.reserved_handles += count;
        Ok(Self {
            target: Rc::downgrade(target),
            source: entry.imports.clone(),
            request,
            slots,
            reserved: count,
            prepared: None,
        })
    }

    pub fn transfer(
        &mut self,
        value: &mut Value,
        schema: Schema<'_>,
        limits: ValueLimits,
    ) -> Result<Option<Error>, Error> {
        let count = value.references.len();
        if count > self.slots.len() {
            return Err(Error::PROVIDER);
        }
        let source = self.source.as_ref().and_then(Weak::upgrade);
        self.validate(source.as_ref(), value, schema, limits, None)?;
        let target = self.target.upgrade().ok_or(Error::CLOSED)?;
        let retained = retained(&target.borrow(), self.request);
        if count != 0 {
            consume(&mut source.unwrap().borrow_mut(), &self.slots[..count]);
        }
        let mut target = target.borrow_mut();
        self.release_in(&mut target);
        for (slot, token) in self.slots.iter_mut().zip(&mut value.references) {
            install(slot, token, &mut target, retained);
        }
        Ok(None)
    }

    pub fn prepare_factory(
        &mut self,
        value: &Value,
        method: &Method,
        limits: ValueLimits,
        factory: &Factory,
    ) -> Result<Option<Error>, Error> {
        let count = value.references.len();
        if count + 1 > self.slots.len() {
            return Err(Error::PROVIDER);
        }
        let source = self.source.as_ref().and_then(Weak::upgrade);
        self.validate(
            source.as_ref(),
            value,
            Schema::Value(method.output()),
            limits,
            Some(factory),
        )?;
        let prepared = match self.stage(factory, value, limits) {
            Ok(value) => value,
            Err(error) => {
                self.discard(source.as_ref(), count);
                return Ok(Some(error));
            }
        };
        // Returned ownership leaves the producer before asynchronous Open.
        // The private slots keep all refs alive without exposing them yet.
        if count != 0 {
            consume(&mut source.unwrap().borrow_mut(), &self.slots[..count]);
        }
        self.prepared = Some(prepared);
        Ok(None)
    }

    pub fn install_factory(
        &mut self,
        factory: &mut Factory,
        method: &Method,
        limits: ValueLimits,
    ) -> Result<Value, Error> {
        factory.prepare()?;
        self.select(factory);
        self.validate_selected(method, limits)?;
        let target = self.target.upgrade().ok_or(Error::CLOSED)?;
        let mut target = target.borrow_mut();
        let retained = retained(&target, self.request);
        let mut value = self.prepared.take().unwrap();
        factory.commit();
        self.release_in(&mut target);
        for (slot, token) in self.slots.iter_mut().zip(&mut value.references) {
            install(slot, token, &mut target, retained);
        }
        Ok(value)
    }

    fn select(&mut self, factory: &Factory) {
        for slot in &mut self.slots {
            select_slot(slot, factory);
        }
    }

    fn validate_selected(
        &self,
        method: &Method,
        limits: ValueLimits,
    ) -> Result<(), Error> {
        let value = self.prepared.as_ref().unwrap();
        let check = |slot, api, rights| {
            let slot = usize::try_from(slot).map_err(|_| Error::INVALID)?;
            let handle = self.slots[slot].handle.as_ref().unwrap();
            super::access::reference(handle, api, rights)
        };
        method.output().validate(
            &value.data[4..],
            value.references.len() - 1,
            limits,
            check,
        )?;
        let primary = value.references.len() - 1;
        method.created().unwrap().validate(
            &[0; 4],
            1,
            limits,
            |_, api, rights| {
                super::access::reference(
                    self.slots[primary].handle.as_ref().unwrap(),
                    api,
                    rights,
                )
            },
        )
    }

    fn stage(
        &mut self,
        factory: &Factory,
        value: &Value,
        limits: ValueLimits,
    ) -> Result<Value, Error> {
        let entry = factory.prepare()?;
        let result = combined(value, limits)?;
        self.primary(value.references.len(), entry, factory);
        Ok(result)
    }

    fn discard(&mut self, source: Option<&Rc<RefCell<Scope>>>, count: usize) {
        if count != 0 {
            consume(&mut source.unwrap().borrow_mut(), &self.slots[..count]);
        }
        for slot in &mut self.slots[..count] {
            slot.handle.take();
            slot.origin.take();
        }
        self.release();
    }

    fn validate(
        &mut self,
        source: Option<&Rc<RefCell<Scope>>>,
        value: &Value,
        schema: Schema<'_>,
        limits: ValueLimits,
        factory: Option<&Factory>,
    ) -> Result<(), Error> {
        if value.references.is_empty() {
            return schema
                .validate(value, limits, |_, _, _| Err(Error::INVALID));
        }
        let source = source.ok_or(Error::PROVIDER)?;
        self.prepare(&source.borrow(), value, schema, limits, factory)
    }

    fn primary(&mut self, index: usize, entry: Rc<Entry>, factory: &Factory) {
        let handle = self.slots[index].handle.as_mut().unwrap();
        handle.implementation = entry.implementation.clone();
        handle.entry = entry;
        let mut opened = handle.opened.borrow_mut();
        opened.held = factory.rights;
        opened.ceiling = factory.rights;
        opened.ownership =
            Ownership::Owned { retain_loans: factory.retain_loans };
    }

    fn prepare(
        &mut self,
        source: &Scope,
        value: &Value,
        schema: Schema<'_>,
        limits: ValueLimits,
        factory: Option<&Factory>,
    ) -> Result<(), Error> {
        schema.validate(value, limits, |slot, api, rights| {
            let slot = usize::try_from(slot).map_err(|_| Error::INVALID)?;
            check_reference(
                source,
                factory,
                value.references[slot],
                api,
                rights,
            )?;
            self.slots[slot].rights |= rights;
            Ok(())
        })?;
        for (index, token) in value.references.iter().enumerate() {
            prepare_token(&mut self.slots, index, source, factory, *token)?;
        }
        Ok(())
    }

    pub fn release(&mut self) {
        self.prepared = None;
        for slot in &mut self.slots {
            slot.handle = None;
            slot.origin = None;
        }
        if let Some(target) = self.target.upgrade() {
            self.release_in(&mut target.borrow_mut());
        }
    }

    pub const fn release_in(&mut self, scope: &mut Scope) {
        scope.reserved_handles -= self.reserved;
        self.reserved = 0;
    }
}

fn select_slot(slot: &mut Slot, factory: &Factory) {
    if let Some(handle) = &mut slot.handle
        && let Some(selected) = factory.selected(handle.entry.identity)
    {
        handle.implementation = selected;
    }
}

fn prepare_slot(
    slots: &mut [Slot],
    index: usize,
    source: &Handle,
) -> Result<(), Error> {
    let ownership = source.opened.borrow().ownership;
    if !matches!(ownership, Ownership::Owned { .. }) {
        return Err(Error::DENIED);
    }
    let (previous, current) = slots.split_at_mut(index);
    let slot = &mut current[0];
    slot.origin = Some(source.opened.clone());
    let handle = slot.handle.as_mut().unwrap();
    handle.entry = source.entry.clone();
    handle.implementation = source.implementation.clone();
    handle.interface = source.interface;
    if let Some(alias) = previous.iter().find(|slot| {
        slot.origin.as_ref().is_some_and(|o| Rc::ptr_eq(o, &source.opened))
    }) {
        handle.opened = alias.handle.as_ref().unwrap().opened.clone();
    }
    let mut opened = handle.opened.borrow_mut();
    opened.held |= slot.rights;
    opened.ceiling |= slot.rights;
    opened.ownership = ownership;
    opened.create_access = source.opened.borrow().create_access;
    Ok(())
}

fn consume(source: &mut Scope, slots: &[Slot]) {
    for slot in slots {
        slot.origin.as_ref().unwrap().borrow_mut().closed = true;
    }
    source.handles.retain(|handle| {
        !slots.iter().any(|slot| {
            Rc::ptr_eq(slot.origin.as_ref().unwrap(), &handle.opened)
        })
    });
}

fn install(
    slot: &mut Slot,
    token: &mut AccessId,
    target: &mut Scope,
    retained: bool,
) {
    let handle = slot.handle.take().unwrap();
    *token = handle.token;
    if retained {
        target.handles.push(handle);
    }
}

fn retained(scope: &Scope, request: u64) -> bool {
    scope.records.iter().any(|receipt| {
        let record = receipt.borrow();
        record.id == request && record.custody != Custody::Released
    })
}

fn combined(value: &Value, limits: ValueLimits) -> Result<Value, Error> {
    let bytes = value.data.len().checked_add(4).ok_or(Error::PRESSURE)?;
    let references = value.references.len() + 1;
    if bytes > limits.bytes || references > limits.references {
        return Err(Error::PRESSURE);
    }
    let slot =
        u32::try_from(value.references.len()).map_err(|_| Error::PRESSURE)?;
    let mut data = reserve(bytes)?;
    data.extend_from_slice(&slot.to_le_bytes());
    data.extend_from_slice(&value.data);
    let mut refs = reserve(references)?;
    refs.extend_from_slice(&value.references);
    refs.push(AccessId(0));
    Ok(Value {
        data: data.into_boxed_slice(),
        references: refs.into_boxed_slice(),
    })
}

fn check_reference(
    source: &Scope,
    factory: Option<&Factory>,
    token: AccessId,
    api: Option<[u8; 16]>,
    rights: u32,
) -> Result<(), Error> {
    let staged = factory.and_then(|factory| {
        factory.with_auxiliary(token, |handle| {
            super::access::reference(handle, api, rights)
        })
    });
    staged.unwrap_or_else(|| source.reference(token, api, rights))
}

fn prepare_token(
    slots: &mut [Slot],
    index: usize,
    source: &Scope,
    factory: Option<&Factory>,
    token: AccessId,
) -> Result<(), Error> {
    let staged = factory.and_then(|factory| {
        factory
            .with_auxiliary(token, |handle| prepare_slot(slots, index, handle))
    });
    staged.unwrap_or_else(|| prepare_slot(slots, index, source.live(token)?))
}
