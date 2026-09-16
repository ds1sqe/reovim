//! The seven calls, bound to one Module.

use reovim_kernel_interface::{
    Caller, Calls, Decl, DriverId, Error, Flags, Handle, Hash, Kind, ModuleId,
    Op, Out, Outcome, Rd, Reply, Rights, Token,
};

use crate::kernel::{Kernel, State};
use crate::table::{DECLS, Node, Owner};

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
        let handle = s.pending.done(self.module, token.0, outcome)?;
        if let Some(slot) = s.handles.get_mut(&handle) {
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

/// Check, store once, publish at `/sys/decls/<hash>`; refuse a collision.
fn store_decl(s: &mut State, decl: &Decl, now: u64) -> Result<Hash, Error> {
    decl.check()?;
    let hash = decl.hash();
    if let Some(known) = s.decls.get(&hash) {
        return if known.canonical() == decl.canonical() {
            Ok(hash)
        } else {
            Err(Error::Invalid)
        };
    }
    s.decls.insert(hash, decl.clone());
    let name = alloc::format!("{:016x}", hash.0);
    let owner = Owner::Kernel(Node::Decl(hash));
    let rd = s.table.insert(DECLS, Some(&name), owner, Flags::READ, now);
    s.emit(DECLS, Kind::Create, Some(rd));
    Ok(hash)
}
