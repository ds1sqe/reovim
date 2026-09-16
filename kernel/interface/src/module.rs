//! Module construction and private execution, supplied through Interface.
//!
//! Example: composition constructs an inert module with granted imports and a
//! Registrar; Runtime drives it and publishes staged exports only after Ready.

pub mod activation;
pub mod control;
mod declaration;
mod definition;
mod import;
pub use declaration::{Declaration, Dependency, DependencyScope, ModuleId};
pub use definition::Definition;
pub use import::Import;

use alloc::{boxed::Box, rc::Rc};
use core::{any::Any, task::Context};

use crate::{
    Error, ResourceId,
    driver::{DriverOps, Lifecycle, Progress},
    provider::Registrar,
};

/// Instance state scope; for example, Global is shared only in this Runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleScope {
    /// One shared instance within its owning Kernel Runtime.
    Global,
    /// State in an authorized Session, identified by its Resource identity.
    Session(ResourceId),
}

/// Granted activation context; IDs alone convey no operation rights.
/// Example: imports hold a Caller while exports stage below the bound mount.
pub struct ModuleContext<Imports> {
    /// Exact instance Resource; replacement never reuses this identity.
    pub instance: ResourceId,
    /// Granted state scope, e.g. Global.
    pub scope: ModuleScope,
    /// Actual namespace Resource selected by composition.
    pub namespace: ResourceId,
    /// Module-defined granted services, e.g. a Caller and typed API helpers.
    pub imports: Imports,
    /// Provisional exports; retaining a clone cannot reopen activation.
    pub exports: Registrar,
}

/// Inert constructor; rejection returns the unchanged context without effects.
/// The error keeps context inline so rejection requires no allocation.
pub type Construct<State, Imports> =
    fn(
        ModuleContext<Imports>,
    ) -> Result<State, (ModuleContext<Imports>, Error)>;

/// Callbacks for constructing and driving one module instance.
///
/// State is private instance data; Imports contains granted capabilities.
/// For example, construct stores a Registrar, advance stages exports, and
/// close releases the module's work before reporting Closed.
pub struct ModuleImplementation<State, Imports> {
    /// Creates private state without external effects.
    /// Rejection returns the supplied context to its owner.
    pub construct: Construct<State, Imports>,
    /// Inspects lifecycle, advances work, and closes this instance.
    pub driver: DriverOps<State>,
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

/// Owned state, driver and code; for example, Runtime retains this during Busy.
/// Dropping unfinished native work cannot prove teardown, so it retains owners.
pub struct Module {
    implementation: Option<Box<dyn Drive>>,
    code: Rc<dyn Any>,
    closed: bool,
}

impl Module {
    /// Constructs outside registry guards using the selected implementation.
    /// # Errors
    /// Inert rejection returns its cause and releases the unchanged context.
    pub fn construct<State: 'static, Imports>(
        implementation: ModuleImplementation<State, Imports>,
        context: ModuleContext<Imports>,
        code: Rc<dyn Any>,
    ) -> Result<Self, Error> {
        let state = (implementation.construct)(context).map_err(|(_, e)| e)?;
        Ok(Self {
            implementation: Some(Box::new(Implementation {
                state,
                driver: implementation.driver,
            })),
            code,
            closed: false,
        })
    }

    /// Observes the exact instance, e.g. Ready does not itself publish exports.
    #[must_use]
    pub fn status(&self) -> Lifecycle {
        self.implementation.as_ref().map_or(Lifecycle::Closed, |i| i.status())
    }

    /// Drives one bounded turn outside registry guards.
    /// # Errors
    /// Failure retains cleanup ownership. Stopped without Closed is invalid.
    pub fn advance(
        &mut self,
        wake: &mut Context<'_>,
    ) -> Result<Progress, Error> {
        if self.closed {
            return Ok(Progress::Stopped);
        }
        let progress =
            self.implementation.as_mut().ok_or(Error::CLOSED)?.advance(wake)?;
        let lifecycle = self.status();
        if progress == Progress::Stopped && lifecycle != Lifecycle::Closed {
            return Err(Error::PROVIDER);
        }
        self.closed = lifecycle == Lifecycle::Closed;
        Ok(progress)
    }

    /// Begins or joins teardown; for example, Busy requires further driving.
    /// # Errors
    /// Failure keeps state/code; OK requires observed Closed before disposal.
    pub fn close(&mut self) -> Result<(), Error> {
        if self.closed {
            return Ok(());
        }
        self.implementation.as_mut().ok_or(Error::CLOSED)?.close()?;
        self.closed = self.status() == Lifecycle::Closed;
        if self.closed { Ok(()) } else { Err(Error::BUSY) }
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        if !self.closed {
            core::mem::forget(self.implementation.take());
            core::mem::forget(self.code.clone());
        }
    }
}
