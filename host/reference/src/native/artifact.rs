//! Passive catalog metadata retained with the exact loaded executable.

use super::{Library, contract, contributions, load_table};
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::slice;
use reovim_kernel_interface::{
    Connection, Contract, Error, ResourceId, abi,
    driver::{DriverOps, Lifecycle, Progress},
    module::{Factory, Module},
    provider::Contribution,
};
use std::path::Path;

/// One loaded artifact, before any module instance has been constructed.
#[derive(Clone)]
pub struct Artifact {
    identity: Contract,
    dependencies: Box<[Contract]>,
    table: abi::Module,
    library: Rc<Library>,
}

impl Artifact {
    /// Opens and copies passive contract declarations without calling create.
    /// # Safety
    /// The artifact obeys ABI 0.1.0 ownership and no-unwind rules. Library
    /// constructors and entry code run during discovery and must be trusted.
    /// # Errors
    /// Rejects incompatible ABI, unnamed identity or malformed dependencies.
    pub unsafe fn open(path: &Path) -> Result<Self, Error> {
        let (table, library) = unsafe { load_table(path)? };
        let identity = unsafe { contract(table.identity)? };
        let dependencies = unsafe { dependencies(&table)? };
        Ok(Self { identity, dependencies, table, library })
    }

    #[must_use]
    pub const fn identity(&self) -> &Contract {
        &self.identity
    }

    #[must_use]
    pub fn dependencies(&self) -> &[Contract] {
        &self.dependencies
    }

    /// Binds this artifact to the existing catalog's dependency lifecycle.
    #[must_use]
    pub fn factory(&self, mount: &str, imports: Connection) -> Factory {
        let mut factory = Factory::new(
            (self.clone(), Box::<str>::from(mount), imports),
            construct_module,
            self.library.clone(),
        );
        factory.dependencies.clone_from(&self.dependencies);
        factory
    }

    /// Constructs using this retained executable, without reopening its path.
    /// Assembly must resolve the required contracts before calling directly.
    /// # Errors
    /// Returns metadata or inert-construction failure without publication.
    pub fn construct(
        &self,
        mount: &str,
        imports: Option<&Connection>,
    ) -> Result<Vec<Contribution>, Error> {
        unsafe {
            contributions(&self.table, self.library.clone(), mount, imports)
        }
    }
}

fn construct_module(
    bound: &(Artifact, Box<str>, Connection),
    _: ResourceId,
) -> Result<Module, Error> {
    let contributions = bound.0.construct(&bound.1, Some(&bound.2))?;
    Ok(Module::new(
        Lifecycle::Constructed,
        DriverOps {
            status: |state| *state,
            advance: |state, _| {
                if *state == Lifecycle::Closed {
                    return Ok(Progress::Stopped);
                }
                *state = Lifecycle::Ready;
                Ok(Progress::Idle)
            },
            close: |state| {
                *state = Lifecycle::Closed;
                Ok(())
            },
        },
        contributions,
        bound.0.library.clone(),
    ))
}

unsafe fn dependencies(table: &abi::Module) -> Result<Box<[Contract]>, Error> {
    if table.dependency_count == 0 {
        return Ok(Box::new([]));
    }
    if table.dependencies.is_null() || table.dependency_count > 256 {
        return Err(Error::INVALID);
    }
    let declarations = unsafe {
        slice::from_raw_parts(table.dependencies, table.dependency_count)
    };
    declarations.iter().map(|value| unsafe { contract(*value) }).collect()
}
