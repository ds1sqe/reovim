use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::any::Any;
use reovim_kernel_interface::{
    Error, abi,
    caller::Caller,
    declaration::ValueLimits,
    module::{Declaration, ModuleImplementation},
};

use crate::raw;
mod declaration;
mod instance;

struct Definition {
    raw: abi::Module,
    declaration: Declaration,
    required: Box<[Box<[u8]>]>,
    _code: Rc<dyn Any>,
}

/// Admitted native module metadata, e.g. an already mapped `rv_module_entry`.
/// Import copies metadata and constructs no live instance.
#[derive(Clone)]
pub struct NativeModule {
    definition: Rc<Definition>,
}

/// Granted inputs for one native construction; e.g. distinct Callers per scope.
pub struct ModuleImports {
    module: NativeModule,
    caller: Caller,
    limits: ValueLimits,
    views: usize,
}

impl NativeModule {
    /// Admits the descriptor returned by the common module entry.
    /// # Errors
    /// Bad extents/version, invalid bounds or missing constructor reject.
    /// # Safety
    /// The pointer and nested spans are aligned/readable for their extents.
    /// Code retains descriptor storage and callbacks. Native calls obey their
    /// ownership contracts, execute serialized, and never unwind across ABI.
    pub unsafe fn import(
        pointer: *const abi::Module,
        code: Rc<dyn Any>,
    ) -> Result<Self, Error> {
        // SAFETY: The loader supplies the retained readable descriptor;
        // table verifies its exact size before copying callable fields.
        let descriptor = unsafe { raw::table(pointer)? };
        if descriptor.version != abi::VERSION {
            return Err(Error::UNSUPPORTED);
        }
        if descriptor.construct.is_none()
            || descriptor.export_limit > 256
            || descriptor.required_count > descriptor.export_limit
        {
            return Err(Error::INVALID);
        }
        // SAFETY: The artifact retains readable nested metadata spans;
        // admission validates bounded fields before copying their contents.
        let declaration = unsafe { declaration::import(&descriptor)? };
        // SAFETY: The artifact lease retains the immutable span array;
        // elements validates nullness and the required-path count bound.
        let paths = unsafe {
            raw::elements(descriptor.required, descriptor.required_count, 256)?
        };
        let mut required = Vec::new();
        required.try_reserve_exact(paths.len()).map_err(|_| Error::PRESSURE)?;
        for path in paths {
            // SAFETY: Nested required-path bytes satisfy the same descriptor
            // storage contract; admission bounds each path before copying.
            let path = unsafe { raw::elements(path.ptr, path.len, 4096)? };
            let mut owned = Vec::new();
            owned.try_reserve_exact(path.len()).map_err(|_| Error::PRESSURE)?;
            owned.extend_from_slice(path);
            required.push(owned.into_boxed_slice());
        }
        Ok(Self {
            definition: Rc::new(Definition {
                raw: descriptor,
                declaration,
                required: required.into_boxed_slice(),
                _code: code,
            }),
        })
    }

    /// Passive copied package identity, revision, scopes and dependencies.
    #[must_use]
    pub fn declaration(&self) -> &Declaration {
        &self.definition.declaration
    }

    /// Pairs this artifact's metadata with its inert native constructor.
    /// The returned Interface definition retains code without constructing it.
    /// # Errors
    /// Invalid export bounds or required paths reject before execution.
    pub fn definition(
        &self,
        limits: ValueLimits,
        views: usize,
    ) -> Result<reovim_kernel_interface::module::Definition, Error> {
        let module = self.clone();
        reovim_kernel_interface::module::Definition::new(
            self.declaration().clone(),
            self.required().to_vec().into_boxed_slice(),
            self.export_limit(),
            Rc::new(move |context| {
                let context = reovim_kernel_interface::module::ModuleContext {
                    instance: context.instance,
                    namespace: context.namespace,
                    scope: context.scope,
                    imports: module.imports(context.imports, limits, views),
                    exports: context.exports,
                };
                reovim_kernel_interface::module::Module::construct(
                    super::module_implementation(),
                    context,
                    Rc::new(module.clone()),
                )
            }),
        )
    }

    /// Required relative paths, e.g. used to reserve an activation batch.
    #[must_use]
    pub fn required(&self) -> &[Box<[u8]>] {
        &self.definition.required
    }

    /// Maximum staged exports, including optional ones; zero is permitted.
    #[must_use]
    pub fn export_limit(&self) -> usize {
        self.definition.raw.export_limit
    }

    /// Captures granted caller access without invoking native construction.
    /// Example: `imports(caller, limits, 64)` budgets 64 native views.
    #[must_use]
    pub fn imports(
        &self,
        caller: Caller,
        limits: ValueLimits,
        views: usize,
    ) -> ModuleImports {
        ModuleImports { module: self.clone(), caller, limits, views }
    }
}

/// The common adapter table for admitted native modules.
/// Runtime activates this table with `NativeModule::imports` and drives its
/// opaque state; no native module type enters Runtime's dependencies.
#[must_use]
pub fn module_implementation()
-> ModuleImplementation<impl Sized + 'static, ModuleImports> {
    instance::implementation()
}
