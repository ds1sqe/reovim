//! The seven calls, bound to one Module.

use alloc::collections::BTreeMap;

use reovim_kernel_interface::{
    Caller, Calls, Decl, DriverId, Error, Flags, Handle, Hash, Kind, ModuleId,
    Op, Out, Outcome, Rd, Reply, Rights, Token,
};

use crate::kernel::{Kernel, State};
use crate::table::{APIS, CONTAINER, DECLS, Node, Owner};

/// What a Module holds to speak to the Kernel.
pub struct ModuleCalls<'k> {
    kernel: &'k Kernel,
    module: ModuleId,
}

impl<'k> ModuleCalls<'k> {
    pub(crate) const fn new(kernel: &'k Kernel, module: ModuleId) -> Self {
        Self { kernel, module }
    }

    fn caller(&self) -> Caller {
        self.kernel.state.borrow().modules[self.module.0 as usize].caller
    }
}

impl Calls for ModuleCalls<'_> {
    fn register(
        &self,
        driver: DriverId,
        parent: Rd,
        name: Option<&str>,
    ) -> Result<Rd, Error> {
        let now = self.kernel.now();
        let mut s = self.kernel.state.borrow_mut();
        s.driver_index(self.module, driver)?;
        check_parent(&s, parent, self.module)?;
        s.table.check_name(parent, name)?;
        let owner = Owner::Driver { module: self.module, driver };
        let rd = s.table.insert(parent, name, owner, Flags::empty(), now);
        s.emit(parent, Kind::Create, Some(rd));
        Ok(rd)
    }

    fn name(&self, rd: Rd, name: Option<&str>) -> Result<(), Error> {
        let mut s = self.kernel.state.borrow_mut();
        let parent = owned(&s, rd, self.module)?.parent.ok_or(Error::Denied)?;
        s.table.check_name(parent, name)?;
        s.table.get_mut(rd)?.name =
            name.map(alloc::string::ToString::to_string);
        s.emit(parent, Kind::Move, Some(rd));
        Ok(())
    }

    fn report(&self, rd: Rd, flags: Flags, apis: &[Decl]) -> Result<(), Error> {
        let now = self.kernel.now();
        let mut s = self.kernel.state.borrow_mut();
        owned(&s, rd, self.module)?;
        check_decls(&s, apis)?;
        let hashes = apis
            .iter()
            .map(|d| store_decl(&mut s, d, now))
            .collect::<Result<_, _>>()?;
        let entry = s.table.get_mut(rd)?;
        entry.flags = flags;
        entry.apis = hashes;
        entry.ctime = now;
        s.emit(rd, Kind::Attrib, None);
        Ok(())
    }

    fn unregister(&self, rd: Rd) -> Result<(), Error> {
        let mut s = self.kernel.state.borrow_mut();
        let parent = owned(&s, rd, self.module)?.parent.ok_or(Error::Denied)?;
        s.unregister(rd, parent);
        Ok(())
    }

    fn changed(&self, rd: Rd, kind: Kind) -> Result<(), Error> {
        let now = self.kernel.now();
        let mut s = self.kernel.state.borrow_mut();
        owned(&s, rd, self.module)?;
        let entry = s.table.get_mut(rd)?;
        match kind {
            Kind::Modify => entry.mtime = now,
            Kind::Attrib => entry.ctime = now,
            _ => return Err(Error::Invalid),
        }
        s.emit(rd, kind, None);
        Ok(())
    }

    fn done(&self, token: Token, outcome: Outcome) -> Result<(), Error> {
        let mut s = self.kernel.state.borrow_mut();
        let outcome = match s.pending.cookie_of(self.module, token.0) {
            Some(h) => opened(&mut s, h, outcome),
            None => outcome,
        };
        // A listing the Driver has now made is what its rd's cache
        // answers from: the rd counts as listed here, so one that is
        // refused leaves the cache to the next `Children`.
        let listing = s.pending.listed_of(self.module, token.0);
        if let (Some(rd), true) = (listing, outcome.is_ok())
            && let Ok(entry) = s.table.get_mut(rd)
        {
            entry.listed = true;
        }
        let State { pending, decls, .. } = &mut *s;
        let handle = pending.done(self.module, token.0, outcome, decls)?;
        if let Some(slot) = handle.and_then(|h| s.handles.get_mut(&h)) {
            slot.pending = slot.pending.saturating_sub(1);
        }
        Ok(())
    }

    fn open(&self, rd: Rd, rights: Rights) -> Result<Handle, Error> {
        match self.submit(Op::Open { rd, rights }) {
            Reply::Ready(Ok(Out::Handle(handle))) => Ok(handle),
            Reply::Ready(Err(error)) => Err(error),
            Reply::Ready(Ok(_)) | Reply::Pending(_) => Err(Error::Unsupported),
        }
    }

    fn submit(&self, op: Op) -> Reply<Out> {
        self.kernel.run(self.caller(), op)
    }
}

