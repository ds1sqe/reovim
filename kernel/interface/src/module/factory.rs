use alloc::{boxed::Box, rc::Rc};
use core::any::Any;

use super::Module;
use crate::{Contract, Error, ResourceId};

/// Inert imports and a constructor selected by composition.
///
/// Construction creates inert private state; external initialization belongs
/// to the instance driver. Rejection must leave no effect or installed export.
/// Captured imports are granted access, not a driver for the owning Runtime.
#[derive(Clone)]
pub struct Factory {
    /// Exact required catalog identities; resolution precedes construction.
    pub dependencies: Box<[Contract]>,
    construct: Rc<dyn Fn(ResourceId) -> Result<Module, Error>>,
    code: Rc<dyn Any>,
}

impl Factory {
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
    pub fn construct(&self, instance: ResourceId) -> Result<Module, Error> {
        let mut module = (self.construct)(instance)?;
        // Retain factory executable ownership through instance destruction.
        module.code = Rc::new((Rc::clone(&module.code), Rc::clone(&self.code)));
        Ok(module)
    }
}
