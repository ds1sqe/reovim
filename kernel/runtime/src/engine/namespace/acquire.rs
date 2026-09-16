use super::{
    super::{
        access::{Handle, Opened, Ownership, Scope},
        codec,
        custody::Custody,
        execution::Task,
    },
    materialize::{self, Work},
    traversal::Traversal,
};
use alloc::rc::{Rc, Weak};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, Outcome, Pending, Value, call::Completion, caller::Call,
    resource::AccessId,
};

mod driving;

pub(in crate::engine) struct Acquire {
    source: Source,
    opening: super::super::opening::Reservation,
    target: Weak<RefCell<Scope>>,
    token: AccessId,
    opened: Rc<RefCell<Opened>>,
    request: u64,
    output: Option<Value>,
    reserved: bool,
}
#[derive(Clone)]
enum Source {
    Ready(Rc<super::super::Entry>),
    Identity(Rc<RefCell<Work>>),
    Path(Rc<RefCell<Traversal>>),
}
pub(in crate::engine) fn admit(
    scope: &mut Scope,
    target: &Rc<RefCell<Scope>>,
    call: &Call,
) -> Result<Option<Task>, Error> {
    let data = codec::no_refs(&call.input)?;
    let (tag, data) = data.split_at_checked(4).ok_or(Error::INVALID)?;
    let tag = codec::u32_value(tag)?;
    let selected = source(scope, tag, data)?;
    let (source, owner, grant) = if let Some(selected) = selected {
        selected
    } else {
        let (owner, grant) = super::super::access::locate(scope, tag, data)?;
        if !owner.implementation.opens {
            return Ok(None);
        }
        (Source::Ready(owner.clone()), owner, grant)
    };
    if owner.is_closed() {
        return Err(Error::CLOSED);
    }
    let token = scope.token()?;
    let output = codec::acquired(owner.identity, token, scope.limits.values)?;
    let registry = scope.registry.upgrade().ok_or(Error::CLOSED)?;
    let opening = registry.borrow_mut().opening.reserve()?;
    let opened = Rc::new(RefCell::new(Opened {
        held: 0,
        ceiling: grant.access,
        create_access: grant.create_access,
        closed: false,
        ownership: Ownership::Owned { retain_loans: grant.retain_loans },
    }));
    scope.next_handle += 1;
    scope.reserved_handles += 1;
    Ok(Some(Task::acquisition(
        owner,
        Acquire {
            source,
            opening,
            target: Rc::downgrade(target),
            token,
            opened,
            request: call.request,
            output: Some(output),
            reserved: true,
        },
    )))
}

type Selected = (Source, Rc<super::super::Entry>, super::super::Grant);

fn source(
    scope: &Scope,
    tag: u32,
    data: &[u8],
) -> Result<Option<Selected>, Error> {
    let selected = match tag {
        0 => {
            let Some((path, owner, grant)) =
                super::traversal::admit(scope, data)?
            else {
                return Ok(None);
            };
            (Source::Path(path), owner, grant)
        }
        1 => {
            let id = codec::identity(data)?;
            let registry = scope.registry.upgrade().ok_or(Error::CLOSED)?;
            let key = registry
                .borrow()
                .keys
                .iter()
                .find(|key| key.identity == id)
                .cloned();
            let Some(key) = key else { return Ok(None) };
            if key.ready.borrow().upgrade().is_some() {
                return Ok(None);
            }
            let owner = key.owner.upgrade().ok_or(Error::CLOSED)?;
            let grant = authority(scope, &owner)?;
            let work = Work::join(scope.registry.clone(), &key, owner.clone());
            (Source::Identity(work), owner, grant)
        }
        _ => return Err(Error::INVALID),
    };
    Ok(Some(selected))
}
pub(super) fn authority(
    scope: &Scope,
    owner: &Rc<super::super::Entry>,
) -> Result<super::super::Grant, Error> {
    if owner.is_closed() {
        return Err(Error::CLOSED);
    }
    let grant = scope
        .publications
        .iter()
        .find_map(|(authority, grant)| {
            (!authority.closed.get()).then(|| grant.member(owner)).flatten()
        })
        .ok_or(Error::DENIED)?;
    Ok(grant)
}
impl Acquire {
    pub fn begin(&self) -> Pending {
        driving::begin(self.source.clone(), self.opening.clone())
    }
    pub fn validate(
        &mut self,
        completion: &mut Completion,
    ) -> Result<Option<Error>, Error> {
        if !matches!(completion.outcome, Outcome::Ok(_)) {
            return Ok(None);
        }
        let entry = self.source.ready()?;
        let implementation = self.opening.selected()?;
        let output = self.output.as_mut().unwrap();
        output.data[..16].copy_from_slice(&entry.identity.epoch);
        output.data[16..24]
            .copy_from_slice(&entry.identity.serial.to_le_bytes());
        let target = self.target.upgrade().ok_or(Error::CLOSED)?;
        let mut scope = target.borrow_mut();
        if let Source::Path(path) = &self.source {
            let grant = path.borrow().grant.ok_or(Error::PROVIDER)?;
            let mut opened = self.opened.borrow_mut();
            opened.ceiling = grant.access;
            opened.create_access = grant.create_access;
            opened.ownership =
                Ownership::Owned { retain_loans: grant.retain_loans };
        }
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

impl Source {
    fn ready(&self) -> Result<Rc<super::super::Entry>, Error> {
        match self {
            Self::Ready(entry) => Some(entry.clone()),
            Self::Identity(work) => work.borrow().ready.clone(),
            Self::Path(path) => path.borrow().ready.clone(),
        }
        .ok_or(Error::PROVIDER)
    }
}