/// The cookie the Driver answered written into the slot its `open`
/// made, and the caller's own handle in its place. A refusal, a wrong
/// shape, or a handle that was closed meanwhile takes the slot with it.
fn opened(s: &mut State, h: Handle, outcome: Outcome) -> Outcome {
    let Some(slot) = s.handles.get_mut(&h) else {
        return Err(Error::Gone);
    };
    match outcome {
        Ok(Out::Handle(cookie)) => {
            slot.cookie = cookie.0;
            slot.opening = false;
            Ok(Out::Handle(h))
        }
        answer => {
            s.handles.remove(&h);
            Err(answer.err().unwrap_or(Error::Invalid))
        }
    }
}

/// `register` is allowed under this Module's own rds, its home, and any
/// Kernel Container flagged WRITE.
fn check_parent(s: &State, parent: Rd, module: ModuleId) -> Result<(), Error> {
    let entry = s.table.get(parent)?;
    if !entry.flags.contains(Flags::CONTAINER) {
        return Err(Error::Invalid);
    }
    let allowed = match entry.owner {
        Owner::Driver { module: m, .. } | Owner::Kernel(Node::Home(m)) => {
            m == module
        }
        Owner::Kernel(Node::Container) => entry.flags.contains(Flags::WRITE),
        Owner::Kernel(_) => false,
    };
    if allowed { Ok(()) } else { Err(Error::Denied) }
}

/// The entry when this Module serves it.
fn owned(
    s: &State,
    rd: Rd,
    module: ModuleId,
) -> Result<&crate::table::Entry, Error> {
    let entry = s.table.get(rd)?;
    match entry.owner {
        Owner::Driver { module: m, .. } if m == module => Ok(entry),
        _ => Err(Error::Denied),
    }
}

/// A report is checked in full before any declaration is stored.
fn check_decls(s: &State, apis: &[Decl]) -> Result<(), Error> {
    let mut names = BTreeMap::new();
    for decl in apis {
        let hash = check_decl(s, decl)?;
        let key = (&decl.api, decl.ver);
        if names.insert(key, hash).is_some_and(|known| known != hash) {
            return Err(Error::Invalid);
        }
    }
    Ok(())
}

/// Validate identity before publishing any part of a report.
fn check_decl(s: &State, decl: &Decl) -> Result<Hash, Error> {
    decl.check()?;
    let hash = decl.hash();
    if let Some(known) = s.decls.get(&hash)
        && known.canonical() != decl.canonical()
    {
        return Err(Error::Invalid);
    }
    let version = alloc::format!("{}", decl.ver);
    let named = s.table.child(APIS, &decl.api);
    if named.is_none() {
        s.table.check_name(APIS, Some(&decl.api))?;
    }
    let known = named.and_then(|rd| s.table.child(rd, &version));
    if let Some(rd) = known
        && s.table.get(rd)?.owner != Owner::Kernel(Node::Decl(hash))
    {
        return Err(Error::Invalid);
    }
    Ok(hash)
}

/// Store once and publish by hash and by api name and version.
fn store_decl(s: &mut State, decl: &Decl, now: u64) -> Result<Hash, Error> {
    let hash = check_decl(s, decl)?;
    if s.decls.contains_key(&hash) {
        return Ok(hash);
    }
    s.decls.insert(hash, decl.clone());
    let name = alloc::format!("{:016x}", hash.0);
    let owner = Owner::Kernel(Node::Decl(hash));
    let rd = s.table.insert(DECLS, Some(&name), owner, Flags::READ, now);
    s.emit(DECLS, Kind::Create, Some(rd));
    let api = if let Some(rd) = s.table.child(APIS, &decl.api) {
        rd
    } else {
        let owner = Owner::Kernel(Node::Container);
        let rd = s.table.insert(APIS, Some(&decl.api), owner, CONTAINER, now);
        s.emit(APIS, Kind::Create, Some(rd));
        rd
    };
    let version = alloc::format!("{}", decl.ver);
    let rd = s.table.insert(api, Some(&version), owner, Flags::READ, now);
    s.emit(api, Kind::Create, Some(rd));
    Ok(hash)
}
