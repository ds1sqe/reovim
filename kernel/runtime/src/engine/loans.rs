use alloc::{
    rc::{Rc, Weak},
    vec::Vec,
};
use core::cell::RefCell;
use reovim_kernel_interface::{Error, Value, resource::AccessId};

use super::{
    Entry,
    access::{Handle, Opened, Ownership, Scope},
    reserve,
};

pub(super) struct Loans {
    scope: Weak<RefCell<Scope>>,
    sources: Vec<Rc<RefCell<Opened>>>,
    opened: Vec<Rc<RefCell<Opened>>>,
    pub input: Value,
}

impl Loans {
    pub fn admit(
        source: &mut Scope,
        current: &Rc<RefCell<Scope>>,
        provider: &Entry,
        input: &Value,
        rights: &[u32],
    ) -> Result<Option<Self>, Error> {
        if input.references.is_empty() {
            return Ok(None);
        }
        let target = provider
            .imports
            .as_ref()
            .and_then(Weak::upgrade)
            .ok_or(Error::UNSUPPORTED)?;
        let handles = prepare(source, input, rights)?;
        let mut sources = reserve(handles.len())?;
        let mut opened = reserve(handles.len())?;
        for token in &input.references {
            sources.push(source.live(*token)?.opened.clone());
        }
        for handle in &handles {
            opened.push(handle.opened.clone());
        }
        let mut loans = Self {
            scope: Rc::downgrade(&target),
            sources,
            opened,
            input: Value::copy(&input.data, &input.references)?,
        };
        if Rc::ptr_eq(current, &target) {
            install(source, handles, &mut loans.input)?;
        } else {
            let mut target =
                target.try_borrow_mut().map_err(|_| Error::BUSY)?;
            install(&mut target, handles, &mut loans.input)?;
        }
        Ok(Some(loans))
    }

    pub fn uses(&self, opened: &Rc<RefCell<Opened>>) -> bool {
        self.sources.iter().any(|source| Rc::ptr_eq(source, opened))
    }

    pub fn revoke(self) {
        for opened in &self.opened {
            opened.borrow_mut().closed = true;
        }
        let Some(scope) = self.scope.upgrade() else { return };
        // Execution calls this after provider finish, outside caller guards.
        // Bound aliases share the loan fence and cannot extend its lifetime.
        scope.borrow_mut().handles.retain(|handle| {
            !self.opened.iter().any(|opened| Rc::ptr_eq(opened, &handle.opened))
        });
    }
}

fn prepare(
    source: &Scope,
    input: &Value,
    rights: &[u32],
) -> Result<Vec<Handle>, Error> {
    let mut handles = reserve(input.references.len())?;
    for (&token, &rights) in input.references.iter().zip(rights) {
        let source = source.live(token)?;
        handles.push(Handle {
            token: AccessId(0),
            entry: source.entry.clone(),
            implementation: source.implementation.clone(),
            interface: source.interface,
            provisional: None,
            opened: Rc::new(RefCell::new(Opened {
                held: rights,
                ceiling: rights,
                create_access: source.opened.borrow().create_access,
                closed: false,
                ownership: Ownership::Loan {
                    capture: source.opened.borrow().ownership.retains_loans(),
                },
            })),
        });
    }
    Ok(handles)
}

fn install(
    target: &mut Scope,
    handles: Vec<Handle>,
    input: &mut Value,
) -> Result<(), Error> {
    if target.closed {
        return Err(Error::CLOSED);
    }
    let count = u64::try_from(handles.len()).map_err(|_| Error::PRESSURE)?;
    let next = target.next_handle.checked_add(count).ok_or(Error::PRESSURE)?;
    if handles.len()
        > target.limits.handles - target.handles.len() - target.reserved_handles
        || next == u64::MAX
    {
        return Err(Error::PRESSURE);
    }
    // All fallible preparation precedes publication into the provider scope.
    for (mut handle, slot) in handles.into_iter().zip(&mut input.references) {
        handle.token = AccessId(target.next_handle);
        *slot = handle.token;
        target.next_handle += 1;
        target.handles.push(handle);
    }
    Ok(())
}
