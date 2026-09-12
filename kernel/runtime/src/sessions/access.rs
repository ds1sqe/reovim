use crate::namespace::Kind;
use alloc::boxed::Box;
use reovim_kernel_interface::{
    Contract, Error, Message, access::Method, provider::Permissions,
};

pub fn closing(contract: &Contract) -> bool {
    contract == &Contract::new("kernel.session.close", 1)
}

pub fn control(method: Method, contract: &Contract) -> bool {
    contract.version == 1
        && matches!(
            (method, contract.name.as_ref()),
            (Method::Read, "kernel.session.observe" | "kernel.session.attach")
                | (
                    Method::Write,
                    "kernel.session.close"
                        | "kernel.session.seek"
                        | "kernel.session.detach"
                )
        )
}

pub fn surface(
    kind: &Kind,
    method: Method,
    input: &Message,
) -> Result<Box<[Permissions]>, Error> {
    if matches!(kind, Kind::Session(_) | Kind::ClosedSession { .. })
        && method == Method::Write
        && closing(&input.contract)
    {
        return Ok(Box::new([]));
    }
    match kind {
        Kind::Participation(participation) => super::participation::surface(
            &participation.borrow(),
            method,
            input,
        ),
        Kind::Session(_)
            if method == Method::Read
                && super::participation::attaching(&input.contract) =>
        {
            Ok(Box::new([Permissions::ALL]))
        }
        Kind::Session(_)
            if method == Method::Read
                && super::watch::contract(&input.contract) =>
        {
            Ok(Box::new([
                Permissions::READ,
                Permissions::ALL,
                Permissions::READ,
            ]))
        }
        Kind::Session(session)
            if method == Method::Read
                && input.contract == session.borrow().model.borrow().read =>
        {
            Ok(Box::new([Permissions::READ, Permissions::ALL]))
        }
        Kind::Session(session)
            if method == Method::Write
                && input.contract == session.borrow().model.borrow().edit =>
        {
            Ok(Box::new([Permissions::WRITE, Permissions::ALL]))
        }
        Kind::Snapshot(snapshot)
            if method == Method::Read
                && input.contract == snapshot.value.message.contract =>
        {
            Ok(snapshot.value.rights.clone())
        }
        Kind::Snapshot(_)
            if method == Method::Write
                && input.contract
                    == Contract::new("kernel.session.release", 1) =>
        {
            Ok(Box::new([]))
        }
        Kind::ClosedSession { .. } => Err(Error::CLOSED),
        _ => Err(Error::UNSUPPORTED),
    }
}

pub fn read_contract(
    session: &alloc::rc::Rc<core::cell::RefCell<super::Session>>,
) -> Contract {
    session.borrow().model.borrow().read.clone()
}
