//! Inert same-build module ownership, supplied by composition.

mod factory;
pub use factory::Factory;

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{any::Any, task::Context};

use crate::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    driver::{DriverOps, Lifecycle, Progress},
    provider::Contribution,
};

/// Observes readiness of this exact instance without starting another one.
/// # Errors
/// Retains ordinary request recovery; readiness grants no lifetime lease.
#[allow(clippy::future_not_send)] // Connections belong to one executor.
pub async fn ready(
    connection: &Connection,
    instance: ResourceId,
) -> Result<(), RequestError> {
    let contract = Contract::new("kernel.module.ready", 1);
    let reply =
        connection.read(instance, &Message::new(contract.clone(), &[])).await?;
    if reply.contract != contract
        || reply.data.as_ref() != b"ready"
        || !reply.resources.is_empty()
    {
        return Err(RequestError::Failed(Error::PROVIDER));
    }
    Ok(())
}

/// Activates or joins the exact factory selected by this catalog Resource.
/// # Errors
/// Ordinary request failures/recovery apply; success returns a ready instance.
#[allow(clippy::future_not_send)] // Connections belong to one executor.
pub async fn activate(
    connection: &Connection,
    catalog: ResourceId,
) -> Result<ResourceId, RequestError> {
    let contract = Contract::new("kernel.module.activate", 1);
    let reply =
        connection.write(catalog, &Message::new(contract.clone(), &[])).await?;
    match reply.resources.as_ref() {
        [instance] if reply.contract == contract && reply.data.is_empty() => {
            Ok(*instance)
        }
        _ => Err(RequestError::Failed(Error::PROVIDER)),
    }
}

/// Unloads this exact instance through its separately granted write access.
/// # Errors
/// Busy preserves live contributions before teardown. Accepted cleanup and its
/// result remain owned by Runtime across cancellation or a disconnected wait.
#[allow(clippy::future_not_send)] // Connections belong to one executor.
pub async fn unload(
    connection: &Connection,
    instance: ResourceId,
) -> Result<(), RequestError> {
    let contract = Contract::new("kernel.module.unload", 1);
    let reply = connection
        .write(instance, &Message::new(contract.clone(), &[]))
        .await?;
    if reply.contract != contract
        || !reply.data.is_empty()
        || !reply.resources.is_empty()
    {
        return Err(RequestError::Failed(Error::PROVIDER));
    }
    Ok(())
}

/// Withdraws a catalog entry after its selected instance finishes cleanup.
/// # Errors
/// Requires catalog write permission; a live selection returns Busy. A later
/// registration at the same path has a different identity and grants.
#[allow(clippy::future_not_send)] // Connections belong to one executor.
pub async fn withdraw(
    connection: &Connection,
    catalog: ResourceId,
) -> Result<(), RequestError> {
    let contract = Contract::new("kernel.module.withdraw", 1);
    let reply =
        connection.write(catalog, &Message::new(contract.clone(), &[])).await?;
    if reply.contract != contract
        || !reply.data.is_empty()
        || !reply.resources.is_empty()
    {
        return Err(RequestError::Failed(Error::PROVIDER));
    }
    Ok(())
}

trait Drive {
    fn status(&self) -> Lifecycle;
    fn advance(&mut self, wake: &mut Context<'_>) -> Result<Progress, Error>;
    fn close(&mut self) -> Result<(), Error>;
}

struct Implementation<State> {
    state: State,
    driver: DriverOps<State>,
}
impl<State> Drive for Implementation<State> {
    fn status(&self) -> Lifecycle {
        (self.driver.status)(&self.state)
    }
    fn advance(&mut self, wake: &mut Context<'_>) -> Result<Progress, Error> {
        (self.driver.advance)(&mut self.state, wake)
    }
    fn close(&mut self) -> Result<(), Error> {
        (self.driver.close)(&mut self.state)
    }
}

/// An inert instance and its complete provisional contributions.
///
/// Composition supplies granted imports inside State. Runtime alone drives
/// activation, publishes the contributions and retains unfinished cleanup.
/// This ownership carrier is same-build Rust, not a native ABI table.
pub struct Module {
    pub contributions: Vec<Contribution>,
    implementation: Box<dyn Drive>,
    code: Rc<dyn Any>,
}

impl Module {
    #[must_use]
    pub fn new<State: 'static>(
        state: State,
        driver: DriverOps<State>,
        contributions: Vec<Contribution>,
        code: Rc<dyn Any>,
    ) -> Self {
        Self {
            contributions,
            implementation: Box::new(Implementation { state, driver }),
            code,
        }
    }

    #[must_use]
    pub fn status(&self) -> Lifecycle {
        self.implementation.status()
    }

    /// # Errors
    /// Driver failure retains the instance and its cleanup obligations.
    pub fn advance(
        &mut self,
        wake: &mut Context<'_>,
    ) -> Result<Progress, Error> {
        self.implementation.advance(wake)
    }

    /// # Errors
    /// Busy retains ownership; the execution owner must continue driving.
    pub fn close(&mut self) -> Result<(), Error> {
        self.implementation.close()
    }
}
