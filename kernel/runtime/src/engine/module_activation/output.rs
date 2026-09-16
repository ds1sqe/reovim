use super::super::{
    Entry,
    access::{Handle, Opened, Ownership, Scope},
};
use alloc::rc::Rc;
use core::cell::RefCell;
use reovim_kernel_interface::{Error, Value, resource::AccessId};

pub(super) struct Output {
    scope: Rc<RefCell<Scope>>,
    token: AccessId,
    opened: Rc<RefCell<Opened>>,
    value: Option<Value>,
    reserved: bool,
}
impl Output {
    pub fn reserve(scope: &Rc<RefCell<Scope>>) -> Result<Self, Error> {
        let mut state = scope.borrow_mut();
        let token = state.token()?;
        let value = Value::copy(&0_u32.to_le_bytes(), &[token])?;
        let opened = Rc::new(RefCell::new(Opened {
            held: 5,
            ceiling: 5,
            create_access: 0,
            closed: false,
            ownership: Ownership::Owned { retain_loans: false },
        }));
        state.next_handle += 1;
        state.reserved_handles += 1;
        Ok(Self {
            scope: scope.clone(),
            token,
            opened,
            value: Some(value),
            reserved: true,
        })
    }
    pub fn publish(&mut self, entry: Rc<Entry>) -> Value {
        let mut scope = self.scope.borrow_mut();
        scope.reserved_handles -= 1;
        self.reserved = false;
        scope.handles.push(Handle {
            token: self.token,
            implementation: entry.implementation.clone(),
            entry,
            opened: self.opened.clone(),
            interface: Some(0),
            provisional: None,
        });
        self.value.take().unwrap()
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        let mut scope = self.scope.borrow_mut();
        if self.reserved {
            scope.reserved_handles -= 1;
        }
        // Outputs normally consumed this token during receipt installation.
        // Failed/abandoned transfer still releases the private source owner.
        scope.handles.retain(|handle| handle.token != self.token);
    }
}
