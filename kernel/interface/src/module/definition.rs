use super::{Declaration, Module, ModuleContext};
use crate::{Error, caller::Caller};
use alloc::{boxed::Box, rc::Rc};

/// Admitted construction paired with immutable package/export metadata.
/// Composition supplies granted Caller/Registrar contexts at each activation.
pub struct Definition {
    declaration: Declaration,
    required: Box<[Box<[u8]>]>,
    exports: usize,
    construct: Rc<dyn Fn(ModuleContext<Caller>) -> Result<Module, Error>>,
}

impl Definition {
    /// Pairs metadata with an inert constructor retaining its implementation.
    /// # Errors
    /// Rejects invalid export bounds, oversized paths or duplicate paths.
    pub fn new(
        declaration: Declaration,
        required: Box<[Box<[u8]>]>,
        exports: usize,
        construct: Rc<dyn Fn(ModuleContext<Caller>) -> Result<Module, Error>>,
    ) -> Result<Self, Error> {
        validate(&required, exports)?;
        Ok(Self { declaration, required, exports, construct })
    }

    #[must_use]
    pub const fn declaration(&self) -> &Declaration {
        &self.declaration
    }
    #[must_use]
    pub fn required(&self) -> &[Box<[u8]>] {
        &self.required
    }
    #[must_use]
    pub const fn exports(&self) -> usize {
        self.exports
    }

    /// Constructs only the selected instance, outside registry guards.
    /// # Errors
    /// Inert rejection leaves no external effects or retained input context.
    pub fn construct(
        &self,
        context: ModuleContext<Caller>,
    ) -> Result<Module, Error> {
        (self.construct)(context)
    }
}

fn validate(required: &[Box<[u8]>], exports: usize) -> Result<(), Error> {
    if required.len() > exports || exports > 256 {
        return Err(Error::INVALID);
    }
    for (index, path) in required.iter().enumerate() {
        if path.len() > 4096 {
            return Err(Error::PRESSURE);
        }
        if required[..index].contains(path) {
            return Err(Error::CONFLICT);
        }
    }
    Ok(())
}
