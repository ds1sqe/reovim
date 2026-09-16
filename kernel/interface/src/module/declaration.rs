use super::Import;
use crate::Error;
use alloc::{boxed::Box, string::String};

/// Stable package identity, distinct from API and live Resource identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModuleId(pub [u8; 16]);

/// Required dependency state owner, relative to the requesting instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyScope {
    /// Use the requester's Global or exact Session scope.
    Same,
    /// Use an explicitly shared Global instance.
    Global,
}

/// Exact dependency selection; optional absence requires module-defined
/// fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dependency {
    pub module: ModuleId,
    pub revision: u32,
    pub scope: DependencyScope,
    pub optional: bool,
    /// Exact requested exports; an absent optional dependency supplies none.
    pub imports: Box<[Import]>,
}

/// Validated passive package metadata; construction and grants are separate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    id: ModuleId,
    name: Box<str>,
    revision: u32,
    scopes: u32,
    dependencies: Box<[Dependency]>,
}

impl Declaration {
    /// Owns passive dependencies and copies the name without construction.
    /// Scope flags are Global=1 and Session=2; neither grants access.
    /// # Errors
    /// Rejects zero identities/revisions, invalid ASCII names, unknown scope
    /// bits, more than 256 dependencies/imports, duplicate package/scope
    /// selections or duplicate local import names.
    pub fn new(
        id: ModuleId,
        name: &str,
        revision: u32,
        scopes: u32,
        dependencies: Box<[Dependency]>,
    ) -> Result<Self, Error> {
        if id.0 == [0; 16] || revision == 0 || scopes == 0 || scopes & !3 != 0 {
            return Err(Error::INVALID);
        }
        validate(name, &dependencies)?;
        validate_imports(&dependencies)?;
        let mut owned_name = String::new();
        owned_name
            .try_reserve_exact(name.len())
            .map_err(|_| Error::PRESSURE)?;
        owned_name.push_str(name);
        Ok(Self {
            id,
            name: owned_name.into_boxed_str(),
            revision,
            scopes,
            dependencies,
        })
    }

    #[must_use]
    pub const fn id(&self) -> ModuleId {
        self.id
    }
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub const fn revision(&self) -> u32 {
        self.revision
    }
    #[must_use]
    pub const fn scopes(&self) -> u32 {
        self.scopes
    }
    #[must_use]
    pub fn dependencies(&self) -> &[Dependency] {
        &self.dependencies
    }
}

fn validate(name: &str, dependencies: &[Dependency]) -> Result<(), Error> {
    if name.is_empty() || !name.is_ascii() {
        return Err(Error::INVALID);
    }
    if name.len() > 255 || dependencies.len() > 256 {
        return Err(Error::PRESSURE);
    }
    for (index, dependency) in dependencies.iter().enumerate() {
        if dependency.module.0 == [0; 16] || dependency.revision == 0 {
            return Err(Error::INVALID);
        }
        if dependencies[..index].iter().any(|old| {
            old.module == dependency.module && old.scope == dependency.scope
        }) {
            return Err(Error::CONFLICT);
        }
    }
    Ok(())
}

fn validate_imports(dependencies: &[Dependency]) -> Result<(), Error> {
    let count =
        dependencies.iter().try_fold(0_usize, |count, dependency| {
            count.checked_add(dependency.imports.len()).ok_or(Error::PRESSURE)
        })?;
    if count > 256 {
        return Err(Error::PRESSURE);
    }
    let mut names = alloc::vec::Vec::new();
    names.try_reserve_exact(count).map_err(|_| Error::PRESSURE)?;
    for import in dependencies.iter().flat_map(|dependency| &dependency.imports)
    {
        if names.contains(&import.name()) {
            return Err(Error::CONFLICT);
        }
        names.push(import.name());
    }
    Ok(())
}
