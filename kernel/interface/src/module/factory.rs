use alloc::{boxed::Box, rc::Rc};
use core::any::Any;

use super::Module;
use crate::{Contract, Error, ResourceId};

/// Inert imports and a constructor selected by composition.
///
/// Construction creates inert private state; external initialization belongs
/// to the instance driver. Rejection must leave no effect or installed export.
/// Captured imports are granted access, not a driver for the owning Runtime.
///
/// ```
/// use std::rc::Rc;
/// use reovim_kernel_interface::{Error, ResourceId, module::Factory};
/// let factory = Factory::new(false, |allowed, _| {
///     assert!(!allowed);
///     Err(Error::DENIED)
/// }, Rc::new(()));
/// assert!(matches!(factory.construct(ResourceId(1)), Err(Error::DENIED)));
/// ```
#[derive(Clone)]
pub struct Factory {
    /// Exact required catalog identities; resolution precedes construction.
    /// ```
    /// use reovim_kernel_interface::{Contract, module::Factory};
    /// fn require_text(factory: &mut Factory) {
    ///     factory.dependencies = vec![Contract::new("text", 1)].into();
    /// }
    /// ```
    pub dependencies: Box<[Contract]>,
    construct: Rc<dyn Fn(ResourceId) -> Result<Module, Error>>,
    code: Rc<dyn Any>,
}

impl Factory {
    /// Captures imports and code without calling the inert constructor.
    ///
    /// ```
    /// use std::{cell::Cell, rc::Rc};
    /// use reovim_kernel_interface::{Error, ResourceId, module::Factory};
    /// let invoked = Rc::new(Cell::new(false));
    /// let factory = Factory::new(invoked.clone(), |invoked, _| {
    ///     invoked.set(true);
    ///     Err(Error::DENIED)
    /// }, Rc::new(()));
    /// assert!(!invoked.get());
    /// assert!(matches!(factory.construct(ResourceId(1)), Err(Error::DENIED)));
    /// assert!(invoked.get());
    /// ```
    #[must_use]
    pub fn new<Imports: 'static>(
        imports: Imports,
        construct: fn(&Imports, ResourceId) -> Result<Module, Error>,
        code: Rc<dyn Any>,
    ) -> Self {
        Self {
            dependencies: Box::new([]),
            construct: Rc::new(move |instance| construct(&imports, instance)),
            code,
        }
    }

    /// Called by the activation owner after admission, outside registry guards.
    /// # Errors
    /// A rejected inert construction installs no contribution.
    ///
    /// ```
    /// use std::rc::Rc;
    /// use reovim_kernel_interface::{Error, ResourceId, module::Factory};
    /// let factory = Factory::new((), |_, _| Err(Error::INVALID), Rc::new(()));
    /// let result = factory.construct(ResourceId(2));
    /// assert!(matches!(result, Err(Error::INVALID)));
    /// ```
    pub fn construct(&self, instance: ResourceId) -> Result<Module, Error> {
        let mut module = (self.construct)(instance)?;
        // Retain factory executable ownership through instance destruction.
        module.code = Rc::new((Rc::clone(&module.code), Rc::clone(&self.code)));
        Ok(module)
    }
}
