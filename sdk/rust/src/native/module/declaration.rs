use crate::raw;
use alloc::vec::Vec;
use reovim_kernel_interface::{
    Error, abi,
    module::{Declaration, Dependency, DependencyScope, Import, ModuleId},
};

pub(super) unsafe fn import(
    native: &abi::Module,
) -> Result<Declaration, Error> {
    // SAFETY: The loader retains both bounded immutable spans with the
    // artifact;
    // elements checks nullness and maximum extents before exposing their data.
    let name = unsafe { raw::elements(native.name.ptr, native.name.len, 255)? };
    // SAFETY: The same artifact promise retains aligned dependency records.
    let dependencies = unsafe {
        raw::elements(native.dependencies, native.dependency_count, 256)?
    };
    let count =
        dependencies.iter().try_fold(0_usize, |count, dependency| {
            count.checked_add(dependency.import_count).ok_or(Error::PRESSURE)
        })?;
    if count > 256 {
        return Err(Error::PRESSURE);
    }
    let mut owned = Vec::new();
    owned.try_reserve_exact(dependencies.len()).map_err(|_| Error::PRESSURE)?;
    for dependency in dependencies {
        // SAFETY: The artifact promise includes each dependency's nested spans.
        owned.push(unsafe { decode(*dependency)? });
    }
    Declaration::new(
        ModuleId(native.id),
        core::str::from_utf8(name).map_err(|_| Error::INVALID)?,
        native.revision,
        native.scopes,
        owned.into_boxed_slice(),
    )
}

unsafe fn decode(
    dependency: abi::ModuleDependency,
) -> Result<Dependency, Error> {
    let scope = match dependency.scope {
        0 => DependencyScope::Same,
        1 => DependencyScope::Global,
        _ => return Err(Error::INVALID),
    };
    if dependency.optional > 1 {
        return Err(Error::INVALID);
    }
    Ok(Dependency {
        module: ModuleId(dependency.module),
        revision: dependency.revision,
        scope,
        optional: dependency.optional == 1,
        // SAFETY: The caller retains all bounded artifact-owned import records.
        imports: unsafe { imports(dependency)? },
    })
}

unsafe fn imports(
    dependency: abi::ModuleDependency,
) -> Result<alloc::boxed::Box<[Import]>, Error> {
    // SAFETY: Artifact admission promises aligned immutable records for this
    // bounded span and valid nested byte spans through the copy below.
    let records = unsafe {
        raw::elements(dependency.imports, dependency.import_count, 256)?
    };
    let mut imports = Vec::new();
    imports.try_reserve_exact(records.len()).map_err(|_| Error::PRESSURE)?;
    for record in records {
        // SAFETY: The retained artifact covers this record's spans.
        imports.push(unsafe { import_record(record)? });
    }
    Ok(imports.into_boxed_slice())
}
unsafe fn import_record(record: &abi::ModuleImport) -> Result<Import, Error> {
    if record.retain_loans > 1 {
        return Err(Error::INVALID);
    }
    // SAFETY: The artifact retains this bounded immutable name span.
    let name = unsafe { raw::elements(record.name.ptr, record.name.len, 255)? };
    // SAFETY: The artifact retains this bounded relative path span.
    let path = unsafe {
        raw::elements(record.export_path.ptr, record.export_path.len, 4096)?
    };
    // SAFETY: The artifact retains the complete bounded declaration encoding.
    let encoded = unsafe {
        raw::elements(record.declaration.ptr, record.declaration.len, 65536)?
    };
    Import::new(
        core::str::from_utf8(name).map_err(|_| Error::INVALID)?,
        path,
        reovim_kernel_interface::declaration::Declaration::decode(encoded)?,
        record.access,
        record.create_access,
        record.retain_loans == 1,
    )
}
